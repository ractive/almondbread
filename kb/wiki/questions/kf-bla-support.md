---
title: "Does Kalles Fraktaler 2+ implement BLA?"
type: question
status: open
sources: ["[[sources/kf-page]]", "[[sources/muency-bla]]", "[[sources/kf-manual]]", "[[sources/fractalforums-interior-detection]]"]
tags: [kalles-fraktaler, bla, contradiction]
---

# Does Kalles Fraktaler 2+ implement BLA?

## Question

Does any released KF2+ version use bivariate linear approximation, or only
perturbation with series approximation (plus NanoMB1/2)?

## Why it matters

It decides whether KF2+ can serve as a BLA reference implementation for almondbread
and how to describe it in [Kalles Fraktaler 2 +](../implementations/kalles-fraktaler-2.md).

## Contradictions

- The KF page says BLA is "much better" than series approximation, that KF does
  not implement it and is unlikely to, and points to Imagina and fractalshades for
  BLA (src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)).
- Mu-Ency says newer versions of Kalles Fraktaler and contemporaneous deep-zoom
  programs use BLA (src: [Mu-Ency: Bivariate Linear Approximation](../sources/muency-bla.md)).
- In June 2022 mathr benchmarked "KF 2.15.5.2 CPU (no BLA, just regular SA)"
  against his "new program" with BLA (src: [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](../sources/fractalforums-interior-detection.md)),
  which supports the KF page.

## Likely answer

KF2+ (last release 2.15.5.2, 2022-06-07) has no BLA; Mu-Ency probably conflates
KF with mathr's successor Fraktaler 3 ([Fraktaler 3](../implementations/fraktaler-3.md)). Mark as
answered once the KF2+ changelog in [Kalles Fraktaler 2 + manual](../sources/kf-manual.md) is checked for "BLA".
