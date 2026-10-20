//! Linear approximation with period-aligned stages (Imagina-style LA) and the approximation
//! table (AT) for the power-2 Mandelbrot (kb: techniques/linear-approximation-imagina, M6).
//!
//! Our formulation, built on the BLA validity of [`crate::bla`] (per-pixel `c`, `R` and `K`):
//!
//! **Step.** A step starts at a *boundary* `s` of the reference (a near-return of `Z` towards 0)
//! and ends at the next boundary `s'` of its stage. Its first iteration is exact (non-linear),
//! `u = dz (2 Z_s + dz) + dc`, the rest is the affine block over the inner points `s+1 .. s'-1`:
//! `dz_{s'} = A u + B dc`, valid while `|u| + K |dc| < R` (the BLA bound of that block). So a
//! step is "one ordinary perturbation step, then a BLA block that starts right after the
//! near-return": the near-return, where `|Z|` is smallest and a BLA radius `eps |2Z|` collapses,
//! is never an inner point of the step that starts there.
//!
//! **Stages.** Stage 0 cuts the reference at near-returns (a point more than 2^6 below the
//! smallest `|Z|` of the step so far, or one whose radius collapses by 2^10). Stage `k+1`
//! composes consecutive stage-`k` steps, linearising the boundaries in between (each becomes a
//! single BLA step `A = 2 Z_b`), and cuts at a boundary more than 2^3 below the smallest
//! boundary it linearised (a deeper return: the reference's period structure), at a radius
//! collapse, or after 32 steps (a synthetic cut, so stages coarsen by at most 32x). Stages stop
//! when one no longer coarsens. The thresholds follow Imagina's (src: kb
//! sources/imagina-hinfla-evaluator §LAInfo::Step, §CreateLAFromOrbit); the bounds are ours.
//!
//! **Lookup (GPU and [`iterate_model`]).** A pixel does ordinary steps; right after the exact step
//! *from* a stage-0 boundary `b_j` (at `m = b_j + 1`, with `dz = u`) it looks up the chain of
//! steps starting at `b_j` (stage 0, 1, ... while `b_j` is a boundary of that stage; validity
//! is monotone up the chain: each is a prefix of the next) and applies the longest valid block.
//! The rule depends on `m` alone, so results do not depend on how chunks cut the orbit. A
//! valid block ends at a boundary of its stage (hence of stage 0): the next lookup follows one
//! exact step later. Rebasing brings the pixel to `m = 0 = b_0`, so it re-enters the chains
//! after every rebase ("skip one period, rebase" at a minibrot).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use almondbread_math::FloatExpComplex;

use crate::bla::{BlaEntryX, MergedX, XZERO_EXP, Xc, Xr};

/// Stage 0 cuts at a point this far (log2) below the smallest |Z| of the step so far.
pub const NEAR_RETURN_0_LOG2: f64 = 6.0;
/// Higher stages cut at a boundary this far (log2) below the smallest boundary linearised.
pub const NEAR_RETURN_LOG2: f64 = 3.0;
/// A radius collapsing by this much (log2) when a point is added cuts the step there.
pub const COLLAPSE_LOG2: f64 = 10.0;
/// A natural stage step longer than this many steps of the stage below is split ...
pub const SYNTHETIC_MAX: usize = 16;
/// ... into pieces so that stages coarsen by about this factor.
pub const SYNTHETIC_FACTOR: f64 = 4.0;
/// Upper bound on stages (a guard; real tables have ~log_2 of the boundaries).
pub const MAX_STAGES: usize = 64;

/// A boundary's chain as built: per stage, the block and where it ends.
type Chain = Vec<(Option<MergedX>, usize)>;

/// Construction parameters (defaults: the constants above).
#[derive(Clone, Copy, Debug)]
pub struct LaBuild {
    pub near0_log2: f64,
    pub near_log2: f64,
    pub collapse_log2: f64,
    pub synthetic_max: usize,
    pub synthetic_factor: f64,
}

