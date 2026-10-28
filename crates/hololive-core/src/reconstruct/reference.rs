use num_complex::Complex;

use crate::sideband::Sideband;

#[derive(Debug, Clone, PartialEq)]
pub struct WaveSettings {
    pub input_width: usize,
    pub input_height: usize,
    pub output_width: usize,
    pub output_height: usize,
    pub sideband: Sideband,
    pub crop_size: usize,
    pub tukey_alpha: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ReferenceWave<T> {
    pub settings: WaveSettings,
    pub samples: Vec<Complex<T>>,
    pub(crate) minimum_power: T,
}
