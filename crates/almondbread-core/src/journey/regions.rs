//! Regions of the Mandelbrot set and the anchors a journey crosses over to (kb:
//! decisions/controls "Journey", synthesis/places).
//!
//! **Why regions.** Deep in one place every view is decorated with the same local Julia set
//! (the shapes near c are those of J_c), so a journey that keeps diving in one region shows
//! one family of shapes however its targets are chosen. Variety needs other c: other regions.
//!
//! **Region of a point** (mirror images count as one: the conjugate shows the same shapes).
//! The main cardioid is `c = μ/2 − μ²/4` for |μ| ≤ 1; for any c, `μ = 1 − √(1 − 4c)` gives an
//! internal angle `arg μ / 2π`, which for points outside the cardioid points at the bulb or
//! valley they hang off. The angle is snapped to the nearest p/q (q ≤ 9; ties to the simpler
//! fraction), and the point is in the *valley* beside the p/q bulb when |μ| is close to 1, in
//! its *limb* (bulb, antenna, decorations) otherwise. The period-2 bulb is split the same way
//! about its own centre (−1, radius 1/4), the spike (re < −1.4) is "the needle", and the two
//! named valleys keep their names (seahorse: the 1/2 cusp; elephant: the 0/1 cusp).
//!
//! **Anchors** are shallow views a cross-over flight can land on: the catalogue's places up to
//! 1e16 (their framing is curated) and views derived from the maths of the bulbs: for a
//! seeded p/q, the bulb attached to the cardioid at `a = e^{iθ}/2 − e^{2iθ}/4` (θ = 2πp/q),
//! of radius about `sin(πp/q)/q²`, its nucleus by Newton (period q), the hub of its limb (the
//! principal Misiurewicz point M(q+1, 1), about 2.2 radii beyond the nucleus), and the valley
//! on either side of the attachment point; the same for the bulbs of the period-2 bulb
//! (period 2q, attached at `−1 + e^{iθ}/4`).

use std::f64::consts::{PI, TAU};

use almondbread_math::{NewtonOptions, find_misiurewicz, find_nucleus};

use super::Rng;
use crate::viewport::{Center, Scale};

/// A region id (see [`region_of`]).
pub type RegionId = u32;

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The p/q with q ≤ `qmax` nearest to `t` in [0, 1/2] (ties: smaller q).
fn nearest_fraction(t: f64, qmax: u32) -> (u32, u32) {
    let mut best = (0u32, 1u32, f64::INFINITY);
    for q in 1..=qmax {
        for p in 0..=q / 2 {
            if gcd(p, q) != 1 {
                continue;
            }
            let d = (t - p as f64 / q as f64).abs() * (1.0 + 0.04 * q as f64);
            if d < best.2 {
                best = (p, q, d);
            }
        }
    }
    (best.0, best.1)
}

/// Complex square root (principal branch).
fn csqrt(re: f64, im: f64) -> (f64, f64) {
    let m = re.hypot(im);
    let a = ((m + re) / 2.0).max(0.0).sqrt();
    let b = ((m - re) / 2.0).max(0.0).sqrt();
    (a, if im < 0.0 { -b } else { b })
}

/// The region a point belongs to and its name.
pub fn region_of(re: f64, im: f64) -> (RegionId, String) {
    let im = im.abs();
    if re < -1.4 && im < 0.08 {
        return (1, "the needle".into());
    }
    if (re + 0.75).hypot(im - 0.1) < 0.09 {
        return (2, "seahorse valley".into());
    }
    if (re - 0.28).hypot(im) < 0.07 {
        return (3, "elephant valley".into());
    }
    // The period-2 bulb and what hangs off it.
    let (hx, hy) = (re + 1.0, im);
    let hr = hx.hypot(hy);
    if hr < 0.45 && re < -0.8 {
        let t = (hy.atan2(hx) / TAU).abs();
        let t = t.min(1.0 - t);
        // Internal angle of the period-2 bulb, measured from its attachment point (angle 0
        // there): the bulb's own p/q bulbs.
        let s = t;
        let (p, q) = nearest_fraction(s, 7);
        let limb = hr > 0.25 * (1.0 + 0.5 / (q * q) as f64);
        let id = 2000 + 20 * q + 2 * p + u32::from(limb);
        let name = if limb {
            format!("the {p}/{q} limb of the head")
        } else {
            format!("the head's {p}/{q} valley")
        };
        return (id, name);
    }
    // The cardioid's internal angle.
    let (sr, si) = csqrt(1.0 - 4.0 * re, -4.0 * im);
    let (mr, mi) = (1.0 - sr, -si);
    let t = (mi.atan2(mr) / TAU).abs();
    let t = t.min(1.0 - t);
    let (p, q) = nearest_fraction(t, 9);
    let rmu = mr.hypot(mi);
    let limb = rmu > 1.0 + 0.5 / (q * q) as f64;
    let id = 1000 + 20 * q + 2 * p + u32::from(limb);
    let name = match (limb, p, q) {
        (_, 0, 1) => "elephant valley".to_string(),
        (true, p, q) => format!("the {p}/{q} limb"),
        (false, p, q) => format!("the valley by the {p}/{q} bulb"),
    };
    (id, name)
}

