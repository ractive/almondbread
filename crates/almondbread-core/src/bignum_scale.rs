//! "How big is big?": the zoom compared with huge numbers by the *kind* of size, one dial per
//! Knuth up-arrow level (kb: synthesis/how-big-is-big).
//!
//! Dial k reads the arrow height `h_k(x)`: the real `h` with `10 ↑^k h = x`. Dial 1 is
//! `log10` (digits), dial 2 the super-logarithm (height of a tower of 10s), dial 3 the height
//! in towers of towers, and so on. Fractional heights use the *linear approximation*: for
//! `k ≥ 2`
//!
//! ```text
//! h_k(x) = x - 1                    for 0 < x ≤ 1
//! h_k(x) = 1 + h_k(h_(k-1)(x))      for x > 1
//! ```
//!
//! the inverse of the linear approximation of tetration for real heights, `10↑↑t = 1 + t` on
//! `(-1, 0]` and `10↑↑t = 10^(10↑↑(t-1))` above (Hooshmand's "ultra exponential"; kb:
//! sources/wikipedia-tetration), taken to every arrow level. Consequences: `h_k(10↑^k n) = n`
//! for whole `n`, every dial reads 0 at `x = 1` and 1 at `x = 10` (`10 = 10↑1 = 10↑↑1 = ...`),
//! and on `(1, 10]` every dial `k ≥ 2` reads `log10 x`. The functions are continuous and
//! increasing, not smooth (kinks at the integers).
//!
//! Numbers beyond f64 are [`Tower`]s: `10^10^…^top` with a real number of 10s, so a zoom of
//! 10^3000 (or a googolplexian, or a tower of 7.6 trillion 3s) never overflows.

use std::f64::consts::LN_10;

/// `x = 10^10^…^top` with `height` tens (real-valued so towers of 10^12 levels fit).
/// Normal form: `top` in (1, 10] whenever `height > 0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tower {
    pub height: f64,
    pub top: f64,
}

impl Tower {
    pub fn from_f64(x: f64) -> Self {
        Self { height: 0.0, top: x }.normalized()
    }

    /// `10^l`, for any finite `l` (the zoom from its log10).
    pub fn from_log10(l: f64) -> Self {
        Self { height: 1.0, top: l }.normalized()
    }

    /// `10↑↑h` for a whole `h ≥ 0` with `top` on the tower: `10^10^…^top`.
    pub fn tens(height: f64, top: f64) -> Self {
        Self { height, top }.normalized()
    }

    fn normalized(mut self) -> Self {
        while self.top > 10.0 {
            self.top = self.top.log10();
            self.height += 1.0;
        }
        while self.height >= 1.0 && self.top <= 1.0 {
            self.top = 10f64.powf(self.top);
            self.height -= 1.0;
        }
        self
    }

    /// `10^self`.
    pub fn exp10(self) -> Self {
        Self {
            height: self.height + 1.0,
            top: self.top,
        }
        .normalized()
    }

    /// `log10(self)` (for `self > 1` as a tower, or any positive f64).
    pub fn log10(self) -> Self {
        if self.height >= 1.0 {
            Self {
                height: self.height - 1.0,
                top: self.top,
            }
            .normalized()
        } else {
            Self::from_f64(self.top.log10())
        }
    }

    /// The value as f64 (inf beyond).
    pub fn to_f64(self) -> f64 {
        let mut t = self.top;
        let mut h = self.height;
        while h >= 1.0 {
            if t > 308.3 {
                return f64::INFINITY;
            }
            t = 10f64.powf(t);
            h -= 1.0;
        }
        t
    }

    /// `self + a` for a small `a` (absorbed when it changes nothing at f64 precision).
    fn add_small(self, a: f64) -> Self {
        let v = self.to_f64();
        if v.is_finite() { Self::from_f64(v + a) } else { self }
    }

    /// `b^self` for `b > 1` given as `log10 b`.
    pub fn pow_of(self, log10_base: f64) -> Self {
        // b^x = 10^(x·log10 b); log10(x·c) = log10 x + log10 c.
        let v = self.to_f64();
        if v.is_finite() && v * log10_base < 1e300 {
            return Self::from_log10(v * log10_base);
        }
        self.log10().add_small(log10_base.log10()).exp10().exp10()
    }

