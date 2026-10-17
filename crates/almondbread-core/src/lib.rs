//! almondbread-core: platform-neutral application core (kb: decisions/rust-core).
//!
//! R1 provides the reference pipeline that the FFI exposes: parse a view centre, choose
//! the precision, select a reference (nucleus or centre) and compute its orbit. R2a adds
//! the viewport, render settings, palettes, the f64 reference orbit, statistics-driven
//! iteration control, the CPU oracle and verify metrics, the benchmark locations, PNG
//! output and the navigation (input/motion) model every UI forwards its events to.
//! No platform code here: it must build for Apple, Linux and Windows.

#![forbid(unsafe_code)]

pub use almondbread_math as math;

pub mod autoiter;
pub mod autopilot;
pub mod bla;
pub mod cfmt;
pub mod cinematic;
pub mod compass;
pub mod components;
pub mod cosmic;
pub mod cruise;
pub mod cycles;
pub mod flight;
pub mod flyto;
pub mod image;
pub mod input;
pub mod journey;
pub mod la;
pub mod locations;
pub mod oracle;
pub mod orbit;
pub mod palette;
pub mod palette_import;
pub mod palette_library;
pub mod pi_moment;
pub mod places;
pub mod reference;
pub mod seek;
pub mod settings;
pub mod tour;
pub mod viewport;

#[cfg(test)]
mod extreme_tests;

pub use input::{EdgePan, InputEvent, InputMode, NavKey, Navigator};
pub use locations::{BENCHMARK_LOCATIONS, BenchmarkLocation};
pub use oracle::{IterationField, VerifyReport};
pub use palette::Palette;
pub use places::{Place, PlaceCategory};
pub use settings::{InteriorMode, RenderSettings, Supersampling};
pub use viewport::{Center, Viewport};

use almondbread_math::{
    BigComplex, BigFloat, Control, FloatExpComplex, OrbitResult, PeriodMethod, ReferenceChoice, ReferenceOrbitComputer,
};

/// A reference orbit in the two GPU forms (f32 pairs and shared-exponent floatexp).
#[derive(Clone, Debug)]
pub struct ReferenceOrbit {
    pub points: Vec<[f32; 2]>,
    pub extended: Vec<FloatExpComplex>,
    pub result: OrbitResult,
}

/// Computes the orbit of `c` at `precision_bits` (rounded up to whole limbs).
pub fn compute_reference_orbit(
    c: &BigComplex,
    precision_bits: usize,
    max_iterations: usize,
    bailout: f64,
    control: &mut Control<'_>,
) -> ReferenceOrbit {
    let comp = ReferenceOrbitComputer::new(c, precision_bits);
    let mut points = vec![[0.0f32; 2]; max_iterations + 1];
    let mut extended = vec![FloatExpComplex::ZERO; max_iterations + 1];
    let result = comp.compute(max_iterations, bailout, Some(&mut points), Some(&mut extended), control);
    points.truncate(result.count);
    extended.truncate(result.count);
    ReferenceOrbit {
        points,
        extended,
        result,
    }
}

/// Chooses the reference for a view centred at `center` with the given half-diagonal.
pub fn select_reference(
    center: &BigComplex,
    radius: &BigFloat,
    max_iterations: usize,
    precision_bits: Option<usize>,
    method: PeriodMethod,
    is_cancelled: &mut dyn FnMut() -> bool,
) -> ReferenceChoice {
    almondbread_math::select_reference(center, radius, max_iterations, precision_bits, method, is_cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_pipeline() {
        let c = BigComplex::parse("-1.752", "0.0005", 128).unwrap();
        let choice = select_reference(
            &c,
            &BigFloat::from_f64(0.01, 64),
            1000,
            None,
            PeriodMethod::Ball,
            &mut || false,
        );
        assert!(choice.is_nucleus);
        let orbit = almondbread_math::with_no_control(4096, |ctl| {
            compute_reference_orbit(&choice.point, 128, choice.period.unwrap(), 2.0, ctl)
        });
        assert_eq!(orbit.result.iterations(), 3);
        assert_eq!(orbit.points.len(), 4);
    }
}
