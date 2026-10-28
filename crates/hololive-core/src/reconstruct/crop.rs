use std::ops::Range;

use crate::{CoreError, sideband::Sideband};

use super::{ReconstructionSettings, reference::WaveSettings};

pub(super) struct SidebandCrop {
    pub origin_x: usize,
    pub origin_y: usize,
    pub wave: WaveSettings,
}

impl SidebandCrop {
    pub(super) fn new(
        sideband: Sideband,
        radius: usize,
        frame_width: usize,
        frame_height: usize,
        settings: &ReconstructionSettings,
    ) -> Result<Self, CoreError> {
        let x = checked_span(sideband.x, radius)?;
        let y = checked_span(sideband.y, radius)?;
        let side = x.len();
        if x.end > frame_width
            || y.end > frame_height
            || side > settings.output_width
            || side > settings.output_height
        {
            return Err(CoreError::SidebandOutOfBounds);
        }
        Ok(Self {
            origin_x: x.start,
            origin_y: y.start,
            wave: WaveSettings {
                input_width: frame_width,
                input_height: frame_height,
                output_width: settings.output_width,
                output_height: settings.output_height,
                sideband,
                crop_size: side,
                tukey_alpha: settings.tukey_enabled.then_some(settings.tukey_alpha),
            },
        })
    }

    pub(super) fn sideband(&self) -> Sideband {
        self.wave.sideband
    }

    pub(super) fn side(&self) -> usize {
        self.wave.crop_size
    }

    pub(super) fn radius(&self) -> usize {
        self.wave.crop_size / 2
    }
}

fn checked_span(center: usize, radius: usize) -> Result<Range<usize>, CoreError> {
    match (center.checked_sub(radius), center.checked_add(radius)) {
        (Some(start), Some(end)) if start < end => Ok(start..end),
        _ => Err(CoreError::SidebandOutOfBounds),
    }
}

/// Largest crop radius that still fits in both the detector frame and the FFT output.
#[must_use]
pub fn max_output_crop_radius(
    frame_width: usize,
    frame_height: usize,
    output_width: usize,
    output_height: usize,
) -> usize {
    frame_width
        .min(frame_height)
        .min(output_width)
        .min(output_height)
        .saturating_div(2)
        .max(1)
}

/// Half-width of the automatic sideband crop: one-sixth of the center-to-sideband
/// distance, which is one-third of that distance in total.
pub(super) fn automatic_crop_radius(distance: f64) -> usize {
    (distance / 6.0).round() as usize
}
