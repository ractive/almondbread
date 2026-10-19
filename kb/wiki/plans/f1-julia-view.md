---
title: "F1: Julia view (companion inset)"
type: plan
milestone: F1
status: proposed
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
sources: ["[[synthesis/other-fractal-types]]", "[[formulas/julia]]", "[[sources/frax-about]]", "[[sources/kisaka-julia-sets-in-mandelbrot]]", "[[decisions/controls]]"]
tags: [julia, overlay, education, formulas]
created: 2026-10-19
updated: 2026-10-19
---

# F1: Julia view (companion inset)

Status: proposed, awaiting the user's go. First of the fractal-type milestones in
[[roadmap]]; the order and its reasons are in [[synthesis/other-fractal-types]] §5.

## Goal

Show the Julia set of the point under the pointer (Mac) or finger (touch), or of the view
centre while a flight runs, in a small inset next to the mini-maps. Every point `c` of the
Mandelbrot set has a Julia set `z ← z² + c`; seeing it change as you move is the most direct
way to understand what the Mandelbrot set is (src: [[synthesis/other-fractal-types]] §2a
"Julia sets"). It also fixes the gap the Julia tour has today: "No Julia renderer in the app:
the tour uses the orbit overlay and places; a Julia view would improve it" (log 2026-10-19,
[[decisions/controls]] "The Julia tour").

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

- Zooming into the Julia set, deep Julia, perturbation (that is [[plans/f4-deep-julia]]).
- The "Julia twin" of a deep view (same local framing around a Misiurewicz point): needs deep
  Julia.
- Any change to the main `iterate` kernel or the formula model ([[plans/f2-pluggable-formulas]]).
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
  cycle, inference in [[synthesis/other-fractal-types]] §2a); otherwise everything that does not
  escape by the limit is black.
- Layout: one more panel in `fit_maps` (kb: [[decisions/controls]] "Mini-map on small
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

## Effort breakdown

S overall (the research gives S–M; S if the panel reuses the mini-map path as sketched).

- Kernel, pipeline, CPU reference and tests: 1 day.
- Panel, update policy, layout on all screens: 1–2 days.
- Settings, FFI, Swift toggles, help, Julia tour steps: 1 day.
- Verification runs, review fixes: 1 day.
