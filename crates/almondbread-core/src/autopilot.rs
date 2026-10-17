//! Autopilot: a continuous zoom you steer with the pointer like a joystick (kb:
//! decisions/controls, "Autopilot"). Owned by the [`crate::input::Navigator`], which feeds it
//! the steering target and applies the motion it returns; the renderer reports how well it
//! keeps up ([`RenderFeedback`]) and what the current frame looks like ([`Survey`]), and this
//! module decides the speed from both. Everything is integrated in closed form per frame, so
//! a flight is the same at 30, 60 and 120 Hz.
//!
//! **Steering.** The pointer's offset from the view centre, past a dead zone and through a
//! response curve, sets a *heading* `h` (view points from the centre). The zoom expands about
//! `h` and the content pans towards the centre at `k · h` (k grows with the zoom rate), so
//! whatever is under the pointer drifts to the middle while the view dives into it; the true
//! focus of expansion is `h (1 + k / z)`, beyond the pointer, which reads as turning towards it. `h` follows its
//! target on a critically damped spring (no jitter from small mouse moves). No pointer (left
//! the canvas, over the HUD, idle for [`STEER_IDLE`] s, keyboard mode): straight ahead.
//!
//! **Speed.** The zoom rate eases in and out on a spring towards
//! `speed · throttle · safety`. `throttle` (0...1) follows how the renderer keeps up: whether
//! even the coarsest gesture preview misses the frame budget (filtered, [`FIT_TAU`]; the
//! preview step it runs at is the scheduler's business, not a reason to slow), and whether a
//! deep reference is still computing (the view then only shows the stretched history). `safety` drops when the
//! heading region of the current frame is uniform (fully interior or featureless exterior);
//! the HUD then shows a hint.
//!
//! (In A the navigator passes no pointer since 2026-10-17: the steering law above is the
//! cruise's; any user steering hands over to cruise.)
//!
//! **Target lock** ([`lock`], on by default): while A runs and no manual pan holds it off, the
//! heading aims at a boundary target found off-thread (a Misiurewicz point, an embedded Julia
//! stage, a minibrot down to its framing), and the next one is found before the current one's
//! depth is passed.
//!
//! **Pointer wish** ([`Autopilot::set_wish`], kb: decisions/controls "Pointer wish"): the
//! pointer's direction past [`WISH_DEAD_ZONE`], settled with hysteresis, makes the lock's
//! searches and survey steering prefer targets that way and re-targets once when it changes.
//! The aim is always a found target, never the pointer.
//!
//! **Survey steering** (on since 2026-10-17, after the user's foreground comparison): whenever no lock target
//! is followed (none yet, a search pending, a target past its end span with nothing next, or
//! one heading into a uniform region), the heading aims at the GPU survey's boundary point near
//! the current aim: the highest-escape-count texel of a detailed cell, or uphill in escape
//! count when nothing is detailed. Cheap "where is even something" steering.
//!
//! **Rescue**: with the lock on and the user not steering, a heading region that stays uniform
//! (more than [`RESCUE_AFTER`] s without a target, [`RESCUE_AFTER_LOCKED`] s with one) eases
//! the zoom into reverse until a search finds a target in the widening view.

pub mod lock;

use crate::input::Spring;
use crate::viewport::Viewport;
use lock::Lock;

/// Default autopilot zoom speed, e-folds per second (×2.5 per second).
pub const SPEED_DEFAULT: f64 = 0.9;
/// Autopilot zoom speed range (Q / E, scroll).
pub const SPEED_RANGE: (f64, f64) = (0.1, 4.0);
/// Spring frequencies (1/s) of the autopilot zoom rate: speeding up (from rest 50 % after
/// 0.56 s, 90 % after 1.3 s) and slowing down (a stop glides on 2/ω = 0.5 s worth of rate).
pub const RATE_OMEGA: (f64, f64) = (3.0, 4.0);
/// Steering dead zone, as a fraction of the steering radius (half the canvas's smaller side).
pub const STEER_DEAD_ZONE: f64 = 0.1;
/// Spring frequency (1/s) of the heading towards its target (90 % after 0.65 s).
pub const STEER_OMEGA: f64 = 6.0;
/// While the target lock aims: the heading spring's frequency (the aim point itself glides
/// on a target change, [`lock::AIM_BLEND`]) and the rate (1/s) at which the aimed point
/// converges to the centre. The heading for an aim point `p` is `g p` with
/// `g = (z + λ) / (z + k)`: the point then moves at `z (p − h) − k h = −λ p` (zoom about `h`
/// at rate z, pan `−k h`). Aiming the heading straight at `p` through the slower pointer
/// spring oscillated at high zoom rates (the zoom amplifies the spring's lag at rate z).
pub const LOCK_OMEGA: f64 = 20.0;
pub const LOCK_CONVERGE: f64 = 1.5;
/// While the lock aims at a point further out than `CAPTURE.0` steering radii the zoom slows,
/// to `CAPTURE_MIN` at `CAPTURE.1`: the turn can then catch it (a point beyond about
/// `R (1 + k / z)` escapes a dive at rate z).
pub const CAPTURE: (f64, f64) = (0.5, 1.2);
pub const CAPTURE_MIN: f64 = 0.3;
/// Pan rate of the heading towards the centre, 1/s (content at `h` moves at `-k · h`), with
/// `k = STEER_PAN + STEER_PAN_PER_RATE · zoom rate`: turning keeps pace with the dive, so a
/// target up to `R (1 + k / z)` off-centre (R: the steering radius) can still be caught.
pub const STEER_PAN: f64 = 0.5;
pub const STEER_PAN_PER_RATE: f64 = 1.0;
/// A pointer that has not moved for this long (s) no longer steers: straight ahead.
pub const STEER_IDLE: f64 = 3.0;
/// Q / E speed factors, a wheel notch's factor, and precise scrolling (per point).
pub const SPEED_STEP: f64 = 1.25;
pub const NOTCH_FACTOR: f64 = 1.1;
pub const SCROLL_PER_POINT: f64 = 0.004;
/// A double click this soon after a click undoes that click's pause toggle.
pub const CLICK_UNDO: f64 = 0.5;
/// Throttle and safety follow their targets with these time constants (s): down fast, up
/// gently.
pub const THROTTLE_TAU: (f64, f64) = (0.25, 1.0);
/// Gesture-preview resolution (preview texels per pixel, unclamped) at which the throttle is
/// 1, and at which it reaches its floor. The fit reads back the resolution step the scheduler
/// is at (its frames fill the budget at any step, and the GPU clocks follow the load), so it is
/// no measure of headroom; only a fit below the coarsest step (1/8) says that the frames do not
/// fit the budget. 0.1 at 1/8: GPU time 1.56x the budget (half the refresh interval), 0.78 of
/// the interval; 1/16: 4x the budget, every other refresh (kb: decisions/controls "Autopilot",
/// "Steady autopilot rate").
pub const FIT_FULL: f64 = 0.1;
pub const FIT_FLOOR: f64 = 0.0625;
/// The fit enters the throttle through a filter of its log with these time constants (s): a
/// single slow chunk (another process's GPU work) does not dip the rate, an overload that
/// lasts does within about a second.
pub const FIT_TAU: (f64, f64) = (0.5, 1.0);
/// Throttle floor while previews are too coarse, and while the view waits for a reference.
pub const THROTTLE_MIN: f64 = 0.3;
pub const THROTTLE_WAITING: f64 = 0.06;
/// Safety factor in a uniform heading region.
pub const SAFETY_MIN: f64 = 0.3;
/// Band crossings per view width in the heading region: below `DETAIL_LO` the heading is
/// uniform, above `DETAIL_HI` it is detailed.
pub const DETAIL_LO: f64 = 3.0;
pub const DETAIL_HI: f64 = 12.0;
/// Gaussian radius of the heading region, as a fraction of the view's smaller side.
pub const HEADING_SIGMA: f64 = 0.15;
/// Rescue: seconds of a uniform heading region (danger > 0.5) without a lock target, and with
/// one, before the zoom eases into reverse.
pub const RESCUE_AFTER: f64 = 1.0;
pub const RESCUE_AFTER_LOCKED: f64 = 2.5;
/// Rescue zoom-out rate (e-folds/s): this share of the set speed, within the range.
pub const RESCUE_SHARE: f64 = 0.6;
pub const RESCUE_RATE: (f64, f64) = (0.5, 1.5);
/// A rescue that finds nothing speeds up: by 1 + t / `RESCUE_ACCEL` after t s of it, up to
/// `RESCUE_RATE_MAX` e-folds/s.
pub const RESCUE_ACCEL: f64 = 2.0;
pub const RESCUE_RATE_MAX: f64 = 3.0;
/// Rescue ends at this span across the shorter side (the whole set is in view).
pub const RESCUE_MAX_SPAN: f64 = 4.0;
/// Rescue also ends after this long (s) with detail in the surveyed heading region.
pub const RESCUE_CLEAR: f64 = 0.6;
/// Such a rescue aims at the survey's busiest cell for this many e-folds.
pub const SURVEY_PICK_GAIN: f64 = 2.0;
pub const SURVEY_PICK_LABEL: &str = "detail in view";
/// Survey steering (kb: decisions/controls "Survey steering"): while no lock target is
/// followed, the autopilot aims at the survey's boundary point nearest the heading
/// ([`Survey::boundary_point`]). The aim point eases towards each new pick with this
/// time constant (s): surveys arrive at 10 Hz and the peak moves within its cell.
pub const SURVEY_STEER_TAU: f64 = 0.3;
/// Survey steering prefers detail near the current aim: Gaussian radius, as a fraction of
/// the view's shorter side.
pub const STEER_SIGMA: f64 = 0.25;
pub const SURVEY_STEER_LABEL: &str = "towards the nearest detail";
/// With survey steering on, a lock target whose heading region has been uniform this long (s)
/// leads nowhere: it is dropped (and not taken again) and survey steering takes over at once
/// instead of after [`RESCUE_AFTER_LOCKED`].
pub const SURVEY_STEER_OVERRIDE: f64 = 0.4;
/// Manual pans, zooms and drags hold the lock off for this long (s).
pub const MANUAL_HOLD: f64 = 1.0;
/// A survey older than this (s) is ignored.
pub const SURVEY_STALE: f64 = 0.6;
/// Pointer wish (kb: decisions/controls "Autopilot", "Pointer wish"): in A the pointer does not
/// steer; its direction, once deflected past `WISH_DEAD_ZONE.1` steering radii (kept down to
/// `WISH_DEAD_ZONE.0`: no flicker at the edge), is a coarse wish. The lock's searches and the
/// survey's boundary point prefer candidates in that direction; the aim is always a found
/// target or boundary point, never the pointer itself.
pub const WISH_DEAD_ZONE: (f64, f64) = (0.15, 0.2);
/// The wish counts once it has held this long (s), and a new wish replaces the settled one
/// once it points more than `WISH_TURN` radians away for that long: the aim does not jitter
/// with the hand, and a changed wish re-targets once.
pub const WISH_SETTLE: f64 = 0.3;
pub const WISH_TURN: f64 = std::f64::consts::FRAC_PI_4;
/// Candidates closer to the centre than this share of the shorter side are "straight ahead":
/// neutral to the wish (their direction means nothing).
pub const WISH_NEAR: f64 = 0.08;
/// Survey steering's weight of a cell against the wish: `exp(WISH_SHARPNESS (agreement − 1))`
/// (opposite ×0.007, sideways ×0.08).
pub const WISH_SHARPNESS: f64 = 2.5;
/// Survey steering's distance preference is centred this share of the shorter side ahead of
/// its usual point, in the wished direction.
pub const WISH_LEAD: f64 = 0.25;
/// HUD suffix while a wish is settled.
pub const WISH_LABEL: &str = "your direction";

