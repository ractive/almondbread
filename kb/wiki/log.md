# Log

One entry per operation, grouped under `## YYYY-MM-DD` date headings, newest first (OKF §7).
Add entries with `hyalo okf log`; never rewrite old ones.

## 2026-10-23

- **Decision:** F6 Burning Ship built (awaiting review): shown upright as (|x| - i|y|)^2 + c; diffabs perturbation with the fold decided at the delta's scale and a split floatexp reference, so the needle matches the oracle to 1e-1000 in float32 with no needle heuristic; fold-capped matrix BLA; Jacobian Newton; no probe (chaotic bounded interiors stay unresolved); six places; Celtic/Buffalo/Perpendicular, journeys and tour not built; others byte-identical, A/B 1.006

## 2026-10-22

- **Decision:** ⌘-click opens a point's Julia set: on any point of a Mandelbrot view (Mac; iPad with keyboard and pointer) a ⌘-click opens exactly that point's deep Julia set with the way back kept, so the panel no longer changes on the way to it; the event carries ⌘, Host decides (decisions/controls "⌘-click").
- **Decision:** Preview threshold A/B switch: temporary test switch (⌃⌥P, View menu, control center test row) between the 50 ms (default) and 12 ms one-chunk threshold of gesture previews, for the user's foreground comparison; interaction-test variants and proxy numbers in synthesis/benchmarks (z⁸ deep zoom frame-rate swings), key in decisions/controls (Test switches)
- **Decision:** Mandelbrot panel: in a Julia view the Julia panel's place shows the Mandelbrot view the set was opened from with a crosshair at k; a click or tap goes back (back_from_julia). On at every entry into a Julia view, also with the Julia panel off; U hides it. Own small renderer, the way-back reference reused at depth; 1e-32 twin: panel call at most 3.6 ms, frame pacing unchanged (decisions/controls, plans/f4-deep-julia addendum).
- **Decision:** Restored control center: iOS scene restoration wrote true into the inspector binding at launch and the pane sometimes never came up while the round button read shown (testFormulaPicker failing on iPad); the iOS binding now takes only false, the pane starts closed; regression test testControlCenterAfterRelaunch (platform/ios-simulator)
- **Decision:** BB Buddhabrot and Nebulabrot built: GPU walkers (uniform and Metropolis–Hastings with recheck/resampling and warm-up), measured tone levels, Nebulabrot RGB; live mode measured on M1 Pro (116–118 fps at 120 Hz at the whole set, falls back where the live accumulation is noise), realtime-ipad likely; whole set clean in 1.6 s, seahorse valley ~1 min, 1e-3 flat; Mandelbrot fields byte-identical, A/B 0.992; plan in-progress awaiting review
- **Decision:** F5 Mandelbar built, awaiting review: the non-conformal path as Wirtinger-form real-linear maps (p d + q conj d) for Jacobians, matrix BLA and Jacobian Newton; probe on even periods; CPU gate passed; no false interior to 1e-1000; Mandelbrot fields byte-identical, bench A/B 1.003 after moving the non-conformal BLA block (its first position slowed plain Mandelbrot pipelines 5-17 %); six seek-found places; interface for F6 in plans/f5-mandelbar As built
- **Decision:** Leftovers after F3+F4: (1) the z⁸ deep zoom's 45 vs 105 fps swings are the gesture-frame mode decision (one K = maxIter chunk below a 50 ms estimate, else progressive) sitting on its threshold with an estimate the settle phase leaves behind; F3 alone shows them too; a 12 ms fallback measured (110 fps, sea-1e20 zoom +12 fps) but not adopted, it changes the feel of deep gestures. (2) The Julia panel's toolbar and round-button switches and U are hidden in Multibrot views (Rust's capabilities, kept in Julia views for the Mandelbrot panel); the twin was already off there. (3) The orbit probe's key is exact (was 1/64 point): the colours tour's path end shows f64's 594 steps (was 959, 2 158, 3 749). See [almondbread benchmarks and verification record](synthesis/benchmarks.md) and [Controls: keyboard, pointer and touch](decisions/controls.md).
- **Research:** F3+F4 Mandelbrot A/B: quiet-machine bench of merged main vs pre-F4 main, geomean 0.977, byte-identical; see synthesis/benchmarks
- **Decision:** F3+F4 integration: Multibrot merged onto deep Julia sets; per-formula hooks handle Julia (power 2, bailout 65536, julia home view, julia line in coordinates), the picker is disabled in a Julia view, set_formula leaves Julia first; Mandelbrot, Julia and Multibrot renders byte-identical to their parents
- **Decision:** F3 Multibrot built: Multibrot z^3..z^8 + c selectable with a Fractal/Power picker, seek and 18 seek-found places; fixed F2's float32 escape overflow (bailout 2^min(16,60/p)), smooth count (log_p), small-|Z| underflow (floatexp steps), and the ball period bound's cancellation; class mismatch <= 0.14 % to 1e-1499, Mandelbrot fields byte-identical (A/B 0.986, noisy). Awaiting the user's review gate.

## 2026-10-21

- **Decision:** F4 deep Julia sets built, awaiting review: step-1 gate passed (critical-orbit rebasing in float32 and floatexp matches a direct high-precision oracle at five k from 1e-20 to 1e-1000, closer than the oracle to itself under a 0.01-texel shift; without the |Z+z|<|z| rebase up to 53 % class mismatch). Built Formula::Julia with k interned in high precision, a two-orbit reference (critical orbit cached per k, then the centre orbit) in one buffer, the kernel's Julia path (no +c, dz/dz0 DE), BLA with B = 0, Julia oracles; entry points: Julia panel click, Julia twin (shift-U, View menu, control center), the way back (HUD, control center, shift-U), links with a julia line. Mandelbrot fields byte-identical to main, GPU geomean 1.0007; deep Julia flights 111-118 fps at 120 Hz. Pages: plans/f4-deep-julia (Step 1 results, As built), formulas/julia, decisions/controls, synthesis/benchmarks, roadmap row F4.
- **Update:** Density pivot checked by the user in a real Mac window: the density slider now behaves as expected ([Coloring methods](techniques/coloring-methods.md), "Density pivot (as built)").
- **Decision:** F1 Julia panel reviewed by the user: key U, off by default, tap-to-open wanted (F4); iPhone placement kept as built until tried on a phone. F1 done ([F1: Julia view (companion inset)](plans/f1-julia-view.md)).
- **Decision:** Tours end with Esc twice within 2 s; the first Esc stops motion like any Esc and the card says "Press Esc again to end the tour" (one Esc was too harsh, and the welcome tour's "press Esc" ended the tour). See [Controls and interaction design](decisions/controls.md).
- **Decision:** Density pivot: user edits of the colour density move the offset so the colour at the view's median escape count stays (no scrambling at depth); techniques/coloring-methods
- **Ingest:** Spiral Galaxy: first hand-found place (spiral-galaxy, 1e22.6, period-461 core, spectrum look); span = app scale x window short side; App Store drafts via --ui-demo=place-<id> and AppStoreShotsUITests.test10SpiralGalaxy
- **Decision:** Pinned place details: the control center keeps the selected place's name, description and Go / Fly / Tour in a bar at its bottom (decisions/controls)

## 2026-10-20

- **Decision:** Review fixes M1, E1, M2, M3, M4, P1, U1, C5, X1: debug-build orbit panic fixed and a debug-profile test run (make rust-check-debug, CI test-debug); DoubleExp exponents saturate; iteration limits capped at 2^24 (float32 escape count); pasted coordinates at most 20 000 digits and MAX_CENTER_BITS; pans saturate in Scale::big_times; orbit helpers only with 6+ performance cores, sleeping after 2 ms stalls; FFI copies return errors; MetalEncoder ends its pass on drop; CLI rejects a --key as a value. Pages: techniques/parallel-reference-orbit, techniques/hang-prevention, techniques/auto-iterations, synthesis/neck-escape-statistics, platform/rust-swift-ffi, synthesis/benchmarks
- **Decision:** A11Y: accessibility of the Rust-drawn controls verified. The cockpit, help badges, compass, mini-maps, Julia panel, cosmic scale and π meter are AppKit / UIKit accessibility elements with labels, values and actions (external review S1); review fixes I1 (FractalNSView removes its event monitor and window observers) and C4 (a paused or stopped Metal surface holds no drawable) landed with it. Mac `--ui-demo=accessibility` OK, iOS AccessibilityUITests OK on iPad and iPhone after a fix: the compass element now follows the last drawn frame, since the display link pauses on a still view. Pitfall: window demos print nothing while the Mac's display sleeps. Plan stays in-progress until a VoiceOver review: [A11Y: Accessibility of the canvas and the Rust-drawn controls](plans/a11y-canvas-controls.md).
- **Decision:** Long references off the frame thread: review findings C1-C3 fixed (f64 orbits past 2^18 points on the reference worker, orbits grow instead of reserving the limit, GPU upload on the worker thread, superseded BLA/LA builds cancelled, cancellation inside selection); as built in techniques/hang-prevention, parallel-reference-orbit, reference-selection
- **Lint:** OKF conversion of the wiki. 2876 body wikilinks (2830 plain, 46 with alias text) became relative Markdown links (`[Title](../dir/page.md)`), with the alias or the target page title as text; 516 frontmatter entries in `sources` / `depends-on` became OKF concept IDs (`sources/kf-manual`), and the `item_pattern`s in `.hyalo.toml` changed to match. Every page got a `# Citations` section: 230 built only from what they already cite; the 9 pages that cite nothing (five decisions, ideas, overview, roadmap, sources/apple-device-tooling) say so in one entry that links to their own section. `index.md` and `log.md` lost their frontmatter (OKF reserved files; the root index carries `okf_version`); this log is now grouped under date headings, newest first; `hyalo okf index` added the managed region to `index.md` and an `index.md` per directory. New conventions in `kb/CLAUDE.md` and `.claude/rules/knowledgebase.md`.

### decision | Tours in plain language; tour writing rules

The user took the tours with his son and met words before their explanations ("settle on one
point (the ringed c = −0.2): period 1", "period", fractions round the heart). Every tour but
colours and the walkthrough got a plain-language pass: each opens with the rule, the ring and the
dots where it needs them, explains heart, bulb, loop, period, nucleus, minibrot, spike, antenna at
first use, and shows orbit behaviour with the dots instead of naming it. The bulbs tour's fractions
became visual steps (labelled markers where each bulb sits, dots counting 2, 3, 4; then 2 + 3 = 5
and 5 + 3 = 8 shown with the probe). The rules for future tours: [Controls: keyboard, pointer and touch](decisions/controls.md) "Tour
writing rules".

### decision | Colours tour: orbits that show escape and loops

User feedback: the colours tour's escape steps showed "a line with dots" (a probe on the real
axis) and the inside steps explained loops in words only. Steps 1–2 now zoom on the heart's dent
(0.3 + 0.15i, span 1.4) and run the probe along a noble-angle normal of the cardioid: 8 steps far
out, then 12, 17, 31, 55, and an end where the orbit whirls round the dent for hundreds or
thousands of steps. Bubbles defines loop and period and moves the probe out of the period-3
nucleus with a "zero" marker; Period visits the nuclei of periods 1–4; Internal angle shows the
dots spiralling into the heart's fixed point. Pitfall: the overlay's 1/64-point orbit key makes
a chaotic end count vary between runs (959, 2 158 shown for an f64 594). See
[Controls: keyboard, pointer and touch](decisions/controls.md) "The colours tour".

### decision | Inline icons in tour texts

The walkthrough's cockpit step listed the driving levels as **M**, **C**, **A**, **J**; the cockpit
shows icons (a steering wheel, a speedometer, a plane, sparkles) and the letters appear nowhere on
screen. Tour texts now take `{icon:<id>}` (ids = the controls' `highlight:` names; unknown ids and
unclosed braces are parse errors with their line). The cockpit glyphs moved to core `icon::Glyph`
as strokes, which the Rust cockpit draws and Swift rasterises into template images for the text
(FFI `inline_icons()`); the round buttons' SF Symbol names live in the same table. The
walkthrough's cockpit, cruise, autopilot and toolbar steps and help page 3 use them. See
[Controls: keyboard, pointer and touch](decisions/controls.md) "Guided tours" ▸ Inline icons.

## 2026-10-19

### decision | Held input lets go when its release never arrives

A key-down without a key-up held the Navigator: E held, then ⌘-Tab, could keep `pedal_hold`,
`is_moving` and the display link alive. `FocusLost` existed (window resign key, iPad focus) but
missed the drag's fling samples, the touch throttle's rest and the twist; macOS dropped a
navigation key's key-up while ⌘ was down or a text field had the focus; app deactivation and the
iOS scene's deactivation / background did not send it; `jumped()` left a held pedal stepping, so a
jump with E held started a new cruise. Now `release_all` covers every held kind, `jumped()`
releases held keys, pedal and gamepad, `Host` drops a compass press, and the platforms send
`FocusLost` on every focus loss. See [Controls: keyboard, pointer and touch](decisions/controls.md) "Held input" (unit tests per held
kind, core-check "E held, focus lost, Esc").

### lint | core-check green again: stale Q / E and frame-bit checks

`make core-check` failed five checks on main since 2026-10-17; nobody ran it in the full check.
All five were stale checks, not regressions. (1) Q / E: the check pressed E without Space and
expected the old Space-speed step; since the pedals (f1b69d2, [Controls: keyboard, pointer and touch](decisions/controls.md) "Driving
levels") E from rest starts a cruise. Its key-down was never released, so the pedal's hold
repeat climbed to gear 26 (×1000/s) and (2) the view never settled in 6000 frames, (3) the
keyboard-mode check saw the link still running (bits 3), and (5) after the place jump the held
pedal (`pedal_hold`, at the detent) kept `is_moving` and the interaction clock alive, so the deep
place did not settle either. (4) The autopilot check read frame bit 4, `FRAME_AUTOPILOT`, later
`FRAME_FINGER_STEERS` (still true in A), removed in "Rest or swipe" (ba90261); the check now
reads `stats().autopilot` and the animating bit. With the checks updated the explorer settles
after 280 frames and spiral-1e1000 after 328 on the windowless layer. The recolour fix's
`missed_frame` (5e3c0b5) was checked explicitly: it counts only a frame the plan wanted, so a
settled view without a drawable reports idle (new test
`recolour_tests::settled_view_without_a_drawable_stays_idle`; the windowless layer gets
drawables, and the keyboard-mode check sees the link pause). That check was also flaky (3 of
about 100 runs, only while the machine was busy): it wanted the link to pause on the very next
refresh after `settled`, but the render progress line, shown when the refinement took over
0.5 s, completes and fades for about 0.75 s after the view settled (measured once: one extra
refresh; once still animating after 120 sleepless refreshes, when the frame clock follows real
time). It now allows 2 s of real-time refreshes and prints the progress state if it fails.

### decision | F2 pluggable formulas built, awaiting review

[F2: Pluggable-formula refactor](plans/f2-pluggable-formulas.md) built on its branch (status in-progress until the driver's review
of the A/B table; roadmap row F2 "built, awaiting review"). `Formula { Mandelbrot,
Multibrot(p) }` in almondbread-math, matched outside the hot loops; per-formula hooks in the
reference orbit, selection (`*_for`), f64 reference, CPU oracle, orbit overlay, both BLA tables
and the Component Newton periods; `FormulaCapabilities` in core (plus `julia_panel` for F1's
`z² + k` panel), mirrored over UniFFI; Swift hides what a formula lacks. Kernels: every step
site calls an inline hook branching on `kFormula` (function constant 16, `kPower` 17,
`kConformal` 18), unset for Mandelbrot so its pipelines are the pre-F2 code; other formulas
compile lazily. Multibrot(p) is the generic path, test-only until F3. Proof (measured
2026-10-19, M1 Pro): bench fields and PNGs byte-identical to main (one interior texel of
elephant with period colouring differs run to run on main too), GPU ms geomean 1.004 over 42
cases, regress and history sessions OK, interaction test not slower. Pages:
[almondbread benchmarks and verification record](synthesis/benchmarks.md) "F2", [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md) "F2", the plan's "As built"
(what F3 needs).

### decision | F1 Julia panel built, awaiting review

[F1: Julia view (companion inset)](plans/f1-julia-view.md) built on its branch (status in-progress until the user's review gate; roadmap
row F1 "built, awaiting review"). A Julia companion panel at the end of the map row: the Julia set
of the orbit's point in its own float32 kernel library (`julia.metal`, compiled on first use; the
Mandelbrot kernels and scheduler untouched), the main palette and colouring, the attracting cycle
of k found on the CPU (critical orbit, f64) used to stop interior samples early (≈ 16× on
interior-heavy k) and, with a non-flat interior mode, to shade the interior. One sample per pixel
while k moves, 2×2 in bands once it rests. Controls: U, View ▸ Julia Set (⌥⌘U), Mac toolbar, iPad
round button, control center, tour key `julia:`; tours/julia.md shows it. Measured (M1 Pro): 0.06–0.1
ms GPU per one-sample picture at 360 × 259 px for typical k, ~1.1 ms near-parabolic; interaction
test "julia sweep" / "julia pan" within noise of the panel off (109–117 fps vs 111–116). Float32
field vs f64 reference: class identical at five k, 62 of 93 240 pixels just outside the set
(seahorse). Pages: [Controls: keyboard, pointer and touch](decisions/controls.md) ("Julia panel", "The Julia tour", tour keys).

### decision | Roadmap through 2026-10-19, fractal-type milestones, plan pages

[Roadmap](roadmap.md): rows for the work after PJ, from the log, [Controls: keyboard, pointer and touch](decisions/controls.md), the iOS pages,
[almondbread benchmarks and verification record](synthesis/benchmarks.md) and the merges on main: R6 and X1 (the rest of 2026-10-15), AP2
(autopilot and driving), Q1 (rendering quality and interior), UX2 (Mac UI and content), RB
(robustness: hang prevention, GPU queue guard, recolour fix, overlay layout), OS (open-source
release), I1 / I2 (iOS / iPad port and touch, before and after the iPad device round), T1 (tour
engine and 10 maths tours, π only through its tour), T2 (walkthrough and tour discovery), ST
(simulator tooling), DS (docs and App Store drafts). Awaiting device review: rest / swipe
thresholds, tours and walkthrough on the iPad. M8 replaced by F1–F8 and BB in the order of
[Other fractal types: what almondbread could support, at what cost](synthesis/other-fractal-types.md) §5 (F7 is the old M8); M9 kept. New page type `plan`
(`plans/`, schema with enums, required body sections, `plans` view sorted by priority; rules in
`kb/CLAUDE.md`) and ten plan pages: [F1: Julia view (companion inset)](plans/f1-julia-view.md) (proposed, awaiting go),
[F2: Pluggable-formula refactor](plans/f2-pluggable-formulas.md), [F3: Multibrot z^p + c](plans/f3-multibrot.md), [F4: Deep Julia sets](plans/f4-deep-julia.md),
[F5: Mandelbar (Tricorn) and the non-conformal path](plans/f5-mandelbar.md), [F6: Burning Ship family](plans/f6-burning-ship.md), [F7: Hybrid formulas via an opcode model](plans/f7-hybrids.md),
[F8: Nova, Newton and Magnet research spike](plans/f8-convergent-spike.md), [Buddhabrot and Nebulabrot as a look](plans/buddhabrot.md), [3D worlds (Mandelbox first): not recommended now](plans/3d-worlds.md) (proposed, last, not
recommended now; also in [Ideas](ideas.md)). The Julia view stays on the roadmap only: [Ideas](ideas.md) holds
the user's ideas, proposals go to plans.

### research | Other fractal types: feasibility and roadmap

User question: which other fractal types could almondbread support on top of what exists (Julia,
3D Mandelbulb/Mandelbox)? Read the formula pages, landscape decision and the code (about 12 modules
are formula-specific; the perturbed step is inline at ~10 kernel sites; nine function constants
already specialise the kernel), plus web research. Result: [Other fractal types: what almondbread could support, at what cost](synthesis/other-fractal-types.md),
with a family × capability matrix, the order Julia companion → pluggable-formula refactor
(`kFormula`/`kConformal` function constants, `Formula` enum in math, capability struct over FFI) →
Multibrot → deep Julia (critical-orbit rebasing: the Mandelbrot orbit of k; untested) →
Mandelbar → Burning Ship family → hybrids, Nova spike; Buddhabrot and Lyapunov as shallow modes.
3D verdict: a separate XL mode with a ~1e4–1e5 float32 ceiling and no published perturbation
scheme; the 2.5D landscape is the cheaper route. New sources: quilez-mandelbulb,
skytopia-mandelbulb, wikipedia-mandelbulb, wikipedia-mandelbox, hvidtfeldt-de-3d-fractals,
mandelbulber2-readme, alltools-mandelbulb-explorer, superliminal-buddhabrot,
wikipedia-fractal-flame, wikipedia-newton-fractal, wikipedia-lyapunov-fractal, frax-about,
hpdz-animations. Pointer added to [Other formulas: feasibility of deep zoom](formulas/others-feasibility.md).

### decision | Maps, cockpit and tour card keep clear on phones

Three iPhone clashes: on its side two mini-maps lifted the cockpit into the HUD; in portrait the
second map ran under the button column and the compact tour card's Next sat under the throttle.
The map row now fits (`fit_maps`): it keeps clear of the trailing controls, narrows its panels to
2/3 before dropping a level, and leaves the cockpit a place beside it or lifted within the lower
half (phone on its side: one map beside the two-row cockpit). The tour card at the top keeps clear
of the throttle, the round buttons, the cockpit and the maps (narrowed beside the column in
portrait, the trailing side on its side when the maps are under it; the text scrolls in what is
left). The UI tests' round-button lookup had matched identifiers only and checked nothing; it
matches labels now. [Controls: keyboard, pointer and touch](decisions/controls.md) "Mini-map on small screens", "Tour card on a phone".

### decision | Overlay layout on narrow canvases; GPU queue hang

From the App Store screenshot pass. The cosmic scale stacks above the cockpit where the cockpit
does not fit beside it (top right on a phone on its side) and keeps clear of the iPhone's button
column; the π panel and its trail inset keep clear of the tour card, the cockpit, the panels and
the probe by rects instead of a fixed 352-point inset (shrinking to 0.75, lifting, the trail
hidden on iPhones); a phone on its side always gets the bottom button row (`TouchLayout`, `make
swift-test`); the Mac control center's hidden width no longer shifts the π panel. A real
main-thread stall (up to 24.8 s in `commandBuffer()` with the queue full of long iterate chunks,
iPad simulator, "Period 2688") is guarded by `GPU_QUEUE_LIMIT`. The Period 2688 step costs about
23 s of M1 Pro GPU at iPad mini size; its text now says so. `--ui-demo-size=WxH`, `--ui-demo-aa=`,
UI tests `OverlayLayoutUITests`, `TourPerformanceUITests`. [Controls: keyboard, pointer and touch](decisions/controls.md) "π meter on
narrow canvases", "Cosmic size scale", "Round buttons"; [Hang prevention (loop guards, extreme-value tests, main-thread watchdog)](techniques/hang-prevention.md) "GPU queue".

### decision | Recolour without a new job; tour views at any depth

Interior-only tour steps (and the control center's interior picker) could leave the old colours
on screen: a recolour keeps the compute key, and once the view had settled nothing kept the
display link running, neither when the recolour refresh got no drawable nor for the AA rerun
after it. Fix: a planned frame without a target keeps `animating`, and `COMPOSE_EXACT` clears
`final_presented`; Metal regression tests in `scheduler/recolour_tests.rs`; the colours tour's
interior `move: jump` workarounds removed; new `--ui-demo=interior-cycle` scene. Tour `span` /
`zoom` already parsed any depth; the bundled-tour test checked f64 and now checks log2. The limits
tour gained a 1e3000 step (`swirl-1e3000`, resolved 417 ms after the jump at 1600×1000).
[Controls: keyboard, pointer and touch](decisions/controls.md) "The colours tour", "The limits tour";
[Frame reuse and progressive refinement](techniques/frame-reuse-and-progressive.md).

### decision | Finding the tours

User request: make the guided tours easy to discover. New tour header keys `place` (comma list),
`thumbnail`, `topics` (closed list `tour::TOPICS`: period, fog, iterations) and `next`; core
`next_tour` (next: or list order, missing and hidden skipped, wraps), `tour_for_topic`,
`Tour::minutes`. UI: "What's a period?" in the HUD next to "Minibrot, period N", "What's the fog?"
on the not-finished line, a link in Iterations & quality, "Next tour" on every last step (the
walkthrough offers the first tour), tour cards with place thumbnails, "9 steps · ~3 min" and a done
check, signpost badges on tour places. The series order is `BUNDLED`'s. In
[Controls: keyboard, pointer and touch](decisions/controls.md) "Finding the tours".

### decision | Tour: Misiurewicz points

New tour `tours/misiurewicz.md` (8 steps): pre-periodic orbits at −2, i, the M(4,1) fork and the
spike's M(3,1) crossing; self-similarity with the cycle multipliers (1.33 / 120° at M(4,1), 5.66 /
45° at i, 4 at −2); the fork at 1e300; spirals at 1e4 and 1e50. [Controls: keyboard, pointer and touch](decisions/controls.md) "The
Misiurewicz tour".

### decision | Tour: every point has a Julia set

New tour `tours/julia.md` (8 steps): fix c and vary the start; connected inside, dust outside;
the set looks like a point's Julia set near it; embedded Julia sets. No Julia renderer in the app:
the tour uses the orbit overlay and places; a Julia view would improve it. [Controls: keyboard, pointer and touch](decisions/controls.md)
"The Julia tour".

### decision | Tour: what the colours mean

New tour `tours/colours.md` (12 steps): escape counts and bands, smooth colouring, palettes and
looks, fog, proven black, and the five interior colourings. Found: an interior-only step change
did not recolour the shown frame in the app until a new job (worked around with `move: jump`;
possible renderer bug). [Controls: keyboard, pointer and touch](decisions/controls.md) "The colours tour".

### decision | Tour: patterns that double

New tour `tours/doubling.md` (9 steps): from the M(4,1) fork to a period-39 minibrot 5.2e-6 away,
the arms double 3 → 6 → 12 → 24 on the way (12 at half of the log-depth, 24 at three quarters:
added to [At what fraction of the log-depth to a minibrot does each doubling appear?](questions/julia-morph-depth-fractions.md)), why squaring doubles, embedded Julia sets and
Julia morphing. [Controls: keyboard, pointer and touch](decisions/controls.md) "The doubling tour".

### decision | Tour: Seahorse and Elephant valleys

New tour `tours/valleys.md` (10 steps): seahorses and their tails, Misiurewicz points at the spiral
centres, M(24,2) at 1e4 and 1e50 (self-similarity), elephant valley, the M(4,1) fork at 0.005 and
1e100. Orbit probes at Misiurewicz points are misleading in f64 (M(4,1) "escapes after 137 steps").
[Controls: keyboard, pointer and touch](decisions/controls.md) "The valleys tour".

### decision | Tour: bulbs and spokes

New tour `tours/bulbs.md` (8 steps): periods round the cardioid with a probe path through the
nuclei, counting the antenna's branches (1/4 and 1/5 bulbs, forks located as M(5,1) and M(6,1)),
the p/q rule and Farey addition (2/5, 3/8). Nuclei and forks in [Controls: keyboard, pointer and touch](decisions/controls.md) "The bulbs
tour".

### decision | Tour: why some points are hard

New tour `tours/limits.md` (9 steps): slow points at the neck with the orbit overlay, proven
interior, fog that stays (the neck gap at 1.5e-8), computing further, f64's 16 digits,
perturbation and skipping, ending at the 1e302 needle minibrot. Two pitfalls found on the way:
low `iterations:` values show no fog in the app (the scheduler extends and computes further), and
the tour test cannot take views below about 1e-305. [Controls: keyboard, pointer and touch](decisions/controls.md) "The limits tour".

### decision | Simulator jobs: one at a time, one shared pair, low priority

After the 2026-10-18 overload (12 simulators from parallel agents, load ~800, swapping):
[iOS and iPadOS on the simulator: setup, checks, what broke](platform/ios-simulator.md) "Low-load rules". A machine-wide lock (`~/Library/Caches/almondbread/
sim.lock`, fcntl via `zsystem flock`, `scripts/sim-common.zsh`) serializes every simulator job;
one shared pair `AB iPhone 17e` / `AB iPad mini`, booted one at a time and shut down by UDID;
`make ios-uitest TESTS=… DEVICE=ipad` for targeted runs (`build-for-testing` outside the lock,
`test-without-building` inside); test builds under `nice -n 10` with 6 jobs (`taskpolicy -b` was
8× slower: E-cores only); `make ios-sim-run` for the user (opens Simulator.app, holds the lock,
Ctrl-C stops). `IOS_DEVICES` is gone. Deleted the stale `ABT`, `DR`, `DT`, `GL`, `PG` simulators.

### lint | Stale statements after the touch, tour and walkthrough changes

[Controls: keyboard, pointer and touch](decisions/controls.md) "Touch": the iPad's round buttons are a top-trailing row only on a canvas
at least 560 points wide; narrower (portrait with the control center open, split views) they
are the phone's bottom-trailing column with the throttle on top; a regular-width phone on its
side gets its row top trailing (read from `ContentView.swift`, not checked). The P key is gone
from the parity table and the full-screen key list; place tiles: Go / Fly clear the selection.
[Overview](overview.md): the iOS port exists, survey steering is the default, tours and touch controls
listed. [iOS and iPadOS on the simulator: setup, checks, what broke](platform/ios-simulator.md) and [iOS and iPadOS port considerations](platform/ios-ipad-considerations.md): superseded
pointers. Help: click / tap streams, Smart zoom, place tiles on touch. CLAUDE.md: the iOS 26.5
simulator runtime is installed; `ALMONDBREAD_UITEST` in the env table.

### lint | Four unmerged branches triaged; their knowledge brought to main

`worktree-agent-a0bf2467567503583` (needle minibrot at 1e3000), `-a3339d320584abaa6` (perf-menu
scene), `-abad90e6e8bdb3fc5` (R4 MetalFX / sample reuse) and `-ade067c897fa45012` (R5 prototype
and a deep-kernel DE fix), left as they are. The DE fix is on main as `f5afcf8` (same patch),
the R5 prototype as `e4ce6e1`, R4's pages, raw notes and prototypes as `635e849`. Re-checked on
main: the `deep_distance_estimate` test passes (0 isolated DE 0, 0 of 40 687 smooth-neighbour
jumps at sea-1e20) and DE-shaded renders of sea-1e20, sea-8007 and dendrite-1e300 show no
outline bands or dotted arcs. Brought over as text only: the needle-of-the-needle research
([Places: a catalogue of famous views to visit](synthesis/places.md), marked not in the catalogue), the 2026-10-16 menu-hang record
([SwiftUI performance: the control center sidebar](platform/swiftui-performance.md), marked superseded by the `VStack` fix; the 2026-10-17
section already pointed at it), R5's corrected image-diff numbers ([Does local series approximation hold in the float32 perturbation kernel?](questions/local-sa-float32.md),
[almondbread benchmarks and verification record](synthesis/benchmarks.md)), and a note that `prototypes/motion-reuse` does not build against main
([Motion sample reuse: fewer iterations per gesture frame (R4)](synthesis/motion-sample-reuse.md)).

### decision | First-launch walkthrough as a hidden tour

The walkthrough is `tours/welcome.md`, a hidden tour; the tour engine gained `platform`, `highlight`,
`control-center` and `goal` step keys, `::: mac` / `::: touch` text blocks and the `hidden` header.
Offered once at the first launch (never in tests unless forced). See [Controls: keyboard, pointer and touch](decisions/controls.md)
"Walkthrough".

## 2026-10-18

### decision | The π meter becomes a tour

User: π only through a tour. New tour `tours/pi.md` (6 steps: the neck, count and multiply, the
neck hunt, why π, the cusp hunt, your turn) and step key `pi-hunt: neck|cusp` (starts the meter's
guided flight in its own step). Removed the P key, View ▸ π Meter / Hunt π at the Neck / at the
Cusp, the control center's π switch, reading and hunt buttons, FFI `pi_flight` and the meter's
persistence; help (getting started, overlays, popovers) and the neck place point to the tour. The
π panel keeps left of the tour card. [Controls: keyboard, pointer and touch](decisions/controls.md) "π meter", "Guided tours" key table,
"The π tour".

### decision | Tour: what a minibrot's period means

New guided tour `tours/minibrot-periods.md` (9 steps, place `airplane`): loop length = period on
the cardioid, the period-2 disc and the 1/3 bulb (counting the antenna's branches), the Airplane,
then the real chain of Seek minibrot from the Airplane (periods 3, 7, 16, 48, 80, 128, 384, 640,
896, 2688) ending at a period-2688 minibrot at 10^15.7 with `iterations: 300000`. Coordinates from
`seek::seek` and f64 checks; headless renders and a background `--ui-demo` run confirm every
fate line. Our size bound `p ≳ 0.83 d` (inference) and the needle places sitting on it:
[Controls: keyboard, pointer and touch](decisions/controls.md) "The minibrot-periods tour". Help: one sentence under Seek a minibrot.

### decision | Touch: rest to steer, swipe to move the picture

[Controls: keyboard, pointer and touch](decisions/controls.md) "Touch" ▸ "Rest or swipe": iPad feedback (C lost the resting finger that
steered; A's wish line from the centre to the finger did not follow a swipe). One rule while A,
C or a journey's flight flies: a finger within 10 points for its first 0.25 s rests and steers
until it lifts (C: like the pointer; A: the wish; journey: takes the wheel off the dead zone),
one that leaves sooner swipes and pans 1:1 from its touch-down point (in A without handing over:
the lock lets go during the 1 s manual hold and picks up from the moved view). Swift sends the
raw single finger (`FingerDown` / `FingerMove` / `FingerUp`) from the passive finger tracker,
Rust classifies (`FINGER_SLOP`, `FINGER_REST`); `TouchSteer`, the steering long press and
`FRAME_FINGER_STEERS` are gone. Manual and the Mac unchanged.

### decision | Closing the control center on touch

Device feedback: no visible way to close the control center on iPhone / iPad. A pinned ✕ at the
trailing end of the pane's search bar and the Control center round button lit (accent fill,
selected trait) while the pane is open; the Mac unchanged. Test
`TouchControlsUITests.testControlCenterClose`. [Controls: keyboard, pointer and touch](decisions/controls.md) "Touch" ▸ "Closing the
control center on touch".

### decision | Flaky small-twist UI test: XCTest's rotate, not the dead band

`testSmallTwistDoesNotTurn` failed 6 of 10 on `AB iPad mini` with the old input. Traced: XCTest's
`rotate(0.15)` is an 18.4° finger jump with no rotation after it (fingers down off the start
line; its last 12° keyframe is dropped), which UIKit reports in one event as 12.3° or 24.6°
from identical synthesised events; 24.6° − 15° is the 9.57° the test saw, so the Rust dead band
worked. The test now draws an exact 8.6° twist through XCTest's event record
(`TwoFingerTwist.swift`): 10 of 10 on the iPad, 1 of 1 on `AB iPhone 17e`. Rust test
`a_single_twist_event_crossing_the_dead_band_turns_by_the_excess`. Noted: UIKit's 5–6° start
hysteresis adds to the 15° band on real fingers. [Controls: keyboard, pointer and touch](decisions/controls.md) "Touch" ▸ "Device round",
[iOS and iPadOS on the simulator: setup, checks, what broke](platform/ios-simulator.md) "QA".

### decision | Touch throttle becomes a position lever

[Controls: keyboard, pointer and touch](decisions/controls.md) "Touch" ▸ "Position lever": user feedback from a real iPad (the jog
lever read as "gear up / gear down"; the drag length chose the gear unnoticed). The knob's tick
is now the gear (M in the middle stops, up in, down out), it stays where it is left, a tap on a
tick goes there, chevrons step one gear, double tap stops. New core event `SetGear { gear }`
(absolute; the relative `Throttle` stays). The knob always shows the navigator's gear (the
nearest rung to A's and J's speed; a fly-to's own speed now reaches the cockpit). Ticks stand
for geometrically spaced gears 1…26 when not all fit at 10 points (`cruise::lever_ticks`): 7 a
side on an iPhone in portrait, 5 on its side, 14 on the iPad (lever up to 360 points there).

### decision | Double click / tap zoom: a gentle glide, and click streams keep zooming

User: double-click zoom felt abrupt and a stream of clicks or taps stopped after the second.
[Controls: keyboard, pointer and touch](decisions/controls.md) "Animated zoom steps": the double click / tap is now its own ×2
minimum-jerk glide of 0.35 s (`TAP_ZOOM_SECONDS`, `Navigator::tap_zoom`, `TapZoom`), extended
by each further click from the current rate (C², exact ×2ⁿ). Mac zooms on every click count
≥ 2; iOS chains taps within 0.4 s and 60 pt through a third tap recogniser. Wheel notches,
+ / − and menu zooms keep the τ = 60 ms ease-out. New interaction-test scenario `click
stream` (15 real clicks 0.18 s apart: ×16384 about the clicked point; measured 2026-10-18,
background window at 120 Hz: 99 fps, interval p50 / p95 / max 8.3 / 16.7 / 41.7 ms, the same
as `space zoom`).

### decision | iOS round buttons: the Mac toolbar instead of autopilot / journey

User on a real iPad: the round buttons duplicated the cockpit. Now Orbit, Mini-map, Size scale
(toggles, filled when on), Fly to and Control center, as the macOS toolbar. Journey stop choices
moved under the HUD as labelled buttons; the autopilot button's menu is covered by the cockpit,
the throttle and a new iOS control-center "Seek minibrot"; the orbit toggle on touch shows the
view centre's orbit (or the last Orbit Here point); touch hints point at the cockpit's plane.
[Controls: keyboard, pointer and touch](decisions/controls.md) "Touch" ▸ "Round buttons".

### decision | Touch device round: pinch release, cruise drag, wish line

First try on a real iPad Pro 11" (M5); [Controls: keyboard, pointer and touch](decisions/controls.md) "Touch", "Device round". (1) A
pinch whose fingers lift one at a time panned with the remaining finger: UIKit keeps the pinch
alive with one finger and its location jumps to it. Swift now sends pinch and twist events only
with two fingers down; Rust ignores `Pan` / `TouchSteer` after a pinch until `TouchesLifted`
(every finger off). (2) Cruise steering by touch was the Mac's hover joystick about the centre
(dead zone, quadratic response): a drag of a few centimetres moved the view ~17 points/s, the
other way. A drag in C and in a journey's flight now pans, as a drag does on the Mac; the finger
steers only in A (the wish). (3) The wish line in A never went away: the lifted finger's wish
was kept until A ended. It is now let go once the autopilot's wish settled, so the line fades.
Simulators `DR iPhone 17e`, `DR iPad Pro 11`.

### decision | Places grid on touch: select, then Go or Fly on the picture

The QA pass made a tap on a place's picture go there on iOS, which left Fly unreachable (its
button is below the 79-picture grid, off screen; found on a real iPad). User's design, iOS only:
the first tap selects (border, name along the bottom edge, Go and Fly over the image, ≥ 44
points, material background); a second tap on the picture goes there; VoiceOver has Go and Fly
as actions. The smallest tiles (~100 × 63 points: iPhone sheet, iPad sidebar at its minimum)
take the label under the icon; wider tiles side by side (`ViewThatFits`). New XCTest
`testPlaceTileGoAndFly`; `testControlCenterPlaceAndTour` now selects, then taps again.
[Controls: keyboard, pointer and touch](decisions/controls.md) "Touch" (places grid).

### decision | iOS QA pass: a first-time user before a child's first try

Branch `ios-qa`, simulators `QA iPad Air 11` and `QA iPhone 17e`. `FirstUseUITests` scripts
first sessions with screenshots at every step ([iOS and iPadOS on the simulator: setup, checks, what broke](platform/ios-simulator.md) "QA"). Fixed: the
long-press seek was never polled (no frame requested: `Navigator::wants_frames`); a tap on a
place's picture only selected it (now goes there on iOS); pinches turned the view (touch twist
dead band 15°, `TouchRotate`, Mac unchanged); "Allow Paste?" on Go to Copied (system Paste
control); click / key words on touch; compass over the throttle on a phone on its side.
[Controls: keyboard, pointer and touch](decisions/controls.md) "Touch" notes the decisions.

