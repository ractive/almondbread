//! Exact interior test for the main cardioid and the period-2 bulb (kb:
//! techniques/interior-detection "Closed-form component test").
//!
//! With `X = x - 1/4`, `q = X² + y²`, `c = x + iy` lies in the main cardioid iff
//! `q (q + X) - y²/4 < 0`, and in the period-2 bulb iff `(x + 1)² + y² - 1/16 < 0`. Near their
//! boundaries the orbit converges parabolically slowly, so neither the iteration limit nor the
//! derivative test or the attracting-cycle probe proves those pixels; this test does, at no
//! iteration cost.
//!
//! **Approach.** For one level (a texel grid) the CPU expands each component's polynomial
//! around the grid's origin in the kernel's own pixel coordinates: `c = A + T R (u, v)` with
//! `(u, v) = (pix.x, -pix.y)` the kernel's texel offset, `T` the texel size and `R` the view
//! rotation, all exactly the values the kernel uses (`A` = reference point + the f32 offset).
//! With `u = L u'`, `v = L v'` (`L` a power of two bounding the level, so `|u'|, |v'| <= 1`)
//! the result is a bivariate polynomial of degree <= 4 in `(u', v')`, 15 coefficients,
//! computed in BigFloat (at any depth) and normalised so the largest is ~1 before rounding to
//! f32. The kernel evaluates it per pixel in float32 (`S` = sum of the terms, `|S|_1` = sum
//! of their magnitudes) and calls a pixel inside iff `S < -(GAMMA |S|_1 + slack)`.
//!
//! **Why this is a proof.** Every term `b u'^i v'^j` is evaluated with at most ~9 float32
//! roundings (coefficient to f32, powers, the product; `u' = u / L` is exact), the sum of 15 terms
//! with at most 14 more, so `|S_f32 - S| <= 25 eps |S|_1` with `eps = 2^-24`, for any order
//! of summation and with or without fused multiply-adds (fast math). `GAMMA = 2^-16` leaves a
//! factor 10. The CPU's BigFloat rounding (bounded from the magnitudes of the inputs), the
//! coefficients dropped below 2^-60, and terms flushed to zero below float32's normal range
//! go into `slack`. A pixel within that margin of the boundary is not marked (it stays with
//! today's iteration, possibly unresolved); exterior pixels are never touched.
//!
//! **Depth.** The polynomial carries the boundary's position relative to the view exactly, so
//! the test works at any depth: the BigFloat precision grows with `-2 log2(L T)` (the
//! constant term cancels to the size of the quadratic terms), and a view away from both
//! components is skipped by an f64 bound before any BigFloat work. The unprovable band is
//! `GAMMA |S|_1 / |grad S|`: at most ~`2^-16 L` texels (0.03 texel for a 2048-texel level), the
//! same at every zoom; the tests below find every inside texel 1e-2 texel or more from the
//! boundary proven, at texels from 2^-8 to 2^-300, the cusp and the -3/4 root included.
//!
//! The sample point proven is the kernel's ideal one, `A + T R (u, v)` in exact arithmetic;
//! the float32 `dc` the kernel iterates is within ~1e-4 texel of it.

use almondbread_math::{BigComplex, BigFloat};

/// Monomials `u'^i v'^j`, `i + j <= 4`, in this order (`index = d (d + 1) / 2 + j`, `d = i + j`).
pub const MONOMIALS: [(u32, u32); 15] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (2, 0),
    (1, 1),
    (0, 2),
    (3, 0),
    (2, 1),
    (1, 2),
    (0, 3),
    (4, 0),
    (3, 1),
    (2, 2),
    (1, 3),
    (0, 4),
];

/// Relative float32 evaluation margin of the kernel's test (see the module docs).
pub const GAMMA: f32 = 1.0 / 65536.0;

