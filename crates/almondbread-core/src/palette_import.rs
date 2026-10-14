//! Palette import and export: Ultra Fractal gradients (`.ugr`, `.gradient`) and Fractint
//! colour maps (`.map`) (kb: techniques/palette-design §File formats).
//!
//! Ultra Fractal: a file holds named entries `title { ... }`; the `gradient:` section has
//! `index=N color=C` pairs, N on a 400-step ring (0..399, values outside wrap) and C a
//! decimal Windows COLORREF (`R + 256 G + 65536 B`). `smooth=yes` joins the control points
//! with a spline, `no` linearly; `rotation=R` shifts the ring. The `opacity:` section is
//! ignored (the renderer has no palette alpha). `;` starts a comment outside quotes.
//!
//! Fractint: one `R G B` line (0..255 each) per palette entry, up to 256, anything after the
//! third number is a comment. Entry 0 is the inside colour in Fractint (colour cycling
//! leaves it alone), so it becomes the deep colour and entries 1.. form the cyclic gradient.

use crate::palette_library::{Category, LibraryPalette, PaletteError, Stop, hex_from_lab, lab_from_hex};

/// Ultra Fractal's gradient ring.
const UGR_RING: f64 = 400.0;

fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() { "palette".to_string() } else { s }
}

/// COLORREF (`0x00BBGGRR`) to `0xRRGGBB`.
fn colorref_to_hex(c: u32) -> u32 {
    let (r, g, b) = (c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF);
    (r << 16) | (g << 8) | b
}

fn hex_to_colorref(h: u32) -> u32 {
    let (r, g, b) = ((h >> 16) & 0xFF, (h >> 8) & 0xFF, h & 0xFF);
    r | (g << 8) | (b << 16)
}

/// Removes `;` comments (outside double quotes) from every line.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut quoted = false;
        for c in line.chars() {
            if c == '"' {
                quoted = !quoted;
            }
            if c == ';' && !quoted {
                break;
            }
            out.push(c);
        }
        out.push('\n');
    }
    out
}

/// Splits `key=value` tokens (values may be double-quoted and contain spaces) and section
/// markers (`gradient:`).
fn tokens(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = body.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }
        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == '=' {
                break;
            }
            key.push(c);
            chars.next();
        }
        if chars.peek() == Some(&'=') {
            chars.next();
            let mut val = String::new();
            if chars.peek() == Some(&'"') {
                chars.next();
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                    val.push(c);
                }
            } else {
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() {
                        break;
                    }
                    val.push(c);
                    chars.next();
                }
            }
            out.push((key.to_ascii_lowercase(), val));
        } else if !key.is_empty() {
            out.push((key.to_ascii_lowercase(), String::new()));
        }
    }
    out
}

/// Parses an Ultra Fractal gradient file (one or more entries).
pub fn parse_ugr(text: &str) -> Result<Vec<LibraryPalette>, PaletteError> {
    let text = strip_comments(text);
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find('{') {
        let entry = rest[..open].trim().rsplit('\n').next().unwrap_or("").trim().to_string();
        let mut depth = 0;
        let mut close = None;
        for (i, c) in rest[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            return Err(PaletteError(format!("{entry}: unclosed {{")));
        };
        out.push(parse_ugr_entry(&entry, &rest[open + 1..close])?);
        rest = &rest[close + 1..];
    }
    if out.is_empty() {
        return Err(PaletteError("no gradient entries".into()));
    }
    Ok(out)
}

fn parse_ugr_entry(entry: &str, body: &str) -> Result<LibraryPalette, PaletteError> {
    let mut section = String::new();
    let mut title = None;
    let mut smooth = true;
    let mut rotation = 0.0;
    let mut points: Vec<(f64, Option<u32>)> = Vec::new();
    for (k, v) in tokens(body) {
        if let Some(s) = k.strip_suffix(':') {
            section = s.to_string();
            continue;
        }
        if section != "gradient" {
            continue;
        }
        match k.as_str() {
            "title" => title = Some(v),
            "smooth" => smooth = v.eq_ignore_ascii_case("yes"),
            "rotation" => rotation = v.parse().unwrap_or(0.0),
            "index" => {
                let i: f64 = v.parse().map_err(|_| PaletteError(format!("{entry}: bad index {v}")))?;
                points.push((i, None));
            }
            "color" => {
                let c: i64 = v.parse().map_err(|_| PaletteError(format!("{entry}: bad color {v}")))?;
                match points.last_mut() {
                    Some((_, slot @ None)) => *slot = Some(colorref_to_hex((c as u32) & 0xFF_FF_FF)),
                    _ => return Err(PaletteError(format!("{entry}: color without index"))),
                }
            }
            _ => {}
        }
    }
    let stops: Vec<Stop> = points
        .into_iter()
        .filter_map(|(i, c)| {
            c.map(|hex| Stop {
                pos: ((i + rotation) / UGR_RING).rem_euclid(1.0),
                lab: lab_from_hex(hex),
            })
        })
        .collect();
    if stops.is_empty() {
        return Err(PaletteError(format!("{entry}: no colours")));
    }
    let name = title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| entry.to_string());
    let mut p = LibraryPalette::new(&slug(&name), &name, Category::User, stops);
    p.smooth = smooth;
    p.source = format!("Ultra Fractal gradient {entry}");
    Ok(p)
}

