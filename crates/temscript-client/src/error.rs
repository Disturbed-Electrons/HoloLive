use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid temscript server URL: {0}")]
    InvalidServerUrl(String),
    #[error("HTTP request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("{endpoint} returned HTTP {status}: {message}")]
    Http {
        endpoint: String,
        status: u16,
        message: String,
    },
    #[error("invalid JSON response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to read HTTP response: {0}")]
    ResponseRead(#[from] std::io::Error),
    #[error("HTTP response exceeds the {limit_bytes}-byte limit")]
    ResponseTooLarge { limit_bytes: usize },
    #[error("invalid base64 array payload: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("unsupported array encoding {0}")]
    UnsupportedEncoding(String),
    #[error("array dimensions are invalid")]
    InvalidDimensions,
    #[error("array byte length is {actual}; expected {expected}")]
    ByteLength { expected: usize, actual: usize },
    #[error("base64 array length is {actual}; expected {expected}")]
    EncodedLength { expected: usize, actual: usize },
    #[error("array has {actual} values; expected {expected} pixels")]
    PixelCount { expected: usize, actual: usize },
    #[error("detector {0:?} was absent from the acquisition response")]
    MissingDetector(String),
    #[error("camera {0:?} is unavailable")]
    MissingCamera(String),
}
