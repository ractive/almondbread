//! almondbread-math: arbitrary-precision arithmetic and reference orbits for deep-zoom
//! Mandelbrot rendering (kb: decisions/arbitrary-precision, decisions/rust-core).
//!
//! - [`BigFloat`] / [`BigComplex`]: correctly rounded multi-limb binary floating point
//!   (u64 limbs, i64 exponent), decimal parse and format.
//! - [`FloatExp`], [`FloatExpComplex`], [`DoubleExp`], [`DoubleExpComplex`]: extended-range
//!   types; `FloatExpComplex` has the MSL layout `{float2 m; int e;}`.
//! - [`ReferenceOrbitComputer`]: allocation-free reference orbit into caller buffers,
//!   specialised for 1...16 limbs.
//! - [`Formula`]: the iteration (F2: Mandelbrot plus the generic polynomial path).
//! - [`selection`]: period detection, Newton nuclei, minibrot size, reference choice.
//!
//! Pure Rust, no platform code, no GMP/MPFR (LGPL): builds for every target.

#![forbid(unsafe_code)]

mod bigfloat;
mod bignat;
mod complex;
mod decimal;
mod floatexp;
mod formula;
#[doc(hidden)]
pub mod limbs;
mod orbit;
mod parallel;
#[doc(hidden)]
pub mod raw;
pub mod rays;
pub mod selection;

#[cfg(test)]
mod extreme_tests;

pub use bigfloat::BigFloat;
pub use complex::BigComplex;
pub use floatexp::{DoubleExp, DoubleExpComplex, FloatExp, FloatExpComplex};
pub use formula::{Formula, JuliaK, MULTIBROT_MAX_POWER};
pub use orbit::{
    AUTOMATIC_MULTIPLICATION_LIMIT, Control, OrbitResult, ReferenceOrbitComputer, Threads, Variant,
    recommended_precision_bits, recommended_precision_bits_with, with_no_control,
};
pub use parallel::{
    MIN_PERFORMANCE_CORES, PARALLEL_MIN_ITERATIONS, PARALLEL_MIN_LIMBS, helpers_pay, set_performance_cores,
};
pub use rays::{BinaryAngle, RayOptions, trace_ray_inward};
pub use selection::{
    MisiurewiczResult, NewtonOptions, NucleusResult, PeriodMethod, ReferenceChoice, atom_domain_period,
    atom_domain_period_for, find_misiurewicz, find_nucleus, find_nucleus_for, find_period, find_period_for,
    find_period_from, find_period_from_for, minibrot_size, minibrot_size_cancellable, minibrot_size_cancellable_for,
    minibrot_size_complex, minibrot_size_complex_for, select_reference, select_reference_for,
};
