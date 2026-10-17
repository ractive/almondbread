//! The cosmic size scale panel (kb: decisions/controls, "Cosmic size scale"): a log ruler of
//! real things (`almondbread_core::cosmic`) sliding past a fixed marker as the view zooms,
//! a sentence and its explanation. Drawn into the overlay scene like the mini-map; costs a scene rebuild per frame while the zoom changes
//! (no GPU work of its own) and nothing when off.

use almondbread_core::cosmic::{CosmicScale, CosmicScaleSettings, Group, SET_WIDTH};
use almondbread_core::viewport::Viewport;

use crate::overlay::{Scene, rgba};

/// How long a "passing" highlight lasts (seconds).
const FLASH: f64 = 1.6;
/// Ruler pixels per decade (view points) in the physical part.
const POINTS_PER_DECADE: f64 = 15.0;

/// Panel state of one view.
#[derive(Default)]
pub struct CosmicOverlay {
    pub settings: CosmicScaleSettings,
    scale: Option<((u64, u64), CosmicScale)>,
    /// Current depth (log10 of set width over view width).
    d: f64,
    passed: Option<usize>,
    /// `d` holds a previous frame's depth.
    primed: bool,
    /// The crossed mark and when.
    flash: Option<(usize, f64)>,
}

impl CosmicOverlay {
    pub fn enabled(&self) -> bool {
        self.settings.enabled
    }

    /// A highlight is fading: keep frames coming.
    pub fn animating(&self, now: f64) -> bool {
        self.enabled() && self.flash.is_some_and(|(_, t)| now - t < FLASH)
    }

    /// Follows the view: depth, the scale for this window, a mark crossed since last time.
    pub fn update(&mut self, view: &Viewport, size: (usize, usize), ppp: f64, now: f64) {
        if !self.enabled() {
            self.scale = None;
            self.passed = None;
            self.flash = None;
            self.primed = false;
            return;
        }
        let width_pt = size.0 as f64 * ppp;
        let d = SET_WIDTH.log10() - view.scale.log10() - width_pt.max(1.0).log10();
        if !d.is_finite() {
            // Nothing sane to show (kb: techniques/hang-prevention); keep the last scale.
            return;
        }
        let key = (
            self.settings.metres_per_point.to_bits(),
            (width_pt * 4.0).round() as u64,
        );
        if self.scale.as_ref().is_none_or(|(k, _)| *k != key) {
            self.scale = Some((key, CosmicScale::new(&self.settings, width_pt)));
            self.passed = None;
        }
        let scale = &self.scale.as_ref().expect("scale").1;
        let passed = scale.passed(d);
        let prev = self.passed;
        // A teleport (reset, jump, history) is not a crossing.
        if prev != passed && self.primed && (d - self.d).abs() < 3.0 {
            let crossed = if d > self.d { passed } else { prev };
            if let Some(i) = crossed {
                self.flash = Some((i, now));
            }
        }
        self.passed = passed;
        self.d = d;
        self.primed = true;
    }

