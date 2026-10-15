//! Journey: an endless, planned, random-but-seeded flight from landmark to landmark (kb:
//! decisions/controls "Journey", techniques/reference-selection, synthesis/places).
//!
//! **Dives and cross-overs (variety).** Deep in one region every view is decorated with the same
//! local Julia set, so a journey that only dives shows one family of shapes. A journey is a
//! sequence of *dives*: a cross-over flight to another region (a catalogue place up to 1e16 or
//! a view from the maths of a bulb, [`regions`]; not one of the last three regions, the region
//! family — valley, limb, needle — alternating), then 1–2 legs within it in a seeded style
//! (scenic, medium, deep), then the next cross-over. Every landing is described ([`look`]:
//! distance spectrum, structure dimension, interior, the spiral pitch of its maths) and scored
//! for novelty against the last six; near-duplicates of the last three are rejected, and a
//! dive goes on only while it shows something new. Long legs fly faster ([`leg_pace`]); with
//! the cinematic effects on, each dive gets its own palette phase.
//!
//! **Targets come from the maths, not from taste.** From the current view the planner finds
//! candidates in and near it (and in a view 2–4 e-folds wider: a short cross-over within the
//! region):
//!
//! - *minibrots*: ball period detection over a small disc at a boundary point of the view,
//!   Newton for the nucleus, the size estimate for the framing (the minibrot fills 1/6 to 1/12
//!   of the view);
//! - *embedded Julia stages*: on the way to a minibrot of size σ the view passes an embedded
//!   Julia set of the minibrot's parent region. Its size is about σ^(3/4) (relative to the whole
//!   set, Heiland-Allen's estimate in Munafo's Mu-Ency "Newton-Raphson zooming"), and between
//!   the view and the minibrot the shapes double at each halving of the remaining depth
//!   ("Julia morphing", Mu-Ency). The planner frames the stage at widths between the view and
//!   the minibrot (geometric interpolation with t = 1/2 and 3/4, and σ^(3/4)) and aims slightly
//!   off the nucleus, then keeps the stage with the best probe; the probe, not the rule, decides;
//! - *Misiurewicz points* (spiral centres, branch points of dendrites): the orbit of a boundary
//!   point shadows a repelling cycle for a while; the preperiod and period where
//!   `|z_{k+p} − z_k|` is smallest seed Newton (`find_misiurewicz`); the multiplier of the cycle
//!   says spiral (complex) or dendrite (real);
//! - a *detail* zoom into the busiest part of the view as a fallback (never twice in a row),
//!   and *resurfacing* to a shallow catalogue place when nothing else is found.
//!
//! **Scoring with cheap image statistics.** Every candidate's framing is rendered as a 40 × 40
//! CPU probe (the oracle: f64, or perturbation at depth) and measured like the GPU survey
//! kernel measures a frame: interior share, band crossings per width (neighbours whose class or
//! integer escape count differ), calm cells. Pure interior, empty exterior and noise are
//! rejected; the rest is scored for structure, a little interior (a minibrot in view), variety
//! (kinds alternate, directions change, depths vary, no target twice) and a seeded random
//! share. The probe runs on the CPU, not on the GPU survey of the shown frame, so a seed gives
//! the same journey on every machine and at every frame rate.
//!
//! **Seeded.** All randomness comes from one SplitMix64 stream advanced only by the planner,
//! and each plan starts from the previous target's view, so `seed` + start view reproduce the
//! journey target for target ("Journey #4711"). The daily journey seeds from the date. Spans
//! are across the view's shorter side, so window shapes do not change the journey.
//!
//! **Motion** is the navigator's: each leg is a fly-to flight (van Wijk–Nuij path), a dwell
//! at the target while it refines, then the next leg. The next target is planned on a thread
//! while the current leg flies, so it is ready at landing; its reference is prefetched during
//! the dwell (scheduler).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use almondbread_math::{
    DoubleExp, DoubleExpComplex, NewtonOptions, PeriodMethod, ReferenceOrbitComputer, find_misiurewicz, find_nucleus,
    find_period, minibrot_size_complex, with_no_control,
};

pub mod look;
pub mod regions;

pub use regions::{RegionId, region_of};

pub use look::Look;

use crate::oracle::{IterationField, cpu_render, cpu_render_deep};
use crate::viewport::{Center, Scale, Viewport};

/// Default journey speed, e-folds per second at the zoom ends, and its range (Q / E, scroll).
pub const SPEED_DEFAULT: f64 = 1.4;
pub const SPEED_RANGE: (f64, f64) = (0.1, 16.0);
/// Seconds at a target (it refines and anti-aliases meanwhile), at the default speed.
pub const DWELL: f64 = 3.0;
/// Legs longer than this many e-folds of path (zoom out, across, zoom in: cross-overs from
/// deep dives) fly faster: only `LONG_LEG_SHARE` of the excess is flown at the journey speed,
/// so a cross-over from 1e40 takes ~20 s instead of ~65 s at the default speed.
pub const LONG_LEG: f64 = 14.0;
pub const LONG_LEG_SHARE: f64 = 0.2;

/// The speed factor of a leg whose path covers `efolds` e-folds (1 for ordinary legs).
pub fn leg_pace(efolds: f64) -> f64 {
    if !efolds.is_finite() || efolds <= LONG_LEG {
        1.0
    } else {
        efolds / (LONG_LEG + LONG_LEG_SHARE * (efolds - LONG_LEG))
    }
}

/// Probe resolution (square, across the shorter side).
pub const PROBE: usize = 40;
/// Probe iteration cap (CPU).
pub const PROBE_ITER_CAP: usize = 30_000;
/// Depth gain of a leg, e-folds: accepted range.
pub const GAIN_RANGE: (f64, f64) = (2.5, 20.0);
/// Cross over after this depth (log10 of the magnification against span 3) whatever the dive.
pub const MAX_DEPTH_LOG10: f64 = 40.0;
/// Boundary points tried per plan.
const SAMPLES: usize = 10;
/// Candidates probed per plan (best by the pre-score).
const MAX_PROBES: usize = 14;

/// What a target is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetKind {
    /// A minibrot (nucleus of period p) framed with its decorations.
    Minibrot,
    /// An embedded Julia stage on the way to a minibrot.
    Julia,
    /// On the way to a minibrot too deep for one leg, centred on its nucleus.
    Approach,
    /// A Misiurewicz point whose cycle multiplier turns: a spiral centre.
    Spiral,
    /// A Misiurewicz point with a real multiplier: a branch point of a dendrite.
    Dendrite,
    /// The busiest part of the view (fallback).
    Detail,
    /// Back up to a shallow catalogue place (the fallback when nothing else is found).
    Surface,
    /// Over to another region: a catalogue place or a view from the maths of a bulb (the
    /// flight zooms out as far as the distance needs and back in).
    Crossing,
}

impl TargetKind {
    pub const ALL: [TargetKind; 8] = [
        Self::Minibrot,
        Self::Julia,
        Self::Approach,
        Self::Spiral,
        Self::Dendrite,
        Self::Detail,
        Self::Surface,
        Self::Crossing,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Minibrot => "minibrot",
            Self::Julia => "embedded Julia",
            Self::Approach => "approach",
            Self::Spiral => "spiral",
            Self::Dendrite => "dendrite",
            Self::Detail => "detail",
            Self::Surface => "resurfacing",
            Self::Crossing => "cross-over",
        }
    }

    /// Spirals and dendrites are both Misiurewicz points: one family for the variety rules.
    fn family(self) -> u8 {
        match self {
            Self::Spiral | Self::Dendrite => 2,
            Self::Minibrot | Self::Approach => 0,
            Self::Julia => 1,
            Self::Detail => 3,
            Self::Surface | Self::Crossing => 4,
        }
    }
}

/// SplitMix64: small, seedable, and the same everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x6A09_E667_F3BC_C909)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    pub fn index(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// The seed of the daily journey for a date (the same for everyone that day).
pub fn daily_seed(year: i32, month: u32, day: u32) -> u64 {
    (year.max(0) as u64) * 10_000 + u64::from(month) * 100 + u64::from(day)
}

