# Deep-zoom renderer landscape and extending deep-zoom techniques beyond Mandelbrot (as of Oct 2026)

Research date: 2026-10-14. About 20 searches and fetches. Where a fetch failed or found nothing, the item is listed under Gaps instead of being guessed. Background knowledge that could not be checked against a source is marked "(unverified)" and appears only in Inferences or Gaps.

## Q1. Survey of deep-zoom renderers: algorithms, precision, GPU, max zoom, features, license, activity

### Takeaway
In 2026 the most advanced open-source techniques (perturbation, BLA/LA, rebasing, floatexp-type number formats) are spread across four projects. Fraktaler 3 is the newest release (v3.1, 2025-12-15, AGPL). FractalShark is the deepest and GPU-heaviest (CUDA only, GPL-3.0, active in 2026). Imagina has the fastest-CPU reputation (AGPL) but is mid-rewrite. Fractal Zoomer is the most feature-complete GUI (Java, GPL-3.0, updated 2026-01). KF2+ is effectively frozen at 2022-06 and never got BLA. At least two 2026 Swift+Metal hobby projects already implement perturbation, BLA and rebasing on Apple Silicon. These are useful references for a macOS/Metal renderer.

### Cited Findings

**Kalles Fraktaler 2+ (Claude Heiland-Allen fork of Karl Runmo's KF2)**
- Uses perturbation plus series approximation (SA), so pixel iterations run in fast low-precision types around a high-precision reference. The page states that BLA is "much better" than SA and that KF does not implement it — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- Number types benchmarked: double, scaled (rescaled) double, long double. Scaled long double and floatexp are used for deeper zooms — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- Formulas: Mandelbrot powers 2–10, Burning Ship, Buffalo, Celtic, Mandelbar, perpendicular variants, many cubic–quintic hybrids, SimonBrot, TheRedshiftRider, HPDZ Buffalo. Has a user hybrid-formula designer (Ctrl-H) and user GLSL colouring (Ctrl-G) — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- Latest version kf-2.15.5.2, 2022-06-07. Known crash past 1e291 in some formulas (workaround: disable Rescaled Double) — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- License AGPLv3+ — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- Status: "development is mostly stopped from my side, though there may be occasional bug fix releases". BLA is unlikely to be added, and the page points users to Imagina and fractalshades for BLA — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- KF has NanoMB1 (knighty's chained biseries, which skips whole periods near a minibrot) as a working feature. NanoMB2 is experimental — [mathr, Deep zoom theory and practice (2021-05-14)](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- KF supports hybrid formulas with "stanzas", rescaling, dual-number distance estimation and OpenCL code generation — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)

**Fraktaler 3 (Claude Heiland-Allen)**
- "Fast deep hybrid escape time fractals". Uses perturbation (with a configurable perturbation iteration limit) and BLA (with a step limit). Hybrids are built by stacking formula lines — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)
- Number types: long double on x86. v3.1 added doubleexp (double + separate exponent) and float128. The number type is picked automatically from "wisdom", a per-device benchmark of each type's speed — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)
- GPU: OpenCL on CPUs and GPUs, and the CPU and OpenCL backends can run together. The GUI needs OpenGL ES 3 — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)
- Platforms: Linux, Windows, Android 5+, web (needs SharedArrayBuffer; slower than native) — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)
- Formula model: Mandelbrot power P plus |X|, |Y|, −X, −Y toggles, which give Burning Ship and Mandelbar/Tricorn. Advanced mode exposes opcodes (store, sqr, mul, absx, absy, negx, negy, rot, add). Lines stack into hybrids — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)
- License AGPL-3.0, © 2021–2025. Latest v3.1 released 2025-12-15 (28 commits since v3). Has an active todo list and bug list — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)
- No macOS build is listed among the platforms — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)

