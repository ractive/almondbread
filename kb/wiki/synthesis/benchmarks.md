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

Since R2b the headless tools are the Rust CLI (`target/release/almondbread`, `make
rust-cli`); the Swift `AlmondbreadMac --bench/--verify/--render` that older sections used
are deleted, and the CLI prints the same lines, so tables compare 1:1.

- `almondbread bench [--sizes WxH,...] [--interior 0|1] [--chunk-ms N] [--out dir]`
  (`make bench`) renders the fixed set below at 3456×2234 and 1600×1000 (best of 2 after
  a warm-up), then GPU-vs-CPU verifies each location at 400×259. `--out` writes PNGs,
  diff heatmaps and raw `.iter` dumps (n, smooth offset per pixel).
- `almondbread verify --center re,im --scale s --size WxH --iterations N`:
  one GPU render vs the CPU oracle (f64, direct iteration from z = 0, no perturbation,
  rayon over rows; same grid, bailout 2¹⁶ and smooth formula). `--fields a.iter
  --against b.iter` diffs two dumps, e.g. before/after a renderer change.
- `AlmondbreadMac --interaction-test` (`make interaction-test`): opens the real window in
  the background (1280×820 pt = 2560×1640 px; since R2b in the bottom-right corner of its
  screen, still rearmost, so presentation is measurable), drives pans, zooms and keys
  through the view's own input paths once per display refresh, and logs frame pacing,
  latency, draw CPU and the per-frame FFI overhead.
- `make selftest`: the swift-run app and the Xcode .app each render their first view
  through the real view and the Rust renderer, check it against a fresh render, write a
  PNG and must print `renderer OK (Rust)`.

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
is measured in R2b, when the app hosts it (section below).

## R2b: the apps on the Rust renderer, interaction parity, 2026-10-14

