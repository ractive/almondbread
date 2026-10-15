//! One frame of the landscape: flight → clipmap placement → DE tile budget → compute + render
//! command buffers. Shared by the window and the offscreen recorder.

use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLCommandBuffer, MTLTexture};

use crate::clipmap::{ClipConfig, Clipmap, ScheduleCtx, TileKey, ViewCtx};
use objc2_metal::{MTLBuffer, MTLCommandBufferStatus};
use crate::flight::{Flight, Input};
use crate::gpu::{Buffer, CommandBuffer, DeParams, Globals, Gpu, TileJob};
use crate::ground::Ground;
use crate::mandel;

#[derive(Clone, Debug, clap::Args)]
pub struct Settings {
    /// Start location: seahorse, elephant, neck, spiral, minibrot3, or re,im.
    #[arg(long, default_value = "seahorse")]
    pub loc: String,
    /// Start scale S (altitude, complex units).
    #[arg(long, default_value_t = 1e-3)]
    pub scale: f64,
    /// Data texels per level and axis.
    #[arg(long, default_value_t = 1024)]
    pub n: i64,
    /// Vertex stride in samples (2 or 4).
    #[arg(long, default_value_t = 2)]
    pub k: i64,
    /// Clipmap levels.
    #[arg(long, default_value_t = 11)]
    pub levels: usize,
    /// Finest sample spacing ≈ S / detail.
    #[arg(long, default_value_t = 1024.0)]
    pub detail: f64,
    /// Horizontal field of view, degrees.
    #[arg(long, default_value_t = 75.0)]
    pub fov: f64,
    /// Fog distance in units of S.
    #[arg(long, default_value_t = 60.0)]
    pub fog: f64,
    /// Planet radius in units of S (hides far walls below the horizon).
    #[arg(long, default_value_t = 300.0)]
    pub curvature: f64,
    /// GPU milliseconds per frame for DE tiles.
    #[arg(long, default_value_t = 2.5)]
    pub budget_ms: f64,
    #[arg(long, default_value_t = 20_000)]
    pub max_iter: u32,
    /// Iterations per sample and frame (longer samples resume in later frames).
    #[arg(long, default_value_t = 1500)]
    pub chunk: u32,
    /// Fade-in time of new tiles, seconds.
    #[arg(long, default_value_t = 0.3)]
    pub fade: f64,
    /// Sun azimuth (degrees from +Re) and elevation.
    #[arg(long, default_value_t = 200.0, allow_hyphen_values = true)]
    pub sun_az: f64,
    #[arg(long, default_value_t = 28.0)]
    pub sun_el: f64,
    /// How the height curve's σ follows the scale: `continuous` (σ = 0.1 S, L0: the whole world
    /// re-shapes while S changes) or `layers` (σ fixed within layers of `layer_octaves`, smooth
    /// risers between them).
    #[arg(long, default_value = "layers")]
    pub sigma_mode: String,
    #[arg(long, default_value_t = 2.0)]
    pub layer_octaves: f64,
    /// Fraction of a layer (in log S) spent in the transition.
    #[arg(long, default_value_t = 0.5)]
    pub riser: f64,
    /// Surface colour: `world` (strata by absolute DE, layer terms cross-faded) or `legacy`
    /// (strata by DE/σ, terms by DE/S: they slide while diving).
    #[arg(long, default_value = "world")]
    pub colour: String,
    /// Start in the auto-zoom mode (steep nose-down zooms in, height above ground zooms out);
    /// Z toggles at runtime.
    #[arg(long)]
    pub auto_zoom: bool,
    /// Detail synthesis below the computed resolution: amplitude per octave as a fraction of its
    /// wavelength (0 = off, the L1 look).
    #[arg(long, default_value_t = 0.12)]
    pub detail_amp: f64,
    /// Detail octaves only on the normals, not in the geometry.
    #[arg(long)]
    pub no_vertex_detail: bool,
    /// Equipotential ridges on plains: height ridges·σ·sin²(πν/2) (ν the smooth iteration
    /// count) where DE/σ > 1.5 (full beyond 10). 0 = off (the L1 terrain).
    #[arg(long, default_value_t = 0.3)]
    pub ridges: f64,
    /// Vertices per ring axis (multiple of 32; 0 = the largest that fits the data window, L1:
    /// 448). Fewer vertices: the fragments still shade from the finest data level.
    #[arg(long, default_value_t = 256)]
    pub ring: i64,
    /// No world-anchored colour variation (field bands, vegetation patches).
    #[arg(long)]
    pub no_variation: bool,
    /// Debug colouring (validity weights).
    #[arg(long)]
    pub debug: bool,
    /// Heading offset at the start (degrees, added to "towards the set").
    #[arg(long, default_value_t = 25.0, allow_hyphen_values = true)]
    pub turn_heading: f64,
}

