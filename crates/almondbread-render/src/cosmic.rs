//! The cosmic size scale panel (kb: decisions/controls, "Cosmic size scale"): a log ruler of
//! real things (`almondbread_core::cosmic`) sliding past a fixed marker as the view zooms,
//! one sentence, its assumption, light's crossing time and the probe energy for small views. Drawn into the
//! overlay scene like the mini-map; costs a scene rebuild per frame while the zoom changes
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
    scale: Option<((String, u64, u64, i64), CosmicScale)>,
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
        let offset = d - view.zoom_log10();
        let key = (
            self.settings.anchor.clone(),
            self.settings.metres_per_point.to_bits(),
            (width_pt * 4.0).round() as u64,
            (offset * 1e4).round() as i64,
        );
        if self.scale.as_ref().is_none_or(|(k, _)| *k != key) {
            self.scale = Some((key, CosmicScale::new(&self.settings, width_pt, offset)));
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
    pub fn draw(
        &self,
        scene: &mut Scene,
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
        let lines: Vec<(&str, f32, [f32; 4])> = [
            Some((reading.line.as_str(), s1, rgba(1.0, 1.0, 1.0, 0.95))),
            Some((reading.detail.as_str(), s2, rgba(0.9, 0.92, 0.95, 0.95))),
            reading.physics.as_deref().map(|e| (e, s2, rgba(0.8, 0.9, 1.0, 0.95))),
        ]
        .into_iter()
        .flatten()
        .collect();
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
        let (rx0, rx1) = (x0 + pad, x1 - pad);
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
        // The unexplored band: smallest probed scale to the Planck length.
        let (b0, b1) = (to_x(scale.desert.0), to_x(scale.desert.1));
        if b1 > rx0 && b0 < rx1 {
            scene.rect(
                [b0.max(rx0), yr - 4.0 * px, b1.min(rx1), yr + 4.0 * px],
                2.0 * px,
                rgba(1.0, 0.35, 0.3, 0.22),
                [0.0; 4],
                0.0,
            );
        }
        // Decade ticks where they are at least 5 points apart (physical modes).
        let (dlo, dhi) = (d - 40.0, d + 40.0);
        let mut k = dlo.ceil();
        while k <= dhi && !scale.relative {
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
            k += 1.0;
        }
        // Inside each ladder (cosmos length) past the milestones: its rungs as faint ticks,
        // so the repeats read as ladders.
        for (md, g) in scale.rungs(d - 400.0, d + 600.0) {
            let x = to_x(md);
            if x > rx0 && x < rx1 {
                let c = group_color(g);
                let al = 0.55 * fade(x);
                scene.segment(
                    [f64::from(x), f64::from(yr - 3.0 * px)],
                    [f64::from(x), f64::from(yr + 3.0 * px)],
                    0.5 * px,
                    rgba(c[0], c[1], c[2], al),
                    rgba(c[0], c[1], c[2], al),
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
            let lx0 = (x - 0.5 * tw).clamp(rx0, (rx1 - tw).max(rx0));
            let lx1 = lx0 + tw;
            if x < rx0 || x > rx1 || lx1 > rx1 || (lx0 + 0.5 * tw - x).abs() > 0.3 * tw {
                continue;
            }
            let free = |r: &Vec<(f32, f32)>| r.iter().all(|&(a0, a1)| lx1 + gap < a0 || lx0 > a1 + gap);
            let row = match m.row {
                Some(r) if free(&rows[r]) => r,
                Some(_) => continue,
                None if free(&rows[0]) => 0,
                None if free(&rows[1]) => 1,
                None => continue,
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
            // Zoom in steadily from home to 1e60 (big steps: every frame crosses something).
            let z = 60.0 * k as f64 / n as f64;
            let view = Viewport::new(-0.75, 0.1, 3.0 / 1000.0 / 10f64.powf(z));
            o.update(&view, (2000, 1200), 0.5, k as f64 * 0.016);
            scene.prims.clear();
            key.clear();
            o.draw(
                &mut scene,
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
        assert!(flashes > 20, "{flashes}");
        if !cfg!(debug_assertions) {
            assert!(per < 3e-4, "{per}");
        }
    }

    #[test]
    fn font_covers_every_reading_and_label() {
        use almondbread_core::cosmic::ANCHORS;
        let missing = |t: &str| {
            t.chars()
                .filter(|c| !c.is_whitespace() && *c != '?')
                .find(|c| crate::font::glyph(*c) == crate::font::glyph('?'))
        };
        for a in ANCHORS {
            let settings = CosmicScaleSettings {
                enabled: true,
                anchor: (*a).into(),
                metres_per_point: 0.0,
            };
            let s = CosmicScale::new(&settings, 1000.0, 0.2);
            for m in &s.marks {
                assert_eq!(missing(&m.label), None, "{a}: {}", m.label);
            }
            for k in -4..6200 {
                let r = s.reading(k as f64 * 0.5);
                for t in [Some(&r.line), Some(&r.detail), r.physics.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    assert_eq!(missing(t), None, "{a}: {t}");
                }
            }
        }
    }

    #[test]
    fn font_covers_the_panel_texts() {
        for t in [
            "The whole set ≈ 3× the Milky Way",
            "seeing it takes ≈ 12 keV (X-rays)",
            "1 ladder: observable universe → Planck length, ×1e61.7",
            "if this view is your 31 cm wide window",
            "Neptune's orbit 1/40 of",
            "238 μs after the Big Bang: quarks bind into protons",
        ] {
            for ch in t.chars().filter(|c| !c.is_whitespace()) {
                assert!(crate::font::glyph(ch) != crate::font::glyph('?') || ch == '?', "{ch}");
            }
        }
    }
}
