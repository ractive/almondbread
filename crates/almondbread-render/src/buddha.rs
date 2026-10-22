//! Buddhabrot and Nebulabrot on the GPU (milestone BB, kb: plans/buddhabrot). The kernels are a
//! library of their own (`shaders/buddhabrot.metal`, compiled when the look is first shown), so
//! nothing here touches the escape-time pipelines.
//!
//! Two accumulations:
//!
//! - **Rest**: a `u32` histogram per channel at the output size for the resting view. Sample
//!   dispatches (uniform, or Metropolis–Hastings when few uniform samples hit the view, with
//!   chains carried between views by recheck and resampling, and warmed up through larger views
//!   when they come from elsewhere) add to
//!   it in command buffers of about `chunk_target` GPU time until the view is clean; any change
//!   of the view, size or mode clears it. The tone pass normalises by the epoch's hit totals,
//!   which the GPU sums itself (`buddhaTotals`), so the shown image never waits for a readback.
//! - **Live** (motion): a coarse histogram per frame, folded into a float accumulation that is
//!   reprojected into each new view and decays (`buddhaFold`), within the gesture's GPU budget.
//!   Noisy but moving; shown until the rest histogram of the final view is as clean.
//!
//! Throughput model: GPU seconds per walker iteration, measured per sampler (uniform / MH) and
//! look, sets the iterations per thread of the next dispatch.

use std::collections::VecDeque;

use almondbread_core::buddha::{self, BinMap, BuddhaMode, REGION_MIN, REGION_SIZE};
use bytemuck::{Pod, Zeroable, bytes_of};

use crate::backend::{ComputeEncoder, GpuBackend, GpuError, MathMode, Storage, Submission, SubmissionStatus};

pub const BUDDHA_SOURCE: &str = include_str!("shaders/buddhabrot.metal");
const FUNCTIONS: [&str; 10] = [
    "buddhaLevels",
    "buddhaSample",
    "buddhaRecheck",
    "buddhaResample",
    "buddhaClear",
    "buddhaTotals",
    "buddhaFold",
    "buddhaSeedLive",
    "buddhaTone",
    "buddhaLayout",
];

/// u32 counters per dispatch slot (the kernel's `COUNTERS`), and slots in the ring.
pub const COUNTERS: usize = 16;
const SLOTS: usize = 64;
/// Counter indices.
const C_PROPOSALS: usize = 0;
const C_UNIFORM: usize = 1;
const C_UNIFORM_S: usize = 2;
const C_STEPS: usize = 3;
const C_HITS: usize = 4;
const C_ACCEPTED: usize = 7;
const C_ITS_LO: usize = 8;
const C_ITS_HI: usize = 9;
const C_ESCAPED: usize = 10;

/// `BuddhaParams` in the kernel, 112 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct BuddhaParams {
    pub limits: [u32; 4],
    pub center: [f32; 2],
    pub rot: [f32; 2],
    pub region_min: [f32; 2],
    pub region_size: [f32; 2],
    pub inv_pixel: f32,
    pub p_jump: f32,
    pub r_max: f32,
    pub r_span: f32,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub steps: u32,
    pub seed: u32,
    pub mode: u32,
    pub generation: u32,
    pub slot: u32,
    pub threads: u32,
    pub deposit: u32,
    pub drain: u32,
    pub pad: u32,
}

/// `FoldParams`, 56 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FoldParams {
    pub origin: [f32; 2],
    pub step_x: [f32; 2],
    pub step_y: [f32; 2],
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub decay: f32,
    pub area_ratio: f32,
    pub rho: f32,
    pub reproject: u32,
    pub slot: u32,
}

/// `ToneParams`, 96 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ToneParams {
    pub scale: [f32; 4],
    pub white: [f32; 4],
    pub black: [f32; 4],
    pub src_width: u32,
    pub src_height: u32,
    pub out_width: u32,
    pub out_height: u32,
    pub channels: u32,
    pub source: u32,
    pub palette_size: u32,
    pub gamma: f32,
    pub palette_peak: f32,
    pub pad: [u32; 3],
}

/// `SeedLiveParams`, 40 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SeedLiveParams {
    pub div: u32,
    pub rest_width: u32,
    pub rest_height: u32,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub samples: f32,
    pub off_x: i32,
    pub off_y: i32,
    pub pad: u32,
}

const CHAIN_BYTES: usize = 104;
const _: () = assert!(size_of::<BuddhaParams>() == 112);
const _: () = assert!(size_of::<FoldParams>() == 56);
const _: () = assert!(size_of::<ToneParams>() == 96);

/// Sampler kinds (the kernel's `mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sampler {
    Uniform = 0,
    Metropolis = 1,
}

/// Below this share of uniform proposals whose orbit hits the view, the rest accumulation
/// switches to Metropolis–Hastings (measured: kb: plans/buddhabrot "As built").
pub const MH_BELOW: f64 = 0.05;
/// MH: probability of a uniform proposal while the chain is in S.
const P_JUMP: f32 = 0.05;
/// MH mutation radii: from the view's diagonal down 2^-R_SPAN of it.
const R_SPAN: f32 = 12.0;
const R_MAX: f32 = 0.5;
/// Parallel chains / threads per dispatch.
pub const THREADS: usize = 1 << 16;

/// Totals of one or more dispatches.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counters {
    pub proposals: u64,
    pub uniform: u64,
    pub uniform_in_view: u64,
    pub steps: u64,
    pub hits: [u64; 3],
    pub accepted: u64,
    pub iterations: u64,
    pub escaped: u64,
    pub gpu_time: f64,
}

impl Counters {
    fn from_slot(v: &[u32]) -> Self {
        Self {
            proposals: u64::from(v[C_PROPOSALS]),
            uniform: u64::from(v[C_UNIFORM]),
            uniform_in_view: u64::from(v[C_UNIFORM_S]),
            steps: u64::from(v[C_STEPS]),
            hits: [u64::from(v[C_HITS]), u64::from(v[C_HITS + 1]), u64::from(v[C_HITS + 2])],
            accepted: u64::from(v[C_ACCEPTED]),
            iterations: u64::from(v[C_ITS_LO]) | u64::from(v[C_ITS_HI]) << 32,
            escaped: u64::from(v[C_ESCAPED]),
            gpu_time: 0.0,
        }
    }

    pub fn add(&mut self, o: &Counters) {
        self.proposals += o.proposals;
        self.uniform += o.uniform;
        self.uniform_in_view += o.uniform_in_view;
        self.steps += o.steps;
        for k in 0..3 {
            self.hits[k] += o.hits[k];
        }
        self.accepted += o.accepted;
        self.iterations += o.iterations;
        self.escaped += o.escaped;
        self.gpu_time += o.gpu_time;
    }

    /// Share of uniform proposals whose orbit hits the view (None: too few to tell).
    pub fn rho(&self) -> Option<f64> {
        (self.uniform_in_view >= 16).then(|| self.uniform_in_view as f64 / self.uniform as f64)
    }
}

