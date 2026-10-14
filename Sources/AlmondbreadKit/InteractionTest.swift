#if os(macOS)
import AppKit
import QuartzCore

/// `--interaction-test`: drives the real window with synthetic pans and zooms through the
/// view's own input methods, one input per display refresh (like a trackpad), and prints
/// frame pacing and latency per scenario, then quits. Interaction smoothness is a milestone 2
/// acceptance criterion (60 fps, ideally 120, at most 1–2 frames input-to-photon); nobody
/// watches the screen during agent runs, so it has to be measured.
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
    private var startViewport: Viewport?
    private var refineLines: [String] = []
    private var failures = 0

    private struct Scenario {
        let name: String
        let viewport: Viewport
        let input: (FractalNSView, Int, Double) -> Void   // view, tick, seconds since input start
    }
    private let inputSeconds = 3.0
    private let settleSeconds = 1.0

    private lazy var scenarios: [Scenario] = {
        let home = Viewport.fitting(width: 1280, height: 820)
        let sea = Viewport(centerRe: -0.743643887037151, centerIm: 0.13182590420533, scale: 3.75e-3 / 1e5)
        // Circular pan, 10 pt per frame; zoom in 2% per frame around an off-centre point.
        let pan: (FractalNSView, Int, Double) -> Void = { v, i, _ in
            let a = Double(i) * 0.05
            v.pan(10 * cos(a), 10 * sin(a))
        }
        let zoom: (FractalNSView, Int, Double) -> Void = { v, i, _ in v.zoom(i < 90 ? 1.02 : 1 / 1.02, at: (120, -60)) }
        // Continuous zoom into the Double precision limit (minScale): the clamp must hold
        // without the centre drifting.
        let limit = Viewport(centerRe: -0.743643887037151, centerIm: 0.13182590420533, scale: 1e-14)
        let deep: (FractalNSView, Int, Double) -> Void = { v, _, _ in v.zoom(1.05, at: (200, 100)) }
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
        let neck = Viewport(centerRe: -0.7500016386523336, centerIm: -0.0021073424656852589, scale: 1.69263e-8)
        return [Scenario(name: "home pan", viewport: home, input: pan),
                Scenario(name: "neck zoom", viewport: neck, input: zoom),
                Scenario(name: "wasd 1e5", viewport: sea, input: wasd),
                Scenario(name: "space zoom", viewport: sea, input: space),
                Scenario(name: "home zoom", viewport: home, input: zoom),
                Scenario(name: "1e5 pan", viewport: sea, input: pan),
                Scenario(name: "1e5 zoom", viewport: sea, input: zoom),
                Scenario(name: "limit zoom", viewport: limit, input: deep)]
    }()

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
        guard let view, let window = view.window else {
            print("interaction-test: no window")
            exit(1)
        }
        window.setContentSize(NSSize(width: 1280, height: 820))

        let fps = window.screen?.maximumFramesPerSecond ?? 60
        print("interaction-test: window key \(window.isKeyWindow), app active \(NSApp.isActive), "
              + "occlusion visible \(window.occlusionState.contains(.visible))")
        print("interaction-test: window 1280x820 pt, backing scale \(window.backingScaleFactor), display \(fps) Hz, "
              + "view preferredFramesPerSecond \(view.preferredFramesPerSecond)")
        print("| scenario | frames | fps | interval p50 / p95 / max ms | latency p50 / p95 ms (input→draw + draw→screen p50) | draw CPU p50 / p95 ms | GPU ms/frame (iter + colorize) | preview % | skipped draws | draws / encoded |")
        print("|---|---:|---:|---|---|---|---|---:|---:|---|")
        let link = view.displayLink(target: self, selector: #selector(step(_:)))
        link.add(to: .main, forMode: .common)
        self.link = link
        next()
    }

    private func next() {
        scenario += 1
        guard scenario < scenarios.count, let view, let renderer = view.renderer else {
            link?.invalidate()
            refineLines.forEach { print($0) }
            print(failures == 0 ? "interaction-test: done" : "interaction-test: FAILED (\(failures) frame checks)")
            fflush(stdout)
            exit(failures == 0 ? 0 : 1)
        }
        renderer.frameStats = nil
        renderer.model.viewport = scenarios[scenario].viewport
        settling = true
        reported = false
        phaseStart = CACurrentMediaTime()
        tick = 0
    }

    @objc private func step(_ link: CADisplayLink) {
        guard let view, let renderer = view.renderer, scenario < scenarios.count else { return }
        let now = CACurrentMediaTime()
        if settling {
            // Settle: the view renders and refines without input, then measurement starts.
            if now - phaseStart > settleSeconds {
                settling = false
                phaseStart = now
                renderer.frameStats = Renderer.FrameStats()
            }
            return
        }
        if now - phaseStart < inputSeconds {
            scenarios[scenario].input(view, tick, now - phaseStart)
            tick += 1
            return
        }
        if !reported {
            report(scenarios[scenario].name, renderer.frameStats, duration: now - phaseStart)
            reported = true
            inputEnd = now
            renderer.frameStats = nil
            let v = renderer.model.viewport
            if scenarios[scenario].name == "space zoom" {
                refineLines.append(String(format: "space zoom: zoom speed now %.2f e/s (E pressed once from 1.6), zoom %.3g, keys held %d",
                                          renderer.model.zoomSpeed, v.zoom, view.keys.held.count))
            }
            if scenarios[scenario].name == "wasd 1e5" {
                refineLines.append(String(format: "wasd 1e5: centre moved %.3g, %.3g view widths, keys held after release %d",
                                          (v.centerRe - scenarios[scenario].viewport.centerRe) / (v.scale * 1280),
                                          (v.centerIm - scenarios[scenario].viewport.centerIm) / (v.scale * 1280),
                                          view.keys.held.count))
            }
            if !view.keys.held.isEmpty {
                refineLines.append("\(scenarios[scenario].name): \(view.keys.held.count) keys still held at input end (released)")
                view.keys.releaseAll()
            }
            if scenarios[scenario].name == "limit zoom" {
                refineLines.append(String(format: "limit zoom: scale %.4g (minScale %.4g, at limit %@), centre %.17g, %.17g",
                                          v.scale, v.minScale, v.atPrecisionLimit ? "yes" : "no", v.centerRe, v.centerIm))
                startViewport = v
            }
        }
        // After input stops, the fully refined full-resolution frame must arrive.
        // Measured from the last real input (keyboard scenarios release keys before the phase ends).
        let final = renderer.lastFinalFrame
        let lastInput = renderer.lastInteraction
        if final.time > lastInput && now - inputEnd > 0.05 {
            refineLines.append(String(format: "%@: full-res %@ final frame %.0f ms after input stopped",
                                      scenarios[scenario].name, final.size, (final.time - lastInput) * 1000))
            // The shown frame must match a fresh headless render of the same view: no stale,
            // unfinished or misclassified pixels from the live pipeline (wedge bug, kb:
            // techniques/interior-detection, Pitfalls).
            if let m = renderer.displayedFrameMismatch() {
                let ok = m.percent <= 0.05
                if !ok { failures += 1 }
                refineLines.append(String(format: "%@: shown %@ frame vs fresh render: %d pixels (%.3f%%) differ in class %@",
                                          scenarios[scenario].name, m.size, m.pixels, m.percent, ok ? "OK" : "FAILED"))
            }
            if let v0 = startViewport {
                let v = renderer.model.viewport
                refineLines.append(String(format: "limit zoom: centre drift during clamped zoom %.3g / %.3g (scale units %.3g)",
                                          v.centerRe - v0.centerRe, v.centerIm - v0.centerIm, v.scale))
                startViewport = nil
            }
            next()
        } else if now - inputEnd > 10 {
            refineLines.append("\(scenarios[scenario].name): NO full-res final frame within 10 s")
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

    private func report(_ name: String, _ s: Renderer.FrameStats?, duration: Double) {
        guard let s else { return }
        func pct(_ a: [Double], _ p: Double) -> Double {
            guard !a.isEmpty else { return .nan }
            let b = a.sorted()
            return b[min(b.count - 1, Int(Double(b.count - 1) * p))] * 1000
        }
        let intervals = zip(s.presented.dropFirst(), s.presented).map { $0 - $1 }
        let n = max(1, s.frames)
        print(String(format: "| %@ | %d | %.1f | %.1f / %.1f / %.1f | %.1f / %.1f (%.1f + %.1f) | %.2f / %.2f | %.2f + %.2f | %.0f | %d | %d / %d |",
                     name, s.presented.count, Double(s.presented.count) / duration,
                     pct(intervals, 0.5), pct(intervals, 0.95), pct(intervals, 1),
                     pct(s.latency, 0.5), pct(s.latency, 0.95), pct(s.inputToDraw, 0.5), pct(s.drawToPresent, 0.5),
                     pct(s.drawCPU, 0.5), pct(s.drawCPU, 0.95),
                     s.chunkGPU * 1000 / Double(n), s.colorizeGPU * 1000 / Double(n),
                     100 * Double(s.previewFrames) / Double(n), s.skipped, s.drawCPU.count, s.frames))
        fflush(stdout)
    }
}
#endif
