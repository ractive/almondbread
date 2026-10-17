//! Target lock: while the autopilot runs, it aims at a boundary point that stays
//! interesting at every depth (kb: decisions/controls "Autopilot", "Target lock").
//!
//! **Why.** Zooming continuously towards almost any point ends in smooth exterior or interior:
//! the boundary has no thickness, so a generic point is at a finite distance from it and the
//! view eventually shows only what is on one side. The old *assist* leaned the heading towards
//! the survey's detail centroid; it lags (the centroid is measured on the last frame and moves
//! with every frame), so it loses the thread and ends in the same voids. Only points *on* the
//! boundary keep the view busy: Misiurewicz points (spiral centres, branch points; the view
//! around them is asymptotically self-similar) and minibrots (a dive passes their embedded
//! Julia stages and ends with the minibrot and its decorations).
//!
//! **How.** The journey's finder ([`crate::journey`]: ball period detection and Newton for
//! nuclei, the orbit's closest return and Newton for Misiurewicz points, all with extra bits
//! at depth) runs on a thread over the current view; the candidates' framings are probed on
//! the CPU and scored like journey targets (structure, not interior, not noise), the closest
//! to the centre preferred. A target is aimed at until its framing's span (`end_span`, where
//! the journey would land); a search *ahead* (the view centred on the target at its end span)
//! starts as soon as a target is locked, so the next target is ready before the current one's
//! interesting depth is passed. Embedded Julia stages and Misiurewicz points are aimed at
//! directly; a minibrot is aimed at its nucleus only down to its framing (it fills 1/6 … 1/18
//! of the view), after which the next search finds its decorations.
//!
//! Everything here is data plus a thread: the navigator calls [`Lock::update`] once per frame
//! with the view (a few big-float subtractions); searching never runs on the render thread
//! unless [`Lock::synchronous`] is set (tests and reproducible flights).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use almondbread_math::{NewtonOptions, find_misiurewicz, find_nucleus, minibrot_size_complex};

use crate::journey::{self, ProbeStats, Rng, TargetKind};
use crate::viewport::{Center, Scale, Viewport};

/// Candidates further than this from the searched view's centre (in spans across the shorter
/// side) are not taken: in or just around the view.
pub const LOCK_REACH: f64 = 0.45;
/// Score per span of distance from the searched view's centre (closer: less steering).
pub const LOCK_DISTANCE_COST: f64 = 1.2;
/// Candidates probed per search (best first by distance and kind).
pub const LOCK_PROBES: usize = 6;
/// Switch to the next target this many e-folds before the current one's end span.
pub const SWITCH_LEAD: f64 = 0.7;
/// A search that found nothing is repeated after this long (s), or once the view's span
/// changed by more than [`RETRY_EFOLDS`].
pub const RETRY_AFTER: f64 = 0.6;
pub const RETRY_EFOLDS: f64 = 0.5;
/// A target further than this from the view centre (view's shorter sides) is lost.
pub const LOST: f64 = 1.5;
/// The aim point is at most this many steering radii from the centre.
pub const AIM_MAX: f64 = 1.0;
/// When the target changes, the aim point glides from where the heading pointed to the new
/// target over this long (s, smoothstep): no jumps.
pub const AIM_BLEND: f64 = 0.8;
/// Pre-score bonus of the next stage of the minibrot the lock is on the way to (its next
/// approach, Julia stage or framing): a dive follows a minibrot through instead of switching
/// to another one deeper down every leg.
pub const FOLLOW_BONUS: f64 = 0.6;
/// Pre-score of diving on into the same Misiurewicz point (refined at the new depth's
/// precision): always possible, so it is the fallback; other targets nearby win when they
/// probe well (variety).
pub const CONTINUE_SCORE: f64 = -0.25;
/// Pre-score of a boundary texel of the probe (the fallback when the maths finds nothing).
pub const DETAIL_SCORE: f64 = -0.5;
/// Targets a rescue (or a lost lock) dropped are remembered, the last `TABU_LEN`, and none
/// within `TABU_REACH` of their end span is taken again (else a minibrot whose framing
/// probes well but whose next stage the CPU probes cannot resolve is dived into, backed out
/// of and dived into again).
pub const TABU_LEN: usize = 4;
pub const TABU_REACH: f64 = 2.0;
/// Seconds a search may spend raising the view probe's limit ([`journey::probe_escalating`]).
pub const ESCALATE_BUDGET: f64 = 0.5;
/// Pointer wish (kb: decisions/controls "Pointer wish"): pre-score per unit of agreement (the
/// cosine of the angle between a candidate's direction from the searched view's centre and
/// the wish, faded to 0 within [`super::WISH_NEAR`] spans): a candidate in the wished
/// direction gains this much, one opposite loses it (more than any kind or follow bonus, so
/// the dive goes the wished way whenever something there probes well).
pub const WISH_WEIGHT: f64 = 1.5;
/// A re-target on a changed wish takes the new search's best target only if it agrees at
/// least this much (else the current target stays).
pub const WISH_ACCEPT: f64 = 0.25;
/// A found target agreeing more than this with the wish lies in the wished half (cosine > 0
/// and off the centre): the best of those is taken first.
pub const WISH_SECTOR: f64 = 0.1;
/// A target that already agrees this much with a new wish is kept (no re-target).
pub const WISH_KEEP: f64 = 0.5;

