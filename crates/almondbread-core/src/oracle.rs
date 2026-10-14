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

use rayon::prelude::*;

use crate::cfmt::g;

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
/// resolves the pixel spacing (scales above about 1e-15). Rows run in parallel.
pub fn cpu_render(
    center_re: f64,
    center_im: f64,
    scale: f64,
    width: usize,
    height: usize,
    max_iter: usize,
    bailout: f64,
) -> IterationField {
    let mut n = vec![0f32; width * height];
    let mut s = vec![0f32; width * height];
    let b2 = bailout * bailout;
    let half_w = width as f64 / 2.0;
    let half_h = height as f64 / 2.0;
    n.par_chunks_mut(width.max(1))
        .zip(s.par_chunks_mut(width.max(1)))
        .enumerate()
        .for_each(|(y, (nrow, srow))| {
            // Same sample position as the kernel: texel centre, y flipped.
            let ci = center_im - (y as f64 + 0.5 - half_h) * scale;
            for x in 0..width {
                let cr = center_re + (x as f64 + 0.5 - half_w) * scale;
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
        let f = cpu_render(-0.75, 0.0, 3.0 / 64.0, 64, 40, 500, 65536.0);
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
}
