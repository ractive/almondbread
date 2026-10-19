//! What each fractal formula supports (F2, kb: plans/f2-pluggable-formulas,
//! synthesis/other-fractal-types §6 "Capabilities").
//!
//! The formula itself ([`Formula`]) lives in `almondbread-math`; this is the platform-free table
//! of capabilities the renderer gates on and the UIs (over the FFI, and the CLI) hide controls
//! by: what a formula lacks is hidden, never faked. Mandelbrot has everything. `Multibrot(p)` is
//! the generic polynomial path (test-only in F2; kb: formulas/multibrot): perturbation with
//! one rebased reference, the scalar BLA, local SA seeds, the derivative interior test and the
//! attracting-cycle probe, DE and the orbit overlay carry over; Imagina LA (power 2), the
//! closed-form cardioid and bulb test, the π meter, external rays and the Mandelbrot content
//! (places, tours, journeys) do not; seek (Newton framing of minibrots) waits for F3.

pub use almondbread_math::{Formula, MULTIBROT_MAX_POWER};

/// How a formula's references rebase (kb: techniques/rebasing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rebasing {
    /// One reference orbit through the critical point 0 (Mandelbrot, Multibrot, Burning Ship).
    One,
    /// One reference per phase of a hybrid's loop (F7).
    PerPhase,
    /// A view-centre orbit, then the critical orbit (deep Julia, F4).
    CriticalOrbit,
}

/// The BLA form (kb: techniques/bla).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlaForm {
    /// Complex `A` (conformal formulas).
    Scalar,
    /// 2x2 real matrices (Mandelbar, Burning Ship; F5).
    Matrix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormulaCapabilities {
    /// Perturbation against a high-precision reference (deep zoom).
    pub perturbation: bool,
    pub rebasing: Rebasing,
    pub bla: BlaForm,
    /// Imagina-style period-aligned LA (power 2 only; kb: techniques/linear-approximation-imagina).
    pub la: bool,
    /// Local series-approximation seeds (M5; off for folded formulas).
    pub sa_seeds: bool,
    /// The `|dz/dz1| n^2` interior test.
    pub interior_derivative: bool,
    /// The attracting-cycle probe and Component Newton (kb: techniques/interior-detection).
    pub interior_probe: bool,
    /// Closed-form main cardioid and period-2 bulb test (Mandelbrot only).
    pub closed_form_components: bool,
    /// Exterior distance estimate (lighting, outlines, glow).
    pub de: bool,
    /// Seek: Newton framing of the nearest minibrot.
    pub seek: bool,
    /// Atom-domain periods (interior colourings by period, the overlay's period).
    pub atom_domain: bool,
    /// The π meter (kb: techniques/pi-in-the-mandelbrot-set).
    pub pi_meter: bool,
    /// External rays (`almondbread_math::rays`).
    pub external_rays: bool,
    /// The orbit overlay and the whole-set map's dot cloud.
    pub orbit_overlay: bool,
    /// Places, tours and journeys (all Mandelbrot content).
    pub content: bool,
}

impl FormulaCapabilities {
    pub const MANDELBROT: Self = Self {
        perturbation: true,
        rebasing: Rebasing::One,
        bla: BlaForm::Scalar,
        la: true,
        sa_seeds: true,
        interior_derivative: true,
        interior_probe: true,
        closed_form_components: true,
        de: true,
        seek: true,
        atom_domain: true,
        pi_meter: true,
        external_rays: true,
        orbit_overlay: true,
        content: true,
    };

    /// The generic polynomial path `z^p + c`.
    pub const MULTIBROT: Self = Self {
        la: false,
        closed_form_components: false,
        seek: false,
        pi_meter: false,
        external_rays: false,
        content: false,
        ..Self::MANDELBROT
    };

    pub fn of(formula: Formula) -> Self {
        match formula {
            Formula::Mandelbrot => Self::MANDELBROT,
            Formula::Multibrot(_) => Self::MULTIBROT,
        }
    }
}

/// The formulas a user can pick (F2: Mandelbrot only; F3 adds Multibrot powers).
pub fn selectable() -> Vec<Formula> {
    vec![Formula::Mandelbrot]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandelbrot_has_everything() {
        let c = FormulaCapabilities::of(Formula::Mandelbrot);
        assert!(c.la && c.seek && c.pi_meter && c.content && c.closed_form_components);
        let g = FormulaCapabilities::of(Formula::Multibrot(2));
        assert!(g.perturbation && g.sa_seeds && g.interior_probe && g.de && g.orbit_overlay);
        assert!(!g.la && !g.pi_meter && !g.content && !g.closed_form_components);
        assert_eq!(selectable(), vec![Formula::Mandelbrot]);
    }
}
