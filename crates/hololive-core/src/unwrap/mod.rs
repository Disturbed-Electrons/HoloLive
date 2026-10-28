//! Arevalillo–Herráez 2002 second-difference path following.
//!
//! Live reconstruction stores wrapped phase in the extract plane and unwraps
//! it in place. There is no quality-map mask and no periodic wrap-around: every
//! 4-neighbour adjacency is an edge. Pixel reliability is a separate SoA
//! buffer so FFT-sized `Pixel` records stay to union–find links and the `i32`
//! wrap count.
//!
//! Pixel reset, the quality field and the edge arrays are built in parallel,
//! and the edges are sorted with a parallel **stable** sort, which yields the
//! same order as a serial one, so equal-quality ties keep insertion order
//! (horizontals, then verticals, both row-major). Kruskal merging
//! ([`edge::gather_pixels`]) stays serial. Final phase conversion and min–max
//! reduction run together in parallel. The result is unique up to a global
//! additive multiple of 2π when the wrapped field is integrable.

mod edge;
mod pixel;
mod reliability;
mod wrap;

use rayon::prelude::*;

use crate::{CoreError, Real, checked_len, error::expect_len};

use edge::{
    Edge, adjacency_edge_count, gather_pixels, horizontal_edge_count, horizontal_edges, sort_edges,
    vertical_edges,
};
use pixel::{Pixel, initialize_pixels, unwrap_image};
use reliability::calculate_reliability;

pub(crate) use wrap::wrap_to_pi;

pub(crate) struct UnwrapWorkspace<T: Real> {
    pixels: Vec<Pixel>,
    reliability: Vec<T>,
    edges: Vec<Edge<T>>,
}

impl<T: Real> Default for UnwrapWorkspace<T> {
    fn default() -> Self {
        Self {
            pixels: Vec::new(),
            reliability: Vec::new(),
            edges: Vec::new(),
        }
    }
}

impl<T: Real> UnwrapWorkspace<T> {
    fn prepare(&mut self, width: usize, height: usize) -> Result<(), CoreError> {
        let image_size = checked_len(width, height)?;
        let no_of_edges = adjacency_edge_count(width, height);
        initialize_pixels(&mut self.pixels, image_size);
        if self.reliability.len() < image_size {
            self.reliability.resize(image_size, T::max_value());
        }
        if self.edges.len() < no_of_edges {
            self.edges.resize_with(no_of_edges, Edge::new);
        }
        Ok(())
    }
}

pub(crate) fn unwrap_into<T: Real>(
    phase: &mut [T],
    width: usize,
    height: usize,
    workspace: &mut UnwrapWorkspace<T>,
) -> Result<(T, T), CoreError> {
    let expected = checked_len(width, height)?;
    expect_len(phase.len(), expected)?;
    if !phase.par_iter().all(|value| value.is_finite()) {
        return Err(CoreError::NonFiniteInput);
    }
    workspace.prepare(width, height)?;
    Ok(unwrap_2d(phase, workspace, width, height))
}

fn unwrap_2d<T: Real>(
    phase: &mut [T],
    workspace: &mut UnwrapWorkspace<T>,
    width: usize,
    height: usize,
) -> (T, T) {
    let image_size = width * height;
    let n_edges = adjacency_edge_count(width, height);
    let pixels = &mut workspace.pixels[..image_size];
    let reliability = &mut workspace.reliability[..image_size];
    let edges = &mut workspace.edges[..n_edges];
    let (horizontal, vertical) = edges.split_at_mut(horizontal_edge_count(width, height));

    calculate_reliability(phase, reliability, width, height);
    horizontal_edges(phase, reliability, horizontal, width);
    vertical_edges(phase, reliability, vertical, width);
    sort_edges(edges);
    gather_pixels(edges, pixels);
    unwrap_image(pixels, phase)
}
