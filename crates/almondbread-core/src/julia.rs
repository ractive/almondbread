//! The filled Julia set of `z ← z² + k` for the Julia companion panel (kb: formulas/julia,
//! plans/f1-julia-view): the panel's framing, an f64 CPU reference of its iteration field (the
//! ground truth the GPU kernel is tested against, and the CLI), and the attracting cycle of `k`
//! (the panel's interior colouring).
//!
//! Shallow only: direct iteration from `z₀ = pixel`, no perturbation (deep Julia is
//! plans/f4-deep-julia).

use rayon::prelude::*;

use crate::formula::{Formula, JuliaK};
use crate::oracle::{ITER_INTERIOR, IterationField};
use crate::viewport::Viewport;

/// Escape radius, as the Mandelbrot kernels (smooth offsets in the same range).
pub const BAILOUT: f64 = 65536.0;

/// Iteration limit of the panel: enough for the shapes at panel resolution (parabolic `k`
/// leave more pixels unresolved; they stay interior-coloured).
pub const MAX_ITERATIONS: usize = 1000;

/// The framing (z units): every Julia set of a `k` in the Mandelbrot set lies in `|z| ≤ 2`
/// (`|k| ≤ 2`); in practice in `|Re z| ≤ 2` and `|Im z| ≤ 1.5`, which a 0.72-high panel shows
/// without cutting the c = −2 segment or the dendrites' tips.
pub const SPAN_RE: f64 = 4.2;
pub const SPAN_IM: f64 = 3.0;

/// z units per pixel of a `w` × `h` panel, centred on 0.
pub fn panel_scale(w: usize, h: usize) -> f64 {
    (SPAN_RE / w.max(1) as f64).max(SPAN_IM / h.max(1) as f64)
}

/// The whole Julia set as a main view (F4, kb: plans/f4-deep-julia): the panel's framing,
/// centred on 0, for a view of `w` x `h` points (or pixels).
pub fn home_view(w: f64, h: f64) -> Viewport {
    if !(w > 0.0 && h > 0.0) {
        return Viewport::new(0.0, 0.0, SPAN_RE / 1000.0);
    }
    Viewport::new(0.0, 0.0, (SPAN_RE / w).max(SPAN_IM / h))
}

/// The bounding box of the filled Julia set of `z² + k` (the centre bound of a Julia view, kb:
/// decisions/controls "View limits"): measured once per `k` (then cached) on a 384² grid over
/// the escape disc `|z| ≤ (1 + √(1 + 4|k|))/2`. A grid point counts when its orbit stays
/// bounded for 400 steps or its distance estimate `|z| ln|z| / |dz/dz₀|` is below 1.5 cells
/// (that finds dendrites and dust a grid misses); the box is the counted points' extent plus
/// half a cell, symmetric (the set is symmetric under z ↦ −z). Nothing counted (dust too fine):
/// the escape disc's box.
pub fn set_box(k: (f64, f64)) -> crate::viewport::SetBox {
    use crate::viewport::SetBox;
    use std::sync::Mutex;
    static CACHE: Mutex<Vec<((f64, f64), SetBox)>> = Mutex::new(Vec::new());
    if !(k.0.is_finite() && k.1.is_finite()) {
        return SetBox::new(-2.0, 2.0, -2.0, 2.0);
    }
    if let Ok(c) = CACHE.lock()
        && let Some(&(_, b)) = c.iter().find(|(kk, _)| *kk == k)
    {
        return b;
    }
    let r = 0.5 * (1.0 + (1.0 + 4.0 * k.0.hypot(k.1)).sqrt());
    const N: usize = 384;
    let h = 2.0 * r / N as f64;
    let (xm, ym) = (0..N)
        .into_par_iter()
        .map(|i| {
            let y = -r + (i as f64 + 0.5) * h;
            let mut m = (f64::NEG_INFINITY, f64::NEG_INFINITY);
            for j in 0..N {
                let x = -r + (j as f64 + 0.5) * h;
                if julia_near(k, (x, y), 400, 1.5 * h) {
                    m = (m.0.max(x.abs()), m.1.max(y.abs()));
                }
            }
            m
        })
        .reduce(
            || (f64::NEG_INFINITY, f64::NEG_INFINITY),
            |a, b| (a.0.max(b.0), a.1.max(b.1)),
        );
    let b = if xm.is_finite() && ym.is_finite() {
        let (x, y) = (xm + 0.5 * h, ym + 0.5 * h);
        SetBox::new(-x, x, -y, y)
    } else {
        SetBox::new(-r, r, -r, r)
    };
    if let Ok(mut c) = CACHE.lock() {
        if c.len() >= 8 {
            c.remove(0);
        }
        c.push((k, b));
    }
    b
}