/// Image statistics of a probe (the survey kernel's measures, on the CPU).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProbeStats {
    /// Interior (never escaped) share, whole probe and its middle third.
    pub interior: f64,
    pub centre_interior: f64,
    /// Band crossings per probe width (neighbours whose class or integer count differ).
    pub detail: f64,
    pub centre_detail: f64,
    /// Share of 5 × 5-texel cells without any crossing (calm or empty).
    pub calm: f64,
    /// Share of exterior texels escaping at least 8 iterations later than the fastest one:
    /// how much of the view is close to the set (filaments, spirals, decorations) rather than
    /// the smooth gradient between them.
    pub filament: f64,
    /// Median escape-count step between neighbouring exterior probe texels: how fast the
    /// palette changes across the view (beyond a few hundred per probe texel the colours
    /// change several times per screen pixel: grey noise, however rich the maths).
    pub roughness: f64,
}

impl ProbeStats {
    pub fn of(f: &IterationField) -> Self {
        let (w, h) = (f.width, f.height);
        if w < 3 || h < 3 {
            return Self::default();
        }
        let cls = |i: usize| -> (bool, i64) {
            let n = f.n[i];
            (n < 0.0, if n < 0.0 { -1 } else { n.floor() as i64 })
        };
        let in_centre = |x: usize, y: usize| x >= w / 3 && x < 2 * w / 3 && y >= h / 3 && y < 2 * h / 3;
        let (mut int, mut cint, mut cn) = (0usize, 0usize, 0usize);
        let (mut pairs, mut edges, mut cpairs, mut cedges) = (0usize, 0usize, 0usize, 0usize);
        let cell = 5;
        let (cx, cy) = (w.div_ceil(cell), h.div_ceil(cell));
        let mut cell_edges = vec![0usize; cx * cy];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let a = cls(i);
                let centre = in_centre(x, y);
                if a.0 {
                    int += 1;
                    if centre {
                        cint += 1;
                    }
                }
                if centre {
                    cn += 1;
                }
                for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                    if nx >= w || ny >= h {
                        continue;
                    }
                    let b = cls(ny * w + nx);
                    let e = a != b;
                    pairs += 1;
                    edges += usize::from(e);
                    if centre {
                        cpairs += 1;
                        cedges += usize::from(e);
                    }
                    if e {
                        cell_edges[(y / cell) * cx + x / cell] += 1;
                    }
                }
            }
        }
        let calm = cell_edges.iter().filter(|&&e| e == 0).count() as f64 / cell_edges.len() as f64;
        let n_min = f.n.iter().copied().filter(|&n| n >= 0.0).fold(f32::INFINITY, f32::min);
        let ext = f.n.iter().filter(|&&n| n >= 0.0).count();
        let near = f.n.iter().filter(|&&n| n >= 0.0 && n >= n_min + 8.0).count();
        let mut steps: Vec<f32> = Vec::with_capacity(2 * w * h);
        for y in 0..h {
            for x in 0..w {
                let a = f.n[y * w + x];
                for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                    if nx < w && ny < h {
                        let b = f.n[ny * w + nx];
                        if a >= 0.0 && b >= 0.0 {
                            steps.push((a - b).abs());
                        }
                    }
                }
            }
        }
        steps.sort_by(f32::total_cmp);
        let roughness = steps.get(steps.len() / 2).copied().unwrap_or(0.0) as f64;
        Self {
            roughness,
            filament: near as f64 / ext.max(1) as f64,
            interior: int as f64 / (w * h) as f64,
            centre_interior: cint as f64 / cn.max(1) as f64,
            detail: edges as f64 / pairs.max(1) as f64 * w as f64,
            centre_detail: cedges as f64 / cpairs.max(1) as f64 * w as f64,
            calm,
        }
    }

    /// Why a framing is not worth flying to, if it is not.
    pub fn reject(&self) -> Option<&'static str> {
        if self.interior > 0.6 || self.centre_interior > 0.75 {
            Some("interior")
        } else if self.detail < 3.0 || self.centre_detail < 1.5 || self.calm > 0.8 || self.filament < 0.1 {
            Some("featureless")
        } else if self.roughness > 400.0 {
            Some("noise")
        } else {
            None
        }
    }

    /// Structure score in about 0 ... 1.5 (higher is better).
    pub fn quality(&self, kind: TargetKind) -> f64 {
        let ss = |t: f64| {
            let t = t.clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        // Richness: how much of the view is near the set. Sparse filaments in a smooth gradient
        // (dendrite country) score low, decorated valleys and spirals high.
        // Too much of it is a carpet without shapes: the best views have structure *and* some
        // smooth space around it (filament share about 0.35 ... 0.75).
        let structure = 0.5 * ss((self.detail - 4.0) / 10.0)
            + 0.3 * ss((self.centre_detail - 3.0) / 10.0)
            + 1.4 * ss((self.filament - 0.15) / 0.3)
            - 1.3 * ss((self.filament - 0.75) / 0.2);
        let noise = 0.6 * ss((self.roughness - 60.0) / 300.0);
        let calm = 1.5 * (self.calm - 0.45).max(0.0);
        let interior = 1.2 * ss((self.interior - 0.3) / 0.3);
        // A little interior is the minibrot (or a bulb) in view: welcome, not required.
        let bonus = if matches!(kind, TargetKind::Minibrot | TargetKind::Julia | TargetKind::Approach) {
            0.25 * ss(self.interior / 0.03)
        } else {
            0.0
        };
        structure - noise - calm - interior + bonus
    }
}

/// Probe iteration limit at a span (the app's automatic depth limit, capped for the CPU).
pub fn probe_iterations(span: Scale) -> usize {
    let depth = (3.0f64.log10() - span.log10()).max(0.0);
    ((300.0 + 120.0 * depth.powf(1.6)) as usize).min(PROBE_ITER_CAP)
}

/// A `PROBE` × `PROBE` render of the square of side `span` around `center` (plane axes).
pub fn probe(center: &Center, span: Scale, extra_iterations: usize) -> IterationField {
    probe_sized(center, span, extra_iterations, PROBE)
}

/// A `size` × `size` probe (see [`probe`]).
pub fn probe_sized(center: &Center, span: Scale, extra_iterations: usize, size: usize) -> IterationField {
    let texel = span / size as f64;
    let it = (probe_iterations(span) + extra_iterations).min(PROBE_ITER_CAP * 2);
    if texel.to_f64() >= 1e-13 {
        cpu_render(center.re(), center.im(), texel.to_f64(), size, size, it, 65536.0, 0.0)
    } else {
        let c = center.with_min_precision(
            Viewport::with_center(center.clone(), texel / 4.0)
                .needed_center_bits()
                .max(center.precision_bits()),
        );
        cpu_render_deep(&c, texel, size, size, it, 65536.0, 0.0).0
    }
}

/// The spiral pitch of a view, from its maths: the orbit of a point near the centre shadows
/// a repelling cycle (the (k, p) of its closest return, as for Misiurewicz candidates); the
/// cycle's multiplier λ = Π 2 z_i says how the structure there repeats: scaled by |λ| and
/// turned by arg λ. The turn per e-fold of scale, `|arg λ| / ln |λ|` (arg folded to [0, π/2]:
/// a negative real multiplier flips, it does not turn), is what the eye sees as curl:
/// about 0 for dendrites and stars, large for tight spirals. Two points off the centre
/// (a nucleus' own cycle is attracting), averaged, mapped to 0 … 1 by `t / (t + 0.5)`.
pub fn pitch(center: &Center, span: Scale) -> f64 {
    let bits = bits_for(span).max(center.precision_bits());
    let base = center.with_min_precision(bits);
    let n_orbit = probe_iterations(span).clamp(500, 20_000);
    let mut sum = 0.0;
    let mut count = 0;
    for (ox, oy) in [(0.21, 0.13), (-0.18, -0.2)] {
        let p = base.offset_scaled(ox, oy, span);
        let comp = ReferenceOrbitComputer::new(p.big(), bits);
        let (orbit, _) = with_no_control(4096, |ctl| comp.compute_double_exp(n_orbit, 1e6, ctl));
        let z: Vec<[f64; 2]> = orbit.iter().map(|v| [v.re.to_f64(), v.im.to_f64()]).collect();
        if z.len() < 8 {
            continue;
        }
        let mut best: Option<(f64, usize, usize)> = None;
        for per in 1..=16usize {
            for k in 1..z.len().saturating_sub(per) {
                let (a, b) = (z[k], z[k + per]);
                let d = (a[0] - b[0]).hypot(a[1] - b[1]) / (a[0].hypot(a[1]) + 1e-3) * (1.0 + 0.05 * per as f64);
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, k, per));
                }
            }
        }
        let Some((_, k, per)) = best else { continue };
        let (mut lr, mut li) = (1.0f64, 0.0f64);
        let mut ln_mag = 0.0;
        for v in &z[k..k + per] {
            let (zr, zi) = (2.0 * v[0], 2.0 * v[1]);
            let m = zr.hypot(zi);
            if m == 0.0 {
                continue;
            }
            ln_mag += m.ln();
            let (r, i) = (lr * zr - li * zi, lr * zi + li * zr);
            let n = r.hypot(i);
            (lr, li) = (r / n, i / n);
        }
        let turn = li.atan2(lr).abs();
        let turn = turn.min(std::f64::consts::PI - turn);
        let t = turn / ln_mag.max(0.05);
        sum += t / (t + 0.5);
        count += 1;
    }
    if count == 0 { 0.0 } else { sum / count as f64 }
}

