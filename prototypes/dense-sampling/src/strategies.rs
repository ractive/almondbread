//! The sampling strategies. Each returns the displayed image of frame `t` and what it cost:
//! iterations (sum of per-sample iteration counts) and samples iterated.

use rayon::prelude::*;

use crate::lattice::{Lattice, SUB};
use crate::sample::Class;
use crate::shade::Rgb;

#[derive(Clone, Debug, Default)]
pub struct Out {
    pub img: Vec<Rgb>,
    pub iters: u64,
    pub samples: u64,
    /// Per pixel: 0 computed, 1 guessed/filled/expected, 2 refined (extra samples).
    pub kind: Vec<u8>,
}

pub fn mean(cs: &[Rgb]) -> Rgb {
    let mut m = [0.0f32; 3];
    for c in cs {
        for k in 0..3 {
            m[k] += c[k];
        }
    }
    let n = cs.len().max(1) as f32;
    [m[0] / n, m[1] / n, m[2] / n]
}

/// Converged reference: box mean of the 16 lattice samples per pixel that no strategy uses
/// (every strategy samples the centre and the 3 x 3 grid at (0, 2, 4)), so a strategy's error
/// is not lowered by samples it shares with the reference. `half`: Some(0|1) uses alternate
/// samples of the 16 (two independent 8-sample references: their difference shows the
/// reference's own noise).
pub fn reference(l: &Lattice, t: usize, half: Option<usize>) -> Vec<Rgb> {
    let (w, h) = (l.view.w, l.view.h);
    (0..w * h)
        .into_par_iter()
        .map(|p| {
            let (i, j) = (p % w, p / w);
            let mut cs = Vec::with_capacity(SUB * SUB);
            let mut idx = 0usize;
            for b in 0..SUB {
                for a in 0..SUB {
                    if a % 2 == 0 && b % 2 == 0 {
                        continue;
                    }
                    if half.is_none_or(|hh| idx % 2 == hh) {
                        cs.push(l.colors[l.idx(t, i, j, a, b)]);
                    }
                    idx += 1;
                }
            }
            mean(&cs)
        })
        .collect()
}

/// One sample per pixel at the centre (the refined frame).
pub fn centre(l: &Lattice, t: usize) -> Out {
    let (w, h) = (l.view.w, l.view.h);
    let mut o = Out {
        img: vec![[0.0; 3]; w * h],
        kind: vec![0; w * h],
        ..Default::default()
    };
    for p in 0..w * h {
        let k = l.centre(t, p % w, p / w);
        o.img[p] = l.colors[k];
        o.iters += l.samples[k].n as u64;
        o.samples += 1;
    }
    o
}

/// The 3 x 3 AA sub-grid inside the 5 x 5 lattice (offsets -0.4, 0, 0.4 px).
pub const AA3: [usize; 3] = [0, 2, 4];

/// FL's adaptive AA list on the centre samples: exterior with DE < 4 px, interior with an
/// exterior neighbour.
pub fn app_aa_list(l: &Lattice, t: usize) -> Vec<bool> {
    let (w, h) = (l.view.w, l.view.h);
    (0..w * h)
        .map(|p| {
            let (i, j) = (p % w, p / w);
            let s = &l.samples[l.centre(t, i, j)];
            match s.class {
                Class::Exterior => s.de < 4.0,
                Class::Interior => {
                    let mut any = false;
                    for dj in -1i32..=1 {
                        for di in -1i32..=1 {
                            let (x, y) = (i as i32 + di, j as i32 + dj);
                            if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                                continue;
                            }
                            any |= l.samples[l.centre(t, x as usize, y as usize)].class == Class::Exterior;
                        }
                    }
                    any
                }
                Class::Unresolved => false,
            }
        })
        .collect()
}

