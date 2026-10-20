---
title: "Glitch detection (Pauldelbrot) and multi-reference correction (legacy)"
type: technique
status: draft
sources: ["[[sources/mathr-deep-zoom-theory]]", "[[sources/kf-manual]]", "[[sources/wikipedia-plotting-algorithms]]", "[[sources/ultrafractal-perturbation-calculations]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]"]
tags: [perturbation, glitches, legacy]
---

# Glitch detection (Pauldelbrot) and multi-reference correction (legacy)

Before rebasing (2021), perturbation renderers detected pixels whose delta
had lost precision and re-rendered them against new references. For
power-2 Mandelbrot this is superseded by [Rebasing (Zhuoran, 2021)](rebasing.md). It stays
useful as a diagnostic overlay and as a fallback for formulas where rebasing
is not derived.

## What a glitch is

Glitched areas look "noisy" or show flat blobs that do not belong. Adding a
reference inside the glitch and recomputing fixed them, but there was no
reliable automatic detection until Pauldelbrot's criterion
(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Glitches).

Derivation: perturb the perturbed iteration once more, `z → z + e`,
`c → c + f`, giving `e → (2(Z + z) + e) e + f`. The ratio `e/z` changes per
step by about `2(Z + z) / 2Z`. When it becomes small, neighbouring pixels
"stick together" and the number type cannot tell them apart
(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Glitches).

## Test

```text
glitch if |Z + z|^2 < G * |Z|^2
```

- G "somewhere between 1e-2 and 1e-8, depending how strict you want to be".
- `|Z + z|^2` is already computed for the bailout test, and `G·|Z|^2` can be
  stored per reference iteration, so the test is nearly free.
- Too large a G flags glitches everywhere (slow); too small misses some (bad
  images). How to choose G "remains open"
  (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Glitches; §Conclusion).
- Pauldelbrot's original "magic number" was 0.001 (notes:
  perturbation_core §2, citing a fractalforums.com KF 2.7 thread; not saved).

For float32 (ε = 2^-24 ≈ 6e-8) the strict end G = 1e-8 is below the
resolution of the type (inference).

## Multi-reference correction (KF)

- Glitched pixels are re-rendered from a new reference chosen among them;
  this may produce new glitches, so it repeats
  (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Glitches).
- KF options: reference selection "Original", "argmin|z|", "Random"; maximum
  references default and hard limit 10000; "special locations can produce
  endless references" (notes: perturbation_core §2, citing the KF manual).
- KF "glitch low tolerance" detects more aggressively; on one location it
  made a correct render possible but took 16× longer. Since 2.15.3 it is a
  fraction between 0 (fast, can be wrong) and 1 (good, slow)
  (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md) §kf-2.15.3).
- "Ignore isolated glitches" trades accuracy for speed. At Dinkydau's
  "Flake", KF needed 97 references with it off and 22 with it on; the
  single-reference rebasing method was closer to the 97-reference image than
  the 22-reference one, and much faster
  (src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #9).
- Ultra Fractal 6 still detects precision problems automatically and adds
  references (src: [Ultra Fractal help: Perturbation calculations](../sources/ultrafractal-perturbation-calculations.md) §Perturbation calculations).
- Wikipedia describes the same idea: measure how far the low-precision point
  strays from the reference and recompute such points from a closer
  reference (src: [Wikipedia: Plotting algorithms for the Mandelbrot set](../sources/wikipedia-plotting-algorithms.md) §Perturbation theory and series approximation).

## For almondbread

Do not build multi-reference correction. Keep the Pauldelbrot test as a
debug overlay (colour pixels where it fires) to catch float32 paths that go
wrong silently, and as a fallback flag for formulas without a rebasing
derivation ([Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md)).
