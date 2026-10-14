---
title: "almondbread benchmarks and verification record"
type: synthesis
date: 2026-10-14
sources: ["[[sources/mlx-watchdog-issue]]", "[[sources/cheritat-mandelbrot-rendering]]", "[[sources/fractalforums-interior-detection]]", "[[sources/metal-benchmarks-readme]]"]
tags: [benchmark, verification, measured, metal, interior, chunking, interaction]
---

# almondbread benchmarks and verification record

The running record of almondbread's own measurements. Append a new dated
section per milestone; keep older tables for comparison. All numbers are ours:
`measured 2026-10-14, M1 Pro (16-core GPU), macOS 26 (Darwin 25.5), release`
unless a section says otherwise.

## Tools

- `AlmondbreadMac --bench [--sizes WxH,...] [--interior 0|1] [--chunk-ms N] [--out dir]`
  renders the fixed set below at 3456×2234 and 1600×1000 (best of 2 after a
  warm-up), then GPU-vs-CPU verifies each location at 400×259. `--out` writes PNGs,
  diff heatmaps and raw `.iter` dumps (n, smooth offset per pixel).
- `AlmondbreadMac --verify --center re,im --scale s --size WxH --iterations N`:
  one GPU render vs the CPU oracle (Double, direct iteration from z = 0, no
  perturbation, `concurrentPerform` over rows; same grid, bailout 2¹⁶ and smooth
  formula). `--fields a.iter --against b.iter` diffs two dumps, e.g. before/after
  a renderer change.
- `AlmondbreadMac --interaction-test`: opens the real window in the background
  (1280×820 pt = 2560×1640 px), drives pans, zooms and keys through the view's own
  input paths once per display refresh, and logs frame pacing and latency.
- `make selftest`: the swift-run app and the Xcode .app each render one frame
  through the real view and must print `renderer OK`.

Metrics: *escape-n differs* = both exterior, integer escape iteration differs;
*class mismatch* = interior/exterior disagree (split by direction);
`|Δmu|` = smooth-count error over pixels exterior in both; *px-iter* = Σ n over
pixels with interior counted at maxIter (the work without interior detection);
*done-iter* = iterations the GPU actually ran (chunk counters).

## Benchmark set

| name | centre | width (complex) | maxIter | what it stresses |
|---|---|---|---:|---|
| home | −0.75, 0 | 3.9 | 1 000 | cheap baseline |
| home-100k | −0.75, 0 | 3.9 | 100 000 | ~15 % interior pixels |
| seahorse-1e5 | −0.743643887037151, 0.13182590420533 | 1.3e−4 | 2 000 | spirals, zoom ~1e5, almost no interior |
| elephant | 0.2925, 0.0149 | 0.025 | 5 000 | cardioid edge, 29 % interior |
| minibrot-p4 | −1.9407998065294847, 0 (period-4 nucleus) | 0.0045 | 50 000 | minibrot centred, 11 % interior |
| deep-5e-14 | −0.7436438870371587, 0.13182590420531198 | 1.728e−10 | 6 000 | 5e−14/px at 3456 wide, near the Double limit |
| feigenbaum | −1.4011551890920506, 0 | 2e−7 | 200 000 | high-iteration filaments, 30 % interior |

Defined in `Sources/AlmondbreadKit/BenchmarkLocations.swift`.

## Milestone 2 (chunked iteration, interior detection), 2026-10-14

Iterate-pass GPU time in ms (sum over chunks for M2). *M1* = single dispatch per
frame; *M2 off* = chunked, interior test off; *M2* = chunked, interior test on
(adaptive, see [[techniques/interior-detection]]).

