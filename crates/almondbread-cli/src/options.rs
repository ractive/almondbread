//! `--key value` options shared by the commands; the same keys and defaults as the Swift
//! `CommandOptions`, so commands compare 1:1.

use std::collections::HashMap;

use almondbread_core::settings::{RenderSettings, Supersampling};
use almondbread_core::viewport::{Center, Scale, Viewport};
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

/// On/off options: given bare (`--log`) they mean `--log 1`; `--log 0` still switches off.
const FLAGS: &[&str] = &[
    "calm",
    "compass",
    "converged",
    "deep",
    "ff",
    "filter",
    "final",
    "interior",
    "interior-stats",
    "ios",
    "jump-check",
    "julia",
    "julia-k",
    "kat",
    "list",
    "log",
    "map",
    "noise",
    "pi-overlay",
    "plain",
    "quiet",
    "scale-overlay",
];

impl Options {
    /// `--key value` pairs and positionals. A value never starts with `--`: `--log --size
    /// 800x600` is an error for a valued key (it used to read as log = "--size"), and the
    /// bare form of an on/off key in [`FLAGS`] (negative numbers such as `-1e-3` are values).
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut o = Options::default();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            if let Some(key) = a.strip_prefix("--") {
                if key.is_empty() {
                    return err("bare -- is not an option");
                }
                match args.get(i + 1).filter(|v| !v.starts_with("--")) {
                    Some(v) => {
                        o.values.insert(key.to_string(), v.clone());
                        i += 2;
                    }
                    None if FLAGS.contains(&key) => {
                        o.values.insert(key.to_string(), "1".to_string());
                        i += 1;
                    }
                    None => return err(format!("missing value for {a}")),
                }
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
            // (`--scale s`: the place's centre at another depth, e.g. a mini-map level's.)
            if let Some(s) = self.get("scale") {
                let Some(scale) = Scale::parse(s) else {
                    return err(format!("bad --scale {s}"));
                };
                return Ok(Viewport::with_center(p.center(), scale));
            }
            return Ok(p.viewport(width, height));
        }
        if let Some(name) = self.get("location") {
            let Some(l) = almondbread_core::locations::location_named(name) else {
                return err(format!("unknown --location {name}"));
            };
            // (`--at-width w`: the pixel size of the location framed w pixels wide, e.g. a centre
            // crop of the 1600-wide framing.)
            return Ok(l.viewport_for_width(self.usize("at-width").unwrap_or(width)));
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
            // (A place is in its own fractal, F3; `--formula` below can override it.)
            s.formula = p.formula;
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
            let max = almondbread_core::settings::MAX_ITERATION_LIMIT;
            if n > max {
                eprintln!(
                    "almondbread: --iterations {n} is above {max} (2^24, where float32 escape counts stay exact); using {max}"
                );
            }
            s.auto_iterations = false;
            s.manual_iterations = n;
            s.max_iterations_cap = s.max_iterations_cap.max(n);
        }
        // `--cap n`: the auto-iteration cap (the app's "Cap" setting; manual `--iterations`
        // raise it to themselves).
        if let Some(n) = self.usize("cap") {
            s.max_iterations_cap = n.max(1);
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
        // Dense-region calm colour (kb: techniques/coloring-methods "Dense regions"): `--calm 0`
        // for references whose pixel scale differs (it depends on the colour bands per pixel).
        if let Some(v) = self.get("calm") {
            s.dense_calm = v != "0";
        }
        // The formula (F2, F3): `mandelbrot` (default), `multibrot3` ... `multibrot8` (z^P + c;
        // `multibrot2` is the generic path's test that it equals Mandelbrot).
        if let Some(v) = self.get("formula") {
            match almondbread_core::formula::Formula::parse(v) {
                Some(f) => s.formula = f,
                None => eprintln!("almondbread: --formula {v}: expected mandelbrot, multibrot2..8 or mandelbar"),
            }
        }
        // F4: the Julia set of `z^2 + k` as the main view (`--julia-k re,im`, any number of
        // digits; kb: plans/f4-deep-julia): the view's points are the starts z_0. (`--julia 1`
        // is F1's panel.)
        if let Some(v) = self.get("julia-k") {
            match v
                .split_once(',')
                .and_then(|(re, im)| almondbread_core::viewport::Center::parse(re.trim(), im.trim()))
            {
                Some(k) => {
                    s.formula =
                        almondbread_core::formula::Formula::Julia(almondbread_core::formula::JuliaK::new(k.big()))
                }
                None => eprintln!("almondbread: --julia-k {v}: expected re,im"),
            }
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
    /// unresolved pixels continue to k x the limit), `--further-budget s`, the local SA tunables `--sa-*`.
    pub fn apply_renderer<B: GpuBackend>(&self, r: &mut Renderer<B>) {
        if let Some(v) = self.f64("chunk-ms") {
            r.config.chunk_target = v / 1000.0;
        }
        if let Some(v) = self.usize("extend") {
            r.config.offscreen_extension = v.max(1);
        }
        // Compute further's GPU budget in seconds (`--further-budget s`, kb: techniques/auto-iterations
        // "Compute further"); `verify --history` runs it only when asked for.
        if let Some(v) = self.f64("further-budget") {
            r.config.further_budget = v;
        }
        // Local series approximation tunables (kb: techniques/local-series-approximation).
        let sa = &mut r.config.local_sa;
        if let Some(e) = self.f64("sa-eps") {
            sa.eps = e;
        }
        if let Some(t) = self.usize("sa-top") {
            sa.top = t.next_power_of_two();
        }
        if let Some(l) = self.usize("sa-leaf") {
            sa.leaf = l.next_power_of_two();
        }
        if let Ok(Some((rel, abs))) = self.pair("sa-gate") {
            sa.gate_rel = rel;
            sa.gate_abs = abs;
        }
        if let Some(b) = self.usize("sa-budget") {
            sa.budget = b as u32;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options> {
        Options::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_key_is_not_a_value() {
        // A valued key followed by another key, or last, is an error.
        let e = parse(&["--size", "--log", "1"]).unwrap_err();
        assert_eq!(e.to_string(), "missing value for --size");
        assert!(parse(&["out.png", "--center"]).is_err());
        assert!(parse(&["--"]).is_err());
        // On/off keys may be bare: `--log --size 800x600` is log = 1, not log = "--size".
        let o = parse(&["out.png", "--log", "--size", "800x600", "--map"]).unwrap();
        assert_eq!(o.get("log"), Some("1"));
        assert_eq!(o.get("map"), Some("1"));
        assert_eq!(o.size("size", (1, 1)).unwrap(), (800, 600));
        assert_eq!(o.positional, vec!["out.png".to_string()]);
        // Explicit values still work, negative numbers included.
        let o = parse(&["--log", "0", "--center", "-0.75,-0.1", "--scale", "-1e-3"]).unwrap();
        assert_eq!(o.get("log"), Some("0"));
        assert_eq!(o.get("center"), Some("-0.75,-0.1"));
        assert_eq!(o.get("scale"), Some("-1e-3"));
    }
}
