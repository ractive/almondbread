//! Local series approximation: samples near a seed point `c0` share its orbit to second order,
//! `z_n(c0 + δ) ≈ z_n(c0) + A_n δ + B_n δ²/2` with `A = dz/dc`, `B = d²z/dc²`, until the
//! quadratic term exceeds `eps` times the linear one over the region's radius. From that
//! iteration each sample continues exactly. The seed is the pixel's own centre (AA samples) or
//! a tile centre (every pixel of a k x k tile). Our design (kb: techniques/adaptive-sampling);
//! the expansion is the classic series approximation, here around a local orbit instead of the
//! view's reference (kb: techniques/series-approximation).

use crate::sample::{BAILOUT2, Class, INTERIOR_EPS, NEVER, Sample};

#[derive(Clone, Copy, Debug)]
pub struct Seed {
    z: (f64, f64),
    a: (f64, f64),
    b: (f64, f64),
    n: u32,
}

#[inline]
fn mul(x: (f64, f64), y: (f64, f64)) -> (f64, f64) {
    (x.0 * y.0 - x.1 * y.1, x.0 * y.1 + x.1 * y.0)
}

/// Iterates the seed `c0` and returns the last state at which the expansion is still valid for
/// |δ| <= `radius` (complex units): `|B| radius / (2 |A|) <= eps`.
pub fn seed(c0: (f64, f64), radius: f64, eps: f64, max_iter: u32) -> Seed {
    let start = Seed {
        z: c0,
        a: (1.0, 0.0),
        b: (0.0, 0.0),
        n: 1,
    };
    seed_from(&start, c0, (0.0, 0.0), radius, eps, max_iter)
}

/// A nested seed: the point `c0 = parent's c + δ` starts from the parent's branch state
/// (z + A δ + B δ²/2, A + B δ, B) and iterates on as a seed for a smaller radius.
pub fn seed_from(p: &Seed, c0: (f64, f64), delta: (f64, f64), radius: f64, eps: f64, max_iter: u32) -> Seed {
    let ad = mul(p.a, delta);
    let bd2 = mul(p.b, mul(delta, delta));
    let bd = mul(p.b, delta);
    let mut z = (p.z.0 + ad.0 + 0.5 * bd2.0, p.z.1 + ad.1 + 0.5 * bd2.1);
    let mut a = (p.a.0 + bd.0, p.a.1 + bd.1);
    let mut b = p.b;
    let mut w = (1.0, 0.0);
    let mut wexp = 0i32;
    let mut n = p.n;
    let mut last = Seed { z, a, b, n };
    while n < max_iter {
        // B <- 2 (A² + z B), A <- 2 z A + 1, w <- 2 z w, z <- z² + c
        let a2 = mul(a, a);
        let zb = mul(z, b);
        b = (2.0 * (a2.0 + zb.0), 2.0 * (a2.1 + zb.1));
        let za = mul(z, a);
        a = (2.0 * za.0 + 1.0, 2.0 * za.1);
        let zw = mul(z, w);
        w = (2.0 * zw.0, 2.0 * zw.1);
        let zz = mul(z, z);
        z = (zz.0 + c0.0, zz.1 + c0.1);
        n += 1;
        let r2 = z.0 * z.0 + z.1 * z.1;
        let an = (a.0 * a.0 + a.1 * a.1).sqrt();
        let bn = (b.0 * b.0 + b.1 * b.1).sqrt();
        if !(bn * radius <= 2.0 * eps * an) || r2 > BAILOUT2 {
            return last;
        }
        // Interior rescale every 8 (same rule as the sample loop; the test itself is left to
        // the continuing samples).
        if n & 7 == 0 {
            let m = w.0.abs().max(w.1.abs());
            if m > 1.152921504606847e18 {
                w = (w.0 * 8.673617379884035e-19, w.1 * 8.673617379884035e-19);
                wexp += 60;
            } else if wexp > 0 && m < 8.673617379884035e-19 {
                w = (w.0 * 1.152921504606847e18, w.1 * 1.152921504606847e18);
                wexp -= 60;
            } else if wexp == 0 && m * (n as f64) * (n as f64) < INTERIOR_EPS {
                // Interior seed: branch here; each sample runs its own interior test (with w
                // restarted at the branch, see continue_from).
                return Seed { z, a, b, n };
            }
        }
        last = Seed { z, a, b, n };
    }
    last
}

impl Seed {
    pub fn n(&self) -> u32 {
        self.n
    }
}

/// Continues the sample at `c = c0 + δ` from the seed's branch state, exactly (f64 direct
/// iteration), with the same outputs as `sample::iterate`. Its cost is `result.n - seed.n`.
pub fn continue_from(s: &Seed, c: (f64, f64), delta: (f64, f64), px: f64, max_iter: u32) -> Sample {
    let ad = mul(s.a, delta);
    let bd2 = mul(s.b, mul(delta, delta));
    let mut z = (s.z.0 + ad.0 + 0.5 * bd2.0, s.z.1 + ad.1 + 0.5 * bd2.1);
    let bd = mul(s.b, delta);
    let mut d = (s.a.0 + bd.0, s.a.1 + bd.1);
    // Interior test: w = dz_n/dz_m restarted at the branch point m (the seed's w does not
    // carry over to other c); the n² guard keeps the absolute n.
    let mut w = (1.0f64, 0.0f64);
    let mut wexp = 0i32;
    let mut n = s.n;
    let mut r2 = z.0 * z.0 + z.1 * z.1;
    let mut class = Class::Unresolved;
    if r2 > BAILOUT2 {
        class = Class::Exterior;
    } else {
        while n < max_iter {
            let zd = mul(z, d);
            d = (2.0 * zd.0 + 1.0, 2.0 * zd.1);
            let zw = mul(z, w);
            w = (2.0 * zw.0, 2.0 * zw.1);
            if n & 7 == 0 {
                let m = w.0.abs().max(w.1.abs());
                if m > 1.152921504606847e18 {
                    w = (w.0 * 8.673617379884035e-19, w.1 * 8.673617379884035e-19);
                    wexp += 60;
                } else if wexp > 0 && m < 8.673617379884035e-19 {
                    w = (w.0 * 1.152921504606847e18, w.1 * 1.152921504606847e18);
                    wexp -= 60;
                } else if wexp == 0 && m * (n as f64) * (n as f64) < INTERIOR_EPS {
                    class = Class::Interior;
                    break;
                }
            }
            let zz = mul(z, z);
            z = (zz.0 + c.0, zz.1 + c.1);
            n += 1;
            r2 = z.0 * z.0 + z.1 * z.1;
            if r2 > BAILOUT2 {
                class = Class::Exterior;
                break;
            }
        }
    }
    match class {
        Class::Exterior => {
            let zr = r2.sqrt();
            let dl = (d.0 * d.0 + d.1 * d.1).sqrt();
            let de = if dl > 0.0 && dl.is_finite() {
                2.0 * zr * zr.ln() / dl / px
            } else {
                0.0
            };
            let ur = z.0 * d.0 + z.1 * d.1;
            let ui = z.1 * d.0 - z.0 * d.1;
            Sample {
                n,
                s: (1.0 - (0.5 * r2.ln()).log2()) as f32,
                de: de.min(65000.0) as f32,
                ang: ui.atan2(ur) as f32,
                class,
                nexp: [NEVER; 4],
            }
        }
        _ => Sample {
            n,
            s: 0.0,
            de: 0.0,
            ang: 0.0,
            class,
            nexp: [NEVER; 4],
        },
    }
}
