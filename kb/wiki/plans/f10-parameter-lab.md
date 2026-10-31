---
title: "F10: Parameter lab (z → zᵖ + c with three knobs, shallow)"
type: plan
milestone: F10
status: in-progress
priority: 10
effort: S
appeal: high
audience: [casual, education, artists, enthusiasts]
depth: "shallow by design: float32 direct iteration, zoom stops at 8 float32 steps per pixel (about ×1000–1500 from a plane's home view)"
realtime-ipad: likely
reuse: "high: the main colour pass, the history and overlay passes, the Julia panel's panel path and the map-row layout; own kernel library, own Rust modules, own Swift file"
depends-on: ["plans/f1-julia-view"]
risks:
  - "users expect to zoom deep; the lab says where it stops and why"
  - "a non-integer p shows the principal branch's seam; it can read as a bug"
  - "the p plane costs a complex power (log, atan2, exp, sin, cos) per step: previews during drags at window size"
sources: ["formulas/others-feasibility", "techniques/double-float", "plans/f1-julia-view", "decisions/controls", "formulas/multibrot"]
tags: [formulas, multibrot, julia, ui, shallow]
created: 2026-10-31
updated: 2026-10-31
---

# F10: Parameter lab (z → zᵖ + c with three knobs, shallow)

Status: built 2026-10-31 on branch `worktree-agent-a384a02aaa7eea03f`, awaiting the user's review
gate; "As built" below. The product shape was decided by the driver session (2026-10-31); details
refined here.

## Goal

`z → zᵖ + c` has three complex knobs: the start `z₀`, the parameter `c` and the exponent `p`. A
picture fixes two and lets the pixel vary the third (the idea of 2swap's video "Mandelbrot's Evil
Twin" (unverified: the video was not saved as a source)):

- pixel = `c`: a Mandelbrot-like set (also with `z₀ ≠ 0` and `p ≠ 2`);
- pixel = `z₀`: a Julia-like set;
- pixel = `p`: the "X-set", the exponent varying per pixel with `z₀` and `c` fixed.

The lab shows one plane large and all three small, each with a marker at its knob's value, so
moving one knob shows at once what it does to the other two pictures. It is a playground, not a
deep-zoom mode.

## Scope

- A mode of its own, "Parameter lab": the formula picker's own family entry (control center,
  Fractal row); another family leaves it.
- Main view: the chosen plane (c, z₀ or p; a segmented control), panned and zoomed as usual down to
  the precision limit.
- Three plane panels in the map row (drawn like the Julia panel), each with a marker at its knob's
  value; a press or drag in a panel sets the knob; a "show" button makes that plane the main view;
  the main plane's panel shows the main view's rectangle.
- Sliders and number fields for the six numbers, Random (interesting ranges), Reset (`z₀ = 0`,
  `c = −0.75`, `p = 2`).
- Real time while a knob moves, on the GPU.
- Colouring through the main colour pass (looks, lighting, outlines), a smooth count for any `p`.
- The orbit overlay: the orbit of the current knobs.
- iPhone and iPad: the same controls in the control center; the panels follow the map-row rules.

## Non-goals

- Deep zoom: non-integer and complex powers do not reduce to a perturbation formula (src:
  [Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md) §Non-integer and
  complex powers), so the lab iterates directly and stops where float32 does.
- Places, tours, journeys, the autopilot, seek, the π meter, the Buddhabrot looks, interior
  proofs and colourings by period, the Julia panel and the mini-map: hidden in the lab (its
  capabilities), never faked.
- Exports of animations, links that encode the knobs (open question).

## Design sketch

- Core `almondbread-core/src/lab.rs`: the knobs (`LabValues`, clamped to slider ranges), the
  planes (`Plane::{C, Z0, P}`, framing, view limits), `zᵖ` (`exp(p · Log z)`, principal branch;
  repeated multiplication for an integer `p` of 1 … 16 outside the p plane; `0ᵖ` is 0 for
  `Re p > 0` and infinite otherwise), an f64 reference of the iteration (tests, orbit overlay),
  the smooth count, Random, the iteration limit and the precision limit.
