//! Wall-clock bounds in tests that run in parallel with the whole workspace: a single OS
//! preemption (tens of ms on a loaded machine) must not fail a bound, a real cost must.

/// Runs `measure` (a fresh, complete measurement each call, in seconds or ms: the bound's
/// unit) up to `tries` times while it exceeds `bound` and returns the smallest result. A
/// stall that is real shows in every run; a descheduling hits one run, not all of them.
pub fn least_within(bound: f64, tries: usize, mut measure: impl FnMut() -> f64) -> f64 {
    let mut least = f64::INFINITY;
    for k in 1..=tries {
        let t = measure();
        least = least.min(t);
        if least < bound {
            break;
        }
        eprintln!("timing: {t:.4} over the bound {bound} (run {k} of {tries})");
    }
    least
}
