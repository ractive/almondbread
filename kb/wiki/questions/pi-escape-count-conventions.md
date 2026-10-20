---
title: "Why do published π escape counts differ by one (Mu-Ency) or disagree at ε = 1e-7 (Boll)?"
type: question
status: answered
sources: ["[[sources/klebanoff-pi-in-mandelbrot]]", "[[sources/boll-pi-post]]", "[[sources/muency-pi]]"]
tags: [pi, escape-time, conventions, precision]
---

# π escape counts: conventions and one bad table entry

## Question

Three published tables of neck escape counts for `c = -3/4 + iε` disagree:

- Boll as quoted by Klebanoff: 33, 315, 3143, 31417, 314160, 3141593, **31415928**
  (ε = 1e-1 ... 1e-7; src: [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](../sources/klebanoff-pi-in-mandelbrot.md) p. 394).
- Boll's 1991 post: the same first five ([Dave Boll's 1991 π post, reproduced by Mark VandeWettering (brainwagon, 2011)](../sources/boll-pi-post.md)).
- Mu-Ency: 34, 316, 3144, 31418, 314161 (ε = 1e-1 ... 1e-5; src: [Mu-Ency (Robert Munafo): pi and R2.C(1/2)](../sources/muency-pi.md)).

## Answer

- **Mu-Ency's +1** is a counting convention, not a different set: every entry is exactly one
  more than Boll's. Counting from `z_0 = c` (or including the start) shifts N by one. The
  product's limit does not care (an O(1) shift changes N·ε by O(ε)), and
  [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](../sources/brockmoeller-pi-everywhere.md) notes that the escape radius shifts N by a constant
  too. The app uses Boll's convention (N = first n ≥ 1 with `|z_n| > 2`, `z_0 = 0`), which
  reproduces all his other entries at both points.
- **31415928 at ε = 1e-7** is not reproduced by anything we ran: the 256-bit BigFloat orbit
  with c exact, double-double, the compensated count and plain f64 all give **31415927**
  (measured 2026-10-15, M1 Pro, `pi_check` example; [π in the Mandelbrot set (neck and cusp escape counts)](../techniques/pi-in-the-mandelbrot-set.md)).
  Most likely Boll's program (1991, probably single or double precision in `z² + c` form)
  drifted at that depth, as textbook f64 visibly does one decade further down (it never escapes
  at ε = 1e-8). (unverified: we do not have Boll's program.)
