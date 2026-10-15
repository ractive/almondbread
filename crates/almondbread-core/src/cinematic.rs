//! Cinematic effects for the autopilot, the journey, fly-to and recorded `fly` videos (kb:
//! decisions/controls "Cinematic effects", ideas "Cinematic effects for autopilot and fly-to").
//! Off by default and switchable as a whole; each effect has its own strength.
//!
//! - **Spin**: the view turns while it zooms, at a rate tied to the zoom rate (degrees per
//!   e-fold of zoom), so a pause stops the turning too and a faster flight turns faster. Only
//!   automatic motion spins (autopilot, journey, fly-to); manual zooming never does.
//! - **Hue drift**: the palette's hues turn in OKLab with depth (degrees per e-fold of
//!   magnification). A function of the view, not of time: the same view always has the same
//!   colours, and history reprojection stays valid.
//! - **Phase drift**: the palette phase moves with depth (cycles per e-fold), like the depth
//!   tint but per e-fold and only with the effects on.
//! - **Flight cycling**: slow colour cycling while flying (cycles per second while the
//!   autopilot, a journey or a flight moves; idle cycling is the separate `cycle_speed`).

/// Cinematic effect settings (per window, persisted with the render settings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cinematic {
    /// Master switch (off by default: "pure" exploration).
    pub enabled: bool,
    /// View turn per e-fold of automatic zoom, degrees (sign: direction).
    pub spin: f64,
    /// Palette hue turn per e-fold of magnification, degrees.
    pub hue_drift: f64,
    /// Palette phase per e-fold of magnification, cycles.
    pub phase_drift: f64,
    /// Palette cycles per second while flying.
    pub fly_cycle: f64,
}

impl Default for Cinematic {
    fn default() -> Self {
        Self {
            enabled: false,
            spin: 4.0,
            hue_drift: 2.5,
            phase_drift: 0.01,
            fly_cycle: 0.02,
        }
    }
}

impl Cinematic {
    /// Spin in radians per e-fold (0 when off).
    pub fn spin_per_efold(&self) -> f64 {
        if self.enabled && self.spin.is_finite() {
            self.spin.clamp(-45.0, 45.0).to_radians()
        } else {
            0.0
        }
    }

    /// Hue turn (radians) and extra palette phase (cycles) at `efolds` of magnification
    /// (ln of the zoom relative to home); (0, 0) when off.
    pub fn palette_at(&self, efolds: f64) -> (f64, f64) {
        if !self.enabled || !efolds.is_finite() {
            return (0.0, 0.0);
        }
        let e = efolds.max(0.0);
        let hue = (self.hue_drift.clamp(-90.0, 90.0) * e).to_radians();
        let phase = self.phase_drift.clamp(-1.0, 1.0) * e;
        (hue.rem_euclid(std::f64::consts::TAU), phase.rem_euclid(1.0))
    }

    /// Palette cycles per second while flying (0 when off).
    pub fn fly_cycle_rate(&self) -> f64 {
        if self.enabled && self.fly_cycle.is_finite() {
            self.fly_cycle.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }

    /// Clamped to the supported ranges.
    pub fn sanitized(mut self) -> Self {
        let f = |v: f64, lo: f64, hi: f64, d: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
        let d = Self::default();
        self.spin = f(self.spin, -45.0, 45.0, d.spin);
        self.hue_drift = f(self.hue_drift, -90.0, 90.0, d.hue_drift);
        self.phase_drift = f(self.phase_drift, -1.0, 1.0, d.phase_drift);
        self.fly_cycle = f(self.fly_cycle, -1.0, 1.0, d.fly_cycle);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_means_no_effect() {
        let c = Cinematic::default();
        assert_eq!(c.spin_per_efold(), 0.0);
        assert_eq!(c.palette_at(10.0), (0.0, 0.0));
        assert_eq!(c.fly_cycle_rate(), 0.0);
        let on = Cinematic { enabled: true, ..c };
        assert!((on.spin_per_efold() - 4f64.to_radians()).abs() < 1e-15);
        let (h, p) = on.palette_at(10.0);
        assert!((h - 25f64.to_radians()).abs() < 1e-12 && (p - 0.1).abs() < 1e-12);
        // A function of depth only: the same depth gives the same colours.
        assert_eq!(on.palette_at(3.5), on.palette_at(3.5));
    }
}
