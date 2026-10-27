//! Seek minibrot (key N; kb: decisions/controls "Seek minibrot"): find the lowest-period
//! minibrot in or near the view, refine its nucleus with Newton and frame it; from a view that
//! already shows a minibrot filling it, the next smaller one around it.
//!
//! The search ([`seek`]) is pure and bounded (a time budget, a cancel flag, a fixed number of
//! discs and retries; kb: techniques/hang-prevention); [`Seeker`] runs it on a thread for the
//! `Navigator` and keeps what the HUD says.
//!
//! **Minibrot, not bulb.** Ball period detection finds the lowest period whose atom touches a
//! disc, and that is often a satellite bulb (period-doubling bulbs of a minibrot have periods
//! 2p, 3p, ...). A minibrot (a primitive component, a cardioid) is told apart by its shape:
//! with the complex size estimate s (a point w of the whole set maps to nucleus + s w), a
//! cardioid spans w = −0.75 … 0.25 on its axis, so w = −0.6 is interior with a period-p
//! attracting cycle; a satellite bulb is a disc of radius ~0.5 |s|, and w = −0.6 lies in its
//! own period-2 bulb (period 2p). (Beyond the cusp is no test: an embedded minibrot's cusp
//! sits on the filament it hangs from.)
//!
//! **Multibrot `z^d + c` (F3, kb: plans/f3-multibrot).** The same search with the formula's
//! period detection, Newton and size estimate (`w -> nucleus + s w` maps the whole Multibrot set
//! onto the minibrot). Its main component has d - 1 cusps and the period-2 bulbs attach at
//! `|w| = (d+1)/d d^(-1/(d-1))` in the directions `e^(i pi (2k+1)/(d-1))`; the test point is 0.8 of
//! the way there (`-0.6` for d = 2). The set is centred on 0, so the framing is too; there is no
//! antenna airship to tune to (the ring search alone finds the next smaller one).
//!
//! **Mandelbar (F5, kb: plans/f5-mandelbar).** Jacobian Newton and a size from the geometric mean
//! of `dz_p/dc`'s singular values (no rotation); no shape test (every component found counts) and
//! no airship; framed on the nucleus, the ring at Mandelbrot's width.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::math::{
    DoubleExp, DoubleExpComplex, Formula, NewtonOptions, PeriodMethod, ReferenceOrbitComputer, find_nucleus_for,
    find_period_from_for, minibrot_size_complex_for, with_no_control,
};
use crate::viewport::{Center, Scale, Viewport};

/// Wall-clock budget of one search (the HUD says "seeking a minibrot…" meanwhile).
pub const BUDGET: Duration = Duration::from_secs(4);
/// Highest period tried by ball period detection.
pub const MAX_PERIOD: usize = 200_000;
/// Disc radii around the view centre, in view spans (shorter side): the whole view (its
/// half-diagonal is under 1 span up to 16:9), then near it.
const RADII: [f64; 3] = [1.0, 3.0, 9.0];
/// Satellites (bulbs) and too-large minibrots skipped per disc before the next disc.
const RETRIES: usize = 6;
/// The framing: the view's shorter side spans this many |s| (the minibrot with its antenna,
/// ~2.5 |s| along its axis, fills most of it), centred at w = −0.55 (inside its cardioid,
/// between the cusp and the period-2 bulb).
pub const FRAME_SPAN: f64 = 2.8;
const FRAME_W: f64 = -0.55;
/// A minibrot whose framing is wider than this share of the view is "the one in view": the
/// search goes on to a smaller one around it.
const HERE_SHARE: f64 = 0.5;

/// A minibrot found by [`seek`].
#[derive(Clone, Debug)]
pub struct Minibrot {
    pub nucleus: Center,
    pub period: usize,
    /// Complex size estimate: w of the whole set maps to `nucleus + size * w`.
    pub size: DoubleExpComplex,
    /// The fractal it is a minibrot of (F3).
    pub formula: Formula,
}