### decision | Touch round 2: a first run by a child

[Controls: keyboard, pointer and touch](decisions/controls.md) "Touch", round 2 (branch `ios-touch-2`, simulators only): where a finger
steers (A, C, a journey's flight; `Navigator::finger_steers`, `touch_hold`) it never opens the
long-press menu — a resting finger steers after 0.2 s, a journey's flight hands over to cruise
on a touch off the dead zone; the menu stays where nothing flies (its point would be stale
under a moving picture). Long press on a mini-map flies there. First-run touch hints (dismissed
by any touch, back from Help). Tour card on a phone at the top, folding and scrolling. iPad menu
bar: Tours, Help, Show Touch Hints. Fixes from the surface review: help popovers off-screen at
the bottom of a phone (iOS `arrowEdge` semantics), fly-to sheet clipped (420-point minimum) and
asking "Allow paste?" on open. Window tests, `--ui-demo` and the iOS UI tests no longer read or
write settings (`TestLaunch.ephemeral`). See also [iOS and iPadOS on the simulator: setup, checks, what broke](platform/ios-simulator.md).

### decision | iPhone/iPad device readiness: device build, memory budget, thermal, background

Prepared without a device. `make ios-device` / `ios-device-selftest` / `ios-device-check` /
`ios-device-doctor` (`scripts/ios-device.sh`): automatic signing for team 9KNMHVRSTW through
command-line overrides (the simulator build stays unsigned), devicectl install, launch and
self-test with its PNG copied back from the app container; clear messages for no device, not
paired, Developer Mode off, no Xcode account. Measured renderer memory (`fly --ios 1`, M1 Pro):
~108 MB fixed + ~67 B per texel on the GPU (449–470 MB at 2752×2064 at home and at 1e-3007),
references and tables < 4 MB at the default limits, ~130 B per reference point for big manual
limits (+408 MB for 10 M vs 1 M). New Rust policy (`memory.rs`, `scheduler/device.rs`): a
budget from `os_proc_available_memory()` caps output texels (the drawable shrinks) and the
iteration limit; memory warnings drop caches; thermal serious/critical lower GPU budgets, drop
compute further, cap 60/30 Hz, critical stops refinement after the first full frame; scenes
in the background submit no GPU work. Licence: no LGPL in the iOS binary (own BigFloat).
Pages: [iOS and iPadOS on a device: first run, signing, memory, thermal, background](platform/ios-device.md), [iOS and iPadOS port considerations](platform/ios-ipad-considerations.md),
[Apple device tooling and iOS process APIs (devicectl, xcodebuild provisioning, os/proc.h)](sources/apple-device-tooling.md).

### decision | Touch controls for iPhone and iPad

[Controls: keyboard, pointer and touch](decisions/controls.md) "Touch": parity audit of the Mac's keyboard and pointer features against
touch, iPad keyboard and pointer, and the touch design built on branch `ios-touch`: a throttle
jog lever (one gear per 14 points, the pedals' paths, zero stop per drag, double tap stops), a
touch cockpit (44-point cells, one or two rows, above a bottom-left map row on a phone),
steering-finger events (the autopilot's wish stays after the lift), a long-press popover menu
(seek minibrot here, orbit here with a draggable probe, fly here, copy / go to copied),
fingertip-sized (?) badges, haptics, finger-sized tour and Continue buttons, the Mac's menu
commands in the iPad menu bar. New navigator events `TouchSteer`, `Throttle`, `Probe`,
`SeekAt`, `FlyHere`; XCTest `TouchControlsUITests` on both simulators. Simulator only; feel
needs a device.

## 2026-10-17

### decision | Cruise: 26 gears, gear-scaled reverse boost, gear / target line

User-approved: the pedal ladder has 26 gears (×1.1 … ×1000/s), a held pedal repeats every
0.15 s, the reverse depth boost ramps over 10 s and its exponent is scaled by the gear (0.5 in
gear 1 to 1 in gear 26), so low gears zoom out gently: home from 1e-10 in gear 1 takes 126 s,
from 1e-3000 in gear 10 (×5) 69 s, in the top gear 10 s (table in [Controls: keyboard, pointer and touch](decisions/controls.md)
"Driving levels"). Two real bugs surfaced while adapting the navigator tests: the soft landing
stalled about 1e-4 of path short of home and snapped after 1.5 s (it left its brake curve for
the spring; now it stays on the curve), and Q could not bring the autopilot or a journey to zero
(their speed floor 0.1 sat above gear 1, 0.095; now 0.09). The cockpit's small line shows the
gear in cruise ("gear 5", "reverse 3 · boost") and the set speed in A and J ("target ×20") only
while the rate flown differs from it by more than 5 %; in J it now compares the journey speed
with the leg's flown speed. Cockpit 306 points wide (fits a phone).

### query | Beach ball opening and scrolling the control center

Hang report: opening the control center and scrolling it hung the main thread for good in a
SwiftUI graph update. New scenes `--ui-demo=perf-open` (open, scroll, the reveal animation
replayed with the split view's divider) and `--ui-demo=perf-stress` (40 seeded rounds, stall
watchdog that `sample`s). Not reproduced as a permanent hang in a background window; found
that the reveal laid the lazy sidebar out at every width from 20 pt up (20-64 ms per step, tile
grids rebuilt, document height estimates of 15 000+ pt) and 63 iterations over 150 ms per
stress run (worst 566 ms). Fix: the content is laid out at the settled width (`LayoutWidth`),
a plain `VStack` replaces the `LazyVStack` (whose prefetch cycle is the only mechanism seen to
hang for good), the place pill offset is cached in the model (no Rust call in a body). Scroll
longest iteration 31-38 → 3-5 ms, first open 77 → 40 ms, stress 63 → 8 long iterations; section
opens 10-20 → 30-150 ms and the search rebuild 67-92 → 330-350 ms are the price. Page:
[SwiftUI performance: the control center sidebar](platform/swiftui-performance.md) "Hang: opening and scrolling the sidebar".

### decision | Guided tours; the Feigenbaum tour

The user asked for guided tours with an engine where new tours are easy to add. A tour is one
Markdown file in `crates/almondbread-core/tours/` (header, then `## Title` per step with
`key: value` settings under the heading and the text after a blank line), parsed and checked in
the core with line-numbered errors; settings carry over between steps, so Back shows a step as
it was. A step sets the view (flown or jumped), look, palette, interior, iterations, overlays,
the orbit probe's c (fixed, or moving along a path as a closed form of time, the same at 30, 60
and 120 Hz) and a marker. The start saves the user's settings and view and stops other motion;
the end restores them. Card with Back / Next / End tour (→ ← Esc), a Tours section in the
control center, View ▸ Tours, and a Tour button on places a tour names. First tour: the
Feigenbaum point in 9 steps, every number computed (nuclei, doubling points, attractors).
Format and keys: [Controls: keyboard, pointer and touch](decisions/controls.md) "Guided tours".

### decision | Pointer wish in the autopilot; target marker

The user asked whether the autopilot could take the coarse direction of the mouse into account.
In A the pointer no longer hands over to cruise; W A S D, a drag, the stick and the pedal to zero
still do. Journey legs keep the pointer hand-over. Instead, the pointer's direction past 0.2 of the
steering radius becomes a wish, settled with 0.3 s / 45° hysteresis. The lock's searches prefer
candidates that way (+1.5 × cosine) and take the best aimable target in the wished half first.
Survey steering weights its cells the same way and leads towards the wish. A changed wish
re-targets once. The aim is always a found target, never the pointer; the HUD adds "· your
direction". Headless `fly` with the pointer held right or left (measured 2026-10-17, M1 Pro,
release, heavy shared load): seahorse 1e3 → 1e18, every target after the wish settled lay the
wished way, no featureless time. User's deep path at 1e237: 91 % of aims went the wished way but
6 of 10 runs backed out of a void at 1e239. Survey steering now ignores the wish while the heading
region is uniform: 1 rescue in 8, 2.8 % uniform (no wish: 0 in 7, 0.1 %), but only 61 % of aims go
the wished way there.

Target marker (the user's extension): a ring where A or a journey leg is heading. It glides on
target changes and fades. A short label shows for 2.5 s for known kinds ("spiral", "minibrot ·
period 34"), and a faint line points towards the wish. Toggle: View ▸ "Autopilot Target Marker",
control center Overlays; on by default; persisted. [Controls: keyboard, pointer and touch](decisions/controls.md) "Pointer wish",
"Target marker".

### lint | Business pages removed from the tree

The 24 business files were still tracked: the two research reports, the distribution and gamification decisions, 13 source pages and 7 raw research notes. The 2026-10-17 entry above had said they had moved. They are now deleted from the repository, after a byte-for-byte check that `../almondbread-private/kb/` holds identical copies. They remain in the git history until the user decides how the history goes public.

### decision | Steady autopilot rate: the throttle slows only when frames miss the budget

User report: the autopilot's rate readout varied between ×1.75/s and ×4.x/s. `fly --log` at the
user's 1e240 point (and seahorse for comparison): the renderer throttle alone moved the rate
(×1.32–×2.46 at the default speed, hint "Rendering" 30–81 %); safety and the lock's capture
slow-down were minor. The throttle followed the gesture fit, which reads back the preview step
the scheduler runs at (median 0.137 at 1/8, 0.238 at 1/4, 0.81 at 7/8), so the 1/8 ↔ 1/4 probe
toggling and cheap/dear regions swung it 0.36 ↔ 0.95, while slowing does not make a fresh preview
cheaper. Fix: throttle full at fit 1/10, floor at 1/16 (only a coarsest preview that misses the
budget slows), fit log filtered 0.5 s down / 1 s up. Rate sd 0.15–0.33 → 0.06–0.09 e-folds/s at
the user point, 0.22 → 0.002 at seahorse; ~5 more decades per 90 s. Page: [Controls: keyboard, pointer and touch](decisions/controls.md)
"Steady autopilot rate".

### decision | Pointer hides while the autopilot or a journey flies

User request. In A or J, the pointer hides after 1.5 s without moving over the canvas (`NSCursor.setHiddenUntilMouseMoves`) and shows again as soon as the mouse moves. It is a direction wish in the autopilot, so it shows whenever it is in use. The HUD, the control center and window tests are unaffected. [Controls: keyboard, pointer and touch](decisions/controls.md).

### decision | Pointer hand-over from the autopilot needs real steering

The user: "when in Autopilot mode, moving the mouse just a bit throws you into cruise mode".
The hand-over now needs all three: 60 points of pointer travel (was 12), the pointer more
than 0.35 of the steering radius off-centre (was the 0.1 dead zone), and that held for 0.6 s
(was 0.25). WASD, a drag and the stick still hand over at once. [Controls: keyboard, pointer and touch](decisions/controls.md) "Driving levels".

### decision | Component Newton: interior near the edge of any low-period component

User report: views touching a hyperbolic component other than the cardioid and the period-2
bulb kept a wide fog band inside its edge. There `|λ|` is 0.99–0.999, so the orbit settles
too slowly for the `|w| n²` test and for the probe's scan. Newton on `f^p(z) − z` from `z = 0`
converges in about 5 passes everywhere in such a component (f64, period-22 and period-64 views).
The CPU now finds the periods at a 5 × 5 grid of sample points per view (atom-domain
candidates, the smallest that Newton shows attracting; < 0.5 ms in f64). The probe tries them
from `z = 0` before its scan, and the existing Kantorovich bound proves each pixel. A lean
chunk dropped the probe state of probing pixels; that is fixed. Results: period-22 view
unresolved 8.3 → 4.4 %; period-64 view 79 → 22 %, settled 20.6 → 3.0 s in the app scheduler.
0 false interior in 13 views. Page: [Interior detection](techniques/interior-detection.md) "Component Newton".

### decision | Compute further: "Continue" at the time limit; cosmic scale without numbers

When compute further stops at its time limit (`further_budget`, 60 GPU s), the HUD's progress line offers **Continue** (button, key G). `Renderer::continue_further` adds another budget to this view's passes (`Further::extra_budget`) and resumes where they stopped. Only a stop at the time limit is offered; at the upper limit more time does not help. The HUD is hit-testable now, for the button. At 100 % the line reads "nothing finished yet" (user: "Is there a reason why not saying 0% finished?").

The cosmic scale's deep phases name no numbers any more (user: "concrete numbers don't make sense anymore"). The thing as big as our universe is named when within a factor 2. Otherwise it reads "something between the solar system and the Milky Way", or beyond the ends "bigger than the Milky Way" / "smaller than the Planck length". [Automatic maximum-iteration control](techniques/auto-iterations.md), [Controls: keyboard, pointer and touch](decisions/controls.md).

### decision | Survey steering on by default

The user tried `ALMONDBREAD_SURVEY_STEER=1` in the foreground at the 1e247 path. It ran into featureless regions twice, backed out quickly and resumed. Headless numbers: 1.3 s instead of 6.7 s featureless per 90 s, 1 reversal instead of 6. Survey steering is now always on, and the switch is removed. [Controls: keyboard, pointer and touch](decisions/controls.md) "Survey steering".

### decision | Smooth journey legs: eased landings and turns, no dull-pace pulses

User report: journeys jumpy (brake-and-accelerate in dives, cross-overs zooming out fast, then a
dead stop, then fast in, full stops). Reproduced headless with `fly --log` on #4711 (zoom rate
per refresh): every leg ended from full speed in one frame (the landing spring lagged its
`√(2 a s)` curve), a dull stretch up to the target braked from 3×, a one-sample gap in a dull
stretch pulsed, and the van Wijk apex swung −28 → +23 e-folds/s in 0.15 s at ×35/s; the renderer
throttle was not involved; cba2d79 shows the same at raised speeds. Fix: landing on `left^(2/3)`
(speed `(1 − t/T)²`), braking ahead of the apex onto a speed-independent turn, dull pace
bridged and ramped and off for the landing, paced legs capped at 16 e-folds/s; worst zoom
acceleration 41 → 17 e-folds/s² (default), 462 → 23 (×35/s), 1069 → 20 (×1000/s); legs take longer. The cruise taken
over from a leg is set to the journey speed (it was the leg's paced rate, ×55/s), and J lets a
released cruise glide out or carry on into the leg instead of halting it. Page:
[Controls: keyboard, pointer and touch](decisions/controls.md) "Journey" (revision "smooth legs").

### lint | Consolidation pass after the 2026-10-16/17 features

Read the log entries of both days and swept the wiki for what they removed or changed.
- Stale claims fixed: [Controls: keyboard, pointer and touch](decisions/controls.md) (cockpit levels shown as icons, not M · C · A · J, and clickable; toolbar buttons for the π meter, autopilot and journey gone; the iOS zoom-out long press and `NavKey::AutopilotOut` went with ⇧Z; the cosmic scale phases 2 and 3 and the ruler reworded to imaginary universes and 61.7-decade Planck levels; survey steering marked as still an env switch, shared with lane assist), [Interior detection](techniques/interior-detection.md) and [Controls: keyboard, pointer and touch](decisions/controls.md) (the grey veil is fog now), sample-cache mentions in [Motion sample reuse: fewer iterations per gesture frame (R4)](synthesis/motion-sample-reuse.md), [Frame reuse and progressive refinement](techniques/frame-reuse-and-progressive.md), [MetalFX upscaling (temporal, spatial) for motion sample reuse](platform/metalfx.md), [Flight benchmark: one fixed flight, a score, a regression guard](synthesis/flight-benchmark.md) and [Fog over unresolved pixels](techniques/unresolved-fog.md) (code deleted), a switch-names note on [almondbread benchmarks and verification record](synthesis/benchmarks.md), its gesture-probe paragraph (no longer opt-in), the M6 row of [Roadmap](roadmap.md) (AT deleted). Other pages already carried "removed 2026-10-17" notes.
- Technique homes: new section "Gesture resolution and GPU clocks" in [Metal compute pipeline for a deep-zoom renderer](platform/metal-compute.md) (the explanation; numbers stay in [almondbread benchmarks and verification record](synthesis/benchmarks.md)). The other techniques already had one home section each (interior-detection probe and closed-form test, auto-iterations "Compute further", unresolved-fog, bla "Pitfalls", places "Endless dives", controls driving levels, lane assist, survey steering, journey stops, cosmic scale); cross-links added.
- [Overview](overview.md) rewritten for the current state (LA without AT, interior tests, fog, compute further, controls, switches removed); [Index](index.md) one-liners updated.
- Links: no broken links, no orphans, `hyalo lint` clean. Left alone: the business pages (market, Steam, gamification, distribution and their sources) are still in the tree but not in the index, although the 2026-10-17 entry says they moved out of the repository.

### decision | Seek minibrot (N)

- Built the user's "seek minibrot": N (menu View ▸ Seek Minibrot, iOS long press on the plane
  button) finds the lowest-period minibrot in or near the view (ball period detection over discs
  of 1, 3, 9 view spans, Newton refined until its last step is far below the size), tells
  minibrots from satellite bulbs by the attracting cycle at w = −0.6, and flies to a framing of
  3.2 |s| centred inside the cardioid. From a minibrot's framing it goes on to the next smaller
  one: the tuned copy of the period-3 minibrot (period 3p) or a lower period in a ring of discs
  around it. Bounded (4 s, cancel flag, fixed discs and retries). Home → 3 → 7; needle →
  4640 → 13920. [Controls: keyboard, pointer and touch](decisions/controls.md) "Seek minibrot".

### decision | Orbit overlay: dot cloud on the whole-set map, plain result text

In slow-escape regions the orbit overlay showed 1000 iterates while the action is at step
10^4 to 10^5, and its text read "analysing ..." / `|z50| 0.5362` (user). The probe now runs at
least 10^5 steps and keeps a decimated `OrbitCloud` (10^5 points, stride doubling); the
whole-set map draws the first 50 steps as a line and the rest as one-pixel dots from a CPU
picture blended by the overlay kernel (0.43 ms per rebuild of 10^5 points, rebuilt every ~19 %
of growth). The text says "escapes after 48 213 steps", "settles into a period-34 cycle",
"still undecided after 100 000 steps". [Controls: keyboard, pointer and touch](decisions/controls.md) "Orbit overlay".

### decision | Closed-form cardioid / period-2 bulb test at any depth

User report: views at the main cardioid's and the period-2 bulb's boundaries (the −3/4 neck at
4176×) showed large fog areas that compute further could not resolve. Built an exact test:
per level the CPU expands both closed forms in BigFloat as degree-4 polynomials of the
kernel's texel offset. The kernel marks a fresh texel interior iff the float32 sum is below
−(2^-16 · sum of magnitudes + slack), which is a proof at any depth. Neck at 4176×: 45 % →
0.05 % unresolved, GPU 25 → 2.5 ms. Cusp at 1e-9: 100 % → 0 %, 210 → 4 ms. No class mismatch
anywhere. The user's second view (−0.6443−0.3662i) is inside a period-64 component, not the
cardioid, and is unchanged. Page: [Interior detection](techniques/interior-detection.md) "Closed-form component
test".

### decision | Cosmic scale reworded; toolbar slimmed, cockpit icons clickable

The user reworded the cosmic scale's deep phases.
- Readout subject: "The whole Mandelbrot set" ("set" alone is jargon for newcomers).
- Phase 2: "… would be as big as an imaginary universe" / "in which an atom is as big as ours".
- Phase 3: imaginary universes nest. At each level the Planck length is as big as the universe below, so a level is P = log10(U / Planck) ≈ 61.7 decades. The readout says "…, 16 levels deep" / "at the bottom, 1/72,000 of a proton is as big as ours".
- The atom-ratio nesting (and `level_decades`) is gone. Ruler marks "Earth = universe" … "Planck = universe", then "3 levels" … "52 levels".

The π meter, autopilot and journey buttons left the macOS toolbar: the π meter is niche, and the cockpit shows the levels. The cockpit's icons now take clicks: manual stops (Esc), cruise starts cruising in (E), autopilot and journey toggle (Z, J). The pointer shows a hand over them as over (?) badges. [Controls: keyboard, pointer and touch](decisions/controls.md).

### decision | Cockpit polish; two-finger scroll setting; help popovers at the bottom edge

User review of the cockpit:
- Icons spaced wider; the cockpit is drawn up to 1.3× where the window has room (base size on
  a phone).
- The pointer turns into a pointing hand over every (?) badge.
- The cockpit's (?) did not open its popover. The canvas popover was anchored in the safe-area
  frame (one toolbar lower than the badge rects, which are in the full canvas frame), so it fell
  below the window for a badge at the bottom edge. The popover modifier now sits inside
  `ignoresSafeArea`. `--ui-demo=help-cockpit` presses the badge through the real hit test.

New setting "Two-finger scroll: Pan / Zoom" (macOS, persisted, default pan). With Zoom, vertical
two-finger scroll zooms about the pointer (0.01 e-folds per point), sideways does nothing,
click-and-drag pans, and a cruise stops as on any manual zoom. Shifting gears with sideways
swipes was rejected: diagonal scrolls would shift by accident, horizontal swipes mean back /
forward in many apps, and continuous deltas feel mushy next to Q / E.

### decision | ⌘C / ⌘V on the canvas; jumps stop all motion

The user asked why coordinates used ⇧⌘C / ⇧⌘V. The canvas view now answers the standard copy: /
paste: actions itself (macOS routes Edit > Copy / Paste to the focused view), so ⌘C copies the
view's coordinates and ⌘V goes to copied ones while the canvas has focus, and text fields keep
normal copy and paste; ⇧⌘C / ⇧⌘V stay as menu items. A jump (pasted coordinates, a link, a
place) now calls `Navigator::jumped`: cruise, autopilot, flights and flings stop dead and a
journey ends (user request). [Controls: keyboard, pointer and touch](decisions/controls.md).

### decision | Behaviour switches removed: a decided choice is the only code

Too many `ALMONDBREAD_*` flags; nobody knew what they did and the combinations were untestable.
Rule: a behaviour flag goes once decided, the winning path becomes the only code, the losing path
is deleted. Default behaviour is unchanged.

- Flag and non-default path deleted (default kept): `ALMONDBREAD_FOG`, `_FURTHER`, `_PROBE`,
  `_PREVIEW_WAIT_BLA`, `_PROBE_ESCALATE`, `_WARM_PIPELINES`, `_ITER_FLOOR`, `_FILL`, `_SA`, `_LA`,
  `_BLA`, `_AUTOPILOT_LOCK`, `_HANG_WATCHDOG`. The `=auto` switches (SA, FILL, LA, BLA) keep
  exactly the auto behaviour; the enums `SaMode`, `BlaMode`, `LaMode` are gone. CLI options
  `--sa`, `--fill`, `--la`, `--bla`, `--bla-eps`, `--at`, `--further`, `--lock` removed
  (`--sa-*` tunables and `--further-budget s` stay). Tests that only compared a switch's two
  paths (SA/fill/BLA/LA on vs off) deleted.
- Opt-in features never enabled, deleted entirely: the motion sample cache
  (`ALMONDBREAD_SAMPLE_CACHE`, `_CACHE_RATIO`, `_CACHE_TEXELS`, `_CACHE_TRACE`;
  `sample_cache.rs`, `sample_cache.metal`, the cache level in the scheduler, the compose
  kernel's cache mode, the survey's toroidal read, `sample_cache_exact`, `verify --history`'s
  cache check, `GPU_TIMING`, `scripts/flights/cache-f14.json`) and the LA "AT" path
  (`ALMONDBREAD_AT`, `--at`, `AtInfo`, the kAT kernel code).
- Tuning knobs made constants (values unchanged): `ALMONDBREAD_BLA_EPS`, `_LA_F`, `_LA_SMAX`,
  `_CAPACITY`, `_NUCLEUS`; `ALMONDBREAD_SERIAL_COMPOSE` (measurement mode) deleted.
- Left alone: traces, measurement and test-harness variables, `ALMONDBREAD_GESTURE_PROBE`.
  CLAUDE.md now has one "Environment variables" table. Wiki pages that described a removed
  switch as current carry a "removed 2026-10-17" note; history pages (this log, benchmarks) are
  unchanged and still name the old switches.

### decision | Gesture resolution probes on by default

The user compared `ALMONDBREAD_GESTURE_PROBE=1` against the default in the foreground app (panning,
zooming, the autopilot near 1e45): smooth, sharp, no stutter noticed, hard to tell apart by eye. The
measured gains (autopilot ×4.5 vs ×2.2 in 2.4 s, preview resolution about doubled) decide: the probes
and median-fit hysteresis are now the only gesture-resolution path; the switch is removed
([almondbread benchmarks and verification record](synthesis/benchmarks.md) "Autopilot throttle and GPU clocks").

### decision | Driving levels: M · C · A · J, pedals, reverse cruise, lane assist hook

The user's car-like design: manual, cruise, autopilot, journey, shown in a Rust-drawn HUD
cockpit with a zero-centred speed gauge and a lane-assist icon. Q / E are pedals on one signed
ladder (×1.5 … ×1000 per second) with a detent at zero; held keys step on. Reverse cruise boosts
with depth, `(1 + D/3)^ramp`, ramping over 4 s, and hands the last stretch to the soft landing:
home from 1e-10 / 1e-50 / 1e-300 / 1e-3000 in 17 / 29 / 42 / 58 s at the first rung. User
steering or pedal-to-zero in A or a journey leg hands over to cruise at the current speed; the
autopilot no longer takes the wheel back. ⇧Z removed. Lane assist (L) is a setting and a hook,
wired to the survey steering in phase 2. Page: [Controls: keyboard, pointer and touch](decisions/controls.md) "Driving levels".

### decision | Autopilot survey steering (opt-in)

User report: Z often flies into the void near 1e247 (−1.7686678314…, 0.0016455577…).
Diagnosis with `fly --log` and `ALMONDBREAD_LOCK_TRACE=1`: the lock finds only probe
"boundary detail" targets there (no nucleus or Misiurewicz point resolves); they are exact to
1/40 of the span, run out of detail 3–6 e-folds deeper, and when the search ahead fails the
lock keeps aiming at the spent point until a rescue. New `ALMONDBREAD_SURVEY_STEER=1` (off by
default): whenever no lock target is followed, aim at the GPU survey's boundary point (per
cell the highest-escape-count texel, a new field of the survey kernel), kept while its region
holds detail, uphill in escape count when nothing is detailed. User point, 90 s × 5
interleaved runs: uniform 6.7 → 1.3 s per run, rescues 6 → 1, zoom 25.4 → 26.8 decades;
other deep paths unchanged (no voids either way). Page: [Controls: keyboard, pointer and touch](decisions/controls.md) "Survey
steering".

### decision | Endless dives: places centred exactly on Misiurewicz points

New place category "Endless dives": `endless-forking` (c = i), `endless-spiral` (M(24,2)),
`endless-junction` (M(4,1)), each repeating every |λ| times magnified turned by arg λ (5.657 at
45°, 1.0789 at −14.87°, 1.3283 at 119.55°), and `endless-wander`, a never-repeating point from
`locate --endless-seed 2026` (1,000 random angle bits, ray tracing + Newton, 42 s; changes down
to ~1e-295, then repeats ×4.605). M(24,2) and M(4,1) extended to 3,307 digits (agree with the
stored digits). Page: [Places: a catalogue of famous views to visit](synthesis/places.md) "Endless dives".

### decision | Unresolved pixels drawn as fog

The flat dark grey veil over unresolved pixels became fog (user: "foggy, cloudy, darker and
fuzzier towards the inner part"): light at the border in the blurred colours of the resolved
pixels around, darker and softer with distance in display points (dark from ~128 pt), almost
black deep inside with a faint blue-grey tint. A pyramid of resolved coverage and colour (one
sampled texel per 2 × 2, box halvings, even/odd atlases) and a walk from fine to coarse give
the distance in octaves and the colours at that scale; resolved at 4 × 4 texels, sampled
bilinearly by `colorize` and `compose` (refine, exact, compute further). Gesture frames,
previews, the sample cache and AA samples draw the plain deep fog. Built only when the view has
unresolved pixels: 0 otherwise, +1.0 ms colour pass / +1.0–1.4 ms compose frame at
3456 × 2234 with ~60 % unresolved (M1 Pro, release; `ALMONDBREAD_COLORIZE_BENCH`).
`ALMONDBREAD_FOG=0` restores the plain fog. A texture from the orbit at the limit (log |z|,
the probe multiplier) was looked at and dropped: near-constant near parabolic points, chunk
seams elsewhere. The `verify --history` compute-further check now compares the texels it
resolved with a fresh render by field data (the fog has no one colour); it found the last
pass's texels left part faded (12 %), so compute further's compose runs on for 4 τ after the
last pass. Help texts say "fog". Page: [Fog over unresolved pixels](techniques/unresolved-fog.md).

### decision | Zoom-out autopilot (⇧Z) and journey with stops (⇧J, Z / X at a stop)

⇧Z flies a van Wijk–Nuij path from the view to the home view at the autopilot's speed with a soft
landing that rides the brake curve in closed form (the fly-to landing reached the end still moving);
from 1e-3000 without overflow. The journey's new "stop at each find" mode (control center setting;
J the set mode, ⇧J the other) rests at each landmark until J (as planned: the continuous journey's
target), Z (`plan_with` `Choice::Deeper`: ≥ 1 e-fold deeper in this view, else the plan with
"nothing deeper here") or X (`Choice::Elsewhere`: cross over now). Reproducible from seed, start view
and the choices; `journey --choices jzx`. Page: [Controls: keyboard, pointer and touch](decisions/controls.md) "Zoom-out autopilot",
"Journey with stops".

### decision | Previews wait for a new reference's BLA table

Autopilot stalls of ~200 ms near 1e-247 (user report on bb69748): the first preview after
each new reference started without BLA (the table builds on a thread after the job asks) and,
as a gesture frame, ran in one K = maxIter chunk sized by the cost measured with BLA: 200–1090
ms of GPU per new reference, a second time when a truncated reference was replaced. Present
on b466f42 too; not caused by the interior probe, compute further or the gesture probe (during
the dive every job is a gesture job: no full levels, probe chunks, further passes or AA). Fix:
a job's first level waits for the table (1.3–6.4 ms) and the history is shown meanwhile
(`ALMONDBREAD_PREVIEW_WAIT_BLA=0` restores the old behaviour). Largest chunk 937–1086 →
10–14 ms, longest unpresented run 13–15 → 0–2 refreshes. Page: [Bivariate linear approximation (BLA)](techniques/bla.md) "Pitfalls".

### decision | Cosmic scale: nested-universe count off by one

The user asked whether the cosmic comparison's maths is right. Phase 1 and 2 check out (set width
`L · 2.25 / w` with the window in points; the universe is reached at zoom ≈ 3e27 for 30 cm, so a
flight passes all of astronomy in its first 27 decades). Phase 3 counted `max(2, floor(n / a))`
universes, one short: at `n = a` the set is already a universe whose atoms are universes. Now
`floor(n / a) + 1` ("… (27 times)" at 1e1000), and the "k× nested" ruler marks sit at
`n = (k - 1) a`. [Controls: keyboard, pointer and touch](decisions/controls.md) "Cosmic size scale" updated.

