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
