---
title: "Wikibooks: Fractals/fraktaler-3"
type: source
authors: ["Wikibooks contributors"]
year: 2024
raw: "web"
status: unread
tags: [fraktaler-3, exr, bailout, exponential-map, anti-aliasing]
---

# Wikibooks: Fractals/fraktaler-3

**URL:** <https://en.wikibooks.org/wiki/Fractals/fraktaler-3> · **Saved:** not saved (`raw: "web"`) · **Terms:** CC BY-SA (Wikibooks)

## What it is

Wikibooks page (part of Adam Majewski's "Fractals" book) about Fraktaler 3: usage notes and examples,
including the metadata found in Fraktaler 3 EXR output. Not saved or read in full for this wiki; the
facts below come from the research notes. The year is a placeholder (revision date not checked).

## What it covers

- Example EXR metadata from a Fraktaler 3 render: `bailout.iterations = 262144`, `bailout.maximum_perturb_iterations = 4096`, `bailout.inscape_radius = 0.000976562` (1/1024), `image.subframes = 64`. This shows a real user setting: a high iteration limit with a perturbation-step cap of a few thousand, and 64 samples per pixel.
- Exponential map rendering is available from the command line (batch) mode.

## Reliability

Secondary, community-written; not read directly (status unread). The metadata values match the
parameter names and the inscape default in [[sources/fraktaler-3-site]] §Bailout Parameters, which
is the primary source. Verify before citing anything else from this page.

## Useful sections

- §(EXR metadata example) — exact heading not recorded.
- §(command line / exponential map) — exact heading not recorded.

## Used by

[[implementations/fraktaler-3]], [[techniques/interior-detection]], [[techniques/anti-aliasing]],
[[techniques/exponential-map]], [[techniques/auto-iterations]]
