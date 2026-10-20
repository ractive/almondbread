---
title: "Dave Boll's 1991 π post, reproduced by Mark VandeWettering (brainwagon, 2011)"
type: source
authors: ["Dave Boll", "Mark VandeWettering"]
year: 2011
raw: "web/brainwagon-boll-pi.html"
status: digested
tags: [pi, parabolic, escape-time, history]
---

# Dave Boll's π post (via brainwagon)

**URL:** <https://brainwagon.org/blog/happy-day/> (pi day 2011) · saved
`raw/web/brainwagon-boll-pi.html` (+ `.txt`), retrieved 2026-10-15 · **Terms:** blog post,
copyright the authors; the counts are facts.

## What it is

Mark VandeWettering's blog post quotes Dave Boll's 1991 sci.math posting (first sent to
alt.fractals) and adds a short C program. Boll wanted to show that the neck of the set at
(-.75, 0) has zero thickness, so he counted the iterations of points (-.75, dy) before they
"blew up": 33, 315, 3143, 31417, 314160 for dy = 0.1 ... 0.00001, and asked what the product
with dy looks like.

## Useful parts

- The original observation and table (neck only; Klebanoff quotes a longer version with the
  cusp, [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](klebanoff-pi-in-mandelbrot.md)).
- The escape convention of the program: `z = 0`, then for `i = 1, 2, ...` `z = z² + c` and stop
  when `|z| > 2`; the count is that i. Our counts match Boll's table with this convention
  ([π in the Mandelbrot set (neck and cusp escape counts)](../techniques/pi-in-the-mandelbrot-set.md)).
- A numerical pitfall worth keeping: the post's own program, in complex double, gives a wrong
  result at the last row (dy = 1e-9: "the Mandelbrot iteration runs to limit, instead of
  stopping where it should"); the author suspected precision. Our measurements reproduce this:
  textbook f64 never escapes from ε = 1e-8 on.

## Reliability

A second-hand quote of a Usenet post; the numbers agree with our 256-bit reference counts.

# Citations

- <https://brainwagon.org/blog/happy-day/>
- [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](klebanoff-pi-in-mandelbrot.md)
