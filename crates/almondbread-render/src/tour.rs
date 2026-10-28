//! What a guided tour (kb: decisions/controls "Guided tours"; core `tour`) puts on the overlay:
//! the orbit probe's point when the tour sets it (instead of the pointer), a highlighted
//! point with its label, the step's labelled points (`labels:`) and its circle (`circle:`). The
//! host fills them from the tour player every refresh.

use almondbread_core::tour::{Circle, Marker};
use almondbread_core::viewport::{Center, Viewport};

use crate::overlay::{Scene, rgba};

/// Marker ring radius and width, view points.
const RING: f64 = 11.0;
const RING_WIDTH: f64 = 1.6;
/// Label size, view points.
const LABEL_SIZE: f64 = 13.0;
/// Gold: apart from the autopilot's white target marker and the orbit's warm dots.
const GOLD: [f32; 3] = [1.0, 0.78, 0.25];
/// A labelled point's dot radius and the gap between the dot and its text, view points.
const DOT: f64 = 3.5;
const DOT_GAP: f64 = 5.0;
/// Labels keep this far from the canvas edges, view points.
const EDGE: f64 = 6.0;
/// A circle's line width, view points: thinner than the orbit's line (1.6) and its dots.
const CIRCLE_WIDTH: f64 = 1.3;

/// The tour's overlay state (empty without a tour).
#[derive(Clone, Debug, Default)]
pub struct TourOverlay {
    /// The orbit probe's point (None: the pointer, as usual).
    pub probe: Option<Center>,
    pub marker: Option<Marker>,
    /// The step's labelled points (`labels:`).
    pub labels: Vec<Marker>,
    /// The step's circle (`circle:`).
    pub circle: Option<Circle>,
}

impl TourOverlay {
    /// Something to draw (the marker, labelled points or a circle).
    pub fn shown(&self) -> bool {
        self.marker.is_some() || !self.labels.is_empty() || self.circle.is_some()
    }

    /// Draws the marker (a gold double ring and its label) and the labelled points where they
    /// are on screen; labels keep clear of the tour card `card` (pixels: x0, y0, x1, y1).
    pub fn draw(
        &self,
        scene: &mut Scene,
        size: (usize, usize),
        ppp: f64,
        view: &Viewport,
        card: Option<[f32; 4]>,
        key: &mut String,
    ) {
        if ppp <= 0.0 {
            return;
        }
        // The circle under the marker and the labels; its text and the marker's keep the
        // labelled points' texts clear.
        let ring_text = self.draw_circle(scene, size, ppp, view, card, key);
        let marker_text = self.draw_marker(scene, size, ppp, view, key);
        let taken: Vec<[f32; 4]> = ring_text.into_iter().chain(marker_text).collect();
        self.draw_labels(scene, size, ppp, view, &taken, card, key);
    }

