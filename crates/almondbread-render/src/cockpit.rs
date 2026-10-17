//! The HUD cockpit (kb: decisions/controls "Driving levels"): a small strip at the bottom of the
//! canvas, drawn into the overlay scene like the compass. Like a car's gear selector
//! (P R N D) it shows the driving level as `M · C · A · J`, the current one lit; a thin speed
//! gauge with zero in the middle (reverse, zooming out, to the left; forward to the right) and
//! the set speed ("↓ ×2/s" out, "↑ ×1.5/s" in); and a small lane icon for lane assist (dim when
//! off, lit when on, brighter for a moment when it nudges). Monochrome, on a dark translucent
//! plate with haloed text, so it reads on bright and dark palettes; about 270 points wide, so it
//! fits a phone. Shown with the HUD (the platform says so); the state comes from the core
//! `Navigator::cockpit`.

use almondbread_core::cruise::{self, Level};
use almondbread_core::input::Cockpit;

use crate::overlay::{Scene, rgba};

/// Cockpit layout and visibility of one view.
#[derive(Clone, Debug, Default)]
pub struct CockpitOverlay {
    /// Shown (with the HUD).
    pub enabled: bool,
    /// Extra right and bottom margins (view points): iPhone buttons at the bottom right.
    pub inset_right: f64,
    pub inset_bottom: f64,
}

/// Font size (points) of the level letters and of the speed.
const LETTER: f64 = 12.0;
const VALUE: f64 = 11.0;
/// Horizontal metrics (points).
const PAD: f64 = 10.0;
const LETTER_PITCH: f64 = 15.0;
const GAUGE: f64 = 84.0;
const VALUE_SLOT: f64 = 66.0;
const LANE: f64 = 12.0;
const GAP: f64 = 10.0;
const HEIGHT: f64 = 24.0;
const MARGIN: f64 = 12.0;

/// The panel's width in points.
pub fn width() -> f64 {
    2.0 * PAD + 3.0 * LETTER_PITCH + LETTER + GAP + GAUGE + GAP + VALUE_SLOT + GAP + LANE
}

/// Gauge deflection 0…1 of a zoom rate (e-folds/s): logarithmic, the top rung at the end.
pub fn gauge(rate: f64) -> f64 {
    let m = rate.abs();
    if !m.is_finite() {
        return 1.0;
    }
    ((m / 0.2).ln_1p() / (cruise::max_speed() / 0.2).ln_1p()).clamp(0.0, 1.0)
}

/// The speed text: the set speed, or the rate while nothing is set (braking, Space zooming);
/// empty at rest.
pub fn value_text(c: &Cockpit) -> String {
    let v = if c.set != 0.0 { c.set } else { c.rate };
    if v.abs() < 0.02 {
        return String::new();
    }
    let arrow = if v < 0.0 { '↓' } else { '↑' };
    format!("{arrow} {}/s", cruise::factor_text(v))
}

