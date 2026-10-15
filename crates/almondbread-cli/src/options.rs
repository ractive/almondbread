//! `--key value` options shared by the commands; the same keys and defaults as the Swift
//! `CommandOptions`, so commands compare 1:1.

use std::collections::HashMap;

use almondbread_core::settings::{RenderSettings, Supersampling};
use almondbread_core::viewport::{Center, Scale, Viewport};
use almondbread_render::scheduler::BlaMode;
use almondbread_render::{GpuBackend, Renderer};

#[derive(Debug)]
pub struct CommandError(pub String);

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CommandError {}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Box::new(CommandError(msg.into())))
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    values: HashMap<String, String>,
    /// Arguments without `--` (e.g. the output path of `render`).
    pub positional: Vec<String>,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut o = Options::default();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            if let Some(key) = a.strip_prefix("--") {
                let Some(v) = args.get(i + 1) else {
                    return err(format!("missing value for {a}"));
                };
                o.values.insert(key.to_string(), v.clone());
                i += 2;
            } else {
                o.positional.push(a.clone());
                i += 1;
            }
        }
        Ok(o)
    }

    /// A copy with `--key value` set.
    pub fn with(&self, key: &str, value: &str) -> Options {
        let mut o = self.clone();
        o.values.insert(key.to_string(), value.to_string());
        o
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    pub fn usize(&self, key: &str) -> Option<usize> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    pub fn size(&self, key: &str, default: (usize, usize)) -> Result<(usize, usize)> {
        match self.get(key) {
            None => Ok(default),
            Some(s) => parse_size(s),
        }
    }

    pub fn pair(&self, key: &str) -> Result<Option<(f64, f64)>> {
        let Some(v) = self.get(key) else { return Ok(None) };
        let parts: Vec<f64> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if parts.len() != 2 {
            return err(format!("bad --{key} {v}"));
        }
        Ok(Some((parts[0], parts[1])))
    }

    /// Viewport from `--center re,im` (decimal strings of any length: M3b) and `--scale s`
    /// (complex units per pixel, any exponent: `1e-1000`); without a scale, the home view's
    /// 3.0 x 2.5 complex units are fitted into the image. `--location name` takes a benchmark
    /// location (deep ones included) at this image width; `--place id` a catalogue place
    /// (kb: synthesis/places), its span fitted to the shorter side. `--angle deg` turns any of
    /// them (view rotation, kb: decisions/controls).
    pub fn viewport(&self, width: usize, height: usize) -> Result<Viewport> {
        let v = self.viewport_unrotated(width, height)?;
        Ok(match self.get("angle") {
            Some(a) => match a.parse::<f64>() {
                Ok(deg) if deg.is_finite() => v.with_angle(deg.to_radians()),
                _ => return err(format!("bad --angle {a}")),
            },
            None => v,
        })
    }

    fn viewport_unrotated(&self, width: usize, height: usize) -> Result<Viewport> {
        if let Some(id) = self.get("place") {
            let Some(p) = almondbread_core::places::place_named(id) else {
                return err(format!("unknown --place {id} (see `almondbread places`)"));
            };
            return Ok(p.viewport(width, height));
        }
        if let Some(name) = self.get("location") {
            let Some(l) = almondbread_core::locations::location_named(name) else {
                return err(format!("unknown --location {name}"));
            };
            return Ok(l.viewport_for_width(width));
        }
        let mut center = Viewport::home().center;
        if let Some(v) = self.get("center") {
            let Some((re, im)) = v.split_once(',') else {
                return err(format!("bad --center {v}"));
            };
            center = match Center::parse(re, im) {
                Some(c) => c,
                None => return err(format!("bad --center {v}")),
            };
        }
        let scale = match self.get("scale") {
            Some(s) => match Scale::parse(s) {
                Some(s) => s,
                None => return err(format!("bad --scale {s}")),
            },
            None => Scale::from_f64((3.0 / width as f64).max(2.5 / height as f64)),
        };
        Ok(Viewport::with_center(center, scale))
    }

    /// The rendering settings shared by all commands. Headless tools render one sample per
    /// pixel unless `--ss` asks for a grid (the app's default anti-aliasing is progressive).
    pub fn apply_settings(&self, s: &mut RenderSettings) {
        // Headless tools: the plain M2.5 colouring unless a look is asked for (comparable
        // timings: lighting adds the distance estimate to every iteration).
        let iteration = (
            s.auto_iterations,
            s.manual_iterations,
            s.max_iterations_cap,
            s.interior_detection,
        );
        *s = RenderSettings::plain();
        (
            s.auto_iterations,
            s.manual_iterations,
            s.max_iterations_cap,
            s.interior_detection,
        ) = iteration;
        // A place brings its recommended look and iteration limit (`--look`, `--iterations`
        // override them).
        if let Some(p) = self.get("place").and_then(almondbread_core::places::place_named) {
            if let Some(look) = almondbread_core::palette::Look::named(p.look) {
                look.apply(s);
            }
            if let Some(n) = p.iterations {
                s.auto_iterations = false;
                s.manual_iterations = n;
                s.max_iterations_cap = s.max_iterations_cap.max(n);
            }
        }
        if let Some(l) = self.get("look") {
            match almondbread_core::palette::Look::named(l) {
                Some(look) => look.apply(s),
                None => eprintln!("almondbread: unknown look {l}"),
            }
        }
        s.supersampling = Supersampling::Off;
        if let Some(n) = self.usize("iterations") {
            s.auto_iterations = false;
            s.manual_iterations = n;
            s.max_iterations_cap = s.max_iterations_cap.max(n);
        }
        if let Some(v) = self.get("interior") {
            s.interior_detection = v != "0";
        }
        if let Some(v) = self.f64("shading") {
            s.shading = v;
        }
        if let Some(v) = self.f64("outline") {
            s.outline = v;
        }
        if let Some(v) = self.f64("density") {
            s.color_density = v;
        }
        if let Some(v) = self.f64("offset") {
            s.color_offset = v;
        }
        if let Some(v) = self.get("palette") {
            s.palette_id = v.to_string();
        }
        if self.get("log") == Some("1") {
            s.log_coloring = true;
        }
        if let Some(ss) = self.usize("ss").and_then(Supersampling::from_factor) {
            s.supersampling = ss;
        }
        if let Some(v) = self.f64("outline-width") {
            s.outline_width = v;
        }
        if let Some(v) = self.f64("specular") {
            s.specular = v;
        }
        if let Some(v) = self.f64("glow") {
            s.glow = v;
        }
        if let Some(v) = self.get("filter") {
            s.palette_filter = v != "0";
        }
        // Interior colouring (kb: techniques/coloring-methods): flat|bubbles|period|angle|distance.
        if let Some(v) = self.get("inside") {
            match almondbread_core::InteriorMode::parse(v) {
                Some(m) => s.interior_mode = m,
                None => eprintln!("almondbread: --inside {v}: expected flat, bubbles, period, angle or distance"),
            }
        }
        if let Some(v) = self.f64("inside-density") {
            s.interior_density = v;
        }
        if let Some(v) = self.f64("inside-stripes") {
            s.interior_stripes = v;
        }
        if let Some(v) = self.get("edge")
            && let Ok(hex) = u32::from_str_radix(v.trim_start_matches('#'), 16)
        {
            s.edge_color = almondbread_core::palette::linear(hex);
        }
    }

    /// `--chunk-ms` (GPU time per iterate command buffer), `--extend k` (offscreen
    /// unresolved pixels continue to k x the limit), `--bla 0|1|auto`, `--bla-eps e`.
    pub fn apply_renderer<B: GpuBackend>(&self, r: &mut Renderer<B>) {
        if let Some(v) = self.f64("chunk-ms") {
            r.config.chunk_target = v / 1000.0;
        }
        if let Some(v) = self.usize("extend") {
            r.config.offscreen_extension = v.max(1);
        }
        // BLA iteration skipping (kb: techniques/bla): `--bla 0` off (the skip-off diff),
        // `1` whenever a table exists, `auto` (default) where the forecast says it pays;
        // `--bla-eps e` for validity epsilon 2^-e.
        if let Some(v) = self.get("bla") {
            match BlaMode::parse(v) {
                Some(m) => r.config.bla = m,
                None => eprintln!("almondbread: --bla {v}: expected 0, 1 or auto"),
            }
        }
        if let Some(e) = self.f64("bla-eps") {
            r.config.bla_epsilon = 2f64.powf(-e);
        }
    }
}

