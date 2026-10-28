use super::settings::DisplayMode;
use crate::sideband::Sideband;

/// Intensity window for live-view or export mapping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayRange {
    pub minimum: f32,
    pub maximum: f32,
}

impl DisplayRange {
    pub const UNIT: Self = Self {
        minimum: 0.0,
        maximum: 1.0,
    };

    #[must_use]
    pub fn from_bounds(minimum: f32, maximum: f32) -> Self {
        if maximum > minimum {
            Self { minimum, maximum }
        } else {
            Self {
                minimum,
                maximum: minimum + 1.0,
            }
        }
    }
}

impl Default for DisplayRange {
    fn default() -> Self {
        Self::UNIT
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StageTimings {
    pub forward_fft_ms: f64,
    pub sideband_ms: f64,
    pub inverse_fft_ms: f64,
    pub extract_ms: f64,
    pub total_ms: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReconstructionMetrics {
    pub fringe_contrast: f64,
    pub fringe_spacing_pixels: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReconstructionInfo {
    pub frame_width: usize,
    pub frame_height: usize,
    pub sideband: Sideband,
    pub crop_radius: usize,
    pub metrics: ReconstructionMetrics,
    pub timings: StageTimings,
    pub display_mode: DisplayMode,
    pub unwrapped_range: Option<DisplayRange>,
}

#[derive(Debug, Clone)]
pub struct Reconstruction<T> {
    pub width: usize,
    pub height: usize,
    pub frame_width: usize,
    pub frame_height: usize,
    pub values: Vec<T>,
    pub sideband: Sideband,
    pub crop_radius: usize,
    pub metrics: ReconstructionMetrics,
    pub timings: StageTimings,
    pub display_mode: DisplayMode,
    pub unwrapped_range: Option<DisplayRange>,
}

impl<T> Reconstruction<T> {
    #[must_use]
    pub fn info(&self) -> ReconstructionInfo {
        ReconstructionInfo {
            frame_width: self.frame_width,
            frame_height: self.frame_height,
            sideband: self.sideband,
            crop_radius: self.crop_radius,
            metrics: self.metrics,
            timings: self.timings,
            display_mode: self.display_mode,
            unwrapped_range: self.unwrapped_range,
        }
    }
}

impl<T> Default for Reconstruction<T> {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            frame_width: 0,
            frame_height: 0,
            values: Vec::new(),
            sideband: Sideband { x: 0, y: 0 },
            crop_radius: 0,
            metrics: ReconstructionMetrics::default(),
            timings: StageTimings::default(),
            display_mode: DisplayMode::Phase,
            unwrapped_range: None,
        }
    }
}