/// Kinds of region, for alternation: what depth shows there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Between a bulb and its parent (seahorse, elephant, the p/q valleys): spirals.
    Valley,
    /// A bulb, its antenna and decorations (p/q limbs, the head's limbs): dendrites, stars.
    Limb,
    /// The spike on the real axis: needles and minibrots in line.
    Needle,
}

/// The family of a region id from [`region_of`].
pub fn family(id: RegionId) -> Family {
    match id {
        1 => Family::Needle,
        2 | 3 => Family::Valley,
        _ if id % 2 == 1 => Family::Limb,
        _ => Family::Valley,
    }
}

/// A view a cross-over can land on.
#[derive(Clone, Debug)]
pub struct Anchor {
    pub center: Center,
    /// Across the view's shorter side.
    pub span: Scale,
    pub label: String,
    pub region: RegionId,
    /// The catalogue place's look (palette id), if it is one.
    pub look: Option<&'static str>,
    pub period: Option<usize>,
    pub preperiod: Option<usize>,
}

/// The catalogue places a cross-over can land on (up to 1e16, not the whole set).
fn place_anchors() -> Vec<Anchor> {
    crate::places::places()
        .iter()
        .filter(|p| p.zoom_log10() > 0.7 && p.zoom_log10() < 16.0)
        .map(|p| {
            let c = p.center();
            Anchor {
                region: region_of(c.re(), c.im()).0,
                center: c,
                span: p.span_scale(),
                label: p.name.to_string(),
                look: Some(p.look),
                period: None,
                preperiod: None,
            }
        })
        .collect()
}

/// Anchors from the maths of one p/q bulb (of the cardioid, or of the period-2 bulb when
/// `head`): the bulb with its antenna, the hub of its limb, the valleys either side.
fn bulb_anchors(p: u32, q: u32, head: bool, flip: bool, out: &mut Vec<Anchor>) {
    let th = TAU * p as f64 / q as f64;
    let (a, tangent, r, period) = if head {
        // Period-2 bulb: centre −1, radius 1/4; θ measured from its attachment point (−3/4).
        let ang = th;
        let a = (-1.0 + 0.25 * ang.cos(), 0.25 * ang.sin());
        let t = (-ang.sin(), ang.cos());
        (
            a,
            t,
            0.25 * (PI * p as f64 / q as f64).sin() / (q * q) as f64 * 1.6,
            2 * q as usize,
        )
    } else {
        let a = (
            0.5 * th.cos() - 0.25 * (2.0 * th).cos(),
            0.5 * th.sin() - 0.25 * (2.0 * th).sin(),
        );
        let t = (
            -0.5 * th.sin() + 0.5 * (2.0 * th).sin(),
            0.5 * th.cos() - 0.5 * (2.0 * th).cos(),
        );
        let m = t.0.hypot(t.1).max(1e-12);
        (
            a,
            (t.0 / m, t.1 / m),
            (PI * p as f64 / q as f64).sin() / (q * q) as f64,
            q as usize,
        )
    };
    // Outward normal: the tangent turned clockwise (θ runs counter-clockwise).
    let mut n = (tangent.1, -tangent.0);
    if head {
        // Outward from the head's centre.
        n = ((a.0 + 1.0) * 4.0, a.1 * 4.0);
    }
    let sgn = if flip { -1.0 } else { 1.0 };
    let pt = |x: f64, y: f64| Center::new(x, sgn * y);
    let bits = 128;
    let start = pt(a.0 + r * n.0, a.1 + r * n.1);
    let nucleus = find_nucleus(
        start.with_min_precision(bits).big(),
        period,
        &NewtonOptions {
            precision_bits: Some(bits),
            max_steps: 64,
            max_distance: Some(Scale::from_f64(r).big_times(1.0, 64)),
        },
        &mut || false,
    )
    .map(|res| Center::from_big(res.nucleus));
    let place = if head {
        format!("the {p}/{q} bulb of the head")
    } else {
        format!("the {p}/{q} bulb")
    };
    let mut push = |center: Center, span: f64, label: String, per: Option<usize>, pre: Option<usize>| {
        out.push(Anchor {
            region: region_of(center.re(), center.im()).0,
            center,
            span: Scale::from_f64(span),
            label,
            look: None,
            period: per,
            preperiod: pre,
        });
    };
    if let Some(nc) = &nucleus {
        // (The bulb itself, framed with its antenna, was a dark disc beside the cardioid in
        // every review: not offered.) The hub of its limb: M(q + 1, 1) beyond the nucleus (period-2 bulb: M(2q + 1, 1)
        // is not reliable; its bulbs get the bulb and the valleys only).
        if !head {
            let guess = pt(nc.re() + 2.2 * r * n.0, sgn * nc.im() + 2.2 * r * n.1);
            if let Some(m) = find_misiurewicz(
                guess.with_min_precision(bits).big(),
                q as usize + 1,
                1,
                &NewtonOptions {
                    precision_bits: Some(bits),
                    max_steps: 48,
                    max_distance: Some(Scale::from_f64(r).big_times(1.5, 64)),
                },
            ) {
                push(
                    Center::from_big(m.point),
                    1.4 * r,
                    format!("the hub of the {p}/{q} limb"),
                    Some(1),
                    Some(q as usize + 1),
                );
            }
        }
    }
    // The valleys either side of the attachment point.
    for side in [-1.0, 1.0] {
        let c = pt(
            a.0 + 0.5 * r * side * tangent.0 + 0.1 * r * n.0,
            a.1 + 0.5 * r * side * tangent.1 + 0.1 * r * n.1,
        );
        push(c, 0.9 * r, format!("a valley by {place}"), None, None);
    }
}

