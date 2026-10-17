# almondbread

A super-fast deep-zoom fractal explorer (Mandelbrot first) for macOS on
Apple Silicon, with an iPhone/iPad port; Windows/Linux UIs later. Everything
but the UI is Rust (kb: decisions/rust-core): renderer, Metal kernels
(float32), scheduler, navigation, maths. Swift + SwiftUI is the UI on Apple
platforms only.

## Layout

- Rust workspace (`Cargo.toml`, `crates/`; kb: decisions/rust-core,
  platform/render-architecture). math and core stay free of platform code and
  must build for Linux and Windows too.
  - `almondbread-math`: BigFloat, floatexp, reference orbit, reference selection;
    oracle fixtures in `tests/fixtures` (generator: `tests/oracle/generate_fixtures.py`).
  - `almondbread-core`: viewport, render settings, palettes, f64 reference,
    auto-iteration learning, CPU oracle and verify metrics, benchmark locations, PNG,
    and the `Navigator` input/motion model every UI forwards events to (all motion
    policy lives here).
  - `almondbread-render`: `GpuBackend` trait, the chunked scheduler `Renderer<B>`,
    `Host<B>` (a renderer hosted in a view: size, input, one frame per refresh,
    interaction measurements), MSL kernels in `src/shaders/` (the only copy), Metal
    backend + `CAMetalLayer` surface in `src/metal/` behind `cfg(target_vendor = "apple")`.
  - `almondbread-cli`: binary `almondbread`: headless render / verify / bench / regress.
  - `almondbread-ffi`: UniFFI exports; `Explorer` (explorer.rs) is the app's renderer:
    Swift hands over its `CAMetalLayer` once and calls `frame(timestamp)` once per
    display refresh (kb: platform/rust-swift-ffi). `uniffi-bindgen`: the generator.
- `Sources/AlmondbreadCore/`: Swift face of the Rust crates (`Generated/` is written by
  `make rust`, never edit it).
- `Sources/AlmondbreadKit/`: UI only: SwiftUI views, `ExplorerModel` (settings mirror
  forwarded to Rust, ~10 Hz stats), platform views (`FractalView.swift`: macOS NSView /
  UIKit view with the CAMetalLayer, display link, event decoding into `InputEvent`),
  self-test and interaction test.
- `Sources/AlmondbreadMac/`: `swift run` entry point. `Sources/almondbread-core-check/`:
  bridge check through the real bindings.
- `Apps/macOS`, `Apps/iOS`: thin app entry points for the Xcode project.
- `project.yml`: XcodeGen spec; `Almondbread.xcodeproj` is generated, never
  edit or commit it.

## Build

`make rust` builds the release static libs (macOS arm64, iOS arm64, iOS simulator), the
UniFFI Swift bindings and `build/rust/AlmondbreadCoreFFI.xcframework` (incremental; `run`,
`build`, `app`, `ios-sim` and `core-check` depend on it). `make run` (swift run),
`make build`, `make app` (macOS .app via Xcode), `make ios-sim`.
`make rust-check` = `cargo test --workspace --release`, clippy with `-D warnings`,
`cargo fmt --check`. Headless tools are the Rust CLI: `make render ARGS="--center re,im
--scale s"`, `make bench` / `make rust-bench [ARGS=...]`, `make regress` / `make
rust-regress`, `target/release/almondbread render out.png|verify|bench|regress ...`.
`make core-check`: the Swift-Rust bridge (round trips, Explorer on a windowless layer,
offscreen PNG in `build/core-check.png`) and FFI call costs. Orbit benchmark:
`cargo run --release -p almondbread-math --example math_check -- bench [bits...]`;
`cargo bench -p almondbread-math` for criterion. `xcode-select` points at Xcode; the
Makefile's `DEVELOPER_DIR` default is the same Xcode (harmless, override if needed).
No iOS simulator runtime is installed yet.

Checks (prefer headless; window tests run in the background, never take focus,
and quit by themselves):
- `make selftest`: swift-run app and Xcode .app each render their first view through
  the real view and the Rust renderer, compare the shown frame with a fresh render,
  write `build/selftest-*.png` (look at them) and print `renderer OK (Rust)` (a running
  process proves nothing). Path flag form: `--selftest-png=path` (a separate path
  argument makes AppKit open it as a file and SwiftUI opens no window).
- `make bench [ARGS="--sizes 1600x1000 --out dir"]`: fixed benchmark set with
  GPU-vs-CPU verify metrics; record results in `kb/wiki/synthesis/benchmarks.md`.
- `almondbread verify --center re,im --scale s --size WxH --iterations N
  [--heatmap d.png]`, `verify --fields a.iter --against b.iter` (before/after).
- `make interaction-test` (`AlmondbreadMac --interaction-test`): frame pacing, latency,
  draw CPU and per-frame FFI overhead of pans, zooms and keys in a background window
  (`ALMONDBREAD_SCENARIOS=` to pick some); fails if a shown frame differs from a fresh
  render.
- `make hang-test` (`AlmondbreadMac --hang-test`): the main-thread hang watchdog blocks its main
  thread for 3 s in a background window and prints the report (kb: techniques/hang-prevention;
  real reports go to `~/Library/Logs/Almondbread/hang-*.txt`). `cargo test extreme_`: the
  extreme-value tests (inf/NaN/1e-3010 inputs return quickly).
- `almondbread verify --history zoom|out|pan|jump --center re,im --scale s [--then f]`:
  a scripted session through the real per-refresh scheduler, headless, diffed against
  the CPU oracle and a fresh render; `make regress` runs the -3/4 neck regressions.
