//! `metalfx-probe test`: synthetic sanity checks of the conventions (jitter sign, motion
//! sign, depth value) with PNGs in `out/`. `metalfx-probe bench`: GPU time per upscale.

use std::fs;
use std::path::Path;
use std::time::Instant;

use metalfx_probe::{Kind, Upscaler, affine_motion, device_support, halton};

type Img = Vec<[f32; 3]>;

/// Test pattern in content units (= output pixels of the initial view). Asymmetric on
/// purpose: a zone plate (R), thin rotated lines (G), sharp-edged checker blocks (B).
fn pattern(u: f64, v: f64) -> [f32; 3] {
    let (dx, dy) = (u - 260.0, v - 170.0);
    let r2 = dx * dx + dy * dy;
    // local frequency k*r/pi cycles per output px: 0.3 c/px at r = 300 (above the input's
    // Nyquist of 0.25 c/output px beyond r = 250, below the output's 0.5 everywhere)
    let k = 0.3 * std::f64::consts::PI / 300.0;
    let zone = 0.5 + 0.5 * (k * r2).cos();
    let t = (u * 0.94 + v * 0.34) / 12.0; // lines every 12 px across, 3 px wide, sharp edges
    let lines = if t - t.floor() < 0.25 { 1.0 } else { 0.1 };
    let cb = ((u / 9.0).floor() as i64 + (v / 13.0).floor() as i64).rem_euclid(2) as f64;
    let blocks = if u > 380.0 && v < 150.0 {
        cb
    } else {
        0.3 + 0.4 * (u / 640.0)
    };
    [zone as f32, lines as f32, blocks as f32]
}

/// View: content = centre + (p - c) * h, p in pixels of a w x h image, h content units per px.
#[derive(Clone, Copy)]
struct View {
    cu: [f64; 2],
    h: f64, // content units per INPUT pixel
}

fn render_input(view: View, w: usize, hgt: usize, jitter: [f32; 2]) -> Img {
    let (cx, cy) = (w as f64 * 0.5, hgt as f64 * 0.5);
    let mut img = Vec::with_capacity(w * hgt);
    for j in 0..hgt {
        for i in 0..w {
            let px = i as f64 + jitter[0] as f64;
            let py = j as f64 + jitter[1] as f64;
            img.push(pattern(
                view.cu[0] + (px - cx) * view.h,
                view.cu[1] + (py - cy) * view.h,
            ));
        }
    }
    img
}

/// 4x4 box-supersampled reference at output resolution.
fn render_reference(view: View, ratio: f64, w: usize, hgt: usize) -> Img {
    let (cx, cy) = (w as f64 * 0.5, hgt as f64 * 0.5);
    let hh = view.h / ratio;
    let mut img = Vec::with_capacity(w * hgt);
    for j in 0..hgt {
        for i in 0..w {
            let mut acc = [0.0f32; 3];
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = i as f64 + (sx as f64 + 0.5) / 4.0;
                    let py = j as f64 + (sy as f64 + 0.5) / 4.0;
                    let c = pattern(view.cu[0] + (px - cx) * hh, view.cu[1] + (py - cy) * hh);
                    for k in 0..3 {
                        acc[k] += c[k] / 16.0;
                    }
                }
            }
            img.push(acc);
        }
    }
    img
}

/// Bilinear upscale of a centre-sampled input image.
fn bilinear(src: &Img, sw: usize, sh: usize, dw: usize, dh: usize) -> Img {
    let (rx, ry) = (sw as f64 / dw as f64, sh as f64 / dh as f64);
    let mut out = Vec::with_capacity(dw * dh);
    for y in 0..dh {
        let fy = ((y as f64 + 0.5) * ry - 0.5).clamp(0.0, (sh - 1) as f64);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let ty = (fy - y0 as f64) as f32;
        for x in 0..dw {
            let fx = ((x as f64 + 0.5) * rx - 0.5).clamp(0.0, (sw - 1) as f64);
            let x0 = fx.floor() as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let tx = (fx - x0 as f64) as f32;
            let mut c = [0.0; 3];
            for k in 0..3 {
                let a = src[y0 * sw + x0][k] * (1.0 - tx) + src[y0 * sw + x1][k] * tx;
                let b = src[y1 * sw + x0][k] * (1.0 - tx) + src[y1 * sw + x1][k] * tx;
                c[k] = a * (1.0 - ty) + b * ty;
            }
            out.push(c);
        }
    }
    out
}

