#if os(macOS)
import AlmondbreadCore
import AppKit
import Metal
import XCTest

@testable import AlmondbreadKit

/// The save and restore path, which every window test skips (`TestLaunch.ephemeral`): a session
/// on a windowless layer through the real `FractalHostBase` and `ExplorerModel`, with its own
/// defaults suite; then quit, a new model and renderer, attach, the first size. Needs a GPU.
@MainActor
final class RestartTests: XCTestCase {
    @MainActor private final class Host: ExplorerHost {
        func requestFrames() {}
        func hudFrameChanged() {}
    }

    /// One launch: what the platform view does (attach at frame zero, then the first size).
    @MainActor private final class Launch {
        let model = ExplorerModel()
        let base: FractalHostBase
        let host = Host()
        var explorer: Explorer { base.explorer! }

        init(sized: Bool = true) {
            base = FractalHostBase(model: model, scale: 2)
            base.attach(host: host)
            base.resize(width: 0, height: 0, scale: 2, host: host)
            if sized { base.resize(width: 800, height: 500, scale: 2, host: host) }
        }

        /// A few refreshes in real time: the journal records a view at rest from them.
        func refreshes(_ n: Int = 12) async throws {
            for _ in 0..<n {
                _ = base.step(timestamp: CACurrentMediaTime())
                try await Task.sleep(for: .milliseconds(5))
            }
        }

        /// The view Reset goes to (the home view of what is shown), which leaves the view there.
        func home() -> ViewportInfo {
            explorer.resetViewport()
            return explorer.viewport()
        }

        var journal: [Substring] { explorer.journalExport().split(separator: "\n") }
    }

    private var suite = ""

    override func setUp() async throws {
        try XCTSkipIf(MTLCreateSystemDefaultDevice() == nil, "no GPU")
        suite = "almondbread.restart-test.\(UUID().uuidString)"
        Preferences.defaults = UserDefaults(suiteName: suite)!
    }

    override func tearDown() async throws {
        Preferences.defaults.removePersistentDomain(forName: suite)
        Preferences.defaults = .standard
    }

    /// A session, then the app quits (its observers flush what waits; the model goes, as a later
    /// quit would otherwise write its state too).
    private func session(_ body: (Launch) async throws -> Void) async throws {
        var launch: Launch? = Launch()
        weak var model = launch?.model
        try await body(launch!)
        NotificationCenter.default.post(name: NSApplication.willTerminateNotification, object: nil)
        try await Task.sleep(for: .milliseconds(20))
        launch = nil
        XCTAssertNil(model, "the model outlives its session")
    }

    private func sameView(_ a: ViewportInfo, _ b: ViewportInfo, _ what: String,
                          file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(a.re, b.re, what, file: file, line: line)
        XCTAssertEqual(a.im, b.im, what, file: file, line: line)
        XCTAssertEqual(a.scale, b.scale, accuracy: b.scale * 1e-12, what, file: file, line: line)
    }

    private func sameCapabilities(_ l: Launch, _ formula: String, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(l.model.capabilities, l.explorer.capabilities(), file: file, line: line)
        XCTAssertEqual(l.model.capabilities?.formula, formula, file: file, line: line)
    }

    // MARK: Formulas (a change saves by itself, capabilities and the home view follow)

    /// The coalesced save alone: no quit flush in this session.
    func testAPickedFormulaIsSavedAndComesBack() async throws {
        var first: Launch? = Launch()
        first?.model.selectFormula("burningship")
        try await Task.sleep(for: .milliseconds(800))
        first = nil
        try await session { l in
            XCTAssertEqual(l.model.formula, "burningship")
            XCTAssertEqual(l.explorer.settings().formula, "burningship")
            sameCapabilities(l, "burningship")
            XCTAssertFalse(l.model.has(\.content) && l.model.has(\.seek) && l.model.has(\.juliaPanel),
                           "Mandelbrot's capabilities")
            let shown = l.explorer.viewport()
            sameView(shown, l.home(), "not fitted to the set")
        }
    }