/// The look of a probe of the view (`center`, `span`) with its spiral pitch.
pub fn look_of(f: &IterationField, center: &Center, span: Scale) -> Look {
    Look::of(f).with_pitch(pitch(center, span))
}

/// A place visited, for the variety rules.
#[derive(Clone, Debug, PartialEq)]
pub struct Visit {
    pub kind: TargetKind,
    pub center: Center,
    pub span: Scale,
    /// Direction of travel to it in the plane (radians), if it moved sideways.
    pub direction: Option<f64>,
    /// What its probe looked like, and its region.
    pub look: Look,
    pub region: RegionId,
}

/// One planned target.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub kind: TargetKind,
    pub center: Center,
    /// Complex extent across the view's shorter side.
    pub span: Scale,
    /// Minibrot or cycle period, Misiurewicz preperiod.
    pub period: Option<usize>,
    pub preperiod: Option<usize>,
    /// "period-34 minibrot", "spiral M(36,2)", ...
    pub label: String,
    /// e-folds deeper than the view it was planned from (negative: resurfacing).
    pub gain: f64,
    pub score: f64,
    pub stats: ProbeStats,
    /// What the probe looks like (novelty scoring).
    pub look: Look,
    /// The journey's palette phase at this target (cycles, unwrapped; applied only with the
    /// cinematic effects on): it changes at cross-overs, eased over the flight.
    pub palette_phase: f64,
}

impl Target {
    /// log10 of the magnification (whole set: span 3).
    pub fn zoom_log10(&self) -> f64 {
        3.0f64.log10() - self.span.log10()
    }

    /// The view for a window whose shorter side is `min_side` points.
    pub fn viewport(&self, min_side: f64) -> Viewport {
        let side = if min_side.is_finite() && min_side > 0.0 {
            min_side
        } else {
            800.0
        };
        Viewport::with_center(self.center.clone(), self.span / side)
    }
}

/// How a dive goes: seeded at every cross-over (kb: decisions/controls "Journey", variety).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiveStyle {
    /// The cross-over and one short leg near the surface: the region's scenery.
    Scenic,
    /// The cross-over and one or two legs, to 1e8 … 1e20.
    Medium,
    /// The cross-over and two long legs, down to minibrots and their Julia stages (to 1e40).
    Deep,
}

impl DiveStyle {
    /// The depth gain a leg prefers (e-folds, seeded within).
    fn gain(self) -> (f64, f64) {
        match self {
            Self::Scenic => (2.5, 5.0),
            Self::Medium => (4.0, 8.0),
            Self::Deep => (11.0, 17.0),
        }
    }

    /// Cross over once a dive is this deep (log10 of the magnification).
    fn max_depth_log10(self) -> f64 {
        match self {
            Self::Scenic => 10.0,
            Self::Medium => 22.0,
            Self::Deep => MAX_DEPTH_LOG10,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Scenic => "scenic",
            Self::Medium => "medium",
            Self::Deep => "deep",
        }
    }
}

/// The planner's state between plans: what makes the journey reproducible.
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    pub seed: u64,
    pub rng: Rng,
    pub history: Vec<Visit>,
    /// Legs left in this dive before the journey crosses over to another region (0: the
    /// next leg crosses over; a new journey starts with one).
    pub dive_left: u32,
    pub style: DiveStyle,
    /// The regions of the dives so far, newest last (at most 12).
    pub regions: Vec<RegionId>,
    /// Palette phase (cycles, unwrapped) of the current dive: every cross-over moves it on by
    /// 0.15 … 0.4, so each region gets its own colours when the cinematic effects are on.
    pub palette_phase: f64,
}

impl Chain {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rng: Rng::new(seed),
            history: Vec::new(),
            dive_left: 0,
            style: DiveStyle::Medium,
            regions: Vec::new(),
            palette_phase: 0.0,
        }
    }

    /// The chain of a journey started from the view (`center`, `span`): from home the first
    /// leg crosses over; started away from home (J pressed while exploring), it first dives
    /// two legs where the user is.
    pub fn starting_at(seed: u64, center: &Center, span: Scale) -> Self {
        let mut chain = Self::new(seed);
        let (c, s) = canonical_start(center, span);
        if s.to_f64() < 1.5 {
            chain.dive_left = 2;
            chain.regions.push(region_of(c.re(), c.im()).0);
        }
        chain
    }
}

/// A planned leg and the chain after it.
#[derive(Clone, Debug)]
pub struct Plan {
    pub target: Target,
    pub chain: Chain,
    /// Candidates found and probed.
    pub candidates: usize,
    pub probed: usize,
    pub seconds: f64,
}

/// Candidate before probing.
#[derive(Clone, Debug)]
struct Candidate {
    kind: TargetKind,
    center: Center,
    span: Scale,
    period: Option<usize>,
    preperiod: Option<usize>,
    label: String,
    prescore: f64,
    /// Extra probe iterations (a minibrot's period).
    extra: usize,
}

fn scale_of(d: DoubleExp) -> Option<Scale> {
    let l = d.log2_magnitude();
    l.is_finite().then(|| Scale::from_log2(l))
}

/// Bits a centre needs to frame `span` (with the probe's resolution and guard bits).
fn bits_for(span: Scale) -> usize {
    Viewport::with_center(Center::new(0.0, 0.0), span / (4.0 * PROBE as f64)).needed_center_bits()
}

/// The canonical whole-set start: a view near the fitted home view plans from centre -0.75
/// with span 3, so "a journey from home" is the same in every window.
fn canonical_start(center: &Center, span: Scale) -> (Center, Scale) {
    let s = span.to_f64();
    if (1.5..5.0).contains(&s) && (center.re() + 0.75).abs() < 0.3 && center.im().abs() < 0.3 {
        (Center::new(-0.75, 0.0), Scale::from_f64(3.0))
    } else {
        (center.clone(), span)
    }
}

/// Visits compared for novelty; the distance below which a probe is a near-duplicate of one
/// of the last `DUPLICATE_RECENT` landings (rejected outright), and the stricter one for a
/// dive's later legs: a dive goes on only while it shows something new, else the journey
/// crosses over. Calibrated on journey #4711's landings sorted into four families by eye
/// (dendrite carpets, bulbs, thin stars and medallions, spirals): distances within a family
/// 0.24 / 0.45 / 0.77 (10th / 50th / 90th percentile), across families 0.45 / 0.73 / 1.13.
const NOVELTY_RECENT: usize = 6;

const DUPLICATE_RECENT: usize = 3;
pub const DUPLICATE: f64 = 0.22;
pub const FAMILIAR: f64 = 0.33;
/// Novelty score: per unit of look distance to the most similar recent landing, up to a cap.
const NOVELTY_WEIGHT: f64 = 1.2;
const NOVELTY_CAP: f64 = 0.8;

/// How new a look is against the recent landings: the score term, and whether it is a
/// near-duplicate (closer than `dup` to one of the last three). Older landings count less
/// (their distance is stretched by 12 % per leg).
pub fn novelty(look: &Look, history: &[Visit], dup: f64) -> (f64, bool) {
    let mut dmin = f64::INFINITY;
    let mut duplicate = false;
    for (k, v) in history.iter().rev().take(NOVELTY_RECENT).enumerate() {
        let d = look.distance(&v.look);
        if k < DUPLICATE_RECENT && d < dup {
            duplicate = true;
        }
        dmin = dmin.min(d * (1.0 + 0.12 * k as f64));
    }
    if !dmin.is_finite() {
        return (NOVELTY_WEIGHT * NOVELTY_CAP, false);
    }
    (NOVELTY_WEIGHT * dmin.min(NOVELTY_CAP), duplicate)
}

