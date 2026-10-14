//! Plain f64 Mandelbrot iteration with the two derivatives the terrain needs.
//!
//! - `dz/dc` for the exterior distance estimate (kb: techniques/distance-estimation),
//!   carried with a separate power-of-two exponent so long orbits do not overflow.
//! - `dz/dz1` for the interior test `|w|·n² < eps` (kb: techniques/interior-detection,
//!   Pitfalls: a fixed threshold paints wedges near parabolic points).
//!
//! No perturbation: c is absolute f64, which is fine down to a view scale of
//! about 1e-13 around |c| ~ 1.

pub const BAILOUT: f64 = 1e5;
const BAILOUT2: f64 = BAILOUT * BAILOUT;
const LN2: f64 = std::f64::consts::LN_2;
const INTERIOR_EPS: f64 = 1e-3;

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    /// Exterior distance estimate in complex-plane units (0 for interior).
    pub de: f64,
    /// Smooth iteration count (negative for interior).
    pub mu: f64,
    /// ln of the potential G = ln|z_n| / 2^n.
    pub log_g: f64,
    /// Unit gradient of G (direction away from the set), complex-plane frame.
    pub grad: (f64, f64),
    pub interior: bool,
    /// Iterations actually spent (cost accounting).
    pub iters: u32,
}

#[inline]
fn in_cardioid_or_bulb(x: f64, y: f64) -> bool {
    let xq = x - 0.25;
    let q = xq * xq + y * y;
    if q * (q + xq) <= 0.25 * y * y {
        return true;
    }
    let xb = x + 1.0;
    xb * xb + y * y <= 1.0 / 16.0
}

pub fn eval(cr: f64, ci: f64, max_iter: u32) -> Sample {
    if in_cardioid_or_bulb(cr, ci) {
        return Sample { interior: true, mu: -1.0, iters: 1, ..Default::default() };
    }
    // State after the first step: z1 = c, dz1/dc = 1, dz1/dz1 = 1.
    let (mut zr, mut zi) = (cr, ci);
    let (mut dr, mut di) = (1.0f64, 0.0f64); // dz/dc stored as dz * 2^-dexp
    let mut dexp: i32 = 0;
    let mut dinc = 1.0f64; // 2^-dexp, the "+1" in stored units
    let (mut wr, mut wi) = (1.0f64, 0.0f64); // dz/dz1 stored as w * 2^-wexp
    let mut wexp: i32 = 0;
    let mut n: u32 = 1;
    const BIG: f64 = 1e100;
    const DOWN: f64 = 1.0 / (1u128 << 100) as f64 / (1u128 << 100) as f64 / (1u128 << 100) as f64; // 2^-300
    loop {
        let r2 = zr * zr + zi * zi;
        if r2 > BAILOUT2 {
            let lnz = 0.5 * r2.ln();
            let absz = r2.sqrt();
            let dabs = (dr * dr + di * di).sqrt();
            let ln_de = (2.0 * absz * lnz).ln() - dabs.ln() - dexp as f64 * LN2;
            let mu = n as f64 - (lnz / BAILOUT.ln()).log2();
            let log_g = lnz.ln() - n as f64 * LN2;
            // grad G ∝ conj(dz / z)
            let gx = (dr * zr + di * zi) / r2;
            let gy = -(di * zr - dr * zi) / r2;
            let gl = (gx * gx + gy * gy).sqrt().max(1e-300);
            return Sample {
                de: ln_de.exp(),
                mu,
                log_g,
                grad: (gx / gl, gy / gl),
                interior: false,
                iters: n,
            };
        }
        if n >= max_iter {
            // Unresolved: treat as lake.
            return Sample { interior: true, mu: -2.0, iters: n, ..Default::default() };
        }
        // derivatives before z
        let tr = 2.0 * (zr * dr - zi * di) + dinc;
        let ti = 2.0 * (zr * di + zi * dr);
        dr = tr;
        di = ti;
        let ur = 2.0 * (zr * wr - zi * wi);
        let ui = 2.0 * (zr * wi + zi * wr);
        wr = ur;
        wi = ui;
        let nzr = zr * zr - zi * zi + cr;
        zi = 2.0 * zr * zi + ci;
        zr = nzr;
        n += 1;
        if n & 7 == 0 {
            if dr.abs() > BIG || di.abs() > BIG {
                dr *= DOWN;
                di *= DOWN;
                dexp += 300;
                dinc = if dexp > 900 { 0.0 } else { 2f64.powi(-dexp) }; // avoid subnormals
            }
            if wr.abs() > BIG || wi.abs() > BIG {
                wr *= DOWN;
                wi *= DOWN;
                wexp += 300;
            }
            if wexp == 0 {
                let w2 = wr * wr + wi * wi;
                let nf = n as f64;
                if w2 * nf * nf * nf * nf < INTERIOR_EPS * INTERIOR_EPS {
                    return Sample { interior: true, mu: -1.0, iters: n, ..Default::default() };
                }
            }
        }
    }
}

