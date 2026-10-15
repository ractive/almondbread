# Ultra Fractal gradients and the .ugr format

Accessed 2026-10-14. Sources:

- UF help, gradients: https://www.ultrafractal.com/help/gradients/gradients.html,
  .../how.html, .../editing.html, .../transparency.html, .../adjust.html,
  .../gradientfiles.html
- UF help, colouring settings (density, transfer, offset, repeat):
  https://www.ultrafractal.com/help/coloring/coloringsettings.html
- UF help, file types: https://www.ultrafractal.com/help/browsers/filetypes.html
- UF tutorial with a pasteable gradient block:
  https://www.ultrafractal.com/help/tutorials/basicskills/bs-saveufr.html
- Wikibooks "Fractals/ultrafractal" (parameter sets with `gradient:`/`opacity:` blocks):
  https://en.wikibooks.org/wiki/Fractals/ultrafractal
- Independent readers/writers confirming the colour encoding:
  JWildfire `UgrGradientReader.java`
  (https://github.com/thargor6/JWildfire/blob/master/src/org/jwildfire/create/tina/io/UgrGradientReader.java),
  and T. Ford's "Newton's Color Wheel" generator, https://www.cattail.nu/newton/ (`works.js`).
- ChaosPro history notes on .ugr import:
  https://www.chaospro.de/documentation/html/appendix/history/history3_3.htm

Licences: Ultra Fractal is proprietary software (Frederik Slijkerman); its help text is
copyright and is paraphrased here. Gradients that ship with UF, or are posted by users, carry
their authors' copyright unless stated otherwise; we only use the file format (a format is not
copyrightable) and our own example values. Wikibooks text: Creative Commons Attribution-ShareAlike (page footer; version not
checked, Wikibooks currently uses 4.0).
JWildfire: LGPL-2.1-or-later (file header). Newton's Color Wheel: no licence stated.

## How UF uses a gradient

- The colouring algorithm returns a float "index" per pixel; the gradient maps index to colour.
  Only indices are stored, so recolouring needs no recalculation.
- A gradient has 400 entries. Index 1.0 spans the whole gradient. The gradient wraps at its ends
  (the last control point blends into the first), so smooth images need no seam handling.
- Per inside/outside colouring there are these settings, applied in this order:
  1. Color Density: index is multiplied by it (must be > 0).
  2. Transfer Function on (index * density): None (solid colour), Linear, Sqr, Sqrt, Cube,
     CubeRoot, Log (natural log), Exp, Sin (repeats, range -1..1), ArcTan (saturates at pi/2).
  3. Gradient Offset: added after the transfer, in gradient entries 0..399 (same effect as
     rotating the gradient but separate for inside and outside).
  4. Repeat Gradient: if on, the index wraps at 1 (0, 1, 2, 3 map to the same colour); if off,
     it clamps, so every colour is used once.
- Gradient editor: Red/Green/Blue curves, Hue/Saturation/Luminance curves, an Opacity curve and
  a Controls view for typing values. Control points carry position and colour; "Smooth Curves"
  toggles linear vs smooth (spline) interpolation between them. A rotation slider (always
  visible) rotates the whole gradient. The opacity curve can have its own control points or be
  "linked" to the colour points.
- Whole-gradient commands: Adjust Colors (balance, hue, saturation, brightness, contrast),
  Randomize / Randomize Bright / Randomize Misty / Randomize Custom, Reverse (mirror positions),
  Invert (colours or opacity).
- Gradient files (*.ugr) are plain text and hold any number of gradients. UF also opens older
  *.ual gradient files and Fractint *.map palettes as gradients.

## .ugr / gradient text syntax

One entry per gradient, the same text UF puts on the clipboard (Copy/Paste in the editor):

```
<entry-name> {
gradient:
  title="<title>" smooth=yes|no rotation=<int>
  index=<int> color=<int> index=<int> color=<int> ...
opacity:
  smooth=yes|no index=<int> opacity=<0..255> ...
}
```

Facts a parser needs:

