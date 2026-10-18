//! Help badges for the UI (kb: decisions/controls, "Help"): the "?" circles Rust draws in the
//! corners of the cosmic scale, the π meter and the mini-map row. The platform asks `help_hit`
//! on every press before the compass and the maps; a hit opens its own popover with the topic's
//! text at the badge and the press goes nowhere else. Coordinates: view points from the view
//! centre, y up.

use crate::explorer::Explorer;

/// A help badge: its topic id ("cosmic-scale", "pi-meter", "mini-map", "orbit"), centre and
/// radius in view points (from the view centre, y up).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HelpBadgeInfo {
    pub topic: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

fn info((topic, (x, y), radius): (almondbread_render::help::HelpTopic, (f64, f64), f64)) -> HelpBadgeInfo {
    HelpBadgeInfo {
        topic: topic.id().to_string(),
        x,
        y,
        radius,
    }
}

/// A cockpit control as drawn in the last frame: its id ("manual", "cruise", "autopilot",
/// "journey", "lane"), centre and the half sizes of its hit cell, view points from the view
/// centre, y up.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CockpitControlInfo {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub half_width: f64,
    pub half_height: f64,
}

#[uniffi::export]
impl Explorer {
    /// The help badge under a press at (x, y), if any (as drawn in the last frame).
    pub fn help_hit(&self, x: f64, y: f64) -> Option<HelpBadgeInfo> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        self.with_host(|h| h.help_hit(x, y)).map(info)
    }

    /// The cockpit's level icon under a press at (x, y) view points from the centre, y up:
    /// "manual", "cruise", "autopilot", "journey" or "lane"; None elsewhere.
    pub fn cockpit_hit(&self, x: f64, y: f64) -> Option<String> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        self.with_host(|h| h.cockpit_hit(x, y)).map(str::to_string)
    }

    /// Every cockpit control drawn in the last frame (UI tests aim at them).
    pub fn cockpit_controls(&self) -> Vec<CockpitControlInfo> {
        self.with_host(|h| h.cockpit_controls())
            .into_iter()
            .map(|(id, (x, y), (hw, hh))| CockpitControlInfo {
                id: id.to_string(),
                x,
                y,
                half_width: hw,
                half_height: hh,
            })
            .collect()
    }

    /// Every help badge drawn in the last frame (window tests aim at them).
    pub fn help_badges(&self) -> Vec<HelpBadgeInfo> {
        self.with_host(|h| h.help_badges()).into_iter().map(info).collect()
    }
}
