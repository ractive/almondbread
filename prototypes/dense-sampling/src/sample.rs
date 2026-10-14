//! One sample: direct f64 iteration from z = 0 with the GPU kernel's outputs (escape n, smooth
//! offset, DE in output pixels, normal angle) and the GPU's derivative interior test
//! (kb: techniques/interior-detection). Plus what the study needs: the first iteration at which
//! the pixel footprint's image `|dz/dc| * pixel` reaches each threshold of [`EXPAND`].
//!
//! Direct f64 iteration is valid while f64 resolves the sample spacing (scales above ~1e-15 per
//! sample; all study locations are above). It is not the GPU's float32 perturbation, but every
//! strategy here is measured against the same function, so differences are the strategy's.

pub const BAILOUT2: f64 = 65536.0 * 65536.0;
pub const INTERIOR_EPS: f64 = 1e-3;

/// Footprint thresholds T: `nexp[k]` is the first n with `|dz_n/dc| * pixel >= EXPAND[k]`.
pub const EXPAND: [f64; 4] = [1e-3, 1e-2, 1e-1, 1.0];
pub const NEVER: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Class {
    Exterior = 0,
    Interior = 1,
    Unresolved = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    /// Escape iteration (exterior), or iterations spent (interior: until the test fired;
    /// unresolved: maxIter). This is also the sample's cost.
    pub n: u32,
    /// Smooth offset `1 - log2(ln|z|)`.
    pub s: f32,
    /// Distance estimate in output pixels.
    pub de: f32,
    /// Normal angle `arg(z conj(dz/dc))`.
    pub ang: f32,
    pub class: Class,
    pub nexp: [u32; 4],
}

impl Default for Sample {
    fn default() -> Self {
        Self {
            n: 0,
            s: 0.0,
            de: 0.0,
            ang: 0.0,
            class: Class::Interior,
            nexp: [NEVER; 4],
        }
    }
}

impl Sample {
    /// Iterations a sample costs when it stops at the footprint threshold `k` (the
    /// "texture" cut) instead of running to escape.
    #[inline]
    pub fn cut_cost(&self, k: usize) -> u32 {
        self.n.min(self.nexp[k])
    }
    /// True when the footprint reached threshold `k` before the orbit escaped or was
    /// proven interior: the sample would be classed "texture" by the cut.
    #[inline]
    pub fn is_texture(&self, k: usize) -> bool {
        self.nexp[k] < self.n
    }
}

/// Iterates `c = (cr, ci)`; `px` is the output pixel size in complex units (DE and
/// footprint are measured in output pixels).
pub fn iterate(cr: f64, ci: f64, px: f64, max_iter: u32) -> Sample {
    // z_1 = c, dz_1/dc = 1, w_1 = dz_1/dz_1 = 1 (as the GPU's fresh-pixel step).
    let (mut zr, mut zi) = (cr, ci);
    let (mut dr, mut di) = (1.0f64, 0.0f64);
    let (mut wr, mut wi) = (1.0f64, 0.0f64);
    let mut wexp = 0i32;
    let mut n: u32 = 1;
    let mut nexp = [NEVER; 4];
    let thr2: [f64; 4] = EXPAND.map(|t| (t / px) * (t / px));
    let mut next_k = 0usize;
    let mut r2 = zr * zr + zi * zi;
    let mut class = Class::Unresolved;
    if r2 > BAILOUT2 {
        class = Class::Exterior;
    } else {
        while n < max_iter {
            // dz/dc <- 2 z dz/dc + 1 ; w <- 2 z w ; z <- z^2 + c
            let ndr = 2.0 * (zr * dr - zi * di) + 1.0;
            let ndi = 2.0 * (zr * di + zi * dr);
            dr = ndr;
            di = ndi;
            let nwr = 2.0 * (zr * wr - zi * wi);
            let nwi = 2.0 * (zr * wi + zi * wr);
            wr = nwr;
            wi = nwi;
            if n & 7 == 0 {
                let a = wr.abs().max(wi.abs());
                if a > 1.152921504606847e18 {
                    wr *= 8.673617379884035e-19;
                    wi *= 8.673617379884035e-19;
                    wexp += 60;
                } else if wexp > 0 && a < 8.673617379884035e-19 {
                    wr *= 1.152921504606847e18;
                    wi *= 1.152921504606847e18;
                    wexp -= 60;
                } else if wexp == 0 && a * (n as f64) * (n as f64) < INTERIOR_EPS {
                    class = Class::Interior;
                    break;
                }
            }
            let t = zr * zr - zi * zi + cr;
            zi = 2.0 * zr * zi + ci;
            zr = t;
            n += 1;
            if next_k < 4 {
                let d2 = dr * dr + di * di;
                while next_k < 4 && d2 >= thr2[next_k] {
                    nexp[next_k] = n;
                    next_k += 1;
                }
            }
            r2 = zr * zr + zi * zi;
            if r2 > BAILOUT2 {
                class = Class::Exterior;
                break;
            }
        }
    }
    match class {
        Class::Exterior => {
            let zr_abs = r2.sqrt();
            let dl = (dr * dr + di * di).sqrt();
            let de = if dl > 0.0 && dl.is_finite() {
                2.0 * zr_abs * zr_abs.ln() / dl / px
            } else {
                0.0
            };
            // u = z * conj(d)
            let ur = zr * dr + zi * di;
            let ui = zi * dr - zr * di;
            Sample {
                n,
                s: (1.0 - (0.5 * r2.ln()).log2()) as f32,
                de: de.min(65000.0) as f32,
                ang: ui.atan2(ur) as f32,
                class,
                nexp,
            }
        }
        _ => Sample {
            n,
            s: 0.0,
            de: 0.0,
            ang: 0.0,
            class,
            nexp,
        },
    }
}