/// `z₀` is in the filled Julia set of `z² + k` (bounded for `max_iter` steps) or within `eps` of
/// it by the distance estimate.
pub(crate) fn julia_near(k: (f64, f64), z0: (f64, f64), max_iter: usize, eps: f64) -> bool {
    let (mut x, mut y) = z0;
    let (mut dx, mut dy) = (1.0, 0.0);
    for _ in 0..max_iter {
        // dz ← 2 z dz, z ← z² + k.
        let (ndx, ndy) = (2.0 * (x * dx - y * dy), 2.0 * (x * dy + y * dx));
        (dx, dy) = (ndx, ndy);
        let t = x * x - y * y + k.0;
        y = 2.0 * x * y + k.1;
        x = t;
        let r2 = x * x + y * y;
        if r2 > 1e12 {
            let r = r2.sqrt();
            return r * r.ln() / dx.hypot(dy) < eps;
        }
    }
    true
}

/// Auto iterations of a Julia view (F4): linear in the depth (measured 2026-10-21 at the step-1
/// gate views: escape counts to about 3 200 at 1e-300 and 6 000 at 1e-1000, generous margin;
/// the learner raises it where pixels stay unresolved).
pub fn depth_iterations(viewport: &Viewport) -> usize {
    let depth = viewport.zoom_log10().max(0.0);
    (1000.0 + 40.0 * depth) as usize
}

/// The Julia twin of a view (F4): the Julia set of the view's centre `c`, framed exactly like the
/// view (around a Misiurewicz point the two look alike, Tan Lei; kb: formulas/julia). `k` is the
/// centre at full precision.
pub fn twin(view: &Viewport) -> (Formula, Viewport) {
    let k = view.center.with_min_precision(view.needed_center_bits());
    (Formula::Julia(JuliaK::new(k.big())), view.clone())
}

/// `k` for the HUD and the clipboard: `re ± im i` with `digits` significant digits.
pub fn k_text(k: JuliaK, digits: usize) -> String {
    let v = k.value();
    let re = v.re.to_decimal_string(Some(digits));
    let im = v.im.abs().to_decimal_string(Some(digits));
    let sign = if v.im.is_negative() { '−' } else { '+' };
    format!("{re} {sign} {im}i")
}

/// The `julia re im` line of a view's text (F4: `Host::coordinates_text` writes it for a Julia
/// view): `k` at the precision its digits imply.
pub fn parse_k_line(text: &str) -> Option<crate::viewport::Center> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next(), parts.next()) {
            (Some("julia"), Some(re), Some(im)) => {
                crate::viewport::Center::parse(re, im).filter(|k| k.re().is_finite() && k.im().is_finite())
            }
            _ => None,
        }
    })
}

/// The start `z₀` of pixel (x, y) (its centre; y down) in a `w` × `h` panel at `scale`.
pub fn pixel_z(x: usize, y: usize, w: usize, h: usize, scale: f64) -> (f64, f64) {
    (
        (x as f64 + 0.5 - w as f64 / 2.0) * scale,
        (h as f64 / 2.0 - y as f64 - 0.5) * scale,
    )
}

/// The escape of `z₀` under `z² + k`: (n, smooth offset s) as the kernels store them
/// (`s = 1 − log2(log|z|)`), or [`ITER_INTERIOR`] when it does not escape within `max_iter`.
pub fn escape(k: (f64, f64), z0: (f64, f64), max_iter: usize) -> (f32, f32) {
    let b2 = BAILOUT * BAILOUT;
    let (mut zr, mut zi) = z0;
    let mut r2 = zr * zr + zi * zi;
    let mut n = 0usize;
    while n < max_iter && r2 <= b2 {
        let t = zr * zr - zi * zi + k.0;
        zi = 2.0 * zr * zi + k.1;
        zr = t;
        n += 1;
        r2 = zr * zr + zi * zi;
    }
    if r2 > b2 {
        (n as f32, (1.0 - (0.5 * r2.ln()).log2()) as f32)
    } else {
        (ITER_INTERIOR, 0.0)
    }
}

