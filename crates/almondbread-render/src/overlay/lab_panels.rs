//! The parameter lab's panels in the overlay row (F10, kb: plans/f10-parameter-lab): one panel
//! per plane (c, z₀, p), drawn as the Julia panel is (frame, picture, title, value), each with a
//! marker at the current value of its knob. A press on a panel sets that knob to the value under
//! it and a drag moves it; the "show" button in a panel's corner makes its plane the main view.
//! The main view's own plane has a panel too (a whole-plane map with the main view's rectangle).
//! Where the row has room for fewer than three panels (a phone), the main plane's panel goes
//! first. The orbit overlay, when on, draws the orbit of the current knobs in the z₀ panel (the
//! plane z moves in) and over the main view when that is the c or z₀ plane. Zooming into the
//! precision limit shows a note at the bottom of the view for a few seconds.

use almondbread_core::lab::{self, Plane};
use almondbread_core::settings::OverlaySettings;
use almondbread_core::viewport::{Center, Viewport};

use super::{MAP_ASPECT, ORBIT_COOL, ORBIT_WARM, Overlay, Scene, complex_text, fit_maps, map_width, rgba, row_origins};
use crate::backend::GpuBackend;
use crate::scheduler::Renderer;

/// A press the lab panels took: its drag sets a knob, or it is on a panel's "show" button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabDrag {
    Value(Plane),
    Show(Plane),
}

/// Seconds the precision-limit note stays after zooming last ran into the limit.
pub const LIMIT_NOTE: f64 = 3.0;

/// The note at the precision limit.
pub const LIMIT_TEXT: &str = "Zoom ends here: the lab computes with 7-digit numbers";

/// The panels' order in the row: the planes not shown in the main view first.
fn row_order(main: Plane) -> [Plane; 3] {
    let mut v = [Plane::C, Plane::Z0, Plane::P];
    v.sort_by_key(|p| (*p == main, p.index()));
    v
}

/// The "show" button of a panel at `r` (pixels): its rect.
fn show_button(r: [f32; 4], px: f32) -> [f32; 4] {
    let (w, h) = (40.0 * px, 18.0 * px);
    let m = 5.0 * px;
    [r[2] - m - w, r[1] + m, r[2] - m, r[1] + m + h]
}

impl<B: GpuBackend> Overlay<B> {
    /// Lays out the lab panels and draws new pictures where needed (`prepare`, while the lab is
    /// on).
    pub(super) fn prepare_lab(
        &mut self,
        renderer: &mut Renderer<B>,
        size: (usize, usize),
        ppp: f64,
        now: f64,
        s: &OverlaySettings,
    ) {
        let Some(lab) = renderer.lab.as_ref() else {
            self.lab_at = None;
            return;
        };
        let main = lab.plane;
        let values = lab.values;
        let kernels = renderer.lab_kernels();
        let px_per_pt = 1.0 / ppp;
        let fit = fit_maps(
            3,
            map_width(s, size, ppp),
            false,
            s.map_corner,
            size,
            ppp,
            renderer.navigator.insets(),
            self.cosmic.inset_right,
            self.pi.inset_top,
            &self.cockpit,
        );
        let panel = (
            ((fit.width * px_per_pt).round() as usize).clamp(32, 4096),
            ((fit.width * MAP_ASPECT * px_per_pt).round() as usize).clamp(24, 4096),
        );
        let n = fit.levels.clamp(1, 3);
        let origins = row_origins(
            n,
            s.map_corner,
            panel,
            size,
            renderer.navigator.insets(),
            &fit,
            px_per_pt,
        );
        let order = row_order(main);
        self.lab_main = Some(main);
        self.lab_at = Some((order.iter().take(n).copied().zip(origins).collect(), panel));
        if let Some(k) = kernels {
            let settings = renderer.settings.clone();
            if let Err(e) = self.lab.update(renderer.backend(), &k, &values, panel, &settings, now) {
                eprintln!("almondbread: lab panels: {e}");
            }
        }
    }

    /// The panels' textures in row order (the overlay kernel's map slots).
    pub(super) fn lab_textures(&self) -> Vec<B::Texture> {
        let Some((at, _)) = &self.lab_at else {
            return Vec::new();
        };
        at.iter()
            .filter_map(|(p, _)| self.lab.panels[p.index()].texture().cloned())
            .collect()
    }

