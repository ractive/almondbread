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
  5 radii from M(4,1) found with `--radius 1e-60`. Searching for a minibrot right at the
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
  mismatch 0 % for all 14 places beyond f64 (1e-14 to 1e-1000); escape-n differs 0.08–18 %
  in the dense spiral and mandala views and 39 % at the seahorse minibrot, the known
  float32-vs-f64 chaos of dense regions ([[synthesis/benchmarks]] M3b: sea-8007 23 % at
  400 wide), and below 0.2 % at the needle minibrots.
- Thumbnails: HEIC through `sips` (quality 70), 2 × 67 files, 4.3 MB; JPEG at the same
  size was 2× larger.

## Using a place

```text
almondbread places                          # list: id, category, depth, look
almondbread render out.png --place spiral-1e1000 --size 1600x1000
almondbread places --render-thumbs Sources/AlmondbreadKit/Resources/Places [--only a,b]
almondbread places --format landscape       # landscape-rt start commands (f64 places)
```

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
