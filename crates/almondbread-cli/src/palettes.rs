//! `almondbread palettes`: previews of the palette library (kb: techniques/palette-design).
//!
//! ```text
//! almondbread palettes --out dir [--views seahorse,antenna,home,name:re:im:width] [--thumb 256x160]
//!     [--cols 6] [--only id,id] [--import a.ugr,b.map,c.json] [--ss 2] [--plain 1] [--list 1]
//! ```
//!
//! Writes `strips.png` (every palette as a labelled strip, by category) and one
//! `sheet-<view>.png` per view: a thumbnail per palette, rendered by the real renderer with
//! the app's default look and the palette's own density, offset and deep colour.

use std::fs;

use almondbread_core::image::write_png_bgra;
use almondbread_core::palette_import::{parse_map, parse_ugr};
use almondbread_core::palette_library::{self, Category, LibraryPalette};
use almondbread_core::settings::{RenderSettings, Supersampling};
use almondbread_core::viewport::Viewport;
use almondbread_render::{GpuBackend, Renderer};

use crate::options::{Options, Result, err};

/// A preview view: name, centre, complex width.
struct View {
    name: String,
    re: f64,
    im: f64,
    width: f64,
}

fn views(o: &Options) -> Result<Vec<View>> {
    let spec = o.get("views").unwrap_or("seahorse,antenna");
    let mut out = Vec::new();
    for v in spec.split(',').filter(|s| !s.is_empty()) {
        let view = match v {
            // Seahorse valley spirals at zoom ~3e4 (the seahorse-1e5 benchmark centre).
            "seahorse" => View {
                name: "seahorse".into(),
                re: -0.743643887037151,
                im: 0.13182590420533,
                width: 1.3e-4,
            },
            // The user's -1.7687 view: 1.875e-6 per point at 850 points wide.
            "antenna" => View {
                name: "antenna".into(),
                re: -1.7686774605423412,
                im: 0.00423684791873126,
                width: 1.875e-6 * 850.0,
            },
            // The whole set.
            "home" => View {
                name: "home".into(),
                re: -0.75,
                im: 0.0,
                width: 3.4,
            },
            custom => {
                let Some((name, rest)) = custom.split_once(':') else {
                    return err(format!(
                        "bad view {custom} (seahorse, antenna, home or name:re:im:width)"
                    ));
                };
                let n: Vec<f64> = rest.split(':').filter_map(|x| x.parse().ok()).collect();
                if n.len() != 3 {
                    return err(format!("bad view {custom}: name:re:im:width"));
                }
                View {
                    name: name.into(),
                    re: n[0],
                    im: n[1],
                    width: n[2],
                }
            }
        };
        out.push(view);
    }
    Ok(out)
}

fn imported(o: &Options) -> Result<Vec<&'static LibraryPalette>> {
    let mut out = Vec::new();
    for path in o.get("import").unwrap_or("").split(',').filter(|s| !s.is_empty()) {
        let text = fs::read_to_string(path)?;
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported");
        let list = if path.ends_with(".map") {
            vec![parse_map(&text, stem)?]
        } else if path.ends_with(".json") {
            palette_library::parse_json(&text)?
        } else {
            parse_ugr(&text)?
        };
        out.extend(list.into_iter().map(palette_library::register_user));
    }
    Ok(out)
}

