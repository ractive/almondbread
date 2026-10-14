---
title: "Wikibooks: Triangle inequality average (TIA) colouring"
type: source
authors: ["Wikibooks contributors"]
year: 2024
raw: "web"
status: unread
tags: [coloring-methods, triangle-inequality-average, curvature-average]
---

# Wikibooks: Triangle inequality average (TIA) colouring

**URL:** <https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/triangle_ineq> · **Saved:** not saved (`raw: "web"`) · **Terms:** CC BY-SA (Wikibooks)

## What it is

Wikibooks page on averaging colouring methods, in particular Kerry Mitchell's Triangle Inequality
Average (TIA) and the curvature average. Not saved or read for this wiki; the formulas below are the
commonly published forms, not checked against the page. The year is a placeholder.

## What it covers

- TIA (unverified): per iteration `t_n = (|z_n| - m_n) / (M_n - m_n)` with `m_n = | |z_{n-1}^2| - |c| |` and `M_n = |z_{n-1}^2| + |c|` (the triangle-inequality bounds on `|z_n|`); average `t_n` over the orbit (skipping the first iterations), then blend the last two averages using the smooth iteration fraction.
- Curvature average (unverified): `t_n = |arg((z_n - z_{n-1}) / (z_{n-1} - z_{n-2}))|`, averaged likewise.
- These need the full orbit value `z_n` each iteration, so in a perturbation renderer they use `Z + z` (inference); KF's TIA requires OpenCL and disables series approximation ([[sources/kf-manual]] §Tri.Ineq.Avg.).

## Reliability

Secondary; not read (status unread). Formulas marked unverified until the page is fetched and checked.
[[sources/cheritat-mandelbrot-rendering]] §Radial strands mentions TIA and prefers stripe average.

## Useful sections

- Not recorded (page not read).

## Used by

[[techniques/coloring-methods]], [[techniques/smooth-coloring]]
