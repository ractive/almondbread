---
title: "Landscape mode: fly over the Mandelbrot set as 2.5D terrain"
type: decision
date: 2026-10-14
status: proposed
tags: [3d, landscape, flight, navigation, product]
---

# Landscape mode: fly over the Mandelbrot set as 2.5D terrain

## Context

The user wants to fly through a 3D fractal landscape and asked how that
combines with zooming. True 3D fractals (Mandelbulb, Mandelbox) have no
practical deep-zoom scheme ([[formulas/others-feasibility]]); the 2D
engine does ([[techniques/perturbation]], [[techniques/bla]]).

## Proposal

- **Terrain:** height = f(exterior distance estimate)
  ([[techniques/distance-estimation]]); the set boundary is the valley
  floor at 0, interior regions (minibrots) are flat lakes, the exterior
  rises. DE scales with zoom, so the terrain is self-similar at every
  depth. Candidate f: linear, or a soft log curve so plateaus do not
  dominate; decided by prototype images.
- **One fixed world; zoom is just distance:** h(c) is a fixed function of
  the location, independent of any zoom level; the terrain merely looks
  alike at every scale. There is no separate zoom parameter: the camera
  moves in 3D, perspective makes nearby ground larger, and each part of
  the ground is computed at the level of detail its distance needs (LOD).
  Sinking a little is a small zoom; a dive is a lot of sinking. New detail
  fades in (LOD blending, no popping).
- **Flight:** speed proportional to height above ground, so descending
  from 1e3 to 1e300 feels uniform. Terrain-following clearance is measured
  against the ground smoothed to the scale of the camera's altitude, so
  small side canyons do not make the camera twitch. Level flight = constant
  detail scale through valleys and over ridges; nose-dive = zoom in; pull
  up = zoom out. Camera position in arbitrary precision; height stored
  relative to the current scale.
- **Flight recorder:** every flight is recorded (position, clearance,
  heading per sample; exact coordinates as decimal strings, kilobytes per
  flight). Uses: rewind ("go back in time") and resume flying from any
  point, replay, video export of the flown path, sharing a whole flight,
  automatic waypoints at dives and sharp turns.
- **Rendering:** nested rings of 2D renders around the point below the
  camera, each at its own scale (clipmap / exponential-map idea,
  [[techniques/exponential-map]]), reused while flying; height-field ray
  marching with the existing normals, fog, sky, water, shadows.
- **Controls:** the crosshair and keyboard model of [[decisions/controls]]
  carries over (Space = dive towards the crosshair, WASD = fly/steer).

## Plan

1. Prototype (after R2b): still images from the Rust CLI at a few
   locations and depths, height-formula variants side by side; the user
   judges the look.
2. If it convinces: a landscape-mode milestone after BLA/LA (M4/M6), since
   the near rings need fast iteration while moving.
3. True 3D fractals stay a separate later track ("3D worlds", limited to
   about 1e4–1e5 magnification in float32).

## Open questions

Height function; clearance policy (fixed multiple of scale vs adjustable
dive speed); interior surface; cost of the near rings at depth while
moving.
