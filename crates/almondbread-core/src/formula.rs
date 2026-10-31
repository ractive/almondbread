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
//! `places::places_for`) and the Julia panel (`z^2 + k`) do not. `Mandelbar` (F5,
//! kb: plans/f5-mandelbar) is the first non-conformal formula: BLA with 2x2 coefficients.

pub use almondbread_math::{
    DoubleExp, DoubleExpComplex, FloatExpComplex, FloatExpSplit, Formula, JuliaK, MULTIBROT_MAX_POWER, TurnPlane,
};

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
    /// The Buddhabrot and Nebulabrot looks (BB, kb: plans/buddhabrot): orbit histograms of
    /// `z^2 + c` with the closed-form cardioid and bulb rejection.
    pub buddhabrot: bool,
    /// Pixels unresolved at the limit draw as the interior colour, not the fog (kb:
    /// techniques/unresolved-fog, formulas/burning-ship "Bounded orbits"): the formula has
    /// large regions of bounded orbits that no test proves (the Burning Ship's chaotic hull),
    /// so "not escaped" means "in the set" for almost all of them. Off where unresolved pixels
    /// are mostly slow escape or slow convergence that compute further resolves (Mandelbrot).
    pub bounded_interior: bool,
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
        buddhabrot: true,
        bounded_interior: false,
    };

    /// The generic polynomial path `z^p + c` (seek since F3).
    pub const MULTIBROT: Self = Self {
        la: false,
        closed_form_components: false,
        pi_meter: false,
        external_rays: false,
        content: false,
        julia_panel: false,
        buddhabrot: false,
        ..Self::MANDELBROT
    };

    /// A Julia set of `z^2 + k` (F4, kb: plans/f4-deep-julia): perturbation against the
    /// view-centre orbit, then the critical orbit (the Mandelbrot orbit of `k`), the scalar BLA
    /// with `B = 0`, DE by `dz/dz_0`, the derivative interior test (every interior pixel falls
    /// into `k`'s attracting cycle). No LA until measured, no SA seeds (their c-expansion), no
    /// attracting-cycle probe or interior cycle analysis (both follow the c-plane derivatives),
    /// no closed-form components, seek, π meter, rays, orbit overlay, content or Julia panel.
    pub const JULIA: Self = Self {
        perturbation: true,
        rebasing: Rebasing::CriticalOrbit,
        bla: BlaForm::Scalar,
        la: false,
        sa_seeds: false,
        interior_derivative: true,
        interior_probe: false,
        closed_form_components: false,
        de: true,
        seek: false,
        atom_domain: false,
        pi_meter: false,
        external_rays: false,
        orbit_overlay: false,
        content: false,
        julia_panel: false,
        buddhabrot: false,
        bounded_interior: false,
    };

    /// The Mandelbar set (Tricorn, F5, kb: plans/f5-mandelbar): the non-conformal path.
    /// Perturbation with one rebased reference, the BLA with real-linear (2x2) coefficients, DE
    /// and the derivative interior test with Jacobians, the attracting-cycle probe (even periods:
    /// `f^k` is holomorphic in z for even k), seek (Jacobian Newton) and the orbit overlay. Not
    /// built (until measured or derived for 2x2 maps): LA, local SA seeds, Component Newton, the
    /// interior cycle analysis (atom domains), closed-form components, the π meter, rays, content
    /// and the Julia panel.
    pub const MANDELBAR: Self = Self {
        perturbation: true,
        rebasing: Rebasing::One,
        bla: BlaForm::Matrix,
        la: false,
        sa_seeds: false,
        interior_derivative: true,
        interior_probe: true,
        closed_form_components: false,
        de: true,
        seek: true,
        atom_domain: false,
        pi_meter: false,
        external_rays: false,
        orbit_overlay: true,
        content: false,
        julia_panel: false,
        buddhabrot: false,
        bounded_interior: false,
    };

    /// The Burning Ship (F6, kb: plans/f6-burning-ship): Mandelbar's non-conformal path plus the
    /// fold (`diffabs` in every perturbation step, BLA blocks capped at the fold lines). Not
    /// built besides Mandelbar's gaps: the attracting-cycle probe (its step mixes holomorphic and
    /// antiholomorphic Jacobians, so neither Mandelbrot's nor Mandelbar's even-period rule holds;
    /// the derivative test proves interior pixels).
    pub const BURNING_SHIP: Self = Self {
        interior_probe: false,
        bounded_interior: true,
        ..Self::MANDELBAR
    };

    /// The Burning Ship's variants (F6b, kb: plans/f6b-burning-ship-variants): Celtic, Buffalo
    /// and the perpendicular ship carry the ship's whole path (their own fold cases, Jacobians
    /// and fold caps), so the same capabilities: no probe, no LA or SA seeds, no atom domains,
    /// no content beyond their places; unresolved pixels draw as the set (each has bounded
    /// orbits no test proves; measured per variant on the plan page).
    pub const SHIP_VARIANT: Self = Self::BURNING_SHIP;
    /// A turned plane or a start value (F9, kb: plans/f9-turn): Julia's two-orbit perturbation
    /// (the centre orbit from its own `z_0`, the critical orbit of the same `c` as the rebase
    /// target) with Mandelbrot's `+dc` and BLA (`B = 1`, applied to `dz` and `dc = cos t dp`),
    /// DE by `dz/dp` (starts at `sin t`, gains `cos t` per step), the derivative interior test.
    /// Everything that assumes `z_0 = 0` in the `c` plane (or `c` fixed) is off: LA and SA seeds
    /// (expansions from `z_0 = 0` in `c`), the attracting-cycle probe and the interior cycle
    /// analysis (they restart orbits at the critical point, which is not this view's start),
    /// closed-form components, seek, the π meter, rays, the orbit overlay, content, the Julia
    /// panel and the Buddhabrot.
    pub const TURN: Self = Self::JULIA;

    pub fn of(formula: Formula) -> Self {
        match formula {
            Formula::Mandelbrot => Self::MANDELBROT,
            Formula::Multibrot(_) => Self::MULTIBROT,
            Formula::Julia(_) => Self::JULIA,
            Formula::Turn(_) => Self::TURN,
            Formula::Mandelbar => Self::MANDELBAR,
            Formula::BurningShip => Self::BURNING_SHIP,
            Formula::Celtic | Formula::Buffalo | Formula::Perpendicular => Self::SHIP_VARIANT,
        }
    }
}

