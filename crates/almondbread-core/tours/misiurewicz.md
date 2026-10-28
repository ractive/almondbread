---
id: misiurewicz
title: Misiurewicz points
summary: Points whose z lands on a loop after a few steps: the tips and forks of the antennas, the centres of spirals, and why the picture repeats around them at every zoom.
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

Remember: every point of the picture is a number c, and z → z² + c is used again and again from z = 0. The **marker** (the small circle where the dots start) is at c = −2: the far left tip of the **spike**, the thin line that sticks out on the left of the set. In this view the spike runs from the marker to the right. The white dots are the values of z, one per step.

Here z goes 0, −2, **2, 2, 2, …**, because (−2)² − 2 = 2 and 2² − 2 = 2. After two steps z is 2, and it stays at 2. The white line from the marker to the right leads to the dot at 2, outside this view and outside the mini-map.

But z stays at 2 only if it is exactly 2. If z is a little away from 2, the next step moves it 4 times further away. So z does not escape, and it does not come closer and closer to a loop either: the line above the mini-map says *still undecided*. c = −2 lies exactly on the **edge** of the set.

## Pre-periodic
place: dendrite-i
probe: 0, 1

The marker is now at the tip of a thin branch, at the top right of this view. The mini-map shows where this is: straight above the **heart**, the big heart-shaped part of the set. This point is c = i, one unit straight up from 0.

Here z goes 0, i, −1 + i, −i, −1 + i, −i, … After two steps the dots land on a **loop** of two places, −1 + i and −i, and stay on it. On the mini-map, the white lines go from the marker left to −1 + i, then down to −i below the heart. The line above the mini-map says *still undecided* again: z does not escape and does not come closer to the loop. It lands exactly on it.

Points like this, whose z lands **exactly** on a loop after some steps but never comes back to 0, are called **Misiurewicz points** (after Michał Misiurewicz), or *pre-periodic* points. They all lie on the edge of the set: at tips, at forks and at the centres of spirals.

## A fork
place: three-way-junction
orbit: off
map: off
probe: pointer

The bulb on top of the heart has a thin line on top, its **antenna**. This view is far up the antenna, magnified 675 times. In the middle of this view, the antenna splits into three branches. This fork is a Misiurewicz point too: after **4** steps its z lands on one value and stays there.

This step shows no dots: ordinary computer numbers do not have enough digits to follow this point for long.

## A crossroads
place: antenna-junction
marker: -1.54368901269207636157085597180174798652520329765098394, 0
marker-label: −1.5437…

This view is on the spike. Side branches cross the spike in many places. The labelled point *−1.5437…* in the middle of the view is the point c = **−1.5437…** on the spike. It is another Misiurewicz point: after **3** steps its z lands on one value and stays there. Here z goes 0, −1.5437, 0.8393, −0.8393, −0.8393, …

## The same at every zoom
place: junction-1e300
marker: off

This is the three-way fork again, now magnified **10³⁰⁰** times (a 1 with 300 zeros). It still has three branches: it is the same fork.

Around a Misiurewicz point, the picture **repeats**. The loop that z lands on moves nearby values away by a fixed factor and turns them by a fixed angle. So if you zoom in by that factor and turn by that angle, you see the same picture again:

- at this fork: 1.33 times and a third of a turn (120°)
- around i: 5.66 times and an eighth of a turn (45°)
- at −2: 4 times, with no turn

## Spirals
place: two-arm-spiral
move: jump

At some Misiurewicz points the branches do not meet in a simple fork. They wind round and round the point and form a **spiral**.

This spiral is in seahorse valley. Its centre is in the middle of this view. The z of the centre lands, after 24 steps, on a loop of two places. So the centre is called M(24,2).

## Fifty digits deep
place: spiral-1e50

The same spiral, magnified **10⁵⁰** times: it still has two arms, and they still turn. Around these points, spirals and forks never end, however far you zoom in.

## Your turn
place: antenna-junction
move: jump
orbit: on
map: on

::: mac
The marker and its dots follow the pointer again.
:::
::: touch
To move the marker and its dots, hold a finger on the picture and choose *Orbit Here*.
:::

Move the marker to the tips and forks of the antenna. Where the dots land on a few places and do not come closer to them, you are near a Misiurewicz point.

**End tour** (or **Esc** twice) brings back your view and settings.