/// A settled pointer wish: a unit direction in view points (y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wish {
    pub dir: (f64, f64),
    /// Bumped whenever the settled wish appears or turns (re-target once per change).
    pub epoch: u32,
}

impl Wish {
    /// How well the offset `p` (view points) agrees with the wish: the cosine of the angle
    /// between them, faded to 0 within `near` points of the centre (straight ahead).
    pub fn agreement(&self, p: (f64, f64), near: f64) -> f64 {
        let r = p.0.hypot(p.1);
        if !(r.is_finite() && r > 0.0) {
            return 0.0;
        }
        let cos = (p.0 * self.dir.0 + p.1 * self.dir.1) / r;
        cos * smoothstep(r / near.max(1e-9))
    }
}

/// Why the autopilot is slower than its set speed (the HUD shows it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Hint {
    #[default]
    None,
    /// The renderer cannot keep the previews sharp at this speed.
    Rendering,
    /// A deep reference is computing; the view waits for it.
    Reference,
    /// Heading into the interior (black).
    Interior,
    /// Heading into featureless exterior.
    Featureless,
    /// The depth limit: zooming stops.
    Limit,
    /// Nothing ahead and no target found: zooming out until there is.
    Rescue,
}

/// How the renderer keeps up, reported every frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderFeedback {
    /// Preview texels per pixel that fit the gesture frame budget (unclamped; the scheduler
    /// uses 1/8 ... 1). NaN while unknown.
    pub fit: f64,
    /// The view waits for a deep reference: frames show the reprojected history only.
    pub waiting_reference: bool,
    /// At the depth limit.
    pub at_limit: bool,
}

impl Default for RenderFeedback {
    fn default() -> Self {
        Self {
            fit: f64::NAN,
            waiting_reference: false,
            at_limit: false,
        }
    }
}

/// Counts of one survey cell (a GPU pass over the shown level's iteration texture).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurveyCell {
    /// Sampled texels (pending ones are skipped).
    pub samples: u32,
    /// Interior or unresolved (drawn as interior) samples.
    pub interior: u32,
    /// Neighbour pairs compared, and those whose class or integer escape count differ.
    pub pairs: u32,
    pub edges: u32,
    /// The escaped sample with the highest escape count (view points from the centre of the
    /// surveyed view, y up) and that count; meaningful when `samples > interior`. The escape
    /// count rises towards the set, so this is the cell's sample nearest the boundary.
    pub peak: (f64, f64),
    pub peak_n: f64,
}

impl SurveyCell {
    /// Some sample escaped (the peak is meaningful).
    pub fn has_peak(&self) -> bool {
        self.samples > self.interior
    }
}

/// A coarse picture of a frame: `cols` x `rows` cells in raster order from the top-left.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Survey {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<SurveyCell>,
    /// Size of the surveyed view in view points.
    pub view: (f64, f64),
    /// Texels across and down the surveyed level (band crossings per view width = edge rate
    /// × `texels_across`).
    pub texels_across: f64,
    pub texels_down: f64,
    /// When it was taken (the navigator's clock).
    pub time: f64,
    /// The surveyed view (cell positions are view points in it); None: the current view.
    pub viewport: Option<Viewport>,
}

/// What a survey says about a heading.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Assessment {
    /// Interior share of the heading region, 0...1.
    pub interior: f64,
    /// Band crossings per view width in the heading region.
    pub detail: f64,
}

impl Assessment {
    /// 0 (detailed) ... 1 (uniform): how featureless the heading region is.
    pub fn danger(&self) -> f64 {
        1.0 - smoothstep((self.detail - DETAIL_LO) / (DETAIL_HI - DETAIL_LO))
    }
}

impl Survey {
    /// Centre of cell (i, j) in view points from the view centre, y up.
    pub fn cell_center(&self, i: usize, j: usize) -> (f64, f64) {
        (
            ((i as f64 + 0.5) / self.cols as f64 - 0.5) * self.view.0,
            (0.5 - (j as f64 + 0.5) / self.rows as f64) * self.view.1,
        )
    }

    /// The centre (view points) of the cell with the most boundary detail (band crossings
    /// per view width, discounted by interior), if any is detailed.
    pub fn busiest(&self) -> Option<(f64, f64)> {
        if self.cols == 0 || self.rows == 0 || self.cells.len() != self.cols * self.rows {
            return None;
        }
        let mut best: Option<(f64, usize, usize)> = None;
        for j in 0..self.rows {
            for i in 0..self.cols {
                let c = self.cells[j * self.cols + i];
                if c.samples == 0 || c.pairs == 0 {
                    continue;
                }
                let det = c.edges as f64 / c.pairs as f64 * self.texels_across;
                let rich = det.min(4.0 * DETAIL_HI) * (1.0 - c.interior as f64 / c.samples as f64);
                // Near the centre first: less turning.
                let (x, y) = self.cell_center(i, j);
                let side = 0.35 * self.view.0.min(self.view.1);
                let score = rich * (-(x * x + y * y) / (2.0 * side * side)).exp();
                if rich >= DETAIL_HI && best.is_none_or(|b| score > b.0) {
                    best = Some((score, i, j));
                }
            }
        }
        best.map(|(_, i, j)| self.cell_center(i, j))
    }

    /// Band crossings per view width in cell `c`.
    fn cell_detail(&self, c: &SurveyCell) -> f64 {
        if c.pairs == 0 {
            return 0.0;
        }
        c.edges as f64 / c.pairs as f64 * self.texels_across
    }