### decision | Compute further after AA; off by default in `verify --history`

Compute further now waits for the view's AA samples, then marks the texels still unresolved
(one bit each) and composes only those as they resolve, so a still view is anti-aliased as
soon as before (neck 1.9–2.3 s against 2.3 s on main, seahorse and home unchanged) and keeps
its samples. New `verify --history` check: resolved pixels drawn in the unresolved colour (0
on home, seahorse and the neck with AA; 14 % with the compose mode disabled on purpose).
`verify --history` defaults to `--further 0`; one regress session (neck, 400 × 250, AA, 4 s
budget) runs it: `make regress` 302.8 s against 300.2 s on main. Page:
[Automatic maximum-iteration control](techniques/auto-iterations.md) "Compute further".

### decision | Compute further: background passes up to the Upper limit

Refined views left unresolved by the levels and extension passes keep iterating exactly those
texels in passes of x4 the limit, each over the view in segments of at most the unresolved
list's capacity (a GPU scan of the iteration texture lists a segment's unresolved texels), up
to the Upper limit, the reference's exact reach (a cut f64 orbit is extended on a thread) and
60 GPU s; small chunks, any input drops it. The learner's 4x depth cap stays: the base limit
keeps first frames fast (61 ms at 1600 x 1000 near the -3/4 neck) and the deep work is
incremental. Neck at 4.48e-12/px: 1600 x 1000 15.2 -> 3.78 % unresolved (200 k cap, 3.6 s),
0.06 % with a 10 M cap (14 s); 4000 x 2500, where the overflowing list gave no pass at all,
100 -> 6.9 % (24 s) and 0.064 % with 10 M (75 s). Matches `neck_truth` (15.1 % at 65 536,
~3.8 % at 200 k); class mismatch against the CPU oracle 0.020 % at 8.3 M, all chaos. HUD:
"computing further · ... · 12 % not finished", settled "3.8 % not finished at 200 k steps
(upper limit)". Page: [Automatic maximum-iteration control](techniques/auto-iterations.md) "Compute further". Image:
`~/Desktop/compute-further.png`.

