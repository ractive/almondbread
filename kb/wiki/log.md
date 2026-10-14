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
