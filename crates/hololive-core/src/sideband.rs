use std::ops::Range;

use num_complex::Complex;
use rayon::prelude::*;

use crate::{
    CoreError,
    dsp::fft2::{fftshift_segments, shifted_to_unshifted},
    precision::Real,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchRegion {
    Upper,
    Lower,
    Left,
    Right,
}

impl SearchRegion {
    pub const ALL: [Self; 4] = [Self::Upper, Self::Lower, Self::Left, Self::Right];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Upper => "Upper",
            Self::Lower => "Lower",
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }

    #[must_use]
    pub const fn compass_label(self) -> &'static str {
        match self {
            Self::Upper => "N",
            Self::Lower => "S",
            Self::Left => "W",
            Self::Right => "E",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sideband {
    pub x: usize,
    pub y: usize,
}

impl Sideband {
    #[must_use]
    pub fn distance_from_center(self, width: usize, height: usize) -> f64 {
        let (dx, dy) = self.offset_from_center(width, height);
        dx.hypot(dy)
    }

    #[inline]
    #[must_use]
    pub(crate) fn offset_from_center(self, width: usize, height: usize) -> (f64, f64) {
        (
            self.x as f64 - (width / 2) as f64,
            self.y as f64 - (height / 2) as f64,
        )
    }

    #[must_use]
    pub fn max_crop_radius(self, width: usize, height: usize) -> usize {
        let to_right = width.saturating_sub(self.x);
        let to_bottom = height.saturating_sub(self.y);
        self.x.min(self.y).min(to_right).min(to_bottom)
    }

    #[must_use]
    pub fn clamp_to_crop(self, width: usize, height: usize, radius: usize) -> Self {
        let clamp_axis = |coord: usize, len: usize| {
            let hi = len.saturating_sub(radius);
            let lo = radius.min(hi);
            coord.clamp(lo, hi)
        };
        Self {
            x: clamp_axis(self.x, width),
            y: clamp_axis(self.y, height),
        }
    }
}

pub(crate) fn locate<T: Real>(
    spectrum: &[Complex<T>],
    width: usize,
    height: usize,
    region: SearchRegion,
    center_mask_percent: f32,
) -> Result<Sideband, CoreError> {
    let center_x = (width / 2) as f64;
    let center_y = (height / 2) as f64;
    let radius = width.min(height) as f64 * f64::from(center_mask_percent) / 100.0;
    let radius_sq = radius * radius;
    let rows = region_rows(height, region);
    let columns = region_columns(width, region);
    let best = rows
        .into_par_iter()
        .filter_map(|y| {
            let source_y = shifted_to_unshifted(y, height);
            let source = &spectrum[source_y * width..(source_y + 1) * width];
            let dy = y as f64 - center_y;
            row_peak(
                source,
                y,
                width,
                columns.clone(),
                center_x,
                dy * dy,
                radius_sq,
            )
        })
        .reduce_with(|left, right| {
            if right.0 > left.0 || right.0 == left.0 && right.1 < left.1 {
                right
            } else {
                left
            }
        });
    best.map(|(_, index)| Sideband {
        x: index % width,
        y: index / width,
    })
    .ok_or(CoreError::SidebandNotFound)
}

fn region_rows(height: usize, region: SearchRegion) -> Range<usize> {
    match region {
        SearchRegion::Upper => 0..height / 2,
        SearchRegion::Lower => height / 2..height,
        SearchRegion::Left | SearchRegion::Right => 0..height,
    }
}

fn region_columns(width: usize, region: SearchRegion) -> Range<usize> {
    match region {
        SearchRegion::Left => 0..width / 2,
        SearchRegion::Right => width / 2..width,
        SearchRegion::Upper | SearchRegion::Lower => 0..width,
    }
}

fn row_peak<T: Real>(
    source: &[Complex<T>],
    y: usize,
    width: usize,
    columns: Range<usize>,
    center_x: f64,
    dy_sq: f64,
    radius_sq: f64,
) -> Option<(T, usize)> {
    let (first_segment, shift) = fftshift_segments(width);
    let mut best = None;
    let mut visit = |unshifted: &[Complex<T>], display_start: usize, display_end: usize| {
        let start = display_start.max(columns.start);
        let end = display_end.min(columns.end);
        if start >= end {
            return;
        }
        for (offset, value) in unshifted[start - display_start..end - display_start]
            .iter()
            .enumerate()
        {
            let x = start + offset;
            let dx = x as f64 - center_x;
            if dx.mul_add(dx, dy_sq) < radius_sq {
                continue;
            }
            let magnitude = value.norm_sqr();
            if !magnitude.is_finite() {
                continue;
            }
            if best.is_none_or(|(current, _)| magnitude > current) {
                best = Some((magnitude, x));
            }
        }
    };
    visit(&source[shift..], 0, first_segment);
    visit(&source[..shift], first_segment, width);
    best.map(|(magnitude, x)| (magnitude, y * width + x))
}

pub(crate) fn magnitude<T: Real>(value: Complex<T>) -> f64 {
    value.norm().as_f64()
}
