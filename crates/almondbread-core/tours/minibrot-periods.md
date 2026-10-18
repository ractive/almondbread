---
id: minibrot-periods
title: What a minibrot's period means
summary: Why the tiny copies are called "period 3", "period 128" or "period 2688", and why the deep ones have such long loops.
place: airplane
topics: period
---

## Points that loop
center: -0.6, 0
span: 2.6
look: aurora
interior: period
orbit: on
orbit-length: 200
probe: -0.2, 0
marker: off

The rule: start at 0, square the number, add c, repeat. Inside the set the numbers never run away; they settle into a loop that repeats for ever.

The ringed dot is **c = −0.2**. Its steps close in on **one point, −0.17**, and stay there: a loop of length 1. The length of the loop is the point's **period**. Every c in the big heart (the main cardioid) has period 1, so the period colouring paints it all one colour.

## Two steps round
probe: -1, 0

In the round disc on the left every c has **period 2**. At its centre, **c = −1**, the numbers go 0, −1, 0, −1, …: back at zero after exactly two steps.

Every bulb has such a centre, its *nucleus*: start there, and after *period* steps the number is exactly 0 again.

## Count the branches
center: -0.12, 0.855
span: 0.46
probe: -0.12256116687665362, 0.74486176661974424
marker: -0.10109636384562, 0.95628651080914
marker-label: three branches

The bulb at the top has **period 3**: from its nucleus the orbit visits three points (one of them 0) and starts again.

You can read the period off the picture: where the bulb's antenna forks (the gold ring), count the branches, the one back to the bulb included. Three branches, period 3. Bulbs further round the heart have 4, 5, 6, … branches, and that period.

## A copy of the whole set
center: -1.762, 0
span: 0.04
probe: -1.7548776662466927, 0
marker: off

Far out on the spike sits a tiny copy of the whole set: a **minibrot**. Every c in its heart settles into the same loop length, and that is the minibrot's period: **3**. Its nucleus, c = −1.7549, goes 0, −1.755, 1.325, 0, …

Its own bulbs copy the whole set's, multiplied by 3: the disc on its left has period 6.

## Press N: period 7
center: -1.674312445949552832839810755913428373148, 0
span: 0.00125
probe: -1.674066091474787971565296029172325596206, 0

From the copy above, **Seek a minibrot** (**N**) goes on to the next smaller copy around it: this one, **period 7**, 40 times smaller.

From its nucleus the orbit takes **seven steps** to come back to 0. On the map its dots land on seven spots.

## Deeper, longer
center: -1.668339878887826301830926098299792284187, 2.315710200002254669909918285244744398221e-9
span: 4.8e-11
probe: -1.668339878897302863730079449910646071093, 2.315625838208075433203679764357113475186e-9

Press **N** again and again and the copies get smaller and their loops longer: periods **16, 48, 80**, and here, about 6·10¹⁰ times deeper than the whole set, **128**.

The orbit of this nucleus visits 128 points all over the set (the map shows them) before it lands exactly on 0 again.

## Period 2688
center: -1.668339878870128466154007021521551527867, 2.316257816636023822388578982325180068007e-9
span: 6.3e-16
iterations: 300000
orbit: off
probe: pointer

Four presses on (periods 384, 640, 896), at a zoom of 5·10¹⁵: a **period-2688 minibrot**. Every c in its heart settles into a loop of 2688 steps; from its nucleus the number comes back to exactly 0 after step 2688, not one step sooner. The disc beside it has period 5376, twice as long.

That is all "period-2600 minibrot" means: the length of the loop. Points here go round it many times before they are proven inside, so this step raises the iteration limit to 300 000 and the picture takes a few seconds.

## Why deeper means longer

A copy is small because the rule stretches. Points near a minibrot follow its loop, and every step pulls neighbouring numbers further apart. Over one round the stretches multiply; the more they multiply, the smaller the copy.

One step stretches by at most about 4 times, so a very small copy needs many steps: a long loop. Each digit of zoom costs almost one step at the least; here, near the spike, periods grow much faster (2688 at only 16 digits).

## Your turn
center: -0.6, 0
span: 2.6
orbit: on
iterations: auto
probe: pointer

The orbit follows the pointer again (on a touch screen: hold a finger and choose *Orbit Here*). Point into the bulbs and copies: the line under the map says *settles into a period-… cycle*.

**N** always picks the copy with the **lowest period** in or near the view, the biggest and simplest one, and each further press a longer loop. **End tour** (or **Esc**) brings back your view and settings.
