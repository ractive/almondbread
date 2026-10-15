//! The motion sample cache (kb: techniques/sample-reuse, synthesis/motion-sample-reuse): gesture
//! frames reuse the exact samples of earlier frames and compute only where the view lacks them.
//!
//! Samples sit on a **lattice fixed in the complex plane**: point `(i, j)` of a level is
//! `anchor + h · R(angle) · (i, −j)` (`h` the spacing, `j` downwards like texture rows). A level
//! is held as a **toroidal window** of that lattice: a W × H texture set in which window texel
//! `t` (lattice index `16 · win0 + t`) is texel `(lattice index) mod (W, H)`. So:
//!
//! - **Pans** slide the window by whole 16 × 16 tiles: tiles that enter are cleared on the GPU
//!   and computed, everything else stays where it is. Bookkeeping is per tile on the CPU
//!   (O(window tiles), a few thousand), never per pixel.
//! - **Zooms** switch the level when the spacing leaves its band of texels per output pixel:
//!   the coarser level's points are every other point of the finer one, so a switch copies the
//!   samples that coincide (one GPU pass, `scResample`) and computes the rest tile by tile.
//! - **Rest**: the refined frame's full level seeds a new epoch (its texel centres are a
//!   lattice), so a gesture starting from rest begins with a complete cache.
//!
//! Only the iterate kernel's chunks compute samples, against the job's reference with the
//! epoch's pinned regime (deep kernel, BLA, interior test, DE, interior analysis), so every
//! sample equals what a fresh computation at its point gives in that regime. A sample does not
//! depend on the reference (perturbation with rebasing is exact for any reference; the float32
//! rounding of `dc` moves the point by ~1e-4 texel), so references may change within an epoch;
//! a regime change starts a new epoch. Samples remember the limit they ran to per tile; tiles
//! whose limit fell 25 % behind the view's are recomputed.
//!
//! Each frame the visible window is re-shaded with the current colours (`scShade`) and the
//! compose pass reconstructs the image from the lattice samples (Gaussian point samples plus
//! the reprojected history, as for FL's jittered previews; kb: techniques/frame-reuse-and-
//! progressive).

use std::collections::VecDeque;
use std::time::Instant;

use almondbread_core::autoiter::InteriorStats;
use almondbread_core::viewport::{Center, Scale, Viewport};
use bytemuck::bytes_of;

use crate::backend::{ComputeEncoder, GpuBackend, GpuError, Storage, Submission, SubmissionStatus, TextureFormat};
use crate::gpu_types::{CacheParams, ColorParams};

const TILE: i64 = 16;
/// Steps of the target ratio below the cap (cap / 8 at most: FL's 1/8 floor).
const MAX_STEPS: u32 = 3;
/// A finer level when the ratio (texels per output pixel) falls below this share of the target.
const FINER_AT: f64 = 0.45;
/// Rotation between view and lattice the window is sized for (radians, ~10 degrees).
const ROTATION_SLACK: f64 = 0.175;
/// Lattice angle realigned with the view at a finer switch when they differ by more (2 degrees).
const REALIGN: f64 = 0.035;

/// What the samples of an epoch were computed with. Samples of different regimes never mix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regime {
    /// The deep (extended-range) kernel.
    pub deep: bool,
    pub bla: bool,
    /// The derivative interior test.
    pub interior: bool,
    pub de: bool,
    /// The interior cycle analysis (aux texture).
    pub interior_color: bool,
}

/// The cache's settings.
#[derive(Clone, Debug)]
pub struct CacheConfig {
    pub enabled: bool,
    /// Texels per output pixel at most (√2: up to two samples per pixel, one on average over
    /// the level band); lowered for large views by `max_texels`.
    pub max_ratio: f64,
    /// Texels of the visible window at the largest ratio (bounds the memory: ~32 B per texel
    /// of the allocation, which adds the rotation slack and tile rounding).
    pub max_texels: usize,
    /// Print slow planning and GPU passes (`ALMONDBREAD_CACHE_TRACE=1`).
    pub trace: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        let env = |k: &str| std::env::var(k).ok();
        Self {
            enabled: env("ALMONDBREAD_SAMPLE_CACHE").is_none_or(|v| v != "0"),
            max_ratio: env("ALMONDBREAD_CACHE_RATIO")
                .and_then(|v| v.parse().ok())
                .filter(|&v: &f64| v > 0.0 && v <= 2.0)
                .unwrap_or(std::f64::consts::SQRT_2),
            max_texels: env("ALMONDBREAD_CACHE_TEXELS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(4_200_000),
            trace: env("ALMONDBREAD_CACHE_TRACE").is_some_and(|v| v == "1"),
        }
    }
}

/// Measurements (interaction tests, `fly`, benchmarks).
#[derive(Clone, Debug, Default)]
pub struct CacheStats {
    /// Gesture frames the cache planned, and those that fell back to a fresh preview.
    pub frames: usize,
    pub fallback_frames: usize,
    /// Tiles and texels handed to the iterate kernel, and texels whose batch completed.
    pub tiles_planned: usize,
    pub texels_planned: usize,
    pub texels_done: usize,
    /// Iterations of completed batches (chunk counters).
    pub iterations: f64,
    /// Visible texels at planning time, and those already holding a sample (reused).
    pub visible_texels: usize,
    pub reused_texels: usize,
    /// CPU seconds: planning (incl. encoding the maintenance passes), shading encode, seeding.
    pub cpu_plan: f64,
    pub cpu_shade: f64,
    pub cpu_seed: f64,
    /// Largest planning time of one frame.
    pub cpu_plan_max: f64,
    /// GPU seconds of the cache's own passes (clear / resample / seed, shade), and how many.
    pub gpu_maintain: f64,
    pub gpu_shade: f64,
    pub gpu_passes: usize,
    pub gpu_shades: usize,
    pub epochs: usize,
    pub seeds: usize,
    pub finer: usize,
    pub coarser: usize,
    pub slides: usize,
    pub cleared_tiles: usize,
    /// Texels the shade passes coloured.
    pub shaded_texels: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Empty,
    /// Some texels hold samples (a finer switch copied a quarter, or a batch was abandoned).
    Partial,
    Full,
}

#[derive(Clone, Copy, Debug)]
struct Tile {
    /// Lattice tile this texture tile holds (in the current window).
    holder: (i64, i64),
    state: State,
    /// Iteration limit its samples ran to.
    limit: u32,
    /// Generation of the batch computing it (0: none).
    inflight: u64,
    /// The colour texture holds its samples shaded with the current colour key.
    shaded: bool,
}

/// The cache's lattice and window.
#[derive(Clone, Debug)]
struct Epoch {
    regime: Regime,
    anchor: Center,
    angle: f64,
    /// Spacing of the current level (complex units).
    h: Scale,
    /// Window origin in lattice tiles: window texel t is lattice index 16 · win0 + t.
    win0: (i64, i64),
    /// Bumped by every level switch and re-anchoring (batches of older levels are ignored).
    serial: u64,
}

struct Batch {
    generation: u64,
    serial: u64,
    tiles: Vec<(usize, (i64, i64))>,
    texels: usize,
    limit: u32,
    /// GPU seconds and iterations of its chunks so far.
    gpu: f64,
    iterations: f64,
    /// Estimated iteration units (cost model calibration).
    units: f64,
}

/// A sample cache texture set.
pub(crate) struct CacheSet<B: GpuBackend> {
    pub iter: B::Texture,
    pub de: B::Texture,
    pub aux: Option<B::Texture>,
}

impl<B: GpuBackend> Clone for CacheSet<B> {
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            de: self.de.clone(),
            aux: self.aux.clone(),
        }
    }
}

struct Alloc<B: GpuBackend> {
    sets: [CacheSet<B>; 2],
    color: B::Texture,
    /// Per-slot sample statistics the shade passes write (uint4), CPU-visible.
    stats: B::Buffer,
    /// Window size in tiles.
    wt: usize,
    ht: usize,
    /// What it was sized for: output size, ratio cap.
    for_view: (usize, usize, u64),
}

/// The lattice frame of a planned cache level, for the iterate parameters.
pub(crate) struct LatticeFrame {
    pub anchor: Center,
    pub h: Scale,
    /// (cos, sin) of the lattice angle (the kernel's `rot`).
    pub rot: (f64, f64),
    /// Lattice index of window texel (0, 0).
    pub origin: (i64, i64),
    /// Packed wrap for the kernel: texel gid holds window texel (gid + wrap) mod size.
    pub wrap: u32,
}

/// A planned gesture level: textures, frame, the tiles to compute (texture tile indices).
pub(crate) struct Plan<B: GpuBackend> {
    pub set: CacheSet<B>,
    pub color: B::Texture,
    pub width: usize,
    pub height: usize,
    pub frame: LatticeFrame,
    pub order: Option<B::Buffer>,
    pub tiles: usize,
    /// Texels per output pixel of the level.
    pub ratio: f64,
}

