//! The orbit of one point c, for the orbit overlay (kb: decisions/controls, "Orbit overlay";
//! techniques/coloring-methods, "Interior colouring as built"): its first iterates z_1 ... z_n
//! as view offsets (drawn as dots joined by lines) and as plane values (the mini-map), and
//! what becomes of it: escape at iteration n, an attracting cycle (period, multiplier) or
//! unresolved at the limit. While classifying, the probe records the orbit's plane values
//! ([`OrbitCloud`], up to 10^5 points, decimated beyond) for the whole-set map's dot cloud.
//!
//! Any depth: shallow views iterate c directly; deep views perturb against the renderer's
//! reference orbit (`z = Z_m + δ`, rebasing as the kernels do) in [`DoubleExpComplex`]
//! (f64 mantissa, i64 exponent), so view offsets of iterates that return near c stay exact.
//! The classification follows the GPU kernels: the derivative test `|dz/dz_1| n² < 1e-3`
//! every 8 iterations, the atom-domain index q (the smallest |z_n|), then the period as the
//! first divisor k of q at which the orbit closes with an attracting multiplier. It runs in
//! steps of a budget (the host spends at most ~1 ms of a frame on it), resumable.

use std::sync::Arc;

use almondbread_math::{DoubleExp, DoubleExpComplex, Formula};

use crate::reference::Reference;

/// Most iterates the overlay draws as a line (the main view; the map draws the first
/// [`MAP_LINE_POINTS`] of them).
pub const MAX_ORBIT_POINTS: usize = 1000;
/// Iterates the whole-set map joins by a line (the start of the orbit stays readable; the rest
/// is the dot cloud).
pub const MAP_LINE_POINTS: usize = 50;
/// Most points the whole-set map's dot cloud keeps ([`OrbitCloud`]): beyond it the cloud keeps
/// every 2nd, 4th, ... iterate.
pub const CLOUD_POINTS: usize = 100_000;
/// The probe follows the orbit at least this far before it calls the point undecided (slow
/// escapes near the necks take 10^4 to 10^5 steps; kb: decisions/controls "Orbit overlay").
pub const PROBE_MIN_STEPS: usize = 100_000;
/// Escape radius of the probe, squared: the kernels' (2^16 for Mandelbrot; `Formula::bailout`).
fn bailout2(formula: Formula) -> f64 {
    let b = formula.bailout();
    b * b
}
const INTERIOR_EPS: f64 = 1e-3;
const CLOSE_TAU2: f64 = 1e-6;

fn dx(re: f64, im: f64) -> DoubleExpComplex {
    DoubleExpComplex::new(DoubleExp::from_f64(re), DoubleExp::from_f64(im))
}

fn neg(z: DoubleExpComplex) -> DoubleExpComplex {
    DoubleExpComplex::new(-z.re, -z.im)
}

fn sub(a: DoubleExpComplex, b: DoubleExpComplex) -> DoubleExpComplex {
    a + neg(b)
}

fn to_f64(z: DoubleExpComplex) -> [f64; 2] {
    [z.re.to_f64(), z.im.to_f64()]
}

/// A reference orbit in the probe's number format (made once per reference).
#[derive(Clone, Debug)]
pub struct ProbeReference {
    /// Z_0 ... Z_{len-1}.
    pub points: Vec<DoubleExpComplex>,
}

impl ProbeReference {
    pub fn from_reference(r: &Reference) -> Self {
        let points = if r.extended.is_empty() {
            r.points.iter().map(|p| dx(f64::from(p[0]), f64::from(p[1]))).collect()
        } else {
            r.extended
                .iter()
                .map(|z| {
                    let [re, im] = z.mantissa;
                    if re == 0.0 && im == 0.0 {
                        DoubleExpComplex::ZERO
                    } else {
                        let e = i64::from(z.exponent);
                        DoubleExpComplex::new(
                            DoubleExp::from_f64(f64::from(re)).times_power_of_two(e),
                            DoubleExp::from_f64(f64::from(im)).times_power_of_two(e),
                        )
                    }
                })
                .collect()
        };
        Self { points }
    }

    fn z(&self, m: usize) -> DoubleExpComplex {
        self.points[m]
    }

    fn last(&self) -> usize {
        self.points.len() - 1
    }
}

/// Where c is, for the probe: relative to a reference orbit (deep views), or absolute.
#[derive(Clone, Debug)]
pub struct ProbePoint {
    /// The iteration (F2; the reference, if any, is an orbit of it).
    pub formula: Formula,
    /// The reference (None: iterate c directly).
    pub reference: Option<Arc<ProbeReference>>,
    /// c minus the reference point (absolute c without a reference).
    pub dc: DoubleExpComplex,
}

