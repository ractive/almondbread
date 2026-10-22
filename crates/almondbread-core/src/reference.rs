//! Reference orbits the GPU perturbation kernels iterate against (kb: techniques/perturbation,
//! techniques/reference-selection, techniques/rebasing).
//!
//! Two kinds:
//! - **f64** (`Reference::compute_f64`): `Z_{n+1} = Z_n^2 + C` in f64 at the view centre. Its
//!   cost is linear in its length (about 0.36 ms per 1e5 points when the centre never escapes,
//!   M1 Pro): the scheduler computes it on the frame thread only up to [`F64_SYNC_POINTS`],
//!   longer ones (manual limits, raised caps) on the [`ReferenceWorker`]
//!   ([`ReferenceRequest::f64_orbit`]). Serves views down to [`DEEP_SCALE`] per point, exactly
//!   as M1–M4 did (bit-identical fields at the benchmark locations).
//! - **big** (`Reference::compute_big`, M3b): the orbit at the precision the view needs
//!   (`almondbread_math`, BigFloat), at a minibrot nucleus inside the view when the ball period
//!   detector and Newton find one (orbit length = period, `Z_p = 0`, wraps by rebasing), else at
//!   the view centre. Computed off the main thread by a [`ReferenceWorker`]; points are stored
//!   as float32 pairs (hot path) and in shared-exponent floatexp (the kernel's full steps where
//!   `|Z|` is tiny).
//!
//! Rebasing makes any reference near the view exact (one reference serves the whole image), so
//! a reference is reused while panning and zooming as long as its precision covers the pixel
//! spacing and it lies within float32 reach of the pixels ([`Reference::fit`]).

use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use almondbread_math::{
    BigFloat, DoubleExpComplex, FloatExpComplex, FloatExpSplit, Formula, PeriodMethod, ReferenceOrbitComputer, Threads, recommended_precision_bits_with,
};

use crate::julia_reference::{self, JuliaLayout};
use crate::viewport::{Center, Scale, Viewport};

/// Views with a scale per point below this use big references (and, per texel, the deep
/// kernel when deltas leave float32's comfortable range). The f64 orbit is verified to the
/// f64 limit (~4e-16 |c|); 1e-12 leaves room for long orbits.
pub const DEEP_SCALE: f64 = 1e-12;

/// Big references made for views below this scale per point run the deep (rescaled,
/// extended-range) kernel; above it the plain float32 kernel, which is ~30 % faster per
/// iteration and safe while deltas stay far above float32's squared-norm floor (~1e-19).
pub const DEEP_KERNEL_SCALE: f64 = 1e-17;
/// A plain-kernel big reference serves views down to this scale (below: unusable), and is
/// replaced in the background from `DEEP_KERNEL_SCALE` on.
pub const PLAIN_KERNEL_FLOOR: f64 = 1e-18;

/// Guard bits on top of the pixel spacing (and view size) for a reference to be usable at all,
/// and to be considered good (below: recompute in the background, keep using meanwhile).
pub const MIN_GUARD_BITS: usize = 20;
pub const GOOD_GUARD_BITS: usize = 40;
/// Extra bits a new big reference gets, so it stays good while zooming ~19 more decades.
pub const EXTRA_BITS: usize = 64;
/// A reference farther than this many texels from the view centre is unusable (float32
/// texel offsets lose sub-texel resolution beyond).
pub const MAX_OFFSET_TEXELS: f64 = 65536.0;

/// The first prefix a progressive centre reference hands out, as a fraction of the
/// requested length (R6: 1/16 of 2x the limit = 1/8 of the limit).
pub const PARTIAL_FIRST_FRACTION: f64 = 1.0 / 16.0;
/// Prefixes shorter than this are not handed out (the whole orbit is quick anyway).
pub const PARTIAL_MIN_POINTS: usize = 4096;

/// Longest f64 orbit the scheduler computes on the frame thread (C1 of the 2026-10 review):
/// 2^18 points, ~0.9 ms where the centre never escapes (M1 Pro). The default automatic cap
/// (200 000) stays below it, so the default path is unchanged; longer orbits (manual limits,
/// a raised cap) go to the [`ReferenceWorker`].
pub const F64_SYNC_POINTS: usize = 1 << 18;

/// Orbit storage reserved up front; longer orbits grow as they are computed (an orbit
/// usually escapes long before a high limit, and a request of 1e9 must not reserve 8 GB).
const PREALLOC_POINTS: usize = 1 << 18;

/// A big centre orbit longer than this is computed in stages that double its storage (below,
/// as before: the stages of the prefixes it hands out, storage for each reserved at once).
const GROW_FROM_POINTS: usize = 1 << 20;

/// What a [`ReferencePrepare`] hook made of a reference on the worker thread (the renderer's
/// GPU buffers): opaque to this crate.
pub type ReferenceAttachment = Arc<dyn Any + Send + Sync>;

/// Runs on the [`ReferenceWorker`]'s thread for every reference it hands out (prefixes too),
/// so that work proportional to the orbit's length (GPU upload, conversions) stays off the
/// frame thread (C2 of the 2026-10 review).
pub type ReferencePrepare = Arc<dyn Fn(&Reference) -> Option<ReferenceAttachment> + Send + Sync>;

