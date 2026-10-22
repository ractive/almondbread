//! Real-linear maps of the plane for the non-conformal formulas (F5, kb: plans/f5-mandelbar,
//! formulas/mandelbar).
//!
//! A conformal formula's derivative is one complex number: `δ ↦ a δ`. A non-conformal one
//! (Mandelbar `conj(z)² + c`, the Burning Ship family) has a real 2×2 Jacobian. Every real-linear
//! map of the plane can be written in Wirtinger form
//!
//! ```text
//! L δ = p δ + q conj(δ)        (p, q complex; [[a, b], [c, d]] <-> p = ((a+d) + i(c-b))/2,
//!                                                                  q = ((a-d) + i(c+b))/2)
//! ```
//!
//! which keeps all the arithmetic complex (one more complex number than a conformal map, the
//! existing complex and floatexp operations reused) and gives the singular values for free:
//! `sup|L| = |p| + |q|`, `inf|L| = ||p| - |q||`, `det L = |p|² - |q|²`. A conformal map is
//! `q = 0`; Mandelbar's step Jacobian at `z` is `p = 0, q = conj(2z)` (a scaled reflection);
//! Burning Ship's is one of the two, chosen by the signs of `x y` (inference, for F6).
//!
//! Composition (`self` first, then `y`): `y ∘ x = (p_y p_x + q_y conj(q_x),
//! p_y q_x + q_y conj(p_x))`. A product of two reflections is conformal again; sums (the BLA's
//! `B`, the DE derivative `dz/dc`, which gains `+ I` per step) mix both parts.

use crate::floatexp::{DoubleExp, DoubleExpComplex};

type C64 = (f64, f64);

#[inline(always)]
fn cmul(a: C64, b: C64) -> C64 {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

#[inline(always)]
fn conj(a: C64) -> C64 {
    (a.0, -a.1)
}

#[inline(always)]
fn cabs(a: C64) -> f64 {
    a.0.hypot(a.1)
}

/// `δ ↦ p δ + q conj(δ)` in f64 (module docs).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Linear {
    pub p: C64,
    pub q: C64,
}

impl Linear {
    pub const ZERO: Linear = Linear {
        p: (0.0, 0.0),
        q: (0.0, 0.0),
    };
    pub const IDENTITY: Linear = Linear {
        p: (1.0, 0.0),
        q: (0.0, 0.0),
    };

    /// Multiplication by a complex number (a conformal map).
    pub fn conformal(a: C64) -> Self {
        Linear { p: a, q: (0.0, 0.0) }
    }

    /// From the real matrix `[[a, b], [c, d]]` acting on `(x, y)`.
    pub fn from_matrix(m: [[f64; 2]; 2]) -> Self {
        let [[a, b], [c, d]] = m;
        Linear {
            p: (0.5 * (a + d), 0.5 * (c - b)),
            q: (0.5 * (a - d), 0.5 * (c + b)),
        }
    }

    /// The real matrix `[[a, b], [c, d]]`.
    pub fn matrix(self) -> [[f64; 2]; 2] {
        let (p, q) = (self.p, self.q);
        [[p.0 + q.0, q.1 - p.1], [p.1 + q.1, p.0 - q.0]]
    }

    pub fn apply(self, d: C64) -> C64 {
        let a = cmul(self.p, d);
        let b = cmul(self.q, conj(d));
        (a.0 + b.0, a.1 + b.1)
    }

    /// `self` first, then `y`: `y ∘ self`.
    pub fn then(self, y: Linear) -> Linear {
        let a = cmul(y.p, self.p);
        let b = cmul(y.q, conj(self.q));
        let c = cmul(y.p, self.q);
        let d = cmul(y.q, conj(self.p));
        Linear {
            p: (a.0 + b.0, a.1 + b.1),
            q: (c.0 + d.0, c.1 + d.1),
        }
    }

