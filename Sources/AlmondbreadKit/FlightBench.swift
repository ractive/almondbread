#if os(macOS)
import AlmondbreadCore
import AppKit
import QuartzCore

/// `--flight-bench=flight.json`: the flight benchmark's real-time mode in the window (mode B,
/// kb: synthesis/flight-benchmark). Rust plays the flight through the real per-refresh path
/// (`Explorer.startFlightBench`: the script's events go in as decoded input at their times)
/// and measures frames due and shown, frame times, input-to-photon latency and time to sharp;
/// this side only sizes the window to 1920x1080 pixels, waits for the report, writes it to
/// `--flight-bench-out=path` and quits. `--flight-bench-laps=n`, `--flight-bench-warmup=n`.
/// A background window like the interaction test (TestLaunch): never key, never in front.
/// Usually started by `almondbread benchmark --live-app` (`make flight-bench-live`).
@MainActor
final class FlightBench: NSObject {
    private static func argument(_ name: String) -> String? {
        CommandLine.arguments.first { $0.hasPrefix("--\(name)=") }.map { String($0.dropFirst(name.count + 3)) }
    }
    static let scriptPath = argument("flight-bench")
    static var enabled: Bool { scriptPath != nil }
    static let shared = FlightBench()

    private weak var view: FractalNSView?
    private var started: CFTimeInterval = 0

    func attach(view: FractalNSView) {
        guard self.view == nil else { return }
        self.view = view
        // Let the window appear and the first frame render.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { self.start() }
    }

    private func fail(_ message: String) -> Never {
        print("flight-bench: \(message)")
        fflush(stdout)
        exit(1)
    }

    private func start() {
        guard let view, let window = view.window, let explorer = view.explorer else { fail("no window or renderer") }
        guard let path = Self.scriptPath, let script = try? String(contentsOfFile: path, encoding: .utf8) else {
            fail("cannot read \(Self.scriptPath ?? "")")
        }
        // 1920x1080 pixels whatever the backing scale: devices compare at the same work.
        let scale = window.backingScaleFactor
        window.setContentSize(NSSize(width: 1920 / scale, height: 1080 / scale))
        TestLaunch.placeAtScreenEdge(window)
        let hz = window.screen?.maximumFramesPerSecond ?? 60
        let laps = UInt32(Self.argument("flight-bench-laps") ?? "") ?? 3
        let warmup = UInt32(Self.argument("flight-bench-warmup") ?? "") ?? 1
        print("flight-bench: window key \(window.isKeyWindow), app active \(NSApp.isActive), "
              + "occlusion visible \(window.occlusionState.contains(.visible)), backing scale \(scale), display \(hz) Hz")
        do {
            try explorer.startFlightBench(script: script, hz: Double(hz), laps: laps, warmup: warmup)
        } catch {
            fail("\(error)")
        }
        view.requestFrames()
        started = CACurrentMediaTime()
        poll()
    }

    private func poll() {
        guard let view, let explorer = view.explorer else { return }
        // Frames must keep coming even if the link paused between laps.
        view.requestFrames()
        if let report = explorer.flightBenchReport() {
            let out = Self.argument("flight-bench-out") ?? "build/benchmark/live.json"
            do {
                try FileManager.default.createDirectory(at: URL(fileURLWithPath: out).deletingLastPathComponent(),
                                                        withIntermediateDirectories: true)
                try report.write(toFile: out, atomically: true, encoding: .utf8)
            } catch {
                fail("cannot write \(out): \(error)")
            }
            let visible = view.window?.occlusionState.contains(.visible) ?? false
            print("flight-bench: done in \(Int(CACurrentMediaTime() - started)) s, window \(visible ? "visible" : "occluded"), report \(out)")
            fflush(stdout)
            exit(0)
        }
        if CACurrentMediaTime() - started > 30 * 60 {
            fail("no report within 30 minutes (\(explorer.flightBenchProgress()))")
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { self.poll() }
    }
}
#endif
