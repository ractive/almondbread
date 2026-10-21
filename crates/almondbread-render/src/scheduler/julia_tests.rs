//! F4 (kb: plans/f4-deep-julia): deep Julia sets on the GPU. The kernel's Julia path (pixel =
//! `z_0`, no `+c`, the centre orbit then the critical orbit) against the CPU oracles, shallow
//! (f64 reference, plain kernel) and deep (big reference, deep kernel, BLA with `B = 0`) at two
//! of the step-1 gate's views; a new `k` never reuses the old reference, and Mandelbrot after
//! a Julia set is the same field bit for bit.

use almondbread_core::formula::{Formula, JuliaK};
use almondbread_core::oracle::{IterationField, compare, cpu_render_deep_for, cpu_render_for};
use almondbread_core::viewport::{Center, Scale, Viewport};

use super::{BAILOUT, Renderer};
use crate::metal::MetalBackend;

const W: usize = 200;
const H: usize = 125;

fn julia(re: &str, im: &str) -> Formula {
    Formula::Julia(JuliaK::new(Center::parse(re, im).unwrap().big()))
}

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

fn renderer() -> Renderer<MetalBackend> {
    let mut r = Renderer::new(MetalBackend::new().expect("Metal")).expect("renderer");
    r.settings = almondbread_core::settings::RenderSettings::plain();
    r.settings.supersampling = almondbread_core::settings::Supersampling::Off;
    r
}

#[test]
fn julia_sets_match_their_oracles() {
    let mut r = renderer();
    let rabbit = julia("-0.1225611668766536", "0.7448617666197442");
    // The whole set: f64 references, plain kernel.
    let whole = Viewport::new(0.0, 0.0, 4.2 / W as f64);
    let g = field(&mut r, rabbit, &whole, 1000);
    let cpu = cpu_render_for(rabbit, 0.0, 0.0, 4.2 / W as f64, W, H, 1000, BAILOUT, 0.0);
    let (rep, _) = compare(&g, &cpu);
    println!("rabbit whole vs oracle: {}", rep.summary());
    assert!(
        rep.false_interior_pct() + rep.false_exterior_pct() < 0.05,
        "{}",
        rep.summary()
    );
    assert!(rep.escape_differs_pct() < 0.5, "{}", rep.summary());
    assert!(rep.cpu_interior > W * H / 10, "the rabbit inside");
    // Gate views (kb: plans/f4-deep-julia "Step 1"): a preimage of 0 at 1e-100 (period-3 bulb)
    // and at 1e-300 (Misiurewicz i), deep kernel and BLA.
    let deep = [
        (
            rabbit,
            "0.13582194234832306384726920844974750963803955544158433695104985552061703718356220531423914345539064577159014155396788582297739",
            "-0.98408937840354738561303053036751272330190031920474351403645897719626417827245062205230473648568576800988084871583518785926966",
            "1.0000000000000226e-100",
            3670,
        ),
        (
            julia("0", "1"),
            "-1.316217188613384182892301938270293124103893412616323208720307172274596124394581156543859132946211679304026231908661961024336819202086203360168036350362781617862595986112142227701026008880468481898614759175229768762325953986190536194031254729435187793227165530004224246648352068591889334276616386741309913104214101345554899367",
            "0.6785478674150520237952461553430622991323380058277815194421221054868252650933002485440546541350165849932149350420951796822304685241394963385899335024961001636274825482729138595485370773903166706946136063947425419828687390342905631095799823421179334798563580045190217945069289191899700554854070505266217696214286174348262202054",
            "1.0000000000000678e-300",
            6850,
        ),
    ];
    for (f, re, im, scale, it) in deep {
        let view = Viewport::with_center(Center::parse(re, im).unwrap(), Scale::parse(scale).unwrap());
        let g = field(&mut r, f, &view, it);
        let (cpu, _) = cpu_render_deep_for(f, &view.center, view.scale, W, H, it, BAILOUT, 0.0);
        let (rep, _) = compare(&g, &cpu);
        println!("{f:?} at {scale} vs oracle: {}", rep.summary());
        assert!(
            r.reference()
                .is_some_and(|x| x.big && x.deep_kernel && x.julia.is_some())
        );
        assert!(
            rep.false_interior_pct() + rep.false_exterior_pct() < 0.05,
            "{}",
            rep.summary()
        );
        assert!(rep.escape_differs_pct() < 0.5, "{}", rep.summary());
        // Not a flat picture: escape counts vary across the view.
        let n: Vec<f32> = g.n.iter().copied().filter(|&n| n >= 0.0).collect();
        let (lo, hi) = n.iter().fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(hi - lo > 20.0, "{lo}..{hi}");
    }
}

#[test]
fn a_new_k_never_reuses_the_old_reference() {
    let mut r = renderer();
    let view = Viewport::new(0.1, 0.2, 3.0 / W as f64);
    let m = field(&mut r, Formula::Mandelbrot, &view, 500);
    let a = julia("-0.12", "0.75");
    let fa = field(&mut r, a, &view, 500);
    let b = julia("-0.75", "0.1");
    let fb = field(&mut r, b, &view, 500);
    assert!(fa.bitwise_differences(&fb) > W * H / 4);
    // The same k again (another precision, one value): the same set.
    let a2 = Formula::Julia(JuliaK::new(
        &Center::parse("-0.12", "0.75").unwrap().big().with_precision(512),
    ));
    assert_eq!(a2, a);
    // Mandelbrot after Julia sets: bit for bit what it was.
    let again = field(&mut r, Formula::Mandelbrot, &view, 500);
    assert_eq!(again.bitwise_differences(&m), 0);
}
