//! A cheap visual descriptor of a probe, for the journey's novelty scoring (kb:
//! decisions/controls "Journey", variety).
//!
//! The planner probes every candidate framing at 40 × 40 texels anyway; the descriptor reads
//! that field (plus two short orbits for the pitch) and says what kind of picture it is, so the
//! journey can penalise a landing that looks like one of the last few and reject a
//! near-duplicate. Components (all about 0 … 1) and what they turned out to be worth, measured
//! on 30 landings of journey #4711 sorted by eye into four families (dendrite carpets, bulbs,
//! thin stars and medallions, spirals; Fisher separation of the family means):
//!
//! - **pitch** (from the maths, [`super::pitch`]): the turn per e-fold of the repelling cycle
//!   the view's orbits shadow. The one measure that tells spirals from dendrite carpets
//!   (separation 1.2 – 1.3); it is a property of the region, so it changes at cross-overs;
//! - **g10 / g50 / g90**: quantiles of `log10 |∇ν|` (ν = n + s, the smooth count, per texel):
//!   the distance spectrum, how much of the view is close to the set; **dim**: the
//!   box-counting dimension of the structure mask (|∇ν| ≥ 2). Thin stars on a smooth ground
//!   against everything denser (separation 2 – 3.4);
//! - **interior** (square root of the share: a small minibrot already counts), **filament**
//!   (exterior texels escaping 8+ iterations after the fastest), **calm** (5 × 5 cells without
//!   a band crossing), **fine** (crossings per width / 40): bulbs and minibrots in view;
//! - weak on 40 texels, kept with small weights: **rings** / **swirl** (`cos` / `|sin|` of
//!   2(φ − ψ), φ the gradient angle of the blurred `ln(1 + ν)`, ψ the direction from the
//!   centre), **spirals** (the same about local maxima), **aniso** (doubled-angle resultant of
//!   the gradients). Seahorse spirals are 3 – 4 texels wide in a 40-texel probe: no image
//!   measure resolves them, hence the pitch;
//! - **colour**: the mean linear-RGB colour and the luminance spread of the default palette
//!   over the escape counts (`frac(density · n)`); nearly random between views of one shape,
//!   so weighted low.
//!
//! The distance is a weighted Euclidean one: within a family 0.24 / 0.45 / 0.77 (10th / 50th
//! / 90th percentile), across families 0.45 / 0.73 / 1.13.

use crate::oracle::IterationField;

/// Number of components.
pub const DIM: usize = 17;
/// Component weights of the distance (see the module docs: the separating measures count most).
const WEIGHTS: [f64; DIM] = [
    1.2, 0.8, 1.0, 0.6, 0.5, 0.6, 0.3, 0.6, 0.15, 0.15, 0.15, 0.3, 2.0, 0.8, 1.5, 2.5, 3.0,
];

/// The descriptor; see the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    pub v: [f64; DIM],
}

/// The default palette as a table (linear RGB) and its density, for the colour components.
fn palette() -> &'static (Vec<[f32; 4]>, f64) {
    static P: std::sync::OnceLock<(Vec<[f32; 4]>, f64)> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let s = crate::settings::RenderSettings::default();
        (s.palette_table(64), s.color_density)
    })
}

impl Look {
    pub const NAMES: [&'static str; DIM] = [
        "interior", "filament", "calm", "fine", "rings", "swirl", "spirals", "aniso", "r", "g", "b", "lspread", "dim",
        "g10", "g50", "g90", "pitch",
    ];

    pub fn interior(&self) -> f64 {
        self.v[0]
    }

    /// Spiral pitch (see the module docs).
    pub fn pitch(&self) -> f64 {
        self.v[16]
    }

