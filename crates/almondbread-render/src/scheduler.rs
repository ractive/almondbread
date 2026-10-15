//! The chunked progressive scheduler, written once against [`GpuBackend`]
//! (kb: platform/render-architecture, platform/gpu-watchdog-and-chunking,
//! techniques/frame-reuse-and-progressive). A port of the Swift `Renderer`: same decisions,
//! same constants, same kernel inputs, so iteration fields are bit-identical.
//!
//! Rendering is a *job* per view: one or two *levels* (an optional coarse preview, then the
//! full resolution), each iterated in resumable chunks of ~8–16 ms of GPU time. A chunk is
//! one command buffer: size the indirect dispatch, iterate every live pixel (plus freshly
//! admitted ones) for at most K iterations, record the counters. K follows from a measured
//! throughput, so no command buffer gets near the watchdog. Each display refresh harvests
//! finished chunks, encodes the next ones within a time budget and colorizes a progressive
//! image (unfinished pixels show the preview underneath). A changed view starts a new job
//! (new generation); older generations only feed the throughput model, and since the queue is
//! serial and every admitted pixel is rewritten by its own job, no stale chunk can show.
//!
//! Time is passed in (`now`, seconds on any monotonic clock): the host's display-link
//! timestamps in the app, [`Renderer::clock`] in the headless tools.

use std::collections::VecDeque;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use almondbread_core::autoiter::{self, InteriorStats, LevelOutcome};
use almondbread_core::bla::{BLA_EPSILON, BlaForecast, BlaTable, BlaTableX};
use almondbread_core::input::{InputEvent, Navigator};
use almondbread_core::oracle::ITER_UNRESOLVED;
use almondbread_core::reference::{DEEP_SCALE, Fit, Reference, ReferenceRequest, ReferenceWorker};
use almondbread_core::settings::{RenderSettings, depth_iterations};
use almondbread_core::viewport::{Scale, Viewport};
use bytemuck::bytes_of;

use crate::backend::{ComputeEncoder, GpuBackend, GpuError, Storage, Submission, SubmissionStatus, TextureFormat};
use crate::gpu_types::{
    AaListParams, BlaParams, ChunkStats, ColorParams, ComposeParams, IterateParams, LEVEL_COUNTERS_SIZE, PixelState,
    PixelStateIC, SeedNode, SeedParams, SurveyParams,
};
use crate::sample_cache::{CacheMap, CacheStats, PlanInput, Regime, SampleCache};
use almondbread_core::autopilot::{RenderFeedback, Survey, SurveyCell};

/// Dense-region colours cached per palette: (palette key, deep, filament).
type DenseColors = (String, [f32; 3], [f32; 3]);

/// Escape radius (2^16: the smooth offset stays in a small range, kb: techniques/smooth-coloring).
pub const BAILOUT: f64 = 65536.0;
const STAT_SLOTS: usize = 64;
/// Autopilot survey grid and how often a finished level is surveyed (s).
const SURVEY_COLS: usize = 16;
const SURVEY_ROWS: usize = 10;
const SURVEY_INTERVAL: f64 = 0.1;
const PALETTE_SIZE: usize = 1024;
/// Derivative test threshold on |w| n² (kb: techniques/interior-detection, Pitfalls).
const INTERIOR_EPS: f32 = 1e-3;
const ITERATE_THREADS: usize = 256;
/// Compose modes (the `compose` kernel's `mode`).
const COMPOSE_GESTURE: u32 = 0;
const COMPOSE_REFINE: u32 = 1;
const COMPOSE_AA: u32 = 2;
const COMPOSE_EXACT: u32 = 3;

/// When the iterate kernel steps through the BLA table (kb: techniques/bla).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlaMode {
    /// Never (no table is built).
    Off,
    /// Always when a table exists (skip-on/skip-off diffs, benchmarks).
    On,
    /// When the table's forecast for the view says it pays (`BlaForecast::pays`): a block
    /// costs a few exact steps on the GPU, so short skips lose (M4: every view the f64
    /// reference reaches; deep views win).
    Auto,
}

