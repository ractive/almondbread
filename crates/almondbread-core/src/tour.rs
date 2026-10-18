//! Guided tours (kb: decisions/controls "Guided tours"): a tour is a Markdown file, one per tour
//! in `crates/almondbread-core/tours/<id>.md`, read and checked here so every UI gets the same
//! tours. A tour is a list of steps; a step can set the view (flown to or jumped to), the look,
//! palette, interior colouring and iteration limit, switch overlays, put the orbit probe on a
//! fixed point or move it along a path, mark a point, and says what to look at in a short text.
//!
//! Format (the kb page has the full key list):
//!
//! ```text
//! ---
//! id: feigenbaum
//! title: The Feigenbaum point
//! summary: Period doubling into chaos along the spike.
//! place: feigenbaum-point
//! ---
//!
//! ## A fixed point
//! center: -0.45, 0
//! span: 2.8
//! orbit: on
//! probe: -0.2, 0
//!
//! The text of the step (Markdown).
//! ```
//!
//! The lines right under a step's `## ` heading, up to the first blank line, are its settings;
//! the rest is its text. Settings carry over to later steps until a step changes them, so going
//! back to a step shows exactly what it showed the first time ([`Tour::effective`]).
//!
//! Playback state ([`TourPlayer`]): the user's settings saved at the start (restored at the end),
//! the step and when it was entered (the probe path is a closed form of the time since then, so
//! it moves the same at any refresh rate).

use crate::orbit::MAX_ORBIT_POINTS;
use crate::palette::{Look, Palette};
use crate::pi_moment::PiPoint;
use crate::settings::{InteriorMode, OverlaySettings, RenderSettings};
use crate::viewport::{Center, Scale, Viewport, wrap_angle};

/// The bundled tours: (id, Markdown). The id is the file name; a unit test checks that every
/// file in `tours/` is listed here and parses.
const BUNDLED: &[(&str, &str)] = &[
    ("feigenbaum", include_str!("../tours/feigenbaum.md")),
    ("minibrot-periods", include_str!("../tours/minibrot-periods.md")),
    ("pi", include_str!("../tours/pi.md")),
    ("limits", include_str!("../tours/limits.md")),
    ("bulbs", include_str!("../tours/bulbs.md")),
    ("valleys", include_str!("../tours/valleys.md")),
    ("doubling", include_str!("../tours/doubling.md")),
    ("colours", include_str!("../tours/colours.md")),
];

/// Highest iteration limit a step may set (the FFI's bound).
pub const MAX_STEP_ITERATIONS: usize = 1 << 31;
/// Longest probe pass, seconds.
pub const MAX_PROBE_SECONDS: f64 = 600.0;

/// A tour: its metadata and steps.
#[derive(Clone, Debug, PartialEq)]
pub struct Tour {
    pub id: String,
    pub title: String,
    /// One line for the tour list.
    pub summary: String,
    /// The place (`places::place_named`) whose detail offers this tour, if any.
    pub place: Option<String>,
    pub steps: Vec<Step>,
}

/// What a step shows: every field is optional ("unchanged").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    pub title: String,
    /// Markdown.
    pub text: String,
    /// Line of the step's heading in the file (error messages).
    pub line: usize,
    pub view: Option<StepView>,
    /// Jump to the view instead of flying there.
    pub jump: Option<bool>,
    pub look: Option<String>,
    pub palette: Option<String>,
    pub interior: Option<InteriorMode>,
    pub iterations: Option<Iterations>,
    pub orbit: Option<bool>,
    pub orbit_length: Option<usize>,
    pub map: Option<bool>,
    pub map_levels: Option<usize>,
    pub cosmic: Option<bool>,
    pub pi_meter: Option<bool>,
    /// The π meter's guided hunt at the neck or the cusp (as View ▸ Hunt π did): the view
    /// eases to the valley and the meter's probe slides down the line. Its own step only.
    pub pi_hunt: Option<PiPoint>,
    pub compass: Option<bool>,
    pub probe: Option<Probe>,
    /// `Some(None)`: the marker goes away.
    pub marker: Option<Option<Marker>>,
}

/// A view: centre (decimal strings of any precision), the extent across the window's shorter
/// side (like a place's span) and the turn.
#[derive(Clone, Debug, PartialEq)]
pub struct StepView {
    pub center: Center,
    pub span: Scale,
    /// Degrees, counter-clockwise.
    pub angle: f64,
}

impl StepView {
    /// The view in a window whose shorter side is `short_side` points.
    pub fn viewport(&self, short_side: f64) -> Viewport {
        let mut v = Viewport::with_center(self.center.clone(), self.span / short_side.max(1.0))
            .with_angle(wrap_angle(self.angle.to_radians()));
        v.scale = v
            .scale
            .max(v.min_scale())
            .min(Scale::from_f64(crate::viewport::MAX_SCALE));
        v.ensure_precision();
        v
    }
}

/// The iteration limit a step sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Iterations {
    Auto,
    Fixed(usize),
}

/// Where the orbit overlay's point c is.
#[derive(Clone, Debug, PartialEq)]
pub enum Probe {
    /// Under the pointer (the crosshair in keyboard mode): the overlay's usual behaviour.
    Pointer,
    /// A fixed point.
    Fixed(Center),
    /// Moving along a path.
    Path(ProbePath),
}