`--interaction-test`, Swift renderer (HEAD de0aa8e built in a worktree, test window placed
like the Rust build's) vs Rust renderer through the `Explorer` FFI, three interleaved
rounds each, all with the window partly visible (60 Hz display, 2560×1640 px, M1 Pro,
release). Medians of the three rounds; draw CPU p50 with its range:

| scenario | fps Swift / Rust | interval p95 ms | latency p50 ms | latency p95 ms | draw CPU p50 ms (range) | draw CPU p95 ms | skipped |
|---|---|---|---|---|---|---|---|
| home pan | 58.7 / 58.3 | 16.7 / 16.7 | 49.4 / 49.4 | 49.5 / 49.4 | 0.10 (0.09–0.11) / 0.09 (0.08–0.72) | 0.19 / 0.13 | 0 / 0 |
| neck zoom | 58.7 / 59.0 | 16.7 / 16.7 | 65.1 / 65.2 | 65.4 / 65.4 | 8.61 (0.11–8.73) / 0.91 (0.84–1.04) | 9.50 / 1.20 | 0 / 0 |
| wasd 1e5 | 53.7 / 53.7 | 16.7 / 16.7 | 40.6 / 32.8 | 41.6 / 32.8 | 0.09 / 0.07 | 0.16 / 0.11 | 0 / 0 |
| space zoom (+E) | 55.3 / 56.0 | 16.7 / 16.7 | 40.6 / 32.8 | 57.3 / 49.5 | 0.17 / 0.14 | 8.89 / 1.09 | 8 / 7 |
| home zoom | 58.7 / 59.0 | 16.7 / 16.7 | 49.4 / 49.3 | 49.5 / 49.4 | 0.09 / 0.08 | 0.17 / 0.13 | 0 / 0 |
| 1e5 pan | 58.7 / 59.0 | 16.7 / 16.7 | 49.4 / 49.4 | 49.5 / 49.4 | 0.08 / 0.08 | 0.14 / 0.11 | 0 / 0 |
| 1e5 zoom | 58.7 / 59.0 | 16.7 / 16.7 | 49.4 / 49.3 | 49.5 / 49.4 | 0.09 / 0.08 | 0.17 / 0.13 | 0 / 0 |
| zoom into the precision limit | 58.7 / 58.3 | 16.7 / 16.7 | 49.4 / 65.3 | 49.5 / 65.5 | 0.07 / 0.82 | 0.18 / 1.09 | 0 / 0 |

Full-resolution final frame after input stopped (ms, three rounds): neck Swift 341 / 392 /
341, Rust 399 / 416 / 416; 1e5 pan 209–226 vs 233–250; home pan and zoom 192 vs 200;
WASD 200–217 vs 200–217; Space 466–483 vs 450–483; limit 359–376 vs 366. Shown frame vs
fresh render: 0.000–0.001 % class mismatch in all 48 checks; WASD centre moved 0.851,
0.424 view widths with no keys held after release (identical to Swift); the precision
clamp holds with centre drift 0.

Findings:

- **Frame rate, interval and latency at parity or better.** Key-driven scenarios gain a
  refresh of latency in Rust (WASD and Space p50 32.8 vs 40.6 ms): draw→glass is a
  refresh shorter, input→draw a half refresh longer (the view's display link runs before
  the test's in each refresh). Limit zoom lost a refresh of latency in this set (65.3 vs
  49.4 ms), where the acquire waits ~0.8 ms; an earlier set measured 49.4 ms for both.
- **The 9.4 ms drawable stall is gone** (draw CPU p50 8.6–8.9 ms → 0.8–1.0 ms in neck
  zoom, p95 ≤ 1.2 ms everywhere vs up to 9.5 ms). Two corrections to the R2a design were
  needed (strict pool count 36–38 fps; gate refinement → final frame +50 ms), see
  [[platform/render-architecture]].
- **Final frame is 10–25 ms later** in some scenarios (the neck at 400–416 ms vs Swift's
  bimodal 341/392). The refinement is chunk-for-chunk the same work (neck: 1.12e11 vs
  1.13e11 iterations, 2862 vs 2835 GPU ms over the scenario, same K sequence); two
  scheduling fixes closed most of the initial 50 ms (`frame` uses the current media time,
  not the last-vsync timestamp, +15 ms; the final frame may wait for a drawable).
- **Seahorse interior-switch skew of R2a does not appear in the app.** Traces of "1e5
  zoom" and "neck zoom": both renderers choose the 0.25 preview floor for every gesture
  frame, with the same median chunk rate (1.32e10 vs 1.36e10 iterations/s at the seahorse,
  3.70e10 vs 3.66e10 at the neck). The R2a difference (Swift's small previews at ~8e9 it/s
  in a headless 120 Hz session at 1280×820) came from the headless driver, not from the
  renderers' decisions. The GPU-ms-per-frame column differs (neck 9.6 vs 11.4) only by
  attribution: chunks counted inside the measuring window.
- **FFI cost of the frame call** (Swift-side minus inside-Rust time): 2–7 µs per frame in
  the window. `input()` 0.9 µs, `stats()` 6 µs ([[platform/rust-swift-ffi]]).
- **Occluded test windows measure nothing.** With the window fully covered, presented
  handlers report no times for either renderer (12 such rounds discarded). The test now
  places its rearmost window in the screen corner and warns when nothing was presented.

## M2.5: pointer and keyboard modes, 2026-10-14

`--interaction-test` at **120 Hz** (the display now runs ProMotion; the R2b set above
was 60 Hz), 2560×1640 px, M1 Pro, release. Three interleaved rounds of the Swift renderer
(de0aa8e), Rust HEAD before M2.5 (b8fba3c) and M2.5, plus two final M2.5 rounds after the
FFI fix below. Medians; latency p50; full-resolution final frame after input stops (ms,
per round):

| scenario | fps Swift / Rust / M2.5 | latency p50 ms Swift / Rust / M2.5 | final frame ms Swift / Rust / M2.5 |
|---|---|---|---|
| home pan | 116 / 113 / 111 | 33–40 / 32.7 / 32.7 | 153–183 / 158–183 / 158–183 |
| neck zoom | 83 (56–93) / 76 / 76 | 41–58 / 49.4 / 49.4 | 354, 354, 625 / 342–350 / 342–357 |
| wasd 1e5 | 105 / 102 / 102–104 | 36.6–44.9 / 24.5 / 24.5 | 192–214 / 183–192 / 176–192 |
| space zoom (+E) | 86 (66–106) / 80 / 80 | 28–50 / 32.8 / 32.8 | 192–473 / 467–483 / 458–467 |
| home zoom | 115 / 109 / 109–111 | 33–58 / 32.7 / 32.7 | 154–180 / 158 / 158 |
| 1e5 pan | 117 / 116 / 114 | 39–50 / 32.7 / 32.7 | 196–245 / 200–208 / 200–208 |
| 1e5 zoom | 117 / 105 / 103 | 32.8 / 32.8 / 32.8 | 196 / 200 / 200 |
| limit zoom | 110 / 110 / 108–109 | 33–40 / 32.8 / 32.8 | 354 / 350–367 / 350–367 |
| pointer space (new) | — / — / 94–96 | 24.5 | 183–200 |
| keyboard glide (new) | — / — / 92–94 | 24.5 | 192–208 |
| edge pan (new; 0.5 s idle by design) | — / — / 76–78 | 24.5 | 175–221 |

New checks (all rounds identical):

- **Fixed point:** Space held 2.5 s in pointer mode with the pointer at (380, −200) pt
  (×146 zoom from 1e5): the coordinate under the pointer moved **0 px**. Keyboard mode
  after the glide, 2 s at the centre: 0 px. Criterion < 1 px per second.
- **Glide:** crosshair from (380, −200) pt to the centre: 1 % left after **267 ms** (design
  265 ms), snapped onto the centre after 508 ms, no overshoot, distance monotonic.
- **Edge pan:** pointer 8 pt inside the right edge: **0.381 view widths/s** while Space is
  held (expected 0.4 × smoothstep = 0.381), no motion in the 0.5 s before Space ("while
  zooming"), 0.00 pt after the pointer left the window.
- Shown frame vs fresh render: 0.000 % in all 22 checks of the final rounds.

Findings:

- **M2.5 costs nothing per frame** once the overlay state is scalars: fps, latency and
  final frames match Rust HEAD within noise. A first version returned a UniFFI record
  from `frame` (mode + crosshair): FFI overhead per frame rose from 1.7–3 µs to 11–18 µs
  in the window (windowless core-check 0.18 → 1.64 µs), since records are serialised
  through a heap buffer on every call. Now `frame` returns bit flags and the crosshair
  position is two scalar calls made only in keyboard mode: 1.7–2.8 µs again (core-check
  0.12 µs).
- **Idle `frame` calls no longer re-present.** Rust HEAD re-coloured and presented the
  unchanged final image on every call while the link ran (core-check idle frame 0.198 ms
  inside Rust); now 0.1 µs. It matters for the crosshair glide, which keeps the link
  running over an idle image.
- **Motion from rest started with a jump** (fixed): the first frame after a key press
  from idle integrated up to 0.1 s (the dt cap) from the last frame before the link
  paused, i.e. 4 % of the view width for a pan key. The measured glide was 175 ms to 1 %
  and edge pan 6 % fast until the navigator integrated from the event's time. The WASD
  scenario now moves 0.821 instead of 0.851 view widths.
- **R2b's "final frame 10–25 ms later" and limit-zoom latency do not reproduce at
  120 Hz.** Final frames are at parity or earlier (neck 342–357 vs Swift 354/354/625; Swift
  bimodal in Space zoom 192–473), limit-zoom latency 32.8 ms vs Swift 33–40. Both
  renderers poll chunk completion once per refresh and wait for a harvest before the next
  level starts (identical code paths); with no reproducible gap there was no scheduling
  fix to make.
- **Open: gesture fps on heavy zooms at 120 Hz.** Swift reaches more frames in 1e5 zoom
  (117 vs 103–105) and neck zoom (83 vs 76, Swift varying 56–93); Rust skips 12–21
  refreshes as "no drawable" in 1e5 zoom where Swift skips none. Latency is equal or
  better in Rust. Likely the drawable-pool prediction of [[platform/render-architecture]]
  (presented callbacks lag a refresh, 8 ms here) is conservative at 120 Hz; not part of
  M2.5, see [[roadmap]] FL.

## FL: feel and look, 2026-10-14

`--interaction-test`, 2560×1640 px window, M1 Pro, release. **60 Hz**: with the external
60 Hz display attached, the background test window on the 120 Hz panel gets 60
display-link callbacks per second whatever `preferredFrameRateRange` asks
([[platform/render-architecture]] FL), so M2.5's 120 Hz rows are not directly comparable.
Before = HEAD a54749b (M2.5) with the test placed on the fastest screen, one round; after =
FL, two rounds. Final frame = all pixels exact; settled = faded in plus progressive AA
(9 samples, adaptive) done. The after-look has DE lighting on by default (the before-look had
no DE, ~30–50 % cheaper iterations), so after-rows do more work per texel.

| scenario | fps before / after | interval p95 ms before / after | latency p50 ms before / after | final frame ms before / after | settled (AA) ms after |
|---|---|---|---|---|---|
| home pan | 58.0 / 57.7–58.3 | 25.0 / 16.7–25.0 | 47.8 / 45.0–48.4 | 200 / 200 | 567 |
| neck zoom | 53.7 / 56.7–58.3 | 25.0 / 25.0 | 64.2 / 48.2–53.1 | 517 / 433–450 | > 3000 (extension passes first) |
| wasd 1e5 | 53.7 / 58.7–59.0 | 25.0 / 25.0 | 22.6 / 27.8–31.2 | 200 / 200–233 | 584–617 |
| space zoom (+E) | 50.0 / 53.3–54.0 | 33.3 / 25.0 | 30.7 / 31.0–35.9 | 483 / 500–517 | 2250–2300 |
| home zoom | 59.0 / 58.3–59.0 | 25.0 / 25.0 | 46.9 / 43.0–47.9 | 200 / 217 | 583 |
| 1e5 pan | 59.0 / 56.0–59.0 | 25.0 / 25.0 | 46.7 / 42.8–47.5 | 250 / 233 | 767–800 |
| 1e5 zoom | 57.3 / 57.7–58.7 | 25.0 / 25.0 | 46.4 / 42.4–43.1 | 250 / 233–250 | 734–800 |
| limit zoom | 58.3 / 21.0–21.7 * | 16.7 / 25.0 | 37.9 / 50.4–55.4 | 367 / 417 | 1717 / > 3000 |
| pointer space | 54.0 / 56.7–58.0 | 25.0 / 25.0 | 29.4 / 25.2–30.1 | 217 / 217–250 | 683–717 |
| keyboard glide | 54.0 / 56.7–58.3 | 25.0 / 16.7–25.0 | 29.2 / 25.0–29.8 | 217 / 267 | 867–1000 |
| edge pan | 43.3 / 48.3–48.7 | 25.0 / 25.0 | 28.9 / 28.8–29.5 | 200 / 217 | 800–817 |

\* At the f64 zoom clamp the view does not change, so FL presents nothing new (draws 169–176,
encoded 64–66); M2.5 re-presented frames whose key differed by a rounding step. Not a stall
(interval p95 25 ms).

Checks: shown frame vs fresh render 0–0.001 % in all 22 checks; zoom-target drift 0 px
(pointer space, keyboard glide after the tapped key's pan has glided out); glide 1 % after
266 ms, no overshoot; edge pan 0.358 view widths/s (expected 0.364 with the eased start),
glide after the pointer left 53.3 pt (expected 54.2). FFI 3.1–3.3 µs per frame.

**120 Hz drawable fix** (pre-acquisition thread, [[platform/render-architecture]] FL), one
round each at 60 Hz before the rendering changes: neck zoom 53.7 → 58.7 fps with "no
drawable" skips 16 → 2, space zoom 50.0 → 55.0 fps (26 → 12 skips, now mostly GPU frames in
flight). Final FL rounds: "no drawable" 0 in 9 of 11 scenarios, 8–10 in space zoom and one
1e5 pan round.

**Progressive AA** (adaptive list, 8 ms chunks, two in flight, 1.5 GPU-s per view):
settled 0.57–0.8 s after the last input in the shallow scenarios (refined frame 0.2–0.25 s,
fade 4τ = 0.12 s, then samples one per refresh); headless home view 0.42 s from a cold
start. Before pipelining and the adaptive list: 0.95–1.2 s, and not within 3 s at 1e5+.
Views whose unresolved pixels need extension passes (neck, limit) spend up to 2 GPU-s on
those first.

**Gesture budget**: half the measured refresh. At 60 Hz the seahorse dive's previews averaged
1500 px wide in its first 5 s (M2.5: 750 px, fixed 6 ms budget); 70 % of a refresh measured
49 fps in home zoom, 50 % gives 58–59.

**Flight recorder** (`almondbread fly`, 1920×1080, 60 Hz refresh, real-time pacing, headless):
review flights in `review/` (gitignored). Presented frames per second over the whole flight
include idle stretches where nothing new is shown: seahorse dive 47.1 (60.0 while flying),
filament cruise 48.7, gestures 43.0, fast dive 37.5. The filament cruise at 1e6
(−1.25066 + 0.02012i) is heavy: previews stay at ratio 0.25 (480 px) and the temporal
accumulation cannot fully converge at 13 px of pan per frame; it is visibly softer while
moving than when settled. A converged rendering of the same seahorse camera path (3×3 grid,
extension passes) took 8.5 min headless.

## M4: BLA (2026-10-14)

Measured 2026-10-14, M1 Pro (16-core GPU), macOS 26, release. The machine had other GPU
load during parts of this session (the user's app, another agent): timings below are from
interleaved runs (`--repeat 3`, two rounds, minimum), single runs varied by up to 2×.
Design and analysis: [[techniques/bla]] (As built).

**New benchmark location** `dense-5e-14` (−1.4080321340304542 + 0.13680236432900417i, width
7.384e−11, maxIter 5791): the user's "everything is slower here" view. 1600×1000: 4.61e9
iterations, 84 ms (unloaded); seahorse at the same depth needs about half the iterations.
Verify 400×259: escape-n differs 29.9 %, median |Δμ| 0.064, >0.05 on 53 % (float32 against
the f64 oracle; every pixel is a chaotic boundary pixel), GPU-int/CPU-ext 0.014 %.

**Default (`auto`) = before.** The forecast rejects the table at every benchmark location,
so the plain pipeline runs: iteration fields bit-identical to the pre-M4 renderer at all 11
locations (1600×1000 dumps, `verify --fields`), verify table unchanged row for row, and GPU
time equal within noise (dense-5e-14 84 vs 84 ms; deep-5e-14 38–40 vs 37–40 ms).

**Forced BLA (`--bla 1`)** at the reachable depths, 1600×1000 GPU ms off → on:

| location | off | on | steps / iterations on |
|---|---:|---:|---:|
| dense-5e-14 | 84 | 150–165 | 0.85 |
| home | 4–5 | 5.5 | – |
| seahorse-1e5 | 6.5 | 8.5 | – |
| elephant | 36 | 62–65 | – |
| minibrot-p4 | 22 | 28 | – |
| deep-5e-14 | 37–39 | 54 | – |
| neck-2e5 | 82 | 102–145 | – |
| cusp-1e5 | 128 | 158–223 | – |

(First kernel version, a lookup before every step: dense-5e-14 216–280 ms; 1.43× fewer
steps, about 4× the cost per step.) Verify with forced BLA: identical to `off` except
deep-5e-14 (escape-n differs 10.20 → 10.23 %, GPU-int/CPU-ext 0.003 → 0.005 %) and
feigenbaum (class mismatch 0.079 → 0.087 %); dense-5e-14 30.30 → 30.26 % escape-n differs,
median |Δμ| 0.0667 → 0.0662.

**Depth sweep around the dense view's reference** (f64 orbit of the centre; beyond ~1e−15/px
not the true set, but the same orbit for both runs), 1600×1000, maxIter 12 000, GPU ms
off → on (steps off → on):

| scale/px | off | on | steps | forecast (iterations / blocks) |
|---|---:|---:|---|---|
| 4.6e−14 | 95 | 166 | 4.62e9 → 3.92e9 | 6 / 2: off |
| 1e−18 | 206 | 286 | 1.06e10 → 4.15e9 | 250 / 6: off |
| 1e−20 | 202 | 82 | 1.06e10 → 1.29e9 | 1762 / 7: on |
| 1e−24 | 199 | 4.2 | 1.06e10 → 5.7e7 | on |

Skip-on vs skip-off fields at 1e−20/px, 400×260 (`verify --dump`, `--fields`): slow view max
|Δμ| 4.8e−7 (with DE as well), neck 2.4e−7 (12.1 → 0.63 ms), Feigenbaum identical
(155 → 1.85 ms), cardioid and minibrot interiors 100 % interior both ways; no class change
anywhere.

**Table build** (f64, single thread): 5.8 k points 0.11–0.22 ms, 50 k 1.1 ms, 200 k 4.0 ms;
24 B per entry, about one entry per reference point (200 k: 4.7 MB). Inline below 16 k
points, on a thread above; built only when a view's largest |dc| is below ε max|2Z|.

**Regressions:** `make regress` has two new sessions at dense-5e-14 (auto, and `--bla 1`):
shown frame = fresh render bit for bit in both. Their false-interior limit is 0.1 %: float32
alone paints 0.054–0.057 % of CPU-exterior pixels interior there (escapes just past the
limit with the interior test switched off), the same with and without BLA. Metal tests
`bla_slow_view` (auto bit-identical to off; forced: no class change, median |Δμ| < 0.01) and
`bla_deep_view` (1e−20: auto takes the table built on a thread, ≥ 4× fewer steps, max |Δμ|
< 1e−4).

**Interaction** (`make interaction-test`, 60 Hz): auto never builds a table in these
scenarios. Under the session's GPU load fps varied run to run (neck zoom 43.7–57.0, space
zoom 43.7–47.0, 1e5 zoom 52.4–58.7) with `ALMONDBREAD_BLA=0` and `auto` interleaved and
indistinguishable; shown vs fresh ≤ 0.001 % in every scenario; FFI 4.3–6.2 µs per frame.

## L1: real-time landscape prototype, 2026-10-14

`prototypes/landscape-rt`, M1 Pro, release. Window: `fly --script NAME --background`, a
1280×800 pt (2560×1600 px) window on the 120 Hz panel, behind other windows, App Nap off;
pacing from `presentedTime`. Default configuration: 1024² × 11 levels, K = 2, 4× MSAA, DE
budget 2.5 GPU-ms per frame, chunk 1500, maxIter 20 000. Final round with no other GPU-heavy
process running:

| script | fps presented | missed refreshes | frame interval p95 / p99 ms | GPU render p50 / p99 ms | GPU DE p50 / p99 ms | CPU frame p50 / p99 ms |
| --- | --- | --- | --- | --- | --- | --- |
| hover (6 s) | 119.3 | 4 | — | 5.0 / 7.0 | — | — |
| cruise (12 s, S-turns) | 119.4 | 7 | 9.6 / 11.3 | 4.4 / 7.2 | 0.8 / 2.0 | 0.5 / 2.8 |
| turns (8 s, full turns) | 119.6 | 3 | 9.7 / 10.6 | 4.3 / 7.4 | 0.8 / 2.3 | 0.4 / 2.4 |
| dive (12.5 s, 1e-3 → 7.5e-7) | 117.9 | 26 | 9.8 / 14.4 | 4.0 / 9.2 | 1.0 / 2.6 | 0.3 / 2.2 |
| tour (35 s, 1e-3 → 8.6e-8) | 118.4 | 54 | 12.1 / 14.5 | 3.5 / 6.8 | 1.4 / 2.9 | 0.6 / 2.5 |

History of the misses: with non-resumable DE tiles, single 16–90-tile batches took 7–12 ms
(slow pixels to maxIter); tour 117.7–119.1 fps, DE p99 6.5–8 ms. With resumable tiles, DE p99
is ≤ 3 ms. Runs while another agent's `AlmondbreadMac --interaction-test` and a browser video
were using the GPU: 109–115 fps, with render p95 10–15 ms even when hovering (static scene),
so those are external.

Offscreen recorder (`record`, 1920×1080, 60 fps, budget = live × 2): render 3.0–3.6 ms p50,
DE 2.4–3.1 ms p50. Visible DE backlog (missing tiles within ±60° and 2 fog distances) in the
dive script: 2048² data 15 k p50 / 32 k p95; 1024² data 0.4 k p50 / 5.9 k p95.

Temporal stability. Pop check: same camera, previous vs new world, mean |diff| 0..255:
level flight 0.0002 p50 / 0.08 max. Dive with σ = 0.1 S: 2.9 p50 every frame. Layered σ:
0.6–1.0 p50 and 6.2 max in transitions ([[decisions/landscape-mode]] L1). Hover frame
difference 0.000. L0's dive video (640×360 at 30 fps): first difference 17.5, second 29.3
(second > first: per-frame noise). L1's videos have second/first ≈ 1.45–1.57 at 60 fps,
dominated by fast low-altitude motion over fine texture, with no spike frames. The pop check
is the meaningful number for L1.

## L2: landscape detail and robustness, 2026-10-15

`prototypes/landscape-rt`, M1 Pro, release; L1 build (with the L2 measurement harness only)
and L2 build interleaved, two repeats each. Other agents' GPU jobs and a user's landscape-rt
window ran throughout: GPU times are noisy (a trivial shader measured 3.8–4.2 ms per frame).
Recorder: 1920×1080, 60 fps, DE budget 5 GPU-ms per video frame (live 2.5 ms × 2), scripts
`low` (valley-seeking pilot, eye 0.065–0.08 S, S = 1.76e-6, seahorse), `zoomlow` (zoom 300×
from 5e-4 while descending, then `low`), `target` (manual dive to 1e-7), `LRT_MASK=fallback`.

| measurement | L1 | L2 |
| --- | --- | --- |
| `low`: tiles completed/s | 2 406, 2 672 | 5 047, 4 259 |
| `low`: in-view backlog p95 / max | 4 529 / 4 882, 2 879 / 3 344 | 4 / 56, 6 / 83 |
| `low`: all missing tiles p95 | 10 505, 7 375 | 4 329, 5 231 |
| `low`: no-data px max; ≥2 levels coarse max | 0.007 %; 6.4 %, 5.8 % | 0; 0, 0.03 % |
| `low --budget-ms 0.4`: tiles/s | 384, 396 | 1 020, 627 |
| same: no-data px p50 / max | 10.2 % / 45.4 % (both) | 0 / 0 |
| `zoomlow`: S after 40 s | 7.5e-6, 5.7e-6 | 1.26e-6, 1.26e-6 |
| `target`: time to 1e-7; no-data px p99 / max | 39.6 s, 40.5 s; 60 % / 99.9 % | 28.5 s; 0 / 0 |
| `bench` cold prefill (360 k tiles, 6 locations) | 82 k, 86 k tiles/s | 98 k, 109 k tiles/s |
| pop check hover / cruise / low, max mean diff | 0.005 / 0.033 / 0.72 | 0.010 / 0.112 / 0.65 |
| recorder render p50, `low` | 5.5, 5.1 ms | 4.8, 4.2 ms |

Window (`fly --background`, 2560×1600, 120 Hz): `low` L1 106.5 / 111.9 fps, L2 111.3 / 106.1;
`dive` L1 114.4 / 106.3, L2 88.3 / 112.5 (L2 zooms to ~1e-6 in the 13 s, L1 to ~1e-4).
Render p50 4.0–5.9 ms both; p99 16–42 ms both under this load. Earlier, with less load:
hashed detail noise 12.9 ms (L1 6.5), texture noise + 256² rings 6.8 ms (L1 6.5–6.6), no
detail 5.8 ms. Details: [[decisions/landscape-mode]] L2.

## M3b: unlimited depth, 2026-10-14

Measured 2026-10-14, M1 Pro, release, with the user's app and another agent running (GPU
timings vary ±50 % between runs; best of 3 given). Design: [[techniques/extended-range-floats]]
"As built (M3b)", [[techniques/bla]] "M3b", [[platform/render-architecture]] "M3b".