/// A reference orbit and where it came from.
#[derive(Clone, Debug)]
pub struct Reference {
    /// The iteration it is an orbit of (F2): a reference serves views of its formula only.
    pub formula: Formula,
    /// The reference point C (the orbit is exactly this point's).
    pub point: Center,
    /// The view scale (per point) it was made for.
    pub scale: Scale,
    /// Precision of the orbit in bits (53 for the f64 kind).
    pub precision_bits: usize,
    /// Iteration limit it was computed for.
    pub max_iter: usize,
    /// Z_0 ... Z_{len-1} rounded to float32 (Z_0 = 0). Shared: table builds and copies of the
    /// reference never copy the orbit.
    pub points: Arc<Vec<[f32; 2]>>,
    /// The same in floatexp (big references only; empty for f64).
    pub extended: Arc<Vec<FloatExpComplex>>,
    /// The same with each part its own exponent (F6, the folding formulas' big references only,
    /// else empty): the GPU's floatexp reference for the Burning Ship, whose fold needs `X Y`
    /// where one part is far below the other ([`FloatExpSplit`]).
    pub split: Arc<Vec<FloatExpSplit>>,
    /// Nucleus period (then `points.len() == period + 1` and `Z_period ~ 0`).
    pub period: Option<usize>,
    /// The period the detector reported (also when Newton was not used).
    pub detected_period: Option<usize>,
    pub escaped: bool,
    pub big: bool,
    /// Iterate with the deep kernel (a big reference made below [`DEEP_KERNEL_SCALE`]).
    pub deep_kernel: bool,
    /// Seconds: selection (period + Newton) and orbit.
    pub select_time: f64,
    pub orbit_time: f64,
    /// Made from it on the worker thread ([`ReferencePrepare`]); None when computed elsewhere.
    pub prepared: Option<ReferenceAttachment>,
    /// A Julia reference (F4): `points` hold the critical orbit, zeros, then the centre orbit
    /// ([`crate::julia_reference`]); `point` is the centre (the orbit's `Z_0`), `escaped` and
    /// `period` describe the centre orbit.
    pub julia: Option<JuliaLayout>,
}

/// How well a reference serves a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Use it.
    Good,
    /// Usable (correct), but a better one should be computed in the background.
    Stale,
    /// Not usable for this view.
    No,
}

/// Bits a reference needs for a view with texel spacing `texel` and `size` texels across.
pub fn needed_bits(point: &Center, texel: Scale, size: usize, guard: usize) -> usize {
    let mag = point.re().abs().max(point.im().abs()).max(1.0);
    recommended_precision_bits_with(texel.log2(), size, mag, guard)
}

impl Reference {
    /// The f64 orbit at the view centre: iterates until it escapes `bailout` or `max_iter`
    /// steps are done. Always at least two points, so the GPU loop has a valid step. Same
    /// arithmetic as the Swift `ReferenceOrbit` (no FMA contraction), so the points are
    /// bit-identical to M1–M4.
    pub fn compute_f64(viewport: &Viewport, max_iter: usize, bailout: f64) -> Self {
        Self::compute_f64_with(viewport, Formula::Mandelbrot, max_iter, bailout)
    }

    /// [`Reference::compute_f64`] for any formula (F2; Mandelbrot runs the loop it always ran).
    pub fn compute_f64_with(viewport: &Viewport, formula: Formula, max_iter: usize, bailout: f64) -> Self {
        Self::compute_f64_cancellable(viewport, formula, max_iter, bailout, &AtomicBool::new(false), None)
            .expect("never cancelled")
    }

