import SwiftUI

/// "Fly to Coordinates…": re, im and scale (decimal strings, any depth) for an animated flight
/// (kb: decisions/controls, fly to). Pasting the app's copied-coordinates text into any field
/// fills all three. The flight itself (path, speed, landing) is Rust's.
struct FlyToSheet: View {
    let model: ExplorerModel
    @Binding var isPresented: Bool
    @State private var re = ""
    @State private var im = ""
    @State private var scale = ""
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Fly to Coordinates").font(.headline)
            Text("Paste copied coordinates (⇧⌘C) or type the centre and the scale (complex units per point). "
                 + "Q / E or scrolling change the speed; Esc stops; any navigation takes over.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            field("re", $re)
            field("im", $im)
            field("scale", $scale)
            if let error {
                Text(error).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button("Cancel") { isPresented = false }
                    .keyboardShortcut(.cancelAction)
                Button("Fly") { fly() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(re.isEmpty || im.isEmpty || scale.isEmpty)
            }
        }
        .padding(20)
        .frame(minWidth: 420)
        .onAppear {
            // Start from the clipboard if it holds coordinates.
            if let text = Clipboard.text(), let c = Self.parse(text) { (re, im, scale) = c }
        }
    }

    private func field(_ label: String, _ value: Binding<String>) -> some View {
        LabeledContent(label) {
            TextField(label, text: value)
                .labelsHidden()
                .textFieldStyle(.roundedBorder)
                .font(.body.monospaced())
                .onChange(of: value.wrappedValue) { _, new in
                    // A pasted block ("re …\nim …\nscale …") fills all three fields.
                    if new.contains("\n") || new.contains(" "), let c = Self.parse(new) { (re, im, scale) = c }
                }
        }
    }

    private func fly() {
        do {
            try model.flyTo(re: re, im: im, scale: scale)
            isPresented = false
        } catch {
            self.error = "Not a view: \(error)"
        }
    }

    /// "re …", "im …", "scale … (zoom …)" lines, or three numbers.
    static func parse(_ text: String) -> (String, String, String)? {
        var re: String?, im: String?, scale: String?
        for line in text.split(whereSeparator: \.isNewline) {
            let parts = line.split(whereSeparator: \.isWhitespace)
            guard parts.count >= 2 else { continue }
            switch parts[0] {
            case "re": re = String(parts[1])
            case "im": im = String(parts[1])
            case "scale": scale = String(parts[1])
            default: break
            }
        }
        if let re, let im, let scale { return (re, im, scale) }
        let words = text.split { $0.isWhitespace || $0 == "," || $0 == ";" }
        return words.count == 3 ? (String(words[0]), String(words[1]), String(words[2])) : nil
    }
}
