import XCTest

/// Heavy tour steps on the simulators: how long they take to settle and whether the app's main
/// thread stays responsive meanwhile (kb: techniques/hang-prevention). Each accessibility query
/// is answered on the app's main thread, so its round trip bounds a main-thread stall.
final class TourPerformanceUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        app = XCUIApplication()
        app.launchEnvironment["ALMONDBREAD_UITEST"] = "1"
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }
    private var next: XCUIElement { app.buttons["Next ›"] }

    private func shot(_ name: String) {
        let png = XCUIScreen.main.screenshot().pngRepresentation
        let a = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
        a.name = name
        a.lifetime = .keepAlways
        add(a)
        let device = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        if let dir = ProcessInfo.processInfo.environment["ALMONDBREAD_UITEST_SHOTS"] {
            try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
            try? png.write(to: URL(fileURLWithPath: "\(dir)/\(device)-\(name).png"))
        }
    }

    /// The hang watchdog's longest main-thread stall so far (" mainlag:x" in the value).
    private func mainLag() -> Double {
        let v = canvas.value as? String ?? ""
        return v.split(separator: " ").first { $0.hasPrefix("mainlag:") }.flatMap { Double($0.dropFirst(8)) } ?? .nan
    }

    /// The minibrot-periods tour's deepest step: its time to settle, the slowest main-thread
    /// answer while it renders, and a screenshot once it settled.
    func testPeriodStepStaysResponsive() {
        let title = "Period 2688"
        // The default anti-aliasing (off), as a user has it; the demo scenes otherwise use x9.
        app.launchArguments = ["--ui-demo=tour-minibrot-periods", "--ui-demo-aa=off"]
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        XCTAssertTrue(next.waitForExistence(timeout: 15), "the tour did not start")
        Thread.sleep(forTimeInterval: 2)
        let before = "Deeper, longer"
        for _ in 0..<12 where !app.staticTexts[before].exists {
            next.tap()
            Thread.sleep(forTimeInterval: 1.2)
        }
        XCTAssertTrue(app.staticTexts[before].exists, "no step \(before)")
        // Let the previous step settle first, so the timing below is the deep step's own.
        Thread.sleep(forTimeInterval: 15)
        let lagBefore = mainLag()
        next.tap()
        let start = Date()
        var slowest = 0.0
        var settledAt: Double?
        var queries = 0
        while Date().timeIntervalSince(start) < 300 {
            let q0 = Date()
            let value = canvas.value as? String ?? ""
            let dt = Date().timeIntervalSince(q0)
            queries += 1
            slowest = max(slowest, dt)
            let t = Date().timeIntervalSince(start)
            // (The first second or two still shows the previous, settled step.)
            if t > 3, value.contains("settled yes"), app.staticTexts[title].exists {
                settledAt = t
                break
            }
            Thread.sleep(forTimeInterval: 0.25)
        }
        let lag = mainLag()
        let summary = String(format: "period-2688: settled after %@ s, %d queries, slowest query %.2f s, longest main-thread stall %.2f s (before the step %.2f s)",
                             settledAt.map { String(format: "%.1f", $0) } ?? "never (300)", queries, slowest, lag, lagBefore)
        print(summary)
        if let dir = ProcessInfo.processInfo.environment["ALMONDBREAD_UITEST_SHOTS"] {
            let device = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
            try? (summary + "\n").write(toFile: "\(dir)/\(device)-period-2688.txt", atomically: true, encoding: .utf8)
        }
        let note = XCTAttachment(string: summary)
        note.name = "period-2688-timing"
        note.lifetime = .keepAlways
        add(note)
        // The simulator's GPU (translated to the Mac's, at a low priority) may not finish the
        // step at all within the window; that is reported, not failed, and the screenshot only
        // taken once it settled (a saturated GPU starves the simulator's compositor and the
        // request times out). A stall of the main thread is a failure (it blocked for 25 s in
        // `commandBuffer()` before the queue guard).
        if settledAt != nil { shot("period-2688") }
        XCTAssertLessThan(lag, 2.0, "the main thread stalled: \(summary)")
        app.terminate()
    }
}
