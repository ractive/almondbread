//! The HUD cockpit (kb: decisions/controls "Driving levels"): a small strip at the bottom of the
//! canvas, drawn into the overlay scene like the compass. Four icons for the driving levels
//! (manual: a steering wheel, cruise: a speedometer, autopilot: a plane, journey: sparkles; the
//! last two as in the toolbar), the current one lit with its name beside it; a thin gauge with
//! zero in the middle showing the zoom rate flown now (reverse, zooming out, to the left); the
//! rate as text ("↓ ×4.2/s") with a small line under it (lit for a moment after Q / E): in
//! cruise the gear ("gear 5", "reverse 3", plus "boost" while the reverse depth boost adds to
//! it), in the autopilot and a journey the set speed ("target ×20") only while the rate flown
//! differs from it; a small lane icon for lane assist; and a (?) badge explaining the levels
//! and their keys.
//! Monochrome on a dark translucent plate, so it reads on bright and dark palettes; under 300
//! points wide, so it fits a phone. Shown with the HUD; the state comes from the core
//! `Navigator::cockpit`.

use std::cell::Cell;

use almondbread_core::cruise::{self, Level};
use almondbread_core::input::Cockpit;

use crate::help::{HelpBadge, HelpTopic};
use crate::overlay::{Scene, rgba};

/// Cockpit layout and visibility of one view.
#[derive(Clone, Debug, Default)]
pub struct CockpitOverlay {
    /// Shown (with the HUD).
    pub enabled: bool,
    /// Extra right and bottom margins (view points): iPhone buttons at the bottom right.
    pub inset_right: f64,
    pub inset_bottom: f64,
    /// The set speed last drawn and when it changed (the gear / target line lights up after
    /// Q / E).
    last_set: Cell<f64>,
    set_at: Cell<f64>,
    /// The "target" line showed in the last frame (its hysteresis).
    target_shown: Cell<bool>,
    /// The level icons as last drawn: (level, centre x, centre y, half size), pixels. A click
    /// on one switches to that level (they replace the toolbar's autopilot and journey).
    icons: std::cell::RefCell<Vec<(Level, f64, f64, f64)>>,
    /// The lane-assist icon as last drawn (centre x, y, half size; pixels): a click toggles it.
    lane: Cell<Option<(f64, f64, f64)>>,
}

/// Font sizes (points): the level's name and the rate; the gear / target line.
const NAME: f64 = 11.0;
const VALUE: f64 = 11.0;
const SMALL: f64 = 8.0;
/// Horizontal metrics (points).
const PAD: f64 = 7.0;
const ICON_PITCH: f64 = 19.0;
const NAME_SLOT: f64 = 49.0;
const GAUGE: f64 = 44.0;
const VALUE_SLOT: f64 = 72.0;
const LANE: f64 = 12.0;
const GAP: f64 = 5.0;
const BADGE: f64 = 2.0 * crate::help::RADIUS;
const HEIGHT: f64 = 28.0;
const MARGIN: f64 = 12.0;
/// The cockpit is drawn this much larger where the window has room (at its base size on a
/// phone).
const ZOOM_MAX: f64 = 1.3;
/// How long the gear / target line stays lit after it changed (s).
const SET_FLASH: f64 = 1.2;

/// The panel's width in points.
pub fn width() -> f64 {
    2.0 * PAD + 4.0 * ICON_PITCH + NAME_SLOT + GAUGE + VALUE_SLOT + LANE + BADGE + 5.0 * GAP
}

/// Gauge deflection 0…1 of a zoom rate (e-folds/s): logarithmic, the top rung at the end.
pub fn gauge(rate: f64) -> f64 {
    let m = rate.abs();
    if !m.is_finite() {
        return 1.0;
    }
    ((m / 0.2).ln_1p() / (cruise::max_speed() / 0.2).ln_1p()).clamp(0.0, 1.0)
}

/// The rate flown now as text ("↓ ×4.2/s"); empty at rest.
pub fn value_text(c: &Cockpit) -> String {
    if c.rate.abs() < 0.02 {
        return String::new();
    }
    let arrow = if c.rate < 0.0 { '↓' } else { '↑' };
    format!("{arrow} {}/s", cruise::factor_text(c.rate))
}

