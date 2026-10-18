---
id: feigenbaum
title: The Feigenbaum point
summary: Period doubling into chaos along the spike: watch one orbit split into 2, 4, 8, … and then smear.
place: feigenbaum-point
---

## Square and add
center: -0.6, 0
span: 2.6
look: aurora
interior: period
orbit: on
orbit-length: 120
probe: -0.2, 0
marker: off

Every point c gets the same test: start at 0, square, add c, and repeat. Does the number stay small, or run away?

The ringed dot is **c = −0.2**; the other dots are the steps. They hop from side to side and close in on **one point, −0.17**: a *fixed point*. Every c in the big heart-shaped region (the main cardioid) settles like this. Inside the set, each colour stands for how many points the orbit ends up visiting.

## Two points
probe: -1.1, 0

Move c left past **−0.75**, into the round bulb. The fixed point no longer pulls the orbit in. Instead it swings between **two points, −1.09 and 0.09**, for ever: a *period-2 cycle*.

The whole bulb, from −0.75 to −1.25, behaves like this.

## Watch it split
probe-path: -0.7, 0 -> -1.0, 0 -> -1.25, 0 -> -1.33, 0 -> -1.37, 0 -> -1.39, 0 -> -1.40, 0 -> -1.45, 0
probe-seconds: 24

Now c slides along the real axis from −0.7 to −1.45 and back. Watch the dots: **one point, then 2, then 4, then 8.** Each time a cycle stops attracting, it splits into two: *period doubling*.

The splits come faster and faster: at c = −0.75, −1.25, −1.368, −1.394, −1.3996, … and at the far end the dots no longer settle at all.

## Period 4
center: -1.31, 0
span: 0.3
probe: -1.3107026413368328, 0

The next bulb on the spike has period 4. At its centre, c = −1.3107, the orbit visits **−1.31, 0.41, −1.14 and 0**, then starts again.

This bulb runs from −1.25 to −1.368: 0.118 long, **4.2 times shorter** than the period-2 bulb (0.5).

## Ever smaller
center: -1.388, 0
span: 0.03
probe: -1.3815474844320617, 0

Period 8 sits from −1.368 to −1.394 (0.026 long), period 16 up to −1.3996 (0.0056), then 32, 64, …

Each bulb is shorter than the one before by **4.23, 4.55, 4.65, …**, and the ratio tends to **4.669…**, *Feigenbaum's constant*. The orbit of this c, the centre of the period-8 bulb, visits 8 points.

## The Feigenbaum point
center: -1.4011551890920506, 0
span: 0.002
probe: -1.4011551890920506, 0
marker: -1.4011551890920506, 0
marker-label: Feigenbaum point

All the doublings end here, at **c = −1.40115518909…**, the *Feigenbaum point*. The bulbs of period 2, 4, 8, 16, … shrink by about 4.669 each and pile up against it.

Here the orbit never settles into a cycle and never escapes. The same 4.669 turns up in dripping taps, electronic circuits and animal populations: wherever things double their way into chaos.

## Chaos
center: -0.4, 0
span: 2.4
probe-path: -1.4012, 0 -> -1.43, 0 -> -1.46, 0
probe-seconds: 16
marker: off

Past the Feigenbaum point, most c on the spike give **chaos**. The orbit never repeats: its dots fill a whole band (see the cloud on the map), and two starts a hair apart soon end up far from each other.

Now and then a short burst of order flashes by: tiny windows where the orbit settles into a cycle of 12, 10 or 14 points.

## Order returns
center: -1.768, 0
span: 0.05
probe: -1.7548776662466927, 0

At **−1.75** order comes back. A tiny copy of the whole set sits on the spike, and for c in its heart the orbit settles into a **period-3 cycle**: here −1.755, 0 and 1.325.

And the story repeats: to the left of this copy's heart, its own bulbs double the period again, 6, 12, 24, …, shrinking by the same 4.669.

## Your turn
center: -1.25, 0
span: 1.2
probe: pointer

The orbit follows the pointer again (on a touch screen: hold a finger on the picture and drag). Move it along the spike and through the bulbs, and watch the cycles split and smear.

**Back** and **Next** (or **←** and **→**) step through the tour; **End tour** (or **Esc**) ends it and brings back your view and settings.