    /// Draws the panel with its bottom-left corner at `corner` (output pixels); appends what
    /// it drew to `key`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        scene: &mut Scene,
        help: &mut Vec<crate::help::HelpBadge>,
        size: (usize, usize),
        ppp: f64,
        corner: [f32; 2],
        now: f64,
        key: &mut String,
    ) {
        let [left, bottom] = corner;
        let Some((_, scale)) = &self.scale else { return };
        let px = (1.0 / ppp) as f32;
        let w = size.0 as f32;
        let pw = (340.0 * px).min(w - 2.0 * left).max(120.0 * px);
        let pad = 9.0 * px;
        let d = self.d;
        let reading = scale.reading(d);
        let s1 = 12.5 * px;
        let s2 = 10.5 * px;
        let sl = 9.5 * px;
        let cap = crate::font::cap_height();
        // The sentence wraps onto a second line rather than shrinking to a small size.
        let avail = pw - 2.0 * pad;
        let mut lines: Vec<(&str, f32, [f32; 4])> = Vec::with_capacity(3);
        let white = rgba(1.0, 1.0, 1.0, 0.95);
        // Every line wraps at spaces in its own size (the sentence can take three lines).
        let wrap = |text: &'_ str, size: f32, color: [f32; 4], lines: &mut Vec<(String, f32, [f32; 4])>| {
            let mut rest = text.trim();
            for _ in 0..6 {
                if rest.is_empty() {
                    break;
                }
                match wrap_point(rest, size, avail) {
                    Some(i) => {
                        lines.push((rest[..i].to_string(), size, color));
                        rest = rest[i..].trim_start();
                    }
                    None => {
                        lines.push((rest.to_string(), size, color));
                        break;
                    }
                }
            }
        };
        let mut owned: Vec<(String, f32, [f32; 4])> = Vec::with_capacity(4);
        wrap(&reading.line, s1, white, &mut owned);
        wrap(&reading.detail, s2, rgba(0.9, 0.92, 0.95, 0.95), &mut owned);
        lines.extend(owned.iter().map(|(t, s, c)| (t.as_str(), *s, *c)));
        let ruler_h = 2.0 * (sl * cap + 6.0 * px) + 10.0 * px;
        let text_h: f32 = lines.iter().map(|l| l.1 * 1.45).sum();
        let ph = pad + ruler_h + text_h + pad * 0.6;
        let (x0, y1) = (left, bottom);
        let (x1, y0) = (x0 + pw, y1 - ph);
        scene.set_clip(None);
        scene.rect(
            [x0, y0, x1, y1],
            8.0 * px,
            rgba(0.015, 0.015, 0.025, 0.86),
            rgba(1.0, 1.0, 1.0, 0.22),
            px,
        );
        // ---- Ruler.
        let yr = y0 + pad + ruler_h * 0.5;
        // The ruler stops short of the help badge in the top-right corner.
        let (rx0, rx1) = (x0 + pad, x1 - pad - crate::help::reserve(px));
        let xc = x0 + 0.36 * pw;
        let ppd = POINTS_PER_DECADE * f64::from(px);
        let fc = scale.ruler(d);
        let to_x = |m: f64| xc + ((scale.ruler(m) - fc) * ppd) as f32;
        let fade = |x: f32| ((x - rx0).min(rx1 - x) / (22.0 * px)).clamp(0.0, 1.0);
        scene.set_clip(Some([rx0, y0, rx1, y1]));
        scene.segment(
            [f64::from(rx0), f64::from(yr)],
            [f64::from(rx1), f64::from(yr)],
            0.6 * px,
            rgba(1.0, 1.0, 1.0, 0.35),
            rgba(1.0, 1.0, 1.0, 0.35),
        );
        // Decade ticks where they are at least 5 points apart (physical modes).
        // An integer count, not `k += 1.0` up to d + 40: past 2^53 (or at inf) that never
        // ended (kb: techniques/hang-prevention).
        let k0 = (d - 40.0).ceil();
        for i in 0..=80 {
            let k = k0 + f64::from(i);
            if !k.is_finite() || k > d + 40.0 {
                break;
            }
            let x = to_x(k);
            let slope = (to_x(k + 1.0) - x) / px;
            if x > rx0 && x < rx1 && slope >= 5.0 {
                let al = 0.3 * fade(x);
                scene.segment(
                    [f64::from(x), f64::from(yr - 2.0 * px)],
                    [f64::from(x), f64::from(yr + 2.0 * px)],
                    0.4 * px,
                    rgba(1.0, 1.0, 1.0, al),
                    rgba(1.0, 1.0, 1.0, al),
                );
            }
        }
        // Marks: a dot each; labels placed greedily nearest the marker first, above or below.
        let flash = self
            .flash
            .filter(|(_, t)| now - t < FLASH)
            .map(|(i, t)| (i, ((now - t) / FLASH) as f32));
        let mut visible: Vec<(usize, f32)> = scale
            .marks
            .iter()
            .enumerate()
            .map(|(i, m)| (i, to_x(m.d)))
            .filter(|(_, x)| *x > rx0 - 2.0 && *x < rx1 + 2.0)
            .collect();
        for &(i, x) in &visible {
            let m = &scale.marks[i];
            let c = group_color(m.group);
            let al = fade(x);
            scene.disc([f64::from(x), f64::from(yr)], 2.6 * px, rgba(0.0, 0.0, 0.0, 0.6 * al));
            scene.disc([f64::from(x), f64::from(yr)], 1.9 * px, rgba(c[0], c[1], c[2], al));
        }
        visible.sort_by(|p, q| (p.1 - xc).abs().total_cmp(&(q.1 - xc).abs()));
        let mut rows: [Vec<(f32, f32)>; 2] = [
            vec![(xc - 3.0 * px, xc + 3.0 * px)],
            vec![(xc - 3.0 * px, xc + 3.0 * px)],
        ];
        let gap = 9.0 * px;
        for &(i, x) in &visible {
            let m = &scale.marks[i];
            let (_, tw) = crate::font::layout(&m.label, sl);
            // Kept inside the ruler (a mark at the edge labels inwards, but a label never
            // strays far from its dot: it would read as its neighbour's).
            let mut lx0 = (x - 0.5 * tw).clamp(rx0, (rx1 - tw).max(rx0));
            // Nudged off the marker's clearance (within the stray limit below), so a mark
            // close to the marker keeps its label (on a compressed ruler it stays there long).
            let clear = 3.0 * px + gap + px;
            if x > xc && lx0 < xc + clear {
                lx0 = (xc + clear).min((rx1 - tw).max(rx0));
            } else if x < xc && lx0 + tw > xc - clear {
                lx0 = (xc - clear - tw).max(rx0);
            }
            let lx1 = lx0 + tw;
            if x < rx0 || x > rx1 || lx1 > rx1 || (lx0 + 0.5 * tw - x).abs() > 0.3 * tw {
                continue;
            }
            let free = |r: &Vec<(f32, f32)>| r.iter().all(|&(a0, a1)| lx1 + gap < a0 || lx0 > a1 + gap);
            let row = if free(&rows[0]) {
                0
            } else if free(&rows[1]) {
                1
            } else {
                continue;
            };
            rows[row].push((lx0, lx1));
            let ly = if row == 0 {
                yr - 6.0 * px - sl * cap
            } else {
                yr + 6.0 * px
            };
            let flashing = flash.filter(|(f, _)| *f == i).map(|(_, t)| 1.0 - t);
            let col = match flashing {
                Some(t) => {
                    let c = [1.0, 0.85, 0.3];
                    rgba(
                        1.0 + (c[0] - 1.0) * t,
                        1.0 + (c[1] - 1.0) * t,
                        1.0 + (c[2] - 1.0) * t,
                        0.9 * fade(x).max(t),
                    )
                }
                None => rgba(1.0, 1.0, 1.0, 0.78 * fade(x)),
            };
            scene.text_halo(
                [lx0, ly],
                sl,
                &m.label,
                col,
                rgba(0.0, 0.0, 0.0, 0.9 * col[3]),
                2.4 * px,
            );
        }
        // The crossed mark: an expanding ring.
        if let Some((i, t)) = flash
            && let Some(m) = scale.marks.get(i)
        {
            let x = to_x(m.d);
            if x > rx0 && x < rx1 {
                scene.ring(
                    [f64::from(x), f64::from(yr)],
                    (4.0 + 10.0 * t) * px,
                    1.4 * px,
                    rgba(1.0, 0.85, 0.3, 0.9 * (1.0 - t)),
                );
            }
        }
        // The view.
        let gold = rgba(1.0, 0.85, 0.3, 1.0);
        scene.segment(
            [f64::from(xc), f64::from(yr - 9.0 * px)],
            [f64::from(xc), f64::from(yr + 9.0 * px)],
            1.8 * px,
            rgba(0.0, 0.0, 0.0, 0.6),
            rgba(0.0, 0.0, 0.0, 0.6),
        );
        scene.segment(
            [f64::from(xc), f64::from(yr - 8.0 * px)],
            [f64::from(xc), f64::from(yr + 8.0 * px)],
            1.0 * px,
            gold,
            gold,
        );
        scene.disc([f64::from(xc), f64::from(yr)], 3.0 * px, gold);
        // ---- Text, shrunk to fit.
        scene.set_clip(Some([x0, y0, x1, y1]));
        let mut y = y0 + pad + ruler_h + 0.25 * s1;
        for (t, s, c) in &lines {
            let (_, tw) = crate::font::layout(t, *s);
            let s = if tw > pw - 2.0 * pad {
                s * (pw - 2.0 * pad) / tw
            } else {
                *s
            };
            scene.text_halo([x0 + pad, y], s, t, *c, rgba(0.0, 0.0, 0.0, 0.92), 2.6 * px);
            y += s.max(s2) * 1.45;
            key.push_str(t);
        }
        // ---- Help badge (kb: decisions/controls "Help").
        crate::help::draw(
            scene,
            help,
            crate::help::HelpTopic::CosmicScale,
            crate::help::top_right([x0, y0, x1, y1], px),
            px,
            key,
        );
        // ---- "passing ...", above the panel while the highlight lasts.
        scene.set_clip(None);
        if let Some((i, t)) = flash
            && let Some(m) = scale.marks.get(i)
        {
            let s = 11.0 * px;
            let al = (1.0 - t).min(1.0) * if t < 0.1 { t / 0.1 } else { 1.0 };
            scene.label(
                [x0 + 2.0 * px, y0 - 6.0 * px - s * cap],
                s,
                &format!("passing {}", m.label),
                rgba(1.0, 0.85, 0.3, al),
            );
            key.push_str(&format!("flash {i} {:.2} ", t));
        }
        key.push_str(&format!("cosmic {d:.4} {x0} {y1} {pw} "));
    }
}

