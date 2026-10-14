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
//! Derivatives compose through a block: `dz/dc -> A dz/dc + B` (distance estimate) and
//! `dz/dz_1 -> A dz/dz_1` (interior test), each to the same relative error `eps` per step.
//!
//! **When it pays (M4 measurements, kb: synthesis/benchmarks).** A block only exists while
//! `|dz| < eps |2Z|`; at the depths the f64 reference reaches (>= ~1e-14 per pixel) deltas
//! start at ~1e-11 and leave that range after a few hundred iterations, blocks between the
//! reference's near-returns to 0 are 2-8 steps long, and a GPU lookup costs more than it
//! skips. [`BlaTable::forecast`] estimates the skip for a view; the renderer only uses the
//! table where it promises long blocks (deep views: 1e-20 per pixel 2.4x, 1e-24 47x faster).

use std::time::Instant;

use almondbread_math::FloatExpComplex;

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
    /// Seconds the build took.
    pub build_time: f64,
}

impl BlaTable {
    /// Builds the table from the reference points the GPU iterates against (float32 pairs,
    /// so the coefficients describe exactly the orbit the kernel sees), in f64.
    pub fn build(points: &[[f32; 2]], epsilon: f64) -> Self {
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
            return t;
        }
        let mut level: Vec<Merged> = points[1..1 + singles]
            .iter()
            .map(|&z| Merged::single(z, epsilon))
            .collect();
        let mut lev = 0u32;
        let mut entries: Vec<BlaEntry> = Vec::with_capacity(singles);
        loop {
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
        t
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

    pub fn bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<BlaEntry>()
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
fn frexp(x: f64) -> (f64, i64) {
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

fn pow2(k: i64) -> f64 {
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
struct Xr {
    m: f64,
    e: i64,
}

impl Xr {
    const ZERO: Xr = Xr { m: 0.0, e: 0 };
    fn new(x: f64, e: i64) -> Xr {
        if x == 0.0 || !x.is_finite() {
            return Xr::ZERO;
        }
        let (m, k) = frexp(x);
        Xr { m, e: e + k }
    }
    fn from_f64(x: f64) -> Xr {
        Xr::new(x, 0)
    }
    fn is_zero(self) -> bool {
        self.m == 0.0
    }
    fn mul(self, b: Xr) -> Xr {
        Xr::new(self.m * b.m, self.e + b.e)
    }
    fn div(self, b: Xr) -> Xr {
        if b.is_zero() {
            return Xr::ZERO;
        }
        Xr::new(self.m / b.m, self.e - b.e)
    }
    fn add(self, b: Xr) -> Xr {
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
    fn lt(self, b: Xr) -> bool {
        if self.m <= 0.0 || b.m <= 0.0 {
            return self.m < b.m;
        }
        (self.e, self.m) < (b.e, b.m)
    }
    fn min(self, b: Xr) -> Xr {
        if b.lt(self) { b } else { self }
    }
    fn max(self, b: Xr) -> Xr {
        if self.lt(b) { b } else { self }
    }
    fn gpu(self) -> (f32, i32) {
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
struct Xc {
    re: f64,
    im: f64,
    e: i64,
}

impl Xc {
    const ZERO: Xc = Xc { re: 0.0, im: 0.0, e: 0 };
    const ONE: Xc = Xc { re: 0.5, im: 0.0, e: 1 };
    fn new(re: f64, im: f64, e: i64) -> Xc {
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
    fn is_zero(self) -> bool {
        self.re == 0.0 && self.im == 0.0
    }
    fn mul(self, b: Xc) -> Xc {
        Xc::new(
            self.re * b.re - self.im * b.im,
            self.re * b.im + self.im * b.re,
            self.e + b.e,
        )
    }
    fn add(self, b: Xc) -> Xc {
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
    fn abs(self) -> Xr {
        Xr::new(self.re.hypot(self.im), self.e)
    }
    fn gpu(self) -> ([f32; 2], i32) {
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
struct MergedX {
    a: Xc,
    b: Xc,
    r: Xr,
    k: Xr,
}

const INVALID_X: MergedX = MergedX {
    a: Xc::ZERO,
    b: Xc::ZERO,
    r: Xr::ZERO,
    k: Xr::ZERO,
};

impl MergedX {
    fn single(z: &FloatExpComplex, eps: f64) -> Self {
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

    fn then(&self, y: &MergedX) -> MergedX {
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

    fn entry(&self) -> BlaEntryX {
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

    fn e_out_of_range(&self) -> bool {
        let big = 1i64 << 27;
        self.a.e.abs() > big || self.b.e.abs() > big || self.r.e.abs() > big || self.k.e.abs() > big
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
    pub build_time: f64,
}

impl BlaTableX {
    pub fn build(points: &[FloatExpComplex], epsilon: f64) -> Self {
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
            return t;
        }
        let mut level: Vec<MergedX> = points[1..1 + singles]
            .iter()
            .map(|z| MergedX::single(z, epsilon))
            .collect();
        let mut lev = 0u32;
        let mut entries: Vec<BlaEntryX> = Vec::with_capacity(singles);
        loop {
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
        t
    }

    pub fn bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<BlaEntryX>()
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

    fn reference(re: f64, im: f64, max_iter: usize) -> Vec<[f32; 2]> {
        Reference::compute_f64(&Viewport::new(re, im, 1e-10), max_iter, 65536.0).points
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
