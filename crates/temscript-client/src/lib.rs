//! HTTP client and wire array codec for the temscript v1 API.
//!
//! [`TemscriptClient`] speaks the temscript v1 HTTP endpoints. [`PackedArray`]
//! encodes and decodes the base64 scalar payloads on the wire.

pub mod array;
pub mod client;
pub mod error;
pub mod protocol;

pub use array::{DecodedArray, Endianness, PackedArray, ScalarType};
pub use client::TemscriptClient;
pub use error::ProtocolError;
pub use protocol::{CameraInfo, CameraParameters};