/// What becomes of the orbit.
#[derive(Clone, Debug, PartialEq)]
pub enum OrbitClass {
    /// Still analysing.
    Pending,
    /// |z_n| passed the escape radius at iteration n.
    Escaped { iteration: usize },
    /// An attracting cycle of this period and multiplier (re, im).
    Interior { period: usize, multiplier: [f64; 2] },
    /// The derivative test proved it interior, but no period was found.
    InteriorUnknown,
    /// Neither escaped nor proven interior by the limit.
    Unresolved { limit: usize },
}

/// A count with its thousands apart: 48213 -> "48 213".
pub fn grouped(n: usize) -> String {
    let d = n.to_string();
    let mut out = String::with_capacity(d.len() + d.len() / 3);
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

impl OrbitClass {
    /// What becomes of the point, in plain words ("escapes after 48 213 steps", "settles into
    /// a period-34 cycle", "still undecided after 100 000 steps").
    pub fn text(&self) -> String {
        let steps = |n: usize| {
            if n == 1 {
                "1 step".to_string()
            } else {
                format!("{} steps", grouped(n))
            }
        };
        match self {
            Self::Pending => "following the orbit ...".into(),
            Self::Escaped { iteration } => format!("escapes after {}", steps(*iteration)),
            Self::Interior { period: 1, .. } => "settles on a fixed point".into(),
            Self::Interior { period, .. } => format!("settles into a period-{period} cycle"),
            Self::InteriorUnknown => "settles into a cycle (period not found)".into(),
            Self::Unresolved { limit } => format!("still undecided after {}", steps(*limit)),
        }
    }
}

/// The orbit's values in the plane for the whole-set map's dot cloud: z_1, z_2, ... while they
/// fit in `cap` points; then every 2nd (the kept points are halved and the stride doubles),
/// every 4th, ... so the cloud covers the whole orbit so far within a fixed budget. Point k is
/// iterate `1 + k * stride`.
#[derive(Clone, Debug, PartialEq)]
pub struct OrbitCloud {
    pub points: Vec<[f32; 2]>,
    pub stride: usize,
    /// Iterates offered so far.
    pub seen: usize,
    cap: usize,
}

impl OrbitCloud {
    /// `cap` is rounded up to an even number (at least 2).
    pub fn new(cap: usize) -> Self {
        Self {
            points: Vec::new(),
            stride: 1,
            seen: 0,
            cap: cap.max(2).next_multiple_of(2),
        }
    }

    /// Offers the next iterate.
    pub fn push(&mut self, z: [f64; 2]) {
        let n = self.seen;
        self.seen += 1;
        if !n.is_multiple_of(self.stride) {
            return;
        }
        if self.points.len() >= self.cap {
            let mut i = 0usize;
            self.points.retain(|_| {
                i += 1;
                i % 2 == 1
            });
            self.stride *= 2;
            if !n.is_multiple_of(self.stride) {
                return;
            }
        }
        let f = |v: f64| {
            if v.is_finite() {
                v.clamp(-1e30, 1e30) as f32
            } else {
                1e30f32.copysign(v as f32)
            }
        };
        self.points.push([f(z[0]), f(z[1])]);
    }

    /// The iterate index of point `k`.
    pub fn iterate(&self, k: usize) -> usize {
        1 + k * self.stride
    }
}

/// The first iterates z_1 ... z_n (fewer when it escapes first).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrbitPoints {
    /// Offsets from the view centre in view points (x right, y up); f64 beyond 1e300 is
    /// clamped (such points are far off screen anyway).
    pub view: Vec<[f64; 2]>,
    /// The values in the plane (f64; the mini-map draws them at full-set scale).
    pub plane: Vec<[f64; 2]>,
    /// |z_n| of the last iterate.
    pub last_abs: f64,
}

