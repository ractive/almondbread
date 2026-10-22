//! Cyclic colour gradients and "looks" (FL, kb: techniques/coloring-methods).
//!
//! A palette is a list of sRGB stops; the gradient wraps from the last stop back to the
//! first. Stops are joined by a periodic Catmull-Rom spline in **OKLab**, a perceptual space
//! (Björn Ottosson 2020): equal steps look like equal changes, hue blends stay saturated
//! instead of going grey through sRGB's middle, and the curve has no kinks at the stops (a
//! kink shows as a visible line in smooth exteriors). The table is linear RGB for the GPU.
//!
//! A look is a palette plus the colouring parameters that make it work: density, lighting,
//! highlight, glow, outline and interior colour. The inspector applies a look in one step.

use crate::settings::RenderSettings;

/// A palette: `(position 0..1, sRGB hex)` stops; the gradient wraps from the last stop back
/// to the first.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    pub stops: Vec<(f64, u32)>,
}

fn evenly(colors: &[u32]) -> Vec<(f64, u32)> {
    colors
        .iter()
        .enumerate()
        .map(|(i, &c)| (i as f64 / colors.len() as f64, c))
        .collect()
}

impl Palette {
    /// The palettes, in menu order (the first is the default's).
    pub fn all() -> Vec<Palette> {
        let p = |id, name, stops: Vec<(f64, u32)>| Palette { id, name, stops };
        vec![
            // Night sky to teal, mint and pale gold, then coral and violet back into the night.
            p(
                "aurora",
                "Aurora",
                evenly(&[0x040A1F, 0x0B3B8C, 0x0E9AA7, 0x9EE6B8, 0xF6E7A8, 0xF08A5D, 0x7A2E8A]),
            ),
            // The Ultra Fractal-style default (navy, sky, white, amber), refined: OKLab joins.
            p(
                "classic",
                "Classic",
                vec![
                    (0.0, 0x000764),
                    (0.16, 0x206BCB),
                    (0.42, 0xEDFFFF),
                    (0.6425, 0xFFAA00),
                    (0.8575, 0x000200),
                ],
            ),
            // Embers: deep red through orange to a pale flame, and back down.
            p(
                "ember",
                "Ember",
                evenly(&[
                    0x0D0103, 0x5C0A0E, 0xB3200A, 0xF26B0F, 0xFFC66D, 0xFFF4D6, 0xC2410C, 0x3B0608,
                ]),
            ),
            // Deep water, teal, sea glass and foam.
            p(
                "lagoon",
                "Lagoon",
                evenly(&[0x00121C, 0x004E64, 0x0A8A8F, 0x7CCFB8, 0xE6F7EC, 0x5FA8C9, 0x0D3B66]),
            ),
            // Violet and magenta gas with pale pink light.
            p(
                "nebula",
                "Nebula",
                evenly(&[0x0E0026, 0x3A0B6E, 0x7B2CBF, 0xC77DFF, 0xFFD6F0, 0xFF5D8F, 0x5A0F6E]),
            ),
            // Dusk: indigo, rose, apricot and warm sand.
            p(
                "dusk",
                "Dusk",
                evenly(&[0x140A2E, 0x4A1D6B, 0xA8326E, 0xF06B54, 0xFFC27A, 0xFFF0C9, 0x6E3A8A]),
            ),
            // Moss and lichen with a gold highlight and bark.
            p(
                "moss",
                "Moss",
                evenly(&[0x07140C, 0x1B4332, 0x3E8E5C, 0xA7D88C, 0xF2E8C4, 0xB5782F, 0x3D2A14]),
            ),
            // Ice: midnight blue to frost and white.
            p(
                "ice",
                "Ice",
                evenly(&[0x020617, 0x15325C, 0x4B83B8, 0xBFE3FF, 0xFFFFFF, 0x89B6DE, 0x1F3A63]),
            ),
            // Graphite: neutral greys with a warm paper white.
            p(
                "graphite",
                "Graphite",
                evenly(&[0x08080A, 0x34343B, 0x8C8C94, 0xF3F0E8, 0x6A6A72, 0x1C1C21]),
            ),
            // A hue wheel at even OKLCH lightness and chroma (a rainbow without the bright and
            // dark bands of HSV), rising out of a deep indigo base so far exteriors stay dark.
            Palette {
                id: "spectrum",
                name: "Spectrum",
                stops: std::iter::once((0.0, 0x0A0820))
                    .chain((0..10).map(|i| {
                        let h = 260.0 + 36.0 * f64::from(i);
                        (0.12 + 0.08 * f64::from(i), oklch_hex(0.72, 0.14, h))
                    }))
                    .collect(),
            },
            // The earlier simple palettes, kept for old settings.
            p(
                "fire",
                "Fire (simple)",
                vec![
                    (0.0, 0x000000),
                    (0.25, 0x7A0000),
                    (0.5, 0xFF5A00),
                    (0.75, 0xFFE070),
                    (0.9, 0xFFFFFF),
                ],
            ),
            p("grey", "Greyscale", vec![(0.0, 0x000000), (0.5, 0xFFFFFF)]),
        ]
    }