### query | Autopilot check failure: GPU clocks trap the gesture resolution at 1/8

The interaction test's "autopilot" scenario (×3.16 in 2.4 s) failed on main at ×2.2–2.5.
Bisected over worktree builds back to the autopilot's own commit (4a1c949: ×3.3, passes); no
single commit: the gesture resolution is bistable. Apple's GPU clocks follow the load, so at
1/8 resolution the measured GPU time per texel rises 2.5x within ~15 frames, the
proportional cost model never leaves 1/8, and the autopilot throttles to ~0.4 from that
`fit`. Other agents' GPU load kept the clocks up, which is why it "passed on a busy machine".
Not the pipeline warm-up, the lock probes, the progress line or the watchdog (switches off:
same ×2.2). Fix in `Renderer`, opt-in (`ALMONDBREAD_GESTURE_PROBE=1`, off until the user compares it in
the foreground app): upward resolution probes and a median-fit hysteresis. Autopilot
×4.3–4.7 in 8/8 interleaved runs (main 3/8), preview scale about 2x in most gesture
scenarios, skipped draws equal except a few more at dense views; numbers in
[almondbread benchmarks and verification record](synthesis/benchmarks.md) "Autopilot throttle and GPU clocks".

### decision | Attracting-cycle probe for interior detection

The `|w| n²` test proved only the fast core of a component (satellite: `|λ| < ~0.7` at 51
periods), so minibrot bodies and bulbs stayed unresolved grey. Pixels unresolved at the limit
now park and a kProbe pipeline variant searches their orbit for the cycle (near-0 base point,
candidate period from a return), runs Newton on `f^P` and proves an attracting fixed point
with a Kantorovich bound and float32 noise margins; repelling parents of satellite bulbs
resume the search for longer periods. No false interior in 19 views against the CPU oracle
at high limits; satellite proven 1.1 → 3.4 % of 3.7 %, cardioid edge 55.7 → 84.0 % of 86.1 %,
cusp 35.7 → 87.9 %. Near-parabolic views (neck, 1/3 root) stay unresolved and cost 10–40 %
offscreen. Pages: [Interior detection](techniques/interior-detection.md) "Attracting-cycle probe",
[almondbread benchmarks and verification record](synthesis/benchmarks.md) "Interior probe". Contact sheet: `~/Desktop/interior-before-after.png`.

### decision | Help: (?) popovers, a help window, Markdown texts

Help in three layers ([Controls: keyboard, pointer and touch](decisions/controls.md) "Help"): (?) popovers in the control center's
section headers (Iterations & quality, Look & colour, Interior, Overlays, Controls) and on the
Rust-drawn panels (cosmic scale, π meter, mini-map row); one help window (Help ▸ Almondbread
Help, ⌘?; a sheet on iOS) with six pages; the texts as Markdown in
`Sources/AlmondbreadKit/Resources/Help/`. The panels' "?" badges are drawn and hit-tested in
Rust (`render::help`, `Explorer.help_hit`), checked before the compass and the maps; Swift
anchors a SwiftUI popover at the badge's rect from the one change-only model property
`canvasHelp`. Only the circle plus 2.5 pt takes presses. Screens: `--ui-demo=help-canvas`,
`help-control`, `help-window`.
Interaction test unchanged: every shown-frame check OK; the one failure, "autopilot" zooming
×2.2–2.3 in 2.4 s (check wants ×3.2), fails identically on the base commit b466f42 (measured
2026-10-17, M1-class Mac, release, loaded machine), so it is not from this change.

### research | Finding good places at extreme depth

