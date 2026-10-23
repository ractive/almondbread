//! F2 (kb: plans/f2-pluggable-formulas): the generic formula path on the GPU. Multibrot(2)
//! through the generic kernel hooks renders what the Mandelbrot kernels render (shallow, plain
//! and deep), a formula switch never reuses the other formula's reference, and z^3 + c agrees
//! with its CPU oracle.

use almondbread_core::formula::Formula;
use almondbread_core::oracle::{IterationField, compare, cpu_render_deep_for, cpu_render_for};
use almondbread_core::viewport::{Center, Scale, Viewport};

use super::{BAILOUT, Renderer};
use crate::metal::MetalBackend;

const W: usize = 240;
const H: usize = 160;

fn field(r: &mut Renderer<MetalBackend>, formula: Formula, view: &Viewport, iterations: usize) -> IterationField {
    r.settings.formula = formula;
    r.settings.auto_iterations = false;
    r.settings.manual_iterations = iterations;
    r.settings.max_iterations_cap = iterations;
    r.viewport = view.clone();
    let res = r.render_offscreen(W, H, true).expect("render");
    assert_eq!(r.reference().map(|x| x.formula), Some(formula));
    IterationField::from_texels(W, H, res.iterations.as_deref().expect("readback"))
}

#[test]
fn generic_power_two_renders_mandelbrot() {
    let mut r = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
    r.settings = almondbread_core::settings::RenderSettings::plain();
    r.settings.supersampling = almondbread_core::settings::Supersampling::Off;
    let views = [
        (Viewport::new(-0.75, 0.1, 2.6 / W as f64), 2000),
        // Deep: big reference, BLA / deep kernel.
        (
            Viewport::with_center(
                Center::parse("-1.76858859491416404040862718", "0.00176401338497818838612").unwrap(),
                Scale::parse("4e-20").unwrap(),
            ),
            8000,
        ),
    ];
    for (view, it) in &views {
        let m = field(&mut r, Formula::Mandelbrot, view, *it);
        let g = field(&mut r, Formula::Multibrot(2), view, *it);
        let (rep, _) = compare(&g, &m);
        println!("generic p = 2 vs Mandelbrot at {}: {}", view.scale, rep.summary());
        assert!(
            rep.false_interior_pct() + rep.false_exterior_pct() < 0.05,
            "{}",
            rep.summary()
        );
        assert!(rep.escape_differs_pct() < 2.0, "{}", rep.summary());
        // And back: the Mandelbrot field again, bit for bit (no state of the other formula).
        let again = field(&mut r, Formula::Mandelbrot, view, *it);
        assert_eq!(again.bitwise_differences(&m), 0);
    }
    // z^3 + c, shallow and at 1e-20, against its oracle.
    let cubic = Formula::Multibrot(3);
    let shallow = Viewport::new(0.0, 0.0, 3.0 / W as f64);
    let g = field(&mut r, cubic, &shallow, 1000);
    let cpu = cpu_render_for(cubic, 0.0, 0.0, 3.0 / W as f64, W, H, 1000, BAILOUT, 0.0);
    let (rep, _) = compare(&g, &cpu);
    println!("z^3 + c shallow vs oracle: {}", rep.summary());
    assert!(
        rep.false_interior_pct() < 0.05 && rep.mismatch_pct() < 0.5,
        "{}",
        rep.summary()
    );
    let deep = Viewport::with_center(
        Center::parse("-0.41520234186", "0.60398787545").unwrap(),
        Scale::parse("2e-20").unwrap(),
    );
    let g = field(&mut r, cubic, &deep, 4000);
    let (cpu, _) = cpu_render_deep_for(cubic, &deep.center, deep.scale, W, H, 4000, BAILOUT, 0.0);
    let (rep, _) = compare(&g, &cpu);
    println!("z^3 + c at 2e-20 vs oracle: {}", rep.summary());
    assert!(
        rep.false_interior_pct() < 0.05 && rep.mismatch_pct() < 0.5,
        "{}",
        rep.summary()
    );
}

