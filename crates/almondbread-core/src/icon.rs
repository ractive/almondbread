//! Icons every UI shares (kb: decisions/controls "Guided tours", "Driving levels"): the
//! cockpit's glyphs as strokes, and the inline icons a tour's text can show
//! (`{icon:cockpit-manual}`).
//!
//! The cockpit draws its icons from [`Glyph::strokes`] (render `cockpit.rs`), and the UIs draw
//! the same strokes into the small images a tour card shows in its text, so the card shows what
//! the cockpit shows. Icons of controls the platform UI draws itself (the round buttons, the
//! Mac toolbar) are SF Symbols; their names live in [`INLINE_ICONS`] too, and the Swift UI takes
//! them from there.

use crate::cruise::Level;

/// One stroke of a glyph, in design points from the glyph's centre (y down). A glyph is drawn
/// in one colour; it fits a square of half side [`Glyph::HALF_EXTENT`].
#[derive(Clone, Debug, PartialEq)]
pub enum Stroke {
    /// A polyline with round caps and joins, `half_width` either side of its centre line.
    Line { points: Vec<[f64; 2]>, half_width: f64 },
    /// A circle outline: centre line at `radius`, `width` across.
    Ring { center: [f64; 2], radius: f64, width: f64 },
    /// A filled circle.
    Disc { center: [f64; 2], radius: f64 },
}

/// A glyph drawn from strokes: the cockpit's four driving levels and its lane-assist icon, and a
/// small lever standing for the touch throttle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// A steering wheel: rim, hub and three spokes.
    Manual,
    /// A speedometer: an open arc and a needle.
    Cruise,
    /// A plane, nose up: fuselage, swept wings, tail.
    Autopilot,
    /// Sparkles: a four-pointed star and a small one.
    Journey,
    /// Lane assist: two lane lines converging upwards, a dashed centre line.
    Lane,
    /// The throttle: a lever's slot with the middle notch and the knob above it.
    Throttle,
}

/// Level icons' line half width (points).
const LINE: f64 = 0.55;

impl Glyph {
    /// Half the side of the square every glyph fits, line widths included (points).
    pub const HALF_EXTENT: f64 = 7.0;

    /// The cockpit's icon of a driving level.
    pub fn of(level: Level) -> Self {
        match level {
            Level::Manual => Self::Manual,
            Level::Cruise => Self::Cruise,
            Level::Autopilot => Self::Autopilot,
            Level::Journey => Self::Journey,
        }
    }

    /// The glyph's strokes, in the order they are drawn.
    pub fn strokes(self) -> Vec<Stroke> {
        let line = |pts: &[(f64, f64)]| Stroke::Line {
            points: pts.iter().map(|&(x, y)| [x, y]).collect(),
            half_width: LINE,
        };
        match self {
            Self::Manual => vec![
                Stroke::Ring {
                    center: [0.0, 0.0],
                    radius: 4.6,
                    width: 1.0,
                },
                Stroke::Disc {
                    center: [0.0, 0.0],
                    radius: 1.3,
                },
                line(&[(-4.3, -0.3), (0.0, 0.0), (4.3, -0.3)]),
                line(&[(0.0, 0.0), (0.0, 4.3)]),
            ],
            Self::Cruise => {
                let arc: Vec<(f64, f64)> = (0..=12)
                    .map(|k| {
                        let a = (200.0 - 220.0 * f64::from(k) / 12.0).to_radians();
                        (4.8 * a.cos(), 1.2 - 4.8 * a.sin())
                    })
                    .collect();
                vec![
                    line(&arc),
                    line(&[(0.0, 1.2), (2.6, -1.8)]),
                    Stroke::Disc {
                        center: [0.0, 1.2],
                        radius: 0.9,
                    },
                ]
            }
            Self::Autopilot => vec![
                line(&[(0.0, -5.0), (0.0, 4.6)]),
                line(&[(-5.0, 1.4), (0.0, -1.2), (5.0, 1.4)]),
                line(&[(-2.0, 4.8), (0.0, 3.6), (2.0, 4.8)]),
            ],
            Self::Journey => vec![
                line(&[(-1.5, -5.0), (-1.5, 4.0)]),
                line(&[(-5.6, -0.5), (2.6, -0.5)]),
                line(&[(3.6, -5.0), (3.6, -2.0)]),
                line(&[(2.1, -3.5), (5.1, -3.5)]),
            ],
            Self::Lane => {
                let rail = |s: f64| Stroke::Line {
                    points: vec![[s * 5.5, 6.0], [s * 2.5, -6.0]],
                    half_width: 0.7,
                };
                let dash = |a: f64, b: f64| Stroke::Line {
                    points: vec![[0.0, a], [0.0, b]],
                    half_width: 0.5,
                };
                vec![rail(-1.0), rail(1.0), dash(4.0, 1.5), dash(-1.0, -3.5)]
            }
            Self::Throttle => vec![
                // The slot (a capsule's outline), the middle notch, the knob above it.
                Stroke::Line {
                    points: vec![[-1.6, -4.4], [-1.6, 4.4]],
                    half_width: 0.5,
                },
                Stroke::Line {
                    points: vec![[1.6, -4.4], [1.6, 4.4]],
                    half_width: 0.5,
                },
                Stroke::Ring {
                    center: [0.0, -4.4],
                    radius: 1.6,
                    width: 1.0,
                },
                Stroke::Ring {
                    center: [0.0, 4.4],
                    radius: 1.6,
                    width: 1.0,
                },
                Stroke::Line {
                    points: vec![[-0.8, 0.0], [0.8, 0.0]],
                    half_width: 0.4,
                },
                Stroke::Line {
                    points: vec![[-3.6, -2.4], [3.6, -2.4]],
                    half_width: 1.1,
                },
            ],
        }
    }
}

