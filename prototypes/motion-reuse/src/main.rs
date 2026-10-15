//! R4 prototype: how many pixel-iterations per displayed gesture frame can be avoided by
//! reusing samples while zooming and panning (kb: synthesis/motion-sample-reuse).
//!
//! For a flight script (scripts/flights/*.json) the camera path is stepped with the app's
//! `Navigator` at virtual 60 Hz refreshes. Every gesture refresh, the real GPU kernel renders
//! the iteration fields each strategy needs (offscreen, deterministic: fixed limit
//! `depth_iterations`, no timing-dependent decisions), and the strategies are run on the CPU:
//!
//! - `fl`: the FL gesture compose (port of `compose` mode 0) over a jittered preview whose
//!   ratio follows a deterministic cost model of the dynamic resolution (a fixed iteration
//!   budget per frame, calibrated to a real `fly` run);
//! - `fl-full`: the same compose with a full-resolution jittered level every frame;
//! - `sc-*`: the sample cache (cache.rs) with different policies / reconstructions;
//! - `xn-*`: XaoS-like nearest reuse with a positional tolerance.
//!
//! Every `--eval-every` gesture frames a converged reference (4 x 4 grid per pixel, box) is
//! rendered for that frame and the previous one: OKLab ΔE mean / p99 against it, and flicker
//! (the "flicker" column is the temporal error: the change of a frame's error against its
//! reference from the previous frame's, warped along the motion). Costs are exact iteration
//! counts of the samples each strategy computes.
//!
//! ```text
//! motion-reuse --script s.json [--size 1280x720] [--out dir] [--eval-every 10]
//!              [--budget 1.6e8] [--c0 30] [--stills 120,300 | --still-every 150]
//!              [--from k] [--to k] [--set default|first|mfx] [--fl-ratios 0.25,0.5]
//!              [--fixed-ratio r] [--mfx-ratios 0.5,0.375 (feature "metalfx")]
//! motion-reuse --scgpu 1|2     GPU bookkeeping bench of a cache (SC_FUSED=1: one command buffer)
//! ```
//!
//! Runs used for kb (R4): `run-all.sh`; tables: `analyze.py <out-dir>`.

mod cache;
mod field;
mod fl;
mod path;
mod scgpu;
mod scgpu2;
mod shade;
mod transform;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use almondbread_core::settings::depth_iterations;
use almondbread_render::scheduler::halton;
use rayon::prelude::*;

use cache::{Cache, Policy, Recon};
use field::{Field, Gpu};
use fl::Fl;
use shade::{Rgb, Shader, lab};
use transform::Affine;

struct Args(HashMap<String, String>);

