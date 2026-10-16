import Foundation
import Darwin
#if canImport(MetricKit)
import MetricKit
#endif

/// Main-thread hang detector (kb: techniques/hang-prevention). A background thread posts a
/// heartbeat block to the main queue every 0.5 s; when one has gone unanswered for more than
/// 2 s it writes one report per hang to `~/Library/Logs/Almondbread/hang-<time>.txt` (the
/// sandbox container's Library/Logs on iOS): the main thread's stack, sampled from the
/// watchdog thread with public Mach calls (suspend, read registers, walk the frame-pointer
/// chain with `vm_read_overwrite`, resume; nothing allocates while the main thread is
/// stopped), and the last UI events and settings changes from a small ring buffer. When the
/// main thread answers again the report gets a line with the hang's duration.
///
/// Cost: one tiny main-queue block per 0.5 s and a lock per recorded event. Off during window
/// tests unless `--hang-test` or `ALMONDBREAD_HANG_WATCHDOG=1`; `ALMONDBREAD_HANG_WATCHDOG=0`
/// turns it off. `--hang-dir=path` puts the reports elsewhere. It never blocks: the thread
/// sleeps, posts, and does its file work itself; every failure is ignored.
public enum HangWatchdog {
    static let interval = 0.5
    static let threshold = 2.0

    @MainActor static var wanted: Bool {
        let env = ProcessInfo.processInfo.environment["ALMONDBREAD_HANG_WATCHDOG"]
        if env == "0" { return false }
        if env == "1" || HangTest.enabled { return true }
        return !TestLaunch.enabled
    }

    /// Where reports go.
    static var directory: URL {
        let prefix = "--hang-dir="
        if let p = CommandLine.arguments.first(where: { $0.hasPrefix(prefix) }) {
            return URL(fileURLWithPath: String(p.dropFirst(prefix.count)), isDirectory: true)
        }
        let library = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
        return library.appendingPathComponent("Logs/Almondbread", isDirectory: true)
    }

    private static let state = WatchdogState()

    /// Starts the watchdog once (call on the main thread at launch).
    @MainActor public static func start() {
        guard wanted, state.claimStart() else { return }
        // This thread's port (the main thread's; one send right, kept for the process's life).
        let main = mach_thread_self()
        let t = Thread { run(main: main) }
        t.name = "almondbread.hang-watchdog"
        t.qualityOfService = .utility
        t.stackSize = 1 << 18
        t.start()
        #if canImport(MetricKit) && !targetEnvironment(simulator)
        MetricKitHangs.subscribe()
        #endif
    }

    /// Records a UI event or model action for the next report (cheap; a no-op while off).
    public static func note(_ what: StaticString, _ detail: @autoclosure () -> String = "") {
        guard state.running else { return }
        state.record(what, detail())
    }

    /// Brackets one `Explorer.frame` call: a hang inside it says so in the report.
    static func frame(begin: Bool) {
        guard state.running else { return }
        state.setInFrame(begin)
    }

    /// The path of the last report written (the hang test prints it).
    static var lastReport: URL? { state.lastReport }

    // MARK: The thread

    private static func run(main: mach_port_t) {
        let frames = UnsafeMutablePointer<UInt>.allocate(capacity: 128)
        while true {
            Thread.sleep(forTimeInterval: interval)
            let now = ProcessInfo.processInfo.systemUptime
            switch state.tick(now: now) {
            case .send:
                DispatchQueue.main.async { state.pong(now: ProcessInfo.processInfo.systemUptime) }
            case .waiting:
                break
            case .hung(let since):
                let n = sampleStack(of: main, into: frames, capacity: 128)
                let stack = (0..<n).map { symbolicate(frames[$0], index: $0) }
                writeReport(waited: now - since, stack: stack)
            case .recovered(let duration):
                appendRecovery(duration: duration)
            }
        }
    }

    // MARK: Stack sampling

    /// The main thread's return addresses, innermost first (pc, lr, then the frame chain).
    /// No allocation and no locks between suspend and resume: the stopped thread may hold
    /// malloc's or the runtime's.
    private static func sampleStack(of thread: mach_port_t, into buf: UnsafeMutablePointer<UInt>, capacity: Int) -> Int {
        #if arch(arm64)
        guard thread_suspend(thread) == KERN_SUCCESS else { return 0 }
        var state = arm_thread_state64_t()
        var count = mach_msg_type_number_t(MemoryLayout<arm_thread_state64_t>.stride / MemoryLayout<natural_t>.stride)
        let kr = withUnsafeMutablePointer(to: &state) {
            $0.withMemoryRebound(to: natural_t.self, capacity: Int(count)) {
                thread_get_state(thread, thread_state_flavor_t(ARM_THREAD_STATE64), $0, &count)
            }
        }
        var n = 0
        if kr == KERN_SUCCESS {
            buf[0] = UInt(state.__pc)
            buf[1] = UInt(state.__lr)
            n = 2
            var fp = UInt(state.__fp)
            // At most `capacity` frames; the chain must move up the stack and stay aligned.
            while n < capacity, fp != 0, fp & 7 == 0 {
                var frame: (UInt, UInt) = (0, 0) // saved fp, return address
                var got: vm_size_t = 0
                let r = withUnsafeMutablePointer(to: &frame) {
                    vm_read_overwrite(mach_task_self_, vm_address_t(fp), vm_size_t(16),
                                      vm_address_t(UInt(bitPattern: $0)), &got)
                }
                if r != KERN_SUCCESS || got != 16 || frame.1 == 0 { break }
                buf[n] = frame.1
                n += 1
                if frame.0 <= fp { break }
                fp = frame.0
            }
        }
        thread_resume(thread)
        return n
        #else
        return 0
        #endif
    }