    /// Survey steering (kb: decisions/controls "Survey steering"): the boundary point to steer
    /// to from `from` (view points): the peak of the detailed cell (band crossings ≥
    /// [`DETAIL_HI`], not all interior) with the most detail near `from` (detail × escaped
    /// share × a Gaussian of the distance, σ = [`STEER_SIGMA`] of the shorter side); with none
    /// detailed, the highest peak in view (the escape count rises towards the set: uphill is
    /// where something is).
    ///
    /// With a pointer `wish`, each cell's score is weighted by its direction from the view
    /// centre (`exp(WISH_SHARPNESS (agreement − 1))`) and its distance preference is centred
    /// [`WISH_LEAD`] ahead in that direction; a detailed cell whose peak lies in the wished half
    /// wins over any other, and uphill takes the highest peak in the wished half if any.
    pub fn boundary_point(&self, from: (f64, f64), wish: Option<&Wish>) -> Option<(f64, f64)> {
        if self.cols == 0 || self.rows == 0 || self.cells.len() != self.cols * self.rows {
            return None;
        }
        let side = self.view.0.min(self.view.1).max(1.0);
        let sigma = STEER_SIGMA * side;
        let agree = |p: (f64, f64)| wish.map_or(0.0, |w| w.agreement(p, WISH_NEAR * side));
        // The distance preference is centred ahead of `from` in the wished direction.
        let from = wish.map_or(from, |w| {
            (from.0 + WISH_LEAD * side * w.dir.0, from.1 + WISH_LEAD * side * w.dir.1)
        });
        // The best detailed cell and the highest escape count in the wished half (with a wish):
        // they win over the others (detail elsewhere still beats uphill here).
        let mut best_wished: Option<(f64, (f64, f64))> = None;
        let mut high_wished: Option<(f64, (f64, f64))> = None;
        // (score, peak): the best detailed cell, and the highest escape count.
        let mut best: Option<(f64, (f64, f64))> = None;
        let mut high: Option<(f64, (f64, f64))> = None;
        for j in 0..self.rows {
            for i in 0..self.cols {
                let c = &self.cells[j * self.cols + i];
                if !c.has_peak() || !(c.peak.0.is_finite() && c.peak.1.is_finite() && c.peak_n.is_finite()) {
                    continue;
                }
                let det = self.cell_detail(c);
                if det >= DETAIL_HI {
                    let (x, y) = self.cell_center(i, j);
                    let d2 = (x - from.0).powi(2) + (y - from.1).powi(2);
                    let escaped = 1.0 - c.interior as f64 / c.samples as f64;
                    let wished = if wish.is_some() {
                        (WISH_SHARPNESS * (agree((x, y)) - 1.0)).exp()
                    } else {
                        1.0
                    };
                    let score = det.min(4.0 * DETAIL_HI) * escaped * (-d2 / (2.0 * sigma * sigma)).exp() * wished;
                    if best.is_none_or(|b| score > b.0) {
                        best = Some((score, c.peak));
                    }
                    if agree(c.peak) > 0.0 && best_wished.is_none_or(|b| score > b.0) {
                        best_wished = Some((score, c.peak));
                    }
                }
                if high.is_none_or(|b| c.peak_n > b.0) {
                    high = Some((c.peak_n, c.peak));
                }
                if agree(c.peak) > 0.0 && high_wished.is_none_or(|b| c.peak_n > b.0) {
                    high_wished = Some((c.peak_n, c.peak));
                }
            }
        }
        best_wished.or(best).or(high_wished).or(high).map(|b| b.1)
    }

    /// The region around `p` (view points) is not uniform by the heading test (danger ≤ 0.5,
    /// not mostly interior): a survey-steering pick there is kept.
    pub fn holds_detail(&self, p: (f64, f64)) -> bool {
        self.assess(p).is_some_and(|a| a.danger() <= 0.5 && a.interior <= 0.5)
    }

    /// Interior share and detail around `heading` (Gaussian weights).
    pub fn assess(&self, heading: (f64, f64)) -> Option<Assessment> {
        if self.cols == 0 || self.rows == 0 || self.cells.len() != self.cols * self.rows {
            return None;
        }
        let side = self.view.0.min(self.view.1);
        if side <= 0.0 {
            return None;
        }
        let s1 = HEADING_SIGMA * side;
        let (mut w_sum, mut interior, mut detail) = (0.0, 0.0, 0.0);
        for j in 0..self.rows {
            for i in 0..self.cols {
                let c = self.cells[j * self.cols + i];
                if c.samples == 0 {
                    continue;
                }
                let (x, y) = self.cell_center(i, j);
                let d2 = (x - heading.0).powi(2) + (y - heading.1).powi(2);
                let int = c.interior as f64 / c.samples as f64;
                let det = if c.pairs > 0 {
                    c.edges as f64 / c.pairs as f64 * self.texels_across
                } else {
                    0.0
                };
                let w = (-d2 / (2.0 * s1 * s1)).exp();
                w_sum += w;
                interior += w * int;
                detail += w * det;
            }
        }
        if w_sum <= 1e-9 {
            return None;
        }
        Some(Assessment {
            interior: interior / w_sum,
            detail: detail / w_sum,
        })
    }
}

/// Smooth 0 → 1 ramp of `t` (zero slope at both ends).
pub fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Steering response of a deflection `r` (in steering radii): 0 inside the dead zone, then
/// `0.35 m + 0.65 m²` of the rescaled magnitude (fine near the centre, full at the edge).
pub fn steer_response(r: f64) -> f64 {
    let m = ((r - STEER_DEAD_ZONE) / (1.0 - STEER_DEAD_ZONE)).clamp(0.0, 1.0);
    0.35 * m + 0.65 * m * m
}

/// Moves `x` towards `target` with time constant `tau`, exactly for any `dt`.
fn approach(x: f64, target: f64, tau: f64, dt: f64) -> f64 {
    target + (x - target) * (-dt / tau).exp()
}

/// The autopilot's state; see the module docs.
#[derive(Clone, Debug)]
pub struct Autopilot {
    active: bool,
    paused: bool,
    speed: f64,
    /// When the speed last changed (HUD highlight).
    pub speed_changed: f64,
    /// Zoom rate (e-folds/s) and the heading per axis (view points; the spring's value).
    rate: Spring,
    heading: [Spring; 2],
    throttle: f64,
    /// ln of the renderer's fit, filtered with [`FIT_TAU`] (None while unknown).
    fit_ln: Option<f64>,
    safety: f64,
    hint: Hint,
    feedback: RenderFeedback,
    survey: Option<Survey>,
    /// Target lock (kb: decisions/controls "Autopilot").
    lock: Lock,
    /// The user does not steer (set by [`Autopilot::aim`] each frame).
    free: bool,
    /// Seconds the heading region has been uniform while free; zooming out to find detail.
    uniform_for: f64,
    /// Seconds the surveyed heading region has shown detail (ends a rescue).
    clear_for: f64,
    /// A rescue ended on survey detail without a target: aim at the busiest cell once.
    survey_pick: bool,
    rescue: bool,
    /// ln of the view's span across its shorter side (last [`Autopilot::aim`]).
    span_ln: f64,
    /// The lock aims this frame (the heading spring is then [`LOCK_OMEGA`]).
    lock_aiming: bool,
    /// The lock's aim point's distance from the centre, in steering radii.
    aim_far: f64,
    last_click: f64,
    /// The heading target of the last frame (tests, HUD).
    target: (f64, f64),
    /// Survey steering on (always, outside tests that compare), and it aimed in the last frame.
    survey_steer: bool,
    survey_steering: bool,
    /// Survey steering: the latest pick, the aim point easing towards it, the survey picked
    /// from (its time) and the last frame's time.
    steer_pick: Option<crate::viewport::Center>,
    steer_aim: Option<crate::viewport::Center>,
    steer_survey: f64,
    steer_time: f64,
    /// Seconds the current lock target (by [`Lock::switches`]) has headed into a uniform region.
    target_uniform: (f64, u32),
    /// `ALMONDBREAD_LOCK_TRACE`: survey steering prints its picks.
    trace: bool,
    /// The pointer's direction past the wish dead zone this frame (unit, view points), the
    /// settled wish, and since when the pointer has differed from the settled wish.
    wish_raw: Option<(f64, f64)>,
    wish: Option<Wish>,
    wish_epochs: u32,
    wish_change: Option<f64>,
    /// The wish epoch survey steering's pick was made with.
    steer_epoch: u32,
}

impl Default for Autopilot {
    fn default() -> Self {
        Self {
            active: false,
            paused: false,
            speed: SPEED_DEFAULT,
            speed_changed: f64::NEG_INFINITY,
            rate: Spring::default(),
            heading: [Spring::default(); 2],
            throttle: 1.0,
            fit_ln: None,
            safety: 1.0,
            hint: Hint::None,
            feedback: RenderFeedback::default(),
            survey: None,
            lock: Lock::default(),
            free: false,
            uniform_for: 0.0,
            clear_for: 0.0,
            survey_pick: false,
            rescue: false,
            span_ln: f64::NEG_INFINITY,
            lock_aiming: false,
            aim_far: 0.0,
            last_click: f64::NEG_INFINITY,
            target: (0.0, 0.0),
            survey_steer: true,
            survey_steering: false,
            steer_pick: None,
            steer_aim: None,
            steer_survey: f64::NEG_INFINITY,
            steer_time: f64::NEG_INFINITY,
            target_uniform: (0.0, 0),
            trace: std::env::var_os("ALMONDBREAD_LOCK_TRACE").is_some(),
            wish_raw: None,
            wish: None,
            wish_epochs: 0,
            wish_change: None,
            steer_epoch: 0,
        }
    }
}

/// One frame of autopilot motion.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    /// Content pan this frame, view points (at the frame's midpoint scale).
    pub pan: (f64, f64),
    /// ln of the zoom factor this frame, and the point it zooms about (the mean heading).
    pub ln_zoom: f64,
    pub at: (f64, f64),
}

impl Autopilot {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Set speed, e-folds per second.
    pub fn speed(&self) -> f64 {
        self.speed
    }

    pub fn set_speed(&mut self, v: f64, now: f64) {
        if v.is_finite() {
            self.speed = v.clamp(SPEED_RANGE.0, SPEED_RANGE.1);
            self.speed_changed = now;
        }
    }

    /// Current zoom rate, e-folds per second (eased).
    pub fn rate(&self) -> f64 {
        self.rate.v
    }

    pub fn throttle(&self) -> f64 {
        self.throttle
    }

    pub fn safety(&self) -> f64 {
        self.safety
    }

    pub fn hint(&self) -> Hint {
        self.hint
    }

    /// Current heading, view points from the centre.
    pub fn heading(&self) -> (f64, f64) {
        (self.heading[0].v, self.heading[1].v)
    }

    /// The heading's target in the last frame.
    pub fn heading_target(&self) -> (f64, f64) {
        self.target
    }

    /// Target lock on (the default) or off (a pure joystick).
    pub fn lock_enabled(&self) -> bool {
        self.lock.is_enabled()
    }