pub fn loc_center(name: &str) -> (f64, f64) {
    match name {
        "seahorse" => (-0.743643887037151, 0.131825904205330),
        "elephant" => (0.2925755, -0.0149977),
        "neck" => (-0.75, 0.05),
        "spiral" => (-0.7746806106269039, -0.1374168856037867),
        "minibrot3" => (-1.7548776662466927, 0.0),
        other => {
            let mut it = other.split(',').map(|v| v.trim().parse::<f64>().expect("loc: name or re,im"));
            (it.next().unwrap(), it.next().unwrap())
        }
    }
}

/// Per-frame record for the measurements.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameRec {
    pub serial: u64,
    pub t: f64,
    pub dt: f64,
    pub cpu_ms: f64,
    pub tiles: usize,
    pub missing: usize,
    pub missing_ahead: usize,
    pub s: f64,
    /// layer transition progress (0 = rigid phase), dive rate (e-folds/s)
    pub riser: f64,
    pub dive_rate: f64,
    pub blocks: usize,
    pub hag: f64,
}

pub struct Sim {
    pub set: Settings,
    pub gpu: Gpu,
    pub clip: Clipmap,
    pub flight: Flight,
    pub ground: Ground,
    /// world origin (absolute c of offset 0)
    pub c0: (f64, f64),
    pub ref_off: [f64; 2],
    /// the scale at which the reference was chosen
    pub ref_s: f64,
    pub ref_note: String,
    pub time: f64,
    pub serial: u64,
    /// EMA of GPU ms per DE tile
    pub ms_per_tile: f64,
    /// (GPU ms, tiles) of finished DE batches not yet in the estimate
    cost_acc: (f64, usize),
    pub tiles_per_frame: usize,
    /// window: GPU time per refresh the frame must fit in (render + DE); None offscreen
    pub frame_target_ms: Option<f64>,
    /// (serial, tiles) of submitted compute buffers awaiting their time
    pending: Vec<(u64, usize)>,
    pub gpu_ms: Vec<(u64, f64, f64)>, // serial, compute ms, render ms
    blocks: Vec<[u16; 2]>,
    pub recs: Vec<FrameRec>,
    pub last_blocks: usize,
    /// σ of the previous frame (pop check renders the previous world)
    prev_sigma: Option<f64>,
    prev_s: f64,
    free_states: Vec<u32>,
    resume: Vec<(TileJob, TileKey)>,
    inflight: Vec<(CommandBuffer, Buffer, Vec<(TileJob, TileKey)>)>,
    pub resumed_total: u64,
    /// tiles finished (all 256 samples written)
    pub tiles_done: u64,
    pub rereferences: u64,
    /// When set, each frame first renders its camera with the previous world state into it.
    pub popcheck: Option<crate::gpu::Texture>,
}

const SIGMA: f64 = 0.1;
const K_HEIGHT: f64 = 4.0;

/// Colour layers at scale `s`: (σ of the layer, σ of the next finer layer, cross-fade 0..1).
/// Same staircase as `sigma_at`, but hard steps: colour is keyed to a σ that stays put and
/// cross-fades instead of sliding.
pub fn sigma_layers(set: &Settings, s: f64) -> (f64, f64, f64) {
    if set.sigma_mode == "continuous" {
        return (SIGMA * s, SIGMA * s, 0.0);
    }
    let d = set.layer_octaves;
    let u = s.log2() / d;
    let f = u.floor();
    let t = u - f;
    let r = set.riser.clamp(0.01, 0.99);
    let x = ((t - (1.0 - r)) / r).clamp(0.0, 1.0);
    let base = SIGMA * (d * (1.0 - r)).exp2();
    (base * (d * f).exp2(), base * (d * (f + 1.0)).exp2(), x * x * (3.0 - 2.0 * x))
}

/// σ (complex units) of the height curve h = σ·ln(1 + k·DE/σ) at scale `s`, and the riser
/// progress (0 outside a transition, peaks at 1 mid-transition) for the mist.
pub fn sigma_at(set: &Settings, s: f64) -> (f64, f64) {
    if set.sigma_mode == "continuous" {
        return (SIGMA * s, 0.0);
    }
    let d = set.layer_octaves;
    let u = s.log2() / d;
    let f = u.floor();
    let t = u - f;
    let r = set.riser.clamp(0.01, 0.99);
    let x = ((t - (1.0 - r)) / r).clamp(0.0, 1.0);
    let q = f + x * x * (3.0 - 2.0 * x);
    // anchor: σ = 0.1·S at the top of a layer's static part
    let sigma = SIGMA * (d * q).exp2() * (d * (1.0 - r)).exp2();
    (sigma, (std::f64::consts::PI * x).sin())
}

