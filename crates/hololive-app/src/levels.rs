//! Pixel-level value mapping for live view and PNG export, not the egui viewport.
//!
//! Wrapped phase maps `[-π, π]` to opaque gray. Unwrapped phase uses the min–max
//! recorded while writing unwrapped samples (0–100). Hologram, spectrum, and
//! amplitude use HoloDesk percentiles on a rotating 4×4 grid, smoothed across
//! frames. PNG export uses full min–max over finite values.

use hololive_core::{DisplayRange, Real};

/// Spatial stride of the live-view grid (one sample per `N×N` cell).
const GRID_STRIDE: usize = 4;
const GRID_CELL: usize = GRID_STRIDE * GRID_STRIDE;
const LEVEL_SMOOTH: f32 = 0.25;

/// Inclusive percentile window for live-view auto-contrast.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PercentileWindow {
    pub lower: f32,
    pub upper: f32,
}

/// Hologram auto-contrast: `[0.1, 99.9]`.
const HOLOGRAM_PERCENTILES: PercentileWindow = PercentileWindow {
    lower: 0.1,
    upper: 99.9,
};

/// FFT spectrum auto-contrast: `[0.01, 99.9]`.
const SPECTRUM_PERCENTILES: PercentileWindow = PercentileWindow {
    lower: 0.01,
    upper: 99.9,
};

/// Amplitude auto-contrast: `[1, 99]`.
const AMPLITUDE_PERCENTILES: PercentileWindow = PercentileWindow {
    lower: 1.0,
    upper: 99.0,
};

/// Calibrated display range for wrapped phase (`[-π, π]`).
#[must_use]
pub(crate) fn phase_wrap_range() -> DisplayRange {
    DisplayRange {
        minimum: -std::f32::consts::PI,
        maximum: std::f32::consts::PI,
    }
}

/// Detector or reconstruction plane size plus the frame sequence used to rotate the grid origin.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridSpec {
    pub width: usize,
    pub height: usize,
    pub sequence: u64,
}

#[derive(Clone, Copy)]
enum LevelChannel {
    Hologram,
    Spectrum,
    Amplitude,
}

impl LevelChannel {
    const fn index(self) -> usize {
        match self {
            Self::Hologram => 0,
            Self::Spectrum => 1,
            Self::Amplitude => 2,
        }
    }

    const fn window(self) -> PercentileWindow {
        match self {
            Self::Hologram => HOLOGRAM_PERCENTILES,
            Self::Spectrum => SPECTRUM_PERCENTILES,
            Self::Amplitude => AMPLITUDE_PERCENTILES,
        }
    }
}

/// Reconstruction-thread owner of EMA state and grid scratch.
#[derive(Default)]
pub(crate) struct DisplayLevels {
    scratch: Vec<f32>,
    ranges: [Option<DisplayRange>; 3],
    frame_width: usize,
    frame_height: usize,
    output_width: usize,
    output_height: usize,
}

impl DisplayLevels {
    pub(crate) fn hologram<T: Real>(&mut self, values: &[T], grid: GridSpec) -> DisplayRange {
        self.sync_frame_size(grid.width, grid.height);
        self.sample(LevelChannel::Hologram, values, grid)
    }

    pub(crate) fn spectrum<T: Real>(&mut self, values: &[T], grid: GridSpec) -> DisplayRange {
        self.sync_frame_size(grid.width, grid.height);
        self.sample(LevelChannel::Spectrum, values, grid)
    }

    pub(crate) fn amplitude<T: Real>(&mut self, values: &[T], grid: GridSpec) -> DisplayRange {
        self.sync_output_size(grid.width, grid.height);
        self.sample(LevelChannel::Amplitude, values, grid)
    }

    fn sample<T: Real>(
        &mut self,
        channel: LevelChannel,
        values: &[T],
        grid: GridSpec,
    ) -> DisplayRange {
        let range = live_range(
            values,
            grid,
            channel.window(),
            self.ranges[channel.index()],
            &mut self.scratch,
        );
        self.ranges[channel.index()] = Some(range);
        range
    }

    fn sync_frame_size(&mut self, width: usize, height: usize) {
        if width != self.frame_width || height != self.frame_height {
            self.ranges[LevelChannel::Hologram.index()] = None;
            self.ranges[LevelChannel::Spectrum.index()] = None;
            self.frame_width = width;
            self.frame_height = height;
        }
    }

    fn sync_output_size(&mut self, width: usize, height: usize) {
        if width != self.output_width || height != self.output_height {
            self.ranges[LevelChannel::Amplitude.index()] = None;
            self.output_width = width;
            self.output_height = height;
        }
    }
}

/// Grid sample, percentile window, then optional EMA toward `previous`.
#[must_use]
fn live_range<T: Real>(
    values: &[T],
    grid: GridSpec,
    window: PercentileWindow,
    previous: Option<DisplayRange>,
    scratch: &mut Vec<f32>,
) -> DisplayRange {
    sample_grid(values, grid, scratch);
    let sample = percentile_range(scratch, window);
    match previous {
        Some(previous) => smooth_range(previous, sample),
        None => sample,
    }
}