/// The framing's centre in the minibrot's own coordinates: inside the cardioid for Mandelbrot,
/// the centre of the (d - 1)-fold symmetric set for Multibrot (F3).
fn frame_w(formula: Formula) -> f64 {
    if formula.is_mandelbrot() { FRAME_W } else { 0.0 }
}

/// The interior test point of a minibrot's main component (w): 0.8 of the way to its period-2
/// bulb (`-0.6` for Mandelbrot).
fn test_w(formula: Formula) -> (f64, f64) {
    let d = f64::from(formula.power());
    let r = 0.8 * (d + 1.0) / d * d.powf(-1.0 / (d - 1.0));
    let a = std::f64::consts::PI / (d - 1.0);
    if formula.is_mandelbrot() {
        (-0.6, 0.0)
    } else {
        (r * a.cos(), r * a.sin())
    }
}

impl Minibrot {
    /// |size| as a scale.
    pub fn sigma(&self) -> Scale {
        scale_of(self.size.magnitude())
    }

    /// The framing: centre and span of the view's shorter side.
    pub fn framing(&self) -> (Center, Scale) {
        let span = self.sigma() * FRAME_SPAN;
        let bits = Viewport::with_center(Center::new(0.0, 0.0), span / 4096.0).needed_center_bits();
        let c = offset_by(
            &self.nucleus.with_min_precision(bits),
            self.size,
            frame_w(self.formula),
            0.0,
        );
        (c, span)
    }

    /// The framing as a view for a window whose shorter side is `min_side` points.
    pub fn viewport(&self, min_side: f64) -> Viewport {
        let (c, span) = self.framing();
        let side = if min_side.is_finite() && min_side > 0.0 {
            min_side
        } else {
            800.0
        };
        Viewport::with_center(c, span / side)
    }

    /// The point w of the whole set, mapped into this minibrot.
    fn at(&self, w: (f64, f64)) -> Center {
        offset_by(&self.nucleus, self.size, w.0, w.1)
    }
}

fn scale_of(d: DoubleExp) -> Scale {
    let l = d.log2_magnitude();
    if l.is_finite() {
        Scale::from_log2(l)
    } else {
        Scale::pow2(-100_000)
    }
}

/// `c + s * (wr + i wi)` at c's precision.
fn offset_by(c: &Center, s: DoubleExpComplex, wr: f64, wi: f64) -> Center {
    let d = s * DoubleExpComplex::new(DoubleExp::from_f64(wr), DoubleExp::from_f64(wi));
    let m = d.magnitude();
    if m.is_zero() {
        return c.clone();
    }
    c.offset_scaled((d.re / m).to_f64(), (d.im / m).to_f64(), scale_of(m))
}

/// Bits for a search at radius `r` (as `almondbread locate`).
fn bits_for(r: Scale) -> usize {
    (((-r.log2()).max(0.0) as usize) + 128).div_ceil(64) * 64
}

/// The search's stop condition: cancelled or out of time.
struct Stop<'a> {
    cancel: &'a AtomicBool,
    deadline: Instant,
}

impl Stop<'_> {
    fn now(&self) -> bool {
        self.cancel.load(Ordering::Relaxed) || Instant::now() >= self.deadline
    }
}

/// What [`lowest_in_disc`] does with a minibrot it found.
enum Verdict {
    Take,
    /// Look on (higher periods).
    Skip,
    Stop,
}

/// The minibrot of the lowest period touching the disc (`center`, `r`) from `min_period` on
/// (to `max_period`, never `skip_period`), skipping satellite bulbs and what `verdict` skips;
/// Newton from the disc centre (within 16 r, as `almondbread locate`).
#[allow(clippy::too_many_arguments)]
fn lowest_in_disc(
    formula: Formula,
    center: &Center,
    r: Scale,
    mut min_period: usize,
    max_period: usize,
    skip_period: usize,
    stop: &Stop,
    verdict: &mut dyn FnMut(&Minibrot) -> Verdict,
) -> Option<Minibrot> {
    let bits = bits_for(r).max(center.precision_bits());
    let start = center.with_min_precision(bits);
    let rb = r.big_times(1.0, 64);
    for _ in 0..RETRIES {
        if min_period > max_period || stop.now() {
            return None;
        }
        let p = find_period_from_for(
            formula,
            start.big(),
            &rb,
            min_period,
            max_period,
            PeriodMethod::Ball,
            1024,
            &mut || stop.now(),
        )?;
        min_period = p + 1;
        if p == skip_period {
            continue;
        }
        if let Some(m) = nucleus_near(formula, &start, p, r * 16.0, stop)
            && is_cardioid(&m, stop)
        {
            match verdict(&m) {
                Verdict::Take => return Some(m),
                Verdict::Skip => {}
                Verdict::Stop => return None,
            }
        }
    }
    None
}