/// The formulas a user can pick, in menu order: Mandelbrot, then Multibrot `z^3 + c` ...
/// `z^8 + c` (F3; `Multibrot(2)` is the tests' generic Mandelbrot and not offered), then
/// Mandelbar (F5) and the Burning Ship family (F6: the ship; F6b: Celtic, Buffalo,
/// Perpendicular).
pub fn selectable() -> Vec<Formula> {
    let mut v = vec![Formula::Mandelbrot];
    v.extend((3..=MULTIBROT_MAX_POWER).map(Formula::Multibrot));
    v.push(Formula::Mandelbar);
    v.extend([
        Formula::BurningShip,
        Formula::Celtic,
        Formula::Buffalo,
        Formula::Perpendicular,
    ]);
    v
}

/// A formula's family, the formula picker's first choice (F3): `mandelbrot`, `multibrot`. A
/// family with several formulas (Multibrot's powers) gets a second choice in the picker. A Julia
/// set (F4, never in the picker) is in its parent's family, the Mandelbrot set's: the picker
/// shows that while a Julia view is open (disabled; kb: decisions/controls).
pub fn family(formula: Formula) -> &'static str {
    match formula {
        Formula::Mandelbrot | Formula::Julia(_) | Formula::Turn(_) => "mandelbrot",
        Formula::Multibrot(_) => "multibrot",
        Formula::Mandelbar => "mandelbar",
        Formula::BurningShip | Formula::Celtic | Formula::Buffalo | Formula::Perpendicular => "burningship",
    }
}

/// The family's name in menus.
pub fn family_title(formula: Formula) -> &'static str {
    match formula {
        Formula::Mandelbrot | Formula::Julia(_) | Formula::Turn(_) => "Mandelbrot",
        Formula::Multibrot(_) => "Multibrot",
        Formula::Mandelbar => "Mandelbar",
        Formula::BurningShip | Formula::Celtic | Formula::Buffalo | Formula::Perpendicular => "Burning Ship",
    }
}

/// The formula as maths ("z² + c", "z⁵ + c"; a Julia set's "z² + k"): the picker's label for one
/// formula of a family.
pub fn title(formula: Formula) -> String {
    if formula.is_julia() {
        return "z² + k".into();
    }
    if formula == Formula::Mandelbar {
        // (z with a combining macron: the conjugate.)
        return "z\u{304}² + c".into();
    }
    // F6b: the Burning Ship family's members by name in its second menu (as Multibrot's
    // powers); their formulas are on `formula_text`.
    match formula {
        Formula::BurningShip => return "Burning Ship".into(),
        Formula::Celtic => return "Celtic".into(),
        Formula::Buffalo => return "Buffalo".into(),
        Formula::Perpendicular => return "Perpendicular".into(),
        _ => {}
    }
    const SUPERSCRIPTS: [&str; 10] = ["⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹"];
    let p = formula.power();
    let exp: String = p
        .to_string()
        .bytes()
        .map(|b| SUPERSCRIPTS[usize::from(b - b'0')])
        .collect();
    format!("z{exp} + c")
}

