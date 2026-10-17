//! Places: a curated catalogue of well-known, beautiful views of the Mandelbrot set to visit
//! (kb: synthesis/places). Each has a centre as decimal strings of any precision, a span, a
//! recommended look and a citation for where the coordinate comes from. The UIs list them
//! (with the thumbnails `almondbread places --render-thumbs` writes), jump or fly to them,
//! and pick a random one; the landscape prototype can start from the shallow ones.
//!
//! Coordinates are facts; the descriptions are our own words. A place marked `generated` was
//! computed here (`almondbread locate`: Newton nuclei and Misiurewicz points at full
//! precision) from a cited starting point, or is our own find.

use crate::viewport::{Center, Scale, Viewport};

/// What kind of place it is (menu sections, in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlaceCategory {
    /// The whole set, bulbs, cusp, antenna: the large-scale geography.
    Landmark,
    /// The cusps between components, where seahorses, elephants and scepters grow.
    Valley,
    /// Spirals: centres of spiral arms (Misiurewicz points) and spiral wheels.
    Spiral,
    /// Tips and branch points of the filaments (Misiurewicz points).
    Dendrite,
    /// Small copies of the whole set and their surroundings.
    Minibrot,
    /// Embedded Julia sets: islands surrounded by Julia-set-shaped decorations.
    JuliaIsland,
    /// Showcases beyond f64 (1e-30 and deeper): only arbitrary precision gets here.
    Deep,
    /// Centred exactly on a Misiurewicz point (a boundary point), so a straight zoom never
    /// ends in a featureless area down to the depth limit (kb: synthesis/places "Endless dives").
    EndlessDive,
}

impl PlaceCategory {
    pub const ALL: [PlaceCategory; 8] = [
        PlaceCategory::Landmark,
        PlaceCategory::Valley,
        PlaceCategory::Spiral,
        PlaceCategory::Dendrite,
        PlaceCategory::Minibrot,
        PlaceCategory::JuliaIsland,
        PlaceCategory::Deep,
        PlaceCategory::EndlessDive,
    ];

    pub fn id(self) -> &'static str {
        match self {
            PlaceCategory::Landmark => "landmark",
            PlaceCategory::Valley => "valley",
            PlaceCategory::Spiral => "spiral",
            PlaceCategory::Dendrite => "dendrite",
            PlaceCategory::Minibrot => "minibrot",
            PlaceCategory::JuliaIsland => "julia-island",
            PlaceCategory::Deep => "deep",
            PlaceCategory::EndlessDive => "endless-dive",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PlaceCategory::Landmark => "Landmarks",
            PlaceCategory::Valley => "Valleys",
            PlaceCategory::Spiral => "Spirals",
            PlaceCategory::Dendrite => "Dendrites",
            PlaceCategory::Minibrot => "Minibrots",
            PlaceCategory::JuliaIsland => "Julia islands",
            PlaceCategory::Deep => "Deep showcases",
            PlaceCategory::EndlessDive => "Endless dives",
        }
    }
}

/// One place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// Stable id (kebab-case): thumbnail file names, links, `--place`.
    pub id: &'static str,
    pub name: &'static str,
    pub category: PlaceCategory,
    /// One or two sentences: why go there.
    pub description: &'static str,
    /// One line: what the view shows, as a fact (Misiurewicz point, period-n minibrot, ...).
    pub feature: &'static str,
    /// Centre, decimal strings (any number of digits).
    pub re: &'static str,
    pub im: &'static str,
    /// Complex extent across the view's shorter side (any exponent: "1e-300"), so the
    /// feature fits in portrait and landscape windows alike.
    pub span: &'static str,
    /// Recommended look (`palette::Look::named`).
    pub look: &'static str,
    /// Iteration limit, or `None` for the automatic limit.
    pub iterations: Option<usize>,
    /// Where the coordinate comes from.
    pub source: &'static str,
    pub source_url: &'static str,
    /// Computed or found by us (see `source` for the starting point).
    pub generated: bool,
}

impl Place {
    pub fn center(&self) -> Center {
        Center::parse(self.re, self.im).expect("place centre")
    }

    pub fn span_scale(&self) -> Scale {
        Scale::parse(self.span).expect("place span")
    }

    /// Complex units per pixel for a `width` x `height` view.
    pub fn scale_for(&self, width: usize, height: usize) -> Scale {
        self.span_scale() / width.min(height).max(1) as f64
    }

    pub fn viewport(&self, width: usize, height: usize) -> Viewport {
        let mut v = Viewport::with_center(self.center(), self.scale_for(width, height));
        v.ensure_precision();
        v
    }

    /// log10 of the magnification relative to the whole set (span 3).
    pub fn zoom_log10(&self) -> f64 {
        (3.0f64).log10() - self.span_scale().log10()
    }

    /// The start scale S for the landscape prototype (`prototypes/landscape-rt`, `--scale`):
    /// it starts 2 S off the boundary with fog at 60 S, so S = span / 30 shows about the
    /// place's extent. `None` beyond f64 (the prototype iterates in f64).
    pub fn landscape_scale(&self) -> Option<f64> {
        let s = self.span_scale().to_f64() / 30.0;
        (s > 1e-12).then_some(s)
    }
}

/// Every place, in menu order (by category, then shallow to deep).
pub fn places() -> &'static [Place] {
    PLACES
}

