# Per-pixel and image-level speedup and quality techniques for Mandelbrot rendering (excluding perturbation core and BLA)

Target context: Apple Silicon Metal GPU, float32 only, perturbation deep zoom. Research date: 2026-10-14.
Note on sourcing: much of the authoritative material is in Claude Heiland-Allen's (mathr) blog and the KF2+ manual, plus Arnaud Chéritat's technique wiki. Several mathr URLs (Fraktaler 3 docs, an exponential-map blog post) returned 404 during research. Items marked "Inference" are my own reasoning, not sourced claims.

## 1. Interior detection (cardioid/bulb, periodicity, derivative-based, interior DE) and behaviour under perturbation

### Takeaway
The cheapest robust interior test for deep-zoom perturbation on a GPU is the derivative test. Track der = dz/dz along the orbit (der ← 2·z·der, using the full orbit value Z_n + z_n) and stop when |der|² < ε², with ε ≈ 1e-3. It is branch-light, needs no history buffer, and stops maxiter-bound interior pixels early. Cardioid/bulb tests are useless at depth because the view is almost never inside them. Periodicity (cycle) checking needs a tolerance tied to precision, which is fragile in float32. Interior distance estimation (Newton on the cycle) is exact but much more costly, and it breaks near double-precision limits unless perturbation is used.

