//! `fly`: the headless flight recorder (kb: synthesis/benchmarks FL, techniques/frame-reuse-and-progressive).
//!
//! Drives the interactive pipeline the app runs per display refresh: the same `Navigator`
//! (scripted input events at their times), the same `Renderer::render_frame` with its
//! real-time budgets, paced in real time at the refresh rate. Every refresh shows what the
//! window would show: gesture previews, reprojection, refinement, progressive AA, and the
//! previous image on refreshes that present nothing. Not converged frames: what a user sees.
//! `--converged 1` renders the same camera path (the navigator is stepped with the same
//! virtual timestamps) fully refined and anti-aliased per frame instead, for comparison.
//!
//! ```text
//! almondbread fly --script s.json [--out review/x.mp4 | --out dir/] [--fps 60] [--hz 60]
//!                 [--size 1920x1080] [--converged 1] [--stills 0,60,120] [--codec hevc|h264]
//!                 [--bitrate 40M] [--log x.csv]
//! ```
//!
//! Script (JSON): `size`, `fps`, `duration` (s), `start` (`re`, `im` decimal strings and
//! `zoom` relative to the fitted home view, or `scale` in complex units per point),
//! `settings` (as in [`apply_json_settings`]) and `events`, each either at a time (`at`) or
//! over an interval (`from`, `to`, applied every refresh):
//!
//! | event | meaning |
//! | --- | --- |
//! | `{"at": t, "key": "zoom", "down": true}` | key down / up (`down` omitted: a tap). Keys: up down left right w a s d zoom space slower q faster e in out reset |
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

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use almondbread_core::image::write_png_bgra;
use almondbread_core::input::{InputEvent, NavKey};
use almondbread_core::settings::{RenderSettings, Supersampling};
use almondbread_core::viewport::{Center, Scale, Viewport};
use almondbread_render::{FrameStats, GpuBackend, Renderer, Storage, TextureFormat};
use serde_json::Value;

use crate::options::{Options, Result, err, parse_size};

/// Frames handed to the encoder.
enum Sink {
    Ffmpeg(std::process::Child),
    Pngs(String),
}

/// One flight: scripted events plus where it starts.
struct Script {
    size: (usize, usize),
    fps: f64,
    duration: f64,
    start: Option<Value>,
    settings: Option<Value>,
    events: Vec<Value>,
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn pair(v: Option<&Value>) -> Option<(f64, f64)> {
    let a = v?.as_array()?;
    Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?))
}

impl Script {
    fn load(path: &str) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|e| crate::options::CommandError(format!("{path}: {e}")))?;
        let v: Value = serde_json::from_str(&text)?;
        let size = match v.get("size").and_then(Value::as_str) {
            Some(s) => parse_size(s)?,
            None => (1920, 1080),
        };
        Ok(Self {
            size,
            fps: num(&v, "fps").unwrap_or(60.0),
            duration: num(&v, "duration").unwrap_or(10.0),
            start: v.get("start").cloned(),
            settings: v.get("settings").cloned(),
            events: v.get("events").and_then(Value::as_array).cloned().unwrap_or_default(),
        })
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

/// Settings from a JSON object: the CLI option names (`palette`, `shading`, `outline`,
/// `density`, `offset`, `log`, `ss`, `iterations`, `interior`) and the colouring fields.
pub fn apply_json_settings(v: &Value, s: &mut RenderSettings) {
    if let Some(l) = v
        .get("look")
        .and_then(Value::as_str)
        .and_then(almondbread_core::palette::Look::named)
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
    crate::options::apply_look_json(v, s);
}

fn start_viewport(start: Option<&Value>, w_pts: f64, h_pts: f64) -> Result<Viewport> {
    let fitted = Viewport::fitting(w_pts, h_pts);
    let Some(s) = start else { return Ok(fitted) };
    let text = |k: &str| {
        s.get(k)
            .map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))
    };
    let center = match (text("re"), text("im")) {
        (Some(re), Some(im)) => {
            Center::parse(&re, &im).ok_or_else(|| crate::options::CommandError("bad start".into()))?
        }
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
    Ok(Viewport::with_center(center, scale))
}