/// The panel's field in f64 (rows in parallel): what the GPU kernel `juliaField` computes in
/// float32.
pub fn cpu_field(k: (f64, f64), w: usize, h: usize, max_iter: usize) -> IterationField {
    let scale = panel_scale(w, h);
    let mut n = vec![0f32; w * h];
    let mut s = vec![0f32; w * h];
    n.par_chunks_mut(w.max(1))
        .zip(s.par_chunks_mut(w.max(1)))
        .enumerate()
        .for_each(|(y, (nrow, srow))| {
            for x in 0..w {
                let (a, b) = escape(k, pixel_z(x, y, w, h, scale), max_iter);
                nrow[x] = a;
                srow[x] = b;
            }
        });
    IterationField {
        width: w,
        height: h,
        n,
        s,
    }
}

/// An attracting cycle of `z² + k`: a point of it, its period and `|λ|` (the multiplier over
/// one period).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cycle {
    pub point: (f64, f64),
    pub period: usize,
    pub multiplier: f64,
}

/// Longest cycle looked for (the panel's interior shading; longer ones stay flat).
pub const MAX_PERIOD: usize = 4096;

/// The attracting cycle of `k`, if it has one we can find: the critical orbit (from 0, which is
/// the Mandelbrot orbit of `k`) converges to it whenever there is one (Fatou), so iterate it,
/// then look for the smallest p with `z_{n+p} ≈ z_n`. None when the orbit escapes, or has not
/// settled within the budget (parabolic and near-parabolic `k`, Siegel discs, the boundary).
/// Every pixel in the filled Julia set's interior then falls into the same cycle (inference,
/// kb: synthesis/other-fractal-types §2a). Cost: at most ~24 000 f64 steps (tens of µs).
pub fn attracting_cycle(k: (f64, f64)) -> Option<Cycle> {
    const SETTLE: usize = 20_000;
    let step = |z: (f64, f64)| (z.0 * z.0 - z.1 * z.1 + k.0, 2.0 * z.0 * z.1 + k.1);
    let mut z = (0.0, 0.0);
    for _ in 0..SETTLE {
        z = step(z);
        if z.0 * z.0 + z.1 * z.1 > 16.0 {
            return None;
        }
    }
    let start = z;
    let mut w = start;
    let mut lambda = (1.0f64, 0.0f64);
    let tol2 = 1e-18 * (1.0 + start.0 * start.0 + start.1 * start.1);
    for p in 1..=MAX_PERIOD {
        lambda = (
            2.0 * (lambda.0 * w.0 - lambda.1 * w.1),
            2.0 * (lambda.0 * w.1 + lambda.1 * w.0),
        );
        w = step(w);
        let (dr, di) = (w.0 - start.0, w.1 - start.1);
        if dr * dr + di * di <= tol2 {
            let m = lambda.0.hypot(lambda.1);
            return (m < 1.0).then_some(Cycle {
                point: start,
                period: p,
                multiplier: m,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_of_known_points() {
        // The basilica: c = −1, the critical orbit 0, −1, 0, ... (superattracting, λ = 0).
        let c = attracting_cycle((-1.0, 0.0)).expect("basilica");
        assert_eq!(c.period, 2);
        assert!(c.multiplier < 1e-9);
        // The rabbit: the period-3 nucleus.
        let c = attracting_cycle((-0.122_561_166_876_653_6, 0.744_861_766_619_744_2)).expect("rabbit");
        assert_eq!(c.period, 3);
        // Inside the main cardioid: a fixed point.
        assert_eq!(attracting_cycle((0.1, 0.1)).expect("cardioid").period, 1);
        // Outside: escapes. The Misiurewicz point i: preperiodic, repelling, no attracting cycle.
        assert!(attracting_cycle((0.5, 0.5)).is_none());
        assert!(attracting_cycle((0.0, 1.0)).is_none());
    }

    #[test]
    fn field_shapes() {
        let (w, h) = (90, 64);
        // The basilica is connected and contains 0 and the fixed point; the dust of 0.5 + 0.5i
        // has no pixel that stays.
        let b = cpu_field((-1.0, 0.0), w, h, MAX_ITERATIONS);
        let centre = (h / 2) * w + w / 2;
        assert_eq!(b.n[centre], ITER_INTERIOR);
        let inside = b.n.iter().filter(|&&n| n == ITER_INTERIOR).count();
        assert!(inside > w * h / 20, "{inside}");
        let d = cpu_field((0.5, 0.5), w, h, MAX_ITERATIONS);
        assert!(d.n.iter().all(|&n| n != ITER_INTERIOR));
        // Symmetric under z -> -z (pixel (x, y) and (w-1-x, h-1-y)).
        for y in 0..h {
            for x in 0..w {
                assert_eq!(b.n[y * w + x], b.n[(h - 1 - y) * w + (w - 1 - x)]);
            }
        }
    }
}
