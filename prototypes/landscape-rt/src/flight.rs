//! Flight model (kb: decisions/landscape-mode, L1 controls). The camera flies its nose freely in
//! 3D through a rigid world; the zoom S (the world's scale relative to the pilot) is a separate
//! control. With S held, nothing in the world changes: σ is fixed, so the terrain is a fixed
//! 3D shape you can fly into, down to the valley floors. Zooming in shrinks the pilot about
//! the point the nose points at (it stays under the crosshair); zooming out grows the pilot
//! about its own ground point.
//!
//! Units: positions in complex units (f64 offsets from the world origin; z absolute), speed in
//! S per second, scaled down near the ground. Every input is eased (first order,
//! frame-rate independent), so presses ramp in and releases glide out.

use crate::ground::Ground;

/// Analog input, each in −1..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// stick: + push (nose down), − pull (nose up)
    pub pitch: f64,
    /// + turn left, − right (banked)
    pub turn: f64,
    /// + faster, − slower (cruise control, persists)
    pub throttle: f64,
    /// + zoom in (towards the crosshair), − zoom out (about the camera)
    pub zoom: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct FlightCfg {
    pub max_turn: f64,
    pub max_pitch_rate: f64,
    /// zoom rate at full input, e-folds per second
    pub max_zoom: f64,
    /// collision floor above the ground, in S
    pub clearance: f64,
    pub s_min: f64,
    pub s_max: f64,
}