/// The autopilot's and journey's set speed shows ("target ×20") while the rate flown differs
/// from it by more than this share (of the e-folds per second), and hides again under
/// [`TARGET_HIDE`] (no flicker at the edge).
pub const TARGET_SHOW: f64 = 0.05;
pub const TARGET_HIDE: f64 = 0.03;

/// The small line under the rate; empty when nothing is set. In cruise the gear, always:
/// "gear 5" forward, "reverse 3" zooming out, plus " · boost" while the depth boost adds
/// noticeably. In the autopilot and a journey the set speed, "target ×20", only while the rate
/// flown differs from it (speeding up, braking, the renderer back-off; `shown`: it showed in
/// the last frame, for the hysteresis).
pub fn set_text(c: &Cockpit, shown: bool) -> String {
    if c.set == 0.0 || !c.set.is_finite() {
        return String::new();
    }
    match c.level {
        Level::Cruise => {
            let Some(g) = cruise::gear(c.set) else {
                return String::new();
            };
            if c.set > 0.0 {
                format!("gear {g}")
            } else {
                let boost = if c.boost > 1.05 { " · boost" } else { "" };
                format!("reverse {g}{boost}")
            }
        }
        Level::Autopilot | Level::Journey => {
            let tol = if shown { TARGET_HIDE } else { TARGET_SHOW };
            if (c.rate - c.set).abs() > tol * c.set.abs() {
                format!("target {}", cruise::factor_text(c.set))
            } else {
                String::new()
            }
        }
        Level::Manual => String::new(),
    }
}

/// The level's name in the cockpit.
pub fn level_name(l: Level) -> &'static str {
    match l {
        Level::Manual => "Manual",
        Level::Cruise => "Cruise",
        Level::Autopilot => "Autopilot",
        Level::Journey => "Journey",
    }
}

/// Draws the icon of level `l` centred at `c` (pixels; `u` pixels per point) in `col`.
fn icon(scene: &mut Scene, l: Level, c: [f64; 2], u: f64, col: [f32; 4]) {
    let q = |x: f64, y: f64| [c[0] + x * u, c[1] + y * u];
    let w = (0.55 * u) as f32;
    let line = |scene: &mut Scene, pts: &[(f64, f64)]| {
        for s in pts.windows(2) {
            scene.segment(q(s[0].0, s[0].1), q(s[1].0, s[1].1), w, col, col);
        }
    };
    match l {
        // A steering wheel: rim, hub and three spokes.
        Level::Manual => {
            scene.ring(c, (4.6 * u) as f32, (1.0 * u) as f32, col);
            scene.disc(c, (1.3 * u) as f32, col);
            line(scene, &[(-4.3, -0.3), (0.0, 0.0), (4.3, -0.3)]);
            line(scene, &[(0.0, 0.0), (0.0, 4.3)]);
        }
        // A speedometer: an open arc and a needle.
        Level::Cruise => {
            let arc: Vec<(f64, f64)> = (0..=12)
                .map(|k| {
                    let a = (200.0 - 220.0 * f64::from(k) / 12.0).to_radians();
                    (4.8 * a.cos(), 1.2 - 4.8 * a.sin())
                })
                .collect();
            line(scene, &arc);
            line(scene, &[(0.0, 1.2), (2.6, -1.8)]);
            scene.disc(q(0.0, 1.2), (0.9 * u) as f32, col);
        }
        // A plane, nose up: fuselage, swept wings, tail.
        Level::Autopilot => {
            line(scene, &[(0.0, -5.0), (0.0, 4.6)]);
            line(scene, &[(-5.0, 1.4), (0.0, -1.2), (5.0, 1.4)]);
            line(scene, &[(-2.0, 4.8), (0.0, 3.6), (2.0, 4.8)]);
        }
        // Sparkles: a four-pointed star and a small one.
        Level::Journey => {
            line(scene, &[(-1.5, -5.0), (-1.5, 4.0)]);
            line(scene, &[(-5.6, -0.5), (2.6, -0.5)]);
            line(scene, &[(3.6, -5.0), (3.6, -2.0)]);
            line(scene, &[(2.1, -3.5), (5.1, -3.5)]);
        }
    }
}

