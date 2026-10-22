//! The CPU oracle and the GPU-vs-CPU verify metrics (kb: synthesis/benchmarks,
//! questions/float32-error-long-orbits).
//!
//! Iteration fields use the GPU's iteration-texture encoding, so GPU and CPU renders compare
//! value by value: `n` is the escape iteration (-1 interior or maxIter, -2 not computed,
//! -3 unresolved), `s` the smooth offset, smooth count `mu = n + s`
//! (kb: techniques/smooth-coloring). For interior pixels `s` is 1 when the GPU's derivative
//! test fired, 0 when the pixel ran to maxIter.

use std::fs;
use std::io;
use std::path::Path;

use almondbread_math::{DoubleExp, DoubleExpComplex, Formula, ReferenceOrbitComputer};
use rayon::prelude::*;

use crate::cfmt::g;
use crate::julia_reference::JuliaLayout;
use crate::viewport::{Center, Scale};
#[cfg(test)]
use almondbread_math::BigFloat;

/// Interior (or the limit without the interior test).
pub const ITER_INTERIOR: f32 = -1.0;
/// Admitted but still iterating.
pub const ITER_PENDING: f32 = -2.0;
/// Past the nominal limit, neither escaped nor proven interior.
pub const ITER_UNRESOLVED: f32 = -3.0;

/// Per-pixel iteration results, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct IterationField {
    pub width: usize,
    pub height: usize,
    pub n: Vec<f32>,
    pub s: Vec<f32>,
}

impl IterationField {
    /// From interleaved (n, s) texels, as read back from the rg32Float iteration texture.
    pub fn from_texels(width: usize, height: usize, texels: &[[f32; 2]]) -> Self {
        Self {
            width,
            height,
            n: texels.iter().map(|t| t[0]).collect(),
            s: texels.iter().map(|t| t[1]).collect(),
        }
    }

    /// Raw dump for before/after comparisons, the same format as the Swift tools: width,
    /// height (i32, little endian) then (n, s) f32 pairs.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 8 * self.n.len());
        out.extend_from_slice(&(self.width as i32).to_le_bytes());
        out.extend_from_slice(&(self.height as i32).to_le_bytes());
        for (n, s) in self.n.iter().zip(&self.s) {
            out.extend_from_slice(&n.to_le_bytes());
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    pub fn write(&self, path: impl AsRef<Path>) -> io::Result<()> {
        fs::write(path, self.to_bytes())
    }

    pub fn from_bytes(data: &[u8]) -> io::Result<Self> {
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "corrupt iteration field");
        if data.len() < 8 {
            return Err(bad());
        }
        let w = i32::from_le_bytes(data[0..4].try_into().unwrap());
        let h = i32::from_le_bytes(data[4..8].try_into().unwrap());
        let (w, h) = (
            usize::try_from(w).map_err(|_| bad())?,
            usize::try_from(h).map_err(|_| bad())?,
        );
        let floats: Vec<f32> = data[8..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        if floats.len() != 2 * w * h {
            return Err(bad());
        }
        Ok(Self {
            width: w,
            height: h,
            n: floats.iter().step_by(2).copied().collect(),
            s: floats.iter().skip(1).step_by(2).copied().collect(),
        })
    }

    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::from_bytes(&fs::read(path)?)
    }

    /// Texels that differ bit for bit in n or s.
    pub fn bitwise_differences(&self, other: &Self) -> usize {
        self.n
            .iter()
            .zip(&other.n)
            .zip(self.s.iter().zip(&other.s))
            .filter(|((a, b), (c, d))| a.to_bits() != b.to_bits() || c.to_bits() != d.to_bits())
            .count()
    }
}

/// Direct f64 iteration `z <- z^2 + c` from z = 0: no perturbation, no interior test, the
/// same pixel grid, bailout and smooth formula as the GPU kernel. Float32 errors in the GPU
/// path are silent, so this is the ground truth the verifier diffs against. Valid while f64
/// resolves the pixel spacing (scales above about 1e-15). Rows run in parallel. `angle`: the
/// view's rotation (radians, [`crate::viewport::Viewport::angle`]).
#[allow(clippy::too_many_arguments)]
pub fn cpu_render(
    center_re: f64,
    center_im: f64,
    scale: f64,
    width: usize,
    height: usize,
    max_iter: usize,
    bailout: f64,
    angle: f64,
) -> IterationField {
    cpu_render_for(
        Formula::Mandelbrot,
        center_re,
        center_im,
        scale,
        width,
        height,
        max_iter,
        bailout,
        angle,
    )
}

/// [`cpu_render`] for any formula (F2: `z <- f(z) + c`; Mandelbrot runs its own loop).
#[allow(clippy::too_many_arguments)]
pub fn cpu_render_for(
    formula: Formula,
    center_re: f64,
    center_im: f64,
    scale: f64,
    width: usize,
    height: usize,
    max_iter: usize,
    bailout: f64,
    angle: f64,
) -> IterationField {
    let (rs, rc) = if angle == 0.0 { (0.0, 1.0) } else { angle.sin_cos() };
    let mut n = vec![0f32; width * height];
    let mut s = vec![0f32; width * height];
    let b2 = bailout * bailout;
    let cycle = formula.julia_k().map(julia_cycle).unwrap_or_default();
    let half_w = width as f64 / 2.0;
    let half_h = height as f64 / 2.0;
    n.par_chunks_mut(width.max(1))
        .zip(s.par_chunks_mut(width.max(1)))
        .enumerate()
        .for_each(|(y, (nrow, srow))| {
            // Same sample position as the kernel: texel centre, y flipped, turned by the angle.
            let oy = -(y as f64 + 0.5 - half_h);
            for x in 0..width {
                let ox = x as f64 + 0.5 - half_w;
                let (cr, ci) = if angle == 0.0 {
                    (center_re + ox * scale, center_im + oy * scale)
                } else {
                    (
                        center_re + (ox * rc - oy * rs) * scale,
                        center_im + (ox * rs + oy * rc) * scale,
                    )
                };
                let (mut zr, mut zi, mut r2) = (0.0f64, 0.0f64, 0.0f64);
                let mut i = 0usize;
                if let Formula::Julia(k) = formula {
                    // F4: the pixel is z_0, k is fixed (tested before the first step too, as
                    // the kernels do); a pixel at k's attracting cycle is interior.
                    let (kr, ki) = k.approx();
                    (zr, zi) = (cr, ci);
                    r2 = zr * zr + zi * zi;
                    while i < max_iter && r2 <= b2 {
                        let t = zr * zr - zi * zi + kr;
                        zi = 2.0 * zr * zi + ki;
                        zr = t;
                        i += 1;
                        r2 = zr * zr + zi * zi;
                        if i & 15 == 0 && near_cycle(&cycle, (zr, zi)) {
                            break;
                        }
                    }
                } else if formula.is_mandelbrot() {
                    while i < max_iter {
                        let t = zr * zr - zi * zi + cr;
                        zi = 2.0 * zr * zi + ci;
                        zr = t;
                        i += 1;
                        r2 = zr * zr + zi * zi;
                        if r2 > b2 {
                            break;
                        }
                    }
                } else {
                    while i < max_iter {
                        (zr, zi) = formula.step((zr, zi), (cr, ci));
                        i += 1;
                        r2 = zr * zr + zi * zi;
                        if r2 > b2 {
                            break;
                        }
                    }
                }
                if r2 > b2 {
                    nrow[x] = i as f32;
                    srow[x] = if formula.is_mandelbrot() {
                        (1.0 - (0.5 * r2.ln()).log2()) as f32
                    } else {
                        formula.smooth_offset(r2) as f32
                    };
                } else {
                    nrow[x] = ITER_INTERIOR;
                    srow[x] = 0.0;
                }
            }
        });
    IterationField { width, height, n, s }
}