impl BlaMode {
    pub fn parse(v: &str) -> Option<Self> {
        match v {
            "0" | "off" => Some(Self::Off),
            "1" | "on" => Some(Self::On),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Tunables. Defaults are the Swift renderer's.
#[derive(Clone, Debug)]
pub struct SchedulerConfig {
    /// Live-list slots (bounds state memory: 40 B per slot, two buffers).
    pub live_capacity: usize,
    /// Upper bound on unresolved-list entries (4 bytes each, two buffers).
    pub max_list_capacity: usize,
    /// GPU seconds per command buffer when idle.
    pub chunk_target: f64,
    /// GPU seconds per command buffer during gestures.
    pub interactive_chunk_target: f64,
    /// GPU seconds a gesture frame's iteration may take (a 120 Hz refresh minus colorize).
    pub interactive_frame_budget: f64,
    /// Hard limit for unresolved pixels as a multiple of the nominal one (view refinement).
    pub extension_factor: usize,
    /// The same for offscreen renders (1: none, so verify diffs at the same limit).
    pub offscreen_extension: usize,
    /// Estimated GPU seconds the extension passes of a view may take in all.
    pub extension_budget: f64,
    /// Frames committed but not finished on the GPU.
    pub max_frames_in_flight: usize,
    /// Upper bound on queued chunk command buffers of the current job.
    pub max_chunks_in_flight: usize,
    /// Print every harvested chunk (`ALMONDBREAD_TRACE=1`).
    pub trace: bool,
    /// Size the gesture frame budget from the measured display refresh (about 70 % of a
    /// refresh minus compose) instead of `interactive_frame_budget` (FL).
    pub adaptive_budget: bool,
    /// Time constant (s) of the per-pixel fade of refined pixels over the history (FL).
    pub fade_tau: f64,
    /// Temporal upsampling during gestures: confidence cap of the history (higher: smoother,
    /// slower to take new samples), sample kernel sigma (output pixels), weight of the
    /// bilinear preview where history or samples exist.
    pub taa_conf_cap: f64,
    pub taa_sigma: f64,
    pub taa_base_weight: f64,
    /// GPU seconds per command buffer of idle anti-aliasing passes (small: input interrupts
    /// them at the next chunk boundary).
    pub aa_chunk_target: f64,
    /// GPU seconds the AA samples of one view may take in all (heavy views settle with
    /// fewer samples).
    pub aa_budget: f64,
    /// BLA iteration skipping (kb: techniques/bla; `ALMONDBREAD_BLA=0|1|auto`).
    pub bla: BlaMode,
    /// Wait for the frame's chunks before its colour passes (measurements only;
    /// `ALMONDBREAD_SERIAL_COMPOSE=1`).
    pub serial_compose: bool,
    /// BLA validity: dropped terms at most `bla_epsilon` of the linear term per step
    /// (`ALMONDBREAD_BLA_EPS=e` sets 2^-e).
    pub bla_epsilon: f64,
    /// References at least this long build their BLA table on a thread (the UI never waits
    /// for it; previews start without, full levels wait for it), shorter ones inline
    /// (~20 ns per point on M1 Pro, so inline builds stay under ~0.4 ms).
    pub bla_async_from: usize,
    /// BLA lookups only at reference indices m = 1 (mod this): a lookup costs several exact
    /// steps on the GPU, and a rule on m keeps results independent of chunking.
    pub bla_lookup_every: u32,
    /// M5: local series approximation (`ALMONDBREAD_SA=0|1|auto`; kb:
    /// techniques/local-series-approximation).
    pub local_sa: LocalSa,
    /// M5: interior-only tile fill on full levels with the interior test and flat interior
    /// colouring (`ALMONDBREAD_FILL=0|1|auto`; kb: techniques/guessing-and-subdivision).
    /// `SaMode` doubles as its mode: Auto fills where the last level's statistics forecast a
    /// gain.
    pub interior_fill: SaMode,
    /// M5: auto iterations start at the big reference's floor (`period_floor`;
    /// `ALMONDBREAD_ITER_FLOOR=0` off, for before/after measurements).
    pub iteration_floor: bool,
}

/// When levels start their pixels from local series approximation seeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaMode {
    /// Never (no seed passes).
    Off,
    /// Every tile of every level (skip-on/skip-off diffs, benchmarks).
    On,
    /// Seed every level; a 16 x 16 tile starts from its seeds where they saved enough (the
    /// per-tile forecast of `seedGate`, decided from the seeds alone, so deterministic).
    Auto,
}

impl SaMode {
    pub fn parse(v: &str) -> Option<Self> {
        match v {
            "0" | "off" => Some(Self::Off),
            "1" | "on" => Some(Self::On),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Quadtree seeds of the local series approximation (M5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalSa {
    pub mode: SaMode,
    /// Root node size in texels (a power of two).
    pub top: usize,
    /// Leaf node size (pixels start from it; a power of two in 2..=16).
    pub leaf: usize,
    /// Branch when the quadratic term exceeds `eps` of the linear one over the node.
    pub eps: f64,
    /// Auto: a tile is seeded when its leaves saved on average at least `gate_rel` of the last
    /// level's (unseeded-equivalent) steps per texel and at least `gate_abs` seed steps;
    /// without that estimate, when their mean branch iteration is `gate_rel` of the limit.
    pub gate_rel: f64,
    pub gate_abs: f64,
    /// Seed steps per node and round (0: from the throughput model).
    pub budget: u32,
}

impl Default for LocalSa {
    fn default() -> Self {
        Self {
            mode: std::env::var("ALMONDBREAD_SA")
                .ok()
                .and_then(|v| SaMode::parse(&v))
                .unwrap_or(SaMode::Auto),
            top: 32,
            leaf: 4,
            eps: 1e-2,
            gate_rel: 1.0 / 6.0,
            gate_abs: 8.0,
            budget: 0,
        }
    }
}

/// What the seed passes of the last seeded level did.
#[derive(Clone, Debug, Default)]
pub struct SeedReport {
    /// Per quadtree level: node size, nodes, accepted seed iterations, executed seed steps,
    /// mean branch iteration.
    pub levels: Vec<(usize, usize, u64, u64, f64)>,
    pub gpu_time: f64,
    pub rounds: usize,
    /// Tiles starting from their seeds, of all tiles.
    pub tiles_seeded: usize,
    pub tiles: usize,
    /// Leaves whose seed stopped proven interior, and their branch iterations (sorted).
    pub interior_leaves: Vec<u32>,
    pub leaves: usize,
    /// Mean over the leaves of the seed steps from the root (what a pixel saves).
    pub leaf_path: f64,
}

impl SeedReport {
    pub fn iterations(&self) -> u64 {
        self.levels.iter().map(|l| l.2).sum()
    }
    pub fn steps(&self) -> u64 {
        self.levels.iter().map(|l| l.3).sum()
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        let ios = cfg!(target_os = "ios");
        let mut live = if ios { 1 << 19 } else { 1 << 20 };
        if let Some(v) = std::env::var("ALMONDBREAD_CAPACITY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0)
        {
            live = v;
        }
        Self {
            live_capacity: live,
            max_list_capacity: if ios { 1 << 21 } else { 1 << 22 },
            chunk_target: 0.016,
            interactive_chunk_target: 0.008,
            interactive_frame_budget: 0.006,
            extension_factor: 16,
            offscreen_extension: 1,
            extension_budget: 2.0,
            max_frames_in_flight: 2,
            max_chunks_in_flight: 3,
            trace: std::env::var("ALMONDBREAD_TRACE").is_ok_and(|v| v == "1"),
            adaptive_budget: true,
            fade_tau: 0.03,
            taa_conf_cap: 6.0,
            taa_sigma: 0.5,
            taa_base_weight: 0.02,
            aa_chunk_target: 0.008,
            aa_budget: 1.5,
            serial_compose: std::env::var("ALMONDBREAD_SERIAL_COMPOSE").is_ok_and(|v| v == "1"),
            bla: std::env::var("ALMONDBREAD_BLA")
                .ok()
                .and_then(|v| BlaMode::parse(&v))
                .unwrap_or(BlaMode::Auto),
            bla_epsilon: std::env::var("ALMONDBREAD_BLA_EPS")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .map_or(BLA_EPSILON, |e| 2f64.powf(-e)),
            bla_async_from: 16_384,
            bla_lookup_every: 8,
            local_sa: LocalSa::default(),
            interior_fill: std::env::var("ALMONDBREAD_FILL")
                .ok()
                .and_then(|v| SaMode::parse(&v))
                .unwrap_or(SaMode::Auto),
            iteration_floor: std::env::var("ALMONDBREAD_ITER_FLOOR").map_or(true, |v| v != "0"),
        }
    }
}

/// Iteration results for one level: textures (possibly larger than the level: a preview
/// lives in the top-left corner of a texture allocated once at its maximum size) and the
/// used size.
struct TextureSet<B: GpuBackend> {
    iter: B::Texture,
    de: B::Texture,
    color: Option<B::Texture>,
    /// Interior cycle analysis (rg32f: interior distance, log2 closest approach), allocated
    /// when an interior colouring first needs it.
    aux: Option<B::Texture>,
    width: usize,
    height: usize,
}

impl<B: GpuBackend> Clone for TextureSet<B> {
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            de: self.de.clone(),
            color: self.color.clone(),
            aux: self.aux.clone(),
            width: self.width,
            height: self.height,
        }
    }
}

impl<B: GpuBackend> TextureSet<B> {
    fn with_size(&self, width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            ..self.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ComputeKey {
    viewport: Viewport,
    max_iter: usize,
    points_per_pixel: f64,
    de: bool,
    interior: bool,
    /// Interior cycle analysis for the interior colourings (kInteriorColor).
    interior_color: bool,
    width: usize,
    height: usize,
    supersampling: usize,
    /// Bumped to render the same view again (a forced retry with the interior test).
    retry: u32,
}

struct Level<B: GpuBackend> {
    set: TextureSet<B>,
    /// Texels per output pixel.
    ratio: f64,
    /// Template; chunk fields filled per dispatch.
    params: IterateParams,
    total: usize,
    // As of the last harvested chunk (the GPU is ahead of these):
    admitted: usize,
    live: usize,
    encoded: usize,
    /// Nothing left to iterate (or the extension budget is spent).
    done: bool,
    /// Every pixel escaped, was proven interior or passed the nominal limit.
    complete: bool,
    crossed: usize,
    max_escape: usize,
    escaped_past: [usize; 4],
    /// Hard limit for extension passes (0: none).
    extend_to: usize,
    list_in: usize,
    list_out: usize,
    gpu_time: f64,
    iterations: f64,
    interior_pixels: usize,
    use_interior: bool,
    /// Loop steps (BLA + perturbation) the GPU did for this level.
    steps: f64,
    /// Iterates through the BLA table (decided when the level starts).
    use_bla: bool,
    preview: bool,
    /// Admission order: tile order and its inverse (None: raster tile order).
    order: Option<TileOrder<B>>,
    /// An idle anti-aliasing sample (index >= 1 of the pixel's samples; 0 is the full level).
    aa: Option<u32>,
    /// Admission list of a list level (AA samples: the adaptive list), instead of the
    /// unresolved lists.
    list_buffer: Option<B::Buffer>,
    /// M5: fresh pixels start from quadtree seeds (the leaf level's parameters); set for the
    /// level that seeds and for its extension passes and AA samples, which reuse its leaves.
    seeded: Option<SeedParams>,
    /// The level's own seed passes (None: not seeded by this level).
    seeding: Option<Seeding>,
    /// A gesture level of the motion sample cache (kb: techniques/sample-reuse): its textures
    /// are the cache's toroidal window, it computes the listed tiles only.
    cache: Option<CacheLevel<B>>,
}

/// What a sample cache level iterates with: the epoch's pinned regime and its BLA table.
struct CacheLevel<B: GpuBackend> {
    regime: Regime,
    bla: Option<BlaGpu<B>>,
    /// GPU seconds the planner expects the batch to take (0: unknown).
    predicted: f64,
}

/// A level's seed passes (M5): the quadtree layout and how far the rounds got.
struct Seeding {
    layout: Vec<SeedParams>,
    /// Every node is final (the last round reported none unfinished).
    done: bool,
    /// Tiles that start from their seeds (known once done), and the sum of their leaves' mean
    /// path (seed steps their pixels skip, per pixel).
    tiles_seeded: usize,
    path_sum: u64,
    /// Nodes the last round left unfinished (later rounds size their budgets by them).
    unfinished: Option<usize>,
    rounds: usize,
    gpu_time: f64,
    /// Pipeline variant: interior test, interior colouring, BLA blocks.
    interior: bool,
    interior_color: bool,
    bla: bool,
}

/// A seed round on the GPU.
struct SeedRound<B: GpuBackend> {
    submission: B::Submission,
    generation: u64,
    level: usize,
    /// Budget steps x waves of the round (`Renderer::seed_unit` learns from it).
    units: f64,
}

/// M5 interior fill: border pixels must be proven interior within this many iterations (the
/// insides wait at most this long for the borders).
const FILL_PROOF: u32 = 8192;
/// Auto fill: the last level's interior fraction and iterations per pixel at least these.
const FILL_MIN_INTERIOR: f64 = 0.15;
const FILL_MIN_ITERATIONS: f64 = 300.0;

/// Seed nodes the GPU runs at once (about; M1 Pro: 16 cores x 1024 threads).
const SEED_WAVE: f64 = 16384.0;

/// The adaptive AA list of a refined view: the GPU pass that builds it and its length.
struct AaList<B: GpuBackend> {
    count_buffer: B::Buffer,
    submission: B::Submission,
    len: Option<usize>,
}

/// A priority order of a level's 16x16 tiles: `order[rank] = tile`, `rank[tile] = rank`.
struct TileOrder<B: GpuBackend> {
    order: B::Buffer,
    rank: B::Buffer,
}

impl<B: GpuBackend> Clone for TileOrder<B> {
    fn clone(&self) -> Self {
        Self {
            order: self.order.clone(),
            rank: self.rank.clone(),
        }
    }
}

/// 16x16 tiles of a w x h level: (per row, rows).
pub fn tiles(w: usize, h: usize) -> (usize, usize) {
    (w.div_ceil(16), h.div_ceil(16))
}

/// Halton low-discrepancy sequence, base `b`, index `i` >= 1, in (0, 1).
pub fn halton(mut i: u32, b: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while i > 0 {
        f /= b as f32;
        r += f * (i % b) as f32;
        i /= b;
    }
    r
}

impl<B: GpuBackend> Level<B> {
    fn new(set: TextureSet<B>, ratio: f64, preview: bool, mut params: IterateParams, total: Option<usize>) -> Self {
        let (tx, ty) = tiles(set.width, set.height);
        // Tiled admission: every tile has 256 slots (edge tiles' padding is skipped).
        let total = total.unwrap_or(tx * ty * 256);
        params.tiles_x = tx as u32;
        params.height = set.height as u32;
        params.total = total as u32;
        Self {
            set,
            ratio,
            params,
            total,
            admitted: 0,
            live: 0,
            encoded: 0,
            done: false,
            complete: false,
            crossed: 0,
            max_escape: 0,
            escaped_past: [0; 4],
            extend_to: 0,
            list_in: 0,
            list_out: 0,
            gpu_time: 0.0,
            iterations: 0.0,
            interior_pixels: 0,
            use_interior: false,
            steps: 0.0,
            use_bla: false,
            preview,
            order: None,
            aa: None,
            list_buffer: None,
            seeded: None,
            seeding: None,
            cache: None,
        }
    }

    fn started(&self) -> bool {
        self.encoded > 0
    }

    fn is_extension(&self) -> bool {
        self.params.list_mode == 2
    }
}

/// What a gesture job needs to append its full level when input idles.
struct FullLevelTemplate<B: GpuBackend> {
    set: TextureSet<B>,
    ratio: f64,
    params: IterateParams,
    extend_to: usize,
}

struct Job<B: GpuBackend> {
    generation: u64,
    key: ComputeKey,
    reference_buffer: B::Buffer,
    /// Serial of the reference the job iterates against (its BLA table must match).
    reference_serial: u64,
    /// The BLA table of that reference, once built and if it is used for this view (levels
    /// started before stay without).
    bla: Option<BlaGpu<B>>,
    /// Largest |dc| of the view's pixels, log2 (the forecast's distance from the reference).
    max_dc_log2: f64,
    /// Iterates with the deep (rescaled, extended-range) kernel against a big reference (M3b).
    deep: bool,
    /// The reference's floatexp points (deep kernel).
    refx_buffer: Option<B::Buffer>,
    /// The table's forecast for this view, once decided.
    bla_forecast: Option<BlaForecast>,
    levels: Vec<Level<B>>,
    start: f64,
    /// A gesture frame: preview only, superseded every frame.
    interactive: bool,
    presented: bool,
    full_level: Option<FullLevelTemplate<B>>,
    gpu_time: f64,
    chunks: usize,
    iterations: f64,
    steps: f64,
    failed: bool,
    /// Shown after the last pixel resolved (or the budget ran out), anti-aliasing included.
    final_presented: bool,
    complete_presented: bool,
    /// When the complete frame was first composed (the per-pixel fade runs on from there).
    complete_at: Option<f64>,
    /// AA samples composed into the history (besides the full level's centre sample).
    aa_composed: u32,
    /// Bumped when the AA samples are dropped (a colour change recolours from the exact
    /// samples): chunks of dropped AA levels still in flight carry the old value and are
    /// ignored at harvest. Before this, such a chunk indexed a level that was gone (or a
    /// newer AA level at the same index): "selected Lagoon and almondbread died".
    aa_epoch: u64,
    /// Which pixels take AA samples (built once the view is refined).
    aa_list: Option<AaList<B>>,
}

/// The CPU copy of a BLA table: float32 (plain kernel, f64 references) or extended range
/// (deep kernel, big references; M3b).
#[derive(Clone)]
enum BlaCpu {
    Plain(Arc<BlaTable>),
    Deep(Arc<BlaTableX>),
}

impl BlaCpu {
    fn forecast(&self, max_dc_log2: f64, max_iter: usize) -> BlaForecast {
        match self {
            BlaCpu::Plain(t) => t.forecast(max_dc_log2.exp2(), max_iter),
            BlaCpu::Deep(t) => t.forecast(max_dc_log2, max_iter),
        }
    }
}

/// A BLA table on the GPU, for the reference with serial `serial`.
struct BlaGpu<B: GpuBackend> {
    serial: u64,
    buffer: B::Buffer,
    params: BlaParams,
    /// The CPU copy, for the per-view forecast.
    table: BlaCpu,
}

impl<B: GpuBackend> Clone for BlaGpu<B> {
    fn clone(&self) -> Self {
        Self {
            serial: self.serial,
            buffer: self.buffer.clone(),
            params: self.params,
            table: self.table.clone(),
        }
    }
}

/// Whether a job's BLA table is there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlaReady {
    Ready,
    /// Building on a thread.
    Pending,
    /// Off, or no table (too short a reference, or the build failed).
    Without,
}

impl<B: GpuBackend> Job<B> {
    fn done(&self) -> bool {
        self.levels.iter().all(|l| l.done)
    }
    fn complete(&self) -> bool {
        self.levels.iter().all(|l| l.complete)
    }
    fn current(&self) -> Option<usize> {
        self.levels.iter().position(|l| !l.done)
    }
    /// The level to show: the furthest one that has started (AA samples are not shown as
    /// levels; they are accumulated into the history).
    fn display(&self) -> usize {
        self.levels
            .iter()
            .rposition(|l| l.started() && l.aa.is_none())
            .unwrap_or(0)
    }
    /// The finished AA sample not composed yet, if any.
    fn aa_ready(&self) -> Option<usize> {
        self.levels
            .iter()
            .position(|l| l.aa == Some(self.aa_composed + 1) && l.done)
    }
    fn aa_pending(&self) -> bool {
        self.levels.iter().any(|l| l.aa.is_some() && !l.done)
    }
    /// Every AA sample the target asks for is composed, no pixel needs any, or the AA budget
    /// is spent (`budget` GPU seconds over the composed samples).
    fn aa_finished(&self, target: u32, budget: f64) -> bool {
        let spent: f64 = self
            .levels
            .iter()
            .filter(|l| l.aa.is_some_and(|n| n <= self.aa_composed))
            .map(|l| l.gpu_time)
            .sum();
        self.aa_composed + 1 >= target || self.aa_list.as_ref().is_some_and(|l| l.len == Some(0)) || spent > budget
    }
    fn drop_aa(&mut self) {
        self.levels.retain(|l| l.aa.is_none());
        self.aa_composed = 0;
        self.aa_epoch += 1;
    }
}

/// A preview level's colour texture as the compose pass samples it.
struct PreviewColor<B: GpuBackend> {
    texture: B::Texture,
    size: (usize, usize),
    ratio: f64,
    jitter: [f32; 2],
    /// The sample cache's colour window and how the view maps onto it.
    cache: Option<CacheMap>,
}

/// The last composed image (FL): which view it shows, at what scale, with which colours.
#[derive(Clone, Debug)]
struct HistoryView {
    viewport: Viewport,
    /// Complex units per pixel.
    scale: Scale,
    width: usize,
    height: usize,
    /// Colour parameters it was coloured with (phase excluded) and the palette.
    colors: (ColorParams, String),
    /// Palette phase it was coloured with.
    offset: f32,
}

struct ChunkRecord<B: GpuBackend> {
    submission: B::Submission,
    generation: u64,
    level: usize,
    /// The job's AA epoch when this chunk belongs to an AA sample level (see `Job::aa_epoch`).
    aa_epoch: Option<u64>,
    slot: usize,
    iterations: usize,
    /// K came from the throughput model (feeds `rate` back).
    adaptive: bool,
    /// A chunk of a preview level (gesture frames, the sample cache's tiles).
    preview: bool,
    /// A chunk of a sample cache level (its tiles are no sample of the view's cost).
    cache: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobMode {
    Interactive,
    Refine,
    Offscreen,
}

/// What a frame colorizes into: a texture, and the drawable to present with it (none for
/// offscreen targets).
pub type FrameTarget<B> = (<B as GpuBackend>::Texture, Option<<B as GpuBackend>::Drawable>);

/// One big reference computed (M3b reports: reference times per depth).
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceLogEntry {
    /// log10 of the view scale per point it was made for.
    pub scale_log10: f64,
    pub precision_bits: usize,
    /// Points (period + 1 for a nucleus).
    pub length: usize,
    pub period: Option<usize>,
    pub select_time: f64,
    pub orbit_time: f64,
    /// Seconds from the request to its arrival on the render thread.
    pub latency: f64,
}

/// Values for a HUD / inspector (the Swift model's renderer-written stats).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShownStats {
    pub viewport: Option<Viewport>,
    pub max_iterations: usize,
    pub reference_time: f64,
    pub reference_length: usize,
    /// The shown view's reference is a minibrot nucleus of this period.
    pub reference_period: Option<usize>,
    /// Precision of the reference in bits (53: f64), and whether a better one is computing.
    pub reference_bits: usize,
    pub reference_pending: bool,
    /// The deep kernel renders the shown view.
    pub deep: bool,
    /// Size of the shown level's texture region.
    pub render_size: (usize, usize),
    pub is_preview: bool,
    /// GPU time of all iterate chunks of the shown job so far.
    pub gpu_time: f64,
    pub chunks: usize,
    pub is_rendering: bool,
}

/// Per-frame counters for interaction tests (None: no cost).
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    pub frames: usize,
    pub preview_frames: usize,
    /// Sum over those frames of the shown level's texels per output pixel (preview scale).
    pub preview_scale_sum: f64,
    /// Refreshes that encoded nothing (frames in flight, or no drawable without blocking).
    pub skipped: usize,
    /// ... of which with `max_frames_in_flight` colorize passes still on the GPU,
    pub skipped_in_flight: usize,
    /// ... and with no target available without blocking.
    pub skipped_no_target: usize,
    pub chunk_gpu: f64,
    /// Frames that showed only the reprojected history (deep view, reference computing).
    pub waiting_frames: usize,
    /// GPU seconds of the colorize passes that completed while measuring.
    pub colorize_gpu: f64,
    /// Iterations of all chunks and of preview-level chunks (gesture frames), and the GPU
    /// seconds of the latter.
    pub iterations: f64,
    pub preview_iterations: f64,
    pub preview_chunk_gpu: f64,
}

/// The shown frame against a fresh offscreen render of the same texel grid
/// ([`Renderer::displayed_frame_mismatch`]).
#[derive(Clone, Debug, PartialEq)]
pub struct FrameMismatch {
    /// Pixels whose class (escaped / interior) differs; unresolved pixels are not counted.
    pub pixels: usize,
    pub percent: f64,
    pub width: usize,
    pub height: usize,
}

/// Result of one display refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameOutcome {
    /// Keep the display link running (keys held, gesture, unfinished job).
    pub animating: bool,
    /// A frame was committed for presentation.
    pub presented: bool,
}

/// An offscreen render (headless checks, export).
#[derive(Clone, Debug)]
pub struct OffscreenResult {
    /// BGRA8 pixels, sRGB-encoded, `width * 4` bytes per row.
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
    /// GPU time of all iterate chunks, seconds.
    pub iterate_gpu_time: f64,
    pub colorize_gpu_time: f64,
    pub wall_time: f64,
    pub chunks: usize,
    /// Iterations executed by the GPU (chunk counters).
    pub iterations_done: u64,
    /// Loop steps the GPU took for them (BLA steps + perturbation steps; equal to
    /// `iterations_done` without BLA).
    pub steps_done: u64,
    /// The BLA table's forecast for the view (None: no table), and whether it was used.
    pub bla_forecast: Option<BlaForecast>,
    pub bla_used: bool,
    /// Raw iteration texels (n, smooth offset), row-major, if requested.
    pub iterations: Option<Vec<[f32; 2]>>,
    /// M5: the seed passes of the last seeded level (None: not seeded).
    pub seeds: Option<SeedReport>,
}

/// A scripted headless session through the interactive scheduler.
#[derive(Clone, Debug)]
pub struct SimulationResult {
    /// The last presented frame, BGRA8 sRGB.
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
    /// Iteration texels of the level the final frame showed, row-major.
    pub iterations: Vec<[f32; 2]>,
    pub level_width: usize,
    pub level_height: usize,
    pub viewport: Viewport,
    pub max_iter: usize,
    /// Hard limit unresolved pixels were continued to.
    pub hard_iter: usize,
    /// Complex units per texel of the final frame's level.
    pub texel_scale: Scale,
    pub preview: bool,
    /// Rendered by the deep kernel (M3b).
    pub deep: bool,
    pub frames: usize,
    /// Seconds from the last input to the complete (refined) frame, and to the frame after
    /// which nothing was left to iterate.
    pub final_after: f64,
    pub done_after: f64,
    /// Seconds from the last input (a jump: the start) to the first frame computed for the
    /// current view (not the history shown while its reference computes); negative when
    /// that frame came during the gesture.
    pub first_after: f64,
    pub unresolved: usize,
    pub reference_length: usize,
}

/// The renderer: scheduler state plus the GPU resources it owns.
pub struct Renderer<B: GpuBackend> {
    backend: B,
    pub viewport: Viewport,
    pub settings: RenderSettings,
    pub navigator: Navigator,
    pub config: SchedulerConfig,
    pub stats: ShownStats,
    pub frame_stats: Option<FrameStats>,

    prepare_pipeline: B::Pipeline,
    finish_pipeline: B::Pipeline,
    colorize_pipeline: B::Pipeline,
    compose_pipeline: B::Pipeline,
    aa_list_pipeline: B::Pipeline,
    /// Overlay scene to composite over presented frames (orbit, maps; see `overlay`), its font,
    /// the pipeline (made on first use) and the version last presented (None: none shown).
    pub overlay: Option<crate::overlay::OverlayDraw<B>>,
    pub overlay_font: Option<B::Buffer>,
    overlay_pipeline: Option<B::Pipeline>,
    overlay_shown: Option<u64>,
    /// Autopilot: the survey kernel, its result buffer, the survey in flight (its view
    /// geometry filled in) and when the last one was encoded.
    survey_pipeline: B::Pipeline,
    survey_buffer: B::Buffer,
    pending_survey: Option<(B::Submission, Survey)>,
    last_survey: f64,
    /// The adaptive AA list (texel indices) and its capacity.
    aa_list_buffer: Option<(B::Buffer, usize)>,
    /// Ping-pong history images (linear colour + confidence) at the output size (FL).
    history: Option<[B::Texture; 2]>,
    history_ping: usize,
    history_view: Option<HistoryView>,
    /// When the last frame was composed (fade and AA timing).
    last_compose: f64,
    /// Gesture frames so far (Halton jitter index of the temporal upsampling).
    taa_index: u32,
    /// Measured display refresh interval (s), smoothed, and the last frame's time.
    refresh_interval: f64,
    last_frame_time: f64,
    /// Placeholder bound where no tile order buffer is used.
    dummy_buffer: B::Buffer,
    /// Exact samples of earlier gesture frames, reused while moving (kb: techniques/sample-reuse).
    sample_cache: SampleCache<B>,
    /// Scripted sessions (`simulate`) check the sample cache after their input frames with this
    /// renderer and collect the results (`verify --history`).
    pub cache_check_renderer: Option<Box<Renderer<B>>>,
    pub cache_checks: Vec<Option<CacheCheck>>,
    /// Colour cycling: accumulated phase (cycles) and when it last advanced.
    cycle_phase: f64,
    cycle_time: f64,
    /// Iterate pipelines keyed by `de | interior << 1 | bla << 2 | deep << 3 | interior_color << 4 |
    /// seeded << 5`.
    iterate_pipelines: [Option<B::Pipeline>; 64],
    /// Interior cycle analysis state, parallel to `state_buffers` (allocated on first use).
    ic_buffers: Option<[B::Buffer; 2]>,
    /// Level textures carry the interior analysis texture (set per job).
    want_aux: bool,
    /// M5 (kb: techniques/local-series-approximation): seed pipelines keyed by
    /// `interior | deep << 1 | bla << 2 | interior_color << 3`; the node and tile-flag buffers
    /// (grown to the largest level, then kept) with their capacities; the round status; the
    /// seeding epoch (node stamps); the round on the GPU; the last seeding's layout and stats.
    seed_pipelines: [Option<B::Pipeline>; 16],
    seed_begin_pipeline: B::Pipeline,
    seed_gate_pipeline: B::Pipeline,
    seed_nodes: Option<(B::Buffer, usize)>,
    seed_tiles: Option<(B::Buffer, usize)>,
    seed_status: B::Buffer,
    seed_epoch: u32,
    /// GPU seconds per budget step of one wave of seed nodes (learnt).
    seed_unit: f64,
    seed_round: Option<SeedRound<B>>,
    seed_last: Option<(Vec<SeedParams>, f64, usize)>,
    /// Loop steps per texel of the last completed level as if unseeded (its pixels' steps
    /// plus what its seeds skipped): the auto SA gate's cost estimate.
    steps_per_texel: Option<f64>,
    /// The view of the last `make_key` (jumps reset the learned state) and the retry epoch.
    last_key_view: Option<Viewport>,
    retry_epoch: u32,
    /// M5 interior fill: per-tile stamps (a border pixel not proven interior writes its
    /// level's epoch) and their capacity; the last epoch.
    tile_bad: Option<(B::Buffer, usize)>,
    fill_epoch: u32,

    full_set: Option<TextureSet<B>>,
    /// One preview allocation at the largest preview size (the output size); levels use its
    /// top-left corner, so changing the gesture preview resolution never allocates.
    preview_alloc: Option<TextureSet<B>>,

    state_buffers: [B::Buffer; 2],
    count_buffers: [B::Buffer; 2],
    stats_buffer: B::Buffer,
    args_buffer: B::Buffer,
    level_buffer: B::Buffer,
    list_buffers: Option<[B::Buffer; 2]>,
    list_count_buffer: B::Buffer,
    list_capacity: usize,
    stat_slot: usize,
    ping: usize,

    reference: Option<Reference>,
    reference_time: f64,
    reference_buffer: Option<B::Buffer>,
    /// The reference's floatexp points (big references; the deep kernel's full steps).
    refx_buffer: Option<B::Buffer>,
    /// Counts references made (a BLA table belongs to one).
    reference_serial: u64,
    /// Big references are computed here, off the main thread (M3b).
    reference_worker: ReferenceWorker,
    /// A finished background reference that did not serve the view it arrived for (a journey's
    /// prefetch for its next target): adopted once the view gets there.
    spare_reference: Option<Reference>,
    /// Big references computed so far and their times (selection + orbit), for reports.
    pub reference_log: Vec<ReferenceLogEntry>,
    /// The current reference's BLA table on the GPU, or its build on a thread.
    bla: Option<BlaGpu<B>>,
    bla_build: Option<(u64, JoinHandle<BlaCpu>)>,
    /// log2 of eps max|2 Z| of the current reference: no BLA block is valid at |dc| beyond it.
    bla_reach_log2: f64,
    /// Build time (s), entries and bytes of the last table.
    bla_info: (f64, usize, usize),

    palette_id: Option<String>,
    /// Dense-region colours of the palette (key, deep, filament).
    dense_cache: std::cell::RefCell<Option<DenseColors>>,
    palette_buffer: Option<B::Buffer>,

    job: Option<Job<B>>,
    generation: u64,
    in_flight: VecDeque<ChunkRecord<B>>,
    /// Lane-iterations per GPU second; adapted from every chunk, persists across jobs.
    rate: f64,
    /// GPU seconds per texel of the last completed level.
    cost_per_texel: f64,
    has_presented: bool,
    /// The current view waits for its (deep) reference: no job.
    waiting_reference: bool,
    /// Colorize submissions not finished on the GPU, with their frame serial.
    frames_in_flight: Vec<(u64, B::Submission)>,
    /// Frames committed so far (serial of the last one).
    frames_committed: u64,
    last_frame_commit: f64,
    /// When the last fully refined, full-resolution frame was committed, and its size.
    pub last_final_frame: (f64, String),
    /// When the first job frame (not history) was presented since the last reset (simulate).
    pub first_job_frame: Option<f64>,
    /// Colorize submissions that failed on the GPU (a drawable left unpainted).
    pub failed_frames: usize,
    interior_stats: Option<InteriorStats>,
    pinned_iterations: Option<(Viewport, usize)>,
    /// The learner raised the limit for a view it left unresolved: the view renders again
    /// (cleared when a job starts).
    retry_pending: bool,
    epoch: Instant,
}

impl<B: GpuBackend> Renderer<B> {
    pub fn new(backend: B) -> Result<Self, GpuError> {
        Self::with_config(backend, SchedulerConfig::default())
    }

    pub fn with_config(backend: B, config: SchedulerConfig) -> Result<Self, GpuError> {
        let prepare_pipeline = backend.pipeline("prepareChunk", &[])?;
        let mut sample_cache = SampleCache::new();
        sample_cache.warm_up(&backend);
        let finish_pipeline = backend.pipeline("finishChunk", &[])?;
        let colorize_pipeline = backend.pipeline("colorize", &[])?;
        let compose_pipeline = backend.pipeline("compose", &[])?;
        let aa_list_pipeline = backend.pipeline("aaList", &[])?;
        let survey_pipeline = backend.pipeline("survey", &[])?;
        let seed_begin_pipeline = backend.pipeline("seedBegin", &[])?;
        let seed_gate_pipeline = backend.pipeline("seedGate", &[])?;
        let seed_status = backend.create_buffer_with_data(&[0u8; 16])?;
        let survey_buffer = backend.create_buffer(SURVEY_COLS * SURVEY_ROWS * 16, Storage::Shared)?;
        let dummy_buffer = backend.create_buffer(16, Storage::Private)?;
        let state_bytes = config.live_capacity * std::mem::size_of::<PixelState>();
        let state_buffers = [
            backend.create_buffer(state_bytes, Storage::Private)?,
            backend.create_buffer(state_bytes, Storage::Private)?,
        ];
        let count_buffers = [
            backend.create_buffer(4, Storage::Private)?,
            backend.create_buffer(4, Storage::Private)?,
        ];
        let stats_buffer = backend.create_buffer(STAT_SLOTS * std::mem::size_of::<ChunkStats>(), Storage::Shared)?;
        let args_buffer = backend.create_buffer(16, Storage::Private)?;
        let level_buffer = backend.create_buffer(LEVEL_COUNTERS_SIZE, Storage::Private)?;
        let list_count_buffer = backend.create_buffer(4, Storage::Private)?;
        Ok(Self {
            backend,
            viewport: Viewport::home(),
            settings: RenderSettings::default(),
            navigator: Navigator::default(),
            config,
            stats: ShownStats::default(),
            frame_stats: None,
            prepare_pipeline,
            finish_pipeline,
            colorize_pipeline,
            compose_pipeline,
            aa_list_pipeline,
            overlay: None,
            overlay_font: None,
            overlay_pipeline: None,
            overlay_shown: None,
            survey_pipeline,
            survey_buffer,
            pending_survey: None,
            last_survey: f64::NEG_INFINITY,
            aa_list_buffer: None,
            history: None,
            history_ping: 0,
            history_view: None,
            last_compose: 0.0,
            taa_index: 0,
            refresh_interval: 1.0 / 60.0,
            last_frame_time: 0.0,
            dummy_buffer,
            sample_cache,
            cache_check_renderer: None,
            cache_checks: Vec::new(),
            cycle_phase: 0.0,
            cycle_time: 0.0,
            iterate_pipelines: std::array::from_fn(|_| None),
            ic_buffers: None,
            want_aux: false,
            seed_pipelines: Default::default(),
            seed_begin_pipeline,
            seed_gate_pipeline,
            seed_nodes: None,
            seed_tiles: None,
            seed_status,
            seed_epoch: 0,
            seed_unit: 1.5e-6,
            seed_round: None,
            seed_last: None,
            steps_per_texel: None,
            last_key_view: None,
            retry_epoch: 0,
            tile_bad: None,
            fill_epoch: 0,
            full_set: None,
            preview_alloc: None,
            state_buffers,
            count_buffers,
            stats_buffer,
            args_buffer,
            level_buffer,
            list_buffers: None,
            list_count_buffer,
            list_capacity: 0,
            stat_slot: 0,
            ping: 0,
            reference: None,
            reference_time: 0.0,
            reference_buffer: None,
            refx_buffer: None,
            reference_serial: 0,
            reference_worker: ReferenceWorker::default(),
            spare_reference: None,
            reference_log: Vec::new(),
            bla: None,
            bla_build: None,
            bla_reach_log2: f64::NEG_INFINITY,
            bla_info: (0.0, 0, 0),
            palette_id: None,
            dense_cache: std::cell::RefCell::new(None),
            palette_buffer: None,
            job: None,
            generation: 0,
            in_flight: VecDeque::new(),
            rate: 3e10,
            cost_per_texel: 0.0,
            has_presented: false,
            waiting_reference: false,
            frames_in_flight: Vec::new(),
            frames_committed: 0,
            last_frame_commit: 0.0,
            last_final_frame: (0.0, String::new()),
            first_job_frame: None,
            failed_frames: 0,
            interior_stats: None,
            pinned_iterations: None,
            retry_pending: false,
            epoch: Instant::now(),
        })
    }

    /// After a panic inside a frame (caught at the FFI boundary): forget the job and the GPU
    /// work in flight, so the next refresh starts the view afresh instead of tripping over the
    /// same half-updated state. Counted as a failed frame.
    pub fn recover_after_panic(&mut self) {
        self.job = None;
        self.in_flight.clear();
        self.seed_round = None;
        self.frames_in_flight.clear();
        self.pending_survey = None;
        self.sample_cache.reset();
        self.failed_frames += 1;
    }

    /// The motion sample cache's measurements.
    pub fn sample_cache_stats(&self) -> &CacheStats {
        &self.sample_cache.stats
    }

    pub fn reset_sample_cache_stats(&mut self) {
        self.sample_cache.stats = CacheStats::default();
    }

    /// The sample cache's settings (on/off, ratio cap, memory bound).
    pub fn sample_cache_config(&mut self) -> &mut crate::sample_cache::CacheConfig {
        &mut self.sample_cache.config
    }

    /// Chunks of anti-aliasing samples are on the GPU (tests).
    #[doc(hidden)]
    pub fn aa_chunks_in_flight(&self) -> bool {
        self.in_flight.iter().any(|r| r.aa_epoch.is_some())
    }

    /// The view waits for a deep reference (frames show the history only).
    pub fn waiting_reference(&self) -> bool {
        self.waiting_reference
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Seconds since the renderer was created (the headless tools' clock).
    pub fn clock(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    /// The iteration limit the next frame uses.
    pub fn max_iterations(&self) -> usize {
        self.settings.max_iterations(&self.viewport)
    }

    /// Forwards a decoded input event to the navigator (which moves the viewport).
    pub fn handle_input(&mut self, event: InputEvent, now: f64) -> bool {
        self.navigator.handle(event, &mut self.viewport, now)
    }

    /// Marks input that moved the view now (gesture frames follow).
    pub fn note_interaction(&mut self, now: f64) {
        self.navigator.note_interaction(now);
    }

    fn iterate_pipeline(
        &mut self,
        de: bool,
        interior: bool,
        bla: bool,
        deep: bool,
        interior_color: bool,
        seeded: bool,
    ) -> Result<B::Pipeline, GpuError> {
        let k = usize::from(de)
            | (usize::from(interior) << 1)
            | (usize::from(bla) << 2)
            | (usize::from(deep) << 3)
            | (usize::from(interior_color) << 4)
            | (usize::from(seeded) << 5);
        if let Some(p) = &self.iterate_pipelines[k] {
            return Ok(p.clone());
        }
        let p = self
            .backend
            .pipeline("iterate", &[de, interior, bla, deep, interior_color, seeded])?;
        self.iterate_pipelines[k] = Some(p.clone());
        Ok(p)
    }

    // MARK: Interior fill (M5, kb: techniques/guessing-and-subdivision)

    /// Whether a full level about to start should fill tiles whose border is all proven
    /// interior (Mariani-Silver on the 16 x 16 admission tiles), seeds aside. Only with the
    /// derivative interior test (the fill claims "proven interior", never "unresolved") and
    /// flat interior colouring (the period, multiplier and interior distance of the other
    /// modes are per pixel; a fill would leave the inside of a tile without them). Previews
    /// and gesture frames stay as they were (cheap, and a gesture frame must complete in its
    /// chunks).
    fn fill_wanted(&self, level_index: usize) -> bool {
        let Some(job) = &self.job else { return false };
        let level = &job.levels[level_index];
        if level.started()
            || level.preview
            || level.is_extension()
            || level.aa.is_some()
            || !level.use_interior
            || job.key.interior_color
        {
            return false;
        }
        // Auto: the border-first admission costs chunk occupancy (the borders are 23 % of the
        // texels, admitted alone), a short wait, and the seeds' admission coherence; it pays
        // where many pixels are interior and proving them is slow (measured M5: elephant
        // 1.38x; home-100k and minibrot-p4, whose interior proves in tens of iterations, 5-17 %
        // slower; cusp-1e5 seeded 16 % slower, no tile filled). Decided from the last level's
        // statistics, which a fill does not change (filled pixels count as interior; the
        // iterations per pixel stay above the bar where it fills).
        match self.config.interior_fill {
            SaMode::Off => false,
            SaMode::On => true,
            SaMode::Auto => self
                .interior_stats
                // (Proven interior: the kernel counts pixels left unresolved as finished too; at
                // sea-8007, 98 % unresolved, filling nothing cost 9 %.)
                .is_some_and(|s| {
                    s.fraction - s.unresolved >= FILL_MIN_INTERIOR && s.exterior_iterations >= FILL_MIN_ITERATIONS
                }),
        }
    }

    /// When a full level starts: its fill decision (`fill_wanted`, and in auto only where no
    /// tile starts from local SA seeds: the seeds' leaves are known by now).
    fn decide_fill(&mut self, level_index: usize) {
        if !self.fill_wanted(level_index) {
            return;
        }
        let Some(job) = &self.job else { return };
        let level = &job.levels[level_index];
        if self.config.interior_fill == SaMode::Auto
            && level.seeding.as_ref().is_some_and(|s| !s.done || s.tiles_seeded > 0)
        {
            return;
        }
        let (tx, ty) = tiles(level.set.width, level.set.height);
        if self.tile_bad.as_ref().is_none_or(|(_, n)| *n < tx * ty) {
            match self.backend.create_buffer_with_data(&vec![0u8; tx * ty * 4]) {
                Ok(b) => self.tile_bad = Some((b, tx * ty)),
                Err(_) => return,
            }
        }
        self.fill_epoch = self.fill_epoch.wrapping_add(1).max(1);
        let epoch = self.fill_epoch;
        let level = &mut self.job.as_mut().expect("job").levels[level_index];
        level.params.fill_border = (tx * ty * 60) as u32;
        level.params.fill_epoch = epoch;
        // (Most interior pixels prove within a few hundred iterations; slower ones are near a
        // component's boundary, where a fill would not be safe anyway.)
        level.params.fill_proof = FILL_PROOF.min(level.params.soft_iter);
        if self.config.trace {
            println!("interior fill: level {level_index}, {} tiles", tx * ty);
        }
    }

    // MARK: Local series approximation (M5, kb: techniques/local-series-approximation)

    fn seed_pipeline(
        &mut self,
        interior: bool,
        deep: bool,
        bla: bool,
        interior_color: bool,
    ) -> Result<B::Pipeline, GpuError> {
        let k = usize::from(interior)
            | (usize::from(deep) << 1)
            | (usize::from(bla) << 2)
            | (usize::from(interior_color) << 3);
        if let Some(p) = &self.seed_pipelines[k] {
            return Ok(p.clone());
        }
        let p = self
            .backend
            .pipeline("seedLevel", &[false, interior, bla, deep, interior_color, false])?;
        self.seed_pipelines[k] = Some(p.clone());
        Ok(p)
    }

    /// The quadtree layout of a `w x h` level (root size first, leaf last).
    fn seed_layout(sa: LocalSa, w: usize, h: usize, soft_iter: u32, jitter: [f32; 2]) -> Vec<SeedParams> {
        let leaf = sa.leaf.clamp(2, 16).next_power_of_two();
        let top = sa.top.max(leaf).next_power_of_two();
        let (tx, ty) = tiles(w, h);
        let mut levels: Vec<SeedParams> = Vec::new();
        let mut size = top;
        let mut offset = 0usize;
        while size >= leaf {
            let (gw, gh) = (w.div_ceil(size), h.div_ceil(size));
            let parent = levels.last().copied();
            levels.push(SeedParams {
                grid_w: gw as u32,
                grid_h: gh as u32,
                size: size as u32,
                offset: offset as u32,
                parent_grid_w: parent.map_or(0, |p| p.grid_w),
                parent_offset: parent.map_or(0, |p| p.offset),
                root: u32::from(parent.is_none()),
                soft_iter,
                radius: (0.5 * size as f64 * std::f64::consts::SQRT_2) as f32,
                eps: sa.eps as f32,
                epoch: 0,
                budget: 0,
                jitter,
                tiles_x: tx as u32,
                gate_mode: if sa.mode == SaMode::On { 2 } else { 1 },
                gate_rel: sa.gate_rel as f32,
                gate_abs: 0.0,
                tiles_y: ty as u32,
                truncate: 0,
            });
            offset += gw * gh;
            size /= 2;
        }
        levels
    }

    /// Starts a level's seed passes: layout, buffers (grown, never shrunk), a new epoch, and
    /// the first round. `unbounded`: the round runs every node to its branch (gesture frames,
    /// whose chunks also run to the limit).
    fn start_seeding(
        &mut self,
        level_index: usize,
        interior: bool,
        interior_color: bool,
        unbounded: bool,
    ) -> Result<(), GpuError> {
        let sa = self.config.local_sa;
        let (params, deep, bla) = {
            let job = self.job.as_ref().expect("job");
            let l = &job.levels[level_index];
            // Seeds take BLA blocks in the deep kernel when the level does (they would otherwise
            // iterate in floatexp what the pixels' blocks skip; R5).
            (l.params, job.deep, l.use_bla && job.deep && job.bla.is_some())
        };
        let w = params.width as usize;
        let h = (params.height as usize).max(1);
        self.seed_epoch = self.seed_epoch.wrapping_add(1) & 0x3fff_ffff;
        let mut layout = Self::seed_layout(sa, w, h, params.soft_iter, params.jitter);
        // The gate's bar in seed steps, from the last level's cost per texel (see seedGate).
        let bar = self.steps_per_texel.map_or(0.0, |s| (sa.gate_rel * s).max(sa.gate_abs)) as f32;
        for l in &mut layout {
            l.epoch = self.seed_epoch;
            l.gate_abs = bar;
        }
        let nodes: usize = layout.iter().map(|l| (l.grid_w * l.grid_h) as usize).sum();
        if self.seed_nodes.as_ref().is_none_or(|(_, n)| *n < nodes) {
            // (Shared: the CLI reads the nodes back for its report. Zeroed: stamps start "new".)
            let buf = self
                .backend
                .create_buffer_with_data(&vec![0u8; nodes * std::mem::size_of::<SeedNode>()])?;
            self.seed_nodes = Some((buf, nodes));
        }
        let (tx, ty) = tiles(w, h);
        if self.seed_tiles.as_ref().is_none_or(|(_, n)| *n < tx * ty) {
            let buf = self.backend.create_buffer(tx * ty * 4, Storage::Shared)?;
            self.seed_tiles = Some((buf, tx * ty));
        }
        // Make sure the pipeline exists before the level counts as seeded.
        self.seed_pipeline(interior, deep, bla, interior_color)?;
        // Gesture frames (M5): one round whose serial chain of seed steps (each level continues
        // its parent's orbit, on few threads: the step latency counts) takes about a third of
        // the frame budget; nodes out of budget branch where they are. Unbounded seeds stalled
        // the spiral's gesture frames at 12 fps (root seeds run ~23 000 steps).
        // (Also the preview of a refined view: previews are never compared with fresh renders,
        // and one waiting for several rounds delayed the first image after every look switch.)
        let gesture = self
            .job
            .as_ref()
            .is_some_and(|j| j.interactive || j.levels[level_index].preview);
        let mut unbounded = unbounded;
        if gesture {
            let chain = 0.3 * self.gesture_budget() / (self.seed_unit * layout.len() as f64);
            let b = chain.clamp(64.0, f64::from(params.soft_iter.max(64))) as u32;
            // (Seeds that cannot get as far as the auto gate's bar would only cost time.)
            if sa.mode == SaMode::Auto && f64::from(b) * (layout.len() as f64) < f64::from(bar) {
                return Ok(());
            }
            for l in &mut layout {
                l.truncate = 1;
                l.budget = b;
            }
            unbounded = false;
        }
        let leaf = *layout.last().expect("leaf level");
        let level = &mut self.job.as_mut().expect("job").levels[level_index];
        level.seeded = Some(leaf);
        level.seeding = Some(Seeding {
            layout,
            done: false,
            tiles_seeded: 0,
            path_sum: 0,
            unfinished: None,
            rounds: 0,
            gpu_time: 0.0,
            interior,
            interior_color,
            bla,
        });
        self.encode_seed_round(level_index, if unbounded { u32::MAX } else { 0 })
    }

    /// One seed round (one command buffer): reset the status, one pass per quadtree level with
    /// a step budget per node, then the per-tile gate. `budget` 0: from the throughput model.
    fn encode_seed_round(&mut self, level_index: usize, budget: u32) -> Result<(), GpuError> {
        let variant = {
            let job = self.job.as_ref().expect("job");
            let Some(sd) = &job.levels[level_index].seeding else {
                return Ok(());
            };
            (sd.interior, job.deep, sd.bla, sd.interior_color)
        };
        let pipeline = self.seed_pipeline(variant.0, variant.1, variant.2, variant.3)?;
        let job = self.job.as_ref().expect("job");
        let level = &job.levels[level_index];
        let Some(seeding) = &level.seeding else { return Ok(()) };
        // Steps per node and level: a level of `waves` x SEED_WAVE nodes costs about
        // budget x waves x `seed_unit` (the root levels run on few threads, where only the
        // dependent step latency counts), and the round's levels share about a chunk of GPU
        // time. `seed_unit` is learnt from rounds whose budget bound.
        // (At most the nodes the last round left: a tail of a few slow nodes gets long passes.)
        let left = seeding.unfinished.unwrap_or(usize::MAX);
        let waves = |l: &SeedParams| {
            (((l.grid_w * l.grid_h) as usize).min(left) as f64 / SEED_WAVE)
                .ceil()
                .max(1.0)
        };
        let per_level = self.config.chunk_target / (self.seed_unit * seeding.layout.len() as f64);
        let budgets: Vec<u32> = seeding
            .layout
            .iter()
            .map(|l| {
                if l.truncate != 0 {
                    l.budget
                } else if self.config.local_sa.budget != 0 {
                    self.config.local_sa.budget
                } else if budget != 0 {
                    budget
                } else {
                    (per_level / waves(l)).clamp(256.0, f64::from(l.soft_iter.max(256))) as u32
                }
            })
            .collect();
        let units: f64 = seeding
            .layout
            .iter()
            .zip(&budgets)
            .map(|(l, &b)| f64::from(b) * waves(l))
            .sum();
        let (Some((nodes_buf, _)), Some((tiles_buf, _))) = (&self.seed_nodes, &self.seed_tiles) else {
            return Ok(());
        };
        let mut enc = self.backend.begin("seed round")?;
        enc.set_pipeline(&self.seed_begin_pipeline);
        enc.set_buffer(20, &self.seed_status, 0);
        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
        enc.set_pipeline(&pipeline);
        enc.set_bytes(0, bytes_of(&level.params));
        enc.set_buffer(1, &job.reference_buffer, 0);
        enc.set_buffer(16, job.refx_buffer.as_ref().unwrap_or(&self.dummy_buffer), 0);
        match (&job.bla, seeding.bla) {
            (Some(b), true) => {
                enc.set_bytes(14, bytes_of(&b.params));
                enc.set_buffer(17, &b.buffer, 0);
            }
            _ => {
                enc.set_bytes(14, bytes_of(&BlaParams::default()));
                enc.set_buffer(17, &self.dummy_buffer, 0);
            }
        }
        enc.set_buffer(18, nodes_buf, 0);
        enc.set_buffer(20, &self.seed_status, 0);
        for (l, &b) in seeding.layout.iter().zip(&budgets) {
            let mut lp = *l;
            lp.budget = b;
            enc.set_bytes(19, bytes_of(&lp));
            enc.dispatch_threads([(l.grid_w * l.grid_h) as usize, 1, 1], [64, 1, 1]);
        }
        let leaf = *seeding.layout.last().expect("leaf");
        enc.set_pipeline(&self.seed_gate_pipeline);
        enc.set_buffer(18, nodes_buf, 0);
        enc.set_bytes(19, bytes_of(&leaf));
        enc.set_buffer(22, tiles_buf, 0);
        enc.dispatch_threads([(leaf.tiles_x * leaf.tiles_y) as usize, 1, 1], [64, 1, 1]);
        let submission = enc.commit();
        let generation = job.generation;
        self.seed_round = Some(SeedRound {
            submission,
            generation,
            level: level_index,
            units,
        });
        if let Some(s) = &mut self.job.as_mut().expect("job").levels[level_index].seeding {
            s.rounds += 1;
        }
        Ok(())
    }

    /// Collects a finished seed round: GPU time, and whether every node is final. `wait`
    /// blocks for it.
    fn harvest_seed_round(&mut self, wait: bool) {
        let Some(round) = &self.seed_round else { return };
        if wait {
            round.submission.wait();
        }
        let status = round.submission.status();
        if status == SubmissionStatus::Pending {
            return;
        }
        let round = self.seed_round.take().expect("round");
        let gpu = round.submission.gpu_time();
        if let Some(fs) = &mut self.frame_stats {
            fs.chunk_gpu += gpu;
        }
        let mut bytes = [0u8; 16];
        self.backend.read_buffer(&self.seed_status, 0, &mut bytes);
        let st: crate::gpu_types::SeedStatus = bytemuck::pod_read_unaligned(&bytes);
        let unfinished = st.unfinished;
        let seeded_tiles = st.seeded_tiles as usize;
        let path_sum = (u64::from(st.path_hi) << 32) | u64::from(st.path_lo);
        let failed = matches!(status, SubmissionStatus::Failed(_));
        if unfinished > 0 && gpu > 1e-3 && round.units.is_finite() && round.units > 0.0 {
            // The budget bound: learn the cost of a budget step. Up quickly, down slowly: a
            // round whose nodes mostly finished early under-reads the cost.
            let unit = gpu / round.units;
            self.seed_unit = unit.clamp(self.seed_unit * 0.9, self.seed_unit * 4.0);
        }
        if let SubmissionStatus::Failed(e) = &status {
            eprintln!("almondbread: seed round failed ({e}); retrying with smaller rounds");
            self.rate /= 8.0;
        }
        let trace = self.config.trace;
        let Some(job) = &mut self.job else { return };
        if job.generation != round.generation {
            return;
        }
        if failed {
            job.failed = true;
            return;
        }
        job.gpu_time += gpu;
        let Some(level) = job.levels.get_mut(round.level) else {
            return;
        };
        level.gpu_time += gpu;
        let Some(seeding) = &mut level.seeding else { return };
        seeding.gpu_time += gpu;
        seeding.done = unfinished == 0;
        seeding.unfinished = Some(unfinished as usize);
        seeding.tiles_seeded = seeded_tiles;
        seeding.path_sum = path_sum;
        if seeding.done && seeded_tiles == 0 {
            // No tile starts from its seeds: the rest of the level (and its extension passes
            // and AA samples) runs the plain pipeline, which is a few % faster and gives the
            // same fields bit for bit (checked: elephant, `--sa 0` vs auto).
            level.seeded = None;
        }
        if trace {
            println!(
                "seed round {} of level {}: {unfinished} nodes unfinished, {seeded_tiles} tiles seeded, gpu {:.2} ms",
                seeding.rounds,
                round.level,
                gpu * 1000.0
            );
        }
        if seeding.done {
            self.seed_last = Some((seeding.layout.clone(), seeding.gpu_time, seeding.rounds));
        }
    }

    /// The level's seeds are not final yet: encodes the next round when none is on the GPU.
    /// Returns whether iterate chunks may be encoded now (they admit nothing until the seeds
    /// are final; only the first round is followed by such chunks right away, so a view whose
    /// seeds finish in one round starts its pixels in the same refresh).
    fn advance_seeding(&mut self, level_index: usize) -> bool {
        let Some(job) = &self.job else { return false };
        let level = &job.levels[level_index];
        let Some(seeding) = &level.seeding else { return true };
        if seeding.done {
            return true;
        }
        let first = seeding.rounds <= 1;
        if self.seed_round.is_some() {
            return first && !(self.config.interior_fill == SaMode::Auto && self.fill_wanted(level_index));
        }
        if let Err(e) = self.encode_seed_round(level_index, 0) {
            eprintln!("almondbread: seed round failed: {e}");
            if let Some(j) = &mut self.job {
                j.failed = true;
            }
            return false;
        }
        false
    }

    /// What the last finished seeding did (reads the nodes back; headless reports).
    pub fn seed_report(&self) -> Option<SeedReport> {
        let (layout, gpu_time, rounds) = self.seed_last.as_ref()?;
        let (buf, _) = self.seed_nodes.as_ref()?;
        let total: usize = layout.iter().map(|l| (l.grid_w * l.grid_h) as usize).sum();
        let mut bytes = vec![0u8; total * std::mem::size_of::<SeedNode>()];
        self.backend.read_buffer(buf, 0, &mut bytes);
        let nodes: Vec<SeedNode> = bytes
            .as_chunks::<{ std::mem::size_of::<SeedNode>() }>()
            .0
            .iter()
            .map(|c| bytemuck::pod_read_unaligned(c))
            .collect();
        let mut report = SeedReport {
            gpu_time: *gpu_time,
            rounds: *rounds,
            ..SeedReport::default()
        };
        for l in layout {
            let (mut iters, mut steps, mut nsum) = (0u64, 0u64, 0f64);
            let count = (l.grid_w * l.grid_h) as usize;
            for i in 0..count {
                let nd = nodes[l.offset as usize + i];
                let parent_n = if l.root != 0 {
                    0
                } else {
                    let (x, y) = (i as u32 % l.grid_w, i as u32 / l.grid_w);
                    nodes[(l.parent_offset + (y / 2) * l.parent_grid_w + x / 2) as usize].n
                };
                iters += u64::from(nd.n.saturating_sub(parent_n));
                steps += u64::from(nd.steps);
                nsum += f64::from(nd.n);
            }
            report
                .levels
                .push((l.size as usize, count, iters, steps, nsum / count.max(1) as f64));
        }
        if let Some(leaf) = layout.last() {
            let count = (leaf.grid_w * leaf.grid_h) as usize;
            report.leaves = count;
            report.leaf_path = nodes[leaf.offset as usize..leaf.offset as usize + count]
                .iter()
                .map(|n| f64::from(n.path))
                .sum::<f64>()
                / count.max(1) as f64;
            report.interior_leaves = nodes[leaf.offset as usize..leaf.offset as usize + count]
                .iter()
                .filter(|n| n.stamp & 3 == 3)
                .map(|n| n.n)
                .collect();
            report.interior_leaves.sort_unstable();
        }
        if let (Some((tiles, _)), Some(leaf)) = (&self.seed_tiles, layout.last()) {
            let n = (leaf.tiles_x * leaf.tiles_y) as usize;
            let mut t = vec![0u8; n * 4];
            self.backend.read_buffer(tiles, 0, &mut t);
            report.tiles = n;
            report.tiles_seeded = t
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|c| u32::from_le_bytes(**c) != 0)
                .count();
        }
        Some(report)
    }

    // MARK: Interactive frames

    /// One display refresh: integrate held keys, harvest, start or keep the job, encode
    /// chunks, colorize into the target `acquire` returns (with the drawable to present, if
    /// any). `acquire(final_frame)` is called only when a frame will be committed;
    /// returning None (no drawable available without blocking) skips presentation this
    /// refresh. `final_frame` marks the view's refined frame, which a surface may wait for.
    pub fn render_frame(
        &mut self,
        now: f64,
        width: usize,
        height: usize,
        points_per_pixel: f64,
        acquire: &mut dyn FnMut(bool) -> Option<FrameTarget<B>>,
    ) -> FrameOutcome {
        self.render_frame_gated(now, width, height, points_per_pixel, true, acquire)
    }

    /// [`Renderer::render_frame`] for a surface that can tell in advance whether a target is
    /// available without blocking: with `target_ready` false during a gesture the refresh is
    /// skipped before any GPU work is encoded (a gesture frame that cannot be shown would
    /// only delay the next one), like a refresh with too many frames in flight.
    pub fn render_frame_gated(
        &mut self,
        now: f64,
        width: usize,
        height: usize,
        points_per_pixel: f64,
        target_ready: bool,
        acquire: &mut dyn FnMut(bool) -> Option<FrameTarget<B>>,
    ) -> FrameOutcome {
        self.navigator.set_cinematic(self.settings.cinematic);
        if self.navigator.wants_feedback() {
            // How the renderer keeps up, for the autopilot's and fly-to's speed (kb: decisions/controls,
            // autopilot): the gesture-preview resolution the frame budget affords (unclamped),
            // and whether the view waits for a deep reference.
            let texels = (width * height) as f64;
            let fit = if self.cost_per_texel > 0.0 && texels > 0.0 {
                (self.gesture_budget() / (self.cost_per_texel * texels)).sqrt()
            } else {
                f64::NAN
            };
            let feedback = RenderFeedback {
                fit,
                waiting_reference: self.waiting_reference,
                at_limit: self.viewport.at_precision_limit(),
            };
            self.navigator.autopilot_mut().set_feedback(feedback);
        }
        let keep_going = self.navigator.advance(&mut self.viewport, now);
        let mut outcome = FrameOutcome {
            animating: keep_going,
            presented: false,
        };
        if width == 0 || height == 0 || points_per_pixel <= 0.0 {
            return outcome;
        }
        let interacting = self.navigator.is_interacting(now);
        // The display's refresh interval, from consecutive frames (gesture frame budget).
        let since = now - self.last_frame_time;
        if since > 0.002 && since < 0.05 {
            self.refresh_interval = 0.9 * self.refresh_interval + 0.1 * since;
        }
        self.last_frame_time = now;
        // Colour cycling advances only while the view rests (flying keeps colours still, so
        // the temporal reprojection stays valid), except the cinematic cycling while flying,
        // slow enough for the reprojected history (a few thousandths of a cycle per frame).
        let fly_cycle = self.settings.cinematic.fly_cycle_rate();
        if self.settings.cycle_speed != 0.0 && !interacting {
            let dt = (now - self.cycle_time).clamp(0.0, 0.1);
            self.cycle_phase = (self.cycle_phase + self.settings.cycle_speed * dt).rem_euclid(1.0);
        } else if fly_cycle != 0.0 && self.navigator.is_flying() {
            let dt = (now - self.cycle_time).clamp(0.0, 0.1);
            self.cycle_phase = (self.cycle_phase + fly_cycle * dt).rem_euclid(1.0);
        }
        self.cycle_time = now;

        // Never block on the GPU: with frames still queued, skip this refresh (the next one
        // picks up the latest input). Safety: a drawable never shown might never complete.
        let (failed, frame_stats) = (&mut self.failed_frames, &mut self.frame_stats);
        self.frames_in_flight.retain(|(_, s)| match s.status() {
            SubmissionStatus::Pending => true,
            SubmissionStatus::Completed => {
                if let Some(fs) = frame_stats.as_mut() {
                    fs.colorize_gpu += s.gpu_time();
                }
                false
            }
            SubmissionStatus::Failed(_) => {
                *failed += 1;
                false
            }
        });
        if !self.frames_in_flight.is_empty() && now - self.last_frame_commit > 0.25 {
            self.frames_in_flight.clear();
        }
        let busy = self.frames_in_flight.len() >= self.config.max_frames_in_flight;
        // Without a target, a gesture frame is pointless work; refinement keeps encoding its
        // chunks (presentation is skipped at acquire), or the final frame would arrive
        // several refreshes later (measured: neck zoom +50 ms).
        if busy || (!target_ready && interacting) {
            if let Some(fs) = &mut self.frame_stats {
                fs.skipped += 1;
                if busy {
                    fs.skipped_in_flight += 1;
                } else {
                    fs.skipped_no_target += 1;
                }
            }
            outcome.animating = true;
            return outcome;
        }
        self.harvest();
        self.seed_sample_cache();
        self.poll_survey(now);
        let key = self.make_key(width, height, points_per_pixel, false);
        if self.job.as_ref().is_some_and(|j| j.failed) {
            self.job = None;
        }
        let (same_key, job_interactive, first_complete, job_start) = match &self.job {
            Some(j) => (j.key == key, j.interactive, j.levels[0].complete, j.start),
            None => (false, false, false, 0.0),
        };
        let has_job = self.job.is_some();
        if interacting {
            // Every frame of a gesture shows its own view: a preview level sized to fit the
            // frame budget, computed to completion (kb: techniques/frame-reuse-and-progressive).
            if !same_key || !job_interactive {
                // An unfinished sample cache batch carries over into the new frame's job.
                let carried = self.take_cache_level();
                let old = self.job.as_ref().map(|j| j.generation);
                self.start_job(key.clone(), JobMode::Interactive, now);
                if self.job.as_ref().map(|j| j.generation) == old {
                    // (No new job: the level goes back.)
                    if let (Some((l, _)), Some(j)) = (carried, &mut self.job) {
                        j.levels.insert(0, l);
                    }
                } else {
                    self.attach_cache_level(carried);
                }
            }
        } else if has_job && same_key {
            // Input idle: refine the gesture's last frame at full resolution over its preview.
            if job_interactive {
                self.extend_for_refinement();
            }
        } else if has_job && !job_interactive && !first_complete && now - job_start < 0.25 {
            // A settings change while idle: let a young first level finish before replacing it.
        } else {
            self.start_job(key.clone(), JobMode::Refine, now);
        }
        if self.waiting_reference {
            // M3b: a deep view whose reference is still computing (kb:
            // techniques/frame-reuse-and-progressive): no job yet; show the history reprojected
            // into the current view and keep polling.
            self.job = None;
            outcome.animating = true;
            if self.history_view.as_ref().is_none_or(|h| h.viewport != key.viewport)
                && let Some((target, drawable)) = acquire(false)
            {
                let colors = self.color_params();
                if let Ok(mut enc) = self.encode_history_only(&target, &colors, &key) {
                    if let Some(d) = &drawable {
                        enc.present(d);
                    }
                    let sub = enc.commit();
                    self.frames_committed += 1;
                    self.frames_in_flight.push((self.frames_committed, sub));
                    self.last_frame_commit = now;
                    self.has_presented = true;
                    outcome.presented = true;
                    if let Some(fs) = &mut self.frame_stats {
                        fs.frames += 1;
                        fs.preview_frames += 1;
                        fs.waiting_frames += 1;
                    }
                }
            }
            return outcome;
        }
        if self.job.is_none() {
            return outcome;
        }
        let aa_target = self.aa_target();
        self.prepare_aa_list(aa_target);
        self.queue_aa_sample(aa_target);
        let job = self.job.as_ref().expect("job");
        if self.config.trace {
            let d = &job.levels[job.display()];
            println!(
                "draw: job {} {} level {}x{} interactive {}",
                job.generation,
                if d.preview { "preview" } else { "full" },
                d.set.width,
                d.set.height,
                if job.interactive { "yes" } else { "no" }
            );
        }
        if job.interactive {
            self.encode_interactive();
        } else {
            let aa = job.current().is_some_and(|i| job.levels[i].aa.is_some());
            let budget = if aa {
                self.config.aa_chunk_target
            } else {
                self.config.chunk_target
            };
            self.encode_chunks(budget);
        }

        let colors = self.color_params();
        let plan = self.plan_frame(now, &colors, aa_target);
        // The refined frame (first presentation of a complete, non-gesture job) may wait for
        // a target; every other frame takes one only if it is free (see `render_frame`).
        let job = self.job.as_ref().expect("job");
        let final_frame = !job.interactive && job.complete() && !job.complete_presented;
        if let Some((mode, aa_level)) = plan
            && let Some((target, drawable)) = acquire(final_frame)
        {
            // Measurement mode (`ALMONDBREAD_SERIAL_COMPOSE=1`): the GPU runs the frame's
            // chunks to the end first, so the colour passes' GPU times are their own (Apple GPUs
            // overlap consecutive command buffers; a compose overlapping a 20 ms chunk reports
            // 20 ms).
            if self.config.serial_compose {
                for r in &self.in_flight {
                    r.submission.wait();
                }
            }
            if let Ok(mut enc) = self.encode_compose(&target, &colors, mode, aa_level, now) {
                if let Some(d) = &drawable {
                    enc.present(d);
                }
                let job = self.job.as_ref().expect("job");
                if let Some(fs) = &mut self.frame_stats {
                    fs.frames += 1;
                    if mode == COMPOSE_GESTURE || job.levels[job.display()].preview {
                        fs.preview_frames += 1;
                        fs.preview_scale_sum += job.levels[job.display()].ratio;
                    }
                }
                let sub = enc.commit();
                self.frames_committed += 1;
                self.frames_in_flight.push((self.frames_committed, sub));
                self.last_frame_commit = now;
                self.last_compose = now;
                self.first_job_frame.get_or_insert(now);
                self.has_presented = true;
                outcome.presented = true;
                let job = self.job.as_mut().expect("job");
                job.presented = true;
                match mode {
                    COMPOSE_AA => job.aa_composed += 1,
                    COMPOSE_EXACT => {
                        // Recoloured from the exact samples: anti-aliasing starts over.
                        job.drop_aa();
                        job.complete_at = Some(f64::NEG_INFINITY);
                    }
                    _ => {}
                }
                if job.complete() && !job.interactive {
                    let d = &job.levels[job.display()];
                    // The refined frame: unresolved pixels may keep resolving later.
                    if !job.complete_presented && !d.preview {
                        self.last_final_frame = (now, format!("{}x{}", d.set.width, d.set.height));
                    }
                    if !d.preview {
                        job.complete_presented = true;
                        job.complete_at.get_or_insert(now);
                    }
                }
            }
        } else if plan.is_some()
            && let Some(fs) = &mut self.frame_stats
        {
            fs.skipped += 1;
            fs.skipped_no_target += 1;
        }
        // The next AA sample right after the previous one was composed (it reuses the same
        // textures), so a sample costs one refresh, not two.
        if !self.job.as_ref().expect("job").interactive && plan.is_some_and(|p| p.0 == COMPOSE_AA) {
            self.queue_aa_sample(aa_target);
            let budget = self.config.aa_chunk_target;
            self.encode_chunks(budget);
        }
        // Settled: refined, faded in, extended and anti-aliased.
        let fade = 4.0 * self.config.fade_tau;
        let job = self.job.as_mut().expect("job");
        if !job.interactive
            && job.complete_presented
            && job.levels.iter().all(|l| l.done || l.aa.is_some())
            && job.complete_at.is_some_and(|t| now - t >= fade)
            && !job.aa_pending()
            && job.aa_finished(aa_target, self.config.aa_budget)
        {
            job.final_presented = true;
        }
        // Only the overlay changed (the pointer moved the orbit, a map finished): present the
        // shown image again with it, from the history (no recolouring, ~0.2 ms GPU).
        let mut overlay_pending = false;
        if !outcome.presented && self.overlay_stale() {
            overlay_pending = true;
            if self.frames_in_flight.len() < self.config.max_frames_in_flight
                && let Some((target, drawable)) = acquire(false)
                && let Ok(mut enc) = self.encode_overlay_only(&target)
            {
                if let Some(d) = &drawable {
                    enc.present(d);
                }
                let sub = enc.commit();
                self.frames_committed += 1;
                self.frames_in_flight.push((self.frames_committed, sub));
                self.last_frame_commit = now;
                outcome.presented = true;
                overlay_pending = false;
                if let Some(fs) = &mut self.frame_stats {
                    fs.frames += 1;
                }
            }
        }
        self.publish_stats();
        let job = self.job.as_ref().expect("job");
        let busy = overlay_pending
            || interacting
            || !job.final_presented
            || job.key != key
            || self.retry_pending
            || self.settings.cycle_speed != 0.0;
        outcome.animating = keep_going || busy;
        outcome
    }

    /// Samples per pixel the idle anti-aliasing accumulates (the supersampling setting's
    /// factor squared; 1 while colours cycle, since every frame is recoloured).
    fn aa_target(&self) -> u32 {
        if self.settings.cycle_speed != 0.0 {
            1
        } else {
            (self.settings.supersampling.factor() * self.settings.supersampling.factor()) as u32
        }
    }

    /// What this refresh composes, if anything: the compose mode and, for an AA sample, its
    /// level (kb: techniques/frame-reuse-and-progressive, FL).
    fn plan_frame(&self, now: f64, colors: &ColorParams, aa_target: u32) -> Option<(u32, Option<usize>)> {
        let job = self.job.as_ref()?;
        if job.interactive {
            return (!job.presented || !job.done()).then_some((COMPOSE_GESTURE, None));
        }
        let colors_changed = match &self.history_view {
            Some(h) => {
                !self.same_colors(h, colors) || h.offset != colors.offset || h.colors.0.hue_rot != colors.hue_rot
            }
            None => true,
        };
        let exact_done = job.levels.iter().all(|l| l.done || l.aa.is_some());
        if !job.complete_presented {
            // While refining, hold the previous image for a moment until the first level is
            // complete, so cheap views appear finished in one step.
            let show = !self.has_presented || job.levels[0].complete || now - job.start > 0.15;
            return show.then_some((COMPOSE_REFINE, None));
        }
        if colors_changed {
            return Some((COMPOSE_EXACT, None));
        }
        let fading = job.complete_at.is_none_or(|t| now - t < 4.0 * self.config.fade_tau);
        if fading || !exact_done {
            return Some((COMPOSE_REFINE, None));
        }
        if !job.aa_finished(aa_target, self.config.aa_budget)
            && let Some(i) = job.aa_ready()
        {
            return Some((COMPOSE_AA, Some(i)));
        }
        None
    }

    /// The history was coloured like `colors` (phase aside) with the current palette.
    fn same_colors(&self, h: &HistoryView, colors: &ColorParams) -> bool {
        let mut a = h.colors.0;
        a.offset = colors.offset;
        // The hue drifts with depth like the phase: a zoom must not drop the history.
        a.hue_rot = colors.hue_rot;
        a == *colors && h.colors.1 == self.palette_key()
    }

    fn make_key(&mut self, width: usize, height: usize, points_per_pixel: f64, offscreen: bool) -> ComputeKey {
        // A jump (not a continuous move from the last view): what was learned there says
        // nothing here. After a dive at c = i (1e-280) and a jump to the spiral, its interior-test
        // switch (off: nothing interior at the dendrite) drew the spiral's unresolved disc
        // interior, and the learner could not retry without the test: an all-black settled
        // frame (M5 fix).
        if let Some(prev) = &self.last_key_view {
            let view = self.viewport.scale * (width.max(height) as f64 * points_per_pixel);
            let ratio = prev.scale.ratio(self.viewport.scale);
            let far = prev.center.distance_in(&self.viewport.center, view);
            if !(ratio < 8.0 && ratio > 1.0 / 8.0 && far < 2.0) {
                if self.config.trace {
                    println!("jump: learned iteration state reset");
                }
                self.settings.learned_iterations = 0;
                self.pinned_iterations = None;
                self.interior_stats = None;
                self.steps_per_texel = None;
                // No sample reuse across a jump either (its statistics would come back too).
                if !offscreen {
                    self.sample_cache.reset();
                }
            }
        }
        self.last_key_view = Some(self.viewport.clone());
        let mut max_iter = self.max_iterations();
        // A learned limit applies from the next view on: the view it was learned at is
        // already resolved by the extension, re-rendering it would only repeat the work.
        if let Some((vp, pinned)) = &self.pinned_iterations {
            if *vp == self.viewport && self.settings.auto_iterations {
                max_iter = *pinned;
            } else {
                self.pinned_iterations = None;
            }
        }
        // (As `start_job` raises it once the reference is there, so the job keeps its key.)
        let ss = if offscreen {
            self.settings.supersampling.factor()
        } else {
            1
        };
        let texel = self.viewport.scale * points_per_pixel / ss as f64;
        let viewport = self.viewport.clone();
        max_iter = max_iter.max(self.period_floor(&viewport, texel, width.max(height) * ss, max_iter));
        ComputeKey {
            viewport: self.viewport.clone(),
            max_iter,
            points_per_pixel,
            de: self.settings.de_needed(),
            interior: self.settings.interior_detection,
            interior_color: self.settings.interior_mode.needs_analysis(),
            width,
            height,
            // Interactive views anti-alias progressively at 1x (a supersampling change only
            // changes the number of idle samples); offscreen renders use a grid.
            supersampling: if offscreen {
                self.settings.supersampling.factor()
            } else {
                1
            },
            retry: self.retry_epoch,
        }
    }

    // MARK: Jobs and chunks

    /// Auto iterations from the view's big reference (M5), so a view that needs far more than
    /// the depth heuristic is not first rendered fully unresolved and then again at 4x (the
    /// learner's retry; the user's spiral-32412-1e19 spent 37 % of its iterations on that
    /// pass): the nucleus of a period-P minibrot found in the view needs more than P
    /// iterations, so the limit starts at 1.25 P; a centre orbit (no nucleus) that escapes
    /// only after the heuristic, or not within its length, starts at the retry level, 4x the
    /// heuristic (cheap where most pixels escape earlier: a limit costs only the pixels that
    /// reach it). Capped like the learner (4x the heuristic) and at the user's cap; a function
    /// of the reference alone, so the key `make_key` builds stays the job's. 0: none applies.
    fn period_floor(&self, viewport: &Viewport, texel: Scale, size: usize, max_iter: usize) -> usize {
        if !self.settings.auto_iterations || !self.config.iteration_floor {
            return 0;
        }
        let Some(r) = self.reference.as_ref().filter(|r| r.big) else {
            return 0;
        };
        if r.fit(viewport, texel, size, max_iter) == Fit::No {
            return 0;
        }
        let heuristic = depth_iterations(viewport);
        let floor = match r.period {
            Some(p) => (p as f64 * 1.25).ceil() as usize,
            // (A centre orbit runs to twice the limit it was made for; its last index is where
            // it escaped, or how far it is known not to.)
            None if r.points.len() - 1 > heuristic => 4 * heuristic,
            None => 0,
        };
        floor.min(4 * heuristic).min(self.settings.max_iterations_cap)
    }

    fn start_job(&mut self, mut key: ComputeKey, mode: JobMode, now: f64) {
        // A gesture frame superseded before its preview completed never reported its cost:
        // what its finished chunks took is a lower bound (M5: after a refined frame made cheap
        // by local SA the dynamic resolution started too fine at the neck and stayed there).
        if mode == JobMode::Interactive
            && let Some(old) = &self.job
            && old.interactive
            && let Some(l) = old.levels.first()
            && !l.complete
            && l.gpu_time > 0.0
            && l.cache.is_none()
        {
            let lower = l.gpu_time / (l.set.width * l.set.height).max(1) as f64;
            self.cost_per_texel = self.cost_per_texel.max(lower);
        }
        self.waiting_reference = false;
        self.retry_pending = false;
        // Interactive views anti-alias progressively (idle AA samples); only offscreen renders
        // supersample on a grid (kb: techniques/anti-aliasing, FL).
        let ss_factor = if mode == JobMode::Offscreen {
            key.supersampling
        } else {
            1
        };
        let ss = ss_factor as f64;
        let tex_w = key.width * ss_factor;
        let tex_h = key.height * ss_factor;
        self.want_aux = key.interior_color;
        if key.interior_color && self.ic_buffers.is_none() {
            let bytes = self.config.live_capacity * std::mem::size_of::<PixelStateIC>();
            if let (Ok(a), Ok(b)) = (
                self.backend.create_buffer(bytes, Storage::Private),
                self.backend.create_buffer(bytes, Storage::Private),
            ) {
                self.ic_buffers = Some([a, b]);
            } else {
                return;
            }
        }
        let Ok(full) = self.full_texture_set(tex_w, tex_h) else {
            return;
        };
        self.ensure_list_capacity(tex_w * tex_h);
        let viewport = key.viewport.clone();
        let full_texel = viewport.scale * key.points_per_pixel / ss;
        if !self.prepare_reference(&key, full_texel, tex_w.max(tex_h), mode == JobMode::Offscreen) {
            // Deep view without a usable reference yet: the caller shows the history.
            self.waiting_reference = true;
            return;
        }
        key.max_iter = key
            .max_iter
            .max(self.period_floor(&viewport, full_texel, tex_w.max(tex_h), key.max_iter));
        let (Some(reference), Some(reference_buffer)) = (&self.reference, self.reference_buffer.clone()) else {
            return;
        };
        let deep = reference.deep_kernel;
        let refx_buffer = if deep { self.refx_buffer.clone() } else { None };
        let big = reference.big;
        let ref_len = reference.points.len() as u32;
        let reference_serial = self.reference_serial;
        let half_diag = 0.5 * (key.width as f64).hypot(key.height as f64);
        // Plain kernel: f64 offsets and texel sizes. Deep kernel (M3b): mantissas in units of
        // 2^scaleExp, the texel's binary exponent (kb: techniques/extended-range-floats).
        let (dre, dim) = if deep {
            (0.0, 0.0)
        } else if big {
            // Plain kernel on a big reference: the offset in f64 (views >= 1e-18 per point).
            let u = Scale::from_f64(1.0);
            viewport.center.delta_in(&reference.point, u)
        } else {
            viewport.center.delta_from(&reference.point)
        };
        let max_dc_log2 = if deep {
            let (tm, te) = full_texel.parts();
            let (ox, oy) = viewport.center.delta_in(&reference.point, Scale::pow2(te));
            (ox.hypot(oy) + half_diag * ss * tm).log2() + te as f64
        } else {
            (dre.hypot(dim) + viewport.scale.to_f64() * key.points_per_pixel * half_diag).log2()
        };
        let ref_point = reference.point.clone();
        let live_capacity = self.config.live_capacity as u32;
        let (rc, rs) = viewport.rotation();
        let rot = [rc as f32, rs as f32];
        let params = |ratio: f64, w: usize, h: usize| {
            let texel = viewport.scale * key.points_per_pixel / ratio;
            let (texel_scale, ref_offset, scale_exp) = if deep {
                let (tm, te) = texel.parts();
                let (ox, oy) = viewport.center.delta_in(&ref_point, Scale::pow2(te));
                (tm as f32, [ox as f32, oy as f32], te as i32)
            } else {
                (texel.to_f64() as f32, [dre as f32, dim as f32], 0)
            };
            IterateParams {
                // Texel (0,0) is the top-left; the kernel flips y so imaginary grows upwards.
                half_size: [w as f32 / 2.0, h as f32 / 2.0],
                ref_offset,
                jitter: [0.5, 0.5],
                texel_scale,
                bailout2: (BAILOUT * BAILOUT) as f32,
                max_iter: key.max_iter as u32,
                ref_len,
                width: w as u32,
                total: (w * h) as u32,
                capacity: live_capacity,
                interior_eps: INTERIOR_EPS,
                soft_iter: key.max_iter as u32,
                scale_exp,
                rot,
                ..IterateParams::default()
            }
        };
        let level_size = |ratio: f64| {
            (
                ((key.width as f64 * ratio).ceil() as usize).max(1),
                ((key.height as f64 * ratio).ceil() as usize).max(1),
            )
        };
        let mut levels: Vec<Level<B>> = Vec::new();
        let full_texels = (tex_w * tex_h) as f64;
        let full_level = |full: &TextureSet<B>| Level::new(full.clone(), ss, false, params(ss, tex_w, tex_h), None);
        match mode {
            JobMode::Interactive => {
                // Dynamic resolution: the largest scale (1/8 steps, 1/4 ... 1) whose measured
                // cost fits the gesture frame budget. Always a preview level (also at scale 1):
                // it is jittered for the temporal upsampling, and the refined frame is computed
                // at the pixel centres afterwards.
                let mut ratio = 0.5;
                if self.cost_per_texel > 0.0 {
                    let fit = (self.gesture_budget() / (self.cost_per_texel * (key.width * key.height) as f64)).sqrt();
                    ratio = ((fit * 8.0).floor() / 8.0).clamp(0.125, 1.0);
                }
                if !self.settings.interactive_preview {
                    ratio = 1.0;
                }
                let (w, h) = level_size(ratio);
                if let Ok(set) = self.preview_texture_set(key.width, key.height, w, h) {
                    self.taa_index = self.taa_index % 64 + 1;
                    let mut p = params(ratio, w, h);
                    p.jitter = [halton(self.taa_index, 2), halton(self.taa_index, 3)];
                    levels.push(Level::new(set, ratio, true, p, None));
                }
            }
            JobMode::Refine => {
                // Preview first when the full level is expected to take more than about a
                // frame; quarter resolution if even half would.
                let full_cost = self.cost_per_texel * full_texels;
                if self.settings.interactive_preview && full_cost > 1.0 / 90.0 {
                    let ratio = if full_cost * 0.25 <= 1.0 / 60.0 { 0.5 } else { 0.25 };
                    let (w, h) = level_size(ratio);
                    if let Ok(set) = self.preview_texture_set(key.width, key.height, w, h) {
                        levels.push(Level::new(set, ratio, true, params(ratio, w, h), None));
                    }
                }
                let mut l = full_level(&full);
                l.order = self.priority_order(&key);
                l.params.tile_order = u32::from(l.order.is_some());
                levels.push(l);
            }
            JobMode::Offscreen => levels.push(full_level(&full)),
        }
        // The full level keeps iterating unresolved pixels past the nominal limit (not
        // gesture frames: they are superseded every refresh).
        let factor = if mode == JobMode::Offscreen {
            self.config.offscreen_extension
        } else {
            self.config.extension_factor
        };
        let cap = self.settings.max_iterations_cap;
        if mode != JobMode::Interactive
            && factor > 1
            && let Some(last) = levels.last_mut()
            && !last.preview
        {
            last.extend_to = (key.max_iter * factor).min(key.max_iter.max(cap));
            last.params.list_capacity = self.list_capacity as u32;
        }
        if levels.is_empty() {
            return;
        }
        let extension_factor = self.config.extension_factor;
        let mut template_params = params(ss, tex_w, tex_h);
        template_params.list_capacity = self.list_capacity as u32;
        let template = FullLevelTemplate {
            set: full,
            ratio: ss,
            params: template_params,
            extend_to: if extension_factor > 1 {
                (key.max_iter * extension_factor).min(key.max_iter.max(cap))
            } else {
                0
            },
        };
        self.generation += 1;
        self.job = Some(Job {
            generation: self.generation,
            key,
            reference_buffer,
            reference_serial,
            bla: None,
            max_dc_log2,
            deep,
            refx_buffer,
            bla_forecast: None,
            levels,
            start: now,
            interactive: mode == JobMode::Interactive,
            presented: false,
            full_level: Some(template),
            gpu_time: 0.0,
            chunks: 0,
            iterations: 0.0,
            steps: 0.0,
            failed: false,
            final_presented: false,
            complete_presented: false,
            complete_at: None,
            aa_composed: 0,
            aa_epoch: 0,
            aa_list: None,
        });
        // Offscreen renders always iterate with the table (deterministic results); views
        // take it when it is there (previews), and full levels wait for it (encode_chunks).
        self.job_bla(mode == JobMode::Offscreen);
    }

    // MARK: BLA table (kb: techniques/bla)

    /// Attaches the BLA table of the job's reference once it is built; `wait` joins a build
    /// still running on its thread.
    fn job_bla(&mut self, wait: bool) -> BlaReady {
        let Some(job) = &self.job else {
            return BlaReady::Without;
        };
        if job.bla.is_some() {
            return BlaReady::Ready;
        }
        // Off, or Auto decided against it for this view.
        if self.config.bla == BlaMode::Off || job.bla_forecast.is_some() {
            return BlaReady::Without;
        }
        let serial = job.reference_serial;
        let building = self.bla.as_ref().is_some_and(|t| t.serial == serial)
            || self.bla_build.as_ref().is_some_and(|(s, _)| *s == serial);
        if !building {
            // Built on demand: no block is valid for a pixel at |dc| >= eps max|2 Z| (a block's
            // R is at most a single step's eps |2 Z|), so shallow views never build one.
            if self.config.bla == BlaMode::Auto && job.max_dc_log2 >= self.bla_reach_log2 {
                self.job.as_mut().expect("job").bla_forecast = Some(BlaForecast::default());
                return BlaReady::Without;
            }
            self.start_bla_build();
        }
        self.finish_bla_build(wait);
        match &self.bla {
            Some(t) if t.serial == serial => {
                let t = t.clone();
                let job = self.job.as_mut().expect("job");
                let forecast = t.table.forecast(job.max_dc_log2, job.key.max_iter);
                job.bla_forecast = Some(forecast);
                if self.config.trace {
                    println!(
                        "BLA forecast: |dc| <= {} covers {} iterations in {} blocks: {}",
                        almondbread_core::cfmt::g(job.max_dc_log2.exp2(), 3),
                        forecast.covered,
                        forecast.blocks,
                        if forecast.pays() { "pays" } else { "does not pay" }
                    );
                }
                let pays = if matches!(t.table, BlaCpu::Deep(_)) {
                    forecast.pays_deep()
                } else {
                    forecast.pays()
                };
                if self.config.bla == BlaMode::Auto && !pays {
                    return BlaReady::Without;
                }
                job.bla = Some(t);
                BlaReady::Ready
            }
            _ if self.bla_build.as_ref().is_some_and(|(s, _)| *s == serial) => BlaReady::Pending,
            _ => {
                // No table (too short a reference, or the build failed): decided for this job.
                self.job.as_mut().expect("job").bla_forecast = Some(BlaForecast::default());
                BlaReady::Without
            }
        }
    }

    /// Collects a finished table build (or waits for it) and uploads it.
    fn finish_bla_build(&mut self, wait: bool) {
        let Some((serial, handle)) = self.bla_build.take() else {
            return;
        };
        if !wait && !handle.is_finished() {
            self.bla_build = Some((serial, handle));
            return;
        }
        match handle.join() {
            Ok(table) if serial == self.reference_serial => self.upload_bla(serial, table),
            Ok(_) => {}
            Err(_) => eprintln!("almondbread: BLA table build failed"),
        }
    }

    fn upload_bla(&mut self, serial: u64, table: BlaCpu) {
        let (build_time, entries, bytes, min_level, max_level, offsets, data): (
            f64,
            usize,
            usize,
            u32,
            u32,
            [u32; 32],
            &[u8],
        ) = match &table {
            BlaCpu::Plain(t) => (
                t.build_time,
                t.entries.len(),
                t.bytes(),
                t.min_level,
                t.max_level,
                t.offsets,
                bytemuck::cast_slice(&t.entries),
            ),
            BlaCpu::Deep(t) => (
                t.build_time,
                t.entries.len(),
                t.bytes(),
                t.min_level,
                t.max_level,
                t.offsets,
                bytemuck::cast_slice(&t.entries),
            ),
        };
        self.bla_info = (build_time, entries, bytes);
        self.bla = None;
        if entries == 0 {
            return;
        }
        match self.backend.create_buffer_with_data(data) {
            Ok(buffer) => {
                self.bla = Some(BlaGpu {
                    serial,
                    buffer,
                    params: BlaParams {
                        min_level,
                        max_level,
                        lookup_every: self.config.bla_lookup_every.next_power_of_two().max(1 << min_level),
                        pad1: 0,
                        offset: offsets,
                    },
                    table,
                });
            }
            Err(e) => eprintln!("almondbread: BLA table buffer: {e}"),
        }
    }

    /// Starts the BLA table of the current reference: inline when short, else on a thread.
    /// Big references get the extended-range table (M3b).
    fn start_bla_build(&mut self) {
        let Some(r) = &self.reference else { return };
        let serial = self.reference_serial;
        let eps = self.config.bla_epsilon;
        let build: Box<dyn FnOnce() -> BlaCpu + Send> = if r.deep_kernel {
            let points = r.extended.clone();
            Box::new(move || BlaCpu::Deep(Arc::new(BlaTableX::build(&points, eps))))
        } else {
            let points = r.points.clone();
            Box::new(move || BlaCpu::Plain(Arc::new(BlaTable::build(&points, eps))))
        };
        if r.points.len() >= self.config.bla_async_from {
            let handle = std::thread::Builder::new().name("almondbread-bla".into()).spawn(build);
            match handle {
                Ok(h) => self.bla_build = Some((serial, h)),
                Err(e) => eprintln!("almondbread: BLA build thread: {e}"),
            }
        } else {
            let table = build();
            self.upload_bla(serial, table);
        }
    }

    /// Build time (s), entries and bytes of the last BLA table.
    pub fn bla_info(&self) -> (f64, usize, usize) {
        self.bla_info
    }

    /// GPU seconds a gesture frame's iteration may take: half the measured refresh interval
    /// (the compose pass and the cost model's error take the rest; 70 % measured 49 fps at
    /// 60 Hz in home zoom). M2's fixed 6 ms assumed 120 Hz, so a 60 Hz display
    /// rendered previews at about half the affordable texels (FL).
    fn gesture_budget(&self) -> f64 {
        if !self.config.adaptive_budget {
            return self.config.interactive_frame_budget;
        }
        (0.5 * self.refresh_interval).clamp(0.004, 0.012)
    }

    /// Admission order for a refinement level (FL, kb: techniques/frame-reuse-and-progressive):
    /// tiles the history does not cover (newly exposed by a pan or zoom-out) first, then by
    /// distance from the zoom target, so what the eye is on sharpens first. None when no
    /// buffer could be made (raster order then).
    fn priority_order(&self, key: &ComputeKey) -> Option<TileOrder<B>> {
        let (tx, ty) = tiles(key.width, key.height);
        let ppp = key.points_per_pixel;
        let (fx, fy) = self.navigator.zoom_target();
        // Focus in pixels from the top-left.
        let focus = (key.width as f64 / 2.0 + fx / ppp, key.height as f64 / 2.0 - fy / ppp);
        // The history's footprint: a pixel of this view (from the top-left) in history pixels
        // (a rotated view turns the footprint).
        let to_history = self.history_view.as_ref().map(|h| {
            let s = key.viewport.scale * ppp;
            let r = s.ratio(h.scale);
            let (dx, dy) = h
                .viewport
                .unrotate_vec(key.viewport.center.delta_in(&h.viewport.center, h.scale));
            let (kv, hv) = (key.viewport.clone(), h.viewport.clone());
            let (w, hgt, hw, hh) = (key.width as f64, key.height as f64, h.width as f64, h.height as f64);
            move |x: f64, y: f64| {
                // Screen offset (y up) in this view -> plane, in history pixels -> history axes.
                let (px, py) = kv.rotate(x - w / 2.0, hgt / 2.0 - y);
                let (ux, uy) = hv.unrotate(px * r, py * r);
                (hw / 2.0 + dx + ux, hh / 2.0 - dy - uy)
            }
        });
        let (hw, hh) = self
            .history_view
            .as_ref()
            .map_or((0.0, 0.0), |h| (h.width as f64, h.height as f64));
        let mut keyed: Vec<(bool, f64, u32)> = (0..tx * ty)
            .map(|t| {
                let (x0, y0) = ((t % tx * 16) as f64, (t / tx * 16) as f64);
                let (x1, y1) = ((x0 + 16.0).min(key.width as f64), (y0 + 16.0).min(key.height as f64));
                let inside = to_history.as_ref().is_some_and(|f| {
                    [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].iter().all(|&(x, y)| {
                        let (u, v) = f(x, y);
                        (0.0..=hw).contains(&u) && (0.0..=hh).contains(&v)
                    })
                });
                let (cx, cy) = (0.5 * (x0 + x1) - focus.0, 0.5 * (y0 + y1) - focus.1);
                (inside, cx * cx + cy * cy, t as u32)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let order: Vec<u32> = keyed.iter().map(|k| k.2).collect();
        let mut rank = vec![0u32; order.len()];
        for (r, &t) in order.iter().enumerate() {
            rank[t as usize] = r as u32;
        }
        let order = self
            .backend
            .create_buffer_with_data(bytemuck::cast_slice(&order))
            .ok()?;
        let rank = self.backend.create_buffer_with_data(bytemuck::cast_slice(&rank)).ok()?;
        Some(TileOrder { order, rank })
    }

    /// Turns a finished gesture frame into a refinement job: same view, the full level added
    /// after the preview, which stays visible underneath while the full level fills in.
    fn extend_for_refinement(&mut self) {
        let order = match &self.job {
            Some(j) if !j.levels.iter().any(|l| !l.preview) => self.priority_order(&j.key),
            _ => None,
        };
        let Some(job) = &mut self.job else { return };
        job.interactive = false;
        if !job.levels.iter().any(|l| !l.preview)
            && let Some(t) = &job.full_level
        {
            let mut l = Level::new(t.set.clone(), t.ratio, false, t.params, None);
            l.extend_to = t.extend_to;
            l.params.tile_order = u32::from(order.is_some());
            l.order = order;
            job.levels.push(l);
            job.final_presented = false;
        }
    }

    // MARK: Motion sample cache (kb: techniques/sample-reuse)

    /// The regime new cache samples of the current job would be computed in, whether its BLA
    /// table is still building, and the table.
    fn cache_regime(&self) -> Option<(Regime, bool, Option<BlaGpu<B>>)> {
        let job = self.job.as_ref()?;
        let key = &job.key;
        let pinned = self.sample_cache.regime();
        let table = self
            .bla
            .clone()
            .filter(|t| t.serial == job.reference_serial && self.reference_serial == job.reference_serial);
        let building = self.bla_build.as_ref().is_some_and(|(s, _)| *s == job.reference_serial);
        // BLA as pinned by the epoch, or (a new epoch, or one that started before the table
        // was built) as the job decided: an epoch moves to BLA once, never back while its table
        // exists.
        let want = job.bla.is_some() || pinned.is_some_and(|r| r.bla);
        let (bla, pending) = match (want, table.is_some(), building) {
            (false, _, _) => (false, false),
            (true, true, _) => (true, false),
            (true, false, true) => (true, true),
            (true, false, false) => (false, false),
        };
        // The interior test whenever it is enabled: cached samples live for many frames, and an
        // interior pixel without the test runs to the limit (a 1e-305 needle: 200 000
        // iterations per interior texel, 0.5 s chunks). The statistics-driven switch is for
        // fresh levels.
        let interior = key.interior;
        let regime = Regime {
            deep: job.deep,
            bla,
            interior,
            de: key.de,
            interior_color: key.interior_color && self.ic_buffers.is_some(),
        };
        Some((regime, pending, if bla { table } else { None }))
    }

    /// Replaces a fresh gesture job's preview level by a level of the sample cache: the
    /// cache's window, computing only the tiles the view lacks within the frame budget. Keeps
    /// the fresh preview when the cache cannot serve the view.
    /// The current gesture job's sample cache level if its batch is unfinished (it continues in
    /// the next frame's job), with the job's generation.
    fn take_cache_level(&mut self) -> Option<(Level<B>, u64)> {
        let job = self.job.as_mut()?;
        let l = job.levels.first()?;
        if !job.interactive || l.cache.is_none() || l.done || !l.started() {
            return None;
        }
        Some((job.levels.remove(0), job.generation))
    }

    fn attach_cache_level(&mut self, carried: Option<(Level<B>, u64)>) {
        if !self.sample_cache.config.enabled || !self.settings.interactive_preview {
            return;
        }
        let Some(job) = &self.job else { return };
        if !job.interactive || job.levels.len() != 1 || !job.levels[0].preview || job.levels[0].started() {
            return;
        }
        let Some((regime, bla_pending, table)) = self.cache_regime() else {
            return;
        };
        let job = self.job.as_ref().expect("job");
        let key = job.key.clone();
        let generation = job.generation;
        let template = job.levels[0].params;
        let mut live: Vec<u64> = self.in_flight.iter().map(|r| r.generation).collect();
        live.dedup();
        let (fx, fy) = self.navigator.zoom_target();
        let ppp = key.points_per_pixel;
        let input = PlanInput {
            view: &key.viewport,
            ppp,
            width: key.width,
            height: key.height,
            regime,
            bla_pending,
            limit: key.max_iter,
            budget: self.gesture_budget(),
            cost_hint: self.cost_per_texel,
            focus: (fx / ppp, -fy / ppp),
            generation,
            live_capacity: self.config.live_capacity,
            live_generations: &live,
            want_aux: regime.interior_color,
            carry: carried.as_ref().map(|c| c.1),
        };
        let Some(plan) = self.sample_cache.plan(&self.backend, &input) else {
            return;
        };
        if plan.carried
            && let Some((l, old)) = carried
        {
            // The unfinished batch goes on: its chunks on the GPU now count for this job.
            for r in &mut self.in_flight {
                if r.generation == old && r.level == 0 {
                    r.generation = generation;
                }
            }
            self.job.as_mut().expect("job").levels[0] = l;
            return;
        }
        if self.config.trace {
            println!(
                "sample cache level: gen {generation} {} tiles, ratio {:.3}, regime {regime:?}",
                plan.tiles, plan.ratio
            );
        }
        let Some(reference) = &self.reference else { return };
        let f = &plan.frame;
        let mut p = template;
        if regime.deep {
            let (tm, te) = f.h.parts();
            let (ox, oy) = f.anchor.delta_in(&reference.point, Scale::pow2(te));
            p.texel_scale = tm as f32;
            p.ref_offset = [ox as f32, oy as f32];
            p.scale_exp = te as i32;
        } else {
            let (dre, dim) = if reference.big {
                f.anchor.delta_in(&reference.point, Scale::from_f64(1.0))
            } else {
                f.anchor.delta_from(&reference.point)
            };
            p.texel_scale = f.h.to_f64() as f32;
            p.ref_offset = [dre as f32, dim as f32];
            p.scale_exp = 0;
        }
        // Sample position of window texel t: lattice index origin + t (jitter 0).
        p.half_size = [-(f.origin.0 as f32), -(f.origin.1 as f32)];
        p.jitter = [0.0, 0.0];
        p.rot = [f.rot.0 as f32, f.rot.1 as f32];
        p.width = plan.width as u32;
        p.wrap = f.wrap;
        p.list_mode = 0;
        let set = TextureSet {
            iter: plan.set.iter,
            de: plan.set.de,
            color: Some(plan.color),
            aux: plan.set.aux,
            width: plan.width,
            height: plan.height,
        };
        let mut l = Level::new(set, plan.ratio, true, p, Some(plan.tiles * 256));
        l.params.tile_order = 1;
        l.order = plan.order.map(|order| TileOrder {
            order,
            rank: self.dummy_buffer.clone(),
        });
        l.cache = Some(CacheLevel {
            regime,
            bla: table,
            predicted: plan.predicted,
        });
        l.use_bla = regime.bla;
        l.use_interior = regime.interior;
        let empty = plan.tiles == 0;
        if empty {
            l.done = true;
            l.complete = true;
        }
        self.job.as_mut().expect("job").levels[0] = l;
        if empty {
            self.interior_stats = self.cache_interior_stats();
            self.survey_level(0);
        }
    }

    /// The interior-test statistics after a sample cache frame. Its new tiles are a biased
    /// sample of the view, so the counts come from the whole cached view (its shade pass);
    /// those may only turn the test on: cached samples carry no record of what was unresolved
    /// at the view's limit before an extension pass resolved them, and the test off draws
    /// such pixels interior (the refined frame then differed from a fresh render by 0.1 %).
    fn cache_interior_stats(&self) -> Option<InteriorStats> {
        let max_iter = self.job.as_ref().map_or(0, |j| j.key.max_iter);
        match (self.sample_cache.view_stats(), self.interior_stats) {
            (Some(v), Some(old))
                if autoiter::use_interior_test(true, Some(v), max_iter)
                    && !autoiter::use_interior_test(true, Some(old), max_iter) =>
            {
                Some(v)
            }
            (Some(_), old) => old,
            (None, old) => old,
        }
    }

    /// Seeds the sample cache from the refined frame once a view's full level (and its
    /// extension passes) are done: a gesture starting here reuses every sample.
    fn seed_sample_cache(&mut self) {
        if !self.sample_cache.config.enabled || !self.settings.interactive_preview {
            return;
        }
        let Some(job) = &self.job else { return };
        if job.interactive || job.failed || self.sample_cache.seeded_generation == job.generation {
            return;
        }
        let Some(full) = job
            .levels
            .iter()
            .find(|l| !l.preview && l.aa.is_none() && !l.is_extension())
        else {
            return;
        };
        if !job.levels.iter().filter(|l| l.aa.is_none()).all(|l| l.done)
            || full.set.width != job.key.width
            || full.set.height != job.key.height
        {
            return;
        }
        // Tiles whose pixels started from local SA seeds (M5) are not exact samples: the cache
        // leaves them out (their 16 x 16 tiles of the full level, from the seed pass's flags).
        let mut seeded_tiles: Vec<bool> = Vec::new();
        if let (Some(leaf), Some((buf, n))) = (full.seeded, &self.seed_tiles) {
            let (tx, ty) = tiles(full.set.width, full.set.height);
            if leaf.tiles_x as usize != tx || *n < tx * ty {
                return;
            }
            let mut bytes = vec![0u8; tx * ty * 4];
            self.backend.read_buffer(buf, 0, &mut bytes);
            seeded_tiles = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c) != 0)
                .collect();
            // Mostly seeded: the gesture's own samples serve better than what is left.
            if seeded_tiles.iter().filter(|&&t| t).count() * 4 > seeded_tiles.len() {
                self.sample_cache.seeded_generation = job.generation;
                return;
            }
        }
        let key = &job.key;
        let regime = Regime {
            deep: job.deep,
            bla: full.use_bla,
            interior: full.use_interior,
            de: key.de,
            interior_color: key.interior_color && self.ic_buffers.is_some() && full.set.aux.is_some(),
        };
        let set = full.set.clone();
        let (view, ppp, limit, generation) = (key.viewport.clone(), key.points_per_pixel, key.max_iter, job.generation);
        self.sample_cache.seed(
            &self.backend,
            (&set.iter, &set.de, set.aux.as_ref(), set.width, set.height),
            &view,
            ppp,
            regime,
            limit,
            generation,
            &seeded_tiles,
        );
    }

    /// Builds the adaptive AA list once the refined view is shown and everything but AA is
    /// done (one small GPU pass), and reads its length when the pass has finished.
    fn prepare_aa_list(&mut self, target: u32) {
        let Some(job) = &self.job else { return };
        if let Some(l) = &job.aa_list {
            if l.len.is_none() && l.submission.status() != SubmissionStatus::Pending {
                let mut bytes = [0u8; 4];
                self.backend.read_buffer(&l.count_buffer, 0, &mut bytes);
                let cap = self.aa_list_buffer.as_ref().map_or(0, |b| b.1);
                let len = (u32::from_le_bytes(bytes) as usize).min(cap);
                self.job.as_mut().expect("job").aa_list.as_mut().expect("list").len = Some(len);
            }
            return;
        }
        if target <= 1
            || job.interactive
            || !job.complete_presented
            || job.failed
            || !job.levels.iter().all(|l| l.done || l.aa.is_some())
        {
            return;
        }
        let Some(full) = job.levels.iter().find(|l| !l.preview && l.aa.is_none()) else {
            return;
        };
        let (w, h) = (full.set.width, full.set.height);
        let (iter, de) = (full.set.iter.clone(), full.set.de.clone());
        let order = full.order.clone();
        let (tx, ty) = tiles(w, h);
        let Ok(aa_set) = self.preview_texture_set(w, h, w, h) else {
            return;
        };
        if self.aa_list_buffer.as_ref().is_none_or(|b| b.1 < w * h) {
            match self.backend.create_buffer(w * h * 4, Storage::Private) {
                Ok(b) => self.aa_list_buffer = Some((b, w * h)),
                Err(_) => return,
            }
        }
        let (list, cap) = self.aa_list_buffer.clone().expect("list");
        let Ok(count_buffer) = self.backend.create_buffer_with_data(&[0u8; 4]) else {
            return;
        };
        let Ok(mut enc) = self.backend.begin("aa list") else {
            return;
        };
        let p = AaListParams {
            width: w as u32,
            height: h as u32,
            tiles_x: tx as u32,
            tile_order: u32::from(order.is_some()),
            total: (tx * ty * 256) as u32,
            capacity: cap as u32,
            de_threshold: 4.0,
            pad: 0,
        };
        enc.set_pipeline(&self.aa_list_pipeline);
        enc.set_texture(0, &iter);
        enc.set_texture(1, &de);
        enc.set_texture(2, &aa_set.iter);
        enc.set_bytes(0, bytes_of(&p));
        enc.set_buffer(1, order.as_ref().map_or(&self.dummy_buffer, |o| &o.order), 0);
        enc.set_buffer(2, &count_buffer, 0);
        enc.set_buffer(3, &list, 0);
        enc.dispatch_threads([tx * ty * 256, 1, 1], [ITERATE_THREADS, 1, 1]);
        let submission = enc.commit();
        self.job.as_mut().expect("job").aa_list = Some(AaList {
            count_buffer,
            submission,
            len: None,
        });
    }

    /// Queues the next idle anti-aliasing sample once everything else of the refined view is
    /// done (kb: techniques/anti-aliasing, FL): the pixels on the adaptive list again with a
    /// Halton jitter, into the preview allocation (full size; previews are not needed while
    /// idle), at the nominal limit. Input replaces the job, which drops the samples at once.
    fn queue_aa_sample(&mut self, target: u32) {
        let Some(job) = &self.job else { return };
        let Some(len) = job.aa_list.as_ref().and_then(|l| l.len) else {
            return;
        };
        if len == 0
            || job.interactive
            || !job.done()
            || job.aa_pending()
            || job.failed
            || job.aa_finished(target, self.config.aa_budget)
        {
            return;
        }
        let next = job.aa_composed + 1;
        if next >= target || job.levels.iter().any(|l| l.aa == Some(next)) {
            return;
        }
        let Some(full_index) = job.levels.iter().position(|l| !l.preview && l.aa.is_none()) else {
            return;
        };
        let (w, h) = (job.key.width, job.key.height);
        let Ok(set) = self.preview_texture_set(w, h, w, h) else {
            return;
        };
        let list = self.aa_list_buffer.as_ref().map(|b| b.0.clone());
        let job = self.job.as_mut().expect("job");
        let full = &job.levels[full_index];
        let mut p = full.params;
        p.jitter = [halton(next, 2), halton(next, 3)];
        // Admit the listed texels; collect nothing.
        p.list_mode = 2;
        p.list_capacity = 0;
        p.fill_border = 0;
        p.max_iter = full.params.soft_iter;
        let mut l = Level::new(set, 1.0, false, p, Some(len));
        l.list_buffer = list;
        l.aa = Some(next);
        l.complete = true;
        l.use_interior = full.use_interior;
        // (Samples branch from the full level's leaves at their own jitter.)
        l.seeded = full.seeded;
        job.levels.push(l);
    }

    /// A gesture frame: the preview level in ceil(texels / capacity) chunks with K = maxIter,
    /// so it is complete when the frame's colorize runs. If the measured cost says that would
    /// take long, fall back to budgeted progressive chunks.
    fn encode_interactive(&mut self) {
        let Some(job) = &self.job else { return };
        let Some(level) = job.levels.first() else { return };
        // (A sample cache level with nothing to compute is done at once.)
        if level.done {
            return;
        }
        if level.encoded != 0 {
            if !level.done {
                let b = self.config.interactive_chunk_target;
                self.encode_chunks(b);
            }
            return;
        }
        let estimate = self.cost_per_texel * level.total as f64;
        // A sample cache batch runs at once when the planner expects it to fit the frame, else
        // in budgeted chunks that carry over into the next frames' jobs.
        let progressive = match &level.cache {
            Some(c) => c.predicted > 1.5 * self.gesture_budget(),
            None => estimate > 0.05,
        };
        if progressive {
            let b = self.config.interactive_chunk_target;
            self.encode_chunks(b);
            return;
        }
        let chunks = level.total.div_ceil(self.config.live_capacity);
        let k = level.params.max_iter as usize;
        for _ in 0..chunks {
            if !self.encode_chunk(0, k, false) {
                break;
            }
        }
    }

    /// Encodes chunk command buffers until about `budget` seconds of estimated GPU work are
    /// queued or enough chunks are outstanding. Returns the number encoded.
    fn encode_chunks(&mut self, budget: f64) -> usize {
        let mut spent = 0.0;
        let mut encoded = 0;
        while let Some(job) = &self.job {
            let generation = job.generation;
            // Idle AA samples keep two small chunks queued: input waits for at most ~16 ms of
            // GPU (FL: AA is interruptible).
            let aa = job.current().is_some_and(|i| job.levels[i].aa.is_some());
            let max = if aa { 2 } else { self.config.max_chunks_in_flight };
            if !(encoded < 8 && spent < budget)
                || job.failed
                || self.in_flight.iter().filter(|r| r.generation == generation).count() >= max
                || self.in_flight.len() >= 2 * max
            {
                break;
            }
            let Some(i) = job.current() else { break };
            // A level must be known finished before the next starts: they share the live lists.
            if i > 0 && !job.levels[i - 1].done {
                break;
            }
            // A level decides on BLA when it starts: previews take the table if it is built,
            // full levels wait for it (deterministic results; the build runs on its thread).
            if !job.levels[i].started() && job.bla.is_none() {
                let preview = job.levels[i].preview;
                if self.job_bla(false) == BlaReady::Pending && !preview {
                    break;
                }
            }
            // M5: a seeded level's later seed rounds come before its chunks.
            if !self.advance_seeding(i) {
                break;
            }
            let Some(job) = &self.job else { break };
            let level = &job.levels[i];
            // Upper bound on the threads of this chunk, from the last harvested counts.
            let threads = self.config.live_capacity.min(level.live + level.total - level.admitted);
            // K from the throughput model: work <= threads x K lane-iterations.
            let target = (budget - spent).max(0.002);
            let k = (level.params.max_iter as f64).min((target * self.rate / threads.max(1) as f64).max(16.0)) as usize;
            if !self.encode_chunk(i, k, true) {
                break;
            }
            spent += threads as f64 * k as f64 / self.rate;
            encoded += 1;
        }
        encoded
    }

    /// Encodes and commits one chunk (its own command buffer: the watchdog counts per command
    /// buffer, and its GPU time measures this chunk alone).
    fn encode_chunk(&mut self, level_index: usize, k: usize, adaptive: bool) -> bool {
        let interior_enabled = self.job.as_ref().is_some_and(|j| j.key.interior);
        let (de, key_max_iter) = match &self.job {
            Some(j) => (j.key.de, j.key.max_iter),
            None => return false,
        };
        let stats = self.interior_stats;
        {
            let job = self.job.as_mut().expect("job");
            let has_bla = job.bla.is_some();
            let level = &mut job.levels[level_index];
            if let Some(c) = &level.cache {
                // The cache epoch's pinned regime (samples of one epoch never mix regimes).
                level.use_bla = c.regime.bla;
                level.use_interior = c.regime.interior;
            } else if !level.started() {
                level.use_bla = has_bla;
            }
            if !level.started() && !level.is_extension() && level.cache.is_none() {
                level.use_interior = autoiter::use_interior_test(interior_enabled, stats, key_max_iter);
                // Collect unresolved pixels for extension passes.
                if level.use_interior && level.extend_to > level.params.soft_iter as usize {
                    level.params.list_mode = 1;
                }
            }
        }

        let (use_interior, use_bla, deep, interior_color, seed_now) = {
            let j = self.job.as_ref().expect("job");
            let l = &j.levels[level_index];
            let ic = j.key.interior_color && l.set.aux.is_some() && self.ic_buffers.is_some();
            let seed_now = self.config.local_sa.mode != SaMode::Off
                && l.aa.is_none()
                && !l.started()
                && !l.is_extension()
                && l.seeding.is_none()
                // (Sample cache levels compute scattered toroidal tiles, exact samples only.)
                && l.cache.is_none();
            (l.use_interior, l.use_bla, j.deep, ic, seed_now)
        };
        if seed_now {
            // A chunk with K = the whole limit (gesture frames) gets seeds without a budget, so
            // the frame completes; otherwise rounds of the throughput model's size.
            let unbounded = k >= self.job.as_ref().expect("job").levels[level_index].params.max_iter as usize;
            if let Err(e) = self.start_seeding(level_index, use_interior, interior_color, unbounded) {
                eprintln!("almondbread: seed passes failed: {e}");
            }
        }
        // (A fill candidate waits for its seeds' tile count before its first chunk.)
        if seed_now && self.config.interior_fill == SaMode::Auto && self.fill_wanted(level_index) {
            return false;
        }
        self.decide_fill(level_index);
        let (seeded, seed_gate) = {
            let l = &self.job.as_ref().expect("job").levels[level_index];
            (l.seeded, l.seeding.is_some())
        };
        let pipeline = match self.iterate_pipeline(de, use_interior, use_bla, deep, interior_color, seeded.is_some()) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("almondbread: iterate pipeline failed: {e}");
                if let Some(j) = &mut self.job {
                    j.failed = true;
                }
                return false;
            }
        };
        let Ok(mut enc) = self.backend.begin("iterate chunk") else {
            return false;
        };
        let job = self.job.as_ref().expect("job");
        let level = &job.levels[level_index];
        let mut p = level.params;
        p.chunk_iters = k as u32;
        p.stat_slot = self.stat_slot as u32;
        p.first_chunk = u32::from(!level.started());
        p.seed_gate = u32::from(seed_gate);
        let tg = ITERATE_THREADS as u32;
        let (src, dst) = (self.ping, 1 - self.ping);

        enc.set_pipeline(&self.prepare_pipeline);
        enc.set_bytes(0, bytes_of(&p));
        enc.set_buffer(4, &self.count_buffers[src], 0);
        enc.set_buffer(5, &self.count_buffers[dst], 0);
        enc.set_buffer(6, &self.stats_buffer, 0);
        enc.set_buffer(7, &self.args_buffer, 0);
        enc.set_bytes(8, bytes_of(&tg));
        enc.set_buffer(9, &self.level_buffer, 0);
        // (No lists allocated: listCapacity is 0 and nothing is collected; bind a placeholder.)
        match (&level.list_buffer, &self.list_buffers) {
            (Some(list), _) => {
                enc.set_buffer(10, list, 0);
                enc.set_buffer(12, list, 0);
            }
            (None, Some(lists)) => {
                enc.set_buffer(10, &lists[level.list_in], 0);
                enc.set_buffer(12, &lists[level.list_out], 0);
            }
            (None, None) => {
                enc.set_buffer(10, &self.list_count_buffer, 0);
                enc.set_buffer(12, &self.list_count_buffer, 0);
            }
        }
        enc.set_buffer(11, &self.list_count_buffer, 0);
        enc.set_buffer(13, level.order.as_ref().map_or(&self.dummy_buffer, |o| &o.order), 0);
        enc.set_buffer(20, &self.seed_status, 0);
        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);

        enc.set_pipeline(&pipeline);
        enc.set_texture(0, &level.set.iter);
        enc.set_texture(1, &level.set.de);
        enc.set_buffer(1, &job.reference_buffer, 0);
        enc.set_buffer(2, &self.state_buffers[src], 0);
        enc.set_buffer(3, &self.state_buffers[dst], 0);
        if interior_color && let (Some(ic), Some(aux)) = (&self.ic_buffers, &level.set.aux) {
            enc.set_texture(2, aux);
            enc.set_buffer(18, &ic[src], 0);
            enc.set_buffer(19, &ic[dst], 0);
        }
        let table = level.cache.as_ref().map_or(job.bla.as_ref(), |c| c.bla.as_ref());
        match (table, use_bla) {
            (Some(b), true) => {
                enc.set_bytes(14, bytes_of(&b.params));
                // Plain tables at 15, extended-range (deep) tables at 17.
                enc.set_buffer(15, if deep { &self.dummy_buffer } else { &b.buffer }, 0);
                enc.set_buffer(17, if deep { &b.buffer } else { &self.dummy_buffer }, 0);
            }
            _ => {
                enc.set_bytes(14, bytes_of(&BlaParams::default()));
                enc.set_buffer(15, &self.dummy_buffer, 0);
                enc.set_buffer(17, &self.dummy_buffer, 0);
            }
        }
        enc.set_buffer(16, job.refx_buffer.as_ref().unwrap_or(&self.dummy_buffer), 0);
        enc.set_buffer(23, self.tile_bad.as_ref().map_or(&self.dummy_buffer, |t| &t.0), 0);
        enc.set_buffer(24, &self.level_buffer, 0);
        if let (Some((nodes, _)), Some((tiles, _)), Some(leaf)) = (&self.seed_nodes, &self.seed_tiles, seeded) {
            enc.set_buffer(20, nodes, 0);
            enc.set_bytes(21, bytes_of(&leaf));
            enc.set_buffer(22, tiles, 0);
        }
        enc.dispatch_indirect(&self.args_buffer, 0, [ITERATE_THREADS, 1, 1]);

        enc.set_pipeline(&self.finish_pipeline);
        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
        let submission = enc.commit();

        let generation = job.generation;
        let aa_epoch = job.levels[level_index].aa.map(|_| job.aa_epoch);
        self.ping = dst;
        self.job.as_mut().expect("job").levels[level_index].encoded += 1;
        self.in_flight.push_back(ChunkRecord {
            submission,
            generation,
            level: level_index,
            aa_epoch,
            slot: self.stat_slot,
            iterations: k,
            adaptive,
            preview: self.job.as_ref().is_some_and(|j| j.levels[level_index].preview),
            cache: self.job.as_ref().is_some_and(|j| j.levels[level_index].cache.is_some()),
        });
        self.stat_slot = (self.stat_slot + 1) % STAT_SLOTS;
        true
    }

    fn read_stats(&self, slot: usize) -> ChunkStats {
        let mut bytes = [0u8; std::mem::size_of::<ChunkStats>()];
        self.backend
            .read_buffer(&self.stats_buffer, slot * std::mem::size_of::<ChunkStats>(), &mut bytes);
        bytemuck::pod_read_unaligned(&bytes)
    }

    /// Processes completed chunks in commit order: throughput, live counts, level completion.
    fn harvest(&mut self) {
        self.harvest_seed_round(false);
        while let Some(front) = self.in_flight.front() {
            let status = front.submission.status();
            if status == SubmissionStatus::Pending {
                break;
            }
            let rec = self.in_flight.pop_front().expect("record");
            let gpu = rec.submission.gpu_time();
            if let SubmissionStatus::Failed(err) = status {
                // Most likely a watchdog timeout: assume the GPU is much slower than measured
                // and restart the job with smaller chunks (kb: platform/gpu-watchdog-and-chunking).
                eprintln!("almondbread: iterate chunk failed ({err}); retrying with smaller chunks");
                self.rate /= 8.0;
                if let Some(j) = &mut self.job
                    && j.generation == rec.generation
                {
                    j.failed = true;
                }
                continue;
            }
            let stats = self.read_stats(rec.slot);
            let work = (stats.live_in as f64 + stats.fresh as f64) * rec.iterations as f64;
            if rec.adaptive && gpu > 2e-4 && work > 0.0 {
                // Damped: at most x4 per chunk, so one odd chunk cannot swing K wildly.
                self.rate = (work / gpu).max(self.rate / 4.0).min(self.rate * 4.0);
            }
            let pixels = stats.live_in as f64 + stats.fresh as f64;
            if !rec.adaptive && gpu > 2e-4 && pixels > 0.0 && !rec.cache {
                // A gesture frame's chunk ran its pixels to the limit: their cost per texel,
                // also when the job was superseded before it finished (every frame of a
                // gesture is). Without this, a refined frame made cheap by local SA left the
                // dynamic resolution too fine for plain previews: 166 ms gesture frames at the
                // spiral (M5).
                self.cost_per_texel = gpu / pixels;
            }
            if self.config.trace {
                let use_interior = self
                    .job
                    .as_ref()
                    .filter(|j| j.generation == rec.generation)
                    .and_then(|j| j.levels.get(rec.level))
                    .map(|l| l.use_interior)
                    .unwrap_or(false);
                println!(
                    "chunk gen {} slot {:2}: liveIn {:8} fresh {:8} K {:8} -> liveOut {:8}  iters {}  steps {}  interior {} (test {})  gpu {:.2} ms  rate {}",
                    rec.generation,
                    rec.slot,
                    stats.live_in,
                    stats.fresh,
                    rec.iterations,
                    stats.live_out,
                    almondbread_core::cfmt::g(stats.iterations() as f64, 3),
                    almondbread_core::cfmt::g(stats.steps() as f64, 3),
                    stats.interior,
                    if use_interior { "on" } else { "off" },
                    gpu * 1000.0,
                    almondbread_core::cfmt::g(self.rate, 3)
                );
            }
            if let Some(fs) = &mut self.frame_stats {
                fs.chunk_gpu += gpu;
                fs.iterations += stats.iterations() as f64;
                if rec.preview {
                    fs.preview_iterations += stats.iterations() as f64;
                    fs.preview_chunk_gpu += gpu;
                }
            }
            // The sample cache's batches complete even after their job was superseded.
            if rec.level == 0 {
                self.sample_cache.on_chunk(
                    rec.generation,
                    stats.admitted as usize,
                    stats.live_out as usize,
                    gpu,
                    stats.iterations() as f64,
                );
            }
            let Some(job) = &mut self.job else { continue };
            // Another job's chunk, or one of AA samples dropped since (a colour change).
            if job.generation != rec.generation
                || rec.aa_epoch.is_some_and(|e| e != job.aa_epoch)
                || rec.level >= job.levels.len()
            {
                continue;
            }
            job.gpu_time += gpu;
            job.chunks += 1;
            job.iterations += stats.iterations() as f64;
            job.steps += stats.steps() as f64;
            // (Defensive: a record never outlives its level within one generation.)
            let Some(level) = job.levels.get_mut(rec.level) else {
                continue;
            };
            level.gpu_time += gpu;
            level.iterations += stats.iterations() as f64;
            level.steps += stats.steps() as f64;
            level.interior_pixels += stats.interior as usize;
            level.admitted = stats.admitted as usize;
            level.live = stats.live_out as usize;
            level.crossed += stats.crossed as usize;
            level.max_escape = level.max_escape.max(stats.max_escape as usize);
            for k in 0..4 {
                level.escaped_past[k] += stats.escaped_past[k] as usize;
            }
            if level.live == 0 && level.admitted == level.total {
                self.finish_level(rec.level);
            }
        }
    }

    /// The level's frame is complete: cost and interior statistics for the next decisions.
    fn complete_level(&mut self, index: usize) {
        let Some(job) = &mut self.job else { return };
        let level = &mut job.levels[index];
        if level.complete {
            return;
        }
        level.complete = true;
        // (A sample cache level computes the tiles a frame lacks, no sample of the view's cost:
        // the cost per texel stays the fresh previews' and refined levels', which a fresh
        // preview is sized by.)
        let texels = level.set.width * level.set.height;
        if level.gpu_time > 0.0 && !level.is_extension() && texels > 0 && level.cache.is_none() {
            self.cost_per_texel = level.gpu_time / texels as f64;
        }
        // (A filled level skipped its insides: keep the last unfilled estimate, or the SA gate
        // would seed the next level, which then may not fill: elephant alternated 23 / 33 ms.)
        if !level.is_extension()
            && level.aa.is_none()
            && level.total > 0
            && level.params.fill_border == 0
            && level.cache.is_none()
        {
            let skipped = level.seeding.as_ref().map_or(0.0, |s| s.path_sum as f64 * 256.0);
            self.steps_per_texel = Some((level.steps + skipped) / level.total as f64);
        }
        self.interior_stats = if level.cache.is_some() {
            self.cache_interior_stats()
        } else {
            Some(InteriorStats::from_level(
                level.total,
                level.interior_pixels,
                level.iterations,
                level.use_interior,
                level.params.max_iter as usize,
                level.crossed,
            ))
        };
        self.survey_level(index);
    }

    /// Autopilot: surveys a just-completed level (at most every [`SURVEY_INTERVAL`]). The pass
    /// is committed now, before anything else is encoded this refresh, so it reads the level
    /// as finished (the preview allocation is reused by the next gesture job).
    fn survey_level(&mut self, index: usize) {
        if !self.navigator.autopilot().is_active()
            || self.pending_survey.is_some()
            || self.last_frame_time - self.last_survey < SURVEY_INTERVAL
        {
            return;
        }
        let Some(job) = &self.job else { return };
        let level = &job.levels[index];
        if level.aa.is_some() || level.is_extension() {
            return;
        }
        let (mut w, mut h) = (level.set.width, level.set.height);
        let (mut origin, mut wrap, mut tex) = ([0u32; 2], 0u32, w as u32 | ((h as u32) << 16));
        if level.cache.is_some() {
            // The view's rectangle of the sample cache's window.
            let k = &job.key;
            let Some((o, size, wr, t)) =
                self.sample_cache
                    .survey_rect(&k.viewport, k.points_per_pixel, k.width, k.height)
            else {
                return;
            };
            (w, h, origin, wrap, tex) = (size[0] as usize, size[1] as usize, o, wr, t);
        }
        let iter = level.set.iter.clone();
        let view = (
            job.key.width as f64 * job.key.points_per_pixel,
            job.key.height as f64 * job.key.points_per_pixel,
        );
        let Ok(mut enc) = self.backend.begin("autopilot survey") else {
            return;
        };
        let p = SurveyParams {
            width: w as u32,
            height: h as u32,
            cols: SURVEY_COLS as u32,
            rows: SURVEY_ROWS as u32,
            origin,
            wrap,
            tex,
        };
        enc.set_pipeline(&self.survey_pipeline);
        enc.set_texture(0, &iter);
        enc.set_bytes(0, bytes_of(&p));
        enc.set_buffer(1, &self.survey_buffer, 0);
        enc.dispatch_threads([SURVEY_COLS, SURVEY_ROWS, 1], [SURVEY_COLS, SURVEY_ROWS, 1]);
        let submission = enc.commit();
        self.last_survey = self.last_frame_time;
        let survey = Survey {
            cols: SURVEY_COLS,
            rows: SURVEY_ROWS,
            cells: Vec::new(),
            view,
            texels_across: w as f64,
            time: self.last_frame_time,
        };
        self.pending_survey = Some((submission, survey));
    }

    /// Hands a finished survey to the autopilot.
    fn poll_survey(&mut self, now: f64) {
        let Some((sub, _)) = &self.pending_survey else { return };
        match sub.status() {
            SubmissionStatus::Pending => {}
            SubmissionStatus::Failed(_) => self.pending_survey = None,
            SubmissionStatus::Completed => {
                let (_, mut survey) = self.pending_survey.take().expect("survey");
                let mut bytes = vec![0u8; SURVEY_COLS * SURVEY_ROWS * 16];
                self.backend.read_buffer(&self.survey_buffer, 0, &mut bytes);
                survey.cells = bytes
                    .as_chunks::<16>()
                    .0
                    .iter()
                    .map(|c| {
                        let u = |k: usize| u32::from_le_bytes([c[4 * k], c[4 * k + 1], c[4 * k + 2], c[4 * k + 3]]);
                        SurveyCell {
                            samples: u(0),
                            interior: u(1),
                            pairs: u(2),
                            edges: u(3),
                        }
                    })
                    .collect();
                survey.time = survey.time.min(now);
                self.navigator.autopilot_mut().set_survey(survey);
            }
        }
    }

    fn finish_level(&mut self, index: usize) {
        if self.job.as_ref().is_none_or(|j| j.levels[index].done) {
            return;
        }
        self.complete_level(index);
        let rate = self.rate;
        let budget = self.config.extension_budget;
        let list_capacity = self.list_capacity;
        let trace = self.config.trace;
        let job = self.job.as_mut().expect("job");
        let is_last = index + 1 == job.levels.len();
        let spent: f64 = job.levels.iter().filter(|l| l.is_extension()).map(|l| l.gpu_time).sum();
        let level = &mut job.levels[index];
        level.done = true;
        if level.preview || level.aa.is_some() {
            return;
        }
        if trace {
            println!(
                "finish level: mode {} maxIter {} total {} crossed {} gpu {}",
                level.params.list_mode, level.params.max_iter, level.total, level.crossed, level.gpu_time
            );
        }
        if level.params.list_mode != 0
            && level.crossed > 0
            && level.crossed <= list_capacity
            && (level.params.max_iter as usize) < level.extend_to
            && is_last
        {
            // Extension: continue the unresolved pixels in passes of x4 (or x2) the limit,
            // each from the start; a pass runs only if its estimated cost fits the budget and
            // is never cut short (a scanline-ordered list cut short leaves a straight edge).
            let count = level.crossed;
            let fits = |n: usize| spent + count as f64 * n as f64 / rate <= budget;
            let mut next = level.extend_to.min(level.params.max_iter as usize * 4);
            if !fits(next) {
                next = level.extend_to.min(level.params.max_iter as usize * 2);
            }
            if fits(next) {
                let mut p = level.params;
                p.max_iter = next as u32;
                p.list_mode = 2;
                p.total = count as u32;
                p.fill_border = 0;
                let mut ext = Level::new(level.set.clone(), level.ratio, false, p, Some(count));
                ext.use_interior = true;
                ext.seeded = level.seeded;
                // The frame already is complete; this only resolves more pixels.
                ext.complete = true;
                ext.extend_to = level.extend_to;
                ext.list_in = level.list_out;
                ext.list_out = 1 - level.list_out;
                ext.escaped_past = level.escaped_past;
                ext.max_escape = level.max_escape;
                job.levels.push(ext);
                job.final_presented = false;
                return;
            }
        }
        self.learn_iterations(index);
    }

    /// Statistics-driven auto iterations (kb: techniques/auto-iterations).
    fn learn_iterations(&mut self, index: usize) {
        if !self.settings.auto_iterations {
            return;
        }
        let Some(job) = &self.job else { return };
        let level = &job.levels[index];
        let outcome = LevelOutcome {
            soft_iter: level.params.soft_iter as usize,
            texels: level.set.width * level.set.height,
            crossed: level.crossed,
            max_escape: level.max_escape,
            escaped_past: level.escaped_past,
            reached: level.params.max_iter as usize,
            // (Also without the interior test: a view all at the limit is then drawn interior
            // and must not settle so; the retry runs with the test, below.)
            may_retry: !job.interactive,
        };
        let no_test_unresolved = !job.interactive
            && job.key.interior
            && !level.use_interior
            && level.crossed as f64 >= autoiter::RETRY_FRACTION * outcome.texels as f64;
        let old = self.settings.learned_iterations;
        let autoiter::Learned { limit: learned, retry } = autoiter::learn_iterations(
            old,
            &outcome,
            self.settings.max_iterations_cap,
            depth_iterations(&self.viewport),
        );
        if self.config.trace {
            println!(
                "learn: soft {} reached {} total {} crossed {} escapedPast {:?} maxEscape {} unresolved {} -> {}{}",
                outcome.soft_iter,
                outcome.reached,
                level.total,
                level.crossed,
                level.escaped_past,
                level.max_escape,
                level.live,
                learned,
                if retry { " (render again)" } else { "" }
            );
        }
        if no_test_unresolved {
            // A quarter or more of the view reached the limit without the interior test (drawn
            // interior, never extended): render it again with the test on, at the new limit
            // if there is one, else at the same limit (the retry epoch makes it a new key).
            self.interior_stats = None;
            self.pinned_iterations = None;
            self.retry_pending = true;
            self.retry_epoch = self.retry_epoch.wrapping_add(1);
            self.settings.learned_iterations = learned;
            return;
        }
        if learned == old {
            return;
        }
        if retry {
            // The view is left unresolved: render it again at the new limit (make_key no
            // longer matches the job's key, so the next refresh starts a job).
            self.pinned_iterations = None;
            self.retry_pending = true;
        } else {
            self.pinned_iterations = Some((job.key.viewport.clone(), job.key.max_iter));
        }
        self.settings.learned_iterations = learned;
    }

    fn publish_stats(&mut self) {
        let Some(job) = &self.job else { return };
        let shown = &job.levels[job.display()];
        self.stats = ShownStats {
            viewport: Some(job.key.viewport.clone()),
            max_iterations: job.key.max_iter,
            reference_time: self.reference_time,
            reference_length: self.reference.as_ref().map_or(0, |r| r.points.len()),
            reference_period: self.reference.as_ref().and_then(|r| r.period),
            reference_bits: self.reference.as_ref().map_or(0, |r| r.precision_bits),
            reference_pending: self.reference_worker.pending().is_some(),
            deep: job.deep,
            render_size: if shown.cache.is_some() {
                (
                    (job.key.width as f64 * shown.ratio).round() as usize,
                    (job.key.height as f64 * shown.ratio).round() as usize,
                )
            } else {
                (shown.set.width, shown.set.height)
            },
            is_preview: shown.preview,
            gpu_time: job.gpu_time,
            chunks: job.chunks,
            is_rendering: !job.done(),
        };
    }

    // MARK: Colorize and compose

    /// Colorize into `target` (offscreen renders: the whole level box-filtered over its
    /// supersamples, sRGB). A preview level is first coloured at its own resolution into its
    /// colour texture (linear light), which the final pass samples bilinearly wherever the
    /// shown level is unfinished, or everywhere when the preview itself is shown.
    fn encode_colorize(&mut self, target: &B::Texture, linear: bool) -> Result<B::Encoder, GpuError> {
        self.ensure_palette()?;
        let colors = self.color_params();
        let job = self.job.as_ref().expect("job");
        let shown = &job.levels[job.display()];
        let preview = if shown.preview {
            Some(shown)
        } else {
            job.levels.iter().find(|l| l.preview && l.done)
        };
        let mut enc = self.backend.begin("colorize")?;
        let mut under = None;
        if let Some(p) = preview
            && let Some(color) = &p.set.color
        {
            self.encode_preview_color(&mut enc, p, color, &colors);
            under = Some((color, (p.set.width, p.set.height)));
        }
        let (target_w, target_h) = self.backend.texture_size(target);
        // Showing the preview: no main level (nothing "finished"), every pixel samples it.
        let level = if shown.preview { None } else { Some(shown) };
        let mut c = colors;
        c.ratio = shown.ratio as f32;
        c.has_under = if under.is_some() { 1.0 } else { 0.0 };
        c.linear_out = u32::from(linear);
        if let Some(l) = level {
            self.level_admission(l, &mut c);
            c.de_scale = l.ratio as f32;
        }
        let src = &shown.set;
        c.src_size = [src.width as u32, src.height as u32];
        c.out_size = [target_w as u32, target_h as u32];
        if let Some((_, (uw, uh))) = under {
            c.under_size = [uw as f32, uh as f32];
        }
        let rank = level
            .and_then(|l| l.order.as_ref())
            .map_or(&self.dummy_buffer, |o| &o.rank);
        enc.set_pipeline(&self.colorize_pipeline);
        enc.set_buffer(1, self.palette_buffer.as_ref().expect("palette"), 0);
        enc.set_buffer(2, &self.level_buffer, 0);
        enc.set_buffer(3, rank, 0);
        enc.set_texture(0, &src.iter);
        enc.set_texture(1, &src.de);
        enc.set_texture(2, target);
        enc.set_texture(3, under.map_or(target, |u| u.0));
        enc.set_texture(4, src.aux.as_ref().unwrap_or(&src.iter));
        enc.set_bytes(0, bytes_of(&c));
        let (tw, th) = self.threadgroup();
        enc.dispatch_threads([target_w, target_h, 1], [tw, th, 1]);
        Ok(enc)
    }

    fn threadgroup(&self) -> (usize, usize) {
        let (w, max_threads) = self.backend.pipeline_limits(&self.colorize_pipeline);
        (w, (max_threads / w / 2).max(1))
    }

    /// Admission fields of `c` for a main level: which texels are finished this frame.
    fn level_admission(&self, l: &Level<B>, c: &mut ColorParams) {
        if l.is_extension() {
            // Every texel is written: the extension only replaces unresolved ones.
            c.admitted = u32::MAX;
            c.admitted_on_gpu = 0;
        } else {
            // The running level's admission count lives on the GPU, ahead of the CPU's.
            c.admitted = if l.done { u32::MAX } else { 0 };
            c.admitted_on_gpu = u32::from(l.started() && !l.done);
        }
        c.tiles_x = tiles(l.set.width, l.set.height).0 as u32;
        c.tile_order = u32::from(l.order.is_some() && !l.is_extension());
        c.fill_border = if l.is_extension() { 0 } else { l.params.fill_border };
    }

    /// Colours a preview level at its own resolution into its colour texture (linear light).
    fn encode_preview_color(&self, enc: &mut B::Encoder, p: &Level<B>, color: &B::Texture, colors: &ColorParams) {
        let mut c = *colors;
        c.ratio = 1.0;
        self.level_admission(p, &mut c);
        c.de_scale = p.ratio as f32;
        c.has_under = 0.0;
        c.linear_out = 1;
        c.src_size = [p.set.width as u32, p.set.height as u32];
        c.out_size = c.src_size;
        enc.set_pipeline(&self.colorize_pipeline);
        enc.set_buffer(1, self.palette_buffer.as_ref().expect("palette"), 0);
        enc.set_buffer(2, &self.level_buffer, 0);
        enc.set_buffer(3, &self.dummy_buffer, 0);
        enc.set_texture(0, &p.set.iter);
        enc.set_texture(1, &p.set.de);
        enc.set_texture(2, color);
        enc.set_texture(3, color);
        enc.set_texture(4, p.set.aux.as_ref().unwrap_or(&p.set.iter));
        enc.set_bytes(0, bytes_of(&c));
        let (tw, th) = self.threadgroup();
        enc.dispatch_threads([p.set.width, p.set.height, 1], [tw, th, 1]);
    }

    /// One presented interactive frame (FL, kb: techniques/frame-reuse-and-progressive): the
    /// preview's colour pass if there is one, then `compose` into `target` and the next
    /// history image. See the kernel for the modes.
    fn encode_compose(
        &mut self,
        target: &B::Texture,
        colors: &ColorParams,
        mode: u32,
        aa_level: Option<usize>,
        now: f64,
    ) -> Result<B::Encoder, GpuError> {
        self.ensure_palette()?;
        let (out_w, out_h) = self.backend.texture_size(target);
        self.ensure_history(out_w, out_h)?;
        let history = self.history.clone().expect("history");
        let full = self.full_set.clone().expect("full level textures");
        let job = self.job.as_ref().expect("job");
        let key = job.key.clone();
        let mut enc = self.backend.begin("compose")?;

        // The preview: the gesture level, or a refinement's finished preview (its iteration
        // texels are gone once AA samples reuse the allocation; by then nothing needs it).
        let aa_started = job.levels.iter().any(|l| l.aa.is_some() && l.started());
        let preview = if mode == COMPOSE_GESTURE {
            job.levels.first().filter(|l| l.preview)
        } else if aa_started {
            None
        } else {
            job.levels.iter().find(|l| l.preview && l.done)
        };
        let mut under: Option<PreviewColor<B>> = None;
        if let Some(p) = preview
            && p.cache.is_some()
        {
            // The sample cache: its visible samples re-shaded with the current colours.
            let palette = self.palette_buffer.clone().expect("palette");
            if let Some((texture, m)) = self.sample_cache.shade(
                &self.backend,
                &key.viewport,
                key.points_per_pixel,
                key.width,
                key.height,
                colors,
                &palette,
                key.max_iter,
            ) {
                under = Some(PreviewColor {
                    texture,
                    size: m.size,
                    ratio: m.ratio,
                    jitter: [0.0, 0.0],
                    cache: Some(m),
                });
            }
        } else if let Some(p) = preview
            && let Some(color) = &p.set.color
        {
            self.encode_preview_color(&mut enc, p, color, colors);
            under = Some(PreviewColor {
                texture: color.clone(),
                size: (p.set.width, p.set.height),
                ratio: p.ratio,
                jitter: p.params.jitter,
                cache: None,
            });
        }

        // The exact samples: the shown full level (refine, exact) or an AA sample.
        let exact = match mode {
            COMPOSE_AA => aa_level.map(|i| &job.levels[i]),
            COMPOSE_GESTURE => None,
            _ => Some(&job.levels[job.display()]).filter(|l| !l.preview),
        };
        let mut c = *colors;
        c.ratio = 1.0;
        c.de_scale = 1.0;
        c.out_size = [out_w as u32, out_h as u32];
        let (iter_tex, de_tex, aux_tex, rank) = match exact {
            Some(l) => {
                self.level_admission(l, &mut c);
                c.src_size = [l.set.width as u32, l.set.height as u32];
                let rank = l.order.as_ref().map_or(self.dummy_buffer.clone(), |o| o.rank.clone());
                let aux = l.set.aux.clone().unwrap_or_else(|| l.set.iter.clone());
                (l.set.iter.clone(), l.set.de.clone(), aux, rank)
            }
            None => (
                full.iter.clone(),
                full.de.clone(),
                full.iter.clone(),
                self.dummy_buffer.clone(),
            ),
        };
        let mut q = ComposeParams {
            conf_cap: self.config.taa_conf_cap as f32,
            base_weight: self.config.taa_base_weight as f32,
            inv_two_sigma2: (1.0 / (2.0 * self.config.taa_sigma * self.config.taa_sigma)) as f32,
            mode,
            out_centre: [out_w as f32 / 2.0, out_h as f32 / 2.0],
            ..ComposeParams::default()
        };
        if let Some(u) = &under {
            let (uw, uh) = u.size;
            c.has_under = 1.0;
            c.under_size = [uw as f32, uh as f32];
            q.new_ratio = u.ratio as f32;
            q.jitter = u.jitter;
            q.new_centre = [uw as f32 / 2.0, uh as f32 / 2.0];
            if let Some(m) = &u.cache {
                q.new_centre = [m.centre[0] as f32, m.centre[1] as f32];
                q.new_rot = [m.rot[0] as f32, m.rot[1] as f32];
                q.wrap = m.wrap;
                q.cache = 1;
            }
        }
        // Reprojection of the history into this view: scale, rotation and translation.
        let s = key.viewport.scale * key.points_per_pixel;
        q.hist_rot = [1.0, 0.0];
        if let Some(h) = &self.history_view
            && self.same_colors(h, colors)
        {
            Self::reproject(h, &key.viewport, s, (out_w, out_h), &mut q);
        }
        q.blend = match mode {
            COMPOSE_REFINE => {
                let dt = (now - self.last_compose).clamp(0.0, 0.1);
                (1.0 - (-dt / self.config.fade_tau).exp()) as f32
            }
            COMPOSE_AA => 1.0 / (job.aa_composed as f32 + 2.0),
            _ => 1.0,
        };
        let (hist_in, hist_out) = (&history[self.history_ping], &history[1 - self.history_ping]);
        enc.set_pipeline(&self.compose_pipeline);
        enc.set_texture(0, &iter_tex);
        enc.set_texture(1, &de_tex);
        enc.set_texture(2, under.as_ref().map_or(hist_in, |u| &u.texture));
        enc.set_texture(3, hist_in);
        enc.set_texture(4, hist_out);
        enc.set_texture(5, target);
        enc.set_texture(6, &aux_tex);
        enc.set_bytes(0, bytes_of(&c));
        enc.set_buffer(1, self.palette_buffer.as_ref().expect("palette"), 0);
        enc.set_buffer(2, &self.level_buffer, 0);
        enc.set_buffer(3, &rank, 0);
        enc.set_bytes(4, bytes_of(&q));
        let (tw, th) = self.threadgroup();
        enc.dispatch_threads([out_w, out_h, 1], [tw, th, 1]);
        let hist_new = hist_out.clone();
        self.encode_overlay(&mut enc, &hist_new, target, false);
        self.history_ping = 1 - self.history_ping;
        self.history_view = Some(HistoryView {
            viewport: key.viewport.clone(),
            scale: s,
            width: out_w,
            height: out_h,
            colors: (*colors, self.palette_key()),
            offset: colors.offset,
        });
        Ok(enc)
    }

    /// Sets the compose pass's history mapping (FL; rotation: cinematic effects): output pixel
    /// `g` (texture axes, y down) reads history texel `origin + step · R · g`, where R turns
    /// by the angle between the two views. Leaves `q` alone (no history) for another size or
    /// an absurd scale ratio.
    fn reproject(h: &HistoryView, view: &Viewport, s: Scale, out: (usize, usize), q: &mut ComposeParams) {
        let (out_w, out_h) = out;
        let ratio = s.ratio(h.scale);
        if h.width != out_w || h.height != out_h || !(ratio > 1e-6 && ratio < 1e6) {
            return;
        }
        let step = ratio;
        // The new centre in history pixels (screen axes of the history, y up).
        let (dx, dy) = h
            .viewport
            .unrotate_vec(view.center.delta_in(&h.viewport.center, h.scale));
        // Output offsets (y up) turn by phi = new angle - history angle; in texture axes
        // (y down) that is a turn by -phi.
        let phi = view.angle - h.viewport.angle;
        let (c, sn) = if phi == 0.0 {
            (1.0, 0.0)
        } else {
            ((-phi).cos(), (-phi).sin())
        };
        let (ox, oy) = {
            let (x, y) = (0.5 - out_w as f64 / 2.0, 0.5 - out_h as f64 / 2.0);
            (x * c - y * sn, x * sn + y * c)
        };
        q.hist_step = step as f32;
        q.hist_rot = [c as f32, sn as f32];
        q.hist_origin = [
            (step * ox + dx + h.width as f64 / 2.0) as f32,
            (step * oy - dy + h.height as f64 / 2.0) as f32,
        ];
        q.hist_size = [h.width as f32, h.height as f32];
        // Zoom-in stretches each history texel over 1/step^2 pixels: its confidence shrinks
        // with it, and new samples take over the detail.
        q.hist_decay = step.min(1.0).powi(2) as f32;
    }

    /// A frame of the reprojected history only (M3b: while a deep view's reference computes):
    /// the compose pass in gesture mode with no new samples.
    fn encode_history_only(
        &mut self,
        target: &B::Texture,
        colors: &ColorParams,
        key: &ComputeKey,
    ) -> Result<B::Encoder, GpuError> {
        self.ensure_palette()?;
        let (out_w, out_h) = self.backend.texture_size(target);
        self.ensure_history(out_w, out_h)?;
        let history = self.history.clone().expect("history");
        let mut enc = self.backend.begin("compose history")?;
        let mut c = *colors;
        c.ratio = 1.0;
        c.de_scale = 1.0;
        c.out_size = [out_w as u32, out_h as u32];
        c.src_size = [1, 1];
        c.has_under = 0.0;
        let mut q = ComposeParams {
            conf_cap: self.config.taa_conf_cap as f32,
            base_weight: self.config.taa_base_weight as f32,
            inv_two_sigma2: (1.0 / (2.0 * self.config.taa_sigma * self.config.taa_sigma)) as f32,
            mode: COMPOSE_GESTURE,
            out_centre: [out_w as f32 / 2.0, out_h as f32 / 2.0],
            ..ComposeParams::default()
        };
        let s = key.viewport.scale * key.points_per_pixel;
        q.hist_rot = [1.0, 0.0];
        if let Some(h) = &self.history_view
            && self.same_colors(h, colors)
        {
            Self::reproject(h, &key.viewport, s, (out_w, out_h), &mut q);
        }
        let (hist_in, hist_out) = (&history[self.history_ping], &history[1 - self.history_ping]);
        enc.set_pipeline(&self.compose_pipeline);
        enc.set_texture(0, hist_in);
        enc.set_texture(1, hist_in);
        enc.set_texture(2, hist_in);
        enc.set_texture(3, hist_in);
        enc.set_texture(4, hist_out);
        enc.set_texture(5, target);
        enc.set_texture(6, hist_in);
        enc.set_bytes(0, bytes_of(&c));
        enc.set_buffer(1, self.palette_buffer.as_ref().expect("palette"), 0);
        enc.set_buffer(2, &self.level_buffer, 0);
        enc.set_buffer(3, &self.dummy_buffer, 0);
        enc.set_bytes(4, bytes_of(&q));
        let (tw, th) = self.threadgroup();
        enc.dispatch_threads([out_w, out_h, 1], [tw, th, 1]);
        let hist_new = hist_out.clone();
        self.encode_overlay(&mut enc, &hist_new, target, false);
        self.history_ping = 1 - self.history_ping;
        self.history_view = Some(HistoryView {
            viewport: key.viewport.clone(),
            scale: s,
            width: out_w,
            height: out_h,
            colors: (*colors, self.palette_key()),
            offset: colors.offset,
        });
        Ok(enc)
    }

    /// Composites the overlay scene over `target` (kb: decisions/controls, "Orbit overlay and
    /// mini-map"): sparse after a compose pass (only the tiles with primitives, from the new
    /// history `src`), full when only the overlay changed (every pixel from the history).
    /// Without a scene a full pass copies the clean history (an overlay was switched off).
    fn encode_overlay(&mut self, enc: &mut B::Encoder, src: &B::Texture, target: &B::Texture, full: bool) {
        let (w, h) = self.backend.texture_size(target);
        // No scene, or one built for another size (a resize in between): nothing to add.
        if self.overlay.is_none() || self.overlay.as_ref().is_some_and(|d| d.size != (w, h)) {
            self.overlay_shown = None;
            if !full {
                return;
            }
        }
        if self.overlay_pipeline.is_none() {
            match self.backend.pipeline("overlay", &[]) {
                Ok(p) => self.overlay_pipeline = Some(p),
                Err(e) => {
                    eprintln!("almondbread: overlay pipeline: {e}");
                    return;
                }
            }
        }
        if self.overlay_font.is_none() {
            self.overlay_font = self.backend.create_buffer_with_data(&crate::overlay::font_bytes()).ok();
        }
        let empty;
        let d = match &self.overlay {
            Some(d) if d.size == (w, h) => d,
            _ => {
                let Ok(e) = crate::overlay::OverlayDraw::upload(
                    &self.backend,
                    &crate::overlay::Scene::default(),
                    (w, h),
                    Vec::new(),
                    0,
                ) else {
                    return;
                };
                empty = e;
                &empty
            }
        };
        if !full && d.active_count == 0 {
            self.overlay_shown = self.overlay.as_ref().map(|d| d.version);
            return;
        }
        let (Some(pipeline), Some(font)) = (&self.overlay_pipeline, &self.overlay_font) else {
            return;
        };
        let (tx, _) = tiles(w, h);
        let params = crate::overlay::OverlayParams {
            out_size: [w as u32, h as u32],
            tiles_x: tx as u32,
            full: u32::from(full),
        };
        enc.set_pipeline(pipeline);
        enc.set_texture(0, src);
        enc.set_texture(1, target);
        for i in 0..4 {
            enc.set_texture(2 + i, d.maps.get(i).unwrap_or(src));
        }
        enc.set_bytes(0, bytes_of(&params));
        enc.set_buffer(1, &d.prims, 0);
        enc.set_buffer(2, &d.tile_start, 0);
        enc.set_buffer(3, &d.tile_list, 0);
        enc.set_buffer(4, &d.active, 0);
        enc.set_buffer(5, font, 0);
        if full {
            enc.dispatch_threads([w, h, 1], [16, 16, 1]);
        } else {
            enc.dispatch_threads([16, 16, d.active_count], [16, 16, 1]);
        }
        self.overlay_shown = self.overlay.as_ref().filter(|d| d.size == (w, h)).map(|d| d.version);
    }

    /// The overlay changed (or went away) since the last presented frame while nothing else
    /// needs composing: present the latest history with the current overlay.
    fn overlay_stale(&self) -> bool {
        self.overlay.as_ref().map(|d| d.version) != self.overlay_shown && self.history_view.is_some()
    }

    fn encode_overlay_only(&mut self, target: &B::Texture) -> Result<B::Encoder, GpuError> {
        let history = self.history.clone().ok_or_else(|| GpuError("no history".into()))?;
        let mut enc = self.backend.begin("overlay")?;
        let src = history[self.history_ping].clone();
        self.encode_overlay(&mut enc, &src, target, true);
        Ok(enc)
    }

    /// The shown image with the current overlay, as BGRA8 sRGB pixels (screenshots of the view
    /// for documentation and tests; blocks for the GPU). None before the first frame.
    pub fn capture_shown(&mut self) -> Option<(Vec<u8>, usize, usize)> {
        let history = self.history.clone()?;
        self.history_view.as_ref()?;
        let (w, h) = self.backend.texture_size(&history[0]);
        let target = self
            .backend
            .create_texture(w, h, TextureFormat::Bgra8Unorm, Storage::Shared)
            .ok()?;
        let shown = self.overlay_shown;
        let mut enc = self.backend.begin("capture").ok()?;
        let src = history[self.history_ping].clone();
        self.encode_overlay(&mut enc, &src, &target, true);
        self.overlay_shown = shown;
        enc.commit().wait();
        let mut pixels = vec![0u8; w * h * 4];
        self.backend
            .read_texture(&target, w, h, TextureFormat::Bgra8Unorm, &mut pixels)
            .ok()?;
        Some((pixels, w, h))
    }

    /// The history images at the output size (reallocated, and forgotten, on resize).
    fn ensure_history(&mut self, width: usize, height: usize) -> Result<(), GpuError> {
        if let Some(h) = &self.history
            && self.backend.texture_size(&h[0]) == (width, height)
        {
            return Ok(());
        }
        let b = &self.backend;
        self.history = Some([
            b.create_texture(width, height, TextureFormat::Rgba16Float, Storage::Private)?,
            b.create_texture(width, height, TextureFormat::Rgba16Float, Storage::Private)?,
        ]);
        self.history_view = None;
        Ok(())
    }

    /// Colour parameters of the current settings; the palette phase includes colour cycling
    /// and the depth tint (FL).
    fn color_params(&self) -> ColorParams {
        let s = &self.settings;
        let a = s.light_angle * std::f64::consts::PI / 180.0;
        let i = s.interior_color;
        let e = s.edge_color;
        let g = s.glow_color;
        let depth = self.viewport.zoom_log10().max(0.0);
        // Cinematic palette drift: hue and phase as functions of depth (the same view always
        // has the same colours), plus cycling while flying (in `cycle_phase`).
        let (hue, phase) = s.cinematic.palette_at(depth * std::f64::consts::LN_10);
        let offset = (s.color_offset + self.cycle_phase + s.depth_tint * depth + phase).rem_euclid(1.0);
        let (deep, filament) = self.dense_colors();
        ColorParams {
            interior: [i[0], i[1], i[2], 1.0],
            edge: [e[0], e[1], e[2], 1.0],
            glow_color: [g[0], g[1], g[2], 1.0],
            light: [a.cos() as f32, a.sin() as f32],
            density: s.color_density as f32,
            offset: offset as f32,
            ratio: 1.0,
            shading: s.shading as f32,
            outline: s.outline as f32,
            outline_width: s.outline_width as f32,
            palette_size: PALETTE_SIZE as u32,
            log_scale: u32::from(s.log_coloring),
            smooth: u32::from(s.smooth_coloring),
            de_scale: 1.0,
            specular: s.specular as f32,
            shininess: s.shininess as f32,
            light_height: s.light_height as f32,
            glow: s.glow as f32,
            glow_width: s.glow_width as f32,
            palette_levels: almondbread_core::palette::mip_levels(PALETTE_SIZE) as u32,
            palette_filter: if s.palette_filter { 1.0 } else { 0.0 },
            // The shown job's textures must hold the cycle analysis (a mode switch from flat
            // starts a new job; until it shows, the interior stays flat).
            interior_mode: if self.job.as_ref().is_some_and(|j| j.key.interior_color) {
                s.interior_mode.code()
            } else {
                0
            },
            interior_density: s.interior_density as f32,
            interior_stripes: s.interior_stripes as f32,
            hue_rot: if hue == 0.0 {
                [1.0, 0.0]
            } else {
                [hue.cos() as f32, hue.sin() as f32]
            },
            deep: [deep[0], deep[1], deep[2], if s.dense_calm { 1.0 } else { 0.0 }],
            filament: [filament[0], filament[1], filament[2], 1.0],
            ..ColorParams::default()
        }
    }

    fn ensure_palette(&mut self) -> Result<(), GpuError> {
        let key = self.palette_key();
        if self.palette_id.as_deref() == Some(key.as_str()) && self.palette_buffer.is_some() {
            return Ok(());
        }
        let table = almondbread_core::palette::mip_chain(&self.settings.palette_table(PALETTE_SIZE));
        self.palette_buffer = Some(self.backend.create_buffer_with_data(bytemuck::cast_slice(&table))?);
        self.palette_id = Some(key);
        Ok(())
    }

    /// Identifies the palette in use: the built-in id, or the custom stops.
    fn palette_key(&self) -> String {
        match &self.settings.palette_stops {
            Some(stops) if !stops.is_empty() => format!("stops:{stops:?}"),
            _ => self.settings.palette_id.clone(),
        }
    }

    /// The dense-region colours of the current palette (cached per palette).
    fn dense_colors(&self) -> ([f32; 3], [f32; 3]) {
        let key = self.palette_key();
        let mut cache = self.dense_cache.borrow_mut();
        if let Some((k, d, f)) = cache.as_ref()
            && *k == key
        {
            return (*d, *f);
        }
        let (mut d, f) = almondbread_core::palette::dense_colors(&self.settings.palette_table(256));
        // A library palette carries its designed deep colour: keep its hue, at the calm
        // luminance (the library's deep is near black; interior and unresolved pixels must stay
        // distinguishable from dense regions).
        if self.settings.palette_stops.is_none()
            && let Some(lib) = almondbread_core::palette_library::find(&self.settings.palette_id)
        {
            let c = lib.deep_linear();
            let lum = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
            if lum > 1e-5 {
                let k = 0.012 / lum;
                d = [(c[0] * k).min(1.0), (c[1] * k).min(1.0), (c[2] * k).min(1.0)];
            }
        }
        *cache = Some((key, d, f));
        (d, f)
    }

    /// The full-level textures, reused while the size holds.
    fn full_texture_set(&mut self, width: usize, height: usize) -> Result<TextureSet<B>, GpuError> {
        if let Some(s) = &mut self.full_set
            && s.width == width
            && s.height == height
        {
            if self.want_aux && s.aux.is_none() {
                s.aux = Some(
                    self.backend
                        .create_texture(width, height, TextureFormat::Rg32Float, Storage::Private)?,
                );
            }
            return Ok(s.clone());
        }
        let b = &self.backend;
        let s = TextureSet {
            iter: b.create_texture(width, height, TextureFormat::Rg32Float, Storage::Private)?,
            de: b.create_texture(width, height, TextureFormat::Rg16Float, Storage::Private)?,
            color: None,
            aux: if self.want_aux {
                Some(b.create_texture(width, height, TextureFormat::Rg32Float, Storage::Private)?)
            } else {
                None
            },
            width,
            height,
        };
        self.full_set = Some(s.clone());
        Ok(s)
    }

    /// A preview level's textures: the top-left `width` x `height` of one allocation sized for
    /// the largest preview of a `max_w` x `max_h` view (preview ratios are at most 1), made
    /// once per window size. The Swift renderer cached one texture set per preview size.
    fn preview_texture_set(
        &mut self,
        max_w: usize,
        max_h: usize,
        width: usize,
        height: usize,
    ) -> Result<TextureSet<B>, GpuError> {
        if let Some(s) = &mut self.preview_alloc {
            let (aw, ah) = self.backend.texture_size(&s.iter);
            if aw >= width && ah >= height {
                if self.want_aux && s.aux.is_none() {
                    s.aux = Some(
                        self.backend
                            .create_texture(aw, ah, TextureFormat::Rg32Float, Storage::Private)?,
                    );
                }
                return Ok(s.with_size(width, height));
            }
        }
        let (aw, ah) = (max_w.max(width), max_h.max(height));
        let b = &self.backend;
        let s = TextureSet {
            iter: b.create_texture(aw, ah, TextureFormat::Rg32Float, Storage::Private)?,
            de: b.create_texture(aw, ah, TextureFormat::Rg16Float, Storage::Private)?,
            color: Some(b.create_texture(aw, ah, TextureFormat::Rgba16Float, Storage::Private)?),
            aux: if self.want_aux {
                Some(b.create_texture(aw, ah, TextureFormat::Rg32Float, Storage::Private)?)
            } else {
                None
            },
            width: aw,
            height: ah,
        };
        self.preview_alloc = Some(s.clone());
        Ok(s.with_size(width, height))
    }

    /// Unresolved lists sized for the full level (allocated on first need, grown with it).
    fn ensure_list_capacity(&mut self, texels: usize) {
        let want = texels.min(self.config.max_list_capacity);
        if want <= self.list_capacity {
            return;
        }
        let (Ok(a), Ok(b)) = (
            self.backend.create_buffer(want * 4, Storage::Private),
            self.backend.create_buffer(want * 4, Storage::Private),
        ) else {
            return;
        };
        self.list_buffers = Some([a, b]);
        self.list_capacity = want;
    }

    /// Computes the reference orbit for `viewport` now, so a following render of a nearby view
    /// reuses it as panning and zooming do (`verify --ref-offset`). Deep views: a big reference
    /// at `viewport`'s centre (no nucleus search) with the precision a `size`-texel view at
    /// `texel` needs.
    pub fn prime_reference(&mut self, viewport: &Viewport, max_iter: usize, texel: Scale, size: usize) {
        self.reference = None;
        if viewport.scale.to_f64() >= DEEP_SCALE {
            let r = Reference::compute_f64(viewport, max_iter, BAILOUT);
            self.set_reference(r);
            return;
        }
        let mut req = ReferenceRequest::for_view(viewport, texel, size, 0.0, max_iter, BAILOUT);
        req.nucleus = false;
        self.reference_worker.start(req);
        if let Some(r) = self.reference_worker.poll(true) {
            self.set_reference(r);
        }
    }

    /// Makes `r` the current reference: GPU buffers, BLA reach (the old table goes; in-flight
    /// chunks keep their buffers).
    fn set_reference(&mut self, r: Reference) {
        let buf = match self.backend.create_buffer_with_data(bytemuck::cast_slice(&r.points)) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("almondbread: reference buffer: {e}");
                return;
            }
        };
        let refx = if r.extended.is_empty() {
            None
        } else {
            // FloatExpComplex has a padding word: copy into the padded GPU form.
            let gpu: Vec<[u32; 4]> = r
                .extended
                .iter()
                .map(|z| [z.mantissa[0].to_bits(), z.mantissa[1].to_bits(), z.exponent as u32, 0])
                .collect();
            match self.backend.create_buffer_with_data(bytemuck::cast_slice(&gpu)) {
                Ok(b) => Some(b),
                Err(e) => {
                    eprintln!("almondbread: reference buffer: {e}");
                    return;
                }
            }
        };
        self.reference_time = r.time();
        self.reference_buffer = Some(buf);
        self.refx_buffer = refx;
        self.reference_serial += 1;
        // The table of the old reference goes (in-flight chunks keep their buffer); a build
        // still running for it finishes on its thread and is dropped.
        self.bla = None;
        self.bla_build = None;
        let eps = self.config.bla_epsilon;
        self.bla_reach_log2 = if r.deep_kernel {
            BlaTableX::reach_log2(&r.extended, eps)
        } else {
            let n = r.points.len().saturating_sub(1);
            r.points[1.min(n)..n]
                .iter()
                .map(|z| eps * 2.0 * f64::from(z[0]).hypot(f64::from(z[1])))
                .fold(0.0, f64::max)
                .log2()
        };
        if self.config.trace {
            println!(
                "reference: {} {} bits, {} points{}, {:.1} ms",
                if r.big { "big" } else { "f64" },
                r.precision_bits,
                r.points.len(),
                r.period.map_or(String::new(), |p| format!(" (nucleus, period {p})")),
                r.time() * 1000.0
            );
        }
        self.reference = Some(r);
    }

    /// Takes a finished background reference if it serves the view at least as well as the
    /// current one (kb: techniques/reference-selection).
    fn adopt_worker_reference(&mut self, viewport: &Viewport, texel: Scale, size: usize, max_iter: usize, wait: bool) {
        let Some((started, _)) = self.reference_worker.running_for() else {
            return;
        };
        let Some(r) = self.reference_worker.poll(wait) else {
            return;
        };
        self.reference_log.push(ReferenceLogEntry {
            scale_log10: r.scale.log10(),
            precision_bits: r.precision_bits,
            length: r.points.len(),
            period: r.period,
            select_time: r.select_time,
            orbit_time: r.orbit_time,
            latency: started,
        });
        let new_fit = r.fit(viewport, texel, size, max_iter);
        let old_fit = self
            .reference
            .as_ref()
            .map_or(Fit::No, |c| c.fit(viewport, texel, size, max_iter));
        let better = matches!(
            (new_fit, old_fit),
            (Fit::Good, _) | (Fit::Stale, Fit::No) | (Fit::Stale, Fit::Stale)
        );
        if better {
            self.set_reference(r);
        } else {
            self.spare_reference = Some(r);
        }
    }

    /// The journey rests at a target and knows the next one (kb: decisions/controls,
    /// "Journey"): compute the next target's reference now, while the worker is idle, so the
    /// next leg does not wait for it. It is kept as the spare until a view needs it.
    fn prefetch_journey_reference(&mut self, texel: Scale, size: usize, viewport: &Viewport) {
        if self.reference_worker.running_for().is_some() {
            return;
        }
        let Some(t) = self.navigator.prefetch_target() else {
            return;
        };
        if t.scale.to_f64() >= DEEP_SCALE {
            return;
        }
        let t_texel = t.scale * texel.ratio(viewport.scale);
        let max_iter = self.settings.max_iterations(&t);
        let serves = |r: &Reference| r.fit(&t, t_texel, size, max_iter) == Fit::Good;
        if self.reference.as_ref().is_some_and(serves) || self.spare_reference.as_ref().is_some_and(serves) {
            return;
        }
        if self.pending_serves(&t, t_texel, size) {
            return;
        }
        let radius = 0.5 * size as f64 * t_texel.ratio(t.scale);
        let mut q = ReferenceRequest::for_view(&t, t_texel, size, radius, max_iter * 2, BAILOUT);
        q.period_limit = max_iter;
        if self.config.trace {
            println!(
                "reference prefetch (journey): scale {} bits {}",
                t.scale, q.precision_bits
            );
        }
        self.reference_worker.start(q);
    }

    /// Whether a pending request will give a reference that serves this view.
    fn pending_serves(&self, viewport: &Viewport, texel: Scale, size: usize) -> bool {
        self.reference_worker.pending().is_some_and(|q| {
            q.precision_bits >= almondbread_core::reference::needed_bits(&viewport.center, texel, size, 40)
                && viewport.center.distance_in(&q.center, texel) <= size as f64
        })
    }

    /// Makes sure a reference serving `key` is current (kb: techniques/rebasing: any nearby
    /// reference is exact). Shallow views compute the f64 orbit at once. Deep views reuse the
    /// current big reference while it serves, request a better one in the background when it
    /// gets stale, and return false while none is usable (the caller shows the history). With
    /// `wait` (offscreen renders) a missing reference is waited for.
    fn prepare_reference(&mut self, key: &ComputeKey, texel: Scale, size: usize, wait: bool) -> bool {
        let viewport = &key.viewport;
        let max_iter = key.max_iter;
        self.adopt_worker_reference(viewport, texel, size, max_iter, false);
        let mut fit = self
            .reference
            .as_ref()
            .map_or(Fit::No, |r| r.fit(viewport, texel, size, max_iter));
        if fit != Fit::Good
            && let Some(spare) = self.spare_reference.take()
        {
            if spare.fit(viewport, texel, size, max_iter) == Fit::Good {
                self.set_reference(spare);
                fit = Fit::Good;
            } else {
                self.spare_reference = Some(spare);
            }
        }
        if viewport.scale.to_f64() >= DEEP_SCALE {
            if fit != Fit::Good {
                let r = Reference::compute_f64(viewport, max_iter, BAILOUT);
                self.set_reference(r);
            }
            return self.reference_buffer.is_some();
        }
        if fit == Fit::Good {
            if !wait {
                let v = viewport.clone();
                self.prefetch_journey_reference(texel, size, &v);
            }
            return true;
        }
        if !self.pending_serves(viewport, texel, size) {
            let radius = 0.5 * (key.width as f64).hypot(key.height as f64) * key.points_per_pixel;
            let req = ReferenceRequest::for_view(viewport, texel, size, radius, max_iter * 2, BAILOUT);
            let mut req = req;
            // Nuclei up to the learner's reach (4x the depth heuristic): a longer-period
            // minibrot in view then sets the limit (`period_floor`) instead of an unresolved
            // first pass.
            let search = if self.settings.auto_iterations {
                (4 * depth_iterations(viewport)).min(self.settings.max_iterations_cap)
            } else {
                0
            };
            req.period_limit = max_iter.max(search);
            // Flying to a deeper target inside this view: one reference at the target, with
            // the precision the target needs, serves every view of the rest of the dive (kb:
            // decisions/controls, fly to; techniques/rebasing).
            if let Some(t) = self.navigator.fly_target()
                && t.scale < viewport.scale
                && viewport.center.distance_in(&t.center, texel) <= 0.5 * size as f64
            {
                let t_texel = t.scale * texel.ratio(viewport.scale);
                // As a jump to the target would request it: the nucleus search around the target
                // and the target's own scale (kernel). A plain centre reference at the target
                // (nucleus search off, kernel picked at the current depth) left smooth wrong
                // fans in the settled frame at sea-8007 (35.8 % of escape counts differed from a
                // fresh render; `verify --history flyto` now gates on that).
                let mut q = ReferenceRequest::for_view(t, t_texel, size, radius, max_iter * 2, BAILOUT);
                q.period_limit = max_iter;
                req = q;
            }
            if self.config.trace {
                println!(
                    "reference request: scale {} bits {} period limit {} ({})",
                    viewport.scale,
                    req.precision_bits,
                    req.period_limit,
                    if fit == Fit::Stale { "stale" } else { "none usable" }
                );
            }
            self.reference_worker.start(req);
        }
        if fit == Fit::Stale {
            return true;
        }
        if wait {
            self.adopt_worker_reference(viewport, texel, size, max_iter, true);
        }
        self.reference
            .as_ref()
            .is_some_and(|r| r.fit(viewport, texel, size, max_iter) != Fit::No)
    }

    /// Reference orbit time and length of the last view (for reports).
    pub fn reference_info(&self) -> (f64, usize) {
        (
            self.reference_time,
            self.reference.as_ref().map_or(0, |r| r.points.len()),
        )
    }

    /// The current reference (diagnostics).
    pub fn reference(&self) -> Option<&Reference> {
        self.reference.as_ref()
    }

    /// Waits for the colorize passes in flight and returns how many frames failed on the
    /// GPU so far (self-tests: a committed frame is not yet a drawn one).
    pub fn finish_frames(&mut self) -> usize {
        for (_, f) in std::mem::take(&mut self.frames_in_flight) {
            f.wait();
            if let SubmissionStatus::Failed(e) = f.status() {
                eprintln!("almondbread: frame failed on the GPU: {e}");
                self.failed_frames += 1;
            }
        }
        self.failed_frames
    }

    /// The current view's refined frame has been presented and nothing is left to iterate.
    /// Serial of the current reference (changes whenever a new one is adopted).
    pub fn reference_serial(&self) -> u64 {
        self.reference_serial
    }

    pub fn is_settled(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|j| j.final_presented && !j.interactive && j.key.viewport == self.viewport)
    }