/// Plans the next target from the view (`center`, `span` across the shorter side). Returns
/// None only when cancelled. Deterministic in (`center`, `span`, `chain`).
///
/// A journey is a sequence of *dives*: a cross-over flight to another region (a catalogue
/// place or a bulb's view, not one of the last regions), then one to five legs within it in a
/// seeded style (scenic, medium, deep), then the next cross-over. Within a dive the targets
/// are the maths' (minibrots, Julia stages, Misiurewicz points) in and near the view, and
/// those of a wider view around it (a short cross-over within the region). Every candidate's
/// probe is scored for structure and for novelty against the last landings; near-duplicates
/// are rejected, and a dive whose candidates all look like what was just seen ends early.
pub fn plan(center: &Center, span: Scale, chain: &Chain, cancel: &AtomicBool) -> Option<Plan> {
    let t0 = std::time::Instant::now();
    let (center, span) = canonical_start(center, span);
    let mut chain = chain.clone();
    let mut rng = chain.rng;
    let depth = 3.0f64.log10() - span.log10();
    let cancelled = || cancel.load(Ordering::Relaxed);
    let trace = std::env::var_os("ALMONDBREAD_JOURNEY_TRACE").is_some();
    let mut found = 0;
    let mut probed = 0;

    // ---- Within the dive.
    let mut best: Option<Target> = None;
    let cross_due = chain.dive_left == 0 || depth > chain.style.max_depth_log10();
    if !cross_due {
        let mut candidates = find_candidates(&center, span, &mut rng, cancel)?;
        // A short cross-over within the region: targets of a view 2-4 e-folds wider, away
        // from this one (the flight zooms out a little and back in beside it).
        if depth > 2.5 {
            let wide = span * rng.range(2.0, 4.0).exp();
            let mut side = find_candidates(&center, wide, &mut rng, cancel)?;
            side.retain(|c| {
                c.kind != TargetKind::Detail
                    && c.center.distance_in(&center, span) > 0.8
                    && span.ratio(c.span).ln() > -1.0
            });
            candidates.extend(side);
        }
        if cancelled() {
            return None;
        }
        let (lo, hi) = chain.style.gain();
        let want_gain = rng.range(lo, hi);
        prescore_dive(&mut candidates, &center, span, &chain, want_gain, &mut rng);
        found += candidates.len();
        // The dive's first leg after the cross-over needs only to differ; later ones must
        // show something new.
        let in_dive = chain
            .history
            .iter()
            .rev()
            .take_while(|v| !matches!(v.kind, TargetKind::Crossing | TargetKind::Surface))
            .count();
        let dup = if in_dive == 0 { DUPLICATE } else { FAMILIAR };
        let (b, n) = probe_best(candidates, span, &chain.history, dup, trace, cancel)?;
        probed += n;
        // A boundary detail after a boundary detail: the dive has run out of landmarks.
        best = b.filter(|t| {
            t.kind != TargetKind::Detail || chain.history.last().is_none_or(|v| v.kind != TargetKind::Detail)
        });
        if best.is_none() && trace {
            println!("    dive ends: nothing new here");
        }
    }

    // ---- Cross over to another region.
    let crossing = best.is_none();
    if crossing {
        // The next dive's style: no two deep dives in a row.
        let r = rng.f64();
        let style = if r < 0.35 {
            DiveStyle::Scenic
        } else if r < 0.72 || chain.style == DiveStyle::Deep || chain.regions.is_empty() {
            DiveStyle::Medium
        } else {
            DiveStyle::Deep
        };
        chain.style = style;
        let here = region_of(center.re(), center.im()).0;
        let mut candidates = crossover_candidates(here, span, &chain, &mut rng);
        found += candidates.len();
        candidates.truncate(MAX_PROBES);
        let (b, n) = probe_best(candidates, span, &chain.history, DUPLICATE, trace, cancel)?;
        probed += n;
        best = b;
        chain.dive_left = match style {
            DiveStyle::Scenic => 1,
            DiveStyle::Medium => 1 + rng.index(2) as u32,
            DiveStyle::Deep => 2,
        };
    } else {
        chain.dive_left = chain.dive_left.saturating_sub(1);
    }
    let mut target = match best {
        Some(t) => t,
        None => surface(span, &mut rng, &chain),
    };
    if crossing || target.kind == TargetKind::Surface {
        chain.palette_phase += rng.range(0.15, 0.4);
    }
    target.palette_phase = chain.palette_phase;
    let region = region_of(target.center.re(), target.center.im()).0;
    if crossing || target.kind == TargetKind::Surface {
        chain.regions.push(region);
        if chain.regions.len() > 12 {
            chain.regions.remove(0);
        }
    }
    let (dx, dy) = target.center.delta_in(&center, span);
    chain.history.push(Visit {
        kind: target.kind,
        center: target.center.clone(),
        span: target.span,
        direction: (dx.hypot(dy) > 0.05).then(|| dy.atan2(dx)),
        look: target.look,
        region,
    });
    if chain.history.len() > 32 {
        chain.history.remove(0);
    }
    chain.rng = rng;
    Some(Plan {
        target,
        chain,
        candidates: found,
        probed,
        seconds: t0.elapsed().as_secs_f64(),
    })
}

/// Variety rules before probing (kinds alternate, the leg's preferred gain, directions
/// change, no place twice) and a seeded share.
fn prescore_dive(
    candidates: &mut Vec<Candidate>,
    center: &Center,
    span: Scale,
    chain: &Chain,
    want_gain: f64,
    rng: &mut Rng,
) {
    let last_dir = chain.history.last().and_then(|v| v.direction);
    let recent: Vec<TargetKind> = chain.history.iter().rev().take(4).map(|v| v.kind).collect();
    for c in candidates.iter_mut() {
        let gain = span.ratio(c.span).ln();
        let mut s = 0.0;
        // Kinds alternate: the same family as the last target costs, twice in a row more.
        if let Some(k) = recent.first()
            && k.family() == c.kind.family()
        {
            s -= 0.45;
            if recent.get(1).is_some_and(|k2| k2.family() == c.kind.family()) {
                s -= 0.5;
            }
        }
        if !recent.iter().any(|k| k.family() == c.kind.family()) {
            s += 0.2;
        }
        if c.kind == TargetKind::Detail {
            s -= 0.6;
            if recent.first() == Some(&TargetKind::Detail) {
                s -= 1.0;
            }
        }
        // Straight down onto the last target's centre again is a plain zoom.
        if chain
            .history
            .last()
            .is_some_and(|v| c.center.distance_in(&v.center, c.span) < 0.05)
        {
            s -= 0.6;
        }
        s -= 0.06 * (gain - want_gain).abs();
        // Directions change: a leg along the last one's direction costs a little.
        let (dx, dy) = c.center.delta_in(center, span);
        if dx.hypot(dy) > 0.05
            && let Some(ld) = last_dir
        {
            let d = (dy.atan2(dx) - ld).abs() % std::f64::consts::TAU;
            let d = d.min(std::f64::consts::TAU - d);
            if d < 0.5 {
                s -= 0.15;
            }
        }
        if visited(&chain.history, &c.center, c.span) {
            s -= 10.0;
        }
        c.prescore = s + rng.range(0.0, 0.25);
    }
    candidates.retain(|c| c.prescore > -5.0);
    candidates.sort_by(|a, b| b.prescore.total_cmp(&a.prescore));
}

/// A place already landed on (same place, similar depth).
fn visited(history: &[Visit], center: &Center, span: Scale) -> bool {
    history
        .iter()
        .any(|v| center.distance_in(&v.center, span) < 0.08 && v.span.ratio(span).ln().abs() < 1.5)
}

