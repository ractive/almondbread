import Foundation

/// The travel journal's keys (kb: decisions/controls "Travel journal"), pure logic.
///
/// macOS: ⌘Z / ⇧⌘Z are the Edit menu's one Undo / Redo pair, whose meaning follows the context:
/// while a text field edits (in any window: the main window, the Settings window, the Fly-to
/// sheet) they are the field's own undo / redo (`undo:` / `redo:` up the responder chain, titles
/// from its undo manager, e.g. "Undo Typing"); otherwise they read "Back" / "Forward" and step
/// the journal. ⌘Y is an extra forward key, taken by the canvas's key monitor (no menu item).
/// iPad: the canvas's key commands (a focused text field is not in their responder chain).
/// Policy for where to travel lives in Rust (`Explorer::journal_back` / `journal_forward`).
enum JournalKeys {
    /// A text field's undo state while it edits: its undo manager's menu titles and abilities.
    struct TextUndo: Equatable {
        var undoTitle: String
        var redoTitle: String
        var canUndo: Bool
        var canRedo: Bool
    }

    /// What one of the Edit menu's pair does.
    enum Action: Equatable {
        /// The text field's own undo / redo.
        case text
        /// The journal's back / forward.
        case journal
    }

    struct MenuItem: Equatable {
        var title: String
        var enabled: Bool
        var action: Action
    }

    /// The Edit menu's Undo (`redo` false) or Redo item: the text field's while one edits
    /// (`text` non-nil), otherwise the journal's Back / Forward (`canTravel`: there is a place to
    /// go to). An empty title from the undo manager falls back to "Undo" / "Redo".
    static func menuItem(redo: Bool, text: TextUndo?, canTravel: Bool) -> MenuItem {
        if let text {
            let title = redo ? text.redoTitle : text.undoTitle
            return MenuItem(title: title.isEmpty ? (redo ? "Redo" : "Undo") : title,
                            enabled: redo ? text.canRedo : text.canUndo, action: .text)
        }
        return MenuItem(title: redo ? "Forward" : "Back", enabled: canTravel, action: .journal)
    }

    /// Whether a key press is ⌘Y, the extra forward key (not while a text field edits: there it
    /// goes on). `key`: the characters ignoring modifiers; ⇧, ⌥ or ⌃ with it is something else.
    /// ⌘Z / ⇧⌘Z are not keys of the canvas: the Edit menu owns them.
    static func isForwardY(key: String, command: Bool, shift: Bool, option: Bool, control: Bool,
                           textEditing: Bool) -> Bool {
        command && !shift && !option && !control && !textEditing && key.lowercased() == "y"
    }
}
