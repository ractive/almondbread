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
use almondbread_core::icon::{Glyph, Stroke};
use almondbread_core::input::Cockpit;

use crate::help::{HelpBadge, HelpTopic};
use crate::overlay::{Scene, rgba};

/// A level icon's hit cell as drawn: level, centre x, y, half width, half height (pixels).
type IconCell = (Level, f64, f64, f64, f64);

/// A control's hit cell: id, centre x, y, half width, half height (pixels).
pub type ControlCell = (&'static str, f64, f64, f64, f64);

/// Cockpit layout and visibility of one view.
#[derive(Clone, Debug, Default)]
pub struct CockpitOverlay {
    /// Shown (with the HUD).
    pub enabled: bool,
    /// Extra right and bottom margins (view points): iPhone buttons at the bottom right.
    pub inset_right: f64,
    pub inset_bottom: f64,
    /// Touch screens (kb: decisions/controls "Touch"): every control gets a 44-point cell (the
    /// icons drawn larger), in one row where the canvas has room, else the icons and the lane
    /// icon on a row of their own above the name, gauge and rate.
    pub touch: bool,
    /// Extra bottom margin of this frame (view points): above a bottom-left map row (touch).
    pub lift: Cell<f64>,
    /// The set speed last drawn and when it changed (the gear / target line lights up after
    /// Q / E).
    last_set: Cell<f64>,
    set_at: Cell<f64>,
    /// The "target" line showed in the last frame (its hysteresis).
    target_shown: Cell<bool>,
    /// The level icons as last drawn: (level, centre x, centre y, half width, half height),
    /// pixels. A click on one switches to that level (they replace the toolbar's autopilot and
    /// journey).
    icons: std::cell::RefCell<Vec<IconCell>>,
    /// The lane-assist icon as last drawn (centre x, y, half width, half height; pixels): a
    /// click toggles it.
    lane: Cell<Option<(f64, f64, f64, f64)>>,
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
/// Touch: the cell of each control (points; Apple's minimum touch target), the icons' drawing
/// scale, and the height of the second row in the two-row layout.
const TOUCH_CELL: f64 = 44.0;
const TOUCH_ICON: f64 = 1.35;
const TOUCH_ROW2: f64 = 22.0;
/// Touch: the badge's distance from the lane cell, so its fingertip radius
/// (`help::TOUCH_RADIUS`) stays out of the lane icon's cell.
const TOUCH_BADGE_GAP: f64 = 12.0;

/// Where the cockpit's parts sit, in design points from the plate's top-left corner (scaled by
/// the zoom when drawn).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Geometry {
    w: f64,
    h: f64,
    /// Icon cell width, the icons' drawing scale, the first cell's left edge and the row's centre.
    pitch: f64,
    icon_scale: f64,
    icons_x0: f64,
    icons_y: f64,
    /// Half the height of an icon's (and the lane icon's) hit cell.
    hit_half_h: f64,
    /// The name's left edge, the gauge's left end and the rate's left edge, on row `row_y`.
    name_x: f64,
    gauge_x0: f64,
    value_x: f64,
    row_y: f64,
    /// Lane icon centre and its cell's half width; the (?) badge's centre.
    lane: (f64, f64),
    lane_half: f64,
    badge: (f64, f64),
}

impl Geometry {
    /// The desktop layout (one row, 28 points high): what the Mac draws.
    fn desktop() -> Self {
        let y = HEIGHT / 2.0;
        let name_x = PAD + 4.0 * ICON_PITCH + GAP;
        let gauge_x0 = name_x + NAME_SLOT + GAP;
        let value_x = gauge_x0 + GAUGE + GAP;
        let lane_x = value_x + VALUE_SLOT + GAP + LANE / 2.0;
        let badge_x = lane_x + LANE / 2.0 + GAP + BADGE / 2.0;
        Self {
            w: width(),
            h: HEIGHT,
            pitch: ICON_PITCH,
            icon_scale: 1.0,
            icons_x0: PAD,
            icons_y: y,
            hit_half_h: ICON_PITCH / 2.0,
            name_x,
            gauge_x0,
            value_x,
            row_y: y,
            lane: (lane_x, y),
            lane_half: LANE / 2.0 + GAP / 2.0,
            badge: (badge_x, y),
        }
    }

    /// The touch layout in one row (`two_rows` false, 44 points high) or two (the icon row
    /// on top).
    fn touch(two_rows: bool) -> Self {
        let c = TOUCH_CELL;
        let y = c / 2.0;
        if two_rows {
            let lane_x = PAD + 4.0 * c + c / 2.0;
            let badge_x = lane_x + c / 2.0 + TOUCH_BADGE_GAP + BADGE / 2.0;
            let name_x = PAD + 4.0;
            let gauge_x0 = name_x + NAME_SLOT + GAP;
            Self {
                w: badge_x + BADGE / 2.0 + PAD,
                h: c + TOUCH_ROW2,
                pitch: c,
                icon_scale: TOUCH_ICON,
                icons_x0: PAD,
                icons_y: y,
                hit_half_h: c / 2.0,
                name_x,
                gauge_x0,
                value_x: gauge_x0 + GAUGE + GAP,
                row_y: c + TOUCH_ROW2 / 2.0 - 3.0,
                lane: (lane_x, y),
                lane_half: c / 2.0,
                badge: (badge_x, y),
            }
        } else {
            let name_x = PAD + 4.0 * c + GAP;
            let gauge_x0 = name_x + NAME_SLOT + GAP;
            let value_x = gauge_x0 + GAUGE + GAP;
            let lane_x = value_x + VALUE_SLOT + GAP + c / 2.0;
            let badge_x = lane_x + c / 2.0 + TOUCH_BADGE_GAP + BADGE / 2.0;
            Self {
                w: badge_x + BADGE / 2.0 + PAD,
                h: c,
                pitch: c,
                icon_scale: TOUCH_ICON,
                icons_x0: PAD,
                icons_y: y,
                hit_half_h: c / 2.0,
                name_x,
                gauge_x0,
                value_x,
                row_y: y,
                lane: (lane_x, y),
                lane_half: c / 2.0,
                badge: (badge_x, y),
            }
        }
    }
}

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

/// Draws glyph `g` (core `icon::Glyph`, the shapes the UIs also show inline in tour texts)
/// centred at `c` (pixels; `u` pixels per point) in `col`.
fn glyph(scene: &mut Scene, g: Glyph, c: [f64; 2], u: f64, col: [f32; 4]) {
    let q = |p: [f64; 2]| [c[0] + p[0] * u, c[1] + p[1] * u];
    for s in g.strokes() {
        match s {
            Stroke::Line { points, half_width } => {
                let w = (half_width * u) as f32;
                for p in points.windows(2) {
                    scene.segment(q(p[0]), q(p[1]), w, col, col);
                }
            }
            Stroke::Ring { center, radius, width } => {
                scene.ring(q(center), (radius * u) as f32, (width * u) as f32, col);
            }
            Stroke::Disc { center, radius } => scene.disc(q(center), (radius * u) as f32, col),
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
            .find(|(_, cx, cy, hw, hh)| (x - cx).abs() <= *hw && (y - cy).abs() <= *hh)
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
            .filter(|(cx, cy, hw, hh)| (x - cx).abs() <= *hw && (y - cy).abs() <= *hh)
            .map(|_| "lane")
    }

    /// Every control as last drawn: id ("manual", …, "lane"), centre and half sizes of its hit
    /// cell, pixels (UI tests aim at them).
    pub fn controls(&self) -> Vec<ControlCell> {
        let mut v: Vec<_> = self
            .icons
            .borrow()
            .iter()
            .map(|&(l, x, y, hw, hh)| {
                let id = match l {
                    Level::Manual => "manual",
                    Level::Cruise => "cruise",
                    Level::Autopilot => "autopilot",
                    Level::Journey => "journey",
                };
                (id, x, y, hw, hh)
            })
            .collect();
        if let Some((x, y, hw, hh)) = self.lane.get() {
            v.push(("lane", x, y, hw, hh));
        }
        v
    }

    /// The geometry for a canvas with `room` points free across (touch: one row if it fits).
    fn geometry(&self, room: f64) -> Geometry {
        if !self.touch {
            return Geometry::desktop();
        }
        let one = Geometry::touch(false);
        if room >= one.w { one } else { Geometry::touch(true) }
    }

    /// The cockpit fits between `left` (view points) and the right edge.
    pub fn fits(&self, size: (usize, usize), ppp: f64, insets: (f64, f64, f64, f64), left: f64) -> bool {
        let room = self.room(size, ppp, insets, left);
        room >= self.geometry(room).w
    }

    /// Points free across the canvas for the cockpit.
    fn room(&self, size: (usize, usize), ppp: f64, insets: (f64, f64, f64, f64), left: f64) -> f64 {
        let w = size.0 as f64 * ppp;
        w - insets.3 - self.inset_right - MARGIN - (insets.1.max(left) + MARGIN)
    }

    /// The panel in pixels (x0, y0, x1, y1) for a `size` pixel target with `ppp` points per
    /// pixel and canvas `insets` (top, left, bottom, right; view points), centred at the bottom
    /// between `left` (view points from the left edge, e.g. the cosmic scale) and the right edge.
    pub fn layout(&self, size: (usize, usize), ppp: f64, insets: (f64, f64, f64, f64), left: f64) -> [f64; 4] {
        self.layout_lifted(size, ppp, insets, left, self.lift.get())
    }

    /// [`Self::layout`] with `lift` points of extra bottom margin instead of this frame's
    /// [`Self::lift`] (the map row asks where a lifted cockpit would go).
    pub fn layout_lifted(
        &self,
        size: (usize, usize),
        ppp: f64,
        insets: (f64, f64, f64, f64),
        left: f64,
        lift: f64,
    ) -> [f64; 4] {
        let px = 1.0 / ppp;
        let (w, h) = (size.0 as f64 * ppp, size.1 as f64 * ppp);
        let k = self.zoom(size, ppp, insets, left);
        let g = self.geometry(self.room(size, ppp, insets, left));
        let pw = g.w * k;
        let lo = insets.1.max(left) + MARGIN;
        let hi = w - insets.3 - self.inset_right - MARGIN;
        let mut x0 = (w - pw) / 2.0;
        if x0 < lo {
            x0 = lo;
        }
        if x0 + pw > hi {
            x0 = (hi - pw).max(insets.1 + MARGIN.min(4.0));
        }
        let y1 = h - insets.2 - self.inset_bottom - lift - MARGIN;
        [x0 * px, (y1 - g.h * k) * px, (x0 + pw) * px, y1 * px]
    }

    /// The drawing scale: up to [`ZOOM_MAX`] while the panel still fits between `left` and
    /// the right edge, never below 1 (touch: always 1, its cells are touch-sized already).
    pub fn zoom(&self, size: (usize, usize), ppp: f64, insets: (f64, f64, f64, f64), left: f64) -> f64 {
        if self.touch {
            return 1.0;
        }
        (self.room(size, ppp, insets, left) / width()).clamp(1.0, ZOOM_MAX)
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
        let g = self.geometry(self.room(size, ppp, insets, left));
        // A design point of the panel in pixels.
        let at = |x: f64, y: f64| (r[0] + x * px, r[1] + y * px);
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
        let lit = rgba(1.0, 1.0, 1.0, 0.96);
        let dim = rgba(1.0, 1.0, 1.0, 0.34);
        let cap = |size: f64| f64::from(crate::font::cap_height()) * size * px;
        // Level icons, the current one lit; its name after them (touch, two rows: under them).
        let mut icons = self.icons.borrow_mut();
        icons.clear();
        for (i, l) in Level::ALL.into_iter().enumerate() {
            let on = l == c.level;
            let (cx, cy) = at(g.icons_x0 + (i as f64 + 0.5) * g.pitch, g.icons_y);
            glyph(
                scene,
                Glyph::of(l),
                [cx, cy],
                px * g.icon_scale,
                if on { lit } else { dim },
            );
            icons.push((l, cx, cy, g.pitch * px / 2.0, g.hit_half_h * px));
        }
        drop(icons);
        let (x, yc) = at(g.name_x, g.row_y);
        scene.label(
            [x as f32, (yc - cap(NAME) / 2.0) as f32],
            p(NAME),
            level_name(c.level),
            lit,
        );
        // Gauge: a track with the zero tick in the middle, the rate flown as a bar from zero.
        let g0 = at(g.gauge_x0, 0.0).0;
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
        let tx = at(g.value_x, 0.0).0;
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
        let (lx, ly) = at(g.lane.0, g.lane.1);
        let alpha = match (c.assist, c.level == Level::Cruise) {
            (false, _) => 0.26,
            (true, false) => 0.6,
            (true, true) => 0.92,
        };
        let u = px * g.icon_scale;
        if c.nudging {
            scene.disc([lx, ly], (9.0 * u) as f32, rgba(1.0, 1.0, 1.0, 0.18));
        }
        self.lane.set(Some((lx, ly, g.lane_half * px, g.hit_half_h * px)));
        let col = rgba(1.0, 1.0, 1.0, if c.nudging { 1.0 } else { alpha });
        glyph(scene, Glyph::Lane, [lx, ly], u, col);
        // The (?) badge: the levels and their keys.
        let (bx, by) = at(g.badge.0, g.badge.1);
        crate::help::draw(scene, help, HelpTopic::Cockpit, [bx as f32, by as f32], px as f32, key);
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

    /// Touch (kb: decisions/controls "Touch"): every control has a 44-point cell; a phone in
    /// portrait gets two rows left of the button column, a wide canvas one row of 44 points.
    #[test]
    fn touch_cells_are_44_points_in_one_or_two_rows() {
        let state = Cockpit::default();
        let mut key = String::new();
        let mut help = Vec::new();
        for (size, ppp, inset_right, rows) in [
            ((1170, 2532), 1.0 / 3.0, 56.0, 2), // iPhone portrait, 390 points
            ((2532, 1170), 1.0 / 3.0, 0.0, 1),  // iPhone landscape
            ((1488, 2266), 0.5, 0.0, 1),        // iPad mini portrait
        ] {
            let c = CockpitOverlay {
                enabled: true,
                inset_right,
                touch: true,
                ..Default::default()
            };
            let mut s = Scene::default();
            help.clear();
            c.draw(
                &mut s,
                &mut help,
                size,
                ppp,
                (0.0, 0.0, 0.0, 0.0),
                0.0,
                &state,
                0.0,
                &mut key,
            );
            let r = c.layout(size, ppp, (0.0, 0.0, 0.0, 0.0), 0.0);
            let pt = 1.0 / ppp;
            let w = size.0 as f64 * ppp;
            assert!(r[0] >= 0.0 && r[2] <= (w - inset_right) * pt, "{size:?}: {r:?}");
            let h = (r[3] - r[1]) * ppp;
            assert_eq!(
                h,
                if rows == 2 { TOUCH_CELL + TOUCH_ROW2 } else { TOUCH_CELL },
                "{size:?}"
            );
            let icons = c.icons.borrow().clone();
            assert_eq!(icons.len(), 4);
            for (l, cx, cy, hw, hh) in icons {
                assert!(hw * ppp >= 22.0 - 1e-9 && hh * ppp >= 22.0 - 1e-9, "{l:?} {hw} {hh}");
                // Anywhere in the 44-point cell, not only on the drawn icon.
                for (dx, dy) in [(0.0, 0.0), (20.0, 20.0), (-20.0, -20.0)] {
                    let want = match l {
                        Level::Manual => "manual",
                        Level::Cruise => "cruise",
                        Level::Autopilot => "autopilot",
                        Level::Journey => "journey",
                    };
                    assert_eq!(c.hit(cx + dx * pt, cy + dy * pt), Some(want), "{size:?} {l:?}");
                }
            }
            let (lx, ly, lw, lh) = c.lane.get().expect("lane");
            assert!(lw * ppp >= 22.0 - 1e-9 && lh * ppp >= 22.0 - 1e-9);
            assert_eq!(c.hit(lx + 20.0 * pt, ly), Some("lane"));
            // The (?) badge takes a fingertip around it, but not inside the lane icon's cell.
            let b = help[0].centre;
            let (bx, by) = (f64::from(b[0]), f64::from(b[1]));
            assert!(crate::help::hit_touch(&help, bx - 17.0 * pt, by, pt).is_some());
            assert!(crate::help::hit_touch(&help, lx + 21.0 * pt, ly, pt).is_none());
        }
        // The desktop layout is unchanged: 28 points high, 306 wide (at zoom 1).
        let c = CockpitOverlay {
            enabled: true,
            ..Default::default()
        };
        let r = c.layout((320, 800), 1.0, (0.0, 0.0, 0.0, 0.0), 0.0);
        assert!(
            (r[3] - r[1] - HEIGHT).abs() < 1e-9 && (r[2] - r[0] - width()).abs() < 1e-9,
            "{r:?}"
        );
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