How the deep-zoom community finds places past 1e100: shape stacking and Julia morphing (the
path to a minibrot is recorded in its shells; a morph at 1/2 and a doubling at 3/4 of the
log-depth to the target), Dinkydau's evolution method, Newton zooming in KF/F3,
Heiland-Allen's automated Julia morphing (angle extrapolation O(p²), Misiurewicz/nucleus
Newton O(p), the symmetry trap), domain maps and periodicity scans, FractalShark's Feature
Finder. Known deep places and records from YouTube descriptions (e275 in 2010 to e122768 in
2022; FractalShark's minibrot at 1e650452) and how they are shared (`.kfr`, descriptions,
web pages, orbit save files). Density is a colouring problem (KF iteration division,
log/histogram, DE and slopes); our flat 1.4 M-period needle minibrot is read as palette
cycles per pixel, an inference. Recommendations: `locate --frame f`, a thumbnail scorer,
period-aware colouring, a morph walker, a GPU domain scan. New pages:
[Finding good places at extreme depth (1e100 to 1e100000+)](synthesis/finding-deep-places.md), [At what fraction of the log-depth to a minibrot does each doubling appear?](questions/julia-morph-depth-fractions.md) and nine source
pages ([mathr blog: automated Julia morphing (2016-2021)](sources/mathr-julia-morphing-posts.md), [mathr blog: navigating embedded Julia sets in the hairs](sources/mathr-embedded-julia-navigation.md),
[mathr blog: atom domain size, Misiurewicz domains, periodicity scan](sources/mathr-domains-and-periodicity-scan.md),
[Heiland-Allen: Patterns in Inflected Julia Sets (2025)](sources/heiland-allen-inflected-julia-sets-2025.md), [Dinkydau: evolution zooms, Julia morphing simulation, ExploreFractals](sources/dinkydau-evolution-journals.md),
[Fractal Forums: Pathfinding in the Mandelbrot set - Revisited (2019)](sources/fractalforums-pathfinding-revisited.md), [Fractal Forums: How deep was the deepest Mandelbrot zoom you've ever made? (2016)](sources/fractalforums-deepest-zoom-poll.md),
[Deep-zoom videos: record and landmark YouTube descriptions](sources/deep-zoom-videos.md), [Wikibooks: Fractals/emndl (emndl_autotune)](sources/wikibooks-emndl.md)); "Used by" backlinks on seven
existing sources and one line in [Places: a catalogue of famous views to visit](synthesis/places.md). Nothing saved to `raw/` (read on the
web; the forum thread via the Wayback Machine).

### lint | Business pages moved out of the public repository

Before the first public push, the business pages moved to a private folder outside the
repository: the market and game/Steam research reports, the distribution and gamification
decisions, thirteen business source pages (store listings, pricing and conversion benchmarks,
Steam process) and their raw research notes. Links to them were replaced with plain text,
index entries removed, and price and conversion figures trimmed from the remaining pages.
Pages: [Index](index.md), [Ideas](ideas.md), [Open-source release: licence, distribution, CI](decisions/open-source-release.md), [Mini-Mandelbrot atlas](decisions/minibrot-atlas.md),
[Places: a catalogue of famous views to visit](synthesis/places.md), [How big is big? Zoom depth against huge numbers, one dial per up-arrow](synthesis/how-big-is-big.md), and the "Used by" sections of the
remaining market and engine source pages.

### decision | Open-source release

[Open-source release: licence, distribution, CI](decisions/open-source-release.md): MIT with saturnus's AI notice, `THIRD_PARTY_NOTICES.md`
(Inter OFL, colour-map licences, crates), `deny.toml`, and the pipeline: CI on push (Linux
Rust gates without the Apple-only ffi crate, Windows check of math and core, kb lint, a
clean-checkout macOS build), a tag-driven Developer ID signed, notarised and stapled `.dmg`
in a draft release (saturnus's six Apple secrets), and the `almondbread` cask in
`ractive/homebrew-tap`. Publication audit: no secrets or large files in history, no
GPL/AGPL crates, no copied code from KF, F3, Imagina or FractalShark; the business pages
are listed for the user to decide on. Donations dropped for the first release.

### decision | Cosmic scale: one mode, three phases, nested universes

Agreed with the user after the "N × the observable universe" version: the scale has one mode
(your window stays 30 cm or its measured width; the picker, "The set is X" and the cosmos
metaphor are removed from the UI, the FFI, the CLI and the core). Three phases from
`n = log10(set / universe)`: the set grows ("The whole set would be as big as Earth"), the
universe shrinks inside the set with set/U = U/thing ("In the whole set, our universe would be
as small as an atom", zoom ≈ 1e64; Planck at ≈ 1e89), then nested universes from the Planck
rung, level `max(2, floor(n / 36.92))` ("A universe in a universe", "… (26 times)" at 1e1000,
"(80 times)" at 1e3010; line 2 "imagine every atom were a universe, and every atom in it too").
Ruler: things, "1,000 × universe", "universe = galaxy" … "universe = Planck", "googol ×",
"3× nested" …; linear to the Planck rung, then slope 1/8; marks ≥ 25 pt apart. No light times
or GeV anywhere. Long sentences wrap onto a second line. Contact sheet of the stills:
`~/Desktop/cosmic-scale-stills.png`. Page: [Controls: keyboard, pointer and touch](decisions/controls.md) "Cosmic size scale".

### decision | Render progress: a top line and HUD text for long renders

A thin line at the top of the canvas (Rust overlay, below the toolbar) and a HUD line
("refining · 2.1 M of 10 M steps · 3 % of pixels left · ~40 s") while a settled view computes
for more than 0.5 s. Time-based and pessimistic: remaining = the level's pixels over their
resolve rate blended with the youngest live pixel's steps to the limit (new `liveMinN` chunk
counter), plus predicted extension and AA stages. Measured with the new `almondbread progress`
command: at a 2 M limit at the neck (3–4 min) the line stays within −10 / +6 points of the
share of real time, at a 10 M limit with a slow tail (8 s) within ±10. Shallow zooms show
nothing. Found: the first AA sample at a 2 M limit costs as much as the full level. Page:
[Controls: keyboard, pointer and touch](decisions/controls.md) "Render progress".

### query | The −3/4 neck gap at 1e8–1e9: ground truth, GPU check, seed horizon

User report: just above the neck (`−0.74999996… + 0.00036918… i`, 1.9e8–1.3e9, −3.2°) the view
is almost black with thin filaments, areas appear while zooming, and a 10 M limit changes
nothing. New tool `examples/neck_truth` (direct f64 with an interior proof, escape histograms,
`|dz/dc|`, double-double checks, `--paint` for any `.iter` dump). Findings
([The −3/4 neck gap at 1e8–1e9 (escape statistics, what is true)](synthesis/neck-escape-statistics.md)): everything in view escapes (minimum ~8 500 ≈ π/ε,
medians 4e4–4e5, a disc at 2e7–7e7), escape counts are chaotic far below the pixel (f64 vs
double-double differ for 9 % of pixels; only statistics are meaningful); the GPU's float32
histograms at 1e6 and 1e7 match the f64 oracle to sampling noise, and the straight seam of
`render --iterations 1e6` is real structure. The app's black: the learner caps at 4× depth
(15–18 k) whatever the user's cap, the extension list overflows at Retina sizes, and with a
manual 10 M the local SA seeds stalled the level (no pixel ran in 20 s at 4000 × 2500; fixed:
`SEED_HORIZON` = 2^16, [Local series approximation (tile and pixel seeds)](techniques/local-series-approximation.md) "Seed horizon"); after that the
frame is a 10–20 minute render in progress, whose early escapes look like filaments ending
mid-screen. `f77cf7b` is not involved (same reference, 7 283 203 points, escaped). CLI: `--cap n`
(auto-iteration cap), history `--timeout s`, `ALMONDBREAD_SNAPSHOT=png` (the shown frame at a
history timeout).

UI proposal (not built): (1) draw unresolved pixels (`ITER_UNRESOLVED`) in their own colour,
a dim desaturated tint of the palette's deep colour with a faint fixed dither or hatch, so
"not escaped by the limit" never looks like proven interior; pixels still iterating in a frame in
progress the same, slightly brighter or animated; (2) a HUD line when the shown frame has more
than ~5 % unresolved: "62 % not resolved at 16 k iterations", with the reason (auto limit at 4×
depth / cap / extension skipped) and an action "Resolve deeper" that starts an idle background
refinement towards the user's cap, with progress and an estimate; (3) a progress ring or
percentage while a long manual-limit frame is being computed (the 10 M frame here), so a
partial frame does not read as the result; (4) in the settings, label the auto cap "Upper bound
for automatic iterations" and show the current effective limit next to it.

## 2026-10-16

### decision | Cosmic scale: "Your screen" keeps the window, the set grows

The screen anchor no longer turns around at the observable universe (the set became the
universe and the view shrank to the Planck length at zoom ~1e62: "very confusing", user). The
window stays its 30 cm (or measured) width at every depth; only the whole set grows: "The whole
set would be 3 km wide", "… as wide as Earth", "… as big as the observable universe", then "…
5.2 million × the observable universe", "5e102 ×", "5e972 ×", "5e2972 ×" at zoom 1e3000,
counted from the log so nothing overflows to the depth limit; line 2 "your window: 30 cm", no
third line. Ruler: things up to "universe", then "1,000 × universe" … "1 trillion ×", "1e20 ×",
"1e30 ×", "1e50 ×", "googol ×", every 100 decades to "1e4000 ×"; no small things, milestones or
ladders; marks at least 25 points apart (unit test). Set-as-object anchors: the third line is
plain words ("to see it you need X-rays", "Smaller than anything ever measured", "Smaller than
the Planck length: only maths from here") instead of light times and GeV; the "GUT scale?" mark
is gone everywhere (the metaphor grid now has 23 steps of 8.7 decades). Labels of marks close
to the marker are nudged off it instead of hidden. Stills: `build/cosmic-stills/` (not
committed). Page: [Controls: keyboard, pointer and touch](decisions/controls.md) "Cosmic size scale".

### decision | Hang prevention: loop audit, lints, extreme-value tests, main-thread watchdog

After two beach balls (the mini-map level loop at depths past 1e308; a SwiftUI layout loop
under an open menu) every loop and recursion whose end depends on computed values was audited
in all crates and the Swift sources. Real hazards found and guarded: the cosmic ruler's
`k += 1.0` decade ticks (endless past 2^53 or at inf), π meter ticks over f64 bounds, the
autopilot's `dt / 0.002` substeps at inf dt, Halton base 1, `frexp`/`exponent_of`/
`exponent_significand` recursing on 0, the float-float GPU loop without a dispatch cap,
`BigFloat::parse` computing 10^999999999 for pasted text (now `None` past 1e±100000), the
centre precision for a 2^-1e9 scale before the caller clamps it, precision bits at inf, NaN/inf
input events (a BigFloat panic) and NaN frame times in the navigator, inf view sizes. Clippy
`maybe_infinite_iter`, `while_float`, `infinite_loop` (warn) and `infinite_iter` (deny) are on
workspace-wide. 13 extreme-value unit tests (`cargo test extreme_`, ~10 ms in all). New
`HangWatchdog` (heartbeat every 0.5 s, report after 2 s with the main thread's stack via
public Mach calls and a ring buffer of recent events, to ~/Library/Logs/Almondbread) and
`--hang-test`; MetricKit hang diagnostics saved alongside. Pages:
[Hang prevention (loop guards, extreme-value tests, main-thread watchdog)](techniques/hang-prevention.md), [Apple: MetricKit MXHangDiagnostic and main-thread hangs](sources/apple-metrickit-hang-diagnostics.md).

### decision | Extension passes stop at the end of an f64 orbit cut at the limit

User report near the −3/4 neck (`−0.74999994719705121 + 0.00036918485351065104 i`, 4.5e-12 per
point): after zooming in to ~1e-13 and back out, black blobs with a straight-edged wedge and a
mostly black frame, while a fresh `render` looked right. Reproduced headlessly with
`verify --history zoom --scale 8e-14 --angle -185.69 --seconds 5.75 --then 0.05,0.5,0.25`
(`--then` now takes a list of steps). Not the learner, local SA, tile fill, LA or the sample
cache: the f64 reference (views ≥ 1e-12 per point) was computed to exactly the limit (~4 400
there, the centre escapes at 17 019), and the extension passes to 16× restarted every pixel at
that cut with the rounding loss of a truncated reference. Medians of |Δmu| against the oracle
1.9–6.5 and class mismatch up to 25 % before; ≤ 3e-4 after. Fix: `Reference::exact_reach` caps
the extension for every reference; the f64 orbit is computed as far as the passes may go. Bench
iteration counts and verify metrics are unchanged; the regression is in `make regress`
(`--max-cpu-median 0.05`, a new history gate). Pages: [Rebasing (Zhuoran, 2021)](techniques/rebasing.md) (Truncated
references), [Automatic maximum-iteration control](techniques/auto-iterations.md).

### decision | Motion fixes: pipeline warm-up, gesture cost after jumps, lock probes, deeper journeys

Five reported motion bugs, measured headless and in background windows on a shared machine.
(1) Fly-to "pauses": the motion model never stopped in eight flights; the picture froze for
161–310 ms where a flight first needed a new iterate variant (BLA, LA, deep kernel) with a
cold Metal shader cache, and up to 250 ms when other processes held the GPU. (2) The π hunt's
127–146 ms first frame was the same: its place look turns on DE and interior colouring. Fix:
the renderer warms all 105 variants on `cores / 3` threads from its first frame
(`GpuBackend::pipelines_in_background`); cold, no slow frame after start-up, π start 0.7–0.9
ms. (5) "Spiral zoom" failed because after a jump the gesture frames used the old view's cost
(or none): 1/2–3/4 resolution as one 2.6 s command buffer, the frame thread blocked in
`commandBuffer`; a jump now resets the gesture resolution to 1/8 until it is measured
(headless gesture frames 738 → 10 ms, settled 3 s after the input instead of never); the
remaining window failures are load (GPU 76–100 % busy with other processes), threshold kept.
(4) The target lock at sea-8007 found nothing: CPU probes at 30 000 iterations read the view as
interior; the view probe now escalates to 240 000 (uniform 5 → 0 %, rescue 2.6 → 0 s).
(3) Journeys: deeper dive styles and smaller period-search discs for deep dives (median dive
1e4.5–1e7.2 → 1e8.4–1e16.9); each leg's path is probed and self-similar stretches (look change
< 0.15 per e-fold) fly 3× faster (Pine Compass tail 5.9 → 2.0 s); black frames not reproduced
in 62 700 recorded frames. Pages: [Metal compute pipeline for a deep-zoom renderer](platform/metal-compute.md) "Pipeline warm-up",
[Controls: keyboard, pointer and touch](decisions/controls.md) (Fly to stalls, target lock, Journey revision),
[π in the Mandelbrot set (neck and cusp escape counts)](techniques/pi-in-the-mandelbrot-set.md), [What is the exact Metal GPU watchdog limit on macOS and iOS?](questions/metal-watchdog-limit.md),
[almondbread benchmarks and verification record](synthesis/benchmarks.md) "Motion fixes".

### decision | Needle axis band for local SA; AA samples filter over their share; learned limit survives AA

Three rendering-quality reports, each reproduced headlessly first. (1) Needle pixels on and
next to the real axis: M5 seeds start a pixel from an off-axis leaf with an isotropic error,
and near the needle the escape depends on `Im c` alone; on-axis pixels escaped (0.09–0.12 %
GPU-ext/CPU-int at the needles, a whole row at a shallow needle), the rows next to it had 14×
the plain |Δμ|. A real reference now skips seeds within 16 texels of the axis: 0 % class
mismatch, rows equal to plain, no cost. (2) "Learned limit resets after AA": not reproduced; the
trace line was the flight benchmark's fresh jump-check renderer. Test
`learned_limit_survives_aa` added. (3) Grey fine rings after AA: every AA sample was
palette-filtered over the whole pixel, then averaged (a second box filter); samples now filter
over `1/√N` of the pixel (the offscreen grid over `1/ss`): chroma 86 → 97 % of a 256-sample
reference, error −33 %. CLI: `verify --dump-cpu`, `--calm 0|1`. Pages:
[Local series approximation (tile and pixel seeds)](techniques/local-series-approximation.md), [Anti-aliasing and supersampling](techniques/anti-aliasing.md),
[Colouring methods](techniques/coloring-methods.md), [Automatic maximum-iteration control](techniques/auto-iterations.md),
[Flight benchmark: one fixed flight, a score, a regression guard](synthesis/flight-benchmark.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md).

### decision | Places zoom pills, full screen with F and Esc

Places thumbnails show only the zoom in a small dark pill at the bottom right; the name is the
tooltip (iOS: tap shows it under the grid, long press opens a menu titled with it). Full screen
on macOS: F toggles (as well as ⌃⌘F, ensured in the View menu), the control center closes on
entering and is restored on leaving, the toolbar auto-hides (macOS 15+), insets are re-sent
after transitions and on `contentLayoutRect` changes. In full screen Esc stops motion first
and leaves full screen only when nothing moved; repeats never leave. Interaction-test scenario
"esc full screen" checks it through a probe (no window enters full screen). Page:
[Controls: keyboard, pointer and touch](decisions/controls.md) "Full screen".

### decision | Truncated references: a prefix's end is no rebase point

Mini-map levels between ~1e300 and ~1e670 near `spiral-1e1000` settled as one flat
colour. Cause: the map's renderer got the 1/16 prefix of its centre orbit (R6
progressive delivery), and pixels that outran it rebased with `z ← Z + z` in float32
while `|z| ≪ |Z|`, losing their offset: all continued as one orbit and escaped at the
prefix end plus ~400 (median |Δmu| 1.1e4 against the oracle). Nothing polled the
worker while the job's key stood, so the whole orbit (20,899 points) never replaced
it. Main views were affected the same way after a jump (`verify --history jump --place
spiral-1e1000 --scale 1e-302 --iterations 50000`: 100 % escape counts wrong). Fix in
the scheduler: `Reference::truncated_before`, re-render on arrival of an orbit that
reaches the limit, no learning from provisional frames, offscreen waits, extension
capped at a truncated orbit's end; `verify --history jump|flyto` now fails on a median
|Δmu| > 10 against a fresh render. A second main-view case: at sea-8007 with
`ALMONDBREAD_NUCLEUS=0` the period floor lifted the limit to 115,436 past a 57,719-point
centre orbit (class mismatch 0.061 %, resolved 2.9 s); now 0 %, 0.5 s. In the flight
benchmark's heavy segment the shown limit is now 41,536 like the jump render (was 10,384,
the path-dependent black frame FB reported); noisy machine, no clean timing comparison. Pages: [Rebasing (Zhuoran, 2021)](techniques/rebasing.md) (Truncated
references), [Faster deep references: three-thread orbit, progressive delivery, cheap Newton](techniques/parallel-reference-orbit.md).

### research | Places below 1e1000: four views to the depth limit

Four deep places: `pinwheel-1e1500`, `star-1e2000` (M(5,1), a four-way junction found with
`locate --misiurewicz 5,1`), `swirl-1e3000` and `rock-bottom` at 1e3007.5, one doubling above
the depth limit (2^-10000 per point) in a 1000-point window. Deeper Misiurewicz points only
repeat themselves, so these are embedded Julia sets: `locate --radius r` next to c = i, M(4,1)
and M(5,1) gives a minibrot of size ~r²; centred on it, the junction appears with 4k arms at
the geometric mean and 8k at 3/4 of the log-distance (by eye). Newton found no minibrot next
to the seahorse spiral M(24,2) at any radius. Cost with `verify --history jump` at 1600×1000:
first frame 85 ms to 1.0 s, resolved 0.2-1.1 s, all of it the reference (period 2659-17046 at
5120-10176 bits); class mismatch 0 against the oracle. Page: [Places: a catalogue of famous views to visit](synthesis/places.md).

### decision | Control center: settings only, lazy sidebar, place captions

The user decided the sidebar holds only what can be set. Removed every live value (centre,
zoom, scale, rotation angle, motion and journey status lines, current iterations, the
Diagnostics section, orbit / map-note / π readouts) and `panelStats`; buttons read change-only
model state. The HUD now shows zoom, centre and speed; iterations and the diagnostics are its
developer details, off by default (Overlays, View menu, Settings). On the Mac the grouped
`Form` became a `ScrollView` + `LazyVStack` with one stack row per settings row (and per tile
grid row), styled like the grouped form; iOS keeps its `Form`. Places thumbnails show name and
zoom on a caption strip. Measured (3 runs each): rendering with sections open 23 % → 14 % main
busy (collapsed baseline 13-16 %), no sidebar body re-runs while rendering; section toggles
160-400 ms → 10-20 ms; Places open/close 433 → 84 ms; search rebuild 425 → 78 ms. Page:
[SwiftUI performance: the control center sidebar](platform/swiftui-performance.md), [Controls: keyboard, pointer and touch](decisions/controls.md).

### decision | Control center performance: invalidation, search, thumbnails

The user reported stuttering search, Places thumbnails that visibly load, and slow
accordions. Measured with a new scripted background scene (`--ui-demo=perf`: body counters,
main-thread busy time from run-loop observers, per-action latency, `sample`). Causes: the
rows of open sections reading `stats` re-ran at 10 Hz through `ForEach` (main thread 43 %
busy while rendering vs ~9 % collapsed); on macOS the grouped `Form` is a non-lazy SwiftUI
stack, so any row change lays out every open row again (~10 ms per update, toggles 60-450 ms
depending on what is open); search text was form state (worst keystroke 0.6-0.9 s);
thumbnails were decoded on first view at full size (733 ms to fill, ~71 MB). Fixes: sections
as view structs, leaf views, change-only `zoomSpeed`, `panelStats` at most 4 Hz (busy 23 %),
a search field that owns its text with a 120 ms debounce to the form (one rebuild per pause),
thumbnails pre-decoded at launch at display size (13.3 MB, all shown in the first frame, 51
ms), and a fixed-geometry `TileGrid`. The toggle cost is not fixed: it needs a lazy container
that would change how the sidebar looks. Page: [SwiftUI performance: the control center sidebar](platform/swiftui-performance.md).

### decision | Compass

Built a compass after Apple Maps: a rose at the top right with a red north needle, shown while
the view is turned (or always, a setting), fading 3 s after north. Click or tap eases back to
north in 0.4 s (exactly 0, the spin of an autopilot, flight or journey goes on from north;
flights are turned along); drag turns the view 1:1. Rust draws (new triangle overlay
primitive), hit-tests and moves; Swift routes presses. Mini-map panels now respect the
safe-area insets. Pages: [Controls: keyboard, pointer and touch](decisions/controls.md) "Compass", [Index](index.md).

### decision | Autopilot target lock and rescue

The user reported that plain Z and Z with assist both end in featureless regions. Why: a
continuous zoom towards a generic point ends on one side of the boundary; the centroid assist
lags. Built: the target lock (Misiurewicz points, Julia stages, minibrots found off-thread with
the journey's finder, retargeted ahead of each end span, minibrots followed through, gain law
for the heading, pointer always wins) and rescue (backs out of a uniform heading region), on
by default with a control-center toggle and `ALMONDBREAD_AUTOPILOT_LOCK=0|1`; the assist is
gone. Headless tools: `almondbread autopilot` (CPU simulator), `fly --lock`, fly metrics.
Uniform share over 90 s went from 94–100 % (plain Z) to 0–8 % at the default speed on 11 of
12 starts; high-period minibrot starts at max speed stay weak (CPU probe iteration cap).
Pages: [Controls: keyboard, pointer and touch](decisions/controls.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md).

## 2026-10-15

### decision | π hunt: the view stays in the valley, a probe and the orbit trail

