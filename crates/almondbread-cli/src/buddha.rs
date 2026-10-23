//! `almondbread buddha`: measurements of the Buddhabrot look (milestone BB, kb: plans/buddhabrot):
//! the rest accumulation's timeline (throughput, sampler, hits per pixel) with stills at given
//! times, the histogram's statistics, atomic contention (`--deposit 0`: count only), and the live
//! mode (`--live pan|zoom`: per-frame sampling at the gesture budget through the renderer's
//! frame loop, stills of frames in motion).
//!
//! ```text
//! almondbread buddha --center re,im --scale s --size WxH --look buddhabrot|nebulabrot
//!     [--seconds 20] [--snap 0.25,1,4,16] [--out dir/name] [--sampler uniform|mh] [--deposit 0]
//! almondbread buddha --live pan|zoom --center re,im --scale s --size WxH --look nebulabrot
//!     [--frames 240] [--rate 120] [--ppp 0.5] [--snap 30,120,240] [--out dir/name]
//! ```

use std::time::Instant;

use almondbread_core::buddha::{BinMap, BuddhaMode};
use almondbread_core::image::write_png_bgra;
use almondbread_render::backend::{ComputeEncoder, GpuBackend, Storage, Submission, TextureFormat};
use almondbread_render::buddha::{Buddha, HistKey, Sampler};
use almondbread_render::{FrameTarget, Renderer};

use crate::options::{Options, Result, err};

