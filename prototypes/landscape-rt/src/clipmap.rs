//! Clipmap bookkeeping on the CPU: which absolute levels are active, where each ring and each
//! data window sits, which 16×16 DE tiles are missing, which to compute this frame, and the
//! per-ring draw parameters and visible blocks.
//!
//! World: complex offsets from a fixed origin `c0` (f64). Level ℓ samples sit at `c0 + (i, j)·2^ℓ`
//! for integer i, j: camera-independent, so a sample once computed never changes (L0 anchored
//! its sample grid on the camera's ground point and S every frame, which resampled the whole
//! terrain per frame: the jitter of its dive video).

use crate::gpu::{LevelRef, RingDraw, RingParams, TileJob};

pub const TILE: i64 = 16;
pub const BLOCK: i64 = 16;
/// Finer levels added below the zoom's finest when the eye is low over the ground.
pub const EXTRA_LEVELS: usize = 5;

/// (level, world tile x, world tile y)
pub type TileKey = (i32, i64, i64);

#[derive(Clone, Copy, Debug)]
pub struct ClipConfig {
    /// Data texels per level and axis (multiple of 16·TILE...; 2048).
    pub n: i64,
    /// Vertex stride in samples (2 or 4).
    pub k: i64,
    /// Vertices per ring axis (multiple of 2·BLOCK).
    pub m: i64,
    /// Active levels.
    pub levels: usize,
    /// Texture array slots (> levels).
    pub slots: usize,
    /// Finest sample spacing ≈ S / detail.
    pub detail: f64,
}

impl ClipConfig {
    pub fn new(n: i64, k: i64, levels: usize, detail: f64, ring: i64) -> Self {
        // largest M (multiple of 2·BLOCK) whose ring plus snapping slack fits in the data window
        let mut m = (n / k) / (2 * BLOCK) * (2 * BLOCK);
        if ring > 0 {
            m = m.min(ring / (2 * BLOCK) * (2 * BLOCK));
        }
        while m > 0 && m * k + 32 * k + TILE + 4 > n {
            m -= 2 * BLOCK;
        }
        let c = ClipConfig { n, k, m, levels, slots: levels + EXTRA_LEVELS + 1, detail };
        assert!(c.m >= 4 * BLOCK, "data window too small for the ring");
        assert!(c.n % (TILE * 8) == 0);
        c
    }
    /// Ring origins are multiples of this many samples (so nested rings and their holes align
    /// with 16×16-quad blocks of the coarser ring).
    pub fn snap(&self) -> i64 {
        2 * BLOCK * self.k
    }
    pub fn tiles(&self) -> i64 {
        self.n / TILE
    }
    /// Data window origin = floor16(cam − data_off): covers the ring with all slack.
    fn data_off(&self) -> i64 {
        let half = self.m * self.k / 2;
        let lo = half + self.snap() + 1; // need data_off >= lo
        let hi = self.n - TILE - half - 2; // and data_off <= hi
        assert!(lo <= hi, "data window too small");
        (lo + hi) / 2
    }
    /// L∞ distance (in vertex spacings) where α reaches 1, and the transition width.
    fn alpha_vertices(&self) -> (f64, f64) {
        let r_min = (self.m / 2 - 2 * BLOCK) as f64; // nearest possible ring edge
        let hole_max = (self.m / 4 + BLOCK) as f64; // farthest possible finer-ring edge
        let w = ((r_min - hole_max) * 0.9).min(self.m as f64 * 0.15);
        (r_min, w)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub l: i32,
    pub slot: usize,
    /// Ring vertex (0,0) as a level-ℓ sample index (multiple of snap()).
    pub geo: [i64; 2],
    /// Data window origin (multiple of TILE).
    pub data: [i64; 2],
}

pub fn spacing(l: i32) -> f64 {
    (l as f64).exp2()
}

fn level_bits(l: i32) -> u32 {
    (((l & 63) + 1) as u32) << 16
}

fn tile_tag(l: i32, div: [i64; 2]) -> u32 {
    level_bits(l) | (((div[0] & 255) as u32) << 8) | ((div[1] & 255) as u32)
}

/// Status bit: the tile's GPU job has finished (all 256 samples written). Without it, the tag
/// alone means "requested" (computing, or waiting to resume).
const DONE: u32 = 0x8000_0000;

pub struct Candidate {
    score: f64,
    level: u16,
    tx: i64,
    ty: i64,
    d3: f64,
    vis: f64,
}

pub struct Clipmap {
    pub cfg: ClipConfig,
    pub finest: Option<i32>,
    base_finest: Option<i32>,
    pub levels: Vec<Level>,
    status: Vec<Vec<u32>>,
    slot_level: Vec<Option<i32>>,
    pub missing: usize,
    /// unscheduled missing tiles in the view frustum and within 2× the fog (after this frame's batch)
    pub missing_ahead: usize,
    /// unscheduled missing tiles within 1.5 S of the eye (3D, any level)
    pub missing_near: usize,
    /// missing tiles that wait for their parent tile (not schedulable yet)
    pub blocked: usize,
    cands: Vec<Candidate>,
}

pub struct ScheduleCtx {
    pub cam: [f64; 2],
    pub s: f64,
    pub ref_off: [f64; 2],
    pub fog: f64,
    /// eye height above the ground, S
    pub hag: f64,
    /// camera basis (forward, right, up) and tan of the half FOV (horizontal) and aspect
    pub fwd: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub tan_half: f64,
    pub aspect: f64,
}

pub struct ViewCtx {
    pub cam: [f64; 2],
    pub s: f64,
    pub eye_z: f64,
    pub fwd: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub tan_half: f64,
    pub aspect: f64,
    pub inv_2r: f64,
    pub cull_dist: f64,
    pub hcurve: (f64, f64),
}

impl Clipmap {
    pub fn new(cfg: ClipConfig) -> Self {
        let t = (cfg.tiles() * cfg.tiles()) as usize;
        Clipmap {
            cfg,
            finest: None,
            base_finest: None,
            levels: Vec::new(),
            status: vec![vec![u32::MAX; t]; cfg.slots],
            slot_level: vec![None; cfg.slots],
            missing: 0,
            missing_ahead: 0,
            missing_near: 0,
            blocked: 0,
            cands: Vec::new(),
        }
    }

