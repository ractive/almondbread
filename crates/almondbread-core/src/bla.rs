//! Bivariate linear approximation (BLA) skip table (kb: techniques/bla).
//!
//! While the delta `z` to the reference orbit is small against the reference `Z`, `l`
//! perturbation steps `z -> 2 Z_k z + z^2 + c` compose into one affine map
//! `z_{m+l} = A z_m + B c`. The table holds such maps for aligned blocks of the reference: level
//! `lev` has one entry per block of `2^lev` steps starting at `m = 1 + j 2^lev` (iteration 0 is
//! the critical point, never linear), merged pairwise from the level below.
//!
//! **Validity, per pixel `c` (our formulation, kb: techniques/bla "As built").** Dropping `z^2`
//! at inner step `k` costs no more than float32 rounding of the linear term when
//! `|z_k| <= eps |2 Z_k|` (eps = 2^-24: the "corrected" radius of the kb page, `R = eps |A|` for a
//! single step). Inside a block `z_k = A_k z + B_k c` (`A_k`, `B_k` the partial maps), so the
//! block is valid iff `|A_k| |z| + |B_k| |c| <= eps |2 Z_k|` for every inner `k`, i.e. iff
//! `|z| <= min_k (r_k - |B_k| |c|) / |A_k|`. That is a minimum of affine functions of `|c|`;
//! each entry stores a conservative affine bound of it,
//!
//! ```text
//! valid  <=>  |z| + K |c| < R,   R = min_k r_k / |A_k|,   K = max_k |B_k| / |A_k|
//! single step:   A = 2 Z_m, B = 1, R = eps |2 Z_m|, K = 0
//! x then y:      A = A_y A_x, B = A_y B_x + B_y,
//!                R = min(R_x, R_y / |A_x|),  K = max(K_x, (K_y + |B_x|) / |A_x|)
//! ```
//!
//! Unlike mathr's radius with the image's `max|c|` baked in, `c` is the pixel's own delta at
//! lookup time, so one table serves every framing its reference serves (reference reuse while
//! panning and zooming) and is never rebuilt for a new view. Radii shrink and `K` grows with
//! the level, so validity is monotone: a lookup tests levels upwards and keeps the last valid.
//!
//! **Other formulas (F2).** For `z^p + c` a single step has `A = f'(Z) = p Z^{p-1}`, `B = 1`,
//! and the dropped term is `C(p,2) Z^{p-2} z^2`, so the step is linear to `eps` while
//! `|z| <= eps 2 |Z| / (p - 1)` (`eps |2Z|` at p = 2; kb: formulas/multibrot "BLA form").
//! [`BlaTable::build_for`] and [`BlaTableX::build_for`] take the formula; merging is the same.
//!
//! **Julia sets (F4).** A Julia step `z' = 2Zz + z^2` has no `c`: `B = 0` (so `K = 0`, and
//! every block's validity is `|z| < R` for any pixel), built through the same hooks on the
//! concatenated reference of [`crate::julia_reference`], whose zeros between the two orbits
//! invalidate every block that would straddle them.
//!
//! **Non-conformal formulas (F5, Mandelbar; kb: plans/f5-mandelbar, formulas/mandelbar).** The
//! step's Jacobian is a real-linear map, so `A` and `B` are too ([`Linear`]: `δ ↦ p δ +
//! q conj(δ)`, a real 2x2 matrix in Wirtinger form). With `sup|M|`, `inf|M|` its singular values:
//!
//! ```text
//! single step:   A = J_f(Z_m), B = I, R = eps inf|A|, K = 0
//! x then y:      A = A_y A_x, B = A_y B_x + B_y,
//!                R = min(R_x, R_y / sup|A_x|),  K = max(K_x, (K_y + sup|B_x|) / sup|A_x|)
//! ```
//!
//! (the conformal rule with `|.|` read as the operator norm, the inner bound `|A_k z + B_k c| <=
//! sup|A_k| |z| + sup|B_k| |c|`; the single step drops `conj(z)^2` against `inf|A| |z|`, src:
//! mathr's deep-zoom page §4.1, kb: sources/mathr-deep-zoom-page). Mandelbar's `A = (0,
//! conj(2Z))` is a scaled reflection, `inf = sup = 2|Z|`, so its radii equal Mandelbrot's. The
//! tables store these entries in `matrix` ([`BlaEntryM`], [`BlaEntryMX`]) instead of `entries`;
//! levels, offsets and lookup are shared. F6's fold caps (`|X|`, `|Y|`) go into the single-step
//! radius ([`single_radius`]).
//!
//! Derivatives compose through a block: `dz/dc -> A dz/dc + B` (distance estimate) and
//! `dz/dz_1 -> A dz/dz_1` (interior test), each to the same relative error `eps` per step.
//!
//! **When it pays (M4 measurements, kb: synthesis/benchmarks).** A block only exists while
//! `|dz| < eps |2Z|`; at the depths the f64 reference reaches (>= ~1e-14 per pixel) deltas
//! start at ~1e-11 and leave that range after a few hundred iterations, blocks between the
//! reference's near-returns to 0 are 2-8 steps long, and a GPU lookup costs more than it
//! skips. [`BlaTable::forecast`] estimates the skip for a view; the renderer only uses the
//! table where it promises long blocks (deep views: 1e-20 per pixel 2.4x, 1e-24 47x faster).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use almondbread_math::{FloatExpComplex, Formula, Linear};

/// float32 hardware precision: dropped terms stay at the rounding level of the step.
pub const BLA_EPSILON: f64 = 1.0 / 16_777_216.0;
/// Largest `|A|` an entry may have: the kernel multiplies its interior derivative (rescaled
/// below 2^60) by `A` before renormalising, which must stay inside float32 range (2^128).
pub const MAX_COEFFICIENT: f64 = 1_099_511_627_776.0; // 2^40
/// Levels below this are not stored: a 1-step BLA costs as much as the exact step.
pub const MIN_LEVEL: u32 = 1;
/// Upper bound on levels (blocks of up to 2^31 steps).
pub const MAX_LEVELS: usize = 32;

/// One table entry as the GPU reads it: `A` (complex), `B` (complex), `R`, `K` (float32).
pub type BlaEntry = [f32; 6];

/// One entry of a non-conformal table (F5, `BlaEntryM` in MSL): `A = (ap, aq)`, `B = (bp, bq)`
/// in Wirtinger form ([`Linear`]), `R`, `K`: `[ap.re, ap.im, aq.re, aq.im, bp.re, bp.im, bq.re,
/// bq.im, r, k]`.
pub type BlaEntryM = [f32; 10];

/// The single-step validity radius of a non-conformal formula at `Z` (F5): `eps inf|A|`, the
/// dropped quadratic term (`conj(z)^2` for Mandelbar) against the linear one. F6 adds its fold
/// caps here.
pub fn single_radius(formula: Formula, z: (f64, f64), eps: f64) -> f64 {
    eps * formula.jacobian(z).inf()
}

#[derive(Clone, Copy, Debug)]
struct MergedM {
    a: Linear,
    b: Linear,
    r: f64,
    k: f64,
}

const INVALID_M: MergedM = MergedM {
    a: Linear::ZERO,
    b: Linear::ZERO,
    r: 0.0,
    k: 0.0,
};

impl MergedM {
    fn single(z: [f32; 2], eps: f64, formula: Formula) -> Self {
        let zd = (f64::from(z[0]), f64::from(z[1]));
        MergedM {
            a: formula.jacobian(zd),
            b: Linear::IDENTITY,
            r: single_radius(formula, zd, eps),
            k: 0.0,
        }
    }

    fn then(&self, y: &MergedM) -> MergedM {
        let ax = self.a.sup();
        if self.r <= 0.0 || y.r <= 0.0 || ax <= 0.0 {
            return INVALID_M;
        }
        let a = self.a.then(y.a);
        let b = self.b.then(y.a).plus(y.b);
        let r = self.r.min(y.r / ax);
        let k = self.k.max((y.k + self.b.sup()) / ax);
        if !(a.sup() <= MAX_COEFFICIENT && b.sup().is_finite() && k.is_finite() && r.is_finite()) {
            return INVALID_M;
        }
        MergedM { a, b, r, k }
    }