    /// `h_k(self)`: dial `k` (see the module docs). `k = 1` is `log10`, inf beyond f64.
    pub fn arrow_height(self, k: u32) -> f64 {
        match k {
            0 => self.to_f64(),
            1 => self.log10().to_f64(),
            2 => self.height + arrow_height(2, self.top),
            _ if self.height == 0.0 => arrow_height(k, self.top),
            _ => 1.0 + arrow_height(k, self.arrow_height(k - 1)),
        }
    }
}

/// `h_k(x)` for an f64 `x` (see the module docs). Below the domain (x ≤ 0 for k ≥ 2) it
/// continues with `h_k(x) = h_k(10↑^(k-1) x) - 1` as far as defined, else -inf.
pub fn arrow_height(k: u32, x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    match k {
        0 => x,
        1 => x.log10(),
        _ if x == f64::INFINITY => f64::INFINITY,
        _ if x > 1.0 => 1.0 + arrow_height(k, arrow_height(k - 1, x)),
        _ if x > 0.0 => x - 1.0,
        2 if x > f64::NEG_INFINITY => 10f64.powf(x) - 2.0,
        // 10↑^(k-1) t = 1 + t on (-1, 0] for k - 1 ≥ 2.
        _ if x > -1.0 => x - 1.0,
        _ => f64::NEG_INFINITY,
    }
}

/// Dials 1..=`levels` for a zoom of `10^zoom_log10` (dial 1 is `zoom_log10` itself).
pub fn zoom_dials(zoom_log10: f64, levels: u32) -> Vec<f64> {
    let t = Tower::from_log10(zoom_log10);
    (1..=levels).map(|k| t.arrow_height(k)).collect()
}

/// `b↑↑n` (a tower of `n` copies of `b`, `b > 1` given as `log10 b`) for a real `n ≥ 1`
/// (whole numbers are exact up to rounding; fractional `n` only through the final shift).
/// Iterates until the tower's top settles at f64 precision, then adds the remaining levels:
/// beyond a few levels each further `b^` only adds one 10 to the tower.
pub fn tower_of(log10_base: f64, n: f64) -> Tower {
    let mut t = Tower::from_log10(log10_base);
    let mut done = 1.0;
    while done + 1.0 <= n {
        let next = t.pow_of(log10_base);
        done += 1.0;
        if next.height >= t.height + 1.0 && (next.top - t.top).abs() <= 1e-15 * t.top && t.height > 3.0 {
            return Tower {
                height: next.height + (n - done),
                top: next.top,
            };
        }
        t = next;
    }
    t
}

/// 3↑↑↑3 = 3↑↑(3↑↑3) = a tower of 7 625 597 484 987 threes.
pub const TRITRI_TOWER: f64 = 7_625_597_484_987.0;

/// The dials of 3↑↑↑3 (dial 1 and 2 are beyond f64: inf).
pub fn tritri_dials(levels: u32) -> Vec<f64> {
    let t = tower_of(3f64.log10(), TRITRI_TOWER);
    (1..=levels).map(|k| t.arrow_height(k)).collect()
}

/// The dials of Graham's g₁ = 3↑↑↑↑3 = 3↑↑↑(3↑↑↑3). Dials 1-3 are beyond f64 (inf); dial 4 is
/// `2 + h_4(h_3(3↑↑↑3))`: `h_3(3↑↑↑n) ≈ n - 3 + h_3(3↑↑↑3)` (each further 3↑↑ adds one level
/// on dial 3 up to a shift that vanishes after a level), so `h_3(g₁) ≈ 3↑↑↑3` and
/// `h_4(g₁) = 1 + h_4(h_3(g₁)) ≈ 2 + h_4(h_3(3↑↑↑3))`. Dial k ≥ 5: `1 + h_k(h_(k-1))`.
pub fn graham_g1_dials(levels: u32) -> Vec<f64> {
    let tritri = tower_of(3f64.log10(), TRITRI_TOWER);
    let h3 = tritri.arrow_height(3);
    let mut out = vec![f64::INFINITY; levels.min(3) as usize];
    let mut prev = 2.0 + arrow_height(4, h3);
    for k in 4..=levels {
        if k > 4 {
            prev = 1.0 + arrow_height(k, prev);
        }
        out.push(prev);
    }
    out
}