impl Default for FlightCfg {
    fn default() -> Self {
        FlightCfg {
            max_turn: 55f64.to_radians(),
            max_pitch_rate: 55f64.to_radians(),
            max_zoom: 1.0,
            clearance: 0.04,
            s_min: 1e-12,
            s_max: 0.5,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Flight {
    pub cfg: FlightCfg,
    /// Camera ground position: complex offset from the world origin.
    pub pos: [f64; 2],
    /// Eye height, complex units (absolute: the world is rigid while S is held).
    pub z: f64,
    /// The zoom: camera scale in complex units.
    pub s: f64,
    pub heading: f64,
    /// Nose pitch (radians, + up), −90° = straight down.
    pub pitch: f64,
    pub pitch_rate: f64,
    pub yaw_rate: f64,
    pub bank: f64,
    pub speed: f64,
    pub speed_target: f64,
    /// Current zoom rate, e-folds per second (+ in).
    pub zoom_rate: f64,
    /// Auto-zoom mode: steep nose-down zooms in, height above ground zooms out.
    pub auto_zoom: bool,
    /// Height of the eye above the (finely smoothed) ground, in S.
    pub hag: f64,
    /// Where the nose ray meets the ground (world offset, absolute z), if it does within reach.
    pub marker: Option<[f64; 3]>,
    /// 0..1: scales the zoom-in rate (set from the DE backlog)
    pub zoom_limit: f64,
}

fn ease(x: f64, target: f64, tau: f64, dt: f64) -> f64 {
    x + (target - x) * (1.0 - (-dt / tau).exp())
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Flight {
    pub fn new(s: f64, heading: f64, speed: f64) -> Self {
        Flight {
            cfg: FlightCfg::default(),
            pos: [0.0, 0.0],
            z: s,
            s,
            heading,
            pitch: (-12f64).to_radians(),
            pitch_rate: 0.0,
            yaw_rate: 0.0,
            bank: 0.0,
            speed,
            speed_target: speed,
            zoom_rate: 0.0,
            auto_zoom: false,
            hag: 1.0,
            marker: None,
            zoom_limit: 1.0,
        }
    }

    /// Absolute ground height under p, smoothed at `scale` (complex units).
    fn ground_at(ground: &mut Ground, p: [f64; 2], scale: f64) -> f64 {
        ground.smooth(p, scale)
    }

    fn fine_scale(&self) -> f64 {
        (self.hag * self.s * 0.5).clamp(self.s / 256.0, self.s / 8.0)
    }

    /// Places the eye `alt` S above the ground (start, reset).
    pub fn settle(&mut self, ground: &mut Ground, alt: f64) {
        let g = Self::ground_at(ground, self.pos, self.s / 8.0);
        self.z = g + alt * self.s;
        self.hag = alt;
    }

    pub fn dir(&self) -> [f64; 3] {
        let (ch, sh) = (self.heading.cos(), self.heading.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        [ch * cp, sh * cp, sp]
    }

    /// March the nose ray against the smoothed ground (up to 60 S).
    pub fn nose_hit(&self, ground: &mut Ground) -> Option<[f64; 3]> {
        let d = self.dir();
        if d[2] > 0.02 {
            return None;
        }
        let s = self.s;
        let sc = (self.fine_scale() * 2.0).min(s / 8.0);
        let (mut t, mut tp) = (0.0f64, 0.0f64);
        for _ in 0..240 {
            let p = [self.pos[0] + d[0] * t, self.pos[1] + d[1] * t];
            let zr = self.z + d[2] * t;
            let gap = zr - Self::ground_at(ground, p, sc);
            if gap <= 0.0 {
                // bisect between tp and t
                let (mut a, mut b) = (tp, t);
                for _ in 0..8 {
                    let m = 0.5 * (a + b);
                    let q = [self.pos[0] + d[0] * m, self.pos[1] + d[1] * m];
                    if self.z + d[2] * m - Self::ground_at(ground, q, sc) <= 0.0 {
                        b = m;
                    } else {
                        a = m;
                    }
                }
                let m = 0.5 * (a + b);
                return Some([self.pos[0] + d[0] * m, self.pos[1] + d[1] * m, self.z + d[2] * m]);
            }
            tp = t;
            t += (0.6 * gap / (-d[2]).max(0.05)).clamp(0.01 * s, 2.0 * s);
            if t > 60.0 * s {
                break;
            }
        }
        None
    }

    /// `sigma_of(S)` gives the height curve's σ for the ground (it depends on the zoom).
    pub fn update(&mut self, dt: f64, input: Input, ground: &mut Ground, sigma_of: &dyn Fn(f64) -> f64) {
        let dt = dt.clamp(0.0, 0.1);
        // throttle (Q/E): multiplicative, persists
        if input.throttle != 0.0 {
            let base = self.speed_target.max(0.05);
            self.speed_target = (base * (input.throttle * 0.9 * dt).exp()).clamp(0.05, 8.0);
        }
        self.speed = ease(self.speed, self.speed_target, 0.5, dt);
        // bank-turn
        self.yaw_rate = ease(self.yaw_rate, input.turn * self.cfg.max_turn, 0.3, dt);
        self.heading += self.yaw_rate * dt;
        self.bank = ease(self.bank, (self.yaw_rate * 0.7).clamp(-0.6, 0.6), 0.25, dt);
        // stick pitch: rate control, down to straight down
        self.pitch_rate = ease(self.pitch_rate, -input.pitch * self.cfg.max_pitch_rate, 0.15, dt);
        self.pitch += self.pitch_rate * dt;
        let (lo, hi) = ((-90f64).to_radians(), 70f64.to_radians());
        if self.pitch < lo || self.pitch > hi {
            self.pitch = self.pitch.clamp(lo, hi);
            self.pitch_rate = 0.0;
        }

        ground.set_sigma(sigma_of(self.s));
        let g_here = Self::ground_at(ground, self.pos, self.fine_scale());
        self.hag = (self.z - g_here) / self.s;

        // zoom: manual (Shift) plus, in auto mode, steep nose-down zooms in (beyond 45°) and
        // height above ground zooms out (keeps the terrain's apparent size while you look ahead)
        let mut want = input.zoom * self.cfg.max_zoom;
        if self.auto_zoom {
            let down = (-self.pitch).to_degrees();
            want += self.cfg.max_zoom * smoothstep(45.0, 85.0, down);
            want -= self.cfg.max_zoom * smoothstep(1.5, 4.0, self.hag);
        }
        if want > 0.0 {
            want *= self.zoom_limit;
        }
        self.zoom_rate = ease(self.zoom_rate, want, 0.3, dt);
        self.marker = self.nose_hit(ground);
        let s_old = self.s;
        self.s = (self.s * (-self.zoom_rate * dt).exp()).clamp(self.cfg.s_min, self.cfg.s_max);
        let f = self.s / s_old;
        if f < 1.0 && self.marker.is_some() {
            // zoom in: shrink about the crosshair point, which stays fixed on screen
            let p = self.marker.unwrap();
            self.pos = [p[0] + (self.pos[0] - p[0]) * f, p[1] + (self.pos[1] - p[1]) * f];
            self.z = p[2] + (self.z - p[2]) * f;
        } else if f != 1.0 {
            // zoom out (or nothing ahead): about the camera's own ground point
            self.z = g_here + (self.z - g_here) * f;
        }
        ground.set_sigma(sigma_of(self.s));

        // fly the nose; speed ∝ S, and ∝ height above ground below 2/3 S, so the ground's angular
        // speed stays bounded (√ scaling measured 7 rad/s at 0.04 S: a blur)
        let d = self.dir();
        let v = self.speed * self.s * (1.5 * self.hag.max(0.0)).clamp(0.05, 1.0);
        self.pos[0] += d[0] * v * dt;
        self.pos[1] += d[1] * v * dt;
        self.z += d[2] * v * dt;
        // collision floor: the ground (finely smoothed) plus a small clearance
        let g = Self::ground_at(ground, self.pos, self.fine_scale());
        let floor = g + self.cfg.clearance * self.s;
        if self.z < floor {
            self.z = floor;
        }
        self.hag = (self.z - g) / self.s;
    }

    /// Camera basis (forward, right, up) with bank, in world axes (x = Re, y = Im, z = up).
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let f = self.dir();
        // right = f × z (heading-based, also valid looking straight down)
        let r0 = [self.heading.sin(), -self.heading.cos(), 0.0];
        let u0 = [r0[1] * f[2] - r0[2] * f[1], r0[2] * f[0] - r0[0] * f[2], r0[0] * f[1] - r0[1] * f[0]];
        let (cb, sb) = (self.bank.cos(), self.bank.sin());
        let r = [r0[0] * cb + u0[0] * sb, r0[1] * cb + u0[1] * sb, r0[2] * cb + u0[2] * sb];
        let u = [u0[0] * cb - r0[0] * sb, u0[1] * cb - r0[1] * sb, u0[2] * cb - r0[2] * sb];
        (f, r, u)
    }
}

/// A scripted flight: piecewise-constant inputs over time.
#[derive(Clone, Debug)]
pub struct Script {
    pub name: String,
    pub segs: Vec<(f64, Input)>,
    pub start_speed: f64,
    /// closed loop: point the nose at the start location's centre (a lake), fly down to it,
    /// then zoom in on it (manual zoom, or the auto-zoom mode's pitch rule)
    pub target: bool,
    pub auto_zoom: bool,
}

impl Script {
    pub fn duration(&self) -> f64 {
        self.segs.iter().map(|s| s.0).sum()
    }

    pub fn input_at(&self, t: f64) -> Input {
        let mut acc = 0.0;
        for (d, i) in &self.segs {
            acc += d;
            if t < acc {
                return *i;
            }
        }
        Input::default()
    }

    pub fn named(name: &str) -> Option<Script> {
        // i(pitch stick, turn, throttle, zoom)
        let i = |pitch: f64, turn: f64, throttle: f64, zoom: f64| Input { pitch, turn, throttle, zoom };
        let (segs, speed, target, auto) = match name {
            // level flight with S-turns at fixed zoom
            "cruise" => (vec![(2.0, i(0., 0., 0., 0.)), (3.0, i(0., 0.6, 0., 0.)), (3.5, i(0., -0.6, 0., 0.)), (3.5, i(0., 0., 0., 0.))], 1.0, false, false),
            // zoom in while flying (Shift+↑ held): five octaves
            "dive" => (vec![(1.5, i(0., 0., 0., 0.)), (0.6, i(0.6, 0., 0., 0.)), (8.0, i(0., 0.25, 0., 1.0)), (3.0, i(0., -0.3, 0., 0.))], 1.0, false, false),
            "turns" => (vec![(1.0, i(0., 0., 0.3, 0.)), (2.5, i(0., 1.0, 0., 0.)), (2.5, i(0., -1.0, 0., 0.)), (2.0, i(0., 0., 0., 0.))], 1.0, false, false),
            "hover" => (vec![(6.0, i(0., 0., 0., 0.))], 0.0, false, false),
            "lakelook" => (vec![(1.2, i(1.0, 0., 0., 0.)), (4.0, i(0., 0., 0., 0.))], 0.0, false, false),
            // fixed zoom: nose down into a valley, fly low along it, pull up over the ridge
            "valley" => (
                vec![
                    (1.0, i(0., 0., 0., 0.)),
                    (0.9, i(1.0, 0., 0., 0.)),
                    (2.2, i(0., 0., 0., 0.)),
                    (0.8, i(-1.0, 0., 0., 0.)),
                    (4.0, i(0., 0.4, 0., 0.)),
                    (0.5, i(-1.0, 0., 0., 0.)),
                    (2.5, i(0., 0., 0., 0.)),
                ],
                1.0,
                false,
                false,
            ),
            // the review flight: cruise, turns, down into the canyons, zoom in, climb, zoom out
            "tour" => (
                vec![
                    (2.0, i(0., 0., 0., 0.)),
                    (2.5, i(0., 0.5, 0., 0.)),
                    (0.8, i(1.0, 0., 0., 0.)),
                    (2.5, i(0., -0.4, 0., 0.)),
                    (0.6, i(-1.0, 0., 0., 0.)),
                    (5.0, i(0., 0.2, 0., 1.0)),
                    (2.0, i(0., -0.7, 0., 0.)),
                    (0.7, i(1.0, 0., 0., 0.)),
                    (2.0, i(0., 0., 0., 0.)),
                    (0.7, i(-1.0, 0., 0., 0.)),
                    (4.0, i(0., 0.3, 0., 1.0)),
                    (2.0, i(0., -0.5, 0.5, 0.)),
                    (3.0, i(0., 0., 0., -1.0)),
                    (4.0, i(0., 0.4, 0., 1.0)),
                ],
                1.0,
                false,
                false,
            ),
            "target" => (vec![(45.0, Input::default())], 1.0, true, false),
            "target-auto" => (vec![(45.0, Input::default())], 1.0, true, true),
            _ => return None,
        };
        Some(Script { name: name.into(), segs, start_speed: speed, target, auto_zoom: auto })
    }
}
