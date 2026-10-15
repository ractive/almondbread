//! `benchmark`: the flight benchmark with a score (kb: synthesis/flight-benchmark). One fixed,
//! versioned flight (`scripts/flights/benchmark-v1.json`) through every regime once, in two
//! modes:
//!
//! - **A, throughput**: the flight's camera path sampled at `step_hz` (fixed timestep), each
//!   sample rendered through the real per-refresh scheduler to the settled full-quality frame
//!   (refined, extended, anti-aliased) before the next, uncapped by any display. Spot checks
//!   diff the shown frame against a fresh render (reuse must not fake a frame). Reports wall,
//!   CPU and GPU seconds in all and per segment.
//! - **B, real time**: the flight played live at the display rate. Headless here (`approx`,
//!   paced at `hz` through `Renderer::render_frame`); in the app's window with `--live-app`
//!   (the app runs `--flight-bench=`, see `Sources/AlmondbreadKit/FlightBench.swift`) or
//!   `--live file.json`. Reports frames due while the camera moves, frames shown, p99 frame
//!   time, late frames, time to first image and to sharp after each landing.
//!
//! Scores: each mode relative to the reference file (the M1 Pro reference machine = 1000),
//! combined by geometric mean. Warm-up laps first, then `--repeat` laps: medians and spread.
//! A busy machine (other processes' CPU, GPU utilisation) marks the result noisy. JSON goes to
//! `build/benchmark/`; `--compare a.json b.json` prints deltas per segment.
//!
//! ```text
//! almondbread benchmark [--flight f.json] [--modes a,approx] [--repeat 3] [--warmup 1]
//!                       [--out build/benchmark] [--reference r.json] [--check-every 100]
//!                       [--write-reference r.json [--final 1]] [--stills dir]
//! almondbread benchmark --live-app .build/release/AlmondbreadMac [--repeat 3] [--warmup 1]
//! (any mode: --sa 0|1|auto, --bla 0|1|auto for A/B runs; the JSON records them)
//! almondbread benchmark --live run.json
//! almondbread benchmark --compare a.json b.json
//! almondbread benchmark --noise 1
//! ```

use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use almondbread_core::flight::{
    FlightPlayer, FlightScript, LiveRecorder, apply_json_settings, camera_path, start_viewport,
};
use almondbread_core::input::InputEvent;
use almondbread_render::{FrameStats, GpuBackend, Renderer, Storage, TextureFormat};
use serde_json::{Value, json};

use crate::options::{Options, Result, err};

const DEFAULT_FLIGHT: &str = "scripts/flights/benchmark-v1.json";
/// A shown frame differing from a fresh render in more than this % of pixels (class) fails.
const MAX_MISMATCH: f64 = 0.05;
/// A settled frame whose 16x16-pixel blocks differ from a from-scratch render of the view by
/// more than 32 levels of mean luma in more than this % of blocks fails (anti-aliasing and
/// float noise average out in a block; a wrong iteration limit or stale image does not).
const MAX_JUMP_BLOCKS: f64 = 2.0;

/// % of 16x16 blocks whose mean luma differs by more than 32 of 255 (BGRA8).
fn block_difference(a: &[u8], b: &[u8], w: usize, h: usize) -> f64 {
    const B: usize = 16;
    let luma = |p: &[u8], x: usize, y: usize| {
        let i = (y * w + x) * 4;
        0.0722 * p[i] as f64 + 0.7152 * p[i + 1] as f64 + 0.2126 * p[i + 2] as f64
    };
    let (mut n, mut differ) = (0usize, 0usize);
    for by in 0..h / B {
        for bx in 0..w / B {
            let (mut sa, mut sb) = (0.0, 0.0);
            for y in by * B..(by + 1) * B {
                for x in bx * B..(bx + 1) * B {
                    sa += luma(a, x, y);
                    sb += luma(b, x, y);
                }
            }
            n += 1;
            if ((sa - sb) / (B * B) as f64).abs() > 32.0 {
                differ += 1;
            }
        }
    }
    100.0 * differ as f64 / n.max(1) as f64
}

// MARK: Machine state

/// CPU seconds (user + system) of this process so far.
fn cpu_time() -> f64 {
    // SAFETY: getrusage writes into the zeroed struct we pass; no other preconditions.
    let mut u: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut u) } != 0 {
        return f64::NAN;
    }
    let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 * 1e-6;
    tv(u.ru_utime) + tv(u.ru_stime)
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// CPU % of all other processes (100 = one core), from `ps`, excluding `exclude` pids.
fn others_cpu(exclude: &[u32]) -> Option<f64> {
    let text = run("ps", &["-A", "-o", "pid=,%cpu="])?;
    Some(
        text.lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                let pid: u32 = it.next()?.parse().ok()?;
                let cpu: f64 = it.next()?.parse().ok()?;
                (!exclude.contains(&pid)).then_some(cpu)
            })
            .sum(),
    )
}

/// The GPU's "Device Utilization %" (IOAccelerator performance statistics), if readable.
fn gpu_utilization() -> Option<f64> {
    let text = run("ioreg", &["-r", "-d", "1", "-c", "IOAccelerator"])?;
    text.split("\"Device Utilization %\"=")
        .skip(1)
        .filter_map(|s| {
            s.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<f64>()
                .ok()
        })
        .reduce(f64::max)
}

