//! The cosmic size scale (kb: decisions/controls, "Cosmic size scale"): your window stays
//! the window (30 cm, or its measured width) at every depth and only the whole set grows.
//! Platform-neutral: the table, the readout text and the ruler marks; the render crate draws
//! them (`almondbread_render::cosmic`).
//!
//! Depth coordinate: `d = log10(set width / view width)`, both in the complex plane (the set
//! spans re -2 ... 1/4, width 2.25), so the whole set is `L · 10^d` metres for a window `L`
//! wide. Three phases, all computed from `n = d - d_U` (log10 of set / universe) so nothing
//! overflows at any depth:
//! 1. the set grows up to the observable universe `U` ("as big as Earth");
//! 2. the set is an imaginary universe in which a familiar thing is as big as ours:
//!    set / U = U / thing, down to the Planck length ("… an imaginary universe in which an
//!    atom is as big as our universe");
//! 3. past the Planck rung, the imaginary universes nest: at each level the Planck length is as
//!    big as the universe below (`P = log10(U / Planck)` ≈ 61.7 decades a level), and at the
//!    bottom a familiar thing is as big as ours: `n = k·P + r` reads "k + 1 levels deep, at the
//!    bottom <thing of U / 10^r>" (user's wording, 2026-10-17).

/// Width of the Mandelbrot set along the real axis: from -2 to 1/4 (both exact: the tip of
/// the antenna and the cusp of the main cardioid).
pub const SET_WIDTH: f64 = 2.25;

/// Physical width assumed for the view when the platform cannot say (metres).
pub const DEFAULT_SCREEN_M: f64 = 0.30;

/// Broad kinds of things, for the ruler's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Cosmos,
    Space,
    Star,
    Planet,
    Earthly,
    Life,
    Molecule,
    Particle,
    /// The Planck length.
    Limit,
    /// Multiples of the universe ("1,000 × universe", "googol ×").
    Number,
    /// Nested universes ("3× nested").
    Ladder,
}

/// One real thing on the ladder.
#[derive(Clone, Copy, Debug)]
pub struct Thing {
    /// Stable id.
    pub key: &'static str,
    /// Short label on the ruler.
    pub label: &'static str,
    /// Name in a sentence ("the Milky Way", "a proton").
    pub name: &'static str,
    /// Characteristic size in metres (a diameter or a distance, see `what`).
    pub size_m: f64,
    /// Which size: "diameter", "distance", ...
    pub what: &'static str,
    pub group: Group,
    /// Where the number comes from (a kb source page; "(unverified)" when none).
    pub source: &'static str,
}

const NASA: &str = "kb: sources/nasa-planetary-fact-sheets";
const CODATA: &str = "kb: sources/codata-2022-constants";
const IAU: &str = "kb: sources/iau-astronomical-units";
const REFS: &str = "kb: sources/cosmic-scale-references";

