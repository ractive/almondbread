//! Guided tours through a real host on Metal (windowless layer). State-based: loops run until
//! a condition holds (with generous caps), never against the wall clock, so a loaded machine
//! only makes them slower.

use super::*;
use crate::metal::{MetalBackend, MetalLayerSurface};
use almondbread_core::tour::Platform;
use objc2_quartz_core::CAMetalLayer;

const W: f64 = 400.0;
const H: f64 = 250.0;

/// The user's view when "What's the fog?" did nothing (2026-10-26): 3.6e-337 per point.
const RE: &str = "-1.25356534474835587527493183735940999452038946488377636763638567580192395042423734111416289926432818195925721840346476271763350256350820210346221813715651311190681317855773406530396569172420601078221781911615997030054587135617063831007620733225861683734353843533264126876298820821076436958681047710647471004158309200444018932683387362186166762";
const IM: &str = "0.343076237238640216296082924363370657183074268923787958856435417090662626375457223830906518729830214630275591508889966174381411005560784997998363748427943042064661644232910452971500638952887763167151422177556279022114279575979360589306127818429870628189086655163684566521372014205806059531509250460025426531246835854074159683263199350074806172";

fn deep_view() -> Viewport {
    almondbread_core::flyto::view_from_text(RE, IM, "3.59344e-337")
        .expect("view")
        .with_angle(0.3383402526)
}

struct Rig {
    host: Host<MetalBackend>,
    surface: MetalLayerSurface,
    start: std::time::Instant,
    presented: usize,
}

impl Rig {
    fn new() -> Self {
        let backend = MetalBackend::new().expect("metal");
        let mut surface = MetalLayerSurface::new(CAMetalLayer::new(), &backend);
        surface.set_drawable_size((W * 2.0) as usize, (H * 2.0) as usize);
        let mut host = Host::new(Renderer::new(backend).expect("renderer"));
        host.resize(W, H, 2.0, 0.0);
        // Automatic iterations as the user had them, under a low cap: the reference at
        // 1e-337 stays cheap (the picture does not matter here, the path through it does).
        host.renderer.settings.auto_iterations = true;
        host.renderer.settings.max_iterations_cap = 20_000;
        Self {
            host,
            surface,
            start: std::time::Instant::now(),
            presented: 0,
        }
    }

    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn frame(&mut self) {
        let start = self.start;
        let now = move || start.elapsed().as_secs_f64();
        let t = now();
        let ready = self.surface.drawable_ready(t);
        let surface = &mut self.surface;
        let o = self
            .host
            .frame(t, &now, ready, &mut |f| surface.next_drawable(t, f).map(|d| (d, 0)));
        self.presented += usize::from(o.presented);
        std::thread::sleep(std::time::Duration::from_millis(4));
    }

    /// Refreshes until `done` holds (at most `cap`); whether it did.
    fn until(&mut self, cap: usize, done: impl Fn(&Self) -> bool) -> bool {
        for _ in 0..cap {
            if done(self) {
                return true;
            }
            self.frame();
        }
        done(self)
    }

    fn start_limits_tour(&mut self) {
        let tour = almondbread_core::tour::named("limits")
            .expect("limits tour")
            .for_platform(Platform::Mac);
        let t = self.now();
        self.host.start_tour(tour, t);
    }
}

/// "What's the fog?" at 1e-334 (2026-10-26): the limits tour's first step (centre −0.75 +
/// 0.05i, span 0.3, a probe at −0.75 + 0.001i) is minutes away by air at the fly-to speed, so it
/// jumps there at once, and frames are drawn there (before: the tour's probe, 10^336 view
/// widths off, overflowed to inf in the orbit overlay and every refresh panicked before the
/// navigator moved: the view stood still). Its end jumps back to the exact view.
#[test]
fn fog_link_tour_from_far_below_jumps_there_and_back() {
    let mut rig = Rig::new();
    let deep = deep_view();
    rig.host.renderer.viewport = deep.clone();
    assert!(rig.until(2000, |r| r.presented >= 3), "the deep view draws");
    rig.start_limits_tour();
    let h = &rig.host;
    assert!(h.renderer.navigator.flight().is_none(), "no 20-minute flight");
    let v = h.renderer.viewport.clone();
    assert!((v.center.re() + 0.75).abs() < 1e-12 && (v.center.im() - 0.05).abs() < 1e-12);
    assert!((v.scale.to_f64() * W.min(H) - 0.3).abs() < 1e-9, "span 0.3");
    // Frames are drawn there and the view stays put.
    let before = rig.presented;
    assert!(rig.until(3000, |r| r.presented >= before + 20), "the step's view draws");
    assert_eq!(rig.host.renderer.viewport, v);
    // The end: back down to 1e-337 is a jump too, to the exact saved view.
    let t = rig.now();
    assert!(rig.host.end_tour(t));
    assert!(rig.host.renderer.navigator.flight().is_none());
    assert_eq!(rig.host.renderer.viewport, deep);
    let before = rig.presented;
    assert!(
        rig.until(3000, |r| r.presented >= before + 5),
        "the deep view draws again"
    );
}

/// A tour's fixed probe seen from a deep view (the user zooms far in during a step with
/// `probe:`): the orbit overlay works on the deep reference without overflowing, frames go on.
#[test]
fn tour_probe_far_outside_a_deep_view_does_not_stop_the_frames() {
    let mut rig = Rig::new();
    rig.start_limits_tour();
    // A short trip from the home view: flown.
    assert!(rig.host.renderer.navigator.flight().is_some(), "a short step flies");
    assert!(
        rig.until(20_000, |r| r.host.renderer.navigator.flight().is_none()),
        "the flight lands"
    );
    // Deep down while the step's probe stays at −0.75 + 0.001i.
    rig.host.renderer.navigator.jumped();
    rig.host.renderer.viewport = deep_view();
    let t = rig.now();
    rig.host.renderer.note_interaction(t);
    assert!(rig.host.overlay.settings.orbit, "the step shows the orbit");
    assert!(
        rig.until(3000, |r| r.host.renderer.reference().is_some_and(|x| x.big)
            && r.presented >= 10),
        "a deep reference and frames"
    );
    assert!(rig.host.overlay.tour.probe.is_some());
    let before = rig.presented;
    assert!(rig.until(3000, |r| r.presented >= before + 10), "frames go on");
}
