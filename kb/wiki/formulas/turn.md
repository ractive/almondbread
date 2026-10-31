---
title: "Turned planes of the (c, z0) space (Mandelbrot ↔ Julia)"
type: formula
status: draft
sources: ["sources/fractalforums-zhuoran-rebasing-bla"]
tags: [formula, mandelbrot, julia, perturbation, rebasing, bla]
---

# Turned planes of the (c, z0) space (Mandelbrot ↔ Julia)

## Iteration

`z_{n+1} = z_n^2 + c` has two complex inputs, the start `z_0` and `c`: a 4-dimensional real
space. The [Mandelbrot set](mandelbrot.md) is its plane `z_0 = 0` (the pixel is `c`), a
[Julia set](julia.md) the plane `c = k` (the pixel is `z_0`). The idea of turning the view plane
from one to the other is the user's, after 2swap's video "Mandelbrot's Evil Twin" (unverified:
we have not saved or read the video).

Our parametrisation (F9, [F9: The turn between the Mandelbrot and the Julia plane](../plans/f9-turn.md)): a plane through a pivot `(c*, z*)`,
turned by `t ∈ [0°, 90°]`; the pixel at view coordinate `p` is

```text
c   = c* + cos t (p - P)
z_0 = z* + sin t (p - P),     P = cos t c* + sin t z*
```

so `p` is the coordinate along the turned axis `(cos t, sin t)` (the same real rotation of
both real and imaginary parts) and `-sin t c + cos t z_0` is constant on the plane. At `t = 0`
the view coordinate is `c` (the Mandelbrot plane with start `z*`); at `t = 90°` it is `z_0` (the
Julia set of `k = c*`, in F4's viewport). Turning keeps the view centre's point of the 4-space:
the new pivot is that point, so the picture turns about the middle of the screen and turning
back returns exactly (our design).

## Perturbed form (our derivation)

Reference: the orbit of the reference point's `(C, Z_0)` (`Z_{n+1} = Z_n^2 + C`). A pixel
`dp` from it starts at `dz_0 = sin t dp` with `dc = cos t dp`:

`z_{n+1} = 2 Z_n z_n + z_n^2 + dc`, `z_0 = dz_0`,

Mandelbrot's step (with its `+dc`) from Julia's start. Derivative along the plane (distance
estimate): `d_0 = sin t`, `d_{n+1} = 2 z_n d_n + cos t`.

## Rebasing

Zhuoran's rebase needs an orbit point equal to 0 with the pixel's own dynamics
(src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #10). Every pixel has its own `c`, but the step is
`z ↦ z^2 + c` for all of them: the orbit of 0 under `z^2 + C` (the Mandelbrot orbit of the
reference's `C`) shares the reference's `C`, so `z ← Z_m + z` onto its index 0 keeps `dc`
unchanged. A turned reference is therefore F4's two-orbit layout (critical orbit, zeros,
centre orbit from `Z_0`) with Mandelbrot's step (measured, [F9: The turn between the Mandelbrot and the Julia plane](../plans/f9-turn.md) "Gate").
With `z* = 0` and the reference at the pivot the two orbits coincide.

## BLA

`z_{m+l} = A z_m + B dc` holds for any `z_m`, so Mandelbrot's table (single steps `A = 2Z`,
`B = 1`, validity `|z| + K |dc| < R`) serves turned views with `dc = cos t dp`; built on the
concatenated reference, where the zeros between the orbits invalidate straddling blocks. In
the general form `z_n = A_n dz_0 + B_n dc` (Mandelbrot `dz_0 = 0`, Julia `dc = 0`), the table
covers the part after the first step: blocks start at aligned indices `m ≥ 1`, never at the
centre orbit's start. LA (power-2 Imagina) and local SA seeds are off: their expansions are in
`c` from `z_0 = 0`.

## Near the critical point

A pixel at `z ≈ 0` loses its offset to the square: neighbours `Δ` apart are `2|z| Δ + Δ^2 +
cos t Δp` apart one step later. With `cos t ≈ 0` (a Julia set, a plane near 90°) and a view at
`z_0 ≈ 0` (where turning a Mandelbrot view lands), differences shrink to `texel^2`: the
reference needs about twice the bits, and the deep kernel must not rescale such a delta to the
pixel's offset. Found by the gate (measured; [F9: The turn between the Mandelbrot and the Julia plane](../plans/f9-turn.md) "Near the critical point"), fixed for
F4's Julia sets too.

## Interior

The derivative test `|dz_n/dz_1| n^2 < eps` holds (an interior pixel's own orbit falls into
an attracting cycle) when the first step from `z_0` is not multiplied in (`2 z_0` is tiny at
`z_0 ≈ 0`). The attracting-cycle probe, the closed-form cardioid and bulb test and the interior
cycle analysis all assume `z_0 = 0`: off.

## What the turn looks like at depth (our observation)

With `z* = 0` the start enters quadratically (`z_1 = z_0^2 + c`): a deep Mandelbrot view turned
by `t < 90°` is the Mandelbrot picture stretched by `1 / cos t` with a correction of order
`(sin t dp)^2`, so it widens and only changes character close to 90°, where it becomes the Julia
set of the centre's `c` around 0 (the Julia set near `k` at the squared scale). The morph is
rich at shallow depth and with a start value `z* ≠ 0` on the Julia set (inference from the
maths above and the gate pictures).

# Citations

- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [F9: The turn between the Mandelbrot and the Julia plane](../plans/f9-turn.md) (our derivation and measurements)
- [Mandelbrot set (power 2)](mandelbrot.md)
- [Julia sets](julia.md)