/// How a probe path is run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathMotion {
    /// Start to end, then rest at the end.
    Once,
    /// Start to end and back, again and again (eased at both ends).
    Bounce,
    /// Start to end, then from the start again.
    Loop,
}

/// A polyline of points (f64: the probe paths of tours are shallow) run in `seconds` per pass,
/// each segment taking the same time: put points closer together where c should move slower.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbePath {
    pub points: Vec<(f64, f64)>,
    pub seconds: f64,
    pub motion: PathMotion,
}

impl ProbePath {
    /// The point `t` seconds after the step began: a closed form of `t`, so the motion is the
    /// same at any refresh rate.
    pub fn at(&self, t: f64) -> (f64, f64) {
        let n = self.points.len();
        if n == 0 {
            return (0.0, 0.0);
        }
        if n == 1 || !t.is_finite() {
            return self.points[0];
        }
        let x = (t.max(0.0) / self.seconds.max(1e-3)).max(0.0);
        let u = match self.motion {
            // Smooth at the turns: (1 − cos πx) / 2 runs 0 → 1 → 0 with period 2.
            PathMotion::Bounce => (1.0 - (std::f64::consts::PI * x).cos()) / 2.0,
            PathMotion::Once if x >= 1.0 => 1.0,
            PathMotion::Once => (1.0 - (std::f64::consts::PI * x).cos()) / 2.0,
            PathMotion::Loop => x.fract(),
        };
        let s = u.clamp(0.0, 1.0) * (n - 1) as f64;
        let i = (s.floor() as usize).min(n - 2);
        let f = s - i as f64;
        let (a, b) = (self.points[i], self.points[i + 1]);
        (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
    }

    /// Still moving `t` seconds after the step began.
    pub fn moving(&self, t: f64) -> bool {
        self.points.len() > 1 && (self.motion != PathMotion::Once || t < self.seconds)
    }
}

/// A highlighted point with an optional label.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub at: Center,
    pub label: String,
}

/// A parse or validation error with its line (1-based) in the tour file.
#[derive(Clone, Debug, PartialEq)]
pub struct TourError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for TourError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TourError {}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, TourError> {
    Err(TourError {
        line,
        message: message.into(),
    })
}

/// Every key a step may set.
pub const STEP_KEYS: &[&str] = &[
    "center",
    "span",
    "zoom",
    "angle",
    "place",
    "move",
    "look",
    "palette",
    "interior",
    "iterations",
    "orbit",
    "orbit-length",
    "map",
    "map-levels",
    "cosmic",
    "pi-meter",
    "pi-hunt",
    "compass",
    "probe",
    "probe-path",
    "probe-seconds",
    "probe-motion",
    "marker",
    "marker-label",
];

/// Every key of the header (front matter).
pub const HEADER_KEYS: &[&str] = &["id", "title", "summary", "place"];

/// The bundled tours, parsed (a broken one is left out; the unit test keeps them all valid).
pub fn bundled() -> Vec<Tour> {
    BUNDLED.iter().filter_map(|(_, text)| parse(text).ok()).collect()
}

/// A bundled tour by id.
pub fn named(id: &str) -> Option<Tour> {
    BUNDLED
        .iter()
        .find(|(i, _)| *i == id)
        .and_then(|(_, text)| parse(text).ok())
}

/// The ids and sources of the bundled tours (tests, the CLI).
pub fn bundled_sources() -> &'static [(&'static str, &'static str)] {
    BUNDLED
}

fn on_off(line: usize, key: &str, v: &str) -> Result<bool, TourError> {
    match v {
        "on" | "yes" | "true" => Ok(true),
        "off" | "no" | "false" => Ok(false),
        _ => err(line, format!("`{key}` is `on` or `off`, not `{v}`")),
    }
}

fn number(line: usize, key: &str, v: &str) -> Result<f64, TourError> {
    match v.parse::<f64>() {
        Ok(x) if x.is_finite() => Ok(x),
        _ => err(line, format!("`{key}` needs a number, not `{v}`")),
    }
}

fn count(line: usize, key: &str, v: &str, max: usize) -> Result<usize, TourError> {
    match v.replace(['_', ' '], "").parse::<usize>() {
        Ok(n) if (1..=max).contains(&n) => Ok(n),
        _ => err(line, format!("`{key}` needs a whole number from 1 to {max}, not `{v}`")),
    }
}

/// "re, im" as a centre (decimal strings of any precision).
fn point(line: usize, key: &str, v: &str) -> Result<Center, TourError> {
    let mut parts = v.split(',');
    let (Some(re), Some(im), None) = (parts.next(), parts.next(), parts.next()) else {
        return err(line, format!("`{key}` is a point `re, im`, not `{v}`"));
    };
    match Center::parse(re.trim(), im.trim()) {
        Some(c) if c.re().is_finite() && c.im().is_finite() && c.re().abs() <= 4.0 && c.im().abs() <= 4.0 => Ok(c),
        _ => err(
            line,
            format!("`{key}`: `{v}` is not a point within 4 of 0 (`re, im`, decimals)"),
        ),
    }
}

fn parse_scale(line: usize, key: &str, v: &str) -> Result<Scale, TourError> {
    match Scale::parse(v) {
        Some(s) if s.log10().is_finite() && s.log10() <= 2.0 => Ok(s),
        _ => err(line, format!("`{key}` needs a positive number up to 100, not `{v}`")),
    }
}

