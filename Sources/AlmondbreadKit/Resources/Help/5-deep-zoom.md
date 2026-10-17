# Deep zoom & precision

The Mandelbrot set never runs out of detail, but numbers do. Ordinary computer numbers keep about 16 digits, enough to zoom about 10¹⁵ times. Past that, neighbouring pixels would get the same number and the picture would fall apart into blocks.

Almondbread goes much deeper. It follows one point with as many digits as the zoom needs, and works out every other pixel as a small difference from it, which the graphics chip does quickly. Shortcuts then skip many steps at once where the numbers behave simply.

That is how views at 10¹⁰⁰ (a googol) or even 10¹⁰⁰⁰ still draw in seconds. The limit is around 10³⁰⁰⁰: there the zoom stops and the status overlay says so.

Deep views need more steps per point, so they take longer. The blue line at the top shows how far a slow view has come.
