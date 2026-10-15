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
//! **Lookup (GPU and [`iterate_la`]).** A pixel does ordinary steps; right after the exact step
//! *from* a stage-0 boundary `b_j` (at `m = b_j + 1`, with `dz = u`) it looks up the chain of
//! steps starting at `b_j` (stage 0, 1, ... while `b_j` is a boundary of that stage; validity
//! is monotone up the chain: each is a prefix of the next) and applies the longest valid block.
//! The rule depends on `m` alone, so results do not depend on how chunks cut the orbit. A
//! valid block ends at a boundary of its stage (hence of stage 0): the next lookup follows one
//! exact step later. Rebasing brings the pixel to `m = 0 = b_0`, so it re-enters the chains
//! after every rebase ("skip one period, rebase" at a minibrot).
//!
//! **AT.** For a periodic (nucleus) reference of period `P`, the top-stage step `[0, P)` maps
//! the full `z` at a period boundary to the next: `z' = Z_P + A (z^2 + dc) + B dc`. In
//! `y = A z` that is the Mandelbrot map itself, `y' = y^2 + c'` with `c' = A (A + B) dc +
//! A Z_P`, valid while the step is (`|z|^2 + K |dc| < R`, i.e. `|y|^2 < |A|^2 (R - K |dc|)`).
//! A pixel at `m = 0` iterates `y` in float32 while that holds, `P` iterations per step
//! (kb: techniques/linear-approximation-imagina "AT"; knighty's renormalisation).

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
pub const SYNTHETIC_MAX: usize = 64;
/// ... into pieces so that stages coarsen by about this factor.
pub const SYNTHETIC_FACTOR: f64 = 16.0;
/// Upper bound on stages (a guard; real tables have ~log_2 of the boundaries).
pub const MAX_STAGES: usize = 64;

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

/// The kernel's LA parameters (`LaParams` in MSL, 96 bytes): the lookup boundary count and the
/// AT of a periodic reference (`period` 0: none). Complex values are floatexp (mantissa with
/// max-abs in [0.5, 1), exponent), reals mantissa + exponent.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LaParams {
    pub count: u32,
    pub period: u32,
    pub pad0: u32,
    pub pad1: u32,
    pub at_a: [f32; 2],
    pub at_ae: i32,
    pub pad2: i32,
    pub at_ai: [f32; 2],
    pub at_aie: i32,
    pub pad3: i32,
    pub at_cc: [f32; 2],
    pub at_cce: i32,
    pub pad4: i32,
    pub at_rc: [f32; 2],
    pub at_rce: i32,
    pub pad5: i32,
    pub at_r: f32,
    pub at_re: i32,
    pub at_k1: f32,
    pub at_k1e: i32,
}

const _: () = assert!(std::mem::size_of::<LaParams>() == 96);

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
    if x.is_zero() { f64::NEG_INFINITY } else { x.m.log2() + x.e as f64 }
}

fn log2_abs(z: &FloatExpComplex) -> f64 {
    let m = f64::from(z.mantissa[0]).hypot(f64::from(z.mantissa[1]));
    if m == 0.0 { f64::NEG_INFINITY } else { m.log2() + f64::from(z.exponent) }
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
        Xc::new(f64::from(self.block.a[0]), f64::from(self.block.a[1]), i64::from(self.block.ae))
    }

    fn b(&self) -> Xc {
        Xc::new(f64::from(self.block.b[0]), f64::from(self.block.b[1]), i64::from(self.block.be))
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
        let r = if e.r > 0.0 { f(e.r, e.re).min(f64::from(f32::MAX)) } else { 0.0 };
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
    /// Approximation table of a periodic reference, if usable.
    pub at: Option<AtInfo>,
    pub build_time: f64,
}

/// The AT of a periodic reference (see the module docs): `y = A z`, `y <- y^2 + c'` with
/// `c' = cc dc + ref_c`, valid while `|y|^2 < |A|^2 (R - K |dc|)`; each step is `period`
/// iterations.
#[derive(Clone, Copy, Debug)]
pub struct AtInfo {
    pub period: usize,
    pub(crate) a: Xc,
    /// `A (A + B)`.
    pub(crate) cc: Xc,
    /// `A Z_P` (0 for an exact nucleus).
    pub(crate) ref_c: Xc,
    pub(crate) r: Xr,
    pub(crate) k: Xr,
}

