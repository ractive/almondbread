---
title: "F3: Multibrot z^p + c"
type: plan
milestone: F3
status: in-progress
priority: 3
effort: M
appeal: medium
audience: [casual, artists, enthusiasts]
depth: "unlimited"
realtime-ipad: likely
reuse: "high: every per-formula hook with scalar complex arithmetic; no LA, no closed-form components, no π meter"
depends-on: ["plans/f2-pluggable-formulas"]
risks:
  - "float32 iteration-band glitches at high powers (F3 warns of them in plain float)"
  - "the shapes get repetitive; low novelty at depth"
  - "reference cost grows with p on the CPU"
sources: ["synthesis/other-fractal-types", "formulas/multibrot", "sources/kf-page", "sources/fraktaler-3-site", "techniques/bla"]
tags: [formulas, multibrot, perturbation, bla]
created: 2026-10-19
updated: 2026-10-22
---

# F3: Multibrot z^p + c

Status: built (2026-10-22, branch `worktree-agent-a1fc8ed548ebe2577`), awaiting the review gate
below (the user decides whether Multibrot ships visible); "As built" below. After
[F2: Pluggable-formula refactor](f2-pluggable-formulas.md).

## Goal

The first new formula, chosen because it exercises every per-formula hook (reference, BLA,
oracle, derivative, Newton) with scalar complex arithmetic only. It is "the best test that the
refactor kept the Mandelbrot fast path intact" (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §5).
Users get p-fold symmetric sets, `p = 3..8`, at any depth.

## Scope

- Formula `Multibrot { p }`, `p = 3..8` (integer only).
- Reference orbit in BigFloat; perturbed step from the binomial expansion of `(Z + z)^p − Z^p`;
  BLA with `A = pZ^{p−1}`, `B = 1` and its radius ([Multibrot (integer power p)](../formulas/multibrot.md), [Bivariate linear approximation (BLA)](../techniques/bla.md)).
