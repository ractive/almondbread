//! The target marker (kb: decisions/controls "Target marker"): a small ring where the autopilot
//! or a journey leg is heading (the lock target, survey steering's boundary point, the leg's
//! target), a short label for a moment when the target is a known kind ("spiral", "minibrot ·
//! period 34"), and a faint line from the centre towards the pointer wish. Drawn into the
//! overlay scene like the compass; nothing in manual or cruise, nothing while switched off.
//!
//! The marker sits on a complex point, so it moves with the zoom and pan by itself; when the
//! target changes it glides from where it was drawn to the new one ([`GLIDE`] s, smoothstep)
//! and it fades in and out ([`FADE`] s). Its state advances with the clock, not per frame.

use almondbread_core::input::AimMarker;
use almondbread_core::viewport::{Center, Viewport};

use crate::overlay::{Scene, rgba};

/// Glide to a new target, label time and fade (s).
pub const GLIDE: f64 = 0.6;
pub const LABEL_TIME: f64 = 2.5;
pub const FADE: f64 = 0.35;
/// Ring radius and width, label size, wish line from / to (points; the line's end is a share
/// of the shorter side), edge margin for a target off the canvas.
pub const RING: f64 = 9.0;
pub const RING_WIDTH: f64 = 1.5;
pub const LABEL_SIZE: f64 = 11.0;
pub const WISH_FROM: f64 = 16.0;
pub const WISH_TO: f64 = 0.22;
pub const EDGE: f64 = 14.0;

/// Target marker state of one view.
#[derive(Clone, Debug)]
pub struct TargetMarker {
    /// The setting (View menu, control center); on by default.
    pub enabled: bool,
    /// The target, and the point (complex) and time the glide to it started from.
    target: Option<Center>,
    from: Option<(Center, f64)>,
    /// The label of the current target and when it was set.
    label: Option<(String, f64)>,
    /// The wish direction (unit, view points, y up), last non-empty one (for the fade).
    wish: Option<(f64, f64)>,
    /// Fade levels of the ring and the wish line (0...1) and the clock of the last update.
    alpha: f64,
    wish_alpha: f64,
    last: f64,
    /// Show the ring / the line (targets of the fades).
    on: bool,
    wish_on: bool,
}

impl Default for TargetMarker {
    fn default() -> Self {
        Self {
            enabled: true,
            target: None,
            from: None,
            label: None,
            wish: None,
            alpha: 0.0,
            wish_alpha: 0.0,
            last: f64::NAN,
            on: false,
            wish_on: false,
        }
    }
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fade(a: f64, on: bool, dt: f64) -> f64 {
    let step = dt / FADE;
    if on { (a + step).min(1.0) } else { (a - step).max(0.0) }
}

impl TargetMarker {
    /// Follows what the navigator aims at (`m`, None in manual and cruise) in `view` at `now`.
    pub fn update(&mut self, m: Option<&AimMarker>, view: &Viewport, now: f64) {
        let dt = if self.last.is_finite() {
            (now - self.last).clamp(0.0, 0.25)
        } else {
            0.0
        };
        self.last = now;
        let m = m.filter(|_| self.enabled);
        let center = m.and_then(|m| m.center.as_ref());
        match center {
            Some(c) if self.target.as_ref() != Some(c) => {
                // Glide from where the ring is drawn (if it shows), else appear there.
                self.from = if self.alpha > 0.0 {
                    self.position(view, now).map(|(x, y)| (view.point_at_offset(x, y), now))
                } else {
                    None
                };
                self.target = Some(c.clone());
                self.label = m.and_then(|m| m.label.clone()).map(|l| (l, now));
            }
            _ => {}
        }
        self.on = center.is_some();
        if let Some(w) = m.and_then(|m| m.wish) {
            self.wish = Some(w);
        }
        self.wish_on = m.is_some_and(|m| m.wish.is_some());
        self.alpha = fade(self.alpha, self.on, dt);
        self.wish_alpha = fade(self.wish_alpha, self.wish_on, dt);
        if self.alpha <= 0.0 && !self.on {
            self.target = None;
            self.from = None;
            self.label = None;
        }
        if self.wish_alpha <= 0.0 && !self.wish_on {
            self.wish = None;
        }
    }

