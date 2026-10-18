---
id: pi
title: π in the Mandelbrot set
summary: Count the steps of points near two pinch points, multiply by their distance, and π appears.
place: pi-at-the-neck, pi-at-the-cusp
thumbnail: seahorse-valley
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

The round disc on the left touches the big heart at a single point, **c = −0.75**: the *neck*. Points straight above it are outside the set, but only just.

Their numbers do fly off in the end, but very slowly: first they crawl through a narrow gap. Let's count their steps.

## Count and multiply
place: pi-at-the-neck
pi-meter: on
marker: off

The middle of the view is **c = −0.75 + 0.02i**: a distance **ε = 0.02** above the neck. Its numbers escape after **158 steps**. Multiply: 158 × 0.02 = **3.16**. Almost **π = 3.14159…**!

The **π meter** (the panel) does the counting for you and lights the digits that already agree with π.

## Closer and closer
pi-hunt: neck

Now a probe slides down the line towards the neck. Each time it gets ten times closer, its count gets ten times longer: 315 steps at ε = 0.01, 31 417 at 0.0001, 3 141 593 at 0.000001. And steps × ε gains **one more digit of π** each time.

At the end, ε = 0.00000001 and the point takes **314 159 266 steps**: a few seconds of counting. **E** and **Q** change the speed of the slide.

## Why π?

Near the neck the numbers hop from side to side and slowly **turn**. The circle in the upper panel shows it: from start to escape the orbit goes round **once**, by a tiny angle of **2ε** at every step.

Measured in *radians*, the angle mathematicians use, one full turn is **2π**. So steps × 2ε ≈ 2π, which means steps × ε ≈ **π**, and the closer to the neck, the better it fits.

## The cusp
pi-hunt: cusp

There is a second spot: the dent at the right of the heart, **c = 0.25** (the *cusp*). For points a distance ε to its right, the steps times **√ε** (the square root) close in on π.

The probe slides down to ε = 0.0000000000000001, where the count is **314 159 263**. Here a hundred times closer gives one more digit.

## Your turn
place: pi-at-the-neck

The π meter stays on to the end of the tour. It counts the middle of the view whenever the view is centred on the line above the neck or right of the cusp: zoom in down the line yourself and watch the digits.

**End tour** (or **Esc**) switches it off and brings back your view and settings.