fn load_average() -> Option<f64> {
    let text = run("sysctl", &["-n", "vm.loadavg"])?;
    text.trim_matches(|c| c == '{' || c == '}' || c == ' ')
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// A snapshot of how busy the machine is while this process idles.
fn noise_probe() -> Value {
    let me = std::process::id();
    json!({
        "others_cpu_pct": others_cpu(&[me]),
        "gpu_utilization_pct": gpu_utilization(),
        "load_1m": load_average(),
    })
}

/// Busy if other processes use more than a core, or the GPU is more than 10 % busy while we idle.
const NOISY_CPU: f64 = 100.0;
const NOISY_GPU: f64 = 10.0;

fn probe_is_noisy(p: &Value) -> bool {
    p["others_cpu_pct"].as_f64().is_some_and(|c| c > NOISY_CPU)
        || p["gpu_utilization_pct"].as_f64().is_some_and(|g| g > NOISY_GPU)
}

/// Samples other processes' CPU every 2 s while a lap runs.
struct Sampler {
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<f64>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Sampler {
    fn start(exclude: Vec<u32>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let (s, v) = (stop.clone(), samples.clone());
        let handle = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                if let Some(c) = others_cpu(&exclude)
                    && let Ok(mut v) = v.lock()
                {
                    v.push(c);
                }
                for _ in 0..20 {
                    if s.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Self {
            stop,
            samples,
            handle: Some(handle),
        }
    }

    fn finish(mut self) -> Vec<f64> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.samples.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

fn device() -> Value {
    let sys = |k: &str| run("sysctl", &["-n", k]);
    json!({
        "model": sys("hw.model"),
        "chip": sys("machdep.cpu.brand_string"),
        "memory_gb": sys("hw.memsize").and_then(|m| m.parse::<f64>().ok()).map(|m| m / (1u64 << 30) as f64),
        "cores": std::thread::available_parallelism().map(|n| n.get()).ok(),
        "os": run("sw_vers", &["-productVersion"]),
        "os_build": run("sw_vers", &["-buildVersion"]),
    })
}

fn commit() -> Value {
    let hash = run("git", &["rev-parse", "--short=12", "HEAD"]);
    let dirty = run("git", &["status", "--porcelain", "--untracked-files=no"]).map(|s| !s.is_empty());
    json!({ "hash": hash, "dirty": dirty })
}

fn unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// MARK: Statistics

fn median(v: &[f64]) -> f64 {
    let mut v: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// (max − min) / median in %.
fn spread(v: &[f64]) -> f64 {
    let m = median(v);
    let (lo, hi) = v
        .iter()
        .filter(|x| x.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    if m > 0.0 && hi >= lo {
        100.0 * (hi - lo) / m
    } else {
        f64::NAN
    }
}

fn nums(laps: &[Value], f: impl Fn(&Value) -> Option<f64>) -> Vec<f64> {
    laps.iter().map(|l| f(l).unwrap_or(f64::NAN)).collect()
}

fn jnum(x: f64) -> Value {
    if x.is_finite() { json!(x) } else { Value::Null }
}

fn geomean(v: &[f64]) -> f64 {
    let v: Vec<f64> = v.iter().copied().filter(|x| x.is_finite() && *x > 0.0).collect();
    if v.is_empty() {
        return f64::NAN;
    }
    (v.iter().map(|x| x.ln()).sum::<f64>() / v.len() as f64).exp()
}

// MARK: Mode A: throughput

/// The renderer switches this run uses (A/B comparisons: SA on or off, BLA), as the JSON
/// records them.
fn renderer_config<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<Value> {
    let mut r = Renderer::new(backend.clone())?;
    o.apply_renderer(&mut r);
    // A live run: the app takes the mode from the environment the CLI passes on.
    let sa = match o.get("sa").and_then(almondbread_render::scheduler::SaMode::parse) {
        Some(m) => m,
        None => r.config.local_sa.mode,
    };
    Ok(json!({
        "sa": format!("{sa:?}"),
        "sa_env": std::env::var("ALMONDBREAD_SA").ok(),
        "bla": format!("{:?}", r.config.bla),
    }))
}

/// A renderer set up like the app's for the flight (script settings, one point per pixel).
fn flight_renderer<B: GpuBackend + Clone>(backend: &B, script: &FlightScript, o: &Options) -> Result<Renderer<B>> {
    let mut r = Renderer::new(backend.clone())?;
    // Renderer switches for A/B runs (`--sa 0|1|auto`, `--bla 0|1|auto`, ...); the default is
    // the app's (and `ALMONDBREAD_SA`).
    o.apply_renderer(&mut r);
    if let Some(s) = &script.settings {
        apply_json_settings(s, &mut r.settings);
    }
    let (w, h) = (script.size.0 as f64, script.size.1 as f64);
    r.handle_input(InputEvent::Resize { width: w, height: h }, 0.0);
    r.viewport = start_viewport(script.start.as_ref(), w, h, 1.0).map_err(crate::options::CommandError)?;
    Ok(r)
}

/// Refreshes back to back (no display) until the view is settled: refined, extended and
/// anti-aliased, and its last frame finished on the GPU.
fn settle<B: GpuBackend>(
    r: &mut Renderer<B>,
    target: &B::Texture,
    size: (usize, usize),
    timeout: f64,
) -> Result<usize> {
    let deadline = r.clock() + timeout;
    let mut refreshes = 0;
    loop {
        let now = r.clock();
        let out = r.render_frame(now, size.0, size.1, 1.0, &mut |_| Some((target.clone(), None)));
        refreshes += 1;
        if r.is_settled() && !r.retry_pending() {
            r.finish_frames();
            return Ok(refreshes);
        }
        if now > deadline {
            return err(format!("view did not settle within {timeout} s"));
        }
        if !out.presented {
            // GPU busy or a reference computing: do not spin a core.
            std::thread::sleep(Duration::from_micros(250));
        }
    }
}

fn throughput_lap<B: GpuBackend + Clone>(
    backend: &B,
    script: &FlightScript,
    path: &[almondbread_core::flight::CameraSample],
    check_every: usize,
    lap_name: &str,
    o: &Options,
) -> Result<Value> {
    let size = script.size;
    let mut r = flight_renderer(backend, script, o)?;
    // The fade-in of refined pixels is presentation only (real time); every frame here must
    // reach the settled frame, so it would only add idle refreshes.
    r.config.fade_tau = 1e-6;
    let target = backend.create_texture(size.0, size.1, TextureFormat::Bgra8Unorm, Storage::Shared)?;
    r.frame_stats = Some(FrameStats::default());
    let n = script.segments.len().max(1);
    let mut wall = vec![0.0; n];
    let mut cpu = vec![0.0; n];
    let mut gpu = vec![0.0; n];
    let mut frames = vec![0usize; n];
    let mut checks: Vec<Value> = Vec::new();
    let mut worst = 0.0f64;
    let mut jump_checks: Vec<Value> = Vec::new();
    let mut worst_jump = 0.0f64;
    let gpu_now = |r: &Renderer<B>| r.frame_stats.as_ref().map_or(0.0, |f| f.chunk_gpu + f.colorize_gpu);
    for (i, s) in path.iter().enumerate() {
        let seg = script.segment_at(s.t).unwrap_or(0);
        let last_of_segment = path.get(i + 1).is_none_or(|n| script.segment_at(n.t) != Some(seg));
        r.viewport = s.viewport.clone();
        let (t0, c0, g0) = (Instant::now(), cpu_time(), gpu_now(&r));
        settle(&mut r, &target, size, 120.0)
            .map_err(|e| crate::options::CommandError(format!("frame {i} (t {:.1} s): {e}", s.t)))?;
        wall[seg] += t0.elapsed().as_secs_f64();
        cpu[seg] += cpu_time() - c0;
        gpu[seg] += gpu_now(&r) - g0;
        frames[seg] += 1;
        if last_of_segment {
            let mut px = vec![0u8; size.0 * size.1 * 4];
            backend.read_shared_texture(&target, size.0, size.1, TextureFormat::Bgra8Unorm, &mut px);
            let name = script.segments.get(seg).map_or("?", |s| s.name.as_str()).to_string();
            // `--stills dir`: the settled last frame of each segment, to look at.
            if let Some(dir) = o.get("stills") {
                fs::create_dir_all(dir)?;
                let path = format!("{dir}/{}-{name}.png", lap_name.replace(' ', "-"));
                almondbread_core::image::write_png_bgra(&path, &px, size.0, size.1)?;
            }
            // Not timed: the frame against a from-scratch render of the same view (a jump: new
            // renderer, its own auto iterations). The shown-vs-fresh check below uses the
            // shown frame's own limit, so a view settled at a too-low limit (all black) passes
            // it; this one does not.
            if o.get("jump-check") != Some("0") {
                let mut fresh = flight_renderer(backend, script, o)?;
                fresh.viewport = r.viewport.clone();
                let res = fresh.render_offscreen(size.0, size.1, false)?;
                let pct = block_difference(&px, &res.pixels, size.0, size.1);
                worst_jump = worst_jump.max(pct);
                jump_checks.push(json!({"frame": i, "t": s.t, "segment": name, "blocks_differ_pct": pct,
                                        "shown_iterations": r.stats.max_iterations, "jump_iterations": fresh.stats.max_iterations}));
            }
        }
        // Spot checks (not timed): the shown frame against a fresh render of the same view.
        if (check_every > 0 && i % check_every == 0) || last_of_segment {
            match r.displayed_frame_mismatch() {
                Some(m) => {
                    worst = worst.max(m.percent);
                    checks.push(
                        json!({"frame": i, "t": s.t, "segment": script.segments.get(seg).map(|s| s.name.clone()),
                                       "mismatch_pct": m.percent, "pixels": m.pixels}),
                    );
                }
                None => checks
                    .push(json!({"frame": i, "t": s.t, "mismatch_pct": null, "note": "no settled frame to compare"})),
            }
        }
    }
    let segments: Vec<Value> = script
        .segments
        .iter()
        .enumerate()
        .map(|(i, s)| json!({"name": s.name, "frames": frames[i], "wall_s": wall[i], "cpu_s": cpu[i], "gpu_s": gpu[i]}))
        .collect();
    let total: f64 = wall.iter().sum();
    eprintln!(
        "benchmark: {lap_name}: throughput {total:.2} s ({} frames, worst shown-vs-fresh {worst:.3} %)",
        frames.iter().sum::<usize>()
    );
    Ok(json!({
        "total_s": total,
        "cpu_s": cpu.iter().sum::<f64>(),
        "gpu_s": gpu.iter().sum::<f64>(),
        "frames": frames.iter().sum::<usize>(),
        "segments": segments,
        "spot_checks": checks,
        "worst_mismatch_pct": worst,
        "jump_checks": jump_checks,
        "worst_jump_blocks_pct": worst_jump,
        "valid": worst <= MAX_MISMATCH && worst_jump <= MAX_JUMP_BLOCKS,
    }))
}

// MARK: Mode B: real time, headless approximation

fn realtime_lap<B: GpuBackend + Clone>(
    backend: &B,
    script: &FlightScript,
    hz: f64,
    lap_name: &str,
    o: &Options,
) -> Result<Value> {
    let size = script.size;
    let mut r = flight_renderer(backend, script, o)?;
    let target = backend.create_texture(size.0, size.1, TextureFormat::Bgra8Unorm, Storage::Shared)?;
    // The flight starts from the settled start view, as in the app.
    settle(&mut r, &target, size, 60.0)?;
    r.frame_stats = Some(FrameStats::default());
    let base = r.clock() + 0.05;
    let mut rec = LiveRecorder::new(script, hz, base);
    let mut player = FlightPlayer::new(1.0);
    let refreshes = (script.duration * hz).round() as usize;
    let interval = 1.0 / hz;
    let mut prev = r.viewport.clone();
    let mut worst_late = 0.0f64;
    let c0 = cpu_time();
    for k in 0..refreshes {
        let t = k as f64 * interval;
        let now = base + t;
        let wait = now - r.clock();
        if wait > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(wait));
        } else if -wait > interval * 0.5 {
            rec.late_refreshes += 1;
            worst_late = worst_late.max(-wait);
        }
        let t_prev = if k == 0 { -interval } else { (k - 1) as f64 * interval };
        player.feed(&mut r, &script.events, t_prev, t, now);
        if rec.landing_in(t_prev, t) {
            r.first_job_frame = None;
        }
        let out = r.render_frame(now, size.0, size.1, 1.0, &mut |_| Some((target.clone(), None)));
        let moving = r.viewport != prev;
        if moving {
            prev = r.viewport.clone();
        }
        let settled = r.is_settled() && !r.retry_pending();
        rec.refresh(t, now, moving, out.presented, settled, r.first_job_frame);
    }
    r.finish_frames();
    let fs = r.frame_stats.take().unwrap_or_default();
    let mut report = rec.report();
    report["worst_late_ms"] = json!(worst_late * 1000.0);
    report["iterate_gpu_s"] = json!(fs.chunk_gpu);
    report["colorize_gpu_s"] = json!(fs.colorize_gpu);
    report["cpu_s"] = json!(cpu_time() - c0);
    report["preview_frames"] = json!(fs.preview_frames);
    report["skipped"] = json!(fs.skipped);
    eprintln!(
        "benchmark: {lap_name}: real time {:.1} % of due frames shown, p99 {:.1} ms, sharp {:.2} s in all",
        100.0 * report["presented_frac"].as_f64().unwrap_or(f64::NAN),
        report["p99_ms"].as_f64().unwrap_or(f64::NAN),
        report["sharp_total_s"].as_f64().unwrap_or(f64::NAN)
    );
    Ok(report)
}

// MARK: Summaries and scores

fn throughput_summary(laps: &[Value]) -> Value {
    let totals = nums(laps, |l| l["total_s"].as_f64());
    let names: Vec<String> = laps
        .first()
        .and_then(|l| l["segments"].as_array())
        .map(|a| {
            a.iter()
                .map(|s| s["name"].as_str().unwrap_or("?").to_string())
                .collect()
        })
        .unwrap_or_default();
    let segments: Vec<Value> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let f = |k: &str| median(&nums(laps, |l| l["segments"][i][k].as_f64()));
            json!({"name": name, "frames": laps[0]["segments"][i]["frames"], "wall_s": jnum(f("wall_s")),
                   "cpu_s": jnum(f("cpu_s")), "gpu_s": jnum(f("gpu_s")),
                   "spread_pct": jnum(spread(&nums(laps, |l| l["segments"][i]["wall_s"].as_f64())))})
        })
        .collect();
    json!({
        "laps": laps.len(),
        "total_s": jnum(median(&totals)),
        "spread_pct": jnum(spread(&totals)),
        "cpu_s": jnum(median(&nums(laps, |l| l["cpu_s"].as_f64()))),
        "gpu_s": jnum(median(&nums(laps, |l| l["gpu_s"].as_f64()))),
        "worst_mismatch_pct": laps.iter().filter_map(|l| l["worst_mismatch_pct"].as_f64()).fold(0.0, f64::max),
        "worst_jump_blocks_pct": laps.iter().filter_map(|l| l["worst_jump_blocks_pct"].as_f64()).fold(0.0, f64::max),
        "jump_checks": laps.first().map_or(Value::Null, |l| l["jump_checks"].clone()),
        "valid": laps.iter().all(|l| l["valid"].as_bool() == Some(true)),
        "segments": segments,
    })
}

fn realtime_summary(laps: &[Value]) -> Value {
    let m = |k: &str| median(&nums(laps, |l| l[k].as_f64()));
    let names: Vec<String> = laps
        .first()
        .and_then(|l| l["segments"].as_array())
        .map(|a| {
            a.iter()
                .map(|s| s["name"].as_str().unwrap_or("?").to_string())
                .collect()
        })
        .unwrap_or_default();
    let segments: Vec<Value> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let f = |k: &str| median(&nums(laps, |l| l["segments"][i][k].as_f64()));
            let nl = laps[0]["segments"][i]["landings"].as_array().map_or(0, Vec::len);
            let landings: Vec<Value> = (0..nl)
                .map(|j| {
                    let g = |k: &str| median(&nums(laps, |l| l["segments"][i]["landings"][j][k].as_f64()));
                    let censored = laps
                        .iter()
                        .filter(|l| l["segments"][i]["landings"][j]["censored"].as_bool() == Some(true))
                        .count();
                    json!({"at_s": laps[0]["segments"][i]["landings"][j]["at_s"], "first_image_ms": jnum(g("first_image_ms")),
                           "sharp_ms": jnum(g("sharp_ms")), "censored_laps": censored,
                           "window_ms": laps[0]["segments"][i]["landings"][j]["window_ms"]})
                })
                .collect();
            json!({"name": name, "due": jnum(f("due")), "presented_frac": jnum(f("presented_frac")),
                   "p99_ms": jnum(f("p99_ms")), "late_frames": jnum(f("late_frames")),
                   "missed_refreshes": jnum(f("missed_refreshes")), "landings": landings})
        })
        .collect();
    json!({
        "laps": laps.len(),
        "hz": laps.first().map_or(Value::Null, |l| l["hz"].clone()),
        "presented_frac": jnum(m("presented_frac")),
        "p99_ms": jnum(m("p99_ms")),
        "p50_ms": jnum(m("p50_ms")),
        "late_frames": jnum(m("late_frames")),
        "missed_refreshes": jnum(m("missed_refreshes")),
        "late_refreshes": jnum(m("late_refreshes")),
        "sharp_total_s": jnum(m("sharp_total_s")),
        "latency_p50_ms": jnum(m("latency_p50_ms")),
        "latency_p95_ms": jnum(m("latency_p95_ms")),
        "spread_pct": {
            "presented_frac": jnum(spread(&nums(laps, |l| l["presented_frac"].as_f64()))),
            "p99_ms": jnum(spread(&nums(laps, |l| l["p99_ms"].as_f64()))),
            "sharp_total_s": jnum(spread(&nums(laps, |l| l["sharp_total_s"].as_f64()))),
        },
        "segments": segments,
    })
}