/// FL behaviour at rest: centre sample everywhere, plus 8 more (3 x 3) on the adaptive list.
pub fn app_aa(l: &Lattice, t: usize) -> Out {
    let (w, h) = (l.view.w, l.view.h);
    let mut o = centre(l, t);
    let list = app_aa_list(l, t);
    for p in 0..w * h {
        if !list[p] {
            continue;
        }
        let (i, j) = (p % w, p / w);
        let mut cs = Vec::with_capacity(9);
        for &b in &AA3 {
            for &a in &AA3 {
                let k = l.idx(t, i, j, a, b);
                cs.push(l.colors[k]);
                if !(a == 2 && b == 2) {
                    o.iters += l.samples[k].n as u64;
                    o.samples += 1;
                }
            }
        }
        o.img[p] = mean(&cs);
        o.kind[p] = 2;
    }
    o
}

/// Separable Gaussian blur of a w x h field of (sum colour, weight).
pub fn blur(sum: &mut [[f32; 4]], w: usize, h: usize, sigma: f32) {
    let r = (3.0 * sigma).ceil() as i32;
    let ker: Vec<f32> = (-r..=r)
        .map(|x| (-(x * x) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let mut tmp = vec![[0.0f32; 4]; w * h];
    tmp.par_chunks_mut(w).enumerate().for_each(|(j, row)| {
        for (i, out) in row.iter_mut().enumerate() {
            let mut acc = [0.0f32; 4];
            for (q, kv) in ker.iter().enumerate() {
                let x = i as i32 + q as i32 - r;
                if x < 0 || x >= w as i32 {
                    continue;
                }
                let v = sum[j * w + x as usize];
                for c in 0..4 {
                    acc[c] += v[c] * kv;
                }
            }
            *out = acc;
        }
    });
    sum.par_chunks_mut(w).enumerate().for_each(|(j, row)| {
        for (i, out) in row.iter_mut().enumerate() {
            let mut acc = [0.0f32; 4];
            for (q, kv) in ker.iter().enumerate() {
                let y = j as i32 + q as i32 - r;
                if y < 0 || y >= h as i32 {
                    continue;
                }
                let v = tmp[y as usize * w + i];
                for c in 0..4 {
                    acc[c] += v[c] * kv;
                }
            }
            *out = acc;
        }
    });
}

fn hash(i: usize, j: usize) -> u32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B1) ^ (j as u32).wrapping_mul(0x85EB_CA77);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x
}

#[derive(Clone, Copy, Debug)]
pub struct TextureParams {
    /// Footprint threshold index into `sample::EXPAND`.
    pub k: usize,
    /// One probe per `probe_every` texture pixels (by hash): the probe runs to escape and its
    /// colour feeds the expected-colour field.
    pub probe_every: u32,
    /// Gaussian sigma of the expected-colour field, output pixels.
    pub sigma: f32,
    /// Also add the 3 x 3 AA samples on the FL adaptive list (non-texture pixels only;
    /// texture subsamples take the expected colour at their cut cost).
    pub aa: bool,
}

