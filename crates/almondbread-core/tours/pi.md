---
id: pi
title: π in the Mandelbrot set
summary: Count the steps of points near two pinch points, multiply by their distance, and π appears.
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

The round bulb on the left touches the big heart at a single point: the **neck** (marked). Points straight above it are outside the set, but only just.

Every point gets the same test: start at 0, square the number, add the point, repeat. Outside the set the numbers fly off in the end. Above the neck they first crawl for a long time through a narrow gap. Let's count their steps.

## Count and multiply
place: pi-at-the-neck
pi-meter: on
marker: off

The middle of the view sits **0.02** above the neck; call that distance **ε** (*epsilon*, the Greek letter for a small distance). Its numbers fly off after **158 steps**. Multiply: 158 × 0.02 = **3.16**. Almost **π = 3.14159…**, the way round any circle divided by its width!

The **π meter** (the panel) does the counting for you and lights the digits that already agree with π.

## Closer and closer
pi-hunt: neck

Now the counted point slides down the line towards the neck. Each time it gets ten times closer, its count gets ten times longer: 315 steps at ε = 0.01, 31 417 at 0.0001, 3 141 593 at 0.000001. And steps × ε gains **one more digit of π** each time.

At the end, ε = 0.00000001 and the point takes **314 159 266 steps**: a few seconds of counting. **E** and **Q** change the speed of the slide.

## Why π?

Near the neck the numbers hop from side to side and slowly **turn**. The circle in the upper panel shows it: from start to escape the numbers go round **once**, by a tiny angle of **2ε** at every step.

Measured in *radians*, the angle mathematicians use, one full turn is **2π**. So steps × 2ε ≈ 2π, which means steps × ε ≈ **π**, and the closer to the neck, the better it fits.

## The cusp
pi-hunt: cusp

There is a second spot: the dent at the right of the heart, the **cusp**. For points a distance ε to its right, the steps times **√ε** (the square root) close in on π.

The counted point slides down to ε = 0.0000000000000001, where the count is **314 159 263**. Here a hundred times closer gives one more digit.

Holly Krieger counts these steps at the cusp in Numberphile's [Pi and the Mandelbrot Set](https://www.youtube.com/watch?v=d0vY0CKYhPY) (with [extra footage](https://www.youtube.com/watch?v=r8Ksuc7T-VQ)).

## Your turn
place: pi-at-the-neck

The π meter stays on to the end of the tour. It counts the middle of the view whenever the view is centred on the line above the neck or right of the cusp: zoom in down the line yourself and watch the digits.

**End tour** (or **Esc** twice) switches it off and brings back your view and settings.
