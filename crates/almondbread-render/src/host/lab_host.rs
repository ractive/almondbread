//! The parameter lab through the host (F10, kb: plans/f10-parameter-lab): entering and leaving
//! it (the view and the travel journal from before come back; the live journal meanwhile sees
//! the lab's views and goes with them, so what persists is `user_journal`), the plane switch,
//! presses on the plane panels (the platform routes them as it routes the compass's: press, drag, release), a
//! click in the main view setting the shown plane's knob, and what the lab does not have (the
//! autopilot, journeys, seek, the ⌘-click Julia set, the journal's steps).

use almondbread_core::cruise::Level;
use almondbread_core::input::{InputEvent, NavKey};
use almondbread_core::journal::Journal;
use almondbread_core::lab::{LabValues, Plane};
use almondbread_core::viewport::Viewport;

use super::{Host, InputOutcome};
use crate::backend::GpuBackend;
use crate::overlay::LabDrag;

impl<B: GpuBackend + Clone> Host<B> {
    /// The lab is the main view.
    pub fn lab_on(&self) -> bool {
        self.renderer.lab.is_some()
    }

    /// Enters the lab with these knobs, showing `plane`: a tour ends, motion stops, a Julia view
    /// is left first; the view and the travel journal are kept for the way back. False when the
    /// lab is on already.
    pub fn enter_lab(&mut self, values: LabValues, plane: Plane, now: f64) -> bool {
        if self.lab_on() {
            return false;
        }
        self.end_tour(false, now);
        self.leave_julia();
        self.stop_motion(now);
        let r = &mut self.renderer;
        r.navigator.journal_flush(now);
        self.lab_return = Some((r.viewport.clone(), r.navigator.journal().clone()));
        r.enter_lab(values, plane, self.view_points);
        self.overlay.lab_drag = None;
        true
    }

    /// Leaves the lab for the view it was entered from (its journal as it was); returns the
    /// knobs and plane (None: the lab was off).
    pub fn leave_lab(&mut self) -> Option<(LabValues, Plane)> {
        let out = self.renderer.leave_lab()?;
        let (w, h) = self.view_points;
        let r = &mut self.renderer;
        match self.lab_return.take() {
            Some((view, journal)) => {
                r.viewport = view;
                *r.navigator.journal_mut() = journal;
            }
            None => r.viewport = Viewport::fitting_for(r.settings.formula, w, h),
        }
        self.overlay.lab_drag = None;
        Some(out)
    }

    /// The user's travel journal (what persists): in the lab the one it was entered from.
    pub fn user_journal(&self) -> &Journal {
        match &self.lab_return {
            Some((_, journal)) => journal,
            None => self.renderer.navigator.journal(),
        }
    }

    /// The user's travel journal to change (a restored one).
    pub fn user_journal_mut(&mut self) -> &mut Journal {
        match &mut self.lab_return {
            Some((_, journal)) => journal,
            None => self.renderer.navigator.journal_mut(),
        }
    }

    /// Shows `plane` in the main view (its last view there, else its home view).
    pub fn set_lab_plane(&mut self, plane: Plane) -> bool {
        if !self.lab_on() {
            return false;
        }
        let vp = self.view_points;
        self.renderer.set_lab_plane(plane, vp);
        true
    }

    /// Sets the knobs (each clamped into its range).
    pub fn set_lab_values(&mut self, values: LabValues) -> bool {
        match self.renderer.lab.as_mut() {
            Some(l) => {
                l.values = values.sanitized();
                true
            }
            None => false,
        }
    }

    /// The knobs and the shown plane (None: the lab is off).
    pub fn lab_state(&self) -> Option<(LabValues, Plane)> {
        self.renderer.lab.as_ref().map(|l| (l.values, l.plane))
    }

    /// (x, y) view points from the centre, y up, as output pixels.
    fn to_pixels(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let ppp = self.points_per_pixel;
        (ppp > 0.0 && x.is_finite() && y.is_finite())
            .then(|| (self.width as f64 / 2.0 + x / ppp, self.height as f64 / 2.0 - y / ppp))
    }