/// Seed of the searches (each search advances it: reproducible from the same views).
const SEED: u64 = 0x5eed_10c4;

/// What the autopilot aims at.
#[derive(Clone, Debug, PartialEq)]
pub struct LockTarget {
    pub kind: TargetKind,
    pub center: Center,
    /// Span across the view's shorter side at which the target is reached (its framing): the
    /// next target takes over [`SWITCH_LEAD`] e-folds before.
    pub end_span: Scale,
    pub label: String,
    pub score: f64,
    /// Minibrot or cycle period, Misiurewicz preperiod.
    pub period: Option<usize>,
    pub preperiod: Option<usize>,
}

/// What the search ahead continues: the current target's maths.
#[derive(Clone, Debug, PartialEq)]
pub struct Follow {
    pub kind: TargetKind,
    pub center: Center,
    pub period: usize,
    pub preperiod: Option<usize>,
}

impl LockTarget {
    /// What a search ahead continues: the minibrot an approach or a Julia stage leads to, or
    /// a Misiurewicz point.
    pub fn follows(&self) -> Option<Follow> {
        let period = self.period?;
        match self.kind {
            TargetKind::Approach | TargetKind::Julia | TargetKind::Spiral | TargetKind::Dendrite => Some(Follow {
                kind: self.kind,
                center: self.center.clone(),
                period,
                preperiod: self.preperiod,
            }),
            _ => None,
        }
    }
}

/// Whether a probed framing is something to aim at: not (mostly) interior, not uniform. Looser
/// than the journey's landing test ([`ProbeStats::reject`]): sparse filaments in a smooth
/// gradient (dendrite country) are dull to land on but fine to dive through.
pub fn aimable(st: &ProbeStats) -> bool {
    st.interior <= 0.75 && st.centre_interior <= 0.9 && st.detail >= super::DETAIL_LO && st.centre_detail >= 1.0
}

/// What a search found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchResult {
    /// Best first.
    pub targets: Vec<LockTarget>,
    /// The searched view's probe is (nearly) uniform: interior or smooth exterior.
    pub view_uniform: bool,
    /// Candidates found before probing, probes run, wall time (s).
    pub candidates: usize,
    pub probed: usize,
    pub seconds: f64,
}

