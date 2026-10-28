use std::mem::size_of;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::ProtocolError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScalarType {
    INT8,
    INT16,
    INT32,
    INT64,
    UINT8,
    #[default]
    UINT16,
    UINT32,
    UINT64,
    FLOAT32,
    FLOAT64,
}

impl ScalarType {
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Self::INT8 | Self::UINT8 => 1,
            Self::INT16 | Self::UINT16 => 2,
            Self::INT32 | Self::UINT32 | Self::FLOAT32 => 4,
            Self::INT64 | Self::UINT64 | Self::FLOAT64 => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Endianness {
    #[default]
    LITTLE,
    BIG,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackedArray {
    pub width: usize,
    pub height: usize,
    #[serde(rename = "type")]
    pub scalar_type: ScalarType,
    pub endianness: Endianness,
    pub encoding: String,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PixelData {
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodedArray {
    pub width: usize,
    pub height: usize,
    pub pixels: PixelData,
}

macro_rules! decode_pixels {
    ($bytes:expr, $endian:expr, $kind:expr) => {
        decode_pixels!(@ $bytes, $endian, $kind;
            INT16, I16, i16; INT32, I32, i32; INT64, I64, i64;
            UINT16, U16, u16; UINT32, U32, u32; UINT64, U64, u64;
            FLOAT32, F32, f32; FLOAT64, F64, f64
        )
    };
    (@ $bytes:expr, $endian:expr, $kind:expr; $($scalar:ident, $variant:ident, $ty:ty);+) => {
        match $kind {
            ScalarType::INT8 => PixelData::I8($bytes.into_iter().map(|byte| byte as i8).collect()),
            ScalarType::UINT8 => PixelData::U8($bytes),
            $(
                ScalarType::$scalar => PixelData::$variant(decode_chunks(
                    &$bytes,
                    $endian,
                    <$ty>::from_le_bytes,
                    <$ty>::from_be_bytes,
                )),
            )+
        }
    };
}

macro_rules! encode_scalar {
    ($out:expr, $value:expr, $endian:expr, $kind:expr) => {
        encode_scalar!(@ $out, $value, $endian, $kind;
            INT8, i8; INT16, i16; INT32, i32; INT64, i64;
            UINT8, u8; UINT16, u16; UINT32, u32; UINT64, u64
        )
    };
    (@ $out:expr, $value:expr, $endian:expr, $kind:expr; $($scalar:ident, $ty:ty);+) => {
        match $kind {
            $(
                ScalarType::$scalar => push_bytes(
                    $out,
                    saturating_round($value, <$ty>::MIN as f64, <$ty>::MAX as f64) as $ty,
                    $endian,
                ),
            )+
            ScalarType::FLOAT32 => push_bytes($out, $value as f32, $endian),
            ScalarType::FLOAT64 => push_bytes($out, $value, $endian),
        }
    };
}

macro_rules! fill_as {
    ($pixels:expr, $output:expr, $t:ty) => {
        fill_as!(@ $pixels, $output, $t; I8 I16 I32 I64 U8 U16 U32 U64 F32 F64)
    };
    (@ $pixels:expr, $output:expr, $t:ty; $($variant:ident)+) => {{
        $output.clear();
        match $pixels {
            $(PixelData::$variant(values) => {
                $output.extend(values.iter().map(|&value| value as $t));
            })+
        }
    }};
}

impl PackedArray {
    pub fn decode(&self) -> Result<DecodedArray, ProtocolError> {
        if !self.encoding.eq_ignore_ascii_case("BASE64") {
            return Err(ProtocolError::UnsupportedEncoding(self.encoding.clone()));
        }
        let count = pixel_count(self.width, self.height)?;
        let expected = count
            .checked_mul(self.scalar_type.bytes())
            .ok_or(ProtocolError::InvalidDimensions)?;
        let encoded = expected
            .checked_add(2)
            .map(|length| length / 3)
            .and_then(|length| length.checked_mul(4))
            .ok_or(ProtocolError::InvalidDimensions)?;
        if self.data.len() != encoded {
            return Err(ProtocolError::EncodedLength {
                expected: encoded,
                actual: self.data.len(),
            });
        }
        let bytes = STANDARD.decode(&self.data)?;
        if bytes.len() != expected {
            return Err(ProtocolError::ByteLength {
                expected,
                actual: bytes.len(),
            });
        }
        let pixels = decode_pixels!(bytes, self.endianness, self.scalar_type);
        Ok(DecodedArray {
            width: self.width,
            height: self.height,
            pixels,
        })
    }

    pub fn encode(
        values: &[f64],
        width: usize,
        height: usize,
        scalar_type: ScalarType,
        endianness: Endianness,
    ) -> Result<Self, ProtocolError> {
        let expected = pixel_count(width, height)?;
        if values.len() != expected {
            return Err(ProtocolError::PixelCount {
                expected,
                actual: values.len(),
            });
        }
        let mut bytes = Vec::with_capacity(values.len() * scalar_type.bytes());
        for &value in values {
            encode_scalar!(&mut bytes, value, endianness, scalar_type);
        }
        Ok(Self {
            width,
            height,
            scalar_type,
            endianness,
            encoding: "BASE64".into(),
            data: STANDARD.encode(bytes),
        })
    }
}

pub(crate) const MAX_IMAGE_PIXELS: usize = 16_384 * 16_384;

fn pixel_count(width: usize, height: usize) -> Result<usize, ProtocolError> {
    width
        .checked_mul(height)
        .filter(|&count| count > 0 && count <= MAX_IMAGE_PIXELS)
        .ok_or(ProtocolError::InvalidDimensions)
}

impl DecodedArray {
    pub fn fill_f32(&self, output: &mut Vec<f32>) {
        fill_as!(&self.pixels, output, f32);
    }

    pub fn fill_f64(&self, output: &mut Vec<f64>) {
        fill_as!(&self.pixels, output, f64);
    }
}

trait ByteValue: Copy {
    type Bytes: AsRef<[u8]>;
    fn little(self) -> Self::Bytes;
    fn big(self) -> Self::Bytes;
}

macro_rules! impl_byte_value {
    ($($ty:ty),+) => {
        $(
            impl ByteValue for $ty {
                type Bytes = [u8; size_of::<$ty>()];
                fn little(self) -> Self::Bytes {
                    self.to_le_bytes()
                }
                fn big(self) -> Self::Bytes {
                    self.to_be_bytes()
                }
            }
        )+
    };
}
impl_byte_value!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

fn push_bytes<T: ByteValue>(output: &mut Vec<u8>, value: T, endianness: Endianness) {
    let bytes = match endianness {
        Endianness::LITTLE => value.little(),
        Endianness::BIG => value.big(),
    };
    output.extend_from_slice(bytes.as_ref());
}

fn decode_chunks<T, const N: usize>(
    bytes: &[u8],
    endianness: Endianness,
    little: fn([u8; N]) -> T,
    big: fn([u8; N]) -> T,
) -> Vec<T> {
    bytes
        .as_chunks::<N>()
        .0
        .iter()
        .copied()
        .map(|chunk| match endianness {
            Endianness::LITTLE => little(chunk),
            Endianness::BIG => big(chunk),
        })
        .collect()
}

fn saturating_round(value: f64, minimum: f64, maximum: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.round().clamp(minimum, maximum)
    }
}
