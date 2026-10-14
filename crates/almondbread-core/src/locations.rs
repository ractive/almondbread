//! The fixed benchmark set (kb: synthesis/benchmarks). Keep entries stable: the kb tables
//! compare runs across milestones by name.

use crate::viewport::Viewport;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BenchmarkLocation {
    pub name: &'static str,
    pub center_re: f64,
    pub center_im: f64,
    /// Complex width of the view; scale per pixel = width / image width.
    pub width: f64,
    pub max_iterations: usize,
    pub note: &'static str,
}

impl BenchmarkLocation {
    pub fn scale_for_width(&self, pixels: usize) -> f64 {
        self.width / pixels as f64
    }

    pub fn viewport_for_width(&self, pixels: usize) -> Viewport {
        Viewport::new(self.center_re, self.center_im, self.scale_for_width(pixels))
    }

    pub fn named(name: &str) -> Option<&'static BenchmarkLocation> {
        BENCHMARK_LOCATIONS.iter().find(|l| l.name == name)
    }
}

const fn loc(
    name: &'static str,
    center_re: f64,
    center_im: f64,
    width: f64,
    max_iterations: usize,
    note: &'static str,
) -> BenchmarkLocation {
    BenchmarkLocation {
        name,
        center_re,
        center_im,
        width,
        max_iterations,
        note,
    }
}

// Literals exactly as in the Swift BenchmarkLocations (some carry more digits than f64 holds;
// they parse to the same doubles).
#[allow(clippy::excessive_precision)]
pub const BENCHMARK_LOCATIONS: &[BenchmarkLocation] = &[
    loc("home", -0.75, 0.0, 3.9, 1_000, "whole set, cheap baseline"),
    loc(
        "home-100k",
        -0.75,
        0.0,
        3.9,
        100_000,
        "about a third interior pixels: interior detection case",
    ),
    loc(
        "seahorse-1e5",
        -0.743643887037151,
        0.13182590420533,
        1.3e-4,
        2_000,
        "seahorse valley spirals, zoom ~1e5",
    ),
    loc(
        "elephant",
        0.2925,
        0.0149,
        0.025,
        5_000,
        "elephant valley, near the cardioid cusp",
    ),
    loc(
        "minibrot-p4",
        -1.9407998065294847,
        0.0,
        0.0045,
        50_000,
        "period-4 minibrot centred, interior-heavy",
    ),
    loc(
        "deep-5e-14",
        -0.7436438870371587,
        0.13182590420531198,
        1.728e-10,
        6_000,
        "scale 5e-14/px at 3456 wide, near Double limit",
    ),
    loc(
        "feigenbaum",
        -1.4011551890920506,
        0.0,
        2e-7,
        200_000,
        "high-iteration filaments around the Feigenbaum point",
    ),
    // Parabolic regressions (kb: techniques/interior-detection, Pitfalls): the verifier must
    // see no GPU-interior/CPU-exterior pixels here.
    loc(
        "neck-2e5",
        -0.7500016386523336,
        -0.0021073424656852589,
        2.1666e-5,
        2_053,
        "the -3/4 neck at 2.2e5 (user report: 21 % false interior)",
    ),
    loc(
        "neck-7e6",
        -0.75000661155604931,
        -0.0021104008406531076,
        6.7986e-7,
        2_907,
        "the -3/4 neck at 7e6, mostly unresolved at the depth limit",
    ),
    loc(
        "cusp-1e5",
        0.25005,
        0.0,
        1e-4,
        3_000,
        "the cardioid cusp c = 1/4 (1.7 % false interior before)",
    ),
    // M4 (kb: synthesis/benchmarks M4): boundary-dense view the user found slow (every pixel
    // near the boundary, thousands of iterations each; 2x seahorse's iterations at the same
    // depth). BLA cannot help at this depth; a regression check for the auto decision.
    loc(
        "dense-5e-14",
        -1.4080321340304542,
        0.13680236432900417,
        7.384e-11,
        5_791,
        "boundary-dense filaments near a period-7 cascade, 4.6e-14/px at 1600 wide",
    ),
    loc(
        "root-third",
        -0.1260734027823272,
        0.6495203781908359,
        2e-5,
        3_000,
        "valley at the root of the 1/3 bulb",
    ),
];
