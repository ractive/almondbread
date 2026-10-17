//! What a guided tour (kb: decisions/controls "Guided tours"; core `tour`) puts on the overlay:
//! the orbit probe's point when the tour sets it (instead of the pointer) and a highlighted
//! point with its label. The host fills both from the tour player every refresh.

use almondbread_core::tour::Marker;
use almondbread_core::viewport::{Center, Viewport};

use crate::overlay::{Scene, rgba};

/// Marker ring radius and width, view points.
const RING: f64 = 11.0;
const RING_WIDTH: f64 = 1.6;
/// Label size, view points.
const LABEL_SIZE: f64 = 13.0;
/// Gold: apart from the autopilot's white target marker and the orbit's warm dots.
const GOLD: [f32; 3] = [1.0, 0.78, 0.25];

/// The tour's overlay state (empty without a tour).
#[derive(Clone, Debug, Default)]
pub struct TourOverlay {
    /// The orbit probe's point (None: the pointer, as usual).
    pub probe: Option<Center>,
    pub marker: Option<Marker>,
}

impl TourOverlay {
    /// Draws the marker (a gold double ring and its label) where it is on screen.
    pub fn draw(&self, scene: &mut Scene, size: (usize, usize), ppp: f64, view: &Viewport, key: &mut String) {
        let Some(m) = &self.marker else { return };
        if ppp <= 0.0 {
            return;
        }
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64, size.1 as f64);
        let (x, y) = view.offset_of(&m.at);
        let c = [w / 2.0 + x * px, h / 2.0 - y * px];
        if !(c[0].is_finite() && c[1].is_finite()) || c[0] < -50.0 || c[1] < -50.0 || c[0] > w + 50.0 || c[1] > h + 50.0
        {
            return;
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
            scene.label(origin, size_px, &m.label, rgba(GOLD[0], GOLD[1], GOLD[2], 1.0));
            key.push_str(&m.label);
        }
    }
}
