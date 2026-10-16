import Combine
import Foundation
import QuartzCore
#if os(macOS)
import AppKit
#endif

/// `--ui-demo=perf`: a scripted control-center session in a background window (TestLaunch:
/// never key, never in front) that measures SwiftUI cost: body evaluations per view type,
/// main-thread busy time per phase (idle, rendering, typing), the latency of each scripted
/// keystroke and accordion toggle, and the time until every place thumbnail is visible. Prints
/// one `ui-perf:` report and quits. kb: platform/swiftui-performance.
@MainActor
enum UIPerf {
    static let enabled = UIDemo.scene == "perf"

    /// What the script asks the control center to do.
    enum Action {
        case search(String)
        case toggle(ControlSection)
    }

    static let actions = PassthroughSubject<Action, Never>()

    // MARK: Counters

    private static var bodies: [String: Int] = [:]

    /// Counts one body evaluation of `view` (no-op outside the perf scene).
    static func body(_ view: String) {
        guard enabled else { return }
        bodies[view, default: 0] += 1
    }

    // MARK: Run-loop timing

    private static var observers: [CFRunLoopObserver] = []
    private static var iterationStart: CFTimeInterval = 0
    private static var busy: CFTimeInterval = 0
    private static var longest: CFTimeInterval = 0
    private static var phaseStart: CFTimeInterval = 0
    private static var pending: (label: String, t0: CFTimeInterval)?
    private static var latencies: [String: [Double]] = [:]

