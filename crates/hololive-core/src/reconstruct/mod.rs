mod crop;
mod output;
mod reference;
mod settings;
mod spectrum;

use std::time::Instant;

use num_complex::Complex;
use rayon::prelude::*;
use rustfft::FftDirection;

use crate::{
    CoreError,
    dsp::{
        fft2::{Fft2, shifted_to_unshifted},
        filters,
    },
    frame::Frame,
    precision::Real,
    sideband,
    unwrap::{self, wrap_to_pi},
};

pub use crop::max_output_crop_radius;
pub use output::{
    DisplayRange, Reconstruction, ReconstructionInfo, ReconstructionMetrics, StageTimings,
};
pub use reference::ReferenceWave;
pub use settings::{DisplayMode, ReconstructionSettings};

use crop::{SidebandCrop, automatic_crop_radius};
use reference::WaveSettings;

pub struct ReconstructionEngine<T: Real> {
    forward: FftBuffers<T>,
    inverse: FftBuffers<T>,
    unwrap: unwrap::UnwrapWorkspace<T>,
    window_key: Option<(usize, u32)>,
    window: Vec<T>,
    latest_wave: Option<(WaveSettings, T)>,
    /// Detector sequence and size of [`Self::forward`] after a successful FFT.
    /// Crop, Tukey, mask, and lock changes reuse that spectrum.
    forward_spectrum: Option<(u64, usize, usize)>,
}

impl<T: Real> Default for ReconstructionEngine<T> {
    fn default() -> Self {
        Self {
            forward: FftBuffers::default(),
            inverse: FftBuffers::default(),
            unwrap: unwrap::UnwrapWorkspace::default(),
            window_key: None,
            window: Vec::new(),
            latest_wave: None,
            forward_spectrum: None,
        }
    }
}

impl<T: Real> ReconstructionEngine<T> {
    /// Live control updates on a retained detector frame.
    ///
    /// Skips the forward FFT when `frame` has the same sequence and size as the
    /// spectrum already in this engine. Crop, Tukey, mask, lock, and extract
    /// settings still run. A new sequence or a rebuilt forward buffer always
    /// transforms again.
    pub fn reconstruct_retained_into(
        &mut self,
        frame: &Frame<T>,
        settings: &ReconstructionSettings,
        reference: Option<&ReferenceWave<T>>,
        output: &mut Reconstruction<T>,
    ) -> Result<(), CoreError> {
        let total_start = Instant::now();
        let forward_fft_ms = self.ensure_forward_spectrum(frame)?;
        settings.validate()?;
        self.inverse.ensure(
            settings.output_width,
            settings.output_height,
            FftDirection::Inverse,
        )?;
        let (crop, sideband_ms) = self.locate_sideband_and_crop(frame, settings, reference)?;
        self.fill_inverse_input_from_crop(frame, settings, &crop)?;
        let (inverse_scale, inverse_fft_ms) = self.run_inverse_fft(settings)?;
        let extract_ms = self.extract_wave_values(settings, reference, inverse_scale, output)?;

        output.timings = StageTimings {
            forward_fft_ms,
            sideband_ms,
            inverse_fft_ms,
            extract_ms,
            total_ms: milliseconds(total_start),
        };
        self.fill_output_metrics(frame, settings, &crop, output);
        self.latest_wave = Some((crop.wave, inverse_scale));
        Ok(())
    }

    fn ensure_forward_spectrum(&mut self, frame: &Frame<T>) -> Result<f64, CoreError> {
        self.forward
            .ensure(frame.width(), frame.height(), FftDirection::Forward)?;
        let key = (frame.meta.sequence, frame.width(), frame.height());
        if self.forward_spectrum == Some(key) {
            return Ok(0.0);
        }
        self.forward
            .samples
            .par_iter_mut()
            .zip(frame.pixels().par_iter())
            .for_each(|(destination, &source)| *destination = Complex::new(source, T::zero()));
        let start = Instant::now();
        self.forward.process()?;
        self.forward_spectrum = Some(key);
        Ok(milliseconds(start))
    }