/// Throughput score: 1000 × reference seconds / seconds.
fn throughput_score(summary: &Value, reference: &Value) -> f64 {
    let (r, t) = (
        reference["throughput"]["total_s"].as_f64().unwrap_or(f64::NAN),
        summary["total_s"].as_f64().unwrap_or(f64::NAN),
    );
    1000.0 * r / t
}

/// Real-time score: 1000 × geometric mean of shown-frame fraction, p99 frame time and total
/// time to sharp, each relative to the reference.
fn realtime_score(summary: &Value, reference: &Value) -> f64 {
    let g = |v: &Value, k: &str| v[k].as_f64().unwrap_or(f64::NAN);
    if reference.is_null() {
        return f64::NAN;
    }
    1000.0
        * geomean(&[
            g(summary, "presented_frac") / g(reference, "presented_frac"),
            g(reference, "p99_ms") / g(summary, "p99_ms"),
            g(reference, "sharp_total_s") / g(summary, "sharp_total_s"),
        ])
}

fn fmt(x: Option<f64>, digits: usize) -> String {
    match x {
        Some(v) if v.is_finite() => format!("{v:.digits$}"),
        _ => "-".into(),
    }
}

fn print_throughput(s: &Value, reference: &Value) {
    println!(
        "\nMode A, throughput (median of {} laps; every frame settled at full quality, no display):",
        s["laps"]
    );
    println!("| segment | frames | wall s | CPU s | GPU s | spread % | reference s |");
    println!("|---|---:|---:|---:|---:|---:|---:|");
    let refseg = |name: &str| {
        reference["throughput"]["segments"]
            .as_array()
            .and_then(|a| a.iter().find(|x| x["name"] == name))
            .and_then(|x| x["wall_s"].as_f64())
    };
    for seg in s["segments"].as_array().into_iter().flatten() {
        let name = seg["name"].as_str().unwrap_or("?");
        println!(
            "| {name} | {} | {} | {} | {} | {} | {} |",
            seg["frames"],
            fmt(seg["wall_s"].as_f64(), 2),
            fmt(seg["cpu_s"].as_f64(), 2),
            fmt(seg["gpu_s"].as_f64(), 2),
            fmt(seg["spread_pct"].as_f64(), 1),
            fmt(refseg(name), 2)
        );
    }
    println!(
        "| **total** | | **{}** | {} | {} | {} | {} |",
        fmt(s["total_s"].as_f64(), 2),
        fmt(s["cpu_s"].as_f64(), 2),
        fmt(s["gpu_s"].as_f64(), 2),
        fmt(s["spread_pct"].as_f64(), 1),
        fmt(reference["throughput"]["total_s"].as_f64(), 2)
    );
    let class = s["worst_mismatch_pct"].as_f64().unwrap_or(0.0);
    println!(
        "shown vs fresh render (spot checks, same limit): worst {} % of pixels differ in class ({})",
        fmt(Some(class), 3),
        if class <= MAX_MISMATCH {
            "OK"
        } else {
            "FAILED: reuse produced a frame a fresh render does not"
        }
    );
    let jump = s["worst_jump_blocks_pct"].as_f64().unwrap_or(0.0);
    println!(
        "settled vs from-scratch jump render (segment ends): worst {} % of 16x16 blocks differ ({})",
        fmt(Some(jump), 2),
        if jump <= MAX_JUMP_BLOCKS {
            "OK"
        } else {
            "FAILED: a settled frame differs from what a jump to the view renders"
        }
    );
    for c in s["jump_checks"].as_array().into_iter().flatten() {
        if c["blocks_differ_pct"].as_f64().unwrap_or(0.0) > MAX_JUMP_BLOCKS {
            println!(
                "  {} at {} s: {} % of blocks differ, limit {} shown vs {} after a jump",
                c["segment"].as_str().unwrap_or("?"),
                fmt(c["t"].as_f64(), 1),
                fmt(c["blocks_differ_pct"].as_f64(), 1),
                c["shown_iterations"],
                c["jump_iterations"]
            );
        }
    }
    if s["valid"].as_bool() != Some(true) {
        println!("THROUGHPUT RESULT INVALID: not every frame reached the full-quality image.");
    }
}

