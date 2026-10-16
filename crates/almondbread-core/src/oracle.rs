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

use almondbread_math::{DoubleExp, DoubleExpComplex, ReferenceOrbitComputer};
use rayon::prelude::*;

use crate::cfmt::g;
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
    let (rs, rc) = if angle == 0.0 { (0.0, 1.0) } else { angle.sin_cos() };
    let mut n = vec![0f32; width * height];
    let mut s = vec![0f32; width * height];
    let b2 = bailout * bailout;
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
                if r2 > b2 {
                    nrow[x] = i as f32;
                    srow[x] = (1.0 - (0.5 * r2.ln()).log2()) as f32;
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
    pub points: Vec<[f64; 2]>,
    pub extended: Vec<DoubleExpComplex>,
    pub precision_bits: usize,
    pub time: f64,
}

impl OracleReference {
    pub fn compute(center: &Center, texel: Scale, size: usize, max_iter: usize, bailout: f64) -> Self {
        let t0 = std::time::Instant::now();
        let bits = crate::reference::needed_bits(center, texel, size, ORACLE_GUARD_BITS) + 64;
        let comp = ReferenceOrbitComputer::new(center.big(), bits);
        let (extended, _) =
            almondbread_math::with_no_control(4096, |ctl| comp.compute_double_exp(max_iter, bailout, ctl));
        let points = extended.iter().map(|z| [z.re.to_f64(), z.im.to_f64()]).collect();
        Self {
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
    let reference = OracleReference::compute(center, texel, width.max(height), max_iter, bailout);
    let field = cpu_render_deep_with(&reference, texel, width, height, max_iter, bailout, (0.0, 0.0), angle);
    (field, reference)
}

/// [`cpu_render_deep`] against a given reference whose point lies `offset` texels from the
/// view centre (offset = centre - reference point, in texels, plane axes); `angle` turns the
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
                let (it, sm) = deep_pixel(reference, (px, py), te, max_iter, bailout * bailout);
                nrow[x] = it;
                srow[x] = sm;
            }
        });
    IterationField { width, height, n, s }
}

/// One pixel of [`cpu_render_deep`]: `c = cm 2^ce`. Returns the field's (n, s).
fn deep_pixel(r: &OracleReference, cm: (f64, f64), ce: i64, max_iter: usize, b2: f64) -> (f32, f32) {
    const P_MIN2: f64 = 9.332636185032189e-302; // 2^-1000: |z|^2 below -> rescaled
    const TO_P_EXP: i64 = -480; // |z| >= 2^-481 -> plain
    const SMALL_Z: f64 = 3.872591914849318e-121; // 2^-400
    let pts = &r.points;
    let ext = &r.extended;
    let last = pts.len() - 1;
    let c64 = (cm.0 * pow2(ce), cm.1 * pow2(ce));
    let cx = dx(cm.0, cm.1, ce);
    let c_exp = dx_exponent(cx);
    let escaped = |n: usize, r2: f64| (n as f32, (1.0 - (0.5 * r2.ln()).log2()) as f32);
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
                    let t0 = 2.0 * zr[0] + z.0;
                    let t1 = 2.0 * zr[1] + z.1;
                    z = (t0 * z.0 - t1 * z.1 + c64.0, t0 * z.1 + t1 * z.0 + c64.1);
                    m += 1;
                    n += 1;
                }
            }
            Some(mut ee) => {
                let zm = pts[m];
                if m == 0 || m == last || zm[0].abs().max(zm[1].abs()) < SMALL_Z {
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
                    zx = (zrx.times_two() + zx) * zx + cx;
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
                let t0 = 2.0 * zm[0] + sw.0;
                let t1 = 2.0 * zm[1] + sw.1;
                w = (t0 * w.0 - t1 * w.1 + dd.0, t0 * w.1 + t1 * w.0 + dd.1);
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
}
