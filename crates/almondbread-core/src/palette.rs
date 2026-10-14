//! Cyclic colour gradients defined by sRGB stops, sampled into a linear-RGB lookup table.

/// A palette: `(position 0..1, sRGB hex)` stops; the gradient wraps from the last stop back
/// to the first.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    pub stops: Vec<(f64, u32)>,
}

impl Palette {
    pub fn all() -> Vec<Palette> {
        let p = |id, name, stops: &[(f64, u32)]| Palette {
            id,
            name,
            stops: stops.to_vec(),
        };
        vec![
            p(
                "classic",
                "Classic",
                &[
                    (0.0, 0x000764),
                    (0.16, 0x206BCB),
                    (0.42, 0xEDFFFF),
                    (0.6425, 0xFFAA00),
                    (0.8575, 0x000200),
                ],
            ),
            p(
                "fire",
                "Fire",
                &[
                    (0.0, 0x000000),
                    (0.25, 0x7A0000),
                    (0.5, 0xFF5A00),
                    (0.75, 0xFFE070),
                    (0.9, 0xFFFFFF),
                ],
            ),
            p(
                "ocean",
                "Ocean",
                &[
                    (0.0, 0x001018),
                    (0.3, 0x005F73),
                    (0.55, 0x0A9396),
                    (0.75, 0x94D2BD),
                    (0.9, 0xE9D8A6),
                ],
            ),
            p(
                "electric",
                "Electric",
                &[
                    (0.0, 0x05001A),
                    (0.2, 0x3A0CA3),
                    (0.4, 0x4CC9F0),
                    (0.6, 0xF72585),
                    (0.8, 0xFFF3B0),
                ],
            ),
            p(
                "forest",
                "Forest",
                &[
                    (0.0, 0x0B1A0B),
                    (0.3, 0x2D6A4F),
                    (0.55, 0x95D5B2),
                    (0.75, 0xD8C99B),
                    (0.9, 0x6B4226),
                ],
            ),
            p("grey", "Greyscale", &[(0.0, 0x000000), (0.5, 0xFFFFFF)]),
            Palette {
                id: "rainbow",
                name: "Rainbow",
                stops: (0..12)
                    .map(|i| {
                        let h = f64::from(i) / 12.0;
                        (h, hsv_hex(h, 0.85, 1.0))
                    })
                    .collect(),
            },
        ]
    }

    pub fn default_palette() -> Palette {
        Self::all().swap_remove(0)
    }

    /// The palette with `id`, or the default.
    pub fn named(id: &str) -> Palette {
        Self::all()
            .into_iter()
            .find(|p| p.id == id)
            .unwrap_or_else(Self::default_palette)
    }

    /// Samples the gradient into `size` linear-RGB entries (rgba, a = 1), smoothstep between
    /// stops. Same arithmetic (f32 per channel) as the Swift palette.
    pub fn table(&self, size: usize) -> Vec<[f32; 4]> {
        let s: Vec<(f64, [f32; 3])> = self.stops.iter().map(|&(t, hex)| (t, linear(hex))).collect();
        (0..size)
            .map(|i| {
                let t = i as f64 / size as f64;
                let mut ai = s.len() - 1;
                for (k, stop) in s.iter().enumerate() {
                    if stop.0 <= t {
                        ai = k;
                    }
                }
                let bi = (ai + 1) % s.len();
                let (ta, ca) = s[ai];
                let (mut tb, cb) = s[bi];
                let mut tt = t;
                if tb <= ta {
                    tb += 1.0;
                    if tt < ta {
                        tt += 1.0;
                    }
                }
                let f = ((tt - ta) / (tb - ta).max(1e-9)) as f32;
                let w = f * f * (3.0 - 2.0 * f);
                let c = |k: usize| ca[k] + (cb[k] - ca[k]) * w;
                [c(0), c(1), c(2), 1.0]
            })
            .collect()
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

/// sRGB hex to linear RGB.
pub fn linear(hex: u32) -> [f32; 3] {
    let ch = |v: u32| {
        let c = (v & 0xFF) as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    [ch(hex >> 16), ch(hex >> 8), ch(hex)]
}

fn hsv_hex(h: f64, s: f64, v: f64) -> u32 {
    let i = ((h * 6.0) as i64) % 6;
    let f = h * 6.0 - ((h * 6.0) as i64) as f64;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    (((r * 255.0) as u32) << 16) | (((g * 255.0) as u32) << 8) | ((b * 255.0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_endpoints() {
        let t = Palette::named("grey").table(1024);
        assert_eq!(t.len(), 1024);
        assert_eq!(t[0], [0.0, 0.0, 0.0, 1.0]);
        assert!((t[512][0] - 1.0).abs() < 1e-6);
        assert_eq!(Palette::named("nope").id, "classic");
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
