//! `landscape`: L0 prototype of almondbread's landscape mode (kb: decisions/landscape-mode).
//! Renders the Mandelbrot set as a 2.5D terrain h(c) = f(DE(c)) seen from a camera.

mod font;
mod mandel;
mod render;
mod terrain;

use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;
use terrain::{HeightCfg, HeightFn, Terrain};

#[derive(Parser)]
#[command(name = "landscape", about = "Mandelbrot terrain stills (L0 prototype)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Render one still.
    Render {
        #[command(flatten)]
        shot: Shot,
        /// Output file (.png or .jpg).
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Render a dive: N frames on a glide path towards the boundary point of the shot.
    Dive {
        #[command(flatten)]
        shot: Shot,
        /// Final scale.
        #[arg(long)]
        scale_end: f64,
        #[arg(long, default_value_t = 8)]
        frames: u32,
        /// Output directory; frames are dive-000.png ...
        #[arg(long)]
        out_dir: PathBuf,
        /// Extra rotation of the heading per frame (degrees), for a curving descent.
        #[arg(long, default_value_t = 0.0)]
        turn: f64,
    },
    /// Combine images into a labelled grid (JPEG/PNG by extension).
    Montage {
        #[arg(long, default_value_t = 2)]
        cols: u32,
        /// Width of each cell in pixels (images are resized).
        #[arg(long, default_value_t = 960)]
        cell: u32,
        #[arg(long, default_value_t = 82)]
        quality: u8,
        /// Labels, one per image (use "" for none); "|" separates.
        #[arg(long, default_value = "")]
        labels: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, short)]
        out: PathBuf,
        inputs: Vec<PathBuf>,
    },
    /// Print the resolved camera ground point, minibrot search and DE at it.
    Info {
        #[command(flatten)]
        shot: Shot,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Snap {
    None,
    Boundary,
    Minibrot,
}

#[derive(Args, Clone, Debug)]
struct Shot {
    /// Named location: seahorse, elephant, neck, spiral, home, minibrot3.
    #[arg(long)]
    loc: Option<String>,
    /// Centre re,im (overrides --loc).
    #[arg(long, allow_hyphen_values = true)]
    center: Option<String>,
    /// Camera scale S = altitude in complex-plane units; the detail scale.
    #[arg(long, default_value_t = 0.1)]
    scale: f64,
    /// How to place the camera: snap the centre onto the nearest boundary point,
    /// or onto the shore of a nearby minibrot.
    #[arg(long, value_enum, default_value_t = Snap::Boundary)]
    snap: Snap,
    /// After snapping, back off this many S outwards (ground point = valley side).
    #[arg(long, default_value_t = 2.0)]
    back: f64,
    /// Heading in degrees (0 = +Re). Default: look towards the set, plus --turn-heading.
    #[arg(long, allow_hyphen_values = true)]
    heading: Option<f64>,
    #[arg(long, default_value_t = 25.0, allow_hyphen_values = true)]
    turn_heading: f64,
    #[arg(long, default_value_t = -12.0, allow_hyphen_values = true)]
    pitch: f64,
    /// Horizontal field of view, degrees.
    #[arg(long, default_value_t = 70.0)]
    fov: f64,
    /// Altitude above the smoothed ground in units of S.
    #[arg(long, default_value_t = 1.0)]
    altitude: f64,
    #[arg(long, value_enum, default_value_t = HeightFn::Log)]
    height: HeightFn,
    /// Height gain k (slope at the valley floor for linear/log).
    #[arg(long, default_value_t = 4.0)]
    k: f64,
    /// Scale of the log/sqrt curves in units of S.
    #[arg(long, default_value_t = 0.1)]
    sigma: f64,
    #[arg(long, default_value = "1920x1080")]
    size: String,
    /// Supersampling per axis.
    #[arg(long, default_value_t = 1)]
    ss: u32,
    #[arg(long, default_value_t = 50_000)]
    max_iter: u32,
    /// Level-of-detail bias: sample spacing = bias × pixel footprint.
    #[arg(long, default_value_t = 1.5)]
    lod_bias: f64,
    /// Fog distance in units of S.
    #[arg(long, default_value_t = 120.0)]
    fog: f64,
    /// Water level in units of S (lakes are at 0).
    #[arg(long, default_value_t = 0.0)]
    water: f64,
    /// Sun azimuth relative to the heading, degrees.
    #[arg(long, default_value_t = 150.0, allow_hyphen_values = true)]
    sun_az: f64,
    #[arg(long, default_value_t = 22.0)]
    sun_el: f64,
    #[arg(long)]
    no_shadows: bool,
    #[arg(long)]
    no_reflections: bool,
    /// Far clip in units of S.
    #[arg(long, default_value_t = 3000.0)]
    max_dist: f64,
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    palette_shift: f64,
    /// Planet radius in units of S (0 = flat world); hides far walls below the horizon.
    #[arg(long, default_value_t = 150.0)]
    curvature: f64,
    /// Label burnt into the image (top-left).
    #[arg(long)]
    label: Option<String>,
}

fn loc_center(name: &str) -> (f64, f64) {
    match name {
        "seahorse" => (-0.743643887037151, 0.131825904205330),
        "elephant" => (0.2925755, -0.0149977),
        "neck" => (-0.75, 0.05),
        "spiral" => (-0.7746806106269039, -0.1374168856037867),
        "home" => (-0.5, 0.9),
        "minibrot3" => (-1.7548776662466927, 0.0),
        _ => panic!("unknown --loc {name}"),
    }
}

fn parse_pair(s: &str) -> (f64, f64) {
    let mut it = s.split(',').map(|v| v.trim().parse::<f64>().expect("number"));
    (it.next().unwrap(), it.next().unwrap())
}

struct Resolved {
    ground: (f64, f64),
    heading: f64,
    note: String,
}

fn resolve(shot: &Shot) -> Resolved {
    let c = match (&shot.center, &shot.loc) {
        (Some(c), _) => parse_pair(c),
        (None, Some(l)) => loc_center(l),
        _ => loc_center("seahorse"),
    };
    let s = shot.scale;
    let mut note = String::new();
    let (b, out) = match shot.snap {
        Snap::None => {
            let e = mandel::eval(c.0, c.1, shot.max_iter);
            (c, if e.interior { (1.0, 0.0) } else { e.grad })
        }
        Snap::Boundary => mandel::snap_to_boundary(c, s * 1e-3, shot.max_iter),
        Snap::Minibrot => match minibrot_near(c, s) {
            Some((nuc, p, size)) => {
                let sz = (size.0.hypot(size.1)).max(1e-300);
                note = format!("minibrot nucleus {:.17},{:.17} period {} size {:.3e}", nuc.0, nuc.1, p, sz);
                // approach the shore from the side opposite the cusp
                let ang = size.1.atan2(size.0);
                let start = (nuc.0 - 1.2 * sz * ang.cos(), nuc.1 - 1.2 * sz * ang.sin());
                mandel::snap_to_boundary(start, s * 1e-3, shot.max_iter)
            }
            None => {
                note = "no minibrot found; boundary snap".into();
                mandel::snap_to_boundary(c, s * 1e-3, shot.max_iter)
            }
        },
    };
    let ground = if shot.snap == Snap::None { b } else { (b.0 + shot.back * s * out.0, b.1 + shot.back * s * out.1) };
    let heading = match shot.heading {
        Some(h) => h.to_radians(),
        None => (-out.1).atan2(-out.0) + shot.turn_heading.to_radians(),
    };
    Resolved { ground, heading, note }
}

/// A minibrot whose size is comparable to the scale (0.2 S .. 40 S) near `c`.
fn minibrot_near(c: (f64, f64), s: f64) -> Option<((f64, f64), u32, (f64, f64))> {
    let mut r = 0.05 * s;
    let mut best: Option<((f64, f64), u32, (f64, f64))> = None;
    while r < 2000.0 * s {
        if let Some((nuc, p, size)) = mandel::find_minibrot(c, r, 1_000_000) {
            let sz = size.0.hypot(size.1);
            let dist = (nuc.0 - c.0).hypot(nuc.1 - c.1);
            if sz.is_finite() && sz > 0.2 * s && sz < 40.0 * s && dist < 200.0 * s {
                if best.map_or(true, |b| b.2 .0.hypot(b.2 .1) < sz) {
                    best = Some((nuc, p, size));
                }
                if sz > 2.0 * s {
                    break;
                }
            }
        }
        r *= 1.6;
    }
    best
}

fn parse_size(s: &str) -> (u32, u32) {
    let (w, h) = s.split_once('x').expect("WxH");
    (w.parse().unwrap(), h.parse().unwrap())
}

fn render_one(shot: &Shot, ground: (f64, f64), heading: f64, out: &Path) -> String {
    let (w, h) = parse_size(&shot.size);
    let cfg = HeightCfg { func: shot.height, k: shot.k, sigma: shot.sigma, log_grad_g0: 0.0 };
    let t0 = Instant::now();
    let terrain = Terrain::new(ground, shot.scale, cfg, shot.max_iter);
    let view = render::View {
        width: w,
        height: h,
        heading,
        pitch: shot.pitch.to_radians(),
        fov: shot.fov.to_radians(),
        altitude: shot.altitude,
        lod_bias: shot.lod_bias,
        sun_az: shot.sun_az.to_radians(),
        sun_el: shot.sun_el.to_radians(),
        fog: shot.fog,
        water: shot.water,
        shadows: !shot.no_shadows,
        reflections: !shot.no_reflections,
        ss: shot.ss,
        max_dist: shot.max_dist,
        palette_shift: shot.palette_shift,
        curvature: shot.curvature,
    };
    let (buf, st) = render::render(&terrain, &view);
    let secs = t0.elapsed().as_secs_f64();
    let mut img = image::RgbImage::from_raw(w, h, buf).unwrap();
    if let Some(l) = &shot.label {
        let sc = (w / 640).max(2);
        font::draw(&mut img, 8 * sc, 8 * sc, sc, l);
    }
    save(&img, out, 90);
    use std::sync::atomic::Ordering::Relaxed;
    let tiles = terrain.tiles.load(Relaxed);
    let evals = terrain.evals.load(Relaxed);
    let iters = terrain.iters.load(Relaxed);
    let unres = terrain.unresolved.load(Relaxed);
    let tile_s = terrain.tile_ns.load(Relaxed) as f64 * 1e-9;
    let threads = rayon::current_num_threads() as f64;
    format!(
        "{}: {:.2}s  {}x{} ss{}  ground {:.17},{:.17} S {:.3e} heading {:.1}°  tiles {} DE evals {:.2}M iters {:.2}G (avg {:.0}/eval, {:.2}% unresolved)  tile CPU {:.1}s (~{:.0}% of wall×{} threads)  march steps {:.1}/ray",
        out.display(),
        secs,
        w,
        h,
        shot.ss,
        ground.0,
        ground.1,
        shot.scale,
        heading.to_degrees(),
        tiles,
        evals as f64 * 1e-6,
        iters as f64 * 1e-9,
        iters as f64 / evals.max(1) as f64,
        100.0 * unres as f64 / evals.max(1) as f64,
        tile_s,
        100.0 * tile_s / (secs * threads),
        threads,
        st.steps as f64 / st.rays as f64
    )
}

fn save(img: &image::RgbImage, out: &Path, quality: u8) {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
    if ext == "jpg" || ext == "jpeg" {
        let f = std::fs::File::create(out).unwrap();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(f), quality);
        enc.encode_image(img).unwrap();
    } else {
        img.save(out).unwrap();
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Render { shot, out } => {
            let r = resolve(&shot);
            if !r.note.is_empty() {
                eprintln!("{}", r.note);
            }
            println!("{}", render_one(&shot, r.ground, r.heading, &out));
        }
        Cmd::Info { shot } => {
            let r = resolve(&shot);
            let e = mandel::eval(r.ground.0, r.ground.1, shot.max_iter);
            println!("{}", r.note);
            println!("ground {:.17},{:.17} heading {:.2}° DE/S {:.3} interior {} mu {:.1}", r.ground.0, r.ground.1, r.heading.to_degrees(), e.de / shot.scale, e.interior, e.mu);
        }
        Cmd::Dive { shot, scale_end, frames, out_dir, turn } => {
            // target: the boundary point, snapped to the precision of the final scale
            let r = resolve(&Shot { back: 0.0, scale: scale_end, ..shot.clone() });
            // r.ground is now the boundary target; approach along the outward normal
            let tgt = r.ground;
            let base = r;
            let out_dir_v = {
                let h = base.heading - shot.turn_heading.to_radians();
                (-h.cos(), -h.sin()) // heading looks towards the set; outward is opposite
            };
            let dir = (shot.back * out_dir_v.0, shot.back * out_dir_v.1);
            let n = frames.max(2);
            for i in 0..n {
                let f = i as f64 / (n - 1) as f64;
                let s = shot.scale * (scale_end / shot.scale).powf(f);
                // the offset from the target shrinks with the scale: a glide path into the target
                let rot = (turn * i as f64).to_radians();
                let (cr, sr) = (rot.cos(), rot.sin());
                let d = (dir.0 * cr - dir.1 * sr, dir.0 * sr + dir.1 * cr);
                let g = (tgt.0 + d.0 * s, tgt.1 + d.1 * s);
                let mut fs = shot.clone();
                fs.scale = s;
                if let Some(l) = &shot.label {
                    fs.label = Some(format!("{l} {}/{} S={:.0e}", i + 1, n, s));
                }
                let out = out_dir.join(format!("dive-{i:03}.png"));
                println!("{}", render_one(&fs, g, base.heading + rot, &out));
            }
        }
        Cmd::Montage { cols, cell, quality, labels, title, out, inputs } => {
            let labels: Vec<&str> = if labels.is_empty() { vec![] } else { labels.split('|').collect() };
            let imgs: Vec<image::RgbImage> = inputs.iter().map(|p| image::open(p).expect("open").to_rgb8()).collect();
            let (w0, h0) = imgs[0].dimensions();
            let ch = (cell as f64 * h0 as f64 / w0 as f64).round() as u32;
            let rows = (imgs.len() as u32).div_ceil(cols);
            let top = if title.is_some() { 40 } else { 0 };
            let mut canvas = image::RgbImage::from_pixel(cols * cell, rows * ch + top, image::Rgb([20, 20, 20]));
            if let Some(t) = &title {
                font::draw(&mut canvas, 6, 6, 3, t);
            }
            for (i, im) in imgs.iter().enumerate() {
                let small = image::imageops::resize(im, cell, ch, image::imageops::FilterType::Lanczos3);
                let x = (i as u32 % cols) * cell;
                let y = (i as u32 / cols) * ch + top;
                image::imageops::replace(&mut canvas, &small, x as i64, y as i64);
                if let Some(l) = labels.get(i) {
                    if !l.is_empty() {
                        font::draw(&mut canvas, x + 6, y + 6, 2, l);
                    }
                }
            }
            save(&canvas, &out, quality);
            println!("{} {}x{}", out.display(), canvas.width(), canvas.height());
        }
    }
}