/// A big number for the panel: where it sits on each dial.
#[derive(Clone, Debug, PartialEq)]
pub struct BigNumberMarker {
    pub id: &'static str,
    pub name: &'static str,
    /// How it is written ("10^100", "10↑↑257 < mega < 10↑↑258").
    pub notation: &'static str,
    /// One plain sentence.
    pub note: &'static str,
    /// Dials 1..=levels (inf: beyond f64 / off every scale; nan: can't be placed honestly).
    pub dials: Vec<f64>,
}

fn marker(
    id: &'static str,
    name: &'static str,
    notation: &'static str,
    note: &'static str,
    t: Tower,
    levels: u32,
) -> BigNumberMarker {
    BigNumberMarker {
        id,
        name,
        notation,
        note,
        dials: (1..=levels).map(|k| t.arrow_height(k)).collect(),
    }
}

/// The deepest zoom: `HOME_SCALE / 2^MIN_SCALE_LOG2` (≈ 10^3008).
pub fn max_zoom_log10() -> f64 {
    crate::viewport::HOME_SCALE.log10() - crate::viewport::MIN_SCALE_LOG2 as f64 * 2f64.log10()
}

/// The panel's markers, smallest first (kb: synthesis/how-big-is-big for the sources).
pub fn big_number_markers(levels: u32) -> Vec<BigNumberMarker> {
    // Skewes: e^e^e^x = 10^10^(e^x·log10 e + log10 log10 e) since log10(e^y) = y·log10 e.
    let log10_e = std::f64::consts::LOG10_E;
    let ee = |x: f64| x.exp() * log10_e + log10_e.log10();
    // 1933: e^e^e^79 = 10^10^(8.85e33).
    let sk1 = Tower::from_f64(ee(79.0)).exp10().exp10();
    // 1955: e^e^e^e^7.705 = 10^10^10^(961.8…): one more e^, whose log10 log10 log10 is
    // e^7.705·log10 e + log10 log10 e up to an additive term below 1e-900.
    let sk2 = Tower::from_f64(ee(7.705)).exp10().exp10().exp10();
    let mut v = vec![
        marker(
            "zoom-limit",
            "Our deepest zoom",
            "≈ 10^3008",
            "Where almondbread stops: reference orbits need 10 000 bits.",
            Tower::from_log10(max_zoom_log10()),
            levels,
        ),
        marker(
            "googol",
            "Googol",
            "10^100",
            "1 followed by 100 zeros (Kasner's nephew, 1920).",
            Tower::from_log10(100.0),
            levels,
        ),
        marker(
            "googolplex",
            "Googolplex",
            "10^10^100",
            "1 followed by a googol zeros: too many digits to write in the universe.",
            Tower::from_log10(100.0).exp10(),
            levels,
        ),
        marker(
            "skewes-1",
            "Skewes' number (1933)",
            "e^e^e^79 < 10^10^10^34",
            "Bound for the first sign change of π(x) − li(x), assuming the Riemann hypothesis.",
            sk1,
            levels,
        ),
        marker(
            "googolplexian",
            "Googolplexian",
            "10^10^10^100",
            "10 to the googolplex; an informal name.",
            Tower::from_log10(100.0).exp10().exp10(),
            levels,
        ),
        marker(
            "skewes-2",
            "Skewes' number (1955)",
            "e^e^e^e^7.705 < 10^10^10^964",
            "The same bound without the Riemann hypothesis.",
            sk2,
            levels,
        ),
        marker(
            "mega",
            "Mega",
            "10↑↑257 < mega < 10↑↑258",
            "2 in a circle (Steinhaus–Moser): a tower of about 257 tens.",
            Tower::tens(257.0, 10f64.powf(0.5)), // placed mid-way: 10↑↑257.5
            levels,
        ),
        BigNumberMarker {
            id: "tritri",
            name: "3↑↑↑3",
            notation: "3↑↑(3↑↑3)",
            note: "A tower of 7 625 597 484 987 threes.",
            dials: tritri_dials(levels),
        },
        BigNumberMarker {
            id: "graham-g1",
            name: "Graham's g₁",
            notation: "3↑↑↑↑3",
            note: "The first of Graham's 64 rounds: 3↑↑↑(3↑↑↑3).",
            dials: graham_g1_dials(levels),
        },
    ];
    v.sort_by(|a, b| {
        let key = |m: &BigNumberMarker| m.dials.iter().rev().find(|d| d.is_finite()).copied().unwrap_or(0.0);
        let level = |m: &BigNumberMarker| m.dials.iter().take_while(|d| !d.is_finite()).count();
        (level(a), key(a)).partial_cmp(&(level(b), key(b))).unwrap()
    });
    v
}

