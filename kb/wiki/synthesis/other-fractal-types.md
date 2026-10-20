---
title: "Other fractal types: what almondbread could support, at what cost"
type: synthesis
date: 2026-10-19
sources: ["[[sources/mathr-deep-zoom-page]]", "[[sources/mathr-deep-zoom-theory]]", "[[sources/mathr-deep-zoom-theory-again]]", "[[sources/fractalforums-zhuoran-rebasing-bla]]", "[[sources/fraktaler-3-site]]", "[[sources/kf-manual]]", "[[sources/kf-page]]", "[[sources/fractalzoomer-readme]]", "[[sources/fractalshades-readme]]", "[[sources/ultrafractal-perturbation-equations]]", "[[sources/hpdz-animations]]", "[[sources/quilez-mandelbulb]]", "[[sources/skytopia-mandelbulb]]", "[[sources/wikipedia-mandelbulb]]", "[[sources/wikipedia-mandelbox]]", "[[sources/hvidtfeldt-de-3d-fractals]]", "[[sources/mandelbulber2-readme]]", "[[sources/alltools-mandelbulb-explorer]]", "[[sources/superliminal-buddhabrot]]", "[[sources/wikipedia-fractal-flame]]", "[[sources/wikipedia-newton-fractal]]", "[[sources/wikipedia-lyapunov-fractal]]", "[[sources/frax-about]]", "[[sources/frax-help-explore]]"]
tags: [formulas, feasibility, roadmap, julia, burning-ship, multibrot, mandelbar, hybrids, nova, buddhabrot, 3d, mandelbulb, mandelbox, architecture]
---

# Other fractal types: what almondbread could support, at what cost

**Question (user, 2026-10-19):** which other fractal types could almondbread support without
reinventing the wheel, building on what we have? Julia sets, and the 3D fractals (Mandelbulb,
Mandelbox) once considered.

**Short answer.** The deep-zoom engine (high-precision reference, float32 perturbation with
rebasing, BLA, floatexp deltas, chunked scheduler, colouring, navigation) carries over to every
**polynomial escape-time formula with one critical point**: Julia sets, Multibrot `z^p + c`,
Mandelbar, and the Burning Ship family. Mandelbar and Burning Ship also need a 2×2 matrix BLA path
and per-formula derivatives. Those four families are the realistic set, and Burning Ship is the
one with the most appeal. Hybrids come after them. Convergent formulas (Nova, Newton, Magnet) are
a research spike. Buddhabrot and Lyapunov are cheap shallow-only "modes". 3D (Mandelbox,
Mandelbulb) is a separate renderer with a separate product feel and a hard depth ceiling: XL work
that reuses roughly the app shell and nothing of the deep-zoom engine. The cheaper "3D" is the
2.5D landscape mode we already prototyped ([Landscape mode: fly over the Mandelbrot set as 2.5D terrain](../decisions/landscape-mode.md)), which keeps the whole
engine and its depth.

Effort scale used below (agent-driven, verified the way CLAUDE.md demands): **S** a few days,
**M** 1–2 weeks, **L** 3–6 weeks, **XL** months.

## 1. What we have, and how formula-specific it is

Read from the code on 2026-10-19 (main at e68df79).