    pub fn of(f: &IterationField) -> Self {
        let (w, h) = (f.width, f.height);
        let mut v = [0.0; DIM];
        if w < 8 || h < 8 {
            return Self { v };
        }
        let n = |x: usize, y: usize| f.n[y * w + x];
        let ext = |x: usize, y: usize| n(x, y) >= 0.0;
        let total = (w * h) as f64;
        let interior = f.n.iter().filter(|&&n| n < 0.0).count() as f64 / total;
        let n_min = f.n.iter().copied().filter(|&n| n >= 0.0).fold(f32::INFINITY, f32::min);
        let n_ext = f.n.iter().filter(|&&n| n >= 0.0).count();
        let near = f.n.iter().filter(|&&n| n >= 0.0 && n >= n_min + 8.0).count();
        // Band crossings and calm cells (as the probe statistics).
        let cls = |x: usize, y: usize| {
            let a = n(x, y);
            if a < 0.0 { -1 } else { a.floor() as i64 }
        };
        let cell = 5;
        let (cx, cy) = (w.div_ceil(cell), h.div_ceil(cell));
        let mut cell_edges = vec![0usize; cx * cy];
        let (mut pairs, mut edges) = (0usize, 0usize);
        for y in 0..h {
            for x in 0..w {
                for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                    if nx < w && ny < h {
                        pairs += 1;
                        if cls(x, y) != cls(nx, ny) {
                            edges += 1;
                            cell_edges[(y / cell) * cx + x / cell] += 1;
                        }
                    }
                }
            }
        }
        let calm = cell_edges.iter().filter(|&&e| e == 0).count() as f64 / cell_edges.len() as f64;
        let fine = (edges as f64 / pairs.max(1) as f64).min(1.0);