User feedback: "Since the π hunting is only on black regions, it's not really exciting." The
guided flight zoomed into the pinch point, where the gap is ~ε² and the image honestly black.
Reworked: the view eases to seahorse valley (neck) or elephant valley (cusp), chosen from six
rendered candidates each, and a glowing probe slides down the line on a log ruler (true
position as text) while the meter counts it. The counting thread now also accumulates the
probe's orbit as a glowing density picture (published every 0.12 s, uploaded as a shared
buffer, added by a new overlay primitive; overlay binning made two-pass, 0.68 → 0.05 ms). Looked first: in the full z-plane the orbit is a cross at
the parabolic point; zoomed to ±4√ε the neck's two arms pass the gate between the fixed point
and the 2-cycle, the same shape at every ε; the cusp's orbit is real, and plotted against √ε·n
it is the tangent curve with the escape at π; in `1 + iε/w²` (neck) or the Cayley angle (cusp)
the orbit goes round the unit circle once at 2ε (2√ε) per step, which is where π comes from.
The inset shows the orbit plot and that turn circle. Tracing costs 20 % (neck) and 5 % (cusp)
of the count. The magnifier at scale ε was not built (it would be black and cost π/ε
iterations per pixel). Menu items renamed "Hunt π at the Neck/Cusp". Pages:
[π in the Mandelbrot set (neck and cusp escape counts)](techniques/pi-in-the-mandelbrot-set.md) "The orbit", "In the app"; [Controls: keyboard, pointer and touch](decisions/controls.md)
"π meter".

### decision | "How big is big?" panel removed

The user removed the panel: "It's an own window, which is not like the rest; way too much text;
not sure if people understand it." Gone from the app: `HowBigView.swift`, the `how-big` window
scene, its View-menu item and control-center button (and search keywords), the `howbig`
ui-demo scenes, the cosmic size scale's "How big is big? →" link with `Explorer.cosmicHint()`
and its Rust side (`CosmicOverlay::hint`, `Host::cosmic_hint_points`, `CosmicScale::past_planck`),
`almondbread-ffi/src/how_big.rs` and `almondbread-core/src/bignum_scale.rs` with its tests.
The maths (linear super-log convention, marker table, facts) stays in
[How big is big? Zoom depth against huge numbers, one dial per up-arrow](synthesis/how-big-is-big.md), marked removed, for achievements later; the sources pages stay.
Pages: [How big is big? Zoom depth against huge numbers, one dial per up-arrow](synthesis/how-big-is-big.md), [Controls: keyboard, pointer and touch](decisions/controls.md), [Ideas](ideas.md), [Index](index.md).

### ingest | π in the Mandelbrot set, and the π meter

Ingested Boll's 1991 post (via brainwagon), Klebanoff 2001 (cusp proof), Brockmoeller, Scherz,
Srkalovic 2025 (neck proof, all parabolic points, N = π/ε + O(1)), Gerald Edgar's page and
Mu-Ency's R2.C(1/2) entry. Measured the counts at both points against a 256-bit BigFloat orbit
and double-double: errors O(ε) (neck offset N − π/ε in 0.35 ... 1.6) and O(√ε) (cusp offset
about −2); textbook f64 never escapes at the neck from ε = 1e-8 and drifts at the cusp from
1e-12; f64 in shifted coordinates drifts at the cusp from 1e-16; a compensated (TwoSum) shifted
count matches double-double everywhere at 11–13 ns per iteration. Boll's 1e-7 neck entry
(31415928) is not reproduced (31415927); Mu-Ency counts one higher (convention). Built the
π meter, two places and guided flights. Raising the image's iteration limit near the points
was measured and rejected (8 s GPU per frame at the cusp, minutes at the neck). Pages:
[π in the Mandelbrot set (neck and cusp escape counts)](techniques/pi-in-the-mandelbrot-set.md), [Why do published π escape counts differ by one (Mu-Ency) or disagree at ε = 1e-7 (Boll)?](questions/pi-escape-count-conventions.md),
[Controls: keyboard, pointer and touch](decisions/controls.md) "π meter", [Places: a catalogue of famous views to visit](synthesis/places.md), [Ideas](ideas.md), [Index](index.md).

### decision | Cosmic size scale: relative mode paced in linear zoom

User: "From hair to blood cell it takes ages." The relative cosmos now spans zoom 1e0 ... 1e200,
with the things evenly spaced in zoom decades (about 12 s a step at the default speed). The
epochs share the same grid, placed by thermal wavelength. Past 1e200 the panel counts cosmos
lengths. Also: a darker backing and outlines, labels kept near their dots, a captioned anchor
picker, and a "How big is big?" link past the Planck length. Pages: [Controls: keyboard, pointer and touch](decisions/controls.md).

### ingest | How big is big? panel

