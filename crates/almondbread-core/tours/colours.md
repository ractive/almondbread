---
id: colours
title: What the colours mean
summary: Escape counts and bands, smooth colours, palettes and looks, the fog, and the five ways to colour the inside.
place: whole-set
thumbnail: spiral-wheel
---

## Colours count steps
center: 0.3, 0.15
span: 1.4
look: aurora
interior: flat
orbit: on
orbit-length: 1000
map: on
probe: 0.55, 0.2

Each point of the picture starts a little game: begin with zero, square the number and add the point, again and again. The **orbit** {icon:orbit} shows where the number jumps, one dot per step, for the point in the circle.

A coloured point **escapes**: its number flies off sooner or later. The colour says **how many steps** that took. This point is far out: its dots leave the picture at once, and the line above the map says it escapes after 8 steps.

Points with the same count form a **band**, and the bands follow the shape of the edge, like the lines on a map.

## Closer means more steps
probe-path: 0.55, 0.2 -> 0.371, -0.044 -> 0.3237, 0.007 -> 0.31, 0.0217 -> 0.30538, 0.0268 -> 0.30402, 0.02827 -> 0.30381, 0.0292
probe-seconds: 16
probe-motion: once

Now the point creeps up to the edge of the black heart, just above its dent. Watch the dots: the closer it gets, the longer they whirl round before they fly off. The count above the map grows (12, 17, 31, 55 steps) and the colour changes band by band.

At the very end they whirl round for hundreds or thousands of steps, and the count jumps about: a hair's move changes it a lot. Right at the edge points take thousands or millions of steps. That is where the colours crowd together into fine detail.

## Smooth colours
probe: pointer
orbit: off
map: off

Counting whole steps alone would give stairs: flat bands with hard edges. Almondbread also looks at **how far** the number jumped past the size where it counts as flown off, in its last step, and turns the count into a smooth fraction.

That is why the colours flow from one band into the next instead of jumping.

## Palettes
place: seahorse-valley
palette: grey

A **palette** is a loop of colours. The count picks a spot on the loop; after a while round, the colours start again. Here the palette is just black to white.

The count is the same as before: only the paint changed.

## Looks
look: ember

A **look** sets the palette together with lighting, an outline along the edge and the colour of the inside. This one is *Ember*.

The control center's **Look & colour** has more, and you can fine-tune each of them.

## Fog
center: -0.74999996120361811, 0.00036918279150454967
span: 1.5e-8
look: aurora

Grey **fog** means "not decided yet": these points, in the narrow gap above the neck, have neither escaped nor been proven inside within the steps tried so far. They do escape, after millions of steps. The *Why some points are hard* tour tells more.

## Black means proven
place: seahorse-valley
move: jump

Black is the **inside**: points whose number stays small for ever. Almondbread paints a point black only once it has **proven** that it stays, like the round bulbs along these walls.

The inside can be coloured too. Let's look at the big heart and its bulbs.

## Bubbles
center: -0.4, 0.3
span: 1.5
interior: bubbles
orbit: on
orbit-length: 100
probe-path: -0.1226, 0.7449 -> -0.1226, 0.82
probe-seconds: 6
probe-motion: bounce
marker: 0, 0
marker-label: zero

Inside, the number never flies off: it ends up going round a **loop**, visiting the same few spots again and again. How many spots the loop has is its **period**. The point in the circle moves about in the top bulb, where the period is 3: the dots form a triangle.

**Bubbles** colours each point by how close its loop comes to the spot marked **zero**. At the bulb's centre the loop passes right through zero; further out it misses by more, and the colour changes ring by ring.

## Period
center: -0.4, 0.3
span: 1.5
interior: period
orbit-length: 60
probe-path: 0, 0 -> 0, 0 -> -1, 0 -> -1, 0 -> -0.1226, 0.7449 -> -0.1226, 0.7449 -> 0.2822, 0.5301 -> 0.2822, 0.5301
probe-seconds: 16
probe-motion: bounce
marker: off

**Period** gives each loop length its own colour. The point in the circle visits the centres of the heart (period 1: all dots on one spot), the big disc (2), the top bulb (3) and a bulb on the right (4). The line above the map names each loop.

Bulbs with the same period share a colour. The tour *What a minibrot's period means* tells more.

## Internal angle
center: -0.4, 0.3
span: 1.5
interior: angle
probe: 0.3, 0.1

**Internal angle** draws spokes and rings in every bulb. Inside, the number spirals in towards its loop, turning by the same angle each time round: here the dots spiral into the heart's one-spot loop.

Points that turn by the same angle lie on one spoke; the rings show how fast the spiral closes in. The spokes meet at each bulb's centre.

## Interior distance
center: -0.4, 0.3
span: 1.5
interior: distance
orbit: off

**Interior distance**: bright deep inside, dark towards the edge. It estimates how far each point is from the boundary.

## Your turn
center: -0.6, 0
span: 2.6
interior: flat
probe: pointer

The control center's **Interior** section switches between flat, bubbles, period, angle and distance at any time, and **Look & colour** changes the rest.

**End tour** (or **Esc** twice) brings back your view and settings.
