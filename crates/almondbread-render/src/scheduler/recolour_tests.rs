//! A settings change that only recolours a settled view (an interior colouring switch: same
//! compute key, no new job) must reach the shown frame, also when the refresh that wanted to
//! recolour got no drawable (kb: decisions/controls "The colours tour").

use almondbread_core::settings::InteriorMode;
use almondbread_core::viewport::Viewport;

use super::Renderer;
use crate::backend::{GpuBackend, Storage, TextureFormat};
use crate::host::Host;
use crate::metal::MetalBackend;

const W: usize = 320;
const H: usize = 200;

type Texture = <MetalBackend as GpuBackend>::Texture;

fn cardioid() -> Viewport {
    Viewport::new(-0.45, 0.25, 1.3 / W as f64)
}

/// Refreshes at 60 Hz until the renderer reports idle (the display link would pause), at most
/// `timeout` seconds; returns the shown pixels.
fn settle(r: &mut Renderer<MetalBackend>, target: &Texture, timeout: f64) -> Vec<u8> {
    let end = r.clock() + timeout;
    loop {
        let now = r.clock();
        let o = r.render_frame(now, W, H, 1.0, &mut |_| Some((target.clone(), None)));
        if !o.animating {
            break;
        }
        assert!(r.clock() < end, "did not settle within {timeout} s");
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    read(r, target)
}

fn read(r: &mut Renderer<MetalBackend>, target: &Texture) -> Vec<u8> {
    r.wait_frames();
    r.drain_in_flight();
    let mut px = vec![0u8; W * H * 4];
    r.backend
        .read_texture(target, W, H, TextureFormat::Bgra8Unorm, &mut px)
        .unwrap();
    px
}

/// Share of pixels whose colour differs visibly.
fn diff(a: &[u8], b: &[u8]) -> f64 {
    let n = a
        .chunks(4)
        .zip(b.chunks(4))
        .filter(|(x, y)| x.iter().zip(y.iter()).take(3).any(|(p, q)| p.abs_diff(*q) > 8))
        .count();
    n as f64 / (a.len() / 4) as f64
}

fn renderer(mode: InteriorMode) -> (Renderer<MetalBackend>, Texture) {
    let mut r = Renderer::new(MetalBackend::new().unwrap()).unwrap();
    r.viewport = cardioid();
    r.settings.interior_mode = mode;
    let t = r
        .backend
        .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
        .unwrap();
    (r, t)
}

/// The view rendered from scratch with `mode`.
fn fresh(mode: InteriorMode) -> Vec<u8> {
    let (mut r, t) = renderer(mode);
    settle(&mut r, &t, 20.0)
}

#[test]
fn interior_switch_recolours_the_shown_frame() {
    let (mut r, t) = renderer(InteriorMode::Flat);
    let mut prev = settle(&mut r, &t, 20.0);
    // Flat -> bubbles needs the cycle analysis (a new job); the rest only recolour.
    for mode in [
        InteriorMode::Bubbles,
        InteriorMode::Period,
        InteriorMode::Angle,
        InteriorMode::Distance,
        InteriorMode::Flat,
    ] {
        r.settings.interior_mode = mode;
        let shown = settle(&mut r, &t, 20.0);
        let d_fresh = diff(&shown, &fresh(mode));
        let d_prev = diff(&shown, &prev);
        // Also the anti-aliased edges: a recolour restarts AA, which must run before idle.
        assert!(
            d_fresh < 0.005,
            "{mode:?}: {d_fresh} of the pixels differ from a fresh render"
        );
        assert!(d_prev > 0.2, "{mode:?}: only {d_prev} of the pixels changed");
        prev = shown;
    }
}

/// The bug: the one refresh that wanted to recolour got no drawable (none parked yet, e.g. a
/// background window) and reported idle, so the display link paused on the old colours.
#[test]
fn recolour_without_a_drawable_keeps_animating() {
    let (mut r, t) = renderer(InteriorMode::Period);
    let before = settle(&mut r, &t, 20.0);
    r.settings.interior_mode = InteriorMode::Angle;
    let now = r.clock();
    let o = r.render_frame(now, W, H, 1.0, &mut |_| None);
    assert!(!o.presented);
    assert!(o.animating, "a recolour without a target must ask for the next refresh");
    let shown = settle(&mut r, &t, 20.0);
    assert!(diff(&shown, &before) > 0.2, "the angle colouring did not show");
    assert!(diff(&shown, &fresh(InteriorMode::Angle)) < 0.005);
}

/// The other side of the fix: a settled view with nothing to recompose must stay idle on
/// refreshes without a drawable (a background window, the windowless layer of `make
/// core-check`), else the display link never pauses (battery). `missed_frame` counts only a
/// frame the plan wanted.
#[test]
fn settled_view_without_a_drawable_stays_idle() {
    let (mut r, t) = renderer(InteriorMode::Period);
    settle(&mut r, &t, 20.0);
    for _ in 0..30 {
        let now = r.clock();
        let o = r.render_frame(now, W, H, 1.0, &mut |_| None);
        assert!(!o.animating, "a settled view asked for frames without a drawable");
        assert!(r.is_settled());
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

/// The colours tour's interior steps as the app plays them (through the Host), without
/// repeating the view with `move: jump`.
#[test]
fn colours_tour_interior_steps_recolour() {
    let tour = almondbread_core::tour::named("colours").unwrap();
    let index = |title: &str| tour.steps.iter().position(|s| s.title == title).unwrap();
    let steps = [
        (index("Bubbles"), InteriorMode::Bubbles),
        (index("Period"), InteriorMode::Period),
        (index("Internal angle"), InteriorMode::Angle),
        (index("Interior distance"), InteriorMode::Distance),
    ];
    let mut h = Host::new(Renderer::new(MetalBackend::new().unwrap()).unwrap());
    let now = h.renderer.clock();
    h.resize(W as f64, H as f64, 1.0, now);
    let t = h
        .renderer
        .backend
        .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
        .unwrap();
    let settle_host = |h: &mut Host<MetalBackend>| -> Vec<u8> {
        let end = h.renderer.clock() + if cfg!(debug_assertions) { 120.0 } else { 30.0 };
        // The Bubbles and Period steps animate their probe orbit for as long as they are shown
        // (`probe-motion: bounce`), so the host never idles there: count the step settled once
        // the fractal is (settled job, no motion, no overlay work) for 30 refreshes in a row,
        // time enough for a pending recolour to reach the target.
        let mut quiet = 0;
        loop {
            let now = h.renderer.clock();
            let o = h.frame(now, &|| now, true, &mut |_| Some(((t.clone(), None), 0)));
            if !o.animating {
                break;
            }
            let fractal_settled = h.renderer.is_settled() && !h.renderer.navigator.is_animating() && !h.overlay.busy();
            quiet = if fractal_settled { quiet + 1 } else { 0 };
            if quiet >= 30 {
                break;
            }
            assert!(h.renderer.clock() < end, "the step did not settle");
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        read(&mut h.renderer, &t)
    };
    h.start_tour(tour, now);
    let mut prev: Option<Vec<u8>> = None;
    for (i, mode) in steps {
        let now = h.renderer.clock();
        h.tour_go(i, now);
        assert_eq!(h.renderer.settings.interior_mode, mode);
        let shown = settle_host(&mut h);
        if let Some(p) = &prev {
            let d = diff(&shown, p);
            // (The interior is ~17-20 % of these views since the tour's 2026-10-20 rework.)
            assert!(d > 0.1, "{mode:?}: only {d} of the pixels changed");
        }
        prev = Some(shown);
    }
}

/// Density pivot (kb: techniques/coloring-methods, "Density pivot"): a settled deep view has
/// a pivot from the live frame path (no blocking readback), and a +3 % density edit with the
/// offset pivoted on it changes far fewer pixels than the same edit without (the place
/// spiral-galaxy at 1e22.6, its own look).
#[test]
fn density_edit_pivots_on_the_median_escape_count() {
    let place = almondbread_core::places::place_named("spiral-galaxy").unwrap();
    let mut r = Renderer::new(MetalBackend::new().unwrap()).unwrap();
    r.viewport = place.viewport(W, H);
    almondbread_core::palette::Look::named(place.look)
        .unwrap()
        .apply(&mut r.settings);
    let t = r
        .backend
        .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
        .unwrap();
    let before = settle(&mut r, &t, 30.0);
    let pivot = r.density_pivot().expect("a pivot once the view settled");
    assert!(pivot > 100.0, "{pivot}");
    let (d1, o1) = (r.settings.color_density, r.settings.color_offset);
    let d2 = d1 * 1.03;
    r.settings.color_density = d2;
    let plain = diff(&settle(&mut r, &t, 30.0), &before);
    r.settings.color_offset =
        almondbread_core::density_pivot::pivoted_offset(o1, d1, d2, Some(pivot), r.settings.log_coloring);
    let pivoted = diff(&settle(&mut r, &t, 30.0), &before);
    // A recolour starts no job: the pivot is still the view's.
    assert_eq!(r.density_pivot(), Some(pivot));
    println!("density edit +3 %: pivot {pivot:.1}, pixels changed pivoted {pivoted:.3}, plain {plain:.3}");
    assert!(pivoted < 0.5 * plain, "pivoted {pivoted}, plain {plain}");
}
