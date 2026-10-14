//! The headless commands, generic over the GPU backend. Output lines are those of the former
//! Swift tools (`AlmondbreadMac --render / --verify / --bench`, deleted in R2b), so results
//! diff 1:1 with older records.

use std::fs;
use std::time::Instant;

use almondbread_core::cfmt::g;
use almondbread_core::image::write_png_bgra;
use almondbread_core::input::PAN_SPEED;
use almondbread_core::locations::BENCHMARK_LOCATIONS;
use almondbread_core::oracle::{IterationField, VerifyReport, compare, cpu_render};
use almondbread_core::settings::Supersampling;
use almondbread_core::viewport::Viewport;
use almondbread_render::scheduler::BAILOUT;
use almondbread_render::{GpuBackend, OffscreenResult, Renderer, SimulationResult};

use crate::options::{Options, Result, err};

fn new_renderer<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<Renderer<B>> {
    let mut r = Renderer::new(backend.clone())?;
    o.apply_settings(&mut r.settings);
    o.apply_renderer(&mut r);
    Ok(r)
}

/// Warm-up renders discarded first, so timings exclude pipeline and allocation warm-up.
fn render<B: GpuBackend>(
    r: &mut Renderer<B>,
    width: usize,
    height: usize,
    warmup: usize,
    readback: bool,
) -> Result<OffscreenResult> {
    for _ in 0..warmup {
        r.render_offscreen(width, height, false)?;
    }
    Ok(r.render_offscreen(width, height, readback)?)
}

fn suffixed(path: &str, suffix: &str) -> String {
    path.replace(".png", &format!("{suffix}.png"))
}

// MARK: render

/// `render out.png`: PNG output through the same GPU passes as the app.
pub fn render_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    let Some(path) = o.positional.first().cloned().or_else(|| o.get("out").map(String::from)) else {
        return err("missing output path");
    };
    let (width, height) = o.size("size", (1600, 1000))?;
    let mut r = new_renderer(backend, o)?;
    r.viewport = o.viewport(width, height)?;
    let repeats = o.usize("repeat").unwrap_or(1).max(1);
    let mut times = Vec::new();
    let mut walls = Vec::new();
    let mut out = None;
    for i in 0..repeats {
        let res = render(&mut r, width, height, usize::from(i == 0), false)?;
        times.push(res.iterate_gpu_time);
        walls.push(res.wall_time);
        out = Some(res);
    }
    let out = out.expect("rendered");
    write_png_bgra(&path, &out.pixels, width, height)?;
    let (ref_time, ref_len) = r.reference_info();
    let ms: Vec<String> = times.iter().map(|t| format!("{:.2}", t * 1000.0)).collect();
    let wall: Vec<String> = walls.iter().map(|t| format!("{:.1}", t * 1000.0)).collect();
    println!(
        "{path}: {width}x{height}  zoom {}  maxIter {}  ref {} pts {:.2} ms  iterate GPU [{}] ms  wall [{}] ms  chunks {}  iters {}  steps {}  colorize {:.2} ms",
        g(r.viewport.zoom(), 3),
        r.max_iterations(),
        ref_len,
        ref_time * 1000.0,
        ms.join(", "),
        wall.join(", "),
        out.chunks,
        g(out.iterations_done as f64, 3),
        g(out.steps_done as f64, 3),
        out.colorize_gpu_time * 1000.0
    );
    if let Some(f) = out.bla_forecast {
        let (t, entries, bytes) = r.bla_info();
        println!(
            "BLA {}: {entries} entries ({} KB) built in {:.3} ms, eps 2^{:.1}; forecast {} iterations in {} blocks ({}); {:.2}x fewer steps than iterations",
            if out.bla_used { "used" } else { "not used" },
            bytes / 1024,
            t * 1000.0,
            r.config.bla_epsilon.log2(),
            f.covered,
            f.blocks,
            if f.pays() { "pays" } else { "does not pay" },
            out.iterations_done as f64 / out.steps_done.max(1) as f64
        );
    }
    Ok(0)
}

