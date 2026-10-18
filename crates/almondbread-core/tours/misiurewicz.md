---
id: misiurewicz
title: Misiurewicz points
summary: Points whose orbit lands on a loop after a few steps: the tips and forks of the antennas, the centres of spirals, and why the picture repeats around them at every zoom.
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

The ring is at **c = −2**, the far tip of the spike. Its numbers go 0, −2, **2, 2, 2, …**: after two steps they land on 2 and stay there for ever.

But 2 is a slippery place: a start next to it is pushed away 4 times further each step. So c = −2 neither escapes nor settles into a pull-in loop. The orbit overlay says *still undecided*: the point lies exactly on the **edge** of the set.

## Pre-periodic
place: dendrite-i
probe: 0, 1

**c = i**, the tip of a thin branch above the heart. Its numbers go 0, i, −1 + i, **−i, −1 + i, −i, …**: after two steps they land on a loop of two.

Points like these, whose orbit lands **exactly** on a loop after some steps but never comes back to 0, are called **Misiurewicz points** (after Michał Misiurewicz), or *pre-periodic*. They all lie on the edge, at tips, forks and centres of spirals.

## A fork
place: three-way-junction
orbit: off
map: off
probe: pointer

The antenna of the top bulb forks here into three. The fork is a Misiurewicz point too: after **4** steps its numbers land on a single number and stay. (Computer numbers are too short to follow it for long, so the orbit overlay is off here.)

## A crossroads
place: antenna-junction

On the spike, side twigs cross the antenna like masts. The biggest crossing, at **c = −1.5437…**, is another Misiurewicz point: after **3** steps its numbers land on one number.

## The same at every zoom
place: junction-1e300

Back to the fork, now magnified **10³⁰⁰** times. Still three branches, the same fork.

Around a Misiurewicz point the picture **repeats**: the loop it lands on pushes neighbours away by a fixed factor and turns them by a fixed angle. Zoom by that factor, turn by that angle, and you see the same picture again. At this fork the factor is 1.33 and the turn 120°; around c = i it is 5.66 and 45°; at −2 it is 4 with no turn.

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

Point the orbit at tips and forks of the antenna (on a touch screen: hold a finger and choose *Orbit Here*): where the dots land on a few spots and stay without settling, you are near a Misiurewicz point.

**End tour** (or **Esc**) brings back your view and settings.
