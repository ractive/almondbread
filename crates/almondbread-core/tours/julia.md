---
id: julia
title: Every point has a Julia set
summary: The other way to play the same rule: keep the point and vary the start. Connected Julia sets inside the Mandelbrot set, dust outside, and the places where the two look alike.
place: cantor-island
---

## Two ways to play
center: -0.6, 0
span: 2.6
look: aurora
interior: flat
orbit: on
orbit-length: 60
map: on
julia: on
probe: -1, 0
marker: off

Every point of this picture gets the same test, the **rule**: start at 0, square the number, add the point, repeat. The ringed point is being tested; each dot is the number one more step makes. Here they hop between 0 and −1 for ever.

Now turn it round: keep the ringed point as the number you add, but **start** from every other number instead of 0. The starts whose numbers stay small make a picture of their own, the **Julia set** of that point (after Gaston Julia). The panel next to the map shows it: the *basilica*, one piece pinched into smaller and smaller bulbs.

## Inside: one piece
probe: -0.12256116687665362, 0.74486176661974424

The ring is now in the middle of the big bulb on top: its dots hop round a loop of **3** spots. The panel shows its Julia set, the famous **Douady rabbit**: a fat body with ears, all in **one connected piece**.

The point −1 from the first step gave the *basilica*, and the middle of the *Airplane*, a tiny copy of the set on the spike, gives the *airplane*. All in one piece.

## Outside: dust
probe: 0.5, 0.5

Now the ring is outside the set: its dots fly off. Then its Julia set is **dust**, as in the panel: endlessly many separate specks, no two of them joined.

That is the deepest fact about the Mandelbrot set: it is exactly the points whose Julia sets are in **one piece**. Black here means "connected Julia set", colour means "dust".

## Dust you can see
place: cantor-island
orbit: off
map: off
probe: pointer

On the **spike**, the thin line on the left of the set, an island wears a ruff with four lobes. It looks connected, but zoom in and it falls apart into specks: the shape of a dust-like Julia set, copied into the Mandelbrot set.

The panel follows the pointer now (on a touch screen: the middle of the picture). The Julia set of a point here is dust too, strung out along a line.

## Like its own Julia set
place: two-arm-spiral

Zoom in on a point on the edge, and the Mandelbrot set there looks like **that point's own Julia set**, seen up close at the same spot. This double spiral in seahorse valley sits on such a point, and the panel shows its Julia set: it winds into the same two-armed spirals.

The closer you zoom, the better they match (Tan Lei proved this in 1990).

## A fork for a fork
place: three-way-junction

Here the **antenna** of the big top bulb, its thin hair, forks into three. The Julia set of this point, in the panel, is a tree of thin branches with **three-way forks** everywhere: the same fork, again.

## Julia sets inside
place: embedded-julia
julia: off

Deep inside the set, around **minibrots**, tiny copies of the whole set, whole Julia-set shapes appear: rings with lobes and lace. People call them **embedded Julia sets**: they look like the Julia set of a point close by, wrapped around the minibrot.

## Your turn
center: -0.6, 0
span: 2.6
move: jump
orbit: on
map: on
julia: on

The ring, its dots and the Julia set follow the pointer again (on a touch screen: hold a finger and choose *Orbit Here*). Wherever the dots settle or loop, that point has a Julia set in one piece; where they fly off, dust. Watch the panel change as you move: **U** shows or hides it later.

**End tour** (or **Esc** twice) brings back your view and settings.