fn print_realtime(s: &Value, title: &str) {
    println!("\n{title} (median of {} laps at {} Hz):", s["laps"], s["hz"]);
    println!("| segment | due | shown % | p99 ms | late | missed | landings: first image / sharp ms |");
    println!("|---|---:|---:|---:|---:|---:|---|");
    for seg in s["segments"].as_array().into_iter().flatten() {
        let landings: Vec<String> = seg["landings"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|l| {
                let censored = l["censored_laps"].as_u64().unwrap_or(0);
                format!(
                    "@{}s {} / {}{}",
                    fmt(l["at_s"].as_f64(), 1),
                    fmt(l["first_image_ms"].as_f64(), 0),
                    fmt(l["sharp_ms"].as_f64(), 0),
                    if censored > 0 {
                        format!(
                            " (not sharp within {} ms in {censored} laps)",
                            fmt(l["window_ms"].as_f64(), 0)
                        )
                    } else {
                        String::new()
                    }
                )
            })
            .collect();
        println!(
            "| {} | {} | {} | {} | {} | {} | {} |",
            seg["name"].as_str().unwrap_or("?"),
            fmt(seg["due"].as_f64(), 0),
            fmt(seg["presented_frac"].as_f64().map(|x| 100.0 * x), 1),
            fmt(seg["p99_ms"].as_f64(), 1),
            fmt(seg["late_frames"].as_f64(), 0),
            fmt(seg["missed_refreshes"].as_f64(), 0),
            landings.join("; ")
        );
    }
    println!(
        "| **total** | {} | **{}** | **{}** | {} | {} | sharp **{} s** in all |",
        fmt(s["due"].as_f64(), 0),
        fmt(s["presented_frac"].as_f64().map(|x| 100.0 * x), 1),
        fmt(s["p99_ms"].as_f64(), 1),
        fmt(s["late_frames"].as_f64(), 0),
        fmt(s["missed_refreshes"].as_f64(), 0),
        fmt(s["sharp_total_s"].as_f64(), 2)
    );
    if s["latency_p50_ms"].is_number() {
        println!(
            "input to photon: p50 {} ms, p95 {} ms",
            fmt(s["latency_p50_ms"].as_f64(), 1),
            fmt(s["latency_p95_ms"].as_f64(), 1)
        );
    }
}

