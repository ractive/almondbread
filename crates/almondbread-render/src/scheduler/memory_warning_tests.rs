//! A memory warning while a view is still working (kb: platform/ios-device). The AA list marks
//! the texels it skips in the preview allocation only, so the warning dropping that allocation
//! between two samples made the next sample shade a new, uninitialised texture; and since the
//! job's levels held the old one, nothing was freed either.

use almondbread_core::settings::{RenderSettings, Supersampling};
use almondbread_core::viewport::Viewport;
use objc2::rc::Weak;
use objc2::runtime::ProtocolObject;
use objc2_metal::MTLTexture;

use super::{Job, Renderer};
use crate::backend::{GpuBackend, Storage, TextureFormat};
use crate::metal::MetalBackend;

const W: usize = 320;
const H: usize = 200;

/// What a warning reports: plenty left, so only the release itself matters.
const AVAILABLE: u64 = 1 << 30;
/// MSL `ITER_SKIP`: the list pass's mark on a texel that takes no samples.
const ITER_SKIP: f32 = -4.0;

type Texture = <MetalBackend as GpuBackend>::Texture;

struct Session {
    r: Renderer<MetalBackend>,
    target: Texture,
}

impl Session {
    /// The main cardioid's edge: flat interior and exterior the AA list skips, and a boundary it
    /// takes. `factor`: the AA setting (1: no samples).
    fn new(factor: usize) -> Self {
        Self::at(Viewport::new(-0.45, 0.25, 1.3 / W as f64), factor)
    }

    fn at(view: Viewport, factor: usize) -> Self {
        let backend = MetalBackend::new().expect("Metal");
        let target = backend
            .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
            .expect("target");
        let mut r = Renderer::new(backend).expect("renderer");
        r.settings = RenderSettings::plain();
        r.settings.supersampling = Supersampling::from_factor(factor).expect("factor");
        r.config.further = false;
        r.viewport = view;
        Self { r, target }
    }

    fn frame(&mut self) -> bool {
        let now = self.r.clock();
        let t = self.target.clone();
        self.r
            .render_frame(now, W, H, 1.0, &mut |_| Some((t.clone(), None)))
            .animating
    }

    /// Refreshes until `reached` holds of the job.
    fn until(&mut self, reached: impl Fn(&Job<MetalBackend>) -> bool) {
        let end = self.r.clock() + 60.0;
        while !self.r.job.as_ref().is_some_and(&reached) {
            assert!(self.r.clock() < end, "the state never came");
            self.frame();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// Refreshes until the renderer reports idle; the pixels shown.
    fn settle(&mut self) -> Vec<u8> {
        let end = self.r.clock() + 120.0;
        while self.frame() {
            assert!(self.r.clock() < end, "did not settle");
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        self.r.wait_frames();
        self.r.drain_in_flight();
        let mut px = vec![0u8; W * H * 4];
        self.r
            .backend
            .read_texture(&self.target, W, H, TextureFormat::Bgra8Unorm, &mut px)
            .expect("read");
        px
    }

    /// A handle on the preview allocation that does not keep it alive.
    fn allocation(&self) -> Weak<ProtocolObject<dyn MTLTexture>> {
        Weak::from_retained(&self.r.preview_alloc.as_ref().expect("preview allocation").iter)
    }
}

fn differs(a: &[u8], b: &[u8]) -> bool {
    a.iter().zip(b).take(3).any(|(p, q)| p.abs_diff(*q) > 8)
}

/// A sample running, one composed: the warning ends the samples. Every pixel off the list stays
/// the colour it had without AA (before: they all went towards a wrong colour), the allocation
/// really goes (before: the job's levels kept it), and the view settles.
#[test]
fn warning_between_samples_keeps_the_image_and_frees_the_allocation() {
    let plain = Session::new(1).settle();
    let mut s = Session::new(3);
    s.until(|j| j.aa_composed >= 1 && j.aa_pending());
    let job = s.r.job.as_ref().expect("job");
    let composed = job.aa_composed;
    let list = job.aa_list.as_ref().expect("list");
    let marks = s.r.read_iterations(&list.set).expect("marks");
    let allocation = s.allocation();
    s.r.set_memory_available(AVAILABLE, true);
    let shown = s.settle();

    let off_list: Vec<usize> = (0..W * H).filter(|&i| marks[i][0] == ITER_SKIP).collect();
    let moved = off_list
        .iter()
        .filter(|&&i| differs(&plain[4 * i..4 * i + 4], &shown[4 * i..4 * i + 4]))
        .count();
    println!("{} pixels off the list, {moved} moved", off_list.len());
    assert!(off_list.len() > W * H / 2, "{}", off_list.len());
    assert_eq!(moved, 0, "of {} pixels off the list", off_list.len());

    let job = s.r.job.as_ref().expect("job");
    assert_eq!(job.aa_composed, composed, "samples went on after the warning");
    assert!(s.r.preview_alloc.is_none() && s.r.aa_list_buffer.is_none());
    assert!(allocation.load().is_none(), "a level still holds the allocation");
}

/// While the view refines, its preview is on screen: the warning waits for the full level, which
/// comes out as without it, and takes no samples. (Its image is not compared: a view that had a
/// preview keeps some of it in the fade, warning or not.)
#[test]
fn warning_while_refining_waits_for_the_preview() {
    let mut plain = Session::new(1);
    plain.settle();
    let job = plain.r.job.as_ref().expect("job");
    let full = job.levels.iter().find(|l| !l.preview).expect("full level");
    let exact = plain.r.read_iterations(&full.set).expect("field");

    let mut s = Session::new(3);
    // (A preview level first: the full level is expected to take long.)
    s.r.cost_per_texel = 1e-3;
    s.until(|j| j.levels.first().is_some_and(|l| l.preview));
    assert!(!s.r.job.as_ref().expect("job").complete_presented);
    let allocation = s.allocation();
    s.r.set_memory_available(AVAILABLE, true);
    assert!(s.r.preview_alloc.is_some(), "the preview is in use");
    s.settle();

    let job = s.r.job.as_ref().expect("job");
    assert!(job.levels.iter().all(|l| l.aa.is_none()), "a sample ran");
    let full = job.levels.iter().find(|l| !l.preview).expect("full level");
    assert_eq!(s.r.read_iterations(&full.set).expect("field"), exact);
    assert!(s.r.preview_alloc.is_none());
    assert!(allocation.load().is_none(), "a level still holds the allocation");
}

/// Compute further waits for the AA samples: samples ended by a warning must let it go on.
#[test]
fn warning_lets_compute_further_start() {
    // The -3/4 neck (further_continue_tests): unresolved pixels, slow escapes.
    let mut s = Session::at(
        Viewport::new(-0.749_999_947_197_051_2, 0.00036918485351065104, 1.79254e-11),
        3,
    );
    s.r.config.further = true;
    s.r.config.further_budget = 0.5;
    s.until(|j| j.further.as_ref().is_some_and(|f| f.deferred));
    s.r.set_memory_available(AVAILABLE, true);
    s.settle();
    let f = s.r.job.as_ref().and_then(|j| j.further.as_ref()).expect("further");
    assert!(f.passes > 0, "compute further never started");
    assert!(!f.deferred);
}
