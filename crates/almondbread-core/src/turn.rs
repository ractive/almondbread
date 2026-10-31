//! F9: the turn between the Mandelbrot and the Julia plane, and the start value `z_0`
//! (kb: plans/f9-turn, formulas/turn).
//!
//! `z <- z^2 + c` has two complex inputs, the start `z_0` and `c`: a 4-dimensional space. The
//! Mandelbrot set is the plane where the pixel is `c` (`z_0 = 0`), a Julia set the plane where
//! the pixel is `z_0` (`c = k` fixed). A view of almondbread is a plane of that space through a
//! pivot `(c*, z*)`, turned by `t` from the Mandelbrot plane towards the Julia plane: the pixel
//! at view coordinate `p` is
//!
//! ```text
//! c   = c* + cos t (p - P)
//! z_0 = z* + sin t (p - P),      P = cos t c* + sin t z*   (the pivot's view coordinate)
//! ```
//!
//! i.e. `p` is the coordinate along the turned axis `(cos t, sin t)` and the plane's other
//! coordinate `-sin t c + cos t z_0` is fixed. At `t = 0` the view coordinate is `c` and the
//! plane holds `z_0 = z*` (Mandelbrot with a start value; `z* = 0`: the Mandelbrot set itself,
//! [`Formula::Mandelbrot`]); at `t = 90°` it is `z_0` and the plane holds `c = c*` (the Julia
//! set of `k = c*`, [`Formula::Julia`], the same viewport as F4's). Between them it is
//! [`Formula::Turn`].
//!
//! **Turning** keeps the view centre's point of the 4-space: the new plane's pivot is that
//! point and the new view centre its view coordinate, so whatever is in the middle of the screen
//! stays there and turning back returns exactly (the centre's point is the pivot itself: no
//! rounding at the ends). Panning at a fixed angle keeps the plane. Setting `z_0` moves the
//! centre's point to the new start with the same `c` (and the angle).
//!
//! **Deep zoom** (kb: plans/f9-turn "Design"): a pixel's offset `dp` from the reference's view
//! coordinate gives `dz_0 = sin t dp` and `dc = cos t dp`. Perturbation against the reference's
//! orbit (from its own `z_0`, with its `c`) steps `z' = 2Zz + z^2 + dc` from `z = dz_0`;
//! Zhuoran's rebase needs an orbit through 0 with the same `c`: the critical orbit (the orbit of
//! 0 under `z^2 + C`, the Mandelbrot orbit of the reference's `c`). So a turned reference is
//! laid out as F4's Julia reference ([`crate::julia_reference`]): the critical orbit, zeros, the
//! centre orbit. The BLA is Mandelbrot's (`B = 1` per step), applied to `dz` and `dc`.

use almondbread_math::{BigComplex, BigFloat, TURN_MAX_CENTIDEGREES, TurnPlane};

use crate::formula::{Formula, JuliaK};
use crate::viewport::{Center, SetBox, Viewport};

/// The turn angle of a formula in hundredths of a degree: 0 for the Mandelbrot set, 9000 for a
/// Julia set, the plane's for a turned view; None for formulas the turn does not apply to
/// (Multibrot, Mandelbar, Burning Ship: `z^2 + c` only).
pub fn centidegrees(formula: Formula) -> Option<u16> {
    match formula {
        Formula::Mandelbrot => Some(0),
        Formula::Julia(_) => Some(TURN_MAX_CENTIDEGREES),
        Formula::Turn(t) => Some(t.centidegrees()),
        _ => None,
    }
}

/// Whether the turn and the start value apply to `formula` (the `z^2 + c` family).
pub fn applies(formula: Formula) -> bool {
    centidegrees(formula).is_some()
}

/// Whether `formula` is a plane the ordinary formulas do not cover (turned, or a start value).
pub fn is_turned(formula: Formula) -> bool {
    matches!(formula, Formula::Turn(_))
}