fn reference_note(reference: &Value) -> String {
    if reference.is_null() {
        return "no reference file: no scores".into();
    }
    format!(
        "reference: {} ({}){}",
        reference["machine"].as_str().unwrap_or("?"),
        reference["date"].as_str().unwrap_or("?"),
        if reference["provisional"].as_bool() == Some(true) {
            ", PROVISIONAL (not calibrated on an idle machine: scores are not comparable yet)"
        } else {
            ""
        }
    )
}

fn print_noise(noise: &Value) {
    if noise["noisy"].as_bool() == Some(true) {
        let bar = "!".repeat(78);
        println!(
            "\n{bar}\n!!! NOISY: the machine was busy ({}). Results are not comparable.\n{bar}",
            noise["why"].as_str().unwrap_or("")
        );
    } else {
        println!("\nmachine quiet: {}", noise["why"].as_str().unwrap_or(""));
    }
}

fn noise_verdict(before: &[Value], during: &[f64]) -> Value {
    let worst_gpu = before
        .iter()
        .filter_map(|p| p["gpu_utilization_pct"].as_f64())
        .fold(f64::NAN, f64::max);
    let worst_cpu_before = before
        .iter()
        .filter_map(|p| p["others_cpu_pct"].as_f64())
        .fold(f64::NAN, f64::max);
    let during_median = median(during);
    let noisy = before.iter().any(probe_is_noisy) || during_median > NOISY_CPU;
    let why = format!(
        "other processes {} % CPU before laps (worst), {} % during (median); GPU {} % busy before laps (worst); thresholds {NOISY_CPU} % CPU, {NOISY_GPU} % GPU",
        fmt(Some(worst_cpu_before), 0),
        fmt(Some(during_median), 0),
        fmt(Some(worst_gpu), 0)
    );
    json!({"noisy": noisy, "why": why, "before_laps": before, "others_cpu_during_pct": during})
}

fn load_json(path: &str) -> Result<Value> {
    let text = fs::read_to_string(path).map_err(|e| crate::options::CommandError(format!("{path}: {e}")))?;
    Ok(serde_json::from_str(&text)?)
}

fn reference_path(flight: &str, o: &Options) -> String {
    o.get("reference")
        .map(String::from)
        .unwrap_or_else(|| flight.replace(".json", "-reference.json"))
}

