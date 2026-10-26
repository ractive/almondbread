//! "Raise to …" at compute further's upper limit (kb: techniques/auto-iterations "Raise the
//! upper limit"). User request 2026-10-26: after N landed on a period-30 957 minibrot the whole
//! view was fog at the default 200 k Upper limit ("4.8 % not finished at 200 k steps (upper
//! limit)"); the HUD now offers the next Upper limit choice, and accepting it continues compute
//! further to the new ceiling in the same job. The checks judge state (offer, cap, reached,
//! unresolved, the job kept), not timing: the machine may be loaded.

use almondbread_core::viewport::{Center, Scale, Viewport};

use super::{FurtherStop, RAISE_MIN_SHARE, Renderer};
use crate::backend::{GpuBackend, Storage, TextureFormat};
use crate::metal::MetalBackend;

const W: usize = 400;
const H: usize = 250;

struct Session {
    r: Renderer<MetalBackend>,
    target: <MetalBackend as GpuBackend>::Texture,
}

impl Session {
    /// The -3/4 neck at 4e8 (the regression session's view): 15 % unresolved at 65 k steps,
    /// ~3.8 % at 200 k, slow escapes up to millions of steps.
    fn new(cap: usize) -> Self {
        let backend = MetalBackend::new().expect("metal");
        let target = backend
            .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
            .expect("target");
        let mut r = Renderer::new(backend).expect("renderer");
        r.config.further = true;
        r.config.further_budget = 60.0;
        r.settings.max_iterations_cap = cap;
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

    fn share(&self) -> f64 {
        self.r
            .further_info()
            .map_or(0.0, |f| f.unresolved as f64 / (W * H) as f64)
    }
}

/// At the 50 k choice the neck stops at the upper limit with much of the view unresolved: the
/// offer is the next choice (200 k); accepting it sets the Upper limit, keeps the job (no new
/// render), and compute further continues to 200 k, where fewer pixels are left. There the
/// offer is the next choice again (1 M) while the share is still notable.
#[test]
fn raise_offers_the_next_choice_and_continues_in_the_same_job() {
    let mut s = Session::new(50_000);
    s.settle();
    let f = s.r.further_info().expect("compute further started");
    assert_eq!(f.stop, Some(FurtherStop::Limit), "{f:?}");
    let before = s.share();
    assert!(before >= RAISE_MIN_SHARE, "neck at 50 k: {before}");
    assert_eq!(s.r.raise_offer(), Some(200_000));
    let job = s.r.job_started();
    assert_eq!(s.r.raise_upper_limit(), Some(200_000));
    assert_eq!(s.r.settings.max_iterations_cap, 200_000);
    // Working again at once (the HUD shows "computing further", not the settled line).
    assert!(!s.r.progress_sample(s.r.clock()).settled);
    assert_eq!(s.r.raise_offer(), None, "offered while computing");
    s.settle();
    assert_eq!(s.r.job_started(), job, "the raise rendered the view again");
    let g = s.r.further_info().expect("further");
    assert!(g.reached > f.reached, "{f:?} -> {g:?}");
    assert_eq!(g.reached, 200_000, "{g:?}");
    let after = s.share();
    assert!(after < before, "unresolved {before} -> {after}");
    if g.stop == Some(FurtherStop::Limit) && after >= RAISE_MIN_SHARE {
        assert_eq!(s.r.raise_offer(), Some(1_000_000));
        // iOS: a memory budget whose iteration cap (25 % of the ceiling at 130 B per point, the
        // reference twice the limit) stops below 1 M offers nothing; one that allows 1 M but
        // not 10 M offers 1 M even where the period would ask for more.
        s.r.set_memory_budget(crate::memory::MemoryBudget {
            ceiling: Some(800 << 20),
        });
        assert_eq!(s.r.raise_offer(), None, "800 MB ceiling: the limit stays below 1 M");
        s.r.set_memory_budget(crate::memory::MemoryBudget { ceiling: Some(3 << 30) });
        assert_eq!(s.r.raise_offer(), Some(1_000_000), "3 GB ceiling: 1 M fits, 10 M not");
    }
}

/// Nothing is offered at the top choice (more is never promised there), with a manual limit,
/// or while compute further stopped for another reason.
#[test]
fn no_offer_at_the_top_choice_or_with_a_manual_limit() {
    let mut s = Session::new(50_000);
    s.r.settings.auto_iterations = false;
    s.r.settings.manual_iterations = 50_000;
    s.settle();
    let f = s.r.further_info().expect("compute further started");
    assert_eq!(f.stop, Some(FurtherStop::Limit), "{f:?}");
    assert!(s.share() >= RAISE_MIN_SHARE);
    assert_eq!(s.r.raise_offer(), None, "manual limit");
    assert_eq!(s.r.raise_upper_limit(), None);

    // At the top choice (the view stopped at its limit, the setting already 10 M: nothing
    // above it): no offer.
    s.r.settings.auto_iterations = true;
    assert!(s.r.raise_offer().is_some(), "automatic mode at 50 k");
    s.r.settings.max_iterations_cap = 10_000_000;
    assert_eq!(s.r.raise_offer(), None, "top choice");
}

/// A time-limit stop is Continue's offer, not this one.
#[test]
fn no_offer_at_the_time_limit() {
    let mut s = Session::new(10_000_000);
    s.r.config.further_budget = 0.02;
    s.settle();
    let f = s.r.further_info().expect("compute further started");
    assert_eq!(f.stop, Some(FurtherStop::Budget), "{f:?}");
    assert_eq!(s.r.raise_offer(), None, "{f:?}");
}

/// The user's case: N landed on a period-30 957 minibrot at ~1e-166 and the whole view stayed
/// fog at the default 200 k. The offer skips to the smallest choice of at least 100 P (10 M, not
/// 1 M), and after accepting it the minibrot shows: most of the view resolves.
#[test]
fn after_seek_the_offer_follows_the_minibrots_period() {
    let mut s = Session::new(200_000);
    s.r.viewport = Viewport::with_center(
        Center::parse(
            "-1.25356534474835587527493183735940999452038946488377636763638567580192395042423734111416289926432818195925721840346476271763350256350820210346221813715651311190681110426723575567067949455901507413607124893670181271",
            "0.343076237238640216296082924363370657183074268923787958856435417090662626375457223830906518729830214630275591508889966174381411005560784997998363748427943042064661098977896576691498886101353087867833483948599347664",
        )
        .expect("centre"),
        // (The N framing at 1280 x 800, 2.04384e-166 per point, at 400 x 250.)
        Scale::parse("6.540288e-166").expect("scale"),
    );
    s.settle();
    let f = s.r.further_info().expect("compute further started");
    assert_eq!(f.stop, Some(FurtherStop::Limit), "{f:?}");
    let before = s.share();
    assert!(before > 0.5, "the view is mostly fog at 200 k: {before}");
    assert_eq!(s.r.raise_offer(), Some(10_000_000));
    let job = s.r.job_started();
    assert_eq!(s.r.raise_upper_limit(), Some(10_000_000));
    s.settle();
    assert_eq!(s.r.job_started(), job, "the raise rendered the view again");
    let after = s.share();
    assert!(after < 0.2, "unresolved {before} -> {after}");
    // At the top choice nothing more is offered, whatever is left.
    assert_eq!(s.r.raise_offer(), None);
}