/// How a view maps onto the cache (compose, survey).
#[derive(Clone, Copy, Debug)]
pub(crate) struct CacheMap {
    /// Window-relative lattice coordinates of the view centre.
    pub centre: [f64; 2],
    /// Texels per output pixel.
    pub ratio: f64,
    /// (cos, sin) of view angle - lattice angle.
    pub rot: [f64; 2],
    /// Texel of window texel (0, 0).
    pub wrap: [u32; 2],
    pub size: (usize, usize),
}

/// What the scheduler knows when it plans a gesture frame.
pub(crate) struct PlanInput<'a> {
    pub view: &'a Viewport,
    pub ppp: f64,
    pub width: usize,
    pub height: usize,
    pub regime: Regime,
    /// The epoch's BLA regime cannot be served this frame (its table is still building).
    pub bla_pending: bool,
    pub limit: usize,
    /// GPU seconds the frame's samples may take, and the fallback cost per texel.
    pub budget: f64,
    pub cost_hint: f64,
    /// Focus in output pixels from the centre (y down).
    pub focus: (f64, f64),
    pub generation: u64,
    pub live_capacity: usize,
    /// Generations with chunks still on the GPU.
    pub live_generations: &'a [u64],
    pub want_aux: bool,
}

pub(crate) struct SampleCache<B: GpuBackend> {
    pub config: CacheConfig,
    pub stats: CacheStats,
    alloc: Option<Alloc<B>>,
    cur: usize,
    epoch: Option<Epoch>,
    tiles: Vec<Tile>,
    batches: VecDeque<Batch>,
    serial: u64,
    /// Target ratio (texels per output pixel), the cap it steps down from.
    target: f64,
    cap: f64,
    /// Seconds of GPU per computed texel (completed batches), smoothed.
    cost: f64,
    /// Demand: texels a frame needs in units of the visible texels at the target, smoothed.
    demand: f64,
    calm_frames: u32,
    backlog: usize,
    prev_view: Option<(Viewport, Scale)>,
    /// GPU passes still running: (shade?, submission).
    pending: VecDeque<Pass<B>>,
    /// Per-slot cost estimate of the samples a tile holds (from the shade passes): its holder
    /// and epoch serial (valid while they match), iteration units per sample, largest escape.
    est: Vec<Est>,
    /// GPU seconds per iteration unit (completed batches against their estimate), smoothed.
    unit_cost: f64,
    /// What the last finished shade pass counted in the view: interior fraction and exterior
    /// iterations per sample (the interior-test statistics of a fresh preview).
    view_stats: Option<InteriorStats>,
    pipelines: Option<[B::Pipeline; 4]>,
    /// The limit of the last shade pass.
    last_limit: usize,
    /// Mean estimated units per sample of the last planned candidates.
    last_mean_units: f64,
    /// GPU seconds per planned frame (completed batches, empty frames as 0), smoothed; frames
    /// measured; the cache gave way to fresh previews for this gesture.
    frame_gpu: f64,
    frames_measured: usize,
    slow: bool,
    /// Tiles of the last batch and the cost per texel of the last completed one.
    last_tiles: usize,
    last_cost: f64,
    /// Phase timestamps of the last plan (trace).
    marks: Vec<(&'static str, Instant)>,
    /// Colour parameters and texture set the shaded tiles were coloured with.
    shade_key: Option<(Vec<u8>, usize)>,
    /// Generation of the job whose full level seeded the current epoch.
    pub seeded_generation: u64,
}

fn pmod(a: i64, m: i64) -> i64 {
    a.rem_euclid(m)
}

impl<B: GpuBackend> SampleCache<B> {
    pub fn new() -> Self {
        Self {
            config: CacheConfig::default(),
            stats: CacheStats::default(),
            alloc: None,
            cur: 0,
            epoch: None,
            tiles: Vec::new(),
            batches: VecDeque::new(),
            serial: 0,
            target: 1.0,
            cap: 1.0,
            cost: 0.0,
            demand: 0.0,
            calm_frames: 0,
            backlog: 0,
            prev_view: None,
            pending: VecDeque::new(),
            est: Vec::new(),
            unit_cost: 0.0,
            view_stats: None,
            pipelines: None,
            shade_key: None,
            marks: Vec::new(),
            last_tiles: 0,
            last_cost: 0.0,
            last_limit: 0,
            last_mean_units: 0.0,
            frame_gpu: 0.0,
            frames_measured: 0,
            slow: false,
            seeded_generation: 0,
        }
    }

    /// Forgets every sample (after a panic, or a change that alters samples).
    pub fn reset(&mut self) {
        self.epoch = None;
        self.batches.clear();
        self.prev_view = None;
        self.view_stats = None;
        self.shade_key = None;
        self.cost = 0.0;
        self.last_cost = 0.0;
        self.last_tiles = 0;
        self.unit_cost = 0.0;
        self.frame_gpu = 0.0;
        self.frames_measured = 0;
        self.slow = false;
        self.demand = 0.0;
    }

    fn pipelines(&mut self, backend: &B) -> Result<[B::Pipeline; 4], GpuError> {
        if let Some(p) = &self.pipelines {
            return Ok(p.clone());
        }
        let p = [
            backend.pipeline("scClear", &[])?,
            backend.pipeline("scResample", &[])?,
            backend.pipeline("scShade", &[])?,
            backend.pipeline("scShadeTiles", &[])?,
        ];
        self.pipelines = Some(p.clone());
        Ok(p)
    }

    /// Compiles the kernels now (renderer creation), not at the first gesture.
    pub fn warm_up(&mut self, backend: &B) {
        let _ = self.pipelines(backend);
    }

    /// GPU times of finished passes.
    fn poll(&mut self, backend: &B) {
        while let Some(pass) = self.pending.front() {
            let status = pass.sub.status();
            if status == SubmissionStatus::Pending {
                break;
            }
            let pass = self.pending.pop_front().expect("pass");
            if status != SubmissionStatus::Completed {
                continue;
            }
            let gpu = pass.sub.gpu_time();
            if !pass.tiles.is_empty()
                && let Some(a) = &self.alloc
                && self.epoch.as_ref().is_some_and(|e| e.serial == pass.serial)
            {
                // The shaded tiles' statistics -> cost estimates (the tile still holds them).
                let mut bytes = vec![0u8; a.wt * a.ht * 16];
                backend.read_buffer(&a.stats, 0, &mut bytes);
                let limit = pass.limit.max(1) as f64;
                for &(slot, holder) in &pass.tiles {
                    if self.tiles[slot].holder != holder {
                        continue;
                    }
                    let w = |k: usize| {
                        let o = slot * 16 + 4 * k;
                        u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
                    };
                    let (sum, esc, ir, max_n) = (w(0) as f64, w(1) as f64, w(2), w(3));
                    let (int, unres) = ((ir & 0xffff) as f64, (ir >> 16) as f64);
                    let valid = esc + int + unres;
                    if valid < 1.0 {
                        continue;
                    }
                    // Proven interior costs about what escaping costs nearby (unknown here).
                    let interior_unit = if esc > 0.0 { sum / esc } else { limit / 4.0 };
                    let units = (sum + int * interior_unit + unres * limit) / valid;
                    self.est[slot] = Est {
                        holder,
                        serial: pass.serial,
                        units: units as f32,
                        max_n: max_n.max(if unres > 0.0 { limit as u32 } else { 0 }),
                        esc: esc as u32,
                        sum: w(0),
                        interior: ir & 0xffff,
                        unresolved: ir >> 16,
                    };
                }
            }
            if self.config.trace && gpu > 0.002 {
                eprintln!(
                    "sample cache: {} pass {:.2} ms GPU",
                    if pass.shade { "shade" } else { "maintenance" },
                    gpu * 1000.0
                );
            }
            if pass.shade {
                self.stats.gpu_shade += gpu;
                self.stats.gpu_shades += 1;
            } else {
                self.stats.gpu_maintain += gpu;
                self.stats.gpu_passes += 1;
            }
        }
        // (Never more than a handful: the frames-in-flight limit bounds them.)
        while self.pending.len() > 64 {
            self.pending.pop_front();
        }
    }

    /// The largest ratio for a `width` x `height` view: `max_ratio`, lowered so the visible
    /// window holds at most `max_texels`.
    fn cap_for(&self, width: usize, height: usize) -> f64 {
        let n = (width * height) as f64;
        self.config
            .max_ratio
            .min((self.config.max_texels as f64 / n.max(1.0)).sqrt())
    }

