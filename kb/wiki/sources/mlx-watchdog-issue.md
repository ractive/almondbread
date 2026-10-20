---
title: "MLX issue 3302: GPU watchdog kills long SDPA dispatch"
type: source
authors: ["MLX contributors"]
year: 2026
raw: "web/mlx-watchdog-issue.html"
status: digested
tags: [metal, gpu-watchdog, command-buffer, chunking, apple-silicon]
---

# MLX issue 3302: GPU watchdog kills long SDPA dispatch

**URL:** <https://github.com/ml-explore/mlx/issues/3302> · **Saved:** `raw/web/mlx-watchdog-issue.html` (+ `.txt`) · **Terms:** GitHub issue; copyright the posters

## What it is

A GitHub issue in Apple's MLX machine-learning framework, opened 24 March 2026 by user Thump604, titled "GPU watchdog kills process during long-context SDPA prefill (65K+ keys)". A single Metal compute dispatch ran past the macOS GPU watchdog and the process was killed. Closed 26 April 2026 as a duplicate of #3267.

## What it covers

- Symptom: one `steel_attention` dispatch covering all keys at 65K+ key length runs longer than the macOS GPU watchdog threshold, "~5 seconds". Error `kIOGPUCommandBufferCallbackErrorImpactingInteractivity`, then process termination.
- Hardware: M2 Ultra 128 GB, macOS 26.3.1. A 122B model at 128K+ was borderline at ~4–5 s for a single dispatch.
- Key claim: the watchdog is per command buffer, not per dispatch. Several dispatches in one command buffer do not reset the timer.
- Failed fixes: committing the command buffer mid-primitive hit the Metal assertion "A command encoder is already encoding to this command buffer". Smaller application-level steps (separate command buffers) partly worked, with overhead.
- `AGX_RELAX_CDM_CTXSTORE_TIMEOUT=1` works but is a system-wide environment variable that disables the watchdog; rejected as a permanent fix. Later commits referencing the issue nevertheless default it on or add opt-in guards.
- Fix: split the work into chunks (e.g. 65,536 keys), dispatch each with partial outputs, then reduce (online softmax), as MLX already did for decode (`sdpa_vector_2pass`). Thresholds by env vars `MLX_SDPA_CHUNK_THRESHOLD`, `MLX_SDPA_CHUNK_SIZE`.
- Independent check on M3 Ultra 256 GB: 65K/128K/262K contexts ran in 0.03–0.16 s with no watchdog kill.
- Related issue #3267: watchdog during training when a display is active (a different trigger).

## Reliability

- Primary user report with reproduction; the "~5 seconds" and "per command buffer" claims are the reporter's observations, not Apple documentation. Apple publishes no watchdog limit (see [What is the exact Metal GPU watchdog limit on macOS and iOS?](../questions/metal-watchdog-limit.md)).
- The validating comment lists "macOS 25.3.0 (Darwin 25.3.0)", which mixes up macOS and Darwin version numbers; the hardware facts are still usable.
- The issue is about compute kernels on a machine driving a display. Whether the limit differs on a headless Mac or with no display attached is not covered.

## Useful sections

- §Summary — 65K-key dispatch exceeds ~5 s watchdog, error name.
- §Root Cause — single dispatch; watchdog per command buffer.
- §What I Tried — mid-buffer commit assertion, `AGX_RELAX_CDM_CTXSTORE_TIMEOUT`, smaller steps.
- §Proposed Solution: Chunked Full Attention — chunk + reduce design.
- Activity (hnshah comment) — M3 Ultra validation timings.

## Used by

- [GPU watchdog and chunked, resumable kernels](../platform/gpu-watchdog-and-chunking.md)
- [What is the exact Metal GPU watchdog limit on macOS and iOS?](../questions/metal-watchdog-limit.md)
- [Metal compute pipeline for a deep-zoom renderer](../platform/metal-compute.md)

# Citations

- <https://github.com/ml-explore/mlx/issues/3302>
