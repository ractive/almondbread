//! The cosmic size scale (kb: decisions/controls, "Cosmic size scale"; ideas.md): how the
//! zoom compares with real things, from the observable universe down to the Planck length,
//! then famous big numbers and universe-to-Planck "ladders" for the depths no physical
//! object reaches. Platform-neutral: the table, the readout text and the ruler marks; the
//! render crate draws them (`almondbread_render::cosmic`).
//!
//! Depth coordinate: `d = log10(set width / view width)`, both in the complex plane (the set
//! spans re -2 ... 1/4, width 2.25). Two anchors:
//! - screen: the view is a physical width `L` (the window on the screen) at every depth, so
//!   the whole set is `L · 10^d` metres: things up to the observable universe, then
//!   multiples of it ("2 million × the observable universe"), counted in logs to the depth
//!   limit (no turn-around to the Planck length: "very confusing", user, 2026-10-16).
//! - set as object X: the view is `X · 10^-d`, down to the Planck length.
//!
//! Past the Planck length (set-as-object anchors) the zoom factor itself is the story:
//! milestones (Planck times since the Big Bang, atoms in the universe, a googol, ...) and
//! then ladders.

/// Width of the Mandelbrot set along the real axis: from -2 to 1/4 (both exact: the tip of
/// the antenna and the cusp of the main cardioid).
pub const SET_WIDTH: f64 = 2.25;

/// `h c` in eV·m (exact since the 2019 SI; kb: sources/codata-2022-constants).
pub const HC_EV_M: f64 = 1.239_841_984e-6;

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
    /// The smallest scale probed, the Planck length.
    Limit,
    /// Big numbers past the Planck length.
    Number,
    /// Universe-to-Planck ladders.
    Ladder,
}

/// One real thing on the ladder.
#[derive(Clone, Copy, Debug)]
pub struct Thing {
    /// Stable id (persisted anchor choice).
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
        name: "the solar system out to Neptune",
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
        key: "dna",
        label: "DNA",
        name: "the width of DNA",
        size_m: 2e-9,
        what: "width of the double helix",
        group: Group::Molecule,
        source: REFS,
    },
    Thing {
        key: "atom",
        label: "atom",
        name: "a hydrogen atom",
        size_m: 1.058e-10,
        what: "2 Bohr radii",
        group: Group::Particle,
        source: CODATA,
    },
    Thing {
        key: "nucleus",
        label: "nucleus",
        name: "a gold nucleus",
        size_m: 1.4e-14,
        what: "diameter, 2 × 1.2 fm × 197^(1/3)",
        group: Group::Particle,
        source: REFS,
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
        key: "probed",
        label: "limit probed",
        name: "the smallest scale ever probed",
        size_m: 1e-19,
        what: "about what colliders resolve (quark size limit)",
        group: Group::Limit,
        source: REFS,
    },
    Thing {
        key: "planck",
        label: "Planck (strings?)",
        name: "the Planck length",
        size_m: 1.616_255e-35,
        what: "the Planck length; strings, if they exist, are about this size (hypothetical)",
        group: Group::Limit,
        source: CODATA,
    },
];

/// An epoch of cosmic history: its time after the Big Bang.
#[derive(Clone, Copy, Debug)]
pub struct Epoch {
    pub label: &'static str,
    /// What happens then ("the first stars shine").
    pub name: &'static str,
    /// Seconds after the Big Bang.
    pub t_s: f64,
    pub source: &'static str,
}

const TIMELINE: &str = "kb: sources/cosmic-history-timeline";

/// Newest first (relative mode travels back in time as it zooms in).
pub static EPOCHS: &[Epoch] = &[
    Epoch {
        label: "today",
        name: "today",
        t_s: 13.787e9 * YEAR_S,
        source: REFS,
    },
    Epoch {
        label: "Sun forms",
        name: "the Sun and Earth form",
        t_s: 9.2e9 * YEAR_S,
        source: REFS,
    },
    Epoch {
        label: "first stars",
        name: "the first stars shine (100-200 Myr)",
        t_s: 1e8 * YEAR_S,
        source: TIMELINE,
    },
    Epoch {
        label: "CMB",
        name: "light breaks free (the CMB)",
        t_s: 3.8e5 * YEAR_S,
        source: TIMELINE,
    },
    Epoch {
        label: "first nuclei",
        name: "the first nuclei form",
        t_s: 180.0,
        source: TIMELINE,
    },
    Epoch {
        label: "neutrinos",
        name: "neutrinos decouple",
        t_s: 1.0,
        source: TIMELINE,
    },
    Epoch {
        label: "protons form",
        name: "quarks bind into protons",
        t_s: 2e-5,
        source: TIMELINE,
    },
    Epoch {
        label: "electroweak",
        name: "the electroweak transition",
        t_s: 1e-11,
        source: TIMELINE,
    },
    Epoch {
        label: "inflation ends",
        name: "inflation ends (hypothetical)",
        t_s: 1e-32,
        source: TIMELINE,
    },
    Epoch {
        label: "inflation?",
        name: "inflation begins (hypothetical)",
        t_s: 1e-36,
        source: TIMELINE,
    },
    Epoch {
        label: "Planck epoch",
        name: "the Planck epoch",
        t_s: PLANCK_TIME_S,
        source: CODATA,
    },
];

