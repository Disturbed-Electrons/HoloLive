use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SimulationError {
    #[error("dimensions must be non-zero")]
    EmptyDimensions,
    #[error("{width} × {height} exceeds the configured pixel limit")]
    ImageTooLarge { width: usize, height: usize },
    #[error("invalid setting: {0}")]
    InvalidSetting(&'static str),
}

const MAX_IMAGE_PIXELS: usize = 16_384 * 16_384;

pub(crate) fn checked_len(width: usize, height: usize) -> Result<usize, SimulationError> {
    if width == 0 || height == 0 {
        return Err(SimulationError::EmptyDimensions);
    }
    width
        .checked_mul(height)
        .filter(|&len| len <= MAX_IMAGE_PIXELS)
        .ok_or(SimulationError::ImageTooLarge { width, height })
}
