//! Scripted flights: the JSON flight scripts of `almondbread fly` and the flight benchmark
//! (kb: synthesis/flight-benchmark, synthesis/benchmarks FL), played into any renderer through
//! [`FlightTarget`], plus [`LiveRecorder`], the real-time measurements of a flight (mode B of
//! the benchmark: headless through the scheduler, or in the app's window).
//!
//! Script (JSON): `size`, `fps`, `duration` (s), `start` (`re`, `im` decimal strings and
//! `zoom` relative to the fitted home view, or `scale` in complex units per script unit),
//! `settings` (as in [`apply_json_settings`]), `events` and, for benchmarks, `segments`
//! (`name`, `from`, `to`, `landings`: times a jump happens or input stops) and a
//! `benchmark` object (`name`, `version`, `step_hz`, `hz`). Positions and speeds are in
//! script units: pixels of the script's `size` (1 point per pixel headless); a window of
//! another size scales them with [`FlightPlayer::unit`]. Each event is either at a time
//! (`at`) or over an interval (`from`, `to`, applied every refresh):
//!
//! | event | meaning |
//! | --- | --- |
//! | `{"at": t, "key": "zoom", "down": true}` | key down / up (`down` omitted: a tap). Keys: up down left right w a s d zoom space slower q faster e in out reset autopilot z stop esc |
//! | `{"at": t, "shift": true}` | Shift state (Shift+Space zooms out) |
//! | `{"at": t, "pointer": [x, y]}` / `null` | pointer position in points from the centre (y up), or off the canvas |
//! | `{"from": a, "to": b, "pointer_to": [x, y]}` | pointer moves linearly to (x, y) |
//! | `{"from": a, "to": b, "pan": [vx, vy], "momentum": true, "release": true}` | 1:1 pan at vx, vy points/s; then trackpad-like OS momentum (τ 0.35 s) or a release (fling) |
//! | `{"from": a, "to": b, "magnify": rate, "point": [x, y], "release": true}` | trackpad pinch at `rate` e-folds/s |
//! | `{"from": a, "to": b, "pinch": rate, "point": [x, y], "release": true}` | touch pinch |
//! | `{"at": t, "wheel": notches, "point": [x, y]}` | mouse wheel notches |
//! | `{"at": t, "double_click": [x, y], "out": false}` | double click |
//! | `{"from": a, "to": b, "gamepad": {"lx": .., "ly": .., "ry": .., "lt": .., "rt": .., "boost": ..}}` | controller state held over the interval |
//! | `{"at": t, "settings": {...}}` | settings change mid-flight |
//! | `{"at": t, "click": [x, y]}` | single click (autopilot: pause / resume) |
//! | `{"from": a, "to": b, "scroll": [vx, vy]}` | trackpad scroll at vx, vy points/s (autopilot: speed) |
//! | `{"from": a, "to": b, "aim": {"re": "..", "im": ".."}, "gain": 2.5, "lag": 0.25}` | a scripted pilot: the pointer follows `gain` × the target's screen offset with a first-order lag (s), like a hand keeping the target in view |
//! | `{"at": t, "lock": false}` | autopilot target lock on / off (on by default); `"lock_synchronous": true` searches on the calling thread (reproducible, slower frames) |
//! | `{"at": t, "fly_to": {"re": "..", "im": "..", "scale": "1e-34"}}` | fly to coordinates (scale per script unit) |
//! | `{"at": t, "jump": {"re": "..", "im": "..", "scale": "1e-1000"}}` | go to coordinates (no animation, like pasting coordinates); `{"location": "spiral-32412-1e19", "pixels": 1920}` takes a benchmark location framed as at that width (also for `fly_to`); `{"place": "spiral-1e1000"}` a place from the catalogue framed as at `pixels` × `rows` (1920 × 1080) |
//! | `{"at": t, "journey": 4711}` / `{"at": t, "journey": "daily", "date": "2026-10-15"}` | start a journey with that seed (kb: decisions/controls, journey); keys `j` (resume / stop), `rotate_ccw` `,`, `rotate_cw` `.`, `straighten` `/` |
//! | `{"from": a, "to": b, "rotate": rate}` | trackpad rotation at `rate` radians/s (counter-clockwise image) about `point` |
//!
//! Settings also take `"cinematic": true` or `{"spin": 4, "hue_drift": 2.5, "phase_drift": 0.01, "fly_cycle": 0.02}`
//! (kb: decisions/controls, cinematic effects).

use serde_json::{Value, json};

use crate::input::{InputEvent, NavKey, Navigator};
use crate::settings::{RenderSettings, Supersampling};
use crate::viewport::{Center, Scale, Viewport};

/// What a flight script drives: the renderer headless, or the app's host (whose `input`
/// also records input-to-photon latency).
pub trait FlightTarget {
    fn input(&mut self, event: InputEvent, now: f64);
    fn navigator(&mut self) -> &mut Navigator;
    fn viewport(&mut self) -> &mut Viewport;
    fn settings(&mut self) -> &mut RenderSettings;
}

