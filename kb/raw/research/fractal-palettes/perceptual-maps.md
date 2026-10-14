# Perceptually uniform colour maps: licences and raw data

Accessed 2026-10-14. All licences below were read from the licence file or record named.
"Raw data" URLs were fetched and checked to return text data on that date.

## matplotlib family: viridis, magma, inferno, plasma

- Authors: Nathaniel J. Smith, Stefan van der Walt (viridis also Eric Firing). Designed in
  CAM02-UCS with monotonic lightness. Talk/site: https://bids.github.io/colormap/
- Licence: CC0 1.0 (public domain dedication). `LICENSE.txt` of https://github.com/BIDS/colormap
  says "To the extent possible under law, the persons who associated CC0 with mpl-colormaps
  have waived all copyright and related or neighboring rights". The header of `colormaps.py`
  repeats "released under the CC0 license / public domain dedication" and asks for credit.
- Raw data (256 entries each, sRGB floats 0..1, as Python lists `_magma_data`,
  `_inferno_data`, `_plasma_data`, `_viridis_data`):
  https://raw.githubusercontent.com/BIDS/colormap/master/colormaps.py
  The same lists are in matplotlib's
  https://raw.githubusercontent.com/matplotlib/matplotlib/main/lib/matplotlib/_cm_listed.py
  (matplotlib licence, a PSF-style BSD-compatible licence; prefer the CC0 source).

## cividis

- Nuñez, Anderton, Renslow, "Optimizing colormaps with consideration for color vision
  deficiency to enable accurate interpretation of scientific data", PLOS ONE 13(7): e0199239,
  2018. https://doi.org/10.1371/journal.pone.0199239 . Built from viridis with their
  `cmaputil` tool: simulate full deuteranopia (Machado model), equalise steps in CIECAM02-UCS,
  make lightness linear; near-optimal for protan and tritan too. Looks the same to normal and
  red-green colour-blind viewers (blue to yellow only).
- Licence: the article is CC0 (PLOS page). The data/code repo https://github.com/pnnl/cmaputil
  has a Battelle Memorial Institute BSD-style licence (redistribution allowed with copyright
  notice and disclaimer retained; Battelle name not to be used for endorsement); a
  `cividis_disclaimer_pnnl.docx` sits next to it. Matplotlib also ships cividis under its own
  licence. Safest: take values from the CC0 article's S2 file or credit Battelle as the BSD
  licence asks.
- Raw data (256 lines, `R G B` floats with 4 decimals):
  https://raw.githubusercontent.com/pnnl/cmaputil/master/colormaps/cividis.txt
  (also `cividisHexValues.txt`, `cividis.lut` in the same folder).

## Crameri Scientific colour maps (batlow, roma, romaO, vikO, bamO, ...)

- Fabio Crameri, "Scientific colour maps", version 8.0.1 (2023-10-05), Zenodo
  https://doi.org/10.5281/zenodo.1243862 (record 8409685, the latest version as of today).
  Site: https://www.fabiocrameri.ch/colourmaps/ (states no licence itself).
