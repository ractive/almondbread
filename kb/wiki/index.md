---
okf_version: "0.1"
---

# Index

Catalogue of every wiki page, one line each. Updated on every ingest.

## Meta

- [Overview](overview.md) — the current big picture
- [Ideas](ideas.md) — backlog of the user's ideas with first thoughts
- [Roadmap](roadmap.md) — milestone status (driver-maintained)
- [Log](log.md) — chronological record of ingests, research runs, queries, lint passes

## Sources

- [AllTools: Mandelbulb 3D Explorer (WebGL)](sources/alltools-mandelbulb-explorer.md) — Anonymous WebGL Mandelbulb page: float32 ray marching shows artefacts by ~1e4–1e5 zoom (low reliability) (skimmed)
- [Apple device tooling and iOS process APIs (devicectl, xcodebuild provisioning, os/proc.h)](sources/apple-device-tooling.md) — Xcode 26.6 devicectl (install, launch --console, copy from the app container), xcodebuild automatic provisioning flags, os_proc_available_memory (digested)
- [Apple tech talk: Explore GPU advancements in M3 and A17 Pro (Dynamic Caching)](sources/apple-dynamic-caching-talk.md) — An Apple developer tech talk (video) on the GPU in M3 and A17 Pro, mainly Dynamic Caching. (unread)
- [Apple Developer Forums thread 718737: FP64 emulation on Apple GPUs](sources/apple-forums-fp64-emulation.md) — An Apple Developer Forums thread on emulating 64-bit floating point on Apple GPUs, which have no FP64 hardware. (unread)
- [Apple docs: MTLCompileOptions mathMode](sources/apple-metal-compile-options.md) — Apple's reference page for the `mathMode` property of `MTLCompileOptions`, the per-library setting for how aggressively the Metal compiler may... (unread)
- [Apple Metal Feature Set Tables (May 2026)](sources/apple-metal-feature-set-tables.md) — Apple Metal Feature Set Tables (May 2026): GPU families per MetalFX feature (temporal scaler Apple7+, denoiser Apple9+) (digested)
- [Apple docs and SDK headers: MetalFX framework](sources/apple-metalfx-docs.md) — Apple docs and SDK headers for MetalFX: spatial/temporal scalers, jitter and motion conventions, texture usage, reactive mask, dynamic resolution (digested)
- [Apple sample: Applying temporal antialiasing and upscaling using MetalFX](sources/apple-metalfx-taa-sample.md) — Apple sample code: temporal antialiasing and upscaling with MetalFX (digested)
- [Apple: MetricKit MXHangDiagnostic and main-thread hangs](sources/apple-metrickit-hang-diagnostics.md) — Apple docs on MetricKit `MXHangDiagnostic` and app hangs (thresholds, delivery); for the hang watchdog (skimmed)
- [Apple simulator and UI-test tooling (simctl, xcodebuild, XCTest)](sources/apple-simulator-tooling.md) — Xcode 26.6 simctl, xcodebuild test and XCTest touch synthesis: what the iOS simulator checks use (digested)
- [Apple SwiftUI and Observation documentation, and the macOS 26 SwiftUI call stacks](sources/apple-swiftui-observation.md) — Observation and SwiftUI update docs, plus our macOS 26 call stacks: the grouped Form is a non-lazy SwiftUI layout
- [Bevy 0.20 release notes](sources/bevy-0-20-release-notes.md) — Bevy 0.20 release notes (2026-10-08): WESL shader migration, Feathers widgets, no editor; seven breaking releases since 0.14 (skimmed)
- [Wolfgang Beyer: Mandelbrot zoom sequence (Wikimedia Commons)](sources/beyer-mandelbrot-zoom-sequence.md) — Wolfgang Beyer's 15-image Commons zoom into seahorse valley (2005): centre and width of each view, to 6e10 (digested)
- [BIDS colormap: viridis, magma, inferno, plasma](sources/bids-viridis-colormaps.md) — viridis, magma, inferno, plasma colour maps (CC0); 256-entry data (digested)
- [Dave Boll's 1991 π post, reproduced by Mark VandeWettering (brainwagon, 2011)](sources/boll-pi-post.md) — Dave Boll's 1991 sci.math post (via brainwagon, 2011): neck escape counts 33, 315, 3143 ... whose product with ε is π; a C program that fails at ε = 1e-9 (digested)
- [Brockmoeller, Scherz, Srkalovic: Pi in the Mandelbrot set everywhere (arXiv 2025)](sources/brockmoeller-pi-everywhere.md) — Brockmoeller, Scherz, Srkalovic 2025 (arXiv): first proof at the neck -3/4, π at every parabolic parameter along its rays, N = π/ε + O(1) (digested)
- [Chéritat: Mandelbrot set (wiki-draw rendering techniques)](sources/cheritat-mandelbrot-rendering.md) — Rendering-techniques page by Arnaud Chéritat, a complex-dynamics mathematician (Toulouse), on "Techniques for computer generated pictures in... (digested)
- [Nuñez et al.: cividis, a colour map for colour vision deficiency](sources/cividis-paper.md) — cividis (PLOS ONE 2018): CVD-optimised blue-yellow map; article CC0, data Battelle BSD-style (digested)
- [cmocean: colour maps for oceanography](sources/cmocean.md) — cmocean oceanography colour maps (MIT), incl. cyclic phase (digested)
- [NIST CODATA 2022 recommended values (Planck units, Bohr radius, proton radius, h, c)](sources/codata-2022-constants.md) — CODATA 2022 values from NIST: Planck length and time, Bohr radius, proton radius, h, c (digested)
- [colorcet: the CET perceptually uniform colour maps](sources/colorcet.md) — Kovesi's CET colour maps (CC BY 4.0): linear, cyclic, colour-blind sets (digested)
- [Cosmic history timeline: epoch times after the Big Bang (PDG, Planck, NASA, ESA, CERN)](sources/cosmic-history-timeline.md) — Times after the Big Bang (first stars, CMB, nucleosynthesis, transitions, inflation, Planck epoch) for the cosmic size scale (digested)
- [Cosmic size scale: sources for galaxies, cosmology, life, particles and big numbers](sources/cosmic-scale-references.md) — Sizes, distances and big numbers for the cosmic size scale: Milky Way, universe, cells, nuclei, quark limit, googol, Shannon number (digested)
- [Crameri: Scientific colour maps](sources/crameri-scientific-colour-maps.md) — Crameri Scientific colour maps v8 (MIT): batlow, cyclic romaO/vikO/bamO, ... (digested)
- [Deep-zoom videos: record and landmark YouTube descriptions](sources/deep-zoom-videos.md) — YouTube descriptions of record and landmark deep zooms: e275 (2010) to e122768 (2022), iterations, `.kfr` and coordinate sharing, Maths Town record rules (digested)
- [mandelbrot-webgpu README (Desarso)](sources/desarso-mandelbrot-webgpu-readme.md) — The README of a browser Mandelbrot renderer (WebGPU, with a WebGL2 fallback), live at mandelbrot.gabrielmalek.com. (digested)
- [Dinkydau: evolution zooms, Julia morphing simulation, ExploreFractals](sources/dinkydau-evolution-journals.md) — Dinkydau on evolution zooms, Julia-morph depth rules (morph at 1/2, doubling at 3/4 of log-depth), Newton zooming, ExploreFractals inflections (digested)
- [Gerald Edgar: Pi and the Mandelbrot set (with the 1992 alt.fractals heuristics)](sources/edgar-pi-and-mandelbrot.md) — Gerald Edgar's page on Boll's π (1992 alt.fractals post): Petry's tan heuristic at the cusp, Edgar's f∘f integral at the neck, -5/4 (digested)
- [Fractal Forums: How deep was the deepest Mandelbrot zoom you've ever made? (2016)](sources/fractalforums-deepest-zoom-poll.md) — 2016 forum poll of members' deepest zooms (e450 to ~e22522) (skimmed)
- [fractalforums.org: Interior detection for deep Mandelbrot set zooms (topic 4802)](sources/fractalforums-interior-detection.md) — Archived single-page thread started by GBy (fractalshades author) on 25 June 2022: interior detection via the derivative `dz/dZ_1` combined with... (digested)
- [fractalforums.com: Mandel Machine board](sources/fractalforums-mandel-machine.md) — Legacy fractalforums.com board for Mandel Machine, a closed-source freeware deep-zoom Mandelbrot renderer by Botond Kósa. (unread)
- [Fractal Forums: Pathfinding in the Mandelbrot set - Revisited (2019)](sources/fractalforums-pathfinding-revisited.md) — 2019 thread: the path to a minibrot is recorded in its shells; 2/4/8 arms at 3/4, 7/8, 15/16 of the depth; "the path you choose BUILDS the shape" (digested)
- [fractalforums.org: Another solution to perturbation glitches (topic 4360)](sources/fractalforums-zhuoran-rebasing-bla.md) — Archived forum thread where Zhuoran (later author of Imagina) posted the rebasing idea on 3 Aug 2021. (digested)
- [fractalset.com viewer and gradient editor](sources/fractalset-viewer.md) — fractalset.com viewer: 24 palettes, log density 2^(d/10), sine ping-pong mapping (proprietary, described only) (digested)
- [Fractalshades README](sources/fractalshades-readme.md) — The README of Fractalshades (GitHub `GBillotey/Fractalshades`, MIT, pushed 2026-08-30), a Python package for deep-zoom fractal rendering with a... (digested)
- [FractalShark README](sources/fractalshark-readme.md) — The README of FractalShark (GitHub `mattsaccount364/FractalShark`, GPL-3.0, pushed 2026-10-12): a Mandelbrot-only deep-zoom renderer for NVIDIA... (digested)
- [Fractal Zoomer README](sources/fractalzoomer-readme.md) — The short README of Fractal Zoomer (GitHub `hrkalona/Fractal-Zoomer`, GPL-3.0, pushed 2026-10-13), a Java fractal program with hundreds of formulas. (digested)
- [Fractint documentation: palette maps (.map)](sources/fractint-map-format.md) — Fractint .map format: up to 256 `R G B` lines, comments after the triplet; bundled maps not free (digested)
- [Fractint / Xfractint help: drawing passes](sources/fractint-passes-help.md) — Fractint/Xfractint help on drawing passes: solid guessing, boundary tracing, tesseral (Mariani–Silver) and their failure modes (digested)
- [Fraktaler 3 (documentation site, v3.1)](sources/fraktaler-3-site.md) — The manual of Fraktaler 3 version 3.1 (2025-12-15): a cross-platform (Linux, Windows, Android, Web) renderer for "fast deep zooming of hybrid... (digested)
- [Frax: What is Frax?](sources/frax-about.md) — Frax (iOS) about page: Mandelbrot, Julia, ~20 formulas, lit 3D height, real-time lights, ~1e12+ depth (skimmed)
- [Frax help: Explore (precision limit)](sources/frax-help-explore.md) — Frax help page: the 64-bit limit (~2e14) is "absolute and final"; image sizes per tier (skimmed)
- [Fixed-size multiplication kernels on M1 (gmp-devel, 2024)](sources/gmp-fixed-size-mul.md) — A gmp-devel reply by Albin Ahlbäck, 5 April 2024, in the thread "How to calculate cycles/limb in assembly routines". (digested)
- [GMP on Apple M1 (gmp-discuss, 2020)](sources/gmp-m1-discuss.md) — A mailing-list post, "Surprise performance from Apple M1", by GMP's lead developer, 21 November 2020, after the project bought an M1 Mac mini. (digested)
- [Heiland-Allen: Patterns in Inflected Julia Sets (2025)](sources/heiland-allen-inflected-julia-sets-2025.md) — Alpaca 2025 paper: inflected Julia sets, disk/tree/line building blocks, evolution, half the zoom again per morph, automation open (digested)
- [Hacker News: Mandelbrot Set Explorer thread (2025)](sources/hn-mandelbrot-site-thread.md) — HN thread on mandelbrot.site (2025, 86 points): "zoom forever", jerky zoom, iteration scaling, URL sharing (skimmed)
- [HPDZ.NET: Burning Ship and Nova deep zooms](sources/hpdz-animations.md) — HPDZ.NET: first Burning Ship deep zoom (~2.35e-100), Nova needing 192 bits at 4.3e-21, brute-force era (skimmed)
- [Hvidtfeldt: Distance Estimated 3D Fractals (blog series)](sources/hvidtfeldt-de-3d-fractals.md) — Hvidtfeldt's Distance Estimated 3D Fractals series: folding, Mandelbulb and Mandelbox DE, dual numbers (snippets only) (skimmed)
- [IAU astronomical unit (2012 Resolution B2) and the light-year](sources/iau-astronomical-units.md) — IAU astronomical unit (exact) and the light year (digested)
- [Imagina HInfLAEvaluator (source code)](sources/imagina-hinfla-evaluator.md) — The main Mandelbrot evaluator of Imagina (pre-rewrite `master`, last pushed 2024-01-23), by Zhuoran, the originator of rebasing and of this "LA"... (skimmed)
- [Imagina README](sources/imagina-readme.md) — The very short README of Imagina (GitHub `5E-324/Imagina`), Zhuoran's "fast fractal renderer & explorer". (digested)
- [Inter typeface (Rasmus Andersson), release 4.1](sources/inter-font.md) — Inter 4.1 typeface (OFL 1.1): the overlay font; SemiBold used for the SDF atlas (digested)
- [Kalles Fraktaler 2 + manual](sources/kf-manual.md) — The full KF2+ manual (about 160 KB of text): limits, known bugs, a change log from 2017 to kf-2.15.5.2 (2022), TODO, build notes, a menu-by-menu... (skimmed)
- [Kalles Fraktaler 2 + (home page)](sources/kf-page.md) — Home page of Kalles Fraktaler 2+ (KF2+), the fork of Karl Runmo's Kalles Fraktaler maintained by Claude Heiland-Allen: status, limits, downloads,... (digested)
- [Kisaka: Julia sets in the Mandelbrot set and small Mandelbrot sets (seminar abstract)](sources/kisaka-julia-sets-in-mandelbrot.md) — Kisaka 2015 (seminar abstract): zooming in shows a Julia-like figure, then nested preimages, then a small Mandelbrot set; the stage order the journey's embedded Julia targets rely on (skimmed)
- [Klebanoff: π in the Mandelbrot set (Fractals 9(4), 2001)](sources/klebanoff-pi-in-mandelbrot.md) — Klebanoff, Fractals 9(4) 2001: first proof that √ε·N → π at the cusp 1/4 + ε; Boll's neck and cusp tables (digested)
- [Kopf et al. 2007: Joint Bilateral Upsampling](sources/kopf-2007-joint-bilateral-upsampling.md) — Kopf et al. 2007: upsample a low-resolution solution with a full-resolution guide image; needs a cheap guide whose edges match (digested)
- [Kovesi: Good Colour Maps: How to Design Them](sources/kovesi-good-colour-maps.md) — Kovesi 2015: lightness carries detail; recipes for vivid cyclic maps (digested)
- [luma.gl: GPU floating-point precision techniques](sources/luma-gl-gpu-precision.md) — A guide page in the luma.gl (vis.gl, WebGL/WebGPU) docs on ways to get more than f32 precision on GPUs: double-single (float-float),... (digested)
- [Make: Arthur Ganson's "Machine with Concrete"](sources/make-ganson-machine-with-concrete.md) — Make 2012 on Ganson's Machine with Concrete: twelve 1:50 worm pairs at 200 rpm, last gear in concrete, over 2 trillion years per turn (skimmed)
- [How mandelbrot.site was built](sources/mandelbrot-site.md) — How mandelbrot.site was built: Rust/Wasm tiles, perturbation, share links, PNG with params, video recording (skimmed)
- [Mandelbulber v2 repository](sources/mandelbulber2-readme.md) — Mandelbulber v2 repo: GPL 3D ray marcher, OpenCL multi-GPU, keyframes; no documented precision limit (skimmed)
- [mathr: Adaptive super-sampling using distance estimate (2014)](sources/mathr-adaptive-supersampling-de.md) — mathr 2014: supersample only pixels with DE < 4 px; on a dense view no better than uniform supersampling (digested)
- [mathr: Deep zoom (reference page)](sources/mathr-deep-zoom-page.md) — mathr's maintained deep-zoom reference page as used in Fraktaler 3: perturbation, rebasing, BLA build/lookup, matrix and abs BLA, DE, interior detection (digested)
- [mathr: Deep zoom theory and practice (again) (2022)](sources/mathr-deep-zoom-theory-again.md) — Short follow-up blog post (21 Feb 2022) explaining Zhuoran's two 2021 ideas from fractalforums.org: rebasing and bilinear approximation (BLA),... (digested)
- [mathr: Deep zoom theory and practice (2021)](sources/mathr-deep-zoom-theory.md) — Blog post (14 May 2021) by the maintainer of Kalles Fraktaler 2+ summarising the state of deep-zoom rendering as implemented in KF just before... (digested)
- [mathr blog: atom domain size, Misiurewicz domains, periodicity scan](sources/mathr-domains-and-periodicity-scan.md) — mathr: atom-domain size formula, Misiurewicz domains, periodicity scan listing every nucleus in a view (digested)
- [mathr blog: navigating embedded Julia sets in the hairs](sources/mathr-embedded-julia-navigation.md) — mathr: navigating embedded Julia sets in the hairs by external-angle blocks; binary period subdivision (digested)
- [mathr: Practical interior distance rendering (2014)](sources/mathr-interior-distance.md) — Blog post (2 Nov 2014) on computing Mandelbrot interior distance estimates in practice: find the period from atom-domain partials, find the... (digested)
- [mathr blog: automated Julia morphing (2016-2021)](sources/mathr-julia-morphing-posts.md) — mathr 2016–2021: automated Julia morphing by angle extrapolation (O(p²)) and by alternating Misiurewicz/nucleus Newton (O(p)), symmetry trap (digested)
- [KF Movie Maker user guide (Maths Town)](sources/maths-town-kfmoviemaker-guide.md) — KF Movie Maker guide: KF keyframes into paid After Effects; weeks of export, manual keyframes, crashes (skimmed)
- [Maths Town videos and video descriptions](sources/maths-town-videos.md) — Maths Town video catalogue and archived descriptions: depths to 4e2656, render times of weeks to months (skimmed)
- [metal-benchmarks README (Apple GPU microarchitecture)](sources/metal-benchmarks-readme.md) — Philip Turner's long README describing the M1/M2 (Apple GPU families 7 and 8) GPU from his own microbenchmarks: per-chip FLOPS, on-chip memory,... (digested)
- [Mitchell 1987: Generating Antialiased Images at Low Sampling Densities](sources/mitchell-1987-adaptive-sampling.md) — Mitchell 1987: low-density blue-noise sampling plus contrast-driven supersampling and reconstruction filters (digested)
- [MLX issue 3302: GPU watchdog kills long SDPA dispatch](sources/mlx-watchdog-issue.md) — A GitHub issue in Apple's MLX machine-learning framework, opened 24 March 2026 by user Thump604, titled "GPU watchdog kills process during... (digested)
- [Mu-Ency: Adjacency optimization (with Mariani/Silver and circle tiling)](sources/muency-adjacency-optimization.md) — Mu-Ency on adjacency optimisations: successive refinement, Mariani/Silver, circle tiling, and where each misses features (digested)
- [Mu-Ency: Atom Domain](sources/muency-atom-domain.md) — Glossary entry (oldest revision 1996, updated 2023-07-08) defining atom domains (dominant period regions), their computation and their size... (digested)
- [Mu-Ency: Bivariate Linear Approximation](sources/muency-bla.md) — Glossary entry in Robert Munafo's Mandelbrot Set Glossary and Encyclopedia (first version 2023-06-20, rewritten 2023-07-03, page updated... (digested)
- [Mu-Ency: floatexp](sources/muency-floatexp.md) — Very short glossary entry (2023-06-21) defining the floatexp number format. (digested)
- [Mu-Ency: named features and favourite views (coordinates)](sources/muency-named-places.md) — Mu-Ency feature entries and favourite views with coordinates: valleys, medallions, embedded Julia sets, to 1.5e-27 (digested)
- [Mu-Ency: Newton-Raphson zooming (embedded Julia set sizes)](sources/muency-newton-raphson-zooming.md) — Mu-Ency on zooming to a minibrot by Newton; Heiland-Allen's estimate that the 2-fold embedded Julia set is about s^(3/4) of the island size s; "Julia morphing" (digested)
- [Mu-Ency: Newton-Raphson method](sources/muency-newton-raphson.md) — Glossary entry (written 2010, updated 2023-06-22) on Newton's method and its use to find the nucleus of a mu-atom (hyperbolic component) of known... (digested)
- [Mu-Ency (Robert Munafo): pi and R2.C(1/2)](sources/muency-pi.md) — Mu-Ency R2.C(1/2) on π: neck counts one higher than Boll's (another convention), "occurs in all the cusps" (digested)
- [NASA NSSDCA planetary fact sheets (Earth, Moon, Jupiter, Sun, Neptune)](sources/nasa-planetary-fact-sheets.md) — NASA planetary fact sheets: radii of Earth, Moon, Jupiter, Sun; Neptune's orbit (digested)
- [Ottosson: A perceptual color space for image processing (OKLab)](sources/ottosson-oklab.md) — OKLab/OKLCH definition; why OKLab gradients beat sRGB/HSV lerp (digested)
- [Phil Thompson: Faster Mandelbrot set rendering with BLA (2023)](sources/philthompson-bla.md) — A blog post (9 May 2023) by the author of the JavaScript viewer Very Plotter, on adding experimental BLA in its v0.10.0 release. (digested)
- [Quilez: procedural colour palettes](sources/quilez-cosine-palettes.md) — cosine palettes a + b cos(2π(ct + d)); mean = a; integer c is seamless (digested)
- [Inigo Quilez: Mandelbulb](sources/quilez-mandelbulb.md) — Quilez on the Mandelbulb: triplex power 8, potential-gradient DE, ray marching, real time at 720p, orbit traps (digested)
- [Quilez: Smooth iteration count for generalized Mandelbrot sets](sources/quilez-smooth-iteration.md) — Short article by Inigo Quilez deriving the smooth (continuous) iteration count for polynomial fractals of any integer degree `d`, with a GLSL... (digested)
- [Robot Moon: The Mandelbrot Set (interactive explorer, deep zooms)](sources/robotmoon-mandelbrot-set.md) — Robot Moon's illustrated explorer page: named regions with coordinates and spans, deep galleries to 1e-111 (digested)
- [Schied et al. 2017: Spatiotemporal Variance-Guided Filtering (SVGF)](sources/schied-2017-svgf.md) — Schied et al. 2017 (SVGF): temporal accumulation plus variance-guided à-trous filtering of 1 spp path tracing (digested)
- [Daniel White: Mandelbulb (skytopia)](sources/skytopia-mandelbulb.md) — Daniel White's Mandelbulb history: whipped-cream quaternions, Nylander's power 8, 2D keeps more variety (digested)
- [Melinda Green: The Buddhabrot Technique](sources/superliminal-buddhabrot.md) — Melinda Green's Buddhabrot: orbit-density histogram, Nebulabrot limits, not deeply zoomable, Metropolis–Hastings (digested)
- [swift-mandelbrot README (drquicksilver)](sources/swift-mandelbrot-readme.md) — The README of a SwiftUI + Metal Mandelbrot explorer for iPhone, iPad and Mac. (digested)
- [Thall: Extended-precision floating-point numbers for GPU computation](sources/thall-extended-precision.md) — Andrew Thall's (Alma College) technical report CIM-007-01, dated March 2007, expanding a SIGGRAPH 2006 poster. (digested)
- [Google Turbo colour map](sources/turbo-colormap.md) — Google Turbo rainbow (Apache-2.0) (digested)
- [Ultra Fractal help: Anti-aliasing and drawing methods](sources/ultrafractal-antialiasing.md) — Ultra Fractal help on anti-aliasing (adaptive threshold, depth) and guessing vs linear drawing (digested)
- [Ultra Fractal help: gradients and colouring settings](sources/ultrafractal-gradients-help.md) — UF gradients: 400-entry wrapping ring, density/transfer/offset order, .ugr syntax (COLORREF colours) (digested)
- [Ultra Fractal help: Perturbation calculations](sources/ultrafractal-perturbation-calculations.md) — A user-facing help page of Ultra Fractal (commercial Windows/macOS fractal program) on how its perturbation mode works and how to tune it. (digested)
- [Ultra Fractal help: Perturbation equations](sources/ultrafractal-perturbation-equations.md) — The Ultra Fractal formula-writer's guide to deriving a perturbed formula and adding it to a user formula, using Mandelbrot as the worked example. (digested)
- [The UniFFI user guide (Mozilla)](sources/uniffi-user-guide.md) — Mozilla's guide to UniFFI: proc-macro interfaces, objects/records/errors, library-mode bindgen, Swift output and XCFramework packaging (skimmed)
- [Smooth and efficient zooming and panning (van Wijk & Nuij)](sources/van-wijk-nuij-smooth-zooming.md) — van Wijk & Nuij (InfoVis 2003): the optimal zoom-and-pan path between two views, closed form with one parameter ρ; used for fly to coordinates (skimmed)
- [Very Plotter (visualize-primes) README](sources/very-plotter-readme.md) — The short README of Phil Thompson's GitHub repository `visualize-primes`, which hosts Very Plotter, a browser (JavaScript) Mandelbrot explorer... (digested)
- [wgpu CHANGELOG (v27–v30) and Features docs](sources/wgpu-changelog-v30.md) — wgpu CHANGELOG v27–v30 and Features docs: subgroups, indirect dispatch, timestamps, immediates, passthrough shaders; no Metal fast-math control (skimmed)
- [Wikibooks: Fractals/emndl (emndl_autotune)](sources/wikibooks-emndl.md) — Wikibooks on emndl and emndl_autotune, an undocumented "find points that might look nice" search over nuclei of growing period (skimmed)
- [Wikibooks: Fractals/fraktaler-3](sources/wikibooks-fraktaler-3.md) — Wikibooks page (part of Adam Majewski's "Fractals" book) about Fraktaler 3: usage notes and examples, including the metadata found in Fraktaler 3... (unread)
- [Wikibooks: Mandelbrot set interior](sources/wikibooks-mandelbrot-interior.md) — Wikibooks chapter (Adam Majewski's "Fractals" book) on the interior of the Mandelbrot set: how to detect interior points and how to colour them. (unread)
- [Wikibooks: Triangle inequality average (TIA) colouring](sources/wikibooks-triangle-inequality.md) — Wikibooks page on averaging colouring methods, in particular Kerry Mitchell's Triangle Inequality Average (TIA) and the curvature average. (unread)
- [Wikipedia: Fast-growing hierarchy](sources/wikipedia-fast-growing-hierarchy.md) — f_ω is a unary Ackermann function, f_(ω+1)(64) > Graham's number (skimmed)
- [Wikipedia: Fractal flame](sources/wikipedia-fractal-flame.md) — Wikipedia: fractal flames (Draves 1992): chaos game, variations, log density, Electric Sheep, Apophysis (skimmed)
- [Wikipedia: Googolplex](sources/wikipedia-googolplex.md) — googolplex = 10^10^100, Kasner and Sirotta 1920, impossible to write out (skimmed)
- [Wikipedia: Graham's number](sources/wikipedia-grahams-number.md) — g₁ = 3↑↑↑↑3, g_n = 3↑^(g_(n-1))3, G = g₆₄; Gardner 1977 (skimmed)
- [Wikipedia: Knuth's up-arrow notation](sources/wikipedia-knuth-up-arrow-notation.md) — Knuth 1976: definition of a↑^n b, 3↑↑3 = 7 625 597 484 987, 3↑↑↑3 (skimmed)
- [Wikipedia: Kruskal's tree theorem (TREE(3))](sources/wikipedia-kruskal-tree-theorem.md) — TREE(3): far beyond Graham's number, faster than f_Γ₀, weak lower bound A^A(187196)(1) (skimmed)
- [Wikipedia: Lyapunov fractal](sources/wikipedia-lyapunov-fractal.md) — Wikipedia: Lyapunov fractals: logistic map with AB sequences, exponent colouring (skimmed)
- [Wikipedia: Mandelbox](sources/wikipedia-mandelbox.md) — Wikipedia: Mandelbox (Tom Lowe 2010): box and ball folds, scale, solid core, any dimension (skimmed)
- [Wikipedia: Mandelbulb](sources/wikipedia-mandelbulb.md) — Wikipedia: Mandelbulb (White & Nylander 2009): triplex n-th power, n = 8, ray-marched (skimmed)
- [Wikipedia: Misiurewicz point](sources/wikipedia-misiurewicz-point.md) — Wikipedia on Misiurewicz points M(k,n): six examples (tips, branch points, the seahorse spiral, whose label is off by one) (digested)
- [Wikipedia: Names of large numbers](sources/wikipedia-names-of-large-numbers.md) — googolplexian = 10^googolplex, an informal name (skimmed)
- [Wikipedia: Newton fractal](sources/wikipedia-newton-fractal.md) — Wikipedia: Newton fractal: basins, relaxed Newton, Nova (Derbyshire), colouring by root (skimmed)
- [Wikipedia: Observable universe (atom count)](sources/wikipedia-observable-universe.md) — about 10^80 atoms in the observable universe (Eddington number) (skimmed)
- [Wikipedia: Plotting algorithms for the Mandelbrot set](sources/wikipedia-plotting-algorithms.md) — Encyclopedia survey of classic Mandelbrot rendering: escape time, colouring (histogram, smooth, exponential/cyclic, HSV/LCH), distance estimates,... (digested)
- [Wikipedia: Skewes's number](sources/wikipedia-skewes-number.md) — Skewes' bounds e^e^e^79 < 10^10^10^34 (1933, RH) and e^e^e^e^7.705 < 10^10^10^964 (1955) (skimmed)
- [Wikipedia: Steinhaus–Moser notation (mega, Moser's number)](sources/wikipedia-steinhaus-moser-notation.md) — mega between 10↑↑257 and 10↑↑258; Moser's number < 3→3→4→2, ≪ Graham's number (skimmed)
- [Wikipedia: Tetration](sources/wikipedia-tetration.md) — linear approximation of tetration for real heights (Hooshmand 2006), continuity and smoothness (skimmed)
- [Wikipedia: XaoS](sources/wikipedia-xaos.md) — Encyclopedia article on XaoS, the GPL real-time fractal zoomer by Jan Hubička and Thomas Marsh, mainly notable here for describing the Hubička... (digested)
- [WWDC22 session 10103: Boost performance with MetalFX Upscaling](sources/wwdc22-metalfx-upscaling.md) — WWDC22 session 10103: MetalFX spatial and temporal upscaling, inputs, jitter example (digested)
- [WWDC25 session 211: Go further with Metal 4 games](sources/wwdc25-metal4-games.md) — WWDC25 session 211: Metal 4 games, MetalFX frame interpolation and denoised upscaler, exposure and reactive-mask advice (digested)
- [XaoS: Algorithms document](sources/xaos-algorithms.md) — XaoS algorithms document: frame reuse, line guessing, dynamic resolution, periodicity checking only after guessing (digested)
- [XaoS README](sources/xaos-readme.md) — The README of XaoS (GNU XaoS), a real-time interactive fractal zoomer in development since 1996. (digested)
- [zoomasm (zoom video assembler)](sources/zoomasm.md) — manual of zoomasm 3.4 (2025), the exponential-map strip reprojector and colouring tool for zoom videos (digested)

## Techniques

- [Adaptive and sparse sampling, reconstruction, early termination](techniques/adaptive-sampling.md) — adaptive/sparse sampling, reconstruction and early termination; R3 measured: none pays in dense regions except cheaper samples
- [Anti-aliasing and supersampling](techniques/anti-aliasing.md) — uniform jitter and subframe accumulation, gamma-correct resolve, DE coverage; as built (FL): progressive adaptive AA in linear light, capped by the supersampling setting
- [Automatic maximum-iteration control](techniques/auto-iterations.md) — KF's undocumented auto iterations and zoom-out bug, Fraktaler caps, statistics + hysteresis rule (inference); as built: unresolved ≠ interior, extension passes, learned limit, floor from the reference (M5); compute further (background passes of ×4 the limit up to the Upper limit, after AA, HUD "not finished"), the learner's 4× cap and the neck gap
- [Bivariate linear approximation (BLA)](techniques/bla.md) — mathr's binary-tree BLA: single-step radius variants, merge rule, table build and lookup pseudocode, matrix/abs variants, costs; contradictions on the radius; as built (M4/M3b): auto only, no gain at f64 depths, previews wait for a new reference's table (2026-10-17)
- [Colouring methods](techniques/coloring-methods.md) — transfer functions, log-potential palette, histogram, phase, slopes/normal maps, TIA/curvature/stripe averages; as built (FL): OKLab palettes, looks, palette anti-aliasing by DE, outline-then-light
- [Distance estimation](techniques/distance-estimation.md) — Milnor exterior DE, Koebe ¼, pixel-scaled dual numbers for float32, DE through skips, AA/boundary tests
- [Double-float (float-float) arithmetic](techniques/double-float.md) — float-float error-free transforms, costs, Metal reassociation and math-mode pitfalls; M7 as built: safe-math library, known-answer test, fx verify renderer, 7–12× / 27–52× slower than float32
- [Exponential map (log-polar) rendering](techniques/exponential-map.md) — log-polar strips (9:1, zoom 2 per keyframe), KF/zoomasm/Fraktaler practice, geometry
- [Extended range: floatexp, rescaled iterations, depth thresholds](techniques/extended-range-floats.md) — float32/64 range limits, floatexp, rescaled iterations with full floatexp steps, skip-table exponents, depth→type table
- [Frame reuse and progressive refinement](techniques/frame-reuse-and-progressive.md) — XaoS row/column reuse, KF reuse reference and interlacing; as built: M2 gesture previews, FL history reprojection with temporal upsampling, per-pixel refinement fade, focus-first tiles
- [Glitch detection (Pauldelbrot) and multi-reference correction (legacy)](techniques/glitch-detection.md) — Pauldelbrot's |Z+z|² < G|Z|² test and KF multi-reference correction (legacy; keep as debug overlay)
- [Hang prevention (loop guards, extreme-value tests, main-thread watchdog)](techniques/hang-prevention.md) — hang patterns at extreme zoom (inf/NaN depths, f64 step loops, saturating casts, absurd allocations), the guards and lints, 13 extreme-value tests, the main-thread watchdog and its reports, MetricKit
- [Guessing, border tracing and rectangle subdivision](techniques/guessing-and-subdivision.md) — border tracing, Mariani–Silver, KF guessing (speckles, 1.5× on OpenCL); as built in M5: interior-only fill of 16 × 16 tiles (border-first admission, bit-identical, gated)
- [Interior detection](techniques/interior-detection.md) — derivative test |dz/dz₁| < 1e-3 with perturbation/BLA, cardioid/bulb, periodicity, interior DE; as built: attracting-cycle probe (Newton on f^P + Kantorovich proof, 2026-10-17) and the closed-form cardioid / period-2 test at any depth
- [Linear approximation with period-aligned stages and AT (Imagina)](techniques/linear-approximation-imagina.md) — Imagina's period-aligned LA stages (non-linear first step, Chebyshev thresholds, O(1) walk) and the AT minibrot shortcut (AT code removed 2026-10-17, never enabled); as built in M6: LA on deep nucleus references only
- [Local series approximation (tile and pixel seeds)](techniques/local-series-approximation.md) — second-order expansion around tile and pixel seeds (quadtree); R3 CPU 5.2×; R5 GPU prototype; as built in M5: resumable seed rounds, per-tile gate, float32 deep seeds, 4–9× in dense/deep views
- [Palette design for fractals](techniques/palette-design.md) — fractal palette design: key colours + mapping, OKLab spline, lightness rhythm, cyclic/mirror, deep tones, CVD, licences, .ugr/.map/JSON formats; as built: the palette library
- [Perturbation](techniques/perturbation.md) — reference orbit in high precision, per-pixel delta z ← 2Zz + z² + c in floats; loop, costs, pitfalls
- [Faster deep references: three-thread orbit, progressive delivery, cheap Newton](techniques/parallel-reference-orbit.md) — R6: three-thread squaring orbit (bit-identical), progressive delivery of centre references, Newton with an f64 derivative, short-product round-down fix; measured
- [π in the Mandelbrot set (neck and cusp escape counts)](techniques/pi-in-the-mandelbrot-set.md) — Boll's π at the neck (N·ε) and cusp (N·√ε): proofs, O(ε) / O(√ε) errors measured, f64 drift, compensated shifted count, the π meter
- [Rebasing (Zhuoran, 2021)](techniques/rebasing.md) — Zhuoran's rebase rule (|Z+z| < |z| or reference exhausted → z = Z+z, m = 0); one reference per critical point/phase; float32 pitfalls
- [Sample reuse while zooming and panning (sample cache in fractal coordinates)](techniques/sample-reuse.md) — reuse samples while zooming/panning: XaoS row/column approximation, exact-position sample cache (R4), and the production cache as built: plane-fixed lattice in a toroidal tile window, GPU clear/resample/shade, per-tile planning, carry-over batches, the cache check (code removed 2026-10-17: opt-in, never default)
- [Reference orbit: precision, cost, storage, compression](techniques/reference-orbit-precision.md) — bits vs zoom rule, cost per iteration, MT3 threading, GPU NTT verdict, storage, Imagina orbit compression; our implementation and measured error growth
- [Reference selection: nucleus, period detection, Newton](techniques/reference-selection.md) — view centre vs nucleus; atom domain, box and ball period detection, Newton for the nucleus; Misiurewicz; measured period/Newton behaviour
- [Series approximation (legacy)](techniques/series-approximation.md) — SA recurrences, probe points, why BLA replaced it, Very Plotter SA-vs-BLA table, nanoMB (legacy)
- [Smooth (continuous) iteration count](techniques/smooth-coloring.md) — smooth iteration count μ, bailout ≥ 256, split uint n + float fraction at 1e6–1e9 iterations
- [Fog over unresolved pixels](techniques/unresolved-fog.md) — as built: pixels unresolved at the limit drawn as fog (lit by the resolved colours around, almost black inside) from a coverage/colour pyramid; ~1 ms at 3456 × 2234, 0 without unresolved pixels; orbit textures tried and dropped; the compute-further fade check

## Formulas

- [Burning Ship and abs variations](formulas/burning-ship.md) — abs variations: diffabs case analysis, rescaled form, deep needle, fold-capped matrix BLA
- [Hybrid formulas](formulas/hybrids.md) — KF stanzas and Fraktaler opcodes, per-operator perturbation, one reference and table per phase, codegen
- [Julia sets](formulas/julia.md) — delta step with B = 0; rebasing needs a zero in the reference orbit (open)
- [Mandelbar (Tricorn)](formulas/mandelbar.md) — Tricorn: conjugated delta, 2×2 real BLA matrix
- [Mandelbrot set (power 2)](formulas/mandelbrot.md) — power-2 baseline: perturbed step, rebasing, BLA coefficients and radius, derivatives, rescaling
- [Multibrot (integer power p)](formulas/multibrot.md) — integer power p: binomial delta, BLA Jacobian pZ^(p−1), depth limits
- [Other formulas: feasibility of deep zoom](formulas/others-feasibility.md) — non-integer powers, Nova/Newton/magnet/lambda, multiple critical points, Mandelbulb, Lyapunov; contradictions

## Implementations

- [Fractal Zoomer](implementations/fractal-zoomer.md) — Java, 500+ formulas, BLA, rebasing on convergent formulas, GPL-3.0, active
- [Fractalshades](implementations/fractalshades.md) — Python perturbation + BLA for Mandelbrot and Burning Ship, derivative interior detection, MIT
- [FractalShark](implementations/fractalshark.md) — CUDA LA/LAv2, 2×32+exp type, GPU NTT reference, orbit compression, ~1e650452, GPL-3.0, active
- [Fraktaler 3](implementations/fraktaler-3.md) — BLA + rebasing, wisdom number-type choice, OpenCL tiles, opcode hybrids, v3.1 2025-12, AGPL
- [Imagina](implementations/imagina.md) — period-staged LA + AT, double/FloatExp per stage, being rewritten, last push 2024-01, AGPL
- [Kalles Fraktaler 2 +](implementations/kalles-fraktaler-2.md) — KF2+: SA/NanoMB1, rebasing since 2.15.5, number-type ladder, frozen 2022-06, AGPL
- [Mandel Machine](implementations/mandel-machine.md) — Windows explorer of the 2014 era, little documented (stub)
- [mandelbrot-webgpu (Desarso)](implementations/mandelbrot-webgpu.md) — float32 WebGPU: GPU fixed-point reference, exponent-carrying deltas, second-order skip table, GPL-3.0
- [SuperFractalThing](implementations/superfractalthing.md) — K.I. Martin's 2013 origin of perturbation and SA (stub)
- [swift-mandelbrot (drquicksilver)](implementations/swift-mandelbrot.md) — SwiftUI/Metal for iOS and Mac: Float → double-float → perturbation, BLA + rebasing, no licence
- [Ultra Fractal 6](implementations/ultra-fractal.md) — commercial; automatic extra references, hand-written perturbation equations
- [Very Plotter](implementations/very-plotter.md) — JavaScript BLA with auto-epsilon and the public SA-vs-BLA benchmark, MIT, dormant
- [XaoS](implementations/xaos.md) — real-time zoomer with Hubička frame reuse, no deep zoom, GPL-2.0+

## Platform

- [Apple GPU architecture for compute](platform/apple-gpu-architecture.md) — M1-class GPU: fp32/fp16 rates, no fp64, int mul 1/8, SIMD 32, registers, occupancy, dynamic caching
- [GPU watchdog and chunked, resumable kernels](platform/gpu-watchdog-and-chunking.md) — ~5 s per-command-buffer watchdog, resumable per-pixel state, compaction, chunk scheduler
- [iOS and iPadOS port considerations](platform/ios-ipad-considerations.md) — port notes: memory, thermal, unknown iOS watchdog, background limits (mostly inference)
- [MetalFX upscaling (temporal, spatial) for motion sample reuse](platform/metalfx.md) — MetalFX temporal/spatial upscaling from Rust (objc2-metal-fx): conventions verified, 3× limit, GPU cost and memory, quality on fractal flights (R4: no gain over our compose)
- [iOS and iPadOS on a device: first run, signing, memory, thermal, background](platform/ios-device.md) — first run on an iPad (`make ios-device`: signing, Developer Mode, troubleshooting), measured renderer memory per texel / per reference point and the memory budget, thermal and background policy, licence check
- [iOS and iPadOS on the simulator: setup, checks, what broke](platform/ios-simulator.md) — iOS 26.5 simulator runs: self-test and XCTest touch UI test (pan, pinch, rotate, taps, buttons), the presented-handler crash and HEIC thumbnail deadlock fixed, compact HUD/button layout
- [Metal compute pipeline for a deep-zoom renderer](platform/metal-compute.md) — runtime compilation and mathMode, function-constant pipelines, shared buffers, indirect dispatch, timestamps; pipeline warm-up of all variants at start, gesture resolution probes and GPU clocks (2026-10-17)
- [Arbitrary precision on Apple Silicon (GMP, MPFR)](platform/mpfr-gmp-on-apple-silicon.md) — GMP on M1, fixed-size mul, MT3 threading, softfloat vs floatexp timings, no Metal bignum; our AlmondbreadMath vs GMP mpf measured; Rust port kernel findings
- [Overlay text: SDF font atlas in Rust and Metal](platform/overlay-text.md) — overlay labels: Inter SemiBold (OFL) as a signed-distance-field atlas, layout in Rust, bilinear SDF with a soft dark halo in the overlay kernel
- [SwiftUI performance: the control center sidebar](platform/swiftui-performance.md) — control center sidebar: 10 Hz stats re-running form rows, the non-lazy macOS grouped Form, debounced search, thumbnails pre-decoded at launch; `--ui-demo=perf` measures it; sidebar since reduced to settings only, lazy `LazyVStack` on the Mac (2026-10-16)
- [Render architecture: GPU backend trait, chunked scheduler, Metal backend (Rust)](platform/render-architecture.md) — Rust renderer: GpuBackend trait, chunked scheduler written once against it, Metal backend (objc2-metal) and CAMetalLayer surface, Host and app hosting as built (R2b: layer handover, one frame call per refresh, events), the drawable-wait finding (9.4 ms neck-zoom draw CPU) the pool-predictor fix, and FL's drawable pre-acquisition thread and compose pass; the motion sample cache section is history (deleted 2026-10-17)
- [Rust core to Swift: UniFFI and the XCFramework pipeline](platform/rust-swift-ffi.md) — Rust core to Swift: UniFFI proc macros, XCFramework build script, measured call costs (3.6 ns scalar, Vec return 1700x slower than a copy, frame call 2–7 µs), the Explorer interface (R2b), FFI rules

## Decisions

- [App stack: Swift, SwiftUI/AppKit and Metal](decisions/app-stack.md) — Swift + SwiftUI/AppKit + Metal compute, CPU arbitrary-precision reference orbits (accepted)
- [Mini-Mandelbrot atlas](decisions/minibrot-atlas.md) — catalogue of mini-Mandelbrots (period, nucleus, size, neighbourhood) as experimental maths feeding journey, places and game (proposed, waits for go)
- [Arbitrary precision: own Swift/C implementation, not GMP/MPFR](decisions/arbitrary-precision.md) — own multi-limb Swift arithmetic instead of LGPL GMP/MPFR, for iOS (accepted; built, beats GMP mpf to 1024 bits)
- [Rust core with a Rust Metal backend; Swift for UI only](decisions/rust-core.md) — all non-UI code in Rust (maths, core, scheduler, Metal backend via objc2-metal); Swift UI only (accepted; R1, R2a, R2b done: Rust maths 1.05x to 4x the Swift orbit speed, renderer bit-identical, apps on Rust with interaction parity, Swift renderer and maths deleted)
- [Landscape mode: fly over the Mandelbrot set as 2.5D terrain](decisions/landscape-mode.md) — 2.5D terrain from the distance estimate; altitude = zoom; L0 stills, L1 real-time prototype (geometry clipmap, layered σ)
- [Open-source release: licence, distribution, CI](decisions/open-source-release.md) — MIT + AI notice, signed/notarised dmg via GitHub releases, Homebrew cask, CI on push (accepted)
- [Controls: keyboard, pointer and touch](decisions/controls.md) — keyboard (WASD/arrows pan, Space zoom, Q/E zoom speed), pointer and touch controls; pointer/keyboard modes, crosshair zoom target, spring glide, edge panning (accepted, as built in M2.5); FL motion feel (eased velocities, flings, animated steps) and gamepad; autopilot (Z, pointer steers, adaptive speed, target lock and rescue, survey steering opt-in) and fly to coordinates (van Wijk & Nuij path at any depth, exact landing); driving levels (manual, cruise, autopilot, journey as clickable cockpit icons; Q/E pedals, reverse cruise, lane assist), journey with stops (J / Z / X), cosmic size scale (C: your window stays, the set grows, imaginary universes nest by Planck-length levels), full screen, render progress line, help popovers; compass (click to north, drag to turn), π meter

## Plans

One page per planned milestone (type `plan`; `hyalo find --view plans` lists them by priority).

- [F1: Julia view (companion inset)](plans/f1-julia-view.md) — F1: Julia set of the point under the pointer or finger in an inset next to the mini-maps; shallow, no refactor; improves the Julia tour (proposed, awaiting go; S)
- [F2: Pluggable-formula refactor](plans/f2-pluggable-formulas.md) — F2: `kFormula` function constant, `Formula` enum, capability struct over FFI; proof: bit-identical Mandelbrot fields, `bench` A/B ≈ 1.0 (planned; M)
- [F3: Multibrot z^p + c](plans/f3-multibrot.md) — F3: Multibrot `z^p + c`, p = 3..8, every per-formula hook with scalar arithmetic (planned; M)
- [F4: Deep Julia sets](plans/f4-deep-julia.md) — F4: deep Julia sets via critical-orbit rebasing (the Mandelbrot orbit of k; untested), Julia twin (planned; M)
- [F5: Mandelbar (Tricorn) and the non-conformal path](plans/f5-mandelbar.md) — F5: Mandelbar / Tricorn, the non-conformal path: 2×2 BLA, dual numbers, Jacobian Newton (planned; M)
- [F6: Burning Ship family](plans/f6-burning-ship.md) — F6: Burning Ship family: diffabs, fold-capped BLA, needle heuristic, places and journeys (planned; L)
- [F7: Hybrid formulas via an opcode model](plans/f7-hybrids.md) — F7: hybrids via an opcode model (old M8): per-phase references and BLA, runtime MSL (planned; XL)
- [F8: Nova, Newton and Magnet research spike](plans/f8-convergent-spike.md) — F8: one-week spike on perturbation for Nova / Newton / Magnet (planned; M)
- [Buddhabrot and Nebulabrot as a look](plans/buddhabrot.md) — BB: Buddhabrot / Nebulabrot as an optional shallow look, any time (planned; M)
- [3D worlds (Mandelbox first): not recommended now](plans/3d-worlds.md) — 3D worlds (Mandelbox first) as a separate mode: not recommended now (proposed, last; XL)
- [A11Y: Accessibility of the canvas and the Rust-drawn controls](plans/a11y-canvas-controls.md) — A11Y: cockpit, help badges, compass, mini-maps, Julia panel, π meter and cosmic scale as accessibility elements with labels, values and actions; the canvas as one sentence (built, awaiting review; S)

## Questions

- [Do Apple GPUs flush float32 denormals to zero?](questions/apple-gpu-denormals.md) — do Apple GPUs flush float32 subnormals?
- [What auto-maxIter rule should almondbread use?](questions/auto-iteration-heuristic.md) — which auto-maxIter rule to use
- [What exactly is the ball-method period recurrence?](questions/ball-period-recurrence.md) — exact ball-method period recurrence
- [How much faster is Imagina-style LA than binary-tree BLA on a GPU?](questions/bla-vs-la-benchmark.md) — BLA tree vs Imagina LA+AT (and SA) on the GPU (answered M6: LA 1.9–2.4× at deep needle minibrots, no gain elsewhere)
- [Which extended-precision emulation is fastest and correct on Apple GPUs?](questions/double-float-speed-on-m1.md) — fma float-float vs integer emulation on Apple GPUs; answered: fma float-float correct under .safe, broken under .fast, 7–12× slower than float32
- [At what fraction of the log-depth to a minibrot does each doubling appear?](questions/julia-morph-depth-fractions.md) — at which fractions of the log-depth to a minibrot the pattern doubles (1/2, 3/4, 7/8 …) and from where
- [What is the FFMA32 latency on Apple GPUs?](questions/ffma32-latency.md) — FFMA32 latency 2.21 vs 6 cycles in metal-benchmarks
- [How large is float32 perturbation error over long orbits?](questions/float32-error-long-orbits.md) — how wrong float32 deltas get over long orbits; answered (M7): within a 0.001–0.2 px grid shift, no visible artefacts
- [Is a higher-order (second-order) BLA worth it?](questions/higher-order-bla.md) — second-order BLA: 21× reported vs "nothing adopted" (contradiction)
- [Does Kalles Fraktaler 2+ implement BLA?](questions/kf-bla-support.md) — does KF2+ have BLA? (contradiction: KF page vs Mu-Ency)
- [Does local series approximation hold in the float32 perturbation kernel?](questions/local-sa-float32.md) — does local series approximation hold in the float32 perturbation kernel, and does the gain survive on the GPU? (answered R5: yes in dense/filament/deep-seahorse views, 3–8.5× GPU; no gain in interior-heavy views; carry the interior derivative, never restart it)
- [What does Metal's default math mode allow for runtime-compiled kernels?](questions/metal-fast-math-defaults.md) — default mathMode and reassociation/contraction in Metal; .fast erases TwoSum, .safe keeps it (M7)
- [What is the exact Metal GPU watchdog limit on macOS and iOS?](questions/metal-watchdog-limit.md) — exact watchdog threshold on macOS and iOS
- [Can perturbation and rebasing handle Nova, Newton and Magnet formulas?](questions/perturbation-convergent-formulas.md) — perturbation/rebasing for Nova, Newton, magnet (contradiction)
- [Why do published π escape counts differ by one (Mu-Ency) or disagree at ε = 1e-7 (Boll)?](questions/pi-escape-count-conventions.md) — why Mu-Ency's neck counts are Boll's + 1 and why Boll's 1e-7 entry (31415928) is off by one (answered)
- [How many bits does the reference orbit need?](questions/precision-bits-formula.md) — reference bits vs zoom; working rule
- [Can almondbread play a world-record zoom live, without post-processing?](questions/record-zoom-live-feasibility.md) — can a world-record zoom be played live without post-processing? (open; estimate: yes to ~1e1000–1e3000 along a known path)
- [How does Imagina/FractalShark reference-orbit compression work?](questions/reference-orbit-compression.md) — how Imagina/FractalShark compress orbits (answered from Imagina source; FractalShark part open)

## Synthesis

- [almondbread benchmarks and verification record](synthesis/benchmarks.md) — running record of almondbread measurements: benchmark set, M1 vs M2 timings, GPU-vs-CPU verify metrics, scheduler and interaction findings, reference-orbit speed vs GMP, R2a/R2b Rust vs Swift parity, M2.5 and FL interaction numbers, L1 landscape frame pacing; later sections: interior probe, autopilot throttle and GPU clocks, motion fixes (switch names in old sections are history, the switches were removed 2026-10-17)
- [Deep zoom Mandelbrot rendering techniques](synthesis/deep-zoom-rendering-techniques.md) — 2026 research report: settled pipeline, float32 constraints, renderer landscape, ranked roadmap
- [Flight benchmark: one fixed flight, a score, a regression guard](synthesis/flight-benchmark.md) — the flight benchmark: one fixed versioned 60 s flight through every regime, mode A throughput (settled full quality, checked against fresh and jump renders), mode B real time (headless and window), score vs the M1 Pro reference, hygiene, calibration, first noisy findings (heavy spot settles black after the dive)
- [Dense boundary regions: fewer pixel-iterations for the same image (R3)](synthesis/dense-region-sampling.md) — R3: fewer pixel-iterations in dense boundary regions; per-location numbers, recommendation (local SA in M5, interior-only tile fill), scheduler design, risks
- [Finding good places at extreme depth (1e100 to 1e100000+)](synthesis/finding-deep-places.md) — how the community finds deep places (shape stacking, Julia morphing, Newton zoom), known deep locations and records, density and colouring, what almondbread should build on `locate`
- [How big is big? Zoom depth against huge numbers, one dial per up-arrow](synthesis/how-big-is-big.md) — the "How big is big?" panel (removed from the app 2026-10-15; maths kept): zoom depth vs googol, googolplex, Skewes, mega, Graham's g₁ on one dial per up-arrow; linear super-log convention, marker table, recomputed facts, TREE(3)
- [Places: a catalogue of famous views to visit](synthesis/places.md) — the places catalogue: famous views with cited coordinates, conventions (span on the shorter side), generated Misiurewicz points and minibrots, endless dives (self-similarity factors, `locate --endless-seed`), checks, API, landscape-prototype use (Misiurewicz self-similarity factors and the external-ray generator are in "Endless dives")
- [Fractal palettes: research and the palette library](synthesis/fractal-palettes.md) — palette research and the 67-palette library: findings, licence table, what was dropped after the contact sheets, integration API
- [The −3/4 neck gap at 1e8–1e9 (escape statistics, what is true)](synthesis/neck-escape-statistics.md) — known hard case: the −3/4 neck gap at 1e8–1e9; ground truth to 1e7 (everything escapes, minimum ~8 500, medians 4e4–4e5, chaotic below the pixel), GPU float32 matches it statistically, the seam is real, why the app showed black (4× depth cap, list overflow, seed stall, long render in progress)
- [Other fractal types: what almondbread could support, at what cost](synthesis/other-fractal-types.md) — which other fractal types almondbread could support: per-family matrix (reuse, perturbation/BLA, depth, interior/DE/seek, effort, appeal), recommended order (Julia companion → formula refactor → Multibrot → deep Julia → Mandelbar → Burning Ship → hybrids), 3D verdict (separate XL mode; landscape first), pluggable-formula architecture
- [Motion sample reuse: fewer iterations per gesture frame (R4)](synthesis/motion-sample-reuse.md) — R4: fewer iterations per gesture frame: sample cache vs FL vs MetalFX on five flights, how many iterations can go (2–50 %), bookkeeping cost, integration design, risks; production as built (bookkeeping 0.1–0.6 ms, 55–92 % fewer iterations) (code removed 2026-10-17)

<!-- okf:index:begin -->

## Subdirectories

* [decisions](decisions/index.md)
* [formulas](formulas/index.md)
* [implementations](implementations/index.md)
* [plans](plans/index.md)
* [platform](platform/index.md)
* [questions](questions/index.md)
* [sources](sources/index.md)
* [synthesis](synthesis/index.md)
* [techniques](techniques/index.md)

## meta

* [Ideas](ideas.md)
* [Overview](overview.md)
* [Roadmap](roadmap.md)

<!-- okf:index:end -->