/// The point `(c, z_0)` of the 4-space at view coordinate `p` of `formula`, at `bits`; None
/// where the turn does not apply.
pub fn point_of(formula: Formula, p: &BigComplex, bits: usize) -> Option<(BigComplex, BigComplex)> {
    match formula {
        Formula::Mandelbrot => Some((p.with_precision(bits), BigComplex::zero(bits))),
        Formula::Julia(k) => Some((
            k.value().with_precision(bits.max(k.value().precision_bits())),
            p.with_precision(bits),
        )),
        Formula::Turn(t) => {
            let pv = t.pivot();
            let b = bits.max(pv.0.precision_bits()).max(pv.1.precision_bits());
            Some(t.point(p, b))
        }
        _ => None,
    }
}

/// [`point_of`] in f64.
pub fn point_of_f64(formula: Formula, p: (f64, f64)) -> Option<((f64, f64), (f64, f64))> {
    match formula {
        Formula::Mandelbrot => Some((p, (0.0, 0.0))),
        Formula::Julia(k) => Some((k.approx(), p)),
        Formula::Turn(t) => Some(t.point_f64(p)),
        _ => None,
    }
}

fn is_zero(z: &BigComplex) -> bool {
    z.re.is_zero() && z.im.is_zero()
}

/// The plane at `centidegrees` through the point `(c, z0)`, and that point's view coordinate:
/// the Mandelbrot set at 0° with `z0 = 0` (view coordinate `c`), a Julia set of `k = c` at 90°
/// (view coordinate `z0`), else a turned plane (at 0° its pivot is `(0, z0)`: one plane per
/// start value whatever `c`, view coordinate `c`).
pub fn plane_through(centidegrees: u16, c: &BigComplex, z0: &BigComplex) -> (Formula, BigComplex) {
    let t = centidegrees.min(TURN_MAX_CENTIDEGREES);
    if t == 0 {
        if is_zero(z0) {
            return (Formula::Mandelbrot, c.clone());
        }
        let zero = BigComplex::zero(64);
        return (Formula::Turn(TurnPlane::new(&zero, z0, 0)), c.clone());
    }
    if t == TURN_MAX_CENTIDEGREES {
        return (Formula::Julia(JuliaK::new(c)), z0.clone());
    }
    let plane = TurnPlane::new(c, z0, t);
    (Formula::Turn(plane), plane.pivot_view())
}

/// The view turned to `centidegrees` around its centre's point (the centre keeps its point of
/// the 4-space, the scale and the rotation stay); None where the turn does not apply.
pub fn turned(formula: Formula, view: &Viewport, centidegrees: u16) -> Option<(Formula, Viewport)> {
    let (c, z0) = centre_point(formula, view)?;
    let (f, p) = plane_through(centidegrees, &c, &z0);
    Some((f, with_center(view, p)))
}

/// The view with the start value of its centre set to `z0` (the centre keeps its `c` and the
/// angle); None where the turn does not apply. At 90° (a Julia view, where the pixel is `z_0`)
/// that moves the view to `z0`.
pub fn with_start(formula: Formula, view: &Viewport, z0: &BigComplex) -> Option<(Formula, Viewport)> {
    let t = centidegrees(formula)?;
    let (c, _) = centre_point(formula, view)?;
    let (f, p) = plane_through(t, &c, z0);
    Some((f, with_center(view, p)))
}

/// The centre's point of the 4-space at the precision a pivot keeps: what the view needs, or
/// more where the centre's coordinate (Mandelbrot, Julia) or the old pivot has more. A centre at
/// the pivot gives the pivot exactly (so turning back and forth reuses it and returns exactly);
/// a panned one is rounded to that precision (sub-pixel), so pivots do not grow with each turn.
fn centre_point(formula: Formula, view: &Viewport) -> Option<(BigComplex, BigComplex)> {
    let keep = match formula {
        Formula::Turn(t) => {
            let pv = t.pivot();
            pv.0.precision_bits().max(pv.1.precision_bits())
        }
        Formula::Julia(k) => view.center.precision_bits().max(k.value().precision_bits()),
        _ => view.center.precision_bits(),
    };
    let bits = view.needed_center_bits().max(keep);
    let (c, z) = point_of(formula, view.center.big(), bits)?;
    Some((c.with_precision(bits), z.with_precision(bits)))
}