    pub fn plus(self, y: Linear) -> Linear {
        Linear {
            p: (self.p.0 + y.p.0, self.p.1 + y.p.1),
            q: (self.q.0 + y.q.0, self.q.1 + y.q.1),
        }
    }

    pub fn scale(self, k: f64) -> Linear {
        Linear {
            p: (k * self.p.0, k * self.p.1),
            q: (k * self.q.0, k * self.q.1),
        }
    }

    /// The largest singular value (the operator norm).
    pub fn sup(self) -> f64 {
        cabs(self.p) + cabs(self.q)
    }

    /// The smallest singular value.
    pub fn inf(self) -> f64 {
        (cabs(self.p) - cabs(self.q)).abs()
    }

    pub fn det(self) -> f64 {
        let (a, b) = (cabs(self.p), cabs(self.q));
        (a - b) * (a + b)
    }

    /// `x` with `self x = w` (None if singular).
    pub fn solve(self, w: C64) -> Option<C64> {
        let det = self.det();
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let a = cmul(conj(self.p), w);
        let b = cmul(self.q, conj(w));
        Some(((a.0 - b.0) / det, (a.1 - b.1) / det))
    }

    /// The equivalent complex derivative for a distance estimate at the orbit point `z`
    /// (`self` = dz/dc): `d` with `|z| |d| = |∇_c ln|z|| |z|^2` and `z / d` along that gradient,
    /// so the conformal DE formula `|z| ln|z| / |d|` and normal `z / d` apply unchanged. The
    /// gradient of `ln|z|` is `J^T z / |z|^2`, as a complex number `z conj(p) + conj(z) q`
    /// (our derivation), which gives `d = p + conj(q) z / conj(z)`.
    pub fn de_derivative(self, z: C64) -> C64 {
        let n2 = z.0 * z.0 + z.1 * z.1;
        if n2 == 0.0 {
            return self.p;
        }
        // z / conj(z) = z^2 / |z|^2.
        let u = cmul(z, z);
        let r = cmul(conj(self.q), (u.0 / n2, u.1 / n2));
        (self.p.0 + r.0, self.p.1 + r.1)
    }
}

/// [`Linear`] in extended range (Newton, the deep oracle, size estimates).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinearX {
    pub p: DoubleExpComplex,
    pub q: DoubleExpComplex,
}

impl LinearX {
    pub const ZERO: LinearX = LinearX {
        p: DoubleExpComplex::ZERO,
        q: DoubleExpComplex::ZERO,
    };
    pub const IDENTITY: LinearX = LinearX {
        p: DoubleExpComplex::ONE,
        q: DoubleExpComplex::ZERO,
    };

    pub fn conformal(a: DoubleExpComplex) -> Self {
        LinearX {
            p: a,
            q: DoubleExpComplex::ZERO,
        }
    }

    pub fn apply(self, d: DoubleExpComplex) -> DoubleExpComplex {
        self.p * d + self.q * d.conj()
    }

    /// `self` first, then `y`.
    pub fn then(self, y: LinearX) -> LinearX {
        LinearX {
            p: y.p * self.p + y.q * self.q.conj(),
            q: y.p * self.q + y.q * self.p.conj(),
        }
    }

    pub fn plus(self, y: LinearX) -> LinearX {
        LinearX {
            p: self.p + y.p,
            q: self.q + y.q,
        }
    }

    pub fn sup(self) -> DoubleExp {
        self.p.magnitude() + self.q.magnitude()
    }

    pub fn inf(self) -> DoubleExp {
        (self.p.magnitude() - self.q.magnitude()).abs()
    }

    pub fn det(self) -> DoubleExp {
        self.p.magnitude_squared() - self.q.magnitude_squared()
    }

    /// `x` with `self x = w` (None if singular or not finite).
    pub fn solve(self, w: DoubleExpComplex) -> Option<DoubleExpComplex> {
        let det = self.det();
        if det.is_zero() || !det.mantissa.is_finite() {
            return None;
        }
        let n = self.p.conj() * w - self.q * w.conj();
        Some(DoubleExpComplex::new(n.re / det, n.im / det))
    }

