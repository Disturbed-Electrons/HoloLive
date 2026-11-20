//! Second-difference pixel reliability for edge sorting (Arevalillo *et al.*, 2002).
//!
//! Lower reliability is better. Interior pixels only: the H/V/D1/D2 stencil
//! needs a one-pixel border, which gets `T::max_value()` so those pixels are
//! merged last. Each row writes a disjoint slice of the reliability buffer.
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
    let border = T::max_value();
    reliability
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(row, dest)| {
            if width < 3 || row == 0 || row + 1 == height {
                dest.fill(border);
                return;
            }
            dest[0] = border;
            dest[width - 1] = border;
            let (above, rest) = phase[(row - 1) * width..(row + 2) * width].split_at(width);
            let (middle, below) = rest.split_at(width);
            for (((slot, top), middle), bottom) in dest[1..width - 1]
                .iter_mut()
                .zip(above.array_windows::<3>())
                .zip(middle.array_windows::<3>())
                .zip(below.array_windows::<3>())
            {
                let &[nw, n, ne] = top;
                let &[w, center, e] = middle;
                let &[sw, s, se] = bottom;
                let h = wrap(w - center) - wrap(center - e);
                let v = wrap(n - center) - wrap(center - s);
                let d1 = wrap(nw - center) - wrap(center - se);
                let d2 = wrap(ne - center) - wrap(center - sw);
                *slot = h * h + v * v + d1 * d1 + d2 * d2;
            }
        });
}
