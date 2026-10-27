---
id: steps
title: z² + c, step by step
summary: How the picture is made: the formula z → z² + c, worked out one step and one dot at a time.
next: colours
---

## A map of numbers
center: 0, 0
span: 2.8
look: aurora
interior: flat
orbit: off
map: off
marker: 0.2, 0.5
marker-label: c = 0.2 + 0.5i

This picture is a map of numbers. Every point of the picture is one number. We call that number **c**.

Each c has two parts. Both are distances from the middle of the picture. The middle is the number 0. The first part, the *real part*, says how far to the right (or left) the point is. The second part, the *imaginary part*, says how far up (or down) it is. The imaginary part is written with an i after it, like 0.5i.

The letter i stands for a special number: i × i = −1. No ordinary number does that. So i does not fit on the left–right line; it gets its own direction on the picture: up.

The box at the top left shows the middle of the picture as *re 0* and *im 0*; its other lines are about zooming.

The small gold circle is the **marker**. It sits on c = 0.2 + 0.5i: 0.2 to the right of the middle and 0.5 up. For the scale: the thin tip of the black shape at the far left is at −2, two to the left of the middle. So 0.2 is a tenth of that distance.

## The formula

The black shape is the **Mandelbrot set**. A set is a group of things; this one is a group of points. To find out if a point c is in the set, we use one formula, again and again:

**z → z² + c**

The arrow means "becomes": take the number z, square it and add c. The result is the new z. At the start, z = 0. Each use of the formula is one **step**. In every step, c stays the same; only z changes.

Then we watch z. If z stays small, c is in the set. If z grows bigger and bigger, c is not in the set. We stop when z is far from 0 (card 7 gives the exact rule), or after many steps.

The app does this for every point of the picture. The next cards do it by hand for the c under the marker.

## Step 1
center: 0, 0.4
span: 1.2
orbit: on
orbit-length: 1
probe: 0.2, 0.5

The view now zooms in, closer to the marker.

Step 1 starts with z = 0:

z = 0² + c = c = 0.2 + 0.5i

So after step 1, z is c itself. z is a number with two parts too, so it has a place on the same picture. A white dot shows the new z, with its step number next to it: **dot 1**. It sits exactly on the marker. These white **dots** show the values of z, one dot per step.

The small map at the bottom left, the **mini-map**, shows where in the whole set we are. You can ignore its text line for now.

## Step 2
orbit-length: 2

Step 2 squares z = 0.2 + 0.5i. To square a number with two parts, use (a + b)² = a² + 2ab + b². Here a = 0.2 and b = 0.5i:

0.2² + 2 × 0.2 × 0.5i + (0.5i)² = 0.04 + 0.2i − 0.25 = −0.21 + 0.2i

The last part is negative because (0.5i)² = 0.25 × i × i = 0.25 × (−1) = −0.25.

Then add c, real part to real part, imaginary part to imaginary part: z = −0.21 + 0.2i + 0.2 + 0.5i = **−0.01 + 0.7i**. This is **dot 2**, up and to the left of dot 1 (0.21 to the left, 0.2 up).

A line joins dot 1 to dot 2. It only shows the order of the dots: z jumps from dot 1 to dot 2; it does not travel along the line.

## Steps 3 and 4
orbit-length: 4

Step 3 squares z = −0.01 + 0.7i in the same way:

(−0.01)² + 2 × (−0.01) × 0.7i + (0.7i)² = 0.0001 − 0.014i − 0.49 = −0.4899 − 0.014i

Add c: z = **−0.2899 + 0.486i**. This is **dot 3**, on the left.

Step 4 squares this, rounded to −0.29 + 0.486i, the same way and adds c. The result is 0.0479 + 0.2181i, about **0.05 + 0.22i**. This is **dot 4**, at the bottom.

The dots go round in this order: right, top, left, bottom. That is against the direction of a clock's hands.

## In the set
orbit-length: 50

Here are the first 50 dots. They go round and round, and each turn is a little smaller. They come closer and closer to one place, about −0.03 + 0.48i, in the middle of the spiral.

The app follows z for many more steps than these 50, and z never gets further than 2 from 0 (2 is the distance from 0 to the tip at −2 on the first card). So z stays small, and c = 0.2 + 0.5i is **in the set**.

Every c whose z stays small is in the set, and we paint it black. That is why the marker sits on a black point, close to the edge of the shape.

## Escaping
center: -0.1, 0.65
span: 3.2
orbit-length: 12
probe: 0.2, 0.65
marker: 0.2, 0.65
marker-label: c = 0.2 + 0.65i

The view now zooms out. The marker sits on a new c, a little higher up: c = 0.2 + 0.65i. The edge of the black shape runs between the two c's: the first one was just inside it, this one is just outside.

Dots 1 to 5 stay in or near the black shape. Dots 6 and 7 are further out. Dot 8, at the top right, is about 1.08 + 1.92i. Its distance from 0 is √(1.08² + 1.92²) = √(1.17 + 3.69) = √4.86, about 2.2. That is further than 2: more than from 0 to the tip at −2, on the left of the picture. Dot 9, at about −2.33 + 4.81i, is off the picture.

Once z is further than 2 from 0, its distance from 0 grows at every step, and it never comes back. We say that z **escapes**, and c is not in the set.

The line above the mini-map says *escapes after 12 steps*; the next card explains the 12.

## Where the colours come from
center: -0.5, 0
span: 2.8

The view now shows the whole set again. The marker and its dots are the same as on the last card.

Every c outside the set escapes, but some escape after a few steps and others after many. The colour of a point shows how many steps its z needs to get far from 0. In this picture, the dark blue far from the black shape means few steps. Closer to the shape the colours get lighter, and right at its edge, where z needs many steps, they are pale yellow and pink.

The line above the mini-map says *escapes after 12 steps* for the marker. That is more than 8. Dot 8 was the first dot further than 2 from 0, but the app keeps counting a little longer, until z is very far from 0. The extra steps give smoother colours.

The tour *What the colours mean* explains more.

## Your turn
probe: pointer
marker: off
orbit-length: 50

::: touch
Hold a finger still on the picture for a moment, then move it without lifting. The marker is now a small white circle. It follows your finger, and its dots come with it.
:::
::: mac
Move the pointer over the picture; no click is needed. The marker is now a small white circle. It follows the pointer, and its dots come with it.
:::

Move the marker into the black shape: its dots stay on the picture, close together or going round a few places. Move it outside: after a few steps or after many, its dots leave the picture, because z escapes.

::: touch
**End tour** and **Finish** do the same: they end the tour and bring back the view and colours you had before the tour.
:::
::: mac
**End tour** and **Finish** (or **Esc** twice) do the same: they end the tour and bring back the view and colours you had before the tour.
:::
