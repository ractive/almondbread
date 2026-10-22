//! Buddhabrot and Nebulabrot as a look (milestone BB, kb: plans/buddhabrot,
//! sources/superliminal-buddhabrot): instead of colouring each `c` by its escape time, the image
//! is a histogram of where the orbits of escaping `c` go. Nebulabrot maps three iteration limits
//! to red, green and blue (Melinda Green's 5000 / 500 / 50).
//!
//! This module is the platform-free half: the mode, its limits, the sampling region and rules
//! (closed-form cardioid and bulb rejection, the periodicity test), the bin mapping of a view, the
//! tone curve, and the CPU oracle the GPU histogram is tested against (uniform sampling in f64,
//! [`cpu_histogram`]). The GPU sampler (uniform and Metropolis–Hastings) is
//! `almondbread-render/src/buddha.rs` with `shaders/buddhabrot.metal`.

use crate::viewport::Viewport;

/// Which orbit-density look is on (none: the escape-time colouring).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BuddhaMode {
    #[default]
    Off,
    /// One histogram, coloured through the palette.
    Buddhabrot,
    /// Three histograms (limits [`NEBULABROT_LIMITS`]) as red, green and blue.
    Nebulabrot,
}

/// Escape limit of the Buddhabrot (one channel).
pub const BUDDHABROT_LIMIT: u32 = 2000;
/// Red, green and blue limits of the Nebulabrot (src: sources/superliminal-buddhabrot).
pub const NEBULABROT_LIMITS: [u32; 3] = [5000, 500, 50];

/// Uniform samples are drawn from this square (every `c` outside the disc |c| <= 2 escapes at
/// once, so its only orbit point is `c` itself, outside any useful view): lower-left, size.
pub const REGION_MIN: [f64; 2] = [-2.0, -2.0];
pub const REGION_SIZE: [f64; 2] = [4.0, 4.0];
/// Escape at |z|^2 > 4 (the classic Buddhabrot radius: every escaping orbit point but the last is
/// inside the disc of radius 2).
pub const BAILOUT2: f64 = 4.0;
/// An orbit whose point at an iteration 2^k comes back within this (squared) distance is periodic
/// (interior) and stops early (Brent's test; float32 orbits on an attracting cycle land within a
/// few ulps).
pub const PERIOD_EPS2: f64 = 1e-12;

/// The finest bin the float32 sampler resolves: an orbit point near |z| = 1 is quantised to
/// 2^-24 (6e-8); bins of at least ~4 ulps keep that quantisation out of the image. Deeper views
/// fall back to the escape-time colouring (kb: plans/buddhabrot "Non-goals": no deep zoom).
pub const MIN_PIXEL: f64 = 2.5e-7;

impl BuddhaMode {
    pub const ALL: [BuddhaMode; 3] = [Self::Off, Self::Buddhabrot, Self::Nebulabrot];

    /// Stable name (CLI, FFI, persisted settings).
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Buddhabrot => "buddhabrot",
            Self::Nebulabrot => "nebulabrot",
        }
    }

    pub fn parse(v: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == v)
    }

    pub fn is_on(self) -> bool {
        self != Self::Off
    }

    /// Histogram channels (0 when off).
    pub fn channels(self) -> usize {
        match self {
            Self::Off => 0,
            Self::Buddhabrot => 1,
            Self::Nebulabrot => 3,
        }
    }

    /// Escape limit per channel (unused channels repeat the first).
    pub fn limits(self) -> [u32; 3] {
        match self {
            Self::Off | Self::Buddhabrot => [BUDDHABROT_LIMIT; 3],
            Self::Nebulabrot => NEBULABROT_LIMITS,
        }
    }

    /// The largest limit: the sampler iterates every `c` up to it.
    pub fn max_limit(self) -> u32 {
        let l = self.limits();
        l[0].max(l[1]).max(l[2])
    }
}

/// Inside the main cardioid or the period-2 bulb (closed form): never escapes, so the sampler
/// rejects it without iterating (most of the set's area).
pub fn in_main_components(x: f64, y: f64) -> bool {
    let xq = x - 0.25;
    let q = xq * xq + y * y;
    if q * (q + xq) <= 0.25 * y * y {
        return true;
    }
    let x1 = x + 1.0;
    x1 * x1 + y * y <= 0.0625
}

