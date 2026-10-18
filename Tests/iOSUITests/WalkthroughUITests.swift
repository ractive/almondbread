import XCTest

/// The first-launch walkthrough (kb: decisions/controls "Walkthrough"): the offer, the tour's
/// rings around the controls it explains, its try-it step, the control center it opens and
/// closes. Screenshots `build/ios-screens/<idiom>-walkthrough-*.png`.
final class WalkthroughUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchEnvironment["ALMONDBREAD_UITEST"] = "1"
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }
    private var offer: XCUIElement { app.descendants(matching: .any)["walkthrough-offer"].firstMatch }
    private var highlight: XCUIElement { app.descendants(matching: .any)["tour-highlight"].firstMatch }
    private var next: XCUIElement { app.buttons["Next ›"] }

    private func settle(_ s: Double = 1.5) { Thread.sleep(forTimeInterval: s) }

    private func launchSettled() {
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
        settle(3)
    }

    private func shot(_ name: String) {
        let png = XCUIScreen.main.screenshot().pngRepresentation
        let device = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        let a = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
        a.name = "walkthrough-\(name)"
        a.lifetime = .keepAlways
        add(a)
        if let dir = ProcessInfo.processInfo.environment["ALMONDBREAD_UITEST_SHOTS"] {
            try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
            try? png.write(to: URL(fileURLWithPath: "\(dir)/\(device)-walkthrough-\(name).png"))
        }
    }

    /// The step counter "n of m" on the card.
    private func counter() -> (Int, Int)? {
        let texts = app.staticTexts.allElementsBoundByIndex.map(\.label)
        for t in texts {
            let p = t.split(separator: " ")
            if p.count == 3, p[1] == "of", let a = Int(p[0]), let b = Int(p[2]) { return (a, b) }
        }
        return nil
    }

    /// Next until the card's title is `title` (at most `limit` steps).
    private func step(to title: String, limit: Int = 10) {
        for _ in 0..<limit {
            if app.staticTexts[title].exists { return }
            next.tap()
            settle(1.2)
        }
        XCTAssertTrue(app.staticTexts[title].exists, "no step \(title)")
    }

    /// Forced offer → walkthrough: a ring at the cockpit, a try-it step that notices a zoom, the
    /// control center opened with its Places ringed, closed again at the end.
    func testWalkthrough() throws {
        app.launchArguments = ["--walkthrough-offer"]
        launchSettled()
        XCTAssertTrue(offer.waitForExistence(timeout: 5), "no walkthrough offer")
        shot("offer")
        app.buttons["walkthrough-start"].tap()
        settle(2)
        XCTAssertFalse(offer.exists, "the offer stayed")
        XCTAssertTrue(next.waitForExistence(timeout: 5), "the walkthrough did not start")
        let (first, count) = try XCTUnwrap(counter())
        XCTAssertEqual(first, 1)
        XCTAssertTrue((6...10).contains(count), "\(count) steps")
        XCTAssertFalse(highlight.exists, "a ring on the first step")
        shot("1")

        // Try it: zoom in.
        step(to: "Move and zoom")
        let goal = app.descendants(matching: .any)["tour-goal"].firstMatch
        XCTAssertTrue(goal.waitForExistence(timeout: 3))
        XCTAssertEqual(goal.value as? String, "waiting")
        canvas.pinch(withScale: 3, velocity: 3)
        settle(2)
        XCTAssertEqual(goal.value as? String, "done", "the zoom was not noticed")
        shot("goal")

        step(to: "The cockpit")
        XCTAssertTrue(highlight.waitForExistence(timeout: 3), "no ring at the cockpit")
        XCTAssertEqual(highlight.value as? String, "cockpit")
        shot("cockpit")

        step(to: "Cruise")
        XCTAssertTrue(highlight.waitForExistence(timeout: 3))
        XCTAssertEqual(highlight.value as? String, "throttle")
        // The card keeps clear of what it rings.
        XCTAssertFalse(highlight.frame.intersects(next.frame), "the card covers the throttle")
        shot("throttle")

        step(to: "The round buttons")
        XCTAssertTrue(highlight.waitForExistence(timeout: 3))
        XCTAssertEqual(highlight.value as? String, "toolbar")
        XCTAssertFalse(highlight.frame.intersects(next.frame), "the card covers the round buttons")
        shot("toolbar")

        step(to: "The control center")
        settle(1.5)
        XCTAssertTrue(app.buttons["control-center-close"].waitForExistence(timeout: 5), "the control center did not open")
        XCTAssertTrue(highlight.waitForExistence(timeout: 5), "no ring in the control center")
        XCTAssertEqual(highlight.value as? String, "section-places")
        XCTAssertTrue(next.isHittable, "the card is covered")
        shot("control-center")

        step(to: "Tours")
        settle(1.5)
        XCTAssertEqual(highlight.value as? String, "section-tours")
        shot("tours")

        step(to: "Off you go!")
        settle(1.5)
        XCTAssertFalse(app.buttons["control-center-close"].exists, "the control center stayed open")
        let finish = app.buttons["Finish"]
        XCTAssertTrue(finish.exists)
        finish.tap()
        settle(1.5)
        XCTAssertFalse(next.exists || finish.exists, "the card stayed")
        XCTAssertFalse(highlight.exists, "a ring stayed")
    }

    /// No offer in UI tests (settings are ephemeral) or when suppressed, even if forced.
    func testOfferNotShownUnlessForced() throws {
        launchSettled()
        XCTAssertFalse(offer.exists, "the offer showed in a UI test")
        app.terminate()
        app.launchArguments = ["--walkthrough-offer", "--no-walkthrough-offer"]
        launchSettled()
        XCTAssertFalse(offer.exists, "the offer showed although suppressed")
        // "Just explore" closes it (and starts nothing).
        app.terminate()
        app.launchArguments = ["--walkthrough-offer"]
        launchSettled()
        XCTAssertTrue(offer.waitForExistence(timeout: 5))
        app.buttons["walkthrough-skip"].tap()
        settle(1)
        XCTAssertFalse(offer.exists)
        XCTAssertFalse(next.exists, "Just explore started the walkthrough")
    }
}