/// The period-p nucleus Newton finds from `start` (within `max_distance`), resolved far below
/// the minibrot's size: the precision its size asks for, and Newton's last step well below it
/// (Newton may stall at half the precision; a nucleus off by more than the size gives a wrong
/// size and framing). `None` when Newton fails, or lands on the nucleus of a divisor of p
/// (z_p = 0 there too; its size estimate is then 0 or absurdly large).
fn nucleus_near(formula: Formula, start: &Center, p: usize, max_distance: Scale, stop: &Stop) -> Option<Minibrot> {
    let mut bits = start.precision_bits();
    let mut opts = NewtonOptions {
        precision_bits: Some(bits),
        max_steps: 64,
        max_distance: Some(max_distance.big_times(1.0, 64)),
    };
    let mut cancelled = || stop.now();
    let mut n = find_nucleus_for(formula, start.big(), p, &opts, &mut cancelled)?;
    let mut size = minibrot_size_complex_for(formula, &n.nucleus, p);
    for _ in 0..6 {
        let size_log2 = size.magnitude().log2_magnitude();
        if size_log2.is_nan() || size_log2 >= 0.6 {
            // Zero, absurd or NaN: a divisor's nucleus.
            return None;
        }
        let need = ((-size_log2).max(0.0) as usize)
            .saturating_add(96)
            .min(1 << 17)
            .div_ceil(64)
            * 64;
        if need <= bits && n.last_step_log2 < size_log2 - 48.0 {
            return Some(Minibrot {
                nucleus: Center::from_big(n.nucleus),
                period: p,
                size,
                formula,
            });
        }
        bits = need.max(bits + 128).min(1 << 17);
        opts.precision_bits = Some(bits);
        n = find_nucleus_for(formula, &n.nucleus.with_precision(bits), p, &opts, &mut cancelled)?;
        size = minibrot_size_complex_for(formula, &n.nucleus, p);
    }
    None
}

