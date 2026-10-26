//! "Continue" at compute further's time limit (kb: techniques/auto-iterations "Compute
//! further"). User report 2026-10-26: "14 % not finished at 5.2 M steps (time limit)
//! [Continue]", and Continue did nothing: the budget was a worst-case estimate (every pixel left
//! to the next limit), so another 60 s still did not "fit" a pass, `continue_further` stopped
//! again at once and the HUD kept offering the button. The time limit is now measured (GPU
//! seconds spent, checked between segments), so Continue always buys at least a segment.
//! The checks judge state (passes, reached, unresolved), not timing: the machine may be loaded.

use almondbread_core::viewport::Viewport;

use super::{FurtherStop, Renderer};
use crate::backend::{GpuBackend, Storage, TextureFormat};
use crate::metal::MetalBackend;

const W: usize = 400;
const H: usize = 250;

struct Session {
    r: Renderer<MetalBackend>,
    target: <MetalBackend as GpuBackend>::Texture,
}

impl Session {
    /// The -3/4 neck at 4e8 (the regression session's view): 15 % unresolved at the base
    /// limit, slow escapes up to millions of steps.
    fn new(budget: f64) -> Self {
        let backend = MetalBackend::new().expect("metal");
        let target = backend
            .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
            .expect("target");
        let mut r = Renderer::new(backend).expect("renderer");
        r.config.further = true;
        r.config.further_budget = budget;
        r.settings.max_iterations_cap = 10_000_000;
        r.viewport = Viewport::new(-0.749_999_947_197_051_2, 0.00036918485351065104, 1.79254e-11);
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
        let end = self.r.clock() + 300.0;
        while self.frame() {
            assert!(self.r.clock() < end, "did not settle");
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
    }
}

/// A budget the view's own extension passes already use up: compute further stops at the time
/// limit before its first pass (the user's state: the next pass's worst case is far above any
/// budget). Each Continue resumes and makes progress, the HUD shows it working at once, and
/// "time limit" (the HUD's offer) is only ever shown when Continue resumes.
#[test]
fn continue_makes_progress_at_the_time_limit() {
    let mut s = Session::new(0.02);
    s.settle();
    let f = s.r.further_info().expect("compute further started");
    assert_eq!(f.stop, Some(FurtherStop::Budget), "{f:?}");
    let mut last = f;
    for k in 0..3 {
        assert!(s.r.continue_further(), "Continue {k} did not resume: {last:?}");
        let p = s.r.progress_sample(s.r.clock());
        assert!(!p.settled, "Continue {k}: the HUD still shows the settled line");
        s.settle();
        let f = s.r.further_info().expect("further");
        assert!(
            f.reached > last.reached || f.unresolved < last.unresolved,
            "Continue {k} made no progress: {last:?} -> {f:?}"
        );
        assert!(f.gpu > last.gpu, "Continue {k} ran nothing: {last:?} -> {f:?}");
        if f.stop != Some(FurtherStop::Budget) {
            // Finished, or the cap: nothing to continue, and Continue says so.
            assert!(!s.r.continue_further());
            return;
        }
        last = f;
    }
}