/// Writes `p` as an Ultra Fractal gradient entry (`samples` control points of its curve,
/// at most 400).
pub fn write_ugr(p: &LibraryPalette, samples: usize) -> String {
    let n = samples.clamp(2, 400);
    let mut s = format!(
        "{} {{\ngradient:\n  title=\"{}\" smooth=yes rotation=0\n",
        slug(&p.name),
        p.name.replace('"', "'")
    );
    for i in 0..n {
        let index = (i as f64 * UGR_RING / n as f64).round() as u32;
        let hex = hex_from_lab(p.color_at(f64::from(index) / UGR_RING));
        s.push_str(&format!("  index={index} color={}\n", hex_to_colorref(hex)));
    }
    s.push_str("opacity:\n  smooth=no index=0 opacity=255\n}\n");
    s
}

/// Parses a Fractint colour map. `name` names the palette (usually the file stem).
pub fn parse_map(text: &str, name: &str) -> Result<LibraryPalette, PaletteError> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let nums: Vec<u32> = line
            .split_whitespace()
            .take(3)
            .map_while(|w| w.parse::<u32>().ok())
            .collect();
        if nums.len() < 3 {
            continue; // blank, comment-only or malformed line
        }
        let ch = |v: u32| v.min(255);
        entries.push((ch(nums[0]) << 16) | (ch(nums[1]) << 8) | ch(nums[2]));
        if entries.len() == 256 {
            break;
        }
    }
    if entries.len() < 2 {
        return Err(PaletteError(format!("{name}: fewer than 2 colours")));
    }
    let (deep, ring) = if entries.len() >= 3 {
        (Some(entries[0]), &entries[1..])
    } else {
        (None, &entries[..])
    };
    let n = ring.len();
    let stops = ring
        .iter()
        .enumerate()
        .map(|(i, &hex)| Stop {
            pos: i as f64 / n as f64,
            lab: lab_from_hex(hex),
        })
        .collect();
    let mut p = LibraryPalette::new(&slug(name), name, Category::User, stops);
    // Dense samples: straight segments reproduce the map, steps included, without overshoot.
    p.smooth = n <= 32;
    if let Some(d) = deep {
        p.deep = d;
    }
    p.source = format!("Fractint map {name}");
    Ok(p)
}

