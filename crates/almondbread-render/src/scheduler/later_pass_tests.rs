//! Extension and compute-further passes take the first pass's rules (`probe_steps`) and its
//! reference (`JobReference`) as a whole. Each pass rebuilt its parameters from the level before
//! it, field by field: the 2026-11 review found the interior probe back on for formulas that
//! have none, and Celtic's fold values left on the old orbit's buffer. Neither shows in an
//! offscreen render (no such passes) or against an oracle at the session's own limit (the pixels
//! it gets wrong escape later), so these run the live scheduler against a higher one.

use almondbread_core::formula::{Formula, FormulaCapabilities, JuliaK};
use almondbread_core::oracle::{IterationField, VerifyReport, compare, cpu_render_for};
use almondbread_core::settings::{RenderSettings, Supersampling};
use almondbread_core::turn::plane_through;
use almondbread_core::viewport::{Center, Scale, Viewport};
use objc2::rc::Retained;

use super::{Renderer, SimulationResult, probe_steps};
use crate::metal::MetalBackend;

const W: usize = 160;
const H: usize = 100;

/// A jump to `view` through the real per-refresh scheduler, settled.
fn session(
    formula: Formula,
    view: &Viewport,
    limit: usize,
    cap: usize,
    further: bool,
) -> (Renderer<MetalBackend>, SimulationResult) {
    let mut r = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
    r.settings = RenderSettings::plain();
    r.settings.supersampling = Supersampling::Off;
    r.settings.formula = formula;
    r.settings.auto_iterations = false;
    r.settings.manual_iterations = limit;
    r.settings.max_iterations_cap = cap;
    // (As `verify --history`: a loaded machine must not cost the session its passes.)
    r.config.extension_budget = 120.0;
    r.config.further = further;
    r.viewport = view.clone();
    let sim = r
        .simulate(W, H, 1.0, 120.0, 0, 120.0, &mut |_, _, _| {})
        .expect("the session settles");
    (r, sim)
}

/// The shown level against the CPU at `limit`.
fn against_oracle(formula: Formula, view: &Viewport, sim: &SimulationResult, limit: usize) -> VerifyReport {
    let gpu = IterationField::from_texels(W, H, &sim.iterations);
    let cpu = cpu_render_for(
        formula,
        view.center.re(),
        view.center.im(),
        sim.texel_scale.to_f64(),
        W,
        H,
        limit,
        formula.bailout(),
        0.0,
    );
    let (report, _) = compare(&gpu, &cpu);
    println!("{formula:?} vs oracle at {limit}: {}", report.summary());
    report
}

/// Slow exterior pixels unresolved at the limit: the extension pass must leave them unresolved
/// or escaped. With the probe on, Component Newton proved the attracting cycle of the view's
/// parameter from z = 0, which says nothing of where this pixel's orbit goes, and painted them.
fn no_false_interior(formula: Formula, view: &Viewport, oracle_limit: usize) {
    assert!(!FormulaCapabilities::of(formula).interior_probe);
    let (r, sim) = session(formula, view, 200, 3200, false);
    let levels = &r.job.as_ref().expect("job").levels;
    assert!(levels.iter().any(|l| l.is_extension()), "no extension pass ran");
    let report = against_oracle(formula, view, &sim, oracle_limit);
    assert!(report.false_interior_pct() < 0.05, "{}", report.summary());
    assert!(levels.iter().all(|l| l.params.probe == 0), "a level kept the probe");
}

/// The review's view: `z^2 + 0.2499` at its repelling fixed point. Near-parabolic, so the exterior
/// wedge escapes slowly: past the limit of the extension pass that painted 1.8 % of it interior.
#[test]
fn julia_extension_passes_prove_nothing_by_the_probe() {
    let k = Center::parse("0.2499", "0").expect("k");
    let view = Viewport::new(0.51, 0.0, 2e-9);
    no_false_interior(Formula::Julia(JuliaK::new(k.big())), &view, 20_000);
}

/// The same fixed point in the plane turned 40 degrees, through `z_0 = -r` (one step from `r`):
/// the view coordinate is then in the main cardioid, where the cycle search finds a period, and
/// `c` closer to 1/4 keeps the exterior pixels past the pass to 800.
#[test]
fn turned_plane_extension_passes_prove_nothing_by_the_probe() {
    let c = Center::parse("0.24999", "0").expect("c");
    let z0 = Center::parse("-0.50316227766", "0").expect("z0");
    let (formula, p) = plane_through(4000, c.big(), z0.big());
    assert!(matches!(formula, Formula::Turn(_)));
    let view = Viewport::with_center(Center::from_big(p), Scale::parse("2e-9").expect("scale"));
    no_false_interior(formula, &view, 200_000);
}

/// Celtic where the centre's orbit stays bounded: compute further extends it twice. The kernel
/// reads a fold value at every plain step, so past the old orbit's end it read past its buffer:
/// 13 % of the pixels escaped that stay inside. The pixels are the CPU's at the session's limit.
#[test]
fn a_longer_orbit_brings_its_fold_values() {
    let view = Viewport::new(0.25, 0.0, 1e-6);
    let (r, sim) = session(Formula::Celtic, &view, 500, 128_000, true);
    let first = 16 * 500 + 1;
    let points = r.reference().expect("reference").points.len();
    assert!(points > first, "the orbit was not extended ({points} points)");
    let report = against_oracle(Formula::Celtic, &view, &sim, 128_000);
    assert!(
        report.false_exterior_pct() + report.false_interior_pct() < 0.05,
        "{}",
        report.summary()
    );
    let job = r.job.as_ref().expect("job");
    assert_eq!(job.reference.len as usize, points);
    assert_eq!(job.reference.serial, r.reference_serial);
    let (held, current) = (job.reference.refx.as_ref(), r.refx_buffer.as_ref());
    assert_eq!(
        held.map(Retained::as_ptr),
        current.map(Retained::as_ptr),
        "the job kept the old fold values"
    );
    assert!(job.levels.iter().all(|l| l.params.probe == 0), "a level kept the probe");
}

/// The rule itself: formulas without the probe never get it, at any limit; with it, the limit's
/// steps, up to the cap; a nucleus reference's period needs 4 periods of them. A later pass has
/// more steps than the first, so a period the first pass gave up on can come back.
#[test]
fn probe_steps_hold_the_rules() {
    let k = Center::parse("0", "1").expect("k");
    let c = Center::parse("0.1", "0.1").expect("c");
    let (turn, _) = plane_through(4000, c.big(), c.big());
    for f in [
        Formula::Julia(JuliaK::new(k.big())),
        turn,
        Formula::BurningShip,
        Formula::Celtic,
        Formula::Buffalo,
        Formula::Perpendicular,
    ] {
        for limit in [100, 1000, 1 << 24] {
            assert_eq!(probe_steps(f, None, limit), 0, "{f:?} at {limit}");
        }
    }
    for f in [Formula::Mandelbrot, Formula::Multibrot(3), Formula::Mandelbar] {
        assert_eq!(probe_steps(f, None, 1000), 381);
        assert_eq!(probe_steps(f, None, 1 << 24), 2048);
    }
    let m = Formula::Mandelbrot;
    assert_eq!(probe_steps(m, Some(95), 1000), 381);
    assert_eq!(probe_steps(m, Some(96), 1000), 0);
    assert_eq!(probe_steps(m, Some(96), 4000), 756);
    assert_eq!(probe_steps(m, Some(512), 1 << 24), 2048);
    assert_eq!(probe_steps(m, Some(513), 1 << 24), 0);
}