/// Footprint cut ("texture" early termination): a sample stops as soon as its footprint
/// `|dz/dc| * pixel` reaches T before it escapes; such pixels take the local expected colour of
/// the texture, estimated from sparse probes that do run to escape.
pub fn texture(l: &Lattice, t: usize, p: TextureParams) -> Out {
    let (w, h) = (l.view.w, l.view.h);
    let mut o = Out {
        img: vec![[0.0; 3]; w * h],
        kind: vec![0; w * h],
        ..Default::default()
    };
    let mut field = vec![[0.0f32; 4]; w * h];
    let mut tex = vec![false; w * h];
    let mut gsum = [0.0f64; 4];
    for q in 0..w * h {
        let (i, j) = (q % w, q / w);
        let k = l.centre(t, i, j);
        let s = &l.samples[k];
        o.samples += 1;
        if s.is_texture(p.k) {
            tex[q] = true;
            o.kind[q] = 1;
            o.iters += s.cut_cost(p.k) as u64;
            if hash(i, j) % p.probe_every == 0 {
                o.iters += (s.n - s.cut_cost(p.k)) as u64;
                let c = l.colors[k];
                field[q] = [c[0], c[1], c[2], 1.0];
                for c2 in 0..3 {
                    gsum[c2] += c[c2] as f64;
                }
                gsum[3] += 1.0;
            }
        } else {
            o.iters += s.n as u64;
            o.img[q] = l.colors[k];
        }
    }
    blur(&mut field, w, h, p.sigma);
    let global = if gsum[3] > 0.0 {
        [
            (gsum[0] / gsum[3]) as f32,
            (gsum[1] / gsum[3]) as f32,
            (gsum[2] / gsum[3]) as f32,
        ]
    } else {
        [0.5; 3]
    };
    let expected = |q: usize| -> Rgb {
        let f = field[q];
        if f[3] > 1e-3 {
            [f[0] / f[3], f[1] / f[3], f[2] / f[3]]
        } else {
            global
        }
    };
    for q in 0..w * h {
        if tex[q] {
            o.img[q] = expected(q);
        }
    }
    if p.aa {
        let list = app_aa_list(l, t);
        for q in 0..w * h {
            if !list[q] || tex[q] {
                continue;
            }
            let (i, j) = (q % w, q / w);
            let e = expected(q);
            let mut cs = Vec::with_capacity(9);
            for &b in &AA3 {
                for &a in &AA3 {
                    let k = l.idx(t, i, j, a, b);
                    let s = &l.samples[k];
                    if a == 2 && b == 2 {
                        cs.push(l.colors[k]);
                        continue;
                    }
                    o.samples += 1;
                    if s.is_texture(p.k) {
                        o.iters += s.cut_cost(p.k) as u64;
                        cs.push(e);
                    } else {
                        o.iters += s.n as u64;
                        cs.push(l.colors[k]);
                    }
                }
            }
            o.img[q] = mean(&cs);
            o.kind[q] = 2;
        }
    }
    o
}

/// Coarse-to-fine guessing on centre samples (KF-style interlaced guessing, in colour): pixels
/// on a grid of step 8 are computed; each finer level guesses a new pixel as the mean of its
/// known neighbours (cell corners or edge ends) when they share a class and their colours are
/// within `tau` (ΔE x 100), else computes it.
pub fn guess(l: &Lattice, t: usize, tau: f32) -> Out {
    let (w, h) = (l.view.w, l.view.h);
    let mut o = Out {
        img: vec![[0.0; 3]; w * h],
        kind: vec![0; w * h],
        ..Default::default()
    };
    let mut known = vec![false; w * h];
    let mut class = vec![0u8; w * h];
    let compute = |o: &mut Out, known: &mut [bool], class: &mut [u8], i: usize, j: usize| {
        let k = l.centre(t, i, j);
        let q = j * w + i;
        o.img[q] = l.colors[k];
        o.iters += l.samples[k].n as u64;
        o.samples += 1;
        known[q] = true;
        class[q] = l.samples[k].class as u8;
        o.kind[q] = 0;
    };
    const S0: usize = 8;
    for j in (0..h).step_by(S0) {
        for i in (0..w).step_by(S0) {
            compute(&mut o, &mut known, &mut class, i, j);
        }
    }
    let mut s = S0;
    while s > 1 {
        let hs = s / 2;
        // Two passes: cell centres (from 4 corners), then edge midpoints (from 2 ends + the
        // 2 adjacent centres where they exist).
        for pass in 0..2 {
            for j in (0..h).step_by(hs) {
                for i in (0..w).step_by(hs) {
                    let q = j * w + i;
                    if known[q] {
                        continue;
                    }
                    let (oi, oj) = (i % s, j % s);
                    let nbrs: Vec<(i64, i64)> = match (pass, oi == hs, oj == hs) {
                        (0, true, true) => vec![(-1, -1), (1, -1), (-1, 1), (1, 1)],
                        (1, true, false) => vec![(-1, 0), (1, 0), (0, -1), (0, 1)],
                        (1, false, true) => vec![(0, -1), (0, 1), (-1, 0), (1, 0)],
                        _ => continue,
                    };
                    let mut cs = Vec::new();
                    let mut cl = Vec::new();
                    let mut ok = true;
                    for (k, (di, dj)) in nbrs.iter().enumerate() {
                        let (x, y) = (i as i64 + di * hs as i64, j as i64 + dj * hs as i64);
                        let inside = x >= 0 && y >= 0 && x < w as i64 && y < h as i64;
                        let need = pass == 0 || k < 2;
                        if !inside || !known[y as usize * w + x as usize] {
                            if need {
                                ok = false;
                            }
                            continue;
                        }
                        let qq = y as usize * w + x as usize;
                        cs.push(o.img[qq]);
                        cl.push(class[qq]);
                    }
                    if ok && cl.iter().all(|&c| c == cl[0]) {
                        let mut maxd = 0.0f32;
                        for a in 0..cs.len() {
                            for b in a + 1..cs.len() {
                                maxd = maxd.max(crate::shade::delta_e(cs[a], cs[b]));
                            }
                        }
                        if maxd <= tau {
                            o.img[q] = mean(&cs);
                            known[q] = true;
                            class[q] = cl[0];
                            o.kind[q] = 1;
                            continue;
                        }
                    }
                    compute(&mut o, &mut known, &mut class, i, j);
                }
            }
        }
        s = hs;
    }
    o
}