    /// "3  AlmondbreadMac  0x0000000100a1b2c4  base+0xa1b2c4  symbol + 12". Pointer
    /// authentication bits are stripped (the first mask that names an image wins).
    private static func symbolicate(_ raw: UInt, index: Int) -> String {
        for mask: UInt in [0x0000_7FFF_FFFF_FFFF, 0x0000_000F_FFFF_FFFF] {
            let a = raw & mask
            var info = Dl_info()
            guard let p = UnsafeRawPointer(bitPattern: a), dladdr(p, &info) != 0, let fname = info.dli_fname else { continue }
            let image = (String(cString: fname) as NSString).lastPathComponent
            let base = UInt(bitPattern: info.dli_fbase)
            let name = image.count < 26 ? image.padding(toLength: 26, withPad: " ", startingAt: 0) : image
            var line = String(format: "%3ld  ", index) + name + String(format: " 0x%016lx  base+0x%lx", a, a &- base)
            if let s = info.dli_sname {
                line += "  \(String(cString: s)) + \(a &- UInt(bitPattern: info.dli_saddr))"
            }
            return line
        }
        return String(format: "%3ld  ?  0x%016lx", index, raw)
    }

    // MARK: Reports

    private static func writeReport(waited: Double, stack: [String]) {
        let date = Date()
        let stamp = DateFormatter()
        stamp.locale = Locale(identifier: "en_US_POSIX")
        stamp.dateFormat = "yyyyMMdd-HHmmss"
        let iso = ISO8601DateFormatter().string(from: date)
        let info = Bundle.main.infoDictionary ?? [:]
        let snap = state.snapshot(now: ProcessInfo.processInfo.systemUptime)
        var lines = [
            "Almondbread hang report",
            "time: \(iso)",
            "process: \(ProcessInfo.processInfo.processName) pid \(getpid()), version \(info["CFBundleShortVersionString"] ?? "dev") (\(info["CFBundleVersion"] ?? "-"))",
            "os: \(ProcessInfo.processInfo.operatingSystemVersionString)",
            String(format: "main thread: no answer to a heartbeat for %.2f s (threshold %.1f s)", waited, threshold),
            snap.frame,
            "",
            "recent events (seconds before the report, oldest first):",
        ]
        lines += snap.events.isEmpty ? ["  (none)"] : snap.events.map { "  " + $0 }
        lines += ["", "main thread stack (sampled from the watchdog thread; symbolicate unnamed frames with `atos -o <binary> -l <base> <address>`):"]
        lines += stack.isEmpty ? ["  (not available)"] : stack
        let text = lines.joined(separator: "\n") + "\n"
        let dir = directory
        let url = dir.appendingPathComponent("hang-\(stamp.string(from: date)).txt")
        do {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            try text.write(to: url, atomically: true, encoding: .utf8)
            state.setLastReport(url)
            FileHandle.standardError.write(Data("almondbread: main thread hung for \(String(format: "%.1f", waited)) s; report: \(url.path)\n".utf8))
        } catch {
            FileHandle.standardError.write(Data("almondbread: main thread hung; report not written: \(error)\n".utf8))
        }
    }

    private static func appendRecovery(duration: Double) {
        let line = String(format: "\nrecovered: the main thread answered the heartbeat %.2f s after it was sent\n", duration)
        FileHandle.standardError.write(Data("almondbread: main thread recovered after \(String(format: "%.2f", duration)) s\n".utf8))
        guard let url = state.lastReport, let h = try? FileHandle(forWritingTo: url) else { return }
        defer { try? h.close() }
        _ = try? h.seekToEnd()
        try? h.write(contentsOf: Data(line.utf8))
    }
}

/// The watchdog's shared state behind one lock.
private final class WatchdogState: @unchecked Sendable {
    enum Tick { case send, waiting, hung(since: Double), recovered(Double) }
    private struct Event { var at: Double; var what: StaticString; var detail: String }