    fn locate_sideband_and_crop(
        &self,
        frame: &Frame<T>,
        settings: &ReconstructionSettings,
        reference: Option<&ReferenceWave<T>>,
    ) -> Result<(SidebandCrop, f64), CoreError> {
        let start = Instant::now();
        let mut sideband = if let Some(sideband) = settings.locked_sideband {
            if sideband.x >= frame.width() || sideband.y >= frame.height() {
                return Err(CoreError::SidebandOutOfBounds);
            }
            sideband
        } else {
            sideband::locate(
                &self.forward.samples,
                frame.width(),
                frame.height(),
                settings.search_region,
                settings.center_mask_percent,
            )?
        };
        let distance = sideband.distance_from_center(frame.width(), frame.height());
        let max_image_radius = max_output_crop_radius(
            frame.width(),
            frame.height(),
            settings.output_width,
            settings.output_height,
        );
        let requested = settings
            .crop_radius
            .unwrap_or_else(|| automatic_crop_radius(distance))
            .max(1);
        let fit = sideband.max_crop_radius(frame.width(), frame.height());
        let radius = requested.min(fit).min(max_image_radius);
        if settings.crop_radius.is_some() && settings.locked_sideband.is_none() && fit < requested {
            sideband = sideband.clamp_to_crop(frame.width(), frame.height(), radius);
        }
        let crop = SidebandCrop::new(sideband, radius, frame.width(), frame.height(), settings)?;
        if reference.is_some_and(|reference| {
            reference.settings != crop.wave
                || reference.samples.len() != crop.wave.output_width * crop.wave.output_height
        }) {
            return Err(CoreError::IncompatibleReference);
        }
        Ok((crop, milliseconds(start)))
    }

    fn fill_inverse_input_from_crop(
        &mut self,
        frame: &Frame<T>,
        settings: &ReconstructionSettings,
        crop: &SidebandCrop,
    ) -> Result<(), CoreError> {
        self.ensure_tukey_window(crop.side(), settings)?;
        self.inverse
            .samples
            .fill(Complex::new(T::zero(), T::zero()));
        let side = crop.side();
        let pad_x = (settings.output_width - side) / 2;
        let pad_y = (settings.output_height - side) / 2;
        let source_xs: Vec<_> = (0..side)
            .map(|crop_x| shifted_to_unshifted(crop.origin_x + crop_x, frame.width()))
            .collect();
        let dest_xs: Vec<_> = (0..side)
            .map(|crop_x| shifted_to_unshifted(pad_x + crop_x, settings.output_width))
            .collect();
        let input_width = frame.width();
        let output_width = settings.output_width;
        let tukey = !self.window.is_empty();
        for crop_y in 0..side {
            let source_y = shifted_to_unshifted(crop.origin_y + crop_y, frame.height());
            let dest_y = shifted_to_unshifted(pad_y + crop_y, settings.output_height);
            let source_row = &self.forward.samples[source_y * input_width..][..input_width];
            let dest_row = &mut self.inverse.samples[dest_y * output_width..][..output_width];
            if tukey {
                let weights = &self.window[crop_y * side..][..side];
                for crop_x in 0..side {
                    dest_row[dest_xs[crop_x]] = source_row[source_xs[crop_x]] * weights[crop_x];
                }
            } else {
                for crop_x in 0..side {
                    dest_row[dest_xs[crop_x]] = source_row[source_xs[crop_x]];
                }
            }
        }
        Ok(())
    }

    fn ensure_tukey_window(
        &mut self,
        side: usize,
        settings: &ReconstructionSettings,
    ) -> Result<(), CoreError> {
        if !settings.tukey_enabled {
            self.window.clear();
            self.window_key = None;
            return Ok(());
        }
        let key = (side, settings.tukey_alpha.to_bits());
        if self.window_key != Some(key) {
            self.window = filters::radial_tukey(side, f64::from(settings.tukey_alpha))?
                .into_iter()
                .map(<T as Real>::from_f64)
                .collect();
            self.window_key = Some(key);
        }
        Ok(())
    }

    fn run_inverse_fft(
        &mut self,
        settings: &ReconstructionSettings,
    ) -> Result<(T, f64), CoreError> {
        let start = Instant::now();
        self.inverse.process()?;
        let inverse_scale =
            T::one() / <T as Real>::from_usize(settings.output_width * settings.output_height);
        Ok((inverse_scale, milliseconds(start)))
    }