- Licence: MIT (Zenodo record metadata `license: mit-license`). The Python package cmcrameri
  (https://github.com/callumrollo/cmcrameri) is also MIT, "Colormaps ... Copyright (c) 2020
  Fabio Crameri".
- Families (from the file list): sequential (batlow, batlowK, batlowW, acton, bamako,
  bilbao, buda, davos, devon, glasgow, grayC, hawaii, imola, lajolla, lapaz, lipari, navia,
  nuuk, oslo, tokyo, turku), diverging (bam, berlin, broc, cork, lisbon, managua, roma,
  tofino, vanimo, vik), multi-sequential (bukavu, fes, oleron), cyclic (bamO, brocO, corkO,
  romaO, vikO), and categorical variants with an `S` suffix (batlowS, ...). batlow is
  colour-blind safe, perceptually uniform and readable in greyscale.
- Raw data (256 lines of `R G B` floats 0..1 per map):
  https://raw.githubusercontent.com/callumrollo/cmcrameri/main/cmcrameri/cmaps/<name>.txt
  e.g. .../batlow.txt, .../romaO.txt, .../vikO.txt, .../bamO.txt. The canonical source is the
  Zenodo zip `ScientificColourMaps8.zip`.
- berlin, managua and vanimo are also in matplotlib 3.10 `_cm_listed.py`.

## cmocean

- Kristen Thyng et al., oceanography maps (thermal, haline, solar, ice, gray, oxy, deep,
  dense, algae, matter, turbid, speed, amp, tempo, rain, phase, topo, balance, delta, curl,
  diff, tarn). `phase` is cyclic.
- Licence: MIT, "Copyright (c) 2015 Kristen M. Thyng" (`LICENSE.txt` of
  https://github.com/matplotlib/cmocean).
- Raw data (256 lines, `R G B` floats, scientific notation):
  https://raw.githubusercontent.com/matplotlib/cmocean/main/cmocean/rgb/<name>-rgb.txt
  e.g. .../thermal-rgb.txt, .../phase-rgb.txt (some maps only as `<name>.py` or `.jscm`).

## ColorBrewer

- Cynthia Brewer, Mark Harrower, Penn State. Discrete schemes (3 to 9-12 classes):
  sequential, diverging, qualitative.
- Licence: Apache License 2.0. matplotlib's `LICENSE/LICENSE_COLORBREWER` titles it
  "Apache-Style Software License for ColorBrewer software and ColorBrewer Color Schemes" and
  licenses it under Apache 2.0; https://github.com/axismaps/colorbrewer `LICENCE.txt` is the
  Apache 2.0 text. The attribution line used in its JS: "This product includes color
  specifications and designs developed by Cynthia Brewer (http://colorbrewer.org/)."
- Raw data: https://raw.githubusercontent.com/axismaps/colorbrewer/master/colorbrewer_schemes.js
  (JS object, `rgb(r,g,b)` strings by scheme and class count). `cb.csv` in the same repo has
  CMYK only.
- Few stops: useful as key colours for a smooth fractal gradient (e.g. the 9-class sequential
  and 11-class diverging schemes), not as a ready table.

## Kovesi / colorcet (CET maps)

- Peter Kovesi, "Good Colour Maps: How to Design Them", arXiv:1509.03700 (2015);
  https://colorcet.com/. Linear, diverging, rainbow, cyclic, isoluminant and colour-blind
  (protanopic-deuteranopic, tritanopic) maps.
- Licence: Creative Commons Attribution. colorcet.com download page: "These colour maps are
  released under the Creative Commons BY License ... free to use these colour maps in anyway
  you wish as long as you give appropriate credit". The Python package
  https://github.com/holoviz/colorcet `LICENSE.txt` is "Creative Commons Attribution 4.0
  International Public License (CC-BY)". Attribution required (cite the site and the paper).
- Raw data (256 lines, `r,g,b` floats 0..1, no header):
  https://raw.githubusercontent.com/holoviz/colorcet/main/assets/CET/<name>.csv
  e.g. .../cyclic_mygbm_30-95_c78_n256.csv, .../linear_kryw_0-100_c71_n256.csv.
  Official zips: CETperceptual_csv_0_1.zip and CETperceptual_csv_0_255.zip from
  https://colorcet.com/download/index.html. Names encode type, hue letters, lightness range,
  chroma and size; `_s25` suffix = cyclic map rotated by 25 %.

## Google Turbo

- Anton Mikhailov (Google), 2019: an improved rainbow (jet replacement), smooth and roughly
  lightness-balanced but not monotonic in lightness, so not for data needing order.
- Licence: Apache 2.0: "Copyright 2019 Google LLC. SPDX-License-Identifier: Apache-2.0" in the
  gist header.
- Raw data (256 sRGB float triples and 256 byte triples as C arrays):
  https://gist.githubusercontent.com/mikhailov-work/6a308c20e494d9e0ccc29036b28faa7a/raw/turbo_colormap.c
  Also in matplotlib `_cm_listed.py` as `_turbo_data`.

## twilight (cyclic, matplotlib)

- Bastian Bechtold, https://github.com/bastibe/twilight . The repo has no licence file; the
  data ships in matplotlib (`_twilight_data` in `_cm_listed.py`) under the matplotlib licence.
  Use via matplotlib terms or skip.

## Notes for a fractal palette library

- These maps are sequential (one end dark, other light). Repeated with sawtooth mapping they
  jump from light back to dark at every period, which reads as hard contour lines; use a
  mirror (ping-pong) mapping, the cyclic variants (romaO, vikO, bamO, brocO, corkO, CET
  cyclic_*, cmocean phase, twilight) or accept the seam as a design feature.
- Attribution: CC-BY (CET) and Apache/MIT/BSD (ColorBrewer, Crameri, cmocean, Turbo, cividis)
  need credit and a licence notice in the app's acknowledgements; CC0 (viridis family) does not
  but credit is requested.