**Deep benchmark locations** (`almondbread locate`; `DEEP_LOCATIONS` in
`crates/almondbread-core/src/locations.rs`): `sea-1e20` (seahorse valley, 1e-20/px),
`sea-8007` (the period-8007 minibrot near the seahorse point, 2e-34/px), `needle-1e94`,
`needle-1e305`, `needle-1e1004` (needle minibrots of periods 79, 255, 834 near −2),
`dendrite-1e50/1e300/1e1000` (the Misiurewicz point c = i, self-similar at every depth).
`deep-5e-14` and `dense-5e-14` now use big references (below 1e-12 per point).

**Correctness.** The deep CPU oracle (`cpu_render_deep`: centre reference at view bits + 64,
f64 rescaled perturbation, rebasing, no BLA) agrees with direct BigFloat iteration of every
pixel tested (384/384 escape counts, max |Δμ| 1.2e-7 = float32 storage of s) at 1e-50,
1e-300, 1e-1000 (c = i) and at the period-79 needle minibrot (test
`deep_oracle_matches_direct_iteration`). GPU against it (`bench --sizes 1600x1000`, verify
400×259, interior test off as in bench):

| location | escape-n differs | class mismatch | GPU-int/CPU-ext | median |Δμ| | >0.05 |
|---|---:|---:|---:|---:|---:|
| deep-5e-14 | 8.9 % | 0.005 % | 0.002 % | 6.9e-5 | 13.1 % |
| dense-5e-14 | 12.7 % | 0.010 % | 0.005 % | 5.8e-3 | 24.5 % |
| sea-1e20 | 10.5 % | 0 | 0 | 1.7e-4 | 15.6 % |
| sea-8007 | 23.3 % | 0.053 % | 0 | 9.7e-2 | 33.1 % |
| needle-1e94 | 0.05 % | 0 | 0 | 9.5e-6 | 0.12 % |
| needle-1e305 | 0.11 % | 0 | 0 | 2.3e-5 | 0.18 % |
| needle-1e1004 | 0.09 % | 0 | 0 | 1.4e-4 | 0.17 % |
| dendrite-1e50 / 1e300 / 1e1000 | 0.004–0.005 % | 0 | 0 | 2–5e-7 | 0.01 % |

Escape-n differences are the chaotic boundary pixels float32 cannot resolve (the M2 caveat,
[[questions/float32-error-long-orbits]]); they are independent of BLA and of the reference
(at the sea view 1.36e-31/px, 1280×820: class mismatch 0.264 % with BLA, 0.270 % without,
0.266 % with a centre instead of a nucleus reference, all GPU-ext/CPU-int ⇄ GPU-unresolved/CPU-ext
symmetric). Smooth views (needle, dendrite) match to 1e-5 or better at every depth, also at
1e-2900 per pixel (c = i, 300×200: median 4.8e-7).

**`make regress`** adds three sessions through the interactive scheduler: Space zoom
1.4e-10 → 2e-31 per pixel in 8 s (crosses the f64 reference limit, the plain→deep kernel
switch and two background references; vs oracle class mismatch 0.136 %, GPU-int/CPU-ext 0.063 %,
threshold 0.15 % as for the float32 noise above; vs fresh render 0.039 %), and zoom and pan at
1e-305 per pixel around `needle-1e305` (vs oracle mismatch 0.002 % / 0.001 %, vs fresh 0).
All 8 sessions pass.

**GPU time at 1600×1000, BLA off → auto** (`render --bla 0` vs default; ms; "fewer steps" =
iterations / loop steps):

| location | maxIter | off | auto | speed-up | fewer steps |
|---|---:|---:|---:|---:|---:|
| sea-1e20 | 20 000 | 478 | 270 | 1.8× | 6.3× |
| sea-8007 | 40 000 | 2242 | 560 | 4.0× | 30× |
| needle-1e94 | 20 000 | 63 | 29 | 2.2× | 10× |
| needle-1e305 | 50 000 | 173 | 48 | 3.6× | 25× |
| needle-1e1004 | 100 000 | 673 | 62 | 11× | 125× |
| dendrite-1e50 | 20 000 | 10.3 | 6.8 | 1.5× | 4.5× |
| dendrite-1e300 | 20 000 | 60 | 7.6 | 8× | 27× |
| dendrite-1e1000 | 20 000 | 156 | 5.9 | 26× | 88× |