/// Relative mode maps zoom 1 ... 1e200 onto the cosmos, the things evenly spaced in zoom
/// decades: 23 steps of 8.7 decades, about 12.6 s each at the default zoom speed (×4.95/s,
/// 0.69 decades/s), the whole cosmos in under 5 minutes. A first version spaced them in
/// log10(1 + decades) over 1e3000: steps grew from 1 s to 20 min ("from hair to blood cell
/// takes ages", user, 2026-10-15).
pub const RELATIVE_DEPTH: f64 = 200.0;

/// Where the epochs sit on the things' grid (index into [`THINGS`], fractional allowed): from
/// the CMB on, where the view is about the thermal wavelength `hc/kT` of the epoch's
/// temperature (CMB ~3000 K: a cell; nucleosynthesis ~0.1 MeV: an atom; quark-hadron ~150
/// MeV: a nucleus; electroweak ~160 GeV: below a proton; inflation: inside the unexplored
/// band; Planck: Planck), so size and time do not drift apart; today, the Sun and the first
/// stars spread over the astronomical part (kb: sources/cosmic-history-timeline).
const EPOCH_GRID: [f64; 11] = [0.0, 7.0, 14.0, 15.0, 19.0, 19.5, 20.0, 21.5, 22.5, 22.75, 23.0];

/// Relative mode's grid coordinate of zoom decades `z`: 0 (the universe, today) ... the last
/// thing (the Planck length, the Planck epoch).
fn rel_grid(z: f64) -> f64 {
    (z / RELATIVE_DEPTH).clamp(0.0, 1.0) * (THINGS.len() - 1) as f64
}

/// Zoom decades of grid position `g`.
fn rel_z(g: f64) -> f64 {
    g / (THINGS.len() - 1) as f64 * RELATIVE_DEPTH
}

/// Ruler units per zoom decade in relative mode (a step of 8.3 decades is ~3 ruler decades).
const REL_RULER: f64 = 0.36;

/// Piecewise-linear `ys` at `x` over increasing `xs`.
fn interp(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    let i = xs.partition_point(|&a| a <= x).clamp(1, xs.len() - 1);
    let (x0, x1) = (xs[i - 1], xs[i]);
    let f = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
    ys[i - 1] + (ys[i] - ys[i - 1]) * f
}

/// A big number the zoom factor passes.
#[derive(Clone, Copy, Debug)]
pub struct Milestone {
    pub label: &'static str,
    /// In a sentence ("the atoms in the observable universe").
    pub name: &'static str,
    /// log10 of the number.
    pub log10: f64,
    pub source: &'static str,
}

pub static MILESTONES: &[Milestone] = &[
    Milestone {
        label: "Planck times",
        name: "the Planck times since the Big Bang",
        // 13.787 Gyr / 5.391247e-44 s = 8.07e60.
        log10: 60.907,
        source: REFS,
    },
    Milestone {
        label: "atoms",
        name: "the atoms in the observable universe",
        log10: 80.0,
        source: REFS,
    },
    Milestone {
        label: "googol",
        name: "a googol",
        log10: 100.0,
        source: REFS,
    },
    Milestone {
        label: "chess games",
        name: "Shannon's count of chess games",
        log10: 120.0,
        source: REFS,
    },
    Milestone {
        label: "Planck volumes",
        name: "the Planck volumes in the observable universe",
        // 4/3 π (4.35e26 m)^3 / (1.616255e-35 m)^3 = 8.2e184.
        log10: 184.91,
        source: REFS,
    },
];

fn thing(key: &str) -> Option<usize> {
    THINGS.iter().position(|t| t.key == key)
}

fn size_of(key: &str) -> f64 {
    THINGS[thing(key).expect("cosmic thing")].size_m
}

/// Decades from the observable universe to the Planck length (≈ 61.7): one "ladder".
pub fn ladder_decades() -> f64 {
    (size_of("universe") / size_of("planck")).log10()
}

/// The anchors offered in the UI: "screen", then things the set can be.
pub const ANCHORS: &[&str] = &[
    "screen",
    "universe",
    "milky-way",
    "sun",
    "earth",
    "human",
    "cell",
    "atom",
    "cosmos",
];

/// The picker's group of an anchor: 0 the screen, 1 "the set is ...", 2 the metaphor.
pub fn anchor_group(key: &str) -> u32 {
    match key {
        "screen" => 0,
        "cosmos" => 2,
        _ => 1,
    }
}

/// A short label for an anchor key.
pub fn anchor_label(key: &str) -> String {
    match key {
        "screen" => "Your screen".into(),
        "cosmos" => "Zoom range as cosmos (metaphor)".into(),
        _ => match thing(key) {
            Some(i) => format!("The set is {}", THINGS[i].name),
            None => key.into(),
        },
    }
}

/// One line saying what an anchor compares.
pub fn anchor_description(key: &str) -> String {
    match key {
        "screen" => "Your window stays 30 cm (or its real size): how big would the whole set be?".into(),
        "cosmos" => {
            "A metaphor: home is the universe and today, the deepest zoom the Planck length and the Big Bang.".into()
        }
        _ => match thing(key) {
            Some(i) => format!("The whole set is {}: how big is the part you see?", THINGS[i].name),
            None => String::new(),
        },
    }
}

