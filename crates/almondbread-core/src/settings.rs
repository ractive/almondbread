//! User-facing render settings (the Swift `FractalModel` minus the viewport and UI state).

use crate::palette::Palette;
use crate::viewport::Viewport;

/// Supersampling factor per axis (1, 2 or 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Supersampling {
    #[default]
    Off = 1,
    X4 = 2,
    X9 = 3,
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

    // Quality
    pub supersampling: Supersampling,
    pub interactive_preview: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            auto_iterations: true,
            manual_iterations: 1000,
            max_iterations_cap: 200_000,
            learned_iterations: 0,
            interior_detection: true,
            palette_id: Palette::default_palette().id.to_string(),
            color_density: 0.02,
            color_offset: 0.0,
            log_coloring: false,
            smooth_coloring: true,
            interior_color: [0.0, 0.0, 0.0],
            shading: 0.0,
            light_angle: 45.0,
            outline: 0.0,
            outline_width: 1.5,
            supersampling: Supersampling::Off,
            interactive_preview: true,
        }
    }
}

impl RenderSettings {
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
    let depth = viewport.zoom().log10().max(0.0);
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
