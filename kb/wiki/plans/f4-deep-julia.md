---
title: "F4: Deep Julia sets"
type: plan
milestone: F4
status: in-progress
priority: 4
effort: M
appeal: medium
audience: [deep-zoom-creators, artists, education]
depth: "as Mandelbrot views: critical-orbit rebasing matches a direct oracle in float32 to 1e-1000 (measured)"
realtime-ipad: likely
reuse: "high: the critical orbit of k is the Mandelbrot reference orbit of k we already compute"
depends-on: ["plans/f2-pluggable-formulas", "plans/f1-julia-view"]
risks:
  - "Siegel discs stay unresolved (no attracting cycle to prove interior)"
  - "deep Julia zooms repeat themselves (less variety than Mandelbrot dives)"
  - "no seek: there are no minibrots in a Julia set"
sources: ["synthesis/other-fractal-types", "formulas/julia", "sources/fractalforums-zhuoran-rebasing-bla", "techniques/rebasing", "techniques/bla"]
tags: [formulas, julia, perturbation, rebasing]
created: 2026-10-19
updated: 2026-10-22
---

# F4: Deep Julia sets

Status: built, awaiting the user's review ("Step 1 results", "As built" below); planned after [F3: Multibrot z^p + c](f3-multibrot.md), built beside it.

## Goal

Open the Julia set of any `k` as the main view and zoom it as deep as Mandelbrot views. The
F1 inset becomes the entry point ("open this Julia set"). No Apple-native app offers deep Julia
zooms at interactive speed; Frax stops at about 1e14 (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §3).

## Scope

- Formula `Julia { k }` with `k` in high precision (copied from the point under the pointer,
  a place, or the view centre).
