---
title: "Flight benchmark: one fixed flight, a score, a regression guard"
type: synthesis
date: 2026-10-15
sources: ["[[sources/metal-benchmarks-readme]]"]
tags: [benchmark, measured, interaction, regression, decision]
---

# Flight benchmark

A benchmark that plays one fixed, versioned flight and produces a score. It is mainly a
regression guard between commits; later it may become a user-facing "Benchmark" feature.
Design agreed with the user on 2026-10-15; built the same day (our own design, no source).

Code: `crates/almondbread-cli/src/benchmark.rs` (`almondbread benchmark`), the flight
player and real-time recorder in `crates/almondbread-core/src/flight.rs` (shared by `fly`,
the CLI and the app), the window side in `crates/almondbread-ffi/src/flight_bench.rs` and
`Sources/AlmondbreadKit/FlightBench.swift`. Flight: `scripts/flights/benchmark-v1.json`.
Reference: `scripts/flights/benchmark-v1-reference.json`.

## The flight (v1)

60 s at 1920×1080 pixels (independent of the screen, so devices compare), palette
`classic`, supersampling 3×3 (the app default, pinned), interior detection on, no random
input. Mode A samples the camera at 5 Hz; mode B headless runs at 120 Hz. Each regime
appears once, as a segment. At each landing (a jump, or the end of input) the benchmark
measures time to first image and time to sharp:

