//! The cosmic size scale for the UI (kb: decisions/controls, "Cosmic size scale"): its
//! settings record and the explorer's setter.

use crate::explorer::Explorer;

/// Cosmic size scale overlay: off by default.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CosmicScaleSettings {
    pub enabled: bool,
    /// Physical size of one view point in metres; 0: unknown (the view counts as 30 cm wide).
    pub metres_per_point: f64,
}

impl From<&almondbread_core::cosmic::CosmicScaleSettings> for CosmicScaleSettings {
    fn from(s: &almondbread_core::cosmic::CosmicScaleSettings) -> Self {
        Self {
            enabled: s.enabled,
            metres_per_point: s.metres_per_point,
        }
    }
}

impl CosmicScaleSettings {
    fn to_core(&self) -> almondbread_core::cosmic::CosmicScaleSettings {
        almondbread_core::cosmic::CosmicScaleSettings {
            enabled: self.enabled,
            metres_per_point: if self.metres_per_point.is_finite() && self.metres_per_point > 0.0 {
                self.metres_per_point
            } else {
                0.0
            },
        }
    }
}

#[uniffi::export]
impl Explorer {
    pub fn cosmic_scale(&self) -> CosmicScaleSettings {
        self.with_cosmic(|c| CosmicScaleSettings::from(&c.settings))
    }

    /// Switches the cosmic size scale (a frame follows: resume the display link).
    pub fn set_cosmic_scale(&self, settings: CosmicScaleSettings) {
        let s = settings.to_core();
        self.with_cosmic(|c| c.settings = s);
    }
}
