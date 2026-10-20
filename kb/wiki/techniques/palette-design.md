---
title: "Palette design for fractals"
type: technique
status: draft
sources: ["sources/fractalset-viewer", "sources/ultrafractal-gradients-help", "sources/fractint-map-format", "sources/kf-manual", "sources/kovesi-good-colour-maps", "sources/colorcet", "sources/crameri-scientific-colour-maps", "sources/bids-viridis-colormaps", "sources/cmocean", "sources/turbo-colormap", "sources/cividis-paper", "sources/quilez-cosine-palettes", "sources/ottosson-oklab"]
tags: [palette, coloring, oklab, cyclic, file-format, cvd]
---

# Palette design for fractals

How to build palettes that look good on an escape-time fractal, and how
almondbread's palette library (`crates/almondbread-core/src/palette_library.rs`)
applies it. The colouring pipeline around it (smooth count, transfer, lighting,
palette filtering) is in [Colouring methods](coloring-methods.md) and
[Smooth (continuous) iteration count](smooth-coloring.md); the library itself is summarised in
[Fractal palettes: research and the palette library](../synthesis/fractal-palettes.md).

## Palette = key colours + mapping

Every serious program stores a few key colours and keeps the mapping separate:

- Ultra Fractal: a 400-entry gradient that wraps; index x density, then a
  transfer function (linear, sqrt, log, ...), then an offset, then wrap or clamp
  (src: [Ultra Fractal help: gradients and colouring settings](../sources/ultrafractal-gradients-help.md)).
- Kalles Fraktaler: 1-1024 key colours expanded into a cyclic 1024 table with
  sine-eased blends; smooth count through a transfer, divided by "iteration
  division", plus offset (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)).
- fractalset.com: 5-10 RGB keys, log-scaled density `2^(d/10)`, a phase offset in
  degrees, and a sine ping-pong mapping (src: [fractalset.com viewer and gradient editor](../sources/fractalset-viewer.md)).

So a palette entry needs: stops (position, colour), whether it is cyclic, and a
recommended density, offset and transfer. almondbread stores exactly that, plus
a deep colour and attribution.

## Interpolate in OKLab with a spline

- sRGB lerp greys out between distant hues and HSV/HSL swings lightness; OKLab
  keeps chroma and even lightness steps (src: [Ottosson: A perceptual color space for image processing (OKLab)](../sources/ottosson-oklab.md)).
- Linear segments show kinks at stops in a smooth exterior; a Catmull-Rom spline
  through the stops does not (src: [Ultra Fractal help: gradients and colouring settings](../sources/ultrafractal-gradients-help.md) on
  smooth vs linear gradients). almondbread: periodic Catmull-Rom in OKLab.
- Stops given in OKLCH can lie outside sRGB; reduce chroma at constant lightness
  and hue until they fit (CSS Color 4 practice, src: [Ottosson: A perceptual color space for image processing (OKLab)](../sources/ottosson-oklab.md)),
  rather than clipping channels, which shifts the hue.

## Lightness carries the detail

- Colour discrimination collapses for small features; lightness does not, so
  neighbouring keys should alternate light and dark
  (src: [Kovesi: Good Colour Maps: How to Design Them](../sources/kovesi-good-colour-maps.md)).
- A hue sweep at constant lightness looks flat on fine filaments and fails for
  colour-blind viewers (src: [Kovesi: Good Colour Maps: How to Design Them](../sources/kovesi-good-colour-maps.md),
  [Nuñez et al.: cividis, a colour map for colour vision deficiency](../sources/cividis-paper.md)).
- Uneven band widths and one narrow bright accent (gold, white, cyan) on 2-3
  related hues read as designed (unverified, artist practice).

## Seamless cycling

The palette repeats many times across an image, so the wrap must not show:

- Cyclic palettes match colour and slope at the wrap. Kovesi's recipes for vivid
  cyclic maps: zig-zag between two light and two dark colours, or a diamond with
  one light, one dark and two mid colours far apart in hue; smooth each
  lightness reversal (src: [Kovesi: Good Colour Maps: How to Design Them](../sources/kovesi-good-colour-maps.md)).