/// Cross-over targets: anchors in other regions than this one and the last dives', best
/// first by a seeded pre-score.
fn crossover_candidates(here: RegionId, span: Scale, chain: &Chain, rng: &mut Rng) -> Vec<Candidate> {
    let anchors = regions::anchors(rng);
    let last: Vec<RegionId> = chain.regions.iter().rev().take(3).copied().collect();
    let older: Vec<RegionId> = chain.regions.iter().rev().take(8).copied().collect();
    let last_family = chain.regions.last().map(|&r| regions::family(r));
    let mut out: Vec<Candidate> = Vec::new();
    for a in anchors {
        if a.region == here || last.contains(&a.region) || visited(&chain.history, &a.center, a.span) {
            continue;
        }
        // Far places of the catalogue and the maths views alike; regions seen in the last
        // eight dives cost; scenic dives prefer shallow anchors, deep ones need not.
        let zoom = 3.0f64.log10() - a.span.log10();
        let mut s = rng.range(0.0, 0.6);
        if older.contains(&a.region) {
            s -= 0.5;
        }
        // Region families alternate (valleys, limbs, the needle): deep in a limb every view
        // is dendrite filigree, deep in a valley spirals, so a limb after a limb looks the
        // same however far apart they are. Deep dives go where depth brings new shapes.
        let family = regions::family(a.region);
        if last_family == Some(family) {
            s -= 0.7;
        }
        if chain.style == DiveStyle::Deep && family == regions::Family::Limb {
            s -= 0.8;
        }
        if chain.style == DiveStyle::Scenic && zoom > 6.0 {
            s -= 0.3;
        }
        if a.look.is_none() {
            s += 0.1;
        }
        let gain = span.ratio(a.span).ln();
        out.push(Candidate {
            kind: TargetKind::Crossing,
            center: a.center,
            span: a.span,
            period: a.period,
            preperiod: a.preperiod,
            label: format!("over to {}", a.label),
            prescore: s - 0.01 * gain.abs().min(60.0),
            extra: a.period.map_or(0, |p| (4 * p).min(PROBE_ITER_CAP)),
        });
    }
    out.sort_by(|a, b| b.prescore.total_cmp(&a.prescore));
    out
}

/// Probes the candidates in order (up to `max`) and keeps the best: structure, the
/// pre-score and novelty against the recent landings; rejects featureless, interior and noisy
/// framings and near-duplicates. None (outer) only when cancelled.
fn probe_best(
    candidates: Vec<Candidate>,
    span: Scale,
    history: &[Visit],
    dup: f64,
    trace: bool,
    cancel: &AtomicBool,
) -> Option<(Option<Target>, usize)> {
    let mut best: Option<Target> = None;
    let mut probed = 0;
    for c in candidates.into_iter().take(MAX_PROBES) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let f = probe(&c.center, c.span, c.extra);
        probed += 1;
        let st = ProbeStats::of(&f);
        if let Some(why) = st.reject() {
            if trace {
                println!("    reject {:<40} {why}", c.label);
            }
            continue;
        }
        let look = look_of(&f, &c.center, c.span);
        let (nov, duplicate) = novelty(&look, history, dup);
        if duplicate {
            if trace {
                println!("    reject {:<40} looks like a recent landing", c.label);
            }
            continue;
        }
        // A cross-over landing beside a big dark body (the cardioid's or a bulb's edge) reads
        // as the same black wedge every time: catalogue framings with > 12 % interior cost.
        let dark = if c.kind == TargetKind::Crossing {
            2.0 * (st.interior - 0.12).max(0.0)
        } else {
            0.0
        };
        let score = st.quality(c.kind) + c.prescore + nov - dark;
        if trace {
            println!(
                "    probe  {:<40} score {score:.2} (pre {:.2} novelty {nov:.2}) filament {:.2} detail {:.1} gain {:.1}",
                c.label,
                c.prescore,
                st.filament,
                st.detail,
                span.ratio(c.span).ln()
            );
        }
        if best.as_ref().is_none_or(|b| score > b.score) {
            best = Some(Target {
                kind: c.kind,
                center: c.center,
                span: c.span,
                period: c.period,
                preperiod: c.preperiod,
                label: c.label,
                gain: span.ratio(c.span).ln(),
                score,
                stats: st,
                look,
                palette_phase: 0.0,
            });
        }
    }
    Some((best, probed))
}

/// A shallow catalogue place not visited recently (seeded): the fallback when nothing else
/// is found.
fn surface(span: Scale, rng: &mut Rng, chain: &Chain) -> Target {
    let pool: Vec<&crate::places::Place> = crate::places::places()
        .iter()
        .filter(|p| p.zoom_log10() < 6.0 && p.zoom_log10() > 0.5)
        .filter(|p| {
            !chain
                .history
                .iter()
                .any(|v| v.center.distance_in(&p.center(), p.span_scale()) < 0.1)
        })
        .collect();
    let all: Vec<&crate::places::Place> = crate::places::places()
        .iter()
        .filter(|p| p.zoom_log10() < 6.0 && p.zoom_log10() > 0.5)
        .collect();
    let pool = if pool.is_empty() { all } else { pool };
    let p = pool[rng.index(pool.len())];
    let center = p.center();
    let t_span = p.span_scale();
    let f = probe(&center, t_span, 0);
    let st = ProbeStats::of(&f);
    Target {
        kind: TargetKind::Surface,
        label: format!("resurfacing: {}", p.name),
        gain: span.ratio(t_span).ln(),
        score: 0.0,
        stats: st,
        look: look_of(&f, &center, t_span),
        palette_phase: 0.0,
        center,
        span: t_span,
        period: None,
        preperiod: None,
    }
}

/// Boundary points of the view, seeded and weighted by detail, as plane offsets in texels.
fn boundary_points(f: &IterationField, rng: &mut Rng, count: usize) -> Vec<(f64, f64)> {
    let (w, h) = (f.width, f.height);
    let mut pts: Vec<(usize, usize)> = Vec::new();
    let margin = w / 10;
    for y in margin..h - margin {
        for x in margin..w - margin {
            let i = y * w + x;
            let a = f.n[i];
            let edge = [(x + 1, y), (x, y + 1)].iter().any(|&(nx, ny)| {
                let b = f.n[ny * w + nx];
                (a < 0.0) != (b < 0.0) || (a >= 0.0 && b >= 0.0 && a.floor() != b.floor())
            });
            if edge {
                pts.push((x, y));
            }
        }
    }
    // Weighted by how busy the neighbourhood is (texels escaping well after the fastest:
    // near the set), so samples go where the structure is rich, not along lone filaments.
    let n_min = f.n.iter().copied().filter(|&n| n >= 0.0).fold(f32::INFINITY, f32::min);
    let busy = |x: usize, y: usize| -> f64 {
        let mut c = 0usize;
        for yy in y.saturating_sub(3)..(y + 4).min(h) {
            for xx in x.saturating_sub(3)..(x + 4).min(w) {
                let n = f.n[yy * w + xx];
                if n < 0.0 || n >= n_min + 8.0 {
                    c += 1;
                }
            }
        }
        c as f64
    };
    let weights: Vec<f64> = pts.iter().map(|&(x, y)| 1.0 + busy(x, y).powi(2)).collect();
    let total: f64 = weights.iter().sum();
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut tries = 0;
    while out.len() < count && !pts.is_empty() && tries < 200 {
        tries += 1;
        let mut r = rng.f64() * total;
        let mut i = 0;
        while i + 1 < weights.len() && r >= weights[i] {
            r -= weights[i];
            i += 1;
        }
        let p = pts[i];
        if out
            .iter()
            .any(|q| (q.0 as f64 - p.0 as f64).hypot(q.1 as f64 - p.1 as f64) < w as f64 / 8.0)
        {
            continue;
        }
        out.push(p);
    }
    out.iter()
        .map(|&(x, y)| {
            let jx = rng.f64() - 0.5;
            let jy = rng.f64() - 0.5;
            (
                x as f64 + 0.5 + jx - w as f64 / 2.0,
                -(y as f64 + 0.5 + jy - h as f64 / 2.0),
            )
        })
        .collect()
}