/// Finds lock targets in and around the view (`center`, `span` across its shorter side);
/// `follow` is the current target's maths, whose continuation is added (the minibrot's next
/// stage, preferred; the same Misiurewicz point deeper, the fallback). `wish` is the pointer
/// wish as a unit direction in the complex plane (re, im): candidates that way gain up to
/// [`WISH_WEIGHT`]. None only when cancelled. Deterministic in its inputs.
pub fn search(
    center: &Center,
    span: Scale,
    seed: u64,
    follow: Option<&Follow>,
    wish: Option<(f64, f64)>,
    cancel: &AtomicBool,
) -> Option<SearchResult> {
    let t0 = std::time::Instant::now();
    let mut rng = Rng::new(seed);
    // The view's limit escalates where the CPU limit leaves it unresolved (deep high-period
    // minibrots); candidate probes start from the same limit, they are deeper.
    let (view, view_iterations) = journey::probe_escalating(center, span, ESCALATE_BUDGET);
    let stats = ProbeStats::of(&view);
    // Fewer band crossings than the survey's "uniform" threshold (a 40-texel probe misses the
    // finest bands, so this is the stricter test).
    let view_uniform = stats.detail < super::DETAIL_LO;
    let mut candidates = journey::find_candidates_in(center, span, &view, 0.0, &mut rng, cancel)?;
    if let Some(f) = follow
        && matches!(f.kind, TargetKind::Spiral | TargetKind::Dendrite)
        && let Some(k) = f.preperiod
    {
        // The same Misiurewicz point a few e-folds deeper, at that depth's precision.
        let t_span = span * (-rng.range(4.0, 7.0)).exp();
        let bits = journey::bits_for(t_span).max(f.center.precision_bits());
        if let Some(m) = find_misiurewicz(
            &f.center.big().with_precision(bits),
            k,
            f.period,
            &NewtonOptions {
                precision_bits: Some(bits),
                max_steps: 40,
                max_distance: Some(span.big_times(0.05, 64)),
            },
        ) {
            let kind = if f.kind == TargetKind::Spiral {
                "spiral"
            } else {
                "branch point"
            };
            candidates.push(journey::Candidate {
                kind: f.kind,
                center: Center::from_big(m.point),
                span: t_span,
                period: Some(f.period),
                preperiod: Some(k),
                label: format!("{kind} M({k},{})", f.period),
                prescore: CONTINUE_SCORE,
                extra: 0,
            });
        }
    }
    if let Some(f) = follow
        && matches!(f.kind, TargetKind::Approach | TargetKind::Julia)
    {
        let (c, period) = (&f.center, &f.period);
        // The minibrot's nucleus again at this depth's precision, then its next stages.
        let bits = journey::bits_for(span).max(c.precision_bits()) + 128;
        let mut is_cancelled = || cancel.load(Ordering::Relaxed);
        if let Some(n) = find_nucleus(
            &c.big().with_precision(bits),
            *period,
            &NewtonOptions {
                precision_bits: Some(bits),
                max_steps: 64,
                max_distance: Some(span.big_times(0.5, 64)),
            },
            &mut is_cancelled,
        ) {
            let nucleus = Center::from_big(n.nucleus);
            if let Some(sigma) = journey::scale_of(minibrot_size_complex(nucleus.big(), *period).magnitude()) {
                let mut next = Vec::new();
                journey::minibrot_candidates(&mut next, span, &nucleus, sigma, *period, &mut rng, cancel);
                for mut c in next {
                    c.prescore = FOLLOW_BONUS;
                    candidates.push(c);
                }
            }
        }
    }
    let found = candidates.len();
    let trace = std::env::var_os("ALMONDBREAD_LOCK_TRACE").is_some();
    if trace {
        println!(
            "lock search at 1e{:.1} (view detail {:.1}, interior {:.2}, probe limit {view_iterations}): {found} candidates",
            3.0f64.log10() - span.log10(),
            stats.detail,
            stats.interior
        );
        for c in &candidates {
            println!(
                "    {:<36} at {:.2} spans, gain {:.1}",
                c.label,
                c.center.distance_in(center, span),
                span.ratio(c.span).ln()
            );
        }
    }
    candidates.retain(|c| c.center.distance_in(center, span) <= LOCK_REACH && span.ratio(c.span).ln() > 1.0);
    for c in candidates.iter_mut() {
        let d = c.center.distance_in(center, span);
        // Points on the boundary first: embedded Julia stages and Misiurewicz points keep
        // their structure at every depth; a minibrot's nucleus only down to its framing. A
        // boundary texel of the probe ("detail") is the fallback: exact only to a probe texel,
        // it is aimed at for one short leg whose end view the probe has checked.
        let kind = match c.kind {
            TargetKind::Spiral | TargetKind::Julia => 0.15,
            TargetKind::Dendrite => 0.1,
            TargetKind::Detail => DETAIL_SCORE,
            _ => 0.0,
        };
        let wished = wish.map_or(0.0, |w| WISH_WEIGHT * agreement(c.center.delta_in(center, span), w));
        c.prescore += kind - LOCK_DISTANCE_COST * d + wished + rng.range(0.0, 0.1);
    }
    candidates.sort_by(|a, b| b.prescore.total_cmp(&a.prescore));
    let mut targets = Vec::new();
    let mut probed = 0;
    for c in candidates.into_iter().take(LOCK_PROBES) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let base = (journey::probe_iterations(c.span) + c.extra).min(journey::PROBE_ITER_CAP * 2);
        let f = journey::probe_limit(&c.center, c.span, base.max(view_iterations), journey::PROBE);
        probed += 1;
        let st = ProbeStats::of(&f);
        if trace {
            println!(
                "    probe {:<36} detail {:.1} centre {:.1} interior {:.2} {}",
                c.label,
                st.detail,
                st.centre_detail,
                st.interior,
                if aimable(&st) { "ok" } else { "rejected" }
            );
        }
        if !aimable(&st) {
            continue;
        }
        targets.push(LockTarget {
            kind: c.kind,
            score: st.quality(c.kind) + c.prescore,
            center: c.center,
            end_span: c.span,
            label: c.label,
            period: c.period,
            preperiod: c.preperiod,
        });
    }
    targets.sort_by(|a, b| b.score.total_cmp(&a.score));
    Some(SearchResult {
        targets,
        view_uniform,
        candidates: found,
        probed,
        seconds: t0.elapsed().as_secs_f64(),
    })
}