    /// [`Reference::compute_f64_with`] that polls `cancel` (and stores the iterations done in
    /// `progress`) every 65 536 iterations; None when cancelled. The points are the same (the
    /// loop only runs in slices). Storage grows with the orbit past [`PREALLOC_POINTS`]
    /// instead of reserving `max_iter` points up front.
    pub fn compute_f64_cancellable(
        viewport: &Viewport,
        formula: Formula,
        max_iter: usize,
        bailout: f64,
        cancel: &AtomicBool,
        progress: Option<&AtomicUsize>,
    ) -> Option<Self> {
        const SLICE: usize = 1 << 16;
        let start = Instant::now();
        let (cr, ci) = (viewport.center.re(), viewport.center.im());
        if let Formula::Julia(k) = formula {
            // F4: the critical orbit of k, then the centre orbit (crate::julia_reference).
            let kf = k.approx();
            let (crit, crit_escaped) = julia_reference::orbit_f64(kf, (0.0, 0.0), max_iter, bailout, cancel)?;
            let (center, escaped) = julia_reference::orbit_f64(kf, (cr, ci), max_iter, bailout, cancel)?;
            if let Some(p) = progress {
                p.store(center.len() - 1, Ordering::Relaxed);
            }
            let layout = JuliaLayout {
                crit_last: crit.len() - 1,
                center_start: julia_reference::center_start(crit.len()),
                crit_escaped,
            };
            return Some(Self {
                formula,
                point: Center::new(cr, ci),
                scale: viewport.scale,
                precision_bits: 53,
                max_iter,
                points: Arc::new(julia_reference::assemble(&crit, &center, [0.0; 2])),
                extended: Arc::default(),
            split: Arc::default(),
                period: None,
                detected_period: None,
                escaped,
                big: false,
                deep_kernel: false,
                select_time: 0.0,
                orbit_time: start.elapsed().as_secs_f64(),
                prepared: None,
                julia: Some(layout),
            });
        }
        let mut pts = Vec::with_capacity(max_iter.min(PREALLOC_POINTS) + 1);
        let (mut x, mut y) = (0.0f64, 0.0f64);
        let b2 = bailout * bailout;
        pts.push([0.0f32, 0.0]);
        let mut escaped = false;
        let mut done = 0usize;
        while done < max_iter && !escaped {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let n = (max_iter - done).min(SLICE);
            if formula.is_mandelbrot() {
                for _ in 0..n {
                    let nx = x * x - y * y + cr;
                    y = 2.0 * x * y + ci;
                    x = nx;
                    pts.push([x as f32, y as f32]);
                    if x * x + y * y > b2 {
                        escaped = true;
                        break;
                    }
                }
            } else {
                for _ in 0..n {
                    (x, y) = formula.step((x, y), (cr, ci));
                    pts.push([x as f32, y as f32]);
                    if x * x + y * y > b2 {
                        escaped = true;
                        break;
                    }
                }
            }
            done += n;
            if let Some(p) = progress {
                p.store(pts.len() - 1, Ordering::Relaxed);
            }
        }
        if pts.len() < 2 {
            pts.push([cr as f32, ci as f32]);
        }
        Some(Self {
            formula,
            // The orbit is of the f64-rounded centre: offsets are taken from that point.
            point: Center::new(cr, ci),
            scale: viewport.scale,
            precision_bits: 53,
            max_iter,
            points: Arc::new(pts),
            extended: Arc::default(),
            split: Arc::default(),
            period: None,
            detected_period: None,
            escaped,
            big: false,
            deep_kernel: false,
            select_time: 0.0,
            orbit_time: start.elapsed().as_secs_f64(),
            prepared: None,
            julia: None,
        })
    }

    /// A big reference for `request` (see the module docs). `cancel` is polled every 4096
    /// iterations; returns None when cancelled. `progress` counts orbit iterations done.
    pub fn compute_big(request: &ReferenceRequest, cancel: &AtomicBool, progress: &AtomicUsize) -> Option<Self> {
        Self::compute_big_progressive(request, cancel, progress, &mut |_| {})
    }

