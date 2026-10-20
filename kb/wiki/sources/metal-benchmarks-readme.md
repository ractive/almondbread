---
title: "metal-benchmarks README (Apple GPU microarchitecture)"
type: source
authors: ["Philip Turner"]
year: 2024
raw: "web/metal-benchmarks-readme.md"
status: digested
tags: [apple-gpu, metal, microbenchmarks, latency, throughput, fp64-emulation]
---

# metal-benchmarks README (Apple GPU microarchitecture)

**URL:** <https://raw.githubusercontent.com/philipturner/metal-benchmarks/main/README.md> · **Saved:** `raw/web/metal-benchmarks-readme.md` · **Terms:** MIT (repository licence)

## What it is

Philip Turner's long README describing the M1/M2 (Apple GPU families 7 and 8) GPU from his own microbenchmarks: per-chip FLOPS, on-chip memory, per-instruction latency and throughput, pipeline layout, and comparisons with AMD, Intel and Nvidia. Repository pushed 2024-09-22 (brief). It is the most detailed public instruction-level data for Apple GPUs and covers no chip newer than M2/A16.

## What it covers

- Chip table: M1 1.278 GHz, 8 cores, 2617 GFLOPS32; M1 Pro 16 cores 5308; M1 Max 32 cores 10617; M1 Ultra 64 cores 21233; M2 10 cores 3579; M2 Pro 19 cores 6800; M2 Max 38 cores 13600. F16 FLOPS equal F32 on M1+; A14 has half the F32 rate. L2 is small and uneven (M1 768 KB, M1 Pro 256 KB, M1 Max 512 KB); SLC 8–96 MB.
- Per core (Apple 7/8): 384–3072 threads, ~208 KB register file, ~60 KB threadgroup memory, 12 KB instruction cache, 8 KB L1 data cache, 128-byte cache line.
- Per core per cycle: 256 F32 FLOPs (128 FFMA), 128 F32 adds, 128 I32 adds, 32 I32 muls, 32 I64 adds, 8 I64 muls, 32 I32 bit-shifts, 128 bitwise. No hardware FP64.
- Emulated FP64: the table quotes IEEE-style "FP59" (e11m48) emulation at ADD 1:36, MUL 1:52, FMA 1:68 of the F32 rate; non-IEEE e8m48 roughly triples that.
- Dependent-chain penalty: back-to-back dependent FMUL costs 1.84 cycles throughput in 32-bit vs 1.56 in 16-bit. At minimum occupancy combined latency is 6.6 vs 3.9 cycles (FMA 11.3 vs 3.9).
- Note in §ALU Bottlenecks: later benchmarks reduced absolute latency to 3 cycles for FFMA16/FADD32/FMUL32 and 6 cycles for FFMA32.
- Instruction table (adjusted latency, cycles): FADD32/FMUL32/FFMA32 ≈ 2.2 (raw 3.50–3.91), FP16 versions ≈ 2.17. Throughput 1 per cycle per ALU on M1 Max (2 on A14). RECIP32 6.5, RSQRT32 ~9, EXP2/LOG2 ~4.3, DIV32 = RECIP + FMUL (throughput 6), SQRT32 throughput 8. "Precise" RECIP/DIV/SQRT/SIN are 2–10x slower than fast versions.
- Compare/select: FCMPSEL32 and FMAX32/FMIN32 have ~4.74 adjusted latency (vs 2.17 for 16-bit).
- Integer: IADD32 ≈ 2.2; IMUL32/IMAD32 throughput 4; IMULHI32 and 32x32→64 multiply throughput 8; 64x64→64 multiply 16. IADD64 is emulated (about 4 instructions). Write `ulong(x) * ulong(y)` for a widening multiply (8 cycles); splitting into `x*y` plus `mulhi` costs 12.
- Pipelines: four schedulers per core, each issuing one 32-thread SIMD instruction per cycle. Four FP32/int/conditional pipes; one shared "integer and complex" pipe for IMAD, shifts, conversions, EXP2/LOG2, RECIP, RSQRT, 64-bit multiply, SIN.
- Concurrency: IADD64 runs partly alongside IMUL32, which helps FP64 emulation, but dynamic shifts (4 cycles) serialise with IMUL in the complex pipe.
- Occupancy: ALU utilisation maxes out at 24 SIMDs per core, also the lowest occupancy reachable by over-allocating registers. "Apple would rather you spill to device memory".
- Register-cache trick: FFMA chains of the form `x*x + 1` reach ~95 % FP32 utilisation at ILP 1, because only one operand is live.
- Concurrency: M1 Max runs up to 96 compute commands at once within one `MTLComputeCommandEncoder`. Across command queues only 2x concurrency for the whole GPU.
- Power: one active SIMD draws about 800 mW on M1 Max.

## Reliability

- Single-author microbenchmarks (scripts are in the repo, so they can be reproduced). M1/M2 only; nothing on M3 Dynamic Caching or later (see [Apple tech talk: Explore GPU advancements in M3 and A17 Pro (Dynamic Caching)](apple-dynamic-caching-talk.md)).
- Internal contradiction on FP32 latency: the instruction table gives FFMA32 adjusted latency ~2.21 cycles (raw 3.5–3.9), while §ALU Bottlenecks says later, better benchmarks measured 6 cycles for FFMA32 and 3 for FADD32/FMUL32. The ILP tables also show 11.34 cycles for FFMA32 at 4 SIMDs/core. Tracked in [What is the FFMA32 latency on Apple GPUs?](../questions/ffma32-latency.md).
- The "FP59" emulation ratios are estimates from instruction counts, not a measured kernel. They differ from his own forum estimate (see [Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs](apple-forums-fp64-emulation.md)).
- Opinionated passages (core counts, AI, Nanite history) are commentary, not measurement.

## Contradictions

- FFMA32 latency: ~2.21 (table) vs 6 cycles (later note) vs 11.34 (4 SIMDs, ILP 1). See [What is the FFMA32 latency on Apple GPUs?](../questions/ffma32-latency.md).

## Useful sections

- §Overview — chip table (GHz, cores, GFLOPS, caches).
- §On-Chip Memory — registers, threadgroup memory, caches per core.
- §Operations per Second — per-cycle op rates, emulated FP64 ratios.
- §ALU Bottlenecks — dependency penalties, ILP/occupancy tables, the 3/6-cycle note, 24-SIMD saturation.
- §ALU Layout — scheduler and pipeline model.
- §Instruction Throughputs — float, integer, 64-bit integer and mixed-workload tables; widening multiply advice.
- §Power Efficiency — command concurrency, cache sizes.

## Used by

- [Apple GPU architecture for compute](../platform/apple-gpu-architecture.md)
- [Metal compute pipeline for a deep-zoom renderer](../platform/metal-compute.md)
- [Double-float (float-float) arithmetic](../techniques/double-float.md)
- [Extended range: floatexp, rescaled iterations, depth thresholds](../techniques/extended-range-floats.md)
- [What is the FFMA32 latency on Apple GPUs?](../questions/ffma32-latency.md)
- [Which extended-precision emulation is fastest and correct on Apple GPUs?](../questions/double-float-speed-on-m1.md)