    /// Picks the active levels (with hysteresis) and places rings and data windows: the
    /// coarsest follows the zoom `s` (far coverage, fog), the finest follows `s_detail` (the
    /// eye's distance to the ground; ≤ s), at most EXTRA_LEVELS below the zoom's finest.
    pub fn update(&mut self, cam: [f64; 2], s_detail: f64, s: f64) {
        let hyst = |cur: Option<i32>, want: f64| match cur {
            None => want.floor() as i32,
            Some(f) if want < f as f64 - 0.15 || want > f as f64 + 1.15 => want.floor() as i32,
            Some(f) => f,
        };
        let base = hyst(self.base_finest, (s / self.cfg.detail).log2());
        self.base_finest = Some(base);
        let finest = hyst(self.finest, (s_detail / self.cfg.detail).log2()).clamp(base - EXTRA_LEVELS as i32, base);
        self.finest = Some(finest);
        let cfg = self.cfg;
        self.levels.clear();
        let count = cfg.levels + (base - finest) as usize;
        for i in 0..count {
            let l = finest + i as i32;
            let slot = l.rem_euclid(cfg.slots as i32) as usize;
            if self.slot_level[slot] != Some(l) {
                self.slot_level[slot] = Some(l);
                self.status[slot].fill(u32::MAX);
            }
            let sp = spacing(l);
            let cs = [cam[0] / sp, cam[1] / sp];
            let half = (cfg.m * cfg.k / 2) as f64;
            let snap = cfg.snap() as f64;
            let geo = [((cs[0] - half) / snap).floor() as i64 * cfg.snap(), ((cs[1] - half) / snap).floor() as i64 * cfg.snap()];
            let off = cfg.data_off() as f64;
            let data = [((cs[0] - off) / TILE as f64).floor() as i64 * TILE, ((cs[1] - off) / TILE as f64).floor() as i64 * TILE];
            debug_assert!(data[0] < geo[0] && data[0] + cfg.n > geo[0] + cfg.m * cfg.k + 1);
            self.levels.push(Level { l, slot, geo, data });
        }
    }