- Kernel `shaders/lab.metal`, a library of its own (compiled on entering the lab): `labField`
  writes iteration texels `(n, s)` and DE texels `(DE in texels, normal angle)` exactly as the
  Mandelbrot kernels do, so the existing `colorize` pass colours them; `labPanel` colours a panel
  picture directly (the Julia panel's way).
- Frame loop `scheduler/lab_frame.rs`: while the lab is on it replaces the escape-time job (as the
  Buddhabrot look does): moving → one field per refresh at the resolution the budget affords,
  upsampled bilinearly by the colour pass's preview path; resting → the full field, then a 2×2 one,
  in bands. History and overlay passes as every other frame.
- Overlay `overlay/lab_panels.rs`, host `host/lab_host.rs` (press / drag / release routed like
  the compass's, so the Mac's mouse and iOS's pan and tap reach the panels without new platform
  code), FFI `almondbread-ffi/src/lab.rs`, Swift `ParameterLab.swift`.

### The smooth count for any p

Near infinity `ln|zᵖ| = Re p · ln|z| − Im p · arg z` with `|arg z| ≤ π`, so the escape is
double-exponential with base `Re p`, not `|p|` (the brief said `|p|`; for small `Im p` they are
nearly equal, and `Re p` is the one that is right). The offset is

`s = (1 − log₂(ln|z|) / log₂ k) · w`, `k = max(Re p, 1.5)`, `w = clamp(2 (Re p − 1), 0, 1)`,

with `ln|z|` capped at 80 (float32 overflow). For `p = 2` this is the Mandelbrot kernels'
`1 − log₂(ln|z|)`. Towards `Re p = 1` the escape is no longer double-exponential and no smooth
count exists; the weight fades the offset to the plain count there, which removed a hard seam
across the p plane at `Re p = 1.1` (the first version's clamp). Bailout `|z| > 256` (not 2¹⁶):
`|z|ᵖ` of a large z overflows float32 for large `Re p`.

### The branch seam

`zᵖ = exp(p · Log z)` uses the principal logarithm, whose argument jumps from π to −π across the
negative real axis of z. For a non-integer or complex `p` the orbit's values jump there too, and
the pictures show straight lines and sharp edges where an orbit crosses that axis (in the c plane
of seed 3 of Random: a horizontal edge through the middle). That is the formula, not an error; the
lab's caption says so. An integer `p` has no seam (repeated multiplication, and `exp(p Log z)` is
single-valued then).

## Affected modules

New: `crates/almondbread-core/src/lab.rs`, `crates/almondbread-render/src/lab.rs`,
`crates/almondbread-render/src/shaders/lab.metal`, `crates/almondbread-render/src/scheduler/lab_frame.rs`,
`crates/almondbread-render/src/overlay/lab_panels.rs`, `crates/almondbread-render/src/host/lab_host.rs`,
`crates/almondbread-ffi/src/lab.rs`, `crates/almondbread-cli/src/lab.rs`,
`Sources/AlmondbreadKit/ParameterLab.swift`, `Tests/iOSUITests/ParameterLabUITests.swift`.

Touched (a few lines each): `scheduler.rs` (module, `Renderer::lab`, the frame and offscreen
hooks, `is_settled`), `overlay.rs` (fields, the row, textures, scene), `host.rs` (input, compass
routing, touch spot, reset, journal), `input.rs` (`Navigator::set_lab_plane`: home view and view
limits), FFI `explorer.rs` (capabilities, π meter, journeys, places, formula picker, PNG export),
CLI `commands.rs`, Swift `ExplorerModel.swift` (mirror, picker family), `ControlCenterView.swift`
(the Fractal row), `FractalView.swift` (UI-test value), `SelfTest.swift` (demo scenes),
`InteractionTest.swift` (scenarios).

## Verification

- f64 reference against the float32 kernel at the home views of six cases (the three planes at
  Reset's knobs and at `p = 2.6 + 0.15i`): render test `lab::tests::gpu_field_matches_the_cpu_reference`.
- Precision limit measured, not guessed: `lab::tests::precision_limit` (see "Zoom limit").
- Panels redraw only when another knob moves: `lab::tests::panels_redraw_when_the_other_knobs_move`.
- Host: entering, leaving (the view and journal back), plane switch keeping each plane's view, a
  click setting the knob, Z doing nothing, the zoom clamp: `host::lab_host::tests`.
- Headless stills of the three planes and the panels (`render --lab-plane …`), looked at.
- Mandelbrot fast path unaffected: `mandelbrot.metal` and the escape-time pipelines are not
  touched (the lab is its own library; its frame path returns before the job when the lab is off:
  `Renderer::lab` is None).
- Frame rate while a knob is dragged: `make interaction-test` scenarios `lab drag c z0 plane`,
  `lab drag c p plane`, `lab pan c plane`, `lab zoom c plane`.
- iOS: `ParameterLabUITests.testLabFromThePickerAndThePanels` on the iPhone 17e and the iPad mini.

## Review gate

The user tries the lab in a real window and on the iPad: entering from the picker, dragging the
knobs in the panels and by slider, Random, switching planes, the zoom limit's note. Decide there:
whether the lab gets a menu item or key, whether the knobs belong in links / the clipboard,
whether a float-float kernel (deeper zoom) is worth it, and whether the panels need a help badge.

## Risks

- The p plane's general power is the costly case: previews during drags (measured below).
- Users may expect deep zoom: the note at the limit and the caption say it is shallow.
- The seam can read as a bug: documented in the caption.

## Open questions

- Float-float for the lab: `zᵖ` for complex `p` needs float-float `log`, `atan2`, `exp`, `sin`,
  `cos` ([Double-float (float-float) arithmetic](../techniques/double-float.md) has add / mul / div
  only); it would move the limit from about 8 float32 steps per pixel (~10⁻⁶) to ~10⁻¹³. Worth it
  only if the user zooms into the X-set a lot.
- Knobs in the clipboard text and in the travel journal (today the journal is paused in the lab and
  restored on leaving).
- A help badge and a tour card for the lab.

## Effort breakdown

S overall: core and kernel 1 day, frame path, panels and host 1 day, FFI, Swift and tests 1 day.

## As built (2026-10-31, awaiting review)

Differences from the sketch, and the measurements (measured 2026-10-31, M1 Pro, release build,
while two other agents built and tested on the same machine: GPU numbers are pessimistic).

- **Kernel and colour pass.** As sketched. `labField` computes the derivative with respect to
  the pixel's knob alongside (`d ← p zᵖ⁻¹ d + 1` in the c plane, `d ← p zᵖ⁻¹ d` from 1 in the z₀
  plane, `d ← zᵖ (Log z + p d / z)` in the p plane), so the distance estimate `2 |z| ln|z| / |d|`
  feeds lighting, outlines and the palette filter like the Mandelbrot kernels'. Interior: Brent's
  cycle check (`|z − z_saved| < 10⁻⁶`, saved at powers of two) ends cycling orbits early; the rest
  run to the limit; flat interior colour only. NaN and infinity are tested on the float's bits
  (the library is fast-math, which folds `isfinite` away).
- **Iterations.** Automatic: 300 at a pixel of 10⁻³ (the home views), +150 per decade deeper, at
  most 2000; a manual limit when automatic iterations are off.
- **f64 against float32** (`lab::tests::gpu_field_matches_the_cpu_reference`, 320 × 220 at each
  home view, limit 500): class (escaped or not) differs in 0–30 pixels of 70 400 (≤ 0.04 %), the
  escape count by more than one in ≤ 0.17 %; 99.79–100 % agree.
- **Zoom limit** (`lab::tests::precision_limit`). At a boundary point of each of the six cases,
  float32 fields from 10⁻⁴ to 3 · 10⁻⁸ per pixel against the f64 reference, next to the f64
  reference against itself shifted by a quarter pixel (near the boundary much of the picture is
  chaotic, so "agreement" alone says little), and the share of escaped neighbours with the very
  same `(n, s)` (blocks). Blocks appear at 1–2 float32 steps per pixel (10⁻⁷ at |x| ≈ 0.7: 1.6 %
  to 81 % equal neighbours; 3 · 10⁻⁸: 50–95 %). The GPU's disagreement passes the quarter-pixel
  shift's at about 2–4 steps per pixel (z₀ plane at `p = 2.6 + 0.15i`: 99.1 % vs 99.6 % at
  3 · 10⁻⁷). The limit is set at **8 float32 steps of max(|centre|, 1) per pixel**
  (`lab::LIMIT_ULPS`): 9.5 · 10⁻⁷ per pixel at |centre| ≤ 1, 2.4 · 10⁻⁶ around p = 2.5; there
  every case agrees at least as well as the quarter-pixel shift and no neighbours are equal. From
  a home view on a 2880-pixel-wide window that is about ×1500. Float-float would move it to
  ~10⁻¹³ (open question).