    fn entry(&self) -> BlaEntryM {
        let r = self.r as f32;
        let k = self.k as f32;
        if !(r >= f32::MIN_POSITIVE && k.is_finite()) {
            return [0.0; 10];
        }
        let f = |v: f64| v as f32;
        let (a, b) = (self.a, self.b);
        [
            f(a.p.0),
            f(a.p.1),
            f(a.q.0),
            f(a.q.1),
            f(b.p.0),
            f(b.p.1),
            f(b.q.0),
            f(b.q.1),
            r,
            k,
        ]
    }
}

/// The `A` and `B` of a matrix entry.
fn entry_maps(e: &BlaEntryM) -> (Linear, Linear) {
    let d = |i: usize| (f64::from(e[i]), f64::from(e[i + 1]));
    (Linear { p: d(0), q: d(2) }, Linear { p: d(4), q: d(6) })
}

#[derive(Clone, Copy, Debug)]
struct Merged {
    a: (f64, f64),
    b: (f64, f64),
    r: f64,
    k: f64,
}

fn cmul(x: (f64, f64), y: (f64, f64)) -> (f64, f64) {
    (x.0 * y.0 - x.1 * y.1, x.0 * y.1 + x.1 * y.0)
}

fn abs(x: (f64, f64)) -> f64 {
    x.0.hypot(x.1)
}

const INVALID: Merged = Merged {
    a: (0.0, 0.0),
    b: (0.0, 0.0),
    r: 0.0,
    k: 0.0,
};

impl Merged {
    fn single(z: [f32; 2], eps: f64) -> Self {
        let a = (2.0 * f64::from(z[0]), 2.0 * f64::from(z[1]));
        Merged {
            a,
            b: (1.0, 0.0),
            r: eps * abs(a),
            k: 0.0,
        }
    }

    /// A single step of another formula (F2): `A = f'(Z)`, `R = eps 2 |Z| / (p - 1)`.
    fn single_for(z: [f32; 2], eps: f64, formula: Formula) -> Self {
        let zd = (f64::from(z[0]), f64::from(z[1]));
        let p = f64::from(formula.power());
        Merged {
            a: formula.prime(zd),
            // (A Julia step has no `+c`: B = 0, so K = 0 and the block's radius does not
            // depend on the pixel; F4, kb: formulas/julia.)
            b: if formula.is_julia() { (0.0, 0.0) } else { (1.0, 0.0) },
            r: eps * 2.0 * abs(zd) / (p - 1.0),
            k: 0.0,
        }
    }

    /// `self` (first) followed by `y`.
    fn then(&self, y: &Merged) -> Merged {
        let ax = abs(self.a);
        if self.r <= 0.0 || y.r <= 0.0 || ax <= 0.0 {
            return INVALID;
        }
        let a = cmul(y.a, self.a);
        let ab = cmul(y.a, self.b);
        let b = (ab.0 + y.b.0, ab.1 + y.b.1);
        let r = self.r.min(y.r / ax);
        let k = self.k.max((y.k + abs(self.b)) / ax);
        if !(abs(a) <= MAX_COEFFICIENT && abs(b).is_finite() && k.is_finite() && r.is_finite()) {
            return INVALID;
        }
        Merged { a, b, r, k }
    }

    fn entry(&self) -> BlaEntry {
        let r = self.r as f32;
        let k = self.k as f32;
        // Not representable as a usable float32 bound: never valid.
        if !(r >= f32::MIN_POSITIVE && k.is_finite()) {
            return [0.0; 6];
        }
        [self.a.0 as f32, self.a.1 as f32, self.b.0 as f32, self.b.1 as f32, r, k]
    }
}

/// The BLA table of one reference orbit.
#[derive(Clone, Debug, Default)]
pub struct BlaTable {
    /// Number of reference points (`last = ref_len - 1`).
    pub ref_len: usize,
    pub epsilon: f64,
    /// Lowest stored level (blocks of `2^min_level` steps).
    pub min_level: u32,
    /// Highest stored level (inclusive); `max_level < min_level`: an empty table.
    pub max_level: u32,
    /// Index of each level's first entry in `entries` (levels below `min_level`: 0).
    pub offsets: [u32; MAX_LEVELS],
    pub entries: Vec<BlaEntry>,
    /// A non-conformal formula's entries (F5; then `entries` is empty), same layout.
    pub matrix: Vec<BlaEntryM>,
    /// Seconds the build took.
    pub build_time: f64,
}

impl BlaTable {
    /// Builds the table from the reference points the GPU iterates against (float32 pairs,
    /// so the coefficients describe exactly the orbit the kernel sees), in f64.
    pub fn build(points: &[[f32; 2]], epsilon: f64) -> Self {
        Self::build_for(points, epsilon, Formula::Mandelbrot)
    }

    /// [`BlaTable::build`] for the orbit of any formula (F2: its single steps).
    pub fn build_for(points: &[[f32; 2]], epsilon: f64, formula: Formula) -> Self {
        Self::build_cancellable(points, epsilon, formula, &AtomicBool::new(false)).expect("never cancelled")
    }

    /// [`BlaTable::build_for`] that polls `cancel` before each level (the renderer cancels the
    /// build of a reference it replaced); None when cancelled.
    pub fn build_cancellable(points: &[[f32; 2]], epsilon: f64, formula: Formula, cancel: &AtomicBool) -> Option<Self> {
        let start = Instant::now();
        let mut t = BlaTable {
            ref_len: points.len(),
            epsilon,
            min_level: MIN_LEVEL,
            max_level: 0,
            ..Default::default()
        };
        // Single steps start at m = 1 ..= last - 1.
        let singles = points.len().saturating_sub(2);
        if singles < (1 << MIN_LEVEL) {
            t.build_time = start.elapsed().as_secs_f64();
            return Some(t);
        }
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        if !formula.is_conformal() {
            let mut level: Vec<MergedM> = points[1..1 + singles]
                .iter()
                .map(|&z| MergedM::single(z, epsilon, formula))
                .collect();
            let mut lev = 0u32;
            let mut matrix: Vec<BlaEntryM> = Vec::with_capacity(singles);
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }
                if lev >= MIN_LEVEL {
                    t.offsets[lev as usize] = matrix.len() as u32;
                    matrix.extend(level.iter().map(MergedM::entry));
                    t.max_level = lev;
                }
                if level.len() < 2 || lev as usize + 1 >= MAX_LEVELS {
                    break;
                }
                level = level.as_chunks::<2>().0.iter().map(|p| p[0].then(&p[1])).collect();
                lev += 1;
            }
            t.matrix = matrix;
            t.build_time = start.elapsed().as_secs_f64();
            return Some(t);
        }
        let steps = points[1..1 + singles].iter();
        let mut level: Vec<Merged> = if formula.is_mandelbrot() {
            steps.map(|&z| Merged::single(z, epsilon)).collect()
        } else {
            steps.map(|&z| Merged::single_for(z, epsilon, formula)).collect()
        };
        let mut lev = 0u32;
        let mut entries: Vec<BlaEntry> = Vec::with_capacity(singles);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            if lev >= MIN_LEVEL {
                t.offsets[lev as usize] = entries.len() as u32;
                entries.extend(level.iter().map(Merged::entry));
                t.max_level = lev;
            }
            if level.len() < 2 || lev as usize + 1 >= MAX_LEVELS {
                break;
            }
            level = level.as_chunks::<2>().0.iter().map(|p| p[0].then(&p[1])).collect();
            lev += 1;
        }
        t.entries = entries;
        t.build_time = start.elapsed().as_secs_f64();
        Some(t)
    }

    /// The entry of `level` starting at reference index `m`, if one exists.
    pub fn lookup(&self, m: usize, level: u32) -> Option<&BlaEntry> {
        if level < self.min_level || level > self.max_level || m == 0 {
            return None;
        }
        let len = 1usize << level;
        if !(m - 1).is_multiple_of(len) || m + len > self.ref_len - 1 {
            return None;
        }
        self.entries
            .get(self.offsets[level as usize] as usize + ((m - 1) >> level))
    }

    /// The matrix entry of `level` at `m` (F5, non-conformal tables), if one exists.
    pub fn lookup_m(&self, m: usize, level: u32) -> Option<&BlaEntryM> {
        if level < self.min_level || level > self.max_level || m == 0 {
            return None;
        }
        let len = 1usize << level;
        if !(m - 1).is_multiple_of(len) || m + len > self.ref_len - 1 {
            return None;
        }
        self.matrix
            .get(self.offsets[level as usize] as usize + ((m - 1) >> level))
    }

    /// Whether the table holds matrix entries (a non-conformal formula, F5).
    pub fn is_matrix(&self) -> bool {
        !self.matrix.is_empty()
    }

    /// Entries (either kind).
    pub fn len(&self) -> usize {
        self.entries.len() + self.matrix.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The GPU's bytes (either kind).
    pub fn data(&self) -> &[u8] {
        if self.is_matrix() {
            bytemuck::cast_slice(&self.matrix)
        } else {
            bytemuck::cast_slice(&self.entries)
        }
    }

    pub fn bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<BlaEntry>() + self.matrix.len() * std::mem::size_of::<BlaEntryM>()
    }

    /// [`BlaTable::longest`] of a matrix table (F5): the entry and its length.
    pub fn longest_m(&self, m: usize, n: usize, max_iter: usize, zn: f64, cn: f64) -> Option<(&BlaEntryM, usize)> {
        if m == 0 {
            return None;
        }
        let mut best = None;
        for lev in self.min_level..=self.max_level {
            let len = 1usize << lev;
            if n + len > max_iter {
                break;
            }
            let Some(e) = self.lookup_m(m, lev) else { break };
            if zn + f64::from(e[9]) * cn >= f64::from(e[8]) {
                break;
            }
            best = Some((e, len));
        }
        best
    }

    /// The longest valid block at reference index `m` for a pixel at iteration `n` with
    /// `|dz| = zn` and `|dc| = cn` (the kernel's lookup): the entry and its length.
    pub fn longest(&self, m: usize, n: usize, max_iter: usize, zn: f64, cn: f64) -> Option<(&BlaEntry, usize)> {
        if m == 0 {
            return None;
        }
        let mut best = None;
        for lev in self.min_level..=self.max_level {
            let len = 1usize << lev;
            if n + len > max_iter {
                break;
            }
            let Some(e) = self.lookup(m, lev) else { break };
            if zn + f64::from(e[5]) * cn >= f64::from(e[4]) {
                break;
            }
            best = Some((e, len));
        }
        best
    }

    /// Forecast for a view whose pixels lie within `c_abs` of the reference: a pixel at that
    /// distance, from iteration 1 (`z_1 = dc`), takes the longest valid block while one is
    /// valid, with `|dz|` bounded by `|A| |dz| + |B| |dc|` (no direction assumed). Returns the
    /// iterations covered and the blocks taken. Blocks are as costly as a few exact steps on
    /// the GPU (dependent table loads, SIMD divergence), so BLA only pays when this covers
    /// long runs (kb: techniques/bla "As built").
    pub fn forecast(&self, c_abs: f64, max_iter: usize) -> BlaForecast {
        let (mut m, mut zn, mut covered, mut blocks) = (1usize, c_abs, 0usize, 0usize);
        if self.is_matrix() {
            while let Some((e, len)) = self.longest_m(m, 1 + covered, max_iter, zn, c_abs) {
                let (a, b) = entry_maps(e);
                zn = a.sup() * zn + b.sup() * c_abs;
                m += len;
                covered += len;
                blocks += 1;
            }
            return BlaForecast { covered, blocks };
        }
        while let Some((e, len)) = self.longest(m, 1 + covered, max_iter, zn, c_abs) {
            zn = f64::from(e[0]).hypot(f64::from(e[1])) * zn + f64::from(e[2]).hypot(f64::from(e[3])) * c_abs;
            m += len;
            covered += len;
            blocks += 1;
        }
        BlaForecast { covered, blocks }
    }
}

