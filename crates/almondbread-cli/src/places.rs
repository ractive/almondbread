//! `almondbread places`: the places catalogue (kb: synthesis/places).
//!
//! ```text
//! almondbread places [--format text|tsv|landscape] [--category c] [--catalog multibrot3|all]
//! almondbread places --render-thumbs dir [--only a,b] [--size 320x200] [--ss 3] [--oversample 3] [--blur 0.5] [--image heic|jpeg] [--quality 70]
//! ```
//!
//! `--render-thumbs` writes `<id>.heic` at `--size` and `<id>@2x.heic` at twice that, each in
//! the place's look with `--ss` supersampling, rendered `--oversample` times larger and
//! box-filtered down in linear light (the needle minibrots' ring bands are far finer than a
//! thumbnail pixel: 9 samples per pixel left moiré, 81 do not) (HEIC or JPEG through macOS `sips`; HEIC is
//! about half the size of JPEG for this much fine detail), and fails if a thumbnail is flat
//! (all set or all outside). `--format landscape` prints the
//! `prototypes/landscape-rt` start options of the places f64 reaches.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use almondbread_core::image::write_png_bgra;
use almondbread_core::places::{Place, PlaceCategory, all_places, places, places_for};
use almondbread_render::{GpuBackend, Renderer};

use crate::options::{Options, Result, err, parse_size};

pub fn places_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    let category = match o.get("category") {
        None => None,
        Some(c) => match PlaceCategory::ALL.into_iter().find(|k| k.id() == c) {
            Some(k) => Some(k),
            None => return err(format!("unknown --category {c}")),
        },
    };
    let only: Option<Vec<&str>> = o.get("only").map(|v| v.split(',').map(str::trim).collect());
    // F3: `--catalog multibrotP` lists (and renders) that power's places, `--catalog all` every
    // formula's; the default is the Mandelbrot catalogue.
    let from: Vec<&Place> = match o.get("catalog") {
        None => places().iter().collect(),
        Some("all") => all_places().collect(),
        Some(f) => match almondbread_core::formula::Formula::parse(f) {
            Some(f) => places_for(f),
            None => return err(format!("unknown --catalog {f}")),
        },
    };
    let list: Vec<&Place> = from
        .into_iter()
        .filter(|p| category.is_none_or(|c| p.category == c))
        .filter(|p| only.as_ref().is_none_or(|ids| ids.contains(&p.id)))
        .collect();
    if let Some(ids) = &only {
        for id in ids {
            if !list.iter().any(|p| p.id == *id) {
                return err(format!("unknown place {id}"));
            }
        }
    }
    if let Some(dir) = o.get("render-thumbs") {
        return render_thumbs(backend, o, &list, dir);
    }
    match o.get("format").unwrap_or("text") {
        "text" => {
            for p in &list {
                println!(
                    "{:<28} {:<13} 1e{:<7.1} {:<9} {}{}",
                    p.id,
                    p.category.id(),
                    p.zoom_log10(),
                    p.look,
                    p.name,
                    if p.generated { "  (generated)" } else { "" }
                );
            }
            println!("{} places", list.len());
        }
        "tsv" => {
            println!("id\tname\tcategory\tre\tim\tspan\tlook\tsource\turl");
            for p in &list {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    p.id,
                    p.name,
                    p.category.id(),
                    p.re,
                    p.im,
                    p.span,
                    p.look,
                    p.source,
                    p.source_url
                );
            }
        }
        "landscape" => {
            // f64 centres: the prototype parses `--loc re,im` as f64.
            for p in &list {
                if let Some(s) = p.landscape_scale() {
                    let c = p.center();
                    println!(
                        "{:<28} cargo run --release -- fly --loc={:.17},{:.17} --scale {s:.3e}",
                        p.id,
                        c.re(),
                        c.im()
                    );
                }
            }
        }
        f => return err(format!("unknown --format {f}")),
    }
    Ok(0)
}

fn render_thumbs<B: GpuBackend + Clone>(backend: &B, o: &Options, list: &[&Place], dir: &str) -> Result<i32> {
    let (w, h) = parse_size(o.get("size").unwrap_or("320x200"))?;
    let ss = o.get("ss").unwrap_or("3").to_string();
    let over = o.usize("oversample").unwrap_or(3).clamp(1, 8);
    let blur = o.f64("blur").unwrap_or(0.5).max(0.0) as f32;
    let quality = o.usize("quality").unwrap_or(70).clamp(1, 100);
    let (format, ext) = match o.get("image").unwrap_or("heic") {
        "heic" => ("heic", "heic"),
        "jpeg" | "jpg" => ("jpeg", "jpg"),
        f => return err(format!("unknown --image {f} (heic or jpeg)")),
    };
    std::fs::create_dir_all(dir)?;
    let mut status = 0;
    for p in list {
        let t0 = Instant::now();
        let mut notes = Vec::new();
        for (suffix, scale) in [("", 1), ("@2x", 2)] {
            let (pw, ph) = (w * scale, h * scale);
            let po = o.with("place", p.id).with("ss", &ss);
            let mut r = Renderer::new(backend.clone())?;
            po.apply_settings(&mut r.settings);
            po.apply_renderer(&mut r);
            let (rw, rh) = (pw * over, ph * over);
            r.viewport = p.viewport(rw, rh);
            let out = r.render_offscreen(rw, rh, true)?;
            let pixels = downsample_bgra(&out.pixels, rw, rh, over, blur);
            if scale == 1 {
                // Reject empty thumbnails: all one colour means the view missed the set.
                let (lo, hi) = luma_range(&pixels);
                if hi - lo < 40.0 {
                    notes.push(format!("FLAT (luma {lo:.0}..{hi:.0})"));
                    status = 1;
                }
                notes.push(format!("maxIter {}", r.stats.max_iterations));
            }
            let png = Path::new(dir).join(format!("{}{suffix}.png", p.id));
            let image = Path::new(dir).join(format!("{}{suffix}.{ext}", p.id));
            write_png_bgra(png.to_str().unwrap_or_default(), &pixels, pw, ph)?;
            let ok = Command::new("sips")
                .args(["-s", "format", format, "-s", "formatOptions", &quality.to_string()])
                .arg(&png)
                .arg("--out")
                .arg(&image)
                .output()
                .map(|s| s.status.success())
                .unwrap_or(false);
            if !ok {
                return err("sips failed (image conversion needs macOS)");
            }
            if o.get("keep-png") != Some("1") {
                std::fs::remove_file(&png)?;
            }
        }
        println!(
            "{:<28} 1e{:<7.1} {:.2} s  {}",
            p.id,
            p.zoom_log10(),
            t0.elapsed().as_secs_f64(),
            notes.join("  ")
        );
    }
    Ok(status)
}

