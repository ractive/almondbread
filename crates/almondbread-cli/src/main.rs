//! `almondbread`: the headless tools of the Rust renderer (kb: platform/render-architecture,
//! synthesis/benchmarks). Same options and output as the Swift `AlmondbreadMac` commands, so
//! results compare 1:1:
//!
//! ```text
//! almondbread render out.png --center re,im --scale s --size WxH [--iterations N] [--repeat n]
//! almondbread verify --center re,im --scale s --size WxH [--iterations N] [--dump f.iter] [--heatmap d.png]
//! almondbread verify --fields a.iter --against b.iter
//! almondbread verify --history zoom|out|pan|jump --center re,im --scale s [--ppp p] [--pointer x,y] [--then f]
//! almondbread bench [--sizes 3456x2234,1600x1000] [--only a,b] [--repeat n] [--out dir]
//! almondbread regress
//! almondbread precision --location sea-8007 [--shifts 0.001,0.01,0.1] [--out dir/name]
//! almondbread palettes --out dir [--views seahorse,antenna] [--import a.ugr,b.map] [--list 1]
//! almondbread fly --script flight.json [--out review/x.mp4] [--fps 60] [--size 1920x1080] [--converged 1]
//! almondbread benchmark [--modes a,approx] [--repeat 3] [--warmup 1] | --live-app app | --compare a.json b.json
//! almondbread places [--format text|tsv|landscape] [--render-thumbs dir [--only a,b]]
//! almondbread locate --center re,im [--radius r | --misiurewicz k,p]
//! almondbread journey --seed 4711 [--legs 12] [--daily 2026-10-15] [--center re,im --span s] [--thumbs dir]
//! ```
//!
//! `render`, `verify` and `fly` views take `--place id` (the places catalogue).

// The commands are generic over the backend; only Apple targets have one yet.
#![cfg_attr(not(target_vendor = "apple"), allow(dead_code))]

mod benchmark;
mod commands;
mod fly;
mod options;
mod palettes;
mod places;
mod precision;

use options::Options;

const USAGE: &str = "usage: almondbread render|verify|precision|bench|regress|fly|benchmark|places|locate|journey|palettes [--key value ...]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let options = match Options::parse(&args[1..]) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("almondbread: {e}");
            std::process::exit(2);
        }
    };
    std::process::exit(match run(&command, &options) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("almondbread: {e}");
            1
        }
    });
}

#[cfg(target_vendor = "apple")]
fn run(command: &str, o: &Options) -> options::Result<i32> {
    use almondbread_render::metal::MetalBackend;
    let backend = MetalBackend::new()?;
    MetalBackend::pool(|| match command {
        "render" => commands::render_command(&backend, o),
        "verify" => commands::verify_command(&backend, o),
        "bench" => commands::bench_command(&backend, o),
        "regress" => commands::regress_command(&backend, o),
        "fly" => fly::fly_command(&backend, o),
        "benchmark" => benchmark::benchmark_command(&backend, o),
        "palettes" => palettes::palettes_command(&backend, o),
        "locate" => commands::locate_command(o),
        "journey" => commands::journey_command(&backend, o),
        "places" => places::places_command(&backend, o),
        "precision" => precision::precision_command(&backend, o),
        _ => options::err(USAGE),
    })
}

#[cfg(not(target_vendor = "apple"))]
fn run(_command: &str, _o: &Options) -> options::Result<i32> {
    options::err("no GPU backend on this platform yet (Metal only)")
}
