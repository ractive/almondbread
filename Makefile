# Almondbread build shortcuts. xcode-select may point at the Command Line Tools; xcodebuild
# needs the full Xcode, so pin it unless the caller already set DEVELOPER_DIR.
DEVELOPER_DIR ?= /Applications/Xcode.app/Contents/Developer
export DEVELOPER_DIR

DERIVED := .build/xcode
APP     := $(DERIVED)/Build/Products/Release/Almondbread.app

.PHONY: run build project app ios-sim render selftest bench clean

## swift run, release (the macOS app without an Xcode project)
run:
	swift run -c release AlmondbreadMac

build:
	swift build -c release

project: project.yml
	xcodegen generate --quiet

## macOS .app via the generated Xcode project (ad-hoc signed)
app: project
	xcodebuild -project Almondbread.xcodeproj -scheme Almondbread -configuration Release -destination 'platform=macOS,arch=arm64' \
		-derivedDataPath $(DERIVED) -quiet build
	@echo "$(abspath $(APP))"

## iOS simulator build. Uses -sdk rather than -destination so it works without an installed
## simulator runtime (generic/platform=iOS Simulator needs one).
ios-sim: project
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

clean:
	rm -rf .build Almondbread.xcodeproj
