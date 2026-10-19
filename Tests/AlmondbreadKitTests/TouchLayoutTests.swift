import CoreGraphics
import XCTest
@testable import AlmondbreadKit

/// The touch layout for every screen class (kb: decisions/controls "Touch", "Round buttons" ▸
/// Fit), including the large iPhones no shared simulator has.
final class TouchLayoutTests: XCTestCase {
    private func layout(_ w: CGFloat, _ h: CGFloat, hCompact: Bool, vCompact: Bool) -> TouchLayout {
        TouchLayout(canvas: CGSize(width: w, height: h), horizontalCompact: hCompact, verticalCompact: vCompact)
    }

    func testPhonePortraitTakesTheColumn() {
        for (w, h) in [(390.0, 844.0), (440.0, 956.0)] { // iPhone 17e, a Pro Max
            let l = layout(w, h, hCompact: true, vCompact: false)
            XCTAssertTrue(l.buttonColumn)
            XCTAssertEqual(l.buttonCorner, .bottomTrailing)
            XCTAssertEqual(l.trailingInset, 56)
            XCTAssertEqual(l.topInset, 0)
            XCTAssertEqual(l.bottomInset, 0)
        }
    }

    /// Every phone on its side gets the bottom row: the 17e (compact width class) and the large
    /// models (regular width class, 932 or 956 points), which used to get a top-trailing row.
    func testPhoneOnItsSideTakesTheBottomRowWhateverItsWidthClass() {
        for (w, h, hCompact) in [(844.0, 390.0, true), (956.0, 440.0, false), (932.0, 430.0, false)] {
            let l = layout(w, h, hCompact: hCompact, vCompact: true)
            XCTAssertFalse(l.buttonColumn, "\(w)")
            XCTAssertEqual(l.buttonCorner, .bottomTrailing, "\(w)")
            XCTAssertEqual(l.cockpitInsetRight, Double(TouchLayout.buttonRow + 12), "\(w)")
            XCTAssertEqual(l.bottomInset, 56, "\(w)")
            XCTAssertEqual(l.topInset, 0, "nothing at the top trailing corner: \(w)")
            XCTAssertEqual(l.trailingInset, Double(TouchLayout.throttleWidth + 12), "\(w)")
        }
    }

    func testIPadTakesTheTopRowUnlessNarrow() {
        for (w, h) in [(744.0, 1133.0), (1133.0, 744.0), (1032.0, 1376.0)] {
            let l = layout(w, h, hCompact: false, vCompact: false)
            XCTAssertFalse(l.compactWidth)
            XCTAssertEqual(l.buttonCorner, .topTrailing)
            XCTAssertEqual(l.topInset, 56)
            XCTAssertEqual(l.cockpitInsetRight, 0)
        }
        // Portrait next to the control center (canvas under 560 points), narrow split views.
        let narrow = layout(414, 1133, hCompact: false, vCompact: false)
        XCTAssertTrue(narrow.compactWidth && narrow.buttonColumn)
        XCTAssertEqual(narrow.buttonCorner, .bottomTrailing)
        XCTAssertEqual(layout(320, 1133, hCompact: true, vCompact: false).buttonCorner, .bottomTrailing)
    }
}
