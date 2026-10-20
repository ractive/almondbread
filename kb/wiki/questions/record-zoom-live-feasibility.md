---
title: "Can almondbread play a world-record zoom live, without post-processing?"
type: question
status: open
tags: [performance, video, exponential-map, linear-approximation, roadmap]
sources: ["[[sources/maths-town-videos]]", "[[sources/mathr-deep-zoom-theory-again]]"]
---

# Can almondbread play a world-record zoom live, without post-processing?

Asked by the user on 2026-10-14. Driver estimate, not measured.

## Estimate

- One reference for the whole path: the target minibrot's nucleus orbit
  (one period) serves every frame through rebasing. At 1e1300 about 4400
  bits; the Rust core does about 300k iterations/s at 4096 bits
  ([almondbread benchmarks and verification record](../synthesis/benchmarks.md)), so period 1e6 takes about 3 s, period
  1e8–1e9 minutes to an hour, once, before playback.
- Exponential-map rendering ([Exponential map (log-polar) rendering](../techniques/exponential-map.md)): one keyframe
  per zoom doubling, intermediate frames reprojected. 1e1300 is about 4300
  doublings; at one doubling per second the video lasts about 72 minutes
  and each keyframe may take up to 1 s.
- With LA + AT ([Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md)), a 4K
  keyframe at about 1000 steps per pixel is about 4e11 operations, around
  0.1 s on the M1 Pro GPU (5.3 TFLOPS), 0.3–0.5 s with rescaled
  float32 / floatexp (inference).
- Record videos took weeks to months with KF2+ and series approximation
  ([Maths Town videos and video descriptions](../sources/maths-town-videos.md)); BLA-era renderers are already about
  10× faster.

## Verdict (provisional)

Live replay of a record path to about 1e1000–1e3000 at 1440p–4K looks
plausible after a precompute of seconds to minutes; free flight is fine
near one minibrot but switching to a new deep minibrot costs seconds to
minutes; 8K heavily supersampled output stays an export job; beyond about
1e10000 the reference cost rules out live playback.

## How to answer it

Acceptance test after M6 (LA + AT) and exponential-map playback: take a
published record location, measure precompute time, keyframe time per
doubling at 1440p and 4K, live playback rate, and image error against an
offline high-quality render. Depends on
[How large is float32 perturbation error over long orbits?](float32-error-long-orbits.md).