    /// Makes sure textures for a `width` x `height` view at ratio cap exist; true if they were
    /// (re)allocated (every sample is gone).
    fn ensure_alloc(&mut self, backend: &B, width: usize, height: usize, want_aux: bool) -> Result<bool, GpuError> {
        let cap = self.cap_for(width, height);
        let key = (width, height, cap.to_bits());
        if let Some(a) = &mut self.alloc
            && a.for_view == key
        {
            if want_aux && a.sets[0].aux.is_none() {
                let (w, h) = (a.wt * TILE as usize, a.ht * TILE as usize);
                for s in &mut a.sets {
                    s.aux = Some(backend.create_texture(w, h, TextureFormat::Rg32Float, Storage::Private)?);
                }
                return Ok(true);
            }
            return Ok(false);
        }
        // The visible footprint at the cap, turned by up to ROTATION_SLACK, plus the compose
        // pass's neighbourhood and tile rounding.
        let (c, s) = (ROTATION_SLACK.cos(), ROTATION_SLACK.sin());
        let (w, h) = (width as f64 * cap, height as f64 * cap);
        let span_u = c * w + s * h + 8.0;
        let span_v = s * w + c * h + 8.0;
        let wt = (span_u / TILE as f64).ceil() as usize + 2;
        let ht = (span_v / TILE as f64).ceil() as usize + 2;
        let (tw, th) = (wt * TILE as usize, ht * TILE as usize);
        if tw > 32768 || th > 32768 {
            return Err(GpuError("sample cache: view too large".into()));
        }
        let make = || -> Result<CacheSet<B>, GpuError> {
            Ok(CacheSet {
                iter: backend.create_texture(tw, th, TextureFormat::Rg32Float, Storage::Private)?,
                de: backend.create_texture(tw, th, TextureFormat::Rg16Float, Storage::Private)?,
                aux: if want_aux {
                    Some(backend.create_texture(tw, th, TextureFormat::Rg32Float, Storage::Private)?)
                } else {
                    None
                },
            })
        };
        let sets = [make()?, make()?];
        let color = backend.create_texture(tw, th, TextureFormat::Rgba16Float, Storage::Private)?;
        let stats = backend.create_buffer(wt * ht * 16, Storage::Shared)?;
        self.est = vec![Est::default(); wt * ht];
        self.alloc = Some(Alloc {
            sets,
            color,
            stats,
            wt,
            ht,
            for_view: key,
        });
        self.cap = cap;
        self.tiles = vec![
            Tile {
                holder: (i64::MIN, i64::MIN),
                state: State::Empty,
                limit: 0,
                inflight: 0,
                shaded: false,
            };
            wt * ht
        ];
        Ok(true)
    }

    fn dims(&self) -> (usize, usize) {
        self.alloc.as_ref().map_or((0, 0), |a| (a.wt, a.ht))
    }

    fn slot(&self, t: (i64, i64)) -> usize {
        let (wt, ht) = self.dims();
        (pmod(t.1, ht as i64) as usize) * wt + pmod(t.0, wt as i64) as usize
    }

    /// Lattice coordinates (u, v) of a complex point.
    fn lattice_of(e: &Epoch, c: &Center) -> (f64, f64) {
        let (dx, dy) = c.delta_in(&e.anchor, e.h);
        let (co, si) = rot(e.angle);
        (dx * co + dy * si, dx * si - dy * co)
    }

    /// The view on the current level: centre (lattice coordinates), ratio, rotation, the
    /// half extents of its footprint (lattice texels).
    fn view_geometry(e: &Epoch, view: &Viewport, ppp: f64, width: usize, height: usize) -> Geometry {
        let s = view.scale * ppp;
        let nr = s.ratio(e.h);
        let c = Self::lattice_of(e, &view.center);
        let d = view.angle - e.angle;
        let (cd, sd) = rot(d);
        let (hw, hh) = (width as f64 / 2.0, height as f64 / 2.0);
        Geometry {
            centre: c,
            ratio: nr,
            rot: (cd, sd),
            eu: nr * (cd.abs() * hw + sd.abs() * hh),
            ev: nr * (sd.abs() * hw + cd.abs() * hh),
            delta: d,
        }
    }

    /// Lattice tile range the footprint (plus `margin` texels) covers.
    fn tile_range(g: &Geometry, margin: f64) -> (i64, i64, i64, i64) {
        let f = |x: f64| (x / TILE as f64).floor() as i64;
        (
            f(g.centre.0 - g.eu - margin),
            f(g.centre.0 + g.eu + margin),
            f(g.centre.1 - g.ev - margin),
            f(g.centre.1 + g.ev + margin),
        )
    }

    /// Starts an epoch: the lattice through the view centre at the view's angle, all tiles
    /// empty (cleared on the GPU).
    fn new_epoch(&mut self, inp: &PlanInput, enc: &mut Maintenance<B>) -> bool {
        let s = inp.view.scale * inp.ppp;
        let target = self.target.min(self.cap);
        let mut e = Epoch {
            regime: inp.regime,
            anchor: inp.view.center.clone(),
            angle: inp.view.angle,
            h: s / target,
            win0: (0, 0),
            serial: 0,
        };
        let g = Self::view_geometry(&e, inp.view, inp.ppp, inp.width, inp.height);
        let (x0, x1, y0, y1) = Self::tile_range(&g, 2.0);
        let (wt, ht) = self.dims();
        if (x1 - x0 + 1) as usize > wt || (y1 - y0 + 1) as usize > ht {
            return false;
        }
        e.win0 = (
            x0 - (wt as i64 - (x1 - x0 + 1)) / 2,
            y0 - (ht as i64 - (y1 - y0 + 1)) / 2,
        );
        self.serial += 1;
        e.serial = self.serial;
        self.batches.clear();
        self.epoch = Some(e);
        self.stats.epochs += 1;
        // Every tile of the window: empty, cleared.
        let e = self.epoch.as_ref().expect("epoch").clone();
        let mut clear = Vec::with_capacity(wt * ht);
        for ty in e.win0.1..e.win0.1 + ht as i64 {
            for tx in e.win0.0..e.win0.0 + wt as i64 {
                let slot = self.slot((tx, ty));
                self.tiles[slot] = Tile {
                    holder: (tx, ty),
                    state: State::Empty,
                    limit: 0,
                    inflight: 0,
                    shaded: false,
                };
                clear.push(slot as u32);
            }
        }
        enc.clear.extend(clear);
        true
    }

    /// Moves the window (whole tiles, as little as possible) so it holds the footprint;
    /// tiles that enter are cleared. False if the footprint does not fit.
    fn slide(&mut self, g: &Geometry, enc: &mut Maintenance<B>) -> bool {
        let (wt, ht) = self.dims();
        let (x0, x1, y0, y1) = Self::tile_range(g, 2.0);
        if (x1 - x0 + 1) as usize > wt || (y1 - y0 + 1) as usize > ht {
            return false;
        }
        let e = self.epoch.as_mut().expect("epoch");
        let mut w = e.win0;
        if x0 < w.0 {
            w.0 = x0;
        } else if x1 >= w.0 + wt as i64 {
            w.0 = x1 - wt as i64 + 1;
        }
        if y0 < w.1 {
            w.1 = y0;
        } else if y1 >= w.1 + ht as i64 {
            w.1 = y1 - ht as i64 + 1;
        }
        if w == e.win0 {
            return true;
        }
        let old = e.win0;
        e.win0 = w;
        self.stats.slides += 1;
        let (wt_i, ht_i) = (wt as i64, ht as i64);
        for ty in w.1..w.1 + ht_i {
            // (Rows inside the old window only gain the columns it did not cover.)
            let row_old = ty >= old.1 && ty < old.1 + ht_i;
            for tx in w.0..w.0 + wt_i {
                if row_old && tx >= old.0 && tx < old.0 + wt_i {
                    continue;
                }
                let slot = self.slot((tx, ty));
                let t = &mut self.tiles[slot];
                if t.holder != (tx, ty) {
                    *t = Tile {
                        holder: (tx, ty),
                        state: State::Empty,
                        limit: 0,
                        inflight: 0,
                        shaded: false,
                    };
                    enc.clear.push(slot as u32);
                }
            }
        }
        true
    }

    /// Re-anchors the lattice at its point `a` (lattice index, multiples of 16) and scales the
    /// spacing by `factor` (1/2: finer, 2: coarser). Returns the old epoch.
    fn reanchor(e: &mut Epoch, a: (i64, i64), factor: f64, bits: usize) {
        let (co, si) = rot(e.angle);
        let (ax, ay) = (a.0 as f64, a.1 as f64);
        e.anchor = e
            .anchor
            .with_min_precision(bits)
            .offset_scaled(ax * co + ay * si, ax * si - ay * co, e.h);
        e.h = e.h * factor;
    }