pub fn buddha_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    let (width, height) = o.size("size", (1200, 800))?;
    let mut r = Renderer::new(backend.clone())?;
    o.apply_settings(&mut r.settings);
    o.apply_renderer(&mut r);
    r.viewport = o.viewport(width, height)?;
    if !r.settings.buddha.is_on() {
        r.settings.buddha = BuddhaMode::Buddhabrot;
    }
    let out = o.get("out").unwrap_or("buddha").to_string();
    if let Some(seconds) = o.f64("idle") {
        return idle(&mut r, width, height, seconds, o.f64("ppp").unwrap_or(1.0), &out);
    }
    if let Some(motion) = o.get("live") {
        return live(backend, &mut r, o, width, height, motion, &out);
    }
    let mode = r.settings.buddha;
    let seconds = o.f64("seconds").unwrap_or(20.0);
    let mut snaps: Vec<f64> = o
        .get("snap")
        .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    snaps.sort_by(f64::total_cmp);
    let map = BinMap::new(&r.viewport, width, height, 1.0);
    if !map.supported() {
        return err("too deep for the Buddhabrot");
    }
    if o.has("compare") {
        return compare_runs(backend, o, &r, width, height, seconds);
    }
    let mut b = Buddha::new(backend)?;
    tune(&mut b, o);
    b.deposit = o.usize("deposit") != Some(0);
    let key = HistKey { map, mode };
    b.ensure_rest(backend, &key)?;
    let table = r.settings.palette_table(1024);
    b.ensure_palette(backend, &r.settings.palette_id, &table)?;
    let budget = o.f64("budget-ms").map_or(r.config.chunk_target, |m| m / 1000.0);
    println!(
        "{} {}x{} pixel {:.3e} (view share of the region {:.2e}), budget {:.1} ms per dispatch",
        mode.name(),
        width,
        height,
        map.pixel,
        map.region_share(),
        budget * 1000.0
    );
    println!("     t s   GPU s  sampler  proposals/s  iter/s  rho       accept  samples(u.e.)  hits/px");
    let start = Instant::now();
    let mut next_print = 0.0;
    let mut snap = 0;
    let mut clean_at = None;
    let mut snap_px: Vec<(f64, Vec<u8>)> = Vec::new();
    loop {
        b.harvest(backend);
        let t = start.elapsed().as_secs_f64();
        let rest = b.rest.as_ref().expect("rest");
        let c = rest.done;
        if t >= next_print || t >= seconds {
            next_print = t + (seconds / 20.0).max(0.25);
            println!(
                "{t:8.2} {:7.2}  {:7}  {:11.3e}  {:6.2e}  {:8.2e}  {:5.3}  {:13.3e}  {:8.1}",
                c.gpu_time,
                if rest.sampler() == Sampler::Metropolis {
                    "MH"
                } else {
                    "uniform"
                },
                c.proposals as f64 / c.gpu_time.max(1e-9),
                c.iterations as f64 / c.gpu_time.max(1e-9),
                c.rho().unwrap_or(f64::NAN),
                c.accepted as f64 / c.proposals.max(1) as f64,
                rest.samples(),
                rest.mean_hits()
            );
        }
        if clean_at.is_none() && rest.mean_hits() >= almondbread_render::scheduler::REST_TARGET_HITS {
            clean_at = Some(t);
            println!(
                "clean (target {} hits/px) at {t:.2} s",
                almondbread_render::scheduler::REST_TARGET_HITS
            );
        }
        if snap < snaps.len() && t >= snaps[snap] {
            let path = format!("{out}-{}s.png", snaps[snap]);
            b.wait_all(backend);
            let px = write_rest(backend, &mut b, width, height, &path)?;
            let hits = b.rest.as_ref().expect("rest").mean_hits();
            let d = snap_px
                .last()
                .map_or(String::new(), |(_, q)| format!(", {}", image_change(q, &px)));
            println!("  {path} (hits/px {hits:.1}{d} vs the previous snap)");
            snap_px.push((hits, px));
            snap += 1;
        }
        if t >= seconds {
            break;
        }
        while b.in_flight() < 2 {
            b.sample_rest(backend, budget)?;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    b.wait_all(backend);
    let last = write_rest(backend, &mut b, width, height, &format!("{out}.png"))?;
    // How far each snap still is from the final image (the remaining visible change).
    for (hits, px) in &snap_px {
        println!(
            "  snap at {hits:.0} hits/px vs the final image: {}",
            image_change(px, &last)
        );
    }
    // Tone curve variants of the final histogram: `--variants white:gamma,white:gamma`.
    if let Some(v) = o.get("variants") {
        for t in v.split(',') {
            let f: Vec<f32> = t.split(':').filter_map(|x| x.parse().ok()).collect();
            if let [w, g, k] = f[..] {
                b.fixed_tone = Some((w, g, k));
                write_rest(backend, &mut b, width, height, &format!("{out}-w{w}-g{g}-b{k}.png"))?;
            }
        }
    }
    if let Some(h) = b.read_rest(backend) {
        for ch in 0..h.channels {
            let mut v: Vec<u32> = h.plane(ch).to_vec();
            v.sort_unstable();
            let q = |p: f64| v[((v.len() - 1) as f64 * p) as usize];
            let mean = v.iter().map(|&x| f64::from(x)).sum::<f64>() / v.len() as f64;
            println!(
                "channel {ch}: mean {mean:.1}  p10 {}  p50 {}  p90 {}  p99 {}  p99.9 {}  max {}",
                q(0.1),
                q(0.5),
                q(0.9),
                q(0.99),
                q(0.999),
                v[v.len() - 1]
            );
        }
    }
    Ok(0)
}

fn tune<B: GpuBackend>(b: &mut Buddha<B>, o: &Options) {
    if let Some(v) = o.f64("p-jump") {
        b.p_jump = v as f32;
    }
    if let Some(v) = o.f64("r-span") {
        b.r_span = v as f32;
    }
    if let Some(v) = o.f64("r-max") {
        b.r_max = v as f32;
    }
    if let (Some(w), Some(g), Some(k)) = (o.f64("white"), o.f64("gamma"), o.f64("black")) {
        b.fixed_tone = Some((w as f32, g as f32, k as f32));
    }
    b.force_sampler = match o.get("sampler") {
        Some("uniform") => Some(Sampler::Uniform),
        Some("mh") => Some(Sampler::Metropolis),
        _ => None,
    };
}

/// Noise at equal wall time: two runs with different seeds for `seconds` each, compared
/// (relative L1 of the normalised histograms, per pixel and over 4x4 blocks).
fn compare_runs<B: GpuBackend>(
    backend: &B,
    o: &Options,
    r: &Renderer<B>,
    width: usize,
    height: usize,
    seconds: f64,
) -> Result<i32> {
    use almondbread_core::buddha::compare;
    let map = BinMap::new(&r.viewport, width, height, 1.0);
    let key = HistKey {
        map,
        mode: r.settings.buddha,
    };
    let mut runs = Vec::new();
    for seed in [1u32, 2] {
        let mut b = Buddha::new(backend)?;
        tune(&mut b, o);
        b.set_seed(seed.wrapping_mul(0x9E37_79B9));
        let start = Instant::now();
        b.accumulate(backend, &key, r.config.chunk_target, |_| {
            start.elapsed().as_secs_f64() >= seconds
        })?;
        let h = b.read_rest(backend).expect("rest");
        let rest = b.rest.as_ref().expect("rest");
        println!(
            "run {seed}: {:?}, {:.3e} samples (u.e.), {:.1} hits/px, rho {:.2e}, accept {:.4}, in S {:.4}, proposals {:.3e}",
            rest.sampler(),
            rest.samples(),
            rest.mean_hits(),
            rest.done.rho().unwrap_or(f64::NAN),
            rest.done.accepted as f64 / rest.done.proposals.max(1) as f64,
            rest.done.steps as f64 / rest.done.proposals.max(1) as f64,
            rest.done.proposals as f64
        );
        runs.push(h);
    }
    for ch in 0..runs[0].channels {
        let (_, l1) = compare(&runs[0], &runs[1], ch, 1);
        let (_, l4) = compare(&runs[0], &runs[1], ch, 4);
        println!("channel {ch}: run-to-run L1 per pixel {l1:.4}, over 4x4 blocks {l4:.4}");
    }
    Ok(0)
}

/// Mean and 99th-percentile absolute difference of two BGRA8 images (largest channel, 8-bit
/// levels) and the share of pixels that differ by more than one level.
fn image_change(a: &[u8], b: &[u8]) -> String {
    let mut d: Vec<u8> = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .map(|(p, q)| (0..3).map(|k| p[k].abs_diff(q[k])).max().unwrap_or(0))
        .collect();
    let n = d.len().max(1);
    let mean = d.iter().map(|&x| f64::from(x)).sum::<f64>() / n as f64;
    let over = d.iter().filter(|&&x| x > 1).count() as f64 / n as f64;
    d.sort_unstable();
    format!(
        "mean |d| {mean:.2} levels, p99 {}, {:.1} % of pixels > 1 level",
        d[(n - 1) * 99 / 100],
        100.0 * over
    )
}

fn write_rest<B: GpuBackend>(
    backend: &B,
    b: &mut Buddha<B>,
    width: usize,
    height: usize,
    path: &str,
) -> Result<Vec<u8>> {
    b.request_levels(backend)?;
    b.poll_levels(backend, true);
    let target = backend.create_texture(width, height, TextureFormat::Bgra8Unorm, Storage::Shared)?;
    let lin = backend.create_texture(width, height, TextureFormat::Rgba16Float, Storage::Private)?;
    let mut enc = backend.begin("buddha still")?;
    b.encode_tone_rest(&mut enc, &target, &lin, (width, height));
    enc.commit().wait();
    let mut px = vec![0u8; width * height * 4];
    backend.read_texture(&target, width, height, TextureFormat::Bgra8Unorm, &mut px)?;
    write_png_bgra(path, &px, width, height)?;
    Ok(px)
}

/// The live mode through the renderer's frame loop: `frames` refreshes at `rate` Hz of a pan or
/// zoom, each frame's sampling at the gesture budget; GPU time per frame and stills of frames.
fn live<B: GpuBackend + Clone>(
    backend: &B,
    r: &mut Renderer<B>,
    o: &Options,
    width: usize,
    height: usize,
    motion: &str,
    out: &str,
) -> Result<i32> {
    let frames = o.usize("frames").unwrap_or(240);
    let rate = o.f64("rate").unwrap_or(120.0);
    let ppp = o.f64("ppp").unwrap_or(1.0);
    let rest_first = o.f64("rest").unwrap_or(1.0);
    let snaps: Vec<usize> = o
        .get("snap")
        .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    let target = backend.create_texture(width, height, TextureFormat::Bgra8Unorm, Storage::Shared)?;
    let dt = 1.0 / rate;
    let mut now = 1.0;
    let mut acquire = |_: bool| -> Option<FrameTarget<B>> { Some((target.clone(), None)) };
    // Rest first (the motion starts from a rested image, as in the app).
    let t0 = Instant::now();
    loop {
        if t0.elapsed().as_secs_f64() >= rest_first {
            break;
        }
        r.render_frame(now, width, height, ppp, &mut acquire);
        now += dt;
        std::thread::sleep(std::time::Duration::from_secs_f64(dt));
    }
    r.finish_frames();
    let save = |r: &mut Renderer<B>, path: &str| -> Result<()> {
        r.finish_frames();
        let mut px = vec![0u8; width * height * 4];
        backend.read_texture(&target, width, height, TextureFormat::Bgra8Unorm, &mut px)?;
        write_png_bgra(path, &px, width, height)?;
        println!("  {path}");
        Ok(())
    };
    save(r, &format!("{out}-rest.png"))?;
    let mut gpu = Vec::new();
    // `--zoom-rate`: octaves per second of the zoom motions. `bursts`: zoom 0.25 s, hold 0.1 s
    // (the gesture goes on), every fourth burst a 0.4 s rest (the view settles in between).
    let zoom_rate = o.f64("zoom-rate").unwrap_or(1.0);
    // `--trace 1`: per frame what was shown (the look's live / rest image or the escape-time
    // colouring), the live estimate and the shown frame's mean colour (reads every frame back).
    let trace = o.usize("trace").unwrap_or(0) != 0;
    let mut switches = 0usize;
    let mut colour_jumps = 0usize;
    let mut last_shown: Option<(String, [f64; 3])> = None;
    let mut segments: Vec<(usize, String)> = Vec::new();
    for f in 1..=frames {
        let t = f as f64 * dt;
        let moving = match motion {
            "pan" => {
                r.viewport.pan_by_points(-0.25 * width as f64 * ppp * dt, 0.0);
                true
            }
            "zoom" => {
                r.viewport.zoom_by(2f64.powf(zoom_rate * dt), 0.0, 0.0);
                true
            }
            "bursts" => {
                let k = (t / 0.35) as usize;
                let phase = t - k as f64 * 0.35;
                let rest = k % 4 == 3;
                let on = phase < 0.25 && !rest;
                if on {
                    r.viewport.zoom_by(2f64.powf(2.0 * zoom_rate * dt), 0.0, 0.0);
                }
                on
            }
            _ => return err("--live pan|zoom|bursts"),
        };
        if moving {
            r.note_interaction(now);
        }
        let tf = Instant::now();
        r.render_frame(now, width, height, ppp, &mut acquire);
        let busy = tf.elapsed().as_secs_f64();
        if let Some(b) = r.buddha_sampler() {
            gpu.push(b.last_live.gpu_time * 1000.0);
        }
        if trace {
            let (shown, est) = r.buddha_shown();
            let name = match shown {
                almondbread_render::BuddhaShown::None => "escape".to_string(),
                almondbread_render::BuddhaShown::Live => "live".to_string(),
                almondbread_render::BuddhaShown::Rest(_) => "rest".to_string(),
            };
            r.finish_frames();
            let mut px = vec![0u8; width * height * 4];
            backend.read_texture(&target, width, height, TextureFormat::Bgra8Unorm, &mut px)?;
            let mut m = [0f64; 3];
            for p in px.chunks_exact(4) {
                m[0] += f64::from(p[2]);
                m[1] += f64::from(p[1]);
                m[2] += f64::from(p[0]);
            }
            let n = (width * height) as f64;
            let m = m.map(|x| x / n);
            if let Some((prev, pm)) = &last_shown {
                if *prev != name {
                    switches += 1;
                }
                let d = (0..3).map(|k| (m[k] - pm[k]).abs()).fold(0.0, f64::max);
                if d > 8.0 {
                    colour_jumps += 1;
                }
            }
            if segments.last().is_none_or(|s| s.1 != name) {
                segments.push((f, name.clone()));
            }
            println!(
                "frame {f:4} t {t:6.3} {} width {:.3e} shown {name:6} estimate {} mean rgb {:5.1} {:5.1} {:5.1}",
                if moving { "move" } else { "hold" },
                r.viewport.scale.to_f64() * width as f64 * ppp,
                est.map_or("-".to_string(), |e| format!("{e:7.1}")),
                m[0],
                m[1],
                m[2]
            );
            last_shown = Some((name, m));
        }
        if snaps.contains(&f) {
            save(r, &format!("{out}-{motion}-f{f}.png"))?;
        }
        now += dt;
        std::thread::sleep(std::time::Duration::from_secs_f64((dt - busy).max(0.0)));
    }
    r.finish_frames();
    if trace {
        println!(
            "live-trace: {switches} mode switches, {colour_jumps} frames whose mean colour jumped > 8 levels, {} segments: {}",
            segments.len(),
            segments
                .iter()
                .map(|(f, n)| format!("{f}:{n}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    gpu.sort_by(f64::total_cmp);
    if !gpu.is_empty() {
        println!(
            "live {motion}: {frames} frames at {rate} Hz, sample GPU ms p50 {:.2} p95 {:.2} max {:.2}",
            gpu[gpu.len() / 2],
            gpu[gpu.len() * 95 / 100],
            gpu[gpu.len() - 1]
        );
    }
    if let Some(b) = r.buddha_sampler()
        && let Some(l) = &b.live
    {
        println!(
            "live accumulation: {}x{} bins, hits/bin {:.1}, rho {:.3}, last frame proposals {:.3e}",
            l.width, l.height, l.hits_per_bin, l.rho, b.last_live.proposals as f64
        );
    }
    // And back to rest: time until the rest image takes over.
    let tr = Instant::now();
    loop {
        r.render_frame(now, width, height, ppp, &mut acquire);
        now += dt;
        std::thread::sleep(std::time::Duration::from_secs_f64(dt));
        if r.is_settled() || tr.elapsed().as_secs_f64() > 30.0 {
            break;
        }
    }
    println!("settled again after {:.2} s", tr.elapsed().as_secs_f64());
    // Memory (kb: platform/ios-device "Memory"): the look's buffers and the device's total.
    let look = r.buddha_sampler().map_or(0, |b| b.bytes());
    println!(
        "memory: Buddhabrot buffers {:.1} MB, GPU allocated in all {:.1} MB",
        look as f64 / 1e6,
        r.backend().allocated_bytes().unwrap_or(0) as f64 / 1e6
    );
    save(r, &format!("{out}-{motion}-settled.png"))?;
    Ok(0)
}

/// The resting view through the renderer's frame loop for `seconds` (a display link at 120 Hz
/// that stops when the renderer says the view is settled): GPU seconds per second of the
/// look's dispatches and frames, the progress readout, and when sampling stopped.
fn idle<B: GpuBackend + Clone>(
    r: &mut Renderer<B>,
    width: usize,
    height: usize,
    seconds: f64,
    ppp: f64,
    out: &str,
) -> Result<i32> {
    let backend = r.backend().clone();
    let target = backend.create_texture(width, height, TextureFormat::Bgra8Unorm, Storage::Shared)?;
    let mut acquire = |_: bool| -> Option<FrameTarget<B>> { Some((target.clone(), None)) };
    let dt = 1.0 / 120.0;
    let t0 = Instant::now();
    let mut now = 1.0;
    let mut last_print = 0.0;
    let mut gpu_at_print = 0.0;
    let mut settled_at: Option<f64> = None;
    let mut refreshes = 0usize;
    let mut presented = 0usize;
    r.frame_stats = Some(Default::default());
    let gpu_total = |r: &mut Renderer<B>| {
        let s = r.buddha_sampler().map_or(0.0, |b| b.harvested_gpu);
        s + r.frame_stats.as_ref().map_or(0.0, |f| f.colorize_gpu)
    };
    loop {
        let t = t0.elapsed().as_secs_f64();
        if t >= seconds {
            break;
        }
        let o = r.render_frame(now, width, height, ppp, &mut acquire);
        refreshes += 1;
        presented += usize::from(o.presented);
        if !o.animating && settled_at.is_none() {
            settled_at = Some(t);
        }
        if o.animating {
            settled_at = None;
        }
        if t - last_print >= 1.0 || (settled_at == Some(t)) {
            let g = gpu_total(r);
            let p = r.orbit_progress(now);
            println!(
                "t {t:6.2} GPU {:5.1} % of the last {:.2} s, refreshes {refreshes}, presented {presented}, {}{}",
                100.0 * (g - gpu_at_print) / (t - last_print).max(1e-3),
                t - last_print,
                p.map_or("no progress".to_string(), |p| format!(
                    "hits/px {:.0} of {:.0}, noise {} levels, progress {:.0} %, {:.2e} samples, {}",
                    p.mean_hits,
                    p.target,
                    p.noise.map_or("-".to_string(), |n| format!("{n:.2}")),
                    100.0 * p.progress,
                    p.samples,
                    if p.metropolis { "MH" } else { "uniform" }
                )),
                if settled_at.is_some() { ", settled" } else { "" }
            );
            last_print = t;
            gpu_at_print = g;
        }
        if !o.animating {
            // The display link stopped: nothing runs until the next input.
            std::thread::sleep(std::time::Duration::from_secs_f64(0.1));
            now += 0.1;
            continue;
        }
        now += dt;
        std::thread::sleep(std::time::Duration::from_secs_f64(dt));
    }
    r.finish_frames();
    let g = gpu_total(r);
    println!(
        "idle-metrics: {seconds:.0} s, GPU {:.1} % average, settled at {}",
        100.0 * g / seconds,
        settled_at.map_or("never".to_string(), |s| format!("{s:.2} s"))
    );
    let mut px = vec![0u8; width * height * 4];
    backend.read_texture(&target, width, height, TextureFormat::Bgra8Unorm, &mut px)?;
    write_png_bgra(&format!("{out}-idle.png"), &px, width, height)?;
    println!("  {out}-idle.png");
    Ok(0)
}
