//! The palette library (kb: techniques/palette-design, synthesis/fractal-palettes): named,
//! categorised gradients with the mapping that suits them (density, offset, log), a
//! designated deep colour for interiors and dense regions, and their attribution and licence.
//!
//! Palettes are data: `palettes/designed.json` (our own, CC0) and `palettes/derived.json`
//! (subsampled open colour maps, written by `palettes/derive.py`), parsed once on first use.
//! The same JSON format holds user palettes ([`LibraryPalette::from_json`],
//! [`LibraryPalette::to_json`]); `.ugr` and `.map` imports are in [`crate::palette_import`].
//!
//! Sampling matches [`crate::palette::Palette::table`]: stops in OKLab joined by a Catmull-Rom
//! spline, sampled into linear RGB. A cyclic palette wraps from its last stop to its first; a
//! sequential one (most scientific maps) is mirrored, there and back, so it still cycles
//! without a seam. The renderer reaches library palettes through `Palette::named`, which
//! falls back to [`find`] for ids that are not built in.

use std::collections::HashMap;
use std::fmt;
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::palette::{Palette, linear, linear_from_oklab, oklab};
use crate::settings::RenderSettings;

/// Library sections, in menu order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Classic,
    Natural,
    FireIce,
    Pastel,
    Neon,
    Monochrome,
    Scientific,
    Cyclic,
    HighContrast,
    User,
}

impl Category {
    pub const ALL: [Category; 10] = [
        Category::Classic,
        Category::Natural,
        Category::FireIce,
        Category::Pastel,
        Category::Neon,
        Category::Monochrome,
        Category::Scientific,
        Category::Cyclic,
        Category::HighContrast,
        Category::User,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Classic => "Classic fractal",
            Category::Natural => "Natural",
            Category::FireIce => "Fire & ice",
            Category::Pastel => "Pastel",
            Category::Neon => "Neon & vapor",
            Category::Monochrome => "Monochrome & ink",
            Category::Scientific => "Scientific",
            Category::Cyclic => "Cyclic",
            Category::HighContrast => "High contrast",
            Category::User => "My palettes",
        }
    }

    /// The JSON spelling (`fire-ice`, ...).
    pub fn key(self) -> &'static str {
        match self {
            Category::Classic => "classic",
            Category::Natural => "natural",
            Category::FireIce => "fire-ice",
            Category::Pastel => "pastel",
            Category::Neon => "neon",
            Category::Monochrome => "monochrome",
            Category::Scientific => "scientific",
            Category::Cyclic => "cyclic",
            Category::HighContrast => "high-contrast",
            Category::User => "user",
        }
    }

    pub fn from_key(key: &str) -> Option<Category> {
        Self::ALL.into_iter().find(|c| c.key() == key)
    }
}

/// A gradient stop: position in `0..1` and colour in OKLab.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub pos: f64,
    pub lab: [f64; 3],
}

/// A library palette.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryPalette {
    pub id: String,
    pub name: String,
    pub category: Category,
    /// Sorted by position; positions in `0..1`.
    pub stops: Vec<Stop>,
    /// Wraps from the last stop to the first; otherwise the gradient is mirrored.
    pub cyclic: bool,
    /// Spline (true) or straight OKLab segments between stops (a hard-edged `.ugr` with
    /// `smooth=no`, a `.map` with steps).
    pub smooth: bool,
    /// Recommended palette cycles per iteration (per log unit when `log`).
    pub density: f64,
    /// Recommended palette phase, `0..1`.
    pub offset: f64,
    /// Recommended log mapping of the smooth count (`t = ln(mu + 1) * density`).
    pub log: bool,
    /// Interior and dense-region colour, sRGB hex.
    pub deep: u32,
    pub attribution: String,
    pub license: String,
    /// Where the data came from (URL or file), empty for our own.
    pub source: String,
}

/// A parse error of a palette file.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteError(pub String);

