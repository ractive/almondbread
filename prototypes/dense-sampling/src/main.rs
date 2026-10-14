//! R3 prototype (kb: techniques/adaptive-sampling, synthesis/dense-region-sampling): how many
//! pixel-iterations can dense boundary regions save for the same perceived image?
//!
//! `dense-sampling run --loc NAME [--size 800x500] [--frames 12] [--out DIR]` computes a 5 x 5
//! lattice per output pixel over the view and a scripted pan (1.2, 0.6 px per frame), shades
//! every sample with the default look (CPU port of the kernel's `shade`), and evaluates the
//! strategies against the converged reference (box mean of the 25 samples): iterations,
//! samples, OKLab ΔE, flicker. All numbers are deterministic (no timing).

mod branch;
mod lattice;
mod metrics;
mod quad;
mod sa;
mod sample;
mod shade;
mod sparse;
mod strategies;
mod variance;

use std::path::{Path, PathBuf};

use almondbread_core::locations::BenchmarkLocation;
use almondbread_core::settings::depth_iterations;
use almondbread_core::viewport::Viewport;

use lattice::{Lattice, View};
use metrics::{diff, errors, flicker, masked, write_heat, write_mask, write_png};
use sample::{Class, EXPAND};
use shade::Shader;
use sparse::SparseParams;
use strategies::{Out, TextureParams};

static SA_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct Loc {
    name: String,
    re: f64,
    im: f64,
    /// Complex width of the 1600-px framing (the benchmark convention).
    width: f64,
    max_iter: u32,
}

fn locate(name: &str) -> Loc {
    let extra: &[(&str, f64, f64, f64)] = &[
        // FL's filament cruise (kb: synthesis/benchmarks FL), zoom ~1e6.
        ("filament-1e6", -1.25066, 0.02012, 6e-6),
        // The dense view's neighbourhood 100x shallower: many minibrots of the period-7 cascade.
        ("minibrots-5e-12", -1.4080321340304542, 0.13680236432900417, 7.384e-9),
    ];
    if let Some(&(n, re, im, width)) = extra.iter().find(|e| e.0 == name) {
        let v = Viewport::new(re, im, width / 1600.0);
        return Loc {
            name: n.into(),
            re,
            im,
            width,
            max_iter: depth_iterations(&v) as u32,
        };
    }
    let b: &BenchmarkLocation = BenchmarkLocation::named(name).unwrap_or_else(|| panic!("unknown location {name}"));
    Loc {
        name: b.name.into(),
        re: b.center_re,
        im: b.center_im,
        width: b.width,
        max_iter: b.max_iterations as u32,
    }
}

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("probe") {
        probe();
        return;
    }
    if args.get(1).map(String::as_str) != Some("run") {
        eprintln!("usage: dense-sampling run --loc NAME [--size WxH] [--frames N] [--out DIR] [--no-motion]");
        std::process::exit(2);
    }
    let loc = locate(arg(&args, "--loc").unwrap_or("dense-5e-14"));
    let (w, h) = arg(&args, "--size")
        .map(|s| {
            let v: Vec<usize> = s.split('x').map(|p| p.parse().unwrap()).collect();
            (v[0], v[1])
        })
        .unwrap_or((800, 500));
    let frames: usize = arg(&args, "--frames").map(|s| s.parse().unwrap()).unwrap_or(12);
    let out = PathBuf::from(arg(&args, "--out").unwrap_or("out")).join(&loc.name);
    std::fs::create_dir_all(&out).unwrap();
    let motion = !args.iter().any(|a| a == "--no-motion");
    // --sa-only: skip the other strategies (quick runs).
    SA_ONLY.store(
        args.iter().any(|a| a == "--sa-only"),
        std::sync::atomic::Ordering::Relaxed,
    );
    run(&loc, w, h, frames, &out, motion);
}

fn rel(o: &Out, base: u64) -> String {
    format!("{:7.3}", o.iters as f64 / base as f64)
}

