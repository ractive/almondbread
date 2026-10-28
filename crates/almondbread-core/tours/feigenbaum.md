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
labels: 0.05, 0.35, the heart; -1, 0.15, a bulb

A reminder: each point of the picture is one number c. The formula starts with z = 0, and at each step z → z² + c (square z, then add c). The set is made of the numbers c for which z stays small. The blue area around it is outside the set.

The small circle (the marker) is the point c = −0.2. The white dots are the values of z, one per step. With c = −0.2, z goes 0, −0.2, −0.16, −0.174, −0.170, … The values come closer and closer to one place, −0.171, and then stay there. So the white dots make one small group right next to the marker. This place is called a *fixed point*; the text above the mini-map says so.

Every c in the heart (labelled), the big shape with the notch on the right, behaves like this. Inside the set the colours change with the number of places, but the same colour comes back for other numbers, so you cannot read the number from a colour.

## Two places
probe: -1.1, 0
labels: -0.75, 0, −0.75; -1.25, 0, −1.25; 0, 0, 0; -2, 0, −2

The horizontal line through the middle of the set is the **real line**: the number line you know from school. 0 is inside the heart and −2 is the tip on the far left (both labelled). The marker is now at c = −1.1, inside the big round bulb on the left. This bulb touches the heart at −0.75 (labelled).

Here the dots no longer come closer to one place. They end up moving between **two places**: −1.09, right next to the marker, and 0.09, just right of the label 0. A white line joins the two. After a moment, the text above the mini-map gives the count. z goes …, −1.09, 0.09, −1.09, 0.09, … again and again. This is called a loop of 2 (or a *period-2 cycle*). Every c in this bulb, from −0.75 to −1.25 (both labelled), behaves like this.

## Watch it split
probe-path: -0.7, 0 -> -1.0, 0 -> -1.25, 0 -> -1.33, 0 -> -1.37, 0 -> -1.39, 0 -> -1.40, 0 -> -1.45, 0
probe-seconds: 24
labels: -0.75, 0, −0.75; -1.25, 0, −1.25; -1.368, 0, −1.368

Now the marker moves slowly along the real line, from −0.7 to −1.45 and back. Watch the white dots on the real line. Each time the marker passes a labelled point, every dot splits into two. The two new dots start close together and then move apart. So 1 place becomes **2**, then **4**, then **8**: the loop has twice as many places each time. This is called *period doubling*.

The splitting points come closer and closer together: −0.75, −1.25, −1.368 (labelled), then −1.394 and −1.3996, too close together to label here. Near −1.45, at the far left end of the path, the dots do not settle into a loop at all.

## Period 4
center: -1.31, 0
span: 0.3
probe: -1.3107026413368328, 0
labels: -1.25, 0, −1.25; -1.368, 0, −1.368

The round bulb in the middle of the view is the next bulb on the real line, to the left of the period-2 bulb (the large area on the right). It goes from −1.368 to −1.25 (both labelled), and the marker is at its middle, c = −1.3107. Here z goes 0, −1.31, 0.41, −1.14, 0, −1.31, … It comes back to 0 after four steps, so the dots move round **four places**: period 4.

Two of the four places are in this view: −1.31 at the marker, and −1.14 to the right, in the period-2 bulb. The other two, 0.41 and 0, are outside the view. The mini-map at the bottom left shows the whole line of dots.

This bulb is 1.368 − 1.25 = 0.118 wide. The period-2 bulb, from −0.75 to −1.25, is 0.5 wide. 0.5 ÷ 0.118 ≈ **4.2**: the period-2 bulb is 4.2 times as wide as this one.

## Ever smaller
center: -1.388, 0
span: 0.03
probe: -1.3815474844320617, 0
labels: -1.368, 0, −1.368; -1.394, 0, −1.394; -1.3996, 0, −1.3996

The white dots move round **8** places; only the one at the marker is in this view. The text above the mini-map gives the count: a period-8 cycle.

The big round bulb with the marker has period 8. It goes from −1.368 to −1.394 (labelled), so it is 0.026 wide. The smaller bulb on its left has period 16. It goes from −1.394 to −1.3996, so it is 0.0056 wide. The small bulb left of −1.3996 has period 32; after it come 64, 128, and so on.

Divide the width of each bulb by the width of the next one: 0.5 ÷ 0.118 ≈ 4.2, then 0.118 ÷ 0.026 ≈ 4.5, then 0.026 ÷ 0.0056 ≈ 4.6. These numbers come closer and closer to **4.669…**, which is called *Feigenbaum's constant*.

## The Feigenbaum point
center: -1.4011551890920506, 0
span: 0.002
probe: -1.4011551890920506, 0
marker: -1.4011551890920506, 0
marker-label: Feigenbaum point
labels: -1.4007, 0.0002, period 32; -1.40096, 0, period 64

Each bulb is about 4.669 times as wide as the next one. So the bulbs of period 2, 4, 8, 16, 32, 64, … get smaller and smaller, and they end at one point: **−1.40115518909…**, the labelled point in the middle of the view. It is called the *Feigenbaum point*. On its right you can see the period-32 bulb and the period-64 bulb (labelled). The next bulbs are too small to see.

