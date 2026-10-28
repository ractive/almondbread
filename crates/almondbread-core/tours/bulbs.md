---
id: bulbs
title: Bulbs and spokes
summary: Every bulb on the heart has a period you can read off its antenna, and the bulbs line up like fractions.
place: third-bulb
---

## One place
center: -0.45, 0
span: 2.6
look: aurora
interior: period
orbit: on
orbit-length: 200
probe: -0.2, 0
marker: off
labels: 0.05, 0.35, the heart

A reminder of the formula: z starts at 0, and at each step z → z² + c (square z, then add c). The small circle (the marker) is the point c = −0.2. The white dots are the values of z, one per step.

With c = −0.2, z goes 0, −0.2, −0.16, −0.174, −0.170, … The values come closer and closer to **one place**, −0.171, and then stay there. That is why the white dots make one small group right next to the marker. Every c in the heart (labelled), the big shape with the notch on the right, does the same: its dots end at one place.

## Bulbs and loops
probe: -1, 0
labels: -1, 0.15, a bulb; -0.122, 0.8, a bulb; 0, 0, 0

Around the heart there are round **bulbs** of every size. Two of them are labelled. The marker is now at c = −1, in the middle of the big bulb on the left. With c = −1, z goes 0, −1, 0, −1, … So the dots move between **two places**, 0 and −1, again and again. You can see them at the two ends of the white line: at the marker and at the label 0. This is a **loop** of 2.

For every c in a bulb, the dots end up in a loop of the same length. This length is the bulb's **period**. Inside the set, the colour changes with the period. The colours repeat, so they do not tell you the period.

## Round the heart
probe-path: -1, 0 -> -0.6224, 0.4249 -> -0.5043, 0.5628 -> -0.3591, 0.6174 -> -0.1226, 0.7449 -> 0.1212, 0.6106 -> 0.2823, 0.5301 -> 0.3795, 0.3349 -> 0.3890, 0.2159
probe-seconds: 48

Now the marker moves from bulb to bulb round the top of the heart, through the middle of each bulb, and then back. In each bulb, count the places the white dots move round: **2** in the big bulb on the left, then **7, 5, 8, 3, 7, 4, 5, 6**.

Between two bulbs the marker crosses the heart, where the dots go to 1 place. Or it is outside the set for a moment: then z keeps growing, and the white lines go far out of the view.

## Count the branches
center: 0.32, 0.57
span: 0.15
probe: 0.28227139076691388, 0.5300606175785253
marker: 0.36636298342276435, 0.59153377326144522
marker-label: first fork

The marker is in the middle of the big round bulb at the bottom of the view. This bulb has **period 4**: its dots move round four places. Only one of them is in this view; the white lines lead to the others. The mini-map at the bottom left shows all four.

Now look at the thin, branching line that grows from the bulb towards the top right. It is called the bulb's **antenna**. Follow it to the labelled point "first fork", where it splits for the first time. Count the branches that meet there, the one back to the bulb included: **four**.

## Five
center: 0.40, 0.355
span: 0.07
probe: 0.37951358801592375, 0.33493230559749759
marker: 0.43792424135946278, 0.34189208433811611
marker-label: first fork

This is the next bulb down along the edge of the heart: the big round bulb on the left, with the marker in it. Its dots move round **five** places. At the labelled point "first fork" on the right, its antenna splits into **five** branches.

This works for every bulb on the heart: count the branches at the first fork, and you know the period without any calculation.

## Halfway round
center: -0.35, 0.3
span: 1.5
probe: -1, 0
marker: -0.75, 0
marker-label: halfway round

There is another way to know the period of a bulb: its place on the edge of the heart. Start at the dent on the right side of the heart. Go along the edge, over the top. **Halfway** round, you reach the labelled point "halfway round". There the big round bulb on the left touches the heart.

The marker is in this bulb. Its dots move between **2** places: period 2.

## A third of the way
probe: -0.12256116687665362, 0.74486176661974424
marker: -0.125, 0.6495
marker-label: a third of the way

Start again at the dent. Only a **third** of the way round, at the labelled point, the big round bulb on top touches the heart. The marker is in this bulb, and its dots move round **3** places: period 3. The white lines make a triangle.

## A quarter of the way
probe: 0.28227139076691388, 0.5300606175785253
marker: 0.25, 0.5
marker-label: a quarter of the way
labels: 0.3795, 0.3349, a fifth: period 5

A **quarter** of the way round, at the labelled point, sits the period-4 bulb from before, now small at the top right. The marker is in it, and its dots move round **4** places: period 4.

