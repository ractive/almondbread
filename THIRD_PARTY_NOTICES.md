# Third-party notices

Almondbread is MIT-licensed (see `LICENSE` and `AI_NOTICE`). It includes or is derived from
the following third-party material.

## Inter typeface (SIL Open Font License 1.1)

The on-screen overlay text uses a signed-distance-field atlas rendered from **Inter SemiBold
4.1**, Copyright (c) 2016 The Inter Project Authors (https://github.com/rsms/inter).
The atlas (`crates/almondbread-render/src/font/overlay_font.sdf`) is built by
`crates/almondbread-render/examples/gen_overlay_font.rs` and compiled into the app. The full
licence text is in `crates/almondbread-render/src/font/OFL.txt` and ships inside the app
bundle as `Contents/Resources/Inter-OFL.txt`.

## Colour palettes

`crates/almondbread-core/palettes/designed.json` holds palettes made for almondbread
(own work, CC0-1.0). `crates/almondbread-core/palettes/derived.json` holds stops subsampled
by `palettes/derive.py` from these open colour maps; each entry records its attribution,
licence and source URL, and the app shows the attribution in the palette library.

| Palettes | Authors | Licence |
|---|---|---|
| viridis, magma, inferno, plasma | Stéfan van der Walt, Nathaniel Smith, Eric Firing (matplotlib / BIDS colormap) | CC0-1.0 |
| cividis | Jamie R. Nuñez, Christopher R. Anderton, Ryan S. Renslow; data from PNNL cmaputil as shipped in matplotlib, Copyright Battelle Memorial Institute | BSD-style (matplotlib licence) |
| twilight | Bastian Bechtold, as shipped in matplotlib | matplotlib licence (PSF-style, BSD-compatible) |
| turbo | Anton Mikhailov, Copyright 2019 Google LLC | Apache-2.0 |
| batlow, hawaii, lajolla, oslo, tokyo, romaO, vikO, bamO | Fabio Crameri, Scientific colour maps (doi:10.5281/zenodo.1243862) | MIT |
| haline, dense, solar, algae | Kristen Thyng et al., cmocean | MIT |
| CET-C1, C2, C3, C6, C9 | Peter Kovesi, Good Colour Maps (arXiv:1509.03700), via holoviz colorcet | CC BY 4.0 |
| cosine-* | own parameters; cosine palette technique by Inigo Quilez | CC0-1.0 |

The "classic" default gradient is our own, in the style of Ultra Fractal's default; no Ultra
Fractal data is included. Ultra Fractal `.ugr` and Fractint `.map` files can be imported;
none are bundled.

## Places

The places catalogue (`crates/almondbread-core/src/places_data.rs`) gives coordinates of
well-known locations with their sources (Robert Munafo's Mu-Ency, Wikipedia, Wolfgang Beyer's
zoom sequence on Wikimedia Commons, papers); coordinates are facts, the descriptions are our
own words, and the thumbnails are rendered by almondbread itself
(`almondbread places --render-thumbs`).

## Rust crates

The Rust workspace depends on crates from crates.io under permissive licences: MIT,
Apache-2.0, MIT OR Apache-2.0, Zlib, BSD-2-Clause/0BSD alternatives, Unicode-3.0 and
Unlicense alternatives, plus **uniffi** (MPL-2.0, used unmodified; its source is on
crates.io). There is no GPL or AGPL dependency. `deny.toml` enforces this in CI
(`cargo deny check`); `cargo metadata` or `cargo deny list` prints the full list.

## Algorithms

The deep-zoom techniques (perturbation with rebasing, bilinear approximation, Imagina-style
linear approximation, reference selection and interior detection) are implemented
from published papers, articles and forum descriptions, cited in `kb/`. No code from Kalles
Fraktaler, Fraktaler 3, Imagina, FractalShark or other fractal programs is included.
