---
title: "Places: a catalogue of famous views to visit"
type: synthesis
date: 2026-10-14
sources: ["[[sources/beyer-mandelbrot-zoom-sequence]]", "[[sources/muency-named-places]]", "[[sources/wikipedia-misiurewicz-point]]", "[[sources/robotmoon-mandelbrot-set]]"]
tags: [places, locations, misiurewicz, product, decision]
---

# Places: a catalogue of famous views to visit

**What.** A curated list of well-known, beautiful views of the Mandelbrot set
("places") that the app shows as a browsable list with thumbnails and can jump or fly to,
plus a random pick. Asked for by the user ("a list of well known coordinates with
landscapes that you can visit"); the market research lists curated destinations and
autopilot dives as expected features ([[synthesis/fractal-app-market-research]]).

**Where.** Data and API in `crates/almondbread-core/src/places.rs` (+ `places_data.rs`),
UniFFI in `crates/almondbread-ffi` (`places()`, `place(id)`, `random_place(seed,
category)`, `place_scale(id, w, h)`, `Explorer::go_to_place(id)`), thumbnails in
`Sources/AlmondbreadKit/Resources/Places/` (`PlaceThumbnails.url(id:retina:)`), CLI
`almondbread places`.

## Conventions

- **Centre** as decimal strings of any length (the deepest carry 1,050 digits).
- **Span**: complex extent across the view's **shorter** side, so a place frames the
  same in portrait (iPhone) and landscape windows. Sources give other extents: Beyer's
  Commons files the horizontal diameter of 4:3 images (span = 0.75 × it), Mu-Ency
  `@ r` the half-height (span = 2r), Robot Moon the vertical span (= span).
- **Look**: one of the existing looks (`palette::Look`), chosen per place by eye.
- **Iterations**: automatic for every place so far (`iterations: None`); the field exists
  for views that need a fixed limit. Interior detection stays on (no place needed it off).
- **Source**: every place cites where its coordinate comes from. `generated: true`
  marks centres we computed (Newton nuclei, Misiurewicz points) or framings we chose
  near a cited point. Descriptions are our own words.
- **Categories** (menu sections): landmark, valley, spiral, dendrite, minibrot,
  julia-island, deep (1e-30 and below). Within a category, shallow to deep.

## Generated coordinates

- Misiurewicz points were refined from Wikipedia's 8–15-digit approximations with a new
  Newton solver, `almondbread_math::find_misiurewicz` (`locate --center re,im
  --misiurewicz k,p --bits b`): Newton on `g = (z_{k+p} − z_k) / ∏_{i<k}(z_{i+p} − z_i)`,
  which divides out the lower-preperiod roots, with the derivative carried alongside.
  Wikipedia's seahorse spiral "M(23,2)" is M(24,2) with z_0 = 0 (orbit check; see
  [[sources/wikipedia-misiurewicz-point]]); M(4,1), M(3,1) converge in 4–8 steps up to
  3,500 bits.
- Minibrots: `almondbread locate` (ball period + Newton nucleus) as for the deep benchmark
  set ([[synthesis/benchmarks]] M3b). The starburst at 1e-112 is the period-477 nucleus
  5 radii from M(4,1) found with `--radius 1e-60`. Robot Moon's "pine compass" names a
  period-70 nucleus near -1.4303584327 but frames it at 1e-34 off-centre without giving
  the centre; `locate --radius 1e-10` finds the nucleus -1.430358432725985534067 (size
  6e-16), framed by us at 1.5e-12 (`pine-compass`) and 2e-14 (`crystal-compass`). Searching for a minibrot right at the
  M(24,2) spiral centre failed: Newton did not converge for periods 1,552–2,764 started at
  the Misiurewicz point itself.

## Checks (2026-10-14, M1 Pro, release)