// ---------------------------------------------------------------------------------------------
// M3b: the oracle at any depth (kb: synthesis/benchmarks M3b, techniques/extended-range-floats).

fn pow2(k: i64) -> f64 {
    if k < -1022 {
        if k < -1074 { 0.0 } else { pow2(k + 64) * pow2(-64) }
    } else if k > 1023 {
        f64::INFINITY
    } else {
        f64::from_bits(((k + 1023) as u64) << 52)
    }
}

/// `a = f 2^k` with f in [0.5, 1) (a > 0 finite).
fn exponent_of(a: f64) -> i64 {
    if a == 0.0 {
        return 0; // the subnormal branch below would recurse forever
    }
    let bits = a.to_bits();
    let be = ((bits >> 52) & 0x7ff) as i64;
    if be == 0 {
        return exponent_of(a * 18446744073709551616.0) - 64;
    }
    be - 1022
}

fn dx(re: f64, im: f64, e: i64) -> DoubleExpComplex {
    DoubleExpComplex::new(
        DoubleExp::from_f64(re).times_power_of_two(e),
        DoubleExp::from_f64(im).times_power_of_two(e),
    )
}

fn dx_exponent(z: DoubleExpComplex) -> i64 {
    match (z.re.is_zero(), z.im.is_zero()) {
        (true, true) => i64::MIN / 4,
        (true, false) => z.im.exponent,
        (false, true) => z.re.exponent,
        (false, false) => z.re.exponent.max(z.im.exponent),
    }
}

/// `z * 2^-e` as f64 pair.
fn dx_scaled(z: DoubleExpComplex, e: i64) -> (f64, f64) {
    (
        z.re.times_power_of_two(-e).to_f64(),
        z.im.times_power_of_two(-e).to_f64(),
    )
}

/// Bits of the deep oracle's reference beyond what the view needs (f64 deltas carry 53 bits;
/// the GPU's references use 40).
const ORACLE_GUARD_BITS: usize = 64;

/// The deep oracle's reference: the view centre's orbit at high precision, as f64 pairs (the
/// hot path) and in extended range (where |Z| is tiny).
pub struct OracleReference {
    /// The iteration (F2).
    pub formula: Formula,
    /// F4: a Julia reference (the critical orbit, zeros, the centre orbit; as the GPU's,
    /// [`crate::julia_reference`]).
    pub julia: Option<JuliaLayout>,
    /// F4: the points of `k`'s attracting cycle (f64; empty without one): a pixel within
    /// [`JULIA_CYCLE_RADIUS`] of one is interior (it cannot leave the cycle's basin), so interior
    /// pixels need not run to the limit.
    pub cycle: Vec<(f64, f64)>,
    pub points: Vec<[f64; 2]>,
    pub extended: Vec<DoubleExpComplex>,
    pub precision_bits: usize,
    pub time: f64,
}

impl OracleReference {
    pub fn compute(center: &Center, texel: Scale, size: usize, max_iter: usize, bailout: f64) -> Self {
        Self::compute_for(Formula::Mandelbrot, center, texel, size, max_iter, bailout)
    }

    /// [`OracleReference::compute`] for any formula (F2).
    pub fn compute_for(
        formula: Formula,
        center: &Center,
        texel: Scale,
        size: usize,
        max_iter: usize,
        bailout: f64,
    ) -> Self {
        let t0 = std::time::Instant::now();
        let bits = crate::reference::needed_bits(center, texel, size, ORACLE_GUARD_BITS) + 64;
        if let Formula::Julia(k) = formula {
            let kv = k.value();
            let crit = ReferenceOrbitComputer::new(&kv, bits);
            let (ce, _) =
                almondbread_math::with_no_control(4096, |ctl| crit.compute_double_exp(max_iter, bailout, ctl));
            let comp = ReferenceOrbitComputer::new(&kv, bits).with_start(center.big());
            let (ze, _) =
                almondbread_math::with_no_control(4096, |ctl| comp.compute_double_exp(max_iter, bailout, ctl));
            let layout = JuliaLayout {
                crit_last: ce.len() - 1,
                center_start: crate::julia_reference::center_start(ce.len()),
                crit_escaped: false,
            };
            let extended = crate::julia_reference::assemble(&ce, &ze, DoubleExpComplex::ZERO);
            let points = extended.iter().map(|z| [z.re.to_f64(), z.im.to_f64()]).collect();
            return Self {
                formula,
                julia: Some(layout),
                cycle: julia_cycle(k),
                points,
                extended,
                precision_bits: bits,
                time: t0.elapsed().as_secs_f64(),
            };
        }
        let comp = ReferenceOrbitComputer::new(center.big(), bits).with_formula(formula);
        let (extended, _) =
            almondbread_math::with_no_control(4096, |ctl| comp.compute_double_exp(max_iter, bailout, ctl));
        let points = extended.iter().map(|z| [z.re.to_f64(), z.im.to_f64()]).collect();
        Self {
            formula,
            julia: None,
            cycle: Vec::new(),
            points,
            extended,
            precision_bits: bits,
            time: t0.elapsed().as_secs_f64(),
        }
    }
}

/// The CPU oracle at any depth: perturbation against the view centre's high-precision orbit
/// with f64 deltas (53 bits against the GPU's 24), Zhuoran rebasing, no BLA, no interior test.
/// Deltas below 2^-500 (where squared norms would underflow) are rescaled, `z = w 2^e`, with
/// full extended-range steps where |Z| < 2^-400 (KF's "rescaled double", mathr's "full
/// iterations"). Same pixel grid, bailout and smooth formula as the GPU kernels. Validated
/// against direct BigFloat iteration (tests). Rows run in parallel.
pub fn cpu_render_deep(
    center: &Center,
    texel: Scale,
    width: usize,
    height: usize,
    max_iter: usize,
    bailout: f64,
    angle: f64,
) -> (IterationField, OracleReference) {
    cpu_render_deep_for(
        Formula::Mandelbrot,
        center,
        texel,
        width,
        height,
        max_iter,
        bailout,
        angle,
    )
}

