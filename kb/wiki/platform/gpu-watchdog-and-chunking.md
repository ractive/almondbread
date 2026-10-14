---
title: "GPU watchdog and chunked, resumable kernels"
type: platform
status: draft
sources: ["[[sources/mlx-watchdog-issue]]", "[[sources/fraktaler-3-site]]", "[[sources/kf-manual]]", "[[sources/fractalforums-interior-detection]]"]
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
  (src: [[sources/mlx-watchdog-issue]]). This is a community report, not Apple
  documentation; see [[questions/metal-watchdog-limit]].
- **Error codes.** The MLX report shows
  `kIOGPUCommandBufferCallbackErrorImpactingInteractivity` and process termination
  (src: [[sources/mlx-watchdog-issue]]). The research notes also name
  `kIOGPUCommandBufferCallbackErrorTimeout` and an Apple error for a device whose
  access was revoked after too many timeouts (notes: gpu_apple_silicon §Apple GPU architecture facts).
  Handle both: treat a timeout as "chunk too big", halve the budget, retry.
- **Display-active variant.** A separate MLX issue (#3267) reports the watchdog
  killing work while a display is active, a different trigger
  (src: [[sources/mlx-watchdog-issue]]). Whether headless machines differ is unknown.
- **Unofficial override.** `AGX_RELAX_CDM_CTXSTORE_TIMEOUT=1` works as a system-wide
  workaround but disables the watchdog entirely; not acceptable for an app
  (src: [[sources/mlx-watchdog-issue]]).
- **Fraktaler 3 hits the same wall** with OpenCL: larger tiles are faster (one
  location: 960x1080 tile 1m34s vs 3 min with default 128x128 tiles), but too-large
  tiles or excessive perturbation/BLA step limits cause OS GPU timeouts
  (src: [[sources/fraktaler-3-site]]).
- MLX's fix is the same idea as ours: split the work into chunks with partial
  results and reduce afterwards (src: [[sources/mlx-watchdog-issue]]).

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
with the normal escape, rebase and interior tests from [[techniques/perturbation]],
[[techniques/rebasing]], [[techniques/bla]] and [[techniques/interior-detection]],
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
  for the same reason (src: [[sources/fractalforums-interior-detection]]).
- KF2+ shows progressive previews by interlacing (src: [[sources/kf-manual]]);
  chunking gives the same effect for free: publish the state buffer as an image
  between command buffers ([[techniques/frame-reuse-and-progressive]]).

## Pitfalls

- Fence or event waits inside a command buffer count toward the watchdog
  (src: [[sources/mlx-watchdog-issue]]).
- Iteration counts per pixel vary by orders of magnitude; a fixed budget either
  wastes dispatches on shallow pixels or times out on deep ones. Adapt from timing.
- Register-heavy kernels lose occupancy on M1/M2 ([[platform/apple-gpu-architecture]]);
  chunking does not fix that, specialisation does ([[platform/metal-compute]]).

## As built in almondbread (milestone 2)

Measured 2026-10-14, M1 Pro, release; numbers in [[synthesis/benchmarks]].

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
  bound from the last harvested counts and `rate` (lane-iterations per GPU second)
  adapted from every chunk, damped to ×4 per update. Target 16 ms idle, 8 ms during
  gestures. Measured on a 1e7-iteration render: chunk GPU time median 16.0 ms, p99
  17.6 ms, max 70 ms.
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
  completed without error (headless, display on); see [[questions/metal-watchdog-limit]].

## Gaps

- No Apple document states the threshold or how it differs on iOS, without a
  display, or under load ([[questions/metal-watchdog-limit]], [[platform/ios-ipad-considerations]]).
