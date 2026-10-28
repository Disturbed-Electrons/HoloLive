use std::f64::consts::PI;

use rayon::prelude::*;

use crate::{CoreError, checked_len};

/// Symmetric one-dimensional Tukey window.
pub fn tukey(length: usize, alpha: f64) -> Result<Vec<f64>, CoreError> {
    if !(0.0..=1.0).contains(&alpha) {
        return Err(CoreError::InvalidSetting("Tukey alpha must be in [0, 1]"));
    }
    if length == 0 {
        return Err(CoreError::EmptyDimensions);
    }
    if length == 1 || alpha == 0.0 {
        return Ok(vec![1.0; length]);
    }
    let last = (length - 1) as f64;
    let taper = alpha * last / 2.0;
    Ok((0..length)
        .map(|index| {
            let distance = index.min(length - 1 - index) as f64;
            if distance < taper {
                0.5 * (1.0 - (PI * (distance / taper)).cos())
            } else {
                1.0
            }
        })
        .collect())
}

/// Square radial Tukey window, linearly interpolated from a symmetric
/// one-dimensional window. Two-pixel crops are left
/// unwindowed because their sampled Tukey window would be entirely zero.
pub fn radial_tukey(side: usize, alpha: f64) -> Result<Vec<f64>, CoreError> {
    let len = checked_len(side, side)?;
    let window = tukey(side, alpha)?;
    if side <= 2 {
        return Ok(vec![1.0; len]);
    }
    let center = (side as f64 - 1.0) / 2.0;
    Ok((0..len)
        .map(|index| {
            let x = (index % side) as f64 - center;
            let y = (index / side) as f64 - center;
            let position = (x * x + y * y).sqrt() + center;
            let lower = position.floor() as usize;
            let fraction = position - lower as f64;
            let first = window.get(lower).copied().unwrap_or(0.0);
            let second = window.get(lower + 1).copied().unwrap_or(0.0);
            first * (1.0 - fraction) + second * fraction
        })
        .collect())
}

/// Unshifted FFT buffer (DC at index 0).
pub fn butterworth(
    width: usize,
    height: usize,
    cutoff: f64,
    order: i32,
    high_pass: bool,
    squared: bool,
) -> Result<Vec<f64>, CoreError> {
    let len = checked_len(width, height)?;
    if !cutoff.is_finite() || cutoff <= 0.0 || order <= 0 {
        return Err(CoreError::InvalidSetting(
            "Butterworth cutoff and order must be positive",
        ));
    }
    let fx_scale = width as f64 * cutoff;
    let fy_scale = height as f64 * cutoff;
    let fx: Vec<f64> = (0..width).map(|x| fft_coord(x, width) / fx_scale).collect();
    let mut mask = vec![0.0; len];
    mask.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        let fy = fft_coord(y, height) / fy_scale;
        let fy_sq = fy * fy;
        for (x, value) in row.iter_mut().enumerate() {
            *value = butterworth_sample(fx[x], fy_sq, order, high_pass, squared);
        }
    });
    Ok(mask)
}

fn fft_coord(index: usize, len: usize) -> f64 {
    if index <= len / 2 {
        index as f64
    } else {
        index as f64 - len as f64
    }
}

