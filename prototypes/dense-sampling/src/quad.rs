//! Hierarchical (quadtree) local series approximation.

use rayon::prelude::*;

use crate::branch::{Seed, continue_from, seed, seed_from};
use crate::lattice::{Lattice, SUB};
use crate::sa::{SaStats, coord, same};
use crate::shade::{Rgb, Shader};
use crate::strategies::{AA3, Out, app_aa_list, mean};

struct Acc {
    px: Vec<(usize, Rgb, u8)>,
    iters: u64,
    seed_iters: u64,
    samples: u64,
    branched: u64,
    differs: u64,
    share: f64,
}

struct Ctx<'a> {
    l: &'a Lattice,
    t: usize,
    list: &'a [bool],
    shader: &'a Shader,
    eps: f64,
}

fn node(cx: &Ctx, parent: &Seed, pc: (f64, f64), i0: usize, j0: usize, size: usize, acc: &mut Acc) {
    let l = cx.l;
    let t = cx.t;
    let (w, h) = (l.view.w, l.view.h);
    let px = l.view.px;
    let mi = l.view.max_iter;
    if i0 >= w || j0 >= h {
        return;
    }
    if size == 1 {
        let q = j0 * w + i0;
        let cp = coord(l, t, i0 as f64 + 0.5, j0 as f64 + 0.5);
        let dp = (cp.0 - pc.0, cp.1 - pc.1);
        let exact = &l.samples[l.centre(t, i0, j0)];
        let (centre, ps) = if cx.list[q] {
            let ps = seed_from(parent, cp, dp, 0.4 * std::f64::consts::SQRT_2 * px, cx.eps, mi);
            acc.seed_iters += (ps.n() - parent.n()) as u64;
            (continue_from(&ps, cp, (0.0, 0.0), px, mi), Some(ps))
        } else {
            (continue_from(parent, cp, dp, px, mi), None)
        };
        acc.iters += (centre.n - parent.n()) as u64;
        acc.samples += 1;
        acc.branched += 1;
        acc.share += parent.n() as f64 / exact.n.max(1) as f64;
        if !same(&centre, exact) {
            acc.differs += 1;
        }
        let mut cols = vec![cx.shader.shade(&centre)];
        if let Some(ps) = ps {
            for &b in &AA3 {
                for &a in &AA3 {
                    if a == 2 && b == 2 {
                        continue;
                    }
                    let u = i0 as f64 + 0.5 + (a as f64 - 2.0) / SUB as f64;
                    let v = j0 as f64 + 0.5 + (b as f64 - 2.0) / SUB as f64;
                    let c = coord(l, t, u, v);
                    let s = continue_from(&ps, c, (c.0 - cp.0, c.1 - cp.1), px, mi);
                    acc.iters += (s.n - ps.n()) as u64;
                    acc.samples += 1;
                    cols.push(cx.shader.shade(&s));
                }
            }
        }
        acc.px.push((q, mean(&cols), if cols.len() > 1 { 2 } else { 0 }));
        return;
    }
    let half = size / 2;
    for (di, dj) in [(0, 0), (half, 0), (0, half), (half, half)] {
        let (ci0, cj0) = (i0 + di, j0 + dj);
        if ci0 >= w || cj0 >= h {
            continue;
        }
        if half == 1 {
            node(cx, parent, pc, ci0, cj0, 1, acc);
            continue;
        }
        let cc = coord(l, t, ci0 as f64 + half as f64 / 2.0, cj0 as f64 + half as f64 / 2.0);
        let r = (half as f64 / 2.0) * std::f64::consts::SQRT_2 * px;
        let s = seed_from(parent, cc, (cc.0 - pc.0, cc.1 - pc.1), r, cx.eps, mi);
        acc.iters += (s.n() - parent.n()) as u64;
        acc.seed_iters += (s.n() - parent.n()) as u64;
        node(cx, &s, cc, ci0, cj0, half, acc);
    }
}

/// Quadtree local series approximation: a `top` x `top` tile (power of two) is a seed; each
/// node's four children re-seed from their parent's branch state with half the radius
/// (`seed_from`), down to 2 x 2 nodes; pixels continue from their 2 x 2 node or, on the FL AA
/// list with `aa`, become seeds for their own 3 x 3 AA samples. A fold (the orbit passing near
/// 0 inside the region) stops a large node early; its children pick up from there instead of
/// from the start. Returns the image, stats, and the share of iterations run in seed mode
/// (with the second derivative).
pub fn quad_sa(l: &Lattice, t: usize, top: usize, eps: f64, aa: bool, shader: &Shader) -> (Out, SaStats, f64) {
    let (w, h) = (l.view.w, l.view.h);
    let list = if aa { app_aa_list(l, t) } else { vec![false; w * h] };
    let tiles_x = w.div_ceil(top);
    let tiles_y = h.div_ceil(top);
    let cx = Ctx {
        l,
        t,
        list: &list,
        shader,
        eps,
    };
    let results: Vec<Acc> = (0..tiles_x * tiles_y)
        .into_par_iter()
        .map(|ti| {
            let (i0, j0) = ((ti % tiles_x) * top, (ti / tiles_x) * top);
            let c0 = coord(l, t, i0 as f64 + top as f64 / 2.0, j0 as f64 + top as f64 / 2.0);
            let r = (top as f64 / 2.0) * std::f64::consts::SQRT_2 * l.view.px;
            let sd = seed(c0, r, eps, l.view.max_iter);
            let mut acc = Acc {
                px: Vec::new(),
                iters: sd.n() as u64,
                seed_iters: sd.n() as u64,
                samples: 0,
                branched: 0,
                differs: 0,
                share: 0.0,
            };
            node(&cx, &sd, c0, i0, j0, top, &mut acc);
            acc
        })
        .collect();
    let mut out = Out {
        img: vec![[0.0; 3]; w * h],
        kind: vec![0; w * h],
        ..Default::default()
    };
    let (mut br, mut df, mut si, mut sh) = (0u64, 0u64, 0u64, 0.0f64);
    for r in results {
        for (q, c, kd) in r.px {
            out.img[q] = c;
            out.kind[q] = kd;
        }
        out.iters += r.iters;
        out.samples += r.samples;
        br += r.branched;
        df += r.differs;
        si += r.seed_iters;
        sh += r.share;
    }
    let st = SaStats {
        branch_share: sh / br.max(1) as f64,
        differs: df as f64 / br.max(1) as f64,
        dn: f64::NAN,
        dde: f64::NAN,
    };
    let seed_share = si as f64 / out.iters.max(1) as f64;
    (out, st, seed_share)
}
