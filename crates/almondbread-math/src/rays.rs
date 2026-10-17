//! External rays of the Mandelbrot set: binary angles and tracing a ray inward from the
//! escape circle (kb: synthesis/places "Endless dives"; the method is Heiland-Allen's, mathr
//! "m_d_exray_in": per angle doubling a few points at radii R^(2^(-m/s)), each solved by
//! Newton on z_n(c) = r e^(2 pi i 2^(n-1) theta)). Used to find Misiurewicz points named by
//! a long preperiodic angle (`almondbread locate --endless-seed n`): the landing point of a
//! ray with preperiod k' and period p is a Misiurewicz point of preperiod k' + 1 (z_0 = 0)
//! whose period divides p. Slow (BigComplex): catalogue work, not per frame.

use crate::{BigComplex, BigFloat};

/// A preperiodic binary angle 0.pre(per)... (turns).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryAngle {
    pub preperiod: Vec<bool>,
    pub period: Vec<bool>,
}

impl BinaryAngle {
    /// Bit `i` after the binary point (0-based).
    pub fn bit(&self, i: usize) -> bool {
        if i < self.preperiod.len() {
            self.preperiod[i]
        } else {
            self.period[(i - self.preperiod.len()) % self.period.len()]
        }
    }

    /// 2^n theta mod 1 to 53 bits.
    pub fn doubled(&self, n: usize) -> f64 {
        let mut x = 0.0f64;
        for i in (n..n + 53).rev() {
            x = (x + if self.bit(i) { 1.0 } else { 0.0 }) * 0.5;
        }
        x
    }

    /// A pseudo-random angle: `preperiod` bits from `seed` (SplitMix64), then `period` bits
    /// from the same stream, not all equal, with the last preperiodic bit differing from the
    /// last periodic one so that the preperiod is exactly `preperiod`.
    pub fn seeded(seed: u64, preperiod: usize, period: usize) -> Self {
        assert!(period >= 2, "period must be >= 2");
        let mut state = seed;
        let mut next_bit = || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = state;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (x ^ (x >> 31)) >> 63 == 1
        };
        let mut pre: Vec<bool> = (0..preperiod).map(|_| next_bit()).collect();
        let mut per: Vec<bool> = (0..period).map(|_| next_bit()).collect();
        if per.iter().all(|&b| b == per[0]) {
            per[period - 1] = !per[0];
        }
        if let Some(last) = pre.last_mut() {
            *last = !per[period - 1];
        }
        BinaryAngle {
            preperiod: pre,
            period: per,
        }
    }

    /// `.pre(per)` with 0/1 digits.
    pub fn to_binary_string(&self) -> String {
        let s = |v: &[bool]| v.iter().map(|&b| if b { '1' } else { '0' }).collect::<String>();
        format!(".{}({})", s(&self.preperiod), s(&self.period))
    }
}

/// Settings for [`trace_ray_inward`].
#[derive(Clone, Debug)]
pub struct RayOptions {
    /// Points per angle doubling (mathr's "sharpness"; 4 to 8).
    pub sharpness: usize,
    /// Radius of the escape circle where the ray starts.
    pub escape_radius: f64,
    /// Newton steps allowed per ray point.
    pub max_newton_steps: usize,
}

impl Default for RayOptions {
    fn default() -> Self {
        RayOptions {
            sharpness: 4,
            escape_radius: 65536.0,
            max_newton_steps: 32,
        }
    }
}

/// Traces the external ray of `angle` from the escape circle inward through `doublings`
/// angle doublings and returns the last point (on the ray at potential log R / 2^doublings).
/// Precision grows with the depth (2 n + 128 bits at doubling n). `progress(n)` is called
/// once per doubling. `None` if a Newton solve fails (derivative zero or no convergence).
pub fn trace_ray_inward(
    angle: &BinaryAngle,
    doublings: usize,
    options: &RayOptions,
    progress: &mut dyn FnMut(usize),
) -> Option<BigComplex> {
    let r0 = options.escape_radius;
    let t0 = angle.doubled(0) * std::f64::consts::TAU;
    let bits_for = |n: usize| BigFloat::limb_count_for_bits(2 * n + 128) * 64;
    let mut c = BigComplex::from_f64(r0 * t0.cos(), r0 * t0.sin(), bits_for(1));
    for n in 1..=doublings {
        let bits = bits_for(n);
        c = c.with_precision(bits);
        let one = BigComplex::from_f64(1.0, 0.0, bits);
        // z_n = z_{n-1}^2 + c with z_1 = c, so arg z_n = 2^(n-1) theta on the ray.
        let turn = angle.doubled(n - 1) * std::f64::consts::TAU;
        for m in 1..=options.sharpness {
            let r = r0.powf(0.5f64.powf(m as f64 / options.sharpness as f64));
            let target = BigComplex::from_f64(r * turn.cos(), r * turn.sin(), bits);
            let mut converged = false;
            for _ in 0..options.max_newton_steps {
                let mut z = c.clone();
                let mut dz = one.clone();
                for _ in 1..n {
                    dz = &(&z * &dz).times_power_of_two(1) + &one;
                    z = &z.square() + &c;
                }
                let f = &z - &target;
                let [fr, fi] = f.to_f64();
                if (fr * fr + fi * fi).sqrt() < r * 1e-12 {
                    converged = true;
                    break;
                }
                if dz.re.is_zero() && dz.im.is_zero() {
                    return None;
                }
                c = &c - &(&f / &dz);
            }
            if !converged {
                return None;
            }
        }
        progress(n);
    }
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewtonOptions, find_misiurewicz};

    #[test]
    fn seeded_angle_has_exact_preperiod() {
        let a = BinaryAngle::seeded(7, 20, 3);
        assert_eq!(a, BinaryAngle::seeded(7, 20, 3));
        assert_ne!(a.preperiod[19], a.period[2]);
        assert!(a.period.iter().any(|&b| b != a.period[0]));
        assert!((a.doubled(20) - a.doubled(23)).abs() < 1e-15);
    }

    #[test]
    fn ray_one_sixth_lands_on_i() {
        // 1/6 = .0(01): preperiod 1, period 2 -> c = i = M(2,2).
        let a = BinaryAngle {
            preperiod: vec![false],
            period: vec![false, true],
        };
        let c = trace_ray_inward(&a, 12, &RayOptions::default(), &mut |_| {}).unwrap();
        let opts = NewtonOptions {
            precision_bits: Some(256),
            max_steps: 64,
            max_distance: None,
        };
        let m = find_misiurewicz(&c, 2, 2, &opts).unwrap();
        let [re, im] = m.point.to_f64();
        assert!(re.abs() < 1e-12 && (im - 1.0).abs() < 1e-12, "{re} {im}");
    }
}