/// True if the component is a minibrot (a cardioid), not a satellite bulb: at w = −0.6 a
/// cardioid is interior with an attracting cycle of period p (|λ| ≈ 0.84), while a satellite
/// (a disc of radius ~0.5 |s|) has its period-2 bulb there (period 2p) or nothing. Perturbation
/// about the nucleus's periodic orbit (rebased to its start when the orbit comes back near 0;
/// kb: techniques/rebasing) in extended range at any depth, for at most [`TEST_PERIODS`]
/// periods; the cycle is attracting when the orbit after successive periods moves by less than
/// 1e-4 of its size (a 2p-cycle: after every second period).
fn is_cardioid(m: &Minibrot, stop: &Stop) -> bool {
    let p = m.period;
    let f = m.formula;
    let bits = m.nucleus.precision_bits();
    let (z, r) = with_no_control(4096, |c| {
        ReferenceOrbitComputer::new(m.nucleus.big(), bits)
            .with_formula(f)
            .compute_double_exp(p, 1e10, c)
    });
    if r.escaped || z.len() < p + 1 {
        return false;
    }
    if !f.is_conformal() {
        // F5 (Mandelbar): a minibrot is a skewed copy (a Tricorn or Mandelbrot shape) whose
        // orientation the size estimate does not give, so there is no test point: every
        // component Newton reached counts (satellite bulbs are not told apart; kb:
        // plans/f5-mandelbar "As built").
        return true;
    }
    let (tr, ti) = test_w(f);
    let dc = m.size * DoubleExpComplex::new(DoubleExp::from_f64(tr), DoubleExp::from_f64(ti));
    let four = DoubleExp::from_f64(4.0);
    let tol = DoubleExp::from_f64(1e-8);
    let mut d = DoubleExpComplex::ZERO;
    let mut k = 0usize;
    // The orbit after the last two periods: settling on one point is the period-p cycle (a
    // cardioid), on two points a period-2p cycle (a satellite's own period-2 bulb).
    let mut prev: [Option<DoubleExpComplex>; 2] = [None, None];
    let apart = |a: DoubleExpComplex, b: DoubleExpComplex, f2: DoubleExp| {
        DoubleExpComplex::new(a.re - b.re, a.im - b.im).magnitude_squared() / f2
    };
    let tol2 = DoubleExp::from_f64(1e-4);
    for i in 0..TEST_PERIODS * p {
        if i % 4096 == 4095 && stop.now() {
            return false;
        }
        d = if f.is_mandelbrot() {
            z[k].times_two() * d + d * d + dc
        } else {
            d * f.delta_factor_x(z[k], z[k] + d) + dc
        };
        k += 1;
        let full = z[k] + d;
        let f2 = full.magnitude_squared();
        if f2 > four {
            return false;
        }
        if (i + 1) % p == 0 {
            if let Some(q) = prev[0] {
                let one = apart(full, q, f2);
                if one < tol {
                    return true;
                }
                // Two points 1 % apart or more, each period-2 step back on itself.
                if let Some(q2) = prev[1]
                    && apart(full, q2, f2) < tol
                    && one > tol2
                {
                    return false;
                }
            }
            prev = [Some(full), prev[0]];
        }
        if k == p || f2 < d.magnitude_squared() {
            d = full;
            k = 0;
        }
    }
    false
}

/// Periods [`is_cardioid`] follows the orbit for at most.
const TEST_PERIODS: usize = 160;

/// The lowest-period minibrot in or near the view (`center`, `span` = the shorter side), or,
/// when the view already shows one filling it, the next smaller one around that one. Bounded
/// by `budget` and `cancel`.
pub fn seek(center: &Center, span: Scale, cancel: &AtomicBool, budget: Duration) -> Option<Minibrot> {
    seek_for(Formula::Mandelbrot, center, span, cancel, budget)
}

/// [`seek`] in any formula's set (F3).
pub fn seek_for(
    formula: Formula,
    center: &Center,
    span: Scale,
    cancel: &AtomicBool,
    budget: Duration,
) -> Option<Minibrot> {
    let stop = Stop {
        cancel,
        deadline: Instant::now() + budget,
    };
    for f in RADII {
        if stop.now() {
            return None;
        }
        let mut here: Option<Minibrot> = None;
        let mut verdict = |m: &Minibrot| {
            let frame = m.sigma() * FRAME_SPAN;
            if frame.ratio(span) <= HERE_SHARE {
                return Verdict::Take;
            }
            // The minibrot in view (or about the view's size, near it): go on from it. A much
            // larger one (the view is in its decorations) is skipped.
            if frame.ratio(span) < 8.0 && m.nucleus.distance_in(center, span) < 2.0 {
                here = Some(m.clone());
                return Verdict::Stop;
            }
            Verdict::Skip
        };
        let found = lowest_in_disc(formula, center, span * f, 1, MAX_PERIOD, 0, &stop, &mut verdict);
        if let Some(m) = found {
            return Some(m);
        }
        if let Some(h) = here {
            let mut r = smaller_than(&h, &stop);
            // F5: a non-conformal set's components are not told apart from satellites (no shape
            // test), so the one "in view" may be a larger neighbour of the minibrot framed now,
            // and the next smaller one around it the framed one again: go on from that.
            if !formula.is_conformal() {
                for _ in 0..4 {
                    match &r {
                        Some(m)
                            if (m.sigma() * FRAME_SPAN).ratio(span) > HERE_SHARE
                                && m.nucleus.distance_in(center, span) < 2.0 =>
                        {
                            let m = m.clone();
                            r = smaller_than(&m, &stop);
                        }
                        _ => break,
                    }
                }
            }
            return r;
        }
    }
    None
}