    /// [`Reference::compute_big`] that also hands out prefixes of a long centre orbit as
    /// soon as they are computed (R6, kb: techniques/parallel-reference-orbit), so the first
    /// frame need not wait for the whole orbit. A prefix is exact only for pixels that
    /// escape before its end: one that outruns it rebases with a rounding loss
    /// ([`Reference::truncated_before`]), so a frame rendered with a prefix is provisional
    /// and the scheduler renders it again when the whole orbit arrives. `partial` gets the
    /// prefix of [`PARTIAL_FIRST_FRACTION`] of the request (a short, `Stale` reference),
    /// then the prefix of half of it (the view's own limit when the request is the
    /// scheduler's 2x: `Good`). The final reference is bit-identical to an uninterrupted
    /// computation's.
    pub fn compute_big_progressive(
        request: &ReferenceRequest,
        cancel: &AtomicBool,
        progress: &AtomicUsize,
        partial: &mut dyn FnMut(Reference),
    ) -> Option<Self> {
        let bits = request.precision_bits.div_ceil(64) * 64;
        let center = request.center.with_min_precision(bits);
        if let Formula::Julia(k) = request.formula {
            return Self::compute_julia_big(request, k, &center, bits, cancel, progress);
        }
        let t0 = Instant::now();
        let mut is_cancelled = || cancel.load(Ordering::Relaxed);
        let (point, period, detected) = if request.nucleus {
            let radius = request.scale.big_times(request.radius_points, 64);
            let choice = almondbread_math::select_reference_for(
                request.formula,
                center.big(),
                &radius,
                request.period_limit.max(1),
                Some(bits),
                PeriodMethod::Ball,
                &mut is_cancelled,
            );
            if is_cancelled() {
                return None;
            }
            (Center::from_big(choice.point), choice.period, choice.detected_period)
        } else {
            (center.clone(), None, None)
        };
        let select_time = t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let iters = period.unwrap_or(request.max_iter).max(1);
        let comp = ReferenceOrbitComputer::new(point.big(), bits)
            .with_threads(Threads::Automatic)
            .with_formula(request.formula);
        // Storage grows stage by stage (a centre orbit usually escapes long before the end of
        // the request; C1 of the 2026-10 review: twice a 1e9 limit reserved 48 GB up front).
        let mut points: Vec<[f32; 2]> = Vec::new();
        let mut extended: Vec<FloatExpComplex> = Vec::new();
        // F6: a folding formula's orbit also with each part its own exponent.
        let folds = request.formula.folds();
        let mut split: Vec<FloatExpSplit> = Vec::new();
        let mut never_progress = |n: usize| progress.store(n, Ordering::Relaxed);
        let mut ctl = almondbread_math::Control {
            check_interval: 4096,
            is_cancelled: &mut is_cancelled,
            progress: &mut never_progress,
        };
        // Centre orbits run in stages, handing out the prefixes at `handouts`; a nucleus orbit
        // (one period) in one go.
        let mut handouts = Vec::new();
        if period.is_none() {
            let first = (iters as f64 * PARTIAL_FIRST_FRACTION) as usize;
            if first >= PARTIAL_MIN_POINTS {
                handouts.push(first);
            }
            if iters / 2 > first && iters / 2 >= PARTIAL_MIN_POINTS {
                handouts.push(iters / 2);
            }
        }
        let mut stages = handouts.clone();
        stages.push(iters);
        // Past GROW_FROM_POINTS the storage doubles stage by stage (resuming is exact), so an
        // orbit that escapes early never holds the whole request.
        if period.is_none() {
            let mut s = GROW_FROM_POINTS;
            while s < iters {
                stages.push(s);
                s *= 2;
            }
            stages.sort_unstable();
            stages.dedup();
        }
        let mut r: Option<almondbread_math::OrbitResult> = None;
        for &stage in &stages {
            points.resize(stage + 1, [0.0; 2]);
            extended.resize(stage + 1, FloatExpComplex::ZERO);
            let next = if folds {
                split.resize(stage + 1, FloatExpSplit::ZERO);
                comp.compute_with_split(
                    r.as_ref(),
                    stage,
                    request.bailout,
                    Some(&mut points),
                    Some(&mut extended),
                    &mut split,
                    &mut ctl,
                )
            } else {
                match &r {
                None => comp.compute(stage, request.bailout, Some(&mut points), Some(&mut extended), &mut ctl),
                Some(prev) => comp.resume(
                    prev,
                    stage,
                    request.bailout,
                    Some(&mut points),
                    Some(&mut extended),
                    &mut ctl,
                ),
                }
            };
            if next.cancelled {
                return None;
            }
            let done = next.escaped || stage == iters;
            if !done && handouts.contains(&stage) {
                partial(Self {
                    formula: request.formula,
                    point: point.clone(),
                    scale: request.scale,
                    precision_bits: bits,
                    max_iter: request.max_iter,
                    points: Arc::new(points[..next.count].to_vec()),
                    extended: Arc::new(extended[..next.count].to_vec()),
                    split: Arc::new(split[..split.len().min(next.count)].to_vec()),
                    period: None,
                    detected_period: detected,
                    escaped: false,
                    big: true,
                    deep_kernel: request.scale.to_f64() < DEEP_KERNEL_SCALE,
                    select_time,
                    orbit_time: t1.elapsed().as_secs_f64(),
                    prepared: None,
                    julia: None,
                });
            }
            r = Some(next);
            if done {
                break;
            }
        }
        let r = r.expect("at least one stage");
        points.truncate(r.count);
        extended.truncate(r.count);
        split.truncate(r.count);
        if points.len() < 2 {
            // C itself escaped at once: still give the kernel one valid step.
            let c = point.big().to_f32();
            points.push(c);
            extended.push(point.big().float_exp_complex());
            if folds {
                let b = point.big();
                split.push(FloatExpSplit::from_double_exp(DoubleExpComplex::new(
                    b.re.double_exp(),
                    b.im.double_exp(),
                )));
            }
        }
        if period.is_some() {
            // Z_p is 0 up to rounding at this precision: make the wrap exact.
            let p = points.len() - 1;
            points[p] = [0.0, 0.0];
            extended[p] = FloatExpComplex::ZERO;
            if folds {
                split[p] = FloatExpSplit::ZERO;
            }
        }
        Some(Self {
            formula: request.formula,
            point,
            scale: request.scale,
            precision_bits: bits,
            max_iter: request.max_iter,
            points: Arc::new(points),
            extended: Arc::new(extended),
            split: Arc::new(split),
            period,
            detected_period: detected,
            escaped: r.escaped,
            big: true,
            deep_kernel: request.scale.to_f64() < DEEP_KERNEL_SCALE,
            select_time,
            orbit_time: t1.elapsed().as_secs_f64(),
            prepared: None,
            julia: None,
        })
    }

    /// F4: a big Julia reference: the critical orbit of `k` (cached per `k`, at least the
    /// centre orbit's precision) and the centre orbit, concatenated
    /// ([`crate::julia_reference`]). No nucleus search and no prefixes: both orbits whole.
    fn compute_julia_big(
        request: &ReferenceRequest,
        k: almondbread_math::JuliaK,
        center: &Center,
        bits: usize,
        cancel: &AtomicBool,
        progress: &AtomicUsize,
    ) -> Option<Self> {
        let t1 = Instant::now();
        let iters = request.max_iter.max(1);
        let crit = julia_reference::critical_orbit(k, bits, iters, request.bailout, cancel)?;
        let (cp, ce, escaped) = julia_reference::center_orbit(k, center.big(), bits, iters, request.bailout, cancel)?;
        progress.store(cp.len() - 1, Ordering::Relaxed);
        let layout = JuliaLayout {
            crit_last: crit.points.len() - 1,
            center_start: julia_reference::center_start(crit.points.len()),
            crit_escaped: crit.escaped,
        };
        Some(Self {
            formula: request.formula,
            point: center.clone(),
            scale: request.scale,
            precision_bits: bits,
            max_iter: request.max_iter,
            points: Arc::new(julia_reference::assemble(&crit.points, &cp, [0.0; 2])),
            extended: Arc::new(julia_reference::assemble(&crit.extended, &ce, FloatExpComplex::ZERO)),
            split: Arc::default(),
            period: None,
            detected_period: None,
            escaped,
            big: true,
            deep_kernel: request.scale.to_f64() < DEEP_KERNEL_SCALE,
            select_time: 0.0,
            orbit_time: t1.elapsed().as_secs_f64(),
            prepared: None,
            julia: Some(layout),
        })
    }