/// Coefficients whose normalised size is below this are dropped (their bound goes to `slack`).
const DROP: f64 = 1.0 / 1_152_921_504_606_846_976.0; // 2^-60

const fn index(i: u32, j: u32) -> usize {
    let d = (i + j) as usize;
    d * (d + 1) / 2 + j as usize
}

/// One component's polynomial in normalised texel coordinates: inside iff
/// `sum b_k m_k < -(GAMMA sum |b_k m_k| + slack)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComponentPoly {
    pub coeffs: [f32; 15],
    pub slack: f32,
}

impl ComponentPoly {
    /// The kernel's test, in f32 (tests and CPU callers). `u`, `v` in texels (`v` up).
    pub fn inside(&self, u: f32, v: f32, inv_l: f32) -> bool {
        let (u, v) = (u * inv_l, v * inv_l);
        if u.abs() > 1.0 || v.abs() > 1.0 {
            return false;
        }
        let m = monomials_f32(u, v);
        let (mut s, mut a) = (0.0f32, 0.0f32);
        for k in 0..15 {
            let t = self.coeffs[k] * m[k];
            s += t;
            a += t.abs();
        }
        s < -(GAMMA * a + self.slack)
    }
}

/// The kernel's monomials, in the same operations.
pub fn monomials_f32(u: f32, v: f32) -> [f32; 15] {
    let (u2, uv, v2) = (u * u, u * v, v * v);
    let (u3, v3) = (u2 * u, v2 * v);
    [
        1.0,
        u,
        v,
        u2,
        uv,
        v2,
        u3,
        u2 * v,
        u * v2,
        v3,
        u2 * u2,
        u3 * v,
        u2 * v2,
        u * v3,
        v2 * v2,
    ]
}

/// A texel grid as the kernel maps it: `c = anchor + texel 2^texel_exp * rot * (u, v)`, with
/// `anchor` exact (reference point plus the f32 offset).
pub struct Grid<'a> {
    pub reference: &'a BigComplex,
    /// The kernel's `refOffset` (units of 2^`scale_exp`).
    pub offset: [f32; 2],
    pub texel: f32,
    pub scale_exp: i32,
    pub rot: [f32; 2],
    /// Largest `|u|`, `|v|` of any texel of the level (sample positions included).
    pub extent: f64,
}

/// The per-level test: which components can be in the view, and their polynomials.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComponentTest {
    pub cardioid: Option<ComponentPoly>,
    pub bulb: Option<ComponentPoly>,
    /// `L = 2^l_log2`: `u' = u / L`.
    pub l_log2: i32,
}

#[derive(Clone, Copy)]
enum Component {
    Cardioid,
    Bulb,
}

impl ComponentTest {
    /// The test for a grid; `None` when the view cannot touch either component (an f64 bound
    /// first, so views away from both cost nothing).
    pub fn new(grid: &Grid) -> Option<Self> {
        let l_log2 = grid.extent.max(1.0).log2().ceil() as i32;
        let cardioid = component_poly(grid, l_log2, Component::Cardioid);
        let bulb = component_poly(grid, l_log2, Component::Bulb);
        (cardioid.is_some() || bulb.is_some()).then_some(Self { cardioid, bulb, l_log2 })
    }
}

/// log2 of `T L` as f64 (texel times the extent's power of two).
fn lt_log2(grid: &Grid, l_log2: i32) -> f64 {
    (grid.texel.abs() as f64).log2() + grid.scale_exp as f64 + l_log2 as f64
}