impl CockpitOverlay {
    /// The panel in pixels (x0, y0, x1, y1) for a `size` pixel target with `ppp` points per
    /// pixel and canvas `insets` (top, left, bottom, right; view points), centred at the bottom
    /// between `left` (view points from the left edge, e.g. the cosmic scale) and the right edge.
    pub fn layout(&self, size: (usize, usize), ppp: f64, insets: (f64, f64, f64, f64), left: f64) -> [f64; 4] {
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64 * ppp, size.1 as f64 * ppp);
        let pw = width();
        let lo = insets.1.max(left) + MARGIN;
        let hi = w - insets.3 - self.inset_right - MARGIN;
        let mut x0 = (w - pw) / 2.0;
        if x0 < lo {
            x0 = lo;
        }
        if x0 + pw > hi {
            x0 = (hi - pw).max(insets.1 + MARGIN.min(4.0));
        }
        let y1 = h - insets.2 - self.inset_bottom - MARGIN;
        [x0 * px, (y1 - HEIGHT) * px, (x0 + pw) * px, y1 * px]
    }

    /// Draws the cockpit; appends what it drew to `key`. Nothing while disabled.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        scene: &mut Scene,
        size: (usize, usize),
        ppp: f64,
        insets: (f64, f64, f64, f64),
        left: f64,
        c: &Cockpit,
        key: &mut String,
    ) {
        if !self.enabled || ppp <= 0.0 || size.0 == 0 || size.1 == 0 {
            return;
        }
        let px = 1.0 / ppp;
        let r = self.layout(size, ppp, insets, left);
        let p = |v: f64| (v * px) as f32;
        scene.set_clip(None);
        // Plate.
        scene.rect(
            [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32],
            p(8.0),
            rgba(0.03, 0.03, 0.04, 0.46),
            rgba(1.0, 1.0, 1.0, 0.10),
            p(0.75),
        );
        let yc = (r[1] + r[3]) / 2.0;
        let lit = rgba(1.0, 1.0, 1.0, 0.96);
        let dim = rgba(1.0, 1.0, 1.0, 0.34);
        // Gear letters, the current one lit with a short bar under it.
        let mut x = r[0] + PAD * px;
        let cap = crate::font::cap_height() * LETTER as f32 * px as f32;
        for (i, l) in Level::ALL.iter().enumerate() {
            let s = l.letter().to_string();
            let wl = crate::font::layout(&s, p(LETTER)).1 as f64;
            let cx = x + LETTER * px / 2.0;
            let on = *l == c.level;
            scene.label(
                [(cx - wl / 2.0) as f32, (yc - cap as f64 / 2.0 - 1.0 * px) as f32],
                p(LETTER),
                &s,
                if on { lit } else { dim },
            );
            if on {
                scene.rect(
                    [
                        (cx - 4.0 * px) as f32,
                        (yc + 7.0 * px) as f32,
                        (cx + 4.0 * px) as f32,
                        (yc + 8.2 * px) as f32,
                    ],
                    p(0.6),
                    lit,
                    [0.0; 4],
                    0.0,
                );
            }
            if i < 3 {
                scene.disc([cx + LETTER_PITCH * px / 2.0, yc], p(0.9), dim);
            }
            x += LETTER_PITCH * px;
        }
        // Gauge: a track with the zero tick in the middle, the rate as a bar from zero.
        let g0 = r[0] + (PAD + 3.0 * LETTER_PITCH + LETTER + GAP) * px;
        let g1 = g0 + GAUGE * px;
        let gm = (g0 + g1) / 2.0;
        scene.segment(
            [g0, yc],
            [g1, yc],
            p(0.75),
            rgba(1.0, 1.0, 1.0, 0.22),
            rgba(1.0, 1.0, 1.0, 0.22),
        );
        scene.segment(
            [gm, yc - 5.0 * px],
            [gm, yc + 5.0 * px],
            p(0.6),
            rgba(1.0, 1.0, 1.0, 0.6),
            rgba(1.0, 1.0, 1.0, 0.6),
        );
        // End ticks: reverse left, forward right.
        for e in [g0, g1] {
            scene.segment(
                [e, yc - 2.5 * px],
                [e, yc + 2.5 * px],
                p(0.5),
                rgba(1.0, 1.0, 1.0, 0.3),
                rgba(1.0, 1.0, 1.0, 0.3),
            );
        }
        let frac = gauge(c.rate);
        let end = gm + c.rate.signum() * frac * (GAUGE / 2.0) * px;
        if frac > 0.0 {
            scene.segment(
                [gm, yc],
                [end, yc],
                p(1.4),
                rgba(1.0, 1.0, 1.0, 0.92),
                rgba(1.0, 1.0, 1.0, 0.92),
            );
        }
        // The set speed as a small notch (where the rate is heading).
        if c.set != 0.0 {
            let s = gm + c.set.signum() * gauge(c.set) * (GAUGE / 2.0) * px;
            scene.segment([s, yc - 3.5 * px], [s, yc + 3.5 * px], p(0.7), lit, lit);
        }
        // The value.
        let text = value_text(c);
        let tx = g1 + GAP * px;
        let vcap = crate::font::cap_height() * VALUE as f32 * px as f32;
        if !text.is_empty() {
            scene.label([tx as f32, (yc - vcap as f64 / 2.0) as f32], p(VALUE), &text, lit);
        }
        // Lane assist: two lane lines converging upwards; lit when on (in cruise brighter),
        // glowing for a moment when it nudges.
        let lx = tx + (VALUE_SLOT + GAP + LANE / 2.0) * px;
        let alpha = match (c.assist, c.level == Level::Cruise) {
            (false, _) => 0.26,
            (true, false) => 0.6,
            (true, true) => 0.92,
        };
        if c.nudging {
            scene.disc([lx, yc], p(9.0), rgba(1.0, 1.0, 1.0, 0.18));
        }
        let col = rgba(1.0, 1.0, 1.0, if c.nudging { 1.0 } else { alpha });
        let (hh, wb, wt) = (6.0 * px, 5.5 * px, 2.5 * px);
        scene.segment([lx - wb, yc + hh], [lx - wt, yc - hh], p(0.7), col, col);
        scene.segment([lx + wb, yc + hh], [lx + wt, yc - hh], p(0.7), col, col);
        // Centre dashes.
        for (a, b) in [(4.0, 1.5), (-1.0, -3.5)] {
            scene.segment([lx, yc + a * px], [lx, yc + b * px], p(0.5), col, col);
        }
        // Quantised so the scene is rebuilt only when what shows changes.
        key.push_str(&format!(
            "cockpit {:.0} {:.0} {} {:.3} {} {} {} {} ",
            r[0],
            r[1],
            c.level.letter(),
            (frac * c.rate.signum() * 200.0).round() / 200.0,
            text,
            c.assist,
            c.nudging,
            c.level == Level::Cruise
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_a_phone_and_keeps_clear_of_buttons() {
        let c = CockpitOverlay {
            enabled: true,
            inset_right: 56.0,
            inset_bottom: 0.0,
        };
        assert!(width() < 300.0, "{}", width());
        // iPhone portrait (375 points, 3 px per point): inside the canvas, left of the buttons.
        let r = c.layout((1125, 2436), 1.0 / 3.0, (0.0, 0.0, 0.0, 0.0), 0.0);
        assert!(r[0] >= 0.0 && r[2] <= (375.0 - 56.0) * 3.0, "{r:?}");
        // A wide window: centred.
        let c = CockpitOverlay {
            enabled: true,
            ..Default::default()
        };
        let r = c.layout((2000, 1200), 0.5, (0.0, 0.0, 0.0, 0.0), 0.0);
        assert!(((r[0] + r[2]) / 2.0 - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn draws_level_gauge_and_value_and_nothing_when_off() {
        let mut c = CockpitOverlay::default();
        let mut s = Scene::default();
        let mut key = String::new();
        let state = Cockpit {
            level: Level::Cruise,
            set: -cruise::rung(1),
            rate: -cruise::rung(1),
            boost: 1.0,
            landing: false,
            assist: true,
            nudging: false,
        };
        c.draw(&mut s, (2000, 1200), 0.5, (0.0, 0.0, 0.0, 0.0), 0.0, &state, &mut key);
        assert!(s.prims.is_empty() && key.is_empty());
        c.enabled = true;
        c.draw(&mut s, (2000, 1200), 0.5, (0.0, 0.0, 0.0, 0.0), 0.0, &state, &mut key);
        assert!(s.prims.len() > 15, "{}", s.prims.len());
        assert!(key.contains("↓ ×2/s") && key.contains(" C "), "{key}");
        assert_eq!(value_text(&Cockpit::default()), "");
        // Every glyph the cockpit writes is in the font.
        for t in ["M", "C", "A", "J", "↑ ×1.5/s", "↓ ×1e434/s"] {
            for ch in t.chars().filter(|c| !c.is_whitespace()) {
                assert!(crate::font::glyph(ch) != crate::font::glyph('?') || ch == '?', "{ch}");
            }
        }
        assert!(gauge(0.0) == 0.0 && gauge(cruise::max_speed()) == 1.0 && gauge(1e6) == 1.0);
    }
}
