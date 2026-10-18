---
title: Log
type: meta
---

# Log

Append-only. One entry per operation, newest last.

## [2026-10-14] setup | Wiki created

Skeleton created on the pattern of the calculator knowledge base: root
`.hyalo.toml` (dir `kb/wiki`, schemas per page type), `kb/CLAUDE.md`,
`kb/raw/`, `kb/sources/manifest.json`, `kb/scripts/fetch-sources`, and
`index.md`, `log.md`, `overview.md`. A deep-research run on deep-zoom
rendering techniques is in progress; its notes go to
`raw/research/deep-zoom-mandelbrot-rendering-techniques/`.

## [2026-10-14] research | Deep zoom Mandelbrot rendering techniques

Deep-research run on the state of the art in fast deep-zoom rendering for a
float32 Metal GPU. Five sub-reports saved unchanged under
`raw/research/deep-zoom-mandelbrot-rendering-techniques/`
(perturbation_core, iteration_skipping, pixel_and_image_techniques,
gpu_apple_silicon, implementations_and_formulas). Final report filed as
[[synthesis/deep-zoom-rendering-techniques]]: single reference with
rebasing, BLA then Imagina-style LA/AT, rescaled float32 past ~1e-25,
chunked kernels under the watchdog, and a ranked 12-step roadmap.

## [2026-10-14] ingest | Deep zoom research run: sources and topic pages

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
its URL citations into `[[sources/...]]` links. Rewrote [[overview]] and
[[index]].

