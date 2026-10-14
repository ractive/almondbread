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
    pub blocks: usize,
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
    pub ref_note: String,
    pub time: f64,
    pub serial: u64,
    /// EMA of GPU ms per DE tile
    pub ms_per_tile: f64,
    pub tiles_per_frame: usize,
    /// (serial, tiles) of submitted compute buffers awaiting their time
    pending: Vec<(u64, usize)>,
    pub gpu_ms: Vec<(u64, f64, f64)>, // serial, compute ms, render ms
    blocks: Vec<[u16; 2]>,
    pub recs: Vec<FrameRec>,
    pub last_blocks: usize,
    /// coast-following steering assist
    pub autopilot: bool,
    ap_side: f64,
    /// σ of the previous frame (pop check renders the previous world)
    prev_sigma: Option<f64>,
    free_states: Vec<u32>,
    resume: Vec<(TileJob, TileKey)>,
    inflight: Vec<(CommandBuffer, Buffer, Vec<(TileJob, TileKey)>)>,
    pub resumed_total: u64,
    /// When set, each frame first renders its camera with the previous world state into it.
    pub popcheck: Option<crate::gpu::Texture>,
}

const SIGMA: f64 = 0.1;
const K_HEIGHT: f64 = 4.0;
const K_OVER_SIGMA: f64 = 40.0;

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
        let cfg = ClipConfig::new(set.n, set.k, set.levels, set.detail);
        let gpu = Gpu::new(cfg.n as usize, cfg.slots, crate::clipmap::BLOCK as usize)?;
        let (c0, heading) = place(&set);
        let mut sim = Sim {
            clip: Clipmap::new(cfg),
            flight: Flight::new(set.scale, heading, start_speed),
            ground: Ground::new(c0, SIGMA, K_OVER_SIGMA),
            prev_sigma: None,
            gpu,
            c0,
            ref_off: [0.0, 0.0],
            ref_note: String::new(),
            time: 0.0,
            serial: 0,
            ms_per_tile: 0.01,
            tiles_per_frame: 64,
            pending: Vec::new(),
            gpu_ms: Vec::new(),
            blocks: Vec::new(),
            recs: Vec::new(),
            last_blocks: 0,
            autopilot: false,
            ap_side: 1.0,
            popcheck: None,
            free_states: (0..crate::gpu::STATE_SLOTS as u32).rev().collect(),
            resume: Vec::new(),
            inflight: Vec::new(),
            resumed_total: 0,
            set,
        };
        sim.choose_reference()?;
        sim.set_ground_sigma();
        sim.flight.settle(&mut sim.ground);
        Ok(sim)
    }

    /// Reference orbit: the nucleus of the lowest-period minibrot near the camera (periodic,
    /// never escapes, so rebasing wraps it), else the camera point's own orbit.
    pub fn choose_reference(&mut self) -> Result<(), String> {
        let c = (self.c0.0 + self.flight.pos[0], self.c0.1 + self.flight.pos[1]);
        let s = self.flight.s;
        let mut chosen = None;
        let mut r = 4.0 * s;
        for _ in 0..8 {
            if let Some((nuc, period, _size)) = mandel::find_minibrot(c, r, 300_000) {
                let d = (nuc.0 - c.0).hypot(nuc.1 - c.1);
                if d < 4.0 * r && period >= 1 {
                    chosen = Some((nuc, period));
                    break;
                }
            }
            r *= 4.0;
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
            pad: 0,
        }
    }

    fn sched_ctx(&self) -> ScheduleCtx {
        ScheduleCtx { cam: self.flight.pos, s: self.flight.s, heading: self.flight.heading, ref_off: self.ref_off, fog: self.set.fog }
    }

    /// Computes every missing tile now (cold start), already faded in.
    pub fn prefill(&mut self) -> usize {
        self.clip.update(self.flight.pos, self.flight.s);
        let mut total = 0;
        loop {
            let ctx = self.sched_ctx();
            let jobs = self.clip.schedule(&ctx, 4096);
            if jobs.is_empty() {
                break;
            }
            total += jobs.len();
            let jobs: Vec<TileJob> = jobs.iter().enumerate().map(|(i, j)| TileJob { state: i as u32, ..j.0 }).collect();
            let cb = self.gpu.command_buffer();
            let mut p = self.de_params(-1e6, jobs.len());
            p.chunk = self.set.max_iter;
            self.gpu.encode_tiles(&cb, &jobs, p);
            cb.commit();
            cb.waitUntilCompleted();
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
                    if tiles > 0 {
                        // fast rise, slow decay: a costly batch (minibrot shores, slow interior)
                        // must shrink the next ones at once, or the GPU misses refreshes
                        let per = ms / tiles as f64;
                        self.ms_per_tile = if per > self.ms_per_tile { per } else { 0.95 * self.ms_per_tile + 0.05 * per };
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
                }
            }
        }
    }

    /// Advances by `dt` and encodes the frame. Returns the render command buffer (not committed:
    /// the caller adds a present and commits) after committing the compute buffer.
    pub fn frame(&mut self, dt: f64, input: Input, target: &ProtocolObject<dyn MTLTexture>) -> CommandBuffer {
        let t0 = std::time::Instant::now();
        self.serial += 1;
        let mut input = input;
        if self.autopilot {
            input.turn = (self.autopilot_turn() + 0.35 * input.turn).clamp(-1.0, 1.0);
        }
        self.set_ground_sigma();
        self.flight.update(dt, input, &mut self.ground);
        self.set_ground_sigma();
        // pop check: this camera with the previous frame's world (rings, levels, data, fades)
        if let Some(t) = self.popcheck.take() {
            let cur = self.prev_sigma;
            let cb = self.encode_view_sigma(&t, self.time, cur);
            cb.commit();
            self.popcheck = Some(t);
        }
        self.prev_sigma = Some(sigma_at(&self.set, self.flight.s).0);
        self.time += dt;
        self.clip.update(self.flight.pos, self.flight.s);

        // DE tiles within the GPU budget
        self.drain_times();
        let want = (self.set.budget_ms / self.ms_per_tile.max(1e-4)) as usize;
        self.tiles_per_frame = want.clamp(16, 6000);
        self.poll_inflight();
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
            blocks: self.blocks.len(),
        });
        cb
    }

    fn set_ground_sigma(&mut self) {
        let sr = sigma_at(&self.set, self.flight.s).0 / self.flight.s;
        self.ground.sigma = sr;
        self.ground.k_over_sigma = K_HEIGHT / sr;
    }

    fn encode_view(&mut self, target: &ProtocolObject<dyn MTLTexture>, time: f64) -> CommandBuffer {
        self.encode_view_sigma(target, time, None)
    }

    /// Encodes the current camera and world state at time `time` (not committed); `sigma`
    /// overrides the height curve's σ (complex units).
    fn encode_view_sigma(&mut self, target: &ProtocolObject<dyn MTLTexture>, time: f64, sigma: Option<f64>) -> CommandBuffer {
        let (sig_cur, mist) = sigma_at(&self.set, self.flight.s);
        let sr = sigma.unwrap_or(sig_cur) / self.flight.s;
        // camera and rings
        let (w, h) = (target.width() as f64, target.height() as f64);
        let aspect = w / h;
        let tan_half = (self.set.fov.to_radians() * 0.5).tan();
        let (f, r, u) = self.flight.basis();
        let eye_z = self.flight.y;
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
            misc: [
                1.0 / self.set.n as f32,
                // strata keyed by log2(DE/σ) (σ-relative, so they slide only with σ); the
                // constant keeps the L0 look at σ = 0.1 S
                -(sr / SIGMA).log2() as f32 * 0.4,
                if self.set.debug { 1.0 } else { 0.0 },
                std::env::var("LRT_PROFILE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            ],
        };
        let cb = self.gpu.command_buffer();
        self.gpu.encode_render(&cb, target, &g, &rings, &self.blocks);
        self.gpu.track(&cb, self.serial, 1);
        cb
    }

    /// Steering that keeps the camera near the boundary: straight towards the set when far
    /// (DE ≫ S), along the coast (set on one side) when close. Smoothed DE on the CPU grid.
    fn autopilot_turn(&mut self) -> f64 {
        let s = self.flight.s;
        let p = self.flight.pos;
        let sc = 0.25 * s;
        let x = self.ground.de_smooth(p, sc) / s;
        let e = 0.5 * s;
        let gx = self.ground.de_smooth([p[0] + e, p[1]], sc) - self.ground.de_smooth([p[0] - e, p[1]], sc);
        let gy = self.ground.de_smooth([p[0], p[1] + e], sc) - self.ground.de_smooth([p[0], p[1] - e], sc);
        if gx == 0.0 && gy == 0.0 {
            return 0.0;
        }
        let inward = (-gy).atan2(-gx);
        let off = 80f64.to_radians() * ((0.9 - x) / 0.7).clamp(0.0, 1.0);
        let wrap = |a: f64| (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        let h = self.flight.heading;
        // keep the side unless the other one is much closer to the current heading
        let a = wrap(inward + self.ap_side * off - h);
        let b = wrap(inward - self.ap_side * off - h);
        if b.abs() + 2.0 < a.abs() {
            self.ap_side = -self.ap_side;
        }
        let d = wrap(inward + self.ap_side * off - h);
        if std::env::var("LRT_TRACE").is_ok() && self.serial.is_multiple_of(30) {
            eprintln!("t {:.1} S {:.2e} x {:.3} inward {:.0} heading {:.0} d {:.0} y {:.2}", self.time, s, x, inward.to_degrees(), h.to_degrees(), d.to_degrees(), self.flight.y);
        }
        (d * 1.2).clamp(-1.0, 1.0)
    }

    pub fn hud(&self) -> String {
        let last = self.gpu_ms.iter().rev().take(30).collect::<Vec<_>>();
        let n = last.len().max(1) as f64;
        let c: f64 = last.iter().map(|g| g.1).sum::<f64>() / n;
        let r: f64 = last.iter().map(|g| g.2).sum::<f64>() / n;
        format!(
            "{}S={:.2e}  speed {:.2} S/s  alt {:.2} S  GPU de {:.1} + draw {:.1} ms  tiles {}/frame  missing {} (ahead {})",
            if self.autopilot { "[auto] " } else { "" },
            self.flight.s,
            self.flight.speed,
            self.flight.y - self.flight.ground_here,
            c,
            r,
            self.tiles_per_frame,
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

fn f16_to_f32(h: u16) -> f32 {
    let s = (h >> 15) as u32;
    let e = ((h >> 10) & 0x1f) as u32;
    let m = (h & 0x3ff) as u32;
    let bits = if e == 0 {
        if m == 0 { s << 31 } else {
            // subnormal
            let mut e2 = 127 - 15 + 1;
            let mut m2 = m;
            while m2 & 0x400 == 0 { m2 <<= 1; e2 -= 1; }
            (s << 31) | ((e2 as u32) << 23) | ((m2 & 0x3ff) << 13)
        }
    } else if e == 31 { (s << 31) | 0x7f80_0000 | (m << 13) } else { (s << 31) | ((e + 127 - 15) << 23) | (m << 13) };
    f32::from_bits(bits)
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
        for lv in self.clip.levels.clone() {
            let data = self.gpu.read_data_slot(lv.slot, n);
            let sp = crate::clipmap::spacing(lv.l);
            let (mut worst, mut bad, mut lake_mismatch, mut cnt) = (0.0f64, 0, 0, 0);
            for _ in 0..per_level {
                let s = [(next() % n as u64) as i64, (next() % n as u64) as i64];
                let a = [lv.data[0] + s[0], lv.data[1] + s[1]];
                let texel = [a[0].rem_euclid(n as i64) as usize, a[1].rem_euclid(n as i64) as usize];
                let g = f16_to_f32(data[texel[1] * n + texel[0]]) as f64;
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
