//! Offscreen recorder: a scripted flight rendered at a fixed frame rate into a shared texture,
//! read back and piped into ffmpeg; plus temporal-stability metrics of the frames. The DE tile
//! budget per video frame is the live budget times `live_hz / fps`, so detail arrives as fast
//! as it would in the window at `live_hz`.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use objc2_metal::MTLCommandBuffer;

use crate::flight::Script;
use crate::gpu::read_texture;
use crate::sim::{Settings, Sim};
use crate::stats::{self, Stability};

#[derive(Clone, Debug, clap::Args)]
pub struct RecordArgs {
    #[command(flatten)]
    pub set: Settings,
    #[arg(long, default_value = "tour")]
    pub script: String,
    #[arg(long, default_value = "1920x1080")]
    pub size: String,
    #[arg(long, default_value_t = 60)]
    pub fps: u32,
    /// Output video (.mp4); omitted: metrics only.
    #[arg(long, short)]
    pub out: Option<PathBuf>,
    /// The display rate whose DE budget each video frame gets (budget_ms · live_hz / fps).
    #[arg(long, default_value_t = 120.0)]
    pub live_hz: f64,
    /// Per-frame CSV: frame, d1, d2, pop, S, tiles, missing, gpu render ms.
    #[arg(long)]
    pub csv: Option<PathBuf>,
    /// Also render each frame's camera with the previous frame's world and report the
    /// difference (popping from LOD/data changes, independent of camera motion).
    #[arg(long)]
    pub popcheck: bool,
    /// After the flight, check the clipmap's DE samples against CPU f64 (same reference orbit).
    #[arg(long)]
    pub check_end: bool,
    /// Write the pop-check difference image of the worst frame here (PNG via ffmpeg).
    #[arg(long)]
    pub worst: Option<PathBuf>,
}

fn luma_quarter(bgra: &[u8], w: usize, h: usize) -> Vec<f32> {
    let (qw, qh) = (w / 4, h / 4);
    let mut out = vec![0f32; qw * qh];
    for y in 0..qh {
        for x in 0..qw {
            let mut s = 0.0f32;
            for dy in 0..4 {
                let row = (y * 4 + dy) * w;
                for dx in 0..4 {
                    let p = (row + x * 4 + dx) * 4;
                    s += 0.0722 * bgra[p] as f32 + 0.7152 * bgra[p + 1] as f32 + 0.2126 * bgra[p + 2] as f32;
                }
            }
            out[y * qw + x] = s / (16.0 * 255.0);
        }
    }
    out
}