| Area | Files | Formula-specific? |
|---|---|---|
| High-precision reference orbit | `almondbread-math/src/orbit.rs` (`Z² + C` on `Raw` limb registers, specialised for 1–16 limbs), `parallel.rs` (three threads for X², Y², (X+Y)²) | Hard-coded `Z² + C`. The three-product split also serves Mandelbar and Burning Ship (`2|XY|` comes from the same products), not powers > 2 (inference). |
| Period, nucleus, size | `math/selection.rs`, `math/rays.rs`, `core/seek.rs`, `core/cycles.rs` | Mandelbrot: ball/box period, Newton with `dz/dc = 2 z dz/dc + 1`, external rays. Non-conformal formulas need 2×2 real Jacobians. |
| f64 reference, oracle, overlay | `core/reference.rs`, `core/oracle.rs` (CPU truth for `verify`), `core/orbit.rs` (orbit overlay) | One `z² + c` step each; small, but the oracle must exist per formula or verification is lost. |
| Skip tables | `core/bla.rs` (`A = 2Z`, `B = 1`, radius), `core/la.rs` + `render/la_gpu.rs` (Imagina LA, power 2 only) | BLA generalises (scalar for analytic formulas, 2×2 for the others, [Bivariate linear approximation (BLA)](../techniques/bla.md)). LA stays a Mandelbrot capability ([Linear approximation with period-aligned stages and AT (Imagina)](../techniques/linear-approximation-imagina.md)). |
| Kernels | `render/src/shaders/mandelbrot.metal` (3 565 lines; `iterate` alone ~1 400) | The perturbed step `cmul(2Z + z, z) + c` and its derivatives are written inline at about 10 sites: plain float32, deep floatexp, BLA/LA walk, seed rounds (local SA coefficients `A2`, `B2`), interior probe (`f^P` Newton), interior-colour cycle analysis, rebasing. The colour, fog, compose, AA, overlay and survey kernels are formula-free. Nine function constants (`kDE`, `kInterior`, `kBLA`, `kDeep`, `kInteriorColor`, `kSeeded`, `kProbe`, `kLA`, ...) already specialise one source into pipelines. |
| Interior | `core/components.rs` (closed-form cardioid / period-2 bulb), derivative test, attracting-cycle probe ([Interior detection](../techniques/interior-detection.md)) | The closed-form test is Mandelbrot-only; the derivative test generalises (scalar or 2×2); the probe needs the formula's `f^P` and Jacobian. |
| Maths moments and content | `core/pi_moment.rs`, `render/pi_meter.rs`, `core/places_data.rs`, `core/tour.rs`, `core/journey.rs` + `journey/regions.rs`, `core/autopilot/lock.rs` | π meter: Mandelbrot only ([π in the Mandelbrot set (neck and cusp escape counts)](../techniques/pi-in-the-mandelbrot-set.md)). Places, tours, journey regions: Mandelbrot data. Autopilot and target lock read the iteration field and the survey; mostly formula-free but tuned on Mandelbrot. |
| Scheduler, host, navigator, colour, palettes, UI | `render/scheduler.rs`, `host.rs`, `core/input.rs`, `core/palette*.rs`, Swift UI | Formula-free (2D viewport). |

So per-formula variants are needed in about **12 modules**: orbit/parallel, selection/seek/cycles,
reference, oracle, orbit overlay, bla, the kernel step sites, components, and the content modules
(places/tours/journeys/π). The rest, which is most of the code by line count (scheduler 6.5 k lines, input 5.8 k,
overlays, palettes, UI), is reused unchanged.

## 2. The families

### 2a. Escape-time, perturbation-friendly (the engine carries over)

**Plane transformations of the Mandelbrot set (free).** The logistic/λ plane is the Mandelbrot
set under `c = λ/2 − λ²/4` (the logistic map is conjugate to `z² + c`; standard result, unverified
here). The `1/μ` plane and the exponential (Mercator) view ([Exponential map (log-polar) rendering](../techniques/exponential-map.md)) are
other analytic maps `c = T(μ)`. Map the reference point in high precision and the pixel deltas by
`δc = T'(μ₀)δμ + ½T''(μ₀)δμ²`, and every feature works unchanged, seek and π included (our
derivation). Honest limit: at depth any analytic map is locally affine, so deep views look like
rotated, scaled Mandelbrot views. Only shallow views look new. Fractal Zoomer lists plane
transformations (src: [Fractal Zoomer README](../sources/fractalzoomer-readme.md)).