/// What the overlay shows.
#[derive(Clone, Debug, PartialEq)]
pub struct CosmicScaleSettings {
    pub enabled: bool,
    /// "screen" or a [`THINGS`] key: the set is that big.
    pub anchor: String,
    /// Physical size of a view point in metres (0: unknown, the view is taken as 30 cm wide).
    pub metres_per_point: f64,
}

impl Default for CosmicScaleSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            anchor: "screen".into(),
            metres_per_point: 0.0,
        }
    }
}

/// One mark on the ruler, at depth `d`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub d: f64,
    pub label: String,
    pub group: Group,
    /// Label row: Some(0) above the ruler, Some(1) below; None: whichever is free.
    pub row: Option<usize>,
}

/// The readout at one depth.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Reading {
    /// The main sentence.
    pub line: String,
    /// The assumption or what comes next.
    pub detail: String,
    /// Light's crossing time (length and time are one ladder through c) and, for small
    /// views, the probe energy needed to see them.
    pub physics: Option<String>,
}

/// The scale for one anchor and window: marks in depth and the phase boundaries.
#[derive(Clone, Debug)]
pub struct CosmicScale {
    /// The set's thing, None: the screen (or relative mode).
    anchor: Option<usize>,
    /// The view's physical width (screen anchor), metres.
    screen_m: f64,
    /// `d - log10(zoom)`: the HUD zoom of a depth (milestones count the zoom the HUD shows).
    offset: f64,
    /// Screen anchor: the set reaches the universe here.
    pub universe_d: Option<f64>,
    /// The view reaches the Planck length here (end of the physical scale); None: the screen
    /// anchor, whose view never shrinks.
    pub planck_d: Option<f64>,
    /// The unexplored band (smallest probed scale to the Planck length), in depth; None: the
    /// screen anchor.
    pub desert: Option<(f64, f64)>,
    /// The ruler is linear in depth up to here, compressed beyond.
    compress_from: f64,
    /// Increasing in `d`.
    pub marks: Vec<Mark>,
    /// Relative mode: the zoom range 1 ... 1e200 stands for the universe ... the Planck
    /// length and today ... the Planck epoch (a metaphor, not a measurement).
    pub relative: bool,
}

/// Screen anchor: multiples of the observable universe on the ruler (log10 of the multiple).
/// Dense while the ruler is linear, then sparse enough that they never crowd where it is
/// compressed (100 decades ≈ 125 points there), up to past the depth limit (~1e3010).
fn universe_multiples() -> impl Iterator<Item = (f64, String)> {
    [
        (3.0, "1,000 × universe".to_string()),
        (6.0, "1 million ×".into()),
        (9.0, "1 billion ×".into()),
        (12.0, "1 trillion ×".into()),
        (20.0, "1e20 ×".into()),
        (30.0, "1e30 ×".into()),
        (50.0, "1e50 ×".into()),
        (100.0, "googol ×".into()),
    ]
    .into_iter()
    .chain((2..=40).map(|k| (k as f64 * 100.0, format!("1e{} ×", k * 100))))
}

/// Screen anchor: the ruler is linear up to this many decades past the universe (just after
/// "1 billion ×"), compressed beyond.
const SCREEN_LINEAR: f64 = 10.0;

impl CosmicScale {
    /// `view_width_pt`: the view's width in points; `offset`: `d - log10(HUD zoom)`.
    pub fn new(s: &CosmicScaleSettings, view_width_pt: f64, offset: f64) -> Self {
        let measured = s.metres_per_point.is_finite() && s.metres_per_point > 0.0 && view_width_pt > 0.0;
        let screen_m = if measured {
            s.metres_per_point * view_width_pt
        } else {
            DEFAULT_SCREEN_M
        };
        if s.anchor == "cosmos" {
            return Self::relative(offset);
        }
        let mut marks = Vec::new();
        let Some(i) = thing(&s.anchor).filter(|_| s.anchor != "screen") else {
            // The window stays the window: only the set grows, through every thing larger
            // than the window, then multiples of the universe.
            let ud = (size_of("universe") / screen_m).log10();
            for t in THINGS.iter().filter(|t| t.size_m >= screen_m) {
                marks.push(Mark {
                    d: (t.size_m / screen_m).log10(),
                    label: t.label.into(),
                    group: t.group,
                    row: None,
                });
            }
            for (n, label) in universe_multiples() {
                marks.push(Mark {
                    d: ud + n,
                    label,
                    group: Group::Cosmos,
                    row: None,
                });
            }
            marks.sort_by(|a, b| a.d.total_cmp(&b.d));
            return Self {
                anchor: None,
                screen_m,
                offset,
                universe_d: Some(ud),
                planck_d: None,
                desert: None,
                compress_from: ud + SCREEN_LINEAR,
                marks,
                relative: false,
            };
        };
        let top = THINGS[i].size_m;
        // Shrinking view inside a set of size `top`: things no larger than the set.
        for t in THINGS.iter().filter(|t| t.size_m <= top) {
            marks.push(Mark {
                d: (top / t.size_m).log10(),
                label: t.label.into(),
                group: t.group,
                row: None,
            });
        }
        let planck_d = (top / size_of("planck")).log10();
        let desert = ((top / size_of("probed")).log10(), planck_d);
        for m in MILESTONES {
            marks.push(Mark {
                d: m.log10 + offset,
                label: m.label.into(),
                group: Group::Number,
                row: None,
            });
        }
        let ladder = ladder_decades();
        // Ladders count once the milestones are done (1e185 is three ladders).
        for k in 4..=60 {
            marks.push(Mark {
                d: k as f64 * ladder + offset,
                label: format!("{k} ladders"),
                group: Group::Ladder,
                row: None,
            });
        }
        marks.sort_by(|a, b| a.d.total_cmp(&b.d));
        Self {
            anchor: Some(i),
            screen_m,
            offset,
            universe_d: None,
            planck_d: Some(planck_d),
            desert: Some(desert),
            compress_from: planck_d + 1.0,
            marks,
            relative: false,
        }
    }