    /// Iterations of the orbit the view's pixels start on (the centre orbit; for Julia the
    /// second part of `points`).
    pub fn orbit_iterations(&self) -> usize {
        match &self.julia {
            Some(j) => j.center_iterations(self.points.len()),
            None => self.points.len() - 1,
        }
    }

    /// Seconds it took in all.
    pub fn time(&self) -> f64 {
        self.select_time + self.orbit_time
    }

    /// True when the orbit stops before `max_iter` although it neither escaped nor closed a
    /// period: a prefix of a centre orbit still being computed, or one made for a lower limit.
    /// Its end is no exact rebase point (kb: techniques/rebasing, "Truncated references"): a
    /// pixel that reaches it has `|z| << |Z + z|`, and `z <- Z + z` rounds its offset from the
    /// reference away, so every such pixel continues as the same orbit (a flat colour). After
    /// an escape (`|z|` is then comparable to `|Z|`) or at a nucleus (`Z_p = 0`) the reset is
    /// exact. Usable for a first frame, which must be rendered again once the whole orbit is in.
    pub fn truncated_before(&self, max_iter: usize) -> bool {
        if let Some(j) = &self.julia {
            // (Both orbits: a pixel's index on either never exceeds its n.)
            return self.big
                && ((!self.escaped && self.orbit_iterations() < max_iter)
                    || (!j.crit_escaped && j.crit_last < max_iter));
        }
        self.big && self.period.is_none() && !self.escaped && self.points.len() - 1 < max_iter
    }

    /// How far pixels may be iterated with this reference: unbounded when its end is an exact
    /// rebase point (an escape or a nucleus period), else its last index. Holds for the f64
    /// reference too: one cut at the limit it was made for (the centre did not escape) is exact
    /// up to that limit, and an extension pass past it rebases every pixel at the cut with a
    /// rounding loss (kb: techniques/rebasing, "Truncated references").
    pub fn exact_reach(&self) -> usize {
        if let Some(j) = &self.julia {
            let center = if self.escaped {
                usize::MAX
            } else {
                self.orbit_iterations()
            };
            let crit = if j.crit_escaped { usize::MAX } else { j.crit_last };
            return center.min(crit);
        }
        if self.escaped || self.period.is_some() {
            usize::MAX
        } else {
            self.points.len() - 1
        }
    }

    /// How well this reference serves `viewport` rendered with texels of `texel` complex units,
    /// `size` texels across (largest side) and `max_iter` iterations.
    pub fn fit(&self, viewport: &Viewport, texel: Scale, size: usize, max_iter: usize) -> Fit {
        let deep = viewport.scale.to_f64() < DEEP_SCALE;
        if !self.big {
            // The f64 reference: as in M1–M4 (within the view, made at a scale within 4x,
            // long enough).
            if deep {
                return Fit::No;
            }
            let ok = self.max_iter >= max_iter && f64_reference_near(&self.point, self.scale, viewport, size);
            return if ok { Fit::Good } else { Fit::No };
        }
        if !deep {
            // Shallow views keep the f64 path (bit-identical with earlier milestones).
            return Fit::No;
        }
        if self.precision_bits < needed_bits(&self.point, texel, size, MIN_GUARD_BITS) {
            return Fit::No;
        }
        let offset = self.point.distance_in(&viewport.center, texel);
        if offset.is_nan() || offset > MAX_OFFSET_TEXELS {
            return Fit::No;
        }
        let vs = viewport.scale.to_f64();
        if !self.deep_kernel && vs < PLAIN_KERNEL_FLOOR {
            return Fit::No;
        }
        let kernel_stale = !self.deep_kernel && vs < DEEP_KERNEL_SCALE;
        let short = self.truncated_before(max_iter);
        let far = offset > 2.0 * size as f64;
        if self.precision_bits < needed_bits(&self.point, texel, size, GOOD_GUARD_BITS) || short || far || kernel_stale
        {
            return Fit::Stale;
        }
        Fit::Good
    }
}

/// Whether an f64 reference made at `point` for a view of `scale` lies close enough to
/// `viewport` (`size` texels across): within the view, made at a scale within 4x (M1–M4).
pub fn f64_reference_near(point: &Center, scale: Scale, viewport: &Viewport, size: usize) -> bool {
    let radius = viewport.scale.to_f64() * size as f64;
    let (dr, di) = point.delta_from(&viewport.center);
    dr.abs() < radius && di.abs() < radius && scale <= viewport.scale * 4.0 && scale >= viewport.scale / 4.0
}

