# fractalset.com: colouring options of the web viewer

Accessed 2026-10-14. Sources:

- https://fractalset.com/ (home, gallery cards with palette labels)
- https://fractalset.com/assets ("Gradients": community gradient editor and the built-in list)
- https://fractalset.com/about
- The site's client JavaScript bundles (Next.js chunks under `/_next/static/chunks/`), read
  to find the viewer's controls and URL parameters. Observations below are our own reading of
  minified code; they describe behaviour, not copied code.

Licence: none stated. The footer only says "© 2026 FractalSet". Treat the built-in palette
colours, names and gradient definitions as proprietary. Nothing here copies their colour data;
use the observations only as design input.

## What the viewer offers

- Built-in palettes: 24 plus a "Custom Gradient" slot. Names (labels only): Original, Aurora
  (the default), Inferno, Ocean, Electric, Monochrome, Sunset, Toxic, Gold, Twilight, Ice,
  Spectrum, Neon, Copper, Pastels, Deep Sea, Plasma, Tropical, Mandelbrot, Lava, Fall, Pop,
  Jewellery, Sky & Water. Names are mood words, not data-vis names; "Inferno" and "Plasma"
  borrow matplotlib names but nothing indicates the data is the same.
- Each built-in palette is a short list of RGB key colours (most have roughly 5 to 10 keys)
  interpolated linearly in RGB. Some built-ins are defined as stacked gradient layers, each
  with its own stops, interpolation mode, blend mode (e.g. additive), opacity percentage and a
  repeat count, so a fine repeating band can be laid over a slow base gradient.
- Palette density: an integer slider from -25 to +25 (URL parameter `d`, default 0). The
  factor applied to the smooth iteration count is `2^(d/10)`, i.e. about 0.18x to 5.7x, with
  10 steps per doubling. A log scale for density is the right UI choice: equal slider steps
  give equal perceived changes in band width.
- Palette rotation: -180 to +180 degrees (URL parameter `r`). It is a phase offset of the
  palette cycle, not a hue rotation.
- Max iterations slider: 128 to 4096 in steps of 64 (URL parameter `i`, default 256).
- Camera rotation (`cr`), fractal type (Mandelbrot, Julia with c, Burning Ship), and a shader
  mode (`s`): Smooth (default, "long-period sine"), Angle ("last-z argument"), Contour
  ("topographic lines"), Curvature ("angular change"), Neon ("glowing edges"), Rings
  ("concentric bands"), Stripes ("flowing textures"), Orbit Trap ("circular trap") and Zebra
  ("high-contrast stripes").
- Animation modes: Off, Color Scroll (palette phase flows over time), Density Pulse (density
  oscillates, "bands breathe") and Hue Shift (spectrum cycles).
- Interior: plain black. No interior colour option was found.
- Smooth colouring is on by default; the state is shareable as URL parameters (palette id,
  density, rotation, shader mode, iterations, centre, zoom).

## How the palette is indexed (smooth mode)

From the fallback/preview renderer in the client code (our paraphrase):

```
mu = n - log2(log2(|z|^2)) + 4          # bailout |z|^2 > 65536
phase = 2*pi * mu * 2^(d/10) / 400 + radians(r)
t = 0.5 + 0.5 * sin(phase)              # t in [0,1]
colour = palette(t)                     # linear interpolation between key colours
```

Design points worth keeping:

- One full palette sweep covers 400 iterations at density 0 (the same 400 as Ultra Fractal's
  gradient length, possibly a coincidence).
- The sine makes the mapping a ping-pong: t runs 0 -> 1 -> 0 per cycle. Any palette is then
  seamless without having to be cyclic, because the ends never meet. The cost: the two ends of
  the palette appear twice as wide as the middle (the sine dwells at its extremes), and the
  palette's direction reverses every half period, so a "dark to bright" palette produces
  symmetric bands rather than a sawtooth.
- Density and rotation are separate from the palette, so one palette serves both sparse and
  dense views.

## Community gradients

- The /assets page is an editor: stops are dragged on a bar or added by clicking; each stop has
  a position in percent and an HSL colour. A new gradient starts with 5 stops (start, three
  mid, end) and has a maximum stop count. "Add Midpoint" inserts a stop.
- Gradients get a generated two-word name: a word picked from the dominant hue family (families
  such as solar, verdant, aqua, cobalt, orchid, spectral, monochrome; e.g. "Cobalt", "Jade",
  "Graphite") plus a word chosen by brightness/energy (e.g. "Pulse", "Bloom" for vivid,
  "Drift", "Haze" for calm). Nice touch for a palette library: names derived from content.
- Gradients can be published to a public feed or saved privately and opened in the viewer.

## Takeaways for almondbread

- About 25 palettes is the size of a curated "starter" set; mood names work well.
- Log-scaled density slider, phase offset in degrees, and colour-scroll animation are the
  baseline controls users expect.
- Ping-pong mapping is a cheap way to make non-cyclic palettes (e.g. viridis) usable in a
  repeating fractal colouring; offer it as a per-palette or global "mirror" option rather than
  forcing it, because it halves the visible variety per period.