### Cited Findings
- **Main cardioid test.** Let p = √((x − 1/4)² + y²). The point is inside if x ≤ p − 2p² + 1/4. Square-root-free form: with q = (x − 1/4)² + y², inside if q·(q + (x − 1/4)) ≤ ¼·y². **Period-2 bulb:** (x + 1)² + y² ≤ 1/16. No exact test exists for higher-order bulbs. — [Wikipedia: Plotting algorithms](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **Simple periodicity check.** Keep a saved reference iterate. If the current z ≈ the saved one, declare the point interior and set its count to maxiter. Refresh the saved point every 20 iterations, so periods up to 20 are caught. Tradeoff: the bookkeeping costs a little, the early exit saves a lot. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **Brent's cycle detection** finds the cycle length in one stage with an exponential (power-of-two) search, and needs one function evaluation per step versus three for Floyd's. — [Rosetta Code: Cycle detection](https://RosettaCode.org/wiki/Cycle_detection)
- One public periodicity implementation uses a fixed threshold of 1e-30 on the squared distance, not one derived from pixel size. — [albertlobo.com](https://albertlobo.com/fractals/interior-exterior-distance-bounds-mandelbrot-set)
- FFmpeg's mandelbrot source changed its period-detection seed to come from the left neighbouring pixel instead of the previously computed pixel, because the previous pixel is often farther away. This is a neighbour-coherence heuristic. — [FFmpeg-cvslog 2011](https://lists.ffmpeg.org/pipermail/ffmpeg-cvslog/2011-November/043431.html)
- Wikibooks's "quick and dirty" interior test checks whether |z_n| < eps, with eps set to the numerical precision. It gives no concrete value and does not mention Brent. — [Wikibooks: Mandelbrot set interior](https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/Mandelbrot_set_interior)
- **Derivative-based interior detection (Chéritat).** Track der = (Pⁿ)′ along the orbit, starting from z = c and der = 1. Multiply the derivative before updating z, because the order matters. Each step: if |der| < ε, the point is interior; if |z| > R, it is exterior. Typical ε = 0.001. ε = 0.1 gives errors near parabolic parameters, for example a wrong neck near c = −3/4. Cost is about 2× per iteration, so about 1.5× faster overall. The main benefit is that a very large N no longer slows the image much. Without it, N = 10⁶ took 45 s. — [Chéritat wiki: Mandelbrot set](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set)
- **Multiplier test.** The multiplier of a period-p cycle is d = (f^p)′(z₀), computed as d ← 2·z·d. If |d| ≤ 1 the cycle is attracting and c is interior. A Lyapunov variant averages ln|2z| and treats a positive value as exterior. — [Wikibooks interior](https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/Mandelbrot_set_interior)
- **Neighbour heuristic.** A pixel is probably interior if it is marked interior, all its neighbours are marked interior, and all of them share the same period. — [Wikibooks interior](https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/Mandelbrot_set_interior)
- **mathr's practical interior DE algorithm:**
  1. Each new minimum of |z_n| gives a candidate period p (atom domains).
  2. Newton-solve f^p(z₀) = z₀ with z₀ ← z₀ − (f^p(z₀) − z₀)/((f^p)′(z₀) − 1). Stop at step < 1e-10, with at most 64 Newton steps.
  3. If |d| = |(f^p)′(z₀)| ≤ 1, the point is interior. Compute DE_int = (1 − |d|²) / |∂_c d + (∂²_z f^p · ∂_c f^p)/(1 − d)|.
  - **Variants:** "plain" (no interior checks), "unbiased" (check at every new minimum), and "biased" (if the previous pixel was exterior, defer the checks until maxiter is reached). Biased was best overall, at near-plain speed on exterior-heavy views.
  - Near double-precision limits interior DE fails visibly while exterior DE still works. The fix is extra precision, or perturbation with a reference at the nucleus of the dominant island.
  - [mathr: Practical interior distance rendering (2014)](https://mathr.co.uk/blog/2014-11-02_practical_interior_distance_rendering.html)
- **Interior DE formula and caveats.** b = (1 − |∂_z P^p(z₀)|²) / |∂_c∂_z P^p(z₀) + ∂_z∂_z P^p(z₀)·∂_c P^p(z₀)/(1 − ∂_z P^p(z₀))|. All points within b/4 of c are interior. Caveats: z₀ converges only in theory, and rounding can make the detected period a multiple of the true one (86 instead of 43). The distance is then overestimated and the disk may include exterior points. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **KF2+.** "Interior checking" exists only for power-2 Mandelbrot rendered with NanoMB1/NanoMB2. It "may or may not speed up per-pixel calculations", is labelled experimental, and NanoMB2 disables glitch detection. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- **Fraktaler 3** EXR headers show `bailout.iterations = 262144`, `bailout.maximum_reference_iterations`, `bailout.maximum_perturb_iterations = 4096`, and `bailout.inscape_radius = 0.000976562`. The inscape radius (≈ 2⁻¹⁰) is plausibly the interior-detection threshold, but the page does not explain it. — [Wikibooks: Fraktaler-3](https://en.wikibooks.org/wiki/Fractals/fraktaler-3)

### Inferences
- **Under perturbation the derivative test is cheap and float32-safe.** der_{n+1} = 2·(Z_n + z_n)·der_n. Only the magnitude of der matters, and Z_n + z_n is O(1). Its float32 rounding (≈ 1e-7 relative) barely affects |der|. der itself only shrinks in interior regions, though it can underflow, which is harmless because that already means interior. For exterior pixels |der| grows, so clamp it or stop tracking once it exceeds, say, 1e30.
- A cheap combined test is `if (dot(der,der) < eps2) interior`, with eps2 = 1e-6 (ε = 1e-3, per Chéritat). The cost is one extra complex multiply per iteration (about 30–50% more ALU per iteration). The payoff is very large where interior pixels would otherwise run to maxiter, which is typical near minibrots at depth.
- Brent periodicity comparisons need a tolerance relative to the delta scale in perturbation. Comparing the full orbit Z + z in float32 at 1e-7 resolution cannot resolve periodic points at deep zoom, and comparing the deltas z needs the reference to be periodic too. The derivative test avoids both problems. Treat Brent as a secondary option.
- Prefer a fixed-check-interval form over mathr's "biased" history-dependent strategy on GPU, because neighbouring threads diverge less. Neighbouring pixels in a SIMD group tend to share an outcome anyway.
- Cardioid/bulb tests are only worth having at low zoom (|scale| > ~1e-3). They cost about 6 flops per pixel, so keeping them is harmless.

### Gaps
- No source specified how a periodicity epsilon should scale with pixel spacing or zoom depth.
- The Fraktaler 3 documentation was unreachable (404), so the exact semantics of its interior/inscape test are unconfirmed.
- No published benchmark of derivative interior detection under perturbation on GPU was found.

## 2. Boundary tracing, Mariani–Silver, solid guessing: safety and GPU suitability

### Takeaway
Border tracing and rectangle subdivision rely on the Mandelbrot set being full (simply connected interior). They can skip over 90% of iterations on interior-heavy views. They are only exact on regions of one discrete value, and the corner-only (Fractint-style) guessing variant visibly damages images. KF2+ found guessing caused speckles, and on a GPU via OpenCL it gave only about 1.5× instead of the expected 4×. For a GPU, use tile-level variants: coarse-pass, then refine only tiles whose border is not uniform, with exact checking of the full tile border.

### Cited Findings
- **Border tracing** follows iteration-level bands and fills enclosed regions. This works because the set is full. It reduced iterations by 93.16% in a 1000-iteration example, and the benefit grows with maxiter. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **Mariani–Silver.** If all border pixels of a rectangle have the same iteration count, fill the rectangle. Otherwise subdivide (25×25 → 13×13 → 7×7 → 4×4), reusing border pixels. A halving variant with 1.4:1 boxes is faster. The corner-only Fractint method "causes damaged pictures more often" and only works OK with boxes of about 6×6 and no recursion. Both methods only work on regions of a single discrete colour. Smooth-coloured exteriors benefit only if the inner area uses a smooth method such as interior DE. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **KF2+ "Use guessing"** means "interpolation of neighbouring pixel data when the iteration count is the same". It speeds up interior regions and glitches. It was disabled in 2.12.4 because it caused random speckles, then re-enabled behind a menu option in 2.12.7. It has a race condition with progressive interlacing. With OpenCL on GPU the speedup was about 1.5× instead of 4×, and image dimensions must be multiples of 2. The manual recommends disabling it for exterior regions when using OpenCL. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- **XaoS** uses an "adjacency optimization to exclude calculating pixels in the interior of large connected solid regions". — [mrob Mu-Ency: XaoS](https://mrob.com/pub/muency/xaos.html)

### Inferences
- **Safety.** With smooth colouring, iteration counts are rarely identical in the exterior, so guessing mostly fires on interior (maxiter or derivative-interior) pixels. This is where it is safest and most valuable. Restricting guessing to "all border pixels interior" is a safe and very effective policy, since interior detection makes interior pixels cheap anyway.
- **Artifact risk.** Thin exterior filaments that cross a tile without touching its border can be lost. Mitigations: small tiles (8×8 or 16×16), checking every border pixel, and an optional DE check. If any border pixel has DE < tile size, refine. The DE gives a rigorous bound via Koebe 1/4: no set point lies closer than DE/4.
- **GPU mapping.** Pass 1 computes every Nth pixel or tile borders. Pass 2 is a compute pass that classifies tiles (uniform interior → fill, else enqueue) and uses an indirect dispatch (Metal `dispatchThreadgroups(indirectBuffer:)`) over the non-uniform tiles. This avoids recursion and per-thread divergence. The KF2+ OpenCL result (1.5×) suggests the gain is bounded by divergence and the overhead of extra passes.
- With a derivative interior test the cost difference between interior and exterior pixels shrinks, so the payoff from guessing shrinks too. Measure before investing.

### Gaps
- No published GPU (Metal/CUDA) benchmark of tile-based early-out for perturbation deep zoom was found.

## 3. Exterior distance estimation, its perturbation form, and uses (colouring, AA, adaptive sampling, slopes/normal maps)

### Takeaway
Exterior DE needs the c-derivative der_c ← 2·z·der_c + 1 alongside z. DE = 2|z|·ln|z| / |der_c|, and it is a lower bound up to a factor of 4. Under perturbation, use the full orbit value Z + z in the derivative recurrence. Initialise the derivative with the pixel spacing (mathr's trick) so it stays in range. This matters in float32, whose maximum is about 3.4e38. DE gives cheap anti-aliased filaments, boundary detection, adaptive-sampling triggers, and normal-map "3D" lighting.

### Cited Findings
- **Exterior DE.** Start with ∂_c P⁰ = 1 and use ∂_c P^{n+1} = 2·Pⁿ·∂_c Pⁿ + 1. b = 2|Pⁿ|·ln|Pⁿ| / |∂_c Pⁿ|. By Koebe 1/4, no point of the set is closer than b/4. A few extra iterations past bailout improve the estimate. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **Chéritat on Milnor DE.** d_n = 2|z_n|·log|z_n| / |ρ_n|, with ρ_{n+1} = 2z_nρ_n + 1. The caveat is "1/5 < d/lim d_n < 1 provided |z| < 5.7".
  - **Boundary test, square-root and division free:** rsq·(log rsq)² < |thickness·pixel_size·ρ|².
  - **AA without oversampling:** t = clamp(d/(thickness·pixel_size), 0, 1), lerp from boundary colour to exterior colour, thickness 1.414.
  - **Exterior banding:** parity of floor(log(d)/k).
  - [Chéritat wiki](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set)
- **Henriksen's method (pixel as a disk).** der ← 2z·der + dc with dc = pixel_size·thickness. Boundary if |z|² < |der|². Thickness above 0.25 gives bad images, and R = 10 works well. A spherical-metric variant exists. — [Chéritat wiki](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set)
- **Perturbation DE (mathr).** Use dual numbers for automatic differentiation. "initialize the dual parts with the pixel spacing and don't scale at the end." This avoids derivative overflow, and the same rescaling factor applies to the regular and dual parts. Single precision (24 bits) "can be used successfully" for perturbed orbits but underflows below about 1e-38. Floatexp gives range but is slow. Rescaling gives a constant-factor speedup over floatexp. — [mathr: Deep zoom theory and practice (2021)](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- **KF2+ derivatives.**
  - Analytic DE requires enabling derivatives. Disabling them speeds up rendering but breaks analytic DE.
  - Numerical DE options: Traditional, Forward 3×3, Central 3×3, Diagonal 2×2, Least Squares 2×2/3×3, Laplacian 3×3. These are computed from neighbour iteration counts and need no derivative.
  - Directional DE is stored as DEX/DEY in EXR and is used for analytic slopes.
  - Known issue: analytic DE breaks at some power-3 locations unless series approximation is disabled.
  - [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- **Normal map / slope lighting (Chéritat).** u = z/der (der = dz/dc), normalised. With light direction v = exp(i·angle), t = (Re(u)Re(v) + Im(u)Im(v) + h2)/(1 + h2), clamped to ≥ 0. Parameters used: h2 = 1.5, angle 45°, R = 100. There is a Milnor-based variant with a second derivative der2 ← 2(der2·z + der²) and u = z·der·((1 + lo)·conj(der²) − lo·conj(z·der2)), lo = ½ log|z|². The resulting normals do not correspond to any real height field. — [Chéritat wiki](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set)
- **KF2+ slopes** were rewritten in 2.15.1 to be independent of zoom level and iteration count. Controls: magnification (start 100) and percentage applied. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)

### Inferences
- **Metal float32 recipe.** Store der in units of pixels: der₀ = 0, der ← 2(Z + z)·der + 1·(pixel_spacing / pixel_spacing). In practice, initialise or add the pixel-spacing-scaled term so that |der| ~ |z|/DE_in_pixels. DE in pixels is then 2|z|ln|z|/|der|, directly usable for AA (`coverage = clamp(DE_px/thickness, 0, 1)`). Without this, der overflows float32 near 1e38 at moderate iteration counts or zoom depths.
- If der is in pixel units it can still overflow for pixels very close to the set after many iterations. Rescale der and track an exponent as needed (cheap, mirroring the delta rescaling).
- The normal map needs only u = z/der normalised, so the scale of der cancels. That makes slopes almost free once der exists.
- Numerical DE (KF2+'s "Traditional"/Laplacian over neighbour smooth iteration counts) is a post-process fallback that needs no per-iteration derivative. It costs one 3×3 texture pass and is good enough for slopes and AA guidance.

### Gaps
- The perturbation-form derivative recurrence was not written out explicitly in fetched sources. The dual-number formulation implies der_{n+1} = 2(Z_n + z_n)·der_n + 1 (scaled), but treat this as derived rather than quoted.

## 4. Smooth colouring, bailout choice, histogram, palettes, stripe/TIA/curvature averages, binary decomposition, field lines

### Takeaway
Smooth iteration μ = n + 1 − log₂(log|z_n|) (with a large bailout, R ≥ ~50 and commonly 256 or higher) is the near-free baseline. Store μ and colour later in a separate pass. Log-potential scaling (log of μ or of the potential) keeps deep-zoom colouring usable as iteration counts grow. Histogram colouring needs a full-image reduction pass. Branching averages (TIA, curvature, stripe) add 1–3 complex ops per iteration plus a smoothing interpolation. Phase (arg z_final) gives binary decomposition and field-line effects for free.

### Cited Findings
- **Normalised iteration count.** ν = n − log_P(log|z_n|/log N), where N is the bailout. Pseudocode with bailout 2⁸: `log_zn = log(x²+y²)/2; nu = log(log_zn/log 2)/log 2; iteration = iteration + 1 − nu`, then lerp between palette entries using the fractional part. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **Quilez.** `n - log(log(length(z))/log(B))/log(2.0)`. A bailout "in the order of 50" is enough for smoothness, and the example uses B = 256. The optimised form is `log2(log2(dot(z,z))) + 4.0` for B = 2⁸. Use dot(z,z) against B² to skip the square root. — [Inigo Quilez: msetsmooth](https://iquilezles.org/articles/msetsmooth/)
- **Chéritat.**
  - Potential V = log|z|²/2ⁿ, stopping when |z|² > 10⁶. The relative error is about 1/R.
  - **Deep-zoom log-potential palette:** x = log(V)/K with K = log 2, colour = g(x) periodic, for example g(x) = 255·(1 + cos 2πx)/2. Use a larger K for "furry" deep zooms.
  - [Chéritat wiki](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set)
- **Histogram colouring:**
  1. Store the counts.
  2. Build a histogram NumIterationsPerPixel[i].
  3. Compute the total.
  4. hue = Σ_{i ≤ iteration} NumIterationsPerPixel[i]/total.
  - It spreads colours by area independently of maxiter, and can be combined with smooth colouring. The section is flagged as needing citations. — [Wikipedia](https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set)
- **KF2+ colouring options:**
  - Smooth method: "Log" is the renormalised smooth count; "Linear" interpolates between the last two iterations and suits small bailouts and Flat colouring. Log can produce seams.
  - Escape radius: High, Bailout = 2, Low bailout, Custom.
  - Transfer functions: Standard, Square root, Cubic root, Logarithm, Stretched, distance-based, LogLog, ATan, Fourth root.
  - Phase channel T = arg of the final iterate, plus "Phase strength".
  - Triangle Inequality Average is OpenCL-only and replaces the final angle in the phase channel.
  - Palette waves, Infinite waves, Texture, and dithering at the end of colouring to reduce banding.
  - [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- **KF2+ GLSL colouring.**
  - Users write `vec3 colour(void);`. Accessors include getN, getDE and getGlitch, plus helpers KF_Colour, KF_Slopes and KF_InfiniteWaves.
  - Uses a `float49` type: libqd-derived extended precision of about 49 bits, so the smooth iteration count is preserved when N is large.
  - The default shader matches the CPU result to 1–2 ULP. Custom GLSL is zoomasm-compatible.
  - [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- **zoomasm** stores N in split channels N0/N1/NF (offset by IterationsBias), because a single float32 cannot hold large smooth iteration counts precisely. getN() returns it "at higher precision than single-precision float". — [zoomasm](https://mathr.co.uk/zoomasm/)
- **Triangle inequality average.** Per iteration: min = ||z_old| − |c||, max = |z_old| + |c|, sum += (|z_new| − min)/(max − min). Divide by i. Smooth with sum2 (the previous average) using f = (1/log p)·log(log(B)/2) − (1/log p)·log(log|z|), index = sum2 + (sum − sum2)·(f + 1). **Curvature average:** a += |arg((z_n − z_{n−1})/(z_{n−1} − z_{n−2}))|, interpolated the same way. TIA is discontinuous where points map to zero; curvature is continuous there. — [Wikibooks: triangle_ineq](https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/triangle_ineq)
- **Stripe average** belongs to the branching-average family (with TIA and curvature) and has a stripe-density parameter. It is credited to Jussi Härkönen. — [Wikibooks: Ultra Fractal](https://en.wikibooks.org/wiki/Fractals/ultrafractal); [Rosetta Code diff](https://rosettacode.org/wiki/Special:MobileDiff/347705)
- **Interior colouring options:** closest-approach index = √(min|z|)·75/256 (bof60), argument of the last iterate (Fractint INSIDE=ATAN), and capture time. — [Wikibooks interior](https://en.wikibooks.org/wiki/Fractals/Iterations_in_the_complex_plane/Mandelbrot_set_interior)

### Inferences
- **Float32 pitfall.** At deep zoom μ can be about 10⁶–10⁹. A float32 holds μ to only about 0.06–60 units there, which destroys the fractional part and causes banding. Store integer n (uint32) plus fractional f (float) separately, as zoomasm does with N0/N1/NF and KF with float49. Colour from (n − n_min) + f, or apply log(μ) in mixed form.
- **Cheapest good-looking stack:** smooth μ split as (uint n, float f), a log transfer function, a periodic cosine palette, and DE-based darkening plus slope lighting. TIA and stripe add per-iteration cost and give "texture". Stripe is the cheapest of these (one sin of arg per iteration; the standard formula is assumed rather than sourced here).
- Histogram colouring needs a GPU reduction (atomic histogram plus prefix sum), but it is cheap at screen resolution. It flickers across zoom frames unless smoothed temporally.
- Use a bailout R ≥ 256 for smooth colouring. Large R costs only a few extra iterations, and under perturbation the escape test is on |Z + z|², which is fine in float32.

### Gaps
- Exact stripe-average and binary-decomposition/field-line formulas were not retrieved (the Rosetta Code Python section was truncated in fetches). Fraktaler 3's GLSL colouring API could not be fetched.

## 5. Anti-aliasing, supersampling, denoising

### Takeaway
State-of-practice renderers (KF2+, Fraktaler 3, zoomasm) use uniform random per-pixel jitter (about 1 pixel), accumulating subframes or stacking seeds, rather than regular-grid supersampling. This reduces moiré at the cost of noise. Downscale gamma-correctly (in linear light). DE gives analytic coverage for filaments without extra samples. DE can also drive adaptive sampling: spend extra samples only where DE_px < ~2.

### Cited Findings
- **KF2+ jitter.** A non-zero "Jitter seed" enables a pseudo-random-number generator. "Jitter scale" is in pixels, and 1 px is sensible. Uniform jitter is the default since 2.12.11, and Gaussian is "Probably best left disabled (uniform jitter looks better)". Different seeds can be stacked for AA as an alternative to rendering large and downscaling, which reduces moiré but adds noise. Downscaling is anti-aliased via Pixman and has been sRGB gamma-correct since 2.15.2. Dithering reduces banding. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- Fraktaler 3 records `image.subframes = 64` in its EXR metadata, which indicates multi-subframe accumulation. — [Wikibooks: Fraktaler-3](https://en.wikibooks.org/wiki/Fractals/fraktaler-3)
- zoomasm offers samples per pixel (reduces "sparkle") and shutter speed (motion blur). It recommends enabling KF jitter to reduce moiré. — [zoomasm](https://mathr.co.uk/zoomasm/)
- Chéritat supersamples by rendering large and downscaling by an integer factor (4801×4001 ÷ 5 or 6, 8000² → 800²), with "extra attention at performing a gamma-correct downscale". Pixel-centre sampling hides thin strands, and the DE-based AA variant gives partial AA without oversampling. — [Chéritat wiki](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set)

### Inferences
- **Interactive progressive AA for Metal:**
  1. Frame 0: one sample at the pixel centre.
  2. Each idle frame adds one jittered sample (low-discrepancy, e.g. R2 or Halton) per pixel.
  3. Accumulate a running mean of linear-light colour (or of colour computed per sample) in an RGBA16F/32F texture.
  4. Reset on camera change.
  This converges to high-quality AA with no extra cost while interacting.
- **Adaptive:** only pixels with DE_px < 1–2, or with high neighbour colour variance, get extra samples. With perturbation each extra sample is just another delta (same reference), so it is cheap.
- **Denoising:** no fractal-specific denoiser source was found. A simple bilateral filter guided by DE and iteration count could be used, but blurring fine filaments is a risk.

### Gaps
- No sources were found on ML or temporal denoisers applied to fractal renders as of 2026.

## 6. Reusing previous results: reprojection, progressive refinement, exponential map / zoomasm, instant-feeling interaction

### Takeaway
XaoS pioneered reusing the previous frame. It reprojects whole rows and columns (cost ~xsize + ysize per frame for the coordinate approximation) and computes only rows and columns without a good match, prioritising the zoom target. KF2+ uses Adam7-style progressive interlacing. For videos, the exponential-map approach renders log-polar strips once (e.g. 1152×128, 9:1), and zoomasm reprojects them on the GPU into any number of frames. For an interactive Metal viewer: reproject the previous frame's stored per-pixel data as a placeholder, then progressively recompute coarse-to-fine, prioritising the screen centre or zoom focus.

### Cited Findings
- **XaoS (Hubička algorithm).**
  - Keeps only the previous frame plus per-row real and per-column imaginary coordinates. The work is about xsize + ysize instead of xsize × ysize for the approximation step.
  - Choosing which old rows and columns map where is treated as an optimisation problem, and errors show as distortion.
  - Rows and columns without good data take the colour of the nearest one, then are recalculated as CPU allows, prioritising the area being zoomed into (reversed when zooming out).
  - Main loop: approximate rows, approximate columns, move old pixels, compute pixels lacking good approximations.
  - [Wikipedia: XaoS](https://en.wikipedia.org/wiki/XaoS)
- **KF2+** progressive previews use Adam7-style interlacing. "Reuse reference" stops the reference from being recalculated during further zooming, and works with auto glitch solving since 2.15.3. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- **KF2+ exponential map** (added in 2.15.1). Use a wide aspect ratio such as 9:1 (e.g. 1152×128) for more conformal results. Zoom-out sequences are stored with zoom size 2 and combined in zoomasm. 2.15.1.3 fixed OpenCL exponential-map DE scaling, which had caused visible rings in zoomasm output. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- **zoomasm:**
  - Renders "exponentially spaced rings around the zoom center, before reprojecting to a sequence of flat images".
  - Keyframes are EXR strips with N, NF, T (phase), DEX/DEY, RGB, interior and glitch flags. It rescales DE to screen space.
  - 16 layers are suggested for 2D and 25 for 360 projection. Raise the count if "small circles near the zoom focus" appear.
  - Colouring is a GLSL `colour()` function. Samples per pixel and shutter speed are configurable.
  - Recommended strip sizes: 1920×1080 output from 6912×768 strips; 3840×2160 from 12288×1360.
  - Fractal computation "is often the slow part", which the method does once. zoomasm 2.0 re-renders only on change.
  - [zoomasm](https://mathr.co.uk/zoomasm/)
- Fraktaler 3 supports an exponential map transform in its command-line version for rendering zoom-out sequences. — [Wikibooks: Fraktaler-3](https://en.wikibooks.org/wiki/Fractals/fraktaler-3)

### Inferences
- **Exponential map geometry (standard, not quoted from a fetched source).** Pixel (u, v) maps to c = c₀ + r₀·exp((v·2π/W) + i·(u·2π/W)). The x axis is angle, the y axis is log-radius, and a pixel square in log space corresponds to a locally square pixel in the image. A W×H strip therefore spans a zoom factor of exp(2π·H/W). For 9:1 that is exp(2π/9) ≈ 2.01, which matches "zoom size 2" per keyframe.
- **Interactive reuse on Metal.** Keep a float buffer per pixel (n, f, DE, flags) for the last completed view. On pan or zoom, sample it with an affine reprojection as an immediate placeholder (XaoS-style, but per-pixel via the GPU texture sampler). Then schedule recomputation from coarse to fine: 1/16, 1/4, then full, and finally jittered AA samples. Prioritise tiles near the zoom focus.
- When zooming in by a factor of 2, about 1/4 of the new pixels coincide exactly with old pixel centres if the zoom is centred on a pixel. These can be copied exactly rather than recomputed. Off-grid zooms only allow approximate reuse.
- Combine reuse with "reuse reference": the reference orbit stays valid while the view stays within it, so an incremental frame costs only the perturbed pixels.
- The exponential map also makes a cheap "instant zoom" preview possible. One log-polar strip covering the current zoom range can be reprojected for continuous zoom animation without per-frame computation.

### Gaps
- XaoS's precise cost function and error thresholds were not retrieved (its original algorithm.txt failed TLS).
- No published 2025–2026 interactive deep-zoom renderer documentation on temporal reprojection with perturbation was found.

## 7. Maximum iteration auto-adjustment heuristics

### Takeaway
No standard published formula exists. KF2+ ships "auto iterations" (on by default) but does not document the algorithm. Its known failure is zoom-out sequences dropping maxiter too far, which produces black regions. With interior detection, a very large maxiter is cheap. The robust approach is therefore a high cap plus derivative interior detection, plus image statistics (the fraction of pixels hitting maxiter that were not detected as interior).

### Cited Findings
- **KF2+ "Use auto iterations"** under the Advanced menu: "Turns automatic iteration control on or off. This is on per default." It has been refactored. Known bug: zoom-out sequences produce black regions because maxiter drops too far before spirals appear in the next frame (workaround: disable it). It does not work in "Save KFR" mode, since no images are rendered for analysis, which implies it analyses rendered images. — [KF2+ manual](https://mathr.co.uk/kf/manual.html)
- Chéritat: interior detection makes large N cheap. He suggests avoiding N = 10¹⁰. — [Chéritat wiki](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set)
- Fraktaler 3 uses separate caps: total iterations (262144), maximum reference iterations, and maximum perturb iterations (4096). — [Wikibooks: Fraktaler-3](https://en.wikibooks.org/wiki/Fractals/fraktaler-3)
- General explorers raise maxiter with zoom depth. A high-precision renderer used a fixed cap of 65535 together with rho-style orbit detection. — [search summary: math.muni.cz](https://www.math.muni.cz/~xmacharacekj/mandelbrotExploring.html); [h-schmidt.net](https://h-schmidt.net/MandelApplet/mandelapplet.html)

### Inferences
- **Heuristic for this project:**
  1. After each frame, compute statistics over pixels: n_max_escaped (high percentile, e.g. the 99.9th, of escaped n) and frac_unresolved (pixels that reached maxiter without escaping and without passing the interior test).
  2. If frac_unresolved > 0.1% and n_max_escaped > 0.5·maxiter, set maxiter ← 2·maxiter, or set it to n_max_escaped·4.
  3. Never decrease maxiter during a zoom-out without a hysteresis margin. This fixes the KF2+ failure mode.
  4. Cap maxiter at the reference orbit length.
- The minimum escaped iteration count at depth rises roughly in proportion to the period of nearby minibrots. This is why fixed zoom-based formulas (e.g. k·log(zoom)) underestimate. Statistics-driven adjustment is more robust.

### Gaps
- KF2+'s actual auto-iteration algorithm is undocumented in the manual. Reading its source code would be needed.
