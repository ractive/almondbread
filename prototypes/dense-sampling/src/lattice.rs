//! A dense sample lattice over a view and a short pan: `SUB x SUB` samples per output pixel,
//! wide enough for every frame of the scripted motion. Every strategy picks its samples from
//! it, and the converged reference is the box mean of all `SUB²` samples per pixel, so all
//! numbers are deterministic and comparable.

use rayon::prelude::*;

use crate::sample::{Sample, iterate};
use crate::shade::{Rgb, Shader};

pub const SUB: usize = 5;
/// Lattice steps per frame of the scripted pan: (6, 3) = (1.2, 0.6) output pixels.
pub const PAN: (usize, usize) = (6, 3);

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub re: f64,
    pub im: f64,
    /// Output pixel size in complex units.
    pub px: f64,
    pub w: usize,
    pub h: usize,
    pub max_iter: u32,
}

pub struct Lattice {
    pub view: View,
    #[allow(dead_code)]
    pub frames: usize,
    pub lw: usize,
    pub lh: usize,
    pub samples: Vec<Sample>,
    pub colors: Vec<Rgb>,
}

impl Lattice {
    pub fn compute(view: View, frames: usize, shader: &Shader) -> Self {
        let lw = SUB * view.w + PAN.0 * (frames - 1);
        let lh = SUB * view.h + PAN.1 * (frames - 1);
        let mut samples = vec![Sample::default(); lw * lh];
        samples.par_chunks_mut(lw).enumerate().for_each(|(y, row)| {
            let v = (y as f64 + 0.5) / SUB as f64;
            let ci = view.im - (v - view.h as f64 / 2.0) * view.px;
            for (x, s) in row.iter_mut().enumerate() {
                let u = (x as f64 + 0.5) / SUB as f64;
                let cr = view.re + (u - view.w as f64 / 2.0) * view.px;
                *s = iterate(cr, ci, view.px, view.max_iter);
            }
        });
        let colors = samples.par_iter().map(|s| shader.shade(s)).collect();
        Self {
            view,
            frames,
            lw,
            lh,
            samples,
            colors,
        }
    }

    /// Lattice index of sample (a, b) of pixel (i, j) in frame t.
    #[inline]
    pub fn idx(&self, t: usize, i: usize, j: usize, a: usize, b: usize) -> usize {
        let x = SUB * i + a + PAN.0 * t;
        let y = SUB * j + b + PAN.1 * t;
        y * self.lw + x
    }

    #[inline]
    pub fn centre(&self, t: usize, i: usize, j: usize) -> usize {
        self.idx(t, i, j, SUB / 2, SUB / 2)
    }
}