    /// A lab panel is under (x, y) view points from the centre, y up (as drawn last).
    pub fn lab_panel_hit(&self, x: f64, y: f64) -> bool {
        self.lab_on()
            && self
                .to_pixels(x, y)
                .is_some_and(|(px, py)| self.overlay.lab_hit(px, py, self.points_per_pixel).is_some())
    }

    /// A press at (x, y) view points from the centre, y up: on a lab panel it sets that panel's
    /// knob to the value under it (its drag moves it, `lab_drag`), on a panel's "show" button it
    /// waits for the release; true if a panel took it.
    pub fn lab_press(&mut self, x: f64, y: f64) -> bool {
        if !self.lab_on() {
            return false;
        }
        let Some((px, py)) = self.to_pixels(x, y) else {
            return false;
        };
        match self.overlay.lab_hit(px, py, self.points_per_pixel) {
            Some((plane, None)) => {
                self.overlay.lab_drag = Some(LabDrag::Show(plane));
                true
            }
            Some((plane, Some(v))) => {
                self.overlay.lab_drag = Some(LabDrag::Value(plane));
                self.set_knob(plane, v);
                true
            }
            None => false,
        }
    }

    /// The pointer of a lab panel press moved to (x, y): its knob follows (beyond the panel
    /// too, within the knob's range).
    pub fn lab_drag(&mut self, x: f64, y: f64) {
        let Some(LabDrag::Value(plane)) = self.overlay.lab_drag else {
            return;
        };
        let Some((px, py)) = self.to_pixels(x, y) else { return };
        if let Some(v) = self.overlay.lab_value_at(plane, px, py) {
            self.set_knob(plane, v);
        }
    }

    /// A lab panel press ended: on a "show" button, its plane becomes the main view. True if
    /// the press was a lab panel's.
    pub fn lab_release(&mut self) -> bool {
        match self.overlay.lab_drag.take() {
            Some(LabDrag::Show(plane)) => {
                self.set_lab_plane(plane);
                true
            }
            Some(LabDrag::Value(_)) => true,
            None => false,
        }
    }

    fn set_knob(&mut self, plane: Plane, v: (f64, f64)) {
        if let Some(l) = self.renderer.lab.as_mut() {
            l.values.set(plane, v);
        }
    }

    /// Input in the lab, before the navigator (None: the navigator's as usual). A click in the
    /// main view (⌘ or not) sets the shown plane's knob to the clicked value; the keys of what
    /// the lab lacks (autopilot, journeys, seek) do nothing.
    pub(super) fn lab_input(&mut self, event: &InputEvent) -> Option<InputOutcome> {
        if !self.lab_on() {
            return None;
        }
        match *event {
            InputEvent::Click { x, y, .. } => {
                let (w, h) = self.view_points;
                if !(x.is_finite() && y.is_finite() && w > 0.0 && h > 0.0) {
                    return None;
                }
                let c = self.renderer.viewport.point_at_offset(x, y);
                let plane = self.renderer.lab.as_ref().map(|l| l.plane)?;
                self.set_knob(plane, (c.re(), c.im()));
                Some(InputOutcome {
                    handled: true,
                    redraw: true,
                })
            }
            InputEvent::KeyDown(k) | InputEvent::KeyUp(k)
                if matches!(
                    k,
                    NavKey::Autopilot | NavKey::Journey | NavKey::JourneyAlt | NavKey::Elsewhere | NavKey::SeekMinibrot
                ) =>
            {
                Some(InputOutcome {
                    handled: false,
                    redraw: false,
                })
            }
            _ => None,
        }
    }

    /// After input in the lab: an autopilot or journey that some other control started stops
    /// (the lab has neither; they steer by the escape-time job's surveys).
    pub(super) fn lab_after_input(&mut self, now: f64) {
        if self.lab_on() && matches!(self.renderer.navigator.level(), Level::Autopilot | Level::Journey) {
            self.stop_motion(now);
        }
    }

