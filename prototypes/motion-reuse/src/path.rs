//! The camera path of a flight script: the same `Navigator` the app and `almondbread fly`
//! drive, stepped at virtual refresh times (as `fly --converged 1` does), so the path is
//! deterministic. The event feeding is a copy of `crates/almondbread-cli/src/fly.rs` `feed`
//! (settings events ignored).

use almondbread_core::input::{InputEvent, NavKey};
use almondbread_core::viewport::{Center, Scale, Viewport};
use almondbread_render::{GpuBackend, Renderer};
use serde_json::Value;

pub struct Script {
    pub size: (usize, usize),
    pub fps: f64,
    pub duration: f64,
    pub start: Option<Value>,
    pub events: Vec<Value>,
}

/// One refresh of the flight.
#[derive(Clone)]
pub struct PathFrame {
    pub k: usize,
    pub t: f64,
    pub viewport: Viewport,
    pub interacting: bool,
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn pair(v: Option<&Value>) -> Option<(f64, f64)> {
    let a = v?.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

pub fn parse_size(s: &str) -> Option<(usize, usize)> {
    let (a, b) = s.split_once('x')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

impl Script {
    pub fn load(path: &str) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let v: Value = serde_json::from_str(&text).expect("script json");
        Self {
            size: v
                .get("size")
                .and_then(Value::as_str)
                .and_then(parse_size)
                .unwrap_or((1920, 1080)),
            fps: num(&v, "fps").unwrap_or(60.0),
            duration: num(&v, "duration").unwrap_or(10.0),
            start: v.get("start").cloned(),
            events: v.get("events").and_then(Value::as_array).cloned().unwrap_or_default(),
        }
    }
}

fn nav_key(name: &str) -> Option<NavKey> {
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
        _ => return None,
    })
}

fn start_viewport(start: Option<&Value>, w: f64, h: f64) -> Viewport {
    let fitted = Viewport::fitting(w, h);
    let Some(s) = start else { return fitted };
    let text = |k: &str| {
        s.get(k)
            .map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))
    };
    let center = match (text("re"), text("im")) {
        (Some(re), Some(im)) => Center::parse(&re, &im).expect("start centre"),
        _ => fitted.center.clone(),
    };
    let scale = match (
        text("scale").and_then(|t| Scale::parse(t.trim_matches('"'))),
        num(s, "zoom"),
    ) {
        (Some(sc), _) => sc,
        (None, Some(z)) => fitted.scale / z,
        _ => fitted.scale,
    };
    Viewport::with_center(center, scale)
}