Findings that changed the notes: the archived forum topic dates rebasing to
3 Aug 2021 (notes had a conflict between Aug and Dec 2021); topic 4802 is
the interior-detection thread, not the BLA thread; Fractal Zoomer's author
reports rebasing working for Nova, magnet and Newton (contradicts "no
scheme found", see [[questions/perturbation-convergent-formulas]]); mathr
measured software floats (uint32 + int) about 2× faster than float32 + int32
floatexp on CPU. Contradictions raised as questions: KF BLA support,
higher-order BLA, FFMA32 latency, integer vs float-float emulation, Ultra
Fractal SA use. The deep-zoom page calls an earlier BLA validity check
"nonsense", most likely the 2022 radius formula the synthesis quotes; noted
under `## Contradictions` on [[techniques/bla]] (synthesis body left as
written). Lint, broken links and orphans clean after the pass.

## [2026-10-14] decision | Milestone 1 built and measured

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
[[decisions/arbitrary-precision]] for the next step.

## [2026-10-14] decision | Milestone 2: chunked iteration, interior detection, verification harness

Built roadmap step 2, step 3 and the interior part of step 4, plus interaction fixes.
New [[synthesis/benchmarks]] holds the benchmark set (7 locations), M1 vs M2 timings,
GPU-vs-CPU verify metrics and interaction measurements (measured 2026-10-14, M1 Pro,
release). Findings filed: compaction must preserve pixel order (per-SIMD-group atomic
appends cost 31 % throughput through incoherent `ref[m]` reads), float atomics are very
slow, GPU-side pixel admission, K from a throughput model gives 16 ms chunks (p99 17.6 ms
at 1e7 iterations) ([[platform/gpu-watchdog-and-chunking]]); Xcode precompiles `.metal`
resources, compositor latency ~28 ms, SwiftUI HUD invalidation throttles MTKView
([[platform/metal-compute]]); derivative interior test bit-identical to maxIter at
ε = 1e−3, ~1.5× per-iteration cost with an every-8th-iteration check, adaptive switch,
15–46× on interior-heavy views ([[techniques/interior-detection]]); gesture frames with
dynamic-resolution bilinear previews and refine-after-idle
([[techniques/frame-reuse-and-progressive]]). A 10.4 s single command buffer completed
without a watchdog kill ([[questions/metal-watchdog-limit]], still open). The float32
question got a partial answer: boundary disagreement at shallow depth is chaotic
sensitivity, real float32 error appears near 5e−14 per pixel
([[questions/float32-error-long-orbits]], still open). New [[decisions/controls]].

## [2026-10-14] decision | Milestone 3a: own arbitrary precision, reference orbits, nuclei

Built `Sources/AlmondbreadMath` per [[decisions/arbitrary-precision]]: correctly rounded
BigFloat/BigComplex, `ReferenceOrbitComputer` (float32 + shared-exponent floatexp output,
cancellation, progress), ball/first-order/box period detection, Newton nuclei, minibrot
size estimate, `ReferenceSelector`. Tests bit-exact against an exact Python oracle and
within 1 bit of mpmath for nuclei down to 6e-300. Measured (M1 Pro, release): 1.08x to
1.55x GMP mpf orbit speed up to 1024 bits, 0.56x at 16384 bits; short products with exact
rounding, two-accumulator Comba and unrolled 3...8-limb kernels were what mattered,
Karatsuba lost to short Comba up to 256 limbs ([[platform/mpfr-gmp-on-apple-silicon]],
[[synthesis/benchmarks]]). Findings on the ball recurrence and precision rule added to
[[questions/ball-period-recurrence]] and [[questions/precision-bits-formula]] (both still
open); Newton/period pitfalls on [[techniques/reference-selection]].

## [2026-10-14] decision | R1: Rust workspace, arithmetic port, UniFFI XCFramework

Implemented R1 of [[decisions/rust-core]]: Cargo workspace (`crates/`: math, core,
render skeleton, ffi, uniffi-bindgen), the AlmondbreadMath port with all oracle tests
bit-exact plus rational property tests, `scripts/build-rust-xcframework.sh` (`make
rust`), `AlmondbreadCoreFFI` binary target and `AlmondbreadCore` Swift module linked by
the swift-run, macOS and iOS-simulator builds, and `almondbread-core-check` (bit-for-bit
parity with Swift through the bindings). Measured (M1 Pro, release): reference orbit
1.05x to 4x the Swift build; kernel findings (const-generic storage, unrolled
`carrying_add` chains, fused shift in addition, two-column Comba) on
[[platform/mpfr-gmp-on-apple-silicon]]; FFI call costs and rules on the new
[[platform/rust-swift-ffi]] with source [[sources/uniffi-user-guide]]; table in
[[synthesis/benchmarks]]; R1 marked done in [[roadmap]].

## [2026-10-14] decision | Wedge bug: derivative interior test false positives near parabolic points

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
(near-parabolic interior is slower to prove). Pages: [[techniques/interior-detection]]
(Pitfalls), [[techniques/auto-iterations]], [[platform/gpu-watchdog-and-chunking]]
(Pitfalls), [[synthesis/benchmarks]].

## [2026-10-14] decision | R2a: renderer in Rust, headless parity with Swift

Implemented R2a of [[decisions/rust-core]]: `almondbread-core` (viewport with an
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
[[platform/render-architecture]], [[synthesis/benchmarks]] (R2a), [[roadmap]] (R2a done).

## [2026-10-14] research | Fractal app market research

Deep-research run on the fractal-app market (competitors, audience, business models). The
report, its raw notes and the business source pages are kept private, outside this
repository. Public source pages from the run: [[sources/frax-help-explore]],
[[sources/maths-town-videos]], [[sources/maths-town-kfmoviemaker-guide]],
[[sources/mandelbrot-site]], [[sources/hn-mandelbrot-site-thread]].

## [2026-10-14] decision | R2b: apps on the Rust renderer, Swift UI only

Implemented R2b of [[decisions/rust-core]]: `Explorer` UniFFI object over a new
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
[[platform/render-architecture]], [[platform/rust-swift-ffi]], [[synthesis/benchmarks]] (R2b),
[[decisions/rust-core]] (R2b outcome), [[roadmap]], [[overview]]; Swift-path notes on
[[decisions/arbitrary-precision]], [[techniques/reference-orbit-precision]],
[[techniques/reference-selection]], [[platform/mpfr-gmp-on-apple-silicon]],
[[platform/metal-compute]].

## [2026-10-14] research | Fractal games and Steam

Deep-research run on a game edition and a Steam release. The report, its raw notes, the
game-mechanics decision and the Steam source pages are kept private, outside this
repository. Technical outcome: a portable edition would use no engine (wgpu + winit) and
keep Metal on Apple. Public source pages: [[sources/wgpu-changelog-v30]],
[[sources/bevy-0-20-release-notes]].

## [2026-10-14] decision | M2.5: pointer and keyboard modes, crosshair zoom target

Implemented the controls revision ([[decisions/controls]], As built). The Rust core's `Navigator` owns
the input mode (pointer mode by default; W A S D / arrows switch to keyboard mode; more than 6 pt of
pointer travel, a double-click, a drag or a touch pinch switch back), the zoom target for every zoom
input, the crosshair glide (critically damped spring in closed form, ω = 25 /s, 1 % after 265 ms, no
overshoot), edge panning (8 % band, smoothstep ramp to the key speed, setting off / while zooming /
always) and the canvas from safe-area insets. Swift presents: crosshair cursor rects (macOS),
hidden-until-moved cursor and a Core Animation crosshair overlay in keyboard mode, iPad hover and
`UIPointerInteraction`. Fixed on the way: motion from rest jumped by up to 0.1 s of motion on its first
frame; idle `frame` calls re-presented the unchanged image. Measured at 120 Hz against the Swift
renderer and Rust HEAD ([[synthesis/benchmarks]] M2.5): zoom-target drift 0 px, edge pan at the
expected 0.381 view widths/s, no per-frame cost after replacing a UniFFI record result with bit flags
(11–18 µs → 2–3 µs); R2b's late final frame does not reproduce at 120 Hz; open: fewer gesture frames
than Swift on heavy zooms at 120 Hz. Updated [[platform/render-architecture]],
[[platform/rust-swift-ffi]], [[roadmap]].

## [2026-10-14] decision | L0: landscape prototype results

CPU prototype in `prototypes/landscape/` (own Cargo workspace): f64 iteration with
`dz/dc` and `dz/dz1`, lazily memoised LOD tiles, height-field ray marching, soft shadows,
AO, water, fog. Gallery of 9 JPEGs (height variants at two places, far field,
self-similarity at 1e-3…1e-12 and at four places, an 8-frame dive, three hero shots).
Recommendation: `h = σ·ln(1 + k·DE/σ)`, σ = 0.1 S, k = 4. It is fixed-world near
the camera, and only the far field depends on the camera scale. Linear gives a
horizon wall or a flat plain, and the potential is unusable. Timings: 20–50 s per
1920×1080 2×2-SS still, 110–160 s near deep minibrots. GPU estimate: ~140 k DE
evaluations per frame while flying and sinking. Results and open problems in
[[decisions/landscape-mode]]; [[roadmap]] L0 done.

## [2026-10-14] decision | FL: feel and look

Built milestone FL ([[roadmap]]). Motion: eased velocities on critically damped springs
(closed form, frame-rate independent), drag/touch flings that respect the macOS trackpad's
own momentum, animated zoom steps, gamepad through GameController with dead zones and
curves in Rust ([[decisions/controls]] FL). Rendering: a `compose` pass with a reprojected
linear-light history, Halton-jittered temporal upsampling during gestures, per-pixel fade of
refined pixels, focus-first 16×16 tile admission, adaptive progressive AA capped by the
supersampling setting, a gesture budget from the measured refresh, and a drawable
pre-acquisition thread replacing the pool prediction ([[techniques/frame-reuse-and-progressive]],
[[techniques/anti-aliasing]], [[platform/render-architecture]]). Look: OKLab palettes,
ten looks, DE lighting with highlight and outline by default, palette anti-aliasing from the
DE gradient, colour cycling and depth tint ([[techniques/coloring-methods]]). Review: the
`almondbread fly` flight recorder renders scripted flights in real time through the same
navigator and scheduler and encodes them with ffmpeg; four review flights plus a converged
reference in `review/` (gitignored). Numbers in [[synthesis/benchmarks]] FL. Found: background
test windows get 60 display-link callbacks/s on the 120 Hz panel while an external display is
attached, so FL's interaction numbers are at 60 Hz.

## [2026-10-14] decision | M4: BLA built, off where it cannot pay

BLA table with per-pixel `c` validity (`|z| + K|c| < R`, framing-independent), merged
levels from 2 steps, built on demand (thread above 16 k points), kernel pipeline `kBLA` with
lookups at m ≡ 1 (mod 8) and derivative propagation (DE, interior with `n + len`). At the
user's slow boundary view (4.6e−14/px) and every other reachable view it loses on the GPU:
blocks are 2–8 steps between the reference's period-7 near-zeros, only 56 % of steps are
linear at ε = 2⁻²⁴, and a lookup costs several steps. AT does not apply there either. A
forecast from the table enables it per view (`auto`), which happens from about 1e−20/px
(2.4×; 47× at 1e−24). Default renders bit-identical to before. Pages:
[[techniques/bla]] (As built), [[synthesis/benchmarks]] M4, [[roadmap]].

## [2026-10-14] decision | L1: real-time landscape prototype

Built milestone L1 ([[roadmap]]) in `prototypes/landscape-rt/`: Metal (objc2-metal) +
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
[[decisions/landscape-mode]] L1 and [[synthesis/benchmarks]] L1.

## [2026-10-14] decision | L1: world-fixed terrain colour

User feedback on layered σ: rings on hilltops collapse while diving. A same-camera
two-state diff on a hilltop mask isolated the cause: strata keyed to DE/σ shift 0.8 colour
cycles per σ transition (colour-only change 5.7/255 per frame, 21 % of pixels > 8). Strata are
now keyed to the absolute DE; layer-relative terms cross-fade between σ layers. Colour change
in transitions 0.28, 0 % of pixels > 8. What remains on hilltops is lighting from the real shape
change. Details in [[decisions/landscape-mode]] L1.

## [2026-10-14] decision | M3b: unlimited depth

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
[[techniques/extended-range-floats]], [[techniques/bla]], [[techniques/reference-selection]],
[[platform/render-architecture]], [[synthesis/benchmarks]] M3b, [[roadmap]].

## [2026-10-14] decision | L1: lake flicker fix, R32F data, nose-flying controls with explicit zoom

The fragment now checks data validity per level itself. Invalid coarser texels had painted
ground over lakes in tile-shaped flickering blocks; the bug was there from the first build.
Data moved to R32F: f16 log2 DE made 0.002 S terraces when flying low. New controls: fly
the nose in a rigid world (W/S/↑/↓ stick, A/D/←/→ bank, Q/E throttle), explicit zoom
(Shift+stick) about the crosshair, optional auto zoom (Z: steep nose-down zooms in, height
zooms out), detail following the eye's height, speed scaled by height. A closed-loop target
dive onto a minibrot lake reaches 1e-7 within 3.6 S of the chosen point. Details in
[[decisions/landscape-mode]] L1.

## [2026-10-14] decision | Auto iterations: a view where nothing escaped renders again at 4×

`render --location sea-8007` came out flat grey. Every pixel reached the depth limit
(29 575) unresolved, none escaped, and the learner only grows on escapes past the limit.
No extension pass ran: offscreen has none, and at full Retina size the list overflows and the
budget is too small. Now a view that is ≥ 25 % unresolved, with no pass past 4× its limit,
raises the limit 4× (still capped at 4× depth) and renders the *same* view again (no pin;
offscreen renders loop). sea-8007 settles at ~118 000 with the seahorse rings. Views inside
the minibrot and the cardioid settle as before. New `verify --history jump` (Go to
coordinates), `--max-unresolved`, two regress sessions, two Metal tests. Pages:
[[techniques/auto-iterations]] "Nothing escaped", [[techniques/interior-detection]].

## [2026-10-14] research | R3: dense-region sampling

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
[[synthesis/dense-region-sampling]], [[techniques/adaptive-sampling]],
[[techniques/local-series-approximation]], [[questions/local-sa-float32]]; updated
[[techniques/guessing-and-subdivision]], [[techniques/series-approximation]],
[[techniques/anti-aliasing]], [[synthesis/benchmarks]] R3, [[index]].

## [2026-10-15] decision | L2: landscape detail and robustness

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
it takes is on the page). Pages: [[decisions/landscape-mode]] L2, [[synthesis/benchmarks]] L2,
[[roadmap]].

## [2026-10-15] decision | Interior colourings, calm dense regions, orbit overlay and mini-map, control center

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
Pages: [[techniques/coloring-methods]] (Interior colouring as built, Dense regions),
[[decisions/controls]] (control center, orbit overlay and mini-map),
[[synthesis/benchmarks]] (Interior colourings, overlays, control center).

## [2026-10-15] decision | Autopilot and fly to coordinates (as built); Lagoon crash fix

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
`~/Library/Logs/Almondbread/panic.log`). Pages: [[decisions/controls]] Autopilot and Fly to,
[[sources/van-wijk-nuij-smooth-zooming]], [[synthesis/benchmarks]] Autopilot and fly to,
[[roadmap]] AP, [[index]].