pub fn place_named(id: &str) -> Option<&'static Place> {
    PLACES.iter().find(|p| p.id == id)
}

/// A random place: `seed` picks it (any u64, e.g. a time or a random number), so the caller
/// owns the randomness and tests stay deterministic. Optionally limited to a category.
pub fn random_place(seed: u64, category: Option<PlaceCategory>) -> &'static Place {
    let pool: Vec<&'static Place> = PLACES
        .iter()
        .filter(|p| category.is_none_or(|c| p.category == c))
        .collect();
    let pool = if pool.is_empty() { PLACES.iter().collect() } else { pool };
    // SplitMix64 finaliser: neighbouring seeds pick unrelated places.
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    pool[(x % pool.len() as u64) as usize]
}

const BEYER_URL: &str = "https://commons.wikimedia.org/wiki/Category:Mandelbrot_zoom_sequence_by_Wolfgang_Beyer";
const WIKI_MIS: &str = "https://en.wikipedia.org/wiki/Misiurewicz_point";

include!("places_data.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Look;
    use std::collections::HashSet;

    #[test]
    fn ids_unique_and_kebab() {
        let mut seen = HashSet::new();
        for p in PLACES {
            assert!(seen.insert(p.id), "duplicate id {}", p.id);
            assert!(
                p.id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "id {}",
                p.id
            );
            assert!(!p.name.is_empty() && !p.description.is_empty() && !p.feature.is_empty());
            assert!(!p.source.is_empty(), "{} has no source", p.id);
        }
        assert!(PLACES.len() >= 40);
    }

    #[test]
    fn coordinates_parse_and_are_in_range() {
        for p in PLACES {
            let c = Center::parse(p.re, p.im).unwrap_or_else(|| panic!("{}: centre", p.id));
            assert!(
                c.re() * c.re() + c.im() * c.im() <= 4.0 + 1e-9,
                "{} outside |c| <= 2",
                p.id
            );
            let s = Scale::parse(p.span).unwrap_or_else(|| panic!("{}: span", p.id));
            assert!(s.log10() <= 1.0, "{}: span {}", p.id, p.span);
            // Reachable: in a window up to 1000 points on its shorter side the place's scale stays
            // at or above the deepest scale zooming allows (`go_to_place` clamps larger windows).
            assert!(
                p.scale_for(1000, 1000) >= Scale::pow2(crate::viewport::MIN_SCALE_LOG2),
                "{}: span {} beyond the depth limit",
                p.id,
                p.span
            );
            // The centre carries enough digits for its depth (else it lands somewhere else).
            let digits =
                p.re.trim_start_matches('-')
                    .len()
                    .max(p.im.trim_start_matches('-').len());
            assert!(
                (digits as f64) >= -s.log10() - 1.0 || (p.re.len() <= 4 && p.im.len() <= 4),
                "{}: {} digits for span {}",
                p.id,
                digits,
                p.span
            );
            let v = p.viewport(480, 300);
            assert!(v.scale.log10() < s.log10());
            assert!(Look::named(p.look).is_some(), "{}: unknown look {}", p.id, p.look);
        }
    }

    #[test]
    fn categories_and_depths_covered() {
        for c in PlaceCategory::ALL {
            assert!(PLACES.iter().any(|p| p.category == c), "no place in {c:?}");
        }
        let deepest = PLACES.iter().map(|p| p.zoom_log10()).fold(0.0, f64::max);
        // Rock Bottom sits at the depth limit (kb: synthesis/places).
        assert!(deepest > 3000.0);
        assert!(PLACES.iter().any(|p| p.zoom_log10() < 1.0));
        // Grouped by category in menu order.
        let order: Vec<usize> = PLACES
            .iter()
            .map(|p| PlaceCategory::ALL.iter().position(|&c| c == p.category).unwrap())
            .collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]), "places not grouped by category");
    }

    #[test]
    fn endless_dives_are_exact_to_the_depth_limit() {
        // Centred on the point itself to more digits than the depth limit (5e-3011 per
        // point) needs, or exact (c = i).
        let dives: Vec<_> = PLACES
            .iter()
            .filter(|p| p.category == PlaceCategory::EndlessDive)
            .collect();
        assert!(dives.len() >= 4);
        for p in dives {
            let digits = p.re.len().min(p.im.len());
            assert!(
                digits >= 3100 || (p.re.len() <= 4 && p.im.len() <= 4),
                "{}: {digits} digits",
                p.id
            );
        }
    }

    #[test]
    fn random_place_is_deterministic_and_spread() {
        assert_eq!(random_place(7, None).id, random_place(7, None).id);
        let picked: HashSet<&str> = (0..500).map(|s| random_place(s, None).id).collect();
        assert!(picked.len() > PLACES.len() / 2);
        for s in 0..50 {
            assert_eq!(random_place(s, Some(PlaceCategory::Deep)).category, PlaceCategory::Deep);
        }
        assert!(place_named("seahorse-valley").is_some());
        assert!(place_named("nowhere").is_none());
    }

    #[test]
    fn landscape_scale_only_within_f64() {
        let shallow = place_named("seahorse-valley").unwrap();
        assert!(shallow.landscape_scale().is_some());
        assert!(PLACES.iter().any(|p| p.landscape_scale().is_none()));
    }
}
