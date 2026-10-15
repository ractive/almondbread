//! `almondbread precision`: are float32 rounding artefacts real? (M7, kb: techniques/double-float,
//! questions/float32-error-long-orbits.)
//!
//! A GPU render (float32 perturbation) against the CPU oracle (f64 deltas), next to *control*
//! oracles whose whole pixel grid is shifted by a fraction of a pixel. A view whose escape
//! counts change under a 0.001-pixel shift is chaotic at this sampling: any image of it is one
//! random draw, and the GPU's disagreement with the oracle is an artefact only where it is
//! larger than what a sub-pixel shift does. The "equivalent shift" printed is the smallest
//! control shift that disagrees with the oracle at least as much as the GPU does.
//!
//! Both fields are also coloured by one CPU colouring (the plain look's mapping: palette at
//! `fract(n density) + s density`, no filtering), so the images differ only by their fields:
//! `--out prefix` writes `prefix-gpu.png`, `prefix-cpu.png`, `prefix-heat.png` and, for each
//! control, `prefix-shift<k>.png`.
//!
//! ```text
//! almondbread precision --location sea-8007 --size 400x260 [--shifts 0.001,0.01,0.1,0.5]
//!                       [--out dir/name] [--interior 0] [--ff 1] [view and renderer options as verify]
//! almondbread precision --kat 1 [--threads n --steps k --repeat r]
//! ```
//!
//! `--shifts` takes diagonal lengths (`0.1`) or x:y pairs (`0.2:-0.1`) in pixels;
//! `--control-zoom f` / `--control-angle a` also scale / turn the controls' grid, and each
//! control line prints how well its smooth-count error correlates with the GPU's (is the
//! GPU's error a shift, a scale, a turn or noise?). `--out` also writes `prefix-signed.png`
//! (signed error map) and `prefix-look.png` (the renderer's own colouring).
//!
//! `--ff 1` adds the float-float field (M7, [`almondbread_render::floatfloat`]) against the
//! oracle and the float32 field; `--kat 1` runs the float-float known-answer test under safe
//! and fast math and the float32 / float-float / fx throughput benchmark.

use almondbread_core::cfmt::g;
use almondbread_core::image::write_png_bgra;
use almondbread_core::oracle::{
    IterationField, OracleReference, VerifyReport, compare, cpu_render, cpu_render_deep_with,
};
use almondbread_core::palette::Palette;
use almondbread_core::reference::DEEP_SCALE;
use almondbread_core::settings::RenderSettings;
use almondbread_core::viewport::Viewport;
use almondbread_render::floatfloat::{BenchKind, FloatFloat};
use almondbread_render::scheduler::BAILOUT;
use almondbread_render::{GpuBackend, MathMode, Renderer};

use crate::options::{Options, Result};

/// The oracle of `view` with its pixel grid shifted by (sx, sy) texels.
pub fn shifted_oracle(
    view: &Viewport,
    width: usize,
    height: usize,
    max_iter: usize,
    shift: (f64, f64),
    reference: &mut Option<OracleReference>,
) -> IterationField {
    let texel = view.scale;
    if texel.to_f64() >= DEEP_SCALE {
        let s = texel.to_f64();
        let (sn, cs) = view.angle.sin_cos();
        let (dx, dy) = (shift.0 * cs - shift.1 * sn, shift.0 * sn + shift.1 * cs);
        return cpu_render(
            view.center.re() + dx * s,
            view.center.im() + dy * s,
            s,
            width,
            height,
            max_iter,
            BAILOUT,
            view.angle,
        );
    }
    let r = reference
        .get_or_insert_with(|| OracleReference::compute(&view.center, texel, width.max(height), max_iter, BAILOUT));
    cpu_render_deep_with(r, texel, width, height, max_iter, BAILOUT, shift, view.angle)
}