impl CockpitOverlay {
    /// The control under (x, y) pixels, if any (the last drawn cockpit): a level icon
    /// ("manual", "cruise", "autopilot", "journey") or the lane-assist icon ("lane").
    pub fn hit(&self, x: f64, y: f64) -> Option<&'static str> {
        let level = self
            .icons
            .borrow()
            .iter()
            .find(|(_, cx, cy, h)| (x - cx).abs() <= *h && (y - cy).abs() <= *h)
            .map(|(l, ..)| *l);
        if let Some(l) = level {
            return Some(match l {
                Level::Manual => "manual",
                Level::Cruise => "cruise",
                Level::Autopilot => "autopilot",
                Level::Journey => "journey",
            });
        }
        self.lane
            .get()
            .filter(|(cx, cy, h)| (x - cx).abs() <= *h && (y - cy).abs() <= *h)
            .map(|_| "lane")
    }

    /// The panel in pixels (x0, y0, x1, y1) for a `size` pixel target with `ppp` points per
    /// pixel and canvas `insets` (top, left, bottom, right; view points), centred at the bottom
    /// between `left` (view points from the left edge, e.g. the cosmic scale) and the right edge.
    pub fn layout(&self, size: (usize, usize), ppp: f64, insets: (f64, f64, f64, f64), left: f64) -> [f64; 4] {
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64 * ppp, size.1 as f64 * ppp);
        let k = self.zoom(size, ppp, insets, left);
        let pw = width() * k;
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
        [x0 * px, (y1 - HEIGHT * k) * px, (x0 + pw) * px, y1 * px]
    }

    /// The drawing scale: up to [`ZOOM_MAX`] while the panel still fits between `left` and
    /// the right edge, never below 1.
    pub fn zoom(&self, size: (usize, usize), ppp: f64, insets: (f64, f64, f64, f64), left: f64) -> f64 {
        let w = size.0 as f64 * ppp;
        let room = w - insets.3 - self.inset_right - MARGIN - (insets.1.max(left) + MARGIN);
        (room / width()).clamp(1.0, ZOOM_MAX)
    }

    /// Draws the cockpit and its (?) badge (into `help`); appends what it drew to `key`.
    /// Nothing while disabled.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        scene: &mut Scene,
        help: &mut Vec<HelpBadge>,
        size: (usize, usize),
        ppp: f64,
        insets: (f64, f64, f64, f64),
        left: f64,
        c: &Cockpit,
        now: f64,
        key: &mut String,
    ) {
        if !self.enabled || ppp <= 0.0 || size.0 == 0 || size.1 == 0 {
            self.icons.borrow_mut().clear();
            self.lane.set(None);
            return;
        }
        // Points of the cockpit's own design, scaled (`zoom`), in pixels.
        let px = self.zoom(size, ppp, insets, left) / ppp;
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
        let cap = |size: f64| f64::from(crate::font::cap_height()) * size * px;
        // Level icons, the current one lit; its name after them.
        let mut x = r[0] + PAD * px;
        let mut icons = self.icons.borrow_mut();
        icons.clear();
        for l in Level::ALL {
            let on = l == c.level;
            let cx = x + ICON_PITCH * px / 2.0;
            icon(scene, l, [cx, yc], px, if on { lit } else { dim });
            icons.push((l, cx, yc, ICON_PITCH * px / 2.0));
            x += ICON_PITCH * px;
        }
        drop(icons);
        x += GAP * px;
        scene.label(
            [x as f32, (yc - cap(NAME) / 2.0) as f32],
            p(NAME),
            level_name(c.level),
            lit,
        );
        // Gauge: a track with the zero tick in the middle, the rate flown as a bar from zero.
        let g0 = x + (NAME_SLOT + GAP) * px;
        let g1 = g0 + GAUGE * px;
        let gm = (g0 + g1) / 2.0;
        let faint = rgba(1.0, 1.0, 1.0, 0.22);
        scene.segment([g0, yc], [g1, yc], p(0.75), faint, faint);
        let tick = rgba(1.0, 1.0, 1.0, 0.6);
        scene.segment([gm, yc - 5.0 * px], [gm, yc + 5.0 * px], p(0.6), tick, tick);
        let end_tick = rgba(1.0, 1.0, 1.0, 0.3);
        for e in [g0, g1] {
            scene.segment([e, yc - 2.5 * px], [e, yc + 2.5 * px], p(0.5), end_tick, end_tick);
        }
        let frac = gauge(c.rate);
        if frac > 0.0 {
            let end = gm + c.rate.signum() * frac * (GAUGE / 2.0) * px;
            let bar = rgba(1.0, 1.0, 1.0, 0.92);
            scene.segment([gm, yc], [end, yc], p(1.4), bar, bar);
        }
        // The rate flown, and the set speed under it (lit for a moment after it changed).
        let tx = g1 + GAP * px;
        let text = value_text(c);
        let set = set_text(c, self.target_shown.get());
        self.target_shown.set(set.starts_with("target"));
        if c.set != self.last_set.get() {
            self.last_set.set(c.set);
            self.set_at.set(now);
        }
        let fresh = c.set != 0.0 && now - self.set_at.get() < SET_FLASH;
        let two = !set.is_empty();
        let vy = if two {
            yc - 1.5 * px - cap(VALUE)
        } else {
            yc - cap(VALUE) / 2.0
        };
        if !text.is_empty() {
            scene.label([tx as f32, vy as f32], p(VALUE), &text, lit);
        }
        if two {
            let col = if fresh { lit } else { rgba(1.0, 1.0, 1.0, 0.55) };
            scene.label([tx as f32, (yc + 2.5 * px) as f32], p(SMALL), &set, col);
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
        self.lane.set(Some((lx, yc, (LANE / 2.0 + GAP / 2.0) * px)));
        let col = rgba(1.0, 1.0, 1.0, if c.nudging { 1.0 } else { alpha });
        let (hh, wb, wt) = (6.0 * px, 5.5 * px, 2.5 * px);
        scene.segment([lx - wb, yc + hh], [lx - wt, yc - hh], p(0.7), col, col);
        scene.segment([lx + wb, yc + hh], [lx + wt, yc - hh], p(0.7), col, col);
        for (a, b) in [(4.0, 1.5), (-1.0, -3.5)] {
            scene.segment([lx, yc + a * px], [lx, yc + b * px], p(0.5), col, col);
        }
        // The (?) badge: the levels and their keys.
        let bx = lx + (LANE / 2.0 + GAP + BADGE / 2.0) * px;
        crate::help::draw(scene, help, HelpTopic::Cockpit, [bx as f32, yc as f32], px as f32, key);
        // Quantised so the scene is rebuilt only when what shows changes.
        key.push_str(&format!(
            "cockpit {:.0} {:.0} {} {:.3} {} {} {} {} {} {} ",
            r[0],
            r[1],
            c.level.letter(),
            (frac * c.rate.signum() * 200.0).round() / 200.0,
            text,
            set,
            fresh,
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
            ..Default::default()
        };
        assert!(width() < 310.0, "{}", width());
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
    fn draws_icons_name_rate_set_and_badge_and_nothing_when_off() {
        let mut c = CockpitOverlay::default();
        let mut s = Scene::default();
        let mut help = Vec::new();
        let mut key = String::new();
        let state = Cockpit {
            level: Level::Cruise,
            set: -cruise::rung(1),
            rate: -2.0 * cruise::rung(1),
            boost: 2.0,
            landing: false,
            assist: true,
            nudging: false,
        };
        let no_insets = (0.0, 0.0, 0.0, 0.0);
        c.draw(
            &mut s,
            &mut help,
            (2000, 1200),
            0.5,
            no_insets,
            0.0,
            &state,
            0.0,
            &mut key,
        );
        assert!(s.prims.is_empty() && key.is_empty() && help.is_empty());
        c.enabled = true;
        c.draw(
            &mut s,
            &mut help,
            (2000, 1200),
            0.5,
            no_insets,
            0.0,
            &state,
            0.0,
            &mut key,
        );
        assert!(s.prims.len() > 20, "{}", s.prims.len());
        assert!(
            key.contains("↓ ×1.44/s") && key.contains("reverse 2 · boost") && key.contains(" true "),
            "{key}"
        );
        assert_eq!(help.len(), 1);
        assert_eq!(help[0].topic, HelpTopic::Cockpit);
        // The set line stops being lit after the flash.
        let mut later = String::new();
        c.draw(
            &mut s,
            &mut help,
            (2000, 1200),
            0.5,
            no_insets,
            0.0,
            &state,
            5.0,
            &mut later,
        );
        assert!(later.contains(" false "), "{later}");
        assert_eq!(value_text(&Cockpit::default()), "");
        assert_eq!(set_text(&Cockpit::default(), false), "");
        // Every glyph the cockpit writes is in the font, and the longest lines fit their slot.
        let words = [
            "Manual",
            "Cruise",
            "Autopilot",
            "Journey",
            "↑ ×1.5/s",
            "↓ ×1e434/s",
            "reverse 26 · boost",
            "gear 26",
            "target ×1000",
        ];
        for t in words {
            for ch in t.chars().filter(|c| !c.is_whitespace()) {
                assert!(crate::font::glyph(ch) != crate::font::glyph('?') || ch == '?', "{ch}");
            }
        }
        for t in ["reverse 26 · boost", "target ×1000", "target ×2.46"] {
            let w = f64::from(crate::font::layout(t, SMALL as f32).1);
            assert!(w <= VALUE_SLOT, "{t}: {w} points");
        }
        let w = f64::from(crate::font::layout("↓ ×1e869/s", VALUE as f32).1);
        assert!(w <= VALUE_SLOT, "{w} points");
        assert!(gauge(0.0) == 0.0 && gauge(cruise::max_speed()) == 1.0 && gauge(1e6) == 1.0);
    }

    /// The small line: the gear in cruise (always), the target in A and J only while the rate
    /// flown differs from it by more than 5 % (with hysteresis).
    #[test]
    fn small_line_shows_the_gear_or_a_differing_target() {
        let at = |level, set: f64, rate: f64, boost: f64| Cockpit {
            level,
            set,
            rate,
            boost,
            ..Default::default()
        };
        let r = cruise::rung;
        assert_eq!(set_text(&at(Level::Cruise, r(4), 0.0, 1.0), false), "gear 5");
        assert_eq!(set_text(&at(Level::Cruise, r(4), r(4), 1.0), false), "gear 5");
        assert_eq!(set_text(&at(Level::Cruise, -r(2), -r(2), 1.0), false), "reverse 3");
        assert_eq!(set_text(&at(Level::Cruise, -r(2), -r(2), 1.02), false), "reverse 3");
        assert_eq!(
            set_text(&at(Level::Cruise, -r(2), -3.0, 2.0), false),
            "reverse 3 · boost"
        );
        assert_eq!(set_text(&at(Level::Cruise, r(25), r(25), 1.0), false), "gear 26");
        // Braking to rest (nothing set): no line.
        assert_eq!(set_text(&at(Level::Cruise, 0.0, 0.5, 1.0), false), "");
        let s = 20f64.ln();
        for level in [Level::Autopilot, Level::Journey] {
            assert_eq!(set_text(&at(level, s, s, 1.0), false), "", "{level:?} on target");
            assert_eq!(
                set_text(&at(level, s, 0.97 * s, 1.0), false),
                "",
                "{level:?} within 5 %"
            );
            assert_eq!(set_text(&at(level, s, 0.9 * s, 1.0), false), "target ×20", "{level:?}");
            assert_eq!(set_text(&at(level, s, 0.0, 1.0), false), "target ×20", "{level:?}");
            assert_eq!(set_text(&at(level, s, -0.5, 1.0), false), "target ×20", "{level:?}");
            // Hysteresis: shown, it stays until within 3 %.
            assert_eq!(set_text(&at(level, s, 0.96 * s, 1.0), true), "target ×20");
            assert_eq!(set_text(&at(level, s, 0.98 * s, 1.0), true), "");
        }
        assert_eq!(set_text(&at(Level::Manual, s, 0.0, 1.0), false), "");
    }

    #[test]
    fn target_line_follows_the_rate_while_drawing() {
        let c = CockpitOverlay {
            enabled: true,
            ..Default::default()
        };
        let mut s = Scene::default();
        let mut help = Vec::new();
        let set = 20f64.ln();
        let mut draw = |rate: f64, t: f64| {
            let mut key = String::new();
            let state = Cockpit {
                level: Level::Autopilot,
                set,
                rate,
                ..Default::default()
            };
            c.draw(
                &mut s,
                &mut help,
                (2000, 1200),
                0.5,
                (0.0, 0.0, 0.0, 0.0),
                0.0,
                &state,
                t,
                &mut key,
            );
            key
        };
        assert!(draw(0.5 * set, 0.0).contains("target ×20"));
        assert!(draw(0.96 * set, 0.1).contains("target ×20"), "hysteresis");
        assert!(!draw(0.99 * set, 0.2).contains("target"));
        assert!(
            !draw(0.96 * set, 0.3).contains("target"),
            "hidden stays hidden within 5 %"
        );
    }
}