- `almondbread precision --location L|--place P [--ff 1] [--out dir/x]`: float32 vs the
  oracle next to the oracle with its grid shifted 0.001–0.5 px (is a difference an artefact
  or chaos?), same-colouring PNGs; `--ff 1` the float-float field, `--kat 1` the float-float
  known-answer test and number-type speeds (kb: techniques/double-float). `verify --ff 1` too.
- `almondbread autopilot --center re,im --scale s [--speed 4] [--seconds 90] [--lock 0|1]`:
  Z with no input, headless on the CPU (synchronous target lock, CPU survey), prints the
  timeline and the uniform-heading share; `fly ... --lock 0|1` measures the real pipeline
  (`autopilot-metrics:` line). `ALMONDBREAD_LOCK_TRACE=1` prints each lock search;
  `ALMONDBREAD_AUTOPILOT_LOCK=0|1` overrides the app's target-lock setting (A/B);
  `ALMONDBREAD_SURVEY_STEER=1` steers to the GPU survey's boundary point while no lock target is
  followed (opt-in, kb: decisions/controls "Survey steering").
- `ALMONDBREAD_TRACE=1` prints every iterate chunk (live counts, K, GPU ms) and seed round.
- Hitches: `ALMONDBREAD_FRAME_LOG=1` makes the interaction test print each scenario's longest
  presentation gaps and slowest draws with their times; `ALMONDBREAD_KERNEL_SALT=x` (new x)
  measures with a cold Metal shader cache (as after an update), `ALMONDBREAD_WARM_PIPELINES=0`
  turns the pipeline warm-up off (kb: platform/metal-compute); `ALMONDBREAD_GESTURE_PROBE=1`
  (off by default, also `ALMONDBREAD_GESTURE_PROBE=1 make interaction-test`) turns on the
  gesture-resolution probes and hysteresis against GPU-clock-inflated costs (kb:
  synthesis/benchmarks "Autopilot throttle and GPU clocks"); `ALMONDBREAD_PREVIEW_WAIT_BLA=0`
  lets previews start without a new reference's BLA table again (the 200 ms+ GPU stalls of
  autopilot dives, kb: techniques/bla "Pitfalls"); `fly --log x.csv` has per-refresh
  zoom, frame CPU and late ms (`fly_to` and `jump` take `{"place": id}`).
- Journeys: `almondbread journey --seed n --path 1|2` prints each leg's dull stretches and the
  per-dive depths; `ALMONDBREAD_PROBE_ESCALATE=0` restores the fixed CPU probe limit (target
  lock near deep high-period minibrots, kb: decisions/controls).
- M5 switches for before/after checks (default auto/on): `ALMONDBREAD_SA=0|1|auto` (local SA,
  CLI `--sa`), `ALMONDBREAD_FILL=0|1|auto` (interior tile fill, `--fill`),
  `ALMONDBREAD_ITER_FLOOR=0` (auto-iteration floor from the reference).
- M6 switches: `ALMONDBREAD_LA=0|1|auto` (Imagina-style LA instead of BLA, CLI `--la`; auto =
  deep nucleus references only), `ALMONDBREAD_AT=1` (its AT, `--at 1`; off by default).
  `cargo run --release -p almondbread-core --example la_check -- [name...]`: CPU model step counts
  of exact / BLA / LA / LA+AT per location (kb: techniques/linear-approximation-imagina).
- Ground truth for slow-escape views: `cargo run --release -p almondbread-core --example neck_truth
  -- --center re,im --scale s --size 160x100 --limit 1e7 [--dd k]` (direct f64, escape histogram,
  interior proof; `--paint a.iter [--cycle p]` paints any `verify --dump`; kb:
  synthesis/neck-escape-statistics). `verify --history ... --cap n --timeout s` with
  `ALMONDBREAD_SNAPSHOT=x.png` saves the frame shown at the timeout (a render in progress).
- Stills of features: `almondbread render out.png --inside flat|bubbles|period|angle|distance`
  (interior colourings; prints the period histogram), `--orbit x,y --map 1` (orbit overlay and
  mini-map), `scripts/render-interior-stills.sh`, `scripts/render-dense-looks.sh`.
- `scripts/ui-screenshots.sh`: control center and overlays from background `--ui-demo`
  windows; the app captures its own window (no screen-recording permission, no focus) into
  `build/screens/window-*.png`.

## Working mode

While another agent is working in the tree, commit only with explicit
pathspecs (`git commit -- <paths>`): a plain `git commit` also takes what
that agent has staged.

The main session steers and reviews; coding tasks go to fresh-context
agents with a self-contained brief. Verify every change with builds of all
three targets and headless renders that are actually looked at. "The
process is running" is not a launch check: prove the renderer initialised
and drew a frame, in both the `swift run` and the Xcode-built app.
Interactive smoothness (frame rate and latency while panning and zooming)
is an acceptance criterion and must be measured, not assumed from GPU time.

Do not interrupt the user's screen. Verify headlessly (`almondbread render`,
`verify`, `bench`) wherever possible. Tests that need a window run in
the background: test flags switch to the `.accessory` activation policy,
never activate the app or make its window key, keep it behind other
windows, and quit when done (`open -g -j` for the .app). Kill leftover
test instances. Only the driver session launches the app for the user.

## Knowledge base

`kb/` is an LLM-maintained wiki of the maths and engineering behind the
app. Its rules are in `kb/CLAUDE.md`; read them before touching `kb/`.
Use `hyalo` for wiki operations (`.hyalo.toml` sets `dir = "kb/wiki"`).
Before implementing a technique, read its page (`hyalo read
techniques/<name>.md`) and the roadmap in
`synthesis/deep-zoom-rendering-techniques.md`; cite pages in code comments
as `kb: techniques/bla`. When work produces new knowledge (benchmarks,
pitfalls, decisions), file it in the wiki and log it.