## [2026-10-15] ingest | Places catalogue (P1)

User request: a list of well-known places to visit. Sources ingested:
[[sources/beyer-mandelbrot-zoom-sequence]] (Commons wikitext), [[sources/muency-named-places]],
[[sources/wikipedia-misiurewicz-point]] (its "M(23,2)" is M(24,2) with z_0 = 0), and
[[sources/robotmoon-mandelbrot-set]] (saved to `raw/web/`, manifest entries; user tip). New
`find_misiurewicz` (Newton with lower preperiods divided out; `locate --misiurewicz k,p`).
69 places (landmark 9, valley 14, spiral 7, dendrite 4, minibrot 9, julia-island 17, deep 9;
whole set to 1e-1000), each thumbnail rendered and looked at, deep ones verified against
the CPU oracle (class mismatch 0 %). Pages: [[synthesis/places]], [[techniques/reference-selection]]
(Misiurewicz Newton), [[roadmap]] P1, [[index]].

## [2026-10-15] research | Fractal palettes and the palette library

Research run on fractal palette design (fractalset.com viewer, Ultra Fractal
gradients and `.ugr`, Fractint `.map`, KF palettes, cosine palettes, open
perceptual maps and their licences, OKLab interpolation, cyclic maps, density
mapping, dense regions, CVD safety). Notes saved unchanged under
`raw/research/fractal-palettes/` (9 files, manifest entries added). Pages:
[[techniques/palette-design]], [[synthesis/fractal-palettes]], and 12 source
pages (fractalset-viewer, ultrafractal-gradients-help, fractint-map-format,
kovesi-good-colour-maps, colorcet, crameri-scientific-colour-maps,
bids-viridis-colormaps, cmocean, turbo-colormap, cividis-paper,
quilez-cosine-palettes, ottosson-oklab); [[index]] updated. Built: the
67-palette library (`almondbread-core` `palette_library`, `palette_import`)
and `almondbread palettes` contact sheets.

## [2026-10-15] decision | R6: faster deep references

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
157 limbs. Pages: [[techniques/parallel-reference-orbit]] (new), [[synthesis/benchmarks]] R6,
[[decisions/arbitrary-precision]], [[techniques/reference-selection]],
[[techniques/reference-orbit-precision]], [[platform/mpfr-gmp-on-apple-silicon]] (corrected
the unmeasured MT3 handoff guess), [[index]].

## [2026-10-15] decision | Integration: autopilot, places, palette library and R6 into the control center

Merged main (autopilot, fly-to, deep DE fix, L2), the places catalogue, the palette library
and R6 (faster deep reference orbits) into the control-center branch. Control center:
autopilot and fly-to buttons, assist toggle and the autopilot keys in View & navigation;
Places as a thumbnail grid by category with Go, Fly (`Explorer.flyToPlace`) and Random;
the palette picker lists the library by category, applies a palette's density, offset and
deep colour, and imports .ugr / .map / JSON palettes into "My palettes" (JSON kept in user
defaults, registered again at launch). A library palette's deep colour gives the calm dense
regions their hue (at the calm luminance). Option-click on a mini-map flies there. Place
thumbnails re-rendered with calm dense regions and the bubbles interior
([[synthesis/places]]); Go / Fly switch a flat interior to bubbles to match. Checks after
the merge: the R6 orbit digest is identical to the R6 branch's; `verify --history jump` at
dendrite-1e1000 shows the first frame after 93 ms. Pages: [[decisions/controls]],
[[synthesis/places]], [[techniques/coloring-methods]].

## [2026-10-14] research | R5: local series approximation on the GPU (float32 check)

Answered [[questions/local-sa-float32]]. Metal prototype in the render crate (behind `--sa 1` /
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
admission, chunked seed passes. Pages: [[techniques/local-series-approximation]] (GPU
prototype, pitfalls, M5 design), [[synthesis/benchmarks]] R5, [[roadmap]], [[index]].

## [2026-10-15] decision | PJ: journey autopilot, view rotation, cinematic effects

Pilot track. Journey (J): an endless seeded flight from landmark to landmark; targets from the
maths (ball period + Newton minibrots at 128 extra bits, embedded Julia stages at t = 1/2,
3/4, 7/8 and σ^(3/4), approaches, Misiurewicz spirals and branch points from the orbit's
near-repeats, detail zooms, resurfacing to shallow places), scored on 40 × 40 CPU probes with
survey-style statistics plus a filament share and a roughness measure, and variety rules;
legs are fly-to flights with a rest; the next plan runs on a thread during the leg and the
next reference is prefetched during the rest. View rotation end to end (kernel, reprojection,
tile priorities, oracle, overlays, navigator keys and gestures, coordinates text, regress
sessions). Cinematic effects (spin with the zoom, OKLab hue and phase drift with depth,
cycling in flight). Ingested [[sources/muency-newton-raphson-zooming]] and
[[sources/kisaka-julia-sets-in-mandelbrot]]. Pages: [[decisions/controls]] (Journey, View
rotation, Cinematic effects), [[ideas]] (status), [[roadmap]] (PJ), [[synthesis/benchmarks]]
(PJ), [[index]].

## [2026-10-15] decision | M5: local series approximation and interior fill in production

Shipped local SA ([[techniques/local-series-approximation]] "As built (M5)") on by default:
quadtree seeds 32 → 4 texels in resumable rounds (a node keeps its whole loop state; rounds
sized by a learnt latency model; pixel admission gated on the GPU until the seeds are final),
every level seeded at its own texel size, extension passes and AA samples from the full
level's leaves, float32 deep seeds down to 2^−80 texels (the spiral's seeds 150 → 40 ms), a
deterministic per-tile gate whose bar comes from the last level's steps per texel, Morton order
inside admission tiles, bounded rounds for gesture frames. Interior-only tile fill
([[techniques/guessing-and-subdivision]] "As built (M5)"): borders first, insides of
all-proven-interior tiles written without iterating, bit-identical, gated (elephant 1.35×,
off with analysed interior colourings). New benchmark location `spiral-32412-1e19` (the
user's slowest view) and regress sessions; auto iterations take a floor from the big
reference and keep the interior test on while pixels end unresolved
([[techniques/auto-iterations]]). Results ([[synthesis/benchmarks]] M5): dense-5e-14 4.95×,
sea-1e20 8.7×, neck-2e5 5.8×, root-third 3.3×, deep-5e-14 4.1×, dendrite-1e300 2.5×, spiral
3.96× (first render 4.0 → 0.78 s GPU), interactive refined frames 2–5× sooner, 0–3 % cost
where nothing is seeded; 0.000 % class change and no false interior at all 17 locations.
Pages: [[techniques/local-series-approximation]], [[techniques/guessing-and-subdivision]],
[[techniques/auto-iterations]], [[synthesis/benchmarks]], [[roadmap]], [[index]].

## [2026-10-15] decision | FB: flight benchmark with a score

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
[[synthesis/flight-benchmark]], [[synthesis/benchmarks]] (FB), [[index]].

## [2026-10-15] decision | M5 fix: learned state across jumps

The flight benchmark found the spiral settling black after a dive at c = i (1e-280): the
old view's interior-test statistics turned the test off and the learner could not retry
without it. Jumps now reset the learned iteration and gate state; a view at the limit without
the test renders again with it; `verify --history` checks against a from-scratch render; a
regress session runs the path. Page: [[techniques/auto-iterations]].

## [2026-10-15] decision | M7: float32 artefacts measured (none visible); float-float as scaffolding

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
1.014 under load, fields bit-identical. Pages: [[questions/float32-error-long-orbits]] (answered),
[[questions/double-float-speed-on-m1]] (answered), [[questions/metal-fast-math-defaults]]
(partial), [[techniques/double-float]] (As built), [[synthesis/benchmarks]] (M7), [[roadmap]]
(M7), [[overview]], [[index]].

## [2026-10-15] decision | M6: Imagina-style LA and AT built; used only where it pays

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
[[techniques/linear-approximation-imagina]] (As built), [[synthesis/benchmarks]] (M6),
[[questions/bla-vs-la-benchmark]] (answered), [[roadmap]] (M6), [[overview]], [[index]].

## [2026-10-15] decision | Overlay font, place thumbnail moiré, 120 Hz measurement, iOS simulator

Overlay text in Rust/Metal is now Inter SemiBold (OFL 1.1, licence shipped) as a
signed-distance-field atlas with a soft dark halo, crisp at any backing scale and legible on
pale and dark palettes ([[platform/overlay-text]]); `render --ppp` sets the overlay scale for
stills. Place thumbnails render 3× larger and resample with a Gaussian (σ 0.5 px, linear
light); the three needle minibrots were re-rendered without moiré ([[synthesis/places]]).
120 Hz: the interaction test reports the display link's actual and granted rate and can run
from a CVDisplayLink; a background window gets 20–33 callbacks per second (granted 120) and,
fed at 120 Hz, half the refreshes find no drawable, so real 120 Hz presentation needs a
frontmost window: `make interaction-test-120` for a person to run
([[synthesis/benchmarks]] "120 Hz on ProMotion"). iOS: simulator runtime installed, iPhone and
iPad simulators, self-test and an XCTest touch UI test; fixed a crash (presented handlers
missing on simulator drawables), a main-thread HEIC decode deadlock when the control center
opened, the HUD under the buttons on narrow canvases and the status bar
([[platform/ios-simulator]]). Updated [[platform/render-architecture]], [[index]].

## [2026-10-15] decision | Journey variety: dives, cross-overs, novelty

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
[[decisions/controls]] (Journey, variety revision), [[synthesis/benchmarks]] (PJ variety).

## [2026-10-15] decision | FB: window test took focus, fixed

The flight bench's window run activated the app (three of four runs; one with a key
window). `AlmondbreadMac/main.swift` activated a bare executable unless `--selftest` or
`--interaction-test` was given. It now checks `TestLaunch.enabled`, which covers
`--flight-bench=` and `--ui-demo=` too. The bench ends with `FOCUS TAKEN` and exit 1 on any
activation or key window; 5 runs were clean. Also explained `stats.max_iterations` 10384
against 41536 (nominal limit vs extension to the hard limit) and the 1e−1004 disc
(supersampling of sub-pixel rings). Page: [[synthesis/flight-benchmark]].

