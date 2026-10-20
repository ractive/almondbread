---
title: "fractalforums.org: Another solution to perturbation glitches (topic 4360)"
type: source
authors: ["Zhuoran", "fractalforums.org members"]
year: 2021
raw: "web/fractalforums-zhuoran-rebasing-bla.html"
status: digested
tags: [rebasing, perturbation, glitch-detection, bla, forum]
---

# fractalforums.org: Another solution to perturbation glitches (topic 4360)

**URL:** <https://web.archive.org/web/20230125202704/https://fractalforums.org/f/28/t/4360> · **Saved:** `raw/web/fractalforums-zhuoran-rebasing-bla.html` (+ `.txt`) · **Terms:** copyright the author; personal research copy, not redistributed

## What it is

Archived forum thread where Zhuoran (later author of Imagina) posted the rebasing idea on
3 Aug 2021. Only page 1 of 10 is saved (posts up to 5 Nov 2021); the later pages, where BLA was
hinted at and discussed, are not in the raw file.

## What it covers

- Zhuoran's pseudocode (opening post): reference orbit array must start with 0; per pixel keep `dz`, `dc`, iteration and reference iteration counters. Each step: `dz = 2 dz Z[ref] + dz^2 + dc`, `ref++`, `z = Z[ref] + dz`; break on bailout; if `|z| < |dz|` or `ref == MaxRefIteration` then `dz = z`, `ref = 0`. (Our paraphrase; the post is pseudocode, not GPL code.)
- Claimed: needs no extra references, very reliable "even when single precision is used". Example rendered correctly in single float: `Re=0.2500000002`, `Im=3.5e-15`, height `4e-15`, 5,000,000 iterations.
- Note the second rebase trigger: running out of reference orbit (`ref == MaxRefIteration`) also rebases, so a short reference still works.
- claude (Reply #1): similar to his mandelbrot-perturbator (rebase to a new reference), but reusing the original reference.
- claude (Reply #6) explains why it works: `Z_pixel = Z_ref1 + dz1 = Z_ref2 + dz2`, so `dz2 = Z_ref1 + dz1 - Z_ref2`; with `Z_ref2 = Z_0 = 0` this is just `dz2 = Z_pixel`, valid because the reference starts at 0 at iteration 0. `dc` is unchanged because `C_ref` is unchanged.
- claude (Reply #7): one reference looks almost identical to 338 references at a `9.13e93` location with 50,100,100 iterations, much faster.
- claude (Reply #9): Dinkydau's Flake: 1 reference 18.5 s vs 97 references several minutes; 22 references (ignore isolated glitches) 29.8 s; 1-ref result closer to the 97-ref image than 22-ref is.
- claude (Reply #10): only needs the perturbation property and an iteration where the reference is 0 (the critical point at iteration 0); nothing Mandelbrot-specific.
- superheal (Reply #8, #11): works for Mandelbrot powers 2-5, Burning Ship, lambda, nova, magnet, Newton parameter space, and Julia sets.

## Reliability

Primary source for rebasing (the original post). Claims of reliability are empirical, from
a few locations; no proof. Image attachments are not saved (Reply #12-14 confusion is about
difference images). Since only page 1 is archived, statements about BLA "hinted at in the thread"
([mathr: Deep zoom theory and practice (again) (2022)](mathr-deep-zoom-theory-again.md)) cannot be checked here. Mu-Ency's date of "2021 Dec"
for this idea is wrong; the post is dated 3 Aug 2021.

## Useful sections

- §Another solution to perturbation glitches (opening post, 3 Aug 2021) — the rebasing pseudocode.
- §Reply #2 — single-precision example location.
- §Reply #6 — derivation of why rebasing to iteration 0 is valid.
- §Reply #7, §Reply #9 — 1 vs 338 and 1 vs 22 vs 97 reference comparisons.
- §Reply #8, §Reply #10 — generality across formulas.

## Used by

[Rebasing (Zhuoran, 2021)](../techniques/rebasing.md), [Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](../techniques/glitch-detection.md), [Perturbation](../techniques/perturbation.md),
[Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md), [Imagina](../implementations/imagina.md), [Kalles Fraktaler 2 +](../implementations/kalles-fraktaler-2.md),
[How large is float32 perturbation error over long orbits?](../questions/float32-error-long-orbits.md)

# Citations

- <https://web.archive.org/web/20230125202704/https://fractalforums.org/f/28/t/4360>
- <https://fractalforums.org/f/28/t/4360>
- [mathr: Deep zoom theory and practice (again) (2022)](mathr-deep-zoom-theory-again.md)