/// F3 (kb: plans/f3-multibrot): every user power at its whole set and at its seek-found minibrot
/// near 1e-20 agrees with its oracle (same bailout, `Formula::bailout`), with finite smooth
/// counts everywhere (before F3 the escaped |z|^2 overflowed float32 from p = 4 on: inf bands).
#[test]
fn multibrot_powers_match_their_oracle() {
    let mut r = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
    r.settings = almondbread_core::settings::RenderSettings::plain();
    r.settings.supersampling = almondbread_core::settings::Supersampling::Off;
    for p in 3..=8 {
        let f = Formula::Multibrot(p);
        let whole = Viewport::new(0.0, 0.0, 2.9 / H as f64);
        let g = field(&mut r, f, &whole, 1000);
        let cpu = cpu_render_for(f, 0.0, 0.0, 2.9 / H as f64, W, H, 1000, f.bailout(), 0.0);
        let (rep, _) = compare(&g, &cpu);
        println!("z^{p} + c whole set vs oracle: {}", rep.summary());
        assert!(
            rep.false_interior_pct() < 0.05 && rep.mismatch_pct() < 0.5,
            "{}",
            rep.summary()
        );
        assert!(g.s.iter().all(|s| s.is_finite()), "p = {p}: non-finite smooth offset");
        let place = almondbread_core::places::places_for(f)
            .into_iter()
            .find(|q| q.category == almondbread_core::places::PlaceCategory::Deep)
            .expect("deep place");
        let deep = place.viewport(W, H);
        let g = field(&mut r, f, &deep, 20_000);
        let (cpu, _) = cpu_render_deep_for(f, &deep.center, deep.scale, W, H, 20_000, f.bailout(), 0.0);
        let (rep, _) = compare(&g, &cpu);
        println!("z^{p} + c at {} vs oracle: {}", place.id, rep.summary());
        assert!(
            rep.false_interior_pct() < 0.1 && rep.mismatch_pct() < 0.5,
            "{}",
            rep.summary()
        );
        assert!(g.s.iter().all(|s| s.is_finite()), "p = {p}: non-finite smooth offset");
    }
}

/// F5 (kb: plans/f5-mandelbar): the Mandelbar set through the non-conformal path agrees with its
/// oracle at the whole set and at its places (plain kernel, deep kernel with the matrix BLA), and
/// the BLA changes the field no more than float32 noise: the same view with BLA off (no table)
/// against with BLA on (the skip-off diff).
#[test]
fn mandelbar_matches_its_oracle_with_and_without_bla() {
    let f = Formula::Mandelbar;
    let mut r = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
    r.settings = almondbread_core::settings::RenderSettings::plain();
    r.settings.supersampling = almondbread_core::settings::Supersampling::Off;
    let whole = Viewport::fitting_for(f, W as f64, H as f64);
    let g = field(&mut r, f, &whole, 1000);
    let cpu = cpu_render_for(
        f,
        whole.center.re(),
        0.0,
        whole.scale.to_f64(),
        W,
        H,
        1000,
        f.bailout(),
        0.0,
    );
    let (rep, _) = compare(&g, &cpu);
    println!("mandelbar whole set vs oracle: {}", rep.summary());
    assert!(
        rep.false_interior_pct() < 0.05 && rep.mismatch_pct() < 0.5,
        "{}",
        rep.summary()
    );
    for place in almondbread_core::places::places_for(f).into_iter().skip(1) {
        let view = place.viewport(W, H);
        let it = 20_000;
        let g = field(&mut r, f, &view, it);
        let bla_used = r.bla_info().1 > 0;
        let cpu = if view.scale.to_f64() >= 1e-13 {
            cpu_render_for(
                f,
                view.center.re(),
                view.center.im(),
                view.scale.to_f64(),
                W,
                H,
                it,
                f.bailout(),
                0.0,
            )
        } else {
            cpu_render_deep_for(f, &view.center, view.scale, W, H, it, f.bailout(), 0.0).0
        };
        let (rep, _) = compare(&g, &cpu);
        println!(
            "mandelbar {} vs oracle (BLA table {bla_used}): {}",
            place.id,
            rep.summary()
        );
        assert!(
            rep.false_interior_pct() < 0.1 && rep.mismatch_pct() < 0.5,
            "{}",
            rep.summary()
        );
        // Skip-off: no BLA table at all.
        let mut off = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
        off.settings = r.settings.clone();
        off.config.bla = false;
        let h = field(&mut off, f, &view, it);
        assert_eq!(off.bla_info().1, 0);
        let (d, _) = compare(&g, &h);
        println!("  BLA on vs off: {}", d.summary());
        // (Where the view is chaotic at this sampling, as at the period-620 minibrot, BLA
        // changes escape counts as float32 rounding does: no more than the GPU differs from
        // the oracle.)
        assert!(
            d.mismatch_pct() < 0.1 && d.median_error <= 2.0 * rep.median_error.max(1e-6),
            "{}",
            d.summary()
        );
    }
}