    pub fn default_palette() -> Palette {
        Self::named(Look::default_look().palette)
    }

    /// The palette with `id`, or the default. Ids of removed palettes map to their closest
    /// successor.
    pub fn named(id: &str) -> Palette {
        let id = match id {
            "ocean" => "lagoon",
            "electric" => "nebula",
            "forest" => "moss",
            "rainbow" => "spectrum",
            other => other,
        };
        Self::all()
            .into_iter()
            .find(|p| p.id == id)
            // Palette library and user palettes (kb: techniques/palette-design).
            .or_else(|| crate::palette_library::find(id).map(|p| p.to_palette()))
            .unwrap_or_else(|| Self::all().swap_remove(0))
    }

    /// Samples the gradient into `size` linear-RGB entries (rgba, a = 1): a periodic
    /// Catmull-Rom spline through the stops in OKLab, clamped to the sRGB gamut.
    pub fn table(&self, size: usize) -> Vec<[f32; 4]> {
        gradient_table(&self.stops, size)
    }

    /// `count` evenly spaced sRGB samples of the gradient (swatches in the UI).
    pub fn swatch(&self, count: usize) -> Vec<u32> {
        let table = self.table(1024);
        (0..count)
            .map(|i| {
                let c = table[i * 1024 / count];
                let ch = |v: f32| (srgb_from_linear(v as f64) * 255.0).round().clamp(0.0, 255.0) as u32;
                (ch(c[0]) << 16) | (ch(c[1]) << 8) | ch(c[2])
            })
            .collect()
    }
}

/// Samples any palette given as `(position 0..1, sRGB hex)` stops into `size` linear-RGB
/// entries (rgba, a = 1): a periodic Catmull-Rom spline through the stops in OKLab, clamped
/// to the sRGB gamut. Works for imported or custom palettes as well as the built-in ones.
pub fn gradient_table(stops: &[(f64, u32)], size: usize) -> Vec<[f32; 4]> {
    let n = stops.len();
    if n == 0 {
        return vec![[0.0, 0.0, 0.0, 1.0]; size];
    }
    let lab: Vec<[f64; 3]> = stops.iter().map(|&(_, hex)| oklab_from_hex(hex)).collect();
    let pos: Vec<f64> = stops.iter().map(|s| s.0).collect();
    (0..size)
        .map(|i| {
            let t = i as f64 / size as f64;
            if n == 1 {
                let c = linear_from_oklab(lab[0]);
                return [c[0] as f32, c[1] as f32, c[2] as f32, 1.0];
            }
            // Segment k: from stop k to stop k+1 (wrapping).
            let mut k = n - 1;
            for (j, &p) in pos.iter().enumerate() {
                if p <= t {
                    k = j;
                }
            }
            let (a, mut b) = (pos[k], pos[(k + 1) % n]);
            let mut tt = t;
            if b <= a {
                b += 1.0;
                if tt < a {
                    tt += 1.0;
                }
            }
            let u = (tt - a) / (b - a).max(1e-9);
            let p0 = lab[(k + n - 1) % n];
            let p1 = lab[k];
            let p2 = lab[(k + 1) % n];
            let p3 = lab[(k + 2) % n];
            let mut c = [0.0; 3];
            for d in 0..3 {
                c[d] = catmull_rom(p0[d], p1[d], p2[d], p3[d], u);
            }
            let rgb = linear_from_oklab(c);
            [rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0]
        })
        .collect()
}

