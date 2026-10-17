//! `autopilot`: the autopilot with no input, headless and on the CPU only (kb:
//! decisions/controls "Autopilot"). The app's `Navigator` steps at a fixed refresh rate; the
//! target lock searches on the calling thread, so a run is reproducible. The GPU survey is
//! replaced by a CPU one every `--every` seconds: an 80 × 80 probe of the square across the
//! view's shorter side, counted into 16 × 16 cells like the survey kernel counts (interior
//! texels, neighbour pairs whose class or integer escape count differ), so safety, hints and
//! rescue run as in the app, and the reported "uniform" share is the HUD's condition
//! (danger > 0.5 around the heading). `fly` measures the real pipeline.
//!
//! ```text
//! almondbread autopilot --center re,im --scale s [--size 1280x800] [--speed 0.9] [--seconds 90]
//!                       [--hz 60] [--every 0.25] [--quiet 1]
//! ```

use almondbread_core::autopilot::{Survey, SurveyCell};
use almondbread_core::input::{InputEvent, NavKey, Navigator};
use almondbread_core::journey::{self, probe_limit};
use almondbread_core::viewport::{Center, Scale, Viewport};

use crate::options::{CommandError, Options, Result, err};

/// Probe texels across the surveyed square, and survey cells across.
const TEXELS: usize = 80;
const CELLS: usize = 16;

/// A survey of the square across the view's shorter side (`side` points), from a CPU probe
/// at `*limit` iterations or the depth's probe limit, whichever is larger. Like the app's auto
/// iterations, the limit rises (4x, up to the escalated probe's cap) while most of the probe
/// never escapes and a higher limit lets more escape: at sea-8007 the CPU cap of 30 000 left
/// the whole survey unresolved, read as interior (the GPU survey sees ~118 000 iterations).
fn cpu_survey(v: &Viewport, side: f64, time: f64, limit: &mut usize) -> Survey {
    let span = v.scale * side;
    let unresolved = |f: &almondbread_core::oracle::IterationField| {
        f.n.iter().filter(|&&n| n < 0.0).count() as f64 / f.n.len().max(1) as f64
    };
    let mut it = (*limit).max(journey::probe_iterations(span));
    let mut f = probe_limit(&v.center, span, it, TEXELS);
    let mut share = unresolved(&f);
    while share > 0.5 && it < journey::PROBE_ESCALATE_CAP {
        let next = (4 * it).min(journey::PROBE_ESCALATE_CAP);
        let g = probe_limit(&v.center, span, next, TEXELS);
        let gs = unresolved(&g);
        if gs > share - 0.02 {
            break;
        }
        (f, it, share) = (g, next, gs);
        *limit = it;
    }
    let cell = TEXELS / CELLS;
    let cls = |x: usize, y: usize| {
        let n = f.n[y * TEXELS + x];
        if n < 0.0 { -1 } else { n.floor() as i64 }
    };
    let mut cells = vec![SurveyCell::default(); CELLS * CELLS];
    for y in 0..TEXELS {
        for x in 0..TEXELS {
            let c = &mut cells[(y / cell) * CELLS + x / cell];
            c.samples += 1;
            let a = cls(x, y);
            c.interior += u32::from(a < 0);
            // The escaped texel with the highest count (survey steering aims there).
            let n = f.n[y * TEXELS + x] as f64;
            if a >= 0 && (c.samples == c.interior + 1 || n > c.peak_n) {
                c.peak_n = n;
                c.peak = (
                    ((x as f64 + 0.5) / TEXELS as f64 - 0.5) * side,
                    (0.5 - (y as f64 + 0.5) / TEXELS as f64) * side,
                );
            }
            for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                if nx < TEXELS && ny < TEXELS {
                    c.pairs += 1;
                    c.edges += u32::from(cls(nx, ny) != a);
                }
            }
        }
    }
    Survey {
        cols: CELLS,
        rows: CELLS,
        cells,
        view: (side, side),
        texels_across: TEXELS as f64,
        texels_down: TEXELS as f64,
        time,
        viewport: Some(v.clone()),
    }
}

