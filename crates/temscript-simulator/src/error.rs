use std::{
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimulatorError {
    #[error("IO error reading {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("JSON error in {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("bind failed on {address}: {message}")]
    Bind { address: String, message: String },

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("thread spawn failed: {source}")]
    Thread {
        #[source]
        source: io::Error,
    },
}

impl SimulatorError {
    pub fn io(path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub fn json(path: impl AsRef<Path>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}