    /// Relative mode: sizes above the ruler, epochs below, on one grid evenly spaced in zoom
    /// decades; past the end, "cosmos lengths" (repeats of the whole ladder).
    fn relative(offset: f64) -> Self {
        let mut marks = Vec::new();
        for (i, t) in THINGS.iter().enumerate() {
            marks.push(Mark {
                d: rel_z(i as f64) + offset,
                label: t.label.into(),
                group: t.group,
                row: Some(0),
            });
        }
        for (e, g) in EPOCHS.iter().zip(EPOCH_GRID) {
            marks.push(Mark {
                d: rel_z(g) + offset,
                label: e.label.into(),
                group: Group::Number,
                row: Some(1),
            });
        }
        for k in 2..=20 {
            marks.push(Mark {
                d: k as f64 * RELATIVE_DEPTH + offset,
                label: format!("{k} cosmoses"),
                group: Group::Ladder,
                row: None,
            });
        }
        marks.sort_by(|a, b| a.d.total_cmp(&b.d));
        let at = |key: &str| rel_z(thing(key).expect("thing") as f64) + offset;
        Self {
            anchor: None,
            screen_m: DEFAULT_SCREEN_M,
            offset,
            universe_d: None,
            planck_d: Some(RELATIVE_DEPTH + offset),
            desert: Some((at("probed"), at("planck"))),
            compress_from: RELATIVE_DEPTH + 4.0,
            marks,
            relative: true,
        }
    }

    /// Ruler position of depth `d` in ruler decades: decades up to the Planck length (screen
    /// anchor: up to a billion universes), then compressed (big numbers and ladders, or
    /// multiples of the universe); relative mode: linear in zoom decades, compressed past the
    /// end in the same way.
    pub fn ruler(&self, d: f64) -> f64 {
        let compress = |x: f64, a: f64, c: f64, tau: f64| {
            if x <= a {
                x
            } else {
                let y = x - a;
                a + y / c + (1.0 - 1.0 / c) * tau * (1.0 - (-y / tau).exp())
            }
        };
        if self.relative {
            return compress(d - self.offset, self.compress_from, 6.0, 40.0) * REL_RULER;
        }
        compress(d, self.compress_from, 12.0, 15.0)
    }

    /// Faint rungs between `lo` and `hi`: the things repeated in every ladder (physical: from
    /// the big numbers on, universe-sized set to the Planck length; relative: every cosmos
    /// length past the first).
    pub fn rungs(&self, lo: f64, hi: f64) -> Vec<(f64, Group)> {
        if self.planck_d.is_none() {
            // The screen anchor has no ladders.
            return Vec::new();
        }
        let (len, from, place): (f64, f64, &dyn Fn(usize) -> f64) = if self.relative {
            (RELATIVE_DEPTH, RELATIVE_DEPTH, &|i| rel_z(i as f64))
        } else {
            let u = THINGS[0].size_m;
            (
                ladder_decades(),
                MILESTONES.last().map_or(0.0, |m| m.log10),
                &move |i| (u / THINGS[i].size_m).log10(),
            )
        };
        let mut out = Vec::new();
        let k0 = ((lo - self.offset) / len).floor().max(0.0) as i64;
        let k1 = ((hi - self.offset) / len).ceil() as i64;
        for k in k0..=k1.min(k0.saturating_add(8)) {
            let base = k as f64 * len + self.offset;
            for (i, t) in THINGS.iter().enumerate() {
                let d = base + place(i);
                if d - self.offset > from && d >= lo && d <= hi {
                    out.push((d, t.group));
                }
            }
        }
        out
    }

    /// Relative mode's view size (m) and cosmic time (s) at zoom decades `z`.
    pub fn relative_values(z: f64) -> (f64, f64) {
        let g = rel_grid(z);
        let grid: Vec<f64> = (0..THINGS.len()).map(|i| i as f64).collect();
        let sizes: Vec<f64> = THINGS.iter().map(|t| t.size_m.log10()).collect();
        let times: Vec<f64> = EPOCHS.iter().map(|e| e.t_s.log10()).collect();
        (
            10f64.powf(interp(g, &grid, &sizes)),
            10f64.powf(interp(g, &EPOCH_GRID, &times)),
        )
    }

