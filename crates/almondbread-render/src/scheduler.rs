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
use std::time::{Duration, Instant};

use almondbread_core::autoiter::{self, InteriorStats, LevelOutcome};
use almondbread_core::input::{InputEvent, Navigator};
use almondbread_core::oracle::ITER_UNRESOLVED;
use almondbread_core::reference::DoubleReference;
use almondbread_core::settings::{RenderSettings, depth_iterations};
use almondbread_core::viewport::Viewport;
use bytemuck::bytes_of;

use crate::backend::{ComputeEncoder, GpuBackend, GpuError, Storage, Submission, SubmissionStatus, TextureFormat};
use crate::gpu_types::{ChunkStats, ColorParams, IterateParams, LEVEL_COUNTERS_SIZE, PixelState};

/// Escape radius (2^16: the smooth offset stays in a small range, kb: techniques/smooth-coloring).
pub const BAILOUT: f64 = 65536.0;
const STAT_SLOTS: usize = 64;
const PALETTE_SIZE: usize = 1024;
/// Derivative test threshold on |w| n² (kb: techniques/interior-detection, Pitfalls).
const INTERIOR_EPS: f32 = 1e-3;
const ITERATE_THREADS: usize = 256;

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
    width: usize,
    height: usize,
}

impl<B: GpuBackend> Clone for TextureSet<B> {
    fn clone(&self) -> Self {
        Self {
            iter: self.iter.clone(),
            de: self.de.clone(),
            color: self.color.clone(),
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
    width: usize,
    height: usize,
    supersampling: usize,
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
    preview: bool,
}

impl<B: GpuBackend> Level<B> {
    fn new(set: TextureSet<B>, ratio: f64, preview: bool, params: IterateParams, total: Option<usize>) -> Self {
        let total = total.unwrap_or(set.width * set.height);
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
            preview,
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
    levels: Vec<Level<B>>,
    start: f64,
    /// A gesture frame: preview only, superseded every frame.
    interactive: bool,
    presented: bool,
    full_level: Option<FullLevelTemplate<B>>,
    gpu_time: f64,
    chunks: usize,
    iterations: f64,
    failed: bool,
    /// Shown after the last pixel resolved (or the budget ran out).
    final_presented: bool,
    complete_presented: bool,
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
    /// The level to show: the furthest one that has started.
    fn display(&self) -> usize {
        self.levels.iter().rposition(|l| l.started()).unwrap_or(0)
    }
}

struct ChunkRecord<B: GpuBackend> {
    submission: B::Submission,
    generation: u64,
    level: usize,
    slot: usize,
    iterations: usize,
    /// K came from the throughput model (feeds `rate` back).
    adaptive: bool,
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

/// Values for a HUD / inspector (the Swift model's renderer-written stats).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShownStats {
    pub viewport: Option<Viewport>,
    pub max_iterations: usize,
    pub reference_time: f64,
    pub reference_length: usize,
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
    /// Refreshes that encoded nothing (frames in flight, or no drawable without blocking).
    pub skipped: usize,
    pub chunk_gpu: f64,
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
    /// Raw iteration texels (n, smooth offset), row-major, if requested.
    pub iterations: Option<Vec<[f32; 2]>>,
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
    pub texel_scale: f64,
    pub preview: bool,
    pub frames: usize,
    /// Seconds from the last input to the complete (refined) frame, and to the frame after
    /// which nothing was left to iterate.
    pub final_after: f64,
    pub done_after: f64,
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
    /// Iterate pipelines keyed by `de | interior << 1`.
    iterate_pipelines: [Option<B::Pipeline>; 4],

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

    reference: Option<DoubleReference>,
    reference_time: f64,
    reference_buffer: Option<B::Buffer>,

    palette_id: Option<String>,
    palette_buffer: Option<B::Buffer>,

    job: Option<Job<B>>,
    generation: u64,
    in_flight: VecDeque<ChunkRecord<B>>,
    /// Lane-iterations per GPU second; adapted from every chunk, persists across jobs.
    rate: f64,
    /// GPU seconds per texel of the last completed level.
    cost_per_texel: f64,
    has_presented: bool,
    /// Colorize submissions not finished on the GPU.
    frames_in_flight: Vec<B::Submission>,
    last_frame_commit: f64,
    /// When the last fully refined, full-resolution frame was committed, and its size.
    pub last_final_frame: (f64, String),
    interior_stats: Option<InteriorStats>,
    pinned_iterations: Option<(Viewport, usize)>,
    epoch: Instant,
}

impl<B: GpuBackend> Renderer<B> {
    pub fn new(backend: B) -> Result<Self, GpuError> {
        Self::with_config(backend, SchedulerConfig::default())
    }

    pub fn with_config(backend: B, config: SchedulerConfig) -> Result<Self, GpuError> {
        let prepare_pipeline = backend.pipeline("prepareChunk", &[])?;
        let finish_pipeline = backend.pipeline("finishChunk", &[])?;
        let colorize_pipeline = backend.pipeline("colorize", &[])?;
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
            iterate_pipelines: [None, None, None, None],
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
            palette_id: None,
            palette_buffer: None,
            job: None,
            generation: 0,
            in_flight: VecDeque::new(),
            rate: 3e10,
            cost_per_texel: 0.0,
            has_presented: false,
            frames_in_flight: Vec::new(),
            last_frame_commit: 0.0,
            last_final_frame: (0.0, String::new()),
            interior_stats: None,
            pinned_iterations: None,
            epoch: Instant::now(),
        })
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

    fn iterate_pipeline(&mut self, de: bool, interior: bool) -> Result<B::Pipeline, GpuError> {
        let k = usize::from(de) | (usize::from(interior) << 1);
        if let Some(p) = &self.iterate_pipelines[k] {
            return Ok(p.clone());
        }
        let p = self.backend.pipeline("iterate", &[de, interior])?;
        self.iterate_pipelines[k] = Some(p.clone());
        Ok(p)
    }

    // MARK: Interactive frames

    /// One display refresh: integrate held keys, harvest, start or keep the job, encode
    /// chunks, colorize into the target `acquire` returns (with the drawable to present, if
    /// any). `acquire` is called only when a frame will be committed; returning None (no
    /// drawable available without blocking) skips presentation this refresh.
    pub fn render_frame(
        &mut self,
        now: f64,
        width: usize,
        height: usize,
        points_per_pixel: f64,
        acquire: &mut dyn FnMut() -> Option<FrameTarget<B>>,
    ) -> FrameOutcome {
        let keep_going = self.navigator.advance(&mut self.viewport, now);
        let mut outcome = FrameOutcome {
            animating: keep_going,
            presented: false,
        };
        if width == 0 || height == 0 || points_per_pixel <= 0.0 {
            return outcome;
        }
        let interacting = self.navigator.is_interacting(now);

        // Never block on the GPU: with frames still queued, skip this refresh (the next one
        // picks up the latest input). Safety: a drawable never shown might never complete.
        self.frames_in_flight
            .retain(|s| s.status() == SubmissionStatus::Pending);
        if !self.frames_in_flight.is_empty() && now - self.last_frame_commit > 0.25 {
            self.frames_in_flight.clear();
        }
        if self.frames_in_flight.len() >= self.config.max_frames_in_flight {
            if let Some(fs) = &mut self.frame_stats {
                fs.skipped += 1;
            }
            outcome.animating = true;
            return outcome;
        }
        self.harvest();
        let key = self.make_key(width, height, points_per_pixel);
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
                self.start_job(key.clone(), JobMode::Interactive, now);
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
        let Some(job) = &self.job else {
            return outcome;
        };
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
            let budget = self.config.chunk_target;
            self.encode_chunks(budget);
        }

        // While refining, hold the previous image for a moment until the first level is
        // complete, so cheap views appear finished in one step. Gesture frames always show.
        let job = self.job.as_ref().expect("job");
        let present = (job.interactive && (!job.presented || !job.done()))
            || !self.has_presented
            || job.levels[0].complete
            || now - job.start > 0.15;
        if present && let Some((target, drawable)) = acquire() {
            if let Ok(mut enc) = self.encode_colorize(&target) {
                if let Some(d) = &drawable {
                    enc.present(d);
                }
                let job = self.job.as_ref().expect("job");
                if let Some(fs) = &mut self.frame_stats {
                    fs.frames += 1;
                    if job.levels[job.display()].preview {
                        fs.preview_frames += 1;
                    }
                }
                let sub = enc.commit();
                self.frames_in_flight.push(sub);
                self.last_frame_commit = now;
                self.has_presented = true;
                outcome.presented = true;
                let job = self.job.as_mut().expect("job");
                job.presented = true;
                if job.complete() && !job.interactive {
                    let d = &job.levels[job.display()];
                    // The refined frame: unresolved pixels may keep resolving later.
                    if !job.complete_presented && !d.preview {
                        self.last_final_frame = (now, format!("{}x{}", d.set.width, d.set.height));
                    }
                    job.complete_presented = true;
                    if job.done() {
                        job.final_presented = true;
                    }
                }
            }
        } else if present && let Some(fs) = &mut self.frame_stats {
            fs.skipped += 1;
        }
        self.publish_stats();
        let job = self.job.as_ref().expect("job");
        let busy = interacting || !job.final_presented || job.key != key;
        outcome.animating = keep_going || busy;
        outcome
    }

    fn make_key(&mut self, width: usize, height: usize, points_per_pixel: f64) -> ComputeKey {
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
        ComputeKey {
            viewport: self.viewport.clone(),
            max_iter,
            points_per_pixel,
            de: self.settings.de_needed(),
            interior: self.settings.interior_detection,
            width,
            height,
            supersampling: self.settings.supersampling.factor(),
        }
    }

    // MARK: Jobs and chunks

    fn start_job(&mut self, key: ComputeKey, mode: JobMode, now: f64) {
        let ss = key.supersampling as f64;
        let tex_w = key.width * key.supersampling;
        let tex_h = key.height * key.supersampling;
        let Ok(full) = self.full_texture_set(tex_w, tex_h) else {
            return;
        };
        self.ensure_list_capacity(tex_w * tex_h);
        let viewport = key.viewport.clone();
        let view_radius = viewport.scale * key.points_per_pixel * (key.width + key.height) as f64;
        self.prepare_reference(&viewport, key.max_iter, view_radius);
        let (Some(reference), Some(reference_buffer)) = (&self.reference, self.reference_buffer.clone()) else {
            return;
        };
        let (dre, dim) = viewport.center.delta_from(&reference.center);
        let ref_len = reference.points.len() as u32;
        let live_capacity = self.config.live_capacity as u32;
        let params = |ratio: f64, w: usize, h: usize| IterateParams {
            // Texel (0,0) is the top-left; the kernel flips y so imaginary grows upwards.
            half_size: [w as f32 / 2.0, h as f32 / 2.0],
            ref_offset: [dre as f32, dim as f32],
            jitter: [0.5, 0.5],
            texel_scale: (viewport.scale * key.points_per_pixel / ratio) as f32,
            bailout2: (BAILOUT * BAILOUT) as f32,
            max_iter: key.max_iter as u32,
            ref_len,
            width: w as u32,
            total: (w * h) as u32,
            capacity: live_capacity,
            interior_eps: INTERIOR_EPS,
            soft_iter: key.max_iter as u32,
            ..IterateParams::default()
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
                // cost fits the gesture frame budget.
                let mut ratio = 0.5;
                if self.cost_per_texel > 0.0 {
                    let fit = (self.config.interactive_frame_budget
                        / (self.cost_per_texel * (key.width * key.height) as f64))
                        .sqrt();
                    ratio = ((fit * 8.0).floor() / 8.0).clamp(0.25, 1.0);
                }
                if !self.settings.interactive_preview {
                    ratio = ss;
                }
                if ratio >= ss {
                    levels.push(full_level(&full));
                } else {
                    let (w, h) = level_size(ratio);
                    if let Ok(set) = self.preview_texture_set(key.width, key.height, w, h) {
                        levels.push(Level::new(set, ratio, true, params(ratio, w, h), None));
                    }
                }
            }
            JobMode::Refine => {
                // Preview first when the full level is expected to take more than about a
                // frame; quarter resolution if even half would.
                let full_cost = self.cost_per_texel * full_texels;
                if self.settings.interactive_preview && full_cost > 1.0 / 90.0 {
                    let ratio = if full_cost / ss / ss * 0.25 <= 1.0 / 60.0 {
                        0.5
                    } else {
                        0.25
                    };
                    let (w, h) = level_size(ratio);
                    if let Ok(set) = self.preview_texture_set(key.width, key.height, w, h) {
                        levels.push(Level::new(set, ratio, true, params(ratio, w, h), None));
                    }
                }
                levels.push(full_level(&full));
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
            levels,
            start: now,
            interactive: mode == JobMode::Interactive,
            presented: false,
            full_level: Some(template),
            gpu_time: 0.0,
            chunks: 0,
            iterations: 0.0,
            failed: false,
            final_presented: false,
            complete_presented: false,
        });
    }