// MARK: verify

pub struct VerifyResult {
    pub report: VerifyReport,
    pub heatmap: Vec<u8>,
    pub image: Vec<u8>,
    pub gpu: IterationField,
    pub gpu_time: f64,
    pub cpu_time: f64,
    pub chunks: usize,
}

/// GPU render with readback, CPU render of the same grid, diff.
pub fn verify<B: GpuBackend>(
    r: &mut Renderer<B>,
    width: usize,
    height: usize,
    cpu_iterations: Option<usize>,
) -> Result<VerifyResult> {
    let res = render(r, width, height, 0, true)?;
    let gpu = IterationField::from_texels(width, height, res.iterations.as_deref().unwrap_or(&[]));
    let v = r.viewport.clone();
    let t0 = Instant::now();
    let cpu_iter = cpu_iterations.unwrap_or_else(|| r.max_iterations());
    let cpu = cpu_render(v.center.re(), v.center.im(), v.scale, width, height, cpu_iter, BAILOUT);
    let cpu_time = t0.elapsed().as_secs_f64();
    let (report, heatmap) = compare(&gpu, &cpu);
    Ok(VerifyResult {
        report,
        heatmap,
        image: res.pixels,
        gpu,
        gpu_time: res.iterate_gpu_time,
        cpu_time,
        chunks: res.chunks,
    })
}

/// `verify`: a GPU render against the CPU f64 oracle; `--fields a.iter --against b.iter`
/// diffs two dumps (b plays the reference); `--history zoom|out|pan` runs a scripted session.
pub fn verify_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    if let (Some(a), Some(b)) = (o.get("fields"), o.get("against")) {
        let fa = IterationField::read(a)?;
        let fb = IterationField::read(b)?;
        if fa.width != fb.width || fa.height != fb.height {
            return err("size mismatch");
        }
        let (report, heatmap) = compare(&fa, &fb);
        println!("{}", report.summary());
        let differ = fa.bitwise_differences(&fb);
        println!(
            "bitwise: {differ} of {} texels differ ({})",
            fa.n.len(),
            if differ == 0 { "identical" } else { "NOT identical" }
        );
        if let Some(path) = o.get("heatmap") {
            write_png_bgra(path, &heatmap, fa.width, fa.height)?;
        }
        return Ok(0);
    }
    if o.has("history") {
        return history(backend, o);
    }
    let (width, height) = o.size("size", (400, 260))?;
    let mut r = new_renderer(backend, o)?;
    r.viewport = o.viewport(width, height)?;
    if let Some((dx, dy)) = o.pair("ref-offset")? {
        // Reference reuse: the orbit of a point dx,dy pixels from the centre, made at
        // `--ref-scale` times the scale (default 1).
        let mut rv = r.viewport.clone();
        rv.center = rv.center.offset(dx * rv.scale, dy * rv.scale);
        rv.scale *= o.f64("ref-scale").unwrap_or(1.0);
        let max_iter = r.max_iterations();
        r.prime_reference(&rv, max_iter);
    }
    let v = verify(&mut r, width, height, o.usize("cpu-iterations"))?;
    println!("{}", v.report.summary());
    println!(
        "GPU {:.2} ms ({} chunks), CPU {:.0} ms",
        v.gpu_time * 1000.0,
        v.chunks,
        v.cpu_time * 1000.0
    );
    if let Some(path) = o.get("heatmap") {
        write_png_bgra(path, &v.heatmap, width, height)?;
    }
    if let Some(path) = o.get("out") {
        write_png_bgra(path, &v.image, width, height)?;
    }
    if let Some(path) = o.get("dump") {
        v.gpu.write(path)?;
    }
    Ok(0)
}

