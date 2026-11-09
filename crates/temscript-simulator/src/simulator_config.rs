use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use hololive_simulation::SimulationSettings;
use serde::{Deserialize, Serialize};
use temscript_client::{Endianness, ScalarType};

use crate::error::SimulatorError;

pub const SIMULATED_CAMERA: &str = "HoloLive Simulator";
const SUPPORTED_BINNINGS: [u32; 4] = [1, 2, 4, 8];
const SIMULATION_JSON: &str = "simulation.json";
const SIMULATION_FIXTURE_PATH: &str = "fixtures/simulation.json";
const MAX_SIMULATED_PIXELS: usize = 2_048 * 2_048;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaultInjection {
    pub extra_latency_ms: u64,
    pub http_error_every: Option<u64>,
    pub malformed_payload_every: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulatorConfig {
    #[serde(default = "default_simulation")]
    pub simulation: SimulationSettings,
    #[serde(default)]
    pub wire_type: ScalarType,
    #[serde(default)]
    pub endianness: Endianness,
    #[serde(default = "default_exposure_seconds")]
    pub exposure_seconds: f64,
    #[serde(default = "default_binning")]
    pub binning: u32,
    #[serde(default)]
    pub faults: FaultInjection,
}

fn default_exposure_seconds() -> f64 {
    0.1
}

fn default_simulation() -> SimulationSettings {
    let binning = default_binning();
    let area = f64::from(binning * binning);
    SimulationSettings {
        width: 512 * binning as usize,
        height: 512 * binning as usize,
        counts: 1_000.0 / area,
        sampling: 5.0 * f64::from(binning),
        poisson_scale: Some(0.05 * area),
        ..SimulationSettings::default()
    }
}

fn default_binning() -> u32 {
    4
}

impl Default for SimulatorConfig {
    fn default() -> Self {
        Self {
            simulation: default_simulation(),
            wire_type: ScalarType::UINT16,
            endianness: Endianness::LITTLE,
            exposure_seconds: default_exposure_seconds(),
            binning: default_binning(),
            faults: FaultInjection::default(),
        }
    }
}

impl SimulatorConfig {
    pub(crate) fn load_file(path: impl AsRef<Path>) -> Result<Self, SimulatorError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|source| SimulatorError::io(path, source))?;
        let config: Self =
            serde_json::from_str(&text).map_err(|source| SimulatorError::json(path, source))?;
        config.validate()?;
        Ok(config)
    }

    pub(crate) fn validate(&self) -> Result<(), SimulatorError> {
        self.simulation
            .validate()
            .map_err(|error| SimulatorError::validation(error.to_string()))?;
        validate_binning(self, self.binning)?;
        validate_timing(self)?;
        if !self.exposure_seconds.is_finite() || self.exposure_seconds < 0.0 {
            return Err(SimulatorError::validation(
                "exposure must be finite and non-negative",
            ));
        }
        Ok(())
    }

    pub(crate) fn supported_binnings(&self) -> Vec<u32> {
        SUPPORTED_BINNINGS
            .into_iter()
            .filter(|&binning| validate_binning(self, binning).is_ok())
            .collect()
    }

    pub(crate) fn load_from_directory(
        dir: impl AsRef<Path>,
    ) -> Result<(Option<PathBuf>, Self), SimulatorError> {
        let dir = dir.as_ref();
        for relative in [SIMULATION_JSON, SIMULATION_FIXTURE_PATH] {
            let path = dir.join(relative);
            if path.is_file() {
                let config = Self::load_file(&path)?;
                return Ok((Some(path), config));
            }
        }
        Ok((None, Self::default()))
    }

    pub fn load_fixture() -> Result<(Option<PathBuf>, Self), SimulatorError> {
        let cwd = env::current_dir()
            .map_err(|source| SimulatorError::io(Path::new("current directory"), source))?;
        Self::load_from_directory(cwd)
    }
}

fn validate_timing(config: &SimulatorConfig) -> Result<(), SimulatorError> {
    let longest_interval = (1.0 + config.simulation.jitter_fraction) / config.simulation.target_fps;
    let interval = Duration::try_from_secs_f64(longest_interval)
        .map_err(|_| SimulatorError::validation("frame interval is too large"))?;
    let latency = Duration::from_millis(config.faults.extra_latency_ms);
    let now = Instant::now();
    if now.checked_add(interval).is_none() || now.checked_add(latency).is_none() {
        return Err(SimulatorError::validation("simulator delay is too large"));
    }
    Ok(())
}

fn validate_binning(config: &SimulatorConfig, binning: u32) -> Result<(), SimulatorError> {
    if !SUPPORTED_BINNINGS.contains(&binning)
        || !config.simulation.width.is_multiple_of(binning as usize)
        || !config.simulation.height.is_multiple_of(binning as usize)
    {
        return Err(SimulatorError::validation(
            "binning must be supported and divide both detector dimensions",
        ));
    }
    let output_pixels = (config.simulation.width / binning as usize)
        * (config.simulation.height / binning as usize);
    if output_pixels > MAX_SIMULATED_PIXELS {
        return Err(SimulatorError::validation(
            "binned simulator output exceeds 2048 × 2048 pixels",
        ));
    }
    Ok(())
}
