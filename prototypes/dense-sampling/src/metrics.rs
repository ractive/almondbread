//! Image error metrics (OKLab ΔE x 100), flicker under the scripted pan, image output.

use std::path::Path;

use rayon::prelude::*;

use crate::lattice::{PAN, SUB};
use crate::shade::{Rgb, delta_e, srgb8};

#[derive(Clone, Copy, Debug, Default)]
pub struct Err {
    pub mean: f32,
    pub p95: f32,
    pub p99: f32,
    pub max: f32,
    /// Share of pixels with ΔE > 2 and > 5.
    pub over2: f32,
    pub over5: f32,
}

impl std::fmt::Display for Err {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "dE mean {:5.2} p95 {:5.2} p99 {:5.2} max {:5.1} >2 {:5.1}% >5 {:5.1}%",
            self.mean,
            self.p95,
            self.p99,
            self.max,
            100.0 * self.over2,
            100.0 * self.over5
        )
    }
}

pub fn errors(d: &[f32]) -> Err {
    if d.is_empty() {
        return Err::default();
    }
    let mut v = d.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| v[((v.len() - 1) as f64 * p) as usize];
    Err {
        mean: v.iter().sum::<f32>() / v.len() as f32,
        p95: q(0.95),
        p99: q(0.99),
        max: *v.last().unwrap(),
        over2: v.iter().filter(|&&x| x > 2.0).count() as f32 / v.len() as f32,
        over5: v.iter().filter(|&&x| x > 5.0).count() as f32 / v.len() as f32,
    }
}

pub fn diff(a: &[Rgb], b: &[Rgb]) -> Vec<f32> {
    a.par_iter().zip(b.par_iter()).map(|(x, y)| delta_e(*x, *y)).collect()
}

pub fn masked(d: &[f32], mask: &[bool]) -> Vec<f32> {
    d.iter().zip(mask).filter(|(_, m)| **m).map(|(x, _)| *x).collect()
}

/// Bilinear sample of a w x h image at pixel coordinates (centres at i + 0.5); None outside.
pub fn bilerp(img: &[Rgb], w: usize, h: usize, x: f32, y: f32) -> Option<Rgb> {
    let fx = x - 0.5;
    let fy = y - 0.5;
    if fx < 0.0 || fy < 0.0 || fx > (w - 1) as f32 || fy > (h - 1) as f32 {
        return None;
    }
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let p = |x: usize, y: usize| img[y * w + x];
    let mut out = [0.0; 3];
    for c in 0..3 {
        let a = p(x0, y0)[c] * (1.0 - tx) + p(x1, y0)[c] * tx;
        let b = p(x0, y1)[c] * (1.0 - tx) + p(x1, y1)[c] * tx;
        out[c] = a * (1.0 - ty) + b * ty;
    }
    Some(out)
}

/// The pan's content shift between consecutive frames in output pixels.
pub fn pan_px() -> (f32, f32) {
    (PAN.0 as f32 / SUB as f32, PAN.1 as f32 / SUB as f32)
}

/// Flicker: mean ΔE between each frame and the previous frame reprojected onto it (bilinear),
/// averaged over the frame pairs. The converged reference has a small floor from the
/// bilinear reprojection itself.
pub fn flicker(frames: &[Vec<Rgb>], w: usize, h: usize) -> f32 {
    let (sx, sy) = pan_px();
    let mut total = 0.0f64;
    let mut count = 0usize;
    for t in 1..frames.len() {
        let (prev, cur) = (&frames[t - 1], &frames[t]);
        let d: Vec<f32> = (0..w * h)
            .into_par_iter()
            .filter_map(|p| {
                let (i, j) = (p % w, p / w);
                bilerp(prev, w, h, i as f32 + 0.5 + sx, j as f32 + 0.5 + sy).map(|q| delta_e(q, cur[p]))
            })
            .collect();
        total += d.iter().map(|&x| x as f64).sum::<f64>();
        count += d.len();
    }
    (total / count.max(1) as f64) as f32
}

/// Temporal accumulation as in the FL gesture mode, simplified: the previous history is
/// reprojected (bilinear) and blended with the new frame with confidence capped at `cap`
/// (the app caps at 6), so the new frame weighs 1 / (min(conf, cap) + 1).
pub fn accumulate(frames: &[Vec<Rgb>], w: usize, h: usize, cap: f32) -> Vec<Vec<Rgb>> {
    let (sx, sy) = pan_px();
    let mut out: Vec<Vec<Rgb>> = Vec::new();
    let mut conf = vec![1.0f32; w * h];
    out.push(frames[0].clone());
    for t in 1..frames.len() {
        let prev = &out[t - 1];
        let cur = &frames[t];
        let mut next = vec![[0.0; 3]; w * h];
        let mut nconf = vec![0.0f32; w * h];
        for p in 0..w * h {
            let (i, j) = (p % w, p / w);
            match bilerp(prev, w, h, i as f32 + 0.5 + sx, j as f32 + 0.5 + sy) {
                Some(hc) => {
                    // Confidence of the reprojected history: nearest previous pixel's.
                    let pi = ((i as f32 + sx).round() as usize).min(w - 1);
                    let pj = ((j as f32 + sy).round() as usize).min(h - 1);
                    let c = conf[pj * w + pi].min(cap);
                    for k in 0..3 {
                        next[p][k] = (hc[k] * c + cur[p][k]) / (c + 1.0);
                    }
                    nconf[p] = c + 1.0;
                }
                None => {
                    next[p] = cur[p];
                    nconf[p] = 1.0;
                }
            }
        }
        conf = nconf;
        out.push(next);
    }
    out
}

pub fn write_png(path: &Path, img: &[Rgb], w: usize, h: usize) {
    let mut bgra = Vec::with_capacity(w * h * 4);
    for c in img {
        bgra.extend_from_slice(&[srgb8(c[2]), srgb8(c[1]), srgb8(c[0]), 255]);
    }
    almondbread_core::image::write_png_bgra(path, &bgra, w, h).expect("png");
}

/// ΔE heat map: black 0, through red (5) to yellow/white (>= 15).
pub fn write_heat(path: &Path, d: &[f32], w: usize, h: usize) {
    let img: Vec<Rgb> = d
        .iter()
        .map(|&x| {
            let t = (x / 15.0).clamp(0.0, 1.0);
            [
                (3.0 * t).min(1.0),
                (3.0 * t - 1.0).clamp(0.0, 1.0),
                (3.0 * t - 2.0).clamp(0.0, 1.0),
            ]
        })
        .collect();
    write_png(path, &img, w, h);
}

pub fn write_mask(path: &Path, m: &[u8], w: usize, h: usize) {
    // 0 black, 1 grey, 2 white, 3 red, 4 blue
    let img: Vec<Rgb> = m
        .iter()
        .map(|&v| match v {
            0 => [0.0, 0.0, 0.0],
            1 => [0.2, 0.2, 0.2],
            2 => [1.0, 1.0, 1.0],
            3 => [1.0, 0.05, 0.05],
            _ => [0.1, 0.3, 1.0],
        })
        .collect();
    write_png(path, &img, w, h);
}