    /// The panel under (x, y) pixels and what a press there does: the knob's value under it, or
    /// its "show" button (None value).
    pub fn lab_hit(&self, x: f64, y: f64, ppp: f64) -> Option<(Plane, Option<(f64, f64)>)> {
        let (at, (pw, ph)) = self.lab_at.as_ref()?;
        let px = (1.0 / ppp) as f32;
        let main = self.lab_main?;
        for (plane, o) in at {
            let r = [o[0], o[1], o[0] + *pw as f32, o[1] + *ph as f32];
            if !(f64::from(r[0])..f64::from(r[2])).contains(&x) || !(f64::from(r[1])..f64::from(r[3])).contains(&y) {
                continue;
            }
            if *plane != main {
                // (A fingertip's margin around the button on touch screens.)
                let b = show_button(r, px);
                let m = if self.cockpit.touch { 8.0 * f64::from(px) } else { 0.0 };
                if x >= f64::from(b[0]) - m
                    && x <= f64::from(b[2]) + m
                    && y >= f64::from(b[1]) - m
                    && y <= f64::from(b[3]) + m
                {
                    return Some((*plane, None));
                }
            }
            return Some((*plane, Some(self.lab_value_at(*plane, x, y)?)));
        }
        None
    }

    /// The value of `plane`'s knob at (x, y) pixels over its panel (outside it too: a drag may
    /// leave the panel).
    pub fn lab_value_at(&self, plane: Plane, x: f64, y: f64) -> Option<(f64, f64)> {
        let (at, size) = self.lab_at.as_ref()?;
        let (_, o) = at.iter().find(|(p, _)| *p == plane)?;
        Some(crate::lab::panel_value(
            plane,
            *size,
            x - f64::from(o[0]),
            y - f64::from(o[1]),
        ))
    }