/// Agreement of an offset `d` (in spans, complex plane) with the wish direction `w` (unit):
/// the cosine of the angle, faded to 0 within [`super::WISH_NEAR`] spans (straight ahead).
pub fn agreement(d: (f64, f64), w: (f64, f64)) -> f64 {
    super::Wish { dir: w, epoch: 0 }.agreement(d, super::WISH_NEAR)
}

/// The wish's direction (view points) as a unit direction in the complex plane of `view`
/// (the view may be rotated).
pub fn wish_in_plane(view: &Viewport, dir: (f64, f64)) -> Option<(f64, f64)> {
    let (x, y) = view.point_at_offset(dir.0, dir.1).delta_in(&view.center, view.scale);
    let r = x.hypot(y);
    (r.is_finite() && r > 0.0).then(|| (x / r, y / r))
}

/// Why a search runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    /// From the current view: nothing is locked (start, lost, rescue).
    Pickup,
    /// From the current target's end view: the next target.
    Ahead,
    /// From the current view after the pointer wish changed: a target that way replaces the
    /// current one (which is kept until then, and if none agrees).
    Retarget,
}

/// Where a search looked and with which wish (its result is judged against them).
#[derive(Clone, Debug)]
struct Origin {
    center: Center,
    span: Scale,
    /// The wish as a complex-plane direction, and its epoch (0: none).
    wish: Option<(f64, f64)>,
    epoch: u32,
}

#[derive(Debug, Default)]
struct Slot {
    cancel: AtomicBool,
    result: Mutex<Option<Option<SearchResult>>>,
}

/// A search on its thread.
#[derive(Clone, Debug)]
struct Pending {
    slot: Arc<Slot>,
    purpose: Purpose,
    origin: Origin,
}

