//! Fly to coordinates: an animated, smooth and efficient zoom-pan path from the current view
//! to a target view at any depth (kb: decisions/controls, "Fly to"; techniques path after van
//! Wijk & Nuij, "Smooth and efficient zooming and panning", InfoVis 2003).
//!
//! **Path.** In van Wijk & Nuij's (u, w) space (u: position along the line from the start
//! centre to the target, w: view width) the optimal path for the metric with parameter ρ is
//!
//! ```text
//! b_i = (w1² − w0² ± ρ⁴ d²) / (2 w_i ρ² d)      (+ for i = 0, − for i = 1)
//! r_i = ln(−b_i + √(b_i² + 1)) = −asinh(b_i)
//! S   = (r1 − r0) / ρ
//! w(s) = w0 cosh r0 / cosh(ρ s + r0)
//! u(s) = w0/ρ² · (cosh r0 · tanh(ρ s + r0) − sinh r0)
//! ```
//!
//! zooming out as far as the distance needs, panning, and zooming in; at its ends it is a pure
//! zoom at `d ln w / ds = ±ρ`. Deep views break f64 (w1 / w0 may be 10^-3000), so the path is
//! cut into a *head* (a pure zoom out about the start centre until the view is 10^-6 of the
//! distance), the van Wijk part between widths f64 holds comfortably, and a *tail* (a pure zoom
//! in about the target from 10^-6 of the distance down to the target width). The head and tail
//! zoom at exactly the van Wijk end rate ρ per unit of s, and the van Wijk part's centre speed
//! there is ~10^-6 view widths per unit, so the joins are smooth to the eye. Everything is in
//! log space where it matters: `ln w` and the remaining offset `d − u(s)` come from
//! cancellation-free forms (`tanh a − tanh b = sinh(a − b) / (cosh a cosh b)`), so the target's
//! screen position is continuous and the path ends *exactly* on the target: every frame's view
//! is computed from the path parameter (centre = target − offset, in arbitrary precision), not
//! integrated, so nothing drifts.
//!
//! **Motion.** The path parameter advances at `speed / ρ · throttle` (speed in e-folds per
//! second: the zoom rate at the ends; Q / E, scroll), eased in and out on a spring. Speeds are
//! capped ahead of the van Wijk part's apex (the turn from zooming out to zooming in:
//! [`FlyPath::turn_limit`], [`TURN_TIME`]), and the flight lands on a curve whose speed falls
//! as `(1 − t/T)²` ([`BRAKE_TIME`], rate-proportional). `throttle` is the autopilot's renderer
//! back-off (preview resolution, deep reference computing).

use crate::input::Spring;
use crate::viewport::{Center, Scale, Viewport};

/// The path's zoom/pan trade-off; van Wijk & Nuij found ρ ≈ 1.42 (√2) best in user tests.
pub const RHO: f64 = std::f64::consts::SQRT_2;
/// Default flight speed (e-folds per second at the zoom ends: ×7.4 per second) and range.
pub const SPEED_DEFAULT: f64 = 2.0;
pub const SPEED_RANGE: (f64, f64) = (0.1, 16.0);
/// Spring frequencies (1/s) of the path speed: speeding up, slowing down (Esc).
pub const PATH_OMEGA: (f64, f64) = (3.0, 4.0);
/// Braking (onto the target, and into the turn of a path that zooms out and back in) takes
/// `BRAKE_TIME.0 · √(speed / BRAKE_REF)` seconds, within `BRAKE_TIME`: rate-proportional, so a
/// fast flight does not stop dead (a fixed 0.75 s brake at the constant deceleration
/// `v = √(2 a left)` stopped journey legs from 25 e-folds/s within 0.1 s, the spring reaching
/// the end still moving and snapping; kb: decisions/controls "Journey", smooth legs). A fixed
/// deceleration instead made fast flights brake for most of a short path (measured 4.6 s for
/// 15 e-folds at 12 e-folds/s).
pub const BRAKE_TIME: (f64, f64) = (0.8, 1.5);
pub const BRAKE_REF: f64 = 1.4;
/// The landing rides `v = c · left^(2/3)` (the speed falls as `(1 − t/T)²`: the deceleration
/// eases to zero at the target instead of ending abruptly).
const LAND_POWER: f64 = 2.0 / 3.0;
/// The turn of a zoom-out-and-in path (its apex) takes at least this long (s) for the zoom
/// rate to swing from zooming out to zooming in (between x = −1 and 1 of the van Wijk part),
/// whatever the speed: a cross-over at a raised journey speed swung from −28 to +23 e-folds/s
/// in 0.15 s before.
pub const TURN_TIME: f64 = 1.0;
/// A soft landing (the zoom-out autopilot, kb: decisions/controls "Zoom-out autopilot"): the
/// deceleration stops the set speed in this many seconds, so the last e-fold or so eases out
/// instead of braking hard.
pub const SOFT_LAND_TIME: f64 = 2.5;
/// Within this many path units of the end the flight snaps onto the target.
pub const LAND_SNAP: f64 = 1e-4;
/// The head and tail start where the view is this fraction of the distance (or of the start
/// width, for targets inside the view).
const SPLIT: f64 = 1e-6;
/// Targets closer than this fraction of the view width are zoomed to straight.
const PURE: f64 = 1e-100;

