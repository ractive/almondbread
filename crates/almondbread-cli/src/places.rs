//! `almondbread places`: the places catalogue (kb: synthesis/places).
//!
//! ```text
//! almondbread places [--format text|tsv|landscape] [--category c]
//! almondbread places --render-thumbs dir [--only a,b] [--size 320x200] [--ss 3] [--image heic|jpeg] [--quality 70]
//! ```
//!
//! `--render-thumbs` writes `<id>.heic` at `--size` and `<id>@2x.heic` at twice that, each in
//! the place's look with `--ss` supersampling (HEIC or JPEG through macOS `sips`; HEIC is
//! about half the size of JPEG for this much fine detail), and fails if a thumbnail is flat
//! (all set or all outside). `--format landscape` prints the
//! `prototypes/landscape-rt` start options of the places f64 reaches.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use almondbread_core::image::write_png_bgra;
use almondbread_core::places::{Place, PlaceCategory, places};
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
    let list: Vec<&Place> = places()
        .iter()
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
            r.viewport = p.viewport(pw, ph);
            let out = r.render_offscreen(pw, ph, true)?;
            if scale == 1 {
                // Reject empty thumbnails: all one colour means the view missed the set.
                let (lo, hi) = luma_range(&out.pixels);
                if hi - lo < 40.0 {
                    notes.push(format!("FLAT (luma {lo:.0}..{hi:.0})"));
                    status = 1;
                }
                notes.push(format!("maxIter {}", r.stats.max_iterations));
            }
            let png = Path::new(dir).join(format!("{}{suffix}.png", p.id));
            let image = Path::new(dir).join(format!("{}{suffix}.{ext}", p.id));
            write_png_bgra(png.to_str().unwrap_or_default(), &out.pixels, pw, ph)?;
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
