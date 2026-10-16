//! The compass (kb: decisions/controls, "Compass"): policy and geometry, platform-neutral.
//! The overlay draws it (`almondbread_render::compass`), the [`crate::input::Navigator`]
//! applies its motion (the eased turn back to north, drag-to-turn); this module holds the
//! maths both use, so it can be tested without a GPU.
//!
//! Angles: the view's angle is the camera's counter-clockwise turn (kb: decisions/controls
//! "View rotation"); on screen (y up, counter-clockwise from +x) north, the plane's +im
//! direction, then points at `π/2 − angle`.

use std::f64::consts::FRAC_PI_2;

use crate::viewport::wrap_angle;

/// Duration (s) of the eased turn back to north (a click on the compass).
pub const NORTH_DURATION: f64 = 0.4;
/// The compass shows while the view is turned by more than this (radians, 0.5°).
pub const SHOW_ANGLE: f64 = 0.5 * std::f64::consts::PI / 180.0;
/// After the view is back at north the compass stays this long (s), then fades out over
/// [`FADE`] (Apple Maps behaves alike).
pub const HOLD: f64 = 2.5;
pub const FADE: f64 = 0.5;
/// Radius of the rose and its margin from the canvas edges, view points.
pub const RADIUS: f64 = 22.0;
pub const MARGIN: f64 = 14.0;
/// Hits count a little outside the rose (a 22 pt rose is a small target), view points.
pub const HIT_SLOP: f64 = 5.0;
/// Pointer travel (view points) below which a press and release is a click, not a drag.
pub const CLICK_TRAVEL: f64 = 4.0;

/// Ease in and out (cubic): 0 → 1 with zero slope at both ends.
pub fn ease_in_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u / 2.0
    }
}

/// Screen angle of the north needle (radians, counter-clockwise from +x, y up) for a view
/// turned by `view_angle`.
pub fn needle_angle(view_angle: f64) -> f64 {
    FRAC_PI_2 - view_angle
}

/// The view turn (radians, added to the view's angle) that makes the needle follow the pointer
/// from `from` to `to`, both relative to the compass centre (view points, y up): 1:1, the short
/// way round. Zero when either point is too close to the centre to have a direction.
pub fn drag_turn(from: (f64, f64), to: (f64, f64)) -> f64 {
    const MIN: f64 = 1.0;
    if from.0.hypot(from.1) < MIN || to.0.hypot(to.1) < MIN {
        return 0.0;
    }
    let d = wrap_angle(to.1.atan2(to.0) - from.1.atan2(from.0));
    // The needle turns by d: its angle π/2 − angle grows by d, so the angle falls by d.
    -d
}

/// Is (x, y) on a compass of `radius` centred at `centre` (any units, the same for all)?
pub fn hit(centre: (f64, f64), radius: f64, x: f64, y: f64, slop: f64) -> bool {
    (x - centre.0).hypot(y - centre.1) <= radius + slop
}

/// The eased turn back to north: the angle at time t of the turn is `a0 · (1 − e(t / T))`
/// with `e` [`ease_in_out`]. Applied as a ratio of what is left, so motion added meanwhile
/// (the cinematic spin) is absorbed too and the turn always ends at exactly 0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NorthTurn {
    elapsed: f64,
}

impl NorthTurn {
    /// Advances by `dt` seconds from the current `angle`: the new angle and whether the turn
    /// is over (the angle is then exactly 0).
    pub fn step(&mut self, angle: f64, dt: f64) -> (f64, bool) {
        let left0 = 1.0 - ease_in_out(self.elapsed / NORTH_DURATION);
        self.elapsed += dt.max(0.0);
        if self.elapsed >= NORTH_DURATION || left0 <= 0.0 {
            return (0.0, true);
        }
        let left1 = 1.0 - ease_in_out(self.elapsed / NORTH_DURATION);
        (wrap_angle(angle) * (left1 / left0), false)
    }

    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }
}

/// When the compass shows: "always" or a turned view, then a hold and a fade after the view is
/// back at north.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Visibility {
    /// Setting "Always show compass".
    pub always: bool,
    /// The last time the compass had a reason to show (turned view, a drag, a turn to north).
    shown_at: Option<f64>,
}

impl Visibility {
    /// Follows the view: `angle` its angle, `busy` a drag or a turn to north is going on.
    pub fn update(&mut self, angle: f64, busy: bool, now: f64) {
        if self.always || busy || wrap_angle(angle).abs() > SHOW_ANGLE {
            self.shown_at = Some(now);
        }
    }

    /// Opacity now (0: hidden, nothing is drawn).
    pub fn alpha(&self, now: f64) -> f64 {
        if self.always {
            return 1.0;
        }
        let Some(t) = self.shown_at else { return 0.0 };
        let age = now - t;
        if age <= HOLD {
            1.0
        } else {
            (1.0 - (age - HOLD) / FADE).clamp(0.0, 1.0)
        }
    }

    pub fn shown(&self, now: f64) -> bool {
        self.alpha(now) > 0.0
    }

