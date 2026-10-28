use num_complex::Complex;
use rayon::prelude::*;

use crate::{
    dsp::fft2::{fftshift_segments, shifted_to_unshifted},
    precision::Real,
};

use super::ReconstructionEngine;

impl<T: Real> ReconstructionEngine<T> {
    pub fn write_spectrum(&self, spectrum: &mut Vec<f32>) -> Option<(usize, usize)> {
        let (_, width, height) = self.forward_spectrum?;
        spectrum.resize(self.forward.samples.len(), 0.0);
        spectrum
            .par_chunks_mut(width)
            .enumerate()
            .for_each(|(y, row)| {
                let source_y = shifted_to_unshifted(y, height);
                let source = &self.forward.samples[source_y * width..(source_y + 1) * width];
                write_ln_norm_fftshifted_row(source, row);
            });
        Some((width, height))
    }
}

fn write_ln_norm_fftshifted_row<T: Real>(source: &[Complex<T>], row: &mut [f32]) {
    debug_assert_eq!(source.len(), row.len());
    let (first_segment, shift) = fftshift_segments(source.len());
    write_ln_norm(&source[shift..], &mut row[..first_segment]);
    write_ln_norm(&source[..shift], &mut row[first_segment..]);
}

#[inline]
fn write_ln_norm<T: Real>(source: &[Complex<T>], row: &mut [f32]) {
    for (output, &value) in row.iter_mut().zip(source) {
        *output = value.norm().as_f32().ln_1p();
    }
}