fn with_center(view: &Viewport, p: BigComplex) -> Viewport {
    let mut v = view.clone();
    v.center = Center::from_big(p);
    v.ensure_precision();
    v
}

/// The start value `z_0` and the parameter `c` of the view centre (f64, for the UI); None
/// where the turn does not apply.
pub fn centre_point_f64(formula: Formula, view: &Viewport) -> Option<((f64, f64), (f64, f64))> {
    let (c, z) = point_of(formula, view.center.big(), view.center.precision_bits())?;
    let (c, z) = (c.to_f64(), z.to_f64());
    Some(((c[0], c[1]), (z[0], z[1])))
}

/// The home view of a turned plane (R): Mandelbrot's framing, centred where the plane passes
/// closest to the middle of the sets (`c = -1/2`, `z_0 = 0`): `p = P - (cos t (c* + 1/2) + sin t
/// z*)`, which is `-1/2` at 0° and 0 at 90°.
pub fn home_view(plane: TurnPlane, width: f64, height: f64) -> Viewport {
    let base = Viewport::fitting(width, height);
    let pv = plane.pivot();
    let (cos, sin) = plane.cos_sin();
    let (c, z) = (pv.0.to_f64(), pv.1.to_f64());
    let pp = (cos * c[0] + sin * z[0], cos * c[1] + sin * z[1]);
    let w = (-(cos * (c[0] + 0.5) + sin * z[0]), -(cos * c[1] + sin * z[1]));
    let mut v = base.clone();
    v.center = Center::new(pp.0 + w.0, pp.1 + w.1);
    // (The Julia framing is a little wider than Mandelbrot's: the wider of the two.)
    let jul = crate::julia::home_view(width, height);
    if jul.scale > v.scale {
        v.scale = jul.scale;
    }
    v
}

/// The centre bound of a turned plane (kb: decisions/controls "View limits"): every bounded
/// orbit there has `|c|` and `|z_0|` within about 2.5 (generous: a start value moves the set),
/// so the view coordinates `p = P + w` with `c* + cos t w` and `z* + sin t w` in that square.
pub fn set_box(plane: TurnPlane) -> SetBox {
    const R: f64 = 2.5;
    let pv = plane.pivot();
    let (cos, sin) = plane.cos_sin();
    let (c, z) = (pv.0.to_f64(), pv.1.to_f64());
    let pp = (cos * c[0] + sin * z[0], cos * c[1] + sin * z[1]);
    let mut lo = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut hi = (f64::INFINITY, f64::INFINITY);
    for (f, v) in [(cos, c), (sin, z)] {
        if f > 1e-9 {
            lo = (lo.0.max((-R - v[0]) / f), lo.1.max((-R - v[1]) / f));
            hi = (hi.0.min((R - v[0]) / f), hi.1.min((R - v[1]) / f));
        }
    }
    if !(lo.0 <= hi.0 && lo.1 <= hi.1) {
        return SetBox::new(pp.0 - R, pp.0 + R, pp.1 - R, pp.1 + R);
    }
    SetBox::new(pp.0 + lo.0, pp.0 + hi.0, pp.1 + lo.1, pp.1 + hi.1)
}

/// The reference's start and parameter for view coordinate `p` (its orbit is `Z_0 = z_0`,
/// `Z <- Z^2 + c`), at `bits`.
pub fn reference_point(plane: TurnPlane, p: &BigComplex, bits: usize) -> (BigComplex, BigComplex) {
    let pv = plane.pivot();
    let b = bits.max(pv.0.precision_bits()).max(pv.1.precision_bits());
    let (c, z) = plane.point(p, b);
    (c.with_precision(bits.max(64)), z.with_precision(bits.max(64)))
}

