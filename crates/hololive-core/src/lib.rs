//! Numerical engine for HoloLive.
//!
//! Provides FFT, spatial filters, phase unwrapping, and hologram reconstruction.
//! This crate does not include hologram simulation or HTTP client/server code.

pub mod dsp;
pub mod error;
pub mod frame;
pub mod precision;
pub mod reconstruct;
pub mod sideband;
pub(crate) mod unwrap;

pub use dsp::fft2::next_fast_len;
pub use error::{CoreError, checked_len};
pub use frame::{Frame, FrameMeta};
pub use precision::{Precision, Real};
pub use reconstruct::{
    DisplayMode, DisplayRange, Reconstruction, ReconstructionEngine, ReconstructionInfo,
    ReconstructionSettings, ReferenceWave, max_output_crop_radius,
};
pub use sideband::{SearchRegion, Sideband};