| location | size | M1 | M2 off | M2 | M2 chunks | speedup M1→M2 |
|---|---|---:|---:|---:|---:|---:|
| home | 3456×2234 | 23.7 | 26.1 | 14.1 | 9 | 1.7× |
| home | 1600×1000 | 5.5 | 6.1 | 4.7 | 4 | 1.2× |
| home-100k | 3456×2234 | 2250 | 2211 | 48.4 | 13 | 46× |
| home-100k | 1600×1000 | 534 | 469 | 25.1 | 10 | 21× |
| seahorse-1e5 | 3456×2234 | 29.0 | 33.0 | 32.5 | 10 | 0.9× |
| seahorse-1e5 | 1600×1000 | 6.5 | 7.7 | 7.7 | 4 | 0.8× |
| elephant | 3456×2234 | 213 | 222 | 112 | 10 | 1.9× |
| elephant | 1600×1000 | 46.4 | 44.8 | 29.5 | 4 | 1.6× |
| minibrot-p4 | 3456×2234 | 829 | 790 | 51.6 | 12 | 16× |
| minibrot-p4 | 1600×1000 | 204 | 173 | 15.8 | 9 | 13× |
| deep-5e-14 | 3456×2234 | 185 | 193 | 196 | 19 | 0.95× |
| deep-5e-14 | 1600×1000 | 39.8 | 42.0 | 39.9 | 10 | 1.0× |
| feigenbaum | 3456×2234 | 10 387 | 10 100 | 7 996 | 511 | 1.3× |
| feigenbaum | 1600×1000 | 2 285 | 2 139 | 1 497 | 103 | 1.5× |

Wall time adds ~10 ms at 3456×2234 (texture readback, colorize, CPU setup), the
same for M1 and M2. Throughput of the plain kernel: 45–55 Giter/s of useful
iterations on M1 Pro (seahorse 30: short orbits, more divergence).

**Results are bit-identical to M1** (`--fields` diff of the full-resolution dumps,
max |Δmu| = 0 over 7.7 M pixels) for every location, with the interior test on and
off: chunking does not change the arithmetic, and the derivative test with ε = 1e−3
classified exactly the pixels that M1 ran to maxIter.

1e7 iterations, home at 800×500, interior test off: 11.4 s in 715 chunks, chunk GPU
time median 16.0 ms, p99 17.6 ms, max 70 ms (first chunk, before the rate model
adapts); no command-buffer errors. With the interior test: 83 ms (138×).

### GPU vs CPU Double oracle (400×259)

| location | escape-n differs | class mismatch (GPU-int/CPU-ext, GPU-ext/CPU-int) | median \|Δmu\| | p99 | >0.05 | CPU interior | GPU detected |
|---|---:|---|---:|---:|---:|---:|---:|
| home | 0.21 % | 0.008 % (0.004, 0.004) | 2.4e−7 | 9.8e−4 | 0.34 % | 15.4 % | 14.9 % |
| home-100k | 0.25 % | 0 | 2.4e−7 | 1.2e−3 | 0.38 % | 15.4 % | 15.3 % |
| seahorse-1e5 | 3.8 % | 0 | 4.1e−6 | 83 | 5.7 % | 0.1 % | 0 % |
| elephant | 2.5 % | 0.090 % (0.042, 0.047) | 4.8e−7 | 157 | 3.3 % | 28.6 % | 26.9 % |
| minibrot-p4 | 0.34 % | 0.004 % (0.002, 0.002) | 9.5e−7 | 6.9e−3 | 0.53 % | 11.4 % | 11.1 % |
| deep-5e-14 | 10.2 % | 0.004 % (0.003, 0.001) | 1.2e−4 | 193 | 14.9 % | 0 % | 0 % |
| feigenbaum | 54 % | 0.15 % (0.073, 0.079) | 6.2 | 6 300 | 64 % | 29.5 % | 24.4 % |

Identical for M1 and M2. Disagreements sit on the boundary (heatmaps show
filament bands, no clusters or glitch blobs). **Control:** the same CPU render with
every sample shifted by 1e−5 pixel differs from itself by >0.05 on 45 % of pixels
at feigenbaum, 5.1 % at seahorse, but only 0.7 % at deep-5e-14. So at seahorse
and feigenbaum the GPU disagreement is mostly chaotic sensitivity of boundary
pixels (any tiny perturbation of c does it), while at deep-5e-14 the 14.9 % is
real float32 error (float32 `dc`/reference rounding at 5e−14/px). See
[[questions/float32-error-long-orbits]].

