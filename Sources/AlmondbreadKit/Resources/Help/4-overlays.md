# Overlays

Overlays are drawn on top of the picture. Turn them on in the control center's **Overlays** section, in the toolbar (the round buttons on iPhone and iPad; the Julia set has its button on iPad only), or with their keys. Each panel has a small **?** in its corner that explains it.

## Cosmic size scale (C)

The cosmic scale turns the zoom into sizes you know. Imagine your window stays its real size (30 cm when the screen does not say) while the whole set grows as you zoom in.

- First the set grows past things you know: a few kilometres across, as big as Earth, as big as the Sun.
- Then it becomes as big as the observable universe.
- Deeper, the whole Mandelbrot set would be as big as an imaginary universe in which a planet, then a cell, then an atom is as big as ours.
- Deeper still, the imaginary universes nest: in each, the Planck length (the smallest length physics knows) is as big as the universe below. The scale counts the levels and says what is as big as our universe at the bottom.

The ruler shows what you are passing; a gold flash marks each one.

## π meter

The number π hides in the Mandelbrot set. Near two special points, the pinch between the two biggest bulbs (−0.75) and the cusp at the right (0.25), points take a very long time to escape. Multiply the number of steps by the distance from the special point and you get π, with more correct digits the closer you go. (At the cusp, use the square root of the distance.)

The π meter counts for you and shows in gold which digits already agree. It belongs to the tour **π in the Mandelbrot set** (**View ▸ Tours**), which slides a probe closer step by step; the meter switches off when the tour ends.

## Mini-map (M)

The mini-map shows the whole set and up to three zoomed-in levels, so you always know where you are. A gold rectangle, or a crosshair once it gets too small, marks where the next level or your view sits. **Click a map** to jump there at your current zoom; **⌥-click** flies there. On iPhone and iPad: **tap** a map to jump, **hold** it to fly there. The **arrows button** below the map's **?** makes the maps large, and small again.

## Orbit (O)

The orbit overlay shows the numbers the rule makes for the point under the pointer: a ring at the point itself, then each step as a dot joined by a line. Inside the set the steps settle into a loop; outside they wander off. The line under the map says what happens to the point: it escapes after so many steps, settles into a cycle of some period, or is still undecided after 100 000 steps.

On iPhone and iPad there is no pointer: the orbit button shows the orbit of the point in the middle of the picture, or of the last point you picked. Hold a finger on the picture and choose **Orbit Here** to pick another point; keep the finger down and drag to move it.

Near the set's edge a point can take tens of thousands of steps to decide. The whole-set map shows them all: the first 50 steps as a line, the rest as a cloud of fine dots, older ones faint and the newest bright, which grows while the point is worked out.

## Julia set (U)

Every point c of the Mandelbrot set has a picture of its own. The Mandelbrot set always starts the rule at 0 and changes c from pixel to pixel; turn it around, keep one c and start from every number z instead. The starts whose numbers stay small form the **Julia set** of c (named after Gaston Julia), filled in with the inside's colour; the others take the palette's colours by how fast they escape.

The Julia panel sits next to the mini-map and shows the Julia set of the orbit's point: the point under the pointer, or on iPhone and iPad the middle of the picture or the point you chose with **Orbit Here**. Move the pointer and watch it change:

- Inside the Mandelbrot set the Julia set is **one connected piece**: the basilica at −1, the Douady rabbit in the top bulb.
- Outside it falls apart into **dust**: specks with no two joined.
- On the edge it is a thin tree of branches, like the set's own filaments.

That is what the black of the Mandelbrot set means: it is exactly the points whose Julia sets are in one piece. The panel shows the whole Julia set; click or tap it to open that set as the main view and zoom into it. On the way to the panel the pointer crosses other points; to open the Julia set of the point you are pointing at, **⌘-click** it instead (on a Mac, or an iPad with a keyboard and a trackpad or mouse): exactly that point, also with the panel off. On a small screen it may take the place of the deeper map levels.

In a Julia view the panel turns into the **Mandelbrot panel**: the Mandelbrot view you came from, with a crosshair at the point c whose Julia set you are looking at. Click or tap it to go back there (or press **⇧U**). It shows in every Julia view, also when the Julia panel was off; **U** hides it.

## Compass

When the picture is turned, a compass shows where north is. Click it to turn the picture upright, or drag it to turn.

## Target marker

While the autopilot flies, or a journey flies to its next landmark, a small ring marks where it is heading. When it has found something with a name, a short label shows next to the ring for a moment, for example *spiral* or *minibrot · period 34*. When you suggest a direction with the mouse during the autopilot, a faint line from the middle points that way, so you can see whether the target agrees. Nothing shows while you drive yourself. Turn it on or off in View ▸ Autopilot Target Marker or in the control center's **Overlays**; it is on at first.

## Status overlay

The status overlay at the top left shows the zoom, the centre and the speed, and how far a long render has come. Its developer details (in **Overlays**) add numbers for the curious.
