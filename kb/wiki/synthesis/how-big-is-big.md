---
title: "How big is big? Zoom depth against huge numbers, one dial per up-arrow"
type: synthesis
date: 2026-10-15
sources: ["[[sources/wikipedia-knuth-up-arrow-notation]]", "[[sources/wikipedia-tetration]]", "[[sources/wikipedia-grahams-number]]", "[[sources/wikipedia-kruskal-tree-theorem]]", "[[sources/wikipedia-steinhaus-moser-notation]]", "[[sources/wikipedia-skewes-number]]", "[[sources/wikipedia-googolplex]]", "[[sources/wikipedia-names-of-large-numbers]]", "[[sources/wikipedia-fast-growing-hierarchy]]", "[[sources/wikipedia-observable-universe]]", "[[sources/make-ganson-machine-with-concrete]]"]
tags: [how-big, big-numbers, up-arrow, product, ui, removed]
---

# How big is big? Zoom depth against huge numbers, one dial per up-arrow

> **Removed from the app on 2026-10-15.** The user's reasons: "It's an own window, which is not
> like the rest; way too much text; not sure if people understand it." The panel, its window,
> menu item, control-center button, the cosmic scale's "How big is big? →" link
> (`Explorer.cosmicHint()`) and the Rust maths (`bignum_scale.rs`, `how_big.rs`) are gone. The
> maths below stays here because it may be useful for achievements later. The last commit
> with the code is `ab82452`
> (`git show ab82452:crates/almondbread-core/src/bignum_scale.rs`, tests included).

The user's idea ([[ideas]]): compare our zoom (up to about 10^3008, a 3009-digit number) with
googol, googolplex, Graham's number and TREE(3). Comparing values makes no sense, so the panel
compares the *kind* of size with Knuth's up-arrows, as a gear train with one dial per arrow
level. The metaphor is Arthur Ganson's "Machine with Concrete": twelve 1:50 worm reductions
driven at 200 rpm, whose last gear is set in concrete and would turn once in about 2.3·10^12
years (src: [[sources/make-ganson-machine-with-concrete]]).

As built 2026-10-15 (removed the same day, see above): maths in
`almondbread-core/src/bignum_scale.rs`, FFI in `almondbread-ffi/src/how_big.rs`, UI in
`Sources/AlmondbreadKit/HowBigView.swift`.

## The dials and the fractional convention

Dial k reads the arrow height `h_k(x)`: the real `h` with `10 ↑^k h = x`
(notation: src: [[sources/wikipedia-knuth-up-arrow-notation]]). Dial 1 is `log10` (digits), dial
2 the super-logarithm (height of a tower of tens), dial 3 the height in towers of towers.

Fractional heights use the **linear approximation**. Tetration for real heights is
`10↑↑t = 1 + t` on `(-1, 0]` and `10^(10↑↑(t-1))` above (src: [[sources/wikipedia-tetration]]
§Linear approximation for real heights). Its inverse, applied at every arrow level (our own
generalisation):

```text
h_1(x) = log10 x
h_k(x) = x - 1                    for 0 < x <= 1       (k >= 2)
h_k(x) = 1 + h_k(h_(k-1)(x))      for x > 1
```

Properties (our own, tested in `bignum_scale.rs`): `h_k(10 ↑^k n) = n` for whole `n`
(`10↑↑2 = 10^10` reads 2); every dial reads 0 at zoom 1 and 1 at zoom 10
(`10 = 10↑1 = 10↑↑1 = 10↑↑↑1`); on `(1, 10]` every dial `k ≥ 2` reads `log10 x`; continuous and
increasing, with kinks at the integers. Other conventions (smooth super-logarithms) change the
decimals, never the whole numbers.

Numbers beyond f64 are towers `10^10^…^top` with a real-valued count of tens, normalised to
`top ∈ (1, 10]`; then `h_2 = height + log10 top`, and dial `k ≥ 3` runs in f64 from there. The
zoom enters as its log10, so 10^3000 never overflows. Towers of threes (`3↑↑n`) are built by
iterating `3^x` until the top settles at f64 precision, then adding the remaining levels.

## Where things sit

Computed by `big_number_markers` (2026-10-15; values to 3 decimals):

| Number | ↑ (digits) | ↑↑ | ↑↑↑ | ↑↑↑↑ | ↑⁵ |
| --- | --- | --- | --- | --- | --- |
| googol `10^100` | 100 | 2.301 | 1.362 | 1.134 | 1.055 |
| our deepest zoom ≈ `10^3008` | 3008 | 2.541 | 1.405 | 1.148 | 1.060 |
| googolplex `10^10^100` | 10^100 | 3.301 | 1.519 | 1.181 | 1.072 |
| Skewes 1933, `e^e^e^79` | — | 4.185 | 1.622 | 1.210 | 1.083 |
| googolplexian `10^10^10^100` | — | 4.301 | 1.634 | 1.213 | 1.084 |
| Skewes 1955, `e^e^e^e^7.705` | — | 4.475 | 1.651 | 1.218 | 1.086 |
| mega (`10↑↑257 … 10↑↑258`, placed at 257.5) | — | 257.5 | 2.141 | 1.331 | 1.124 |
| `3↑↑↑3` (tower of 7 625 597 484 987 threes) | — | 7.6·10^12 | 2.311 | 1.364 | 1.135 |
| Graham's `g₁ = 3↑↑↑↑3` | — | — | — | 2.364 | 1.374 |