// ---------------------------------------------------------------------------------------------
// M3b: the same table in extended range (kb: techniques/bla "M3b", techniques/extended-range-floats).

/// One deep-table entry as the GPU reads it (`BlaEntryX` in MSL, 40 bytes): `A = a 2^ae`,
/// `B = b 2^be` (complex mantissas with max-abs in [0.5, 1)), `R = r 2^re`, `K = k 2^ke`
/// (`r, k` in [0.5, 1); `r = 0`: never valid; `k = 0`: no `|c|` term).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlaEntryX {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub ae: i32,
    pub be: i32,
    pub r: f32,
    pub re: i32,
    pub k: f32,
    pub ke: i32,
}

const _: () = assert!(std::mem::size_of::<BlaEntryX>() == 40);

/// Exponent the kernel uses for zero (loses every alignment and comparison).
pub const XZERO_EXP: i32 = -(1 << 28);

/// `x = m 2^e` with `m` in [0.5, 1) for finite non-zero normal `x`.
pub(crate) fn frexp(x: f64) -> (f64, i64) {
    let bits = x.to_bits();
    let be = ((bits >> 52) & 0x7ff) as i64;
    if be == 0 {
        if x == 0.0 {
            return (0.0, 0);
        }
        let (m, e) = frexp(x * 18446744073709551616.0);
        return (m, e - 64);
    }
    (f64::from_bits((bits & !(0x7ffu64 << 52)) | (1022u64 << 52)), be - 1022)
}

pub(crate) fn pow2(k: i64) -> f64 {
    if k < -1022 {
        if k < -1074 { 0.0 } else { pow2(k + 64) * pow2(-64) }
    } else if k > 1023 {
        f64::INFINITY
    } else {
        f64::from_bits(((k + 1023) as u64) << 52)
    }
}

/// Extended-range non-negative real `m 2^e` (m in [0.5, 1), or 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Xr {
    pub(crate) m: f64,
    pub(crate) e: i64,
}

impl Xr {
    pub(crate) const ZERO: Xr = Xr { m: 0.0, e: 0 };
    pub(crate) fn new(x: f64, e: i64) -> Xr {
        if x == 0.0 || !x.is_finite() {
            return Xr::ZERO;
        }
        let (m, k) = frexp(x);
        Xr { m, e: e + k }
    }
    pub(crate) fn from_f64(x: f64) -> Xr {
        Xr::new(x, 0)
    }
    pub(crate) fn is_zero(self) -> bool {
        self.m == 0.0
    }
    pub(crate) fn mul(self, b: Xr) -> Xr {
        Xr::new(self.m * b.m, self.e + b.e)
    }
    pub(crate) fn div(self, b: Xr) -> Xr {
        if b.is_zero() {
            return Xr::ZERO;
        }
        Xr::new(self.m / b.m, self.e - b.e)
    }
    pub(crate) fn add(self, b: Xr) -> Xr {
        if self.is_zero() {
            return b;
        }
        if b.is_zero() {
            return self;
        }
        let (x, y) = if self.e >= b.e { (self, b) } else { (b, self) };
        let d = x.e - y.e;
        if d > 64 {
            return x;
        }
        Xr::new(x.m + y.m * pow2(-d), x.e)
    }
    pub(crate) fn lt(self, b: Xr) -> bool {
        if self.m <= 0.0 || b.m <= 0.0 {
            return self.m < b.m;
        }
        (self.e, self.m) < (b.e, b.m)
    }
    pub(crate) fn min(self, b: Xr) -> Xr {
        if b.lt(self) { b } else { self }
    }
    pub(crate) fn max(self, b: Xr) -> Xr {
        if self.lt(b) { b } else { self }
    }
    /// `|self - b|`.
    pub(crate) fn abs_diff(self, b: Xr) -> Xr {
        let (x, y) = if self.lt(b) { (b, self) } else { (self, b) };
        if y.is_zero() {
            return x;
        }
        let d = x.e - y.e;
        if d > 64 {
            return x;
        }
        Xr::new(x.m - y.m * pow2(-d), x.e)
    }
    pub(crate) fn gpu(self) -> (f32, i32) {
        if self.is_zero() {
            return (0.0, XZERO_EXP);
        }
        let mut m = self.m as f32;
        let mut e = self.e;
        if m >= 1.0 {
            m *= 0.5;
            e += 1;
        }
        (m, e.clamp(-(1 << 27), 1 << 27) as i32)
    }
}

/// Extended-range complex with one exponent: `(re + i im) 2^e`, max-abs mantissa in [0.5, 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Xc {
    pub(crate) re: f64,
    pub(crate) im: f64,
    pub(crate) e: i64,
}