/// Largest first. Sizes are rounded; the sources give the exact figures.
pub static THINGS: &[Thing] = &[
    Thing {
        key: "universe",
        label: "universe",
        name: "the observable universe",
        size_m: 8.7e26,
        what: "diameter, about 92 billion light years",
        group: Group::Cosmos,
        source: REFS,
    },
    Thing {
        key: "andromeda",
        label: "to Andromeda",
        name: "the way to Andromeda",
        size_m: 2.4e22,
        what: "distance, about 2.5 million light years",
        group: Group::Space,
        source: REFS,
    },
    Thing {
        key: "milky-way",
        label: "Milky Way",
        name: "the Milky Way",
        size_m: 9.5e20,
        what: "diameter of the disc, about 100 000 light years",
        group: Group::Space,
        source: REFS,
    },
    Thing {
        key: "proxima",
        label: "nearest star",
        name: "the way to the nearest star",
        size_m: 4.0e16,
        what: "distance to Proxima Centauri, 4.24 light years",
        group: Group::Star,
        source: REFS,
    },
    Thing {
        key: "light-year",
        label: "light year",
        name: "a light year",
        size_m: 9.460_730_472_580_8e15,
        what: "the IAU light year",
        group: Group::Star,
        source: IAU,
    },
    Thing {
        key: "solar-system",
        label: "Neptune's orbit",
        name: "the solar system",
        size_m: 9.03e12,
        what: "diameter of Neptune's orbit (2 × 4.515e9 km)",
        group: Group::Planet,
        source: NASA,
    },
    Thing {
        key: "earth-orbit",
        label: "Earth's orbit",
        name: "Earth's orbit",
        size_m: 2.992e11,
        what: "diameter, 2 au",
        group: Group::Planet,
        source: IAU,
    },
    Thing {
        key: "sun",
        label: "Sun",
        name: "the Sun",
        size_m: 1.3914e9,
        what: "diameter (2 × 695 700 km)",
        group: Group::Star,
        source: NASA,
    },
    Thing {
        key: "jupiter",
        label: "Jupiter",
        name: "Jupiter",
        size_m: 1.398e8,
        what: "mean diameter (2 × 69 911 km)",
        group: Group::Planet,
        source: NASA,
    },
    Thing {
        key: "earth",
        label: "Earth",
        name: "Earth",
        size_m: 1.2742e7,
        what: "mean diameter (2 × 6371 km)",
        group: Group::Planet,
        source: NASA,
    },
    Thing {
        key: "moon",
        label: "Moon",
        name: "the Moon",
        size_m: 3.4748e6,
        what: "mean diameter (2 × 1737.4 km)",
        group: Group::Planet,
        source: NASA,
    },
    Thing {
        key: "everest",
        label: "Everest",
        name: "Mount Everest",
        size_m: 8848.86,
        what: "height above sea level",
        group: Group::Earthly,
        source: REFS,
    },
    Thing {
        key: "human",
        label: "you",
        name: "you",
        size_m: 1.7,
        what: "a typical adult height",
        group: Group::Life,
        source: "(unverified)",
    },
    Thing {
        key: "ant",
        label: "ant",
        name: "an ant",
        size_m: 5e-3,
        what: "a typical worker ant (unverified)",
        group: Group::Life,
        source: "(unverified)",
    },
    Thing {
        key: "hair",
        label: "hair",
        name: "a hair's width",
        size_m: 7e-5,
        what: "width of a human hair (15-120 µm)",
        group: Group::Life,
        source: REFS,
    },
    Thing {
        key: "cell",
        label: "cell",
        name: "a red blood cell",
        size_m: 7.5e-6,
        what: "diameter",
        group: Group::Life,
        source: REFS,
    },
    Thing {
        key: "bacterium",
        label: "bacterium",
        name: "a bacterium",
        size_m: 2e-6,
        what: "length of E. coli",
        group: Group::Life,
        source: REFS,
    },
    Thing {
        key: "virus",
        label: "virus",
        name: "a virus",
        size_m: 1e-7,
        what: "diameter of an influenza virus",
        group: Group::Molecule,
        source: REFS,
    },
    Thing {
        key: "atom",
        label: "atom",
        name: "an atom",
        size_m: 1.058e-10,
        what: "a hydrogen atom, 2 Bohr radii",
        group: Group::Particle,
        source: CODATA,
    },
    Thing {
        key: "proton",
        label: "proton",
        name: "a proton",
        size_m: 1.6815e-15,
        what: "2 × the rms charge radius 0.84075 fm",
        group: Group::Particle,
        source: CODATA,
    },
    Thing {
        key: "planck",
        label: "Planck",
        name: "the Planck length",
        size_m: 1.616_255e-35,
        what: "the Planck length, the smallest length physics can talk about",
        group: Group::Limit,
        source: CODATA,
    },
];