    /// Anything to draw.
    pub fn visible(&self) -> bool {
        self.alpha > 0.0 || self.wish_alpha > 0.0
    }

    /// Fading, gliding or showing a label: frames needed.
    pub fn animating(&self, now: f64) -> bool {
        (self.alpha > 0.0 && self.alpha < 1.0)
            || (self.wish_alpha > 0.0 && self.wish_alpha < 1.0)
            || (self.alpha > 0.0 && !self.on)
            || (self.wish_alpha > 0.0 && !self.wish_on)
            || self.from.as_ref().is_some_and(|(_, t0)| now - t0 < GLIDE)
            || self.label.as_ref().is_some_and(|(_, t0)| now - t0 < LABEL_TIME + FADE)
    }

    /// Where the ring is drawn: view points from the centre, y up (None without a target).
    pub fn position(&self, view: &Viewport, now: f64) -> Option<(f64, f64)> {
        let (x, y) = view.offset_of(self.target.as_ref()?);
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        let Some((from, t0)) = &self.from else {
            return Some((x, y));
        };
        let s = smoothstep((now - t0) / GLIDE);
        let (fx, fy) = view.offset_of(from);
        if s >= 1.0 || !(fx.is_finite() && fy.is_finite()) {
            return Some((x, y));
        }
        Some((fx + (x - fx) * s, fy + (y - fy) * s))
    }