### Scheduler findings

- **Float atomics are very slow.** One `atomic_float` add per SIMD-group for an
  iteration counter cost ~28 ms per million-pixel chunk (30 ms vs 2 ms). Native
  `atomic_uint` (low word + carry into a high word) is free.
- **Compaction order matters (−31 % throughput).** Appending survivors with one
  atomic per SIMD-group scrambles pixel order every chunk; after a few chunks a
  SIMD-group holds pixels from all over the image with unrelated reference
  indices `m`, and `ref[m]` reads stop coalescing. Throughput fell from 57 to
  31 Giter/s over a frame at feigenbaum; 16 ms chunks took 13.9 s vs 9.5 s with
  50 ms chunks. One atomic per threadgroup (in-group prefix sums) keeps survivors
  roughly in scanline order: 9.6 s at 16 ms, 9.4 s at 50 ms. Chunk size then barely
  matters.
- **GPU-side admission.** With the CPU admitting fresh pixels into free live-list
  capacity, a 7.7 Mpx frame with a 1 M-slot pool needed one CPU round trip per
  million pixels; moving admission into the 1-thread `prepareChunk` kernel lets
  chunks run back to back.
- **Live-list capacity.** 1 M slots (2 × 40 MB) on macOS: 256 K slots cost 30 %
  at feigenbaum, 4 M slots gained ~5 %.
- **Watchdog.** M1's single 10.4 s command buffer (feigenbaum 3456×2234) completed
  without error from a headless CLI process with the display on; see
  [[questions/metal-watchdog-limit]]. M2 never runs a command buffer much past
  ~20 ms.

### Interaction (2560×1640 px window, 120 Hz ProMotion)

`--interaction-test`, 3 s of input per scenario. Before = first chunked renderer
(hold-until-first-level-done rule, 60 fps view setting); after = M2 final.

| scenario | before fps | after fps | after interval p50 / p95 | latency p50 (input→draw + draw→glass) | GPU ms/frame | full-res final after input stops |
|---|---:|---:|---|---|---:|---:|
| home pan | 0 | 117 | 8.3 / 8.3 ms | 34 ms (6 + 28) | 4.1 + 0.8 | 156 ms |
| home zoom | 0 | 114 | 8.3 / 8.3 ms | 34 ms (5 + 29) | 3.6 + 0.9 | 153 ms |
| 1e5 pan | 0 | 117 | 8.3 / 8.3 ms | 34 ms (6 + 28) | 4.4 + 0.6 | 204 ms |
| 1e5 zoom | 0 | 113 | 8.3 / 8.3 ms | 34 ms (6 + 28) | 3.8 + 0.6 | 205 ms |
| WASD at 1e5 | – | 106 | 8.3 / 8.3 ms | 28 ms (0 + 28) | 4.5 + 0.5 | 191 ms |
| Space zoom (+E) | – | 86 | 8.3 / 16.7 ms | 37 ms | 8.8 + 0.4 | 483 ms |
| zoom into Double limit | – | 110 | 8.3 / 16.7 ms | 34 ms | 1.7 + 0.9 | 353 ms |

"Before" presented **no frame at all** during continuous input: each new view
restarted a job whose first level was not yet done, and the hold rule kept the
old image. Further findings:

- **SwiftUI invalidation throttles Metal.** A HUD reading `model.viewport` re-rendered
  (with a material blur) on every zoom event and the MTKView only got ~67 draws/s;
  publishing a throttled 10 Hz snapshot for the HUD/inspector restored 114 fps.
