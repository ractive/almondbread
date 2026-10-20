---
title: "Mu-Ency: Adjacency optimization (with Mariani/Silver and circle tiling)"
type: source
authors: ["Robert P. Munafo"]
year: 2020
raw: "web"
status: digested
tags: [guessing, mariani-silver, circle-tiling, distance-estimation, adjacency-optimization, muency]
---

# Mu-Ency: Adjacency optimization (with Mariani/Silver and circle tiling)

**URL:** <https://mrob.com/pub/muency/adjacencyoptimization.html> · **Saved:** not saved · **Terms:** copyright the author (some Mu-Ency pages state CC BY-NC 4.0)

## What it is

Robert Munafo's Mandelbrot glossary entry comparing region-skipping methods,
read with its linked entries on the Mariani/Silver algorithm
(<https://mrob.com/pub/muency/marianisilveralgorithm.html>, 2010, updated
2020) and circle tiling (<https://mrob.com/pub/muency/circletiling.html>,
1996, marked unfinished).

## What it covers

- Methods: successive refinement with solid guessing, boundary scanning,
  Mariani/Silver (rectangle subdivision), circle tiling.
- Successive refinement "can miss narrow features such as cusps". Boundary
  scanning and Mariani/Silver can miss features when the maximum dwell is too
  low. For the fewest missed pixels, use Mariani/Silver.
- Mariani/Silver: compute a rectangle's border; if all equal, fill; else split
  into halves or quarters and recurse (halves reuse three known edges). Often
  misses cusps narrower than one pixel. Non-rectangular splits (triangles,
  pentagons) also used.
- Circle tiling: an exterior or interior DE gives a disc that lies wholly
  outside or inside the set; points on its edge seed new discs. It never marks
  a set point as exterior if DE is computed correctly, and is the only
  adjacency method without the sub-pixel-cusp flaw. But it shows dwell bands
  badly, performs poorly where box-counting dimension is high, must ignore
  discs below a minimum size, and cannot reach areas behind narrow channels.
- Origin of disc filling given as *The Beauty of Fractals*.

## Reliability

Secondary reference by a long-time Mandelbrot programmer. Qualitative only:
no costs, no proofs, the circle tiling page is unfinished. The Mariani/Silver
page gives no history (inventors, dates).

## Useful sections

- Adjacency optimization: method comparison and failure modes.
- Mariani/Silver Algorithm: subdivision rule, sub-pixel cusp flaw.
- Circle Tiling: DE disc guarantee and its limits.

## Used by

[Adaptive and sparse sampling, reconstruction, early termination](../techniques/adaptive-sampling.md)

# Citations

- <https://mrob.com/pub/muency/adjacencyoptimization.html>
- <https://mrob.com/pub/muency/marianisilveralgorithm.html>
- <https://mrob.com/pub/muency/circletiling.html>
