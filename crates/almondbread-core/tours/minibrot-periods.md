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

Remember: every point of the picture is a number c, and z → z² + c is used again and again from z = 0. The small white circle (the **marker**) is at c = −0.2. Each white dot is the value of z after one more step.

Inside the set, the dots end up going round a **loop**: they visit the same few places again and again. The number of places in the loop is the **period** of c.

Here z goes 0, −0.2, −0.16, −0.1744, −0.1696, −0.1712, … It comes closer and closer to **one place**, about −0.171, just to the right of the marker. That is a loop of 1 place: period 1. The line above the mini-map calls this *settles on a fixed point*.

Every point in the **heart**, the big heart-shaped part of the set, has period 1. So the whole heart has one colour.

## Two steps round
probe: -1, 0

The round **bulbs** on the heart have longer loops. In the big bulb on the left, every point has **period 2**. The marker is now at the middle of this bulb, at c = −1.

Here z goes 0, −1, 0, −1, …, because (−1)² − 1 = 0 and 0² − 1 = −1. The dots move between **two places**, 0 and −1, and z is back at 0 after exactly two steps.

Every bulb has such a middle point, called its **nucleus**. If c is the nucleus, z is exactly 0 again after as many steps as the period.

## Count the branches
center: -0.12, 0.855
span: 0.46
probe: -0.12256116687665362, 0.74486176661974424
marker: -0.10109636384562, 0.95628651080914
marker-label: first fork

The big round bulb in the lower half of this view is the bulb on top of the heart (the edge of the heart is at the very bottom). This bulb has **period 3**. The marker (the small white circle) is at its nucleus. From there, z visits **three places**, one of them 0, and then starts again.

You can also see the period in the shape. The thin line above the bulb is its **antenna**. It first splits at the gold circle labelled *first fork*. Count the branches there, including the one that goes back down to the bulb: there are three. Three branches, period 3. Bulbs further round the heart have 4, 5, 6, … branches at their first fork. A bulb with 4 branches has period 4, with 5 branches period 5, and so on.

## A copy of the whole set
center: -1.762, 0
span: 0.04
probe: -1.7548776662466927, 0
marker: off

Far out on the **spike**, the thin line on the left of the set, there is a small copy of the whole set: a **minibrot**. Every point in its heart has the same period, the period of the minibrot: **3**.

The marker is at its nucleus, in the heart of the minibrot. There z goes 0, −1.755, 1.325, 0, …: three places. The other two places are outside this view: the white line from the marker leads to them. On the mini-map at the bottom left, this line runs along the middle of the whole set.

The bulbs of the minibrot copy the bulbs of the whole set, with loops 3 times as long: the big round bulb on the left of its heart has period 6.

## Press N: period 7
center: -1.674312445949552832839810755913428373148, 0
span: 0.00125
probe: -1.674066091474787971565296029172325596206, 0

From the minibrot of the last card, **Seek a minibrot** (**N**) goes on to the next smaller copy near it. That is this minibrot: it has **period 7**. The view is magnified 32 times more than before (the box at the top left says 2700× instead of 84×).

The marker is at its nucleus. Its z takes **seven steps** to come back to 0. On the mini-map the dots land on seven places.

## Deeper, longer
center: -1.668339878887826301830926098299792284187, 2.315710200002254669909918285244744398221e-9
span: 4.8e-11
probe: -1.668339878897302863730079449910646071093, 2.315625838208075433203679764357113475186e-9

Press **N** again and again, and the copies get smaller and their loops get longer: periods **16, 48, 80**, and here **128**. This view is magnified about 70 billion times (the box at the top left says 7.0e10×).

The marker is at the nucleus of this minibrot, in its heart. From there, z visits 128 places before it is exactly 0 again. All of them lie on the middle line of the set: on the mini-map they form one white line along the middle.

## Period 2688
center: -1.668339878870128466154007021521551527867, 2.316257816636023822388578982325180068007e-9
span: 6.3e-16
iterations: 300000
orbit: off
probe: pointer

Four more presses of **N** (periods 384, 640, 896) bring you here, magnified 5·10¹⁵ times (a 5 with 15 zeros): a **period-2688 minibrot**. Every point in its heart goes round a loop of 2688 places. From its nucleus, z comes back to exactly 0 after step 2688, and not one step earlier. The round bulb to the right of its heart has period 5376, twice as long.

That is all a name like "period-2688 minibrot" means: the length of the loop. Points here go round the loop many times before Almondbread can prove that they are in the set. So this step raises the iteration limit to 300 000, and the picture takes half a minute or more to fill in. The box at the top left shows how long it will still take.

## Why deeper means longer

Why do the small copies have such long loops? Near a minibrot, the dots of a point follow the loop of the minibrot. Each step of the formula moves two nearby values of z further apart, by at most 4 times. Over one round of the loop these factors multiply. The bigger the product, the smaller the copy.

So a very small copy needs many steps: a long loop. Each time a copy is ten times smaller, its loop needs at least about 1.7 more steps. Here, near the spike, the loops grow much faster than that: period 2688 at a magnification of only 5·10¹⁵.

## Your turn
center: -0.6, 0
span: 2.6
orbit: on
iterations: auto
probe: pointer

::: mac
The marker and its dots follow the pointer again.
:::
::: touch
To move the marker and its dots, hold a finger on the picture and choose *Orbit Here*.
:::

Move the marker into the bulbs and the copies. The line above the mini-map then says *settles into a period-… cycle*: a loop of that length.

**N** always picks the copy with the **lowest period** in or near the view: the biggest and simplest one. Each further press picks a longer loop. **End tour** (or **Esc** twice) brings back your view and settings.
