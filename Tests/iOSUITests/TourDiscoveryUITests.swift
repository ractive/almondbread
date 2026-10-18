import XCTest

/// Finding the tours (kb: decisions/controls "Finding the tours"): the last step's "Next tour"
/// starts the next one; the HUD's "What's a period?" next to "Minibrot, period N" starts the
/// tour on periods. Screenshots `build/ios-screens/<idiom>-tours-*.png`.
final class TourDiscoveryUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchEnvironment["ALMONDBREAD_UITEST"] = "1"
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }
    private var nextTour: XCUIElement { app.buttons["tour-next-tour"] }

    private func settle(_ s: Double = 1.5) { Thread.sleep(forTimeInterval: s) }

    private func shot(_ name: String) {
        let png = XCUIScreen.main.screenshot().pngRepresentation
        let device = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        let a = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
        a.name = "tours-\(name)"
        a.lifetime = .keepAlways
        add(a)
        if let dir = ProcessInfo.processInfo.environment["ALMONDBREAD_UITEST_SHOTS"] {
            try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
            try? png.write(to: URL(fileURLWithPath: "\(dir)/\(device)-tours-\(name).png"))
        }
    }

    /// The step counter "n of m" on the card.
    private func counter() -> (Int, Int)? {
        for t in app.staticTexts.allElementsBoundByIndex.map(\.label) {
            let p = t.split(separator: " ")
            if p.count == 3, p[1] == "of", let a = Int(p[0]), let b = Int(p[2]) { return (a, b) }
        }
        return nil
    }

    /// The π tour (6 steps) to its last step: "Next tour" is offered; tapping it starts that tour
    /// at its first step, with its title on the card.
    func testNextTourStartsTheNextTour() throws {
        app.launchArguments = ["--ui-demo=tour-pi"]
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        let next = app.buttons["Next ›"]
        XCTAssertTrue(next.waitForExistence(timeout: 10), "the tour did not start")
        XCTAssertFalse(nextTour.exists, "Next tour before the last step")
        for _ in 0..<12 where !app.buttons["Finish"].exists {
            next.tap()
            settle(1.2)
        }
        XCTAssertTrue(app.buttons["Finish"].exists, "never reached the last step")
        XCTAssertTrue(nextTour.waitForExistence(timeout: 3), "no Next tour on the last step")
        XCTAssertTrue(nextTour.isHittable, "Next tour is covered")
        let label = nextTour.label
        XCTAssertTrue(label.hasPrefix("Next tour: "), label)
        let title = String(label.dropFirst("Next tour: ".count))
        XCTAssertNotEqual(title, "π in the Mandelbrot set", "offers itself")
        shot("next-tour")
        nextTour.tap()
        settle(2)
        let (n, count) = try XCTUnwrap(counter(), "no card after Next tour")
        XCTAssertEqual(n, 1, "the next tour starts at its first step")
        XCTAssertGreaterThan(count, 1)
        XCTAssertTrue(app.staticTexts[title].exists, "the card does not show \(title)")
        shot("next-tour-started")
        // End tour: the card goes.
        app.buttons["End tour"].tap()
        settle(1.5)
        XCTAssertFalse(app.buttons["Next ›"].exists, "the tour did not end")
    }

    /// Seek minibrot at the Airplane (the `seek` demo scene): the HUD says "Minibrot, period 7"
    /// with "What's a period?" under it; tapping that starts the minibrot-periods tour.
    func testPeriodLinkStartsTheTour() throws {
        app.launchArguments = ["--ui-demo=seek"]
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        let link = app.buttons["tour-topic-period"]
        // The demo opens the control center; the phone's sheet covers the picture's touches.
        if UIDevice.current.userInterfaceIdiom != .pad, app.buttons["control-center-close"].waitForExistence(timeout: 5) {
            app.buttons["control-center-close"].tap()
            settle(1)
        }
        XCTAssertTrue(link.waitForExistence(timeout: 30), "no period link in the HUD")
        XCTAssertTrue(link.isHittable, "the period link is covered")
        shot("period-link")
        link.tap()
        settle(2)
        let (n, count) = try XCTUnwrap(counter(), "the link started no tour")
        XCTAssertEqual(n, 1)
        XCTAssertGreaterThan(count, 1)
        XCTAssertTrue(app.staticTexts["What a minibrot's period means"].exists, "not the period tour")
        XCTAssertFalse(link.exists, "the link stays while its tour runs")
        shot("period-tour")
    }
}
