//! Statistics-driven iteration control (kb: techniques/auto-iterations) and the adaptive
//! interior-test switch (kb: techniques/interior-detection). Pure functions of a completed
//! level's counters, so every backend's scheduler makes the same decisions.

/// Fraction of pixels allowed to escape beyond the learned limit.
pub const UNRESOLVED_TOLERANCE: f64 = 0.01;

/// Fraction of unresolved pixels from which a view counts as unresolved as a whole (a large
/// flat area) and is rendered again at a higher limit when no pass looked past its limit.
pub const RETRY_FRACTION: f64 = 0.25;

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
    /// Limit the last pass iterated to (the nominal limit when no extension pass ran).
    pub reached: usize,
    /// The level ran the derivative interior test, so `crossed` pixels are undecided rather
    /// than drawn as interior, and the view may be re-rendered at a higher limit (refined and
    /// offscreen views; gesture frames are superseded anyway).
    pub may_retry: bool,
}

/// The learner's decision after a completed view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Learned {
    /// The new learned floor.
    pub limit: usize,
    /// The view itself is left unresolved and nothing past its limit was looked at: render it
    /// again at the new limit (instead of applying it from the next view on).
    pub retry: bool,
}

/// The learned iteration floor after a completed view: the smallest of 2x or 4x the limit
/// that leaves fewer than 1 % of the pixels escaping beyond it, if at least 1 % escaped past
/// the limit; 4x the limit, and a re-render of the view, when at least a quarter of the pixels
/// are unresolved and no pass looked as far as 4x the limit (nothing to learn from: near a
/// deep minibrot every pixel can reach the depth heuristic's limit, a uniform unresolved
/// view; smaller unresolved areas are left to the extension passes); a
/// slow decay (x0.7) when nothing came near the limit; capped at the iteration cap and at 4x
/// the depth heuristic, so a view whose unresolved pixels are interior settles after at most
/// one re-render.
pub fn learn_iterations(old: usize, level: &LevelOutcome, cap: usize, depth_iterations: usize) -> Learned {
    let soft = level.soft_iter;
    let mut learned = old;
    let mut retry = false;
    let tolerance = UNRESOLVED_TOLERANCE * level.texels as f64;
    if level.escaped_past[0] as f64 >= tolerance {
        let mut k = 1;
        while k < 2 && level.escaped_past[k] as f64 >= tolerance {
            k += 1;
        }
        learned = old.max(soft << k);
    } else if level.may_retry
        && level.crossed as f64 >= RETRY_FRACTION * level.texels as f64
        && level.reached < 4 * soft
    {
        learned = old.max(soft << 2);
        retry = true;
    } else if level.crossed == 0 && level.max_escape < soft / 4 {
        learned = (old as f64 * 0.7) as usize;
    }
    let limit = learned.min(cap).min(4 * depth_iterations);
    Learned {
        limit,
        retry: retry && limit > soft,
    }
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
            reached: 32_000,
            may_retry: true,
        };
        let limit = |old, l: &LevelOutcome, depth| learn_iterations(old, l, 200_000, depth).limit;
        assert_eq!(limit(0, &l, 5000), 8000);
        l.escaped_past = [300, 50, 0, 0];
        assert_eq!(limit(0, &l, 5000), 4000);
        // capped at 4x depth
        assert_eq!(limit(0, &l, 500), 2000);
        // decay
        let quiet = LevelOutcome {
            soft_iter: 2000,
            texels: 10_000,
            crossed: 0,
            max_escape: 100,
            escaped_past: [0; 4],
            reached: 2000,
            may_retry: true,
        };
        assert_eq!(
            learn_iterations(1000, &quiet, 200_000, 5000),
            Learned {
                limit: 700,
                retry: false
            }
        );
    }

    /// sea-8007 at default settings: every pixel reaches the depth limit (29 575), none escapes,
    /// no extension pass ran. The limit grows and the view is rendered again.
    #[test]
    fn nothing_escaped() {
        let mut l = LevelOutcome {
            soft_iter: 29_575,
            texels: 576_000,
            crossed: 576_000,
            max_escape: 0,
            escaped_past: [0; 4],
            reached: 29_575,
            may_retry: true,
        };
        let depth = 29_575;
        assert_eq!(
            learn_iterations(0, &l, 200_000, depth),
            Learned {
                limit: 118_300,
                retry: true
            }
        );
        // At the 4x depth cap: no further retry (a view of unresolved interior settles).
        l.soft_iter = 118_300;
        l.reached = 118_300;
        assert_eq!(
            learn_iterations(118_300, &l, 200_000, depth),
            Learned {
                limit: 118_300,
                retry: false
            }
        );
        // Extension passes already looked to 16x and found nothing: interior, no growth.
        l.soft_iter = 29_575;
        l.reached = 16 * 29_575;
        assert_eq!(learn_iterations(0, &l, 200_000, depth).limit, 0);
        // Gesture frames and views without the interior test do not grow on unresolved pixels.
        l.reached = 29_575;
        l.may_retry = false;
        assert_eq!(learn_iterations(0, &l, 200_000, depth).limit, 0);
        // Unresolved halos (here 10 %) are left to the extension passes.
        l.may_retry = true;
        l.crossed = 57_600;
        assert_eq!(learn_iterations(0, &l, 200_000, depth).limit, 0);
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
