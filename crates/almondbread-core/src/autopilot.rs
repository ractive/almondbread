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
//! `speed · throttle · safety`. `throttle` (0...1) follows how the renderer keeps up: the
//! gesture-preview resolution that fits the frame budget, and whether a deep reference is
//! still computing (the view then only shows the stretched history). `safety` drops when the
//! heading region of the current frame is uniform (fully interior or featureless exterior);
//! the HUD then shows a hint.
//!
//! **Target lock** ([`lock`], on by default): while the user does not steer (pointer in the
//! dead zone, idle or off the canvas, keyboard mode without pan keys), the heading aims at a
//! boundary target found off-thread (a Misiurewicz point, an embedded Julia stage, a minibrot
//! down to its framing), and the next one is found before the current one's depth is passed.
//! A deflected pointer (or any manual pan) always wins and releases the lock; letting go
//! picks up again from the current view.
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
/// 1, and at which it reaches its floor.
pub const FIT_FULL: f64 = 0.25;
pub const FIT_FLOOR: f64 = 0.1;
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
/// Manual pans, zooms and drags hold the lock off for this long (s).
pub const MANUAL_HOLD: f64 = 1.0;
/// A survey older than this (s) is ignored.
pub const SURVEY_STALE: f64 = 0.6;

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurveyCell {
    /// Sampled texels (pending ones are skipped).
    pub samples: u32,
    /// Interior or unresolved (drawn as interior) samples.
    pub interior: u32,
    /// Neighbour pairs compared, and those whose class or integer escape count differ.
    pub pairs: u32,
    pub edges: u32,
}

/// A coarse picture of a frame: `cols` x `rows` cells in raster order from the top-left.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Survey {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<SurveyCell>,
    /// Size of the surveyed view in view points.
    pub view: (f64, f64),
    /// Texels across the surveyed level (band crossings per view width = edge rate × this).
    pub texels_across: f64,
    /// When it was taken (the navigator's clock).
    pub time: f64,
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
            safety: 1.0,
            hint: Hint::None,
            feedback: RenderFeedback::default(),
            survey: None,
            lock: Lock::default(),
            free: false,
            uniform_for: 0.0,
            rescue: false,
            span_ln: f64::NEG_INFINITY,
            lock_aiming: false,
            aim_far: 0.0,
            last_click: f64::NEG_INFINITY,
            target: (0.0, 0.0),
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
        match self.lock.target() {
            // Journey labels read "period-34 minibrot", "towards a period-34 minibrot", ...
            Some(t) if t.label.starts_with("towards") => t.label.clone(),
            Some(t) => format!("towards {}", t.label),
            None if self.lock.is_searching() => "looking for a target".into(),
            None => String::new(),
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
        let (target, hint) = if fb.waiting_reference {
            (THROTTLE_WAITING, Hint::Reference)
        } else if fb.fit.is_finite() {
            let t = smoothstep((fb.fit - FIT_FLOOR) / (FIT_FULL - FIT_FLOOR));
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
        let running = self.active && !self.paused;
        if !running {
            if !self.active {
                self.lock.release();
            }
            return steer;
        }
        if radius <= 0.0 {
            return steer;
        }
        let heading = self.heading();
        let Some((px, py)) = self.lock.update(view, min_side, radius, self.free, heading, now) else {
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
            let locked = self.lock.target().is_some();
            if !self.rescue && ((!locked && self.uniform_for > RESCUE_AFTER) || self.uniform_for > RESCUE_AFTER_LOCKED)
            {
                self.rescue = true;
                self.lock.drop_target();
            }
            if self.rescue && (self.lock.target().is_some() || self.span_ln >= RESCUE_MAX_SPAN.ln()) {
                self.rescue = false;
                self.uniform_for = 0.0;
            }
        } else {
            self.rescue = false;
            self.uniform_for = 0.0;
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
        let n = (dt / 0.002).ceil().max(1.0);
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
                };
                160
            ],
            view: (1600.0, 1000.0),
            texels_across: 400.0,
            time: 0.0,
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
