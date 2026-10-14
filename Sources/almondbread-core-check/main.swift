// almondbread-core-check: headless check of the Swift-Rust bridge (kb: platform/rust-swift-ffi).
//
//   swift run -c release almondbread-core-check [out.png]       (make core-check)
//
// Calls the Rust core through the UniFFI bindings (AlmondbreadCore) and checks the round
// trips the app relies on: coordinate parse/format, a reference orbit copied into caller
// memory, cancellation from another thread, the palette list, and the Explorer on a
// windowless CAMetalLayer (settings, viewport, input, frames, an offscreen PNG). The maths
// itself is tested in Rust (cargo test: oracle fixtures, property tests). Then measures the
// FFI call costs, including the per-frame overhead of `Explorer.frame`. Exits non-zero on
// any failure.

import AlmondbreadCore
import Dispatch
import Foundation
import QuartzCore

var failures = 0
func check(_ ok: Bool, _ what: @autoclosure () -> String) {
    if !ok { failures += 1; print("FAIL: \(what())") }
}

func now() -> UInt64 { DispatchTime.now().uptimeNanoseconds }

print(coreVersion())
let layout = floatExpComplexLayout()
check(layout == [16, 8], "FloatExpComplex layout \(layout), expected [16, 8]")

// 1. Coordinates: correctly rounded parse + format (exact values from the Rust oracle tests).
let c = try normalizeCoordinate(re: "-0.75", im: "0.1", precisionBits: 128, digits: 5)
check(c.re == "-0.75" && c.im == "0.1", "normalize: \(c)")
let pi = try normalizeCoordinate(re: "3.14159265358979323846264338327950288", im: "0", precisionBits: 128, digits: 30)
check(pi.re == "3.14159265358979323846264338328", "128-bit pi to 30 digits: \(pi.re)")
do {
    _ = try normalizeCoordinate(re: "1.2.3", im: "0", precisionBits: 64, digits: nil)
    check(false, "malformed coordinate accepted")
} catch CoreError.Malformed {
} catch {
    check(false, "unexpected error \(error)")
}

// 2. Reference orbit: copied once into caller memory; Z_1 = C.
let orbit = try computeReferenceOrbit(re: "-0.1", im: "0.1", precisionBits: 128, maxIterations: 1000, bailout: 2, cancel: nil)
check(orbit.count() == 1001 && !orbit.escaped(), "orbit count \(orbit.count())")
var pts = [SIMD2<Float>](repeating: .zero, count: Int(orbit.count()))
let copied = pts.withUnsafeMutableBytes { orbit.copyPoints(into: $0) }
check(copied == pts.count && pts[1] == SIMD2<Float>(-0.1, 0.1), "orbit copy: \(copied) points, Z1 \(pts[1])")
let sel = try selectReference(re: "-1.752", im: "0.0005", radius: "0.01", maxIterations: 1000, precisionBits: 128,
                              method: .ball, cancel: nil)
check(sel.isNucleus && sel.period == 3, "reference selection: period \(String(describing: sel.period))")

// 3. Cancellation from another thread.
let token = CancelToken()
DispatchQueue.global().asyncAfter(deadline: .now() + .milliseconds(20)) { token.cancel() }
let cancelled = try computeReferenceOrbit(re: "-0.1", im: "0.1", precisionBits: 4096, maxIterations: 100_000_000,
                                          bailout: 2, cancel: token)
check(cancelled.cancelled() && cancelled.count() < 100_000_001 && token.progress() > 0,
      "cancellation: cancelled=\(cancelled.cancelled()) count=\(cancelled.count())")

// 4. Palettes.
let pals = palettes()
check(pals.first?.id == "classic" && pals.count >= 7 && pals.allSatisfy { !$0.stops.isEmpty }, "palettes: \(pals.map(\.id))")