    pub fn set_lock(&mut self, on: bool) {
        self.lock.set_enabled(on);
        if !on {
            self.rescue = false;
            self.uniform_for = 0.0;
        }
    }

    pub fn lock(&self) -> &Lock {
        &self.lock
    }

    /// Survey steering on.
    pub fn survey_steer(&self) -> bool {
        self.survey_steer
    }

    pub fn set_survey_steer(&mut self, on: bool) {
        self.survey_steer = on;
        self.reset_steer();
    }

    /// Survey steering's current pick (a complex point), if any.
    pub fn survey_pick(&self) -> Option<&crate::viewport::Center> {
        self.steer_pick.as_ref()
    }

    /// Survey steering aimed in the last frame.
    pub fn is_survey_steering(&self) -> bool {
        self.survey_steering
    }

    fn reset_steer(&mut self) {
        self.survey_steering = false;
        self.steer_pick = None;
        self.steer_aim = None;
    }

    pub fn lock_mut(&mut self) -> &mut Lock {
        &mut self.lock
    }

    /// The zoom-rate factor while the lock turns towards a far aim point (1 otherwise).
    pub fn capture(&self) -> f64 {
        if !self.lock_aiming {
            return 1.0;
        }
        1.0 - (1.0 - CAPTURE_MIN) * smoothstep((self.aim_far - CAPTURE.0) / (CAPTURE.1 - CAPTURE.0))
    }

    /// Zooming out to find detail.
    pub fn is_rescuing(&self) -> bool {
        self.rescue
    }

    /// The user did not steer in the last frame (the lock may aim).
    pub fn is_free(&self) -> bool {
        self.free
    }

    /// What the lock does, for the HUD ("" while off or steered).
    pub fn lock_label(&self) -> String {
        if !self.active || !self.lock.is_enabled() || !self.free {
            return String::new();
        }
        let label = if self.survey_steering {
            SURVEY_STEER_LABEL.into()
        } else {
            match self.lock.target() {
                // Journey labels read "period-34 minibrot", "towards a period-34 minibrot", ...
                Some(t) if t.label.starts_with("towards") => t.label.clone(),
                Some(t) => format!("towards {}", t.label),
                None if self.lock.is_searching() => "looking for a target".into(),
                None => String::new(),
            }
        };
        if self.wish.is_some() && !label.is_empty() {
            format!("{label} · {WISH_LABEL}")
        } else {
            label
        }
    }

    /// The settled pointer wish (kb: decisions/controls "Pointer wish").
    pub fn wish(&self) -> Option<Wish> {
        self.wish
    }

    /// The pointer's offset from the view centre this frame (None: no wish: off the canvas,
    /// keyboard mode, not moved since A took the wheel), `radius` the steering radius. Past
    /// [`WISH_DEAD_ZONE`] its direction becomes the settled wish after [`WISH_SETTLE`] s; a
    /// wish turning more than [`WISH_TURN`] (or appearing) bumps the epoch, so the lock and
    /// survey steering re-target once. Letting go clears it after the same delay; nothing is
    /// re-targeted then (the current aim is as good as any).
    pub fn set_wish(&mut self, pointer: Option<(f64, f64)>, radius: f64, now: f64) {
        if !self.active {
            self.wish_raw = None;
            self.wish = None;
            self.wish_change = None;
            return;
        }
        let edge = if self.wish_raw.is_some() {
            WISH_DEAD_ZONE.0
        } else {
            WISH_DEAD_ZONE.1
        };
        self.wish_raw = pointer.and_then(|(x, y)| {
            let r = x.hypot(y);
            (radius > 0.0 && r.is_finite() && r >= edge * radius).then(|| (x / r, y / r))
        });
        let changed = match (self.wish_raw, self.wish) {
            (Some(d), Some(w)) => d.0 * w.dir.0 + d.1 * w.dir.1 < WISH_TURN.cos(),
            (Some(_), None) | (None, Some(_)) => true,
            (None, None) => false,
        };
        if !changed {
            self.wish_change = None;
            return;
        }
        let since = *self.wish_change.get_or_insert(now);
        if now - since >= WISH_SETTLE {
            self.wish_change = None;
            self.wish = self.wish_raw.map(|dir| {
                self.wish_epochs += 1;
                Wish {
                    dir,
                    epoch: self.wish_epochs,
                }
            });
        }
    }

    pub fn survey(&self) -> Option<&Survey> {
        self.survey.as_ref()
    }

    /// Z / the menu: on (eased start) or off (eased stop).
    pub fn toggle(&mut self) {
        if self.active {
            self.stop();
        } else {
            self.lock.release();
            self.active = true;
            self.paused = false;
            self.wish = None;
            self.wish_raw = None;
            self.wish_change = None;
        }
    }

    /// Esc: an eased stop. Returns whether it was on.
    pub fn stop(&mut self) -> bool {
        let was = self.active;
        self.active = false;
        self.paused = false;
        self.lock.release();
        self.rescue = false;
        self.uniform_for = 0.0;
        was
    }

    /// Reset: stop dead.
    pub fn halt(&mut self) {
        self.active = false;
        self.paused = false;
        self.rate = Spring::default();
        self.heading = [Spring::default(); 2];
        self.hint = Hint::None;
        self.lock.release();
        self.rescue = false;
        self.uniform_for = 0.0;
    }

    /// A click on the canvas: pause / resume (eased). Returns whether it was used.
    pub fn click(&mut self, now: f64) -> bool {
        if !self.active {
            return false;
        }
        self.paused = !self.paused;
        self.last_click = now;
        true
    }

    /// A double click: the click that began it toggled the pause; undo that.
    pub fn double_click(&mut self, now: f64) {
        if self.active && now - self.last_click <= CLICK_UNDO {
            self.paused = !self.paused;
            self.last_click = f64::NEG_INFINITY;
        }
    }

    /// Scales the set speed (Q / E, wheel, scroll).
    pub fn scale_speed(&mut self, factor: f64, now: f64) {
        if factor.is_finite() && factor > 0.0 {
            self.set_speed(self.speed * factor, now);
        }
    }

    pub fn set_feedback(&mut self, f: RenderFeedback) {
        self.feedback = f;
    }

    /// The renderer's last feedback (the cruise reads the depth limit and the back-off).
    pub fn feedback(&self) -> RenderFeedback {
        self.feedback
    }

    pub fn set_survey(&mut self, s: Survey) {
        self.survey = Some(s);
    }

    /// Something is still moving (or will: on and not paused).
    pub fn is_moving(&self) -> bool {
        (self.active && !self.paused) || !self.rate_at_rest() || !self.heading_at_rest()
    }

    fn rate_at_rest(&self) -> bool {
        self.rate.v == 0.0 && self.rate.a == 0.0
    }

    fn heading_at_rest(&self) -> bool {
        self.heading.iter().all(|s| s.v == 0.0 && s.a == 0.0)
    }

    /// Moves the throttle towards what the renderer's feedback asks for (also used by
    /// fly-to). Returns the hint that applies while it is low.
    pub fn update_throttle(&mut self, dt: f64) -> Hint {
        let fb = self.feedback;
        self.fit_ln = if fb.fit.is_finite() && fb.fit > 0.0 {
            let x = fb.fit.ln();
            Some(match self.fit_ln {
                Some(f) => approach(f, x, if x < f { FIT_TAU.0 } else { FIT_TAU.1 }, dt),
                None => x,
            })
            .filter(|f| f.is_finite())
        } else {
            None
        };
        let (target, hint) = if fb.waiting_reference {
            (THROTTLE_WAITING, Hint::Reference)
        } else if let Some(f) = self.fit_ln {
            let t = smoothstep((f.exp() - FIT_FLOOR) / (FIT_FULL - FIT_FLOOR));
            (THROTTLE_MIN + (1.0 - THROTTLE_MIN) * t, Hint::Rendering)
        } else {
            (1.0, Hint::None)
        };
        let tau = if target < self.throttle {
            THROTTLE_TAU.0
        } else {
            THROTTLE_TAU.1
        };
        self.throttle = approach(self.throttle, target, tau, dt);
        if self.throttle < 0.85 { hint } else { Hint::None }
    }

    /// Survey assessment at the current heading, if a fresh survey exists.
    pub fn assessment(&self, now: f64) -> Option<Assessment> {
        let s = self.survey.as_ref()?;
        if now - s.time > SURVEY_STALE {
            return None;
        }
        s.assess(self.heading())
    }

    /// The heading target from the steering deflection: `pointer` is the pointer's offset
    /// (None: straight), `radius` the steering radius in points. Returns view points.
    pub fn steer_target(&self, pointer: Option<(f64, f64)>, radius: f64) -> (f64, f64) {
        if radius <= 0.0 {
            return (0.0, 0.0);
        }
        if let Some((x, y)) = pointer {
            let r = x.hypot(y) / radius;
            if r > 0.0 {
                let k = steer_response(r) / r;
                return (x * k, y * k);
            }
        }
        (0.0, 0.0)
    }