/// Seconds a flight at `speed` e-folds per second takes to brake (see [`BRAKE_TIME`]).
pub fn brake_time(speed: f64) -> f64 {
    let s = if speed.is_finite() { speed.max(0.0) } else { 0.0 };
    (BRAKE_TIME.0 * (s / BRAKE_REF).sqrt()).clamp(BRAKE_TIME.0, BRAKE_TIME.1)
}

/// `ln cosh x`, without overflow.
fn ln_cosh(x: f64) -> f64 {
    let a = x.abs();
    a + (-2.0 * a).exp().ln_1p() - std::f64::consts::LN_2
}

/// `ln sinh x` for x >= 0 (−inf at 0), without overflow.
fn ln_sinh(x: f64) -> f64 {
    if x < 20.0 {
        x.sinh().ln()
    } else {
        x - std::f64::consts::LN_2 + (-(-2.0 * x).exp()).ln_1p()
    }
}

#[derive(Clone, Debug)]
enum Kind {
    /// Straight zoom (the target is on the centre line to within `PURE`): the offset decays
    /// with (w / w0)² zooming in, linearly zooming out.
    Pure { ln_w0: f64, ln_w1: f64, d: f64 },
    /// Head, van Wijk part, tail.
    VanWijk {
        ln_w0: f64,
        /// Widths at the van Wijk part's ends.
        ln_a: f64,
        ln_b: f64,
        r0: f64,
        r1: f64,
        head: f64,
        middle: f64,
    },
}

/// A path from a start view to a target view; see the module docs.
#[derive(Clone, Debug)]
pub struct FlyPath {
    /// The start centre (at the target's precision): the first half of the path is anchored
    /// here, the second half at the target, so both ends are exact.
    start: Center,
    target: Viewport,
    /// View width in points (widths are `scale × view_width`).
    view_width: f64,
    /// Complex units per path unit (a power of two near the largest of w0, w1 and d).
    unit: Scale,
    /// Unit vector from the start centre to the target (re, im).
    dir: (f64, f64),
    d: f64,
    kind: Kind,
    length: f64,
}

impl FlyPath {
    /// The path from `start` to `target` for a view `view_width` points wide.
    pub fn new(start: &Viewport, target: &Viewport, view_width: f64) -> Self {
        let target = Viewport::with_center(target.center.clone(), target.scale).with_angle(target.angle);
        let bits = target.center.precision_bits().max(start.center.precision_bits());
        let start_center = start.center.with_min_precision(bits);
        let view_width = if view_width.is_finite() && view_width > 0.0 {
            view_width
        } else {
            1.0
        };
        let lw = view_width.log2();
        let (lw0, lw1) = (start.scale.log2() + lw, target.scale.log2() + lw);
        // A unit near the largest width; then grow it until the distance fits too.
        let mut k = lw0.max(lw1).ceil();
        let mut delta = target.center.delta_in(&start.center, Scale::pow2(k as i64));
        for _ in 0..8 {
            if delta.0.is_finite() && delta.1.is_finite() {
                break;
            }
            k += 1000.0;
            delta = target.center.delta_in(&start.center, Scale::pow2(k as i64));
        }
        let mut d = delta.0.hypot(delta.1);
        if !d.is_finite() {
            // Unreachable in practice (centres are within the plane); fly straight.
            delta = (0.0, 0.0);
            d = 0.0;
        }
        if d > 1.0 {
            let extra = d.log2().ceil();
            k += extra;
            delta = (delta.0 * (-extra).exp2(), delta.1 * (-extra).exp2());
            d = delta.0.hypot(delta.1);
        }
        let unit = Scale::pow2(k as i64);
        let ln2 = std::f64::consts::LN_2;
        let (ln_w0, ln_w1) = ((lw0 - k) * ln2, (lw1 - k) * ln2);
        let dir = if d > 0.0 {
            (delta.0 / d, delta.1 / d)
        } else {
            (1.0, 0.0)
        };
        let (kind, length) = if d == 0.0 || d.ln() - ln_w0.max(ln_w1) < PURE.ln() {
            (Kind::Pure { ln_w0, ln_w1, d }, (ln_w1 - ln_w0).abs() / RHO)
        } else {
            let ln_d = d.ln();
            let split = SPLIT.ln();
            let ln_a = ln_w0.max(ln_d + split);
            let ln_b = ln_w1.max(ln_a.min(ln_d) + split);
            let (wa, wb) = (ln_a.exp(), ln_b.exp());
            let r2 = RHO * RHO;
            let b0 = (wb * wb - wa * wa + r2 * r2 * d * d) / (2.0 * wa * r2 * d);
            let b1 = (wb * wb - wa * wa - r2 * r2 * d * d) / (2.0 * wb * r2 * d);
            let (r0, r1) = (-b0.asinh(), -b1.asinh());
            let head = (ln_a - ln_w0) / RHO;
            let middle = (r1 - r0) / RHO;
            let tail = (ln_b - ln_w1) / RHO;
            (
                Kind::VanWijk {
                    ln_w0,
                    ln_a,
                    ln_b,
                    r0,
                    r1,
                    head,
                    middle,
                },
                head + middle + tail,
            )
        };
        Self {
            start: start_center,
            target,
            view_width,
            unit,
            dir,
            d,
            kind,
            length,
        }
    }

