//! Height field h(c) = f(DE(c)) sampled lazily on a quadtree of LOD tiles.
//!
//! World frame: x = Re, y = Im, measured from the camera's ground point `c0` in units
//! of the camera scale `S` (the altitude). Level `L` has sample spacing `2^L` (world
//! units); a tile holds `TILE+1` × `TILE+1` samples so every bilinear cell lies in one
//! tile. Tiles are computed on first use and memoised (one `OnceLock` per tile, so two
//! threads never compute the same tile).

use crate::mandel;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

pub const TILE: i64 = 32;
const TS: usize = TILE as usize + 1;
pub const LMIN: i32 = -16;
pub const LMAX: i32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum HeightFn {
    /// h = k·DE: the only fixed-world function that is exactly self-similar.
    Linear,
    /// h = s·ln(1 + k·DE/s), s = sigma·S: linear near the floor, flattens far away.
    Log,
    /// h = k·sqrt(s·DE), s = sigma·S: steep canyon walls, flat tops.
    Sqrt,
    /// h = k·G/|∇G|(camera ground point): the potential, normalised once.
    Potential,
}

#[derive(Clone, Copy, Debug)]
pub struct HeightCfg {
    pub func: HeightFn,
    pub k: f64,
    pub sigma: f64,
    /// ln|∇G| at the camera ground point (only for Potential).
    pub log_grad_g0: f64,
}

pub struct Tile {
    pub h: Vec<f32>,   // height, world units
    pub de: Vec<f32>,  // DE / S
    pub mu: Vec<f32>,  // smooth iteration count, < 0 interior
}

type Key = (i32, i64, i64);

pub struct Terrain {
    pub c0: (f64, f64),
    pub scale: f64,
    pub cfg: HeightCfg,
    pub max_iter: u32,
    shards: Vec<Mutex<HashMap<Key, Arc<OnceLock<Arc<Tile>>>>>>,
    pub tiles: AtomicU64,
    pub evals: AtomicU64,
    pub iters: AtomicU64,
    pub tile_ns: AtomicU64,
    pub unresolved: AtomicU64,
}

impl Terrain {
    pub fn new(c0: (f64, f64), scale: f64, mut cfg: HeightCfg, max_iter: u32) -> Self {
        if cfg.func == HeightFn::Potential {
            // |∇G| = 2G/DE; take it at the nearest exterior point to c0
            let s = mandel::eval(c0.0, c0.1, max_iter);
            let s = if s.interior { mandel::eval(c0.0 + scale, c0.1, max_iter) } else { s };
            cfg.log_grad_g0 = s.log_g + std::f64::consts::LN_2 - s.de.max(1e-300).ln();
        }
        Terrain {
            c0,
            scale,
            cfg,
            max_iter,
            shards: (0..64).map(|_| Mutex::new(HashMap::new())).collect(),
            tiles: AtomicU64::new(0),
            evals: AtomicU64::new(0),
            iters: AtomicU64::new(0),
            tile_ns: AtomicU64::new(0),
            unresolved: AtomicU64::new(0),
        }
    }

    /// Height (world units) for a sample.
    pub fn height(&self, s: &mandel::Sample) -> f64 {
        if s.interior {
            return 0.0;
        }
        let de = s.de / self.scale;
        let HeightCfg { func, k, sigma, log_grad_g0 } = self.cfg;
        match func {
            HeightFn::Linear => k * de,
            HeightFn::Log => sigma * (k * de / sigma).ln_1p(),
            HeightFn::Sqrt => k * (sigma * de).sqrt(),
            HeightFn::Potential => k * (s.log_g - log_grad_g0).exp() / self.scale,
        }
    }

    fn compute(&self, (l, tx, ty): Key) -> Tile {
        let t0 = Instant::now();
        let sp = (l as f64).exp2();
        let mut tile = Tile { h: vec![0.0; TS * TS], de: vec![0.0; TS * TS], mu: vec![0.0; TS * TS] };
        let mut pts = Vec::with_capacity(TS * TS);
        for j in 0..TS {
            let y = (ty * TILE + j as i64) as f64 * sp;
            for i in 0..TS {
                let x = (tx * TILE + i as i64) as f64 * sp;
                pts.push((self.c0.0 + x * self.scale, self.c0.1 + y * self.scale));
            }
        }
        let mut out = vec![mandel::Sample::default(); TS * TS];
        mandel::eval_many(&pts, &mut out, self.max_iter);
        let mut it = 0u64;
        let mut unresolved = 0u64;
        for (idx, s) in out.iter().enumerate() {
            it += s.iters as u64;
            unresolved += (s.mu == -2.0) as u64;
            tile.h[idx] = self.height(s) as f32;
            tile.de[idx] = (s.de / self.scale) as f32;
            tile.mu[idx] = s.mu as f32;
        }
        self.unresolved.fetch_add(unresolved, Ordering::Relaxed);
        self.tiles.fetch_add(1, Ordering::Relaxed);
        self.evals.fetch_add((TS * TS) as u64, Ordering::Relaxed);
        self.iters.fetch_add(it, Ordering::Relaxed);
        self.tile_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        tile
    }

