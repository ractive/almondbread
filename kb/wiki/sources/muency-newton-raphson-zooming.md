---
title: "Mu-Ency: Newton-Raphson zooming (embedded Julia set sizes)"
type: source
authors: ["Robert Munafo"]
year: 2023
raw: "web"
status: digested
tags: [embedded-julia-set, newton, minibrot, journey, muency]
---

# Mu-Ency: Newton-Raphson zooming

**URL:** <https://mrob.com/pub/muency/newtonraphsonzooming.html> (with the glossary entries
"Julia morphing" <https://mrob.com/pub/muency/juliamorphing.html> and "Embedded Julia set"
<https://mrob.com/pub/muency/embeddedjuliaset.html>) · **Not saved** (read 2026-10-15) ·
**Terms:** CC BY-NC 4.0 text; we use the facts only.

## What it is

Robert Munafo's glossary entry on zooming by Newton's method: find an island (minibrot)
nucleus to the working precision, estimate its size, and zoom straight to it, instead of
zooming by hand.

## What it covers

- The island nucleus found by Newton "is exact to within the precision being used"; the
  island's size is estimated next ([Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md) has our size formula).
- **Embedded Julia set sizes:** with the island's size `s` estimated, Claude Heiland-Allen
  found that its 2-fold embedded Julia set is about `s^(3/4)` in size, with the caveat that he
  was not sure it always holds. Munafo adds `s^(1/2)` for the 2-fold *second-order* set, and
  reads `s^(3/4)` as the 4-fold set met on the way to the island.
- "Julia morphing" (glossary): the colloquial name for tuning as it applies to shapes inside
  embedded Julia sets, and "the art of locating novel shapes through repeated zooming in to
  ever higher orders of embedding".

## Reliability

A careful amateur source; the size rule is flagged as an estimate by the source itself. The
two readings of `s^(3/4)` (2-fold vs 4-fold) are not reconciled on the page. We use the rule
only to propose framings that a probe render then accepts or rejects
([Controls: keyboard, pointer and touch](../decisions/controls.md) "Journey").

## Used by

[Controls: keyboard, pointer and touch](../decisions/controls.md) (Journey: embedded Julia stages), [Reference selection: nucleus, period detection, Newton](../techniques/reference-selection.md), [Finding good places at extreme depth (1e100 to 1e100000+)](../synthesis/finding-deep-places.md)
