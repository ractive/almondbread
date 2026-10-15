# Fractal palette research notes (2026-10-14)

Raw notes backing a palette library of 40-80 palettes for almondbread. Each file lists its
sources (accessed 2026-10-14) and the licence of any palette data it discusses. No proprietary
palette data is copied.

## Files

- `fractalset.md`: the fractalset.com viewer's palettes and controls (24 built-ins, log-scaled
  density, rotation, ping-pong sine mapping, shader and animation modes). Proprietary, no
  licence.
- `ultra-fractal-gradients.md`: UF gradient practice (density, transfer functions, offset,
  repeat, 400-entry wrapping gradient, smooth toggle, opacity) and the exact `.ugr` syntax
  (`color` = R + 256 G + 65536 B).
- `fractint-map.md`: Fractint `.map` format (up to 256 lines of `R G B`, text after the third
  number is a comment) and how UF, ChaosPro, JWildfire, XaoS (.gpl) and KF (.kfp) relate.
- `kalles-fraktaler.md`: KF 2+ palette model (1-1024 key colours, cyclic 1024 table, sine-eased
  blend, IterDiv, offset, transfer functions, waves, slopes, default palette). AGPL-3.0.
- `cosine-palettes.md`: `a + b cos(2 pi (c t + d))`, parameter behaviour, periodicity, mean
  colour, weaknesses. Article has no licence; formula free, examples not copied.
- `perceptual-maps.md`: viridis family (CC0), cividis (CC0 article, Battelle BSD data), Crameri
  (MIT), cmocean (MIT), ColorBrewer (Apache 2.0), CET/colorcet (CC BY 4.0), Turbo (Apache 2.0),
  twilight; raw data URLs.
- `oklab-interpolation.md`: OKLab/OKLCh for building gradients, why sRGB lerp greys out, hue
  interpolation and gamut-mapping pitfalls.
- `cyclic-and-mapping.md`: cyclic map design (Kovesi), smooth-iteration mapping (linear, root,
  log, histogram), dense regions and palette mean, contrast and rhythm, CVD safety.

## Main design rules (summary)

1. Store palettes as key colours (position + colour), cyclic by default; sample them into a
   table (256-1024 entries) for the renderer. Keep mapping (density, transfer, offset, repeat
   or mirror) separate from the palette.
2. Interpolate keys in OKLab (OKLCh only for deliberate hue sweeps, with per-segment hue
   direction and missing hue for greys); gamut-map by chroma reduction; average pixels in
   linear RGB.
3. Use smooth (spline) interpolation through keys, and smooth lightness reversals; linear
   segments show kinks in smooth colouring.
4. Lightness carries fractal detail: alternate light and dark between neighbouring keys; hue
   changes at equal lightness vanish at small scale and for CVD viewers.
5. Every palette must wrap without a seam (match colour and slope at the ends) or be used with
   mirror mapping; sequential science maps need mirror mapping or a cyclic variant.
6. Know each palette's mean colour: dense regions converge to it. Prefer dark or deliberate
   means; prefilter (mip) the palette when bands go sub-pixel.
7. Offer transfer functions (linear, sqrt, log, and a histogram/stretch option) and a
   log-scaled density control; deep zooms need log or lower density.
8. Ship palettes only from permissive sources (CC0, MIT, Apache 2.0, BSD, CC BY with
   credit) or our own designs; never from UF, KF, Fractint or fractalset collections.
9. Tag palettes as CVD-safe after simulation and greyscale checks; include a few scientific
   maps (batlow, cividis, CET colour-blind maps) for that purpose.
10. Import UF `.ugr` (BGR-packed integers, indices 0..399, wrap, unsorted allowed) and Fractint
    `.map` (R G B lines, trailing comments) so users can bring their own gradients.