/// One CPU colouring for any field (the plain look's mapping, no palette filtering): linear
/// palette entries, sRGB out, interior black, unresolved dark blue.
pub fn colorize_field(f: &IterationField, s: &RenderSettings) -> Vec<u8> {
    let table = Palette::named(&s.palette_id).table(1024);
    let density = s.color_density as f32;
    let offset = s.color_offset as f32;
    let srgb = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (e * 255.0).round() as u8
    };
    let mut out = vec![0u8; f.n.len() * 4];
    for (k, px) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let n = f.n[k];
        let rgb = if n >= 0.0 {
            let t = (n * density).fract() + f.s[k] * density + offset;
            let x = t.rem_euclid(1.0) * 1024.0 - 0.5;
            let i0 = (x.floor() as i64).rem_euclid(1024) as usize;
            let i1 = (i0 + 1) % 1024;
            let w = x - x.floor();
            let a = table[i0];
            let b = table[i1];
            [
                a[0] + (b[0] - a[0]) * w,
                a[1] + (b[1] - a[1]) * w,
                a[2] + (b[2] - a[2]) * w,
            ]
        } else if n == almondbread_core::oracle::ITER_UNRESOLVED {
            [0.0, 0.0, 0.2]
        } else {
            [0.0, 0.0, 0.0]
        };
        px.copy_from_slice(&[srgb(rgb[2]), srgb(rgb[1]), srgb(rgb[0]), 255]);
    }
    out
}

/// Signed smooth-count error gpu - cpu over pixels exterior in both: red positive, blue
/// negative, log ramp from 1e-5 (black) to 1 (full); grey where a class differs or interior.
/// A systematic error (a shifted or scaled grid, a biased escape radius) shows as smooth
/// regions of one sign, rounding noise as speckle.
pub fn signed_map(gpu: &IterationField, cpu: &IterationField) -> Vec<u8> {
    let mut out = vec![0u8; gpu.n.len() * 4];
    for (k, px) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (a, b) = (gpu.n[k], cpu.n[k]);
        if a < 0.0 || b < 0.0 {
            px.copy_from_slice(&[40, 40, 40, 255]);
            continue;
        }
        let d = (a as f64 + gpu.s[k] as f64) - (b as f64 + cpu.s[k] as f64);
        let t = ((d.abs().max(1e-7).log10() + 5.0) / 5.0).clamp(0.0, 1.0);
        let v = (t * 255.0) as u8;
        px.copy_from_slice(&if d > 0.0 { [0, 0, v, 255] } else { [v, 0, 0, 255] });
    }
    out
}

/// Correlation of the smooth-count errors `a - c` and `b - c` over pixels where all three
/// escape at the same n (the smooth part of the error, not the chaotic boundary), and the
/// least-squares gain k in `a - c ~ k (b - c)`.
pub fn error_correlation(a: &IterationField, b: &IterationField, c: &IterationField) -> (f64, f64) {
    let (mut sxx, mut syy, mut sxy, mut count) = (0.0, 0.0, 0.0, 0usize);
    for k in 0..c.n.len() {
        if c.n[k] < 0.0 || a.n[k] != c.n[k] || b.n[k] != c.n[k] {
            continue;
        }
        let x = (b.s[k] - c.s[k]) as f64;
        let y = (a.s[k] - c.s[k]) as f64;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
        count += 1;
    }
    if count == 0 || sxx == 0.0 || syy == 0.0 {
        return (0.0, 0.0);
    }
    (sxy / (sxx * syy).sqrt(), sxy / sxx)
}

/// "Visibly different" pixel count between two colourings: max channel difference > 24/255.
pub fn visible_differences(a: &[u8], b: &[u8]) -> usize {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(p, q)| (0..3).any(|i| (p[i] as i32 - q[i] as i32).abs() > 24))
        .count()
}

/// Smallest control shift (pixels) at least as far from the oracle as `gpu` is, by the
/// share of pixels with |Δmu| > 0.05; "< smallest" or "> largest" outside the range.
pub fn equivalent_shift(gpu: &VerifyReport, controls: &[(f64, VerifyReport)]) -> String {
    let mut sorted: Vec<&(f64, VerifyReport)> = controls.iter().collect();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    match sorted.iter().find(|(_, c)| c.over005 >= gpu.over005) {
        Some((s, _)) => format!("<= {} px", g(*s, 3)),
        None => format!("> {} px", g(sorted.last().map_or(0.0, |c| c.0), 3)),
    }
}

