//! The Mandelbrot panel of a Julia view (kb: decisions/controls "Mandelbrot panel",
//! plans/f4-deep-julia "As built"): in a Julia view the Julia panel's place in the map row
//! shows the Mandelbrot view the Julia set was opened from (the way back), with a marker at
//! `k`; a click there goes back. A panel click entered a shallow view containing `k`; a twin
//! entered the deep view centred on `k`.
//!
//! The picture is a small renderer of its own at the panel's pixel size, like a mini-map level
//! (`overlay.rs`), stepped with the maps (2 ms chunks, never in a gesture's budget). At depth
//! it starts from the reference of the view it came from when the main renderer still had one
//! (offered as its spare, taken when it fits), else the normal background reference path; the
//! last picture shows until a new one is presented. The view it shows never moves, so it renders
//! once and settles.

use almondbread_core::Formula;
use almondbread_core::reference::Reference;
use almondbread_core::settings::RenderSettings;
use almondbread_core::viewport::{Center, Scale, Viewport};

use crate::backend::{GpuBackend, GpuError, Storage, TextureFormat};
use crate::scheduler::{Renderer, SchedulerConfig};

/// The way back as the host hands it to the overlay each frame: the Mandelbrot view the Julia
/// set was opened from (in view points of the main view, `view_points` big), its formula, and
/// the Julia set's `k`.
#[derive(Clone, Debug, PartialEq)]
pub struct WayBack {
    pub view: Viewport,
    pub formula: Formula,
    pub view_points: (f64, f64),
    pub k: Center,
}

/// The panel's viewport (scale per panel pixel): the way-back view's centre and angle, zoomed
/// out so the whole view (`view_points` at its scale) fits in `panel` pixels.
pub fn panel_view(back: &Viewport, view_points: (f64, f64), panel: (usize, usize)) -> Viewport {
    let (w, h) = (view_points.0.max(1.0), view_points.1.max(1.0));
    let (pw, ph) = (panel.0.max(1) as f64, panel.1.max(1) as f64);
    let ratio = (w / pw).max(h / ph);
    let scale = Scale::from_log2(back.scale.log2() + ratio.log2());
    let mut v = Viewport::with_center(back.center.clone(), scale).with_angle(back.angle);
    v.ensure_precision();
    v
}

/// Where `k` lies in a panel showing `view` at `panel` pixels: pixels from the panel's top-left
/// corner (x right, y down). Exact at any depth (`offset_of` takes the difference in high
/// precision); a twin's `k` is the view centre, the panel's middle.
pub fn marker_at(view: &Viewport, k: &Center, panel: (usize, usize)) -> [f64; 2] {
    let (x, y) = view.offset_of(k);
    [panel.0 as f64 / 2.0 + x, panel.1 as f64 / 2.0 - y]
}

struct Picture<B: GpuBackend> {
    renderer: Renderer<B>,
    target: B::Texture,
    size: (usize, usize),
    /// What it shows: the way back it was made for.
    back: WayBack,
    viewport: Viewport,
    marker: [f64; 2],
    /// A frame is presented (the panel shows a picture).
    shown: bool,
    /// The finished picture of the same way back at the previous panel size, shown stretched
    /// until this one is complete (kb: decisions/controls "Large mini-map").
    previous: Option<(B::Texture, (usize, usize))>,
}

/// The panel's state: its picture (None while no Julia view shows it).
pub struct MandelbrotPanel<B: GpuBackend> {
    picture: Option<Picture<B>>,
    /// Counts new pictures (renderers or textures) and every frame presented into the texture:
    /// the overlay scene's key, so the overlay is composited again after each one (a frame
    /// nothing composited stayed off screen while the main view was settled).
    pub serial: u64,
    /// The longest time (seconds, CPU) one call took making or stepping the current picture
    /// (tests: the panel never stalls a frame).
    pub longest_call: f64,
}

impl<B: GpuBackend> Default for MandelbrotPanel<B> {
    fn default() -> Self {
        Self {
            picture: None,
            serial: 0,
            longest_call: 0.0,
        }
    }
}