/// Parses and checks a tour file. Every error names its line.
pub fn parse(text: &str) -> Result<Tour, TourError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    if lines.get(i).map(|l| l.trim()) != Some("---") {
        return err(i + 1, "a tour starts with a `---` header (id, title, summary)");
    }
    i += 1;
    let (mut id, mut title, mut summary, mut place) = (None, None, None, None);
    loop {
        let Some(raw) = lines.get(i) else {
            return err(i, "the header is not closed with `---`");
        };
        let line = i + 1;
        i += 1;
        let l = raw.trim();
        if l == "---" {
            break;
        }
        if l.is_empty() {
            continue;
        }
        let Some((k, v)) = l.split_once(':') else {
            return err(line, format!("`{l}` is not `key: value`"));
        };
        let (k, v) = (k.trim(), v.trim().to_string());
        let slot = match k {
            "id" => &mut id,
            "title" => &mut title,
            "summary" => &mut summary,
            "place" => &mut place,
            _ => {
                return err(
                    line,
                    format!("unknown header key `{k}` (known: {})", HEADER_KEYS.join(", ")),
                );
            }
        };
        if slot.is_some() {
            return err(line, format!("`{k}` is given twice"));
        }
        if v.is_empty() {
            return err(line, format!("`{k}` is empty"));
        }
        *slot = Some(v);
    }
    let Some(id) = id else {
        return err(1, "the header needs an `id`");
    };
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return err(1, format!("the id `{id}` must be kebab-case (a-z, 0-9, -)"));
    }
    let Some(title) = title else {
        return err(1, "the header needs a `title`");
    };
    let Some(summary) = summary else {
        return err(1, "the header needs a `summary`");
    };
    if let Some(p) = &place
        && crate::places::place_named(p).is_none()
    {
        return err(1, format!("unknown place `{p}`"));
    }
    // Steps.
    let mut steps = Vec::new();
    while i < lines.len() {
        let l = lines[i].trim_end();
        if let Some(h) = l.strip_prefix("## ") {
            let line = i + 1;
            i += 1;
            let mut settings = Vec::new();
            while i < lines.len() && !lines[i].trim().is_empty() && !lines[i].starts_with("## ") {
                settings.push((i + 1, lines[i].trim()));
                i += 1;
            }
            let start = i;
            while i < lines.len() && !lines[i].starts_with("## ") {
                i += 1;
            }
            let body = lines[start..i].join("\n").trim().to_string();
            steps.push(parse_step(h.trim(), line, &settings, body)?);
        } else if l.trim().is_empty() {
            i += 1;
        } else {
            return err(i + 1, "text before the first step (`## Title`)");
        }
    }
    if steps.is_empty() {
        return err(lines.len(), "a tour needs at least one step (`## Title`)");
    }
    Ok(Tour {
        id,
        title,
        summary,
        place,
        steps,
    })
}

