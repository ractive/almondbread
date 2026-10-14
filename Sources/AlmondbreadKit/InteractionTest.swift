#if os(macOS)
import AlmondbreadCore
import AppKit
import QuartzCore

/// `--interaction-test`: drives the real window and the Rust renderer with synthetic pans,
/// zooms and keys through the view's own input path, one input per display refresh (like a
/// trackpad), and prints frame pacing and latency per scenario, then quits. Interaction
/// smoothness is an acceptance criterion; nobody watches the screen during agent runs, so it
/// is measured: presentation times come from the drawables (Rust's presented handlers), draw
/// CPU is the time inside `Explorer.frame`, and the Swift-side time of the same call gives
/// the FFI overhead per frame. Runs in the background (TestLaunch) and quits by itself.
@MainActor
final class InteractionTest: NSObject {
    static let enabled = CommandLine.arguments.contains("--interaction-test")
    static let shared = InteractionTest()

    private weak var view: FractalNSView?
    private var link: CADisplayLink?
    private var scenario = -1
    private var phaseStart: CFTimeInterval = 0
    private var tick = 0
    private var settling = true
    private var inputEnd: CFTimeInterval = 0
    private var reported = false
    private var startViewport: ViewportInfo?
    private var refineLines: [String] = []
    private var ffiLines: [String] = []
    private var failures = 0

    private struct View { let re: String, im: String, scale: Double }
    private struct Scenario {
        let name: String
        let viewport: View
        let input: (FractalNSView, Int, Double) -> Void   // view, tick, seconds since input start
    }
    private let inputSeconds = 3.0
    private let settleSeconds = 1.0

