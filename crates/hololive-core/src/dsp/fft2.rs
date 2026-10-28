use std::sync::Arc;

use num_complex::Complex;
use rayon::prelude::*;
use rustfft::{Fft, FftDirection, FftNum, FftPlanner};

use crate::{CoreError, checked_len, error::expect_len};

/// Maps a fftshifted display index to the corresponding unshifted FFT buffer index.
#[inline]
#[must_use]
pub(crate) const fn shifted_to_unshifted(index: usize, len: usize) -> usize {
    (index + len.div_ceil(2)) % len
}

/// Returns `(first_segment_len, shift)` for horizontal fftshift on a row of length `len`.
pub(crate) const fn fftshift_segments(len: usize) -> (usize, usize) {
    let shift = len.div_ceil(2);
    (len - shift, shift)
}

pub struct Fft2<T: FftNum + Send + Sync> {
    width: usize,
    height: usize,
    row: Arc<dyn Fft<T>>,
    column: Arc<dyn Fft<T>>,
    transpose: Vec<Complex<T>>,
    row_scratch: Vec<Vec<Complex<T>>>,
    column_scratch: Vec<Vec<Complex<T>>>,
}

impl<T: FftNum + Send + Sync> Fft2<T> {
    pub fn new(width: usize, height: usize, direction: FftDirection) -> Result<Self, CoreError> {
        let len = checked_len(width, height)?;
        let mut planner = FftPlanner::new();
        let row = planner.plan_fft(width, direction);
        let column = planner.plan_fft(height, direction);
        let workers = rayon::current_num_threads().min(height.max(width)).max(1);
        Ok(Self {
            width,
            height,
            row_scratch: scratch_pool(workers, row.get_inplace_scratch_len()),
            column_scratch: scratch_pool(workers, column.get_inplace_scratch_len()),
            row,
            column,
            transpose: vec![Complex::new(T::zero(), T::zero()); len],
        })
    }

    pub fn process(&mut self, values: &mut [Complex<T>]) -> Result<(), CoreError> {
        expect_len(values.len(), self.width * self.height)?;
        transform_rows(values, self.width, &self.row, &mut self.row_scratch);
        transpose(values, &mut self.transpose, self.width, self.height);
        transform_rows(
            &mut self.transpose,
            self.height,
            &self.column,
            &mut self.column_scratch,
        );
        transpose(&self.transpose, values, self.height, self.width);
        Ok(())
    }
}

/// Smallest length ≥ `requested` whose prime factors are in `{2, 3, 5, 7, 11}`.
#[must_use]
pub fn next_fast_len(mut requested: usize) -> usize {
    requested = requested.max(1);
    loop {
        let mut remainder = requested;
        for radix in [2, 3, 5, 7, 11] {
            while remainder.is_multiple_of(radix) {
                remainder /= radix;
            }
        }
        if remainder == 1 {
            return requested;
        }
        requested += 1;
    }
}

fn scratch_pool<T: FftNum>(workers: usize, len: usize) -> Vec<Vec<Complex<T>>> {
    (0..workers)
        .map(|_| vec![Complex::new(T::zero(), T::zero()); len])
        .collect()
}

fn transform_rows<T: FftNum + Send + Sync>(
    values: &mut [Complex<T>],
    row_len: usize,
    fft: &Arc<dyn Fft<T>>,
    scratch: &mut [Vec<Complex<T>>],
) {
    let rows = values.len() / row_len;
    let rows_per_worker = rows.div_ceil(scratch.len());
    values
        .par_chunks_mut(rows_per_worker * row_len)
        .zip(scratch.par_iter_mut())
        .for_each(|(block, scratch)| {
            for row in block.chunks_exact_mut(row_len) {
                fft.process_with_scratch(row, scratch);
            }
        });
}

fn transpose<T: Copy + Send + Sync>(
    source: &[T],
    destination: &mut [T],
    width: usize,
    height: usize,
) {
    const TILE: usize = 32;
    destination
        .par_chunks_mut(height * TILE)
        .enumerate()
        .for_each(|(tile_index, columns)| {
            let start_x = tile_index * TILE;
            let tile_width = columns.len() / height;
            for start_y in (0..height).step_by(TILE) {
                let tile_height = TILE.min(height - start_y);
                for local_x in 0..tile_width {
                    let x = start_x + local_x;
                    let destination_column = &mut columns
                        [local_x * height + start_y..local_x * height + start_y + tile_height];
                    for (local_y, value) in destination_column.iter_mut().enumerate() {
                        *value = source[(start_y + local_y) * width + x];
                    }
                }
            }
        });
}