/// `--ff 1` of `verify` and `precision`: the view in float-float (M7 scaffolding) against
/// the oracle and against the float32 field. Prints a fallback line when the known-answer
/// test fails. Returns the float-float field.
pub fn ff_report<B: GpuBackend + Clone>(
    backend: &B,
    view: &Viewport,
    width: usize,
    height: usize,
    max_iter: usize,
    gpu: &IterationField,
    cpu: &IterationField,
) -> Option<IterationField> {
    let (ff, _) = match FloatFloat::new(backend) {
        Ok(f) => f,
        Err(e) => {
            println!("float-float unavailable, float32 only: {e}");
            return None;
        }
    };
    match ff.render_view(view, width, height, max_iter, BAILOUT) {
        Ok((field, st)) => {
            let (vs_cpu, _) = compare(&field, cpu);
            let (vs_f32, _) = compare(gpu, &field);
            println!(
                "float-float GPU {:.2} ms in {} command buffers ({} it/s; reference {:.0} ms CPU)",
                st.gpu_time * 1000.0,
                st.dispatches,
                g(st.iterations as f64 / st.gpu_time.max(1e-9), 3),
                st.reference_time * 1000.0
            );
            println!("float-float vs oracle: {}", vs_cpu.summary());
            println!("float32 vs float-float: {}", vs_f32.summary());
            Some(field)
        }
        Err(e) => {
            println!("float-float render failed: {e}");
            None
        }
    }
}

/// `precision --kat 1`: the known-answer test under safe and fast math, and the throughput
/// of each number type (kb: questions/double-float-speed-on-m1).
fn kat_and_bench<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    for math in [MathMode::Safe, MathMode::Fast] {
        let ff = FloatFloat::compile(backend, math)?;
        println!("{}", ff.known_answer_test()?.summary());
    }
    let ff = FloatFloat::compile(backend, MathMode::Safe)?;
    let threads = o.usize("threads").unwrap_or(1 << 18);
    let steps = o.usize("steps").unwrap_or(2000) as u32;
    let rounds = o.usize("repeat").unwrap_or(5);
    let mut med = Vec::new();
    for kind in [BenchKind::F32, BenchKind::FloatFloat, BenchKind::Fx] {
        // Interleaved repeats would be better under load; the median of a few is close enough
        // for ratios of 10x and more.
        let mut v: Vec<f64> = (0..rounds)
            .map(|_| ff.bench(kind, threads, steps))
            .collect::<std::result::Result<_, _>>()?;
        v.sort_by(f64::total_cmp);
        med.push(v[v.len() / 2]);
        println!(
            "{kind:?}: {} iterations/s (median of {rounds}; {threads} threads x {steps} steps), {:.1}x slower than f32",
            g(v[v.len() / 2], 3),
            med[0] / v[v.len() / 2]
        );
    }
    Ok(0)
}