/// The lock's state (owned by the [`super::Autopilot`]).
#[derive(Clone, Debug)]
pub struct Lock {
    enabled: bool,
    /// Search on the calling thread (tests, reproducible flights).
    pub synchronous: bool,
    target: Option<LockTarget>,
    next: Option<LockTarget>,
    pending: Option<Pending>,
    /// The last pickup search found nothing: when, at which span, and whether its view had
    /// no boundary at all.
    failed: Option<(f64, Scale, bool)>,
    /// The search ahead found nothing (a pickup from the view follows at the end span).
    ahead_failed: bool,
    searches: u64,
    /// Targets taken (HUD, measurements).
    pub switches: u32,
    /// The last result's wall time (s) and candidate count (measurements).
    pub last_search: (f64, usize),
    /// The target aimed at in the last frame, and where the aim glides from since it changed
    /// (the point the heading pointed at, and when).
    aimed: Option<Center>,
    glide: Option<(Center, f64)>,
    /// Targets a rescue dropped (centre, end span): not taken again nearby.
    tabu: Vec<(Center, Scale)>,
    /// The wish epochs (0: none) the target and the next one were chosen with, the last one a
    /// re-target was tried for, and the current one.
    target_epoch: u32,
    next_epoch: u32,
    retarget_epoch: u32,
    epoch: u32,
    /// Re-targets taken (measurements).
    pub retargets: u32,
}

impl Default for Lock {
    fn default() -> Self {
        Self {
            enabled: true,
            synchronous: false,
            target: None,
            next: None,
            pending: None,
            failed: None,
            ahead_failed: false,
            searches: 0,
            switches: 0,
            last_search: (0.0, 0),
            aimed: None,
            glide: None,
            tabu: Vec::new(),
            target_epoch: 0,
            next_epoch: 0,
            retarget_epoch: 0,
            epoch: 0,
            retargets: 0,
        }
    }
}

/// A pickup search's view snapped to a grid (span to 1/8 e-fold, up; centre to 1/128 of
/// that span): views a frame apart search the same view, so a flight finds the same targets
/// at 30, 60 and 120 Hz.
pub fn snapped(center: &Center, span: Scale) -> (Center, Scale) {
    let s = Scale::from_log2((span.ln() * 8.0).ceil() / 8.0 / std::f64::consts::LN_2);
    let g = s / 128.0;
    let bits = ((-g.log2()).ceil().max(0.0) as usize).saturating_sub(40).max(64);
    let anchor = Center::from_big(center.big().with_precision(bits)).with_min_precision(center.precision_bits());
    let (dx, dy) = center.delta_in(&anchor, g);
    if !(dx.is_finite() && dy.is_finite() && dx.abs() < 1e15 && dy.abs() < 1e15) {
        return (center.clone(), s);
    }
    (anchor.offset_scaled(dx.round(), dy.round(), g), s)
}

/// The span across the view's shorter side.
pub fn view_span(view: &Viewport, min_side: f64) -> Scale {
    view.scale * min_side.max(1.0)
}

