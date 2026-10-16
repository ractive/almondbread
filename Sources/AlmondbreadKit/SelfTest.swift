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
    /// container: `xcrun simctl get_app_container <device> ch.ractive.almondbread.ios data`).
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
/// `interior` (the period-3 bulb, bubbles), `deep` (the needle-1e94 minibrot, 4 map levels).
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

    /// The control center's search text the scene starts with (shows that section on top).
    static var search: String? {
        switch scene {
        case "palettes": "palette"
        case "autopilot": "autopilot"
        case "pi", "pi-flight": "π"
        case "cosmic", "cosmic-deep", "cosmic-relative": "cosmic"
        default: nil
        }
    }

    @MainActor
    static func configure(_ model: ExplorerModel, explorer: Explorer) {
        guard let scene else { return }
        model.applyLook("aurora")
        model.supersampling = .x9
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
        case "pi-flight":
            // The guided flight into the neck, captured on the way (after the first layout).
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { model.piFlight("neck") }
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
            model.cosmicAnchor = "earth"
        case "cosmic-relative":
            // Relative mode (the zoom range as the cosmos) at 1e100.
            try? explorer.setViewportText(re: "0", im: "1", scale: "1e-103")
            model.cosmicScale = true
            model.cosmicAnchor = "cosmos"
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
        case "autopilot":
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1e-4)
            model.mapEnabled = true
            model.mapLevels = 2
            model.toggleAutopilot()
        default:
            try? explorer.setViewport(re: "-0.743643887037151", im: "0.13182590420533", scale: 1.25e-6)
            model.interiorColoring = .angle
            model.orbitEnabled = true
            model.orbitLength = 120
            model.mapEnabled = true
            model.mapLevels = 3
            _ = explorer.input(event: .pointer(x: 150, y: -80))
        }
        // Report the window once the view settled and the maps had time to render.
        let start = Date()
        Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { timer in
            MainActor.assumeIsolated {
                model.refreshStats()
                #if os(macOS)
                let moving = scene == "autopilot" || scene == "pi-flight"
                if Date().timeIntervalSince(start) > 8, model.stats?.settled == true || (moving && Date().timeIntervalSince(start) > 6),
                   let w = NSApplication.shared.windows.first(where: { $0.isVisible && $0.contentView != nil }) {
                    timer.invalidate()
                    capture(window: w, explorer: explorer, scene: scene)
                    print("ui-demo: window \(w.windowNumber) orbit: \(model.orbit?.text ?? "")")
                    fflush(stdout)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 20) { exit(0) }
                }
                #endif
            }
        }
    }
}