    /// The frame's heading target: the pointer's while it deflects past the dead zone (or
    /// `manual` input moves the view), else the lock's target, else straight ahead. `view` is
    /// the current view, `min_side` its shorter side and `radius` the steering radius (points).
    pub fn aim(
        &mut self,
        view: &Viewport,
        min_side: f64,
        radius: f64,
        pointer: Option<(f64, f64)>,
        manual: bool,
        now: f64,
    ) -> (f64, f64) {
        let steer = self.steer_target(pointer, radius);
        self.free = steer == (0.0, 0.0) && !manual;
        self.span_ln = lock::view_span(view, min_side).ln();
        self.lock_aiming = false;
        self.survey_steering = false;
        let running = self.active && !self.paused;
        if !running {
            if !self.active {
                self.lock.release();
            }
            self.reset_steer();
            return steer;
        }
        if radius <= 0.0 {
            return steer;
        }
        // A survey pick that reached its end with nothing found ahead: pick again (a chain of
        // short legs, each towards detail the GPU shows).
        let span_now = lock::view_span(view, min_side);
        let pick_spent = self.lock.next_target().is_none()
            && self
                .lock
                .target()
                .is_some_and(|t| t.label == SURVEY_PICK_LABEL && span_now <= t.end_span);
        // (Survey steering does this every frame.)
        if self.free && !self.survey_steer && (pick_spent || (self.survey_pick && self.lock.target().is_none())) {
            self.survey_pick = false;
            if let Some((x, y)) = self
                .survey
                .as_ref()
                .filter(|s| now - s.time <= SURVEY_STALE)
                .and_then(Survey::busiest)
            {
                let span = lock::view_span(view, min_side);
                self.lock.set_target(lock::LockTarget {
                    kind: crate::journey::TargetKind::Detail,
                    center: view.point_at_offset(x, y),
                    end_span: span * (-SURVEY_PICK_GAIN).exp(),
                    label: SURVEY_PICK_LABEL.into(),
                    score: 0.0,
                    period: None,
                    preperiod: None,
                });
            }
        }
        if self.survey_steer
            && self.free
            && !self.rescue
            && self.target_uniform.0 > SURVEY_STEER_OVERRIDE
            && self.lock.target().is_some()
        {
            // A target that leads into a void: survey steering now, the lock looks again.
            self.lock.drop_target();
        }
        let heading = self.heading();
        let lock_aim = self
            .lock
            .update(view, min_side, radius, self.free, heading, self.wish.as_ref(), now);
        // A target past its end span with nothing next leads nowhere (the CPU search found
        // nothing ahead): survey steering until the lock has the next one.
        let spent = self.lock.next_target().is_none() && self.lock.target().is_some_and(|t| span_now <= t.end_span);
        let aim = match lock_aim {
            Some(p) if !(self.survey_steer && spent) => {
                self.reset_steer();
                Some(p)
            }
            _ if self.survey_steer && self.free && self.lock.is_enabled() => {
                let a = self.survey_aim(view, min_side, now);
                self.survey_steering = a.is_some();
                a
            }
            other => other,
        };
        let Some((px, py)) = aim else {
            self.reset_steer();
            return steer;
        };
        self.lock_aiming = true;
        self.aim_far = px.hypot(py) / radius;
        // The heading that brings the aim point to the centre at LOCK_CONVERGE (see there).
        let z = self.rate.v.max(0.0);
        let k = (STEER_PAN + STEER_PAN_PER_RATE * z) * self.throttle.max(0.05);
        let g = ((z + LOCK_CONVERGE) / (z + k)).min(4.0);
        let (hx, hy) = (g * px, g * py);
        let r = hx.hypot(hy);
        let max = lock::AIM_MAX * radius;
        if r > max {
            (hx * max / r, hy * max / r)
        } else {
            (hx, hy)
        }
    }

    /// Survey steering's aim point this frame (view points), see [`Survey::boundary_point`]: the
    /// pick (a complex point, so the zoom and pan since its survey do not move it) is kept while
    /// each new survey finds detail around it ([`Survey::holds_detail`]); then the survey's
    /// boundary point near it is picked ([`Survey::boundary_point`]), a chain of short legs. The
    /// aim point eases towards it with [`SURVEY_STEER_TAU`] (exact for any frame rate between
    /// surveys). Starts from where the heading points. None without a pick.
    fn survey_aim(&mut self, view: &Viewport, min_side: f64, now: f64) -> Option<(f64, f64)> {
        let dt = if self.steer_aim.is_some() {
            (now - self.steer_time).clamp(0.0, 0.25)
        } else {
            0.0
        };
        self.steer_time = now;
        let heading = self.heading();
        let fresh = self
            .survey
            .as_ref()
            .filter(|s| now - s.time <= SURVEY_STALE && s.time != self.steer_survey);
        if let Some(s) = fresh {
            let base = s.viewport.as_ref().unwrap_or(view);
            // The current pick is kept while the region around it holds detail (a re-pick every
            // survey hopped between cells near the detail threshold: measured, it oscillated
            // in a sparse region until the rescue); then the next one is picked near it.
            // A new wish (appeared or turned) re-picks once, from the centre towards it.
            let rewish = self.wish.is_some_and(|w| w.epoch != self.steer_epoch);
            let held = self
                .steer_pick
                .as_ref()
                .filter(|_| !rewish)
                .map(|c| base.offset_of(c))
                .filter(|&(x, y)| x.is_finite() && y.is_finite() && s.holds_detail((x, y)));
            let from = match (&self.steer_pick, &self.steer_aim) {
                _ if rewish => base.offset_of(&view.center),
                (Some(c), _) | (None, Some(c)) => base.offset_of(c),
                (None, None) => base.offset_of(&view.point_at_offset(heading.0, heading.1)),
            };
            let from = if from.0.is_finite() && from.1.is_finite() {
                from
            } else {
                (0.0, 0.0)
            };
            // Heading into a uniform region: the nearest detail in any direction, the wish is
            // ignored (kb: decisions/controls "Pointer wish"). With the wish the deep test path at
            // 1e239 picked sparse cells in the wished half and backed out in 6 of 10 runs; with
            // the wish as a soft weight only, still 5 of 8; ignored, 1 of 8.
            let wish = self.wish.filter(|_| self.uniform_for <= 0.0);
            let p = if held.is_some() {
                None
            } else {
                s.boundary_point(from, wish.as_ref())
            };
            if self.trace && held.is_none() {
                let detailed = s
                    .cells
                    .iter()
                    .filter(|c| c.has_peak() && s.cell_detail(c) >= DETAIL_HI)
                    .count();
                println!(
                    "survey steer at 1e{:.2}: from ({:.0},{:.0}) pick {} ({detailed} detailed cells)",
                    view.zoom_log10(),
                    from.0,
                    from.1,
                    p.map_or("none".into(), |(x, y)| format!("({x:.0},{y:.0})")),
                );
            }
            let pick = p.map(|(x, y)| base.point_at_offset(x, y));
            self.steer_survey = s.time;
            if pick.is_some() {
                self.steer_pick = pick;
                self.steer_epoch = self.wish.map_or(self.steer_epoch, |w| w.epoch);
            }
        }
        let (px, py) = view.offset_of(self.steer_pick.as_ref()?);
        if !(px.is_finite() && py.is_finite()) || px.hypot(py) > lock::LOST * min_side {
            self.reset_steer();
            return None;
        }
        let (ax, ay) = match &self.steer_aim {
            Some(c) => view.offset_of(c),
            None => heading,
        };
        let f = 1.0 - (-dt / SURVEY_STEER_TAU).exp();
        let (x, y) = (ax + (px - ax) * f, ay + (py - ay) * f);
        if !(x.is_finite() && y.is_finite()) {
            self.reset_steer();
            return None;
        }
        self.steer_aim = Some(view.point_at_offset(x, y));
        Some((x, y))
    }

