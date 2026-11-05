import XCTest

/// The parameter lab (F10, kb: plans/f10-parameter-lab) on iPhone and iPad simulators through
/// real touches: the formula picker's "Parameter lab" enters it (the canvas value says
/// "lab c"), its panels (one Rust rect "panel-lab", the row) keep clear of the round buttons
/// and the HUD, a tap on a panel's "show" button makes that plane the main view, a drag on a
/// panel moves its knob ("labc re,im"), and the picker's Mandelbrot leaves it for the view it was
/// entered from. The lab's controls can be touched, not only found: the iPad's in the pinned area
/// without scrolling, the phone's in the list, and what is pinned leaves the search, the close
/// button and the bottom bar on the sheet. Screenshots `lab-*` as in `TouchUITests`, the controls'
/// frames in `<idiom>-lab-frames.txt` beside them.
final class ParameterLabUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        app = XCUIApplication()
        app.launchEnvironment["ALMONDBREAD_UITEST"] = "1"
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }
    private func settle(_ s: Double) { Thread.sleep(forTimeInterval: s) }

    /// The view's value as key-value pairs ("formula" → "mandelbrot", "lab" → "c").
    private func fields() -> [String: String] {
        guard let v = canvas.value as? String else { return [:] }
        let f = v.split(separator: " ").map(String.init)
        guard f.count >= 8 else { return [:] }
        var d = ["re": f[0], "im": f[1], "scale": f[2]]
        var i = 4
        while i + 1 < f.count {
            if f[i] == "cockpit" { break }
            d[f[i]] = f[i + 1]
            i += 2
        }
        return d
    }

    /// The Rust panels in canvas points (" panel-<id>:x0,y0,x1,y1").
    private func panels() -> [String: CGRect] {
        guard let v = canvas.value as? String else { return [:] }
        var out: [String: CGRect] = [:]
        for t in v.split(separator: " ") where t.hasPrefix("panel-") {
            let kv = t.dropFirst("panel-".count).split(separator: ":", maxSplits: 1)
            guard kv.count == 2 else { continue }
            let n = kv[1].split(separator: ",").compactMap { Double($0) }
            guard n.count == 4 else { continue }
            out[String(kv[0])] = CGRect(x: n[0], y: n[1], width: n[2] - n[0], height: n[3] - n[1])
        }
        return out
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

    /// A point of the canvas in its own points from the top left.
    private func at(_ x: Double, _ y: Double) -> XCUICoordinate {
        canvas.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y))
    }

    /// The row's panels from its rect: each panel's rect, left to right (panels are 0.72 as
    /// high as wide, 8 points apart; the row may hold fewer than three on a phone).
    private func rowPanels(_ row: CGRect) -> [CGRect] {
        let pw = row.height / 0.72
        let n = max(1, Int(((row.width + 8) / (pw + 8)).rounded()))
        let w = (row.width - 8 * Double(n - 1)) / Double(n)
        return (0..<n).map { CGRect(x: row.minX + Double($0) * (w + 8), y: row.minY, width: w, height: row.height) }
    }

    /// The round buttons over the canvas (canvas points), as far as they exist.
    private func roundButtons() -> [String: CGRect] {
        let origin = canvas.frame.origin
        var out: [String: CGRect] = [:]
        for name in ["Orbit", "Mini-map", "Julia set", "Size scale", "Fly to", "Control center"] {
            let b = app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", name)).firstMatch
            if b.exists, b.frame.width > 0 { out[name] = b.frame.offsetBy(dx: -origin.x, dy: -origin.y) }
        }
        let hud = app.descendants(matching: .any)["hud"].firstMatch
        if hud.exists { out["hud"] = hud.frame.offsetBy(dx: -origin.x, dy: -origin.y) }
        return out
    }

    private func control(_ id: String) -> XCUIElement { app.descendants(matching: .any)[id].firstMatch }

    /// The lab's controls from the top down.
    private static let labControls = ["lab-z0-re", "lab-z0-im", "lab-z0-re-field", "lab-z0-im-field",
                                      "lab-p-re", "lab-p-im", "lab-p-re-field", "lab-p-im-field",
                                      "lab-c-re", "lab-c-im", "lab-c-re-field", "lab-c-im-field",
                                      "lab-random", "lab-reset"]

    /// Drags the control center's list at the rows' left end (the sliders' thumbs are further in).
    /// It holds at the end: without a fling the list moves by the drag, not past the row looked for.
    private func dragList(up: Bool) {
        let list = app.collectionViews.firstMatch
        let (from, to) = up ? (0.8, 0.5) : (0.5, 0.8)
        list.coordinate(withNormalizedOffset: CGVector(dx: 0.12, dy: from))
            .press(forDuration: 0.05, thenDragTo: list.coordinate(withNormalizedOffset: CGVector(dx: 0.12, dy: to)),
                   withVelocity: 300, thenHoldForDuration: 0.2)
        settle(0.7)
    }

    /// The phone's sheet at full height (its grabber), as a user raises it: a half sheet takes a
    /// drag for itself and falls back, the list only scrolls once it is up. No-op in landscape.
    private func raiseSheet() {
        let grabber = app.buttons["Sheet Grabber"]
        if grabber.exists, (grabber.value as? String)?.contains("Half") == true {
            grabber.tap()
            settle(1)
        }
    }

    /// Scrolls the list until `id` is between the pinned rows (asking an element clipped by the
    /// list for `isHittable` fails the test by itself).
    private func reach(_ id: String) -> XCUIElement {
        var e = control(id)
        for _ in 0..<6 {
            let top = control("lab-plane").frame.maxY, bottom = app.buttons["Reset all…"].frame.minY
            if e.exists, e.frame.midY > top, e.frame.midY < bottom { break }
            dragList(up: !e.exists || e.frame.midY >= bottom)
            e = control(id)
        }
        return e
    }

    /// Each control of the lab can be touched: on the iPad at once (pinned), on the phone the
    /// plane picker at once and the rest by scrolling the list to it. All of them pinned, a phone's
    /// sheet showed them but lost the search and the close button above and the list below.
    private func checkControlsCanBeTouched(_ phone: Bool) {
        for e in [app.textFields["Search settings"], app.buttons["control-center-close"], app.buttons["Reset all…"]] {
            XCTAssertTrue(e.exists && e.frame.minY >= 0 && e.frame.maxY <= app.frame.maxY && e.isHittable,
                          "\(e.identifier) \(e.frame) is off the sheet")
        }
        XCTAssertTrue(control("lab-plane").isHittable, "the plane picker cannot be touched")
        if phone { raiseSheet() }
        for id in Self.labControls {
            let e = phone ? reach(id) : control(id)
            XCTAssertTrue(e.exists && e.isHittable, "\(id) cannot be touched")
        }
        let inList = app.collectionViews.firstMatch.descendants(matching: .any)["lab-reset"].exists
        XCTAssertEqual(inList, phone, "Reset in the list: \(inList)")
        XCTAssertTrue(control("lab-plane").isHittable, "the plane picker scrolled away")
    }

    private func writeFrames(_ device: String) {
        guard let dir = ProcessInfo.processInfo.environment["ALMONDBREAD_UITEST_SHOTS"] else { return }
        let lines = (["lab-plane"] + Self.labControls).map { "\($0) \(control($0).frame)" }
        try? lines.joined(separator: "\n").write(toFile: "\(dir)/\(device)-lab-frames.txt", atomically: true, encoding: .utf8)
    }

    private func openPicker() {
        let control = app.buttons["Control center"]
        XCTAssertTrue(control.waitForExistence(timeout: 5))
        if control.value as? String != "shown" { control.tap() }
        settle(2)
        let search = app.textFields["Search settings"]
        XCTAssertTrue(search.waitForExistence(timeout: 5))
        search.tap()
        search.typeText("fractal")
        settle(1.5)
    }

    private func closeControlCenter() {
        let control = app.buttons["Control center"]
        for _ in 0..<3 where control.value as? String == "shown" {
            app.buttons["control-center-close"].tap()
            settle(1)
        }
    }

    func testLabFromThePickerAndThePanels() throws {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        settle(3)
        let start = fields()
        XCTAssertEqual(start["lab"], "off", "the lab is on at launch: \(start)")
        // The picker's Parameter lab.
        openPicker()
        let family = app.buttons["formula-family"]
        XCTAssertTrue(family.waitForExistence(timeout: 5), "no Fractal picker")
        family.tap()
        settle(1)
        app.buttons["Parameter lab"].firstMatch.tap()
        settle(2.5)
        XCTAssertEqual(fields()["lab"], "c", "not in the lab's c plane: \(fields())")
        shot("lab-control-center")
        let phone = UIDevice.current.userInterfaceIdiom == .phone
        writeFrames(phone ? "iphone" : "ipad")
        // As the control center opens, half height on a phone: no search, no keyboard.
        closeControlCenter()
        app.buttons["Control center"].tap()
        settle(2)
        app.buttons["Clear search"].firstMatch.tap()
        settle(1)
        shot("lab-control-center-open")
        checkControlsCanBeTouched(phone)
        shot("lab-control-center-end")
        // Random and Reset work where they are.
        let c = control("lab-c-re-field")
        let reset = c.value as? String
        control("lab-random").tap()
        settle(1)
        XCTAssertNotEqual(c.value as? String, reset, "Random did not change c")
        control("lab-reset").tap()
        settle(1)
        XCTAssertEqual(c.value as? String, reset, "Reset did not bring c back")
        if phone {
            // Landscape: less height still, the controls stay in reach.
            XCUIDevice.shared.orientation = .landscapeLeft
            settle(2.5)
            shot("lab-control-center-landscape")
            checkControlsCanBeTouched(phone)
            shot("lab-control-center-landscape-end")
            XCUIDevice.shared.orientation = .portrait
            settle(2.5)
        }
        closeControlCenter()
        settle(3)
        shot("lab-c")
        // The panels keep clear of the round buttons and the HUD.
        guard let row = panels()["lab"] else {
            XCTFail("no lab panels in \(panels())")
            return
        }
        let canvasRect = CGRect(origin: .zero, size: canvas.frame.size).insetBy(dx: -1, dy: -1)
        XCTAssertTrue(canvasRect.contains(row), "the lab panels \(row) leave the canvas \(canvasRect)")
        for (name, r) in roundButtons() {
            let overlap = row.insetBy(dx: 1, dy: 1).intersection(r)
            XCTAssertTrue(overlap.isNull || overlap.isEmpty, "the lab panels \(row) over \(name) \(r)")
        }
        // The first panel is a plane other than c (z0): its "show" button (top right).
        let first = rowPanels(row)[0]
        at(first.maxX - 25, first.minY + 14).tap()
        settle(2.5)
        XCTAssertEqual(fields()["lab"], "z0", "the show button did not switch to the z0 plane: \(fields())")
        shot("lab-z0")
        // Now the first panel is the c plane's: a drag in it moves c.
        let c0 = fields()["labc"]
        let p = rowPanels(panels()["lab"] ?? row)[0]
        at(p.midX - 20, p.midY + 10).press(forDuration: 0.2, thenDragTo: at(p.midX + 20, p.midY - 15))
        settle(2)
        XCTAssertNotEqual(fields()["labc"], c0, "the drag on the c panel did not move c")
        XCTAssertEqual(fields()["lab"], "z0", "the drag changed the plane")
        shot("lab-dragged")
        // Mandelbrot in the picker: out of the lab, at the view it was entered from.
        openPicker()
        app.buttons["formula-family"].tap()
        settle(1)
        app.buttons["Mandelbrot"].firstMatch.tap()
        settle(2.5)
        let end = fields()
        XCTAssertEqual(end["lab"], "off", "still in the lab: \(end)")
        XCTAssertEqual(end["formula"], "mandelbrot")
        XCTAssertEqual(end["re"], start["re"], "not back at the view the lab was entered from")
        closeControlCenter()
        app.terminate()
    }
}
