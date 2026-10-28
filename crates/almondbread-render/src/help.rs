//! Help badges (kb: decisions/controls, "Help"): a small "?" circle in the corner of an overlay
//! panel drawn by Rust (the cosmic size scale, the π meter, the mini-map row, the Julia panel). The panels lay
//! their badge out while the scene is built; a press is hit-tested here, like the compass, and
//! the platform shows its own popover with the topic's text at the badge. Only the circle (plus a
//! small slop) takes presses, so the canvas and the map panels keep every other click.
//!
//! The mini-map's enlarge / shrink button is a badge of the same kind (same circle, two diagonal
//! arrows for the "?"): it is hit-tested with the help badges, so a press on it never jumps the
//! map, and the platform toggles the large map instead of opening a popover.

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
    /// The Julia companion panel (kb: plans/f1-julia-view).
    Julia,
    /// Not help: the mini-map's button that makes it large (arrows pointing outwards).
    MapEnlarge,
    /// Not help: the large mini-map's button that makes it small again (arrows pointing in).
    MapShrink,
}

impl HelpTopic {
    pub fn id(self) -> &'static str {
        match self {
            Self::CosmicScale => "cosmic-scale",
            Self::PiMeter => "pi-meter",
            Self::MiniMap => "mini-map",
            Self::Orbit => "orbit",
            Self::Cockpit => "cockpit",
            Self::Julia => "julia",
            Self::MapEnlarge => "map-enlarge",
            Self::MapShrink => "map-shrink",
        }
    }

    /// The mini-map's size button, not a help topic.
    pub fn is_map_size(self) -> bool {
        matches!(self, Self::MapEnlarge | Self::MapShrink)
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
    let ink = rgba(1.0, 1.0, 1.0, 0.92);
    if topic.is_map_size() {
        arrows(scene, centre, px, topic == HelpTopic::MapEnlarge, ink);
    } else {
        let size = 10.5 * px;
        let (_, tw) = crate::font::layout("?", size);
        let cap = crate::font::cap_height() * size;
        scene.text([centre[0] - 0.5 * tw, centre[1] - 0.5 * cap], size, "?", ink);
    }
    badges.push(HelpBadge {
        topic,
        centre,
        radius: r,
    });
    key.push_str(&format!("help {} {:.1} {:.1} ", topic.id(), centre[0], centre[1]));
}

/// Two diagonal arrows along the bottom-left to top-right diagonal (SF Symbols'
/// `arrow.up.right.and.arrow.down.left` when `outwards`, `arrow.down.left.and.arrow.up.right`
/// otherwise), centred at `c` (pixels; `px` pixels per point).
fn arrows(scene: &mut Scene, c: [f32; 2], px: f32, outwards: bool, ink: [f32; 4]) {
    // Along the diagonal (x and y offsets, points; inside the 14-point circle): the tips and
    // tails of the two arrows; the heads are right-angled with their legs on the axes, like
    // the symbols'.
    let (tip, tail) = if outwards { (3.3, 0.7) } else { (0.7, 3.4) };
    let head = 2.4;
    for d in [-1.0f32, 1.0] {
        // One arrow towards the top right (x up, y down on screen), the other towards the
        // bottom left.
        let (ux, uy) = (d, -d);
        let at = |t: f32| [f64::from(c[0] + ux * t * px), f64::from(c[1] + uy * t * px)];
        // The head's legs run back from the tip towards the tail.
        let sign = if outwards { -1.0 } else { 1.0 };
        let (back_x, back_y) = (sign * ux * head * px, sign * uy * head * px);
        let t = at(tip);
        let shaft_end = at(if outwards { tip - 0.5 * head } else { tip + 0.5 * head });
        scene.segment(shaft_end, at(tail), 0.55 * px, ink, ink);
        scene.triangle(
            t,
            [t[0] + f64::from(back_x), t[1]],
            [t[0], t[1] + f64::from(back_y)],
            ink,
        );
    }
}