## [2026-10-15] decision | Sample cache: motion sample reuse in production

Built the sample cache R4 recommended ([[techniques/sample-reuse]] "As built",
[[synthesis/motion-sample-reuse]] "Production"). R4's free per-pixel samples and move pass
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
pages, sources, raw notes and prototypes. Numbers: [[synthesis/benchmarks]] "Sample cache".
Pages: [[platform/render-architecture]], [[index]].

## [2026-10-15] decision | Sample cache off by default after user test

The user found the merged build sluggish: blurry zoom-out and less smooth fast zooms. Feature
bisection with env switches in the real window traced it to the sample cache; M5 and M6 felt
as smooth as or smoother than the previous day. The cache is now opt-in
(`ALMONDBREAD_SAMPLE_CACHE=1`). Lesson: headless replays and background windows (capped near 30 Hz)
do not show interactive feel; a speed feature needs a foreground check before it becomes the
default. Pages: [[techniques/sample-reuse]].

## [2026-10-15] decision | Mini-Mandelbrot atlas proposed

Wrote up [[decisions/minibrot-atlas]] (proposed, not scheduled): local atlas from the journey's
target finder first, then a shallow census and deep samples. Filed the π-in-the-set idea in
[[ideas]] with our own numeric check.

## [2026-10-15] ingest | Cosmic size scale: sizes, epochs and the overlay

Built the cosmic size scale (key C, off by default) and filed its numbers: NASA fact sheets,
IAU units, CODATA 2022, a references page (galaxies, universe, cells, viruses, nuclei, the
quark size limit, big numbers, ages) and a cosmic-history timeline (times before ~1 s derived
from transition temperatures with the PDG radiation-era relation). Unconfirmed figures are
marked (unverified): adult height, the 93 vs 92 billion light-year universe (92 used), inflation
times. Design and costs: [[decisions/controls]] "Cosmic size scale"; status in [[ideas]].
Pages: [[sources/nasa-planetary-fact-sheets]], [[sources/iau-astronomical-units]],
[[sources/codata-2022-constants]], [[sources/cosmic-scale-references]],
[[sources/cosmic-history-timeline]], [[index]].

## [2026-10-15] ingest | How big is big? panel

Built the up-arrow gear-train panel and wrote [[synthesis/how-big-is-big]]: the linear
super-logarithm generalised to every arrow level, marker positions (googol 2.301, our limit
2.541, googolplex 3.301 on ↑↑; g₁ 2.364 on ↑↑↑↑), recomputed facts (googol after 2 min 24 s at
×4.95/s, googolplex zoom 4.6·10^92 years and 3.3·10^100 bits vs ~10^80 atoms). Finding: the
four-arrow dial and beyond read 1.15, 1.06, 1.03 … at our limit, not 1.000. Ingested (skimmed,
not saved) eleven web sources: [[sources/wikipedia-knuth-up-arrow-notation]],
[[sources/wikipedia-tetration]], [[sources/wikipedia-grahams-number]],
[[sources/wikipedia-kruskal-tree-theorem]], [[sources/wikipedia-steinhaus-moser-notation]],
[[sources/wikipedia-skewes-number]], [[sources/wikipedia-googolplex]],
[[sources/wikipedia-names-of-large-numbers]], [[sources/wikipedia-fast-growing-hierarchy]],
[[sources/wikipedia-observable-universe]], [[sources/make-ganson-machine-with-concrete]].
Status in [[ideas]].

## [2026-10-15] decision | Cosmic size scale: relative mode paced in linear zoom

User: "From hair to blood cell it takes ages." The relative cosmos now spans zoom 1e0 ... 1e200,
with the things evenly spaced in zoom decades (about 12 s a step at the default speed). The
epochs share the same grid, placed by thermal wavelength. Past 1e200 the panel counts cosmos
lengths. Also: a darker backing and outlines, labels kept near their dots, a captioned anchor
picker, and a "How big is big?" link past the Planck length. Pages: [[decisions/controls]].

## [2026-10-15] ingest | π in the Mandelbrot set, and the π meter

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
[[techniques/pi-in-the-mandelbrot-set]], [[questions/pi-escape-count-conventions]],
[[decisions/controls]] "π meter", [[synthesis/places]], [[ideas]], [[index]].

## [2026-10-15] decision | "How big is big?" panel removed

The user removed the panel: "It's an own window, which is not like the rest; way too much text;
not sure if people understand it." Gone from the app: `HowBigView.swift`, the `how-big` window
scene, its View-menu item and control-center button (and search keywords), the `howbig`
ui-demo scenes, the cosmic size scale's "How big is big? →" link with `Explorer.cosmicHint()`
and its Rust side (`CosmicOverlay::hint`, `Host::cosmic_hint_points`, `CosmicScale::past_planck`),
`almondbread-ffi/src/how_big.rs` and `almondbread-core/src/bignum_scale.rs` with its tests.
The maths (linear super-log convention, marker table, facts) stays in
[[synthesis/how-big-is-big]], marked removed, for achievements later; the sources pages stay.
Pages: [[synthesis/how-big-is-big]], [[decisions/controls]], [[ideas]], [[index]].

## [2026-10-16] decision | Autopilot target lock and rescue

The user reported that plain Z and Z with assist both end in featureless regions. Why: a
continuous zoom towards a generic point ends on one side of the boundary; the centroid assist
lags. Built: the target lock (Misiurewicz points, Julia stages, minibrots found off-thread with
the journey's finder, retargeted ahead of each end span, minibrots followed through, gain law
for the heading, pointer always wins) and rescue (backs out of a uniform heading region), on
by default with a control-center toggle and `ALMONDBREAD_AUTOPILOT_LOCK=0|1`; the assist is
gone. Headless tools: `almondbread autopilot` (CPU simulator), `fly --lock`, fly metrics.
Uniform share over 90 s went from 94–100 % (plain Z) to 0–8 % at the default speed on 11 of
12 starts; high-period minibrot starts at max speed stay weak (CPU probe iteration cap).
Pages: [[decisions/controls]], [[synthesis/benchmarks]].

## [2026-10-15] decision | π hunt: the view stays in the valley, a probe and the orbit trail

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
[[techniques/pi-in-the-mandelbrot-set]] "The orbit", "In the app"; [[decisions/controls]]
"π meter".

## [2026-10-16] decision | Compass

Built a compass after Apple Maps: a rose at the top right with a red north needle, shown while
the view is turned (or always, a setting), fading 3 s after north. Click or tap eases back to
north in 0.4 s (exactly 0, the spin of an autopilot, flight or journey goes on from north;
flights are turned along); drag turns the view 1:1. Rust draws (new triangle overlay
primitive), hit-tests and moves; Swift routes presses. Mini-map panels now respect the
safe-area insets. Pages: [[decisions/controls]] "Compass", [[index]].