/// Phase 2's rungs on the ruler ("Earth = universe": in the imaginary universe as big as the
/// set, Earth is as big as ours): familiar things, spaced at least ~4.7 decades (70 points)
/// apart.
const RUNGS: &[(&str, &str)] = &[
    ("milky-way", "galaxy = universe"),
    ("solar-system", "solar system = universe"),
    ("earth", "Earth = universe"),
    ("human", "you = universe"),
    ("cell", "cell = universe"),
    ("atom", "atom = universe"),
    ("proton", "proton = universe"),
    ("planck", "Planck = universe"),
];

/// Phase 2's sentence compares with these (familiar ones; no distances).
const SMALL: &[&str] = &[
    "milky-way",
    "solar-system",
    "sun",
    "earth",
    "moon",
    "everest",
    "human",
    "ant",
    "hair",
    "cell",
    "virus",
    "atom",
    "proton",
    "planck",
];

fn thing(key: &str) -> Option<usize> {
    THINGS.iter().position(|t| t.key == key)
}

fn size_of(key: &str) -> f64 {
    THINGS[thing(key).expect("cosmic thing")].size_m
}

/// Decades from the universe to the Planck length (≈ 61.7): phase 2's length.
pub fn planck_decades() -> f64 {
    (size_of("universe") / size_of("planck")).log10()
}

/// What the overlay shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CosmicScaleSettings {
    pub enabled: bool,
    /// Physical size of a view point in metres (0: unknown, the view is taken as 30 cm wide).
    pub metres_per_point: f64,
}

/// One mark on the ruler, at depth `d`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub d: f64,
    pub label: String,
    pub group: Group,
}

/// The readout at one depth: a sentence and its explanation.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Reading {
    pub line: String,
    pub detail: String,
}

/// The ruler is linear up to just past the Planck rung, then compressed with this final
/// slope (a nesting level of 61.7 decades ≈ 116 points at 15 points per decade).
const COMPRESS: f64 = 8.0;
/// Decades over which the slope eases from 1 to 1/COMPRESS.
const COMPRESS_TAU: f64 = 15.0;
/// Nested levels marked on the ruler (level 52 is past zoom 1e3100 for a 30 cm window).
const LEVELS: u32 = 52;

/// The scale for one window: marks in depth and the phase boundaries.
#[derive(Clone, Debug)]
pub struct CosmicScale {
    /// The view's physical width, metres.
    screen_m: f64,
    /// The set reaches the universe here.
    pub universe_d: f64,
    /// Increasing in `d`.
    pub marks: Vec<Mark>,
}

impl CosmicScale {
    /// `view_width_pt`: the view's width in points.
    pub fn new(s: &CosmicScaleSettings, view_width_pt: f64) -> Self {
        let measured = s.metres_per_point.is_finite()
            && s.metres_per_point > 0.0
            && view_width_pt.is_finite()
            && view_width_pt > 0.0;
        let screen_m = Some(s.metres_per_point * view_width_pt)
            .filter(|m| measured && m.is_finite() && *m > 0.0)
            .unwrap_or(DEFAULT_SCREEN_M);
        let u = size_of("universe");
        let ud = (u / screen_m).log10();
        let mut marks = Vec::new();
        // Phase 1: every thing larger than the window, by the set's size.
        for t in THINGS.iter().filter(|t| t.size_m >= screen_m) {
            marks.push(Mark {
                d: (t.size_m / screen_m).log10(),
                label: t.label.into(),
                group: t.group,
            });
        }
        // Multiples where no rung sits ("1 million ×" would sit on "universe = galaxy").
        for (n, label) in [(3.0, "1,000 × universe"), (100.0, "googol ×")] {
            marks.push(Mark {
                d: ud + n,
                label: label.into(),
                group: Group::Number,
            });
        }
        // Phase 2: set / U = U / thing.
        for (key, label) in RUNGS {
            let i = thing(key).expect("rung");
            marks.push(Mark {
                d: ud + (u / THINGS[i].size_m).log10(),
                label: (*label).into(),
                group: THINGS[i].group,
            });
        }
        // Phase 3: `k` levels of imaginary universes from n = (k - 1) P (level 2 starts at the
        // Planck rung).
        let p = planck_decades();
        for k in 3..=LEVELS {
            marks.push(Mark {
                d: ud + f64::from(k - 1) * p,
                label: format!("{k} levels"),
                group: Group::Ladder,
            });
        }
        marks.sort_by(|a, b| a.d.total_cmp(&b.d));
        Self {
            screen_m,
            universe_d: ud,
            marks,
        }
    }