/// The badge under a press at (x, y) output pixels, if any (the nearest where two are near).
pub fn hit(badges: &[HelpBadge], x: f64, y: f64, px: f64) -> Option<HelpBadge> {
    nearest(badges.iter().filter(|b| b.hit(x, y, px)), x, y)
}

fn nearest<'a>(badges: impl Iterator<Item = &'a HelpBadge>, x: f64, y: f64) -> Option<HelpBadge> {
    let d = |b: &HelpBadge| (x - f64::from(b.centre[0])).hypot(y - f64::from(b.centre[1]));
    badges.min_by(|a, b| d(a).total_cmp(&d(b))).copied()
}

/// A fingertip's radius for a badge (points; kb: decisions/controls "Touch"): a touch hits a
/// badge this close to its centre (a 36-point target; the drawn badge stays 14 points).
pub const TOUCH_RADIUS: f64 = 18.0;

/// The badge under a touch at (x, y) output pixels: within [`TOUCH_RADIUS`] of its centre (the
/// nearest where two are near).
pub fn hit_touch(badges: &[HelpBadge], x: f64, y: f64, px: f64) -> Option<HelpBadge> {
    let near = badges.iter().filter(|b| {
        let r = (f64::from(b.radius) + HIT_SLOP * px).max(TOUCH_RADIUS * px);
        (x - f64::from(b.centre[0])).hypot(y - f64::from(b.centre[1])) <= r
    });
    nearest(near, x, y)
}

/// Distance from the "?" badge's centre to the mini-map's size button below it (points): on a
/// touch screen far enough apart that each keeps its whole fingertip target.
pub fn map_button_spacing(touch: bool) -> f64 {
    if touch { 2.0 * TOUCH_RADIUS } else { 2.0 * RADIUS + 4.0 }
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
    fn map_size_button_keeps_its_own_target() {
        for (touch, px) in [(false, 2.0), (true, 3.0)] {
            let mut scene = Scene::default();
            let mut badges = Vec::new();
            let mut key = String::new();
            let c = top_right([0.0, 0.0, 600.0, 430.0], px as f32);
            let below = [c[0], c[1] + (map_button_spacing(touch) * px) as f32];
            draw(&mut scene, &mut badges, HelpTopic::MiniMap, c, px as f32, &mut key);
            let n = scene.prims.len();
            draw(
                &mut scene,
                &mut badges,
                HelpTopic::MapEnlarge,
                below,
                px as f32,
                &mut key,
            );
            // The arrows are drawn (shafts and heads), no "?" glyph.
            assert!(scene.prims.len() >= n + 3 + 4, "{}", scene.prims.len() - n);
            let find = |x: f32, y: f32| {
                let (x, y) = (f64::from(x), f64::from(y));
                if touch {
                    hit_touch(&badges, x, y, px)
                } else {
                    hit(&badges, x, y, px)
                }
                .map(|b| b.topic)
            };
            assert_eq!(find(c[0], c[1]), Some(HelpTopic::MiniMap));
            assert_eq!(find(below[0], below[1]), Some(HelpTopic::MapEnlarge));
            // Just above the middle between them: still the "?"; just below: the button.
            let mid = 0.5 * (c[1] + below[1]);
            assert_eq!(find(c[0], mid - px as f32), Some(HelpTopic::MiniMap));
            assert_eq!(find(c[0], mid + px as f32), Some(HelpTopic::MapEnlarge));
            if touch {
                // A whole fingertip around each: the targets do not overlap.
                let r = (TOUCH_RADIUS * px) as f32;
                assert_eq!(find(below[0], below[1] - r + px as f32), Some(HelpTopic::MapEnlarge));
                assert_eq!(find(below[0] - r + px as f32, below[1]), Some(HelpTopic::MapEnlarge));
            }
            assert!(HelpTopic::MapShrink.is_map_size() && !HelpTopic::MiniMap.is_map_size());
        }
    }

    #[test]
    fn question_mark_is_in_the_font() {
        assert!(crate::font::glyph('?').is_some());
    }
}