    fn drain_in_flight(&mut self) {
        if let Some(r) = self.in_flight.back() {
            r.submission.wait();
        }
        self.harvest_seed_round(true);
        self.harvest();
    }

    fn wait_frames(&mut self) {
        for (_, f) in &self.frames_in_flight {
            f.wait();
        }
        self.frames_in_flight.clear();
    }

    /// Serial of the last frame committed for presentation (1, 2, ...; 0: none yet).
    pub fn frames_committed(&self) -> u64 {
        self.frames_committed
    }

    /// Serial up to which every committed frame has finished on the GPU (the queue is in
    /// order). Headless capture reads a frame's target once this reaches its serial.
    pub fn frames_completed(&self) -> u64 {
        self.frames_in_flight
            .iter()
            .find(|(_, s)| s.status() == SubmissionStatus::Pending)
            .map_or(self.frames_committed, |(serial, _)| serial - 1)
    }

    /// Blocks until the frame with `serial` has finished on the GPU.
    pub fn wait_frame(&self, serial: u64) {
        for (s, sub) in &self.frames_in_flight {
            if *s <= serial {
                sub.wait();
            }
        }
    }

    fn read_iterations(&self, set: &TextureSet<B>) -> Result<Vec<[f32; 2]>, GpuError> {
        let mut texels = vec![[0f32; 2]; set.width * set.height];
        self.backend.read_texture(
            &set.iter,
            set.width,
            set.height,
            TextureFormat::Rg32Float,
            bytemuck::cast_slice_mut(&mut texels),
        )?;
        Ok(texels)
    }