/// The kind of orbit a [`ReferenceRequest`] asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A big reference (see the module docs).
    #[default]
    Big,
    /// The f64 orbit at the centre ([`Reference::compute_f64_with`]), too long for the frame
    /// thread ([`F64_SYNC_POINTS`]).
    F64,
}

/// What a reference is computed for.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceRequest {
    pub kind: ReferenceKind,
    /// The iteration (F2; [`ReferenceRequest::for_view`]: Mandelbrot).
    pub formula: Formula,
    pub center: Center,
    /// View scale per point.
    pub scale: Scale,
    /// Nucleus search radius in view points (the view's half-diagonal).
    pub radius_points: f64,
    pub max_iter: usize,
    /// Longest period the detector looks for.
    pub period_limit: usize,
    pub precision_bits: usize,
    pub bailout: f64,
    /// Look for a minibrot nucleus in the view (else: the centre).
    pub nucleus: bool,
}

impl ReferenceRequest {
    /// A request for `viewport` at texel spacing `texel`, `size` texels across.
    pub fn for_view(
        viewport: &Viewport,
        texel: Scale,
        size: usize,
        radius_points: f64,
        max_iter: usize,
        bailout: f64,
    ) -> Self {
        let bits = needed_bits(&viewport.center, texel, size, GOOD_GUARD_BITS) + EXTRA_BITS;
        Self {
            kind: ReferenceKind::Big,
            formula: Formula::Mandelbrot,
            center: viewport.center.clone(),
            scale: viewport.scale,
            radius_points,
            max_iter,
            period_limit: max_iter,
            precision_bits: bits.div_ceil(64) * 64,
            bailout,
            nucleus: true,
        }
    }

    /// The f64 orbit of `formula` at `viewport`'s centre, `max_iter` steps long.
    pub fn f64_orbit(viewport: &Viewport, formula: Formula, max_iter: usize, bailout: f64) -> Self {
        Self {
            kind: ReferenceKind::F64,
            formula,
            center: viewport.center.clone(),
            scale: viewport.scale,
            radius_points: 0.0,
            max_iter,
            period_limit: 0,
            precision_bits: 53,
            bailout,
            nucleus: false,
        }
    }

    /// Computes it on the caller's thread ([`Reference::compute_big_progressive`],
    /// [`Reference::compute_f64_cancellable`]).
    pub fn compute(
        &self,
        cancel: &AtomicBool,
        progress: &AtomicUsize,
        partial: &mut dyn FnMut(Reference),
    ) -> Option<Reference> {
        match self.kind {
            ReferenceKind::Big => Reference::compute_big_progressive(self, cancel, progress, partial),
            ReferenceKind::F64 => {
                let v = Viewport {
                    center: self.center.clone(),
                    scale: self.scale,
                    angle: 0.0,
                };
                Reference::compute_f64_cancellable(
                    &v,
                    self.formula,
                    self.max_iter,
                    self.bailout,
                    cancel,
                    Some(progress),
                )
            }
        }
    }
}

/// Computes big references on a background thread, one at a time; a new request cancels the
/// running one (it stops at its next poll, within 4096 iterations).
#[derive(Default)]
pub struct ReferenceWorker {
    job: Option<WorkerJob>,
    /// Requests started so far (diagnostics).
    pub started: usize,
    /// Run on the worker thread for each reference handed out (the renderer's GPU upload).
    pub prepare: Option<ReferencePrepare>,
}

struct WorkerJob {
    request: ReferenceRequest,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicUsize>,
    handle: JoinHandle<Option<Reference>>,
    /// The latest prefix the computation handed out and nobody took yet.
    partial: Arc<Mutex<Option<Reference>>>,
    start: Instant,
}

impl ReferenceWorker {
    /// A worker that runs `prepare` on its thread for each reference it hands out.
    pub fn with_prepare(prepare: Option<ReferencePrepare>) -> Self {
        Self {
            job: None,
            started: 0,
            prepare,
        }
    }

