---
title: "How does Imagina/FractalShark reference-orbit compression work?"
type: question
status: answered
sources: ["[[sources/fractalshark-readme]]", "[[sources/imagina-readme]]", "[[sources/imagina-hinfla-evaluator]]"]
tags: [reference-orbit, memory, compression, ios]
---

# How does Imagina/FractalShark reference-orbit compression work?

## Question

What is the algorithm behind reference-orbit compression (Imagina, extended by
FractalShark to decompress on the fly while rendering), what ratio does it reach,
and what does decompression cost per pixel step?

## Why it matters

Memory is the main limit for the iPhone/iPad port
([[platform/ios-ipad-considerations]]) and for very long references
([[techniques/reference-orbit-precision]]).

## What sources say

- Reference-orbit compression was first implemented in Imagina for save/load;
  FractalShark decompresses segments on demand during rendering; a period
  600,000,000 case saved multiple GB; files are Imagina-compatible; details are in
  FractalShark.pdf §14 (src: [[sources/fractalshark-readme]] §6. Reference orbit compression and on-the-fly decompression).
- The Imagina README does not describe it (src: [[sources/imagina-readme]]).
- Inferred mechanism (unverified): store sparse exact "waypoints" and regenerate
  the low-precision orbit between them by iterating `Z ← Z² + C` in double,
  re-seeding when the recomputation drifts past a tolerance (notes: perturbation_core §6).

## Answer (2026-10-14, from Imagina's source)

Imagina's `SaveCompressedOrbit` / `LoadCompressedOrbit` (AGPL; described in
our own words) confirm and sharpen the waypoint idea
(src: [[sources/imagina-hinfla-evaluator]] §SaveCompressedOrbit):

1. **Before the first near-zero.** Re-iterate the orbit in low precision
   (`z ← z² + c` in double) and compare with the stored orbit. Whenever the
   relative drift `‖z − Z_i‖∞ / ‖Z_i‖∞` exceeds `2^-q` (q = a "reference
   quality" setting), store a waypoint `(i, Z_i)` and re-seed `z = Z_i`. Stop
   at the first iteration where `‖Z_i‖∞ < 2^-4`, which is stored as a
   waypoint flagged as a rebase.
2. **After it.** The rest of the orbit is regenerated as a *perturbation of
   its own beginning*: a delta against `Z_j` with its own reference index
   `j`, using the ordinary perturbed step and rebasing (`j ← 0` when
   `‖Z_j + dz‖∞ < ‖dz‖∞`). Waypoints are stored when the regenerated value
   drifts past the same tolerance (or when `j` runs past the last waypoint),
   and the iterations where natural rebases happen are stored in a separate
   list, so the loader can replay exactly the same rebase decisions.
3. **Loading** replays both phases from the waypoint and rebase lists, so
   the file holds only a sparse set of exact values plus integers.

Why it compresses well (inference): a periodic or nearly periodic reference
is mostly a perturbation of its own early iterations, so after the first
near return to 0 very few corrections are needed. FractalShark extends this
to decompress segments on demand during rendering
(src: [[sources/fractalshark-readme]] §6. Reference orbit compression and on-the-fly decompression);
its exact segment scheme (FractalShark.pdf §14) is still unread.

## Still open

How FractalShark decompresses on demand during rendering, and the
compression ratios on typical (non-pathological) orbits. Read FractalShark.pdf
§14, then measure on our own references. Write-up lives in
[[techniques/reference-orbit-precision]].