impl LaTable {
    /// Builds the stages from the reference the GPU iterates against (floatexp points; plain
    /// references convert their float32 points with [`LaTable::build_plain`]).
    pub fn build(points: &[FloatExpComplex], epsilon: f64) -> Self {
        Self::build_with(points, epsilon, &LaBuild::default())
    }

    /// [`LaTable::build`] with explicit construction parameters (studies).
    pub fn build_with(points: &[FloatExpComplex], epsilon: f64, params: &LaBuild) -> Self {
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
            return t;
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
        let mut stages = vec![compose_stage(&units, &zl, &single, params.near0_log2, &params)];
        while stages.len() < MAX_STAGES {
            let prev = stages.last().expect("stage");
            if prev.len() <= 1 {
                break;
            }
            let next = compose_stage(prev, &zl, &single, params.near_log2, &params);
            if next.len() == prev.len() {
                break;
            }
            stages.push(next);
        }
        t.stage_steps = stages.iter().map(Vec::len).collect();

        // Chains per stage-0 boundary. A natural boundary b is a near-return: its lookups are at
        // L = b + 1, after the exact step from b. A synthetic one is linearised too: L = b. L must
        // grow strictly from one kept boundary to the next (a synthetic boundary right after a
        // natural one is treated as natural). Blocks of no iterations are left out, and a
        // boundary whose chain is then empty has no lookup.
        let s0 = &stages[0];
        let mut ptr = vec![0usize; stages.len()];
        let mut kept: Vec<(usize, Vec<(Option<MergedX>, usize)>)> = Vec::new();
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

        t.at = Self::make_at(points, &stages, epsilon);
        t.build_time = start.elapsed().as_secs_f64();
        t
    }

    /// [`LaTable::build`] for a float32 reference (f64 views and plain-kernel big references).
    pub fn build_plain(points: &[[f32; 2]], epsilon: f64) -> Self {
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
        Self::build(&x, epsilon)
    }

    /// The AT, from a top-stage step that covers the whole reference `[0, last)`: only for a
    /// periodic reference (its end point `Z_P` near 0, so the step closes a period).
    fn make_at(points: &[FloatExpComplex], stages: &[Vec<Step>], epsilon: f64) -> Option<AtInfo> {
        let last = points.len() - 1;
        // Periodic: |Z_P| far below the orbit's scale (a nucleus reference).
        if last < 2 || log2_abs(&points[last]) > -20.0 {
            return None;
        }
        // The period step [0, P): the top stage's steps composed (its boundaries linearised).
        let top = stages.last()?;
        let mut block = top[0].block;
        for st in &top[1..] {
            let with_b = then_opt(block, &MergedX::single(&points[st.start], epsilon));
            block = Some(match st.block {
                None => with_b,
                Some(y) => with_b.then(&y),
            });
        }
        let block = block?;
        if block.r.is_zero() {
            return None;
        }
        let a = block.a;
        let ab = a.add(block.b);
        let zp = Xc::new(
            f64::from(points[last].mantissa[0]),
            f64::from(points[last].mantissa[1]),
            i64::from(points[last].exponent),
        );
        let at = AtInfo {
            period: last,
            a,
            cc: a.mul(ab),
            ref_c: a.mul(zp),
            r: block.r,
            k: block.k,
        };
        // Useful only if y may pass |y| = 2 (Imagina: SqrEscapeRadius > 4).
        if log2_xr(at.a.abs()) * 2.0 + log2_xr(at.r) < 2.0 {
            return None;
        }
        Some(at)
    }

    /// The kernel's parameters (`at`: include the AT if there is one).
    pub fn gpu_params(&self, at: bool) -> LaParams {
        let mut p = LaParams {
            count: self.boundaries() as u32,
            ..LaParams::default()
        };
        if let (true, Some(a)) = (at, &self.at) {
            let inv = {
                let d = a.a.re * a.a.re + a.a.im * a.a.im;
                Xc::new(a.a.re / d, -a.a.im / d, -a.a.e)
            };
            p.period = a.period as u32;
            (p.at_a, p.at_ae) = a.a.gpu();
            (p.at_ai, p.at_aie) = inv.gpu();
            (p.at_cc, p.at_cce) = a.cc.gpu();
            (p.at_rc, p.at_rce) = a.ref_c.gpu();
            (p.at_r, p.at_re) = a.r.gpu();
            (p.at_k1, p.at_k1e) = a.k.add(Xr::from_f64(1.0)).gpu();
        }
        p
    }