    /// Collects missing tiles, picks the `budget` most urgent ones and returns their jobs
    /// (marking them as requested).
    ///
    /// Priority (L2): a tile is schedulable only when its parent tile (level ℓ+1) is done, so
    /// the valid levels at any point always form a complete pyramid from the coarsest level
    /// down, and the shaders' fallback is never more than the pending depth away. Among
    /// schedulable tiles the score is the screen-space error the tile removes: the parent's
    /// sample spacing over the 3D distance from the eye (the eye height counts: flying low, the
    /// ground under the camera is the nearest), times a visibility factor from the view
    /// frustum (behind the camera 0.1, just outside the view 0.35), and fogged far tiles last.
    pub fn schedule(&mut self, ctx: &ScheduleCtx, budget: usize) -> Vec<(TileJob, TileKey)> {
        let cfg = self.cfg;
        let tn = cfg.tiles();
        self.cands.clear();
        let (mut blocked, mut b_ahead, mut b_near) = (0usize, 0usize, 0usize);
        let ez = ctx.hag.max(0.005);
        let tv = ctx.tan_half / ctx.aspect;
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for (li, lv) in self.levels.iter().enumerate() {
            let sp = spacing(lv.l);
            let parent = self.levels.get(li + 1).copied();
            let st = &self.status[lv.slot];
            let t0 = [lv.data[0] / TILE, lv.data[1] / TILE];
            let sp_s = sp / ctx.s;
            let half = 0.5 * TILE as f64 * sp_s; // tile half-width, S
            for ty in t0[1]..t0[1] + tn {
                let row = ty.rem_euclid(tn) * tn;
                let divy = ty.div_euclid(tn);
                for tx in t0[0]..t0[0] + tn {
                    let idx = (row + tx.rem_euclid(tn)) as usize;
                    let tag = tile_tag(lv.l, [tx.div_euclid(tn), divy]);
                    if st[idx] & !DONE == tag {
                        continue;
                    }
                    // tile centre relative to the camera, S
                    let cx = ((tx * TILE + TILE / 2) as f64 * sp - ctx.cam[0]) / ctx.s;
                    let cy = ((ty * TILE + TILE / 2) as f64 * sp - ctx.cam[1]) / ctx.s;
                    let dh = ((cx * cx + cy * cy).sqrt() - half * std::f64::consts::SQRT_2).max(0.0);
                    let d3 = (dh * dh + ez * ez).sqrt();
                    // error removed: the parent's spacing (what is shown now) over the distance
                    let mut score = 2.0 * sp_s / d3.max(1e-9);
                    // visibility: the tile's nearest point relative to the frustum
                    let rel = [cx, cy, -ez];
                    let z = dot(rel, ctx.fwd);
                    let x = dot(rel, ctx.right).abs();
                    let y = dot(rel, ctx.up).abs();
                    let r = half * 1.5;
                    let outside = ((x - r) / (z.max(0.0) + r)).max(0.0) / ctx.tan_half.max(1e-3) + ((y - r) / (z.max(0.0) + r)).max(0.0) / tv.max(1e-3);
                    let vis = if z < -r { 0.1 } else if outside <= 1.0 { 1.0 } else if outside < 1.6 { 0.35 } else { 0.15 };
                    score *= vis;
                    if dh > 4.0 * ctx.fog {
                        score *= 0.1;
                    }
                    if let Some(p) = parent {
                        let (px, py) = (tx.div_euclid(2), ty.div_euclid(2));
                        let pidx = (py.rem_euclid(tn) * tn + px.rem_euclid(tn)) as usize;
                        if self.status[p.slot][pidx] != tile_tag(p.l, [px.div_euclid(tn), py.div_euclid(tn)]) | DONE {
                            blocked += 1;
                            b_ahead += (vis >= 1.0 && d3 < 2.0 * ctx.fog) as usize;
                            b_near += (d3 < 1.5) as usize;
                            continue;
                        }
                    }
                    self.cands.push(Candidate { score, level: li as u16, tx, ty, d3, vis });
                }
            }
        }
        let take = budget.min(self.cands.len());
        if take < self.cands.len() && take > 0 {
            self.cands.select_nth_unstable_by(take - 1, |a, b| b.score.total_cmp(&a.score));
        }
        // backlog after this frame's batch (what the zoom-in rate yields to): unscheduled
        // schedulable tiles plus the blocked ones
        self.missing = self.cands.len() - take + blocked;
        self.blocked = blocked;
        self.missing_ahead = b_ahead;
        self.missing_near = b_near;
        for c in self.cands.iter().skip(take) {
            if c.vis >= 1.0 && c.d3 < 2.0 * ctx.fog {
                self.missing_ahead += 1;
            }
            if c.d3 < 1.5 {
                self.missing_near += 1;
            }
        }
        let mut jobs = Vec::with_capacity(take);
        for c in self.cands.iter().take(take) {
            let lv = self.levels[c.level as usize];
            let sp = spacing(lv.l);
            let idx = (c.ty.rem_euclid(tn) * tn + c.tx.rem_euclid(tn)) as usize;
            let tag = tile_tag(lv.l, [c.tx.div_euclid(tn), c.ty.div_euclid(tn)]);
            self.status[lv.slot][idx] = tag;
            let x = (c.tx * TILE) as f64 * sp - ctx.ref_off[0];
            let y = (c.ty * TILE) as f64 * sp - ctx.ref_off[1];
            let parent = self.levels.get(c.level as usize + 1);
            let n = cfg.n;
            jobs.push((TileJob {
                ptexel: [((c.tx * TILE / 2).rem_euclid(n)) as u32, ((c.ty * TILE / 2).rem_euclid(n)) as u32],
                pslot: parent.map_or(u32::MAX, |p| p.slot as u32),
                pad: 0,
                dc0: [x as f32, y as f32],
                spacing: sp as f32,
                slot: lv.slot as u32,
                texel: [(c.tx.rem_euclid(tn) * TILE) as u32, (c.ty.rem_euclid(tn) * TILE) as u32],
                tag: tag as f32,
                state: 0,
            }, (lv.l, c.tx, c.ty)));
        }
        jobs
    }

