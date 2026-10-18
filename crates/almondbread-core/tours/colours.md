---
id: colours
title: What the colours mean
summary: Escape counts and bands, smooth colours, palettes and looks, the fog, and the five ways to colour the inside.
place: whole-set
---

## Colours count steps
center: -0.6, 0
span: 2.6
look: aurora
interior: flat
orbit: on
orbit-length: 60
map: on
probe: 0.6, 0

Every coloured point **escapes**: its numbers fly off sooner or later. The colour says **how many steps** that took. The ringed point, far out, escapes after just a few steps.

Points with the same count form a **band**, and the bands follow the shape of the edge, like the lines on a map.

## Closer means more steps
probe-path: 0.6, 0 -> 0.35, 0 -> 0.28, 0 -> 0.26, 0 -> 0.2501, 0
probe-seconds: 10
probe-motion: once

Now the point moves left towards the dent of the heart, the edge of the set. Watch the line under the map: the count grows as it gets closer, and the colour changes band by band.

Right at the edge points take thousands or millions of steps. That is where the colours crowd together into fine detail.

## Smooth colours
probe: pointer
orbit: off
map: off

Counting whole steps alone would give stairs: flat bands with hard edges. Almondbread also looks at **how far** the number jumped past the escape line in its last step, and turns the count into a smooth fraction.

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

Black is the **inside**: points whose numbers stay small for ever. Almondbread paints a point black only once it has **proven** that it stays, like the round bulbs along these walls.

The inside can be coloured too. Let's look at the big heart and its bulbs.

## Bubbles
center: -0.45, 0.25
span: 1.3
interior: bubbles

Inside, the numbers settle into a loop. **Bubbles** colours each point by how close its loop comes to zero: rings around each bulb's centre, where the loop passes exactly through zero.

## Period
center: -0.45, 0.25
span: 1.3
move: jump
interior: period

**Period** gives one colour per loop length: the heart (1), the big disc (2), the top bulbs (3), and so on. Bulbs with the same period share a colour.

## Internal angle
center: -0.45, 0.25
span: 1.3
move: jump
interior: angle

**Internal angle** draws spokes and rings in every bulb. They show *where* inside the bulb a point is, measured by how its loop pulls in nearby numbers. The spokes meet at each bulb's centre.

## Interior distance
center: -0.45, 0.25
span: 1.3
move: jump
interior: distance

**Interior distance**: bright deep inside, dark towards the edge. It estimates how far each point is from the boundary.

## Your turn
center: -0.6, 0
span: 2.6
interior: flat
probe: pointer

The control center's **Interior** section switches between flat, bubbles, period, angle and distance at any time, and **Look & colour** changes the rest.

**End tour** (or **Esc**) brings back your view and settings.
