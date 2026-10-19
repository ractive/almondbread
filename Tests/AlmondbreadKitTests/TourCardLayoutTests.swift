import CoreGraphics
import XCTest
@testable import AlmondbreadKit

/// The tour card at the top of a narrow canvas keeps clear of the touch controls, the cockpit
/// and the maps (kb: decisions/controls "Tour card on a phone"). Layer coordinates (the safe
/// area's), obstacles with their margins, as on the iPhone 17e.
final class TourCardLayoutTests: XCTestCase {
    /// Portrait: the full-width card would reach the throttle on top of the button column, so it
    /// narrows to the leading side of the column and keeps its height.
    func testPortraitCardNarrowsBesideTheThrottle() {
        let obstacles = [
            CGRect(x: 326, y: 257, width: 64, height: 236), // throttle
            CGRect(x: 326, y: 483, width: 64, height: 276), // round buttons
            CGRect(x: 27, y: 480, width: 280, height: 86), // cockpit
        ]
        let f = TourCardLayer.topFit(size: CGSize(width: 390, height: 763), width: 366, leading: false, obstacles: obstacles)
        XCTAssertEqual(f.x, 12)
        XCTAssertEqual(f.width, 302)
        XCTAssertEqual(f.maxHeight, 468)
    }

    /// Portrait with the column far enough down: full width, centred.
    func testPortraitCardKeepsItsWidthWhereThereIsRoom() {
        let obstacles = [CGRect(x: 326, y: 400, width: 64, height: 359)]
        let f = TourCardLayer.topFit(size: CGSize(width: 390, height: 763), width: 366, leading: false, obstacles: obstacles)
        XCTAssertEqual(f.x, 12)
        XCTAssertEqual(f.width, 366)
        XCTAssertEqual(f.maxHeight, 388)
    }

    /// On its side: the maps and the cockpit under the leading side leave too little; the card
    /// goes to the trailing side, left of the throttle, above the cockpit and the button row.
    func testLandscapeCardMovesClearOfTheMaps() {
        let obstacles = [
            CGRect(x: 627, y: 56, width: 64, height: 206), // throttle
            CGRect(x: 415, y: 298, width: 276, height: 60), // round buttons
            CGRect(x: 150, y: 282, width: 280, height: 86), // cockpit
            CGRect(x: 4, y: 172, width: 196, height: 146), // maps
        ]
        let f = TourCardLayer.topFit(size: CGSize(width: 750, height: 369), width: 386, leading: true, obstacles: obstacles)
        XCTAssertEqual(f.x, 229)
        XCTAssertEqual(f.width, 386)
        XCTAssertEqual(f.maxHeight, 270)
        // Without the maps the leading side has room: the card stays there (a π panel fits
        // beside it at the trailing side).
        let g = TourCardLayer.topFit(size: CGSize(width: 750, height: 369), width: 386, leading: true,
                                     obstacles: Array(obstacles.dropLast()))
        XCTAssertEqual(g.x, 12)
        XCTAssertEqual(g.maxHeight, 270)
    }
}
