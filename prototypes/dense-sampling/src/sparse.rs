//! Sparse sampling with texture fill, and the minibrot-loss metric.

use crate::lattice::{Lattice, SUB};
use crate::sample::Class;
use crate::shade::Rgb;
use crate::strategies::{AA3, Out, app_aa_list, blur, mean};

#[derive(Clone, Copy, Debug)]
pub struct SparseParams {
    /// Grid step of the first pass (1: every pixel computed, the texture fill only denoises).
    pub g: usize,
    /// Texture predicate on a computed sample: exterior with DE below this (output pixels).
    pub dmax: f32,
    /// Gaussian sigma of the expected-colour field, output pixels.
    pub sigma: f32,
    /// FL adaptive 3 x 3 AA on computed non-texture pixels.
    pub aa: bool,
}

/// Sparse sampling with texture fill: pixels on a grid of step g are computed; a g x g cell
/// whose four corners all satisfy the texture predicate (exterior, DE < dmax: structure far
/// below a pixel, where the converged colour is a local average) is not computed: its pixels
/// take the expected colour, a Gaussian-weighted mean of the computed texture samples around.
/// Other cells are computed in full. Computed texture pixels show the expected colour as well
/// (their own sample is one draw of the texture). Optionally FL's AA on the rest.
pub fn sparse(l: &Lattice, t: usize, p: SparseParams) -> Out {
    let (w, h) = (l.view.w, l.view.h);
    let g = p.g.max(1);
    let mut o = Out {
        img: vec![[0.0; 3]; w * h],
        kind: vec![0; w * h],
        ..Default::default()
    };
    let mut computed = vec![false; w * h];
    let tex = |k: usize| {
        let s = &l.samples[k];
        s.class == Class::Exterior && s.de < p.dmax
    };
    let comp = |o: &mut Out, computed: &mut [bool], i: usize, j: usize| {
        let q = j * w + i;
        if !computed[q] {
            computed[q] = true;
            o.iters += l.samples[l.centre(t, i, j)].n as u64;
            o.samples += 1;
        }
    };
    for j in (0..h).step_by(g) {
        for i in (0..w).step_by(g) {
            comp(&mut o, &mut computed, i, j);
        }
    }
    let mut filled = vec![false; w * h];
    if g > 1 {
        for j0 in (0..h).step_by(g) {
            for i0 in (0..w).step_by(g) {
                let (i1, j1) = (i0 + g, j0 + g);
                let corners_ok = i1 < w
                    && j1 < h
                    && [(i0, j0), (i1, j0), (i0, j1), (i1, j1)]
                        .iter()
                        .all(|&(i, j)| computed[j * w + i] && tex(l.centre(t, i, j)));
                for j in j0..(j0 + g).min(h) {
                    for i in i0..(i0 + g).min(w) {
                        if corners_ok {
                            if !computed[j * w + i] {
                                filled[j * w + i] = true;
                            }
                        } else {
                            comp(&mut o, &mut computed, i, j);
                        }
                    }
                }
            }
        }
        for q in 0..w * h {
            if !computed[q] && !filled[q] {
                comp(&mut o, &mut computed, q % w, q / w);
            }
        }
    }
    let mut field = vec![[0.0f32; 4]; w * h];
    for q in 0..w * h {
        if computed[q] {
            let k = l.centre(t, q % w, q / w);
            if tex(k) {
                let c = l.colors[k];
                field[q] = [c[0], c[1], c[2], 1.0];
            }
        }
    }
    blur(&mut field, w, h, p.sigma);
    let list = if p.aa { app_aa_list(l, t) } else { vec![false; w * h] };
    for q in 0..w * h {
        let (i, j) = (q % w, q / w);
        let k = l.centre(t, i, j);
        let f = field[q];
        let expected = if f[3] > 1e-3 {
            Some([f[0] / f[3], f[1] / f[3], f[2] / f[3]])
        } else {
            None
        };
        if filled[q] || (computed[q] && tex(k) && expected.is_some()) {
            o.img[q] = expected.unwrap_or(l.colors[k]);
            o.kind[q] = 1;
            continue;
        }
        o.img[q] = l.colors[k];
        if list[q] {
            let mut cs = Vec::with_capacity(9);
            for &b in &AA3 {
                for &a in &AA3 {
                    let kk = l.idx(t, i, j, a, b);
                    cs.push(l.colors[kk]);
                    if !(a == 2 && b == 2) {
                        o.iters += l.samples[kk].n as u64;
                        o.samples += 1;
                    }
                }
            }
            o.img[q] = mean(&cs);
            o.kind[q] = 2;
        }
    }
    o
}

/// Pixels whose converged reference contains interior (minibrot) samples, and how many of them
/// the strategy shows more than 10 ΔE away from the reference.
pub fn minibrot_loss(l: &Lattice, t: usize, img: &[Rgb], reference: &[Rgb]) -> (usize, usize) {
    let (w, h) = (l.view.w, l.view.h);
    let mut total = 0;
    let mut lost = 0;
    for q in 0..w * h {
        let (i, j) = (q % w, q / w);
        let mut any = false;
        for b in 0..SUB {
            for a in 0..SUB {
                any |= l.samples[l.idx(t, i, j, a, b)].class == Class::Interior;
            }
        }
        if any {
            total += 1;
            if crate::shade::delta_e(img[q], reference[q]) > 10.0 {
                lost += 1;
            }
        }
    }
    (total, lost)
}