/// The compiled kernels.
pub struct BuddhaKernels<B: GpuBackend> {
    levels: B::Pipeline,
    sample: B::Pipeline,
    recheck: B::Pipeline,
    resample: B::Pipeline,
    clear: B::Pipeline,
    totals: B::Pipeline,
    fold: B::Pipeline,
    seed_live: B::Pipeline,
    tone: B::Pipeline,
    layout: B::Pipeline,
}

impl<B: GpuBackend> Clone for BuddhaKernels<B> {
    fn clone(&self) -> Self {
        Self {
            levels: self.levels.clone(),
            sample: self.sample.clone(),
            recheck: self.recheck.clone(),
            resample: self.resample.clone(),
            clear: self.clear.clone(),
            totals: self.totals.clone(),
            fold: self.fold.clone(),
            seed_live: self.seed_live.clone(),
            tone: self.tone.clone(),
            layout: self.layout.clone(),
        }
    }
}

impl<B: GpuBackend> BuddhaKernels<B> {
    pub fn compile(backend: &B) -> Result<Self, GpuError> {
        let mut p = backend
            .library_pipelines(BUDDHA_SOURCE, &FUNCTIONS, MathMode::Fast)?
            .into_iter();
        let mut next = || p.next().expect("one pipeline per function");
        Ok(Self {
            levels: next(),
            sample: next(),
            recheck: next(),
            resample: next(),
            clear: next(),
            totals: next(),
            fold: next(),
            seed_live: next(),
            tone: next(),
            layout: next(),
        })
    }
}

/// What an accumulation is for: the view's bins and the mode.
#[derive(Clone, Debug, PartialEq)]
pub struct HistKey {
    pub map: BinMap,
    pub mode: BuddhaMode,
}

/// The rest histogram.
pub struct Rest<B: GpuBackend> {
    pub key: HistKey,
    hist: B::Buffer,
    /// Epoch id: dispatches of an older epoch do not count here.
    epoch: u64,
    /// Completed dispatches' totals.
    pub done: Counters,
    /// The sampler of the next dispatch.
    sampler: Sampler,
    /// Walker iterations encoded so far (completed or not).
    pub encoded: u64,
    /// MH warm-up (a small view the chains did not come from): views 4x, 16x, ... larger
    /// around it, largest first, sampled without deposits for [`WARM_DISPATCHES`] each, so
    /// the chains reach the view's S from states uniform over a larger view's S (which
    /// contains it) instead of by uniform search.
    pub warm: Vec<BinMap>,
    warm_done: u32,
}

/// Dispatches per warm-up stage.
pub const WARM_DISPATCHES: u32 = 2;
/// Warm-up starts from the view enlarged until it covers this share of the region.
const WARM_SHARE: f64 = 1e-2;

impl<B: GpuBackend> Rest<B> {
    /// The sampler of the next dispatch.
    pub fn sampler(&self) -> Sampler {
        self.sampler
    }

    /// Uniform-equivalent samples behind the completed part.
    pub fn samples(&self) -> f64 {
        uniform_equivalent(&self.done)
    }

    /// Mean hits per bin of the faintest channel (the progress measure).
    pub fn mean_hits(&self) -> f64 {
        let ch = self.key.mode.channels().max(1);
        let bins = (self.key.map.width * self.key.map.height).max(1) as f64;
        (0..ch).map(|k| self.done.hits[k]).min().unwrap_or(0) as f64 / bins
    }
}

/// Uniform-equivalent samples of `c` behind counters (MH steps over the share of uniform
/// proposals in S).
pub fn uniform_equivalent(c: &Counters) -> f64 {
    let mh = c.steps.saturating_sub(c.uniform.min(c.steps));
    if c.steps == c.uniform || mh == 0 {
        return c.uniform as f64;
    }
    match c.rho() {
        Some(r) if r > 0.0 => c.steps as f64 / r,
        _ => c.uniform as f64,
    }
}

/// The live accumulation (motion).
pub struct Live<B: GpuBackend> {
    pub key: Option<HistKey>,
    frame: B::Buffer,
    acc: [B::Buffer; 2],
    ping: usize,
    pub width: usize,
    pub height: usize,
    pub mode: BuddhaMode,
    /// Mean density per channel (hits per uniform sample per bin), smoothed over frames.
    pub mean: [f64; 3],
    /// Share of uniform proposals in S, smoothed.
    pub rho: f64,
    /// Effective hits per bin of the accumulation (decayed), the faintest channel.
    pub hits_per_bin: f64,
    sampler: Sampler,
    pub frames: u64,
}

struct Pending<B: GpuBackend> {
    sub: B::Submission,
    slot: usize,
    epoch: u64,
    live: bool,
    sampler: Sampler,
    /// Walker iterations of the dispatch.
    iterations: u64,
}

/// The sampler state: kernels, chains, counters, the accumulations and the cost model.
pub struct Buddha<B: GpuBackend> {
    k: BuddhaKernels<B>,
    counters: B::Buffer,
    /// Rest epoch totals (64-bit per counter) the tone pass normalises by.
    totals: B::Buffer,
    chains: B::Buffer,
    /// The chains' states as the last recheck left them (what resampling copies from).
    snap: B::Buffer,
    /// The view the walkers were last started or rechecked for (rest histograms).
    chain_map: Option<BinMap>,
    generation: u32,
    last_map: Option<BinMap>,
    seed: u32,
    next_slot: usize,
    next_epoch: u64,
    in_flight: VecDeque<Pending<B>>,
    pub rest: Option<Rest<B>>,
    pub live: Option<Live<B>>,
    /// GPU seconds per proposal by sampler (smoothed), for the current mode.
    cost: [Option<f64>; 2],
    cost_mode: BuddhaMode,
    palette: Option<(String, B::Buffer, f32)>,
    /// Count only, no histogram writes (contention measurement).
    pub deposit: bool,
    /// Last harvested live frame's counters.
    pub last_live: Counters,
    /// Use this sampler for the rest histogram regardless of the measured rho (tests).
    pub force_sampler: Option<Sampler>,
    /// The live frame sampled but not folded yet: its counter slot and sampler.
    live_slot: Option<(usize, Sampler)>,
    /// MH tunables (measurements): uniform-jump probability, mutation radius span (log2) and
    /// largest radius as a share of the view's diagonal.
    pub p_jump: f32,
    pub r_span: f32,
    pub r_max: f32,
    /// The look the walkers last sampled a rest histogram of.
    rest_look: Option<BuddhaMode>,
    /// The last rest view (does it enclose the next one?).
    last_rest_map: Option<BinMap>,
    /// A fixed tone curve (white, gamma, black; measurements), else the measured levels.
    pub fixed_tone: Option<(f32, f32, f32)>,
    /// The rest histogram's black and white points (in view means), measured on the GPU
    /// ([`Buddha::request_levels`]); the live mode uses them too.
    pub levels: Option<Levels>,
    levels_buf: B::Buffer,
    levels_pending: Option<(B::Submission, BuddhaMode, usize)>,
    /// Counts measured levels (a new measurement re-tones the shown image).
    pub levels_serial: u64,
    /// GPU seconds of all harvested sample dispatches (frame statistics).
    pub harvested_gpu: f64,
}

