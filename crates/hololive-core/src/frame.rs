use std::sync::Arc;

use crate::{CoreError, checked_len, error::expect_len};

#[derive(Debug, Clone, PartialEq)]
pub struct FrameMeta {
    pub sequence: u64,
    pub acquired_unix_ns: u128,
    pub detector: Arc<str>,
    pub exposure_seconds: f64,
    pub binning: u32,
}

impl Default for FrameMeta {
    fn default() -> Self {
        Self {
            sequence: 0,
            acquired_unix_ns: 0,
            detector: Arc::from(""),
            exposure_seconds: 0.0,
            binning: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame<T> {
    width: usize,
    height: usize,
    pixels: Vec<T>,
    pub meta: FrameMeta,
}

impl<T> Frame<T> {
    pub fn new(
        width: usize,
        height: usize,
        pixels: Vec<T>,
        meta: FrameMeta,
    ) -> Result<Self, CoreError> {
        let expected = checked_len(width, height)?;
        expect_len(pixels.len(), expected)?;
        Ok(Self {
            width,
            height,
            pixels,
            meta,
        })
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[T] {
        &self.pixels
    }

    #[must_use]
    pub fn into_pixels(self) -> Vec<T> {
        self.pixels
    }
}