    /// Starts computing `request` (cancelling a running computation).
    pub fn start(&mut self, request: ReferenceRequest) {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicUsize::new(0));
        let partial = Arc::new(Mutex::new(None));
        let (c, p, r, slot) = (cancel.clone(), progress.clone(), request.clone(), partial.clone());
        let prepare = self.prepare.clone();
        let handle = std::thread::Builder::new()
            .name("almondbread-reference".into())
            .spawn(move || {
                let finish = |mut r: Reference| {
                    if let Some(f) = &prepare {
                        r.prepared = f(&r);
                    }
                    r
                };
                let r = r.compute(&c, &p, &mut |prefix| {
                    let prefix = finish(prefix);
                    if let Ok(mut s) = slot.lock() {
                        *s = Some(prefix);
                    }
                })?;
                // (A cancelled one is dropped unprepared: nobody takes it.)
                if c.load(Ordering::Relaxed) {
                    return None;
                }
                Some(finish(r))
            });
        match handle {
            Ok(handle) => {
                self.started += 1;
                self.job = Some(WorkerJob {
                    request,
                    cancel,
                    progress,
                    handle,
                    partial,
                    start: Instant::now(),
                })
            }
            Err(e) => eprintln!("almondbread: reference thread: {e}"),
        }
    }

    pub fn cancel(&mut self) {
        if let Some(j) = self.job.take() {
            j.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// The running request, if any.
    pub fn pending(&self) -> Option<&ReferenceRequest> {
        self.job.as_ref().map(|j| &j.request)
    }

    /// Seconds the running request has taken so far, and orbit iterations done.
    pub fn running_for(&self) -> Option<(f64, usize)> {
        self.job
            .as_ref()
            .map(|j| (j.start.elapsed().as_secs_f64(), j.progress.load(Ordering::Relaxed)))
    }

    /// The finished reference, once; `wait` blocks until the running one is done. Without
    /// `wait`, while the computation runs, a prefix it handed out since the last poll
    /// ([`Reference::compute_big_progressive`]; the computation stays pending).
    pub fn poll(&mut self, wait: bool) -> Option<Reference> {
        let finished = self.job.as_ref().is_some_and(|j| j.handle.is_finished());
        if !finished && !wait {
            return self
                .job
                .as_ref()
                .and_then(|j| j.partial.lock().ok().and_then(|mut s| s.take()));
        }
        let job = self.job.take()?;
        match job.handle.join() {
            Ok(r) => r,
            Err(_) => {
                eprintln!("almondbread: reference computation failed");
                None
            }
        }
    }
}

impl Drop for ReferenceWorker {
    /// A computation nobody will take stops at its next poll (the thread is detached).
    fn drop(&mut self) {
        self.cancel();
    }
}

/// The nucleus search radius as a BigFloat (diagnostics and tests).
pub fn radius_big(scale: Scale, points: f64) -> BigFloat {
    scale.big_times(points, 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_orbit_cut_at_its_limit_bounds_the_extension() {
        // The -3/4 neck at im 3.7e-4: the centre escapes at 17 019 (about pi/eps).
        let v = Viewport::new(-0.749_999_947_197_051_2, 0.000_369_184_853_510_651_04, 2e-12);
        let cut = Reference::compute_f64(&v, 4_533, 65536.0);
        assert!(!cut.escaped);
        assert_eq!(cut.exact_reach(), 4_533);
        let whole = Reference::compute_f64(&v, 72_528, 65536.0);
        assert!(whole.escaped);
        assert_eq!(whole.points.len() - 1, 17_018);
        assert_eq!(whole.exact_reach(), usize::MAX);
        // The prefix is the same orbit: extension passes may restart on the longer one.
        assert_eq!(cut.points[..], whole.points[..cut.points.len()]);
    }

    #[test]
    fn big_reference_finds_a_nucleus_and_serves_deeper_views() {
        // Near the period-3 minibrot at -1.7549, at 1e-20 per point.
        let v = Viewport::with_center(
            Center::parse(
                "-1.75487766624669276004950889635852869189460661777279",
                "0.0000000000000000000001",
            )
            .unwrap(),
            Scale::from_f64(1e-20),
        );
        let texel = v.scale;
        let req = ReferenceRequest::for_view(&v, texel, 1000, 700.0, 5000, 65536.0);
        let r = Reference::compute_big(&req, &AtomicBool::new(false), &AtomicUsize::new(0)).unwrap();
        assert_eq!(r.period, Some(3));
        assert_eq!(r.points.len(), 4);
        assert_eq!(r.points[3], [0.0, 0.0]);
        assert_eq!(r.fit(&v, texel, 1000, 5000), Fit::Good);
        // 30 decades deeper the precision runs out.
        let mut deeper = v.clone();
        deeper.zoom_by(1e30, 0.0, 0.0);
        assert_eq!(r.fit(&deeper, deeper.scale, 1000, 5000), Fit::No);
        // Shallow views keep the f64 path.
        let shallow = Viewport::new(-1.75, 0.0, 1e-6);
        assert_eq!(r.fit(&shallow, shallow.scale, 1000, 5000), Fit::No);
    }

    /// A long centre orbit is handed out as prefixes (1/16, then 1/2 of the request) that
    /// equal the start of the final orbit; the half-length one serves the view's limit.
    #[test]
    fn progressive_prefixes_match_the_final_orbit() {
        let v = Viewport::with_center(Center::new(-0.5, 0.1), Scale::from_f64(1e-30));
        let limit = 40_000;
        let mut req = ReferenceRequest::for_view(&v, v.scale, 800, 500.0, 2 * limit, 65536.0);
        req.nucleus = false;
        let mut parts = Vec::new();
        let r = Reference::compute_big_progressive(&req, &AtomicBool::new(false), &AtomicUsize::new(0), &mut |p| {
            parts.push(p)
        })
        .unwrap();
        assert_eq!(r.points.len(), 2 * limit + 1);
        let lens: Vec<usize> = parts.iter().map(|p| p.points.len()).collect();
        assert_eq!(lens, vec![2 * limit / 16 + 1, limit + 1]);
        for p in &parts {
            let n = p.points.len();
            assert_eq!(p.points[..], r.points[..n]);
            assert_eq!(p.extended[..], r.extended[..n]);
        }
        assert_eq!(parts[0].fit(&v, v.scale, 800, limit), Fit::Stale);
        assert_eq!(parts[1].fit(&v, v.scale, 800, limit), Fit::Good);
        // The first prefix ends inside the limit without escaping: no exact rebase point.
        assert!(parts[0].truncated_before(limit) && !parts[1].truncated_before(limit));
        assert!(r.truncated_before(usize::MAX) && !r.truncated_before(2 * limit));
        let whole = Reference::compute_big(&req, &AtomicBool::new(false), &AtomicUsize::new(0)).unwrap();
        assert!(whole.points == r.points && whole.extended == r.extended);
    }

    /// C1 (2026-10 review): a limit of 1e9 reserves nothing like 1e9 points when the orbit
    /// escapes early (the f64 orbit reserved 8 GB, a big one 48 GB for twice the limit).
    #[test]
    fn long_requests_grow_instead_of_reserving() {
        let out = Viewport::new(0.3, 0.6, 1e-6);
        let r = Reference::compute_f64(&out, 1_000_000_000, 65536.0);
        assert!(r.escaped && r.points.len() < 1000);
        assert!(r.points.capacity() <= PREALLOC_POINTS + 1, "{}", r.points.capacity());
        let v = Viewport::with_center(Center::new(0.3, 0.6), Scale::from_f64(1e-30));
        let mut req = ReferenceRequest::for_view(&v, v.scale, 800, 500.0, 2_000_000_000, 65536.0);
        req.nucleus = false;
        let big = Reference::compute_big(&req, &AtomicBool::new(false), &AtomicUsize::new(0)).unwrap();
        assert!(big.escaped && big.points.len() < 1000);
        assert!(
            big.points.capacity() <= GROW_FROM_POINTS + 1,
            "{}",
            big.points.capacity()
        );
        assert!(big.extended.capacity() <= GROW_FROM_POINTS + 1);
    }

    /// A centre orbit longer than GROW_FROM_POINTS, computed in doubling stages, equals the
    /// prefix of a longer one (resuming is exact); prefixes are handed out as before.
    #[test]
    fn staged_growth_is_exact() {
        let v = Viewport::with_center(Center::new(-0.1, 0.1), Scale::from_f64(1e-20));
        let mut req = ReferenceRequest::for_view(&v, v.scale, 100, 50.0, 3 * GROW_FROM_POINTS, 65536.0);
        req.nucleus = false;
        req.precision_bits = 128;
        let mut parts = Vec::new();
        let r = Reference::compute_big_progressive(&req, &AtomicBool::new(false), &AtomicUsize::new(0), &mut |p| {
            parts.push(p.points.len())
        })
        .unwrap();
        assert_eq!(r.points.len(), 3 * GROW_FROM_POINTS + 1);
        assert_eq!(parts, vec![3 * GROW_FROM_POINTS / 16 + 1, 3 * GROW_FROM_POINTS / 2 + 1]);
        let mut short = req.clone();
        short.max_iter = GROW_FROM_POINTS / 2;
        let s = Reference::compute_big(&short, &AtomicBool::new(false), &AtomicUsize::new(0)).unwrap();
        assert_eq!(s.points[..], r.points[..s.points.len()]);
        assert_eq!(s.extended[..], r.extended[..s.points.len()]);
    }

    /// The f64 orbit on the worker (C1): the same points as on the caller's thread, and a
    /// cancelled one stops.
    #[test]
    fn f64_orbit_on_the_worker() {
        let v = Viewport::new(-0.1, 0.1, 1e-6);
        let mut w = ReferenceWorker::default();
        w.start(ReferenceRequest::f64_orbit(&v, Formula::Mandelbrot, 300_000, 65536.0));
        let r = w.poll(true).expect("reference");
        let here = Reference::compute_f64(&v, 300_000, 65536.0);
        assert!(!r.big && r.points == here.points && r.max_iter == 300_000 && !r.escaped);
        assert_eq!(r.fit(&v, v.scale, 800, 300_000), Fit::Good);
        let stop = AtomicBool::new(true);
        assert!(Reference::compute_f64_cancellable(&v, Formula::Mandelbrot, 1 << 30, 65536.0, &stop, None).is_none());
        // Dropping the worker cancels what it runs.
        let mut w = ReferenceWorker::default();
        w.start(ReferenceRequest::f64_orbit(&v, Formula::Mandelbrot, 1 << 31, 65536.0));
        let cancel = w.job.as_ref().unwrap().cancel.clone();
        drop(w);
        assert!(cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn worker_cancels_and_delivers() {
        let v = Viewport::with_center(
            Center::new(-0.743643887037151, 0.13182590420533),
            Scale::from_f64(1e-15),
        );
        let mut w = ReferenceWorker::default();
        let mut req = ReferenceRequest::for_view(&v, v.scale, 800, 500.0, 200_000, 65536.0);
        req.nucleus = false;
        w.start(req.clone());
        w.start(req);
        let r = w.poll(true).expect("reference");
        assert!(r.big && r.points.len() > 1000);
        assert!(w.poll(false).is_none());
    }
}
