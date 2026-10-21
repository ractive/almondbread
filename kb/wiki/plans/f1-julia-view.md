---
title: "F1: Julia view (companion inset)"
type: plan
milestone: F1
status: done
priority: 1
effort: S
appeal: high
audience: [casual, education, artists]
depth: "~1e5 float32 (shallow by design); ~1e12 with double-float, not planned"
realtime-ipad: "yes"
reuse: "high: the mini-map's panel renderer path, palettes, colouring, overlay layout; one new direct kernel"
depends-on: []
risks:
  - "no room for another panel on an iPhone"
  - "a live Julia under the pointer could cost frame budget during gestures"
  - "users may expect to zoom the inset deeply (that is F4)"
sources: ["synthesis/other-fractal-types", "formulas/julia", "sources/frax-about", "sources/kisaka-julia-sets-in-mandelbrot", "decisions/controls"]
tags: [julia, overlay, education, formulas]
created: 2026-10-19
updated: 2026-10-19
---

# F1: Julia view (companion inset)

Status: built on branch `worktree-agent-af944c8f0b4aa1b2c` (2026-10-19), awaiting the user's
review gate; "As built" below. First of the fractal-type milestones in [Roadmap](../roadmap.md); the order
and its reasons are in [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §5.

## Goal

Show the Julia set of the point under the pointer (Mac) or finger (touch), or of the view
centre while a flight runs, in a small inset next to the mini-maps. Every point `c` of the
Mandelbrot set has a Julia set `z ← z² + c`; seeing it change as you move is the most direct
way to understand what the Mandelbrot set is (src: [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a
"Julia sets"). It also fixes the gap the Julia tour has today: "No Julia renderer in the app:
the tour uses the orbit overlay and places; a Julia view would improve it" (log 2026-10-19,
[Controls: keyboard, pointer and touch](../decisions/controls.md) "The Julia tour").

## Scope

- An inset panel with the filled Julia set of `k`, framed on `|z| ≤ 2` (a fixed whole-set
  framing, like the first mini-map level).
- `k` follows the pointer (Mac hover), the long-press point or the probe (touch), and the view
  centre while A, C or a journey flies.
- Escape-time colouring with the current palette and look; interior black, or the attracting
  cycle's colour when `k` lies in a hyperbolic component (the period comes from the orbit probe
  we already run for `k`).
- Off by default; a setting under Overlays, a key on the Mac, a round-button toggle on touch.
- Tour step key `julia: on|off` and the Julia tour updated to use it (probe path moves `k`).
- Help text and a (?) badge like the other panels.

## Non-goals

- Zooming into the Julia set, deep Julia, perturbation (that is [F4: Deep Julia sets](f4-deep-julia.md)).
- The "Julia twin" of a deep view (same local framing around a Misiurewicz point): needs deep
  Julia.
- Any change to the main `iterate` kernel or the formula model ([F2: Pluggable-formula refactor](f2-pluggable-formulas.md)).
- Exports: overlays never enter exports or the history.

## Design sketch

- Renderer: the mini-maps are "rendered by small renderers of their own (one per level, at the
  panel's pixel size), stepped only while the main view is not in a gesture"
  (`render/src/overlay.rs` header). The Julia panel uses the same panel slot, layout and
  compositing, but its own tiny kernel: direct float32 iteration of `z ← z² + k` from
  `z₀ = pixel`, smooth iteration count, `dz/dz₀` for an optional DE outline. At 256 × 256 and a
  limit of ~1000 that is at most 65 M iterations, well under 1 ms of M1 Pro GPU (our estimate,
  to be measured).
- Update policy: re-render when `k` moves by more than the panel's pixel size in `c`; during a
  gesture, render at half resolution with a short chunk so the main view's budget is kept; full
  resolution once the gesture ends.