    /// Advances by `dt` towards the heading `target` (view points) and returns the frame's
    /// motion. `now` dates the survey.
    pub fn step(&mut self, dt: f64, target: (f64, f64), now: f64) -> Motion {
        let running = self.active && !self.paused;
        self.target = if running { target } else { (0.0, 0.0) };

        let render_hint = self.update_throttle(dt);
        let fb = self.feedback;

        // Safety: the heading region of the current frame.
        let (mut safety_target, mut safety_hint) = (1.0, Hint::None);
        let assessment = self.assessment(now);
        if let Some(a) = assessment {
            let danger = a.danger();
            safety_target = 1.0 - (1.0 - SAFETY_MIN) * danger;
            if danger > 0.5 {
                safety_hint = if a.interior > 0.5 {
                    Hint::Interior
                } else {
                    Hint::Featureless
                };
            }
        }
        let tau = if safety_target < self.safety {
            THROTTLE_TAU.0
        } else {
            THROTTLE_TAU.1
        };
        self.safety = approach(self.safety, safety_target, tau, dt);
        if fb.at_limit {
            self.safety = 0.0;
        }

        // Rescue: nothing ahead and nothing to aim at while the user lets it fly.
        let uniform = match assessment {
            Some(a) => a.danger() > 0.5,
            None => self.lock.view_is_void(),
        };
        if running && self.free && self.lock.is_enabled() {
            self.uniform_for = if uniform { self.uniform_for + dt } else { 0.0 };
            let switches = self.lock.switches;
            self.target_uniform = match self.lock.target() {
                Some(_) if uniform && self.target_uniform.1 == switches => (self.target_uniform.0 + dt, switches),
                Some(_) if uniform => (dt, switches),
                _ => (0.0, switches),
            };
            // Survey steering heads for detail the GPU shows: as patient as a target.
            let locked = self.lock.target().is_some() || self.survey_steering;
            if !self.rescue && ((!locked && self.uniform_for > RESCUE_AFTER) || self.uniform_for > RESCUE_AFTER_LOCKED)
            {
                self.rescue = true;
                self.lock.drop_target();
            }
            // Detail in the survey's heading region for a while ends it too, target or not: the
            // CPU probes of the search cannot resolve views near very high-period minibrots
            // (iteration cap), which the GPU shows in full.
            self.clear_for = if assessment.is_some() && !uniform {
                self.clear_for + dt
            } else {
                0.0
            };
            if self.rescue
                && (self.lock.target().is_some()
                    || self.clear_for > RESCUE_CLEAR
                    || self.span_ln >= RESCUE_MAX_SPAN.ln())
            {
                self.rescue = false;
                // Without a target, aim once at the survey's most detailed cell (next frame).
                self.survey_pick = self.lock.target().is_none() && !self.survey_steer;
                self.uniform_for = 0.0;
            }
        } else {
            self.rescue = false;
            self.uniform_for = 0.0;
            self.target_uniform.0 = 0.0;
        }

        self.hint = if !running {
            Hint::None
        } else if self.rescue {
            Hint::Rescue
        } else if fb.at_limit {
            Hint::Limit
        } else if render_hint != Hint::None {
            render_hint
        } else if self.safety < 0.85 {
            safety_hint
        } else {
            Hint::None
        };

        // Zoom rate eased towards speed x throttle x safety; the heading; the pan that brings
        // it to the centre (faster with the zoom rate, slowed with the renderer). The pan rate
        // depends on the zoom rate, so their product is integrated in fixed substeps of at
        // most 2 ms: any frame rate gives the same flight to second order.
        let rate_target = if running && self.rescue {
            // Backing out of a long void speeds up (deep inside a big interior).
            let longer = 1.0 + (self.uniform_for - RESCUE_AFTER).max(0.0) / RESCUE_ACCEL;
            let base = (RESCUE_SHARE * self.speed).clamp(RESCUE_RATE.0, RESCUE_RATE.1);
            -(base * longer).min(RESCUE_RATE_MAX) * self.throttle
        } else if running {
            self.speed * self.throttle * self.safety * self.capture()
        } else {
            0.0
        };
        // At most 1000 substeps (2 s): an infinite dt was usize::MAX of them
        // (kb: techniques/hang-prevention).
        let n = if dt.is_nan() {
            1.0
        } else {
            (dt / 0.002).ceil().clamp(1.0, 1000.0)
        };
        let h = dt / n;
        let (mut ln_zoom, mut pan, mut at) = (0.0, (0.0, 0.0), (0.0, 0.0));
        for _ in 0..n as usize {
            let omega = if rate_target > self.rate.v {
                RATE_OMEGA.0
            } else {
                RATE_OMEGA.1
            };
            let dz = self.rate.follow(rate_target, omega, h);
            let omega = if self.lock_aiming { LOCK_OMEGA } else { STEER_OMEGA };
            let ix = self.heading[0].follow(self.target.0, omega, h);
            let iy = self.heading[1].follow(self.target.1, omega, h);
            let k = (STEER_PAN + STEER_PAN_PER_RATE * (dz / h).max(0.0)) * self.throttle;
            ln_zoom += dz;
            pan.0 -= k * ix;
            pan.1 -= k * iy;
            // The zoom's centre: the heading weighted by the zoom it carries.
            at.0 += ix * dz / h;
            at.1 += iy * dz / h;
        }
        let at = if ln_zoom.abs() > 1e-15 {
            (at.0 / ln_zoom, at.1 / ln_zoom)
        } else {
            self.heading()
        };
        if !running && self.rate.v.abs() < 1e-3 && self.rate.a.abs() < 1e-3 * RATE_OMEGA.1 {
            self.rate = Spring::default();
        }
        if !running
            && self
                .heading
                .iter()
                .all(|s| s.v.abs() < 0.05 && s.a.abs() < 0.05 * STEER_OMEGA)
        {
            self.heading = [Spring::default(); 2];
        }
        Motion { pan, ln_zoom, at }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform(interior: u32, edges: u32) -> Survey {
        Survey {
            cols: 16,
            rows: 10,
            cells: vec![
                SurveyCell {
                    samples: 100,
                    interior,
                    pairs: 200,
                    edges,
                    ..Default::default()
                };
                160
            ],
            view: (1600.0, 1000.0),
            texels_across: 400.0,
            texels_down: 250.0,
            time: 0.0,
            viewport: None,
        }
    }

    #[test]
    fn steering_dead_zone_and_response() {
        let a = Autopilot::default();
        let r = 500.0;
        // Inside the dead zone: straight ahead.
        assert_eq!(a.steer_target(Some((40.0, -20.0)), r), (0.0, 0.0));
        assert_eq!(a.steer_target(None, r), (0.0, 0.0));
        // Monotonic and continuous past it, full deflection at the radius, direction kept.
        let mut last = 0.0;
        for k in 11..=100 {
            let x = r * k as f64 / 100.0;
            let (tx, ty) = a.steer_target(Some((x * 0.6, x * 0.8)), r);
            let m = tx.hypot(ty);
            assert!(m >= last && ty / tx > 1.33 && ty / tx < 1.34);
            last = m;
        }
        assert!((last - r).abs() < 1e-9);
        // Beyond the radius: clamped.
        let (tx, _) = a.steer_target(Some((3.0 * r, 0.0)), r);
        assert!((tx - r).abs() < 1e-9);
        // Just past the dead zone the response is tiny (small mouse moves barely steer).
        let (tx, _) = a.steer_target(Some((0.12 * r, 0.0)), r);
        assert!(tx < 0.01 * r, "{tx}");
    }

    /// The rate under a fit that is a function of time, at `hz`: (min, max) after `settle` s.
    fn rate_under(hz: f64, secs: f64, settle: f64, fit: impl Fn(f64) -> f64) -> (f64, f64, Autopilot) {
        let mut a = Autopilot::default();
        a.toggle();
        let dt = 1.0 / hz;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for k in 0..(secs * hz).round() as usize {
            let t = (k + 1) as f64 * dt;
            a.set_feedback(RenderFeedback {
                fit: fit(t),
                ..Default::default()
            });
            a.step(dt, (0.0, 0.0), t);
            if t >= settle {
                lo = lo.min(a.rate());
                hi = hi.max(a.rate());
            }
        }
        (lo, hi, a)
    }

    #[test]
    fn throttle_is_steady_while_frames_fit_and_drops_on_a_real_overload() {
        // The fits a 120 Hz flight at 1e-250 reads (measured): ~0.13 at the 1/8 step, ~0.24
        // while a probe holds 1/4, toggling every 0.5-4 s; the old mapping swung the rate
        // between 0.36 and 0.95 of the speed with them. Plus a slow chunk (fit 0.05 for 30 ms)
        // twice a second. All frames fit: the rate stays at the speed.
        let toggling = |t: f64| {
            let base = if (t / 1.7).floor() as i64 % 2 == 0 { 0.13 } else { 0.24 };
            if t.fract() < 0.03 || (t + 0.5).fract() < 0.03 {
                0.05
            } else {
                base
            }
        };
        for hz in [30.0, 60.0, 120.0] {
            let (lo, hi, _) = rate_under(hz, 20.0, 5.0, toggling);
            assert!(lo > 0.97 * SPEED_DEFAULT, "{hz} Hz: rate dips to {lo}");
            assert!(hi <= SPEED_DEFAULT + 1e-9, "{hz} Hz: {hi}");
        }
        // A real overload (the coarsest preview takes 4x the budget) slows it within ~1.5 s,
        // the same at any frame rate.
        let overload = |t: f64| if t < 5.0 { 0.13 } else { 0.06 };
        let at = |hz: f64, secs: f64| rate_under(hz, secs, 0.0, overload).2;
        for hz in [30.0, 60.0, 120.0] {
            let a = at(hz, 6.5);
            assert!(a.rate() < 0.5 * SPEED_DEFAULT, "{hz} Hz: still {}", a.rate());
            assert_eq!(a.hint(), Hint::Rendering);
            let a = at(hz, 9.0);
            assert!((a.throttle() - THROTTLE_MIN).abs() < 0.02, "{hz} Hz: {}", a.throttle());
        }
        // Mid-transient (falling at ~0.5 e-folds/s²): within one 30 Hz frame of the step's timing.
        let (r30, r120) = (at(30.0, 6.0).rate(), at(120.0, 6.0).rate());
        assert!((r30 - r120).abs() < 0.04, "30 Hz {r30} vs 120 Hz {r120}");
    }

    #[test]
    fn throttle_follows_the_renderer() {
        let mut a = Autopilot::default();
        a.toggle();
        let mut t = 0.0;
        let mut run = |a: &mut Autopilot, secs: f64| {
            for _ in 0..(secs * 60.0) as usize {
                t += 1.0 / 60.0;
                a.step(1.0 / 60.0, (0.0, 0.0), t);
            }
        };
        run(&mut a, 5.0);
        assert!((a.rate() - SPEED_DEFAULT).abs() < 1e-4, "{}", a.rate());
        assert_eq!(a.hint(), Hint::None);
        // Previews that fit only at 1/16 resolution: down to the floor, with the hint.
        a.set_feedback(RenderFeedback {
            fit: 0.06,
            ..Default::default()
        });
        run(&mut a, 2.0);
        assert!((a.throttle() - THROTTLE_MIN).abs() < 0.01);
        assert!((a.rate() - SPEED_DEFAULT * THROTTLE_MIN).abs() < 0.02);
        assert_eq!(a.hint(), Hint::Rendering);
        // Waiting for a reference: almost stopped.
        a.set_feedback(RenderFeedback {
            fit: 0.5,
            waiting_reference: true,
            at_limit: false,
        });
        run(&mut a, 2.0);
        assert!(a.rate() < 0.1, "{}", a.rate());
        assert_eq!(a.hint(), Hint::Reference);
        // Caught up: resumes gently (τ = 1 s), back to full speed.
        a.set_feedback(RenderFeedback {
            fit: 0.5,
            ..Default::default()
        });
        run(&mut a, 0.5);
        assert!(a.rate() < 0.5 * SPEED_DEFAULT, "resumed too fast: {}", a.rate());
        run(&mut a, 6.0);
        assert!((a.rate() - SPEED_DEFAULT).abs() < 0.01);
        assert_eq!(a.hint(), Hint::None);
    }

    #[test]
    fn uniform_heading_slows_down_with_a_hint() {
        let mut a = Autopilot::default();
        a.toggle();
        let mut t = 0.0;
        for (survey, hint) in [
            (uniform(100, 0), Hint::Interior),
            (uniform(0, 1), Hint::Featureless),
            (uniform(0, 40), Hint::None),
        ] {
            for _ in 0..360 {
                t += 1.0 / 60.0;
                let mut s = survey.clone();
                s.time = t;
                a.set_survey(s);
                a.step(1.0 / 60.0, (0.0, 0.0), t);
            }
            assert_eq!(a.hint(), hint);
            let expect = if hint == Hint::None { 1.0 } else { SAFETY_MIN };
            assert!((a.safety() - expect).abs() < 0.03, "{hint:?}: {}", a.safety());
        }
        // A stale survey no longer counts.
        assert!(a.assessment(t + 1.0).is_none());
    }

    /// Flies `secs` at 60 Hz with `pointer`, the survey re-dated every frame; returns the
    /// lowest zoom rate seen.
    fn fly(
        a: &mut Autopilot,
        v: &mut Viewport,
        pointer: Option<(f64, f64)>,
        secs: f64,
        t: &mut f64,
        s: &Survey,
    ) -> f64 {
        let mut lowest = f64::INFINITY;
        for _ in 0..(secs * 60.0).round() as usize {
            *t += 1.0 / 60.0;
            let mut s = s.clone();
            s.time = *t;
            a.set_survey(s);
            let target = a.aim(v, 600.0, 300.0, pointer, false, *t);
            let m = a.step(1.0 / 60.0, target, *t);
            v.pan_by_points(m.pan.0, m.pan.1);
            v.zoom_by(m.ln_zoom.exp(), m.at.0, m.at.1);
            lowest = lowest.min(a.rate());
        }
        lowest
    }

    #[test]
    fn rescue_backs_out_of_a_void_and_never_fights_the_pointer() {
        // Deep inside the main cardioid: the survey says interior, the search finds no boundary.
        let mut a = Autopilot::default();
        a.lock_mut().synchronous = true;
        a.toggle();
        let mut v = Viewport::new(-0.1, 0.0, 1e-6);
        let void = uniform(100, 0);
        let mut t = 0.0;
        fly(&mut a, &mut v, None, 0.8, &mut t, &void);
        assert!(!a.is_rescuing() && a.rate() > 0.0, "not before {RESCUE_AFTER} s");
        assert!(a.lock().view_is_void());
        fly(&mut a, &mut v, None, 1.7, &mut t, &void);
        assert!(a.is_rescuing() && a.rate() < 0.0, "rate {}", a.rate());
        assert_eq!(a.hint(), Hint::Rescue);
        // A deflected pointer wins at once: no rescue, the zoom eases forward again.
        fly(&mut a, &mut v, Some((200.0, 0.0)), 1.0 / 60.0, &mut t, &void);
        assert!(!a.is_rescuing());
        fly(&mut a, &mut v, Some((200.0, 0.0)), 1.5, &mut t, &void);
        assert!(a.rate() > 0.0 && a.lock().target().is_none());
        // Let go: the void is found again, the zoom backs out until a search finds a target.
        let ln0 = v.scale.ln();
        let mut found = false;
        for _ in 0..(40 * 60) {
            fly(&mut a, &mut v, None, 1.0 / 60.0, &mut t, &void);
            if a.lock().target().is_some() {
                found = true;
                break;
            }
        }
        assert!(found, "no target after backing out to scale {}", v.scale.to_f64());
        assert!(!a.is_rescuing());
        assert!(
            v.scale.ln() - ln0 > 3.0,
            "it zoomed out: {} e-folds",
            v.scale.ln() - ln0
        );
        assert!(lock::view_span(&v, 600.0).to_f64() < RESCUE_MAX_SPAN);
    }

    /// A 960 × 600 view surveyed as interior except cell (12, 2) (centre (270, 150)): detailed,
    /// half escaped, its peak at (260, 140).
    fn one_detailed_cell(viewport: Option<Viewport>, time: f64) -> Survey {
        let mut s = uniform(100, 0);
        s.view = (960.0, 600.0);
        s.viewport = viewport;
        s.time = time;
        s.cells[2 * 16 + 12] = SurveyCell {
            samples: 100,
            interior: 50,
            pairs: 200,
            edges: 100,
            peak: (260.0, 140.0),
            peak_n: 5000.0,
        };
        s
    }

    #[test]
    fn boundary_point_prefers_detail_near_the_heading_else_uphill() {
        let mut s = one_detailed_cell(None, 0.0);
        assert_eq!(s.boundary_point((0.0, 0.0), None), Some((260.0, 140.0)));
        // Two detailed cells: the one nearer `from` wins (hysteresis: the cell containing it).
        s.cells[7 * 16 + 2] = SurveyCell {
            peak: (-350.0, -150.0),
            ..s.cells[2 * 16 + 12]
        };
        assert_eq!(s.boundary_point((-300.0, -100.0), None), Some((-350.0, -150.0)));
        assert_eq!(s.boundary_point((200.0, 100.0), None), Some((260.0, 140.0)));
        // Featureless exterior: no detailed cell, the highest escape count (uphill).
        let mut flat = uniform(0, 0);
        for (k, c) in flat.cells.iter_mut().enumerate() {
            c.peak = (k as f64, -(k as f64));
            c.peak_n = if k == 37 { 900.0 } else { 100.0 + k as f64 };
        }
        assert_eq!(flat.boundary_point((0.0, 0.0), None), Some((37.0, -37.0)));
        // All interior: nothing to aim at.
        assert_eq!(uniform(100, 0).boundary_point((0.0, 0.0), None), None);
    }

    /// Flies `secs` at `hz` with survey steering, the survey taken once at the start view.
    fn steer_flight(hz: f64, secs: f64) -> (Autopilot, Viewport) {
        let mut a = Autopilot::default();
        a.set_survey_steer(true);
        a.lock_mut().synchronous = true;
        a.toggle();
        let mut v = Viewport::new(-0.1, 0.0, 1e-6);
        a.set_survey(one_detailed_cell(Some(v.clone()), 0.0));
        let dt = 1.0 / hz;
        for k in 1..=(secs * hz).round() as usize {
            let t = k as f64 * dt;
            let target = a.aim(&v, 600.0, 300.0, None, false, t);
            let m = a.step(dt, target, t);
            v.pan_by_points(m.pan.0, m.pan.1);
            v.zoom_by(m.ln_zoom.exp(), m.at.0, m.at.1);
        }
        (a, v)
    }

    #[test]
    fn survey_steering_brings_the_boundary_point_to_the_centre_at_any_frame_rate() {
        let start = Viewport::new(-0.1, 0.0, 1e-6);
        let peak = start.point_at_offset(260.0, 140.0);
        let mut ends = Vec::new();
        for hz in [30.0, 60.0, 120.0] {
            let (a, v) = steer_flight(hz, 2.0);
            assert!(a.is_survey_steering() && a.lock().target().is_none() && !a.is_rescuing());
            assert_eq!(a.lock_label(), SURVEY_STEER_LABEL);
            // The peak moved from 297 points out to near the centre while the view dove.
            let (x, y) = v.offset_of(&peak);
            assert!(x.hypot(y) < 60.0, "{hz} Hz: peak at ({x:.1}, {y:.1})");
            assert!(v.scale.to_f64() < 0.5e-6, "it zoomed in: {}", v.scale.to_f64());
            ends.push((v.clone(), (x, y)));
        }
        // 30 and 120 Hz agree with 60 Hz to a few points of where the peak ends up, and in zoom
        // to 0.02 e-folds (as the lock: the capture slow-down is set once per frame).
        for (v, (x, y)) in &ends {
            let (x0, y0) = ends[1].1;
            assert!((x - x0).hypot(y - y0) < 3.0, "({x:.2}, {y:.2}) vs ({x0:.2}, {y0:.2})");
            assert!(
                (v.scale.ln() - ends[1].0.scale.ln()).abs() < 0.02,
                "{} vs {}",
                v.scale.ln(),
                ends[1].0.scale.ln()
            );
        }
    }

    #[test]
    fn lock_target_and_pointer_win_over_survey_steering() {
        let (mut a, mut v) = steer_flight(60.0, 1.0);
        assert!(a.is_survey_steering());
        let mut t = 1.0;
        let s = one_detailed_cell(Some(v.clone()), t);
        // A deflected pointer wins at once: its heading alone.
        fly(&mut a, &mut v, Some((250.0, 0.0)), 1.0 / 60.0, &mut t, &s);
        assert!(!a.is_survey_steering() && !a.is_free());
        assert_eq!(a.heading_target(), a.steer_target(Some((250.0, 0.0)), 300.0));
        // Let go: survey steering again; a lock target takes over.
        fly(&mut a, &mut v, None, 0.1, &mut t, &s);
        assert!(a.is_survey_steering());
        a.lock_mut().set_target(lock::LockTarget {
            kind: crate::journey::TargetKind::Spiral,
            center: v.point_at_offset(-100.0, 50.0),
            end_span: lock::view_span(&v, 600.0) * (-5.0f64).exp(),
            label: "spiral".into(),
            score: 0.0,
            period: None,
            preperiod: None,
        });
        fly(&mut a, &mut v, None, 1.0 / 60.0, &mut t, &s);
        assert!(!a.is_survey_steering() && a.lock().target().is_some());
        assert_eq!(a.lock_label(), "towards spiral");
        // Survey steering off: no survey aim at all (straight ahead without a target).
        let (b, _) = {
            let mut b = Autopilot::default();
            b.set_survey_steer(false);
            b.lock_mut().synchronous = true;
            b.toggle();
            let mut w = Viewport::new(-0.1, 0.0, 1e-6);
            let mut t = 0.0;
            fly(&mut b, &mut w, None, 0.5, &mut t, &one_detailed_cell(None, 0.0));
            (b, w)
        };
        assert!(!b.is_survey_steering() && b.heading_target() == (0.0, 0.0));
    }

    /// A 960 × 600 interior survey with `n` detailed cells at pseudo-random places (seed `k`).
    fn scattered(k: u64, n: usize) -> Survey {
        let mut s = uniform(100, 0);
        s.view = (960.0, 600.0);
        let mut x = k.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        for _ in 0..n {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let (i, j) = ((x >> 33) as usize % 16, (x >> 45) as usize % 10);
            let (cx, cy) = s.cell_center(i, j);
            s.cells[j * 16 + i] = SurveyCell {
                samples: 100,
                interior: 40,
                pairs: 200,
                edges: 60 + (x >> 20) as u32 % 100,
                peak: (cx + 5.0, cy - 5.0),
                peak_n: 5000.0,
            };
        }
        s
    }

    /// Kb: decisions/controls "Pointer wish": survey steering's boundary point lies in the
    /// wished half whenever detail is there; without a wish the pick is the old one.
    #[test]
    fn survey_boundary_point_follows_the_wish() {
        let mut checked = 0;
        for k in 0..20 {
            let s = scattered(k, 6);
            for dir in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let w = Wish { dir, epoch: 1 };
                let side = 600.0;
                let any = s.cells.iter().any(|c| {
                    c.has_peak() && s.cell_detail(c) >= DETAIL_HI && w.agreement(c.peak, WISH_NEAR * side) > 0.0
                });
                let Some(p) = s.boundary_point((0.0, 0.0), Some(&w)) else {
                    panic!("survey {k}: no pick");
                };
                if any {
                    checked += 1;
                    assert!(p.0 * dir.0 + p.1 * dir.1 > 0.0, "survey {k}, wish {dir:?}: {p:?}");
                }
            }
        }
        assert!(checked >= 60, "{checked}");
        // Featureless: uphill within the wished half.
        let mut flat = uniform(0, 0);
        flat.view = (960.0, 600.0);
        for j in 0..10 {
            for i in 0..16 {
                let c = flat.cell_center(i, j);
                flat.cells[j * 16 + i].peak = c;
                // Highest on the left.
                flat.cells[j * 16 + i].peak_n = 1000.0 - c.0;
            }
        }
        assert!(flat.boundary_point((0.0, 0.0), None).unwrap().0 < 0.0);
        let right = Wish {
            dir: (1.0, 0.0),
            epoch: 1,
        };
        assert!(flat.boundary_point((0.0, 0.0), Some(&right)).unwrap().0 > 0.0);
    }