    /// Frames are needed for the fade: the compass shows but no longer has a reason to.
    pub fn fading(&self, now: f64) -> bool {
        !self.always && self.shown_at.is_some_and(|t| now - t > 0.0 && now - t < HOLD + FADE)
    }

    /// The compass may take clicks (visible enough to be seen).
    pub fn clickable(&self, now: f64) -> bool {
        self.alpha(now) > 0.25
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn north_turn_eases_to_exactly_zero_at_any_frame_rate() {
        let mut mid = Vec::new();
        for hz in [30.0, 60.0, 120.0, 240.0] {
            let mut n = NorthTurn::default();
            let mut a = 2.0;
            let mut t = 0.0;
            let mut half = None;
            let mut done = false;
            let mut steps = 0;
            while !done {
                let (b, d) = n.step(a, 1.0 / hz);
                // Monotone, no overshoot.
                assert!(b.abs() <= a.abs() + 1e-15 && b * a >= 0.0, "{a} -> {b}");
                a = b;
                done = d;
                t += 1.0 / hz;
                steps += 1;
                if half.is_none() && t >= 0.2 - 1e-6 {
                    half = Some(a);
                }
                assert!(steps < 1000);
            }
            assert_eq!(a, 0.0);
            assert!((t - NORTH_DURATION).abs() <= 1.0 / hz + 1e-9, "{t}");
            mid.push(half.unwrap());
        }
        // Halfway the ease has done half: angle(t) depends on t only.
        for m in &mid {
            assert!((m - 1.0).abs() < 1e-9, "{mid:?}");
        }
    }

    #[test]
    fn north_turn_absorbs_motion_added_meanwhile() {
        // A spin adds a little each frame; the turn still ends at exactly 0.
        let mut n = NorthTurn::default();
        let mut a = -1.0;
        loop {
            a += 0.01;
            let (b, done) = n.step(a, 1.0 / 60.0);
            a = b;
            if done {
                break;
            }
        }
        assert_eq!(a, 0.0);
        // The short way round: 350° is −10°.
        let (b, _) = NorthTurn::default().step(350f64.to_radians(), 0.01);
        assert!(b < 0.0 && b > -10f64.to_radians());
    }

    #[test]
    fn drag_turns_the_needle_with_the_pointer() {
        // A quarter turn counter-clockwise around the centre turns the needle by +90°: the
        // view's angle falls by 90°.
        let d = drag_turn((10.0, 0.0), (0.0, 10.0));
        assert!((d + FRAC_PI_2).abs() < 1e-12);
        let a0 = 0.3;
        let n0 = needle_angle(a0);
        assert!((needle_angle(a0 + d) - n0 - FRAC_PI_2).abs() < 1e-12);
        // Across the ±180° seam: the short way.
        let d = drag_turn((-10.0, 0.1), (-10.0, -0.1));
        assert!(d.abs() < 0.03, "{d}");
        // Radius does not matter, only the angle; the centre itself has no direction.
        assert!((drag_turn((1.0, 1.0), (-30.0, 30.0)) + FRAC_PI_2).abs() < 1e-12);
        assert_eq!(drag_turn((0.1, 0.0), (0.0, 10.0)), 0.0);
        // Unrotated, the needle points up.
        assert_eq!(needle_angle(0.0), FRAC_PI_2);
    }

    #[test]
    fn hit_test_is_the_rose_plus_slop() {
        let c = (100.0, 50.0);
        assert!(hit(c, RADIUS, 100.0, 50.0, HIT_SLOP));
        assert!(hit(c, RADIUS, 100.0 + RADIUS + HIT_SLOP - 0.01, 50.0, HIT_SLOP));
        assert!(!hit(c, RADIUS, 100.0 + RADIUS + HIT_SLOP + 0.01, 50.0, HIT_SLOP));
        assert!(!hit(c, RADIUS, 0.0, 0.0, HIT_SLOP));
    }

    #[test]
    fn shows_when_turned_and_fades_after_north() {
        let mut v = Visibility::default();
        v.update(0.0, false, 0.0);
        assert!(!v.shown(0.0) && !v.fading(0.0));
        v.update(0.004, false, 1.0); // 0.23°: below the threshold
        assert!(!v.shown(1.0));
        v.update(0.2, false, 2.0);
        assert!(v.shown(2.0) && v.clickable(2.0));
        v.update(0.0, false, 3.0);
        assert_eq!(v.alpha(2.0 + HOLD), 1.0);
        assert!(v.fading(2.0 + HOLD));
        assert!((v.alpha(2.0 + HOLD + FADE / 2.0) - 0.5).abs() < 1e-9);
        assert_eq!(v.alpha(2.0 + HOLD + FADE), 0.0);
        assert!(!v.fading(2.0 + HOLD + FADE + 0.1));
        // Always: shown at north, never fades.
        let v = Visibility {
            always: true,
            ..Visibility::default()
        };
        assert!(v.shown(100.0) && !v.fading(100.0));
    }

    // ---- Navigator: the motion policy (crate::input).

    use crate::cinematic::Cinematic;
    use crate::input::{InputEvent, NavKey, Navigator};
    use crate::viewport::Viewport;

    fn navigator() -> (Navigator, Viewport) {
        let mut n = Navigator::default();
        let mut v = Viewport::new(-0.5, 0.25, 1e-3);
        n.handle(
            InputEvent::Resize {
                width: 1000.0,
                height: 600.0,
            },
            &mut v,
            0.0,
        );
        (n, v)
    }

    fn run(n: &mut Navigator, v: &mut Viewport, t0: f64, t1: f64, hz: f64) -> f64 {
        let steps = ((t1 - t0) * hz).round() as usize;
        let mut t = t0;
        for i in 1..=steps {
            t = t0 + i as f64 / hz;
            n.advance(v, t);
        }
        t
    }

    #[test]
    fn navigator_turns_north_in_the_same_time_at_any_frame_rate() {
        let mut mids = Vec::new();
        for hz in [30.0, 60.0, 120.0] {
            let (mut n, mut v) = navigator();
            v.angle = -2.0;
            let c = v.center.clone();
            n.turn_north(0.0);
            assert!(n.is_moving());
            run(&mut n, &mut v, 0.0, 0.2, hz);
            mids.push(v.angle);
            run(&mut n, &mut v, 0.2, 0.5, hz);
            assert_eq!(v.angle, 0.0);
            assert!(!n.is_turning_north() && !n.is_moving());
            // About the centre: the centre stays.
            assert_eq!(v.center, c);
        }
        for m in &mids {
            assert!((m + 1.0).abs() < 1e-6, "{mids:?}");
        }
        // Turning by hand ends it.
        let (mut n, mut v) = navigator();
        v.angle = 1.0;
        n.turn_north(0.0);
        n.handle(InputEvent::KeyDown(NavKey::RotateCw), &mut v, 0.0);
        assert!(!n.is_turning_north());
    }

    #[test]
    fn navigator_drag_turns_the_view_about_its_centre() {
        let (mut n, mut v) = navigator();
        let c = v.center.clone();
        n.turn_north(0.0);
        let d = drag_turn((10.0, 0.0), (0.0, 10.0));
        n.turn_view(d, &mut v, 0.0);
        assert!((v.angle + FRAC_PI_2).abs() < 1e-12);
        assert_eq!(v.center, c);
        assert!(!n.is_turning_north(), "a drag takes over from the turn to north");
    }

    #[test]
    fn north_during_a_spinning_autopilot_straightens_then_spins_on() {
        let (mut n, mut v) = navigator();
        n.set_cinematic(Cinematic {
            enabled: true,
            ..Cinematic::default()
        });
        n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, 0.0);
        let t = run(&mut n, &mut v, 0.0, 2.0, 60.0);
        v.angle = 1.2;
        n.turn_north(t);
        let t = run(&mut n, &mut v, t, t + NORTH_DURATION + 1.0 / 60.0, 60.0);
        assert!(n.autopilot().is_active());
        // At most one frame of spin since the turn ended.
        assert!(v.angle.abs() < 0.01, "{}", v.angle);
        let a = v.angle;
        run(&mut n, &mut v, t, t + 1.0, 60.0);
        assert!(v.angle > a + 0.005, "the spin goes on from north: {} -> {}", a, v.angle);
    }

