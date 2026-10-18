# Autopilot, journeys & places

## Driving levels

Like a car, Almondbread has four ways to drive, shown at the bottom of the picture as **M · C · A · J**: **M**anual, **C**ruise, **A**utopilot and **J**ourney. Next to them a thin gauge shows the speed: zooming out to the left, zooming in to the right, with the value, for example *↓ ×2/s* (out, twice as wide every second) or *↑ ×1.5/s* (in).

## Cruise

**E** and **Q** are pedals. From rest, **E** starts a steady zoom in and **Q** a zoom out; each press is one step faster, and holding the key keeps speeding up. The other pedal slows down step by step and stops at zero: to go the other way, let go and press again. **Esc** brakes gently to a stop.

You always steer yourself: point with the mouse where you want to go, or use **W A S D**; on iPhone and iPad rest a finger where you want to go, or swipe to move the picture. Nothing pulls you anywhere.

There are 26 gears, from ×1.1 to ×1000 per second; the cockpit shows the one you are in, for example *gear 5* zooming in or *reverse 3* zooming out.

Zooming out gets faster the deeper you are (*boost*), more so in the higher gears: the low gears stay gentle (gear 1 takes about two minutes home from a zoom of ten billion), while gear 10 (×5) brings you home from the deepest views in about a minute and the top gear in about ten seconds. The speed builds up over a few seconds, and near the whole set it slows down evenly and comes to rest exactly on it.

**Lane assist** (**L**, or the control center) keeps a cruise out of featureless areas with a gentle nudge; your own steering always wins. The lane icon in the gauge lights up while it is on.

## Autopilot

Press **Z** (or click the plane in the cockpit at the bottom) and the view keeps zooming on its own, heading for interesting places: spirals, branch points and tiny copies of the whole set.

- **E** and **Q**, the mouse wheel or a two-finger scroll change the speed.
- **Click** (or tap) pauses and resumes.
- **Point with the mouse** to suggest a direction: move the pointer away from the middle, for example to the right, and the autopilot looks for its next spiral, minibrot or detail on that side. It still picks the places itself and does not simply follow the pointer; the status overlay then says *your direction*. Back in the middle, it chooses freely again.
- On iPhone and iPad **rest a finger** to suggest a direction (like the mouse) and **swipe** to move the picture: the autopilot stays on and picks its next place from where you moved it.
- Steer yourself (**W A S D**, drag with the mouse, a game controller's stick) and you take the wheel: the view cruises on at the same speed. The autopilot does not take over again by itself; press **Z** for that. **Q** down to zero also hands over to cruise, which then stops.
- The **target marker**, a small ring, shows where the autopilot is heading and names what it found there for a moment (for example *spiral* or *minibrot · period 34*); a faint line from the middle shows the direction you suggested. Turn it off in View ▸ Autopilot Target Marker or the control center's **Overlays**.
- **Esc** stops.

When the picture ahead gets plain, or the computer needs time to keep up, the autopilot slows down by itself; the status overlay says why, and the cockpit shows the speed you set as *target* until it is back up to it.

## Journeys

Press **J** (or click the sparkles in the cockpit) for a journey: an endless flight from landmark to landmark. Almondbread finds the landmarks in the maths itself: tiny copies of the whole set, spirals, and the patterns on the way to them. Each leg flies to one, rests a moment so the view can sharpen, and flies on.

- Steering yourself, or **Q** down to zero, takes the wheel: the view cruises on (or stops) and the journey pauses. Other input pauses it too; **J** or a click resumes. **E** and **Q** change its speed.
- **Esc** stops.
- **Stop at each find** (control center, Journey): the journey waits at each landmark, so the picture can finish. The status overlay names it, for example *Minibrot, period 34*. Then **J** flies on as planned, **Z** goes deeper here, **X** goes somewhere else, and **Esc** ends the journey. Look around in the meantime as you like; if you wander far off, the journey continues from where you are. On iPhone and iPad, use the **Next**, **Deeper here** and **Elsewhere** buttons under the status overlay.
- **J** starts a journey in the mode set in the control center, **Shift + J** in the other one.
- Every journey has a number, its seed. The same seed from the same start gives the same journey, so you can share it. The **daily journey** is the same for everyone on the same day.

## Seek a minibrot

Press **N** (or View ▸ Seek Minibrot; on iPhone and iPad, hold a finger on the picture and choose *Seek Minibrot Here*, or *Seek minibrot* in the control center) to find the simplest tiny copy of the whole set in or near the view, the one with the lowest period, and fly there until it fills the picture. The status overlay names it, for example *Minibrot, period 4640 · 1.0e69×*. Press **N** again and it looks around that one for the next, smaller copy, and so on down. What the period means: the tour *What a minibrot's period means* (**View ▸ Tours**).

- Finding one usually takes a moment; meanwhile the overlay says *Seeking a minibrot…*. **Esc** or any other key or move cancels the search.
- The flight is an ordinary flight: **E** and **Q** change its speed, **Esc** stops it, and steering yourself takes over.
- If there is none close by, the overlay says so and nothing moves.

## Places

The control center starts with **Places**: famous views, from the seahorse valley to views deeper than a googol. **Go** jumps there, **Fly** flies there, **Random** picks one for you.

## Tours

A tour walks you through one piece of the maths, step by step. Start one from **View ▸ Tours**, the **Tours** section of the control center, or the **Tour** button of a place that has one (for example the *Feigenbaum Point*).

- A card at the bottom right explains each step. The tour flies the view there, sets the colours and overlays, and moves the orbit's point for you, so you can watch what the text describes.
- **Next ›** or **→** goes on, **‹ Back** or **←** goes back, **End tour** or **Esc** ends it.
- Look around in between as you like; **Next** carries on from the tour's own next view.
- When the tour ends, your view and settings come back as they were.
- The **walkthrough** is a tour of the app itself: the cockpit, the throttle, the buttons and the control center, each ringed in gold while the card explains it. It sits at the top of the Tours list and in the Help menu.

## Fly to coordinates

Every view has an address: its centre and its zoom. **⌘C** copies it, **⌘V** jumps to a copied one (also **⇧⌘C** / **⇧⌘V** while a text field has focus), **⇧⌘G** flies to it, and **⌘G** flies to an address you type.
