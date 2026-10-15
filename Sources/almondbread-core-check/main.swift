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
check(pals.first?.id == "aurora" && pals.count >= 10 && pals.allSatisfy { !$0.stops.isEmpty }, "palettes: \(pals.map(\.id))")
let lks = looks()
check(lks.first?.id == "aurora" && (6...10).contains(lks.count)
      && lks.allSatisfy { l in pals.contains { $0.id == l.paletteId } }, "looks: \(lks.map(\.id))")

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
// Settling includes the progressive AA samples (FL): give the GPU wall time per frame.
func settle(_ limit: Int) {
    while !explorer.stats().settled && frames < limit {
        _ = explorer.frame(timestamp: t)
        t += 1.0 / 120
        frames += 1
        Thread.sleep(forTimeInterval: 0.001)
    }
}
settle(6000)
check(explorer.stats().settled && explorer.finishFrames() == 0, "explorer did not settle in \(frames) frames")
// Pointer / keyboard modes through the bindings: a direction key switches to keyboard mode,
// the crosshair starts at the pointer and glides to the centre; pointer travel switches back.
_ = explorer.input(event: .pointer(x: 100, y: -40))
let kd = explorer.input(event: .keyDown(key: .left))
_ = explorer.input(event: .keyUp(key: .left))
var fr = explorer.frame(timestamp: t)
check(kd.mode == .keyboard && fr & 2 != 0 && explorer.crosshairX() > 0 && explorer.crosshairX() <= 100,
      "keyboard mode and crosshair: \(kd), bits \(fr), x \(explorer.crosshairX())")
for _ in 0..<240 { t += 1.0 / 120; fr = explorer.frame(timestamp: t) }
check(explorer.crosshairX() == 0 && explorer.crosshairY() == 0, "crosshair glided to the centre")
// The tapped key's pan glided the view on (FL motion): it refines and settles, then the
// link may pause although keyboard mode stays.
settle(frames + 6000)
fr = explorer.frame(timestamp: t)
check(fr & 1 == 0 && fr & 2 != 0, "settled in keyboard mode, link may pause: bits \(fr)")
let back = explorer.input(event: .pointer(x: 100, y: 0))
check(back.mode == .pointer && back.redraw && explorer.frame(timestamp: t) & 2 == 0, "pointer travel: pointer mode")
explorer.setEdgePan(mode: .always)
check(explorer.edgePan() == .always, "edge pan setting")
explorer.setEdgePan(mode: .whileZooming)
// Autopilot and fly-to through the bindings (kb: decisions/controls): frame bit 4 says the
// autopilot is on; Esc stops it and is not ours when nothing flies; jumps cancel a flight.
let ap = explorer.input(event: .keyDown(key: .autopilot))
_ = explorer.input(event: .keyUp(key: .autopilot))
t += 1.0 / 120
fr = explorer.frame(timestamp: t)
check(ap.redraw && fr & 4 != 0 && explorer.stats().autopilot.active, "autopilot on: bits \(fr)")
check(explorer.input(event: .keyDown(key: .stop)).handled && !explorer.stats().autopilot.active, "Esc stops the autopilot")
check(!explorer.input(event: .keyDown(key: .stop)).handled, "Esc is not ours when nothing flies")
check(explorer.flyToCoordinates(text: "re -0.75\nim 0.1\nscale 1e-6 (zoom 1×)") && explorer.stats().flight.active,
      "fly to coordinates")
check(!explorer.flyToCoordinates(text: "nonsense"), "fly to rejects other text")
explorer.resetViewport()
check(!explorer.stats().flight.active, "reset cancels the flight")
settle(frames + 6000)
let png = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : NSTemporaryDirectory() + "almondbread-core-check.png"
let off = try explorer.renderPng(path: png, width: 640, height: 400)
check(FileManager.default.fileExists(atPath: png) && off.chunks > 0, "offscreen PNG \(png)")
print(String(format: "explorer: settled after %d frames; offscreen 640x400 %@ in %.1f ms GPU", frames, png, off.iterateGpuSeconds * 1000))

// Places (kb: synthesis/places): the catalogue, a random pick, and a jump to the deepest one.
let pls = places()
check(pls.count >= 40 && Set(pls.map(\.id)).count == pls.count, "places: \(pls.count)")
check(place(id: "seahorse-valley")?.category == .valley && place(id: "nowhere") == nil, "place lookup")
check(randomPlace(seed: 7, category: .deep).category == .deep, "random place")
check(abs((Double(try placeScale(id: "whole-set", width: 320, height: 200)) ?? 0) - 0.012) < 1e-15, "place scale")
let placeSettings = try explorer.goToPlace(id: "spiral-1e1000")
let pv = explorer.viewport()
check(placeSettings.paletteId == "ice" && placeSettings.autoIterations && pv.re.hasPrefix("-0.775683768009053797")
      && pv.scaleText.hasSuffix("e-1003"), "go to place: \(pv.scaleText) \(placeSettings.paletteId)")
let placeFrames = frames
settle(frames + 6000)
check(explorer.stats().settled && explorer.finishFrames() == 0, "deep place did not settle")
print(String(format: "places: %d; spiral-1e1000 settled after %d frames, maxIter %d", pls.count, frames - placeFrames,
             explorer.stats().maxIterations))

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
// an idle view (nothing new to present: the call is the scheduler's bookkeeping only).
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
