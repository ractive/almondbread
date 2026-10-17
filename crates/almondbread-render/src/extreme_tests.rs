//! Extreme-value smoke tests (kb: techniques/hang-prevention): the overlay maths and layout
//! with absurd inputs (depth past 1e308 or inf, NaN, size 0) return quickly with something
//! sane. CPU only (scenes, no GPU); milliseconds in all.

use std::time::Instant;

use almondbread_core::pi_moment::PiPoint;
use almondbread_core::viewport::{Center, Scale, Viewport};

use crate::compass::CompassOverlay;
use crate::cosmic::CosmicOverlay;
use crate::overlay::{Scene, map_decades, rgba};
use crate::pi_meter::{PiMeter, Reserve, flight_still_view, flight_zoom};
use crate::scheduler::halton;

const BAD: [f64; 7] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0, 1e300, -1e300];

#[test]
fn extreme_map_levels() {
    // The mini-map hang: depth past 1e308 became +inf and the level loop never ended.
    for d in BAD.into_iter().chain([1e308 * 10.0, 3010.0, 1e18]) {
        for want in [0, 1, 3, usize::MAX] {
            let levels = map_decades(d, want, &[]);
            assert!(!levels.is_empty() && levels.len() <= 16, "{d} {want}: {levels:?}");
            assert_eq!(levels[0], 0);
            assert!(levels.windows(2).all(|w| w[0] < w[1]), "{levels:?}");
        }
    }
    assert_eq!(map_decades(f64::INFINITY, 3, &[6, 12]), vec![0]);
    assert_eq!(map_decades(f64::NAN, 3, &[6, 12]), vec![0]);
}

#[test]
fn extreme_cosmic_panel_past_f64_depths() {
    // Depth ~1e18 decades (far past 2^53): the decade ticks counted up with `k += 1.0`, which
    // stops changing k there and never reached the end.
    let t0 = Instant::now();
    let mut o = CosmicOverlay::default();
    o.settings.enabled = true;
    for (k, scale) in [
        Scale::pow2(-10_000),
        Scale::pow2(i64::MIN / 2),
        Scale::pow2(i64::MAX / 2),
    ]
    .into_iter()
    .enumerate()
    {
        let view = Viewport::with_center(Center::new(-0.75, 0.1), scale);
        for (size, ppp) in [
            ((2000, 1200), 0.5),
            ((0, 0), 1.0),
            ((10, 10), f64::NAN),
            ((10, 10), f64::INFINITY),
        ] {
            let now = k as f64;
            o.update(&view, size, ppp, now);
            let mut scene = Scene::default();
            let mut key = String::new();
            o.draw(&mut scene, &mut Vec::new(), size, ppp, [24.0, 1176.0], now, &mut key);
            assert!(scene.prims.len() < 10_000);
        }
    }
    assert!(t0.elapsed().as_secs_f64() < 0.5, "{:?}", t0.elapsed());
}

#[test]
fn extreme_compass_layout_and_draw() {
    let mut c = CompassOverlay::default();
    c.visibility.always = true;
    for a in BAD {
        c.update(a, false, 0.0);
        let _ = c.layout((a, a), (a, 0.0, a, 0.0), Some(a));
        let _ = c.press(a, a, 0.0);
        let _ = c.drag_to(a, 1.0);
        let _ = c.release();
        for ppp in [a, 1.0] {
            let mut scene = Scene::default();
            let mut key = String::new();
            c.draw(&mut scene, (0, 0), ppp, a, 0.0, &mut key);
            c.draw(&mut scene, (800, 600), ppp, a, 0.0, &mut key);
            assert!(scene.prims.len() < 1000);
        }
    }
}

#[test]
fn extreme_pi_meter_layout_and_flight() {
    for eps in BAD {
        let _ = flight_zoom(PiPoint::Neck, 0.1, eps);
        let v = flight_still_view(PiPoint::Cusp, eps, 1000.0);
        assert!(v.scale.log2().is_finite());
    }
    let mut m = PiMeter::default();
    m.enabled = true;
    for px in [f64::NAN, f64::INFINITY, 0.0, -1.0, 1e-300] {
        m.set_layout(1600, 1000, px); // ignored when not a positive finite scale
    }
    m.set_layout(1600, 1000, 1.0);
    let t0 = Instant::now();
    for (w, h, px) in [
        (0.0, 0.0, 1.0),
        (f64::NAN, 1000.0, 1.0),
        (1600.0, 1000.0, f64::NAN),
        (1e300, 1e300, 1e-300),
    ] {
        let mut scene = Scene::default();
        let _ = m.draw(&mut scene, &mut Vec::new(), w, h, px, Reserve::default());
        assert!(scene.prims.len() < 100_000);
    }
    assert!(t0.elapsed().as_secs_f64() < 0.5, "{:?}", t0.elapsed());
}

#[test]
fn extreme_halton_and_tile_binning() {
    // Base 1 looped forever (i /= 1), base 0 divided by zero.
    assert_eq!(halton(5, 1), 0.5);
    assert_eq!(halton(5, 0), 0.5);
    assert!((halton(1, 2) - 0.5).abs() < 1e-7);
    // Primitives with NaN / inf / huge coordinates bin into nothing or a bounded set of tiles.
    let mut s = Scene::default();
    let c = rgba(1.0, 1.0, 1.0, 1.0);
    for a in BAD {
        s.segment([a, 0.0], [10.0, a], 1.0, c, c);
        s.segment([-1e300, -1e300], [1e300, 1e300], 1.0, c, c);
        s.disc([a, a], 4.0, c);
        s.rect([a as f32, 0.0, 1e30, a as f32], 2.0, c, c, 1.0);
        s.text([a as f32, 10.0], 12.0, "x", c);
    }
    for (w, h) in [(0, 0), (1, 1), (800, 600)] {
        let (start, list, tiles) = s.bin(w, h);
        assert!(tiles.len() <= w.div_ceil(16) * h.div_ceil(16));
        assert!(list.len() <= s.prims.len() * tiles.len().max(1));
        let _ = start;
    }
}