/// The escape iteration of `c` (|z_n|^2 > [`BAILOUT2`] at n <= `limit`), or None (did not escape,
/// lies in the cardioid or bulb, or its orbit closed a cycle). `its` counts the iterations.
pub fn escape_time(x: f64, y: f64, limit: u32, its: &mut u64) -> Option<u32> {
    if in_main_components(x, y) {
        return None;
    }
    let (mut zx, mut zy) = (0.0f64, 0.0f64);
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    let mut check = 8u32;
    for n in 1..=limit {
        let t = zx * zx - zy * zy + x;
        zy = 2.0 * zx * zy + y;
        zx = t;
        if zx * zx + zy * zy > BAILOUT2 {
            *its += u64::from(n);
            return Some(n);
        }
        let (dx, dy) = (zx - sx, zy - sy);
        if dx * dx + dy * dy < PERIOD_EPS2 {
            *its += u64::from(n);
            return None;
        }
        if n == check {
            (sx, sy) = (zx, zy);
            check *= 2;
        }
    }
    *its += u64::from(limit);
    None
}

/// How a view maps orbit points to histogram bins: bin (i, j) covers view pixels [i, i+1) x
/// [j, j+1), row 0 at the top, rotation as [`Viewport::offset_of`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BinMap {
    pub center: [f64; 2],
    /// `(cos, sin)` of the view angle.
    pub rot: [f64; 2],
    /// Complex units per bin.
    pub pixel: f64,
    pub width: usize,
    pub height: usize,
}

impl BinMap {
    /// The bins of `viewport` drawn at `width` x `height` pixels of `points_per_pixel` view points.
    pub fn new(viewport: &Viewport, width: usize, height: usize, points_per_pixel: f64) -> Self {
        let (c, s) = viewport.rotation();
        Self {
            center: [viewport.center.re(), viewport.center.im()],
            rot: [c, s],
            pixel: viewport.scale.to_f64() * points_per_pixel,
            width,
            height,
        }
    }

    /// The bin of orbit point `z`, if in the view.
    pub fn bin(&self, zx: f64, zy: f64) -> Option<usize> {
        let (dx, dy) = (zx - self.center[0], zy - self.center[1]);
        let [c, s] = self.rot;
        let (x, y) = ((dx * c + dy * s) / self.pixel, (-dx * s + dy * c) / self.pixel);
        let px = x + self.width as f64 / 2.0;
        let py = self.height as f64 / 2.0 - y;
        if px >= 0.0 && py >= 0.0 && px < self.width as f64 && py < self.height as f64 {
            Some(py as usize * self.width + px as usize)
        } else {
            None
        }
    }

    /// The view is shallow enough for the float32 sampler ([`MIN_PIXEL`]).
    pub fn supported(&self) -> bool {
        self.pixel >= MIN_PIXEL && self.pixel.is_finite()
    }

    /// Share of the uniform sampling region the view covers (1 or more: the whole set).
    pub fn region_share(&self) -> f64 {
        let area = self.pixel * self.pixel * (self.width * self.height) as f64;
        area / (REGION_SIZE[0] * REGION_SIZE[1])
    }
}

/// A histogram: `channels` planes of `width * height` counts, and what made it.
#[derive(Clone, Debug, PartialEq)]
pub struct Histogram {
    pub counts: Vec<u32>,
    pub channels: usize,
    pub width: usize,
    pub height: usize,
    /// Uniform-equivalent samples of `c` behind it.
    pub samples: f64,
}

impl Histogram {
    pub fn plane(&self, ch: usize) -> &[u32] {
        let n = self.width * self.height;
        &self.counts[ch * n..(ch + 1) * n]
    }

    /// Hits of channel `ch` per bin per uniform sample, averaged over the bins: the tone
    /// curve's reference level.
    pub fn mean_density(&self, ch: usize) -> f64 {
        let sum: u64 = self.plane(ch).iter().map(|&v| u64::from(v)).sum();
        sum as f64 / (self.samples.max(1.0) * (self.width * self.height) as f64)
    }
}