/// [`cpu_render_deep`] for any formula (F2): the generic perturbation step `z' = z S + c`
/// ([`Formula::delta_factor`]).
#[allow(clippy::too_many_arguments)]
pub fn cpu_render_deep_for(
    formula: Formula,
    center: &Center,
    texel: Scale,
    width: usize,
    height: usize,
    max_iter: usize,
    bailout: f64,
    angle: f64,
) -> (IterationField, OracleReference) {
    let reference = OracleReference::compute_for(formula, center, texel, width.max(height), max_iter, bailout);
    let field = cpu_render_deep_with(&reference, texel, width, height, max_iter, bailout, (0.0, 0.0), angle);
    (field, reference)
}

/// [`cpu_render_deep`] against a given reference (of its formula) whose point lies `offset`
/// texels from the view centre (offset = centre - reference point, in texels, plane axes); `angle` turns the
/// pixel grid as [`cpu_render`]'s.
#[allow(clippy::too_many_arguments)]
pub fn cpu_render_deep_with(
    reference: &OracleReference,
    texel: Scale,
    width: usize,
    height: usize,
    max_iter: usize,
    bailout: f64,
    offset: (f64, f64),
    angle: f64,
) -> IterationField {
    let (rs, rc) = if angle == 0.0 { (0.0, 1.0) } else { angle.sin_cos() };
    let mut n = vec![0f32; width * height];
    let mut s = vec![0f32; width * height];
    let half_w = width as f64 / 2.0;
    let half_h = height as f64 / 2.0;
    let (tm, te) = texel.parts();
    n.par_chunks_mut(width.max(1))
        .zip(s.par_chunks_mut(width.max(1)))
        .enumerate()
        .for_each(|(y, (nrow, srow))| {
            for x in 0..width {
                let (ox, oy) = (x as f64 + 0.5 - half_w, -(y as f64 + 0.5 - half_h));
                let (ox, oy) = if angle == 0.0 {
                    (ox, oy)
                } else {
                    (ox * rc - oy * rs, ox * rs + oy * rc)
                };
                let px = (ox + offset.0) * tm;
                let py = (oy + offset.1) * tm;
                let (it, sm) = if reference.julia.is_some() {
                    julia_deep_pixel(reference, dx(px, py, te), max_iter, bailout * bailout)
                } else {
                    deep_pixel(reference, (px, py), te, max_iter, bailout * bailout)
                };
                nrow[x] = it;
                srow[x] = sm;
            }
        });
    IterationField { width, height, n, s }
}

/// How close to a point of `k`'s attracting cycle a Julia oracle pixel must come to count as
/// interior: far inside the cycle's immediate basin for any attracting multiplier the cycle
/// finder accepts (inference), far above f64 rounding.
pub const JULIA_CYCLE_RADIUS: f64 = 1e-9;

/// The points of `k`'s attracting cycle (F4; empty without one).
fn julia_cycle(k: almondbread_math::JuliaK) -> Vec<(f64, f64)> {
    let kf = k.approx();
    crate::julia::attracting_cycle(kf)
        .map(|c| {
            let mut p = c.point;
            (0..c.period)
                .map(|_| {
                    let q = p;
                    p = (p.0 * p.0 - p.1 * p.1 + kf.0, 2.0 * p.0 * p.1 + kf.1);
                    q
                })
                .collect()
        })
        .unwrap_or_default()
}

fn near_cycle(cycle: &[(f64, f64)], z: (f64, f64)) -> bool {
    cycle
        .iter()
        .any(|c| (z.0 - c.0).abs() < JULIA_CYCLE_RADIUS && (z.1 - c.1).abs() < JULIA_CYCLE_RADIUS)
}

/// One pixel of a deep Julia view (F4): its start's delta `d0` to the centre orbit, in
/// extended-range f64 throughout (53-bit mantissas; slower than [`deep_pixel`]'s regimes, fine
/// for an oracle), no `+c`, Zhuoran's rebase onto the critical orbit (index 0 = 0) when
/// `|Z_m + z| < |z|` or an orbit ends (kb: plans/f4-deep-julia). Validated against direct
/// BigFloat iteration (tests).
fn julia_deep_pixel(r: &OracleReference, d0: DoubleExpComplex, max_iter: usize, b2: f64) -> (f32, f32) {
    let j = r.julia.expect("a Julia reference");
    let ext = &r.extended;
    let (mut m, mut last, mut n) = (j.center_start, ext.len() - 1, 0usize);
    let mut z = d0;
    loop {
        let mut zr = ext[m];
        let zf = zr + z;
        let r2 = zf.magnitude_squared().to_f64();
        if r2 > b2 {
            return (n as f32, (1.0 - (0.5 * r2.ln()).log2()) as f32);
        }
        if n & 15 == 0 && near_cycle(&r.cycle, (zf.re.to_f64(), zf.im.to_f64())) {
            return (ITER_INTERIOR, 0.0);
        }
        if m == last || (m != 0 && zf.magnitude_squared() < z.magnitude_squared()) {
            z = zf;
            zr = DoubleExpComplex::ZERO;
            m = 0;
            last = j.crit_last;
        }
        if n >= max_iter {
            return (ITER_INTERIOR, 0.0);
        }
        z = (zr.times_two() + z) * z;
        m += 1;
        n += 1;
    }
}