/// Facts that depend on the zoom and the zoom speed.
#[derive(Clone, Debug, PartialEq)]
pub struct BigNumberFacts {
    /// Digits of the zoom (`log10`).
    pub zoom_log10: f64,
    /// Magnification per second.
    pub speed_factor: f64,
    /// Seconds from the home view to a googol zoom at this speed.
    pub seconds_to_googol: f64,
    /// Seconds from the home view to the deepest zoom.
    pub seconds_to_limit: f64,
    /// Years to a googolplex zoom.
    pub years_to_googolplex: f64,
    /// Bits a reference orbit needs here (≈ log2 of the zoom).
    pub bits_here: f64,
    /// Bits for a googolplex zoom.
    pub bits_googolplex: f64,
    /// Atoms in the observable universe (order of magnitude).
    pub atoms_in_universe: f64,
    /// Ganson's Machine with Concrete: years for its last gear to turn once.
    pub ganson_years: f64,
}

/// Seconds in a Julian year.
pub const YEAR_SECONDS: f64 = 365.25 * 86400.0;

/// `zoom_speed_ln`: the zoom speed as ln(magnification per second) (the navigator's unit;
/// default 1.6, ×4.95/s).
pub fn big_number_facts(zoom_log10: f64, zoom_speed_ln: f64) -> BigNumberFacts {
    let digits_per_second = zoom_speed_ln / LN_10;
    let log2_10 = LN_10 / std::f64::consts::LN_2;
    BigNumberFacts {
        zoom_log10,
        speed_factor: zoom_speed_ln.exp(),
        seconds_to_googol: 100.0 / digits_per_second,
        seconds_to_limit: max_zoom_log10() / digits_per_second,
        years_to_googolplex: 1e100 / digits_per_second / YEAR_SECONDS,
        bits_here: zoom_log10.max(0.0) * log2_10,
        bits_googolplex: 1e100 * log2_10,
        atoms_in_universe: 1e80,
        // 12 worm pairs of 1:50 at 200 rpm (Make, 2012).
        ganson_years: 50f64.powi(12) / 200.0 / 60.0 / 24.0 / 365.25,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn whole_towers_are_whole_heights() {
        // 10↑↑1 = 10, 10↑↑2 = 1e10, 10↑↑3 = 10^1e10.
        assert!(close(arrow_height(2, 10.0), 1.0, 1e-12));
        assert!(close(arrow_height(2, 1e10), 2.0, 1e-12));
        assert!(close(Tower::from_log10(1e10).arrow_height(2), 3.0, 1e-12));
        assert!(close(Tower::tens(6.0, 10.0).arrow_height(2), 7.0, 1e-12));
        // 10↑↑↑2 = 10↑↑10.
        assert!(close(Tower::tens(10.0, 1.0).arrow_height(3), 2.0, 1e-9));
        assert!(close(Tower::tens(9.0, 10.0).arrow_height(3), 2.0, 1e-9));
        // 10↑↑↑3 = 10↑↑(10↑↑10): beyond a Tower's f64 height; 10↑↑↑↑2 = 10↑↑↑10 likewise.
        // Every dial reads 0 at 1 and 1 at 10.
        for k in 1..=7 {
            assert!(close(arrow_height(k, 1.0), 0.0, 1e-12), "k={k}");
            assert!(close(arrow_height(k, 10.0), 1.0, 1e-12), "k={k}");
        }
        // On (1, 10] every dial k ≥ 2 reads log10 x.
        for k in 2..=6 {
            assert!(close(arrow_height(k, 3.0), 3f64.log10(), 1e-12));
        }
    }

    #[test]
    fn known_values() {
        let d = zoom_dials(100.0, 4);
        assert!(close(d[0], 100.0, 1e-9));
        assert!(close(d[1], 2.0 + 2f64.log10(), 1e-12), "googol ↑↑ {}", d[1]);
        assert!(close(d[2], 1.0 + (2.0 + 2f64.log10()).log10(), 1e-12));
        let gp = Tower::from_log10(100.0).exp10();
        assert!(close(gp.arrow_height(2), 3.0 + 2f64.log10(), 1e-12));
        assert_eq!(gp.arrow_height(1), 1e100);
        let limit = zoom_dials(max_zoom_log10(), 5);
        assert!(close(max_zoom_log10(), 3007.9, 0.1));
        assert!(close(limit[1], 2.5413, 1e-3), "{limit:?}");
        assert!(close(limit[2], 1.4051, 1e-3), "{limit:?}");
        assert!(close(limit[3], 1.1477, 1e-3), "{limit:?}");
        assert!(close(limit[4], 1.0599, 1e-3), "{limit:?}");
        // 3↑↑3 = 7 625 597 484 987.
        assert!(close(tower_of(3f64.log10(), 3.0).to_f64(), 7_625_597_484_987.0, 1e3));
        let tritri = tritri_dials(4);
        assert!(tritri[0].is_infinite() && tritri[1] > 7.6e12 && tritri[1] < 7.7e12);
        assert!(close(tritri[2], 2.3107, 1e-3), "{tritri:?}");
        let g1 = graham_g1_dials(5);
        assert!(g1[..3].iter().all(|d| d.is_infinite()));
        assert!(
            close(g1[3], 2.0 + tritri[2].log10(), 1e-12) && close(g1[3], 2.3637, 1e-3),
            "{g1:?}"
        );
    }

    #[test]
    fn monotonic_and_continuous() {
        for k in 1..=6 {
            let mut prev = f64::NEG_INFINITY;
            let mut l = 0.0;
            while l < 3100.0 {
                let h = zoom_dials(l, k)[k as usize - 1];
                assert!(h >= prev - 1e-12, "k={k} l={l}: {h} < {prev}");
                // Continuous: no jump bigger than the step allows.
                if prev.is_finite() && k > 1 {
                    assert!(h - prev < 0.05, "k={k} l={l}: jump {prev} -> {h}");
                }
                prev = h;
                l += if l < 20.0 { 0.001 } else { 0.25 };
            }
        }
        // f64 domain too, across the kinks at 1 and 10.
        for k in 2..=5 {
            let mut prev = f64::NEG_INFINITY;
            let mut x: f64 = 0.01;
            while x < 1e300 {
                let h = arrow_height(k, x);
                assert!(h > prev, "k={k} x={x}");
                prev = h;
                x *= 1.07;
            }
        }
    }

    #[test]
    fn markers_in_order() {
        let m = big_number_markers(4);
        let pos = |id: &str| m.iter().position(|x| x.id == id).unwrap();
        let order = [
            "googol",
            "zoom-limit",
            "googolplex",
            "skewes-1",
            "googolplexian",
            "skewes-2",
            "mega",
            "tritri",
            "graham-g1",
        ];
        for w in order.windows(2) {
            assert!(pos(w[0]) < pos(w[1]), "{} before {}", w[0], w[1]);
        }
        let get = |id: &str| m[pos(id)].dials.clone();
        assert!(close(get("googolplexian")[1], 4.0 + 2f64.log10(), 1e-9));
        // Skewes 1933: 10^10^10^33.95 → ↑↑ ≈ 4 + log10(1.5308).
        let sk1 = get("skewes-1");
        assert!(close(sk1[1], 4.1849, 2e-3), "{sk1:?}");
        // Skewes 1955 < 10^10^10^964 → ↑↑ ≈ 4 + log10(log10 964) = 4.4748.
        let sk2 = get("skewes-2");
        assert!(sk2[1] < 4.0 + 964f64.log10().log10() + 1e-6 && sk2[1] > 4.47, "{sk2:?}");
        let mega = get("mega");
        assert!(mega[1] > 257.0 && mega[1] < 258.0);
        // On the ↑↑↑↑ dial: g₁ ≈ 2.36, our limit ≈ 1.15.
        assert!(get("graham-g1")[3] > 2.3 && get("zoom-limit")[3] < 1.2);
    }

    #[test]
    fn facts() {
        let f = big_number_facts(3000.0, 1.6);
        assert!(close(f.speed_factor, 4.953, 1e-3));
        assert!(close(f.seconds_to_googol, 143.9, 0.1));
        assert!(
            close(f.years_to_googolplex / 1e92, 4.56, 0.01),
            "{}",
            f.years_to_googolplex
        );
        assert!(close(f.bits_here, 9965.8, 0.1));
        assert!(close(f.bits_googolplex / 1e100, 3.32, 0.01));
        assert!(close(f.ganson_years / 1e12, 2.32, 0.01), "{}", f.ganson_years);
    }
}