Write these places as fractions: halfway is ½, a third is ⅓, a quarter is ¼. The **bottom number** of the fraction is the period. A fifth (⅕) of the way round sits the period-5 bulb (labelled), a little further down on the right.

And the higher the period, the smaller the bulb.

## Two and three
center: -0.55, 0.35
span: 1.1
probe-path: -1, 0 -> -0.7, 0 -> -0.3, 0.3 -> -0.125, 0.55 -> -0.12256116687665362, 0.74486176661974424
probe-seconds: 16
marker: off
labels: -1, 0.15, period 2; -0.122, 0.8, period 3

Now the marker moves from the middle of the period-2 bulb on the left, through the heart, to the middle of the period-3 bulb on top, and then back. Count the places of the white dots: **2** in the bulb on the left, then 1 in the heart, then **3** in the bulb on top.

## Between them
center: -0.36, 0.6
span: 0.45
probe: -0.504340175446244, 0.56276576145298196
marker: -0.504340175446244, 0.56276576145298196
marker-label: 2 + 3 = 5
labels: -0.12256116687665362, 0.74486176661974424, period 3

Which is the biggest bulb **between** the period-2 bulb and the period-3 bulb? It is the bulb with the label "2 + 3 = 5". The period-3 bulb (labelled) is the big round bulb at the top right. The period-2 bulb is outside the view, further down on the left.

The marker is in the labelled bulb, and its dots move round **5** places. And 2 + 3 = **5**! Between a period-2 bulb and a period-3 bulb, the biggest bulb has period 2 + 3.

## And again
probe: -0.35910239011244916, 0.61735345339882686
marker: -0.35910239011244916, 0.61735345339882686
marker-label: 5 + 3 = 8
labels: -0.504340175446244, 0.56276576145298196, period 5; -0.12256116687665362, 0.74486176661974424, period 3

Again: between the period-5 bulb on the left and the period-3 bulb at the top right (both labelled), the biggest bulb is this small one with the label "5 + 3 = 8". Its dots move round **8** places; the text above the mini-map gives the count. And 5 + 3 = **8**. This works again and again, in every gap between two bulbs.

Mathematicians write the places round the heart as fractions. To find the fraction of the bulb between two others, add the top numbers and add the bottom numbers: ½ and ⅓ give ⅖, and ⅖ and ⅓ give ⅜. The bottom number is always the period. Numberphile has more on this way of adding fractions in [Funny Fractions and Ford Circles](https://www.youtube.com/watch?v=0hlvhQZIOQw).

## Thirteen
center: -0.395, 0.592
span: 0.08
probe: -0.4071040830850983, 0.5848528428681022
marker: -0.4071040830850983, 0.5848528428681022
marker-label: 8 + 5 = 13
labels: -0.35910239011244916, 0.61735345339882686, period 8

Go on with the last two bulbs. The period-8 bulb (labelled) is now the big round bulb at the top right. The period-5 bulb is outside the view, on the left. The biggest bulb between them has the label "8 + 5 = 13": its dots move round **13** places, and 8 + 5 = **13**.

## Fibonacci
center: -0.392, 0.589
span: 0.03
probe: -0.3861604162801374, 0.5914044863955293
marker: -0.3861604162801374, 0.5914044863955293
marker-label: 13 + 8 = 21
labels: -0.4071040830850983, 0.5848528428681022, period 13

The period-13 bulb (labelled) is now the big round bulb on the left. Next to it is the bulb with the label "13 + 8 = 21": period **21**. Each period is the sum of the two before it: 2, 3, 5, 8, 13, 21, 34, … These are the **Fibonacci numbers**.

Their fractions ½, ⅓, ⅖, ⅜, 5⁄13, 8⁄21, … come closer and closer to one place on the edge of the heart, 0.382 of the way round. That number is 1/φ², where φ = 1.618… is the **golden ratio**. Numberphile shows more in [Fibonacci Numbers hidden in the Mandelbrot Set](https://www.youtube.com/watch?v=4LQvjSf6SSw).

## Your turn
center: -0.45, 0
span: 2.6
probe: pointer
marker: off

::: mac
The marker and its dots follow the pointer again. Point into any bulb and count the places its dots move round. Then zoom in on its antenna and count the branches at the first fork.

**End tour** (or **Esc** twice) brings back your view and settings.
:::
::: touch
Now you move the marker: hold a finger on the picture, choose *Orbit Here*, and keep the finger down to drag the marker. Put it into any bulb and count the places its dots move round. Then zoom in on its antenna and count the branches at the first fork.

**End tour** brings back your view and settings.
:::