pub fn precision_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    if o.get("kat") == Some("1") {
        return kat_and_bench(backend, o);
    }
    let (width, height) = o.size("size", (400, 260))?;
    let mut r = Renderer::new(backend.clone())?;
    o.apply_settings(&mut r.settings);
    o.apply_renderer(&mut r);
    r.viewport = o.viewport(width, height)?;
    let res = r.render_offscreen(width, height, true)?;
    let gpu = IterationField::from_texels(width, height, res.iterations.as_deref().unwrap_or(&[]));
    let max_iter = r.stats.max_iterations;
    let view = r.viewport.clone();
    let mut reference = None;
    let t0 = std::time::Instant::now();
    let cpu = shifted_oracle(&view, width, height, max_iter, (0.0, 0.0), &mut reference);
    let cpu_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let (report, heat) = compare(&gpu, &cpu);
    let gpu_png = colorize_field(&gpu, &r.settings);
    let cpu_png = colorize_field(&cpu, &r.settings);
    println!(
        "view {}x{} maxIter {max_iter} texel {}  GPU {:.2} ms  oracle {:.0} ms",
        width,
        height,
        view.scale,
        res.iterate_gpu_time * 1000.0,
        cpu_ms
    );
    println!(
        "GPU   vs oracle: {}  visibly different {:.3}%",
        report.summary(),
        100.0 * visible_differences(&gpu_png, &cpu_png) as f64 / (width * height) as f64
    );
    // `0.1`: a diagonal shift of 0.1 px; `0.2:-0.1`: shift (x, y) in pixels (y up).
    let shifts: Vec<(f64, (f64, f64))> = o
        .get("shifts")
        .unwrap_or("0.001,0.01,0.1,0.5")
        .split(',')
        .filter_map(|v| match v.split_once(':') {
            Some((x, y)) => {
                let (x, y): (f64, f64) = (x.trim().parse().ok()?, y.trim().parse().ok()?);
                Some((x.hypot(y), (x, y)))
            }
            None => {
                let d: f64 = v.trim().parse().ok()?;
                Some((d, (d / std::f64::consts::SQRT_2, d / std::f64::consts::SQRT_2)))
            }
        })
        .collect();
    let prefix = o.get("out").map(String::from);
    let mut controls = Vec::new();
    for (k, &(sh, xy)) in shifts.iter().enumerate() {
        // `--control-zoom f`: the controls' grid also scaled by f about the centre (is an
        // error a shift or a scale?).
        // `--control-angle a` (radians): turned by a.
        let ctl = if o.has("control-zoom") || o.has("control-angle") {
            let mut v = view.clone();
            v.scale = v.scale * o.f64("control-zoom").unwrap_or(1.0);
            v.angle += o.f64("control-angle").unwrap_or(0.0);
            shifted_oracle(&v, width, height, max_iter, xy, &mut None)
        } else {
            shifted_oracle(&view, width, height, max_iter, xy, &mut reference)
        };
        let (corr, gain) = error_correlation(&gpu, &ctl, &cpu);
        let (rep, _) = compare(&ctl, &cpu);
        let png = colorize_field(&ctl, &r.settings);
        println!(
            "shift {:>6} px ({}, {}) vs oracle: {}  visibly different {:.3}%  smooth-error correlation with GPU {:.3} (GPU = {} x this)",
            g(sh, 3),
            g(xy.0, 3),
            g(xy.1, 3),
            rep.summary(),
            100.0 * visible_differences(&png, &cpu_png) as f64 / (width * height) as f64,
            corr,
            g(gain, 3)
        );
        if let Some(p) = &prefix {
            write_png_bgra(format!("{p}-shift{k}.png"), &png, width, height)?;
        }
        controls.push((sh, rep));
    }
    if !controls.is_empty() {
        println!("GPU error ~ a grid shift of {}", equivalent_shift(&report, &controls));
    }
    let ff = if o.get("ff") == Some("1") {
        ff_report(backend, &view, width, height, max_iter, &gpu, &cpu)
    } else {
        None
    };
    if let (Some(f), Some(p)) = (&ff, &prefix) {
        write_png_bgra(format!("{p}-ff.png"), &colorize_field(f, &r.settings), width, height)?;
        write_png_bgra(format!("{p}-ff-signed.png"), &signed_map(f, &cpu), width, height)?;
    }
    if let Some(p) = &prefix {
        write_png_bgra(format!("{p}-gpu.png"), &gpu_png, width, height)?;
        write_png_bgra(format!("{p}-cpu.png"), &cpu_png, width, height)?;
        write_png_bgra(format!("{p}-heat.png"), &heat, width, height)?;
        write_png_bgra(format!("{p}-signed.png"), &signed_map(&gpu, &cpu), width, height)?;
        write_png_bgra(format!("{p}-look.png"), &res.pixels, width, height)?;
    }
    Ok(0)
}