    #[test]
    fn wish_settles_with_hysteresis() {
        let mut a = Autopilot::default();
        a.toggle();
        let r = 300.0;
        let mut t = 0.0;
        let mut hold = |a: &mut Autopilot, p: Option<(f64, f64)>, secs: f64, hz: f64| {
            for _ in 0..(secs * hz).round() as usize {
                t += 1.0 / hz;
                a.set_wish(p, r, t);
            }
        };
        // Inside the dead zone: none.
        hold(&mut a, Some((50.0, 0.0)), 1.0, 60.0);
        assert!(a.wish().is_none());
        // Deflected: not before WISH_SETTLE, then a wish (epoch 1).
        hold(&mut a, Some((200.0, 0.0)), 0.2, 60.0);
        assert!(a.wish().is_none());
        hold(&mut a, Some((200.0, 0.0)), 0.15, 60.0);
        let w = a.wish().expect("wish");
        assert_eq!((w.dir, w.epoch), ((1.0, 0.0), 1));
        // A wobble of the hand (30°) and a brief swing to the other side change nothing.
        hold(&mut a, Some((200.0, 115.0)), 1.0, 120.0);
        hold(&mut a, Some((-200.0, 0.0)), 0.2, 30.0);
        hold(&mut a, Some((200.0, 10.0)), 0.5, 60.0);
        assert_eq!(a.wish().map(|w| (w.dir, w.epoch)), Some(((1.0, 0.0), 1)));
        // Between the dead zone's two edges it stays.
        hold(&mut a, Some((51.0, 0.0)), 1.0, 60.0);
        assert!(a.wish().is_some());
        // A real turn: a new epoch after WISH_SETTLE.
        hold(&mut a, Some((0.0, -200.0)), 0.4, 60.0);
        let w = a.wish().expect("wish");
        assert_eq!(w.epoch, 2);
        assert!(w.dir.1 < -0.99);
        // Let go (centre): cleared without a new epoch.
        hold(&mut a, Some((10.0, 0.0)), 0.4, 60.0);
        assert!(a.wish().is_none());
        // Off: nothing.
        a.stop();
        hold(&mut a, Some((200.0, 0.0)), 1.0, 60.0);
        assert!(a.wish().is_none());
    }

