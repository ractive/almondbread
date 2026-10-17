//! Driving levels and cruise (kb: decisions/controls "Driving levels"). Like a car's
//! gears, the navigator is in one of four levels, shown in the HUD's cockpit:
//!
//! - **M** manual: nothing moves by itself (keys, mouse, gestures as always);
//! - **C** cruise: a continuous zoom at a set speed on one signed axis, Q / E as pedals
//!   (E in, Q out, each press one rung of [`LADDER`], held keys keep stepping, a detent at
//!   zero), steered only by the user (pointer joystick, W A S D), never pulled anywhere;
//! - **A** autopilot (`crate::autopilot`): speed and steering towards interesting areas;
//! - **J** journey (`crate::journey`) and fly-to flights.
//!
//! The navigator owns a [`Cruise`] and decides the level and the hand-overs between levels
//! (`crate::input::Navigator::level`); this module is the cruise's motion and the pedal
//! ladder.
//!
//! **Zoom-out dynamics.** In reverse the effective rate grows with the depth `D` (e-folds
//! below the home view): `rate = set · (1 + D / BOOST_DEPTH)^ramp(t)`, `ramp` rising from
//! 0 to 1 over [`BOOST_RAMP`] seconds after reversing began (smoothstep), so one press does
//! not jump to a ludicrous speed: the log of the speed accelerates evenly. With the boost in full, `D + BOOST_DEPTH` decays
//! exponentially: home from depth `D` takes `BOOST_DEPTH / set · ln(1 + D / BOOST_DEPTH)`
//! seconds (logarithmic in the depth: about a minute from 1e-3000 at the first rung). The
//! rate itself follows its target on the autopilot's rate spring (ω 3 / 4 per second), so the
//! base speed eases in over about a second and a stop brakes on the spring's smooth curve.
//! Near home the navigator hands the last stretch to a soft-landing flight
//! (`Flight::soft_landing`) that rests exactly on the home framing. Everything is integrated
//! in fixed substeps of at most 2 ms (the boost depends on the depth the zoom changes), so 30,
//! 60 and 120 Hz agree to second order; the substep count is bounded
//! (kb: techniques/hang-prevention).

use crate::autopilot::{self, Motion};
use crate::input::Spring;

/// The four driving levels, like a gear selector (kb: decisions/controls "Driving levels").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Level {
    #[default]
    Manual,
    Cruise,
    Autopilot,
    Journey,
}

impl Level {
    /// The letter in the cockpit.
    pub fn letter(self) -> char {
        match self {
            Self::Manual => 'M',
            Self::Cruise => 'C',
            Self::Autopilot => 'A',
            Self::Journey => 'J',
        }
    }

    pub const ALL: [Level; 4] = [Self::Manual, Self::Cruise, Self::Autopilot, Self::Journey];
}

/// The pedal ladder: zoom factors per second of the rungs on either side of zero (E steps
/// right, Q left: … ×2 out, ×1.5 out, stop, ×1.5 in, ×2 in …). It goes well beyond the
/// autopilot's ×55 so that deep zoom-outs can be fast; forward the renderer back-off
/// (throttle) still applies.
/// 26 gears, each 1.1–1.4× the last up to ×100, then wider (user: finer than 12 steps).
pub const LADDER: [f64; 26] = [
    1.1, 1.2, 1.35, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0, 6.5, 8.0, 10.0, 13.0, 16.0, 20.0, 27.0, 35.0, 45.0, 60.0,
    80.0, 100.0, 150.0, 250.0, 400.0, 1000.0,
];

