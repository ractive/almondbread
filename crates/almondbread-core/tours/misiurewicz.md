---
id: misiurewicz
title: Misiurewicz points
summary: Points whose numbers land on a loop after a few steps: the tips and forks of the antennas, the centres of spirals, and why the picture repeats around them at every zoom.
place: antenna-junction
---

## Landing on a loop
center: -1.85, 0
span: 0.4
look: aurora
interior: flat
orbit: on
orbit-length: 20
map: on
probe: -2, 0
marker: off

Every point gets the same test, the **rule**: start at 0, square the number, add the point, repeat. The ringed point is being tested; each dot is the number one more step makes.

The ring is at −2, the far tip of the **spike**. Its numbers go 0, −2, **2, 2, 2, …**: after two steps the dots land on 2 and stay there (see the map).

But 2 is a slippery spot: a number next to it gets pushed 4 times further away each step. So −2 neither flies off nor gets pulled into a loop. The line under the map says *still undecided*: the point lies exactly on the **edge** of the set.

## Pre-periodic
place: dendrite-i
probe: 0, 1

The ring is now at the tip of a thin branch straight above the heart (mathematicians call this point *i*). After two steps its dots land on a **loop** of two spots and hop between them for ever (see the map).

Points like these, whose numbers land **exactly** on a loop after some steps but never come back to 0, are called **Misiurewicz points** (after Michał Misiurewicz), or *pre-periodic*. They all lie on the edge, at tips, forks and centres of spirals.

## A fork
place: three-way-junction
orbit: off
map: off
probe: pointer

The **antenna** of the big top bulb, its thin hair, forks here into three. The fork is a Misiurewicz point too: after **4** steps its numbers land on a single number and stay. (No dots here: computer numbers are too short to follow this point for long.)

## A crossroads
place: antenna-junction

On the spike, side twigs cross the antenna like masts. The biggest crossing, at **−1.5437…**, is another Misiurewicz point: after **3** steps its numbers land on one number.

## The same at every zoom
place: junction-1e300

Back to the fork, now magnified **10³⁰⁰** times (a 1 with 300 zeros). Still three branches, the same fork.

Around a Misiurewicz point the picture **repeats**: the loop it lands on pushes neighbours away by a fixed amount and turns them by a fixed angle. Zoom in by that amount, turn by that angle, and you see the same picture again. At this fork: 1.33 times and a third of a turn (120°); around *i*: 5.66 times and an eighth of a turn (45°); at −2: 4 times, no turn.

## Spirals
place: two-arm-spiral
move: jump

When each zoom step turns the arms by an angle that does not bring them back onto themselves, they wind round and round the point: a **spiral**. This one in seahorse valley lands, after 24 steps, on a loop of two: it is called M(24,2).

## Fifty digits deep
place: spiral-1e50

The same spiral magnified **10⁵⁰** times: still two arms, still turning. Spirals and forks never end, however deep you zoom around these points.

## Your turn
place: antenna-junction
move: jump
orbit: on
map: on

The ring follows the pointer again (on a touch screen: hold a finger and choose *Orbit Here*). Point it at tips and forks of the antenna: where the dots land on a few spots without being pulled in, you are near a Misiurewicz point.

**End tour** (or **Esc** twice) brings back your view and settings.