    /// Where phase 3 begins (the Planck rung).
    pub fn planck_d(&self) -> f64 {
        self.universe_d + planck_decades()
    }

    /// Ruler position of depth `d` in ruler decades: linear up to just past the Planck rung,
    /// then compressed (slope 1 → 1/8 over 15 decades).
    pub fn ruler(&self, d: f64) -> f64 {
        let a = self.planck_d() + 1.0;
        if d <= a {
            return d;
        }
        let y = d - a;
        let c = COMPRESS;
        a + y / c + (1.0 - 1.0 / c) * COMPRESS_TAU * (1.0 - (-y / COMPRESS_TAU).exp())
    }

    /// Index of the last mark at or before `d` (None: before all).
    pub fn passed(&self, d: f64) -> Option<usize> {
        let n = self.marks.partition_point(|m| m.d <= d);
        n.checked_sub(1)
    }

    pub fn reading(&self, d: f64) -> Reading {
        let window = format!("your window: {}", length(self.screen_m));
        let d = if d.is_nan() { 0.0 } else { d };
        // log10(set / universe).
        let n = d - self.universe_d;
        if (0.8f64.log10()..0.3).contains(&n) {
            // The turn (zoom ≈ 3e27 for 30 cm): the set is the universe.
            return Reading {
                line: format!("{WHOLE} would be as big as the observable universe"),
                detail: window,
            };
        }
        let galaxy = (size_of("universe") / size_of("milky-way")).log10();
        if (0.3..galaxy - 0.6).contains(&n) {
            // Between the turn and the first rung no familiar thing is near (only distances
            // to galaxies); the ruler shows "1,000 × universe".
            return Reading {
                line: format!("{WHOLE} would be bigger than the observable universe"),
                detail: window,
            };
        }
        if n < 0.3 {
            // Phase 1: the set grows.
            let m = self.screen_m * 10f64.powf(d);
            let line = if m < 1e4 {
                format!("{WHOLE} would be {} across", length(m))
            } else {
                format!("{WHOLE} would be {}", as_big_as(m))
            };
            return Reading { line, detail: window };
        }
        let p = planck_decades();
        if n < p + 1.25f64.log10() {
            // Phase 2: an imaginary universe as big as the set; set / U = U / v.
            let v = size_of("universe") * 10f64.powf(-n);
            return Reading {
                line: format!("{WHOLE} would be as big as an imaginary universe"),
                detail: format!("in which {} as big as ours", thing_is(v)),
            };
        }
        // Phase 3: imaginary universes nested; at each level the Planck length is as big as
        // the universe below; n = k·P + r.
        let k = (n / p).floor();
        let r = n - k * p;
        let line = if (k + 1.0).is_finite() && k < 1e15 {
            format!(
                "{WHOLE} would be as big as an imaginary universe, {} levels deep",
                count((k + 1.0).log10())
            )
        } else {
            format!("{WHOLE} would be as big as an imaginary universe, countless levels deep")
        };
        let detail = if r < 0.3 {
            "at the bottom: our universe itself".to_string()
        } else {
            format!(
                "at the bottom, {} as big as ours",
                thing_is(size_of("universe") * 10f64.powf(-r))
            )
        };
        Reading { line, detail }
    }
}

/// How the whole set is named in the readout (not "the whole set": jargon for newcomers).
const WHOLE: &str = "The whole Mandelbrot set";

