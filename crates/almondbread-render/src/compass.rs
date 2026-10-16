//! The compass (kb: decisions/controls, "Compass"): a small rose at the top right of the canvas
//! whose red needle points to the plane's north (+im), drawn into the overlay scene like the
//! mini-map. Shown while the view is turned (or always, a setting), it fades out a few seconds
//! after the view is back at north. A click eases the view back to north, a drag turns it; the
//! hit test lives here, the motion in the core `Navigator`, the policy and the maths in
//! `almondbread_core::compass`. Costs nothing while hidden (no primitives, no scene).

use almondbread_core::compass::{self, Visibility};

use crate::overlay::{Scene, rgba};

/// A press on the compass being followed: where it started and where the pointer was last
/// (view points from the compass centre, y up), and whether it has become a drag.
#[derive(Clone, Copy, Debug)]
struct Drag {
    start: (f64, f64),
    last: (f64, f64),
    moved: bool,
}

/// Compass state of one view.
#[derive(Default)]
pub struct CompassOverlay {
    pub visibility: Visibility,
    /// Extra top margin (view points): platform buttons in the top-right corner (iPad).
    pub inset_top: f64,
    /// The rose's centre as last laid out: view points from the view centre, y up.
    centre: Option<(f64, f64)>,
    drag: Option<Drag>,
}

impl CompassOverlay {
    /// Follows the view (call once per refresh, before the overlay decides to draw).
    pub fn update(&mut self, angle: f64, turning_north: bool, now: f64) {
        self.visibility.update(angle, turning_north || self.drag.is_some(), now);
    }

    pub fn shown(&self, now: f64) -> bool {
        self.visibility.shown(now)
    }

    /// The fade after north needs frames.
    pub fn animating(&self, now: f64) -> bool {
        self.visibility.fading(now)
    }