    /// Switches to the finer (`finer`) or coarser level around the view: re-anchors at the
    /// tile-aligned lattice point nearest the view centre, places the new window, resamples
    /// the coinciding samples into the other texture set and derives tile states.
    fn switch_level(&mut self, inp: &PlanInput, finer: bool, enc: &mut Maintenance<B>) -> bool {
        let Some(old) = self.epoch.clone() else { return false };
        let g = Self::view_geometry(&old, inp.view, inp.ppp, inp.width, inp.height);
        let a = (
            TILE * (g.centre.0 / TILE as f64).round() as i64,
            TILE * (g.centre.1 / TILE as f64).round() as i64,
        );
        let mut e = old.clone();
        let realign = finer && g.delta.abs() > REALIGN;
        Self::reanchor(
            &mut e,
            a,
            if finer { 0.5 } else { 2.0 },
            inp.view.center.precision_bits(),
        );
        if realign {
            // The lattice turns with the view (no sample coincides): a fresh level.
            e.angle = inp.view.angle;
        }
        let ng = Self::view_geometry(&e, inp.view, inp.ppp, inp.width, inp.height);
        let (wt, ht) = self.dims();
        let (x0, x1, y0, y1) = Self::tile_range(&ng, 2.0);
        if (x1 - x0 + 1) as usize > wt || (y1 - y0 + 1) as usize > ht {
            return false;
        }
        e.win0 = (
            x0 - (wt as i64 - (x1 - x0 + 1)) / 2,
            y0 - (ht as i64 - (y1 - y0 + 1)) / 2,
        );
        self.serial += 1;
        e.serial = self.serial;
        // Tile states of the new window from the old ones (old tile coordinates of new lattice
        // index I: a + I / 2 (finer) or a + 2 I (coarser), a tile-aligned).
        let old_state = |s: &Self, t: (i64, i64)| -> Option<Tile> {
            let x = &s.tiles[s.slot(t)];
            (x.holder == t
                && t.0 >= old.win0.0
                && t.0 < old.win0.0 + wt as i64
                && t.1 >= old.win0.1
                && t.1 < old.win0.1 + ht as i64)
                .then_some(*x)
        };
        let (at, bt) = (a.0 / TILE, a.1 / TILE);
        let mut next = Vec::with_capacity(wt * ht);
        for ty in e.win0.1..e.win0.1 + ht as i64 {
            for tx in e.win0.0..e.win0.0 + wt as i64 {
                let (state, limit) = if realign {
                    (State::Empty, 0)
                } else if finer {
                    // New tile tx covers old indices a + 8 tx ... + 7: one old tile.
                    match old_state(self, (at + tx.div_euclid(2), bt + ty.div_euclid(2))) {
                        Some(o) if o.state != State::Empty || o.inflight != 0 => (State::Partial, o.limit),
                        _ => (State::Empty, 0),
                    }
                } else {
                    // New tile tx covers old indices a + 32 tx ... + 31: 2 x 2 old tiles.
                    let mut full = true;
                    let mut any = false;
                    let mut limit = u32::MAX;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            match old_state(self, (at + 2 * tx + dx, bt + 2 * ty + dy)) {
                                Some(o) => {
                                    full &= o.state == State::Full && o.inflight == 0;
                                    any |= o.state != State::Empty || o.inflight != 0;
                                    limit = limit.min(o.limit);
                                }
                                None => full = false,
                            }
                        }
                    }
                    if full {
                        (State::Full, limit)
                    } else if any {
                        (State::Partial, limit.min(u32::MAX - 1))
                    } else {
                        (State::Empty, 0)
                    }
                };
                next.push(((tx, ty), state, limit));
            }
        }
        // Cost estimates of the new tiles from their source tiles (iterations per sample hold;
        // the old level's estimate is read before any tile is rewritten).
        let old_est = |s: &Self, t: (i64, i64)| -> Option<Est> {
            let x = s.est[s.slot(t)];
            (x.holder == t && x.serial == old.serial).then_some(x)
        };
        let mut next_est = Vec::with_capacity(next.len());
        for &(t, _, _) in &next {
            let est = if realign {
                None
            } else if finer {
                old_est(self, (at + t.0.div_euclid(2), bt + t.1.div_euclid(2)))
            } else {
                let mut sum = 0.0f32;
                let mut max_n = 0u32;
                let mut k = 0;
                for dy in 0..2 {
                    for dx in 0..2 {
                        if let Some(x) = old_est(self, (at + 2 * t.0 + dx, bt + 2 * t.1 + dy)) {
                            sum += x.units;
                            max_n = max_n.max(x.max_n);
                            k += 1;
                        }
                    }
                }
                (k > 0).then_some(Est {
                    holder: t,
                    serial: 0,
                    units: sum / k as f32,
                    max_n,
                    ..Est::default()
                })
            };
            next_est.push(est.map(|x| Est {
                holder: t,
                serial: e.serial,
                ..x
            }));
        }
        for (i, (t, state, limit)) in next.into_iter().enumerate() {
            let slot = self.slot(t);
            if let Some(x) = next_est[i] {
                self.est[slot] = x;
            }
            self.tiles[slot] = Tile {
                holder: t,
                state,
                limit,
                inflight: 0,
                shaded: false,
            };
        }
        if realign {
            enc.clear.clear();
            enc.clear.extend((0..wt * ht).map(|s| s as u32));
        } else {
            // Old window texel of new lattice index 0: a - 16 win0_old.
            let (tw, th) = (wt as u32 * TILE as u32, ht as u32 * TILE as u32);
            let wrap = |w: (i64, i64)| [pmod(TILE * w.0, tw as i64) as u32, pmod(TILE * w.1, th as i64) as u32];
            enc.resample = Some((
                CacheParams {
                    new_origin: [(TILE * e.win0.0) as i32, (TILE * e.win0.1) as i32],
                    old_base: [(a.0 - TILE * old.win0.0) as i32, (a.1 - TILE * old.win0.1) as i32],
                    new_size: [tw, th],
                    new_wrap: wrap(e.win0),
                    old_size: [tw, th],
                    old_wrap: wrap(old.win0),
                    num: if finer { 1 } else { 2 },
                    shift: u32::from(finer),
                    de_scale: if finer { 2.0 } else { 0.5 },
                    ..CacheParams::default()
                },
                self.cur,
            ));
            // Pending clears refer to the old set: the resample rewrites every texel.
            enc.clear.clear();
            self.cur = 1 - self.cur;
        }
        self.batches.clear();
        self.epoch = Some(e);
        if finer {
            self.stats.finer += 1;
        } else {
            self.stats.coarser += 1;
        }
        true
    }

    /// Keeps lattice indices small: re-anchors by whole windows when the window drifted far
    /// (slots and texels stay where they are).
    fn recentre(&mut self) {
        let (wt, ht) = self.dims();
        let Some(e) = &mut self.epoch else { return };
        let far = 1 << 10;
        let (kx, ky) = (e.win0.0.div_euclid(wt as i64 * 4), e.win0.1.div_euclid(ht as i64 * 4));
        if e.win0.0.abs() < far && e.win0.1.abs() < far || (kx == 0 && ky == 0) {
            return;
        }
        let (sx, sy) = (kx * wt as i64 * 4, ky * ht as i64 * 4);
        let bits = e.anchor.precision_bits();
        Self::reanchor(e, (TILE * sx, TILE * sy), 1.0, bits);
        e.win0 = (e.win0.0 - sx, e.win0.1 - sy);
        for t in &mut self.tiles {
            t.holder = (t.holder.0 - sx, t.holder.1 - sy);
        }
        for b in &mut self.batches {
            for t in &mut b.tiles {
                t.1 = (t.1.0 - sx, t.1.1 - sy);
            }
        }
    }

    /// Per-frame motion: updates the demand estimate (texels a frame needs, in units of the
    /// visible texels at the target ratio).
    fn note_motion(&mut self, view: &Viewport, ppp: f64, width: usize, height: usize) {
        let s = view.scale * ppp;
        if let Some((pv, ps)) = &self.prev_view {
            let zr = ps.ratio(s).ln();
            let (dx, dy) = view.unrotate_vec(view.center.delta_in(&pv.center, s));
            let (w, h) = (width as f64, height as f64);
            let exposure = (dx.abs() * h + dy.abs() * w) / (w * h);
            // Zoom-in: a finer level every ln 2 e-folds, 3/4 of its texels new (1.08 per
            // e-fold); zoom-out exposes 2 per e-fold at the edges.
            let zoom = if zr > 0.0 { 1.08 * zr } else { -2.0 * zr };
            let turn = 0.3 * (view.angle - pv.angle).abs();
            let f = (exposure + zoom + turn).min(1.0);
            if f.is_finite() {
                self.demand = 0.8 * self.demand + 0.2 * f;
            }
        }
        self.prev_view = Some((view.clone(), s));
    }

    /// Chooses the target ratio from the frame budget and the demand.
    fn steer(&mut self, inp: &PlanInput) {
        let cost = if self.cost > 0.0 { self.cost } else { inp.cost_hint };
        if cost.is_nan() || cost <= 0.0 {
            return;
        }
        let budget_texels = inp.budget / cost;
        let n = (inp.width * inp.height) as f64;
        let afford = (budget_texels / (n * self.demand.max(1e-4))).sqrt();
        if self.config.trace {
            eprintln!(
                "sample cache: steer cost {:.3e} s/texel budget {:.1} ms demand {:.4} afford {:.3} target {:.3} backlog {}",
                cost,
                inp.budget * 1000.0,
                self.demand,
                afford,
                self.target,
                self.backlog
            );
        }
        let mut fit = self.cap;
        let mut steps = 0;
        while fit > afford && steps < MAX_STEPS {
            fit *= 0.5;
            steps += 1;
        }
        let floor = self.cap * 0.5f64.powi(MAX_STEPS as i32);
        // Hysteresis both ways (a target change switches the level): down when the budget no
        // longer affords 0.8 x the target, up when it affords 2.5 x and the cache kept up for a
        // while (a finer level costs 3/4 of its texels at once). Level switches make backlogs
        // by design; the demand model already counts them.
        if afford < 0.8 * self.target {
            self.target = fit.min(self.target * 0.5);
            self.calm_frames = 0;
        } else if afford >= 2.5 * self.target && self.target < self.cap * 0.99 {
            if self.backlog == 0 {
                self.calm_frames += 1;
            } else {
                self.calm_frames = 0;
            }
            if self.calm_frames >= 8 {
                self.target = (self.target * 2.0).min(self.cap);
                self.calm_frames = 0;
            }
        } else {
            self.calm_frames = 0;
        }
        self.target = self.target.clamp(floor, self.cap);
    }

    /// Plans a gesture frame (kb: techniques/sample-reuse): epoch and level for the view,
    /// window slide, the maintenance passes (committed here, before the frame's chunks), and
    /// the tiles to compute within the budget. None: the cache cannot serve this view (the
    /// caller renders a fresh preview).
    pub fn plan(&mut self, backend: &B, inp: &PlanInput) -> Option<Plan<B>> {
        let t0 = Instant::now();
        self.marks.clear();
        let out = self.plan_inner(backend, inp);
        let dt = t0.elapsed().as_secs_f64();
        if self.config.trace && dt > 0.001 {
            let mut last = t0;
            let parts: Vec<String> = self
                .marks
                .iter()
                .map(|(l, t)| {
                    let d = t.duration_since(last).as_secs_f64() * 1000.0;
                    last = *t;
                    format!("{l} {d:.2}")
                })
                .collect();
            eprintln!("sample cache: plan phases (ms): {}", parts.join(", "));
        }
        if self.config.trace && dt > 0.001 {
            eprintln!(
                "sample cache: plan {:.2} ms, backlog {} texels, epochs {} switches {}/{} cleared {}",
                dt * 1000.0,
                self.backlog,
                self.stats.epochs,
                self.stats.finer,
                self.stats.coarser,
                self.stats.cleared_tiles
            );
        }
        self.stats.cpu_plan += dt;
        self.stats.cpu_plan_max = self.stats.cpu_plan_max.max(dt);
        if out.is_some() {
            self.stats.frames += 1;
        } else {
            self.stats.fallback_frames += 1;
        }
        out
    }

    fn plan_inner(&mut self, backend: &B, inp: &PlanInput) -> Option<Plan<B>> {
        self.poll(backend);
        if !self.config.enabled || inp.width == 0 || inp.height == 0 {
            return None;
        }
        let pipelines = self.pipelines(backend).ok()?;
        if self.ensure_alloc(backend, inp.width, inp.height, inp.want_aux).ok()? {
            self.epoch = None;
        }
        self.note_motion(inp.view, inp.ppp, inp.width, inp.height);
        // Abandoned batches (superseded before they finished): their tiles are free again.
        let live = inp.live_generations;
        let tiles = &mut self.tiles;
        let stats = &mut self.stats;
        self.batches.retain(|b| {
            if b.generation == inp.generation || live.contains(&b.generation) {
                return true;
            }
            stats.iterations += b.iterations;
            for &(slot, holder) in &b.tiles {
                let t = &mut tiles[slot];
                if t.holder == holder && t.inflight == b.generation {
                    t.inflight = 0;
                    if t.state == State::Empty {
                        t.state = State::Partial;
                    }
                }
            }
            false
        });
        // Regime: a new epoch when the samples would differ.
        if self.epoch.as_ref().is_some_and(|e| e.regime != inp.regime) {
            self.epoch = None;
        }
        self.marks.push(("prologue", Instant::now()));
        self.steer(inp);
        let mut m = Maintenance::default();
        if self.epoch.is_none() && !self.new_epoch(inp, &mut m) {
            return None;
        }
        // Level: within (FINER_AT, 1] x target texels per output pixel.
        for _ in 0..6 {
            let e = self.epoch.as_ref().expect("epoch");
            let nr = (inp.view.scale * inp.ppp).ratio(e.h);
            let finer = nr < FINER_AT * self.target;
            let coarser = nr > self.target * 1.0001;
            if !(finer || coarser) {
                break;
            }
            // (Several switches in one frame: chain the resample passes through encoding.)
            if m.resample.is_some() {
                self.encode_maintenance(backend, &pipelines, &mut m);
            }
            if !self.switch_level(inp, finer, &mut m) {
                self.epoch = None;
                if !self.new_epoch(inp, &mut m) {
                    return None;
                }
                break;
            }
        }
        let e = self.epoch.as_ref().expect("epoch").clone();
        let g = Self::view_geometry(&e, inp.view, inp.ppp, inp.width, inp.height);
        if !self.slide(&g, &mut m) {
            // Turned or resized beyond the window: start over at the view's angle.
            self.epoch = None;
            m = Maintenance::default();
            if !self.new_epoch(inp, &mut m) {
                return None;
            }
        }
        self.marks.push(("level+slide", Instant::now()));
        self.recentre();
        let tm = Instant::now();
        let ncl = m.clear.len();
        self.encode_maintenance(backend, &pipelines, &mut m);
        if self.config.trace && tm.elapsed().as_secs_f64() > 0.0005 {
            eprintln!(
                "sample cache: maintenance encode {:.2} ms ({ncl} clears)",
                tm.elapsed().as_secs_f64() * 1000.0
            );
        }
        let e = self.epoch.as_ref().expect("epoch").clone();
        let g = Self::view_geometry(&e, inp.view, inp.ppp, inp.width, inp.height);

        self.marks.push(("maintenance", Instant::now()));
        // Tiles to compute: visible, not (being) computed, or computed at a limit 25 % behind.
        let (x0, x1, y0, y1) = Self::tile_range(&g, 2.0);
        let (hw, hh) = (inp.width as f64 / 2.0, inp.height as f64 / 2.0);
        let rad = 12.0 / g.ratio + 2.0 / g.ratio;
        let limit = inp.limit as u32;
        let mut cands: Vec<(u64, u32)> = Vec::new();
        let mut visible = 0usize;
        // The view's interior-test statistics from its tiles' shade statistics: escaped, their
        // escape counts, interior, unresolved at the limit.
        let mut vs = [0.0f64; 4];
        let mut reused = 0usize;
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                // Tile centre in output pixels from the view centre (y down).
                let (du, dv) = (
                    (tx * TILE) as f64 + 7.5 - g.centre.0,
                    (ty * TILE) as f64 + 7.5 - g.centre.1,
                );
                let px = (g.rot.0 * du - g.rot.1 * dv) / g.ratio;
                let py = (g.rot.1 * du + g.rot.0 * dv) / g.ratio;
                if px.abs() > hw + rad || py.abs() > hh + rad {
                    continue;
                }
                visible += 1;
                let slot = self.slot((tx, ty));
                let x = &self.est[slot];
                if x.holder == (tx, ty) && x.serial == e.serial {
                    vs[0] += f64::from(x.esc);
                    vs[1] += f64::from(x.sum);
                    vs[2] += f64::from(x.interior);
                    vs[3] += f64::from(x.unresolved);
                }
                let t = &self.tiles[slot];
                debug_assert_eq!(t.holder, (tx, ty));
                if t.inflight != 0 {
                    reused += 1;
                    continue;
                }
                let class: u64 = match t.state {
                    State::Empty => 0,
                    State::Partial => 1,
                    State::Full if (t.limit as u64) * 5 < (limit as u64) * 4 => 2,
                    State::Full => {
                        reused += 1;
                        continue;
                    }
                };
                let (fx, fy) = (px - inp.focus.0, py - inp.focus.1);
                let d2 = ((fx * fx + fy * fy) as u64).min((1 << 40) - 1);
                cands.push(((class << 40) | d2, slot as u32));
            }
        }
        let n_vs = vs[0] + vs[2] + vs[3];
        if n_vs >= 256.0 {
            self.view_stats = Some(InteriorStats {
                fraction: vs[2] / n_vs,
                exterior_iterations: (vs[1] + vs[3] * inp.limit as f64) / n_vs,
                unresolved: vs[3] / n_vs,
            });
        }
        self.stats.visible_texels += visible * 256;
        self.stats.reused_texels += reused * 256;
        self.backlog = cands.len() * 256;
        // Cost estimates (iteration units per sample) of the candidates: their own tile's from
        // its last shade, else the mean over the visible ones known.
        let est_of = |s: &Self, slot: usize| -> Option<f64> {
            let x = s.est[slot];
            (x.holder == s.tiles[slot].holder && x.serial == e.serial).then_some(f64::from(x.units))
        };
        // The cost estimate lags (new tiles may be far denser than the last ones): a batch is
        // at most twice the last one plus 8 tiles, and the last batch's own cost counts.
        let cost = if self.cost > 0.0 {
            self.cost.max(self.last_cost)
        } else {
            inp.cost_hint
        };
        let budget_tiles = if cost > 0.0 {
            ((inp.budget / cost) / 256.0).floor() as usize
        } else {
            64
        }
        .min(2 * self.last_tiles + 8);
        let mut n = if inp.bla_pending {
            0
        } else {
            cands.len().min(inp.live_capacity / 256)
        };
        // With a calibrated cost per iteration unit, the batch is the prefix (in priority
        // order) whose estimated GPU time fits the budget; else the texel model above.
        let by_units = self.unit_cost > 0.0;
        if !by_units {
            n = n.min(budget_tiles.max(1));
        }
        let (wt, ht) = self.dims();
        let a = self.alloc.as_ref().expect("alloc");
        let set = a.sets[self.cur].clone();
        let color = a.color.clone();
        let (tw, th) = (wt * TILE as usize, ht * TILE as usize);
        let wrap_x = pmod(TILE * e.win0.0, tw as i64) as u32;
        let wrap_y = pmod(TILE * e.win0.1, th as i64) as u32;
        let frame = LatticeFrame {
            anchor: e.anchor.clone(),
            h: e.h,
            rot: rot(e.angle),
            origin: (TILE * e.win0.0, TILE * e.win0.1),
            // Kernel: window texel of texel gid = (gid + (size - wrap)) mod size.
            wrap: ((tw as u32 - wrap_x) % tw as u32) | (((th as u32 - wrap_y) % th as u32) << 16),
        };
        self.marks.push(("scan", Instant::now()));
        let mut order = None;
        let units_planned: f64;
        if n > 0 {
            if by_units {
                // (Sort only a generous prefix: at least the texel model's count, several times.)
                let k = n.min(4 * budget_tiles.max(16) + 64);
                if k < cands.len() {
                    cands.select_nth_unstable(k - 1);
                    cands.truncate(k);
                }
                cands.sort_unstable();
                let (mut sum, mut known) = (0.0, 0usize);
                for c in &cands {
                    if let Some(u) = est_of(self, c.1 as usize) {
                        sum += u;
                        known += 1;
                    }
                }
                let mean = if known > 0 {
                    sum / known as f64
                } else {
                    self.last_mean_units.max(1.0)
                };
                self.last_mean_units = mean;
                let tile_budget = inp.budget / (self.unit_cost * 256.0);
                let mut used = 0.0;
                let mut take = 0;
                for c in &cands {
                    let u = est_of(self, c.1 as usize).unwrap_or(mean);
                    if take > 0 && used + u > tile_budget {
                        break;
                    }
                    used += u;
                    take += 1;
                }
                n = take;
                units_planned = used * 256.0;
                cands.truncate(n);
            } else {
                if n < cands.len() {
                    cands.select_nth_unstable(n - 1);
                    cands.truncate(n);
                }
                cands.sort_unstable();
                let mean = self.last_mean_units.max(1.0);
                units_planned = cands
                    .iter()
                    .map(|c| est_of(self, c.1 as usize).unwrap_or(mean))
                    .sum::<f64>()
                    * 256.0;
            }
            let slots: Vec<u32> = cands.iter().map(|c| c.1).collect();
            order = Some(backend.create_buffer_with_data(bytemuck::cast_slice(&slots)).ok()?);
            let mut tiles = Vec::with_capacity(n);
            for &s in &slots {
                let t = &mut self.tiles[s as usize];
                t.inflight = inp.generation;
                tiles.push((s as usize, t.holder));
            }
            self.batches.push_back(Batch {
                generation: inp.generation,
                serial: e.serial,
                tiles,
                texels: n * 256,
                limit,
                gpu: 0.0,
                iterations: 0.0,
                units: units_planned,
            });
            self.last_tiles = n;
            self.stats.tiles_planned += n;
            self.stats.texels_planned += n * 256;
        }
        if n == 0 {
            self.note_frame_gpu(0.0);
        }
        self.marks.push(("batch", Instant::now()));
        Some(Plan {
            set,
            color,
            width: tw,
            height: th,
            frame,
            order,
            tiles: n,
            ratio: g.ratio,
        })
    }

    /// Commits the pending clear / resample passes (one command buffer).
    fn encode_maintenance(&mut self, backend: &B, pipelines: &[B::Pipeline; 4], m: &mut Maintenance<B>) {
        if m.clear.is_empty() && m.resample.is_none() && m.seed.is_none() {
            return;
        }
        let Some(a) = &self.alloc else { return };
        let (tw, th) = (a.wt * TILE as usize, a.ht * TILE as usize);
        let Ok(mut enc) = backend.begin("sample cache") else {
            return;
        };
        if let Some((p, from)) = m.resample.take() {
            let (src, dst) = (&a.sets[from], &a.sets[1 - from]);
            let mut p = p;
            p.has_aux = u32::from(src.aux.is_some() && dst.aux.is_some());
            enc.set_pipeline(&pipelines[1]);
            enc.set_texture(0, &src.iter);
            enc.set_texture(1, &src.de);
            enc.set_texture(2, src.aux.as_ref().unwrap_or(&src.iter));
            enc.set_texture(3, &dst.iter);
            enc.set_texture(4, &dst.de);
            enc.set_texture(5, dst.aux.as_ref().unwrap_or(&dst.iter));
            enc.set_bytes(0, bytes_of(&p));
            enc.dispatch_threads([tw, th, 1], [16, 16, 1]);
        }
        if let Some((p, iter, de, aux)) = m.seed.take() {
            let dst = &a.sets[self.cur];
            let mut p = p;
            p.has_aux = u32::from(aux.is_some() && dst.aux.is_some());
            enc.set_pipeline(&pipelines[1]);
            enc.set_texture(0, &iter);
            enc.set_texture(1, &de);
            enc.set_texture(2, aux.as_ref().unwrap_or(&iter));
            enc.set_texture(3, &dst.iter);
            enc.set_texture(4, &dst.de);
            enc.set_texture(5, dst.aux.as_ref().unwrap_or(&dst.iter));
            enc.set_bytes(0, bytes_of(&p));
            enc.dispatch_threads([tw, th, 1], [16, 16, 1]);
        }
        if !m.clear.is_empty() {
            let count = m.clear.len();
            self.stats.cleared_tiles += count;
            let p = CacheParams {
                new_size: [tw as u32, th as u32],
                count: count as u32,
                ..CacheParams::default()
            };
            if let Ok(list) = backend.create_buffer_with_data(bytemuck::cast_slice(&m.clear)) {
                enc.set_pipeline(&pipelines[0]);
                enc.set_texture(0, &a.sets[self.cur].iter);
                enc.set_bytes(0, bytes_of(&p));
                enc.set_buffer(1, &list, 0);
                enc.dispatch_threads([count * 256, 1, 1], [256, 1, 1]);
            }
            m.clear.clear();
        }
        let s = enc.commit();
        self.pending.push_back(Pass {
            shade: false,
            sub: s,
            limit: 0,
            tiles: Vec::new(),
            serial: 0,
        });
    }

    /// A chunk of `generation` finished: a batch whose texels are all done marks its tiles
    /// full. Called for every harvested chunk of a level 0, superseded jobs included.
    pub fn on_chunk(&mut self, generation: u64, admitted: usize, live_out: usize, gpu: f64, iterations: f64) {
        let Some(i) = self.batches.iter().position(|b| b.generation == generation) else {
            return;
        };
        let b = &mut self.batches[i];
        b.gpu += gpu;
        b.iterations += iterations;
        if admitted < b.texels || live_out > 0 {
            return;
        }
        let b = self.batches.remove(i).expect("batch");
        self.note_frame_gpu(b.gpu);
        self.stats.texels_done += b.texels;
        self.stats.iterations += b.iterations;
        if b.gpu > 0.0 && b.texels > 0 {
            let c = b.gpu / b.texels as f64;
            self.last_cost = c;
            if b.units > 0.0 {
                let u = b.gpu / b.units;
                self.unit_cost = if self.unit_cost > 0.0 {
                    0.7 * self.unit_cost + 0.3 * u
                } else {
                    u
                };
            }
            self.cost = if self.cost > 0.0 { 0.7 * self.cost + 0.3 * c } else { c };
        }
        if self.epoch.as_ref().is_none_or(|e| e.serial != b.serial) {
            return;
        }
        for (slot, holder) in b.tiles {
            let t = &mut self.tiles[slot];
            if t.holder == holder && t.inflight == generation {
                t.inflight = 0;
                t.state = State::Full;
                t.limit = b.limit;
            }
        }
    }

    /// How `view` maps onto the cache, if it has an epoch.
    pub fn map_for(&self, view: &Viewport, ppp: f64, width: usize, height: usize) -> Option<CacheMap> {
        let e = self.epoch.as_ref()?;
        let (wt, ht) = self.dims();
        let g = Self::view_geometry(e, view, ppp, width, height);
        let (tw, th) = (wt * TILE as usize, ht * TILE as usize);
        Some(CacheMap {
            centre: [
                g.centre.0 - (TILE * e.win0.0) as f64,
                g.centre.1 - (TILE * e.win0.1) as f64,
            ],
            ratio: g.ratio,
            rot: [g.rot.0, g.rot.1],
            wrap: [
                pmod(TILE * e.win0.0, tw as i64) as u32,
                pmod(TILE * e.win0.1, th as i64) as u32,
            ],
            size: (tw, th),
        })
    }

    /// Shades the cache texels the view shows into the colour texture (own command buffer,
    /// committed now, so before the compose pass that reads it). Returns the colour texture
    /// and the mapping.
    #[allow(clippy::too_many_arguments)]
    pub fn shade(
        &mut self,
        backend: &B,
        view: &Viewport,
        ppp: f64,
        width: usize,
        height: usize,
        colors: &ColorParams,
        palette: &B::Buffer,
        limit: usize,
    ) -> Option<(B::Texture, CacheMap)> {
        let t0 = Instant::now();
        self.poll(backend);
        let map = self.map_for(view, ppp, width, height)?;
        let pipelines = self.pipelines(backend).ok()?;
        let a = self.alloc.as_ref()?;
        let e = self.epoch.as_ref()?;
        let g = Self::view_geometry(e, view, ppp, width, height);
        let (tw, th) = map.size;
        // The footprint's bounding box plus the compose pass's reach (4 texels), in window texels.
        // Tile-aligned, so a tile is either shaded whole or not at all.
        let ox = TILE * e.win0.0;
        let oy = TILE * e.win0.1;
        let al = |v: i64, lo: bool| {
            if lo {
                v.div_euclid(TILE) * TILE
            } else {
                (v + TILE - 1).div_euclid(TILE) * TILE
            }
        };
        let x0 = al((g.centre.0 - g.eu).floor() as i64 - 4 - ox, true).clamp(0, tw as i64) as usize;
        let x1 = al((g.centre.0 + g.eu).ceil() as i64 + 5 - ox, false).clamp(0, tw as i64) as usize;
        let y0 = al((g.centre.1 - g.ev).floor() as i64 - 4 - oy, true).clamp(0, th as i64) as usize;
        let y1 = al((g.centre.1 + g.ev).ceil() as i64 + 5 - oy, false).clamp(0, th as i64) as usize;
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let set = a.sets[self.cur].clone();
        let color = a.color.clone();
        let win0 = e.win0;
        let mut c = *colors;
        c.de_scale = g.ratio as f32;
        let key = (bytes_of(&c).to_vec(), self.cur);
        let full = self.shade_key.as_ref() != Some(&key);
        if full {
            for t in &mut self.tiles {
                t.shaded = false;
            }
            self.shade_key = Some(key);
        }
        // Tiles of the rectangle whose colours are stale: all after a colour change (the zoom
        // changes the pixel size every frame), else those whose samples changed (a pan).
        let mut list: Vec<u32> = Vec::new();
        let mut covered: Vec<(usize, (i64, i64))> = Vec::new();
        for ty in (y0 / 16) as i64..(y1 / 16) as i64 {
            for tx in (x0 / 16) as i64..(x1 / 16) as i64 {
                let slot = self.slot((win0.0 + tx, win0.1 + ty));
                let t = &mut self.tiles[slot];
                if full || !t.shaded || t.inflight != 0 {
                    if !full {
                        list.push(slot as u32);
                    }
                    covered.push((slot, t.holder));
                    t.shaded = true;
                }
            }
        }
        let serial = e.serial;
        self.last_limit = limit;
        let p = CacheParams {
            new_size: [tw as u32, th as u32],
            new_wrap: map.wrap,
            rect_origin: [x0 as u32, y0 as u32],
            rect_size: [(x1 - x0) as u32, (y1 - y0) as u32],
            count: list.len() as u32,
            limit: limit as u32,
            ..CacheParams::default()
        };
        if !full && list.is_empty() {
            self.stats.cpu_shade += t0.elapsed().as_secs_f64();
            return Some((color, map));
        }
        let mut enc = backend.begin("sample cache shade").ok()?;
        enc.set_texture(0, &set.iter);
        enc.set_texture(1, &set.de);
        enc.set_texture(2, set.aux.as_ref().unwrap_or(&set.iter));
        enc.set_texture(3, &color);
        enc.set_bytes(0, bytes_of(&c));
        enc.set_buffer(1, palette, 0);
        enc.set_bytes(2, bytes_of(&p));
        enc.set_buffer(4, &a.stats, 0);
        if full {
            enc.set_pipeline(&pipelines[2]);
            // (One threadgroup per tile: the rectangle is tile-aligned.)
            enc.dispatch_threads([x1 - x0, y1 - y0, 1], [16, 16, 1]);
        } else {
            let b = backend.create_buffer_with_data(bytemuck::cast_slice(&list)).ok()?;
            enc.set_pipeline(&pipelines[3]);
            enc.set_buffer(3, &b, 0);
            enc.dispatch_threads([list.len() * 256, 1, 1], [256, 1, 1]);
        }
        self.stats.shaded_texels += if full { (x1 - x0) * (y1 - y0) } else { list.len() * 256 };
        let s = enc.commit();
        self.pending.push_back(Pass {
            shade: true,
            sub: s,
            limit,
            tiles: covered,
            serial,
        });
        self.stats.cpu_shade += t0.elapsed().as_secs_f64();
        Some((color, map))
    }

    /// The survey rectangle of `view` (axis-aligned in the window; a turned view is surveyed
    /// unturned): origin, size, packed wrap, packed texture size.
    pub fn survey_rect(
        &self,
        view: &Viewport,
        ppp: f64,
        width: usize,
        height: usize,
    ) -> Option<([u32; 2], [u32; 2], u32, u32)> {
        let m = self.map_for(view, ppp, width, height)?;
        let (w, h) = (width as f64 * m.ratio, height as f64 * m.ratio);
        let x0 = (m.centre[0] - w / 2.0).round().max(0.0) as u32;
        let y0 = (m.centre[1] - h / 2.0).round().max(0.0) as u32;
        let sw = (w.round() as u32).min(m.size.0 as u32 - x0.min(m.size.0 as u32)).max(1);
        let sh = (h.round() as u32).min(m.size.1 as u32 - y0.min(m.size.1 as u32)).max(1);
        Some((
            [x0, y0],
            [sw, sh],
            m.wrap[0] | (m.wrap[1] << 16),
            m.size.0 as u32 | ((m.size.1 as u32) << 16),
        ))
    }

    /// Seeds a new epoch from a refined full level (`iter`, `de`, `aux` of `full` texels at the
    /// pixel centres of `view`): its texel centres are a lattice, so a gesture starting here has
    /// every sample at once. Level: the largest ratio <= the cap, decimating the full level.
    #[allow(clippy::too_many_arguments)]
    pub fn seed(
        &mut self,
        backend: &B,
        full: (&B::Texture, &B::Texture, Option<&B::Texture>, usize, usize),
        view: &Viewport,
        ppp: f64,
        regime: Regime,
        limit: usize,
        generation: u64,
        seeded_tiles: &[bool],
    ) {
        if !self.config.enabled {
            return;
        }
        let t0 = Instant::now();
        self.poll(backend);
        let (iter, de, aux, fw, fh) = full;
        let Ok(pipelines) = self.pipelines(backend) else { return };
        if self.ensure_alloc(backend, fw, fh, aux.is_some()).is_err() {
            return;
        }
        self.seeded_generation = generation;
        self.slow = false;
        self.frames_measured = 0;
        // Step: 2^k full texels per lattice texel, the largest ratio 2^-k <= cap.
        let mut step = 1i64;
        while 1.0 / (step as f64) > self.cap * 1.0001 && step < 8 {
            step *= 2;
        }
        // Anchor: the centre of full texel (fw / 2, fh / 2).
        let (xa, ya) = ((fw / 2) as i64, (fh / 2) as i64);
        let (ox, oy) = (xa as f64 + 0.5 - fw as f64 / 2.0, -(ya as f64 + 0.5 - fh as f64 / 2.0));
        let anchor = view.point_at_offset(ox * ppp, oy * ppp);
        let s = view.scale * ppp;
        self.target = self.cap;
        self.demand = 0.0;
        self.calm_frames = 0;
        self.prev_view = None;
        let mut e = Epoch {
            regime,
            anchor,
            angle: view.angle,
            h: s * step as f64,
            win0: (0, 0),
            serial: 0,
        };
        let g = Self::view_geometry(&e, view, ppp, fw, fh);
        let (wt, ht) = self.dims();
        let (x0, x1, y0, y1) = Self::tile_range(&g, 2.0);
        if (x1 - x0 + 1) as usize > wt || (y1 - y0 + 1) as usize > ht {
            self.epoch = None;
            return;
        }
        e.win0 = (
            x0 - (wt as i64 - (x1 - x0 + 1)) / 2,
            y0 - (ht as i64 - (y1 - y0 + 1)) / 2,
        );
        self.serial += 1;
        e.serial = self.serial;
        // Tiles inside the full level are full; edge tiles partial. Lattice indices with a
        // full-level texel: lo ..= hi per axis.
        let range = |a: i64, n: usize| {
            (
                (-a).div_euclid(step) + i64::from((-a).rem_euclid(step) != 0),
                (n as i64 - 1 - a).div_euclid(step),
            )
        };
        let (ix, iy) = (range(xa, fw), range(ya, fh));
        let mut states = Vec::with_capacity(wt * ht);
        for ty in e.win0.1..e.win0.1 + ht as i64 {
            for tx in e.win0.0..e.win0.0 + wt as i64 {
                let (i0, i1, j0, j1) = (TILE * tx, TILE * tx + TILE - 1, TILE * ty, TILE * ty + TILE - 1);
                let all = i0 >= ix.0 && i1 <= ix.1 && j0 >= iy.0 && j1 <= iy.1;
                let any = i1 >= ix.0 && i0 <= ix.1 && j1 >= iy.0 && j0 <= iy.1;
                let state = if all {
                    State::Full
                } else if any {
                    State::Partial
                } else {
                    State::Empty
                };
                states.push(((tx, ty), state));
            }
        }
        // Leave out (empty, cleared after the copy) tiles that took samples from a seeded tile.
        let mut clear = Vec::new();
        if !seeded_tiles.is_empty() {
            let ftx = fw.div_ceil(16) as i64;
            for (t, state) in &mut states {
                if *state == State::Empty {
                    continue;
                }
                let span = |i: i64, a: i64, n: usize| {
                    let lo = (a + step * TILE * i).max(0);
                    let hi = (a + step * (TILE * i + TILE - 1)).min(n as i64 - 1);
                    (lo / 16, hi / 16)
                };
                let ((x0, x1), (y0, y1)) = (span(t.0, xa, fw), span(t.1, ya, fh));
                let hit = (y0..=y1).any(|y| (x0..=x1).any(|x| seeded_tiles.get((y * ftx + x) as usize) == Some(&true)));
                if hit {
                    *state = State::Empty;
                    clear.push(self.slot(*t) as u32);
                }
            }
        }
        for (t, state) in states {
            let slot = self.slot(t);
            self.tiles[slot] = Tile {
                holder: t,
                state,
                limit: limit as u32,
                inflight: 0,
                shaded: false,
            };
        }
        let (tw, th) = (wt * TILE as usize, ht * TILE as usize);
        let p = CacheParams {
            new_origin: [(TILE * e.win0.0) as i32, (TILE * e.win0.1) as i32],
            old_base: [xa as i32, ya as i32],
            new_size: [tw as u32, th as u32],
            new_wrap: [
                pmod(TILE * e.win0.0, tw as i64) as u32,
                pmod(TILE * e.win0.1, th as i64) as u32,
            ],
            old_size: [fw as u32, fh as u32],
            old_wrap: [0, 0],
            num: step as i32,
            shift: 0,
            de_scale: 1.0 / step as f32,
            ..CacheParams::default()
        };
        self.batches.clear();
        self.epoch = Some(e);
        self.stats.seeds += 1;
        self.stats.epochs += 1;
        let mut m = Maintenance {
            seed: Some((p, iter.clone(), de.clone(), aux.cloned())),
            clear,
            ..Maintenance::default()
        };
        self.encode_maintenance(backend, &pipelines, &mut m);
        self.stats.cpu_seed += t0.elapsed().as_secs_f64();
    }

    /// The current window as a lattice view for a fresh render of the same points (the cache
    /// check): centre and spacing of a `W` x `H` render whose texel t is window texel t, the
    /// angle, the texel of window texel (0, 0), and the cache iteration texture.
    pub fn check_frame(&self) -> Option<CheckFrame<B>> {
        let e = self.epoch.as_ref()?;
        let a = self.alloc.as_ref()?;
        let (tw, th) = (a.wt * TILE as usize, a.ht * TILE as usize);
        let (co, si) = rot(e.angle);
        // Texel t's centre is (t + 0.5 - size / 2) from the render's centre: lattice index
        // 16 win0 + t when the centre is lattice point 16 win0 + size / 2 - 0.5.
        let i = (TILE * e.win0.0) as f64 + tw as f64 / 2.0 - 0.5;
        let j = (TILE * e.win0.1) as f64 + th as f64 / 2.0 - 0.5;
        let centre = e.anchor.offset_scaled(i * co + j * si, i * si - j * co, e.h);
        Some((
            centre,
            e.h,
            e.angle,
            (tw, th),
            [
                pmod(TILE * e.win0.0, tw as i64) as u32,
                pmod(TILE * e.win0.1, th as i64) as u32,
            ],
            a.sets[self.cur].iter.clone(),
        ))
    }

    /// Interior fraction and exterior iterations per sample of the view the last finished shade
    /// pass showed (for `autoiter::use_interior_test`).
    pub fn view_stats(&self) -> Option<InteriorStats> {
        self.view_stats
    }

    /// The view rests: the next gesture decides afresh whether the cache pays.
    pub fn rest(&mut self) {
        self.slow = false;
        self.frames_measured = 0;
    }

    fn note_frame_gpu(&mut self, gpu: f64) {
        self.frame_gpu = if self.frames_measured == 0 {
            gpu
        } else {
            0.8 * self.frame_gpu + 0.2 * gpu
        };
        self.frames_measured += 1;
    }

    /// The cache's GPU time per gesture frame (smoothed) exceeds both the budget and what a
    /// fresh preview would take by a quarter: the gesture is better off without it (until the
    /// view rests, which seeds or resets this).
    pub fn too_slow(&mut self, budget: f64, fresh: f64) -> bool {
        if self.slow {
            return true;
        }
        self.slow = self.frames_measured >= 8 && fresh > 0.0 && self.frame_gpu > 1.25 * budget.max(fresh);
        if self.slow && self.config.trace {
            eprintln!(
                "sample cache: {:.1} ms per frame against {:.1} ms fresh: fresh previews for this gesture",
                self.frame_gpu * 1000.0,
                fresh * 1000.0
            );
        }
        self.slow
    }

    /// The regime of the current epoch.
    pub fn regime(&self) -> Option<Regime> {
        self.epoch.as_ref().map(|e| e.regime)
    }
}

