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
marker: off
labels: 0, 0, 0; -2, 0, −2

This picture is a map of numbers. Every point of the picture is one number. We call that number **c**.

Each c has two parts. The first part, the *real part*, says how far the point is to the right of the middle (a minus sign means to the left). The second part, the *imaginary part*, says how far it is above the middle (a minus sign means below). The imaginary part is written with an i after it, like 0.5i.

The middle of the picture is the number 0, the gold dot labelled 0. For the scale: the far left end of the thin line is −2, two to the left of 0 (labelled −2).

The box at the top left also shows the middle of the picture, as *re 0* and *im 0*; its other lines are about zooming.

## The marker
marker: 0.2, 0.5
marker-label: c = 0.2 + 0.5i
labels: 0, 0, 0; -2, 0, −2

The letter i stands for a special number: i × i = −1. No ordinary number does that, so take it as a rule of this game. Because i does not fit on the left–right line, it gets its own direction on the picture: up.

The small gold circle is the **marker**. It sits on c = 0.2 + 0.5i: 0.2 to the right of 0 and 0.5 up. 0.2 is a tenth of the distance from 0 to −2.

## The formula
labels: 0, 0, 0; -2, 0, −2

The black shape is the **Mandelbrot set**. A set is a group of things; this one is a group of points. To find out if a point c is in the set, we use one formula, again and again:

**z → z² + c**

The arrow means "becomes": take the number z, square it and add c. The result is the new z. At the start, z = 0. Each use of the formula is one **step**. In every step, c stays the same; only z changes.

Then we watch z. If z stays small, c is in the set. If z grows bigger and bigger, c is not in the set. We stop when z is far from 0 (the card *Escaping* gives the exact rule), or after many steps.

The app does this for every point of the picture. The next cards do it by hand for the c under the marker.

## Step 1
center: 0, 0.4
span: 1.2
orbit: on
orbit-length: 1
probe: 0.2, 0.5
labels: 0, 0, 0

The view now zooms in, closer to the marker. The gold dot at the bottom is still 0.

Step 1 starts with z = 0:

z = 0² + c = c = 0.2 + 0.5i

So after step 1, z is c itself. z is a number with two parts too, so it has a place on the same picture. A white dot shows the new z, with its step number next to it: **dot 1**. It sits exactly on the marker. These white **dots** show the values of z, one dot per step.

The small map at the bottom left, the **mini-map**, shows where in the whole set we are.

## Step 2
orbit-length: 2
labels: 0, 0, 0

Step 2 squares z = 0.2 + 0.5i. To square a number with two parts, use (a + b)² = a² + 2ab + b². Here a = 0.2 and b = 0.5i, and (0.5i)² = 0.25 × i × i = −0.25:

0.2² + 2 × 0.2 × 0.5i + (0.5i)² = 0.04 + 0.2i − 0.25

0.04 − 0.25 + 0.2i = −0.21 + 0.2i

Then add c, real part to real part, imaginary part to imaginary part: z = −0.21 + 0.2i + 0.2 + 0.5i = **−0.01 + 0.7i**. This is **dot 2**, up and to the left of dot 1 (0.21 to the left, 0.2 up). A line joins the two dots; it only shows their order.

Dot 2 lies on the coloured edge, outside the black shape. That is fine. The black or colour of a point tells us about that point as c. The white dots are values of z, and they can land on any point, black or coloured.

## Steps 3 and 4
orbit-length: 4
labels: 0, 0, 0

Step 3 squares z = −0.01 + 0.7i in the same way:

(−0.01)² + 2 × (−0.01) × 0.7i + (0.7i)² = 0.0001 − 0.014i − 0.49 = −0.4899 − 0.014i

Add c: z = **−0.2899 + 0.486i**. This is **dot 3**, on the left.

Step 4 squares this, rounded to −0.29 + 0.486i to keep the numbers short:

(−0.29)² + 2 × (−0.29) × 0.486i + (0.486i)² = 0.0841 − 0.2819i − 0.2362 = −0.1521 − 0.2819i

Add c: 0.0479 + 0.2181i, about **0.05 + 0.22i**. This is **dot 4**, below the others.

The dots go round in this order: right, top, left, bottom. That is counter-clockwise, against the direction of a clock's hands.

## Round and round
orbit-length: 50
labels: 0, 0, 0

Here are the first 50 dots. They go round and round, and each turn is a little smaller. They come closer and closer to one place, about −0.03 + 0.48i, in the middle of the spiral. To be sure, the app follows z for many more steps than these 50.

All the dots together are called the **orbit** of c. The one place they come closer to is called a *fixed point*. The line above the mini-map uses these words: *following the orbit* while the app is still working, then *settles on a fixed point*.

## In the set
center: 0, 0
span: 4.6
orbit-length: 50
circle: 0, 0, 2, distance 2 from 0
labels: 0, 0, 0; -2, 0, −2

The view now zooms out. The thin gold circle goes round 0 at a distance of 2: every point on it is 2 from 0. It passes through −2, the far left end of the thin line.

Why 2? Mathematicians have proved it: once z is further than 2 from 0, it keeps growing and never comes back. So the circle separates "small" from "escaping".

All the dots stay well inside the circle: in all the steps the app checked, z never got further than 2 from 0. So c = 0.2 + 0.5i is **in the set**, and we paint it black. That is why the marker sits on a black point, close to the edge of the shape. The circle is for the dots: the coloured points inside it are c's whose own z escapes.

## Escaping
center: 0, 0.3
span: 4.6
orbit-length: 12
probe: 0.2, 0.65
marker: 0.2, 0.65
marker-label: c = 0.2 + 0.65i
circle: 0, 0, 2, distance 2 from 0
labels: 0, 0, 0

The view moves up a little. The marker sits on a new c, a little higher up: c = 0.2 + 0.65i. The first c was just inside the black shape; this c is just outside it.

Dots 1 to 7 stay inside the gold circle. Dot 8, at the top right, is just outside the circle: z is about 1.08 + 1.92i. Its distance from 0 comes from Pythagoras' theorem: √(1.08² + 1.92²) = √(1.17 + 3.69) = √4.86, about 2.2. That is further than 2. Dot 9, at about −2.33 + 4.81i, is off the picture.

Once z is further than 2 from 0, its distance from 0 grows at every step and never comes back. We say that z **escapes**, and c is not in the set.

The line above the mini-map says *escapes after 12 steps*; the next card explains the 12.

## Where the colours come from
center: -0.5, 0
span: 2.8

The view now shows the whole set again. The marker and its dots are the same as on the last card; some of the dots are now off the picture.

Every c outside the set escapes, but some escape after a few steps and others after many. The colour of a point shows how many steps its z needs to get far from 0. In this picture, the dark blue far from the black shape means few steps. Closer to the shape the colours get lighter, and right at its edge, where z needs many steps, they are pale yellow and pink.

The line above the mini-map says *escapes after 12 steps* for the marker. That is more than the 8 steps of dot 8, because the app keeps counting until z is very far from 0. This lets it give each point a count with decimals, so the colours change gradually from point to point instead of in sudden bands.

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

Move the marker into the black shape: its dots stay on the picture. They come closer to one place, or they keep jumping between a few places. Move it outside: after a few steps or after many, its dots leave the picture, because z escapes.

::: touch
**End tour** and **Finish** do the same: they end the tour and bring back the view and colours you had before the tour.
:::
::: mac
**End tour** and **Finish** (or **Esc** twice) do the same: they end the tour and bring back the view and colours you had before the tour.
:::