impl Xc {
    pub(crate) const ZERO: Xc = Xc { re: 0.0, im: 0.0, e: 0 };
    pub(crate) const ONE: Xc = Xc { re: 0.5, im: 0.0, e: 1 };
    pub(crate) fn new(re: f64, im: f64, e: i64) -> Xc {
        let a = re.abs().max(im.abs());
        if a == 0.0 || !a.is_finite() {
            return Xc::ZERO;
        }
        let (_, k) = frexp(a);
        let s = pow2(-k);
        Xc {
            re: re * s,
            im: im * s,
            e: e + k,
        }
    }
    pub(crate) fn is_zero(self) -> bool {
        self.re == 0.0 && self.im == 0.0
    }
    pub(crate) fn mul(self, b: Xc) -> Xc {
        Xc::new(
            self.re * b.re - self.im * b.im,
            self.re * b.im + self.im * b.re,
            self.e + b.e,
        )
    }
    pub(crate) fn add(self, b: Xc) -> Xc {
        if self.is_zero() {
            return b;
        }
        if b.is_zero() {
            return self;
        }
        let (x, y) = if self.e >= b.e { (self, b) } else { (b, self) };
        let d = x.e - y.e;
        if d > 64 {
            return x;
        }
        let s = pow2(-d);
        Xc::new(x.re + y.re * s, x.im + y.im * s, x.e)
    }
    pub(crate) fn abs(self) -> Xr {
        Xr::new(self.re.hypot(self.im), self.e)
    }
    pub(crate) fn conj(self) -> Xc {
        Xc { im: -self.im, ..self }
    }
    /// From a [`DoubleExpComplex`] (separate exponents per part).
    pub(crate) fn from_double_exp(z: almondbread_math::DoubleExpComplex) -> Xc {
        let part = |v: almondbread_math::DoubleExp| {
            if v.is_zero() {
                None
            } else {
                Some((v.mantissa, v.exponent))
            }
        };
        match (part(z.re), part(z.im)) {
            (None, None) => Xc::ZERO,
            (Some((m, e)), None) => Xc::new(m, 0.0, e),
            (None, Some((m, e))) => Xc::new(0.0, m, e),
            (Some((a, ea)), Some((b, eb))) => {
                let e = ea.max(eb);
                Xc::new(a * pow2(ea - e), b * pow2(eb - e), e)
            }
        }
    }
    pub(crate) fn gpu(self) -> ([f32; 2], i32) {
        if self.is_zero() {
            return ([0.0, 0.0], XZERO_EXP);
        }
        let (mut re, mut im, mut e) = (self.re as f32, self.im as f32, self.e);
        if re.abs().max(im.abs()) >= 1.0 {
            re *= 0.5;
            im *= 0.5;
            e += 1;
        }
        ([re, im], e.clamp(-(1 << 27), 1 << 27) as i32)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MergedX {
    pub(crate) a: Xc,
    pub(crate) b: Xc,
    pub(crate) r: Xr,
    pub(crate) k: Xr,
}

const INVALID_X: MergedX = MergedX {
    a: Xc::ZERO,
    b: Xc::ZERO,
    r: Xr::ZERO,
    k: Xr::ZERO,
};

impl MergedX {
    pub(crate) fn single(z: &FloatExpComplex, eps: f64) -> Self {
        let a = Xc::new(
            2.0 * f64::from(z.mantissa[0]),
            2.0 * f64::from(z.mantissa[1]),
            i64::from(z.exponent),
        );
        MergedX {
            a,
            b: Xc::ONE,
            r: Xr::from_f64(eps).mul(a.abs()),
            k: Xr::ZERO,
        }
    }

    /// A single step of another formula (F2), as [`Merged::single_for`].
    pub(crate) fn single_for(z: &FloatExpComplex, eps: f64, formula: Formula) -> Self {
        let big_z = Xc::new(
            f64::from(z.mantissa[0]),
            f64::from(z.mantissa[1]),
            i64::from(z.exponent),
        );
        let p = formula.power();
        let mut w = big_z;
        for _ in 2..p {
            w = w.mul(big_z);
        }
        let pf = f64::from(p);
        MergedX {
            a: Xc::new(pf * w.re, pf * w.im, w.e),
            b: if formula.is_julia() { Xc::ZERO } else { Xc::ONE },
            r: Xr::from_f64(eps * 2.0 / (pf - 1.0)).mul(big_z.abs()),
            k: Xr::ZERO,
        }
    }

    pub(crate) fn then(&self, y: &MergedX) -> MergedX {
        let ax = self.a.abs();
        if self.r.is_zero() || y.r.is_zero() || ax.is_zero() {
            return INVALID_X;
        }
        MergedX {
            a: y.a.mul(self.a),
            b: y.a.mul(self.b).add(y.b),
            r: self.r.min(y.r.div(ax)),
            k: self.k.max(y.k.add(self.b.abs()).div(ax)),
        }
    }

    pub(crate) fn entry(&self) -> BlaEntryX {
        if self.r.is_zero() || self.e_out_of_range() {
            return BlaEntryX {
                ae: XZERO_EXP,
                be: XZERO_EXP,
                re: XZERO_EXP,
                ke: XZERO_EXP,
                ..BlaEntryX::default()
            };
        }
        let (a, ae) = self.a.gpu();
        let (b, be) = self.b.gpu();
        let (r, re) = self.r.gpu();
        let (k, ke) = self.k.gpu();
        BlaEntryX {
            a,
            b,
            ae,
            be,
            r,
            re,
            k,
            ke,
        }
    }

    pub(crate) fn e_out_of_range(&self) -> bool {
        let big = 1i64 << 27;
        self.a.e.abs() > big || self.b.e.abs() > big || self.r.e.abs() > big || self.k.e.abs() > big
    }
}

/// A real-linear map `δ ↦ p δ + q conj(δ)` in extended range (F5; [`Linear`] with [`Xc`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Xl {
    pub(crate) p: Xc,
    pub(crate) q: Xc,
}

impl Xl {
    pub(crate) const ZERO: Xl = Xl {
        p: Xc::ZERO,
        q: Xc::ZERO,
    };
    pub(crate) const IDENTITY: Xl = Xl {
        p: Xc::ONE,
        q: Xc::ZERO,
    };
    /// `self` first, then `y`.
    pub(crate) fn then(self, y: Xl) -> Xl {
        Xl {
            p: y.p.mul(self.p).add(y.q.mul(self.q.conj())),
            q: y.p.mul(self.q).add(y.q.mul(self.p.conj())),
        }
    }
    pub(crate) fn add(self, y: Xl) -> Xl {
        Xl {
            p: self.p.add(y.p),
            q: self.q.add(y.q),
        }
    }
    pub(crate) fn sup(self) -> Xr {
        self.p.abs().add(self.q.abs())
    }
    pub(crate) fn inf(self) -> Xr {
        self.p.abs().abs_diff(self.q.abs())
    }
    /// Mantissas of p and q with one shared exponent (max-abs in [0.5, 1)), as the GPU reads.
    pub(crate) fn gpu(self) -> ([f32; 2], [f32; 2], i32) {
        if self.p.is_zero() && self.q.is_zero() {
            return ([0.0; 2], [0.0; 2], XZERO_EXP);
        }
        let e = match (self.p.is_zero(), self.q.is_zero()) {
            (true, _) => self.q.e,
            (_, true) => self.p.e,
            _ => self.p.e.max(self.q.e),
        };
        let at = |c: Xc| {
            if c.is_zero() {
                [0.0f32; 2]
            } else {
                let f = pow2(c.e - e);
                [(c.re * f) as f32, (c.im * f) as f32]
            }
        };
        (at(self.p), at(self.q), e.clamp(-(1 << 27), 1 << 27) as i32)
    }
}

/// One deep entry of a non-conformal table (F5, `BlaEntryMX` in MSL, 56 bytes): `A = (ap, aq)
/// 2^ae`, `B = (bp, bq) 2^be` (Wirtinger mantissas, max-abs in [0.5, 1)), `R = r 2^re`,
/// `K = k 2^ke` as [`BlaEntryX`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlaEntryMX {
    pub ap: [f32; 2],
    pub aq: [f32; 2],
    pub bp: [f32; 2],
    pub bq: [f32; 2],
    pub ae: i32,
    pub be: i32,
    pub r: f32,
    pub re: i32,
    pub k: f32,
    pub ke: i32,
}

const _: () = assert!(std::mem::size_of::<BlaEntryMX>() == 56);

#[derive(Clone, Copy, Debug)]
struct MergedMX {
    a: Xl,
    b: Xl,
    r: Xr,
    k: Xr,
}

const INVALID_MX: MergedMX = MergedMX {
    a: Xl::ZERO,
    b: Xl::ZERO,
    r: Xr::ZERO,
    k: Xr::ZERO,
};