    /// Flies `secs` at `hz` from `v` with the pointer held at `pointer` (wish), the lock
    /// synchronous, no survey.
    fn wish_flight(a: &mut Autopilot, v: &mut Viewport, pointer: (f64, f64), secs: f64, t: &mut f64, hz: f64) {
        for _ in 0..(secs * hz).round() as usize {
            *t += 1.0 / hz;
            a.set_wish(Some(pointer), 300.0, *t);
            let target = a.aim(v, 600.0, 300.0, None, false, *t);
            let m = a.step(1.0 / hz, target, *t);
            v.pan_by_points(m.pan.0, m.pan.1);
            v.zoom_by(m.ln_zoom.exp(), m.at.0, m.at.1);
        }
    }

    /// A wish to the right locks a target on the right; turning the wish left re-targets once
    /// to a target on the left; the autopilot never follows the pointer itself.
    #[test]
    fn a_changed_wish_retargets_once() {
        for hz in [30.0, 60.0, 120.0] {
            let mut a = Autopilot::default();
            a.lock_mut().synchronous = true;
            a.toggle();
            let mut v = Viewport::new(-0.745, 0.11, 0.01 / 600.0);
            let mut t = 0.0;
            wish_flight(&mut a, &mut v, (250.0, 0.0), 0.5, &mut t, hz);
            let tr = a.lock().target().expect("target").clone();
            assert!(
                v.offset_of(&tr.center).0 > 0.0,
                "{hz} Hz: {:?}",
                v.offset_of(&tr.center)
            );
            assert_eq!(a.lock_label(), format!("towards {} · {WISH_LABEL}", tr.label));
            // (The first target was picked up before the wish settled; it re-targeted then.)
            let (switches, re) = (a.lock().switches, a.lock().retargets);
            wish_flight(&mut a, &mut v, (-250.0, 0.0), 0.6, &mut t, hz);
            let tl = a.lock().target().expect("target").clone();
            let (x, _) = v.offset_of(&tl.center);
            assert!(x < 0.0, "{hz} Hz: {x}");
            assert_eq!(a.lock().retargets, re + 1, "{hz} Hz");
            assert!(a.lock().switches <= switches + 1, "{hz} Hz: one switch");
            // Holding it left: no further re-targets (hysteresis).
            wish_flight(&mut a, &mut v, (-240.0, 30.0), 1.0, &mut t, hz);
            assert_eq!(a.lock().retargets, re + 1, "{hz} Hz");
            // The heading aims at the target, not at the pointer.
            assert_ne!(a.heading_target(), a.steer_target(Some((-240.0, 30.0)), 300.0));
        }
    }

    #[test]
    fn click_pauses_and_double_click_undoes_it() {
        let mut a = Autopilot::default();
        assert!(!a.click(0.0), "clicks are not used while off");
        a.toggle();
        assert!(a.click(1.0) && a.is_paused());
        a.double_click(1.2);
        assert!(!a.is_paused());
        assert!(a.click(3.0) && a.is_paused());
        a.double_click(4.0);
        assert!(a.is_paused(), "too late to be the same double click");
        // Speed limits.
        a.scale_speed(100.0, 5.0);
        assert_eq!(a.speed(), SPEED_RANGE.1);
        a.scale_speed(1e-6, 5.0);
        assert_eq!(a.speed(), SPEED_RANGE.0);
        assert!(a.stop() && !a.is_active());
    }
}