/// A named part of a benchmark flight, `from`..`to` seconds. At each of its `landings` a
/// jump happens or the input stops; time to first image and time to sharp are measured from
/// there until the next landing or the segment's end.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub name: String,
    pub from: f64,
    pub to: f64,
    pub landings: Vec<f64>,
}

impl Segment {
    /// The time a landing at `at` has to get sharp: until the next landing or the end.
    pub fn window_end(&self, at: f64) -> f64 {
        self.landings
            .iter()
            .copied()
            .filter(|&l| l > at)
            .fold(self.to, f64::min)
    }
}

/// The `benchmark` object of a flight script.
#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkSpec {
    pub name: String,
    pub version: u32,
    /// Mode A (throughput): camera samples per second of flight, each rendered to settled.
    pub step_hz: f64,
    /// Mode B (real time) headless: the refresh rate it is paced at.
    pub hz: f64,
}

/// One flight script.
#[derive(Clone, Debug)]
pub struct FlightScript {
    pub size: (usize, usize),
    pub fps: f64,
    pub duration: f64,
    pub start: Option<Value>,
    pub settings: Option<Value>,
    pub events: Vec<Value>,
    pub segments: Vec<Segment>,
    pub benchmark: Option<BenchmarkSpec>,
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn pair(v: Option<&Value>) -> Option<(f64, f64)> {
    let a = v?.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

/// `WxH`.
pub fn parse_size(s: &str) -> Option<(usize, usize)> {
    let parts: Vec<usize> = s.split('x').filter_map(|p| p.parse().ok()).collect();
    (parts.len() == 2 && parts[0] > 0 && parts[1] > 0).then(|| (parts[0], parts[1]))
}

impl FlightScript {
    pub fn parse(text: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        Self::from_value(&v)
    }

    pub fn from_value(v: &Value) -> Result<Self, String> {
        let size = match v.get("size").and_then(Value::as_str) {
            Some(s) => parse_size(s).ok_or_else(|| format!("bad size {s}"))?,
            None => (1920, 1080),
        };
        let segments = v
            .get("segments")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|s| Segment {
                        name: s.get("name").and_then(Value::as_str).unwrap_or("?").to_string(),
                        from: num(s, "from").unwrap_or(0.0),
                        to: num(s, "to").unwrap_or(0.0),
                        landings: match s.get("landings") {
                            Some(Value::Array(a)) => a.iter().filter_map(Value::as_f64).collect(),
                            Some(v) => v.as_f64().into_iter().collect(),
                            None => Vec::new(),
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        let benchmark = v.get("benchmark").map(|b| BenchmarkSpec {
            name: b.get("name").and_then(Value::as_str).unwrap_or("flight").to_string(),
            version: b.get("version").and_then(Value::as_u64).unwrap_or(1) as u32,
            step_hz: num(b, "step_hz").unwrap_or(10.0),
            hz: num(b, "hz").unwrap_or(120.0),
        });
        Ok(Self {
            size,
            fps: num(v, "fps").unwrap_or(60.0),
            duration: num(v, "duration").unwrap_or(10.0),
            start: v.get("start").cloned(),
            settings: v.get("settings").cloned(),
            events: v.get("events").and_then(Value::as_array).cloned().unwrap_or_default(),
            segments,
            benchmark,
        })
    }

    /// The segment containing flight time `t` (the last one past the end).
    pub fn segment_at(&self, t: f64) -> Option<usize> {
        if self.segments.is_empty() {
            return None;
        }
        Some(
            self.segments
                .iter()
                .position(|s| t >= s.from && t < s.to)
                .unwrap_or(if t < self.segments[0].from {
                    0
                } else {
                    self.segments.len() - 1
                }),
        )
    }
}

pub fn nav_key(name: &str) -> Option<NavKey> {
    Some(match name {
        "up" | "w" => NavKey::Up,
        "down" | "s" => NavKey::Down,
        "left" | "a" => NavKey::Left,
        "right" | "d" => NavKey::Right,
        "zoom" | "space" => NavKey::Zoom,
        "slower" | "q" => NavKey::SlowerZoom,
        "faster" | "e" => NavKey::FasterZoom,
        "in" | "+" => NavKey::StepIn,
        "out" | "-" => NavKey::StepOut,
        "reset" | "r" => NavKey::Reset,
        "autopilot" | "z" => NavKey::Autopilot,
        "stop" | "esc" => NavKey::Stop,
        "rotate_ccw" | "," => NavKey::RotateCcw,
        "rotate_cw" | "." => NavKey::RotateCw,
        "straighten" | "/" => NavKey::Straighten,
        "journey" | "j" => NavKey::Journey,
        _ => return None,
    })
}

/// Settings from a JSON object: the CLI option names (`palette`, `shading`, `outline`,
/// `density`, `offset`, `log`, `ss`, `iterations`, `interior`) and the colouring fields
/// ([`apply_look_json`]).
pub fn apply_json_settings(v: &Value, s: &mut RenderSettings) {
    if let Some(l) = v
        .get("look")
        .and_then(Value::as_str)
        .and_then(crate::palette::Look::named)
    {
        l.apply(s);
    }
    let f = |k: &str| v.get(k).and_then(Value::as_f64);
    let b = |k: &str| v.get(k).and_then(Value::as_bool);
    if let Some(p) = v.get("palette").and_then(Value::as_str) {
        s.palette_id = p.to_string();
    }
    if let Some(x) = f("shading") {
        s.shading = x;
    }
    if let Some(x) = f("outline") {
        s.outline = x;
    }
    if let Some(x) = f("density") {
        s.color_density = x;
    }
    if let Some(x) = f("offset") {
        s.color_offset = x;
    }
    if let Some(x) = b("log") {
        s.log_coloring = x;
    }
    if let Some(x) = f("ss").and_then(|x| Supersampling::from_factor(x as usize)) {
        s.supersampling = x;
    }
    if let Some(x) = f("iterations") {
        s.auto_iterations = false;
        s.manual_iterations = x as usize;
        s.max_iterations_cap = s.max_iterations_cap.max(x as usize);
    }
    if let Some(x) = b("interior") {
        s.interior_detection = x;
    }
    if let Some(x) = b("preview") {
        s.interactive_preview = x;
    }
    match v.get("cinematic") {
        Some(Value::Bool(on)) => s.cinematic.enabled = *on,
        Some(c @ Value::Object(_)) => {
            let g = |k: &str| c.get(k).and_then(Value::as_f64);
            s.cinematic.enabled = c.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            if let Some(x) = g("spin") {
                s.cinematic.spin = x;
            }
            if let Some(x) = g("hue_drift") {
                s.cinematic.hue_drift = x;
            }
            if let Some(x) = g("phase_drift") {
                s.cinematic.phase_drift = x;
            }
            if let Some(x) = g("fly_cycle") {
                s.cinematic.fly_cycle = x;
            }
            s.cinematic = s.cinematic.sanitized();
        }
        _ => {}
    }
    apply_look_json(v, s);
}

/// Colouring ("look") settings from a JSON object: `look` (a look id, applied first),
/// `specular`, `shininess`, `light_height`, `light_angle`, `glow`, `glow_width`,
/// `outline_width`, `palette_filter`, `cycle` (cycles per second), `depth_tint`.
pub fn apply_look_json(v: &Value, s: &mut RenderSettings) {
    if let Some(l) = v
        .get("look")
        .and_then(Value::as_str)
        .and_then(crate::palette::Look::named)
    {
        l.apply(s);
    }
    let f = |k: &str| v.get(k).and_then(Value::as_f64);
    if let Some(x) = f("specular") {
        s.specular = x;
    }
    if let Some(x) = f("shininess") {
        s.shininess = x;
    }
    if let Some(x) = f("light_height") {
        s.light_height = x;
    }
    if let Some(x) = f("light_angle") {
        s.light_angle = x;
    }
    if let Some(x) = f("glow") {
        s.glow = x;
    }
    if let Some(x) = f("glow_width") {
        s.glow_width = x;
    }
    if let Some(x) = f("outline_width") {
        s.outline_width = x;
    }
    if let Some(x) = v.get("palette_filter").and_then(Value::as_bool) {
        s.palette_filter = x;
    }
    if let Some(x) = f("cycle") {
        s.cycle_speed = x;
    }
    if let Some(x) = f("depth_tint") {
        s.depth_tint = x;
    }
}

fn text_of(v: &Value, k: &str) -> Option<String> {
    v.get(k)
        .map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string()))
}

/// The start view of a script in a view of `w_pts` x `h_pts` points, `unit` points per
/// script unit (a `scale` in the script is per script unit).
pub fn start_viewport(start: Option<&Value>, w_pts: f64, h_pts: f64, unit: f64) -> Result<Viewport, String> {
    let fitted = Viewport::fitting(w_pts, h_pts);
    let Some(s) = start else { return Ok(fitted) };
    let center = match (text_of(s, "re"), text_of(s, "im")) {
        (Some(re), Some(im)) => Center::parse(&re, &im).ok_or_else(|| "bad start".to_string())?,
        _ => fitted.center.clone(),
    };
    let scale = match (
        text_of(s, "scale").and_then(|t| Scale::parse(t.trim_matches('"'))),
        num(s, "zoom"),
    ) {
        (Some(sc), _) => sc / unit,
        (None, Some(z)) => fitted.scale / z,
        _ => fitted.scale,
    };
    let mut v = Viewport::with_center(center, scale);
    v.ensure_precision();
    Ok(v)
}

/// Plays a script's events into a [`FlightTarget`].
#[derive(Clone, Debug)]
pub struct FlightPlayer {
    /// Pointer position (view points from the centre, y up).
    pub pointer: (f64, f64),
    /// View points per script unit (1 headless; a window scales the script to its size).
    pub unit: f64,
}

impl Default for FlightPlayer {
    fn default() -> Self {
        Self {
            pointer: (0.0, 0.0),
            unit: 1.0,
        }
    }
}

impl FlightPlayer {
    pub fn new(unit: f64) -> Self {
        Self {
            pointer: (0.0, 0.0),
            unit,
        }
    }

    fn pt(&self, v: Option<&Value>) -> Option<(f64, f64)> {
        pair(v).map(|(x, y)| (x * self.unit, y * self.unit))
    }

    /// Feeds the events due in (t0, t1] (flight seconds) at time `now`.
    pub fn feed(&mut self, r: &mut dyn FlightTarget, events: &[Value], t0: f64, t1: f64, now: f64) {
        for e in events {
            let at = num(e, "at");
            let due = at.is_some_and(|a| a > t0 && a <= t1) || (t0 < 0.0 && at == Some(0.0));
            let (from, to) = (num(e, "from"), num(e, "to"));
            let during = matches!((from, to), (Some(a), Some(b)) if t1 > a && t0 < b);
            let ends = matches!(to, Some(b) if b > t0 && b <= t1);
            if due {
                self.feed_at(r, e, now);
            }
            if during {
                self.feed_during(r, e, t0, t1, from.unwrap_or(0.0), to.unwrap_or(0.0), now);
            }
            // Trackpad-like momentum after a pan: the OS keeps sending decaying scrolls.
            if let (Some(b), Some((vx, vy))) = (to, self.pt(e.get("pan")))
                && e.get("momentum").and_then(Value::as_bool) == Some(true)
                && t1 > b
            {
                let (s0, s1) = ((t0 - b).max(0.0), t1 - b);
                let tau = 0.35;
                let k = tau * ((-s0 / tau).exp() - (-s1 / tau).exp());
                if vx.hypot(vy) * (-s1 / tau).exp() > 5.0 * self.unit {
                    r.input(InputEvent::Pan { dx: vx * k, dy: vy * k }, now);
                }
            }
            if ends {
                if e.get("release").and_then(Value::as_bool) == Some(true) {
                    r.input(InputEvent::GestureEnd, now);
                }
                if e.get("gamepad").is_some() {
                    r.input(
                        InputEvent::Gamepad {
                            lx: 0.0,
                            ly: 0.0,
                            rx: 0.0,
                            ry: 0.0,
                            lt: 0.0,
                            rt: 0.0,
                            boost: false,
                        },
                        now,
                    );
                }
            }
        }
    }

    fn view_text(&self, f: &Value) -> Option<Viewport> {
        let text = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("");
        if let Some(id) = f.get("place").and_then(Value::as_str) {
            // A place from the catalogue, framed as at `pixels` × `rows` (1920 × 1080).
            let w = f.get("pixels").and_then(Value::as_u64).unwrap_or(1920) as usize;
            let h = f.get("rows").and_then(Value::as_u64).unwrap_or(1080) as usize;
            let mut v = crate::places::place_named(id)?.viewport(w, h);
            v.ensure_precision();
            if self.unit != 1.0 {
                v.scale = v.scale / self.unit;
                v.ensure_precision();
            }
            return Some(v);
        }
        let mut v = match f.get("location").and_then(Value::as_str) {
            // A benchmark location (`almondbread bench`), framed as at `pixels` (1920) wide.
            Some(name) => {
                let pixels = f.get("pixels").and_then(Value::as_u64).unwrap_or(1920) as usize;
                let mut v = crate::locations::location_named(name)?.viewport_for_width(pixels);
                v.ensure_precision();
                v
            }
            None => crate::flyto::view_from_text(text("re"), text("im"), text("scale"))?,
        };
        if self.unit != 1.0 {
            v.scale = v.scale / self.unit;
            v.ensure_precision();
        }
        Some(v)
    }

    fn feed_at(&mut self, r: &mut dyn FlightTarget, e: &Value, now: f64) {
        if let Some(k) = e.get("key").and_then(Value::as_str).and_then(nav_key) {
            match e.get("down").and_then(Value::as_bool) {
                Some(true) => r.input(InputEvent::KeyDown(k), now),
                Some(false) => r.input(InputEvent::KeyUp(k), now),
                None => {
                    r.input(InputEvent::KeyDown(k), now);
                    r.input(InputEvent::KeyUp(k), now);
                }
            }
        }
        if let Some(s) = e.get("shift").and_then(Value::as_bool) {
            r.input(InputEvent::Shift(s), now);
        }
        if let Some(p) = e.get("pointer") {
            let p = self.pt(Some(p));
            if let Some(p) = p {
                self.pointer = p;
            }
            r.input(InputEvent::Pointer(p), now);
        }
        if let Some(n) = num(e, "wheel") {
            let (x, y) = self.pt(e.get("point")).unwrap_or(self.pointer);
            r.input(InputEvent::WheelNotch { delta: n, x, y }, now);
        }
        if let Some((x, y)) = self.pt(e.get("double_click")) {
            let out = e.get("out").and_then(Value::as_bool).unwrap_or(false);
            r.input(InputEvent::DoubleTap { x, y, out }, now);
        }
        if let Some(s) = e.get("settings") {
            apply_json_settings(s, r.settings());
        }
        if let Some((x, y)) = self.pt(e.get("click")) {
            r.input(InputEvent::Click { x, y }, now);
        }
        if let Some(a) = e.get("lock").and_then(Value::as_bool) {
            r.navigator().autopilot_mut().set_lock(a);
        }
        if let Some(a) = e.get("lock_synchronous").and_then(Value::as_bool) {
            r.navigator().autopilot_mut().lock_mut().synchronous = a;
        }
        if let Some(j) = e.get("journey") {
            let start = r.viewport().clone();
            if j.as_str() == Some("daily") {
                let date = e.get("date").and_then(Value::as_str).unwrap_or("2026-10-15");
                let p: Vec<i64> = date.split('-').filter_map(|x| x.parse().ok()).collect();
                let seed = if p.len() == 3 {
                    crate::journey::daily_seed(p[0] as i32, p[1] as u32, p[2] as u32)
                } else {
                    1
                };
                r.navigator().start_journey(seed, true, &start, now);
            } else {
                let seed = j.as_u64().unwrap_or(1);
                r.navigator().start_journey(seed, false, &start, now);
            }
        }
        if let Some(f) = e.get("fly_to") {
            match self.view_text(f) {
                Some(target) => {
                    let start = r.viewport().clone();
                    r.navigator().fly_to(&target, &start, now);
                }
                None => eprintln!("flight: bad fly_to target {f}"),
            }
        }
        if let Some(f) = e.get("jump") {
            // "Go to coordinates": the view is replaced, like the app's coordinate paste.
            match self.view_text(f) {
                Some(mut target) => {
                    target.angle = num(f, "angle").map_or(0.0, f64::to_radians);
                    r.navigator().cancel_flight();
                    *r.viewport() = target;
                }
                None => eprintln!("flight: bad jump target {f}"),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn feed_during(&mut self, r: &mut dyn FlightTarget, e: &Value, t0: f64, t1: f64, a: f64, b: f64, now: f64) {
        // The part of this refresh inside the interval.
        let span = t1.min(b) - t0.max(a);
        if let Some((vx, vy)) = self.pt(e.get("pan")) {
            r.input(
                InputEvent::Pan {
                    dx: vx * span,
                    dy: vy * span,
                },
                now,
            );
        }
        if let Some(rate) = num(e, "magnify") {
            let (x, y) = self.pt(e.get("point")).unwrap_or(self.pointer);
            r.input(
                InputEvent::Magnify {
                    magnification: (rate * span).exp() - 1.0,
                    x,
                    y,
                },
                now,
            );
        }
        if let Some(rate) = num(e, "rotate") {
            let (x, y) = self.pt(e.get("point")).unwrap_or(self.pointer);
            r.input(
                InputEvent::Rotate {
                    radians: rate * span,
                    x,
                    y,
                },
                now,
            );
        }
        if let Some(rate) = num(e, "pinch") {
            let (x, y) = self.pt(e.get("point")).unwrap_or(self.pointer);
            let (vx, vy) = self.pt(e.get("pan")).unwrap_or((0.0, 0.0));
            r.input(
                InputEvent::Pinch {
                    ratio: (rate * span).exp(),
                    x,
                    y,
                    dx: vx * span,
                    dy: vy * span,
                },
                now,
            );
        }
        if let Some((vx, vy)) = self.pt(e.get("scroll")) {
            r.input(
                InputEvent::Scroll {
                    dx: vx * span,
                    dy: vy * span,
                    momentum: false,
                },
                now,
            );
        }
        if let Some(aim) = e.get("aim") {
            // The target's offset on screen (points, y up), scaled by the pilot's gain and
            // followed with a lag: the pointer path a hand would take keeping it in view.
            let text = |k: &str| aim.get(k).and_then(Value::as_str).unwrap_or("0");
            if let Some(c) = Center::parse(text("re"), text("im")) {
                let (ox, oy) = r.viewport().offset_of(&c);
                let gain = num(e, "gain").unwrap_or(2.5);
                let lag = num(e, "lag").unwrap_or(0.25).max(1e-3);
                let f = 1.0 - (-span / lag).exp();
                // A hand stays on the window: the pointer is clamped to the canvas.
                let (w, h) = r.navigator().view_size();
                let (tx, ty) = (
                    (ox * gain).clamp(-0.48 * w, 0.48 * w),
                    (oy * gain).clamp(-0.48 * h, 0.48 * h),
                );
                let p = self.pointer;
                self.pointer = (p.0 + (tx - p.0) * f, p.1 + (ty - p.1) * f);
                r.input(InputEvent::Pointer(Some(self.pointer)), now);
            }
        }
        if let Some((x1, y1)) = self.pt(e.get("pointer_to")) {
            // Linear from where the pointer is to the end point by `b`.
            let left = (b - t0.max(a)).max(1e-9);
            let f = (span / left).clamp(0.0, 1.0);
            let p = self.pointer;
            self.pointer = (p.0 + (x1 - p.0) * f, p.1 + (y1 - p.1) * f);
            r.input(InputEvent::Pointer(Some(self.pointer)), now);
        }
        if let Some(g) = e.get("gamepad")
            && t0 <= a
        {
            let gf = |k: &str| g.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            r.input(
                InputEvent::Gamepad {
                    lx: gf("lx"),
                    ly: gf("ly"),
                    rx: gf("rx"),
                    ry: gf("ry"),
                    lt: gf("lt"),
                    rt: gf("rt"),
                    boost: g.get("boost").and_then(Value::as_bool).unwrap_or(false),
                },
                now,
            );
        }
    }
}

/// A plain navigator + viewport + settings, for computing a script's camera path without a
/// renderer (mode A of the flight benchmark samples it).
pub struct CameraRig {
    pub navigator: Navigator,
    pub viewport: Viewport,
    pub settings: RenderSettings,
}

impl FlightTarget for CameraRig {
    fn input(&mut self, event: InputEvent, now: f64) {
        self.navigator.handle(event, &mut self.viewport, now);
    }
    fn navigator(&mut self) -> &mut Navigator {
        &mut self.navigator
    }
    fn viewport(&mut self) -> &mut Viewport {
        &mut self.viewport
    }
    fn settings(&mut self) -> &mut RenderSettings {
        &mut self.settings
    }
}

/// One camera sample of a flight: flight time and view.
#[derive(Clone, Debug)]
pub struct CameraSample {
    pub t: f64,
    pub viewport: Viewport,
}

/// The camera path of `script` in a `size` view (1 point per pixel), the navigator stepped at
/// `hz` virtual refreshes per second, sampled `step_hz` times per second. No renderer feeds
/// back (autopilot and fly-to throttles never engage), so the path is deterministic.
pub fn camera_path(
    script: &FlightScript,
    base: &RenderSettings,
    size: (usize, usize),
    hz: f64,
    step_hz: f64,
) -> Result<Vec<CameraSample>, String> {
    let (w, h) = (size.0 as f64, size.1 as f64);
    let mut rig = CameraRig {
        navigator: Navigator::default(),
        viewport: Viewport::fitting(w, h),
        settings: base.clone(),
    };
    if let Some(s) = &script.settings {
        apply_json_settings(s, &mut rig.settings);
    }
    let base_t = 1000.0;
    rig.navigator
        .handle(InputEvent::Resize { width: w, height: h }, &mut rig.viewport, base_t);
    rig.viewport = start_viewport(script.start.as_ref(), w, h, 1.0)?;
    let mut player = FlightPlayer::new(1.0);
    let refreshes = (script.duration * hz).round() as usize;
    let every = (hz / step_hz).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(refreshes / every + 1);
    for k in 0..refreshes {
        let t = k as f64 / hz;
        let t_prev = if k == 0 { -1.0 / hz } else { (k - 1) as f64 / hz };
        let now = base_t + t;
        player.feed(&mut rig, &script.events, t_prev, t, now);
        let CameraRig {
            navigator,
            viewport,
            settings,
        } = &mut rig;
        navigator.set_cinematic(settings.cinematic);
        navigator.advance(viewport, now);
        if k % every == 0 {
            out.push(CameraSample {
                t,
                viewport: rig.viewport.clone(),
            });
        }
    }
    Ok(out)
}

// MARK: Real-time measurements (mode B)

/// One display refresh as the recorder sees it.
#[derive(Clone, Copy, Debug)]
struct Refresh {
    t: f64,
    now: f64,
    moving: bool,
    committed: bool,
}

/// A landing: input stopped (or a jump) at `at` (absolute time).
#[derive(Clone, Debug)]
struct Landing {
    segment: usize,
    /// Flight time of the landing, and the same as absolute time.
    t: f64,
    at: f64,
    first_image: Option<f64>,
    sharp: Option<f64>,
}

/// Real-time measurements of one lap of a flight (mode B): frames due while the camera moves,
/// frames shown, frame-time percentiles, late frames, and time to first image and to sharp
/// after each landing. The caller reports every refresh ([`LiveRecorder::refresh`]) and, if
/// the platform has them, presentation times ([`LiveRecorder::presented`]).
#[derive(Clone, Debug)]
pub struct LiveRecorder {
    pub hz: f64,
    segments: Vec<Segment>,
    duration: f64,
    start: f64,
    refreshes: Vec<Refresh>,
    shown: Vec<f64>,
    landings: Vec<Landing>,
    /// Refreshes the loop reached more than half an interval late (headless pacing).
    pub late_refreshes: usize,
}

fn pct(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn finite(x: f64) -> Value {
    if x.is_finite() { json!(x) } else { Value::Null }
}

impl LiveRecorder {
    /// A lap of `script` starting at absolute time `start`, on a display at `hz`.
    pub fn new(script: &FlightScript, hz: f64, start: f64) -> Self {
        Self {
            hz,
            segments: script.segments.clone(),
            duration: script.duration,
            start,
            refreshes: Vec::new(),
            shown: Vec::new(),
            landings: Vec::new(),
            late_refreshes: 0,
        }
    }

    /// Whether a landing falls into the flight-time interval (t0, t1]: the caller resets its
    /// first-image marker then.
    pub fn landing_in(&self, t0: f64, t1: f64) -> bool {
        self.segments
            .iter()
            .any(|s| s.landings.iter().any(|&l| l > t0 && l <= t1))
    }

    /// A refresh at flight time `t` (absolute `now`): the camera moved in it, a frame was
    /// committed, the view is settled (refined, anti-aliased), and the absolute time of the
    /// first frame computed for the current view since the last landing (if any).
    pub fn refresh(
        &mut self,
        t: f64,
        now: f64,
        moving: bool,
        committed: bool,
        settled: bool,
        first_image: Option<f64>,
    ) {
        let t_prev = self.refreshes.last().map_or(-1.0, |r| r.t);
        for (i, s) in self.segments.iter().enumerate() {
            for &l in &s.landings {
                if l > t_prev && l <= t {
                    self.landings.push(Landing {
                        segment: i,
                        t: l,
                        at: self.start + l,
                        first_image: None,
                        sharp: None,
                    });
                }
            }
        }
        self.refreshes.push(Refresh {
            t,
            now,
            moving,
            committed,
        });
        let Some(l) = self.landings.last_mut() else { return };
        let end = self.start + self.segments[l.segment].window_end(l.t);
        if now > end {
            return;
        }
        if l.first_image.is_none()
            && let Some(f) = first_image
            && f >= l.at
        {
            l.first_image = Some(f - l.at);
        }
        if l.sharp.is_none() && settled && !moving && now >= l.at {
            l.sharp = Some(now - l.at);
            l.first_image.get_or_insert(now - l.at);
        }
    }

    /// A frame reached the screen at `time` (absolute).
    pub fn presented(&mut self, time: f64) {
        self.shown.push(time);
    }

    /// The lap's numbers as JSON: totals, per segment, per landing.
    pub fn report(&self) -> Value {
        let period = 1.0 / self.hz;
        // Motion runs: consecutive refreshes in which the camera moved. A frame is due on
        // every display refresh of a run; frames shown during it (until 1.5 refreshes after)
        // count.
        let mut runs: Vec<(f64, f64, usize, usize)> = Vec::new(); // (start, end, segment, refreshes)
        let mut cur: Option<(f64, f64, usize, usize)> = None;
        let seg_of = |t: f64| -> usize {
            self.segments
                .iter()
                .position(|s| t >= s.from && t < s.to)
                .unwrap_or(self.segments.len().saturating_sub(1))
        };
        for r in &self.refreshes {
            if r.moving {
                let s = seg_of(r.t);
                match &mut cur {
                    Some(c) if c.2 == s => {
                        c.1 = r.now;
                        c.3 += 1;
                    }
                    _ => {
                        if let Some(c) = cur.take() {
                            runs.push(c);
                        }
                        cur = Some((r.now, r.now, s, 1));
                    }
                }
            } else if let Some(c) = cur.take() {
                runs.push(c);
            }
        }
        if let Some(c) = cur.take() {
            runs.push(c);
        }
        // Shown times: presentation times if the platform reported them, else commits.
        let shown: Vec<f64> = if self.shown.is_empty() {
            self.refreshes.iter().filter(|r| r.committed).map(|r| r.now).collect()
        } else {
            let mut s = self.shown.clone();
            s.sort_by(f64::total_cmp);
            s
        };
        let nseg = self.segments.len().max(1);
        let mut due = vec![0usize; nseg];
        let mut presented = vec![0usize; nseg];
        let mut intervals: Vec<Vec<f64>> = vec![Vec::new(); nseg];
        for &(a, b, s, n) in &runs {
            // A one-refresh run is a jump: its new image is measured as time to first image.
            if n < 2 {
                continue;
            }
            // Due: every display refresh of the run, whether `frame` was called or not (a
            // blocked main thread skips display-link callbacks).
            let d = ((b - a) * self.hz).round() as usize + 1;
            due[s] += d;
            let inside: Vec<f64> = shown
                .iter()
                .copied()
                .filter(|&x| x >= a && x <= b + 1.5 * period)
                .collect();
            presented[s] += inside.len().min(d);
            intervals[s].extend(inside.windows(2).map(|w| w[1] - w[0]));
        }
        let all_iv: Vec<f64> = intervals.iter().flatten().copied().collect();
        let late = |iv: &[f64]| iv.iter().filter(|&&x| x > 1.5 * period).count();
        let missed = |iv: &[f64]| -> usize {
            iv.iter()
                .map(|&x| ((x / period).round() as usize).saturating_sub(1))
                .sum()
        };
        let landing_json = |l: &Landing| {
            let at = l.t;
            let window = self.segments[l.segment].window_end(at) - at;
            json!({
                "at_s": at,
                "segment": self.segments[l.segment].name,
                "first_image_ms": l.first_image.map(|x| x * 1000.0),
                "sharp_ms": l.sharp.map(|x| x * 1000.0),
                "censored": l.sharp.is_none(),
                "window_ms": window * 1000.0,
            })
        };
        // Censored landings count with the time they were given (a lower bound).
        let sharp_total: f64 = self
            .landings
            .iter()
            .map(|l| l.sharp.unwrap_or(self.segments[l.segment].window_end(l.t) - l.t))
            .sum();
        let segments: Vec<Value> = self
            .segments
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "name": s.name,
                    "due": due[i],
                    "presented": presented[i],
                    "presented_frac": if due[i] > 0 { presented[i] as f64 / due[i] as f64 } else { 1.0 },
                    "p50_ms": finite(pct(&intervals[i], 0.5) * 1000.0),
                    "p99_ms": finite(pct(&intervals[i], 0.99) * 1000.0),
                    "late_frames": late(&intervals[i]),
                    "missed_refreshes": missed(&intervals[i]),
                    "landings": self.landings.iter().filter(|l| l.segment == i).map(landing_json).collect::<Vec<_>>(),
                })
            })
            .collect();
        let (td, tp): (usize, usize) = (due.iter().sum(), presented.iter().sum());
        json!({
            "hz": self.hz,
            "duration_s": self.duration,
            "refreshes": self.refreshes.len(),
            "due": td,
            "presented": tp,
            "presented_frac": if td > 0 { tp as f64 / td as f64 } else { 1.0 },
            "p50_ms": finite(pct(&all_iv, 0.5) * 1000.0),
            "p99_ms": finite(pct(&all_iv, 0.99) * 1000.0),
            "max_ms": finite(pct(&all_iv, 1.0) * 1000.0),
            "late_frames": late(&all_iv),
            "missed_refreshes": missed(&all_iv),
            "late_refreshes": self.late_refreshes,
            "sharp_total_s": sharp_total,
            "presentation_times": !self.shown.is_empty(),
            "segments": segments,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = r#"{
        "size": "320x200", "duration": 2,
        "start": {"re": "-0.75", "im": "0", "zoom": 1},
        "segments": [{"name": "zoom", "from": 0, "to": 1, "landings": [0.5]},
                     {"name": "jump", "from": 1, "to": 2, "landings": 1}],
        "events": [{"from": 0.0, "to": 0.5, "magnify": 2.0, "point": [0, 0]},
                   {"at": 1.0, "jump": {"re": "0", "im": "1", "scale": "1e-300"}}]
    }"#;

    #[test]
    fn camera_path_is_deterministic_and_follows_events() {
        let s = FlightScript::parse(SCRIPT).unwrap();
        let base = RenderSettings::default();
        let a = camera_path(&s, &base, s.size, 120.0, 10.0).unwrap();
        let b = camera_path(&s, &base, s.size, 120.0, 10.0).unwrap();
        assert_eq!(a.len(), 20);
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.viewport, y.viewport);
        }
        // Zoomed in by about e^(2 × 0.5) during the pinch.
        let z = a[6].viewport.zoom_log10() - a[0].viewport.zoom_log10();
        assert!((z - 1.0 / std::f64::consts::LN_10).abs() < 0.05, "zoomed {z}");
        // The jump replaced the view.
        assert!(a[15].viewport.zoom_log10() > 290.0);
        assert_eq!(s.segment_at(1.5), Some(1));
    }

    #[test]
    fn recorder_counts_due_shown_and_landings() {
        let s = FlightScript::parse(SCRIPT).unwrap();
        let hz = 100.0;
        let base = 1234.567;
        let mut rec = LiveRecorder::new(&s, hz, base);
        for k in 0..200 {
            let t = k as f64 / hz;
            // Moving for the first 50 refreshes, every second frame shown; settled 0.2 s
            // after each landing.
            let moving = k < 50;
            let committed = moving && k % 2 == 0;
            let settled = (70..100).contains(&k) || k >= 120;
            rec.refresh(t, base + t, moving, committed, settled, None);
        }
        let r = rec.report();
        assert_eq!(r["due"], 50);
        assert_eq!(r["presented"], 25);
        assert!((r["p99_ms"].as_f64().unwrap() - 20.0).abs() < 1e-3);
        let l0 = &r["segments"][0]["landings"][0];
        assert!((l0["sharp_ms"].as_f64().unwrap() - 200.0).abs() < 1e-3);
        assert!((l0["window_ms"].as_f64().unwrap() - 500.0).abs() < 1e-6);
        let l1 = &r["segments"][1]["landings"][0];
        assert!((l1["sharp_ms"].as_f64().unwrap() - 200.0).abs() < 1e-3);
        assert!((r["sharp_total_s"].as_f64().unwrap() - 0.4).abs() < 1e-6);
    }

    /// The benchmark flight reaches every regime: the dive ends near 1e-300 per pixel, the
    /// heavy spot and the 1e-1000 jump land where the locations are, the rotation turns.
    #[test]
    fn benchmark_flight_visits_every_regime() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/flights/benchmark-v1.json"
        ))
        .unwrap();
        let s = FlightScript::parse(&text).unwrap();
        let p = camera_path(&s, &RenderSettings::default(), s.size, 120.0, 5.0).unwrap();
        let at = |t: f64| &p.iter().find(|c| c.t >= t - 1e-9).unwrap().viewport;
        for c in p.iter().step_by(5) {
            eprintln!(
                "{:5.1} s: 1e{:.1} per px, angle {:.2}",
                c.t,
                c.viewport.scale.log10(),
                c.viewport.angle
            );
        }
        assert!(
            (at(39.0).scale.log10() + 300.0).abs() < 1.0,
            "dive {}",
            at(39.0).scale.log10()
        );
        assert!(
            (at(41.0).scale.log10() - 4.19804e-19f64.log10()).abs() < 0.01,
            "heavy {}",
            at(41.0).scale.log10()
        );
        assert!(at(41.0).center.re() + 0.13415 < 1e-5);
        assert!(at(53.0).angle.abs() > 2.0, "rotation {}", at(53.0).angle);
        assert!(at(59.0).scale.log10() < -1003.0, "jump {}", at(59.0).scale.log10());
    }
}