**Imagina (Zhuoran, GitHub 5E-324/Imagina)**
- Describes itself as "One of the fastest Mandelbrot set renderer & explorer". Depends on MPIR and libpng. Source files include HInfLAEvaluator.cpp, JitEvaluator.cpp, FloatExp.h and ArbitraryPrecision.h, which suggests LA (linear approximation), a JIT formula evaluator and floatexp types (inferred from filenames only) — [GitHub 5E-324/Imagina](https://github.com/5E-324/Imagina)
- License AGPL-3.0. 61 commits, no releases. README: "The project is being rewritten" and the repo may be renamed or replaced — [GitHub 5E-324/Imagina](https://github.com/5E-324/Imagina)
- Zhuoran introduced BLA and the rebasing tests on Fractal Forums in December 2021, and later collaborated with Claude Heiland-Allen on Imagina — [mrob Muency: Bivariate Linear Approximation](https://mrob.com/pub/muency/bivariatelinearapproximati.html)
- KF's page recommends Imagina as a BLA-capable successor — [mathr KF page](https://mathr.co.uk/kf/kf.html)

**FractalShark (Matt Reiman, GitHub mattsaccount364/FractalShark)**
- Self-described as "an experimental research project, not a polished end-user application", "buggy and kludgy" — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- Algorithms: perturbation (KF cited as inspiration), including "perturbed perturbation", which caches intermediate-resolution orbits across zooms. Two CUDA LA implementations ported from Fractal Zoomer. Reference-orbit compression and periodicity detection (from Imagina). GPU antialiasing. A Newton-Raphson "Feature Finder" for periodic points — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- Precision: a custom "2×32 + exponent" type (two floats plus a shared exponent, about 48-bit mantissa, CUDA only), float+exp, double-double, quad-double, and HDR types (e.g. kernel "HDRx32 GPU - LAv2"). Reference orbits use MPIR on Windows and GMP on Linux. A GPU reference orbit uses NTT-based multiplication — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- GPU: CUDA with compute capability 7.5+ (GTX 16xx / RTX 20xx+). The GPU reference orbit needs RTX 20xx+. "Support for AMD cards unless I buy one" is listed as never-planned. The CPU path is "largely a debugging aid" — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- Max zoom: the built-in example minibrot is at about 1e650452 (View #34) — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- Perf claims: v0.532 GPU reference orbit is about 10× faster than multithreaded MPIR+AVX2 at 16384 limbs on an RTX 4090 (the 5090 was about 9×). v0.54 is about 70× faster than single-threaded AVX2 MPIR at 16384 32-bit limbs on an RTX 5090. The GPU orbit "may be faster" only beyond about 1e10000; "at shallower depths, the CPU is more likely to win" — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- License GPL-3.0. Over 1,090 commits. Linux support arrived in v0.53 (2026-06). Footer © 2026, so active in 2026 — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)

**Fractal Zoomer (hrkalona, Java)**
- SourceForge lists "Perturbation Theory", "Mandelbrot deep zooms", "Over 500 fractal functions" and "User formulas". GPLv3. Last update 2026-01-09 (active) — [SourceForge fractalzoomer](https://sourceforge.net/projects/fractalzoomer/)
- Its float+exp type and its LA implementations were the source FractalShark ported to CUDA. FractalShark says Fractal Zoomer has "much more polish" — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- User report (2025-07-09): with perturbation on, a view at −2+0i, size 1e-16, rendered invisible. The developer blamed the bailout check (workaround: bailout 2.00001) — [SourceForge fractalzoomer](https://sourceforge.net/projects/fractalzoomer/)

**fractalshades (G. Billotey, Python)**
- Python package for ultra-deep zooms (1e-2000 and beyond) of Mandelbrot and Burning Ship, using perturbation and chained bilinear approximations. GUI plus Python API. MIT license. Runs on Linux, macOS and Windows — [PyPI fractalshades 1.1.1](https://pypi.org/project/fractalshades/1.1.1/); [LinuxLinks](https://www.linuxlinks.com/fractalshades-create-static-interactive-fractal-visualizations/)

**Ultra Fractal 6 (commercial, Frederik Slijkerman)**
- UF6 added perturbation deep zooming. It computes one full-precision reference at the centre, renders the other pixels as double-precision offsets, and automatically detects precision problems and adds new references. The vendor's example is a 2e243 zoom in 42 s — [UF What's new](https://www.ultrafractal.com/whatsnew.html); [UF Perturbation calculations](https://www.ultrafractal.com/help/formulas/perturbationcalculations.html)
- Formula authors write the perturbed iteration by hand in `perturbinit`/`perturbloop` sections using `#dz` and `#dpixel`. Mandelbrot: `#dz = 2*#z*#dz + sqr(#dz) + #dpixel`. A `perturb` setting declares when the equations are valid: integer powers reduce, non-integer or complex powers do not. "Not all fractal types are suitable." The docs do not mention BLA or SA — [UF Perturbation equations](http://www.ultrafractal.com/help/writing/formulas/perturbationequations.html)

**SuperFractalThing (K.I. Martin, 2013)**
- Popularized perturbation and series approximation for deep zooms — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html); UF credits K.I. Martin and SFT as the origin of the algorithm — [UF Perturbation calculations](https://www.ultrafractal.com/help/formulas/perturbationcalculations.html)

**Mandel Machine (Botond Kósa)**
- Fractal Forums describes it as a "highly efficient Mandelbrot set explorer". Windows, 64-bit. The author was active on the forum in 2014 — [fractalforums.com Mandel Machine](https://www.fractalforums.com/mandel-machine/mandel-machine/)

**Other Swift/Metal and Apple Silicon deep zoomers (2025–2026)**
- drquicksilver/swift-mandelbrot: SwiftUI + Metal for iPhone, iPad and Mac. Precision steps automatically from Float to double-float (Metal has no fp64) to perturbation. Implements BLA and rebasing "reimplemented from Claude Heiland-Allen's deep-zoom write-up". Zoom "goes past 10^1000". Tile cache, worker and compositor architecture. Includes Julia sets. Needs Xcode 26.2 and macOS 15.7. 162 commits, © 2026. License not stated — [GitHub swift-mandelbrot](https://github.com/drquicksilver/swift-mandelbrot)
- lpalm/fractal_spectrum: the search snippet described a Swift+Metal Apple Silicon explorer (Mandelbrot, Burning Ship, Julia) with CPU high-precision reference, float32 GPU deltas, rebasing and BLA, "zoomable past 10^1000 at display frame rate". The repo URL returned 404 when fetched, so it may be deleted or private — [search result: GitHub lpalm/fractal_spectrum](https://github.com/lpalm/fractal_spectrum) (unconfirmed)
- jean-bovet/xFractal (SwiftUI+Metal, Mandelbrot perturbation toggle, CPU uploads a reference orbit per frame). Quick-Eyed-Sky/fractales (perturbation to about 1e30). tomc98/bend-2-mandelbrot (Bend 2 + Metal, experimental). All from search snippets, not opened — [xFractal](https://github.com/jean-bovet/xFractal); [fractales](https://github.com/Quick-Eyed-Sky/fractales); [bend-2-mandelbrot](https://github.com/tomc98/bend-2-mandelbrot)
- rust-fractal-core: a Rust Mandelbrot renderer with an arbitrary-precision reference and machine-precision deltas — [GitHub rust-fractal-core](https://github.com/rust-fractal/rust-fractal-core)
- munsocket/deep-mandelbrot (WebGL deep zoom) appeared in several searches; its details were not fetched — [GitHub deep-mandelbrot](https://github.com/munsocket/deep-mandelbrot)

### Inferences
- The best ideas to borrow for a Metal renderer:
  - From FractalShark: a two-float + shared-exponent type. Metal has no fp64, so this is the natural "fast deep" GPU type. Also reference-orbit compression and the GPU NTT reference orbit, though that only pays off past about 1e10000.
  - From Imagina and Fractal Zoomer: LA/BLA with rebasing.
  - From Fraktaler 3: an opcode-based hybrid formula model and automatic number-type selection by per-device benchmarking ("wisdom").
  - From KF2+: GLSL-like user colouring, and an exponential-map export (from background knowledge; not confirmed on the fetched page).
- SA (and NanoMB) is now considered superseded by BLA/LA. KF's own author says so, and every post-2022 renderer uses BLA/LA.
- No mature, maintained macOS-native deep zoomer showed up. Fraktaler 3 lists no macOS build, FractalShark is CUDA-only, and KF2+/Mandel Machine are Windows-only. That leaves a real niche, though small 2026 hobby Swift/Metal projects already cover the basics.
- Mandel Machine (unverified): it is generally considered abandoned since around 2015–2016. The only dated activity found was from 2014.

### Gaps
- Not researched or not found: Deep Drill (no search results at all), very-plotter (Phil Thompson), XaoS deep-zoom capability (believed limited to long double with no perturbation; unverified), SuperFractalThing's current status and code, and Mandel Machine's last version, date and license.
- Imagina's algorithm details (LA variant, GPU support) and the rewrite's status: the README gives no dates, and no release announcement was found.
- Fractal Zoomer's detailed deep-zoom option list (BLA, BLA2, Nanomb1/2, SA, which formulas support perturbation) was not available from SourceForge. The fractalzoomer.sourceforge.io site redirects to the project page.
- Max zoom and number-type limits for Imagina, Fractal Zoomer and Ultra Fractal were not found.
- WebGPU deep zoomers were not specifically found.

## Q2. Benchmark comparisons between renderers

### Takeaway
No neutral, up-to-date cross-renderer benchmark was found. What exists is author self-reports: FractalShark's GPU-vs-CPU reference-orbit numbers and its qualitative ranking against Imagina and Fractal Zoomer, UF's single vendor timing, and KF's internal number-type benchmarks.

### Cited Findings
- FractalShark's author expects "Imagina will probably beat it on performance in the long term" and says Imagina "easily has [high-quality CPU support] covered". FractalShark's MT3 CPU reference orbit beat the CPU implementations the author compared, but "this is not an exhaustive survey" — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- GPU reference orbit: about 10× faster on an RTX 4090 (v0.532) and about 70× on an RTX 5090 versus single-thread AVX2 MPIR (v0.54) at 16384 limbs. The crossover where GPU beats CPU is about 1e10000 — [GitHub FractalShark](https://github.com/mattsaccount364/FractalShark)
- UF6 vendor claim: a 2e243 Mandelbrot zoom in 42 s — [UF What's new](https://www.ultrafractal.com/whatsnew.html)
- KF2+ publishes benchmarks of its number types (double, scaled double, long double) — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- Claude Heiland-Allen (2022): benchmarks are still needed to decide whether BLA beats SA on speed. BLA is simpler, easier to parallelize and more general — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Fraktaler 3 benchmarks number types per device at runtime ("wisdom") rather than publishing cross-renderer numbers — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)

### Inferences
- A new renderer should ship its own reproducible benchmark set: fixed locations with depth, iteration count and resolution, measuring time-to-image and reference-orbit time. Comparing against Imagina and Fractal Zoomer (both runnable) would make it the first public cross-renderer comparison.

### Gaps
- No Fractal Forums benchmark thread was found or fetched (e.g. "Mandel Machine vs KF" from about 2015, or the 2022 BLA threads). The fractalforums.org successor site was not checked.
- No record-zoom metadata (which software YouTube and deviantART record zooms used) was found.

## Q3. Applying perturbation, rebasing and BLA to other formulas: which are realistic

### Takeaway
Realistic with known methods:
- **Multibrot (integer power)**: perturbation via the binomial expansion; BLA with Jacobian p·Z^(p−1).
- **Burning Ship and other abs/conjugate variants**: perturbation via laser blaster's `diffabs`; BLA with radius using min(|X|,|Y|) and the operator norm of a non-holomorphic Jacobian.
- **Mandelbar/Tricorn**.
- **Hybrids**: operator-wise perturbation, with one reference per hybrid phase for rebasing.
- **Julia sets**: same recurrence without the +c term (from background reasoning; not sourced).

Hard or unrealistic:
- **Non-integer or complex powers**: UF says they don't reduce.
- **Newton/Nova (convergent)**: no published perturbation scheme was found; deep Nova zooms historically used brute-force arbitrary precision.
- **Mandelbulb (3D)**: no perturbation source found.
- **Lyapunov**: not researched.

### Cited Findings
- **Mandelbrot baseline.** Reference Z→Z²+C in high precision; delta z→2Zz+z²+c in low precision. Glitches occur when |Z+z| << |Z|, detected with Pauldelbrot's test |Z+z|² < G|Z|² (G typically 1e-2..1e-8; choosing G reliably is an open problem). Fixes are extra references or rebasing — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- **Rescaling for deep zooms.** Substitute z=Sw and c=Sd, giving w→2Zw+Sw²+d, and re-scale every few hundred iterations. Use full floatexp iterations when |Z| is tiny (Pauldelbrot's Nanoscope approach) — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- **Burning Ship perturbation.** Naive perturbation cancels to 0. The fix is laser blaster's `diffabs(c,d) := |c+d|−|c| = c>=0 ? (c+d>=0 ? d : −(2c+d)) : (c+d>0 ? 2c+d : −d)`, applied as diffabs(XY/s, Xy+xY+sxy) when rescaling. When s underflows this reduces to sign(X)sign(Y)(Xy+xY). "Deep needle" locations (small X or Y) need full floatexp iterations, and KF never solved automatic detection of them — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- **SA for abs variations** is only valid briefly before an abs fold. That is one reason BLA displaced SA for these formulas — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- **Hybrids.** Perturbation is derived per operator (abs x/y, negation, integer power p, complex multiplier a), with S = Σ W^i B^(p−1−i) for the power term (W=Z+z, B=Z). Rescaling is threaded through operators. Distance estimation uses dual numbers. OpenCL code generation removes per-iteration branches and is faster — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- **Rebasing (Zhuoran).** When |Z+z| becomes small, reset to reference iteration 0 (z := Z+z). This avoids glitches instead of detecting them. With several critical points, keep one reference per critical point and pick the one minimizing |(Z−Z_o)+z|. Hybrids need one reference per phase (4 for an (M,BS,M,M) loop). Pre-periodic parts need more — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html). The rebasing test is two tests, one for the c-delta and one for the z-delta (Dec 2021) — [mrob Muency BLA](https://mrob.com/pub/muency/bivariatelinearapproximati.html)
- **BLA (Mandelbrot).** Approximate z→2Zz+z²+c by z→Az+Bc while z is small. Validity radius r = max(0, ε(|Z| − max|c|)/(|J_f(Z)|+1)), with ε the hardware precision (e.g. 2^-24 for float) and J_f=2Z. Merge rule: l=l_y+l_x, A=A_yA_x, B=A_yB_x+B_y, r=min(r_x, max(0,(r_y−|B_x|max|c|)/|A_x|)). Build a 2M-entry table of power-of-two levels. At each step use the longest valid BLA, otherwise do a perturbation step with rebasing. The derivation is called "a bit handwavy" — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html); recurrence A_{l+1}=2Z_nA_l, B_{l+1}=2Z_nB_l+1 — [mrob Muency BLA](https://mrob.com/pub/muency/bivariatelinearapproximati.html)
- **BLA for Burning Ship and other abs formulas.** Replace |Z| with min(|X|,|Y|) in the radius, and use the operator norm of the real 2×2 Jacobian for non-holomorphic formulas. Only single-critical-point-at-0 formulas had been fully implemented as of the post — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- **Software coverage.** Fraktaler 3 does perturbation + BLA for Mandelbrot power P with abs/neg toggles (Burning Ship, Mandelbar) and hybrids — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/). fractalshades does perturbation + chained BLA for Mandelbrot and Burning Ship — [PyPI fractalshades](https://pypi.org/project/fractalshades/1.1.1/). KF2+ has perturbation + SA for many abs-variant formulas and hybrids — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- **Multibrot and non-integer powers.** Positive integer powers reduce, so all terms are small, but non-integer or complex powers do not. UF's `perturb` flag marks validity — [UF Perturbation equations](http://www.ultrafractal.com/help/writing/formulas/perturbationequations.html). KF supports Mandelbrot powers 2–10 — [mathr KF page](https://mathr.co.uk/kf/kf.html)
- **Nova/Newton.** Historical deep Nova zooms (HPDZ) used high fixed precision: "precision 4" showed fuzz and was fixed at 192-bit. That is brute force, not perturbation — [hpdz Nova](https://hpdz.net/StillImages/Nova.htm); [hpdz Convergent](https://www.hpdz.net/TechInfo/Convergent.htm)

### Inferences
- **Julia sets** (unverified but straightforward): the delta recurrence is z→2Zz+z² with the initial delta equal to the pixel offset. There is no +c term, so BLA has B=0. Rebasing is complicated because the critical point is not where the orbit starts, so a reference at a periodic point/cycle of the Julia set is the natural choice. swift-mandelbrot and fractal_spectrum claim Julia support alongside deep zoom.
- **Mandelbar/Tricorn**: conj(Z+z)² gives the delta as conj(2Zz+z²) plus c. It is holomorphic up to conjugation, so BLA works with a real-linear (2×2) A matrix rather than a complex scalar.
- **Multibrot z^p+c**: delta = Σ_{k=1..p} C(p,k) Z^{p−k} z^k + c. BLA uses A=pZ^{p−1}. Rebasing is unchanged (critical point at 0).
- **Newton/Nova**: perturbation in principle applies to any polynomial or rational iteration (rational maps need a quotient-perturbation identity). But convergent fractals' interesting detail sits near basin boundaries where orbits are chaotic for only a short time, and there are no published glitch or rebase criteria. Rate this feasible but research-grade.
- **Lyapunov**: the sequence-driven logistic map is real-valued and parameter-space zooms are rarely deep. No deep-zoom literature was found. Low priority.
- **Mandelbulb/3D**: the triplex power is non-holomorphic and involves trig/atan2. Perturbation would require a per-formula derivation like diffabs, and ray-marching multiplies the cost. Unrealistic for a v1.

### Gaps
- Not found: an explicit source for Julia-set perturbation and BLA, for perturbation of Newton/Nova, for Lyapunov deep zoom, or for Mandelbulb perturbation.
- The Zhuoran/Claude "ABS variation BLA" Fractal Forums thread itself was not fetched. Only mathr's 2022 summary was used.
- KF2+ "et" generic formulas (automatic perturbation via symbolic differentiation from formula definitions) could not be confirmed with a source in this session. From memory, KF 2.15 generates perturbed code from formula XML (unverified).

## Q4. Architecting a renderer so new formulas are cheap to add

### Takeaway
The proven pattern has four parts:
1. Represent a formula as a small op-graph or opcode list (Fraktaler 3's sqr/mul/absx/absy/negx/negy/rot/add; KF's hybrid "stanzas").
2. Derive the perturbed iteration per operator mechanically: binomial expansion for powers, diffabs for abs, conjugation for negation.
3. Derive the BLA coefficients from the Jacobian, using the operator norm for non-holomorphic ops.
4. Code-generate branch-free kernels per formula (OpenCL in KF and Fraktaler 3, a JIT evaluator in Imagina).

UF shows the low-tech alternative, where authors hand-write the perturbed equations.

### Cited Findings
- Fraktaler 3 exposes an opcode set (store, sqr, mul, absx, absy, negx, negy, rot, add) and stackable formula lines for hybrids — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)
- KF/mathr: perturbation is derived operator-by-operator for hybrids. Code generation (e.g. OpenCL) removes per-iteration branches and runs faster. Distance estimation uses dual numbers — [mathr 2021](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
- BLA radius generalizes through the Jacobian (operator norm for non-holomorphic ops). BLA tables can be built in parallel per reference and per level, possibly on GPU — [mathr 2022](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
- Imagina's source includes a JitEvaluator and an "HInfLAEvaluator" (inferred from filenames) — [GitHub Imagina](https://github.com/5E-324/Imagina)
- UF: formula authors hand-write perturbinit/perturbloop and declare validity with `perturb` — [UF Perturbation equations](http://www.ultrafractal.com/help/writing/formulas/perturbationequations.html)
- Fraktaler 3 picks number types automatically from per-device benchmarks — [fraktaler.mathr.co.uk](https://fraktaler.mathr.co.uk/)

### Inferences
- For Metal, a practical design has five layers:
  1. A Swift formula DSL or op-list.
  2. A small symbolic layer that emits three outputs per formula: the high-precision reference step (CPU, MPFR/GMP or a custom bigfloat), the perturbed delta step, and the BLA Jacobian coefficients plus a radius function.
  3. Emitted MSL source per formula, specialized per number type: float, float+exponent (rescaled), and two-float+exponent. Compile it at runtime with `MTLDevice.makeLibrary(source:)` or function constants.
  4. Rebasing driven by a per-formula list of critical points, with one reference per critical point and per hybrid phase.
  5. Glitch fallback using Pauldelbrot's test for formulas where rebasing isn't derived.
- Start with Mandelbrot, then Multibrot (integer p), then Mandelbar, then Burning Ship and abs family, then hybrids, then Julia. Defer Nova and 3D.

### Gaps
- No public write-up on a symbolic engine for auto-deriving perturbed and BLA code was found beyond mathr's operator-wise derivations.
- Imagina's JIT design is not documented in its README.