    #[test]
    fn north_during_a_spinning_flight_straightens_and_the_flight_goes_on() {
        let (mut n, mut v) = navigator();
        n.set_cinematic(Cinematic {
            enabled: true,
            ..Cinematic::default()
        });
        let target = Viewport::new(-0.743643887037151, 0.131825904205330, 1e-9);
        let start = v.clone();
        n.fly_to(&target, &start, 0.0);
        let t = run(&mut n, &mut v, 0.0, 1.0, 60.0);
        assert!(n.is_flying());
        let land0 = n.flight().unwrap().land_angle;
        let a0 = v.angle;
        n.turn_north(t);
        let mut tt = t;
        let mut min_abs = f64::MAX;
        while tt < t + NORTH_DURATION + 0.05 {
            tt += 1.0 / 60.0;
            n.advance(&mut v, tt);
            min_abs = min_abs.min(v.angle.abs());
        }
        assert!(n.is_flying(), "the flight goes on");
        assert_eq!(min_abs, 0.0, "it reached north");
        // The landing turned with it: back by the angle the view had, plus the spin the
        // turn absorbed meanwhile.
        let land1 = n.flight().unwrap().land_angle;
        let shift = crate::viewport::wrap_angle(land1 - land0);
        assert!(shift < -a0 && shift > -a0 - 0.15, "{shift} vs {a0}");
        // It still lands exactly on its (turned) angle.
        while n.is_flying() && tt < 120.0 {
            tt += 1.0 / 60.0;
            n.advance(&mut v, tt);
        }
        assert_eq!(v.angle, land1);
    }
}
