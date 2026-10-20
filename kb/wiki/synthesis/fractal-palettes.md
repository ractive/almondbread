---
title: "Fractal palettes: research and the palette library"
type: synthesis
date: 2026-10-15
sources: ["[[sources/fractalset-viewer]]", "[[sources/ultrafractal-gradients-help]]", "[[sources/fractint-map-format]]", "[[sources/kf-manual]]", "[[sources/kovesi-good-colour-maps]]", "[[sources/colorcet]]", "[[sources/crameri-scientific-colour-maps]]", "[[sources/bids-viridis-colormaps]]", "[[sources/cmocean]]", "[[sources/turbo-colormap]]", "[[sources/cividis-paper]]", "[[sources/quilez-cosine-palettes]]", "[[sources/ottosson-oklab]]"]
tags: [palette, coloring, licence, library]
---

# Fractal palettes: research and the palette library

The user asked for many selectable, well-designed palettes. This page records
the research (notes in `raw/research/fractal-palettes/`, 2026-10-14) and what
was built from it. The design rules are in [Palette design for fractals](../techniques/palette-design.md).

## Findings

1. Every mature program stores a few key colours and keeps density, transfer and
   offset separate (UF, KF, fractalset)
   (src: [Ultra Fractal help: gradients and colouring settings](../sources/ultrafractal-gradients-help.md), [Kalles Fraktaler 2 + manual](../sources/kf-manual.md),
   [fractalset.com viewer and gradient editor](../sources/fractalset-viewer.md)).
2. Interpolate in OKLab with a spline; reduce chroma to fit sRGB
   (src: [Ottosson: A perceptual color space for image processing (OKLab)](../sources/ottosson-oklab.md)).
3. Lightness carries the detail; alternate light and dark keys; isoluminant maps
   lose fractal structure (src: [Kovesi: Good Colour Maps: How to Design Them](../sources/kovesi-good-colour-maps.md)).
4. Every palette must cycle without a seam: cyclic by design, or mirrored
   (src: [Kovesi: Good Colour Maps: How to Design Them](../sources/kovesi-good-colour-maps.md), [fractalset.com viewer and gradient editor](../sources/fractalset-viewer.md)).
5. Dense regions average to the palette mean; give each palette a deliberate
   dark deep tone ([Dense boundary regions: fewer pixel-iterations for the same image (R3)](dense-region-sampling.md)).
6. About 25 curated palettes is fractalset's size; our library is larger but
   grouped into categories (src: [fractalset.com viewer and gradient editor](../sources/fractalset-viewer.md)).

## Licences

| Family | Licence | Shipped |
| --- | --- | --- |
| almondbread designs (40, incl. 3 cosine palettes) | CC0-1.0 (own work) | yes |
| viridis, magma, inferno, plasma | CC0-1.0 (src: [BIDS colormap: viridis, magma, inferno, plasma](../sources/bids-viridis-colormaps.md)) | yes |
| cividis | BSD-style, Battelle/PNNL; article CC0 (src: [Nuñez et al.: cividis, a colour map for colour vision deficiency](../sources/cividis-paper.md)) | yes, credited |
| Turbo | Apache-2.0 (src: [Google Turbo colour map](../sources/turbo-colormap.md)) | yes, credited |
| Crameri batlow, hawaii, lajolla, oslo, tokyo, romaO, vikO, bamO | MIT (src: [Crameri: Scientific colour maps](../sources/crameri-scientific-colour-maps.md)) | yes, credited |
| cmocean haline, dense, solar, algae | MIT (src: [cmocean: colour maps for oceanography](../sources/cmocean.md)) | yes, credited |
| CET C1, C2, C3, C6, C9 | CC-BY-4.0 (src: [colorcet: the CET perceptually uniform colour maps](../sources/colorcet.md)) | yes, credited |
| twilight | matplotlib licence only (author's repository has none) | yes, credited |
| ColorBrewer | Apache-2.0 | not used (few classes, made for maps) |
| UF, KF (AGPL), Fractint (non-commercial), fractalset gradients | not free | no; formats only |

Each palette carries its `attribution`, `license` and `source` URL; an about
screen should list the credited ones.

## The library as built (2026-10-15)

67 palettes in 9 categories: classic fractal 6, natural 8, fire & ice 6,
pastel 5, neon & vapor 7, monochrome & ink 7, scientific 15, cyclic 9, high
contrast 4. Data in `crates/almondbread-core/palettes/designed.json` (hand
made) and `derived.json` (written by `derive.py` from the open data above,
subsampled to 12-32 stops within 0.006 OKLab of the original). Dropped after
looking at the contact sheets (`almondbread palettes`, seahorse 1.3e-4 wide
and the user's -1.7687 view at 1.875e-6 per point): Canyon, Hi-Vis, Cosine
Electric, and the near duplicates matter, thermal, imola, davos, tempo, deep,
corkO, brocO, CET C7, C11; cmocean phase for being isoluminant (OKLab
lightness span 0.02).

Observations from the sheets (our own, 2026-10-15, M-series Mac, release
build, default look): dense filament regions still turn grey-brown with every
palette, which is the colourer's filtered-mean behaviour, not the palette's;
the deep colour is there for that fix. Pastels need the dark deep tone most.
Lowest lightness spans (least CVD-robust): CET C6 0.26, CET C1 0.29, Macaron
0.35, Sorbet 0.39.

## Integration

- `palette_library::{builtin, find, all, in_category, register_user}`;
  `LibraryPalette::{table, swatch, color_at, apply, mean_linear, deep_linear,
  lightness_span, to_json, from_json}`.
- `Palette::named(id)` falls back to the library and to registered user
  palettes, so setting `RenderSettings::palette_id` to a library id renders it
  with no other change.
- `palette_import::{parse_ugr, write_ugr, parse_map, write_map}`.

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
