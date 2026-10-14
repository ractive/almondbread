import AlmondbreadCore
import Foundation

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
    static var pngPath: String? {
        let prefix = "--selftest-png="
        return CommandLine.arguments.first { $0.hasPrefix(prefix) }.map { String($0.dropFirst(prefix.count)) }
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
