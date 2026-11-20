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
            ScalarType::INT8 => PixelData::I8($bytes.into_iter().map(u8::cast_signed).collect()),
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

macro_rules! encode_pixels {
    ($values:expr, $out:expr, $byte_order:ident, $kind:expr) => {
        encode_pixels!(@ $values, $out, $byte_order, $kind;
            INT8, i8; INT16, i16; INT32, i32; INT64, i64;
            UINT8, u8; UINT16, u16; UINT32, u32; UINT64, u64
        )
    };
    (@ $values:expr, $out:expr, $byte_order:ident, $kind:expr; $($scalar:ident, $ty:ty);+) => {
        match $kind {
            $(
                ScalarType::$scalar => encode_samples(
                    $values,
                    $out,
                    |value| saturating_round(value, <$ty>::MIN as f64, <$ty>::MAX as f64) as $ty,
                    <$ty>::$byte_order,
                ),
            )+
            ScalarType::FLOAT32 => {
                encode_samples($values, $out, |value| value as f32, f32::$byte_order)
            }
            ScalarType::FLOAT64 => {
                encode_samples($values, $out, std::convert::identity, f64::$byte_order)
            }
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
        let encoded =
            base64::encoded_len(expected, true).ok_or(ProtocolError::InvalidDimensions)?;
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
        match endianness {
            Endianness::LITTLE => encode_pixels!(values, &mut bytes, to_le_bytes, scalar_type),
            Endianness::BIG => encode_pixels!(values, &mut bytes, to_be_bytes, scalar_type),
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

fn encode_samples<T, const N: usize>(
    values: &[f64],
    output: &mut Vec<u8>,
    convert: impl Fn(f64) -> T,
    to_bytes: impl Fn(T) -> [u8; N],
) {
    for &value in values {
        output.extend_from_slice(&to_bytes(convert(value)));
    }
}

fn decode_chunks<T, const N: usize>(
    bytes: &[u8],
    endianness: Endianness,
    little: impl Fn([u8; N]) -> T,
    big: impl Fn([u8; N]) -> T,
) -> Vec<T> {
    let chunks = bytes.as_chunks::<N>().0.iter().copied();
    match endianness {
        Endianness::LITTLE => chunks.map(little).collect(),
        Endianness::BIG => chunks.map(big).collect(),
    }
}

fn saturating_round(value: f64, minimum: f64, maximum: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.round().clamp(minimum, maximum)
    }
}