    /// Marks a requested tile as done (its job finished).
    pub fn mark_done(&mut self, key: TileKey) {
        let (l, tx, ty) = key;
        let tn = self.cfg.tiles();
        let tag = tile_tag(l, [tx.div_euclid(tn), ty.div_euclid(tn)]);
        let idx = (ty.rem_euclid(tn) * tn + tx.rem_euclid(tn)) as usize;
        for lv in &self.levels {
            if lv.l == l && self.status[lv.slot][idx] == tag {
                self.status[lv.slot][idx] = tag | DONE;
            }
        }
    }

    /// Ring draw parameters and visible blocks (frustum-culled), finest ring first.
    pub fn rings(&self, v: &ViewCtx, blocks: &mut Vec<[u16; 2]>) -> Vec<RingDraw> {
        let cfg = self.cfg;
        let nb = cfg.m / BLOCK;
        let (r_min_v, w_v) = cfg.alpha_vertices();
        let mut out = Vec::with_capacity(self.levels.len());
        blocks.clear();
        let sin_side = 1.0 / (1.0 + v.tan_half * v.tan_half).sqrt();
        let tv = v.tan_half / v.aspect;
        let sin_vert = 1.0 / (1.0 + tv * tv).sqrt();
        for (i, lv) in self.levels.iter().enumerate() {
            let sp = spacing(lv.l);
            let g_s = cfg.k as f64 * sp / v.s;
            let pos0 = [(lv.geo[0] as f64 * sp - v.cam[0]) / v.s, (lv.geo[1] as f64 * sp - v.cam[1]) / v.s];
            let mut p = RingParams {
                pos0: [pos0[0] as f32, pos0[1] as f32],
                g_s: g_s as f32,
                k: cfg.k as i32,
                alpha_start: ((r_min_v - w_v) * g_s) as f32,
                alpha_width: (w_v * g_s) as f32,
                block_verts: BLOCK as i32,
                ..Default::default()
            };
            if i + 1 == self.levels.len() {
                p.alpha_start = 1e30; // no coarser ring
            }
            // the finer levels (fragment shading only) and every coarser active level: the shaders
            // fall back through all of them (L2)
            let n = cfg.n;
            // all finer levels: the fragment picks its level by footprint and validity alone, so
            // the shading at a point does not depend on which ring draws it (when a ring snapped,
            // pixels changing ring changed level: the pop check's worst frames)
            let fine = std::env::var("LRT_FINE").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX).min(i);
            let mut count = 0;
            for j in 0..crate::gpu::MAXLV {
                let k = j as i64 - fine as i64; // level relative to the ring's
                if let Some(c) = self.levels.get((i as i64 + k) as usize) {
                    let sh = (k as f64).exp2();
                    p.lv[j] = LevelRef {
                        data_mod: [c.data[0].rem_euclid(n) as i32, c.data[1].rem_euclid(n) as i32],
                        data_div: [c.data[0].div_euclid(n) as i32, c.data[1].div_euclid(n) as i32],
                        ro: [(lv.geo[0] as f64 / sh - c.data[0] as f64) as f32, (lv.geo[1] as f64 / sh - c.data[1] as f64) as f32],
                        sp_over_s: (spacing(c.l) / v.s) as f32,
                        slot: c.slot as u32,
                        level_bits: level_bits(c.l),
                        lvl: c.l,
                    };
                    count = j + 1;
                } else {
                    p.lv[j] = LevelRef { sp_over_s: 1.0, level_bits: 0xFFFF_FFFF, ..Default::default() };
                }
            }
            p.n_lv = count as i32;
            p.fine = fine as i32;
            p.lvl = lv.l;
            // detail lattice: absolute sample index of data-local 0 (level ℓ) in octave units,
            // mod 128 cells, exact; plus a fixed per-octave offset that decorrelates octaves
            for (j, b) in p.nbase.iter_mut().enumerate() {
                let oct = lv.l + j as i32 - 6;
                let sh = lv.l - oct;
                let per = crate::gpu::NOISE_CELLS as i64;
                let off = [((oct * 37).rem_euclid(128)) as f64, ((oct * 71 + 13).rem_euclid(128)) as f64];
                for a in 0..2 {
                    let d = lv.data[a];
                    let x = if sh >= 0 {
                        ((d % per) << sh).rem_euclid(per) as f64
                    } else {
                        let n = -sh;
                        let m = per << n;
                        d.rem_euclid(m) as f64 / (1i64 << n) as f64
                    };
                    b[a] = ((x + off[a]) % per as f64) as f32;
                }
            }
            // hole: the finer ring's footprint, in blocks of this ring
            let hole = if i > 0 {
                let f = self.levels[i - 1];
                let hx = ((f.geo[0] / 2) - lv.geo[0]) / (cfg.k * BLOCK);
                let hy = ((f.geo[1] / 2) - lv.geo[1]) / (cfg.k * BLOCK);
                debug_assert!(((f.geo[0] / 2) - lv.geo[0]) % (cfg.k * BLOCK) == 0);
                Some((hx, hy, nb / 2))
            } else {
                None
            };
            let first = blocks.len();
            let bw = BLOCK as f64 * g_s;
            for by in 0..nb {
                for bx in 0..nb {
                    if let Some((hx, hy, hs)) = hole
                        && bx >= hx
                        && bx < hx + hs
                        && by >= hy
                        && by < hy + hs
                    {
                        continue;
                    }
                    let x0 = pos0[0] + bx as f64 * bw;
                    let y0 = pos0[1] + by as f64 * bw;
                    if !block_visible(v, x0, y0, bw, sin_side, sin_vert) {
                        continue;
                    }
                    blocks.push([bx as u16, by as u16]);
                }
            }
            out.push(RingDraw { params: p, first_block: first, blocks: blocks.len() - first });
        }
        out
    }

