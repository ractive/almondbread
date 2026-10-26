import XCTest
@testable import AlmondbreadKit

/// The travel journal's keys (kb: decisions/controls "Travel journal"): the Edit menu's one
/// Undo / Redo pair is the text field's undo while one edits, otherwise Back / Forward; ⌘Y is
/// an extra forward key on the canvas.
final class JournalKeysTests: XCTestCase {
    private typealias Item = JournalKeys.MenuItem

    func testNoTextEditingIsBackAndForward() {
        XCTAssertEqual(JournalKeys.menuItem(redo: false, text: nil, canTravel: true),
                       Item(title: "Back", enabled: true, action: .journal))
        XCTAssertEqual(JournalKeys.menuItem(redo: true, text: nil, canTravel: false),
                       Item(title: "Forward", enabled: false, action: .journal))
    }

    func testTextEditingIsItsUndo() {
        let text = JournalKeys.TextUndo(undoTitle: "Undo Typing", redoTitle: "Redo", canUndo: true, canRedo: false)
        XCTAssertEqual(JournalKeys.menuItem(redo: false, text: text, canTravel: false),
                       Item(title: "Undo Typing", enabled: true, action: .text),
                       "the field's undo, whether or not the journal can go back")
        XCTAssertEqual(JournalKeys.menuItem(redo: true, text: text, canTravel: true),
                       Item(title: "Redo", enabled: false, action: .text),
                       "nothing to redo: disabled, not the journal's forward")
    }

    func testEmptyUndoManagerTitlesFallBack() {
        let text = JournalKeys.TextUndo(undoTitle: "", redoTitle: "", canUndo: false, canRedo: false)
        XCTAssertEqual(JournalKeys.menuItem(redo: false, text: text, canTravel: true).title, "Undo")
        XCTAssertEqual(JournalKeys.menuItem(redo: true, text: text, canTravel: true).title, "Redo")
    }

    func testCommandYForward() {
        func y(_ key: String, shift: Bool = false, option: Bool = false, control: Bool = false,
               command: Bool = true, text: Bool = false) -> Bool {
            JournalKeys.isForwardY(key: key, command: command, shift: shift, option: option, control: control,
                                   textEditing: text)
        }
        XCTAssertTrue(y("y"))
        XCTAssertFalse(y("y", text: true), "⌘Y is no text redo on the Mac: it goes on")
        XCTAssertFalse(y("y", command: false))
        XCTAssertFalse(y("Y", shift: true))
        XCTAssertFalse(y("y", option: true))
        XCTAssertFalse(y("y", control: true))
        XCTAssertFalse(y("z"), "⌘Z belongs to the Edit menu")
    }
}