/// "as big as Earth", "3× as big as the Sun", "40% as big as the Moon": `m` metres against
/// the nearest (in log) thing.
fn as_big_as(m: f64) -> String {
    let t = nearest(m, THINGS.iter());
    let r = m / t.size_m;
    if (0.8..1.25).contains(&r) {
        format!("as big as {}", t.name)
    } else if r >= 1.25 {
        format!("{}× as big as {}", count(r.log10()), t.name)
    } else if r >= 0.1 {
        format!("{:.0}% as big as {}", r * 100.0, t.name)
    } else {
        format!("1/{} as big as {}", count(-r.log10()), t.name)
    }
}

/// The subject of "… is as big as ours" for a thing of `v` metres, against the nearest (in
/// log) familiar thing: "an atom is", "you are", "something 3× as big as the Sun is",
/// "1/40 of a proton is".
fn thing_is(v: f64) -> String {
    let t = nearest(v, SMALL.iter().map(|k| &THINGS[thing(k).expect("small thing")]));
    let r = v / t.size_m;
    let verb = if t.key == "human" { "are" } else { "is" };
    if (0.8..1.25).contains(&r) {
        format!("{} {verb}", t.name)
    } else if r >= 1.25 {
        format!("something {}× as big as {} is", count(r.log10()), t.name)
    } else if r >= 0.1 {
        format!("something {:.0}% as big as {} is", r * 100.0, t.name)
    } else {
        format!("1/{} of {} is", count(-r.log10()), t.name)
    }
}

fn nearest<'a>(v: f64, things: impl Iterator<Item = &'a Thing>) -> &'a Thing {
    let key = |t: &Thing| (v / t.size_m).log10().abs();
    things.min_by(|a, b| key(a).total_cmp(&key(b))).unwrap_or(&THINGS[0])
}

/// A count given as its log10 `n` (any size, never overflows): "1.5", "200", "45,000",
/// "3.4 million", "12 billion", "2 trillion", then "3e15", "1e16", ... .
pub fn count(n: f64) -> String {
    if !n.is_finite() {
        return if n > 0.0 { "∞".into() } else { "0".into() };
    }
    if n < 1.0 {
        let r = 10f64.powf(n);
        let t = format!("{r:.1}");
        return t.strip_suffix(".0").unwrap_or(&t).to_string();
    }
    if n < 3.0 {
        return format!("{:.0}", 10f64.powf(n));
    }
    // Two significant digits, from the log: `sig` in 10 ... 99 times 10^(e-1).
    let mut e = n.floor();
    let mut sig = 10f64.powf(n - e + 1.0).round();
    if sig >= 100.0 {
        sig = 10.0;
        e += 1.0;
    }
    if e < 15.0 {
        let v = sig * 10f64.powf(e - 1.0);
        let (div, unit) = match e as i32 {
            ..=5 => {
                let k = v.round() as u64;
                return format!("{},{:03}", k / 1000, k % 1000);
            }
            6..=8 => (1e6, "million"),
            9..=11 => (1e9, "billion"),
            _ => (1e12, "trillion"),
        };
        let x = v / div;
        let t = format!("{x:.1}");
        return format!("{} {unit}", t.strip_suffix(".0").unwrap_or(&t));
    }
    // One digit from here on: 1e15, 3e15, 1e16; 9.5 and up rounds to the next decade.
    let mut e = n.floor();
    let m = 10f64.powf(n - e);
    let m = if m >= 9.5 {
        e += 1.0;
        1.0
    } else if m < 1.5 {
        1.0
    } else {
        m.round()
    };
    format!("{m:.0}e{e:.0}")
}