const LANES: usize = 8;

#[derive(Clone, Copy, Default)]
struct Lane {
    idx: usize,
    live: bool,
    cr: f64,
    ci: f64,
    zr: f64,
    zi: f64,
    dr: f64,
    di: f64,
    dinc: f64,
    dexp: i32,
    wr: f64,
    wi: f64,
    wexp: i32,
    n: u32,
}

/// Same result as `eval` for every point, but iterates `LANES` independent orbits
/// interleaved (a lane that finishes takes the next point), which hides the latency of
/// the z² + c dependency chain: ~3× the throughput of the scalar loop.
pub fn eval_many(pts: &[(f64, f64)], out: &mut [Sample], max_iter: u32) {
    const BIG: f64 = 1e100;
    let down: f64 = (-300f64).exp2();
    let mut next = 0usize;
    let mut lanes = [Lane::default(); LANES];
    let mut live = 0usize;
    let refill = |l: &mut Lane, next: &mut usize, out: &mut [Sample]| -> bool {
        while *next < pts.len() {
            let i = *next;
            *next += 1;
            let (cr, ci) = pts[i];
            if in_cardioid_or_bulb(cr, ci) {
                out[i] = Sample { interior: true, mu: -1.0, iters: 1, ..Default::default() };
                continue;
            }
            *l = Lane { idx: i, live: true, cr, ci, zr: cr, zi: ci, dr: 1.0, di: 0.0, dinc: 1.0, dexp: 0, wr: 1.0, wi: 0.0, wexp: 0, n: 1 };
            return true;
        }
        l.live = false;
        false
    };
    for l in lanes.iter_mut() {
        if refill(l, &mut next, out) {
            live += 1;
        }
    }
    while live > 0 {
        // 8 steps on structure-of-arrays copies, branch-free (frozen lanes keep their
        // values), so the lanes' dependency chains overlap and LLVM can use NEON
        let mut zr = [0.0f64; LANES];
        let mut zi = [0.0f64; LANES];
        let mut dr = [0.0f64; LANES];
        let mut di = [0.0f64; LANES];
        let mut wr = [0.0f64; LANES];
        let mut wi = [0.0f64; LANES];
        let mut cr = [0.0f64; LANES];
        let mut ci = [0.0f64; LANES];
        let mut dinc = [0.0f64; LANES];
        let mut nn = [0u32; LANES];
        let mut lim = [0u32; LANES];
        for (k, l) in lanes.iter().enumerate() {
            zr[k] = l.zr;
            zi[k] = l.zi;
            dr[k] = l.dr;
            di[k] = l.di;
            wr[k] = l.wr;
            wi[k] = l.wi;
            cr[k] = l.cr;
            ci[k] = l.ci;
            dinc[k] = l.dinc;
            nn[k] = l.n;
            lim[k] = if l.live { max_iter } else { 0 };
        }
        for _ in 0..8 {
            for k in 0..LANES {
                let (a, b) = (zr[k], zi[k]);
                let go = a * a + b * b <= BAILOUT2 && nn[k] < lim[k];
                let tr = 2.0 * (a * dr[k] - b * di[k]) + dinc[k];
                let ti = 2.0 * (a * di[k] + b * dr[k]);
                let ur = 2.0 * (a * wr[k] - b * wi[k]);
                let ui = 2.0 * (a * wi[k] + b * wr[k]);
                let nzr = a * a - b * b + cr[k];
                let nzi = 2.0 * a * b + ci[k];
                if go {
                    dr[k] = tr;
                    di[k] = ti;
                    wr[k] = ur;
                    wi[k] = ui;
                    zr[k] = nzr;
                    zi[k] = nzi;
                }
                nn[k] += go as u32;
            }
        }
        let stop = [false; LANES];
        for (k, l) in lanes.iter_mut().enumerate() {
            l.zr = zr[k];
            l.zi = zi[k];
            l.dr = dr[k];
            l.di = di[k];
            l.wr = wr[k];
            l.wi = wi[k];
            l.n = nn[k];
        }
        for (k, l) in lanes.iter_mut().enumerate() {
            if !l.live {
                continue;
            }
            let (zr, zi, dr, di, n) = (l.zr, l.zi, l.dr, l.di, l.n);
            let done = stop[k] || zr * zr + zi * zi > BAILOUT2 || n >= max_iter;
            let mut result: Option<Sample> = None;
            if done {
                let r2 = zr * zr + zi * zi;
                if r2 > BAILOUT2 {
                    result = Some(finish(zr, zi, dr, di, l.dexp, n));
                } else {
                    result = Some(Sample { interior: true, mu: -2.0, iters: n, ..Default::default() });
                }
            } else {
                if l.dr.abs() > BIG || l.di.abs() > BIG {
                    l.dr *= down;
                    l.di *= down;
                    l.dexp += 300;
                    l.dinc = if l.dexp > 900 { 0.0 } else { (-l.dexp as f64).exp2() };
                }
                if l.wr.abs() > BIG || l.wi.abs() > BIG {
                    l.wr *= down;
                    l.wi *= down;
                    l.wexp += 300;
                }
                if l.wexp == 0 {
                    let w2 = l.wr * l.wr + l.wi * l.wi;
                    let nf = n as f64;
                    if w2 * nf * nf * nf * nf < INTERIOR_EPS * INTERIOR_EPS {
                        result = Some(Sample { interior: true, mu: -1.0, iters: n, ..Default::default() });
                    }
                }
            }
            if let Some(r) = result {
                out[l.idx] = r;
                if !refill(l, &mut next, out) {
                    live -= 1;
                }
            }
        }
    }
}

