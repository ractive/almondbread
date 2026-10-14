# Cyclic palettes, density mapping, dense regions, contrast, colour-blind safety

Accessed 2026-10-14. Sources:

- Kovesi, "Good Colour Maps: How to Design Them", arXiv:1509.03700 (2015), section "Cyclic
  Colour Maps" and the test images: https://arxiv.org/abs/1509.03700 ; https://colorcet.com/
- hpdz.net (High-Precision Deep Zoom), "Technical Info - Colorizing", via the Wayback Machine:
  https://web.archive.org/web/2023/http://www.hpdz.net/TechInfo/Colorizing.htm (live page now
  404s).
- Fractal Forums (archive), "Determining color", reply #15 by Duncan C, 2011-01-21:
  https://www.fractalforums.com/programming/determining-color/15
- Ultra Fractal colouring settings: https://www.ultrafractal.com/help/coloring/coloringsettings.html
- KF manual, Colors dialog: https://mathr.co.uk/kf/manual.html
- Inigo Quilez, smooth iteration count: https://iquilezles.org/articles/msetsmooth
- Crameri, Shephard, Heron, "The misuse of colour in science communication", Nature
  Communications 11, 5444 (2020), https://doi.org/10.1038/s41467-020-19160-7 (CC BY 4.0).
- Nuñez et al. 2018 (cividis), PLOS ONE, https://doi.org/10.1371/journal.pone.0199239 (CC0).

Licences of text: Kovesi arXiv paper (arXiv non-exclusive licence), hpdz.net and Fractal Forums
posts (copyright of their authors, no licence stated; paraphrased), Nature Communications
article CC BY 4.0, PLOS article CC0. No palette data is copied here.

## Cyclic palettes

- Fractal palettes are normally applied repeatedly (index wraps), so the palette must be
  cyclic: colour and, ideally, its first derivative match at the wrap. UF wraps its gradient
  by design ("the gradient wraps around at the endpoints"); KF's key-colour table is cyclic.
- Kovesi on the HSV hue circle: uneven perceptual contrast (low lightness contrast cyan to
  yellow and red to magenta), the lighter secondaries create false features, and it splits the
  circle into three (or six) segments instead of the four we think in.
- A constant-lightness circle in CIELAB is gamut-limited: chroma at most about 40 near L 65-75,
  dull colours, and no lightness contrast, so fine detail is hard to see. Not a good choice for
  fractals, whose detail lives in lightness.
- Kovesi's recipes for vivid cyclic maps with 4 recognisable regions: (1) zig-zag between two
  light and two dark colours of equal lightness (e.g. blue, darkened yellow, dark red, pink,
  back to blue), (2) a diamond: one light and one dark colour at the top and bottom plus two
  mid-lightness colours far apart in hue (magenta, yellow, green, blue works). Smooth every
  lightness reversal or it shows as a false edge. Equalise perceptual contrast along the path.
- Cheap alternative: mirror (ping-pong) a non-cyclic map (fractalset does this with a sine);
  every map becomes seamless, at the cost of symmetric bands and half the colour variety per
  period. Another: diverging-style cyclic (neutral -> colour A -> neutral -> colour B ->
  neutral).
- Ready-made cyclic maps with permissive licences: Crameri romaO, vikO, bamO, brocO, corkO
  (MIT); CET cyclic_* (CC BY 4.0), including colour-blind-safe cyclic variants; cmocean phase
  (MIT).

## Mapping smooth iteration count to palette position

Smooth (continuous) iteration count for z^2 + c with large bailout R:
`mu = n + 1 - log2(log|z_n|)` (variants add constants so that mu is continuous across the
bailout; a larger bailout, e.g. 2^8 to 2^16, makes it more accurate).

Then `t = transfer(mu)`, `pos = fract(t * density + offset)` (UF: density multiplies, then
transfer, then offset; KF: transfer, then divide by IterDiv, then offset). Transfer choices:

- Linear: constant band width in iterations. Good for shallow views and when density is tuned
  per location. Deep zooms have iteration counts in the 10^4..10^7 range and the count rises
  steeply towards the set, so a linear map packs many cycles into the boundary.
