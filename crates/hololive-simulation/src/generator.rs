use std::f64::consts::TAU;

use rand::{RngExt as _, SeedableRng, rngs::Xoshiro256PlusPlus};
use rand_distr::StandardNormal;

use crate::{
    SimulationError, SimulationSettings,
    scene::{Scene, object_profile},
};

pub struct HologramGenerator {
    rng: Xoshiro256PlusPlus,
    seed: u64,
}

impl HologramGenerator {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Xoshiro256PlusPlus::seed_from_u64(seed),
            seed,
        }
    }

    pub fn generate(
        &mut self,
        settings: &SimulationSettings,
        sequence: u64,
        output: &mut Vec<f64>,
    ) -> Result<(), SimulationError> {
        settings.validate()?;
        if self.seed != settings.seed {
            self.seed = settings.seed;
            self.rng = Xoshiro256PlusPlus::seed_from_u64(settings.seed);
        }
        output.resize(settings.width * settings.height, 0.0);
        fill_intensity(output, settings, sequence);

        if let Some(scale) = settings.poisson_scale {
            apply_poisson_noise(output, scale * settings.counts, &mut self.rng);
        }
        Ok(())
    }
}

fn fill_intensity(output: &mut [f64], settings: &SimulationSettings, sequence: u64) {
    let width = settings.width;
    let angle = settings.fringe_angle_degrees.to_radians();
    let kx = -TAU / settings.sampling * angle.sin();
    let ky = TAU / settings.sampling * angle.cos();
    let counts = settings.counts;
    let visibility = settings.visibility;
    let drift = sequence as f64 * 0.004;

    match settings.scene {
        Scene::Vacuum => fill_vacuum(output, width, kx, ky, counts, visibility),
        Scene::Combined => fill_object(output, settings, kx, ky, drift, object_profile),
        Scene::PhaseObject => fill_object(output, settings, kx, ky, drift, |nx, ny, drift| {
            (1.0, object_profile(nx, ny, drift).1)
        }),
        Scene::AmplitudeObject => fill_object(output, settings, kx, ky, drift, |nx, ny, drift| {
            (object_profile(nx, ny, drift).0, 0.0)
        }),
    }
}

fn fill_vacuum(output: &mut [f64], width: usize, kx: f64, ky: f64, counts: f64, visibility: f64) {
    for (y, row) in output.chunks_exact_mut(width).enumerate() {
        let y_term = ky * y as f64;
        for (x, pixel) in row.iter_mut().enumerate() {
            *pixel = visibility.mul_add((y_term + kx * x as f64).cos(), 1.0) * counts;
        }
    }
}

fn fill_object(
    output: &mut [f64],
    settings: &SimulationSettings,
    kx: f64,
    ky: f64,
    drift: f64,
    object: impl Fn(f64, f64, f64) -> (f64, f64),
) {
    let width = settings.width;
    let step = 2.0 / width.min(settings.height) as f64;
    let nx0 = 0.5 * step - 0.5 * width as f64 * step;
    let ny0 = 0.5 * step - 0.5 * settings.height as f64 * step;
    let half_counts = 0.5 * settings.counts;
    let two_vis = 2.0 * settings.visibility;

    for (y, row) in output.chunks_exact_mut(width).enumerate() {
        let yf = y as f64;
        let y_term = ky * yf;
        let ny = ny0 + yf * step;
        for (x, pixel) in row.iter_mut().enumerate() {
            let xf = x as f64;
            let (amp, phase) = object(nx0 + xf * step, ny, drift);
            let carrier = y_term + kx * xf - phase;
            *pixel = half_counts * two_vis.mul_add(amp * carrier.cos(), amp.mul_add(amp, 1.0));
        }
    }
}

/// Shot noise as `Normal(I, sqrt(I * scale))`, the large-λ limit of
/// `scale * Poisson(I / scale)`.
fn apply_poisson_noise(values: &mut [f64], noise_scale: f64, rng: &mut Xoshiro256PlusPlus) {
    for value in values.iter_mut() {
        let intensity = *value;
        let z: f64 = rng.sample(StandardNormal);
        *value = (intensity + (intensity * noise_scale).sqrt() * z).max(0.0);
    }
}