/// The period-3 minibrot of the whole set (w of its nucleus): by tuning, every minibrot of
/// period p carries a copy of it of period 3p on its antenna, the lowest-period minibrot of its
/// own copy of the set.
const AIRSHIP: (f64, f64) = (-1.754_877_666_246_693, 0.0);

/// Disc centres around a minibrot, in its own coordinates w, radius 1: a ring just outside its
/// decorations, where the minibrots of the structure it sits in are.
const RING: [(f64, f64); 8] = [
    (2.6, 0.0),
    (1.84, 1.84),
    (0.0, 2.6),
    (-1.84, 1.84),
    (-2.6, 0.0),
    (-1.84, -1.84),
    (0.0, -2.6),
    (1.84, -1.84),
];

/// The next smaller minibrot around `m`: the copy of the period-3 minibrot on its antenna
/// (Newton for period 3p from where tuning puts it), or a minibrot of a lower period than that
/// in the discs of [`RING`] (ball period detection from period p + 1), smaller than 0.6 of m;
/// the lowest period wins (ties: the larger one). What was found when the budget runs out
/// counts.
fn smaller_than(m: &Minibrot, stop: &Stop) -> Option<Minibrot> {
    let sigma = m.sigma();
    let smaller = |c: &Minibrot| c.sigma().ratio(sigma) < 0.6;
    let start = m.at(AIRSHIP);
    let bits = bits_for(sigma * 0.01).max(start.precision_bits());
    // (Multibrot, F3: no antenna airship; the ring alone.)
    let mut best = m
        .period
        .checked_mul(3)
        .filter(|&p| p <= MAX_PERIOD && m.formula.is_mandelbrot())
        .and_then(|p| nucleus_near(m.formula, &start.with_min_precision(bits), p, sigma * 0.1, stop))
        .filter(|c| smaller(c) && is_cardioid(c, stop));
    // (Multibrot, F3: its decorations end near |w| = 1.3, so the ring is half as wide; the
    // Mandelbar set, F5, reaches |w| = 2 with its antennas, as Mandelbrot.)
    // (F6: the Burning Ship's minibrots are skewed Mandelbrot copies with their antennas: the
    // same width.)
    let k = if m.formula.is_mandelbrot() || !m.formula.is_conformal() {
        1.0
    } else {
        0.5
    };
    for w in RING {
        if stop.now() {
            break;
        }
        // Only a lower period than the best so far can win; 2p near m is its period-2 bulb.
        let max = best.as_ref().map_or(MAX_PERIOD, |b| b.period - 1);
        let mut verdict = |c: &Minibrot| if smaller(c) { Verdict::Take } else { Verdict::Skip };
        if let Some(c) = lowest_in_disc(
            m.formula,
            &m.at((w.0 * k, w.1 * k)),
            sigma * k,
            m.period + 1,
            max,
            2 * m.period,
            stop,
            &mut verdict,
        ) {
            best = Some(c);
        }
    }
    best
}

// MARK: The navigator's seeker

/// What the HUD says about seeking (kb: decisions/controls "Seek minibrot").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SeekPhase {
    #[default]
    Idle,
    /// The search runs ("seeking a minibrot…").
    Searching,
    /// Flying to the minibrot found ("Minibrot, period p · 1e69").
    Flying,
    /// Landed on it (the same text, for [`ARRIVED_SHOWN`]).
    Arrived,
    /// Nothing found within the radius and budget ("no minibrot found nearby").
    NotFound,
}

/// Seconds the arrival and the not-found note stay in the HUD.
pub const ARRIVED_SHOWN: f64 = 6.0;
pub const NOT_FOUND_SHOWN: f64 = 3.0;

#[derive(Debug, Default)]
struct Slot {
    cancel: AtomicBool,
    result: Mutex<Option<Option<Minibrot>>>,
}