/// Bits a turned reference needs beyond the view's (whole limbs): the pixel spacing is
/// `cos t texel` in `c` and `sin t texel` in `z_0`, so the smaller non-zero factor costs
/// `-log2` of it (0.01° from an end: 13 bits).
pub fn extra_bits(plane: TurnPlane) -> usize {
    let (cos, sin) = plane.cos_sin();
    let f = [cos, sin].into_iter().filter(|&x| x > 0.0).fold(1.0f64, f64::min);
    ((-f.log2()).ceil().max(0.0) as usize).div_ceil(64) * 64
}

/// `t` as the HUD shows it: "30°", "12.5°".
pub fn degrees_text(centidegrees: u16) -> String {
    let d = f64::from(centidegrees) / 100.0;
    if centidegrees.is_multiple_of(100) {
        format!("{}°", centidegrees / 100)
    } else {
        format!("{d}°")
    }
}

/// Parses the `turn` line of a view's text (`turn <degrees> <c re> <c im> <z0 re> <z0 im>`: the
/// centre's point of the 4-space and the angle; the view coordinate follows from them).
pub fn parse_turn_line(text: &str) -> Option<(u16, Center, Center)> {
    text.lines().find_map(|line| {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 6 || parts[0] != "turn" {
            return None;
        }
        let d: f64 = parts[1].trim_end_matches('°').parse().ok()?;
        if !(0.0..=90.0).contains(&d) {
            return None;
        }
        let c = Center::parse(parts[2], parts[3])?;
        let z = Center::parse(parts[4], parts[5])?;
        let ok = |p: &Center| p.re().is_finite() && p.im().is_finite();
        (ok(&c) && ok(&z)).then_some(((d * 100.0).round() as u16, c, z))
    })
}

/// The `turn` line for a turned view (None for the others), with `digits` digits.
pub fn turn_line(formula: Formula, view: &Viewport, digits: usize) -> Option<String> {
    let Formula::Turn(t) = formula else {
        return None;
    };
    let (c, z) = point_of(formula, view.center.big(), view.center.precision_bits())?;
    let s = |x: &BigFloat| x.to_decimal_string(Some(digits));
    let d = f64::from(t.centidegrees()) / 100.0;
    Some(format!("turn {d} {} {} {} {}", s(&c.re), s(&c.im), s(&z.re), s(&z.im)))
}