    /// Whether the tile holding level-l sample (i, j) has been requested for this window.
    pub fn requested(&self, l: i32, i: i64, j: i64) -> bool {
        self.still_wanted((l, i.div_euclid(TILE), j.div_euclid(TILE)))
    }

    /// Marks a requested tile as missing again (its job was dropped).
    pub fn forget(&mut self, key: TileKey) {
        let (l, tx, ty) = key;
        let tn = self.cfg.tiles();
        for lv in &self.levels {
            if lv.l == l {
                let idx = (ty.rem_euclid(tn) * tn + tx.rem_euclid(tn)) as usize;
                self.status[lv.slot][idx] = u32::MAX;
            }
        }
    }

    /// Whether a tile (level, world tile coords) is still in an active data window and marked
    /// as requested (an unfinished tile is resumed only then).
    pub fn still_wanted(&self, key: TileKey) -> bool {
        let (l, tx, ty) = key;
        let tn = self.cfg.tiles();
        self.levels.iter().any(|lv| {
            lv.l == l
                && tx >= lv.data[0] / TILE
                && tx < lv.data[0] / TILE + tn
                && ty >= lv.data[1] / TILE
                && ty < lv.data[1] / TILE + tn
                && self.status[lv.slot][(ty.rem_euclid(tn) * tn + tx.rem_euclid(tn)) as usize] & !DONE == tile_tag(l, [tx.div_euclid(tn), ty.div_euclid(tn)])
        })
    }

}