/// The colours of dense regions for a palette table (kb: techniques/coloring-methods, "Dense
/// regions"): where the palette cycles faster than a pixel, the view fades to a calm `deep`
/// colour (the palette's darkest third, averaged, kept saturated and brought to a low
/// luminance) with thin `filament` lines along the boundary (its brightest tenth, brought to
/// a bright luminance), instead of the palette's mean, a grey. Linear RGB.
pub fn dense_colors(table: &[[f32; 4]]) -> ([f32; 3], [f32; 3]) {
    let lum = |c: &[f32; 4]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let mut sorted: Vec<&[f32; 4]> = table.iter().collect();
    sorted.sort_by(|a, b| lum(a).total_cmp(&lum(b)));
    let avg = |s: &[&[f32; 4]]| {
        let n = s.len().max(1) as f32;
        let mut m = [0.0f32; 4];
        for c in s {
            for k in 0..3 {
                m[k] += c[k] / n;
            }
        }
        m
    };
    let n = sorted.len();
    let dark = avg(&sorted[..(n / 3).max(1).min(n)]);
    let bright = avg(&sorted[n - (n / 10).max(1).min(n)..]);
    let to_lum = |c: [f32; 4], target: f32| {
        let l = lum(&c).max(1e-5);
        let f = target / l;
        [(c[0] * f).min(1.0), (c[1] * f).min(1.0), (c[2] * f).min(1.0)]
    };
    (to_lum(dark, 0.012), to_lum(bright, 0.35))
}

fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    let u2 = u * u;
    let u3 = u2 * u;
    0.5 * (2.0 * p1 + (p2 - p0) * u + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u2 + (3.0 * p1 - p0 - 3.0 * p2 + p3) * u3)
}

/// A look: a palette and the colouring that suits it (FL). Colours are sRGB hex.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub id: &'static str,
    pub name: &'static str,
    pub palette: &'static str,
    /// Palette cycles per iteration.
    pub density: f64,
    pub shading: f64,
    pub light_angle: f64,
    pub light_height: f64,
    pub specular: f64,
    pub shininess: f64,
    pub glow: f64,
    pub glow_color: u32,
    pub glow_width: f64,
    pub outline: f64,
    pub outline_width: f64,
    pub edge_color: u32,
    pub interior: u32,
    /// The orbit-density looks (BB): Buddhabrot or Nebulabrot instead of the escape-time
    /// colouring; off for every other look.
    pub buddha: crate::buddha::BuddhaMode,
}