    /// Draws the lab panels, the orbit and the limit note; returns the row's rect (pixels).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_lab(
        &self,
        scene: &mut Scene,
        renderer: &Renderer<B>,
        view: &Viewport,
        size: (usize, usize),
        ppp: f64,
        orbit: bool,
        orbit_length: usize,
        now: f64,
        key: &mut String,
    ) -> Option<[f32; 4]> {
        let lab = renderer.lab.as_ref()?;
        let px = (1.0 / ppp) as f32;
        let (w, h) = (size.0 as f64, size.1 as f64);
        let values = lab.values;
        let pts = if orbit {
            lab::orbit(&values, orbit_length.clamp(2, 400))
        } else {
            Vec::new()
        };
        // ---- The orbit over the main view (c and z0 planes: z moves in the same complex plane).
        if orbit && lab.plane != Plane::P && pts.len() > 1 {
            let to_px = |z: (f64, f64)| {
                let (x, y) = view.offset_of(&Center::new(z.0, z.1));
                [w / 2.0 + x / ppp, h / 2.0 - y / ppp]
            };
            let mp: Vec<[f64; 2]> = pts.iter().map(|&z| to_px(z)).collect();
            draw_orbit(scene, &mp, px, 1.0);
            key.push_str(&format!("lab orbit {} {:?} ", mp.len(), mp.last()));
        }
        // ---- The panels.
        let mut union: Option<[f32; 4]> = None;
        if let Some((at, (pw, ph))) = &self.lab_at {
            let (pw, ph) = (*pw as f32, *ph as f32);
            // (Map slots in `lab_textures` order: the panels that have a texture.)
            let mut slot = 0u32;
            for (plane, o) in at {
                let plane = *plane;
                let r = [o[0], o[1], o[0] + pw, o[1] + ph];
                union = Some(union.map_or(r, |u| [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])]));
                let main = plane == lab.plane;
                let radius = 8.0 * px;
                let pad = 3.0 * px;
                scene.set_clip(None);
                scene.rect(
                    [r[0] - pad, r[1] - pad, r[2] + pad, r[3] + pad],
                    radius + pad,
                    rgba(0.02, 0.02, 0.03, 1.0),
                    if main {
                        rgba(1.0, 0.85, 0.3, 0.9)
                    } else {
                        rgba(1.0, 1.0, 1.0, 0.22)
                    },
                    if main { 1.6 * px } else { px },
                );
                let pic = &self.lab.panels[plane.index()];
                if pic.texture().is_some() {
                    if pic.shown {
                        scene.image(r, slot, 1.0, radius, None);
                    }
                    slot += 1;
                }
                scene.set_clip(Some(r));
                let panel = (pw as usize, ph as usize);
                let to_panel = |v: (f64, f64)| {
                    let (x, y) = crate::lab::panel_position(plane, panel, v);
                    [f64::from(o[0]) + x, f64::from(o[1]) + y]
                };
                // The main view's rectangle on its plane's panel.
                if main {
                    let home = lab::home_view(plane, panel.0 as f64, panel.1 as f64);
                    let ratio = view.scale.to_f64() / home.scale.to_f64() * ppp;
                    let (vw, vh) = (w * ratio, h * ratio);
                    let c = to_panel((view.center.re(), view.center.im()));
                    if vw >= f64::from(6.0 * px) {
                        let (cs, sn) = view.rotation();
                        let corner = |u: f64, v: f64| {
                            let (x, y) = (u * vw / 2.0, v * vh / 2.0);
                            [c[0] + x * cs - y * sn, c[1] - (x * sn + y * cs)]
                        };
                        let q = [
                            corner(-1.0, 1.0),
                            corner(1.0, 1.0),
                            corner(1.0, -1.0),
                            corner(-1.0, -1.0),
                        ];
                        let col = rgba(1.0, 0.85, 0.3, 0.8);
                        for k in 0..4 {
                            let (a, b) = (q[k], q[(k + 1) % 4]);
                            scene.segment(a, b, 1.3 * px, rgba(0.0, 0.0, 0.0, 0.5), rgba(0.0, 0.0, 0.0, 0.5));
                            scene.segment(a, b, 0.6 * px, col, col);
                        }
                    }
                    key.push_str(&format!("labview {:.1} {:.1} {:.1} ", c[0], c[1], vw));
                }
                // The orbit in the z0 panel (the plane z moves in).
                if plane == Plane::Z0 && pts.len() > 1 {
                    let mp: Vec<[f64; 2]> = pts.iter().map(|&z| to_panel(z)).collect();
                    draw_orbit(scene, &mp, px, 0.7);
                }
                // The marker at the knob's value: a crosshair (dark under light).
                let m = to_panel(values.get(plane));
                let m = [
                    m[0].clamp(f64::from(r[0]), f64::from(r[2])),
                    m[1].clamp(f64::from(r[1]), f64::from(r[3])),
                ];
                let (ring, gap, arm) = (f64::from(5.0 * px), f64::from(3.0 * px), f64::from(6.0 * px));
                let dark = rgba(0.0, 0.0, 0.0, 0.75);
                let light = rgba(1.0, 0.86, 0.45, 1.0);
                for (lw, col) in [(1.6 * px, dark), (0.7 * px, light)] {
                    scene.ring(m, ring as f32, lw * 1.4, col);
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        let a = [m[0] + dx * (ring + gap), m[1] + dy * (ring + gap)];
                        let b = [m[0] + dx * (ring + gap + arm), m[1] + dy * (ring + gap + arm)];
                        scene.segment(a, b, lw, col, col);
                    }
                }
                scene.disc(m, 1.3 * px, light);
                // Title, value, and the "show" button.
                let text = (11.0 * px).max(9.0);
                let title = match plane {
                    Plane::C => "c plane",
                    Plane::Z0 => "z0 plane",
                    Plane::P => "p plane",
                };
                let title = if main {
                    format!("{title} (shown)")
                } else {
                    title.to_string()
                };
                scene.label(
                    [o[0] + 0.7 * text, o[1] + 0.7 * text],
                    text,
                    &title,
                    rgba(1.0, 1.0, 1.0, 0.85),
                );
                let small = (10.0 * px).max(8.5);
                let cap = crate::font::cap_height() * small;
                let name = match plane {
                    Plane::C => "c",
                    Plane::Z0 => "z0",
                    Plane::P => "p",
                };
                let value = format!("{name} = {}", complex_text(values.get(plane)));
                scene.label(
                    [o[0] + 0.7 * text, r[3] - 0.7 * text - cap],
                    small,
                    &value,
                    rgba(1.0, 1.0, 1.0, 0.8),
                );
                if !main {
                    let b = show_button(r, px);
                    scene.rect(b, 5.0 * px, rgba(0.1, 0.1, 0.12, 0.85), rgba(1.0, 1.0, 1.0, 0.45), px);
                    let tw = crate::font::layout("show", small).1;
                    scene.label(
                        [0.5 * (b[0] + b[2] - tw), 0.5 * (b[1] + b[3] - cap)],
                        small,
                        "show",
                        rgba(1.0, 1.0, 1.0, 0.95),
                    );
                }
                key.push_str(&format!(
                    "lab {} {} {} {:?} {value} {main} ",
                    plane.name(),
                    pic.serial,
                    slot,
                    o
                ));
            }
            scene.set_clip(None);
        }
        // ---- The precision limit's note, bottom centre.
        if let Some(t) = lab.at_limit
            && now - t < LIMIT_NOTE
        {
            let size_px = (13.0 * px).max(11.0);
            let tw = crate::font::layout(LIMIT_TEXT, size_px).1;
            let bottom = renderer.navigator.insets().2.max(0.0) as f32 * px;
            let y = h as f32 - bottom - 72.0 * px;
            let x = 0.5 * (w as f32 - tw);
            scene.rect(
                [
                    x - 10.0 * px,
                    y - 8.0 * px,
                    x + tw + 10.0 * px,
                    y + crate::font::cap_height() * size_px + 8.0 * px,
                ],
                8.0 * px,
                rgba(0.02, 0.02, 0.03, 0.8),
                rgba(1.0, 1.0, 1.0, 0.25),
                px,
            );
            scene.label([x, y], size_px, LIMIT_TEXT, rgba(1.0, 1.0, 1.0, 0.95));
            key.push_str("lab limit ");
        }
        self.drawn_lab.set(union);
        union
    }
}