    /// The last job's distance-estimate texels (half floats: DE in texels, normal angle) of
    /// its full level, row-major (diagnostics and tests).
    pub fn read_distance_estimates(&self) -> Result<Vec<[u16; 2]>, GpuError> {
        let Some(level) = self.job.as_ref().and_then(|j| j.levels.last()) else {
            return Err(GpuError("no job".into()));
        };
        let set = &level.set;
        let mut texels = vec![[0u16; 2]; set.width * set.height];
        self.backend.read_texture(
            &set.de,
            set.width,
            set.height,
            TextureFormat::Rg16Float,
            bytemuck::cast_slice_mut(&mut texels),
        )?;
        Ok(texels)
    }

    // MARK: Offscreen rendering (headless checks, export)

    /// Renders the current viewport into an offscreen texture through the same chunked
    /// scheduler as the view (no preview level), at one view point per pixel. Blocks.
    pub fn render_offscreen(
        &mut self,
        width: usize,
        height: usize,
        readback: bool,
    ) -> Result<OffscreenResult, GpuError> {
        let start = Instant::now();
        let target = self
            .backend
            .create_texture(width, height, TextureFormat::Bgra8Unorm, Storage::Shared)?;
        let mut finished = false;
        let mut attempts = 0;
        // Failed jobs are retried; so is a view the learner left unresolved and raised the
        // limit for (kb: techniques/auto-iterations; at most one raise per view: the 4x depth
        // cap), so an offscreen render shows the structure without a warm-up render.
        let mut retries = 0;
        while !finished && attempts < 4 {
            attempts += 1;
            self.drain_in_flight();
            self.job = None;
            let now = self.clock();
            let key = self.make_key(width, height, 1.0, true);
            self.start_job(key, JobMode::Offscreen, now);
            if self.job.is_none() {
                return Err(GpuError("offscreen job could not start".into()));
            }
            loop {
                self.harvest();
                let job = self.job.as_ref().expect("job");
                if job.done() || job.failed {
                    break;
                }
                let budget = self.config.chunk_target;
                if self.encode_chunks(budget) == 0 {
                    match self.in_flight.front() {
                        Some(first) => first.submission.wait(),
                        None if self.seed_round.is_some() => self.harvest_seed_round(true),
                        None => {
                            self.job.as_mut().expect("job").failed = true;
                            break;
                        }
                    }
                }
            }
            finished = !self.job.as_ref().expect("job").failed;
            if finished && self.retry_pending && retries < 3 {
                retries += 1;
                attempts -= 1;
                finished = false;
            }
        }
        self.retry_pending = false;
        if !finished {
            return Err(GpuError("offscreen render failed".into()));
        }
        // With an overlay (CLI `--orbit`, `--map`): colour into linear light, then composite.
        let enc = if self.overlay.is_some() {
            let lin = self
                .backend
                .create_texture(width, height, TextureFormat::Rgba16Float, Storage::Private)?;
            let mut enc = self.encode_colorize(&lin, true)?;
            self.encode_overlay(&mut enc, &lin, &target, true);
            enc
        } else {
            self.encode_colorize(&target, false)?
        };
        let sub = enc.commit();
        sub.wait();
        if let SubmissionStatus::Failed(e) = sub.status() {
            return Err(GpuError(format!("offscreen colorize failed: {e}")));
        }
        let mut pixels = vec![0u8; width * height * 4];
        self.backend
            .read_texture(&target, width, height, TextureFormat::Bgra8Unorm, &mut pixels)?;
        let wall = start.elapsed().as_secs_f64();
        self.publish_stats();
        let job = self.job.as_ref().expect("job");
        let iterations = if readback {
            Some(self.read_iterations(&job.levels.last().expect("level").set)?)
        } else {
            None
        };
        Ok(OffscreenResult {
            pixels,
            width,
            height,
            iterate_gpu_time: job.gpu_time,
            colorize_gpu_time: sub.gpu_time(),
            wall_time: wall,
            chunks: job.chunks,
            iterations_done: job.iterations as u64,
            steps_done: job.steps as u64,
            bla_forecast: job.bla_forecast,
            bla_used: job.levels.iter().any(|l| l.use_bla),
            iterations,
            seeds: if job.levels.iter().any(|l| l.seeding.is_some()) {
                self.seed_report()
            } else {
                None
            },
        })
    }

