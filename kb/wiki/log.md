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
