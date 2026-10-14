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

## Build

`make run` (swift run), `make app` (macOS .app via Xcode), `make ios-sim`,
`make render ARGS="--center re,im --scale s"`. `xcode-select` points at the
Command Line Tools; the Makefile sets `DEVELOPER_DIR` to Xcode for
`xcodebuild`. No iOS simulator runtime is installed yet.

Checks (prefer headless; window tests run in the background, never take focus,
and quit by themselves):
- `make selftest`: swift-run app and Xcode .app each render a frame through the
  real view and print `renderer OK` (a running process proves nothing).
- `make bench [ARGS="--sizes 1600x1000 --out dir"]`: fixed benchmark set with
  GPU-vs-CPU verify metrics; record results in `kb/wiki/synthesis/benchmarks.md`.
- `AlmondbreadMac --verify --center re,im --scale s --size WxH --iterations N
  [--heatmap d.png]`, `--verify --fields a.iter --against b.iter` (before/after).
- `AlmondbreadMac --interaction-test`: frame pacing and latency of pans, zooms and
  keys in a background window (`ALMONDBREAD_SCENARIOS=` to pick some).
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
