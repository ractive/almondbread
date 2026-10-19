//! Help badges for the UI (kb: decisions/controls, "Help"): the "?" circles Rust draws in the
//! corners of the cosmic scale, the π meter and the mini-map row. The platform asks `help_hit`
//! on every press before the compass and the maps; a hit opens its own popover with the topic's
//! text at the badge and the press goes nowhere else. Coordinates: view points from the view
//! centre, y up.

use almondbread_render::metal::media_time;

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

/// A panel drawn by Rust, in view points from the top left.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OverlayPanel {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
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

/// What a finger held still does (`Explorer::touch_hold`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TouchHold {
    Nothing,
    FlyToMap,
    Menu,
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

    /// What a finger held still at (x, y) view points from the centre, y up, does (kb:
    /// decisions/controls "Touch"): nothing (a control; in A, C and a journey's flight Rust
    /// classifies the finger itself, `InputEvent::FingerDown`), fly to the mini-map's point or
    /// the long-press menu.
    pub fn touch_hold(&self, x: f64, y: f64) -> TouchHold {
        if !(x.is_finite() && y.is_finite()) {
            return TouchHold::Nothing;
        }
        match self.with_host(|h| h.touch_hold(x, y, media_time())) {
            almondbread_core::TouchHold::Nothing => TouchHold::Nothing,
            almondbread_core::TouchHold::FlyToMap => TouchHold::FlyToMap,
            almondbread_core::TouchHold::Menu => TouchHold::Menu,
        }
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

    /// The Rust panels of the last frame (`cosmic`, `cockpit`, `maps`, `julia`, `pi`, `pi-trail`) in
    /// view points from the top left: UI tests check they keep clear of each other and of the
    /// platform's controls (kb: decisions/controls "Touch").
    pub fn overlay_panels(&self) -> Vec<OverlayPanel> {
        self.with_host(|h| h.overlay_panels())
            .into_iter()
            .map(|(id, [x, y, width, height])| OverlayPanel {
                id: id.to_string(),
                x,
                y,
                width,
                height,
            })
            .collect()
    }
}
