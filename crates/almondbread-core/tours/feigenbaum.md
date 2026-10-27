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

The point being tested is marked with a small circle: the **test point**. Each dot shows the number that one more step makes. Here the dots hop from side to side, close in on **one spot** and stay there: a *fixed point*. Every point in the big heart-shaped body of the set, the **heart**, settles like this. Inside the set, each colour stands for how many spots the dots end up hopping round.

## Two points
probe: -1.1, 0

The middle line of the picture holds the ordinary numbers: 0 is inside the heart, −2 is the far left tip. The test point is now at −1.1. It has passed the **neck** at −0.75, the narrow place where the heart touches the round **bulb** on its left.

Here the dots no longer close in on one spot. Instead they end up hopping between **two spots**, −1.09 and 0.09, for ever: a loop of 2, a *period-2 cycle*. The whole bulb, from −0.75 to −1.25, behaves like this.

## Watch it split
probe-path: -0.7, 0 -> -1.0, 0 -> -1.25, 0 -> -1.33, 0 -> -1.37, 0 -> -1.39, 0 -> -1.40, 0 -> -1.45, 0
probe-seconds: 24

Now the test point slides along the middle line from −0.7 to −1.45 and back. Watch the dots: **one spot, then 2, then 4, then 8.** Each time a loop can no longer hold the dots, it splits into two: *period doubling*.

The splits come faster and faster: at −0.75, −1.25, −1.368, −1.394, −1.3996, … At the far end the dots no longer settle at all.

## Period 4
center: -1.31, 0
span: 0.3
probe: -1.3107026413368328, 0

The next bulb along has period 4. The test point is at its middle, −1.3107. Its dots hop round **four spots**, −1.31, 0.41, −1.14 and 0, then start again. This close in, most of them are off the picture: the mini-map shows all four.

This bulb runs from −1.25 to −1.368: 0.118 long, **4.2 times shorter** than the period-2 bulb (0.5).

## Ever smaller
center: -1.388, 0
span: 0.03
probe: -1.3815474844320617, 0

Period 8 sits from −1.368 to −1.394 (0.026 long), period 16 up to −1.3996 (0.0056), then 32, 64, …

Each bulb is **4.23, 4.55, 4.65, …** times shorter than the one before, and these ratios get closer and closer to **4.669…**, *Feigenbaum's constant*. The test point is at the middle of the period-8 bulb: its dots hop round 8 spots (see the mini-map).

## The Feigenbaum point
center: -1.4011551890920506, 0
span: 0.002
probe: -1.4011551890920506, 0
marker: -1.4011551890920506, 0
marker-label: Feigenbaum point

Each bulb is about 4.669 times shorter than the one before, so the bulbs of period 2, 4, 8, 16, … crowd closer and closer together. They end here, at **−1.40115518909…**, the *Feigenbaum point*. It is named after Mitchell Feigenbaum, like the constant 4.669, but it is a different number.

Here the dots never settle into a loop and never fly off. The same 4.669 turns up in dripping taps, electronic circuits and animal populations: wherever things double their way into chaos. Ben Sparks finds it in a population model and shows that its picture is this middle line of the Mandelbrot set, in Numberphile's [The Feigenbaum Constant (4.669)](https://www.youtube.com/watch?v=ETrYE4MdoLQ).

## Chaos
center: -0.4, 0
span: 2.4
probe-path: -1.4012, 0 -> -1.43, 0 -> -1.46, 0
probe-seconds: 16
marker: off

Past the Feigenbaum point, most points on the **spike**, the thin line to the left, give **chaos**: the dots never repeat but fill a whole band (see the cloud on the mini-map). Move the test point by a hair, and after a few dozen steps its dots are somewhere completely different.

Now and then a short burst of order flashes by: tiny windows where the dots settle into a loop of 12, 10 or 14 spots. Some points keep their secret. Holly Krieger says that nobody knows whether the dots of −1.5 settle into a loop in the end. Whether the points of every region inside the set settle into a loop is an open question too (Numberphile: [The Mystery of Hyperbolicity](https://www.youtube.com/watch?v=eC9-1wY7FvU)).

## Order returns
center: -1.768, 0
span: 0.05
probe: -1.7548776662466927, 0

At **−1.75** order comes back. A **minibrot**, a tiny copy of the whole set, sits on the spike. The test point is in its heart, and the dots hop round **three spots**: −1.755, 0 and 1.325.

And the story repeats: to the left of this copy's heart, its own bulbs double the loop again, 6, 12, 24, …, each again about 4.669 times shorter than the one before.

Number theorists know this point too: −1.75 is the fraction −7/4. Apply the rule with it, starting at 0: 0, −7/4, 21/16, −7/256, … Look at the tops of the fractions: 7, then 21 = 3 × 7, then 7 again. Usually every step brings a prime factor that has not turned up before (21 brings the 3), but the third step here brings none. No other fraction is known to do this, and any other would have to lie deep inside the set (Numberphile: [63 and −7/4 are special](https://www.youtube.com/watch?v=09JslnY7W_k&t=570s), from 9:30).

## Your turn
center: -1.25, 0
span: 1.2
probe: pointer

The test point and its dots follow the pointer again (on a touch screen: hold a finger on the picture and drag). Move it along the spike and through the bulbs, and watch the cycles split and smear.

**Back** and **Next** (or **←** and **→**) step through the tour; **End tour** (or **Esc** twice) ends it and brings back your view and settings.
