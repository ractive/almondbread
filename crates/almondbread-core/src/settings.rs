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

/// Everything that affects the image except the viewport.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
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
    /// Colour cycling: palette cycles per second while the view is idle (0: off).
    pub cycle_speed: f64,
    /// Depth tint: palette phase shift per decade of zoom (0: off).
    pub depth_tint: f64,

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
            cycle_speed: 0.0,
            depth_tint: 0.0,
            supersampling: Supersampling::default(),
            interactive_preview: true,
        }
    }

    /// Iteration limit for `viewport`: manual, or automatic: the depth heuristic raised to what
    /// the renderer learned from the last views' escape statistics.
    pub fn max_iterations(&self, viewport: &Viewport) -> usize {
        if !self.auto_iterations {
            return self.manual_iterations.min(self.max_iterations_cap);
        }
        depth_iterations(viewport)
            .max(self.learned_iterations)
            .min(self.max_iterations_cap)
    }

    /// Distance estimate needed (shading or outline).
    pub fn de_needed(&self) -> bool {
        self.shading > 0.0 || self.outline > 0.0
    }

    pub fn palette(&self) -> Palette {
        Palette::named(&self.palette_id)
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
}
