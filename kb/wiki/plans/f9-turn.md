---
title: "F9: The turn between the Mandelbrot and the Julia plane"
type: plan
milestone: F9
status: in-progress
priority: 9
effort: M
appeal: high
audience: [casual, education, artists, deep-zoom-creators]
depth: "as Mandelbrot views: the CPU model matches direct BigFloat iteration to 1e-300, the GPU the oracle to 1e-1000 at 0°, 30°, 60°, 90° (measured)"
realtime-ipad: likely
reuse: "high: F4's two-orbit reference layout and rebasing, Mandelbrot's step and BLA table; one kernel constant (formula 9)"
depends-on: ["plans/f4-deep-julia", "plans/f2-pluggable-formulas"]
risks:
  - "a deep Mandelbrot view turned around its centre (z0 = 0) looks like the Mandelbrot view stretched by 1/cos t until close to 90°"
  - "the attracting-cycle probe is off: turned views keep more unresolved (fog) interior pixels near component boundaries"
  - "zooming into a preimage of the critical point much deeper than its piece can still fool the derivative interior test (as in F4)"
sources: ["formulas/turn", "formulas/julia", "formulas/mandelbrot", "plans/f4-deep-julia", "techniques/rebasing", "techniques/bla", "sources/fractalforums-zhuoran-rebasing-bla", "decisions/controls", "synthesis/benchmarks"]
tags: [formulas, julia, mandelbrot, perturbation, rebasing, bla, turn]
created: 2026-10-31
updated: 2026-10-31
---

# F9: The turn between the Mandelbrot and the Julia plane