- **Compositor latency.** draw→glass is ~28 ms (3.4 refreshes) whatever we do:
  1 vs 2 frames in flight and 2 vs 3 drawables measured the same; counting frames
  until the presented callback halved the frame rate; `displaySyncEnabled = false`
  cut it to ~10 ms but dropped most frames. Our part (input→draw) is ≤ 1 refresh.
- Dynamic preview resolution (1/8 steps, 0.25…1 of full) from the measured cost per
  texel keeps the home view at full resolution 50–90 % of gesture frames.
- Pacing is the same with the test window in the background (not key, app not
  active, partly visible); App Nap is disabled for window tests.
- The zoom clamp at the Double limit holds with continuous zoom: centre drift 0.

## Milestone 3a (arbitrary precision, CPU), 2026-10-14

`swift run -c release almondbread-math-check bench [bits...]` (ours) and
`Tests/AlmondbreadMathTests/oracle/gmp_orbit_bench.c` (GMP 6.3 `mpf`, same iteration).
Reference-orbit iterations per second, one thread, C inside the main cardioid with all
mantissa bits populated (measured 2026-10-14, M1 Pro, release, best of 2 runs while a
parallel build loaded the machine):

| bits | ours, 2 mul + float/floatexp out | ours, 3 sqr | GMP mpf, 2 mul | GMP mpf, 3 sqr |
| ---: | ---: | ---: | ---: | ---: |
| 64 | 16.2 M | 12.8 M | 10.5 M | 8.6 M |
| 128 | 13.6 M | 11.7 M | 9.8 M | 8.0 M |
| 256 | 11.6 M | 7.9 M | 8.4 M | 6.7 M |
| 512 | 7.0 M | 5.2 M | 5.2 M | 4.4 M |
| 1024 | 2.65 M | 2.28 M | 2.44 M | 2.35 M |
| 2048 | 0.93 M | 0.97 M | 0.93 M | 0.97 M |
| 4096 | 0.26 M | 0.30 M | 0.37 M | 0.35 M |
| 16384 | 20 k | 23 k | 41 k | 39 k |

Target (>= GMP up to 1024 bits) met; GMP is up to 1.8x faster from 4096 bits. Details
and the optimisations that mattered: [[platform/mpfr-gmp-on-apple-silicon]].

Correctness (`swift test --filter AlmondbreadMathTests`, 18 tests, debug and release):
1500 random add/sub/mul/square/div/small-multiply cases at 64...4096 bits, 411 parses
and 400 formats, all bit-exact against exact Python rationals with round-to-nearest-even;
7 reference orbits (64...4096 bits, 119...5000 iterations) bit-exact for both
formulations; 8 nuclei (periods 3...8007, depths to 6e-300) within 1 bit of mpmath.
Reference selection: period 8007 nucleus at 192 bits in 20 ms, period 250 at 2048
bits in 7 ms ([[techniques/reference-selection]]).

## R1: Rust core vs Swift AlmondbreadMath, 2026-10-14

Same machine, same session, interleaved runs (Swift `almondbread-math-check bench`, Rust
`cargo run --release -p almondbread-math --example math_check -- bench`, identical
workload: C inside the main cardioid with all mantissa bits populated, float32 and
floatexp output written). Best of 3 interleaved rounds (1024 and 4096 bits: best of 4)
while another agent's builds loaded the machine (load 5 to 38), so absolute values
are below the quiet-machine M3a table; the ratios are what counts
(measured 2026-10-14, M1 Pro, release, Swift `-Ounchecked`, Rust LTO fat, rustc 1.99):

