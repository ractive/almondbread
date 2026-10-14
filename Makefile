# Almondbread build shortcuts. xcode-select may point at the Command Line Tools; xcodebuild
# needs the full Xcode, so pin it unless the caller already set DEVELOPER_DIR.
DEVELOPER_DIR ?= /Applications/Xcode.app/Contents/Developer
export DEVELOPER_DIR

DERIVED := .build/xcode
APP     := $(DERIVED)/Build/Products/Release/Almondbread.app

.PHONY: run build project app ios-sim render selftest interaction-test bench regress clean rust rust-check core-check rust-cli rust-bench rust-regress

## swift run, release (the macOS app without an Xcode project)
run: rust
	swift run -c release AlmondbreadMac

build: rust
	swift build -c release

project: project.yml
	xcodegen generate --quiet

## macOS .app via the generated Xcode project (ad-hoc signed)
app: project rust
	xcodebuild -project Almondbread.xcodeproj -scheme Almondbread -configuration Release -destination 'platform=macOS,arch=arm64' \
		-derivedDataPath $(DERIVED) -quiet build
	@echo "$(abspath $(APP))"

## iOS simulator build. Uses -sdk rather than -destination so it works without an installed
## simulator runtime (generic/platform=iOS Simulator needs one).
ios-sim: project rust
	xcodebuild -project Almondbread.xcodeproj -target "Almondbread iOS" -sdk iphonesimulator -arch arm64 \
		-configuration Debug SYMROOT=$(abspath .build/xcode-ios) -quiet build
	@echo "$(abspath .build/xcode-ios/Debug-iphonesimulator/Almondbread.app)"

## Headless check render through the Rust renderer: make render ARGS="--center -0.7436,0.1318 --scale 1e-8"
render: rust-cli
	$(RUST_CLI) render render.png --size 1600x1000 $(ARGS)

## Launch checks: the swift-run app and the Xcode .app each render their first view through the
## real view and the Rust renderer, compare the shown frame with a fresh render, write an
## offscreen PNG of the view (build/selftest-*.png, to look at) and print "renderer OK";
## a running process alone proves nothing. Background windows, they quit by themselves.
selftest: build app
	@mkdir -p build
	.build/release/AlmondbreadMac --selftest --selftest-png=$(abspath build/selftest-swiftrun.png)
	$(APP)/Contents/MacOS/Almondbread --selftest --selftest-png=$(abspath build/selftest-app.png)

## Frame pacing and latency of pans, zooms and keys in a background window (Rust renderer);
## ALMONDBREAD_SCENARIOS="home pan,neck zoom" picks some. kb: synthesis/benchmarks
interaction-test: build
	.build/release/AlmondbreadMac --interaction-test

## Benchmark set with GPU-vs-CPU verify metrics (kb: synthesis/benchmarks)
bench: rust-bench

## The -3/4 neck and seahorse history regressions (kb: techniques/interior-detection, Pitfalls)
regress: rust-regress

## Rust core (crates/): release static libs for macOS arm64, iOS arm64 and the iOS simulator,
## Swift bindings (Sources/AlmondbreadCore/Generated) and build/rust/AlmondbreadCoreFFI.xcframework.
## Incremental; build, app and ios-sim depend on it. kb: platform/rust-swift-ffi
rust:
	scripts/build-rust-xcframework.sh

## Rust gates: tests (oracle + property), clippy, rustfmt
rust-check:
	cargo test --workspace --release
	cargo clippy --workspace --all-targets -- -D warnings
	cargo fmt --all --check

## The renderer's headless tools (crates/almondbread-cli, binary `almondbread`; kb:
## platform/render-architecture).
RUST_CLI := target/release/almondbread
rust-cli:
	cargo build --release -p almondbread-cli

## Benchmark set through the Rust renderer: make rust-bench [ARGS="--sizes 1600x1000 --out dir"]
rust-bench: rust-cli
	$(RUST_CLI) bench $(ARGS)

## Headless sessions through the interactive scheduler (Space zoom, then one wheel step) at the
## -3/4 neck views of the wedge report and the seahorse; each fails on exterior painted as
## interior or on a final frame that differs from a fresh render
rust-regress: rust-cli
	$(RUST_CLI) regress $(ARGS)

## The Swift-Rust bridge through the real bindings (round trips, the Explorer on a windowless
## layer, an offscreen PNG) and FFI call costs incl. the per-frame overhead. kb: platform/rust-swift-ffi
core-check: rust
	@mkdir -p build
	swift run -c release almondbread-core-check $(abspath build/core-check.png)

clean:
	rm -rf .build Almondbread.xcodeproj
