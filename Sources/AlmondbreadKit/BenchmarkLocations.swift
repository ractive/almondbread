/// The fixed benchmark set for `AlmondbreadMac --bench` (kb: synthesis/benchmarks). Keep the
/// entries stable: the kb benchmark table compares runs across milestones by name.
public struct BenchmarkLocation: Sendable {
    public let name: String
    public let centerRe: Double
    public let centerIm: Double
    /// Complex width of the view; scale per pixel = width / image width.
    public let width: Double
    public let maxIterations: Int
    public let note: String

    public func scale(forWidth pixels: Int) -> Double { width / Double(pixels) }

    public static let all: [BenchmarkLocation] = [
        BenchmarkLocation(name: "home", centerRe: -0.75, centerIm: 0, width: 3.9, maxIterations: 1_000,
                          note: "whole set, cheap baseline"),
        BenchmarkLocation(name: "home-100k", centerRe: -0.75, centerIm: 0, width: 3.9, maxIterations: 100_000,
                          note: "about a third interior pixels: interior detection case"),
        BenchmarkLocation(name: "seahorse-1e5", centerRe: -0.743643887037151, centerIm: 0.13182590420533,
                          width: 1.3e-4, maxIterations: 2_000, note: "seahorse valley spirals, zoom ~1e5"),
        BenchmarkLocation(name: "elephant", centerRe: 0.2925, centerIm: 0.0149, width: 0.025, maxIterations: 5_000,
                          note: "elephant valley, near the cardioid cusp"),
        BenchmarkLocation(name: "minibrot-p4", centerRe: -1.9407998065294847, centerIm: 0, width: 0.0045,
                          maxIterations: 50_000, note: "period-4 minibrot centred, interior-heavy"),
        BenchmarkLocation(name: "deep-5e-14", centerRe: -0.7436438870371587, centerIm: 0.13182590420531198,
                          width: 1.728e-10, maxIterations: 6_000, note: "scale 5e-14/px at 3456 wide, near Double limit"),
        BenchmarkLocation(name: "feigenbaum", centerRe: -1.4011551890920506, centerIm: 0, width: 2e-7,
                          maxIterations: 200_000, note: "high-iteration filaments around the Feigenbaum point"),
    ]

    public static func named(_ name: String) -> BenchmarkLocation? { all.first { $0.name == name } }
}