    /// Draws into a `size` pixel target (`ppp` view points per pixel); appends what it drew to
    /// `key` (the scene is rebuilt when it changes).
    pub fn draw(&self, scene: &mut Scene, size: (usize, usize), ppp: f64, view: &Viewport, now: f64, key: &mut String) {
        if !self.visible() || ppp <= 0.0 {
            return;
        }
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64, size.1 as f64);
        let (hw, hh) = (w * ppp / 2.0, h * ppp / 2.0);
        let to_px = |x: f64, y: f64| [w / 2.0 + x * px, h / 2.0 - y * px];
        // The wish line: faint, from just outside the centre towards the wish.
        if let Some((dx, dy)) = self.wish
            && self.wish_alpha > 0.0
        {
            let a = (0.45 * self.wish_alpha) as f32;
            let len = WISH_TO * (2.0 * hw).min(2.0 * hh);
            if len > WISH_FROM {
                let p0 = to_px(dx * WISH_FROM, dy * WISH_FROM);
                let p1 = to_px(dx * len, dy * len);
                let lw = (0.75 * px) as f32;
                scene.segment(
                    p0,
                    p1,
                    lw + px as f32,
                    rgba(0.0, 0.0, 0.0, 0.5 * a),
                    rgba(0.0, 0.0, 0.0, 0.0),
                );
                scene.segment(p0, p1, lw, rgba(1.0, 1.0, 1.0, a), rgba(1.0, 1.0, 1.0, 0.0));
                key.push_str(&format!("w{:.0},{:.0},{:.2};", p1[0], p1[1], a));
            }
        }
        let Some((x, y)) = self.position(view, now) else { return };
        if self.alpha <= 0.0 {
            return;
        }
        // A target off the canvas: at the edge, in its direction.
        let (mx, my) = (hw - EDGE, hh - EDGE);
        let k = if mx > 0.0 && my > 0.0 {
            (x.abs() / mx).max(y.abs() / my).max(1.0)
        } else {
            1.0
        };
        let (x, y) = (x / k, y / k);
        let c = to_px(x, y);
        let a = (0.85 * self.alpha) as f32;
        let (r, rw) = ((RING * px) as f32, (RING_WIDTH * px) as f32);
        // A dark halo under a light ring: reads on bright and dark palettes.
        scene.ring(c, r, rw + 2.0 * px as f32, rgba(0.0, 0.0, 0.0, 0.45 * a));
        scene.ring(c, r, rw, rgba(1.0, 1.0, 1.0, a));
        scene.disc(c, (1.4 * px) as f32, rgba(1.0, 1.0, 1.0, a));
        key.push_str(&format!("m{:.1},{:.1},{:.2};", c[0], c[1], a));
        if let Some((text, t0)) = &self.label {
            let age = now - t0;
            let la = self.alpha * (1.0 - ((age - LABEL_TIME) / FADE).clamp(0.0, 1.0));
            if la > 0.0 {
                let size_px = (LABEL_SIZE * px) as f32;
                let origin = [(c[0] + (RING + 6.0) * px) as f32, (c[1] - 0.5 * LABEL_SIZE * px) as f32];
                scene.label(origin, size_px, text, rgba(1.0, 1.0, 1.0, (0.9 * la) as f32));
                key.push_str(&format!("l{text}{la:.2};"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(c: Option<Center>, label: Option<&str>, wish: Option<(f64, f64)>) -> AimMarker {
        AimMarker {
            center: c,
            label: label.map(str::to_string),
            wish,
        }
    }

    /// Kb: decisions/controls "Target marker": the ring sits on the target while the view
    /// zooms and pans, glides (no jump) to a new target, fades out when nothing is aimed at,
    /// and draws nothing while switched off.
    #[test]
    fn marker_follows_the_target() {
        let mut v = Viewport::new(-0.745, 0.11, 1e-5);
        let a = v.point_at_offset(120.0, -40.0);
        let b = v.point_at_offset(-200.0, 90.0);
        let mut m = TargetMarker::default();
        let mut t = 0.0;
        // Fades in on the target, wherever the view goes.
        for k in 0..120 {
            t = k as f64 / 60.0;
            v.zoom_by(1.02, 30.0, 10.0);
            v.pan_by_points(0.5, -0.3);
            m.update(Some(&marker(Some(a.clone()), Some("spiral"), None)), &v, t);
            let (x, y) = m.position(&v, t).expect("position");
            let (tx, ty) = v.offset_of(&a);
            assert!((x - tx).abs() < 1e-6 && (y - ty).abs() < 1e-6, "{k}");
        }
        assert!(m.visible() && m.alpha == 1.0);
        // A new target: it glides, each frame a small step, and arrives after GLIDE.
        let mut last = m.position(&v, t).unwrap();
        let (bx, by) = v.offset_of(&b);
        // Smoothstep's steepest step is 1.5× the mean one.
        let step = 1.6 * (bx - last.0).hypot(by - last.1) / (GLIDE * 60.0);
        let mut arrived = None;
        for k in 1..=60 {
            let tk = t + k as f64 / 60.0;
            m.update(Some(&marker(Some(b.clone()), None, Some((1.0, 0.0)))), &v, tk);
            let p = m.position(&v, tk).unwrap();
            assert!(
                (p.0 - last.0).hypot(p.1 - last.1) <= step,
                "jump at {k}: {last:?} -> {p:?}"
            );
            last = p;
            let (bx, by) = v.offset_of(&b);
            if arrived.is_none() && (p.0 - bx).hypot(p.1 - by) < 1e-6 {
                arrived = Some(k as f64 / 60.0);
            }
        }
        let arrived = arrived.expect("arrived");
        assert!((GLIDE - 0.02..=GLIDE + 0.02).contains(&arrived), "{arrived}");
        // The scene: two rings and a dot, the wish line (2 segments); the label of b (none).
        let mut scene = Scene::default();
        let mut key = String::new();
        m.draw(&mut scene, (1200, 800), 1.0, &v, t + 1.0, &mut key);
        assert_eq!(scene.prims.len(), 5, "{key}");
        // Nothing aimed at: fades out, then nothing to draw.
        let t2 = t + 1.0;
        m.update(None, &v, t2);
        assert!(m.visible() && m.animating(t2));
        for k in 1..=30 {
            m.update(None, &v, t2 + k as f64 / 60.0);
        }
        assert!(!m.visible());
        // Switched off: nothing even while aiming.
        m.enabled = false;
        for k in 0..60 {
            m.update(
                Some(&marker(Some(a.clone()), None, Some((0.0, 1.0)))),
                &v,
                t2 + 1.0 + k as f64 / 60.0,
            );
        }
        assert!(!m.visible());
    }
}
