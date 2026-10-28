use std::fmt::Debug;

use num_traits::Float;
use rustfft::FftNum;

pub trait Real: FftNum + Float + Send + Sync + Debug + 'static {
    fn from_f64(value: f64) -> Self;
    fn as_f64(self) -> f64;
    fn from_usize(value: usize) -> Self;
    fn from_u8(value: u8) -> Self;
    fn from_i32(value: i32) -> Self;
    fn as_f32(self) -> f32;
}

impl Real for f32 {
    fn from_f64(value: f64) -> Self {
        value as Self
    }

    fn as_f64(self) -> f64 {
        f64::from(self)
    }

    fn from_usize(value: usize) -> Self {
        value as Self
    }

    fn from_u8(value: u8) -> Self {
        f32::from(value)
    }

    fn from_i32(value: i32) -> Self {
        value as Self
    }

    fn as_f32(self) -> f32 {
        self
    }
}

impl Real for f64 {
    fn from_f64(value: f64) -> Self {
        value
    }

    fn as_f64(self) -> f64 {
        self
    }

    fn from_usize(value: usize) -> Self {
        value as Self
    }

    fn from_u8(value: u8) -> Self {
        f64::from(value)
    }

    fn from_i32(value: i32) -> Self {
        f64::from(value)
    }

    fn as_f32(self) -> f32 {
        self as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    F32,
    F64,
}

impl Precision {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::F32 => "float32",
            Self::F64 => "float64",
        }
    }
}
