//! Cost of creating the specialised iterate and seed pipelines (kb: platform/metal-compute
//! "Pipeline warm-up"): `ALMONDBREAD_KERNEL_SALT=x cargo run --release -p almondbread-render
//! --example pipeline_cost`. A new salt changes the iterate kernel's code, so Metal's shader
//! cache misses as after an app update; without one (or with a salt used before) the cache
//! answers.

#[cfg(target_vendor = "apple")]
fn main() {
    use almondbread_render::backend::GpuBackend;
    use almondbread_render::metal::MetalBackend;
    use std::time::Instant;
    let t = Instant::now();
    let b = MetalBackend::new().expect("backend");
    println!("library: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let mut times = Vec::new();
    // de, interior, bla, deep, interior_color, seeded, -, -, la, at
    for (name, c) in [
        (
            "iterate plain",
            [false, true, false, false, false, false, false, false, false, false],
        ),
        (
            "iterate +seeded",
            [false, true, false, false, false, true, false, false, false, false],
        ),
        (
            "iterate +bla",
            [false, true, true, false, false, true, false, false, false, false],
        ),
        (
            "iterate deep",
            [false, true, false, true, false, true, false, false, false, false],
        ),
        (
            "iterate deep+bla",
            [false, true, true, true, false, true, false, false, false, false],
        ),
        (
            "iterate la",
            [false, true, false, false, false, true, false, false, true, false],
        ),
        (
            "iterate deep+la",
            [false, true, false, true, false, true, false, false, true, false],
        ),
        (
            "iterate de",
            [true, true, false, false, false, true, false, false, false, false],
        ),
        (
            "iterate test off",
            [false, false, false, false, false, true, false, false, false, false],
        ),
    ] {
        let t = Instant::now();
        b.pipeline("iterate", &c).expect("pipeline");
        let ms = t.elapsed().as_secs_f64() * 1e3;
        times.push(ms);
        println!("{name}: {ms:.1} ms");
    }
    for (name, c) in [
        ("seed plain", [false, true, false, false, false, false]),
        ("seed deep+bla", [false, true, true, true, false, false]),
    ] {
        let t = Instant::now();
        b.pipeline("seedLevel", &c).expect("pipeline");
        println!("{name}: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(f64::total_cmp);
    println!(
        "iterate median {:.1} ms, max {:.1} ms",
        times[times.len() / 2],
        times[times.len() - 1]
    );
}

#[cfg(not(target_vendor = "apple"))]
fn main() {}