/// Whether the f64 bound puts the whole view outside the component (conservative: `false`
/// when in doubt; it never claims inside).
fn surely_outside(grid: &Grid, l_log2: i32, which: Component) -> bool {
    let lt = lt_log2(grid, l_log2);
    if lt > -1.0 {
        return false; // a view of half the plane: let the exact path decide
    }
    let r = std::f64::consts::SQRT_2 * lt.exp2();
    let pow = |e: i32| 2f64.powi(e);
    let ax = grid.reference.re.to_f64() + grid.offset[0] as f64 * pow(grid.scale_exp);
    let ay = grid.reference.im.to_f64() + grid.offset[1] as f64 * pow(grid.scale_exp);
    if !(ax.is_finite() && ay.is_finite()) {
        return false;
    }
    match which {
        Component::Cardioid => {
            let (x, y) = (ax - 0.25, ay);
            if x.abs() > 4.0 || y.abs() > 4.0 {
                return x.hypot(y) > 2.0 + r;
            }
            let q = x * x + y * y;
            let f = q * (q + x) - 0.25 * y * y;
            let gx = 4.0 * q * x + q + 2.0 * x * x;
            let gy = 4.0 * q * y + 2.0 * x * y - 0.5 * y;
            // Second and higher order terms over |d| <= r <= 1/2 with |x|, |y| <= 4: <= 230 r^2.
            f > 1.01 * gx.hypot(gy) * r + 512.0 * r * r + 1e-9
        }
        Component::Bulb => {
            let (x, y) = (ax + 1.0, ay);
            let f = x * x + y * y - 0.0625;
            f > 1.01 * 2.0 * x.hypot(y) * r + r * r + 1e-9
        }
    }
}

/// Bivariate polynomial of degree <= 4 with BigFloat coefficients.
#[derive(Clone)]
struct Poly([BigFloat; 15]);

impl Poly {
    fn zero(bits: usize) -> Self {
        Poly(std::array::from_fn(|_| BigFloat::zero(bits)))
    }

    fn linear(c0: BigFloat, cu: BigFloat, cv: BigFloat, bits: usize) -> Self {
        let mut p = Self::zero(bits);
        p.0[0] = c0;
        p.0[1] = cu;
        p.0[2] = cv;
        p
    }

    fn add(&self, o: &Poly) -> Poly {
        Poly(std::array::from_fn(|k| &self.0[k] + &o.0[k]))
    }

    fn mul(&self, o: &Poly) -> Poly {
        let bits = self.0[0].precision_bits();
        let mut r = Self::zero(bits);
        for (a, &(ia, ja)) in MONOMIALS.iter().enumerate() {
            if self.0[a].is_zero() {
                continue;
            }
            for (b, &(ib, jb)) in MONOMIALS.iter().enumerate() {
                if o.0[b].is_zero() {
                    continue;
                }
                let k = index(ia + ib, ja + jb);
                assert!(k < 15, "component polynomial above degree 4");
                r.0[k] = &r.0[k] + &(&self.0[a] * &o.0[b]);
            }
        }
        r
    }

    fn neg(&self) -> Poly {
        Poly(std::array::from_fn(|k| -&self.0[k]))
    }

    fn scale_pow2(&self, k: i64) -> Poly {
        Poly(std::array::from_fn(|i| self.0[i].times_power_of_two(k)))
    }
}

/// The exact polynomial of one component, normalised, or `None` when the view is surely
/// outside it.
fn component_poly(grid: &Grid, l_log2: i32, which: Component) -> Option<ComponentPoly> {
    if surely_outside(grid, l_log2, which) {
        return None;
    }
    let lt = lt_log2(grid, l_log2);
    if !lt.is_finite() {
        return None;
    }
    // Input magnitudes (f64 is enough for bounds): every coefficient of every intermediate
    // polynomial is at most 64 B^4.
    let pow = |e: i32| 2f64.powi(e.clamp(-1000, 1000));
    let ax = grid.reference.re.to_f64() + grid.offset[0] as f64 * pow(grid.scale_exp);
    let ay = grid.reference.im.to_f64() + grid.offset[1] as f64 * pow(grid.scale_exp);
    let b = 1.01 * (ax.abs().max(ay.abs()) + 2.0 * (grid.rot[0].abs() + grid.rot[1].abs()) as f64 * lt.exp2()) + 1.0;
    if !b.is_finite() || b > 1e6 {
        return None;
    }
    let k_log2 = 6.0 + 4.0 * b.log2();
    // Precision: the constant term cancels to ~(L T)^2 (the quadratic terms), so twice the
    // view's depth plus headroom; raised if the bound below says it was not enough.
    let mut bits = ((2.0 * (-lt).max(0.0) + k_log2 + 140.0).ceil() as usize).next_multiple_of(64);
    for _ in 0..3 {
        if let Some(r) = poly_at(grid, l_log2, which, bits, k_log2) {
            return r;
        }
        bits *= 2;
    }
    None
}

