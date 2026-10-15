# Fractint .map palettes and who reads them

Accessed 2026-10-14. Sources:

- Fractint documentation `fractint.doc`, section 3.2 "Palette Maps", in the Fractint source
  mirror https://github.com/LegalizeAdulthood/fractint-legacy (file `fractint.doc`).
- Fractint loader `loadmap.c` (function `ValidateLuts`), same repository.
- Sample maps shipped with Fractint, same repository: `default.map`, `altern.map`,
  `blues.map`, `chroma.map`, `firestrm.map`, `neon.map`, `royal.map`, `topo.map`,
  `volcano.map`, ...
- Fractint source licence text `fractsrc.doc`, same repository.
- Cyotek's Fractint palette serializer (MIT): https://github.com/cyotek/Cyotek.Drawing.PaletteFormat.Fractint
- Ultra Fractal file types: https://www.ultrafractal.com/help/browsers/filetypes.html
- ChaosPro compatibility and palette editor:
  https://chaospro.de/documentation/html/compatibility/compatibility.htm,
  https://chaospro.de/documentation/html/paletteeditor/palette.htm
- XaoS source `src/ui-hlp/menu.cpp` (GPL-2.0): https://github.com/xaos-project/XaoS
- KF manual (AGPL-3.0 program): https://mathr.co.uk/kf/manual.html
- gfract (uses Fractint's format with longer palettes): https://openports.pl/path/graphics/gfract
- JWildfire 0.70 release notes (reads .map): https://blog.overwhale.com/?p=2313

Licences:

- Fractint source: not open source in the modern sense. `fractsrc.doc` says the source "may be
  copied freely and may be used in other programs" provided "It may not be used in a commercial
  program which produces fractal images" and the Stone Soup Group is credited. No separate
  licence is stated for the bundled .map files; they come from named contributors (Daniel
  Egnor, Lee Crocker, Monte Davis, ...). Treat the bundled maps as non-commercial,
  attribution-required and do not ship them in almondbread. The format itself is free to
  implement.
- Cyotek serializer: MIT (repo LICENSE.txt).

## Format

From the Fractint doc: colour-map files are "ASCII text files set up as a series of RGB triplet
values (one triplet per line, encoded as the red, green, and blue [RGB] components of the
color)". Values are "in GIF format", 0 (low) to 255 (high). Default extension `.map`.

Exact rules, from `loadmap.c` and the bundled files:

- Up to 256 lines are read; line N is palette index N (index 0 first).
- Each line is parsed with the equivalent of `sscanf("%u %u %u")`: three unsigned decimal
  integers separated by whitespace (spaces, often right-aligned in fixed columns). Anything after
  the third number is ignored, which is how comments work: free text after the triplet, usually
  on the first line. Real examples:
  - `0 0 0            The default VGA color map` (default.map line 1)
  - `  0   0   0    The famous Peterson-Vigneau Pseudo-Grey Sequence` (altern.map)
  - `  0   0   0  For them rainy days ... by Daniel Egnor` (blues.map)
- There is no header, no count line and no `#` comment syntax. Fractint stops at end of file;
  missing entries (file shorter than 256 lines) are filled with a grey (DAC value 40 of 63,
  roughly 160/255).
- Values are taken modulo 256, then divided by 4 for the 6-bit VGA DAC (so 6 and 7 collapse).
  A modern reader should keep full 8 bits.
- Lines are read with a 100-character buffer; longer comments are truncated (a long line would
  spill into the next read in the original C, so keep comments short when writing).
- Entry 0 is black in the bundled maps read (default, altern, blues), and Fractint's colour
  cycling leaves entry 0 alone and rotates the rest (unverified: from memory of Fractint's
  behaviour, not checked in the doc). A palette meant for
  smooth colouring should therefore treat entry 0 with care: many maps put black there and
  start the real gradient at entry 1.

Writer recipe: 256 lines of `R G B` with values 0..255, optional short text after the third
number on line 1 (name, author, licence).

Parser recipe: for each non-empty line, read three integers; ignore the rest of the line;
accept fewer or more than 256 lines (gfract and others write longer palettes); clamp 0..255.
Cyotek's reader validates the first line by trying to parse three integers and splits on
whitespace with a limit of 4 parts (the fourth part being the comment).

## Use in other programs

- Ultra Fractal opens `.map` files as gradients (the browser lists them with *.ugr and *.ual).
- ChaosPro: "All FractInt and ChaosPro *.map files can be read". On import the palette editor
  shows about 252 control points, one per colour; a "reorganize" command removes redundant
  points. ChaosPro also reads UF `.ugr` since 4.0, and its editor interpolates in RGB, CMY, HSL
  or HSV (switching the model changes the gradient without moving points).
- JWildfire reads `.map` ("ApoMap", the Apophysis flavour) and `.ugr`.
- XaoS does not use `.map`. Its palettes are algorithmic (palette type, seed, shift; default
  and random palettes) or a 31-colour "segment" palette saved as a GIMP palette (`.gpl`: 4
  header lines, then 31 `R G B` lines). It also has colour cycling.
- Kalles Fraktaler stores palettes as `.kfp` (a KFR parameter file renamed, with a
  `Colors:` line of key colours) and does not import `.map`; importing .ugr/.gradient, .map
  and flam3 .xml is on its TODO list (KF manual, future work: "import gradients from various
  formats ... inspired by Mandel Meute + padleywood's gradient convert tool").
- gfract uses "the same format as Fractint does, except that it supports arbitrarily large
  palettes".

## Notes for a palette library

- A 256-entry map is a sampled gradient, not key colours. Importing it as 256 stops is fine
  for lookup; for editing, reduce it to key colours (ChaosPro's "reorganize").
- Fractint maps were designed for integer iteration bands and colour cycling on a 256-colour
  display; many are high-frequency (stripes, alternating greys like altern.map) and look noisy
  under smooth colouring with high density. Expect to need a lower density for them.
