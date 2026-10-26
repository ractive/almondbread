//! Fog takes history (kb: techniques/frame-reuse-and-progressive "Fog takes history"): a
//! gesture frame shows what the rest frame showed where its preview has no result. Previews
//! have no interior probe and no extension, so near-boundary interior texels are unresolved
//! there although the rest frame proved them interior (user report 2026-10-25: dark-grey
//! concentric rings in the black bulbs while panning, black again at rest). And no history
//! result survives a change of the settings that decide the results.

use almondbread_core::formula::Formula;
use almondbread_core::oracle::{ITER_INTERIOR, ITER_UNRESOLVED};
use almondbread_core::settings::InteriorMode;
use almondbread_core::viewport::Viewport;

use super::Renderer;
use crate::backend::{GpuBackend, Storage, TextureFormat};
use crate::metal::MetalBackend;

const W: usize = 480;
const H: usize = 300;

struct Session {
    r: Renderer<MetalBackend>,
    target: <MetalBackend as GpuBackend>::Texture,
}

impl Session {
    /// The period-3 bulb on top of the cardioid with its own bulbs (the user's kind of view).
    fn new(mode: InteriorMode) -> Self {
        let backend = MetalBackend::new().expect("metal");
        let target = backend
            .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
            .expect("target");
        let mut r = Renderer::new(backend).expect("renderer");
        r.settings.interior_mode = mode;
        r.viewport = Viewport::new(-0.1226, 0.7449, 0.3 / W as f64);
        Self { r, target }
    }

    fn frame(&mut self) -> bool {
        let now = self.r.clock();
        let t = self.target.clone();
        self.r
            .render_frame(now, W, H, 1.0, &mut |_| Some((t.clone(), None)))
            .animating
    }