    fn tile(&self, key: Key) -> Arc<Tile> {
        let h = (key.0 as u64).wrapping_mul(0x9E37_79B9) ^ (key.1 as u64).wrapping_mul(0x85EB_CA6B) ^ (key.2 as u64).wrapping_mul(0xC2B2_AE35);
        let cell = {
            let mut m = self.shards[(h >> 7) as usize & 63].lock().unwrap();
            m.entry(key).or_insert_with(|| Arc::new(OnceLock::new())).clone()
        };
        cell.get_or_init(|| Arc::new(self.compute(key))).clone()
    }
}

/// Values interpolated at a point.
#[derive(Clone, Copy, Debug, Default)]
pub struct Probe {
    pub h: f64,
    pub de: f64,
    pub mu: f64,
    /// Fraction of interior (lake) samples in the bilinear footprint.
    pub lake: f64,
}

/// Per-thread view with a small memo of tile handles (no locking on hits).
pub struct Local<'a> {
    pub t: &'a Terrain,
    map: HashMap<Key, Arc<Tile>>,
}

impl<'a> Local<'a> {
    pub fn new(t: &'a Terrain) -> Self {
        Local { t, map: HashMap::with_capacity(512) }
    }

    #[inline]
    fn get(&mut self, key: Key) -> &Tile {
        if !self.map.contains_key(&key) {
            if self.map.len() > 4096 {
                self.map.clear();
            }
            let tile = self.t.tile(key);
            self.map.insert(key, tile);
        }
        &self.map[&key]
    }

    #[inline]
    fn cell(&mut self, x: f64, y: f64, l: i32) -> (&Tile, usize, f64, f64) {
        let inv = (-l as f64).exp2();
        let gx = x * inv;
        let gy = y * inv;
        let fx = gx.floor();
        let fy = gy.floor();
        let ix = fx as i64;
        let iy = fy as i64;
        let key = (l, ix.div_euclid(TILE), iy.div_euclid(TILE));
        let idx = iy.rem_euclid(TILE) as usize * TS + ix.rem_euclid(TILE) as usize;
        (self.get(key), idx, gx - fx, gy - fy)
    }

    #[inline]
    fn h_level(&mut self, x: f64, y: f64, l: i32) -> f64 {
        let (t, i, u, v) = self.cell(x, y, l);
        let a = t.h[i] as f64;
        let b = t.h[i + 1] as f64;
        let c = t.h[i + TS] as f64;
        let d = t.h[i + TS + 1] as f64;
        (a + (b - a) * u) * (1.0 - v) + (c + (d - c) * u) * v
    }

    /// Height at (x, y) for a footprint of 2^lvl world units, blended between levels.
    #[inline]
    pub fn height(&mut self, x: f64, y: f64, lvl: f64) -> f64 {
        let lvl = lvl.clamp(LMIN as f64, LMAX as f64 - 1.0);
        let l0 = lvl.floor();
        let f = lvl - l0;
        let l0 = l0 as i32;
        let a = self.h_level(x, y, l0);
        if f < 1e-3 {
            return a;
        }
        let b = self.h_level(x, y, l0 + 1);
        a + (b - a) * f
    }

    fn probe_level(&mut self, x: f64, y: f64, l: i32) -> Probe {
        let (t, i, u, v) = self.cell(x, y, l);
        let w = [(1.0 - u) * (1.0 - v), u * (1.0 - v), (1.0 - u) * v, u * v];
        let ids = [i, i + 1, i + TS, i + TS + 1];
        let mut p = Probe::default();
        let mut wmu = 0.0;
        for k in 0..4 {
            p.h += w[k] * t.h[ids[k]] as f64;
            p.de += w[k] * t.de[ids[k]] as f64;
            let m = t.mu[ids[k]] as f64;
            if m < 0.0 {
                p.lake += w[k];
            } else {
                p.mu += w[k] * m;
                wmu += w[k];
            }
        }
        p.mu = if wmu > 0.0 { p.mu / wmu } else { -1.0 };
        p
    }

    pub fn probe(&mut self, x: f64, y: f64, lvl: f64) -> Probe {
        let lvl = lvl.clamp(LMIN as f64, LMAX as f64 - 1.0);
        let l0 = lvl.floor();
        let f = lvl - l0;
        let a = self.probe_level(x, y, l0 as i32);
        let b = self.probe_level(x, y, l0 as i32 + 1);
        let mix = |p: f64, q: f64| p + (q - p) * f;
        Probe {
            h: mix(a.h, b.h),
            de: mix(a.de, b.de),
            mu: if a.mu < 0.0 { b.mu } else if b.mu < 0.0 { a.mu } else { mix(a.mu, b.mu) },
            lake: mix(a.lake, b.lake),
        }
    }
}
