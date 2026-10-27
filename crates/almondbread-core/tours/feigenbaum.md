---
id: feigenbaum
title: The Feigenbaum point
summary: Period doubling into chaos along the real line: watch one loop split into 2, 4, 8, … places and then turn into chaos.
place: feigenbaum-point
---

## One place
center: -0.6, 0
span: 2.6
look: aurora
interior: period
orbit: on
orbit-length: 120
probe: -0.2, 0
marker: off

A reminder of the formula: z starts at 0, and at each step z → z² + c (square z, then add c). The small circle (the marker) is the point c = −0.2. The white dots are the values of z, one per step.

With c = −0.2, z goes 0, −0.2, −0.16, −0.174, −0.170, … The values go a little to one side, then a little to the other, but they come closer and closer to one place, −0.171, and then stay there. That is why the white dots make one small group right next to the marker. This place is called a *fixed point*. Every c in the heart, the big heart-shaped part of the set, behaves like this.

Inside the set, the colour shows how many places the dots end up moving round. The heart is light orange: one place. The big round bulb on its left is blue-green: two places.

## Two places
probe: -1.1, 0

The horizontal line through the middle of the set is the real line: it holds the ordinary numbers. 0 is inside the heart, and −2 is the tip on the far left. The marker is now at c = −1.1. It has passed the **neck** at −0.75, the narrow place where the heart touches the big round bulb on its left.

Here the dots no longer come closer to one place. They end up moving between **two places**: −1.09, right next to the marker, and 0.09, inside the heart on the right. A white line joins the two. z goes …, −1.09, 0.09, −1.09, 0.09, … again and again. This is a loop of 2, a *period-2 cycle*. Every c in this bulb, from −0.75 to −1.25, behaves like this.

## Watch it split
probe-path: -0.7, 0 -> -1.0, 0 -> -1.25, 0 -> -1.33, 0 -> -1.37, 0 -> -1.39, 0 -> -1.40, 0 -> -1.45, 0
probe-seconds: 24

Now the marker moves slowly along the real line, from −0.7 to −1.45 and back. Watch the white dots on the real line: first they go to **one place**, then they move between **2 places**, then **4**, then **8**. Each time, every place splits into two places close together, so the loop becomes twice as long. This is called *period doubling*.

The splits come faster and faster: at −0.75, −1.25, −1.368, −1.394, −1.3996, … Near −1.45, at the far end of the path, the dots do not settle into a loop at all.

## Period 4
center: -1.31, 0
span: 0.3
probe: -1.3107026413368328, 0

The light green circle in the middle of the view is the next bulb to the left. It has period 4. The marker is at its middle, c = −1.3107. Starting from 0, z goes −1.31, 0.41, −1.14, 0, and then the same four values again. So the dots move round **four places**. Two of them are in this view: −1.31 at the marker, and −1.14 on the right, inside the big blue-green bulb. The other two are outside the view. The mini-map at the bottom left shows all four.

This bulb goes from −1.25 to −1.368, so it is 0.118 long. That is **4.2 times shorter** than the period-2 bulb, which is 0.5 long.

## Ever smaller
center: -1.388, 0
span: 0.03
probe: -1.3815474844320617, 0

The period-8 bulb goes from −1.368 to −1.394 (0.026 long). The period-16 bulb goes on to −1.3996 (0.0056 long). After it come bulbs of period 32, 64, and so on.

Each bulb is shorter than the one before: **4.23, 4.55, 4.65, …** times shorter. These numbers come closer and closer to **4.669…**, which is called *Feigenbaum's constant*.

In this view, the big dark circle is the period-8 bulb, and the marker is at its middle. The smaller purple circle on its left is the period-16 bulb. The dots of the marker move round 8 places, all outside the view except the one at the marker. The mini-map at the bottom left shows them on the real line; some of them are so close together that they look like one dot.

## The Feigenbaum point
center: -1.4011551890920506, 0
span: 0.002
probe: -1.4011551890920506, 0
marker: -1.4011551890920506, 0
marker-label: Feigenbaum point

