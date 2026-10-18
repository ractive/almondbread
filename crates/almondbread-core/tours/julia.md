---
id: julia
title: Every point has a Julia set
summary: The other way to play the same rule: fix c and vary the start. Connected Julia sets inside the Mandelbrot set, dust outside, and the places where the two look alike.
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
probe: -1, 0
marker: off

The Mandelbrot set always **starts at 0** and changes **c** from pixel to pixel. Turn it around: keep one c, and start from every other number z instead. The starts whose numbers stay small form the **Julia set** of that c (strictly, its filled-in shape; named after Gaston Julia).

So every point c of this picture has its own Julia set: a whole picture of its own. The ring marks c = −1, whose orbit from 0 goes 0, −1, 0, −1, … Almondbread draws only the Mandelbrot set, but this tour shows how the two fit together.

## Inside: one piece
probe: -0.12256116687665362, 0.74486176661974424

The ring is at the centre of the top bulb. Its orbit from 0 settles into a loop of 3, and its Julia set is the famous **Douady rabbit**: a fat body with ears, all in **one connected piece**.

The c = −1 from the first step gives a Julia set called the *basilica*, and the Airplane's centre on the spike gives the *airplane*. All connected.

## Outside: dust
probe: 0.5, 0.5

Now c is outside the set: from 0 the numbers fly off. Then the Julia set of c is **dust**: infinitely many separate specks, no two of them joined.

That is the deepest fact about the Mandelbrot set: it is exactly the set of the c whose Julia sets are in **one piece**. Black here means "connected Julia set", colour means "dust".

## Dust you can see
place: cantor-island
orbit: off
map: off
probe: pointer

On the spike near −1.75, an island wears a ruff with four lobes. It looks connected, but zoom in and it falls apart into specks: the shape of a dust-like Julia set, copied into the Mandelbrot set.

## Like its own Julia set
place: two-arm-spiral

At a point on the edge, the Mandelbrot set looks like the **Julia set of that very c**, seen up close at c. This double spiral in seahorse valley sits on such a point (M(24,2)), and the Julia set of that c winds into the same two-armed spirals.

The closer you zoom, the better they match (Tan Lei proved this in 1990).

## A fork for a fork
place: three-way-junction

Here the antenna of the top bulb forks into three. The Julia set of this c is a tree of thin branches with **three-way forks** everywhere: the same fork, again.

## Julia sets inside
place: embedded-julia

Deep inside the set, around small minibrots, whole Julia-set shapes appear: rings with lobes and lace. People call them **embedded Julia sets**: they look like the Julia set of a c close by, wrapped around the minibrot.

## Your turn
center: -0.6, 0
span: 2.6
move: jump
orbit: on
map: on

The orbit follows the pointer again (on a touch screen: hold a finger and choose *Orbit Here*). Wherever the line says the orbit settles or loops, that c has a Julia set in one piece; where it escapes, dust.

**End tour** (or **Esc**) brings back your view and settings.
