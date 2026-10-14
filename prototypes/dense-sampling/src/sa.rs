//! Experiments with the local series approximation (branch.rs): AA samples branching off their
//! pixel's centre orbit, and every pixel of a k x k tile branching off the tile centre's orbit.

use rayon::prelude::*;

use crate::branch::{continue_from, seed, seed_from};
use crate::lattice::{Lattice, PAN, SUB};
use crate::sample::{Class, Sample};
use crate::shade::{Rgb, Shader};
use crate::strategies::{AA3, Out, app_aa_list, mean};

/// Complex coordinate of pixel-space point (u, v) of frame t (u, v in output pixels of the
/// frame, pixel centres at i + 0.5).
pub fn coord(l: &Lattice, t: usize, u: f64, v: f64) -> (f64, f64) {
    let vw = &l.view;
    let u = u + (PAN.0 * t) as f64 / SUB as f64;
    let v = v + (PAN.1 * t) as f64 / SUB as f64;
    (
        vw.re + (u - vw.w as f64 / 2.0) * vw.px,
        vw.im - (v - vw.h as f64 / 2.0) * vw.px,
    )
}

#[derive(Clone, Debug, Default)]
pub struct SaStats {
    /// Branch iteration / the exact sample's iteration count, averaged over branched samples.
    pub branch_share: f64,
    /// Branched samples whose escape iteration (or class) differs from the exact sample.
    pub differs: f64,
    /// Mean (branched n - exact n) over branched exterior samples.
    pub dn: f64,
    /// Mean (branched DE - exact DE), output pixels.
    pub dde: f64,
}

/// Same escape iteration and class.
pub fn same(a: &Sample, b: &Sample) -> bool {
    a.class == b.class && (a.class != Class::Exterior || a.n == b.n)
}

/// Tile series approximation: tiles of k x k pixels; the tile centre is a seed (cost: its
/// iterations up to the branch point); every pixel centre, and with `aa` the FL AA list's 3 x 3
/// samples, continues from the seed. k = 1 with `aa`: AA samples branch off their own pixel
/// centre (whose own sample is the exact one).
pub fn tile_sa(l: &Lattice, t: usize, k: usize, eps: f64, aa: bool, shader: &Shader) -> (Out, SaStats) {
    let (w, h) = (l.view.w, l.view.h);
    let px = l.view.px;
    let mi = l.view.max_iter;
    let list = if aa { app_aa_list(l, t) } else { vec![false; w * h] };
    let tiles_x = w.div_ceil(k);
    let tiles_y = h.div_ceil(k);
    struct TileOut {
        px: Vec<(usize, Rgb, u8)>,
        iters: u64,
        samples: u64,
        share: f64,
        branched: u64,
        differs: u64,
        dn: f64,
        dde: f64,
    }
    let results: Vec<TileOut> = (0..tiles_x * tiles_y)
        .into_par_iter()
        .map(|ti| {
            let (tx, ty) = (ti % tiles_x, ti / tiles_x);
            let (i0, j0) = (tx * k, ty * k);
            let (i1, j1) = ((i0 + k).min(w), (j0 + k).min(h));
            let (cu, cv) = ((i0 + i1) as f64 / 2.0, (j0 + j1) as f64 / 2.0);
            let c0 = coord(l, t, cu, cv);
            let half = if k == 1 {
                0.0
            } else {
                ((i1 - i0).max(j1 - j0)) as f64 / 2.0
            };
            let radius = (half + if aa { 0.4 } else { 0.0 }) * std::f64::consts::SQRT_2 * px;
            let mut o = TileOut {
                px: Vec::new(),
                iters: 0,
                samples: 0,
                share: 0.0,
                branched: 0,
                differs: 0,
                dn: 0.0,
                dde: 0.0,
            };
            // k == 1: the seed is the pixel centre itself, its exact sample is the pixel's.
            let sd = seed(c0, radius, eps, mi);
            if k > 1 {
                o.iters += sd.n() as u64;
            }
            for j in j0..j1 {
                for i in i0..i1 {
                    let q = j * w + i;
                    let mut cols = Vec::with_capacity(9);
                    let subs: Vec<(usize, usize)> = if list[q] {
                        AA3.iter().flat_map(|&b| AA3.iter().map(move |&a| (a, b))).collect()
                    } else {
                        vec![(2, 2)]
                    };
                    for (a, b) in subs {
                        let exact = &l.samples[l.idx(t, i, j, a, b)];
                        if k == 1 && a == 2 && b == 2 {
                            cols.push(l.colors[l.idx(t, i, j, a, b)]);
                            o.iters += exact.n as u64;
                            o.samples += 1;
                            continue;
                        }
                        let u = i as f64 + 0.5 + (a as f64 - 2.0) / SUB as f64;
                        let v = j as f64 + 0.5 + (b as f64 - 2.0) / SUB as f64;
                        let c = coord(l, t, u, v);
                        let s = continue_from(&sd, c, (c.0 - c0.0, c.1 - c0.1), px, mi);
                        o.iters += (s.n - sd.n()) as u64;
                        o.samples += 1;
                        o.branched += 1;
                        o.share += sd.n() as f64 / exact.n.max(1) as f64;
                        if !same(&s, exact) {
                            o.differs += 1;
                        }
                        o.dn += s.n as f64 - exact.n as f64;
                        o.dde += (s.de - exact.de) as f64;
                        cols.push(shader.shade(&s));
                    }
                    let kind = if cols.len() > 1 { 2 } else { 0 };
                    o.px.push((q, mean(&cols), kind));
                }
            }
            o
        })
        .collect();
    let mut out = Out {
        img: vec![[0.0; 3]; w * h],
        kind: vec![0; w * h],
        ..Default::default()
    };
    let mut st = SaStats::default();
    let mut branched = 0u64;
    let mut differs = 0u64;
    for r in results {
        for (q, c, kd) in r.px {
            out.img[q] = c;
            out.kind[q] = kd;
        }
        out.iters += r.iters;
        out.samples += r.samples;
        st.branch_share += r.share;
        branched += r.branched;
        differs += r.differs;
        st.dn += r.dn;
        st.dde += r.dde;
    }
    st.dn /= branched.max(1) as f64;
    st.dde /= branched.max(1) as f64;
    st.branch_share /= branched.max(1) as f64;
    st.differs = differs as f64 / branched.max(1) as f64;
    (out, st)
}