impl MergedMX {
    fn single(z: &FloatExpComplex, eps: f64, formula: Formula) -> Self {
        use almondbread_math::{DoubleExp, DoubleExpComplex};
        let part = |m: f32| DoubleExp::from_f64(f64::from(m)).times_power_of_two(i64::from(z.exponent));
        let zd = DoubleExpComplex::new(part(z.mantissa[0]), part(z.mantissa[1]));
        let j = formula.jacobian_x(zd);
        let a = Xl {
            p: Xc::from_double_exp(j.p),
            q: Xc::from_double_exp(j.q),
        };
        MergedMX {
            a,
            b: Xl::IDENTITY,
            r: Xr::from_f64(eps).mul(a.inf()),
            k: Xr::ZERO,
        }
    }

    fn then(&self, y: &MergedMX) -> MergedMX {
        let ax = self.a.sup();
        if self.r.is_zero() || y.r.is_zero() || ax.is_zero() {
            return INVALID_MX;
        }
        MergedMX {
            a: self.a.then(y.a),
            b: self.b.then(y.a).add(y.b),
            r: self.r.min(y.r.div(ax)),
            k: self.k.max(y.k.add(self.b.sup()).div(ax)),
        }
    }

    fn entry(&self) -> BlaEntryMX {
        let big = 1i64 << 27;
        let out = [self.a.p.e, self.a.q.e, self.b.p.e, self.b.q.e, self.r.e, self.k.e]
            .iter()
            .any(|e| e.abs() > big);
        if self.r.is_zero() || out {
            return BlaEntryMX {
                ae: XZERO_EXP,
                be: XZERO_EXP,
                re: XZERO_EXP,
                ke: XZERO_EXP,
                ..BlaEntryMX::default()
            };
        }
        let (ap, aq, ae) = self.a.gpu();
        let (bp, bq, be) = self.b.gpu();
        let (r, re) = self.r.gpu();
        let (k, ke) = self.k.gpu();
        BlaEntryMX {
            ap,
            aq,
            bp,
            bq,
            ae,
            be,
            r,
            re,
            k,
            ke,
        }
    }
}

/// The BLA table of a big reference in extended range (the deep kernel's table). Same layout
/// and validity as [`BlaTable`], built from the floatexp reference points with f64 mantissas
/// and i64 exponents, so radii of 1e-1000 and coefficients of 1e+100000 are exact to f64.
#[derive(Clone, Debug, Default)]
pub struct BlaTableX {
    pub ref_len: usize,
    pub epsilon: f64,
    pub min_level: u32,
    pub max_level: u32,
    pub offsets: [u32; MAX_LEVELS],
    pub entries: Vec<BlaEntryX>,
    /// A non-conformal formula's entries (F5; then `entries` is empty).
    pub matrix: Vec<BlaEntryMX>,
    pub build_time: f64,
}

impl BlaTableX {
    pub fn build(points: &[FloatExpComplex], epsilon: f64) -> Self {
        Self::build_for(points, epsilon, Formula::Mandelbrot)
    }

    /// [`BlaTableX::build`] for the orbit of any formula (F2).
    pub fn build_for(points: &[FloatExpComplex], epsilon: f64, formula: Formula) -> Self {
        Self::build_cancellable(points, epsilon, formula, &AtomicBool::new(false)).expect("never cancelled")
    }

    /// [`BlaTableX::build_for`] that polls `cancel` before each level; None when cancelled.
    pub fn build_cancellable(
        points: &[FloatExpComplex],
        epsilon: f64,
        formula: Formula,
        cancel: &AtomicBool,
    ) -> Option<Self> {
        let start = Instant::now();
        let mut t = BlaTableX {
            ref_len: points.len(),
            epsilon,
            min_level: MIN_LEVEL,
            max_level: 0,
            ..Default::default()
        };
        let singles = points.len().saturating_sub(2);
        if singles < (1 << MIN_LEVEL) {
            t.build_time = start.elapsed().as_secs_f64();
            return Some(t);
        }
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        if !formula.is_conformal() {
            let mut level: Vec<MergedMX> = points[1..1 + singles]
                .iter()
                .map(|z| MergedMX::single(z, epsilon, formula))
                .collect();
            let mut lev = 0u32;
            let mut matrix: Vec<BlaEntryMX> = Vec::with_capacity(singles);
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }
                if lev >= MIN_LEVEL {
                    t.offsets[lev as usize] = matrix.len() as u32;
                    matrix.extend(level.iter().map(MergedMX::entry));
                    t.max_level = lev;
                }
                if level.len() < 2 || lev as usize + 1 >= MAX_LEVELS {
                    break;
                }
                level = level.as_chunks::<2>().0.iter().map(|p| p[0].then(&p[1])).collect();
                lev += 1;
            }
            t.matrix = matrix;
            t.build_time = start.elapsed().as_secs_f64();
            return Some(t);
        }
        let steps = points[1..1 + singles].iter();
        let mut level: Vec<MergedX> = if formula.is_mandelbrot() {
            steps.map(|z| MergedX::single(z, epsilon)).collect()
        } else {
            steps.map(|z| MergedX::single_for(z, epsilon, formula)).collect()
        };
        let mut lev = 0u32;
        let mut entries: Vec<BlaEntryX> = Vec::with_capacity(singles);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            if lev >= MIN_LEVEL {
                t.offsets[lev as usize] = entries.len() as u32;
                entries.extend(level.iter().map(MergedX::entry));
                t.max_level = lev;
            }
            if level.len() < 2 || lev as usize + 1 >= MAX_LEVELS {
                break;
            }
            level = level.as_chunks::<2>().0.iter().map(|p| p[0].then(&p[1])).collect();
            lev += 1;
        }
        t.entries = entries;
        t.build_time = start.elapsed().as_secs_f64();
        Some(t)
    }

    pub fn bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<BlaEntryX>() + self.matrix.len() * std::mem::size_of::<BlaEntryMX>()
    }

    /// Whether the table holds matrix entries (a non-conformal formula, F5).
    pub fn is_matrix(&self) -> bool {
        !self.matrix.is_empty()
    }

    /// Entries (either kind).
    pub fn len(&self) -> usize {
        self.entries.len() + self.matrix.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The GPU's bytes (either kind).
    pub fn data(&self) -> &[u8] {
        if self.is_matrix() {
            bytemuck::cast_slice(&self.matrix)
        } else {
            bytemuck::cast_slice(&self.entries)
        }
    }

    /// The matrix entry of `level` at `m` (F5), if one exists.
    pub fn lookup_m(&self, m: usize, level: u32) -> Option<&BlaEntryMX> {
        if level < self.min_level || level > self.max_level || m == 0 {
            return None;
        }
        let len = 1usize << level;
        if !(m - 1).is_multiple_of(len) || m + len > self.ref_len - 1 {
            return None;
        }
        self.matrix
            .get(self.offsets[level as usize] as usize + ((m - 1) >> level))
    }

    fn longest_m(&self, m: usize, n: usize, max_iter: usize, zn: Xr, cn: Xr) -> Option<(&BlaEntryMX, usize)> {
        if m == 0 {
            return None;
        }
        let mut best = None;
        for lev in self.min_level..=self.max_level {
            let len = 1usize << lev;
            if n + len > max_iter {
                break;
            }
            let Some(e) = self.lookup_m(m, lev) else { break };
            if e.r.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                break;
            }
            let r = Xr::new(f64::from(e.r), i64::from(e.re));
            let k = Xr::new(f64::from(e.k), i64::from(e.ke));
            if !zn.add(k.mul(cn)).lt(r) {
                break;
            }
            best = Some((e, len));
        }
        best
    }

    pub fn lookup(&self, m: usize, level: u32) -> Option<&BlaEntryX> {
        if level < self.min_level || level > self.max_level || m == 0 {
            return None;
        }
        let len = 1usize << level;
        if !(m - 1).is_multiple_of(len) || m + len > self.ref_len - 1 {
            return None;
        }
        self.entries
            .get(self.offsets[level as usize] as usize + ((m - 1) >> level))
    }

    fn valid(e: &BlaEntryX, zn: Xr, cn: Xr) -> bool {
        if e.r.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return false;
        }
        let r = Xr::new(f64::from(e.r), i64::from(e.re));
        let k = Xr::new(f64::from(e.k), i64::from(e.ke));
        zn.add(k.mul(cn)).lt(r)
    }

    /// The longest valid block at `m` for `|dz| = 2^zn_log2`, `|dc| = 2^cn_log2`.
    pub(crate) fn longest_pub(
        &self,
        m: usize,
        n: usize,
        max_iter: usize,
        zn: Xr,
        cn: Xr,
    ) -> Option<(&BlaEntryX, usize)> {
        self.longest(m, n, max_iter, zn, cn)
    }

    fn longest(&self, m: usize, n: usize, max_iter: usize, zn: Xr, cn: Xr) -> Option<(&BlaEntryX, usize)> {
        if m == 0 {
            return None;
        }
        let mut best = None;
        for lev in self.min_level..=self.max_level {
            let len = 1usize << lev;
            if n + len > max_iter {
                break;
            }
            let Some(e) = self.lookup(m, lev) else { break };
            if !Self::valid(e, zn, cn) {
                break;
            }
            best = Some((e, len));
        }
        best
    }

    /// [`BlaTable::forecast`] for a pixel `|dc| = c_abs` (log2 given, so any depth).
    pub fn forecast(&self, c_abs_log2: f64, max_iter: usize) -> BlaForecast {
        if !c_abs_log2.is_finite() {
            return BlaForecast::default();
        }
        let whole = c_abs_log2.floor();
        let cn = Xr::new(2f64.powf(c_abs_log2 - whole), whole as i64);
        let (mut m, mut zn, mut covered, mut blocks) = (1usize, cn, 0usize, 0usize);
        if self.is_matrix() {
            let sup = |p: [f32; 2], q: [f32; 2], e: i32| {
                Xr::new(
                    f64::from(p[0]).hypot(f64::from(p[1])) + f64::from(q[0]).hypot(f64::from(q[1])),
                    i64::from(e),
                )
            };
            while let Some((e, len)) = self.longest_m(m, 1 + covered, max_iter, zn, cn) {
                zn = sup(e.ap, e.aq, e.ae).mul(zn).add(sup(e.bp, e.bq, e.be).mul(cn));
                m += len;
                covered += len;
                blocks += 1;
            }
            return BlaForecast { covered, blocks };
        }
        while let Some((e, len)) = self.longest(m, 1 + covered, max_iter, zn, cn) {
            let a = Xr::new(f64::from(e.a[0]).hypot(f64::from(e.a[1])), i64::from(e.ae));
            let b = Xr::new(f64::from(e.b[0]).hypot(f64::from(e.b[1])), i64::from(e.be));
            zn = a.mul(zn).add(b.mul(cn));
            m += len;
            covered += len;
            blocks += 1;
        }
        BlaForecast { covered, blocks }
    }

    /// `eps max |2 Z|` in log2: no block is valid for a pixel farther than this (in |c|).
    pub fn reach_log2(points: &[FloatExpComplex], epsilon: f64) -> f64 {
        let n = points.len().saturating_sub(1);
        points[1.min(n)..n]
            .iter()
            .map(|z| {
                let m = f64::from(z.mantissa[0]).hypot(f64::from(z.mantissa[1]));
                if m == 0.0 {
                    f64::NEG_INFINITY
                } else {
                    m.log2() + f64::from(z.exponent) + 1.0
                }
            })
            .fold(f64::NEG_INFINITY, f64::max)
            + epsilon.log2()
    }
}