fn parse_step(title: &str, line: usize, settings: &[(usize, &str)], text: String) -> Result<Step, TourError> {
    if title.is_empty() {
        return err(line, "a step needs a title after `## `");
    }
    if text.is_empty() {
        return err(
            line,
            "a step needs text after its settings (a blank line, then the text)",
        );
    }
    let mut step = Step {
        title: title.to_string(),
        text,
        line,
        ..Step::default()
    };
    let mut seen: Vec<&str> = Vec::new();
    let get = |k: &str| -> Option<(usize, &str)> {
        settings
            .iter()
            .find(|(_, l)| key_of(l) == Some(k))
            .map(|(n, l)| (*n, value_of(l)))
    };
    for (n, l) in settings {
        let Some(k) = key_of(l) else {
            return err(
                *n,
                format!("`{l}` is not `key: value` (settings end at the first blank line)"),
            );
        };
        if !STEP_KEYS.contains(&k) {
            return err(*n, format!("unknown key `{k}` (known: {})", STEP_KEYS.join(", ")));
        }
        if seen.contains(&k) {
            return err(*n, format!("`{k}` is given twice"));
        }
        if value_of(l).is_empty() {
            return err(*n, format!("`{k}` is empty"));
        }
        seen.push(k);
    }
    // View: place, or centre with span or zoom; angle.
    let center = get("center").map(|(n, v)| point(n, "center", v)).transpose()?;
    let span = get("span").map(|(n, v)| parse_scale(n, "span", v)).transpose()?;
    // Zoom: magnification of the whole set (span 3), like a place's zoom; in logs, so a zoom
    // beyond f64 ("1e400") works.
    let zoom = get("zoom")
        .map(|(n, v)| match Scale::parse(v) {
            Some(z) if z.log10().is_finite() && z.log10() >= -2.0 => Ok(Scale::from_log2((3.0f64).log2() - z.log2())),
            _ => err(n, format!("`zoom` needs a magnification of at least 0.01, not `{v}`")),
        })
        .transpose()?;
    let angle = get("angle").map(|(n, v)| number(n, "angle", v)).transpose()?;
    let place = match get("place") {
        Some((n, v)) => match crate::places::place_named(v) {
            Some(p) => Some(p),
            None => return err(n, format!("unknown place `{v}`")),
        },
        None => None,
    };
    if let Some(p) = place {
        if let Some((n, _)) = get("center").or(get("span")).or(get("zoom")) {
            return err(n, "`place` sets the view: no `center`, `span` or `zoom` with it");
        }
        step.view = Some(StepView {
            center: p.center(),
            span: p.span_scale(),
            angle: angle.unwrap_or(0.0),
        });
        step.look = Some(p.look.to_string());
    } else {
        match (center, span, zoom) {
            (Some(c), Some(s), None) | (Some(c), None, Some(s)) => {
                step.view = Some(StepView {
                    center: c,
                    span: s,
                    angle: angle.unwrap_or(0.0),
                });
            }
            (Some(_), Some(_), Some(_)) => {
                return err(get("zoom").expect("zoom").0, "`span` or `zoom`, not both");
            }
            (Some(_), None, None) => {
                return err(get("center").expect("center").0, "`center` needs a `span` or a `zoom`");
            }
            (None, Some(_), _) | (None, _, Some(_)) => {
                let n = get("span").or(get("zoom")).expect("span or zoom").0;
                return err(n, "`span` and `zoom` need a `center`");
            }
            (None, None, None) => {
                if let Some((n, _)) = get("angle") {
                    return err(n, "`angle` needs a view (`center` with `span` or `zoom`, or `place`)");
                }
            }
        }
    }
    if let Some((n, v)) = get("move") {
        if step.view.is_none() {
            return err(n, "`move` needs a view in the same step");
        }
        step.jump = Some(match v {
            "fly" => false,
            "jump" => true,
            _ => return err(n, format!("`move` is `fly` or `jump`, not `{v}`")),
        });
    }
    if let Some((n, v)) = get("look") {
        if Look::named(v).is_none() {
            let known: Vec<&str> = Look::all().iter().map(|l| l.id).collect();
            return err(n, format!("unknown look `{v}` (known: {})", known.join(", ")));
        }
        step.look = Some(v.to_string());
    }
    if let Some((n, v)) = get("palette") {
        let builtin = Palette::all().iter().any(|p| p.id == v);
        if !builtin && crate::palette_library::find(v).is_none() {
            return err(n, format!("unknown palette `{v}`"));
        }
        step.palette = Some(v.to_string());
    }
    if let Some((n, v)) = get("interior") {
        step.interior = Some(match v {
            "flat" => InteriorMode::Flat,
            "bubbles" => InteriorMode::Bubbles,
            "period" => InteriorMode::Period,
            "angle" => InteriorMode::Angle,
            "distance" => InteriorMode::Distance,
            _ => {
                return err(
                    n,
                    format!("`interior` is flat, bubbles, period, angle or distance, not `{v}`"),
                );
            }
        });
    }
    if let Some((n, v)) = get("iterations") {
        step.iterations = Some(if v == "auto" {
            Iterations::Auto
        } else {
            Iterations::Fixed(count(n, "iterations", v, MAX_STEP_ITERATIONS)?)
        });
    }
    let flag = |k: &str| get(k).map(|(n, v)| on_off(n, k, v)).transpose();
    step.orbit = flag("orbit")?;
    step.map = flag("map")?;
    step.cosmic = flag("cosmic")?;
    step.pi_meter = flag("pi-meter")?;
    if let Some((n, v)) = get("pi-hunt") {
        let Some(p) = PiPoint::named(v) else {
            return err(n, format!("`pi-hunt` is neck or cusp, not `{v}`"));
        };
        if step.view.is_some() {
            return err(
                n,
                "`pi-hunt` flies its own view: no `center`, `span`, `zoom` or `place` with it",
            );
        }
        if step.pi_meter == Some(false) {
            return err(n, "`pi-hunt` needs the π meter: no `pi-meter: off` with it");
        }
        step.pi_hunt = Some(p);
        step.pi_meter = Some(true);
    }
    step.compass = flag("compass")?;
    step.orbit_length = get("orbit-length")
        .map(|(n, v)| count(n, "orbit-length", v, MAX_ORBIT_POINTS))
        .transpose()?;
    step.map_levels = get("map-levels")
        .map(|(n, v)| count(n, "map-levels", v, 4))
        .transpose()?;
    // Probe: pointer, a point, or a path.
    match (get("probe"), get("probe-path")) {
        (Some(_), Some((n, _))) => return err(n, "`probe` or `probe-path`, not both"),
        (Some((n, v)), None) => {
            step.probe = Some(if v == "pointer" {
                Probe::Pointer
            } else {
                Probe::Fixed(point(n, "probe", v)?)
            });
        }
        (None, Some((n, v))) => {
            let mut points = Vec::new();
            for p in v.split("->") {
                let c = point(n, "probe-path", p.trim())?;
                points.push((c.re(), c.im()));
            }
            if points.len() < 2 {
                return err(n, "`probe-path` needs at least two points: `re, im -> re, im`");
            }
            let seconds = match get("probe-seconds") {
                Some((m, s)) => match number(m, "probe-seconds", s)? {
                    x if x > 0.0 && x <= MAX_PROBE_SECONDS => x,
                    _ => return err(m, format!("`probe-seconds` is from 0 to {MAX_PROBE_SECONDS}")),
                },
                None => 10.0,
            };
            let motion = match get("probe-motion") {
                Some((_, "bounce")) | None => PathMotion::Bounce,
                Some((_, "once")) => PathMotion::Once,
                Some((_, "loop")) => PathMotion::Loop,
                Some((m, s)) => return err(m, format!("`probe-motion` is bounce, once or loop, not `{s}`")),
            };
            step.probe = Some(Probe::Path(ProbePath {
                points,
                seconds,
                motion,
            }));
        }
        (None, None) => {}
    }
    if !matches!(step.probe, Some(Probe::Path(_))) {
        for k in ["probe-seconds", "probe-motion"] {
            if let Some((n, _)) = get(k) {
                return err(n, format!("`{k}` needs a `probe-path` in the same step"));
            }
        }
    }
    match get("marker") {
        Some((_, "off")) => {
            if let Some((n, _)) = get("marker-label") {
                return err(n, "`marker-label` needs a `marker` point");
            }
            step.marker = Some(None);
        }
        Some((n, v)) => {
            step.marker = Some(Some(Marker {
                at: point(n, "marker", v)?,
                label: get("marker-label").map(|(_, l)| l.to_string()).unwrap_or_default(),
            }));
        }
        None => {
            if let Some((n, _)) = get("marker-label") {
                return err(n, "`marker-label` needs a `marker` point");
            }
        }
    }
    Ok(step)
}