fn finish(zr: f64, zi: f64, dr: f64, di: f64, dexp: i32, n: u32) -> Sample {
    let r2 = zr * zr + zi * zi;
    let lnz = 0.5 * r2.ln();
    let absz = r2.sqrt();
    let dabs = (dr * dr + di * di).sqrt();
    let ln_de = (2.0 * absz * lnz).ln() - dabs.ln() - dexp as f64 * LN2;
    let mu = n as f64 - (lnz / BAILOUT.ln()).log2();
    let log_g = lnz.ln() - n as f64 * LN2;
    let gx = (dr * zr + di * zi) / r2;
    let gy = -(di * zr - dr * zi) / r2;
    let gl = (gx * gx + gy * gy).sqrt().max(1e-300);
    Sample { de: ln_de.exp(), mu, log_g, grad: (gx / gl, gy / gl), interior: false, iters: n }
}

/// Walk from `c` towards the set along -grad G with steps of half the DE until the
/// DE drops below `tol` (or we step into the interior; then bisect the last step).
/// Returns (boundary point, outward unit direction at the last exterior point).
pub fn snap_to_boundary(c: (f64, f64), tol: f64, max_iter: u32) -> ((f64, f64), (f64, f64)) {
    let mut p = c;
    let mut out = (1.0, 0.0);
    let s = eval(p.0, p.1, max_iter);
    if s.interior {
        // walk outwards: try rays in 16 directions with growing radius
        let mut r = tol;
        for _ in 0..80 {
            for k in 0..16 {
                let a = k as f64 * std::f64::consts::TAU / 16.0;
                let q = (c.0 + r * a.cos(), c.1 + r * a.sin());
                if !eval(q.0, q.1, max_iter).interior {
                    return snap_to_boundary(q, tol, max_iter);
                }
            }
            r *= 1.5;
        }
        return (c, out);
    }
    for _ in 0..400 {
        let s = eval(p.0, p.1, max_iter);
        if s.interior {
            break;
        }
        out = s.grad;
        if s.de < tol {
            return (p, out);
        }
        let step = 0.5 * s.de;
        let q = (p.0 - step * s.grad.0, p.1 - step * s.grad.1);
        if eval(q.0, q.1, max_iter).interior {
            // bisect between p (exterior) and q (interior)
            let (mut a, mut b) = (p, q);
            for _ in 0..60 {
                let m = (0.5 * (a.0 + b.0), 0.5 * (a.1 + b.1));
                if eval(m.0, m.1, max_iter).interior {
                    b = m;
                } else {
                    a = m;
                }
                if ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() < tol {
                    break;
                }
            }
            return (a, out);
        }
        p = q;
    }
    (p, out)
}