impl Sim {
    pub fn new(set: Settings, start_speed: f64) -> Result<Self, String> {
        let cfg = ClipConfig::new(set.n, set.k, set.levels, set.detail, set.ring);
        let gpu = Gpu::new(cfg.n as usize, cfg.slots, crate::clipmap::BLOCK as usize)?;
        let (c0, heading) = place(&set);
        let mut sim = Sim {
            clip: Clipmap::new(cfg),
            flight: Flight::new(set.scale, heading, start_speed),
            ground: Ground::new(c0, sigma_at(&set, set.scale).0, K_HEIGHT),
            prev_sigma: None,
            prev_s: 0.0,
            gpu,
            c0,
            ref_off: [0.0, 0.0],
            ref_s: 1.0,
            ref_note: String::new(),
            time: 0.0,
            serial: 0,
            ms_per_tile: 0.01,
            cost_acc: (0.0, 0),
            tiles_per_frame: 64,
            frame_target_ms: None,
            pending: Vec::new(),
            gpu_ms: Vec::new(),
            blocks: Vec::new(),
            recs: Vec::new(),
            last_blocks: 0,
            popcheck: None,
            free_states: (0..crate::gpu::STATE_SLOTS as u32).rev().collect(),
            resume: Vec::new(),
            inflight: Vec::new(),
            resumed_total: 0,
            tiles_done: 0,
            rereferences: 0,
            set,
        };
        sim.ground.ridges = sim.set.ridges;
        sim.ground.ridge_from = 1.5;
        sim.choose_reference()?;
        sim.flight.auto_zoom = sim.set.auto_zoom;
        sim.flight.settle(&mut sim.ground, 1.0);
        Ok(sim)
    }

    /// Reference orbit: the nucleus of the lowest-period minibrot near the camera (periodic,
    /// never escapes, so rebasing wraps it), else the camera point's own orbit.
    pub fn choose_reference(&mut self) -> Result<(), String> {
        let c = (self.c0.0 + self.flight.pos[0], self.c0.1 + self.flight.pos[1]);
        let s = self.flight.s;
        // candidates from ball-period searches at a few radii; the highest period within 24 S
        // wins (the deepest minibrot nearby: the orbits at this scale follow it). The lowest
        // period near the camera (e.g. 78 where 624 governs at 1e-7) left most fine samples wrong.
        let mut chosen: Option<((f64, f64), u32)> = None;
        for r in [0.5 * s, 2.0 * s, 8.0 * s, 32.0 * s] {
            if let Some((nuc, period, _size)) = mandel::find_minibrot(c, r, 300_000) {
                let d = (nuc.0 - c.0).hypot(nuc.1 - c.1);
                if d < 24.0 * s && period >= 1 && chosen.is_none_or(|ch| period > ch.1) {
                    chosen = Some((nuc, period));
                }
            }
        }
        let (cref, orbit, note) = match chosen {
            Some((nuc, p)) => {
                let o = mandel::reference_orbit(nuc, p as usize + 1);
                (nuc, o, format!("reference: nucleus period {p} at {:.3e} S", (nuc.0 - c.0).hypot(nuc.1 - c.1) / s))
            }
            None => {
                let o = mandel::reference_orbit(c, self.set.max_iter as usize + 1);
                let n = o.len();
                (c, o, format!("reference: camera point, {n} iterations"))
            }
        };
        self.ref_off = [cref.0 - self.c0.0, cref.1 - self.c0.1];
        self.ref_s = s;
        self.ref_note = note;
        self.gpu.set_reference(&orbit)
    }

    fn de_params(&self, now: f32, jobs: usize) -> DeParams {
        DeParams {
            ref_len: self.gpu.ref_len,
            max_iter: self.set.max_iter,
            bailout2: 1e10,
            interior_eps: 1e-3,
            now,
            job_count: jobs as u32,
            chunk: self.set.chunk,
            pad: 0.0,
        }
    }

    fn sched_ctx(&self) -> ScheduleCtx {
        let (fwd, right, up) = self.flight.basis();
        ScheduleCtx {
            cam: self.flight.pos,
            s: self.flight.s,
            ref_off: self.ref_off,
            fog: self.set.fog,
            hag: self.flight.hag,
            fwd,
            right,
            up,
            tan_half: (self.set.fov.to_radians() * 0.5).tan(),
            aspect: 16.0 / 10.0,
        }
    }