- Interior: if the probe classifies `k` as interior with period `p`, iterate the panel pixels
  and test convergence to that cycle (every interior pixel falls into the same attracting
  cycle, inference in [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §2a); otherwise everything that does not
  escape by the limit is black.
- Layout: one more panel in `fit_maps` (kb: [Controls: keyboard, pointer and touch](../decisions/controls.md) "Mini-map on small
  screens"); on an iPhone it replaces the second map level when both are on.

## Affected modules

- `crates/almondbread-render/src/shaders/mandelbrot.metal`: a new `julia_direct` kernel
  function (separate from `iterate`, so the Mandelbrot pipelines do not change).
- `crates/almondbread-render/src/metal/mod.rs`, `gpu_types.rs`: its pipeline and parameters.
- `crates/almondbread-render/src/overlay.rs`: the panel, its update policy, hit testing,
  `fit_maps`.
- `crates/almondbread-core/src/settings.rs` (`OverlaySettings::julia`), `orbit.rs` (period and
  class of `k` from the probe), `tour.rs` (step key), `tours/julia.md`.
- `crates/almondbread-core/src/oracle.rs` or a small `julia.rs`: an f64 CPU Julia for tests.
- `crates/almondbread-ffi/src/explorer.rs`: the setting; `help.rs` text.
- Swift: `ControlCenterView.swift` (toggle), `ContentView.swift` / `TouchLayout.swift` (round
  button, layout), `Help.swift`.

## Verification

- CPU vs GPU: an f64 Julia in core; the panel's iteration field matches it within the float32
  tolerance at five `k` (cardioid interior, period-3 bulb, Misiurewicz `i`, seahorse valley,
  outside the set). Rust test on Metal like the scheduler tests.
- Headless stills: `almondbread render out.png --julia 1 --orbit x,y` at those `k`; look at them.
- Mandelbrot fast path unaffected: the main kernel's source for `iterate` is unchanged; still
  run `make bench` A/B (geomean ≈ 1.0) and compare fields with
  `verify --fields a.iter --against b.iter` (bit-identical) with the panel off and on (overlays
  never enter the field).
- Interaction: `make interaction-test` with the panel on (new scenario: hover sweep across the
  set): frame interval p95 and draw CPU within noise of the panel off.
- `make selftest` (both apps), `OverlayLayoutUITests` on the shared simulator pair for the
  phone and iPad layouts, the Julia tour test.

## Review gate

The user tries the panel in a real Mac window and on the iPad (pointer hover, finger, flights,
the Julia tour) before it is on in any tour by default. Decide there: default on or off, where
the panel sits on an iPhone, whether a "tap to open as the main view" is wanted (that would be
F4's entry point).

Decided by the user on 2026-10-21: the key stays **U**, the panel stays **off by default**, and
"tap to open as the main view" is wanted (it is F4's entry point, built there). The iPhone
placement stays as built (portrait: whole-set map + Julia panel; on its side: the panel alone)
until the user has tried it on a phone; the user had no preference yet.

## Risks

- iPhone space: maps, cockpit, tour card and round buttons already compete (log 2026-10-19
  "Maps, cockpit and tour card keep clear on phones").
- Frame budget during gestures: mitigated by half resolution and short chunks; measured in the
  interaction test.
- Expectation of zoom: the panel says "Julia set of this point", and F4 is the deep version.

## Open questions

- Key on the Mac (J is the journey; candidates: U, ⇧O) and the iPhone placement.
- Should the panel also show the probe's orbit dots on the Julia set (the same orbit lies in
  both pictures)?
- Fixed framing or a slight zoom towards `k` when `k` is near the boundary?

## As built (2026-10-19, awaiting review)

Differences from the sketch above, and the measurements.

- **Kernel**: `crates/almondbread-render/src/shaders/julia.metal`, a library of its own compiled
  through `GpuBackend::library_pipelines` (fast math) when the panel is first shown, so
  `mandelbrot.metal`, the scheduler and the Mandelbrot pipelines are untouched (no bench A/B or
  field comparison needed: the iterate kernel's source and binary path did not change; overlays
  never enter fields). `juliaField` (one sample, the iteration texel as the Mandelbrot kernels
  store it, for the tests) and `juliaColor` (2×2 samples, the main palette at level 0, colour
  density, offset, log and smooth colouring; sRGB into the panel's BGRA8 texture, which the
  overlay kernel reads like a map level). No DE outline and no cinematic hue drift.
- **Interior**: `almondbread_core::julia::attracting_cycle(k)` iterates the critical orbit of k
  (its Mandelbrot orbit) 20 000 steps in f64 and looks for the period (≤ 4096) and |λ|; it does not
  use the orbit probe, so the panel works with the orbit off. With a cycle known, a sample within
  1e-3 of the cycle point stops as interior (16× faster for interior-heavy k: cardioid 1.09 →
  0.07 ms, basilica 0.80 → 0.05 ms for a one-sample field). Interior colour: the main view's flat
  interior colour, or, with any other interior mode, bands of the time to reach the cycle (one
  palette step per period, darkened).
- **Update policy**: a new picture when k moves more than half a panel pixel (z units), the size
  or the colours change; one submission in flight. While k moves: one sample per pixel, one
  dispatch. Once k rests 0.2 s: 2×2, in bands of rows (one per refresh) sized from the one-sample
  picture's time (≈ 0.75 ms each, at most 8). No separate gesture path: the one-sample picture is
  the cheap one.
- **k**: the orbit's point (`Overlay::probe_c`): a tour's probe, else the touch probe, else the
  zoom target (pointer; crosshair in keyboard mode and while the autopilot flies, which is the
  view centre's neighbourhood rather than exactly the centre). f64: at depth the whole view is
  one panel pixel of k, so the panel shows the Julia set of "this place".
- **Framing**: centre 0, 4.2 × 3.0 z units fitted to the panel (`julia::SPAN_RE`, `SPAN_IM`).
- **Layout**: the last panel of the map row, same size and frame as a map; `fit_maps` gets one
  more slot. Map levels go first where the row has no room (iPhone portrait: whole-set map + Julia;
  iPhone on its side: the Julia panel alone). With the panel on, at most three map levels (the
  overlay kernel binds four map textures). The row's union keeps the cockpit, cosmic scale and π
  meter clear as before. Panel id `julia` in `overlay_panels`. Clicks on the panel are taken and
  do nothing. Labels "Julia set" and "c = re ± im i" (four decimals); help badge topic `julia`.
- **Controls**: control center Overlays "Julia set of that point"; View ▸ Julia Set (⌥⌘U); Mac key
  **U** (also an iPad hardware keyboard); Mac toolbar toggle (SF Symbol `seal`); round button on the
  iPad's top row only (the phone's column and row have no room for a sixth). Persisted with the
  overlays, off by default and on Reset. Tour key `julia: on|off`; tours/julia.md uses it (steps
  1–6 and 8; off at "Julia sets inside", where the whole Julia set does not show the embedded
  shape). CLI `render --julia 1 [--orbit x,y]`.
- **Measurements** (M1 Pro, 2026-10-19). GPU per picture at 360 × 259 px (a 180-point Retina
  panel), unit test `panel_redraws_only_when_k_moves`: one sample 0.06–0.10 ms for typical k
  (basilica, rabbit, cardioid, dust), 1.05–1.1 ms near-parabolic (−0.75 + 0.01i); 2×2 0.22–0.43 ms
  in one band, 6.3 ms in 8 bands near-parabolic (slowest band 1.4–1.9 ms; GPU clocks low in the
  test, pessimistic). Interaction test (1280 × 820 window, 120 Hz, background, noisy machine): "sweep"
  (pointer across the set, orbit + maps) 116 / 111 fps vs "julia sweep" 117 / 109 fps, interval
  p95 8.3 ms both; "overlays pan" 114 / 112 fps vs "julia pan" 115 / 111 fps, p95 8.3 / 16.7 ms in
  both runs of either; draw CPU p50 +0.05 ms. Within the run-to-run noise.
- **Tests**: `julia::tests` in core (cycles of the basilica, rabbit, cardioid, none for 0.5 + 0.5i
  and i; field shapes and symmetry); render `gpu_field_matches_the_cpu_reference` (six k: class
  differs in 0 pixels except seahorse 62 of 93 240 (just outside the set, chaotic), escape
  iteration in ≥ 98 %, capture changes no pixel's class, MSL struct size 96);
  `julia_panel_takes_the_last_place_of_the_row`; tour `julia_key_switches_the_panel`; iOS
  `OverlayLayoutUITests.testJuliaPanelKeepsClearOfTheControls` and `TouchControlsUITests` (the
  iPad's Julia toggle) pass on the iPhone 17e and the iPad mini.

## Effort breakdown

S overall (the research gives S–M; S if the panel reuses the mini-map path as sketched).

- Kernel, pipeline, CPU reference and tests: 1 day.
- Panel, update policy, layout on all screens: 1–2 days.
- Settings, FFI, Swift toggles, help, Julia tour steps: 1 day.
- Verification runs, review fixes: 1 day.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Julia sets](../formulas/julia.md)
- [Frax: What is Frax?](../sources/frax-about.md)
- [Kisaka: Julia sets in the Mandelbrot set and small Mandelbrot sets (seminar abstract)](../sources/kisaka-julia-sets-in-mandelbrot.md)
- [Controls: keyboard, pointer and touch](../decisions/controls.md)