impl Default for LaBuild {
    fn default() -> Self {
        LaBuild {
            near0_log2: NEAR_RETURN_0_LOG2,
            near_log2: NEAR_RETURN_LOG2,
            collapse_log2: COLLAPSE_LOG2,
            synthetic_max: SYNTHETIC_MAX,
            synthetic_factor: SYNTHETIC_FACTOR,
        }
    }
}

/// The kernel's LA parameters (`LaParams` in MSL, 16 bytes): the lookup boundary count.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LaParams {
    pub count: u32,
    pub pad: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<LaParams>() == 16);

/// One step of a stage as built: the block over the inner points (None: no inner point, the
/// step is a single exact iteration), its start and end in the reference.
#[derive(Clone, Copy, Debug)]
struct Step {
    start: usize,
    end: usize,
    block: Option<MergedX>,
    /// Starts at a natural cut (a near-return or a radius collapse), not a synthetic one.
    natural: bool,
}

fn then_opt(x: Option<MergedX>, y: &MergedX) -> MergedX {
    match x {
        None => *y,
        Some(x) => x.then(y),
    }
}

fn log2_xr(x: Xr) -> f64 {
    if x.is_zero() {
        f64::NEG_INFINITY
    } else {
        x.m.log2() + x.e as f64
    }
}

fn log2_abs(z: &FloatExpComplex) -> f64 {
    let m = f64::from(z.mantissa[0]).hypot(f64::from(z.mantissa[1]));
    if m == 0.0 {
        f64::NEG_INFINITY
    } else {
        m.log2() + f64::from(z.exponent)
    }
}

fn r_log2(b: &Option<MergedX>) -> f64 {
    b.as_ref().map_or(f64::INFINITY, |b| log2_xr(b.r))
}

/// Composes consecutive `prev` steps into the steps of the next stage (see the module docs):
/// natural cuts at a boundary more than `near_log2` below the smallest boundary linearised so
/// far, or where linearising it collapses the radius; a natural step of more than
/// [`SYNTHETIC_MAX`] steps is split into pieces of `c^(1/round(log_F c))` steps
/// (F = [`SYNTHETIC_FACTOR`]), so stages coarsen geometrically where the reference has no
/// period structure (Imagina's synthetic step length).
fn compose_stage(
    prev: &[Step],
    zl: &[f64],
    single: &dyn Fn(usize) -> MergedX,
    near_log2: f64,
    params: &LaBuild,
) -> Vec<Step> {
    let merge = |comp: &Step, st: &Step| -> Step {
        let with_b = then_opt(comp.block, &single(st.start));
        Step {
            start: comp.start,
            end: st.end,
            block: Some(match st.block {
                None => with_b,
                Some(y) => with_b.then(&y),
            }),
            natural: comp.natural,
        }
    };
    // Natural segments [i0, i1) of prev.
    let mut cuts = vec![0usize];
    let mut comp = prev[0];
    let mut minz = f64::INFINITY;
    for (i, st) in prev.iter().enumerate().skip(1) {
        let b = st.start;
        let with_b = then_opt(comp.block, &single(b));
        let near = minz.is_finite() && zl[b] < minz - near_log2;
        let collapse = comp.block.is_some() && log2_xr(with_b.r) < r_log2(&comp.block) - params.collapse_log2;
        if near || collapse {
            cuts.push(i);
            comp = *st;
            minz = f64::INFINITY;
            continue;
        }
        comp = merge(&comp, st);
        minz = minz.min(zl[b]);
    }
    cuts.push(prev.len());
    let mut out = Vec::new();
    for w in cuts.windows(2) {
        let (i0, i1) = (w[0], w[1]);
        let c = i1 - i0;
        let piece = if c > params.synthetic_max {
            let k = ((c as f64).ln() / params.synthetic_factor.ln()).round().max(1.0);
            ((c as f64).powf(1.0 / k).round() as usize).max(2)
        } else {
            c
        };
        let mut i = i0;
        while i < i1 {
            let e = (i + piece).min(i1);
            let mut comp = prev[i];
            comp.natural = i == i0;
            for st in &prev[i + 1..e] {
                comp = merge(&comp, st);
            }
            out.push(comp);
            i = e;
        }
    }
    out
}