impl Lock {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on {
            self.release();
        }
    }

    pub fn target(&self) -> Option<&LockTarget> {
        self.target.as_ref()
    }

    pub fn next_target(&self) -> Option<&LockTarget> {
        self.next.as_ref()
    }

    pub fn is_searching(&self) -> bool {
        self.pending.is_some()
    }

    /// The last pickup search from (about) this view found no boundary at all.
    pub fn view_is_void(&self) -> bool {
        self.target.is_none() && self.failed.is_some_and(|f| f.2)
    }

    /// Drops the target and any search (the pointer took over, the autopilot stopped).
    pub fn release(&mut self) {
        self.cancel();
        self.target = None;
        self.next = None;
        self.failed = None;
        self.ahead_failed = false;
        self.aimed = None;
        self.glide = None;
    }

    /// A target set from outside (the autopilot's survey pick after a rescue).
    pub fn set_target(&mut self, t: LockTarget) {
        if self.pending.as_ref().is_some_and(|p| p.purpose == Purpose::Pickup) {
            self.cancel();
        }
        self.target = Some(t);
        self.target_epoch = self.epoch;
        self.next = None;
        self.failed = None;
        self.ahead_failed = false;
        self.switches += 1;
    }

    /// Drops the target but keeps searching from the view (rescue).
    pub fn drop_target(&mut self) {
        if self.pending.as_ref().is_some_and(|p| p.purpose == Purpose::Ahead) {
            self.cancel();
        }
        if let Some(t) = self.target.take() {
            self.tabu.push((t.center, t.end_span));
            if self.tabu.len() > TABU_LEN {
                self.tabu.remove(0);
            }
        }
        self.next = None;
        self.ahead_failed = false;
    }

    fn cancel(&mut self) {
        if let Some(p) = self.pending.take() {
            p.slot.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn request(
        &mut self,
        center: &Center,
        span: Scale,
        follow: Option<Follow>,
        purpose: Purpose,
        wish: Option<(f64, f64)>,
        now: f64,
    ) {
        self.cancel();
        self.searches += 1;
        let seed = SEED ^ self.searches.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let origin = Origin {
            center: center.clone(),
            span,
            wish,
            epoch: if wish.is_some() { self.epoch } else { 0 },
        };
        if self.synchronous {
            let never = AtomicBool::new(false);
            let r = search(center, span, seed, follow.as_ref(), wish, &never);
            self.take_result(r, purpose, &origin, now);
            return;
        }
        let slot = Arc::new(Slot::default());
        let s = slot.clone();
        let c = center.clone();
        let spawned = std::thread::Builder::new()
            .name("autopilot-lock".into())
            .spawn(move || {
                let r = search(&c, span, seed, follow.as_ref(), wish, &s.cancel);
                if let Ok(mut out) = s.result.lock() {
                    *out = Some(r);
                }
            });
        if spawned.is_ok() {
            self.pending = Some(Pending { slot, purpose, origin });
        }
    }

    /// A search from (the snapped) current view.
    fn pickup(&mut self, view: &Viewport, span: Scale, purpose: Purpose, wish: Option<(f64, f64)>, now: f64) {
        let (c, s) = snapped(&view.center, span);
        self.request(&c, s, None, purpose, wish, now);
    }

    fn take_result(&mut self, r: Option<SearchResult>, purpose: Purpose, origin: &Origin, now: f64) {
        let Some(r) = r else { return };
        let span = origin.span;
        self.last_search = (r.seconds, r.candidates);
        // Not a target a rescue just gave up on (its view led into a void).
        let mut ok: Vec<LockTarget> = r
            .targets
            .into_iter()
            .filter(|t| {
                !self
                    .tabu
                    .iter()
                    .any(|(c, s)| t.center.distance_in(c, *s) < TABU_REACH && t.end_span.ratio(*s).ln().abs() < 3.0)
            })
            .collect();
        // With a wish, the best target in the wished half wins over a better one elsewhere
        // (the pre-score only decides which candidates are probed); none there: the best.
        let sector = origin.wish.and_then(|w| {
            ok.iter()
                .position(|t| agreement(t.center.delta_in(&origin.center, origin.span), w) > WISH_SECTOR)
        });
        let best = match sector {
            Some(i) => Some(ok.swap_remove(i)),
            None => ok.into_iter().next(),
        };
        match purpose {
            Purpose::Pickup => match best {
                Some(t) => {
                    self.target = Some(t);
                    self.target_epoch = origin.epoch;
                    self.next = None;
                    self.failed = None;
                    self.ahead_failed = false;
                    self.switches += 1;
                }
                None => self.failed = Some((now, span, r.view_uniform)),
            },
            Purpose::Ahead => match best {
                Some(t) => {
                    self.next = Some(t);
                    self.next_epoch = origin.epoch;
                }
                None => self.ahead_failed = true,
            },
            Purpose::Retarget => {
                // Only a target the wished way replaces the current one.
                let agrees = |t: &LockTarget| {
                    origin
                        .wish
                        .is_some_and(|w| agreement(t.center.delta_in(&origin.center, span), w) >= WISH_ACCEPT)
                };
                if let Some(t) = best.filter(agrees) {
                    self.target = Some(t);
                    self.target_epoch = origin.epoch;
                    self.next = None;
                    self.ahead_failed = false;
                    self.switches += 1;
                    self.retargets += 1;
                }
            }
        }
    }

    fn poll(&mut self, now: f64) {
        let Some(p) = &self.pending else { return };
        let done = p.slot.result.lock().ok().and_then(|mut r| r.take());
        if let Some(r) = done {
            let (purpose, origin) = (p.purpose, p.origin.clone());
            self.pending = None;
            self.take_result(r, purpose, &origin, now);
        }
    }

    /// One frame: `free` says the user does not steer (no pan, drag or held key). Returns the
    /// aim point (view points from the centre) while a target is locked: the target, or on the
    /// way to it from where `heading` pointed when it changed. `radius` is the steering radius,
    /// `min_side` the view's shorter side in points. `wish` is the settled pointer wish: the
    /// searches prefer candidates that way, and a new wish (its epoch changed) re-targets once
    /// unless the target already lies that way.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        view: &Viewport,
        min_side: f64,
        radius: f64,
        free: bool,
        heading: (f64, f64),
        wish: Option<&super::Wish>,
        now: f64,
    ) -> Option<(f64, f64)> {
        if !self.enabled || !free {
            self.release();
            return None;
        }
        let span = view_span(view, min_side);
        self.poll(now);
        if let Some(t) = &self.target {
            let (x, y) = view.offset_of(&t.center);
            if !(x.is_finite() && y.is_finite()) || x.hypot(y) > LOST * min_side {
                self.drop_target();
            }
        }
        let epoch = wish.map_or(0, |w| w.epoch);
        let wish_c = wish.and_then(|w| wish_in_plane(view, w.dir));
        if epoch != self.epoch {
            // The wish changed: a search still running under the old one is stale.
            self.epoch = epoch;
            if self.pending.as_ref().is_some_and(|p| p.origin.epoch != epoch) {
                self.cancel();
            }
            if self.next_epoch != epoch {
                self.next = None;
                self.ahead_failed = false;
            }
        }
        if let Some(w) = wish
            && let Some(t) = &self.target
            && self.target_epoch != w.epoch
            && self.retarget_epoch != w.epoch
        {
            let p = view.offset_of(&t.center);
            if w.agreement(p, super::WISH_NEAR * min_side) >= WISH_KEEP {
                // Already that way: keep it (its search ahead runs with the new wish).
                self.target_epoch = w.epoch;
            } else if self.pending.is_none() {
                self.retarget_epoch = w.epoch;
                self.pickup(view, span, Purpose::Retarget, wish_c, now);
            }
        }
        let retry = self
            .failed
            .is_none_or(|(t, s, _)| now - t >= RETRY_AFTER || s.ratio(span).ln().abs() >= RETRY_EFOLDS);
        match &self.target {
            None => {
                if self.pending.is_none() && retry {
                    self.pickup(view, span, Purpose::Pickup, wish_c, now);
                }
            }
            Some(t) => {
                let left = span.ratio(t.end_span).ln();
                if left <= SWITCH_LEAD
                    && let Some(n) = self.next.take()
                {
                    self.target = Some(n);
                    self.target_epoch = self.next_epoch;
                    self.switches += 1;
                    self.ahead_failed = false;
                } else if self.next.is_none() && self.pending.is_none() {
                    if !self.ahead_failed {
                        let (c, s, f) = (t.center.clone(), t.end_span, t.follows());
                        self.request(&c, s, f, Purpose::Ahead, wish_c, now);
                    } else if left <= 0.0 && retry {
                        // Nothing ahead of this target: a fresh look from where the view is.
                        self.pickup(view, span, Purpose::Pickup, wish_c, now);
                    }
                }
            }
        }
        let Some(t) = self.target.as_ref() else {
            self.aimed = None;
            self.glide = None;
            return None;
        };
        if self.aimed.as_ref() != Some(&t.center) {
            self.glide = Some((view.point_at_offset(heading.0, heading.1), now));
            self.aimed = Some(t.center.clone());
        }
        let (mut x, mut y) = view.offset_of(&t.center);
        if let Some((from, t0)) = &self.glide {
            let s = super::smoothstep((now - t0) / AIM_BLEND);
            if s >= 1.0 {
                self.glide = None;
            } else {
                let (fx, fy) = view.offset_of(from);
                x = fx + (x - fx) * s;
                y = fy + (y - fy) * s;
            }
        }
        let r = x.hypot(y);
        let max = AIM_MAX * radius;
        Some(if r > max { (x * max / r, y * max / r) } else { (x, y) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Views with boundary in several directions.
    fn views() -> Vec<(Center, Scale)> {
        vec![
            (
                Center::new(-0.7436438870371587, 0.131825904205312),
                Scale::from_f64(2e-5),
            ),
            (Center::new(-0.745, 0.11), Scale::from_f64(0.01)),
            (Center::new(0.2865, 0.0115), Scale::from_f64(1e-3)),
            (Center::new(-0.1011, 0.9563), Scale::from_f64(1e-3)),
            (Center::new(-1.7590, 0.0187), Scale::from_f64(5e-4)),
        ]
    }

    /// The target a fresh synchronous lock picks up from `view` (600 points on the shorter
    /// side) with the wish `dir` (view points), and its offset on screen.
    fn pick(view: &Viewport, dir: Option<(f64, f64)>) -> Option<(LockTarget, (f64, f64))> {
        let mut lock = Lock {
            synchronous: true,
            ..Lock::default()
        };
        let wish = dir.map(|dir| crate::autopilot::Wish { dir, epoch: 1 });
        lock.update(view, 600.0, 300.0, true, (0.0, 0.0), wish.as_ref(), 0.0);
        let t = lock.target()?.clone();
        let p = view.offset_of(&t.center);
        Some((t, p))
    }

    /// Kb: decisions/controls "Pointer wish": with a wish the picked target lies in the wished
    /// half (several real views with boundary on every side, one rotated); without one the
    /// choice is the search's best, as before the wish existed.
    #[test]
    fn wish_picks_targets_in_the_wished_half_and_no_wish_is_unchanged() {
        let never = AtomicBool::new(false);
        let mut in_half = 0;
        for (k, (c, span)) in views().into_iter().enumerate() {
            let mut view = Viewport::new(0.0, 0.0, 1.0);
            view.center = c;
            view.scale = span / 600.0;
            if k == 2 {
                view.angle = 1.0;
            }
            let (sc, ss) = snapped(&view.center, view_span(&view, 600.0));
            let seed = SEED ^ 1u64.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            for dir in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let (t, (x, y)) = pick(&view, Some(dir)).expect("a target");
                let along = x * dir.0 + y * dir.1;
                // Some views have nothing aimable on one side (only the fallback is then
                // allowed: the best target elsewhere).
                let wc = wish_in_plane(&view, dir);
                let r = search(&sc, ss, seed, None, wc, &never).expect("search");
                let any = r.targets.iter().any(|t| {
                    let (x, y) = view.offset_of(&t.center);
                    x * dir.0 + y * dir.1 > 0.0
                });
                if any {
                    in_half += 1;
                    assert!(along > 0.0, "view {k}, wish {dir:?}: {} at ({x:.0}, {y:.0})", t.label);
                }
            }
            // No wish: the first search's best (the seed of a lock's first search).
            let best = search(&sc, ss, seed, None, None, &never).and_then(|r| r.targets.into_iter().next());
            assert_eq!(pick(&view, None).map(|p| p.0), best, "view {k}");
        }
        assert!(
            in_half >= 15,
            "only {in_half} of 20 cases had a target in the wished half"
        );
    }
}
