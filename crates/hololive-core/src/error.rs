use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum CoreError {
    #[error("dimensions must be non-zero")]
    EmptyDimensions,
    #[error("{width} × {height} exceeds the configured pixel limit")]
    ImageTooLarge { width: usize, height: usize },
    #[error("expected {expected} samples, received {actual}")]
    LengthMismatch { expected: usize, actual: usize },
    #[error("invalid setting: {0}")]
    InvalidSetting(&'static str),
    #[error("no valid sideband exists in the selected region")]
    SidebandNotFound,
    #[error("sideband crop lies outside the spectrum")]
    SidebandOutOfBounds,
    #[error("reference wave is incompatible with this reconstruction")]
    IncompatibleReference,
    #[error("phase contains a non-finite value")]
    NonFiniteInput,
}

/// Largest width×height HoloLive will allocate for a reconstruction plane.
pub const MAX_IMAGE_PIXELS: usize = 16_384 * 16_384;

pub fn checked_len(width: usize, height: usize) -> Result<usize, CoreError> {
    if width == 0 || height == 0 {
        return Err(CoreError::EmptyDimensions);
    }
    let len = width
        .checked_mul(height)
        .filter(|&len| len <= MAX_IMAGE_PIXELS)
        .ok_or(CoreError::ImageTooLarge { width, height })?;
    Ok(len)
}

pub(crate) fn expect_len(len: usize, expected: usize) -> Result<(), CoreError> {
    if len != expected {
        return Err(CoreError::LengthMismatch {
            expected,
            actual: len,
        });
    }
    Ok(())
}
