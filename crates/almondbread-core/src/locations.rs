//! The fixed benchmark set (kb: synthesis/benchmarks). Keep entries stable: the kb tables
//! compare runs across milestones by name.

use crate::viewport::{Center, Scale, Viewport};

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

/// A deep benchmark location (M3b): centre and width as decimal strings of any precision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeepLocation {
    pub name: &'static str,
    pub re: &'static str,
    pub im: &'static str,
    /// Complex width of the view (decimal, any exponent).
    pub width: &'static str,
    pub max_iterations: usize,
    pub note: &'static str,
}

impl DeepLocation {
    pub fn center(&self) -> Center {
        Center::parse(self.re, self.im).expect("deep location centre")
    }

    pub fn viewport_for_width(&self, pixels: usize) -> Viewport {
        let w = Scale::parse(self.width).expect("deep location width");
        Viewport::with_center(self.center(), w / pixels as f64)
    }
}

/// Either kind of location, by name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Location {
    Shallow(&'static BenchmarkLocation),
    Deep(&'static DeepLocation),
}

impl Location {
    pub fn name(&self) -> &'static str {
        match self {
            Location::Shallow(l) => l.name,
            Location::Deep(l) => l.name,
        }
    }

    pub fn max_iterations(&self) -> usize {
        match self {
            Location::Shallow(l) => l.max_iterations,
            Location::Deep(l) => l.max_iterations,
        }
    }

    pub fn viewport_for_width(&self, pixels: usize) -> Viewport {
        match self {
            Location::Shallow(l) => l.viewport_for_width(pixels),
            Location::Deep(l) => l.viewport_for_width(pixels),
        }
    }
}

/// Every location, shallow first.
pub fn all_locations() -> impl Iterator<Item = Location> {
    BENCHMARK_LOCATIONS
        .iter()
        .map(Location::Shallow)
        .chain(DEEP_LOCATIONS.iter().map(Location::Deep))
}

pub fn location_named(name: &str) -> Option<Location> {
    all_locations().find(|l| l.name() == name)
}

/// Deep locations (M3b, kb: synthesis/benchmarks M3b), generated with `almondbread locate`
/// (ball period detection + Newton nuclei at full precision): minibrots and their surroundings
/// framed a few minibrot sizes wide.
pub const DEEP_LOCATIONS: &[DeepLocation] = &[
    DeepLocation {
        name: "sea-1e20",
        re: "-0.743643887037158704752191506114774",
        im: "0.131825904205311970493132056385139",
        width: "1.6e-17",
        max_iterations: 20000,
        note: "seahorse valley at 1e-20/px (1600 wide): boundary-dense spirals; f64 cannot reach it",
    },
    DeepLocation {
        name: "sea-8007",
        re: "-0.74364388703715870475219150611477977822",
        im: "0.13182590420531197049313205638514067897",
        width: "3.2e-31",
        max_iterations: 40000,
        note: "the period-8007 minibrot near the seahorse point, framed ~35 sizes wide (2e-34/px)",
    },
    DeepLocation {
        name: "needle-1e94",
        re: "-1.99999999999999999999999999999999999999999999898704139472119329940998074630723121589755867176089901",
        im: "0",
        width: "1.6e-91",
        max_iterations: 20000,
        note: "period-79 minibrot on the needle near -2, 1e-94/px",
    },
    DeepLocation {
        name: "needle-1e305",
        re: "-1.999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999006253221176052595584875199939847987724978450887324375677348230768323849761806288997972092760579988865708298929517777848376761353681860186589874625886142463567",
        im: "0",
        width: "1.6e-302",
        max_iterations: 50000,
        note: "period-255 needle minibrot, 1e-305/px: below f64's range",
    },
    DeepLocation {
        name: "needle-1e1004",
        re: "-1.999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999989846851309726740494202988820869208597271795304402524686388559176095296096185085161685844575581996230403487769353601636361711487490318913167938133371670614744658266324252012278999170397592543617054886370410070618964157552099031046101818252302812257492294545794690756299730430294643222538740335023472720010588733901460017429195590405963714879204355073035848349612863210238661848415630564274996700061955886273541375379292062866918583681435545594069167116068451242825082982712765937246785068723657390128667670215",
        im: "0",
        width: "6.4e-1001",
        max_iterations: 100000,
        note: "period-834 needle minibrot, 4e-1004/px",
    },
    DeepLocation {
        name: "dendrite-1e50",
        re: "0",
        im: "1",
        width: "1.6e-47",
        max_iterations: 20000,
        note: "Misiurewicz point c = i at 1e-50/px: self-similar dendrite",
    },
    DeepLocation {
        name: "dendrite-1e300",
        re: "0",
        im: "1",
        width: "1.6e-297",
        max_iterations: 20000,
        note: "c = i at 1e-300/px",
    },
    DeepLocation {
        name: "dendrite-1e1000",
        re: "0",
        im: "1",
        width: "1.6e-997",
        max_iterations: 20000,
        note: "c = i at 1e-1000/px",
    },
    DeepLocation {
        name: "spiral-32412-1e19",
        re: "-0.13415002160674054265664",
        im: "-0.64991906714433853974391",
        width: "6.716864e-16",
        max_iterations: 41536,
        note: "dense spiral around a period-32412 minibrot, 4.2e-19/px (user: the slowest area found)",
    },
];