    private lazy var scenarios: [Scenario] = {
        // The home view fitted to 1280x820 points (Viewport::fitting).
        let home = View(re: "-0.75", im: "0", scale: max(3.2 / 1280, 2.6 / 820))
        let sea = View(re: "-0.743643887037151", im: "0.13182590420533", scale: 3.75e-3 / 1e5)
        // Circular pan, 10 pt per frame; zoom in 2% per frame around an off-centre point.
        let pan: (FractalNSView, Int, Double) -> Void = { v, i, _ in
            let a = Double(i) * 0.05
            v.send(.pan(dx: 10 * cos(a), dy: 10 * sin(a)))
        }
        let zoom: (FractalNSView, Int, Double) -> Void = { v, i, _ in
            v.send(.magnify(magnification: i < 90 ? 0.02 : 1 / 1.02 - 1, x: 120, y: -60))
        }
        // Continuous zoom into the Double precision limit (minScale): the clamp must hold
        // without the centre drifting.
        let limit = View(re: "-0.743643887037151", im: "0.13182590420533", scale: 1e-14)
        let deep: (FractalNSView, Int, Double) -> Void = { v, _, _ in v.send(.magnify(magnification: 0.05, x: 200, y: 100)) }
        // Keyboard, scheduled in seconds (tick counts depend on the achieved frame rate): hold
        // D, add W at 1 s (diagonal), release both at 2.5 s; Space zoom with one E press at
        // 1.25 s, released at 2.5 s.
        var sent = Set<String>()
        func once(_ id: String, _ due: Bool, _ action: () -> Void) {
            if due && !sent.contains(id) { sent.insert(id); action() }
        }
        let wasd: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 { sent.removeAll() }
            once("d", true) { InteractionTest.key(v, "d", down: true) }
            once("w", t >= 1) { InteractionTest.key(v, "w", down: true) }
            once("up", t >= 2.5) { InteractionTest.key(v, "d", down: false); InteractionTest.key(v, "w", down: false) }
        }
        let space: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 { sent.removeAll() }
            once("space", true) { InteractionTest.key(v, " ", down: true) }
            once("e", t >= 1.25) { InteractionTest.key(v, "e", down: true); InteractionTest.key(v, "e", down: false) }
            once("up", t >= 2.5) { InteractionTest.key(v, " ", down: false) }
        }
        // The -3/4 neck of the wedge report (zoom 2.2e5): wheel-like zoom in and out.
        let neck = View(re: "-0.7500016386523336", im: "-0.0021073424656852589", scale: 1.69263e-8)

        // Controls revision (kb: decisions/controls): the zoom target's fixed point, the
        // keyboard crosshair's glide and edge panning, driven per refresh like the keys above.
        let target = (x: 380.0, y: -200.0)
        var c0: (re: Double, im: Double)?
        var zoomStart = 0.0
        // Pointer mode, pointer off-centre, Space held 2.5 s: the coordinate under the
        // pointer must stay put (< 1 px per second of zooming).
        let pointerSpace: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 {
                sent.removeAll()
                v.send(.pointer(x: target.x, y: target.y))
                c0 = InteractionTest.point(v, target.x, target.y)
                zoomStart = CACurrentMediaTime()
                InteractionTest.key(v, " ", down: true)
            }
            once("up", t >= 2.5) {
                InteractionTest.key(v, " ", down: false)
                self.checkDrift("pointer space", v, c0, target.x, target.y, seconds: CACurrentMediaTime() - zoomStart,
                                note: String(format: "pointer at (%.0f, %.0f) pt, mode %@", target.x, target.y,
                                             "\(v.base.mode)"))
            }
        }
        // From pointer mode, D tapped for 0.1 s while Space is held: keyboard mode, the
        // crosshair glides from the pointer to the centre (no overshoot, ~0.27 s to 1 %), the
        // zoom follows it; after arrival the centre must stay put.
        var glide = (start: 0.0, last: 0.0, pct1: Double?.none, arrived: Double?.none, overshoot: false, monotonic: true)
        let keyboardGlide: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 {
                sent.removeAll()
                v.send(.pointer(x: target.x, y: target.y))
                c0 = nil
                InteractionTest.key(v, " ", down: true)
                InteractionTest.key(v, "d", down: true)
                let d0 = hypot(target.x, target.y)
                glide = (CACurrentMediaTime(), d0, nil, nil, false, true)
            }
            if let f = v.base.lastFrame, f.keyboard {
                let d = hypot(f.x, f.y)
                if d > glide.last + 1e-9 { glide.monotonic = false }
                // Started at (+x, −y): crossing either axis would be an overshoot.
                if f.x < -1e-9 || f.y > 1e-9 { glide.overshoot = true }
                glide.last = d
                let now = CACurrentMediaTime() - glide.start
                if glide.pct1 == nil && d < 0.01 * hypot(target.x, target.y) { glide.pct1 = now }
                if glide.arrived == nil && d == 0 { glide.arrived = now }
                // The fixed point is measured once the tapped key's pan has glided out too
                // (FL: released keys ease out over ~0.5 s instead of stopping dead).
                if c0 == nil && glide.arrived != nil && now >= 0.9 {
                    c0 = InteractionTest.point(v, 0, 0)
                    zoomStart = CACurrentMediaTime()
                }
            }
            once("dup", t >= 0.1) { InteractionTest.key(v, "d", down: false) }
            once("up", t >= 2.5) {
                InteractionTest.key(v, " ", down: false)
                let ok = !glide.overshoot && glide.monotonic && (glide.pct1 ?? 1) <= 0.35 && glide.arrived != nil
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "keyboard glide: mode %@, crosshair from (%.0f, %.0f) pt: 1 %% after %.0f ms, at the centre after %.0f ms, overshoot %@, monotonic %@ %@",
                                               "\(v.base.mode)", target.x, target.y, (glide.pct1 ?? .nan) * 1000,
                                               (glide.arrived ?? .nan) * 1000, glide.overshoot ? "YES" : "no",
                                               glide.monotonic ? "yes" : "NO", ok ? "OK" : "FAILED"))
                self.checkDrift("keyboard glide", v, c0, 0, 0, seconds: CACurrentMediaTime() - zoomStart,
                                note: "centre after arrival")
            }
        }
        // Pointer mode, pointer 8 pt inside the right edge: no pan without a zoom (setting
        // "while zooming"), then Space: the view pans right at the ramped key speed; the
        // pointer leaves the window at 2 s: the pan stops, the zoom goes on.
        var edge = (prev: ViewportInfo?.none, start: ViewportInfo?.none, pan: 0.0, panAfterExit: 0.0, idleMoved: false,
                    x: 0.0, expected: 0.0, panStart: 0.0, panSeconds: 0.0)
        let edgePan: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                let ins = v.safeAreaInsets
                let w = v.bounds.width - ins.left - ins.right, h = v.bounds.height - ins.top - ins.bottom
                let band = 0.08 * Double(min(w, h))
                let x = Double(v.bounds.width / 2 - ins.right) - 8
                let r = max(0, min(1, 1 - 8 / band))
                edge = (nil, explorer.viewport(), 0, 0, false, x, 0.4 * r * r * (3 - 2 * r), 0, 0)
                v.send(.pointer(x: x, y: -100))
            }
            let vp = explorer.viewport()
            if t < 0.5 {
                if vp != edge.start { edge.idleMoved = true }
                return
            }
            once("space", true) {
                InteractionTest.key(v, " ", down: true)
                edge.prev = vp
                edge.panStart = CACurrentMediaTime()
            }
            // Pan of this step in points: the centre's move minus what zooming at the target
            // alone moves it (the pan is applied before the zoom, at the previous scale).
            if let p = edge.prev, let re = Double(vp.re), let pre = Double(p.re) {
                let tx = sent.contains("exit") ? 0 : edge.x
                let pan = (re - (pre + tx * (p.scale - vp.scale))) / p.scale
                if sent.contains("exit") { edge.panAfterExit += pan } else { edge.pan += pan }
            }
            edge.prev = vp
            once("exit", t >= 2.0) {
                v.send(.pointerExited)
                edge.panSeconds = CACurrentMediaTime() - edge.panStart
            }
            once("up", t >= 2.5) {
                InteractionTest.key(v, " ", down: false)
                let rate = edge.pan / max(edge.panSeconds, 1e-3) / Double(v.bounds.width)
                // FL motion feel: the pan eases in (critically damped, ω = 30/s: the average
                // over the run loses 2/ω s of speed) and glides out after the pointer left
                // (2/ω = 1/9 s of speed at ω = 18/s).
                let expected = edge.expected * (1 - (2.0 / 30) / max(edge.panSeconds, 0.1))
                let glide = edge.expected * Double(v.bounds.width) * 2 / 18
                let ok = !edge.idleMoved && abs(rate - expected) <= 0.1 * expected
                    && abs(edge.panAfterExit - glide) <= 0.35 * glide
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "edge pan: pointer %.0f pt right of centre: %.3f view widths/s while zooming (expected %.3f with the eased start), moved without zoom %@, glided %.1f pt after the pointer left (expected %.1f) %@",
                                               edge.x, rate, expected, edge.idleMoved ? "YES" : "no",
                                               edge.panAfterExit, glide, ok ? "OK" : "FAILED"))
            }
        }
        return [Scenario(name: "home pan", viewport: home, input: pan),
                Scenario(name: "neck zoom", viewport: neck, input: zoom),
                Scenario(name: "wasd 1e5", viewport: sea, input: wasd),
                Scenario(name: "space zoom", viewport: sea, input: space),
                Scenario(name: "home zoom", viewport: home, input: zoom),
                Scenario(name: "1e5 pan", viewport: sea, input: pan),
                Scenario(name: "1e5 zoom", viewport: sea, input: zoom),
                Scenario(name: "limit zoom", viewport: limit, input: deep),
                Scenario(name: "pointer space", viewport: sea, input: pointerSpace),
                Scenario(name: "keyboard glide", viewport: sea, input: keyboardGlide),
                Scenario(name: "edge pan", viewport: sea, input: edgePan)]
    }()

    /// The complex coordinate at (x, y) view points from the centre.
    static func point(_ v: FractalNSView, _ x: Double, _ y: Double) -> (re: Double, im: Double)? {
        guard let vp = v.explorer?.viewport(), let re = Double(vp.re), let im = Double(vp.im) else { return nil }
        return (re + x * vp.scale, im + y * vp.scale)
    }

    /// The fixed-point property of the zoom target: how far the coordinate that was under
    /// (x, y) moved on screen, in pixels per second of zooming (must be < 1).
    private func checkDrift(_ name: String, _ v: FractalNSView, _ c0: (re: Double, im: Double)?, _ x: Double, _ y: Double,
                            seconds: Double, note: String) {
        guard let c0, let c1 = Self.point(v, x, y), let vp = v.explorer?.viewport(), seconds > 0.2 else {
            failures += 1
            refineLines.append("\(name): fixed point not measured (\(note)) FAILED")
            return
        }
        let px = hypot(c1.re - c0.re, c1.im - c0.im) / vp.scale * Double(v.window?.backingScaleFactor ?? 2)
        let rate = px / seconds
        let ok = rate < 1
        if !ok { failures += 1 }
        refineLines.append(String(format: "%@: zoom target drift %.2g px over %.2f s of zooming (%.2g px/s, ×%.3g zoom; %@) %@",
                                  name, px, seconds, rate, startScale(name) / vp.scale, note, ok ? "OK" : "FAILED"))
    }

    private func startScale(_ name: String) -> Double {
        scenarios.first { $0.name == name }?.viewport.scale ?? .nan
    }

    func attach(view: FractalNSView) {
        // ALMONDBREAD_SCENARIOS="home zoom,1e5 pan" runs a subset.
        if let only = ProcessInfo.processInfo.environment["ALMONDBREAD_SCENARIOS"] {
            let names = Set(only.split(separator: ",").map(String.init))
            scenarios = scenarios.filter { names.contains($0.name) }
        }
        guard self.view == nil else { return }
        self.view = view
        // Let the window appear and the first frame render.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { self.start() }
    }

    private func start() {
        guard let view, let window = view.window, view.explorer != nil else {
            print("interaction-test: no window or renderer")
            exit(1)
        }
        window.setContentSize(NSSize(width: 1280, height: 820))
        TestLaunch.placeAtScreenEdge(window)

        let fps = window.screen?.maximumFramesPerSecond ?? 60
        print("interaction-test: window key \(window.isKeyWindow), app active \(NSApp.isActive), "
              + "occlusion visible \(window.occlusionState.contains(.visible))")
        print("interaction-test: Rust renderer, window 1280x820 pt, backing scale \(window.backingScaleFactor), display \(fps) Hz")
        print("| scenario | frames | fps | interval p50 / p95 / max ms | latency p50 / p95 ms (input→draw + draw→screen p50) | draw CPU p50 / p95 ms | GPU ms/frame (iter + colorize) | preview % | skipped draws | draws / encoded |")
        print("|---|---:|---:|---|---|---|---|---:|---:|---|")
        let link = view.displayLink(target: self, selector: #selector(step(_:)))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: min(60, Float(fps)), maximum: Float(fps), preferred: Float(fps))
        view.setPreferredFrameRate(Float(fps))
        link.add(to: .main, forMode: .common)
        self.link = link
        next()
    }

    private func next() {
        scenario += 1
        guard scenario < scenarios.count, let view, let explorer = view.explorer else {
            link?.invalidate()
            refineLines.forEach { print($0) }
            ffiLines.forEach { print($0) }
            print(failures == 0 ? "interaction-test: done" : "interaction-test: FAILED (\(failures) checks)")
            fflush(stdout)
            exit(failures == 0 ? 0 : 1)
        }
        let v = scenarios[scenario].viewport
        do {
            try explorer.setViewport(re: v.re, im: v.im, scale: v.scale)
        } catch {
            print("interaction-test: \(scenarios[scenario].name): \(error)")
            exit(1)
        }
        // Every scenario starts in pointer mode with the pointer off the canvas (the real
        // pointer is ignored during the test): zooms go to the centre unless a scenario
        // moves the synthetic pointer. Pointer travel switches back from keyboard mode.
        view.send(.pointerExited)
        view.send(.pointer(x: 0, y: 0))
        view.send(.pointer(x: 50, y: 0))
        view.send(.pointerExited)
        explorer.setEdgePan(mode: .whileZooming)
        view.requestFrames()
        settling = true
        reported = false
        phaseStart = CACurrentMediaTime()
        tick = 0
    }

    @objc private func step(_ link: CADisplayLink) {
        guard let view, let explorer = view.explorer, scenario < scenarios.count else { return }
        let now = CACurrentMediaTime()
        let name = scenarios[scenario].name
        if settling {
            // Settle: the view renders and refines without input, then measurement starts.
            if now - phaseStart > settleSeconds {
                settling = false
                phaseStart = now
                explorer.beginMeasurement()
                view.base.frameTimes = []
            }
            return
        }
        if now - phaseStart < inputSeconds {
            scenarios[scenario].input(view, tick, now - phaseStart)
            tick += 1
            return
        }
        if !reported {
            let r = explorer.endMeasurement()
            report(name, r)
            let swift = view.base.frameTimes ?? []
            view.base.frameTimes = nil
            if !swift.isEmpty {
                let mean = swift.reduce(0, +) / Double(swift.count) * 1000
                ffiLines.append(String(format: "%@: frame() %d calls, Swift-side mean %.3f ms, inside Rust %.3f ms: FFI + call %.1f µs per frame; skipped %d in flight + %d no drawable",
                                       name, swift.count, mean, r.drawCpuMean, (mean - r.drawCpuMean) * 1000,
                                       r.skippedInFlight, r.skippedNoTarget))
            }
            reported = true
            inputEnd = now
            let v = explorer.viewport()
            let s = explorer.stats()
            if name == "space zoom" {
                refineLines.append(String(format: "space zoom: zoom speed now %.2f e/s (E pressed once from 1.6), zoom %.3g, keys held %d",
                                          s.zoomSpeed, v.zoom, s.heldKeys))
            }
            if name == "wasd 1e5", let re = Double(v.re), let im = Double(v.im),
               let re0 = Double(scenarios[scenario].viewport.re), let im0 = Double(scenarios[scenario].viewport.im) {
                refineLines.append(String(format: "wasd 1e5: centre moved %.3g, %.3g view widths, keys held after release %d",
                                          (re - re0) / (v.scale * 1280), (im - im0) / (v.scale * 1280), s.heldKeys))
            }
            if s.heldKeys > 0 {
                refineLines.append("\(name): \(s.heldKeys) keys still held at input end (released)")
                view.send(.focusLost)
            }
            if name == "limit zoom" {
                refineLines.append(String(format: "limit zoom: scale %.4g (minScale %.4g, at limit %@), centre %@, %@",
                                          v.scale, v.minScale, v.atPrecisionLimit ? "yes" : "no", v.re, v.im))
                startViewport = v
            }
        }
        // After input stops, the fully refined full-resolution frame must arrive.
        // Measured from the last real input (keyboard scenarios release keys before the phase ends).
        let final = explorer.finalFrame()
        if final.time > final.lastInput && now - inputEnd > 0.05 {
            refineLines.append(String(format: "%@: full-res %@ final frame %.0f ms after input stopped",
                                      name, final.size, (final.time - final.lastInput) * 1000))
            // The shown frame must match a fresh headless render of the same view: no stale,
            // unfinished or misclassified pixels from the live pipeline (wedge bug, kb:
            // techniques/interior-detection, Pitfalls).
            if let m = explorer.displayedFrameMismatch() {
                let ok = m.percent <= 0.05
                if !ok { failures += 1 }
                refineLines.append(String(format: "%@: shown %dx%d frame vs fresh render: %d pixels (%.3f%%) differ in class %@",
                                          name, m.width, m.height, m.pixels, m.percent, ok ? "OK" : "FAILED"))
            }
            if let v0 = startViewport {
                let v = explorer.viewport()
                let dre = (Double(v.re) ?? .nan) - (Double(v0.re) ?? .nan)
                let dim = (Double(v.im) ?? .nan) - (Double(v0.im) ?? .nan)
                refineLines.append(String(format: "limit zoom: centre drift during clamped zoom %.3g / %.3g (scale units %.3g)",
                                          dre, dim, v.scale))
                startViewport = nil
            }
            next()
        } else if now - inputEnd > 10 {
            failures += 1
            refineLines.append("\(name): NO full-res final frame within 10 s")
            next()
        }
    }

    /// A synthetic key event through the same handler the window's key monitor uses.
    static func key(_ view: FractalNSView, _ chars: String, down: Bool) {
        guard let e = NSEvent.keyEvent(with: down ? .keyDown : .keyUp, location: .zero, modifierFlags: [],
                                       timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: view.window?.windowNumber ?? 0, context: nil,
                                       characters: chars, charactersIgnoringModifiers: chars,
                                       isARepeat: false, keyCode: 0) else { return }
        _ = view.handleKey(e)
    }

    private func report(_ name: String, _ r: InteractionReport) {
        print(String(format: "| %@ | %d | %.1f | %.1f / %.1f / %.1f | %.1f / %.1f (%.1f + %.1f) | %.2f / %.2f | %.2f + %.2f | %.0f | %d | %d / %d |",
                     name, r.presented, r.fps, r.intervalP50, r.intervalP95, r.intervalMax,
                     r.latencyP50, r.latencyP95, r.inputToDrawP50, r.drawToPresentP50,
                     r.drawCpuP50, r.drawCpuP95, r.iterateGpuPerFrame, r.colorizeGpuPerFrame,
                     r.previewPct, r.skipped, r.draws, r.encoded))
        if r.presented == 0 {
            let visible = view?.window?.occlusionState.contains(.visible) ?? false
            refineLines.append("\(name): no presentation times (window \(visible ? "visible" : "occluded")): frame rate and latency not measurable")
        }
        fflush(stdout)
    }
}
#endif