Each bulb is about 4.669 times shorter than the one before. So the bulbs of period 2, 4, 8, 16, … come closer and closer together, and they end at one point: **−1.40115518909…**, the labelled point in the middle of the view. It is called the *Feigenbaum point*. It is named after Mitchell Feigenbaum, like the constant 4.669, but it is a different number.

In this view, the big pink circle on the right is the period-32 bulb, and the smaller yellow circle is the period-64 bulb. The next bulbs are too small to see. At the labelled point, the dots never settle into a loop, and they never escape. The text above the mini-map says so: the app is still undecided after 100 000 steps.

The same number 4.669 appears in dripping taps, in electronic circuits and in animal populations: wherever a loop doubles again and again until there is no loop any more. Ben Sparks finds it in a population model, and he shows that the picture of that model is the real line of the Mandelbrot set. See Numberphile's [The Feigenbaum Constant (4.669)](https://www.youtube.com/watch?v=ETrYE4MdoLQ).

## Chaos
center: -0.4, 0
span: 2.4
probe-path: -1.4012, 0 -> -1.43, 0 -> -1.46, 0
probe-seconds: 16
marker: off

To the left of the Feigenbaum point is the **spike**, the thin line that goes left to −2. The marker now moves along the start of the spike, at the left end of the bulbs. For most points on the spike, the dots never repeat. Instead they spread over a long part of the real line: look at the many white dots on the real line, from the marker to beyond the right side of the heart. This is called **chaos**. If the marker moves a very small distance, then after a few dozen steps its dots are in completely different places.

Sometimes order comes back for a moment. In some very short parts of the spike, the dots settle into a loop of 12, 10 or 14 places.

For some points, nobody knows what the dots do in the end. Holly Krieger says that nobody knows if the dots of c = −1.5 settle into a loop in the end. It is also an open question whether the points of every region inside the set settle into a loop (Numberphile: [The Mystery of Hyperbolicity](https://www.youtube.com/watch?v=eC9-1wY7FvU)).

## Order returns
center: -1.768, 0
span: 0.05
probe: -1.7548776662466927, 0

At **−1.75** order comes back. A **minibrot**, a small copy of the whole set, sits on the spike. Its heart is the big purple shape. The marker is in this heart, at c = −1.7549. Its dots move round **three places**: −1.755, 0 and 1.325. Only the first is in this view; the mini-map at the bottom left shows all three.

And the pattern repeats. To the left of this small heart, its own bulbs double the loop again: the orange circle has period 6, the small light green circle has period 12, then come 24, 48, … Each bulb is again about 4.669 times shorter than the one before.

Number theorists know this point too: −1.75 is the fraction −7/4. Use the formula with c = −7/4, starting at z = 0: z goes 0, −7/4, 21/16, −7/256, … Look at the top numbers of the fractions: 7, then 21 = 3 × 7, then 7 again. Usually every step brings a prime factor that has not appeared before (21 brings the 3). But the third step here brings none. No other fraction is known to do this, and any other one would have to lie deep inside the set (Numberphile: [63 and −7/4 are special](https://www.youtube.com/watch?v=09JslnY7W_k&t=570s), from 9:30).

## Your turn
center: -1.25, 0
span: 1.2
probe: pointer

::: mac
The marker and its dots follow the pointer again. Move the pointer along the real line, through the bulbs and onto the spike. Watch the loops split into 2, 4, 8, … places, and then turn into chaos.

**Back** and **Next** (or **←** and **→**) step through the tour. **End tour** (or **Esc** twice) ends it and brings back your view and settings.
:::
::: touch
Now you move the marker: hold a finger on the picture, choose *Orbit Here*, and keep the finger down to drag the marker. Move it along the real line, through the bulbs and onto the spike. Watch the loops split into 2, 4, 8, … places, and then turn into chaos.

**Back** and **Next** step through the tour. **End tour** ends it and brings back your view and settings.
:::
