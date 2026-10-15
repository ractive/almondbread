//! M6 study: loop steps of the CPU models (extended range) with no skipping, the deep BLA table
//! (lookups at m = 1 mod 8, the GPU policy), and the LA table with and without AT, on a grid of
//! pixels of each benchmark location, against the reference the renderer would use
//! (kb: techniques/linear-approximation-imagina, synthesis/benchmarks M6).
//!
//!   cargo run --release -p almondbread-core --example la_check -- [--grid WxH] [--size W] [name...]

use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::time::Instant;

use almondbread_core::bla::{BLA_EPSILON, BlaTableX};
use almondbread_core::la::{LaBuild, LaTable, ModelOutcome, Skip, iterate_model};
use almondbread_core::locations::{all_locations, location_named};
use almondbread_core::math::FloatExpComplex;
use almondbread_core::reference::{DEEP_SCALE, Reference, ReferenceRequest};
use rayon::prelude::*;

const BAILOUT: f64 = 65536.0;

fn to_x(points: &[[f32; 2]]) -> Vec<FloatExpComplex> {
    points
        .iter()
        .map(|p| {
            let a = p[0].abs().max(p[1].abs());
            if a == 0.0 {
                return FloatExpComplex::ZERO;
            }
            let e = a.log2().floor() as i32 + 1;
            let s = (-e as f32).exp2();
            FloatExpComplex {
                mantissa: [p[0] * s, p[1] * s],
                exponent: e,
            }
        })
        .collect()
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |flag: &str, default: &str| -> String {
        if let Some(i) = args.iter().position(|a| a == flag) {
            let v = args[i + 1].clone();
            args.drain(i..i + 2);
            v
        } else {
            default.to_string()
        }
    };
    let grid = take("--grid", "64x40");
    let (gw, gh) = grid.split_once('x').map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap())).unwrap();
    let width: usize = take("--size", "1600").parse().unwrap();
    let height = width * 5 / 8;
    let names: Vec<String> = if args.is_empty() {
        all_locations().map(|l| l.name().to_string()).collect()
    } else {
        args
    };
    println!(
        "{:<18} {:>7} {:>6} {:>5} {:>9} | {:>8} {:>8} {:>8} {:>8} | {:>6} {:>6} {:>6} | {:>5} {:>5} {:>5} | {:>8} {:>8}",
        "location", "points", "stages", "bnds", "build ms", "exact", "bla", "la", "la+at", "bla x", "la x", "at x",
        "clsB", "clsL", "clsA", "dmuL", "tests/px"
    );
    for name in &names {
        let loc = location_named(name).unwrap_or_else(|| panic!("unknown location {name}"));
        let v = loc.viewport_for_width(width);
        let max_iter = loc.max_iterations();
        let texel = v.scale;
        let r = if texel.to_f64() >= DEEP_SCALE {
            Reference::compute_f64(&v, max_iter, BAILOUT)
        } else {
            let radius = 0.5 * (width as f64).hypot(height as f64);
            let mut req = ReferenceRequest::for_view(&v, texel, width, radius, max_iter * 2, BAILOUT);
            req.period_limit = max_iter;
            Reference::compute_big(&req, &AtomicBool::new(false), &AtomicUsize::new(0)).expect("reference")
        };
        let pts = if r.extended.is_empty() { to_x(&r.points) } else { r.extended.clone() };
        let off = v.center.delta_in(&r.point, texel);
        let t0 = Instant::now();
        let bla = BlaTableX::build(&pts, BLA_EPSILON);
        let tb = t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        let env = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let d = LaBuild::default();
        let params = LaBuild {
            near0_log2: env("LA_NEAR0", d.near0_log2),
            near_log2: env("LA_NEAR", d.near_log2),
            collapse_log2: env("LA_COLLAPSE", d.collapse_log2),
            synthetic_max: env("LA_SMAX", d.synthetic_max as f64) as usize,
            synthetic_factor: env("LA_F", d.synthetic_factor),
        };
        let la = LaTable::build_with(&pts, BLA_EPSILON, &params);
        let tl = t0.elapsed().as_secs_f64();
        let (tm, te) = texel.parts();
        if std::env::var("LA_CHAIN").is_ok() {
            for j in [0usize, 1, 2, 3] {
                if j < la.boundaries() {
                    println!("{}", la.describe_chain(j));
                }
            }
            println!("|dc| max 2^{:.1}", (tm * (width as f64).hypot(height as f64) / 2.0).log2() + te as f64);
        }
        let pixels: Vec<(f64, f64)> = (0..gh)
            .flat_map(|j| {
                (0..gw).map(move |i| {
                    let x = (i as f64 + 0.5) * width as f64 / gw as f64;
                    let y = (j as f64 + 0.5) * height as f64 / gh as f64;
                    (x - width as f64 / 2.0 + off.0, -(y - height as f64 / 2.0) + off.1)
                })
            })
            .collect();
        let run = |skip: Skip<'_>| -> Vec<ModelOutcome> {
            pixels
                .par_iter()
                .map(|&(x, y)| iterate_model(&pts, skip, (x * tm, y * tm, te), max_iter, BAILOUT * BAILOUT))
                .collect()
        };
        if let Ok(k) = std::env::var("LA_TRACE") {
            let k: usize = k.parse().unwrap();
            let (x, y) = pixels[k];
            almondbread_core::la::TRACE.with(|t| t.set(true));
            let o = iterate_model(&pts, Skip::La(&la, false), (x * tm, y * tm, te), max_iter, BAILOUT * BAILOUT);
            almondbread_core::la::TRACE.with(|t| t.set(false));
            println!("pixel {k}: {o:?}");
        }
        let exact = run(Skip::None);
        let ob = run(Skip::Bla(&bla, 8));
        let ol = run(Skip::La(&la, false));
        let oa = run(Skip::La(&la, true));
        let steps = |o: &[ModelOutcome]| o.iter().map(|x| x.steps).sum::<usize>() as f64 / o.len() as f64;
        let tests = |o: &[ModelOutcome]| o.iter().map(|x| x.tests).sum::<usize>() as f64 / o.len() as f64;
        let esc = exact.iter().filter(|o| o.escaped).count() as f64 / exact.len() as f64 * 100.0;
        let class = |o: &[ModelOutcome]| exact.iter().zip(o).filter(|(a, b)| a.escaped != b.escaped).count();
        let dmu = |o: &[ModelOutcome]| {
            let mut d: Vec<f64> = exact
                .iter()
                .zip(o)
                .filter(|(a, b)| a.escaped && b.escaped)
                .map(|(a, b)| (a.mu() - b.mu()).abs())
                .collect();
            d.sort_by(f64::total_cmp);
            d.get(d.len() / 2).copied().unwrap_or(0.0)
        };
        let (se, sb, sl, sa) = (steps(&exact), steps(&ob), steps(&ol), steps(&oa));
        println!(
            "{:<18} {:>7} {:>6} {:>5} {:>4.1}/{:<4.1} | {:>8.0} {:>8.0} {:>8.0} {:>8.0} | {:>6.1} {:>6.1} {:>6.1} | {:>5} {:>5} {:>5} | {:>8.1e} {:>4.1}/{:<4.1}  stages {:?}{}",
            name,
            pts.len(),
            la.stage_steps.len(),
            la.boundaries(),
            tb * 1e3,
            tl * 1e3,
            se,
            sb,
            sl,
            sa,
            se / sb,
            se / sl,
            se / sa,
            class(&ob),
            class(&ol),
            class(&oa),
            dmu(&ol),
            tests(&ol),
            tests(&ob),
            la.stage_steps,
            la.at.as_ref().map_or(String::new(), |a| format!(" AT period {}", a.period)),
        );
        println!("    escaped {esc:.1} %");
    }
}