    // MARK: Scripted sessions (headless checks of the interactive pipeline)

    /// Drives the interactive scheduler headlessly through [`Renderer::render_frame`], the
    /// code the app runs per display refresh: `input(frame, dt, viewport)` runs before each
    /// of the first `input_frames` refreshes (counted as a gesture), then the renderer idles
    /// until the refined final frame is presented. Paced in real time, since the gesture/idle
    /// logic is time based.
    #[allow(clippy::too_many_arguments)]
    pub fn simulate(
        &mut self,
        width: usize,
        height: usize,
        points_per_pixel: f64,
        refresh_rate: f64,
        input_frames: usize,
        timeout: f64,
        input: &mut dyn FnMut(usize, f64, &mut Viewport),
    ) -> Result<SimulationResult, GpuError> {
        let target = self
            .backend
            .create_texture(width, height, TextureFormat::Bgra8Unorm, Storage::Shared)?;
        let interval = 1.0 / refresh_rate;
        let mut tick = self.clock();
        let mut frames = 0;
        let refresh = |r: &mut Self, tick: &mut f64, frames: &mut usize| {
            let now = r.clock();
            let t = target.clone();
            r.render_frame(now, width, height, points_per_pixel, &mut |_| Some((t.clone(), None)));
            *frames += 1;
            *tick += interval;
            let left = *tick - r.clock();
            if left > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(left));
            }
        };
        let mut prev = self.clock();
        let sim_start = prev;
        self.first_job_frame = None;
        for i in 0..input_frames {
            let now = self.clock();
            self.navigator.note_interaction(now);
            let dt = if i == 0 { interval } else { now - prev };
            input(i, dt, &mut self.viewport);
            prev = now;
            refresh(self, &mut tick, &mut frames);
        }
        if input_frames > 0
            && let Some(mut f) = self.cache_check_renderer.take()
        {
            let c = self.sample_cache_check_with(&mut f);
            self.cache_checks.push(c);
            self.cache_check_renderer = Some(f);
        }
        // A jump (no input frames) counts from the start.
        let input_end = if input_frames == 0 {
            sim_start
        } else {
            self.navigator.last_interaction
        };
        let deadline = self.clock() + timeout;
        loop {
            let finished = self
                .job
                .as_ref()
                .is_some_and(|j| j.final_presented && !j.interactive && j.key.viewport == self.viewport)
                && !self.retry_pending;
            if finished {
                break;
            }
            if self.clock() >= deadline {
                return Err(GpuError("simulated session did not finish".into()));
            }
            refresh(self, &mut tick, &mut frames);
        }
        let final_after = self.last_final_frame.0 - input_end;
        let done_after = self.clock() - input_end;
        let first_after = self.first_job_frame.map_or(f64::NAN, |t| t - input_end);
        self.wait_frames();
        self.drain_in_flight();
        let mut pixels = vec![0u8; width * height * 4];
        self.backend
            .read_texture(&target, width, height, TextureFormat::Bgra8Unorm, &mut pixels)?;
        let job = self.job.as_ref().expect("job");
        let shown = &job.levels[job.display()];
        let iterations = self.read_iterations(&shown.set)?;
        let unresolved = iterations.iter().filter(|t| t[0] == ITER_UNRESOLVED).count();
        Ok(SimulationResult {
            pixels,
            width,
            height,
            level_width: shown.set.width,
            level_height: shown.set.height,
            viewport: job.key.viewport.clone(),
            max_iter: job.key.max_iter,
            hard_iter: shown.params.max_iter as usize,
            texel_scale: job.key.viewport.scale * job.key.points_per_pixel / shown.ratio,
            preview: shown.preview,
            deep: job.deep,
            frames,
            final_after,
            done_after,
            first_after,
            unresolved,
            reference_length: shown.params.ref_len as usize,
            iterations,
        })
    }
}

