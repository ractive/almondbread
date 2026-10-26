---
title: "GPU watchdog and chunked, resumable kernels"
type: platform
status: draft
sources: ["sources/mlx-watchdog-issue", "sources/fraktaler-3-site", "sources/kf-manual", "sources/fractalforums-interior-detection"]
tags: [metal, watchdog, scheduling, divergence, indirect-dispatch]
---

# GPU watchdog and chunked, resumable kernels

macOS kills GPU work that runs too long. A deep-zoom pixel can need 1e6 to 1e9
iterations, so almondbread cannot render a frame in one dispatch. This page
gives the facts and the scheduler design.

## Facts

- **Threshold about 5 s, per command buffer.** An MLX report (M2 Ultra, macOS
  26.3.1) says a single long dispatch exceeds "the macOS GPU watchdog threshold
  (~5 seconds)" and that "the watchdog fires per-command-buffer, not per-dispatch";
  several dispatches in one command buffer do not reset the timer
  (src: [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)). This is a community report, not Apple
  documentation; see [What is the exact Metal GPU watchdog limit on macOS and iOS?](../questions/metal-watchdog-limit.md).
- **Error codes.** The MLX report shows
  `kIOGPUCommandBufferCallbackErrorImpactingInteractivity` and process termination
  (src: [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)). The research notes also name
  `kIOGPUCommandBufferCallbackErrorTimeout` and an Apple error for a device whose
  access was revoked after too many timeouts (notes: gpu_apple_silicon §Apple GPU architecture facts).
  Handle both: treat a timeout as "chunk too big", halve the budget, retry.
- **Display-active variant.** A separate MLX issue (#3267) reports the watchdog
  killing work while a display is active, a different trigger
  (src: [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)). Whether headless machines differ is unknown.
- **Unofficial override.** `AGX_RELAX_CDM_CTXSTORE_TIMEOUT=1` works as a system-wide
  workaround but disables the watchdog entirely; not acceptable for an app
  (src: [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)).
- **Fraktaler 3 hits the same wall** with OpenCL: larger tiles are faster (one
  location: 960x1080 tile 1m34s vs 3 min with default 128x128 tiles), but too-large
  tiles or excessive perturbation/BLA step limits cause OS GPU timeouts
  (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)).
- MLX's fix is the same idea as ours: split the work into chunks with partial
  results and reduce afterwards (src: [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)).

## Design: resumable per-pixel state

Each pixel's whole loop state lives in a device buffer, so any dispatch can stop
after a budget and the next one continues (inference, following the cited facts).

```text
struct PixelState {            // one per sample; SoA in practice
    delta      : T             // z (float2, or float2 + int exponent when rescaled)
    scaleExp   : int32         // rescaling exponent (techniques/extended-range-floats)
    n          : uint32        // pixel iteration count
    m          : uint32        // reference index (differs from n after rebasing)
    der        : float2 (+exp) // dz/dc for DE, optional
    derZ       : float2        // dz/dz for interior detection, optional
    flags      : uint8         // live / escaped / interior / maxiter
}
```

Kernel body: load state, run at most `budget` iterations (or skip-table steps)
with the normal escape, rebase and interior tests from [Perturbation](../techniques/perturbation.md),
[Rebasing (Zhuoran, 2021)](../techniques/rebasing.md), [Bivariate linear approximation (BLA)](../techniques/bla.md) and [Interior detection](../techniques/interior-detection.md),
store state, and if still live append the pixel index to the next live list.

## Scheduler pseudocode (our own)

```text
live = all pixel indices; budget = 4096 iterations
while live not empty and not cancelled:
    cb = queue.makeCommandBuffer()
    repeat k times (k small, e.g. 1..4):
        dispatch iterateKernel over live (indirect: count from GPU counter)
            -> writes nextLive via atomic append, nextCount
        swap(live, nextLive)
    cb.commit(); wait or overlap with next frame's CPU work
    t = cb.gpuEndTime - cb.gpuStartTime
    budget = clamp(budget * targetMs / t, minBudget, maxBudget)   // target 10-30 ms
    if cb.error is timeout: budget /= 4; mark chunk for retry
    publish progress image every few command buffers
```

