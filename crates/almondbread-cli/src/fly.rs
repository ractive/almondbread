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
//!                 [--bitrate 40M] [--log x.csv] [--lock 0|1] [--pi-flight neck|cusp]
//! ```
//!
//! With the autopilot on it reports how often the heading region was uniform (the survey's
//! "slower: featureless / interior" condition, danger > 0.5), the longest such stretch, the
//! rescue time, lock targets taken and the deepest zoom (kb: decisions/controls "Autopilot").
//! `--lock 0` turns the autopilot's target lock off (the pure joystick), `--lock 1` on.
//!
//! Script format and events: [`almondbread_core::flight`] (`jump` added for the flight
//! benchmark, kb: synthesis/flight-benchmark).

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use almondbread_core::flight::{FlightPlayer, FlightScript, apply_json_settings, start_viewport};
use almondbread_core::image::write_png_bgra;
use almondbread_core::input::InputEvent;
use almondbread_render::{FrameStats, GpuBackend, Renderer, Storage, TextureFormat};

use crate::options::{Options, Result, err};

/// Loads a flight script.
pub fn load_script(path: &str) -> Result<FlightScript> {
    let text = fs::read_to_string(path).map_err(|e| crate::options::CommandError(format!("{path}: {e}")))?;
    FlightScript::parse(&text).or_else(|e| err(format!("{path}: {e}")))
}

/// Frames handed to the encoder.
enum Sink {
    Ffmpeg(std::process::Child),
    Pngs(String),
    /// `--out null`: measurements only (no frame readback, no encoding).
    Null,
}