- Every place renders headless at 320×200 and 640×400 (`--ss 3`) with its look; each
  thumbnail was looked at, and the command fails if one is flat (luma range < 40).
  Rejected or reframed while looking: a duplicate cusp view (same as elephant valley),
  the antenna tip at -2 (an empty grey line), the 1/3-limb tip M(3,1) (mostly empty),
  c = i framed exactly (moved the centre to -0.02 + 0.98i so the twigs fill the view),
  a deep 1/3-limb tip at 1e-1000 (wide grey bands), Beyer's images 11 and 14 at their own
  width (grey moiré at thumbnail size: zoomed 3× and 5×), two medallions zoomed 2×.
- Deep places through `verify --place id` against the CPU oracle at 160×100: class
  mismatch 0 % for all 15 places beyond f64 (1e-14 to 1e-1000); escape-n differs 0.08–18 %
  in the dense spiral and mandala views and 39 % at the seahorse minibrot, the known
  float32-vs-f64 chaos of dense regions ([[synthesis/benchmarks]] M3b: sea-8007 23 % at
  400 wide), and below 0.2 % at the needle minibrots.
- Thumbnails: HEIC through `sips` (quality 70), 2 × 69 files, 4.6 MB; JPEG at the same
  size was 2× larger.

## Using a place

```text
almondbread places                          # list: id, category, depth, look
almondbread render out.png --place spiral-1e1000 --size 1600x1000
almondbread places --render-thumbs Sources/AlmondbreadKit/Resources/Places --inside bubbles --extend 16 [--only a,b] [--oversample 3 --blur 0.5]
almondbread places --format landscape       # landscape-rt start commands (f64 places)
```

