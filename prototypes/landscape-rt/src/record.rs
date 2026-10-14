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
    sim.autopilot = script.autopilot;
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
    let t = std::time::Instant::now();
    for i in 0..frames {
        let input = script.input_at(i as f64 * dt);
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
    stab.report(&script.name);
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
            ("hilltops, level flight", Box::new(|h: &(f64, f64, f64, f64, usize)| h.1.abs() < 0.05) as Box<dyn Fn(&(f64, f64, f64, f64, usize)) -> bool>),
            ("hilltops, diving, rigid phase", Box::new(|h: &(f64, f64, f64, f64, usize)| h.1.abs() >= 0.05 && h.0 == 0.0)),
            ("hilltops, diving, transition", Box::new(|h: &(f64, f64, f64, f64, usize)| h.1.abs() >= 0.05 && h.0 > 0.0)),
        ] {
            let v: Vec<&(f64, f64, f64, f64, usize)> = hills.iter().filter(|h| sel(h) && h.4 > 2000).collect();
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