/// Minibrots, their Julia stages, Misiurewicz points and detail zooms in and near the view.
fn find_candidates(center: &Center, span: Scale, rng: &mut Rng, cancel: &AtomicBool) -> Option<Vec<Candidate>> {
    let texel = span / PROBE as f64;
    let view = probe(center, span, 0);
    let points = boundary_points(&view, rng, SAMPLES);
    let bits = bits_for(span).max(center.precision_bits());
    let base = center.with_min_precision(bits);
    let max_period = probe_iterations(span).clamp(64, 40_000);
    let mut out: Vec<Candidate> = Vec::new();
    let mut seen_nuclei: Vec<(usize, Center)> = Vec::new();
    // Newton for a nucleus needs the precision of the minibrot, not of the view: deep minibrots
    // in view are 2^-40 ... 2^-100 of it.
    let nbits = bits + 128;
    for (si, &(px, py)) in points.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let p = base.offset_scaled(px, py, texel);
        let radius_f = rng.range(0.7, 2.5);
        // ---- Minibrot.
        let radius = texel.big_times(radius_f, 64);
        let mut is_cancelled = || cancel.load(Ordering::Relaxed);
        let trace = std::env::var_os("ALMONDBREAD_JOURNEY_TRACE").is_some();
        let found = find_period(
            p.big(),
            &radius,
            max_period,
            PeriodMethod::Ball,
            1024,
            &mut is_cancelled,
        );
        if trace {
            println!("    sample ({px:.1},{py:.1}) r {radius_f:.2} texels: period {found:?} (max {max_period})");
        }
        if let Some(period) = found
            && let Some(n) = find_nucleus(
                p.big(),
                period,
                &NewtonOptions {
                    precision_bits: Some(nbits),
                    max_steps: 64,
                    max_distance: Some(span.big_times(0.75, 64)),
                },
                &mut is_cancelled,
            )
        {
            let nucleus = Center::from_big(n.nucleus);
            let size = minibrot_size_complex(nucleus.big(), period);
            if trace {
                println!(
                    "      nucleus at {:.2} spans, size 2^{:.1} (view 2^{:.1})",
                    nucleus.distance_in(center, span),
                    size.magnitude().log2_magnitude(),
                    span.log2()
                );
            }
            if nucleus.distance_in(center, span) < 0.5
                && let Some(sigma) = scale_of(size.magnitude())
                && !seen_nuclei
                    .iter()
                    .any(|(q, c)| *q == period && c.distance_in(&nucleus, sigma) < 1.0)
            {
                seen_nuclei.push((period, nucleus.clone()));
                minibrot_candidates(&mut out, span, &nucleus, sigma, period, rng, cancel);
            }
        }
        // ---- Misiurewicz point (the first few samples: Newton on long orbits is the costly
        // part); the periods tried include the minibrot period found here (deep spirals live in
        // embedded copies whose cycles have that period).
        if si < 4
            && let Some(c) = misiurewicz_candidate(center, span, &p, bits, found, rng)
        {
            out.push(c);
        }
        // ---- Detail: straight into this boundary point.
        let gain = rng.range(3.0, 6.0);
        out.push(Candidate {
            kind: TargetKind::Detail,
            center: p.clone(),
            span: span * (-gain).exp(),
            period: None,
            preperiod: None,
            label: "boundary detail".into(),
            prescore: 0.0,
            extra: 0,
        });
    }
    Some(out)
}

#[allow(clippy::too_many_arguments)]
fn minibrot_candidates(
    out: &mut Vec<Candidate>,
    span: Scale,
    nucleus: &Center,
    sigma: Scale,
    period: usize,
    rng: &mut Rng,
    cancel: &AtomicBool,
) {
    let gain_of = |s: Scale| span.ratio(s).ln();
    let ok = |s: Scale| (GAIN_RANGE.0..=GAIN_RANGE.1).contains(&gain_of(s));
    // The nucleus at the precision a framing needs (Newton again from the nucleus: quick).
    let refine = |s: Scale| -> Option<Center> {
        let bits = bits_for(s);
        if nucleus.precision_bits() >= bits {
            return Some(nucleus.clone());
        }
        let mut c = || cancel.load(Ordering::Relaxed);
        find_nucleus(
            &nucleus.big().with_precision(bits),
            period,
            &NewtonOptions {
                precision_bits: Some(bits),
                max_steps: 32,
                max_distance: Some(sigma.big_times(0.5, 64)),
            },
            &mut c,
        )
        .map(|r| Center::from_big(r.nucleus))
    };
    // The minibrot itself, filling 1/8 ... 1/3 of the view (its cardioid spans ~2.5 σ), with
    // its decorations around it.
    let m_span = sigma * rng.range(6.0, 18.0);
    if ok(m_span)
        && let Some(c) = refine(m_span)
    {
        out.push(Candidate {
            kind: TargetKind::Minibrot,
            center: c,
            span: m_span,
            period: Some(period),
            preperiod: None,
            label: format!("period-{period} minibrot"),
            prescore: 0.0,
            extra: (4 * period).min(PROBE_ITER_CAP),
        });
    }
    // Embedded Julia stages between the view and the minibrot: the shapes around it double at
    // each halving of the remaining depth (t = 1/2, 3/4, 7/8 of the way in log scale), and
    // Heiland-Allen's estimate puts the 2-fold embedded Julia set at about σ^(3/4) (σ relative
    // to the whole set). Those that fit a leg are candidates; the probe decides.
    let ln_v = span.ln();
    let ln_s = sigma.ln();
    let at = |t: f64| Scale::from_log2(((1.0 - t) * ln_v + t * ln_s) / std::f64::consts::LN_2) * 3.0;
    let mut stages: Vec<Scale> = [0.5, 0.75, 0.875].iter().map(|&t| at(t)).collect();
    stages.push(Scale::from_log2(0.75 * sigma.log2()) * 2.0);
    stages.retain(|&s| ok(s) && s > m_span * 3.0);
    let off_r = rng.range(0.03, 0.18);
    let off_a = rng.range(0.0, std::f64::consts::TAU);
    if !stages.is_empty() {
        let j_span = stages[rng.index(stages.len())];
        if let Some(c) = refine(j_span) {
            // Aim slightly off the nucleus: the shapes around it double instead of the view
            // diving straight down the minibrot's axis.
            let c = c.offset_scaled(off_r * off_a.cos(), off_r * off_a.sin(), j_span);
            out.push(Candidate {
                kind: TargetKind::Julia,
                center: c,
                span: j_span,
                period: Some(period),
                preperiod: None,
                label: format!("embedded Julia (period {period})"),
                prescore: 0.0,
                extra: (2 * period).min(PROBE_ITER_CAP),
            });
        }
    }
    // A minibrot too deep for one leg: a leg on the way to it, centred on its nucleus (the
    // structure there leads the eye in; a later leg lands on it).
    if !ok(m_span) {
        let a_span = span * (-rng.range(5.0, 11.0)).exp();
        if a_span > m_span * 3.0
            && let Some(c) = refine(a_span)
        {
            out.push(Candidate {
                kind: TargetKind::Approach,
                center: c,
                span: a_span,
                period: Some(period),
                preperiod: None,
                label: format!("towards a period-{period} minibrot"),
                prescore: 0.0,
                extra: (2 * period).min(PROBE_ITER_CAP),
            });
        }
    }
}

/// The Misiurewicz point the orbit of `p` shadows, if Newton finds one near the view.
fn misiurewicz_candidate(
    center: &Center,
    span: Scale,
    p: &Center,
    bits: usize,
    atom_period: Option<usize>,
    rng: &mut Rng,
) -> Option<Candidate> {
    let mut periods: Vec<usize> = (1..=12).collect();
    if let Some(q) = atom_period
        && q > 12
    {
        periods.extend([q, 2 * q]);
    }
    let n_orbit = probe_iterations(span)
        .min(4 * periods.iter().max().copied().unwrap_or(12) + 3000)
        .min(20_000);
    let comp = ReferenceOrbitComputer::new(p.big(), bits);
    let (orbit, _) = with_no_control(4096, |ctl| comp.compute_double_exp(n_orbit, 1e6, ctl));
    let z: Vec<[f64; 2]> = orbit.iter().map(|v| [v.re.to_f64(), v.im.to_f64()]).collect();
    if z.len() < 8 {
        return None;
    }
    // (k, p) where the orbit comes closest to repeating itself after p steps.
    let mut best: Option<(f64, usize, usize)> = None;
    for &per in &periods {
        for k in 1..z.len().saturating_sub(per) {
            let a = z[k];
            let b = z[k + per];
            let d = (a[0] - b[0]).hypot(a[1] - b[1]) / (a[0].hypot(a[1]) + 1e-3) * (1.0 + 0.05 * per as f64);
            if best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, k, per));
            }
        }
    }
    let (_, k, per) = best?;
    let gain = rng.range(3.5, 8.5);
    let t_span = span * (-gain).exp();
    let tbits = bits_for(t_span).max(bits);
    for kk in [k, k.saturating_sub(1), k + 1] {
        // Preperiod 1 would be a nucleus (z_{p+1} = z_1 = c means c is periodic).
        if kk < 2 || kk + per > 20_000 {
            continue;
        }
        let r = find_misiurewicz(
            &p.big().with_precision(tbits),
            kk,
            per,
            &NewtonOptions {
                precision_bits: Some(tbits),
                max_steps: 40,
                max_distance: Some(span.big_times(0.5, 64)),
            },
        )?;
        let m = Center::from_big(r.point);
        if m.distance_in(center, span) > 0.5 {
            continue;
        }
        // The cycle's multiplier: complex (turning) for a spiral, real for a branch point.
        let comp = ReferenceOrbitComputer::new(m.big(), tbits);
        let (o, _) = with_no_control(4096, |ctl| comp.compute_double_exp(kk + per + 1, 1e6, ctl));
        if o.len() < kk + per {
            continue;
        }
        let mut lambda = DoubleExpComplex::ONE;
        for v in &o[kk..kk + per] {
            lambda = lambda * v.times_two();
        }
        let mag = lambda.magnitude();
        let (re, im) = if mag.is_zero() {
            (1.0, 0.0)
        } else {
            ((lambda.re / mag).to_f64(), (lambda.im / mag).to_f64())
        };
        let turn = im.atan2(re).abs();
        let spiral = turn > 0.25 && turn < std::f64::consts::PI - 0.25;
        let kind = if spiral {
            TargetKind::Spiral
        } else {
            TargetKind::Dendrite
        };
        return Some(Candidate {
            kind,
            center: m,
            span: t_span,
            period: Some(per),
            preperiod: Some(kk),
            label: format!("{} M({kk},{per})", if spiral { "spiral" } else { "branch point" }),
            prescore: 0.0,
            extra: 0,
        });
    }
    None
}

