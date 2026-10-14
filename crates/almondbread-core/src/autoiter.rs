//! Statistics-driven iteration control (kb: techniques/auto-iterations) and the adaptive
//! interior-test switch (kb: techniques/interior-detection). Pure functions of a completed
//! level's counters, so every backend's scheduler makes the same decisions.

/// Fraction of pixels allowed to escape beyond the learned limit.
pub const UNRESOLVED_TOLERANCE: f64 = 0.01;

/// The derivative test costs ~50 % more per iteration; it pays off when
/// `interior fraction × maxIter > INTERIOR_TEST_OVERHEAD × exterior iterations per pixel`.
pub const INTERIOR_TEST_OVERHEAD: f64 = 0.6;

/// Counters of a completed full-resolution level (and its extension passes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LevelOutcome {
    /// Nominal limit of the level.
    pub soft_iter: usize,
    /// Texels of the level.
    pub texels: usize,
    /// Pixels that reached the (last pass's) limit unresolved.
    pub crossed: usize,
    /// Largest escape iteration seen.
    pub max_escape: usize,
    /// Escapes at n >= soft_iter << k, k = 0..3.
    pub escaped_past: [usize; 4],
}

/// The learned iteration floor after a completed view: the smallest of 2x or 4x the limit
/// that leaves fewer than 1 % of the pixels escaping beyond it, if at least 1 % escaped past
/// the limit; a slow decay (x0.7) when nothing came near the limit; capped at the iteration
/// cap and at 4x the depth heuristic.
pub fn learn_iterations(old: usize, level: &LevelOutcome, cap: usize, depth_iterations: usize) -> usize {
    let soft = level.soft_iter;
    let mut learned = old;
    let tolerance = UNRESOLVED_TOLERANCE * level.texels as f64;
    if level.escaped_past[0] as f64 >= tolerance {
        let mut k = 1;
        while k < 2 && level.escaped_past[k] as f64 >= tolerance {
            k += 1;
        }
        learned = old.max(soft << k);
    } else if level.crossed == 0 && level.max_escape < soft / 4 {
        learned = (old as f64 * 0.7) as usize;
    }
    learned.min(cap).min(4 * depth_iterations)
}

/// Interior fraction and exterior iterations per pixel of the last completed level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InteriorStats {
    pub fraction: f64,
    pub exterior_iterations: f64,
}

impl InteriorStats {
    /// From a completed level: without the test, interior pixels ran to maxIter; take them out
    /// of the exterior count.
    pub fn from_level(total: usize, interior_pixels: usize, iterations: f64, used_test: bool, max_iter: usize) -> Self {
        let n = total as f64;
        let mut exterior = iterations;
        if !used_test {
            exterior -= interior_pixels as f64 * max_iter as f64;
        }
        Self {
            fraction: interior_pixels as f64 / n,
            exterior_iterations: exterior.max(0.0) / n,
        }
    }
}

/// Whether the next level runs the derivative interior test (GBy's advice: only when a
/// minibrot or the main set is in view). Unknown statistics: on (safe at high maxIter).
pub fn use_interior_test(enabled: bool, stats: Option<InteriorStats>, max_iter: usize) -> bool {
    if !enabled {
        return false;
    }
    match stats {
        None => true,
        Some(s) => s.fraction * max_iter as f64 > INTERIOR_TEST_OVERHEAD * s.exterior_iterations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning() {
        let mut l = LevelOutcome {
            soft_iter: 2000,
            texels: 10_000,
            crossed: 500,
            max_escape: 7000,
            escaped_past: [300, 150, 50, 0],
        };
        assert_eq!(learn_iterations(0, &l, 200_000, 5000), 8000);
        l.escaped_past = [300, 50, 0, 0];
        assert_eq!(learn_iterations(0, &l, 200_000, 5000), 4000);
        // capped at 4x depth
        assert_eq!(learn_iterations(0, &l, 200_000, 500), 2000);
        // decay
        let quiet = LevelOutcome {
            soft_iter: 2000,
            texels: 10_000,
            crossed: 0,
            max_escape: 100,
            escaped_past: [0; 4],
        };
        assert_eq!(learn_iterations(1000, &quiet, 200_000, 5000), 700);
    }

    #[test]
    fn interior_switch() {
        assert!(use_interior_test(true, None, 1000));
        assert!(!use_interior_test(false, None, 1000));
        let s = InteriorStats::from_level(100, 0, 5000.0, true, 1000);
        assert!(!use_interior_test(true, Some(s), 1000));
        let s = InteriorStats::from_level(100, 30, 30.0 * 1000.0 + 70.0 * 50.0, false, 1000);
        assert!(use_interior_test(true, Some(s), 1000));
    }
}