**Julia sets.** `z ← z² + k`, pixel = `z₀`. Perturbed step `z' = 2Zz + z²` with no `+c`, so BLA
has `B = 0` ([Julia sets](../formulas/julia.md)). Rebasing needs a reference orbit through 0
(src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #10). Our observation: the orbit of the
critical point 0 under `z² + k` is exactly the **Mandelbrot reference orbit of `k`**, which we
already compute. So a deep Julia view needs two references: the view-centre orbit for the first
steps, then rebasing onto the critical orbit at index 0 (inference, not yet tested; Fractal Zoomer's
author reports rebasing "also works in julia sets", src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
§Reply #11). Interior is simple: if `k` is in a hyperbolic component, every interior pixel falls
into the same attracting cycle, whose period we already find for `k` (inference). DE uses
`dz/dz₀` instead of `dz/dc`. There are no minibrots in a Julia set, so "seek" does not apply. The
nearest analogue is zooming to preimages of 0 (inference). Appeal: high, both as a **Julia
companion** (the Julia set of the point under the cursor or the view centre, shallow, direct
float32 or double-float) and as a **Julia twin** of a deep Mandelbrot view: near a Misiurewicz
point the Julia set of `c` and the Mandelbrot set look alike (Tan Lei, unverified here). Deep
Julia zooms on their own repeat themselves (a Julia set is mapped onto itself by `f`), so they
have less variety than Mandelbrot dives (inference). Frax sells Julia sets as zoomable
"infinitely" (src: [Frax: What is Frax?](../sources/frax-about.md)).

**Multibrot `z^p + c`.** Binomial delta, `A = pZ^{p−1}`, one critical point, rebasing as for
Mandelbrot ([Multibrot (integer power p)](../formulas/multibrot.md)). Newton for nuclei, atom domains, interior derivative and DE
generalise by replacing `2z` with `pz^{p−1}` (inference). LA (power 2) and the closed-form cardioid
test do not. KF supports powers 2–10 and Fraktaler 3 any power per line
(src: [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md), [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)). F3 warns that plain float gives
iteration-band glitches at high powers (src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)), a float32 risk for
us. Appeal: medium (p-fold symmetry; the shapes get repetitive).

**Mandelbar (Tricorn).** Conjugated delta, 2×2 real BLA matrix that is a scaled reflection, so the
radius equals the conformal one ([Mandelbar (Tricorn)](../formulas/mandelbar.md)). DE and interior need dual numbers with
2 or 4 dual parts and a matrix norm (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §5, §6). Its value for
us is as the cheapest way to build the non-conformal path; its own appeal is medium.

**Burning Ship and abs variants** (Celtic, Buffalo, Perpendicular, and the cubic ones). `diffabs`
case analysis for the delta; 2×2 BLA with fold caps `min(..., |X|, |Y|)`, which Fraktaler 3 halves
for safety (src: [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md) §4.2; [Burning Ship and abs variations](../formulas/burning-ship.md)). Series
approximation, and so our local SA seeds, is "only good so long as the region is not folded"
(src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Series Approximation), so seeds go off for these
formulas. The known hard case is the **deep needle**: full floatexp steps whenever *either* part
of `Z` is small, and automatic detection "remains unresolved in KF" (src:
[mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Deep Needle). KF defaults "reference strict zero" on for
non-Mandelbrot formulas near the needle (src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)). Depth is demonstrated:
fractalshades to 1e-2000 (src: [Fractalshades README](../sources/fractalshades-readme.md)); the first Burning Ship deep zoom
video (about 2.35e-100) dates from the brute-force era (src: [HPDZ.NET: Burning Ship and Nova deep zooms](../sources/hpdz-animations.md)).
Newton zooming works for these formulas in Fraktaler 3, with power presets by degree
(src: [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)), so seek and minibrot framing carry over once
`selection.rs` has 2×2 Jacobians. Appeal: **high**. The Burning Ship's ships and needles are the
most popular non-Mandelbrot deep-zoom subject (unverified: forum anecdote, no view counts found).

**Hybrids** (KF stanzas, F3 opcode lines). Per-operator perturbation, one reference and one BLA
table per loop phase, code generation (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Hybrid Fractals,
§Code Generation; [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md) §Rebasing; [Hybrid formulas](../formulas/hybrids.md)). Even KF's
rebasing had bugs with hybrids because they need references at each phase of the loop
(src: [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)). Appeal: high for the KF/F3 community, low for casual users. This is
roadmap F7 (was M8), [F7: Hybrid formulas via an opcode model](../plans/f7-hybrids.md).

**Phoenix** (`z_{n+1} = z_n² + p + q z_{n−1}`). The delta is easy (one extra linear term), BLA
works on the state pair (a 2×2 complex matrix), but rebasing needs both state components to meet
a zero of the reference together, which nobody has worked out (inference; no source found).
Appeal low–medium.

**Convergent and rational: Nova, Newton, Magnet.** Fractal Zoomer reportedly uses perturbation
with rebasing for nova, magnet, lambda and Newton parameter space
(src: [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md) §Reply #8). Rational maps need the quotient
identity, and BLA radius criteria for convergence to a root are not published
([Other formulas: feasibility of deep zoom](../formulas/others-feasibility.md), [Can perturbation and rebasing handle Nova, Newton and Magnet formulas?](../questions/perturbation-convergent-formulas.md)). Brute-force Nova
at 4.3e-21 already needed 192-bit arithmetic (src: [HPDZ.NET: Burning Ship and Nova deep zooms](../sources/hpdz-animations.md)). Newton basins
are Julia sets of the Newton map, coloured by root and convergence time
(src: [Wikipedia: Newton fractal](../sources/wikipedia-newton-fractal.md)). Appeal: medium (a distinct, smooth look; "mini-novas"
in parameter space). Research spike first.

**Transcendental (Collatz, exp, sin).** Perturbation works through addition theorems (e.g.
`cos(Z+z) − cos Z = −2 sin(Z + z/2) sin(z/2)`), but the reference needs BigFloat sin/cos/exp,
which `almondbread-math` does not have (inference). Non-integer powers do not reduce at all
(src: [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md)). Appeal low. Not recommended.

### 2b. Other 2D families (not deep-zoom escape-time)

**Buddhabrot / Nebulabrot.** A histogram of escaping orbits, the Mandelbrot set shown
differently; Nebulabrot = three iteration limits as RGB (src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)).
It relates directly to our orbit overlay ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Orbit overlay"): the same orbits,
accumulated. On Metal it is a compute kernel with atomic adds into a texture, plus progressive
accumulation, which our progressive display and tone mapping already do (inference). Depth is the
limit: "not possible to zoom arbitrarily deeply"; Metropolis–Hastings sampling (Boswell) speeds
up close-ups (src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)). Effort M, appeal high (a famous image),
shallow only.