    /// The rose's centre as last laid out (view points from the view centre, y up).
    pub fn center(&self) -> Option<(f64, f64)> {
        self.centre
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Lays the rose out for a view of `view` points with canvas `insets` (top, left, bottom,
    /// right); `left_of`: x (view points from the centre) the rose must stay left of (map
    /// panels in the top-right corner). Returns the centre, view points from the centre, y up.
    pub fn layout(&mut self, view: (f64, f64), insets: (f64, f64, f64, f64), left_of: Option<f64>) -> (f64, f64) {
        let r = compass::RADIUS + compass::MARGIN;
        let mut x = view.0 / 2.0 - insets.3 - r;
        if let Some(l) = left_of {
            x = x.min(l - r);
        }
        let y = view.1 / 2.0 - insets.0 - self.inset_top.max(0.0) - r;
        self.centre = Some((x, y));
        (x, y)
    }

    /// A press at (x, y) view points from the view centre, y up: on the (visible) rose it
    /// starts following the pointer and returns true; elsewhere false.
    pub fn press(&mut self, x: f64, y: f64, now: f64) -> bool {
        let Some(c) = self.centre else { return false };
        if !self.visibility.clickable(now) || !compass::hit(c, compass::RADIUS, x, y, compass::HIT_SLOP) {
            return false;
        }
        let p = (x - c.0, y - c.1);
        self.drag = Some(Drag {
            start: p,
            last: p,
            moved: false,
        });
        true
    }

    /// The pointer moved to (x, y) during a press: the view turn that keeps the needle under it
    /// (radians, added to the view's angle). 0 until the pointer has left the click radius;
    /// then the needle catches up with it from the press point, 1:1.
    pub fn drag_to(&mut self, x: f64, y: f64) -> f64 {
        let (Some(c), Some(d)) = (self.centre, self.drag.as_mut()) else {
            return 0.0;
        };
        let p = (x - c.0, y - c.1);
        if !d.moved && (p.0 - d.start.0).hypot(p.1 - d.start.1) < compass::CLICK_TRAVEL {
            return 0.0;
        }
        d.moved = true;
        let turn = compass::drag_turn(d.last, p);
        d.last = p;
        turn
    }

    /// The press ended: true if it was a click (it never became a drag; the view then turns
    /// to north).
    pub fn release(&mut self) -> bool {
        self.drag.take().is_some_and(|d| !d.moved)
    }

    /// Draws the rose for a view turned by `angle` into a `size` pixel target; appends what
    /// it drew to `key`. Nothing while hidden.
    pub fn draw(&self, scene: &mut Scene, size: (usize, usize), ppp: f64, angle: f64, now: f64, key: &mut String) {
        let alpha = self.visibility.alpha(now) as f32;
        let Some(c) = self.centre else { return };
        if alpha <= 0.0 {
            return;
        }
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64, size.1 as f64);
        let (cx, cy) = (w / 2.0 + c.0 * px, h / 2.0 - c.1 * px);
        let r = compass::RADIUS * px;
        let a = |v: f32| v * alpha;
        scene.set_clip(None);
        // Body: a dark translucent disc with a thin light rim (brighter while dragged).
        scene.disc([cx, cy], (r + 1.0 * px) as f32, rgba(0.0, 0.0, 0.0, a(0.25)));
        scene.disc([cx, cy], r as f32, rgba(0.035, 0.035, 0.045, a(0.68)));
        let rim = if self.drag.is_some() { 0.6 } else { 0.28 };
        scene.ring([cx, cy], (r - 0.5 * px) as f32, px as f32, rgba(1.0, 1.0, 1.0, a(rim)));
        // Screen direction of north (pixels: y down).
        let n = compass::needle_angle(angle);
        let dir = |phi: f64| (phi.cos(), -phi.sin());
        let at = |phi: f64, rad: f64| {
            let (dx, dy) = dir(phi);
            [cx + dx * rad, cy + dy * rad]
        };
        // Ticks: east, south and west longer, the diagonals short (north has the label).
        for k in 1..8 {
            let phi = n - k as f64 * std::f64::consts::FRAC_PI_4;
            let long = k % 2 == 0;
            let (r0, r1) = if long {
                (r - 6.0 * px, r - 2.5 * px)
            } else {
                (r - 4.5 * px, r - 2.5 * px)
            };
            let o = if long { 0.6 } else { 0.38 };
            scene.segment(
                at(phi, r0),
                at(phi, r1),
                (0.55 * px) as f32,
                rgba(1.0, 1.0, 1.0, a(o)),
                rgba(1.0, 1.0, 1.0, a(o)),
            );
        }
        // Needle: a kite, red north half, pale south half, each with a lit and a shaded side.
        let tip = 0.47 * r;
        let half = 0.15 * r;
        let north = at(n, tip);
        let south = at(n + std::f64::consts::PI, tip);
        let left = at(n + std::f64::consts::FRAC_PI_2, half);
        let right = at(n - std::f64::consts::FRAC_PI_2, half);
        let centre = [cx, cy];
        scene.triangle(north, left, right, rgba(0.70, 0.05, 0.04, a(1.0)));
        scene.triangle(north, left, centre, rgba(0.98, 0.16, 0.10, a(1.0)));
        scene.triangle(south, left, right, rgba(0.62, 0.64, 0.68, a(1.0)));
        scene.triangle(south, right, centre, rgba(0.93, 0.94, 0.96, a(1.0)));
        scene.disc(centre, (1.4 * px) as f32, rgba(0.05, 0.05, 0.06, a(0.9)));
        // A math-italic "i", upright, beyond the needle's tip: north is the +imaginary direction
        // (user idea). Drawn from strokes, not the font: Inter's sans "i" read as a tick mark.
        // Points in view points around the label centre, y down: entry serif, slanted stem,
        // foot curling right, round dot.
        let p = at(n, 0.68 * r);
        let q = |x: f64, y: f64| [p[0] + x * px, p[1] + y * px];
        let ink = rgba(1.0, 1.0, 1.0, a(0.95));
        let w = (0.85 * px) as f32;
        let stroke = |scene: &mut Scene, pts: &[(f64, f64)]| {
            for s in pts.windows(2) {
                scene.segment(q(s[0].0, s[0].1), q(s[1].0, s[1].1), w, ink, ink);
            }
        };
        stroke(scene, &[(-1.4, -1.4), (0.8, -2.0), (-0.5, 3.0), (0.2, 3.7), (1.6, 3.0)]);
        scene.disc(q(1.3, -4.6), (0.8 * px) as f32, ink);
        key.push_str(&format!(
            "compass {cx:.1} {cy:.1} {:.5} {alpha:.3} {} ",
            n,
            self.drag.is_some()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown() -> CompassOverlay {
        let mut c = CompassOverlay::default();
        c.update(0.5, false, 0.0);
        c.layout((1000.0, 600.0), (40.0, 0.0, 0.0, 300.0), None);
        c
    }

    #[test]
    fn sits_top_right_clear_of_insets_and_panels() {
        let mut c = shown();
        let (x, y) = c.layout((1000.0, 600.0), (40.0, 0.0, 0.0, 300.0), None);
        let r = compass::RADIUS + compass::MARGIN;
        assert_eq!((x, y), (500.0 - 300.0 - r, 300.0 - 40.0 - r));
        let (x, _) = c.layout((1000.0, 600.0), (40.0, 0.0, 0.0, 0.0), Some(100.0));
        assert_eq!(x, 100.0 - r);
        c.inset_top = 56.0;
        let (_, y) = c.layout((1000.0, 600.0), (40.0, 0.0, 0.0, 300.0), None);
        assert_eq!(y, 300.0 - 40.0 - 56.0 - r);
    }

    #[test]
    fn click_and_drag_on_the_rose() {
        let mut c = shown();
        let (x, y) = c.centre.unwrap();
        // Off the rose: not ours.
        assert!(!c.press(x - 60.0, y, 0.0));
        // A click: little travel.
        assert!(c.press(x + 10.0, y, 0.0));
        assert_eq!(c.drag_to(x + 11.0, y + 1.0), 0.0);
        assert!(c.release());
        // A drag: a quarter turn of the pointer around the centre turns the needle as much.
        assert!(c.press(x + 15.0, y, 0.0));
        let mut turn = 0.0;
        for k in 1..=9 {
            let phi = k as f64 * 10f64.to_radians();
            turn += c.drag_to(x + 15.0 * phi.cos(), y + 15.0 * phi.sin());
        }
        assert!((turn + std::f64::consts::FRAC_PI_2).abs() < 1e-9, "{turn}");
        assert!(!c.release());
        // Hidden: no hits.
        let mut h = CompassOverlay::default();
        h.layout((1000.0, 600.0), (0.0, 0.0, 0.0, 0.0), None);
        let (x, y) = h.centre.unwrap();
        assert!(!h.press(x, y, 0.0));
    }

    #[test]
    fn draws_nothing_while_hidden() {
        let mut c = CompassOverlay::default();
        c.layout((1000.0, 600.0), (0.0, 0.0, 0.0, 0.0), None);
        let mut s = Scene::default();
        let mut key = String::new();
        c.draw(&mut s, (2000, 1200), 0.5, 0.0, 0.0, &mut key);
        assert!(s.prims.is_empty() && key.is_empty());
        c.update(0.3, false, 0.0);
        c.draw(&mut s, (2000, 1200), 0.5, 0.3, 0.0, &mut key);
        assert!(s.prims.len() > 10 && s.prims.len() < 30, "{}", s.prims.len());
    }
}