/// Two levels: pixel centres branch off a k x k tile seed and are themselves seeds for their
/// AA samples (FL list). Cost: tile seed to its branch point, each pixel from the tile branch
/// to its escape, each AA sample from its pixel's branch point. Also returns how many of the
/// iterations ran in seed mode (with the second derivative, the more expensive step).
pub fn nested_sa(l: &Lattice, t: usize, k: usize, eps: f64, shader: &Shader) -> (Out, SaStats, f64) {
    let (w, h) = (l.view.w, l.view.h);
    let px = l.view.px;
    let mi = l.view.max_iter;
    let list = app_aa_list(l, t);
    let tiles_x = w.div_ceil(k);
    let tiles_y = h.div_ceil(k);
    let results: Vec<(Vec<(usize, Rgb, u8)>, u64, u64, u64, u64, u64)> = (0..tiles_x * tiles_y)
        .into_par_iter()
        .map(|ti| {
            let (tx, ty) = (ti % tiles_x, ti / tiles_x);
            let (i0, j0) = (tx * k, ty * k);
            let (i1, j1) = ((i0 + k).min(w), (j0 + k).min(h));
            let c0 = coord(l, t, (i0 + i1) as f64 / 2.0, (j0 + j1) as f64 / 2.0);
            let half = ((i1 - i0).max(j1 - j0)) as f64 / 2.0;
            let sd = seed(c0, half * std::f64::consts::SQRT_2 * px, eps, mi);
            let (mut iters, mut seed_iters, mut samples, mut branched, mut differs) =
                (sd.n() as u64, sd.n() as u64, 0u64, 0u64, 0u64);
            let mut px_out = Vec::new();
            for j in j0..j1 {
                for i in i0..i1 {
                    let q = j * w + i;
                    let cp = coord(l, t, i as f64 + 0.5, j as f64 + 0.5);
                    let dp = (cp.0 - c0.0, cp.1 - c0.1);
                    let exact_c = &l.samples[l.centre(t, i, j)];
                    let mut cols = Vec::with_capacity(9);
                    let ps = if list[q] {
                        let s2 = seed_from(&sd, cp, dp, 0.4 * std::f64::consts::SQRT_2 * px, eps, mi);
                        seed_iters += (s2.n() - sd.n()) as u64;
                        s2
                    } else {
                        sd
                    };
                    let centre = if list[q] {
                        continue_from(&ps, cp, (0.0, 0.0), px, mi)
                    } else {
                        continue_from(&sd, cp, dp, px, mi)
                    };
                    iters += (centre.n - sd.n()) as u64;
                    samples += 1;
                    branched += 1;
                    if !same(&centre, exact_c) {
                        differs += 1;
                    }
                    cols.push(shader.shade(&centre));
                    if list[q] {
                        for &b in &AA3 {
                            for &a in &AA3 {
                                if a == 2 && b == 2 {
                                    continue;
                                }
                                let u = i as f64 + 0.5 + (a as f64 - 2.0) / SUB as f64;
                                let v = j as f64 + 0.5 + (b as f64 - 2.0) / SUB as f64;
                                let c = coord(l, t, u, v);
                                let s = continue_from(&ps, c, (c.0 - cp.0, c.1 - cp.1), px, mi);
                                iters += (s.n - ps.n()) as u64;
                                samples += 1;
                                cols.push(shader.shade(&s));
                            }
                        }
                    }
                    px_out.push((q, mean(&cols), if list[q] { 2 } else { 0 }));
                }
            }
            (px_out, iters, samples, branched, differs, seed_iters)
        })
        .collect();
    let mut out = Out {
        img: vec![[0.0; 3]; w * h],
        kind: vec![0; w * h],
        ..Default::default()
    };
    let (mut br, mut df, mut si) = (0u64, 0u64, 0u64);
    for (pxs, it, sm, b, d, s) in results {
        for (q, c, kd) in pxs {
            out.img[q] = c;
            out.kind[q] = kd;
        }
        out.iters += it;
        out.samples += sm;
        br += b;
        df += d;
        si += s;
    }
    let st = SaStats {
        branch_share: f64::NAN,
        differs: df as f64 / br.max(1) as f64,
        dn: f64::NAN,
        dde: f64::NAN,
    };
    let seed_share = si as f64 / out.iters.max(1) as f64;
    (out, st, seed_share)
}