    /// A change just before quitting: the flush in the quit observer, nothing waited for.
    func testAJuliaSetComesBackWithItsK() async throws {
        var k = ("", "")
        try await session { l in
            try l.explorer.setViewport(re: "-0.75", im: "0.1", scale: 1e-3)
            l.model.juliaTwin()
            XCTAssertTrue(l.model.isJulia)
            k = (l.model.juliaRe, l.model.juliaIm)
        }
        try await session { l in
            XCTAssertEqual(l.model.formula, "julia")
            XCTAssertEqual(l.model.juliaRe, k.0)
            XCTAssertEqual(l.model.juliaIm, k.1)
            XCTAssertEqual(l.explorer.settings().juliaRe, k.0)
            sameCapabilities(l, "julia")
            let shown = l.explorer.viewport()
            sameView(shown, l.home(), "not the whole Julia set")
            // The way back is the fitted Mandelbrot set.
            l.model.backFromJulia()
            XCTAssertEqual(l.model.formula, "mandelbrot")
            let back = l.explorer.viewport()
            sameView(back, l.home(), "the way back is not fitted")
        }
        // Back in Mandelbrot, quit: that persists too.
        try await session { l in XCTAssertEqual(l.model.formula, "mandelbrot") }
    }

    func testATurnedPlaneComesBack() async throws {
        var plane = (0.0, "")
        try await session { l in
            try l.explorer.setViewport(re: "-0.75", im: "0.1", scale: 1e-3)
            l.model.setTurn(30)
            XCTAssertEqual(l.model.formula, "turn")
            plane = (l.model.turnDegrees, l.model.turnPivot)
            XCTAssertFalse(plane.1.isEmpty)
        }
        try await session { l in
            XCTAssertEqual(l.model.formula, "turn")
            XCTAssertEqual(l.model.turnDegrees, plane.0)
            XCTAssertEqual(l.model.turnPivot, plane.1)
            XCTAssertEqual(l.explorer.settings().turnPivot, plane.1)
            sameCapabilities(l, "turn")
            XCTAssertEqual(l.model.turn?.degrees, 30)
            let shown = l.explorer.viewport()
            sameView(shown, l.home(), "not the plane's home view")
        }
    }

    // MARK: The lab and the journal

    func testQuittingInTheLabKeepsTheJournalAndTheLabComesBackFitted() async throws {
        var journal: [Substring] = []
        let knobs = LabKnobs(z0Re: 0.05, z0Im: -0.07, cRe: -0.03, cIm: 0.8, pRe: 2.07, pIm: -0.19)
        try await session { l in
            for place in ["seahorse-valley", "elephant-valley", "whole-set"] {
                l.model.goToPlace(place)
                try await l.refreshes()
            }
            journal = l.journal
            XCTAssertGreaterThan(journal.count, 3, "no journal to lose: \(journal)")
            l.model.enterLab()
            l.model.setLabPlane("z0")
            l.model.setLabKnobs(knobs)
            // A far move in the lab, at rest: no entry.
            _ = l.explorer.input(event: .zoomBy(factor: 8))
            try await l.refreshes()
            XCTAssertEqual(l.journal, journal, "the lab's views reached the journal")
        }
        try await session { l in
            XCTAssertTrue(l.model.isLab)
            XCTAssertTrue(l.explorer.lab().on)
            XCTAssertEqual(l.model.labPlane, "z0")
            XCTAssertEqual(l.explorer.lab().knobs, knobs)
            sameCapabilities(l, "lab")
            let shown = l.explorer.viewport()
            sameView(shown, l.home(), "the lab's home view is not fitted")
            XCTAssertEqual(l.journal, journal, "the journal did not come back in the lab")
            // Out of it: the journal stays, the way back is the fitted Mandelbrot set.
            l.model.leaveLab()
            XCTAssertFalse(l.model.isLab)
            sameCapabilities(l, "mandelbrot")
            let back = l.explorer.viewport()
            sameView(back, l.home(), "the lab's way back is not fitted")
            XCTAssertEqual(Array(l.journal.dropFirst(2)), Array(journal.dropFirst(2)), "leaving the lab lost the journal")
            try await l.refreshes()
        }
        try await session { l in
            XCTAssertFalse(l.model.isLab, "the lab came back after it was left")
            XCTAssertEqual(l.journal.count, journal.count, "the persisted journal shrank")
        }
    }

    /// A window closed or a quit before the first size writes nothing over what was saved.
    func testNothingIsWrittenBeforeTheSavedStateIsBack() async throws {
        var journal: [Substring] = []
        try await session { l in
            for place in ["seahorse-valley", "elephant-valley"] {
                l.model.goToPlace(place)
                try await l.refreshes()
            }
            l.model.selectFormula("burningship")
            try await l.refreshes()
            journal = l.journal
        }
        var early: Launch? = Launch(sized: false)
        NotificationCenter.default.post(name: NSApplication.didResignActiveNotification, object: nil)
        try await Task.sleep(for: .milliseconds(700))
        early = nil
        _ = early
        try await session { l in
            XCTAssertEqual(l.model.formula, "burningship")
            XCTAssertEqual(l.journal, journal)
        }
    }