/// F6 (kb: plans/f6-burning-ship): the Burning Ship through the fold path agrees with its oracle
/// at the whole set and at its places (plain kernel, deep kernel with the fold-capped matrix BLA,
/// the needle at 1e-100), and the BLA changes the field no more than float32 does: BLA off (no
/// table) against on. Its views are chaotic where orbits stay bounded without a cycle, so escape
/// counts there differ between any two float paths (as between the oracle and its grid shifted
/// 0.001 px, `almondbread precision`): the bounds are on class errors, and on the BLA diff
/// relative to the GPU's own difference from the oracle.
#[test]
fn burning_ship_matches_its_oracle_with_and_without_bla() {
    let f = Formula::BurningShip;
    let mut r = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
    r.settings = almondbread_core::settings::RenderSettings::plain();
    r.settings.supersampling = almondbread_core::settings::Supersampling::Off;
    let mut views: Vec<(String, Viewport)> = almondbread_core::places::places_for(f)
        .into_iter()
        .map(|p| (p.id.to_string(), p.viewport(W, H)))
        .collect();
    // (Seek's period-3224 minibrot at 6e-27 per pixel, where the BLA skips ~9x of the steps.)
    views.push((
        "period-3224".into(),
        Viewport::with_center(
            Center::parse(
                "-1.7621497225498208521697460048491",
                "0.028770289802978131787848363068104",
            )
            .unwrap(),
            Scale::parse("1.6e-26").unwrap(),
        ),
    ));
    for (id, view) in views {
        let it = if view.scale.to_f64() > 1e-6 { 2000 } else { 20_000 };
        let g = field(&mut r, f, &view, it);
        let bla_used = r.bla_info().1 > 0;
        let cpu = if view.scale.to_f64() >= 1e-13 {
            cpu_render_for(
                f,
                view.center.re(),
                view.center.im(),
                view.scale.to_f64(),
                W,
                H,
                it,
                f.bailout(),
                0.0,
            )
        } else {
            cpu_render_deep_for(f, &view.center, view.scale, W, H, it, f.bailout(), 0.0).0
        };
        let (rep, _) = compare(&g, &cpu);
        println!("burning ship {id} vs oracle (BLA table {bla_used}): {}", rep.summary());
        assert!(rep.false_interior_pct() < 0.1, "{}", rep.summary());
        if id == "burningship-needle-1e100" {
            assert!(
                rep.escape_differs_pct() < 0.1 && rep.mismatch_pct() < 0.05,
                "{}",
                rep.summary()
            );
        }
        let mut off = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
        off.settings = r.settings.clone();
        off.config.bla = false;
        let h = field(&mut off, f, &view, it);
        assert_eq!(off.bla_info().1, 0);
        let (d, _) = compare(&g, &h);
        println!("  BLA on vs off: {}", d.summary());
        assert!(
            d.mismatch_pct() <= rep.mismatch_pct() + 0.1 && d.escape_differs_pct() <= rep.escape_differs_pct() + 0.5,
            "{}",
            d.summary()
        );
    }
}

/// IEEE half float bits to f32 (DE texels are Rg16Float).
fn half(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1f);
    let m = f32::from(h & 0x3ff);
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// The Burning Ship's lighting normal follows its smooth count (user report 2026-10-23: nested
/// axis-aligned rectangles with sharp steps at 8e94x on the needle). The normal's direction
/// (the DE texel's angle, z / dz/dc) must point along -grad mu, measured by finite differences
/// of the iteration field. The Jacobian's fold sign was taken from the float32 sum Z + s w,
/// which flushes Y + y (~1e-97) to 0 on the needle: the reflection at every step, so the
/// derivative was wrong wherever the orbit's sign pattern differed from that (rectangles
/// bounded by fold lines); 9382 of 33238 smooth pixels (28 %) pointed the wrong way.
#[test]
fn burning_ship_needle_normal_follows_the_smooth_count() {
    let f = Formula::BurningShip;
    let mut r = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
    r.settings = almondbread_core::settings::RenderSettings::plain();
    r.settings.supersampling = almondbread_core::settings::Supersampling::Off;
    r.settings.shading = 0.5; // distance estimate (and its normal) on
    let view = Viewport::with_center(
        Center::parse(
            "-1.900000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "-5.7536583899403481757905856401e-97",
        )
        .unwrap(),
        Scale::parse("1.5e-97").unwrap(),
    );
    let g = field(&mut r, f, &view, 180_000);
    let de = r.read_distance_estimates().expect("de");
    let mu = |x: usize, y: usize| {
        let k = y * W + x;
        (g.n[k] >= 0.0).then(|| f64::from(g.n[k]) + f64::from(g.s[k]))
    };
    let (mut smooth, mut wrong) = (0, 0);
    for y in 1..H - 1 {
        for x in 1..W - 1 {
            let k = y * W + x;
            let [Some(l), Some(rt), Some(up), Some(dn)] = [mu(x - 1, y), mu(x + 1, y), mu(x, y - 1), mu(x, y + 1)]
            else {
                continue;
            };
            // (Rows run down, the imaginary axis up.)
            let (gx, gy) = (0.5 * (rt - l), -0.5 * (dn - up));
            let d = half(de[k][0]);
            // Away from the boundary (DE > 8 texels) and where mu has a measurable slope.
            if !(d > 8.0 && gx.hypot(gy) > 1e-5) {
                continue;
            }
            let a = f64::from(half(de[k][1]));
            let c = -(gx * a.cos() + gy * a.sin()) / gx.hypot(gy);
            smooth += 1;
            if c < 0.7 {
                wrong += 1;
            }
        }
    }
    println!("burning ship needle normal: {wrong} of {smooth} smooth pixels point away from -grad mu");
    assert!(smooth > 10_000 && wrong * 200 < smooth, "{wrong} of {smooth}");
}