/// One pixel of [`cpu_render_deep`]: `c = cm 2^ce`. Returns the field's (n, s).
fn deep_pixel(r: &OracleReference, cm: (f64, f64), ce: i64, max_iter: usize, b2: f64) -> (f32, f32) {
    const P_MIN2: f64 = 9.332636185032189e-302; // 2^-1000: |z|^2 below -> rescaled
    const TO_P_EXP: i64 = -480; // |z| >= 2^-481 -> plain
    const SMALL_Z: f64 = 3.872591914849318e-121; // 2^-400
    let pts = &r.points;
    let ext = &r.extended;
    let last = pts.len() - 1;
    let formula = r.formula;
    let generic = !formula.is_mandelbrot();
    // F5: a non-conformal formula (Mandelbar) steps through its own delta hooks.
    let conformal = formula.is_conformal();
    let c64 = (cm.0 * pow2(ce), cm.1 * pow2(ce));
    let cx = dx(cm.0, cm.1, ce);
    let c_exp = dx_exponent(cx);
    let escaped = |n: usize, r2: f64| {
        if generic {
            (n as f32, formula.smooth_offset(r2) as f32)
        } else {
            (n as f32, (1.0 - (0.5 * r2.ln()).log2()) as f32)
        }
    };
    // F3 (kb: plans/f3-multibrot "As built"): for z^p the step's factor S grows like
    // |Z|^(p-1), and S z must stay far inside f64 for |z| >= 2^-500 (plain) and the rescaled
    // w: where |Z| < 2^(-500/(p-1)) (p = 8: 2^-71) the step runs in extended range, in either
    // regime. Mandelbrot keeps its thresholds (and Multibrot(2) gets the same ones).
    let small_z = if generic {
        SMALL_Z.max((-500.0 / f64::from(formula.power() - 1)).exp2())
    } else {
        SMALL_Z
    };
    // State: plain z (e == None) or z = w 2^e.
    let (mut z, mut w, mut e): ((f64, f64), (f64, f64), Option<i64>);
    if c_exp >= TO_P_EXP {
        z = c64;
        w = (0.0, 0.0);
        e = None;
    } else {
        z = (0.0, 0.0);
        w = cm;
        e = Some(ce);
    }
    let (mut m, mut n) = (1usize, 1usize);
    let rescale = |w: &mut (f64, f64), e: &mut i64| -> (f64, (f64, f64)) {
        if *e < c_exp - 900 {
            let f = pow2(*e - c_exp);
            *w = (w.0 * f, w.1 * f);
            *e = c_exp;
        }
        let f = pow2((ce - *e).min(1000));
        (pow2(*e), (cm.0 * f, cm.1 * f))
    };
    let (mut sc, mut dd) = (0.0, (0.0, 0.0));
    if let Some(ee) = e.as_mut() {
        (sc, dd) = rescale(&mut w, ee);
    }
    loop {
        match e {
            None => {
                // Plain f64 perturbation.
                loop {
                    let zm = pts[m];
                    let zf = (zm[0] + z.0, zm[1] + z.1);
                    let r2 = zf.0 * zf.0 + zf.1 * zf.1;
                    if r2 > b2 {
                        return escaped(n, r2);
                    }
                    let mut z2 = z.0 * z.0 + z.1 * z.1;
                    let mut zr = zm;
                    if r2 < z2 || m == last {
                        z = zf;
                        m = 0;
                        zr = [0.0, 0.0];
                        z2 = r2;
                    }
                    if z2 < P_MIN2 {
                        let a = z.0.abs().max(z.1.abs());
                        let k = if a > 0.0 { exponent_of(a) } else { c_exp };
                        let f = pow2(-k);
                        w = (z.0 * f, z.1 * f);
                        let mut ee = k;
                        (sc, dd) = rescale(&mut w, &mut ee);
                        e = Some(ee);
                        break;
                    }
                    if n >= max_iter {
                        return (ITER_INTERIOR, 0.0);
                    }
                    if generic && zr[0].abs().max(zr[1].abs()) < small_z {
                        // Z small (|Z + z| >= |z| after the rebase test, so S may be tiny):
                        // the step in extended range from the reference's extended value.
                        let zrx = if m == 0 { DoubleExpComplex::ZERO } else { ext[m] };
                        let zx0 = dx(z.0, z.1, 0);
                        let zx = if conformal {
                            zx0 * formula.delta_factor_x(zrx, zrx + zx0) + cx
                        } else {
                            formula.delta_step_x(zrx, zx0, cx)
                        };
                        m += 1;
                        n += 1;
                        let k = dx_exponent(zx);
                        if k >= TO_P_EXP {
                            z = dx_scaled(zx, 0);
                            continue;
                        }
                        let mut ee = if zx.re.is_zero() && zx.im.is_zero() { c_exp } else { k };
                        w = dx_scaled(zx, ee);
                        (sc, dd) = rescale(&mut w, &mut ee);
                        e = Some(ee);
                        break;
                    }
                    if !conformal {
                        z = formula.delta_step((zr[0], zr[1]), z, c64);
                        m += 1;
                        n += 1;
                        continue;
                    }
                    let (t0, t1) = if generic {
                        formula.delta_factor((zr[0], zr[1]), (zr[0] + z.0, zr[1] + z.1))
                    } else {
                        (2.0 * zr[0] + z.0, 2.0 * zr[1] + z.1)
                    };
                    z = (t0 * z.0 - t1 * z.1 + c64.0, t0 * z.1 + t1 * z.0 + c64.1);
                    m += 1;
                    n += 1;
                }
            }
            Some(mut ee) => {
                let zm = pts[m];
                if m == 0 || m == last || zm[0].abs().max(zm[1].abs()) < small_z {
                    // Full step in extended range.
                    let zx = dx(w.0, w.1, ee);
                    let zrx = ext[m];
                    let zf = zrx + zx;
                    if m == last {
                        let r2 = zf.magnitude_squared().to_f64();
                        if r2 > b2 {
                            return escaped(n, r2);
                        }
                    }
                    let (mut zx, mut zrx) = (zx, zrx);
                    if m != 0 && (m == last || zf.magnitude_squared() < zx.magnitude_squared()) {
                        zx = zf;
                        zrx = DoubleExpComplex::ZERO;
                        m = 0;
                    }
                    let k = dx_exponent(zx);
                    if k >= TO_P_EXP {
                        z = dx_scaled(zx, 0);
                        e = None;
                        continue;
                    }
                    if n >= max_iter {
                        return (ITER_INTERIOR, 0.0);
                    }
                    zx = if !conformal {
                        formula.delta_step_x(zrx, zx, cx)
                    } else if generic {
                        zx * formula.delta_factor_x(zrx, zrx + zx) + cx
                    } else {
                        (zrx.times_two() + zx) * zx + cx
                    };
                    m += 1;
                    n += 1;
                    let k = dx_exponent(zx);
                    if k >= TO_P_EXP {
                        z = dx_scaled(zx, 0);
                        e = None;
                        continue;
                    }
                    ee = if zx.re.is_zero() && zx.im.is_zero() { c_exp } else { k };
                    w = dx_scaled(zx, ee);
                    (sc, dd) = rescale(&mut w, &mut ee);
                    e = Some(ee);
                    continue;
                }
                if n >= max_iter {
                    return (ITER_INTERIOR, 0.0);
                }
                let sw = (sc * w.0, sc * w.1);
                w = if conformal {
                    let (t0, t1) = if generic {
                        formula.delta_factor((zm[0], zm[1]), (zm[0] + sw.0, zm[1] + sw.1))
                    } else {
                        (2.0 * zm[0] + sw.0, 2.0 * zm[1] + sw.1)
                    };
                    (t0 * w.0 - t1 * w.1 + dd.0, t0 * w.1 + t1 * w.0 + dd.1)
                } else {
                    // (F6: the Burning Ship's fold compares X Y / s with the scaled perturbation;
                    // X Y from the extended reference, exact where a part is below f64's range.)
                    let xy_s = if formula.folds() {
                        (ext[m].re * ext[m].im).times_power_of_two(-ee).to_f64()
                    } else {
                        0.0
                    };
                    formula.delta_step_scaled((zm[0], zm[1]), sw, w, dd, xy_s)
                };
                m += 1;
                n += 1;
                let a = w.0.abs().max(w.1.abs());
                if (a > 1.8446744073709552e19 || a < 5.421010862427522e-20) && a > 0.0 {
                    let k = exponent_of(a);
                    let f = pow2(-k);
                    w = (w.0 * f, w.1 * f);
                    ee += k;
                    if ee >= TO_P_EXP {
                        let f = pow2(ee);
                        z = (w.0 * f, w.1 * f);
                        e = None;
                        continue;
                    }
                    (sc, dd) = rescale(&mut w, &mut ee);
                    e = Some(ee);
                }
            }
        }
    }
}