    /// The circle: a thin gold ring on a dark one (seen on bright and dark palettes), its text
    /// beside the first point of the ring, going round from the top right, that is on the
    /// canvas and not under the tour card. Returns the text's box (pixels).
    fn draw_circle(
        &self,
        scene: &mut Scene,
        size: (usize, usize),
        ppp: f64,
        view: &Viewport,
        card: Option<[f32; 4]>,
        key: &mut String,
    ) -> Option<[f32; 4]> {
        let ring = self.circle.as_ref()?;
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64, size.1 as f64);
        let (x, y) = view.offset_of(&ring.center);
        let c = [w / 2.0 + x * px, h / 2.0 - y * px];
        let r = ring.radius / view.scale.to_f64() * px;
        // Beyond the overlay kernel's float range, or too small to see: nothing.
        if !(c[0].is_finite() && c[1].is_finite() && r.is_finite()) || r > 1e6 || r < 0.5 {
            return None;
        }
        // The ring crosses the canvas only if the canvas's nearest point is inside it and its
        // farthest corner outside.
        let near = (c[0] - c[0].clamp(0.0, w)).hypot(c[1] - c[1].clamp(0.0, h));
        let far = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
            .iter()
            .map(|&(x, y)| (c[0] - x).hypot(c[1] - y))
            .fold(0.0, f64::max);
        if r < near || r > far {
            return None;
        }
        let rw = (CIRCLE_WIDTH * px) as f32;
        scene.ring(c, r as f32, rw + 2.0 * px as f32, rgba(0.0, 0.0, 0.0, 0.45));
        scene.ring(c, r as f32, rw, rgba(GOLD[0], GOLD[1], GOLD[2], 0.85));
        key.push_str(&format!("circ {:.1},{:.1},{:.1};", c[0], c[1], r));
        if ring.label.is_empty() {
            return None;
        }
        let edge = EDGE * px;
        let under_card = |p: [f64; 2]| {
            card.is_some_and(|k| {
                let (x, y) = (p[0] as f32, p[1] as f32);
                x >= k[0] - 20.0 && x <= k[2] && y >= k[1] - 20.0 && y <= k[3]
            })
        };
        // From the top right round anticlockwise, in steps of 15°.
        let spot = (0..24)
            .map(|k| (45.0 + 15.0 * f64::from(k)).to_radians())
            .map(|a| [c[0] + r * a.cos(), c[1] - r * a.sin()])
            .find(|p| p[0] > edge && p[1] > edge && p[0] < w - edge && p[1] < h - edge && !under_card(*p))?;
        let size_px = (LABEL_SIZE * px) as f32;
        let tw = crate::font::layout(&ring.label, size_px).1;
        let b = place_label(spot, [tw, size_px], [w as f32, h as f32], px as f32, card.as_slice());
        scene.label([b[0], b[1]], size_px, &ring.label, rgba(GOLD[0], GOLD[1], GOLD[2], 1.0));
        key.push_str(&ring.label);
        Some(b)
    }

    /// The marker; returns the box its label took (pixels), so labelled points keep clear.
    fn draw_marker(
        &self,
        scene: &mut Scene,
        size: (usize, usize),
        ppp: f64,
        view: &Viewport,
        key: &mut String,
    ) -> Option<[f32; 4]> {
        let m = self.marker.as_ref()?;
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64, size.1 as f64);
        let (x, y) = view.offset_of(&m.at);
        let c = [w / 2.0 + x * px, h / 2.0 - y * px];
        if !(c[0].is_finite() && c[1].is_finite()) || c[0] < -50.0 || c[1] < -50.0 || c[0] > w + 50.0 || c[1] > h + 50.0
        {
            return None;
        }
        let gold = rgba(GOLD[0], GOLD[1], GOLD[2], 0.95);
        let (r, rw) = ((RING * px) as f32, (RING_WIDTH * px) as f32);
        scene.ring(c, r, rw + 2.0 * px as f32, rgba(0.0, 0.0, 0.0, 0.5));
        scene.ring(c, r, rw, gold);
        scene.ring(
            c,
            r + (4.0 * px) as f32,
            (0.8 * px) as f32,
            rgba(GOLD[0], GOLD[1], GOLD[2], 0.5),
        );
        key.push_str(&format!("tour {:.1},{:.1};", c[0], c[1]));
        if !m.label.is_empty() {
            let size_px = (LABEL_SIZE * px) as f32;
            // Above the ring, so the probe's dots on the axis stay clear.
            let origin = [
                (c[0] - 0.25 * RING * px) as f32,
                (c[1] - (RING + 10.0) * px - LABEL_SIZE * px) as f32,
            ];
            let tw = scene.label(origin, size_px, &m.label, rgba(GOLD[0], GOLD[1], GOLD[2], 1.0));
            key.push_str(&m.label);
            return Some([origin[0], origin[1], origin[0] + tw, origin[1] + size_px]);
        }
        None
    }

    /// The labelled points: a small gold dot each, its text beside it (above right by
    /// preference, else above left, below right, below left: the first place on the canvas
    /// that no earlier label, the marker's or the circle's text or the tour card covers). Points
    /// outside the view or under the card are left out; a text that fits nowhere is pushed onto
    /// the canvas.
    #[allow(clippy::too_many_arguments)]
    fn draw_labels(
        &self,
        scene: &mut Scene,
        size: (usize, usize),
        ppp: f64,
        view: &Viewport,
        taken: &[[f32; 4]],
        card: Option<[f32; 4]>,
        key: &mut String,
    ) {
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64, size.1 as f64);
        let mut boxes: Vec<[f32; 4]> = taken.iter().copied().chain(card).collect();
        let size_px = (LABEL_SIZE * px) as f32;
        let gold = rgba(GOLD[0], GOLD[1], GOLD[2], 1.0);
        for m in &self.labels {
            let (x, y) = view.offset_of(&m.at);
            let c = [w / 2.0 + x * px, h / 2.0 - y * px];
            if !(c[0].is_finite() && c[1].is_finite()) || c[0] < 0.0 || c[1] < 0.0 || c[0] > w || c[1] > h {
                continue;
            }
            let under_card = card.is_some_and(|k| {
                (c[0] as f32) >= k[0] && (c[0] as f32) <= k[2] && (c[1] as f32) >= k[1] && (c[1] as f32) <= k[3]
            });
            if under_card {
                continue;
            }
            let r = (DOT * px) as f32;
            scene.disc(c, r + (1.5 * px) as f32, rgba(0.0, 0.0, 0.0, 0.6));
            scene.disc(c, r, gold);
            key.push_str(&format!("lab {:.1},{:.1};", c[0], c[1]));
            if m.label.is_empty() {
                continue;
            }
            let tw = crate::font::layout(&m.label, size_px).1;
            let b = place_label(c, [tw, size_px], [w as f32, h as f32], px as f32, &boxes);
            scene.label([b[0], b[1]], size_px, &m.label, gold);
            key.push_str(&m.label);
            boxes.push(b);
        }
    }
}

