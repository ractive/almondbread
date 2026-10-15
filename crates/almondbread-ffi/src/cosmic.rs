//! The cosmic size scale for the UI (kb: decisions/controls, "Cosmic size scale"): its
//! settings record, the anchor choices, and the explorer's setter.

use crate::explorer::Explorer;

/// Cosmic size scale overlay: off by default.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CosmicScaleSettings {
    pub enabled: bool,
    /// "screen" or a thing the whole set is as big as (see `cosmic_anchors`).
    pub anchor: String,
    /// Physical size of one view point in metres; 0: unknown (the view counts as 30 cm wide).
    pub metres_per_point: f64,
}

impl From<&almondbread_core::cosmic::CosmicScaleSettings> for CosmicScaleSettings {
    fn from(s: &almondbread_core::cosmic::CosmicScaleSettings) -> Self {
        Self {
            enabled: s.enabled,
            anchor: s.anchor.clone(),
            metres_per_point: s.metres_per_point,
        }
    }
}

impl CosmicScaleSettings {
    fn to_core(&self) -> almondbread_core::cosmic::CosmicScaleSettings {
        let anchor = if almondbread_core::cosmic::ANCHORS.contains(&self.anchor.as_str()) {
            self.anchor.clone()
        } else {
            "screen".into()
        };
        almondbread_core::cosmic::CosmicScaleSettings {
            enabled: self.enabled,
            anchor,
            metres_per_point: if self.metres_per_point.is_finite() && self.metres_per_point > 0.0 {
                self.metres_per_point
            } else {
                0.0
            },
        }
    }
}

/// The cosmic panel's rect in view points (origin top left).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct CosmicPanelRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One anchor choice for the picker.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CosmicAnchor {
    pub key: String,
    /// "Your screen", "The set is Earth", "Zoom range as cosmos (metaphor)".
    pub label: String,
    /// One line on what it compares.
    pub description: String,
    /// Picker group: 0 the screen, 1 "the set is ...", 2 the metaphor.
    pub group: u32,
}

/// The anchors in display order.
#[uniffi::export]
pub fn cosmic_anchors() -> Vec<CosmicAnchor> {
    almondbread_core::cosmic::ANCHORS
        .iter()
        .map(|k| CosmicAnchor {
            key: (*k).into(),
            label: almondbread_core::cosmic::anchor_label(k),
            description: almondbread_core::cosmic::anchor_description(k),
            group: almondbread_core::cosmic::anchor_group(k),
        })
        .collect()
}

#[uniffi::export]
impl Explorer {
    pub fn cosmic_scale(&self) -> CosmicScaleSettings {
        self.with_cosmic(|c| CosmicScaleSettings::from(&c.settings))
    }

    /// While the view is past the Planck length (or the relative cosmos): the panel's rect in
    /// view points from the top left, for a "How big is big?" link next to it.
    pub fn cosmic_hint(&self) -> Option<CosmicPanelRect> {
        self.cosmic_hint_points().map(|r| CosmicPanelRect {
            x: r[0],
            y: r[1],
            width: r[2],
            height: r[3],
        })
    }

    /// Switches the cosmic size scale (a frame follows: resume the display link).
    pub fn set_cosmic_scale(&self, settings: CosmicScaleSettings) {
        let s = settings.to_core();
        self.with_cosmic(|c| c.settings = s);
    }
}