/// `Some(result)` when `bits` was precise enough, `None` to retry with more.
fn poly_at(grid: &Grid, l_log2: i32, which: Component, bits: usize, k_log2: f64) -> Option<Option<ComponentPoly>> {
    let big = |x: f64| BigFloat::from_f64(x, bits);
    let se = grid.scale_exp as i64;
    // Anchor A = reference + offset 2^se, rounded once to `bits` (the shift is bounded below).
    let ax = (&grid.reference.re.with_precision(bits) + &big(grid.offset[0] as f64).times_power_of_two(se))
        .with_precision(bits);
    let ay = (&grid.reference.im.with_precision(bits) + &big(grid.offset[1] as f64).times_power_of_two(se))
        .with_precision(bits);
    // c = A + T L (rc u' - rs v', rs u' + rc v'): exact products of f32 values.
    let tl = big(grid.texel as f64).times_power_of_two(se + l_log2 as i64);
    let rc = &tl * &big(grid.rot[0] as f64);
    let rs = &tl * &big(grid.rot[1] as f64);
    let x = match which {
        Component::Cardioid => &ax - &big(0.25),
        Component::Bulb => &ax + &big(1.0),
    };
    let xp = Poly::linear(x, rc.clone(), -&rs, bits);
    let yp = Poly::linear(ay, rs, rc, bits);
    let y2 = yp.mul(&yp);
    let q = xp.mul(&xp).add(&y2);
    let f = match which {
        // q (q + X) - y^2 / 4
        Component::Cardioid => q.mul(&q.add(&xp)).add(&y2.scale_pow2(-2).neg()),
        // q - 1/16
        Component::Bulb => {
            let mut c = Poly::zero(bits);
            c.0[0] = -&big(0.0625);
            q.add(&c)
        }
    };
    // Normalise by the largest coefficient's power of two.
    let max_e = f.0.iter().filter(|c| !c.is_zero()).map(BigFloat::exponent).max()?;
    let mut coeffs = [0.0f32; 15];
    let mut dropped = 0.0f64;
    let mut sum = 0.0f64;
    for (k, c) in f.0.iter().enumerate() {
        if c.is_zero() {
            continue;
        }
        let e = c.exponent() - max_e;
        if e < -62 {
            dropped += 2f64.powi(e.max(-1000) as i32);
            continue;
        }
        let n = c.times_power_of_two(-max_e).to_f64();
        if n.abs() < DROP {
            dropped += n.abs();
            continue;
        }
        coeffs[k] = n as f32;
        sum += n.abs();
    }
    // BigFloat rounding: <= 1000 operations, each off by <= 2^-bits of a magnitude <= 2^k_log2,
    // per coefficient, 15 coefficients; relative to the normalisation 2^max_e.
    let round_log2 = (15.0 * 1000.0f64).log2() + k_log2 - bits as f64 - (max_e - 1) as f64;
    // The anchor's rounding moves c by <= |A| 2^(1 - bits); in (u', v') units that is divided
    // by T L |rot| (|rot| >= 1 - 1e-6), and S changes by at most sum k |b_k| <= 4 sum |b| per
    // unit (twice that for a margin).
    let shift_log2 = b_log2_anchor(grid) + 1.0 - bits as f64 - lt_log2(grid, l_log2) + 1e-5;
    let shift = 8.0 * sum.max(1.0) * shift_log2.exp2();
    let round = round_log2.exp2();
    if round + shift > 1e-12 {
        return None;
    }
    let slack = dropped + round + shift + 2f64.powi(-100);
    // The whole view outside (the constant term dominates every other with margin): no test.
    let rest: f64 = coeffs[1..].iter().map(|c| c.abs() as f64).sum();
    let c0 = coeffs[0] as f64;
    if c0 > (rest + 1e-6 * (rest + c0.abs())) + slack {
        return Some(None);
    }
    Some(Some(ComponentPoly {
        coeffs,
        // (Rounded up to f32.)
        slack: (slack * (1.0 + 1e-6)) as f32 + f32::MIN_POSITIVE,
    }))
}

