//! Help badges (kb: decisions/controls, "Help"): a small "?" circle in the corner of an overlay
//! panel drawn by Rust (the cosmic size scale, the π meter, the mini-map row). The panels lay
//! their badge out while the scene is built; a press is hit-tested here, like the compass, and
//! the platform shows its own popover with the topic's text at the badge. Only the circle (plus a
//! small slop) takes presses, so the canvas and the map panels keep every other click.

use crate::overlay::{Scene, rgba};

/// Radius of the drawn circle, view points.
pub const RADIUS: f64 = 7.0;
/// Extra hit radius around the circle, view points (a little more forgiving than the drawing,
/// still far smaller than a map panel).
pub const HIT_SLOP: f64 = 2.5;
/// Gap between the circle and its panel's edges, view points.
pub const INSET: f64 = 5.0;

/// What a badge explains. The id is the platform's key for the popover text and help page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HelpTopic {
    CosmicScale,
    PiMeter,
    MiniMap,
    /// The whole-set map shown by the orbit overlay alone (no mini-map).
    Orbit,
    /// The driving-level cockpit at the bottom.
    Cockpit,
}

impl HelpTopic {
    pub fn id(self) -> &'static str {
        match self {
            Self::CosmicScale => "cosmic-scale",
            Self::PiMeter => "pi-meter",
            Self::MiniMap => "mini-map",
            Self::Orbit => "orbit",
            Self::Cockpit => "cockpit",
        }
    }
}

/// A badge as laid out in the last scene: centre and radius in output pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HelpBadge {
    pub topic: HelpTopic,
    pub centre: [f32; 2],
    pub radius: f32,
}

impl HelpBadge {
    /// Whether a press at (x, y) output pixels hits it (`px`: pixels per point).
    pub fn hit(&self, x: f64, y: f64, px: f64) -> bool {
        let r = f64::from(self.radius) + HIT_SLOP * px;
        (x - f64::from(self.centre[0])).hypot(y - f64::from(self.centre[1])) <= r
    }
}

/// The centre of a badge in the top-right corner of the panel `r` (pixels; `px` pixels per
/// point).
pub fn top_right(r: [f32; 4], px: f32) -> [f32; 2] {
    let o = ((INSET + RADIUS) as f32) * px;
    [r[2] - o, r[1] + o]
}

/// Width a panel keeps free at its right for a top-right badge (pixels).
pub fn reserve(px: f32) -> f32 {
    ((2.0 * RADIUS + INSET) as f32) * px
}

/// Draws a badge for `topic` centred at `centre` (pixels; `px` pixels per point), records it
/// in `badges` and its position in `key`.
pub fn draw(
    scene: &mut Scene,
    badges: &mut Vec<HelpBadge>,
    topic: HelpTopic,
    centre: [f32; 2],
    px: f32,
    key: &mut String,
) {
    let r = RADIUS as f32 * px;
    let c = [f64::from(centre[0]), f64::from(centre[1])];
    scene.set_clip(None);
    // A dark disc with a thin light rim: quiet next to the panel's text, readable on any image.
    scene.disc(c, r + 0.8 * px, rgba(0.0, 0.0, 0.0, 0.35));
    scene.disc(c, r, rgba(0.10, 0.10, 0.13, 0.92));
    scene.ring(c, r - 0.45 * px, 0.9 * px, rgba(1.0, 1.0, 1.0, 0.55));
    let size = 10.5 * px;
    let (_, tw) = crate::font::layout("?", size);
    let cap = crate::font::cap_height() * size;
    scene.text(
        [centre[0] - 0.5 * tw, centre[1] - 0.5 * cap],
        size,
        "?",
        rgba(1.0, 1.0, 1.0, 0.92),
    );
    badges.push(HelpBadge {
        topic,
        centre,
        radius: r,
    });
    key.push_str(&format!("help {} {:.1} {:.1} ", topic.id(), centre[0], centre[1]));
}

/// The badge under a press at (x, y) output pixels, if any.
pub fn hit(badges: &[HelpBadge], x: f64, y: f64, px: f64) -> Option<HelpBadge> {
    badges.iter().copied().find(|b| b.hit(x, y, px))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_circle_takes_presses() {
        let px = 2.0;
        let mut scene = Scene::default();
        let mut badges = Vec::new();
        let mut key = String::new();
        let panel = [100.0, 200.0, 780.0, 400.0];
        let c = top_right(panel, px as f32);
        draw(&mut scene, &mut badges, HelpTopic::CosmicScale, c, px as f32, &mut key);
        assert_eq!(badges.len(), 1);
        assert!(!scene.prims.is_empty() && key.contains("cosmic-scale"));
        let (cx, cy) = (f64::from(c[0]), f64::from(c[1]));
        // Inside the panel, clear of its edges.
        assert!(cx + RADIUS * px < 780.0 && cy - RADIUS * px > 200.0);
        // The centre and the rim hit.
        assert_eq!(hit(&badges, cx, cy, px).map(|b| b.topic), Some(HelpTopic::CosmicScale));
        assert!(hit(&badges, cx + (RADIUS + HIT_SLOP - 0.1) * px, cy, px).is_some());
        // Just beyond the slop, and the rest of the panel, do not.
        assert!(hit(&badges, cx + (RADIUS + HIT_SLOP + 0.5) * px, cy, px).is_none());
        assert!(hit(&badges, cx, cy + 12.0 * px, px).is_none());
        assert!(hit(&badges, 300.0, 300.0, px).is_none());
    }

    #[test]
    fn question_mark_is_in_the_font() {
        assert!(crate::font::glyph('?').is_some());
    }
}