/// The plane and view of a parsed `turn` line, with the text's viewport for scale and angle.
pub fn from_turn_line(centidegrees: u16, c: &Center, z0: &Center, view: &Viewport) -> (Formula, Viewport) {
    let (f, p) = plane_through(centidegrees, c.big(), z0.big());
    (f, with_center(view, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(re: f64, im: f64) -> BigComplex {
        BigComplex::from_f64(re, im, 128)
    }

    #[test]
    fn the_ends_are_the_ordinary_formulas() {
        let c = big(-0.75, 0.1);
        let (f, p) = plane_through(0, &c, &BigComplex::zero(128));
        assert_eq!(f, Formula::Mandelbrot);
        assert_eq!(p, c);
        let z = big(0.25, -0.5);
        let (f, p) = plane_through(9000, &c, &z);
        assert_eq!(f, Formula::Julia(JuliaK::new(&c)));
        assert_eq!(p, z);
        let (f, p) = plane_through(0, &c, &z);
        assert!(is_turned(f));
        assert_eq!(p, c);
        // At 0° the plane is the start value's, whatever c: pans keep the formula.
        assert_eq!(plane_through(0, &big(0.3, 0.0), &z).0, f);
        let (pc, pz) = point_of(f, &big(0.1, 0.2), 128).unwrap();
        assert_eq!((pc, pz), (big(0.1, 0.2), z));
    }

    #[test]
    fn turning_keeps_the_centre_and_turns_back_exactly() {
        let view = Viewport::new(-0.743643887037151, 0.131825904205330, 1e-10);
        let (f30, v30) = turned(Formula::Mandelbrot, &view, 3000).unwrap();
        let (c, z) = point_of(f30, v30.center.big(), 256).unwrap();
        let (c0, z0) = point_of(Formula::Mandelbrot, view.center.big(), 256).unwrap();
        assert_eq!(c.with_precision(256), c0);
        assert_eq!(z.with_precision(256), z0);
        // Turning again around the same point reuses the pivot.
        let (f60, v60) = turned(f30, &v30, 6000).unwrap();
        let (f60b, _) = turned(Formula::Mandelbrot, &view, 6000).unwrap();
        assert_eq!(f60, f60b);
        let (f0, v0) = turned(f60, &v60, 0).unwrap();
        assert_eq!(f0, Formula::Mandelbrot);
        assert_eq!(
            v0.center.big().with_precision(256),
            view.center.big().with_precision(256)
        );
        let (f90, v90) = turned(f60, &v60, 9000).unwrap();
        assert_eq!(f90.julia_k().unwrap().approx(), (view.center.re(), view.center.im()));
        assert_eq!((v90.center.re(), v90.center.im()), (0.0, 0.0));
        // A pixel offset: c moves by cos t, z_0 by sin t of it.
        let Formula::Turn(t) = f30 else { panic!() };
        let (cos, sin) = t.cos_sin();
        let q = v30.center.offset(1e-12, 0.0);
        let (c1, z1) = point_of(f30, q.big(), 256).unwrap();
        let dc = (&c1 - &c).to_f64();
        let dz = (&z1 - &z).to_f64();
        assert!((dc[0] - cos * 1e-12).abs() < 1e-24 && dc[1].abs() < 1e-24);
        assert!((dz[0] - sin * 1e-12).abs() < 1e-24 && dz[1].abs() < 1e-24);
    }

    #[test]
    fn start_values_and_lines_round_trip() {
        let view = Viewport::new(-0.5, 0.25, 0.003);
        let (f, v) = with_start(Formula::Mandelbrot, &view, &big(0.125, -0.25)).unwrap();
        assert!(is_turned(f));
        assert_eq!(centre_point_f64(f, &v), Some(((-0.5, 0.25), (0.125, -0.25))));
        let (f2, v2) = with_start(f, &v, &BigComplex::zero(64)).unwrap();
        assert_eq!(f2, Formula::Mandelbrot);
        assert_eq!((v2.center.re(), v2.center.im()), (-0.5, 0.25));
        let (ft, vt) = turned(f, &v, 4550).unwrap();
        let line = turn_line(ft, &vt, 30).unwrap();
        assert!(line.starts_with("turn 45.5 "), "{line}");
        let (d, c, z) = parse_turn_line(&line).unwrap();
        assert_eq!(d, 4550);
        let (ft2, vt2) = from_turn_line(d, &c, &z, &vt);
        assert_eq!(ft2.turn().unwrap().centidegrees(), 4550);
        assert!((vt2.center.re() - vt.center.re()).abs() < 1e-15);
        assert!((vt2.center.im() - vt.center.im()).abs() < 1e-15);
        assert_eq!(degrees_text(4550), "45.5°");
        assert_eq!(degrees_text(3000), "30°");
        assert!(!applies(Formula::BurningShip) && applies(Formula::Mandelbrot));
    }

    #[test]
    fn home_views_and_boxes_hold_the_set_middle() {
        let view = Viewport::new(-0.75, 0.1, 1e-3);
        for t in [1u16, 3000, 6000, 8999] {
            let (f, _) = turned(Formula::Mandelbrot, &view, t).unwrap();
            let plane = f.turn().unwrap();
            let h = home_view(plane, 1000.0, 800.0);
            let ((c, z), b) = (plane.point_f64((h.center.re(), h.center.im())), set_box(plane));
            // The home centre's c and z_0 are nearer the sets' middles than the pivot's.
            assert!(
                (c.0 + 0.5).hypot(c.1) + z.0.hypot(z.1) <= 0.75 + 0.1 + 1e-9,
                "{t}: {c:?} {z:?}"
            );
            let (re, im) = (h.center.re(), h.center.im());
            assert!(
                b.re.0 <= re && re <= b.re.1 && b.im.0 <= im && im <= b.im.1,
                "{t}: {b:?}"
            );
        }
    }
}