impl fmt::Display for PaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PaletteError {}

fn perr<T>(msg: impl Into<String>) -> Result<T, PaletteError> {
    Err(PaletteError(msg.into()))
}

// MARK: colour helpers

fn srgb_from_linear(c: f64) -> f64 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB hex to OKLab.
pub fn lab_from_hex(hex: u32) -> [f64; 3] {
    let c = linear(hex);
    oklab([f64::from(c[0]), f64::from(c[1]), f64::from(c[2])])
}

/// OKLab to sRGB hex (gamut-clamped).
pub fn hex_from_lab(lab: [f64; 3]) -> u32 {
    let rgb = linear_from_oklab(lab);
    let ch = |v: f64| (srgb_from_linear(v) * 255.0).round().clamp(0.0, 255.0) as u32;
    (ch(rgb[0]) << 16) | (ch(rgb[1]) << 8) | ch(rgb[2])
}

/// Linear RGB of an OKLab colour without clamping.
fn linear_unclamped(lab: [f64; 3]) -> [f64; 3] {
    let [lv, a, b] = lab;
    let l = (lv + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (lv - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (lv - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
}

/// OKLCH (hue in degrees) to OKLab, chroma reduced at constant lightness and hue until the
/// colour fits sRGB (clipping channels instead would shift the hue).
pub fn lab_from_oklch(l: f64, c: f64, h: f64) -> [f64; 3] {
    let l = l.clamp(0.0, 1.0);
    let (s, co) = h.to_radians().sin_cos();
    let lab = |c: f64| [l, c * co, c * s];
    let fits = |c: f64| {
        linear_unclamped(lab(c))
            .iter()
            .all(|&v| (-1e-6..=1.0 + 1e-6).contains(&v))
    };
    if fits(c) {
        return lab(c);
    }
    let (mut lo, mut hi) = (0.0, c);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lab(lo)
}

/// Parses `#RRGGBB`, `RRGGBB` or `oklch(L C H)` (L in 0..1 or a percentage, H in degrees).
pub fn parse_color(s: &str) -> Result<[f64; 3], PaletteError> {
    let t = s.trim();
    if let Some(inner) = t.strip_prefix("oklch(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<&str> = inner
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() != 3 {
            return perr(format!("bad colour {s}"));
        }
        let num = |p: &str| -> Result<f64, PaletteError> {
            let (v, pct) = match p.strip_suffix('%') {
                Some(v) => (v, true),
                None => (p.strip_suffix("deg").unwrap_or(p), false),
            };
            let x: f64 = v.parse().map_err(|_| PaletteError(format!("bad colour {s}")))?;
            Ok(if pct { x / 100.0 } else { x })
        };
        return Ok(lab_from_oklch(num(parts[0])?, num(parts[1])?, num(parts[2])?));
    }
    let h = t.trim_start_matches('#');
    if h.len() == 6
        && let Ok(v) = u32::from_str_radix(h, 16)
    {
        return Ok(lab_from_hex(v));
    }
    perr(format!("bad colour {s}"))
}

fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    let u2 = u * u;
    let u3 = u2 * u;
    0.5 * (2.0 * p1 + (p2 - p0) * u + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u2 + (3.0 * p1 - p0 - 3.0 * p2 + p3) * u3)
}

// MARK: sampling

impl LibraryPalette {
    /// A palette from stops in OKLab (positions sorted, clamped to 0..1), with defaults:
    /// cyclic, smooth, density 0.02, deep colour derived from the darkest stop.
    pub fn new(id: &str, name: &str, category: Category, mut stops: Vec<Stop>) -> LibraryPalette {
        for s in &mut stops {
            s.pos = s.pos.clamp(0.0, 1.0);
        }
        stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        let deep = derived_deep(&stops);
        LibraryPalette {
            id: id.to_string(),
            name: name.to_string(),
            category,
            stops,
            cyclic: true,
            smooth: true,
            density: 0.02,
            offset: 0.0,
            log: false,
            deep,
            attribution: String::new(),
            license: String::new(),
            source: String::new(),
        }
    }

    /// The colour (OKLab) at phase `t` of one palette cycle (any real; wraps).
    pub fn color_at(&self, t: f64) -> [f64; 3] {
        let n = self.stops.len();
        if n == 0 {
            return [0.0; 3];
        }
        if n == 1 {
            return self.stops[0].lab;
        }
        let t = t.rem_euclid(1.0);
        if self.cyclic {
            self.cyclic_at(t)
        } else {
            // There and back: 0 -> first stop, 0.5 -> last stop, 1 -> first again.
            self.sequential_at(1.0 - (2.0 * t - 1.0).abs())
        }
    }

    fn cyclic_at(&self, t: f64) -> [f64; 3] {
        let st = &self.stops;
        let n = st.len();
        // Segment k: stop k to stop k+1, wrapping past the end.
        let k = st.iter().rposition(|s| s.pos <= t).unwrap_or(n - 1);
        let (a, mut b) = (st[k].pos, st[(k + 1) % n].pos);
        let mut tt = t;
        if b <= a {
            b += 1.0;
            if tt < a {
                tt += 1.0;
            }
        }
        let u = ((tt - a) / (b - a).max(1e-9)).clamp(0.0, 1.0);
        self.blend(
            st[(k + n - 1) % n].lab,
            st[k].lab,
            st[(k + 1) % n].lab,
            st[(k + 2) % n].lab,
            u,
        )
    }

    fn sequential_at(&self, t: f64) -> [f64; 3] {
        let st = &self.stops;
        let n = st.len();
        if t <= st[0].pos {
            return st[0].lab;
        }
        if t >= st[n - 1].pos {
            return st[n - 1].lab;
        }
        let k = st.iter().rposition(|s| s.pos <= t).unwrap_or(0).min(n - 2);
        let (a, b) = (st[k].pos, st[k + 1].pos);
        let u = ((t - a) / (b - a).max(1e-9)).clamp(0.0, 1.0);
        // Mirrored end neighbours: zero slope at both ends, so the turn-around is smooth.
        let p0 = if k == 0 { st[1].lab } else { st[k - 1].lab };
        let p3 = if k + 2 >= n { st[n - 2].lab } else { st[k + 2].lab };
        self.blend(p0, st[k].lab, st[k + 1].lab, p3, u)
    }

    fn blend(&self, p0: [f64; 3], p1: [f64; 3], p2: [f64; 3], p3: [f64; 3], u: f64) -> [f64; 3] {
        let mut c = [0.0; 3];
        for d in 0..3 {
            c[d] = if self.smooth {
                catmull_rom(p0[d], p1[d], p2[d], p3[d], u)
            } else {
                p1[d] + (p2[d] - p1[d]) * u
            };
        }
        c
    }

    /// `size` linear-RGB entries (rgba, a = 1), entry `i` at phase `i / size`: the format of
    /// [`Palette::table`], so it can go straight into [`crate::palette::mip_chain`].
    pub fn table(&self, size: usize) -> Vec<[f32; 4]> {
        (0..size)
            .map(|i| {
                let rgb = linear_from_oklab(self.color_at(i as f64 / size as f64));
                [rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0]
            })
            .collect()
    }

    /// `count` evenly spaced sRGB samples over one cycle (swatches).
    pub fn swatch(&self, count: usize) -> Vec<u32> {
        (0..count)
            .map(|i| hex_from_lab(self.color_at(i as f64 / count as f64)))
            .collect()
    }

    /// The deep colour in linear RGB (for `interior_color` / `edge_color`).
    pub fn deep_linear(&self) -> [f32; 3] {
        linear(self.deep)
    }

    /// Mean colour of a cycle in linear RGB: what a fully filtered palette (dense regions,
    /// palette anti-aliasing at its coarsest level) averages to.
    pub fn mean_linear(&self) -> [f32; 3] {
        let t = self.table(256);
        let mut m = [0.0f32; 3];
        for c in &t {
            for k in 0..3 {
                m[k] += c[k] / 256.0;
            }
        }
        m
    }

    /// OKLab lightness range over a cycle (0..1). Palettes whose bands differ in lightness
    /// keep their structure for colour-blind viewers and in greyscale; 0.4 or more is a
    /// reasonable bar (kb: techniques/palette-design §Colour-vision safety).
    pub fn lightness_span(&self) -> f64 {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for i in 0..256 {
            let l = self.color_at(f64::from(i) / 256.0)[0];
            lo = lo.min(l);
            hi = hi.max(l);
        }
        hi - lo
    }

    /// This palette as a renderer [`Palette`]: 128 evenly spaced sRGB stops of [`Self::table`]'s
    /// curve (the renderer joins them with the same spline, so the two agree to 8-bit
    /// rounding). Mirroring and straight segments are baked into the stops.
    pub fn to_palette(&'static self) -> Palette {
        const N: usize = 128;
        Palette {
            id: self.id.as_str(),
            name: self.name.as_str(),
            stops: (0..N)
                .map(|i| {
                    let t = i as f64 / N as f64;
                    (t, hex_from_lab(self.color_at(t)))
                })
                .collect(),
        }
    }

    /// Sets the palette and its recommended mapping and deep colour on `s`: palette id,
    /// density, offset, log mapping, smooth colouring, interior and edge colour. Lighting,
    /// glow, iterations and anti-aliasing stay (a palette is chosen independently of a look).
    pub fn apply(&self, s: &mut RenderSettings) {
        s.palette_id = self.id.clone();
        s.color_density = self.density;
        s.color_offset = self.offset;
        s.log_coloring = self.log;
        s.smooth_coloring = true;
        s.interior_color = self.deep_linear();
        s.edge_color = self.deep_linear();
    }
}

/// Default deep colour: the darkest stop, darker still and less saturated (an interior that
/// sits below every band, so the set's edge reads as depth rather than as another band).
fn derived_deep(stops: &[Stop]) -> u32 {
    let Some(dark) = stops.iter().min_by(|a, b| a.lab[0].total_cmp(&b.lab[0])) else {
        return 0;
    };
    let [l, a, b] = dark.lab;
    hex_from_lab([(l * 0.5).min(0.2), a * 0.5, b * 0.5])
}

// MARK: JSON

/// One palette in the JSON format (see `palettes/designed.json`). Stops are `"#RRGGBB"`,
/// `"oklch(L C H)"` or `{"at": 0.25, "color": "..."}`; stops without `at` are spaced evenly
/// (cyclic: `i / n`; sequential: `i / (n - 1)`).
#[derive(Serialize, Deserialize)]
struct PaletteJson {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    category: Option<String>,
    stops: Vec<Value>,
    #[serde(default)]
    cyclic: Option<bool>,
    #[serde(default)]
    smooth: Option<bool>,
    #[serde(default)]
    density: Option<f64>,
    #[serde(default)]
    offset: Option<f64>,
    #[serde(default)]
    log: Option<bool>,
    #[serde(default)]
    deep: Option<String>,
    #[serde(default)]
    attribution: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

fn from_json_value(v: Value) -> Result<LibraryPalette, PaletteError> {
    let p: PaletteJson = serde_json::from_value(v).map_err(|e| PaletteError(e.to_string()))?;
    if p.id.trim().is_empty() {
        return perr("palette without id");
    }
    let cyclic = p.cyclic.unwrap_or(true);
    let n = p.stops.len();
    if n == 0 {
        return perr(format!("{}: no stops", p.id));
    }
    let mut stops = Vec::with_capacity(n);
    for (i, s) in p.stops.iter().enumerate() {
        let even = if cyclic {
            i as f64 / n as f64
        } else {
            i as f64 / (n.max(2) - 1) as f64
        };
        let (pos, color) = match s {
            Value::String(c) => (even, c.as_str()),
            Value::Object(o) => {
                let Some(c) = o.get("color").and_then(Value::as_str) else {
                    return perr(format!("{}: stop {i} has no color", p.id));
                };
                (o.get("at").and_then(Value::as_f64).unwrap_or(even), c)
            }
            _ => return perr(format!("{}: bad stop {i}", p.id)),
        };
        stops.push(Stop {
            pos,
            lab: parse_color(color)?,
        });
    }
    let category = match p.category.as_deref() {
        None => Category::User,
        Some(k) => Category::from_key(k).ok_or_else(|| PaletteError(format!("{}: unknown category {k}", p.id)))?,
    };
    let mut out = LibraryPalette::new(&p.id, p.name.as_deref().unwrap_or(&p.id), category, stops);
    out.cyclic = cyclic;
    out.smooth = p.smooth.unwrap_or(true);
    out.density = p.density.unwrap_or(0.02);
    out.offset = p.offset.unwrap_or(0.0);
    out.log = p.log.unwrap_or(false);
    if let Some(d) = p.deep {
        out.deep = hex_from_lab(parse_color(&d)?);
    }
    out.attribution = p.attribution.unwrap_or_default();
    out.license = p.license.unwrap_or_default();
    out.source = p.source.unwrap_or_default();
    Ok(out)
}

/// Parses a palette file: one palette object, or `{"palettes": [...]}`.
pub fn parse_json(text: &str) -> Result<Vec<LibraryPalette>, PaletteError> {
    let v: Value = serde_json::from_str(text).map_err(|e| PaletteError(e.to_string()))?;
    match v {
        Value::Object(mut o) if o.contains_key("palettes") => match o.remove("palettes") {
            Some(Value::Array(list)) => list.into_iter().map(from_json_value).collect(),
            _ => perr("palettes is not a list"),
        },
        v @ Value::Object(_) => Ok(vec![from_json_value(v)?]),
        _ => perr("not a palette object"),
    }
}

fn hex_text(hex: u32) -> String {
    format!("#{hex:06X}")
}

impl LibraryPalette {
    /// Parses one palette (the user-palette format).
    pub fn from_json(text: &str) -> Result<LibraryPalette, PaletteError> {
        let mut all = parse_json(text)?;
        if all.len() != 1 {
            return perr(format!("expected one palette, found {}", all.len()));
        }
        Ok(all.remove(0))
    }

    /// The palette as JSON (stops as `oklch(...)`, lossless to 4 decimals), readable by
    /// [`Self::from_json`].
    pub fn to_json(&self) -> String {
        let stops: Vec<Value> = self
            .stops
            .iter()
            .map(|s| {
                let [l, a, b] = s.lab;
                let c = a.hypot(b);
                let h = b.atan2(a).to_degrees().rem_euclid(360.0);
                serde_json::json!({ "at": round(s.pos, 6), "color": format!("oklch({l:.4} {c:.4} {h:.2})") })
            })
            .collect();
        let v = serde_json::json!({
            "almondbread_palette": 1,
            "id": self.id,
            "name": self.name,
            "category": self.category.key(),
            "cyclic": self.cyclic,
            "smooth": self.smooth,
            "density": self.density,
            "offset": self.offset,
            "log": self.log,
            "deep": hex_text(self.deep),
            "attribution": self.attribution,
            "license": self.license,
            "source": self.source,
            "stops": stops,
        });
        serde_json::to_string_pretty(&v).expect("json")
    }
}

fn round(x: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (x * f).round() / f
}

// MARK: registry

const DESIGNED: &str = include_str!("../palettes/designed.json");
const DERIVED: &str = include_str!("../palettes/derived.json");

/// The built-in library palettes, in menu order (by category, then file order).
pub fn builtin() -> &'static [LibraryPalette] {
    static LIB: OnceLock<Vec<LibraryPalette>> = OnceLock::new();
    LIB.get_or_init(|| {
        let mut all = parse_json(DESIGNED).expect("designed.json");
        for p in &mut all {
            if p.license.is_empty() {
                p.license = "CC0-1.0".into();
                p.attribution = "almondbread (own design)".into();
            }
        }
        all.extend(parse_json(DERIVED).expect("derived.json"));
        let rank = |c: Category| Category::ALL.iter().position(|&x| x == c).unwrap_or(usize::MAX);
        all.sort_by_key(|p| rank(p.category));
        all
    })
}

fn user_store() -> &'static RwLock<HashMap<String, &'static LibraryPalette>> {
    static USER: OnceLock<RwLock<HashMap<String, &'static LibraryPalette>>> = OnceLock::new();
    USER.get_or_init(Default::default)
}

/// Prefix of user palette ids (keeps them apart from built-in ids).
pub const USER_PREFIX: &str = "user:";

/// Registers (or replaces) a user palette so [`find`] and the renderer (`Palette::named`)
/// see it. The id gets the [`USER_PREFIX`] and the category becomes `User`. Returns the
/// stored palette. Registered palettes live for the process: replacing one keeps the old
/// copy alive (a few KB per edit) because renderer palettes borrow `'static` names.
pub fn register_user(mut p: LibraryPalette) -> &'static LibraryPalette {
    if !p.id.starts_with(USER_PREFIX) {
        p.id = format!("{USER_PREFIX}{}", p.id);
    }
    p.category = Category::User;
    let stored: &'static LibraryPalette = Box::leak(Box::new(p));
    user_store()
        .write()
        .expect("palette store")
        .insert(stored.id.clone(), stored);
    stored
}

/// Removes a user palette from the registry.
pub fn unregister_user(id: &str) -> bool {
    user_store().write().expect("palette store").remove(id).is_some()
}

/// A palette by id: built-in library first, then user palettes.
pub fn find(id: &str) -> Option<&'static LibraryPalette> {
    builtin()
        .iter()
        .find(|p| p.id == id)
        .or_else(|| user_store().read().expect("palette store").get(id).copied())
}

