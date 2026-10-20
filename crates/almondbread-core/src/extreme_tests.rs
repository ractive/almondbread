//! Extreme-value smoke tests (kb: techniques/hang-prevention): absurd inputs (zoom 1e-3010
//! and far beyond, +-inf, NaN, size 0, negative spans) must return quickly with something
//! sane. Plain unit tests, a few calls each; the whole set runs in milliseconds.

use crate::compass::{NorthTurn, drag_turn, ease_in_out, needle_angle};
use crate::cosmic::{CosmicScale, CosmicScaleSettings};
use crate::flyto::view_from_text;
use crate::input::{InputEvent, Navigator};
use crate::viewport::{Center, MAX_SCALE, MIN_SCALE_LOG2, Scale, Viewport};

const BAD: [f64; 7] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0, 1e300, -1e300];

fn scale_ok(v: &Viewport) -> bool {
    let l = v.scale.log2();
    l.is_finite() && v.scale.to_f64() >= 0.0
}

#[test]
fn extreme_viewport_scales_stay_in_range() {
    // Non-positive or non-finite scales fall back to 1; no precision blow-up.
    for s in BAD {
        let v = Viewport::new(-0.75, 0.0, s);
        assert!(scale_ok(&v), "{s}");
        assert!(v.needed_center_bits() <= 12_000, "{s}");
    }
    // Far past the deepest scale (2^-1e9): the centre precision is capped, not 1e9 bits.
    let v = Viewport::with_center(Center::new(-0.75, 0.1), Scale::pow2(-1_000_000_000));
    assert!(v.needed_center_bits() <= 12_000, "{}", v.needed_center_bits());
    let v = Viewport::with_center(Center::new(0.0, 0.0), Scale::pow2(i64::MIN / 4));
    assert!(v.needed_center_bits() <= 12_000);
    // zoom_by ignores bad factors and clamps good ones to [min_scale, MAX_SCALE].
    let mut v = Viewport::home();
    for f in BAD.into_iter().chain([1e308, 1e-308]) {
        v.zoom_by(f, 0.0, 0.0);
        assert!(scale_ok(&v));
        assert!(v.scale >= v.min_scale() && v.scale <= Scale::from_f64(MAX_SCALE), "{f}");
    }
}

#[test]
fn extreme_coordinate_digits_are_bounded() {
    // A pasted 200 000-digit coordinate is rejected quickly; 15 000 digits parse and are
    // rounded to the deepest view's precision.
    let t0 = std::time::Instant::now();
    let long = format!("-0.{}", "7".repeat(200_000));
    assert!(Center::parse(&long, "0.1").is_none());
    let ok = format!("-0.{}", "7".repeat(15_000));
    let c = Center::parse(&ok, "0.1").expect("15 000 digits");
    assert!(
        c.precision_bits() <= crate::viewport::MAX_CENTER_BITS,
        "{}",
        c.precision_bits()
    );
    assert!(c.precision_bits() >= 11_000);
    assert!(t0.elapsed().as_secs_f64() < 0.5, "{:?}", t0.elapsed());
}

#[test]
fn extreme_view_text_is_rejected_or_clamped() {
    // "1e-999999999" used to compute 10^999999999 exactly (a beach ball on paste).
    let t0 = std::time::Instant::now();
    assert!(view_from_text("-0.75", "0.1", "1e-999999999").is_none());
    assert!(view_from_text("1e999999999", "0", "1e-3").is_none());
    assert!(view_from_text("-0.75", "0.1", "-1e-5").is_none());
    assert!(view_from_text("nan", "0", "1").is_none());
    // Deeper than the limit but parseable: clamped to the deepest scale.
    let v = view_from_text("-0.75", "0.1", "1e-5000").expect("view");
    assert_eq!(v.scale, Scale::pow2(MIN_SCALE_LOG2));
    assert!(t0.elapsed().as_secs_f64() < 0.5, "{:?}", t0.elapsed());
}