- Rebasing as for Mandelbrot (one critical point at 0).
- DE and the interior derivative test with `pz^{p−1}`; Newton nucleus and period detection
  (seek minibrot) with the same derivative (inference in [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).
- CPU oracle, f64 reference, orbit overlay for the formula.
- A small places set (10–20 views per power, found with seek), formula picker in the control
  center, capability-driven hiding of LA, π, closed-form components, external rays.

## Non-goals

- Non-integer powers: they "do not reduce at all" for perturbation
  (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a "Transcendental").
- LA (power 2 only, [Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md)), local SA seeds for p > 2
  unless measured to pay.
- Tours and journeys tuned for Multibrot (journeys may run, untuned).

## Design sketch

- `fStep` for formula Multibrot: `z' = Σ_{j=1..p} C(p,j) Z^{p−j} z^j + δc`, evaluated in Horner
  form; `fStepX` the same in floatexp.
- `parallel.rs`: the three-product split serves only squaring; for p > 2 the reference runs on
  one thread, or squares are chained (p = 4, 8 as repeated squaring) (inference).
- BLA table built from `pZ^{p−1}`; skip lengths shorter than Mandelbrot's because the
  validity radius scales with `|Z|^{p−1}` (inference, to be measured).
- Iteration-band guard: compare float32 against the oracle at p = 8 deep; if bands show, use
  floatexp steps near small `|Z|` (F3 warns plain float "gives iteration-band glitches at high
  powers", src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).

## Affected modules

- `almondbread-math`: `orbit.rs` (Multibrot step), `parallel.rs` (dispatch), `selection.rs`
  (derivative for Newton and period).
- `almondbread-core`: `reference.rs`, `oracle.rs`, `orbit.rs`, `bla.rs`, `cycles.rs` and the
  interior probe, `seek.rs`, `places_data.rs` (tagged places), `settings.rs`.
- `almondbread-render`: `mandelbrot.metal` step functions for `kFormula = Multibrot`,
  `metal/mod.rs` pipeline keys.
- `almondbread-ffi`: formula setting, capabilities. Swift: formula picker, hiding.
- `tests/fixtures` + `tests/oracle/generate_fixtures.py`: Multibrot reference fixtures.

## Verification

- Oracle fixtures per power (BigFloat reference orbits vs the Python generator).
- `verify` metrics vs the CPU oracle at 5 views per power (shallow, 1e-20, 1e-100, 1e-300,
  1e-1000), class mismatch within the Mandelbrot tolerance; `precision` at p = 8.
- `verify --history zoom|pan` at two depths, 0 % mismatch vs fresh.
- Mandelbrot fast path: `make bench` A/B geomean ≈ 1.0 and bit-identical fields vs main, as in
  F2 (the new formula must not change formula 0's pipelines).
- `make interaction-test` with a Multibrot view (new scenario), `make selftest`, headless
  stills looked at.

## Review gate

The user explores p = 3..8 on the Mac and the iPad (picker, seek, places). Decide whether
Multibrot ships visible or stays as a hidden test formula, and whether to continue with F4.

## Risks

- Band glitches in float32 at high p (above).
- Repetitive shapes: appeal medium (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).
- Seek and the autopilot target lock tuned on Mandelbrot; may need per-formula thresholds.

## Open questions

- Which powers to offer (all of 3..8, or 3, 4, 5, 8)?
- Does BLA still pay at p = 8, or should the table be off for high powers?

## As built (2026-10-22, awaiting review)

Measured 2026-10-21/22, M1 Pro, release, on a machine shared with other agents (load average
30–85); tables in [almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F3: Multibrot".
Formula facts in [Multibrot (integer power p)](../formulas/multibrot.md) "As built in almondbread (F3)".

- **Already there from F2.** `Formula::Multibrot(p)` (p ≤ 8) through every hook: the BigFloat
  reference (one thread), the generic kernel step `z' = zS + c` with `kFormula`/`kPower`, the
  floatexp step, derivatives, seeds, probe, BLA with `A = pZ^{p-1}` and radius
  `eps 2|Z|/(p-1)`, the CPU oracles, the f64 reference, the orbit overlay, capabilities and the
  Swift `has(\.x)` hiding. The BLA radius held up unchanged at every power and depth tested.
- **Fixed (glitches F2's generic path had).** (1) The 2^16 escape radius overflowed float32 from
  p = 4 (`|z|^2` up to `2^{32p}`): infinite smooth counts, black bands; now
  `R = 2^min(16, 60/p)` everywhere (kernel, references, oracle, probe). (2) The smooth count
  is `1 - log_p ln|z|`. (3) Underflow of `S ~ |Z|^{p-1}` near small `|Z|` (the plan's "band
  glitch" risk): those steps run in floatexp in both deep regimes (GPU threshold
  `2^(-70/(p-1))`, oracle `2^(-500/(p-1))`); F2 showed false-interior blobs (6.6 % class
  mismatch at a p = 5 minibrot at 1e-124) and dark bands. (4) DE / interior derivative
  renormalised every step once `|z| > 1`. (5) Ball period detection for p > 2 cancelled to
  zero in f64 below ~1e-16 (`(|z|+R)^p - |z|^p`): now a binomial sum. (6) The interaction
  test's shown-vs-fresh check always rendered Mandelbrot.
- **Added.** Exact Newton finish for z^p + c; minibrot size `1/(b l^{d/(d-1)})` (our
  derivation); seek for every formula (test point 0.8 of the way to the period-2 bulb,
  centred framing, half-width ring, no airship); `almondbread seek`, `locate --formula`;
  Multibrot oracle fixtures (bit-exact orbits p = 3..8 at 128–1024 bits, mpmath nuclei incl.
  three at 1024 bits); formulas 3..8 selectable; FFI `formula_options()`, `set_formula`
  (goes to the set's home: Reset and the autopilot's way out use it too), `places_for`;
  copied coordinates carry `formula multibrotP` (none for Mandelbrot; labelled text without
  it pastes as Mandelbrot); map clicks snap to the formula's minibrot cusp; the orbit probe key
  includes the formula; the target lock (Mandelbrot's journey finder) is off outside
  Mandelbrot and comes back with it.
- **UI.** View section: "Fractal" (Mandelbrot / Multibrot, from `formula_options()`, so Julia
  is one more Rust entry) and "Power" (z³ … z⁸ + c). Places lists the current formula's places;
  tours, journeys, π meter, Julia panel and the target lock are hidden for Multibrot; seek and
  the orbit overlay stay.
- **Places.** 18, all from automated seek runs (no hand search): per power the whole set, a
  minibrot seek found from it, and one it found beside a Misiurewicz point at 1e-20.
- **Verification.** `verify` at 5 Misiurewicz-centred views per power (1e-5, 1e-20, 1e-100,
  1e-300, 1e-1000 per pixel): class mismatch 0.000–0.015 % (no interior in view); at the 30
  minibrots seek found from them (down to 1e-1499): class mismatch 0.008–0.141 %, false interior
  ≤ 0.020 %, within Mandelbrot's range (feigenbaum 0.15 %). Escape-n differs reaches 80 % at
  the deepest minibrots, but a 0.001-pixel grid shift of the oracle alone changes 8–23 % of them
  and `precision` puts the GPU's error at a ≤ 0.01–0.1 pixel shift (p = 8 at 1e-343 and
  1e-1142; p = 4, 1e-399): float32 noise in chaotic views, no band glitches. `verify --history
  zoom|pan` at z⁵ (3.5e-8) and z⁸ (1.7e-23), and zoom at z³ (8e-29): class mismatch ≤ 0.002 %
  vs the oracle and vs fresh. Interaction test "multibrot5 pan" 77–91 fps, "multibrot8 deep
  zoom" 72–94 fps (Mandelbrot "1e5 zoom" 86 fps in the same run), shown frame = fresh render.
- **Mandelbrot unchanged.** `bench --out` fields and PNGs byte-identical to main (63/63 each);
  GPU ms geomean F3 / main 0.986 over 54 cases (load average 20–160: noise; the driver
  re-runs the A/B on a quiet machine); `regress` OK.
- **Not done / open.** Unsupervised LA for p > 2 stays off (non-goal); journeys, tours and the
  π meter stay Mandelbrot; the Julia panel waits for F4's `z^p + k`; the deepest minibrot
  views leave 3–9 % unresolved at the auto limit (GPU-ext/CPU-int is 0: limit, not error).
  Not verified: a foreground session on the Mac and the iPad (the review gate).

## F3+F4 integration (2026-10-22)

Merged 2026-10-22 (main = F4, ea4aed7, plus F3). The two features did not know about each
other; the merge makes every per-formula hook handle the third case
([Controls: keyboard, pointer and touch](../decisions/controls.md) "Fractal picker and Julia views"):

- Julia stays `z² + k` (power 2): `Formula::bailout()` and `smooth_offset()` give Julia
  Mandelbrot's 65536 and `1 - log2 ln|z|`; the kernel's F3 additions are guarded by
  `kFormula == 1u` (they were `!= 0u`, which would have put Julia, formula 2, on the Multibrot
  floatexp and renormalisation paths) and `deepSmallZ()` is `DEEP_SMALL_Z` for formulas 0 and 2,
  next to F4's per-orbit `LAST(m)`. Formulas 0 and 2 fold to exactly their pre-merge code.
- `Viewport::fitting_for(Julia)` is `julia::home_view` (Reset, the autopilot's way out, the
  navigator's home); `coordinates_text_for` writes F4's `julia re im` line for a Julia view and
  F3's `formula multibrotP` for a Multibrot view; paste reads the `julia` line first, else leaves
  a Julia view and applies the `formula` line.
- The picker lists Mandelbrot and Multibrot only; in a Julia view it shows "Mandelbrot",
  disabled. `Explorer::set_formula` from a Julia view goes back first. Twin and panel stay
  Mandelbrot's; `places_for(Julia)` is empty; a place from a Julia view is a jump that drops the
  way back. The mini-map's whole level of a Multibrot set is its home view (was Mandelbrot's).
- Checks: Mandelbrot `bench --no-verify 1 --out` 84 of 84 files byte-identical to old main;
  Julia renders (rabbit 1e-100, `k = i` 1e-300, 800 × 500) byte-identical to the F4 binary and
  Multibrot renders (z⁵ 3.5e-8, z⁸ 1.7e-23) to the F3 binary; `verify --julia-k` class mismatch
  0.008 % / 0.000 %; `verify --history zoom|pan` at rabbit 1e-100, z⁵ and z⁸: OK, class
  mismatch ≤ 0.005 % vs the oracle. FFI tests `explorer::formula_integration_tests` (copied
  coordinates round trip for Julia and Multibrot views, `set_formula` from a Julia view, Reset
  in each formula).
- Interaction test (1280 × 820 pt, 120 Hz, background window): "julia twin deep zoom" 109.0 fps,
  "julia 1e-100 pan" 117.8, "julia 1e-300 zoom" 117.0, "multibrot5 pan" 117.1, "multibrot8 deep
  zoom" 41.9 / 104.3 / 46.9 in three runs (bimodal: 19–20 ms GPU per frame in the slow runs; the
  kernel itself is as fast as F3's, a 2560 × 1640 z⁸ still 111–113 ms vs F3's 113–122 ms); shown
  frame vs fresh render 0–0.003 % in class. The first run crashed the app: the Places grid
  indexed its tiles through a list that follows the model, so a tile updated after a formula
  change read the new, shorter list with an old index (F3's; now one snapshot for ids and tiles).
- iOS UI tests on the iPad mini simulator: `testFormulaPicker`, `testJuliaTwinAndBack`,
  `testJuliaPanelTapOpensTheSet` pass.

## Effort breakdown

M overall (1–2 weeks).

- Maths, reference, oracle, fixtures: 2–3 days.
- Kernel step functions, BLA: 2–3 days.
- Places, UI picker, capabilities: 1–2 days.
- Verification and review fixes: 2 days.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Multibrot (integer power p)](../formulas/multibrot.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
- [almondbread benchmarks and verification record](../synthesis/benchmarks.md)
- [F2: Pluggable-formula refactor](f2-pluggable-formulas.md)
