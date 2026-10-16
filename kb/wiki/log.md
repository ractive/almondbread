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

Deep-research run on how to position almondbread against the fractal-app market. Notes
in `raw/research/fractal-app-market-research/` (App Store apps, desktop/Steam/web tools,
audience and demand, market size and business models). Report:
[[synthesis/fractal-app-market-research]]. Findings: at least six 2025–26 App Store apps
already claim perturbation depth, and several ship autopilot, links and video (closest:
Fractal Dive, $1.99 Pro). Only sustained real-time motion at extreme depth is unclaimed.
The Mac lacks any native deep-zoom creator tool. The niche is small (Frax: 232 US ratings
in 13 years). Recommendation: position as a Mac/iPad flight and video studio; free tier
with full depth, $9.99 one-time Universal Purchase Pro gated on output, later ~$29.99 Mac
Studio tier, no subscription; launch through depth-titled videos and a technical HN
write-up; defer Windows/Linux. New source pages: [[sources/fractal-dive-app-store]],
[[sources/frax-app-store]], [[sources/frax-help-explore]], [[sources/mandelbrot-metal-app-store]],
[[sources/revenuecat-subscription-apps-2026]], [[sources/apple-eu-terms-oct-2026]],
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

Deep-research run on turning almondbread into a game and shipping a Steam edition. Notes in
`raw/research/fractal-games-and-steam/` (fractal and maths games, Steam publishing, Steam tech
stack); report [[synthesis/fractal-games-and-steam]]. Findings: no commercial deep-zoom
Mandelbrot game exists; Fractal Block World is the only fractal hit (1,652 reviews) and fractal
tools stall at 20–30 reviews; mechanics ranked daily challenge > atlas with discoverer naming >
depth missions > journey mode > canyon racing; Steam costs $100 but wishlists decide the
outcome (Bronze ~5k); Steamworks covers leaderboards and ghosts, the atlas needs an own
backend; no engine: wgpu + winit + steamworks-rs, Metal kept on Apple, new crates
`almondbread-wgpu`, `-game`, `-platform`, `-steam`, `-app`. New sources:
[[sources/fractal-block-world-steam]], [[sources/steamworks-onboarding]],
[[sources/steamworks-leaderboards]], [[sources/steamworks-steam-deck-compat]],
[[sources/steam-hardware-survey-sept-2026]], [[sources/gamediscoverco-wishlist-conversion]],
[[sources/htmag-benchmarks]], [[sources/wgpu-changelog-v30]], [[sources/steamworks-rs]],
[[sources/bevy-0-20-release-notes]]. Research outcome added to [[decisions/gamification]].

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
Pages: [[synthesis/how-big-is-big]], [[decisions/controls]], [[decisions/gamification]],
[[ideas]], [[index]].

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