    /// Esc: the autopilot, a journey or a flight stops.
    fn stop_motion(&mut self, now: f64) {
        let r = &mut self.renderer;
        r.handle_input(InputEvent::KeyDown(NavKey::Stop), now);
        r.handle_input(InputEvent::KeyUp(NavKey::Stop), now);
        r.navigator.cancel_flight();
    }
}

#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use super::*;
    use crate::metal::MetalBackend;
    use crate::scheduler::Renderer;

    fn host() -> Host<MetalBackend> {
        let backend = MetalBackend::new().expect("metal");
        let mut h = Host::new(Renderer::new(backend).expect("renderer"));
        h.resize(800.0, 500.0, 1.0, 0.0);
        h
    }

    fn frames(h: &mut Host<MetalBackend>, from: f64, n: usize) -> f64 {
        let b = h.renderer.backend().clone();
        let target = b
            .create_texture(
                800,
                500,
                crate::backend::TextureFormat::Bgra8Unorm,
                crate::backend::Storage::Shared,
            )
            .expect("target");
        let mut t = from;
        for _ in 0..n {
            let mut acquire = |_: bool| Some((target.clone(), None));
            h.renderer.render_frame(t, 800, 500, 1.0, &mut acquire);
            t += 1.0 / 60.0;
        }
        t
    }

    /// At rest the lab draws its full field, then the 2×2 one, and settles (no more frames);
    /// a knob change starts it again.
    #[test]
    fn the_lab_settles_at_rest() {
        let mut h = host();
        h.overlay.settings.orbit = true;
        h.enter_lab(LabValues::default(), Plane::Z0, 0.0);
        let b = h.renderer.backend().clone();
        let target = b
            .create_texture(
                800,
                500,
                crate::backend::TextureFormat::Bgra8Unorm,
                crate::backend::Storage::Shared,
            )
            .expect("target");
        let mut t = 0.0;
        let mut settled = false;
        // (A deadline, not a count of refreshes: the bands take what the GPU gives them, and
        // on a loaded machine that was more than 240 refreshes of 4 ms.)
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !settled && std::time::Instant::now() < deadline {
            // (Through the host, overlay and panels as in the app.)
            let mut acquire = |_: bool| Some(((target.clone(), None), 0));
            let clock = || 0.0;
            h.frame(t, &clock, true, &mut acquire);
            t += 1.0 / 60.0;
            settled = h.renderer.is_settled();
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        assert!(settled, "the lab did not settle within 30 s");
        h.set_lab_values(LabValues {
            c: (-0.1, 0.7),
            ..LabValues::default()
        });
        frames(&mut h, t, 1);
        assert!(!h.renderer.is_settled());
    }

    /// Into the lab from a tour: the tour ends at once, and the way back is the view before the
    /// tour, not the step's. A tour started in the lab leaves it first.
    #[test]
    fn a_tour_and_the_lab_end_each_other() {
        let mut h = host();
        let before = h.renderer.viewport.clone();
        let tour = almondbread_core::tour::named("feigenbaum").expect("tour");
        h.start_tour(tour.clone(), 0.0);
        h.tour_go(1, 0.5);
        h.renderer.viewport.zoom_by(16.0, 0.0, 0.0);
        assert_ne!(h.renderer.viewport, before);
        assert!(h.enter_lab(LabValues::default(), Plane::C, 1.0));
        assert!(h.tour.is_none());
        h.leave_lab();
        assert_eq!(h.renderer.viewport, before);
        assert!(h.enter_lab(LabValues::default(), Plane::C, 2.0));
        h.start_tour(tour, 2.5);
        assert!(!h.lab_on() && h.tour.is_some());
        assert!(h.end_tour(false, 3.0));
        assert_eq!(h.renderer.viewport, before);
    }

    /// The live journal sees the lab's views; what persists does not (`user_journal`), and a
    /// journal restored while the lab is on is the one that comes back.
    #[test]
    fn the_labs_views_stay_out_of_the_users_journal() {
        const TEXT: &str = "almondbread-journal 1\nat 1\n-0.75 0 0.004 0.0 mandelbrot\n-0.1 0.8 1e-5 0.0 mandelbrot\n";
        let mut h = host();
        assert_eq!(h.user_journal_mut().import(TEXT, 800.0), 2);
        let before = h.user_journal().export();
        assert!(h.enter_lab(LabValues::default(), Plane::C, 0.0));
        let mut t = frames(&mut h, 0.0, 2);
        for scale in [16.0, 16.0] {
            h.renderer.viewport.zoom_by(scale, 0.0, 0.0);
            t = frames(&mut h, t, 2);
            t = frames(&mut h, t + 3.0, 2);
        }
        assert!(
            h.renderer.navigator.journal().entries().len() > 2,
            "no lab view recorded"
        );
        assert_eq!(h.user_journal().export(), before);
        h.leave_lab();
        assert_eq!(h.renderer.navigator.journal().export(), before);

        let mut h = host();
        assert!(h.enter_lab(LabValues::default(), Plane::C, 0.0));
        assert_eq!(h.user_journal_mut().import(TEXT, 800.0), 2);
        h.leave_lab();
        assert_eq!(h.renderer.navigator.journal().entries().len(), 2);
    }

    /// Entering and leaving: the lab's home view, the view from before back, the plane switch
    /// keeps each plane's view; a click sets the shown plane's knob; the autopilot key does
    /// nothing; zoom stops at the precision limit.
    #[test]
    fn enter_switch_click_and_leave() {
        let mut h = host();
        let before = h.renderer.viewport.clone();
        assert!(h.enter_lab(LabValues::default(), Plane::C, 0.0));
        assert!(!h.enter_lab(LabValues::default(), Plane::C, 0.0));
        assert!((h.renderer.viewport.center.re() + 0.5).abs() < 1e-12);
        // A click 100 points right of the centre: c there.
        let c = h.renderer.viewport.point_at_offset(100.0, 0.0);
        let o = h.input(
            InputEvent::Click {
                x: 100.0,
                y: 0.0,
                command: false,
            },
            0.1,
        );
        assert!(o.handled);
        assert!((h.lab_state().unwrap().0.c.0 - c.re()).abs() < 1e-12);
        // Z: no autopilot.
        h.input(InputEvent::KeyDown(NavKey::Autopilot), 0.2);
        h.input(InputEvent::KeyUp(NavKey::Autopilot), 0.2);
        assert_eq!(h.renderer.navigator.level(), Level::Manual);
        // The p plane and back: the c plane's view is kept.
        h.renderer.viewport.zoom_by(4.0, 10.0, 0.0);
        let c_view = h.renderer.viewport.clone();
        assert!(h.set_lab_plane(Plane::P));
        assert!((h.renderer.viewport.center.re() - 2.5).abs() < 1e-12);
        assert!(h.set_lab_plane(Plane::C));
        assert_eq!(h.renderer.viewport, c_view);
        // Zooming far past the limit stops there.
        let t = frames(&mut h, 1.0, 3);
        h.renderer.viewport.zoom_by(1e6, 0.0, 0.0);
        frames(&mut h, t, 2);
        let s = h.renderer.viewport.scale.to_f64();
        let limit =
            almondbread_core::lab::min_pixel((h.renderer.viewport.center.re(), h.renderer.viewport.center.im()));
        assert!((s / limit - 1.0).abs() < 1e-6, "{s} vs {limit}");
        assert!(h.renderer.lab.as_ref().unwrap().at_limit.is_some());
        let (v, p) = h.leave_lab().expect("was on");
        assert_eq!(p, Plane::C);
        assert!((v.c.0 - c.re()).abs() < 1e-12);
        assert_eq!(h.renderer.viewport, before);
        assert!(!h.lab_on());
    }
}
