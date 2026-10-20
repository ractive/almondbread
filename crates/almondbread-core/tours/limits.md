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

Every point gets the same test: start at 0, square the number, add the point, repeat. If the numbers grow and fly off, the point **escapes**: it is outside the set. The ringed point is being tested; its dots are the numbers, one per step.

The ring sits just above the **neck**, where the round bulb on the left meets the big heart, only 0.001 away. Its dots crawl round and round before they fly off: about **3 150 steps** (the line under the map counts them). The closer to the neck, the slower.

## Still undecided
probe: -0.75, 0.00001

Here it is: after **100 000 steps** the dots still have not flown off, and the line under the map says *still undecided*. The point does escape, after about 314 000 steps, but nothing tells in advance.

That is why some views take long, why the blue line at the top runs for a while, and why the **upper limit** in *Iterations & quality* matters.

## Proven inside
probe: -0.74, 0

Black has to be earned too. This point is just inside the heart: its dots close in on **one spot**, but slowly, and it takes about two thousand steps before Almondbread can **prove** they stay.

Only then does a point turn black. A point that is merely "still small" stays fog.

## Fog
center: -0.74999996120361811, 0.00036918279150454967
span: 1.5e-8
orbit: off
map: off
probe: pointer

Now a whole view of slow points: the narrow gap just above the neck, 100 million times magnified. The grey patches are **fog**: "not decided yet".

They are not inside the set. Those points do escape, but only after **20 to 70 million** steps, far beyond the 200 000 Almondbread tries by default. Fog is work left over, not part of the picture.

## Computing further

Almondbread does not give up at once. Set to **automatic**, it picks a number of steps (*iterations*) for each view, then keeps going on just the undecided points, in bigger and bigger rounds, up to the **upper limit**. The status overlay says how much is still open.

To clear fog like this, raise the upper limit in **Iterations & quality**, and be patient: here a single picture would need about a million million steps.

## Sixteen digits
place: seahorse-minibrot-1e31

Now the other limit. Ordinary computer numbers keep about **16 digits**: enough to magnify about 10¹⁵ times (a 1 with 15 zeros). Deeper, neighbouring pixels would get the same number and the picture would fall apart into blocks.

In the middle sits a **minibrot**, a tiny copy of the whole set, magnified **10³¹** times: a 1 with 31 zeros. Its centre needs 31 digits just to be written down, and more to be followed.

## One precise point

Almondbread follows **one point** with all the digits the view needs: the *reference*. Every other pixel is worked out as a small **difference** from it.

Small differences fit in short, fast numbers, which the graphics chip handles by the million. If a difference grows too big, the pixel jumps back to an earlier spot on the reference's path, close to its own, and carries on.

## Skipping ahead
place: needle-minibrot-1e302
move: jump

A minibrot magnified **10³⁰²** times, on the spike, the thin line on the left of the set. Its points need thousands of steps each.

Where the numbers behave simply, one step looks like the next, so Almondbread **skips** many of them at once with a short formula (a *linear approximation*). In this view it does about 50 steps in one go on average, and the picture still draws in moments.

## Three thousand digits
place: swirl-1e3000
move: jump

This swirl is magnified **10³⁰⁰⁰** times: its centre needs more than 3 000 digits. The same tricks still work: one precise point, small differences, and skipping ahead, here about 60 steps in one go, and the picture still draws in moments.

Almondbread stops a little deeper, at about **10³⁰⁰⁷**. The place called *Rock Bottom* is right there.

## Your turn
place: seahorse-valley
move: jump

Back in the seahorse valley. **Iterations & quality** in the control center has the automatic limit, its **upper limit** for slow views, and a manual maximum.

**End tour** (or **Esc**) brings back your view and settings.
