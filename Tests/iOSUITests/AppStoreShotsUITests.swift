import XCTest

/// Draft App Store screenshots (not a check): tour cards over their views, the walkthrough's
/// ring, the control center's tours, a deep place, cruise with the cockpit, the mini-map, the
/// size scale and a place as a picture (Spiral Galaxy). `make ios-uitest
/// TESTS=AppStoreShotsUITests DEVICE=iphone|ipad` writes `build/ios-screens/<idiom>-appstore-*.png`;
/// scripts/appstore-shots.sh adds captions.
final class AppStoreShotsUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        app = XCUIApplication()
        app.launchEnvironment["ALMONDBREAD_UITEST"] = "1"
    }

    private var canvas: XCUIElement { app.otherElements["fractal"] }
    private var next: XCUIElement { app.buttons["Next ›"] }

    private func settle(_ s: Double) { Thread.sleep(forTimeInterval: s) }

    private func shot(_ name: String) {
        let png = XCUIScreen.main.screenshot().pngRepresentation
        let device = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        let a = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
        a.name = "appstore-\(name)"
        a.lifetime = .keepAlways
        add(a)
        if let dir = ProcessInfo.processInfo.environment["ALMONDBREAD_UITEST_SHOTS"] {
            try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
            try? png.write(to: URL(fileURLWithPath: "\(dir)/\(device)-appstore-\(name).png"))
        }
    }

    private func launch(_ args: [String]) {
        app.launchArguments = args
        app.launch()
        XCTAssertTrue(canvas.waitForExistence(timeout: 30), "no fractal view")
    }

    /// The demo scenes open the control center; close it for a view of the picture.
    private func closeControlCenter() {
        let close = app.buttons["control-center-close"]
        if close.waitForExistence(timeout: 5) { close.tap() }
    }

    /// A tour (`--ui-demo=tour-<id>`) stepped with Next to the step titled `title`, shot after
    /// `wait` seconds (the flight lands, the view renders, a probe slides).
    private func tour(_ id: String, to title: String, wait: Double, name: String) {
        launch(["--ui-demo=tour-\(id)"])
        XCTAssertTrue(next.waitForExistence(timeout: 15), "the tour did not start")
        settle(2)
        for _ in 0..<12 where !app.staticTexts[title].exists {
            next.tap()
            settle(1.2)
        }
        XCTAssertTrue(app.staticTexts[title].exists, "no step \(title)")
        settle(wait)
        shot(name)
        app.terminate()
    }

    // (Not "Period 2688": its 300 000-step render keeps the simulator's GPU so busy that its
    // compositor, and so the screenshot request, gets no GPU time; the app's main thread stays
    // responsive since the queue guard, TourPerformanceUITests.)
    func test1PeriodTour() { tour("minibrot-periods", to: "Deeper, longer", wait: 40, name: "1-period-128") }
    func test2PiTour() { tour("pi", to: "Closer and closer", wait: 20, name: "2-pi") }
    func test3BulbsTour() { tour("bulbs", to: "Five", wait: 10, name: "3-bulbs") }
    func test4DoublingTour() { tour("doubling", to: "Twenty-four arms", wait: 12, name: "4-doubling") }
    func test5DeepTour() { tour("limits", to: "Skipping ahead", wait: 20, name: "5-deep-1e302") }

    /// The walkthrough: the offer, then its gold ring round the cockpit.
    func test6Walkthrough() {
        launch(["--walkthrough-offer"])
        let offer = app.descendants(matching: .any)["walkthrough-offer"].firstMatch
        XCTAssertTrue(offer.waitForExistence(timeout: 10), "no walkthrough offer")
        settle(4)
        shot("6-walkthrough-offer")
        app.buttons["walkthrough-start"].tap()
        XCTAssertTrue(next.waitForExistence(timeout: 5), "the walkthrough did not start")
        settle(1)
        for _ in 0..<10 where !app.staticTexts["The cockpit"].exists {
            next.tap()
            settle(1.2)
        }
        settle(3)
        shot("6-walkthrough-cockpit")
        for _ in 0..<10 where !app.staticTexts["Tours"].exists {
            next.tap()
            settle(1.2)
        }
        settle(3)
        shot("6-walkthrough-tours")
        app.terminate()
    }

    /// The control center's Tours section (one tour done).
    func test7ControlCenterTours() {
        launch(["--ui-demo=tours"])
        settle(6)
        shot("7-control-center-tours")
        if UIDevice.current.userInterfaceIdiom == .phone {
            app.swipeUp()
            settle(2)
            shot("7-control-center-tours-large")
        }
        app.terminate()
    }

    /// Cruise with the cockpit (the autopilot from the seahorse valley, 12 s in) and the mini-map.
    func test8Autopilot() {
        launch(["--ui-demo=autopilot"])
        closeControlCenter()
        settle(14)
        shot("8-autopilot")
        app.terminate()
    }

    /// The size scale above the mini-map, and the 1e94 needle minibrot with four map levels.
    func test9MapsAndScale() {
        launch(["--ui-demo=cosmic"])
        closeControlCenter()
        settle(10)
        shot("9-cosmic")
        app.terminate()
        launch(["--ui-demo=deep"])
        closeControlCenter()
        settle(14)
        shot("9-deep-1e94")
        app.terminate()
    }

    /// A place as a picture: Spiral Galaxy (1e22) in its look.
    func test10SpiralGalaxy() {
        launch(["--ui-demo=place-spiral-galaxy"])
        closeControlCenter()
        settle(15)
        shot("10-spiral-galaxy")
        app.terminate()
    }
}
