//! User-facing render settings (everything that affects the image except the viewport).

use crate::palette::{Look, Palette};
use crate::viewport::Viewport;

/// Anti-aliasing: samples per axis (1 to 4). Offscreen renders supersample on a grid; the
/// interactive view accumulates `factor²` jittered samples progressively while idle (FL, kb:
/// techniques/anti-aliasing), so this is the maximum, not a cost paid before the first frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Supersampling {
    Off = 1,
    X4 = 2,
    #[default]
    X9 = 3,
    X16 = 4,
}

impl Supersampling {
    pub fn factor(self) -> usize {
        self as usize
    }

    pub fn from_factor(v: usize) -> Option<Self> {
        match v {
            1 => Some(Self::Off),
            2 => Some(Self::X4),
            3 => Some(Self::X9),
            4 => Some(Self::X16),
            _ => None,
        }
    }
}

/// How the inside of the set is coloured (kb: techniques/coloring-methods, "Interior colouring
/// as built"). Every mode but `Flat` analyses the attracting cycle of each interior pixel on
/// the GPU after the interior test: period by cycle closure, multiplier, interior distance and
/// the cycle's closest approach to 0, at any depth (floatexp).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InteriorMode {
    /// One colour (`interior_color`).
    #[default]
    Flat,
    /// Closest approach of the attracting cycle to 0, log2 |z*| ("bubbles" around each nucleus).
    Bubbles,
    /// One colour per period, shaded by the multiplier's magnitude.
    Period,
    /// Internal angle and radius of the multiplier: stripes along the internal rays.
    Angle,
    /// Interior distance estimate: bright inside, darkening towards the boundary.
    Distance,
}

impl InteriorMode {
    pub const ALL: [InteriorMode; 5] = [Self::Flat, Self::Bubbles, Self::Period, Self::Angle, Self::Distance];

    /// Stable name (CLI `--inside`, FFI, persisted settings).
    pub fn name(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Bubbles => "bubbles",
            Self::Period => "period",
            Self::Angle => "angle",
            Self::Distance => "distance",
        }
    }

    pub fn parse(v: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == v)
    }

    /// The kernel's colorize code (0: flat).
    pub fn code(self) -> u32 {
        self as u32
    }

    /// The iterate kernel must analyse interior cycles.
    pub fn needs_analysis(self) -> bool {
        self != Self::Flat
    }
}

/// Upper bound of every iteration limit (manual, automatic, extension and "compute further"
/// passes): 2^24. The iterate kernel stores the escape iteration n as float32, exact only up
/// to 2^24; above, counts were quantised (n +- 1, 2, ...) in the smooth colouring, the
/// verifier and the learner's statistics (review M2). Far above any view's need (the default
/// cap is 200 000; the deepest benchmark needs 100 000).
pub const MAX_ITERATION_LIMIT: usize = 1 << 24;

