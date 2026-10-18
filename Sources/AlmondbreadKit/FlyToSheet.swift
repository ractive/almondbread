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
            Text(Self.explanation)
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
                #if os(iOS)
                PasteButton(payloadType: String.self) { strings in
                    if let text = strings.first, let c = Self.parse(text) { (re, im, scale) = c }
                }
                .labelStyle(.iconOnly)
                #endif
                Spacer()
                Button("Cancel") { isPresented = false }
                    .keyboardShortcut(.cancelAction)
                Button("Fly") { fly() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(re.isEmpty || im.isEmpty || scale.isEmpty)
                    #if os(iOS)
                    .buttonStyle(.borderedProminent)
                    #endif
            }
            #if os(iOS)
            // Finger-sized (kb: decisions/controls "Touch").
            .buttonStyle(.bordered)
            .controlSize(.large)
            #endif
        }
        .padding(20)
        #if os(macOS)
        .frame(minWidth: 420)
        #else
        // A phone is 375-440 points wide: a 420-point minimum clipped the fields and the buttons.
        .frame(maxWidth: 520, maxHeight: .infinity, alignment: .top)
        .presentationDetents([.medium, .large])
        #endif
        .onAppear {
            // Start from the clipboard if it holds coordinates. Not on iOS: reading another
            // app's clipboard asks "Allow Paste?" every time the sheet opens (seen 2026-10-18);
            // the Paste button there reads it on request without asking.
            #if os(macOS)
            if let text = Clipboard.text(), let c = Self.parse(text) { (re, im, scale) = c }
            #endif
        }
    }

    #if os(macOS)
    static let explanation = "Paste copied coordinates (⇧⌘C) or type the centre and the scale (complex units per point). "
        + "Q / E or scrolling change the speed; Esc stops; any navigation takes over."
    #else
    static let explanation = "Paste copied coordinates or type the centre and the scale (complex units per point). "
        + "The throttle changes the speed; a double tap on it stops; any touch on the picture takes over."
    #endif

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
