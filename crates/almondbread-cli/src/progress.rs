//! `almondbread progress`: a jump to a view through the real per-refresh scheduler (headless,
//! as `verify --history jump`), printing the render progress the app shows (kb:
//! decisions/controls "Render progress") against the time the view really took: the fraction
//! the line draws next to t / T (T: when the view settled), the HUD text, and the scheduler's
//! counters behind them.

use std::time::Duration;

use almondbread_render::progress::Stage;
use almondbread_render::{GpuBackend, Renderer, TextureFormat};

use crate::options::{Options, Result, err};

/// `progress --location L | --center re,im --scale s [--iterations N] [--size WxH] [--ppp p]
/// [--every s] [--timeout s] [--settings plain|app]`.
pub fn progress_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    let (width, height) = o.size("size", (2560, 1600))?;
    let ppp = o.f64("ppp").unwrap_or(0.5);
    let every = o.f64("every").unwrap_or(0.25);
    let timeout = o.f64("timeout").unwrap_or(300.0);
    let mut view = o.viewport(width, height)?;
    view.scale = view.scale / ppp;
    let mut r = Renderer::new(backend.clone())?;
    let app_settings = r.settings.clone();
    o.apply_settings(&mut r.settings);
    o.apply_renderer(&mut r);
    if o.get("settings") != Some("plain") {
        // The app's look and progressive anti-aliasing, with the asked iteration settings.
        let s = r.settings.clone();
        r.settings = app_settings;
        r.settings.auto_iterations = s.auto_iterations;
        r.settings.manual_iterations = s.manual_iterations;
        r.settings.max_iterations_cap = s.max_iterations_cap;
        r.settings.interior_detection = s.interior_detection;
    }
    // `--zoom-seconds s`: first hold a zoom into the view for s seconds at `--speed` e-folds
    // per second (a gesture, as `verify --history zoom`); the session then counts from its end.
    let zoom_seconds = o.f64("zoom-seconds").unwrap_or(0.0);
    let speed = o.f64("speed").unwrap_or(1.6);
    if zoom_seconds > 0.0 {
        view.zoom_by((speed * zoom_seconds).exp(), 0.0, 0.0);
    }
    r.viewport = view;
    let target = backend.create_texture(
        width,
        height,
        TextureFormat::Bgra8Unorm,
        almondbread_render::Storage::Shared,
    )?;
    let interval = 1.0 / 120.0;
    let mut tick = r.clock();
    if zoom_seconds > 0.0 {
        let begin = r.clock();
        let mut prev = begin;
        let mut shown = false;
        // (Bounded: one refresh per 1/120 s at most.)
        for _ in 0..(zoom_seconds * 240.0).ceil() as usize {
            if r.clock() - begin >= zoom_seconds {
                break;
            }
            let now = r.clock();
            r.navigator.note_interaction(now);
            r.viewport.zoom_by((-speed * (now - prev)).exp(), 0.0, 0.0);
            prev = now;
            let t = target.clone();
            r.render_frame(now, width, height, ppp, &mut |_| Some((t.clone(), None)));
            shown |= r.progress.current().alpha > 0.0;
            tick += interval;
            let left = tick - r.clock();
            if left > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(left));
            }
        }
        println!("zoom gesture {zoom_seconds:.1} s: line shown during it: {shown}");
    }
    let start = r.clock();
    let mut next_print = 0.0;
    let mut last_stage = None;
    let mut first_shown = None;
    let mut rows: Vec<(f64, f64, Option<f64>)> = Vec::new();
    println!(
        "progress: {}x{} px, ppp {ppp}, auto iterations {}, limit {}",
        width,
        height,
        r.settings.auto_iterations,
        r.max_iterations()
    );
    let settled_at = loop {
        let now = r.clock();
        let t = target.clone();
        r.render_frame(now, width, height, ppp, &mut |_| Some((t.clone(), None)));
        let elapsed = now - start;
        let s = r.progress_sample(now);
        let p = r.progress.current();
        if p.alpha > 0.0 && first_shown.is_none() {
            first_shown = Some(elapsed);
        }
        if elapsed >= next_print || last_stage != Some(s.stage) {
            next_print = elapsed + every;
            last_stage = Some(s.stage);
            rows.push((elapsed, p.fraction, p.eta));
            println!(
                "t {elapsed:7.2}  line {:5.1}% a{:.2} rem {:>7} eta {:>7}  | {:<13} adm {:>8}/{:<8} live {:>8} minN {:>10} / {:<10} unres {:>8} ext {} aa {}/{} gpu {:.2}{}  | hud: {}",
                100.0 * p.fraction,
                p.alpha,
                r.progress.remaining.map_or("-".into(), |e| format!("{e:.1}")),
                p.eta.map_or("-".into(), |e| format!("{e:.1}")),
                s.stage.name(),
                s.level_admitted,
                s.level_total,
                s.level_live,
                s.live_min_n.map_or("-".into(), |n| n.to_string()),
                s.level_limit,
                s.unresolved.map_or("-".into(), |n| n.to_string()),
                s.extension_passes,
                s.aa_composed,
                s.aa_target,
                s.gpu_time,
                s.reference
                    .map_or(String::new(), |(secs, n, m)| format!(" ref {secs:.1}s {n}/{m}")),
                p.text(),
            );
        }
        if s.settled && s.stage == Stage::Idle {
            break elapsed;
        }
        if elapsed > timeout {
            return err("the view did not settle in time");
        }
        tick += interval;
        let left = tick - r.clock();
        if left > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(left));
        }
    };
    println!(
        "settled after {settled_at:.2} s; line shown from {}",
        first_shown.map_or("never".into(), |t| format!("{t:.2} s"))
    );
    // Honesty: the line's fraction against the share of the real time.
    println!("   t/T   line   eta  true-left");
    for (t, f, eta) in rows {
        println!(
            "{:5.1}% {:5.1}% {:>6} {:6.1}",
            100.0 * t / settled_at,
            100.0 * f,
            eta.map_or("-".into(), |e| format!("{e:.1}")),
            settled_at - t
        );
    }
    Ok(0)
}