Status: in progress (built 2026-10-31 on branch `worktree-agent-aba2c88edf47b24dd`, awaiting the
user's review; "Gate", "As built" below).

## Goal

`z ↦ z² + c` has two complex inputs, the start `z₀` and `c`: a 4-dimensional space. The
Mandelbrot set is the plane where the pixel is `c` (`z₀ = 0`), a Julia set the plane where the
pixel is `z₀` (`c` fixed). A slider turns the view plane from one to the other, live, and a start
value `z₀ ≠ 0` gives the Mandelbrot-like planes in between; both at any depth. The idea is the
user's, after 2swap's video "Mandelbrot's Evil Twin" (unverified: we have not saved or read it).

## Scope

- `Formula::Turn(TurnPlane)`: the angle in hundredths of a degree and an interned pivot
  `(c*, z*)`; the ends stay the existing formulas (0° with `z* = 0`: Mandelbrot, 90°: Julia of
  `k = c*`), so the default path is untouched by construction ([Turned planes of the (c, z0) space (Mandelbrot ↔ Julia)](../formulas/turn.md)).
- Deep zoom: perturbation with `dz₀ = sin t dp`, `dc = cos t dp`; F4's two-orbit reference
  (critical orbit of the reference's `c`, its own orbit from its `z₀`) and rebasing; Mandelbrot's
  BLA table.
- Gate first: the CPU model against direct BigFloat iteration at 0°, 30°, 60°, 90° with
  `z* ≠ 0`, at 1e-30, 1e-100, 1e-300; then the GPU against the oracle at the same views plus
  1e-1000.
- CLI `--turn degrees` and `--z0 re,im` on `render` and `verify` (`--history` too).
- Capabilities: what assumes `z₀ = 0` in the `c` plane is off, never faked.
- UI: the turn slider and the start value in the pinned fractal row of the control center (Mac,
  iPad, iPhone).

## Non-goals

- Multibrot, Mandelbar and Burning Ship turns (their Julia sets do not exist yet; the turn needs
  both ends). The rows are hidden for them.
- Other 2-planes of the 4-space (turning the real and imaginary parts differently, the "x/y"
  slices of 2swap's video): one real angle, the same for both parts.
- Places, tours and journeys in turned planes.

## Design sketch

A view is the plane through a pivot `(c*, z*)` turned by `t`; the pixel at view coordinate `p`
is `c = c* + cos t (p − P)`, `z₀ = z* + sin t (p − P)` with `P = cos t c* + sin t z*`. Turning
re-pivots at the view centre's point (it stays in the middle, turning back is exact), panning
keeps the plane. At 0° the pivot is `(0, z*)`: one plane per start value. The reference at view
coordinate `P_ref` iterates from its `(C, Z₀)`; a pixel `dp` away starts at `z = sin t dp` on the
reference's centre orbit (index `center_start`, `n = 0`) and steps `z' = 2Zz + z² + cos t dp`,
rebasing onto the critical orbit of `C` (index 0) when `|Z + z| < |z|` or an orbit ends.

## Gate (2026-10-31)

Verdict: **go**. Rebasing onto the critical orbit and Mandelbrot's BLA hold on turned planes at
every angle and depth tried (measured 2026-10-31, M1 Pro, release,
`cargo run --release -p almondbread-core --example turn_gate`, 128 × 80 texels).

**Method.** Three pivots: `rabbit-pre` and `i-pre` (`c*` the Douady rabbit's nucleus or the
Misiurewicz point `i`, `z*` an N-th preimage of 0 under `z² + c*` from backward iteration, N
chosen so the preimage's piece pulled back along the plane, `0.25 / |sin t A_N + cos t B_N|`, is
about a quarter of the view), and `i-zero` (`(i, 0)`: a Mandelbrot view of `i` turned). The view
centre is offset from the pivot by (0.23 W, 0.17 H) texels so the centre orbit does not pass
through 0 but pixels near the pivot do. *Direct*: every pixel's own `(c, z₀)` iterated in
BigComplex at twice the view's bits + 128 until its spacing `|dz_n/dp| texel` reaches 2^-24 (or
4 × bits + 2000 steps), then f64; a second direct field on a grid shifted by (0.013, 0.007) texels
measures chaos. *Oracle*: `oracle::cpu_render_deep_with` on the turned reference (extended-range
f64 perturbation and the rebase above, no BLA). *BLA*: the same model taking the deep table's
longest valid block at every index (the strictest use; the kernel looks at aligned indices only).

Class = interior/exterior disagreement, esc = escape count differs, |Δμ| = median smooth-count
error over pixels exterior in both, against direct:

| view | angle | depth | shifted direct: class / esc / med\|Δμ\| | oracle: class / esc | BLA: class / esc / med\|Δμ\| | BLA skip |
|---|---|---|---|---|---|---:|
| rabbit-pre | 0° | 1e-30 | 0.21 / 0.70 / 2.7e-4 | 0 / 0 | 0.01 / 0.01 / 2.4e-7 | 68 % |
| | 30° | 1e-30 | 0.42 / 1.57 / 2.1e-3 | 0 / 0 | 0.02 / 0 / 4.8e-7 | 67 % |
| | 60° | 1e-30 | 0.28 / 0.99 / 6.4e-4 | 0 / 0 | 0 / 0 / 2.4e-7 | 67 % |
| | 90° | 1e-30 | 0.18 / 0.91 / 4.2e-4 | 0 / 0 | 0 / 0.02 / 2.4e-7 | 96 % |
| | 0° | 1e-100 | 0.42 / 1.41 / 9.4e-4 | 0 / 0 | 0.01 / 0.03 / 9.5e-7 | 97 % |
| | 30° | 1e-100 | 0.43 / 1.61 / 2.3e-3 | 0 / 0 | 0 / 0.05 / 2.6e-6 | 98 % |
| | 0° | 1e-300 | 0.08 / 0.14 / 1.5e-2 | 0 / 0 | 0 / 0.01 / 6.1e-5 | 99 % |
| | 60° | 1e-300 | 0.04 / 0.18 / 2.2e-2 | 0 / 0 | 0 / 0 / 9.9e-5 | 99 % |
| | 90° | 1e-300 | 0.36 / 0.98 / 3.4e-3 | 0 / 0 | 0 / 0.05 / 8.8e-6 | 99 % |
| i-pre | 0° … 90° | 1e-30 | 0 / 0.9–1.5 / 5–10e-4 | 0 / 0 | 0 / ≤ 0.02 / ≤ 1.2e-6 | 73 % |
| | 0° … 90° | 1e-100 | 0 / 0.6–1.0 / 5–10e-4 | 0 / 0 | 0 / ≤ 0.01 / ≤ 7.2e-7 | 92 % |
| | 0° … 90° | 1e-300 | 0 / 0.9–1.4 / 1.0–1.2e-3 | 0 / 0 | 0 / ≤ 0.02 / ≤ 1.9e-6 | 97 % |
| i-zero | 0° … 90° | 1e-30 | 0 / 0.9–1.1 / 6–8e-4 | 0 / 0 | 0 / ≤ 0.01 / ≤ 2.4e-7 | 68–84 % |
| | 0° … 90° | 1e-100 | 0 / 0.6–1.7 / 4–8e-4 | 0 / 0 | 0 / ≤ 0.01 / ≤ 2.4e-7 | 91–95 % |
| | 0° … 90° | 1e-300 | 0 / 0.7–1.0 / 7–10e-4 | 0 / 0 | 0 / 0 / ≤ 2.4e-7 | 97–98 % |

(Rabbit views at 60°/90° 1e-100 and 30° 1e-300 lie wholly inside one piece: 0 everywhere.) The
oracle agrees with direct iteration in every pixel's class and escape count; the BLA model to
≤ 0.02 % class and ≤ 0.05 % escape counts, far below the chaos of a 0.01-texel shift; the BLA
skips 67–99 % of the iterations.

### Near the critical point

The first gate run failed `i-zero` at 90° from 1e-100 (100 % class mismatch): a Julia view (or a
plane near 90°) centred on `z₀ ≈ 0`, where turning a Mandelbrot view lands. One step maps a
neighbourhood of 0 to the squared scale (`z₁ = z₀² + c`), so neighbours `Δ` apart end up `Δ²`
apart and the reference must resolve the squared scale: about twice the view's bits. The direct
oracle at the view's bits + 128 lost `z₀²` too (it called the dust interior), the model's
reference at the view's bits diverged from the true orbit before the pixels separated. Fixed
(and these are F4 bugs as well, since 90° is F4's Julia formula):

- **Reference bits** (`julia_reference::near_critical_bits`): along the centre orbit with the
  plane derivative `D₀ = sin t`, `D_{m+1} = 2 Z_m D_m + cos t`, where the view's image (radius
  `R |D_m|`, `R` four times the view's half-diagonal) reaches 0, neighbours end up `ρ texel`
  apart with `ρ = max(2 max(|Z_m| − I_m, 0) |D_m|, |D_m|² texel, cos t)`; `−log2 ρ` bits (plus 8,
  whole limbs) are added and both orbits recomputed. The fit test asks for those bits plus one per
  halving of the scale since. 0 for `cos t ≥ 1/2`. Julia views use big references from 1e-6 per
  point, turned ones from `1e-12 / cos t` (`reference::deep_scale_for`): f64 cannot hold the
  squared scale beyond that.
- **Kernel rescale**: `deepRescale` re-expressed a delta more than 2^-60 below the pixel's offset
  at the offset's exponent, flushing a Julia delta near 0 (`z₀²`) to zero; off for Julia (its
  step has no `+c`).
- **Interior test**: the derivative `dz_n/dz_1` of a Julia or turned pixel now starts after its
  first step (`2 z₀` is tiny at `z₀ ≈ 0`, and every pixel of such a view was "interior").

## As built (2026-10-31)

- **Formula and plane** (`almondbread-math/src/formula.rs`: `Formula::Turn`, `TurnPlane`,
  `turn_cos_sin`; `almondbread-core/src/turn.rs`): the angle in hundredths of a degree (`cos`,
  `sin` exact at 0 and 90°), the pivot interned by value at one precision; `P` computed at the
  pivot's bits + 64, so the centre at `P` gives the pivot exactly. `turn::turned` (the slider),
  `with_start` (the field), `plane_through` (0° and `z* = 0` → Mandelbrot, 90° → Julia),
  `home_view` (R: Mandelbrot's framing where the plane passes nearest `c = −1/2, z₀ = 0`),
  `set_box` (view limits: `|c|, |z₀| ≤ 2.5`), the `turn` line of a link. Kernel code 9 (clear of
  the codes other formula work takes).
- **References** (`reference.rs` `compute_turn_big`, `julia_reference::turn_orbits`): the two
  orbits side by side on two threads (one when `Z₀ = 0`), F4's layout, not cached (`c` changes
  per reference); `turn::extra_bits` adds `−log2 min(cos, sin)` bits near the ends; f64
  references above `deep_scale_for`.
- **Kernel** (`mandelbrot.metal`, `kTurn = kFormula == 9u`, `kTwoOrbits = kJulia || kTurn`):
  `IterateParams.turn = (cos t, sin t)` (152 bytes); at the top `dz0 = sin t dc`, `dc = cos t
  dc`; fresh pixels start like a Julia pixel at `centerStart` with `dz0`; the step and the BLA are
  Mandelbrot's (`fStep`/`fStepX` take formula 9 with 0); the DE derivative starts at `sin t
  texel` and gains `cos t texel` per step and per BLA block (`tsC`). Formulas 0 and 2 fold every
  change away (fields byte-identical below).
- **Oracle** (`oracle.rs`): `turn_pixel` (the CPU model, with an optional BLA table),
  `turn_split`, the turned reference in `OracleReference::compute_for` (with the near-critical
  bits, as Julia's), direct f64 for shallow views (`cpu_render_for`).
- **Capabilities** (`FormulaCapabilities::TURN` = Julia's): perturbation with critical-orbit
  rebasing, scalar BLA, DE, the derivative interior test. **Hidden when turned or `z₀ ≠ 0`**: LA,
  local SA seeds, the attracting-cycle probe and Component Newton, the interior cycle analysis
  (period / bubble colourings fall back), closed-form cardioid and bulb test, seek (N), the π
  meter, external rays, the orbit overlay and the mini-map's dot cloud, places, tours, journeys
  and the autopilot's target lock (content), the Julia panel and twin, the Buddhabrot looks.
- **Journal and links**: journal lines `turn centidegrees c_re c_im z_re z_im bits`; copied
  coordinates carry `turn <degrees> <c re> <c im> <z0 re> <z0 im>` (the centre's point); pasting
  it from any view opens the plane. FFI `ExplorerSettings.turn_degrees` / `turn_pivot` (`bits
  c_re c_im z_re z_im`, exact) for a restored session.
- **Host and FFI**: `Host::set_turn` / `set_start` (a jump shown with gesture frames, the way back
  of a Julia view dropped when the turn leaves it), `Explorer::turn_info`, `set_turn`,
  `set_start`, `reset_turn`.
- **CLI**: `--turn degrees --z0 re,im` on `render`, `verify` and `verify --history`, around the
  point (`--center`, `--z0`).
- **UI** ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Turn: Mandelbrot ↔ Julia"): in the pinned fractal row, under
  the pickers or the Julia view's Back: "Turn: Mandelbrot ↔ Julia" with its degrees and a slider
  0–90° (0.1° steps, live), "Start z₀" with two fields (Return applies) and ↺ (back to the
  Mandelbrot set at the centre's c; disabled when already there). Hidden for the other fractals.

**How the turn relates to the Julia twin.** Both reach the Julia set of the view centre's `c`.
The twin keeps the framing (the Julia set around `z₀ = c`, alike near Misiurewicz points); the
turn rotates about the centre's point `(c, z₀ = 0)`, so at 90° it shows the Julia set around
`z₀ = 0`, the critical point (the picture near `k` at the squared scale). Turning a twin's Julia
view (90°) down rotates about `(k, c)`: at 0° that is the Mandelbrot plane with start `z₀ = c`,
which ↺ brings back to the Mandelbrot set.

**Verify** (real kernel vs the CPU oracle, `almondbread verify --size 200x125` at the gate's
views, `build/turn/verify_all.sh`; measured 2026-10-31, M1 Pro, release): 48 views (three pivots,
0°/30°/60°/90°, 1e-30, 1e-100, 1e-300, 1e-1000): class mismatch ≤ 0.012 %, escape-n differs
≤ 0.072 %, median |Δμ| ≤ 3.6e-5. Before the near-critical fixes `i-zero` at 90° showed 61 %
escape-n differs and 36 % unresolved at 1e-100, and 30°/60° at 1e-30 100 % false interior.
Shallow (0.004 per pixel, 400 × 260, `z₀ = 0.1 + 0.05i`): 0°/30°/60°/89° class mismatch
0.015–0.025 %, the same as the Mandelbrot set's 0.032 % there; unresolved 7.6–8.7 % against
Mandelbrot's 0.5 % (no probe; resolved by compute further).

**Mandelbrot unaffected**: `bench --no-verify 1 --out` fields and PNGs byte-identical to main
4d846fa, 84 of 84 files (plain); with `--shading 0.7 --inside period` 80 of 84, the other two
(`elephant-1600`, `home-100k-3456`) differ between two runs of main as well; GPU ms A/B F9/main
0.994 geomean over three locations × two sizes (sea-1e20, needle-1e305, dendrite-1e300, median
of three alternating runs of five, release both sides).

**Frame pacing** (`make interaction-test` scenarios, background window 1280 × 820 pt at 2x, 120 Hz,
release static libs, load average 28–110; measured 2026-10-31, M1 Pro): "turn slider home" (the
slider dragged 0° → 90° → 0° in 3 s, a new plane every frame) 102.3 fps, interval p50 / p95 / max
8.3 / 8.3 / 108 ms, previews at 0.91 of full resolution; "turned 1e-100 pan" (c = i turned 45°)
109.0 fps, 8.3 / 8.3 / 25 ms; "turn slider 1e-20" 100.2 fps but no new picture while the slider
moves (each angle needs a big reference, which the next angle cancels before it arrives): the
picture updates 0.87 s after the slider rests. Every shown final frame equal to a fresh render
(0 % in class). In the same run "home pan" 115.3, "1e-100 pan" 115.0, "sea-1e20 pan" 98.2,
"julia twin deep zoom" 109.8 fps. First run: every slider frame was a jump that reset what the
scheduler learned, so previews stayed at 1/8 scale (0.12); a step to, from or between turned
planes now keeps it (`scheduler.rs`, `plane_step`).

## Affected modules

- `almondbread-math`: `formula.rs` (`Formula::Turn`, `TurnPlane`).
- `almondbread-core`: `turn.rs` (new), `formula.rs` (capabilities), `reference.rs`,
  `julia_reference.rs`, `oracle.rs`, `bla.rs` (`longest_at`), `viewport.rs`, `journal.rs`,
  `examples/turn_gate.rs` (new).
- `almondbread-render`: `shaders/mandelbrot.metal`, `gpu_types.rs`, `scheduler.rs`, `host.rs`.
- `almondbread-ffi`: `explorer.rs` (`TurnInfo`, settings fields, methods, tests).
- `almondbread-cli`: `options.rs`, `commands.rs`, `precision.rs`.
- Swift: `ExplorerModel.swift`, `ControlCenterView.swift` (`TurnRows`), `ControlSearch.swift`,
  `InteractionTest.swift` (three scenarios), `SelfTest.swift` (`--ui-demo=turn`, `turn-julia`),
  tests `ControlSearchTests`, `TouchControlsUITests.testTurnSlider`.

## Verification

- The gate above; `verify` at the gate's views to 1e-1000 (above).
- `cargo test`: `turn::tests` (ends, exact return, start values, lines, home views),
  `explorer::turn_tests` (FFI: slider, settings round trip, text paste, reset, other formulas).
- Mandelbrot: fields byte-identical, `bench` A/B ≈ 1.0 (above).
- Interaction test scenarios "turn slider home", "turn slider 1e-20", "turned 1e-100 pan".

## Review gate

The user drags the slider on the whole set and at a few depths, types a start value, turns a
Julia twin down, and decides whether the deep behaviour around `z₀ = 0` (stretched Mandelbrot
until close to 90°) wants a different pivot or a non-linear slider.

## Risks

- Deep turns of a Mandelbrot view around `z₀ = 0` change little until close to 90° (the start
  enters quadratically); a richer morph at depth needs `z*` on the Julia set.
- No probe: interior near component boundaries resolves later (fog).
- The derivative interior test can still be fooled by a pixel passing 0 within its spread later
  in its orbit (zooming far into a preimage of 0); unchanged from F4.

## Open questions

- Should turning pivot elsewhere at depth (e.g. at a point of the Julia set) so deep turns morph?
- Should the probe's scan (from the pixel's own orbit, not Component Newton from 0) be enabled
  for turned and Julia views? Its deep loop ends orbits at `last` rather than per orbit.
- 2swap's other slices (`z₀` real part against `c` imaginary part, ...) as more planes?

## Effort breakdown

M overall: gate 1 day, kernel and references 1 day, FFI/UI 1 day, verification and kb 1 day
(as built in one session).

# Citations

- [Turned planes of the (c, z0) space (Mandelbrot ↔ Julia)](../formulas/turn.md)
- [Julia sets](../formulas/julia.md)
- [Mandelbrot set (power 2)](../formulas/mandelbrot.md)
- [F4: Deep Julia sets](f4-deep-julia.md)
- [F2: Pluggable-formula refactor](f2-pluggable-formulas.md)
- [Rebasing (Zhuoran, 2021)](../techniques/rebasing.md)
- [Bivariate linear approximation (BLA)](../techniques/bla.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [Controls: keyboard, pointer and touch](../decisions/controls.md)
- [almondbread benchmarks and verification record](../synthesis/benchmarks.md)