    fn extract_wave_values(
        &mut self,
        settings: &ReconstructionSettings,
        reference: Option<&ReferenceWave<T>>,
        inverse_scale: T,
        output: &mut Reconstruction<T>,
    ) -> Result<f64, CoreError> {
        let start = Instant::now();
        output.values.resize(self.inverse.samples.len(), T::zero());
        output.unwrapped_range = None;
        let wave_at = |index: usize| {
            let numerator = self.inverse.samples[index] * inverse_scale;
            if let Some(reference) = reference {
                let denominator = reference.samples[index];
                if denominator.norm_sqr() <= reference.minimum_power
                    || !denominator.re.is_finite()
                    || !denominator.im.is_finite()
                {
                    Complex::new(T::zero(), T::zero())
                } else {
                    numerator / denominator
                }
            } else {
                numerator
            }
        };
        match settings.display_mode {
            DisplayMode::Amplitude => {
                output
                    .values
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(index, output)| {
                        *output = wave_at(index).norm();
                    });
            }
            DisplayMode::Phase if settings.phase_unwrap => {
                output
                    .values
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(index, output)| {
                        let wave = wave_at(index);
                        *output = wave.im.atan2(wave.re);
                    });
                let (minimum, maximum) = unwrap::unwrap_into(
                    &mut output.values,
                    settings.output_width,
                    settings.output_height,
                    &mut self.unwrap,
                )?;
                output.unwrapped_range = Some(unwrapped_display_range(minimum, maximum));
            }
            DisplayMode::Phase => {
                let factor = <T as Real>::from_u8(settings.phase_amplification);
                output
                    .values
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(index, output)| {
                        let wave = wave_at(index);
                        *output = wrap_to_pi(wave.im.atan2(wave.re) * factor);
                    });
            }
        }
        Ok(milliseconds(start))
    }

    fn fill_output_metrics(
        &self,
        frame: &Frame<T>,
        settings: &ReconstructionSettings,
        crop: &SidebandCrop,
        output: &mut Reconstruction<T>,
    ) {
        let source_x = shifted_to_unshifted(crop.sideband().x, frame.width());
        let source_y = shifted_to_unshifted(crop.sideband().y, frame.height());
        let sideband_intensity =
            sideband::magnitude(self.forward.samples[source_y * frame.width() + source_x]);
        let center_intensity = sideband::magnitude(self.forward.samples[0]);
        let (dx, dy) = crop
            .sideband()
            .offset_from_center(frame.width(), frame.height());
        let frequency = (dx / frame.width() as f64).hypot(dy / frame.height() as f64);
        output.width = settings.output_width;
        output.height = settings.output_height;
        output.frame_width = frame.width();
        output.frame_height = frame.height();
        output.sideband = crop.sideband();
        output.crop_radius = crop.radius();
        output.metrics = ReconstructionMetrics {
            fringe_contrast: if center_intensity > 0.0 {
                2.0 * sideband_intensity / center_intensity
            } else {
                0.0
            },
            fringe_spacing_pixels: if frequency > 0.0 {
                f64::from(frame.meta.binning) / frequency
            } else {
                0.0
            },
        };
        output.display_mode = settings.display_mode;
    }

    #[must_use]
    pub fn capture_reference(&self) -> Option<ReferenceWave<T>> {
        let (settings, scale) = self.latest_wave.as_ref()?;
        let samples: Vec<_> = self
            .inverse
            .samples
            .par_iter()
            .map(|&sample| sample * *scale)
            .collect();
        let maximum_power = samples
            .par_iter()
            .map(|sample| sample.norm_sqr())
            .reduce(T::zero, |left, right| left.max(right));
        let relative_epsilon = T::epsilon() * <T as Real>::from_f64(100.0);
        Some(ReferenceWave {
            settings: settings.clone(),
            samples,
            minimum_power: maximum_power * relative_epsilon * relative_epsilon,
        })
    }
}

struct FftBuffers<T: Real> {
    dimensions: Option<(usize, usize)>,
    plan: Option<Fft2<T>>,
    samples: Vec<Complex<T>>,
}

impl<T: Real> Default for FftBuffers<T> {
    fn default() -> Self {
        Self {
            dimensions: None,
            plan: None,
            samples: Vec::new(),
        }
    }
}

impl<T: Real> FftBuffers<T> {
    fn ensure(
        &mut self,
        width: usize,
        height: usize,
        direction: FftDirection,
    ) -> Result<(), CoreError> {
        if self.dimensions == Some((width, height)) {
            return Ok(());
        }
        let plan = Fft2::new(width, height, direction)?;
        self.samples
            .resize(width * height, Complex::new(T::zero(), T::zero()));
        self.plan = Some(plan);
        self.dimensions = Some((width, height));
        Ok(())
    }

    fn process(&mut self) -> Result<(), CoreError> {
        self.plan
            .as_mut()
            .expect("FFT plans exist after prepare")
            .process(&mut self.samples)
    }
}

fn milliseconds(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000.0
}

fn unwrapped_display_range<T: Real>(minimum: T, maximum: T) -> DisplayRange {
    DisplayRange::from_bounds(minimum.as_f32(), maximum.as_f32())
}