pub fn run(args: RecordArgs) -> Result<(), String> {
    let script = Script::named(&args.script).ok_or("unknown script")?;
    let (w, h) = {
        let mut it = args.size.split('x').map(|v| v.parse::<usize>().expect("size"));
        (it.next().unwrap(), it.next().unwrap())
    };
    let mut set = args.set.clone();
    set.budget_ms *= args.live_hz / args.fps as f64;
    let mut sim = Sim::new(set, script.start_speed)?;
    sim.flight.auto_zoom = sim.flight.auto_zoom || script.auto_zoom;
    // closed-loop target dive: the start location's centre (minibrot3: the period-3 nucleus,
    // the middle of the lake), as a world offset
    let goal = {
        let c = crate::sim::loc_center(&args.set.loc);
        [c.0 - sim.c0.0, c.1 - sim.c0.1]
    };
    let mut target_done = None::<usize>;
    eprintln!("{}", sim.ref_note);
    let t = std::time::Instant::now();
    let n = sim.prefill();
    eprintln!("prefill: {n} tiles in {:.2} s", t.elapsed().as_secs_f64());
    let target = sim.gpu.offscreen_target(w, h)?;
    if args.popcheck {
        sim.popcheck = Some(sim.gpu.offscreen_target(w, h)?);
    }
    let mut buf_a = Vec::new();
    let mut pops: Vec<(f64, f64, f64)> = Vec::new(); // mean |diff|, % pixels > 8, % > 24
    let mut hills: Vec<(f64, f64, f64, f64, usize)> = Vec::new(); // riser, dive rate, hilltop mean |diff|, % > 8, pixels
    let mut worst: (f64, usize, Vec<u8>, Vec<u8>) = (0.0, 0, Vec::new(), Vec::new());
    let mut ff = match &args.out {
        Some(out) => Some(
            Command::new("ffmpeg")
                .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "bgra", "-s", &format!("{w}x{h}"), "-r", &args.fps.to_string(), "-i", "-"])
                .args(["-c:v", "libx264", "-preset", "medium", "-crf", "16", "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
                .arg(out)
                .stdin(Stdio::piped())
                .spawn()
                .map_err(|e| format!("ffmpeg: {e}"))?,
        ),
        None => None,
    };
    let dt = 1.0 / args.fps as f64;
    let frames = (script.duration() * args.fps as f64).ceil() as usize;
    let mut buf = Vec::new();
    let mut stab = Stability::default();
    let fallback_mask = std::env::var("LRT_MASK").as_deref() == Ok("fallback");
    let mut fb: Vec<(f64, f64)> = Vec::new();
    let done0 = sim.tiles_done;
    let t = std::time::Instant::now();
    for i in 0..frames {
        let input = if let Some(h) = script.low {
            low_input(&mut sim, i as f64 * dt, h)
        } else if script.target {
            let (inp, done) = target_input(&sim, goal, script.auto_zoom, script.depth);
            if done && target_done.is_none() {
                target_done = Some(i);
            }
            if target_done.is_some_and(|d| i > d + 30) {
                break;
            }
            inp
        } else {
            script.input_at(i as f64 * dt)
        };
        let cb = sim.frame(dt, input, &target);
        cb.commit();
        cb.waitUntilCompleted();
        read_texture(&target, &mut buf);
        if let Some(a) = &sim.popcheck {
            read_texture(a, &mut buf_a);
            let (mut sum, mut n8, mut n24) = (0u64, 0usize, 0usize);
            let (mut hsum, mut hn, mut h8) = (0u64, 0usize, 0usize);
            let npx = w * h;
            for p in 0..npx {
                let mut m = 0i32;
                for c in 0..3 {
                    m = m.max((buf[p * 4 + c] as i32 - buf_a[p * 4 + c] as i32).abs());
                }
                if buf[p * 4 + 3] >= 128 {
                    hsum += m as u64;
                    hn += 1;
                    h8 += (m > 8) as usize;
                }
                sum += m as u64;
                n8 += (m > 8) as usize;
                n24 += (m > 24) as usize;
            }
            let e = (sum as f64 / npx as f64, 100.0 * n8 as f64 / npx as f64, 100.0 * n24 as f64 / npx as f64);
            if i > 0 && e.1 > worst.0 {
                worst = (e.1, i, buf_a.clone(), buf.clone());
            }
            pops.push(e);
            let r = sim.recs.last().unwrap();
            hills.push((r.riser, r.dive_rate, hsum as f64 / hn.max(1) as f64, 100.0 * h8 as f64 / hn.max(1) as f64, hn));
        }
        if fallback_mask {
            let (mut nd, mut co) = (0usize, 0usize);
            for p in 0..w * h {
                let a = buf[p * 4 + 3];
                nd += (a >= 192) as usize;
                co += (64..192).contains(&a) as usize;
            }
            fb.push((100.0 * nd as f64 / (w * h) as f64, 100.0 * co as f64 / (w * h) as f64));
        }
        stab.push(luma_quarter(&buf, w, h));
        if let Some(ff) = &mut ff {
            ff.stdin.as_mut().unwrap().write_all(&buf).map_err(|e| e.to_string())?;
        }
    }
    let wall = t.elapsed().as_secs_f64();
    if let Some(mut ff) = ff {
        drop(ff.stdin.take());
        ff.wait().map_err(|e| e.to_string())?;
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    let comp: Vec<f64> = sim.gpu_ms.iter().map(|g| g.1).filter(|&x| x > 0.0).collect();
    let rend: Vec<f64> = sim.gpu_ms.iter().map(|g| g.2).collect();
    let tiles: Vec<f64> = sim.recs.iter().map(|r| r.tiles as f64).collect();
    let missing: Vec<f64> = sim.recs.iter().map(|r| r.missing as f64).collect();
    let ahead: Vec<f64> = sim.recs.iter().map(|r| r.missing_ahead as f64).collect();
    println!("record {}: {frames} frames {w}x{h} @ {} fps in {wall:.1} s; S {:.3e} -> {:.3e}", script.name, args.fps, sim.recs.first().map(|r| r.s).unwrap_or(0.0), sim.flight.s);
    println!(
        "  GPU render ms p50 {:.2} p95 {:.2} p99 {:.2}; DE ms/frame p50 {:.2} p95 {:.2}; tiles/frame mean {:.0}; missing tiles p50 {:.0} p95 {:.0} max {:.0}; ahead (±60°, < 2 fog) p50 {:.0} p95 {:.0} max {:.0}",
        stats::pct(&rend, 0.5),
        stats::pct(&rend, 0.95),
        stats::pct(&rend, 0.99),
        stats::pct(&comp, 0.5),
        stats::pct(&comp, 0.95),
        tiles.iter().sum::<f64>() / tiles.len().max(1) as f64,
        stats::pct(&missing, 0.5),
        stats::pct(&missing, 0.95),
        stats::pct(&missing, 1.0),
        stats::pct(&ahead, 0.5),
        stats::pct(&ahead, 0.95),
        stats::pct(&ahead, 1.0)
    );
    let secs = sim.recs.len() as f64 * dt;
    println!("  tiles completed {} in {:.1} s of flight = {:.0} tiles/s (sim time; budget per video frame = live x {:.1})", sim.tiles_done - done0, secs, (sim.tiles_done - done0) as f64 / secs.max(1e-9), args.live_hz / args.fps as f64);
    if !fb.is_empty() {
        let nd: Vec<f64> = fb.iter().map(|x| x.0).collect();
        let co: Vec<f64> = fb.iter().map(|x| x.1).collect();
        println!("  fallback pixels: no data (geometry only) % p50 {:.4} p99 {:.4} max {:.4}; 2+ levels coarse % p50 {:.3} p99 {:.3} max {:.3}", stats::pct(&nd, 0.5), stats::pct(&nd, 0.99), stats::pct(&nd, 1.0), stats::pct(&co, 0.5), stats::pct(&co, 0.99), stats::pct(&co, 1.0));
    }
    let hagv: Vec<f64> = sim.recs.iter().map(|r| r.hag).collect();
    println!("  height above ground (S) p50 {:.3} min {:.3}", stats::pct(&hagv, 0.5), stats::pct(&hagv, 0.0));
    stab.report(&script.name);
    if args.check_end {
        sim.check(600);
    }
    if script.target {
        let f = &sim.flight;
        let miss = ((f.pos[0] - goal[0]).powi(2) + (f.pos[1] - goal[1]).powi(2)).sqrt();
        let c = (sim.c0.0 + f.pos[0], sim.c0.1 + f.pos[1]);
        let e = crate::mandel::eval(c.0, c.1, 50_000);
        println!(
            "  target: reached S {:.2e} at frame {:?} ({:.1} s); camera ground point {:.3} S = {:.2e} from the target; over the lake (interior): {}; height {:.2} S",
            f.s,
            target_done,
            target_done.unwrap_or(0) as f64 * dt,
            miss / f.s,
            miss,
            e.interior,
            f.hag
        );
    }
    if !pops.is_empty() {
        let m: Vec<f64> = pops.iter().map(|p| p.0).collect();
        let a: Vec<f64> = pops.iter().map(|p| p.1).collect();
        let b: Vec<f64> = pops.iter().map(|p| p.2).collect();
        println!("  pop check (same camera, previous vs new world):");
        println!("    mean |diff| (0..255)   p50 {:.4}  p99 {:.4}  max {:.4}", stats::pct(&m, 0.5), stats::pct(&m, 0.99), stats::pct(&m, 1.0));
        println!("    % pixels diff > 8      p50 {:.4}  p99 {:.4}  max {:.4}", stats::pct(&a, 0.5), stats::pct(&a, 0.99), stats::pct(&a, 1.0));
        println!("    % pixels diff > 24     p50 {:.4}  p99 {:.4}  max {:.4}  (worst frame {})", stats::pct(&b, 0.5), stats::pct(&b, 0.99), stats::pct(&b, 1.0), worst.1);
        // hilltops only (alpha mask), split by phase while diving
        for (name, sel) in [
            ("masked, level flight", Box::new(|h: &(f64, f64, f64, f64, usize)| h.1.abs() < 0.05) as Box<dyn Fn(&(f64, f64, f64, f64, usize)) -> bool>),
            ("masked, diving, rigid phase", Box::new(|h: &(f64, f64, f64, f64, usize)| h.1.abs() >= 0.05 && h.0 == 0.0)),
            ("masked, diving, transition", Box::new(|h: &(f64, f64, f64, f64, usize)| h.1.abs() >= 0.05 && h.0 > 0.0)),
        ] {
            let v: Vec<&(f64, f64, f64, f64, usize)> = hills.iter().filter(|h| sel(h) && h.4 > 2000).collect();
            let worst = v.iter().map(|h| h.3).fold(0.0, f64::max);
            let _ = worst;
            let m: Vec<f64> = v.iter().map(|h| h.2).collect();
            let a: Vec<f64> = v.iter().map(|h| h.3).collect();
            println!(
                "    {name:<32} frames {:4}  mean |diff| p50 {:.3} p95 {:.3}  % > 8: p50 {:.3} p95 {:.3}",
                v.len(),
                stats::pct(&m, 0.5),
                stats::pct(&m, 0.95),
                stats::pct(&a, 0.5),
                stats::pct(&a, 0.95)
            );
        }
        if let Some(p) = &args.worst {
            for (suffix, data) in [("a", &worst.2), ("b", &worst.3)] {
                let path = p.with_file_name(format!("{}-{}.png", p.file_stem().unwrap().to_string_lossy(), suffix));
                let mut ff = Command::new("ffmpeg")
                    .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "bgra", "-s", &format!("{w}x{h}"), "-i", "-"])
                    .arg(&path)
                    .stdin(Stdio::piped())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                ff.stdin.as_mut().unwrap().write_all(data).map_err(|e| e.to_string())?;
                drop(ff.stdin.take());
                ff.wait().map_err(|e| e.to_string())?;
            }
        }
    }
    if let Some(p) = &args.csv {
        let mut out = String::from("frame,d1,d2,pop,s,tiles,missing,render_ms\n");
        for (i, r) in sim.recs.iter().enumerate() {
            let d1 = if i >= 1 { stab.d1[i - 1] * 255.0 } else { 0.0 };
            let (d2, pop) = if i >= 2 { (stab.d2[i - 2] * 255.0, stab.pop[i - 2] * 100.0) } else { (0.0, 0.0) };
            let rm = sim.gpu_ms.iter().find(|g| g.0 == r.serial).map(|g| g.2).unwrap_or(0.0);
            out += &format!("{i},{d1:.4},{d2:.4},{pop:.4},{:.4e},{},{},{rm:.3}\n", r.s, r.tiles, r.missing);
        }
        std::fs::write(p, out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Stability metrics of an existing video (decoded by ffmpeg to 480x270 grey).
pub fn framestats(path: &PathBuf) -> Result<(), String> {
    let (w, h) = (480usize, 270usize);
    let mut child = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-i"])
        .arg(path)
        .args(["-vf", &format!("scale={w}:{h}:flags=area,format=gray"), "-f", "rawvideo", "-"])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut out = child.stdout.take().unwrap();
    let mut stab = Stability::default();
    let mut frame = vec![0u8; w * h];
    while out.read_exact(&mut frame).is_ok() {
        stab.push(frame.iter().map(|&b| b as f32 / 255.0).collect());
    }
    let _ = child.wait();
    stab.report(&path.display().to_string());
    Ok(())
}

/// A pilot for the target dive, through the same stick: point the nose at the target (turn,
/// push/pull), fly down to it; once the eye is within 0.6 S of the ground or the target is
/// within 25° of straight down, hold Shift+↑ (manual mode) until S ≤ `depth` (1e-7; the `deep`
/// script 1.5e-12, the depth limit). In the auto-zoom
/// mode it only steers: the steep nose does the zooming. Returns (input, reached).
fn target_input(sim: &Sim, t: [f64; 2], auto: bool, depth: f64) -> (crate::flight::Input, bool) {
    let f = &sim.flight;
    let dx = t[0] - f.pos[0];
    let dy = t[1] - f.pos[1];
    let horiz = (dx * dx + dy * dy).sqrt();
    let want_h = dy.atan2(dx);
    let wrap = |a: f64| (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    let dh = wrap(want_h - f.heading);
    // the target lies on the lake surface (height 0); pitch to look at it
    let mut want_p = (-f.z).atan2(horiz.max(1e-300));
    // auto-zoom pilot: hold the altitude until the target is steeply below (> 55°), then put
    // the nose on it; the steep nose does the zooming
    if auto && want_p > (-55f64).to_radians() {
        want_p = ((1.2 - f.hag) * 0.5).clamp(-0.3, 0.3); // hold ~1.2 S above the ground
    }
    let dp = want_p - f.pitch;
    let close = horiz < 1e-3 * f.s;
    let turn = if close { 0.0 } else { (dh * 2.0).clamp(-1.0, 1.0) * (horiz / f.s).min(1.0) };
    let pitch = (-dp * 3.0).clamp(-1.0, 1.0);
    let steep = f.pitch < (-65f64).to_radians();
    let zoom = if !auto && (f.hag < 0.6 || steep) && f.s > depth { 1.0 } else { 0.0 };
    let done = f.s <= 1.05 * depth;
    // near the target, slow down (Q) so the turn radius does not make the pilot orbit it
    let throttle = if horiz < 2.0 * f.s && f.speed_target > 0.25 { -1.0 } else { 0.0 };
    (crate::flight::Input { pitch, turn, throttle, zoom }, done)
}

/// A pilot for low flight (the user's L1 screenshot: alt 0.04 S, speed 0.16). First zooms in by
/// `zoom` (Shift+↑) while flying at ~0.5 S, then dives at −30° to 3× the wanted height, holds
/// `h` S above the ground with the stick, throttles back to 0.16 and steers towards the lower
/// ground ahead (follows valley floors, where the boundary detail is).
pub fn low_input(sim: &mut Sim, t: f64, low: (f64, f64)) -> crate::flight::Input {
    let (h, zoom) = low;
    let seek = zoom > 0.0;
    let zoom = zoom.abs();
    let s0 = sim.set.scale;
    let f = &sim.flight;
    let zooming = f.s > s0 / zoom * 1.02;
    let want_p = if zooming {
        ((0.5 - f.hag) * 1.0).clamp(-0.3, 0.2)
    } else if f.hag > 3.0 * h && f.pitch > (-29f64).to_radians() - 0.2 && f.hag > 0.2 {
        (-30f64).to_radians()
    } else {
        ((h - f.hag) * 4.0).clamp(-0.35, 0.25)
    };
    let pitch = (-(want_p - f.pitch) * 3.0).clamp(-1.0, 1.0);
    let throttle = if !zooming && f.hag < 4.0 * h && f.speed_target > 0.16 { -1.0 } else { 0.0 };
    // valley seeking: compare the ground 0.4 S ahead, 35° left and right
    let (pos, hd, s) = (f.pos, f.heading, f.s);
    let probe = |a: f64, g: &mut crate::ground::Ground| {
        let p = [pos[0] + 0.4 * s * (hd + a).cos(), pos[1] + 0.4 * s * (hd + a).sin()];
        g.smooth(p, s / 16.0)
    };
    let l = probe(0.6, &mut sim.ground);
    let r = probe(-0.6, &mut sim.ground);
    let wiggle = 0.25 * (std::f64::consts::TAU * t / 9.0).sin();
    let turn = if zooming || !seek { wiggle } else { (((r - l) / (0.05 * s)).clamp(-1.0, 1.0) * 0.6 + wiggle).clamp(-1.0, 1.0) };
    crate::flight::Input { pitch, turn, throttle, zoom: if zooming { 1.0 } else { 0.0 } }
}