/// Collect one pixel per 4×4 cell, rotating the origin with `sequence`.
///
/// Width and height do not need to be multiples of 4: leftover rows and
/// columns are included whenever they line up with that frame's origin.
/// Images smaller than the stride in either axis are scanned densely.
fn sample_grid<T: Real>(values: &[T], grid: GridSpec, scratch: &mut Vec<f32>) {
    scratch.clear();
    if grid.width == 0 || grid.height == 0 || values.len() != grid.width * grid.height {
        return;
    }
    let stride_x = GRID_STRIDE.min(grid.width);
    let stride_y = GRID_STRIDE.min(grid.height);
    if stride_x < GRID_STRIDE || stride_y < GRID_STRIDE {
        scratch.extend(
            values
                .iter()
                .map(|value| value.as_f32())
                .filter(|value| value.is_finite()),
        );
        return;
    }
    let sequence = grid.sequence as usize % GRID_CELL;
    let origin_x = sequence % stride_x;
    let origin_y = sequence / stride_x;
    let columns = (grid.width - origin_x).div_ceil(stride_x);
    let rows = (grid.height - origin_y).div_ceil(stride_y);
    scratch.reserve(columns * rows);
    for y in (origin_y..grid.height).step_by(stride_y) {
        let row = &values[y * grid.width..y * grid.width + grid.width];
        for x in (origin_x..grid.width).step_by(stride_x) {
            let value = row[x].as_f32();
            if value.is_finite() {
                scratch.push(value);
            }
        }
    }
}

/// Percentiles of `values` at `window`. Empty input yields [`DisplayRange::UNIT`].
#[must_use]
fn percentile_range(values: &mut [f32], window: PercentileWindow) -> DisplayRange {
    if values.is_empty() {
        return DisplayRange::UNIT;
    }
    let lower = percentile_index(values.len(), window.lower);
    let upper = percentile_index(values.len(), window.upper);
    values.select_nth_unstable_by(lower, f32::total_cmp);
    let minimum = values[lower];
    values[lower..].select_nth_unstable_by(upper - lower, f32::total_cmp);
    DisplayRange::from_bounds(minimum, values[upper])
}

fn percentile_index(len: usize, percent: f32) -> usize {
    if percent >= 100.0 {
        return len - 1;
    }
    if percent <= 0.0 {
        return 0;
    }
    let index = (f64::from(percent) / 100.0 * (len - 1) as f64).round() as usize;
    index.min(len - 1)
}

fn smooth_range(previous: DisplayRange, sample: DisplayRange) -> DisplayRange {
    let retain = 1.0 - LEVEL_SMOOTH;
    let minimum = LEVEL_SMOOTH * sample.minimum + retain * previous.minimum;
    let maximum = LEVEL_SMOOTH * sample.maximum + retain * previous.maximum;
    DisplayRange::from_bounds(minimum, maximum)
}

/// Live-view RGBA packer. Non-finite samples become black so auto-contrast
/// and the texture cannot lock onto NaN or infinity.
pub(crate) fn pack_rgba_into<T: Real>(values: &[T], range: DisplayRange, output: &mut Vec<u8>) {
    let scale = 255.0 / (range.maximum - range.minimum).max(f32::EPSILON);
    output.resize(values.len().saturating_mul(4), 255);
    for (pixel, &value) in output.as_chunks_mut::<4>().0.iter_mut().zip(values) {
        let gray = pack_finite_u8(value.as_f32(), range.minimum, scale);
        pixel[0] = gray;
        pixel[1] = gray;
        pixel[2] = gray;
        pixel[3] = 255;
    }
}

/// Min–max normalization for PNG export (full finite range, not percentiles).
pub(crate) fn minmax_normalize<T: Real>(values: &[T]) -> Vec<u8> {
    let (mut minimum, mut maximum) = (f64::INFINITY, f64::NEG_INFINITY);
    for &value in values {
        let value = value.as_f64();
        if value.is_finite() {
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
    }
    let range = maximum - minimum;
    if !range.is_finite() || range <= f64::EPSILON {
        return vec![0; values.len()];
    }
    values
        .iter()
        .map(|value| pack_finite_u8_f64(value.as_f64(), minimum, range))
        .collect()
}

#[inline]
fn pack_finite_u8(value: f32, minimum: f32, scale: f32) -> u8 {
    if value.is_finite() {
        ((value - minimum) * scale).clamp(0.0, 255.0) as u8
    } else {
        0
    }
}

#[inline]
fn pack_finite_u8_f64(value: f64, minimum: f64, range: f64) -> u8 {
    if value.is_finite() {
        ((value - minimum) * 255.0 / range).clamp(0.0, 255.0) as u8
    } else {
        0
    }
}