pub fn autopilot_command(o: &Options) -> Result<i32> {
    let Some((re, im)) = o.get("center").and_then(|c| c.split_once(',')) else {
        return err("autopilot needs --center re,im --scale s");
    };
    let center = Center::parse(re, im).ok_or_else(|| CommandError("bad --center".into()))?;
    let scale = Scale::parse(o.get("scale").unwrap_or("0.003")).ok_or_else(|| CommandError("bad --scale".into()))?;
    let (w, h) = o.size("size", (1280, 800))?;
    let (w, h) = (w as f64, h as f64);
    let hz = o.f64("hz").unwrap_or(60.0);
    let seconds = o.f64("seconds").unwrap_or(90.0);
    let every = o.f64("every").unwrap_or(0.25);
    let quiet = o.get("quiet") == Some("1");
    let mut v = Viewport::with_center(center, scale);
    v.ensure_precision();
    let mut n = Navigator::default();
    n.handle(InputEvent::Resize { width: w, height: h }, &mut v, 0.0);
    n.autopilot_mut().lock_mut().synchronous = true;
    n.handle(InputEvent::Pointer(None), &mut v, 0.0);
    n.advance(&mut v, 0.0);
    n.handle(InputEvent::KeyDown(NavKey::Autopilot), &mut v, 0.0);
    n.handle(InputEvent::KeyUp(NavKey::Autopilot), &mut v, 0.0);
    if let Some(s) = o.f64("speed") {
        n.autopilot_mut().set_speed(s, 0.0);
    }
    let side = w.min(h);
    let zoom0 = v.zoom_log10();
    let steps = (seconds * hz).round() as usize;
    let every_k = ((every * hz).round() as usize).max(1);
    let (mut sampled, mut uniform, mut sampled_late, mut uniform_late) = (0usize, 0usize, 0usize, 0usize);
    let (mut run, mut longest, mut rescue) = (0.0f64, 0.0f64, 0.0f64);
    let mut max_zoom = zoom0;
    let t0 = std::time::Instant::now();
    let mut limit = 0;
    n.autopilot_mut().set_survey(cpu_survey(&v, side, 0.0, &mut limit));
    for k in 1..=steps {
        let t = k as f64 / hz;
        n.advance(&mut v, t);
        if n.autopilot().is_rescuing() {
            rescue += 1.0 / hz;
        }
        max_zoom = max_zoom.max(v.zoom_log10());
        if k % every_k != 0 {
            continue;
        }
        n.autopilot_mut().set_survey(cpu_survey(&v, side, t, &mut limit));
        let a = n.autopilot();
        let Some(assessment) = a.assessment(t) else { continue };
        let u = assessment.danger() > 0.5;
        sampled += 1;
        uniform += usize::from(u);
        if t >= 5.0 {
            sampled_late += 1;
            uniform_late += usize::from(u);
        }
        run = if u { run + every } else { 0.0 };
        longest = longest.max(run);
        if !quiet {
            let lock = a.lock();
            let target = lock.target().map_or(String::from("-"), |lt| {
                let (x, y) = v.offset_of(&lt.center);
                format!(
                    "{} left {:.1} e-folds at ({x:.0},{y:.0})",
                    lt.label,
                    (v.scale * side).ratio(lt.end_span).ln()
                )
            });
            println!(
                "{t:6.2}s zoom 1e{:<7.2} rate {:>5.2} detail {:>6.1} interior {:.2} {:?}{} | {target}",
                v.zoom_log10(),
                a.rate(),
                assessment.detail,
                assessment.interior,
                a.hint(),
                if u { " UNIFORM" } else { "" },
            );
        }
    }
    let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
    println!(
        "autopilot-sim: lock={} speed={:.2} uniform={:.1}% uniform_after5s={:.1}% longest_uniform={:.1}s rescue={:.1}s targets={} zoom 1e{:.1} -> 1e{:.1} (max 1e{:.1}) cpu {:.1} s",
        u8::from(n.autopilot().lock_enabled()),
        n.autopilot().speed(),
        pct(uniform, sampled),
        pct(uniform_late, sampled_late),
        longest,
        rescue,
        n.autopilot().lock().switches,
        zoom0,
        v.zoom_log10(),
        max_zoom,
        t0.elapsed().as_secs_f64(),
    );
    Ok(0)
}