/// `verify --history zoom|out|pan`: a scripted session through the interactive scheduler
/// (headless, the code the app runs per refresh). Holding Space: starts exp(speed x seconds)
/// zoomed out from the view and zooms in around `--pointer x,y` (points from the centre,
/// y up) at `--speed` e-folds/s with auto iterations, at `--ppp` view points per pixel. After
/// the refined final frame, diffs it against the CPU oracle at the hard limit and a fresh
/// headless render. Fails on a preview final frame, more than `--max-false-interior` %
/// (0.05) exterior painted interior, or more than `--max-mismatch` % (0.05) differing from
/// the fresh render. `--then f` adds a one-step zoom of factor f.
pub fn history<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    let (width, height) = o.size("size", (1280, 820))?;
    let ppp = o.f64("ppp").unwrap_or(0.5);
    let speed = o.f64("speed").unwrap_or(1.6);
    let seconds = o.f64("seconds").unwrap_or(2.0);
    let max_mismatch = o.f64("max-mismatch").unwrap_or(0.05);
    let max_false_interior = o.f64("max-false-interior").unwrap_or(0.05);
    let pointer = o.pair("pointer")?.unwrap_or((0.0, 0.0));
    // --scale is per pixel as for verify; the viewport counts view points.
    let mut end = o.viewport(width, height)?;
    end.scale /= ppp;
    let mut r = new_renderer(backend, o)?;
    // Deterministic for the diff: let the extension run to its limit.
    r.config.extension_budget = o.f64("extension-budget").unwrap_or(120.0);
    let mode = o.get("history").unwrap_or("zoom").to_string();
    let pan_length = (pointer.0 * pointer.0 + pointer.1 * pointer.1).sqrt();
    let pan = if pan_length > 0.0 {
        (pointer.0 / pan_length, pointer.1 / pan_length)
    } else {
        (1.0, 0.0)
    };
    let pan_speed = PAN_SPEED * width as f64 * ppp;
    let mut start = end.clone();
    match mode.as_str() {
        "zoom" => start.zoom_by((-speed * seconds).exp(), pointer.0, pointer.1),
        "out" => start.zoom_by((speed * seconds).exp(), pointer.0, pointer.1),
        "pan" => start.pan_by_points(pan.0 * pan_speed * seconds, pan.1 * pan_speed * seconds),
        _ => return err(format!("unknown --history {mode} (zoom, out, pan)")),
    }
    r.viewport = start;
    // Settle on the start view first (the session begins from a refined frame).
    let _ = r.simulate(width, height, ppp, 120.0, 0, 30.0, &mut |_, _, _| {});
    let frames = (seconds * 120.0) as usize;
    let mut sessions: Vec<SimulationResult> = Vec::new();
    let m = mode.clone();
    let first = r
        .simulate(
            width,
            height,
            ppp,
            120.0,
            frames,
            30.0,
            &mut |_, dt, v: &mut Viewport| match m.as_str() {
                "zoom" => v.zoom_by((speed * dt).exp(), pointer.0, pointer.1),
                "out" => v.zoom_by((-speed * dt).exp(), pointer.0, pointer.1),
                _ => v.pan_by_points(-pan.0 * pan_speed * dt, -pan.1 * pan_speed * dt),
            },
        )
        .map_err(|_| crate::options::CommandError("simulated session did not finish".into()))?;
    sessions.push(first);
    if let Some(f) = o.f64("then") {
        let second = r
            .simulate(width, height, ppp, 120.0, 1, 30.0, &mut |_, _, v: &mut Viewport| {
                v.zoom_by(f, pointer.0, pointer.1)
            })
            .map_err(|_| crate::options::CommandError("second session did not finish".into()))?;
        sessions.push(second);
    }
    let mut failed: Vec<String> = Vec::new();
    let n = sessions.len();
    for (i, sim) in sessions.iter().enumerate() {
        let v = &sim.viewport;
        let tag = if n > 1 { format!("[{}] ", i + 1) } else { String::new() };
        println!(
            "{tag}history {mode}: {} frames, final view {}, {} texel {} (zoom {}), maxIter {} (hard {}), ref {} pts, level {}x{}{}, complete {:.0} ms / resolved {:.0} ms after input, unresolved {}, learned limit {}",
            sim.frames,
            g(v.center.re(), 17),
            g(v.center.im(), 17),
            g(sim.texel_scale, 4),
            g(v.zoom(), 3),
            sim.max_iter,
            sim.hard_iter,
            sim.reference_length,
            sim.level_width,
            sim.level_height,
            if sim.preview { " (PREVIEW)" } else { "" },
            sim.final_after * 1000.0,
            sim.done_after * 1000.0,
            sim.unresolved,
            r.settings.learned_iterations
        );
        let field = IterationField::from_texels(sim.level_width, sim.level_height, &sim.iterations);
        let cpu_iter = o.usize("cpu-iterations").unwrap_or(sim.hard_iter);
        let cpu = cpu_render(
            v.center.re(),
            v.center.im(),
            sim.texel_scale,
            sim.level_width,
            sim.level_height,
            cpu_iter,
            BAILOUT,
        );
        let (vs_cpu, heatmap) = compare(&field, &cpu);
        println!("{tag}vs CPU oracle at {cpu_iter}:  {}", vs_cpu.summary());
        // Fresh headless render of the same texel grid, settings and limits.
        let mut fresh = new_renderer(backend, o)?;
        fresh.viewport = Viewport {
            center: v.center.clone(),
            scale: sim.texel_scale,
        };
        fresh.settings.supersampling = Supersampling::Off;
        fresh.settings.auto_iterations = false;
        fresh.settings.manual_iterations = sim.max_iter;
        fresh.settings.max_iterations_cap = r.settings.max_iterations_cap.max(sim.max_iter);
        fresh.config.offscreen_extension = (sim.hard_iter / sim.max_iter.max(1)).max(1);
        fresh.config.extension_budget = r.config.extension_budget;
        let gres = render(&mut fresh, sim.level_width, sim.level_height, 0, true)?;
        let fresh_field = IterationField::from_texels(
            sim.level_width,
            sim.level_height,
            gres.iterations.as_deref().unwrap_or(&[]),
        );
        let (vs_fresh, fresh_heatmap) = compare(&field, &fresh_field);
        println!("{tag}vs fresh GPU:   {}", vs_fresh.summary());
        if let Some(path) = o.get("heatmap") {
            let base = if n > 1 {
                suffixed(path, &format!("-{}", i + 1))
            } else {
                path.to_string()
            };
            write_png_bgra(&base, &heatmap, sim.level_width, sim.level_height)?;
            write_png_bgra(
                suffixed(&base, "-fresh"),
                &fresh_heatmap,
                sim.level_width,
                sim.level_height,
            )?;
        }
        if let Some(path) = o.get("out") {
            let base = if n > 1 {
                suffixed(path, &format!("-{}", i + 1))
            } else {
                path.to_string()
            };
            write_png_bgra(&base, &sim.pixels, width, height)?;
        }
        if let Some(path) = o.get("dump") {
            let base = if n > 1 {
                path.replace(".iter", &format!("-{}.iter", i + 1))
            } else {
                path.to_string()
            };
            field.write(base)?;
        }
        if sim.preview {
            failed.push(format!("{tag}final frame is a preview"));
        }
        if vs_cpu.false_interior_pct() > max_false_interior {
            failed.push(format!(
                "{tag}{:.3}% exterior pixels painted interior (> {:.3}%)",
                vs_cpu.false_interior_pct(),
                max_false_interior
            ));
        }
        if vs_fresh.mismatch_pct() > max_mismatch {
            failed.push(format!(
                "{tag}{:.3}% differ from a fresh render (> {:.3}%)",
                vs_fresh.mismatch_pct(),
                max_mismatch
            ));
        }
    }
    if !failed.is_empty() {
        eprintln!("almondbread: history {mode} FAILED: {}", failed.join("; "));
        return Ok(1);
    }
    println!("history {mode} OK");
    Ok(0)
}

