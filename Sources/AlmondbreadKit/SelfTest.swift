import AlmondbreadCore
import Foundation
import SwiftUI
#if os(macOS)
import AppKit
#endif

/// Launch check for builds (`make selftest`): with `--selftest` or `ALMONDBREAD_SELFTEST=1`
/// the app renders its first view through the real view and the Rust renderer, and once the
/// refined frame is presented and the GPU finished it, checks it against a fresh render of
/// the same view, writes an offscreen PNG of the view (`--selftest-png=path`, to look at),
/// prints "renderer OK ..." and quits. Exit status 1 if the renderer could not be created or
/// a check failed, 2 if no frame completed within 30 s.
public enum SelfTest {
    public static let enabled = CommandLine.arguments.contains("--selftest")
        || ProcessInfo.processInfo.environment["ALMONDBREAD_SELFTEST"] == "1"

    /// `--selftest-png=path`. One argument: AppKit would take a separate path argument for
    /// a file to open, and SwiftUI then opens no window.
    /// On iOS a relative path goes to the app's Documents directory (the simulator's data
    /// container: `xcrun simctl get_app_container <device> ch.ractive.almondbread data`).
    static var pngPath: String? {
        let prefix = "--selftest-png="
        guard let p = CommandLine.arguments.first(where: { $0.hasPrefix(prefix) }).map({ String($0.dropFirst(prefix.count)) })
        else { return nil }
        #if os(iOS)
        if !p.hasPrefix("/"), let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first {
            return docs.appendingPathComponent(p).path
        }
        #endif
        return p
    }

    @MainActor
    static func start() {
        guard enabled else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 30) {
            finish(ok: false, "renderer FAILED: no completed frame within 30 s", status: 2)
        }
    }

    @MainActor
    static func report(explorer: Explorer, stats: ExplorerStats, viewWidth: Double) {
        let failed = explorer.finishFrames()
        var lines = [String(format: "renderer %@ (Rust) %dx%d drawable, iterate GPU %.1f ms in %d chunks, %d failed frames",
                            failed == 0 ? "OK" : "FAILED", stats.renderWidth, stats.renderHeight,
                            stats.gpuTime * 1000, stats.chunks, failed)]
        var ok = failed == 0
        if let m = explorer.displayedFrameMismatch() {
            ok = ok && m.percent <= 0.05
            lines.append(String(format: "selftest: shown %dx%d frame vs fresh render: %d pixels (%.3f%%) differ in class",
                                m.width, m.height, m.pixels, m.percent))
        }
        // The places thumbnails ship as package resources in both the swift-run and the
        // Xcode-built app (kb: synthesis/places).
        let catalogue = places()
        let missing = catalogue.filter {
            PlaceThumbnails.url(id: $0.id) == nil || PlaceThumbnails.url(id: $0.id, retina: false) == nil
        }
        ok = ok && !catalogue.isEmpty && missing.isEmpty
        lines.append("selftest: places \(catalogue.count), thumbnails missing: "
                     + (missing.isEmpty ? "none" : missing.map(\.id).joined(separator: ", ")))
        if let path = pngPath {
            do {
                let w = UInt32(max(16, viewWidth.rounded()))
                let h = UInt32(max(16, (Double(w) * Double(stats.renderHeight) / Double(max(1, stats.renderWidth))).rounded()))
                let r = try explorer.renderPng(path: path, width: w, height: h)
                lines.append(String(format: "selftest: offscreen %dx%d PNG %@ (iterate GPU %.1f ms)", r.width, r.height,
                                    path, r.iterateGpuSeconds * 1000))
            } catch {
                ok = false
                lines.append("selftest: offscreen PNG FAILED: \(error)")
            }
        }
        // What the device's memory budget sees (kb: platform/ios-device "Memory").
        lines.append("selftest: " + ProcessMemory.describe(explorer))
        if !ok { lines[0] = lines[0].replacingOccurrences(of: "renderer OK", with: "renderer FAILED") }
        finish(ok: ok, lines.joined(separator: "\n"))
    }

    @MainActor
    static func finish(ok: Bool, _ message: String, status: Int32 = 1) {
        print(message)
        fflush(stdout)
        exit(ok ? 0 : status)
    }
}