/// Runs [`seek`] off the main thread and keeps its state for the HUD.
#[derive(Clone, Debug, Default)]
pub struct Seeker {
    slot: Option<Arc<Slot>>,
    phase: SeekPhase,
    since: f64,
    /// The last minibrot found: period and the framing's zoom (log10).
    found: Option<(usize, f64)>,
    /// Search on the calling thread (tests, headless tools).
    pub synchronous: bool,
    /// The fractal searched (F3; the host keeps it in step with the settings).
    pub formula: Formula,
}

impl Seeker {
    /// Starts a search from the view (`min_side`: the window's shorter side in points). A
    /// search already running goes on.
    pub fn start(&mut self, view: &Viewport, min_side: f64, now: f64) {
        if self.phase == SeekPhase::Searching {
            return;
        }
        let side = if min_side.is_finite() && min_side > 0.0 {
            min_side
        } else {
            800.0
        };
        let span = view.scale * side;
        let center = view.center.clone();
        let formula = self.formula;
        let slot = Arc::new(Slot::default());
        self.phase = SeekPhase::Searching;
        self.since = now;
        self.found = None;
        if self.synchronous {
            let r = seek_for(formula, &center, span, &slot.cancel, BUDGET);
            if let Ok(mut g) = slot.result.lock() {
                *g = Some(r);
            }
            self.slot = Some(slot);
            return;
        }
        let s = slot.clone();
        let spawned = std::thread::Builder::new().name("seek-minibrot".into()).spawn(move || {
            let r = seek_for(formula, &center, span, &s.cancel, BUDGET);
            if let Ok(mut g) = s.result.lock() {
                *g = Some(r);
            }
        });
        if spawned.is_ok() {
            self.slot = Some(slot);
        } else {
            self.phase = SeekPhase::NotFound;
        }
    }

    pub fn is_searching(&self) -> bool {
        self.phase == SeekPhase::Searching
    }

    /// A finished search: the minibrot to fly to (the phase is then `Flying`), or none (the
    /// HUD says so).
    pub fn poll(&mut self, now: f64) -> Option<Minibrot> {
        let slot = self.slot.as_ref()?;
        let done = slot.result.lock().ok().and_then(|mut g| g.take())?;
        self.slot = None;
        self.since = now;
        match done {
            Some(m) => {
                self.phase = SeekPhase::Flying;
                Some(m)
            }
            None => {
                self.phase = SeekPhase::NotFound;
                None
            }
        }
    }

    /// The flight to `m` started (its target's zoom, log10).
    pub fn flying(&mut self, m: &Minibrot, zoom_log10: f64, now: f64) {
        self.phase = SeekPhase::Flying;
        self.since = now;
        self.found = Some((m.period, zoom_log10));
    }

    /// The flight ended: landed, or stopped on the way (Esc).
    pub fn landed(&mut self, now: f64, stopped: bool) {
        if self.phase == SeekPhase::Flying {
            if stopped {
                self.clear();
            } else {
                self.phase = SeekPhase::Arrived;
                self.since = now;
            }
        }
    }

    /// Cancels a running search. Returns whether one ran.
    pub fn cancel(&mut self) -> bool {
        if let Some(s) = self.slot.take() {
            s.cancel.store(true, Ordering::Relaxed);
        }
        if self.phase == SeekPhase::Searching {
            self.phase = SeekPhase::Idle;
            return true;
        }
        false
    }

    /// Cancels the search and forgets the label (navigation took over).
    pub fn clear(&mut self) {
        self.cancel();
        self.phase = SeekPhase::Idle;
        self.found = None;
    }