/// The first `count` iterates of c. `ref_offset`: the reference point minus the view centre
/// (zero without a reference, where `view_center` is subtracted instead), `scale`: complex
/// units per view point.
pub fn orbit_points(
    p: &ProbePoint,
    count: usize,
    view_center: [f64; 2],
    ref_offset: DoubleExpComplex,
    scale: DoubleExp,
) -> OrbitPoints {
    let count = count.min(MAX_ORBIT_POINTS);
    let mut out = OrbitPoints::default();
    if scale.mantissa.is_nan() || scale.mantissa <= 0.0 {
        return out;
    }
    let inv = DoubleExp::ONE / scale;
    let centre = dx(view_center[0], view_center[1]);
    let clamp = |x: f64| {
        if x.is_finite() {
            x.clamp(-1e300, 1e300)
        } else {
            1e300f64.copysign(x)
        }
    };
    let mut it = OrbitProbe::new(p.clone(), usize::MAX);
    for n in 1..=count {
        let (z, zm) = it.full_z();
        let rel = match &p.reference {
            // (Z_m - Z_1) + (C_ref - C_view) + delta: exact where m = 1, which is where the
            // orbit comes back near c after a rebase.
            Some(r) => {
                let d = if it.m == 1 {
                    it.delta
                } else {
                    sub(zm, r.z(1)) + it.delta
                };
                d + ref_offset
            }
            None => sub(z, centre),
        };
        out.view
            .push([clamp((rel.re * inv).to_f64()), clamp((rel.im * inv).to_f64())]);
        let zf = to_f64(z);
        out.plane.push([clamp(zf[0]), clamp(zf[1])]);
        out.last_abs = z.magnitude().to_f64();
        if z.magnitude_squared().to_f64() > bailout2(p.formula) || n == count {
            break;
        }
        it.advance(zm);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Iterating: escape, derivative test, atom domain.
    Main,
    /// Searching the cycle's period from a settled point.
    Cycle,
    Done,
}

/// The classification of one c, advanced in budgeted steps.
#[derive(Clone, Debug)]
pub struct OrbitProbe {
    point: ProbePoint,
    max_iter: usize,
    phase: Phase,
    m: usize,
    n: usize,
    delta: DoubleExpComplex,
    w: DoubleExpComplex,
    atom_min: f64,
    q: usize,
    // Cycle search
    zs: DoubleExpComplex,
    lam: DoubleExpComplex,
    k: usize,
    tries: usize,
    pub class: OrbitClass,
    /// The orbit so far in the plane (the whole-set map's dots).
    pub cloud: OrbitCloud,
}

impl OrbitProbe {
    pub fn new(point: ProbePoint, max_iter: usize) -> Self {
        let delta = point.dc;
        Self {
            point,
            max_iter: max_iter.max(1),
            phase: Phase::Main,
            m: 1,
            n: 1,
            delta,
            w: DoubleExpComplex::ONE,
            atom_min: f64::INFINITY,
            q: 1,
            zs: DoubleExpComplex::ZERO,
            lam: DoubleExpComplex::ONE,
            k: 0,
            tries: 0,
            class: OrbitClass::Pending,
            cloud: OrbitCloud::new(CLOUD_POINTS),
        }
    }

    pub fn done(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Iterations done so far.
    pub fn iterations(&self) -> usize {
        self.n
    }

    /// The overlay's line: the class, or while analysing how far it got (to two digits, so
    /// the text, and with it the overlay scene, changes only every ~10 %).
    pub fn text(&self) -> String {
        match self.class {
            OrbitClass::Pending if self.n > 1 => {
                let unit = 10usize.pow(self.n.ilog10().saturating_sub(1));
                format!("following the orbit: {} steps so far", grouped(self.n / unit * unit))
            }
            _ => self.class.text(),
        }
    }

    /// The full z of the current state (and rebases the delta when due).
    fn full_z(&mut self) -> (DoubleExpComplex, DoubleExpComplex) {
        match &self.point.reference {
            Some(r) => {
                let mut zm = r.z(self.m);
                let z = zm + self.delta;
                if self.m == r.last() || (self.m != 0 && z.magnitude_squared() < self.delta.magnitude_squared()) {
                    self.delta = z;
                    self.m = 0;
                    zm = DoubleExpComplex::ZERO;
                }
                (z, zm)
            }
            None => (self.delta, DoubleExpComplex::ZERO),
        }
    }

    /// `f'(z)`: the multiplier of the derivatives. (F5 Mandelbar: its conformal stand-in `2z`;
    /// every step's Jacobian is a scaled reflection, so the product's modulus, all the interior
    /// test and the cycle's |λ| read, is exact; its argument is not.)
    fn prime(&self, z: DoubleExpComplex) -> DoubleExpComplex {
        match self.point.formula {
            Formula::Mandelbrot => z.times_two(),
            f => f.prime_x(z),
        }
    }

    fn advance(&mut self, zm: DoubleExpComplex) {
        let f = self.point.formula;
        self.delta = match (self.point.reference.is_some(), f) {
            (true, Formula::Mandelbrot) => (zm.times_two() + self.delta) * self.delta + self.point.dc,
            (false, Formula::Mandelbrot) => self.delta * self.delta + self.point.dc,
            (true, _) => f.delta_step_x(zm, self.delta, self.point.dc),
            (false, _) => f.step_x(self.delta, self.point.dc),
        };
        self.m += 1;
        self.n += 1;
    }

    /// Runs at most `budget` iterations; returns whether the class is final.
    pub fn step(&mut self, budget: usize) -> bool {
        for _ in 0..budget {
            match self.phase {
                Phase::Done => return true,
                Phase::Main => {
                    let (z, zm) = self.full_z();
                    let r2 = z.magnitude_squared();
                    if r2.to_f64() > bailout2(self.point.formula) {
                        self.class = OrbitClass::Escaped { iteration: self.n };
                        self.phase = Phase::Done;
                        return true;
                    }
                    let l = 0.5 * r2.log2_magnitude();
                    if l < self.atom_min {
                        self.atom_min = l;
                        self.q = self.n;
                    }
                    if self.n >= self.max_iter {
                        self.class = OrbitClass::Unresolved { limit: self.max_iter };
                        self.phase = Phase::Done;
                        return true;
                    }
                    self.w = self.prime(z) * self.w;
                    if self.n.is_multiple_of(8) {
                        let a = self.w.re.abs().to_f64().max(self.w.im.abs().to_f64());
                        let nn = self.n as f64;
                        if a * nn * nn < INTERIOR_EPS {
                            self.phase = Phase::Cycle;
                            self.k = 0;
                            continue;
                        }
                    }
                    self.cloud.push(to_f64(z));
                    self.advance(zm);
                }
                Phase::Cycle => {
                    let (z, zm) = self.full_z();
                    if z.magnitude_squared().to_f64() > bailout2(self.point.formula) {
                        self.class = OrbitClass::InteriorUnknown;
                        self.phase = Phase::Done;
                        return true;
                    }
                    if self.k == 0 {
                        self.zs = z;
                        self.lam = DoubleExpComplex::ONE;
                    } else if self.q.is_multiple_of(self.k) {
                        let d = sub(z, self.zs).magnitude_squared();
                        let s = self.zs.magnitude_squared();
                        let close = d.is_zero()
                            || (!s.is_zero() && d.log2_magnitude() - s.log2_magnitude() <= CLOSE_TAU2.log2());
                        if close && self.lam.magnitude_squared().to_f64() < 1.0 {
                            self.class = OrbitClass::Interior {
                                period: self.k,
                                multiplier: to_f64(self.lam),
                            };
                            self.phase = Phase::Done;
                            return true;
                        }
                    }
                    self.lam = self.prime(z) * self.lam;
                    self.cloud.push(to_f64(z));
                    self.advance(zm);
                    self.k += 1;
                    if self.k > self.q {
                        self.tries += 1;
                        if self.tries < 127 && self.tries * self.q < self.q.max(4096) {
                            self.k = 0;
                        } else {
                            self.class = OrbitClass::InteriorUnknown;
                            self.phase = Phase::Done;
                            return true;
                        }
                    }
                }
            }
        }
        self.done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(re: f64, im: f64) -> OrbitClass {
        let mut p = OrbitProbe::new(
            ProbePoint {
                formula: Formula::Mandelbrot,
                reference: None,
                dc: dx(re, im),
            },
            100_000,
        );
        while !p.step(10_000) {}
        p.class
    }

    #[test]
    fn classes() {
        // (escape radius 2^16, as in the kernels)
        assert_eq!(probe(1.0, 1.0), OrbitClass::Escaped { iteration: 6 });
        match probe(-0.1, 0.1) {
            OrbitClass::Interior { period, multiplier } => {
                assert_eq!(period, 1);
                // lambda = 1 - sqrt(1 - 4c) for the fixed point
                assert!((multiplier[0].hypot(multiplier[1]) - 0.257).abs() < 2e-3);
            }
            c => panic!("{c:?}"),
        }
        match probe(-1.0, 0.2) {
            OrbitClass::Interior { period, multiplier } => {
                assert_eq!(period, 2);
                // lambda = 4 (c + 1) = 0.8 i
                assert!(multiplier[0].abs() < 1e-3 && (multiplier[1] - 0.8).abs() < 1e-3);
            }
            c => panic!("{c:?}"),
        }
        match probe(-0.1226, 0.7449) {
            OrbitClass::Interior { period, .. } => assert_eq!(period, 3),
            c => panic!("{c:?}"),
        }
    }

    #[test]
    fn plain_texts() {
        assert_eq!(grouped(7), "7");
        assert_eq!(grouped(100_000), "100 000");
        assert_eq!(grouped(48_213), "48 213");
        assert_eq!(grouped(1_234_567), "1 234 567");
        assert_eq!(
            OrbitClass::Escaped { iteration: 48_213 }.text(),
            "escapes after 48 213 steps"
        );
        assert_eq!(OrbitClass::Escaped { iteration: 1 }.text(), "escapes after 1 step");
        let cycle = |period| OrbitClass::Interior {
            period,
            multiplier: [0.5, 0.0],
        };
        assert_eq!(cycle(34).text(), "settles into a period-34 cycle");
        assert_eq!(cycle(1).text(), "settles on a fixed point");
        assert_eq!(
            OrbitClass::Unresolved { limit: 100_000 }.text(),
            "still undecided after 100 000 steps"
        );
        // (c = 0.3 escapes after ~ 30 steps; c = -0.75 + 1e-3 i takes thousands.)
        let mut p = OrbitProbe::new(
            ProbePoint {
                formula: Formula::Mandelbrot,
                reference: None,
                dc: dx(-0.75, 1e-4),
            },
            PROBE_MIN_STEPS,
        );
        p.step(12_000);
        assert_eq!(p.iterations(), 12_001);
        assert_eq!(p.text(), "following the orbit: 12 000 steps so far");
    }

    #[test]
    fn cloud_decimates_by_doubling_the_stride() {
        let mut c = OrbitCloud::new(8);
        for n in 1..=8 {
            c.push([n as f64, 0.0]);
        }
        assert_eq!((c.points.len(), c.stride), (8, 1));
        // The 9th halves the cloud (iterates 1, 3, 5, 7) and is itself kept (9 = 1 + 4 * 2).
        c.push([9.0, 0.0]);
        assert_eq!(c.stride, 2);
        let xs: Vec<f32> = c.points.iter().map(|p| p[0]).collect();
        assert_eq!(xs, vec![1.0, 3.0, 5.0, 7.0, 9.0]);
        for n in 10..=100 {
            c.push([n as f64, 0.0]);
        }
        // Bounded, evenly spread, and point k is iterate 1 + k * stride.
        assert!(c.points.len() <= 8 && c.points.len() > 4, "{}", c.points.len());
        assert_eq!(c.seen, 100);
        for (k, p) in c.points.iter().enumerate() {
            assert_eq!(p[0] as usize, c.iterate(k));
        }
        assert!(c.iterate(c.points.len() - 1) + c.stride > 100);
        // Escaping values stay finite.
        let mut e = OrbitCloud::new(4);
        e.push([f64::INFINITY, -1e300]);
        assert_eq!(e.points[0], [1e30, -1e30]);
    }

    #[test]
    fn probe_collects_a_slow_escape() {
        // Just outside the cusp at 1/4: escapes after ~ pi / sqrt(eps) steps.
        let mut p = OrbitProbe::new(
            ProbePoint {
                formula: Formula::Mandelbrot,
                reference: None,
                dc: dx(0.25 + 1e-6, 0.0),
            },
            PROBE_MIN_STEPS,
        );
        while !p.step(50_000) {}
        let OrbitClass::Escaped { iteration } = p.class else {
            panic!("{:?}", p.class)
        };
        assert!(iteration > 3000, "{iteration}");
        // Every iterate before the escape is in the cloud (under the cap: stride 1).
        assert_eq!(p.cloud.stride, 1);
        assert_eq!(p.cloud.points.len(), iteration - 1);
        assert_eq!(p.cloud.points[0], [(0.25 + 1e-6) as f32, 0.0]);
    }

    #[test]
    fn points_direct() {
        let p = ProbePoint {
            formula: Formula::Mandelbrot,
            reference: None,
            dc: dx(-1.0, 0.0),
        };
        let o = orbit_points(&p, 4, [-1.0, 0.0], DoubleExpComplex::ZERO, DoubleExp::from_f64(0.5));
        // z: -1, 0, -1, 0 ; offsets from -1 in units of 0.5: 0, 2, 0, 2
        assert_eq!(o.plane, vec![[-1.0, 0.0], [0.0, 0.0], [-1.0, 0.0], [0.0, 0.0]]);
        assert_eq!(o.view, vec![[0.0, 0.0], [2.0, 0.0], [0.0, 0.0], [2.0, 0.0]]);
    }
}