/// The key of a settings line (`key: value`), if it is one.
fn key_of(l: &str) -> Option<&str> {
    let (k, _) = l.split_once(':')?;
    let k = k.trim();
    (!k.is_empty() && k.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')).then_some(k)
}

fn value_of(l: &str) -> &str {
    l.split_once(':').map_or("", |(_, v)| v.trim())
}

/// Everything a tour changes and restores: the image settings, the overlays and the three
/// overlay switches kept outside `OverlaySettings`.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub settings: RenderSettings,
    pub overlay: OverlaySettings,
    pub cosmic: bool,
    pub pi_meter: bool,
    pub compass: bool,
}

impl Step {
    /// `base` with this step's settings applied: look first, then palette, interior,
    /// iterations, overlays.
    pub fn apply(&self, base: &Snapshot) -> Snapshot {
        let mut out = base.clone();
        let s = &mut out.settings;
        if let Some(look) = self.look.as_deref().and_then(Look::named) {
            look.apply(s);
        }
        if let Some(p) = &self.palette {
            if let Some(lib) = crate::palette_library::find(p) {
                lib.apply(s);
            } else {
                s.palette_id = p.clone();
            }
            s.palette_stops = None;
        }
        if let Some(m) = self.interior {
            s.interior_mode = m;
        }
        match self.iterations {
            Some(Iterations::Auto) => s.auto_iterations = true,
            Some(Iterations::Fixed(n)) => {
                s.auto_iterations = false;
                s.manual_iterations = n;
            }
            None => {}
        }
        let o = &mut out.overlay;
        if let Some(v) = self.orbit {
            o.orbit = v;
        }
        if let Some(v) = self.orbit_length {
            o.orbit_length = v;
        }
        if let Some(v) = self.map {
            o.map = v;
        }
        if let Some(v) = self.map_levels {
            o.map_levels = v;
        }
        if let Some(v) = self.cosmic {
            out.cosmic = v;
        }
        if let Some(v) = self.pi_meter {
            out.pi_meter = v;
        }
        if let Some(v) = self.compass {
            out.compass = v;
        }
        out
    }
}

impl Tour {
    /// Step `index` with everything earlier steps set and it does not change (settings carry
    /// over; `jump` does not: it belongs to the step's own view).
    pub fn effective(&self, index: usize) -> Step {
        let index = index.min(self.steps.len().saturating_sub(1));
        let mut e = self.steps[index].clone();
        // A look sets the palette too: a palette from before the latest look does not count.
        let mut look_found = e.look.is_some();
        for s in self.steps[..index].iter().rev() {
            macro_rules! inherit {
                ($($f:ident),*) => { $( if e.$f.is_none() { e.$f = s.$f.clone(); } )* };
            }
            inherit!(
                view,
                interior,
                iterations,
                orbit,
                orbit_length,
                map,
                map_levels,
                cosmic,
                pi_meter,
                compass,
                probe,
                marker
            );
            // (Within a step the look comes first, then the palette.)
            if e.palette.is_none() && !look_found {
                e.palette = s.palette.clone();
            }
            if e.look.is_none() && s.look.is_some() {
                e.look = s.look.clone();
                look_found = true;
            }
        }
        if self.steps[index].view.is_none() {
            e.jump = None;
        }
        e
    }
}

/// What entering a step does: the settings to show and where the view goes.
#[derive(Clone, Debug, PartialEq)]
pub struct StepPlan {
    pub snapshot: Snapshot,
    /// The view to go to and whether to jump (else fly); None: the view stays.
    pub view: Option<(StepView, bool)>,
    /// Start the π meter's guided hunt there (it moves the view itself).
    pub pi_hunt: Option<PiPoint>,
}

/// A tour being played.
#[derive(Clone, Debug)]
pub struct TourPlayer {
    pub tour: Tour,
    index: usize,
    /// When the step was entered (the probe path's clock).
    entered: f64,
    saved: Snapshot,
    saved_view: Viewport,
    /// The current step, resolved (settings carried over).
    current: Step,
}

