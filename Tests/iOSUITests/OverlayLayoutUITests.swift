import XCTest

/// The Rust panels keep clear of the touch controls and of each other (kb: decisions/controls
/// "Touch", "Cosmic size scale", "π meter on narrow canvases"), in portrait and on its side:
/// the cosmic size scale against the cockpit, the maps, the round buttons and the throttle; the
/// π meter and its trail inset against the tour card during the π tour. Rects come from the
/// fractal view's value (" panel-<id>:x0,y0,x1,y1", `ALMONDBREAD_UITEST=1`). Screenshots
/// `layout-*` as in `TouchUITests`.
final class OverlayLayoutUITests: XCTestCase {
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
    private var next: XCUIElement { app.buttons["Next ›"] }
    private func settle(_ s: Double) { Thread.sleep(forTimeInterval: s) }

    /// The Rust panels in global points.
    private func panels() -> [String: CGRect] {
        guard let v = canvas.value as? String else { return [:] }
        let origin = canvas.frame.origin
        var out: [String: CGRect] = [:]
        for t in v.split(separator: " ") where t.hasPrefix("panel-") {
            let kv = t.dropFirst("panel-".count).split(separator: ":", maxSplits: 1)
            guard kv.count == 2 else { continue }
            let n = kv[1].split(separator: ",").compactMap { Double($0) }
            guard n.count == 4 else { continue }
            out[String(kv[0])] = CGRect(x: origin.x + n[0], y: origin.y + n[1], width: n[2] - n[0], height: n[3] - n[1])
        }
        return out
    }

    static let roundButtons = ["Orbit", "Mini-map", "Size scale", "Fly to", "Control center"]

    /// A round button by its label (`app.buttons[name]` matches identifiers only: before
    /// 2026-10-19 this file found none and checked nothing against them).
    private func round(_ name: String) -> XCUIElement {
        app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", name)).firstMatch
    }