    /// Main-thread busy time = afterWaiting ... beforeWaiting (after SwiftUI's update and the
    /// Core Animation commit: the observer has the highest order).
    private static func installObservers() {
        guard observers.isEmpty else { return }
        let start = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.afterWaiting.rawValue, true, Int.min) { _, _ in
            MainActor.assumeIsolated { iterationStart = CACurrentMediaTime() }
        }
        let end = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, true, Int.max) { _, _ in
            MainActor.assumeIsolated {
                let now = CACurrentMediaTime()
                if iterationStart > 0 {
                    let d = now - iterationStart
                    busy += d
                    longest = max(longest, d)
                }
                if let p = pending {
                    latencies[p.label, default: []].append((now - p.t0) * 1000)
                    pending = nil
                }
                iterationStart = 0
            }
        }
        for o in [start, end].compactMap({ $0 }) {
            CFRunLoopAddObserver(CFRunLoopGetMain(), o, .commonModes)
            observers.append(o)
        }
    }

    private static func send(_ action: Action, label: String) {
        pending = (label, CACurrentMediaTime())
        actions.send(action)
    }

    // MARK: Thumbnails

    private static var placesShown: CFTimeInterval?
    private static var thumbsVisible: Set<String> = []
    private static var thumbsAtFirstBody = 0
    private static var thumbsAllVisible: Double?

    static func placesAppeared() {
        guard enabled, placesShown == nil else { return }
        placesShown = CACurrentMediaTime()
    }

    /// A thumbnail shows its image (`immediate`: in its first body, no placeholder ever shown).
    static func thumbnailVisible(_ id: String, immediate: Bool) {
        guard enabled, !thumbsVisible.contains(id) else { return }
        thumbsVisible.insert(id)
        if immediate { thumbsAtFirstBody += 1 }
        if let t = placesShown { thumbsAllVisible = (CACurrentMediaTime() - t) * 1000 }
    }

    // MARK: Script

    private static var report: [String] = []

    private static func beginPhase(_ name: String = "") {
        if !name.isEmpty { print("ui-perf: begin \(name)"); fflush(stdout) }
        bodies = [:]
        busy = 0
        longest = 0
        latencies = [:]
        phaseStart = CACurrentMediaTime()
    }

    private static func endPhase(_ name: String) {
        let span = CACurrentMediaTime() - phaseStart
        let counts = bodies.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" }.joined(separator: " ")
        var line = String(format: "ui-perf: %@ %.1f s: main busy %.0f ms (%.0f%%), longest %.1f ms; bodies %@",
                          name, span, busy * 1000, 100 * busy / span, longest * 1000, counts)
        for (label, v) in latencies.sorted(by: { $0.key < $1.key }) {
            let s = v.sorted()
            line += String(format: "; %@ n=%d median %.1f ms max %.1f ms", label, s.count, s[s.count / 2], s.last ?? 0)
        }
        report.append(line)
        print(line)
        fflush(stdout)
    }

    private static func wait(_ s: Double) async {
        try? await Task.sleep(nanoseconds: UInt64(s * 1e9))
    }

    private static func type(_ text: String, from start: String = "", interval: Double) async {
        var s = start
        for ch in text {
            s.append(ch)
            send(.search(s), label: "keystroke")
            await wait(interval)
        }
        // A pause: the debounced query applies (the form shows the matching sections).
        await wait(0.6)
        while !s.isEmpty {
            s.removeLast()
            send(.search(s), label: "keystroke")
            await wait(interval)
        }
    }

    /// Runs the scripted session once the view settled.
    static func run(_ model: ExplorerModel) {
        guard enabled else { return }
        installObservers()
        Task { @MainActor in
            // Settle: first render, references, maps.
            for _ in 0..<40 {
                await wait(0.25)
                model.refreshStats()
                if model.stats?.settled == true { break }
            }
            await wait(1)

            beginPhase()
            await wait(3)
            endPhase("idle")

            beginPhase()
            model.toggleAutopilot()
            await wait(3)
            endPhase("rendering")
            model.stopMotion()
            await wait(1.5)

            // First open of Places (nothing decoded at launch unless pre-decoded).
            beginPhase()
            send(.toggle(.places), label: "open-places")
            await wait(2.5)
            endPhase("open-places")
            report.append(String(format: "ui-perf: thumbnails: %d created, %d with an image at first body, last visible after %@",
                                 thumbsVisible.count, thumbsAtFirstBody,
                                 thumbsAllVisible.map { String(format: "%.0f ms", $0) } ?? "never"))
            print(report.last!)

            beginPhase()
            model.toggleAutopilot()
            await wait(3)
            endPhase("rendering-places-open")
            model.stopMotion()
            await wait(1.5)

            // Baseline: everything collapsed while rendering (the renderer's own main-thread cost).
            let open = ControlSection.allCases.filter { ![.controls, .interior, .journey, .lighting, .overlays].contains($0) }
            for s in open { send(.toggle(s), label: "collapse") }
            await wait(1)
            beginPhase()
            model.toggleAutopilot()
            await wait(3)
            endPhase("rendering-all-collapsed")
            model.stopMotion()
            await wait(1)
            // Each section alone while rendering: what one open section costs per stats tick.
            for s in open {
                send(.toggle(s), label: "expand")
                await wait(1)
                beginPhase()
                model.toggleAutopilot()
                await wait(2.5)
                endPhase("rendering-only-\(s.rawValue)")
                model.stopMotion()
                await wait(0.7)
                send(.toggle(s), label: "collapse")
                await wait(0.5)
            }
            // Each section opened and closed with every other one collapsed: its own cost.
            beginPhase()
            for s in ControlSection.allCases {
                send(.toggle(s), label: "alone-\(s.rawValue)")
                await wait(0.45)
                send(.toggle(s), label: "alone-\(s.rawValue)")
                await wait(0.45)
            }
            endPhase("accordion-alone")
            for s in open { send(.toggle(s), label: "expand") }
            await wait(1.5)

            beginPhase("accordion")
            for s in ControlSection.allCases {
                send(.toggle(s), label: "toggle-\(s.rawValue)")
                await wait(0.45)
                send(.toggle(s), label: "toggle-\(s.rawValue)")
                await wait(0.45)
            }
            endPhase("accordion")

            beginPhase()
            await type("palette", interval: 0.08)
            await type("zoom", interval: 0.08)
            endPhase("typing-idle")

            beginPhase()
            model.toggleAutopilot()
            await wait(0.5)
            await type("palette", interval: 0.08)
            await type("zoom", interval: 0.08)
            endPhase("typing-rendering")
            model.stopMotion()

            print("ui-demo: perf done")
            fflush(stdout)
            exit(0)
        }
    }
}
