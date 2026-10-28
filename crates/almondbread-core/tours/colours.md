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

Remember: every point of the picture is a number c, and z → z² + c is used again and again from z = 0. The small white circle is the **marker**: it shows the c we look at. The **orbit** {icon:orbit}, the white dots, shows the values of z, one dot per step.

A coloured point **escapes**: its z keeps growing. The colour shows **how many steps** z needed to escape. The marker here is far outside the black shape, so its dots leave the picture at once. The line above the mini-map, the small map at the bottom left, says that it escapes after 8 steps.

Points with the same number of steps form a **band** around the set. The bands follow the shape of the edge of the set. In this view the bands blend into each other: the next card explains why.

## Closer means more steps
probe-path: 0.55, 0.2 -> 0.371, -0.044 -> 0.3237, 0.007 -> 0.31, 0.0217 -> 0.30538, 0.0268 -> 0.30402, 0.02827 -> 0.30381, 0.0292
probe-seconds: 16
probe-motion: once

The big black shape on the left is the **heart**, the big heart-shaped part of the set (its proper name is the *main cardioid*). Now the marker moves slowly to the edge of the heart, just above the dent on its right side.

Watch the dots: the closer the marker comes to the edge, the longer the dots go round before they escape. The count above the mini-map grows (12, 14, 17, 31, 55 steps), and the colour at the marker changes.

At the end of the path the dots go round for about 600 steps, and a very small move of the marker changes the count a lot. Right at the edge, z needs thousands or millions of steps to escape. That is why the colours there are very close together and show fine detail.

## Smooth colours
probe: pointer
orbit: off
map: off

If the colours used only whole numbers of steps, the picture would show flat bands with hard edges, like stairs.

So Almondbread also looks at the last step: how far z went past the size at which it counts as escaped. From this it turns the count into a number with a fraction. That is why the colours flow smoothly from one band into the next.

## Palettes
place: seahorse-valley
palette: grey

The view has moved to *seahorse valley*: the gap between the heart (the black area on the right) and the big round part on its left (the black area on the left).

A **palette** is a list of colours. The step count picks a place in the list. After the last colour, the list starts again with the first. This palette has only black, white and the greys between them.

A palette changes only the colours. The step counts stay the same.

## Looks
look: ember

A **look** is a palette together with lighting, a line along the edge of the set, and a colour for the inside. This look is called *Ember*.

The section **Look & colour** in the control center has more looks, and you can change each part of them.

## Fog
center: -0.74999996120361811, 0.00036918279150454967
span: 1.5e-8
look: aurora

The dark grey areas are **fog**. Fog means "not decided yet". This view is the narrow gap between the heart and the big round part on its left, just above the point where they touch, very much magnified (the box at the top left says 2.3e8×).

Almondbread has tried many steps for these points. Their z has not escaped yet. Almondbread has also not proven that they are in the set. The box at the top left says *computing further*: Almondbread is still working on them.

These points do escape, after millions of steps. The tour *Why some points are hard* tells more.

## Black means proven
place: seahorse-valley
move: jump

Black shows points that are **in the set**: their z always stays small. Almondbread paints a point black only after it has **proven** that its z stays small. The round parts along the walls of this valley are black for this reason.

The inside of the set can have colours too. The next cards show the heart and its **bulbs**, the round parts attached to it.

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

Now the inside of the set has colours too. Inside the set, z does not escape. Instead the dots end up going round a **loop**: they visit the same few places again and again. The number of places in the loop is its **period**.

The marker (the small white circle) moves up and down in the bulb at the top of the heart. There the period is 3, so the white dots form a triangle.

**Bubbles** colours each point by how close its loop comes to 0, the gold circle labelled **zero**. At the centre of the bulb, the loop goes exactly through 0. Further from the centre, the loop misses 0 by more, and the colour changes ring by ring.

## Period
center: -0.4, 0.3
span: 1.5
interior: period
orbit-length: 60
probe-path: 0, 0 -> 0, 0 -> -1, 0 -> -1, 0 -> -0.1226, 0.7449 -> -0.1226, 0.7449 -> 0.2822, 0.5301 -> 0.2822, 0.5301
probe-seconds: 16
probe-motion: bounce
marker: off

**Period** gives each loop length its own colour. The marker visits the centres of four parts of the set, and stops at each one:

- the heart: period 1, all dots on one place
- the big round bulb on the left: period 2
- the bulb at the top: period 3
- a bulb on the upper right: period 4

When the marker stops, the line above the mini-map names its loop, for example *settles into a period-3 cycle* in the bulb at the top (a cycle is a loop). Bulbs with the same period have the same colour. But different periods can have similar colours, so the line above the mini-map is the sure way to read a period. The tour *What a minibrot's period means* tells more.

## Internal angle
center: -0.4, 0.3
span: 1.5
interior: angle
probe: 0.3, 0.1

**Internal angle** draws spokes and rings in every bulb.

The marker is now inside the heart, near its right edge. Its dots spiral in towards one place, as for every point in the heart. The line above the mini-map calls this *settles on a fixed point*.

At each step the dots turn by about the same angle. Points whose dots turn by the same angle lie on one spoke. The rings show how fast the dots come closer to their place. The spokes meet at the centre of each bulb.

## Interior distance
center: -0.4, 0.3
span: 1.5
interior: distance
orbit: off

**Interior distance** colours each point inside the set by an estimate of its distance from the edge. Points at the same distance from the edge get the same colour. So the colours form rings that follow the edge of the heart and the edge of each bulb.

## Your turn
center: -0.6, 0
span: 2.6
interior: flat
probe: pointer

The section **Interior** in the control center switches between flat, bubbles, period, angle and distance at any time. **Look & colour** changes the rest.

**End tour** (or **Esc** twice) brings back your view and settings.
