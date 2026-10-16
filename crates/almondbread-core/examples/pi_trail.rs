//! The π meter's orbit trail as PNGs (kb: techniques/pi-in-the-mandelbrot-set "The orbit"):
//! the picture the counting thread publishes, at several ε at the neck and the cusp, and the
//! counting cost with and without the trail.
//!
//! cargo run --release -p almondbread-core --example pi_trail -- outdir

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use almondbread_core::image::write_png_bgra;
use almondbread_core::pi_moment::{PiPoint, Trail, TrailLayout, escape_count, escape_count_traced};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "review/pi-trail".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let layout = TrailLayout {
        orbit: [340, 300],
        turn: 240,
        gap: 24,
    };
    for (point, list) in [
        (PiPoint::Neck, [1e-2, 1e-4, 1e-6, 1e-8]),
        (PiPoint::Cusp, [1e-4, 1e-8, 1e-12, 1e-16]),
    ] {
        for eps in list {
            let slot = Arc::new(Mutex::new(None));
            let mut trail = Trail::new(point, eps, layout, slot.clone());
            let (c, p) = (AtomicBool::new(false), AtomicU64::new(0));
            let t0 = Instant::now();
            let n = escape_count_traced(point, eps, 4_000_000_000, &c, &p, Some(&mut trail));
            let traced = t0.elapsed().as_secs_f64();
            let t0 = Instant::now();
            let n2 = escape_count(point, eps, 4_000_000_000, &c, &p);
            let plain = t0.elapsed().as_secs_f64();
            assert_eq!(n, n2);
            let img = slot.lock().unwrap().take().expect("published");
            let (w, h) = layout.size();
            let path = format!("{out}/{}_{eps:e}.png", point.name());
            write_png_bgra(&path, &img.bgra, w, h).expect("png");
            let t0 = Instant::now();
            let _ = trail.render();
            let render = t0.elapsed().as_secs_f64();
            println!(
                "{} eps {eps:e}: {n:?}, stride {}, samples {}, counted {plain:.3} s plain, {traced:.3} s traced, picture {:.2} ms -> {path}",
                point.name(),
                trail.stride,
                img.samples,
                render * 1000.0
            );
        }
    }
}
