//! Density edits pivot on the view's own escape counts (kb: techniques/coloring-methods,
//! "Density pivot").
//!
//! The colour pass maps a smooth count mu to the palette position `t = mu * density + offset`
//! (linear) or `t = ln(mu + 1) * density + offset` (log scale). Density stretches the palette
//! around mu = 0, which at depth is nowhere in view: with escape counts of 1e4 to 1e6, a 3 %
//! density change (one point of slider travel) moves every pixel by hundreds of palette
//! cycles, differently for neighbouring counts, and the picture scrambles. A user edit of the
//! density therefore moves the offset with it so that the colour at a reference count n_ref
//! (the median escape count of the shown view) stays: bands widen or narrow around the main
//! colour, like zooming the gradient. Only user edits of the density control pivot; looks,
//! places, links and resets set density and offset as stored.

/// Fewest escaped samples a pivot is taken from (fewer: the view is nearly all interior or
/// unresolved, no pivot).
pub const MIN_ESCAPED_SAMPLES: usize = 16;

/// The pivot of a sample of the view's smooth escape counts: the median of the escaped
/// ones (finite, >= 0; interior, pending and unresolved texels carry negative sentinels).
/// None without enough escaped samples. Reorders `samples`.
pub fn escape_pivot(samples: &mut [f32]) -> Option<f64> {
    let mut n = 0;
    for i in 0..samples.len() {
        let v = samples[i];
        if v.is_finite() && v >= 0.0 {
            samples.swap(n, i);
            n += 1;
        }
    }
    if n < MIN_ESCAPED_SAMPLES {
        return None;
    }
    let escaped = &mut samples[..n];
    let mid = n / 2;
    let (_, m, _) = escaped.select_nth_unstable_by(mid, f32::total_cmp);
    Some(f64::from(*m))
}

/// The colour offset after a user changes the density from `from` to `to`, so that the
/// colour at the smooth count `pivot` stays: linear `offset + pivot (from - to)`, log scale
/// `offset + ln(pivot + 1) (from - to)`, wrapped into [0, 1). Without a pivot (no exterior
/// pixels, no data yet) or for invalid input the offset is unchanged.
pub fn pivoted_offset(offset: f64, from: f64, to: f64, pivot: Option<f64>, log_scale: bool) -> f64 {
    let Some(n) = pivot.filter(|n| n.is_finite() && *n >= 0.0) else {
        return offset;
    };
    if !(offset.is_finite() && from.is_finite() && to.is_finite()) || from == to {
        return offset;
    }
    let x = if log_scale { n.ln_1p() } else { n };
    let shifted = (offset + x * (from - to)).rem_euclid(1.0);
    // rem_euclid can round up to exactly 1.0 for tiny negative values.
    if shifted >= 1.0 { 0.0 } else { shifted }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Palette position as the colour pass computes it (fract), in f64.
    fn t(mu: f64, density: f64, offset: f64, log: bool) -> f64 {
        let x = if log { mu.ln_1p() } else { mu };
        (x * density + offset).rem_euclid(1.0)
    }

    fn same_phase(a: f64, b: f64) -> bool {
        let d = (a - b).rem_euclid(1.0);
        d.min(1.0 - d) < 1e-9
    }

    #[test]
    fn linear_keeps_the_pivot_colour() {
        let (n, d1, d2, o1) = (123_456.7, 0.02, 0.0206, 0.3);
        let o2 = pivoted_offset(o1, d1, d2, Some(n), false);
        assert!((0.0..1.0).contains(&o2));
        assert!(same_phase(t(n, d1, o1, false), t(n, d2, o2, false)));
        // A pixel 100 counts away moves by 100 * (d2 - d1) cycles.
        let moved = (t(n + 100.0, d2, o2, false) - t(n + 100.0, d1, o1, false)).rem_euclid(1.0);
        assert!(same_phase(moved, 100.0 * (d2 - d1)));
    }

    #[test]
    fn log_scale_pivots_on_log_count() {
        let (n, d1, d2, o1) = (5.0e5, 0.4, 0.43, 0.9);
        let o2 = pivoted_offset(o1, d1, d2, Some(n), true);
        assert!(same_phase(t(n, d1, o1, true), t(n, d2, o2, true)));
        assert!(!same_phase(o2, pivoted_offset(o1, d1, d2, Some(n), false)));
    }

    #[test]
    fn wraps_into_unit_range() {
        // A large shift either way stays in [0, 1).
        for (d1, d2) in [(0.02, 0.5), (0.5, 0.02), (0.001, 1.0)] {
            let o = pivoted_offset(0.95, d1, d2, Some(1.0e6), false);
            assert!((0.0..1.0).contains(&o), "{o}");
        }
        assert!(same_phase(pivoted_offset(0.25, 0.1, 0.35, Some(1.0), false), 0.0));
        let o = pivoted_offset(0.1, 0.0, 0.4, Some(1.0), false);
        assert!((o - 0.7).abs() < 1e-12, "{o}");
    }

    #[test]
    fn no_pivot_no_change() {
        assert_eq!(pivoted_offset(0.4, 0.02, 0.03, None, false), 0.4);
        assert_eq!(pivoted_offset(0.4, 0.02, 0.03, Some(f64::NAN), true), 0.4);
        assert_eq!(pivoted_offset(0.4, 0.02, 0.02, Some(1e5), false), 0.4);
        assert_eq!(pivoted_offset(0.4, f64::INFINITY, 0.02, Some(1e5), false), 0.4);
    }

    #[test]
    fn pivot_is_the_median_of_escaped_samples() {
        // All interior / pending / unresolved: none.
        let mut inside = vec![-1.0f32; 400];
        inside.extend([-2.0, -3.0, -4.0, f32::NAN]);
        assert_eq!(escape_pivot(&mut inside), None);
        // Too few escaped samples: none.
        let mut few = vec![-1.0f32; 100];
        few.extend([10.0; MIN_ESCAPED_SAMPLES - 1]);
        assert_eq!(escape_pivot(&mut few), None);
        // Interior samples do not count towards the median.
        let mut mixed: Vec<f32> = (0..101).map(|k| 1000.0 + k as f32).collect();
        mixed.extend(vec![-1.0f32; 500]);
        mixed.reverse();
        assert_eq!(escape_pivot(&mut mixed), Some(1050.0));
    }
}
