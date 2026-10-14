//! The camera's ground: DE sampled on the CPU (f64) at camera-independent grid points of the
//! absolute levels near the camera scale, smoothed with a cubic B-spline and blended between two
//! levels by the continuous log2 of the scale. The result is C² in position and continuous in S,
//! so terrain following never twitches over single samples (kb: decisions/landscape-mode, L1).

use std::collections::HashMap;

use crate::mandel;

pub struct Ground {
    pub c0: (f64, f64),
    cache: HashMap<(i32, i64, i64), f32>,
    pub max_iter: u32,
    pub evals: u64,
    /// height curve h = σ·ln(1 + k·DE/σ), absolute (complex units)
    sigma: f64,
    k: f64,
}

fn bspline(t: f64) -> [f64; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        (1.0 - 3.0 * t + 3.0 * t2 - t3) / 6.0,
        (4.0 - 6.0 * t2 + 3.0 * t3) / 6.0,
        (1.0 + 3.0 * t + 3.0 * t2 - 3.0 * t3) / 6.0,
        t3 / 6.0,
    ]
}

impl Ground {
    pub fn new(c0: (f64, f64), sigma: f64, k: f64) -> Self {
        Ground { c0, cache: HashMap::new(), max_iter: 6000, evals: 0, sigma, k }
    }

    pub fn set_sigma(&mut self, sigma: f64) {
        self.sigma = sigma;
    }

    /// DE (complex units, 0 = interior/unresolved) at sample (i, j) of level l.
    fn de(&mut self, l: i32, i: i64, j: i64) -> f64 {
        if self.cache.len() > 200_000 {
            self.cache.clear();
        }
        let c0 = self.c0;
        let max_iter = self.max_iter;
        let mut miss = false;
        let v = *self.cache.entry((l, i, j)).or_insert_with(|| {
            miss = true;
            let sp = (l as f64).exp2();
            let s = mandel::eval(c0.0 + i as f64 * sp, c0.1 + j as f64 * sp, max_iter);
            if s.interior { 0.0 } else { s.de as f32 }
        });
        if miss {
            self.evals += 1;
        }
        v as f64
    }

    fn h(&self, de: f64) -> f64 {
        self.sigma * (1.0 + self.k * de / self.sigma).ln()
    }

    /// B-spline smoothed height (complex units) at offset p on level l.
    fn smooth_level(&mut self, p: [f64; 2], l: i32) -> f64 {
        let sp = (l as f64).exp2();
        let gx = p[0] / sp;
        let gy = p[1] / sp;
        let fx = gx.floor();
        let fy = gy.floor();
        let wx = bspline(gx - fx);
        let wy = bspline(gy - fy);
        let (ix, iy) = (fx as i64, fy as i64);
        let mut acc = 0.0;
        for b in 0..4 {
            for a in 0..4 {
                let de = self.de(l, ix + a as i64 - 1, iy + b as i64 - 1);
                acc += wx[a] * wy[b] * self.h(de);
            }
        }
        acc
    }

    /// Ground height (complex units) at offset p, smoothed to `scale` (complex units), blended
    /// continuously between the two neighbouring power-of-two grids.
    pub fn smooth(&mut self, p: [f64; 2], scale: f64) -> f64 {
        let lam = scale.log2();
        let l0 = lam.floor();
        let f = lam - l0;
        let a = self.smooth_level(p, l0 as i32);
        if f < 1e-6 {
            return a;
        }
        let b = self.smooth_level(p, l0 as i32 + 1);
        a + (b - a) * f
    }
}