impl TourPlayer {
    /// Starts at the first step; `saved` and `view` are what the tour restores at its end.
    /// Returns the player and the first step's plan.
    pub fn start(tour: Tour, saved: Snapshot, view: Viewport, now: f64) -> (Self, StepPlan) {
        let current = tour.effective(0);
        let mut p = Self {
            tour,
            index: 0,
            entered: now,
            saved,
            saved_view: view,
            current,
        };
        let plan = p.go(0, now);
        (p, plan)
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn len(&self) -> usize {
        self.tour.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tour.steps.is_empty()
    }

    /// The current step as authored (title, text).
    pub fn step(&self) -> &Step {
        &self.tour.steps[self.index]
    }

    /// The current step with the settings carried over from earlier ones.
    pub fn current(&self) -> &Step {
        &self.current
    }

    /// The look the current step shows (its own or carried over), if any.
    pub fn look(&self) -> Option<&str> {
        self.current.look.as_deref()
    }

    /// Enters step `index` (clamped): its settings on top of the saved ones, its view if it
    /// sets one (going back to a step without one returns to the view it showed).
    pub fn go(&mut self, index: usize, now: f64) -> StepPlan {
        let index = index.min(self.tour.steps.len().saturating_sub(1));
        let back = index < self.index;
        self.index = index;
        self.entered = now;
        self.current = self.tour.effective(index);
        let own = self.tour.steps[index].view.is_some();
        // A π hunt flies its own view, also when coming back to its step.
        let hunt = self.tour.steps[index].pi_hunt.is_some();
        let view = if !hunt && (own || back) {
            self.current
                .view
                .clone()
                .map(|v| (v, self.tour.steps[index].jump.unwrap_or(false)))
        } else {
            None
        };
        StepPlan {
            snapshot: self.current.apply(&self.saved),
            view,
            pi_hunt: self.tour.steps[index].pi_hunt,
        }
    }

    /// The probe's point at `now` (None: the pointer).
    pub fn probe_at(&self, now: f64) -> Option<Center> {
        match &self.current.probe {
            None | Some(Probe::Pointer) => None,
            Some(Probe::Fixed(c)) => Some(c.clone()),
            Some(Probe::Path(p)) => {
                let (re, im) = p.at(now - self.entered);
                Some(Center::new(re, im))
            }
        }
    }

    /// The probe moves: keep frames coming.
    pub fn animating(&self, now: f64) -> bool {
        matches!(&self.current.probe, Some(Probe::Path(p)) if p.moving(now - self.entered))
    }

    pub fn marker(&self) -> Option<&Marker> {
        self.current.marker.as_ref().and_then(|m| m.as_ref())
    }

    /// Ends the tour: the settings and the view to restore.
    pub fn finish(self) -> (Snapshot, Viewport) {
        (self.saved, self.saved_view)
    }

    /// The saved settings (what the end restores).
    pub fn saved(&self) -> &Snapshot {
        &self.saved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            settings: RenderSettings::default(),
            overlay: OverlaySettings::default(),
            cosmic: true,
            pi_meter: false,
            compass: false,
        }
    }

    const SMALL: &str = "---\nid: t\ntitle: T\nsummary: S\n---\n\n## One\ncenter: -0.5, 0\nspan: 3\norbit: on\nprobe: -0.2, 0\niterations: 64\nlook: ember\n\nFirst *text*.\n\n## Two\nprobe-path: -0.7, 0 -> -1.2, 0 -> -1.4, 0\nprobe-seconds: 4\ninterior: period\nmarker: -1.4, 0\nmarker-label: here\n\nSecond.\n\n## Three\ncenter: -1.75, 0\nzoom: 100\nmove: jump\niterations: auto\nprobe: pointer\nmarker: off\n\nThird.\n";

    #[test]
    fn every_bundled_tour_parses_and_every_file_is_listed() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tours");
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .expect("tours dir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".md"))
            .collect();
        files.sort();
        let mut listed: Vec<String> = BUNDLED.iter().map(|(id, _)| format!("{id}.md")).collect();
        listed.sort();
        assert_eq!(files, listed, "every tours/*.md is listed in BUNDLED");
        for (id, text) in BUNDLED {
            let tour = parse(text).unwrap_or_else(|e| panic!("tour {id}: {e}"));
            assert_eq!(tour.id, *id, "the id is the file name");
            assert!(!tour.steps.is_empty());
            let saved = snapshot();
            for (i, step) in tour.steps.iter().enumerate() {
                assert!(!step.title.is_empty() && !step.text.is_empty(), "{id} step {i}");
                // Every step resolves to settings and a view that fits a window.
                let e = tour.effective(i);
                let s = e.apply(&saved);
                assert!(
                    s.overlay.clone().sanitized() == s.overlay,
                    "{id} step {i}: overlay in range"
                );
                if let Some(v) = &e.view {
                    let vp = v.viewport(800.0);
                    assert!(vp.scale.to_f64() > 0.0 && vp.center.re().is_finite());
                }
                if let Some(Probe::Path(p)) = &e.probe {
                    for k in 0..=40 {
                        let (re, im) = p.at(k as f64 * p.seconds / 20.0);
                        assert!(re.is_finite() && im.is_finite());
                    }
                }
            }
            assert_eq!(named(id).as_ref(), Some(&tour));
        }
        assert_eq!(bundled().len(), BUNDLED.len());
    }