At the Feigenbaum point, the dots never settle into a loop, and they never escape. The app stops after 100 000 steps. So the text above the mini-map says "still undecided after 100 000 steps".

The point and the constant 4.669 are both named after Mitchell Feigenbaum; they are two different numbers.

## The same number everywhere

The number 4.669 is not only in this picture. It also appears in dripping taps, in electronic circuits and in animal populations: wherever a loop doubles again and again until there is no loop any more.

In a video on Numberphile, a YouTube channel about maths, Ben Sparks finds the number 4.669 in a simple model of an animal population. He shows that the picture of this model (a diagram of where its loops split) lines up with the real line of the Mandelbrot set: [The Feigenbaum Constant (4.669)](https://www.youtube.com/watch?v=ETrYE4MdoLQ).

## Chaos
center: -0.4, 0
span: 2.4
probe-path: -1.4012, 0 -> -1.43, 0 -> -1.46, 0
probe-seconds: 16
marker: off
labels: -1.4011551890920506, 0, Feigenbaum point; -1.5, 0, −1.5; -2, 0, −2

To the left of the Feigenbaum point (labelled), the set looks like a thin line along the real line, out to −2 (labelled). This thin line is called the **spike**. The marker now moves slowly along the start of the spike. For many points on the spike, the dots never repeat. Instead they spread over a long part of the real line, from the marker to beyond the notch of the heart. This is called **chaos**. Watch the white dots while the marker moves: they jump to new places all the time, even when the marker moves only a very small distance.

In a few very short parts of the spike, order comes back and the dots settle into a loop again (of 12, 14 or 10 places, in the order the marker meets them). These parts are too short to see while the marker moves; the last card lets you look for them.

Some questions here are still open: no mathematician knows the answer yet. Holly Krieger, a mathematician, says that nobody knows if the dots of c = −1.5 (labelled) settle into a loop in the end.

Another open question: in the heart and in every bulb, the dots settle into a loop. Is that so everywhere inside the set, with no exception? Nobody knows (Numberphile: [The Mystery of Hyperbolicity](https://www.youtube.com/watch?v=eC9-1wY7FvU)).

## Order returns
center: -1.768, 0
span: 0.05
probe: -1.7548776662466927, 0
labels: -1.75, 0, −1.75; -1.77288, 0, period 6; -1.77828, 0, period 12

Near −1.75, order comes back. A **minibrot**, a small copy of the whole set, sits on the spike. Its heart is the big shape in the middle of the view, with the notch on the right at −1.75 (labelled). The marker is at the centre of this small heart, at about −1.755. Here z goes 0, −1.755, 1.325, 0, −1.755, … so the dots move round **three places**. Only the first is in this view; the mini-map at the bottom left shows the whole line of dots.

And the pattern repeats. To the left of the small heart are its own bulbs, and the doubling starts again: period 6, then 12 (both labelled), then 24, 48, … Again each bulb is about 4.669 times as wide as the next one.

## A special fraction
labels: -1.75, 0, −1.75 = −7/4

The view is the same as before. The notch of the small heart, c = −1.75 (labelled), is special in one more way. −1.75 is the fraction −7/4, and this fraction is special in number theory, the study of whole numbers. Use the formula with c = −7/4, starting at z = 0: z goes 0, −7/4, 21/16, −7/256, …

Look at the top number of each fraction (the numerator): 7, then 21, then 7. A prime is a whole number that only 1 and itself divide (2, 3, 5, 7, …); a prime factor of a number is a prime that divides it: 21 = 3 × 7 has the prime factors 3 and 7. Usually every step brings a prime factor that has not appeared before: 21 brings the 3. But the third step, −7/256, has the top number 7 again: it brings no new prime factor. No other fraction is known to do this (Numberphile: [63 and −7/4 are special](https://www.youtube.com/watch?v=09JslnY7W_k&t=570s), from 9:30).

## Your turn
center: -1.1, 0
span: 1.2
probe: pointer
labels: -0.75, 0, −0.75; -1.25, 0, −1.25; -1.4011551890920506, 0, Feigenbaum point; -2, 0, −2

::: mac
Now the marker and its dots follow the pointer. Move the pointer along the real line: through the big bulb, past −1.25 into the smaller bulbs, and onto the spike. Watch the loops split into 2, 4, 8, … places, and then turn into chaos. To see the small bulbs near the Feigenbaum point, zoom in there.

**Finish** or **End tour** (or **Esc** twice) ends the tour and brings back your view and settings. **Back** (or **←**) goes back a card.
:::
::: touch
Now you move the marker: hold a finger on the picture, choose *Orbit Here*, and keep the finger down to drag the marker. Move it along the real line: through the big bulb, past −1.25 into the smaller bulbs, and onto the spike. Watch the loops split into 2, 4, 8, … places, and then turn into chaos. To see the small bulbs near the Feigenbaum point, zoom in there.

**Finish** or **End tour** ends the tour and brings back your view and settings. **Back** goes back a card.
:::