fn run(loc: &Loc, w: usize, h: usize, frames: usize, out: &Path, motion: bool) {
    let px = loc.width / 1600.0;
    // Unresolved pixels get extension passes in the app (x4 the limit, kb: synthesis/benchmarks);
    // the samples here run to 4x the nominal limit directly.
    let view = View {
        re: loc.re,
        im: loc.im,
        px,
        w,
        h,
        max_iter: loc.max_iter * 4,
    };
    let shader = Shader::default_look();
    let t0 = std::time::Instant::now();
    let l = Lattice::compute(view, frames, &shader);
    println!(
        "# {}  centre {} {}  px {:.4e}  {}x{}  maxIter {}  lattice {}x{} ({:.1} M samples, {:.1} s CPU wall)",
        loc.name,
        loc.re,
        loc.im,
        px,
        w,
        h,
        loc.max_iter,
        l.lw,
        l.lh,
        l.samples.len() as f64 / 1e6,
        t0.elapsed().as_secs_f64()
    );

    // ---- Where the iterations go (centre samples, frame 0).
    let n_px = w * h;
    let cs: Vec<&sample::Sample> = (0..n_px).map(|p| &l.samples[l.centre(0, p % w, p / w)]).collect();
    let total: u64 = cs.iter().map(|s| s.n as u64).sum();
    let bucket = |f: &dyn Fn(&sample::Sample) -> bool| -> (f64, f64) {
        let sel: Vec<_> = cs.iter().filter(|s| f(s)).collect();
        (
            100.0 * sel.len() as f64 / n_px as f64,
            100.0 * sel.iter().map(|s| s.n as u64).sum::<u64>() as f64 / total as f64,
        )
    };
    println!(
        "1 spp: {:.3e} iterations, {:.0} per pixel",
        total as f64,
        total as f64 / n_px as f64
    );
    for (name, f) in [
        (
            "exterior DE < 0.25 px",
            Box::new(|s: &sample::Sample| s.class == Class::Exterior && s.de < 0.25)
                as Box<dyn Fn(&sample::Sample) -> bool>,
        ),
        (
            "exterior DE 0.25-1 px",
            Box::new(|s: &sample::Sample| s.class == Class::Exterior && s.de >= 0.25 && s.de < 1.0),
        ),
        (
            "exterior DE 1-4 px",
            Box::new(|s: &sample::Sample| s.class == Class::Exterior && s.de >= 1.0 && s.de < 4.0),
        ),
        (
            "exterior DE >= 4 px",
            Box::new(|s: &sample::Sample| s.class == Class::Exterior && s.de >= 4.0),
        ),
        ("interior", Box::new(|s: &sample::Sample| s.class == Class::Interior)),
        (
            "unresolved",
            Box::new(|s: &sample::Sample| s.class == Class::Unresolved),
        ),
    ] {
        let (a, b) = bucket(&*f);
        println!("  {name:24} {a:5.1}% of pixels, {b:5.1}% of iterations");
    }
    for k in 0..EXPAND.len() {
        let tex = cs.iter().filter(|s| s.is_texture(k)).count();
        let cut: u64 = cs.iter().map(|s| s.cut_cost(k) as u64).sum();
        let tex_iters: u64 = cs.iter().filter(|s| s.is_texture(k)).map(|s| s.n as u64).sum();
        let tex_cut: u64 = cs
            .iter()
            .filter(|s| s.is_texture(k))
            .map(|s| s.cut_cost(k) as u64)
            .sum();
        println!(
            "  footprint cut T={:<6} texture {:5.1}% of pixels ({:5.1}% of iterations); cut keeps {:5.1}% of their iterations; all pixels {:5.1}% of iterations",
            EXPAND[k],
            100.0 * tex as f64 / n_px as f64,
            100.0 * tex_iters as f64 / total as f64,
            100.0 * tex_cut as f64 / tex_iters.max(1) as f64,
            100.0 * cut as f64 / total as f64
        );
    }

    // ---- Static strategies (frame 0) against the converged reference.
    let reference = strategies::reference(&l, 0, None);
    let half0 = strategies::reference(&l, 0, Some(0));
    let half1 = strategies::reference(&l, 0, Some(1));
    let a1 = strategies::centre(&l, 0);
    let base = a1.iters;
    write_png(&out.join("ref.png"), &reference, w, h);
    write_png(&out.join("a1.png"), &a1.img, w, h);
    println!(
        "strategy (frame 0)                          iters/1spp samples/px  error vs converged reference                            minibrot px lost"
    );
    let report = |name: &str, o: &Out| {
        let d = diff(&o.img, &reference);
        let (mb, lost) = sparse::minibrot_loss(&l, 0, &o.img, &reference);
        println!(
            "  {name:42} {} {:9.3}  {}  {lost}/{mb}",
            rel(o, base),
            o.samples as f64 / n_px as f64,
            errors(&d)
        );
        d
    };
    let dh = diff(&half0, &half1);
    println!(
        "  {:42} {:>7} {:>9}  {}  (8 vs 8 samples; the 16-sample reference ≈ 0.7x this)",
        "reference noise",
        "-",
        "-",
        errors(&dh)
    );
    let d_a1 = report("A1: 1 spp centre (refined frame)", &a1);
    write_heat(&out.join("heat-a1.png"), &d_a1, w, h);
    let a9 = strategies::app_aa(&l, 0);
    let d_a9 = report("A9: FL adaptive 3x3 AA (settled)", &a9);
    write_png(&out.join("a9.png"), &a9.img, w, h);
    write_heat(&out.join("heat-a9.png"), &d_a9, w, h);
    for tau in [1.5f32, 3.0] {
        let v = variance::variance_aa(&l, 0, tau, 3);
        report(&format!("V: variance-stopped AA, tau {tau}"), &v);
    }

    if !SA_ONLY.load(std::sync::atomic::Ordering::Relaxed) {
        for tau in [5.0f32, 10.0] {
            let g = strategies::guess(&l, 0, tau);
            let d = report(&format!("G: colour guess 8->1, tau {tau}"), &g);
            let dg = diff(&g.img, &a1.img);
            println!("  {:42} {:>7} {:>9}  {}  (vs A1)", "", "", "", errors(&dg));
            if tau == 5.0 {
                write_png(&out.join("guess5.png"), &g.img, w, h);
                write_heat(&out.join("heat-guess5.png"), &d, w, h);
            }
        }
        let (ms, wrong) = strategies::mariani(&l, 0, 32);
        report("M: Mariani-Silver interior fill", &ms);
        println!(
            "  {:42} filled {:.2}% of pixels, {} of them exterior in A1",
            "",
            100.0 * ms.kind.iter().filter(|&&k| k == 1).count() as f64 / n_px as f64,
            wrong
        );
        let t0 = strategies::texture(
            &l,
            0,
            TextureParams {
                k: 0,
                probe_every: 16,
                sigma: 6.0,
                aa: true,
            },
        );
        report("T: footprint cut T=1e-3, probes 1/16, +AA", &t0);

        // Sparse sampling with texture fill.
        let mut sparse_rows = Vec::new();
        for (g, dmax, sigma) in [
            (1usize, 0.25f32, 6.0f32),
            (2, 0.1, 6.0),
            (2, 0.25, 6.0),
            (2, 0.5, 6.0),
            (4, 0.1, 6.0),
            (4, 0.25, 6.0),
            (4, 0.25, 3.0),
            (4, 0.5, 6.0),
            (8, 0.25, 6.0),
        ] {
            let p = SparseParams {
                g,
                dmax,
                sigma,
                aa: true,
            };
            let o = sparse::sparse(&l, 0, p);
            let d = report(&format!("S: sparse g{g} DE<{dmax} s{sigma} +AA"), &o);
            let tex: Vec<bool> = o.kind.iter().map(|&k| k == 1).collect();
            let other: Vec<bool> = tex.iter().map(|t| !t).collect();
            println!(
                "  {:42} {:>7} {:>9}  texture {:4.1}%: {}",
                "",
                "",
                "",
                100.0 * tex.iter().filter(|&&t| t).count() as f64 / n_px as f64,
                errors(&masked(&d, &tex))
            );
            println!(
                "  {:42} {:>7} {:>9}  other:         {}",
                "",
                "",
                "",
                errors(&masked(&d, &other))
            );
            if (g, dmax, sigma) == (4, 0.25, 6.0) || (g, dmax, sigma) == (2, 0.25, 6.0) {
                let tag = format!("sparse-g{g}");
                write_png(&out.join(format!("{tag}.png")), &o.img, w, h);
                write_heat(&out.join(format!("heat-{tag}.png")), &d, w, h);
                let mask: Vec<u8> = o.kind.iter().map(|&v| [1u8, 2, 4][v as usize]).collect();
                write_mask(&out.join(format!("mask-{tag}.png")), &mask, w, h);
            }
            sparse_rows.push((g, dmax, o.iters as f64 / base as f64));
        }
        let _ = sparse_rows;
    }

    // Local series approximation (branch.rs): AA samples off their pixel's orbit, pixels off a
    // tile centre's orbit.
    let rows: &[(usize, bool, f64)] = &[
        (1, true, 1e-9),
        (1, true, 1e-3),
        (1, true, 1e-2),
        (4, false, 1e-3),
        (8, false, 1e-3),
        (16, false, 1e-3),
        (16, false, 1e-2),
        (32, false, 1e-2),
    ];
    for &(k, aa, eps) in rows {
        let (o, st) = sa::tile_sa(&l, 0, k, eps, aa, &shader);
        let name = if k == 1 {
            format!("L: AA off pixel orbit eps {eps}")
        } else {
            format!("L: tile {k}x{k} eps {eps}{}", if aa { " +AA" } else { "" })
        };
        let d = report(&name, &o);
        let exact = if aa { &a9.img } else { &a1.img };
        println!(
            "  {:42} {:>7} {:>9}  vs exact {}: {}  branch at {:.1}% of n, escape n differs {:.1}%, mean dn {:+.1} dDE {:+.4}",
            "",
            "",
            "",
            if aa { "A9" } else { "A1" },
            errors(&diff(&o.img, exact)),
            100.0 * st.branch_share,
            100.0 * st.differs,
            st.dn,
            st.dde
        );
        if (k, aa, eps) == (16, true, 1e-3) {
            write_png(&out.join("sa-16-aa.png"), &o.img, w, h);
            write_heat(&out.join("heat-sa-16-aa.png"), &d, w, h);
        }
        if (k, aa, eps) == (16, false, 1e-3) {
            write_png(&out.join("sa-16.png"), &o.img, w, h);
        }
    }
    for (top, eps, aa) in [
        (16usize, 1e-2f64, false),
        (32, 1e-2, false),
        (64, 1e-2, false),
        (32, 1e-3, false),
        (32, 1e-2, true),
    ] {
        let (o, st, seed_share) = quad::quad_sa(&l, 0, top, eps, aa, &shader);
        let d = report(
            &format!("Q: quadtree {top} eps {eps}{}", if aa { " +AA" } else { "" }),
            &o,
        );
        println!(
            "  {:42} {:>7} {:>9}  vs exact {}: {}  pixel branch at {:.1}% of n, escape n differs {:.1}%, seed-mode iterations {:.1}%",
            "",
            "",
            "",
            if aa { "A9" } else { "A1" },
            errors(&diff(&o.img, if aa { &a9.img } else { &a1.img })),
            100.0 * st.branch_share,
            100.0 * st.differs,
            100.0 * seed_share
        );
        if (top, eps, aa) == (32, 1e-2, true) {
            write_png(&out.join("quad-32-aa.png"), &o.img, w, h);
            write_heat(&out.join("heat-quad-32-aa.png"), &d, w, h);
        }
    }
    for (k, eps) in [(16usize, 1e-2f64)] {
        let (o, st, seed_share) = sa::nested_sa(&l, 0, k, eps, &shader);
        let d = report(&format!("N: tile {k} -> pixel -> AA, eps {eps}"), &o);
        println!(
            "  {:42} {:>7} {:>9}  vs exact A9: {}  centre escape n differs {:.1}%, seed-mode iterations {:.1}%",
            "",
            "",
            "",
            errors(&diff(&o.img, &a9.img)),
            100.0 * st.differs,
            100.0 * seed_share
        );
        if (k, eps) == (16, 1e-2) {
            write_png(&out.join("nested-16.png"), &o.img, w, h);
            write_heat(&out.join("heat-nested-16.png"), &d, w, h);
        }
    }

    if !motion {
        return;
    }
    // ---- Motion: per-frame images under the scripted pan.
    let refs: Vec<_> = (0..frames).map(|t| strategies::reference(&l, t, None)).collect();
    let a1s: Vec<_> = (0..frames).map(|t| strategies::centre(&l, t)).collect();
    let sp = |g: usize| -> Vec<Out> {
        (0..frames)
            .map(|t| {
                sparse::sparse(
                    &l,
                    t,
                    SparseParams {
                        g,
                        dmax: 0.25,
                        sigma: 6.0,
                        aa: false,
                    },
                )
            })
            .collect()
    };
    let s1 = sp(1);
    let s4 = sp(4);
    let ls: Vec<Out> = (0..frames)
        .map(|t| quad::quad_sa(&l, t, 32, 1e-2, false, &shader).0)
        .collect();
    let imgs = |v: &[Out]| v.iter().map(|o| o.img.clone()).collect::<Vec<_>>();
    let seq_err = |seq: &[Vec<shade::Rgb>]| -> (f32, f32) {
        let mut m = 0.0;
        let mut p = 0.0;
        let mut c = 0.0;
        for t in 3..seq.len() {
            let e = errors(&diff(&seq[t], &refs[t]));
            m += e.mean;
            p += e.p99;
            c += 1.0;
        }
        (m / c, p / c)
    };
    let cost =
        |v: &[Out]| v.iter().map(|o| o.iters).sum::<u64>() as f64 / a1s.iter().map(|o| o.iters).sum::<u64>() as f64;
    println!(
        "motion ({frames} frames, pan (1.2, 0.6) px/frame; errors over frames 3..)   iters/A1  dE mean  dE p99  flicker"
    );
    let rows: Vec<(&str, Vec<Vec<shade::Rgb>>, f64)> = vec![
        ("converged reference", refs.clone(), f64::NAN),
        ("A1 point samples", imgs(&a1s), 1.0),
        (
            "A1 + temporal accumulation (cap 6)",
            metrics::accumulate(&imgs(&a1s), w, h, 6.0),
            1.0,
        ),
        ("S g1 DE<0.25 (texture fill only)", imgs(&s1), cost(&s1)),
        (
            "S g1 + temporal accumulation",
            metrics::accumulate(&imgs(&s1), w, h, 6.0),
            cost(&s1),
        ),
        ("S g4 DE<0.25", imgs(&s4), cost(&s4)),
        (
            "S g4 + temporal accumulation",
            metrics::accumulate(&imgs(&s4), w, h, 6.0),
            cost(&s4),
        ),
        ("Q quadtree 32 eps 1e-2, 1 spp", imgs(&ls), cost(&ls)),
        (
            "Q quadtree 32 + temporal accumulation",
            metrics::accumulate(&imgs(&ls), w, h, 6.0),
            cost(&ls),
        ),
    ];
    for (name, seq, c) in &rows {
        let (m, p) = seq_err(seq);
        println!("  {name:58} {c:7.3}  {m:7.2} {p:7.2}  {:6.2}", flicker(seq, w, h));
    }
    write_png(&out.join("motion-a1-acc-last.png"), rows[2].1.last().unwrap(), w, h);
    write_png(&out.join("motion-s4-acc-last.png"), rows[6].1.last().unwrap(), w, h);
    write_png(&out.join("motion-ref-last.png"), refs.last().unwrap(), w, h);
}

/// Debug: exact vs branched samples for a few AA offsets at the dense view.
fn probe() {
    let loc = locate("dense-5e-14");
    let px = loc.width / 1600.0;
    let mi = loc.max_iter * 4;
    for p in 0..6 {
        let c0 = (
            loc.re + (p as f64 * 37.0 - 100.0) * px,
            loc.im + (p as f64 * 23.0 - 60.0) * px,
        );
        for eps in [1e-9, 1e-6, 1e-3, 1e-2] {
            let sd = branch::seed(c0, 0.566 * px, eps, mi);
            let mut line = format!("p{p} eps {eps:e} branch {:5}:", sd.n());
            for (dx, dy) in [(0.4, 0.4), (-0.4, 0.0), (0.0, -0.4)] {
                let c = (c0.0 + dx * px, c0.1 + dy * px);
                let e = sample::iterate(c.0, c.1, px, mi);
                let b = branch::continue_from(&sd, c, (dx * px, dy * px), px, mi);
                line += &format!(
                    "  n {}/{} de {:.3}/{:.3} ang {:.2}/{:.2}",
                    e.n, b.n, e.de, b.de, e.ang, b.ang
                );
            }
            println!("{line}");
        }
    }
}