    // MARK: Tours, the picker, links

    /// The lab during a tour: the tour ends here too, the user's settings come back and stay.
    func testTheLabDuringATourKeepsTheUsersSettings() async throws {
        try await session { l in
            l.model.paletteID = "fire"
            l.model.shading = 0.25
            let view = l.explorer.viewport()
            l.model.startTour("feigenbaum")
            XCTAssertNotNil(l.model.tour)
            l.model.tourGo(2)
            try await l.refreshes(4)
            l.model.enterLab()
            XCTAssertNil(l.model.tour, "the tour's card stays up")
            XCTAssertTrue(l.model.isLab)
            XCTAssertEqual(l.model.paletteID, "fire")
            XCTAssertEqual(l.model.shading, 0.25)
            XCTAssertEqual(l.explorer.settings().paletteId, "fire")
            // Ending the tour again finds nothing to do and changes nothing.
            l.model.endTour()
            l.model.shading = 0.5
            XCTAssertEqual(l.explorer.settings().paletteId, "fire")
            l.model.leaveLab()
            sameView(l.explorer.viewport(), view, "not back at the view before the tour")
        }
        try await session { l in
            XCTAssertEqual(l.model.paletteID, "fire")
            XCTAssertEqual(l.model.shading, 0.5)
        }
    }

    /// After a turned view the family choice comes back to the Mandelbrot set.
    func testThePickerGetsBackFromATurnedView() async throws {
        try await session { l in
            try l.explorer.setViewport(re: "-0.75", im: "0.1", scale: 1e-3)
            l.model.setTurn(30)
            XCTAssertEqual(l.model.formula, "turn")
            l.model.selectFormula("burningship")
            XCTAssertEqual(l.model.formula, "burningship")
            l.model.selectFormulaFamily("mandelbrot")
            XCTAssertEqual(l.model.formula, "mandelbrot")
        }
    }

    /// A paste, a flight and a tour leave the lab; the mirror knows at once, and it persists.
    func testViewsLeaveTheLabAndTheMirrorFollows() async throws {
        try await session { l in
            l.model.enterLab()
            XCTAssertTrue(l.model.pasteCoordinates(text: "re 0.1\nim 0.2\nscale 1e-3\nformula multibrot3"))
            XCTAssertFalse(l.model.isLab)
            XCTAssertEqual(l.model.formula, "multibrot3")
            sameCapabilities(l, "multibrot3")
            l.model.enterLab()
            XCTAssertTrue(l.model.flyTo(text: "re -0.75\nim 0.1\nscale 1e-3\njulia -0.4 0.6"))
            XCTAssertFalse(l.model.isLab)
            XCTAssertEqual(l.model.formula, "julia")
            l.model.enterLab()
            XCTAssertTrue(l.model.isLab)
            XCTAssertNotEqual(l.model.formula, "julia", "Rust left the Julia view: the mirror did not")
            l.model.startTour("feigenbaum")
            XCTAssertFalse(l.model.isLab)
            XCTAssertNotNil(l.model.tour)
            l.model.endTour(fly: false)
            l.model.enterLab()
            try l.model.flyTo(re: "-0.75", im: "0.1", scale: "1e-3")
            XCTAssertFalse(l.model.isLab)
        }
        try await session { l in XCTAssertFalse(l.model.isLab, "the left lab came back") }
    }

    /// The ⌘G sheet flies to all a pasted link says while its fields show that link.
    func testTheSheetKeepsALinkUntilAFieldChanges() async throws {
        let link = "re -1.7\nim 0.02\nscale 1e-3 (zoom 1×)\nformula burningship"
        XCTAssertEqual(FlyToSheet.link(link, re: "-1.7", im: "0.02", scale: "1e-3"), link)
        XCTAssertNil(FlyToSheet.link(link, re: "-1.8", im: "0.02", scale: "1e-3"))
        XCTAssertNil(FlyToSheet.link(nil, re: "-1.7", im: "0.02", scale: "1e-3"))
        try await session { l in
            XCTAssertTrue(l.model.flyTo(text: link))
            XCTAssertEqual(l.model.formula, "burningship")
            sameCapabilities(l, "burningship")
        }
    }
}
#endif
