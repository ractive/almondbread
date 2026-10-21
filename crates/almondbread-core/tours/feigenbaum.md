---
id: feigenbaum
title: The Feigenbaum point
summary: Period doubling into chaos along the spike: watch one loop split into 2, 4, 8, … and then smear.
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

Every point of the picture gets the same test, the **rule**: start at 0, square the number, add the point, and repeat. Does the number stay small, or fly off?

The ringed point is the one being tested; each dot is the number one more step makes. They hop from side to side and close in on **one spot** and stay: a *fixed point*. Every point in the big **heart** settles like this. Inside the set, each colour stands for how many spots the dots end up hopping round.

## Two points
probe: -1.1, 0

Points on the middle line have a number for a name: 0 inside the heart, −2 at the far left tip. The ring is now at −1.1, past the **neck** at −0.75 where the heart touches the round **bulb** on its left.

The one spot no longer pulls the dots in. Instead they hop between **two spots**, −1.09 and 0.09, for ever: a loop of 2, a *period-2 cycle*. The whole bulb, from −0.75 to −1.25, behaves like this.

## Watch it split
probe-path: -0.7, 0 -> -1.0, 0 -> -1.25, 0 -> -1.33, 0 -> -1.37, 0 -> -1.39, 0 -> -1.40, 0 -> -1.45, 0
probe-seconds: 24

Now the ring slides along the middle line from −0.7 to −1.45 and back. Watch the dots: **one spot, then 2, then 4, then 8.** Each time a loop stops pulling the dots in, it splits into two: *period doubling*.

The splits come faster and faster: at −0.75, −1.25, −1.368, −1.394, −1.3996, … and at the far end the dots no longer settle at all.

## Period 4
center: -1.31, 0
span: 0.3
probe: -1.3107026413368328, 0

The next bulb along has period 4. The ring is at its middle, −1.3107: the dots hop round **four spots**, −1.31, 0.41, −1.14 and 0, then start again.

This bulb runs from −1.25 to −1.368: 0.118 long, **4.2 times shorter** than the period-2 bulb (0.5).

## Ever smaller
center: -1.388, 0
span: 0.03
probe: -1.3815474844320617, 0

Period 8 sits from −1.368 to −1.394 (0.026 long), period 16 up to −1.3996 (0.0056), then 32, 64, …

Each bulb is shorter than the one before by **4.23, 4.55, 4.65, …**, and the ratio tends to **4.669…**, *Feigenbaum's constant*. The ring is at the middle of the period-8 bulb: its dots hop round 8 spots.

## The Feigenbaum point
center: -1.4011551890920506, 0
span: 0.002
probe: -1.4011551890920506, 0
marker: -1.4011551890920506, 0
marker-label: Feigenbaum point

All the doublings end here, at **−1.40115518909…**, the *Feigenbaum point*. The bulbs of period 2, 4, 8, 16, … shrink by about 4.669 each and pile up against it.

Here the dots never settle into a loop and never fly off. The same 4.669 turns up in dripping taps, electronic circuits and animal populations: wherever things double their way into chaos.

## Chaos
center: -0.4, 0
span: 2.4
probe-path: -1.4012, 0 -> -1.43, 0 -> -1.46, 0
probe-seconds: 16
marker: off

Past the Feigenbaum point, most points on the **spike**, the thin line to the left, give **chaos**: the dots never repeat but fill a whole band (see the cloud on the map), and two points a hair apart soon go completely different ways.

Now and then a short burst of order flashes by: tiny windows where the dots settle into a loop of 12, 10 or 14 spots.

## Order returns
center: -1.768, 0
span: 0.05
probe: -1.7548776662466927, 0

At **−1.75** order comes back. A **minibrot**, a tiny copy of the whole set, sits on the spike. The ring is in its heart, and the dots hop round **three spots**: −1.755, 0 and 1.325.

And the story repeats: to the left of this copy's heart, its own bulbs double the loop again, 6, 12, 24, …, shrinking by the same 4.669.

## Your turn
center: -1.25, 0
span: 1.2
probe: pointer

The ring and its dots follow the pointer again (on a touch screen: hold a finger on the picture and drag). Move it along the spike and through the bulbs, and watch the cycles split and smear.

**Back** and **Next** (or **←** and **→**) step through the tour; **End tour** (or **Esc** twice) ends it and brings back your view and settings.
