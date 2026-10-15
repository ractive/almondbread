# Kalles Fraktaler 2+: palette and colour options

Accessed 2026-10-14. Sources:

- KF manual (kf-2.15.x), "Colors dialog", "OpenGL Shader Language API" and the change log:
  https://mathr.co.uk/kf/manual.html
- KF source mirror (AGPL-3.0): https://github.com/LegalizeAdulthood/kalles-fraktaler,
  files `fraktal_sft/fraktal_sft.cpp` (constructor defaults, `GenerateColors`, `ApplyColors`,
  `SetColor`), `fraktal_sft/Parameter.cpp` (KFR read/write), `fraktal_sft/fraktal_sft.h`
  (`COLOR14`), and sample files `benchmarks/*.kfr`, `palettes/**/*.kfp`.

Licence: Kalles Fraktaler 2+ is "Free Software under GNU Affero General Public License,
version 3 or greater" (manual, licence section). Facts and behaviour below are described in our
own words; no code is copied. The palettes shipped in the repo (`palettes/...`) are part of
that AGPL distribution (no separate licence seen); do not bundle them.

## Palette model

- A palette is a list of 1 to 1024 "key colours" (Number of key colors). KF expands them into a
  fixed 1024-entry table: key k sits at table position k*1024/N, and between keys the blend
  factor follows a sine ease (`0.5 + 0.5*sin((f - 0.5)*pi)` for f in 0..1), so colours dwell
  near each key and move fast between keys. The table is cyclic: the last key blends into the
  first.
- Colour index = smooth iteration count, then transfer function, then divided by "Divide
  iteration" (IterDiv), then plus "Color offset". One table entry per unit, so with IterDiv 1
  a key-to-key span is 1024/N iterations and the full palette repeats every 1024 iterations.
  The integer part picks the entry (mod 1024), the fraction blends to the next entry when
  "Smooth color transition" is on. "Inverse smooth color transition" flips the fraction,
  which makes band edges visible.
- Transfer functions (ColorMethod): Standard (linear), Square root, Cubic root, Logarithm
  (`log(max(1, n))`), Stretched (linear between the image's min and max iteration, scaled to
  the 1024 table), Distance (Linear / Logarithm / Square Root), DE + Standard, LogLog
  (`log(1 + log(1 + n))`), ATan, Fourth root. LogLog, ATan and Fourth root were ported from
  Mandel Machine.
- Divide iteration: "for dense images this value can be greater than 1. For DE, values less
  than 1 can be useful." Note: KF's density control is a divisor, the inverse of UF's
  multiplier.
- Interior colour: settable since 2.13.5, default black.
- Stored in KFR/KFP text as `Colors: r,g,b,r,g,b,...,` (one comma-separated line, trailing
  comma), plus `IterDiv:`, `ColorMethod:`, `ColorOffset:`, `Smooth:`, `InteriorColor:`,
  `MultiColor:`, `BlendMC:`, `MultiColors:`, `Slopes:`, `SlopePower:`, `SlopeRatio:`,
  `SlopeAngle:`. A `.kfp` palette "is just a .kfr renamed to .kfp".
- Channel order caveat: the in-memory struct fields are called r, g, b and the file writes them
  in that order, but the colouring code writes field r to byte 0 of a Windows BGR bitmap, and
  the EXR export "convert[s] BGR to RGB" by swapping them. So the displayed red is the third
  number of each triple (inferred from `SetColor` and the EXR writer; check by rendering a
  one-key palette before trusting it).

## Default palette

The built-in default parameters (constructor of `CFraktalSFT`) set 8 key colours, in file
order: white; a dark purple; a dark blue; a mid azure blue; a dark green; yellow; orange;
red (named using the BGR reading above; with an RGB reading the cool and warm colours swap).
Other defaults: IterDiv 0.01, ColorMethod 7 = Distance (Logarithm), smooth transitions on,
slopes on (power 50, ratio 20, angle 45), interior black. So KF's out-of-the-box look is a
distance-estimate colouring with slope shading, not a plain iteration palette. The change log
lists "nicer default palette" in the kf-2.13.7 section.

"Random" fills keys from a seed (C `rand()` after `srand(seed)`, values 0..255 per channel;
seed 1 keeps key 0 black). Random palettes are reproducible only with the same C library, so
store colours, not seeds.

## Other colour controls (Colors dialog)

- More/Less contrast: push RGB values towards the extremes or the middle.
- Expand double / Expand all / Double: re-sample the key list to more keys without changing
  the look (or repeat it), for finer editing.
- Merge colors: merge a picked colour into every n-th key.
- Show index: hover the image to highlight the key colour used there.
- Palette waves: fill the keys from sine waves on R, G, B and black-and-white; each box is the
  number of periods over the key list (the "P" button makes it prime, "since different prime
  numbers probably give more variation"), plus an offset; "Seed" randomises the fields.
- Infinite waves (Pauldelbrot's multiwave colouring): up to 32 waves applied to Hue,
  Saturation or Brightness with a period in iterations (not a count of periods); coprime
  periods give a non-repeating colour sequence. A negative value means a constant percentage.
  Waves of the same type are averaged; the HSV result can be blended 50/50 with the palette
  colour ("BlendMC").
- Slopes: shading from the gradient of the smooth iteration (or analytic DE). First value =
  magnification ("start value of 100 is suitable for the unzoomed view. Deep views requires a
  couple of magnitudes higher value"), second = percentage applied; plus angle.
- Phase strength: adds the final-z argument (phase channel) into the index for a "chrome"
  effect.
- Texture: an image distorted by the slope field.
- OpenGL: custom GLSL colouring, with an "Use sRGB" option that converts to linear light for
  the shader and back (KF's own default colouring works in sRGB values).
- Also: Load palette from image (PNG/JPEG), drag-and-drop of .kfp files.

## Takeaways

- Key colours + cyclic, eased interpolation + "iterations per palette cycle" is the deep-zoom
  community's mental model. Our palette format should store key colours (not a sampled table)
  and an independent density/offset.
- Users expect several transfer functions; log and sqrt are the important ones for dense
  regions, and a "stretched" (fit to image range) mode is the cheap version of histogram
  colouring.
- KF's sine-eased interpolation makes few-key palettes look stepped (plateaus at keys). An
  OKLab interpolation with linear or smooth-step blend is a deliberate design choice, not a
  copy.
- KF's random and wave tools produce endless variety without curation; a curated library
  should offer a few "procedural" palettes (waves, cosine) next to hand-made ones.