    /// Computes every missing tile now (cold start), already faded in.
    pub fn prefill(&mut self) -> usize {
        self.clip.update(self.flight.pos, self.detail_scale(), self.flight.s);
        let mut total = 0;
        loop {
            let ctx = self.sched_ctx();
            let batch = self.clip.schedule(&ctx, 4096);
            if batch.is_empty() {
                break;
            }
            total += batch.len();
            let jobs: Vec<TileJob> = batch.iter().enumerate().map(|(i, j)| TileJob { state: i as u32, ..j.0 }).collect();
            let cb = self.gpu.command_buffer();
            let mut p = self.de_params(-1e6, jobs.len());
            p.chunk = self.set.max_iter;
            self.gpu.encode_tiles(&cb, &jobs, p);
            cb.commit();
            cb.waitUntilCompleted();
            for (_, k) in batch {
                self.clip.mark_done(k);
            }
        }
        total
    }

    fn drain_times(&mut self) {
        let done: Vec<(u64, u8, f64)> = match self.gpu.times.lock() {
            Ok(mut g) => std::mem::take(&mut g.done),
            Err(_) => return,
        };
        for (serial, kind, t) in done {
            let ms = t * 1e3;
            if kind == 0
                && let Some(pos) = self.pending.iter().position(|p| p.0 == serial) {
                    let (_, tiles) = self.pending.remove(pos);
                    // EMA of the GPU cost per tile over at least 32 tiles (several small batches
                    // pooled; a single small batch is dominated by its fixed overhead, and L2's
                    // first version skipped batches under 24 tiles, which froze the estimate once
                    // the budget had fallen to 16 tiles a frame). Only a large overshoot rises at
                    // once. (L1 took every rise at once and decayed by 5 % per frame: small
                    // batches kept the budget at a fraction for seconds while the GPU idled.)
                    self.cost_acc.0 += ms;
                    self.cost_acc.1 += tiles;
                    if self.cost_acc.1 >= 32 {
                        let per = self.cost_acc.0 / self.cost_acc.1 as f64;
                        self.cost_acc = (0.0, 0);
                        self.ms_per_tile = if per > 1.6 * self.ms_per_tile { per } else { 0.7 * self.ms_per_tile + 0.3 * per };
                    }
                }
            match self.gpu_ms.iter_mut().find(|g| g.0 == serial) {
                Some(g) => {
                    if kind == 0 {
                        g.1 = ms
                    } else {
                        g.2 = ms
                    }
                }
                None => self.gpu_ms.push(if kind == 0 { (serial, ms, 0.0) } else { (serial, 0.0, ms) }),
            }
        }
    }

    /// GPU milliseconds for DE tiles this frame. Fixed (`budget_ms`) offscreen; in the window
    /// (`frame_target_ms` set) whatever the render leaves of the refresh period: target minus
    /// the recent render p90, at most 1.6 × `budget_ms`, at least 0.8 ms. The floor keeps the
    /// terrain filling when another process shares the GPU (its work shows up in our command
    /// buffers' GPU times, so the render looks expensive and nothing would be left).
    pub fn de_budget_ms(&self) -> f64 {
        match self.frame_target_ms {
            None => self.set.budget_ms,
            Some(t) => {
                let r: Vec<f64> = self.gpu_ms.iter().rev().filter(|g| g.2 > 0.0).take(40).map(|g| g.2).collect();
                if r.len() < 5 {
                    return self.set.budget_ms;
                }
                (t - crate::stats::pct(&r, 0.9) - 0.3).clamp(0.8, 1.6 * self.set.budget_ms)
            }
        }
    }

    /// The scale the finest clipmap level follows: the zoom S, or less when the eye is lower
    /// than S above the ground (detail follows the eye's distance, at most 5 octaves below S).
    pub fn detail_scale(&self) -> f64 {
        let f = &self.flight;
        // while zooming, extra levels would be obsolete within a second: follow S only
        if f.zoom_rate.abs() > 0.1 {
            return f.s;
        }
        // L2: one sample of the finest level ≈ 2 px at the ground under the eye (L1: 4 px, at
        // most 4 octaves below S); the detail synthesis takes over below that
        (f.hag.max(0.0) * f.s).clamp(f.s / 32.0, f.s)
    }