At 1e-12…1e-17 per point big references run the plain kernel (deep-5e-14 35 ms, dense-5e-14
75 ms; the deep kernel took 51 and 115 ms there, M4's f64 path 38–40 and 84 ms). BLA there
stays off (M4 result unchanged).

**Reference times** (orbit of c = i, 100 001 points, no escape; precision = view bits + 104):
1e-20 3.2 ms, 1e-50 5.0 ms, 1e-100 10 ms, 1e-300 72 ms, 1e-1000 597 ms, 1e-2000 1686 ms.
Nucleus selection (ball period + Newton): period 8007 at 256 bits 10 ms in all, period 255 at
1152 bits 1.5 ms, period 834 at 3456 bits 8 ms. Centre references are computed to 2 × the
iteration limit (auto limit at 1e-300: 200 000 → 400 001 points, ~290 ms at 1152 bits); the
view shows the reprojected history until it arrives.

**Interaction** (`make interaction-test`, 60 Hz callbacks, 2560×1640 px; fps / latency p50 /
full-res final frame after input / shown frame vs fresh render):

| scenario | fps | latency p50 | final frame | vs fresh |
|---|---:|---:|---:|---|
| deep zoom 1e-12 to 1e-40 (0.36 e-folds per frame, to 3.9e-39/pt) | 51–56 | 50 ms | 217 ms | 0 px |
| 1e-100 pan (c = i) | 58.7 | 41.5 ms | 250 ms | 0 px |
| 1e-300 zoom (c = i) | 59.0 | 41.3 ms | 217 ms | 0 px |
| limit zoom (1e-14 → 3e-18, formerly clamped) | 53.0 | 43 ms | 550 ms | 0 px |

The first version of the deep zoom ran at 26 fps (33 ms GPU per gesture frame: the
dynamic preview could not go below 1/4 resolution while previews in the dense seahorse
valley need 25 000 iterations per texel); allowing 1/8 previews gave 51–56 fps. The shallow
scenarios are within the noise of a same-day run of the M4 build (neck zoom 48–52 vs 48,
home zoom 49–54 vs 55–57, keyboard glide 55–56 vs 52–55, edge pan 48 vs 44–49 fps).
FFI per frame 2.6–4 µs; `stats()` 19 µs (decimal coordinates; 0.1 ms for 1010 digits).

**Depth reached.** Interactively: zoom and pan at 1e-300 per point at 59 fps; stills and
verify to 1e-2900 per pixel; the zoom stops at 2^-10000 ≈ 1e-3010 per point
(`MIN_SCALE_LOG2`), where references need 10 000+ bits (≈0.5 M iterations/s single thread
at 4096 bits).

## R3: dense-region sampling (CPU study), 2026-10-14

Measured 2026-10-14, M1 Pro, release, CPU only (`prototypes/dense-sampling`, f64 direct
iteration, CPU port of the default look); deterministic iteration counts and OKLab ΔE × 100
against an independent 16-sample-per-pixel reference, no timing. 800 × 500 crops at each
location's 1600-wide pixel size (Feigenbaum 400 × 250), limit 4 × the location's. Method,
all strategies and the motion numbers: [[synthesis/dense-region-sampling]].

Iterations relative to the exact 1 spp render; ΔE mean / p99:

| location | exact 1 spp ΔE | FL AA (settled): cost, ΔE | local SA 1 spp | local SA + AA: cost, ΔE | interior-only Mariani–Silver |
|---|---|---|---:|---|---:|
| dense-5e-14 | 5.41 / 18.1 | 8.94, 1.96 / 7.1 | 0.194 | 1.24, 1.96 / 7.1 | 1.000 |
| deep-5e-14 | 3.40 / 16.8 | 6.83, 1.24 / 6.4 | 0.243 | 1.81, 1.23 / 6.5 | 1.000 |
| filament-1e6 | 1.60 / 14.7 | 4.43, 0.59 / 5.7 | 0.177 | 1.10, 0.60 / 5.7 | 1.000 |
| minibrots-5e-12 | 5.20 / 19.7 | 5.56, 2.04 / 11.5 | 0.674 | 2.22, 2.04 / 11.3 | 1.000 |
| seahorse-1e5 | 2.31 / 15.7 | 5.83, 0.85 / 6.1 | 0.540 | 3.36, 0.85 / 6.1 | 0.994 |
| neck-2e5 | 1.20 / 23.8 | 2.40, 0.92 / 23.4 | 0.516 | 1.41, 0.90 / 23.0 | 1.000 |
| root-third | 1.93 / 26.3 | 2.51, 1.47 / 26.1 | 0.643 | 1.70, 1.42 / 25.4 | 1.000 |
| elephant | 1.48 / 15.2 | 1.86, 0.57 / 6.0 | 0.963 | 1.75, 0.57 / 5.9 | 0.628 |
| minibrot-p4 | 0.76 / 13.2 | 5.01, 0.31 / 5.3 | 0.873 | 4.48, 0.31 / 5.3 | 0.757 |
| feigenbaum | 4.22 / 12.8 | 2.27, 1.59 / 7.1 | 0.937 | 1.94, 1.60 / 7.2 | 0.736 |

Local SA: quadtree from 32 × 32 tiles, ε = 1e−2, AA samples from pixel seeds
([[techniques/local-series-approximation]]). Not GPU numbers: the GPU check is
[[questions/local-sa-float32]].

## Interior colourings, overlays, control center (2026-10-15)

Measured 2026-10-15, M1 Pro, release, while other agents used the GPU (timings noisy; counts
are exact). See [[techniques/coloring-methods]] "Interior colouring as built" and
[[decisions/controls]] "control center, orbit overlay and mini-map".

**Interior cycle analysis** (`almondbread render --inside period --extend 16`, 960×600,
look aurora; iterations and loop steps from the chunk counters):

| view | flat: iterations / steps | period: iterations / steps | interior px | without period |
| --- | --- | --- | ---: | ---: |
| home | 1.48e7 / 1.48e7 | 1.49e7 / 1.49e7 | 70 540 | 0 |
| sea-8007 (no extension) | 2.21e10 / 8.76e8 | 2.21e10 / 9.03e8 | 1 502 | 367 (unresolved near the boundary) |
| needle-1e305 | 1.84e9 / 7.36e7 | 1.84e9 / 7.59e7 | 6 648 | 0 |

The analysis adds under 1 % of iterations at home and about 3 % of loop steps at depth
(one period of floatexp steps per interior pixel, no BLA). Pipelines without it are
unchanged (separate state buffer, `kInteriorColor` off).

**Overlays** (`--interaction-test`, 1280×820 pt window at 2×, 120 Hz panel):

| scenario | fps | draw CPU p50 / p95 ms | GPU ms/frame (iter + colorize/overlay) |
| --- | ---: | --- | --- |
| orbit pointer (200 iterates following the pointer over a resting view: every frame overlay-only) | 105.0 | 0.42 / 0.54 | 0.00 + 0.49 |
| overlays zoom (orbit + 3 map levels while zooming) | 61.7 | 0.47 / 0.59 | 5.62 + 2.04 |
| 1e5 zoom (overlays off, same run) | 66.9 | 0.08 / 0.17 | 4.63 + 1.77 |
| home pan (overlays off) | 52.9 | 0.10 / 0.17 | 6.42 + 3.87 |

With the overlays off nothing is built or encoded (the host skips it; draw CPU as before). On,
the CPU cost is ~0.35–0.4 ms per frame (orbit iterates, scene, tile binning, upload; the
probe's analysis runs at most 1.5 ms per frame until it is done) and an overlay-only frame
costs ~0.5 ms GPU at 2560×1640. The fps differences in this run are within the noise of the
shared GPU. The map renderers run only between gestures, with 2 ms chunks. The shown-frame
checks stay at 0 pixels differing in class.

## Autopilot and fly to, 2026-10-15

Measured 2026-10-15, M1 Pro, release, with other agents' GPU jobs running (motion-reuse
renders, deep verifies): absolute frame rates and the depth reached in real time are
pessimistic. Design: [[decisions/controls]] "Autopilot", "Fly to coordinates".

**Interaction test** (`make interaction-test`; this run had **120 Hz** callbacks, so fps are
not comparable with the 60 Hz FL table; 2560×1640 px):

| scenario | fps | interval p50 / p95 ms | latency p50 ms | draw CPU p50 ms | vs fresh render |
|---|---:|---|---:|---:|---|
| autopilot (Z, pointer circling 220 pt, Esc at 2.4 s) | 88.8–90.2 | 8.3 / 25.0 | 24.5 | 0.15–0.16 | 0–0.001 % |
| fly to (1e5 → 1e-11 per point, 11.9 e-folds/s) | 52.6–86.1 | 8.3 / 25.0–33.3 | 24.5 | 0.12–0.19 | 0 % |
| looks home / deep, idle / panning (22 looks and palettes in 2.2 s) | 7 idle (recolours only), 59–89 panning | — | 32.7–49.4 | 0.08–0.13 | 0–0.000 % |

Autopilot zoomed ×3.3–3.9 in 2.4 s with the eased start and steered the centre 504–649 pt;
off after Esc, refined frame 208 ms after the eased stop. Fly to landed exactly (scale text
`1e-11`) 1.53 s after the start. FFI 4–6 µs per frame. Same-run shallow scenarios: 74–102 fps.

**Fly to through the interactive scheduler** (`verify --history flyto`, 640×410, in
`make regress`; landed exactly in every run):

| flight | e-folds | speed | time | references | vs CPU oracle (class mismatch) | vs fresh |
|---|---:|---:|---:|---:|---:|---:|
| home → sea-8007 (5e-34/px) | 72 | 4 e/s | 40.3 s (loaded GPU: back-off; 19.4 s on a quieter one) | 2 | 0 (escape-n 39 %: float32 noise, cf. M3b) | 0, escape-n 0 % |
| home → c = i at 1e-300 | 686 | 16 e/s | 44.0 s | 6 | 0 | 0, escape-n 0 % |

Prefetching the reference at the target makes one reference serve most of the dive (2 for 72
e-folds, 6 for 686: a new one when the iteration limit outgrows the orbit or the kernel
switches plain → deep). The first prefetch (a plain centre reference at the target) passed
the class check but 35.8 % of the landed frame's escape counts differed from a fresh render
(smooth wrong fans at sea-8007, seen in the review video); the request now matches a jump's
and the session gates on escape counts (≤ 1 %).

**Review flights** (`almondbread fly`, 1920×1080, 60 Hz, `review/`, gitignored):

- `autopilot-seahorse.mp4` (78 s): scripted pilot (gain 2.5, lag 0.15 s) from home into
  seahorse valley, a look to the right at 2.5–3.3 s, Esc at 72 s. 54.0 fps presented, 8 late
  refreshes. The target stayed ~45 px off-centre (the pilot's equilibrium) through the dive;
  the throttle was 0.3 for most of the deep part (previews at 1/8 resolution: "Rendering" 75 %
  of the time), so the flight reached 3.2e-27 per point instead of ~1e-30. Images sharp at
  every still (seahorse valley, spirals, minibrot clusters at 1e-24).
- Earlier tuning runs (same day) are in [[decisions/controls]] Autopilot "Tuning".
- `fly-to-seahorse.mp4`: see the log entry of 2026-10-15.

**Crash fix** (1fef638): a colour change while progressive AA chunks were on the GPU
indexed a dropped AA level (the "Lagoon" crash). Regression test
`colour_change_during_progressive_aa` (every palette and look applied with AA in flight)
fails with the reported panic without the fix.

## PJ: journey, view rotation, cinematic effects, 2026-10-15

Measured 2026-10-15, M1 Pro, release, with another agent's GPU jobs (M5) running at times.
Design: [[decisions/controls]] "Journey", "View rotation", "Cinematic effects".

**Planner** (`almondbread journey --seed s --legs 18`, CPU only, one thread plus the oracle's
rayon rows): 0.01–0.3 s per plan from home to 1e43 (seeds 4711, 7, 20261015), 10–20
candidates, up to 14 probed per plan. Legs gain 2.8–18 e-folds (median ~6), resurfacing after
6 legs or below 1e40.

**Rotation through the interactive scheduler** (`verify --history`, in `make regress`; all
three new sessions pass, the 12 existing ones are unchanged):

| session | vs CPU oracle (class mismatch) | vs fresh render |
|---|---:|---|
| rotate (0.35 rad/e-fold) at the neck, 640×410, then ×1.32 | 0.355 % GPU-ext/CPU-int, 0 % false interior (unrotated zoom: 0.460 %) | 0 % class; escape-n 4.9 % in the first view (unresolved neck pixels), 0 % after the step |
| rotate at needle-1e305, 40°, 3 s at 3 e/s | 0.001 % | 0 %, escape-n 0 % |
| flyto home → sea-8007 turning to 60° | 0 % (escape-n 39 %: float32 noise, as unrotated) | 0 %, escape-n 0 %; landed exactly, 42.2 s at 4 e/s, 2 references |

Still renders: `verify --angle 30` at 2e-7 per pixel: class mismatch 0 %, escape-n 5.5 %
(unrotated 5.6 %); spiral-1e1000 at 75°: class mismatch 0 % (escape-n 25.8 % vs 18.1 %
unrotated: the float32-vs-f64 chaos of dense spirals, which the turned grid samples at other
points). The
kernel's extra cost is two multiply-adds per pixel start.

**Review videos** (`almondbread fly`, 1920×1080, 60 Hz, 85 s, `review/`, gitignored):

| video | presented | preview frames | late refreshes (worst) | iterate GPU / frame |
|---|---:|---:|---|---:|
| `journey-4711.mp4` (effects off) | 3906 of 5100 (46.0 fps; the rests present only when the image changes) | 3757 | 70 (395 ms; GPU shared) | 6.53 ms |
| `journey-4711-cinematic.mp4` | 3937 (46.3 fps) | 3788 | 1 (8.8 ms) | 6.73 ms |

Judged on stills every 5 s and frame strips: every landing is sharp and composed around a
structure (minibrot with rings, Julia-like stars, spirals), the flights are smooth and the
rests long enough to see the refined image. Variety is the weak point: the first six legs stay
in one region (dendritic, star-shaped embedded Julia filigree under the M(113,10) spiral), so
the first minute looks like one family of shapes; the resurfacing to "A Seahorse" at ~70 s
brings seahorse spirals. With 9 legs per dive (first recording) the whole 85 s stayed in one
region, hence 6. The cinematic run turns slowly (4°/e-fold) and shifts colours; at the first
default hue drift (2.5°/e-fold) the later legs turned muddy brown and pink, at 1°/e-fold the
look stays recognisable.

**Not measured: interaction tests and self-test.** The screen was locked for the whole
session (`CGSSessionScreenIsLocked`), so the background test windows get no display-link
callbacks and present nothing (the existing "home pan" scenario hangs the same way). The new
scenarios `rotate zoom`, `cinematic autopilot` and `journey` are in
`Sources/AlmondbreadKit/InteractionTest.swift` for the next unlocked run.

### PJ variety: dives and cross-overs, 2026-10-15

Design: [[decisions/controls]] "Journey", variety revision. Measured 2026-10-15, M1 Pro,
release, with several other agents' builds, GPU benches and regress runs on the machine
(load average 120–190 during the second set of videos: the pacing numbers of that set are
the machine's, see below).

**Planner** (`almondbread journey --seed s --legs 30`, seeds 4711, 7, 20261015, load ~160):
0.00–0.26 s per plan (median 0.01–0.02 s), within the PJ range (0.01–0.3 s). A cross-over
plan probes up to 14 anchors (shallow, cheap); a dive plan searches the view and a view 2–4
e-folds wider (two candidate searches).

**Contact sheets** (`--thumbs`, every landing at 480 × 270, `review/v*_sheet.jpg`; stills every
5 s of the videos, `review/journey-*-3min_sheet.jpg`). Before: #4711 legs 1–6 dendritic stars
under M(113,10), 7–13 seahorse spirals, 14–20 spirals again: three looks in 20 legs. After:
#4711 in 3 minutes visits snowflake dendrites (3/7 limb), Quad Spiral Valley, the Cauliflower
Medallion (thin stars around a medallion, a minibrot with rings), a seahorse, the needle
(Pine Compass: eight-armed needle stars down to a mandala at 1e15), the 5/8 limb and Scepter
Valley; #20261015 and #7 likewise 6–7 regions with spirals, dendrites, medallions, needles
and minibrots alternating. What still looks weak: (1) cross-over flights from a deep view
pass over the cardioid's or a bulb's edge, so 3–5 of 37 stills are mostly black mid-flight;
(2) a needle dive is 35–40 s of needle stars (cross-over to 1e12 plus a leg), the longest
stretch of one look; (3) dendrite carpets of the 3/7 limb come back after ~60 s in #7.

**Review videos** (`almondbread fly`, 1920 × 1080, 60 Hz, 180 s, `review/`, gitignored):

| video | presented | late refreshes (worst) | iterate GPU / frame | machine |
|---|---:|---|---:|---|
| `journey-4711-3min` (first variety build, effects off) | 8274 of 10800 (46.0 fps) | 73 (180 ms) | 6.83 ms | loaded |
| `journey-20261015-cinematic-3min` (first build) | 7923 (44.0 fps) | 30 (113 ms) | 6.76 ms | loaded |
| `journey-4711-3min` (final) | 6644 (36.9 fps) | 2578 (7478 ms) | 6.45 ms | load 120–190 |
| `journey-20261015-cinematic-3min` (final) | 6377 (35.4 fps) | 236 (709 ms) | 11.21 ms | load 120–190 |
| `journey-7-needle-3min` (final, starts in the needle) | 7860 (43.7 fps) | 810 (569 ms) | 6.63 ms | load 120–190 |
| `journey-4711-3min` (final, after merging main d7ab9e1, re-recorded) | 7701 (42.8 fps) | 194 (424 ms) | 6.09 ms | load 20–40 |

Interaction test (`ALMONDBREAD_SCENARIOS=journey make interaction-test`, screen unlocked, load
~50, after the merge): 20.1 fps, interval p50 / p95 16.7 / 100 ms, latency p50 / p95 27.9 /
44.5 ms, GPU 8.1 + 2.0 ms/frame, shown frame vs fresh render 0 % class difference; an earlier
run at load ~150 gave 10.2 fps with one GPU hang retried (other agents' GPU work). No earlier
journey interaction number exists to compare (PJ ran with the screen locked).

The first-build runs match PJ (46.0 / 46.3 fps, 70 / 1 late refreshes over 85 s). The final
runs' stalls are the recorder falling behind wall time while the view rested
(frame CPU ~0 ms, `late_ms` counting down 16.7 ms per refresh from 810 ms at 30 s in #4711):
the process was descheduled under load ~150, not planning or rendering work; the planner runs
on its own thread and took ≤ 0.26 s per plan at the same load. Re-measure on a quiet machine.

## R6: faster deep references, 2026-10-15

What changed: [[techniques/parallel-reference-orbit]]. Measured 2026-10-15, M1 Pro (8 P +
2 E cores), release, base = `b781bee` (M3b/R3 main) against the R6 branch, runs
interleaved base/new. Other agents shared the machine throughout: "loaded" = 1-minute load
average 35-90, "lighter" = load 4-12 (the quietest it got). Both are given.

**Orbit with outputs, one vs three threads** (`math_check par`, iterations/s, best / median
of 5-7 interleaved runs; the single-thread column is the faster form, unchanged by R6):

| bits | depth | 1 thread (lighter) | 3 threads (lighter) | x best / median (lighter) | x best / median (load ~20) |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1024 | | 2.86 M | 2.70 M | 1.01 / 0.94 | 1.05 / 0.85 |
| 1536 | | 1.43 M | 1.63 M | 1.44 / 1.14 | 1.31 / 1.06 |
| 2048 | | 0.97 M | 1.33 M | 1.53 / 1.37 | 1.29 / 1.16 |
| 3456 | 1e-1000 | 439 k | 796 k | 1.92 / 1.81 | 1.74 / 1.66 |
| 6784 | 1e-2000 | 142 k | 313 k | 2.21 / 2.21 | 2.24 / 2.19 |
| 10048 | 1e-3000 | 69 k | 158 k | 2.29 / 2.27 | 2.37 / 2.10 |

Helpers are used from 24 limbs (1536 bits) for orbits of 256+ iterations on 8+ core machines,
by the reference worker and Newton only (the CPU oracle stays single-threaded per orbit). So
per 100 000 iterations of a full-mantissa reference: 1e-20 (192 bits), 1e-100 (448) and 1e-300
(1152) are unchanged (3.3 ms, 7.2 ms, 47 ms single-thread); 1e-1000 0.23 s -> 0.13 s;
1e-2000 0.70 s -> 0.32 s. The spin-then-yield exchange degrades under load: at load 85 the
1e-1000 orbit gained only 1.27x in one series, and medians trail best runs by up to 25 %.

**Jump reference** (`reference_check`, 640 texels, limit 200 000, centre orbit to 400 000; ms;
"first" = first usable reference: base waits for the whole orbit, R6 hands out a 25 001-point
prefix; median of 3 (lighter) / 5 (loaded)):

| view | bits | base select + orbit (lighter) | R6 select + orbit (lighter) | base first | R6 first (lighter / loaded) |
| --- | ---: | ---: | ---: | ---: | ---: |
| dendrite@1e-20 | 192 | 0.1 + 10.9 | 0.1 + 8.2 | 11.1 | 1.2 / 1.3 |
| dendrite@1e-100 | 448 | 1.0 + 34.8 | 0.4 + 16.7 | 35.8 | 1.6 / 1.6 |
| dendrite@1e-300 | 1152 | 10.1 + 269 | 4.4 + 127 | 279 | 12.7 / 13.1 |
| dendrite@1e-1000 | 3456 | 205 + 1532 | 42 + 431 | 1735 | 72 / 109 |
| dendrite@1e-2000 | 6784 | 1379 + 4773 | 229 + 1238 | 6155 | 311 / 433 |
| needle-1e305 | 1152 | 0.3 + 0.1 | 0.4 + 0.1 | 0.4 | 0.5 / 0.5 |
| needle-1e1004 | 3456 | 6.3 + 1.2 | 5.0 + 0.9 | 7.5 | 5.9 / 16 |

The dendrite (c = i) orbit is exact small numbers; its 2.2x orbit gain at 1e-300 is the
short-product round-down fix alone, at 1e-1000/2000 that fix times the three threads. The
needles use one-period nucleus orbits and were already fast (no change beyond noise).

**Time to first frame, "Go to coordinates"** (`verify --history jump --location L --size
640x410`, ms from the jump to the first frame of the new view, which is also the complete
frame here; all sessions pass the CPU-oracle and fresh-render checks):

| location | base (lighter, 2 runs) | R6 (lighter) | base (loaded, 3 runs) | R6 (loaded) |
| --- | --- | --- | --- | --- |
| needle-1e305 | 43, 51 | 43, 35 | 60, 77, 52 | 43, 65, 68 |
| needle-1e1004 | 27, 27 | 27, 35 | 35, 49, 34 | 43, 60, 43 |
| dendrite-1e300 | 318, 327 | 27, 35 | 379, 376, 368 | 44, 36, 48 |
| dendrite-1e1000 | 1818, 1835 | 93, 110 | 2078, 2017, 2067 | 135, 269, 135 |

The first frame renders with the 25 001-point prefix (pixels rebase at its end); the dendrite
views escape by ~2700 iterations, so the prefix covers them entirely. `make regress`
(all sessions) passes.

**Multiplication algorithms at deep sizes** (`math_check kernels`, ns per square, best of 5,
load ~20): short Comba vs full square (Comba < 48 limbs, Karatsuba above): 18 limbs 101 vs
170, 54 limbs 616 vs 1128, 106 limbs 2071 vs 3553, 157 limbs 4596 vs 6784, 256 limbs 11206
vs 15576. No Toom-3, Mulders or NTT was built: they would have to beat the short product,
not Karatsuba ([[platform/mpfr-gmp-on-apple-silicon]]).

**Correctness**: `orbit_digest` (5 points incl. c = i, 64-10048 bits, all variants, one and
three threads) identical to base; unit tests for three threads through escape, resume after
cancel, short-product rounding on sparse mantissas, progressive prefixes equal to the final
orbit; nuclei within the oracle tolerance (Newton not bit-identical by design);
`cargo test --workspace --release`, clippy `-D warnings`, fmt clean.

## R5: local series approximation on the GPU (prototype), 2026-10-14

Measured 2026-10-14, M1 Pro (16-core GPU), macOS 26, release, branch
`worktree-agent-ade067c897fa45012`. **The GPU was shared with other agents throughout**:
single renders varied by up to 3× (dense-5e-14 plain 75–255 ms across sessions). Timings are
interleaved A/B (3 rounds, alternating order, 3 renders each after a warm-up; min / median /
max of 9); step counts are deterministic. Prototype and design:
[[techniques/local-series-approximation]] "GPU prototype (R5)"; answer:
[[questions/local-sa-float32]].

`almondbread render --sa 0|1 --size 1600x1000 --iterations <location's>` (offscreen, plain
look, interior test as configured). Steps: loop steps (BLA blocks count one); SA = seed steps
+ pixel steps. Quadtree 32 → 2 texels, ε = 1e−2; deep seeds take BLA blocks.

| location | maxIter | plain steps | SA steps | seed share | fewer steps | plain GPU ms | SA GPU ms (seeds, median) | speed-up median (min) |
|---|---:|---:|---:|---:|---:|---|---|---:|
| home | 1 000 | 6.31e7 | 5.74e7 | 2.3 % | 1.10 | 3.2 / 4.9 / 8.1 | 3.9 / 5.6 / 11.1 (0.5) | 0.87 (0.82) |
| home-100k | 100 000 | 1.75e8 | 1.69e8 | 0.8 % | 1.03 | 26.6 / 36.5 / 64.6 | 30.4 / 43.5 / 73.9 (1.3) | 0.84 (0.87) |
| seahorse-1e5 | 2 000 | 2.05e8 | 7.6e7 | 6.2 % | 2.70 | 6.0 / 6.3 / 15.3 | 4.3 / 5.0 / 9.6 (0.8) | 1.25 (1.37) |
| elephant | 5 000 | 1.06e9 | 1.00e9 | 0.8 % | 1.05 | 37.6 / 44.4 / 65.1 | 39.6 / 46.3 / 52.0 (1.1) | 0.96 (0.95) |
| minibrot-p4 | 50 000 | 3.80e8 | 3.59e8 | 0.8 % | 1.06 | 28.3 / 31.3 / 37.7 | 29.1 / 32.5 / 41.1 (0.6) | 0.96 (0.97) |
| deep-5e-14 | 6 000 | 1.92e9 | 2.16e8 | 5.7 % | 8.88 | 38.5 / 48.8 / 60.5 | 7.5 / 10.6 / 15.1 (0.9) | 4.61 (5.11) |
| feigenbaum | 200 000 | 8.62e10 | 7.97e10 | 1.1 % | 1.08 | 2533 / 2976 / 4967 | 3048 / 3994 / 6315 (188) | 0.75 (0.83) |
| neck-2e5 | 2 053 | 2.99e9 | 2.42e8 | 15.1 % | 12.4 | 94.5 / 147 / 310 | 16.3 / 49.2 / 122 (24) | 2.99 (5.79) |
| neck-7e6 | 2 907 | 4.64e9 | 5.09e8 | 41.4 % | 9.11 | 119 / 122 / 371 | 43.3 / 43.6 / 56.2 (33) | 2.79 (2.75) |
| cusp-1e5 | 3 000 | 4.67e9 | 3.00e9 | 7.0 % | 1.56 | 120 / 121 / 130 | 133 / 137 / 140 (37) | 0.89 (0.91) |
| dense-5e-14 | 5 791 | 4.61e9 | 5.29e8 | 8.7 % | 8.72 | 75.3 / 78.2 / 82.8 | 15.0 / 15.6 / 18.8 (3.1) | 5.03 (5.04) |
| root-third | 3 000 | 4.22e9 | 7.57e8 | 15.2 % | 5.57 | 108 / 111 / 116 | 34.2 / 34.4 / 37.8 (16) | 3.23 (3.17) |
| filament-1e6 (−1.25066 + 0.02012i, 3.75e−9/px) | 2 409 | 9.0e8 | 1.6e8 | 6.4 % | 5.63 | 17.5 / 19.2 / 20.5 | 9.0 / 10.3 / 18.5 (1.2) | 1.85 (1.94) |
| minibrots-5e-12 (dense centre, 4.6e−12/px) | 17 084 | 1.13e10 | 9.15e9 | 2.4 % | 1.23 | 301 / 309 / 342 | 280 / 289 / 417 (34) | 1.07 (1.07) |
| sea-1e20 | 20 000 | 2.05e9 (1.29e10 it) | 1.18e8 | 12.5 % | 17.4 | 229 / 293 / 335 | 28.3 / 34.3 / 53.8 (16) | 8.52 (8.10) |
| sea-8007 | 40 000 | 1.96e9 (5.97e10 it) | 1.82e9 | 1.9 % | 1.08 | 401 / 555 / 759 | 454 / 599 / 1216 (27) | 0.93 (0.88) |
| needle-1e305 | 50 000 | 1.95e8 (4.83e9 it) | 1.50e8 | 2.9 % | 1.30 | 64.4 / 85.2 / 115 | 59.0 / 70.3 / 111 (2.2) | 1.21 (1.09) |
| dendrite-1e300 | 20 000 | 4.66e7 (1.28e9 it) | 6.86e6 | 17.3 % | 6.79 | 5.5 / 8.4 / 13.3 | 3.5 / 4.5 / 6.1 (1.4) | 1.87 (1.55) |

Deep seeds without BLA blocks (`ALMONDBREAD_SA_SEED_BLA=0`, same session): sea-1e20 15.8×
fewer steps (seeds 20 % of the steps, 27 ms), sea-8007 0.34× (seeds 70 %, 1.1 s), needle
1.08×, dendrite 5.8×.

**Correctness vs the CPU oracle** (`verify`, 800 × 500 centre crop at the 1600-wide pixel
size; needle 1600 × 1000; plain / SA / SA vs plain):

| location | escape-n differs % | class mismatch % | GPU-int/CPU-ext % | median \|Δμ\| | >0.05 % |
|---|---|---|---|---|---|
| home | 0.42 / 0.49 / 0.44 | 0.021 / 0.021 | 0 / 0 | 2.4e−7 / 7.2e−7 | 0.64 / 0.73 |
| home-100k | 0.53 / 0.60 / 0.52 | 0.001 / 0.001 | 0 / 0 | 2.4e−7 / 7.2e−7 | 0.75 / 0.84 |
| seahorse-1e5 | 6.21 / 7.96 / 8.00 | 0.001 / 0.001 | 0 / 0 | 1.5e−5 / 4.7e−5 | 9.2 / 11.8 |
| elephant | 3.95 / 5.79 / 5.78 | 0.060 / 0.065 | 0 / 0 | 1.4e−6 / 1.8e−5 | 5.4 / 8.2 |
| minibrot-p4 | 1.08 / 1.12 / 0.90 | 0.001 / 0.002 | 0 / 0 | 2.6e−6 / 4.1e−6 | 1.58 / 1.65 |
| deep-5e-14 | 12.8 / 12.7 / 13.4 | 0.008 / 0.007 | 0 / 0 | 3.3e−4 / 3.4e−4 | 18.7 / 18.6 |
| feigenbaum | 62.2 / 62.2 / 55.3 | 0.084 / 0.084 | 0 / 0 | 11.7 / 11.6 | 70.9 / 70.9 |
| neck-2e5 | 0.005 / 0.011 / 0.011 | 0.001 / 0.001 | 0 / 0 | 2.4e−6 / 5.5e−6 | 0.013 / 0.025 |
| neck-7e6 | 0.002 / 0.010 / 0.010 | 0 / 0.001 | 0 / 0 | 2.3e−4 / 1.5e−3 | 0.004 / 0.022 |
| cusp-1e5 | 0.065 / 0.089 / 0.082 | 0.010 / 0.015 | 0 / 0 | 3.0e−5 / 1.5e−4 | 0.13 / 0.19 |
| dense-5e-14 | 16.0 / 16.9 / 15.1 | 0.008 / 0.009 | 0 / 0 | 0.0109 / 0.0123 | 30.6 / 32.1 |
| root-third | 0.011 / 0.025 / 0.022 | 0.002 / 0.003 | 0 / 0 | 5.5e−6 / 1.3e−5 | 0.034 / 0.059 |
| filament-1e6 | 3.86 / 5.19 / 5.29 | 0.004 / 0.004 | 0 / 0 | 1.5e−5 / 4.9e−5 | 6.2 / 8.4 |
| minibrots-5e-12 | 18.3 / 18.4 / 17.0 | 0.026 / 0.026 | 0 / 0 | 0.0134 / 0.0139 | 31.3 / 31.5 |
| sea-1e20 | 9.91 / 10.1 / 8.59 | 0 / 0 | 0 / 0 | 1.6e−4 / 1.7e−4 | 14.6 / 14.9 |
| sea-8007 | 0.39 / 0.39 / 0.17 | 0.006 / 0.006 | 0 / 0 | 0.028 / 0.028 | 0.59 / 0.59 |
| needle-1e305 | 0.10 / 0.11 / 0.08 | 0.001 / 0.001 | 0 / 0 | 2.3e−5 / 2.6e−5 | 0.19 / 0.20 |
| dendrite-1e300 | 0.001 / 0.010 / 0.009 | 0 / 0 | 0 / 0 | 4.8e−7 / 1.2e−6 | 0.004 / 0.028 |

Interior and unresolved fractions are equal plain vs SA at every location (e.g. Feigenbaum
3.4 % detected / 21.8 % unresolved both ways). Before the `w` fix (interior derivative
restarted at the branch) needle-1e305 had 96.3 % GPU-int/CPU-ext.

**Images** (default look Aurora, 1600 × 1000, 100 % and 300 % crops at dense, filament,
neck-2e5, seahorse, minibrots-5e-12, sea-1e20, needle-1e305, elephant, root-third): no tile
seams, no wrong-coloured patches, no missing minibrots. Pixels differing by more than 3 % in
colour: 0.2–0.6 % (dense, filament, seahorse, elephant, minibrots), < 0.003 % (neck,
root-third, needle), 1.5 % (sea-1e20); the differences are isolated boundary pixels.

**Deep-kernel DE bugs found on the way (both in the plain deep kernel, fixed in the R5
branch):** the DE was clamped to 65 000 *before* scaling by `2^-dExp`, drawing thick
stair-stepped outline bands in smooth exterior at sea-1e20; and the DE mantissa, renormalised
every 8th step only, can overflow (`|d|²` → inf, DE 0) in the steps before escape where
`|2z|` reaches 2^17 per step: dotted outline arcs along iteration contours. Fix: clamp after
scaling, renormalise every step once `|z| > 16`, normalise `d` before `length`.

## M5: local series approximation and interior fill, 2026-10-15

Measured 2026-10-15, M1 Pro (16-core GPU), macOS 26, release, branch
`worktree-agent-a9436fdde1a6473db` after merging main (view rotation). **The GPU was shared**
with the user's open app and other agents: the same plain render took 77 ms in the morning
and 125 ms (dense-5e-14) in the afternoon runs, so compare within a row only. Design and
as-built notes: [[techniques/local-series-approximation]] "As built (M5)",
[[techniques/guessing-and-subdivision]] "As built (M5)", [[techniques/auto-iterations]]
"Floor from the reference".

### Offscreen GPU time, M5 default vs all off

`almondbread render --size 1600x1000 --iterations <location's> --repeat 4` per process,
first render dropped (it has no statistics yet, as a fresh view in the app); "off" is
`--sa 0 --fill 0`; order A B B A per location, so 6 renders per variant; min / median / max.
Steps: loop steps of the pixels (BLA blocks count one), seeds extra.

| location | off GPU ms min / med / max | M5 GPU ms min / med / max | speed-up (median) | steps off → M5 | seeds |
|---|---|---|---:|---|---|
| home | 2.9 / 4.4 / 5.6 | 3.5 / 4.0 / 4.7 | 1.08 | 6.31e+07 → 6.29e+07 | 0.28 ms, 1 rounds, 1 % tiles |
| home-100k | 25.9 / 26.2 / 27.1 | 26.4 / 27.0 / 27.7 | 0.97 | 1.75e+08 → 1.75e+08 | 1.15 ms, 1 rounds, 0 % tiles |
| seahorse-1e5 | 6.6 / 6.8 / 7.6 | 4.2 / 4.7 / 5.5 | 1.45 | 2.05e+08 → 8.1e+07 | 0.32 ms, 1 rounds, 100 % tiles |
| elephant | 32.0 / 32.9 / 33.7 | 24.1 / 24.3 / 24.8 | 1.35 | 1.06e+09 → 7.06e+08 | 0.29 ms, 1 rounds, 0 % tiles (fill) |
| minibrot-p4 | 20.5 / 21.2 / 22.5 | 21.1 / 21.9 / 22.5 | 0.97 | 3.8e+08 → 3.8e+08 | 0.26 ms, 1 rounds, 0 % tiles |
| deep-5e-14 | 39.1 / 40.4 / 41.0 | 8.3 / 9.9 / 10.7 | 4.07 | 1.92e+09 → 2.33e+08 | 0.63 ms, 1 rounds, 100 % tiles |
| feigenbaum | 2366.1 / 2387.7 / 2408.8 | 2407.9 / 2414.7 / 2420.8 | 0.99 | 8.62e+10 → 8.62e+10 | 24.63 ms, 2 rounds, 0 % tiles |
| neck-2e5 | 76.3 / 77.5 / 78.0 | 12.8 / 13.4 / 13.6 | 5.79 | 2.99e+09 → 2.77e+08 | 4.64 ms, 1 rounds, 100 % tiles |
| neck-7e6 | 118.9 / 119.5 / 119.8 | 38.5 / 39.5 / 40.0 | 3.03 | 4.64e+09 → 8.32e+08 | 15.01 ms, 2 rounds, 100 % tiles |
| cusp-1e5 | 119.8 / 120.8 / 121.3 | 110.8 / 112.6 / 114.7 | 1.07 | 4.67e+09 → 3.8e+09 | 9.93 ms, 2 rounds, 50 % tiles |
| dense-5e-14 | 124.9 / 125.3 / 126.0 | 24.4 / 25.3 / 27.8 | 4.95 | 4.61e+09 → 6.02e+08 | 3.60 ms, 1 rounds, 100 % tiles |
| root-third | 107.9 / 111.3 / 115.4 | 31.9 / 33.3 / 33.9 | 3.34 | 4.22e+09 → 9.65e+08 | 7.04 ms, 2 rounds, 100 % tiles |
| sea-1e20 | 201.8 / 203.9 / 207.4 | 22.9 / 23.4 / 23.8 | 8.71 | 2.05e+09 → 1.28e+08 | 4.38 ms, 1 rounds, 100 % tiles |
| sea-8007 | 393.7 / 396.1 / 411.3 | 393.7 / 397.4 / 409.6 | 1.00 | 1.96e+09 → 1.96e+09 | 0.99 ms, 1 rounds, 0 % tiles |
| needle-1e305 | 45.4 / 45.8 / 46.3 | 40.9 / 41.1 / 41.6 | 1.11 | 1.8e+08 → 1.52e+08 | 0.78 ms, 1 rounds, 65 % tiles |
| dendrite-1e300 | 4.9 / 5.2 / 10.1 | 2.1 / 2.1 / 2.1 | 2.49 | 4.66e+07 → 7.58e+06 | 0.34 ms, 1 rounds, 100 % tiles |
| spiral-32412-1e19 | 3053.4 / 3066.8 / 3072.2 | 771.1 / 774.7 / 778.5 | 3.96 | 4.14e+10 → 7.69e+09 | 78.96 ms, 8 rounds, 100 % tiles |

Where nothing is seeded or filled (home-100k, minibrot-p4, Feigenbaum, sea-8007) the cost is
the seed pass (0.3–25 ms) and the first chunks' seeded pipeline: 0–3 %. Earlier in the day,
on a quieter GPU, the same build-up measured dense 77 → 15 ms and spiral 2 250 → 510 ms (both
~4.5–5×); the ratios hold, the absolute times do not.

### The user's spiral (spiral-32412-1e19), auto iterations, as the app sees a jump

`render --center … --scale 4.19804e-19 --size 1600x1000` without `--iterations`; "main" =
`ALMONDBREAD_SA=0 ALMONDBREAD_FILL=0 ALMONDBREAD_ITER_FLOOR=0` (main's behaviour); GPU of every
chunk and seed round, 3 runs each, alternating:

| | first render of the view | once the limit is known |
|---|---|---|
| main | 944 ms at 10 384 (every pixel unresolved) + 3 061 ms at 41 536 = **4.0 s** | 3.06 s |
| M5 | 691 ms pixels + 88 ms seeds at 41 536 (floor from the reference) = **0.78 s** | 0.78 s |

5.1× for the first view, 3.9× after. Wall 787 ms vs 3 067 ms for a single pass. BLA does not
help here and could not: relaxing its ε from 2^−24 to 2^−10 only extends its coverage from
1 302 to 2 016 iterations (1.07× → 1.11× fewer steps), because the pixels' deltas from the
centre orbit grow to the orbit's size within ~2 000 iterations in this chaotic spiral; local SA
works because neighbouring pixels stay close to *each other* for ~23 000 iterations (leaf
branch). The disc in the middle is pixels escaping after 41 536 (the extension passes resolve
it in the app), not a period-32 412 minibrot: "ref 32412 pts" is the centre orbit escaping at
32 411; no nucleus is in view (ball detection finds period 2 235, Newton fails).

### Correctness (verify vs the CPU oracle)

`verify`, 800 × 500 centre crop at the 1600-wide pixel size (needle 1600 × 1000), SA off / on /
auto, and the on field against the off field. GPU-interior/CPU-exterior is 0.000 % everywhere,
class mismatch on vs off 0.000 % at all 17 locations; escape-n differences and median |Δμ| move
as in R5 (dense 15.6 → 16.7 %, 0.0102 → 0.0117; sea-1e20 9.9 → 10.0 %; seahorse-1e5 6.2 → 8.0 %,
1.5e−5 → 4.7e−5; elephant 3.95 → 5.80 %; spiral 46.2 → 47.0 %, where float32 itself is far
from the oracle). Auto equals off bit for bit where no tile is seeded (home, home-100k,
elephant, minibrot-p4, Feigenbaum, sea-8007). Images (Aurora, 100 % crops at seahorse,
sea-1e20, spiral, elephant, root-third, dense, neck-2e5, needle): no seams, patches or missing
minibrots. Interior fill: bit-identical fields with and without (test `interior_fill_exact`).
`make regress` with SA auto and forced on: all sessions OK, including two new spiral sessions
(jump and pan; the pan exposed the interior-test switch bug, fixed, see
[[techniques/auto-iterations]]).

### Interactive (interaction test, background window 1280 × 820 pt at 2x, 120 Hz display)

Two runs each, M5 vs all off; the background window is throttled to ~31 presented fps in
both, so fps compare only where they drop below that. Final = full-resolution refined frame
after the last input.

| scenario | final frame ms M5 (run 1 / 2) | off | iterate GPU ms/frame M5 | off | preview scale M5 / off |
|---|---|---|---|---|---|
| dense pan | 517 / 417 | 933 / 966 | 5.8 / 7.1 | 7.5 / 7.9 | 0.13 / 0.12 |
| dense zoom | 433 / 450 | 1067 / 983 | 7.0 / 7.7 | 8.7 / 9.0 | 0.13 / 0.12 |
| neck2e5 pan | 533 / 783 | 1350 / 1100 | 10.1 / 9.1 | 10.5 / 11.1 | 0.12 / 0.12 |
| neck zoom (1.7e−8) | 333 / 400 | 733 / 550 | 4.3 / 4.4 | 8.0 / 7.8 | 0.14 / 0.12 |
| sea-1e20 pan | 333 / 433 | 1767 / 1733 | 10.1 / 11.5 | 13.4 / 12.6 | 0.12 / 0.12 |
| sea-1e20 zoom | 317 / 416 | 1617 / 1616 | 9.5 / 9.6 | 11.2 / 11.4 | 0.13 / 0.12 |
| spiral pan | 4 850 / 4 600 | none within 10 s | 19.0 / 19.3 (29 fps) | 39.5 / 40.0 (20 fps) | 0.12 / 0.12 |
| spiral zoom | 3 517 / 3 750 | none within 10 s | 14.5 / 10.6 | 10.4 / 11.7 | 0.12 / 0.12 |
| home pan | 216 / 216 | 216 / 216 | 6.9 / 6.0 | 7.2 / 6.8 | 0.93–0.96 / 0.90–0.94 |
| 1e5 zoom | 533 / 533 | 533 / 433 | 5.3 / 5.5 | 5.7 / 5.5 | 0.13 / 0.13 |
| deep zoom 1e-12 to 1e-40 | 400 / 217 | 283 / 283 | 7.1 / 6.9 | 5.9 / 6.0 | 0.75 / 0.43 |

Shown frame vs fresh render: ≤ 21 pixels (0.001 %) differ in class in every scenario, both
ways. Headless sessions through the same scheduler at 2560 × 1640 (`verify --history`, 120 Hz
pacing, prints the gesture frames): spiral jump complete 1 884 ms (off 10 895 ms); spiral pan
83 previews/s (off 90) after the gesture-frame fixes (unbounded gesture seed rounds had dropped
it to 12/s; a cheap seeded refined frame had misled the dynamic resolution to 166 ms gesture
frames), neck zoom 84 vs 74 previews/s, dense pan 118 vs 120.

Cost where views restart often: "looks deep idle" (a look or palette every 0.1 s, idle, at
1e−20) presented 12–16 fps with M5 against 13–25 off (noisy; 28–33 vs 17–27 ms GPU per frame):
every look that needs the distance estimate restarts the job, and its full level now starts
with seed rounds. Previews of refined views seed in one bounded round since (they waited for
several before: 12 fps).

## FB: flight benchmark v1 (first, noisy runs), 2026-10-15

`almondbread benchmark` (`make flight-bench`, `make flight-bench-live`) plays one fixed
60 s flight at 1920×1080 through every regime and scores it against a reference machine
(M1 Pro = 1000). Design, checks and the calibration procedure are on
[[synthesis/flight-benchmark]]. The reference in the repo is **provisional**: it was written
from single laps while other agents loaded the machine (the NOISY banner fired on every run:
3–9 other cores busy, GPU 24–97 % busy before the laps).

Measured 2026-10-15, M1 Pro, release, commit 8b9155c (main eadb176 + M5, SA auto), one lap
each, noisy:

| mode | result |
|---|---|
| A throughput (settled full quality, 300 frames at 5 Hz) | 36.5 s: shallow 10.1, dense 10.3, dive 4.5, heavy 5.6, rotate 5.5, jump 0.6; CPU 2.3 s, GPU 26.2 s. **INVALID** (below) |
| B headless (120 Hz) | 91.0 % of due frames shown, p99 25 ms, time to sharp 12.1 s in all (heavy 6.2 s; the 1e−1004 jump: first image 258 ms, sharp 1.05 s) |
| B window (120 Hz panel) | 24.6 % shown, p99 92 ms, input to photon p50 29 / p95 33 ms; the display link ran at ~38 Hz. The interaction test at the same time got 27–28 fps, so the cause is the load |

Findings: at the heavy spot (`spiral-32412-1e19`), reached by a jump straight after the dive
into c = i, mode A sometimes **settles all black** in 14 ms of GPU time. The limit is 10384,
below the period 32412; the reference search is capped at that limit, and the learner does
not raise it. In another run the view rendered but differed from a fresh render in 0.591 %
of its pixels' class. The existing shown-vs-fresh check cannot see the first case, so the
benchmark also compares each segment's settled frame with a from-scratch jump render
(76.7 % of 16×16 blocks differ in the black case). Both cases make the run INVALID. The
first case was reported to the driver for the M5 work.

## M7: float32 rounding artefacts and float-float, 2026-10-15

Measured 2026-10-15, M1 Pro (16-core GPU), release, with other agents' GPU jobs running
(load average 140–600). Tool: `almondbread precision` (`crates/almondbread-cli/src/precision.rs`):
the float32 GPU field against the CPU oracle (f64 deltas, [[questions/float32-error-long-orbits]])
**next to control oracles whose pixel grid is shifted by 0.001–0.5 pixel**. Where a 0.001-pixel
shift changes as many escape counts as float32 does, the float32 "error" is the view's own
chaotic sensitivity, not an artefact: any image of those pixels is one random draw. Both fields
are coloured by one CPU colouring (the plain look, unfiltered) for side-by-side crops. 400×260,
interior test off (as the oracle), auto iterations unless noted; "differs" = escape-n differs,
">0.05" = |Δμ| > 0.05, in % of pixels.

| view (texel) | float32 differs / >0.05 | 0.001 px shift | 0.01 px shift | float32 ~ shift of | float-float differs / >0.05 |
|---|---:|---:|---:|---:|---:|
| seahorse-1e5 (3e-7) | 3.84 / 5.63 | 8.70 / 12.8 | 14.0 / 20.6 | < 0.001 px | |
| elephant (6e-5) | 1.45 / 2.26 | 4.15 / 6.37 | 6.97 / 10.6 | < 0.001 px | |
| minibrot-p4 (1e-5) | 0.20 / 0.39 | 1.04 / 1.89 | 2.45 / 4.93 | < 0.001 px | |
| neck-2e5, cusp-1e5, root-third | ≤ 0.04 / ≤ 0.09 | 0.03–0.05 / 0.05–0.13 | | < 0.001 px | |
| feigenbaum (5e-10, 2e5 it) | 54.2 / 64.4 | 58.6 / 67.6 | 67.5 / 70.2 | < 0.001 px | |
| deep-5e-14 (4.3e-13) | 8.99 / 13.1 | 15.4 / 22.2 | 24.4 / 35.0 | < 0.001 px | 0.30 / 0.42 |
| dense-5e-14 (1.8e-13) | 12.8 / 24.8 | 20.4 / 37.8 | 42.3 / 71.3 | < 0.001 px | |
| crystal-compass (7.7e-17) | 0.64 / 1.49 | 2.63 / 5.86 | 8.14 / 18.8 | < 0.001 px | |
| sunflower-island (8.5e-18) | 0.13 / 0.28 | 1.10 / 2.45 | 3.55 / 8.39 | < 0.001 px | |
| spiral-32412-1e19 (1.7e-18) | 52.8 / 66.2 | 51.6 / 64.5 | 67.1 / 83.8 | ~0.001 px | 6.27 / 8.02 |
| sea-1e20 (4e-20) | 10.5 / 15.7 | 12.9 / 19.0 | 20.8 / 30.8 | < 0.001 px | 0.29 / 0.44 |
| copper-mandala (2.1e-24) | 3.71 / 7.37 | 5.42 / 10.9 | 13.1 / 25.5 | < 0.001 px | |
| frost-mandala (1.2e-29) | 1.70 / 4.25 | 3.46 / 8.37 | 10.9 / 24.9 | < 0.001 px | |
| sea-8007 (8e-34, 1e5 it) | 39.1 / 55.1 | 39.9 / 56.1 | 59.5 / 79.7 | < 0.001 px | |
| spiral-1e50 (3.8e-53) | 9.48 / 17.2 | 4.06 / 7.46 | 8.96 / 16.2 | ~0.01 px | 0.021 / 0.036 |
| needle-1e94 (4e-94) | 0.08 / 0.14 | 0.83 / 1.92 | 3.61 / 10.1 | < 0.001 px | |
| junction-1e100 (3.8e-103) | 0.96 / 2.14 | | 2.37 / 5.32 | < 0.01 px | |
| starburst-1e112 (3.8e-115, 2e5 it) | 3.19 / 7.34 | 2.72 / 6.13 | 8.60 / 20.1 | < 0.01 px | 0.001 / 0.002 |
| junction-1e300 (3.8e-303, 2e5 it) | 1.42 / 3.31 | | 2.16 / 4.88 | < 0.01 px | |
| spiral-1e1000 (3.8e-1003, 2e5 it) | 17.8 / 34.0 | 2.80 / 5.08 | 6.07 / 11.1 | 0.1–0.2 px | 0.031 / 0.062 |
| needle-minibrot-1e1000 (1e6 it, 200×130) | 0.15 / 0.24 | 5.15 / 15.7 | 23.8 / 77.5 | < 0.001 px | |

(Places are from the catalogue, [[synthesis/places]]; sea-8007 framed 400 wide needs 1e5
iterations, at its auto limit of 29 004 every pixel is still iterating.)

**No visible artefacts.** At 100 % and 4× crops (GPU | oracle | 0.001-px control) of
sea-8007, spiral-32412-1e19, spiral-1e50, spiral-1e1000, deep-5e-14 and needle-1e94 the float32
and oracle images are indistinguishable; the differences sit in boundary pixels that are
noise in the oracle too (the 0.001-px control looks equally different). Smooth regions agree to
|Δμ| 1e-7…1e-4 except at the two catalogue spirals at 1e50 and 1e1000, where the signed error
map shows a **smooth, spatially coherent** error (opposite signs on the two sides of each arm,
median |Δμ| 0.002 and 0.022): neighbouring pixels share their rounding events, so float32 bends
the image by a fraction of a pixel rather than speckling it. At the default density 0.02 that is
4e-4 of a palette cycle; correlation with a 1.001× grid zoom is −0.48, with grid shifts and turns
≈ 0, so it is not a plain shift, scale or turn. Float-float removes it (signed map black).

**The oracle is not ground truth in chaotic views either.** At spiral-32412-1e19 float-float
(48 bits) and the f64 oracle (53 bits) still disagree on 6.3 % of escape counts and 0.74 % of
classes: those pixels have no stable answer at any practical precision, and float32 (52.8 %)
sits at the 0.001-px-shift level (51.6 %).

**Float-float known-answer test** (`precision --kat 1`; same library compiled twice):

| check | `MathMode::Safe` | `MathMode::Fast` |
|---|---|---|
| TwoSum(1, 1e-8) low part | 1e-8 ✓ | 0 ✗ |
| QuickTwoSum(1, 2^-30) low part | 2^-30 ✓ | 0 ✗ |
| TwoProd(1+2^-12, 1+2^-12) via `fma` | 2^-24 ✓ | 2^-24 ✓ |
| ff(1/3)·3 | 1 exactly ✓ | 1 exactly ✓ |
| 1000-step ff orbit vs f64 (c = −0.5+0.3i) | 9.4e-16 ✓ (float32 6.2e-9) | 4.5e-8 ✗ |
| fx a²+a at 2^-1000 | exact ✓ | exact ✓ |

**Throughput of the number types** (`precision --kat 1`, z = z² + c, 262 144 threads × 2000
steps, medians of 5–7, three runs under load; all in the safe library): float32 1.8–3.0e11
it/s, float-float 2.58e10 (**7–12× slower**), fx (float-float + exponent, normalised after
every operation) 3.4–6.6e9 (27–52× slower).

**Cost of a float-float field** (`precision --ff 1`, every pixel, fx deltas and reference, no
BLA/SA, 400×260) against the float32 renderer's iterate GPU time: deep-5e-14 91 vs 8 ms,
sea-1e20 638 vs 18 ms, spiral-1e50 396 vs 12 ms, starburst-1e112 117 vs 2 ms, spiral-1e1000
1766 vs 6 ms, spiral-32412-1e19 1770 vs 254 ms (1.3–3.6e9 it/s). A repair pass would have to
stay on a few flagged pixels; none was built (nothing to repair).

**No cost to the float32 path** (A/B `bench --sizes 1600x1000 --no-verify 1 --repeat 3`, base
`eadb176` vs the M7 branch, interleaved, 3 rounds, medians): geomean new/base 1.014 over 21
locations (per-location 0.86–1.29, the spread of the load); float32 fields bit-identical at
seahorse-1e5, deep-5e-14, sea-1e20, needle-1e305, spiral-32412-1e19. The main kernel library
is unchanged; the float-float library compiles only when a `FloatFloat` is created (verify
tools and tests).