Thumbnails (re-rendered 2026-10-15 after the merge into the control-center branch) use the
calm dense regions and the *bubbles* interior colouring with extension passes
([[techniques/coloring-methods]] "Interior colouring as built"): the flat black interiors
hid most landmarks, and the dense bands of deep minibrots were grey. The three needle
minibrots (1e91, 1e302, 1e1000) showed moiré: their radial spokes and ring bands come
within a pixel or two of the thumbnail grid, and the calm dense colouring does not touch
them (it keys on the radial band density, the spokes are angular). More samples alone did
not fix it: 9 → 81 samples per pixel (3× oversampling with a box filter) still left beats
near the Nyquist rate, because a box filter passes them. Since 2026-10-15 `--render-thumbs`
renders 3× larger (`--oversample 3`, times the `--ss 3` grid) and resamples with a Gaussian
of σ 0.5 output pixels in linear light (`--blur 0.5`; response 0.29 at Nyquist instead of
the box's 0.64); the needle thumbnails were re-rendered with it and the beats are gone
(before/after looked at, `build/thumbs/`; the remaining fine rings are real bands). The
other 66 thumbnails were not re-rendered (no visible aliasing). In the app,
Go and Fly to a place switch a flat interior to bubbles, so the view matches its thumbnail;
the control center's Places section shows them as a grid by category with Go, Fly
(`Explorer.flyToPlace`) and Random.

Swift: `places()` for the list (sections by `category`), `PlaceThumbnails.url(id:)`
for the image, `explorer.goToPlace(id:)` to jump (centre, span fitted to the view's
shorter side, look, automatic iterations), or `placeScale(id:width:height:)` with
`setViewportText` for a fly-to target. `randomPlace(seed:category:)` for "surprise me".

**Landscape prototype** (`prototypes/landscape-rt`, f64 only): its `--loc` takes a name or
`re,im`. Its names map to places: `seahorse` ≈ `julia-island` centre (Beyer's seahorse
point), `elephant` = `elephants`, `minibrot3` = `airplane`, `spiral` ≈ the mirror image of
`two-arm-spiral`. Any place within f64 starts with
`--loc=re,im --scale S`, S = span / 30 (it starts 2 S off the boundary, fog at 60 S);
`almondbread places --format landscape` prints these lines.

## The catalogue

<!-- places-table -->
71 places (landmark 11, valley 14, spiral 7, dendrite 4, minibrot 9, julia-island 17, deep 9). By magnification: < 1e5: 36, 1e5-1e13: 20, 1e13-1e30: 6, 1e30-1e300: 5, >= 1e300: 4. Depth is log10 of the magnification relative to the whole set (span 3); `almondbread places --format tsv` prints centres and spans.

| id | name | category | depth (log10 zoom) | look | source |
| --- | --- | --- | --- | --- | --- |
| `whole-set` | The Whole Set | landmark | 0.1 | aurora | [[sources/beyer-mandelbrot-zoom-sequence]] 00 |
| `head-and-shoulders` | Head and Shoulders | landmark | 0.9 | lagoon | [[sources/beyer-mandelbrot-zoom-sequence]] 01 |
| `pi-at-the-neck` | π at the Neck | landmark | 1.6 | aurora | [[sources/klebanoff-pi-in-mandelbrot]] (Boll 1991); ε = 0.02 on -3/4 + iε, [[techniques/pi-in-the-mandelbrot-set]] |
| `pi-at-the-cusp` | π at the Cusp | landmark | 3.7 | aurora | [[sources/klebanoff-pi-in-mandelbrot]]; ε = 2e-4 on 1/4 + ε, [[techniques/pi-in-the-mandelbrot-set]] |
| `the-spike` | The Spike | landmark | 0.3 | ember | [[sources/muency-named-places]]: R2F(1/2B) (the spike / main antenna) |
| `third-bulb` | The 1/3 Bulb | landmark | 0.9 | dusk | Nucleus of period 3 (root of c³ + 2c² + c + 1), computed with almondbread locate (generated) |
| `quarter-bulb` | The 1/4 Bulb | landmark | 0.8 | moss | [[sources/muency-named-places]]: R2.1/4 |
| `fifth-bulb` | The 1/5 Bulb | landmark | 1.1 | spectrum | [[sources/muency-named-places]]: R2.1/5 |
| `two-fifths-bulb` | The 2/5 Bulb | landmark | 0.9 | classic | [[sources/muency-named-places]]: R2.2/5 |
| `five-eighths-limb` | The 5/8 Limb | landmark | 1.3 | aurora | [[sources/muency-named-places]]: R2F(5/8B) |
| `feigenbaum-point` | Feigenbaum Point | landmark | 3.2 | spectrum | [[sources/muency-named-places]]: Feigenbaum point |
| `seahorse-valley` | Seahorse Valley | valley | 1.9 | aurora | [[sources/beyer-mandelbrot-zoom-sequence]] 02 |
| `seahorse` | A Seahorse | valley | 2.4 | lagoon | [[sources/beyer-mandelbrot-zoom-sequence]] 03 |
| `elephant-valley` | Elephant Valley | valley | 1.2 | ember | [[sources/muency-named-places]]: Elephant Valley (R2.C(0)) |
| `elephant-herd` | Elephant Herd | valley | 2.3 | ember | [[sources/muency-named-places]]: R2.C(0) |
| `elephants` | Among the Elephants | valley | 3.5 | moss | [[sources/muency-named-places]]: R2.C(0) |
| `triple-spiral-valley` | Triple Spiral Valley | valley | 2.3 | nebula | [[sources/muency-named-places]]: R2.C(1/3) |
| `quad-spiral-valley` | Quad Spiral Valley | valley | 2.4 | ice | [[sources/muency-named-places]]: Quad Spiral Valley and Rational Coordinates (1/4 + 1/2 i) (generated) |
| `scepter-valley` | Scepter Valley | valley | 2.6 | classic | [[sources/muency-named-places]]: R2.1/2.C(1/2) (Scepter Valley) |
| `scepter` | A Scepter | valley | 3.5 | ember | [[sources/muency-named-places]]: R2.1/2.C(1/2) |
| `scepter-crown` | Scepter Crown | valley | 3.8 | ember | [[sources/robotmoon-mandelbrot-set]] |
| `double-scepter-valley` | Double Scepter Valley | valley | 2.8 | lagoon | [[sources/muency-named-places]]: R2.1/3.C(1/2) (Double Scepter Valley) |
| `peacock-eye` | Peacock Eye | valley | 2.6 | spectrum | [[sources/muency-named-places]]: Peacock eye |
| `shepherds-crook` | Shepherd's Crook | valley | 5.1 | graphite | [[sources/muency-named-places]]: Shepherds crook |
| `double-hook` | Double Hook | valley | 4.5 | aurora | [[sources/beyer-mandelbrot-zoom-sequence]] 06 |
| `spiral-sanctuary` | Spiral Sanctuary | spiral | 2.7 | lagoon | [[sources/robotmoon-mandelbrot-set]] |
| `triple-spiral` | Triple Spiral | spiral | 3.2 | dusk | [[sources/muency-named-places]]: R2.C(1/3), figure 2 |
| `seahorse-tail` | Seahorse Tail | spiral | 3.2 | nebula | [[sources/beyer-mandelbrot-zoom-sequence]] 04 |
| `tail-part` | Inside the Tail | spiral | 3.7 | moss | [[sources/beyer-mandelbrot-zoom-sequence]] 05 |
| `two-arm-spiral` | Two-Arm Spiral | spiral | 4.5 | lagoon | [[sources/wikipedia-misiurewicz-point]] + `locate --misiurewicz` (generated) |
| `satellite-double-spiral` | Satellite Double Spirals | spiral | 8.4 | aurora | [[sources/beyer-mandelbrot-zoom-sequence]] 11 |
| `spiral-wheel` | Spiral Wheel | spiral | 9.0 | ember | [[sources/beyer-mandelbrot-zoom-sequence]] 12 |
| `dendrite-i` | The Tip at i | dendrite | 1.7 | ice | [[sources/wikipedia-misiurewicz-point]] + `locate --misiurewicz` (generated) |
| `three-way-junction` | Three-Way Junction | dendrite | 2.8 | dusk | [[sources/wikipedia-misiurewicz-point]] + `locate --misiurewicz` (generated) |
| `antenna-junction` | Antenna Crossroads | dendrite | 1.8 | ember | [[sources/wikipedia-misiurewicz-point]] + `locate --misiurewicz` (generated) |
| `snowflake` | Snowflakes | dendrite | 3.5 | ice | [[sources/robotmoon-mandelbrot-set]] |
| `airplane` | The Airplane | minibrot | 1.9 | nebula | Nucleus of period 3 on the real axis, computed with almondbread locate (generated) |
| `period-4-minibrot` | Period-4 Minibrot | minibrot | 2.9 | classic | benchmark set ([[synthesis/benchmarks]]), `locate` (generated) |
| `third-limb-island` | Island on the 1/3 Limb | minibrot | 1.7 | moss | [[sources/muency-named-places]]: R2F(1/3B1)S |
| `satellite` | Satellite | minibrot | 5.4 | aurora | [[sources/beyer-mandelbrot-zoom-sequence]] 07 |
| `satellite-antenna` | Satellite Antenna | minibrot | 6.1 | lagoon | [[sources/beyer-mandelbrot-zoom-sequence]] 08 |
| `satellite-head` | Satellite Head and Shoulders | minibrot | 6.8 | nebula | [[sources/beyer-mandelbrot-zoom-sequence]] 09 |
| `satellite-seahorse-valley` | Satellite Seahorse Valley | minibrot | 7.5 | dusk | [[sources/beyer-mandelbrot-zoom-sequence]] 10 |
| `pine-compass` | Pine Compass | minibrot | 12.3 | moss | [[sources/robotmoon-mandelbrot-set]] (generated) |
| `crystal-compass` | Crystal Compass | minibrot | 14.2 | ice | [[sources/robotmoon-mandelbrot-set]] (generated) |
| `embedded-julia` | Embedded Julia Set | julia-island | 5.2 | ember | [[sources/muency-named-places]]: Embedded Julia Set |
| `cantor-island` | Cantor Island | julia-island | 6.7 | moss | [[sources/muency-named-places]]: Favorite Views 1992 |
| `paramecium` | Paramecium | julia-island | 6.3 | ice | [[sources/muency-named-places]]: Paramecia |
| `cauliflower-medallion` | Cauliflower Medallion | julia-island | 6.0 | moss | [[sources/muency-named-places]]: Cauliflower Medallion |
| `triple-spiral-medallion` | Triple Spiral Medallion | julia-island | 6.6 | nebula | [[sources/muency-named-places]]: Triple Spiral Medallion |
| `elephant-medallion` | Elephant Medallion | julia-island | 7.0 | ember | [[sources/muency-named-places]]: Elephant Medallion |
| `seahorse-medallion` | Seahorse Medallion | julia-island | 8.4 | spectrum | [[sources/muency-named-places]]: Seahorse Medallion |
| `second-order-julia` | Julia Within Julia | julia-island | 8.3 | classic | [[sources/muency-named-places]]: Second-Order Embedded Julia Set |
| `two-fold-julia` | Two-Fold Julia | julia-island | 9.9 | dusk | [[sources/muency-named-places]]: 2-fold Embedded Julia Set |
| `julia-island-tail` | Island in the Tail | julia-island | 9.8 | nebula | [[sources/beyer-mandelbrot-zoom-sequence]] 13 |
| `julia-island` | Julia Island | julia-island | 11.6 | lagoon | [[sources/beyer-mandelbrot-zoom-sequence]] 14 |
| `spiral-ring-island` | Ring of Spirals | julia-island | 11.2 | aurora | [[sources/muency-named-places]]: Favorite Views 1994 |
| `four-fold-julia` | Four-Fold Julia | julia-island | 14.4 | spectrum | [[sources/muency-named-places]]: 4-fold Embedded Julia Set |
| `sunflower-island` | Sunflower Island | julia-island | 15.1 | dusk | [[sources/muency-named-places]]: Favorite Views 1993 |
| `third-order-satellite` | Third-Order Satellite | julia-island | 19.0 | nebula | [[sources/muency-named-places]]: Favorite Views 1994 |
| `copper-mandala` | Copper Mandala | julia-island | 21.7 | ember | [[sources/muency-named-places]]: Second-Order Embedded Julia Set |
| `frost-mandala` | Frost Mandala | julia-island | 27.0 | ice | [[sources/muency-named-places]]: Favorite Views 1994 |
| `seahorse-minibrot-1e31` | Minibrot of the Seahorse | deep | 31.2 | aurora | benchmark set ([[synthesis/benchmarks]]), `locate` (generated) |
| `spiral-1e50` | Spiral at 1e50 | deep | 50.5 | lagoon | [[sources/wikipedia-misiurewicz-point]] + `locate --misiurewicz` (generated) |
| `needle-minibrot-1e91` | Needle Minibrot | deep | 91.5 | aurora | benchmark set ([[synthesis/benchmarks]]), `locate` (generated) |
| `junction-1e100` | Junction at 1e100 | deep | 100.5 | nebula | [[sources/wikipedia-misiurewicz-point]] + `locate --misiurewicz` (generated) |
| `starburst-1e112` | Starburst | deep | 112.5 | dusk | Generated: almondbread locate --radius 1e-60 at M(4,1) (generated) |
| `junction-1e300` | Junction at 1e300 | deep | 300.5 | dusk | [[sources/wikipedia-misiurewicz-point]] + `locate --misiurewicz` (generated) |
| `needle-minibrot-1e302` | Needle Minibrot at 1e302 | deep | 302.5 | aurora | benchmark set ([[synthesis/benchmarks]]), `locate` (generated) |
| `spiral-1e1000` | Spiral at 1e1000 | deep | 1000.5 | ice | [[sources/wikipedia-misiurewicz-point]] + `locate --misiurewicz` (generated) |
| `needle-minibrot-1e1000` | Needle Minibrot at 1e1000 | deep | 1000.9 | spectrum | benchmark set ([[synthesis/benchmarks]]), `locate` (generated) |