/// One table entry as the deep kernel reads it (`LaEntryX` in MSL, 48 bytes): the block of a
/// step (`A`, `B`, `R`, `K` exactly as [`BlaEntryX`]) applied at `m = start + 1`, its length
/// `len` (iterations of the block: step length - 1; 0 = no block) and the stage-0 index of the
/// boundary it ends at (`end_j`; the table's boundary count when it ends at the reference's end).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LaEntryX {
    pub block: BlaEntryX,
    pub len: u32,
    pub end_j: u32,
}

const _: () = assert!(std::mem::size_of::<LaEntryX>() == 48);

/// The plain kernel's entry (`LaEntry` in MSL, 32 bytes): `A`, `B` complex float32, `R`, `K`,
/// as [`crate::bla::BlaEntry`]; `r = 0`: never valid.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LaEntry {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub r: f32,
    pub k: f32,
    pub len: u32,
    pub end_j: u32,
}

const _: () = assert!(std::mem::size_of::<LaEntry>() == 32);

/// Largest `|A|` and `|B|` a plain entry may carry: the plain kernel multiplies its interior
/// derivative (kept below 2^60) by `A`, which must stay in float32 range.
pub const PLAIN_MAX_COEFFICIENT: f64 = 1_099_511_627_776.0; // 2^40

impl LaEntryX {
    fn from_step(block: &Option<MergedX>, len: usize, end_j: usize) -> Self {
        let block = match block {
            // No inner point: an identity block, always valid, skipping nothing.
            None => BlaEntryX {
                a: [0.5, 0.0],
                ae: 1,
                b: [0.0, 0.0],
                be: XZERO_EXP,
                r: 0.5,
                re: 1 << 27,
                k: 0.0,
                ke: XZERO_EXP,
            },
            Some(b) => b.entry(),
        };
        LaEntryX {
            block,
            len: len as u32,
            end_j: end_j as u32,
        }
    }

    fn r(&self) -> Xr {
        Xr::new(f64::from(self.block.r), i64::from(self.block.re))
    }

    fn k(&self) -> Xr {
        Xr::new(f64::from(self.block.k), i64::from(self.block.ke))
    }

    fn a(&self) -> Xc {
        Xc::new(
            f64::from(self.block.a[0]),
            f64::from(self.block.a[1]),
            i64::from(self.block.ae),
        )
    }

    fn b(&self) -> Xc {
        Xc::new(
            f64::from(self.block.b[0]),
            f64::from(self.block.b[1]),
            i64::from(self.block.be),
        )
    }

    /// `|u| + K |c| < R`.
    fn valid(&self, un: Xr, cn: Xr) -> bool {
        self.block.r > 0.0 && un.add(self.k().mul(cn)).lt(self.r())
    }

    /// The plain kernel's form: float32 coefficients, or an entry that is never valid.
    fn plain(&self) -> LaEntry {
        let e = &self.block;
        let f = |m: f32, x: i32| -> f64 { f64::from(m) * crate::bla::pow2(i64::from(x)) };
        let (a0, a1) = (f(e.a[0], e.ae), f(e.a[1], e.ae));
        let (b0, b1) = (f(e.b[0], e.be), f(e.b[1], e.be));
        let r = if e.r > 0.0 {
            f(e.r, e.re).min(f64::from(f32::MAX))
        } else {
            0.0
        };
        let k = f(e.k, e.ke);
        let ok = r >= f64::from(f32::MIN_POSITIVE)
            && a0.hypot(a1) <= PLAIN_MAX_COEFFICIENT
            && b0.hypot(b1) <= PLAIN_MAX_COEFFICIENT
            && k <= f64::from(f32::MAX);
        if !ok {
            return LaEntry {
                len: self.len,
                end_j: self.end_j,
                ..LaEntry::default()
            };
        }
        LaEntry {
            a: [a0 as f32, a1 as f32],
            b: [b0 as f32, b1 as f32],
            r: r as f32,
            k: k as f32,
            len: self.len,
            end_j: self.end_j,
        }
    }
}