/// Mariani–Silver on interior only: a rectangle whose computed border is all interior is filled
/// as interior without computing its inside; otherwise split (down to 4 px). Returns the image
/// and the number of filled pixels that the full render has as exterior (holes lost).
pub fn mariani(l: &Lattice, t: usize, tile: usize) -> (Out, usize) {
    let (w, h) = (l.view.w, l.view.h);
    let mut o = Out {
        img: vec![[0.0; 3]; w * h],
        kind: vec![0; w * h],
        ..Default::default()
    };
    let mut done = vec![false; w * h];
    let mut wrong = 0usize;
    fn rect(
        l: &Lattice,
        t: usize,
        o: &mut Out,
        done: &mut [bool],
        wrong: &mut usize,
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
    ) {
        let w = l.view.w;
        let comp = |o: &mut Out, done: &mut [bool], i: usize, j: usize| -> bool {
            let q = j * w + i;
            let k = l.centre(t, i, j);
            if !done[q] {
                done[q] = true;
                o.img[q] = l.colors[k];
                o.iters += l.samples[k].n as u64;
                o.samples += 1;
            }
            l.samples[k].class == Class::Interior
        };
        let mut all_in = true;
        for i in x0..=x1 {
            all_in &= comp(o, done, i, y0);
            all_in &= comp(o, done, i, y1);
        }
        for j in y0..=y1 {
            all_in &= comp(o, done, x0, j);
            all_in &= comp(o, done, x1, j);
        }
        if all_in && x1 > x0 + 1 && y1 > y0 + 1 {
            let inside = l.colors[l.centre(t, x0, y0)];
            for j in y0 + 1..y1 {
                for i in x0 + 1..x1 {
                    let q = j * w + i;
                    if !done[q] {
                        done[q] = true;
                        o.img[q] = inside;
                        o.kind[q] = 1;
                        if l.samples[l.centre(t, i, j)].class != Class::Interior {
                            *wrong += 1;
                        }
                    }
                }
            }
            return;
        }
        if x1 - x0 <= 4 || y1 - y0 <= 4 {
            for j in y0..=y1 {
                for i in x0..=x1 {
                    comp(o, done, i, j);
                }
            }
            return;
        }
        let (xm, ym) = ((x0 + x1) / 2, (y0 + y1) / 2);
        rect(l, t, o, done, wrong, x0, y0, xm, ym);
        rect(l, t, o, done, wrong, xm, y0, x1, ym);
        rect(l, t, o, done, wrong, x0, ym, xm, y1);
        rect(l, t, o, done, wrong, xm, ym, x1, y1);
    }
    let mut y = 0;
    while y < h {
        let mut x = 0;
        let y1 = (y + tile).min(h - 1);
        while x < w {
            let x1 = (x + tile).min(w - 1);
            rect(l, t, &mut o, &mut done, &mut wrong, x, y, x1, y1);
            x = x1;
            if x1 == w - 1 {
                break;
            }
        }
        y = y1;
        if y1 == h - 1 {
            break;
        }
    }
    (o, wrong)
}