| bits | Swift 3 sqr | Rust 3 sqr | Swift 2 mul | Rust 2 mul | Rust / Swift (best form each) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 64 | 12.8 M | 46.7 M | 15.9 M | 65.0 M | 4.1x |
| 128 | 11.7 M | 25.4 M | 13.4 M | 30.0 M | 2.2x |
| 192 | 8.5 M | 20.3 M | 11.9 M | 24.8 M | 2.1x |
| 256 | 8.0 M | 19.1 M | 11.4 M | 27.2 M | 2.4x |
| 384 | 6.2 M | 8.8 M | 8.3 M | 14.8 M | 1.8x |
| 512 | 5.1 M | 6.4 M | 7.0 M | 11.6 M | 1.7x |
| 768 | 2.4 M | 3.9 M | 3.6 M | 4.3 M | 1.18x |
| 1024 | 2.27 M | 2.76 M | 2.54 M | 2.82 M | 1.11x |
| 2048 | 0.93 M | 0.97 M | 0.89 M | 0.94 M | 1.05x |
| 4096 | 287 k | 326 k | 242 k | 281 k | 1.14x |
| 8192 | 72 k | 94 k | 67 k | 80 k | 1.29x |
| 16384 | 22.4 k | 27.5 k | 19.3 k | 21.9 k | 1.23x |

Acceptance of [[decisions/rust-core]] (Rust >= Swift within noise at every precision)
met; 2048 bits is the narrowest margin. The kernel changes that got there are on
[[platform/mpfr-gmp-on-apple-silicon]].

Allocating BigFloat operators (`math_check ops` vs `almondbread-math-check ops`, same
session): 128 bits mul 55 ns (Swift 101), add 65 ns (104), div 298 ns (594), format
3 µs (14), parse 1 µs (14); 4096 bits mul 1.75 µs (1.83), div 7.6 µs (8.6), format
37 µs (170), parse 18 µs (135).

Correctness (`cargo test --workspace`, debug with overflow checks and release): all 17
ported oracle tests bit-exact on the same fixtures (1500 arithmetic cases, 411 parses,
400 formats, 7 orbits in both forms, 8 nuclei), plus property tests against exact
rationals (num-rational): add, sub, mul, square, div, small multiply and compare
correctly rounded at 64 to 4480 bits (2000 random cases per run, cancellation-heavy
operands half the time), decimal parse correctly rounded and round-tripping, sqrt within
1 ulp, and both orbit forms equal to the same iteration written with BigFloat
operators. A deliberately broken rounding step was caught. `make core-check` matches
the Rust core through the Swift bindings against AlmondbreadMath bit for bit
(coordinates, 5 orbits point by point, reference selection). FFI costs:
[[platform/rust-swift-ffi]].

## Contradictions

None with sources. Chéritat's "about 2× per iteration" for the derivative test
matched our first kernel (seahorse 30 → 66 ms); testing every 8th iteration brought
it to ~1.5× (48 ms), and the adaptive switch removes it where no interior is in view.

## Interior test `|w| n²`, unresolved pixels, extension passes (2026-10-14)

The fix for the −3/4 neck wedge (see [[techniques/interior-detection]] Pitfalls and
[[techniques/auto-iterations]]). `--bench`, best of 2, iterate GPU ms. *Before* is
the same tree with the previous kernel and renderer (built side by side, run back to
back).

| location | size | before | after | detected before → after |
|---|---|---:|---:|---|
| home | 3456×2234 | 14.7 | 17.8 | 14.9 → 14.2 % |
| home | 1600×1000 | 5.2 | 5.4 | |
| home-100k | 3456×2234 | 63.7 | 54.3 | 15.3 → 15.3 % |
| home-100k | 1600×1000 | 24.5 | 26.2 | |
| seahorse-1e5 | 3456×2234 | 32.3 | 32.8 | 0 (test off) |
| elephant | 3456×2234 | 101 | 162–177 | 26.9 → 23.3 % |
| elephant | 1600×1000 | 28.9 | 35–39 | |
| minibrot-p4 | 3456×2234 | 53.1 | 78–81 | 11.1 → 11.1 % |
| minibrot-p4 | 1600×1000 | 17.4 | 23–28 | |
| deep-5e-14 | 3456×2234 | 192 | 190–197 | 0 |
| feigenbaum | 3456×2234 | 8 334 | 14 517–14 811 | 24.4 → 15.0 % |
| feigenbaum | 1600×1000 | 1 528 | 2 474–2 503 | |

