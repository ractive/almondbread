//! Flight model (kb: decisions/landscape-mode, L1). Everything is in units of the camera scale
//! S (the zoom): speed in S per second, altitude in S above the smoothed ground. All inputs go
//! through first-order easing (frame-rate independent, `1 − e^(−dt/τ)`), so a key press ramps
//! in and a release glides out, like the app's Navigator. The altitude follows the smoothed
//! ground with a critically damped spring, looking ahead along the flight path.

use crate::ground::Ground;

/// Analog input, each in −1..1 (keys give ±1, the model eases them).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// + faster, − slower
    pub throttle: f64,
    /// + turn left, − right
    pub turn: f64,
    /// + dive (zoom in), − climb (zoom out)
    pub dive: f64,
    /// + look up, − look down
    pub pitch: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct FlightCfg {
    pub clearance: f64,
    pub max_turn: f64,
    pub max_dive: f64,
    pub s_min: f64,
    pub s_max: f64,
}

impl Default for FlightCfg {
    fn default() -> Self {
        FlightCfg { clearance: 1.0, max_turn: 50f64.to_radians(), max_dive: 0.9, s_min: 1e-12, s_max: 0.5 }
    }
}

#[derive(Clone, Debug)]
pub struct Flight {
    pub cfg: FlightCfg,
    /// Camera ground position: complex offset from the world origin.
    pub pos: [f64; 2],
    /// Camera scale (complex units): the zoom; target altitude = clearance·S.
    pub s: f64,
    /// Eye height in units of S, and its rate (S/s).
    pub y: f64,
    pub vy: f64,
    pub heading: f64,
    pub yaw_rate: f64,
    pub speed: f64,
    pub speed_target: f64,
    pub dive_rate: f64,
    pub pitch_user: f64,
    pub pitch_user_target: f64,
    pub pitch: f64,
    pub bank: f64,
    /// last ground height under the camera (S units), for the HUD
    pub ground_here: f64,
    /// Dive target (world offset): the ground point the view centre aims at, snapped towards
    /// the nearest boundary. Diving zooms towards it (it stays fixed on screen).
    pub aim: Option<[f64; 2]>,
}

fn ease(x: f64, target: f64, tau: f64, dt: f64) -> f64 {
    x + (target - x) * (1.0 - (-dt / tau).exp())
}

impl Flight {
    pub fn new(s: f64, heading: f64, speed: f64) -> Self {
        Flight {
            cfg: FlightCfg::default(),
            pos: [0.0, 0.0],
            s,
            y: 1.0,
            vy: 0.0,
            heading,
            yaw_rate: 0.0,
            speed,
            speed_target: speed,
            dive_rate: 0.0,
            pitch_user: (-14f64).to_radians(),
            pitch_user_target: (-14f64).to_radians(),
            pitch: (-14f64).to_radians(),
            bank: 0.0,
            ground_here: 0.0,
            aim: None,
        }
    }

    /// Places the eye at the target altitude with no vertical motion (start, teleport).
    pub fn settle(&mut self, ground: &mut Ground) {
        let (target, _) = self.targets(ground);
        self.y = target;
        self.vy = 0.0;
    }

    fn dir(&self) -> [f64; 2] {
        [self.heading.cos(), self.heading.sin()]
    }

    /// (altitude target, hard floor) in S units.
    fn targets(&self, ground: &mut Ground) -> (f64, f64) {
        let d = self.dir();
        let s = self.s;
        let mut g_max = f64::MIN;
        // look ahead along the path (seconds); later points count a little less
        for (k, t) in [0.0, 0.5, 1.0, 1.8].iter().enumerate() {
            let p = [self.pos[0] + d[0] * self.speed * s * t, self.pos[1] + d[1] * self.speed * s * t];
            let g = ground.smooth(p, 0.35 * s, s) - 0.04 * k as f64;
            g_max = g_max.max(g);
        }
        let here_fine = ground.smooth(self.pos, s / 16.0, s);
        (g_max + self.cfg.clearance, here_fine + 0.3 * self.cfg.clearance)
    }