/// A committed GPU pass of the cache: shade or maintenance, and (shade) the tiles whose
/// statistics it writes.
struct Pass<B: GpuBackend> {
    shade: bool,
    sub: B::Submission,
    /// The limit the shade pass counted against.
    limit: usize,
    tiles: Vec<(usize, (i64, i64))>,
    serial: u64,
}

#[derive(Clone, Copy, Debug)]
struct Est {
    holder: (i64, i64),
    serial: u64,
    units: f32,
    max_n: u32,
    /// Samples escaped below the limit, their escape counts, interior, unresolved at it.
    esc: u32,
    sum: u32,
    interior: u32,
    unresolved: u32,
}

impl Default for Est {
    fn default() -> Self {
        Self {
            holder: (i64::MIN, i64::MIN),
            serial: 0,
            units: 0.0,
            max_n: 0,
            esc: 0,
            sum: 0,
            interior: 0,
            unresolved: 0,
        }
    }
}

/// Passes to encode before the frame's chunks.
struct Maintenance<B: GpuBackend> {
    clear: Vec<u32>,
    /// Resample parameters and the source set.
    resample: Option<(CacheParams, usize)>,
    seed: Option<SeedSource<B>>,
}

/// Seeding: parameters and the full level's iteration, DE and interior analysis textures.
type SeedSource<B> = (
    CacheParams,
    <B as GpuBackend>::Texture,
    <B as GpuBackend>::Texture,
    Option<<B as GpuBackend>::Texture>,
);

/// [`SampleCache::check_frame`]: render centre, spacing, angle, size, wrap, iteration texture.
pub(crate) type CheckFrame<B> = (Center, Scale, f64, (usize, usize), [u32; 2], <B as GpuBackend>::Texture);

impl<B: GpuBackend> Default for Maintenance<B> {
    fn default() -> Self {
        Self {
            clear: Vec::new(),
            resample: None,
            seed: None,
        }
    }
}

struct Geometry {
    centre: (f64, f64),
    ratio: f64,
    /// (cos, sin) of the view angle minus the lattice angle.
    rot: (f64, f64),
    /// Half extents of the footprint, lattice texels.
    eu: f64,
    ev: f64,
    delta: f64,
}

fn rot(a: f64) -> (f64, f64) {
    if a == 0.0 {
        (1.0, 0.0)
    } else {
        let (s, c) = a.sin_cos();
        (c, s)
    }
}
