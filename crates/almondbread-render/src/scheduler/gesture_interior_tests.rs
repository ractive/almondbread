//! Interior colourings in gesture frames (kb: techniques/coloring-methods "Gesture frames"):
//! a gesture frame shows the same interior as the rest frame. Gesture levels have no interior
//! probe and no extension past the limit, so the near-boundary rings of the bulbs the
//! closed-form test does not cover were unresolved (fog) while panning and bubbles at rest
//! (user report 2026-10-24: the bulbs' rings "recalculated" during pans).

use almondbread_core::oracle::{ITER_INTERIOR, ITER_UNRESOLVED};
use almondbread_core::settings::InteriorMode;
use almondbread_core::viewport::Viewport;

use super::Renderer;
use crate::backend::{GpuBackend, Storage, TextureFormat};
use crate::metal::MetalBackend;

const W: usize = 480;
const H: usize = 300;

/// The texels of the level the view shows, and whether it is a gesture frame's.
fn shown(r: &mut Renderer<MetalBackend>) -> (Vec<[f32; 2]>, bool) {
    r.wait_frames();
    r.drain_in_flight();
    let job = r.job.as_ref().expect("job");
    let level = &job.levels[job.display()];
    let (set, gesture) = (level.set.clone(), job.interactive);
    (r.read_iterations(&set).expect("read"), gesture)
}

/// (unresolved, interior with a period) shares of the texels.
fn shares(t: &[[f32; 2]]) -> (f64, f64) {
    let n = t.len() as f64;
    let unresolved = t.iter().filter(|x| x[0] == ITER_UNRESOLVED).count() as f64;
    let analysed = t.iter().filter(|x| x[0] == ITER_INTERIOR && x[1] >= 1.0).count() as f64;
    (unresolved / n, analysed / n)
}

#[test]
fn gesture_frames_analyse_the_interior_like_the_rest_frame() {
    let backend = MetalBackend::new().expect("metal");
    let target = backend
        .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
        .expect("target");
    let mut r = Renderer::new(backend).expect("renderer");
    r.settings.interior_mode = InteriorMode::Bubbles;
    // The period-3 bulb on top of the cardioid and its own bulbs (no closed-form test there).
    r.viewport = Viewport::new(-0.1226, 0.7449, 0.3 / W as f64);
    let frame = |r: &mut Renderer<MetalBackend>| {
        let now = r.clock();
        r.render_frame(now, W, H, 1.0, &mut |_| Some((target.clone(), None)))
    };
    let end = r.clock() + 30.0;
    while frame(&mut r).animating {
        assert!(r.clock() < end, "did not settle");
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    let (rest, gesture) = shown(&mut r);
    assert!(!gesture);
    let (rest_unresolved, rest_analysed) = shares(&rest);
    // A slow pan: every refresh is a gesture frame.
    let mut worst = 0.0f64;
    let mut seen = 0;
    for _ in 0..40 {
        let now = r.clock();
        r.navigator.note_interaction(now);
        r.viewport.pan_by_points(0.5, 0.0);
        frame(&mut r);
        let (t, gesture) = shown(&mut r);
        if !gesture {
            continue;
        }
        seen += 1;
        let (unresolved, analysed) = shares(&t);
        // (Before the fix: 37 % vs 52 %; a few rim texels still fail within the step limit.)
        assert!(
            (analysed - rest_analysed).abs() < 0.02,
            "gesture frame: {:.2} % analysed interior vs {:.2} % at rest",
            100.0 * analysed,
            100.0 * rest_analysed
        );
        worst = worst.max(unresolved);
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    assert!(seen >= 10, "only {seen} gesture frames");
    assert!(
        worst < rest_unresolved + 0.01,
        "gesture frames up to {:.2} % unresolved vs {:.2} % at rest",
        100.0 * worst,
        100.0 * rest_unresolved
    );
}