/// RMSE over the interior (margin = fraction of each side excluded).
fn rmse(a: &Img, b: &Img, w: usize, h: usize, margin: f64) -> f64 {
    let (mx, my) = ((w as f64 * margin) as usize, (h as f64 * margin) as usize);
    let mut s = 0.0;
    let mut n = 0usize;
    for y in my..h - my {
        for x in mx..w - mx {
            for k in 0..3 {
                let d = (a[y * w + x][k] - b[y * w + x][k]) as f64;
                s += d * d;
            }
            n += 3;
        }
    }
    (s / n as f64).sqrt()
}

fn to_srgb8(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

fn write_png(path: &Path, img: &Img, w: usize, h: usize) {
    let data: Vec<u8> = img.iter().flat_map(|c| c.iter().map(|&v| to_srgb8(v))).collect();
    let f = fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("png header")
        .write_image_data(&data)
        .expect("png data");
}

/// Absolute error x4 as a grey image.
fn err_img(a: &Img, b: &Img) -> Img {
    a.iter()
        .zip(b)
        .map(|(p, q)| {
            let e = ((p[0] - q[0]).abs() + (p[1] - q[1]).abs() + (p[2] - q[2]).abs()) / 3.0 * 4.0;
            [e, e, e]
        })
        .collect()
}

fn jit(frame: u32) -> [f32; 2] {
    [halton(frame + 1, 2), halton(frame + 1, 3)]
}

const OW: usize = 640;
const OH: usize = 400;
const IW: usize = 320;
const IH: usize = 200;
const RATIO: f64 = 2.0;

/// Runs a sequence of views through a temporal upscaler. `motion_mode`: 1 = correct,
/// 0 = zero, -1 = negated. Returns the last output and its reference.
fn run_sequence(
    up: &mut Upscaler,
    views: &[View],
    motion_mode: f64,
    use_jitter: bool,
) -> Result<(Img, Img, Vec<f64>), String> {
    let mut last = Vec::new();
    let mut errs = Vec::new();
    let mut gpu = Vec::new();
    for (t, &v) in views.iter().enumerate() {
        let j = if use_jitter { jit(t as u32) } else { [0.5, 0.5] };
        let input = render_input(v, IW, IH, j);
        let motion = if t == 0 {
            vec![[0.0, 0.0]; IW * IH]
        } else {
            let p = views[t - 1];
            let step = v.h / p.h;
            let shift = [
                (v.cu[0] - p.cu[0]) / p.h * motion_mode,
                (v.cu[1] - p.cu[1]) / p.h * motion_mode,
            ];
            let step = 1.0 + (step - 1.0) * motion_mode;
            affine_motion(IW, IH, step, shift)
        };
        let (out, ms) = up.upscale(&input, Some(&motion), j, t == 0)?;
        gpu.push(ms);
        if t + 1 == views.len() {
            last = out;
        } else if t % 4 == 3 {
            let r = render_reference(v, RATIO, OW, OH);
            errs.push(rmse(&out, &r, OW, OH, 0.1));
        }
    }
    let r = render_reference(*views.last().unwrap(), RATIO, OW, OH);
    errs.push(rmse(&last, &r, OW, OH, 0.1));
    let _ = gpu;
    Ok((last, r, errs))
}

fn test() -> Result<(), String> {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("out");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let ds = device_support()?;
    println!("device {:?}", ds);

    let v0 = View {
        cu: [OW as f64 * 0.5, OH as f64 * 0.5],
        h: RATIO,
    };
    let reference = render_reference(v0, RATIO, OW, OH);
    write_png(&out.join("reference.png"), &reference, OW, OH);
    let centre_in = render_input(v0, IW, IH, [0.5, 0.5]);
    let bil = bilinear(&centre_in, IW, IH, OW, OH);
    write_png(&out.join("bilinear.png"), &bil, OW, OH);
    let e_bil = rmse(&bil, &reference, OW, OH, 0.1);
    println!("\n(a) static scene {IW}x{IH} -> {OW}x{OH}, 16 jittered frames (Halton 2,3)");
    println!("  bilinear of centre-sampled input      RMSE {e_bil:.4}");

    let mut sp = Upscaler::new(Kind::Spatial, IW, IH, OW, OH)?;
    let (spo, sp_ms) = sp.upscale(&centre_in, None, [0.5, 0.5], true)?;
    write_png(&out.join("spatial.png"), &spo, OW, OH);
    println!(
        "  MetalFX spatial                       RMSE {:.4} ({sp_ms:.3} ms GPU)",
        rmse(&spo, &reference, OW, OH, 0.1)
    );

    let statics = vec![v0; 16];
    let mut best = (f64::MAX, [0.0f32; 2]);
    for sign in [
        [1.0, 1.0],
        [-1.0, -1.0],
        [1.0, -1.0],
        [-1.0, 1.0],
        [-2.0, -2.0],
        [-0.5, -0.5],
    ] {
        let mut up = Upscaler::new(Kind::Temporal, IW, IH, OW, OH)?;
        up.jitter_sign = sign;
        let (img, r, errs) = run_sequence(&mut up, &statics, 1.0, true)?;
        let e = rmse(&img, &r, OW, OH, 0.1);
        println!(
            "  temporal jitterOffset = ({:+},{:+}) * (j - 0.5): RMSE after 4/8/12/16 frames {:?}",
            sign[0],
            sign[1],
            errs.iter().map(|e| format!("{e:.4}")).collect::<Vec<_>>()
        );
        write_png(
            &out.join(format!("static_temporal_sign{:+}{:+}.png", sign[0], sign[1])),
            &img,
            OW,
            OH,
        );
        if e < best.0 {
            best = (e, sign);
        }
    }
    {
        let mut up = Upscaler::new(Kind::Temporal, IW, IH, OW, OH)?;
        let (img, _, errs) = run_sequence(&mut up, &statics, 1.0, false)?;
        println!(
            "  temporal, no jitter (all samples at centres): RMSE {:?}",
            errs.iter().map(|e| format!("{e:.4}")).collect::<Vec<_>>()
        );
        write_png(&out.join("static_temporal_nojitter.png"), &img, OW, OH);
    }
    println!(
        "  best jitter sign {:?} (RMSE {:.4}) vs library default {:?}",
        best.1,
        best.0,
        metalfx_probe::JITTER_SIGN
    );

    // (b) pan: content moves right/down on screen by (1.3, 0.7) output px per frame.
    println!("\n(b) pan, content moving (+1.3,+0.7) output px/frame (= -0.65,-0.35 input px motion), 32 frames");
    let pan: Vec<View> = (0..32)
        .map(|t| View {
            cu: [v0.cu[0] - 1.3 * t as f64, v0.cu[1] - 0.7 * t as f64],
            h: RATIO,
        })
        .collect();
    let mut pan_results = Vec::new();
    for (name, mode) in [
        ("correct", 1.0),
        ("zero", 0.0),
        ("negated", -1.0),
        ("x2", 2.0),
        ("x0.5", 0.5),
    ] {
        let mut up = Upscaler::new(Kind::Temporal, IW, IH, OW, OH)?;
        let (img, r, errs) = run_sequence(&mut up, &pan, mode, true)?;
        println!(
            "  motion {name:8}: RMSE every 4 frames {:?}",
            errs.iter().map(|e| format!("{e:.4}")).collect::<Vec<_>>()
        );
        write_png(&out.join(format!("pan_motion_{name}.png")), &img, OW, OH);
        write_png(
            &out.join(format!("pan_motion_{name}_err.png")),
            &err_img(&img, &r),
            OW,
            OH,
        );
        if name == "correct" {
            write_png(&out.join("pan_reference.png"), &r, OW, OH);
        }
        pan_results.push((name, *errs.last().unwrap()));
    }
    {
        let last = *pan.last().unwrap();
        let b = bilinear(&render_input(last, IW, IH, [0.5, 0.5]), IW, IH, OW, OH);
        let r = render_reference(last, RATIO, OW, OH);
        println!(
            "  bilinear of the last frame alone: RMSE {:.4}",
            rmse(&b, &r, OW, OH, 0.1)
        );
    }

    // (c) zoom in by 3 %/frame around the centre (exercises affine_motion's step).
    println!("\n(c) zoom in 3 %/frame, 32 frames");
    let zoom: Vec<View> = (0..32)
        .map(|t| View {
            cu: [300.0, 180.0],
            h: RATIO * 0.97f64.powi(t),
        })
        .collect();
    for (name, mode) in [("correct", 1.0), ("zero", 0.0), ("negated", -1.0)] {
        let mut up = Upscaler::new(Kind::Temporal, IW, IH, OW, OH)?;
        let (img, r, errs) = run_sequence(&mut up, &zoom, mode, true)?;
        println!(
            "  motion {name:8}: RMSE every 4 frames {:?}",
            errs.iter().map(|e| format!("{e:.4}")).collect::<Vec<_>>()
        );
        write_png(&out.join(format!("zoom_motion_{name}.png")), &img, OW, OH);
        if name == "correct" {
            write_png(&out.join("zoom_reference.png"), &r, OW, OH);
        }
    }
    {
        let last = *zoom.last().unwrap();
        let b = bilinear(&render_input(last, IW, IH, [0.5, 0.5]), IW, IH, OW, OH);
        let r = render_reference(last, RATIO, OW, OH);
        println!(
            "  bilinear of the last frame alone: RMSE {:.4}",
            rmse(&b, &r, OW, OH, 0.1)
        );
    }

    // (e) pan speed sweep (correct motion): how much of the static convergence survives motion.
    println!("\n(e) pan speed sweep, correct motion, RMSE after 32 frames (bilinear of last frame alone)");
    for vel in [
        [0.1, 0.05],
        [0.25, 0.0],
        [0.5, 0.5],
        [1.3, 0.7],
        [2.0, 0.0],
        [4.0, 0.0],
        [8.0, 3.0],
    ] {
        let seq: Vec<View> = (0..32)
            .map(|t| View {
                cu: [v0.cu[0] - vel[0] * t as f64, v0.cu[1] - vel[1] * t as f64],
                h: RATIO,
            })
            .collect();
        let mut up = Upscaler::new(Kind::Temporal, IW, IH, OW, OH)?;
        let (_, _, errs) = run_sequence(&mut up, &seq, 1.0, true)?;
        let last = *seq.last().unwrap();
        let b = bilinear(&render_input(last, IW, IH, [0.5, 0.5]), IW, IH, OW, OH);
        let r = render_reference(last, RATIO, OW, OH);
        println!(
            "  content velocity ({:+.2},{:+.2}) output px/frame: temporal {:.4}  bilinear {:.4}",
            vel[0],
            vel[1],
            errs.last().unwrap(),
            rmse(&b, &r, OW, OH, 0.1)
        );
    }

    // (d) depth value / depthReversed on the pan with correct motion.
    println!("\n(d) constant depth variants (pan, correct motion), final RMSE");
    for (d, rev) in [(1.0, false), (0.0, false), (1.0, true), (0.0, true), (0.5, false)] {
        let mut up = Upscaler::new(Kind::Temporal, IW, IH, OW, OH)?;
        up.set_depth(d, rev);
        let (_, _, errs) = run_sequence(&mut up, &pan, 1.0, true)?;
        println!("  depth {d} reversed {rev}: {:.4}", errs.last().unwrap());
    }
    println!("\nPNGs in {}", out.display());
    let _ = pan_results;
    Ok(())
}

fn stats(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
    (q(0.5), q(0.9), v[0])
}

fn bench() -> Result<(), String> {
    let ds = device_support()?;
    println!("{ds:?}");
    let outs = [(1600usize, 1000usize), (3456, 2160)];
    let scales = [1.5f64, 2.0, 3.0];
    struct Cfg {
        name: String,
        up: Upscaler,
        times: Vec<Vec<f64>>,
    }
    let mut cfgs = Vec::new();
    println!("\ncreation (requiresSynchronousInitialization = true for temporal):");
    println!(
        "{:<34} {:>10} {:>14} {:>12} {:>16}",
        "config", "create ms", "scaler alloc MB", "io alloc MB", "contentScale min/max"
    );
    for kind in [Kind::Temporal, Kind::Spatial] {
        for &(ow, oh) in &outs {
            for &s in &scales {
                let (iw, ih) = ((ow as f64 / s).round() as usize, (oh as f64 / s).round() as usize);
                let name = format!("{kind:?} {iw}x{ih}->{ow}x{oh} (1/{s})");
                match Upscaler::new(kind, iw, ih, ow, oh) {
                    Ok(mut up) => {
                        // upload one frame of content so the scaler sees real data
                        let img: Img = (0..iw * ih)
                            .map(|i| pattern((i % iw) as f64 * s, (i / iw) as f64 * s))
                            .collect();
                        let mo = affine_motion(iw, ih, 0.99, [0.3, -0.2]);
                        up.upscale(&img, Some(&mo), [0.5, 0.5], true)?;
                        println!(
                            "{:<34} {:>10.1} {:>14.1} {:>12.1} {:>8.2}/{:.2}",
                            name,
                            up.info.create_ms,
                            up.info.scaler_alloc_bytes as f64 / 1048576.0,
                            up.info.io_alloc_bytes as f64 / 1048576.0,
                            up.info.input_content_min_scale,
                            up.info.input_content_max_scale
                        );
                        cfgs.push(Cfg {
                            name,
                            up,
                            times: vec![],
                        });
                    }
                    Err(e) => println!("{name:<34} UNSUPPORTED: {e}"),
                }
            }
        }
    }
    // Async-init creation time for comparison.
    let t0 = Instant::now();
    let _a = Upscaler::with_options(Kind::Temporal, 1728, 1080, 3456, 2160, false)?;
    println!(
        "temporal 1728x1080->3456x2160 with requiresSynchronousInitialization = false: create {:.1} ms (total new incl. textures {:.1} ms)",
        _a.info.create_ms,
        t0.elapsed().as_secs_f64() * 1e3
    );

    let mut errors = 0usize;
    let warm = 30;
    let frames = 200;
    for round in 0..3 {
        for c in cfgs.iter_mut() {
            c.times.push(Vec::with_capacity(frames));
        }
        for f in 0..warm + frames {
            for c in cfgs.iter_mut() {
                match c.up.bench_frame(jit(f as u32), false) {
                    Ok(ms) if f >= warm => c.times[round].push(ms),
                    Ok(_) => {}
                    Err(e) => {
                        errors += 1;
                        eprintln!("{}: command buffer error: {e}", c.name);
                    }
                }
            }
        }
    }
    println!("\nGPU ms per upscale (command buffer with only the scaler), {frames} frames x 3 rounds, round robin:");
    println!(
        "{:<34} {:>22} {:>22} {:>22} {:>8} {:>8} {:>8} {:>8}",
        "config", "round1 med/p90/min", "round2", "round3", "med", "p90", "min", "p10"
    );
    for c in cfgs.iter_mut() {
        let mut row = format!("{:<34}", c.name);
        let mut all = Vec::new();
        for r in c.times.iter_mut() {
            all.extend_from_slice(r);
            let (m, p, mn) = stats(r);
            row += &format!(" {:>22}", format!("{m:.3}/{p:.3}/{mn:.3}"));
        }
        let (m, p, mn) = stats(&mut all);
        let p10 = all[(all.len() - 1) / 10];
        row += &format!(" {m:>8.3} {p:>8.3} {mn:>8.3} {p10:>8.3}");
        println!("{row}");
    }

    // Batched: 10 scaler command buffers committed back to back per config (GPU busy,
    // clocks up), round robin over configs, 20 batches x 3 rounds = 600 frames each.
    let mut batched: Vec<Vec<f64>> = vec![Vec::new(); cfgs.len()];
    for _round in 0..3 {
        for b in 0..20u32 {
            for (i, c) in cfgs.iter_mut().enumerate() {
                let js: Vec<[f32; 2]> = (0..10).map(|k| jit(b * 10 + k)).collect();
                match c.up.bench_batch(&js) {
                    Ok(t) => batched[i].extend(t),
                    Err(e) => {
                        errors += 1;
                        eprintln!("{}: command buffer error (batch dropped): {e}", c.name);
                    }
                }
            }
        }
    }
    println!("\ncommand buffer errors during the bench: {errors}");
    println!("\nGPU ms per upscale, batched (10 command buffers back to back), 600 frames:");
    println!("{:<34} {:>8} {:>8} {:>8} {:>8}", "config", "med", "p90", "min", "p10");
    for (c, t) in cfgs.iter().zip(batched.iter_mut()) {
        let (m, p, mn) = stats(t);
        let p10 = t[(t.len() - 1) / 10];
        println!("{:<34} {m:>8.3} {p:>8.3} {mn:>8.3} {p10:>8.3}", c.name);
    }
    Ok(())
}

/// Creation cost in a fresh process: `create sync|async` (temporal 1728x1080 -> 3456x2160),
/// then the GPU time of the first frames (the async scaler starts on an interim upscaler).
fn create(mode: &str) -> Result<(), String> {
    if mode == "limits" {
        for (kind, iw, ih) in [
            (Kind::Temporal, 400usize, 250usize),
            (Kind::Temporal, 528, 330),
            (Kind::Temporal, 1, 1),
            (Kind::Spatial, 400, 250),
            (Kind::Spatial, 200, 125),
            (Kind::Temporal, 1600, 1000),
            (Kind::Spatial, 1600, 1000),
        ] {
            let r = Upscaler::new(kind, iw, ih, 1600, 1000);
            println!(
                "{kind:?} {iw}x{ih} -> 1600x1000 (ratio {:.2}): {}",
                1600.0 / iw as f64,
                match r {
                    Ok(_) => "created".to_string(),
                    Err(e) => format!("FAILED: {e}"),
                }
            );
        }
        return Ok(());
    }
    let sync = mode != "async";
    let t0 = Instant::now();
    let mut up = Upscaler::with_options(Kind::Temporal, 1728, 1080, 3456, 2160, sync)?;
    let total = t0.elapsed().as_secs_f64() * 1e3;
    println!(
        "temporal 1728x1080->3456x2160 requiresSynchronousInitialization={sync}: scaler create {:.1} ms, Upscaler::new {total:.1} ms",
        up.info.create_ms
    );
    println!(
        "  usage bits: color {:#x} depth {:#x} motion {:#x} output {:#x} (1 = ShaderRead, 2 = ShaderWrite, 4 = RenderTarget, 0x10 = PixelFormatView)",
        up.info.color_usage, up.info.depth_usage, up.info.motion_usage, up.info.output_usage
    );
    let mut first = Vec::new();
    let t1 = Instant::now();
    for f in 0..120u32 {
        let ms = up.bench_frame(jit(f), f == 0)?;
        if f < 10 || f % 20 == 19 {
            first.push(format!("{f}:{ms:.2}"));
        }
    }
    println!(
        "  GPU ms by frame: {}  (120 frames in {:.0} ms wall)",
        first.join(" "),
        t1.elapsed().as_secs_f64() * 1e3
    );
    Ok(())
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let r = match arg.as_str() {
        "test" => test(),
        "bench" => bench(),
        "create" => create(&std::env::args().nth(2).unwrap_or_default()),
        _ => Err("usage: metalfx-probe test|bench|create sync|async".into()),
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
