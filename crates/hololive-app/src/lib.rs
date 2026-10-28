//! GUI, acquisition/reconstruction pipeline, and the `HoloLive` binary.

pub(crate) mod cli;
pub(crate) mod config;
pub(crate) mod export;
pub(crate) mod levels;
pub(crate) mod pipeline;
pub(crate) mod ui;

pub use cli::{Arguments, init_tracing, run};
pub(crate) use config::AppConfig;
