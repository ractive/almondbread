//! What each fractal formula supports (F2, kb: plans/f2-pluggable-formulas,
//! synthesis/other-fractal-types §6 "Capabilities").
//!
//! The formula itself ([`Formula`]) lives in `almondbread-math`; this is the platform-free table
//! of capabilities the renderer gates on and the UIs (over the FFI, and the CLI) hide controls
//! by: what a formula lacks is hidden, never faked. Mandelbrot has everything. `Multibrot(p)` is
//! the generic polynomial path (F3, kb: plans/f3-multibrot, formulas/multibrot): perturbation
//! with one rebased reference, the scalar BLA, local SA seeds, the derivative interior test and
//! the attracting-cycle probe, DE, the orbit overlay and seek (Newton framing of minibrots, F3)
//! carry over; Imagina LA (power 2), the closed-form cardioid and bulb test, the π meter,
//! external rays, the Mandelbrot content (tours, journeys; places are per formula,
//! `places::places_for`) and the Julia panel (`z^2 + k`) do not.

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
    /// The Julia companion panel (F1, kb: plans/f1-julia-view): the Julia set of `z^2 + k` for
    /// the orbit's point k, a Mandelbrot parameter; other formulas need their own (`z^p + k`).
    pub julia_panel: bool,
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
        julia_panel: true,
    };

    /// The generic polynomial path `z^p + c` (seek since F3).
    pub const MULTIBROT: Self = Self {
        la: false,
        closed_form_components: false,
        pi_meter: false,
        external_rays: false,
        content: false,
        julia_panel: false,
        ..Self::MANDELBROT
    };

    pub fn of(formula: Formula) -> Self {
        match formula {
            Formula::Mandelbrot => Self::MANDELBROT,
            Formula::Multibrot(_) => Self::MULTIBROT,
        }
    }
}

/// The formulas a user can pick, in menu order: Mandelbrot, then Multibrot `z^3 + c` ...
/// `z^8 + c` (F3; `Multibrot(2)` is the tests' generic Mandelbrot and not offered).
pub fn selectable() -> Vec<Formula> {
    let mut v = vec![Formula::Mandelbrot];
    v.extend((3..=MULTIBROT_MAX_POWER).map(Formula::Multibrot));
    v
}

/// A formula's family, the formula picker's first choice (F3): `mandelbrot`, `multibrot`. A
/// family with several formulas (Multibrot's powers) gets a second choice in the picker.
pub fn family(formula: Formula) -> &'static str {
    match formula {
        Formula::Mandelbrot => "mandelbrot",
        Formula::Multibrot(_) => "multibrot",
    }
}

/// The family's name in menus.
pub fn family_title(formula: Formula) -> &'static str {
    match formula {
        Formula::Mandelbrot => "Mandelbrot",
        Formula::Multibrot(_) => "Multibrot",
    }
}

/// The formula as maths ("z² + c", "z⁵ + c"): the picker's label for one formula of a family.
pub fn title(formula: Formula) -> String {
    const SUPERSCRIPTS: [&str; 10] = ["⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹"];
    let p = formula.power();
    let exp: String = p
        .to_string()
        .bytes()
        .map(|b| SUPERSCRIPTS[usize::from(b - b'0')])
        .collect();
    format!("z{exp} + c")
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
        assert!(!g.la && !g.pi_meter && !g.content && !g.closed_form_components && !g.julia_panel && c.julia_panel);
        assert!(g.seek && c.seek);
        let all = selectable();
        assert_eq!(all[0], Formula::Mandelbrot);
        assert_eq!(&all[1..], &(3..=8).map(Formula::Multibrot).collect::<Vec<_>>()[..]);
        assert!(!all.contains(&Formula::Multibrot(2)));
    }

    #[test]
    fn picker_names() {
        assert_eq!(title(Formula::Mandelbrot), "z² + c");
        assert_eq!(title(Formula::Multibrot(5)), "z⁵ + c");
        assert_eq!(family(Formula::Multibrot(8)), "multibrot");
        assert_eq!(family_title(Formula::Mandelbrot), "Mandelbrot");
    }
}