/// Feeds the events due in (t0, t1] (virtual seconds since the start) at time `now`.
fn feed<B: GpuBackend>(r: &mut Renderer<B>, events: &[Value], t0: f64, t1: f64, now: f64, pointer: &mut (f64, f64)) {
    let dt = t1 - t0;
    for e in events {
        let at = num(e, "at");
        let due = at.is_some_and(|a| a > t0 && a <= t1) || (t0 < 0.0 && at == Some(0.0));
        let (from, to) = (num(e, "from"), num(e, "to"));
        let during = matches!((from, to), (Some(a), Some(b)) if t1 > a && t0 < b);
        let ends = matches!(to, Some(b) if b > t0 && b <= t1);
        if due {
            if let Some(k) = e.get("key").and_then(Value::as_str).and_then(nav_key) {
                match e.get("down").and_then(Value::as_bool) {
                    Some(d) => {
                        let ev = if d {
                            InputEvent::KeyDown(k)
                        } else {
                            InputEvent::KeyUp(k)
                        };
                        r.handle_input(ev, now);
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
            if let Some(s) = e.get("settings") {
                apply_json_settings(s, &mut r.settings);
            }
        }
        if during {
            let (a, b) = (from.unwrap_or(0.0), to.unwrap_or(0.0));
            // The part of this refresh inside the interval.
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
                // Linear from where the pointer is to the end point by `b`.
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
        // Trackpad-like momentum after a pan: the OS keeps sending decaying scrolls.
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
    let _ = dt;
}

fn open_sink(out: &str, size: (usize, usize), fps: f64, o: &Options) -> Result<Sink> {
    if out.ends_with(".mp4") || out.ends_with(".mov") {
        if let Some(dir) = std::path::Path::new(out).parent() {
            fs::create_dir_all(dir)?;
        }
        let codec = match o.get("codec").unwrap_or("hevc") {
            "h264" => "h264_videotoolbox",
            _ => "hevc_videotoolbox",
        };
        let bitrate = o.get("bitrate").unwrap_or("40M");
        let ffmpeg = o.get("ffmpeg").unwrap_or("/opt/homebrew/bin/ffmpeg");
        let mut cmd = Command::new(ffmpeg);
        cmd.args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "bgra", "-s"])
            .arg(format!("{}x{}", size.0, size.1))
            .args([
                "-framerate",
                &format!("{fps}"),
                "-i",
                "-",
                "-c:v",
                codec,
                "-b:v",
                bitrate,
            ])
            .args(["-vf", "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p"])
            .args([
                "-colorspace",
                "bt709",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "iec61966-2-1",
            ]);
        if codec.starts_with("hevc") {
            cmd.args(["-tag:v", "hvc1"]);
        }
        cmd.args(["-movflags", "+faststart", out]);
        let child = cmd.stdin(Stdio::piped()).spawn()?;
        Ok(Sink::Ffmpeg(child))
    } else {
        fs::create_dir_all(out)?;
        Ok(Sink::Pngs(out.to_string()))
    }
}

/// A video frame and, for a still, where to save it as PNG.
type Frame = (Arc<Vec<u8>>, Option<String>);

/// The encoder thread: frames in order, written to ffmpeg's stdin or as PNGs.
type WriterResult = std::result::Result<usize, String>;

fn spawn_writer(mut sink: Sink, size: (usize, usize)) -> (SyncSender<Frame>, JoinHandle<WriterResult>) {
    let (tx, rx) = sync_channel::<Frame>(24);
    let handle = std::thread::spawn(move || -> WriterResult {
        let mut n = 0;
        for (frame, still) in rx {
            // Stills are encoded here, off the render loop (PNG encoding took ~60 ms).
            if let Some(path) = still {
                write_png_bgra(&path, &frame, size.0, size.1).map_err(|e| format!("{path}: {e}"))?;
            }
            match &mut sink {
                Sink::Ffmpeg(child) => {
                    let stdin = child.stdin.as_mut().ok_or("ffmpeg stdin")?;
                    stdin.write_all(&frame).map_err(|e| format!("ffmpeg: {e}"))?;
                }
                Sink::Pngs(dir) => write_png_bgra(format!("{dir}/frame_{n:05}.png"), &frame, size.0, size.1)
                    .map_err(|e| format!("{dir}: {e}"))?,
            }
            n += 1;
        }
        if let Sink::Ffmpeg(mut child) = sink {
            drop(child.stdin.take());
            let status = child.wait().map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("ffmpeg failed: {status}"));
            }
        }
        Ok(n)
    });
    (tx, handle)
}

pub fn fly_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    let Some(path) = o.get("script") else {
        return err("fly needs --script path.json");
    };
    let script = Script::load(path)?;
    let size = o.size("size", script.size)?;
    let fps = o.f64("fps").unwrap_or(script.fps);
    let hz = o.f64("hz").unwrap_or(fps);
    let per_video = (hz / fps).round().max(1.0) as usize;
    let duration = o.f64("duration").unwrap_or(script.duration);
    let converged = o.get("converged") == Some("1");
    let out = o
        .get("out")
        .map(String::from)
        .unwrap_or_else(|| "review/flight.mp4".into());
    let stills: Vec<usize> = o
        .get("stills")
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_default();
    let stills_dir = o
        .get("stills-dir")
        .map(String::from)
        .unwrap_or_else(|| format!("{}_stills", out.trim_end_matches(".mp4")));
    if !stills.is_empty() {
        fs::create_dir_all(&stills_dir)?;
    }

    let mut r = Renderer::new(backend.clone())?;
    // The app's defaults (the default look, progressive AA), then CLI flags, then the script.
    if o.has("look") || o.has("ss") || o.has("palette") || o.has("iterations") {
        let app_ss = r.settings.supersampling;
        o.apply_settings(&mut r.settings);
        if !o.has("ss") {
            r.settings.supersampling = app_ss;
        }
    }
    if let Some(s) = &script.settings {
        apply_json_settings(s, &mut r.settings);
    }
    o.apply_renderer(&mut r);
    // One view point per pixel: script positions and speeds are in video pixels.
    let (w_pts, h_pts) = (size.0 as f64, size.1 as f64);
    r.handle_input(
        InputEvent::Resize {
            width: w_pts,
            height: h_pts,
        },
        0.0,
    );
    r.viewport = start_viewport(script.start.as_ref(), w_pts, h_pts)?;
    let refreshes = (duration * hz).round() as usize;
    let interval = 1.0 / hz;
    let (tx, writer) = spawn_writer(open_sink(&out, size, fps, o)?, size);
    let mut log = String::from("refresh,t,presented,preview,render_w,render_h,zoom,late_ms,frame_cpu_ms\n");
    let mut pointer = (0.0, 0.0);
    let bytes = size.0 * size.1 * 4;
    let still = |k: usize| stills.contains(&k).then(|| format!("{stills_dir}/frame_{k:05}.png"));

    if converged {
        // Same camera path (the navigator stepped at the same virtual refresh times), each
        // video frame rendered to completion with the extension passes.
        r.config.offscreen_extension = r.config.extension_factor;
        let base = 1000.0;
        let mut video = 0;
        for k in 0..refreshes {
            let t = k as f64 * interval;
            let now = base + t;
            feed(&mut r, &script.events, t - interval, t, now, &mut pointer);
            r.navigator.advance(&mut r.viewport, now);
            if k % per_video == 0 {
                let res = r.render_offscreen(size.0, size.1, false)?;
                tx.send((Arc::new(res.pixels), still(video)))?;
                video += 1;
                if video % 60 == 0 {
                    eprintln!("fly: converged frame {video}");
                }
            }
        }
        drop(tx);
        let n = writer
            .join()
            .map_err(|_| "writer panicked")?
            .map_err(crate::options::CommandError)?;
        println!(
            "fly (converged): {n} frames {}x{} at {fps} fps -> {out}",
            size.0, size.1
        );
        return Ok(0);
    }

    // Real time: a ring of shared targets, read back once their frame finished on the GPU.
    const RING: usize = 8;
    let ring: Vec<B::Texture> = (0..RING)
        .map(|_| backend.create_texture(size.0, size.1, TextureFormat::Bgra8Unorm, Storage::Shared))
        .collect::<std::result::Result<_, _>>()?;
    let mut slot_serial = [0u64; RING];
    let mut next_slot = 0;
    // Video frames waiting for their image: (serial shown, slot).
    let mut queue: std::collections::VecDeque<(usize, u64, usize)> = Default::default();
    let mut last_image: (u64, Arc<Vec<u8>>) = (0, Arc::new(vec![0u8; bytes]));
    let mut shown: (u64, usize) = (0, 0);
    let mut video = 0usize;
    let mut late = 0usize;
    let mut worst_late = 0.0f64;
    r.frame_stats = Some(FrameStats::default());
    let base = r.clock() + 0.05;
    let mut last_input_t = 0.0f64;
    let mut final_after: Option<f64> = None;
    let mut presented_frames = 0usize;

    let flush = |r: &Renderer<B>,
                 queue: &mut std::collections::VecDeque<(usize, u64, usize)>,
                 last_image: &mut (u64, Arc<Vec<u8>>),
                 force: bool|
     -> Result<()> {
        while let Some(&(k, serial, slot)) = queue.front() {
            if serial > 0 && serial != last_image.0 {
                if r.frames_completed() < serial {
                    if !force {
                        break;
                    }
                    r.wait_frame(serial);
                }
                let mut px = vec![0u8; bytes];
                backend.read_shared_texture(&ring[slot], size.0, size.1, TextureFormat::Bgra8Unorm, &mut px);
                *last_image = (serial, Arc::new(px));
            }
            tx.send((Arc::clone(&last_image.1), still(k)))?;
            queue.pop_front();
        }
        Ok(())
    };

    for k in 0..refreshes {
        let t = k as f64 * interval;
        let now = base + t;
        // Pace: never before the refresh's time; count refreshes we reach late.
        let wait = now - r.clock();
        if wait > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(wait));
        } else if -wait > interval * 0.5 {
            late += 1;
            worst_late = worst_late.max(-wait);
        }
        let before = r.navigator.last_interaction;
        feed(&mut r, &script.events, t - interval, t, now, &mut pointer);
        // The slot this refresh may draw into must not hold an image still waiting.
        if queue
            .iter()
            .any(|q| q.2 == next_slot && q.1 == slot_serial[next_slot] && q.1 != last_image.0)
        {
            flush(&r, &mut queue, &mut last_image, true)?;
        }
        let target = ring[next_slot].clone();
        let cpu0 = r.clock();
        let outcome = r.render_frame(now, size.0, size.1, 1.0, &mut |_| Some((target.clone(), None)));
        let frame_cpu = r.clock() - cpu0;
        if r.navigator.last_interaction != before || r.navigator.is_moving() {
            last_input_t = t;
            final_after = None;
        }
        if outcome.presented {
            presented_frames += 1;
            slot_serial[next_slot] = r.frames_committed();
            shown = (r.frames_committed(), next_slot);
            next_slot = (next_slot + 1) % RING;
            if final_after.is_none() && r.is_settled() {
                final_after = Some(t - last_input_t);
            }
        }
        if k % per_video == 0 {
            queue.push_back((video, shown.0, shown.1));
            video += 1;
        }
        let st = &r.stats;
        log.push_str(&format!(
            "{k},{t:.4},{},{},{},{},{:.6e},{:.2},{:.3}\n",
            u8::from(outcome.presented),
            u8::from(st.is_preview),
            st.render_size.0,
            st.render_size.1,
            r.viewport.zoom(),
            (-wait).max(0.0) * 1000.0,
            frame_cpu * 1000.0
        ));
        flush(&r, &mut queue, &mut last_image, false)?;
    }
    flush(&r, &mut queue, &mut last_image, true)?;
    drop(tx);
    let n = writer
        .join()
        .map_err(|_| "writer panicked")?
        .map_err(crate::options::CommandError)?;
    let fs = r.frame_stats.take().unwrap_or_default();
    if let Some(p) = o.get("log") {
        fs::write(p, &log)?;
    } else if out.ends_with(".mp4") {
        fs::write(out.replace(".mp4", ".csv"), &log)?;
    }
    println!(
        "fly: {n} video frames {}x{} at {fps} fps ({hz} Hz refresh) -> {out}\n  presented {presented_frames} of {refreshes} refreshes ({:.1} fps), preview frames {}, skipped {} (in flight {}), late refreshes {late} (worst {:.1} ms), iterate GPU {:.2} ms/frame, colorize {:.2} ms/frame, final view settled {} after the last input",
        size.0,
        size.1,
        presented_frames as f64 / duration,
        fs.preview_frames,
        fs.skipped,
        fs.skipped_in_flight,
        worst_late * 1000.0,
        fs.chunk_gpu * 1000.0 / fs.frames.max(1) as f64,
        fs.colorize_gpu * 1000.0 / fs.frames.max(1) as f64,
        final_after.map_or("not".into(), |s| format!("{:.0} ms", s * 1000.0)),
    );
    Ok(0)
}