impl Look {
    /// The looks, in menu order; the first is the default.
    pub fn all() -> Vec<Look> {
        let base = Look {
            id: "aurora",
            name: "Aurora",
            palette: "aurora",
            density: 0.03,
            shading: 0.75,
            light_angle: 135.0,
            light_height: 1.4,
            specular: 0.35,
            shininess: 28.0,
            glow: 0.0,
            glow_color: 0xFFFFFF,
            glow_width: 6.0,
            outline: 0.7,
            outline_width: 1.6,
            edge_color: 0x02040A,
            interior: 0x02040C,
            buddha: crate::buddha::BuddhaMode::Off,
        };
        vec![
            base.clone(),
            Look {
                id: "classic",
                name: "Classic",
                palette: "classic",
                density: 0.025,
                interior: 0x000000,
                edge_color: 0x000000,
                ..base.clone()
            },
            Look {
                id: "ember",
                name: "Ember",
                palette: "ember",
                density: 0.02,
                specular: 0.25,
                glow: 0.12,
                glow_color: 0xFF7A1A,
                glow_width: 10.0,
                edge_color: 0x050000,
                interior: 0x050001,
                ..base.clone()
            },
            Look {
                id: "lagoon",
                name: "Lagoon",
                palette: "lagoon",
                density: 0.025,
                specular: 0.45,
                shininess: 36.0,
                edge_color: 0x000A10,
                interior: 0x00080E,
                ..base.clone()
            },
            Look {
                id: "nebula",
                name: "Nebula",
                palette: "nebula",
                density: 0.03,
                glow: 0.15,
                glow_color: 0xD7A6FF,
                glow_width: 12.0,
                edge_color: 0x05000D,
                interior: 0x060012,
                ..base.clone()
            },
            Look {
                id: "dusk",
                name: "Dusk",
                palette: "dusk",
                density: 0.025,
                edge_color: 0x0A0410,
                interior: 0x0C0618,
                ..base.clone()
            },
            Look {
                id: "moss",
                name: "Moss",
                palette: "moss",
                density: 0.025,
                specular: 0.2,
                edge_color: 0x050A05,
                interior: 0x050A06,
                ..base.clone()
            },
            Look {
                id: "ice",
                name: "Ice",
                palette: "ice",
                density: 0.02,
                specular: 0.5,
                shininess: 48.0,
                glow: 0.1,
                glow_color: 0xCFE8FF,
                glow_width: 8.0,
                edge_color: 0x01030A,
                interior: 0x01030B,
                ..base.clone()
            },
            Look {
                id: "graphite",
                name: "Graphite",
                palette: "graphite",
                density: 0.02,
                shading: 0.85,
                specular: 0.3,
                edge_color: 0x000000,
                interior: 0x050506,
                ..base.clone()
            },
            Look {
                id: "spectrum",
                name: "Spectrum",
                palette: "spectrum",
                density: 0.015,
                shading: 0.6,
                specular: 0.3,
                edge_color: 0x000000,
                interior: 0x000000,
                ..base.clone()
            },
            // The orbit-density looks (BB, kb: plans/buddhabrot): Mandelbrot only (the pickers
            // hide them for other formulas, `FormulaCapabilities::buddhabrot`). The Buddhabrot
            // runs its tone curve through the palette's rising half; the Nebulabrot's channels
            // are red, green and blue (its own colour map, the palette is not used). The rest of
            // the look is the escape-time colouring shown while moving and past the depth limit.
            Look {
                id: "buddhabrot",
                name: "Buddhabrot",
                palette: "ice",
                density: 0.02,
                specular: 0.5,
                shininess: 48.0,
                edge_color: 0x01030A,
                interior: 0x01030B,
                buddha: crate::buddha::BuddhaMode::Buddhabrot,
                ..base.clone()
            },
            Look {
                id: "nebulabrot",
                name: "Nebulabrot",
                palette: "nebula",
                density: 0.03,
                edge_color: 0x05000D,
                interior: 0x060012,
                buddha: crate::buddha::BuddhaMode::Nebulabrot,
                ..base
            },
        ]
    }

    pub fn default_look() -> Look {
        Self::all().swap_remove(0)
    }

    pub fn named(id: &str) -> Option<Look> {
        Self::all().into_iter().find(|l| l.id == id)
    }

    /// Applies the look's colouring to `s` (the iteration settings, AA and cycling stay).
    pub fn apply(&self, s: &mut RenderSettings) {
        let lin = |hex: u32| linear(hex);
        s.palette_id = self.palette.to_string();
        s.color_density = self.density;
        s.color_offset = 0.0;
        s.log_coloring = false;
        s.smooth_coloring = true;
        s.shading = self.shading;
        s.light_angle = self.light_angle;
        s.light_height = self.light_height;
        s.specular = self.specular;
        s.shininess = self.shininess;
        s.glow = self.glow;
        s.glow_color = lin(self.glow_color);
        s.glow_width = self.glow_width;
        s.outline = self.outline;
        s.outline_width = self.outline_width;
        s.edge_color = lin(self.edge_color);
        s.interior_color = lin(self.interior);
        s.palette_filter = true;
        s.dense_calm = true;
        s.palette_stops = None;
        s.buddha = self.buddha;
    }
}

/// The palette as a cyclic mip chain for filtered lookups (palette anti-aliasing, FL): level 0
/// is `table` (`size` entries, a power of two), each further level halves the previous one
/// by averaging neighbouring pairs, down to one entry (the palette's mean colour). Levels
/// are concatenated, level 0 first; `2 * size - 1` entries in all.
pub fn mip_chain(table: &[[f32; 4]]) -> Vec<[f32; 4]> {
    assert!(table.len().is_power_of_two(), "palette size must be a power of two");
    let mut out = table.to_vec();
    let mut level = table.to_vec();
    while level.len() > 1 {
        level = level
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                let m = |k: usize| 0.5 * (p[0][k] + p[1][k]);
                [m(0), m(1), m(2), 1.0]
            })
            .collect();
        out.extend_from_slice(&level);
    }
    out
}