    /// Keeps the reference orbit near the camera. Tiles store dc = c − c_ref in float32; its
    /// rounding (|dc|·2^-24) must stay well below the finest sample spacing (S/32768), so the
    /// reference may be at most ~48 S away. At 1e-7 with the start's reference 6e-3 away, 4–66 %
    /// of the fine samples were wrong (spikes). Unfinished tiles hold loop state relative to the
    /// old reference: they are dropped and requested again.
    fn maybe_rereference(&mut self) {
        if std::env::var("LRT_NO_REREF").is_ok() {
            return;
        }
        let d = ((self.flight.pos[0] - self.ref_off[0]).powi(2) + (self.flight.pos[1] - self.ref_off[1]).powi(2)).sqrt();
        // also by scale: a nucleus picked at a coarser scale (period 39 at 6e-6) failed for the
        // samples at 1e-7 (66 % wrong) where a nearby nucleus of that scale (period 624) is exact
        if d < 48.0 * self.flight.s && self.flight.s > self.ref_s / 4.0 {
            return;
        }
        let t0 = std::time::Instant::now();
        for (cb, _, _) in &self.inflight {
            cb.waitUntilCompleted();
        }
        self.poll_inflight();
        for (j, k) in std::mem::take(&mut self.resume) {
            self.clip.forget(k);
            self.free_states.push(j.state & 0x7FFF_FFFF);
        }
        if self.choose_reference().is_ok() {
            self.rereferences += 1;
            if std::env::var("LRT_TRACE").is_ok() {
                eprintln!("re-reference at S {:.2e} ({:.1} S away): {} in {:.1} ms", self.flight.s, d / self.flight.s, self.ref_note, t0.elapsed().as_secs_f64() * 1e3);
            }
        }
    }

    /// Collects finished DE batches: tiles with samples left go to the resume queue (with the
    /// resume bit), finished tiles free their state slot.
    fn poll_inflight(&mut self) {
        let mut i = 0;
        while i < self.inflight.len() {
            if self.inflight[i].0.status() != MTLCommandBufferStatus::Completed {
                i += 1;
                continue;
            }
            let (_cb, left, batch) = self.inflight.remove(i);
            // SAFETY: shared buffer of batch.len() u32, written by the completed command buffer.
            let flags = unsafe { std::slice::from_raw_parts(left.contents().as_ptr() as *const u32, batch.len()) };
            for (k, (mut j, key)) in batch.into_iter().enumerate() {
                if flags[k] > 0 {
                    j.state |= 0x8000_0000;
                    self.resume.push((j, key));
                } else {
                    self.free_states.push(j.state & 0x7FFF_FFFF);
                    self.tiles_done += 1;
                    self.clip.mark_done(key);
                }
            }
        }
    }

