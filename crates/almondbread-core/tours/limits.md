---
id: limits
title: Why some points are hard
summary: Slow points, fog, proven black, and the limits of computer numbers, and how Almondbread gets past them.
place: seahorse-minibrot-1e31
topics: fog, iterations
---

## A very slow point
center: -0.75, 0.05
span: 0.3
look: aurora
interior: flat
iterations: auto
orbit: on
map: on
probe: -0.75, 0.001

Remember: every point of the picture is a number c, and z → z² + c is used again and again from z = 0. If z keeps growing, c **escapes**: it is outside the set. The small white circle (the **marker**) shows the c we test now. The white dots are its values of z, one per step.

The marker is just above the **neck**: the narrow place where the big round part of the set (the black area on the left) touches the **heart**, the big heart-shaped part (the black area on the right). The marker is only 0.001 away from the neck.

Its white dots crowd together on a line to the right of the marker: z changes very slowly. The line above the mini-map, at the bottom left, counts the steps: *escapes after 3 147 steps*. The closer c is to the neck, the more steps z needs.

## Still undecided
probe: -0.75, 0.00001

Now the marker is even closer to the neck: only 0.00001 away. The line above the mini-map first counts the steps (*following the orbit*). After **100 000 steps** z has still not escaped, and the line says *still undecided after 100 000 steps*. This c does escape, after about 314 000 steps, but nothing tells you that in advance.

That is why some views take a long time, and why the **upper limit** in *Iterations & quality* matters.

## Proven inside
probe: -0.74, 0

Black is also not painted at once. The marker is now just inside the heart. Its dots come closer and closer to **one place**, but slowly. It takes about two thousand steps before Almondbread can **prove** that z stays small. Then the line above the mini-map says *settles on a fixed point*: a loop of one place.

Only then does Almondbread paint the point black. If z is still small after all the steps, but not proven to stay small, the point is not painted black. It is painted grey: the next card shows this grey.

## Fog
center: -0.74999996120361811, 0.00036918279150454967
span: 1.5e-8
orbit: off
map: off
probe: pointer

Now a whole view of slow points: the narrow gap just above the neck, magnified about 230 million times (the box at the top left says 2.3e8×). The dark grey areas are **fog**. Fog means "not decided yet".

These points are not in the set. Their z does escape, but only after **20 to 70 million** steps. That is far more than the 200 000 steps Almondbread tries by default. The box at the top left says *computing further*: Almondbread is still working on these points. Fog is work that is not finished. It is not part of the shape of the set.

## Computing further

Almondbread does not stop after the first try. The number of steps it tries is called the **iterations**. When iterations are set to *automatic*, it picks a number of steps for each view. Then it goes on with only the undecided points, in bigger and bigger rounds, up to the **upper limit**. The box at the top left says how much is not finished yet, and the thin blue line at the top of the view shows the progress.

To clear fog like this, raise the upper limit in **Iterations & quality**, and wait. For this view, all the pixels together would need about 10¹² steps (a 1 with 12 zeros).

## Sixteen digits
place: seahorse-minibrot-1e31

There is a second limit. Ordinary computer numbers keep about **16 digits**. That is enough to magnify about 10¹⁵ times (a 1 with 15 zeros). Deeper than that, neighbouring pixels would get the same number, and the picture would break up into blocks.

This view is magnified about **10³¹** times: a 1 with 31 zeros (the box at the top left says 1.7e31×). The small dark spot in the middle is a **minibrot**, a small copy of the whole set. At this size you cannot see its shape. Its centre needs 31 digits to write down, and more digits to work out its steps.

## One precise point

So Almondbread works out the steps of **one point** with all the digits the view needs. This point is called the *reference*. For every other pixel, it works out only the small **difference** from the reference.

A small difference needs only a few digits. Numbers with few digits are fast, and the graphics chip works on millions of them at once. If a difference grows too big, the pixel goes back to an earlier step of the reference, at a place close to its own, and continues from there.

## Skipping ahead
place: needle-minibrot-1e302
move: jump

This minibrot is magnified **10³⁰²** times. It is on the **spike**, the thin line on the left of the set. Every pixel in this view needs thousands of steps.

Where z changes in a simple way, one step looks like the next. There Almondbread **skips** many steps at once with a short formula (a *linear approximation*). In this view it does about 50 steps at once on average, and the picture still appears quickly.

## Three thousand digits
place: swirl-1e3000
move: jump

This swirl is magnified **10³⁰⁰⁰** times: its centre needs more than 3 000 digits. The same methods still work: one precise point, small differences, and skipping ahead (here about 60 steps at once). The picture still appears quickly.

Almondbread stops a little deeper, at about **10³⁰⁰⁷**. The place called *Rock Bottom* is there.

## Your turn
place: seahorse-valley
move: jump

Back in seahorse valley. The section **Iterations & quality** in the control center has the automatic limit, its **upper limit** for slow views, and a fixed maximum that you set yourself.

**End tour** (or **Esc** twice) brings back your view and settings.
