import XCTest
@testable import AlmondbreadKit

/// The travel journal's keys (kb: decisions/controls "Travel journal"): ⌘Z back, ⇧⌘Z and ⌘Y
/// forward on the canvas; a text field being edited keeps its undo.
final class JournalKeysTests: XCTestCase {
    private func step(_ key: String, shift: Bool = false, option: Bool = false, control: Bool = false,
                      command: Bool = true, text: Bool = false) -> JournalKeys.Step? {
        JournalKeys.step(key: key, command: command, shift: shift, option: option, control: control, textEditing: text)
    }

    func testCommandZBackShiftCommandZAndCommandYForward() {
        XCTAssertEqual(step("z"), .back)
        XCTAssertEqual(step("Z", shift: true), .forward)
        XCTAssertEqual(step("z", shift: true), .forward)
        XCTAssertEqual(step("y"), .forward)
    }

    func testTextEditingKeepsItsUndo() {
        XCTAssertEqual(step("z", text: true), .textUndo)
        XCTAssertEqual(step("Z", shift: true, text: true), .textRedo)
        XCTAssertNil(step("y", text: true), "⌘Y is no text redo on the Mac: it goes on")
    }

    func testOtherKeysAndModifiersGoOn() {
        XCTAssertNil(step("z", command: false), "plain Z is the autopilot")
        XCTAssertNil(step("z", option: true))
        XCTAssertNil(step("z", control: true))
        XCTAssertNil(step("Y", shift: true))
        XCTAssertNil(step("c"))
    }
}