## [2026-10-16] decision | Control center performance: invalidation, search, thumbnails

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
that would change how the sidebar looks. Page: [[platform/swiftui-performance]].

## [2026-10-16] decision | Control center: settings only, lazy sidebar, place captions

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
[[platform/swiftui-performance]], [[decisions/controls]].

## [2026-10-16] research | Places below 1e1000: four views to the depth limit

Four deep places: `pinwheel-1e1500`, `star-1e2000` (M(5,1), a four-way junction found with
`locate --misiurewicz 5,1`), `swirl-1e3000` and `rock-bottom` at 1e3007.5, one doubling above
the depth limit (2^-10000 per point) in a 1000-point window. Deeper Misiurewicz points only
repeat themselves, so these are embedded Julia sets: `locate --radius r` next to c = i, M(4,1)
and M(5,1) gives a minibrot of size ~r²; centred on it, the junction appears with 4k arms at
the geometric mean and 8k at 3/4 of the log-distance (by eye). Newton found no minibrot next
to the seahorse spiral M(24,2) at any radius. Cost with `verify --history jump` at 1600×1000:
first frame 85 ms to 1.0 s, resolved 0.2-1.1 s, all of it the reference (period 2659-17046 at
5120-10176 bits); class mismatch 0 against the oracle. Page: [[synthesis/places]].

## [2026-10-16] decision | Truncated references: a prefix's end is no rebase point

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
the path-dependent black frame FB reported); noisy machine, no clean timing comparison. Pages: [[techniques/rebasing]] (Truncated
references), [[techniques/parallel-reference-orbit]].

## [2026-10-16] decision | Places zoom pills, full screen with F and Esc

Places thumbnails show only the zoom in a small dark pill at the bottom right; the name is the
tooltip (iOS: tap shows it under the grid, long press opens a menu titled with it). Full screen
on macOS: F toggles (as well as ⌃⌘F, ensured in the View menu), the control center closes on
entering and is restored on leaving, the toolbar auto-hides (macOS 15+), insets are re-sent
after transitions and on `contentLayoutRect` changes. In full screen Esc stops motion first
and leaves full screen only when nothing moved; repeats never leave. Interaction-test scenario
"esc full screen" checks it through a probe (no window enters full screen). Page:
[[decisions/controls]] "Full screen".

## [2026-10-16] decision | Needle axis band for local SA; AA samples filter over their share; learned limit survives AA

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
[[techniques/local-series-approximation]], [[techniques/anti-aliasing]],
[[techniques/coloring-methods]], [[techniques/auto-iterations]],
[[synthesis/flight-benchmark]], [[synthesis/benchmarks]].

## [2026-10-16] decision | Motion fixes: pipeline warm-up, gesture cost after jumps, lock probes, deeper journeys

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
in 62 700 recorded frames. Pages: [[platform/metal-compute]] "Pipeline warm-up",
[[decisions/controls]] (Fly to stalls, target lock, Journey revision),
[[techniques/pi-in-the-mandelbrot-set]], [[questions/metal-watchdog-limit]],
[[synthesis/benchmarks]] "Motion fixes".

## [2026-10-16] decision | Extension passes stop at the end of an f64 orbit cut at the limit

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
(`--max-cpu-median 0.05`, a new history gate). Pages: [[techniques/rebasing]] (Truncated
references), [[techniques/auto-iterations]].

## [2026-10-16] decision | Hang prevention: loop audit, lints, extreme-value tests, main-thread watchdog

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
[[techniques/hang-prevention]], [[sources/apple-metrickit-hang-diagnostics]].

## [2026-10-17] query | The −3/4 neck gap at 1e8–1e9: ground truth, GPU check, seed horizon

User report: just above the neck (`−0.74999996… + 0.00036918… i`, 1.9e8–1.3e9, −3.2°) the view
is almost black with thin filaments, areas appear while zooming, and a 10 M limit changes
nothing. New tool `examples/neck_truth` (direct f64 with an interior proof, escape histograms,
`|dz/dc|`, double-double checks, `--paint` for any `.iter` dump). Findings
([[synthesis/neck-escape-statistics]]): everything in view escapes (minimum ~8 500 ≈ π/ε,
medians 4e4–4e5, a disc at 2e7–7e7), escape counts are chaotic far below the pixel (f64 vs
double-double differ for 9 % of pixels; only statistics are meaningful); the GPU's float32
histograms at 1e6 and 1e7 match the f64 oracle to sampling noise, and the straight seam of
`render --iterations 1e6` is real structure. The app's black: the learner caps at 4× depth
(15–18 k) whatever the user's cap, the extension list overflows at Retina sizes, and with a
manual 10 M the local SA seeds stalled the level (no pixel ran in 20 s at 4000 × 2500; fixed:
`SEED_HORIZON` = 2^16, [[techniques/local-series-approximation]] "Seed horizon"); after that the
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

## [2026-10-17] decision | Render progress: a top line and HUD text for long renders

A thin line at the top of the canvas (Rust overlay, below the toolbar) and a HUD line
("refining · 2.1 M of 10 M steps · 3 % of pixels left · ~40 s") while a settled view computes
for more than 0.5 s. Time-based and pessimistic: remaining = the level's pixels over their
resolve rate blended with the youngest live pixel's steps to the limit (new `liveMinN` chunk
counter), plus predicted extension and AA stages. Measured with the new `almondbread progress`
command: at a 2 M limit at the neck (3–4 min) the line stays within −10 / +6 points of the
share of real time, at a 10 M limit with a slow tail (8 s) within ±10. Shallow zooms show
nothing. Found: the first AA sample at a 2 M limit costs as much as the full level. Page:
[[decisions/controls]] "Render progress".

## [2026-10-16] decision | Cosmic scale: "Your screen" keeps the window, the set grows

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
committed). Page: [[decisions/controls]] "Cosmic size scale".

## [2026-10-17] decision | Cosmic scale: one mode, three phases, nested universes

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
`~/Desktop/cosmic-scale-stills.png`. Page: [[decisions/controls]] "Cosmic size scale".

## [2026-10-17] decision | Open-source release

