---
title: "Buddhabrot and Nebulabrot as a look"
type: plan
milestone: BB
status: in-progress
priority: 9
effort: M
appeal: high
audience: [casual, artists, education]
depth: "shallow: whole set to ~0.1 wide clean in seconds to a minute; ~1e-3 views are nearly flat and need minutes (Metropolis–Hastings with recheck/resampling)"
realtime-ipad: likely
reuse: "low–medium: the orbit overlay's orbits, progressive display, palettes and tone mapping"
depends-on: []
risks:
  - "slow convergence: noisy for seconds to minutes"
  - "iPad thermals under a long accumulation"
  - "atomic adds into a texture contend on hot pixels"
sources: ["synthesis/other-fractal-types", "sources/superliminal-buddhabrot", "decisions/controls", "techniques/frame-reuse-and-progressive"]
tags: [buddhabrot, nebulabrot, look, accumulation, optional]
created: 2026-10-19
updated: 2026-10-22
---

# Buddhabrot and Nebulabrot as a look

Status: built 2026-10-22, awaiting the user's review (in-progress); see [As built](#as-built)
and [Live mode](#live-mode-measured-2026-10-22). Planned as an optional look, independent of the
formula track ([Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md) §5, item 3).

## Goal

Show the Mandelbrot set as the famous Buddhabrot: a histogram of where escaping orbits go.
Nebulabrot maps three iteration limits to RGB (src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)). It is
the same orbits the orbit overlay draws, accumulated ([Controls: keyboard, pointer and touch](../decisions/controls.md) "Orbit overlay").

## Scope

- A look "Buddhabrot" (and "Nebulabrot") for still views, shallow (whole set to ~1e-3).
- GPU sampling of `c` (uniform, then Metropolis–Hastings for close-ups, Boswell's method,
  src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)), orbit iteration, atomic adds into a histogram
  texture, progressive display with tone mapping.
- Accumulation runs while the view rests; any motion clears and restarts at low resolution.
- Export as PNG at full quality (long accumulation).

## Non-goals