fn block_visible(v: &ViewCtx, x0: f64, y0: f64, bw: f64, sin_side: f64, sin_vert: f64) -> bool {
    let (cx, cy) = (x0 + 0.5 * bw, y0 + 0.5 * bw);
    let gap = |a: f64| if a > 0.0 { a } else if a + bw < 0.0 { -(a + bw) } else { 0.0 };
    let dmin = (gap(x0).powi(2) + gap(y0).powi(2)).sqrt();
    let dmax = ((x0.abs().max((x0 + bw).abs())).powi(2) + (y0.abs().max((y0 + bw).abs())).powi(2)).sqrt();
    if dmin > v.cull_dist {
        return false;
    }
    // height range: the log height curve is bounded by h(DE ~ distance), minus the curvature drop
    // DE can exceed the distance by far over plateaus: bound by DE/S = 1e9 (h ≈ 2.4 S)
    let _ = dmax;
    let hmax = v.hcurve.0 * (1.0 + v.hcurve.1 * 1e9).ln();
    let z_lo = -dmax * dmax * v.inv_2r;
    let z_hi = hmax - dmin * dmin * v.inv_2r;
    let cz = 0.5 * (z_lo + z_hi);
    let half_d = 0.5 * bw * std::f64::consts::SQRT_2;
    let rad = (half_d * half_d + 0.25 * (z_hi - z_lo).powi(2)).sqrt();
    let rel = [cx, cy, cz - v.eye_z];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let z = dot(rel, v.fwd);
    if z < -rad {
        return false;
    }
    let x = dot(rel, v.right);
    let y = dot(rel, v.up);
    // plane distance for |x| = z·t: (|x| − z t)/sqrt(1+t²) = (|x| − z t)·sin
    if (x.abs() - z * v.tan_half) * sin_side > rad {
        return false;
    }
    let tv = v.tan_half / v.aspect;
    if (y.abs() - z * tv) * sin_vert > rad {
        return false;
    }
    true
}