/// The LA table of one reference orbit (any depth: built from the floatexp points).
#[derive(Clone, Debug, Default)]
pub struct LaTable {
    /// Reference points (`last = ref_len - 1`).
    pub ref_len: usize,
    pub epsilon: f64,
    /// Stage-0 boundaries `b_j` (ascending, `b_0 = 0`) with the offset of their chain in
    /// `entries`: `(b_j, offset_j)`; one extra pair at the end `(last, entries.len())`.
    pub bounds: Vec<[u32; 2]>,
    /// Chains: for boundary `j`, entries `offset_j .. offset_{j+1}`, stage 0 upwards.
    pub entries: Vec<LaEntryX>,
    /// Per reference index `m`: the stage-0 index `j` of the next lookup, the smallest `j` with
    /// `b_j + 1 >= m` (the boundary count when there is none).
    pub next_j: Vec<u32>,
    /// Steps per stage (diagnostics).
    pub stage_steps: Vec<usize>,
    pub build_time: f64,
}

impl LaTable {
    /// Builds the stages from the reference the GPU iterates against (floatexp points; plain
    /// references convert their float32 points with [`LaTable::build_plain`]).
    pub fn build(points: &[FloatExpComplex], epsilon: f64) -> Self {
        Self::build_with(points, epsilon, &LaBuild::default())
    }

    /// [`LaTable::build`] with explicit construction parameters (studies).
    pub fn build_with(points: &[FloatExpComplex], epsilon: f64, params: &LaBuild) -> Self {
        Self::build_cancellable(points, epsilon, params, &AtomicBool::new(false)).expect("never cancelled")
    }

