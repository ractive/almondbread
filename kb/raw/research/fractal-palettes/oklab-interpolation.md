# Gradient interpolation in OKLab / OKLCh

Accessed 2026-10-14. Sources:

- Björn Ottosson, "A perceptual color space for image processing" (2020):
  https://bottosson.github.io/posts/oklab/
- W3C CSS Color Module Level 4 (Candidate Recommendation Draft), sections 13 "Color
  Interpolation" (13.2 interpolation space, 13.3 missing components, 13.5 hue interpolation)
  and 14 "Gamut Mapping", plus 4.4 "missing"/"powerless" components:
  https://www.w3.org/TR/css-color-4/
- Kovesi, "Good Colour Maps: How to Design Them", arXiv:1509.03700 (for lightness-gradient
  advice): https://arxiv.org/abs/1509.03700

Licences: Ottosson's Oklab code is "available in public domain, feel free to use it any way you
please", and "also available under an MIT licensee [sic]" (post text). The matrices are
published numbers and free to use. CSS Color 4 is under the W3C Software and Document License
(permissive, attribution). Kovesi's arXiv paper: arXiv non-exclusive distribution licence
(read, cite; do not redistribute the PDF).

## Why plain sRGB lerp greys out

- sRGB values are gamma-encoded. Interpolating them linearly between two saturated colours
  of different hue cuts across the RGB cube near its grey diagonal: e.g. red to green passes
  through a dark olive/brown, blue to yellow through grey. Chroma dips in the middle and
  lightness is uneven ("overly dark or greyish mixes", CSS Color 4 §13.2 on sRGB).
- Linear-light RGB (srgb-linear) fixes the physical mixing (correct for blending light,
  antialiasing, averaging samples) but makes midpoints too light perceptually; still dips in
  chroma between complementary hues.
- HSV/HSL hue interpolation keeps saturation but lightness swings wildly: yellow, cyan and
  magenta are much lighter than blue and red (Ottosson's comparison), creating false bands.
- CIELAB is closer but blue-to-white blends shift towards purple (Ottosson: CIELAB, CIELUV and
  HSV "show the same shift"); CAM16-UCS desaturates quickly in blends.

## OKLab in brief

Linear sRGB -> LMS-like cone response (3x3 matrix), cube root each, -> L, a, b (second 3x3
matrix). Matrices (linear sRGB to LMS, then LMS' to Lab) from the post:

```
l = 0.4122214708 r + 0.5363325363 g + 0.0514459929 b
m = 0.2119034982 r + 0.6806995451 g + 0.1073969566 b
s = 0.0883024619 r + 0.2817188376 g + 0.6299787005 b
l' = cbrt(l); m' = cbrt(m); s' = cbrt(s)
L = 0.2104542553 l' + 0.7936177850 m' - 0.0040720468 s'
a = 1.9779984951 l' - 2.4285922050 m' + 0.4505937099 s'
b = 0.0259040371 l' + 0.7827717662 m' - 0.8086757660 s'
```

Inverse: invert the second matrix, cube, invert the first. OKLCh: C = sqrt(a^2 + b^2),
h = atan2(b, a). Hue is fairly uniform (no blue->purple shift), L predicts lightness well.
Cost: two 3x3 matrices and three cube roots; precompute a palette table so the shader only
does a lookup.

## Choosing the interpolation space (CSS Color 4 §13.2 paraphrase)

- Physically mixing light (averaging samples, AA): linear RGB / XYZ.
- Evenly spaced perceptual steps (a gradient): OKLab (default for CSS host syntaxes that do not
  declare one).
- Maximum chroma through the transition (avoid greying): OKLCh, at the cost of hue sweeping
  through intermediate hues.

For fractal palettes: build the palette table by interpolating key colours in OKLab (or
OKLCh for vivid hue sweeps), but average/antialias pixels in linear RGB.

## Hue interpolation pitfalls (polar spaces like OKLCh)

- Hue is an angle; interpolating 350 deg -> 10 deg naively goes the long way through 180.
  CSS defines four strategies: `shorter` (default; fix up so the difference is within
  [-180, 180]), `longer`, `increasing`, `decreasing`. A palette editor needs a per-segment
  choice; "increasing" for intentional rainbow sweeps, "shorter" otherwise.
- Achromatic endpoints (white, black, greys) have no meaningful hue ("powerless" in CSS when
  chroma is below an epsilon). Interpolating from white (h arbitrary, often 0 = pinkish) to
  blue in OKLCh sweeps through unintended hues. Rule: treat the hue of an achromatic stop as
  missing and take the other stop's hue (CSS §13.3), or interpolate that segment in OKLab.
- Exactly opposite hues (180 deg apart) are ambiguous under `shorter`; pick a direction
  explicitly.
- OKLCh interpolation at constant-ish chroma leaves the sRGB gamut easily (especially between
  blue and yellow, and at high L with high C). Gamut-map each table entry: CSS Color 4 §14 uses
  chroma reduction in OKLCh with a local MINDE check (keep L and h, reduce C until in gamut
  within a small deltaE). Plain per-channel clipping shifts hue and creates flat spots.
- Lightness reversals inside a palette create false edges (Kovesi): if L goes up and down
  between stops, smooth the reversal (spline the L channel) rather than leaving a kink.

## Interpolation shape

- Linear segments in OKLab give constant speed within a segment but a visible kink at each key
  in smooth fractal colouring (the derivative of colour vs iteration jumps). Use a smooth
  spline through the keys (Catmull-Rom or monotone cubic per channel, cyclic for wrapping
  palettes), or ease each segment (KF uses a sine ease, UF has a "smooth" toggle).
- Equalise speed: resample the curve so equal steps in t give equal deltaE (OKLab distance),
  the "perceptual contrast equalisation" Kovesi and the CET maps use. Optional per palette:
  a fractal artist may want uneven speed (dwell on a colour).
- Table size: 256 entries are enough for display, 1024 avoids visible steps when density is
  low and many pixels fall within one table step; sample the table with linear interpolation
  (in the renderer) between entries.
