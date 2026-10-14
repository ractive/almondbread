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