fn feed<B: GpuBackend>(r: &mut Renderer<B>, events: &[Value], t0: f64, t1: f64, now: f64, pointer: &mut (f64, f64)) {
    for e in events {
        let at = num(e, "at");
        let due = at.is_some_and(|a| a > t0 && a <= t1) || (t0 < 0.0 && at == Some(0.0));
        let (from, to) = (num(e, "from"), num(e, "to"));
        let during = matches!((from, to), (Some(a), Some(b)) if t1 > a && t0 < b);
        let ends = matches!(to, Some(b) if b > t0 && b <= t1);
        if due {
            if let Some(k) = e.get("key").and_then(Value::as_str).and_then(nav_key) {
                match e.get("down").and_then(Value::as_bool) {
                    Some(true) => {
                        r.handle_input(InputEvent::KeyDown(k), now);
                    }
                    Some(false) => {
                        r.handle_input(InputEvent::KeyUp(k), now);
                    }
                    None => {
                        r.handle_input(InputEvent::KeyDown(k), now);
                        r.handle_input(InputEvent::KeyUp(k), now);
                    }
                }
            }
            if let Some(s) = e.get("shift").and_then(Value::as_bool) {
                r.handle_input(InputEvent::Shift(s), now);
            }
            if let Some(p) = e.get("pointer") {
                let p = pair(Some(p));
                if let Some(p) = p {
                    *pointer = p;
                }
                r.handle_input(InputEvent::Pointer(p), now);
            }
            if let Some(n) = num(e, "wheel") {
                let (x, y) = pair(e.get("point")).unwrap_or(*pointer);
                r.handle_input(InputEvent::WheelNotch { delta: n, x, y }, now);
            }
            if let Some((x, y)) = pair(e.get("double_click")) {
                let out = e.get("out").and_then(Value::as_bool).unwrap_or(false);
                r.handle_input(InputEvent::DoubleTap { x, y, out }, now);
            }
        }
        if during {
            let (a, b) = (from.unwrap_or(0.0), to.unwrap_or(0.0));
            let span = t1.min(b) - t0.max(a);
            if let Some((vx, vy)) = pair(e.get("pan")) {
                r.handle_input(
                    InputEvent::Pan {
                        dx: vx * span,
                        dy: vy * span,
                    },
                    now,
                );
            }
            if let Some(rate) = num(e, "magnify") {
                let (x, y) = pair(e.get("point")).unwrap_or(*pointer);
                r.handle_input(
                    InputEvent::Magnify {
                        magnification: (rate * span).exp() - 1.0,
                        x,
                        y,
                    },
                    now,
                );
            }
            if let Some(rate) = num(e, "pinch") {
                let (x, y) = pair(e.get("point")).unwrap_or(*pointer);
                let (vx, vy) = pair(e.get("pan")).unwrap_or((0.0, 0.0));
                r.handle_input(
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
            if let Some((x1, y1)) = pair(e.get("pointer_to")) {
                let left = (b - t0.max(a)).max(1e-9);
                let f = (span / left).clamp(0.0, 1.0);
                *pointer = (pointer.0 + (x1 - pointer.0) * f, pointer.1 + (y1 - pointer.1) * f);
                r.handle_input(InputEvent::Pointer(Some(*pointer)), now);
            }
            if let Some(g) = e.get("gamepad")
                && t0 <= a
            {
                let gf = |k: &str| g.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                r.handle_input(
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
        if let (Some(b), Some((vx, vy))) = (to, pair(e.get("pan")))
            && e.get("momentum").and_then(Value::as_bool) == Some(true)
            && t1 > b
        {
            let (s0, s1) = ((t0 - b).max(0.0), t1 - b);
            let tau = 0.35;
            let k = tau * ((-s0 / tau).exp() - (-s1 / tau).exp());
            if vx.hypot(vy) * (-s1 / tau).exp() > 5.0 {
                r.handle_input(InputEvent::Pan { dx: vx * k, dy: vy * k }, now);
            }
        }
        if ends {
            if e.get("release").and_then(Value::as_bool) == Some(true) {
                r.handle_input(InputEvent::GestureEnd, now);
            }
            if e.get("gamepad").is_some() {
                r.handle_input(
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

/// The flight's viewports per refresh at `hz`, for a view of `size` pixels at one point per
/// pixel (script speeds are in points; relative speeds do not depend on the size).
pub fn camera_path<B: GpuBackend>(
    r: &mut Renderer<B>,
    script: &Script,
    size: (usize, usize),
    hz: f64,
) -> Vec<PathFrame> {
    let (w, h) = (size.0 as f64, size.1 as f64);
    r.handle_input(InputEvent::Resize { width: w, height: h }, 0.0);
    r.viewport = start_viewport(script.start.as_ref(), w, h);
    let interval = 1.0 / hz;
    let refreshes = (script.duration * hz).round() as usize;
    let base = 1000.0;
    let mut pointer = (0.0, 0.0);
    let mut out = Vec::with_capacity(refreshes);
    for k in 0..refreshes {
        let t = k as f64 * interval;
        let now = base + t;
        feed(r, &script.events, t - interval, t, now, &mut pointer);
        let mut vp = r.viewport.clone();
        r.navigator.advance(&mut vp, now);
        r.viewport = vp.clone();
        out.push(PathFrame {
            k,
            t,
            viewport: vp,
            interacting: r.navigator.is_interacting(now),
        });
    }
    out
}