    fn relative_reading(&self, z: f64) -> Reading {
        if z > RELATIVE_DEPTH {
            return Reading {
                line: format!("≈ {:.1} cosmos lengths deep", z / RELATIVE_DEPTH),
                detail: format!("1 cosmos length: universe → Planck length, zoom ×1e{RELATIVE_DEPTH:.0} (a metaphor)"),
                physics: None,
            };
        }
        let (v, t) = Self::relative_values(z);
        let g = rel_grid(z);
        // The epoch we are at (within 0.4 of a step), else just the time.
        let near = EPOCH_GRID.iter().position(|e| (e - g).abs() < 0.4);
        let detail = match near {
            Some(0) => "cosmic time: today".to_string(),
            Some(k) if k == EPOCHS.len() - 1 => "cosmic time: the Planck epoch".to_string(),
            Some(k) => format!("{} after the Big Bang: {}", plain(&duration(t)), EPOCHS[k].name),
            None => format!("{} after the Big Bang", plain(&duration(t))),
        };
        let next = (g.floor() as usize + 1).min(THINGS.len() - 1);
        let tail = if g >= (THINGS.len() - 1) as f64 {
            String::new()
        } else {
            format!(" · next: {}", THINGS[next].label)
        };
        Reading {
            line: format!("This view ≈ {}", compare(v, THINGS)),
            detail,
            physics: Some(format!(
                "a metaphor: zoom 1 … 1e{RELATIVE_DEPTH:.0} as the cosmos{tail}"
            )),
        }
    }

    /// `d - log10(HUD zoom)`.
    pub fn offset(&self) -> f64 {
        self.offset
    }

    /// The size of the thing the set is (or the universe, past the turn of the screen anchor).
    fn set_size(&self) -> f64 {
        self.anchor.map_or(size_of("universe"), |i| THINGS[i].size_m)
    }

    /// Index of the last mark at or before `d` (None: before all).
    pub fn passed(&self, d: f64) -> Option<usize> {
        let n = self.marks.partition_point(|m| m.d <= d);
        n.checked_sub(1)
    }

    pub fn reading(&self, d: f64) -> Reading {
        let zoom = d - self.offset;
        if self.relative {
            return self.relative_reading(zoom);
        }
        if let Some(ud) = self.universe_d {
            return self.screen_reading(d, ud);
        }
        let planck_d = self.planck_d.unwrap_or(f64::INFINITY);
        if d < planck_d {
            let top = self.set_size();
            let v = top * 10f64.powf(-d);
            let set_name = self.anchor.map_or("the observable universe", |i| THINGS[i].name);
            let probed = size_of("probed");
            let line = if v < probed {
                // The desert: no experiment reaches here; count Planck lengths.
                let p = &THINGS[thing("planck").expect("planck")];
                format!("This view ≈ {}", compare(v, std::slice::from_ref(p)))
            } else {
                let smaller: Vec<Thing> = THINGS.iter().filter(|t| t.size_m <= top * 1.0001).copied().collect();
                format!("This view ≈ {}", compare(v, &smaller))
            };
            let light = duration(v / C_M_S);
            let physics = if v >= 1e-5 {
                format!("light crosses it in {light}")
            } else if v >= probed {
                format!("to see it you need {}", band(v, HC_EV_M / v))
            } else {
                "Smaller than anything ever measured".to_string()
            };
            return Reading {
                line,
                detail: format!("if the whole set were {set_name}"),
                physics: Some(physics),
            };
        }
        // Past the Planck length: the zoom factor itself.
        let ladders = zoom / ladder_decades();
        let last = MILESTONES.iter().rev().find(|m| m.log10 <= zoom);
        let next = MILESTONES.iter().find(|m| m.log10 > zoom);
        let line = match last {
            Some(m) if next.is_some() => format!("Zoom 1e{:.0}: more than {}", zoom.floor(), m.name),
            Some(_) => format!("≈ {ladders:.1} universe-to-Planck ladders deep"),
            None => format!("Zoom 1e{:.0}: below the Planck length", zoom.floor()),
        };
        let detail = match next {
            Some(m) => format!("next: {} (1e{:.0})", m.name, m.log10.floor()),
            None => format!(
                "1 ladder: observable universe → Planck length, ×1e{:.1}",
                ladder_decades()
            ),
        };
        Reading {
            line,
            detail,
            physics: Some("Smaller than the Planck length: only maths from here".into()),
        }
    }

    /// The screen anchor: the window keeps its width, the whole set grows; up to the universe
    /// in things, then in multiples of it, counted in logs (`n = d - ud`, any depth).
    fn screen_reading(&self, d: f64, ud: f64) -> Reading {
        let detail = format!("your window: {}", length(self.screen_m));
        let n = d - ud;
        let line = if n >= 1.25f64.log10() {
            match count(n) {
                c if c == "a googol" => "The whole set would be a googol × the observable universe".to_string(),
                c => format!("The whole set would be {c} × the observable universe"),
            }
        } else {
            let m = self.screen_m * 10f64.powf(d);
            if m < 1e4 {
                format!("The whole set would be {} wide", length(m))
            } else {
                format!("The whole set would be {}", as_big_as(m))
            }
        };
        Reading {
            line,
            detail,
            physics: None,
        }
    }
}

/// How a thing's size reads in "as … as": a distance is long, a height tall, the universe big.
fn adjective(t: &Thing) -> &'static str {
    match t.key {
        "universe" => "big",
        "andromeda" | "proxima" | "light-year" => "long",
        "everest" | "human" => "tall",
        _ => "wide",
    }
}

/// "as wide as Earth", "3× as long as a light year", "40% as wide as the Sun": `m` metres
/// against the nearest (in log) thing.
fn as_big_as(m: f64) -> String {
    let Some(t) = THINGS
        .iter()
        .min_by(|a, b| (m / a.size_m).log10().abs().total_cmp(&(m / b.size_m).log10().abs()))
    else {
        return length(m);
    };
    let (r, adj) = (m / t.size_m, adjective(t));
    if (0.8..1.25).contains(&r) {
        format!("as {adj} as {}", t.name)
    } else if r >= 1.25 {
        format!("{}× as {adj} as {}", factor(r), t.name)
    } else if r >= 0.1 {
        format!("{:.0}% as {adj} as {}", r * 100.0, t.name)
    } else {
        format!("1/{} as {adj} as {}", factor(1.0 / r), t.name)
    }
}