/// Where a label `text` (width, height in pixels) goes next to the dot at `c` on a canvas
/// `canvas` pixels big (`px` pixels per point): its box (x0, y0, x1, y1). Tries above right,
/// above left, below right, below left; the first that stays on the canvas and clear of
/// `taken` wins, else the first that stays on the canvas, else above right pushed onto it.
pub(crate) fn place_label(c: [f64; 2], text: [f32; 2], canvas: [f32; 2], px: f32, taken: &[[f32; 4]]) -> [f32; 4] {
    let (cx, cy) = (c[0] as f32, c[1] as f32);
    let gap = (DOT + DOT_GAP) as f32 * px;
    let edge = EDGE as f32 * px;
    // The outline reaches a little past the glyphs; two texts keep a word space apart, so
    // neighbours never read as one label.
    let halo = 5.0 * px;
    let [tw, th] = text;
    let candidates = [
        [cx + gap * 0.6, cy - gap - th],
        [cx - gap * 0.6 - tw, cy - gap - th],
        [cx + gap * 0.6, cy + gap],
        [cx - gap * 0.6 - tw, cy + gap],
    ]
    .map(|o| [o[0], o[1], o[0] + tw, o[1] + th]);
    let on_canvas = |b: &[f32; 4]| b[0] >= edge && b[1] >= edge && b[2] <= canvas[0] - edge && b[3] <= canvas[1] - edge;
    let clear = |b: &[f32; 4]| {
        !taken.iter().any(|t| {
            b[0] - halo < t[2] + halo
                && t[0] - halo < b[2] + halo
                && b[1] - halo < t[3] + halo
                && t[1] - halo < b[3] + halo
        })
    };
    if let Some(b) = candidates.iter().find(|b| on_canvas(b) && clear(b)) {
        return *b;
    }
    if let Some(b) = candidates.iter().find(|b| on_canvas(b)) {
        return *b;
    }
    let b = candidates[0];
    let x0 = b[0].min(canvas[0] - edge - tw).max(edge);
    let y0 = b[1].min(canvas[1] - edge - th).max(edge);
    [x0, y0, x0 + tw, y0 + th]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab(re: f64, im: f64, text: &str) -> Marker {
        Marker {
            at: Center::new(re, im),
            label: text.to_string(),
        }
    }

    /// Labels stay on the canvas, keep clear of each other where they can, and points outside
    /// the view are left out.
    #[test]
    fn labels_are_placed_on_the_canvas_and_clear_of_each_other() {
        let canvas = [800.0, 500.0];
        // Room above right.
        let b = place_label([400.0, 250.0], [60.0, 13.0], canvas, 1.0, &[]);
        assert!(b[0] > 400.0 && b[3] < 250.0, "{b:?}");
        // At the right edge: above left instead.
        let b = place_label([790.0, 250.0], [60.0, 13.0], canvas, 1.0, &[]);
        assert!(b[2] < 790.0 && b[3] < 250.0, "{b:?}");
        // At the top edge: below.
        let b = place_label([400.0, 5.0], [60.0, 13.0], canvas, 1.0, &[]);
        assert!(b[1] > 5.0, "{b:?}");
        // Taken above right: another place.
        let first = place_label([400.0, 250.0], [60.0, 13.0], canvas, 1.0, &[]);
        let second = place_label([405.0, 250.0], [60.0, 13.0], canvas, 1.0, &[first]);
        assert_ne!(first, second);
        // A label that would only touch another one end to end moves too.
        let near = place_label([330.0, 250.0], [60.0, 13.0], canvas, 1.0, &[first]);
        assert!(near[2] + 8.0 <= first[0] || near[1] >= first[3], "{near:?} {first:?}");
        // Too wide for any place: pushed onto the canvas.
        let b = place_label([10.0, 10.0], [790.0, 13.0], canvas, 1.0, &[]);
        assert!(b[0] >= 0.0 && b[2] <= 800.0, "{b:?}");

        // Drawn through the overlay: two points in view (a dot and a text each), one outside.
        let view = Viewport::new(-0.6, 0.0, 2.6 / 500.0);
        let t = TourOverlay {
            labels: vec![
                lab(-0.75, 0.0, "neck −0.75"),
                lab(-1.25, 0.0, "−1.25"),
                lab(3.0, 0.0, "far"),
            ],
            ..Default::default()
        };
        assert!(t.shown());
        let mut scene = Scene::default();
        let mut key = String::new();
        t.draw(&mut scene, (800, 500), 1.0, &view, None, &mut key);
        assert_eq!(key.matches("lab ").count(), 2, "{key}");
        assert!(key.contains("neck −0.75") && key.contains("−1.25") && !key.contains("far"));
        assert!(!scene.prims.is_empty());
        // A point under the tour card is left out; a label next to the card moves off it.
        let (mut scene, mut key) = (Scene::default(), String::new());
        let (x, _) = view.offset_of(&Center::new(-1.25, 0.0));
        let dot = 400.0 + x as f32;
        t.draw(
            &mut scene,
            (800, 500),
            1.0,
            &view,
            Some([dot - 5.0, 240.0, 800.0, 500.0]),
            &mut key,
        );
        assert_eq!(key.matches("lab ").count(), 0, "{key}");
        let (mut scene, mut key) = (Scene::default(), String::new());
        let card = [dot + 12.0, 200.0, 800.0, 500.0];
        t.draw(&mut scene, (800, 500), 1.0, &view, Some(card), &mut key);
        assert!(key.contains("−1.25"), "{key}");
        let b = place_label([dot as f64, 250.0], [40.0, 13.0], [800.0, 500.0], 1.0, &[card]);
        assert!(b[2] <= card[0], "the label keeps clear of the card: {b:?}");
        assert!(!TourOverlay::default().shown());
    }

    /// A circle is drawn where it crosses the canvas (a ring and its text, the text clear of
    /// the card), and left out where the canvas lies wholly inside or outside it.
    #[test]
    fn circle_is_drawn_where_it_crosses_the_canvas() {
        // 0.005 per point: radius 2 round 0 is 400 pixels, the canvas 800 x 500 round 0.
        let view = Viewport::new(0.0, 0.0, 0.005);
        let ring = |r: f64, text: &str| TourOverlay {
            circle: Some(Circle {
                center: Center::new(0.0, 0.0),
                radius: r,
                label: text.to_string(),
            }),
            ..Default::default()
        };
        let draw = |t: &TourOverlay, card: Option<[f32; 4]>| {
            let (mut scene, mut key) = (Scene::default(), String::new());
            t.draw(&mut scene, (800, 500), 1.0, &view, card, &mut key);
            (scene, key)
        };
        let t = ring(2.0, "2 from 0");
        assert!(t.shown());
        let (scene, key) = draw(&t, None);
        assert!(
            key.contains("circ 400.0,250.0,400.0;") && key.contains("2 from 0"),
            "{key}"
        );
        // Two rings (dark under gold) and the text's glyphs.
        assert!(scene.prims.len() > 2);
        // The text keeps clear of the card in the lower right.
        let card = [400.0, 150.0, 800.0, 500.0];
        let (scene, key) = draw(&t, Some(card));
        assert!(key.contains("2 from 0"));
        let glyphs: Vec<_> = scene.prims.iter().skip(2).collect();
        assert!(
            glyphs.iter().all(|p| p.geom[2] <= card[0] || p.geom[3] <= card[1]),
            "the circle's text is not on the card"
        );
        // The canvas wholly inside the circle (radius 20 = 4000 pixels) or wholly outside it (a
        // circle far off to the right): nothing.
        assert!(!draw(&ring(20.0, "far"), None).1.contains("circ"));
        let off = TourOverlay {
            circle: Some(Circle {
                center: Center::new(10.0, 0.0),
                radius: 1.0,
                label: String::new(),
            }),
            ..Default::default()
        };
        assert!(!draw(&off, None).1.contains("circ"));
        // A small circle inside the canvas is drawn.
        assert!(draw(&ring(0.1, ""), None).1.contains("circ 400.0,250.0,20.0;"));
    }
}