fn print_scores(result: &Value) {
    let s = &result["score"];
    println!(
        "\nSCORE {} = geometric mean of throughput {} and real time {} ({})",
        fmt(s["total"].as_f64(), 0),
        fmt(s["throughput"].as_f64(), 0),
        fmt(s["realtime"].as_f64(), 0),
        s["realtime_mode"].as_str().unwrap_or("-")
    );
    println!("{}", s["reference"].as_str().unwrap_or(""));
    if result["noise"]["noisy"].as_bool() == Some(true) {
        println!("NOISY RUN: do not compare these numbers.");
    }
    if s["throughput_valid"].as_bool() == Some(false) {
        println!("INVALID: the throughput run did not reach the full-quality image everywhere (see above).");
    }
}

fn write_result(result: &Value, o: &Options, tag: &str) -> Result<String> {
    let dir = o.get("out").unwrap_or("build/benchmark");
    fs::create_dir_all(dir)?;
    let hash = result["commit"]["hash"].as_str().unwrap_or("nocommit");
    let path = format!(
        "{dir}/flight-v{}-{tag}-{hash}-{}.json",
        result["flight"]["version"],
        unix_time()
    );
    fs::write(&path, serde_json::to_string_pretty(result)?)?;
    Ok(path)
}

/// The reference file from a run's medians (calibration; kb: synthesis/flight-benchmark).
fn write_reference(result: &Value, path: &str, provisional: bool, previous: &Value) -> Result<()> {
    let pick = |k: &str| {
        if result[k].is_null() {
            previous[k].clone()
        } else {
            result[k].clone()
        }
    };
    let realtime = |s: &Value| {
        if s.is_null() {
            Value::Null
        } else {
            json!({"presented_frac": s["presented_frac"], "p99_ms": s["p99_ms"], "sharp_total_s": s["sharp_total_s"]})
        }
    };
    let throughput = pick("throughput");
    let reference = json!({
        "_about": "Reference values of the flight benchmark (kb: synthesis/flight-benchmark): this machine scores 1000 in each mode. Written by `almondbread benchmark --write-reference`; calibrate on an idle machine (see the kb page) and commit.",
        "flight": result["flight"],
        "provisional": provisional,
        "machine": format!("{} {} GB", result["device"]["chip"].as_str().unwrap_or("?"), fmt(result["device"]["memory_gb"].as_f64(), 0)),
        "device": result["device"],
        "commit": result["commit"],
        "date": run("date", &["+%Y-%m-%d"]),
        "noisy": result["noise"]["noisy"],
        "renderer": result["renderer"],
        "throughput": if throughput.is_null() { Value::Null } else {
            json!({"total_s": throughput["total_s"], "segments": throughput["segments"].as_array().map(|a| a.iter().map(|s| json!({"name": s["name"], "wall_s": s["wall_s"]})).collect::<Vec<_>>())})
        },
        "realtime_approx": if result["realtime_approx"].is_null() { previous["realtime_approx"].clone() } else { realtime(&result["realtime_approx"]) },
        "realtime_live": if result["realtime_live"].is_null() { previous["realtime_live"].clone() } else { realtime(&result["realtime_live"]) },
    });
    fs::write(path, serde_json::to_string_pretty(&reference)? + "\n")?;
    println!(
        "wrote reference {path}{}",
        if provisional { " (provisional)" } else { "" }
    );
    Ok(())
}

fn scores(result: &mut Value, reference: &Value) {
    let a = if result["throughput"].is_null() {
        f64::NAN
    } else {
        throughput_score(&result["throughput"], reference)
    };
    let (b, mode) = if !result["realtime_live"].is_null() {
        (
            realtime_score(&result["realtime_live"], &reference["realtime_live"]),
            "live window",
        )
    } else if !result["realtime_approx"].is_null() {
        (
            realtime_score(&result["realtime_approx"], &reference["realtime_approx"]),
            "headless approximation",
        )
    } else {
        (f64::NAN, "-")
    };
    let total = geomean(&[a, b]);
    result["score"] = json!({
        "throughput": jnum(a), "realtime": jnum(b), "realtime_mode": mode,
        "total": jnum(if a.is_finite() && b.is_finite() { total } else { f64::NAN }),
        "reference": reference_note(reference),
        "reference_provisional": reference["provisional"],
        "throughput_valid": result["throughput"]["valid"],
    });
}

// MARK: Commands