impl<B: GpuBackend + Clone> Renderer<B> {
    /// Diffs the iteration texels of the level the view shows against a fresh offscreen
    /// render (a new renderer on the same device, so no shared state) of the same texel grid,
    /// view and limit: pixels whose class (escaped / interior) differs. Unresolved pixels on
    /// either side are not counted. For interaction tests: stale, unfinished or misclassified
    /// pixels of the live pipeline show up here (kb: techniques/interior-detection, Pitfalls).
    /// None while a gesture or preview frame is shown.
    pub fn displayed_frame_mismatch(&mut self) -> Option<FrameMismatch> {
        let job = self.job.as_ref()?;
        if job.interactive {
            return None;
        }
        let shown = &job.levels[job.display()];
        if shown.preview {
            return None;
        }
        let set = shown.set.clone();
        let scale = job.key.viewport.scale * job.key.points_per_pixel / shown.ratio;
        let center = job.key.viewport.center.clone();
        let (max_iter, interior) = (job.key.max_iter, job.key.interior);
        let mine = self.read_iterations(&set).ok()?;
        let mut fresh = Renderer::new(self.backend.clone()).ok()?;
        fresh.viewport = Viewport {
            center,
            scale,
            angle: job.key.viewport.angle,
        };
        fresh.settings.auto_iterations = false;
        fresh.settings.manual_iterations = max_iter;
        fresh.settings.max_iterations_cap = self.settings.max_iterations_cap.max(max_iter);
        fresh.settings.interior_detection = interior;
        // The shown level is one sample per pixel (anti-aliasing accumulates separately).
        fresh.settings.supersampling = almondbread_core::settings::Supersampling::Off;
        let theirs = fresh.render_offscreen(set.width, set.height, true).ok()?.iterations?;
        let differ = mine
            .iter()
            .zip(&theirs)
            .filter(|(a, b)| a[0] != ITER_UNRESOLVED && b[0] != ITER_UNRESOLVED && (a[0] < 0.0) != (b[0] < 0.0))
            .count();
        Some(FrameMismatch {
            pixels: differ,
            percent: 100.0 * differ as f64 / mine.len().max(1) as f64,
            width: set.width,
            height: set.height,
        })
    }