pub fn palettes_command<B: GpuBackend + Clone>(backend: &B, o: &Options) -> Result<i32> {
    let user = imported(o)?;
    let mut list: Vec<&'static LibraryPalette> = palette_library::builtin().iter().chain(user).collect();
    if let Some(only) = o.get("only") {
        let ids: Vec<&str> = only.split(',').collect();
        list.retain(|p| ids.contains(&p.id.as_str()));
    }
    if let Some(c) = o.get("category") {
        list.retain(|p| p.category.key() == c);
    }
    if o.get("list") == Some("1") {
        for p in &list {
            println!(
                "{:<14} {:<16} {:<28} {:<4} {:<6} L{:.2} {}",
                p.category.key(),
                p.id,
                p.name,
                if p.cyclic { "cyc" } else { "seq" },
                p.density,
                p.lightness_span(),
                if p.license.is_empty() { "-" } else { &p.license }
            );
        }
        return Ok(0);
    }
    let Some(dir) = o.get("out") else {
        return err("missing --out dir");
    };
    fs::create_dir_all(dir)?;
    let strips = format!("{dir}/strips.png");
    let (w, h, px) = strip_sheet(&list);
    write_png_bgra(&strips, &px, w, h)?;
    println!("{strips}: {} palettes", list.len());

    let (tw, th) = o.size("thumb", (256, 160))?;
    let cols = o.usize("cols").unwrap_or(6).max(1);
    for view in views(o)? {
        let mut r = Renderer::new(backend.clone())?;
        o.apply_renderer(&mut r);
        let mut base = if o.get("plain") == Some("1") {
            RenderSettings::plain()
        } else {
            RenderSettings::default()
        };
        base.supersampling = Supersampling::from_factor(o.usize("ss").unwrap_or(2)).unwrap_or(Supersampling::X4);
        r.viewport = Viewport::new(view.re, view.im, view.width / tw as f64);
        let mut sheet = Canvas::new(cols * tw, list.len().div_ceil(cols) * (th + LABEL_H));
        for (i, p) in list.iter().enumerate() {
            let mut s = base.clone();
            p.apply(&mut s);
            r.settings = s;
            let out = r.render_offscreen(tw, th, false)?;
            let (x0, y0) = ((i % cols) * tw, (i / cols) * (th + LABEL_H));
            sheet.blit(&out.pixels, tw, th, x0, y0);
            sheet.text(x0 + 3, y0 + th + 3, &p.name, 0xE0E0E0);
        }
        let path = format!("{dir}/sheet-{}.png", view.name);
        write_png_bgra(&path, &sheet.px, sheet.w, sheet.h)?;
        println!(
            "{path}: {} thumbnails {tw}x{th} at {} {} width {}",
            list.len(),
            view.re,
            view.im,
            view.width
        );
    }
    Ok(0)
}

const LABEL_H: usize = 13;

/// Every palette as a strip: category headings, name, two cycles of the gradient, the deep
/// colour swatch.
fn strip_sheet(list: &[&LibraryPalette]) -> (usize, usize, Vec<u8>) {
    const NAME_W: usize = 190;
    const STRIP_W: usize = 640;
    const ROW: usize = 20;
    let mut rows: Vec<Option<&LibraryPalette>> = Vec::new();
    let mut heads: Vec<(usize, Category)> = Vec::new();
    for c in Category::ALL {
        let members: Vec<&LibraryPalette> = list.iter().copied().filter(|p| p.category == c).collect();
        if members.is_empty() {
            continue;
        }
        heads.push((rows.len(), c));
        rows.push(None);
        rows.extend(members.into_iter().map(Some));
    }
    let w = NAME_W + STRIP_W + 40;
    let mut cv = Canvas::new(w, rows.len() * ROW + 6);
    for &(row, c) in &heads {
        cv.text(4, row * ROW + 8, &c.label().to_uppercase(), 0xFFD27A);
    }
    for (row, p) in rows.iter().enumerate() {
        let Some(p) = p else { continue };
        let y = row * ROW + 3;
        cv.text(12, y + 5, &p.name, 0xE0E0E0);
        let sw = p.swatch(STRIP_W / 2);
        for x in 0..STRIP_W {
            cv.fill(NAME_W + x, y, 1, ROW - 4, sw[x % sw.len()]);
        }
        cv.fill(NAME_W + STRIP_W + 12, y, 20, ROW - 4, p.deep);
    }
    (cv.w, cv.h, cv.px)
}

/// A BGRA8 canvas with a dark background.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            px: [0x18, 0x16, 0x16, 0xFF].repeat(w * h),
        }
    }

    fn set(&mut self, x: usize, y: usize, rgb: u32) {
        if x < self.w && y < self.h {
            let i = (y * self.w + x) * 4;
            self.px[i..i + 4].copy_from_slice(&[rgb as u8, (rgb >> 8) as u8, (rgb >> 16) as u8, 0xFF]);
        }
    }

    fn fill(&mut self, x: usize, y: usize, w: usize, h: usize, rgb: u32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, rgb);
            }
        }
    }

    fn blit(&mut self, bgra: &[u8], w: usize, h: usize, x0: usize, y0: usize) {
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let rgb = (u32::from(bgra[i + 2]) << 16) | (u32::from(bgra[i + 1]) << 8) | u32::from(bgra[i]);
                self.set(x0 + x, y0 + y, rgb);
            }
        }
    }

    /// Text in a 5x7 pixel font (upper case; unknown characters as blanks).
    fn text(&mut self, x: usize, y: usize, s: &str, rgb: u32) {
        for (i, c) in s.chars().enumerate() {
            let g = glyph(c.to_ascii_uppercase());
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) != 0 {
                        self.set(x + i * 6 + col, y + row, rgb);
                    }
                }
            }
        }
    }
}

fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '&' => [0x0C, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0D],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        _ => [0; 7],
    }
}