    /// The HUD state at `now`: phase, period, the target's zoom (log10).
    pub fn status(&self, now: f64) -> (SeekPhase, usize, f64) {
        let (p, z) = self.found.unwrap_or((0, 0.0));
        let phase = match self.phase {
            SeekPhase::Arrived if now - self.since > ARRIVED_SHOWN => SeekPhase::Idle,
            SeekPhase::NotFound if now - self.since > NOT_FOUND_SHOWN => SeekPhase::Idle,
            ph => ph,
        };
        (phase, p, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::BigComplex;

    fn run(re: &str, im: &str, span: &str) -> Option<Minibrot> {
        let c = Center::parse(re, im).expect("centre");
        let span = Scale::parse(span).expect("span");
        let never = AtomicBool::new(false);
        seek(&c, span, &never, Duration::from_secs(30))
    }

    #[test]
    fn home_finds_the_airship() {
        // The whole set is the main cardioid's view: the next smaller minibrot is period 3.
        let m = run("-0.75", "0", "3").expect("found");
        assert_eq!(m.period, 3);
        assert!((m.nucleus.re() + 1.754_877_666).abs() < 1e-6, "{}", m.nucleus.re());
        // Its framing shows it filling the view.
        let (c, span) = m.framing();
        assert!((span.to_f64() - FRAME_SPAN * m.sigma().to_f64()).abs() < 1e-12);
        assert!(c.distance_in(&m.nucleus, span) < 0.3);
        // N again: a smaller one around the airship (not the airship again).
        let never = AtomicBool::new(false);
        let n = seek(&c, span, &never, Duration::from_secs(30)).expect("deeper");
        assert!(n.period > 3 && n.sigma().ratio(m.sigma()) < 0.6, "period {}", n.period);
    }

    #[test]
    fn bulbs_are_not_minibrots() {
        let stop_flag = AtomicBool::new(false);
        let stop = Stop {
            cancel: &stop_flag,
            deadline: Instant::now() + Duration::from_secs(10),
        };
        let mk = |re: f64, im: f64, p: usize| {
            let n = find_nucleus_for(
                Formula::Mandelbrot,
                &BigComplex::from_f64(re, im, 128),
                p,
                &NewtonOptions::default(),
                &mut || false,
            )
            .expect("nucleus")
            .nucleus;
            Minibrot {
                size: minibrot_size_complex_for(Formula::Mandelbrot, &n, p),
                nucleus: Center::from_big(n),
                period: p,
                formula: Formula::Mandelbrot,
            }
        };
        assert!(is_cardioid(&mk(0.0, 0.0, 1), &stop));
        assert!(is_cardioid(&mk(-1.7549, 0.0, 3), &stop));
        assert!(!is_cardioid(&mk(-1.0, 0.0, 2), &stop));
        assert!(!is_cardioid(&mk(-0.1226, 0.7449, 3), &stop));
        assert!(!is_cardioid(&mk(-1.3107, 0.0, 4), &stop));
    }

    #[test]
    fn needle_finds_the_period_4640_minibrot_and_then_a_smaller_one() {
        // `almondbread locate` finds period 4640 (size 1e-67) 6.95 radii from this view.
        let re = "-1.4303584327259864377084589925832673307971990283708853";
        let im = "-1.3419188621437467531262066807358441689824585030600614e-11";
        let m = run(re, im, "4.2e-45").expect("found");
        assert_eq!(m.period, 4640);
        let l = m.sigma().log10();
        assert!((-67.5..-66.5).contains(&l), "size 1e{l}");
        // From its framing: a smaller one of a higher period, not the same again.
        let (c, span) = m.framing();
        let never = AtomicBool::new(false);
        let t = Instant::now();
        let n = seek(&c, span, &never, Duration::from_secs(30)).expect("deeper");
        // (A timing bound for optimised builds; the debug-profile run checks only the result.)
        if !cfg!(debug_assertions) {
            assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
        }
        // The copy of the period-3 minibrot on its antenna (tuning: 3 x 4640).
        assert_eq!(n.period, 13920);
        assert!(n.sigma().ratio(m.sigma()) < 0.6);
    }

    #[test]
    fn seahorse_valley_finds_a_minibrot_in_view() {
        let m = run("-0.7436438870371587", "0.1318259042053120", "1e-6").expect("found");
        assert!(m.period > 3);
        assert!(m.sigma().to_f64() * FRAME_SPAN <= 0.5e-6);
        let c = Center::parse("-0.7436438870371587", "0.1318259042053120").unwrap();
        assert!(m.nucleus.distance_in(&c, Scale::from_f64(1e-6)) < 72.0);
    }

    /// The navigator: N flies to the minibrot (a fly-to), lands on its framing, N again goes on
    /// to a smaller one; nothing found: the HUD says so and nothing moves; Esc cancels a search.
    #[test]
    fn n_seeks_flies_and_lands() {
        use crate::input::{InputEvent, NavKey, Navigator};
        let mut n = Navigator::default();
        let mut v = Viewport::fitting(1000.0, 600.0);
        let resize = InputEvent::Resize {
            width: 1000.0,
            height: 600.0,
        };
        n.handle(resize, &mut v, 0.0);
        n.seek_mut().synchronous = true;
        let mut t = 0.0;
        let mut periods = Vec::new();
        for _ in 0..2 {
            assert!(n.handle(InputEvent::KeyDown(NavKey::SeekMinibrot), &mut v, t));
            n.handle(InputEvent::KeyUp(NavKey::SeekMinibrot), &mut v, t);
            let (phase, period, zoom) = n.seek_status(t);
            assert_eq!(phase, SeekPhase::Flying);
            let target = n.fly_target().expect("flying").clone();
            assert!((target.zoom_log10() - zoom).abs() < 1e-9);
            let t0 = t;
            while n.flight().is_some() && t < t0 + 60.0 {
                t += 1.0 / 60.0;
                n.advance(&mut v, t);
            }
            assert!(n.flight().is_none(), "landed");
            assert_eq!(v.scale, target.scale);
            assert!(v.center.distance_in(&target.center, v.scale) < 1e-6);
            assert_eq!(n.seek_status(t).0, SeekPhase::Arrived);
            assert_eq!(n.seek_status(t + ARRIVED_SHOWN + 0.1).0, SeekPhase::Idle);
            periods.push(period);
            t += 1.0;
        }
        assert_eq!(periods[0], 3);
        assert!(periods[1] > 3, "{periods:?}");
        // A pan ends the label.
        n.handle(InputEvent::Pan { dx: 3.0, dy: 0.0 }, &mut v, t);
        assert_eq!(n.seek_status(t).0, SeekPhase::Idle);
        // Far outside the set: nothing found, nothing moves.
        let mut far = Viewport::new(3.0, 3.0, 1e-6);
        assert!(n.handle(InputEvent::KeyDown(NavKey::SeekMinibrot), &mut far, t));
        assert_eq!(n.seek_status(t).0, SeekPhase::NotFound);
        assert!(n.flight().is_none());
        assert_eq!(n.seek_status(t + NOT_FOUND_SHOWN + 0.1).0, SeekPhase::Idle);
        // Esc cancels a search on its thread (and is handled).
        n.seek_mut().synchronous = false;
        n.handle(InputEvent::KeyDown(NavKey::SeekMinibrot), &mut v, t);
        // (The thread may already have found it on a fast machine: then it flies, and the
        // cancel is not exercised this run.)
        let phase = n.seek_status(t).0;
        assert!(matches!(phase, SeekPhase::Searching | SeekPhase::Flying), "{phase:?}");
        assert!(n.advance(&mut v, t), "frames while searching");
        assert!(n.handle(InputEvent::KeyDown(NavKey::Stop), &mut v, t));
        if phase == SeekPhase::Searching {
            assert_eq!(n.seek_status(t).0, SeekPhase::Idle);
        }
        // On its thread the search lands as a flight a few frames later.
        n.handle(InputEvent::KeyDown(NavKey::SeekMinibrot), &mut v, t);
        let t0 = Instant::now();
        while n.flight().is_none() && t0.elapsed() < BUDGET + Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(2));
            t += 1.0 / 60.0;
            n.advance(&mut v, t);
        }
        assert!(n.flight().is_some(), "{:?}", n.seek_status(t));
    }

    #[test]
    fn cancelled_search_returns_quickly() {
        let c = Center::new(-0.75, 0.0);
        let yes = AtomicBool::new(true);
        let t = Instant::now();
        assert!(seek(&c, Scale::from_f64(3.0), &yes, Duration::from_secs(30)).is_none());
        assert!(t.elapsed() < Duration::from_millis(100));
    }
}
