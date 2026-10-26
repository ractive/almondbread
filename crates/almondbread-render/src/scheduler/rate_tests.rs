//! The throughput model (`Renderer::rate`, `learned_rate`) through a deep view with long level
//! tails: the N view (a period-30 957 minibrot at ~1e-166) at a 10 M cap runs three levels whose
//! tails park ~110 k pixels for the interior probe. Learned from lanes x K, the tails' nearly
//! empty chunks drove the rate x4 per chunk to ~50x the job's mean throughput (2e11 vs ~4e9
//! on an M1 Pro; now ~2x), and the next level's first chunks ran 150-470 ms instead of 16. The check
//! compares the highest rate of the session with the job's own measured loop steps per GPU
//! second (a ratio, not a time: the machine may be loaded).

use almondbread_core::viewport::{Center, Scale, Viewport};

use super::{RATE_COLD, Renderer};
use crate::backend::{GpuBackend, Storage, TextureFormat};
use crate::metal::MetalBackend;

// (The N framing of the report: 1280 x 800 at 2.04384e-166 per point.)
const W: usize = 1280;
const H: usize = 800;

#[test]
fn the_rate_stays_near_the_measured_throughput_through_level_tails() {
    let backend = MetalBackend::new().expect("metal");
    let target = backend
        .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
        .expect("target");
    let mut r = Renderer::new(backend).expect("renderer");
    r.config.further = false;
    // (The CLI's extension budget: the passes x4 and x4 again always fit, whatever else the GPU
    // runs, so the session has level starts after tails.)
    r.config.extension_budget = 120.0;
    r.settings.max_iterations_cap = 10_000_000;
    r.viewport = Viewport::with_center(
        Center::parse(
            "-1.25356534474835587527493183735940999452038946488377636763638567580192395042423734111416289926432818195925721840346476271763350256350820210346221813715651311190681110426723575567067949455901507413607124893670181271",
            "0.343076237238640216296082924363370657183074268923787958856435417090662626375457223830906518729830214630275591508889966174381411005560784997998363748427943042064661098977896576691498886101353087867833483948599347664",
        )
        .expect("centre"),
        Scale::parse("2.04384e-166").expect("scale"),
    );
    let end = r.clock() + 300.0;
    // (Sampled after every frame: a level's first chunks are sized from what the frame before
    // harvested.)
    let mut peak: f64 = 0.0;
    loop {
        let now = r.clock();
        let t = target.clone();
        let animating = r
            .render_frame(now, W, H, 1.0, &mut |_| Some((t.clone(), None)))
            .animating;
        peak = peak.max(r.rate[1]);
        if !animating {
            break;
        }
        assert!(r.clock() < end, "did not settle");
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    let job = r.job.as_ref().expect("job");
    assert!(job.deep, "the N view runs the deep kernel");
    let levels = job.levels.iter().filter(|l| !l.preview && l.aa.is_none()).count();
    assert!(levels >= 3, "levels: {levels}");
    // (Alone ~2x the mean, before ~50x. A GPU shared with other tests stretches every chunk's
    // time and reads the mean as low as 1e9, so the bound is on the larger of the mean and the
    // cold deep rate.)
    let mean = job.steps / job.gpu_time;
    assert!(
        peak <= 8.0 * mean.max(RATE_COLD[1]),
        "peak rate {peak:.3e} vs the job's mean {mean:.3e} loop steps per GPU second ({levels} levels)"
    );
    // (A deep job leaves the plain kernel's rate alone.)
    assert_eq!(r.rate[0], RATE_COLD[0]);
}