    /// Path length (in units of s; the zoom ends cover ρ e-folds per unit).
    pub fn length(&self) -> f64 {
        self.length
    }

    pub fn target(&self) -> &Viewport {
        &self.target
    }

    /// At parameter `s`: the remaining offset from the view centre to the target along the
    /// start line (path units) and ln of the view width (path units).
    pub fn state(&self, s: f64) -> (f64, f64) {
        let (_, off, ln_w) = self.full_state(s);
        (off, ln_w)
    }

    /// Distance travelled from the start centre, remaining offset (both cancellation-free) and
    /// ln of the view width.
    fn full_state(&self, s: f64) -> (f64, f64, f64) {
        let s = s.clamp(0.0, self.length);
        match self.kind {
            Kind::Pure { ln_w0, ln_w1, d } => {
                if self.length == 0.0 {
                    return (d, 0.0, ln_w1);
                }
                let sign = if ln_w1 < ln_w0 { -1.0 } else { 1.0 };
                let ln_w = ln_w0 + sign * RHO * s;
                let off = if sign < 0.0 {
                    d * (2.0 * (ln_w - ln_w0)).exp()
                } else {
                    d * (1.0 - s / self.length)
                };
                (d - off, off, ln_w)
            }
            Kind::VanWijk {
                ln_w0,
                ln_a,
                ln_b,
                r0,
                r1,
                head,
                middle,
                ..
            } => {
                if s < head {
                    return (0.0, self.d, ln_w0 + RHO * s);
                }
                let t = s - head;
                if t < middle {
                    let x = RHO * t + r0;
                    let ln_w = ln_a + ln_cosh(r0) - ln_cosh(x);
                    // d − u(t) = wa cosh r0 / ρ² · sinh(ρ(S − t)) / (cosh r1 cosh x)
                    let ln_off =
                        ln_a + ln_cosh(r0) - 2.0 * RHO.ln() - ln_cosh(r1) - ln_cosh(x) + ln_sinh(RHO * (middle - t));
                    // u(t) = wa/ρ² · (cosh r0 tanh x − sinh r0) = wa/ρ² · sinh(ρt) / cosh x
                    let ln_u = ln_a - 2.0 * RHO.ln() - ln_cosh(x) + ln_sinh(RHO * t);
                    return (ln_u.exp(), ln_off.exp(), ln_w);
                }
                (self.d, 0.0, ln_b - RHO * (t - middle))
            }
        }
    }

    /// The view at parameter `s`: exactly the target at the end.
    pub fn viewport_at(&self, s: f64) -> Viewport {
        if s >= self.length {
            return self.target.clone();
        }
        let (u, off, ln_w) = self.full_state(s);
        // Anchored at the nearer end, so the f64 offset is small next to the view.
        let center = if off == 0.0 {
            self.target.center.clone()
        } else if off <= u {
            self.target
                .center
                .offset_scaled(-off * self.dir.0, -off * self.dir.1, self.unit)
        } else if u == 0.0 {
            self.start.clone()
        } else {
            self.start.offset_scaled(u * self.dir.0, u * self.dir.1, self.unit)
        };
        let log2 = ln_w / std::f64::consts::LN_2 + self.unit.log2() - self.view_width.log2();
        Viewport::with_center(center, Scale::from_log2(log2))
    }

    /// The highest path speed (units per second) at `s` from which the flight can ease into the
    /// turn between zooming out and zooming in (the van Wijk part's apex, where `ln w` peaks):
    /// near the apex the speed is at most `k cosh x` (x = ρ (s − apex): the zoom rate there is
    /// `ρ k sinh x`, so it swings through zero at a pace that does not depend on the flight
    /// speed), and before it the speed brakes onto that cap at `decel` path units per second²
    /// (closed form: the cap plus the braking line is convex in the point braked to).
    /// Infinite where no turn lies ahead (head, tail, pure zooms, past the apex region).
    pub fn turn_limit(&self, s: f64, k: f64, decel: f64) -> f64 {
        let Kind::VanWijk { r0, head, middle, .. } = self.kind else {
            return f64::INFINITY;
        };
        let (m0, m1) = (head, head + middle);
        // (Written so that NaN arguments give no limit.)
        let ahead = s < m1 && k > 0.0 && decel > 0.0 && middle > 0.0;
        if !ahead {
            return f64::INFINITY;
        }
        let apex = head - r0 / RHO;
        // Minimiser of g(p) = k² cosh²(ρ (p − apex)) + 2 decel (p − s) over p in [max(s, m0), m1].
        let u = -(2.0 * decel / (k * k * RHO)).asinh() / (2.0 * RHO);
        let p = (apex + u).clamp(s.max(m0), m1);
        let x = RHO * (p - apex);
        if x.abs() > 300.0 {
            return f64::INFINITY;
        }
        let g = k * k * x.cosh().powi(2) + 2.0 * decel * (p - s);
        if g.is_finite() { g.sqrt() } else { f64::INFINITY }
    }

