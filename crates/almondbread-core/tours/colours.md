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

Remember: every point of the picture is a number c, and z → z² + c is used again and again from z = 0. The **marker** (the small circle where the dots start) shows the c we look at. The **orbit** {icon:orbit}, the white dots, shows the values of z, one dot per step.

A coloured point **escapes**: its z gets further than 2 from 0 and then keeps growing. The colour shows **how many steps** z needed to escape. The marker here is far outside the black shape, so its dots leave the picture at once: only the first dot after the marker is on the picture, and the line leads to the next ones, far outside it. The line above the mini-map, the small map at the bottom left, says that it escapes after 8 steps.

Points with the same number of steps get the same colour. So the colours follow the shape of the edge of the set: dark blue far away from it, lighter blue and then pale yellow close to it. In this view the colour changes smoothly from point to point. The card *Smooth colours*, after the next one, explains why.

## Closer means more steps
probe-path: 0.55, 0.2 -> 0.371, -0.044 -> 0.3237, 0.007 -> 0.31, 0.0217 -> 0.30538, 0.0268 -> 0.30402, 0.02827 -> 0.30381, 0.0292
probe-seconds: 16
probe-motion: once

The big black shape on the left is the **heart**, the big heart-shaped part of the set (here it looks more like a bean lying on its side). Now the marker moves slowly to the edge of the heart, just above the pointed notch on its right side.

Watch the dots: the closer the marker comes to the edge, the more dots stay on the picture before they leave it. Near the end of the path, the dots make long jumps (one goes right across the heart) and form two curved chains to the right of the notch. The count above the mini-map grows: 12, 14, 17, 31, 55 steps.

At the end of the path z needs about 600 steps to escape, and a very small move of the marker changes the count a lot. Just outside the edge, z needs thousands or millions of steps. So points very close together have very different counts, and very different colours. That is why the colours change quickly near the edge and show fine detail.

## Smooth colours
probe: pointer
orbit: off
map: off

If the colours used only whole numbers of steps, the picture would show flat **bands** of one colour each, with hard edges between them, like stairs. (This view shows only the smooth result.)

The first tour counted until z was further than 2 from 0. Almondbread goes on counting until z is further than 65 536 from 0, and it looks at how far z went past that size in its last step. From this it gives the count decimals, for example 12.4 steps instead of 12. That is why the colours flow smoothly from one band into the next.

## Palettes
place: seahorse-valley
palette: grey
map: on
labels: -0.735, 0.135, heart; -0.7862, 0.1216, bulb

The view has moved to *seahorse valley*, magnified 87 times. The black area on the right, labelled *heart*, is part of the heart. The black area on the left, labelled *bulb*, is part of the big **bulb** on its left (a bulb is a round part attached to the heart). The valley is the gap between them. The mini-map at the bottom left shows where this view is.

A **palette** is a list of colours. The step count picks a place in the list. After the last colour, the list starts again with the first. This palette has only black, white and the greys between them.

Careful: this palette has black too, so here black does not always mean "in the set". The set is the two big black areas and the black circles along their edges. Most of the black in the thin lace between them is the palette's black.

A palette changes only the colours. The step counts stay the same.

## Looks
look: ember

A **look** is a palette together with a few more settings: lighting (a soft shine on the shapes), a glow along the edge of the set, and a colour for the inside. This look is called *Ember*: orange and red colours, an orange glow along the edge, and a black inside.

The **control center** is the panel that the button {icon:control-center} opens. Its section **Look & colour** has more looks, and you can change each part of them.

## Fog
center: -0.74999996120361811, 0.00036918279150454967
span: 1.5e-8
look: aurora

The dark grey areas are **fog**. Fog means "not decided yet". This view is in the narrow gap between the heart and the big bulb on its left, just above the point where they touch. It is magnified about 230 million times (the box at the top left says 2.3e8×, that is 2.3 × 10⁸). The mini-map at the bottom left shows where it is.

Almondbread has tried many steps for these points. Their z has not escaped yet. Almondbread has also not proven that they are in the set. The box at the top left says *computing further*: Almondbread is still working on them.

