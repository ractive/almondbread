// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Almondbread",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .library(name: "AlmondbreadKit", targets: ["AlmondbreadKit"]),
        .executable(name: "AlmondbreadMac", targets: ["AlmondbreadMac"]),
        .library(name: "AlmondbreadMath", targets: ["AlmondbreadMath"]),
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
