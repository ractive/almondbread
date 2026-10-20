---
title: "Mu-Ency (Robert Munafo): pi and R2.C(1/2)"
type: source
authors: ["Robert P. Munafo"]
year: 2024
raw: "web/muency-r2c12.html"
status: digested
tags: [pi, neck, encyclopedia]
---

# Mu-Ency: pi and R2.C(1/2)

**URLs:** <https://www.mrob.com/pub/muency/r2c12.html> (the π passage) and
<https://www.mrob.com/pub/muency/pi.html> (a short pointer page, 2002/2003) · saved
`raw/web/muency-r2c12.html` and `raw/web/muency-pi.html` (+ `.txt`), retrieved 2026-10-15 ·
**Terms:** CC BY-NC 4.0.

## What it is

Munafo's Mandelbrot set encyclopedia. The "pi" entry only points to the R2.C(1/2) entry, the
cusp between the cardioid and the period-2 disc, which has a "Relation to Pi" paragraph.

## Useful parts

- The vertical line at -3/4 contains no member point except -3/4 itself (stated, no proof; the
  question is open per [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](brockmoeller-pi-everywhere.md) Thm. 2 discussion).
- A table for `c = -3/4 + i/x`: x = 10 ... 100000 gives 34, 316, 3144, 31418, 314161
  iterations, "x times π plus error" of about 2. Each is one more than Boll's count
  ([Dave Boll's 1991 π post, reproduced by Mark VandeWettering (brainwagon, 2011)](boll-pi-post.md)): a different counting convention
  ([Why do published π escape counts differ by one (Mu-Ency) or disagree at ε = 1e-7 (Boll)?](../questions/pi-escape-count-conventions.md)).
- Explanation offered: `(1 + εi)^{π/ε} ≈ -1` (Euler), and the relation is said to occur "in
  all the cusps", matching the general theorem of [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](brockmoeller-pi-everywhere.md).
- No escape radius stated, no references beyond the encyclopedia itself.

## Reliability

An informal encyclopedia entry; the counts are consistent with ours up to the +1 convention.