[[decisions/open-source-release]]: MIT with saturnus's AI notice, `THIRD_PARTY_NOTICES.md`
(Inter OFL, colour-map licences, crates), `deny.toml`, and the pipeline: CI on push (Linux
Rust gates without the Apple-only ffi crate, Windows check of math and core, kb lint, a
clean-checkout macOS build), a tag-driven Developer ID signed, notarised and stapled `.dmg`
in a draft release (saturnus's six Apple secrets), and the `almondbread` cask in
`ractive/homebrew-tap`. Publication audit: no secrets or large files in history, no
GPL/AGPL crates, no copied code from KF, F3, Imagina or FractalShark; the business pages
are listed for the user to decide on. Donations dropped for the first release.

## [2026-10-17] lint | Business pages moved out of the public repository

Before the first public push, the business pages moved to a private folder outside the
repository: the market and game/Steam research reports, the distribution and gamification
decisions, thirteen business source pages (store listings, pricing and conversion benchmarks,
Steam process) and their raw research notes. Links to them were replaced with plain text,
index entries removed, and price and conversion figures trimmed from the remaining pages.
Pages: [[index]], [[ideas]], [[decisions/open-source-release]], [[decisions/minibrot-atlas]],
[[synthesis/places]], [[synthesis/how-big-is-big]], and the "Used by" sections of the
remaining market and engine source pages.

## [2026-10-17] research | Finding good places at extreme depth

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
[[synthesis/finding-deep-places]], [[questions/julia-morph-depth-fractions]] and nine source
pages ([[sources/mathr-julia-morphing-posts]], [[sources/mathr-embedded-julia-navigation]],
[[sources/mathr-domains-and-periodicity-scan]],
[[sources/heiland-allen-inflected-julia-sets-2025]], [[sources/dinkydau-evolution-journals]],
[[sources/fractalforums-pathfinding-revisited]], [[sources/fractalforums-deepest-zoom-poll]],
[[sources/deep-zoom-videos]], [[sources/wikibooks-emndl]]); "Used by" backlinks on seven
existing sources and one line in [[synthesis/places]]. Nothing saved to `raw/` (read on the
web; the forum thread via the Wayback Machine).

## [2026-10-17] decision | Help: (?) popovers, a help window, Markdown texts

Help in three layers ([[decisions/controls]] "Help"): (?) popovers in the control center's
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

## [2026-10-17] decision | Attracting-cycle probe for interior detection

The `|w| n²` test proved only the fast core of a component (satellite: `|λ| < ~0.7` at 51
periods), so minibrot bodies and bulbs stayed unresolved grey. Pixels unresolved at the limit
now park and a kProbe pipeline variant searches their orbit for the cycle (near-0 base point,
candidate period from a return), runs Newton on `f^P` and proves an attracting fixed point
with a Kantorovich bound and float32 noise margins; repelling parents of satellite bulbs
resume the search for longer periods. No false interior in 19 views against the CPU oracle
at high limits; satellite proven 1.1 → 3.4 % of 3.7 %, cardioid edge 55.7 → 84.0 % of 86.1 %,
cusp 35.7 → 87.9 %. Near-parabolic views (neck, 1/3 root) stay unresolved and cost 10–40 %
offscreen. Pages: [[techniques/interior-detection]] "Attracting-cycle probe",
[[synthesis/benchmarks]] "Interior probe". Contact sheet: `~/Desktop/interior-before-after.png`.

## [2026-10-17] query | Autopilot check failure: GPU clocks trap the gesture resolution at 1/8

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
[[synthesis/benchmarks]] "Autopilot throttle and GPU clocks".

## [2026-10-17] decision | Compute further: background passes up to the Upper limit

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
(upper limit)". Page: [[techniques/auto-iterations]] "Compute further". Image:
`~/Desktop/compute-further.png`.

## [2026-10-17] decision | Compute further after AA; off by default in `verify --history`

Compute further now waits for the view's AA samples, then marks the texels still unresolved
(one bit each) and composes only those as they resolve, so a still view is anti-aliased as
soon as before (neck 1.9–2.3 s against 2.3 s on main, seahorse and home unchanged) and keeps
its samples. New `verify --history` check: resolved pixels drawn in the unresolved colour (0
on home, seahorse and the neck with AA; 14 % with the compose mode disabled on purpose).
`verify --history` defaults to `--further 0`; one regress session (neck, 400 × 250, AA, 4 s
budget) runs it: `make regress` 302.8 s against 300.2 s on main. Page:
[[techniques/auto-iterations]] "Compute further".

## [2026-10-17] decision | Cosmic scale: nested-universe count off by one

The user asked whether the cosmic comparison's maths is right. Phase 1 and 2 check out (set width
`L · 2.25 / w` with the window in points; the universe is reached at zoom ≈ 3e27 for 30 cm, so a
flight passes all of astronomy in its first 27 decades). Phase 3 counted `max(2, floor(n / a))`
universes, one short: at `n = a` the set is already a universe whose atoms are universes. Now
`floor(n / a) + 1` ("… (27 times)" at 1e1000), and the "k× nested" ruler marks sit at
`n = (k - 1) a`. [[decisions/controls]] "Cosmic size scale" updated.

## [2026-10-17] decision | Previews wait for a new reference's BLA table

Autopilot stalls of ~200 ms near 1e-247 (user report on bb69748): the first preview after
each new reference started without BLA (the table builds on a thread after the job asks) and,
as a gesture frame, ran in one K = maxIter chunk sized by the cost measured with BLA: 200–1090
ms of GPU per new reference, a second time when a truncated reference was replaced. Present
on b466f42 too; not caused by the interior probe, compute further or the gesture probe (during
the dive every job is a gesture job: no full levels, probe chunks, further passes or AA). Fix:
a job's first level waits for the table (1.3–6.4 ms) and the history is shown meanwhile
(`ALMONDBREAD_PREVIEW_WAIT_BLA=0` restores the old behaviour). Largest chunk 937–1086 →
10–14 ms, longest unpresented run 13–15 → 0–2 refreshes. Page: [[techniques/bla]] "Pitfalls".

## [2026-10-17] decision | Zoom-out autopilot (⇧Z) and journey with stops (⇧J, Z / X at a stop)

⇧Z flies a van Wijk–Nuij path from the view to the home view at the autopilot's speed with a soft
landing that rides the brake curve in closed form (the fly-to landing reached the end still moving);
from 1e-3000 without overflow. The journey's new "stop at each find" mode (control center setting;
J the set mode, ⇧J the other) rests at each landmark until J (as planned: the continuous journey's
target), Z (`plan_with` `Choice::Deeper`: ≥ 1 e-fold deeper in this view, else the plan with
"nothing deeper here") or X (`Choice::Elsewhere`: cross over now). Reproducible from seed, start view
and the choices; `journey --choices jzx`. Page: [[decisions/controls]] "Zoom-out autopilot",
"Journey with stops".

## [2026-10-17] decision | Unresolved pixels drawn as fog

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
last pass. Help texts say "fog". Page: [[techniques/unresolved-fog]].

## [2026-10-17] decision | Endless dives: places centred exactly on Misiurewicz points

New place category "Endless dives": `endless-forking` (c = i), `endless-spiral` (M(24,2)),
`endless-junction` (M(4,1)), each repeating every |λ| times magnified turned by arg λ (5.657 at
45°, 1.0789 at −14.87°, 1.3283 at 119.55°), and `endless-wander`, a never-repeating point from
`locate --endless-seed 2026` (1,000 random angle bits, ray tracing + Newton, 42 s; changes down
to ~1e-295, then repeats ×4.605). M(24,2) and M(4,1) extended to 3,307 digits (agree with the
stored digits). Page: [[synthesis/places]] "Endless dives".

## [2026-10-17] decision | Autopilot survey steering (opt-in)

User report: Z often flies into the void near 1e247 (−1.7686678314…, 0.0016455577…).
Diagnosis with `fly --log` and `ALMONDBREAD_LOCK_TRACE=1`: the lock finds only probe
"boundary detail" targets there (no nucleus or Misiurewicz point resolves); they are exact to
1/40 of the span, run out of detail 3–6 e-folds deeper, and when the search ahead fails the
lock keeps aiming at the spent point until a rescue. New `ALMONDBREAD_SURVEY_STEER=1` (off by
default): whenever no lock target is followed, aim at the GPU survey's boundary point (per
cell the highest-escape-count texel, a new field of the survey kernel), kept while its region
holds detail, uphill in escape count when nothing is detailed. User point, 90 s × 5
interleaved runs: uniform 6.7 → 1.3 s per run, rescues 6 → 1, zoom 25.4 → 26.8 decades;
other deep paths unchanged (no voids either way). Page: [[decisions/controls]] "Survey
steering".

## [2026-10-17] decision | Driving levels: M · C · A · J, pedals, reverse cruise, lane assist hook

The user's car-like design: manual, cruise, autopilot, journey, shown in a Rust-drawn HUD
cockpit with a zero-centred speed gauge and a lane-assist icon. Q / E are pedals on one signed
ladder (×1.5 … ×1000 per second) with a detent at zero; held keys step on. Reverse cruise boosts
with depth, `(1 + D/3)^ramp`, ramping over 4 s, and hands the last stretch to the soft landing:
home from 1e-10 / 1e-50 / 1e-300 / 1e-3000 in 17 / 29 / 42 / 58 s at the first rung. User
steering or pedal-to-zero in A or a journey leg hands over to cruise at the current speed; the
autopilot no longer takes the wheel back. ⇧Z removed. Lane assist (L) is a setting and a hook,
wired to the survey steering in phase 2. Page: [[decisions/controls]] "Driving levels".

## [2026-10-17] decision | Gesture resolution probes on by default

The user compared `ALMONDBREAD_GESTURE_PROBE=1` against the default in the foreground app (panning,
zooming, the autopilot near 1e45): smooth, sharp, no stutter noticed, hard to tell apart by eye. The
measured gains (autopilot ×4.5 vs ×2.2 in 2.4 s, preview resolution about doubled) decide: the probes
and median-fit hysteresis are now the only gesture-resolution path; the switch is removed
([[synthesis/benchmarks]] "Autopilot throttle and GPU clocks").

## [2026-10-17] decision | Behaviour switches removed: a decided choice is the only code

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

## [2026-10-17] decision | ⌘C / ⌘V on the canvas; jumps stop all motion

The user asked why coordinates used ⇧⌘C / ⇧⌘V. The canvas view now answers the standard copy: /
paste: actions itself (macOS routes Edit > Copy / Paste to the focused view), so ⌘C copies the
view's coordinates and ⌘V goes to copied ones while the canvas has focus, and text fields keep
normal copy and paste; ⇧⌘C / ⇧⌘V stay as menu items. A jump (pasted coordinates, a link, a
place) now calls `Navigator::jumped`: cruise, autopilot, flights and flings stop dead and a
journey ends (user request). [[decisions/controls]].