- Deep zoom: "not possible to zoom arbitrarily deeply" (src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)).
- ~~Real-time flight with the look on (the view falls back to the normal look while
  moving).~~ Overridden 2026-10-22 at the user's request: measured and built, see
  [Live mode](#live-mode-measured-2026-10-22). Motion keeps the look where its live accumulation
  is clean enough and falls back to the escape-time colouring elsewhere.
- Anti-Buddhabrot and other variants (later, if wanted).

## Design sketch

- New kernel `buddha_accumulate`: each thread samples `c`, iterates up to the limit, and if the
  orbit escapes, iterates again and adds 1 to the histogram bin of each `z_n` in the view.
  Rejecting `c` in the main cardioid and the period-2 bulb with the closed-form test saves most
  of the non-escaping work.
- Metropolis–Hastings: mutate `c` near samples whose orbits hit the view; needed for views much
  smaller than the set.
- Display: tone-map the histogram (log or power curve), palette from the current look; the
  progressive display and history code show it like a refining frame
  ([Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md)).

## Affected modules

- `almondbread-render/src/shaders/mandelbrot.metal` (new kernels, not touching `iterate`),
  `metal/mod.rs`, `scheduler.rs` (a rest-only job kind), `host.rs`.
- `almondbread-core/src/settings.rs` (look), `palette.rs` (tone map), `oracle.rs` (a CPU
  Buddhabrot for tests).
- FFI and Swift: look picker entry, a progress readout.

## Verification

- CPU Buddhabrot vs GPU histogram at a small size and fixed seed: same distribution within
  sampling noise (chi-square or relative L1 below a bound).
- Stills at the whole set, seahorse valley and a 1e-3 view, looked at; time to a clean image
  on the Mac and on the iPad (thermal state logged).
- Mandelbrot fast path unaffected: `iterate` unchanged; bench A/B ≈ 1.0 with the look off.
- `make interaction-test`: motion with the look selected falls back without stalls.

## Review gate

The user looks at Buddhabrot and Nebulabrot on the Mac and the iPad. Decide whether it is a look
in the picker, a tour step ("where the orbits go"), or both.

## Risks

- Noise for a long time at large sizes; set expectations with a progress line.
- iPad thermals: the device policy already lowers GPU budgets at serious thermal state
  ([iOS and iPadOS on a device: first run, signing, memory, thermal, background](../platform/ios-device.md)).
- Atomic contention on bright bins; measure, consider per-threadgroup bins.

## Open questions

- Does the look keep the current palette, or a dedicated nebula colour map? **Recommendation
  (as built):** both, by look. The Buddhabrot has one channel and runs its tone value through the
  current palette's rising half (dark at 0, the palette's brightest entry at 1), so the palette
  picker still changes it; the look sets Ice. The Nebulabrot has three channels and maps them to
  red, green and blue directly (its own colour map; a cyclic palette cannot carry three
  channels). Measured looks: [As built](#as-built) "Tone curve".
- Worth adding the orbit overlay's "orbit cloud" as a mini Buddhabrot of one `c`? Not done; the
  sampler could deposit one `c`'s orbit cheaply, but it would be a new overlay, not this look.

## Effort breakdown

M overall (1–2 weeks).

- Kernels, histogram, tone map: 3 days.
- Metropolis–Hastings sampling: 2 days.
- Scheduler job kind, UI, export: 2 days.
- Verification, iPad thermals: 2 days.

## As built

Built 2026-10-22 (branch of the BB agent; measured on M1 Pro, release, macOS, load average 6–11
from other sessions unless noted). Numbers also in
[almondbread benchmarks and verification record](../synthesis/benchmarks.md) "BB".

**Where it lives.** `almondbread-core/src/buddha.rs`: the mode (`BuddhaMode`: off, Buddhabrot,
Nebulabrot), limits (Buddhabrot 2000; Nebulabrot 5000 / 500 / 50 as red / green / blue, each
channel takes the orbits escaping within its limit, src: [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)), sampling region
`[-2, 2]²`, escape at `|z|² > 4`, the bin mapping of a view, the tone curve's defaults and the
CPU oracle (uniform f64 sampling on all cores). `RenderSettings::buddha`, set by two new looks
"Buddhabrot" and "Nebulabrot" (the other looks set it off, so a place or tour that applies its look leaves the
Buddhabrot; the interaction test's look switcher skips the two); `FormulaCapabilities::buddhabrot` is true for the Mandelbrot set only, so
the pickers hide the looks for Multibrot and Julia sets and the renderer ignores the setting there.
`almondbread-render/src/shaders/buddhabrot.metal` is a library of its own (compiled when the look
is first shown, like the Julia panel's), so `mandelbrot.metal` and `iterate` are untouched;
`buddha.rs` drives it and `scheduler/buddha_frame.rs` puts it in the frame loop: while the look is
on and the view is shallow enough (bins of at least `MIN_PIXEL` = 2.5e-7, about four float32 ulps
of an orbit point near |z| = 1), a refresh does not run the escape-time job at all.

**Sampler: persistent walkers.** The first kernel gave each thread a fixed number of proposals:
2.45e8 proposals/s at the whole set, 2.7e9 iterations/s, because a SIMD group waits for its
longest orbit (a near-boundary `c` runs 2000 steps twice while its neighbours escape in five).
As built, each of 65 536 threads is a walker with state in a buffer (104 B): it runs a fixed
number of loop steps per dispatch, one iteration per step whatever its phase (escape test, deposit
pass, a held MH state's pass), and starts the next proposal the step after an orbit ends; orbits go
on across dispatches. 5.3e8 proposals/s at the whole set (2.2×). Rejected at once, without a
step: `c` outside the region or in the main cardioid or period-2 bulb (closed form); Brent's
periodicity test (`|z − z_saved|² < 1e-12` at powers of two) stops most other interior orbits.
Persistent walkers cut the orbits in progress at any moment, and those are disproportionately the
long ones: the first GPU-vs-CPU test after the change failed (L1 0.15) until a histogram that ends
(export, tests) ends with a **drain** dispatch (no new proposals, every orbit in progress
finished, held MH counts deposited). 4× more walkers gave 2.8 % (not kept: 4× the state).

**Atomic contention: not an issue.** Count-only dispatches (`--deposit 0`) ran at the same
5.29e8 proposals/s as depositing ones (5.28e8; 1.1e9 atomic adds/s); with the first kernel the
difference was 12 %. No per-threadgroup bins.

**Metropolis–Hastings** (used when fewer than 5 % of uniform proposals hit the view). Target:
uniform over S, the `c` whose orbit has a point in the view. A proposal is a uniform jump
(probability 0.05) or a mutation by a radius log-uniform over 12 octaves below half the view's
diagonal; it is accepted iff it is in S. Every step deposits the chain's current orbit once (an
accepted proposal its own during its deposit pass; a rejected one adds to the state's hold count,
deposited with that weight when the state is left), so counts stay integers and the histogram is
unbiased; M steps stand for M / ρ uniform samples, ρ the counted share of uniform proposals in S
(only the progress readout and the live fold need it; the tone curve normalises by the hits).
Two additions made close-ups work: (1) **recheck and resample between views** (sequential Monte
Carlo): on a new view every chain judges its state again, and chains that fell out copy a random
surviving chain's state; when the new view lies inside the old one its S lies inside the old S,
so survivors uniform over the old S are uniform over the new; (2) a **warm-up** through views 4×,
16×, … larger (deposits off, two dispatches each) when the chains do not come from an enclosing
view. Measured at a view 1e-3 wide in the seahorse valley (400 × 400, 2 s, ρ = 7.5e-7): uniform
0 hits per pixel, MH 6.5, with the warm-up 29, with recheck and resampling 1142 (37 % accepted,
67 % of steps from a state in S). At a view 0.4 wide (ρ = 2.2 %, 400 × 400, 2 s each) the
run-to-run L1 of two seeds was 0.023 with MH against 0.046 uniform (4× the effective speed).

**Tone curve.** Mean-relative, so it needs no readback (the GPU sums each epoch's hits itself)
and works the same for the live accumulation: a bin's density in view means, from a black point
to a white point, linear (gamma 1). The points are measured on the GPU about four times a second
(a 1/16-octave histogram of the bins, `buddhaLevels`): white at the 99.8th percentile, black at
the 15th (Buddhabrot) or 35th (Nebulabrot) percentile but never below 0.5 / 0.8 means, white at
least one mean above black, and the Nebulabrot's channels share the brightest channel's white so
the colour balance holds (a channel of flat haze is not stretched to white). Defaults before a
measurement: white 4 means (about the whole set's 99.9th percentile), black 0.5 / 0.8. Without
the black point the haze of short orbits (each escaping `c` deposits `z_1 = c`) was a mid-blue
background; without the shared white the whole-set Nebulabrot's blue haze turned pale.

**Display and the frame loop.** Rest: two sample dispatches in flight of `chunk_target` (16 ms)
GPU each, the tone pass writes the drawable and the history image, so the overlay pass,
overlay-only refreshes and `capture_shown` work unchanged; sampling stops when the faintest
channel has `REST_TARGET_HITS` = 1500 hits per pixel. Any change of view, size or look restarts
the histogram (the walkers are kept and resampled). The progress readout is a new stage "orbits"
("orbits · 340 of 1.5 k per pixel · 120 M samples · ~10 s", ETA from the hit rate). Export and
CLI (`render --look buddhabrot|nebulabrot`, the app's export through `render_offscreen`)
accumulate to 4 × 1500 hits per pixel or 120 s (`--buddha-hits`, `--buddha-seconds`) with a
sampler of their own, drained, toned with freshly measured levels. `almondbread buddha` measures
timelines, stills, noise of two seeds (`--compare 1`), contention (`--deposit 0`) and the live
mode (`--live pan|zoom`).

**GPU vs CPU** (`buddha::tests::gpu_histogram_matches_the_cpu_oracle`, 64 × 64 bins compared in
4 × 4 blocks; statistics: two-sample χ² per degree of freedom and relative L1 of the normalised
histograms; bound: within 1.5× the CPU-vs-CPU value of two seeds at the same sample count, since
one orbit deposits a correlated run of hits and χ² is far above 1 for two CPU runs too).
Uniform, whole set, 16 M samples: Buddhabrot L1 0.0076 vs CPU-CPU 0.0074 (χ² 10.9 vs 10.1);
Nebulabrot red 0.0112 vs 0.0093 (χ² 24.0 vs 19.4), green 0.0049 vs 0.0059, blue 0.0032 vs
0.0032. MH, a view 0.4 wide at −0.1 + 0.75i, to 1e10 uniform-equivalent samples (~3.5 s)
against 40 M CPU samples: GPU-CPU L1 0.0166; noise GPU-GPU 0.0031, CPU-CPU 0.0193 (expected
GPU-CPU ≈ 0.014); bound 1.5 × 0.014 + 0.005. **MH has a transient bias** that decays as the
chains mix: against 4e8 CPU samples (noise ~0.005) L1 0.052 / 0.022 / 0.008 after 0.25 / 1 /
4 s (jump probability 0.05), 0.046 / 0.014 / 0.006 with 0.5, 0.024 / 0.009 / 0.007 with 1
(`mh_bias_scan`, ignored test). Our explanation (unverified): walkers run a fixed number of
iterations, not steps, so a chain in a region of long orbits makes fewer steps; until the chains
have mixed, those regions are under-weighted. A first time-bounded test (1 s) failed under load
for this reason; the test now runs to a sample count. A jump probability of 0.5 would halve the
bias at the close-up but halves the mutations where uniform jumps nearly never land (1e-3 views);
not changed.

**Stills** (1600 × 1000 through the export path, to 6000 hits per pixel or 120 s; looked at):
whole set Buddhabrot 8.6 s, Nebulabrot 22.4 s; seahorse valley (−0.75 + 0.1i, 0.16 wide)
Buddhabrot 55.7 s, Nebulabrot 120 s (cap); a view 1e-3 wide at −0.7435 + 0.1314i: 120 s (cap)
for both, and the image is a nearly flat field with speckle. The Buddhabrot at that scale is
smooth (the orbits through a tiny view come from everywhere); what structure there is comes from
short orbits (the set's boundary imprints through `z_1 = c`), and MH chains with long hold
counts leave bright dots. Time to the clean target (1500 hits per pixel): whole set 1200 × 1000
Buddhabrot 1.6 s, Nebulabrot 3.3 s; 2420 × 1668 (an iPad Pro 11" screen) Nebulabrot whole set at
1.9 wide ~34 s (44 hits per pixel per second in its 50-step channel). Verdict: the look is for
the whole set down to views about 0.1 wide; 1e-3 is reachable but not worth showing.

**Memory** (kb: platform/ios-device "Memory"): per output pixel 4 B (Buddhabrot) or 12 B
(Nebulabrot) of histogram; walkers and their snapshot 7.9 MB; live accumulation at 1/9 of the
pixels 44 B per live bin. At 2420 × 1668: Nebulabrot buffers 56.3 MB measured (48.4 MB histogram)
plus 19.7 MB live when moving; an export allocates its own histogram and walkers on top. Within
the device budget (the renderer's 80 B per texel are 323 MB there).

**Checks** (2026-10-22): `make rust-check` and `make rust-check-debug` 412 tests passed each
(2 ignored, one of them `mh_bias_scan`), `make regress` OK, `make build`, `make app`,
`make ios-sim`, `make ios-device-check`, `make swift-test` (14), `make core-check`, `make
selftest` (both apps `renderer OK (Rust)`, shown frame vs fresh render 0 pixels) all OK; the
targeted iOS UI test `TouchControlsUITests/testBuddhabrotLook` on the iPad mini simulator passed
(the look applies, the HUD shows "orbits · 1.7 k of 1.5 k per pixel · 2.2 G samples", the entries
are hidden for a Multibrot set). It found that the iOS Look picker (a navigation-link picker)
was disabled in the iPad pane, which has no navigation stack; it is a menu now, as on the Mac.

## Live mode (measured 2026-10-22)

The user asked whether the look can stay on while moving (the iPad Pro 11", M5, is faster than
the M1 Pro this was built on). Built and measured: while the view moves, each refresh samples a
coarse histogram (one bin per 3 × 3 pixels) for 0.8 of the gesture budget (half a refresh) in its
own command buffer; `buddhaFold` adds it to a float accumulation (hits per channel and
uniform-equivalent samples per bin) reprojected from the previous view by bilinear resampling,
scaled by the bin-area ratio and decayed with a 0.12 s time constant (more when zooming in), and
the tone pass shows `hits / samples` per bin, upsampled. Motion starts from the rest image (the
accumulation is seeded from the rest histogram, summed 3 × 3) and the rest histogram takes over
once it is as clean as the live one. The MH chains follow the view (recheck after the orbit in
progress).

- **Frame rate** (`make interaction-test` scenarios "buddha pan", "buddha zoom", "nebula zoom",
  window 1280 × 820 pt at 2× = 2560 × 1640 px, 120 Hz via `ALMONDBREAD_TEST_LINK=cv`, background
  window): buddha pan 116.7–118 fps, interval p50 / p95 / max 8.3 / 8.3 / 8.3–16.7 ms, GPU per
  frame 3.4 ms sampling + 1.7 ms fold, tone and overlay; buddha zoom 115.7–118 fps, 8.3 / 8.3 /
  8.3–25 ms. The escape-time "home pan" in the same runs: 111.6–113.7 fps. No stalls; the CPU
  per frame is lower than the escape-time path's (0.14 vs 0.23 ms).
- **Quality**: noisy but moving at the whole set (480 × 300 live bins at 1440 × 900, about 110
  hits per live bin at equilibrium); at a
  seahorse-valley view 0.15 wide only ~6–18 hits per live bin: noise. So motion keeps the look
  only where the expected hits per live bin (from the last live frames, or the rest histogram's
  rate) reach 25, and comes back above 35; elsewhere motion shows the escape-time colouring of
  the same look ("nebula zoom" fell back for most of its frames: preview scale 0.65). At a full
  iPad screen (2420 × 1668) the same rule falls back at the whole set 1.9 wide too (estimate
  ~18).
- **`realtime-ipad: likely`**: measured *yes* on the M1 Pro (the slower machine) for the whole
  set at a Mac window's size; the iPad Pro M5 should have more headroom per pixel (unverified:
  not measured on the device), but its full-screen pixel count is about that of the test window,
  and the thermal policy halves the gesture budget at serious state, which halves the live hits.

# Citations

- [Other fractal types: what almondbread could support, at what cost](../synthesis/other-fractal-types.md)
- [Melinda Green: The Buddhabrot Technique](../sources/superliminal-buddhabrot.md)
- [Controls: keyboard, pointer and touch](../decisions/controls.md)
- [Frame reuse and progressive refinement](../techniques/frame-reuse-and-progressive.md)