pub fn parse_size(s: &str) -> Result<(usize, usize)> {
    let parts: Vec<usize> = s.split('x').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 2 || parts[0] == 0 || parts[1] == 0 {
        return err(format!("bad size {s}"));
    }
    Ok((parts[0], parts[1]))
}

/// Colouring ("look") settings from a JSON object (flight scripts; see `fly`): `look` (a look
/// id, applied first), `specular`, `shininess`, `light_height`, `light_angle`, `glow`,
/// `glow_width`, `outline_width`, `palette_filter`, `cycle` (cycles per second), `depth_tint`.
pub fn apply_look_json(v: &serde_json::Value, s: &mut RenderSettings) {
    use serde_json::Value;
    if let Some(l) = v
        .get("look")
        .and_then(Value::as_str)
        .and_then(almondbread_core::palette::Look::named)
    {
        l.apply(s);
    }
    let f = |k: &str| v.get(k).and_then(Value::as_f64);
    if let Some(x) = f("specular") {
        s.specular = x;
    }
    if let Some(x) = f("shininess") {
        s.shininess = x;
    }
    if let Some(x) = f("light_height") {
        s.light_height = x;
    }
    if let Some(x) = f("light_angle") {
        s.light_angle = x;
    }
    if let Some(x) = f("glow") {
        s.glow = x;
    }
    if let Some(x) = f("glow_width") {
        s.glow_width = x;
    }
    if let Some(x) = f("outline_width") {
        s.outline_width = x;
    }
    if let Some(x) = v.get("palette_filter").and_then(Value::as_bool) {
        s.palette_filter = x;
    }
    if let Some(x) = f("cycle") {
        s.cycle_speed = x;
    }
    if let Some(x) = f("depth_tint") {
        s.depth_tint = x;
    }
}
