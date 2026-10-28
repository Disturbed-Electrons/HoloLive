//! Adjacency edges: the [`Edge`] record, row-parallel fill, reliability
//! sorting, and union–find group merging.
//!
//! Horizontal edges connect `(i, j)` to `(i, j+1)`; vertical edges connect
//! `(i, j)` to `(i+1, j)`. Each row writes a disjoint slice of the edge buffer
//! (`i · (width−1)` for horizontals, then `n_horizontal + i · width` for
//! verticals), so construction is row-parallel. Edge reliability is the
//! **sum** of endpoint pixel reliabilities.
//!
//! Edges are processed in **ascending** reliability after a **stable** parallel
//! sort ([`sort_edges`]), so equal keys keep insertion order: all horizontals
//! in row-major order, then all verticals. [`gather_pixels`] is Kruskal’s
//! algorithm and stays serial.
//! The unwrapped plane is unique up to a global 2π offset when the wrapped
//! field is integrable; ties do not change that case.

use rayon::prelude::*;

use crate::precision::Real;

use super::pixel::{NO_NEXT, Pixel};
use super::wrap::find_wrap;

/// Adjacency edge between two pixels, sorted by combined endpoint reliability.
///
/// 16 bytes for `f32`, 24 for `f64`; `increment` is an `i32` because a
/// narrower type would only become padding next to the `u32` indices.
pub(super) struct Edge<T: Real> {
    pub(super) reliab: T,
    pub(super) pointer_1: u32,
    pub(super) pointer_2: u32,
    /// Wrap from `pointer_1` toward `pointer_2` ([`super::wrap::find_wrap`],
    /// always −1, 0, or 1).
    pub(super) increment: i32,
}

impl<T: Real> Edge<T> {
    pub(super) fn new() -> Self {
        Self {
            reliab: T::max_value(),
            pointer_1: 0,
            pointer_2: 0,
            increment: 0,
        }
    }
}

/// Horizontal adjacencies: `height · (width − 1)`, the first block of the edge buffer.
pub(super) fn horizontal_edge_count(width: usize, height: usize) -> usize {
    width.saturating_sub(1).saturating_mul(height)
}

/// Vertical adjacencies: `width · (height − 1)`, the second block of the edge buffer.
pub(super) fn vertical_edge_count(width: usize, height: usize) -> usize {
    height.saturating_sub(1).saturating_mul(width)
}

/// Total 4-neighbour adjacencies (horizontal block followed by vertical block).
pub(super) fn adjacency_edge_count(width: usize, height: usize) -> usize {
    horizontal_edge_count(width, height).saturating_add(vertical_edge_count(width, height))
}

/// Fill one slot for the adjacency `first → second`.
#[inline]
fn write_edge<T: Real>(
    edge: &mut Edge<T>,
    first: usize,
    second: usize,
    phase: &[T],
    reliability: &[T],
) {
    edge.pointer_1 = first as u32;
    edge.pointer_2 = second as u32;
    edge.reliab = reliability[first] + reliability[second];
    edge.increment = find_wrap(phase[first], phase[second]);
}

/// Fill the horizontal block; `edges` must hold exactly
/// [`horizontal_edge_count`] slots.
pub(super) fn horizontal_edges<T: Real>(
    phase: &[T],
    reliability: &[T],
    edges: &mut [Edge<T>],
    width: usize,
) {
    let span = width.saturating_sub(1);
    if span == 0 {
        return;
    }
    edges
        .par_chunks_mut(span)
        .enumerate()
        .for_each(|(row, row_edges)| {
            let origin = row * width;
            for (column, edge) in row_edges.iter_mut().enumerate() {
                let left = origin + column;
                write_edge(edge, left, left + 1, phase, reliability);
            }
        });
}

/// Fill the vertical block; `edges` must hold exactly
/// [`vertical_edge_count`] slots.
pub(super) fn vertical_edges<T: Real>(
    phase: &[T],
    reliability: &[T],
    edges: &mut [Edge<T>],
    width: usize,
) {
    if width == 0 {
        return;
    }
    edges
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(row, row_edges)| {
            let origin = row * width;
            let below = origin + width;
            for (column, edge) in row_edges.iter_mut().enumerate() {
                write_edge(edge, origin + column, below + column, phase, reliability);
            }
        });
}

/// Sort edges by ascending combined reliability.
///
/// Rayon's `par_sort_by` is a stable merge sort. A stable sort's output is
/// fully determined by the input order and the comparator, so this yields the
/// same permutation as the serial `sort_by` on every thread count: equal
/// reliabilities keep fill order. Keys are finite (or `+∞` from two
/// `T::max_value()` endpoints), so the `partial_cmp` fallback never fires.
pub(super) fn sort_edges<T: Real>(edges: &mut [Edge<T>]) {
    edges.par_sort_by(|a, b| {
        a.reliab
            .partial_cmp(&b.reliab)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

pub(super) fn gather_pixels<T: Real>(edges: &[Edge<T>], pixels: &mut [Pixel]) {
    for edge in edges {
        let idx1 = edge.pointer_1 as usize;
        let idx2 = edge.pointer_2 as usize;
        if pixels[idx2].head == pixels[idx1].head {
            continue;
        }
        let head1 = pixels[idx1].head;
        let head2 = pixels[idx2].head;

        if pixels[idx2].next == NO_NEXT && pixels[idx2].head == edge.pointer_2 {
            let head1_last = pixels[head1 as usize].last;
            pixels[head1_last as usize].next = edge.pointer_2;
            pixels[head1 as usize].last = edge.pointer_2;
            pixels[head1 as usize].number_of_pixels_in_group += 1;
            pixels[idx2].head = head1;
            pixels[idx2].increment = pixels[idx1].increment - edge.increment;
        } else if pixels[idx1].next == NO_NEXT && pixels[idx1].head == edge.pointer_1 {
            let head2_last = pixels[head2 as usize].last;
            pixels[head2_last as usize].next = edge.pointer_1;
            pixels[head2 as usize].last = edge.pointer_1;
            pixels[head2 as usize].number_of_pixels_in_group += 1;
            pixels[idx1].increment = pixels[idx2].increment + edge.increment;
            pixels[idx1].head = head2;
        } else {
            let size1 = pixels[head1 as usize].number_of_pixels_in_group;
            let size2 = pixels[head2 as usize].number_of_pixels_in_group;
            if size1 > size2 {
                let head1_last = pixels[head1 as usize].last;
                pixels[head1_last as usize].next = head2;
                pixels[head1 as usize].last = pixels[head2 as usize].last;
                pixels[head1 as usize].number_of_pixels_in_group += size2;
                let delta = pixels[idx1].increment - edge.increment - pixels[idx2].increment;
                let mut current = head2;
                while current != NO_NEXT {
                    let curr_idx = current as usize;
                    pixels[curr_idx].head = head1;
                    pixels[curr_idx].increment += delta;
                    current = pixels[curr_idx].next;
                }
            } else {
                let head2_last = pixels[head2 as usize].last;
                pixels[head2_last as usize].next = head1;
                pixels[head2 as usize].last = pixels[head1 as usize].last;
                pixels[head2 as usize].number_of_pixels_in_group += size1;
                let delta = pixels[idx2].increment + edge.increment - pixels[idx1].increment;
                let mut current = head1;
                while current != NO_NEXT {
                    let curr_idx = current as usize;
                    pixels[curr_idx].head = head2;
                    pixels[curr_idx].increment += delta;
                    current = pixels[curr_idx].next;
                }
            }
        }
    }
}
