//! `almondbread`: the headless tools of the Rust renderer (kb: platform/render-architecture,
//! synthesis/benchmarks). Same options and output as the Swift `AlmondbreadMac` commands, so
//! results compare 1:1:
//!
//! ```text
//! almondbread render out.png --center re,im --scale s --size WxH [--iterations N] [--repeat n]
//! almondbread verify --center re,im --scale s --size WxH [--iterations N] [--dump f.iter] [--heatmap d.png]
//! almondbread verify --fields a.iter --against b.iter
//! almondbread verify --history zoom|out|pan --center re,im --scale s [--ppp p] [--pointer x,y] [--then f]
//! almondbread bench [--sizes 3456x2234,1600x1000] [--only a,b] [--repeat n] [--out dir]
//! almondbread regress
//! almondbread fly --script flight.json [--out review/x.mp4] [--fps 60] [--size 1920x1080] [--converged 1]
//! ```

// The commands are generic over the backend; only Apple targets have one yet.
#![cfg_attr(not(target_vendor = "apple"), allow(dead_code))]

mod commands;
mod fly;
mod options;

use options::Options;

const USAGE: &str = "usage: almondbread render|verify|bench|regress|fly [--key value ...]";

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
        "locate" => commands::locate_command(o),
        _ => options::err(USAGE),
    })
}

#[cfg(not(target_vendor = "apple"))]
fn run(_command: &str, _o: &Options) -> options::Result<i32> {
    options::err("no GPU backend on this platform yet (Metal only)")
}
