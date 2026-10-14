---
title: "Series approximation (legacy)"
type: technique
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/wikipedia-plotting-algorithms]]", "[[sources/philthompson-bla]]", "[[sources/kf-manual]]", "[[sources/kf-page]]", "[[sources/desarso-mandelbrot-webgpu-readme]]"]
tags: [iteration-skipping, series-approximation, legacy]
---

# Series approximation (legacy)

Series approximation (SA) skips the first iterations of every pixel at once
by evaluating a polynomial in `c` whose coefficients come from the
reference. It was the main acceleration from 2013 to 2021. For the
Mandelbrot set it is superseded by [[techniques/bla]] and
[[techniques/linear-approximation-imagina]]. almondbread should not
implement it.

## Formula

Iterating the perturbed step gives a polynomial in `c` with no constant term
(src: [[sources/mathr-deep-zoom-theory]] §Series Approximation):

```text
z_n = Σ_{k≥1} A_{n,k} c^k
```

Substituting into `z_{n+1} = 2 Z_n z_n + z_n^2 + c` and equating powers of
`c` gives the recurrences. Low orders, as Wikipedia writes them with
`A_1 = 1, B_1 = 0, C_1 = 0` (src: [[sources/wikipedia-plotting-algorithms]] §Perturbation theory and series approximation):

```text
A_{n+1} = 2 Z_n A_n + 1
B_{n+1} = 2 Z_n B_n + A_n^2
C_{n+1} = 2 Z_n C_n + 2 A_n B_n
general: coefficient k  ←  2 Z_n a_k + Σ_{i+j=k} a_i a_j   (+1 for k = 1)
```

Wikipedia claims about two orders of magnitude speedup
(src: [[sources/wikipedia-plotting-algorithms]] §Perturbation theory and series approximation).

## Choosing the skip count

- Probe points: run the series against regular or perturbed iterations at a
  set of probe points; when it starts to deviate, roll back one iteration and
  initialise every pixel from the series there
  (src: [[sources/mathr-deep-zoom-theory]] §Series Approximation).
- No rigorous error bound exists: "no complete mathematical proof of
  correctness with rigourous error bounds", though images "do most often
  look plausible" (src: [[sources/mathr-deep-zoom-theory]] §Conclusion).
- KF's term-count and probe heuristics were not retrieved; term counts in
  KF-era renderers were configurable (unverified).

## Why it lost

- One skip count for the whole image: the worst pixel limits all of them
  (inference).
- Silent failure: probe points can miss the region where truncation error
  blows up (inference).
- Abs formulas: SA becomes two bivariate real series and holds only until a
  fold, so "typically only a few iterations can be skipped"
  (src: [[sources/mathr-deep-zoom-theory]] §Series Approximation).
- BLA is "conceptually simpler, easier to implement, easier to parallelize,
  has better understood stopping conditions, is more general"
  (src: [[sources/mathr-deep-zoom-theory-again]] §Bilinear Approximation).
- SA only skips the initial segment; a BLA step applies anywhere in the
  orbit. The mandelbrot-webgpu author rejected SA for that reason
  (src: [[sources/desarso-mandelbrot-webgpu-readme]] §Measured and rejected).
- KF's author points users from SA-based KF to BLA renderers
  (src: [[sources/kf-page]]).

## Measured comparison

Very Plotter (JavaScript, 7 workers, M1), BLA vs perturbation+SA
(src: [[sources/philthompson-bla]] §Speed Comparison):

| Location | Max iterations | BLA | SA | BLA speedup |
| --- | --- | --- | --- | --- |
| 2.9e89 | 350k | 99 s | 1009 s | 10.2× |
| 4.99e152 | 2M | 358 s | 12,958 s | 36.2× |
| 1.8e157 | 50.1k | | | 1.5–2.4× |
| 5.62e227 | 1.2M | | | 1.7× |
| 9.5e693 (floatexp) | 350k | 3438 s | 2076 s | 0.6× (SA faster) |

At 5.62e227 BLA skipped about 989k iterations per pixel and SA about 1,064k.
At 9.5e693 SA won at every resolution tested, and BLA needed ε lowered to
about 1e-122 (src: [[sources/philthompson-bla]] §Example Location 3). This is one naïve
binary-tree BLA with a tuned ε, so it is weak evidence. Note that Very
Plotter's "float" means a JavaScript double; its SA runs used 6 to 9 terms
(`perturb-sapx6` to `sapx9`). A size sweep at 9.5e693 fitted BLA at 422 s +
4562 s/MP and SA at 158 s + 3915 s/MP, so SA won at every size there
(src: [[sources/philthompson-bla]] §Example Location 3); Imagina and
Fraktaler 3 ship no SA for the Mandelbrot set (notes: iteration_skipping §1).

## Related legacy methods

- **NanoMB1** (knighty): a bivariate series in `z` and `c` around a periodic
  reference (minibrot nucleus) that skips a whole period per evaluation,
  repeated until an escape radius; in KF
  (src: [[sources/mathr-deep-zoom-theory]] §Series Approximation).
- **NanoMB2**: chains minibrots at successive depths, rebasing outward;
  "highly experimental and fails for many locations"
  (src: [[sources/mathr-deep-zoom-theory]] §Series Approximation).
  Its idea survives robustly as the AT step of
  [[techniques/linear-approximation-imagina]].
- KF's rescaled SA for power 2 was about 30% faster than all-floatexp SA
  (src: [[sources/kf-manual]] §kf-2.15.3).

## For almondbread

Skip SA and nanoMB entirely. Implement [[techniques/bla]] first, then
[[techniques/linear-approximation-imagina]]. Re-test the SA counter-example
only if our own BLA loses at floatexp depths ([[questions/bla-vs-la-benchmark]]).