    #[test]
    fn parses_steps_and_carries_settings_over() {
        let t = parse(SMALL).expect("parses");
        assert_eq!((t.id.as_str(), t.title.as_str(), t.summary.as_str()), ("t", "T", "S"));
        assert_eq!(t.steps.len(), 3);
        assert_eq!(t.steps[0].text, "First *text*.");
        assert_eq!(t.steps[0].iterations, Some(Iterations::Fixed(64)));
        assert_eq!(t.steps[2].jump, Some(true));
        // Zoom 100 = span 3 / 100.
        let v = t.steps[2].view.as_ref().expect("view");
        assert!((v.span.to_f64() - 0.03).abs() < 1e-12);
        // Step two inherits the view, the orbit, the look and the iterations of step one.
        let e = t.effective(1);
        assert_eq!(e.view, t.steps[0].view);
        assert_eq!(e.orbit, Some(true));
        assert_eq!(e.look.as_deref(), Some("ember"));
        assert_eq!(e.iterations, Some(Iterations::Fixed(64)));
        assert_eq!(e.interior, Some(InteriorMode::Period));
        assert!(matches!(e.probe, Some(Probe::Path(_))));
        assert_eq!(
            e.marker.as_ref().and_then(|m| m.as_ref()).map(|m| m.label.as_str()),
            Some("here")
        );
        // Step three switches the probe back to the pointer and the marker off.
        let e = t.effective(2);
        assert_eq!(e.probe, Some(Probe::Pointer));
        assert_eq!(e.marker, Some(None));
        assert_eq!(e.iterations, Some(Iterations::Auto));
    }

    #[test]
    fn a_pi_hunt_switches_the_meter_on_and_belongs_to_its_own_step() {
        let t = parse(
            "---\nid: t\ntitle: T\nsummary: S\n---\n\n## A\ncenter: -0.5, 0\nspan: 3\n\nA.\n\n\
             ## B\npi-hunt: neck\n\nB.\n\n## C\n\nC.\n",
        )
        .expect("parses");
        assert_eq!(t.steps[1].pi_hunt, Some(PiPoint::Neck));
        assert_eq!(t.steps[1].pi_meter, Some(true));
        let saved = Snapshot {
            settings: RenderSettings::default(),
            overlay: OverlaySettings::default(),
            cosmic: false,
            pi_meter: false,
            compass: false,
        };
        let (mut p, first) = TourPlayer::start(t, saved, Viewport::fitting(800.0, 500.0), 0.0);
        assert_eq!(first.pi_hunt, None);
        assert!(!first.snapshot.pi_meter);
        let b = p.go(1, 1.0);
        assert_eq!(
            (b.pi_hunt, b.view.is_none(), b.snapshot.pi_meter),
            (Some(PiPoint::Neck), true, true)
        );
        // The next step keeps the meter on but starts no hunt; going back starts it again.
        let c = p.go(2, 2.0);
        assert_eq!((c.pi_hunt, c.snapshot.pi_meter), (None, true));
        let back = p.go(1, 3.0);
        assert_eq!((back.pi_hunt, back.view.is_none()), (Some(PiPoint::Neck), true));
    }

    #[test]
    fn rejects_mistakes_with_their_line() {
        let head = "---\nid: t\ntitle: T\nsummary: S\n---\n\n## One\n";
        let cases: &[(&str, usize, &str)] = &[
            ("colour: red\n\nText.", 8, "unknown key `colour`"),
            ("center: -0.5, 0\n\nText.", 8, "needs a `span`"),
            ("center: x, 0\nspan: 1\n\nText.", 8, "not a point"),
            ("center: -0.5, 0\nspan: 1\nzoom: 3\n\nText.", 10, "not both"),
            ("look: nope\n\nText.", 8, "unknown look"),
            ("palette: nope\n\nText.", 8, "unknown palette"),
            ("place: nope\n\nText.", 8, "unknown place"),
            ("orbit: maybe\n\nText.", 8, "`on` or `off`"),
            ("orbit-length: 0\n\nText.", 8, "whole number"),
            ("iterations: lots\n\nText.", 8, "whole number"),
            ("probe-path: -1, 0\n\nText.", 8, "at least two points"),
            ("probe: -1, 0\nprobe-seconds: 3\n\nText.", 9, "needs a `probe-path`"),
            (
                "probe-path: -1, 0 -> -2, 0\nprobe-motion: spin\n\nText.",
                9,
                "bounce, once or loop",
            ),
            ("marker-label: x\n\nText.", 8, "needs a `marker`"),
            ("move: jump\n\nText.", 8, "needs a view"),
            ("pi-hunt: tail\n\nText.", 8, "neck or cusp"),
            (
                "center: -0.75, 0.1\nspan: 1\npi-hunt: neck\n\nText.",
                10,
                "flies its own view",
            ),
            ("pi-meter: off\npi-hunt: cusp\n\nText.", 9, "needs the π meter"),
            ("orbit: on\norbit: off\n\nText.", 9, "twice"),
            ("orbit: on\n", 7, "needs text"),
            ("Some text right under the heading.\n\nText.", 8, "not `key: value`"),
        ];
        for (body, line, msg) in cases {
            let e = parse(&format!("{head}{body}")).expect_err(body);
            assert_eq!(e.line, *line, "{body}: {e}");
            assert!(e.message.contains(msg), "{body}: {e}");
        }
        let e = parse("---\nid: t\ntitle: T\nsummary: S\nauthor: me\n---\n## A\n\nx").expect_err("header key");
        assert!(e.message.contains("unknown header key `author`"), "{e}");
        assert_eq!(e.line, 5);
        assert!(parse("## A\n\nx").is_err());
        assert!(parse("---\nid: t\ntitle: T\nsummary: S\n---\n").is_err());
        assert!(parse("---\nid: T x\ntitle: T\nsummary: S\n---\n## A\n\nx").is_err());
    }

