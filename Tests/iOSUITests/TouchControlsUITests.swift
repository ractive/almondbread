import XCTest

/// The touch controls (kb: decisions/controls "Touch") on iPhone and iPad simulators: the
/// throttle lever, the cockpit's icons and (?) badge, the long-press menu and the tour card,
/// through real UIKit touches. The fractal view's accessibility value says what they did
/// ("… level c gear 3 orbit on seek flying lane off cockpit manual:x,y …"; the cockpit's
/// positions only with `ALMONDBREAD_UITEST=1`). Screenshots as in `TouchUITests`.
final class TouchControlsUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        app = XCUIApplication()
        app.launchEnvironment["ALMONDBREAD_UITEST"] = "1"
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }

    /// The view's value as key-value pairs ("scale" → "0.0042", "level" → "c", "manual" → "120,700").
    private func fields() -> [String: String] {
        guard let v = canvas.value as? String else { return [:] }
        let f = v.split(separator: " ").map(String.init)
        guard f.count >= 8 else { return [:] }
        var d = ["re": f[0], "im": f[1], "scale": f[2]]
        var i = 4
        while i + 1 < f.count {
            if f[i] == "cockpit" {
                for t in f[(i + 1)...] {
                    let kv = t.split(separator: ":", maxSplits: 1).map(String.init)
                    if kv.count == 2 { d[kv[0]] = kv[1] }
                }
                break
            }
            d[f[i]] = f[i + 1]
            i += 2
        }
        return d
    }

    private func scale() -> Double { Double(fields()["scale"] ?? "") ?? .nan }
    private func gear() -> Int { Int(fields()["gear"] ?? "") ?? -999 }
    private func level() -> String { fields()["level"] ?? "?" }

    /// A point of the canvas in view points from its top-left (the cockpit's positions).
    private func point(_ id: String) -> XCUICoordinate? {
        guard let p = fields()[id] else { return nil }
        let xy = p.split(separator: ",").compactMap { Double($0) }
        guard xy.count == 2 else { return nil }
        return canvas.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: xy[0], dy: xy[1]))
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

    private func settle(_ seconds: Double = 1.5) {
        Thread.sleep(forTimeInterval: seconds)
    }

    /// Drags the throttle's knob by `detents` (up positive) in one drag.
    private func throttle(_ lever: XCUIElement, detents: Double) {
        let from = lever.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let to = from.withOffset(CGVector(dx: 0, dy: -detents * 14 - 7 * (detents > 0 ? 1 : -1)))
        from.press(forDuration: 0.05, thenDragTo: to, withVelocity: 120, thenHoldForDuration: 0.1)
    }

    private func launchSettled() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        settle(3)
    }

    /// The throttle: up three detents is cruise gear 3 and zooms in; down past zero in the same
    /// drag stops at zero; a new drag goes into reverse; a double tap goes to manual. In the
    /// autopilot it steps the autopilot's speed.
    func testThrottle() throws {
        try launchSettled()
        let lever = app.descendants(matching: .any)["throttle"]
        XCTAssertTrue(lever.waitForExistence(timeout: 5), "no throttle")
        XCTAssertTrue(lever.isHittable, "throttle not hittable")
        XCTAssertGreaterThanOrEqual(lever.frame.width, 44)
        shot("throttle-start")
        let s0 = scale()
        throttle(lever, detents: 3)
        settle(0.5)
        XCTAssertEqual(level(), "c", "the throttle did not start the cruise")
        XCTAssertEqual(gear(), 3, "three detents, gear 3")
        settle(2)
        XCTAssertLessThan(scale(), s0 / 1.2, "cruise did not zoom in")
        shot("throttle-gear3")
        // Down past zero in one drag: stops at zero.
        throttle(lever, detents: -6)
        settle(0.5)
        XCTAssertEqual(gear(), 0, "the drag crossed zero")
        // A new drag: reverse.
        throttle(lever, detents: -2)
        settle(0.5)
        XCTAssertEqual(gear(), -2, "a new drag goes into reverse")
        shot("throttle-reverse")
        // Double tap: manual (the cruise brakes to rest).
        lever.doubleTap()
        settle(4)
        XCTAssertEqual(level(), "m", "double tap did not stop")
        XCTAssertEqual(gear(), 0)
        // In the autopilot: its speed.
        app.buttons["Autopilot"].tap()
        settle(1)
        XCTAssertEqual(level(), "a")
        let g = gear()
        throttle(lever, detents: 2)
        settle(0.5)
        XCTAssertEqual(level(), "a", "the throttle left the autopilot")
        // Its default speed is off the ladder: the first detent goes to the next rung.
        XCTAssertGreaterThan(gear(), g, "the autopilot's speed did not step")
        shot("throttle-autopilot")
        app.buttons["Autopilot"].tap()
    }

    /// The cockpit's icons take taps (44-point cells) through Rust's hit test; its (?) opens the
    /// help popover.
    func testCockpitTaps() throws {
        try launchSettled()
        let cruise = try XCTUnwrap(point("cruise"), "no cockpit positions: \(fields())")
        cruise.tap()
        settle(1)
        XCTAssertEqual(level(), "c", "the cruise icon did not start cruising")
        XCTAssertEqual(gear(), 1)
        // 18 points off the autopilot icon's centre is still the autopilot (a 44-point cell).
        try XCTUnwrap(point("autopilot")).withOffset(CGVector(dx: 0, dy: -18)).tap()
        settle(1)
        XCTAssertEqual(level(), "a", "the autopilot icon")
        shot("cockpit-autopilot")
        try XCTUnwrap(point("manual")).tap()
        settle(3)
        XCTAssertEqual(level(), "m", "the manual icon did not stop")
        XCTAssertEqual(fields()["assist"], "off")
        try XCTUnwrap(point("lane")).tap()
        settle(0.5)
        XCTAssertEqual(fields()["assist"], "on", "the lane icon did not toggle lane assist")
        try XCTUnwrap(point("lane")).tap()
        settle(0.5)
        XCTAssertEqual(fields()["assist"], "off")
        try XCTUnwrap(point("journey")).tap()
        settle(2)
        XCTAssertEqual(level(), "j", "the journey icon")
        try XCTUnwrap(point("journey")).tap()
        settle(1)
        // The (?) badge: a fingertip next to it opens the levels' help.
        try XCTUnwrap(point("help-cockpit")).withOffset(CGVector(dx: 8, dy: -6)).tap()
        settle(1.5)
        shot("cockpit-help")
        XCTAssertTrue(app.staticTexts["Driving levels"].waitForExistence(timeout: 5), "no cockpit help popover")
    }

    /// The long-press menu: Orbit here, Seek minibrot here, Copy coordinates (then Go to copied
    /// is offered), Fly here; a long-press drag moves the orbit probe (screenshot).
    func testLongPressMenu() throws {
        try launchSettled()
        let at = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.35, dy: 0.42))
        func menu(_ title: String) throws {
            at.press(forDuration: 0.8)
            let item = app.descendants(matching: .any)[title].firstMatch
            XCTAssertTrue(item.waitForExistence(timeout: 5), "no '\(title)' in the long-press menu")
            shot("longpress-menu")
            item.tap()
        }
        try menu("Orbit Here")
        settle(1.5)
        XCTAssertEqual(fields()["orbit"], "on", "Orbit Here did not turn the orbit on")
        shot("longpress-orbit")
        // A long-press drag moves the probe (no menu).
        let to = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: 0.5))
        at.press(forDuration: 0.8, thenDragTo: to, withVelocity: 150, thenHoldForDuration: 0.3)
        settle(1)
        XCTAssertFalse(app.descendants(matching: .any)["Fly Here"].firstMatch.exists, "a drag opened the menu")
        shot("longpress-probe-moved")
        try menu("Copy Coordinates")
        settle(0.5)
        at.press(forDuration: 0.8)
        XCTAssertTrue(app.descendants(matching: .any)["Go to Copied"].firstMatch.waitForExistence(timeout: 5),
                      "no Go to Copied after a copy")
        // Dismiss with a tap elsewhere.
        canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.15)).tap()
        settle(1)
        let s0 = scale()
        try menu("Fly Here")
        settle(8)
        XCTAssertLessThan(scale(), s0 / 6, "Fly Here did not fly in (×8)")
        try menu("Seek Minibrot Here")
        settle(0.5)
        let phase = fields()["seek"] ?? ""
        XCTAssertTrue(["searching", "flying", "arrived", "not-found"].contains(phase), "seek phase \(phase)")
        settle(6)
        shot("longpress-seek")
    }

    /// The tour card's buttons are finger-sized and step the tour.
    func testTourCard() throws {
        app.launchArguments = ["--ui-demo=tour-feigenbaum"]
        try launchSettled()
        let next = app.buttons["Next ›"]
        XCTAssertTrue(next.waitForExistence(timeout: 10), "no tour card")
        XCTAssertGreaterThanOrEqual(next.frame.height, 30, "Next is not finger-sized")
        shot("tour-1")
        next.tap()
        settle(2)
        XCTAssertTrue(app.staticTexts["2 of 9"].waitForExistence(timeout: 5), "Next did not step")
        app.buttons["‹ Back"].tap()
        settle(2)
        XCTAssertTrue(app.staticTexts["1 of 9"].waitForExistence(timeout: 5), "Back did not step")
        shot("tour-back")
        app.buttons["End tour"].tap()
        settle(1)
        XCTAssertFalse(next.exists, "End tour did not end it")
    }

    /// Landscape: the throttle, the buttons and the cockpit still fit and do not overlap.
    func testLandscapeLayout() throws {
        try launchSettled()
        XCUIDevice.shared.orientation = .landscapeLeft
        settle(3)
        shot("controls-landscape")
        let lever = app.descendants(matching: .any)["throttle"]
        XCTAssertTrue(lever.isHittable)
        for name in ["Autopilot", "Journey", "Control center"] {
            let b = app.buttons[name]
            XCTAssertTrue(b.isHittable, "\(name) not hittable in landscape")
            XCTAssertFalse(b.frame.intersects(lever.frame), "\(name) under the throttle")
        }
        if let c = point("journey") {
            let p = c.screenPoint
            XCTAssertFalse(lever.frame.contains(p), "cockpit under the throttle")
            for name in ["Autopilot", "Journey", "Control center"] {
                XCTAssertFalse(app.buttons[name].frame.insetBy(dx: -4, dy: -4).contains(p), "cockpit under \(name)")
            }
        }
        throttle(lever, detents: 1)
        settle(0.5)
        XCTAssertEqual(level(), "c")
        XCUIDevice.shared.orientation = .portrait
        settle(1)
        shot("controls-portrait")
    }
}