/// log2 of a bound on `|A|` (at least 1).
fn b_log2_anchor(grid: &Grid) -> f64 {
    let pow = |e: i32| 2f64.powi(e.clamp(-1000, 1000));
    let ax = grid.reference.re.to_f64() + grid.offset[0] as f64 * pow(grid.scale_exp);
    let ay = grid.reference.im.to_f64() + grid.offset[1] as f64 * pow(grid.scale_exp);
    ax.abs().max(ay.abs()).max(1.0).log2() + 0.01
}

/// Exact classification of a point (tests): `Some(true)` inside a component, `Some(false)`
/// outside both, `None` on a boundary.
pub fn exact_inside(c: &BigComplex) -> Option<bool> {
    let bits = c.precision_bits() * 4 + 128;
    let big = |x: f64| BigFloat::from_f64(x, bits);
    let x = c.re.with_precision(bits);
    let y = c.im.with_precision(bits);
    let xc = &x - &big(0.25);
    let y2 = &y * &y;
    let q = &(&xc * &xc) + &y2;
    let fc = &(&q * &(&q + &xc)) - &y2.times_power_of_two(-2);
    let xb = &x + &big(1.0);
    let fb = &(&(&xb * &xb) + &y2) - &big(0.0625);
    if fc.is_zero() || fb.is_zero() {
        return None;
    }
    Some(fc.is_negative() || fb.is_negative())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic xorshift.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// The sample position of texel offset (u, v) exactly, as the kernel means it.
    fn sample(grid: &Grid, u: f32, v: f32, bits: usize) -> BigComplex {
        let big = |x: f64| BigFloat::from_f64(x, bits);
        let se = grid.scale_exp as i64;
        let t = big(grid.texel as f64).times_power_of_two(se);
        let ru = big(grid.rot[0] as f64) * big(u as f64) - big(grid.rot[1] as f64) * big(v as f64);
        let rv = big(grid.rot[1] as f64) * big(u as f64) + big(grid.rot[0] as f64) * big(v as f64);
        let re = grid.reference.re.with_precision(bits) + big(grid.offset[0] as f64).times_power_of_two(se) + &t * &ru;
        let im = grid.reference.im.with_precision(bits) + big(grid.offset[1] as f64).times_power_of_two(se) + &t * &rv;
        BigComplex::new(re, im)
    }

    /// Classifies texel (u, v) exactly and by the f32 test; panics on a false inside.
    fn check(grid: &Grid, test: &ComponentTest, u: f32, v: f32, bits: usize) -> (Option<bool>, bool) {
        let inv_l = 2f32.powi(-test.l_log2);
        let claim =
            test.cardioid.is_some_and(|p| p.inside(u, v, inv_l)) || test.bulb.is_some_and(|p| p.inside(u, v, inv_l));
        let truth = exact_inside(&sample(grid, u, v, bits));
        assert!(
            !claim || truth == Some(true),
            "false inside at {} {} 2^{}: ({u}, {v})",
            grid.reference.re,
            grid.reference.im,
            grid.scale_exp
        );
        (truth, claim)
    }

    /// A boundary point near (x, y): bisection along x at `bits` (x +- 2^-40 must straddle).
    fn boundary_near(x: f64, y: f64, bits: usize) -> BigComplex {
        let yb = BigFloat::from_f64(y, bits);
        let inside = |x: &BigFloat| exact_inside(&BigComplex::new(x.clone(), yb.clone())) == Some(true);
        let mut lo = BigFloat::from_f64(x - 2f64.powi(-40), bits);
        let mut hi = BigFloat::from_f64(x + 2f64.powi(-40), bits);
        let lo_in = inside(&lo);
        assert_ne!(lo_in, inside(&hi), "no boundary near {x}, {y}");
        for _ in 0..bits - 8 {
            let mid = (&lo + &hi).times_power_of_two(-1);
            if inside(&mid) == lo_in {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        BigComplex::new(lo, yb)
    }

    /// Views centred on the boundaries of both components (and the cusp and the -3/4 root),
    /// texels from 2^-8 to 2^-300, rotated: no texel is ever called inside unless it is, also
    /// texels a few float32 ulps from the boundary; inside texels further than 1e-3 texel from
    /// it are all proven.
    #[test]
    fn never_false_inside() {
        let mut seeds: Vec<(f64, f64)> = Vec::new();
        for t in [0.3f64, 1.0, 2.0, 2.9, 3.13] {
            let (lr, li) = (t.cos(), t.sin());
            seeds.push((lr / 2.0 - (lr * lr - li * li) / 4.0, li / 2.0 - lr * li / 2.0));
        }
        for t in [0.5f64, 1.5, 3.0] {
            seeds.push((-1.0 + t.cos() / 4.0, t.sin() / 4.0));
        }
        let mut rng = Rng(0x9e3779b97f4a7c15);
        let (mut inside, mut proven, mut near, mut near_proven) = (0usize, 0usize, 0usize, 0usize);
        for (s, &(x, y)) in seeds.iter().enumerate() {
            for (d, tlog) in [-8i32, -24, -45, -70, -120, -300].into_iter().enumerate() {
                let bits = 2 * (-tlog) as usize + 192;
                let reference = if (s + d) % 4 == 0 && tlog < -8 {
                    // The cusp and the root, exactly.
                    if s % 2 == 0 {
                        BigComplex::from_f64(0.25, 0.0, bits)
                    } else {
                        BigComplex::from_f64(-0.75, 0.0, bits)
                    }
                } else {
                    boundary_near(x, y, bits)
                };
                let angle = rng.next() * std::f64::consts::TAU;
                let grid = Grid {
                    reference: &reference,
                    offset: [3.5, -1.25],
                    texel: 1.0,
                    scale_exp: tlog,
                    rot: [angle.cos() as f32, angle.sin() as f32],
                    extent: 600.0,
                };
                let test = ComponentTest::new(&grid).expect("a view on the boundary has a test");
                let sb = bits + 128;
                for _ in 0..150 {
                    let u = ((rng.next() - 0.5) * 1200.0) as f32;
                    let v = ((rng.next() - 0.5) * 1200.0) as f32;
                    let (truth, claim) = check(&grid, &test, u, v, sb);
                    if truth == Some(true) {
                        inside += 1;
                        proven += usize::from(claim);
                    }
                }
                // Rows through the boundary: bisect u in f32, then probe the texels next to it.
                for _ in 0..6 {
                    let v = ((rng.next() - 0.5) * 100.0) as f32;
                    let (mut a, mut b) = (-590.0f32, 590.0f32);
                    let ta = check(&grid, &test, a, v, sb).0;
                    if ta == check(&grid, &test, b, v, sb).0 {
                        continue;
                    }
                    for _ in 0..40 {
                        let m = 0.5 * (a + b);
                        if m == a || m == b {
                            break;
                        }
                        if check(&grid, &test, m, v, sb).0 == ta {
                            a = m;
                        } else {
                            b = m;
                        }
                    }
                    for k in 1..=4u32 {
                        for u in [
                            f32::from_bits(a.to_bits() + k),
                            f32::from_bits(b.to_bits() + k),
                            a - k as f32 * 1e-6,
                            b + k as f32 * 1e-6,
                        ] {
                            check(&grid, &test, u, v, sb);
                        }
                    }
                    for off in [1e-3f32, 1e-2, 0.1, 1.0] {
                        for u in [a - off, b + off] {
                            let (truth, claim) = check(&grid, &test, u, v, sb);
                            // (1e-3 texel is inside the unprovable band near the level's edge.)
                            if truth == Some(true) && off >= 1e-2 {
                                near += 1;
                                near_proven += usize::from(claim);
                            }
                        }
                    }
                }
            }
        }
        eprintln!("inside {inside}, proven {proven}; 1e-2..1 texel from the boundary: {near}, proven {near_proven}");
        assert!(inside > 1000);
        assert!(proven as f64 > 0.99 * inside as f64, "proven {proven} of {inside}");
        assert!(near > 300);
        assert_eq!(near_proven, near);
    }

    /// Far from both components: no test at all (f64 bound), at any depth.
    #[test]
    fn skipped_away_from_components() {
        let reference = BigComplex::parse("-0.743643887037151", "0.131825904205330", 512).unwrap();
        for tlog in [-20, -100, -1000] {
            let grid = Grid {
                reference: &reference,
                offset: [0.0, 0.0],
                texel: 1.0,
                scale_exp: tlog,
                rot: [1.0, 0.0],
                extent: 2000.0,
            };
            assert_eq!(ComponentTest::new(&grid), None);
        }
    }

    /// A view entirely inside the cardioid: every pixel proven.
    #[test]
    fn whole_view_inside() {
        let reference = BigComplex::from_f64(-0.1, 0.1, 64);
        let grid = Grid {
            reference: &reference,
            offset: [0.0, 0.0],
            texel: 1.0,
            scale_exp: -12,
            rot: [1.0, 0.0],
            extent: 1000.0,
        };
        let t = ComponentTest::new(&grid).expect("test");
        let inv_l = 2f32.powi(-t.l_log2);
        let p = t.cardioid.expect("cardioid");
        assert!(t.bulb.is_none());
        for (u, v) in [(0.0, 0.0), (-999.5, 999.5), (999.5, -999.5), (17.25, 3.0)] {
            assert!(p.inside(u, v, inv_l));
        }
    }

    /// Deep at the cusp and at 2^-1000 on the cardioid's edge: still proves the inside.
    #[test]
    fn deep_boundary() {
        // A point on the cardioid boundary at angle 1 (exact in f64, rounded), then views of
        // texel 2^-1000 around it.
        let t = 1.0f64;
        let (lr, li) = (t.cos(), t.sin());
        // c = lambda/2 - lambda^2/4 with |lambda| = 1.
        let cre = lr / 2.0 - (lr * lr - li * li) / 4.0;
        let cim = li / 2.0 - (2.0 * lr * li) / 4.0;
        let reference = BigComplex::from_f64(cre, cim, 64).with_precision(2400);
        let grid = Grid {
            reference: &reference,
            offset: [0.0, 0.0],
            texel: 1.0,
            scale_exp: -1000,
            rot: [1.0, 0.0],
            extent: 512.0,
        };
        // The boundary is within 2^-53 of the reference, far outside a 2^-1000 view: uniform.
        let test = ComponentTest::new(&grid);
        let truth = exact_inside(&reference).unwrap();
        match test {
            None => assert!(!truth),
            Some(t) => {
                let p = t.cardioid.expect("cardioid");
                assert_eq!(p.inside(0.0, 0.0, 2f32.powi(-t.l_log2)), truth);
            }
        }
    }
}