impl<B: GpuBackend + Clone> MandelbrotPanel<B> {
    /// The panel's texture (bound as the map slot after the maps).
    pub fn texture(&self) -> Option<&B::Texture> {
        self.picture.as_ref().map(|p| &p.target)
    }

    /// A picture is presented.
    pub fn shown(&self) -> bool {
        self.picture.as_ref().is_some_and(|p| p.shown)
    }

    /// What the panel shows: the picture once complete, before that the previous size's
    /// picture (stretched: its size), else its partial frames once there are any.
    pub fn picture(&self) -> Option<crate::overlay::PanelPicture<'_, B>> {
        let p = self.picture.as_ref()?;
        if !(p.shown && p.renderer.complete_presented())
            && let Some((t, size)) = &p.previous
        {
            return Some((t, Some(*size)));
        }
        p.shown.then_some((&p.target, None))
    }

    /// The marker at k: pixels from the panel's top-left corner.
    pub fn marker(&self) -> Option<[f64; 2]> {
        self.picture.as_ref().map(|p| p.marker)
    }

    /// The panel's viewport (scale per panel pixel).
    pub fn viewport(&self) -> Option<&Viewport> {
        self.picture.as_ref().map(|p| &p.viewport)
    }

    /// The renderer (tests: is it settled, which reference does it use).
    pub fn renderer(&self) -> Option<&Renderer<B>> {
        self.picture.as_ref().map(|p| &p.renderer)
    }

    /// Still rendering: keep frames coming.
    pub fn busy(&self) -> bool {
        self.picture
            .as_ref()
            .is_some_and(|p| !p.shown || !p.renderer.is_settled())
    }

    /// Not shown (outside a Julia view, or hidden with U): frees the renderer and its reference.
    pub fn clear(&mut self) {
        if self.picture.take().is_some() {
            self.serial += 1;
        }
    }

    /// Shows `back` at `panel` pixels. A new way back or panel size makes a new picture (the
    /// renderer is reused for a new view; a new size needs a new texture); `reference`: the
    /// way-back view's reference if the main renderer still had it (offered to a new picture).
    pub fn update(
        &mut self,
        main: &Renderer<B>,
        back: &WayBack,
        panel: (usize, usize),
        reference: Option<&Reference>,
    ) -> Result<(), GpuError> {
        if self
            .picture
            .as_ref()
            .is_some_and(|p| p.size == panel && p.back == *back)
        {
            return Ok(());
        }
        let start = std::time::Instant::now();
        let viewport = panel_view(&back.view, back.view_points, panel);
        let marker = marker_at(&viewport, &back.k, panel);
        let old = self.picture.take();
        // The same way back at a new size: its finished picture (or, resized again before
        // it finished, the one before) shows stretched until the new one is complete.
        let mut previous = None;
        let (mut renderer, target) = match old {
            Some(p) if p.size == panel => (p.renderer, p.target),
            old => {
                if let Some(p) = old.filter(|p| p.back == *back) {
                    previous = if p.shown && p.renderer.complete_presented() {
                        Some((p.target, p.size))
                    } else {
                        p.previous
                    };
                }
                let config = SchedulerConfig {
                    live_capacity: 1 << 16,
                    max_list_capacity: 1 << 16,
                    chunk_target: 0.002,
                    interactive_chunk_target: 0.002,
                    max_chunks_in_flight: 1,
                    extension_budget: 0.05,
                    aa_budget: 0.02,
                    aa_chunk_target: 0.002,
                    ..SchedulerConfig::default()
                };
                let r = Renderer::with_config(main.backend().clone(), config)?;
                let t = main
                    .backend()
                    .create_texture(panel.0, panel.1, TextureFormat::Bgra8Unorm, Storage::Private)?;
                (r, t)
            }
        };
        renderer.settings = settings_for(&main.settings, back.formula);
        renderer.viewport = viewport.clone();
        if let Some(r) = reference {
            renderer.offer_reference(r.clone());
        }
        self.serial += 1;
        self.picture = Some(Picture {
            renderer,
            target,
            size: panel,
            back: back.clone(),
            viewport,
            marker,
            shown: false,
            previous,
        });
        self.longest_call = start.elapsed().as_secs_f64();
        Ok(())
    }

    /// Steps the renderer (with the maps: not during a gesture, or every 8th refresh of one).
    pub fn step(&mut self, main: &RenderSettings, now: f64) {
        let Some(p) = &mut self.picture else { return };
        let want = settings_for(main, p.back.formula);
        if p.renderer.settings.palette_id != want.palette_id
            || p.renderer.settings.palette_stops != want.palette_stops
            || p.renderer.settings.color_density != want.color_density
        {
            p.renderer.settings = want;
        }
        if p.shown && p.renderer.is_settled() {
            return;
        }
        let start = std::time::Instant::now();
        let t = p.target.clone();
        let out = p
            .renderer
            .render_frame(now, p.size.0, p.size.1, 1.0, &mut |_| Some((t.clone(), None)));
        self.longest_call = self.longest_call.max(start.elapsed().as_secs_f64());
        if out.presented {
            p.shown = true;
            self.serial += 1;
        }
        if p.shown && p.renderer.complete_presented() {
            p.previous = None;
        }
    }
}