/// Planning in the background: the thread writes into its slot; a cancelled plan is dropped.
#[derive(Debug, Default)]
struct PlanSlot {
    cancel: AtomicBool,
    result: Mutex<Option<Option<Plan>>>,
}

/// Where a journey is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Waiting for the next target.
    Planning,
    /// A leg is flying.
    Flying,
    /// Resting at a target while it refines.
    Dwelling,
}

/// The journey's state in the navigator; see the module docs.
#[derive(Clone, Debug)]
pub struct Journey {
    active: bool,
    suspended: bool,
    seed: u64,
    daily: bool,
    leg: u32,
    phase: Phase,
    since: f64,
    speed: f64,
    /// This leg's speed factor ([`leg_pace`]).
    pace: f64,
    /// Palette phase at the start and the end of this leg (cycles, unwrapped; kept after a
    /// stop so the colours do not jump).
    palette: (f64, f64),
    pub speed_changed: f64,
    current: Option<Target>,
    next: Option<Plan>,
    chain: Chain,
    slot: Option<Arc<PlanSlot>>,
    /// Plan synchronously (tests and recorded flights: reproducible timing).
    pub synchronous: bool,
    /// The view the running plan started from (resume replans when the view moved).
    planned_from: Option<(Center, Scale)>,
}

impl Default for Journey {
    fn default() -> Self {
        Self {
            active: false,
            suspended: false,
            seed: 0,
            daily: false,
            leg: 0,
            phase: Phase::Planning,
            since: 0.0,
            speed: SPEED_DEFAULT,
            pace: 1.0,
            palette: (0.0, 0.0),
            speed_changed: f64::NEG_INFINITY,
            current: None,
            next: None,
            chain: Chain::new(0),
            slot: None,
            synchronous: false,
            planned_from: None,
        }
    }
}

/// What the navigator should do this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Fly a new leg to this target.
    Fly(Target),
}

impl Journey {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// On and not taken over by manual input.
    pub fn is_running(&self) -> bool {
        self.active && !self.suspended
    }