    pub fn to_f64(self) -> Linear {
        Linear {
            p: (self.p.re.to_f64(), self.p.im.to_f64()),
            q: (self.q.re.to_f64(), self.q.im.to_f64()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: C64, b: C64) -> bool {
        (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12
    }

    #[test]
    fn matrix_round_trip_and_composition() {
        let m = [[1.5, -0.25], [0.75, 2.0]];
        let l = Linear::from_matrix(m);
        assert_eq!(l.matrix(), m);
        let v = (0.3, -0.7);
        let want = (m[0][0] * v.0 + m[0][1] * v.1, m[1][0] * v.0 + m[1][1] * v.1);
        assert!(close(l.apply(v), want));
        let k = Linear::from_matrix([[0.2, 1.0], [-0.4, 0.1]]);
        assert!(close(l.then(k).apply(v), k.apply(l.apply(v))));
        // Singular values against the matrix: sup * inf = |det|, sup^2 + inf^2 = |M|_F^2.
        let f2: f64 = m.iter().flatten().map(|x| x * x).sum();
        assert!((l.sup() * l.inf() - l.det().abs()).abs() < 1e-12);
        assert!((l.sup().powi(2) + l.inf().powi(2) - f2).abs() < 1e-12);
        let x = l.solve(v).unwrap();
        assert!(close(l.apply(x), v));
    }

    /// Mandelbar's step Jacobian `(0, conj(2z))` is `δ ↦ conj(2 z δ)`, a scaled reflection.
    #[test]
    fn mandelbar_jacobian_is_a_scaled_reflection() {
        let z = (0.4, -0.3);
        let j = Linear {
            p: (0.0, 0.0),
            q: conj((2.0 * z.0, 2.0 * z.1)),
        };
        let d = (1e-3, 2e-3);
        assert!(close(j.apply(d), conj(cmul((2.0 * z.0, 2.0 * z.1), d))));
        assert!((j.sup() - j.inf()).abs() < 1e-15 && (j.sup() - 2.0 * cabs(z)).abs() < 1e-15);
        // Two reflections compose to a rotation-scaling.
        assert_eq!(j.then(j).q, (0.0, 0.0));
    }

    /// The DE derivative of a conformal map is the map's own multiplier.
    #[test]
    fn de_derivative_is_conformal_for_q_zero() {
        let l = Linear::conformal((0.3, 0.9));
        assert_eq!(l.de_derivative((1.2, -0.4)), (0.3, 0.9));
        // In general |z| |d| = |J^T z|.
        let l = Linear::from_matrix([[1.0, 2.0], [0.5, -1.0]]);
        let z = (0.6, 0.8);
        let m = l.matrix();
        let jt = (m[0][0] * z.0 + m[1][0] * z.1, m[0][1] * z.0 + m[1][1] * z.1);
        assert!((cabs(z) * cabs(l.de_derivative(z)) - cabs(jt)).abs() < 1e-12);
    }

    #[test]
    fn extended_matches_f64() {
        let dx = |a: C64| DoubleExpComplex::new(DoubleExp::from_f64(a.0), DoubleExp::from_f64(a.1));
        let l = Linear::from_matrix([[1.5, -0.25], [0.75, 2.0]]);
        let k = Linear::from_matrix([[0.2, 1.0], [-0.4, 0.1]]);
        let lx = LinearX { p: dx(l.p), q: dx(l.q) };
        let kx = LinearX { p: dx(k.p), q: dx(k.q) };
        let c = lx.then(kx).to_f64();
        let want = l.then(k);
        assert!(close(c.p, want.p) && close(c.q, want.q));
        assert!((lx.sup().to_f64() - l.sup()).abs() < 1e-12);
        let s = lx.solve(dx((0.3, 0.1))).unwrap();
        assert!(close((s.re.to_f64(), s.im.to_f64()), l.solve((0.3, 0.1)).unwrap()));
    }
}