- Tokens are whitespace-separated `key=value` pairs; line breaks are not significant
  (the published examples wrap at about 70-80 columns, and an `index=` can end one line with
  its `color=` on the next). `title="..."`
  and `comments="..."` are double-quoted and may contain spaces and commas (the tutorial's
  entry name and title contain a comma).
- Section keywords: `gradient:` then `opacity:`. Keys seen in `gradient:`: `title`, `comments`,
  `smooth`, `rotation`, `linked` (yes when colour and opacity share points), then repeated
  `index`/`color` pairs. Keys in `opacity:`: `smooth`, then repeated `index`/`opacity` pairs.
  `opacity` is 0 (transparent) to 255 (opaque); default is a single point `index=0 opacity=255`.
- `index` is the control point position in gradient entries, nominally 0..399 (400 entries;
  confirmed by the colouring-settings page). A gradient with a single point is a solid colour.
- `color` is a decimal integer packed as Windows COLORREF, i.e. 0x00BBGGRR:
  `color = R + 256*G + 65536*B`; decode `R = c & 0xFF, G = (c >> 8) & 0xFF,
  B = (c >> 16) & 0xFF`. Evidence: Newton's Color Wheel builds the value from the hex string
  B,G,R (`h2d(d2h(B)+d2h(G)+d2h(R))`), and JWildfire reads the int as ARGB then swaps red and
  blue. Sanity check on a real gradient (Wikibooks "troubledTree4"):
  `index=0 color=8716288` (0x850000 -> R0 G0 B133, dark blue), `index=100 color=16121855`
  (0xF5FFFF -> R255 G255 B245, near white), `index=200 color=46591` (0x00B5FF -> R255 G181 B0,
  orange), `index=300 color=156` (R156 G0 B0, dark red). That blue-white-orange-dark-red
  sequence is the familiar UF default look (unverified that it is byte-identical to the
  default).
- `rotation` is stored with the gradient; presumably it shifts the mapping by that many
  entries, the same unit as Gradient Offset (unverified: not documented on the pages read).
- Indices need not be sorted: ChaosPro had to fix an import bug "when the base control points
  were not sorted", which happens after a UF gradient has been rotated. Sort by index (mod 400)
  before building the table, and treat the gradient as cyclic: interpolate from the last point
  through 400 back to the first.
- Comment lines start with `;` (Newton's Color Wheel writes `; Gradients by ...` headers, and
  JWildfire 0.72 added "skip comments in *.ugr files").
- JWildfire's reader shows a robust, permissive strategy: split the file on `{` ... `}`, accept
  a block if it contains `title="`, `index=` and `color=`, and take `index=`/`color=` pairs in
  order. It pads missing index 0 and 399 with black, which is wrong for UF semantics (UF wraps);
  do not copy that.
- In UF parameter files (*.upr) the same `gradient:` / `opacity:` sections appear inside a
  fractal's parameter block, e.g. after the layer's `inside:`/`outside:` sections.

Example (our own values, a 4-point cyclic gradient):

```
almondbread-example {
gradient:
  title="Example" smooth=yes rotation=0 index=0 color=8716288
  index=100 color=16777215 index=200 color=46591 index=300 color=156
opacity:
  smooth=no index=0 opacity=255
}
```

Public examples (verbatim formats in sources): tutorial "Gradient-Fractal2,Background" with
`smooth=yes rotation=155` and three points at 84, 247, 332; Wikibooks "mbrot2_slope" with
`comments="..." smooth=no rotation=1` and five points (0, 64, 168, 257, 343); "Lichtenberg"
with two points `index=0 color=16777215 index=399 color=0` (white to black).

## Practice worth copying

- Separate the gradient (colours) from the mapping (density, transfer, offset, repeat): one
  gradient serves many locations.
- Density as a multiplier plus a choice of transfer (sqrt and log for deep, dense images where
  the iteration count range is huge) covers most needs; Sin gives a built-in ping-pong.
- Smooth (spline) interpolation between few control points (4-8) is the common UF style;
  linear interpolation shows "kinks" at control points in smooth colouring.
- Opacity per control point exists because UF composites layers; a single-layer explorer can
  ignore opacity on import but should not reject files that carry it.
