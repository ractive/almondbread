# Almondbread build shortcuts. xcode-select may point at the Command Line Tools; xcodebuild
# needs the full Xcode, so pin it unless the caller already set DEVELOPER_DIR.
DEVELOPER_DIR ?= /Applications/Xcode.app/Contents/Developer
export DEVELOPER_DIR

DERIVED := .build/xcode
APP     := $(DERIVED)/Build/Products/Release/Almondbread.app

.PHONY: run build project app ios-sim render selftest bench regress clean rust rust-check core-check

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
	@# The kernels are compiled at runtime from the shipped source: it must be in the bundle.
	@test -f .build/xcode-ios/Debug-iphonesimulator/Almondbread.app/Almondbread_AlmondbreadKit.bundle/Mandelbrot.metalsrc \
		|| { echo "ios-sim: Mandelbrot.metalsrc missing from the app bundle"; exit 1; }
	@echo "$(abspath .build/xcode-ios/Debug-iphonesimulator/Almondbread.app)"

## Headless check render: make render ARGS="--center -0.7436,0.1318 --scale 1e-8"
render: build
	.build/release/AlmondbreadMac --render render.png --size 1600x1000 $(ARGS)

## Launch checks: the swift-run app and the Xcode .app each render one frame through the real
## view and must print "renderer OK"; a running process alone proves nothing.
selftest: build app
	.build/release/AlmondbreadMac --selftest
	@test ! -f $(APP)/Contents/Resources/Almondbread_AlmondbreadKit.bundle/Contents/Resources/default.metallib \
		|| { echo "selftest: stale default.metallib in the app bundle"; exit 1; }
	$(APP)/Contents/MacOS/Almondbread --selftest

## Benchmark set with GPU-vs-CPU verify metrics (kb: synthesis/benchmarks)
bench: build
	.build/release/AlmondbreadMac --bench $(ARGS)

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

## The Rust core through the Swift bindings, checked bit for bit against AlmondbreadMath,
## plus FFI call overhead
core-check: rust
	swift run -c release almondbread-core-check

## Headless sessions through the interactive scheduler (Space zoom, then one wheel step) at the
## -3/4 neck views of the wedge report; each fails on exterior painted as interior or on a
## final frame that differs from a fresh render (kb: techniques/interior-detection, Pitfalls)
regress: build
	.build/release/AlmondbreadMac --verify --history zoom --center -0.7500016386523336,-0.0021073424656852589 \
		--scale 3.38526e-8 --ppp 2 --size 640x410 --then 1.3225
	.build/release/AlmondbreadMac --verify --history zoom --center -0.75000661155604931,-0.0021104008406531076 \
		--scale 1.062288e-9 --ppp 2 --size 640x410 --pointer 0,-58.5 --then 1.3225
	.build/release/AlmondbreadMac --verify --history zoom --center -0.743643887037151,0.13182590420533 \
		--scale 8.5e-9 --size 1280x820 --pointer 200,100

clean:
	rm -rf .build Almondbread.xcodeproj