    /// Where the view centre meets the (smoothed, flat-approximated) ground, then up to three
    /// steps down the DE towards the nearest boundary (at most 3 S in all).
    fn aim_point(&self, ground: &mut Ground) -> [f64; 2] {
        let s = self.s;
        let down = (-self.pitch).max(0.12);
        let dist = ((self.y - self.ground_here).max(0.3) / down.tan()).min(8.0);
        let d = self.dir();
        let mut a = [self.pos[0] + d[0] * dist * s, self.pos[1] + d[1] * dist * s];
        let sc = s / 8.0;
        let mut moved = 0.0;
        for _ in 0..3 {
            let de = ground.de_smooth(a, sc);
            if de < 0.05 * s || moved > 3.0 * s {
                break;
            }
            let e = 0.25 * s;
            let gx = ground.de_smooth([a[0] + e, a[1]], sc) - ground.de_smooth([a[0] - e, a[1]], sc);
            let gy = ground.de_smooth([a[0], a[1] + e], sc) - ground.de_smooth([a[0], a[1] - e], sc);
            let gl = (gx * gx + gy * gy).sqrt();
            if gl == 0.0 {
                break;
            }
            let step = (0.9 * de).min(1.5 * s);
            a = [a[0] - gx / gl * step, a[1] - gy / gl * step];
            moved += step;
        }
        a
    }