#[test]
fn extreme_navigator_survives_bad_times_and_sizes() {
    let mut n = Navigator::default();
    let mut v = Viewport::new(-0.5, 0.25, 1e-3);
    for (w, h) in [
        (f64::NAN, 600.0),
        (f64::INFINITY, 1.0),
        (-5.0, -5.0),
        (0.0, 0.0),
        (1000.0, 600.0),
    ] {
        n.handle(InputEvent::Resize { width: w, height: h }, &mut v, 0.0);
    }
    for e in [
        InputEvent::ZoomBy { factor: f64::NAN },
        InputEvent::ZoomBy { factor: 0.0 },
        InputEvent::ZoomBy { factor: f64::INFINITY },
        InputEvent::WheelZoom {
            delta: f64::NAN,
            x: 0.0,
            y: 0.0,
        },
        InputEvent::Magnify {
            magnification: f64::INFINITY,
            x: 0.0,
            y: 0.0,
        },
        InputEvent::Pan {
            dx: f64::NAN,
            dy: f64::INFINITY,
        },
        InputEvent::Rotate {
            radians: f64::NAN,
            x: 0.0,
            y: 0.0,
        },
    ] {
        n.handle(e, &mut v, 0.1);
    }
    // Huge finite pans, wheels and pinches (x * scale overflowed to inf in Scale::big_times
    // and hit BigFloat::from_f64's assertion).
    let mut huge = Viewport::new(-0.5, 0.25, 1e-3);
    for e in [
        InputEvent::Pan { dx: 1e308, dy: -1e308 },
        InputEvent::Pan {
            dx: f64::MAX,
            dy: f64::MAX,
        },
        InputEvent::WheelZoom {
            delta: 1e308,
            x: 1e308,
            y: -1e308,
        },
        InputEvent::Magnify {
            magnification: 1e308,
            x: -1e308,
            y: 1e308,
        },
        InputEvent::ZoomBy { factor: 1e308 },
    ] {
        n.handle(e, &mut huge, 0.1);
        n.advance(&mut huge, 0.2);
    }
    assert!(scale_ok(&huge));
    for x in [1e308, -f64::MAX, f64::MAX, f64::NAN] {
        let b = Scale::from_f64(1e-3).big_times(x, 64);
        assert!(!b.to_f64().is_nan(), "{x}");
    }
    huge.pan_by_points(f64::MAX, -f64::MAX);
    assert!(scale_ok(&huge));
    // Bad clocks: NaN, backwards, a huge jump. Frames end; the view stays usable.
    let mut frames = 0;
    for t in [f64::NAN, -1e9, 0.2, f64::INFINITY, 1e12, f64::NAN] {
        n.advance(&mut v, t);
        frames += 1;
    }
    assert_eq!(frames, 6);
    assert!(scale_ok(&v));
    assert!(v.center.re().is_finite() && v.center.im().is_finite(), "{:?}", v.center);
}

#[test]
fn extreme_cosmic_scale_readings() {
    for mpp in [f64::NAN, f64::INFINITY, -1.0, 0.0, 3e-4] {
        for w in [f64::NAN, 0.0, -10.0, f64::INFINITY, 1e300] {
            let s = CosmicScaleSettings {
                enabled: true,
                metres_per_point: mpp,
            };
            let c = CosmicScale::new(&s, w);
            assert!(c.universe_d.is_finite());
            for d in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 3010.0, 1e300, -1e300] {
                let r = c.reading(d);
                assert!(!r.line.is_empty());
                let _ = (c.ruler(d), c.passed(d));
            }
        }
    }
}

#[test]
fn extreme_compass_maths() {
    for a in BAD {
        let _ = needle_angle(a);
        let _ = ease_in_out(a);
        let _ = drag_turn((a, 1.0), (1.0, a));
    }
    // The north turn ends for any angle and any (finite or infinite) frame time.
    for (angle, dt) in [
        (1e300, 1.0 / 60.0),
        (f64::INFINITY, 1.0 / 60.0),
        (f64::NAN, 1e-3),
        (2.0, f64::INFINITY),
    ] {
        let mut t = NorthTurn::default();
        let done = (0..1000).any(|_| t.step(angle, dt).1);
        assert!(done, "{angle} {dt}");
    }
}