fn open_sink(out: &str, size: (usize, usize), fps: f64, o: &Options) -> Result<Sink> {
    if out == "null" {
        return Ok(Sink::Null);
    }
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
                Sink::Null => {}
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
    let script = load_script(path)?;
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
    match o.get("lock") {
        Some("0") => r.navigator.autopilot_mut().set_lock(false),
        Some("1") => r.navigator.autopilot_mut().set_lock(true),
        _ => {}
    }
    // One view point per pixel: script positions and speeds are in video pixels.
    let (w_pts, h_pts) = (size.0 as f64, size.1 as f64);
    r.handle_input(
        InputEvent::Resize {
            width: w_pts,
            height: h_pts,
        },
        0.0,
    );
    r.viewport = start_viewport(script.start.as_ref(), w_pts, h_pts, 1.0).map_err(crate::options::CommandError)?;
    // `--pi-flight neck|cusp`: the π meter's guided flight (kb: decisions/controls "π
    // meter") started at the first refresh, with the overlay drawn as in the app (the place's
    // look, the probe, the panels, the orbit trail).
    let pi_flight = match o.get("pi-flight") {
        Some(name) => Some(
            almondbread_core::pi_moment::PiPoint::named(name)
                .ok_or_else(|| crate::options::CommandError(format!("--pi-flight: neck or cusp, not {name}")))?,
        ),
        None => None,
    };
    let mut overlay: Option<almondbread_render::overlay::Overlay<B>> = None;
    if let Some(p) = pi_flight {
        if let Some(place) = almondbread_core::places::place_named(p.place_id())
            && let Some(look) = almondbread_core::palette::Look::named(place.look)
        {
            look.apply(&mut r.settings);
        }
        overlay = Some(Default::default());
    }
    // (One view point per pixel, as for the scripts.)
    let ppp = 1.0;
    let refreshes = (duration * hz).round() as usize;
    let interval = 1.0 / hz;
    let null = out == "null";
    let (tx, writer) = spawn_writer(open_sink(&out, size, fps, o)?, size);
    let mut log = String::from(
        "refresh,t,presented,preview,render_w,render_h,zoom_log10,late_ms,frame_cpu_ms,ap_rate,ap_throttle,ap_safety,ap_hint,pointer_x,pointer_y,fly_progress,ap_detail,ap_uniform,ap_rescue,lock\n",
    );
    let mut hints: std::collections::BTreeMap<String, usize> = Default::default();
    let mut ap_frames = 0usize;
    let mut apm = ApMetrics::default();
    let mut player = FlightPlayer::new(1.0);
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
            // The previous refresh's time exactly: `t − interval` rounds differently, and an event
            // on a refresh boundary then fired twice.
            let t_prev = if k == 0 { -interval } else { (k - 1) as f64 * interval };
            player.feed(&mut r, &script.events, t_prev, t, now);
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
            if null {
                queue.pop_front();
                continue;
            }
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
        // The previous refresh's time exactly: `t − interval` rounds differently, and an event
        // on a refresh boundary then fired twice.
        let t_prev = if k == 0 { -interval } else { (k - 1) as f64 * interval };
        player.feed(&mut r, &script.events, t_prev, t, now);
        // The slot this refresh may draw into must not hold an image still waiting.
        if queue
            .iter()
            .any(|q| q.2 == next_slot && q.1 == slot_serial[next_slot] && q.1 != last_image.0)
        {
            flush(&r, &mut queue, &mut last_image, true)?;
        }
        let target = ring[next_slot].clone();
        let cpu0 = r.clock();
        if let Some(ov) = &mut overlay {
            if k == 0
                && let Some(p) = pi_flight
            {
                ov.start_pi_flight(&mut r, p, w_pts.min(h_pts), now);
            }
            ov.prepare(&mut r, size, ppp, now);
        }
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
        let ap = r.navigator.autopilot();
        let assessment = ap.assessment(now);
        let uniform = assessment.map(|a| a.danger() > 0.5);
        if ap.is_active() {
            ap_frames += 1;
            *hints.entry(format!("{:?}", ap.hint())).or_default() += 1;
            apm.frame(
                t,
                interval,
                uniform,
                ap.is_rescuing(),
                r.viewport.zoom_log10(),
                ap.lock().switches,
            );
        }
        log.push_str(&format!(
            "{k},{t:.4},{},{},{},{},{:.4},{:.2},{:.3},{:.4},{:.3},{:.3},{:?},{:.1},{:.1},{:.4},{:.2},{},{},\"{}\"\n",
            u8::from(outcome.presented),
            u8::from(st.is_preview),
            st.render_size.0,
            st.render_size.1,
            r.viewport.zoom_log10(),
            (-wait).max(0.0) * 1000.0,
            frame_cpu * 1000.0,
            ap.rate(),
            ap.throttle(),
            ap.safety(),
            ap.hint(),
            player.pointer.0,
            player.pointer.1,
            r.navigator.flight().map_or(-1.0, |f| f.progress()),
            assessment.map_or(f64::NAN, |a| a.detail),
            uniform.map_or(-1, i32::from),
            u8::from(ap.is_rescuing()),
            ap.lock_label(),
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
    {
        // Gesture work and the motion sample cache (kb: techniques/sample-reuse).
        let pf = fs.preview_frames.max(1) as f64;
        let c = r.sample_cache_stats();
        println!(
            "  gesture frames: {:.3e} iterations/frame, iterate GPU {:.2} ms/frame; all chunks {:.3e} iterations",
            fs.preview_iterations / pf,
            fs.preview_chunk_gpu * 1000.0 / pf,
            fs.iterations,
        );
        if c.frames + c.fallback_frames > 0 {
            let f = c.frames.max(1) as f64;
            println!(
                "  sample cache: {} frames ({} fresh), reused {:.1} % of visible texels, computed {:.0} texels/frame, CPU plan {:.3} ms/frame (max {:.3}) + shade encode {:.3}, GPU shade {:.3} ms/frame, maintenance {:.3} ms/frame ({} passes), epochs {} (seeded {}), finer {} coarser {}",
                c.frames,
                c.fallback_frames,
                100.0 * c.reused_texels as f64 / c.visible_texels.max(1) as f64,
                c.texels_planned as f64 / f,
                c.cpu_plan * 1000.0 / f,
                c.cpu_plan_max * 1000.0,
                c.cpu_shade * 1000.0 / f,
                c.gpu_shade * 1000.0 / c.gpu_shades.max(1) as f64,
                c.gpu_maintain * 1000.0 / f,
                c.gpu_passes,
                c.epochs,
                c.seeds,
                c.finer,
                c.coarser,
            );
        }
    }
    if ap_frames > 0 {
        let share: Vec<String> = hints
            .iter()
            .map(|(h, n)| format!("{h} {:.0} %", 100.0 * *n as f64 / ap_frames as f64))
            .collect();
        println!(
            "  autopilot: {:.1} s on, hints {}; final scale {} per point (zoom {})",
            ap_frames as f64 / hz,
            share.join(", "),
            r.viewport.scale.to_text(4),
            almondbread_render::host::zoom_text(r.viewport.zoom_log10()),
        );
        println!("  {}", apm.summary(r.navigator.autopilot().lock_enabled()));
    }
    Ok(0)
}

/// What a headless autopilot run measures (kb: decisions/controls "Autopilot").
#[derive(Default)]
struct ApMetrics {
    /// Frames with a fresh survey, uniform ones, and both after the first 5 s.
    surveyed: usize,
    uniform: usize,
    surveyed_late: usize,
    uniform_late: usize,
    /// Current and longest uniform stretch (s).
    run: f64,
    longest: f64,
    rescue: f64,
    max_zoom: f64,
    final_zoom: f64,
    switches: u32,
}

impl ApMetrics {
    fn frame(&mut self, t: f64, dt: f64, uniform: Option<bool>, rescue: bool, zoom: f64, switches: u32) {
        if let Some(u) = uniform {
            self.surveyed += 1;
            self.uniform += usize::from(u);
            if t >= 5.0 {
                self.surveyed_late += 1;
                self.uniform_late += usize::from(u);
            }
            self.run = if u { self.run + dt } else { 0.0 };
            self.longest = self.longest.max(self.run);
        }
        if rescue {
            self.rescue += dt;
        }
        self.max_zoom = self.max_zoom.max(zoom);
        self.final_zoom = zoom;
        self.switches = switches;
    }

    fn summary(&self, lock: bool) -> String {
        let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
        format!(
            "autopilot-metrics: lock={} uniform={:.1}% uniform_after5s={:.1}% longest_uniform={:.1}s rescue={:.1}s targets={} max_zoom=1e{:.1} final_zoom=1e{:.1} surveyed_frames={}",
            u8::from(lock),
            pct(self.uniform, self.surveyed),
            pct(self.uniform_late, self.surveyed_late),
            self.longest,
            self.rescue,
            self.switches,
            self.max_zoom,
            self.final_zoom,
            self.surveyed,
        )
    }
}