**Lyapunov.** Logistic map with an AB sequence, colour by exponent
(src: [Wikipedia: Lyapunov fractal](../sources/wikipedia-lyapunov-fractal.md)). A small standalone kernel; reuses viewport,
scheduler chunking, palettes and UI, but none of the deep machinery. No perturbation. Effort S–M,
appeal medium (striking, smooth stills), shallow only.

**IFS / fractal flames.** Chaos game, nonlinear variations, log-density histograms, density
estimation (src: [Wikipedia: Fractal flame](../sources/wikipedia-fractal-flame.md)). A different algorithm and a different
community (Apophysis, Chaotica, Electric Sheep). It reuses palettes, the Metal host and
accumulation, but not the engine, and flames are not zoom subjects. Effort L. Not recommended.

**L-systems.** Geometry, not iteration fields. Out of scope.

### 2c. 3D: Mandelbulb, Mandelbox, KIFS, quaternion Julia

**What they are.** Mandelbulb: the triplex power (radius^n, both spherical angles × n), n = 8
(src: [Wikipedia: Mandelbulb](../sources/wikipedia-mandelbulb.md), [Inigo Quilez: Mandelbulb](../sources/quilez-mandelbulb.md)). Mandelbox: box fold, ball
fold, `scale·z + c`, found by Tom Lowe in 2010 (src: [Wikipedia: Mandelbox](../sources/wikipedia-mandelbox.md)). KIFS and the
Menger sponge use folds and scales (src: [Hvidtfeldt: Distance Estimated 3D Fractals (blog series)](../sources/hvidtfeldt-de-3d-fractals.md), snippets).
Quaternion Julia sets are the "whipped cream" swirls that disappointed White
(src: [Daniel White: Mandelbulb (skytopia)](../sources/skytopia-mandelbulb.md)).

**How they are rendered.** Ray marching with a distance estimator. For the Mandelbulb it comes
from the potential gradient (src: [Inigo Quilez: Mandelbulb](../sources/quilez-mandelbulb.md)). For the Mandelbox a running
scalar derivative works because the folds are conformal
(src: [Hvidtfeldt: Distance Estimated 3D Fractals (blog series)](../sources/hvidtfeldt-de-3d-fractals.md), snippets). Shading uses DE normals, soft shadows, AO
and orbit traps. Quilez: real time "for moderate screen resolutions (say, 720p) with shadows but
no antialiasing" on GPUs of that time (src: [Inigo Quilez: Mandelbulb](../sources/quilez-mandelbulb.md)).