/// Resamples a BGRA (sRGB) image `k` times smaller in each direction with a Gaussian of
/// `sigma` output pixels (separable, in linear light). A box filter keeps bands near the
/// output Nyquist rate as beats (moiré); sigma 0.5 damps them to under a third.
fn downsample_bgra(bgra: &[u8], w: usize, h: usize, k: usize, sigma: f32) -> Vec<u8> {
    if k == 1 && sigma <= 0.0 {
        return bgra.to_vec();
    }
    let to_lin: Vec<f32> = (0..256)
        .map(|v| {
            let c = v as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
        .collect();
    let to_srgb = |l: f32| {
        let c = if l <= 0.0031308 {
            l * 12.92
        } else {
            1.055 * l.powf(1.0 / 2.4) - 0.055
        };
        (c * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let (ow, oh) = (w / k, h / k);
    // Taps of one output pixel in source pixels: a box of k convolved with the Gaussian.
    let kf = k as f32;
    let radius = (3.0 * sigma * kf).ceil() as isize + k as isize;
    let taps = |o: usize, n: usize| -> Vec<(usize, f32)> {
        let centre = (o as f32 + 0.5) * kf;
        let lo = (centre as isize - radius).max(0);
        let hi = (centre as isize + radius).min(n as isize - 1);
        let mut t: Vec<(usize, f32)> = (lo..=hi)
            .map(|i| {
                let d = (i as f32 + 0.5 - centre) / kf; // output pixels
                let wgt = if sigma > 0.0 {
                    (-0.5 * (d / sigma).powi(2)).exp()
                } else if d.abs() < 0.5 {
                    1.0
                } else {
                    0.0
                };
                (i as usize, wgt)
            })
            .filter(|&(_, w)| w > 1e-4)
            .collect();
        let sum: f32 = t.iter().map(|&(_, w)| w).sum();
        for x in &mut t {
            x.1 /= sum;
        }
        t
    };
    let lin: Vec<[f32; 4]> = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            [
                to_lin[p[0] as usize],
                to_lin[p[1] as usize],
                to_lin[p[2] as usize],
                p[3] as f32,
            ]
        })
        .collect();
    // Horizontal pass: h rows of ow pixels.
    let mut mid = vec![[0.0f32; 4]; ow * h];
    for x in 0..ow {
        let t = taps(x, w);
        for y in 0..h {
            let mut acc = [0.0f32; 4];
            for &(i, wgt) in &t {
                let p = lin[y * w + i];
                for c in 0..4 {
                    acc[c] += p[c] * wgt;
                }
            }
            mid[y * ow + x] = acc;
        }
    }
    let mut out = vec![0u8; ow * oh * 4];
    for y in 0..oh {
        let t = taps(y, h);
        for x in 0..ow {
            let mut acc = [0.0f32; 4];
            for &(i, wgt) in &t {
                let p = mid[i * ow + x];
                for c in 0..4 {
                    acc[c] += p[c] * wgt;
                }
            }
            let o = (y * ow + x) * 4;
            for c in 0..3 {
                out[o + c] = to_srgb(acc[c]);
            }
            out[o + 3] = acc[3].round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Lowest and highest luma (1st and 99th percentile) of a BGRA image.
fn luma_range(bgra: &[u8]) -> (f64, f64) {
    let mut l: Vec<f64> = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| 0.0722 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.2126 * p[2] as f64)
        .collect();
    if l.is_empty() {
        return (0.0, 0.0);
    }
    l.sort_by(f64::total_cmp);
    (l[l.len() / 100], l[l.len() * 99 / 100])
}

#[cfg(test)]
mod tests {
    use super::downsample_bgra;

    #[test]
    fn downsampling_averages_in_linear_light() {
        // Black and white texels average to linear 0.5, sRGB 188.
        let img = [0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255];
        let out = downsample_bgra(&img, 2, 2, 2, 0.0);
        assert_eq!(out, vec![188, 188, 188, 255]);
        assert_eq!(downsample_bgra(&img, 2, 2, 1, 0.0), img.to_vec());
        // A flat image stays flat under the Gaussian.
        let flat = [90u8, 120, 200, 255].repeat(36);
        assert_eq!(downsample_bgra(&flat, 6, 6, 3, 0.5), [90u8, 120, 200, 255].repeat(4));
    }
}