/// Everything that affects the image except the viewport.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
    /// The fractal (F2; kb: plans/f2-pluggable-formulas). Mandelbrot by default; the
    /// capabilities of the others gate features ([`crate::formula::FormulaCapabilities`]).
    pub formula: crate::formula::Formula,

    // Iterations
    pub auto_iterations: bool,
    pub manual_iterations: usize,
    pub max_iterations_cap: usize,
    /// Floor learned from escapes past the nominal limit (statistics-driven auto iterations,
    /// kb: techniques/auto-iterations); applies from the next view on.
    pub learned_iterations: usize,
    /// Stop pixels whose orbit derivative shows an attracting cycle (kb: techniques/interior-detection).
    pub interior_detection: bool,

    // Colouring
    pub palette_id: String,
    /// Palette cycles per iteration.
    pub color_density: f64,
    pub color_offset: f64,
    pub log_coloring: bool,
    pub smooth_coloring: bool,
    /// Linear RGB.
    pub interior_color: [f32; 3],
    /// Interior colouring (flat: `interior_color`).
    pub interior_mode: InteriorMode,
    /// Buddhabrot or Nebulabrot instead of the escape-time colouring (BB, kb: plans/buddhabrot;
    /// Mandelbrot only, [`crate::formula::FormulaCapabilities::buddhabrot`]; set by the looks).
    pub buddha: crate::buddha::BuddhaMode,
    /// Palette cycles of the interior modes (per halving of |z*| for bubbles, per doubling of
    /// the distance for the distance mode, per turn for the angle mode).
    pub interior_density: f64,
    /// Angle mode: stripes per turn of the internal angle.
    pub interior_stripes: f64,
    pub shading: f64,
    /// Degrees.
    pub light_angle: f64,
    pub outline: f64,
    pub outline_width: f64,
    /// Linear RGB of the outline (boundary emphasis).
    pub edge_color: [f32; 3],
    /// Strength of the specular highlight of the lighting (0..1).
    pub specular: f64,
    pub shininess: f64,
    /// Elevation of the light: 0 grazing, larger is more from the front.
    pub light_height: f64,
    /// Soft halo along the boundary: strength, linear RGB, width in pixels.
    pub glow: f64,
    pub glow_color: [f32; 3],
    pub glow_width: f64,
    /// Filter the palette by each pixel's colour gradient (palette anti-aliasing).
    pub palette_filter: bool,
    /// Dense regions (the palette cycles faster than a pixel) fade to a calm dark colour with
    /// thin bright filaments; off: the filtered palette's mean (a grey), as before.
    pub dense_calm: bool,
    /// A palette given as stops (imported or custom: `(position 0..1, sRGB hex)`), used
    /// instead of `palette_id`'s built-in one when set.
    pub palette_stops: Option<Vec<(f64, u32)>>,
    /// Colour cycling: palette cycles per second while the view is idle (0: off).
    pub cycle_speed: f64,
    /// Depth tint: palette phase shift per decade of zoom (0: off).
    pub depth_tint: f64,
    /// Cinematic effects (spin, palette drift with depth, cycling while flying; off by
    /// default; kb: decisions/controls "Cinematic effects").
    pub cinematic: crate::cinematic::Cinematic,

    // Quality
    pub supersampling: Supersampling,
    pub interactive_preview: bool,
}

impl Default for RenderSettings {
    /// The default look (FL: smooth iteration, DE lighting with a soft highlight, a subtle
    /// boundary outline, palette anti-aliasing) and progressive 9x anti-aliasing.
    fn default() -> Self {
        let mut s = Self::plain();
        Look::default_look().apply(&mut s);
        s
    }
}

impl RenderSettings {
    /// The M2.5 colouring (no lighting, so no distance estimate): what the headless tools
    /// default to, so benchmark timings stay comparable across milestones.
    pub fn plain() -> Self {
        Self {
            formula: crate::formula::Formula::Mandelbrot,
            auto_iterations: true,
            manual_iterations: 1000,
            max_iterations_cap: 200_000,
            learned_iterations: 0,
            interior_detection: true,
            palette_id: Palette::named("classic").id.to_string(),
            color_density: 0.02,
            color_offset: 0.0,
            log_coloring: false,
            smooth_coloring: true,
            interior_color: [0.0, 0.0, 0.0],
            interior_mode: InteriorMode::Flat,
            buddha: crate::buddha::BuddhaMode::Off,
            interior_density: 0.25,
            interior_stripes: 8.0,
            shading: 0.0,
            light_angle: 45.0,
            outline: 0.0,
            outline_width: 1.5,
            edge_color: [0.0, 0.0, 0.0],
            specular: 0.0,
            shininess: 24.0,
            light_height: 1.0,
            glow: 0.0,
            glow_color: [1.0, 1.0, 1.0],
            glow_width: 6.0,
            palette_filter: false,
            dense_calm: true,
            palette_stops: None,
            cycle_speed: 0.0,
            depth_tint: 0.0,
            cinematic: crate::cinematic::Cinematic::default(),
            supersampling: Supersampling::default(),
            interactive_preview: true,
        }
    }

    /// Iteration limit for `viewport`: manual, or automatic: the depth heuristic raised to what
    /// the renderer learned from the last views' escape statistics. The cap bounds only the
    /// automatic limit (the UI hides it in manual mode; before 2026-10-17 it silently clipped a
    /// manual 10 M to the default 200 k).
    pub fn max_iterations(&self, viewport: &Viewport) -> usize {
        if !self.auto_iterations {
            return self.manual_iterations.clamp(1, MAX_ITERATION_LIMIT);
        }
        self.depth_iterations(viewport)
            .max(self.learned_iterations)
            .min(self.max_iterations_cap)
            .clamp(1, MAX_ITERATION_LIMIT)
    }