/// The formula as maths in our coordinates (the Burning Ship family's iterations, shown with the
/// imaginary axis flipped against the literature so the ships stand upright; kb:
/// formulas/burning-ship "Variants"); [`title`] for the others.
pub fn formula_text(formula: Formula) -> String {
    match formula {
        // F6: the literature's (|x| + i|y|)² + c with the imaginary axis flipped.
        Formula::BurningShip => "(|x| − i|y|)² + c".into(),
        // F6b: KF's Celtic |Re z²| + i Im z² + c (unchanged by the flip).
        Formula::Celtic => "|x² − y²| + 2ixy + c".into(),
        // KF's Buffalo |Re z²| + i|Im z²| + c, flipped.
        Formula::Buffalo => "|x² − y²| − 2i|xy| + c".into(),
        // KF's perpendicular Burning Ship (x − i|y|)² + c, flipped.
        Formula::Perpendicular => "(x + i|y|)² + c".into(),
        f => title(f),
    }
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
        assert!(c.buddhabrot && !g.buddhabrot && !FormulaCapabilities::JULIA.buddhabrot);
        let all = selectable();
        assert_eq!(all[0], Formula::Mandelbrot);
        assert_eq!(&all[1..7], &(3..=8).map(Formula::Multibrot).collect::<Vec<_>>()[..]);
        assert_eq!(
            all[7..],
            [
                Formula::Mandelbar,
                Formula::BurningShip,
                Formula::Celtic,
                Formula::Buffalo,
                Formula::Perpendicular
            ]
        );
        // F6b: the variants carry the ship's capabilities.
        for f in [Formula::Celtic, Formula::Buffalo, Formula::Perpendicular] {
            assert_eq!(
                FormulaCapabilities::of(f),
                FormulaCapabilities::of(Formula::BurningShip)
            );
        }
        // F6: Mandelbar's path without the probe.
        let s = FormulaCapabilities::of(Formula::BurningShip);
        assert_eq!(s.bla, BlaForm::Matrix);
        assert!(s.perturbation && s.de && s.interior_derivative && s.seek && s.orbit_overlay);
        assert!(!s.interior_probe && !s.la && !s.sa_seeds && !s.atom_domain && !s.content && !s.buddhabrot);
        // F5: the non-conformal path.
        let b = FormulaCapabilities::of(Formula::Mandelbar);
        assert_eq!(b.bla, BlaForm::Matrix);
        assert!(b.perturbation && b.de && b.interior_derivative && b.seek && b.orbit_overlay);
        assert!(b.interior_probe);
        assert!(!b.la && !b.sa_seeds && !b.atom_domain && !b.content && !b.julia_panel);
        assert!(!all.contains(&Formula::Multibrot(2)));
    }

    #[test]
    fn picker_names() {
        assert_eq!(title(Formula::Mandelbrot), "z² + c");
        assert_eq!(title(Formula::Multibrot(5)), "z⁵ + c");
        assert_eq!(family(Formula::Multibrot(8)), "multibrot");
        assert_eq!(family_title(Formula::Mandelbrot), "Mandelbrot");
        assert_eq!(family(Formula::Mandelbar), "mandelbar");
        assert_eq!(title(Formula::Mandelbar), "z\u{304}² + c");
        assert_eq!(family(Formula::BurningShip), "burningship");
        assert_eq!(family_title(Formula::BurningShip), "Burning Ship");
        // F6b: one family, the members by name, their formulas in our coordinates.
        for (f, t) in [
            (Formula::BurningShip, "Burning Ship"),
            (Formula::Celtic, "Celtic"),
            (Formula::Buffalo, "Buffalo"),
            (Formula::Perpendicular, "Perpendicular"),
        ] {
            assert_eq!(
                (family(f), family_title(f), title(f).as_str()),
                ("burningship", "Burning Ship", t)
            );
        }
        assert_eq!(formula_text(Formula::BurningShip), "(|x| − i|y|)² + c");
        assert_eq!(formula_text(Formula::Perpendicular), "(x + i|y|)² + c");
        assert_eq!(formula_text(Formula::Multibrot(3)), "z³ + c");
        // F4: a Julia set is no picker entry; it belongs to the Mandelbrot family.
        let j = Formula::Julia(almondbread_math::JuliaK::new(
            &almondbread_math::BigComplex::parse("-0.12", "0.74", 128).unwrap(),
        ));
        assert_eq!(family(j), "mandelbrot");
        assert_eq!(title(j), "z² + k");
        assert!(!selectable().contains(&j));
    }
}