The slowdown is the price of correctness: near-parabolic interior can no longer be
"proven" in the regime where an exterior passage looks the same. GPU-int/CPU-ext
(400×259 verify) dropped to 0 at elephant (0.042 %), feigenbaum (0.073 %) and home
(0.004 %). Those had been exterior pixels painted interior, not float noise. New
regression locations (all now 0.000 % GPU-int/CPU-ext; before: neck-2e5 20.85 %,
cusp-1e5 1.69 %):

| location | size | GPU ms | CPU interior | unresolved |
|---|---|---:|---:|---:|
| neck-2e5 | 3456×2234 | 391 | 67.0 % | 67.0 % |
| neck-7e6 | 3456×2234 | 599 | 99.3 % | 99.3 % |
| cusp-1e5 | 3456×2234 | 606 | 97.8 % | 90.9 % |
| root-third | 3456×2234 | 548 | 73.1 % | 73.1 % |

Interactive pipeline at the reported views, 2560×1640, `--verify --history zoom`
(Space zoom 2 s at 1.6 e-folds/s, then one 1.3225× wheel step), extension budget 2 s.
*Complete* is the refined frame after input stopped, *resolved* is after the last
extension pass:

| view | limit (hard) | complete | resolved |
|---|---|---:|---:|
| neck 2.2e5 | 2 053–2 766 (32 848–44 256) | 0.40–0.46 s | 1.7–3.1 s |
| neck 2.9e5, after a step | 8 216 (learned; 131 456) | 0.73 s | 2.2 s |
| neck 7.1e6 (A) | 8 452 (learned; 33 808) | 1.04 s | 3.0 s |
| neck 9.4e6 (B) | 11 632 (learned; 46 528) | 1.32 s | 4.0 s |

Before, the neck at 2.2e5 completed in 0.2 s, with the wedge. A first version that
continued unresolved pixels inside the level took 2.9 s to complete (live-list
starvation, [[platform/gpu-watchdog-and-chunking]] Pitfalls). The final frames of
these sessions are identical in class to a fresh headless render (0.000 %).
`--interaction-test` (60 Hz display) with the new "neck zoom" scenario: 58.5 fps,
final frame 407 ms after input, shown frame vs fresh render 0 pixels differing.

## R2a: Rust renderer vs Swift renderer, 2026-10-14

The Rust scheduler and Metal backend ([[platform/render-architecture]]) against the Swift
`Renderer`, same machine, same session: three interleaved rounds of
`AlmondbreadMac --bench --out dir` and `almondbread bench --out dir` (each best of 2 after a
warm-up), load average 9–17 from other work. Iterate GPU ms (measured 2026-10-14, M1 Pro,
release, Rust LTO fat):

