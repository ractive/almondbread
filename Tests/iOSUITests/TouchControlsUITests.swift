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

    /// The round buttons over the canvas (kb: decisions/controls "Touch"), in their order; the
    /// iPad's top row also has the Julia panel's (kb: plans/f1-julia-view).
    static var roundButtons: [String] {
        UIDevice.current.userInterfaceIdiom == .pad
            ? ["Orbit", "Mini-map", "Julia set", "Size scale", "Fly to", "Control center"]
            : ["Orbit", "Mini-map", "Size scale", "Fly to", "Control center"]
    }

    /// The toggles among them, with the name of their screenshots.
    static var roundToggles: [(String, String)] {
        UIDevice.current.userInterfaceIdiom == .pad
            ? [("Orbit", "orbit"), ("Mini-map", "map"), ("Julia set", "julia"), ("Size scale", "scale")]
            : [("Orbit", "orbit"), ("Mini-map", "map"), ("Size scale", "scale")]
    }

    private func round(_ name: String) -> XCUIElement {
        app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", name)).firstMatch
    }

    /// The screen as RGBA on a grid of 2 × 2 points, top row first.
    private func pixels() -> (data: [UInt8], w: Int, h: Int) {
        let image = XCUIScreen.main.screenshot().image
        let w = Int(image.size.width / 2), h = Int(image.size.height / 2)
        var data = [UInt8](repeating: 0, count: w * h * 4)
        guard let cg = image.cgImage else { return (data, w, h) }
        data.withUnsafeMutableBytes { buf in
            let ctx = CGContext(data: buf.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                space: CGColorSpaceCreateDeviceRGB(),
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            ctx?.interpolationQuality = .low
            ctx?.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
        }
        return (data, w, h)
    }

    /// The share of the screen (outside `excluding`, in points) whose colour changed visibly.
    private func changed(_ a: (data: [UInt8], w: Int, h: Int), _ b: (data: [UInt8], w: Int, h: Int),
                         excluding: [CGRect]) -> Double {
        guard a.w == b.w, a.h == b.h, a.w > 0 else { return 1 }
        var n = 0, total = 0
        for y in 0..<a.h {
            for x in 0..<a.w {
                let p = CGPoint(x: x * 2 + 1, y: y * 2 + 1)
                if excluding.contains(where: { $0.contains(p) }) { continue }
                total += 1
                let i = (y * a.w + x) * 4
                let d = abs(Int(a.data[i]) - Int(b.data[i])) + abs(Int(a.data[i + 1]) - Int(b.data[i + 1]))
                    + abs(Int(a.data[i + 2]) - Int(b.data[i + 2]))
                if d > 60 { n += 1 }
            }
        }
        return total == 0 ? 0 : Double(n) / Double(total)
    }

    /// The round buttons (the Mac toolbar's items): Orbit, Mini-map and Size scale each switch
    /// their overlay on (the picture changes outside the buttons, the button reads "on") and off
    /// again (the picture is back); Fly to opens its sheet. On iOS the orbit, without a pointer,
    /// is the view centre's (kb: decisions/controls "Touch", "Round buttons").
    func testOverlayToggles() throws {
        try launchSettled()
        settle(2)
        let buttons = Self.roundButtons.map { round($0) }
        for b in buttons {
            XCTAssertTrue(b.exists && b.isHittable, "\(b) missing")
            XCTAssertGreaterThanOrEqual(b.frame.width, 44)
            XCTAssertGreaterThanOrEqual(b.frame.height, 44)
        }
        // The buttons themselves change colour.
        let mask = buttons.map { $0.frame.insetBy(dx: -6, dy: -6) }
        for (name, field) in Self.roundToggles {
            let toggle = round(name)
            XCTAssertEqual(toggle.value as? String, "off", "\(name) starts off")
            let before = pixels()
            toggle.tap()
            settle(2.5)
            XCTAssertEqual(toggle.value as? String, "on", "\(name) did not switch on")
            if field == "orbit" { XCTAssertEqual(fields()["orbit"], "on", "the orbit overlay is off") }
            let on = pixels()
            shot("toggle-\(field)-on")
            let shown = changed(before, on, excluding: mask)
            toggle.tap()
            settle(2.5)
            XCTAssertEqual(toggle.value as? String, "off", "\(name) did not switch off")
            let after = pixels()
            let back = changed(before, after, excluding: mask)
            print("toggle \(name): changed \(String(format: "%.4f", shown)) on, \(String(format: "%.4f", back)) after off")
            XCTAssertGreaterThan(shown, 0.002, "\(name): the picture did not change")
            XCTAssertLessThan(back, shown / 4, "\(name): the overlay stayed after switching off")
        }
        // All of them together, for a look.
        for (name, _) in Self.roundToggles { round(name).tap() }
        settle(3)
        shot("toggle-all-on")
        XCUIDevice.shared.orientation = .landscapeLeft
        settle(3)
        shot("toggle-all-on-landscape")
        XCUIDevice.shared.orientation = .portrait
        settle(2)
        for (name, _) in Self.roundToggles { round(name).tap() }
        // Fly to: the sheet.
        round("Fly to").tap()
        let cancel = app.buttons["Cancel"]
        XCTAssertTrue(cancel.waitForExistence(timeout: 5), "Fly to did not open its sheet")
        shot("toggle-fly-to")
        cancel.tap()
        settle(1)
    }

    /// F4 (kb: plans/f4-deep-julia): the control center's "Julia twin of this view" opens the
    /// Julia set of the view's centre at the same zoom (the HUD names its k, the zoom stays), and
    /// the fractal row's "Back to Mandelbrot" there, or the HUD's back button, returns.
    func testJuliaTwinAndBack() throws {
        try launchSettled()
        let scale0 = scale()
        let control = app.buttons["Control center"]
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        if control.value as? String != "shown" { control.tap() }
        settle(2)
        XCTAssertEqual(control.value as? String, "shown", "the control center did not open")
        // The Overlays section alone (the search filters sections by keyword).
        let search = app.textFields["Search settings"]
        if !search.waitForExistence(timeout: 10) {
            // (Opened before the scene was ready: close and open again.)
            control.tap()
            settle(1.5)
            control.tap()
        }
        XCTAssertTrue(search.waitForExistence(timeout: 10), "no search field")
        search.tap()
        search.typeText("twin\n")
        settle(1.5)
        let twin = app.buttons["julia-twin"]
        for _ in 0..<4 where !(twin.exists && twin.isHittable) {
            app.swipeUp()
            settle(0.8)
        }
        reveal(twin)
        XCTAssertTrue(twin.exists && twin.isHittable, "no Julia twin button in the control center")
        twin.tap()
        settle(2)
        let backHUD = app.buttons["julia-back-hud"]
        XCTAssertTrue(backHUD.waitForExistence(timeout: 5), "the twin did not open (no back button in the HUD)")
        XCTAssertEqual(scale(), scale0, accuracy: scale0 * 1e-9, "the twin changed the zoom")
        let back = app.buttons["fractal-julia-back"]
        XCTAssertTrue(back.waitForExistence(timeout: 5), "no back button in the control center")
        shot("julia-twin-cc")
        back.tap()
        settle(2)
        XCTAssertFalse(app.buttons["julia-back-hud"].exists, "still in the Julia set after Back")
        // Again, and back through the HUD this time.
        let twinAgain = app.buttons["julia-twin"]
        XCTAssertTrue(twinAgain.waitForExistence(timeout: 5), "no twin button after Back")
        reveal(twinAgain)
        twinAgain.tap()
        settle(2)
        let close = app.buttons["control-center-close"]
        if close.exists && close.isHittable { close.tap() }
        settle(2)
        let hudBack = app.buttons["julia-back-hud"]
        XCTAssertTrue(hudBack.waitForExistence(timeout: 5) && hudBack.isHittable, "no hittable back button in the HUD")
        shot("julia-twin-hud")
        hudBack.tap()
        settle(2)
        XCTAssertFalse(app.buttons["julia-back-hud"].exists, "still in the Julia set after the HUD's back")
    }

    /// Makes sure `element` in the control center is clear of the "Reset all…" bar pinned at its
    /// bottom. Rows scroll under that bar and the bar takes the touches there, but XCTest still
    /// calls a row under it hittable: on a phone's half-height sheet the search's last row sits
    /// there (since "Orbit iterations shown" joined Overlays, the Julia twin; Lighting's More),
    /// and a tap on it opened "Reset all settings?" or nothing. The sheet goes to full height
    /// (its grabber), as a user would raise it.
    private func reveal(_ element: XCUIElement) {
        let bar = app.buttons["Reset all…"]
        // (The bar: the button and 8 points of padding above it.)
        let clear = { element.isHittable && (!bar.exists || element.frame.maxY <= bar.frame.minY - 10) }
        guard element.exists, !clear() else { return }
        let grabber = app.buttons["Sheet Grabber"]
        if grabber.exists, (grabber.value as? String)?.contains("Half") == true {
            grabber.tap()
            settle(1)
        }
        XCTAssertTrue(clear(), "\(element.identifier) under the Reset all bar: \(element.frame) vs \(bar.frame)")
    }

    private func launchSettled() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        settle(3)
    }

    /// A section's "More…" (kb: decisions/controls "Essential rows and More"): closed by default
    /// (Lighting shows its three strengths, not Light angle); a tap opens it; a hidden setting
    /// moved shows "1 changed" on the More line, whose ↺ puts it back; a second tap closes it.
    func testMoreOpensAndCounts() throws {
        try launchSettled()
        let control = app.buttons["Control center"]
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        if control.value as? String != "shown" { control.tap() }
        settle(2)
        let search = app.textFields["Search settings"]
        if !search.waitForExistence(timeout: 10) {
            control.tap()
            settle(1.5)
            control.tap()
        }
        XCTAssertTrue(search.waitForExistence(timeout: 10), "no search field")
        search.tap()
        search.typeText("shading\n")  // Lighting alone; an essential row, so its More stays closed
        settle(1.5)
        let more = app.buttons["more-lighting"]
        XCTAssertTrue(more.waitForExistence(timeout: 5), "no More line in Lighting")
        XCTAssertTrue(app.sliders["Shading"].exists, "Shading is not visible")
        XCTAssertFalse(app.sliders["Light angle"].exists, "Light angle shown with More closed")
        XCTAssertFalse((more.value as? String ?? "").contains("changed"), "changed before any change")
        reveal(more)
        shot("more-closed")
        more.tap()
        settle(1)
        let angle = app.sliders["Light angle"]
        XCTAssertTrue(angle.waitForExistence(timeout: 5), "More did not open")
        XCTAssertTrue((more.value as? String ?? "").hasPrefix("expanded"), "value: \(String(describing: more.value))")
        XCTAssertTrue(more.label.hasPrefix("More, 7 "), "label: \(more.label)")
        reveal(angle)
        angle.adjust(toNormalizedSliderPosition: 0.1)
        settle(1)
        XCTAssertTrue((more.value as? String ?? "").contains("1 changed"), "no count: \(String(describing: more.value))")
        shot("more-open-changed")
        let reset = app.buttons["more-lighting-reset"]
        XCTAssertTrue(reset.exists, "no reset next to the count")
        reset.tap()
        settle(1)
        XCTAssertFalse((more.value as? String ?? "").contains("changed"), "the ↺ left a change")
        more.tap()
        settle(1)
        XCTAssertFalse(app.sliders["Light angle"].exists, "More did not close")
        // A search for a hidden row opens its More.
        search.tap()
        search.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 7) + "highlight\n")
        settle(1.5)
        XCTAssertTrue(app.sliders["Highlight"].waitForExistence(timeout: 5), "the search did not open More")
        // A More the search opened closes on a tap during the search (search-local).
        reveal(more)
        more.tap()
        settle(1)
        XCTAssertFalse(app.sliders["Highlight"].exists, "a tap did not close the More the search opened")
        XCTAssertTrue((more.value as? String ?? "").hasPrefix("collapsed"), "value: \(String(describing: more.value))")
    }

    /// "Orbit iterations shown" (kb: decisions/controls "Disabled, not hidden"): a long press on
    /// the round Orbit button offers the dot counts and leaves the orbit off; the control center
    /// shows the row (found by "dots") disabled with the count picked, enabled once the orbit is on.
    func testOrbitDotsMenu() throws {
        try launchSettled()
        let orbit = round("Orbit")
        XCTAssertTrue(orbit.waitForExistence(timeout: 5))
        XCTAssertEqual(orbit.value as? String, "off")
        orbit.press(forDuration: 1.0)
        let choice = app.buttons["200 dots"]
        XCTAssertTrue(choice.waitForExistence(timeout: 5), "no long-press menu on the Orbit button")
        shot("orbit-dots-menu")
        choice.tap()
        settle(1)
        XCTAssertEqual(orbit.value as? String, "off", "the long press toggled the orbit")
        let control = app.buttons["Control center"]
        if control.value as? String != "shown" { control.tap() }
        settle(2)
        let search = app.textFields["Search settings"]
        XCTAssertTrue(search.waitForExistence(timeout: 10), "no search field")
        search.tap()
        search.typeText("dots\n")
        settle(1.5)
        let slider = app.sliders["Orbit iterations shown"]
        XCTAssertTrue(slider.waitForExistence(timeout: 5), "the search did not find the orbit's dot count")
        XCTAssertEqual(slider.value as? String, "200 dots")
        XCTAssertFalse(slider.isEnabled, "enabled with the orbit off")
        shot("orbit-dots-disabled")
        // The switch itself, the row's trailing child: a tap on the row's middle hits the label,
        // and a fixed fraction of the row's width missed the switch on the iPad's narrower
        // column (0.95 of a 258-point row lands a point past its trailing edge).
        let row = app.switches["Orbit of the point under the pointer"].firstMatch
        let knob = row.switches.firstMatch
        XCTAssertTrue(knob.exists, "no switch in the Orbit row")
        knob.tap()
        settle(1.5)
        shot("orbit-dots-enabled")
        XCTAssertEqual(fields()["orbit"], "on", "the switch did not turn the orbit on")
        XCTAssertTrue(slider.isEnabled, "still disabled with the orbit on")
    }

    /// The control center's ✕ and the round button's open state: open, the round button reads
    /// selected ("shown"); the ✕ at the top of the pane is hittable, clear of the search field,
    /// and stays put when the content scrolls (and, on a phone, at the large detent); a tap on it
    /// closes the pane and the round button is plain again.
    func testControlCenterClose() throws {
        try launchSettled()
        let isPhone = UIDevice.current.userInterfaceIdiom == .phone
        let control = app.buttons["Control center"]
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        XCTAssertEqual(control.value as? String, "hidden")
        XCTAssertFalse(control.isSelected, "the round button reads selected with the pane closed")
        control.tap()
        settle(2)
        XCTAssertEqual(control.value as? String, "shown")
        XCTAssertTrue(control.isSelected, "the round button does not read selected with the pane open")
        let close = app.buttons["control-center-close"]
        XCTAssertTrue(close.waitForExistence(timeout: 5), "no ✕ in the control center")
        XCTAssertEqual(close.label, "Close control center")
        XCTAssertTrue(close.isHittable, "✕ not hittable")
        // A 44-point target; the phone's medium-detent sheet is drawn scaled (42 on screen).
        XCTAssertGreaterThanOrEqual(close.frame.width, 40)
        XCTAssertGreaterThanOrEqual(close.frame.height, 40)
        let search = app.textFields["Search settings"]
        XCTAssertTrue(search.exists, "no search field")
        XCTAssertFalse(close.frame.intersects(search.frame), "✕ over the search field")
        XCTAssertLessThanOrEqual(close.frame.maxX, app.windows.firstMatch.frame.maxX, "✕ off screen")
        shot("cc-close-open")
        // Scroll the content (a phone: the first swipe raises the sheet to its large detent).
        app.swipeUp()
        settle(1)
        app.swipeUp()
        settle(1.5)
        shot("cc-close-scrolled")
        XCTAssertTrue(close.isHittable, "✕ scrolled away")
        if isPhone {
            XCTAssertGreaterThanOrEqual(close.frame.minY, app.windows.firstMatch.frame.minY, "✕ above the screen")
        }
        close.tap()
        settle(2)
        shot("cc-close-closed")
        XCTAssertEqual(control.value as? String, "hidden", "the ✕ did not close the control center")
        XCTAssertFalse(control.isSelected, "the round button still reads selected after the ✕")
        XCTAssertFalse(app.buttons["control-center-close"].exists, "✕ still there after closing")
        XCTAssertFalse(app.textFields["Search settings"].exists, "the pane is still there after the ✕")
    }

    /// The round button's state matches the pane after a relaunch: the app is killed with the
    /// control center open (as iOS ends a backgrounded app) and launched again. iOS restores the
    /// open inspector, which sometimes never came up while the button read "shown"; the control
    /// center starts closed (button "hidden", no pane) and one tap opens it.
    func testControlCenterAfterRelaunch() throws {
        try launchSettled()
        let control = app.buttons["Control center"]
        let close = app.buttons["control-center-close"]
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        if control.value as? String != "shown" { control.tap() }
        settle(2)
        XCTAssertTrue(close.waitForExistence(timeout: 5), "control center not open")
        XCUIDevice.shared.press(.home)
        settle(2)
        app.terminate()
        try launchSettled()
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        let shown = control.value as? String == "shown"
        let paneThere = close.waitForExistence(timeout: 3)
        shot("cc-relaunch")
        if shown || paneThere { dumpTree("cc-relaunch") }
        XCTAssertEqual(shown, paneThere, "the round button reads \(shown ? "shown" : "hidden") but the pane is \(paneThere ? "up" : "not up") after a relaunch")
        // The control center starts closed (iOS's restore of the open pane is not taken).
        XCTAssertFalse(shown, "the round button reads shown after a relaunch")
        XCTAssertFalse(control.isSelected, "the round button reads selected after a relaunch")
        XCTAssertFalse(paneThere, "the pane is up after a relaunch")
        // One tap opens it.
        control.tap()
        settle(2)
        XCTAssertTrue(close.waitForExistence(timeout: 5), "the first tap did not open the control center")
        XCTAssertEqual(control.value as? String, "shown")
        XCTAssertTrue(app.textFields["Search settings"].waitForExistence(timeout: 5), "no search field")
        for _ in 0..<3 where control.value as? String == "shown" {
            close.tap()
            settle(1)
        }
    }

    /// The Fractal picker (F3, kb: plans/f3-multibrot): Multibrot goes to its whole set (centred on
    /// 0) as z³ + c, the power picker switches to z⁵ + c, and Mandelbrot comes back to its home
    /// (centre -0.75). The canvas value names the formula.
    func testFormulaPicker() throws {
        try launchSettled()
        let control = app.buttons["Control center"]
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        // (The control center starts closed, whatever the last launch left.)
        XCTAssertEqual(control.value as? String, "hidden", "the control center reads open at launch")
        control.tap()
        settle(2)
        XCTAssertTrue(app.buttons["control-center-close"].waitForExistence(timeout: 5), "control center not open")
        shot("formula-cc-open")
        let search = app.textFields["Search settings"]
        XCTAssertTrue(search.waitForExistence(timeout: 5))
        search.tap()
        search.typeText("fractal")
        settle(1.5)
        let family = app.buttons["formula-family"]
        XCTAssertTrue(family.waitForExistence(timeout: 5), "no Fractal picker")
        family.tap()
        settle(1)
        app.buttons["Multibrot"].firstMatch.tap()
        settle(2.5)
        shot("formula-multibrot3")
        XCTAssertEqual(fields()["formula"], "multibrot3")
        XCTAssertEqual(Double(fields()["re"] ?? "") ?? .nan, 0, accuracy: 1e-9, "not at the Multibrot set's home")
        let power = app.buttons["formula-power"]
        XCTAssertTrue(power.waitForExistence(timeout: 5), "no power picker")
        power.tap()
        settle(1)
        app.buttons["z⁵ + c"].firstMatch.tap()
        settle(2.5)
        shot("formula-multibrot5")
        XCTAssertEqual(fields()["formula"], "multibrot5")
        // F5: the Mandelbar set (Tricorn), a family of one: no power picker, its home at -1/2.
        family.tap()
        settle(1)
        app.buttons["Mandelbar"].firstMatch.tap()
        settle(2.5)
        shot("formula-mandelbar")
        XCTAssertEqual(fields()["formula"], "mandelbar")
        XCTAssertEqual(Double(fields()["re"] ?? "") ?? .nan, -0.5, accuracy: 1e-9, "not at the Mandelbar set's home")
        XCTAssertFalse(app.buttons["formula-power"].exists, "a power picker for Mandelbar")
        // F6: the Burning Ship, a family of one, its home at -0.45 (shown upright).
        family.tap()
        settle(1)
        app.buttons["Burning Ship"].firstMatch.tap()
        settle(2.5)
        shot("formula-burningship")
        XCTAssertEqual(fields()["formula"], "burningship")
        XCTAssertEqual(Double(fields()["re"] ?? "") ?? .nan, -0.45, accuracy: 1e-9, "not at the Burning Ship's home")
        XCTAssertFalse(app.buttons["formula-power"].exists, "a power picker for the Burning Ship")
        family.tap()
        settle(1)
        app.buttons["Mandelbrot"].firstMatch.tap()
        settle(2.5)
        XCTAssertEqual(fields()["formula"], "mandelbrot")
        XCTAssertEqual(Double(fields()["re"] ?? "") ?? .nan, -0.75, accuracy: 1e-9, "not at the Mandelbrot home")
        XCTAssertFalse(app.buttons["formula-power"].exists, "the power picker stays for Mandelbrot")
        // The ✕ closes the pane (the first tap may only end the search field's editing).
        for _ in 0..<3 where control.value as? String == "shown" {
            app.buttons["control-center-close"].tap()
            settle(1)
        }
        XCTAssertEqual(control.value as? String, "hidden", "the control center stayed open")
    }

    /// The orbit-density looks (BB, kb: plans/buddhabrot): the Look picker offers Buddhabrot and
    /// Nebulabrot for the Mandelbrot set and applies them (the canvas value names the look);
    /// for a Multibrot set the entries are hidden. Back to Aurora at the end.
    func testBuddhabrotLook() throws {
        try launchSettled()
        let control = app.buttons["Control center"]
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        // (The control center starts closed, whatever the last launch left.)
        XCTAssertEqual(control.value as? String, "hidden", "the control center reads open at launch")
        control.tap()
        settle(2)
        if !app.buttons["control-center-close"].waitForExistence(timeout: 5) { dumpTree("control-center") }
        XCTAssertTrue(app.buttons["control-center-close"].exists, "control center not open")
        let search = app.textFields["Search settings"]
        XCTAssertTrue(search.waitForExistence(timeout: 5))
        func openLookPicker() {
            search.tap()
            search.clearAndType("look")
            settle(1.5)
            let picker = app.buttons["look-picker"]
            XCTAssertTrue(picker.waitForExistence(timeout: 5), "no Look picker")
            picker.tap()
            settle(1)
        }
        func entry(_ name: String) -> XCUIElement {
            app.buttons.matching(NSPredicate(format: "label CONTAINS %@", name)).firstMatch
        }
        openLookPicker()
        if !entry("Nebulabrot").waitForExistence(timeout: 5) { dumpTree("look-picker") }
        XCTAssertTrue(entry("Nebulabrot").exists, "no Nebulabrot entry")
        let buddha = entry("Buddhabrot")
        XCTAssertTrue(buddha.exists, "no Buddhabrot entry")
        buddha.tap()
        settle(4)
        shot("look-buddhabrot")
        XCTAssertEqual(fields()["buddha"], "buddhabrot")
        // Another formula: the orbit-density looks are not offered.
        search.tap()
        search.clearAndType("fractal")
        settle(1.5)
        let family = app.buttons["formula-family"]
        XCTAssertTrue(family.waitForExistence(timeout: 5), "no Fractal picker")
        family.tap()
        settle(1)
        app.buttons["Multibrot"].firstMatch.tap()
        settle(2.5)
        openLookPicker()
        XCTAssertFalse(entry("Buddhabrot").exists, "Buddhabrot offered for a Multibrot set")
        let aurora = entry("Aurora")
        XCTAssertTrue(aurora.waitForExistence(timeout: 5), "no Aurora entry")
        aurora.tap()
        settle(1.5)
        XCTAssertEqual(fields()["buddha"], "off")
        search.tap()
        search.clearAndType("fractal")
        settle(1.5)
        family.tap()
        settle(1)
        app.buttons["Mandelbrot"].firstMatch.tap()
        settle(2.5)
        XCTAssertEqual(fields()["formula"], "mandelbrot")
        for _ in 0..<3 where control.value as? String == "shown" {
            app.buttons["control-center-close"].tap()
            settle(1)
        }
    }

    /// The accessibility tree as an attachment (a failing UI query's evidence).
    private func dumpTree(_ name: String) {
        let a = XCTAttachment(string: app.debugDescription)
        a.name = "tree-" + name
        a.lifetime = .keepAlways
        add(a)
    }

    /// The Julia panel's round toggle (iPad) and the control center's Julia twin follow Rust's
    /// capabilities (kb: decisions/controls "Fractal picker and Julia views"): shown in
    /// Mandelbrot, hidden in a Multibrot view; a Julia view keeps the toggle (its panel shows
    /// the Mandelbrot view) and has the way back in the twin's place.
    func testJuliaControlsFollowFormula() throws {
        try launchSettled()
        let pad = UIDevice.current.userInterfaceIdiom == .pad
        let control = app.buttons["Control center"]
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        let search = app.textFields["Search settings"]
        // (The round buttons are checked with the pane closed, as a user sees them.)
        func closePane() {
            for _ in 0..<3 where control.value as? String == "shown" {
                app.buttons["control-center-close"].tap()
                settle(1)
            }
            XCTAssertEqual(control.value as? String, "hidden", "the control center stayed open")
        }
        func find(_ text: String) {
            if control.value as? String != "shown" { control.tap() }
            settle(2)
            if !search.waitForExistence(timeout: 10) {
                control.tap()
                settle(1.5)
                control.tap()
            }
            XCTAssertTrue(search.waitForExistence(timeout: 10), "no search field")
            search.clearAndType(text + "\n")
            settle(1.5)
        }
        func pick(_ family: String) {
            find("fractal")
            let picker = app.buttons["formula-family"]
            XCTAssertTrue(picker.waitForExistence(timeout: 5), "no Fractal picker")
            picker.tap()
            settle(1)
            app.buttons[family].firstMatch.tap()
            settle(2.5)
        }
        let toggle = { self.round("Julia set") }
        // Mandelbrot: the round button and the twin.
        settle(1)
        if pad { XCTAssertTrue(toggle().exists, "no Julia toggle in Mandelbrot") }
        find("twin")
        XCTAssertTrue(app.buttons["julia-twin"].waitForExistence(timeout: 5), "no Julia twin in Mandelbrot")
        // Multibrot: neither (nor the control center's panel switch).
        pick("Multibrot")
        XCTAssertEqual(fields()["formula"], "multibrot3")
        find("twin")
        XCTAssertFalse(app.buttons["julia-twin"].exists, "Julia twin shown in Multibrot")
        find("julia")
        XCTAssertFalse(app.switches["Julia set of that point"].exists, "Julia panel switch shown in Multibrot")
        closePane()
        shot("julia-controls-multibrot")
        if pad { XCTAssertFalse(toggle().exists, "Julia toggle shown in Multibrot") }
        // Back to Mandelbrot: both again.
        pick("Mandelbrot")
        XCTAssertEqual(fields()["formula"], "mandelbrot")
        find("twin")
        let twin = app.buttons["julia-twin"]
        for _ in 0..<4 where !(twin.exists && twin.isHittable) {
            app.swipeUp()
            settle(0.8)
        }
        reveal(twin)
        XCTAssertTrue(twin.exists && twin.isHittable, "no Julia twin back in Mandelbrot")
        // The twin (a Julia view): the round button stays (the Mandelbrot panel's switch there),
        // Back takes the twin's place.
        twin.tap()
        settle(2)
        XCTAssertTrue(app.buttons["fractal-julia-back"].waitForExistence(timeout: 5), "the twin did not open")
        XCTAssertFalse(app.buttons["julia-twin"].exists, "Julia twin shown in a Julia view")
        closePane()
        shot("julia-controls-julia")
        if pad { XCTAssertTrue(toggle().exists, "the panel's toggle left a Julia view") }
        let hudBack = app.buttons["julia-back-hud"]
        XCTAssertTrue(hudBack.waitForExistence(timeout: 5), "no back button in the HUD")
        hudBack.tap()
        settle(2)
        XCTAssertEqual(fields()["formula"], "mandelbrot")
        if pad { XCTAssertTrue(toggle().exists, "no Julia toggle after Back") }
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
        // In the autopilot (the cockpit's plane): the knob shows its speed's gear and sets it.
        try XCTUnwrap(point("autopilot")).tap()
        settle(1)
        XCTAssertEqual(level(), "a")
        let g = gear()
        XCTAssertGreaterThan(g, 0, "the autopilot's gear")
        XCTAssertEqual(lever.gear, g, "the knob did not follow the autopilot")
        lever.tapChevron(up: true)
        settle(0.5)
        XCTAssertEqual(level(), "a", "the throttle left the autopilot")
        XCTAssertEqual(gear(), g + 1, "the autopilot's speed did not step")
        // A speed change elsewhere (the hardware keyboard's E) moves the knob.
        app.typeKey("e", modifierFlags: [])
        settle(1)
        XCTAssertEqual(gear(), g + 2, "E did not step the autopilot")
        XCTAssertEqual(lever.gear, g + 2, "the knob did not follow")
        shot("throttle-autopilot")
        try XCTUnwrap(point("autopilot")).tap()
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
        let steps = try tourStepCount()
        shot("tour-1")
        next.tap()
        settle(2)
        XCTAssertTrue(app.staticTexts["2 of \(steps)"].waitForExistence(timeout: 5), "Next did not step")
        app.buttons["‹ Back"].tap()
        settle(2)
        XCTAssertTrue(app.staticTexts["1 of \(steps)"].waitForExistence(timeout: 5), "Back did not step")
        shot("tour-back")
        app.buttons["End tour"].tap()
        settle(1)
        XCTAssertFalse(next.exists, "End tour did not end it")
    }

    /// The open tour's step count, read off the card's "1 of N" (tours change length as they
    /// are rewritten; these tests step a tour, they do not pin its length).
    private func tourStepCount() throws -> Int {
        let first = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH '1 of '")).firstMatch
        XCTAssertTrue(first.waitForExistence(timeout: 5), "the card does not read 1 of N")
        return try XCTUnwrap(Int(first.label.dropFirst("1 of ".count)), "step counter: \(first.label)")
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
        for name in Self.roundButtons {
            let b = round(name)
            XCTAssertTrue(b.isHittable, "\(name) not hittable in landscape")
            XCTAssertFalse(b.frame.intersects(lever.frame), "\(name) under the throttle")
        }
        for cell in ["manual", "cruise", "autopilot", "journey", "lane"] {
            guard let c = point(cell) else { continue }
            let p = c.screenPoint
            XCTAssertFalse(lever.frame.contains(p), "cockpit \(cell) under the throttle")
            for name in Self.roundButtons {
                XCTAssertFalse(round(name).frame.insetBy(dx: -4, dy: -4).contains(p), "cockpit \(cell) under \(name)")
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

    /// Rest or swipe (kb: decisions/controls "Touch", "Rest or swipe"): while something flies,
    /// a finger that rests goes there and a finger that swipes moves the picture. A: a held
    /// finger is the wish (no menu, A stays, its line goes after the lift); a quick swipe moves
    /// the picture and leaves no wish line, A stays. C: a held finger steers, also when it then
    /// slides (the picture is not dragged: it turns towards the finger); a quick swipe pans with
    /// the finger, the gear stays. M: the long press is still the menu.
    func testRestSteersSwipePans() throws {
        try launchSettled()
        let num = { (f: [String: String], k: String) in Double(f[k] ?? "") ?? .nan }
        let rest = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.8, dy: 0.4))
        let away = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.85, dy: 0.3))
        let from = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.6))
        let to = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.7, dy: 0.4))
        try XCTUnwrap(point("autopilot")).tap()
        settle(1)
        XCTAssertEqual(level(), "a")
        // A: at rest 1 s, then a slide: no menu, still A (a touch never hands over in A).
        rest.press(forDuration: 1.0, thenDragTo: away, withVelocity: 120, thenHoldForDuration: 0.5)
        let justLifted = fields()["wish"]
        shot("finger-autopilot-lifted")
        settle(0.5)
        XCTAssertFalse(menuShown, "a resting finger opened the menu in A")
        XCTAssertEqual(level(), "a")
        // The wish line goes after the finger lifted (device round: it stayed until A ended).
        print("UITEST wish right after the lift: \(justLifted ?? "-")")
        settle(2)
        XCTAssertEqual(fields()["wish"], "off", "the wish line stayed after the finger lifted")
        shot("finger-autopilot-line-gone")
        // A: a quick swipe moves the picture with the finger (right and up: the centre moves
        // left and down), draws no wish line and stays in A.
        let a0 = fields()
        from.press(forDuration: 0.05, thenDragTo: to, withVelocity: 300, thenHoldForDuration: 0.2)
        let a1 = fields()
        shot("finger-autopilot-swiped")
        let are = (num(a0, "re") - num(a1, "re")) / num(a1, "scale")
        let aim = (num(a0, "im") - num(a1, "im")) / num(a1, "scale")
        print("UITEST autopilot swipe moved the centre by \(are), \(aim) points (left, down)")
        XCTAssertGreaterThan(are, 0.2 * canvas.frame.width * 0.4, "a swipe in A did not move the picture")
        XCTAssertGreaterThan(aim, 0.2 * canvas.frame.height * 0.2, "a swipe in A did not move the picture")
        XCTAssertEqual(a1["wish"], "off", "a swipe in A drew a wish line")
        XCTAssertEqual(level(), "a", "a swipe left the autopilot")
        settle(1)
        XCTAssertEqual(fields()["wish"], "off", "a swipe in A drew a wish line")
        XCTAssertEqual(level(), "a", "a swipe left the autopilot")
        try XCTUnwrap(point("autopilot")).tap()
        settle(2)
        // C (the throttle on its first tick).
        let lever = app.descendants(matching: .any)["throttle"]
        throttle(lever, tick: 1)
        settle(0.5)
        XCTAssertEqual(level(), "c")
        // C: a quick swipe pans at once, the content following the finger; the gear stays.
        let f0 = fields()
        from.press(forDuration: 0.05, thenDragTo: to, withVelocity: 300, thenHoldForDuration: 0.3)
        let f1 = fields()
        shot("finger-cruise-dragged")
        let dre = (num(f0, "re") - num(f1, "re")) / num(f1, "scale")
        let dim = (num(f0, "im") - num(f1, "im")) / num(f1, "scale")
        print("UITEST cruise swipe moved the centre by \(dre), \(dim) points (left, down)")
        XCTAssertGreaterThan(dre, 0.2 * canvas.frame.width * 0.4, "a swipe in C did not pan with the finger")
        XCTAssertGreaterThan(dim, 0.2 * canvas.frame.height * 0.2, "a swipe in C did not pan with the finger")
        XCTAssertEqual(level(), "c", "a swipe left the cruise")
        XCTAssertEqual(gear(), 1, "a swipe changed the gear")
        // C: held 1 s right of the centre, then slid further right and held: it steers towards
        // the finger, so the centre moves right (a pan would have moved it left by the slide,
        // 15 % of the width).
        let steerAt = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: 0.5))
        let steerOn = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5))
        let r0 = fields()
        steerAt.press(forDuration: 1.0, thenDragTo: steerOn, withVelocity: 200, thenHoldForDuration: 1.0)
        let r1 = fields()
        shot("finger-cruise-steered")
        let steered = (num(r1, "re") - num(r0, "re")) / num(r1, "scale")
        print("UITEST cruise hold moved the centre by \(steered) points (right)")
        XCTAssertGreaterThan(steered, 0, "a held finger in C did not steer towards it (or it panned)")
        settle(0.3)
        XCTAssertFalse(menuShown, "a resting finger opened the menu in C")
        XCTAssertEqual(level(), "c")
        XCTAssertEqual(gear(), 1, "steering changed the gear")
        lever.doubleTap()
        settle(4)
        XCTAssertEqual(level(), "m")
        // M: the long press is the menu.
        rest.press(forDuration: 0.8)
        XCTAssertTrue(app.descendants(matching: .any)["Fly Here"].firstMatch.waitForExistence(timeout: 5),
                      "no menu in M")
    }

    /// After a pinch every finger is up again (`TouchesLifted`): a drag pans, in M and in C. (The
    /// device bug, a finger left down after the other lifted panning the view, needs fingers
    /// lifting one at a time, which XCUITest cannot synthesise; Rust's
    /// `a_finger_left_after_a_pinch_does_not_pan` covers the events.) The pinch itself zooms
    /// about its centre and does not carry the view off.
    func testPinchThenDragPans() throws {
        try launchSettled()
        let f0 = fields()
        canvas.pinch(withScale: 3, velocity: 3)
        settle(1.5)
        let f1 = fields()
        let s1 = Double(f1["scale"] ?? "") ?? .nan
        XCTAssertLessThan(s1, Double(f0["scale"] ?? "") ?? .nan, "the pinch did not zoom")
        let drift = abs(((Double(f0["re"] ?? "") ?? .nan) - (Double(f1["re"] ?? "") ?? .nan)) / s1)
        print("UITEST pinch moved the centre by \(drift) points")
        XCTAssertLessThan(drift, 40, "the pinch carried the view off")
        let from = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.5))
        let to = canvas.coordinate(withNormalizedOffset: CGVector(dx: 0.7, dy: 0.5))
        from.press(forDuration: 0.05, thenDragTo: to)
        settle(1)
        let f2 = fields()
        let dre = ((Double(f1["re"] ?? "") ?? .nan) - (Double(f2["re"] ?? "") ?? .nan)) / s1
        XCTAssertGreaterThan(dre, 0.4 * canvas.frame.width * 0.5, "a drag after a pinch did not pan")
        shot("pinch-then-drag")
        // In C: the same.
        let lever = app.descendants(matching: .any)["throttle"]
        throttle(lever, tick: 1)
        settle(0.5)
        canvas.pinch(withScale: 2, velocity: 3)
        settle(0.5)
        // A manual zoom ends the cruise (kb: decisions/controls "Driving levels"): back into C.
        if level() != "c" { throttle(lever, tick: 1); settle(0.5) }
        XCTAssertEqual(level(), "c")
        let g0 = fields()
        from.press(forDuration: 0.05, thenDragTo: to)
        let g1 = fields()
        XCTAssertNotEqual(g0["re"], g1["re"], "a drag after a pinch did not pan in C")
        lever.doubleTap()
        settle(3)
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
        let steps = try tourStepCount()
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
        XCTAssertTrue(app.staticTexts["2 of \(steps)"].waitForExistence(timeout: 5), "Next did not step while folded")
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

    /// The mini-map's arrows button below its (?): a tap makes the map large, a fingertip off its
    /// centre makes it small again; neither moves the view.
    func testMiniMapSizeButton() throws {
        try launchSettled()
        app.typeKey("m", modifierFlags: [])
        settle(2)
        let before = fields()
        let width = { (f: [String: String]) -> Int? in
            let r = (f["panel-maps"] ?? "").split(separator: ",").compactMap { Int($0) }
            return r.count == 4 ? r[2] - r[0] : nil
        }
        try XCTUnwrap(point("help-map-enlarge"), "no enlarge button: \(before)").tap()
        settle(2)
        let large = fields()
        shot("map-large")
        XCTAssertNotNil(large["help-map-shrink"], "the tap did not make the map large: \(large)")
        XCTAssertGreaterThan(width(large) ?? 0, width(before) ?? .max, "the map row did not grow")
        XCTAssertEqual(before["re"], large["re"], "the button moved the view")
        XCTAssertEqual(before["im"], large["im"], "the button moved the view")
        try XCTUnwrap(point("help-map-shrink")).withOffset(CGVector(dx: 10, dy: 10)).tap()
        settle(2)
        let small = fields()
        shot("map-small-again")
        XCTAssertNotNil(small["help-map-enlarge"], "the second tap did not make the map small: \(small)")
        XCTAssertEqual(width(small), width(before))
        XCTAssertEqual(before["re"], small["re"], "the button moved the view")
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
        // A normal launch (no test environment): the user's settings, orbit off. (Without
        // ALMONDBREAD_UITEST the canvas's value is VoiceOver's summary, so read the round toggle.)
        let normal = XCUIApplication()
        normal.launch()
        XCTAssertTrue(normal.otherElements["fractal"].waitForExistence(timeout: 30))
        settle(3)
        let orbit = normal.descendants(matching: .any).matching(NSPredicate(format: "label == %@", "Orbit")).firstMatch
        XCTAssertTrue(orbit.waitForExistence(timeout: 5), "no Orbit toggle")
        XCTAssertEqual(orbit.value as? String, "off", "the test's Orbit Here persisted")
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
        try XCTUnwrap(point("journey"), "no cockpit positions").tap()
        let deeper = app.buttons["Journey: deeper here"]
        let stopButtons = ["Journey: next landmark", "Journey: deeper here", "Journey: somewhere else"]
        /// The stop's choices: finger-sized, hittable, clear of the round buttons and each other.
        func checkStop(_ when: String) {
            for name in stopButtons {
                let b = app.buttons[name]
                XCTAssertTrue(b.isHittable, "\(name) not hittable at a stop (\(when))")
                XCTAssertGreaterThanOrEqual(b.frame.width, 44)
                XCTAssertGreaterThanOrEqual(b.frame.height, 44)
                for other in Self.roundButtons {
                    XCTAssertFalse(b.frame.intersects(round(other).frame), "\(name) over \(other) (\(when))")
                }
            }
            for name in Self.roundButtons {
                XCTAssertTrue(round(name).isHittable, "\(name) not hittable at a stop (\(when))")
            }
        }
        if deeper.waitForExistence(timeout: 90) {
            settle(1)
            shot("surface-journey-stop")
            checkStop("portrait")
            XCUIDevice.shared.orientation = .landscapeLeft
            settle(2)
            shot("surface-journey-stop-landscape")
            checkStop("landscape")
            XCUIDevice.shared.orientation = .portrait
            settle(1)
        } else {
            shot("surface-journey-no-stop")
            XCTFail("no journey stop within 90 s")
        }
    }
}

private extension XCUIElement {
    /// Replaces a search field's text (focuses it, deletes what is there, then types).
    func clearAndType(_ text: String) {
        tap()
        if let old = value as? String, !old.isEmpty, old != placeholderValue, old != "Search settings" {
            typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: old.count))
        }
        typeText(text)
    }
}