Sources for the numbers: googol and googolplex (src: [[sources/wikipedia-googolplex]]),
googolplexian is an informal name (src: [[sources/wikipedia-names-of-large-numbers]]), Skewes'
bounds (src: [[sources/wikipedia-skewes-number]]), mega's range (src:
[[sources/wikipedia-steinhaus-moser-notation]]), `3↑↑↑3` (src:
[[sources/wikipedia-knuth-up-arrow-notation]]), `g₁` (src: [[sources/wikipedia-grahams-number]]).
Skewes' numbers are placed at their published upper-bound expressions, not the true crossing.

`g₁` on dial 4 is our own tower arithmetic: `h_3(3↑↑↑n) ≈ n − 3 + h_3(3↑↑↑3)` (each further
`3↑↑` adds one level up to a shift that vanishes after a level), so `h_3(g₁) ≈ 3↑↑↑3` and
`h_4(g₁) ≈ 2 + h_4(h_3(3↑↑↑3)) = 2 + log10 2.311` (good to about 0.001, (unverified) by an
independent source).

The driver's quick numbers (googol ≈ 2.3, googolplex ≈ 3.3, our limit ≈ 2.5 on ↑↑; ↑↑↑ "1.something")
hold under this convention. One correction: dial ↑↑↑↑ and beyond do **not** read 1.000. Every
dial reads exactly 1 at zoom 10, and the four-arrow dial creeps from 1.11 (zoom 10^10) to 1.148
at our limit; ↑⁵ reads 1.060, ↑⁶ 1.025, ↑⁷ 1.011, ↑⁸ 1.005, tending to 1. The panel says so.

## Beyond every dial

- **Graham's number**: `g_n = 3 ↑^(g_(n-1)) 3`, `G = g₆₄`: the number of arrows itself
  recurses 64 times (src: [[sources/wikipedia-grahams-number]]). Every dial has a fixed arrow
  count; `g₂` already needs `g₁` arrows.
- **Moser's number**: only bounded from above, `moser < 3→3→4→2`, and `≪ G`
  (src: [[sources/wikipedia-steinhaus-moser-notation]]). `3→3→4→2 = g₂` by the Conway-chain
  rules (our reading, (unverified)). With no lower bound in arrow terms we do not put it on a dial.
- **TREE(3)**: "even the number of arrows can't describe it". In the fast-growing hierarchy a
  fixed arrow count is below level ω, `f_ω` is a unary Ackermann function, `f_(ω+1)(64) > G`
  (src: [[sources/wikipedia-fast-growing-hierarchy]]), and TREE grows far faster than `f_Γ₀`
  (src: [[sources/wikipedia-kruskal-tree-theorem]]). The page's own lower bound `A^A(187196)(1)`
  is called extremely weak.

## Facts along the way (recomputed)

At the default zoom speed `e^1.6 = ×4.953/s` (`input.rs`, `zoom_speed: 1.6`), 0.6949 digits per
second:

- A googol zoom after 143.9 s (2 min 24 s; the brief's "~2½ min").
- Our limit, 10^3007.9 (`HOME_SCALE / 2^-10000`, `viewport.rs`), after 4329 s (1.2 h); 10^10
  (↑↑ = 2) after 14 s.
- A googolplex zoom: `10^100 / 0.6949` s = 4.56·10^92 years (Julian years; the brief's ~5·10^92).
- Bits: a reference orbit needs about `log2` of the zoom, 3.32 bits per digit: 9966 bits at
  10^3000 (the app's cap is `2^-10000` per pixel, [[decisions/arbitrary-precision]]). A
  googolplex zoom needs `10^100 · log2 10` = 3.3·10^100 bits, more than the ~10^80 atoms in the
  observable universe (src: [[sources/wikipedia-observable-universe]]) by 10^20.
- Ganson: `50^12 / 200 rpm` = 2.32·10^12 years (src: [[sources/make-ganson-machine-with-concrete]]).

## UI (removed 2026-10-15)

What was built, for the record; none of it is in the app any more.

- A window on the Mac (`Window` scene "How Big Is Big?", View menu item, control-center button
  under View & navigation); it follows the window that opened it and redraws with the ~10 Hz
  stats, so it stays open while zooming. On iOS a sheet from the control center (medium and
  large detents, background interaction up to medium).
- Content: header with the live zoom, a gear train (four brass gears, each turned by its
  dial's reading, meshing into a concrete block), one clock-like dial per level with coloured
  marker pips and a legend, a concrete card for ↑⁵ and beyond, the facts, "Beyond every dial",
  and the small print with the convention and sources. The digits dial turns once per googol.
- Screenshots: `SCENES="howbig howbig-100 howbig-1000 howbig-3000" scripts/ui-screenshots.sh`
  renders the panel offscreen (`ImageRenderer`, 760 pt and 390 pt wide) next to the usual
  window captures; no window comes to the front.
- A gold "How big is big? →" link above the cosmic size scale panel past the Planck length
  ([[decisions/controls]]), placed from `Explorer.cosmicHint()`.
- Lessons: a separate window broke the app's one-window, overlay-and-control-center pattern,
  and the panel needed too much explanatory text for an idea most people would not follow.
  A future take should be one glanceable number or an achievement, not an essay.