/// Find a minibrot near `c`: ball-period for a disk of radius `r`, then Newton for
/// the nucleus. Returns (nucleus, period, complex size estimate) if it converged.
pub fn find_minibrot(c: (f64, f64), r: f64, max_period: u32) -> Option<((f64, f64), u32, (f64, f64))> {
    // ball period: first n with |z_n| < |dz_n/dc| * r
    let (mut zr, mut zi) = (0.0f64, 0.0f64);
    let (mut dr, mut di) = (0.0f64, 0.0f64);
    let mut period = 0;
    for n in 1..=max_period {
        let tr = 2.0 * (zr * dr - zi * di) + 1.0;
        let ti = 2.0 * (zr * di + zi * dr);
        dr = tr;
        di = ti;
        let nzr = zr * zr - zi * zi + c.0;
        zi = 2.0 * zr * zi + c.1;
        zr = nzr;
        if zr * zr + zi * zi > 1e10 {
            return None;
        }
        if (zr * zr + zi * zi).sqrt() < (dr * dr + di * di).sqrt() * r {
            period = n;
            break;
        }
    }
    if period == 0 {
        return None;
    }
    // Newton on z_p(c) = 0
    let mut cc = c;
    for _ in 0..64 {
        let (mut zr, mut zi, mut dr, mut di) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for _ in 0..period {
            let tr = 2.0 * (zr * dr - zi * di) + 1.0;
            let ti = 2.0 * (zr * di + zi * dr);
            dr = tr;
            di = ti;
            let nzr = zr * zr - zi * zi + cc.0;
            zi = 2.0 * zr * zi + cc.1;
            zr = nzr;
        }
        let den = dr * dr + di * di;
        if den == 0.0 {
            return None;
        }
        let qr = (zr * dr + zi * di) / den;
        let qi = (zi * dr - zr * di) / den;
        cc = (cc.0 - qr, cc.1 - qi);
        if (qr * qr + qi * qi).sqrt() < 1e-17_f64.max(cc.0.abs().max(cc.1.abs()) * 1e-16) {
            break;
        }
    }
    // size estimate (mathr): z=0,l=1,b=1; for q in 1..p: z=z²+c; l=2zl; b+=1/l; size=1/(b l²)
    let (mut zr, mut zi) = (0.0f64, 0.0f64);
    let (mut lr, mut li) = (1.0f64, 0.0f64);
    let (mut br, mut bi) = (1.0f64, 0.0f64);
    for _ in 1..period {
        let nzr = zr * zr - zi * zi + cc.0;
        zi = 2.0 * zr * zi + cc.1;
        zr = nzr;
        let nlr = 2.0 * (zr * lr - zi * li);
        li = 2.0 * (zr * li + zi * lr);
        lr = nlr;
        let d = lr * lr + li * li;
        br += lr / d;
        bi -= li / d;
    }
    // l²
    let l2r = lr * lr - li * li;
    let l2i = 2.0 * lr * li;
    let mr = br * l2r - bi * l2i;
    let mi = br * l2i + bi * l2r;
    let d = mr * mr + mi * mi;
    Some((cc, period, (mr / d, -mi / d)))
}
