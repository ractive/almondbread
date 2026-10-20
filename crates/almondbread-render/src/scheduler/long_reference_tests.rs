//! Long references stay off the frame thread (C1/C2 of the 2026-10 review; kb:
//! techniques/hang-prevention "Long references"): a manual limit far above the default makes
//! the f64 orbit on the reference worker, references from the worker arrive with their GPU
//! form, and the table build of a replaced reference is cancelled.

use std::sync::atomic::Ordering;
use std::time::Instant;

use almondbread_core::reference::{Reference, ReferenceRequest};
use almondbread_core::settings::MAX_ITERATION_LIMIT;
use almondbread_core::viewport::Viewport;

use super::{BAILOUT, Renderer};
use crate::backend::{GpuBackend, Storage, TextureFormat};
use crate::metal::MetalBackend;

const W: usize = 320;
const H: usize = 200;

type Texture = <MetalBackend as GpuBackend>::Texture;

fn renderer() -> (Renderer<MetalBackend>, Texture) {
    let mut r = Renderer::new(MetalBackend::new().unwrap()).unwrap();
    // Inside the main cardioid: the centre never escapes, so its orbit runs to the limit.
    r.viewport = Viewport::new(-0.1, 0.1, 1.0 / W as f64);
    let t = r
        .backend
        .create_texture(W, H, TextureFormat::Bgra8Unorm, Storage::Shared)
        .unwrap();
    (r, t)
}

/// One refresh; its wall time in ms.
fn refresh(r: &mut Renderer<MetalBackend>, t: &Texture) -> f64 {
    let start = Instant::now();
    let now = r.clock();
    r.render_frame(now, W, H, 1.0, &mut |_| Some((t.clone(), None)));
    start.elapsed().as_secs_f64() * 1e3
}

/// Refreshes at ~60 Hz until the renderer stops animating (or `timeout` s); returns the
/// longest refresh in ms.
fn settle(r: &mut Renderer<MetalBackend>, t: &Texture, timeout: f64) -> f64 {
    let end = r.clock() + timeout;
    let mut worst: f64 = 0.0;
    loop {
        let now = r.clock();
        let start = Instant::now();
        let o = r.render_frame(now, W, H, 1.0, &mut |_| Some((t.clone(), None)));
        worst = worst.max(start.elapsed().as_secs_f64() * 1e3);
        if !o.animating || r.clock() > end {
            return worst;
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

/// C1: a manual limit of 1e9 at a centre that never escapes. Before, the first refresh ran
/// 1e9 f64 iterations into an 8 GB vector on the frame thread (seconds). Now the limit is
/// capped at `MAX_ITERATION_LIMIT` (M2) and the refresh starts the orbit on the worker and
/// returns; the view waits (history) meanwhile. The capped orbit (~60 ms) may already be in
/// by the last refresh, so only the refresh times are asserted.
#[test]
fn huge_manual_limit_does_not_block_the_frame() {
    let (mut r, t) = renderer();
    // Warm up at the default limit (pipelines, buffers).
    settle(&mut r, &t, 20.0);
    r.settings.auto_iterations = false;
    r.settings.manual_iterations = 1_000_000_000;
    let mut worst: f64 = 0.0;
    for _ in 0..20 {
        worst = worst.max(refresh(&mut r, &t));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    println!("1e9 manual limit: longest refresh {worst:.2} ms while the orbit computes");
    assert!(worst < 20.0, "longest refresh {worst:.1} ms");
    // (Dropping the renderer cancels the orbit: `ReferenceWorker`'s Drop.)
}

/// E2: the bound the scheduler's comment states for the f64 orbit it still computes on the
/// frame thread (`F64_SYNC_POINTS` points at a centre that never escapes).
#[test]
fn f64_orbit_on_the_frame_thread_is_short() {
    use almondbread_core::reference::F64_SYNC_POINTS;
    let v = Viewport::new(-0.1, 0.1, 1e-6);
    let mut best = f64::INFINITY;
    for _ in 0..5 {
        let t0 = Instant::now();
        let r = Reference::compute_f64(&v, F64_SYNC_POINTS, BAILOUT);
        best = best.min(t0.elapsed().as_secs_f64() * 1e3);
        assert_eq!(r.points.len(), F64_SYNC_POINTS + 1);
    }
    println!("f64 orbit of {F64_SYNC_POINTS} points: {best:.2} ms");
    assert!(best < 5.0, "{best:.2} ms");
}

/// C1: a limit far past what the frame thread computes (`MAX_ITERATION_LIMIT` = 2^24 points,
/// ~60 ms of f64 orbit and 134 MB): every refresh stays short, the orbit arrives from the worker and the view
/// settles on it.
#[test]
fn long_f64_orbit_arrives_from_the_worker() {
    let (mut r, t) = renderer();
    settle(&mut r, &t, 20.0);
    r.settings.auto_iterations = false;
    r.settings.manual_iterations = MAX_ITERATION_LIMIT;
    let serial = r.reference_serial();
    // Until the reference is in: refreshes only show the history.
    let mut waiting: f64 = 0.0;
    let end = Instant::now() + std::time::Duration::from_secs(30);
    while r.reference_serial() == serial {
        assert!(Instant::now() < end, "the orbit did not arrive");
        waiting = waiting.max(refresh(&mut r, &t));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let reference = r.reference().expect("reference");
    assert!(!reference.big && reference.points.len() == MAX_ITERATION_LIMIT + 1);
    println!("2^24 manual limit: longest refresh {waiting:.2} ms until the orbit arrived");
    assert!(waiting < 20.0, "longest refresh {waiting:.1} ms");
}

/// C2: a reference from the worker comes with its GPU buffers made on the worker's thread, so
/// taking it costs the frame thread no copy; one made on the frame thread is uploaded there.
#[test]
fn worker_reference_is_uploaded_off_the_frame_thread() {
    let (mut r, _t) = renderer();
    let v = r.viewport.clone();
    let n = 4_000_000;
    r.reference_worker
        .start(ReferenceRequest::f64_orbit(&v, r.settings.formula, n, BAILOUT));
    let worker = r.reference_worker.poll(true).expect("reference");
    assert!(worker.prepared.is_some(), "uploaded on the worker");
    let mut plain = worker.clone();
    plain.prepared = None;
    let t0 = Instant::now();
    assert!(r.set_reference(worker));
    let taken = t0.elapsed().as_secs_f64() * 1e3;
    let t0 = Instant::now();
    assert!(r.set_reference(plain));
    let uploaded = t0.elapsed().as_secs_f64() * 1e3;
    println!("4M-point reference: {taken:.3} ms taken from the worker, {uploaded:.3} ms uploaded here");
    assert!(taken < 1.0, "{taken:.3} ms");
}

/// C2: replacing a reference cancels the BLA table build still running for the old one.
#[test]
fn replaced_reference_cancels_its_table_build() {
    let (mut r, _t) = renderer();
    let v = r.viewport.clone();
    assert!(r.set_reference(Reference::compute_f64_with(&v, r.settings.formula, 2_000_000, BAILOUT)));
    r.start_bla_build();
    let build = r.bla_build.as_ref().expect("an async build (2M points)");
    let flag = build.cancel_flag();
    assert!(!flag.load(Ordering::Relaxed));
    assert!(r.set_reference(Reference::compute_f64_with(&v, r.settings.formula, 1000, BAILOUT)));
    assert!(r.bla_build.is_none());
    assert!(flag.load(Ordering::Relaxed), "the old build was not cancelled");
}
