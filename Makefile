# Almondbread build shortcuts. xcode-select may point at the Command Line Tools; xcodebuild
# needs the full Xcode, so pin it unless the caller already set DEVELOPER_DIR.
DEVELOPER_DIR ?= /Applications/Xcode.app/Contents/Developer
export DEVELOPER_DIR

DERIVED := .build/xcode
APP     := $(DERIVED)/Build/Products/Release/Almondbread.app

.PHONY: run build project app dmg ios-sim ios-selftest ios-uitest render selftest interaction-test hang-test interaction-test-120 bench regress clean rust rust-check core-check rust-cli rust-bench rust-regress flight-bench flight-bench-live

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

## Release .dmg in dist/ (scripts/make-dmg.sh): ad-hoc signed unless SIGN_IDENTITY is set;
## notarised and stapled when NOTARY_PROFILE (a notarytool keychain profile) also is.
## make dmg VERSION=0.2.0 [SIGN_IDENTITY="Developer ID Application: ..."] [NOTARY_PROFILE=name]
dmg: project rust
	scripts/make-dmg.sh

## iOS simulator build. Uses -sdk rather than -destination so it works without an installed
## simulator runtime (generic/platform=iOS Simulator needs one).
ios-sim: project rust
	xcodebuild -project Almondbread.xcodeproj -target "Almondbread iOS" -sdk iphonesimulator -arch arm64 \
		-configuration Debug SYMROOT=$(abspath .build/xcode-ios) -quiet build
	@echo "$(abspath .build/xcode-ios/Debug-iphonesimulator/Almondbread.app)"

## iOS checks on the simulators (kb: platform/ios-simulator): self-test (first view through the
## real view and Rust, shown vs fresh render, offscreen PNG pulled to build/ios-selftest-*.png)
## and the touch UI test (real UIKit gestures and the toolbar buttons, screenshots in
## build/ios-screens). IOS_DEVICES: simulator names (xcrun simctl create).
IOS_DEVICES ?= AB iPhone 17e,AB iPad mini
ios-selftest: ios-sim
	scripts/ios-selftest.sh "$(IOS_DEVICES)" $(abspath .build/xcode-ios/Debug-iphonesimulator/Almondbread.app)

ios-uitest: project rust
	scripts/ios-uitest.sh "$(IOS_DEVICES)"

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
## ALMONDBREAD_SCENARIOS="home pan,neck zoom" picks some; renderer switches pass through the
## environment (ALMONDBREAD_FRAME_LOG=1 make interaction-test). kb: synthesis/benchmarks
interaction-test: build
	.build/release/AlmondbreadMac --interaction-test

## The main-thread hang watchdog in a background window: blocks the main thread for 3 s, prints
## the report it wrote (build/hang/hang-*.txt) and "hang test OK". kb: techniques/hang-prevention
hang-test: build
	.build/release/AlmondbreadMac --hang-test --hang-dir=$(abspath build/hang)

## The same in FRONT of other windows (takes focus for ~2 minutes; run it yourself, never from
## an agent): the only way to measure 120 Hz presentation on a ProMotion panel. Results in
## build/interaction-120-foreground.txt. kb: synthesis/benchmarks "120 Hz on ProMotion"
interaction-test-120: build
	ALMONDBREAD_TEST_FOREGROUND=1 ALMONDBREAD_SCENARIOS="home pan,home zoom,1e5 pan,1e5 zoom,deep zoom 1e-12 to 1e-40,autopilot,fly to,rotate zoom,cinematic autopilot,journey" \
		.build/release/AlmondbreadMac --interaction-test | tee build/interaction-120-foreground.txt

## Benchmark set with GPU-vs-CPU verify metrics (kb: synthesis/benchmarks)
bench: rust-bench

## Flight benchmark with a score (kb: synthesis/flight-benchmark): the fixed flight
## scripts/flights/benchmark-v1.json in mode A (throughput: every frame settled at full quality,
## no display) and mode B's headless approximation (real time at 120 Hz through the scheduler);
## warm-up lap, 3 laps, medians, noise check, JSON in build/benchmark/. Quick: ARGS="--repeat 1
## --warmup 0". Compare: target/release/almondbread benchmark --compare a.json b.json
flight-bench: rust-cli
	$(RUST_CLI) benchmark $(ARGS)

## Mode B live: the flight in a background window at the display's rate (never takes focus,
## quits by itself; needs an unlocked screen), scored against the live reference
flight-bench-live: build rust-cli
	$(RUST_CLI) benchmark --live-app .build/release/AlmondbreadMac $(ARGS)

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
	rm -rf .build Almondbread.xcodeproj dist