/// GPU-vs-reference diff metrics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VerifyReport {
    pub pixels: usize,
    /// Both exterior, escape iteration n differs.
    pub escape_differs: usize,
    /// GPU says interior, CPU escapes. Unresolved GPU pixels are not counted.
    pub false_interior: usize,
    /// GPU unresolved where the CPU escapes.
    pub unresolved_exterior: usize,
    /// GPU unresolved pixels.
    pub unresolved: usize,
    /// GPU escapes, CPU says interior.
    pub false_exterior: usize,
    /// GPU interior pixels found by the derivative test (s == 1) rather than maxIter.
    pub detected_interior: usize,
    pub cpu_interior: usize,
    /// Smooth-count error |Δmu| over pixels exterior in both.
    pub max_error: f64,
    pub median_error: f64,
    pub p99_error: f64,
    /// Pixels (both exterior) with |Δmu| > 0.05.
    pub over005: usize,
}

impl VerifyReport {
    pub fn pct(&self, k: usize) -> f64 {
        if self.pixels > 0 {
            100.0 * k as f64 / self.pixels as f64
        } else {
            0.0
        }
    }
    pub fn escape_differs_pct(&self) -> f64 {
        self.pct(self.escape_differs)
    }
    pub fn mismatch_pct(&self) -> f64 {
        self.pct(self.false_interior + self.false_exterior)
    }
    pub fn false_interior_pct(&self) -> f64 {
        self.pct(self.false_interior)
    }
    pub fn false_exterior_pct(&self) -> f64 {
        self.pct(self.false_exterior)
    }
    pub fn unresolved_pct(&self) -> f64 {
        self.pct(self.unresolved)
    }
    pub fn over005_pct(&self) -> f64 {
        self.pct(self.over005)
    }

    /// One line, the same text as the Swift `VerifyReport.summary`.
    pub fn summary(&self) -> String {
        format!(
            "pixels {}  escape-n differs {:.3}%  class mismatch {:.3}% (GPU-int/CPU-ext {:.3}%, GPU-ext/CPU-int {:.3}%)  \
             |Δmu| max {} median {} p99 {}  >0.05: {:.3}%  interior CPU {:.1}%, GPU detected {:.1}%  \
             unresolved {:.3}% (CPU escapes {:.3}%)",
            self.pixels,
            self.escape_differs_pct(),
            self.mismatch_pct(),
            self.false_interior_pct(),
            self.false_exterior_pct(),
            g(self.max_error, 3),
            g(self.median_error, 3),
            g(self.p99_error, 3),
            self.over005_pct(),
            self.pct(self.cpu_interior),
            self.pct(self.detected_interior),
            self.unresolved_pct(),
            self.pct(self.unresolved_exterior)
        )
    }
}

