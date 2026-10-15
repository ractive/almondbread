//! A sample cache in fractal coordinates (R4): computed point samples keep their exact
//! position (re-expressed in each frame's pixels through the pan/zoom map) and raw data
//! (n, smooth offset, DE, normal angle), are re-shaded every frame (DE in the current frame's
//! pixels) and reconstructed with a filter; only pixels whose cell lacks samples get new ones.
//! Variants: `Fill(m)` (new sample where a pixel has fewer than m), `Budget` (spend a given
//! iteration budget on the pixels with the fewest samples), `Nearest(eps)` (XaoS-like: show
//! the sample nearest the pixel centre, compute a new centre sample when none lies within eps).

use rayon::prelude::*;

use crate::field::{Field, Sample};
use crate::shade::{Rgb, Shader, UNRESOLVED};
use crate::transform::Affine;

#[derive(Clone, Copy, Debug)]
pub enum Policy {
    Fill(u32),
    Budget,
    Nearest(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Recon {
    Gauss,
    /// Gaussian splat of the cached samples over this cache's own reprojected output
    /// (Catmull-Rom, weight `hist_weight` x zoom-in decay): the history only fills where
    /// samples are sparse.
    Hybrid,
    Box,
    Nearest,
}

pub struct Cache {
    pub name: String,
    pub w: usize,
    pub h: usize,
    pub kmax: usize,
    pub policy: Policy,
    pub recon: Recon,
    pub samples: Vec<Sample>,
    start: Vec<u32>,
    perm: Vec<usize>,
    pub sigma: f32,
    /// Iteration budget per frame as a multiple of FL's (Budget policy).
    pub budget_mult: f64,
    pub hist_weight: f32,
    hist: Vec<[f32; 4]>,
    weights: Vec<f32>,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Step {
    pub new_samples: u64,
    pub cost: u64,
    pub cached: u64,
    pub empty_before: u64,
}

fn hash(a: u64, b: u64) -> u64 {
    let mut z = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_add(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Cache {
    pub fn new(name: &str, w: usize, h: usize, kmax: usize, policy: Policy, recon: Recon) -> Self {
        Self {
            name: name.into(),
            w,
            h,
            kmax,
            policy,
            recon,
            samples: Vec::new(),
            start: vec![0; w * h + 1],
            perm: Vec::new(),
            budget_mult: 1.0,
            hist_weight: 0.3,
            hist: Vec::new(),
            weights: Vec::new(),
            sigma: 0.5,
        }
    }

    /// Groups the samples by cell (stable counting sort: inside a cell older samples come
    /// first, since new ones are appended), keeps the newest kmax per cell, rebuilds the index.
    fn bucket(&mut self) {
        let (w, n) = (self.w, self.w * self.h);
        let cells: Vec<u32> = self
            .samples
            .par_iter()
            .map(|s| ((s.y as usize) * w + s.x as usize) as u32)
            .collect();
        let mut count = vec![0u32; n];
        for &c in &cells {
            count[c as usize] += 1;
        }
        let kmax = self.kmax as u32;
        let mut skip: Vec<u32> = count.iter().map(|&c| c.saturating_sub(kmax)).collect();
        let mut start = vec![0u32; n + 1];
        for c in 0..n {
            start[c + 1] = start[c] + count[c].min(kmax);
        }
        let mut fill = start.clone();
        let mut kept: Vec<Sample> = Vec::with_capacity(start[n] as usize);
        // SAFETY-free fill: every slot below start[n] is written exactly once below.
        kept.resize(start[n] as usize, Sample::default());
        for (s, &c) in self.samples.iter().zip(&cells) {
            let c = c as usize;
            if skip[c] > 0 {
                skip[c] -= 1;
                continue;
            }
            kept[fill[c] as usize] = *s;
            fill[c] += 1;
        }
        self.samples = kept;
        self.start = start;
    }

    pub fn reset(&mut self, field: &Field) {
        self.hist.clear();
        self.samples = field.samples.clone();
        self.bucket();
    }

    fn in_cell(&self, c: usize) -> &[Sample] {
        &self.samples[self.start[c] as usize..self.start[c + 1] as usize]
    }

    /// One frame: move the cached samples by `tf`, add new ones from `cand` (this frame's
    /// candidate field at full resolution, one sample per pixel), reconstruct.
    pub fn frame(
        &mut self,
        k: u32,
        tf: &Affine,
        max_iter: usize,
        cand: &Field,
        budget: u64,
        shader: &Shader,
    ) -> (Vec<Rgb>, Step) {
        let (w, h) = (self.w, self.h);
        assert_eq!(cand.w, w);
        let (wf, hf) = (w as f64, h as f64);
        let a = tf.a as f32;
        self.samples.par_iter_mut().for_each(|s| {
            let p = tf.apply([s.x, s.y]);
            s.x = p[0];
            s.y = p[1];
            s.de *= a;
        });
        let lim = max_iter as f64;
        self.samples.retain(|s| {
            s.x >= 0.0 && s.y >= 0.0 && s.x < wf && s.y < hf && !(s.class == UNRESOLVED && lim > 1.25 * s.limit as f64)
        });
        self.bucket();
        let counts: Vec<u32> = (0..w * h).map(|c| self.start[c + 1] - self.start[c]).collect();
        let mut step = Step {
            empty_before: counts.iter().filter(|&&c| c == 0).count() as u64,
            ..Step::default()
        };
        let budget = (budget as f64 * self.budget_mult) as u64;
        let chosen: Vec<usize> = match self.policy {
            Policy::Fill(m) => (0..w * h).filter(|&p| counts[p] < m).collect(),
            Policy::Budget => {
                // Fewest samples first; inside a class in a pseudo-random order that changes
                // every frame (a fixed permutation, rotated).
                let n = w * h;
                if self.perm.len() != n {
                    let mut v: Vec<(u64, usize)> = (0..n).map(|p| (hash(p as u64, 77), p)).collect();
                    v.sort_unstable();
                    self.perm = v.into_iter().map(|x| x.1).collect();
                }
                let rot = (hash(k as u64, 5) % n as u64) as usize;
                let mut spent = 0u64;
                let mut out = Vec::new();
                'classes: for class in 0..self.kmax as u32 {
                    for idx in 0..n {
                        let p = self.perm[(idx + rot) % n];
                        if counts[p] != class {
                            continue;
                        }
                        let cost = cand.samples[p].cost as u64;
                        if spent + cost > budget {
                            break 'classes;
                        }
                        spent += cost;
                        out.push(p);
                    }
                }
                out
            }
            Policy::Nearest(eps) => (0..w * h)
                .into_par_iter()
                .filter(|&p| {
                    let (i, j) = ((p % w) as f64 + 0.5, (p / w) as f64 + 0.5);
                    !self
                        .in_cell(p)
                        .iter()
                        .any(|s| (s.x - i).abs() <= eps && (s.y - j).abs() <= eps)
                })
                .collect(),
        };
        for &p in &chosen {
            let mut s = cand.samples[p];
            s.birth = k;
            step.cost += s.cost as u64;
            self.samples.push(s);
        }
        step.new_samples = chosen.len() as u64;
        if !chosen.is_empty() {
            self.bucket();
        }
        step.cached = self.samples.len() as u64;
        let mut img = self.reconstruct(shader);
        if self.recon == Recon::Hybrid {
            img = self.hybrid(&img, tf);
        }
        (img, step)
    }

    /// Hybrid: blend the sample reconstruction with the reprojected previous output where the
    /// sample weight is low. `img` was made with weights in `self.weights`.
    fn hybrid(&mut self, img: &[Rgb], tf: &Affine) -> Vec<Rgb> {
        let (w, h) = (self.w, self.h);
        let inv = tf.inverse();
        let decay = (inv.a as f32).min(1.0).powi(2);
        let hw = self.hist_weight * decay;
        let hist = &self.hist;
        let weights = &self.weights;
        let out: Vec<[f32; 4]> = (0..w * h)
            .into_par_iter()
            .map(|p| {
                let (i, j) = (p % w, p / w);
                let c = img[p];
                let ws = weights[p];
                if hist.is_empty() {
                    return [c[0], c[1], c[2], 1.0];
                }
                let q = inv.apply([i as f64 + 0.5, j as f64 + 0.5]);
                if q[0] < 0.0 || q[1] < 0.0 || q[0] > w as f64 || q[1] > h as f64 {
                    return [c[0], c[1], c[2], 1.0];
                }
                let hv = crate::fl::catmull_rom(hist, w, h, [q[0] as f32, q[1] as f32]);
                let mut o = [0.0; 4];
                for k in 0..3 {
                    o[k] = (c[k] * ws + hv[k].max(0.0) * hw) / (ws + hw);
                }
                o[3] = 1.0;
                o
            })
            .collect();
        self.hist = out;
        self.hist.iter().map(|v| [v[0], v[1], v[2]]).collect()
    }

    pub fn reconstruct(&mut self, shader: &Shader) -> Vec<Rgb> {
        let (img, weights) = self.reconstruct_weights(shader);
        self.weights = weights;
        img
    }

    fn reconstruct_weights(&self, shader: &Shader) -> (Vec<Rgb>, Vec<f32>) {
        let (w, h) = (self.w, self.h);
        let cols: Vec<Rgb> = self
            .samples
            .par_iter()
            .map(|s| shader.shade(s.class, s.n, s.s, s.de, s.ang))
            .collect();
        let inv2s2 = 1.0 / (2.0 * self.sigma * self.sigma);
        (0..w * h)
            .into_par_iter()
            .map(|p| {
                let (i, j) = (p % w, p / w);
                let (pcx, pcy) = (i as f64 + 0.5, j as f64 + 0.5);
                let r0 = self.start[p] as usize;
                let r1 = self.start[p + 1] as usize;
                match self.recon {
                    Recon::Box if r1 > r0 => {
                        let mut c = [0.0; 3];
                        for q in r0..r1 {
                            for k in 0..3 {
                                c[k] += cols[q][k];
                            }
                        }
                        let n = (r1 - r0) as f32;
                        return ([c[0] / n, c[1] / n, c[2] / n], n);
                    }
                    Recon::Nearest if r1 > r0 => {
                        let mut best = (f64::MAX, r0);
                        for q in r0..r1 {
                            let s = &self.samples[q];
                            let d = (s.x - pcx).powi(2) + (s.y - pcy).powi(2);
                            if d < best.0 {
                                best = (d, q);
                            }
                        }
                        return (cols[best.1], 1.0);
                    }
                    _ => {}
                }
                // Gaussian over the 3 x 3 cells; fallback: nearest sample within 5 x 5.
                let mut sum = [0.0f32; 3];
                let mut ws = 0.0f32;
                let mut nearest = (f64::MAX, usize::MAX);
                for rad in [1i64, 2] {
                    for dj in -rad..=rad {
                        for di in -rad..=rad {
                            if rad == 2 && di.abs() < 2 && dj.abs() < 2 {
                                continue;
                            }
                            let (x, y) = (i as i64 + di, j as i64 + dj);
                            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                                continue;
                            }
                            let c = y as usize * w + x as usize;
                            for q in self.start[c] as usize..self.start[c + 1] as usize {
                                let s = &self.samples[q];
                                let d2 = (s.x - pcx).powi(2) + (s.y - pcy).powi(2);
                                if d2 < nearest.0 {
                                    nearest = (d2, q);
                                }
                                if rad == 1 {
                                    let wt = (-(d2 as f32) * inv2s2).exp();
                                    for k in 0..3 {
                                        sum[k] += cols[q][k] * wt;
                                    }
                                    ws += wt;
                                }
                            }
                        }
                    }
                    if ws > 1e-3 {
                        return ([sum[0] / ws, sum[1] / ws, sum[2] / ws], ws);
                    }
                }
                if nearest.1 != usize::MAX {
                    (cols[nearest.1], 1e-3)
                } else {
                    ([0.0; 3], 0.0)
                }
            })
            .unzip()
    }
}
