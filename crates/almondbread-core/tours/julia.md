---
id: julia
title: Every point has a Julia set
summary: The other way to use the same formula: keep c and vary the start. Connected Julia sets inside the Mandelbrot set, dust outside, and the places where the two look alike.
place: cantor-island
---

## Two ways to use the formula
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

A reminder: each point of the picture is one number c. The formula starts with z = 0, and at each step z → z² + c. The small circle (the marker) is the point c = −1, and the white dots are the values of z. Here z goes 0, −1, 0, −1, … So the dots move between 0 and −1 again and again.

Now use the formula the other way round. Keep c = −1 fixed, but let z **start** at every other number instead of 0. For some starting numbers, z stays small. Together, these starting numbers make a picture of their own: the **Julia set** of c (named after Gaston Julia).

The panel labelled "Julia set", next to the mini-map at the bottom left, shows it. For c = −1 it is called the *basilica*: one black piece, with a round part in the middle and smaller and smaller round parts on each side, pinched together.

## Inside: one piece
probe: -0.12256116687665362, 0.74486176661974424

The marker is now in the middle of the big bulb on top. Its dots move round a loop of **3** places; the white lines make a triangle. The Julia set panel shows the Julia set of this point. It is the famous **Douady rabbit**: a body with ears, all in **one connected piece**.

c = −1 in the first step gave the *basilica*, also in one piece. The middle of the *Airplane*, a small copy of the set on the spike, gives the *airplane* Julia set: one piece again.

## Outside: dust
probe: 0.5, 0.5

Now the marker is outside the set, at the top right. Its dots escape: z keeps growing, and the white lines go out of the view. Then its Julia set is **dust**, as the panel shows: very many separate specks, and no two of them are joined.

This is the deepest fact about the Mandelbrot set: it is exactly the points c whose Julia sets are in **one piece**. So in this view, black means "the Julia set is one piece", and colour means "the Julia set is dust". Holly Krieger, a mathematician, explains it in a video on Numberphile, a YouTube channel about maths: [Filled Julia Set](https://www.youtube.com/watch?v=oCkQ7WK7vuY).

## Dust you can see
place: cantor-island
orbit: off
map: off
probe: pointer

This view is on the **spike**, the thin line on the left of the set; here it goes across the middle of the view. An island with a ruff of four lobes around it sits on the spike. It looks like one piece, but if you zoom in, it falls apart into specks. It has the shape of a dust-like Julia set, copied into the Mandelbrot set.

::: mac
The Julia set panel now follows the pointer. The Julia set of a point here is dust too, laid out along a line.
:::
::: touch
The Julia set panel now shows the point in the middle of the picture. The Julia set of a point here is dust too, laid out along a line.
:::

## Like its own Julia set
place: two-arm-spiral

Zoom in on a point on the edge of the set, and the Mandelbrot set there looks like **the Julia set of that same point**, seen up close at the same place. This double spiral in seahorse valley sits on such a point. The Julia set panel shows its Julia set: it turns into the same spirals with two arms.

The closer you zoom, the better they match (Tan Lei proved this in 1990).

## A fork for a fork
place: three-way-junction

The big bulb on top of the set has a thin, branching line: its **antenna**. In this view, the antenna splits into three branches. The Julia set of this point, in the panel, is a tree of thin branches with **three-way forks** everywhere: the same fork again.

## Julia sets inside
place: embedded-julia
julia: off

When you zoom in very far, near **minibrots**, the small copies of the whole set, whole Julia-set shapes appear: rings with lobes and lace. In the middle of this view is such a shape, with spirals in it. People call these shapes **embedded Julia sets**: they look like the Julia set of a point close by, wrapped around the minibrot.

## Your turn
center: -0.6, 0
span: 2.6
move: jump
orbit: on
map: on
julia: on

::: mac
Now the marker, its dots and the Julia set panel follow the pointer. Where the dots settle on one place or move round a loop, the Julia set of that point is in one piece. Where the dots escape, it is dust. Watch the panel change as you move the pointer. Later, **U** shows or hides the panel. Ben Sparks plays with Julia sets in the same way in Numberphile's [What's so special about the Mandelbrot Set?](https://www.youtube.com/watch?v=FFftmWSzgmk).

**Finish** or **End tour** (or **Esc** twice) ends the tour and brings back your view and settings. **Back** (or **←**) goes back a card.
:::
::: touch
Now you move the marker: hold a finger on the picture, choose *Orbit Here*, and keep the finger down to drag the marker. Its dots and the Julia set panel follow it. Where the dots settle on one place or move round a loop, the Julia set of that point is in one piece. Where the dots escape, it is dust. Watch the panel change as you move. Ben Sparks plays with Julia sets in the same way in Numberphile's [What's so special about the Mandelbrot Set?](https://www.youtube.com/watch?v=FFftmWSzgmk).

**Finish** or **End tour** ends the tour and brings back your view and settings. **Back** goes back a card.
:::