    fn settle(&mut self) {
        let end = self.r.clock() + 30.0;
        while self.frame() {
            assert!(self.r.clock() < end, "did not settle");
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        self.r.wait_frames();
        self.r.drain_in_flight();
    }

    /// One gesture frame panned by a whole pixel; whether it is a gesture frame whose level
    /// reached the limit everywhere (under GPU load a preview is shown before its last pixels
    /// get there; those are not judged).
    fn pan(&mut self) -> bool {
        let now = self.r.clock();
        self.r.navigator.note_interaction(now);
        self.r.viewport.pan_by_points(1.0, 0.0);
        self.frame();
        self.r.wait_frames();
        self.r.drain_in_flight();
        let job = self.r.job.as_ref().expect("job");
        let level = &job.levels[job.display()];
        job.interactive && (level.live_min_n == u32::MAX || level.live_min_n >= level.params.max_iter)
    }

    /// The shown level's texels (the rest frame's full level).
    fn texels(&mut self) -> Vec<[f32; 2]> {
        let job = self.r.job.as_ref().expect("job");
        let set = job.levels[job.display()].set.clone();
        self.r.read_iterations(&set).expect("read")
    }

    /// The class epochs of the latest history image.
    fn classes(&mut self) -> Vec<u8> {
        let class = self.r.history_class.clone().expect("history class");
        let mut out = vec![0u8; W * H];
        self.r.backend.read_buffer(&class[self.r.history_ping], 0, &mut out);
        out
    }
}

/// Interior texels of the rest frame at least 2 pixels from any other texel: the bulb
/// interiors, not the edges the temporal upsampling resamples.
fn deep_interior(t: &[[f32; 2]]) -> Vec<bool> {
    let interior = |x: i64, y: i64| {
        x < 0 || y < 0 || x >= W as i64 || y >= H as i64 || t[y as usize * W + x as usize][0] == ITER_INTERIOR
    };
    (0..W * H)
        .map(|i| {
            let (x, y) = ((i % W) as i64, (i / W) as i64);
            (-2..=2).all(|j| (-2..=2).all(|k| interior(x + k, y + j)))
        })
        .collect()
}

/// Share of the rest frame's deep-interior pixels (shifted by the pan) that a gesture frame
/// draws differently (more than 8 levels in a channel: the fog is dark, the dither is 1), over
/// the gesture frames at the limit.
fn worst_interior_change(mode: InteriorMode) -> (f64, usize) {
    let mut s = Session::new(mode);
    s.settle();
    let mut mask = deep_interior(&s.texels());
    let rest_view = s.r.viewport.clone();
    let (rest, _, _) = s.r.capture_shown().expect("rest frame");
    // Smooth there (3 x 3 within 8 levels): not the nuclei, whose sharp spots the temporal
    // upsampling resamples.
    for (i, m) in mask.iter_mut().enumerate() {
        let (x, y) = (i % W, i / W);
        if x == 0 || y == 0 || x == W - 1 || y == H - 1 {
            *m = false;
            continue;
        }
        *m = *m
            && (0..3).all(|c| {
                let v = (0..9).map(|k| rest[4 * ((y + k / 3 - 1) * W + x + k % 3 - 1) + c]);
                v.clone().max().unwrap_or(0) - v.min().unwrap_or(0) <= 8
            });
    }
    let mut worst = 0.0f64;
    let mut seen = 0;
    for _ in 0..40 {
        if !s.pan() {
            continue;
        }
        let (shown, _, _) = s.r.capture_shown().expect("gesture frame");
        let (dx, dy) = s.r.viewport.center.delta_in(&rest_view.center, rest_view.scale);
        let (sx, sy) = (dx.round() as i64, -dy.round() as i64);
        let (mut n, mut changed) = (0usize, 0usize);
        for y in 0..H as i64 {
            for x in 0..W as i64 {
                let (rx, ry) = (x + sx, y + sy);
                if rx < 0 || ry < 0 || rx >= W as i64 || ry >= H as i64 || !mask[ry as usize * W + rx as usize] {
                    continue;
                }
                n += 1;
                let (a, b) = (4 * (y as usize * W + x as usize), 4 * (ry as usize * W + rx as usize));
                if (0..3).any(|c| shown[a + c].abs_diff(rest[b + c]) > 8) {
                    changed += 1;
                }
            }
        }
        assert!(n > W * H / 10, "the bulb left the view");
        seen += 1;
        worst = worst.max(changed as f64 / n as f64);
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    (worst, seen)
}

#[test]
fn gesture_frames_keep_the_rest_frames_flat_interior() {
    let (worst, seen) = worst_interior_change(InteriorMode::Flat);
    assert!(seen >= 10, "only {seen} gesture frames at the limit");
    // (Before: up to 25 % of the deep interior drawn as fog rings; with it 0.001 %.)
    assert!(
        worst < 0.005,
        "gesture frames change up to {:.2} % of the rest frame's interior",
        100.0 * worst
    );
}

/// The analysed interiors: the preview's provisional cycle analysis fails on some rim texels
/// (fog) and finds another period on others (period: a 2-4 px ring of another colour).
#[test]
fn gesture_frames_keep_the_rest_frames_interior_analysis() {
    for mode in [InteriorMode::Bubbles, InteriorMode::Period] {
        let (worst, seen) = worst_interior_change(mode);
        assert!(seen >= 10, "{mode:?}: only {seen} gesture frames at the limit");
        // (Before: bubbles 1.4 %, period 3.9 %; with it 0.)
        assert!(
            worst < 0.005,
            "{mode:?}: gesture frames change up to {:.2} % of the rest frame's interior",
            100.0 * worst
        );
    }
}

/// A change of the settings that decide the results right after a rest frame, then a pan: no
/// history pixel keeps a class of the old results (it would stand in for the new preview's
/// unresolved texels), so gesture frames mark none known until an exact level composes.
#[test]
fn settings_changes_forget_the_history_results() {
    type Change = fn(&mut Renderer<MetalBackend>);
    let changes: [(&str, Change); 4] = [
        ("limit down", |r| {
            r.settings.auto_iterations = false;
            r.settings.manual_iterations = 100;
        }),
        ("limit up", |r| {
            r.settings.auto_iterations = false;
            r.settings.manual_iterations = 20_000;
        }),
        ("formula", |r| r.settings.formula = Formula::Multibrot(3)),
        ("interior mode", |r| r.settings.interior_mode = InteriorMode::Bubbles),
    ];
    for (name, change) in changes {
        let mut s = Session::new(InteriorMode::Flat);
        s.settle();
        let known = |c: &[u8], e: u32| c.iter().filter(|&&v| u32::from(v) == e).count();
        // Unchanged settings: a pan keeps the rest frame's results known.
        let epoch = s.r.class_epoch;
        let rest = known(&s.classes(), epoch);
        assert!(rest == W * H, "{name}: rest frame marks {rest} of {} pixels", W * H);
        s.pan();
        let kept = known(&s.classes(), epoch);
        assert!(
            kept > W * H * 9 / 10,
            "{name}: a pan kept {kept} of {} pixels known",
            W * H
        );
        s.settle();
        change(&mut s.r);
        for k in 0..4 {
            s.pan();
            let classes = s.classes();
            let stale = classes.iter().filter(|&&v| v != 0).count();
            assert!(s.r.class_epoch != epoch, "{name}: same class epoch after the change");
            assert_eq!(
                stale, 0,
                "{name}: gesture frame {k} after the change keeps {stale} pixels known"
            );
        }
    }
}

/// Unresolved at rest stays what the rest frame showed: texels the rest frame left
/// unresolved (too slow even with the extension) are known results too.
#[test]
fn unresolved_at_rest_counts_as_known() {
    let mut s = Session::new(InteriorMode::Flat);
    s.settle();
    let t = s.texels();
    let classes = s.classes();
    let epoch = s.r.class_epoch;
    let unresolved: Vec<usize> = (0..W * H).filter(|&i| t[i][0] == ITER_UNRESOLVED).collect();
    assert!(
        !unresolved.is_empty(),
        "no unresolved texels at rest: pick another view"
    );
    assert!(
        unresolved.iter().all(|&i| u32::from(classes[i]) == epoch),
        "unresolved rest texels not marked known"
    );
}
