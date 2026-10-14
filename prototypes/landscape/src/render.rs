//! Per-pixel height-field ray marching with distance-based level of detail, and shading.

use crate::terrain::{Local, Terrain};
use rayon::prelude::*;
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
pub const fn v3(x: f64, y: f64, z: f64) -> V3 {
    V3 { x, y, z }
}
impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, s: f64) -> V3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}
impl V3 {
    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn norm(self) -> V3 {
        self * (1.0 / self.dot(self).sqrt())
    }
    pub fn mulv(self, o: V3) -> V3 {
        v3(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    pub fn lerp(self, o: V3, t: f64) -> V3 {
        self + (o - self) * t
    }
}

#[derive(Clone, Debug)]
pub struct View {
    pub width: u32,
    pub height: u32,
    /// Heading in radians (0 = +Re axis, counter-clockwise).
    pub heading: f64,
    /// Pitch in radians (negative = looking down).
    pub pitch: f64,
    /// Horizontal field of view in radians.
    pub fov: f64,
    /// Camera altitude above the smoothed ground (world units = S).
    pub altitude: f64,
    /// LOD bias: footprint multiplier (1 = one sample per pixel width).
    pub lod_bias: f64,
    pub sun_az: f64,
    pub sun_el: f64,
    /// Fog distance in world units.
    pub fog: f64,
    pub water: f64,
    pub shadows: bool,
    pub reflections: bool,
    pub ss: u32,
    pub max_dist: f64,
    pub palette_shift: f64,
    /// Planet radius in units of S (0 = flat): far ground drops below the horizon.
    pub curvature: f64,
}

pub struct Stats {
    pub rays: u64,
    pub steps: u64,
}

struct Ctx {
    eye: V3,
    fwd: V3,
    right: V3,
    up: V3,
    sun: V3,
    pix_angle: f64,
}

const SKY_ZENITH: V3 = v3(0.09, 0.24, 0.62);
const SKY_HORIZON: V3 = v3(0.62, 0.72, 0.84);
const SUN_COL: V3 = v3(1.75, 1.45, 1.05);

fn sky(d: V3, sun: V3) -> V3 {
    let t = (d.z.max(0.0) * 1.5).min(1.0).powf(0.6);
    let mut c = SKY_HORIZON.lerp(SKY_ZENITH, t);
    let sd = d.dot(sun).max(0.0);
    c = c + v3(1.0, 0.75, 0.45) * (0.25 * sd.powf(8.0));
    c = c + v3(1.0, 0.9, 0.7) * (0.6 * sd.powf(64.0));
    if sd > 0.9995 {
        c = c + v3(6.0, 5.5, 4.5);
    }
    c
}

fn fog_color(d: V3, sun: V3) -> V3 {
    let sd = d.dot(sun).max(0.0);
    SKY_HORIZON.lerp(v3(0.95, 0.85, 0.7), 0.5 * sd.powf(6.0))
}

/// Cosine palette (Quilez form): a + b cos(2π(c t + d)).
fn pal(t: f64, a: V3, b: V3, c: V3, d: V3) -> V3 {
    let f = |a: f64, b: f64, c: f64, d: f64| a + b * (std::f64::consts::TAU * (c * t + d)).cos();
    v3(f(a.x, b.x, c.x, d.x), f(a.y, b.y, c.y, d.y), f(a.z, b.z, c.z, d.z))
}

fn tonemap(c: V3) -> [u8; 3] {
    // ACES-ish filmic (Narkowicz), then sRGB gamma
    let f = |x: f64| {
        let x = x * 0.8;
        let y = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
        (y.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as u8
    };
    [f(c.x), f(c.y), f(c.z)]
}

pub fn render(terrain: &Terrain, view: &View) -> (Vec<u8>, Stats) {
    let (w, h) = (view.width as usize, view.height as usize);
    // camera height: above the ground smoothed to the scale of the altitude
    let ground = {
        let mut l = Local::new(terrain);
        // average a few samples at the altitude scale
        let a = view.altitude;
        let lvl = (2.0 * a).log2();
        let mut s = 0.0;
        for (dx, dy) in [(0.0, 0.0), (a, 0.0), (-a, 0.0), (0.0, a), (0.0, -a)] {
            s += l.height(dx, dy, lvl);
        }
        s / 5.0
    };
    let eye = v3(0.0, 0.0, ground.max(view.water) + view.altitude);
    let (ch, sh) = (view.heading.cos(), view.heading.sin());
    let (cp, sp) = (view.pitch.cos(), view.pitch.sin());
    let fwd = v3(ch * cp, sh * cp, sp);
    let right = fwd.cross(v3(0.0, 0.0, 1.0)).norm();
    let up = right.cross(fwd);
    let sun_az = view.heading + view.sun_az;
    let sun = v3(sun_az.cos() * view.sun_el.cos(), sun_az.sin() * view.sun_el.cos(), view.sun_el.sin());
    let ctx = Ctx { eye, fwd, right, up, sun, pix_angle: view.fov / view.width as f64 };

    let mut buf = vec![0u8; w * h * 3];
    let steps = std::sync::atomic::AtomicU64::new(0);
    // 16-row bands keep a thread's tile memo coherent
    buf.par_chunks_mut(w * 3 * 8).enumerate().for_each(|(band, out)| {
        let mut local = Local::new(terrain);
        let mut nsteps = 0u64;
        let tanh = (view.fov * 0.5).tan();
        let aspect = view.height as f64 / view.width as f64;
        let rows = out.len() / (w * 3);
        for r in 0..rows {
            let py = band * 8 + r;
            for px in 0..w {
                let mut acc = v3(0.0, 0.0, 0.0);
                let n = view.ss.max(1);
                for sy in 0..n {
                    for sx in 0..n {
                        let u = ((px as f64 + (sx as f64 + 0.5) / n as f64) / w as f64 * 2.0 - 1.0) * tanh;
                        let v = (1.0 - (py as f64 + (sy as f64 + 0.5) / n as f64) / h as f64 * 2.0) * tanh * aspect;
                        let d = (ctx.fwd + ctx.right * u + ctx.up * v).norm();
                        let (c, k) = shade_ray(&mut local, &ctx, view, d);
                        nsteps += k;
                        acc = acc + c;
                    }
                }
                let c = tonemap(acc * (1.0 / (n * n) as f64));
                out[(r * w + px) * 3..(r * w + px) * 3 + 3].copy_from_slice(&c);
            }
        }
        steps.fetch_add(nsteps, std::sync::atomic::Ordering::Relaxed);
    });
    let ss = view.ss.max(1) as u64;
    (buf, Stats { rays: (w * h) as u64 * ss * ss, steps: steps.into_inner() })
}

#[inline]
fn lvl_at(t: f64, ctx: &Ctx, view: &View) -> f64 {
    (t * ctx.pix_angle * view.lod_bias).max(1e-9).log2()
}

/// March a ray against max(terrain, water). Returns (t, hit) and step count.
fn march(local: &mut Local, ctx: &Ctx, view: &View, o: V3, d: V3, t0: f64, t_extra: f64, tmax: f64, max_steps: u32) -> (f64, bool, u64) {
    let mut t = t0;
    let mut tp = t0;
    let mut steps = 0u64;
    for _ in 0..max_steps {
        steps += 1;
        let p = o + d * t;
        let lvl = lvl_at(t + t_extra, ctx, view);
        let hh = local.height(p.x, p.y, lvl).max(view.water) - drop(ctx, view, p);
        let gap = p.z - hh;
        let fp = (t + t_extra) * ctx.pix_angle * view.lod_bias;
        if gap < 0.0 {
            // bisect between tp and t
            let (mut a, mut b) = (tp, t);
            for _ in 0..8 {
                let m = 0.5 * (a + b);
                let q = o + d * m;
                let hm = local.height(q.x, q.y, lvl_at(m + t_extra, ctx, view)).max(view.water) - drop(ctx, view, q);
                if q.z - hm < 0.0 {
                    b = m;
                } else {
                    a = m;
                }
            }
            return (0.5 * (a + b), true, steps);
        }
        if d.z > 0.0 && p.z > 1e4 {
            break;
        }
        tp = t;
        t += (0.45 * gap).max(0.6 * fp).max(1e-4 * t);
        if t > tmax {
            break;
        }
    }
    (t, false, steps)
}

#[inline]
fn drop(ctx: &Ctx, view: &View, p: V3) -> f64 {
    if view.curvature <= 0.0 {
        return 0.0;
    }
    let dx = p.x - ctx.eye.x;
    let dy = p.y - ctx.eye.y;
    (dx * dx + dy * dy) / (2.0 * view.curvature)
}

fn normal_at(local: &mut Local, x: f64, y: f64, lvl: f64) -> V3 {
    let e = lvl.exp2() * 1.0;
    let hx = local.height(x + e, y, lvl) - local.height(x - e, y, lvl);
    let hy = local.height(x, y + e, lvl) - local.height(x, y - e, lvl);
    v3(-hx, -hy, 2.0 * e).norm()
}

fn albedo(view: &View, de: f64, mu: f64, h: f64, n: V3, fp: f64) -> V3 {
    // Sandstone strata keyed by log DE: one colour cycle per ~2.5 octaves of distance,
    // the same at every depth (DE is in units of S, so the bands are self-similar).
    let x = (de.max(1e-6)).log2() * 0.4 + view.palette_shift;
    let mut c = pal(x, v3(0.56, 0.36, 0.24), v3(0.24, 0.16, 0.11), v3(1.0, 1.0, 1.0), v3(0.0, 0.06, 0.13));
    // thin pale layers between strata
    let layer = (0.5 + 0.5 * (std::f64::consts::TAU * x * 3.0).cos()).powi(8);
    c = c.lerp(v3(0.80, 0.72, 0.60), 0.35 * layer);
    // fine bands from the smooth iteration count, faded out where they are not resolved
    if mu > 0.0 {
        let res = (de / (4.0 * fp)).clamp(0.0, 1.0);
        let band = 0.5 + 0.5 * (std::f64::consts::TAU * mu.ln() * 5.0).cos();
        c = c * (1.0 - 0.25 * res * band);
    }
    // vegetation on flat ground near the valley floor (DE < ~0.3 S)
    let flat = ((n.z - 0.82) / 0.12).clamp(0.0, 1.0);
    let low = (1.0 - de / 0.3).clamp(0.0, 1.0);
    c = c.lerp(v3(0.16, 0.27, 0.10), 0.8 * flat * low);
    // pale crust on flat high ground
    let high = ((de - 3.0) / 10.0).clamp(0.0, 1.0);
    c = c.lerp(v3(0.78, 0.74, 0.66), 0.5 * flat * high);
    // steep faces: darker, redder rock
    let steep = ((0.6 - n.z) / 0.4).clamp(0.0, 1.0);
    c = c.lerp(c.mulv(v3(0.75, 0.6, 0.55)), steep);
    let _ = h;
    c
}

fn shade_ray(local: &mut Local, ctx: &Ctx, view: &View, d: V3) -> (V3, u64) {
    let (t, hit, mut steps) = march(local, ctx, view, ctx.eye, d, 1e-3, 0.0, view.max_dist, 3000);
    if !hit {
        let c = sky(d, ctx.sun);
        // horizon haze
        let haze = (1.0 - d.z.max(0.0) * 6.0).clamp(0.0, 1.0);
        return (c.lerp(fog_color(d, ctx.sun), 0.6 * haze * haze), steps);
    }
    let p = ctx.eye + d * t;
    let lvl = lvl_at(t, ctx, view);
    let fp = lvl.exp2();
    let pr = local.probe(p.x, p.y, lvl);
    let p_flat = v3(p.x, p.y, p.z + drop(ctx, view, p));
    let is_water = pr.h <= view.water + 1e-9 || (pr.lake > 0.5 && pr.h < view.water + 0.5 * fp);
    let sun = ctx.sun;
    let mut col;
    if is_water {
        let n = v3(0.0, 0.0, 1.0);
        let cos_i = (-d.dot(n)).clamp(0.0, 1.0);
        let fres = 0.02 + 0.98 * (1.0 - cos_i).powi(5);
        let r = d - n * (2.0 * d.dot(n));
        let mut refl = sky(r, sun);
        if view.reflections {
            let o = p + n * (1e-3 * fp);
            let (tr, hr, k) = march(local, ctx, view, o, r, 2.0 * fp, t, view.max_dist * 0.5, 600);
            steps += k;
            if hr {
                let q = o + r * tr;
                let l2 = lvl_at(t + tr, ctx, view) + 1.0;
                let pq = local.probe(q.x, q.y, l2);
                let nq = normal_at(local, q.x, q.y, l2);
                let a = albedo(view, pq.de, pq.mu, pq.h, nq, l2.exp2());
                let dif = nq.dot(sun).max(0.0);
                refl = a.mulv(SUN_COL * dif + SKY_ZENITH * (0.4 + 0.3 * nq.z));
                let fd = 1.0 - (-(tr) / view.fog).exp();
                refl = refl.lerp(fog_color(r, sun), fd);
            }
        }
        let deep = v3(0.01, 0.06, 0.07);
        col = deep.lerp(refl, fres.max(0.25));
        // sun glitter
        let hv = (sun - d).norm();
        col = col + SUN_COL * (2.0 * hv.z.max(0.0).powf(400.0));
        if view.shadows {
            let s = soft_shadow(local, ctx, view, p + n * (0.5 * fp), t);
            col = col * (0.55 + 0.45 * s);
        }
    } else {
        let n = normal_at(local, p.x, p.y, lvl);
        let a = albedo(view, pr.de, pr.mu, pr.h, n, fp);
        let dif = n.dot(sun).max(0.0);
        let shadow = if view.shadows && dif > 0.0 { soft_shadow(local, ctx, view, p + n * (0.5 * fp), t) } else { 1.0 };
        let ao = ambient_occlusion(local, p_flat, n, lvl);
        let sky_l = (0.5 + 0.5 * n.z) * ao;
        // bounce from the ground opposite the sun
        let back = v3(-sun.x, -sun.y, 0.0).norm();
        let bounce = (n.dot(back)).max(0.0) * 0.12 * ao;
        col = a.mulv(SUN_COL * (dif * shadow) + SKY_ZENITH.lerp(SKY_HORIZON, 0.4) * (0.42 * sky_l) + v3(0.6, 0.45, 0.3) * bounce);
    }
    // aerial perspective: exponential fog with distance plus height falloff
    let fd = 1.0 - (-t / view.fog).exp();
    col = col.lerp(fog_color(d, sun), fd.clamp(0.0, 1.0));
    (col, steps)
}

fn soft_shadow(local: &mut Local, ctx: &Ctx, view: &View, o: V3, t_cam: f64) -> f64 {
    let mut res: f64 = 1.0;
    let mut t = 0.01 * (t_cam * ctx.pix_angle).max(1e-6) + 1e-4;
    let tmax = (t_cam * 2.0).max(20.0).min(view.max_dist * 0.25);
    for _ in 0..160 {
        let p = o + ctx.sun * t;
        let lvl = lvl_at(t_cam + t, ctx, view) + 1.0;
        let hh = local.height(p.x, p.y, lvl).max(view.water) - drop(ctx, view, p);
        let gap = p.z - hh;
        if gap < 0.0 {
            return 0.0;
        }
        res = res.min(10.0 * gap / t);
        if res < 0.01 {
            return 0.0;
        }
        t += (0.5 * gap).max(0.02 * t).max(1e-5);
        if t > tmax {
            break;
        }
    }
    res.clamp(0.0, 1.0)
}

fn ambient_occlusion(local: &mut Local, p: V3, n: V3, lvl: f64) -> f64 {
    // horizon-style occlusion: heights at three radii in six directions
    let h0 = p.z;
    let base = lvl.exp2();
    let mut occ = 0.0;
    let mut wsum = 0.0;
    for (ri, r) in [3.0, 10.0, 30.0].iter().enumerate() {
        let rr = base * r;
        let l = lvl + (ri as f64) * 1.5;
        for k in 0..6 {
            let a = k as f64 * std::f64::consts::TAU / 6.0 + ri as f64 * 0.5;
            let hh = local.height(p.x + rr * a.cos(), p.y + rr * a.sin(), l);
            let rise = ((hh - h0) / rr).max(0.0);
            occ += rise / (1.0 + rise); // ~ sin of horizon angle
            wsum += 1.0;
        }
    }
    let _ = n;
    (1.0 - 1.2 * occ / wsum).clamp(0.15, 1.0)
}
