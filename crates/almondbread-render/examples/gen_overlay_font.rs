//! Generates the overlay font: a single-channel signed-distance-field atlas of Inter SemiBold
//! (SIL Open Font License 1.1, `src/font/OFL.txt`) and its metrics table.
//!
//! ```text
//! cargo run --release -p almondbread-render --example gen_overlay_font -- Inter-SemiBold.ttf
//! ```
//!
//! Writes `src/font/overlay_font.sdf` (raw R8, `ATLAS_W` wide) and `src/font/metrics.rs`.
//! Each glyph is rasterised at `OVERSAMPLE` times the atlas size, thresholded, and the exact
//! Euclidean distance (Felzenszwalb-Huttenlocher) to the other side is sampled at the atlas
//! texel centres, clamped to `SPREAD` atlas texels and stored as 128 + d / SPREAD * 127
//! (inside positive). The kernel samples it bilinearly; the 0 iso-line is the glyph outline
//! at any scale, and distances out to SPREAD give the outline halo.

use std::fmt::Write as _;

/// Atlas texels per em.
const EM: f32 = 40.0;
/// Distance range stored, in atlas texels.
const SPREAD: i32 = 8;
const OVERSAMPLE: i32 = 8;
const ATLAS_W: i32 = 512;

/// ASCII plus the symbols the overlay prints.
fn charset() -> Vec<char> {
    let mut v: Vec<char> = (0x21u8..0x7f).map(char::from).collect();
    v.extend("λ×·°…–−μπ≈→←↑↓∞".chars());
    v
}

/// One-dimensional squared distance transform (Felzenszwalb and Huttenlocher 2012).
fn edt_1d(f: &[f64], d: &mut [f64], v: &mut [usize], z: &mut [f64]) {
    let n = f.len();
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        let qf = q as f64;
        loop {
            let p = v[k];
            let pf = p as f64;
            let s = ((f[q] + qf * qf) - (f[p] + pf * pf)) / (2.0 * (qf - pf));
            if s <= z[k] {
                k -= 1;
            } else {
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = f64::INFINITY;
                break;
            }
        }
    }
    k = 0;
    for q in 0..n {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        d[q] = (q as f64 - p as f64).powi(2) + f[p];
    }
}