/// A count given as its log10 `n` (any size, never overflows): "1.5", "200", "45,000",
/// "3.4 million", "12 billion", "2 trillion", then "3e15", "1e16", ..., "a googol" (1e100),
/// "3e2982".
pub fn count(n: f64) -> String {
    if !n.is_finite() {
        return "∞".into();
    }
    if n < 3.0 {
        return factor(10f64.powf(n));
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
    if m == 1.0 && e == 100.0 {
        return "a googol".into();
    }
    format!("{m:.0}e{e:.0}")
}

/// "3× the Milky Way", "the Sun", "1/40 of a proton": `v` metres against the nearest (in
/// log) of `things`.
pub fn compare(v: f64, things: &[Thing]) -> String {
    let Some(t) = things
        .iter()
        .min_by(|a, b| (v / a.size_m).log10().abs().total_cmp(&(v / b.size_m).log10().abs()))
    else {
        return length(v);
    };
    let r = v / t.size_m;
    if (0.8..1.25).contains(&r) {
        t.name.to_string()
    } else if r >= 1.25 {
        format!("{}× {}", factor(r), t.name)
    } else if r >= 0.1 {
        format!("{:.0}% of {}", r * 100.0, t.name)
    } else {
        format!("1/{} of {}", factor(1.0 / r), t.name)
    }
}

/// "2.5", "40", "3,000", "3e9".
fn factor(r: f64) -> String {
    if r < 9.95 {
        let t = format!("{r:.1}");
        t.strip_suffix(".0").map_or(t.clone(), str::to_string)
    } else if r < 9999.5 {
        let n = r.round() as u64;
        if n >= 1000 {
            format!("{},{:03}", n / 1000, n % 1000)
        } else {
            n.to_string()
        }
    } else {
        let e = r.log10().floor();
        let m = r / 10f64.powf(e);
        if m >= 9.5 {
            format!("1e{}", e + 1.0)
        } else if m < 1.5 {
            format!("1e{e}")
        } else {
            format!("{m:.0}e{e}")
        }
    }
}

/// "30 cm", "1.2 m".
fn length(m: f64) -> String {
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

/// Speed of light, m/s (exact; kb: sources/codata-2022-constants).
pub const C_M_S: f64 = 299_792_458.0;

/// Age of the universe, s: 13.787 Gyr (Planck 2018; kb: sources/cosmic-scale-references).
const AGE_UNIVERSE_S: f64 = 13.787e9 * YEAR_S;
/// Age of the Sun, s: about 4.6 Gyr (NASA Sun facts; kb: sources/cosmic-scale-references).
const AGE_SUN_S: f64 = 4.6e9 * YEAR_S;
/// Homo sapiens, s: about 300 000 years (Jebel Irhoud, Hublin et al. 2017).
const AGE_HUMANS_S: f64 = 3.0e5 * YEAR_S;
/// The shortest time interval measured: 247 zs (Grundmann et al. 2020).
const SHORTEST_MEASURED_S: f64 = 247e-21;
/// Planck time, s (CODATA 2022).
const PLANCK_TIME_S: f64 = 5.391_247e-44;
/// Julian year, s.
const YEAR_S: f64 = 365.25 * 86_400.0;

/// "8.5 min", "3 years, longer than humans have existed", "330 zs", "5e-44 s ≈ the Planck time".
/// A duration without its remark ("5e-28 s", not "5e-28 s, shorter than ...").
fn plain(d: &str) -> &str {
    d.split([',', '≈']).next().unwrap_or(d).trim_end()
}

fn duration(t: f64) -> String {
    let num = |x: f64| {
        if x < 9.95 {
            let s = format!("{x:.1}");
            s.strip_suffix(".0").map_or(s.clone(), str::to_string)
        } else {
            format!("{x:.0}")
        }
    };
    if t >= 1.05 * AGE_UNIVERSE_S {
        return format!("{}× the age of the universe", factor(t / AGE_UNIVERSE_S));
    }
    if t >= YEAR_S {
        let y = t / YEAR_S;
        let (x, unit) = if y >= 1e9 {
            (y / 1e9, " billion years")
        } else if y >= 1e6 {
            (y / 1e6, " million years")
        } else if y >= 1e3 {
            (y / 1e3, " thousand years")
        } else {
            (y, if y < 1.95 { " year" } else { " years" })
        };
        let remark = if t >= AGE_SUN_S {
            ", longer than the Sun has shone"
        } else if t >= AGE_HUMANS_S {
            ", longer than humans have existed"
        } else {
            ""
        };
        return format!("{}{unit}{remark}", num(x));
    }
    const UNITS: [(f64, &str); 12] = [
        (86_400.0, " days"),
        (3600.0, " h"),
        (60.0, " min"),
        (1.0, " s"),
        (1e-3, " ms"),
        (1e-6, " μs"), // Greek mu: the overlay font has it, not the micro sign
        (1e-9, " ns"),
        (1e-12, " ps"),
        (1e-15, " fs"),
        (1e-18, " as"),
        (1e-21, " zs"),
        (1e-24, " ys"),
    ];
    let remark = if t < 1.25 * PLANCK_TIME_S {
        " ≈ the Planck time"
    } else if t < SHORTEST_MEASURED_S {
        ", shorter than any time measured"
    } else {
        ""
    };
    if let Some((u, n)) = UNITS.iter().find(|u| t >= u.0) {
        return format!("{}{n}{remark}", num(t / u));
    }
    let mut e = t.log10().floor();
    let mut m = (t / 10f64.powf(e)).round();
    if m >= 10.0 {
        m = 1.0;
        e += 1.0;
    }
    format!("{m:.0}e{e} s{remark}")
}

/// What it takes to see a view `v` metres wide with photons of `ev` (rough band edges, kb:
/// decisions/controls "Cosmic size scale"), in plain words.
fn band(v: f64, ev: f64) -> &'static str {
    if v >= 7.5e-7 {
        "infrared light"
    } else if v >= 3.8e-7 {
        "visible light"
    } else if v >= 1e-8 {
        "ultraviolet light"
    } else if v >= 1e-11 {
        "X-rays"
    } else if ev <= 13.6e12 {
        "gamma rays or particle beams"
    } else {
        "a beam stronger than the LHC"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(anchor: &str) -> CosmicScale {
        CosmicScale::new(
            &CosmicScaleSettings {
                enabled: true,
                anchor: anchor.into(),
                metres_per_point: 0.0,
            },
            1000.0,
            0.0,
        )
    }

    #[test]
    fn table_is_sorted_and_sourced() {
        for w in THINGS.windows(2) {
            assert!(w[0].size_m > w[1].size_m, "{} {}", w[0].key, w[1].key);
        }
        assert!(THINGS.iter().all(|t| !t.source.is_empty()));
        for a in ANCHORS.iter().filter(|a| !["screen", "cosmos"].contains(*a)) {
            assert!(thing(a).is_some(), "{a}");
        }
        assert!((ladder_decades() - 61.74).abs() < 0.01);
    }

    #[test]
    fn screen_anchor_only_grows() {
        let s = scale("screen");
        let line = |zoom: f64| s.reading(zoom).line;
        // The window stays 30 cm: zoom 1e4 makes the set 3 km.
        assert_eq!(line(4.0), "The whole set would be 3 km wide");
        let r = s.reading(4.0);
        assert_eq!(r.detail, "your window: 30 cm");
        assert_eq!(r.physics, None);
        // Earth: 1.27e7 / 0.3 ≈ 4.2e7.
        assert_eq!(
            line((1.2742e7f64 / 0.3).log10()),
            "The whole set would be as wide as Earth"
        );
        assert_eq!(
            line((2.0 * 9.4607e15f64 / 0.3).log10()),
            "The whole set would be 2× as long as a light year"
        );
        let ud = s.universe_d.unwrap();
        assert!((ud - 27.46).abs() < 0.01);
        assert_eq!(s.planck_d, None);
        assert_eq!(s.desert, None);
        // Zoom 3e27: the universe (no turn, no Planck).
        assert_eq!(
            line(3e27f64.log10()),
            "The whole set would be as big as the observable universe"
        );
        let past = |zoom: f64| line(zoom).replace(" × the observable universe", "");
        assert_eq!(past(1e34f64.log10()), "The whole set would be 3.4 million");
        assert_eq!(past(127.0), "The whole set would be 3e99");
        assert_eq!(past(ud + 100.0), "The whole set would be a googol");
        assert_eq!(past(1000.0), "The whole set would be 3e972");
        assert_eq!(past(3010.0), "The whole set would be 3e2982");
        // Every depth to past the limit, finely: never a panic, NaN or Planck, always quick.
        let t0 = std::time::Instant::now();
        for k in -40..=40_000 {
            let r = s.reading(k as f64 * 0.08);
            assert!(r.line.starts_with("The whole set would be"), "{r:?}");
            assert!(!r.line.contains("NaN") && !r.line.contains("inf") && !r.line.contains("Planck"));
            assert_eq!(r.physics, None);
        }
        assert!(t0.elapsed().as_secs_f64() < 5.0);
        // No small things, the multiples to past the limit, marks sorted.
        assert!(
            s.marks
                .iter()
                .all(|m| !["atom", "Planck (strings?)", "limit probed"].contains(&m.label.as_str()))
        );
        assert!(s.marks.iter().any(|m| m.label == "googol ×"));
        assert!(s.marks.last().unwrap().d > 3100.0);
        assert!(s.marks.windows(2).all(|w| w[0].d < w[1].d));
        assert!(s.rungs(0.0, 4000.0).is_empty());
        // Marks never crowd on the ruler: at least 25 ruler points (15 points per ruler decade)
        // apart past the universe.
        let past_universe: Vec<f64> = s.marks.iter().filter(|m| m.d >= ud).map(|m| s.ruler(m.d)).collect();
        for w in past_universe.windows(2) {
            assert!((w[1] - w[0]) * 15.0 >= 25.0, "{w:?}");
        }
        // A measured window says its own size.
        let m = CosmicScale::new(
            &CosmicScaleSettings {
                enabled: true,
                anchor: "screen".into(),
                metres_per_point: 0.0003,
            },
            1500.0,
            0.0,
        );
        assert_eq!(m.reading(4.0).detail, "your window: 45 cm");
    }

    #[test]
    fn counts_read_plainly() {
        assert_eq!(count(0.3), "2");
        assert_eq!(count(2.3), "200");
        assert_eq!(count(3.0), "1,000");
        assert_eq!(count(4.653), "45,000");
        assert_eq!(count(6.3), "2 million");
        assert_eq!(count(6.53), "3.4 million");
        assert_eq!(count(9.48), "3 billion");
        assert_eq!(count(13.0), "10 trillion");
        assert_eq!(count(14.999), "1e15");
        assert_eq!(count(15.48), "3e15");
        assert_eq!(count(99.99), "a googol");
        assert_eq!(count(100.1), "a googol");
        assert_eq!(count(100.5), "3e100");
        assert_eq!(count(2982.5), "3e2982");
        assert_eq!(count(f64::INFINITY), "∞");
    }

    #[test]
    fn past_planck_counts_milestones_then_ladders() {
        let s = scale("universe");
        let r = s.reading(85.0);
        assert!(r.line.contains("atoms in the observable universe"), "{r:?}");
        assert!(r.detail.contains("googol"), "{r:?}");
        assert_eq!(
            r.physics.as_deref(),
            Some("Smaller than the Planck length: only maths from here")
        );
        let r = s.reading(3000.0);
        assert!(r.line.contains("48.6"), "{r:?}");
    }

    #[test]
    fn set_as_earth() {
        let s = scale("earth");
        // Earth / atom ≈ 1.2e17.
        let r = s.reading((1.2742e7f64 / 1.058e-10).log10());
        assert_eq!(r.line, "This view ≈ a hydrogen atom");
        assert_eq!(r.detail, "if the whole set were Earth");
        assert_eq!(r.physics.as_deref(), Some("to see it you need X-rays"));
        // The unexplored band, in plain words.
        let r = s.reading((1.2742e7f64 / 1e-25).log10());
        assert!(r.line.contains("Planck length"), "{r:?}");
        assert_eq!(r.physics.as_deref(), Some("Smaller than anything ever measured"));
        assert!(s.marks.windows(2).all(|w| w[0].d <= w[1].d));
        assert!(s.marks.iter().all(|m| !m.label.contains("GUT")));
        assert_eq!(s.passed(-100.0), None);
    }

    #[test]
    fn relative_mode_travels_back_in_time() {
        let s = scale("cosmos");
        assert!(s.relative);
        let r = s.reading(0.0);
        assert!(r.line.contains("observable universe"), "{r:?}");
        assert_eq!(r.detail, "cosmic time: today");
        let (v, t) = CosmicScale::relative_values(RELATIVE_DEPTH);
        assert!((v / 1.616255e-35 - 1.0).abs() < 1e-9 && (t / 5.391247e-44 - 1.0).abs() < 1e-9);
        // Monotonic down to the Planck length and the Planck epoch.
        let mut last = (f64::MAX, f64::MAX);
        for k in 0..=400 {
            let (v, t) = CosmicScale::relative_values(k as f64 * 0.5);
            assert!(v <= last.0 && t <= last.1);
            last = (v, t);
        }
        // Even pacing: every step 200/23 decades, 12.6 s at ×4.95/s.
        let step = RELATIVE_DEPTH / (THINGS.len() - 1) as f64;
        assert!((step / 4.95f64.log10() - 12.6).abs() < 0.2);
        // Size and time stay together: the CMB at a cell, nuclei at an atom.
        assert!(s.reading(rel_z(15.0)).detail.contains("CMB"));
        assert!(s.reading(rel_z(19.0)).detail.contains("nuclei"));
        assert!(s.reading(rel_z(19.0)).line.contains("hydrogen atom"));
        for i in [10, 50, 100, 200, 500] {
            eprintln!("relative 1e{i}: {:?}", s.reading(i as f64));
        }
        assert!(s.reading(500.0).line.contains("2.5 cosmos lengths"));
        let (b0, b1) = s.desert.unwrap();
        assert!(b0 < b1 && b1 <= RELATIVE_DEPTH + 1e-9);
        assert!(!s.rungs(450.0, 520.0).is_empty());
    }

    #[test]
    fn phrases() {
        let t = &THINGS[thing("milky-way").unwrap()];
        assert_eq!(compare(3.0 * t.size_m, std::slice::from_ref(t)), "3× the Milky Way");
        assert_eq!(
            compare(t.size_m / 40.0, std::slice::from_ref(t)),
            "1/40 of the Milky Way"
        );
        assert_eq!(compare(t.size_m * 0.3, std::slice::from_ref(t)), "30% of the Milky Way");
        assert_eq!(compare(t.size_m * 3e9, std::slice::from_ref(t)), "3e9× the Milky Way");
        assert_eq!(duration(500.0), "8.3 min");
        assert_eq!(duration(3.0 * YEAR_S), "3 years");
        assert_eq!(
            duration(5e5 * YEAR_S),
            "500 thousand years, longer than humans have existed"
        );
        assert_eq!(duration(3e-19), "300 zs");
        assert_eq!(duration(1e-20), "10 zs, shorter than any time measured");
        assert_eq!(duration(5.4e-44), "5e-44 s ≈ the Planck time");
        assert_eq!(duration(3.0 * AGE_UNIVERSE_S), "3× the age of the universe");
    }
}
