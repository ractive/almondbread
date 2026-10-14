// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Almondbread",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .library(name: "AlmondbreadKit", targets: ["AlmondbreadKit"]),
        .executable(name: "AlmondbreadMac", targets: ["AlmondbreadMac"]),
        .library(name: "AlmondbreadMath", targets: ["AlmondbreadMath"]),
        .library(name: "AlmondbreadCore", targets: ["AlmondbreadCore"]),
    ],
    targets: [
        // Shared renderer, model, Metal kernels and SwiftUI views (macOS, iOS, iPadOS).
        .target(
            name: "AlmondbreadKit",
            resources: [
                // Copied verbatim and compiled at runtime with explicit options (math mode etc.).
                // Not named .metal: Xcode would compile a .metal resource into default.metallib
                // with its own options and drop the source (the M1 "Metal Unavailable" bug).
                .copy("Shaders/Mandelbrot.metalsrc")
            ],
            swiftSettings: [
                .unsafeFlags(["-Ounchecked"], .when(configuration: .release))
            ]
        ),
        // `swift run AlmondbreadMac`: the macOS app without an Xcode project, for quick iteration.
        .executableTarget(
            name: "AlmondbreadMac",
            dependencies: ["AlmondbreadKit"]
        ),
        // Arbitrary-precision BigFloat, reference orbits, period detection and Newton nuclei
        // (pure Swift, no GMP: kb decisions/arbitrary-precision).
        .target(
            name: "AlmondbreadMath",
            swiftSettings: [
                .unsafeFlags(["-Ounchecked"], .when(configuration: .release))
            ]
        ),
        // `swift run -c release almondbread-math-check [bench|nucleus|deep|ops]`.
        .executableTarget(
            name: "almondbread-math-check",
            dependencies: ["AlmondbreadMath"],
            swiftSettings: [
                .unsafeFlags(["-Ounchecked"], .when(configuration: .release))
            ]
        ),
        // Rust core (crates/, kb decisions/rust-core): static libs for macOS arm64, iOS arm64 and
        // the iOS simulator, built by `make rust` (scripts/build-rust-xcframework.sh).
        .binaryTarget(
            name: "AlmondbreadCoreFFI",
            path: "build/rust/AlmondbreadCoreFFI.xcframework"
        ),
        // Swift face of the Rust core: UniFFI bindings (Generated/, written by `make rust`) plus
        // thin helpers. kb platform/rust-swift-ffi.
        .target(
            name: "AlmondbreadCore",
            dependencies: ["AlmondbreadCoreFFI"]
        ),
        // `swift run -c release almondbread-core-check`: Rust core vs AlmondbreadMath, FFI cost.
        .executableTarget(
            name: "almondbread-core-check",
            dependencies: ["AlmondbreadCore", "AlmondbreadMath"],
            swiftSettings: [
                .unsafeFlags(["-Ounchecked"], .when(configuration: .release))
            ]
        ),
        // Oracle tests; fixtures from Tests/AlmondbreadMathTests/oracle/generate_fixtures.py.
        .testTarget(
            name: "AlmondbreadMathTests",
            dependencies: ["AlmondbreadMath"],
            exclude: ["oracle"],
            resources: [.copy("Fixtures")]
        ),
    ],
    swiftLanguageModes: [.v5]
)