    /// The sample cache check (kb: techniques/sample-reuse): every sample the cache holds
    /// against a fresh offscreen render of the same lattice points (a new renderer: its own
    /// reference, the epoch's BLA and interior regime). A wrong transform, wrap, resample or
    /// seed, or a stale sample, changes escape counts in chaotic regions at once. Blocks.
    /// None without a cache epoch.
    pub fn sample_cache_check(&mut self) -> Option<CacheCheck> {
        let mut fresh = Renderer::with_config(self.backend.clone(), self.config.clone()).ok()?;
        self.sample_cache_check_with(&mut fresh)
    }
}

impl<B: GpuBackend> Renderer<B> {
    /// [`Renderer::sample_cache_check`] rendering with `fresh` (a renderer of its own on the
    /// same device; its settings are replaced).
    pub fn sample_cache_check_with(&mut self, fresh: &mut Renderer<B>) -> Option<CacheCheck> {
        self.wait_frames();
        self.drain_in_flight();
        for r in std::mem::take(&mut self.in_flight) {
            r.submission.wait();
        }
        let regime = self.sample_cache.regime()?;
        let (centre, h, angle, (w, hgt), wrap, tex) = self.sample_cache.check_frame()?;
        // (Any sample computing is finished: the queue is serial and drained above.)
        let mut mine = vec![[0f32; 2]; w * hgt];
        self.backend
            .read_texture(
                &tex,
                w,
                hgt,
                TextureFormat::Rg32Float,
                bytemuck::cast_slice_mut(&mut mine),
            )
            .ok()?;
        let limit = self.job.as_ref().map_or(self.max_iterations(), |j| j.key.max_iter);
        fresh.config.bla = if regime.bla { BlaMode::On } else { BlaMode::Off };
        fresh.job = None;
        // The live reference (used if it serves the check's view): float32 escape counts in
        // chaotic regions depend on the reference, which is not what this check is about.
        match self.reference.clone() {
            Some(r) => fresh.set_reference(r),
            None => fresh.reference = None,
        }
        fresh.viewport = Viewport::with_center(centre, h).with_angle(angle);
        fresh.settings = self.settings.clone();
        fresh.settings.auto_iterations = false;
        fresh.settings.manual_iterations = limit;
        fresh.settings.max_iterations_cap = self.settings.max_iterations_cap.max(limit);
        fresh.settings.interior_detection = regime.interior;
        fresh.settings.supersampling = almondbread_core::settings::Supersampling::Off;
        fresh.sample_cache.config.enabled = false;
        let theirs = fresh.render_offscreen(w, hgt, true).ok()?.iterations?;
        let mut check = CacheCheck::default();
        for y in 0..hgt {
            for x in 0..w {
                let s = ((y + wrap[1] as usize) % hgt) * w + (x + wrap[0] as usize) % w;
                let a = mine[s];
                let b = theirs[y * w + x];
                if a[0] == ITER_EMPTY || a[0] == ITER_PENDING_CODE {
                    continue;
                }
                check.samples += 1;
                if a[0] == ITER_UNRESOLVED || b[0] == ITER_UNRESOLVED {
                    continue;
                }
                // Escaped vs not; escape counts beyond the fresh limit are not compared.
                if (a[0] < 0.0) != (b[0] < 0.0) {
                    if !(a[0] >= limit as f32 || b[0] >= limit as f32) {
                        check.class_differ += 1;
                    }
                } else if a[0] >= 0.0 && a[0] != b[0] {
                    check.escape_differ += 1;
                }
                if a[0] >= 0.0 && b[0] >= 0.0 {
                    check.escaped += 1;
                }
            }
        }
        Some(check)
    }
}

/// [`Renderer::sample_cache_check`]: the cache's samples against a fresh render of their points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CacheCheck {
    /// Samples compared (texels holding one).
    pub samples: usize,
    /// Escaped in both.
    pub escaped: usize,
    /// Escaped in one but not the other (unresolved ones not counted).
    pub class_differ: usize,
    /// Escaped in both at different iterations.
    pub escape_differ: usize,
}

impl CacheCheck {
    pub fn class_pct(&self) -> f64 {
        100.0 * self.class_differ as f64 / self.samples.max(1) as f64
    }
    pub fn escape_pct(&self) -> f64 {
        100.0 * self.escape_differ as f64 / self.escaped.max(1) as f64
    }
}

/// Iteration texture codes of the sample cache (kernel constants).
const ITER_EMPTY: f32 = -5.0;
const ITER_PENDING_CODE: f32 = -2.0;
