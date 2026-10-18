import XCTest

/// Touch input on iPhone and iPad simulators (`make ios-uitest`): real UIKit gestures
/// synthesised by XCTest reach the fractal view's recognisers; the view's accessibility value
/// ("re im scale angle° autopilot on|off journey on|off", FractalUIView) shows what they did.
/// Simulator Metal says nothing about speed: this checks wiring and layout only. Screenshots
/// go to the test result and, with `ALMONDBREAD_UITEST_SHOTS=dir`, to that host directory.
final class TouchUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        app = XCUIApplication()
        app.launch()
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }

    private struct State {
        let re: Double, im: Double, scale: Double, angle: Double, autopilot: Bool, journey: Bool
    }

    private func state() -> State? {
        guard let v = canvas.value as? String else { return nil }
        let f = v.split(separator: " ").map(String.init)
        guard f.count >= 8 else { return nil }
        return State(re: Double(f[0]) ?? .nan, im: Double(f[1]) ?? .nan, scale: Double(f[2]) ?? .nan,
                     angle: Double(f[3].replacingOccurrences(of: "°", with: "")) ?? .nan,
                     autopilot: f[5] == "on", journey: f[7] == "on")
    }

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

    /// Lets flings and eased motion settle.
    private func settle(_ seconds: Double = 1.5) {
        Thread.sleep(forTimeInterval: seconds)
    }

    func testGesturesAndButtons() throws {
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        settle(3)
        shot("start")
        let s0 = try XCTUnwrap(state(), "no view state in the accessibility value")

        // One-finger drag: the view pans (the centre moves), the scale stays.
        let from = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let to = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.55))
        from.press(forDuration: 0.05, thenDragTo: to, withVelocity: 300, thenHoldForDuration: 0.1)
        settle()
        let s1 = try XCTUnwrap(state())
        XCTAssertGreaterThan(abs(s1.re - s0.re) / s0.scale, 20, "drag did not pan: \(s0) -> \(s1)")
        XCTAssertEqual(s1.scale, s0.scale, accuracy: s0.scale * 1e-6, "drag changed the scale")

        // Pinch out: zoom in at least 1.5x.
        canvas.pinch(withScale: 3, velocity: 2)
        settle()
        let s2 = try XCTUnwrap(state())
        XCTAssertLessThan(s2.scale, s1.scale / 1.5, "pinch did not zoom: \(s1.scale) -> \(s2.scale)")

        // Two-finger rotation: the view turns.
        canvas.rotate(.pi / 3, withVelocity: 1)
        settle()
        let s3 = try XCTUnwrap(state())
        XCTAssertGreaterThan(abs(s3.angle - s2.angle), 20, "rotation did not turn: \(s2.angle) -> \(s3.angle)")
        shot("after-gestures")

        // Double tap zooms in, two-finger tap zooms out.
        canvas.doubleTap()
        settle()
        let s4 = try XCTUnwrap(state())
        XCTAssertLessThan(s4.scale, s3.scale / 1.2, "double tap did not zoom in")
        canvas.twoFingerTap()
        settle()
        let s5 = try XCTUnwrap(state())
        XCTAssertGreaterThan(s5.scale, s4.scale * 1.2, "two-finger tap did not zoom out")

        // Autopilot button: on, zooming; again: off.
        let autopilot = app.buttons["Autopilot"]
        XCTAssertTrue(autopilot.isHittable, "autopilot button not hittable")
        autopilot.tap()
        settle(2)
        let a1 = try XCTUnwrap(state())
        XCTAssertTrue(a1.autopilot, "autopilot button did not start the autopilot")
        XCTAssertLessThan(a1.scale, s5.scale, "autopilot did not zoom")
        shot("autopilot")
        autopilot.tap()
        settle(2)
        XCTAssertFalse(try XCTUnwrap(state()).autopilot, "autopilot button did not stop the autopilot")

        // Journey button: on (planning, then flying); again: off.
        let journey = app.buttons["Journey"]
        XCTAssertTrue(journey.isHittable, "journey button not hittable")
        journey.tap()
        settle(3)
        XCTAssertTrue(try XCTUnwrap(state()).journey, "journey button did not start a journey")
        shot("journey")
        journey.tap()
        settle(2)
        XCTAssertFalse(try XCTUnwrap(state()).journey, "journey button did not stop the journey")

        // Control center: opens (sheet on iPhone, sidebar on iPad) and the app stays
        // responsive (the HEIC thumbnails once deadlocked the main thread here: a hung app
        // shows a stale frame, so the screenshot alone proves nothing; the queries time out).
        let control = app.buttons["Control center"]
        XCTAssertTrue(control.isHittable, "control center button not hittable")
        XCTAssertEqual(control.value as? String, "hidden")
        control.tap()
        settle(3)
        shot("control-center")
        XCTAssertEqual(control.value as? String, "shown", "the tap did not open the control center")
        XCTAssertTrue(app.staticTexts["Places"].waitForExistence(timeout: 10), "control center not on screen")
        // Landscape: the HUD, the buttons and the control center must still fit.
        XCUIDevice.shared.orientation = .landscapeLeft
        settle(3)
        shot("landscape")
        XCTAssertNotNil(state(), "app not responding in landscape")
        XCUIDevice.shared.orientation = .portrait
        settle(1)
    }

    /// A stream of taps keeps zooming (kb: decisions/controls "Animated zoom steps"): every tap
    /// from the second on zooms ×2, so a triple tap zooms ×4 and a quadruple tap ×8; a single
    /// tap later does not zoom.
    func testTapStreamsZoom() throws {
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        settle(3)
        let spot = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.45, dy: 0.55))
        for (taps, factor) in [(2, 2.0), (3, 4.0), (4, 8.0)] {
            let s0 = try XCTUnwrap(state()).scale
            canvas.tap(withNumberOfTaps: taps, numberOfTouches: 1)
            settle()
            let s1 = try XCTUnwrap(state()).scale
            XCTAssertEqual(s0 / s1, factor, accuracy: factor * 0.02, "\(taps) taps: ×\(s0 / s1), expected ×\(factor)")
        }
        let s0 = try XCTUnwrap(state()).scale
        spot.tap()
        settle()
        XCTAssertEqual(try XCTUnwrap(state()).scale, s0, accuracy: s0 * 1e-6, "a lone tap zoomed")
        shot("tap-stream")
    }
}
