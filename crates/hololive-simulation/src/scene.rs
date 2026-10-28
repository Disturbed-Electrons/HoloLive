use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scene {
    Vacuum,
    PhaseObject,
    AmplitudeObject,
    Combined,
}

#[inline]
pub(crate) fn object_profile(nx: f64, ny: f64, drift: f64) -> (f64, f64) {
    let dx1 = nx + 0.28;
    let dy1 = ny - 0.08;
    let dx2 = nx - 0.3;
    let dy2 = ny + 0.2;
    let r1 = dx1.mul_add(dx1, dy1 * dy1).sqrt();
    let r2 = dx2.mul_add(dx2, dy2 * dy2).sqrt();
    let amplitude = 1.0 - 0.28 * soft_disk(r2, 0.24);
    let phase =
        1.3 * soft_disk(r1, 0.3) + 0.35 * nx + 0.08 * (std::f64::consts::TAU * ny + drift).sin();
    (amplitude, phase)
}

#[inline]
fn soft_disk(radius: f64, edge: f64) -> f64 {
    0.5 * (1.0 - (35.0 * (radius - edge)).tanh())
}