impl Args {
    fn parse() -> Self {
        let v: Vec<String> = std::env::args().skip(1).collect();
        let mut m = HashMap::new();
        let mut i = 0;
        while i < v.len() {
            if let Some(k) = v[i].strip_prefix("--") {
                let val = v.get(i + 1).cloned().unwrap_or_default();
                m.insert(k.to_string(), val);
                i += 2;
            } else {
                i += 1;
            }
        }
        Self(m)
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.0.get(k).map(String::as_str)
    }
    fn f64(&self, k: &str, d: f64) -> f64 {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
}

fn reference_image(
    gpu: &mut Gpu,
    shader: &Shader,
    vp: &almondbread_core::viewport::Viewport,
    w: usize,
    h: usize,
    max_iter: usize,
) -> (Vec<Rgb>, u64) {
    const SS: usize = 4;
    let f = gpu.field(vp, w, h, 1.0, SS, [0.5, 0.5], max_iter, 0);
    let cols: Vec<Rgb> = f
        .samples
        .par_iter()
        .map(|s| shader.shade(s.class, s.n, s.s, s.de, s.ang))
        .collect();
    let tw = f.w;
    let img = (0..w * h)
        .into_par_iter()
        .map(|p| {
            let (i, j) = (p % w, p / w);
            let mut c = [0.0f32; 3];
            for dj in 0..SS {
                for di in 0..SS {
                    let q = cols[(j * SS + dj) * tw + i * SS + di];
                    for k in 0..3 {
                        c[k] += q[k];
                    }
                }
            }
            let n = (SS * SS) as f32;
            [c[0] / n, c[1] / n, c[2] / n]
        })
        .collect();
    (img, f.cost())
}

fn shade_field(shader: &Shader, f: &Field) -> Vec<Rgb> {
    f.samples
        .par_iter()
        .map(|s| shader.shade(s.class, s.n, s.s, s.de, s.ang))
        .collect()
}

/// ΔE mean, p99 between two images.
fn errors(a: &[Rgb], b: &[Rgb]) -> (f32, f32, Vec<f32>) {
    let d: Vec<f32> = a
        .par_iter()
        .zip(b.par_iter())
        .map(|(x, y)| shade::delta_e_lab(lab(*x), lab(*y)))
        .collect();
    let mut v = d.clone();
    v.par_sort_unstable_by(|x, y| x.partial_cmp(y).unwrap());
    let mean = v.iter().map(|&x| x as f64).sum::<f64>() / v.len() as f64;
    (mean as f32, v[((v.len() - 1) as f64 * 0.99) as usize], d)
}

/// Flicker: mean ΔE between `cur` and `prev` warped onto it (bilinear), over pixels whose
/// warped position lies inside `prev`.
/// Temporal error ("flicker" in the tables): how much a frame's error against its converged
/// reference changes from the previous frame's error, warped along the motion (OKLab x 100):
/// `|(cur - ref_cur) - warp(prev - ref_prev)|`, mean over pixels covered by both. A strategy
/// whose error is stable in the image (blur, consistent aliasing that moves with the content)
/// scores low; one whose error reshuffles every frame (shimmer, twinkle, popping) scores high.
/// The warp's own resampling error cancels to first order, unlike plain frame differences.
fn temporal_error(
    prev: &[Rgb],
    prev_ref: &[Rgb],
    cur: &[Rgb],
    cur_ref: &[Rgb],
    w: usize,
    h: usize,
    tf: &Affine,
) -> f32 {
    let inv = tf.inverse();
    let (s, n) = (0..w * h)
        .into_par_iter()
        .filter_map(|p| {
            let (i, j) = (p % w, p / w);
            let q = inv.apply([i as f64 + 0.5, j as f64 + 0.5]);
            if q[0] < 0.5 || q[1] < 0.5 || q[0] > w as f64 - 0.5 || q[1] > h as f64 - 0.5 {
                return None;
            }
            let a = lab(fl::bilinear3(prev, w, h, q[0] as f32, q[1] as f32));
            let b = lab(fl::bilinear3(prev_ref, w, h, q[0] as f32, q[1] as f32));
            let c = lab(cur[p]);
            let d = lab(cur_ref[p]);
            let e = [
                (c[0] - d[0]) - (a[0] - b[0]),
                (c[1] - d[1]) - (a[1] - b[1]),
                (c[2] - d[2]) - (a[2] - b[2]),
            ];
            Some(100.0 * ((e[0] * e[0] + e[1] * e[1] + e[2] * e[2]) as f64).sqrt())
        })
        .fold(|| (0.0f64, 0usize), |a, x| (a.0 + x, a.1 + 1))
        .reduce(|| (0.0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    (s / n.max(1) as f64) as f32
}

/// Plain flicker: ΔE between `cur` and `prev` warped onto it.
fn flicker(prev: &[Rgb], cur: &[Rgb], w: usize, h: usize, tf: &Affine) -> f32 {
    let inv = tf.inverse();
    let (s, n) = (0..w * h)
        .into_par_iter()
        .filter_map(|p| {
            let (i, j) = (p % w, p / w);
            let q = inv.apply([i as f64 + 0.5, j as f64 + 0.5]);
            if q[0] < 0.5 || q[1] < 0.5 || q[0] > w as f64 - 0.5 || q[1] > h as f64 - 0.5 {
                return None;
            }
            let c = fl::bilinear3(prev, w, h, q[0] as f32, q[1] as f32);
            Some(shade::delta_e_lab(lab(c), lab(cur[p])) as f64)
        })
        .fold(|| (0.0f64, 0usize), |a, x| (a.0 + x, a.1 + 1))
        .reduce(|| (0.0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    (s / n.max(1) as f64) as f32
}

fn write_png(path: &PathBuf, img: &[Rgb], w: usize, h: usize) {
    let mut bgra = Vec::with_capacity(w * h * 4);
    for c in img {
        bgra.extend_from_slice(&[shade::srgb8(c[2]), shade::srgb8(c[1]), shade::srgb8(c[0]), 255]);
    }
    almondbread_core::image::write_png_bgra(path, &bgra, w, h).expect("png");
}

#[derive(Default, Clone)]
struct Totals {
    frames: u64,
    cost: f64,
    new_samples: f64,
    evals: u64,
    de_mean: f64,
    de_p99: f64,
    flicker: f64,
    cached: f64,
}

fn main() {
    let args = Args::parse();
    if args.get("scgpu").is_some() {
        if args.get("scgpu") == Some("2") {
            almondbread_render::metal::MetalBackend::pool(scgpu2::run);
        } else {
            almondbread_render::metal::MetalBackend::pool(scgpu::run);
        }
        return;
    }
    let script_path = args.get("script").expect("--script");
    let script = path::Script::load(script_path);
    let size = args.get("size").and_then(path::parse_size).unwrap_or((1280, 720));
    let (w, h) = size;
    let name = std::path::Path::new(script_path)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let out = PathBuf::from(args.get("out").unwrap_or("out")).join(&name);
    std::fs::create_dir_all(&out).unwrap();
    let eval_every = args.f64("eval-every", 10.0) as usize;
    let budget_1080 = args.f64("budget", 6e7);
    let c0 = args.f64("c0", 30.0);
    let still_every = args.f64("still-every", 0.0) as usize;
    let fixed_ratio = args.get("fixed-ratio").and_then(|v| v.parse::<f64>().ok());
    let from = args.f64("from", 0.0) as usize;
    let to = args.f64("to", 1e9) as usize;
    let stills: Vec<usize> = args
        .get("stills")
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    let shader = Shader::default_look();
    let mut gpu = Gpu::new();
    let frames = path::camera_path(&mut gpu.r, &script, size, 60.0);
    // Path-only renderer state must not leak into the fields.
    gpu.r.navigator = Default::default();

    // FL variants: the dynamic-resolution model ("fl"), full resolution, fixed ratios.
    let mut fls: Vec<(String, Fl, Option<f64>)> = vec![
        ("fl".into(), Fl::new(w, h), None),
        ("fl-full".into(), Fl::new(w, h), Some(1.0)),
    ];
    for r in args
        .get("fl-ratios")
        .unwrap_or("")
        .split(',')
        .filter_map(|x| x.parse::<f64>().ok())
    {
        fls.push((format!("fl-r{r}"), Fl::new(w, h), Some(r)));
    }
    let set = args.get("set").unwrap_or("default");
    let mut caches: Vec<Cache> = Vec::new();
    let mut add = |name: &str, kmax: usize, policy: Policy, recon: Recon, mult: f64| {
        let mut c = Cache::new(name, w, h, kmax, policy, recon);
        c.budget_mult = mult;
        caches.push(c);
    };
    if set == "first" {
        add("sc-fill1-gauss", 4, Policy::Fill(1), Recon::Gauss, 1.0);
        add("sc-fill1-box", 4, Policy::Fill(1), Recon::Box, 1.0);
        add("sc-fill2-gauss", 4, Policy::Fill(2), Recon::Gauss, 1.0);
        add("sc-budget", 8, Policy::Budget, Recon::Gauss, 1.0);
        add("xn-0.5", 2, Policy::Nearest(0.5), Recon::Nearest, 1.0);
        add("xn-0.25", 2, Policy::Nearest(0.25), Recon::Nearest, 1.0);
    } else if set == "mfx" {
        add("hy-b1", 4, Policy::Budget, Recon::Hybrid, 1.0);
    } else {
        add("sc-fill1", 4, Policy::Fill(1), Recon::Gauss, 1.0);
        add("sc-fill2", 4, Policy::Fill(2), Recon::Gauss, 1.0);
        for m in [0.5, 1.0, 2.0] {
            add(&format!("sc-b{m}"), 4, Policy::Budget, Recon::Gauss, m);
        }
        add("hy-fill1", 4, Policy::Fill(1), Recon::Hybrid, 1.0);
        for m in [0.5, 1.0] {
            add(&format!("hy-b{m}"), 4, Policy::Budget, Recon::Hybrid, m);
        }
        add("xn-0.5", 2, Policy::Nearest(0.5), Recon::Nearest, 1.0);
    }
    // MetalFX upscalers on the same jittered preview (feature "metalfx").
    #[cfg(feature = "metalfx")]
    let mut mfx: Vec<(String, metalfx_probe::Upscaler, f64)> = args
        .get("mfx-ratios")
        .unwrap_or("")
        .split(',')
        .filter_map(|x| x.parse::<f64>().ok())
        .flat_map(|r| {
            let (iw, ih) = ((w as f64 * r).round() as usize, (h as f64 * r).round() as usize);
            [
                (format!("mfx-t{r}"), metalfx_probe::Kind::Temporal),
                (format!("mfx-s{r}"), metalfx_probe::Kind::Spatial),
            ]
            .into_iter()
            .map(move |(n, kind)| (n, metalfx_probe::Upscaler::new(kind, iw, ih, w, h).expect("metalfx"), r))
        })
        .collect();
    #[cfg(feature = "metalfx")]
    let mut mfx_reset = true;
    let xn = |c: &Cache| matches!(c.policy, Policy::Nearest(_));
    let mut names: Vec<String> = fls.iter().map(|f| f.0.clone()).collect();
    #[cfg(feature = "metalfx")]
    names.extend(mfx.iter().map(|m| m.0.clone()));
    names.extend(caches.iter().map(|c| c.name.clone()));
    names.push("reference".into());
    let mut totals: Vec<Totals> = vec![Totals::default(); names.len()];
    let mut prev_imgs: Vec<Option<Vec<Rgb>>> = vec![None; names.len()];
    let mut csv = String::from("k,t,zoom,max_iter,ratio,strategy,cost,new_samples,cached,de_mean,de_p99,flicker\n");
    let mut prev_vp = frames[0].viewport.clone();
    let mut in_gesture = false;
    let mut mean_cost = 0.0f64;
    let mut gesture_index = 0usize;
    let mut ref_prev: Option<(usize, Vec<Rgb>)> = None;
    let t_start = std::time::Instant::now();
    let mut ratios = Vec::new();
    for f in &frames {
        if f.k < from || f.k > to {
            prev_vp = f.viewport.clone();
            continue;
        }
        if !f.interacting {
            in_gesture = false;
            prev_vp = f.viewport.clone();
            continue;
        }
        let k = f.k as u32;
        let max_iter = depth_iterations(&f.viewport);
        if !in_gesture {
            // The resting view before the gesture: its exact centre-sample frame (the app's
            // refined frame) seeds every strategy.
            let lim0 = depth_iterations(&prev_vp);
            let c = gpu.field(&prev_vp, w, h, 1.0, 1, [0.5, 0.5], lim0, k.saturating_sub(1));
            let img = shade_field(&shader, &c);
            for f in fls.iter_mut() {
                f.1.reset(&img, fl::CONF_CAP);
            }
            for cch in caches.iter_mut() {
                cch.reset(&c);
            }
            mean_cost = c.cost() as f64 / (w * h) as f64;
            for p in prev_imgs.iter_mut() {
                *p = Some(img.clone());
            }
            in_gesture = true;
            #[cfg(feature = "metalfx")]
            {
                mfx_reset = true;
            }
            ref_prev = None;
        }
        let tf = Affine::between(&prev_vp, &f.viewport, w, h);
        let ti = (f.k % 64 + 1) as u32;
        let jitter = [halton(ti, 2), halton(ti, 3)];
        // Dynamic resolution model: the largest 1/8 step whose iterations fit the budget.
        let fit = (budget_1080 / ((mean_cost + c0) * 1920.0 * 1080.0)).sqrt();
        let ratio = match fixed_ratio {
            Some(r) => r,
            None => ((fit * 8.0).floor() / 8.0).clamp(0.125, 1.0),
        };
        ratios.push(ratio);
        let full = gpu.field(&f.viewport, w, h, 1.0, 1, jitter, max_iter, k);
        let centre = gpu.field(&f.viewport, w, h, 1.0, 1, [0.5, 0.5], max_iter, k);
        let preview = if ratio < 1.0 {
            gpu.field(&f.viewport, w, h, ratio, 1, jitter, max_iter, k)
        } else {
            gpu.field(&f.viewport, w, h, 1.0, 1, jitter, max_iter, k)
        };
        let fl_cost = preview.cost();
        mean_cost = fl_cost as f64 / preview.samples.len() as f64;

        let mut imgs: Vec<Vec<Rgb>> = Vec::with_capacity(names.len());
        let mut costs: Vec<(u64, u64, u64)> = Vec::new();
        for (_, flx, fixed) in fls.iter_mut() {
            let field_owned;
            let pv: &Field = match fixed {
                None => &preview,
                Some(r) if *r >= 1.0 => &full,
                Some(r) => {
                    field_owned = gpu.field(&f.viewport, w, h, *r, 1, jitter, max_iter, k);
                    &field_owned
                }
            };
            imgs.push(flx.frame(&shader, &tf, Some(pv)));
            costs.push((pv.cost(), pv.samples.len() as u64, 0));
        }
        #[cfg(feature = "metalfx")]
        {
            let inv = tf.inverse();
            for (_, up, r) in mfx.iter_mut() {
                let pv = gpu.field(&f.viewport, w, h, *r, 1, jitter, max_iter, k);
                let cols = shade_field(&shader, &pv);
                let motion = metalfx_probe::affine_motion(pv.w, pv.h, inv.a, [inv.b[0] * *r, inv.b[1] * *r]);
                let (img, _ms) = up.upscale(&cols, Some(&motion), jitter, mfx_reset).expect("upscale");
                imgs.push(img);
                costs.push((pv.cost(), pv.samples.len() as u64, 0));
            }
            mfx_reset = false;
        }
        for cch in caches.iter_mut() {
            let cand = if xn(cch) { &centre } else { &full };
            let (img, st) = cch.frame(k, &tf, max_iter, cand, fl_cost, &shader);
            imgs.push(img);
            costs.push((st.cost, st.new_samples, st.cached));
        }

        let eval = gesture_index % eval_every == 0 && gesture_index > 0;
        let mut evals: Vec<Option<(f32, f32, f32)>> = vec![None; names.len()];
        let still = stills.contains(&f.k) || (still_every > 0 && gesture_index % still_every == 0);
        if eval {
            let (rimg, _) = reference_image(&mut gpu, &shader, &f.viewport, w, h, max_iter);
            let rprev = match ref_prev.take() {
                Some((kk, img)) if kk + 1 == f.k => img,
                _ => reference_image(&mut gpu, &shader, &prev_vp, w, h, depth_iterations(&prev_vp)).0,
            };
            let rfl = flicker(&rprev, &rimg, w, h, &tf);
            for (si, img) in imgs.iter().enumerate() {
                let (m, p99, d) = errors(img, &rimg);
                let flk = prev_imgs[si]
                    .as_ref()
                    .map_or(f32::NAN, |p| temporal_error(p, &rprev, img, &rimg, w, h, &tf));
                evals[si] = Some((m, p99, flk));
                if still {
                    write_png(&out.join(format!("k{:04}-{}.png", f.k, names[si])), img, w, h);
                    let heat: Vec<Rgb> = d
                        .iter()
                        .map(|&x| {
                            let t = (x / 15.0).clamp(0.0, 1.0);
                            [
                                (3.0 * t).min(1.0),
                                (3.0 * t - 1.0).clamp(0.0, 1.0),
                                (3.0 * t - 2.0).clamp(0.0, 1.0),
                            ]
                        })
                        .collect();
                    write_png(&out.join(format!("k{:04}-{}-heat.png", f.k, names[si])), &heat, w, h);
                }
            }
            let ri = names.len() - 1;
            evals[ri] = Some((0.0, 0.0, rfl));
            if still {
                write_png(&out.join(format!("k{:04}-reference.png", f.k)), &rimg, w, h);
            }
            ref_prev = Some((f.k, rimg));
        }
        for si in 0..names.len() {
            let t = &mut totals[si];
            if si < costs.len() {
                t.frames += 1;
                t.cost += costs[si].0 as f64;
                t.new_samples += costs[si].1 as f64;
                t.cached += costs[si].2 as f64;
            }
            if let Some((m, p, fk)) = evals[si] {
                t.evals += 1;
                t.de_mean += m as f64;
                t.de_p99 += p as f64;
                t.flicker += fk as f64;
            }
            let (c, n, cached) = costs.get(si).copied().unwrap_or((0, 0, 0));
            let (m, p, fk) = evals[si].map_or((String::new(), String::new(), String::new()), |(m, p, fk)| {
                (format!("{m:.3}"), format!("{p:.2}"), format!("{fk:.3}"))
            });
            let _ = writeln!(
                csv,
                "{},{:.4},{:.4e},{},{},{},{},{},{},{},{},{}",
                f.k,
                f.t,
                f.viewport.zoom(),
                max_iter,
                ratio,
                names[si],
                c,
                n,
                cached,
                m,
                p,
                fk
            );
        }
        for (si, img) in imgs.into_iter().enumerate() {
            prev_imgs[si] = Some(img);
        }
        gesture_index += 1;
        prev_vp = f.viewport.clone();
        if gesture_index % 30 == 0 {
            eprintln!(
                "{name}: frame {} zoom {:.3e} ratio {ratio} fl cost {:.2e} [{:.0} s, fields {:.0} s wall {:.1} s gpu]",
                f.k,
                f.viewport.zoom(),
                fl_cost as f64,
                t_start.elapsed().as_secs_f64(),
                gpu.wall,
                gpu.gpu
            );
        }
    }
    std::fs::write(out.join("frames.csv"), &csv).unwrap();
    let fl_cost = totals[0].cost / totals[0].frames.max(1) as f64;
    let mut summary = format!(
        "flight {name} {w}x{h}: {} gesture frames, {} evaluated; mean FL preview ratio {:.3}; budget {budget_1080:.2e}/1080p frame, c0 {c0}\n",
        totals[0].frames,
        totals[0].evals,
        ratios.iter().sum::<f64>() / ratios.len().max(1) as f64
    );
    let _ = writeln!(
        summary,
        "{:<16} {:>12} {:>9} {:>11} {:>9} {:>8} {:>8} {:>8}",
        "strategy", "iter/frame", "vs FL", "new/frame", "cached", "dE mean", "dE p99", "flicker"
    );
    for (si, t) in totals.iter().enumerate() {
        let fr = t.frames.max(1) as f64;
        let ev = t.evals.max(1) as f64;
        let _ = writeln!(
            summary,
            "{:<16} {:>12.3e} {:>9.3} {:>11.0} {:>9.0} {:>8.3} {:>8.2} {:>8.3}",
            names[si],
            t.cost / fr,
            (t.cost / fr) / fl_cost,
            t.new_samples / fr,
            t.cached / fr,
            t.de_mean / ev,
            t.de_p99 / ev,
            t.flicker / ev
        );
    }
    print!("{summary}");
    std::fs::write(out.join("summary.txt"), &summary).unwrap();
}