- sqrt / cube root / fourth root: band width grows with iteration count; tames dense
  boundaries moderately.
- log, e.g. `log(mu + 1)` or `log(mu / mu_min)`: hpdz shows the count histogram of a typical
  image is roughly exponential (most pixels have low counts even with maxiter 10^4), so log
  spreads the palette much more evenly than linear or root maps. The forum advice (Duncan C,
  2011): the rate of change of iteration over distance is "MUCH higher" for high-iteration
  pixels, so a linear colour change makes the high-iteration parts "a riot of colors"; use log
  of the iteration number normalised between the image's min and max, or better a histogram.
- loglog, atan: even stronger compression (KF, from Mandel Machine).
- Histogram / rank-order mapping: `u = CDF(count)` over the image's escaped pixels; uses every
  colour equally and is nearly independent of maxiter, which hpdz recommends for animations.
  Downside: depends on the whole frame (flicker when panning unless smoothed over time), and
  "detail gets lost near the edges of the set since all counts at the high end ... get mapped
  to nearly the same color".
- Log mapping is not free either: a log map keeps detail near minibrots but the shallow
  features (e.g. whiskers) lose contrast far down (forum remark summarised in search results;
  not re-verified on the archived thread). Expose the transfer as a user choice.

hpdz's general form for repeating and rolling the palette: `u = frac(u * P + Q)` with P the
number of repetitions and Q the phase (animate Q for colour rolling).

## Dense regions: what to do when bands are smaller than a pixel

- When many palette cycles fall within one pixel, supersampling averages them and the pixel
  converges to the palette's mean colour (for a cosine palette exactly its offset `a`); without
  supersampling the result is aliasing noise (moire, glitter). Point sampling a high-frequency
  palette is the main source of "noisy" deep-zoom images.
- Design consequences for a palette library:
  - Know each palette's mean colour (average of its table in linear RGB) and make it look
    intentional: a dark mean makes dense areas fall to a deep tone (velvety), a mid-grey mean
    makes them muddy. Many fractal artists favour palettes with dark troughs for this reason.
  - Lower density (or log transfer) where counts rise quickly; KF's advice: Divide iteration
    "for dense images this value can be greater than 1".
  - Prefilter the palette: when the derivative of position per pixel is large, sample a blurred
    (mip-mapped) palette instead of the sharp one, which is the palette-space equivalent of
    texture mip-mapping. (almondbread's shader already has a palette mip chain and filter:
    see kb/raw/research/dense-region-sampling/experiments.md.)
  - Distance-estimate or slope shading carries structure in dense areas when the palette
    cannot.

## Contrast and rhythm (artist practice, summarised)

- Alternate light and dark between neighbouring keys so that bands separate in lightness, not
  only in hue; hue-only changes at equal lightness read poorly at small scale (Kovesi's spatial
  frequency point: colour discrimination collapses for small features, lightness carries fine
  detail).
- Vary band widths: equal spacing everywhere gives a mechanical look; UF/KF users place keys
  unevenly, KF users use prime periods in palette/infinite waves for non-repeating rhythm.
- One accent: a palette mostly in 2-3 related hues with a narrow bright accent band (gold,
  white, cyan) reads as "designed" (our judgement, unverified against a source).
- Keep the interior colour (usually black) distinct from the palette's darkest key, or make the
  set boundary glow by ending near the interior colour on purpose.

## Colour-vision deficiency (CVD) safety

- About 1 in 12 men and 1 in 200 women have some CVD, mostly red-green (deutan/protan)
  (common figure, not from the sources above; check before quoting).
- Crameri et al. 2020: avoid rainbow-like and red-green maps; use perceptually uniform maps
  that are readable by people with CVD and in greyscale (batlow is their flagship).
- cividis was optimised so normal and deuteranope viewers see nearly the same map (blue to
  yellow axis, linear lightness).
- CET offers explicit protanopic-deuteranopic and tritanopic linear, diverging and cyclic maps.
- Practical rule for fractal palettes: if adjacent bands differ in lightness by a clear
  margin, the image works for CVD viewers even if hues get confused. Test palettes with a CVD
  simulation (Machado 2009 model, as cividis did) and in greyscale; tag the library's
  CVD-safe palettes.
