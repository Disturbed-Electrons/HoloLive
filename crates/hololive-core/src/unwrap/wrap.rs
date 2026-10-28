use crate::precision::Real;

#[inline]
fn pi<T: Real>() -> T {
    <T as Real>::from_f64(std::f64::consts::PI)
}

#[inline]
fn twopi<T: Real>() -> T {
    let pi = pi::<T>();
    pi + pi
}

pub(super) const TWOPI: f64 = 2.0 * std::f64::consts::PI;

/// Wrap a phase value to the principal range `[-π, π)`.
///
/// Uses modular arithmetic so arbitrary real inputs are mapped correctly.
#[inline]
pub(crate) fn wrap_to_pi<T: Real>(x: T) -> T {
    let pi = pi::<T>();
    let twopi = twopi::<T>();
    let mut wrapped = (x + pi) % twopi;
    if wrapped < T::zero() {
        wrapped = wrapped + twopi;
    }
    wrapped - pi
}

/// Fast wrap for values already near `[-π, π]`.
///
/// Correct for inputs in approximately `[-3π, 3π]`. Used where phase
/// differences stay small, such as second-difference quality estimation.
#[inline]
pub(super) fn wrap<T: Real>(pixel_value: T) -> T {
    let pi = pi::<T>();
    let twopi = twopi::<T>();
    if pixel_value > pi {
        pixel_value - twopi
    } else if pixel_value < -pi {
        pixel_value + twopi
    } else {
        pixel_value
    }
}

/// Sign of the 2π wrap between two wrapped phase values.
///
/// Returns `-1` when `pixel_l - pixel_r > π`, `1` when it is `< -π`, and `0`
/// otherwise; the result is never outside that range regardless of input.
#[inline]
pub(super) fn find_wrap<T: Real>(pixel_l: T, pixel_r: T) -> i32 {
    let difference = pixel_l - pixel_r;
    let pi = pi::<T>();
    if difference > pi {
        -1
    } else if difference < -pi {
        1
    } else {
        0
    }
}