fn butterworth_sample(fx: f64, fy_sq: f64, order: i32, high_pass: bool, squared: bool) -> f64 {
    let radius_sq = fx.mul_add(fx, fy_sq);
    let q = radius_sq.powi(order);
    let mut value = if !high_pass {
        1.0 / (1.0 + q)
    } else if q <= 1.0 {
        q / (1.0 + q)
    } else {
        1.0 / (1.0 + q.recip())
    };
    if !squared {
        value = value.sqrt();
    }
    value
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineMaskSettings {
    pub thickness: usize,
    pub smoothness: f64,
    pub distance_percent: f64,
    pub threshold: Option<f64>,
}

impl Default for LineMaskSettings {
    fn default() -> Self {
        Self {
            thickness: 5,
            smoothness: 1.0,
            distance_percent: 85.0,
            threshold: None,
        }
    }
}

/// Fftshifted display coordinates (DC at the array center).
pub fn fresnel_mask(
    width: usize,
    height: usize,
    center: (f64, f64),
    sideband: (f64, f64),
    settings: LineMaskSettings,
) -> Result<Vec<f64>, CoreError> {
    let mut mask = vec![1.0; checked_len(width, height)?];
    let dx = (sideband.0 - center.0) * settings.distance_percent / 100.0;
    let dy = (sideband.1 - center.1) * settings.distance_percent / 100.0;
    draw_line(
        &mut mask,
        width,
        height,
        center,
        (center.0 + dx, center.1 + dy),
        settings.thickness,
    );
    draw_line(
        &mut mask,
        width,
        height,
        center,
        (center.0 - dx, center.1 - dy),
        settings.thickness,
    );
    smooth_and_threshold(mask, width, height, settings.smoothness, settings.threshold)
}

/// Fftshifted display coordinates (DC at the array center).
pub fn gibbs_mask(
    width: usize,
    height: usize,
    center: (f64, f64),
    settings: LineMaskSettings,
) -> Result<Vec<f64>, CoreError> {
    let mut mask = vec![1.0; checked_len(width, height)?];
    let fraction = settings.distance_percent / 100.0;
    for endpoint in compass_endpoints(center, width as f64, height as f64, fraction) {
        draw_line(
            &mut mask,
            width,
            height,
            center,
            endpoint,
            settings.thickness,
        );
    }
    smooth_and_threshold(mask, width, height, settings.smoothness, settings.threshold)
}

fn compass_endpoints(
    center: (f64, f64),
    width: f64,
    height: f64,
    fraction: f64,
) -> [(f64, f64); 4] {
    let (cx, cy) = center;
    let north = (cx, cy - cy * fraction);
    let south = (cx, cy + (height - 1.0 - cy) * fraction);
    let west = (cx - cx * fraction, cy);
    let east = (cx + (width - 1.0 - cx) * fraction, cy);
    [north, south, west, east]
}

fn draw_line(
    mask: &mut [f64],
    width: usize,
    height: usize,
    start: (f64, f64),
    end: (f64, f64),
    thickness: usize,
) {
    let (mut x0, mut y0) = (start.0.round() as isize, start.1.round() as isize);
    let (x1, y1) = (end.0.round() as isize, end.1.round() as isize);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let step_x = if x0 < x1 { 1 } else { -1 };
    let step_y = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    let radius = thickness.saturating_sub(1).div_ceil(2) as isize;
    loop {
        stamp_square(mask, width, height, x0, y0, radius);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x0 += step_x;
        }
        if twice <= dx {
            error += dx;
            y0 += step_y;
        }
    }
}

fn stamp_square(
    mask: &mut [f64],
    width: usize,
    height: usize,
    cx: isize,
    cy: isize,
    radius: isize,
) {
    for oy in -radius..=radius {
        for ox in -radius..=radius {
            let Some(index) = in_bounds(cx + ox, cy + oy, width, height) else {
                continue;
            };
            mask[index] = 0.0;
        }
    }
}

fn in_bounds(x: isize, y: isize, width: usize, height: usize) -> Option<usize> {
    if x < 0 || y < 0 {
        return None;
    }
    let x = x as usize;
    let y = y as usize;
    if x < width && y < height {
        Some(y * width + x)
    } else {
        None
    }
}

fn smooth_and_threshold(
    mut mask: Vec<f64>,
    width: usize,
    height: usize,
    sigma: f64,
    threshold: Option<f64>,
) -> Result<Vec<f64>, CoreError> {
    if !sigma.is_finite() {
        return Err(CoreError::InvalidSetting(
            "filter smoothness must be finite",
        ));
    }
    if sigma > 0.0 {
        let kernel = super::blur::gaussian_kernel(sigma, 3.0);
        let mut temporary = vec![0.0; mask.len()];
        super::blur::convolve_horizontal(&mask, &mut temporary, width, height, &kernel);
        super::blur::convolve_vertical(&temporary, &mut mask, width, height, &kernel);
    }
    if let Some(threshold) = threshold.filter(|value| *value > 0.0) {
        for value in &mut mask {
            if *value < threshold {
                *value = 0.0;
            }
        }
    }
    Ok(mask)
}