    pub fn boundaries(&self) -> usize {
        self.bounds.len().saturating_sub(1)
    }

    pub fn bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<LaEntryX>()
            + self.bounds.len() * 8
            + self.next_j.len() * 4
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
    fn longest(&self, j: usize, n: usize, max_iter: usize, un: Xr, cn: Xr, hint: usize) -> (Option<(usize, &LaEntryX)>, usize) {
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

/// Outcome of the CPU models [`iterate_la`], [`iterate_bla_deep`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelOutcome {
    pub n: usize,
    pub escaped: bool,
    /// Loop steps: exact steps plus blocks (plus AT steps).
    pub steps: usize,
    pub blocks: usize,
    /// Table entries tested.
    pub tests: usize,
    pub at_steps: usize,
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
    Xc::new(f64::from(z.mantissa[0]), f64::from(z.mantissa[1]), i64::from(z.exponent))
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
    La(&'a LaTable, bool),
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
        // AT at m = 0 (LA with `at`).
        if let Skip::La(t, true) = skip
            && m == 0
            && let Some(at) = &t.at
            && n + at.period <= max_iter
        {
            // The step [0, P) is valid while |z|^2 + |dc| + K |dc| < R: |y|^2 < |A|^2 (R - (K + 1) |dc|).
            let kc = at.k.add(Xr::from_f64(1.0)).mul(cn);
            if kc.lt(at.r) {
                let (lr, lk) = (log2_xr(at.r), log2_xr(kc));
                let rad = 2.0 * log2_xr(at.a.abs()) + lr + (1.0 - (lk - lr).exp2()).log2();
                let cp = at.cc.mul(dc).add(at.ref_c);
                let mut y = at.a.mul(dz);
                let mut moved = false;
                while n + at.period <= max_iter {
                    let y2 = y.mul(y).add(cp);
                    if !(2.0 * log2_xc(y) < rad) {
                        break;
                    }
                    y = y2;
                    n += at.period;
                    out.at_steps += 1;
                    out.steps += 1;
                    moved = true;
                    if log2_xc(y) > 60.0 {
                        break;
                    }
                }
                if moved {
                    // Back: z = y / A at m = 0 (a period boundary).
                    let ainv = {
                        let a = at.a;
                        let d = a.re * a.re + a.im * a.im;
                        Xc::new(a.re / d, -a.im / d, -a.e)
                    };
                    dz = y.mul(ainv);
                }
            }
        }
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
            Skip::La(t, _) => {
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
fn finish(points: &[FloatExpComplex], dz: &mut Xc, m: &mut usize, last: usize, half_log2_b2: f64, out: &mut ModelOutcome) -> bool {
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
        let r = Reference::compute_f64(&Viewport::new(-1.4080321340304542, 0.13680236432900417, 1e-10), 6000, 65536.0);
        let t = LaTable::build_plain(&r.points, BLA_EPSILON);
        println!("stages {:?}, {} entries, {} boundaries", t.stage_steps, t.entries.len(), t.boundaries());
        assert_eq!(t.bounds[0][0], 0);
        for j in 0..t.boundaries() {
            let c = t.chain(j);
            assert!(!c.is_empty());
            // The chain's ends are boundaries further on, each no earlier than the last.
            let mut prev = j;
            for e in c {
                assert!(e.end_j as usize > j && e.end_j as usize >= prev);
                prev = e.end_j as usize;
                let end = if (e.end_j as usize) < t.boundaries() { t.bounds[e.end_j as usize][0] as usize } else { t.ref_len - 1 };
                assert_eq!(end, t.bounds[j][0] as usize + 1 + e.len as usize);
            }
        }
        for m in 0..t.ref_len {
            let j = t.next_j[m] as usize;
            if j < t.boundaries() {
                assert!(t.bounds[j][0] as usize + 1 >= m);
                assert!(j == 0 || (t.bounds[j - 1][0] as usize + 1) < m);
            }
        }
    }
}
