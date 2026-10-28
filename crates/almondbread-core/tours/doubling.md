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

The big round bulb on top of the set has a thin, branching line: its **antenna**. In this view, the antenna splits into **three** arms. Every arm is covered with smaller forks and stars.

Close to this fork there is a **minibrot**, a small copy of the whole set. It is a hundred million times smaller than this view, so you cannot see it yet. In this tour we zoom towards it and watch how the shapes change on the way.

## The hidden minibrot
center: -0.101095952005097279, 0.956281332988737429
span: 2e-5
marker: -0.101095952005097279, 0.956281332988737429
marker-label: a minibrot of period 39

The three-way fork is now at the top of the view. The labelled point in the middle is a minibrot of **period 39**, still much too small to see. Remember the formula z → z² + c: for c in this minibrot, the values of z end up moving round 39 places, again and again. That is a loop of 39, the period of the minibrot.

From now on, the view stays centred on this minibrot.

## Six arms
center: -0.101095952005097279, 0.956281332988737429
span: 1.5e-6
marker: off

We are ten times closer. In the middle of the view there is now a star with **six** arms: twice the three arms of the fork.

## Twelve arms
center: -0.101095952005097279, 0.956281332988737429
span: 1.4e-8

A hundred times closer again, and the star in the middle has **twelve** arms. Around it, the shapes from before are still there, but everything is doubled.

## Twenty-four arms
center: -0.101095952005097279, 0.956281332988737429
span: 8e-10

Now the star has **twenty-four** arms, and in the middle a ring of fine lace appears. Each doubling comes after about half of the zoom that is still left. So the doublings come faster and faster as the minibrot comes nearer.

## The minibrot
center: -0.101095952005097279, 0.956281332988737429
span: 1.2e-10

And there it is in the middle of the view: the period-39 minibrot, a small copy of the whole set. Around it are all the doubled rings from the way down.

## Why it doubles

Close to a minibrot, 39 steps of the formula together act like a single new step: square, then add. And **squaring doubles every angle**. Seen from the minibrot, the direction of a number turns twice as far. So every shape around the minibrot is wrapped around it twice: 3 arms become 6, 6 become 12, and so on.

The rings around the minibrot in this view are these copies, one inside the other. People call such a ring an *embedded Julia set* (the Julia tour shows what a Julia set is).

## What deep-zoom videos show
place: embedded-julia

This view is near the *Airplane*, a minibrot on the spike (the thin line on the left of the set). In the middle of the view is another minibrot, too small to see at this zoom. Around it is a shape of spirals that looks like a Julia set.

Deep-zoom videos go down towards minibrots that are thousands of digits deep, and they pass dozens of these doublings. On the way, artists steer a little away from the centre, to choose which shape gets doubled next. This is called *Julia morphing*.

## Your turn
place: three-way-junction
move: jump

::: mac
We are back at the three-way fork. Choose any place near the edge of the set, keep the pointer on it, and zoom in slowly with **E**. When shapes start to double, a minibrot is near. **N** finds a minibrot for you.

**Finish** or **End tour** (or **Esc** twice) ends the tour and brings back your view and settings. **Back** (or **←**) goes back a card.
:::
::: touch
We are back at the three-way fork. Choose any place near the edge of the set and zoom in slowly towards it. When shapes start to double, a minibrot is near. To find a minibrot, hold a finger on the picture and choose *Seek Minibrot Here*.

**Finish** or **End tour** ends the tour and brings back your view and settings. **Back** goes back a card.
:::