- Sequential maps (viridis, batlow, ...) jump from light back to dark each cycle.
  Mirror them (ping-pong, as fractalset's sine does) or accept the seam as a
  contour line (src: [fractalset.com viewer and gradient editor](../sources/fractalset-viewer.md)). almondbread mirrors every
  non-cyclic library palette with a zero-slope turn at both ends, so a mirrored
  palette shows each colour twice per cycle (bands come in symmetric pairs).
- Cyclic maps from open data start wherever their authors put index 0; the
  library sets their offset to the darkest stop, so the far exterior (few
  iterations, `t` near 0) is dark. Light-first sequential maps are reversed for
  the same reason.

## Density and transfer

- Iteration counts are roughly exponentially distributed and rise steeply near the
  set, so a linear mapping packs many cycles into the boundary; log (or sqrt)
  spreads the palette more evenly; histogram mapping is the most even but depends
  on the whole frame (src: research notes `cyclic-and-mapping.md`; hpdz.net via the
  Wayback Machine, not saved).
- A density control works best on a log scale (equal steps, equal change)
  (src: [fractalset.com viewer and gradient editor](../sources/fractalset-viewer.md)).
- almondbread's shader maps `t = fract(n * density) + frac * density` (linear) or
  `t = ln(mu + 1) * density` (log), then adds the offset; the library recommends
  linear densities of 0.015-0.04 cycles per iteration.

## Dense regions and the deep colour

- Where several cycles fall in one pixel, supersampling or palette filtering
  averages the palette: the pixel takes the palette's mean colour. A mid-grey mean
  makes dense regions muddy; a dark mean makes them velvety
  (src: research notes `cyclic-and-mapping.md`; [Dense boundary regions: fewer pixel-iterations for the same image (R3)](../synthesis/dense-region-sampling.md)).
- Each library palette therefore names a **deep colour** (default: its darkest
  stop at half lightness, half chroma) for the interior, the outline and as the
  intended target for dense regions; `mean_linear()` gives the actual mean so the
  colourer can blend the filtered colour towards the deep tone.
- Cosine palettes `a + b cos(2π(ct + d))` average exactly to `a`
  (src: [Quilez: procedural colour palettes](../sources/quilez-cosine-palettes.md)).

## Colour-vision safety

- Avoid red-green-only contrast; make neighbouring bands differ in lightness
  (src: [Crameri: Scientific colour maps](../sources/crameri-scientific-colour-maps.md), [Nuñez et al.: cividis, a colour map for colour vision deficiency](../sources/cividis-paper.md)).
- cividis and batlow are designed to look nearly the same to deuteranopes; CET
  has explicit colour-blind cyclic maps (src: [colorcet: the CET perceptually uniform colour maps](../sources/colorcet.md)).
- `lightness_span()` in the library measures the OKLab lightness range over a
  cycle; palettes with a span of at least 0.4 keep their structure without hue.

## Licences

Algorithms and file formats are free; palette data is not automatically
(see [Fractal palettes: research and the palette library](../synthesis/fractal-palettes.md) for the table). Ship only CC0, MIT, BSD,
Apache 2.0 or CC BY data with the credit recorded, or our own designs. Do not
ship UF, KF (AGPL), Fractint (non-commercial) or fractalset palettes.

## File formats

- **Ultra Fractal `.ugr`**: entries `name { gradient: title="..." smooth=yes|no
  rotation=R index=N color=C ... opacity: ... }`; whitespace-separated
  `key=value` tokens (line breaks insignificant), quoted values may hold spaces,
  `;` comments; `index` on a 400-step ring, possibly unsorted or outside 0..399
  (wrap); `color` a decimal COLORREF `R + 256 G + 65536 B`
  (src: [Ultra Fractal help: gradients and colouring settings](../sources/ultrafractal-gradients-help.md)). Opacity is ignored on import.
- **Fractint `.map`**: up to 256 lines `R G B` (0..255); text after the third
  number is a comment; no header. Entry 0 is the inside colour, so almondbread
  takes it as the deep colour and entries 1.. as the cyclic gradient
  (src: [Fractint documentation: palette maps (.map)](../sources/fractint-map-format.md)).
- **almondbread JSON** (user palettes and the built-in data): `id`, `name`,
  `category`, `cyclic`, `smooth`, `density`, `offset`, `log`, `deep`,
  `attribution`, `license`, `source`, and `stops` as `"#RRGGBB"`,
  `"oklch(L C H)"` or `{"at": t, "color": ...}`.

## Pitfalls

- KF stores key colours in display BGR order despite naming them r, g, b
  (inferred from its source, unverified) (src: research notes `kalles-fraktaler.md`).
- Many Fractint maps were designed for integer bands and colour cycling and look
  noisy under smooth colouring at high density.
- JWildfire pads missing `.ugr` indices 0 and 399 with black, which breaks UF's
  wrap; do not copy that (src: research notes `ultra-fractal-gradients.md`).

# Citations

- [fractalset.com viewer and gradient editor](../sources/fractalset-viewer.md)
- [Ultra Fractal help: gradients and colouring settings](../sources/ultrafractal-gradients-help.md)
- [Fractint documentation: palette maps (.map)](../sources/fractint-map-format.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [Kovesi: Good Colour Maps: How to Design Them](../sources/kovesi-good-colour-maps.md)
- [colorcet: the CET perceptually uniform colour maps](../sources/colorcet.md)
- [Crameri: Scientific colour maps](../sources/crameri-scientific-colour-maps.md)
- [BIDS colormap: viridis, magma, inferno, plasma](../sources/bids-viridis-colormaps.md)
- [cmocean: colour maps for oceanography](../sources/cmocean.md)
- [Google Turbo colour map](../sources/turbo-colormap.md)
- [Nuñez et al.: cividis, a colour map for colour vision deficiency](../sources/cividis-paper.md)
- [Quilez: procedural colour palettes](../sources/quilez-cosine-palettes.md)
- [Ottosson: A perceptual color space for image processing (OKLab)](../sources/ottosson-oklab.md)
- [Dense boundary regions: fewer pixel-iterations for the same image (R3)](../synthesis/dense-region-sampling.md)