/// Every palette, built-in then user (user sorted by name).
pub fn all() -> Vec<&'static LibraryPalette> {
    let mut user: Vec<&'static LibraryPalette> =
        user_store().read().expect("palette store").values().copied().collect();
    user.sort_by(|a, b| a.name.cmp(&b.name));
    builtin().iter().chain(user).collect()
}

/// The palettes of one category.
pub fn in_category(c: Category) -> Vec<&'static LibraryPalette> {
    all().into_iter().filter(|p| p.category == c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_parses_and_ids_are_unique() {
        let lib = builtin();
        assert!((40..=80).contains(&lib.len()), "{} palettes", lib.len());
        let mut ids: Vec<&str> = lib.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), lib.len(), "duplicate ids");
        // No id shadows (or is shadowed by) a built-in renderer palette.
        for p in Palette::all() {
            assert!(find(p.id).is_none(), "{} collides", p.id);
        }
        for p in lib {
            assert!(p.stops.len() >= 2, "{}", p.id);
            assert!(p.density > 0.0, "{}", p.id);
            assert!(!p.license.is_empty() && !p.attribution.is_empty(), "{}", p.id);
        }
    }

    #[test]
    fn every_category_but_user_is_filled() {
        for c in Category::ALL {
            let n = in_category(c).len();
            if c != Category::User {
                assert!(n >= 3, "{c:?}: {n}");
            }
        }
    }

    #[test]
    fn cyclic_palettes_wrap_without_a_seam() {
        // Neighbouring table entries differ little everywhere, the wrap included; hard-edged
        // palettes (zebra and the like) are allowed their steep but continuous ramps.
        for p in builtin() {
            let t = p.table(4096);
            let mut worst = 0.0f32;
            for i in 0..4096 {
                let (a, b) = (t[i], t[(i + 1) % 4096]);
                worst = worst.max((0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max));
            }
            assert!(worst < 0.08, "{}: step {worst}", p.id);
        }
    }

    #[test]
    fn stops_are_hit() {
        for p in builtin().iter().filter(|p| p.cyclic) {
            for s in &p.stops {
                let c = p.color_at(s.pos);
                for k in 0..3 {
                    assert!((c[k] - s.lab[k]).abs() < 1e-9, "{} at {}", p.id, s.pos);
                }
            }
        }
        // Sequential: first stop at 0, last at 0.5.
        let v = find("viridis").unwrap();
        assert!(!v.cyclic);
        let first = v.stops[0].lab;
        let last = v.stops.last().unwrap().lab;
        let d = |a: [f64; 3], b: [f64; 3]| (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f64::max);
        assert!(d(v.color_at(0.0), first) < 1e-9);
        assert!(d(v.color_at(0.5), last) < 1e-9);
        assert!(d(v.color_at(0.25), v.color_at(0.75)) < 1e-9, "mirrored");
    }

    #[test]
    fn json_round_trip() {
        let p = find("royal-gold").unwrap();
        let back = LibraryPalette::from_json(&p.to_json()).unwrap();
        assert_eq!(back.id, p.id);
        assert_eq!(back.category, p.category);
        assert_eq!(back.deep, p.deep);
        assert_eq!(back.stops.len(), p.stops.len());
        for (a, b) in back.stops.iter().zip(&p.stops) {
            assert!((a.pos - b.pos).abs() < 1e-6);
            for k in 0..3 {
                assert!((a.lab[k] - b.lab[k]).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn minimal_user_json() {
        let p = LibraryPalette::from_json(r##"{"id": "mine", "stops": ["#000000", "oklch(0.8 0.1 90)", "#FFFFFF"]}"##)
            .unwrap();
        assert_eq!(p.category, Category::User);
        assert!(p.cyclic);
        assert!((p.stops[1].pos - 1.0 / 3.0).abs() < 1e-12);
        assert!(LibraryPalette::from_json(r#"{"id": "x", "stops": ["nope"]}"#).is_err());
        assert!(LibraryPalette::from_json(r#"{"stops": []}"#).is_err());
        let stored = register_user(p);
        assert_eq!(stored.id, "user:mine");
        assert_eq!(find("user:mine").unwrap().name, "mine");
        // The renderer path: Palette::named reaches user and library palettes.
        assert_eq!(Palette::named("user:mine").id, "user:mine");
        assert_eq!(Palette::named("batlow").id, "batlow");
        assert!(unregister_user("user:mine"));
        assert!(find("user:mine").is_none());
    }

    #[test]
    fn oklch_is_gamut_mapped() {
        // Far outside sRGB: chroma shrinks, lightness and hue stay.
        let lab = lab_from_oklch(0.7, 0.4, 145.0);
        let rgb = linear_unclamped(lab);
        assert!(rgb.iter().all(|&v| (-1e-5..=1.00001).contains(&v)), "{rgb:?}");
        assert!((lab[0] - 0.7).abs() < 1e-12);
        let h = lab[2].atan2(lab[1]).to_degrees();
        assert!((h - 145.0).abs() < 1e-6);
    }

    #[test]
    fn renderer_palette_matches_table() {
        let p = find("batlow").unwrap();
        let ours = p.table(1024);
        let theirs = p.to_palette().table(1024);
        for i in 0..1024 {
            for k in 0..3 {
                assert!((ours[i][k] - theirs[i][k]).abs() < 0.01, "entry {i}");
            }
        }
    }
}