    /// Advances by `dt` and encodes the frame. Returns the render command buffer (not committed:
    /// the caller adds a present and commits) after committing the compute buffer.
    pub fn frame(&mut self, dt: f64, input: Input, target: &ProtocolObject<dyn MTLTexture>) -> CommandBuffer {
        let t0 = std::time::Instant::now();
        self.serial += 1;
        // zoom-in yields to the visible DE backlog: past 800 missing tiles ahead it slows down
        // (to 15 % at ~3800), so the terrain keeps up: with three missing levels in a row the
        // fallback is flat (tile-shaped blocks popping out of the "lake" on fast zooms)
        // L2: the counts are the backlog *after* the previous frame's batch (L1 counted before
        // scheduling, so each frame's fresh demand throttled the zoom to 0.1–0.15 e-folds/s while
        // the GPU idled)
        let m = self.clip.missing_ahead as f64;
        let near = self.clip.missing_near as f64;
        self.flight.zoom_limit = (1.0 - (m - 300.0) / 2500.0).clamp(0.15, 1.0).min((1.0 - near / 250.0).clamp(0.1, 1.0));
        let set = &self.set;
        self.flight.update(dt, input, &mut self.ground, &|s| sigma_at(set, s).0);
        // pop check: this camera with the previous frame's world (rings, levels, data, fades)
        if let Some(t) = self.popcheck.take() {
            let prev = self.prev_sigma.map(|sg| (sg, self.prev_s));
            let cb = self.encode_view_sigma(&t, self.time, prev);
            cb.commit();
            self.popcheck = Some(t);
        }
        self.prev_sigma = Some(sigma_at(&self.set, self.flight.s).0);
        self.prev_s = self.flight.s;
        self.time += dt;
        self.clip.update(self.flight.pos, self.detail_scale(), self.flight.s);

        // DE tiles within the GPU budget
        self.drain_times();
        let want = (self.de_budget_ms() / self.ms_per_tile.max(1e-4)) as usize;
        self.tiles_per_frame = want.clamp(16, 6000);
        self.poll_inflight();
        self.maybe_rereference();
        // unfinished tiles first (oldest work), then new ones, within the budget and the state pool
        let mut batch: Vec<(TileJob, TileKey)> = Vec::new();
        let resume = std::mem::take(&mut self.resume);
        // at most half the budget for resumed tiles: slow tiles (unresolved interior near
        // minibrots) must not starve the new ones
        let resume_cap = (self.tiles_per_frame / 2).max(8);
        for (j, k) in resume {
            if batch.len() < resume_cap && self.clip.still_wanted(k) {
                batch.push((j, k));
            } else if self.clip.still_wanted(k) {
                self.resume.push((j, k));
            } else {
                self.free_states.push(j.state & 0x7FFF_FFFF);
            }
        }
        self.resumed_total += batch.len() as u64;
        let room = self.tiles_per_frame.saturating_sub(batch.len()).min(self.free_states.len());
        if room > 0 {
            let ctx = self.sched_ctx();
            for (mut j, k) in self.clip.schedule(&ctx, room) {
                j.state = self.free_states.pop().expect("state slot");
                batch.push((j, k));
            }
        } else {
            self.clip.missing = self.clip.missing.max(1);
        }
        if std::env::var("LRT_POOL").is_ok() && self.serial.is_multiple_of(60) {
            eprintln!("t {:.1} free {} resume {} inflight {} batch {} budget {} missing {} ms/tile {:.4}", self.time, self.free_states.len(), self.resume.len(), self.inflight.len(), batch.len(), self.tiles_per_frame, self.clip.missing, self.ms_per_tile);
        }
        let jobs: Vec<TileJob> = batch.iter().map(|b| b.0).collect();
        if !jobs.is_empty() {
            let cb = self.gpu.command_buffer();
            let left = self.gpu.encode_tiles(&cb, &jobs, self.de_params(self.time as f32, jobs.len()));
            self.gpu.track(&cb, self.serial, 0);
            self.pending.push((self.serial, jobs.len()));
            if self.pending.len() > 64 {
                self.pending.remove(0);
            }
            cb.commit();
            if let Some(left) = left {
                self.inflight.push((cb, left, batch));
            }
        }

        let cb = self.encode_view(target, self.time);
        self.recs.push(FrameRec {
            serial: self.serial,
            t: self.time,
            dt,
            cpu_ms: t0.elapsed().as_secs_f64() * 1e3,
            tiles: self.pending.last().filter(|p| p.0 == self.serial).map(|p| p.1).unwrap_or(0),
            missing: self.clip.missing,
            missing_ahead: self.clip.missing_ahead,
            s: self.flight.s,
            riser: sigma_at(&self.set, self.flight.s).1,
            dive_rate: self.flight.zoom_rate,
            blocks: self.blocks.len(),
            hag: self.flight.hag,
        });
        cb
    }

    fn encode_view(&mut self, target: &ProtocolObject<dyn MTLTexture>, time: f64) -> CommandBuffer {
        self.encode_view_sigma(target, time, None)
    }