/// `--ui-demo[=name]`: a background window (TestLaunch: never key, never in front) showing the
/// control center and the overlays at a fixed view, for screenshots of the window by its
/// number (`screencapture -l <n>` captures it behind other windows). Prints
/// "ui-demo: window <n>" once the view settled, then quits after 20 s. Scenes: `orbit`
/// (default: orbit and 3 map levels in the seahorse valley, internal-angle interior),
/// `orbit-neck` / `orbit-slow` (the orbit's dot cloud for slow escapes by the -3/4 neck),
/// `interior` (the period-3 bulb, bubbles), `deep` (the needle-1e94 minibrot, 4 map levels),
/// `developer` (the default scene with the HUD's developer details), `sidebar` / `sidebar-2`
/// (the default scene with View and Look, or Lighting, Overlays and Quality open),
/// `progress` / `progress-late` (render progress mid-render at a 2M limit), `journey-stop`
/// (journey #4711 in the "stop at each find" mode, captured at its first stop), `cruise-reverse`
/// (the cockpit in reverse cruise from 1e-20, 8 s in; `autopilot` shows it in A);
/// `perf` runs the scripted control-center measurement instead (UIPerf).
public enum UIDemo {
    /// `--ui-demo-out=dir`: where the screenshots go (default build/screens).
    static var outDir: String {
        let prefix = "--ui-demo-out="
        return CommandLine.arguments.first { $0.hasPrefix(prefix) }.map { String($0.dropFirst(prefix.count)) }
            ?? "build/screens"
    }