**Real time on Apple GPUs (our estimate, to be measured).** Mandelbulb DE: about 10–15
iterations × ~80 flops, ~60–150 march steps per primary ray plus a shadow ray, so ~0.1–0.2 Mflop per
pixel. At 2560×1600 that is 0.4–0.8 Tflop per frame: 80–150 ms on an M1 Pro (~5.3 TFLOPS,
[Apple GPU architecture for compute](../platform/apple-gpu-architecture.md)). Getting to 60 Hz therefore needs a quarter of the pixels
plus temporal upscaling (MetalFX, [Apple docs and SDK headers: MetalFX framework](../sources/apple-metalfx-docs.md)) and reuse of the previous frame's
march distance. 120 Hz at full quality is not realistic. An iPad has about half to two thirds of
that GPU. The Mandelbox iteration is cheaper (~25 flops) but runs more iterations, so it lands in the same range
(all inference).

**Depth.** Float32 ray marching breaks down at about 1e4–1e5 magnification (src:
[AllTools: Mandelbulb 3D Explorer (WebGL)](../sources/alltools-mandelbulb-explorer.md), low reliability; it agrees with our estimate in
[Landscape mode: fly over the Mandelbrot set as 2.5D terrain](../decisions/landscape-mode.md)). Our double-float library ([Double-float (float-float) arithmetic](../techniques/double-float.md)) costs 7–12×
float32 (measured M7, [almondbread benchmarks and verification record](benchmarks.md)) and would reach roughly 1e10–1e12 (inference). **No
published perturbation scheme for 3D fractals was found** (searched 2026-10-19: fractalforums,
mathr, Mandelbulber; Mandelbulber's precision limits are not documented either). In principle the
Mandelbox is the candidate: its folds are piecewise linear (`diffabs`-like case analysis) and the
ball fold is a piecewise inversion (quotient identity), and `z₀ = 0` gives rebasing a zero
(inference). The Mandelbulb's trig/atan2 form is singular at the origin and on the z-axis. Even
with perturbation, every ray point needs its own delta to a reference, folds branch per sample,
and the march's epsilon policy must work in extended range. That is a research project, not a
feature. White's own verdict is that 2D keeps more variety under zoom than any 3D candidate
(src: [Daniel White: Mandelbulb (skytopia)](../sources/skytopia-mandelbulb.md)).

**What would be reused vs new.** Reused: the Metal host and `CAMetalLayer` surface, the
per-refresh frame loop and hang guards, palettes and looks, overlay text, the flight recorder
idea, tours and places as concepts, the UI shell. New: ray marcher, 6-DOF camera and flight model
(the `Navigator` is 2D pan/zoom/rotate), lighting (shadows, AO, fog), temporal reuse in 3D,
epsilon/LOD policy, the 3D formula library, 3D places and tours, an oracle for verification. The
perturbation engine, reference orbits, BLA/LA, scheduler levels, interior probe, seek, π, journeys
and autopilot target lock do not apply.

**The 2.5D landscape is the cheaper "3D".** The landscape prototype (height from the exterior DE)
already runs at 118–120 fps at 120 Hz on a Metal geometry clipmap, keeps perturbation and
rebasing, and stays self-similar at every depth ([Landscape mode: fly over the Mandelbrot set as 2.5D terrain](../decisions/landscape-mode.md) L1/L2). Its 1e-12
limit is a prototype choice; the deep kernel would lift it (inference from M3b). It is the way to
"fly through a fractal" that keeps almondbread's identity of unlimited depth.

## 3. Competitors (brief)

| App | Formulas | Depth | 3D | Platform |
|---|---|---|---|---|
| Kalles Fraktaler 2+ | 100+ formulas, hybrids, powers 2–10 | floatexp, any | no | Windows, frozen 2022 ([Kalles Fraktaler 2 +](../implementations/kalles-fraktaler-2.md)) |
| Fraktaler 3 | opcode hybrids (abs, neg, power) | any, BLA | no | Linux/Windows/Android/web, AGPL ([Fraktaler 3](../implementations/fraktaler-3.md)) |
| Fractal Zoomer | 500+ formulas, Julia maps, plane transforms, convergent formulas with perturbation, 3D height maps | any | height maps | Java ([Fractal Zoomer](../implementations/fractal-zoomer.md)) |
| fractalshades | Mandelbrot, Burning Ship | 1e-2000 | no | Python library ([Fractalshades README](../sources/fractalshades-readme.md)) |
| Mandelbulber 2 | 3D (bulb, box, IFS, hybrids) | not documented | ray marching, OpenCL, keyframes | GPL desktop ([Mandelbulber v2 repository](../sources/mandelbulber2-readme.md)) |
| Frax | Mandelbrot, Julia, ~20 formulas, lit height | 64-bit (~2e14) | lit height field | iOS, development reportedly dormant ([Frax: What is Frax?](../sources/frax-about.md), [Frax help: Explore (precision limit)](../sources/frax-help-explore.md)) |

Gaps worth filling: no Apple-native app offers **deep** Julia, Burning Ship or Multibrot zooms
at interactive speed. Frax stops at 1e14, and the deep renderers are Windows/Linux/Java and not
real-time. No app combines deep 2D zoom with a real-time 3D-like flight (the landscape). Real-time 3D
Mandelbox flight on iPad is the most crowded space of the gaps (shadertoys, Mandelbulber, several
App Store viewers, unverified count), and it lacks depth.

## 4. The matrix

Reuse = share of the current pipeline that works unchanged. LA = Imagina LA. Effort is after the
pluggable-formula refactor (section 6) unless marked.

| Family | Reuse | Perturbation / rebasing | BLA / LA | Practical depth | Interior / DE / seek | Real time iPad | Effort | Appeal | Main risks |
|---|---|---|---|---|---|---|---|---|---|
| Plane transforms (λ, 1/μ) | ~100 % | yes / yes | yes / yes | unlimited | all, via the map | yes | S | low–med | deep views look like Mandelbrot |
| Julia companion (shallow) | high | n/a (direct float32/ff) | n/a | ~1e5 float32, ~1e12 ff | cycle interior, DE / none | yes | S–M | high | UI space on iPhone |
| Julia deep | high | yes / via critical orbit (to test) | scalar, `B = 0` / probably | unlimited | cycle interior, DE / no seek | yes | M | med–high | rebasing scheme unproven; repetitive at depth |
| Multibrot p = 3..8 | high | yes / yes | scalar / no | unlimited | derivative test, DE, Newton seek / no closed form, no π | yes | M | medium | float32 band glitches at high p |
| Mandelbar | high | yes / yes | 2×2 / no | unlimited | 2×2 duals, Newton with Jacobian | yes | M | medium | builds the non-conformal path |
| Burning Ship + abs family | medium | diffabs / yes | 2×2 with fold caps / no; no SA seeds | unlimited, slow at needles | 2×2 duals, Newton works (F3) | yes, slower | L | **high** | deep needle in float32, autopilot/journey tuning |
| Hybrids (opcodes) | medium | per operator / per phase | per-phase tables / no | unlimited | 2×2, Newton per degree | yes | L–XL | high (enthusiasts) | codegen, references × phases |
| Phoenix | medium | yes / unsolved | 2×2 complex / no | limited without rebasing | unclear | yes | L | low–med | rebasing on a two-step state |
| Nova / Newton / Magnet | low–med | reported / reported (FZ) | criteria unknown | unknown | convergence test / no | yes | L–XL (spike first) | medium | no published scheme |
| Collatz / transcendental | low | yes in principle / ? | yes (analytic) | needs BigFloat trig | partial | yes | L | low | BigFloat sin/cos/exp |
| Buddhabrot / Nebulabrot | low–med (orbits, colour) | no | no | shallow (MH helps) | n/a | progressive | M | high | slow convergence, iPad thermals |
| Lyapunov | low (viewport, palettes) | no | no | shallow | n/a | yes | S–M | medium | none worth noting |
| IFS / flames | low | no | no | n/a | n/a | progressive | L | med (other audience) | separate product |
| 2.5D landscape (Mandelbrot DE) | high (engine) | yes | yes | prototype 1e-12, deep kernel would lift it | DE is the terrain | 118–120 fps measured on Mac | L (L0–L2 done) | high | far-field morph, slow interior |
| 3D Mandelbox / Mandelbulb | ~20–30 % (shell) | none published | n/a | ~1e4–1e5 float32, ~1e10–1e12 ff | DE only | 30–60 Hz with upscaling (estimate) | XL | high "wow" | no depth, crowded niche, new renderer |

## 5. Recommendation

**Cheapest wins, with what we have:**
1. **Julia companion / twin view** (S–M). Shallow Julia of the cursor or centre `c`, plus "show
   the Julia set of this place". Touches the kernel only through a direct (non-perturbed) path
   and reuses colouring and palettes. High user value, educational, fits the journeys' embedded
   Julia stages ([Finding good places at extreme depth (1e100 to 1e100000+)](finding-deep-places.md)).
2. **Plane transforms** (S), only if wanted for shallow variety. Cheap, but they add little at depth.
3. **Buddhabrot as a look** (M, independent of the formula refactor): progressive accumulation
   of the same orbits the overlay draws.

**Needs the pluggable-formula refactor first (M), then in order:**
4. **Multibrot** (M): exercises every per-formula hook (reference, BLA, oracle, derivative,
   Newton) with scalar complex arithmetic only. It is the best test that the refactor kept the
   Mandelbrot fast path intact.
5. **Deep Julia** (M): two references (centre plus the Mandelbrot orbit of `k`), test the
   critical-orbit rebasing.
6. **Mandelbar** (M): builds the 2×2 BLA, dual-number derivatives and Jacobian Newton.
7. **Burning Ship family** (L): the payoff. Diffabs in every step site, fold-capped BLA, SA seeds
   off, a needle heuristic (share of steps with `min(|X|,|Y|)` tiny → floatexp kernel), a Burning
   Ship places catalogue and journey tuning.
8. **Hybrids / opcodes** (L–XL, roadmap F7, was M8), after 7, with runtime MSL generation.
9. **Nova / Newton / Magnet**: a one-week research spike on
   [Can perturbation and rebasing handle Nova, Newton and Magnet formulas?](../questions/perturbation-convergent-formulas.md) before any commitment.

**3D verdict.** Do not integrate true 3D into the deep-zoom explorer. If it is wanted, build it as
a **separate mode ("3D worlds")** with its own renderer and controls, starting with the Mandelbox
(conformal folds, scalar DE, architectural spaces that suit flying through), then the Mandelbulb.
Budget XL (two to four months of agent work to reach the polish level of the 2D app), float32
with temporal upscaling, and a depth ceiling of ~1e4–1e5 (or ~1e10–1e12 at 7–12× cost with
double-float). That is the opposite of almondbread's selling point. Ship the **2.5D landscape**
first: it is mostly built, it is real time, and it keeps unlimited depth. Revisit true 3D only if
the user wants the 3D look itself more than depth.

## 6. Architectural change: pluggable formulas without slowing Mandelbrot

**Principle:** Mandelbrot stays formula 0 and compiles to exactly today's code. Every new formula
is a set of capabilities, and anything a formula lacks is hidden in the UI rather than faked.

**Kernels** (`render/src/shaders/mandelbrot.metal`, `render/src/metal/mod.rs`, `gpu_types.rs`):
- Factor the ~10 inline step sites into inline functions: `fStep(Z, z, c)` (plain float2),
  `fStepX` (floatexp), `fDeriv` (dz/dc or dz/dz₀ update), `fJac` (interior derivative), and the
  seed-SA coefficient recurrences. Each one branches on a new **function constant `kFormula`**
  (uint). The compiler folds the branch, so the `kFormula = 0` pipeline is the current code.
  Prove it the way M7 did: bit-identical fields and a `bench` A/B geomean of ~1.0.
- Add `kConformal` (bool): when false, `PixelState` carries a 2×2 derivative (four floats instead
  of two), the BLA entries are 2×2, and SA seeds are compiled out for abs formulas.
- Pipeline count multiplies by formula. Compile pipelines lazily per formula and measure the cold
  shader cache with `ALMONDBREAD_KERNEL_SALT`.
- Hybrids: generate MSL per formula at runtime (`newLibraryWithSource`) from an opcode list, as KF
  and F3 do with OpenCL (src: [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md) §Code Generation). This is
  permitted on iOS for MSL source (unverified for App Store review, check).

**Maths** (`almondbread-math`): a `Formula` enum (not a trait object in the inner loop) passed to
`orbit.rs`, matched outside the limb-specialised loop; a parallel split per formula in
`parallel.rs` (Mandelbrot, Mandelbar and Burning Ship share three products); `selection.rs`
Newton and period detection with a per-formula derivative, 2×2 for non-conformal formulas; `rays.rs`
stays Mandelbrot-only. Oracle fixtures per formula in `tests/fixtures`.

**Core** (`almondbread-core`): `settings.rs` gains `formula`; `bla.rs` per-formula coefficients
plus the matrix variant; `la.rs` gated to Mandelbrot; `oracle.rs`, `reference.rs`, `orbit.rs`
per-formula steps; `components.rs` Mandelbrot-only; `cycles.rs` and the interior probe per
formula; places, tours and journey regions tagged with a formula, with Mandelbrot-only content
filtered out; autopilot and target lock unchanged but re-tuned per formula.

**Capabilities** (one struct exported over UniFFI so Swift hides what does not apply):
perturbation, rebasing references (1 or per phase or critical-orbit), BLA (scalar or matrix), LA,
SA seeds, interior derivative test, interior probe, closed-form components, DE, seek / Newton
nucleus, atom-domain period, π meter, external rays, orbit overlay, places / tours / journeys.

**Verification per formula**: CPU oracle, `verify` metrics, regress sessions, and selftests
through both apps, as for Mandelbrot today.

## Open questions

- Does critical-orbit rebasing for deep Julia hold in float32? (new, test in step 5)
- Needle detection heuristic for Burning Ship in float32 ([Burning Ship and abs variations](../formulas/burning-ship.md)).
- [Can perturbation and rebasing handle Nova, Newton and Magnet formulas?](../questions/perturbation-convergent-formulas.md) for Nova/Newton/Magnet.
- Measured 3D cost on M1 Pro / iPad (only estimates here).

# Citations

- [mathr: Deep zoom (reference page)](../sources/mathr-deep-zoom-page.md)
- [mathr: Deep zoom theory and practice (2021)](../sources/mathr-deep-zoom-theory.md)
- [mathr: Deep zoom theory and practice (again) (2022)](../sources/mathr-deep-zoom-theory-again.md)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](../sources/fractalforums-zhuoran-rebasing-bla.md)
- [Fraktaler 3 (documentation site, v3.1)](../sources/fraktaler-3-site.md)
- [Kalles Fraktaler 2 + manual](../sources/kf-manual.md)
- [Kalles Fraktaler 2 + (home page)](../sources/kf-page.md)
- [Fractal Zoomer README](../sources/fractalzoomer-readme.md)
- [Fractalshades README](../sources/fractalshades-readme.md)
- [Ultra Fractal help: Perturbation equations](../sources/ultrafractal-perturbation-equations.md)
- [HPDZ.NET: Burning Ship and Nova deep zooms](../sources/hpdz-animations.md)
- [Inigo Quilez: Mandelbulb](../sources/quilez-mandelbulb.md)
- [Daniel White: Mandelbulb (skytopia)](../sources/skytopia-mandelbulb.md)
- [Wikipedia: Mandelbulb](../sources/wikipedia-mandelbulb.md)
- [Wikipedia: Mandelbox](../sources/wikipedia-mandelbox.md)
- [Hvidtfeldt: Distance Estimated 3D Fractals (blog series)](../sources/hvidtfeldt-de-3d-fractals.md)
- [Mandelbulber v2 repository](../sources/mandelbulber2-readme.md)
- [AllTools: Mandelbulb 3D Explorer (WebGL)](../sources/alltools-mandelbulb-explorer.md)
- [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)
- [Wikipedia: Fractal flame](../sources/wikipedia-fractal-flame.md)
- [Wikipedia: Newton fractal](../sources/wikipedia-newton-fractal.md)
- [Wikipedia: Lyapunov fractal](../sources/wikipedia-lyapunov-fractal.md)
- [Frax: What is Frax?](../sources/frax-about.md)
- [Frax help: Explore (precision limit)](../sources/frax-help-explore.md)
- [Burning Ship and abs variations](../formulas/burning-ship.md)
- [Hybrid formulas](../formulas/hybrids.md)
- [Apple docs and SDK headers: MetalFX framework](../sources/apple-metalfx-docs.md)
- [Landscape mode: fly over the Mandelbrot set as 2.5D terrain](../decisions/landscape-mode.md)