    /// [`LaTable::build_with`] that polls `cancel` before each stage and before the chains (the
    /// renderer cancels the build of a reference it replaced); None when cancelled.
    pub fn build_cancellable(
        points: &[FloatExpComplex],
        epsilon: f64,
        params: &LaBuild,
        cancel: &AtomicBool,
    ) -> Option<Self> {
        let cancelled = || cancel.load(Ordering::Relaxed);
        let start = Instant::now();
        let mut t = LaTable {
            ref_len: points.len(),
            epsilon,
            ..Default::default()
        };
        let last = points.len().saturating_sub(1);
        if last < 2 {
            t.bounds = vec![[0, 0], [last as u32, 0]];
            t.next_j = vec![1; points.len()];
            t.build_time = start.elapsed().as_secs_f64();
            return Some(t);
        }
        if cancelled() {
            return None;
        }
        let single = |k: usize| MergedX::single(&points[k], epsilon);
        let zl: Vec<f64> = points.iter().map(log2_abs).collect();

        // Stage 0 composes single iterations, stage k+1 the steps of stage k.
        let units: Vec<Step> = (0..last)
            .map(|k| Step {
                start: k,
                end: k + 1,
                block: None,
                natural: true,
            })
            .collect();
        let mut stages = vec![compose_stage(&units, &zl, &single, params.near0_log2, params)];
        while stages.len() < MAX_STAGES {
            if cancelled() {
                return None;
            }
            let prev = stages.last().expect("stage");
            if prev.len() <= 1 {
                break;
            }
            let next = compose_stage(prev, &zl, &single, params.near_log2, params);
            if next.len() == prev.len() {
                break;
            }
            stages.push(next);
        }
        t.stage_steps = stages.iter().map(Vec::len).collect();
        if cancelled() {
            return None;
        }

        // Chains per stage-0 boundary. A natural boundary b is a near-return: its lookups are at
        // L = b + 1, after the exact step from b. A synthetic one is linearised too: L = b. L must
        // grow strictly from one kept boundary to the next (a synthetic boundary right after a
        // natural one is treated as natural). Blocks of no iterations are left out, and a
        // boundary whose chain is then empty has no lookup.
        let s0 = &stages[0];
        let mut ptr = vec![0usize; stages.len()];
        let mut kept: Vec<(usize, Chain)> = Vec::new();
        for st0 in s0 {
            let prev_look = kept.last().map(|k| k.0);
            let natural = st0.natural || prev_look.is_some_and(|l| st0.start <= l);
            let look = st0.start + usize::from(natural);
            let mut chain = Vec::new();
            for (si, stage) in stages.iter().enumerate() {
                while ptr[si] < stage.len() && stage[ptr[si]].start < st0.start {
                    ptr[si] += 1;
                }
                if ptr[si] >= stage.len() || stage[ptr[si]].start != st0.start {
                    break;
                }
                let step = &stage[ptr[si]];
                if step.end <= look {
                    continue;
                }
                let block = if natural {
                    step.block
                } else {
                    Some(match step.block {
                        None => single(st0.start),
                        Some(b) => single(st0.start).then(&b),
                    })
                };
                chain.push((block, step.end));
            }
            if !chain.is_empty() {
                kept.push((look, chain));
            }
        }
        let looks: Vec<usize> = kept.iter().map(|k| k.0).collect();
        let next_of = |pos: usize| -> usize { looks.partition_point(|&l| l < pos) };
        for (look, chain) in &kept {
            t.bounds.push([*look as u32, t.entries.len() as u32]);
            for (block, end) in chain {
                t.entries.push(LaEntryX::from_step(block, end - look, next_of(*end)));
            }
        }
        t.bounds.push([last as u32, t.entries.len() as u32]);

        // Next lookup per m: the smallest j with L_j >= m.
        t.next_j = (0..points.len()).map(|m| next_of(m) as u32).collect();

        t.build_time = start.elapsed().as_secs_f64();
        Some(t)
    }

    /// [`LaTable::build`] for a float32 reference (f64 views and plain-kernel big references).
    pub fn build_plain(points: &[[f32; 2]], epsilon: f64) -> Self {
        Self::build_plain_with(points, epsilon, &LaBuild::default())
    }

    /// [`LaTable::build_plain`] with explicit construction parameters.
    pub fn build_plain_with(points: &[[f32; 2]], epsilon: f64, params: &LaBuild) -> Self {
        Self::build_plain_cancellable(points, epsilon, params, &AtomicBool::new(false)).expect("never cancelled")
    }

    /// [`LaTable::build_plain_with`] polling `cancel` ([`LaTable::build_cancellable`]).
    pub fn build_plain_cancellable(
        points: &[[f32; 2]],
        epsilon: f64,
        params: &LaBuild,
        cancel: &AtomicBool,
    ) -> Option<Self> {
        let x: Vec<FloatExpComplex> = points
            .iter()
            .map(|p| {
                let a = p[0].abs().max(p[1].abs());
                if a == 0.0 {
                    return FloatExpComplex::ZERO;
                }
                let (_, e) = crate::bla::frexp(f64::from(a));
                let s = crate::bla::pow2(-e) as f32;
                FloatExpComplex {
                    mantissa: [p[0] * s, p[1] * s],
                    exponent: e as i32,
                }
            })
            .collect();
        Self::build_cancellable(&x, epsilon, params, cancel)
    }

    /// The kernel's parameters.
    pub fn gpu_params(&self) -> LaParams {
        LaParams {
            count: self.boundaries() as u32,
            ..LaParams::default()
        }
    }

    pub fn boundaries(&self) -> usize {
        self.bounds.len().saturating_sub(1)
    }