    #if os(macOS)
    /// The window as drawn, without the screen: AppKit draws the SwiftUI content and chrome
    /// (`cacheDisplay`), Rust renders the Metal canvas with its overlays (`capturePng`), and the
    /// canvas goes under the content where the fractal view is. Needs no screen-recording
    /// permission and works for a window behind all others.
    @MainActor
    static func capture(window: NSWindow, explorer: Explorer, scene: String) {
        try? FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)
        // The window server's image of our own window (no permission needed for one's own
        // windows; it has the composited Metal layer and the sidebar's materials). Fallback below.
        if let cg = CGWindowListCreateImage(.null, .optionIncludingWindow, CGWindowID(window.windowNumber),
                                            [.boundsIgnoreFraming, .bestResolution]),
           cg.width > 16 {
            let rep = NSBitmapImageRep(cgImage: cg)
            try? rep.representation(using: .png, properties: [:])?
                .write(to: URL(fileURLWithPath: "\(outDir)/window-\(scene).png"))
        }
        let canvasPath = "\(outDir)/canvas-\(scene).png"
        guard explorer.capturePng(path: canvasPath),
              let canvas = NSImage(contentsOfFile: canvasPath),
              let root = window.contentView?.superview ?? window.contentView,
              let fractal = findFractalView(root),
              let rep = root.bitmapImageRepForCachingDisplay(in: root.bounds) else {
            print("ui-demo: capture failed")
            return
        }
        root.cacheDisplay(in: root.bounds, to: rep)
        let size = root.bounds.size
        let scale = CGFloat(rep.pixelsWide) / size.width
        guard let out = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: rep.pixelsWide, pixelsHigh: rep.pixelsHigh,
                                         bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                         colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
              let ctx = NSGraphicsContext(bitmapImageRep: out) else { return }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = ctx
        ctx.cgContext.scaleBy(x: scale, y: scale)
        let fr = fractal.convert(fractal.bounds, to: root)
        canvas.draw(in: fr)
        rep.draw(in: NSRect(origin: .zero, size: size), from: .zero, operation: .sourceOver, fraction: 1,
                 respectFlipped: true, hints: nil)
        NSGraphicsContext.restoreGraphicsState()
        try? out.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(outDir)/control-center-\(scene).png"))
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(outDir)/chrome-\(scene).png"))
    }

    /// `--ui-demo=help-window`: the help window, kept behind all others.
    @MainActor static var helpWindow: NSWindow?

    /// The window as the window server has it plus its popovers (child windows of their own),
    /// composited in place: `help-<scene>.png`.
    @MainActor
    static func captureWithPopovers(window: NSWindow, scene: String) {
        func image(_ w: NSWindow) -> CGImage? {
            CGWindowListCreateImage(.null, .optionIncludingWindow, CGWindowID(w.windowNumber),
                                    [.boundsIgnoreFraming, .bestResolution])
        }
        guard let base = image(window) else { print("ui-demo: popover capture failed"); return }
        let popovers = NSApplication.shared.windows.filter {
            $0 !== window && $0.isVisible && String(describing: type(of: $0)).contains("Popover")
        }
        var union = window.frame
        for p in popovers { union = union.union(p.frame) }
        let scale = CGFloat(base.width) / window.frame.width
        guard let ctx = CGContext(data: nil, width: Int(union.width * scale), height: Int(union.height * scale),
                                  bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return }
        func draw(_ img: CGImage, _ frame: NSRect) {
            ctx.draw(img, in: CGRect(x: (frame.minX - union.minX) * scale, y: (frame.minY - union.minY) * scale,
                                     width: frame.width * scale, height: frame.height * scale))
        }
        draw(base, window.frame)
        for p in popovers { if let img = image(p) { draw(img, p.frame) } }
        guard let out = ctx.makeImage() else { return }
        let rep = NSBitmapImageRep(cgImage: out)
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(outDir)/help-\(scene).png"))
        print("ui-demo: help capture \(scene): \(popovers.count) popover(s)")
    }

    @MainActor
    private static func findFractalView(_ v: NSView) -> NSView? {
        if v is FractalNSView { return v }
        for s in v.subviews { if let f = findFractalView(s) { return f } }
        return nil
    }
    #endif

    public static var scene: String? {
        let prefix = "--ui-demo"
        guard let a = CommandLine.arguments.first(where: { $0.hasPrefix(prefix) }) else { return nil }
        return a.hasPrefix(prefix + "=") ? String(a.dropFirst(prefix.count + 1)) : "orbit"
    }
    public static var enabled: Bool { scene != nil }
    /// `tour-<id>` scenes show the canvas and the tour card without the control center.
    static var hidesSidebar: Bool { scene?.hasPrefix("tour-") == true }

    /// The control center's section state in a demo: a volatile suite, emptied at launch (all
    /// sections open; the perf scene starts with Places and a few others collapsed), so a
    /// scene neither depends on nor changes the user's sidebar.
    static let defaults: UserDefaults = {
        let name = "almondbread.ui-demo"
        let d = UserDefaults(suiteName: name) ?? .standard
        d.removePersistentDomain(forName: name)
        if scene == "perf" {
            d.set("controls,interior,journey,lighting,overlays,places", forKey: "controlCenter.collapsed")
        } else if scene == "sidebar" || scene == "sidebar-2" {
            // The settings rows: View, Look (or, scrolled past it, Overlays and Quality) open.
            d.set(scene == "sidebar" ? "controls,interior,journey,lighting,places,overlays,quality"
                  : "controls,interior,journey,look,places,view", forKey: "controlCenter.collapsed")
        }
        return d
    }()

    /// The control center's search text the scene starts with (shows that section on top).
    static var search: String? {
        switch scene {
        case "palettes": "palette"
        case "autopilot": "autopilot"
        case "pi", "pi-flight": "π"
        case "cosmic", "cosmic-deep": "cosmic"
        case "help-control": "iterations"
        case "journey-stop": "journey"
        case "tours": "tour"
        default: nil
        }
    }

    @MainActor
    static func configure(_ model: ExplorerModel, explorer: Explorer) {
        guard let scene else { return }
        model.applyLook("aurora")
        model.supersampling = .x9
        // `developer`: the default scene with the HUD's developer details (not stored).
        if scene == "developer" { model.hudDetails = true }
        // `tour-<id>`: every step of a guided tour captured in turn (kb: decisions/controls "Guided
        // tours"); `--ui-demo-tour-wait=3:14,7:12` sets the seconds before a step's capture.
        if scene.hasPrefix("tour-") {
            #if os(macOS)
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { runTour(model, explorer: explorer, id: String(scene.dropFirst(5))) }
            #endif
            return
        }
        switch scene {
        case "interior":
            try? explorer.setViewport(re: "-0.1225", im: "0.7445", scale: 0.0004)
            model.interiorColoring = .bubbles
            model.orbitEnabled = true
            model.orbitLength = 60
            model.mapEnabled = false
            _ = explorer.input(event: .pointer(x: 40, y: 30))
        case "deep":
            try? explorer.setViewportText(
                re: "-1.99999999999999999999999999999999999999999999898704139472119329940998074630723121589755867176089901",
                im: "0", scale: "1.25e-94")
            model.interiorColoring = .period
            model.orbitEnabled = true
            model.orbitLength = 300
            model.mapEnabled = true
            model.mapLevels = 4
            _ = explorer.input(event: .pointer(x: 10, y: 4))
        case "orbit-neck", "orbit-slow":
            // Slow escapes by the -3/4 neck at 1373× (the orbit-only whole-set map with its dot
            // cloud): the user's example point (escapes after a few hundred steps) and one that
            // takes ~31 000.
            let (re, im) = scene == "orbit-neck" ? ("-0.7486543886670294", "0.04900223068004966")
                : ("-0.75", "0.0001")
            try? explorer.setViewport(re: re, im: im, scale: 3.75e-3 / 1373)
            model.interiorColoring = .angle
            model.orbitEnabled = true
            model.orbitLength = 120
            model.mapEnabled = false
            _ = explorer.input(event: .pointer(x: 0, y: 0))
        case "places":
            // (After the first layout: a place is fitted to the view size.)
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { model.goToPlace("elephants") }
        case "palettes":
            model.applyPalette(model.library.first { $0.category == "natural" }?.id ?? "aurora")
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1.25e-6)
        case "pi":
            // The π meter at the cusp (kb: techniques/pi-in-the-mandelbrot-set).
            model.piMeterEnabled = true
            try? explorer.setViewportText(re: "0.2500001", im: "0", scale: "3e-10")
        case "pi-flight", "pi-flight-late", "pi-cusp", "pi-cusp-late":
            // The π hunt (guided flight) at the neck or the cusp, captured on the way: 8 s in
            // (the probe sliding) or 16 s ("-late": the deepest counts, the trail crawling).
            let point = scene.hasPrefix("pi-cusp") ? "cusp" : "neck"
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { model.piFlight(point) }
        case "help-canvas", "help-cockpit":
            // The cosmic scale's help popover (kb: decisions/controls "Help"): the scale above
            // the mini-map, then a press on its "?" badge (below, once the view settled).
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1.25e-9)
            model.mapEnabled = true
            model.mapLevels = 2
            model.cosmicScale = true
            model.piMeterEnabled = true
        case "help-window":
            // The help window's page "What am I seeing?" in a window of its own, behind all others.
            #if os(macOS)
            HelpCenter.shared.page = .seeing
            let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 860, height: 660),
                             styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
                             backing: .buffered, defer: false)
            w.title = "Almondbread Help"
            w.isReleasedWhenClosed = false
            w.contentView = NSHostingView(rootView: HelpView().preferredColorScheme(.dark))
            w.orderBack(nil)
            helpWindow = w
            #endif
        case "cosmic":
            // The size scale stacked above the mini-map (both bottom left).
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1.25e-9)
            model.mapEnabled = true
            model.mapLevels = 2
            model.cosmicScale = true
        case "cosmic-deep":
            try? explorer.setViewportText(
                re: "-1.99999999999999999999999999999999999999999999898704139472119329940998074630723121589755867176089901",
                im: "0", scale: "1.25e-94")
            model.cosmicScale = true
        case "compass":
            // A turned view: the compass at the top right (kb: decisions/controls "Compass").
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1.25e-6)
            explorer.setAngle(degrees: 35)
            model.mapEnabled = true
            model.mapLevels = 2
        case "compass-maps":
            // Maps in the top-right corner too: the compass moves left of them.
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1.25e-6)
            explorer.setAngle(degrees: -120)
            model.mapEnabled = true
            model.mapLevels = 2
            model.mapPlacement = .topRight
        case "progress", "progress-late":
            // Render progress (kb: decisions/controls "Render progress"): a high manual limit
            // at the -3/4 neck, captured mid-render (`-late`: once the slowest pixels decide).
            model.autoIterations = false
            model.maxIterationsCap = 2_000_000
            model.manualIterations = 2_000_000
            try? explorer.setViewport(re: "-0.74999996120361811", im: "0.00036918279150454967", scale: 1e-11)
        case "journey-stop":
            // (After the first layout: the journey starts from the fitted home view.)
            model.journeyStops = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { model.startJourney(seed: 4711) }
        case "cruise-reverse":
            // The HUD cockpit in reverse cruise (kb: decisions/controls "Driving levels"): Q ten
            // times (gear 10, ×5/s out, "reverse 10 · boost") from 1e-20, captured 8 s in, lane
            // assist on.
            try? explorer.setViewport(re: "-0.743643887037158704752191506114774", im: "0.131825904205311970493132056385139",
                                      scale: 1e-20)
            model.laneAssist = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
                for _ in 0..<10 { model.pedalOut() }
            }
        case "autopilot":
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1e-4)
            model.mapEnabled = true
            model.mapLevels = 2
            model.toggleAutopilot()
        case "autopilot-wish":
            // The target marker (kb: decisions/controls "Target marker"): the autopilot from the
            // seahorse valley with a synthetic pointer held to the right (a direction wish; the
            // real mouse is never moved), captured 8 s in.
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1e-4)
            model.targetMarker = true
            model.toggleAutopilot()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                _ = explorer.input(event: .pointer(x: 0, y: 0))
                _ = explorer.input(event: .pointer(x: 150, y: 10))
                _ = explorer.input(event: .pointer(x: 300, y: 20))
            }
        default:
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1.25e-6)
            model.interiorColoring = .angle
            model.orbitEnabled = true
            model.orbitLength = 120
            model.mapEnabled = true
            model.mapLevels = 3
            _ = explorer.input(event: .pointer(x: 150, y: -80))
        }
        if UIPerf.enabled {
            UIPerf.run(model)
            return
        }
        // Report the window once the view settled and the maps had time to render.
        let start = Date()
        var helpShownAt: Date?
        Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { timer in
            MainActor.assumeIsolated {
                model.refreshStats()
                #if os(macOS)
                let moving = scene.hasPrefix("autopilot") || scene == "cruise-reverse" || scene.hasPrefix("pi-flight") || scene.hasPrefix("pi-cusp")
                // journey-stop: captured once the journey rests at its first landmark.
                let stopReady = scene != "journey-stop" || model.journeyAtStop
                let movingAt: Double = scene.hasSuffix("-late") ? 17 : scene.hasPrefix("pi-") ? 9 : 8
                let t = Date().timeIntervalSince(start)
                // Progress scenes: mid-render, with the ETA shown (`-late`: the step count too).
                let progress = model.stats?.progress
                let progressReady = scene.hasPrefix("progress")
                    && (progress?.etaSeconds ?? -1) > 0
                    && (!scene.hasSuffix("-late") || (progress?.stepsReached ?? 0) > 0)
                    && t > 6
                if progressReady || !scene.hasPrefix("progress") && t > (moving ? movingAt : 8) && stopReady,
                   progressReady || model.stats?.settled == true || moving,
                   let w = NSApplication.shared.windows.first(where: { $0.isVisible && $0.contentView != nil && $0 !== helpWindow
                       && !String(describing: type(of: $0)).contains("Popover") }) {
                    // help-canvas: press the cosmic scale's badge, capture once its popover is up.
                    // help-cockpit: the cockpit's badge, pressed through the real hit test.
                    if scene == "help-cockpit" {
                        if helpShownAt == nil, let root = w.contentView, let fractal = findFractalView(root) {
                            let badges = explorer.helpBadges()
                            print("ui-demo: badges " + badges.map { "\($0.topic) \(Int($0.x)),\(Int($0.y))" }.joined(separator: " · "))
                            if let b = badges.first(where: { $0.topic == "cockpit" }) {
                                let hit = model.helpPress(x: b.x, y: b.y, size: fractal.bounds.size)
                                print("ui-demo: cockpit badge press hit=\(hit) popover=\(model.canvasHelp?.topic.rawValue ?? "none")")
                            }
                            helpShownAt = Date()
                        }
                        if Date().timeIntervalSince(helpShownAt ?? Date()) < 2 { return }
                    }
                    if scene == "help-canvas" {
                        if helpShownAt == nil,
                           let b = explorer.helpBadges().first(where: { $0.topic == "cosmic-scale" }),
                           let root = w.contentView, let fractal = findFractalView(root) {
                            model.showCanvasHelp(.cosmicScale, x: b.x, y: b.y, radius: b.radius, size: fractal.bounds.size)
                            helpShownAt = Date()
                            print("ui-demo: badges " + explorer.helpBadges().map { "\($0.topic) \(Int($0.x)),\(Int($0.y))" }.joined(separator: " · "))
                        }
                        if Date().timeIntervalSince(helpShownAt ?? Date()) < 2 { return }
                    }
                    timer.invalidate()
                    capture(window: w, explorer: explorer, scene: scene)
                    if scene.hasPrefix("help") { captureWithPopovers(window: w, scene: scene) }
                    if let h = helpWindow { captureWithPopovers(window: h, scene: scene + "-help") }
                    // The View menu as built (AppKit adds "Enter Full Screen", ⌃⌘F).
                    if let view = NSApplication.shared.mainMenu?.items.first(where: { $0.title == "View" })?.submenu {
                        view.update()
                        let items = view.items.filter { !$0.isSeparatorItem }.map { item in
                            let mods = item.keyEquivalentModifierMask
                            let key = item.keyEquivalent.isEmpty ? "" : " [" + (mods.contains(.control) ? "⌃" : "")
                                + (mods.contains(.option) ? "⌥" : "") + (mods.contains(.shift) ? "⇧" : "")
                                + (mods.contains(.command) ? "⌘" : "") + item.keyEquivalent.uppercased() + "]"
                            return item.title + key
                        }
                        print("ui-demo: view menu: " + items.joined(separator: " | "))
                    }
                    print("ui-demo: window \(w.windowNumber) orbit: \(model.orbit?.text ?? "")")
                    fflush(stdout)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 20) { exit(0) }
                }
                #endif
            }
        }
    }
}