/// What an inline icon shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconArt {
    /// A glyph drawn from strokes (the cockpit's own).
    Glyph(Glyph),
    /// An SF Symbol, by name: the same symbol the platform UI shows on that control.
    Symbol(&'static str),
}

/// An icon a tour's text can show inline (`{icon:<id>}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineIcon {
    /// The id in the text; the same name as the control's `highlight:` target.
    pub id: &'static str,
    /// What a screen reader says for it: what it looks like (alt text), since the text around
    /// it names the control.
    pub label: &'static str,
    pub art: IconArt,
}

/// The inline icons, by id.
pub const INLINE_ICONS: &[InlineIcon] = &[
    InlineIcon {
        id: "cockpit-manual",
        label: "steering wheel",
        art: IconArt::Glyph(Glyph::Manual),
    },
    InlineIcon {
        id: "cockpit-cruise",
        label: "speedometer",
        art: IconArt::Glyph(Glyph::Cruise),
    },
    InlineIcon {
        id: "cockpit-autopilot",
        label: "plane",
        art: IconArt::Glyph(Glyph::Autopilot),
    },
    InlineIcon {
        id: "cockpit-journey",
        label: "sparkles",
        art: IconArt::Glyph(Glyph::Journey),
    },
    InlineIcon {
        id: "cockpit-lane",
        label: "lane lines",
        art: IconArt::Glyph(Glyph::Lane),
    },
    InlineIcon {
        id: "throttle",
        label: "lever",
        art: IconArt::Glyph(Glyph::Throttle),
    },
    InlineIcon {
        id: "orbit",
        label: "curved path",
        art: IconArt::Symbol("point.topleft.down.to.point.bottomright.curvepath"),
    },
    InlineIcon {
        id: "mini-map",
        label: "map",
        art: IconArt::Symbol("map"),
    },
    InlineIcon {
        id: "julia",
        label: "seal",
        art: IconArt::Symbol("seal"),
    },
    InlineIcon {
        id: "size-scale",
        label: "ruler",
        art: IconArt::Symbol("ruler"),
    },
    InlineIcon {
        id: "fly-to",
        label: "arrow",
        art: IconArt::Symbol("location.north.line"),
    },
    InlineIcon {
        id: "control-center",
        label: "sliders",
        art: IconArt::Symbol("slider.horizontal.3"),
    },
];

/// The inline icon `id`.
pub fn inline_icon(id: &str) -> Option<&'static InlineIcon> {
    INLINE_ICONS.iter().find(|i| i.id == id)
}

/// Opens an inline icon in a text.
pub const ICON_OPEN: &str = "{icon:";

/// The inline icons of one line of text, in order: each one's byte offset and id, or the
/// offset and what is wrong (`{icon:` not closed on the line, an unknown id).
pub fn icons_in(line: &str) -> Vec<(usize, Result<&str, String>)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(k) = line[from..].find(ICON_OPEN) {
        let at = from + k;
        let rest = &line[at + ICON_OPEN.len()..];
        let Some(end) = rest.find('}') else {
            out.push((at, Err(format!("`{ICON_OPEN}` is not closed with `}}` on its line"))));
            break;
        };
        let id = rest[..end].trim();
        if inline_icon(id).is_some() {
            out.push((at, Ok(id)));
        } else {
            let known: Vec<&str> = INLINE_ICONS.iter().map(|i| i.id).collect();
            out.push((at, Err(format!("unknown icon `{id}` (known: {})", known.join(", ")))));
        }
        from = at + ICON_OPEN.len() + end + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(s: &Stroke) -> f64 {
        match s {
            Stroke::Line { points, half_width } => points
                .iter()
                .map(|p| p[0].abs().max(p[1].abs()) + half_width)
                .fold(0.0, f64::max),
            Stroke::Ring { center, radius, width } => center[0].abs().max(center[1].abs()) + radius + width / 2.0,
            Stroke::Disc { center, radius } => center[0].abs().max(center[1].abs()) + radius,
        }
    }

    #[test]
    fn glyphs_fit_their_square() {
        for g in [
            Glyph::Manual,
            Glyph::Cruise,
            Glyph::Autopilot,
            Glyph::Journey,
            Glyph::Lane,
            Glyph::Throttle,
        ] {
            let s = g.strokes();
            assert!(!s.is_empty());
            for st in &s {
                assert!(extent(st) <= Glyph::HALF_EXTENT, "{g:?} {st:?}");
            }
        }
    }

    #[test]
    fn ids_are_unique_and_levels_have_icons() {
        for (i, a) in INLINE_ICONS.iter().enumerate() {
            assert!(INLINE_ICONS[i + 1..].iter().all(|b| b.id != a.id), "{} twice", a.id);
            assert!(!a.label.is_empty());
        }
        for l in Level::ALL {
            let id = format!("cockpit-{}", format!("{l:?}").to_lowercase());
            assert_eq!(inline_icon(&id).map(|i| i.art), Some(IconArt::Glyph(Glyph::of(l))));
        }
    }

    #[test]
    fn finds_icons_and_reports_bad_ones() {
        let v = icons_in("a {icon:orbit} b {icon: mini-map } c");
        assert_eq!(v, vec![(2, Ok("orbit")), (17, Ok("mini-map"))]);
        let v = icons_in("{icon:nope} {icon:orbit");
        assert!(matches!(&v[0], (0, Err(e)) if e.contains("unknown icon `nope`")));
        assert!(matches!(&v[1], (12, Err(e)) if e.contains("not closed")));
        assert!(icons_in("no icons {here}").is_empty());
    }
}
