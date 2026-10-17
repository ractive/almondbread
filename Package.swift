// swift-tools-version: 6.0
import PackageDescription

// Swift is the UI on Apple platforms; everything else is the Rust workspace in crates/
// (kb: decisions/rust-core, platform/render-architecture).
let package = Package(
    name: "Almondbread",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .library(name: "AlmondbreadKit", targets: ["AlmondbreadKit"]),
        .executable(name: "AlmondbreadMac", targets: ["AlmondbreadMac"]),
        .library(name: "AlmondbreadCore", targets: ["AlmondbreadCore"]),
    ],
    targets: [
        // SwiftUI views, the settings mirror and the platform views that host the Rust
        // renderer's CAMetalLayer (macOS, iOS, iPadOS).
        .target(
            name: "AlmondbreadKit",
            dependencies: ["AlmondbreadCore"],
            // Places catalogue thumbnails (`almondbread places --render-thumbs`; PlaceThumbnails) and
            // the help texts (Markdown, kb: decisions/controls "Help").
            resources: [.copy("Resources/Places"), .copy("Resources/Help")]
        ),
        // `swift run AlmondbreadMac`: the macOS app without an Xcode project, for quick iteration.
        .executableTarget(
            name: "AlmondbreadMac",
            dependencies: ["AlmondbreadKit"]
        ),
        // Rust core and renderer (crates/almondbread-ffi): static libs for macOS arm64, iOS arm64
        // and the iOS simulator, built by `make rust` (scripts/build-rust-xcframework.sh).
        .binaryTarget(
            name: "AlmondbreadCoreFFI",
            path: "build/rust/AlmondbreadCoreFFI.xcframework"
        ),
        // Swift face of the Rust core: UniFFI bindings (Generated/, written by `make rust`) plus
        // thin helpers. kb platform/rust-swift-ffi.
        .target(
            name: "AlmondbreadCore",
            dependencies: ["AlmondbreadCoreFFI"],
            linkerSettings: [
                // The Rust renderer calls Metal, Core Animation and CoreGraphics directly.
                .linkedFramework("Metal"), .linkedFramework("QuartzCore"), .linkedFramework("CoreGraphics"),
            ]
        ),
        // `swift run -c release almondbread-core-check`: the bridge through the real bindings
        // (round trips, Explorer offscreen and per-frame calls) and FFI call costs.
        .executableTarget(
            name: "almondbread-core-check",
            dependencies: ["AlmondbreadCore"]
        ),
    ],
    swiftLanguageModes: [.v5]
)