A slower test with many more steps shows that these points do escape in the end, after millions of steps. The tour *Why some points are hard* tells more.

## Black means proven
place: seahorse-valley
move: jump
labels: -0.735, 0.135, heart; -0.7862, 0.1216, bulb

Back in seahorse valley, magnified 87 times. At this magnification all points are decided, so there is no fog.

Black shows points that are **in the set**: their z never gets further than 2 from 0. Almondbread paints a point black only after it has **proven** this. The two big black areas, labelled *heart* and *bulb*, and the black circles along their edges are such proven points. The black circles are small bulbs.

The inside of the set can have colours too. The next cards show the heart and its bulbs.

## Bubbles
center: -0.4, 0.3
span: 1.5
interior: bubbles
orbit: on
orbit-length: 100
probe-path: -0.1226, 0.7449 -> -0.1226, 0.82
probe-seconds: 6
probe-motion: bounce
labels: 0, 0, zero

Now the inside of the set has colours too. Inside the set, z does not escape. Instead the dots end up going round a **loop**: they visit the same few places again and again. The number of places in the loop is its **period**.

The marker (the small circle where the dots start) moves up and down in the bulb at the top of the heart. There the period is 3, so the white dots form a triangle.

**Bubbles** colours each point by how close its loop comes to 0. The small dot labelled **zero**, in the heart, is the place of 0 (z = 0 and c = 0 are the same place on the picture). At the centre of the bulb, the loop goes exactly through 0. Further from the centre, the loop misses 0 by more, and the colour changes ring by ring.

The heart has rings too, around the point labelled *zero*. In the heart the loop has only one place, and that place is close to 0 when c is close to 0.

## Period
center: -0.4, 0.3
span: 1.5
interior: period
orbit-length: 60
probe-path: 0, 0 -> 0, 0 -> -1, 0 -> -1, 0 -> -0.1226, 0.7449 -> -0.1226, 0.7449 -> 0.2822, 0.5301 -> 0.2822, 0.5301
probe-seconds: 16
probe-motion: bounce
labels: 0, 0, period 1; -1, 0, period 2; -0.1226, 0.7449, period 3; 0.2822, 0.5301, period 4

**Period** gives each loop length its own colour. The marker visits four labelled points, and stops at each one:

- *period 1*, in the heart: the dots come closer and closer to one place
- *period 2*, in the big bulb on the left: the dots move between 2 places
- *period 3*, in the bulb at the top: 3 places
- *period 4*, in the round bulb on the upper right of the heart: 4 places

When the marker stops, the line above the mini-map names its loop, for example *settles into a period-3 cycle* in the bulb at the top (a cycle is a loop). Bulbs with the same period have the same colour. But different periods can have similar colours, so the line above the mini-map is the sure way to read a period. The tour *What a minibrot's period means* tells more (a **minibrot** is a small copy of the whole set).

## Internal angle
center: -0.4, 0.3
span: 1.5
interior: angle
probe: 0.3, 0.1

**Internal angle** draws spokes and rings in every bulb. The spokes are straight lines from the centre of a bulb to its edge, like the spokes of a bicycle wheel.

The marker is now inside the heart, near its right edge. Its dots spiral in towards one place, as for every point in the heart. The line above the mini-map calls this *settles on a fixed point*: a loop of only one place.

At each step the dots turn round that place by about the same angle. Points whose dots turn by the same angle lie on one spoke. The rings show how fast the dots come closer to their place. The spokes meet at the centre of each bulb.

## Interior distance
center: -0.4, 0.3
span: 1.5
interior: distance
orbit: off

**Interior distance** colours each point inside the set by an estimate of its distance from the edge. Points at the same distance from the edge get the same colour. So the colours form rings that follow the edge of the heart and the edge of each bulb. (With Bubbles, the rings were round the centres; here they follow the edge.)

## Your turn
center: -0.6, 0
span: 2.6
interior: flat
probe: pointer

The section **Interior** in the control center (the button {icon:control-center}) switches between flat (one colour, black, as on the first cards of this tour), bubbles, period, angle and distance at any time. **Look & colour** changes the rest.

**End tour** (or **Esc** twice) brings back your view and settings.
