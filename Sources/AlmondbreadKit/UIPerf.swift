import Combine
import Foundation
import QuartzCore
import os
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
    static let enabled = UIDemo.scene == "perf" || openScene
    /// `--ui-demo=perf-open`: the control center starts hidden; the script opens it and scrolls
    /// through it (the beach ball of 2026-10-17).
    static let openScene = UIDemo.scene == "perf-open" || stressScene
    /// `--ui-demo=perf-stress`: many opens and scrolls at random speeds, stops and directions;
    /// a stuck main thread (> 4 s) is sampled and ends the run.
    static let stressScene = UIDemo.scene == "perf-stress"

    /// What the script asks the control center to do.
    enum Action {
        case search(String)
        case toggle(ControlSection)
        /// Show or hide the control center (perf-open).
        case inspector(Bool)
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
    /// What the stress script is doing (for its long-iteration lines).
    private static var activity = ""
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
                    if stressScene, d > 0.15 { print(String(format: "ui-perf: long iteration %.0f ms during %@", d * 1000, activity)) }
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
            line += String(format: "; %@ n=%d median %.1f ms min %.1f ms max %.1f ms", label, s.count, s[s.count / 2], s.first ?? 0, s.last ?? 0)
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
        if openScene { return runOpen(model) }
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
                send(.toggle(s), label: "alone-open-\(s.rawValue)")
                await wait(0.45)
                send(.toggle(s), label: "alone-close-\(s.rawValue)")
                await wait(0.45)
            }
            endPhase("accordion-alone")
            for s in open { send(.toggle(s), label: "expand") }
            await wait(1.5)

            // With Places, View, Look and Quality open: each section closed and opened again (or
            // opened and closed).
            beginPhase("accordion")
            for s in ControlSection.allCases {
                let first = open.contains(s) ? "close" : "open", second = open.contains(s) ? "open" : "close"
                send(.toggle(s), label: "\(first)-\(s.rawValue)")
                await wait(0.45)
                send(.toggle(s), label: "\(second)-\(s.rawValue)")
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

    // MARK: Opening and scrolling (perf-open)

    #if os(macOS)
    /// The control center's scroll view: the one with the tallest document.
    private static func sidebarScrollView() -> NSScrollView? {
        func all(_ v: NSView) -> [NSScrollView] {
            ((v as? NSScrollView).map { [$0] } ?? []) + v.subviews.flatMap(all)
        }
        let roots = NSApplication.shared.windows.filter(\.isVisible).compactMap { $0.contentView?.superview ?? $0.contentView }
        return roots.flatMap(all).max { ($0.documentView?.frame.height ?? 0) < ($1.documentView?.frame.height ?? 0) }
    }

    /// Scrolls the sidebar with pixel scroll-wheel events (`pointsPerSecond`, 60 events a
    /// second) until it stops moving for half a second; each event's latency is measured.
    private static func scroll(down: Bool, pointsPerSecond: Double = 1200, maxEvents: Int = 3000) async {
        guard let sv = sidebarScrollView() else {
            print("ui-perf: no scroll view found")
            return
        }
        let step = Int32(pointsPerSecond / 60)
        var events = 0, still = 0
        var last = sv.contentView.bounds.origin.y
        while still < 30 && events < maxEvents {
            guard let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1,
                                   wheel1: down ? -step : step, wheel2: 0, wheel3: 0),
                  let e = { () -> NSEvent? in
                      // A trackpad gesture: continuous, began / changed phases.
                      cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
                      cg.setIntegerValueField(.scrollWheelEventPointDeltaAxis1, value: Int64(down ? -step : step))
                      cg.setIntegerValueField(CGEventField(rawValue: 99)!, value: events == 0 ? 1 : 2)
                      return NSEvent(cgEvent: cg)
                  }() else { break }
            pending = ("scroll", CACurrentMediaTime())
            sv.scrollWheel(with: e)
            events += 1
            await wait(1.0 / 60)
            let y = sv.contentView.bounds.origin.y
            still = abs(y - last) < 0.5 ? still + 1 : 0
            last = y
            // At the end (an elastic scroll view keeps moving past it).
            let end = down ? y + sv.contentView.bounds.height >= (sv.documentView?.frame.height ?? 0)
                : y <= -sv.contentView.contentInsets.top
            if end { break }
        }
        if let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: 0, wheel2: 0, wheel3: 0) {
            cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
            cg.setIntegerValueField(CGEventField(rawValue: 99)!, value: 4)
            if let e = NSEvent(cgEvent: cg) { sv.scrollWheel(with: e) }
        }
        print(String(format: "ui-perf: scrolled %@ %d events to y %.0f of %.0f", down ? "down" : "up", events,
                     last, sv.documentView?.frame.height ?? 0))
    }
    #else
    private static func scroll(down: Bool, pointsPerSecond: Double = 1200, maxEvents: Int = 3000) async {}
    #endif

    /// perf-open: open the control center (the first time after launch, then again) and scroll
    /// through it, with the user's sections open (Places, Overlays, Quality), with every section
    /// open, and with only Places open.
    private static func runOpen(_ model: ExplorerModel) {
        Task { @MainActor in
            for _ in 0..<40 {
                await wait(0.25)
                model.refreshStats()
                if model.stats?.settled == true { break }
            }
            await wait(1)
            watchStalls()
            if stressScene { return await stress(model) }
            let all = ControlSection.allCases.map(\.rawValue)
            let configs: [(String, [String])] = [
                ("user", ["controls", "interior", "journey", "lighting", "look", "view"]),
                ("all", []),
                ("places", all.filter { $0 != "places" }),
                ("user-legacy", ["controls", "interior", "journey", "lighting", "look", "view"]),
                ("all-legacy", []),
            ]
            // What one width change costs (the reveal animation lays the sidebar out at 0-330 pt).
            for (name, collapsed) in [configs[0], configs[1]] {
                UIDemo.defaults.set(collapsed.joined(separator: ","), forKey: "controlCenter.collapsed")
                actions.send(.inspector(true))
                await wait(1.5)
                beginPhase()
                for w: CGFloat in [300, 270, 240, 210, 180, 157, 130, 100, 60, 30, 15, 60, 157, 250, 330] {
                    pending = ("width", CACurrentMediaTime())
                    setSidebarWidth(w)
                    await wait(0.4)
                }
                endPhase("widths-\(name)")
                setSidebarWidth(nil)
                actions.send(.inspector(false))
                await wait(1)
            }
            for (name, collapsed) in configs {
                UIDemo.defaults.set(collapsed.joined(separator: ","), forKey: "controlCenter.collapsed")
                await wait(0.5)
                for pass in 1...2 {
                    beginPhase()
                    showInspector(true)
                    await wait(1.5)
                    endPhase("open-\(name)-\(pass)")
                    #if os(macOS)
                    // Always-visible scroll bars (a mouse attached): they narrow the content.
                    if name.hasSuffix("-legacy") { sidebarScrollView()?.scrollerStyle = .legacy }
                    #endif
                    beginPhase()
                    await scroll(down: true)
                    endPhase("scroll-down-\(name)-\(pass)")
                    beginPhase()
                    await scroll(down: false)
                    endPhase("scroll-up-\(name)-\(pass)")
                    beginPhase()
                    showInspector(false)
                    await wait(1)
                    endPhase("close-\(name)-\(pass)")
                }
            }
            print("ui-demo: perf done")
            fflush(stdout)
            exit(0)
        }
    }

    /// Shows or hides the control center as the toolbar button does in a key window, where
    /// AppKit animates the reveal: the sidebar is laid out at every width up to its own over
    /// ~0.13 s (the hang report's insets). A background window does not animate, so the reveal
    /// is replayed with the split view's divider.
    private static func showInspector(_ shown: Bool) {
        pending = (shown ? "open" : "close", CACurrentMediaTime())
        #if os(macOS)
        actions.send(.inspector(shown))
        guard shown else { return }
        // The reveal: the inspector's width from 20 to 330 pt in 16 steps of 8 ms (below its
        // 290 pt minimum, as AppKit's animation lays it out), then back to its own width.
        Task { @MainActor in
            await wait(0.05)
            for k in 0...16 {
                setSidebarWidth(20 + (330 - 20) * CGFloat(k) / 16)
                await wait(0.008)
            }
            setSidebarWidth(nil)
        }
        #else
        actions.send(.inspector(shown))
        #endif
    }

    #if os(macOS)
    private static var savedMinimum: (CGFloat, Bool)?

    /// Lays the control center out at width `w` (below its 290 pt minimum too, as AppKit's
    /// reveal animation does); nil restores the minimum.
    private static func setSidebarWidth(_ w: CGFloat?) {
        func splits(_ v: NSView) -> [NSSplitView] {
            ((v as? NSSplitView).map { [$0] } ?? []) + v.subviews.flatMap(splits)
        }
        let all = NSApplication.shared.windows.compactMap(\.contentView).flatMap(splits)
        guard let split = all.first(where: { $0.isVertical && $0.arrangedSubviews.count >= 2 }),
              let controller = split.delegate as? NSSplitViewController,
              let item = controller.splitViewItems.last else { return }
        guard let w else {
            if let (m, c) = savedMinimum { item.minimumThickness = m; item.canCollapse = c }
            savedMinimum = nil
            return
        }
        if savedMinimum == nil { savedMinimum = (item.minimumThickness, item.canCollapse) }
        item.minimumThickness = 10
        item.canCollapse = false
        split.setPosition(split.bounds.width - w - split.dividerThickness, ofDividerAt: split.arrangedSubviews.count - 2)
    }
    #else
    private static func setSidebarWidth(_ w: CGFloat?) {}
    #endif

    /// A stuck main thread (no answer for 4 s): `sample` this process, then quit.
    private static func watchStalls() {
        let beat = OSAllocatedUnfairLock(initialState: CACurrentMediaTime())
        Thread.detachNewThread {
            while true {
                Thread.sleep(forTimeInterval: 0.25)
                DispatchQueue.main.async { beat.withLock { $0 = CACurrentMediaTime() } }
                let late = CACurrentMediaTime() - beat.withLock { $0 }
                if late > 4 {
                    print(String(format: "ui-perf: STUCK main thread %.1f s; sampling", late))
                    fflush(stdout)
                    #if os(macOS)
                    let p = Process()
                    p.executableURL = URL(fileURLWithPath: "/usr/bin/sample")
                    p.arguments = ["\(getpid())", "3", "-file", "build/perf-stuck-sample.txt"]
                    try? p.run()
                    p.waitUntilExit()
                    print("ui-perf: sample in build/perf-stuck-sample.txt")
                    fflush(stdout)
                    #endif
                    exit(2)
                }
            }
        }
    }

    /// perf-stress: open and close the control center and scroll it at random speeds, distances
    /// and pauses (seeded), with the user's sections or all of them, sometimes while the canvas
    /// renders; reports the slowest run-loop iteration.
    private static func stress(_ model: ExplorerModel) async {
        #if os(macOS)
        // The user's window: the whole screen (1728 × 1083 on a 14" MacBook Pro).
        if let w = NSApplication.shared.windows.first(where: { $0.isVisible && $0.contentView != nil }),
           let screen = w.screen ?? NSScreen.main {
            w.setFrame(screen.visibleFrame, display: true)
            print("ui-perf: window \(Int(w.frame.width))×\(Int(w.frame.height))")
            await wait(1.5)
        }
        #endif
        var rng = UInt64(4711)
        func random() -> Double {
            rng &+= 0x9E37_79B9_7F4A_7C15
            var z = rng
            z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
            z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
            return Double((z ^ (z >> 31)) >> 11) / Double(1 << 53)
        }
        let user = "controls,interior,journey,lighting,look,view"
        var worst = 0.0
        for round in 0..<40 {
            let config = random() < 0.6 ? "user" : "all"
            UIDemo.defaults.set(config == "user" ? user : "", forKey: "controlCenter.collapsed")
            let rendering = random() < 0.3
            if rendering { model.toggleAutopilot() }
            let context = "round \(round) (\(config)\(rendering ? ", rendering" : ""))"
            beginPhase()
            activity = "open " + context
            showInspector(true)
            await wait(random() * 0.6)
            for _ in 0..<(2 + Int(random() * 6)) {
                let down = random() < 0.7, speed = 200 + random() * 3800
                activity = String(format: "scroll %@ %.0f pt/s, %@", down ? "down" : "up", speed, context)
                await scroll(down: down, pointsPerSecond: speed, maxEvents: 5 + Int(random() * 90))
                activity = "pause " + context
                await wait(random() * 0.4)
            }
            activity = "close " + context
            showInspector(false)
            await wait(0.3)
            if rendering { model.stopMotion() }
            worst = max(worst, longest)
            endPhase("stress-\(round)")
        }
        print(String(format: "ui-perf: stress done, longest iteration %.1f ms", worst * 1000))
        print("ui-demo: perf done")
        fflush(stdout)
        exit(0)
    }
}
