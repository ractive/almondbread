import XCTest

/// The controls Rust draws are accessibility elements (kb: plans/a11y-canvas-controls): the
/// cockpit's buttons with their labels and the current level selected, a tap on the cruise
/// element switches the level, the mini-map and Julia panels with their values, help badges as
/// buttons, the canvas still found as `fractal`; VoiceOver's hit test lands on the controls, not
/// the canvas under them (`isHittable`). Without `ALMONDBREAD_UITEST=1` the canvas's value is
/// the short summary VoiceOver reads.
final class AccessibilityUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        app = XCUIApplication()
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }
    private func element(_ id: String) -> XCUIElement { app.descendants(matching: .any)[id].firstMatch }
    private func settle(_ s: Double) { Thread.sleep(forTimeInterval: s) }

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

    private func launch(_ scene: String, uiTest: Bool) {
        if uiTest { app.launchEnvironment["ALMONDBREAD_UITEST"] = "1" }
        app.launchArguments = ["--ui-demo=\(scene)"]
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        let close = app.buttons["control-center-close"]
        if close.waitForExistence(timeout: 5) { close.tap() }
        settle(5)
    }

    /// The level letter in the UI tests' state string ("level m|c|a|j").
    private func level() -> String? {
        guard let v = canvas.value as? String, let r = v.range(of: " level ") else { return nil }
        return v[r.upperBound...].split(separator: " ").first.map(String.init)
    }

    func testCockpitMapsAndJuliaAreElements() {
        launch("julia", uiTest: true)
        shot("a11y-julia")
        // The cockpit: one button per level and lane assist, manual current.
        let labels = ["manual": "Manual", "cruise": "Cruise", "autopilot": "Autopilot", "journey": "Journey",
                      "lane": "Lane assist"]
        for (id, label) in labels {
            let e = element("cockpit-\(id)")
            XCTAssertTrue(e.waitForExistence(timeout: 5), "no cockpit-\(id) element")
            XCTAssertEqual(e.label, label)
            XCTAssertEqual(e.elementType, .button, "cockpit-\(id) is not a button")
        }
        let manual = element("cockpit-manual")
        XCTAssertTrue(manual.isSelected, "manual is not the current level")
        XCTAssertEqual(manual.value as? String, "current")
        XCTAssertEqual(element("cockpit-lane").value as? String, "off")
        // VoiceOver's hit test at the button finds the button, not the canvas under it.
        let cruise = element("cockpit-cruise")
        XCTAssertTrue(cruise.isHittable, "the cruise element is not hittable (canvas over it?)")

        // The maps and the Julia panel.
        let map = element("map-0")
        XCTAssertTrue(map.exists, "no mini-map element")
        XCTAssertEqual(map.label, "Mini-map panel")
        let mapValue = map.value as? String ?? ""
        XCTAssertTrue(mapValue.hasPrefix("whole set, level 1 of "), "map value: \(mapValue)")
        let julia = element("julia")
        XCTAssertTrue(julia.exists, "no Julia panel element")
        XCTAssertTrue((julia.value as? String ?? "").hasPrefix("c = "), "julia value: \(String(describing: julia.value))")
        // Help badges are buttons and hittable inside their panels.
        let help = element("help-mini-map")
        XCTAssertTrue(help.exists, "no mini-map help badge element")
        XCTAssertEqual(help.label, "Help: mini-map")
        XCTAssertTrue(help.isHittable, "the help badge is not hittable (its panel over it?)")

        // Tapping the cruise element switches the level (through the real tap and hit test).
        XCTAssertEqual(level(), "m")
        cruise.tap()
        settle(1.5)
        XCTAssertEqual(level(), "c", "the cruise element did not switch to cruise")
        XCTAssertTrue(element("cockpit-cruise").isSelected, "cruise is not selected after the tap")
        XCTAssertTrue((element("cockpit-cruise").value as? String ?? "").contains("gear"),
                      "cruise value: \(String(describing: element("cockpit-cruise").value))")
        shot("a11y-cruise")
        // Back to manual through its element.
        element("cockpit-manual").tap()
        settle(1.5)
        XCTAssertEqual(level(), "m", "the manual element did not stop cruising")
        app.terminate()
    }

    /// The canvas reads as a short sentence, the compass with its turn.
    func testCanvasSummaryAndCompass() {
        launch("compass", uiTest: false)
        shot("a11y-compass")
        XCTAssertEqual(canvas.label, "Mandelbrot set")
        let v = canvas.value as? String ?? ""
        XCTAssertTrue(v.hasPrefix("zoom "), "canvas value: \(v)")
        XCTAssertTrue(v.contains("turned 35° counter-clockwise"), "canvas value: \(v)")
        XCTAssertLessThan(v.count, 120, "canvas value too long: \(v)")
        let compass = element("compass")
        XCTAssertTrue(compass.exists, "no compass element")
        XCTAssertEqual(compass.value as? String, "turned 35° counter-clockwise")
        XCTAssertTrue(compass.isHittable, "the compass is not hittable")
        app.terminate()
    }
}