Points:

- Target 10 to 30 ms per command buffer for UI responsiveness, far below 5 s
  (inference). A frame becomes many command buffers.
- **Compaction** attacks divergence: after each chunk only still-live pixels are
  dispatched, so SIMD-groups are refilled with work instead of idling behind one
  slow pixel (inference). Use an atomic counter plus indirect dispatch so the CPU
  never reads the count.
- Optionally sort or bin live pixels by tile to keep reference/table reads
  coherent (inference).
- **Persistent threads** ("while queue not empty") are riskier on macOS than CUDA:
  there is no preemption and the timer is per command buffer, so any persistent
  kernel must still be bounded by an iteration budget (inference).
- Skip-table steps cost about the same as perturbation steps, so budget in "steps",
  not "iterations" (inference). mathr's Fraktaler statistics distinguish the two
  for the same reason (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)).
- KF2+ shows progressive previews by interlacing (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md));
  chunking gives the same effect for free: publish the state buffer as an image
  between command buffers ([Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md)).

## Pitfalls

- Fence or event waits inside a command buffer count toward the watchdog
  (src: [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)).
- Iteration counts per pixel vary by orders of magnitude; a fixed budget either
  wastes dispatches on shallow pixels or times out on deep ones. Adapt from timing.
- Register-heavy kernels lose occupancy on M1/M2 ([Apple GPU architecture for compute](apple-gpu-architecture.md));
  chunking does not fix that, specialisation does ([Metal compute pipeline for a deep-zoom renderer](metal-compute.md)).

- **Pixels that keep iterating past the frame's limit must not hold live capacity**
  (found 2026-10-14, M1 Pro, release). A first version of unresolved-pixel extension
  ([Automatic maximum-iteration control](../techniques/auto-iterations.md)) let pixels continue past the nominal limit inside
  the level. Slow pixels (half the frame at the −3/4 neck) filled the 1 M live slots,
  `prepareChunk` admitted only 3 to 9 K fresh pixels per chunk, and the complete frame
  took 2.9 s instead of 0.40 s. Now the level stops at the limit, collects unresolved
  pixel indices into a list (threadgroup-aggregated append), and separate extension
  levels admit from that list. They restart those pixels from n = 0, which repeats
  1/4 of the work per ×4 pass.
- **A budget that stops a scanline-ordered pass leaves a straight edge.** Stopping an
  extension pass on a GPU-time budget showed the resolved top rows above a horizontal
  line. Decide from an estimate *before* a pass (`count × limit / rate`) and always
  finish it. Likewise, never extend a truncated list.
