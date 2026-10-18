import XCTest

/// First-use sessions (kb: platform/ios-simulator "QA"): what a child who has never seen the app
/// does with it, in order, with a screenshot at every step (`build/ios-screens/<idiom>-first-*`).
/// The asserts catch stuck states: every mode started is left again with the control a child
/// would use (the same button, the throttle's double tap, the cockpit), and the app is still
/// running and drawing after rotations and a trip to the background.
final class FirstUseUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        app = XCUIApplication()
        app.launchEnvironment["ALMONDBREAD_UITEST"] = "1"
    }

    override func tearDown() {
        XCUIDevice.shared.orientation = .portrait
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }
    private var isPad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    /// The view's value as key-value pairs (see `TouchControlsUITests.fields`).
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
                    // Prefixed: the cockpit's "autopilot" and "journey" cells share names with
                    // the state fields.
                    if kv.count == 2 { d["ck." + kv[0]] = kv[1] }
                }
                break
            }
            d[f[i]] = f[i + 1]
            i += 2
        }
        return d
    }

    private func scale() -> Double { Double(fields()["scale"] ?? "") ?? .nan }
    private func level() -> String { fields()["level"] ?? "?" }

    private func point(_ id: String) -> XCUICoordinate? {
        guard let p = fields()["ck." + id] else { return nil }
        let xy = p.split(separator: ",").compactMap { Double($0) }
        guard xy.count == 2 else { return nil }
        return canvas.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: xy[0], dy: xy[1]))
    }

    private var step = 0
    private func shot(_ name: String) {
        step += 1
        let png = XCUIScreen.main.screenshot().pngRepresentation
        // "-[FirstUseUITests testFirstSession]" → "firstsession"
        let test = (self.name.split(separator: " ").last.map(String.init) ?? "x")
            .replacingOccurrences(of: "test", with: "").replacingOccurrences(of: "]", with: "").lowercased()
        let label = String(format: "first-%@-%02d-%@", test, step, name)
        let a = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
        a.name = label
        a.lifetime = .keepAlways
        add(a)
        let device = isPad ? "ipad" : "iphone"
        if let dir = ProcessInfo.processInfo.environment["ALMONDBREAD_UITEST_SHOTS"] {
            try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
            try? png.write(to: URL(fileURLWithPath: "\(dir)/\(device)-\(label).png"))
        }
        print("QA \(label): \(canvas.exists ? (canvas.value as? String ?? "-").prefix(160) : "no canvas")")
    }

    private func settle(_ seconds: Double = 1.5) { Thread.sleep(forTimeInterval: seconds) }

    private func alive(_ what: String) {
        XCTAssertEqual(app.state, .runningForeground, "app not in the foreground after \(what)")
        XCTAssertTrue(canvas.waitForExistence(timeout: 10), "no canvas after \(what)")
    }

    /// Drags the throttle's knob from the middle to signed tick `tick` (a position lever).
    private func throttle(_ lever: XCUIElement, tick: Int) {
        ThrottleLeverUI(element: lever).drag(toTick: tick)
    }

    private func waitFor(_ seconds: Double, _ condition: () -> Bool) -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end {
            if condition() { return true }
            settle(0.3)
        }
        return condition()
    }

    /// A whole first session on the canvas: hints, drag, pinch deep, the plane and a steering
    /// finger, the throttle both ways, a journey, rotations, background and back, the cockpit.
    func testFirstSession() throws {
        app.launchArguments = ["--touch-hints"]
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        settle(3)
        shot("hints")
        XCTAssertTrue(app.descendants(matching: .any)["touch-hints"].firstMatch.exists, "no hints on first start")
        // The first touch: a tap in the middle (the hints go; a single tap does nothing else).
        canvas.tap()
        settle(1)
        XCTAssertFalse(app.descendants(matching: .any)["touch-hints"].firstMatch.exists, "hints stayed")
        shot("hints-gone")

        // Drag.
        let v0 = fields()
        canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.5))
            .press(forDuration: 0.05, thenDragTo: canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.7, dy: 0.45)))
        settle(1.5)
        XCTAssertNotEqual(fields()["re"], v0["re"], "the drag did not move")
        XCTAssertEqual(level(), "m", "a drag changed the level")
        shot("dragged")

        // Pinch in deep: five pinches, about ×3 each.
        let s0 = scale()
        for _ in 0..<5 {
            canvas.pinch(withScale: 3, velocity: 3)
            settle(0.6)
        }
        settle(2.5)
        // XCUITest's synthesised pinches zoom much less than their scale says.
        XCTAssertLessThan(scale(), s0 / 2, "pinching did not zoom in")
        XCTAssertEqual(level(), "m", "pinching changed the level")
        shot("pinched-deep")
        settle(4)
        shot("pinched-deep-settled")
        // After the pinches a drag still pans (every finger was lifted; no stuck pinch).
        let p0 = fields()["re"]
        canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.5))
            .press(forDuration: 0.05, thenDragTo: canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: 0.5)))
        settle(1)
        XCTAssertNotEqual(fields()["re"], p0, "a drag after pinching did not move")
        shot("pinched-then-dragged")

        // The cockpit's plane: autopilot. A finger steers; a tap pauses; the plane again stops.
        let plane = try XCTUnwrap(point("autopilot"), "no cockpit plane")
        plane.tap()
        settle(3)
        XCTAssertEqual(level(), "a", "the plane did not start the autopilot")
        shot("autopilot")
        let steerFrom = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let steerTo = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.3))
        steerFrom.press(forDuration: 0.1, thenDragTo: steerTo, withVelocity: 200, thenHoldForDuration: 2)
        settle(0.5)
        XCTAssertEqual(level(), "a", "steering left the autopilot")
        XCTAssertFalse(app.descendants(matching: .any)["Fly Here"].firstMatch.exists, "steering opened the menu")
        shot("autopilot-steered")
        settle(3)
        shot("autopilot-after-steer")
        // The wish line went with the finger (device round: it stayed until the autopilot ended).
        XCTAssertEqual(fields()["wish"], "off", "the wish line stayed after the finger lifted")
        plane.tap()
        settle(3)
        XCTAssertNotEqual(level(), "a", "the plane did not stop the autopilot")
        XCTAssertEqual(fields()["autopilot"], "off")
        shot("autopilot-off")

        // The throttle: up to the third tick, then down into reverse, then a double tap stops.
        let lever = app.descendants(matching: .any)["throttle"]
        XCTAssertTrue(lever.waitForExistence(timeout: 5) && lever.isHittable, "throttle")
        throttle(lever, tick: 3)
        settle(3)
        XCTAssertEqual(level(), "c")
        XCTAssertGreaterThan(ThrottleLeverUI(element: lever).gear, 0, "the knob is not up")
        shot("throttle-forward")
        // Cruising, a drag steers by moving the picture (device round: it did not seem to).
        let c0 = fields()
        canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.5))
            .press(forDuration: 0.05, thenDragTo: canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.7, dy: 0.5)),
                   withVelocity: 300, thenHoldForDuration: 0.2)
        let c1 = fields()
        // The pinches above may have turned the view (XCUITest pinches twist): the distance.
        let num = { (f: [String: String], k: String) in Double(f[k] ?? "") ?? .nan }
        let moved = hypot(num(c0, "re") - num(c1, "re"), num(c0, "im") - num(c1, "im")) / num(c1, "scale")
        XCTAssertGreaterThan(moved, 0.2 * canvas.frame.width * 0.4, "a drag in the cruise did not move the picture")
        XCTAssertEqual(level(), "c", "a drag left the cruise")
        shot("cruise-dragged")
        throttle(lever, tick: -3)
        settle(3)
        XCTAssertLessThan(Int(fields()["gear"] ?? "") ?? 0, 0, "no reverse gear")
        XCTAssertLessThan(ThrottleLeverUI(element: lever).gear, 0, "the knob is not down")
        shot("throttle-reverse")
        lever.doubleTap()
        settle(4)
        XCTAssertEqual(level(), "m", "the throttle's double tap did not stop")
        shot("throttle-stopped")

        // A journey: start, watch, stop with the cockpit's sparkles.
        try XCTUnwrap(point("journey"), "no cockpit journey icon").tap()
        settle(8)
        XCTAssertEqual(fields()["journey"], "on", "the journey icon did not start one")
        shot("journey")
        settle(8)
        shot("journey-later")
        let journey = try XCTUnwrap(point("journey"))
        journey.tap()
        settle(3)
        if fields()["journey"] == "on" {
            // At a stop the icon flies on (J): a child presses again to stop.
            shot("journey-still-on")
            journey.tap()
            settle(3)
        }
        XCTAssertEqual(fields()["journey"], "off", "the journey icon did not stop the journey")
        shot("journey-off")

        // Rotate the device both ways (and upside down on an iPad).
        let orientations: [UIDeviceOrientation] = isPad
            ? [.landscapeLeft, .landscapeRight, .portraitUpsideDown, .portrait]
            : [.landscapeLeft, .landscapeRight, .portrait]
        for o in orientations {
            XCUIDevice.shared.orientation = o
            settle(0.4)
            shot("rotated-\(o.rawValue)-early")
            settle(2.5)
            alive("rotating")
            shot("rotated-\(o.rawValue)")
            XCTAssertTrue(lever.isHittable, "throttle not hittable after rotating to \(o.rawValue)")
            XCTAssertNotNil(point("autopilot"), "no cockpit plane after rotating to \(o.rawValue)")
            for name in ["Orbit", "Mini-map", "Size scale", "Fly to", "Control center"] {
                let button = app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", name)).firstMatch
                XCTAssertTrue(button.isHittable, "\(name) not hittable after rotating to \(o.rawValue)")
            }
        }

        // Background and back, while cruising.
        throttle(lever, tick: 2)
        settle(1)
        XCUIDevice.shared.press(.home)
        settle(3)
        app.activate()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10), "app did not come back")
        settle(0.3)
        shot("foreground-early")
        settle(3)
        alive("the background")
        shot("foreground")
        let s1 = scale()
        settle(2)
        XCTAssertNotEqual(scale(), s1, "the cruise did not go on after the background")

        // End everything with the cockpit's manual icon.
        let manual = try XCTUnwrap(point("manual"), "no cockpit")
        manual.tap()
        settle(4)
        XCTAssertEqual(level(), "m", "the cockpit's manual icon did not stop")
        let s2 = scale()
        settle(2)
        XCTAssertEqual(scale(), s2, "still moving after manual")
        shot("cockpit-manual")
    }

    /// A small twist (as in a pinch) does not turn the view; a deliberate one does (Rust's 15°
    /// dead band). The small one is a synthesised path (`TwoFingerTwist`): XCTest's
    /// `rotate(0.15)` is an 18° finger jump that UIKit reads as 12° or 25°.
    func testSmallTwistDoesNotTurn() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30))
        settle(3)
        func angle() -> Double {
            let v = (canvas.value as? String ?? "").split(separator: " ")
            return v.count > 3 ? Double(v[3].replacingOccurrences(of: "°", with: "")) ?? .nan : .nan
        }
        let f = canvas.frame
        XCTAssertTrue(TwoFingerTwist.perform(center: CGPoint(x: f.midX, y: f.midY), radius: min(f.width, f.height) / 6,
                                             radians: 0.15, duration: 0.6), "XCTest's event record is missing")
        settle(1.5)
        XCTAssertEqual(angle(), 0, accuracy: 0.01, "a 9° twist turned the view")
        canvas.rotate(.pi / 3, withVelocity: 1.5)
        settle(1.5)
        XCTAssertGreaterThan(abs(angle()), 20, "a 60° twist did not turn the view")
        shot("twisted")
    }

    /// The control center: open it, pick a place the way a child would (tap a picture: it
    /// selects and shows Go / Fly on it; tap it again: it goes there), start the Feigenbaum tour
    /// and step through it to the end, close it again.
    func testControlCenterPlaceAndTour() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30))
        settle(3)
        let control = app.buttons["Control center"]
        control.tap()
        settle(3)
        shot("cc-open")
        // A place: tap its picture (selects), then tap it again (goes there).
        let v0 = fields()
        let tile = app.buttons["Head and Shoulders"].firstMatch  // the second tile: on screen in the phone's half sheet too
        if tile.waitForExistence(timeout: 10) {
            tile.tap()
            settle(2)
            shot("cc-place-selected")
            XCTAssertTrue(app.buttons["placeTileGo"].waitForExistence(timeout: 5), "no Go on the selected picture")
            XCTAssertTrue(app.buttons["placeTileFly"].exists, "no Fly on the selected picture")
            XCTAssertEqual(fields()["re"], v0["re"], "the first tap on a picture moved the view")
            XCTAssertEqual(fields()["scale"], v0["scale"], "the first tap on a picture moved the view")
            // The second tap, on the picture away from the buttons (its top left corner).
            tile.coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.1)).tap()
            settle(3)
            shot("cc-place-tapped-again")
            let moved = fields()["re"] != v0["re"] || fields()["scale"] != v0["scale"]
            print("QA second place tap moved the view: \(moved)")
            XCTAssertTrue(moved, "tapping the selected place's picture did not go there")
        } else {
            XCTFail("no Head and Shoulders tile")
            shot("cc-no-tile")
        }
        // Close the control center (a phone: swipe the sheet down; iPad: the button).
        if isPad {
            if control.isHittable { control.tap() }
        } else {
            app.swipeDown(velocity: .fast)
            settle(1)
            if control.value as? String == "shown" { app.swipeDown(velocity: .fast) }
        }
        settle(2)
        shot("cc-closed-at-place")

        // The Feigenbaum tour from the control center's Tours section.
        control.tap()
        settle(2)
        let search = app.textFields["Search settings"]
        XCTAssertTrue(search.waitForExistence(timeout: 5))
        search.tap()
        search.typeText("tour\n")
        settle(1.5)
        shot("cc-tours")
        let start = app.buttons.matching(NSPredicate(format: "label == 'Start'")).firstMatch
        XCTAssertTrue(start.waitForExistence(timeout: 5), "no tour Start")
        start.tap()
        settle(2)
        if !isPad {
            app.swipeDown(velocity: .fast)
            settle(1)
            if control.value as? String == "shown" { app.swipeDown(velocity: .fast) }
        } else if control.isHittable {
            control.tap()
        }
        settle(2)
        shot("tour-1")
        var steps = 1
        while steps < 20 {
            let next = app.buttons["Next ›"]
            let finish = app.buttons["Finish"]
            if finish.exists {
                shot("tour-last")
                finish.tap()
                break
            }
            guard next.waitForExistence(timeout: 5) else {
                XCTFail("tour card lost at step \(steps)")
                shot("tour-lost")
                break
            }
            XCTAssertTrue(next.isHittable, "Next not hittable at step \(steps)")
            next.tap()
            steps += 1
            settle(4)
            shot("tour-\(steps)")
        }
        settle(2)
        XCTAssertFalse(app.buttons["Next ›"].exists, "the tour did not end")
        shot("tour-finished")
        alive("the tour")
    }

    /// The places grid on touch: a tap selects without moving the view and puts Go and Fly on
    /// the picture (one tile at a time); Go jumps; Fly flies (the view moves over several frames).
    func testPlaceTileGoAndFly() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30))
        settle(3)
        app.buttons["Control center"].tap()
        settle(3)
        let first = app.buttons["Head and Shoulders"].firstMatch
        let second = app.buttons["π at the Neck"].firstMatch
        XCTAssertTrue(first.waitForExistence(timeout: 10), "no Head and Shoulders tile")
        XCTAssertTrue(second.exists, "no π at the Neck tile")
        XCTAssertFalse(app.buttons["placeTileGo"].exists, "Go on a picture before any tap")

        // Select: nothing moves; Go and Fly appear, once.
        let v0 = fields()
        first.tap()
        settle(1.5)
        shot("place-selected")
        XCTAssertEqual(fields()["re"], v0["re"], "selecting moved the view")
        XCTAssertEqual(fields()["scale"], v0["scale"], "selecting moved the view")
        let go = app.buttons["placeTileGo"]
        let fly = app.buttons["placeTileFly"]
        XCTAssertTrue(go.waitForExistence(timeout: 5), "no Go on the selected picture")
        XCTAssertEqual(app.buttons.matching(identifier: "placeTileGo").count, 1)
        XCTAssertEqual(go.label, "Go to Head and Shoulders")
        XCTAssertTrue(go.isHittable, "Go not hittable")
        XCTAssertTrue(fly.isHittable, "Fly not hittable")

        // Go jumps.
        go.tap()
        settle(3)
        shot("place-go")
        let v1 = fields()
        XCTAssertTrue(v1["re"] != v0["re"] || v1["scale"] != v0["scale"], "Go did not go there")

        // Another tile moves the selection (still one Go) and does not move the view.
        second.tap()
        settle(1.5)
        shot("place-second-selected")
        XCTAssertEqual(fields()["re"], v1["re"], "selecting another place moved the view")
        XCTAssertEqual(fields()["scale"], v1["scale"], "selecting another place moved the view")
        XCTAssertEqual(app.buttons.matching(identifier: "placeTileGo").count, 1, "more than one selected tile")
        XCTAssertEqual(go.label, "Go to π at the Neck")

        // Fly flies: the view changes over several samples, not in one jump.
        fly.tap()
        var seen: Set<String> = []
        for k in 0..<8 {
            settle(0.5)
            let f = fields()
            seen.insert("\(f["re"] ?? "") \(f["scale"] ?? "")")
            if k == 2 { shot("place-flying") }
        }
        print("QA fly distinct views: \(seen.count)")
        XCTAssertGreaterThanOrEqual(seen.count, 3, "Fly did not move the view over time")
        settle(8)
        shot("place-flown")
        alive("Go and Fly")
    }

    /// Every long-press menu item, including Go to Copied through the system paste control.
    func testLongPressMenuItems() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30))
        settle(3)
        let at = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.38, dy: 0.45))
        func menu(_ title: String, shotName: String) -> Bool {
            at.press(forDuration: 0.8)
            let item = app.descendants(matching: .any)[title].firstMatch
            guard item.waitForExistence(timeout: 5) else {
                XCTFail("no '\(title)' in the long-press menu")
                shot("menu-missing-\(shotName)")
                return false
            }
            shot("menu-\(shotName)")
            item.tap()
            return true
        }
        _ = menu("Orbit Here", shotName: "orbit")
        settle(2)
        XCTAssertEqual(fields()["orbit"], "on")
        shot("orbit-on")
        _ = menu("Copy Coordinates", shotName: "copy")
        settle(0.5)
        let copied = fields()
        // Move away, then Go to Copied comes back.
        canvas.pinch(withScale: 4, velocity: 4)
        settle(2)
        XCTAssertNotEqual(fields()["scale"], copied["scale"])
        at.press(forDuration: 0.8)
        XCTAssertTrue(app.descendants(matching: .any)["Go to Copied"].firstMatch.waitForExistence(timeout: 5),
                      "no Go to Copied")
        // The system paste control beside it (no "Allow Paste?" prompt).
        let paste = app.buttons["Paste"].firstMatch
        XCTAssertTrue(paste.waitForExistence(timeout: 5), "no Paste control in the menu")
        shot("menu-paste")
        paste.tap()
        settle(1)
        // A system "Allow Paste" alert would be here (springboard).
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = springboard.buttons["Allow Paste"]
        if allow.exists || app.alerts.firstMatch.exists {
            shot("paste-prompt")
            XCTFail("Go to Copied asked to allow paste")
            if allow.exists { allow.tap() }
        }
        settle(2)
        shot("pasted")
        let back = fields()
        XCTAssertEqual(back["scale"], copied["scale"], "Go to Copied did not return")
        _ = menu("Fly Here", shotName: "fly")
        settle(8)
        shot("flew-here")
        // Seek from the menu: the search runs with nothing else moving (it was never polled
        // and said "searching" until the next touch).
        _ = menu("Seek Minibrot Here", shotName: "seek")
        settle(1.5)
        shot("seek-searching")
        XCTAssertTrue(waitFor(30) { fields()["seek"] != "searching" }, "the seek stayed 'searching'")
        print("QA seek ended: \(fields()["seek"] ?? "-")")
        settle(3)
        shot("seek")
        // Whatever the seek did, the cockpit's manual gets back to rest.
        if let manual = point("manual") { manual.tap() }
        settle(3)
        XCTAssertEqual(level(), "m")
        alive("the menu")
    }

    /// Small hands do many things at once: quick taps all over, double taps, pinches in and out,
    /// two-finger taps, rotations, buttons pressed twice. The app keeps running, the view stays a
    /// view (no NaN), and the controls still work afterwards.
    func testMashing() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30))
        settle(3)
        let spots: [CGVector] = [CGVector(dx: 0.2, dy: 0.3), CGVector(dx: 0.7, dy: 0.6), CGVector(dx: 0.5, dy: 0.5),
                                 CGVector(dx: 0.3, dy: 0.8), CGVector(dx: 0.8, dy: 0.2)]
        for round in 0..<3 {
            for s in spots { canvas.coordinate(withNormalizedOffset: s).tap() }
            for s in spots { canvas.coordinate(withNormalizedOffset: s).doubleTap() }
            canvas.pinch(withScale: 6, velocity: 8)
            canvas.twoFingerTap()
            canvas.rotate(.pi / 2, withVelocity: 3)
            canvas.pinch(withScale: 0.3, velocity: -6)
            point("autopilot")?.tap()
            point("autopilot")?.tap()
            point("journey")?.tap()
            for name in ["Orbit", "Mini-map", "Size scale"] {
                app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", name)).firstMatch.tap()
            }
            canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.4, dy: 0.4))
                .press(forDuration: 0.05, thenDragTo: canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.6, dy: 0.7)))
            point("journey")?.tap()
            settle(1)
            shot("mash-\(round)")
            alive("mashing")
        }
        let f = fields()
        for k in ["re", "im", "scale"] {
            XCTAssertFalse((f[k] ?? "nan").lowercased().contains("nan"), "\(k) is \(f[k] ?? "-")")
        }
        // Whatever state that left: the cockpit's manual icon stops everything.
        if let manual = point("manual") { manual.tap() }
        settle(4)
        XCTAssertEqual(level(), "m")
        XCTAssertEqual(fields()["journey"], "off")
        XCTAssertEqual(fields()["autopilot"], "off")
        shot("mash-stopped")
        // The control center opens and closes.
        app.buttons["Control center"].tap()
        settle(2)
        shot("mash-cc")
    }
}