pub fn benchmark_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    if let Some(a) = o.get("compare") {
        let Some(b) = o.positional.first() else {
            return err("--compare a.json b.json");
        };
        return compare(&load_json(a)?, &load_json(b)?, a, b);
    }
    if o.has("noise") {
        let p = noise_probe();
        println!("{}", serde_json::to_string_pretty(&p)?);
        return Ok(i32::from(probe_is_noisy(&p)));
    }
    let flight_path = o.get("flight").unwrap_or(DEFAULT_FLIGHT).to_string();
    let script = crate::fly::load_script(&flight_path)?;
    let Some(spec) = script.benchmark.clone() else {
        return err(format!("{flight_path}: not a benchmark flight (no `benchmark` object)"));
    };
    if script.segments.is_empty() {
        return err(format!("{flight_path}: no segments"));
    }
    if script.events.iter().any(|e| e.get("settings").is_some()) {
        return err("benchmark flights must not change settings mid-flight (mode A samples only the camera)");
    }
    let ref_path = reference_path(&flight_path, o);
    let reference = load_json(&ref_path).unwrap_or(Value::Null);
    let repeat = o.usize("repeat").unwrap_or(3).max(1);
    let warmup = o.usize("warmup").unwrap_or(1);
    let mut result = json!({
        "tool": "almondbread benchmark",
        "flight": {"path": flight_path, "name": spec.name, "version": spec.version, "duration_s": script.duration,
                    "size": format!("{}x{}", script.size.0, script.size.1), "step_hz": spec.step_hz, "hz": spec.hz},
        "commit": commit(),
        "device": device(),
        "unix_time": unix_time(),
        "renderer": renderer_config(backend, o)?,
        "repeat": repeat,
        "warmup": warmup,
        "throughput": null, "realtime_approx": null, "realtime_live": null,
    });
    println!(
        "flight benchmark {} v{}: {} s at {}x{}, {} laps after {} warm-up; {}",
        spec.name,
        spec.version,
        script.duration,
        script.size.0,
        script.size.1,
        repeat,
        warmup,
        reference_note(&reference)
    );
    println!(
        "renderer: local SA {}, BLA {} (A/B: --sa 0|1|auto, --bla 0|1|auto)",
        result["renderer"]["sa"].as_str().unwrap_or("?"),
        result["renderer"]["bla"].as_str().unwrap_or("?")
    );
    let me = std::process::id();
    let mut probes = Vec::new();
    let mut during = Vec::new();

    if let Some(file) = o.get("live") {
        // Score a window run written by the app (`--flight-bench=`).
        let live = load_json(file)?;
        result["realtime_live"] = realtime_summary(live["laps"].as_array().map_or(&[][..], Vec::as_slice));
        result["live_run"] = live.clone();
        result["noise"] = live["noise"].clone();
    } else if let Some(app) = o.get("live-app") {
        // Launch the app's background test window and sample the machine meanwhile.
        let out_dir = o.get("out").unwrap_or("build/benchmark");
        fs::create_dir_all(out_dir)?;
        let raw = format!("{out_dir}/live-raw-{}.json", unix_time());
        let abs = |p: &str| {
            std::fs::canonicalize(p)
                .map(|x| x.display().to_string())
                .unwrap_or_else(|_| p.to_string())
        };
        let flight_abs = abs(&flight_path);
        let raw_abs = format!("{}/{}", abs(out_dir), raw.rsplit('/').next().unwrap_or(&raw));
        probes.push(noise_probe());
        let sampler = Sampler::start(vec![me]);
        let mut cmd = Command::new(app);
        // The app reads the SA switch from the environment (A/B: `--sa 0`).
        if let Some(sa) = o.get("sa") {
            cmd.env("ALMONDBREAD_SA", sa);
        }
        let status = cmd
            .arg(format!("--flight-bench={flight_abs}"))
            .arg(format!("--flight-bench-out={raw_abs}"))
            .arg(format!("--flight-bench-laps={repeat}"))
            .arg(format!("--flight-bench-warmup={warmup}"))
            .status()?;
        during = sampler.finish();
        if !status.success() {
            return err(format!("{app} --flight-bench failed: {status}"));
        }
        let live = load_json(&raw_abs)?;
        result["realtime_live"] = realtime_summary(live["laps"].as_array().map_or(&[][..], Vec::as_slice));
        result["live_run"] = live;
    } else {
        let modes = o.get("modes").unwrap_or("a,approx").to_string();
        let do_a = modes.split(',').any(|m| m == "a");
        let do_b = modes.split(',').any(|m| m == "approx" || m == "b");
        let check_every = o.usize("check-every").unwrap_or(100);
        let hz = o.f64("hz").unwrap_or(spec.hz);
        let base = Renderer::new(backend.clone())?.settings;
        let path =
            camera_path(&script, &base, script.size, spec.hz, spec.step_hz).map_err(crate::options::CommandError)?;
        let mut a_laps = Vec::new();
        let mut b_laps = Vec::new();
        for lap in 0..warmup + repeat {
            let warm = lap < warmup;
            let name = if warm {
                format!("warm-up {}", lap + 1)
            } else {
                format!("lap {}", lap - warmup + 1)
            };
            if !warm {
                // Our process idles here: what the GPU and CPU do now is someone else.
                std::thread::sleep(Duration::from_millis(300));
                probes.push(noise_probe());
            }
            let sampler = Sampler::start(vec![me]);
            if do_a {
                let l = throughput_lap(backend, &script, &path, check_every, &name, o)?;
                if !warm {
                    a_laps.push(l);
                }
            }
            if do_b {
                let l = realtime_lap(backend, &script, hz, &name, o)?;
                if !warm {
                    b_laps.push(l);
                }
            }
            let s = sampler.finish();
            if !warm {
                during.extend(s);
            }
        }
        if do_a {
            result["throughput"] = throughput_summary(&a_laps);
            result["throughput_laps"] = json!(a_laps);
        }
        if do_b {
            result["realtime_approx"] = realtime_summary(&b_laps);
            result["realtime_approx_laps"] = json!(b_laps);
        }
    }
    if result["noise"].is_null() {
        result["noise"] = noise_verdict(&probes, &during);
    }
    scores(&mut result, &reference);

    print_noise(&result["noise"]);
    if !result["throughput"].is_null() {
        print_throughput(&result["throughput"], &reference);
    }
    if !result["realtime_approx"].is_null() {
        print_realtime(&result["realtime_approx"], "Mode B, real time, headless approximation");
    }
    if !result["realtime_live"].is_null() {
        print_realtime(&result["realtime_live"], "Mode B, real time, live window");
    }
    print_scores(&result);
    let tag = if result["realtime_live"].is_null() {
        "headless"
    } else {
        "live"
    };
    let path = write_result(&result, o, tag)?;
    println!("wrote {path}");
    let valid = result["throughput"].is_null() || result["throughput"]["valid"].as_bool() == Some(true);
    if let Some(p) = o.get("write-reference") {
        let noisy = result["noise"]["noisy"].as_bool() == Some(true);
        let final_ = o.get("final") == Some("1");
        if (!valid || (noisy && final_)) && o.get("force") != Some("1") {
            eprintln!(
                "almondbread: not writing {p}: the run is {} (--force 1 overrides)",
                if valid { "noisy" } else { "INVALID" }
            );
        } else {
            let previous = load_json(p).unwrap_or(Value::Null);
            write_reference(&result, p, !final_, &previous)?;
        }
    }
    if !valid {
        eprintln!(
            "almondbread: benchmark FAILED: a settled frame differed from a fresh or a from-scratch render (result INVALID)"
        );
        return Ok(1);
    }
    Ok(0)
}

