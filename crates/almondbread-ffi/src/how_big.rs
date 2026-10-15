//! "How big is big?" panel: arrow-level dials and big-number markers from the core
//! (`almondbread_core::bignum_scale`, kb: synthesis/how-big-is-big). The panel calls
//! `zoom_dials` with the shown zoom at the stats rate (cheap: a few logs).

use almondbread_core::bignum_scale as bs;

/// A big number and where it sits on each dial (index 0 = dial ↑). Infinite: beyond that
/// dial's reach (and f64); the panel shows it as off the scale.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BigNumberInfo {
    pub id: String,
    pub name: String,
    pub notation: String,
    pub note: String,
    pub dials: Vec<f64>,
}

/// Facts for the panel at a zoom and zoom speed.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BigNumberFactsInfo {
    pub zoom_log10: f64,
    pub speed_factor: f64,
    pub seconds_to_googol: f64,
    pub seconds_to_limit: f64,
    pub years_to_googolplex: f64,
    pub bits_here: f64,
    pub bits_googolplex: f64,
    pub atoms_in_universe: f64,
    pub ganson_years: f64,
}

/// Dials ↑ … ↑^levels for a zoom of 10^zoom_log10 (linear-approximation arrow heights).
#[uniffi::export]
pub fn zoom_dials(zoom_log10: f64, levels: u32) -> Vec<f64> {
    bs::zoom_dials(zoom_log10, levels.clamp(1, 12))
}

/// The panel's big numbers, smallest first.
#[uniffi::export]
pub fn big_number_markers(levels: u32) -> Vec<BigNumberInfo> {
    bs::big_number_markers(levels.clamp(1, 12))
        .into_iter()
        .map(|m| BigNumberInfo {
            id: m.id.into(),
            name: m.name.into(),
            notation: m.notation.into(),
            note: m.note.into(),
            dials: m.dials,
        })
        .collect()
}

/// log10 of the deepest zoom (≈ 3008).
#[uniffi::export]
pub fn max_zoom_log10() -> f64 {
    bs::max_zoom_log10()
}

/// `zoom_speed_ln`: ln of the magnification per second (the stats' `zoom_speed`).
#[uniffi::export]
pub fn big_number_facts(zoom_log10: f64, zoom_speed_ln: f64) -> BigNumberFactsInfo {
    let f = bs::big_number_facts(zoom_log10, zoom_speed_ln.max(1e-3));
    BigNumberFactsInfo {
        zoom_log10: f.zoom_log10,
        speed_factor: f.speed_factor,
        seconds_to_googol: f.seconds_to_googol,
        seconds_to_limit: f.seconds_to_limit,
        years_to_googolplex: f.years_to_googolplex,
        bits_here: f.bits_here,
        bits_googolplex: f.bits_googolplex,
        atoms_in_universe: f.atoms_in_universe,
        ganson_years: f.ganson_years,
    }
}