/// Compares `gpu` against the reference field `cpu` and renders a heatmap (BGRA8, sRGB):
/// exterior pixels in a log ramp of |Δmu| from 1e-4 (black) through red to yellow-white at
/// 1 and above, false interior magenta, false exterior cyan, agreed interior dark grey; GPU
/// unresolved blue (CPU escapes) or dark blue (CPU interior).
pub fn compare(gpu: &IterationField, cpu: &IterationField) -> (VerifyReport, Vec<u8>) {
    assert!(gpu.width == cpu.width && gpu.height == cpu.height, "field sizes differ");
    let count = gpu.width * gpu.height;
    let mut r = VerifyReport {
        pixels: count,
        ..Default::default()
    };
    let mut errors: Vec<f64> = Vec::with_capacity(count);
    let mut img = vec![0u8; count * 4];
    let put = |img: &mut [u8], k: usize, red: f64, green: f64, blue: f64| {
        let b = |v: f64| (v * 255.0).clamp(0.0, 255.0) as u8;
        img[4 * k] = b(blue);
        img[4 * k + 1] = b(green);
        img[4 * k + 2] = b(red);
        img[4 * k + 3] = 255;
    };
    for k in 0..count {
        let g_in = gpu.n[k] < 0.0;
        let c_in = cpu.n[k] < 0.0;
        if c_in {
            r.cpu_interior += 1;
        }
        if cpu.n[k] == ITER_UNRESOLVED {
            // The reference field is a GPU render too (fields diffs, fresh renders): its
            // unresolved pixels are unknown, not a class.
            put(&mut img, k, 0.05, 0.05, 0.35);
            continue;
        }
        if gpu.n[k] == ITER_UNRESOLVED {
            r.unresolved += 1;
            if !c_in {
                r.unresolved_exterior += 1;
                put(&mut img, k, 0.2, 0.3, 1.0);
            } else {
                put(&mut img, k, 0.05, 0.05, 0.35);
            }
            continue;
        }
        if g_in && gpu.s[k] == 1.0 {
            r.detected_interior += 1;
        }
        match (g_in, c_in) {
            (true, true) => put(&mut img, k, 0.12, 0.12, 0.12),
            (true, false) => {
                r.false_interior += 1;
                put(&mut img, k, 1.0, 0.0, 1.0);
            }
            (false, true) => {
                r.false_exterior += 1;
                put(&mut img, k, 0.0, 1.0, 1.0);
            }
            (false, false) => {
                if gpu.n[k] != cpu.n[k] {
                    r.escape_differs += 1;
                }
                let e = ((gpu.n[k] as f64 + gpu.s[k] as f64) - (cpu.n[k] as f64 + cpu.s[k] as f64)).abs();
                errors.push(e);
                if e > 0.05 {
                    r.over005 += 1;
                }
                let t = ((e.max(1e-6).log10() + 4.0) / 4.0).clamp(0.0, 1.0);
                put(
                    &mut img,
                    k,
                    (2.0 * t).min(1.0),
                    (2.0 * t - 1.0).max(0.0),
                    (3.0 * t - 2.0).max(0.0),
                );
            }
        }
    }
    if !errors.is_empty() {
        errors.sort_by(f64::total_cmp);
        r.max_error = *errors.last().unwrap();
        r.median_error = errors[errors.len() / 2];
        r.p99_error = errors[(errors.len() - 1).min(errors.len() * 99 / 100)];
    }
    (r, img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_home_and_roundtrip() {
        let f = cpu_render(-0.75, 0.0, 3.0 / 64.0, 64, 40, 500, 65536.0, 0.0);
        // the centre row crosses the main cardioid: interior there
        assert!(f.n[20 * 64 + 32] < 0.0);
        // the corner escapes at once
        assert!(f.n[0] >= 1.0);
        let back = IterationField::from_bytes(&f.to_bytes()).unwrap();
        assert_eq!(back, f);
        let (r, img) = compare(&f, &f);
        assert_eq!(r.false_interior + r.false_exterior + r.escape_differs, 0);
        assert_eq!(r.max_error, 0.0);
        assert_eq!(img.len(), 64 * 40 * 4);
        assert_eq!(f.bitwise_differences(&back), 0);
    }

    /// The deep oracle against direct BigFloat iteration of individual pixels (no
    /// perturbation at all) at 1e-50, 1e-300 and 1e-1000 per pixel around c = i (a Misiurewicz
    /// point: structure at every depth) and at a period-79 needle minibrot at 1e-94.
    #[test]
    fn deep_oracle_matches_direct_iteration() {
        use almondbread_math::{BigComplex, ReferenceOrbitComputer, with_no_control};
        let cases = [
            ("0", "1", "1e-50", 2000usize),
            ("0", "1", "1e-300", 4000),
            ("0", "1", "1e-1000", 8000),
            (
                "-1.99999999999999999999999999999999999999999999898704139472119329940998074630723121589755867176089901",
                "0",
                "3e-94",
                3000,
            ),
        ];
        for (re, im, sc, it) in cases {
            let center = Center::parse(re, im).unwrap();
            let texel = Scale::parse(sc).unwrap();
            let (w, h) = (24usize, 16usize);
            let (field, _) = cpu_render_deep(&center, texel, w, h, it, 65536.0, 0.0);
            let bits = crate::reference::needed_bits(&center, texel, w, 64) + 64;
            let (tm, te) = texel.parts();
            let (mut same, mut worst) = (0usize, 0.0f64);
            for y in 0..h {
                for x in 0..w {
                    let k = y * w + x;
                    let dx = (x as f64 + 0.5 - w as f64 / 2.0) * tm;
                    let dy = -(y as f64 + 0.5 - h as f64 / 2.0) * tm;
                    let c = BigComplex::new(
                        center.big().re.with_precision(bits) + BigFloat::from_f64(dx, 64).times_power_of_two(te),
                        center.big().im.with_precision(bits) + BigFloat::from_f64(dy, 64).times_power_of_two(te),
                    );
                    let comp = ReferenceOrbitComputer::new(&c, bits);
                    let r = with_no_control(4096, |ctl| comp.compute(it, 65536.0, None, None, ctl));
                    let (n, mu) = if r.escaped {
                        let r2 = r.final_z.magnitude_squared().to_f64();
                        (
                            r.iterations() as f32,
                            r.iterations() as f64 + 1.0 - (0.5 * r2.ln()).log2(),
                        )
                    } else {
                        (ITER_INTERIOR, 0.0)
                    };
                    if n == field.n[k] {
                        same += 1;
                        if n >= 0.0 {
                            worst = worst.max((mu - (field.n[k] as f64 + field.s[k] as f64)).abs());
                        }
                    }
                }
            }
            println!(
                "deep oracle {re},{im} @ {sc}: {same}/{} same n, max |dmu| {worst:.2e}",
                w * h
            );
            assert!(same * 100 >= w * h * 98, "{sc}: only {same} of {} agree", w * h);
            assert!(worst < 1e-4, "{sc}: |dmu| {worst}");
        }
    }

    /// F4: the deep Julia oracle (f64 perturbation, critical-orbit rebasing) against direct
    /// BigFloat iteration of each pixel's start (no perturbation at all), at two of the step-1
    /// gate's views (kb: plans/f4-deep-julia), whole at 24 x 16: a preimage of 0 at 1e-100
    /// (period-3 bulb) and at 1e-300 (Misiurewicz i).
    #[test]
    fn deep_julia_oracle_matches_direct_iteration() {
        use almondbread_math::{BigComplex, JuliaK, ReferenceOrbitComputer, with_no_control};
        let cases = [
            (
                ("-0.1225611668766536", "0.7448617666197442"),
                "0.13582194234832306384726920844974750963803955544158433695104985552061703718356220531423914345539064577159014155396788582297739",
                "-0.98408937840354738561303053036751272330190031920474351403645897719626417827245062205230473648568576800988084871583518785926966",
                "8.3e-100",
                3670usize,
            ),
            (
                ("0", "1"),
                "-1.316217188613384182892301938270293124103893412616323208720307172274596124394581156543859132946211679304026231908661961024336819202086203360168036350362781617862595986112142227701026008880468481898614759175229768762325953986190536194031254729435187793227165530004224246648352068591889334276616386741309913104214101345554899367",
                "0.6785478674150520237952461553430622991323380058277815194421221054868252650933002485440546541350165849932149350420951796822304685241394963385899335024961001636274825482729138595485370773903166706946136063947425419828687390342905631095799823421179334798563580045190217945069289191899700554854070505266217696214286174348262202054",
                "8.3e-300",
                6850,
            ),
        ];
        for ((kr, ki), re, im, sc, it) in cases {
            let k = Center::parse(kr, ki).unwrap();
            let formula = Formula::Julia(JuliaK::new(k.big()));
            let center = Center::parse(re, im).unwrap();
            let texel = Scale::parse(sc).unwrap();
            let (w, h) = (24usize, 16usize);
            let (field, _) = cpu_render_deep_for(formula, &center, texel, w, h, it, 65536.0, 0.0);
            let bits = crate::reference::needed_bits(&center, texel, w, 64) + 64;
            let kb = k.big().with_precision(bits);
            let (tm, te) = texel.parts();
            let (mut same, mut worst, mut levels) = (0usize, 0.0f64, std::collections::BTreeSet::new());
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    let dx = (x as f64 + 0.5 - w as f64 / 2.0) * tm;
                    let dy = -(y as f64 + 0.5 - h as f64 / 2.0) * tm;
                    let z0 = BigComplex::new(
                        center.big().re.with_precision(bits) + BigFloat::from_f64(dx, 64).times_power_of_two(te),
                        center.big().im.with_precision(bits) + BigFloat::from_f64(dy, 64).times_power_of_two(te),
                    );
                    let comp = ReferenceOrbitComputer::new(&kb, bits).with_start(&z0);
                    let r = with_no_control(4096, |ctl| comp.compute(it, 65536.0, None, None, ctl));
                    let (n, mu) = if r.escaped {
                        let r2 = r.final_z.magnitude_squared().to_f64();
                        (
                            r.iterations() as f32,
                            r.iterations() as f64 + 1.0 - (0.5 * r2.ln()).log2(),
                        )
                    } else {
                        (ITER_INTERIOR, 0.0)
                    };
                    levels.insert(n as i64);
                    if n == field.n[i] {
                        same += 1;
                        if n >= 0.0 {
                            worst = worst.max((mu - (field.n[i] as f64 + field.s[i] as f64)).abs());
                        }
                    }
                }
            }
            println!(
                "deep Julia oracle k = {kr},{ki} @ {sc}: {same}/{} same n, max |dmu| {worst:.2e}",
                w * h
            );
            assert!(levels.len() > 10, "{sc}: a picture, not a flat view ({levels:?})");
            assert!(same * 100 >= w * h * 98, "{sc}: only {same} of {} agree", w * h);
            assert!(worst < 1e-4, "{sc}: |dmu| {worst}");
        }
    }

    const SQRT3: &str = "1.732050807568877293527446341505872366942805253810380628055806979451933016908800037081146186757248575675626141415406703029969945094998952478811655512094373648528093231902305582067974820101084674923265015312343266903322886650672254668921837971227047131660367861588019049986537379859389467650347506576050756618348129606100947602187190325083145829523959832997789824508288714463832917347224163984587855397667958063";

    /// F5: the Mandelbar deep oracle (its conjugated perturbation step, rescaled regimes and
    /// extended-range steps) against direct BigFloat iteration of `conj(z)^2 + c` per pixel, at
    /// a Misiurewicz point off the real axis (1e-30, 1e-100, 1e-320 per pixel) and on it.
    #[test]
    fn mandelbar_oracle_matches_direct_iteration() {
        use almondbread_math::{BigComplex, ReferenceOrbitComputer, with_no_control};
        let f = Formula::Mandelbar;
        // The antenna tip 1 + i sqrt(3) = -2 e^{2 pi i / 3}: the image of the tip -2 under the
        // Tricorn's three-fold symmetry (c -> e^{2 pi i/3} c), a Misiurewicz point off the axis.
        let tip = Center::parse("1", SQRT3).unwrap();
        let mut cases = Vec::new();
        for (sc, it) in [("1e-30", 3000usize), ("1e-100", 4000), ("1e-320", 5000)] {
            cases.push((tip.clone(), sc, it));
        }
        // A real minibrot (period 3; real orbits are Mandelbrot's) with its off-axis pixels.
        cases.push((Center::parse("-1.7548776662466927600495", "0").unwrap(), "2e-3", 3000));
        for (center, sc, it) in cases {
            let texel = Scale::parse(sc).unwrap();
            let (w, h) = (24usize, 16usize);
            let (field, r) = cpu_render_deep_for(f, &center, texel, w, h, it, 65536.0, 0.0);
            assert_eq!(r.formula, f);
            let bits = crate::reference::needed_bits(&center, texel, w, 64) + 64;
            let (tm, te) = texel.parts();
            let (mut same, mut worst) = (0usize, 0.0f64);
            let mut levels = std::collections::BTreeSet::new();
            for y in 0..h {
                for x in 0..w {
                    let k = y * w + x;
                    let dx = (x as f64 + 0.5 - w as f64 / 2.0) * tm;
                    let dy = -(y as f64 + 0.5 - h as f64 / 2.0) * tm;
                    let c = BigComplex::new(
                        center.big().re.with_precision(bits) + BigFloat::from_f64(dx, 64).times_power_of_two(te),
                        center.big().im.with_precision(bits) + BigFloat::from_f64(dy, 64).times_power_of_two(te),
                    );
                    let comp = ReferenceOrbitComputer::new(&c, bits).with_formula(f);
                    let o = with_no_control(4096, |ctl| comp.compute(it, 65536.0, None, None, ctl));
                    let (n, mu) = if o.escaped {
                        let r2 = o.final_z.magnitude_squared().to_f64();
                        (o.iterations() as f32, o.iterations() as f64 + f.smooth_offset(r2))
                    } else {
                        (ITER_INTERIOR, 0.0)
                    };
                    levels.insert(field.n[k] as i64);
                    if n == field.n[k] {
                        same += 1;
                        if n >= 0.0 {
                            worst = worst.max((mu - (field.n[k] as f64 + field.s[k] as f64)).abs());
                        }
                    }
                }
            }
            println!(
                "mandelbar oracle @ {sc}: {same}/{} same n, max |dmu| {worst:.2e}, {} levels",
                w * h,
                levels.len()
            );
            assert!(levels.len() > 5, "{sc}: a picture, not a flat view ({levels:?})");
            assert!(same * 100 >= w * h * 98, "{sc}: only {same} of {} agree", w * h);
            assert!(worst < 1e-4, "{sc}: |dmu| {worst}");
        }
        // The shallow oracle steps conj(z)^2 + c too: symmetric about the real axis.
        let a = cpu_render_for(f, -0.3, 0.0, 3.0 / 80.0, 80, 50, 500, 65536.0, 0.0);
        for y in 0..25 {
            for x in 0..80 {
                assert_eq!(a.n[y * 80 + x], a.n[(49 - y) * 80 + x]);
            }
        }
    }

    /// F6: the Burning Ship deep oracle (diffabs in the plain, rescaled and extended-range
    /// steps, the fold from the extended reference's `X Y`) against direct BigFloat iteration of
    /// `(|x| - i|y|)^2 + c` per pixel: on the needle (the real axis, where every step folds:
    /// 1e-30, 1e-100, 1e-320 per pixel), beside it (the axis crossing the view at 1e-100) and at
    /// shallow views of the ship.
    #[test]
    fn burning_ship_oracle_matches_direct_iteration() {
        use almondbread_math::{BigComplex, ReferenceOrbitComputer, with_no_control};
        let f = Formula::BurningShip;
        let mut cases = Vec::new();
        for (sc, it) in [("1e-30", 3000usize), ("1e-100", 4000), ("1e-320", 6000)] {
            cases.push((Center::parse("-1.9", "0").unwrap(), sc, it));
        }
        cases.push((Center::parse("-1.9", "3e-101").unwrap(), "1e-100", 4000));
        // (Shallow views of the ship are chaotic: bounded chaotic orbits and long transients
        // before escape, so 5-20 % of their pixels differ between direct f64 and BigFloat
        // iteration. There the oracle must agree with BigFloat at least as often as direct f64
        // does; on the needle, where nothing is chaotic at this resolution, on 98 %.)
        cases.push((Center::parse("-1.7616", "0.0284").unwrap(), "2e-3", 3000));
        cases.push((Center::parse("-0.6", "0.7").unwrap(), "3e-3", 3000));
        for (center, sc, it) in cases {
            let texel = Scale::parse(sc).unwrap();
            let (w, h) = (24usize, 16usize);
            let (field, r) = cpu_render_deep_for(f, &center, texel, w, h, it, 65536.0, 0.0);
            assert_eq!(r.formula, f);
            let bits = crate::reference::needed_bits(&center, texel, w, 64) + 64;
            let (tm, te) = texel.parts();
            let (mut same, mut worst, mut f64_same) = (0usize, 0.0f64, 0usize);
            let mut levels = std::collections::BTreeSet::new();
            for y in 0..h {
                for x in 0..w {
                    let k = y * w + x;
                    let dx = (x as f64 + 0.5 - w as f64 / 2.0) * tm;
                    let dy = -(y as f64 + 0.5 - h as f64 / 2.0) * tm;
                    let c = BigComplex::new(
                        center.big().re.with_precision(bits) + BigFloat::from_f64(dx, 64).times_power_of_two(te),
                        center.big().im.with_precision(bits) + BigFloat::from_f64(dy, 64).times_power_of_two(te),
                    );
                    let comp = ReferenceOrbitComputer::new(&c, bits).with_formula(f);
                    let o = with_no_control(4096, |ctl| comp.compute(it, 65536.0, None, None, ctl));
                    let (n, mu) = if o.escaped {
                        let r2 = o.final_z.magnitude_squared().to_f64();
                        (o.iterations() as f32, o.iterations() as f64 + f.smooth_offset(r2))
                    } else {
                        (ITER_INTERIOR, 0.0)
                    };
                    levels.insert(field.n[k] as i64);
                    // Direct f64 iteration of the same c, the chaos control.
                    let [cr, ci] = c.to_f64();
                    let (mut z, mut i) = ((0.0f64, 0.0f64), 0usize);
                    while i < it && z.0 * z.0 + z.1 * z.1 <= 65536.0f64 * 65536.0 {
                        z = f.step(z, (cr, ci));
                        i += 1;
                    }
                    let stable = (if i < it { i as f32 } else { ITER_INTERIOR }) == n;
                    if stable {
                        f64_same += 1;
                    }
                    if n == field.n[k] {
                        same += 1;
                        // (|dmu| where direct f64 agrees too, or on the needle where f64 cannot
                        // resolve the view at all.)
                        if n >= 0.0 && (stable || sc != "2e-3" && sc != "3e-3") {
                            worst = worst.max((mu - (field.n[k] as f64 + field.s[k] as f64)).abs());
                        }
                    }
                }
            }
            println!(
                "burning ship oracle @ {} {} {sc}: {same}/{} same n (direct f64 {f64_same}), max |dmu| {worst:.2e}, {} levels",
                center.re(),
                center.im(),
                w * h,
                levels.len()
            );
            assert!(levels.len() > 5, "{sc}: a picture, not a flat view ({levels:?})");
            assert!(
                same * 100 >= w * h * 98 || same + w * h / 100 >= f64_same,
                "{sc}: only {same} of {} agree (direct f64 {f64_same})",
                w * h
            );
            // (Chaotic views: n agrees but the final |z| of a sensitive orbit carries more than
            // rounding.)
            let bound = if sc == "2e-3" || sc == "3e-3" { 2e-2 } else { 1e-4 };
            assert!(worst < bound, "{sc}: |dmu| {worst}");
        }
    }

    /// F2: the generic path at p = 2 is the Mandelbrot oracle (shallow and deep: escape counts
    /// agree but for chaotic pixels, |dmu| at rounding level elsewhere); p = 3 deep matches its
    /// own direct iteration.
    #[test]
    fn generic_oracle_is_mandelbrot() {
        let g = Formula::Multibrot(2);
        let a = cpu_render(-0.75, 0.1, 3.0 / 160.0, 160, 100, 2000, 65536.0, 0.3);
        let b = cpu_render_for(g, -0.75, 0.1, 3.0 / 160.0, 160, 100, 2000, 65536.0, 0.3);
        let (r, _) = compare(&b, &a);
        assert_eq!(r.false_interior + r.false_exterior, 0);
        assert!(r.escape_differs_pct() < 0.5 && r.median_error < 1e-9, "{}", r.summary());
        for (re, im, sc, it) in [
            ("0", "1", "1e-50", 2000usize),
            (
                "-0.743643887037158704752191506",
                "0.131825904205311970493132",
                "1e-25",
                6000,
            ),
        ] {
            let center = Center::parse(re, im).unwrap();
            let texel = Scale::parse(sc).unwrap();
            let (a, _) = cpu_render_deep(&center, texel, 48, 32, it, 65536.0, 0.0);
            let (b, rb) = cpu_render_deep_for(g, &center, texel, 48, 32, it, 65536.0, 0.0);
            assert_eq!(rb.formula, g);
            let (r, _) = compare(&b, &a);
            assert_eq!(r.false_interior + r.false_exterior, 0, "{sc}");
            assert!(
                r.escape_differs_pct() < 2.0 && r.median_error < 1e-6,
                "{sc}: {}",
                r.summary()
            );
        }
        // z^3 + c at 1e-30 per texel against the direct shallow oracle's arithmetic in BigFloat.
        use almondbread_math::{BigComplex, ReferenceOrbitComputer, with_no_control};
        let cubic = Formula::Multibrot(3);
        let center = Center::parse("-0.4152", "0.6040").unwrap();
        let texel = Scale::parse("1e-30").unwrap();
        let (w, h) = (12usize, 8usize);
        let (field, _) = cpu_render_deep_for(cubic, &center, texel, w, h, 3000, 65536.0, 0.0);
        let bits = crate::reference::needed_bits(&center, texel, w, 64) + 64;
        let (tm, te) = texel.parts();
        let mut same = 0;
        for y in 0..h {
            for x in 0..w {
                let dx = (x as f64 + 0.5 - w as f64 / 2.0) * tm;
                let dy = -(y as f64 + 0.5 - h as f64 / 2.0) * tm;
                let c = BigComplex::new(
                    center.big().re.with_precision(bits) + BigFloat::from_f64(dx, 64).times_power_of_two(te),
                    center.big().im.with_precision(bits) + BigFloat::from_f64(dy, 64).times_power_of_two(te),
                );
                let comp = ReferenceOrbitComputer::new(&c, bits).with_formula(cubic);
                let r = with_no_control(4096, |ctl| comp.compute(3000, 65536.0, None, None, ctl));
                let n = if r.escaped {
                    r.iterations() as f32
                } else {
                    ITER_INTERIOR
                };
                same += usize::from(n == field.n[y * w + x]);
            }
        }
        assert!(same * 100 >= w * h * 95, "cubic: {same} of {}", w * h);
    }
}
