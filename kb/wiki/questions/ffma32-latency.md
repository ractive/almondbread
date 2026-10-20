---
title: "What is the FFMA32 latency on Apple GPUs?"
type: question
status: open
sources: ["[[sources/metal-benchmarks-readme]]"]
tags: [apple-gpu, latency, contradiction]
---

# What is the FFMA32 latency on Apple GPUs?

## Question

Is the fp32 FMA latency on M1-class GPUs about 2.2 cycles or about 6 cycles, and
how many independent chains (ILP) or resident SIMD-groups hide it?

## Contradictions

metal-benchmarks lists FFMA32 with raw latency 3.50 to 3.91 and adjusted latency
2.21 cycles in its instruction table, while its ALU bottlenecks section says those
tables "reflect a sub-optimal shader setup" and later benchmarks gave absolute
latencies of 6 cycles for FFMA32 and 3 for FFMA16, FADD32 and FMUL32
(src: [metal-benchmarks README (Apple GPU microarchitecture)](../sources/metal-benchmarks-readme.md) §ALU Bottlenecks, §Instruction Throughputs).
"Adjusted" subtracts a measured overhead, so the figures are not like-for-like.

## Why it matters

The perturbation step is a short dependent chain (complex square, add, norm). With
6-cycle latency, a kernel needs several independent pixels per thread or high
occupancy to keep the ALUs busy, which argues for 2 pixels per thread in the fp32
path ([Apple GPU architecture for compute](../platform/apple-gpu-architecture.md)) (inference).

## How to resolve

Microbenchmark on the M1 Pro: dependent FFMA32 chain at 1, 2, 4, 8 SIMD-groups per
core and ILP 1 to 4; then time the real kernel with 1 vs 2 pixels per thread.
