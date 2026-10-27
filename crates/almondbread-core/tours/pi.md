---
id: pi
title: π in the Mandelbrot set
summary: Count the steps of points near two narrow places of the set, multiply by their distance, and π appears.
place: pi-at-the-neck, pi-at-the-cusp
thumbnail: head-and-shoulders
---

## The neck
center: -0.7, 0.05
span: 1.1
look: aurora
interior: flat
orbit: off
map: off
pi-meter: off
marker: -0.75, 0
marker-label: the neck

In this view the set is black. The round bulb on the left touches the heart, the big black shape on the right, at a single point: the **neck**, the labelled point at −0.75. The points straight above the neck, in the narrow coloured gap, are outside the set, but only just.

A reminder of the formula: z starts at 0, and at each step z → z² + c. Outside the set, z escapes in the end: it keeps growing. For points just above the neck, this takes a very long time: z first moves slowly through a narrow passage. In this tour we count the steps until z escapes.

## Count and multiply
place: pi-at-the-neck
pi-meter: on
marker: off

The small circle labelled −3/4 + iε is **0.02** straight above the neck (labelled −3/4). We call this distance **ε** (*epsilon*, a Greek letter for small distances).

For this point, z escapes after **158 steps**, and 158 × 0.02 = **3.16**: almost **π = 3.14159…**, the distance round any circle divided by its width!

The **π meter**, the lower panel on the right, counts for you. Digits that agree with π are gold.

## Closer and closer
pi-hunt: neck

Now the counted point, the glowing circle labelled "probe", slides down the vertical ruler towards the neck. Each time it gets ten times closer, it needs ten times more steps: 315 steps at ε = 0.01, 31 417 at 0.0001, 3 141 593 at 0.000001. And each time, steps × ε has **one more correct digit of π**.

At the end, ε = 0.00000001, and z takes **314 159 266 steps** to escape. The counting takes a few seconds.

::: mac
**E** and **Q** change the speed of the slide.
:::

## Why π?

Near the neck, z jumps from one side of −1/2 to the other at every step, and at the same time it slowly **turns**. The circle in the upper panel, on the right, shows this turning. From the start until z escapes, z goes round **once**, by a tiny angle of **2ε** at every step.

Mathematicians measure angles in *radians*. In radians, one full turn is **2π**. So steps × 2ε ≈ 2π, and that means steps × ε ≈ **π**. The closer the point is to the neck, the better this fits.

## The cusp
pi-hunt: cusp

The **cusp**, the dent on the right of the heart (labelled 1/4), works too. For a point ε to its right, steps × **√ε** (the square root of ε) comes closer and closer to π.

The glowing circle labelled "probe" slides to ε = 0.0000000000000001, where z takes **314 159 263** steps. Each new digit needs a point a hundred times closer.

Holly Krieger counts these steps at the cusp in Numberphile's [Pi and the Mandelbrot Set](https://www.youtube.com/watch?v=d0vY0CKYhPY) (with [extra footage](https://www.youtube.com/watch?v=r8Ksuc7T-VQ)).

## Your turn
place: pi-at-the-neck

The π meter stays on until the end of the tour. It counts the point in the middle of the view whenever the view is centred on the vertical line above the neck, or on the horizontal line to the right of the cusp. Zoom in along the line yourself and watch the digits.

::: mac
**End tour** (or **Esc** twice) switches the π meter off and brings back your view and settings.
:::
::: touch
**End tour** switches the π meter off and brings back your view and settings.
:::
