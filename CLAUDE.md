# almondbread

A super-fast deep-zoom fractal explorer (Mandelbrot first) for macOS on
Apple Silicon, with an iPhone/iPad port: Swift + SwiftUI, Metal compute
kernels (float32 only), reference orbits on the CPU.

## Layout

- `Sources/AlmondbreadKit/`: everything shared — renderer, model, Metal
  kernels (`Shaders/Mandelbrot.metalsrc`: Metal source, compiled at runtime;
  the extension keeps Xcode from precompiling it), SwiftUI views,
  platform input views (`FractalView.swift`, `#if os(macOS)` / UIKit).
- `Sources/AlmondbreadMac/`: `swift run` entry point and the headless
  `--render out.png` command used for verification renders.
- `Apps/macOS`, `Apps/iOS`: thin app entry points for the Xcode project.
- `project.yml`: XcodeGen spec; `Almondbread.xcodeproj` is generated, never
  edit or commit it.
- `Sources/AlmondbreadMath/`: Swift arbitrary precision (reference for the Rust
  port; deleted once the Rust core reaches parity, kb: decisions/rust-core).
- Rust workspace (`Cargo.toml`, `crates/`, kb: decisions/rust-core):
  `almondbread-math` (BigFloat, floatexp, reference orbit, reference selection;
  oracle fixtures in `tests/fixtures`), `almondbread-core` (platform-neutral: viewport,
  render settings, palettes, f64 reference, auto-iteration learning, CPU oracle and verify
  metrics, benchmark locations, PNG, the `Navigator` input/motion model every UI forwards
  events to), `almondbread-render` (`GpuBackend` trait, the chunked scheduler
  `Renderer<B>`, MSL kernels in `src/shaders/`, Metal backend + `CAMetalLayer` surface in
  `src/metal/` behind `cfg(target_vendor = "apple")`; kb: platform/render-architecture),
  `almondbread-cli` (binary `almondbread`: headless render/verify/bench/regress),
  `almondbread-ffi` (UniFFI exports), `uniffi-bindgen`. math and core stay free of
  platform code. Until R2b the app still runs the Swift renderer
  (`Sources/AlmondbreadKit/Renderer.swift`, `Shaders/Mandelbrot.metalsrc`); kernel changes
  go into both copies. `Sources/AlmondbreadCore/` is the Swift face of the Rust core
  (`Generated/` is written by `make rust`, never edit it);
  `Sources/almondbread-core-check/` checks it against AlmondbreadMath.

## Build

`make run` (swift run), `make app` (macOS .app via Xcode), `make ios-sim`,
`make render ARGS="--center re,im --scale s"`. `xcode-select` points at the
Command Line Tools; the Makefile sets `DEVELOPER_DIR` to Xcode for
`xcodebuild`. No iOS simulator runtime is installed yet.

Rust: `make rust` builds the release static libs (macOS arm64, iOS arm64, iOS
simulator), the UniFFI Swift bindings and `build/rust/AlmondbreadCoreFFI.xcframework`
(incremental; `run`, `build`, `app` and `ios-sim` depend on it; kb:
platform/rust-swift-ffi). `make rust-check` = `cargo test --workspace --release`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
`make core-check` runs the Rust core through the Swift bindings and compares it
bit for bit with AlmondbreadMath, plus FFI call costs. The Rust renderer's tools take the
same options and print the same output as the Swift ones: `make rust-bench [ARGS=...]`,
`make rust-regress`, `target/release/almondbread render out.png|verify|bench|regress ...`
(`verify --fields a.iter --against b.iter` also reports bitwise-identical texels, so a Swift
dump diffs against a Rust one). Orbit benchmark against Swift:
`cargo run --release -p almondbread-math --example math_check -- bench [bits...]`
(same output as `swift run -c release almondbread-math-check bench`; `BEST_OF=n`
for loaded machines); `cargo bench -p almondbread-math` for criterion.

Checks (prefer headless; window tests run in the background, never take focus,
and quit by themselves):
- `make selftest`: swift-run app and Xcode .app each render a frame through the
  real view and print `renderer OK` (a running process proves nothing).
- `make bench [ARGS="--sizes 1600x1000 --out dir"]`: fixed benchmark set with
  GPU-vs-CPU verify metrics; record results in `kb/wiki/synthesis/benchmarks.md`.
- `AlmondbreadMac --verify --center re,im --scale s --size WxH --iterations N
  [--heatmap d.png]`, `--verify --fields a.iter --against b.iter` (before/after).
- `AlmondbreadMac --interaction-test`: frame pacing and latency of pans, zooms and
  keys in a background window (`ALMONDBREAD_SCENARIOS=` to pick some); fails if a
  shown frame differs from a fresh headless render.
- `AlmondbreadMac --verify --history zoom|out|pan --center re,im --scale s [--then f]`:
  a scripted session through the real per-refresh scheduler, headless, diffed against
  the CPU oracle and a fresh render; `make regress` runs the -3/4 neck regressions.
- `ALMONDBREAD_TRACE=1` prints every iterate chunk (live counts, K, GPU ms).

## Working mode

The main session steers and reviews; coding tasks go to fresh-context
agents with a self-contained brief. Verify every change with builds of all
three targets and headless renders that are actually looked at. "The
process is running" is not a launch check: prove the renderer initialised
and drew a frame, in both the `swift run` and the Xcode-built app.
Interactive smoothness (frame rate and latency while panning and zooming)
is an acceptance criterion and must be measured, not assumed from GPU time.

Do not interrupt the user's screen. Verify headlessly (`--render`,
`--verify`, `--bench`) wherever possible. Tests that need a window run in
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
