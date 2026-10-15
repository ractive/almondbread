//! CPU port of the FL gesture compose (`compose` mode 0 in
//! crates/almondbread-render/src/shaders/mandelbrot.metal, `encode_compose` in scheduler.rs):
//! a jittered preview level shaded at its own resolution, splatted as point samples
//! (Gaussian, sigma 0.5 output px) over the history reprojected with 5-tap Catmull-Rom,
//! whose confidence decays by step^2 on zoom-in and is capped at 6; the bilinear preview
//! fills in where neither exists.

use rayon::prelude::*;

use crate::field::Field;
use crate::shade::{Rgb, Shader};
use crate::transform::Affine;

pub const CONF_CAP: f32 = 6.0;
pub const BASE_WEIGHT: f32 = 0.02;
pub const SIGMA: f32 = 0.5;

/// RGB + confidence image.
pub struct Fl {
    pub w: usize,
    pub h: usize,
    pub hist: Vec<[f32; 4]>,
    pub valid: bool,
}

fn bilinear4(img: &[[f32; 4]], w: usize, h: usize, x: f32, y: f32) -> [f32; 4] {
    // Texel coordinates with centres at i + 0.5, clamp to edge.
    let fx = (x - 0.5).clamp(0.0, (w - 1) as f32);
    let fy = (y - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let mut o = [0.0; 4];
    for c in 0..4 {
        let a = img[y0 * w + x0][c] * (1.0 - tx) + img[y0 * w + x1][c] * tx;
        let b = img[y1 * w + x0][c] * (1.0 - tx) + img[y1 * w + x1][c] * tx;
        o[c] = a * (1.0 - ty) + b * ty;
    }
    o
}

pub fn bilinear3(img: &[Rgb], w: usize, h: usize, x: f32, y: f32) -> Rgb {
    let fx = (x - 0.5).clamp(0.0, (w - 1) as f32);
    let fy = (y - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let mut o = [0.0; 3];
    for c in 0..3 {
        let a = img[y0 * w + x0][c] * (1.0 - tx) + img[y0 * w + x1][c] * tx;
        let b = img[y1 * w + x0][c] * (1.0 - tx) + img[y1 * w + x1][c] * tx;
        o[c] = a * (1.0 - ty) + b * ty;
    }
    o
}

/// The kernel's `catmullRom`: 5 bilinear taps.
pub fn catmull_rom(img: &[[f32; 4]], w: usize, h: usize, p: [f32; 2]) -> [f32; 4] {
    let mut pos1 = [0.0f32; 2];
    let mut wt = [[0.0f32; 4]; 2];
    for a in 0..2 {
        pos1[a] = (p[a] - 0.5).floor() + 0.5;
        let f = p[a] - pos1[a];
        wt[a][0] = f * (-0.5 + f * (1.0 - 0.5 * f));
        wt[a][1] = 1.0 + f * f * (-2.5 + 1.5 * f);
        wt[a][2] = f * (0.5 + f * (2.0 - 1.5 * f));
        wt[a][3] = f * f * (-0.5 + 0.5 * f);
    }
    let w12 = [wt[0][1] + wt[0][2], wt[1][1] + wt[1][2]];
    let p12 = [pos1[0] + wt[0][2] / w12[0], pos1[1] + wt[1][2] / w12[1]];
    let p0 = [pos1[0] - 1.0, pos1[1] - 1.0];
    let p3 = [pos1[0] + 2.0, pos1[1] + 2.0];
    let s = |x: f32, y: f32| bilinear4(img, w, h, x, y);
    let taps = [
        (s(p12[0], p0[1]), w12[0] * wt[1][0]),
        (s(p0[0], p12[1]), wt[0][0] * w12[1]),
        (s(p12[0], p12[1]), w12[0] * w12[1]),
        (s(p3[0], p12[1]), wt[0][3] * w12[1]),
        (s(p12[0], p3[1]), w12[0] * wt[1][3]),
    ];
    let mut r = [0.0; 4];
    let mut norm = 0.0;
    for (v, wgt) in taps {
        for c in 0..4 {
            r[c] += v[c] * wgt;
        }
        norm += wgt;
    }
    for c in r.iter_mut() {
        *c /= norm;
    }
    r
}

impl Fl {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            hist: vec![[0.0; 4]; w * h],
            valid: false,
        }
    }

    /// Starts from an exact image (the refined/settled frame before a gesture).
    pub fn reset(&mut self, img: &[Rgb], conf: f32) {
        for (h, c) in self.hist.iter_mut().zip(img) {
            *h = [c[0], c[1], c[2], conf];
        }
        self.valid = true;
    }

    /// One gesture frame: `prev_to_cur` maps the history's pixels to this frame's; `preview`
    /// is this frame's jittered preview level (None: history only). Returns the shown image.
    pub fn frame(&mut self, shader: &Shader, prev_to_cur: &Affine, preview: Option<&Field>) -> Vec<Rgb> {
        let (w, h) = (self.w, self.h);
        let colors: Option<Vec<Rgb>> = preview.map(|p| {
            p.samples
                .par_iter()
                .map(|s| shader.shade(s.class, s.n, s.s, s.de, s.ang))
                .collect()
        });
        // Inverse map: current pixel -> history pixel: x_h = (x_c - C) / a + C + ...
        let inv = prev_to_cur.inverse();
        let step = inv.a as f32; // history pixels per output pixel
        let decay = step.min(1.0).powi(2);
        let inv2s2 = 1.0 / (2.0 * SIGMA * SIGMA);
        let hist = &self.hist;
        let valid = self.valid;
        let out: Vec<[f32; 4]> = (0..w * h)
            .into_par_iter()
            .map(|p| {
                let (i, j) = (p % w, p / w);
                let pc = [i as f32 + 0.5, j as f32 + 0.5];
                let mut base = [0.0f32; 3];
                let mut has_base = false;
                let mut sum = [0.0f32; 3];
                let mut wn = 0.0f32;
                if let (Some(pv), Some(cols)) = (preview, &colors) {
                    let r = pv.ratio as f32;
                    let (uw, uh) = (pv.w as f32, pv.h as f32);
                    let v = [
                        (pc[0] - w as f32 / 2.0) * r + uw / 2.0,
                        (pc[1] - h as f32 / 2.0) * r + uh / 2.0,
                    ];
                    let jx = pv.jitter;
                    let bp = [
                        (v[0] - jx[0] + 0.5).clamp(0.5, uw - 0.5),
                        (v[1] - jx[1] + 0.5).clamp(0.5, uh - 0.5),
                    ];
                    base = bilinear3(cols, pv.w, pv.h, bp[0], bp[1]);
                    has_base = true;
                    let t0 = [(v[0] - jx[0]).floor() as i64, (v[1] - jx[1]).floor() as i64];
                    for dj in 0..2 {
                        for di in 0..2 {
                            let t = [t0[0] + di, t0[1] + dj];
                            if t[0] < 0 || t[1] < 0 || t[0] >= pv.w as i64 || t[1] >= pv.h as i64 {
                                continue;
                            }
                            let d = [(t[0] as f32 + jx[0] - v[0]) / r, (t[1] as f32 + jx[1] - v[1]) / r];
                            let wgt = (-(d[0] * d[0] + d[1] * d[1]) * inv2s2).exp();
                            let c = cols[t[1] as usize * pv.w + t[0] as usize];
                            for k in 0..3 {
                                sum[k] += c[k] * wgt;
                            }
                            wn += wgt;
                        }
                    }
                }
                let mut hc = base;
                let mut wh = 0.0f32;
                if valid {
                    let hp = inv.apply([i as f64 + 0.5, j as f64 + 0.5]);
                    let hp = [hp[0] as f32, hp[1] as f32];
                    if hp[0] >= 0.0 && hp[1] >= 0.0 && hp[0] <= w as f32 && hp[1] <= h as f32 {
                        let identity = (step - 1.0).abs() < 1e-7
                            && ((hp[0] - 0.5).fract().abs() < 1e-5)
                            && ((hp[1] - 0.5).fract().abs() < 1e-5);
                        let v = if identity {
                            hist[(hp[1] as usize).min(h - 1) * w + (hp[0] as usize).min(w - 1)]
                        } else {
                            catmull_rom(hist, w, h, hp)
                        };
                        hc = [v[0].max(0.0), v[1].max(0.0), v[2].max(0.0)];
                        wh = v[3].clamp(0.0, CONF_CAP) * decay;
                    }
                }
                if !has_base && wh <= 0.0 {
                    return [0.0, 0.0, 0.0, 0.0];
                }
                let wb = if wh + wn < 1e-3 { 1.0 } else { BASE_WEIGHT };
                let mut col = [0.0; 3];
                for k in 0..3 {
                    col[k] = (hc[k] * wh + sum[k] + base[k] * wb) / (wh + wn + wb);
                }
                [col[0], col[1], col[2], CONF_CAP.min(wh + wn)]
            })
            .collect();
        self.hist = out;
        self.valid = true;
        self.hist.iter().map(|v| [v[0], v[1], v[2]]).collect()
    }
}
