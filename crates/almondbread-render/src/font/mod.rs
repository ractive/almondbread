//! The overlay font (kb: platform/overlay-text): Inter SemiBold 4.1 (SIL Open Font License
//! 1.1, `OFL.txt` next to this file) as a single-channel signed-distance-field atlas made by
//! `examples/gen_overlay_font.rs`. The `overlay` kernel samples it bilinearly, so text is
//! crisp at any size and backing scale, and the stored distance beyond the outline gives the
//! dark halo that keeps labels legible on bright and dark parts of the fractal.
//!
//! Layout is here (platform-neutral): advances from the font, digits on a common (tabular)
//! advance so numbers do not jitter as they change, no kerning.

// Line metrics are kept for reference; layout uses the cap height.
#[allow(dead_code, clippy::type_complexity)]
mod metrics;

pub use metrics::{ATLAS_H, ATLAS_W, EM, SPREAD};

/// The atlas: `ATLAS_W` x `ATLAS_H` bytes, 128 + d / SPREAD * 127 with d the signed distance
/// to the outline in texels (inside positive).
pub static ATLAS: &[u8] = include_bytes!("overlay_font.sdf");

/// One glyph: advance (texels) and its atlas cell with the cell's offset from the pen
/// (y down from the baseline).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub advance: f32,
    pub atlas: [f32; 4],
    pub left: f32,
    pub top: f32,
}

fn lookup(ch: char) -> Option<Glyph> {
    metrics::GLYPHS.binary_search_by_key(&ch, |g| g.0).ok().map(|i| {
        let (_, advance, x, y, w, h, left, top) = metrics::GLYPHS[i];
        Glyph {
            advance,
            atlas: [x as f32, y as f32, (x + w) as f32, (y + h) as f32],
            left: left as f32,
            top: top as f32,
        }
    })
}

/// The glyph drawn for `ch` (None: a space, advance only); characters the font lacks print
/// as `?`.
pub fn glyph(ch: char) -> Option<Glyph> {
    if ch.is_whitespace() {
        return None;
    }
    lookup(ch).or_else(|| lookup('?'))
}

fn space_advance() -> f32 {
    // The font's space is about 0.28 em; a figure space reads better between columns.
    0.3 * EM
}

/// The common advance of the digits (texels).
pub fn digit_advance() -> f32 {
    ('0'..='9').filter_map(lookup).map(|g| g.advance).fold(0.0, f32::max)
}

/// Height of the capitals above the baseline, in ems.
pub fn cap_height() -> f32 {
    lookup('H').map_or(0.73, |g| (-g.top - SPREAD) / EM)
}

/// A laid-out glyph: its quad in pixels relative to the pen start on the baseline, and its
/// atlas cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub quad: [f32; 4],
    pub atlas: [f32; 4],
}

/// Lays out `text` at `size` pixels per em; returns the glyphs and the advance width.
pub fn layout(text: &str, size: f32) -> (Vec<Placed>, f32) {
    let s = size / EM;
    let tab = digit_advance();
    let mut pen = 0.0f32;
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let Some(g) = glyph(ch) else {
            pen += space_advance() * s;
            continue;
        };
        let (adv, shift) = if ch.is_ascii_digit() {
            (tab, 0.5 * (tab - g.advance))
        } else {
            (g.advance, 0.0)
        };
        let x0 = pen + (g.left + shift) * s;
        let y0 = g.top * s;
        out.push(Placed {
            quad: [
                x0,
                y0,
                x0 + (g.atlas[2] - g.atlas[0]) * s,
                y0 + (g.atlas[3] - g.atlas[1]) * s,
            ],
            atlas: g.atlas,
        });
        pen += adv * s;
    }
    (out, pen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_matches_the_metrics() {
        assert_eq!(ATLAS.len(), (ATLAS_W * ATLAS_H) as usize);
        for g in metrics::GLYPHS {
            assert!(u32::from(g.2) + u32::from(g.4) <= ATLAS_W && u32::from(g.3) + u32::from(g.5) <= ATLAS_H);
        }
        // Sorted for the binary search.
        assert!(metrics::GLYPHS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn glyph_cells_are_inside_out() {
        // A cell's border is SPREAD texels from the glyph: far outside; the middle of an 'l'
        // stem is inside.
        let g = glyph('l').unwrap();
        let at = |x: f32, y: f32| ATLAS[(y as u32 * ATLAS_W + x as u32) as usize];
        assert_eq!(at(g.atlas[0], g.atlas[1]), 1);
        let mid = (0.5 * (g.atlas[0] + g.atlas[2]), 0.5 * (g.atlas[1] + g.atlas[3]));
        assert!(at(mid.0, mid.1) > 128, "{}", at(mid.0, mid.1));
    }

    #[test]
    fn digits_are_tabular() {
        let (_, w1) = layout("111", 20.0);
        let (_, w8) = layout("888", 20.0);
        assert_eq!(w1, w8);
        assert!(cap_height() > 0.65 && cap_height() < 0.8, "{}", cap_height());
    }
}