/// Seeded cross-over anchors: the catalogue's shallow places and the maths views of a few
/// random bulbs. Deterministic in the rng state.
pub fn anchors(rng: &mut Rng) -> Vec<Anchor> {
    let mut out = place_anchors();
    for _ in 0..4 {
        let head = rng.f64() < 0.3;
        let q = if head {
            3 + rng.index(4) as u32
        } else {
            3 + rng.index(8) as u32
        };
        let mut p = 1 + rng.index((q / 2) as usize) as u32;
        while gcd(p, q) != 1 {
            p -= 1;
        }
        let flip = rng.f64() < 0.5;
        bulb_anchors(p, q, head, flip, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_of_known_places() {
        assert_eq!(region_of(-0.745, 0.11).1, "seahorse valley");
        assert_eq!(region_of(-0.745, -0.11).0, region_of(-0.745, 0.11).0, "mirror images");
        assert_eq!(region_of(0.28, 0.008).1, "elephant valley");
        assert_eq!(region_of(-1.77, 0.0).1, "the needle");
        // The 1/3 limb's branch point and the 1/3 bulb's nucleus.
        assert_eq!(region_of(-0.1011, 0.9563).1, "the 1/3 limb");
        assert_eq!(region_of(-0.1226, 0.7449).1, "the 1/3 limb");
        assert_ne!(region_of(-0.1011, 0.9563).0, region_of(0.38, 0.33).0);
    }

    #[test]
    fn bulb_anchors_find_their_nuclei_and_hubs() {
        let mut out = Vec::new();
        bulb_anchors(1, 3, false, false, &mut out);
        let hub = out.iter().find(|a| a.label.contains("hub")).expect("hub");
        // M(4,1), the 1/3 limb's principal branch point.
        assert!((hub.center.re() + 0.10109636).abs() < 1e-6, "{}", hub.center.re());
        assert!((hub.center.im() - 0.95628651).abs() < 1e-6, "{}", hub.center.im());
        // The valleys either side of the 1/3 bulb's attachment point (-0.125, 0.6495).
        let valleys: Vec<_> = out.iter().filter(|a| a.label.contains("valley")).collect();
        assert_eq!(valleys.len(), 2);
        for v in valleys {
            assert!(v.center.distance_in(&Center::new(-0.125, 0.6495), Scale::from_f64(0.1)) < 0.6);
            assert_eq!(
                family(v.region),
                Family::Valley,
                "{}",
                region_of(v.center.re(), v.center.im()).1
            );
        }
    }
}
