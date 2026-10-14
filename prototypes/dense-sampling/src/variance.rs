//! Variance-stopped AA: FL's adaptive list, but each listed pixel takes its 3 x 3 samples in a
//! fixed order (centre, corners, edges) and stops once the standard error of its mean colour
//! (OKLab, x 100) is below `tau` after at least `min` samples (Mitchell / Painter-Sloan style
//! refinement by observed variance; kb: techniques/adaptive-sampling).

use crate::lattice::Lattice;
use crate::shade::lab;
use crate::strategies::{Out, app_aa_list, centre, mean};

const ORDER: [(usize, usize); 9] = [(2, 2), (0, 0), (4, 4), (4, 0), (0, 4), (2, 0), (2, 4), (0, 2), (4, 2)];

pub fn variance_aa(l: &Lattice, t: usize, tau: f32, min: usize) -> Out {
    let (w, h) = (l.view.w, l.view.h);
    let mut o = centre(l, t);
    let list = app_aa_list(l, t);
    for q in 0..w * h {
        if !list[q] {
            continue;
        }
        let (i, j) = (q % w, q / w);
        let mut cols = Vec::with_capacity(9);
        let mut labs: Vec<[f32; 3]> = Vec::with_capacity(9);
        for (k, &(a, b)) in ORDER.iter().enumerate() {
            let kk = l.idx(t, i, j, a, b);
            cols.push(l.colors[kk]);
            labs.push(lab(l.colors[kk]));
            if k > 0 {
                o.iters += l.samples[kk].n as u64;
                o.samples += 1;
            }
            let n = labs.len();
            if n >= min {
                let mut var = 0.0f32;
                for c in 0..3 {
                    let m = labs.iter().map(|v| v[c]).sum::<f32>() / n as f32;
                    var += labs.iter().map(|v| (v[c] - m).powi(2)).sum::<f32>() / (n - 1) as f32;
                }
                if 100.0 * (var / n as f32).sqrt() < tau {
                    break;
                }
            }
        }
        o.img[q] = mean(&cols);
        o.kind[q] = 2;
    }
    o
}