/// A held pedal steps one more rung after [`HOLD_DELAY`] s, then every [`HOLD_REPEAT`] s.
pub const HOLD_DELAY: f64 = 0.4;
pub const HOLD_REPEAT: f64 = 0.15;
/// Depth boost in reverse: +1× per this many e-folds of depth below home, ramping in over
/// [`BOOST_RAMP`] seconds after reversing began, and scaled by the gear ([`boost_share`]):
/// the low gears barely boost, the top gears fully (user, 2026-10-17).
pub const BOOST_DEPTH: f64 = 3.0;
pub const BOOST_RAMP: f64 = 10.0;
/// The boost exponent's share in the first gear (it rises linearly to 1 in the top gear).
pub const BOOST_SHARE_LOW: f64 = 0.5;
/// The effective rate never exceeds this (e-folds/s; the boost at 1e-3000 at the top rung).
pub const RATE_MAX: f64 = 2000.0;
/// The renderer back-off (throttle) slows a reverse cruise at most to this share: zooming out
/// mostly shows the history shrinking while new references come, and home must stay about a
/// minute away from any depth.
pub const REVERSE_THROTTLE_MIN: f64 = 0.5;
/// The soft landing takes over when the path home is this many seconds of the current rate
/// long (the landing's brake curve then matches the rate: half the soft-landing time).
pub const LAND_LEAD: f64 = crate::flyto::SOFT_LAND_TIME / 2.0;
/// Rates below this (e-folds/s) with nothing set count as at rest.
const REST: f64 = 1e-3;

/// The rung values in e-folds per second.
pub fn rung(i: usize) -> f64 {
    LADDER[i.min(LADDER.len() - 1)].ln()
}

/// The gear (1-based) of a set speed (signed e-folds/s): the nearest rung; None at rest.
pub fn gear(set: f64) -> Option<usize> {
    let m = set.abs();
    if !(m.is_finite() && m > 1e-9) {
        return None;
    }
    (0..LADDER.len())
        .min_by(|a, b| (rung(*a) - m).abs().total_cmp(&(rung(*b) - m).abs()))
        .map(|i| i + 1)
}

/// The share of the depth boost's exponent for a set speed: [`BOOST_SHARE_LOW`] in the first
/// gear, 1 in the top one (linear in the gear; beyond the ladder 1).
pub fn boost_share(set: f64) -> f64 {
    let Some(g) = gear(set) else { return 0.0 };
    let f = (g - 1) as f64 / (LADDER.len() - 1) as f64;
    BOOST_SHARE_LOW + (1.0 - BOOST_SHARE_LOW) * f.clamp(0.0, 1.0)
}

/// The fastest rung (e-folds per second).
pub fn max_speed() -> f64 {
    rung(LADDER.len() - 1)
}

/// The next speed on the ladder from `v` (signed e-folds per second; + zooms in) for a press
/// of the pedal `dir` (+1: E, in; −1: Q, out). Away from zero: the next rung beyond |v| (the
/// last rung stays); towards zero: the next rung below |v|, then zero. From zero a press
/// starts in its direction only if `cross` (a new press: a held key's repeats stop at the
/// detent, so holding Q never flips into reverse).
pub fn pedal(v: f64, dir: f64, cross: bool) -> f64 {
    let dir = if dir >= 0.0 { 1.0 } else { -1.0 };
    let m = v.abs();
    if !(m.is_finite()) || m < 1e-9 {
        return if cross { dir * rung(0) } else { 0.0 };
    }
    let sign = v.signum();
    // 1 % tolerance: a value on a rung counts as that rung.
    if sign == dir {
        let next = (0..LADDER.len()).map(rung).find(|r| *r > m * 1.01);
        sign * next.unwrap_or(m.max(max_speed()))
    } else {
        let below = (0..LADDER.len()).map(rung).rev().find(|r| *r < m * 0.99);
        below.map_or(0.0, |r| sign * r)
    }
}