/// See [`BlaTable::forecast`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlaForecast {
    /// Iterations a pixel at the view's largest distance from the reference skips before its
    /// first exact step.
    pub covered: usize,
    pub blocks: usize,
}

impl BlaForecast {
    /// Whether the BLA pipeline is expected to beat plain perturbation: at least 1024
    /// iterations covered in blocks of 64 on average. Measured crossover on M1 Pro
    /// (kb: synthesis/benchmarks M4): a loss at every view the f64 reference reaches, a win
    /// from about 1e-20 per pixel.
    pub fn pays(&self) -> bool {
        self.covered >= 1024 && self.covered >= 64 * self.blocks
    }

    /// The deep kernel's rule (M3b, kb: synthesis/benchmarks M3b): its exact steps cost ~30 %
    /// more than plain ones and pixels there run many reference periods, each skipped again,
    /// so blocks of 16 on average over a first run of 64 already win (measured 1.3x to 47x on
    /// every deep benchmark location).
    pub fn pays_deep(&self) -> bool {
        self.covered >= 64 && self.covered >= 16 * self.blocks
    }
}

/// Outcome of [`iterate_delta`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeltaOutcome {
    /// Escape iteration, or `max_iter`.
    pub n: usize,
    pub escaped: bool,
    /// Loop steps: BLA blocks plus exact perturbation steps.
    pub steps: usize,
    pub blocks: usize,
    /// |z|^2 at escape (or at `max_iter`).
    pub r2: f64,
}

impl DeltaOutcome {
    /// Smooth iteration count `n + 1 - log2(log|z|)` (kb: techniques/smooth-coloring).
    pub fn mu(&self) -> f64 {
        self.n as f64 + 1.0 - (0.5 * self.r2.ln()).log2()
    }
}

/// CPU model of the `iterate` kernel's loop in f64 with the same reference points, lookup,
/// rebasing and lookup policy (lookups only at m = 1 mod `lookup_every`). For tests and
/// diagnostics.
pub fn iterate_delta(
    points: &[[f32; 2]],
    table: Option<&BlaTable>,
    lookup_every: usize,
    dc: (f64, f64),
    max_iter: usize,
    bailout2: f64,
) -> DeltaOutcome {
    iterate_delta_rounded(points, table, lookup_every, dc, max_iter, bailout2, false)
}

