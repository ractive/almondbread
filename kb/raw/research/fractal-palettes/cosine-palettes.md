# Cosine (procedural) palettes, after Inigo Quilez

Accessed 2026-10-14. Sources:

- Inigo Quilez, "palettes": https://iquilezles.org/articles/palettes/
- Shadertoy examples linked from it: https://www.shadertoy.com/view/Xl2GRc (the palette demo),
  https://www.shadertoy.com/view/lsfGDB, https://www.shadertoy.com/view/XdjXWK,
  https://www.shadertoy.com/view/Mdl3RH
- Inigo Quilez, "smooth iteration count" (context for fractal use):
  https://iquilezles.org/articles/msetsmooth

Licence: the article page states no licence or copyright terms (only the site tagline). Treat
its text and its example parameter sets as all-rights-reserved: the formula is mathematics and
free to use, but we write our own parameter sets and do not copy his seven examples. Shadertoy
code is CC BY-NC-SA 3.0 by default unless the shader says otherwise (Shadertoy's terms; not
re-checked today), which is incompatible with commercial use, so no Shadertoy code either.

## The formula

```
colour(t) = a + b * cos(2*pi * (c*t + d))      # a, b, c, d are RGB triples
```

- a: offset (mean colour, overall brightness);
- b: amplitude per channel (contrast; a +- b should stay within 0..1 or the result clips);
- c: frequency per channel (how many oscillations as t goes 0 -> 1);
- d: phase per channel (where each channel's cycle starts).

12 numbers describe a whole palette; evaluation is a few multiply-adds and three cosines, so it
is cheap in a shader and needs no texture.

## How the parameters behave

- With a = b = 0.5 grey and c = 1 for all channels, d alone sets the character: phases spread
  by a third of a cycle give a rainbow-like hue wheel; small phase differences give a
  two-tone gradient (channels nearly in step, so the palette runs dark to light through a
  tint); a phase difference of one half between channels gives complementary colours.
- Different c per channel (e.g. 1, 0.7, 0.4 style ratios) makes the channels drift apart,
  producing palettes that do not repeat within [0,1] and look less "rainbow".
- Lowering b for one channel and raising a for another tints the whole palette (warm or cool
  cast) and compresses its contrast.
- Periodicity (from the article): the palette repeats exactly over [0,1] when every c is a
  multiple of 0.5; for C1 continuity at the wrap, make every c an integer (then the palette is
  infinitely smooth across the seam). For a fractal palette applied with repetition
  (t = fract(mu * density + offset)) choose integer c, otherwise a visible seam appears at
  every period.
- The article's own caveat: all such palettes share the same rhythm (sinusoids), and they are
  "good enough" for many purposes rather than hand-tuned.

## Weaknesses (our analysis)

- Not perceptually uniform: equal steps in t do not give equal perceived change, lightness is
  not monotonic, and the sum of three cosines often passes through muddy mid-greys when the
  channels are out of phase. Fine for decorative fractal colouring, bad for data.
- Clipping: if a + b > 1 or a - b < 0 in any channel the palette flattens at the gamut edge.
  Clamp, or better, fit parameters so it never clips.
- Mean colour equals a: averaged over a period, a cosine palette is exactly the offset colour.
  In dense regions where many bands fall into one pixel, the pixel averages towards a (often
  mid grey 0.5,0.5,0.5). Choosing a darker or tinted a controls what dense areas fade to (see
  cyclic-and-mapping.md).
- Evaluate in linear light or sRGB? The article works directly on display values (sRGB). A
  cosine in OKLab (L, a, b channels, with L kept monotonic or slowly varying) gives
  better-behaved palettes: e.g. L = L0 + Lamp*cos(...), and (a,b) on a circle of fixed chroma
  for a hue cycle with even lightness, then convert to sRGB and gamut-map.

## Use in a palette library

- Good for a "procedural" family: a handful of 12-number presets, editable with 4 sliders per
  channel or by phase/frequency only, animatable (vary d over time for colour cycling, which is
  exactly a palette offset).
- Can be fitted: given a hand-made gradient, least-squares fit a, b, c, d to approximate it,
  which gives a compact, seamless approximation.
- Store as parameters plus a sampled table (e.g. 256 entries) so the renderer has a single
  lookup path for all palettes.