    /// The target's offset from the view centre at `s`, in view points (y up).
    pub fn target_on_screen(&self, s: f64) -> (f64, f64) {
        let (off, ln_w) = self.state(s);
        let points = off / ln_w.exp() * self.view_width;
        if !points.is_finite() {
            return (f64::INFINITY, f64::INFINITY);
        }
        (points * self.dir.0, points * self.dir.1)
    }
}

/// A flight in progress along a [`FlyPath`].
#[derive(Clone, Debug)]
pub struct Flight {
    pub path: FlyPath,
    /// Path parameter and its speed (path units per second).
    pub s: f64,
    pub speed: Spring,
    /// Esc: slowing to a stop on the path.
    pub stopping: bool,
    /// View rotation (cinematic effects, kb: decisions/controls "View rotation"): the angle
    /// turns from `angle0` to `angle1` (unwrapped radians) in proportion to the path parameter,
    /// so with the spin effect it turns with the zoom; at the end the view has exactly
    /// `land_angle`.
    pub angle0: f64,
    pub angle1: f64,
    pub land_angle: f64,
    /// Brake gently onto the target ([`SOFT_LAND_TIME`]) instead of at the fly-to deceleration.
    pub soft_landing: bool,
    /// The flight rides its landing curve `v = c left^(2/3)` (from the first frame the spring
    /// would outrun it to the target), with this `c` (kept: a pace change on the way down does
    /// not move the curve).
    landing: Option<f64>,
    /// A soft landing rides its brake curve `v = √(2 a left)` with this deceleration `a`.
    soft_curve: Option<f64>,
}

impl Flight {
    pub fn new(path: FlyPath) -> Self {
        let a = path.target().angle;
        Self {
            path,
            s: 0.0,
            speed: Spring::default(),
            stopping: false,
            angle0: a,
            angle1: a,
            land_angle: a,
            soft_landing: false,
            landing: None,
            soft_curve: None,
        }
    }

    /// A flight that turns from `start_angle` to the target's angle the short way round, plus
    /// `spin` radians per e-fold of path (the path's e-folds: ρ per unit of s).
    pub fn turning(path: FlyPath, start_angle: f64, spin: f64) -> Self {
        let target = path.target().angle;
        let mut f = Self::new(path);
        f.angle0 = start_angle;
        if spin != 0.0 {
            f.angle1 = start_angle + spin * f.path.length() * RHO;
            f.land_angle = crate::viewport::wrap_angle(f.angle1);
            f.path.target.angle = f.land_angle;
        } else {
            f.angle1 = start_angle + crate::viewport::wrap_angle(target - start_angle);
            f.land_angle = target;
        }
        f
    }

    /// Turns the whole flight by `delta` radians (the compass: a drag or the turn to north
    /// while flying, kb: decisions/controls "Compass"): the spin goes on from the new angle and
    /// the flight lands turned by as much.
    pub fn shift_angle(&mut self, delta: f64) {
        if !delta.is_finite() || delta == 0.0 {
            return;
        }
        self.angle0 += delta;
        self.angle1 += delta;
        self.land_angle = crate::viewport::wrap_angle(self.land_angle + delta);
        self.path.target.angle = self.land_angle;
    }

    /// The view at the current path parameter, rotation included (exactly the target at the end).
    pub fn viewport(&self) -> Viewport {
        let v = self.path.viewport_at(self.s);
        if self.s >= self.path.length {
            return v.with_angle(self.land_angle);
        }
        let t = self.progress();
        v.with_angle(self.angle0 + (self.angle1 - self.angle0) * t)
    }