        // Unit gradients of a blurred ln(1 + ν), ν = n + s the smooth count (n alone is
        // integral: terraces, no direction): the large-scale flow of the colour bands. Interior
        // texels take the largest exterior value (the set is "high"); the blur is Gaussian with
        // sigma = width / 40.
        let smooth = |i: usize| f.n[i] as f64 + f.s.get(i).copied().unwrap_or(0.0) as f64;
        let lmax = (0..w * h).filter(|&i| f.n[i] >= 0.0).map(smooth).fold(0.0f64, f64::max);
        let mut field: Vec<f64> = (0..w * h)
            .map(|i| (1.0 + if f.n[i] < 0.0 { lmax } else { smooth(i).max(0.0) }).ln())
            .collect();
        blur(&mut field, w, h, (w as f64 / 40.0).max(1.0));
        let val = |x: usize, y: usize| field[y * w + x];
        let mut u = vec![None::<(f64, f64, f64)>; w * h];
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let gx = 0.5 * (val(x + 1, y) - val(x - 1, y));
                // Image rows go down; the plane's y goes up.
                let gy = -0.5 * (val(x, y + 1) - val(x, y - 1));
                let m = gx.hypot(gy);
                if m > 1e-9 {
                    u[y * w + x] = Some((gx / m, gy / m, m));
                }
            }
        }
        let (mut rc, mut rs, mut wsum) = (0.0, 0.0, 0.0);
        let (mut a2c, mut a2s) = (0.0, 0.0);
        let (ox, oy) = ((w as f64 - 1.0) / 2.0, (h as f64 - 1.0) / 2.0);
        for y in 0..h {
            for x in 0..w {
                let Some((ux, uy, m)) = u[y * w + x] else { continue };
                // Weight: the gradient's strength, compressed (filaments dominate otherwise).
                let wt = m.sqrt();
                let (dx, dy) = (x as f64 - ox, -(y as f64 - oy));
                let r = dx.hypot(dy);
                let phi = uy.atan2(ux);
                if r > 2.0 {
                    let d = 2.0 * (phi - dy.atan2(dx));
                    rc += wt * d.cos();
                    rs += wt * d.sin();
                }
                a2c += wt * (2.0 * phi).cos();
                a2s += wt * (2.0 * phi).sin();
                wsum += wt;
            }
        }
        let wsum = wsum.max(1e-12);
        let rings = 0.5 * (rc / wsum + 1.0);
        let swirl = (rs / wsum).abs();
        let aniso = a2c.hypot(a2s) / wsum;
        let spirals = local_spirals(&u, &field, w, h);
        // Colour: the default palette over the exterior texels (interior is black).
        let (table, density) = palette();
        let (mut cr, mut cg, mut cb) = (0.0, 0.0, 0.0);
        let mut lum: Vec<f64> = Vec::with_capacity(w * h);
        for &k in &f.n {
            let c = if k < 0.0 {
                [0.0, 0.0, 0.0]
            } else {
                let t = (k as f64 * density).rem_euclid(1.0);
                let e = table[((t * table.len() as f64) as usize).min(table.len() - 1)];
                [e[0] as f64, e[1] as f64, e[2] as f64]
            };
            cr += c[0];
            cg += c[1];
            cb += c[2];
            lum.push(0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]);
        }
        let lm = lum.iter().sum::<f64>() / total;
        let lsd = (lum.iter().map(|l| (l - lm).powi(2)).sum::<f64>() / total).sqrt();
        // Distance spectrum: quantiles of log10 |∇ν| (ν = n + s, per texel; large next to the
        // set, small in the smooth gradient between structures), and the box-counting
        // dimension of the structure mask (|∇ν| ≥ 2: the escape count changes by two or more
        // per texel): thin dendrites on a smooth ground about 1.2 … 1.5, packed spirals and
        // decorations towards 2.
        let nu = |x: usize, y: usize| smooth(y * w + x);
        let mut grads: Vec<f64> = Vec::with_capacity(w * h);
        let mut mask = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                if !ext(x, y) {
                    continue;
                }
                let mut g: f64 = 0.0;
                for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                    if nx < w && ny < h && ext(nx, ny) {
                        g = g.max((nu(nx, ny) - nu(x, y)).abs());
                    }
                }
                if x > 0 && ext(x - 1, y) {
                    g = g.max((nu(x - 1, y) - nu(x, y)).abs());
                }
                if y > 0 && ext(x, y - 1) {
                    g = g.max((nu(x, y - 1) - nu(x, y)).abs());
                }
                grads.push((g.max(1e-3)).log10());
                mask[y * w + x] = g >= 2.0;
            }
        }
        grads.sort_by(f64::total_cmp);
        let q = |t: f64| {
            if grads.is_empty() {
                0.0
            } else {
                grads[((grads.len() - 1) as f64 * t) as usize]
            }
        };
        // log10 |∇ν| from -3 (flat) to 3: to 0 … 1.
        let gq = |t: f64| ((q(t) + 2.0) / 4.0).clamp(0.0, 1.0);
        let mut boxes: Vec<(f64, f64)> = Vec::new();
        for b in [1usize, 2, 4, 8] {
            let (bw, bh) = (w.div_ceil(b), h.div_ceil(b));
            let mut occ = vec![false; bw * bh];
            for y in 0..h {
                for x in 0..w {
                    if mask[y * w + x] {
                        occ[(y / b) * bw + x / b] = true;
                    }
                }
            }
            let c = occ.iter().filter(|&&o| o).count();
            if c > 0 {
                boxes.push(((1.0 / b as f64).ln(), (c as f64).ln()));
            }
        }
        let dim = if boxes.len() >= 3 {
            let m = boxes.len() as f64;
            let (sx, sy) = boxes.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
            let (mx, my) = (sx / m, sy / m);
            let num: f64 = boxes.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
            let den: f64 = boxes.iter().map(|p| (p.0 - mx).powi(2)).sum();
            if den > 0.0 { num / den } else { 0.0 }
        } else {
            0.0
        };
        v[12] = (dim - 1.0).clamp(0.0, 1.0);
        v[13] = gq(0.1);
        v[14] = gq(0.5);
        v[15] = gq(0.9);
        v[0] = interior.sqrt();
        v[1] = near as f64 / n_ext.max(1) as f64;
        v[2] = calm;
        v[3] = fine;
        v[4] = rings;
        v[5] = (swirl * 2.0).min(1.0);
        v[6] = spirals;
        v[7] = aniso;
        v[8] = cr / total;
        v[9] = cg / total;
        v[10] = cb / total;
        v[11] = (lsd * 2.0).min(1.0);
        Self { v }
    }

    /// Weighted Euclidean distance (about 0 … 3; below ~0.25 two probes look alike).
    pub fn distance(&self, o: &Look) -> f64 {
        self.v
            .iter()
            .zip(o.v.iter())
            .zip(WEIGHTS.iter())
            .map(|((a, b), wt)| wt * (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// With the spiral pitch of the view's maths ([`super::pitch`]), 0 … 1.
    pub fn with_pitch(mut self, pitch: f64) -> Self {
        self.v[16] = pitch.clamp(0.0, 1.0);
        self
    }

    pub fn describe(&self) -> String {
        Self::NAMES
            .iter()
            .zip(self.v.iter())
            .map(|(k, v)| format!("{k} {v:.2}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Separable Gaussian blur in place (clamped edges).
fn blur(v: &mut [f64], w: usize, h: usize, sigma: f64) {
    let r = (2.5 * sigma).ceil() as isize;
    let k: Vec<f64> = (-r..=r)
        .map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp())
        .collect();
    let ks: f64 = k.iter().sum();
    let mut tmp = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (j, kv) in k.iter().enumerate() {
                let xx = (x as isize + j as isize - r).clamp(0, w as isize - 1) as usize;
                s += kv * v[y * w + xx];
            }
            tmp[y * w + x] = s / ks;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (j, kv) in k.iter().enumerate() {
                let yy = (y as isize + j as isize - r).clamp(0, h as isize - 1) as usize;
                s += kv * tmp[yy * w + x];
            }
            v[y * w + x] = s / ks;
        }
    }
}

/// Local spirals: around each local maximum of the blurred field `val` (a point the structure
/// winds around: a Misiurewicz point, a minibrot, a star's hub) the weighted mean of
/// `sin 2(φ − ψ)` over a disc of radius w/8 (φ the gradient angle, ψ the direction from that
/// centre): rings and star arms give about 0 (the arms alternate in sign), a spiral a
/// consistent sign. The strongest eight centres, averaged, ×2, at most 1.
fn local_spirals(u: &[Option<(f64, f64, f64)>], val: &[f64], w: usize, h: usize) -> f64 {
    let rad = (w as f64 / 8.0).max(3.0);
    let mut centres: Vec<(f64, usize, usize)> = Vec::new();
    for y in 2..h.saturating_sub(2) {
        for x in 2..w.saturating_sub(2) {
            let c = val[y * w + x];
            let higher = (y - 2..=y + 2)
                .flat_map(|yy| (x - 2..=x + 2).map(move |xx| (xx, yy)))
                .any(|(xx, yy)| (xx, yy) != (x, y) && val[yy * w + xx] >= c);
            if !higher {
                centres.push((c, x, y));
            }
        }
    }
    centres.sort_by(|a, b| b.0.total_cmp(&a.0));
    let r2 = rad.ceil() as isize;
    let mut spirals: Vec<f64> = Vec::new();
    for &(_, cx, cy) in centres.iter().take(8) {
        let (mut ss, mut sw) = (0.0, 0.0);
        for dy in -r2..=r2 {
            for dx in -r2..=r2 {
                let (x, y) = (cx as isize + dx, cy as isize + dy);
                if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
                    continue;
                }
                let r = ((dx * dx + dy * dy) as f64).sqrt();
                if !(1.5..=rad).contains(&r) {
                    continue;
                }
                let Some((ux, uy, m)) = u[y as usize * w + x as usize] else {
                    continue;
                };
                let wt = m.sqrt();
                // Image rows go down; the plane's y goes up.
                let d = 2.0 * (uy.atan2(ux) - (-(dy as f64)).atan2(dx as f64));
                ss += wt * d.sin();
                sw += wt;
            }
        }
        if sw > 0.0 {
            spirals.push((ss / sw).abs());
        }
    }
    if spirals.is_empty() {
        0.0
    } else {
        (spirals.iter().sum::<f64>() / spirals.len() as f64 * 2.0).min(1.0)
    }
}