/// `--compare a.json b.json`: deltas per segment (b relative to a).
fn compare(a: &Value, b: &Value, an: &str, bn: &str) -> Result<i32> {
    if a["flight"]["version"] != b["flight"]["version"] || a["flight"]["name"] != b["flight"]["name"] {
        return err(format!(
            "different flights ({} v{} vs {} v{}): scores compare only within one version",
            a["flight"]["name"], a["flight"]["version"], b["flight"]["name"], b["flight"]["version"]
        ));
    }
    let delta = |x: Option<f64>, y: Option<f64>| match (x, y) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() && x != 0.0 => format!("{:+.1} %", 100.0 * (y - x) / x),
        _ => "-".into(),
    };
    let sa = |v: &Value| v["renderer"]["sa"].as_str().unwrap_or("?").to_string();
    if sa(a) != sa(b) {
        println!("NOTE: different local SA modes: a {}, b {}", sa(a), sa(b));
    }
    println!(
        "a: {an} ({} {}{})\nb: {bn} ({} {}{})",
        a["commit"]["hash"].as_str().unwrap_or("?"),
        a["device"]["chip"].as_str().unwrap_or("?"),
        if a["noise"]["noisy"].as_bool() == Some(true) {
            ", NOISY"
        } else {
            ""
        },
        b["commit"]["hash"].as_str().unwrap_or("?"),
        b["device"]["chip"].as_str().unwrap_or("?"),
        if b["noise"]["noisy"].as_bool() == Some(true) {
            ", NOISY"
        } else {
            ""
        },
    );
    for (n, v) in [("a", a), ("b", b)] {
        if v["throughput"]["valid"].as_bool() == Some(false) {
            println!(
                "{n} is INVALID (a settled frame was not the full-quality image): its throughput is not comparable"
            );
        }
        if v["renderer"]["sa"].is_string() {
            println!(
                "{n}: local SA {}, BLA {}",
                v["renderer"]["sa"].as_str().unwrap_or("?"),
                v["renderer"]["bla"].as_str().unwrap_or("?")
            );
        }
    }
    if !a["throughput"].is_null() && !b["throughput"].is_null() {
        println!("\nthroughput (lower is better):");
        println!("| segment | a wall s | b wall s | Δ | a GPU s | b GPU s | Δ GPU | a CPU s | b CPU s | Δ CPU |");
        println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        let empty = vec![];
        let sa = a["throughput"]["segments"].as_array().unwrap_or(&empty);
        let sb = b["throughput"]["segments"].as_array().unwrap_or(&empty);
        let rows = sa
            .iter()
            .map(|x| (x["name"].as_str().unwrap_or("?").to_string(), x))
            .chain(std::iter::once(("**total**".to_string(), &a["throughput"])));
        for (name, x) in rows {
            let y = if name == "**total**" {
                &b["throughput"]
            } else {
                match sb.iter().find(|y| y["name"].as_str() == Some(name.as_str())) {
                    Some(y) => y,
                    None => continue,
                }
            };
            let wk = if name == "**total**" { "total_s" } else { "wall_s" };
            let g = |v: &Value, k: &str| v[k].as_f64();
            println!(
                "| {name} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                fmt(g(x, wk), 2),
                fmt(g(y, wk), 2),
                delta(g(x, wk), g(y, wk)),
                fmt(g(x, "gpu_s"), 2),
                fmt(g(y, "gpu_s"), 2),
                delta(g(x, "gpu_s"), g(y, "gpu_s")),
                fmt(g(x, "cpu_s"), 2),
                fmt(g(y, "cpu_s"), 2),
                delta(g(x, "cpu_s"), g(y, "cpu_s")),
            );
        }
    }
    for key in ["realtime_approx", "realtime_live"] {
        let (x, y) = (&a[key], &b[key]);
        if x.is_null() || y.is_null() {
            continue;
        }
        println!("\n{key}:");
        println!("| segment | a shown % | b shown % | a p99 ms | b p99 ms | Δ p99 | a sharp ms | b sharp ms |");
        println!("|---|---:|---:|---:|---:|---:|---|---|");
        let empty = vec![];
        for sx in x["segments"].as_array().unwrap_or(&empty) {
            let Some(sy) = y["segments"]
                .as_array()
                .and_then(|v| v.iter().find(|s| s["name"] == sx["name"]))
            else {
                continue;
            };
            let sharp = |s: &Value| {
                s["landings"]
                    .as_array()
                    .map(|l| {
                        l.iter()
                            .map(|l| fmt(l["sharp_ms"].as_f64(), 0))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default()
            };
            println!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                sx["name"].as_str().unwrap_or("?"),
                fmt(sx["presented_frac"].as_f64().map(|v| 100.0 * v), 1),
                fmt(sy["presented_frac"].as_f64().map(|v| 100.0 * v), 1),
                fmt(sx["p99_ms"].as_f64(), 1),
                fmt(sy["p99_ms"].as_f64(), 1),
                delta(sx["p99_ms"].as_f64(), sy["p99_ms"].as_f64()),
                sharp(sx),
                sharp(sy)
            );
        }
        println!(
            "| **total** | {} | {} | {} | {} | {} | {} s | {} s |",
            fmt(x["presented_frac"].as_f64().map(|v| 100.0 * v), 1),
            fmt(y["presented_frac"].as_f64().map(|v| 100.0 * v), 1),
            fmt(x["p99_ms"].as_f64(), 1),
            fmt(y["p99_ms"].as_f64(), 1),
            delta(x["p99_ms"].as_f64(), y["p99_ms"].as_f64()),
            fmt(x["sharp_total_s"].as_f64(), 2),
            fmt(y["sharp_total_s"].as_f64(), 2)
        );
    }
    println!(
        "\nscore: a {} (throughput {}, real time {}), b {} (throughput {}, real time {}): Δ {}",
        fmt(a["score"]["total"].as_f64(), 0),
        fmt(a["score"]["throughput"].as_f64(), 0),
        fmt(a["score"]["realtime"].as_f64(), 0),
        fmt(b["score"]["total"].as_f64(), 0),
        fmt(b["score"]["throughput"].as_f64(), 0),
        fmt(b["score"]["realtime"].as_f64(), 0),
        delta(a["score"]["total"].as_f64(), b["score"]["total"].as_f64())
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
        assert!((spread(&[9.0, 10.0, 11.0]) - 20.0).abs() < 1e-9);
        assert!((geomean(&[2.0, 8.0]) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn scores_are_relative_to_the_reference() {
        let reference = json!({"throughput": {"total_s": 100.0},
                               "realtime_approx": {"presented_frac": 0.9, "p99_ms": 20.0, "sharp_total_s": 10.0}});
        let mut r = json!({"throughput": {"total_s": 50.0},
                           "realtime_approx": {"presented_frac": 0.9, "p99_ms": 20.0, "sharp_total_s": 10.0},
                           "realtime_live": null});
        scores(&mut r, &reference);
        assert!((r["score"]["throughput"].as_f64().unwrap() - 2000.0).abs() < 1e-9);
        assert!((r["score"]["realtime"].as_f64().unwrap() - 1000.0).abs() < 1e-9);
        assert!((r["score"]["total"].as_f64().unwrap() - 2000f64.sqrt() * 1000f64.sqrt()).abs() < 1e-6);
    }
}
