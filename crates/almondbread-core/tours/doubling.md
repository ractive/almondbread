---
id: doubling
title: Patterns that double
summary: Zoom towards a hidden minibrot and watch the shapes around it double: 3 arms, 6, 12, 24.
place: three-way-junction
---

## A fork with three arms
place: three-way-junction
interior: flat
orbit: off
map: off
marker: off

The big round bulb on top of the set has a thin hair, its **antenna**. Here the antenna forks into **three** arms. Every arm is covered in smaller forks and stars.

Somewhere close to this fork hides a **minibrot**, a tiny copy of the whole set, a hundred million times smaller than this view. Let's zoom towards it and watch what happens on the way.

## Something hides here
center: -0.101095952005097279, 0.956281332988737429
span: 2e-5
marker: -0.101095952005097279, 0.956281332988737429
marker-label: a minibrot of period 39

The fork is at the top now. The marked spot is a minibrot of **period 39**, still far too small to see. Test its points with the rule (start at 0, square the number, add the point, repeat) and the numbers end up going round a loop of 39: its period.

The view stays centred on it from now on.

## Six arms
center: -0.101095952005097279, 0.956281332988737429
span: 1.5e-6
marker: off

Ten times closer. The pattern at the centre is now a star with **six** arms: twice the fork's three.

## Twelve arms
center: -0.101095952005097279, 0.956281332988737429
span: 1.4e-8

Another hundred times closer, and the star has **twelve** arms. Around it the old shapes are still there, but everything is doubled.

## Twenty-four arms
center: -0.101095952005097279, 0.956281332988737429
span: 8e-10

**Twenty-four** arms now, and in the middle a ring of fine lace appears. Each doubling comes after about half of the zoom that is left: the steps get shorter as the minibrot gets nearer.

## The minibrot
center: -0.101095952005097279, 0.956281332988737429
span: 1.2e-10

And there it is: the period-39 minibrot, a tiny copy of the whole set, wrapped in all the doubled rings it gathered on the way.

## Why it doubles

Close to a minibrot, one round of its 39-step loop acts like a single fresh "square and add". And **squaring doubles every angle**: the direction a number points in, seen from the minibrot, turns twice as far. So whatever shape lies around the spot gets wrapped around twice: 3 arms become 6, 6 become 12, and so on.

The rings you passed are these copies, one inside the other. People call such a ring an *embedded Julia set* (the Julia tour shows what a Julia set is).

## What deep-zoom videos show
place: embedded-julia

Here near the *Airplane*, a minibrot on the spike (the thin line on the left of the set), another tiny minibrot (too small to see at this zoom) wears a ruff of spirals shaped like a Julia set.

Deep-zoom videos dive towards minibrots thousands of digits deep and pass dozens of these doublings. Artists steer a little off centre on the way, to choose which shape gets doubled next: that is called *Julia morphing*.

## Your turn
place: three-way-junction
move: jump

Back at the fork. Pick any spot near the edge and cruise in slowly with **E**, keeping the pointer on it: when shapes start to double, a minibrot is near. **N** finds one for you.

**End tour** (or **Esc** twice) brings back your view and settings.