    /// Fraction of the path flown.
    pub fn progress(&self) -> f64 {
        if self.path.length > 0.0 {
            (self.s / self.path.length).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// Advances by `dt` at `speed` e-folds per second times `throttle`; returns true when the
    /// flight is over (landed, or stopped by Esc).
    pub fn step(&mut self, dt: f64, speed: f64, throttle: f64) -> bool {
        if self.soft_landing {
            return self.step_soft(dt, speed, throttle);
        }
        let left = (self.path.length - self.s).max(0.0);
        let cruise = if self.stopping { 0.0 } else { speed / RHO * throttle };
        // Braking into a turn, in path units: the set speed (not throttled: the renderer
        // back-off must not move the curves) stops in `brake_time`.
        let vp = (speed / RHO).max(1e-3);
        let decel = vp / brake_time(speed);
        // The landing curve v = c left^(2/3) through the current speed: from it to rest in
        // `brake_time` of it (sized by the speed flown, not the set speed: a short leg that
        // never reaches the set speed brakes as briefly as it flies).
        let now = self.speed.v.max(1e-3);
        let c = self
            .landing
            .unwrap_or_else(|| (3.0 * now.sqrt() / brake_time(now * RHO)).powf(2.0 / 3.0));
        let k = 2.0 * 1f64.sinh().atan() / (RHO * TURN_TIME);
        let turn = self.path.turn_limit(self.s, k, decel);
        let target = cruise.min(turn);
        let omega = if target > self.speed.v {
            PATH_OMEGA.0
        } else {
            PATH_OMEGA.1
        };
        let mut ds = self.speed.follow(target, omega, dt).max(0.0);
        if !self.stopping && turn.is_finite() && self.speed.v > turn {
            // Ride the turn limit (braking at `decel` onto the apex cap) when the spring would
            // lag above it: the lag of ω = 4 at this deceleration reached the apex at six times
            // the cap.
            let line = (turn * dt - 0.5 * decel * dt * dt).max(0.0);
            if ds > line {
                ds = line;
                let v = self.path.turn_limit(self.s + line, k, decel).min(turn);
                self.speed = Spring {
                    v,
                    a: (v - turn) / dt.max(1e-6),
                };
            }
        }
        if !self.stopping {
            // Ride the landing curve once the spring would outrun it (it lags, and reached the
            // end still moving), in closed form: left^(1/3) falls at c/3 per second, so 30, 60
            // and 120 Hz agree.
            let r = (left.cbrt() - c / 3.0 * dt).max(0.0);
            let curve_left = r * r * r;
            if self.landing.is_some() || ds > left - curve_left {
                self.landing = Some(c);
                ds = left - curve_left;
                self.speed = Spring {
                    v: c * curve_left.powf(LAND_POWER),
                    a: -2.0 / 3.0 * c * c * r,
                };
            }
        }
        self.s = (self.s + ds).min(self.path.length);
        if !self.stopping && self.path.length - self.s < LAND_SNAP {
            self.s = self.path.length;
            return true;
        }
        self.stopping && self.speed.v.abs() < 1e-3
    }

    /// [`Flight::step`] of a soft landing (the cruise's landing on the home view): a constant
    /// deceleration over [`SOFT_LAND_TIME`] onto the target.
    fn step_soft(&mut self, dt: f64, speed: f64, throttle: f64) -> bool {
        let left = (self.path.length - self.s).max(0.0);
        let cruise = if self.stopping { 0.0 } else { speed / RHO * throttle };
        // Constant deceleration onto the end: v = √(2 a s_left) (kept once on the curve).
        let decel = self
            .soft_curve
            .unwrap_or_else(|| (speed / RHO / SOFT_LAND_TIME).max(1e-3));
        let target = cruise.min((2.0 * decel * left).sqrt());
        let omega = if target > self.speed.v {
            PATH_OMEGA.0
        } else {
            PATH_OMEGA.1
        };
        let mut ds = self.speed.follow(target, omega, dt).max(0.0);
        if !self.stopping {
            // On the brake curve v = √(2 a left) the spring lags (it reaches the end still
            // moving and snaps); a soft landing rides the curve itself once the spring would
            // outrun it, in closed form (√left falls at √(a/2) per second), so it decelerates
            // evenly to rest and 30, 60 and 120 Hz agree. Once on the curve it stays on it:
            // released from the curve with its deceleration, the spring undershot to a
            // standstill just short of the end and snapped there after a pause (seen landing
            // from 1e-50 in gear 10).
            let curve_left = (left.sqrt() - (0.5 * decel).sqrt() * dt).max(0.0).powi(2);
            if self.soft_curve.is_some() || ds > left - curve_left {
                self.soft_curve = Some(decel);
                ds = left - curve_left;
                self.speed = Spring {
                    v: (2.0 * decel * curve_left).sqrt(),
                    a: -decel,
                };
            }
        }
        self.s = (self.s + ds).min(self.path.length);
        if !self.stopping && self.path.length - self.s < LAND_SNAP {
            self.s = self.path.length;
            return true;
        }
        self.stopping && self.speed.v.abs() < 1e-3
    }
}

/// Parses "re / im / scale" text: the app's copied coordinates (`re …`, `im …`,
/// `scale … (zoom …)` lines) or three numbers separated by whitespace or commas.
pub fn parse_coordinates(text: &str) -> Option<(String, String, String)> {
    let (mut re, mut im, mut scale) = (None, None, None);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("re"), Some(v)) => re = Some(v.to_string()),
            (Some("im"), Some(v)) => im = Some(v.to_string()),
            (Some("scale"), Some(v)) => scale = Some(v.to_string()),
            _ => {}
        }
    }
    if let (Some(re), Some(im), Some(scale)) = (re.clone(), im.clone(), scale.clone()) {
        return Some((re, im, scale));
    }
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() == 3 {
        return Some((words[0].into(), words[1].into(), words[2].into()));
    }
    None
}