/// [`iterate_delta`], optionally rounding the delta to float32 after every step (a model of
/// the GPU's float32 error, as a yardstick for the error BLA adds).
fn iterate_delta_rounded(
    points: &[[f32; 2]],
    table: Option<&BlaTable>,
    lookup_every: usize,
    dc: (f64, f64),
    max_iter: usize,
    bailout2: f64,
    round32: bool,
) -> DeltaOutcome {
    let last = points.len() - 1;
    let zr = |m: usize| (f64::from(points[m][0]), f64::from(points[m][1]));
    let norm2 = |x: (f64, f64)| x.0 * x.0 + x.1 * x.1;
    let cn = abs(dc);
    let mut out = DeltaOutcome {
        n: 1,
        escaped: false,
        steps: 1,
        blocks: 0,
        r2: 0.0,
    };
    let (mut dz, mut m) = (dc, 1usize);
    // Rebase and escape tests after every step; true when the pixel escaped.
    let finish = |dz: &mut (f64, f64), m: &mut usize, out: &mut DeltaOutcome| {
        let zm = zr(*m);
        let z = (zm.0 + dz.0, zm.1 + dz.1);
        out.r2 = norm2(z);
        if out.r2 > bailout2 {
            out.escaped = true;
            return true;
        }
        if out.r2 < norm2(*dz) || *m == last {
            *dz = z;
            *m = 0;
        }
        false
    };
    if finish(&mut dz, &mut m, &mut out) {
        return out;
    }
    while out.n < max_iter {
        out.steps += 1;
        let block = match table {
            Some(t) if m != 0 && (m - 1).is_multiple_of(lookup_every.max(1)) => {
                t.longest(m, out.n, max_iter, abs(dz), cn).map(|(e, l)| (*e, l))
            }
            _ => None,
        };
        if let Some((e, len)) = block {
            let az = cmul((f64::from(e[0]), f64::from(e[1])), dz);
            let bc = cmul((f64::from(e[2]), f64::from(e[3])), dc);
            dz = (az.0 + bc.0, az.1 + bc.1);
            m += len;
            out.n += len;
            out.blocks += 1;
        } else {
            let zm = zr(m);
            let t = cmul((2.0 * zm.0 + dz.0, 2.0 * zm.1 + dz.1), dz);
            dz = (t.0 + dc.0, t.1 + dc.1);
            if round32 {
                dz = (f64::from(dz.0 as f32), f64::from(dz.1 as f32));
            }
            m += 1;
            out.n += 1;
        }
        if finish(&mut dz, &mut m, &mut out) {
            return out;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::Reference;
    use crate::viewport::Viewport;

    const BAILOUT2: f64 = 65536.0 * 65536.0;
    /// The slow boundary view of the M4 brief (kb: synthesis/benchmarks M4).
    const SLOW: (f64, f64) = (-1.4080321340304542, 0.13680236432900417);

    /// F2: the generic tables at p = 2 hold Mandelbrot's coefficients and radii (to f64
    /// rounding: `p Z^(p-1)` and `eps 2 |Z| / (p-1)` against `2Z` and `eps |2Z|`).
    #[test]
    fn generic_tables_are_mandelbrot() {
        let pts = reference(SLOW.0, SLOW.1, 5000);
        let g = Formula::Multibrot(2);
        let (a, b) = (
            BlaTable::build(&pts, BLA_EPSILON),
            BlaTable::build_for(&pts, BLA_EPSILON, g),
        );
        assert_eq!(
            (a.min_level, a.max_level, a.offsets),
            (b.min_level, b.max_level, b.offsets)
        );
        for (x, y) in a.entries.iter().zip(&b.entries) {
            for k in 0..6 {
                assert!((x[k] - y[k]).abs() <= 1e-6 * x[k].abs().max(1e-30), "{x:?} {y:?}");
            }
        }
        let ext: Vec<FloatExpComplex> = pts
            .iter()
            .map(|p| {
                let a = p[0].abs().max(p[1].abs());
                if a == 0.0 {
                    return FloatExpComplex::ZERO;
                }
                let e = a.log2().floor() as i32 + 1;
                let s = 2f32.powi(-e);
                FloatExpComplex {
                    mantissa: [p[0] * s, p[1] * s],
                    exponent: e,
                }
            })
            .collect();
        let (a, b) = (
            BlaTableX::build(&ext, BLA_EPSILON),
            BlaTableX::build_for(&ext, BLA_EPSILON, g),
        );
        assert_eq!(a.entries.len(), b.entries.len());
        for (x, y) in a.entries.iter().zip(&b.entries) {
            assert!(
                x.ae == y.ae && x.re == y.re && (x.a[0] - y.a[0]).abs() <= 1e-6 && (x.r - y.r).abs() <= 1e-6,
                "{x:?} {y:?}"
            );
        }
        // z^3 + c: a two-step block is A = 3 Z_2^2 3 Z_1^2, B = 3 Z_2^2 + 1.
        let cubic = BlaTable::build_for(&pts, BLA_EPSILON, Formula::Multibrot(3));
        let e = cubic.lookup(1, 1).expect("level-1 entry");
        let p = |z: [f32; 2]| Formula::Multibrot(3).prime((f64::from(z[0]), f64::from(z[1])));
        let want = cmul(p(pts[2]), p(pts[1]));
        assert!(
            (f64::from(e[0]) - want.0).abs() <= 1e-5 * abs(want)
                && (f64::from(e[1]) - want.1).abs() <= 1e-5 * abs(want)
        );
        assert!((f64::from(e[2]) - (p(pts[2]).0 + 1.0)).abs() <= 1e-5 * abs(p(pts[2])).max(1.0));
    }

    fn reference(re: f64, im: f64, max_iter: usize) -> Vec<[f32; 2]> {
        Reference::compute_f64(&Viewport::new(re, im, 1e-10), max_iter, 65536.0)
            .points
            .to_vec()
    }

    #[test]
    fn layout_and_lookup() {
        let pts = reference(-0.75, 0.1, 1000);
        let t = BlaTable::build(&pts, BLA_EPSILON);
        let last = pts.len() - 1;
        let mut expect = 0;
        for lev in t.min_level..=t.max_level {
            assert_eq!(t.offsets[lev as usize] as usize, expect);
            expect += (last - 1) >> lev;
        }
        assert_eq!(t.entries.len(), expect);
        assert!(t.lookup(1, 1).is_some());
        assert!(t.lookup(2, 1).is_none());
        assert!(t.lookup(3, 1).is_some());
        assert!(t.lookup(last - 1, 1).is_none());
        assert!(t.lookup(1, 0).is_none(), "level 0 is not stored");
    }

    #[test]
    fn blocks_compose_the_steps() {
        // An entry's A, B equal the composed single steps (merge order), and R, K are no
        // looser than the per-step bound of the module docs.
        let pts = reference(SLOW.0, SLOW.1, 6000);
        let t = BlaTable::build(&pts, BLA_EPSILON);
        for (m, lev) in [(1usize, 1u32), (1, 3), (65, 6), (1025, 10)] {
            let e = t.lookup(m, lev).unwrap();
            let (mut a, mut b, mut r, mut k) = ((1.0, 0.0), (0.0, 0.0), f64::INFINITY, 0.0f64);
            for j in m..m + (1 << lev) {
                let s = (2.0 * f64::from(pts[j][0]), 2.0 * f64::from(pts[j][1]));
                // Inner step j: |A| |z| + |B| |c| <= eps |2 Z_j|.
                r = r.min(BLA_EPSILON * abs(s) / abs(a));
                k = k.max(abs(b) / abs(a));
                a = cmul(s, a);
                let sb = cmul(s, b);
                b = (sb.0 + 1.0, sb.1);
            }
            let close = |x: f64, y: f64| (x - y).abs() <= 1e-5 * y.abs().max(1e-30);
            if e[4] > 0.0 {
                assert!(close(f64::from(e[0]).hypot(f64::from(e[1])), abs(a)), "A at {m}/{lev}");
                assert!(
                    close(f64::from(e[2]), b.0) && close(f64::from(e[3]), b.1),
                    "B at {m}/{lev}"
                );
                // The merged bound is conservative (triangle inequality on B, min and max
                // taken separately): R no larger, K no smaller than the per-step bound.
                assert!(f64::from(e[4]) <= r * (1.0 + 1e-5), "R at {m}/{lev}: {} vs {r}", e[4]);
                assert!(f64::from(e[5]) >= k * (1.0 - 1e-5), "K at {m}/{lev}: {} vs {k}", e[5]);
            }
        }
    }

    /// Plain f64 perturbation against the table (same model loop) on a grid of `scale` per
    /// pixel around the reference: (class mismatches, median |dmu|, median |dmu| of a
    /// control that rounds the delta to float32 every step, step ratio, blocks).
    fn compare(re: f64, im: f64, scale: f64, max_iter: usize) -> (usize, f64, f64, f64, usize) {
        let pts = reference(re, im, max_iter);
        let t = BlaTable::build(&pts, BLA_EPSILON);
        let (mut class, mut dmu, mut plain, mut bla, mut blocks) = (0, Vec::new(), 0, 0, 0);
        let mut noise = Vec::new();
        for j in 0..40 {
            for i in 0..64 {
                let dc = (
                    (i as f64 - 32.0 + 0.37) * scale * 25.0,
                    (j as f64 - 20.0 + 0.61) * scale * 25.0,
                );
                let p = iterate_delta(&pts, None, 8, dc, max_iter, BAILOUT2);
                let b = iterate_delta(&pts, Some(&t), 8, dc, max_iter, BAILOUT2);
                // Control: plain perturbation with the delta rounded to float32 every step.
                let q = iterate_delta_rounded(&pts, None, 8, dc, max_iter, BAILOUT2, true);
                if p.escaped && q.escaped {
                    noise.push((p.mu() - q.mu()).abs());
                }
                plain += p.steps;
                bla += b.steps;
                blocks += b.blocks;
                if p.escaped != b.escaped {
                    class += 1;
                } else if p.escaped {
                    dmu.push((p.mu() - b.mu()).abs());
                }
            }
        }
        let median = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v.get(v.len() / 2).copied().unwrap_or(0.0)
        };
        (
            class,
            median(&mut dmu),
            median(&mut noise),
            plain as f64 / bla as f64,
            blocks,
        )
    }

    #[test]
    fn agrees_with_perturbation() {
        // Views the GPU renders today (boundary heavy: n itself is chaotic there, so the
        // check is on the class and the median smooth count) and two deeper grids around the
        // same reference, where BLA skips most of the orbit.
        // (The -3/4 neck at 1.35e-8 per pixel: deltas too large for any block.)
        for (re, im, scale, it, skips) in [
            (SLOW.0, SLOW.1, 4.615e-14, 5791, true),
            (-0.7436438870371587, 0.13182590420531198, 1.08e-13, 6000, true),
            (-0.7500016386523336, -0.002107342465685259, 1.35e-8, 2053, false),
            (SLOW.0, SLOW.1, 1e-20, 12000, true),
            (SLOW.0, SLOW.1, 1e-40, 12000, true),
        ] {
            let (class, median, noise, ratio, blocks) = compare(re, im, scale, it);
            println!(
                "{re},{im} @ {scale}: class mismatch {class}, median |dmu| {median:.2e} (control {noise:.2e}), step ratio {ratio:.2}, {blocks} blocks"
            );
            assert!(class <= 2, "{class} pixels changed class");
            // BLA in f64 changes the result about as much as float32 rounding alone does.
            assert!(
                median <= 2.0 * noise.max(1e-9),
                "median |dmu| {median} vs float32 control {noise}"
            );
            assert_eq!(blocks > 0, skips);
        }
    }

    #[test]
    fn forecast_separates_shallow_from_deep() {
        let pts = reference(SLOW.0, SLOW.1, 12000);
        let t = BlaTable::build(&pts, BLA_EPSILON);
        let half = |scale: f64| scale * 1600f64.hypot(1000.0) / 2.0;
        let shallow = t.forecast(half(4.615e-14), 12000);
        let deep = t.forecast(half(1e-20), 12000);
        println!("forecast shallow {shallow:?} deep {deep:?}");
        assert!(!shallow.pays());
        assert!(deep.pays());
        let (_, _, _, ratio, _) = compare(SLOW.0, SLOW.1, 1e-20, 12000);
        assert!(ratio > 4.0, "step ratio at 1e-20: {ratio}");
    }

    /// F5: the CPU model of the kernel loop for a non-conformal formula: matrix blocks
    /// (`z <- A z + B c`), exact steps through [`Formula::delta_step`], the same rebasing.
    fn iterate_delta_m(
        formula: Formula,
        points: &[[f32; 2]],
        table: Option<&BlaTable>,
        dc: (f64, f64),
        max_iter: usize,
        round32: bool,
    ) -> DeltaOutcome {
        let last = points.len() - 1;
        let zr = |m: usize| (f64::from(points[m][0]), f64::from(points[m][1]));
        let norm2 = |x: (f64, f64)| x.0 * x.0 + x.1 * x.1;
        let cn = abs(dc);
        let mut out = DeltaOutcome {
            n: 1,
            escaped: false,
            steps: 1,
            blocks: 0,
            r2: 0.0,
        };
        let (mut dz, mut m) = (dc, 1usize);
        let finish = |dz: &mut (f64, f64), m: &mut usize, out: &mut DeltaOutcome| {
            let zm = zr(*m);
            let z = (zm.0 + dz.0, zm.1 + dz.1);
            out.r2 = norm2(z);
            if out.r2 > BAILOUT2 {
                out.escaped = true;
                return true;
            }
            if out.r2 < norm2(*dz) || *m == last {
                *dz = z;
                *m = 0;
            }
            false
        };
        if finish(&mut dz, &mut m, &mut out) {
            return out;
        }
        while out.n < max_iter {
            out.steps += 1;
            let block = match table {
                Some(t) if m != 0 && (m - 1).is_multiple_of(8) => {
                    t.longest_m(m, out.n, max_iter, abs(dz), cn).map(|(e, l)| (*e, l))
                }
                _ => None,
            };
            if let Some((e, len)) = block {
                let (a, b) = entry_maps(&e);
                let (u, v) = (a.apply(dz), b.apply(dc));
                dz = (u.0 + v.0, u.1 + v.1);
                m += len;
                out.n += len;
                out.blocks += 1;
            } else {
                dz = formula.delta_step(zr(m), dz, dc);
                if round32 {
                    dz = (f64::from(dz.0 as f32), f64::from(dz.1 as f32));
                }
                m += 1;
                out.n += 1;
            }
            if finish(&mut dz, &mut m, &mut out) {
                return out;
            }
        }
        out
    }

    /// F5 step-1 gate (CPU model): Mandelbar's matrix BLA changes escape counts no more than
    /// float32 rounding of plain perturbation does, at the antenna tip `1 + i sqrt 3` (a
    /// Misiurewicz point) from 1e-13 to 1e-40 per pixel, and skips most of the orbit at depth;
    /// a block's `A` and `B` equal the composed single-step maps.
    #[test]
    fn mandelbar_matrix_bla_agrees_with_perturbation() {
        let f = Formula::Mandelbar;
        let tip = (1.0, 3f64.sqrt());
        let pts = Reference::compute_f64_with(&Viewport::new(tip.0, tip.1, 1e-10), f, 8000, 65536.0)
            .points
            .to_vec();
        let t = BlaTable::build_for(&pts, BLA_EPSILON, f);
        assert!(t.is_matrix() && t.entries.is_empty());
        // Composition: the level-3 block at m = 9 is the product of its steps.
        let e = t.lookup_m(9, 3).expect("entry");
        let (a, b) = entry_maps(e);
        let (mut aw, mut bw) = (Linear::IDENTITY, Linear::ZERO);
        for j in 9..17 {
            let s = f.jacobian((f64::from(pts[j][0]), f64::from(pts[j][1])));
            aw = aw.then(s);
            bw = bw.then(s).plus(Linear::IDENTITY);
        }
        let close = |x: Linear, y: Linear| {
            let d = x.plus(y.scale(-1.0)).sup();
            d <= 1e-5 * y.sup().max(1.0)
        };
        assert!(close(a, aw) && close(b, bw), "{a:?} {aw:?} / {b:?} {bw:?}");
        for (scale, it, skips) in [(1e-13, 6000usize, false), (1e-20, 8000, true), (1e-40, 8000, true)] {
            let (mut class, mut dmu, mut noise, mut plain, mut bla) = (0, Vec::new(), Vec::new(), 0, 0);
            let mut blocks = 0;
            for j in 0..40 {
                for i in 0..64 {
                    let dc = (
                        (i as f64 - 32.0 + 0.37) * scale * 25.0,
                        (j as f64 - 20.0 + 0.61) * scale * 25.0,
                    );
                    let p = iterate_delta_m(f, &pts, None, dc, it, false);
                    let q = iterate_delta_m(f, &pts, None, dc, it, true);
                    let b = iterate_delta_m(f, &pts, Some(&t), dc, it, false);
                    if p.escaped && q.escaped {
                        noise.push((p.mu() - q.mu()).abs());
                    }
                    plain += p.steps;
                    bla += b.steps;
                    blocks += b.blocks;
                    if p.escaped != b.escaped {
                        class += 1;
                    } else if p.escaped {
                        dmu.push((p.mu() - b.mu()).abs());
                    }
                }
            }
            let median = |v: &mut Vec<f64>| {
                v.sort_by(f64::total_cmp);
                v.get(v.len() / 2).copied().unwrap_or(0.0)
            };
            let (md, mn) = (median(&mut dmu), median(&mut noise));
            let ratio = plain as f64 / bla as f64;
            println!(
                "mandelbar BLA @ {scale:e}: class {class}, median |dmu| {md:.2e} (float32 control {mn:.2e}), step ratio {ratio:.2}, {blocks} blocks"
            );
            assert!(class <= 2, "{class} pixels changed class");
            assert!(md <= 2.0 * mn.max(1e-9), "median |dmu| {md} vs float32 control {mn}");
            if skips {
                assert!(ratio > 1.5, "step ratio {ratio}");
            }
        }
        // The forecast follows the matrix entries.
        let fc = t.forecast(1e-20 * 1600f64.hypot(1000.0) / 2.0, 8000);
        assert!(fc.covered > 0, "{fc:?}");
    }

    /// Diagnostic, not a gate: step ratio and agreement against epsilon at the slow view
    /// (`cargo test --release -p almondbread-core bla::tests::epsilon_sweep -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn epsilon_sweep() {
        let pts = reference(SLOW.0, SLOW.1, 5791);
        for e in [24, 20, 16, 12] {
            let t = BlaTable::build(&pts, 2f64.powi(-e));
            let (mut plain, mut bla, mut differ) = (0, 0, 0);
            for p in 0..4000usize {
                let dc = (
                    ((p % 80) as f64 - 40.0 + 0.37) * 9.2e-13,
                    ((p / 80) as f64 - 25.0 + 0.61) * 9.2e-13,
                );
                let a = iterate_delta(&pts, None, 8, dc, 5791, BAILOUT2);
                let b = iterate_delta(&pts, Some(&t), 1, dc, 5791, BAILOUT2);
                plain += a.steps;
                bla += b.steps;
                differ += usize::from(a.n != b.n);
            }
            println!(
                "eps 2^-{e}: step ratio {:.2}, escape n differs {:.1} %",
                plain as f64 / bla as f64,
                differ as f64 / 40.0
            );
        }
    }
}