/// Number of levels of [`mip_chain`] for a table of `size` entries.
pub fn mip_levels(size: usize) -> usize {
    size.trailing_zeros() as usize + 1
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_from_linear(c: f64) -> f64 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB hex to linear RGB.
pub fn linear(hex: u32) -> [f32; 3] {
    let ch = |v: u32| srgb_to_linear((v & 0xFF) as f64 / 255.0) as f32;
    [ch(hex >> 16), ch(hex >> 8), ch(hex)]
}

/// Linear sRGB to OKLab (Ottosson's matrices).
pub fn oklab(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

/// OKLab to linear sRGB, clamped to the gamut.
pub fn linear_from_oklab(lab: [f64; 3]) -> [f64; 3] {
    let [lv, a, b] = lab;
    let l = (lv + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (lv - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (lv - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        (4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s).clamp(0.0, 1.0),
        (-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s).clamp(0.0, 1.0),
        (-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s).clamp(0.0, 1.0),
    ]
}

fn oklab_from_hex(hex: u32) -> [f64; 3] {
    let c = linear(hex);
    oklab([c[0] as f64, c[1] as f64, c[2] as f64])
}

/// OKLCH (lightness, chroma, hue in degrees) to sRGB hex.
fn oklch_hex(l: f64, c: f64, h: f64) -> u32 {
    let (s, co) = h.to_radians().sin_cos();
    let rgb = linear_from_oklab([l, c * co, c * s]);
    let ch = |v: f64| (srgb_from_linear(v) * 255.0).round() as u32;
    (ch(rgb[0]) << 16) | (ch(rgb[1]) << 8) | ch(rgb[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_endpoints() {
        let t = Palette::named("grey").table(1024);
        assert_eq!(t.len(), 1024);
        assert!(t[0][0] < 1e-6);
        assert!((t[512][0] - 1.0).abs() < 1e-4);
        assert_eq!(Palette::named("nope").id, "aurora");
        assert_eq!(Palette::named("rainbow").id, "spectrum");
    }

    #[test]
    fn stops_are_hit_exactly() {
        // The spline passes through every stop.
        for p in Palette::all() {
            let t = p.table(4096);
            for &(pos, hex) in &p.stops {
                let i = (pos * 4096.0).round() as usize % 4096;
                let want = linear(hex);
                for k in 0..3 {
                    assert!((t[i][k] - want[k]).abs() < 0.02, "{} at {pos}", p.id);
                }
            }
        }
    }

    #[test]
    fn oklab_round_trips() {
        for hex in [0x123456u32, 0xFFAA00, 0x000764, 0xFFFFFF] {
            let c = linear(hex);
            let back = linear_from_oklab(oklab([c[0] as f64, c[1] as f64, c[2] as f64]));
            for k in 0..3 {
                assert!((back[k] - c[k] as f64).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn looks_name_existing_palettes() {
        let ids: Vec<&str> = Palette::all().iter().map(|p| p.id).collect();
        let looks = Look::all();
        assert!((6..=12).contains(&looks.len()));
        assert_eq!(looks.iter().filter(|l| l.buddha.is_on()).count(), 2);
        for l in &looks {
            assert!(ids.contains(&l.palette), "{}", l.id);
        }
        let mut s = RenderSettings::default();
        looks[2].apply(&mut s);
        assert_eq!(s.palette_id, looks[2].palette);
    }

    #[test]
    fn mip_chain_halves_to_the_mean() {
        let t = Palette::named("grey").table(1024);
        let m = mip_chain(&t);
        assert_eq!(m.len(), 2047);
        assert_eq!(mip_levels(1024), 11);
        let mean: f32 = t.iter().map(|c| c[0]).sum::<f32>() / 1024.0;
        assert!((m[2046][0] - mean).abs() < 1e-5);
        // Level 1 entry 0 averages level 0 entries 0 and 1.
        assert!((m[1024][1] - 0.5 * (t[0][1] + t[1][1])).abs() < 1e-7);
    }
}