/// `x` per second as a factor ("×1.5", "×2.46", "×35", "×1000").
pub fn factor_text(efolds: f64) -> String {
    let f = efolds.abs().exp();
    if !f.is_finite() || f >= 1e5 {
        // ln(10) per decade.
        return format!("×1e{:.0}", efolds.abs() / std::f64::consts::LN_10);
    }
    if f >= 10.0 {
        return format!("×{f:.0}");
    }
    let s = format!("{f:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("×{s}")
}

/// Smooth 0 → 1 ramp (zero slope at both ends).
fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The cruise: see the module docs.
#[derive(Clone, Debug, Default)]
pub struct Cruise {
    /// The level is C: a speed is set, or the cruise still brakes to rest.
    engaged: bool,
    /// Set speed, signed e-folds per second (+ in, − out); 0 brakes to rest.
    set: f64,
    /// When the set speed last changed (HUD highlight).
    pub speed_changed: f64,
    /// The zoom rate (signed e-folds per second, boost included) and the steering heading
    /// (view points from the centre).
    rate: Spring,
    heading: [Spring; 2],
    /// Seconds the set speed has been in reverse (the boost's ramp).
    reverse_for: f64,
    /// The soft landing on the home view flies (the navigator's flight), at this speed.
    landing: Option<f64>,
    /// The depth boost of the last frame (1: none).
    boost: f64,
}

impl Cruise {
    /// The level is C.
    pub fn is_engaged(&self) -> bool {
        self.engaged
    }

    /// Set speed, signed e-folds per second.
    pub fn set_speed(&self) -> f64 {
        self.set
    }

    /// Current zoom rate, signed e-folds per second (eased, boost included).
    pub fn rate(&self) -> f64 {
        self.rate.v
    }

    /// The reverse depth boost of the last frame (1 when none).
    pub fn boost(&self) -> f64 {
        if self.boost.is_finite() && self.boost >= 1.0 {
            self.boost
        } else {
            1.0
        }
    }

    pub fn heading(&self) -> (f64, f64) {
        (self.heading[0].v, self.heading[1].v)
    }

    /// The soft landing on the home view flies, at this speed (e-folds per second).
    pub fn landing(&self) -> Option<f64> {
        self.landing
    }

    /// Something still moves (or will).
    pub fn is_moving(&self) -> bool {
        self.landing.is_none()
            && (self.engaged
                || !(self.rate.v == 0.0 && self.rate.a == 0.0)
                || self.heading.iter().any(|s| s.v != 0.0 || s.a != 0.0))
    }

    /// Engages at set speed `set` (clamped to the ladder's range) with the zoom rate and
    /// heading carried over from what drove before (the autopilot, a journey's flight).
    pub fn engage(&mut self, set: f64, rate: f64, heading: (f64, f64), now: f64) {
        let m = max_speed();
        self.engaged = true;
        self.set = if set.is_finite() { set.clamp(-m, m) } else { 0.0 };
        self.speed_changed = now;
        self.rate = Spring {
            v: if rate.is_finite() {
                rate.clamp(-RATE_MAX, RATE_MAX)
            } else {
                0.0
            },
            a: 0.0,
        };
        self.heading = [
            Spring {
                v: if heading.0.is_finite() { heading.0 } else { 0.0 },
                a: 0.0,
            },
            Spring {
                v: if heading.1.is_finite() { heading.1 } else { 0.0 },
                a: 0.0,
            },
        ];
        self.reverse_for = 0.0;
        self.landing = None;
    }

    /// A pedal press (`cross`: a new press, not a held key's repeat): the next rung. Engages
    /// from rest; `out_ok` false refuses reverse from rest (the whole set is in view already).
    /// Returns the new set speed.
    pub fn pedal(&mut self, dir: f64, cross: bool, out_ok: bool, now: f64) -> f64 {
        let next = pedal(self.set, dir, cross);
        if next < 0.0 && self.set >= 0.0 && !out_ok {
            return self.set;
        }
        if next != self.set {
            self.speed_changed = now;
        }
        if next != 0.0 {
            self.engaged = true;
        }
        self.set = next;
        self.set
    }

    /// Scales the set speed (wheel, scroll), keeping its direction.
    pub fn scale_speed(&mut self, f: f64, now: f64) {
        if f.is_finite() && f > 0.0 && self.set != 0.0 {
            let m = max_speed();
            self.set = (self.set * f).clamp(-m, m);
            self.speed_changed = now;
        }
    }

    /// Esc: brake to rest (smoothly, on the rate spring). Returns whether it was engaged.
    pub fn stop(&mut self, now: f64) -> bool {
        let was = self.engaged;
        if self.set != 0.0 {
            self.speed_changed = now;
        }
        self.set = 0.0;
        was
    }

    /// Hands over to another level: the cruise brakes out on its springs, not engaged.
    pub fn release(&mut self) {
        self.engaged = false;
        self.set = 0.0;
        self.landing = None;
    }

    /// Stops dead (reset, a new flight or journey).
    pub fn halt(&mut self) {
        *self = Self {
            speed_changed: self.speed_changed,
            ..Self::default()
        };
    }

    /// The soft landing takes over at `speed` e-folds per second: the cruise's own motion
    /// stops (the flight carries it on).
    pub fn begin_landing(&mut self, speed: f64) {
        self.landing = Some(speed.abs().max(0.05));
        self.rate = Spring::default();
        self.heading = [Spring::default(); 2];
        self.reverse_for = 0.0;
    }

    /// The landing flight ended (landed, stopped, or taken over): back to rest, level M.
    pub fn end_landing(&mut self) {
        self.landing = None;
        self.engaged = false;
        self.set = 0.0;
    }

    /// The target rate at depth `depth` (e-folds below home) with the renderer `throttle`.
    fn rate_target(&self, depth: f64, throttle: f64, at_limit: bool) -> f64 {
        if !self.engaged || self.set == 0.0 {
            return 0.0;
        }
        if self.set > 0.0 {
            return if at_limit { 0.0 } else { self.set * throttle };
        }
        if depth <= 0.0 {
            // The whole set is in view: zooming out stops (the landing normally took over).
            return 0.0;
        }
        // The boost ramps in on a log scale: (1 + D / D0)^ramp, an even acceleration of the
        // log speed rather than a jump to a large multiple.
        let ramp = smoothstep(self.reverse_for / BOOST_RAMP) * boost_share(self.set);
        let boost = (ramp * (depth / BOOST_DEPTH).ln_1p()).exp();
        (self.set * boost * throttle.max(REVERSE_THROTTLE_MIN)).max(-RATE_MAX)
    }

    /// Advances by `dt`: the rate eases towards the set speed (boosted in reverse by the depth
    /// `depth`, e-folds below home), the heading towards `steer` (view points), and returns
    /// the frame's motion (zoom about the heading, pan bringing it to the centre, as the
    /// autopilot's steering). `throttle`: the renderer back-off; `at_limit`: the depth limit.
    pub fn step(&mut self, dt: f64, steer: (f64, f64), depth: f64, throttle: f64, at_limit: bool) -> Motion {
        if self.landing.is_some() {
            return Motion::default();
        }
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let steer = if self.engaged && steer.0.is_finite() && steer.1.is_finite() {
            steer
        } else {
            (0.0, 0.0)
        };
        let throttle = if throttle.is_finite() {
            throttle.clamp(0.0, 1.0)
        } else {
            1.0
        };
        // At most 1000 substeps (kb: techniques/hang-prevention).
        let n = (dt / 0.002).ceil().clamp(1.0, 1000.0);
        let h = dt / n;
        let (mut ln_zoom, mut pan, mut at) = (0.0, (0.0, 0.0), (0.0, 0.0));
        let mut boost = 1.0;
        for _ in 0..n as usize {
            if self.set < 0.0 {
                self.reverse_for += h;
            } else {
                self.reverse_for = 0.0;
            }
            let d = depth + ln_zoom;
            let target = self.rate_target(d, throttle, at_limit);
            if self.set < 0.0 {
                boost = (target / (self.set * throttle.max(REVERSE_THROTTLE_MIN))).max(1.0);
            }
            // Speeding up (away from zero, same direction) eases in; slowing down brakes.
            let up = target.abs() > self.rate.v.abs() && target * self.rate.v >= 0.0;
            let omega = if up {
                autopilot::RATE_OMEGA.0
            } else {
                autopilot::RATE_OMEGA.1
            };
            let dz = self.rate.follow(target, omega, h);
            let ix = self.heading[0].follow(steer.0, autopilot::STEER_OMEGA, h);
            let iy = self.heading[1].follow(steer.1, autopilot::STEER_OMEGA, h);
            let z = if h > 0.0 { dz / h } else { 0.0 };
            let k = (autopilot::STEER_PAN + autopilot::STEER_PAN_PER_RATE * z.clamp(0.0, autopilot::SPEED_RANGE.1))
                * throttle.max(0.05);
            // In points of the frame's start scale (the zoom so far shrinks or grows them).
            let w = (-ln_zoom - 0.5 * dz).exp();
            // The zoom about the mean heading of the substep moves the centre by
            // h (1 − e^−dz) e^−z (start-scale points): weights for one zoom about `at`.
            let wz = (-ln_zoom).exp() * -(-dz).exp_m1();
            ln_zoom += dz;
            pan.0 -= k * ix * w;
            pan.1 -= k * iy * w;
            if h > 0.0 {
                at.0 += ix / h * wz;
                at.1 += iy / h * wz;
            }
        }
        self.boost = boost;
        // The navigator pans at the frame's midpoint scale.
        let mid = (0.5 * ln_zoom).exp();
        let pan = (pan.0 * mid, pan.1 * mid);
        let total = -(-ln_zoom).exp_m1();
        let w = if total.abs() > 1e-15 {
            (at.0 / total, at.1 / total)
        } else {
            self.heading()
        };
        // Weights of mixed signs (a reversal within the frame) can exceed the heading: clamp.
        let hmax = self.heading[0].v.hypot(self.heading[1].v).max(steer.0.hypot(steer.1)) * 2.0 + 1.0;
        let at = if w.0.hypot(w.1) <= hmax && w.0.is_finite() && w.1.is_finite() {
            w
        } else {
            self.heading()
        };
        // Rest: nothing set and the motion eased out.
        if self.set == 0.0 && self.rate.v.abs() < REST && self.rate.a.abs() < REST * autopilot::RATE_OMEGA.1 {
            self.rate = Spring::default();
            if self
                .heading
                .iter()
                .all(|s| s.v.abs() < 0.05 && s.a.abs() < 0.05 * autopilot::STEER_OMEGA)
            {
                self.heading = [Spring::default(); 2];
                self.engaged = false;
                self.reverse_for = 0.0;
                self.boost = 1.0;
            }
        }
        if !self.engaged
            && steer == (0.0, 0.0)
            && self
                .heading
                .iter()
                .all(|s| s.v.abs() < 0.05 && s.a.abs() < 0.05 * autopilot::STEER_OMEGA)
        {
            self.heading = [Spring::default(); 2];
        }
        Motion { pan, ln_zoom, at }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_steps_with_a_detent_at_zero() {
        let r = |i| rung(i);
        // From rest: E in, Q out; a held key's repeat does not start from zero.
        assert_eq!(pedal(0.0, 1.0, true), r(0));
        assert_eq!(pedal(0.0, -1.0, true), -r(0));
        assert_eq!(pedal(0.0, -1.0, false), 0.0);
        // Up the ladder and down to the detent: never across zero.
        let mut v = 0.0;
        for i in 0..LADDER.len() {
            v = pedal(v, 1.0, true);
            assert!((v - r(i)).abs() < 1e-12, "{i}: {v}");
        }
        assert_eq!(pedal(v, 1.0, false), v, "the last rung stays");
        for _ in 0..LADDER.len() {
            v = pedal(v, -1.0, false);
        }
        assert_eq!(v, 0.0);
        assert_eq!(pedal(v, -1.0, false), 0.0, "held: stops at zero");
        assert_eq!(pedal(v, -1.0, true), -r(0), "a new press reverses");
        // Off-ladder values (the autopilot's 0.9 e-folds/s) step to the neighbouring rungs.
        assert_eq!(pedal(0.9, 1.0, true), r(6));
        assert_eq!(pedal(0.9, -1.0, true), r(5));
        assert_eq!(pedal(-0.05, 1.0, true), 0.0);
        assert_eq!(factor_text(r(0)), "×1.1");
        assert_eq!(factor_text(-r(5)), "×2");
        assert_eq!(factor_text(0.9), "×2.46");
        assert_eq!(factor_text(r(25)), "×1000");
        assert_eq!(factor_text(-1000.0), "×1e434");
    }

    /// Reverse from `depth` e-folds at 60 Hz with no steering until home (depth 0); returns
    /// the seconds and the rates seen.
    fn reverse(depth: f64, set: f64, hz: f64) -> (f64, Vec<f64>) {
        let mut c = Cruise::default();
        c.engage(set, 0.0, (0.0, 0.0), 0.0);
        let mut d = depth;
        let mut t = 0.0;
        let mut rates = Vec::new();
        for _ in 0..(3600.0 * hz) as usize {
            if d <= 0.0 {
                return (t, rates);
            }
            let m = c.step(1.0 / hz, (0.0, 0.0), d, 1.0, false);
            d += m.ln_zoom;
            t += 1.0 / hz;
            rates.push(c.rate());
        }
        panic!("never home from {depth}");
    }

    /// The boost depends on the gear: first gear crawls (no hurry from 1e10), the default-ish
    /// gears bring 1e69 home in under a minute, the top gear brings 1e3000 home in seconds;
    /// always gradual (no jumps between frames, near the base speed after 1 s).
    #[test]
    fn reverse_boost_depends_on_the_gear_and_builds_up_gradually() {
        for (gear, depth, min_t, max_t) in [
            (0, 23.0, 90.0, 180.0),
            (5, 159.0, 30.0, 60.0),
            (9, 6908.0, 50.0, 90.0),
            (25, 6908.0, 5.0, 15.0),
        ] {
            let (t, rates) = reverse(depth, -rung(gear), 60.0);
            assert!((min_t..max_t).contains(&t), "gear {} from {depth} e-folds: {t} s", gear + 1);
            let at1 = -rates[59];
            assert!(at1 < 1.2 * rung(gear), "gear {}: {at1} e-folds/s after 1 s", gear + 1);
            for w in rates.windows(2) {
                assert!((w[1] - w[0]).abs() < 0.12 * w[0].abs().max(1.0), "jump {w:?}");
            }
        }
        assert_eq!(gear(-rung(0)), Some(1));
        assert_eq!(gear(rung(25)), Some(26));
        assert_eq!(gear(0.0), None);
        assert!(boost_share(rung(0)) < boost_share(rung(10)) && boost_share(rung(25)) == 1.0);
    }

    #[test]
    fn rates_agree_at_30_60_120_hz() {
        let at = |hz: f64| {
            let mut c = Cruise::default();
            c.engage(-rung(1), 0.0, (0.0, 0.0), 0.0);
            let mut d = 6908.0;
            for _ in 0..(10.0 * hz).round() as usize {
                let m = c.step(1.0 / hz, (30.0, -10.0), d, 1.0, false);
                d += m.ln_zoom;
            }
            d
        };
        let (a, b, c) = (at(30.0), at(60.0), at(120.0));
        assert!(
            (a - b).abs() < 1e-3 * (6908.0 - a) && (a - c).abs() < 1e-3 * (6908.0 - a),
            "{a} {b} {c}"
        );
    }

    #[test]
    fn a_stop_brakes_smoothly_to_rest() {
        let mut c = Cruise::default();
        c.engage(rung(3), 0.0, (0.0, 0.0), 0.0);
        for _ in 0..240 {
            c.step(1.0 / 60.0, (0.0, 0.0), 10.0, 1.0, false);
        }
        assert!((c.rate() - rung(3)).abs() < 1e-3);
        assert!(c.stop(4.0));
        let mut last = c.rate();
        let mut frames = 0;
        while c.is_engaged() {
            c.step(1.0 / 60.0, (0.0, 0.0), 10.0, 1.0, false);
            assert!(c.rate() <= last + 1e-12 && c.rate() >= 0.0, "monotonic braking");
            last = c.rate();
            frames += 1;
            assert!(frames < 300, "never at rest");
        }
        assert!(!c.is_moving());
    }
}