/// Squared distance from every pixel to the nearest pixel where `seed` is true.
fn edt_2d(seed: &[bool], w: usize, h: usize) -> Vec<f64> {
    const BIG: f64 = 1e12;
    let mut g: Vec<f64> = seed.iter().map(|&s| if s { 0.0 } else { BIG }).collect();
    let n = w.max(h);
    let (mut f, mut d, mut v, mut z) = (vec![0.0; n], vec![0.0; n], vec![0usize; n], vec![0.0; n + 1]);
    for x in 0..w {
        for y in 0..h {
            f[y] = g[y * w + x];
        }
        edt_1d(&f[..h], &mut d[..h], &mut v, &mut z);
        for y in 0..h {
            g[y * w + x] = d[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&g[y * w..(y + 1) * w]);
        edt_1d(&f[..w], &mut d[..w], &mut v, &mut z);
        g[y * w..(y + 1) * w].copy_from_slice(&d[..w]);
    }
    g
}

struct Glyph {
    ch: char,
    advance: f32,
    /// Atlas cell (texels) and its offset from the pen position (x right, y down from baseline).
    w: i32,
    h: i32,
    left: i32,
    top: i32,
    sdf: Vec<u8>,
    x: i32,
    y: i32,
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: gen_overlay_font <font.ttf>");
    let data = std::fs::read(&path).expect("read font");
    let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).expect("parse font");
    let hi_px = EM * OVERSAMPLE as f32;
    let lm = font.horizontal_line_metrics(EM).expect("line metrics");
    let mut glyphs = Vec::new();
    for ch in charset() {
        if font.lookup_glyph_index(ch) == 0 {
            eprintln!("skipping {ch:?}: not in the font");
            continue;
        }
        let (m, bitmap) = font.rasterize(ch, hi_px);
        let advance = font.metrics(ch, EM).advance_width;
        // Glyph box in hi-res pixels relative to the pen (y down): x from xmin, top at
        // -(ymin + height). Align the padded cell to atlas texels.
        let gx0 = m.xmin;
        let gy0 = -(m.ymin + m.height as i32);
        let pad = SPREAD * OVERSAMPLE;
        let left = (gx0 - pad).div_euclid(OVERSAMPLE);
        let top = (gy0 - pad).div_euclid(OVERSAMPLE);
        let right = (gx0 + m.width as i32 + pad + OVERSAMPLE - 1).div_euclid(OVERSAMPLE);
        let bottom = (gy0 + m.height as i32 + pad + OVERSAMPLE - 1).div_euclid(OVERSAMPLE);
        let (w, h) = (right - left, bottom - top);
        let (hw, hh) = ((w * OVERSAMPLE) as usize, (h * OVERSAMPLE) as usize);
        let mut inside = vec![false; hw * hh];
        let (ox, oy) = (gx0 - left * OVERSAMPLE, gy0 - top * OVERSAMPLE);
        for by in 0..m.height {
            for bx in 0..m.width {
                if bitmap[by * m.width + bx] >= 128 {
                    inside[(by + oy as usize) * hw + bx + ox as usize] = true;
                }
            }
        }
        let outside: Vec<bool> = inside.iter().map(|&b| !b).collect();
        let to_in = edt_2d(&inside, hw, hh);
        let to_out = edt_2d(&outside, hw, hh);
        let mut sdf = vec![0u8; (w * h) as usize];
        for ty in 0..h {
            for tx in 0..w {
                // The texel centre falls between hi-res pixels: average the four around it.
                let (cx, cy) = (
                    (tx * OVERSAMPLE + OVERSAMPLE / 2) as usize,
                    (ty * OVERSAMPLE + OVERSAMPLE / 2) as usize,
                );
                let mut d = 0.0f64;
                for (sx, sy) in [(cx - 1, cy - 1), (cx, cy - 1), (cx - 1, cy), (cx, cy)] {
                    let i = sy * hw + sx;
                    // Pixel-centre distances; half a pixel puts the edge between pixels.
                    d += if inside[i] {
                        to_out[i].sqrt() - 0.5
                    } else {
                        -(to_in[i].sqrt() - 0.5)
                    };
                }
                let d = d * 0.25 / OVERSAMPLE as f64;
                let v = 128.0 + (d / SPREAD as f64).clamp(-1.0, 1.0) * 127.0;
                sdf[(ty * w + tx) as usize] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
        glyphs.push(Glyph {
            ch,
            advance,
            w,
            h,
            left,
            top,
            sdf,
            x: 0,
            y: 0,
        });
    }
    // Shelf packing, tallest first.
    let mut order: Vec<usize> = (0..glyphs.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(glyphs[i].h));
    let (mut x, mut y, mut shelf) = (0, 0, 0);
    for &i in &order {
        let g = &mut glyphs[i];
        if x + g.w > ATLAS_W {
            x = 0;
            y += shelf;
            shelf = 0;
        }
        g.x = x;
        g.y = y;
        x += g.w;
        shelf = shelf.max(g.h);
    }
    let atlas_h = y + shelf;
    let mut atlas = vec![0u8; (ATLAS_W * atlas_h) as usize];
    for g in &glyphs {
        for ty in 0..g.h {
            let src = &g.sdf[(ty * g.w) as usize..((ty + 1) * g.w) as usize];
            let o = ((g.y + ty) * ATLAS_W + g.x) as usize;
            atlas[o..o + g.w as usize].copy_from_slice(src);
        }
    }
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/font");
    std::fs::write(format!("{dir}/overlay_font.sdf"), &atlas).expect("write atlas");
    let mut rs = String::new();
    writeln!(
        rs,
        "// Generated by examples/gen_overlay_font.rs from Inter SemiBold 4.1 (OFL 1.1, see OFL.txt)."
    )
    .unwrap();
    writeln!(rs, "// Do not edit.\n").unwrap();
    writeln!(rs, "/// Atlas texels per em.\npub const EM: f32 = {EM:?};").unwrap();
    writeln!(
        rs,
        "/// Distance range stored in the atlas, in texels.\npub const SPREAD: f32 = {:?};",
        SPREAD as f32
    )
    .unwrap();
    writeln!(
        rs,
        "pub const ATLAS_W: u32 = {ATLAS_W};\npub const ATLAS_H: u32 = {atlas_h};"
    )
    .unwrap();
    writeln!(rs, "/// Line metrics in texels (y up from the baseline).").unwrap();
    writeln!(
        rs,
        "pub const ASCENT: f32 = {:?};\npub const DESCENT: f32 = {:?};\npub const LINE_GAP: f32 = {:?};",
        lm.ascent, lm.descent, lm.line_gap
    )
    .unwrap();
    writeln!(
        rs,
        "\n/// (char, advance, atlas x, y, w, h, left, top): cell offset from the pen, y down."
    )
    .unwrap();
    writeln!(
        rs,
        "#[rustfmt::skip]\npub const GLYPHS: &[(char, f32, u16, u16, u16, u16, i16, i16)] = &["
    )
    .unwrap();
    let mut sorted: Vec<&Glyph> = glyphs.iter().collect();
    sorted.sort_by_key(|g| g.ch);
    for g in sorted {
        writeln!(
            rs,
            "    ({:?}, {:?}, {}, {}, {}, {}, {}, {}),",
            g.ch, g.advance, g.x, g.y, g.w, g.h, g.left, g.top
        )
        .unwrap();
    }
    writeln!(rs, "];").unwrap();
    std::fs::write(format!("{dir}/metrics.rs"), rs).expect("write metrics");
    println!(
        "{} glyphs, atlas {ATLAS_W}x{atlas_h} ({} KB)",
        glyphs.len(),
        atlas.len() / 1024
    );
}