    private let lock = NSLock()
    private var started = false
    private(set) var running = false
    private var pending = false
    private var sentAt = 0.0
    private var reported = false
    private var recovered: Double?
    private var events: [Event] = []
    private var next = 0
    private var frameSince: Double?
    private var lastFrameEnd: Double?
    private var report: URL?
    private static let capacity = 32

    func claimStart() -> Bool {
        lock.lock(); defer { lock.unlock() }
        if started { return false }
        started = true
        running = true
        events.reserveCapacity(Self.capacity)
        return true
    }

    func tick(now: Double) -> Tick {
        lock.lock(); defer { lock.unlock() }
        if let d = recovered {
            recovered = nil
            return .recovered(d)
        }
        if !pending {
            pending = true
            sentAt = now
            return .send
        }
        if !reported && now - sentAt > HangWatchdog.threshold {
            reported = true
            return .hung(since: sentAt)
        }
        return .waiting
    }

    func pong(now: Double) {
        lock.lock(); defer { lock.unlock() }
        pending = false
        if reported {
            reported = false
            recovered = now - sentAt
        }
    }

    func record(_ what: StaticString, _ detail: String) {
        let e = Event(at: ProcessInfo.processInfo.systemUptime, what: what, detail: detail)
        lock.lock(); defer { lock.unlock() }
        if events.count < Self.capacity {
            events.append(e)
        } else {
            events[next] = e
        }
        next = (next + 1) % Self.capacity
    }

    func setInFrame(_ begin: Bool) {
        let now = ProcessInfo.processInfo.systemUptime
        lock.lock(); defer { lock.unlock() }
        if begin {
            frameSince = now
        } else {
            frameSince = nil
            lastFrameEnd = now
        }
    }

    func snapshot(now: Double) -> (events: [String], frame: String) {
        lock.lock(); defer { lock.unlock() }
        let ordered = events.count < Self.capacity ? events : Array(events[next...] + events[..<next])
        let lines = ordered.map { e in
            String(format: "-%7.3f s  ", now - e.at) + "\(e.what)" + (e.detail.isEmpty ? "" : ": \(e.detail)")
        }
        let frame: String
        if let s = frameSince {
            frame = String(format: "renderer: inside Explorer.frame for %.2f s", now - s)
        } else if let e = lastFrameEnd {
            frame = String(format: "renderer: not in a frame call (the last one ended %.2f s ago)", now - e)
        } else {
            frame = "renderer: no frame call yet"
        }
        return (lines, frame)
    }

    var lastReport: URL? {
        lock.lock(); defer { lock.unlock() }
        return report
    }

    func setLastReport(_ url: URL) {
        lock.lock(); defer { lock.unlock() }
        report = url
    }
}

#if canImport(MetricKit) && !targetEnvironment(simulator)
/// App Store builds: the system's own hang diagnostics (`MXHangDiagnostic`, delivered at most
/// daily with symbolicatable call-stack trees), saved next to the watchdog's reports.
private final class MetricKitHangs: NSObject, MXMetricManagerSubscriber {
    nonisolated(unsafe) private static var shared: MetricKitHangs?

    static func subscribe() {
        guard shared == nil else { return }
        let s = MetricKitHangs()
        shared = s
        MXMetricManager.shared.add(s)
    }

    func didReceive(_ payloads: [MXDiagnosticPayload]) {
        for p in payloads where !(p.hangDiagnostics ?? []).isEmpty {
            let dir = HangWatchdog.directory
            let name = "hang-metrickit-\(Int(p.timeStampEnd.timeIntervalSince1970)).json"
            try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            try? p.jsonRepresentation().write(to: dir.appendingPathComponent(name))
        }
    }
}
#endif

/// `--hang-test`: a background window test of the watchdog. About 1.5 s after launch the main
/// thread is blocked for 3 s; the watchdog must write a report and then a recovery line.
/// Prints the report and quits (exit 0 when both were written).
public enum HangTest {
    public static let enabled = CommandLine.arguments.contains("--hang-test")
    @MainActor private static var started = false

    @MainActor
    static func start() {
        guard enabled, !started else { return }
        started = true
        HangWatchdog.note("hang test", "blocking the main thread for 3 s")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            block(seconds: 3.0)
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { finish() }
        }
    }

    /// Kept out of line so the report's stack names it.
    @inline(never)
    static func block(seconds: Double) {
        Thread.sleep(forTimeInterval: seconds)
    }

    @MainActor
    private static func finish() {
        guard let url = HangWatchdog.lastReport, let text = try? String(contentsOf: url, encoding: .utf8) else {
            SelfTest.finish(ok: false, "hang test FAILED: no report written")
            return
        }
        print("hang test: report \(url.path)\n\(text)")
        let ok = text.contains("recovered:")
        SelfTest.finish(ok: ok, ok ? "hang test OK" : "hang test FAILED: no recovery line")
    }
}