    #[test]
    fn probe_path_is_the_same_at_any_refresh_rate() {
        let p = ProbePath {
            points: vec![(-0.7, 0.0), (-1.2, 0.0), (-1.4, 0.0)],
            seconds: 6.0,
            motion: PathMotion::Bounce,
        };
        // Ends and turns: start, the end at one pass, back at the start after two.
        assert_eq!(p.at(0.0), (-0.7, 0.0));
        assert!((p.at(6.0).0 + 1.4).abs() < 1e-12);
        assert!((p.at(12.0).0 + 0.7).abs() < 1e-12);
        // Halfway through the pass (eased): the middle point, each segment taking equal time.
        assert!((p.at(3.0).0 + 1.2).abs() < 1e-12);
        // Sampled by players stepping at 30, 60 and 120 Hz: the same point at the same time.
        let saved = Snapshot {
            settings: RenderSettings::default(),
            overlay: OverlaySettings::default(),
            cosmic: false,
            pi_meter: false,
            compass: false,
        };
        let tour = parse(SMALL).expect("tour");
        let mut at = Vec::new();
        for hz in [30.0, 60.0, 120.0] {
            let (mut player, _) = TourPlayer::start(tour.clone(), saved.clone(), Viewport::home(), 100.0);
            player.go(1, 100.0);
            let mut samples = Vec::new();
            let frames = (8.0 * hz) as usize;
            for k in 0..=frames {
                let now = 100.0 + k as f64 / hz;
                let c = player.probe_at(now).expect("probe");
                assert!(player.animating(now));
                if (k as f64 / hz * 2.0).fract().abs() < 1e-9 {
                    samples.push((c.re(), c.im()));
                }
            }
            at.push(samples);
        }
        assert_eq!(at[0].len(), 17);
        for k in 0..at[0].len() {
            for r in 1..3 {
                assert!(
                    (at[0][k].0 - at[r][k].0).abs() < 1e-12,
                    "sample {k}: {:?} vs {:?}",
                    at[0][k],
                    at[r][k]
                );
            }
        }
        // Monotone along the first pass (from -0.7 towards -1.4), within the path.
        let first: Vec<f64> = (0..=60).map(|k| p.at(k as f64 * 0.1).0).collect();
        assert!(first.windows(2).all(|w| w[1] <= w[0] + 1e-15));
        assert!(first.iter().all(|x| (-1.4 - 1e-12..=-0.7 + 1e-12).contains(x)));
        // Once: rests at the end; loop: starts over.
        let once = ProbePath {
            motion: PathMotion::Once,
            ..p.clone()
        };
        assert!((once.at(20.0).0 + 1.4).abs() < 1e-12);
        assert!(!once.moving(7.0) && once.moving(5.0));
        let lp = ProbePath {
            motion: PathMotion::Loop,
            ..p
        };
        assert!((lp.at(6.0).0 + 0.7).abs() < 1e-12);
    }

    #[test]
    fn steps_apply_their_settings_and_the_end_restores_the_saved_ones() {
        let tour = parse(SMALL).expect("tour");
        let mut saved = snapshot();
        saved.settings.auto_iterations = true;
        saved.settings.interior_mode = InteriorMode::Angle;
        saved.overlay.orbit = false;
        let view = Viewport::new(-0.1, 0.65, 1e-5);
        let (mut player, plan) = TourPlayer::start(tour, saved.clone(), view.clone(), 0.0);
        // Step one: its view (flown to), orbit on, 64 iterations, the ember look, the probe.
        let (v, jump) = plan.view.expect("view");
        assert!(!jump);
        assert!((v.center.re() + 0.5).abs() < 1e-15);
        assert!(plan.snapshot.overlay.orbit);
        assert!(!plan.snapshot.settings.auto_iterations);
        assert_eq!(plan.snapshot.settings.manual_iterations, 64);
        let mut ember = RenderSettings::default();
        Look::named("ember").expect("ember").apply(&mut ember);
        assert_eq!(plan.snapshot.settings.palette_id, ember.palette_id);
        assert_eq!(plan.snapshot.settings.interior_mode, InteriorMode::Angle, "unchanged");
        assert!(plan.snapshot.cosmic, "untouched switches keep the user's value");
        assert_eq!(player.probe_at(1.0).map(|c| c.re()), Some(-0.2));
        assert!(!player.animating(1.0));
        // Step two keeps the view where it is (no view of its own), the probe moves.
        let plan = player.go(1, 5.0);
        assert!(plan.view.is_none());
        assert_eq!(plan.snapshot.settings.interior_mode, InteriorMode::Period);
        assert!(player.animating(6.0));
        assert_eq!(player.marker().map(|m| m.label.as_str()), Some("here"));
        // Step three jumps; the probe follows the pointer again, the marker is gone.
        let plan = player.go(2, 9.0);
        assert!(plan.view.as_ref().is_some_and(|(_, j)| *j));
        assert!(plan.snapshot.settings.auto_iterations);
        assert_eq!(player.probe_at(9.5), None);
        assert!(player.marker().is_none());
        // Back to step two: the view step one set (flown to), its settings exactly again.
        let again = player.go(1, 12.0);
        let (v, jump) = again.view.expect("back returns to the step's view");
        assert!(!jump && (v.center.re() + 0.5).abs() < 1e-15);
        assert_eq!(again.snapshot.settings.manual_iterations, 64);
        assert!(!again.snapshot.settings.auto_iterations);
        // The end restores the saved settings and view.
        let (restored, back) = player.finish();
        assert_eq!(restored, saved);
        assert_eq!(back, view);
    }
}