// MARK: bench

/// `bench`: the fixed benchmark set, a markdown table of timings plus GPU-vs-CPU verify
/// metrics. Exits 1 if any location paints more than 0.02 % CPU-exterior pixels as interior.
pub fn bench_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    let sizes: Vec<(usize, usize)> = o
        .get("sizes")
        .unwrap_or("3456x2234,1600x1000")
        .split(',')
        .map(crate::options::parse_size)
        .collect::<Result<_>>()?;
    let verify_size = o.size("verify-size", (400, 259))?;
    let only: Option<Vec<&str>> = o.get("only").map(|v| v.split(',').collect());
    let repeats = o.usize("repeat").unwrap_or(2).max(1);
    let out_dir = o.get("out");
    let do_verify = o.get("no-verify") != Some("1");

    let mut r = new_renderer(backend, o)?;
    r.settings.auto_iterations = false;
    if let Some(dir) = out_dir {
        fs::create_dir_all(dir)?;
    }
    println!("| location | size | maxIter | GPU ms | total ms | chunks | px-iter | done-iter | Giter/s | ref ms |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|");
    let mut verify_rows = Vec::new();
    let mut false_interior = Vec::new();
    for loc in BENCHMARK_LOCATIONS {
        if only.as_ref().is_some_and(|o| !o.contains(&loc.name)) {
            continue;
        }
        r.settings.manual_iterations = loc.max_iterations;
        r.settings.max_iterations_cap = r.settings.max_iterations_cap.max(loc.max_iterations);
        for &(w, h) in &sizes {
            r.viewport = loc.viewport_for_width(w);
            // Warm-up builds the reference orbit and textures; then best of `repeats`.
            let mut last = None;
            let mut gpu_ms = f64::INFINITY;
            let mut wall_ms = f64::INFINITY;
            for i in 0..repeats {
                let res = render(&mut r, w, h, usize::from(i == 0), i == repeats - 1)?;
                gpu_ms = gpu_ms.min(res.iterate_gpu_time * 1000.0);
                wall_ms = wall_ms.min(res.wall_time * 1000.0);
                last = Some(res);
            }
            let res = last.expect("rendered");
            let mut px_iter = 0.0;
            for t in res.iterations.as_deref().unwrap_or(&[]) {
                px_iter += if t[0] >= 0.0 {
                    t[0] as f64
                } else {
                    loc.max_iterations as f64
                };
            }
            let done = if res.iterations_done > 0 {
                g(res.iterations_done as f64, 3)
            } else {
                "-".into()
            };
            let rate = if res.iterations_done > 0 {
                res.iterations_done as f64
            } else {
                px_iter
            } / gpu_ms
                / 1e6;
            let (ref_time, _) = r.reference_info();
            println!(
                "| {} | {}x{} | {} | {:.1} | {:.1} | {} | {} | {} | {:.1} | {:.2} |",
                loc.name,
                w,
                h,
                loc.max_iterations,
                gpu_ms,
                wall_ms,
                res.chunks,
                g(px_iter, 3),
                done,
                rate,
                ref_time * 1000.0
            );
            if let Some(dir) = out_dir {
                write_png_bgra(format!("{dir}/{}-{w}.png", loc.name), &res.pixels, w, h)?;
                if let Some(t) = &res.iterations {
                    IterationField::from_texels(t.len() / h, h, t).write(format!("{dir}/{}-{w}.iter", loc.name))?;
                }
            }
        }
        if do_verify {
            let (w, h) = verify_size;
            r.viewport = loc.viewport_for_width(w);
            let v = verify(&mut r, w, h, None)?;
            let m = &v.report;
            // Exterior painted as interior is a bug, not float noise.
            if m.false_interior_pct() > 0.02 {
                false_interior.push(format!("{} {:.3}%", loc.name, m.false_interior_pct()));
            }
            verify_rows.push(format!(
                "| {} | {}x{} | {:.3} | {:.3} | {:.3} | {:.3} | {} | {} | {} | {:.3} | {:.1} | {:.1} |",
                loc.name,
                w,
                h,
                m.escape_differs_pct(),
                m.mismatch_pct(),
                m.false_interior_pct(),
                m.false_exterior_pct(),
                g(m.max_error, 3),
                g(m.median_error, 3),
                g(m.p99_error, 3),
                m.over005_pct(),
                m.pct(m.cpu_interior),
                m.pct(m.detected_interior)
            ));
            if let Some(dir) = out_dir {
                write_png_bgra(format!("{dir}/{}-diff.png", loc.name), &v.heatmap, w, h)?;
                v.gpu.write(format!("{dir}/{}-verify.iter", loc.name))?;
            }
        }
    }
    if do_verify {
        println!();
        println!(
            "| location | verify size | escape-n differs % | class mismatch % | GPU-int/CPU-ext % | GPU-ext/CPU-int % | max abs Δmu | median | p99 | >0.05 % | CPU interior % | GPU detected % |"
        );
        println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        for row in &verify_rows {
            println!("{row}");
        }
    }
    if !false_interior.is_empty() {
        eprintln!(
            "almondbread: bench FAILED: exterior pixels painted interior (GPU-int/CPU-ext > 0.02%): {}",
            false_interior.join(", ")
        );
        return Ok(1);
    }
    Ok(0)
}