- **Zoom stop.** `Renderer::clamp_lab_zoom` clamps the scale about the point the zoom was heading
  for (inferred from the last refresh's view: exact for a pure zoom), so Space, pinch and the
  wheel stop without drifting; the overlay shows "Zoom ends here: the lab computes with 7-digit
  numbers" for 3 s.
- **GPU cost** (CLI `render --lab-plane … --size 2560x1640`, the full 2×2 field, 5120 × 3280
  texels): c plane at Reset 5.2–5.4 ms, p plane at Reset 9.4–10.2 ms, z₀ plane at c = −0.4 + 0.6i
  10.9–13.0 ms; colour pass 1.5–2.7 ms. Panels (360 × 259, 2×2, all bands; the test's cold GPU):
  c 24 ms, z₀ 3.5 ms, p 9.7 ms, drawn in bands of ~1 ms after the knobs rest; one-sample pictures
  while they move.
- **Frame rate** (`make interaction-test`, 1280 × 820-point window, 2560 × 1640 pixels, 120 Hz
  display, background window; the background CADisplayLink delivered 102–120 callbacks per
  second): dragging c on a circle every refresh over the z₀ plane 96.7 fps (link 104 Hz),
  interval p50 / p95 8.3 / 25.0 ms, 28 % previews; over the p plane 95.7 fps (link 103 Hz),
  8.3 / 25.0 ms, 35 % previews; panning the c plane 113 fps (link 120 Hz), 8.3 / 8.3 ms;
  Mandelbrot "home pan" in the same run 105 fps, 8.3 / 16.7 ms. Draw CPU 0.36–0.39 ms p50 per
  refresh. The "final frame after input stopped" line of the drag scenarios (4.9 s, 9.1 s) is an
  artefact: knob changes are no input events, so it is measured from the scenario's start.
- **Hidden in the lab:** see [Controls: keyboard, pointer and touch](../decisions/controls.md)
  "Parameter lab". The cockpit (driving levels) stays: its pedals are a zoom speed control, the
  autopilot level does nothing there.
- **Tests:** core `lab::tests` (7), render `lab::tests` (3), `host::lab_host::tests` (2),
  `overlay::lab_panels::tests` (1); iOS `ParameterLabUITests.testLabFromThePickerAndThePanels`;
  interaction scenarios `lab drag c z0 plane`, `lab drag c p plane`, `lab pan c plane`,
  `lab zoom c plane`; UI demo scenes `lab`, `lab-z0`, `lab-p`, `lab-reset`.

# Citations

- [Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md)
- [Double-float (float-float) arithmetic](../techniques/double-float.md)
- [F1: Julia view (companion inset)](f1-julia-view.md)
- [Controls: keyboard, pointer and touch](../decisions/controls.md)
- [Multibrot (integer power p)](../formulas/multibrot.md)
- <https://www.youtube.com/@2swap>