    pub fn is_suspended(&self) -> bool {
        self.active && self.suspended
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn is_daily(&self) -> bool {
        self.daily
    }

    pub fn leg(&self) -> u32 {
        self.leg
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// The speed this leg flies at: the journey speed times the leg's pace.
    pub fn leg_speed(&self) -> f64 {
        self.speed * self.pace
    }

    /// Sets the leg's pace from its path length in e-folds (the navigator, as a leg starts).
    pub fn set_leg_efolds(&mut self, efolds: f64) {
        self.pace = leg_pace(efolds);
    }

    /// The journey's palette phase (cycles) at flight progress `t` (0 … 1, eased; 1 when not
    /// flying). Applied by the renderer only with the cinematic effects on.
    pub fn palette_phase(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let e = t * t * (3.0 - 2.0 * t);
        self.palette.0 + (self.palette.1 - self.palette.0) * e
    }

    pub fn set_speed(&mut self, v: f64, now: f64) {
        if v.is_finite() {
            self.speed = v.clamp(SPEED_RANGE.0, SPEED_RANGE.1);
            self.speed_changed = now;
        }
    }

    pub fn scale_speed(&mut self, f: f64, now: f64) {
        if f.is_finite() && f > 0.0 {
            self.set_speed(self.speed * f, now);
        }
    }

    /// The target of the leg flying or dwelt at.
    pub fn current(&self) -> Option<&Target> {
        self.current.as_ref()
    }

    /// The next target, once planned (its reference can be prefetched).
    pub fn next_target(&self) -> Option<&Target> {
        self.next.as_ref().map(|p| &p.target)
    }

    /// Seconds to rest at a target at the current speed.
    pub fn dwell_time(&self) -> f64 {
        (DWELL * (SPEED_DEFAULT / self.speed).sqrt()).clamp(1.0, 6.0)
    }

    /// Starts a journey with `seed` from the view (`center`, shorter-side `span`).
    pub fn start(&mut self, seed: u64, daily: bool, center: &Center, span: Scale) {
        self.cancel_plan();
        self.active = true;
        self.suspended = false;
        self.seed = seed;
        self.daily = daily;
        self.leg = 0;
        self.current = None;
        self.next = None;
        self.chain = Chain::starting_at(seed, center, span);
        // Colours go on from where they are (the chain's phases are relative to this).
        self.palette = (self.palette.1, self.palette.1);
        self.chain.palette_phase = self.palette.1;
        self.phase = Phase::Planning;
        self.request(center, span);
    }

    /// Off (Esc, reset, J while running).
    pub fn stop(&mut self) {
        self.cancel_plan();
        self.active = false;
        self.suspended = false;
        self.next = None;
        self.current = None;
    }

    /// Manual input took over: the journey waits for a resume.
    pub fn suspend(&mut self) {
        if self.active {
            self.suspended = true;
        }
    }

    /// Resumes from the view the user left it at: a fresh plan from there (the seeded chain
    /// continues), unless the view is still the one the pending plan started from.
    pub fn resume(&mut self, center: &Center, span: Scale) {
        if !self.is_suspended() {
            return;
        }
        self.suspended = false;
        let unchanged = self
            .planned_from
            .as_ref()
            .is_some_and(|(c, s)| c.distance_in(center, span) < 1e-3 && s.ratio(span).ln().abs() < 1e-3)
            && (self.next.is_some() || self.slot.is_some());
        let at_target = self
            .current
            .as_ref()
            .is_some_and(|t| t.center.distance_in(center, span) < 1e-3 && t.span.ratio(span).ln().abs() < 1e-3);
        if at_target && (self.next.is_some() || self.slot.is_some()) || unchanged {
            self.phase = if self.next.is_some() {
                Phase::Dwelling
            } else {
                Phase::Planning
            };
            return;
        }
        self.cancel_plan();
        self.next = None;
        self.phase = Phase::Planning;
        self.request(center, span);
    }

    fn cancel_plan(&mut self) {
        if let Some(s) = self.slot.take() {
            s.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Plans the next target from (`center`, `span`) with the current chain.
    fn request(&mut self, center: &Center, span: Scale) {
        self.planned_from = Some((center.clone(), span));
        let chain = self.chain.clone();
        if self.synchronous {
            let never = AtomicBool::new(false);
            self.next = plan(center, span, &chain, &never);
            return;
        }
        let slot = Arc::new(PlanSlot::default());
        let s = slot.clone();
        let c = center.clone();
        let spawned = std::thread::Builder::new().name("journey-plan".into()).spawn(move || {
            let p = plan(&c, span, &chain, &s.cancel);
            if let Ok(mut r) = s.result.lock() {
                *r = Some(p);
            }
        });
        if spawned.is_ok() {
            self.slot = Some(slot);
        }
    }

    /// Collects a finished background plan.
    fn poll(&mut self) {
        let Some(slot) = &self.slot else { return };
        let done = slot.result.lock().ok().and_then(|mut r| r.take());
        if let Some(p) = done {
            self.slot = None;
            self.next = p;
        }
    }

    /// Advances the journey: `flying` says whether a flight runs. Returns a new leg to fly.
    pub fn tick(&mut self, now: f64, flying: bool) -> Option<Action> {
        if !self.is_running() {
            return None;
        }
        self.poll();
        match self.phase {
            Phase::Flying if !flying => {
                self.phase = Phase::Dwelling;
                self.since = now;
                None
            }
            Phase::Flying => None,
            Phase::Dwelling if now - self.since < self.dwell_time() => None,
            Phase::Dwelling | Phase::Planning => {
                let p = self.next.take()?;
                self.chain = p.chain.clone();
                self.leg += 1;
                self.phase = Phase::Flying;
                let t = p.target.clone();
                self.palette = (self.palette.1, t.palette_phase);
                self.current = Some(t.clone());
                // Plan the leg after this one from this target while it flies.
                self.request(&t.center, t.span);
                Some(Action::Fly(t))
            }
        }
    }

    /// Waiting for a plan longer than the dwell (the HUD says "planning").
    pub fn is_planning(&self) -> bool {
        self.is_running() && self.next.is_none() && self.phase != Phase::Flying
    }

    /// The chain after the last leg started (tests).
    pub fn chain(&self) -> &Chain {
        &self.chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain_of(seed: u64, legs: usize) -> Vec<Target> {
        let never = AtomicBool::new(false);
        let mut chain = Chain::new(seed);
        let mut c = Center::new(-0.75, 0.0);
        let mut span = Scale::from_f64(3.0);
        let mut out = Vec::new();
        for _ in 0..legs {
            let p = plan(&c, span, &chain, &never).expect("plan");
            chain = p.chain;
            c = p.target.center.clone();
            span = p.target.span;
            out.push(p.target);
        }
        out
    }

    #[test]
    fn journeys_are_reproducible_by_seed() {
        let a = chain_of(4711, 4);
        let b = chain_of(4711, 4);
        assert_eq!(a, b);
        let c = chain_of(4712, 4);
        assert_ne!(a, c, "another seed, another journey");
        for t in &a {
            println!(
                "{:<30} {:>8.2} e-folds  zoom 10^{:.1}  score {:.2}  {:?}",
                t.label,
                t.gain,
                t.zoom_log10(),
                t.score,
                t.stats
            );
        }
    }

    #[test]
    fn targets_are_on_the_boundary_and_not_interior() {
        for seed in [1u64, 2, 3] {
            let ts = chain_of(seed, 4);
            let kinds: std::collections::HashSet<_> = ts.iter().map(|t| t.kind.family()).collect();
            assert!(
                kinds.len() >= 2,
                "seed {seed}: kinds vary: {:?}",
                ts.iter().map(|t| t.kind).collect::<Vec<_>>()
            );
            for t in &ts {
                assert!(t.stats.reject().is_none() || t.kind == TargetKind::Surface, "{t:?}");
                assert!(t.stats.interior < 0.6, "{}: interior {}", t.label, t.stats.interior);
                // Near the set: the probe has boundary structure around its centre.
                assert!(t.stats.centre_detail > 1.0, "{}: {:?}", t.label, t.stats);
                // Minibrots: the nucleus is periodic (z_p = 0) and inside the set.
                if t.kind == TargetKind::Minibrot {
                    let p = t.period.unwrap();
                    let comp = ReferenceOrbitComputer::new(t.center.big(), t.center.precision_bits());
                    let (o, _) = with_no_control(4096, |ctl| comp.compute_double_exp(p, 4.0, ctl));
                    assert_eq!(o.len(), p + 1, "{} escapes", t.label);
                    let zp = o[p].magnitude().log2_magnitude();
                    assert!(zp < t.span.log2() + 4.0, "{}: |z_p| 2^{zp}", t.label);
                }
            }
        }
    }

    #[test]
    fn journeys_cross_regions_and_do_not_repeat_looks() {
        for seed in [4711u64, 7, 20_261_015] {
            let ts = chain_of(seed, 16);
            let regions: Vec<RegionId> = ts.iter().map(|t| region_of(t.center.re(), t.center.im()).0).collect();
            let distinct: std::collections::HashSet<_> = regions.iter().collect();
            assert!(distinct.len() >= 5, "seed {seed}: regions {regions:?}");
            // A dive is the cross-over and at most two legs.
            let mut run = 1;
            for w in regions.windows(2) {
                run = if w[0] == w[1] { run + 1 } else { 1 };
                assert!(run <= 3, "seed {seed}: {run} legs in one region: {regions:?}");
            }
            // No landing is a near-duplicate of the one before.
            for w in ts.windows(2) {
                let d = w[0].look.distance(&w[1].look);
                assert!(
                    d >= DUPLICATE,
                    "seed {seed}: {} after {}: look distance {d:.2}",
                    w[1].label,
                    w[0].label
                );
            }
            assert!(ts.iter().any(|t| t.kind == TargetKind::Crossing), "seed {seed}");
        }
    }

    #[test]
    fn novelty_rejects_a_repeat_and_rewards_a_new_look() {
        let f = probe(&Center::new(-0.745, 0.11), Scale::from_f64(0.02), 0);
        let seahorse = look_of(&f, &Center::new(-0.745, 0.11), Scale::from_f64(0.02));
        let visit = |look: Look| Visit {
            kind: TargetKind::Spiral,
            center: Center::new(0.0, 0.0),
            span: Scale::from_f64(1.0),
            direction: None,
            look,
            region: 0,
        };
        let (_, dup) = novelty(&seahorse, &[visit(seahorse)], DUPLICATE);
        assert!(dup, "the same look again is a duplicate");
        // A branch point looks different: no duplicate, and more novelty than the repeat.
        // The 1/3 limb's principal branch point M(4,1): straight dendrite arms.
        let c = Center::new(-0.101_096_363_845_622, 0.956_286_510_809_142);
        let needle = look_of(&probe(&c, Scale::from_f64(0.01), 0), &c, Scale::from_f64(0.01));
        let (n_new, dup) = novelty(&needle, &[visit(seahorse)], DUPLICATE);
        let (n_same, _) = novelty(&seahorse, &[visit(seahorse)], DUPLICATE);
        assert!(
            !dup && n_new > n_same + 0.2,
            "{n_new} vs {n_same}: {:?} / {:?}",
            needle,
            seahorse
        );
        // Spiral pitch: the seahorse valley turns, the branch point's arms do not.
        assert!(
            seahorse.v[16] > needle.v[16] + 0.2,
            "{} {}",
            seahorse.v[16],
            needle.v[16]
        );
    }

    #[test]
    fn long_legs_fly_faster() {
        assert_eq!(leg_pace(5.0), 1.0);
        assert_eq!(leg_pace(LONG_LEG), 1.0);
        // 92 e-folds out and back in (a cross-over from 1e40) take what ~29.6 would.
        let p = leg_pace(92.0);
        assert!((92.0 / p - (LONG_LEG + LONG_LEG_SHARE * 78.0)).abs() < 1e-9, "{p}");
    }

    #[test]
    fn daily_seed_is_the_date() {
        assert_eq!(daily_seed(2026, 10, 15), 20_261_015);
    }

    #[test]
    fn probe_stats_reject_interior_and_empty() {
        // Inside the main cardioid: interior.
        let f = probe(&Center::new(-0.1, 0.0), Scale::from_f64(0.05), 0);
        assert_eq!(ProbeStats::of(&f).reject(), Some("interior"));
        // Far outside: featureless.
        let f = probe(&Center::new(1.5, 1.5), Scale::from_f64(0.05), 0);
        assert_eq!(ProbeStats::of(&f).reject(), Some("featureless"));
        // Seahorse valley: structure.
        let f = probe(&Center::new(-0.745, 0.11), Scale::from_f64(0.02), 0);
        let s = ProbeStats::of(&f);
        assert!(s.reject().is_none(), "{s:?}");
    }

    #[test]
    fn journey_state_machine_flies_dwells_and_flies_on() {
        let mut j = Journey {
            synchronous: true,
            ..Journey::default()
        };
        j.start(99, false, &Center::new(-0.75, 0.0), Scale::from_f64(3.0));
        let Some(Action::Fly(t1)) = j.tick(0.0, false) else {
            panic!("first leg")
        };
        assert_eq!(j.phase(), Phase::Flying);
        assert!(j.next_target().is_some(), "the next target is planned during the leg");
        assert_eq!(j.tick(1.0, true), None);
        assert_eq!(j.tick(2.0, false), None);
        assert_eq!(j.phase(), Phase::Dwelling);
        assert_eq!(j.tick(2.0 + j.dwell_time() * 0.5, false), None);
        let Some(Action::Fly(t2)) = j.tick(2.0 + j.dwell_time() + 0.01, false) else {
            panic!("second leg")
        };
        assert_ne!(t1, t2);
        assert_eq!(j.leg(), 2);
        j.suspend();
        assert_eq!(j.tick(10.0, false), None);
        j.resume(&t2.center, t2.span);
        assert!(j.is_running());
    }
}
