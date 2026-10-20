---
title: "Overlay text: SDF font atlas in Rust and Metal"
type: platform
status: draft
sources: ["[[sources/inter-font]]"]
tags: [overlay, font, sdf, text, metal, rendering]
---

# Overlay text: SDF font atlas in Rust and Metal

The orbit overlay and the mini-map draw their text in Rust and the `overlay` kernel
(`crates/almondbread-render/src/overlay.rs`, `src/font/`), not in SwiftUI, so every
platform the renderer runs on gets the same labels ([Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](render-architecture.md),
[Controls: keyboard, pointer and touch](../decisions/controls.md) "Orbit overlay and mini-map"). Built 2026-10-15.

## Before

A 5×7 bitmap font, box-filtered per pixel (exact coverage of the font pixels). At Retina
sizes it looked like a 1980s terminal, and its four-copy black shadow (offset by 0.6 cell)
disappeared on pale palettes: `+0.451` over the white boundary glow was unreadable
(`build/font/before-orbit-map.png`, looked at).

## As built

- **Font: Inter SemiBold 4.1** (Rasmus Andersson, SIL Open Font License 1.1; the licence is
  `crates/almondbread-render/src/font/OFL.txt`, shipped with the atlas). OFL permits bundling
  and redistribution with software as long as the copyright notice and licence travel with
  it; we treat the atlas as derived from the font and ship both next to it (our reading of
  the OFL, unverified by a lawyer; the reserved font name only matters for a renamed font
  file, which we do not ship). Semibold reads better than Regular over busy fractal texture.
- **Atlas: single-channel signed distance field.** `examples/gen_overlay_font.rs` (dev
  dependency `fontdue`, MIT/Apache) rasterises each glyph at 8× the atlas size, thresholds it,
  runs an exact Euclidean distance transform both ways (Felzenszwalb–Huttenlocher, our own
  code) and samples the signed distance at the atlas texel centres (mean of the four hi-res
  pixels around each centre). 40 texels per em, distances clamped to ±8 texels and stored as
  `128 + d/8·127` (inside positive). 109 glyphs (ASCII plus `λ × · ° … – − μ π ≈ → ← ↑ ↓`),
  shelf-packed into 512 × 392 bytes (196 KB, `include_bytes!`), metrics as a generated Rust
  table (`src/font/metrics.rs`).
- **Layout** (`font::layout`, platform-neutral): advances from the font, no kerning; digits
  share the widest digit's advance (tabular figures by centring), so changing numbers do not
  jitter. The caller gives the top of the capitals and the size in pixels per em.
- **Kernel** (`glyphDistance` in `mandelbrot.metal`): one primitive per glyph (quad in output
  pixels, atlas cell in `param`, atlas width, halo width and spread as float bits in `info`).
  Bilinear sample of the atlas (four byte loads from the existing font buffer slot), distance
  in output pixels `d = (v−128)/127 · spread · s` with `s` output pixels per texel; fill
  coverage `saturate(d + 0.5)` (a one-pixel ramp: crisp at any scale), halo
  `saturate((d + w)/max(0.4w, 1))` (opaque next to the glyph, fading over its outer 40 %),
  fill composited over the halo.
- **Halo**: `label` uses black at 85 % of the text alpha, width `max(0.2·size, 2.5)` px,
  clamped to what the atlas stores (`spread·s − 0.25`). The first try (spread 5 texels,
  width 0.16·size, hard edge) was too thin over pale yellow; spread 8 and the soft edge fixed
  it.
- **Sizes**: map level labels 11 pt, info lines 13 pt (× pixels per point), line height
  1.35 em.

## Checks (2026-10-15, M1 Pro, release)

- `almondbread render … --orbit x,y --map 1 [--ppp 0.5]` (`--ppp` now also sets the overlay's
  points per pixel, 0.5 = Retina): stills at 1× and 2× over the deep-blue seahorse view and
  over the pale yellow exterior at `-0.74529+0.11307i`, scale 1e-5 (`build/font/after-*.png`,
  crops looked at): crisp glyphs, legible on both; at 1× (13 px em) the halo is necessarily
  thin (2.35 px reach).
- `scripts/ui-screenshots.sh` (`orbit`, `deep`): window captures at 2× show the labels on the
  map panels (`×1e84` over a cream panel) and the info lines.
- iOS simulator: the same overlays render in the iPhone screenshots
  ([iOS and iPadOS on the simulator: setup, checks, what broke](ios-simulator.md)).
- Tests: atlas size matches the metrics, the table is sorted, a cell's corner is far outside
  and an `l` stem is inside, digits are tabular, one primitive per glyph, halo width clamped.

## Pitfalls

- The atlas stores distances only out to `spread`; a halo wider than `spread·s` would end in
  a hard edge where the clamp starts, hence the clamp in `text_halo`.
- Regenerating the atlas rewrites `metrics.rs`; run `cargo fmt` afterwards (the table itself
  is `#[rustfmt::skip]`).