/// An orbit as a fading line with dots, the start marked (`alpha`: overall strength).
fn draw_orbit(scene: &mut Scene, pts: &[[f64; 2]], px: f32, alpha: f32) {
    let n = pts.len();
    let fade = |k: usize| alpha * (1.0 - 0.8 * k as f32 / n.max(2) as f32);
    for k in 0..n.saturating_sub(1) {
        let c = |k: usize| rgba(ORBIT_COOL[0], ORBIT_COOL[1], ORBIT_COOL[2], 0.9 * fade(k));
        scene.segment(
            pts[k],
            pts[k + 1],
            1.4 * px,
            rgba(0.0, 0.0, 0.0, 0.45 * fade(k)),
            rgba(0.0, 0.0, 0.0, 0.45 * fade(k + 1)),
        );
        scene.segment(pts[k], pts[k + 1], 0.7 * px, c(k), c(k + 1));
    }
    for (k, p) in pts.iter().enumerate().skip(1) {
        scene.disc(*p, 2.6 * px, rgba(0.0, 0.0, 0.0, 0.7 * fade(k)));
        scene.disc(*p, 1.8 * px, rgba(1.0, 1.0, 1.0, fade(k)));
    }
    scene.disc(pts[0], 3.6 * px, rgba(0.0, 0.0, 0.0, 0.8 * alpha));
    scene.disc(
        pts[0],
        2.6 * px,
        rgba(ORBIT_WARM[0], ORBIT_WARM[1], ORBIT_WARM[2], alpha),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_plane_goes_last() {
        assert_eq!(row_order(Plane::C), [Plane::Z0, Plane::P, Plane::C]);
        assert_eq!(row_order(Plane::Z0), [Plane::C, Plane::P, Plane::Z0]);
        assert_eq!(row_order(Plane::P), [Plane::C, Plane::Z0, Plane::P]);
        for ch in LIMIT_TEXT.chars().filter(|c| !c.is_whitespace()) {
            assert_ne!(crate::font::glyph(ch), crate::font::glyph('?'), "{ch}");
        }
    }
}
