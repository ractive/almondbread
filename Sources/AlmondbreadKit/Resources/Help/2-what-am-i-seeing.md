# What am I seeing?

## The rule

Every point of the picture stands for a number, called *c*. Almondbread starts at zero and repeats one step: square the number, then add *c*. For some points the numbers stay small forever. For others they grow and fly off to infinity.

## Black: inside the set

Black points never fly off: they belong to the Mandelbrot set. Almondbread paints a point black only once it has proven that the point stays, so black really means inside.

The inside can be coloured too. The control center's **Interior** section shows hidden patterns there: bubbles, how long each repeating loop is, or stripes along the bulbs.

## Colours: how fast points escape

Every coloured point escapes. The colour tells how many steps it took: points far from the set escape after a few steps, points close to its edge take thousands or millions. The palette turns those counts into bands of colour, which is why the bands follow the shape of the edge.

## Fog: not finished yet

Fog means "not decided yet". Those points have neither escaped nor been proven inside within the steps tried so far. They sit close to the edge, often near the narrow pinches between bulbs, where points take a very long time to decide. The fog is lit by the colours around it where it meets finished points and turns almost black further in, with a faint blue-grey tint, so a large patch of it still looks different from the black inside.

What to do: wait a moment while the view refines, or raise the **upper limit** in **Iterations & quality**. With manual iterations, raise the maximum.

## Iterations and the progress line

The number of steps tried per point is the iteration limit. With **Automatic iterations** on, Almondbread raises it by itself as you zoom deeper, up to the upper limit you choose. With automatic off, the limit stays where you set it.

When a view takes a while, a thin blue line runs along the top edge, like a loading bar in a browser, and the status overlay at the top left says what is happening and roughly how long it will take.

## Sharpening

While you move, Almondbread draws as much as fits into each frame, so the picture can look soft for a moment. When you stop, it finishes the view and smooths the edges.
