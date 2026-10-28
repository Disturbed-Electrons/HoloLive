use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

use crate::{SimulationError, error::checked_len, scene::Scene};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SimulationSettings {
    pub width: usize,
    pub height: usize,
    pub target_fps: f64,
    pub jitter_fraction: f64,
    pub counts: f64,
    pub sampling: f64,
    pub visibility: f64,
    pub fringe_angle_degrees: f64,
    pub poisson_scale: Option<f64>,
    pub scene: Scene,
    #[serde(default = "random_seed")]
    pub seed: u64,
}

fn random_seed() -> u64 {
    rand::random()
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
            target_fps: 30.0,
            jitter_fraction: 0.075,
            counts: 1_000.0,
            sampling: 5.0,
            visibility: 1.0,
            fringe_angle_degrees: 30.0,
            poisson_scale: Some(0.05),
            scene: Scene::Combined,
            seed: random_seed(),
        }
    }
}

impl SimulationSettings {
    pub fn validate(&self) -> Result<(), SimulationError> {
        checked_len(self.width, self.height)?;
        if !self.target_fps.is_finite() || self.target_fps <= 0.0 || self.target_fps > 1_000.0 {
            return Err(SimulationError::InvalidSetting(
                "target FPS must be in (0, 1000]",
            ));
        }
        if !self.jitter_fraction.is_finite() || !(0.0..=1.0).contains(&self.jitter_fraction) {
            return Err(SimulationError::InvalidSetting("jitter must be in [0, 1]"));
        }
        if !self.counts.is_finite() || self.counts <= 0.0 {
            return Err(SimulationError::InvalidSetting(
                "counts must be finite and positive",
            ));
        }
        if !self.sampling.is_finite() || self.sampling <= 0.0 {
            return Err(SimulationError::InvalidSetting(
                "fringe sampling must be finite and positive",
            ));
        }
        if !self.visibility.is_finite() || !(0.0..=1.0).contains(&self.visibility) {
            return Err(SimulationError::InvalidSetting(
                "visibility must be in [0, 1]",
            ));
        }
        if !self.fringe_angle_degrees.is_finite() {
            return Err(SimulationError::InvalidSetting(
                "fringe angle must be finite",
            ));
        }
        if self
            .poisson_scale
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(SimulationError::InvalidSetting(
                "Poisson scale must be positive",
            ));
        }
        Ok(())
    }

    /// Produces sum-binned detector settings without generating an unbinned raster.
    pub fn for_binning(&self, binning: u32) -> Result<Self, SimulationError> {
        self.validate()?;
        let binning = binning as usize;
        if binning == 0
            || !self.width.is_multiple_of(binning)
            || !self.height.is_multiple_of(binning)
        {
            return Err(SimulationError::InvalidSetting(
                "binning must divide both detector dimensions",
            ));
        }

        let factor = binning as f64;
        let angle = self.fringe_angle_degrees.to_radians();
        let kx = -TAU / self.sampling * angle.sin();
        let ky = TAU / self.sampling * angle.cos();
        let area = factor * factor;
        let mut binned = self.clone();
        binned.width /= binning;
        binned.height /= binning;
        binned.counts *= area;
        binned.sampling /= factor;
        binned.visibility *= box_transfer(kx, binning) * box_transfer(ky, binning);
        binned.poisson_scale = binned.poisson_scale.map(|scale| scale / area);
        binned.validate()?;
        Ok(binned)
    }
}

fn box_transfer(frequency: f64, binning: usize) -> f64 {
    // Magnitude response of a normalized discrete box filter.
    let half = 0.5 * frequency;
    let denominator = half.sin();
    if denominator.abs() <= f64::EPSILON {
        1.0
    } else {
        ((binning as f64 * half).sin() / (binning as f64 * denominator))
            .abs()
            .min(1.0)
    }
}