    /// The platform's controls over the canvas: the five round buttons, the throttle.
    private func controls() -> [String: CGRect] {
        var out: [String: CGRect] = [:]
        for name in Self.roundButtons {
            let b = round(name)
            XCTAssertTrue(b.exists, "no round button \(name)")
            if b.exists { out[name] = b.frame }
        }
        let throttle = app.descendants(matching: .any)["throttle"].firstMatch
        if throttle.exists { out["throttle"] = throttle.frame }
        let hud = app.descendants(matching: .any)["hud"].firstMatch
        if hud.exists { out["hud"] = hud.frame }
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

    /// `rect` lies on the canvas and overlaps none of `others` (by more than a point).
    private func assertClear(_ id: String, _ rect: CGRect, of others: [String: CGRect], _ context: String) {
        let c = canvas.frame.insetBy(dx: -1, dy: -1)
        XCTAssertTrue(c.contains(rect), "\(context): \(id) \(rect) leaves the canvas \(canvas.frame)")
        for (name, r) in others where name != id {
            let overlap = rect.insetBy(dx: 1, dy: 1).intersection(r)
            XCTAssertTrue(overlap.isNull || overlap.isEmpty, "\(context): \(id) \(rect) over \(name) \(r)")
        }
    }

    /// The cosmic size scale with the mini-map: clear of the cockpit, the maps, the round
    /// buttons and the throttle (it overlapped the cockpit on a phone in portrait).
    func testCosmicScaleKeepsClearOfTheControls() {
        app.launchArguments = ["--ui-demo=cosmic"]
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        let close = app.buttons["control-center-close"]
        if close.waitForExistence(timeout: 5) { close.tap() }
        for orientation in [UIDeviceOrientation.portrait, .landscapeLeft] {
            XCUIDevice.shared.orientation = orientation
            settle(5)
            let name = orientation == .portrait ? "portrait" : "landscape"
            shot("layout-cosmic-\(name)")
            let p = panels()
            guard let cosmic = p["cosmic"] else {
                XCTFail("\(name): no cosmic panel in \(p)")
                continue
            }
            XCTAssertNotNil(p["cockpit"], "\(name): no cockpit")
            assertClear("cosmic", cosmic, of: p.merging(controls()) { a, _ in a }, name)
            assertPanelsClear(p, name)
        }
        app.terminate()
    }

    /// The cockpit and the map row clear of each other, of the HUD, the round buttons and the
    /// throttle (a phone on its side lifted the cockpit above two maps into the HUD; in
    /// portrait the second map ran under the button column).
    private func assertPanelsClear(_ p: [String: CGRect], _ context: String) {
        let c = controls()
        XCTAssertNotNil(c["hud"], "\(context): no HUD")
        if let cockpit = p["cockpit"] {
            var others = c
            if let maps = p["maps"] { others["maps"] = maps }
            assertClear("cockpit", cockpit, of: others, context)
        }
        if let maps = p["maps"] { assertClear("maps", maps, of: c, context) }
    }

    /// The mini-map at its default three levels, no other panel: the map row (fewer or
    /// narrower panels where there is no room) and the cockpit clear of the HUD, the buttons
    /// and the throttle, in portrait and on its side.
    func testMapsKeepClearOfTheControls() {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        let map = round("Mini-map")
        XCTAssertTrue(map.waitForExistence(timeout: 10), "no Mini-map button")
        map.tap()
        for orientation in [UIDeviceOrientation.portrait, .landscapeLeft] {
            XCUIDevice.shared.orientation = orientation
            settle(5)
            let name = orientation == .portrait ? "portrait" : "landscape"
            shot("layout-maps-\(name)")
            let p = panels()
            XCTAssertNotNil(p["maps"], "\(name): no maps in \(p)")
            XCTAssertNotNil(p["cockpit"], "\(name): no cockpit in \(p)")
            assertPanelsClear(p, name)
        }
        app.terminate()
    }

    /// Every step of a tour: the tour card on the canvas and clear of the throttle, the round
    /// buttons and the cockpit, its Next button hittable (on a phone in portrait the card's
    /// Next sat under the top of the throttle); the last step on its side too.
    func testTourCardKeepsClearOfTheControls() {
        app.launchArguments = ["--ui-demo=tour-feigenbaum"]
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        XCTAssertTrue(next.waitForExistence(timeout: 15), "the tour did not start")
        settle(2)
        func check(_ context: String) {
            let card = app.descendants(matching: .any)["tour-card"].firstMatch
            guard card.exists else {
                XCTFail("\(context): no tour card")
                return
            }
            var others = controls().filter { $0.key != "hud" }
            let p = panels()
            for id in ["cockpit", "maps"] { if let r = p[id] { others[id] = r } }
            assertClear("tour-card", card.frame, of: others, context + " (\(card.value as? String ?? ""))")
            let forward = next.exists ? next : app.buttons["Finish"]
            XCTAssertTrue(forward.isHittable, "\(context): Next not hittable")
            for (name, r) in others where r.contains(CGPoint(x: forward.frame.midX, y: forward.frame.midY)) {
                XCTFail("\(context): Next under \(name)")
            }
            // Readable: in portrait the text keeps at least three lines when it shows (a phone
            // on its side may get one line, scrolling, at a long last step).
            let text = app.descendants(matching: .any)["tour-text"].firstMatch
            if text.exists && context.hasPrefix("portrait") {
                XCTAssertGreaterThanOrEqual(text.frame.height, 40, "\(context): tour text \(text.frame.height) points")
            }
        }
        var step = 1
        while true {
            check("portrait step \(step)")
            if step == 1 || step == 5 { shot("layout-tour-portrait-\(step)") }
            guard next.exists, next.label == "Next ›" else { break }
            next.tap()
            settle(2.5)
            step += 1
            if step > 12 { break }
        }
        XCTAssertGreaterThan(step, 2, "the tour did not step")
        XCUIDevice.shared.orientation = .landscapeLeft
        settle(4)
        shot("layout-tour-landscape")
        check("landscape step \(step)")
        app.terminate()
    }

    /// The π tour's meter step: the π panel (and its trail inset when shown) on the canvas and
    /// clear of the tour card, the cockpit and the buttons (it was half off the left edge on the
    /// iPad mini in portrait).
    func testPiMeterFitsBesideTheTourCard() {
        app.launchArguments = ["--ui-demo=tour-pi"]
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        XCTAssertTrue(next.waitForExistence(timeout: 15), "the tour did not start")
        settle(2)
        for _ in 0..<12 where !app.staticTexts["Closer and closer"].exists {
            next.tap()
            settle(1.2)
        }
        XCTAssertTrue(app.staticTexts["Closer and closer"].exists, "no step Closer and closer")
        for orientation in [UIDeviceOrientation.portrait, .landscapeLeft] {
            XCUIDevice.shared.orientation = orientation
            settle(8)
            let name = orientation == .portrait ? "portrait" : "landscape"
            shot("layout-pi-tour-\(name)")
            let p = panels()
            guard let pi = p["pi"] else {
                XCTFail("\(name): no π panel in \(p)")
                continue
            }
            var others = p.merging(controls()) { a, _ in a }
            let card = app.descendants(matching: .any)["tour-card"].firstMatch
            XCTAssertTrue(card.exists, "\(name): no tour card")
            if card.exists {
                others["tour-card"] = card.frame
                // The card itself clear of the throttle and the round buttons.
                assertClear("tour-card", card.frame, of: controls().filter { $0.key != "hud" }, name)
            }
            assertClear("pi", pi, of: others, name)
            if let trail = p["pi-trail"] { assertClear("pi-trail", trail, of: others, name) }
            // Readable: shrunk at most to 3/4 of its 316 points.
            XCTAssertGreaterThanOrEqual(pi.width, 0.75 * 316 - 1, "\(name): π panel \(pi.width) points wide")
        }
        app.terminate()
    }
}