- **Level tails inflate a lanes × K rate** (found 2026-10-26, M1 Pro). Learned as
  `(liveIn + fresh) × K / gpu` from every chunk, the rate read a level's tail as nearly
  free: pixels parked at the limit for the interior probe occupy lanes but do no steps,
  probe chunks count a probe step as one loop step while K budgets 4 or 16, and a few lanes
  run the whole limit. Each such chunk raised the rate ×4, to ~50× the job's mean
  throughput (2e11 vs 4e9 loop steps/s at the N view, period-30 957 minibrot at
  2e-166/pt, 1280 × 800). The next level's, extension pass's or compute-further segment's
  first chunks were sized from it: 140–470 ms command buffers instead of 16. Within a level
  the same rule is right (the busy share only falls, and in a tail of parked pixels it lets
  K grow so the last few lanes finish in a few cheap chunks; holding K there cost the
  Feigenbaum bench view 2 100 extra ~1 ms chunks, +17 % GPU). Fix: the busy rate learns
  only from non-probe chunks of at least 1 ms whose steps are at least 25 % of lanes × K
  (a chunk under 50 % busy counts at most twice its steps), each level keeps its own
  lanes × K rate for its later chunks, and the deep kernel has its own busy rate (it
  inherited the plain kernel's 3e10, 6× too high: 85 ms first chunks). The extension
  budget check and compute further's fallback rate now convert iterations to loop steps by
  the level's iterations per step (with LA ~280 at N); the inflated rate had hidden that
  units mismatch. Numbers: [almondbread benchmarks and verification record](../synthesis/benchmarks.md#throughput-model-level-tails-2026-10-26).
- **Lesson from the wedge report:** a long straight edge in a fractal image is a
  pipeline or test artefact. But "never written" and "stale" are not the only
  causes: here the culprit was a max-norm test on a nearly linear function of `c`
  ([Interior detection](../techniques/interior-detection.md) Pitfalls). `--verify --history` drives the real
  scheduler (`Renderer.renderFrame`, the code the view runs per refresh) headlessly
  through scripted Space/wheel/pan sessions and diffs the final frame against the
  CPU oracle and a fresh render. `--interaction-test` checks the shown frame against
  a fresh render after every scenario.

## As built in almondbread (milestone 2)

Measured 2026-10-14, M1 Pro, release; numbers in [almondbread benchmarks and verification record](../synthesis/benchmarks.md).

- **State moves with its list slot.** Each live pixel's state (dz, d, w + exponent,
  pixel index, n, m: 40 bytes) sits in a ping-ponged buffer indexed by live-list
  slot, not by pixel. State memory is then bounded by the live capacity (1 M slots,
  2 × 40 MB on macOS; 512 K on iOS) instead of by image size or supersampling.
  Rescaled/floatexp deltas and BLA (milestone 3) only add fields to this struct.
- **Three dispatches per chunk, one command buffer:** a 1-thread `prepareChunk` admits
  fresh pixels into free capacity (`min(total − admitted, capacity − live)`; the
  admission counter lives on the GPU, so chunks run back to back without a CPU round
  trip), clears counters and writes the indirect arguments; `iterate` runs
  `dispatchThreadgroups(indirectBuffer:)` over live + fresh pixels for at most K
  iterations; a 1-thread `finishChunk` copies the live count into a 64-slot stats ring
  the CPU reads on completion.
- **K from a throughput model.** `K = target × rate / threads`, with `threads` an upper
  bound from the last harvested counts and `rate` in lanes × K (loop steps) per GPU
  second, damped to ×4 per update. Target 16 ms idle, 8 ms during gestures. Measured on
  a 1e7-iteration render: chunk GPU time median 16.0 ms, p99 17.6 ms, max 70 ms.
  Two rates since 2026-10-26 (see Pitfalls, "level tails"): a **busy rate** per kernel
  (plain, deep; cold 3e10 and 5e9 loop steps/s) learned only from chunks that measure
  throughput, which sizes a level's first chunks, and a **level rate** learned from every
  chunk of that level, which sizes its later ones.
- **Compaction must keep order.** One atomic per SIMD-group appended survivors in a
  scrambled order; after a few chunks SIMD-groups mixed pixels from the whole image
  and `ref[m]` reads stopped coalescing: −31 % throughput at 16 ms chunks (13.9 s vs
  9.5 s at 50 ms chunks). One atomic per threadgroup with in-group prefix sums keeps
  survivors roughly in scanline order and removes the chunk-size dependence (9.6 s vs
  9.4 s). This contradicts the inference above that compaction alone fixes divergence:
  it does, but only if it preserves locality.
- **No float atomics.** An `atomic_float` iteration counter cost ~28 ms per million-
  pixel chunk; a uint low word with a carry into a high word is free.
- **Cancellation by generation.** A new view starts a new job; harvested chunks of
  older generations only feed the rate model. One serial queue plus "every admitted
  pixel is rewritten by its own job" means no stale chunk can show in a newer frame.
- **Watchdog.** M2 never sees one. Separately, M1's single 10.4 s command buffer
  completed without error (headless, display on); see [What is the exact Metal GPU watchdog limit on macOS and iOS?](../questions/metal-watchdog-limit.md).

## Gaps

- No Apple document states the threshold or how it differs on iOS, without a
  display, or under load ([What is the exact Metal GPU watchdog limit on macOS and iOS?](../questions/metal-watchdog-limit.md), [iOS and iPadOS port considerations](ios-ipad-considerations.md)).

# Citations

- [MLX issue 3302: GPU watchdog kills long SDPA dispatch](../sources/mlx-watchdog-issue.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)