/// Where to break `text` (at size `s`) so its first part fits `avail`: the last space that
/// fits, None when it fits whole (or has no usable space; it is then shrunk to fit).
fn wrap_point(text: &str, s: f32, avail: f32) -> Option<usize> {
    if crate::font::layout(text, s).1 <= avail {
        return None;
    }
    text.match_indices(' ')
        .map(|(i, _)| i)
        .take_while(|&i| crate::font::layout(&text[..i], s).1 <= avail)
        .last()
}

/// Ruler colours by kind.
fn group_color(g: Group) -> [f32; 3] {
    match g {
        Group::Cosmos => [0.78, 0.6, 1.0],
        Group::Space => [0.62, 0.7, 1.0],
        Group::Star => [1.0, 0.85, 0.4],
        Group::Planet => [0.45, 0.75, 1.0],
        Group::Earthly => [0.6, 0.85, 0.5],
        Group::Life => [0.5, 0.95, 0.6],
        Group::Molecule => [0.4, 0.9, 0.85],
        Group::Particle => [1.0, 0.6, 0.45],
        Group::Limit => [1.0, 0.35, 0.35],
        Group::Number => [0.85, 0.85, 0.85],
        Group::Ladder => [1.0, 0.7, 0.3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_costs_little_and_tracks_the_zoom() {
        let mut o = CosmicOverlay::default();
        o.settings.enabled = true;
        let mut scene = Scene::default();
        let mut key = String::new();
        let t0 = std::time::Instant::now();
        let n = 200;
        let mut flashes = 0;
        for k in 0..n {
            // Zoom in steadily from home to 1e100 (big steps: most frames cross something).
            let z = 100.0 * k as f64 / n as f64;
            let view = Viewport::new(-0.75, 0.1, 3.0 / 1000.0 / 10f64.powf(z));
            o.update(&view, (2000, 1200), 0.5, k as f64 * 0.016);
            scene.prims.clear();
            key.clear();
            o.draw(
                &mut scene,
                &mut Vec::new(),
                (2000, 1200),
                0.5,
                [24.0, 1176.0],
                k as f64 * 0.016,
                &mut key,
            );
            flashes += usize::from(o.flash.is_some_and(|(_, t)| t == k as f64 * 0.016));
            assert!(!scene.prims.is_empty());
        }
        let per = t0.elapsed().as_secs_f64() / n as f64;
        eprintln!(
            "cosmic panel: {:.3} ms per frame, {} prims",
            per * 1e3,
            scene.prims.len()
        );
        assert!(flashes > 15, "{flashes}");
        if !cfg!(debug_assertions) {
            assert!(per < 3e-4, "{per}");
        }
    }

    #[test]
    fn font_covers_every_reading_and_label() {
        let missing = |t: &str| {
            t.chars()
                .filter(|c| !c.is_whitespace())
                .find(|c| crate::font::glyph(*c) == crate::font::glyph('?'))
        };
        let s = CosmicScale::new(
            &CosmicScaleSettings {
                enabled: true,
                metres_per_point: 0.0,
            },
            1000.0,
        );
        for m in &s.marks {
            assert_eq!(missing(&m.label), None, "{}", m.label);
        }
        for k in -4..6200 {
            let r = s.reading(k as f64 * 0.5);
            for t in [&r.line, &r.detail] {
                assert_eq!(missing(t), None, "{t}");
            }
        }
    }
}