/// "30 cm", "1.4 m", "120 m", "3 km".
fn length(m: f64) -> String {
    if !m.is_finite() || m <= 0.0 {
        return "0 cm".into();
    }
    if m < 1.0 {
        format!("{:.0} cm", m * 100.0)
    } else if m < 9.95 {
        let t = format!("{m:.1}");
        format!("{} m", t.strip_suffix(".0").unwrap_or(&t))
    } else if m < 1000.0 {
        format!("{m:.0} m")
    } else {
        format!("{:.0} km", m / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(metres_per_point: f64) -> CosmicScale {
        CosmicScale::new(
            &CosmicScaleSettings {
                enabled: true,
                metres_per_point,
            },
            1000.0,
        )
    }

    #[test]
    fn table_is_sorted() {
        for w in THINGS.windows(2) {
            assert!(w[0].size_m > w[1].size_m, "{} {}", w[0].key, w[1].key);
        }
        assert!((planck_decades() - 61.73).abs() < 0.01);
    }

    #[test]
    fn readout_through_the_three_phases() {
        let s = scale(0.0);
        let line = |zoom: f64| s.reading(zoom).line;
        assert_eq!(line(4.0), "The whole Mandelbrot set would be 3 km across");
        assert_eq!(
            line((1.2742e7f64 / 0.3).log10()),
            "The whole Mandelbrot set would be as big as Earth"
        );
        assert_eq!(
            line(3e27f64.log10()),
            "The whole Mandelbrot set would be as big as the observable universe"
        );
        let imaginary = "The whole Mandelbrot set would be as big as an imaginary universe";
        assert_eq!(line(33.0), imaginary);
        let detail = |d: f64| s.reading(d).detail;
        assert_eq!(
            detail(33.0),
            "in which something 2.7× as big as the Milky Way is as big as ours"
        );
        assert_eq!(detail(64.4), "in which an atom is as big as ours");
        assert_eq!(detail(89.2), "in which the Planck length is as big as ours");
        assert_eq!(line(101.0), format!("{imaginary}, 2 levels deep"));
        assert_eq!(
            detail(101.0),
            "at the bottom, something 150× as big as the solar system is as big as ours"
        );
        assert_eq!(line(1000.0), format!("{imaginary}, 16 levels deep"));
        assert_eq!(line(3010.0), format!("{imaginary}, 49 levels deep"));
        // "you are", not "you is".
        let you = (0..2000)
            .map(|k| s.reading(30.0 + k as f64 * 0.05).detail)
            .find(|t| t.starts_with("in which you "));
        assert_eq!(you.as_deref(), Some("in which you are as big as ours"));
        assert_eq!(s.reading(4.0).detail, "your window: 30 cm");
        assert_eq!(scale(0.00045).reading(4.0).detail, "your window: 45 cm");
        // Every depth, finely and absurdly: never a panic, NaN or jargon.
        for k in -40..=40_000 {
            let r = s.reading(k as f64 * 0.08);
            assert!(!r.line.contains("NaN") && !r.line.contains("inf") && !r.line.contains("GeV"));
        }
        for d in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300, -1e300] {
            assert!(!s.reading(d).line.is_empty());
        }
    }

    #[test]
    fn marks_never_crowd() {
        let s = scale(0.0);
        assert!(s.marks.windows(2).all(|w| w[0].d < w[1].d));
        assert!(s.marks.iter().all(|m| !m.label.contains("GUT")));
        assert!(s.marks.last().unwrap().d > 3100.0);
        // Past the universe at least 25 points apart (15 points per ruler decade).
        let past: Vec<f64> = s
            .marks
            .iter()
            .filter(|m| m.d >= s.universe_d - 1e-9)
            .map(|m| s.ruler(m.d))
            .collect();
        for w in past.windows(2) {
            assert!((w[1] - w[0]) * 15.0 >= 25.0, "{w:?}");
        }
    }

    #[test]
    fn counts_read_plainly() {
        assert_eq!(count(0.3), "2");
        assert_eq!(count(0.415), "2.6");
        assert_eq!(count(2.3), "200");
        assert_eq!(count(4.653), "45,000");
        assert_eq!(count(6.53), "3.4 million");
        assert_eq!(count(9.48), "3 billion");
        assert_eq!(count(15.48), "3e15");
        assert_eq!(count(f64::INFINITY), "∞");
    }
}