Built the up-arrow gear-train panel and wrote [How big is big? Zoom depth against huge numbers, one dial per up-arrow](synthesis/how-big-is-big.md): the linear
super-logarithm generalised to every arrow level, marker positions (googol 2.301, our limit
2.541, googolplex 3.301 on ↑↑; g₁ 2.364 on ↑↑↑↑), recomputed facts (googol after 2 min 24 s at
×4.95/s, googolplex zoom 4.6·10^92 years and 3.3·10^100 bits vs ~10^80 atoms). Finding: the
four-arrow dial and beyond read 1.15, 1.06, 1.03 … at our limit, not 1.000. Ingested (skimmed,
not saved) eleven web sources: [Wikipedia: Knuth's up-arrow notation](sources/wikipedia-knuth-up-arrow-notation.md),
[Wikipedia: Tetration](sources/wikipedia-tetration.md), [Wikipedia: Graham's number](sources/wikipedia-grahams-number.md),
[Wikipedia: Kruskal's tree theorem (TREE(3))](sources/wikipedia-kruskal-tree-theorem.md), [Wikipedia: Steinhaus–Moser notation (mega, Moser's number)](sources/wikipedia-steinhaus-moser-notation.md),
[Wikipedia: Skewes's number](sources/wikipedia-skewes-number.md), [Wikipedia: Googolplex](sources/wikipedia-googolplex.md),
[Wikipedia: Names of large numbers](sources/wikipedia-names-of-large-numbers.md), [Wikipedia: Fast-growing hierarchy](sources/wikipedia-fast-growing-hierarchy.md),
[Wikipedia: Observable universe (atom count)](sources/wikipedia-observable-universe.md), [Make: Arthur Ganson's "Machine with Concrete"](sources/make-ganson-machine-with-concrete.md).
Status in [Ideas](ideas.md).

### ingest | Cosmic size scale: sizes, epochs and the overlay

Built the cosmic size scale (key C, off by default) and filed its numbers: NASA fact sheets,
IAU units, CODATA 2022, a references page (galaxies, universe, cells, viruses, nuclei, the
quark size limit, big numbers, ages) and a cosmic-history timeline (times before ~1 s derived
from transition temperatures with the PDG radiation-era relation). Unconfirmed figures are
marked (unverified): adult height, the 93 vs 92 billion light-year universe (92 used), inflation
times. Design and costs: [Controls: keyboard, pointer and touch](decisions/controls.md) "Cosmic size scale"; status in [Ideas](ideas.md).
Pages: [NASA NSSDCA planetary fact sheets (Earth, Moon, Jupiter, Sun, Neptune)](sources/nasa-planetary-fact-sheets.md), [IAU astronomical unit (2012 Resolution B2) and the light-year](sources/iau-astronomical-units.md),
[NIST CODATA 2022 recommended values (Planck units, Bohr radius, proton radius, h, c)](sources/codata-2022-constants.md), [Cosmic size scale: sources for galaxies, cosmology, life, particles and big numbers](sources/cosmic-scale-references.md),
[Cosmic history timeline: epoch times after the Big Bang (PDG, Planck, NASA, ESA, CERN)](sources/cosmic-history-timeline.md), [Index](index.md).

### decision | Mini-Mandelbrot atlas proposed

Wrote up [Mini-Mandelbrot atlas](decisions/minibrot-atlas.md) (proposed, not scheduled): local atlas from the journey's
target finder first, then a shallow census and deep samples. Filed the π-in-the-set idea in
[Ideas](ideas.md) with our own numeric check.

### decision | Sample cache off by default after user test

The user found the merged build sluggish: blurry zoom-out and less smooth fast zooms. Feature
bisection with env switches in the real window traced it to the sample cache; M5 and M6 felt
as smooth as or smoother than the previous day. The cache is now opt-in
(`ALMONDBREAD_SAMPLE_CACHE=1`). Lesson: headless replays and background windows (capped near 30 Hz)
do not show interactive feel; a speed feature needs a foreground check before it becomes the
default. Pages: [Sample reuse while zooming and panning (sample cache in fractal coordinates)](techniques/sample-reuse.md).

### decision | Sample cache: motion sample reuse in production

Built the sample cache R4 recommended ([Sample reuse while zooming and panning (sample cache in fractal coordinates)](techniques/sample-reuse.md) "As built",
[Motion sample reuse: fewer iterations per gesture frame (R4)](synthesis/motion-sample-reuse.md) "Production"). R4's free per-pixel samples and move pass
were replaced: samples sit on a lattice fixed in the plane, held as a toroidal window of
16 × 16 tiles.

- A pan clears and computes the entering tiles; a zoom switches the level and resamples the
  coinciding samples on the GPU.
- The rest seeds the cache from the refined frame.
- Planning is per tile on the CPU, with cost estimates from per-tile shade statistics,
  latency-bounded chunks, and batches that carry over between frames.
- Exactness: epochs pin the BLA/LA/interior regime, a jump resets the cache, and `verify
  --history` and `sample_cache_exact` check every cached sample against fresh renders.

Bookkeeping is 0.1–0.6 ms per frame (budget 1.5 ms). Iterations per gesture frame fall by
55–92 %; frame rates are equal or higher (spiral 58–110 vs 11–13 fps). The compose pass costs
~1 ms more at Retina size. LA views (1e-305 needle) keep fresh previews. Pitfalls are filed on
the technique page: superseded jobs wasting budgeted batches, latency vs throughput, biased
interior statistics, `%` in per-pixel code, overlapping command buffers. Brought in the R4
pages, sources, raw notes and prototypes. Numbers: [almondbread benchmarks and verification record](synthesis/benchmarks.md) "Sample cache".
Pages: [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md), [Index](index.md).

### decision | FB: window test took focus, fixed

The flight bench's window run activated the app (three of four runs; one with a key
window). `AlmondbreadMac/main.swift` activated a bare executable unless `--selftest` or
`--interaction-test` was given. It now checks `TestLaunch.enabled`, which covers
`--flight-bench=` and `--ui-demo=` too. The bench ends with `FOCUS TAKEN` and exit 1 on any
activation or key window; 5 runs were clean. Also explained `stats.max_iterations` 10384
against 41536 (nominal limit vs extension to the hard limit) and the 1e−1004 disc
(supersampling of sub-pixel rings). Page: [Flight benchmark: one fixed flight, a score, a regression guard](synthesis/flight-benchmark.md).

### decision | Journey variety: dives, cross-overs, novelty

The journey planned well-framed targets but stayed in one region for a minute (one family of
shapes: deep in a region every view carries the same local Julia set). Now a journey is a
sequence of dives: a cross-over to another region (catalogue places to 1e16 and views from the
maths of p/q bulbs: limb hubs M(q+1, 1), valleys beside the attachment point), then 1–2 legs
in a seeded style (scenic, medium, deep); regions from the cardioid's internal angle, families
(valley, limb, needle) alternate. Every landing gets a look descriptor (distance spectrum,
structure dimension, interior, and the spiral pitch of the shadowed cycle's multiplier: the
only measure that told seahorse spirals from dendrite carpets on 40-texel probes) and a
novelty score; near-duplicates are rejected and a dive ends when nothing new is left. Long
legs fly faster; with cinematic effects each dive gets its own palette phase. Contact sheets
and three 3-minute review videos: 6–7 regions per 3 minutes instead of 2–3. Pages:
[Controls: keyboard, pointer and touch](decisions/controls.md) (Journey, variety revision), [almondbread benchmarks and verification record](synthesis/benchmarks.md) (PJ variety).

### decision | Overlay font, place thumbnail moiré, 120 Hz measurement, iOS simulator

Overlay text in Rust/Metal is now Inter SemiBold (OFL 1.1, licence shipped) as a
signed-distance-field atlas with a soft dark halo, crisp at any backing scale and legible on
pale and dark palettes ([Overlay text: SDF font atlas in Rust and Metal](platform/overlay-text.md)); `render --ppp` sets the overlay scale for
stills. Place thumbnails render 3× larger and resample with a Gaussian (σ 0.5 px, linear
light); the three needle minibrots were re-rendered without moiré ([Places: a catalogue of famous views to visit](synthesis/places.md)).
120 Hz: the interaction test reports the display link's actual and granted rate and can run
from a CVDisplayLink; a background window gets 20–33 callbacks per second (granted 120) and,
fed at 120 Hz, half the refreshes find no drawable, so real 120 Hz presentation needs a
frontmost window: `make interaction-test-120` for a person to run
([almondbread benchmarks and verification record](synthesis/benchmarks.md) "120 Hz on ProMotion"). iOS: simulator runtime installed, iPhone and
iPad simulators, self-test and an XCTest touch UI test; fixed a crash (presented handlers
missing on simulator drawables), a main-thread HEIC decode deadlock when the control center
opened, the HUD under the buttons on narrow canvases and the status bar
([iOS and iPadOS on the simulator: setup, checks, what broke](platform/ios-simulator.md)). Updated [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md), [Index](index.md).

### decision | M6: Imagina-style LA and AT built; used only where it pays

LA built in our BLA terms: a step is one exact perturbation step from a near-return of the
reference followed by an ordinary BLA block (validity `|u| + K|c| < R`, ε = 2^-24); stages compose
steps and linearise the boundaries in between; synthetic splits coarsen 4× where the orbit has no
period structure; chains per lookup index with a deterministic search (first, last, binary), so
fields do not depend on chunk cuts. Plain and deep kernels (`kLA`, `kAT`), the AT for nucleus
references with y kept in the pixel state between chunks. CPU model study first
(`examples/la_check`), then interleaved GPU A/B at all 21 locations under heavy shared load:
LA 1.9–2.4× faster than BLA at the three needle minibrots, 1.1× at sea-8007, even on centre
references (sea-1e20, dendrites, spiral: deltas non-linear, or the seeds did the work) and
1.4–5× slower forced into plain-kernel views. The AT saves 4–10 % of steps but no GPU time: off by
default. Auto: LA on deep nucleus references whose limit spans ≥ 4 periods, BLA otherwise; LA
does not replace BLA. Verify: 0 % exterior painted interior, class mismatch within 0.001 % of BLA
at all locations; regress all OK; selftest OK; interaction test OK except the autopilot zoom check
(×2.6–2.8 < ×3.16 at load average 534, the same with LA off). Found on the way: a pre-existing M5
real-axis row at the needles (seeded pixels escape where the oracle does not). Pages:
[Linear approximation with period-aligned stages and AT (Imagina)](techniques/linear-approximation-imagina.md) (As built), [almondbread benchmarks and verification record](synthesis/benchmarks.md) (M6),
[How much faster is Imagina-style LA than binary-tree BLA on a GPU?](questions/bla-vs-la-benchmark.md) (answered), [Roadmap](roadmap.md) (M6), [Overview](overview.md), [Index](index.md).

### decision | M7: float32 artefacts measured (none visible); float-float as scaffolding

Step 1 of M7 asked whether float32 rounding artefacts exist at all. New probe `almondbread
precision`: the float32 GPU field against the CPU oracle next to control oracles with the pixel
grid shifted by 0.001–0.5 px, one CPU colouring for both, signed error maps and crops. At 23 views
from 3e-7 to 4e-1003 per pixel (catalogue minibrots and spirals, sea-8007 at 1e5 iterations, the
period-32412 spiral, a needle minibrot at 1e1000 with 1e6 iterations) the float32 error is within
a 0.001-px shift at 17, 0.01 px at 5 and 0.1–0.2 px at spiral-1e1000; crops show no speckle, and
the smooth-region error at the deep spirals is smooth and coherent (|Δμ| ≤ 0.022), not noise.
Float-float (48 bits) and the f64 oracle still disagree on 6 % at the period-32412 spiral: chaotic
pixels have no stable value. So no repair pass, flagging or "max precision" setting was built.
Built instead, at zero runtime cost: the float-float + exponent kernels in their own library
compiled with `MTLMathMode.safe` through a new backend hook, a known-answer test that the
creation runs (passes under `.safe`; under `.fast` TwoSum and QuickTwoSum lose their low parts)
with float32 as the fallback, a pixel-list fx perturbation kernel at any depth used by
`verify --ff 1` / `precision --ff 1` (30–3000× closer to the oracle than float32), and number-type
benchmarks (float-float 7–12×, fx 27–52× slower than float32). `bench` A/B against main: geomean
1.014 under load, fields bit-identical. Pages: [How large is float32 perturbation error over long orbits?](questions/float32-error-long-orbits.md) (answered),
[Which extended-precision emulation is fastest and correct on Apple GPUs?](questions/double-float-speed-on-m1.md) (answered), [What does Metal's default math mode allow for runtime-compiled kernels?](questions/metal-fast-math-defaults.md)
(partial), [Double-float (float-float) arithmetic](techniques/double-float.md) (As built), [almondbread benchmarks and verification record](synthesis/benchmarks.md) (M7), [Roadmap](roadmap.md)
(M7), [Overview](overview.md), [Index](index.md).

### decision | M5 fix: learned state across jumps

The flight benchmark found the spiral settling black after a dive at c = i (1e-280): the
old view's interior-test statistics turned the test off and the learner could not retry
without it. Jumps now reset the learned iteration and gate state; a view at the limit without
the test renders again with it; `verify --history` checks against a from-scratch render; a
regress session runs the path. Page: [Automatic maximum-iteration control](techniques/auto-iterations.md).

### decision | FB: flight benchmark with a score

Built `almondbread benchmark` (`make flight-bench`, `make flight-bench-live`). The flight
`scripts/flights/benchmark-v1.json` is versioned, 60 s at 1920×1080: shallow pan and zoom,
the dense spot, a dive into c = i to 1e−300, the heavy period-32412 spiral, a rotating zoom
in elephant valley, and a jump to 1e−1004. Mode A renders every 5 Hz camera sample through
the real scheduler to the settled full-quality frame, checks it against a fresh render and
a from-scratch jump render, and reports wall, CPU and GPU time per segment. Mode B plays the
flight live: headless at 120 Hz, or in a background window (`--flight-bench=`, never key).
It reports shown/due frames, p99, late frames, input to photon, and time to first image and
to sharp per landing. The score is geomean(A, B) against a **provisional** M1 Pro reference
= 1000. Also: warm-up and repeated laps with medians and spread, a noise check (other
processes' CPU, IOAccelerator utilisation), JSON in `build/benchmark/`, `--compare`, and A/B
switches (`--sa 0`). The script player and real-time recorder moved into
`almondbread_core::flight`, shared with `fly`; `jump` and `location` events were added. The
first, noisy runs found a path-dependent black frame at the heavy spot after the dive (auto
iterations stay at 10384, below the period), reported for M5. Pages:
[Flight benchmark: one fixed flight, a score, a regression guard](synthesis/flight-benchmark.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md) (FB), [Index](index.md).

### decision | M5: local series approximation and interior fill in production

Shipped local SA ([Local series approximation (tile and pixel seeds)](techniques/local-series-approximation.md) "As built (M5)") on by default:
quadtree seeds 32 → 4 texels in resumable rounds (a node keeps its whole loop state; rounds
sized by a learnt latency model; pixel admission gated on the GPU until the seeds are final),
every level seeded at its own texel size, extension passes and AA samples from the full
level's leaves, float32 deep seeds down to 2^−80 texels (the spiral's seeds 150 → 40 ms), a
deterministic per-tile gate whose bar comes from the last level's steps per texel, Morton order
inside admission tiles, bounded rounds for gesture frames. Interior-only tile fill
([Guessing, border tracing and rectangle subdivision](techniques/guessing-and-subdivision.md) "As built (M5)"): borders first, insides of
all-proven-interior tiles written without iterating, bit-identical, gated (elephant 1.35×,
off with analysed interior colourings). New benchmark location `spiral-32412-1e19` (the
user's slowest view) and regress sessions; auto iterations take a floor from the big
reference and keep the interior test on while pixels end unresolved
([Automatic maximum-iteration control](techniques/auto-iterations.md)). Results ([almondbread benchmarks and verification record](synthesis/benchmarks.md) M5): dense-5e-14 4.95×,
sea-1e20 8.7×, neck-2e5 5.8×, root-third 3.3×, deep-5e-14 4.1×, dendrite-1e300 2.5×, spiral
3.96× (first render 4.0 → 0.78 s GPU), interactive refined frames 2–5× sooner, 0–3 % cost
where nothing is seeded; 0.000 % class change and no false interior at all 17 locations.
Pages: [Local series approximation (tile and pixel seeds)](techniques/local-series-approximation.md), [Guessing, border tracing and rectangle subdivision](techniques/guessing-and-subdivision.md),
[Automatic maximum-iteration control](techniques/auto-iterations.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md), [Roadmap](roadmap.md), [Index](index.md).

### decision | PJ: journey autopilot, view rotation, cinematic effects

Pilot track. Journey (J): an endless seeded flight from landmark to landmark; targets from the
maths (ball period + Newton minibrots at 128 extra bits, embedded Julia stages at t = 1/2,
3/4, 7/8 and σ^(3/4), approaches, Misiurewicz spirals and branch points from the orbit's
near-repeats, detail zooms, resurfacing to shallow places), scored on 40 × 40 CPU probes with
survey-style statistics plus a filament share and a roughness measure, and variety rules;
legs are fly-to flights with a rest; the next plan runs on a thread during the leg and the
next reference is prefetched during the rest. View rotation end to end (kernel, reprojection,
tile priorities, oracle, overlays, navigator keys and gestures, coordinates text, regress
sessions). Cinematic effects (spin with the zoom, OKLab hue and phase drift with depth,
cycling in flight). Ingested [Mu-Ency: Newton-Raphson zooming (embedded Julia set sizes)](sources/muency-newton-raphson-zooming.md) and
[Kisaka: Julia sets in the Mandelbrot set and small Mandelbrot sets (seminar abstract)](sources/kisaka-julia-sets-in-mandelbrot.md). Pages: [Controls: keyboard, pointer and touch](decisions/controls.md) (Journey, View
rotation, Cinematic effects), [Ideas](ideas.md) (status), [Roadmap](roadmap.md) (PJ), [almondbread benchmarks and verification record](synthesis/benchmarks.md)
(PJ), [Index](index.md).

### decision | Integration: autopilot, places, palette library and R6 into the control center

Merged main (autopilot, fly-to, deep DE fix, L2), the places catalogue, the palette library
and R6 (faster deep reference orbits) into the control-center branch. Control center:
autopilot and fly-to buttons, assist toggle and the autopilot keys in View & navigation;
Places as a thumbnail grid by category with Go, Fly (`Explorer.flyToPlace`) and Random;
the palette picker lists the library by category, applies a palette's density, offset and
deep colour, and imports .ugr / .map / JSON palettes into "My palettes" (JSON kept in user
defaults, registered again at launch). A library palette's deep colour gives the calm dense
regions their hue (at the calm luminance). Option-click on a mini-map flies there. Place
thumbnails re-rendered with calm dense regions and the bubbles interior
([Places: a catalogue of famous views to visit](synthesis/places.md)); Go / Fly switch a flat interior to bubbles to match. Checks after
the merge: the R6 orbit digest is identical to the R6 branch's; `verify --history jump` at
dendrite-1e1000 shows the first frame after 93 ms. Pages: [Controls: keyboard, pointer and touch](decisions/controls.md),
[Places: a catalogue of famous views to visit](synthesis/places.md), [Colouring methods](techniques/coloring-methods.md).

### decision | R6: faster deep references

Three-thread reference orbit (X², Y², (X+Y)² on the caller and two scoped helpers, one
all-to-all exchange per iteration through padded atomics, bit-identical; pays from 1536 bits,
1.8x at 1e-1000 and 2.2x at 1e-2000 on a lightly loaded M1 Pro; opt-in for the reference
worker and Newton). `ReferenceOrbitComputer::resume`; the reference worker hands out
centre-orbit prefixes (1/8 limit, limit, 2x) that the scheduler adopts with its existing
Stale/Good rules. Newton with an extended-range f64 derivative over the orbit code (~3x per
step, exact Newton finishes very close starts). Short products round down without the full
product when the round bit is clear (exact orbits like c = i 2.5x faster). Jump to
dendrite-1e1000: first frame 1.8 s -> 0.1 s; dendrite-1e300 0.32 s -> 0.03 s; needles
unchanged. Toom-3/NTT not built: the short Comba square beats full Karatsuba 1.5-1.8x up to
157 limbs. Pages: [Faster deep references: three-thread orbit, progressive delivery, cheap Newton](techniques/parallel-reference-orbit.md) (new), [almondbread benchmarks and verification record](synthesis/benchmarks.md) R6,
[Arbitrary precision: own Swift/C implementation, not GMP/MPFR](decisions/arbitrary-precision.md), [Reference selection: nucleus, period detection, Newton](techniques/reference-selection.md),
[Reference orbit: precision, cost, storage, compression](techniques/reference-orbit-precision.md), [Arbitrary precision on Apple Silicon (GMP, MPFR)](platform/mpfr-gmp-on-apple-silicon.md) (corrected
the unmeasured MT3 handoff guess), [Index](index.md).

### research | Fractal palettes and the palette library

Research run on fractal palette design (fractalset.com viewer, Ultra Fractal
gradients and `.ugr`, Fractint `.map`, KF palettes, cosine palettes, open
perceptual maps and their licences, OKLab interpolation, cyclic maps, density
mapping, dense regions, CVD safety). Notes saved unchanged under
`raw/research/fractal-palettes/` (9 files, manifest entries added). Pages:
[Palette design for fractals](techniques/palette-design.md), [Fractal palettes: research and the palette library](synthesis/fractal-palettes.md), and 12 source
pages (fractalset-viewer, ultrafractal-gradients-help, fractint-map-format,
kovesi-good-colour-maps, colorcet, crameri-scientific-colour-maps,
bids-viridis-colormaps, cmocean, turbo-colormap, cividis-paper,
quilez-cosine-palettes, ottosson-oklab); [Index](index.md) updated. Built: the
67-palette library (`almondbread-core` `palette_library`, `palette_import`)
and `almondbread palettes` contact sheets.

### ingest | Places catalogue (P1)

User request: a list of well-known places to visit. Sources ingested:
[Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](sources/beyer-mandelbrot-zoom-sequence.md) (Commons wikitext), [Mu-Ency: named features and favourite views (coordinates)](sources/muency-named-places.md),
[Wikipedia: Misiurewicz point](sources/wikipedia-misiurewicz-point.md) (its "M(23,2)" is M(24,2) with z_0 = 0), and
[Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)](sources/robotmoon-mandelbrot-set.md) (saved to `raw/web/`, manifest entries; user tip). New
`find_misiurewicz` (Newton with lower preperiods divided out; `locate --misiurewicz k,p`).
69 places (landmark 9, valley 14, spiral 7, dendrite 4, minibrot 9, julia-island 17, deep 9;
whole set to 1e-1000), each thumbnail rendered and looked at, deep ones verified against
the CPU oracle (class mismatch 0 %). Pages: [Places: a catalogue of famous views to visit](synthesis/places.md), [Reference selection: nucleus, period detection, Newton](techniques/reference-selection.md)
(Misiurewicz Newton), [Roadmap](roadmap.md) P1, [Index](index.md).

### decision | Autopilot and fly to coordinates (as built); Lagoon crash fix

User requests: an autopilot ("so that you can just navigate with the mouse") and "give it a
coordinate and it automatically zooms in there; Q and E set the speed". Built in the core
`Navigator` (`autopilot.rs`, `flyto.rs`), with a GPU `survey` pass, renderer feedback
(preview resolution that fits the budget, waiting for a deep reference) for adaptive speed,
target-reference prefetch for flights, FFI, the CLI (`fly` script events `aim`, `click`,
`scroll`, `fly_to`; `verify --history flyto`; two regress sessions) and the Swift UI (Z, Esc,
click to pause, scroll for speed, toolbar and menu items, ⌘G / ⇧⌘G, HUD hints, inspector
assist toggle, iOS button and finger steering). Fly to follows van Wijk & Nuij's optimal path
with log-space head and tail for deep zoom and lands exactly. Review flights caught: the
scripted pilot leaving the window (now clamped), a fixed turn rate losing targets at speed
(now grows with the zoom rate), fast flights braking for most of a short path (braking now
scales with speed), and a target reference that rendered wrong fans at sea-8007 (now
requested like a jump's; regress gates on escape counts). Also fixed: `fly` script events on
a refresh boundary fired twice. Crash "selected Lagoon and almondbread died": a colour change
dropped AA levels while their chunks were in flight; harvest indexed a missing level
(commit 1fef638, regression test, panics now caught at the FFI and logged to
`~/Library/Logs/Almondbread/panic.log`). Pages: [Controls: keyboard, pointer and touch](decisions/controls.md) Autopilot and Fly to,
[Smooth and efficient zooming and panning (van Wijk & Nuij)](sources/van-wijk-nuij-smooth-zooming.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md) Autopilot and fly to,
[Roadmap](roadmap.md) AP, [Index](index.md).

### decision | Interior colourings, calm dense regions, orbit overlay and mini-map, control center

User requests: orbit of c as dots and lines, interior colourings, a control center; then a
mini-map with nested levels and click-to-jump; then "grey dense areas too big". Built: GPU
interior cycle analysis at any depth (atom domain q, period = first divisor of q with closure
and |λ| < 1, multiplier, interior DE, closest approach) with bubbles / period / angle /
distance modes and CLI `--inside`; calm dense regions (per-palette deep colour instead of the
filtered palette's grey); the orbit overlay and mini-map drawn in Rust/Metal (tile-binned
vector primitives, own 5×7 font, overlay-only frames from the history), map levels rendered
by small renderers between gestures, click-to-jump with snapping to a minibrot's cusp; the
control center (searchable sections), a macOS Settings window and persistence. Found and
fixed with main: a colour change during progressive AA crashed the app (main's `aa_epoch`).
Pages: [Colouring methods](techniques/coloring-methods.md) (Interior colouring as built, Dense regions),
[Controls: keyboard, pointer and touch](decisions/controls.md) (control center, orbit overlay and mini-map),
[almondbread benchmarks and verification record](synthesis/benchmarks.md) (Interior colourings, overlays, control center).

### decision | L2: landscape detail and robustness

User verdict on L1: "ok, not wow"; zoom in more; low detail at the deepest zoom; screenshot with
flat tile-shaped slabs at S = 1.76e-6, 0.04 S above the ground. Causes: point-sampled vertex
fallback past ℓ+2 and geometry-only fragment shading (slabs); a fill-limited zoom that
throttled itself to 0.1–0.15 e-folds/s; a per-tile cost estimate stuck high. L2: parent-first
scheduling (complete pyramid), fallback through all levels, screen-space-error priority,
post-batch backlog for the zoom limit, window DE budget from the render time, parent-sample
reuse in the DE kernel (1.15–1.27× cold), world-anchored detail noise from a texture,
equipotential ridges on plains (σ-weighted; baked into DE they striped every flank), 256²
rings with footprint-based shading levels, curvature-correct geomorph. No-data pixels 0 in
every run (L1: 45 % max starved, 99.9 % max in the target dive). Depth limit not lifted (what
it takes is on the page). Pages: [Landscape mode: fly over the Mandelbrot set as 2.5D terrain](decisions/landscape-mode.md) L2, [almondbread benchmarks and verification record](synthesis/benchmarks.md) L2,
[Roadmap](roadmap.md).

## 2026-10-14

### research | R5: local series approximation on the GPU (float32 check)

Answered [Does local series approximation hold in the float32 perturbation kernel?](questions/local-sa-float32.md). Metal prototype in the render crate (behind `--sa 1` /
`ALMONDBREAD_SA=1`, offscreen only): `seedLevel` kernel per quadtree level (32 → 2 texels,
ε = 1e−2), float32 seeds in the plain kernel and floatexp seeds with BLA blocks in the deep
kernel, `iterate` with function constant `kSeeded` starting fresh pixels from their leaf.
Interleaved A/B timing at 18 locations on a shared GPU, verify against the CPU oracle, Aurora
crops. Dense-5e-14: 8.7× fewer steps, 78 → 16 ms; deep-5e-14 4.6×, root-third 3.2×, filament
1.9×, sea-1e20 8.5×; interior/unresolved-heavy views 0.75–1.07× (incoherent reference reads,
long seed runs). Error statistics equal the plain kernel's in chaotic views, slightly larger
but invisible μ errors in smooth ones, zero false interior. Found: restarting the interior
derivative at the branch (R3 design) paints 96 % of needle-1e305 interior; carry `w` and
`dw/dc` instead. Also found and fixed two DE bugs of the deep kernel (clamp before scaling,
mantissa overflow before escape). Recommendation: go for M5 with a per-view gate, leaf-sorted
admission, chunked seed passes. Pages: [Local series approximation (tile and pixel seeds)](techniques/local-series-approximation.md) (GPU
prototype, pitfalls, M5 design), [almondbread benchmarks and verification record](synthesis/benchmarks.md) R5, [Roadmap](roadmap.md), [Index](index.md).

### research | R3: dense-region sampling

Question: fewer pixel-iterations for the same perceived image in dense boundary regions
(dense-5e-14, where BLA/AT cannot skip). Literature run (raw notes
`raw/research/dense-region-sampling/literature.md`; new sources: mathr adaptive supersampling,
Mitchell 1987, Ultra Fractal AA, XaoS algorithms, Mu-Ency adjacency optimisation, Fractint
passes, joint bilateral upsampling, SVGF). CPU prototype `prototypes/dense-sampling/`
(5 × 5 lattice per pixel, CPU port of the default look, OKLab ΔE against an independent
16-sample reference, scripted pan for flicker; 10 locations; raw runs in
`raw/research/dense-region-sampling/runs/`). Results: sample-sparse schemes (colour guessing,
texture fill, sparse + reconstruction) lose fibres and small flowers in dense regions; the
pixel footprint expands only in the last ~3 % of the orbit, so there is no early stop;
variance-stopped AA saves little. Local series approximation around quadtree tile seeds and
pixel seeds (second order, ε = 1e−2) cuts iterations 5.2× (refined frame) and 7.2× (settled
AA) at the dense view with unchanged error statistics; 1.5–5.6× on other exterior views;
interior-only Mariani–Silver adds 1.3–1.6× on interior-heavy views with zero wrong pixels.
Recommendation: local SA in M5 after a float32 kernel check. Pages:
[Dense boundary regions: fewer pixel-iterations for the same image (R3)](synthesis/dense-region-sampling.md), [Adaptive and sparse sampling, reconstruction, early termination](techniques/adaptive-sampling.md),
[Local series approximation (tile and pixel seeds)](techniques/local-series-approximation.md), [Does local series approximation hold in the float32 perturbation kernel?](questions/local-sa-float32.md); updated
[Guessing, border tracing and rectangle subdivision](techniques/guessing-and-subdivision.md), [Series approximation (legacy)](techniques/series-approximation.md),
[Anti-aliasing and supersampling](techniques/anti-aliasing.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md) R3, [Index](index.md).

### decision | Auto iterations: a view where nothing escaped renders again at 4×

`render --location sea-8007` came out flat grey. Every pixel reached the depth limit
(29 575) unresolved, none escaped, and the learner only grows on escapes past the limit.
No extension pass ran: offscreen has none, and at full Retina size the list overflows and the
budget is too small. Now a view that is ≥ 25 % unresolved, with no pass past 4× its limit,
raises the limit 4× (still capped at 4× depth) and renders the *same* view again (no pin;
offscreen renders loop). sea-8007 settles at ~118 000 with the seahorse rings. Views inside
the minibrot and the cardioid settle as before. New `verify --history jump` (Go to
coordinates), `--max-unresolved`, two regress sessions, two Metal tests. Pages:
[Automatic maximum-iteration control](techniques/auto-iterations.md) "Nothing escaped", [Interior detection](techniques/interior-detection.md).

### decision | L1: lake flicker fix, R32F data, nose-flying controls with explicit zoom

The fragment now checks data validity per level itself. Invalid coarser texels had painted
ground over lakes in tile-shaped flickering blocks; the bug was there from the first build.
Data moved to R32F: f16 log2 DE made 0.002 S terraces when flying low. New controls: fly
the nose in a rigid world (W/S/↑/↓ stick, A/D/←/→ bank, Q/E throttle), explicit zoom
(Shift+stick) about the crosshair, optional auto zoom (Z: steep nose-down zooms in, height
zooms out), detail following the eye's height, speed scaled by height. A closed-loop target
dive onto a minibrot lake reaches 1e-7 within 3.6 S of the chosen point. Details in
[Landscape mode: fly over the Mandelbrot set as 2.5D terrain](decisions/landscape-mode.md) L1.

### decision | M3b: unlimited depth

Arbitrary-precision viewport centre and extended-range scale end to end (FFI as decimal
strings, paste back); big references with ball-period + Newton nucleus selection on a worker
thread, reused while precision and offset allow, history shown while one computes; the deep
kernel (rescaled float32 deltas with P/T regimes, floatexp full steps where |Z| < 2^-32,
exponent-carrying DE and interior derivatives); extended-range BLA table (1.5–26× at depth); a
deep CPU oracle validated against direct BigFloat; deep benchmark locations made with
`almondbread locate` (needle minibrots to 1e-1004, c = i dendrites); deep regress sessions and
interaction scenarios. Found: the interior test missed nucleus returns whose period is not a
multiple of 8; locate must refine nuclei to the minibrot's size, not the search radius; gesture
previews needed 1/8 resolution in dense deep views (26 → 51–56 fps). Pages:
[Extended range: floatexp, rescaled iterations, depth thresholds](techniques/extended-range-floats.md), [Bivariate linear approximation (BLA)](techniques/bla.md), [Reference selection: nucleus, period detection, Newton](techniques/reference-selection.md),
[Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md) M3b, [Roadmap](roadmap.md).

### decision | L1: world-fixed terrain colour

User feedback on layered σ: rings on hilltops collapse while diving. A same-camera
two-state diff on a hilltop mask isolated the cause: strata keyed to DE/σ shift 0.8 colour
cycles per σ transition (colour-only change 5.7/255 per frame, 21 % of pixels > 8). Strata are
now keyed to the absolute DE; layer-relative terms cross-fade between σ layers. Colour change
in transitions 0.28, 0 % of pixels > 8. What remains on hilltops is lighting from the real shape
change. Details in [Landscape mode: fly over the Mandelbrot set as 2.5D terrain](decisions/landscape-mode.md) L1.

### decision | L1: real-time landscape prototype

Built milestone L1 ([Roadmap](roadmap.md)) in `prototypes/landscape-rt/`: Metal (objc2-metal) +
winit, geometry clipmap over camera-independent DE samples per absolute level (tagged
toroidal tiles, fade-in, geomorph to the coarser ring), float32 perturbation DE on the GPU
with resumable 16×16 tiles, an eased flight model in units of S with dives towards a
coast-snapped aim point, a background 120 Hz test window and an offscreen 1080p60
recorder with frame-difference and same-camera pop checks. Found: L0's jitter came from its
camera-anchored sample grid. dz/dc overflows float32 near the set (NaN became needles):
carry an exponent. A tile with slow pixels can stall a frame by 10 ms: resume tiles across
frames. Straight-down dives lose the set: zoom towards an aim point. The "melting" the user
saw while diving is σ = 0.1 S: fixed with a σ that is constant within layers of 2 octaves and
changes in S-driven transitions inside a mist. 118–120 fps presented at 120 Hz. Results in
[Landscape mode: fly over the Mandelbrot set as 2.5D terrain](decisions/landscape-mode.md) L1 and [almondbread benchmarks and verification record](synthesis/benchmarks.md) L1.

### decision | M4: BLA built, off where it cannot pay

BLA table with per-pixel `c` validity (`|z| + K|c| < R`, framing-independent), merged
levels from 2 steps, built on demand (thread above 16 k points), kernel pipeline `kBLA` with
lookups at m ≡ 1 (mod 8) and derivative propagation (DE, interior with `n + len`). At the
user's slow boundary view (4.6e−14/px) and every other reachable view it loses on the GPU:
blocks are 2–8 steps between the reference's period-7 near-zeros, only 56 % of steps are
linear at ε = 2⁻²⁴, and a lookup costs several steps. AT does not apply there either. A
forecast from the table enables it per view (`auto`), which happens from about 1e−20/px
(2.4×; 47× at 1e−24). Default renders bit-identical to before. Pages:
[Bivariate linear approximation (BLA)](techniques/bla.md) (As built), [almondbread benchmarks and verification record](synthesis/benchmarks.md) M4, [Roadmap](roadmap.md).

### decision | FL: feel and look

Built milestone FL ([Roadmap](roadmap.md)). Motion: eased velocities on critically damped springs
(closed form, frame-rate independent), drag/touch flings that respect the macOS trackpad's
own momentum, animated zoom steps, gamepad through GameController with dead zones and
curves in Rust ([Controls: keyboard, pointer and touch](decisions/controls.md) FL). Rendering: a `compose` pass with a reprojected
linear-light history, Halton-jittered temporal upsampling during gestures, per-pixel fade of
refined pixels, focus-first 16×16 tile admission, adaptive progressive AA capped by the
supersampling setting, a gesture budget from the measured refresh, and a drawable
pre-acquisition thread replacing the pool prediction ([Frame reuse and progressive refinement](techniques/frame-reuse-and-progressive.md),
[Anti-aliasing and supersampling](techniques/anti-aliasing.md), [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md)). Look: OKLab palettes,
ten looks, DE lighting with highlight and outline by default, palette anti-aliasing from the
DE gradient, colour cycling and depth tint ([Colouring methods](techniques/coloring-methods.md)). Review: the
`almondbread fly` flight recorder renders scripted flights in real time through the same
navigator and scheduler and encodes them with ffmpeg; four review flights plus a converged
reference in `review/` (gitignored). Numbers in [almondbread benchmarks and verification record](synthesis/benchmarks.md) FL. Found: background
test windows get 60 display-link callbacks/s on the 120 Hz panel while an external display is
attached, so FL's interaction numbers are at 60 Hz.

### decision | L0: landscape prototype results

CPU prototype in `prototypes/landscape/` (own Cargo workspace): f64 iteration with
`dz/dc` and `dz/dz1`, lazily memoised LOD tiles, height-field ray marching, soft shadows,
AO, water, fog. Gallery of 9 JPEGs (height variants at two places, far field,
self-similarity at 1e-3…1e-12 and at four places, an 8-frame dive, three hero shots).
Recommendation: `h = σ·ln(1 + k·DE/σ)`, σ = 0.1 S, k = 4. It is fixed-world near
the camera, and only the far field depends on the camera scale. Linear gives a
horizon wall or a flat plain, and the potential is unusable. Timings: 20–50 s per
1920×1080 2×2-SS still, 110–160 s near deep minibrots. GPU estimate: ~140 k DE
evaluations per frame while flying and sinking. Results and open problems in
[Landscape mode: fly over the Mandelbrot set as 2.5D terrain](decisions/landscape-mode.md); [Roadmap](roadmap.md) L0 done.

### decision | M2.5: pointer and keyboard modes, crosshair zoom target

Implemented the controls revision ([Controls: keyboard, pointer and touch](decisions/controls.md), As built). The Rust core's `Navigator` owns
the input mode (pointer mode by default; W A S D / arrows switch to keyboard mode; more than 6 pt of
pointer travel, a double-click, a drag or a touch pinch switch back), the zoom target for every zoom
input, the crosshair glide (critically damped spring in closed form, ω = 25 /s, 1 % after 265 ms, no
overshoot), edge panning (8 % band, smoothstep ramp to the key speed, setting off / while zooming /
always) and the canvas from safe-area insets. Swift presents: crosshair cursor rects (macOS),
hidden-until-moved cursor and a Core Animation crosshair overlay in keyboard mode, iPad hover and
`UIPointerInteraction`. Fixed on the way: motion from rest jumped by up to 0.1 s of motion on its first
frame; idle `frame` calls re-presented the unchanged image. Measured at 120 Hz against the Swift
renderer and Rust HEAD ([almondbread benchmarks and verification record](synthesis/benchmarks.md) M2.5): zoom-target drift 0 px, edge pan at the
expected 0.381 view widths/s, no per-frame cost after replacing a UniFFI record result with bit flags
(11–18 µs → 2–3 µs); R2b's late final frame does not reproduce at 120 Hz; open: fewer gesture frames
than Swift on heavy zooms at 120 Hz. Updated [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md),
[Rust core to Swift: UniFFI and the XCFramework pipeline](platform/rust-swift-ffi.md), [Roadmap](roadmap.md).

### research | Fractal games and Steam

Deep-research run on a game edition and a Steam release. The report, its raw notes, the
game-mechanics decision and the Steam source pages are kept private, outside this
repository. Technical outcome: a portable edition would use no engine (wgpu + winit) and
keep Metal on Apple. Public source pages: [wgpu CHANGELOG (v27–v30) and Features docs](sources/wgpu-changelog-v30.md),
[Bevy 0.20 release notes](sources/bevy-0-20-release-notes.md).

### decision | R2b: apps on the Rust renderer, Swift UI only

Implemented R2b of [Rust core with a Rust Metal backend; Swift for UI only](decisions/rust-core.md): `Explorer` UniFFI object over a new
platform-neutral `Host<B>` (layer address once, one `frame` call per display refresh,
`input(InputEvent)`, settings record, 10 Hz stats, offscreen PNG, measurements); Swift
reduced to SwiftUI views, an `ExplorerModel` settings mirror and the platform views
(CAMetalLayer, display link, event decoding). Deleted the Swift renderer, kernel copy,
AlmondbreadMath, math-check, Swift tests and Swift CLI commands. Interaction parity measured
in three interleaved visible rounds against the Swift renderer built from de0aa8e: fps,
interval and latency equal or better, the 9.4 ms drawable stall gone (0.8–1.0 ms). The
R2a pool predictor was corrected twice (presented callbacks lag a refresh; gate only
gesture frames). Final frames 10–25 ms later in some scenarios for the same GPU work. The
R2a seahorse skew does not appear in the app. FFI 2–7 µs per frame. Pitfalls filed:
occluded windows give no presentation times, SwiftUI shows the window after the view
orders it back, a bare path argument suppresses the SwiftUI window. Pages:
[Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md), [Rust core to Swift: UniFFI and the XCFramework pipeline](platform/rust-swift-ffi.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md) (R2b),
[Rust core with a Rust Metal backend; Swift for UI only](decisions/rust-core.md) (R2b outcome), [Roadmap](roadmap.md), [Overview](overview.md); Swift-path notes on
[Arbitrary precision: own Swift/C implementation, not GMP/MPFR](decisions/arbitrary-precision.md), [Reference orbit: precision, cost, storage, compression](techniques/reference-orbit-precision.md),
[Reference selection: nucleus, period detection, Newton](techniques/reference-selection.md), [Arbitrary precision on Apple Silicon (GMP, MPFR)](platform/mpfr-gmp-on-apple-silicon.md),
[Metal compute pipeline for a deep-zoom renderer](platform/metal-compute.md).

### research | Fractal app market research

Deep-research run on the fractal-app market (competitors, audience, business models). The
report, its raw notes and the business source pages are kept private, outside this
repository. Public source pages from the run: [Frax help: Explore (precision limit)](sources/frax-help-explore.md),
[Maths Town videos and video descriptions](sources/maths-town-videos.md), [KF Movie Maker user guide (Maths Town)](sources/maths-town-kfmoviemaker-guide.md),
[How mandelbrot.site was built](sources/mandelbrot-site.md), [Hacker News: Mandelbrot Set Explorer thread (2025)](sources/hn-mandelbrot-site-thread.md).

### decision | R2a: renderer in Rust, headless parity with Swift

Implemented R2a of [Rust core with a Rust Metal backend; Swift for UI only](decisions/rust-core.md): `almondbread-core` (viewport with an
M3b-ready `Center`, render settings, palettes, f64 reference, auto-iteration learning,
rayon CPU oracle, verify metrics, benchmark locations, PNG, `Navigator` input model),
`almondbread-render` (`GpuBackend` trait, scheduler, MSL kernels, Metal backend on
objc2-metal, `CAMetalLayer` surface; kernel struct layouts asserted at compile time and
probed through the Metal compiler), `almondbread-cli` (`almondbread render|verify|bench|regress`,
Swift-identical output; `make rust-bench`, `make rust-regress`). Parity, measured on M1 Pro
in interleaved runs: iteration fields, PNGs and verify tables bit-identical to the Swift
renderer at all 11 locations and with extension passes; GPU ms geomean 0.998; regress passes.
Investigated the 9.4 ms draw CPU in the Swift "neck zoom" scenario: 8.9 ms is blocking in
`currentDrawable` (a presentation queue one frame deeper than the drawable pool), not
texture allocation or GPU load; the Rust surface predicts the pool and skips a refresh
instead. Preview textures are now allocated once at maximum size (sub-regions). Pages: new
[Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md), [almondbread benchmarks and verification record](synthesis/benchmarks.md) (R2a), [Roadmap](roadmap.md) (R2a done).

### decision | Wedge bug: derivative interior test false positives near parabolic points

User report: a black polygon with straight edges and a sharp concave corner cutting
through smooth exterior at zoom 2.2e5 near c = −3/4 − 0.0021i (auto limit 2 053). It
was reproduced headlessly at that view: 20.9 % of the pixels were GPU-interior and
CPU-exterior, the same wedge. With the test off it was 0.001 %. The cause is not the
pipeline (a new headless driver of the real scheduler, `--verify --history`, found
final frames bit-identical to fresh renders across ~700 zoom/pan sessions). It is
Chéritat's fixed `|dz/dz₁| < 1e−3`: exterior orbits passing slowly through a
parabolic gate dip to about `C/n²` mid-passage. The max-norm test turns the nearly
linear `w(c)` into rotated squares. Fix: `|w| n² < 1e−3`. Also: unresolved ≠ interior
(own colour), background extension passes over the collected unresolved pixels, and
a statistics-learned auto limit with hysteresis. Regressions in `--bench` (fails
above 0.02 % false interior), `make regress` and `--interaction-test` (shown frame
vs fresh render). Cost: elephant, minibrot-p4 and feigenbaum are 1.5 to 1.8× slower
(near-parabolic interior is slower to prove). Pages: [Interior detection](techniques/interior-detection.md)
(Pitfalls), [Automatic maximum-iteration control](techniques/auto-iterations.md), [GPU watchdog and chunked, resumable kernels](platform/gpu-watchdog-and-chunking.md)
(Pitfalls), [almondbread benchmarks and verification record](synthesis/benchmarks.md).

### decision | R1: Rust workspace, arithmetic port, UniFFI XCFramework

Implemented R1 of [Rust core with a Rust Metal backend; Swift for UI only](decisions/rust-core.md): Cargo workspace (`crates/`: math, core,
render skeleton, ffi, uniffi-bindgen), the AlmondbreadMath port with all oracle tests
bit-exact plus rational property tests, `scripts/build-rust-xcframework.sh` (`make
rust`), `AlmondbreadCoreFFI` binary target and `AlmondbreadCore` Swift module linked by
the swift-run, macOS and iOS-simulator builds, and `almondbread-core-check` (bit-for-bit
parity with Swift through the bindings). Measured (M1 Pro, release): reference orbit
1.05x to 4x the Swift build; kernel findings (const-generic storage, unrolled
`carrying_add` chains, fused shift in addition, two-column Comba) on
[Arbitrary precision on Apple Silicon (GMP, MPFR)](platform/mpfr-gmp-on-apple-silicon.md); FFI call costs and rules on the new
[Rust core to Swift: UniFFI and the XCFramework pipeline](platform/rust-swift-ffi.md) with source [The UniFFI user guide (Mozilla)](sources/uniffi-user-guide.md); table in
[almondbread benchmarks and verification record](synthesis/benchmarks.md); R1 marked done in [Roadmap](roadmap.md).

### decision | Milestone 3a: own arbitrary precision, reference orbits, nuclei

Built `Sources/AlmondbreadMath` per [Arbitrary precision: own Swift/C implementation, not GMP/MPFR](decisions/arbitrary-precision.md): correctly rounded
BigFloat/BigComplex, `ReferenceOrbitComputer` (float32 + shared-exponent floatexp output,
cancellation, progress), ball/first-order/box period detection, Newton nuclei, minibrot
size estimate, `ReferenceSelector`. Tests bit-exact against an exact Python oracle and
within 1 bit of mpmath for nuclei down to 6e-300. Measured (M1 Pro, release): 1.08x to
1.55x GMP mpf orbit speed up to 1024 bits, 0.56x at 16384 bits; short products with exact
rounding, two-accumulator Comba and unrolled 3...8-limb kernels were what mattered,
Karatsuba lost to short Comba up to 256 limbs ([Arbitrary precision on Apple Silicon (GMP, MPFR)](platform/mpfr-gmp-on-apple-silicon.md),
[almondbread benchmarks and verification record](synthesis/benchmarks.md)). Findings on the ball recurrence and precision rule added to
[What exactly is the ball-method period recurrence?](questions/ball-period-recurrence.md) and [How many bits does the reference orbit need?](questions/precision-bits-formula.md) (both still
open); Newton/period pitfalls on [Reference selection: nucleus, period detection, Newton](techniques/reference-selection.md).

### decision | Milestone 2: chunked iteration, interior detection, verification harness

Built roadmap step 2, step 3 and the interior part of step 4, plus interaction fixes.
New [almondbread benchmarks and verification record](synthesis/benchmarks.md) holds the benchmark set (7 locations), M1 vs M2 timings,
GPU-vs-CPU verify metrics and interaction measurements (measured 2026-10-14, M1 Pro,
release). Findings filed: compaction must preserve pixel order (per-SIMD-group atomic
appends cost 31 % throughput through incoherent `ref[m]` reads), float atomics are very
slow, GPU-side pixel admission, K from a throughput model gives 16 ms chunks (p99 17.6 ms
at 1e7 iterations) ([GPU watchdog and chunked, resumable kernels](platform/gpu-watchdog-and-chunking.md)); Xcode precompiles `.metal`
resources, compositor latency ~28 ms, SwiftUI HUD invalidation throttles MTKView
([Metal compute pipeline for a deep-zoom renderer](platform/metal-compute.md)); derivative interior test bit-identical to maxIter at
ε = 1e−3, ~1.5× per-iteration cost with an every-8th-iteration check, adaptive switch,
15–46× on interior-heavy views ([Interior detection](techniques/interior-detection.md)); gesture frames with
dynamic-resolution bilinear previews and refine-after-idle
([Frame reuse and progressive refinement](techniques/frame-reuse-and-progressive.md)). A 10.4 s single command buffer completed
without a watchdog kill ([What is the exact Metal GPU watchdog limit on macOS and iOS?](questions/metal-watchdog-limit.md), still open). The float32
question got a partial answer: boundary disagreement at shallow depth is chaotic
sensitivity, real float32 error appears near 5e−14 per pixel
([How large is float32 perturbation error over long orbits?](questions/float32-error-long-orbits.md), still open). New [Controls: keyboard, pointer and touch](decisions/controls.md).

### decision | Milestone 1 built and measured

App milestone 1 done: float32 perturbation with rebasing against a Double
reference orbit, split (n, fraction) smooth count, pixel-scaled
derivative distance estimate, normal-map lighting and outline, SwiftUI
inspector and HUD, macOS + iOS targets via XcodeGen. Measured on M1 Pro,
release, iterate pass only, 3456×2234: home 9.2 ms (342 iterations),
seahorse 1.9e5× 43.5 ms (2015), 7.5e10× 185 ms (5763); colorize about
1 ms; reference orbit < 0.5 ms. GPU vs CPU Double diff at 400×300: median
colour error 0.001; 0.2–2 % of pixels differ by > 0.05, all isolated
boundary pixels (no glitch clusters). Fast math measured about 10 % faster
than `.safe` with identical results for this kernel. See
[Arbitrary precision: own Swift/C implementation, not GMP/MPFR](decisions/arbitrary-precision.md) for the next step.

### ingest | Deep zoom research run: sources and topic pages

Saved 36 primary sources (37 files) to `raw/` with `curl` (one at a time, 2 s pause):
mathr's 2021 and 2022 deep-zoom posts and deep-zoom page, KF2+ manual and
page, Fraktaler 3 site, zoomasm, mathr's interior-distance post, Wikipedia
(plotting algorithms, XaoS), Mu-Ency (BLA, Newton-Raphson, atom domain,
floatexp), Chéritat's Mandelbrot page, Quilez on smooth iteration, Phil
Thompson's BLA post, Ultra Fractal perturbation help (2 pages), two GMP list
posts, luma.gl's GPU precision guide, the MLX watchdog issue, READMEs of
FractalShark, Imagina, metal-benchmarks, mandelbrot-webgpu, swift-mandelbrot,
XaoS, Very Plotter, fractalshades and Fractal Zoomer, Imagina's
`HInfLAEvaluator.h/.cpp` (AGPL, read for facts only), Thall's df64 paper
(PDF), and two fractalforums.org topics via web.archive.org (4360:
Zhuoran's rebasing post of 3 Aug 2021; 4802: GBy's interior detection).
HTML and PDF got `.txt` conversions (textutil, pdftotext). 68 manifest
entries (fetch + derived + the 5 research-note files).

Fetch failures: fractalforums.org itself not tried (known 429/redirect
loops; archive copies used instead); SourceForge Fractal Zoomer page 403
(README used); PyPI fractalshades returned a bot-check page (deleted;
README used); Apple developer pages are JS-only and were not saved
(source pages with `raw: "web"`).

Created: source pages for every saved document plus seven web-only ones;
19 technique pages, 7 formula pages, 13 implementation pages, 5 platform
pages, 14 question pages. Filled the synthesis `sources` list and turned
its URL citations into `[[sources/...]]` links. Rewrote [Overview](overview.md) and
[Index](index.md).

Findings that changed the notes: the archived forum topic dates rebasing to
3 Aug 2021 (notes had a conflict between Aug and Dec 2021); topic 4802 is
the interior-detection thread, not the BLA thread; Fractal Zoomer's author
reports rebasing working for Nova, magnet and Newton (contradicts "no
scheme found", see [Can perturbation and rebasing handle Nova, Newton and Magnet formulas?](questions/perturbation-convergent-formulas.md)); mathr
measured software floats (uint32 + int) about 2× faster than float32 + int32
floatexp on CPU. Contradictions raised as questions: KF BLA support,
higher-order BLA, FFMA32 latency, integer vs float-float emulation, Ultra
Fractal SA use. The deep-zoom page calls an earlier BLA validity check
"nonsense", most likely the 2022 radius formula the synthesis quotes; noted
under `## Contradictions` on [Bivariate linear approximation (BLA)](techniques/bla.md) (synthesis body left as
written). Lint, broken links and orphans clean after the pass.

### research | Deep zoom Mandelbrot rendering techniques

Deep-research run on the state of the art in fast deep-zoom rendering for a
float32 Metal GPU. Five sub-reports saved unchanged under
`raw/research/deep-zoom-mandelbrot-rendering-techniques/`
(perturbation_core, iteration_skipping, pixel_and_image_techniques,
gpu_apple_silicon, implementations_and_formulas). Final report filed as
[Deep zoom Mandelbrot rendering techniques](synthesis/deep-zoom-rendering-techniques.md): single reference with
rebasing, BLA then Imagina-style LA/AT, rescaled float32 past ~1e-25,
chunked kernels under the watchdog, and a ranked 12-step roadmap.

### setup | Wiki created

Skeleton created on the pattern of the calculator knowledge base: root
`.hyalo.toml` (dir `kb/wiki`, schemas per page type), `kb/CLAUDE.md`,
`kb/raw/`, `kb/sources/manifest.json`, `kb/scripts/fetch-sources`, and
`index.md`, `log.md`, `overview.md`. A deep-research run on deep-zoom
rendering techniques is in progress; its notes go to
`raw/research/deep-zoom-mandelbrot-rendering-techniques/`.