// MARK: regress

/// The -3/4 neck, seahorse and M4 dense-view regression sessions of `make regress` (kb:
/// techniques/interior-detection, Pitfalls; synthesis/benchmarks M4): Space zoom, then
/// (neck) one wheel step.
pub const REGRESS_SESSIONS: &[&[&str]] = &[
    &[
        "--history",
        "zoom",
        "--center",
        "-0.7500016386523336,-0.0021073424656852589",
        "--scale",
        "3.38526e-8",
        "--ppp",
        "2",
        "--size",
        "640x410",
        "--then",
        "1.3225",
    ],
    &[
        "--history",
        "zoom",
        "--center",
        "-0.75000661155604931,-0.0021104008406531076",
        "--scale",
        "1.062288e-9",
        "--ppp",
        "2",
        "--size",
        "640x410",
        "--pointer",
        "0,-58.5",
        "--then",
        "1.3225",
    ],
    &[
        "--history",
        "zoom",
        "--center",
        "-0.743643887037151,0.13182590420533",
        "--scale",
        "8.5e-9",
        "--size",
        "1280x820",
        "--pointer",
        "200,100",
    ],
    // M4: the boundary-dense view of the slow-render report (kb: synthesis/benchmarks M4),
    // with BLA as the app decides (auto: off here) and forced on (the BLA kernel through the
    // interactive pipeline: previews, the full level waiting for its table). Float32 alone
    // paints 0.057 % of CPU-exterior pixels interior here (escapes just past the limit with
    // the interior test switched off; the same before M4), hence 0.1 %.
    &[
        "--history",
        "zoom",
        "--center",
        "-1.4080321340304542,0.13680236432900417",
        "--scale",
        "4.615e-14",
        "--size",
        "640x400",
        "--max-false-interior",
        "0.1",
    ],
    &[
        "--history",
        "zoom",
        "--center",
        "-1.4080321340304542,0.13680236432900417",
        "--scale",
        "4.615e-14",
        "--size",
        "640x400",
        "--max-false-interior",
        "0.1",
        "--bla",
        "1",
    ],
];

pub fn regress_command<B: GpuBackend + Clone>(backend: &B, extra: &Options) -> Result<i32> {
    let mut status = 0;
    for session in REGRESS_SESSIONS {
        let mut args: Vec<String> = session.iter().map(|s| s.to_string()).collect();
        // Extra options (e.g. --extension-budget) apply to every session.
        for key in ["extension-budget", "max-mismatch", "max-false-interior", "chunk-ms"] {
            if let Some(v) = extra.get(key) {
                args.extend([format!("--{key}"), v.to_string()]);
            }
        }
        let o = Options::parse(&args)?;
        println!("== verify {}", session.join(" "));
        if history(backend, &o)? != 0 {
            status = 1;
        }
    }
    Ok(status)
}