    /// The depth heuristic of the formula: [`depth_iterations`] for the parameter planes; a
    /// Julia set's escape counts grow linearly with depth (a pixel needs about `depth / log10|f'|`
    /// steps to leave its neighbours, then a few hundred to escape; F4, kb: plans/f4-deep-julia),
    /// [`crate::julia::depth_iterations`].
    pub fn depth_iterations(&self, viewport: &Viewport) -> usize {
        if self.formula.is_julia() {
            crate::julia::depth_iterations(viewport)
        } else {
            depth_iterations(viewport)
        }
    }

    /// Distance estimate needed (shading or outline).
    pub fn de_needed(&self) -> bool {
        self.shading > 0.0 || self.outline > 0.0
    }

    pub fn palette(&self) -> Palette {
        Palette::named(&self.palette_id)
    }

    /// The palette as a `size`-entry linear-RGB table: the custom stops if set, else the
    /// built-in palette `palette_id`.
    pub fn palette_table(&self, size: usize) -> Vec<[f32; 4]> {
        match &self.palette_stops {
            Some(stops) if !stops.is_empty() => crate::palette::gradient_table(stops, size),
            _ => self.palette().table(size),
        }
    }
}

/// Which corner of the view the mini-map sits in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Corner {
    #[default]
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

/// Overlays drawn over the fractal by the renderer (kb: decisions/controls, "Orbit overlay
/// and mini-map"). Not part of the image: they never enter the history or the exports.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlaySettings {
    /// The orbit of the point under the pointer (keyboard mode: the crosshair).
    pub orbit: bool,
    /// Iterates drawn (1 ... 1000).
    pub orbit_length: usize,
    /// The mini-map: the whole set with the view marked (the orbit overlay shows one too).
    pub map: bool,
    /// Nested maps ("breadcrumbs"): the whole set plus up to 3 deeper levels (1 ... 4).
    pub map_levels: usize,
    /// Width of one map panel, view points.
    pub map_size: f64,
    pub map_corner: Corner,
    /// The Julia companion panel: the Julia set of the orbit's point (kb: plans/f1-julia-view).
    pub julia: bool,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            orbit: false,
            orbit_length: 50,
            map: false,
            map_levels: 3,
            map_size: 180.0,
            map_corner: Corner::BottomLeft,
            julia: false,
        }
    }
}

impl OverlaySettings {
    /// Anything to draw.
    pub fn any(&self) -> bool {
        self.orbit || self.map || self.julia
    }

    /// Clamped to the supported ranges.
    pub fn sanitized(mut self) -> Self {
        self.orbit_length = self.orbit_length.clamp(1, crate::orbit::MAX_ORBIT_POINTS);
        self.map_levels = self.map_levels.clamp(1, 4);
        self.map_size = if self.map_size.is_finite() {
            self.map_size.clamp(80.0, 400.0)
        } else {
            180.0
        };
        self
    }
}

/// The depth-only part of the automatic limit: `300 + 120 · depth^1.6`, depth = log10(zoom).
pub fn depth_iterations(viewport: &Viewport) -> usize {
    let depth = viewport.zoom_log10().max(0.0);
    (300.0 + 120.0 * depth.powf(1.6)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_iterations() {
        let s = RenderSettings::default();
        assert_eq!(s.max_iterations(&Viewport::home()), 300);
        // neck at 2.2e5: the Swift model gives 2053 (kb: synthesis/benchmarks)
        let neck = Viewport::new(-0.7500016386523336, -0.002107342465685259, 3.75e-3 / 2.2155e5);
        assert_eq!(s.max_iterations(&neck), 2053);
    }

    /// Every limit stays where the GPU's float32 escape count is exact (review M2).
    #[test]
    fn limits_stay_float32_exact() {
        let v = Viewport::home();
        let mut s = RenderSettings {
            auto_iterations: false,
            manual_iterations: 20_000_000,
            ..RenderSettings::default()
        };
        assert_eq!(s.max_iterations(&v), MAX_ITERATION_LIMIT);
        assert_eq!(MAX_ITERATION_LIMIT as f32 as usize, MAX_ITERATION_LIMIT);
        assert_ne!((MAX_ITERATION_LIMIT + 1) as f32 as usize, MAX_ITERATION_LIMIT + 1);
        s.manual_iterations = 1000;
        assert_eq!(s.max_iterations(&v), 1000);
        s.auto_iterations = true;
        s.learned_iterations = 1 << 30;
        s.max_iterations_cap = 1 << 31;
        assert_eq!(s.max_iterations(&v), MAX_ITERATION_LIMIT);
    }
}