/// splitmix64: the CPU oracle's random stream (independent of the GPU's; the test compares
/// distributions, not samples).
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit(state: &mut u64) -> f64 {
    (splitmix(state) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Deposits the orbit of `c` (escaping at `n`) into `counts`; returns the in-view hits.
fn deposit(map: &BinMap, mode: BuddhaMode, x: f64, y: f64, n: u32, counts: &mut [u32]) -> u64 {
    let limits = mode.limits();
    let channels = mode.channels();
    let plane = map.width * map.height;
    let (mut zx, mut zy) = (0.0f64, 0.0f64);
    let mut hits = 0;
    for _ in 0..n {
        let t = zx * zx - zy * zy + x;
        zy = 2.0 * zx * zy + y;
        zx = t;
        if let Some(b) = map.bin(zx, zy) {
            hits += 1;
            for (ch, &limit) in limits.iter().enumerate().take(channels) {
                if n <= limit {
                    counts[ch * plane + b] += 1;
                }
            }
        }
    }
    hits
}

/// The CPU oracle: `samples` uniform samples of `c` over the region (f64 orbits), on all cores,
/// deterministic for a `seed`. Slow (tests at small sizes and the GPU-vs-CPU check).
pub fn cpu_histogram(map: &BinMap, mode: BuddhaMode, samples: u64, seed: u64) -> Histogram {
    use rayon::prelude::*;
    let channels = mode.channels().max(1);
    let plane = map.width * map.height;
    const BLOCK: u64 = 1 << 14;
    let blocks = samples.div_ceil(BLOCK);
    let limit = mode.max_limit();
    let counts = (0..blocks)
        .into_par_iter()
        .fold(
            || vec![0u32; channels * plane],
            |mut acc, b| {
                let mut rng = seed ^ b.wrapping_mul(0xD1B5_4A32_D192_ED03);
                let n = BLOCK.min(samples - b * BLOCK);
                let mut its = 0;
                for _ in 0..n {
                    let x = REGION_MIN[0] + REGION_SIZE[0] * unit(&mut rng);
                    let y = REGION_MIN[1] + REGION_SIZE[1] * unit(&mut rng);
                    if let Some(e) = escape_time(x, y, limit, &mut its) {
                        deposit(map, mode, x, y, e, &mut acc);
                    }
                }
                acc
            },
        )
        .reduce(
            || vec![0u32; channels * plane],
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(b) {
                    *x += y;
                }
                a
            },
        );
    Histogram {
        counts,
        channels,
        width: map.width,
        height: map.height,
        samples: samples as f64,
    }
}

/// The tone curve (the GPU's `buddhaTone` mirrors it): a bin's density `v` over the view's mean
/// density `mean`, from black at [`TONE_BLACK`] means to white at [`TONE_WHITE`] means, through a
/// power curve. Mean-relative, so it needs no readback of the histogram (the GPU sums the hits
/// itself) and works the same for the live accumulation; measured at the whole set, 4 means is
/// about the 99.9th percentile of the bins and half a mean the haze of short orbits.
pub fn tone(mode: BuddhaMode, v: f64, mean: f64) -> f64 {
    if mean <= 0.0 || v <= 0.0 {
        return 0.0;
    }
    let b = mode.tone_black() / TONE_WHITE;
    ((v / (mean * TONE_WHITE) - b) / (1.0 - b))
        .clamp(0.0, 1.0)
        .powf(TONE_GAMMA)
}

/// The densities (in means of the view) that show white and black, and the curve's exponent.
/// The Nebulabrot's black point is higher: its 50-step channel is mostly haze (measured at the
/// whole set: its median is 0.9 means, the 500- and 5000-step channels' 0.7 and 0.55).
pub const TONE_WHITE: f64 = 4.0;
pub const TONE_BLACK: f64 = 0.5;
pub const TONE_BLACK_NEBULA: f64 = 0.8;
pub const TONE_GAMMA: f64 = 1.0;
/// The GPU measures the points per view (`almondbread-render/src/buddha.rs`, `Levels`): white at
/// this percentile of the bins, black at the look's ([`BuddhaMode::tone_black_q`]), and white at
/// least [`TONE_MIN_RANGE`] means above black (a flat view is not stretched into noise). At the
/// whole set they land near the fixed points above.
pub const TONE_WHITE_Q: f64 = 0.998;
pub const TONE_MIN_RANGE: f64 = 1.0;

impl BuddhaMode {
    /// The tone curve's black point (in view means) before a measurement.
    pub fn tone_black(self) -> f64 {
        if self == Self::Nebulabrot {
            TONE_BLACK_NEBULA
        } else {
            TONE_BLACK
        }
    }

    /// The percentile of the bins the measured black point sits at.
    pub fn tone_black_q(self) -> f64 {
        if self == Self::Nebulabrot { 0.35 } else { 0.15 }
    }
}

/// Two-sample comparison of histograms `a` and `b` (same shape) after merging `k` x `k` bins:
/// (chi-square per degree of freedom, relative L1 distance of the normalised histograms). With
/// independent Poisson counts of one distribution the first is about 1; orbits deposit
/// correlated runs of hits, so the GPU-vs-CPU test compares it with the CPU-vs-CPU value of two
/// seeds (kb: plans/buddhabrot "Verification").
pub fn compare(a: &Histogram, b: &Histogram, ch: usize, k: usize) -> (f64, f64) {
    let (w, h) = (a.width / k, a.height / k);
    let merge = |hst: &Histogram| {
        let mut out = vec![0f64; w * h];
        let p = hst.plane(ch);
        for y in 0..h * k {
            for x in 0..w * k {
                out[(y / k) * w + x / k] += f64::from(p[y * hst.width + x]);
            }
        }
        out
    };
    let (ma, mb) = (merge(a), merge(b));
    let (sa, sb): (f64, f64) = (ma.iter().sum(), mb.iter().sum());
    if sa <= 0.0 || sb <= 0.0 {
        return (f64::INFINITY, 2.0);
    }
    let (fa, fb) = ((sb / sa).sqrt(), (sa / sb).sqrt());
    let mut chi = 0.0;
    let mut dof = 0usize;
    let mut l1 = 0.0;
    for (x, y) in ma.iter().zip(&mb) {
        l1 += (x / sa - y / sb).abs();
        if x + y > 0.0 {
            chi += (fa * x - fb * y).powi(2) / (x + y);
            dof += 1;
        }
    }
    (chi / (dof.max(2) - 1) as f64, l1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_and_rejection() {
        for m in BuddhaMode::ALL {
            assert_eq!(BuddhaMode::parse(m.name()), Some(m));
        }
        assert_eq!(BuddhaMode::Nebulabrot.max_limit(), 5000);
        assert!(in_main_components(0.0, 0.0));
        assert!(in_main_components(-1.0, 0.1));
        assert!(!in_main_components(0.3, 0.0));
        assert!(!in_main_components(-0.75, 0.1));
        let mut its = 0;
        assert_eq!(escape_time(1.0, 0.0, 100, &mut its), Some(3));
        // In a period-3 bulb: the cycle test stops it long before the limit.
        assert_eq!(escape_time(-0.1226, 0.7449, 100_000, &mut its), None);
        assert!(its < 20_000, "{its}");
    }

    #[test]
    fn bins_follow_the_view() {
        let v = Viewport::new(-0.5, 0.0, 0.01);
        let m = BinMap::new(&v, 400, 300, 1.0);
        assert_eq!(m.bin(-0.5, 0.0), Some(150 * 400 + 200));
        // Up in the view is up in c (row 0 at the top).
        assert_eq!(m.bin(-0.5, 1.4).unwrap() / 400, 10);
        assert_eq!(m.bin(5.0, 0.0), None);
        assert!(m.supported());
        let deep = BinMap::new(&Viewport::new(-0.5, 0.0, 1e-9), 400, 300, 1.0);
        assert!(!deep.supported());
    }

    #[test]
    fn cpu_histogram_is_symmetric_and_deterministic() {
        // The whole set: the Buddhabrot is mirror-symmetric about the real axis.
        let v = Viewport::new(-0.5, 0.0, 3.0 / 64.0);
        let m = BinMap::new(&v, 64, 64, 1.0);
        let a = cpu_histogram(&m, BuddhaMode::Buddhabrot, 200_000, 1);
        assert_eq!(a, cpu_histogram(&m, BuddhaMode::Buddhabrot, 200_000, 1));
        let flipped = Histogram {
            counts: (0..64 * 64).map(|i| a.counts[(63 - i / 64) * 64 + i % 64]).collect(),
            ..a.clone()
        };
        let (chi, l1) = compare(&a, &flipped, 0, 4);
        let b = cpu_histogram(&m, BuddhaMode::Buddhabrot, 200_000, 2);
        let (chi_seeds, l1_seeds) = compare(&a, &b, 0, 4);
        assert!(l1 < 2.0 * l1_seeds + 0.02, "mirror {l1} vs seeds {l1_seeds}");
        assert!(chi < 3.0 * chi_seeds + 1.0, "mirror {chi} vs seeds {chi_seeds}");
        assert!(a.mean_density(0) > 0.0);
        // Nebulabrot channels nest: red (5000) counts every orbit blue (50) counts.
        let n = cpu_histogram(&m, BuddhaMode::Nebulabrot, 100_000, 3);
        assert!(n.plane(0).iter().zip(n.plane(2)).all(|(r, b)| r >= b));
    }

    #[test]
    fn tone_curve() {
        let m = BuddhaMode::Buddhabrot;
        assert_eq!(tone(m, 0.0, 1.0), 0.0);
        assert_eq!(tone(m, 1e9, 1.0), 1.0);
        assert_eq!(tone(m, TONE_BLACK, 1.0), 0.0);
        assert_eq!(tone(BuddhaMode::Nebulabrot, TONE_BLACK_NEBULA, 1.0), 0.0);
        let mid = 0.5 * (TONE_BLACK + TONE_WHITE);
        assert!((tone(m, mid, 1.0) - 0.5f64.powf(TONE_GAMMA)).abs() < 1e-12);
    }
}
