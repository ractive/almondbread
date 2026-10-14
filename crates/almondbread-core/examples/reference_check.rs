//! Time-to-reference for a jump ("Go to coordinates"): the big reference the scheduler
//! requests for a view with no usable reference, timed as selection (period + Newton) and
//! orbit, plus when the first usable reference was delivered (kb: synthesis/benchmarks R6).
//!
//!   cargo run --release -p almondbread-core --example reference_check -- [--iters N] [--size W]
//!       [--repeat R] [name...]
//!
//! Names are deep benchmark locations or `dendrite@1e-<d>` (c = i at that texel spacing).
//! Same request as the scheduler's `prepare_reference` (centre references to 2x the limit,
//! nucleus search over the view's half-diagonal).

use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::time::Instant;

use almondbread_core::locations::location_named;
use almondbread_core::reference::{Reference, ReferenceRequest};
use almondbread_core::viewport::{Center, Scale, Viewport};

fn view(name: &str, width: usize) -> Viewport {
    if let Some(d) = name.strip_prefix("dendrite@") {
        let s = Scale::parse(d).expect("scale");
        return Viewport::with_center(Center::parse("0", "1").unwrap(), s);
    }
    location_named(name)
        .unwrap_or_else(|| panic!("unknown location {name}"))
        .viewport_for_width(width)
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |flag: &str, default: usize| -> usize {
        if let Some(i) = args.iter().position(|a| a == flag) {
            let v = args[i + 1].parse().expect("number");
            args.drain(i..i + 2);
            v
        } else {
            default
        }
    };
    let max_iter = take("--iters", 200_000);
    let width = take("--size", 640);
    let repeat = take("--repeat", 1).max(1);
    let names: Vec<String> = if args.is_empty() {
        [
            "dendrite@1e-20",
            "dendrite@1e-100",
            "dendrite@1e-300",
            "dendrite@1e-1000",
            "dendrite@1e-2000",
            "needle-1e305",
            "needle-1e1004",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    } else {
        args
    };
    println!("jump reference, {width} texels across, limit {max_iter} (centre orbits to 2x)");
    println!(
        "{:<18} {:>6} {:>9} {:>9} {:>9} {:>9} {:>8}",
        "view", "bits", "select", "orbit", "total", "first", "points"
    );
    for name in &names {
        let v = view(name, width);
        let height = width * 41 / 64;
        let radius = 0.5 * (width as f64).hypot(height as f64);
        for _ in 0..repeat {
            let mut req = ReferenceRequest::for_view(&v, v.scale, width, radius, max_iter * 2, 65536.0);
            req.period_limit = max_iter;
            let t0 = Instant::now();
            let r = Reference::compute_big(&req, &AtomicBool::new(false), &AtomicUsize::new(0)).expect("reference");
            let total = t0.elapsed().as_secs_f64();
            println!(
                "{:<18} {:>6} {:>8.1}ms {:>8.1}ms {:>8.1}ms {:>8.1}ms {:>8}{}",
                name,
                r.precision_bits,
                r.select_time * 1e3,
                r.orbit_time * 1e3,
                r.time() * 1e3,
                total * 1e3,
                r.points.len(),
                r.period.map_or(String::new(), |p| format!(" (period {p})"))
            );
        }
    }
}