## [2026-10-17] decision | Cockpit polish; two-finger scroll setting; help popovers at the bottom edge

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

## [2026-10-17] decision | Cosmic scale reworded; toolbar slimmed, cockpit icons clickable

The user reworded the cosmic scale's deep phases.
- Readout subject: "The whole Mandelbrot set" ("set" alone is jargon for newcomers).
- Phase 2: "… would be as big as an imaginary universe" / "in which an atom is as big as ours".
- Phase 3: imaginary universes nest. At each level the Planck length is as big as the universe below, so a level is P = log10(U / Planck) ≈ 61.7 decades. The readout says "…, 16 levels deep" / "at the bottom, 1/72,000 of a proton is as big as ours".
- The atom-ratio nesting (and `level_decades`) is gone. Ruler marks "Earth = universe" … "Planck = universe", then "3 levels" … "52 levels".

The π meter, autopilot and journey buttons left the macOS toolbar: the π meter is niche, and the cockpit shows the levels. The cockpit's icons now take clicks: manual stops (Esc), cruise starts cruising in (E), autopilot and journey toggle (Z, J). The pointer shows a hand over them as over (?) badges. [[decisions/controls]].

## [2026-10-17] decision | Closed-form cardioid / period-2 bulb test at any depth

User report: views at the main cardioid's and the period-2 bulb's boundaries (the −3/4 neck at
4176×) showed large fog areas that compute further could not resolve. Built an exact test:
per level the CPU expands both closed forms in BigFloat as degree-4 polynomials of the
kernel's texel offset. The kernel marks a fresh texel interior iff the float32 sum is below
−(2^-16 · sum of magnitudes + slack), which is a proof at any depth. Neck at 4176×: 45 % →
0.05 % unresolved, GPU 25 → 2.5 ms. Cusp at 1e-9: 100 % → 0 %, 210 → 4 ms. No class mismatch
anywhere. The user's second view (−0.6443−0.3662i) is inside a period-64 component, not the
cardioid, and is unchanged. Page: [[techniques/interior-detection]] "Closed-form component
test".

## [2026-10-17] decision | Orbit overlay: dot cloud on the whole-set map, plain result text

In slow-escape regions the orbit overlay showed 1000 iterates while the action is at step
10^4 to 10^5, and its text read "analysing ..." / `|z50| 0.5362` (user). The probe now runs at
least 10^5 steps and keeps a decimated `OrbitCloud` (10^5 points, stride doubling); the
whole-set map draws the first 50 steps as a line and the rest as one-pixel dots from a CPU
picture blended by the overlay kernel (0.43 ms per rebuild of 10^5 points, rebuilt every ~19 %
of growth). The text says "escapes after 48 213 steps", "settles into a period-34 cycle",
"still undecided after 100 000 steps". [[decisions/controls]] "Orbit overlay".

## [2026-10-17] decision | Seek minibrot (N)

- Built the user's "seek minibrot": N (menu View ▸ Seek Minibrot, iOS long press on the plane
  button) finds the lowest-period minibrot in or near the view (ball period detection over discs
  of 1, 3, 9 view spans, Newton refined until its last step is far below the size), tells
  minibrots from satellite bulbs by the attracting cycle at w = −0.6, and flies to a framing of
  3.2 |s| centred inside the cardioid. From a minibrot's framing it goes on to the next smaller
  one: the tuned copy of the period-3 minibrot (period 3p) or a lower period in a ring of discs
  around it. Bounded (4 s, cancel flag, fixed discs and retries). Home → 3 → 7; needle →
  4640 → 13920. [[decisions/controls]] "Seek minibrot".

## [2026-10-17] lint | Consolidation pass after the 2026-10-16/17 features

Read the log entries of both days and swept the wiki for what they removed or changed.
- Stale claims fixed: [[decisions/controls]] (cockpit levels shown as icons, not M · C · A · J, and clickable; toolbar buttons for the π meter, autopilot and journey gone; the iOS zoom-out long press and `NavKey::AutopilotOut` went with ⇧Z; the cosmic scale phases 2 and 3 and the ruler reworded to imaginary universes and 61.7-decade Planck levels; survey steering marked as still an env switch, shared with lane assist), [[techniques/interior-detection]] and [[decisions/controls]] (the grey veil is fog now), sample-cache mentions in [[synthesis/motion-sample-reuse]], [[techniques/frame-reuse-and-progressive]], [[platform/metalfx]], [[synthesis/flight-benchmark]] and [[techniques/unresolved-fog]] (code deleted), a switch-names note on [[synthesis/benchmarks]], its gesture-probe paragraph (no longer opt-in), the M6 row of [[roadmap]] (AT deleted). Other pages already carried "removed 2026-10-17" notes.
- Technique homes: new section "Gesture resolution and GPU clocks" in [[platform/metal-compute]] (the explanation; numbers stay in [[synthesis/benchmarks]]). The other techniques already had one home section each (interior-detection probe and closed-form test, auto-iterations "Compute further", unresolved-fog, bla "Pitfalls", places "Endless dives", controls driving levels, lane assist, survey steering, journey stops, cosmic scale); cross-links added.
- [[overview]] rewritten for the current state (LA without AT, interior tests, fog, compute further, controls, switches removed); [[index]] one-liners updated.
- Links: no broken links, no orphans, `hyalo lint` clean. Left alone: the business pages (market, Steam, gamification, distribution and their sources) are still in the tree but not in the index, although the 2026-10-17 entry says they moved out of the repository.

## [2026-10-17] decision | Smooth journey legs: eased landings and turns, no dull-pace pulses

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
[[decisions/controls]] "Journey" (revision "smooth legs").

## [2026-10-17] decision | Survey steering on by default

The user tried `ALMONDBREAD_SURVEY_STEER=1` in the foreground at the 1e247 path. It ran into featureless regions twice, backed out quickly and resumed. Headless numbers: 1.3 s instead of 6.7 s featureless per 90 s, 1 reversal instead of 6. Survey steering is now always on, and the switch is removed. [[decisions/controls]] "Survey steering".

## [2026-10-17] decision | Compute further: "Continue" at the time limit; cosmic scale without numbers

When compute further stops at its time limit (`further_budget`, 60 GPU s), the HUD's progress line offers **Continue** (button, key G). `Renderer::continue_further` adds another budget to this view's passes (`Further::extra_budget`) and resumes where they stopped. Only a stop at the time limit is offered; at the upper limit more time does not help. The HUD is hit-testable now, for the button. At 100 % the line reads "nothing finished yet" (user: "Is there a reason why not saying 0% finished?").

The cosmic scale's deep phases name no numbers any more (user: "concrete numbers don't make sense anymore"). The thing as big as our universe is named when within a factor 2. Otherwise it reads "something between the solar system and the Milky Way", or beyond the ends "bigger than the Milky Way" / "smaller than the Planck length". [[techniques/auto-iterations]], [[decisions/controls]].

## [2026-10-17] decision | Component Newton: interior near the edge of any low-period component

User report: views touching a hyperbolic component other than the cardioid and the period-2
bulb kept a wide fog band inside its edge. There `|λ|` is 0.99–0.999, so the orbit settles
too slowly for the `|w| n²` test and for the probe's scan. Newton on `f^p(z) − z` from `z = 0`
converges in about 5 passes everywhere in such a component (f64, period-22 and period-64 views).
The CPU now finds the periods at a 5 × 5 grid of sample points per view (atom-domain
candidates, the smallest that Newton shows attracting; < 0.5 ms in f64). The probe tries them
from `z = 0` before its scan, and the existing Kantorovich bound proves each pixel. A lean
chunk dropped the probe state of probing pixels; that is fixed. Results: period-22 view
unresolved 8.3 → 4.4 %; period-64 view 79 → 22 %, settled 20.6 → 3.0 s in the app scheduler.
0 false interior in 13 views. Page: [[techniques/interior-detection]] "Component Newton".

## [2026-10-17] decision | Pointer hand-over from the autopilot needs real steering

The user: "when in Autopilot mode, moving the mouse just a bit throws you into cruise mode".
The hand-over now needs all three: 60 points of pointer travel (was 12), the pointer more
than 0.35 of the steering radius off-centre (was the 0.1 dead zone), and that held for 0.6 s
(was 0.25). WASD, a drag and the stick still hand over at once. [[decisions/controls]] "Driving levels".

## [2026-10-17] decision | Pointer hides while the autopilot or a journey flies