| location | size | Swift best / median | Rust best / median | Rust / Swift (best) |
|---|---|---:|---:|---:|
| home | 3456×2234 | 17.9 / 18.0 | 17.8 / 18.0 | 0.99 |
| home | 1600×1000 | 5.4 / 5.4 | 5.3 / 5.5 | 0.98 |
| home-100k | 3456×2234 | 56.1 / 56.2 | 53.1 / 54.3 | 0.95 |
| home-100k | 1600×1000 | 26.1 / 26.1 | 26.7 / 26.7 | 1.02 |
| seahorse-1e5 | 3456×2234 | 33.3 / 34.8 | 32.4 / 33.4 | 0.97 |
| seahorse-1e5 | 1600×1000 | 7.6 / 7.8 | 7.6 / 7.6 | 1.00 |
| elephant | 3456×2234 | 158.4 / 159.0 | 156.9 / 162.2 | 0.99 |
| elephant | 1600×1000 | 34.7 / 34.8 | 33.9 / 36.4 | 0.98 |
| minibrot-p4 | 3456×2234 | 76.5 / 77.2 | 79.8 / 80.6 | 1.04 |
| minibrot-p4 | 1600×1000 | 20.9 / 21.9 | 21.0 / 21.9 | 1.00 |
| deep-5e-14 | 3456×2234 | 191.9 / 193.0 | 189.2 / 193.6 | 0.99 |
| deep-5e-14 | 1600×1000 | 40.7 / 40.9 | 38.8 / 40.3 | 0.95 |
| feigenbaum | 3456×2234 | 14 212 / 14 230 | 14 299 / 14 313 | 1.01 |
| feigenbaum | 1600×1000 | 2 443 / 2 466 | 2 457 / 2 474 | 1.01 |
| neck-2e5 | 3456×2234 | 387.7 / 391.0 | 388.4 / 389.2 | 1.00 |
| neck-2e5 | 1600×1000 | 79.6 / 80.3 | 79.9 / 80.7 | 1.00 |
| neck-7e6 | 3456×2234 | 592.9 / 603.9 | 598.2 / 603.1 | 1.01 |
| neck-7e6 | 1600×1000 | 123.0 / 124.3 | 123.5 / 125.4 | 1.00 |
| cusp-1e5 | 3456×2234 | 594.2 / 605.8 | 608.9 / 610.2 | 1.02 |
| cusp-1e5 | 1600×1000 | 124.8 / 125.1 | 125.2 / 125.5 | 1.00 |
| root-third | 3456×2234 | 543.8 / 551.0 | 556.8 / 558.2 | 1.02 |
| root-third | 1600×1000 | 111.7 / 113.2 | 112.7 / 113.6 | 1.01 |

Geometric mean Rust / Swift 0.998; every location within ±5 %, the run-to-run spread of
either renderer. Chunk counts differ by one or two in some rows (K follows measured GPU
time); results do not.

**Iteration fields are bit-identical.** `almondbread verify --fields rust.iter --against
swift.iter` (which also counts bitwise-differing texels): 0 differing texels in all 33 dumps
of round 1 (11 locations × 3456×2234, 1600×1000 and the 400×259 verify grid) and in the
22 full-resolution dumps of rounds 2 and 3. The final PNGs and diff heatmaps are
pixel-identical, and the GPU-vs-CPU verify tables are identical text in every round (so
the CPU oracle, rayon-parallel in Rust, matches the Swift one too). Extension passes
(list collection, list admission, ×4/×2 passes), not exercised by `--bench`: offscreen
`verify --extend 16` at the three regress views and the neck at 3456×2234, 0 differing
texels in each.

**Scripted interactive sessions** (`almondbread regress` = `make regress` views): all pass
the same guards (no exterior painted interior: 0.000 %; final frame vs fresh render 0.000 %
class mismatch; never a preview). Rust vs Swift for the two neck sessions: complete
184/184 ms and 200/202 ms, learned limits 8 472 and 11 928/11 924, unresolved counts within
0.5 %. The sessions are paced in real time, so final views and timing-driven decisions vary
run to run. One is skewed, not random: in the seahorse session the final level ran the
interior test in 1 of 10 Rust runs and 6 of 10 Swift runs. Traces (`ALMONDBREAD_TRACE=1`)
show the same decisions from the same inputs, but different measured GPU times for
identical gesture chunks: Swift's small previews ran at times at ~8e9 iterations/s
(147 840 texels, 5.1e7 iterations, 6.4 ms) against ~2.9e10 in Rust, so its cost per
texel rose, the dynamic preview resolution settled at 0.25 instead of 0.5–0.625, and the
interior-test switch, which reads the last preview's statistics, flipped more often. Both
outcomes pass the guards. Whether the app sees the same effect is for the R2b interaction
test.

**Interaction (Swift, for the record):** `--interaction-test` "neck zoom" draw CPU p50
9.5 ms (60 Hz display) is blocking in `currentDrawable` (8.9 ms), not CPU work;
frames-in-flight 1 removes it but costs frame rate on heavy views. Analysis and the Rust
design answer on [[platform/render-architecture]]. Interaction parity of the Rust renderer
is measured in R2b, when the app hosts it.