    /// Turns a finished gesture frame into a refinement job: same view, the full level added
    /// after the preview, which stays visible underneath while the full level fills in.
    fn extend_for_refinement(&mut self) {
        let Some(job) = &mut self.job else { return };
        job.interactive = false;
        if !job.levels.iter().any(|l| !l.preview)
            && let Some(t) = &job.full_level
        {
            let mut l = Level::new(t.set.clone(), t.ratio, false, t.params, None);
            l.extend_to = t.extend_to;
            job.levels.push(l);
            job.final_presented = false;
        }
    }

    /// A gesture frame: the preview level in ceil(texels / capacity) chunks with K = maxIter,
    /// so it is complete when the frame's colorize runs. If the measured cost says that would
    /// take long, fall back to budgeted progressive chunks.
    fn encode_interactive(&mut self) {
        let Some(job) = &self.job else { return };
        let Some(level) = job.levels.first() else { return };
        if level.encoded != 0 {
            if !level.done {
                let b = self.config.interactive_chunk_target;
                self.encode_chunks(b);
            }
            return;
        }
        let estimate = self.cost_per_texel * level.total as f64;
        if estimate > 0.05 {
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
            let max = self.config.max_chunks_in_flight;
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
            let level = &mut job.levels[level_index];
            if !level.started() && !level.is_extension() {
                level.use_interior = autoiter::use_interior_test(interior_enabled, stats, key_max_iter);
                // Collect unresolved pixels for extension passes.
                if level.use_interior && level.extend_to > level.params.soft_iter as usize {
                    level.params.list_mode = 1;
                }
            }
        }
        let use_interior = self.job.as_ref().expect("job").levels[level_index].use_interior;
        let pipeline = match self.iterate_pipeline(de, use_interior) {
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
        match &self.list_buffers {
            Some(lists) => {
                enc.set_buffer(10, &lists[level.list_in], 0);
                enc.set_buffer(12, &lists[level.list_out], 0);
            }
            None => {
                enc.set_buffer(10, &self.list_count_buffer, 0);
                enc.set_buffer(12, &self.list_count_buffer, 0);
            }
        }
        enc.set_buffer(11, &self.list_count_buffer, 0);
        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);

        enc.set_pipeline(&pipeline);
        enc.set_texture(0, &level.set.iter);
        enc.set_texture(1, &level.set.de);
        enc.set_buffer(1, &job.reference_buffer, 0);
        enc.set_buffer(2, &self.state_buffers[src], 0);
        enc.set_buffer(3, &self.state_buffers[dst], 0);
        enc.dispatch_indirect(&self.args_buffer, 0, [ITERATE_THREADS, 1, 1]);

        enc.set_pipeline(&self.finish_pipeline);
        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
        let submission = enc.commit();

        let generation = job.generation;
        self.ping = dst;
        self.job.as_mut().expect("job").levels[level_index].encoded += 1;
        self.in_flight.push_back(ChunkRecord {
            submission,
            generation,
            level: level_index,
            slot: self.stat_slot,
            iterations: k,
            adaptive,
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
            if self.config.trace {
                let use_interior = self
                    .job
                    .as_ref()
                    .filter(|j| j.generation == rec.generation)
                    .map(|j| j.levels[rec.level].use_interior)
                    .unwrap_or(false);
                println!(
                    "chunk gen {} slot {:2}: liveIn {:8} fresh {:8} K {:8} -> liveOut {:8}  iters {}  interior {} (test {})  gpu {:.2} ms  rate {}",
                    rec.generation,
                    rec.slot,
                    stats.live_in,
                    stats.fresh,
                    rec.iterations,
                    stats.live_out,
                    almondbread_core::cfmt::g(stats.iterations() as f64, 3),
                    stats.interior,
                    if use_interior { "on" } else { "off" },
                    gpu * 1000.0,
                    almondbread_core::cfmt::g(self.rate, 3)
                );
            }
            if let Some(fs) = &mut self.frame_stats {
                fs.chunk_gpu += gpu;
            }
            let Some(job) = &mut self.job else { continue };
            if job.generation != rec.generation {
                continue;
            }
            job.gpu_time += gpu;
            job.chunks += 1;
            job.iterations += stats.iterations() as f64;
            let level = &mut job.levels[rec.level];
            level.gpu_time += gpu;
            level.iterations += stats.iterations() as f64;
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
        if level.gpu_time > 0.0 {
            self.cost_per_texel = level.gpu_time / level.total as f64;
        }
        self.interior_stats = Some(InteriorStats::from_level(
            level.total,
            level.interior_pixels,
            level.iterations,
            level.use_interior,
            level.params.max_iter as usize,
        ));
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
        if level.preview {
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
                let mut ext = Level::new(level.set.clone(), level.ratio, false, p, Some(count));
                ext.use_interior = true;
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
        };
        let old = self.settings.learned_iterations;
        let learned = autoiter::learn_iterations(
            old,
            &outcome,
            self.settings.max_iterations_cap,
            depth_iterations(&self.viewport),
        );
        if self.config.trace {
            println!(
                "learn: soft {} total {} crossed {} escapedPast {:?} maxEscape {} unresolved {} -> {}",
                outcome.soft_iter,
                level.total,
                level.crossed,
                level.escaped_past,
                level.max_escape,
                level.live,
                learned
            );
        }
        if learned == old {
            return;
        }
        self.pinned_iterations = Some((job.key.viewport.clone(), job.key.max_iter));
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
            render_size: (shown.set.width, shown.set.height),
            is_preview: shown.preview,
            gpu_time: job.gpu_time,
            chunks: job.chunks,
            is_rendering: !job.done(),
        };
    }

    // MARK: Colorize

    /// Colorize into `target` (its whole size). A preview level is first coloured at its own
    /// resolution into its colour texture (linear light), which the final pass samples
    /// bilinearly wherever the shown level is unfinished, or everywhere when the preview
    /// itself is shown.
    fn encode_colorize(&mut self, target: &B::Texture) -> Result<B::Encoder, GpuError> {
        self.ensure_palette()?;
        let job = self.job.as_ref().expect("job");
        let shown_index = job.display();
        let shown = &job.levels[shown_index];
        let preview = if shown.preview {
            Some(shown)
        } else {
            job.levels.iter().find(|l| l.preview && l.done)
        };
        let mut enc = self.backend.begin("colorize")?;
        enc.set_pipeline(&self.colorize_pipeline);
        enc.set_buffer(1, self.palette_buffer.as_ref().expect("palette"), 0);
        enc.set_buffer(2, &self.level_buffer, 0);
        let (tw, max_threads) = self.backend.pipeline_limits(&self.colorize_pipeline);
        let th = (max_threads / tw / 2).max(1);
        let base = self.color_params();
        let (target_w, target_h) = self.backend.texture_size(target);

        let pass = |enc: &mut B::Encoder,
                    level: Option<&Level<B>>,
                    out: &B::Texture,
                    out_size: (usize, usize),
                    ratio: f64,
                    under: Option<(&B::Texture, (usize, usize))>,
                    linear: bool| {
            let mut c = base;
            c.ratio = ratio as f32;
            if let Some(level) = level {
                // The running level's admission count lives on the GPU, ahead of the CPU's.
                c.admitted = if level.done { level.total as u32 } else { 0 };
                c.admitted_on_gpu = u32::from(level.started() && !level.done);
                if level.is_extension() {
                    // Every texel is written: the extension only replaces unresolved ones.
                    c.admitted = (level.set.width * level.set.height) as u32;
                    c.admitted_on_gpu = 0;
                }
                c.de_scale = level.ratio as f32;
            }
            c.has_under = if under.is_some() { 1.0 } else { 0.0 };
            c.linear_out = u32::from(linear);
            let src = &level.unwrap_or(shown).set;
            c.src_size = [src.width as u32, src.height as u32];
            c.out_size = [out_size.0 as u32, out_size.1 as u32];
            if let Some((_, (uw, uh))) = under {
                c.under_size = [uw as f32, uh as f32];
            }
            enc.set_texture(0, &src.iter);
            enc.set_texture(1, &src.de);
            enc.set_texture(2, out);
            enc.set_texture(3, under.map_or(out, |u| u.0));
            enc.set_bytes(0, bytes_of(&c));
            enc.dispatch_threads([out_size.0, out_size.1, 1], [tw, th, 1]);
        };
        let mut under = None;
        if let Some(p) = preview
            && let Some(color) = &p.set.color
        {
            pass(&mut enc, Some(p), color, (p.set.width, p.set.height), 1.0, None, true);
            under = Some((color, (p.set.width, p.set.height)));
        }
        // Showing the preview: no main level (nothing "finished"), every pixel samples it.
        pass(
            &mut enc,
            if shown.preview { None } else { Some(shown) },
            target,
            (target_w, target_h),
            shown.ratio,
            under,
            false,
        );
        Ok(enc)
    }

    fn color_params(&self) -> ColorParams {
        let s = &self.settings;
        let a = s.light_angle * std::f64::consts::PI / 180.0;
        let i = s.interior_color;
        ColorParams {
            interior: [i[0], i[1], i[2], 1.0],
            edge: [0.0, 0.0, 0.0, 1.0],
            light: [a.cos() as f32, a.sin() as f32],
            density: s.color_density as f32,
            offset: s.color_offset as f32,
            ratio: 1.0,
            shading: s.shading as f32,
            outline: s.outline as f32,
            outline_width: s.outline_width as f32,
            palette_size: PALETTE_SIZE as u32,
            log_scale: u32::from(s.log_coloring),
            smooth: u32::from(s.smooth_coloring),
            de_scale: 1.0,
            ..ColorParams::default()
        }
    }

    fn ensure_palette(&mut self) -> Result<(), GpuError> {
        if self.palette_id.as_deref() == Some(self.settings.palette_id.as_str()) && self.palette_buffer.is_some() {
            return Ok(());
        }
        let table = self.settings.palette().table(PALETTE_SIZE);
        self.palette_buffer = Some(self.backend.create_buffer_with_data(bytemuck::cast_slice(&table))?);
        self.palette_id = Some(self.settings.palette_id.clone());
        Ok(())
    }

    /// The full-level textures, reused while the size holds.
    fn full_texture_set(&mut self, width: usize, height: usize) -> Result<TextureSet<B>, GpuError> {
        if let Some(s) = &self.full_set
            && s.width == width
            && s.height == height
        {
            return Ok(s.clone());
        }
        let b = &self.backend;
        let s = TextureSet {
            iter: b.create_texture(width, height, TextureFormat::Rg32Float, Storage::Private)?,
            de: b.create_texture(width, height, TextureFormat::Rg16Float, Storage::Private)?,
            color: None,
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
        if let Some(s) = &self.preview_alloc {
            let (aw, ah) = self.backend.texture_size(&s.iter);
            if aw >= width && ah >= height {
                return Ok(s.with_size(width, height));
            }
        }
        let (aw, ah) = (max_w.max(width), max_h.max(height));
        let b = &self.backend;
        let s = TextureSet {
            iter: b.create_texture(aw, ah, TextureFormat::Rg32Float, Storage::Private)?,
            de: b.create_texture(aw, ah, TextureFormat::Rg16Float, Storage::Private)?,
            color: Some(b.create_texture(aw, ah, TextureFormat::Rgba16Float, Storage::Private)?),
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
    /// reuses it as panning and zooming do (`verify --ref-offset`).
    pub fn prime_reference(&mut self, viewport: &Viewport, max_iter: usize) {
        self.reference = None;
        self.prepare_reference(viewport, max_iter, 0.0);
    }

    /// Reuses the current reference while it serves the view (kb: techniques/rebasing);
    /// in-flight chunks keep the old buffer alive.
    fn prepare_reference(&mut self, viewport: &Viewport, max_iter: usize, view_radius: f64) {
        if self
            .reference
            .as_ref()
            .is_some_and(|r| r.serves(viewport, max_iter, view_radius))
        {
            return;
        }
        let start = Instant::now();
        let r = DoubleReference::compute(viewport, max_iter, BAILOUT);
        match self.backend.create_buffer_with_data(bytemuck::cast_slice(&r.points)) {
            Ok(buf) => {
                self.reference_buffer = Some(buf);
                self.reference = Some(r);
            }
            Err(e) => {
                eprintln!("almondbread: reference buffer: {e}");
                self.reference = None;
                self.reference_buffer = None;
            }
        }
        self.reference_time = start.elapsed().as_secs_f64();
    }

    /// Reference orbit time and length of the last view (for reports).
    pub fn reference_info(&self) -> (f64, usize) {
        (
            self.reference_time,
            self.reference.as_ref().map_or(0, |r| r.points.len()),
        )
    }

    fn drain_in_flight(&mut self) {
        if let Some(r) = self.in_flight.back() {
            r.submission.wait();
        }
        self.harvest();
    }

    fn wait_frames(&mut self) {
        for f in &self.frames_in_flight {
            f.wait();
        }
        self.frames_in_flight.clear();
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
        while !finished && attempts < 4 {
            attempts += 1;
            self.drain_in_flight();
            self.job = None;
            let now = self.clock();
            let key = self.make_key(width, height, 1.0);
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
                        None => {
                            self.job.as_mut().expect("job").failed = true;
                            break;
                        }
                    }
                }
            }
            finished = !self.job.as_ref().expect("job").failed;
        }
        if !finished {
            return Err(GpuError("offscreen render failed".into()));
        }
        let enc = self.encode_colorize(&target)?;
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
            iterations,
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
            r.render_frame(now, width, height, points_per_pixel, &mut || Some((t.clone(), None)));
            *frames += 1;
            *tick += interval;
            let left = *tick - r.clock();
            if left > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(left));
            }
        };
        let mut prev = self.clock();
        for i in 0..input_frames {
            let now = self.clock();
            self.navigator.note_interaction(now);
            let dt = if i == 0 { interval } else { now - prev };
            input(i, dt, &mut self.viewport);
            prev = now;
            refresh(self, &mut tick, &mut frames);
        }
        let input_end = self.navigator.last_interaction;
        let deadline = self.clock() + timeout;
        loop {
            let finished = self
                .job
                .as_ref()
                .is_some_and(|j| j.final_presented && !j.interactive && j.key.viewport == self.viewport);
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
            frames,
            final_after,
            done_after,
            unresolved,
            reference_length: shown.params.ref_len as usize,
            iterations,
        })
    }
}
