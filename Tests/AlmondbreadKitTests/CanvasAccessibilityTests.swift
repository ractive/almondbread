import AlmondbreadCore
import CoreGraphics
import XCTest
@testable import AlmondbreadKit

/// The canvas's accessibility element builder (kb: plans/a11y-canvas-controls): what each Rust
/// control id does when activated, which records become elements, where their centres are for
/// the hit tests, and when the layout counts as changed.
final class CanvasAccessibilityTests: XCTestCase {
    private func record(_ id: String, _ role: AccessibleRole = .button, x: Double = 10, y: Double = 20,
                        w: Double = 44, h: Double = 44, selected: Bool = false) -> AccessibleControl {
        AccessibleControl(id: id, role: role, label: id, value: "", selected: selected, x: x, y: y, width: w, height: h)
    }

    func testActionsFollowTheIds() {
        XCTAssertEqual(CanvasAccessibility.action(for: "cockpit-cruise"), .cockpit("cruise"))
        XCTAssertEqual(CanvasAccessibility.action(for: "cockpit-lane"), .cockpit("lane"))
        XCTAssertEqual(CanvasAccessibility.action(for: "help-mini-map"), .help)
        XCTAssertEqual(CanvasAccessibility.action(for: "compass"), .compass)
        XCTAssertEqual(CanvasAccessibility.action(for: "map-0"), .map)
        XCTAssertNil(CanvasAccessibility.action(for: "julia"))
        XCTAssertNil(CanvasAccessibility.action(for: "cosmic"))
        XCTAssertNil(CanvasAccessibility.action(for: "pi"))
    }

    func testRecordsWithoutAFrameAreDropped() {
        let c = CanvasAccessibility.controls([
            record("cockpit-manual"),
            record("map-0", .image, w: 0),
            record("pi", .text, x: .nan),
            record("cockpit-cruise", selected: true),
        ])
        XCTAssertEqual(c.map(\.id), ["cockpit-manual", "cockpit-cruise"])
        XCTAssertEqual(c[1].frame, CGRect(x: 10, y: 20, width: 44, height: 44))
        XCTAssertTrue(c[1].selected)
    }

    /// Hit tests take view points from the centre, y up.
    func testCentreIsFromTheViewCentreYUp() {
        let p = CanvasAccessibility.centred(CGRect(x: 90, y: 10, width: 20, height: 20), in: CGSize(width: 200, height: 100))
        XCTAssertEqual(p.x, 0)
        XCTAssertEqual(p.y, 30)
    }

    func testLayoutChangeMeansIdsOrFrames() {
        let a = CanvasAccessibility.controls([record("compass"), record("map-0", .image)])
        var b = CanvasAccessibility.controls([record("compass"), record("map-0", .image)])
        XCTAssertFalse(CanvasAccessibility.layoutChanged(a, b))
        b = CanvasAccessibility.controls([record("compass", x: 11), record("map-0", .image)])
        XCTAssertTrue(CanvasAccessibility.layoutChanged(a, b))
        XCTAssertTrue(CanvasAccessibility.layoutChanged(a, Array(a.prefix(1))))
        // A value or selection change alone is not a layout change (read on demand).
        let c = CanvasAccessibility.controls([record("compass", selected: true), record("map-0", .image)])
        XCTAssertFalse(CanvasAccessibility.layoutChanged(a, c))
    }

    /// A help badge sits in its panel's corner: the badge wins there, the panel elsewhere.
    func testControlAtAPointIsTheSmallestThatContainsIt() {
        let c = CanvasAccessibility.controls([record("map-0", .image, x: 0, y: 0, w: 100, h: 100),
                                              record("help-mini-map", x: 80, y: 0, w: 20, h: 20)])
        XCTAssertEqual(CanvasAccessibility.control(at: CGPoint(x: 50, y: 50), in: c)?.id, "map-0")
        XCTAssertEqual(CanvasAccessibility.control(at: CGPoint(x: 90, y: 10), in: c)?.id, "help-mini-map")
        XCTAssertNil(CanvasAccessibility.control(at: CGPoint(x: 150, y: 50), in: c))
    }
}