/// The panel's render settings: the mini-maps' (flat interior, 2×2, no animation) of the
/// way-back formula.
fn settings_for(main: &RenderSettings, formula: Formula) -> RenderSettings {
    let mut s = crate::overlay::map_settings(main);
    s.formula = formula;
    s
}

/// The Mandelbrot panel through a real host on Metal (windowless layer): shown by default in a
/// Julia view, U hides it, its marker sits at k, a click on it goes back to the exact view.
#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use super::*;
    use crate::host::Host;
    use crate::metal::{MetalBackend, MetalLayerSurface};
    use almondbread_core::InputEvent;
    use objc2_quartz_core::CAMetalLayer;

    const W: f64 = 400.0;
    const H: f64 = 250.0;

    struct Rig {
        host: Host<MetalBackend>,
        surface: MetalLayerSurface,
        start: std::time::Instant,
    }

    impl Rig {
        fn new() -> Self {
            let backend = MetalBackend::new().expect("metal");
            let mut surface = MetalLayerSurface::new(CAMetalLayer::new(), &backend);
            surface.set_drawable_size((W * 2.0) as usize, (H * 2.0) as usize);
            let mut host = Host::new(Renderer::new(backend).expect("renderer"));
            host.resize(W, H, 2.0, 0.0);
            host.overlay.settings.julia = false;
            host.overlay.settings.map = false;
            host.overlay.settings.orbit = false;
            Self {
                host,
                surface,
                start: std::time::Instant::now(),
            }
        }

        /// `n` refreshes; the longest `Host::frame` call in seconds.
        fn frames(&mut self, n: usize) -> f64 {
            let start = self.start;
            let now = move || start.elapsed().as_secs_f64();
            let mut longest: f64 = 0.0;
            for _ in 0..n {
                let t = now();
                let ready = self.surface.drawable_ready(t);
                let surface = &mut self.surface;
                self.host
                    .frame(t, &now, ready, &mut |f| surface.next_drawable(t, f).map(|d| (d, 0)));
                longest = longest.max(now() - t);
                std::thread::sleep(std::time::Duration::from_millis(4));
            }
            longest
        }

        /// Refreshes until the Mandelbrot panel shows a settled picture (at most `n`).
        fn until_panel_settled(&mut self, n: usize) -> bool {
            for _ in 0..n {
                self.frames(1);
                if self.host.overlay.mandelbrot.shown() && !self.host.overlay.mandelbrot.busy() {
                    return true;
                }
            }
            false
        }

        fn panel(&self, id: &str) -> Option<[f64; 4]> {
            self.host
                .overlay_panels()
                .into_iter()
                .find(|(p, _)| *p == id)
                .map(|(_, r)| r)
        }

        /// A click at the middle of panel `id`, as the platform sends it (view points from the
        /// centre, y up).
        fn click(&mut self, id: &str) -> bool {
            let r = self.panel(id).unwrap_or_else(|| panic!("no {id} panel"));
            let (x, y) = (r[0] + r[2] / 2.0 - W / 2.0, H / 2.0 - (r[1] + r[3] / 2.0));
            self.host.map_click(x, y, false)
        }
    }

    /// Panel-click entry: the Julia panel's set opens; the Mandelbrot panel takes the Julia
    /// panel's place, its marker at k (the pointer's point); U hides and shows it; a click on
    /// it goes back to the exact view.
    #[test]
    fn panel_click_entry_shows_the_way_back_and_returns() {
        let mut rig = Rig::new();
        rig.host.renderer.viewport = Viewport::new(-0.7453, 0.1127, 6e-5).with_angle(0.3);
        rig.host.overlay.settings.julia = true;
        rig.host.input(InputEvent::Pointer(Some((60.0, 30.0))), 0.0);
        rig.frames(20);
        let julia_rect = rig.panel("julia").expect("the Julia panel");
        let from = rig.host.renderer.viewport.clone();
        let k = rig.host.overlay.julia_k.clone().expect("panel k");
        assert!(rig.click("julia"));
        assert!(rig.host.renderer.settings.formula.is_julia());
        rig.frames(3);
        assert!(rig.panel("julia").is_none(), "the Julia panel leaves its place");
        let r = rig.panel("mandelbrot-panel").expect("the Mandelbrot panel shows");
        assert_eq!(r, julia_rect, "the Julia panel's place and size");
        assert!(rig.until_panel_settled(2000), "the panel's picture settles");
        // The marker at k: from the panel's centre, k's offset in the view (points, screen
        // axes of the turned view) over the panel's zoom-out (the whole view fits).
        let pv = rig.host.overlay.mandelbrot.viewport().expect("viewport").clone();
        let m = rig.host.overlay.mandelbrot.marker().expect("marker");
        let (pw, ph) = ((r[2] * 2.0).round(), (r[3] * 2.0).round());
        let ratio = (W / pw).max(H / ph);
        let rel = pv.scale.to_f64() / (from.scale.to_f64() * ratio) - 1.0;
        assert!(rel.abs() < 1e-12, "panel scale off by {rel}");
        assert_eq!(pv.angle, from.angle);
        let (ox, oy) = from.offset_of(&k);
        assert!(
            (ox - 60.0).abs() < 1e-6 && (oy - 30.0).abs() < 1e-6,
            "k is the pointer's point"
        );
        let want = [pw / 2.0 + ox / ratio, ph / 2.0 - oy / ratio];
        assert!(
            (m[0] - want[0]).abs() < 1e-6 && (m[1] - want[1]).abs() < 1e-6,
            "marker {m:?}, want {want:?}"
        );
        // The panel's point under the marker is k.
        let under = pv.point_at_offset(m[0] - pw / 2.0, ph / 2.0 - m[1]);
        let (dx, dy) = under.delta_in(&k, pv.scale);
        assert!(dx.abs() < 1e-6 && dy.abs() < 1e-6, "({dx}, {dy}) pixels off");
        // U hides it, U shows it again.
        assert!(rig.host.toggle_mandelbrot_panel());
        assert!(!rig.host.mandelbrot_panel_on());
        rig.frames(3);
        assert!(rig.panel("mandelbrot-panel").is_none());
        assert!(
            rig.host.overlay.mandelbrot.texture().is_none(),
            "hidden: renderer freed"
        );
        assert!(rig.host.toggle_mandelbrot_panel());
        rig.frames(3);
        assert!(rig.panel("mandelbrot-panel").is_some());
        // Accessibility: a button with its own id and label.
        let a = rig.host.accessible_controls(0.0);
        let e = a.iter().find(|c| c.id == "mandelbrot-panel").expect("a11y element");
        assert_eq!(e.label, "Mandelbrot set panel");
        assert_eq!(e.role, crate::accessibility::Role::Button);
        assert!(e.value.ends_with("click to return"), "{}", e.value);
        // A click on it: back to the exact view (centre, scale and angle bit for bit).
        assert!(rig.click("mandelbrot-panel"));
        assert!(!rig.host.renderer.settings.formula.is_julia());
        assert_eq!(rig.host.renderer.viewport, from);
        assert_eq!(rig.host.renderer.viewport.center, from.center);
        assert_eq!(rig.host.renderer.viewport.scale, from.scale);
        rig.frames(3);
        assert!(rig.panel("mandelbrot-panel").is_none());
        assert!(
            rig.panel("julia").is_some(),
            "the Julia panel is back in Mandelbrot views"
        );
        // U outside a Julia view is not the Mandelbrot panel's.
        assert!(!rig.host.toggle_mandelbrot_panel());
    }

    /// The panel is on in every Julia view, also with the Julia panel off and after it was
    /// hidden in the previous one; a deep twin (1e-32) renders it without stalling a frame,
    /// from the view's own reference, the marker at the panel's centre (k is the view centre).
    #[test]
    fn deep_twin_shows_the_panel_without_a_stall() {
        let mut rig = Rig::new();
        rig.host.renderer.viewport = Viewport::with_center(
            Center::parse(
                "-0.743643887037158704752191506114774",
                "0.131825904205311970493132056385139",
            )
            .expect("centre"),
            Scale::from_f64(1e-32),
        );
        // The deep view settles first (its reference is what the panel starts from).
        for _ in 0..3000 {
            rig.frames(1);
            if rig.host.renderer.is_settled() {
                break;
            }
        }
        assert!(rig.host.renderer.is_settled(), "the deep view settles");
        let from = rig.host.renderer.viewport.clone();
        let main_ref = rig.host.renderer.reference().expect("reference").point.clone();
        assert!(rig.host.julia_twin());
        rig.frames(2);
        assert!(
            rig.panel("mandelbrot-panel").is_some(),
            "on by default with the Julia panel off"
        );
        // Hidden in this Julia view, on again at the next entry.
        assert!(rig.host.toggle_mandelbrot_panel());
        rig.frames(2);
        assert!(rig.panel("mandelbrot-panel").is_none());
        // The frames while the panel renders (and the Julia view's first ones): the longest
        // `Host::frame` call (CPU) stays far from a stall. One measurement is a fresh entry
        // into the Julia view; it is repeated (at most twice more) only while over the bound,
        // so a stall that happens at every entry fails and one descheduling of the test thread
        // (57 ms seen during the parallel workspace run) does not.
        // Timing bounds in optimised builds only.
        let bound = if cfg!(debug_assertions) { f64::INFINITY } else { 0.008 };
        let call = crate::test_timing::least_within(bound, 3, || {
            assert!(rig.host.back_from_julia());
            rig.frames(2);
            assert!(rig.host.julia_twin());
            let mut longest: f64 = 0.0;
            let mut settled = false;
            let mut n = 0;
            for _ in 0..3000 {
                longest = longest.max(rig.frames(1));
                n += 1;
                if rig.host.overlay.mandelbrot.shown() && !rig.host.overlay.mandelbrot.busy() {
                    settled = true;
                    break;
                }
            }
            assert!(settled, "the panel's deep picture settles");
            let call = rig.host.overlay.mandelbrot.longest_call;
            eprintln!(
                "mandelbrot panel at 1e-32: settled after {n} refreshes; longest panel call {:.2} ms, longest frame {:.2} ms",
                call * 1000.0,
                longest * 1000.0
            );
            call
        });
        // (The panel's own work per refresh; the whole frame also waits for a loaded machine.)
        assert!(call < bound, "a panel call took {:.1} ms", call * 1000.0);
        let r = rig.host.overlay.mandelbrot.renderer().expect("renderer");
        let used = r.reference().expect("panel reference");
        assert!(used.big, "a deep reference");
        assert_eq!(used.point, main_ref, "the panel took the view's reference");
        let m = rig.host.overlay.mandelbrot.marker().expect("marker");
        let p = rig.panel("mandelbrot-panel").expect("panel");
        let (pw, ph) = ((p[2] * 2.0).round(), (p[3] * 2.0).round());
        assert!(
            (m[0] - pw / 2.0).abs() < 1e-9 && (m[1] - ph / 2.0).abs() < 1e-9,
            "{m:?}"
        );
        assert!(rig.click("mandelbrot-panel"));
        assert_eq!(rig.host.renderer.viewport, from);
    }
}