/// The `angle` line of copied coordinates (degrees, the camera's counter-clockwise turn), as
/// radians; None when the text has none.
pub fn parse_angle(text: &str) -> Option<f64> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("angle"), Some(v)) => v
                .trim_end_matches('°')
                .parse::<f64>()
                .ok()
                .filter(|a| a.is_finite())
                .map(|a| crate::viewport::wrap_angle(a.to_radians())),
            _ => None,
        }
    })
}

/// The fractal of copied coordinates (F3): the `formula` line (`formula multibrot5`) when it
/// names a formula a user can pick; Mandelbrot for the app's labelled text without one (what
/// Mandelbrot views and older builds copy); None for bare numbers (the view keeps its formula).
pub fn parse_formula(text: &str) -> Option<crate::formula::Formula> {
    let mut labelled = false;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("formula"), Some(v)) => {
                return crate::formula::Formula::parse(v).filter(|f| crate::formula::selectable().contains(f));
            }
            (Some("re" | "im" | "scale"), Some(_)) => labelled = true,
            _ => {}
        }
    }
    labelled.then_some(crate::formula::Formula::Mandelbrot)
}

/// The view of parsed coordinates (None if they are not numbers or the scale is not positive).
pub fn view_from_text(re: &str, im: &str, scale: &str) -> Option<Viewport> {
    let center = Center::parse(re.trim(), im.trim())?;
    let scale = Scale::parse(scale.trim())?;
    if !center.re().is_finite() || !center.im().is_finite() {
        return None;
    }
    let mut v = Viewport::with_center(center, scale);
    v.scale = v
        .scale
        .max(v.min_scale())
        .min(Scale::from_f64(crate::viewport::MAX_SCALE));
    v.ensure_precision();
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f64 = 1000.0;

    #[test]
    fn coordinates_text_carries_an_optional_angle() {
        let text = "re -0.75\nim 0.1\nscale 1e-5 (zoom 375×)\nangle 30";
        assert_eq!(
            parse_coordinates(text),
            Some(("-0.75".into(), "0.1".into(), "1e-5".into()))
        );
        let a = parse_angle(text).unwrap();
        assert!((a - 30f64.to_radians()).abs() < 1e-15);
        assert!((parse_angle("angle 270°").unwrap() + 90f64.to_radians()).abs() < 1e-12);
        assert_eq!(parse_angle("re 1\nim 2\nscale 3"), None);
        assert_eq!(parse_angle("angle nan"), None);
    }

    #[test]
    fn coordinates_text_carries_the_formula() {
        use crate::formula::Formula;
        let m5 = "re 0.1\nim 0.2\nscale 1e-5 (zoom 375×)\nformula multibrot5";
        assert_eq!(parse_formula(m5), Some(Formula::Multibrot(5)));
        assert_eq!(parse_coordinates(m5), Some(("0.1".into(), "0.2".into(), "1e-5".into())));
        // Labelled text without the line: a Mandelbrot view (and older builds' text).
        assert_eq!(parse_formula("re 0.1\nim 0.2\nscale 1e-5"), Some(Formula::Mandelbrot));
        // Bare numbers keep the view's formula; unknown or test-only formulas are ignored.
        assert_eq!(parse_formula("0.1 0.2 1e-5"), None);
        assert_eq!(parse_formula("re 0\nim 0\nscale 1\nformula multibrot2"), None);
        assert_eq!(parse_formula("re 0\nim 0\nscale 1\nformula burningship"), None);
    }

    fn sea8007() -> Viewport {
        view_from_text(
            "-0.74364388703715870475219150611477977822",
            "0.13182590420531197049313205638514067897",
            "2e-34",
        )
        .unwrap()
    }

    /// Screen offset of `c` in `v`, points.
    fn on_screen(v: &Viewport, c: &Center) -> (f64, f64) {
        c.delta_in(&v.center, v.scale)
    }

    /// Checks a path: starts at `start`, ends exactly at the target, and is continuous: the
    /// target's screen position and ln scale change little between close parameters.
    fn check(start: &Viewport, target: &Viewport) -> FlyPath {
        let p = FlyPath::new(start, target, W);
        let v0 = p.viewport_at(0.0);
        let (dx, dy) = start.center.delta_in(&v0.center, start.scale);
        assert!(dx.hypot(dy) < 1e-6, "start centre off by {dx}, {dy} points");
        assert!((v0.scale.ln() - start.scale.ln()).abs() < 1e-9, "start scale");
        let end = p.viewport_at(p.length());
        assert_eq!(end.center.delta_in(&target.center, target.scale), (0.0, 0.0));
        assert_eq!(end.scale, target.scale);
        // Just before the end: within a hair of the target, no jump onto it.
        let near = p.viewport_at(p.length() - 1e-6);
        let (nx, ny) = on_screen(&near, &target.center);
        assert!(nx.hypot(ny) < 1e-3, "{nx} {ny}");
        // Continuity: 4000 steps; ln scale steps bounded by ρ·ds·(1 + ε), the view centre moves
        // at most a bounded number of view widths per step.
        let n = 4000;
        let ds = p.length() / n as f64;
        let mut prev = p.viewport_at(0.0);
        for i in 1..=n {
            let v = p.viewport_at(i as f64 * ds);
            let dl = (v.scale.ln() - prev.scale.ln()).abs();
            assert!(dl <= RHO * ds * 1.0001 + 1e-12, "zoom jump {dl} at {i}");
            let (mx, my) = v.center.delta_in(&prev.center, prev.scale);
            assert!(
                mx.hypot(my) <= 2.0 * W * RHO * ds + 1e-6,
                "centre jump {mx},{my} pt at {i}"
            );
            prev = v;
        }
        p
    }

    #[test]
    fn home_to_seahorse_minibrot_at_1e_34() {
        let home = Viewport::fitting(W, 600.0);
        let p = check(&home, &sea8007());
        // ~70 e-folds of zoom: the length is about ln(ratio) / ρ plus the pan's detour.
        let e = (home.scale.ln() - sea8007().scale.ln()) / RHO;
        assert!(p.length() > e && p.length() < e + 3.0, "{} vs {e}", p.length());
        // The target comes on screen early and stays there, heading for the centre.
        let mut last = f64::INFINITY;
        for i in 10..=100 {
            let (x, y) = p.target_on_screen(p.length() * i as f64 / 100.0);
            let r = x.hypot(y);
            assert!(r <= last + 1e-6, "target moves away at {i}%: {r}");
            assert!(r < 0.6 * W, "target off screen at {i}%: {r}");
            last = r;
        }
    }

    #[test]
    fn deep_to_far_deep_zooms_out_pans_and_dives() {
        // From 1e-300 at c = i to the seahorse minibrot: out ~690 e-folds, over, in ~78.
        let a = view_from_text("0", "1", "1e-300").unwrap();
        let p = check(&a, &sea8007());
        let widest = (0..=1000)
            .map(|i| p.viewport_at(p.length() * i as f64 / 1000.0).scale.log10())
            .fold(f64::NEG_INFINITY, f64::max);
        // Zooms out to about the distance (|i − sea| ≈ 1.2, / 1000 points).
        assert!(widest > -4.0 && widest < -2.0, "{widest}");
    }

    #[test]
    fn nearby_targets_and_zoom_out() {
        let s = sea8007();
        // A target inside the view, deeper: mostly a zoom, slight drift.
        let start = Viewport::with_center(
            s.center.offset_scaled(120.0, -40.0, Scale::from_f64(1e-20)),
            Scale::from_f64(1e-20),
        );
        check(&start, &s);
        // Zoom out to home.
        check(&s, &Viewport::fitting(W, 600.0));
        // Exactly the same centre: pure zoom; and the same view: an empty path.
        let mut same = s.clone();
        same.scale = Scale::from_f64(1e-10);
        let p = check(&same, &s);
        assert!(matches!(p.kind, Kind::Pure { .. }));
        let p = FlyPath::new(&s, &s, W);
        assert_eq!(p.length(), 0.0);
        // 1e-300 neighbourhood.
        let a = view_from_text("0", "1", "1e-300").unwrap();
        let b = view_from_text("0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005", "1", "1e-305").unwrap();
        check(&a, &b);
    }

    fn fly(hz: f64) -> Vec<(f64, f64, f64)> {
        let start = Viewport::fitting(W, 600.0);
        let target = sea8007();
        let mut f = Flight::new(FlyPath::new(&start, &target, W));
        let mut out = Vec::new();
        let dt = 1.0 / hz;
        let mut t = 0.0;
        let mut done = false;
        for k in 1..=(40.0 * hz) as usize {
            if !done {
                done = f.step(dt, 3.0, 1.0);
            }
            t += dt;
            if k % (hz as usize / 2) == 0 {
                let v = f.path.viewport_at(f.s);
                let (x, y) = on_screen(&v, &target.center);
                out.push((v.scale.ln(), x / W, y / W));
            }
        }
        let _ = t;
        assert!(done, "landed");
        out
    }

    #[test]
    fn flights_are_frame_rate_independent_and_land_exactly() {
        let (a, b, c) = (fly(30.0), fly(60.0), fly(120.0));
        for i in 0..a.len() {
            for o in [b[i], c[i]] {
                assert!((a[i].0 - o.0).abs() < 2e-3, "zoom {i}: {:?} vs {o:?}", a[i]);
                assert!(
                    (a[i].1 - o.1).abs() + (a[i].2 - o.2).abs() < 2e-3,
                    "target {i}: {:?} vs {o:?}",
                    a[i]
                );
            }
        }
        let last = a.last().unwrap();
        assert_eq!((last.1, last.2), (0.0, 0.0));
    }

    #[test]
    fn esc_eases_to_a_stop_on_the_path() {
        let mut f = Flight::new(FlyPath::new(&Viewport::fitting(W, 600.0), &sea8007(), W));
        for _ in 0..120 {
            f.step(1.0 / 60.0, 2.0, 1.0);
        }
        let v = f.speed.v;
        assert!(v > 0.9 * 2.0 / RHO);
        f.stopping = true;
        let s0 = f.s;
        let mut frames = 0;
        while !f.step(1.0 / 60.0, 2.0, 1.0) {
            frames += 1;
            assert!(frames < 300);
        }
        // Glides on about 2/ω seconds' worth of speed.
        let glide = f.s - s0;
        assert!((glide - 2.0 / PATH_OMEGA.1 * v).abs() < 0.05 * glide, "{glide}");
    }

    #[test]
    fn coordinates_text() {
        let t = "re -0.75\nim 0.1\nscale 1.5e-300 (zoom 10^297.4×)";
        assert_eq!(
            parse_coordinates(t),
            Some(("-0.75".into(), "0.1".into(), "1.5e-300".into()))
        );
        assert_eq!(
            parse_coordinates("-0.75, 0.1, 2e-5"),
            Some(("-0.75".into(), "0.1".into(), "2e-5".into()))
        );
        assert_eq!(parse_coordinates("hello"), None);
        assert!(view_from_text("-0.75", "0.1", "1.5e-300").is_some());
        assert!(view_from_text("x", "0.1", "1").is_none());
        assert!(view_from_text("0", "0.1", "-1").is_none());
    }

    /// The zoom rate (e-folds/s, + in) per step of a flight from `a` to `b` at `speed`, `hz`.
    fn rates(a: &Viewport, b: &Viewport, speed: f64, hz: f64) -> Vec<f64> {
        let mut f = Flight::new(FlyPath::new(a, b, W));
        let mut z = f.path.state(0.0).1;
        let mut out = Vec::new();
        for _ in 0..(600.0 * hz) as usize {
            let done = f.step(1.0 / hz, speed, 1.0);
            let z1 = f.path.state(f.s).1;
            out.push((z - z1) * hz);
            z = z1;
            if done {
                return out;
            }
        }
        panic!("no landing");
    }

    /// A cross-over (out of a deep needle view, across, into the seahorse valley) at the default
    /// and the top journey speed: the zoom rate brakes into the turn, swings through zero over
    /// at least ~TURN_TIME, and eases to rest on the target; bounded zoom acceleration (before:
    /// −28 → +23 e-folds/s in 0.15 s, landings from 25 e-folds/s in one frame).
    #[test]
    fn cross_overs_turn_and_land_smoothly_at_any_speed() {
        let a = view_from_text("-1.4304", "0", "1e-15").unwrap();
        let b = view_from_text("-0.745", "0.11", "1e-6").unwrap();
        for speed in [1.4, 4.0, 16.0] {
            let mut by_hz = Vec::new();
            for hz in [30.0, 60.0, 120.0] {
                let r = rates(&a, &b, speed, hz);
                let n = (0.1 * hz) as usize;
                let peak = r.iter().fold(0.0f64, |m, x| m.max(x.abs()));
                assert!(peak > 0.9 * speed, "{speed}: peak {peak}");
                // Zoom acceleration over 0.1 s: at most the spring's ease-in (V ω / e) or the
                // landing's first braking (2 V / T).
                let acc = r
                    .windows(n + 1)
                    .map(|w| (w[n] - w[0]).abs() * hz / n as f64)
                    .fold(0.0, f64::max);
                let bound = (speed * PATH_OMEGA.0 / std::f64::consts::E).max(2.0 * speed / brake_time(speed));
                assert!(acc < 1.2 * bound + 0.5, "{speed} @{hz}: {acc} (bound {bound})");
                // One turn: the rate changes sign once, and the swing from −1 to +1 e-fold/s
                // takes over half a second at any speed.
                let signs: Vec<f64> = r.iter().filter(|x| x.abs() > 1e-3).map(|x| x.signum()).collect();
                let flips = signs.windows(2).filter(|w| w[0] != w[1]).count();
                assert_eq!(flips, 1, "{speed} @{hz}");
                let out = r.iter().rposition(|&x| x < -1.0).unwrap();
                let back = r.iter().position(|&x| x > 1.0).unwrap();
                let swing = (back - out) as f64 / hz;
                assert!(swing > 0.5, "{speed} @{hz}: swing {swing} s");
                // Lands at rest: the last 0.1 s is slow.
                let last = r[r.len() - n..].iter().fold(0.0f64, |m, x| m.max(x.abs()));
                assert!(last < 0.15 * speed, "{speed} @{hz}: last {last}");
                by_hz.push(r.len() as f64 / hz);
            }
            // 30, 60 and 120 Hz take the same time.
            assert!((by_hz[0] - by_hz[2]).abs() < 0.05 * by_hz[2], "{speed}: {by_hz:?}");
        }
    }
}