- Perturbed step `z' = 2Zz + z²` (no `+δc`), BLA with `B = 0` ([Julia sets](../formulas/julia.md)).
- Two references: the view-centre orbit for the first steps, then rebasing onto the critical
  orbit (the orbit of 0 under `z² + k`), which is exactly the Mandelbrot reference orbit of `k`
  (our observation; rebasing for Julia needs an orbit through 0, src:
  [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #10).
- DE with `dz/dz₀`; interior by the attracting cycle of `k` (period from our existing period
  finder; inference).
- Julia twin: open the Julia set of the current deep view's centre at the same framing.
- Capabilities: no seek, no π, no LA (until measured), no closed-form components.

## Non-goals

- Julia sets of other formulas (Multibrot Julia, Burning Ship Julia) until their Mandelbrot-side
  formula exists.
- A "seek preimage of 0" feature (the nearest analogue of seek; inference). Maybe later.

## Design sketch

- Step 1 is a test, not a feature: at five `k` (cardioid, period-3 bulb, Misiurewicz `i`, a
  Siegel-disc boundary point, outside the set) render deep views against the oracle with
  critical-orbit rebasing. Fractal Zoomer's author reports rebasing "also works in julia sets"
  (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #11), but we have not seen it in
  float32. If it fails, the fallback is one reference per view at the centre with glitch
  correction by re-referencing (slower, bounded depth).
- The scheduler and reference worker gain a second reference slot (centre + critical); the
  critical orbit is cached per `k`.
- BLA table on the critical orbit with `B = 0` (longer skips than Mandelbrot's, inference).

## Step 1 results: the gate (2026-10-21)

Verdict: **go**. Critical-orbit rebasing gives oracle-matching deep Julia pictures in float32
and floatexp at every `k` and depth tried, to 1e-1000 (measured 2026-10-21, M1 Pro, release,
`cargo run --release -p almondbread-core --example julia_gate`; heavily loaded machine).

**Method.** A pixel's delta to the view-centre orbit `Z` (`z_0 = centre`, `c = k`, high
precision) starts at its offset and steps `z' = 2Zz + z²`; Zhuoran's rebase `z ← Z_m + z` goes
to index 0 of the *critical orbit* (the orbit of 0 under `z² + k`, which is the Mandelbrot
orbit of `k`) when `|Z_m + z| < |z|` or the orbit ends, and after the first rebase the pixel
stays on the critical orbit exactly like a Mandelbrot pixel. The example emulates the kernel's
arithmetic on the CPU: plain float32 above 2^-40, floatexp with a float32 mantissa and an int
exponent below. Its oracle iterates every pixel *directly* in high precision (the view's bits +
128) until the pixel's own spacing `|(f^n)'(z_0)| · texel` reaches 2^-24, then in f64: no
perturbation, no rebasing. A second oracle on a grid shifted by (0.013, 0.007) texels measures
chaos (how much a correct picture changes under a sub-pixel shift). A control run keeps only the
switch at the centre orbit's end (no `|Z + z| < |z|` test). Views: 200 × 125 texels near an
N-th preimage of the critical point 0 (backward iteration from 0 with seeded random branches,
N such that the preimage's piece is about a quarter of the view), the centre offset by
(0.23 W, 0.17 H) texels so that the pixels near the preimage pass close to 0 while the centre
orbit does not. Limits `max(4 · bits, 3000) + 2N`.

Class = interior/exterior disagreement, esc = escape iteration differs, |Δμ| = median smooth
count error over pixels exterior in both, against the direct oracle:

| k | depth | shifted oracle (chaos): class / esc / med \|Δμ\| | float32 rebased: class / esc / med \|Δμ\| | rebases per pixel | no `\|Z+z\|<\|z\|` rebase: class / esc |
|---|---|---|---|---:|---|
| cardioid −0.0619 + 0.4848i | 1e-20 | 0 % / 0.07 % / 1.1e-3 | 0 % / 0 % / 1.9e-6 | 1.3 | 0 % / 0 % |
| | 1e-100 | 0 % / 0.10 % / 1.6e-3 | 0 % / 0 % / 6.9e-6 | 1.3 | 0 % / 0 % |
| | 1e-300 | 0 % / 0.01 % / 9.8e-4 | 0 % / 0 % / 2.6e-5 | 1.1 | 0 % / 0 % |
| | 1e-1000 | 0.03 % / 0.15 % / 8.1e-4 | 0 % / 0 % / 4.3e-6 | 1.6 | 0 % / 0 % |
| period-3 bulb (rabbit) −0.12256 + 0.74486i | 1e-20 | 0.22 % / 0.86 % / 1.5e-3 | 0 % / 0.02 % / 2.4e-6 | 1.7 | 0 % / 0.02 % |
| | 1e-100 | 0.22 % / 0.96 % / 1.5e-3 | 0.01 % / 0.02 % / 3.3e-6 | 1.8 | 0.01 % / 0.02 % |
| | 1e-300 | 0.24 % / 1.08 % / 2.3e-3 | 0.01 % / 0.06 % / 9.3e-6 | 1.9 | 0.01 % / 0.06 % |
| | 1e-1000 | 0.19 % / 0.73 % / 6.9e-4 | 0.02 % / 0.02 % / 8.8e-6 | 1.7 | **37.3 %** / 5.9 % |
| Misiurewicz i | 1e-20 | 0 % / 0.94 % / 8.4e-4 | 0 % / 0 % / 9.5e-7 | 2.6 | 0 % / **8.0 %** |
| | 1e-100 | 0 % / 1.03 % / 5.2e-4 | 0 % / 0.01 % / 2.2e-6 | 2.6 | 0.02 % / 1.5 % |
| | 1e-300 | 0 % / 0.93 % / 6.4e-4 | 0 % / 0.03 % / 2.9e-6 | 2.7 | **3.1 %** / **19.5 %** |
| | 1e-1000 | 0 % / 1.29 % / 1.1e-3 | 0 % / 0.04 % / 9.3e-6 | 3.0 | 0 % / **13.0 %** |
| Siegel disc (golden mean) −0.39054 − 0.58679i | 1e-20 | 0.90 % / 1.45 % / 9.6e-4 | 0.04 % / 0.08 % / 1.4e-6 | 2.6 | 0.04 % / 0.10 % |
| | 1e-100 | 1.05 % / 1.67 % / 3.2e-3 | 0.13 % / 0.15 % / 1.2e-5 | 2.6 | 0.14 % / 0.14 % |
| | 1e-300 | 0.66 % / 0.82 % / 3.3e-3 | 0.08 % / 0.12 % / 2.2e-5 | 1.8 | 0.09 % / 0.11 % |
| | 1e-1000 | 0.64 % / 1.26 % / 5.7e-4 | 0.10 % / 0.10 % / 7.2e-6 | 2.3 | **53.2 %** / 2.5 % |
| outside −0.8 + 0.156i | 1e-20 | 0 % / 40.2 % / 8.7e-2 | 0 % / 12.5 % / 8.0e-5 | 7.1 | 0 % / **29.9 %** |
| | 1e-100 | 0 % / 21.7 % / 3.1e-3 | 0 % / 7.8 % / 6.0e-6 | 4.8 | 0 % / 16.0 % |
| | 1e-300 | 0 % / 25.8 % / 5.6e-3 | 0 % / 10.3 % / 2.7e-5 | 5.5 | 0 % / 14.0 % |
| | 1e-1000 | 0 % / 28.3 % / 1.1e-2 | 0 % / 11.8 % / 6.6e-5 | 5.7 | 0 % / **51.6 %** |

Read: float32 with critical-orbit rebasing is everywhere at least as close to the direct oracle
as the oracle is to itself under a 0.01-texel shift (its escape differences are chaos, not
error), and its smooth counts agree to 1e-5. Without the `|Z + z| < |z|` rebase the pictures
break where pixel orbits pass near 0 while the centre's does not (rabbit and Siegel interiors
lost at 1e-1000, `i` and the dust at several depths). The Siegel-disc pixels are interior to the
oracle (they never escape and never converge) and unresolved on the GPU: no interior test can
prove them (inference). The dust (`k` outside) is chaotic at the pixel level at every depth
(40 % of escape counts change under the shift), so 8–12 % there is chaos.

The real kernel (`almondbread verify --julia-k k --center c --scale s`, the same views, deep
kernel and BLA, against the CPU oracle, which is f64 perturbation with the same rebasing,
itself checked against direct BigFloat iteration in `oracle::tests`): class mismatch 0–0.04 %,
escape-n differs 0–0.14 % for the four connected `k` at 1e-20 … 1e-1000 (Siegel: the disc
unresolved, as above), 7.7–11.8 % for the dust; GPU 0.2–1.4 ms per 200 × 125 view (Siegel 10–40
ms: the disc runs to the limit). Stills: `julia_gate --png dir` writes the oracle, float32 and
no-rebase pictures.

**Precision of `k` (found here).** A decimal `k` is a binary value at the precision its digits
imply (`Center::parse`: 16 digits → 128 bits). A view at 1e-100 sees a different Julia set for
`k` at 128 bits than for the "same" decimal at 400 bits: the set moves by about the difference
in `k`. So `k` is carried exactly (interned, any precision) and links write it with the
centre's digits; the twin's `k` is the view centre itself (inference from the measurement).

## Affected modules

- `almondbread-core`: `reference.rs` (two references), `oracle.rs` (Julia truth), `bla.rs`
  (`B = 0`), `cycles.rs` (interior by `k`'s cycle), `orbit.rs` (overlay of `z₀`'s orbit),
  `settings.rs` (`k` in BigComplex), `places_data.rs` (Julia places), `tour.rs`
  (`formula: julia` step key), the Julia tour.
- `almondbread-math`: `orbit.rs` (reference from `z₀ ≠ 0` with fixed `k`).
- `almondbread-render`: `mandelbrot.metal` step functions for `kFormula = Julia`, scheduler
  reference handling (`scheduler.rs`).
- `almondbread-render/src/overlay.rs`: "open as main view" from the F1 panel.
- FFI and Swift: formula switch, `k` readout, Julia twin action.

## Verification

- The rebasing test above, `verify` vs the oracle at 1e-20, 1e-100, 1e-300, 1e-1000 for the
  five `k`; `precision` (artefact or chaos?) where they differ.
- `verify --history zoom|pan|jump` for a Julia view.
- Mandelbrot fast path: bench A/B geomean ≈ 1.0, bit-identical fields, `make regress`.
- `make interaction-test` with a deep Julia scenario; `make selftest`; stills looked at.

## Review gate

Show the rebasing test's verdict to the user first (go or fallback). Then the user zooms Julia
sets on the Mac and the iPad, including the twin. Decide whether journeys should visit Julia
sets.

## Risks

- Rebasing unproven (above): the main risk; the test comes first.
- Repetition at depth: a Julia set is mapped onto itself by `f`, so dives look alike
  (inference, [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a).
- Two references raise reference memory on the iPad (small at default limits,
  [iOS and iPadOS on a device: first run, signing, memory, thermal, background](../platform/ios-device.md)).

## Open questions

- Does critical-orbit rebasing hold in float32? (open question in
  [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md))
- Is the twin framing useful away from Misiurewicz points?

## As built (2026-10-22, awaiting review)

Built on branch `worktree-agent-aad72e725c0292486` after the step-1 gate passed; merged with main
(7f5ec27). Differences from the sketch above, and the measurements.

- **`Formula::Julia(JuliaK)`.** `k` is a `BigComplex` at whatever precision it came with,
  interned by value (`JuliaK`, `almondbread-math/src/formula.rs`): `Formula` stays `Copy + Eq +
  Hash`, so every cache keyed by the formula (the scheduler's job key and reference, BLA tables,
  `set_reference`'s formula check) tells two Julia sets apart and a formula change is a jump.
  Kernel formula 2 shares Mandelbrot's derivative hooks; pipelines of other formulas are now keyed
  by their kernel constants, so every `k` shares one set. The orbit loops are Mandelbrot's with
  `c = k`; `ReferenceOrbitComputer::with_start(z0)` starts the centre orbit at the view centre.
- **One buffer, two orbits (the second reference slot).** A Julia `Reference`
  (`core/src/julia_reference.rs`) is `[critical orbit 0 ..= crit_last][zeros][centre orbit from
  center_start]`, `center_start` a multiple of 4096 (BLA alignment). Rebasing keeps target 0 (the
  critical orbit, as in Mandelbrot); fresh pixels start at `center_start` with `n = 0` and their
  offset as the delta; each orbit ends at its own last index (`IterateParams.critLast`,
  `centerStart`; `segLast` in the kernel). The scheduler needed no second binding or slot: the
  concatenated orbit is uploaded, tabled and reused like any reference. The critical orbit is
  cached per `k` (two kept), computed at the centre orbit's precision on three threads where that
  pays. f64 references (views above 1e-12) do the same in f64. Both orbits are computed whole (no
  progressive prefixes); `exact_reach` and `truncated_before` look at both.
- **Kernel** (`mandelbrot.metal`, `kJulia = kFormula == 2`): `fStep`/`fStepX` without `+c`; the
  DE derivative `dz/dz_0` (starts at one texel, gains none per step); the interior test's `n²`
  guard skips `n = 0`. Formula 0 folds every change away (see the A/B below).
- **BLA with `B = 0`** (`bla.rs`): single steps `A = 2Z`, `B = 0`, so `K = 0`; the table is built
  on the concatenated orbit, where the zeros invalidate every block that would straddle the two
  orbits, and the kernel stops a block at its orbit's end. The forecast pays at depth (rabbit at
  1e-100: 22.7× fewer steps than iterations).
- **Not built (capabilities hide them):** LA, local SA seeds (their expansion is in `c`), the
  attracting-cycle probe and the interior cycle analysis (both follow `c`-plane derivatives; the
  interior is flat), closed-form components, seek, π meter, rays, the orbit overlay, places,
  tours, journeys, the Julia panel. Interior is the derivative test: every interior pixel falls
  into `k`'s attracting cycle, and `|dz/dz_0|` shrinks; the oracle stops a pixel within 1e-9 of
  the cycle (`julia::attracting_cycle`). Siegel discs (no attracting cycle) stay unresolved.
- **Auto iterations** grow linearly with depth for Julia sets (`julia::depth_iterations`:
  1000 + 40 per decade; Mandelbrot's `300 + 120 d^1.6` ran 200 000 at 1e-100, where escapes end
  near 4 000).
- **Entry points** ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Deep Julia sets: open, twin, way back"): a click or tap on the
  Julia panel opens its set whole (the panel's `k` is kept while the pointer is over the panel);
  the Julia twin (⇧U, View ▸ Julia Twin of This View ⇧⌘U, control center) opens the Julia set of
  the view's centre at the same framing; ⇧U again, the HUD's back button, the control center or
  the View menu go back to the view it came from. `Host::open_julia`, `julia_twin`,
  `back_from_julia` hold the policy. R frames the whole Julia set. The HUD shows `k`.
  Added 2026-10-23: **⌘-click** on any point of a Mandelbrot view opens that point's Julia set
  the same way (`Host::open_julia_at`), so the pointer need not cross other points on its way to
  the panel ([Controls: keyboard, pointer and touch](../decisions/controls.md) "⌘-click: open a
  point's Julia set").
- **Links and state.** Coordinates text gains `julia re im` (k to the centre's digits + 2: the set
  moves with `k` by about the change in `k`); pasting it opens that Julia view, a text without it
  leaves one. FFI `ExplorerSettings` carries `julia_re`/`julia_im` (round-trip digits; a round
  trip keeps the same `k`), so a restored session opens the Julia set whole. CLI: `--julia-k re,im`
  (`--julia 1` stays F1's panel).
- **Fix found on the way:** a full level whose interior fill waited for its seeds' tile count
  never started when the formula has no seeds (the Julia view at 1e-100 hung in the interaction
  test). It now waits only when a seeding started (Mandelbrot unchanged).

**Measurements** (M1 Pro, release, a shared machine: load average 11–70 during the runs).

- Verify (real kernel vs the CPU oracle, 200 × 125, the gate views): class mismatch 0–0.04 %,
  escape-n differs 0–0.14 % for the four connected `k` at 1e-20 … 1e-1000; the dust 7.7–11.8 %
  escape-n (chaos). Render tests: `scheduler::julia_tests` (rabbit whole, rabbit 1e-100, `i`
  1e-300 vs their oracles; a new `k` never reuses the old reference; Mandelbrot after Julia sets
  bit for bit), `oracle::tests::deep_julia_oracle_matches_direct_iteration` (384 / 384 pixels
  agree with direct BigFloat iteration at 8.3e-100 and 8.3e-300, |Δμ| ≤ 1.2e-7).
- `verify --history zoom|out|pan|jump` at rabbit 1e-100, `i` 1e-300 and cardioid 1e-1000
  (1280 × 820, auto iterations): 12 of 12 OK, class mismatch vs the oracle ≤ 0.007 %, vs a fresh
  render 0 % (rabbit pan 0.007 %).
- Mandelbrot unaffected: `bench --out` fields and PNGs byte-identical to main (126 / 126 plain;
  82 / 84 shaded + period, the two others `elephant-1600`, which differs run to run on main too);
  GPU ms geomean F4/main 1.0007 over 42 cases ([almondbread benchmarks and verification record](../synthesis/benchmarks.md) "F4"); `make regress` OK.
- Frame pacing (`make interaction-test`, background window 1280 × 820 pt at 2x, 120 Hz panel, load
  average 115–190): "julia twin deep zoom" (twin of the seahorse view zoomed 1e-12 → ~1e-40) 111
  fps, interval p50 / p95 8.3 / 16.7 ms; "julia 1e-100 pan" 117.7 fps, 8.3 / 8.3 ms; "julia
  1e-300 zoom" 114.7 fps, 8.3 / 8.3 ms; the Mandelbrot counterparts in the same run 109.8 / 117.3
  / 111.8 fps. Shown frame vs fresh render 0–0.003 % in class. Draw CPU p50 0.28–0.37 ms, GPU
  4.0–5.1 ms per frame.
- Stills at 1600 × 1000 (auto iterations): GPU 7–14 ms per picture at 1e-100 and 1e-1000 for the
  cardioid, rabbit, `i` and the dust; the Siegel disc 0.4–1.0 s (its inside runs to the limit,
  unresolved). References: 0.1–10 ms at 1e-100, 0.13–2.1 s at 1e-1000 (the cardioid's interior
  centre orbit runs to 163 608).

**Unverified.** Julia sets on the iPad (memory: two orbits, ~2× a Mandelbrot reference at the
same limit; the critical orbit is cached) and on a device; the twin's usefulness away from
Misiurewicz points; GPU timing on a quiet machine (the driver re-runs it).

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
  kernel itself is as fast as F3's, a 2560 × 1640 z⁸ still 111–113 ms vs F3's 113–122 ms; F3
  alone is bimodal too: the gesture-frame mode decision sits on its threshold at this view,
  [almondbread benchmarks and verification record](../synthesis/benchmarks.md) "z⁸ deep zoom frame-rate swings"); shown
  frame vs fresh render 0–0.003 % in class. The first run crashed the app: the Places grid
  indexed its tiles through a list that follows the model, so a tile updated after a formula
  change read the new, shorter list with an old index (F3's; now one snapshot for ids and tiles).
- iOS UI tests on the iPad mini simulator: `testFormulaPicker`, `testJuliaTwinAndBack`,
  `testJuliaPanelTapOpensTheSet` pass.

## Addendum: the Mandelbrot panel (2026-10-22, awaiting review)

The user found no way back from a Julia view without menus or the control center. In a Julia
view the Julia panel's place now shows the Mandelbrot view the set came from with a crosshair at
k, and a click or tap there goes back
([Controls: keyboard, pointer and touch](../decisions/controls.md) "Mandelbrot panel: the way
back on the canvas"). On at every entry, also with the Julia panel off; U hides it.

- **Renderer:** `render/src/mandelbrot_panel.rs`, a small `Renderer` like a mini-map level at the
  panel's pixel size, stepped with the maps; `Host::open_julia` keeps the way-back view's
  reference and offers it (`Renderer::offer_reference`, the spare slot, taken only on
  `Fit::Good`), so a deep twin's panel does not recompute it.
- **Tests** (render, Metal): `panel_click_entry_shows_the_way_back_and_returns` (panel in the
  Julia panel's rect, marker at k to 1e-6 px in a turned view, U hides and shows it, the
  accessibility button, the click back gives the way-back viewport bit for bit);
  `deep_twin_shows_the_panel_without_a_stall` (1e-32 twin: on by default with the Julia panel
  off and after being hidden in the previous Julia view, the panel takes the view's own
  reference, settles in 85–142 refreshes; the panel's longest call 1.0–3.6 ms over five runs
  at load average 40–60 (measured 2026-10-22, M1 Pro, release)). iOS
  `OverlayLayoutUITests.testMandelbrotPanelTapGoesBack`.
- **Frame pacing** (`make interaction-test`, 1280 × 820 pt background window, 120 Hz, load
  average 126–165): twin of c = i at 2e-32 per point zoomed in, panel on from the start 112.6
  fps (p50 / p95 / max interval 8.3 / 8.3 / 20.8 ms), panel switched on as the zoom starts (its
  deep picture renders during the measured frames) 115.4 fps (8.3 / 8.3 / 25.0), panel off 111.3
  fps (8.3 / 8.3 / 58.3); draw CPU p50 0.16–0.17 ms in all three; "julia twin deep zoom" (now with
  the panel) 117.3 fps. No cost above the noise.

## Effort breakdown

M overall (1–2 weeks).

- Rebasing test and verdict: 2 days.
- Two-reference scheduling, kernel steps, BLA: 3 days.
- Oracle, places, UI, twin: 2 days.
- Verification and review fixes: 2 days.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Julia sets](../formulas/julia.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [Rebasing (Zhuoran, 2021)](../techniques/rebasing.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
- [Controls: keyboard, pointer and touch](../decisions/controls.md)
- [almondbread benchmarks and verification record](../synthesis/benchmarks.md)
