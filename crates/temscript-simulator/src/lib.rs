//! Loopback temscript HTTP server and fixture loading for HoloLive.

mod error;
mod server;
mod simulator_config;

pub use error::SimulatorError;
pub use server::SimulatorHandle;
pub use simulator_config::{SIMULATED_CAMERA, SimulatorConfig};