    pub fn update(&mut self, dt: f64, input: Input, ground: &mut Ground) {
        let dt = dt.clamp(0.0, 0.1);
        // throttle: multiplicative, persists (cruise control)
        if input.throttle != 0.0 {
            self.speed_target = (self.speed_target * (input.throttle * 0.9 * dt).exp()).clamp(0.08, 8.0);
        }
        self.speed = ease(self.speed, self.speed_target, 0.6, dt);
        // turning with banking
        self.yaw_rate = ease(self.yaw_rate, input.turn * self.cfg.max_turn, 0.35, dt);
        self.heading += self.yaw_rate * dt;
        let bank_target = (self.yaw_rate * 0.6 * (self.speed / 1.5).clamp(0.3, 1.5)).clamp(-0.6, 0.6);
        self.bank = ease(self.bank, bank_target, 0.3, dt);
        // diving = zooming: S shrinks exponentially; the absolute eye height stays, so its
        // height in S units grows and the altitude spring brings it down: a smooth descent
        self.dive_rate = ease(self.dive_rate, input.dive * self.cfg.max_dive, 0.4, dt);
        if self.dive_rate.abs() > 0.01 || input.dive != 0.0 {
            let a = self.aim_point(ground);
            self.aim = Some(match self.aim {
                Some(o) => {
                    let k = 1.0 - (-dt / 0.25).exp();
                    [o[0] + (a[0] - o[0]) * k, o[1] + (a[1] - o[1]) * k]
                }
                None => a,
            });
        } else {
            self.aim = None;
        }
        let s_old = self.s;
        self.s = (self.s * (-self.dive_rate * dt).exp()).clamp(self.cfg.s_min, self.cfg.s_max);
        // zoom towards the aim point: every distance (also the altitude, which stays the same in
        // units of S) scales by S_new / S_old around it
        if let Some(a) = self.aim {
            let f = 1.0 - self.s / s_old;
            self.pos[0] += (a[0] - self.pos[0]) * f;
            self.pos[1] += (a[1] - self.pos[1]) * f;
            // and turn gently towards it
            let want = (a[1] - self.pos[1]).atan2(a[0] - self.pos[0]);
            let d = (want - self.heading + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            self.heading += d.clamp(-0.5, 0.5) * self.dive_rate.abs().min(1.0) * dt * 0.8;
        }
        // move
        let d = self.dir();
        self.pos[0] += d[0] * self.speed * self.s * dt;
        self.pos[1] += d[1] * self.speed * self.s * dt;
        // altitude: critically damped spring towards the smoothed ground ahead (semi-implicit)
        let (target, floor) = self.targets(ground);
        let w = 2.2;
        let sub = (dt / (1.0 / 240.0)).ceil().max(1.0) as usize;
        let h = dt / sub as f64;
        for _ in 0..sub {
            let a = -w * w * (self.y - target) - 2.0 * w * self.vy;
            self.vy += a * h;
            self.y += self.vy * h;
        }
        if self.y < floor {
            self.y += (floor - self.y) * (1.0 - (-dt * 14.0).exp());
            self.vy = self.vy.max(0.0);
        }
        self.ground_here = floor - 0.3 * self.cfg.clearance;
        // view pitch: user look + nose down while diving + flight-path angle
        if input.pitch != 0.0 {
            self.pitch_user_target = (self.pitch_user_target + input.pitch * 40f64.to_radians() * dt).clamp((-75f64).to_radians(), 8f64.to_radians());
        }
        self.pitch_user = ease(self.pitch_user, self.pitch_user_target, 0.2, dt);
        let path = (self.vy / self.speed.max(0.4)).atan() * 0.6;
        let want = (self.pitch_user - 0.28 * self.dive_rate + path).clamp((-80f64).to_radians(), 15f64.to_radians());
        self.pitch = ease(self.pitch, want, 0.25, dt);
    }

    /// Camera basis (forward, right, up) with bank, in world axes (x = Re, y = Im, z = up).
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let (ch, sh) = (self.heading.cos(), self.heading.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let f = [ch * cp, sh * cp, sp];
        // right = f × z, normalised
        let r0 = {
            let v = [f[1], -f[0], 0.0];
            let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
            [v[0] / l, v[1] / l, 0.0]
        };
        // up = right × f
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
    /// steer with the coast-following assist (script turns become small offsets)
    pub autopilot: bool,
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
        let i = |throttle: f64, turn: f64, dive: f64, pitch: f64| Input { throttle, turn, dive, pitch };
        let segs = match name {
            // level flight with S-turns
            "cruise" => vec![(2.0, i(0., 0., 0., 0.)), (3.0, i(0., 0.6, 0., 0.)), (3.5, i(0., -0.6, 0., 0.)), (3.5, i(0., 0., 0., 0.))],
            // a long dive with a gentle turn: five octaves of zoom
            "dive" => vec![(1.5, i(0., 0., 0., 0.)), (8.0, i(0., 0.25, 1.0, 0.)), (3.0, i(0., -0.3, 0., 0.))],
            // hard turns at speed
            "turns" => vec![(1.0, i(0.3, 0., 0., 0.)), (2.5, i(0., 1.0, 0., 0.)), (2.5, i(0., -1.0, 0., 0.)), (2.0, i(0., 0., 0., 0.))],
            // camera held still: any frame-to-frame change is shimmer
            "hover" => vec![(6.0, i(0., 0., 0., 0.))],
            // coast following: autopilot steering, scripted speed and dives
            "coast" => vec![(3.0, i(0., 0., 0., 0.)), (8.0, i(0., 0., 1.0, 0.)), (4.0, i(0., 0., 0., 0.)), (3.0, i(0., 0., -1.0, 0.))],
            // the review flight: cruise, turns, look around, dive deep, climb, dive again
            "tour" => vec![
                (2.5, i(0., 0., 0., 0.)),
                (3.0, i(0., 0.5, 0., 0.)),
                (2.0, i(0.5, -0.4, 0., 0.)),
                (6.0, i(0., -0.2, 1.0, 0.)),
                (2.0, i(0., 0.6, 0., 0.3)),
                (2.0, i(0., 0., 0., -0.3)),
                (3.0, i(0., -0.8, 0., 0.)),
                (3.0, i(0., 0., -1.0, 0.)),
                (6.0, i(0., 0.3, 1.0, 0.)),
                (2.5, i(-0.4, 0., 0., 0.)),
                (3.0, i(0., -0.6, 0.5, 0.)),
            ],
            _ => return None,
        };
        let start_speed = if name == "hover" { 0.0 } else { 1.0 };
        let autopilot = matches!(name, "tour" | "coast");
        Some(Script { name: name.into(), segs, start_speed, autopilot })
    }
}
