//! Union–find pixel groups and the final in-place unwrap pass.
//!
//! Wrapped phase stays in the dense extract plane; second-difference quality
//! lives in a parallel [`super::UnwrapWorkspace::reliability`] buffer so the
//! hot loops do not drag `Pixel` through cache. Image sizes are capped by
//! [`crate::error::MAX_IMAGE_PIXELS`], so group links fit in `u32`.
//!
//! [`Pixel::next`] uses [`NO_NEXT`] instead of a signed sentinel so the struct
//! stays 20 bytes (`i32` wrap count + four `u32` fields). Adjacency edges live
//! in [`super::edge`].

use rayon::prelude::*;

use crate::precision::Real;

use super::wrap::TWOPI;

/// Intrusive-list terminator for [`Pixel::next`].
pub(super) const NO_NEXT: u32 = u32::MAX;

pub(super) struct Pixel {
    pub(super) increment: i32,
    pub(super) number_of_pixels_in_group: u32,
    pub(super) head: u32,
    pub(super) last: u32,
    pub(super) next: u32,
}

impl Pixel {
    fn new(index: usize) -> Self {
        Self {
            increment: 0,
            number_of_pixels_in_group: 1,
            head: index as u32,
            last: index as u32,
            next: NO_NEXT,
        }
    }
}

/// Reset reused pixels to singleton groups and initialize new slots directly.
/// Both passes are parallel; slots beyond the active image remain allocated.
pub(super) fn initialize_pixels(pixels: &mut Vec<Pixel>, image_size: usize) {
    let reused = pixels.len().min(image_size);
    pixels[..reused]
        .par_iter_mut()
        .enumerate()
        .for_each(|(index, pixel)| *pixel = Pixel::new(index));
    if reused < image_size {
        pixels.par_extend((reused..image_size).into_par_iter().map(Pixel::new));
    }
}

/// Write `wrapped + 2π · increment` in place and return that plane's min and max.
/// Pixels are independent: each keeps its arithmetic order, and the parallel
/// reduction only compares their unwrapped values.
pub(super) fn unwrap_image<T: Real>(pixels: &[Pixel], output: &mut [T]) -> (T, T) {
    let twopi = <T as Real>::from_f64(TWOPI);
    output
        .par_iter_mut()
        .zip(pixels.par_iter())
        .map(|(out, pixel)| {
            let value = *out + twopi * <T as Real>::from_i32(pixel.increment);
            *out = value;
            (value, value)
        })
        .reduce(
            || (T::max_value(), T::min_value()),
            |(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)),
        )
}