impl<B: GpuBackend> Buddha<B> {
    /// Another random stream (two runs to compare).
    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }

    pub fn new(backend: &B) -> Result<Self, GpuError> {
        Self::with_kernels(backend, BuddhaKernels::compile(backend)?)
    }

    /// Another sampler with the same kernels (no compile): an export next to the view's.
    pub fn fresh(&self, backend: &B) -> Result<Self, GpuError> {
        Self::with_kernels(backend, self.k.clone())
    }

    fn with_kernels(backend: &B, k: BuddhaKernels<B>) -> Result<Self, GpuError> {
        let counters = backend.create_buffer(SLOTS * COUNTERS * 4, Storage::Shared)?;
        let totals = backend.create_buffer(COUNTERS * 8, Storage::Shared)?;
        let chains = backend.create_buffer_with_data(&vec![0u8; THREADS * CHAIN_BYTES])?;
        let snap = backend.create_buffer(THREADS * 16, Storage::Private)?;
        Ok(Self {
            k,
            counters,
            totals,
            chains,
            snap,
            chain_map: None,
            generation: 1,
            last_map: None,
            seed: 0x5EED,
            next_slot: 0,
            next_epoch: 1,
            in_flight: VecDeque::new(),
            rest: None,
            live: None,
            cost: [None; 2],
            cost_mode: BuddhaMode::Off,
            palette: None,
            deposit: true,
            last_live: Counters::default(),
            force_sampler: None,
            live_slot: None,
            p_jump: P_JUMP,
            r_span: R_SPAN,
            r_max: R_MAX,
            rest_look: None,
            last_rest_map: None,
            fixed_tone: None,
            levels: None,
            levels_buf: backend.create_buffer(LEVELS_WORDS * 4, Storage::Shared)?,
            levels_pending: None,
            levels_serial: 0,
            harvested_gpu: 0.0,
        })
    }

    /// The structs' sizes as the compiler lays them out: BuddhaParams, Chain, FoldParams,
    /// ToneParams, SeedLiveParams.
    pub fn kernel_layout(&self, backend: &B) -> Result<[u32; 6], GpuError> {
        let out = backend.create_buffer(24, Storage::Shared)?;
        let mut enc = backend.begin("buddha layout")?;
        enc.set_pipeline(&self.k.layout);
        enc.set_buffer(0, &out, 0);
        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
        enc.commit().wait();
        let mut b = [0u8; 24];
        backend.read_buffer(&out, 0, &mut b);
        Ok(bytemuck::cast(b))
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// Waits for every sample dispatch.
    pub fn wait_all(&mut self, backend: &B) {
        if let Some(p) = self.in_flight.back() {
            p.sub.wait();
        }
        self.harvest(backend);
    }

    /// Reads the counters of finished dispatches into the rest totals, the live means and the
    /// cost model.
    pub fn harvest(&mut self, backend: &B) {
        self.poll_levels(backend, false);
        while let Some(p) = self.in_flight.front() {
            match p.sub.status() {
                SubmissionStatus::Pending => break,
                SubmissionStatus::Failed(e) => {
                    eprintln!("almondbread: buddhabrot dispatch failed: {e}");
                    self.cost = [None; 2];
                    self.in_flight.pop_front();
                    continue;
                }
                SubmissionStatus::Completed => {}
            }
            let p = self.in_flight.pop_front().expect("front");
            let mut raw = [0u8; COUNTERS * 4];
            backend.read_buffer(&self.counters, p.slot * COUNTERS * 4, &mut raw);
            let v: [u32; COUNTERS] = bytemuck::cast(raw);
            let mut c = Counters::from_slot(&v);
            c.gpu_time = p.sub.gpu_time();
            self.harvested_gpu += c.gpu_time;
            if p.iterations > 0 && c.gpu_time > 0.0 {
                let per = c.gpu_time / p.iterations as f64;
                let e = &mut self.cost[p.sampler as usize];
                *e = Some(e.map_or(per, |o| 0.6 * o + 0.4 * per));
            }
            if p.live {
                if let Some(l) = &mut self.live {
                    let bins = (l.width * l.height) as f64;
                    if let Some(r) = c.rho() {
                        l.rho = if l.frames == 0 { r } else { 0.7 * l.rho + 0.3 * r };
                    }
                    let n = if p.sampler == Sampler::Uniform {
                        c.uniform as f64
                    } else {
                        c.steps as f64 / l.rho.max(1e-9)
                    };
                    if n > 0.0 {
                        for k in 0..3 {
                            let m = c.hits[k] as f64 / (n * bins);
                            l.mean[k] = if l.frames == 0 || l.mean[k] <= 0.0 {
                                m
                            } else {
                                0.7 * l.mean[k] + 0.3 * m
                            };
                        }
                    }
                    l.frames += 1;
                    // Uniform proposals rarely hit a small view: Metropolis–Hastings then.
                    l.sampler = if l.rho < MH_BELOW {
                        Sampler::Metropolis
                    } else {
                        Sampler::Uniform
                    };
                }
                self.last_live = c;
            } else if let Some(r) = &mut self.rest
                && r.epoch == p.epoch
            {
                r.done.add(&c);
                if let Some(f) = self.force_sampler {
                    r.sampler = f;
                } else if let Some(rho) = r.done.rho() {
                    r.sampler = if rho < MH_BELOW {
                        Sampler::Metropolis
                    } else {
                        Sampler::Uniform
                    };
                }
            }
        }
    }

    /// Measures the rest histogram's levels on the GPU (one at a time; read in
    /// [`Buddha::harvest`]).
    pub fn request_levels(&mut self, backend: &B) -> Result<(), GpuError> {
        if self.levels_pending.is_some() {
            return Ok(());
        }
        let Some(r) = &self.rest else { return Ok(()) };
        if r.done.proposals == 0 {
            return Ok(());
        }
        let m = r.key.map;
        let t = self.tone_params(r.key.mode, (m.width, m.height), (m.width, m.height), 0, [0.0; 3]);
        let bins = m.width * m.height;
        let mut enc = backend.begin("buddha levels")?;
        self.clear(&mut enc, &self.levels_buf, 0, LEVELS_WORDS);
        enc.set_pipeline(&self.k.levels);
        enc.set_bytes(0, bytes_of(&t));
        enc.set_buffer(1, &r.hist, 0);
        enc.set_buffer(4, &self.totals, 0);
        enc.set_buffer(5, &self.levels_buf, 0);
        enc.dispatch_threads([bins, 1, 1], [256, 1, 1]);
        let sub = enc.commit();
        self.levels_pending = Some((sub, r.key.mode, bins));
        Ok(())
    }

    /// A levels measurement is on the GPU.
    pub fn levels_pending(&self) -> bool {
        self.levels_pending.is_some()
    }

    /// Reads finished levels (blocking with `wait`).
    pub fn poll_levels(&mut self, backend: &B, wait: bool) {
        let Some((sub, mode, bins)) = &self.levels_pending else {
            return;
        };
        if wait {
            sub.wait();
        }
        if sub.status() == SubmissionStatus::Pending {
            return;
        }
        let (mode, bins) = (*mode, *bins);
        self.levels_pending = None;
        let mut raw = vec![0u8; LEVELS_WORDS * 4];
        backend.read_buffer(&self.levels_buf, 0, &mut raw);
        let words: &[u32] = bytemuck::cast_slice(&raw);
        if let Some(l) = Levels::from_buckets(mode, words, bins) {
            self.levels = Some(l);
            self.levels_serial += 1;
        }
    }

    /// GPU seconds per proposal of `sampler` for `mode` (None: not measured yet).
    pub fn cost(&self, sampler: Sampler) -> Option<f64> {
        self.cost[sampler as usize]
    }

    /// Iterations per walker for `budget` GPU seconds.
    fn steps_for(&self, sampler: Sampler, budget: f64) -> u32 {
        match self.cost[sampler as usize] {
            Some(c) if c > 0.0 => ((budget / c) / THREADS as f64).clamp(16.0, 1e6) as u32,
            // Unmeasured: a short first dispatch.
            _ => 256,
        }
    }

    fn new_generation(&mut self, map: &BinMap) {
        if self.last_map.as_ref() != Some(map) {
            self.generation = self.generation.wrapping_add(1).max(1);
            self.last_map = Some(*map);
        }
    }

    fn params(&mut self, key: &HistKey, sampler: Sampler, steps: u32, slot: usize) -> BuddhaParams {
        let m = &key.map;
        let l = key.mode.limits();
        let diag = m.pixel * ((m.width * m.width + m.height * m.height) as f64).sqrt();
        self.seed = self.seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        BuddhaParams {
            limits: [l[0], l[1], l[2], key.mode.max_limit()],
            center: [m.center[0] as f32, m.center[1] as f32],
            rot: [m.rot[0] as f32, m.rot[1] as f32],
            region_min: [REGION_MIN[0] as f32, REGION_MIN[1] as f32],
            region_size: [REGION_SIZE[0] as f32, REGION_SIZE[1] as f32],
            inv_pixel: (1.0 / m.pixel) as f32,
            p_jump: self.p_jump,
            r_max: (f64::from(self.r_max) * diag) as f32,
            r_span: self.r_span,
            width: m.width as u32,
            height: m.height as u32,
            channels: key.mode.channels() as u32,
            steps,
            seed: self.seed,
            mode: sampler as u32,
            generation: self.generation,
            slot: slot as u32,
            threads: THREADS as u32,
            deposit: u32::from(self.deposit),
            drain: 0,
            pad: 0,
        }
    }

    /// The chains judge their states in `p`'s view again; those out of its S copy a random
    /// chain that is in (sequential Monte Carlo; `buddhabrot.metal`).
    fn encode_rechain(&self, enc: &mut B::Encoder, p: &BuddhaParams) {
        for k in [&self.k.recheck, &self.k.resample] {
            enc.set_pipeline(k);
            enc.set_bytes(0, bytes_of(p));
            enc.set_buffer(2, &self.chains, 0);
            enc.set_buffer(4, &self.snap, 0);
            enc.dispatch_threads([THREADS, 1, 1], [64, 1, 1]);
        }
    }

    fn clear(&self, enc: &mut B::Encoder, buf: &B::Buffer, offset: usize, words: usize) {
        enc.set_pipeline(&self.k.clear);
        enc.set_buffer(0, buf, offset);
        enc.set_bytes(1, bytes_of(&(words as u32)));
        enc.dispatch_threads([words, 1, 1], [256.min(words.max(1)), 1, 1]);
    }

    /// The rest histogram for `key` (cleared when the key changes). True: it was (re)started.
    pub fn ensure_rest(&mut self, backend: &B, key: &HistKey) -> Result<bool, GpuError> {
        if self.rest.as_ref().is_some_and(|r| &r.key == key) {
            return Ok(false);
        }
        if self.cost_mode != key.mode {
            self.cost = [None; 2];
            self.cost_mode = key.mode;
        }
        let words = key.mode.channels() * key.map.width * key.map.height;
        let hist = match self.rest.take() {
            Some(r) if r.key.mode.channels() * r.key.map.width * r.key.map.height == words => r.hist,
            _ => backend.create_buffer(words * 4, Storage::Shared)?,
        };
        let mut enc = backend.begin("buddha clear")?;
        self.clear(&mut enc, &hist, 0, words);
        self.clear(&mut enc, &self.totals, 0, COUNTERS * 2);
        // Another look: fresh walkers. Another view: the next dispatch rechecks and resamples
        // them (`encode_rechain`: orbits in progress dropped, chains kept where they still hit).
        let same_look = self.rest_look == Some(key.mode);
        if !same_look {
            self.clear(&mut enc, &self.chains, 0, THREADS * CHAIN_BYTES / 4);
            self.chain_map = None;
        }
        self.rest_look = Some(key.mode);
        enc.commit();
        let epoch = self.next_epoch;
        self.next_epoch += 1;
        self.new_generation(&key.map);
        // Small views: start with MH right away when the last view's rate says so.
        let sampler = if let Some(f) = self.force_sampler {
            f
        } else if key.map.region_share() < 1e-3 {
            Sampler::Metropolis
        } else {
            Sampler::Uniform
        };
        // Chains from an enclosing view (zoomed in) are already good starting states; otherwise
        // a small view warms them up through larger ones.
        let enclosed = self
            .last_rest_map
            .is_some_and(|old| encloses(&old, &key.map) && same_look);
        let mut warm = Vec::new();
        if sampler == Sampler::Metropolis && !enclosed {
            let mut m = key.map;
            while m.region_share() < WARM_SHARE && warm.len() < 12 {
                m.pixel *= 4.0;
                warm.push(m);
            }
            warm.reverse();
        }
        self.last_rest_map = Some(key.map);
        self.rest = Some(Rest {
            key: key.clone(),
            hist,
            epoch,
            done: Counters::default(),
            sampler,
            encoded: 0,
            warm,
            warm_done: 0,
        });
        Ok(true)
    }

    /// Encodes and commits one rest sample dispatch of about `budget` GPU seconds.
    pub fn sample_rest(&mut self, backend: &B, budget: f64) -> Result<(), GpuError> {
        self.rest_dispatch(backend, budget, false)
    }

    /// Finishes every orbit in progress (no new proposals) and waits: the histogram then holds
    /// whole orbits only (walkers persist across dispatches, so at any moment the long orbits
    /// are the ones most likely cut short; an export or a test ends with this).
    pub fn drain(&mut self, backend: &B) -> Result<(), GpuError> {
        self.rest_dispatch(backend, 0.0, true)?;
        self.wait_all(backend);
        Ok(())
    }

    fn rest_dispatch(&mut self, backend: &B, budget: f64, drain: bool) -> Result<(), GpuError> {
        let Some(r) = &self.rest else {
            return Ok(());
        };
        let (mut key, sampler, mut epoch) = (r.key.clone(), r.sampler, r.epoch);
        let warming = !drain && !r.warm.is_empty();
        if warming {
            // A warm-up stage: no deposits, not counted (epoch 0).
            key.map = r.warm[0];
            epoch = 0;
            self.new_generation(&key.map);
            let r = self.rest.as_mut().expect("rest");
            r.warm_done += 1;
            if r.warm_done >= WARM_DISPATCHES {
                r.warm.remove(0);
                r.warm_done = 0;
            }
        } else {
            self.new_generation(&key.map);
        }
        // Draining: the rest of a test pass, a deposit pass and a held state's pass at most.
        let steps = if drain {
            4 * key.mode.max_limit() + 64
        } else {
            self.steps_for(sampler, budget)
        };
        let slot = self.next_slot;
        self.next_slot = (self.next_slot + 1) % SLOTS;
        let mut p = self.params(&key, sampler, steps, slot);
        p.drain = u32::from(drain);
        if warming {
            p.deposit = 0;
        }
        let hist = self.rest.as_ref().expect("rest").hist.clone();
        let mut enc = backend.begin("buddha sample")?;
        // A new view (or warm-up stage) for walkers that sampled another: recheck and resample.
        if self.chain_map != Some(key.map) {
            if self.chain_map.is_some() {
                self.encode_rechain(&mut enc, &p);
            }
            self.chain_map = Some(key.map);
        }
        self.clear(&mut enc, &self.counters, slot * COUNTERS * 4, COUNTERS);
        enc.set_pipeline(&self.k.sample);
        enc.set_bytes(0, bytes_of(&p));
        enc.set_buffer(1, &hist, 0);
        enc.set_buffer(2, &self.chains, 0);
        enc.set_buffer(3, &self.counters, 0);
        enc.dispatch_threads([THREADS, 1, 1], [64, 1, 1]);
        if !warming {
            enc.set_pipeline(&self.k.totals);
            enc.set_buffer(0, &self.counters, slot * COUNTERS * 4);
            enc.set_buffer(1, &self.totals, 0);
            enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
        }
        let sub = enc.commit();
        // (A drain's iterations are mostly idle, a warm-up's deposit nothing: kept out of the
        // cost model.)
        let iterations = if drain || warming {
            0
        } else {
            u64::from(steps) * THREADS as u64
        };
        if let Some(r) = &mut self.rest {
            r.encoded += iterations;
        }
        self.in_flight.push_back(Pending {
            sub,
            slot,
            epoch,
            live: false,
            sampler,
            iterations,
        });
        Ok(())
    }

    /// The palette buffer (level 0 of the mip chain is the table) and its brightest position.
    pub fn ensure_palette(&mut self, backend: &B, key: &str, table: &[[f32; 4]]) -> Result<(), GpuError> {
        if self.palette.as_ref().is_some_and(|p| p.0 == key) {
            return Ok(());
        }
        let lum = |c: &[f32; 4]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let peak = table
            .iter()
            .enumerate()
            .max_by(|a, b| lum(a.1).total_cmp(&lum(b.1)))
            .map_or(1.0, |(i, _)| i as f32 / table.len() as f32);
        let buf = backend.create_buffer_with_data(bytemuck::cast_slice(table))?;
        self.palette = Some((key.to_string(), buf, peak.max(0.05)));
        Ok(())
    }

    /// Encodes the tone pass of the rest histogram into `target` (sRGB, dithered) and
    /// `history` (linear), both `out` sized.
    pub fn encode_tone_rest(
        &self,
        enc: &mut B::Encoder,
        target: &B::Texture,
        history: &B::Texture,
        out: (usize, usize),
    ) {
        let Some(r) = &self.rest else { return };
        let m = &r.key.map;
        let t = self.tone_params(r.key.mode, (m.width, m.height), out, 0, [0.0; 3]);
        self.encode_tone(enc, &t, &r.hist, &r.hist, target, history, out);
    }

    fn tone_params(
        &self,
        mode: BuddhaMode,
        src: (usize, usize),
        out: (usize, usize),
        source: u32,
        mean: [f64; 3],
    ) -> ToneParams {
        let scale = |k: usize| if mean[k] > 0.0 { (1.0 / mean[k]) as f32 } else { 0.0 };
        let (white, black, gamma) = match self.fixed_tone {
            Some((w, g, b)) => ([w; 4], [b; 4], g),
            None => {
                let l = self.levels.as_ref().filter(|l| l.mode == mode);
                let d = Levels::defaults(mode);
                let (w, b) = l.map_or((d.white, d.black), |l| (l.white, l.black));
                (
                    [w[0], w[1], w[2], 1.0],
                    [b[0], b[1], b[2], 0.0],
                    buddha::TONE_GAMMA as f32,
                )
            }
        };
        ToneParams {
            scale: [scale(0), scale(1), scale(2), 0.0],
            white,
            black,
            src_width: src.0 as u32,
            src_height: src.1 as u32,
            out_width: out.0 as u32,
            out_height: out.1 as u32,
            channels: mode.channels() as u32,
            source,
            palette_size: crate::scheduler::PALETTE_SIZE as u32,
            gamma,
            palette_peak: self.palette.as_ref().map_or(0.5, |p| p.2),
            pad: [0; 3],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_tone(
        &self,
        enc: &mut B::Encoder,
        t: &ToneParams,
        hist: &B::Buffer,
        acc: &B::Buffer,
        target: &B::Texture,
        history: &B::Texture,
        out: (usize, usize),
    ) {
        let Some((_, pal, _)) = &self.palette else { return };
        enc.set_pipeline(&self.k.tone);
        enc.set_bytes(0, bytes_of(t));
        enc.set_buffer(1, hist, 0);
        enc.set_buffer(2, acc, 0);
        enc.set_buffer(3, pal, 0);
        enc.set_buffer(4, &self.totals, 0);
        enc.set_texture(0, target);
        enc.set_texture(1, history);
        enc.dispatch_threads([out.0, out.1, 1], [16, 16, 1]);
    }

    /// The live accumulation at `width` x `height` bins for `mode` (reallocated on change).
    pub fn ensure_live(&mut self, backend: &B, mode: BuddhaMode, width: usize, height: usize) -> Result<(), GpuError> {
        if self
            .live
            .as_ref()
            .is_some_and(|l| l.width == width && l.height == height && l.mode == mode)
        {
            return Ok(());
        }
        if self.cost_mode != mode {
            self.cost = [None; 2];
            self.cost_mode = mode;
        }
        let bins = width * height;
        let frame = backend.create_buffer_with_data(&vec![0u8; bins * 3 * 4])?;
        let acc = [
            backend.create_buffer(bins * 16, Storage::Private)?,
            backend.create_buffer(bins * 16, Storage::Private)?,
        ];
        self.live = Some(Live {
            key: None,
            frame,
            acc,
            ping: 0,
            width,
            height,
            mode,
            mean: [0.0; 3],
            rho: 1.0,
            hits_per_bin: 0.0,
            sampler: Sampler::Uniform,
            frames: 0,
        });
        Ok(())
    }

    /// Forgets the live accumulation's content (the next frame starts from nothing).
    pub fn reset_live(&mut self) {
        if let Some(l) = &mut self.live {
            l.key = None;
            l.hits_per_bin = 0.0;
        }
    }

    /// Samples this view's coarse histogram for about `budget` GPU seconds (its own command
    /// buffer, tracked here); [`Buddha::encode_fold_tone`] then folds it into the frame.
    pub fn sample_live(&mut self, backend: &B, key: &HistKey, budget: f64) -> Result<(), GpuError> {
        let Some(l) = &self.live else { return Ok(()) };
        let sampler = l.sampler;
        let frame = l.frame.clone();
        let steps = self.steps_for(sampler, budget);
        let slot = self.next_slot;
        self.next_slot = (self.next_slot + 1) % SLOTS;
        self.new_generation(&key.map);
        let p = self.params(key, sampler, steps, slot);
        let mut enc = backend.begin("buddha live sample")?;
        self.clear(&mut enc, &self.counters, slot * COUNTERS * 4, COUNTERS);
        enc.set_pipeline(&self.k.sample);
        enc.set_bytes(0, bytes_of(&p));
        enc.set_buffer(1, &frame, 0);
        enc.set_buffer(2, &self.chains, 0);
        enc.set_buffer(3, &self.counters, 0);
        enc.dispatch_threads([THREADS, 1, 1], [64, 1, 1]);
        let sub = enc.commit();
        self.in_flight.push_back(Pending {
            sub,
            slot,
            epoch: 0,
            live: true,
            sampler,
            iterations: u64::from(steps) * THREADS as u64,
        });
        self.live_slot = Some((slot, sampler));
        Ok(())
    }

    /// Folds the last [`Buddha::sample_live`] into the accumulation reprojected into `key`'s
    /// view (old data weighted by `decay`), then tone-maps into `target` / `history`.
    pub fn encode_fold_tone(
        &mut self,
        enc: &mut B::Encoder,
        key: &HistKey,
        decay: f64,
        target: &B::Texture,
        history: &B::Texture,
        out: (usize, usize),
    ) {
        let Some((slot, sampler)) = self.live_slot.take() else {
            return;
        };
        let Some(l) = &self.live else { return };
        let (w, h) = (l.width, l.height);
        let mut f = FoldParams {
            width: w as u32,
            height: h as u32,
            channels: key.mode.channels() as u32,
            decay: decay as f32,
            area_ratio: 1.0,
            rho: if sampler == Sampler::Uniform {
                1.0
            } else {
                l.rho.max(1e-9) as f32
            },
            reproject: 0,
            slot: slot as u32,
            ..FoldParams::default()
        };
        if let Some(old) = &l.key
            && old.mode == key.mode
        {
            let (o, sx, sy, area) = reprojection(&old.map, &key.map);
            f.origin = o;
            f.step_x = sx;
            f.step_y = sy;
            f.area_ratio = area as f32;
            f.reproject = 1;
            // Zooming in stretches old bins over several new ones: they count for less.
            f.decay = (decay * area.min(1.0)) as f32;
        }
        let (acc_in, acc_out) = (l.acc[l.ping].clone(), l.acc[1 - l.ping].clone());
        let frame = l.frame.clone();
        enc.set_pipeline(&self.k.fold);
        enc.set_bytes(0, bytes_of(&f));
        enc.set_buffer(1, &frame, 0);
        enc.set_buffer(2, &acc_in, 0);
        enc.set_buffer(3, &acc_out, 0);
        enc.set_buffer(4, &self.counters, 0);
        enc.dispatch_threads([w, h, 1], [16, 16, 1]);
        let t = self.tone_params(key.mode, (w, h), out, 1, l.mean);
        self.encode_tone(enc, &t, &frame, &acc_out, target, history, out);
        let ch = key.mode.channels().max(1);
        let per_frame = (0..ch).map(|k| self.last_live.hits[k]).min().unwrap_or(0) as f64 / (w * h) as f64;
        let l = self.live.as_mut().expect("live");
        l.ping = 1 - l.ping;
        l.hits_per_bin = if l.key.is_some() {
            f64::from(f.decay) * l.hits_per_bin + per_frame
        } else {
            per_frame
        };
        l.key = Some(key.clone());
    }

    /// Starts the live accumulation from the rest histogram (motion begins from the clean
    /// image): each live bin sums its `div` x `div` rest bins; `key`: the live grid of the same
    /// view.
    pub fn seed_live(&mut self, enc: &mut B::Encoder, key: &HistKey, div: usize) -> bool {
        let (Some(r), Some(l)) = (&self.rest, &self.live) else {
            return false;
        };
        if r.key.mode != key.mode || r.done.proposals == 0 {
            return false;
        }
        // The histogram holds the encoded dispatches; the totals only the completed ones.
        let samples = r.samples() * r.encoded as f64 / r.done.iterations.max(1) as f64;
        let rm = &r.key.map;
        let s = SeedLiveParams {
            div: div as u32,
            rest_width: rm.width as u32,
            rest_height: rm.height as u32,
            width: l.width as u32,
            height: l.height as u32,
            channels: key.mode.channels() as u32,
            samples: samples as f32,
            off_x: ((l.width * div) as i32 - rm.width as i32) / 2,
            off_y: ((l.height * div) as i32 - rm.height as i32) / 2,
            pad: 0,
        };
        let out = l.acc[l.ping].clone();
        let (w, h) = (l.width, l.height);
        enc.set_pipeline(&self.k.seed_live);
        enc.set_bytes(0, bytes_of(&s));
        enc.set_buffer(1, &r.hist, 0);
        enc.set_buffer(2, &out, 0);
        enc.dispatch_threads([w, h, 1], [16, 16, 1]);
        let bins = (rm.width * rm.height) as f64;
        let ch = key.mode.channels().max(1);
        let mut mean = [0.0; 3];
        for (k, m) in mean.iter_mut().enumerate().take(ch) {
            *m = r.done.hits[k] as f64 / (r.samples().max(1.0) * bins) * (div * div) as f64;
        }
        let rest_hits = r.mean_hits() * (div * div) as f64;
        let l = self.live.as_mut().expect("live");
        l.mean = mean;
        l.hits_per_bin = rest_hits;
        l.frames = l.frames.max(1);
        l.key = Some(key.clone());
        true
    }

    /// Re-tones the live accumulation (no new samples) into `target` / `history`: shown after
    /// motion until the rest histogram is as clean.
    pub fn encode_tone_live(
        &self,
        enc: &mut B::Encoder,
        target: &B::Texture,
        history: &B::Texture,
        out: (usize, usize),
    ) {
        let Some(l) = &self.live else { return };
        let Some(key) = &l.key else { return };
        let t = self.tone_params(key.mode, (l.width, l.height), out, 1, l.mean);
        self.encode_tone(enc, &t, &l.frame, &l.acc[l.ping], target, history, out);
    }

    /// The rest histogram as counts (blocks for the GPU; tests and the export's statistics).
    pub fn read_rest(&mut self, backend: &B) -> Option<buddha::Histogram> {
        self.wait_all(backend);
        let r = self.rest.as_ref()?;
        let ch = r.key.mode.channels();
        let n = ch * r.key.map.width * r.key.map.height;
        let mut bytes = vec![0u8; n * 4];
        backend.read_buffer(&r.hist, 0, &mut bytes);
        Some(buddha::Histogram {
            counts: bytemuck::cast_slice(&bytes).to_vec(),
            channels: ch,
            width: r.key.map.width,
            height: r.key.map.height,
            samples: r.samples(),
        })
    }

    /// Accumulates the rest histogram of `key` (blocking) until `done(rest)` says so, in
    /// dispatches of about `budget` GPU seconds with two in flight: offscreen export, tests and
    /// measurements.
    pub fn accumulate(
        &mut self,
        backend: &B,
        key: &HistKey,
        budget: f64,
        mut done: impl FnMut(&Rest<B>) -> bool,
    ) -> Result<(), GpuError> {
        self.ensure_rest(backend, key)?;
        loop {
            self.harvest(backend);
            if done(self.rest.as_ref().expect("rest")) {
                break;
            }
            while self.in_flight.len() < 2 {
                self.sample_rest(backend, budget)?;
            }
            if let Some(p) = self.in_flight.front() {
                p.sub.wait();
            }
        }
        self.drain(backend)
    }

    /// Drops the GPU memory of the accumulations (the look was turned off).
    pub fn release(&mut self) {
        self.rest = None;
        self.live = None;
    }

    /// Bytes held (histograms, accumulation, chains, counters).
    pub fn bytes(&self) -> u64 {
        let rest = self
            .rest
            .as_ref()
            .map_or(0, |r| r.key.mode.channels() * r.key.map.width * r.key.map.height * 4);
        let live = self.live.as_ref().map_or(0, |l| l.width * l.height * (12 + 32));
        (rest + live + THREADS * (CHAIN_BYTES + 16) + SLOTS * COUNTERS * 4 + LEVELS_WORDS * 4) as u64
    }
}

/// `outer`'s view contains `inner`'s (by their circumscribed and inscribed discs; rotation
/// aside).
fn encloses(outer: &BinMap, inner: &BinMap) -> bool {
    let d = ((outer.center[0] - inner.center[0]).powi(2) + (outer.center[1] - inner.center[1]).powi(2)).sqrt();
    let inner_r = 0.5 * inner.pixel * ((inner.width.pow(2) + inner.height.pow(2)) as f64).sqrt();
    let outer_r = 0.5 * outer.pixel * outer.width.min(outer.height) as f64;
    d + inner_r <= outer_r
}

/// Buckets per channel of the levels histogram (1/16 octave each), and the buffer's words.
const LEVEL_BUCKETS: usize = 512;
const LEVELS_WORDS: usize = 3 * LEVEL_BUCKETS + 8;

/// The tone curve's black and white points per channel, in view means (the mean density of the
/// channel's bins): percentiles of the rest histogram ([`buddha::TONE_BLACK_Q`],
/// [`buddha::TONE_WHITE_Q`]), so a flat close-up is stretched to its own range as the whole set
/// is to its.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub mode: BuddhaMode,
    pub black: [f32; 3],
    pub white: [f32; 3],
}

impl Levels {
    /// Before a measurement.
    pub fn defaults(mode: BuddhaMode) -> Self {
        let b = mode.tone_black() as f32;
        let w = buddha::TONE_WHITE as f32;
        Self {
            mode,
            black: [b; 3],
            white: [w; 3],
        }
    }

    /// From the levels buffer: bucket counts per channel and the hit totals (64-bit) of
    /// `bins` bins.
    fn from_buckets(mode: BuddhaMode, words: &[u32], bins: usize) -> Option<Self> {
        let mut l = Self::defaults(mode);
        let ch = mode.channels();
        for k in 0..ch {
            let hits = f64::from(words[3 * LEVEL_BUCKETS + 2 * k])
                + 4294967296.0 * f64::from(words[3 * LEVEL_BUCKETS + 2 * k + 1]);
            let mean = hits / bins as f64;
            if mean <= 0.0 {
                return None;
            }
            let buckets = &words[k * LEVEL_BUCKETS..(k + 1) * LEVEL_BUCKETS];
            let total: u64 = buckets.iter().map(|&c| u64::from(c)).sum();
            let at = |q: f64| {
                let want = (q * total as f64) as u64;
                let mut acc = 0u64;
                for (b, &c) in buckets.iter().enumerate() {
                    acc += u64::from(c);
                    if acc > want {
                        return if b == 0 {
                            0.0
                        } else {
                            2f64.powf((b as f64 - 0.5) / 16.0)
                        };
                    }
                }
                2f64.powf(LEVEL_BUCKETS as f64 / 16.0)
            };
            // (Never below the default black point: the haze stays dark at the whole set.)
            let black = (at(mode.tone_black_q()) / mean).max(mode.tone_black());
            let white = (at(buddha::TONE_WHITE_Q) / mean).max(black + buddha::TONE_MIN_RANGE);
            l.black[k] = black as f32;
            l.white[k] = white as f32;
        }
        // The Nebulabrot's channels share the white point (the brightest channel's), so the
        // colour balance holds: a channel of flat haze is not stretched to white.
        let w = l.white[..ch].iter().copied().fold(0.0f32, f32::max);
        for x in &mut l.white[..ch] {
            *x = w;
        }
        Some(l)
    }
}

/// The fold's mapping from new bins to old: old bin coordinates (x right, y down; texel units)
/// of new bin (0, 0)'s centre, per new bin right and down, and the area ratio new / old.
pub fn reprojection(old: &BinMap, new: &BinMap) -> ([f32; 2], [f32; 2], [f32; 2], f64) {
    // A new bin's centre (i + 0.5, j + 0.5) as a complex point, then in old bin coordinates.
    let to_c = |i: f64, j: f64| {
        let (vx, vy) = (
            (i - new.width as f64 / 2.0) * new.pixel,
            (new.height as f64 / 2.0 - j) * new.pixel,
        );
        let [c, s] = new.rot;
        (new.center[0] + vx * c - vy * s, new.center[1] + vx * s + vy * c)
    };
    let to_old = |(x, y): (f64, f64)| {
        let (dx, dy) = (x - old.center[0], y - old.center[1]);
        let [c, s] = old.rot;
        let (u, v) = ((dx * c + dy * s) / old.pixel, (-dx * s + dy * c) / old.pixel);
        (u + old.width as f64 / 2.0, old.height as f64 / 2.0 - v)
    };
    let o = to_old(to_c(0.5, 0.5));
    let x = to_old(to_c(1.5, 0.5));
    let y = to_old(to_c(0.5, 1.5));
    let area = (new.pixel / old.pixel).powi(2);
    (
        [o.0 as f32, o.1 as f32],
        [(x.0 - o.0) as f32, (x.1 - o.1) as f32],
        [(y.0 - o.0) as f32, (y.1 - o.1) as f32],
        area,
    )
}

#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use super::*;
    use crate::metal::MetalBackend;
    use almondbread_core::buddha::{compare, cpu_histogram};
    use almondbread_core::viewport::Viewport;

    #[test]
    fn layout_matches_the_kernels() {
        let b = MetalBackend::new().expect("metal");
        let g = Buddha::new(&b).expect("kernels");
        assert_eq!(g.kernel_layout(&b).unwrap(), [112, CHAIN_BYTES as u32, 56, 96, 40, 16]);
    }

    fn gpu(b: &MetalBackend, map: BinMap, mode: BuddhaMode, sampler: Sampler, samples: f64) -> buddha::Histogram {
        let mut g = Buddha::new(b).expect("kernels");
        g.force_sampler = Some(sampler);
        let key = HistKey { map, mode };
        g.accumulate(b, &key, 0.01, |r| r.samples() >= samples).unwrap();
        g.read_rest(b).unwrap()
    }

    /// GPU (float32) vs the CPU oracle (f64) at a small size: the same distribution within
    /// sampling noise. Orbit hits are correlated (one orbit deposits a run of hits), so the
    /// bound is relative to the CPU-vs-CPU difference of two seeds at the same sample count.
    #[test]
    fn gpu_histogram_matches_the_cpu_oracle() {
        let b = MetalBackend::new().expect("metal");
        let n = 16_000_000u64;
        // The whole set, uniform sampling, both looks.
        let whole = BinMap::new(&Viewport::new(-0.5, 0.0, 3.2 / 64.0), 64, 64, 1.0);
        for mode in [BuddhaMode::Buddhabrot, BuddhaMode::Nebulabrot] {
            let g = gpu(&b, whole, mode, Sampler::Uniform, n as f64);
            let c1 = cpu_histogram(&whole, mode, g.samples as u64, 11);
            let c2 = cpu_histogram(&whole, mode, g.samples as u64, 12);
            for ch in 0..mode.channels() {
                let (chi, l1) = compare(&g, &c1, ch, 4);
                let (chi0, l10) = compare(&c2, &c1, ch, 4);
                println!(
                    "{} ch {ch}: GPU-CPU chi2/dof {chi:.2} L1 {l1:.4}; CPU-CPU {chi0:.2} {l10:.4}",
                    mode.name()
                );
                assert!(l1 < 1.5 * l10 + 0.005, "L1 {l1} vs {l10}");
                assert!(chi < 1.5 * chi0 + 0.5, "chi2 {chi} vs {chi0}");
            }
        }
        // A close-up by Metropolis–Hastings against uniform CPU sampling. MH samples are
        // correlated (chains), so its noise is measured too: two GPU runs of other seeds. With
        // independent errors the expected GPU-CPU distance is about the root mean square of the
        // two noise levels; a bias would add to it.
        let close = BinMap::new(&Viewport::new(-0.1, 0.75, 0.4 / 64.0), 64, 64, 1.0);
        let n = 40_000_000u64;
        let c1 = cpu_histogram(&close, BuddhaMode::Buddhabrot, n, 21);
        let c2 = cpu_histogram(&close, BuddhaMode::Buddhabrot, n, 22);
        let mh = |seed: u32| {
            let mut g = Buddha::new(&b).expect("kernels");
            g.force_sampler = Some(Sampler::Metropolis);
            g.set_seed(seed);
            let key = HistKey {
                map: close,
                mode: BuddhaMode::Buddhabrot,
            };
            // (By samples, not time: a loaded GPU must not end it early. MH chains carry a
            // transient bias that decays as they mix: L1 0.05 at 1e9 samples, 0.008 at 1e10.)
            g.accumulate(&b, &key, 0.01, |r| r.samples() >= 1e10).unwrap();
            g.read_rest(&b).unwrap()
        };
        let (g1, g2) = (mh(7), mh(8));
        let (_, l1) = compare(&g1, &c1, 0, 4);
        let (_, lgg) = compare(&g1, &g2, 0, 4);
        let (_, lcc) = compare(&c2, &c1, 0, 4);
        let rms = ((lgg * lgg + lcc * lcc) / 2.0).sqrt();
        println!(
            "MH close-up ({:.1e} uniform-equivalent samples): GPU-CPU L1 {l1:.4}; GPU-GPU {lgg:.4}, CPU-CPU {lcc:.4} (rms {rms:.4})",
            g1.samples
        );
        assert!(l1 < 1.5 * rms + 0.005, "L1 {l1} vs noise {rms}");
    }

    /// MH bias against a long CPU run, by GPU time and jump probability (a measurement:
    /// `cargo test --release -p almondbread-render mh_bias -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn mh_bias_scan() {
        let b = MetalBackend::new().expect("metal");
        let close = BinMap::new(&Viewport::new(-0.1, 0.75, 0.4 / 64.0), 64, 64, 1.0);
        let c1 = cpu_histogram(&close, BuddhaMode::Buddhabrot, 400_000_000, 31);
        for p_jump in [0.05f32, 0.5, 1.0] {
            for secs in [0.25, 1.0, 4.0] {
                let mut g = Buddha::new(&b).expect("kernels");
                g.force_sampler = Some(Sampler::Metropolis);
                g.p_jump = p_jump;
                let key = HistKey {
                    map: close,
                    mode: BuddhaMode::Buddhabrot,
                };
                let start = std::time::Instant::now();
                g.accumulate(&b, &key, 0.01, |_| start.elapsed().as_secs_f64() >= secs)
                    .unwrap();
                let h = g.read_rest(&b).unwrap();
                let (_, l1) = compare(&h, &c1, 0, 4);
                println!(
                    "pJump {p_jump} {secs} s: {:.2e} samples, L1 vs CPU 4e8 {l1:.4}",
                    h.samples
                );
            }
        }
    }
}