    pub fn bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<LaEntryX>() + self.bounds.len() * 8 + self.next_j.len() * 4
    }

    /// The chain of boundary `j` as text: per stage, length, log2 R, log2 K, log2 |A|, log2 |B|
    /// (diagnostics).
    pub fn describe_chain(&self, j: usize) -> String {
        let mut out = format!("boundary {j} lookup at m = {}:", self.bounds[j][0]);
        for e in self.chain(j) {
            out += &format!(
                " [len {} R 2^{:.1} K 2^{:.1} A 2^{:.1} B 2^{:.1} -> j {}]",
                e.len,
                log2_xr(e.r()),
                log2_xr(e.k()),
                log2_xr(e.a().abs()),
                log2_xr(e.b().abs()),
                e.end_j
            );
        }
        out
    }

    /// The plain kernel's entries (same order as `entries`).
    pub fn plain_entries(&self) -> Vec<LaEntry> {
        self.entries.iter().map(LaEntryX::plain).collect()
    }

    /// The chain of boundary `j`.
    fn chain(&self, j: usize) -> &[LaEntryX] {
        let (a, b) = (self.bounds[j][1] as usize, self.bounds[j + 1][1] as usize);
        &self.entries[a..b]
    }

    /// The longest valid block of boundary `j`'s chain for `|u| = un`, `|dc| = cn` at
    /// iteration `n`: (entry, tests made).
    fn longest(
        &self,
        j: usize,
        n: usize,
        max_iter: usize,
        un: Xr,
        cn: Xr,
        hint: usize,
    ) -> (Option<(usize, &LaEntryX)>, usize) {
        // Validity is monotone along the chain: start at the stage that served last time and
        // move up while valid or down until valid (about two tests on the GPU).
        let c = self.chain(j);
        let ok = |i: usize| n + c[i].len as usize <= max_iter && c[i].valid(un, cn);
        let mut tests = 1;
        let mut i = hint.min(c.len() - 1);
        if ok(i) {
            while i + 1 < c.len() {
                tests += 1;
                if !ok(i + 1) {
                    break;
                }
                i += 1;
            }
            (Some((i, &c[i])), tests)
        } else {
            while i > 0 {
                i -= 1;
                tests += 1;
                if ok(i) {
                    return (Some((i, &c[i])), tests);
                }
            }
            (None, tests)
        }
    }
}

/// Outcome of the CPU model [`iterate_model`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelOutcome {
    pub n: usize,
    pub escaped: bool,
    /// Loop steps: exact steps plus blocks.
    pub steps: usize,
    pub blocks: usize,
    /// Table entries tested.
    pub tests: usize,
    /// log2 |z| at escape.
    pub log2_r: f64,
}

impl ModelOutcome {
    /// Smooth iteration count `n + 1 - log2(log|z|)`.
    pub fn mu(&self) -> f64 {
        self.n as f64 + 1.0 - (self.log2_r * std::f64::consts::LN_2).log2()
    }
}

fn xc_of(z: &FloatExpComplex) -> Xc {
    Xc::new(
        f64::from(z.mantissa[0]),
        f64::from(z.mantissa[1]),
        i64::from(z.exponent),
    )
}

fn log2_xc(z: Xc) -> f64 {
    log2_xr(z.abs())
}