/// Writes `p` as a 256-entry Fractint map: entry 0 the deep colour, 1..255 one cycle.
pub fn write_map(p: &LibraryPalette) -> String {
    let mut s = String::new();
    let line = |hex: u32| format!("{} {} {}", (hex >> 16) & 0xFF, (hex >> 8) & 0xFF, hex & 0xFF);
    s.push_str(&line(p.deep));
    s.push_str(&format!(" {}\n", p.name));
    for i in 0..255 {
        s.push_str(&line(hex_from_lab(p.color_at(i as f64 / 255.0))));
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette_library::find;

    const UGR: &str = r#"
; two gradients, as Ultra Fractal saves them
Sunrise {
gradient:
  title="Sunrise; warm" smooth=yes rotation=0
  index=0 color=8388608
  index=100 color=16777215
  index=200 color=33023
  index=-50 color=0
opacity:
  smooth=no index=0 opacity=255
}

blocky {
gradient:
  title="" smooth=no rotation=100 index=0 color=255 index=200 color=65280
}
"#;

    #[test]
    fn ugr_entries() {
        let g = parse_ugr(UGR).unwrap();
        assert_eq!(g.len(), 2);
        let a = &g[0];
        assert_eq!(a.name, "Sunrise; warm");
        assert_eq!(a.id, "sunrise-warm");
        assert!(a.smooth);
        assert_eq!(a.stops.len(), 4);
        // Sorted ring positions: 0 navy (COLORREF 8388608 = B 128), 0.25 white,
        // 0.5 orange (33023 = R 255, G 128), 0.875 black (index -50 wraps to 350).
        let hexes: Vec<u32> = a.stops.iter().map(|s| hex_from_lab(s.lab)).collect();
        assert_eq!(hexes, vec![0x000080, 0xFFFFFF, 0xFF8000, 0x000000]);
        assert!((a.stops[3].pos - 0.875).abs() < 1e-12);
        let b = &g[1];
        assert_eq!(b.name, "blocky");
        assert!(!b.smooth);
        // rotation 100 moves red to 0.25 and green to 0.75.
        assert_eq!(hex_from_lab(b.stops[0].lab), 0xFF0000);
        assert!((b.stops[0].pos - 0.25).abs() < 1e-12);
        assert_eq!(hex_from_lab(b.stops[1].lab), 0x00FF00);
    }

    #[test]
    fn ugr_published_layout() {
        // The layout of the UF tutorial's gradient: a comma in the entry name, a quoted title
        // with spaces, an index at a line end with its colour on the next line, `linked`.
        let text = "Gradient-Fractal2,Background {\ngradient:\n  title=\"Gradient - Fractal2, Background\" smooth=yes rotation=155 index=84\n  color=13799050 index=247 color=16448758 index=332 color=9665827 linked=yes\nopacity:\n  smooth=no index=0 opacity=255\n}\n";
        let g = parse_ugr(text).unwrap().remove(0);
        assert_eq!(g.name, "Gradient - Fractal2, Background");
        assert_eq!(g.stops.len(), 3);
        // 84 + 155 = 239 -> 0.5975; 247 + 155 = 402 -> 0.005; 332 + 155 = 487 -> 0.2175.
        let pos: Vec<f64> = g.stops.iter().map(|s| s.pos).collect();
        assert!((pos[0] - 0.005).abs() < 1e-9 && (pos[1] - 0.2175).abs() < 1e-9 && (pos[2] - 0.5975).abs() < 1e-9);
        // 13799050 = 0xD28E8A: R 0x8A, G 0x8E, B 0xD2.
        assert_eq!(hex_from_lab(g.stops[2].lab), 0x8A8ED2);
    }

    #[test]
    fn ugr_errors() {
        assert!(parse_ugr("nothing here").is_err());
        assert!(parse_ugr("x { gradient: index=0 color=0").is_err());
        assert!(parse_ugr("x { gradient: color=5 }").is_err());
        assert!(parse_ugr("x { gradient: index=zero color=5 }").is_err());
    }

    #[test]
    fn ugr_round_trip() {
        let p = find("sunset-beach").unwrap();
        let back = parse_ugr(&write_ugr(p, 100)).unwrap().remove(0);
        assert_eq!(back.name, p.name);
        let (a, b) = (p.table(400), back.table(400));
        for i in 0..400 {
            for k in 0..3 {
                assert!((a[i][k] - b[i][k]).abs() < 0.02, "entry {i}");
            }
        }
    }

    #[test]
    fn fractint_map() {
        let mut text = String::from("0 0 0 inside\n");
        for i in 0..255u32 {
            text.push_str(&format!("{} {} 255   comment {i}\n", i, 255 - i));
        }
        text.push_str("\n7 7 7 ignored beyond 256\n");
        let p = parse_map(&text, "Ramp Map").unwrap();
        assert_eq!(p.id, "ramp-map");
        assert_eq!(p.deep, 0x000000);
        assert_eq!(p.stops.len(), 255);
        assert!(!p.smooth);
        assert_eq!(hex_from_lab(p.stops[10].lab), (10 << 16) | (245 << 8) | 255);
        assert!(parse_map("1 2 3\n", "x").is_err());
        assert!(parse_map("# just words\n", "x").is_err());
    }

    #[test]
    fn map_round_trip() {
        let p = find("royal-gold").unwrap();
        let back = parse_map(&write_map(p), "Royal Gold").unwrap();
        assert_eq!(back.deep, p.deep);
        let (a, b) = (p.table(255), back.table(255));
        for i in 0..255 {
            for k in 0..3 {
                assert!((a[i][k] - b[i][k]).abs() < 0.01, "entry {i}");
            }
        }
    }
}
