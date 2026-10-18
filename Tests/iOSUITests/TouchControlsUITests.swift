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

    /// Drags the throttle's knob from the middle to signed tick `tick` (a position lever).
    private func throttle(_ lever: XCUIElement, tick: Int) {
        ThrottleLeverUI(element: lever).drag(toTick: tick)
    }

    private func launchSettled() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        settle(3)
    }

    /// The throttle, a position lever: a drag to a tick puts the knob there and it stays (the
    /// cruise in that gear, zooming in); the chevrons step one gear; one drag crosses the middle
    /// into reverse; a tap on a tick goes there; a double tap goes to manual (M). In the autopilot
    /// it sets the autopilot's speed, and the knob follows speed changes made elsewhere.
    func testThrottle() throws {
        try launchSettled()
        let element = app.descendants(matching: .any)["throttle"]
        XCTAssertTrue(element.waitForExistence(timeout: 5), "no throttle")
        XCTAssertTrue(element.isHittable, "throttle not hittable")
        XCTAssertGreaterThanOrEqual(element.frame.width, 44)
        let lever = ThrottleLeverUI(element: element)
        let ticks = lever.ticks
        XCTAssertGreaterThanOrEqual(ticks.count, 4, "ticks \(ticks)")
        XCTAssertEqual(ticks.first, 1)
        XCTAssertEqual(ticks.last, 26)
        XCTAssertGreaterThanOrEqual(lever.pitch, 10)
        XCTAssertEqual(lever.gear, 0)
        shot("throttle-start")
        print("QA throttle \(element.frame.size): \(element.value as? String ?? "-")")
        let s0 = scale()
        // Drag to the third tick and let go: the knob stays there, the gear is that tick's.
        lever.drag(toTick: 3)
        settle(0.5)
        XCTAssertEqual(level(), "c", "the throttle did not start the cruise")
        XCTAssertEqual(gear(), lever.gear(tick: 3), "the third tick's gear")
        XCTAssertEqual(lever.gear, gear(), "the knob shows the gear")
        settle(2)
        XCTAssertEqual(lever.gear, lever.gear(tick: 3), "the knob did not stay")
        XCTAssertLessThan(scale(), s0 / 1.2, "cruise did not zoom in")
        shot("throttle-tick3")
        // The chevrons: one gear each.
        let g3 = gear()
        lever.tapChevron(up: true)
        settle(0.5)
        XCTAssertEqual(gear(), g3 + 1, "the up chevron is one gear")
        XCTAssertEqual(lever.gear, g3 + 1)
        shot("throttle-chevron-up")
        lever.tapChevron(up: false)
        settle(0.6)
        lever.tapChevron(up: false)
        settle(0.5)
        XCTAssertEqual(gear(), g3 - 1, "two down chevrons")
        // One drag through the middle: reverse (no detent).
        lever.drag(toTick: -2)
        settle(0.5)
        XCTAssertEqual(gear(), lever.gear(tick: -2), "the drag did not cross into reverse")
        XCTAssertEqual(lever.gear, gear())
        shot("throttle-reverse")
        // A tap on the first tick up.
        settle(0.5)
        lever.tap(tick: 1)
        settle(1)
        XCTAssertEqual(gear(), 1, "a tap on the first tick")
        // Double tap: manual (the cruise brakes to rest).
        element.doubleTap()
        settle(4)
        XCTAssertEqual(level(), "m", "double tap did not stop")
        XCTAssertEqual(gear(), 0)
        XCTAssertEqual(lever.gear, 0)
        shot("throttle-stopped")
        // In the autopilot: the knob shows its speed's gear and sets it.
        app.buttons["Autopilot"].tap()
        settle(1)
        XCTAssertEqual(level(), "a")
        let g = gear()
        XCTAssertGreaterThan(g, 0, "the autopilot's gear")
        XCTAssertEqual(lever.gear, g, "the knob did not follow the autopilot")
        lever.tapChevron(up: true)
        settle(0.5)
        XCTAssertEqual(level(), "a", "the throttle left the autopilot")
        XCTAssertEqual(gear(), g + 1, "the autopilot's speed did not step")
        // A speed change elsewhere (the autopilot button's menu, E) moves the knob.
        app.buttons["Autopilot"].press(forDuration: 1.0)
        let faster = app.buttons["Cruise In / Faster"]
        XCTAssertTrue(faster.waitForExistence(timeout: 5), "no autopilot menu")
        faster.tap()
        settle(1)
        XCTAssertEqual(gear(), g + 2, "E did not step the autopilot")
        XCTAssertEqual(lever.gear, g + 2, "the knob did not follow")
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
        print("QA throttle landscape \(lever.frame.size): \(lever.value as? String ?? "-")")
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
        throttle(lever, tick: 1)
        settle(0.5)
        XCTAssertEqual(level(), "c")
        XCUIDevice.shared.orientation = .portrait
        settle(1)
        shot("controls-portrait")
    }

    // MARK: Round 2 (kb: decisions/controls "Touch", 2026-10-18 evening)

    private var menuShown: Bool { app.descendants(matching: .any)["Fly Here"].firstMatch.exists }

    /// A finger resting before it drags steers in A and C (it opened the long-press menu); in M
    /// the long press is still the menu.
    func testHoldSteersInAutopilotAndCruise() throws {
        try launchSettled()
        let rest = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.8, dy: 0.4))
        let away = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.85, dy: 0.3))
        app.buttons["Autopilot"].tap()
        settle(1)
        XCTAssertEqual(level(), "a")
        // At rest 1 s, then a drag: no menu, still A (a touch never hands over in A).
        rest.press(forDuration: 1.0, thenDragTo: away, withVelocity: 120, thenHoldForDuration: 0.5)
        settle(0.5)
        XCTAssertFalse(menuShown, "a resting finger opened the menu in A")
        XCTAssertEqual(level(), "a")
        // Only at rest: no menu either.
        rest.press(forDuration: 1.0)
        settle(0.5)
        XCTAssertFalse(menuShown, "a held finger opened the menu in A")
        shot("hold-steers-autopilot")
        app.buttons["Autopilot"].tap()
        settle(2)
        // C (the throttle on its first tick): the same.
        let lever = app.descendants(matching: .any)["throttle"]
        throttle(lever, tick: 1)
        settle(0.5)
        XCTAssertEqual(level(), "c")
        let v0 = canvas.value as? String
        rest.press(forDuration: 1.0, thenDragTo: away, withVelocity: 120, thenHoldForDuration: 1.0)
        settle(0.3)
        XCTAssertFalse(menuShown, "a resting finger opened the menu in C")
        XCTAssertEqual(level(), "c")
        XCTAssertNotEqual(canvas.value as? String, v0)
        lever.doubleTap()
        settle(4)
        XCTAssertEqual(level(), "m")
        // M: the long press is the menu.
        rest.press(forDuration: 0.8)
        XCTAssertTrue(app.descendants(matching: .any)["Fly Here"].firstMatch.waitForExistence(timeout: 5),
                      "no menu in M")
    }

    /// First-run hints: shown (forced here; the UI tests count them as seen), gone at the first
    /// touch, back from the help sheet.
    func testTouchHints() throws {
        app.launchArguments = ["--touch-hints"]
        try launchSettled()
        let hints = app.descendants(matching: .any)["touch-hints"].firstMatch
        XCTAssertTrue(hints.waitForExistence(timeout: 5), "no touch hints")
        shot("touch-hints")
        XCUIDevice.shared.orientation = .landscapeLeft
        settle(2)
        shot("touch-hints-landscape")
        XCUIDevice.shared.orientation = .portrait
        settle(2)
        let s0 = scale()
        canvas.doubleTap()
        settle(1)
        XCTAssertFalse(hints.exists, "the hints stayed after a touch")
        XCTAssertLessThan(scale(), s0, "the dismissing touch did not act (double tap zooms in)")
        // Help ▸ Show Touch Hints: the cockpit's (?) popover, More…, the button.
        try XCTUnwrap(point("help-cockpit")).tap()
        let more = app.buttons["More…"]
        XCTAssertTrue(more.waitForExistence(timeout: 5), "no More… in the popover")
        more.tap()
        let again = app.buttons["Show Touch Hints"]
        XCTAssertTrue(again.waitForExistence(timeout: 5), "no Show Touch Hints in help")
        shot("help-sheet")
        again.tap()
        settle(1.5)
        XCTAssertTrue(hints.waitForExistence(timeout: 5), "help did not show the hints again")
    }

    /// The tour card on a phone in portrait: at the top, folding to its title and buttons; the
    /// maps and the cockpit stay free.
    func testTourCardCompact() throws {
        app.launchArguments = ["--ui-demo=tour-feigenbaum"]
        try launchSettled()
        let next = app.buttons["Next ›"]
        XCTAssertTrue(next.waitForExistence(timeout: 10), "no tour card")
        settle(3)
        guard UIDevice.current.userInterfaceIdiom == .phone else {
            shot("tour-card")
            return
        }
        XCTAssertLessThan(next.frame.maxY, canvas.frame.height * 0.45, "the card is not at the top")
        if let p = point("manual") {
            XCTAssertGreaterThan(p.screenPoint.y, next.frame.maxY + 20, "the card covers the cockpit")
        }
        shot("tour-card-compact")
        let fold = app.buttons["tour-fold"]
        XCTAssertTrue(fold.isHittable)
        fold.tap()
        settle(1)
        XCTAssertFalse(app.descendants(matching: .any)["tour-text"].exists, "the text did not fold")
        XCTAssertLessThan(next.frame.maxY, canvas.frame.height * 0.25)
        shot("tour-card-folded")
        next.tap()
        settle(2)
        XCTAssertTrue(app.staticTexts["2 of 9"].waitForExistence(timeout: 5), "Next did not step while folded")
        fold.tap()
        settle(1)
        XCTAssertTrue(app.descendants(matching: .any)["tour-text"].exists, "the text did not unfold")
    }

    /// A long press on the mini-map flies there (the Mac's ⌥-click); no menu.
    func testMiniMapLongPressFlies() throws {
        try launchSettled()
        app.typeKey("m", modifierFlags: [])
        settle(2)
        // The map row: bottom left, its first panel the whole set (aim at its upper part, left
        // of centre: the antenna side, away from the view's spot).
        let h = canvas.frame.height
        let phonePortrait = UIDevice.current.userInterfaceIdiom == .phone && canvas.frame.width < canvas.frame.height
        let mapPoint = canvas.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: 50, dy: h - (phonePortrait ? 120 : 110)))
        shot("map-on")
        let before = fields()
        mapPoint.press(forDuration: 0.8)
        settle(0.5)
        XCTAssertFalse(menuShown, "a long press on the map opened the menu")
        settle(6)
        let after = fields()
        XCTAssertTrue(before["re"] != after["re"] || before["im"] != after["im"], "the map's long press did not move the view")
        shot("map-long-press-flew")
    }

    /// Tests do not persist settings: "Orbit Here" in a UI test leaves the next normal launch
    /// with the orbit off.
    func testTestsDoNotPersistSettings() throws {
        try launchSettled()
        let at = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.35, dy: 0.42))
        at.press(forDuration: 0.8)
        let item = app.descendants(matching: .any)["Orbit Here"].firstMatch
        XCTAssertTrue(item.waitForExistence(timeout: 5))
        item.tap()
        settle(1.5)
        XCTAssertEqual(fields()["orbit"], "on")
        app.terminate()
        // A normal launch (no test environment): the user's settings, orbit off.
        let normal = XCUIApplication()
        normal.launch()
        let c = normal.otherElements["fractal"]
        XCTAssertTrue(c.waitForExistence(timeout: 30))
        settle(3)
        let v = (c.value as? String ?? "").split(separator: " ").map(String.init)
        let i = v.firstIndex(of: "orbit")
        XCTAssertNotNil(i)
        if let i { XCTAssertEqual(v[i + 1], "off", "the test's Orbit Here persisted") }
        normal.terminate()
    }

    /// Every iOS surface for a look (portrait and landscape): the control center (sheet detents
    /// on a phone), the places, fly-to, a help popover, a journey stop's buttons.
    func testSurfaces() throws {
        try launchSettled()
        let isPhone = UIDevice.current.userInterfaceIdiom == .phone
        let control = app.buttons["Control center"]
        control.tap()
        settle(3)
        shot("surface-control-center")
        if isPhone {
            app.swipeUp()
            settle(1.5)
            shot("surface-control-center-large")
        }
        let search = app.textFields["Search settings"]
        XCTAssertTrue(search.waitForExistence(timeout: 5), "no search field")
        func find(_ text: String) {
            search.tap()
            if let old = search.value as? String, !old.isEmpty, old != "Search settings" {
                search.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: old.count))
            }
            search.typeText(text + "\n")
            settle(1.5)
        }
        // Fly to…: the sheet fits a phone (it had a 420-point minimum width).
        find("lock")  // the View section only ("fly" also finds every place first)
        let fly = app.buttons["Fly to…"]
        if fly.waitForExistence(timeout: 5) {
            fly.tap()
            settle(1.5)
            shot("surface-fly-to")
            let cancel = app.buttons["Cancel"]
            XCTAssertTrue(cancel.waitForExistence(timeout: 5), "fly-to sheet")
            XCTAssertTrue(cancel.isHittable, "fly-to Cancel clipped")
            XCTAssertTrue(app.buttons["Fly"].isHittable, "fly-to Fly clipped")
            XCTAssertLessThanOrEqual(app.buttons["Fly"].frame.maxX, app.windows.firstMatch.frame.maxX, "Fly off screen")
            cancel.tap()
            settle(1)
        } else {
            XCTFail("no Fly to… in the control center")
        }
        // Journey: stop at each find.
        find("journey")
        shot("surface-journey-section")
        let picker = app.buttons.containing(NSPredicate(format: "label CONTAINS 'Continuous' OR value CONTAINS 'Continuous'")).firstMatch
        if picker.waitForExistence(timeout: 3) {
            picker.tap()
            settle(0.8)
            let item = app.descendants(matching: .any)["Stop at each find"].firstMatch
            if item.waitForExistence(timeout: 3) { item.tap() }
        }
        settle(0.5)
        XCUIDevice.shared.orientation = .landscapeLeft
        settle(2)
        shot("surface-control-center-landscape")
        XCUIDevice.shared.orientation = .portrait
        settle(2)
        if isPhone {
            app.swipeDown(velocity: .fast)
            settle(1)
            if control.value as? String == "shown" {
                app.swipeDown(velocity: .fast)
                settle(1.5)
            }
        } else {
            control.tap()
            settle(1.5)
        }
        app.buttons["Journey"].firstMatch.tap()
        let deeper = app.buttons["Journey: deeper here"]
        if deeper.waitForExistence(timeout: 90) {
            settle(1)
            shot("surface-journey-stop")
            for name in ["Journey: next landmark", "Journey: deeper here", "Journey: somewhere else", "Control center"] {
                let b = app.buttons[name]
                XCTAssertTrue(b.isHittable, "\(name) not hittable at a stop")
                XCTAssertGreaterThanOrEqual(b.frame.width, 44)
            }
            XCUIDevice.shared.orientation = .landscapeLeft
            settle(2)
            shot("surface-journey-stop-landscape")
            for name in ["Journey: next landmark", "Journey: deeper here", "Journey: somewhere else", "Control center"] {
                XCTAssertTrue(app.buttons[name].isHittable, "\(name) not hittable at a stop (landscape)")
            }
            XCUIDevice.shared.orientation = .portrait
            settle(1)
        } else {
            shot("surface-journey-no-stop")
            XCTFail("no journey stop within 90 s")
        }
    }
}
