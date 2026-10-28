//! Shared Gaussian kernel and separable convolution used by the spectral line masks in `filters`.

use rayon::prelude::*;

/// Build a normalized 1-D Gaussian kernel.
///
/// The kernel radius is `ceil(truncate * sigma)` samples on each side.
#[must_use]
pub fn gaussian_kernel(sigma: f64, truncate: f64) -> Vec<f64> {
    let two_sigma_sq = 2.0 * sigma * sigma;
    if !sigma.is_finite() || sigma <= 0.0 || two_sigma_sq == 0.0 {
        return vec![1.0];
    }
    let radius = (truncate * sigma).ceil() as usize;
    let len = 2 * radius + 1;
    let mut kernel: Vec<f64> = (0..len)
        .map(|index| {
            let offset = index as f64 - radius as f64;
            (-offset * offset / two_sigma_sq).exp()
        })
        .collect();
    let sum: f64 = kernel.iter().sum();
    if sum > 0.0 {
        let scale = 1.0 / sum;
        kernel.iter_mut().for_each(|weight| *weight *= scale);
    }
    kernel
}

pub fn convolve_horizontal(
    source: &[f64],
    destination: &mut [f64],
    width: usize,
    height: usize,
    kernel: &[f64],
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), width * height);
    if kernel.is_empty() || width == 0 {
        destination.fill(0.0);
        return;
    }
    let radius = kernel.len() / 2;
    let width_i = width as i32;
    source
        .par_chunks(width)
        .zip(destination.par_chunks_mut(width))
        .for_each(|(source_row, destination_row)| {
            for x in 0..width {
                destination_row[x] = convolve_symmetric(kernel, radius, source_row[x], |offset| {
                    source_row[reflect_index(x as i32 + offset, width_i)]
                });
            }
        });
}

pub fn convolve_vertical(
    source: &[f64],
    destination: &mut [f64],
    width: usize,
    height: usize,
    kernel: &[f64],
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), width * height);
    if kernel.is_empty() || width == 0 || height == 0 {
        destination.fill(0.0);
        return;
    }
    let radius = kernel.len() / 2;
    let height_i = height as i32;
    destination
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, destination_row)| {
            for x in 0..width {
                let center = source[y * width + x];
                destination_row[x] = convolve_symmetric(kernel, radius, center, |offset| {
                    let sample_y = reflect_index(y as i32 + offset, height_i);
                    source[sample_y * width + x]
                });
            }
        });
}

fn convolve_symmetric(
    kernel: &[f64],
    radius: usize,
    center: f64,
    sample: impl Fn(i32) -> f64,
) -> f64 {
    let mut sum = center * kernel[radius];
    for k in 1..=radius {
        let weight = kernel[radius + k];
        let left = sample(-(k as i32));
        let right = sample(k as i32);
        sum += (left + right) * weight;
    }
    sum
}

/// Symmetric boundary reflection without repeating the edge sample.
///
/// Indices outside `[0, len)` are mirrored back into range.
#[inline]
fn reflect_index(index: i32, len: i32) -> usize {
    if len <= 1 {
        return 0;
    }
    if (0..len).contains(&index) {
        return index as usize;
    }
    let period = 2 * (len - 1);
    let folded = index.rem_euclid(period);
    if folded >= len {
        (period - folded) as usize
    } else {
        folded as usize
    }
}