// 5. The Explorer on a windowless layer (the app's path minus the window).
let layer = CAMetalLayer()
let explorer = try Explorer(layer: UInt64(UInt(bitPattern: Unmanaged.passUnretained(layer).toOpaque())), scale: 2)
explorer.resize(width: 320, height: 200, scale: 2)
check(layer.drawableSize == CGSize(width: 640, height: 400), "drawable size \(layer.drawableSize)")
let fitted = explorer.viewport()
check(fitted.re == "-0.75" && abs(fitted.scale - 0.013) < 1e-12, "first fit: \(fitted)")
var settings = explorer.settings()
settings.paletteId = "fire"
settings.shading = 0.5
try explorer.setSettings(settings: settings)
check(explorer.settings() == settings, "settings round trip")
settings.supersampling = 7
do {
    try explorer.setSettings(settings: settings)
    check(false, "supersampling 7 accepted")
} catch {}
try explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 3.75e-8)
let r = explorer.input(event: .keyDown(key: .fasterZoom))
check(r.handled && r.redraw && abs(explorer.stats().zoomSpeed - 2.0) < 1e-12, "Q/E through input: \(r)")
check(!explorer.input(event: .pan(dx: .nan, dy: 0)).handled, "NaN pan rejected")
var t = CACurrentMediaTime()
var frames = 0
while !explorer.stats().settled && frames < 2000 {
    _ = explorer.frame(timestamp: t)
    t += 1.0 / 120
    frames += 1
}
check(explorer.stats().settled && explorer.finishFrames() == 0, "explorer did not settle in \(frames) frames")
let png = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : NSTemporaryDirectory() + "almondbread-core-check.png"
let off = try explorer.renderPng(path: png, width: 640, height: 400)
check(FileManager.default.fileExists(atPath: png) && off.chunks > 0, "offscreen PNG \(png)")
print(String(format: "explorer: settled after %d frames; offscreen 640x400 %@ in %.1f ms GPU", frames, png, off.iterateGpuSeconds * 1000))

// 6. FFI costs (best of 5 rounds).
func best(_ rounds: Int = 5, _ body: () throws -> Void) rethrows -> Double {
    var b = Double.infinity
    for _ in 0..<rounds {
        let t0 = now()
        try body()
        b = min(b, Double(now() - t0))
    }
    return b
}
let calls = 1_000_000
var acc: UInt64 = 0
let tPing = best { for i in 0..<calls { acc &+= ping(x: UInt64(i)) } } / Double(calls)
let tVersion = best { for _ in 0..<100_000 { acc &+= UInt64(coreVersion().utf8.count) } } / 100_000
let tInput = best { for i in 0..<100_000 {
    acc &+= explorer.input(event: .pointer(x: Double(i & 63), y: 1)).handled ? 1 : 0
} } / 100_000
let tStats = best { for _ in 0..<100_000 { acc &+= explorer.stats().chunks } } / 100_000
// Per-frame overhead: Swift-side time of `frame` minus the time Rust measures inside it, on
// an idle view (each call still colorizes and presents into the windowless layer).
var swiftFrame = 0.0
explorer.beginMeasurement()
for _ in 0..<300 {
    let t0 = now()
    _ = explorer.frame(timestamp: t)
    swiftFrame += Double(now() - t0)
    t += 1.0 / 120
}
let rep = explorer.endMeasurement()
let swiftMean = swiftFrame / 300 / 1e6
print(String(format: "ffi: ping %.1f ns/call, version string %.0f ns/call, input(pointer) %.0f ns/call, stats() %.0f ns/call",
             tPing, tVersion, tInput, tStats))
print(String(format: "ffi: frame() Swift-side %.4f ms vs inside Rust %.4f ms: %.2f µs per frame for the call",
             swiftMean, rep.drawCpuMean, (swiftMean - rep.drawCpuMean) * 1000))
if acc == 42 { print("") }  // keep the work observable

if failures == 0 {
    print("core-check OK")
} else {
    print("core-check: \(failures) failure(s)")
    exit(1)
}
