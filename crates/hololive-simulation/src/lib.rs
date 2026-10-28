//! Synthetic hologram generation for HoloLive (`Scene`, `SimulationSettings`, `HologramGenerator`).

mod error;
mod generator;
mod scene;
mod settings;

pub use error::SimulationError;
pub use generator::HologramGenerator;
pub use scene::Scene;
pub use settings::SimulationSettings;
