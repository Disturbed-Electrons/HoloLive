//! Second-difference pixel reliability for edge sorting (Arevalillo *et al.*, 2002).
//!
//! Lower reliability is better. Interior pixels only: the H/V/D1/D2 stencil
//! needs a one-pixel border, which keeps `T::max_value()` from the fill so
//! those pixels are merged last. Each interior row writes a disjoint slice of
//! the reliability buffer.
//!
//! ```text
//! H  = γ(φ_{i,j−1} − φ) − γ(φ − φ_{i,j+1})
//! V  = γ(φ_{i−1,j} − φ) − γ(φ − φ_{i+1,j})
//! D1 = γ(φ_{i−1,j−1} − φ) − γ(φ − φ_{i+1,j+1})
//! D2 = γ(φ_{i−1,j+1} − φ) − γ(φ − φ_{i+1,j−1})
//! reliability = H² + V² + D1² + D2²
//! ```
//!
//! `γ` is the near-range wrap in [`super::wrap::wrap`]. The full-plane finite
//! check runs separately: borders never enter this stencil.

use rayon::prelude::*;

use crate::precision::Real;

use super::wrap::wrap;

pub(super) fn calculate_reliability<T: Real>(
    phase: &[T],
    reliability: &mut [T],
    width: usize,
    height: usize,
) {
    reliability
        .par_iter_mut()
        .for_each(|slot| *slot = T::max_value());
    if width < 3 || height < 3 {
        return;
    }
    let width_plus_one = width + 1;
    let width_minus_one = width - 1;
    reliability
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(row, dest)| {
            if row == 0 || row + 1 >= height {
                return;
            }
            for (column, slot) in dest.iter_mut().enumerate().take(width_minus_one).skip(1) {
                let idx = row * width + column;
                let wip = phase[idx];
                let h = wrap(phase[idx - 1] - wip) - wrap(wip - phase[idx + 1]);
                let v = wrap(phase[idx - width] - wip) - wrap(wip - phase[idx + width]);
                let d1 = wrap(phase[idx - width_plus_one] - wip)
                    - wrap(wip - phase[idx + width_plus_one]);
                let d2 = wrap(phase[idx - width_minus_one] - wip)
                    - wrap(wip - phase[idx + width_minus_one]);
                *slot = h * h + v * v + d1 * d1 + d2 * d2;
            }
        });
}