    /// Encodes the current camera and world state at time `time` (not committed); `sigma`
    /// overrides the height curve's σ (complex units).
    fn encode_view_sigma(&mut self, target: &ProtocolObject<dyn MTLTexture>, time: f64, prev: Option<(f64, f64)>) -> CommandBuffer {
        // `prev`: (σ, S) of the previous frame: the world as it was, seen by this camera
        let world_s = prev.map(|p| p.1).unwrap_or(self.flight.s);
        let (sig_cur, mist) = sigma_at(&self.set, world_s);
        // LRT_POP_COLOUR: the pop check varies only the colour state (geometry σ current), so
        // the diff isolates colour that slides over a fixed surface
        let colour_only = std::env::var("LRT_POP_COLOUR").is_ok();
        let sr = if colour_only { sigma_at(&self.set, self.flight.s).0 } else { prev.map(|p| p.0).unwrap_or(sig_cur) } / self.flight.s;
        let (lo, hi, fade) = sigma_layers(&self.set, world_s);
        // camera and rings
        let (w, h) = (target.width() as f64, target.height() as f64);
        let aspect = w / h;
        let tan_half = (self.set.fov.to_radians() * 0.5).tan();
        let (f, r, u) = self.flight.basis();
        let eye_z = self.flight.z / self.flight.s;
        let inv_2r = 1.0 / (2.0 * self.set.curvature);
        let view = ViewCtx {
            cam: self.flight.pos,
            s: self.flight.s,
            eye_z,
            fwd: f,
            right: r,
            up: u,
            tan_half,
            aspect,
            inv_2r,
            cull_dist: self.set.fog * 6.0,
            hcurve: (sr, K_HEIGHT / sr),
        };
        let rings = self.clip.rings(&view, &mut self.blocks);
        self.last_blocks = self.blocks.len();
        let near = 0.002;
        let e = [0.0, 0.0, eye_z];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        // rows of P·V (reversed-Z infinite projection), then transposed into columns
        let row0 = [r[0] / tan_half, r[1] / tan_half, r[2] / tan_half, -dot(r, e) / tan_half];
        let row1 = [u[0] * aspect / tan_half, u[1] * aspect / tan_half, u[2] * aspect / tan_half, -dot(u, e) * aspect / tan_half];
        let row2 = [0.0, 0.0, 0.0, near];
        let row3 = [f[0], f[1], f[2], -dot(f, e)];
        let rows = [row0, row1, row2, row3];
        let mut vp = [[0f32; 4]; 4];
        for c in 0..4 {
            for rr in 0..4 {
                vp[c][rr] = rows[rr][c] as f32;
            }
        }
        let (az, el) = (self.set.sun_az.to_radians(), self.set.sun_el.to_radians());
        let sun = [az.cos() * el.cos(), az.sin() * el.cos(), el.sin()];
        let g = Globals {
            view_proj: vp,
            // mist while the world changes layer: the fog closes in mid-transition
            sun: [sun[0] as f32, sun[1] as f32, sun[2] as f32, (self.set.fog * (1.0 - 0.8 * mist)) as f32],
            eye: [0.0, 0.0, eye_z as f32, inv_2r as f32],
            fwd: [f[0] as f32, f[1] as f32, f[2] as f32, tan_half as f32],
            right: [r[0] as f32, r[1] as f32, r[2] as f32, aspect as f32],
            up: [u[0] as f32, u[1] as f32, u[2] as f32, time as f32],
            hcurve: [sr as f32, (K_HEIGHT / sr) as f32, self.set.fade as f32, self.set.n as f32],
            shade: [
                (world_s / self.flight.s).log2() as f32,
                self.flight.s.log2() as f32,
                (lo / self.flight.s) as f32,
                (hi / self.flight.s) as f32,
            ],
            // crosshair at the screen centre (the nose = the flight path); radius grows while zooming
            overlay: [
                (w * 0.5) as f32,
                (h * 0.5) as f32,
                (h * 0.012) as f32,
                self.flight.zoom_rate.clamp(-1.0, 1.0) as f32,
            ],
            shade2: [
                fade as f32,
                if self.set.colour == "legacy" { 1.0 } else { 0.0 },
                match std::env::var("LRT_MASK").as_deref() {
                    Ok("lake") => 1.0,
                    Ok("fallback") => 2.0,
                    _ => 0.0,
                },
                0.0,
            ],
            detail: [
                // largest detail wavelength: 0.05 S (continuous in S: the top octave fades)
                (0.05 * self.flight.s).log2() as f32,
                self.set.detail_amp as f32,
                if self.set.detail_amp > 0.0 { 1.0 } else { 0.0 },
                self.set.ridges as f32,
            ],
            detail2: [
                if self.set.detail_amp > 0.0 && !self.set.no_vertex_detail { 1.0 } else { 0.0 },
                if self.set.no_variation { 0.0 } else { 1.0 },
                self.flight.s.log2() as f32,
                0.0,
            ],
            misc: [
                1.0 / self.set.n as f32,
                // legacy strata: keyed by log2(DE/σ) (the constant keeps the L0 look at σ = 0.1 S)
                -((prev.map(|p| p.0).unwrap_or(sig_cur) / self.flight.s) / SIGMA).log2() as f32 * 0.4,
                if self.set.debug { 1.0 } else { 0.0 },
                std::env::var("LRT_PROFILE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            ],
        };
        let cb = self.gpu.command_buffer();
        self.gpu.encode_render(&cb, target, &g, &rings, &self.blocks);
        self.gpu.track(&cb, self.serial, 1);
        cb
    }

    pub fn hud(&self) -> String {
        let last = self.gpu_ms.iter().rev().take(30).collect::<Vec<_>>();
        let n = last.len().max(1) as f64;
        let c: f64 = last.iter().map(|g| g.1).sum::<f64>() / n;
        let r: f64 = last.iter().map(|g| g.2).sum::<f64>() / n;
        let f = &self.flight;
        format!(
            "{} zoom  S={:.2e}  speed {:.2} (Q/E)  alt {:.2} S  pitch {:.0}°  GPU {:.1}+{:.1} ms  missing {} (ahead {})",
            if f.auto_zoom { "AUTO" } else { "manual" },
            f.s,
            f.speed_target,
            f.hag,
            f.pitch.to_degrees(),
            c,
            r,
            self.clip.missing,
            self.clip.missing_ahead
        )
    }
}

/// Start placement (L0's `--snap boundary`): walk to the boundary, back off 2 S outwards and
/// look towards the set, turned by `turn_heading`.
fn place(set: &Settings) -> ((f64, f64), f64) {
    let c = loc_center(&set.loc);
    let s = set.scale;
    let (b, out) = mandel::snap_to_boundary(c, s * 1e-3, 50_000);
    let start = (b.0 + out.0 * 2.0 * s, b.1 + out.1 * 2.0 * s);
    let heading = (-out.1).atan2(-out.0) + set.turn_heading.to_radians();
    (start, heading)
}


impl Sim {
    /// Compares GPU DE samples with the CPU f64 iteration at `per_level` random texels of every
    /// active level (after a prefill). Prints outliers.
    pub fn check(&mut self, per_level: usize) {
        let n = self.set.n as usize;
        let mut rng = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        // only tiles that are computed: requested, not waiting to resume, GPU done
        for (cb, _, _) in &self.inflight {
            cb.waitUntilCompleted();
        }
        self.poll_inflight();
        let pending: std::collections::HashSet<TileKey> = self.resume.iter().map(|r| r.1).collect();
        for lv in self.clip.levels.clone() {
            let data = self.gpu.read_data_slot(lv.slot, n);
            let sp = crate::clipmap::spacing(lv.l);
            let (mut worst, mut bad, mut lake_mismatch, mut cnt) = (0.0f64, 0, 0, 0);
            for _ in 0..per_level {
                let s = [(next() % n as u64) as i64, (next() % n as u64) as i64];
                let a = [lv.data[0] + s[0], lv.data[1] + s[1]];
                let key = (lv.l, a[0].div_euclid(16), a[1].div_euclid(16));
                if !self.clip.requested(lv.l, a[0], a[1]) || pending.contains(&key) {
                    continue;
                }
                let texel = [a[0].rem_euclid(n as i64) as usize, a[1].rem_euclid(n as i64) as usize];
                let g = data[texel[1] * n + texel[0]] as f64;
                let c = (self.c0.0 + a[0] as f64 * sp, self.c0.1 + a[1] as f64 * sp);
                let e = mandel::eval(c.0, c.1, self.set.max_iter);
                cnt += 1;
                let g_lake = g < -60.0;
                if g_lake != e.interior {
                    lake_mismatch += 1;
                    continue;
                }
                if !g_lake {
                    let cpu = (e.de / sp).log2();
                    let d = (g - cpu).abs();
                    if d > worst {
                        worst = d;
                    }
                    // only errors that change the terrain: DE above 1/64 of the spacing (lower DE
                    // is height ~0 either way; such points are within f32 resolution of the set)
                    if d > 1.0 && g.max(cpu) > -6.0 {
                        bad += 1;
                        if bad <= 3 {
                            eprintln!("    l {} sample {:?}: gpu log2 {:.3} cpu {:.3} (c = {:.17}, {:.17})", lv.l, a, g, cpu, c.0, c.1);
                        }
                    }
                }
            }
            println!("level {:4} (spacing {:.2e}): {} samples, |log2 DE err| max {:.3}, >1: {}, lake mismatch {}", lv.l, sp, cnt, worst, bad, lake_mismatch);
        }
    }
}

/// Cold-prefill throughput at fixed locations, eye 0.04 S above the ground (so the 4 extra fine
/// levels of low flight are included). Prints tiles, wall seconds (GPU-bound: one big batch
/// after another, waited on) and tiles per second.
pub fn bench(set: Settings) -> Result<(), String> {
    let cases: [(&str, f64); 6] = [("seahorse", 1e-3), ("seahorse", 1.76e-6), ("elephant", 1e-5), ("minibrot3", 1e-6), ("neck", 1e-4), ("spiral", 1e-9)];
    let (mut tot_t, mut tot_n) = (0.0, 0usize);
    for (loc, scale) in cases {
        let mut st = set.clone();
        st.loc = loc.into();
        st.scale = scale;
        let mut sim = Sim::new(st, 0.0)?;
        sim.flight.hag = 0.04;
        let t = std::time::Instant::now();
        let n = sim.prefill();
        let dt = t.elapsed().as_secs_f64();
        tot_t += dt;
        tot_n += n;
        println!("{loc:>10} {scale:9.2e}: {n:6} tiles in {dt:6.3} s = {:8.0} tiles/s  ({})", n as f64 / dt, sim.ref_note);
    }
    println!("total {tot_n} tiles in {tot_t:.3} s = {:.0} tiles/s", tot_n as f64 / tot_t);
    Ok(())
}