| segment | s | what happens | landings |
|---|---|---|---|
| shallow | 0–10 | home view, pan right and back, pinch in at 1.5 e-folds/s towards the seahorse valley, two wheel notches | 8.0 |
| dense | 10–20 | jump to the dense spot −1.4080321340304542 + 0.13680236432900417i at 4.615e−14 per pixel; hold; pan with a fling; pinch out | 10.0, 17.0 |
| dive | 20–40 | jump to c = i at 1e−3; pinch at 40.2 e-folds/s to 1e−300 (17 s); hold | 20.0, 38.0 |
| heavy | 40–48 | jump to `spiral-32412-1e19` (the period-32412 spiral, 4.198e−19 per pixel: the location's framing at 1600 px wide); hold | 40.0 |
| rotate | 48–54 | jump to elephant valley (0.2925 + 0.0149i, 1.3e−5); rotate at 0.9 rad/s while pinching in at 0.6 e-folds/s for 3.5 s | 48.0, 52.5 |
| jump | 54–60 | jump to `needle-1e1004` (the period-834 needle minibrot at 3.3e−1004 per pixel) | 54.0 |

The flight uses scripted gestures and jumps only. Fly-to and the autopilot are left out on
purpose: their speed follows the renderer's feedback, so the camera path would depend on
the machine. A jump is a new script event (`{"jump": {"re", "im", "scale"}}` or
`{"location": name, "pixels": w}`), the same as pasting coordinates. **Versioning:** scores
compare only within one `benchmark.version`. Any change to the flight, its settings or the
measuring rules bumps the version and needs a new reference.

## Mode A: throughput (headless)

The camera path is computed once without a renderer: the `Navigator` is stepped at a
virtual 120 Hz and sampled at `step_hz`, so the path is deterministic. Each sample is set
as the view, then the real per-refresh scheduler (`Renderer::render_frame`) runs back to
back, with no display and no sleeping, until the view is **settled**: `is_settled()` (refined,
extended, anti-aliased within the app's AA budget), no retry pending, and the last frame
finished on the GPU. Then the next sample starts. Reuse (reprojection, temporal history,
seeds, any future sample cache) is allowed because it is part of the app, but it cannot
fake a frame, because two quality checks run outside the timed part:

1. **Shown vs fresh** (`displayed_frame_mismatch`, as in the interaction test): the shown
   level's texels against a fresh offscreen render at the same iteration limit. More than
   0.05 % of pixels differing in class fails. Runs every `--check-every` (100) frames and
   at the end of each segment.
2. **Settled vs jump** (added after the first runs): at the end of each segment, the frame
   against a from-scratch render of the same view by a new renderer, which picks its own
   auto iterations. More than 2 % of 16×16 blocks whose mean luma differs by more than
   32/255 fails. Check 1 cannot see a view that settled at a too-low limit, because the
   fresh render uses the same limit.

A failed check makes the run **INVALID**: the score is still printed but flagged, the exit
code is 1, and `--write-reference` refuses to write. The fade-in time constant is set to
~0 (it is presentation only). Wall, CPU (process `getrusage`) and GPU (iterate and colorize
command buffers) seconds are reported per segment. `--stills dir` writes each segment's
last settled frame.

## Mode B: real time

The flight is played live at the display rate through the same per-refresh path, input fed
as decoded events at their times. Per lap and per segment it reports:

- **Due frames:** every display refresh of a motion run (refreshes where the camera
  moved). It is counted from time × rate, so display-link callbacks lost to a blocked main
  thread still count as due. A one-refresh run (a jump) is not counted; its new image is
  the time to first image.
- **Shown frames:** presentation times (window) or commits (headless) that fall inside the
  run.
- **Frame times:** p50, p99 and max of the intervals between shown frames. Late frames are
  intervals over 1.5 refreshes; missed refreshes is Σ(round(interval × rate) − 1).
- **Landings:** time to first image (the first frame computed for the new view;
  `Renderer::first_job_frame` is reset at the landing) and time to sharp (the first refresh
  that is settled and not moving). If the view is not sharp before the next landing or the
  end of the segment, the landing is censored and counts with that window, a lower bound.
- **Window only:** input-to-photon p50/p95 from `Host`'s measurement. The feed goes
  through `Host::input`, which records pending input.

Headless approximation: `realtime_lap`, paced at 120 Hz like `fly`. Window:
`AlmondbreadMac --flight-bench=flight.json --flight-bench-out=x.json
[--flight-bench-laps=n --flight-bench-warmup=n]`. It runs as a background TestLaunch
window like the interaction test: accessory policy, never key, behind other windows, at
the screen edge, and it quits when done. The window is sized to 1920×1080 *pixels*
(960×540 pt at 2×). Script positions are scaled by view points per script pixel. The CLI
starts it with `--live-app` and samples the machine while it runs.

## Score

- Throughput = 1000 × reference total s / median total s.
- Real time = 1000 × geometric mean of (shown fraction / reference), (reference p99 /
  p99) and (reference total time to sharp / total time to sharp). Headless and live have
  separate reference values.
- Total = geometric mean of throughput and real time (live if present, else headless).

The output always shows the sub-scores and the per-segment numbers next to the total. The
reference machine (the user's M1 Pro, 32 GB) scores 1000. A fixed pixel count makes devices comparable. Apple
GPUs differ mainly in core count (M1 2.6, M1 Pro 5.3, M1 Max 10.6 TFLOPS32; src:
[[sources/metal-benchmarks-readme]] chip table), so mode A tracks that. Mode B also depends
on the panel's rate: a 60 Hz display halves the due frames but not the work per frame.

## Hygiene

- Warm-up laps (`--warmup`, default 1) are run and discarded, against thermal and
  pipeline-cache effects.
- `--repeat` laps (default 3) are measured; the result is the median, with the spread
  ((max − min) / median) per segment.
- Busy machine: before each lap (with the benchmark idle) the CLI reads the CPU % of other
  processes (`ps`), the GPU's "Device Utilization %" (`ioreg -c IOAccelerator`; other GPU
  clients show up here) and the load average. During the laps it samples the other
  processes' CPU every 2 s. Over one core of other CPU (median during, or any probe
  before) or a GPU more than 10 % busy before a lap prints a loud NOISY banner and marks
  the JSON `noise.noisy`.
- JSON goes to `build/benchmark/flight-v<version>-<headless|live>-<commit>-<unix>.json`:
  commit (and whether the tree is dirty), device (model, chip, memory, cores, OS), flight
  and settings, renderer switches (`renderer.sa`, `renderer.bla`), noise samples, every
  lap, the medians and the scores. `--compare a.json b.json` prints deltas per segment and
  flags noisy or invalid runs and differing SA modes.
- A/B: `--sa 0|1|auto` and `--bla 0|1|auto` (and every other `apply_renderer` switch) go
  to the headless renderers. For `--live-app`, `--sa` is passed on as `ALMONDBREAD_SA`.

## Commands

```text
make flight-bench                       # A + headless B: warm-up + 3 laps (~6 min busy)
make flight-bench ARGS="--repeat 1 --warmup 0"
make flight-bench ARGS="--sa 0"         # SA off for an A/B against the default
make flight-bench-live                  # B in the background window (unlocked screen)
target/release/almondbread benchmark --compare a.json b.json
target/release/almondbread benchmark --noise 1    # is the machine quiet? (exit 1 if not)
```

## Calibration (the reference = 1000)

The reference in the repo is **provisional**. It comes from single noisy laps on
2026-10-15 while other agents loaded the CPU and GPU, and its throughput lap was INVALID
(see the findings). To calibrate:

1. Quit other GPU users (other agents' benches, video, browsers with WebGL). Run
   `almondbread benchmark --noise 1` until it reports quiet. Use mains power and leave the
   lid open.
2. `make rust-cli build`, then commit, so the reference records a clean commit.
3. `target/release/almondbread benchmark --repeat 5 --warmup 2 --write-reference
   scripts/flights/benchmark-v1-reference.json --final 1`. It refuses if the run is
   invalid, or noisy with `--final 1`.
4. `target/release/almondbread benchmark --live-app .build/release/AlmondbreadMac --repeat
   5 --warmup 1 --write-reference scripts/flights/benchmark-v1-reference.json --final 1`.
   It keeps the throughput and headless values and adds the live ones. The screen must be
   unlocked and the window partly uncovered, or there are no presentation times.
5. Check that the spreads are under ~3 %, commit the reference, and log it here and in
   [[synthesis/benchmarks]].

## Findings from the first (noisy) runs, 2026-10-15

Measured on the M1 Pro at commit 8b9155c (main eadb176 with M5 local SA, `renderer.sa`
Auto), release, while the machine was busy: other processes used 3–9 cores, and the GPU
was 24–97 % busy before laps. Every number here is noisy and only shows that the tool
works.

- **Auto iterations are path-dependent at the heavy spot.** In mode A, the jump from the
  1e−300 dendrite to `spiral-32412-1e19` sometimes settles as an **all-black frame** within
  14 ms of GPU time. The limit stays at 10384, below the period 32412. The reference search
  is capped at that limit ("period limit 10384 (none usable)"), so there is no nucleus
  floor, the learner returns 0 → 0, and nothing retries. The same jump from the home view
  renders correctly (41536 iterations, ~5 s GPU). Check 1 passed it; check 2 catches it
  (76.7 % of blocks differ, limit 10384 shown vs 41536 after a jump). Reported to the
  driver; the fix goes to the M5 work. The flight keeps this path on purpose, so the
  benchmark catches it until the fix lands.
- In a run where the heavy view did render, check 1 failed there: **0.591 % of pixels**
  (12251) differ in class from a fresh render at the same limit (threshold 0.05 %). All
  other segment ends: 0.000 %.
- Mode A, one lap, settled at full quality (the run where heavy rendered):

  | segment | frames | wall s | CPU s | GPU s |
  |---|---:|---:|---:|---:|
  | shallow | 50 | 10.07 | 0.38 | 6.50 |
  | dense | 50 | 10.27 | 0.47 | 7.74 |
  | dive | 100 | 4.52 | 1.08 | 1.47 |
  | heavy | 40 | 5.57 | 0.12 | 5.53 |
  | rotate | 30 | 5.48 | 0.25 | 4.41 |
  | jump | 30 | 0.60 | 0.04 | 0.52 |
  | total | 300 | 36.51 | 2.34 | 26.17 |

  Without M5 (before the merge, at the old 10 Hz step and with the rotation at the seahorse
  1e−7 spot), the same mode took 222 s. That is why the rotation moved to elephant valley
  (290 ms per frame without AA vs 12 ms) and the step to 5 Hz: one lap must stay around a
  minute.
- Mode B headless, one lap: 91.0 % of due frames shown, p99 25 ms, 255 late frames, time
  to sharp 12.05 s in all (heavy 6.2 s, the needle jump: first image 258 ms, sharp 1.05 s).
  A quieter moment earlier gave 99.5 %, p99 8.3 ms and 9.3 s.
- Mode B in the window, one lap at 120 Hz: the display link delivered only ~38 refreshes/s
  (2261 `frame` calls in 60 s, all late). 24.6 % of due frames were shown, p99 92 ms,
  input-to-photon p50 29 ms / p95 33 ms. An interaction-test run at the same time
  (`home pan`, `1e5 zoom`) got 27–28 fps, so the cause is the machine, not the bench.
- In one of three window runs the app reported `window key true, app active true` at
  start, although the window is a TestLaunch background window. The other two runs, and
  the interaction test, reported false. It may have been a click on the visible window.
  The bench now calls `NSApp.deactivate()` and `orderBack` before measuring.

## Open

- Calibrate on an idle machine (above).
- Fix the heavy-spot auto-iteration path and the 0.59 % class mismatch (M5 work). Until
  then mode A is INVALID by design.
- iOS: the FFI calls exist, but no iOS harness drives them yet.
- A user-facing "Benchmark" (later): the score and per-segment table already come from
  Rust, so a SwiftUI sheet only has to show the JSON.

See also [[synthesis/benchmarks]] (the running record), [[techniques/auto-iterations]],
[[techniques/local-series-approximation]], [[techniques/frame-reuse-and-progressive]].
