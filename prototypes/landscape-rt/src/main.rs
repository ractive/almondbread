//! L1 prototype of landscape mode: real-time flight over the Mandelbrot set as 2.5D terrain
//! (kb: decisions/landscape-mode). See README.md.

mod app;
mod clipmap;
mod flight;
mod gpu;
mod ground;
#[allow(dead_code)]
mod mandel;
mod record;
mod sim;
mod stats;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Fly over the Mandelbrot set as terrain (L1 prototype)")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Open the window and fly (default).
    Fly(app::FlyArgs),
    /// Render a scripted flight offscreen into a video, with stability metrics.
    Record(record::RecordArgs),
    /// Check GPU DE samples against the CPU f64 iteration after a prefill.
    Check {
        #[command(flatten)]
        set: sim::Settings,
        #[arg(long, default_value_t = 4000)]
        per_level: usize,
    },
    /// DE tile throughput: cold prefill (all levels, including the 4 extra ones of low flight)
    /// at a few locations; tiles per GPU second.
    Bench {
        #[command(flatten)]
        set: sim::Settings,
    },
    /// Temporal-stability metrics of an existing video.
    Framestats { path: std::path::PathBuf },
}

fn main() {
    let mut argv: Vec<String> = std::env::args().collect();
    if argv.len() < 2 || argv[1].starts_with('-') && argv[1] != "-h" && argv[1] != "--help" {
        argv.insert(1, "fly".into());
    }
    let cli = Cli::parse_from(argv);
    let r = match cli.cmd {
        Some(Cmd::Fly(a)) => app::run(a),
        Some(Cmd::Record(a)) => record::run(a),
        Some(Cmd::Framestats { path }) => record::framestats(&path),
        Some(Cmd::Bench { set }) => sim::bench(set),
        Some(Cmd::Check { set, per_level }) => sim::Sim::new(set, 0.0).map(|mut s| {
            eprintln!("{}", s.ref_note);
            s.prefill();
            s.check(per_level);
        }),
        None => unreachable!(),
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