/// Which skip method the CPU model uses.
#[derive(Clone, Copy, Debug)]
pub enum Skip<'a> {
    None,
    /// Deep BLA with lookups at `m = 1 (mod every)` (the GPU policy).
    Bla(&'a crate::bla::BlaTableX, usize),
    La(&'a LaTable),
}

/// CPU model of a deep pixel (extended range throughout) against the floatexp reference:
/// exact perturbation steps with rebasing, plus the chosen skip method. For tests and the
/// M6 study (`examples/la_check.rs`).
/// `dc = (re + i im) 2^exp`.
pub fn iterate_model(
    points: &[FloatExpComplex],
    skip: Skip<'_>,
    dc: (f64, f64, i64),
    max_iter: usize,
    bailout2: f64,
) -> ModelOutcome {
    let dc = Xc::new(dc.0, dc.1, dc.2);
    let trace = TRACE.with(|t| t.get());
    let mut run = 0usize;
    let last = points.len() - 1;
    let cn = dc.abs();
    let half_log2_b2 = 0.5 * bailout2.log2();
    let mut out = ModelOutcome::default();
    let (mut dz, mut m, mut n) = (Xc::ZERO, 0usize, 0usize);
    let mut j = 0usize; // LA: next lookup boundary
    let mut hint = 0usize; // LA: the stage that served last
    loop {
        if n >= max_iter {
            out.n = n;
            return out;
        }
        // Exact step.
        run += 1;
        let zr = xc_of(&points[m]);
        let two_z = Xc::new(zr.re * 2.0, zr.im * 2.0, zr.e);
        dz = two_z.add(dz).mul(dz).add(dc);
        m += 1;
        n += 1;
        out.steps += 1;
        // Skip.
        match skip {
            Skip::Bla(t, every) => {
                while m != 0 && (m - 1).is_multiple_of(every) && n < max_iter {
                    let Some((e, len)) = t.longest_pub(m, n, max_iter, dz.abs(), cn) else {
                        break;
                    };
                    out.tests += 1;
                    let a = Xc::new(f64::from(e.a[0]), f64::from(e.a[1]), i64::from(e.ae));
                    let b = Xc::new(f64::from(e.b[0]), f64::from(e.b[1]), i64::from(e.be));
                    dz = a.mul(dz).add(b.mul(dc));
                    m += len;
                    n += len;
                    out.steps += 1;
                    out.blocks += 1;
                    if finish(points, &mut dz, &mut m, last, half_log2_b2, &mut out) {
                        out.n = n;
                        return out;
                    }
                }
            }
            Skip::La(t) => {
                while j < t.boundaries() && t.bounds[j][0] as usize == m && m < last {
                    let (best, tests) = t.longest(j, n, max_iter, dz.abs(), cn, hint);
                    hint = best.map_or(0, |b| b.0);
                    let best = best.map(|b| b.1);
                    out.tests += tests;
                    match best {
                        Some(e) if e.len > 0 => {
                            if trace {
                                println!(
                                    "  n {n} m {m}: {run} exact, |u| 2^{:.1}, block {} (tests {tests})",
                                    log2_xc(dz),
                                    e.len
                                );
                                run = 0;
                            }
                            dz = e.a().mul(dz).add(e.b().mul(dc));
                            m += e.len as usize;
                            n += e.len as usize;
                            out.steps += 1;
                            out.blocks += 1;
                            j = e.end_j as usize;
                            if finish(points, &mut dz, &mut m, last, half_log2_b2, &mut out) {
                                out.n = n;
                                return out;
                            }
                            if m == 0 {
                                j = 0;
                            }
                        }
                        _ => {
                            j += 1;
                            break;
                        }
                    }
                }
            }
            Skip::None => {}
        }
        let before = m;
        if finish(points, &mut dz, &mut m, last, half_log2_b2, &mut out) {
            out.n = n;
            return out;
        }
        if m == 0 {
            if trace && before != 0 {
                println!("  n {n} m {before}: {run} exact, rebase, |z| 2^{:.1}", log2_xc(dz));
                run = 0;
            }
            j = 0;
        }
    }
}

thread_local! {
    /// Diagnostics: print the LA model's events.
    pub static TRACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Escape and rebase tests at `m`; true when escaped.
fn finish(
    points: &[FloatExpComplex],
    dz: &mut Xc,
    m: &mut usize,
    last: usize,
    half_log2_b2: f64,
    out: &mut ModelOutcome,
) -> bool {
    let z = xc_of(&points[*m]).add(*dz);
    let lz = log2_xc(z);
    if lz > half_log2_b2 {
        out.escaped = true;
        out.log2_r = lz;
        return true;
    }
    if *m == last || z.abs().lt(dz.abs()) {
        *dz = z;
        *m = 0;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bla::BLA_EPSILON;
    use crate::reference::Reference;
    use crate::viewport::Viewport;

    #[test]
    fn chains_and_next() {
        let r = Reference::compute_f64(
            &Viewport::new(-1.4080321340304542, 0.13680236432900417, 1e-10),
            6000,
            65536.0,
        );
        let t = LaTable::build_plain(&r.points, BLA_EPSILON);
        println!(
            "stages {:?}, {} entries, {} boundaries",
            t.stage_steps,
            t.entries.len(),
            t.boundaries()
        );
        assert_eq!(t.bounds[0][0], 1, "the first lookup follows the step from z_0 = 0");
        let look = |j: usize| t.bounds[j][0] as usize;
        for j in 0..t.boundaries() {
            if j > 0 {
                assert!(look(j) > look(j - 1), "lookup indices grow strictly");
            }
            let c = t.chain(j);
            assert!(!c.is_empty());
            for (k, e) in c.iter().enumerate() {
                assert!(e.len > 0);
                let end = look(j) + e.len as usize;
                assert!(end < t.ref_len);
                // The next lookup after the block is the first one at or after its end.
                assert_eq!(e.end_j, t.next_j[end]);
                // Monotone along the chain: R no larger, K no smaller (the GPU search relies on it).
                if k > 0 && e.block.r > 0.0 {
                    assert!(
                        !c[k - 1].r().lt(e.r()) && !e.k().lt(c[k - 1].k()),
                        "chain {j} stage {k}"
                    );
                }
            }
        }
        for m in 0..t.ref_len {
            let j = t.next_j[m] as usize;
            if j < t.boundaries() {
                assert!(look(j) >= m && (j == 0 || look(j - 1) < m));
            }
        }
    }

    /// The model with LA agrees with exact perturbation (class, median smooth count) and skips,
    /// around the M4 slow view at 1e-20 per pixel (an f64 reference, as the BLA test).
    #[test]
    fn model_agrees_with_perturbation() {
        let r = Reference::compute_f64(
            &Viewport::new(-1.4080321340304542, 0.13680236432900417, 1e-10),
            12000,
            65536.0,
        );
        let pts: Vec<FloatExpComplex> = r
            .points
            .iter()
            .map(|p| {
                let a = p[0].abs().max(p[1].abs());
                if a == 0.0 {
                    return FloatExpComplex::ZERO;
                }
                let (_, e) = crate::bla::frexp(f64::from(a));
                let s = crate::bla::pow2(-e) as f32;
                FloatExpComplex {
                    mantissa: [p[0] * s, p[1] * s],
                    exponent: e as i32,
                }
            })
            .collect();
        let t = LaTable::build(&pts, BLA_EPSILON);
        let (mut class, mut dmu, mut exact, mut la) = (0, Vec::new(), 0usize, 0usize);
        for j in 0..20 {
            for i in 0..32 {
                let dc = (
                    (i as f64 - 16.0 + 0.37) * 2.5e-19,
                    (j as f64 - 10.0 + 0.61) * 2.5e-19,
                    0,
                );
                let a = iterate_model(&pts, Skip::None, dc, 12000, 65536.0 * 65536.0);
                let b = iterate_model(&pts, Skip::La(&t), dc, 12000, 65536.0 * 65536.0);
                exact += a.steps;
                la += b.steps;
                if a.escaped != b.escaped {
                    class += 1;
                } else if a.escaped {
                    dmu.push((a.mu() - b.mu()).abs());
                }
            }
        }
        dmu.sort_by(f64::total_cmp);
        let median = dmu.get(dmu.len() / 2).copied().unwrap_or(0.0);
        println!("class {class}, median |dmu| {median:.2e}, steps {exact} -> {la}");
        assert!(class <= 2);
        assert!(median < 1e-3);
        assert!(la * 4 < exact, "{la} vs {exact}");
    }
}
