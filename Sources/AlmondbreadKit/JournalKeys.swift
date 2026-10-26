import Foundation

/// The travel journal's keys (kb: decisions/controls "Travel journal"): ⌘Z back, ⇧⌘Z and ⌘Y
/// forward, on the canvas. A text field being edited (the control center's search, the coordinate
/// fields) keeps ⌘Z and ⇧⌘Z for its own undo: the canvas's key monitor (macOS) sends them as
/// `undo:` / `redo:` up the field editor's responder chain itself (the menu bar's Edit ▸ Undo no
/// longer carries ⌘Z, View ▸ Back does); on iOS the canvas's key commands are not in a focused
/// text field's responder chain at all.
/// Policy for where to travel lives in Rust (`Explorer::journal_back` / `journal_forward`).
enum JournalKeys {
    enum Step: Equatable {
        /// The journal's back / forward.
        case back, forward
        /// The text field's own undo / redo.
        case textUndo, textRedo
    }

    /// What a key press asks for, or nil (the key goes on as before). `key`: the characters
    /// ignoring modifiers ("z", or "Z" with Shift); ⌥ or ⌃ with it is something else.
    static func step(key: String, command: Bool, shift: Bool, option: Bool, control: Bool,
                     textEditing: Bool) -> Step? {
        guard command, !option, !control else { return nil }
        switch key.lowercased() {
        case "z":
            if textEditing { return shift ? .textRedo : .textUndo }
            return shift ? .forward : .back
        case "y": return shift || textEditing ? nil : .forward
        default: return nil
        }
    }
}