User request. In A or J, the pointer hides after 1.5 s without moving over the canvas (`NSCursor.setHiddenUntilMouseMoves`) and shows again as soon as the mouse moves. It is a direction wish in the autopilot, so it shows whenever it is in use. The HUD, the control center and window tests are unaffected. [[decisions/controls]].

## [2026-10-17] decision | Steady autopilot rate: the throttle slows only when frames miss the budget

User report: the autopilot's rate readout varied between ×1.75/s and ×4.x/s. `fly --log` at the
user's 1e240 point (and seahorse for comparison): the renderer throttle alone moved the rate
(×1.32–×2.46 at the default speed, hint "Rendering" 30–81 %); safety and the lock's capture
slow-down were minor. The throttle followed the gesture fit, which reads back the preview step
the scheduler runs at (median 0.137 at 1/8, 0.238 at 1/4, 0.81 at 7/8), so the 1/8 ↔ 1/4 probe
toggling and cheap/dear regions swung it 0.36 ↔ 0.95, while slowing does not make a fresh preview
cheaper. Fix: throttle full at fit 1/10, floor at 1/16 (only a coarsest preview that misses the
budget slows), fit log filtered 0.5 s down / 1 s up. Rate sd 0.15–0.33 → 0.06–0.09 e-folds/s at
the user point, 0.22 → 0.002 at seahorse; ~5 more decades per 90 s. Page: [[decisions/controls]]
"Steady autopilot rate".

## [2026-10-17] lint | Business pages removed from the tree

The 24 business files were still tracked: the two research reports, the distribution and gamification decisions, 13 source pages and 7 raw research notes. The 2026-10-17 entry above had said they had moved. They are now deleted from the repository, after a byte-for-byte check that `../almondbread-private/kb/` holds identical copies. They remain in the git history until the user decides how the history goes public.

## [2026-10-17] decision | Pointer wish in the autopilot; target marker

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
control center Overlays; on by default; persisted. [[decisions/controls]] "Pointer wish",
"Target marker".

## [2026-10-17] decision | Guided tours; the Feigenbaum tour

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
Format and keys: [[decisions/controls]] "Guided tours".

## [2026-10-17] query | Beach ball opening and scrolling the control center

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
[[platform/swiftui-performance]] "Hang: opening and scrolling the sidebar".

## [2026-10-17] decision | Cruise: 26 gears, gear-scaled reverse boost, gear / target line

User-approved: the pedal ladder has 26 gears (×1.1 … ×1000/s), a held pedal repeats every
0.15 s, the reverse depth boost ramps over 10 s and its exponent is scaled by the gear (0.5 in
gear 1 to 1 in gear 26), so low gears zoom out gently: home from 1e-10 in gear 1 takes 126 s,
from 1e-3000 in gear 10 (×5) 69 s, in the top gear 10 s (table in [[decisions/controls]]
"Driving levels"). Two real bugs surfaced while adapting the navigator tests: the soft landing
stalled about 1e-4 of path short of home and snapped after 1.5 s (it left its brake curve for
the spring; now it stays on the curve), and Q could not bring the autopilot or a journey to zero
(their speed floor 0.1 sat above gear 1, 0.095; now 0.09). The cockpit's small line shows the
gear in cruise ("gear 5", "reverse 3 · boost") and the set speed in A and J ("target ×20") only
while the rate flown differs from it by more than 5 %; in J it now compares the journey speed
with the leg's flown speed. Cockpit 306 points wide (fits a phone).

## [2026-10-18] decision | Touch controls for iPhone and iPad

[[decisions/controls]] "Touch": parity audit of the Mac's keyboard and pointer features against
touch, iPad keyboard and pointer, and the touch design built on branch `ios-touch`: a throttle
jog lever (one gear per 14 points, the pedals' paths, zero stop per drag, double tap stops), a
touch cockpit (44-point cells, one or two rows, above a bottom-left map row on a phone),
steering-finger events (the autopilot's wish stays after the lift), a long-press popover menu
(seek minibrot here, orbit here with a draggable probe, fly here, copy / go to copied),
fingertip-sized (?) badges, haptics, finger-sized tour and Continue buttons, the Mac's menu
commands in the iPad menu bar. New navigator events `TouchSteer`, `Throttle`, `Probe`,
`SeekAt`, `FlyHere`; XCTest `TouchControlsUITests` on both simulators. Simulator only; feel
needs a device.

## [2026-10-18] decision | iPhone/iPad device readiness: device build, memory budget, thermal, background

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
Pages: [[platform/ios-device]], [[platform/ios-ipad-considerations]],
[[sources/apple-device-tooling]].

## [2026-10-18] decision | Touch round 2: a first run by a child

[[decisions/controls]] "Touch", round 2 (branch `ios-touch-2`, simulators only): where a finger
steers (A, C, a journey's flight; `Navigator::finger_steers`, `touch_hold`) it never opens the
long-press menu — a resting finger steers after 0.2 s, a journey's flight hands over to cruise
on a touch off the dead zone; the menu stays where nothing flies (its point would be stale
under a moving picture). Long press on a mini-map flies there. First-run touch hints (dismissed
by any touch, back from Help). Tour card on a phone at the top, folding and scrolling. iPad menu
bar: Tours, Help, Show Touch Hints. Fixes from the surface review: help popovers off-screen at
the bottom of a phone (iOS `arrowEdge` semantics), fly-to sheet clipped (420-point minimum) and
asking "Allow paste?" on open. Window tests, `--ui-demo` and the iOS UI tests no longer read or
write settings (`TestLaunch.ephemeral`). See also [[platform/ios-simulator]].

## [2026-10-18] decision | iOS QA pass: a first-time user before a child's first try

Branch `ios-qa`, simulators `QA iPad Air 11` and `QA iPhone 17e`. `FirstUseUITests` scripts
first sessions with screenshots at every step ([[platform/ios-simulator]] "QA"). Fixed: the
long-press seek was never polled (no frame requested: `Navigator::wants_frames`); a tap on a
place's picture only selected it (now goes there on iOS); pinches turned the view (touch twist
dead band 15°, `TouchRotate`, Mac unchanged); "Allow Paste?" on Go to Copied (system Paste
control); click / key words on touch; compass over the throttle on a phone on its side.
[[decisions/controls]] "Touch" notes the decisions.

## [2026-10-18] decision | Places grid on touch: select, then Go or Fly on the picture

The QA pass made a tap on a place's picture go there on iOS, which left Fly unreachable (its
button is below the 79-picture grid, off screen; found on a real iPad). User's design, iOS only:
the first tap selects (border, name along the bottom edge, Go and Fly over the image, ≥ 44
points, material background); a second tap on the picture goes there; VoiceOver has Go and Fly
as actions. The smallest tiles (~100 × 63 points: iPhone sheet, iPad sidebar at its minimum)
take the label under the icon; wider tiles side by side (`ViewThatFits`). New XCTest
`testPlaceTileGoAndFly`; `testControlCenterPlaceAndTour` now selects, then taps again.
[[decisions/controls]] "Touch" (places grid).

## [2026-10-18] decision | Touch device round: pinch release, cruise drag, wish line

First try on a real iPad Pro 11" (M5); [[decisions/controls]] "Touch", "Device round". (1) A
pinch whose fingers lift one at a time panned with the remaining finger: UIKit keeps the pinch
alive with one finger and its location jumps to it. Swift now sends pinch and twist events only
with two fingers down; Rust ignores `Pan` / `TouchSteer` after a pinch until `TouchesLifted`
(every finger off). (2) Cruise steering by touch was the Mac's hover joystick about the centre
(dead zone, quadratic response): a drag of a few centimetres moved the view ~17 points/s, the
other way. A drag in C and in a journey's flight now pans, as a drag does on the Mac; the finger
steers only in A (the wish). (3) The wish line in A never went away: the lifted finger's wish
was kept until A ended. It is now let go once the autopilot's wish settled, so the line fades.
Simulators `DR iPhone 17e`, `DR iPad Pro 11`.

## [2026-10-18] decision | iOS round buttons: the Mac toolbar instead of autopilot / journey

User on a real iPad: the round buttons duplicated the cockpit. Now Orbit, Mini-map, Size scale
(toggles, filled when on), Fly to and Control center, as the macOS toolbar. Journey stop choices
moved under the HUD as labelled buttons; the autopilot button's menu is covered by the cockpit,
the throttle and a new iOS control-center "Seek minibrot"; the orbit toggle on touch shows the
view centre's orbit (or the last Orbit Here point); touch hints point at the cockpit's plane.
[[decisions/controls]] "Touch" ▸ "Round buttons".
