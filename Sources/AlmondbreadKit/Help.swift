import SwiftUI

// Help in layers (kb: decisions/controls, "Help"): (?) popovers next to features, one help
// window (macOS: Help ▸ Almondbread Help, ⌘?; iOS: a sheet), and the texts as Markdown files
// in Resources/Help, so the same files can become a website. The (?) buttons are static views:
// they read nothing from the model (kb: platform/swiftui-performance).

/// A help window page: one Markdown file, `Resources/Help/<rawValue>.md`, its title the `# `
/// heading.
enum HelpPage: String, CaseIterable, Identifiable, Hashable {
    case gettingStarted = "1-getting-started"
    case seeing = "2-what-am-i-seeing"
    case travel = "3-autopilot-journeys-places"
    case overlays = "4-overlays"
    case deepZoom = "5-deep-zoom"
    case tips = "6-tips"

    var id: String { rawValue }

    var title: String { HelpContent.page(self).title }

    var symbol: String {
        switch self {
        case .gettingStarted: "hand.wave"
        case .seeing: "eye"
        case .travel: "airplane"
        case .overlays: "square.on.square.dashed"
        case .deepZoom: "plus.magnifyingglass"
        case .tips: "lightbulb"
        }
    }
}

/// What a (?) explains. The raw value is the section heading in `popovers.md` and, for the
/// Rust-drawn badges, the topic id Rust reports (`HelpBadgeInfo.topic`).
enum HelpTopic: String, CaseIterable, Identifiable {
    case cosmicScale = "cosmic-scale"
    case piMeter = "pi-meter"
    case miniMap = "mini-map"
    case orbit
    case cockpit
    case iterations
    case look
    case interior
    case overlays
    case controls

    var id: String { rawValue }

    var title: String {
        switch self {
        case .cosmicScale: "Cosmic size scale"
        case .piMeter: "π meter"
        case .miniMap: "Mini-map"
        case .orbit: "Orbit"
        case .cockpit: "Driving levels"
        case .iterations: "Iterations"
        case .look: "Look & colour"
        case .interior: "Interior"
        case .overlays: "Overlays"
        case .controls: "Controls"
        }
    }

    /// The help window page "More…" opens.
    var page: HelpPage {
        switch self {
        case .cosmicScale, .piMeter, .miniMap, .orbit, .overlays: .overlays
        case .iterations, .look, .interior: .seeing
        case .controls: .gettingStarted
        case .cockpit: .travel
        }
    }
}

/// The bundled Markdown, read once.
enum HelpContent {
    static func markdown(_ name: String) -> String {
        guard let url = Bundle.module.url(forResource: name, withExtension: "md", subdirectory: "Help"),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return "" }
        return text
    }

    private static let pages: [HelpPage: (title: String, body: String)] = {
        var out: [HelpPage: (String, String)] = [:]
        for p in HelpPage.allCases {
            let text = markdown(p.rawValue)
            let title = text.split(separator: "\n").first { $0.hasPrefix("# ") }.map { String($0.dropFirst(2)) }
            out[p] = (title ?? p.rawValue, text)
        }
        return out
    }()

    /// `popovers.md` split at its `## id` headings.
    private static let popovers: [String: String] = {
        var out: [String: String] = [:]
        var id: String?
        var lines: [Substring] = []
        func flush() {
            if let id { out[id] = lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines) }
        }
        for line in markdown("popovers").split(separator: "\n", omittingEmptySubsequences: false) {
            if line.hasPrefix("## ") {
                flush()
                id = line.dropFirst(3).trimmingCharacters(in: .whitespaces)
                lines = []
            } else if id != nil {
                lines.append(line)
            }
        }
        flush()
        return out
    }()

    static func page(_ p: HelpPage) -> (title: String, body: String) { pages[p] ?? (p.rawValue, "") }
    static func popover(_ t: HelpTopic) -> String { popovers[t.rawValue] ?? "" }
}

/// Which page the help window shows; the iOS sheet's presentation. Change-only.
@MainActor @Observable
final class HelpCenter {
    static let shared = HelpCenter()
    var page: HelpPage = .gettingStarted
    /// iOS: the help sheet is up.
    var sheetShown = false
}

/// Opens the help window (macOS) or sheet (iOS) at a page.
struct HelpOpener {
    #if os(macOS)
    let openWindow: OpenWindowAction
    #endif

    @MainActor
    func callAsFunction(_ page: HelpPage) {
        HelpCenter.shared.page = page
        #if os(macOS)
        openWindow(id: HelpWindow.id)
        #else
        HelpCenter.shared.sheetShown = true
        #endif
    }
}

// MARK: Markdown

/// A light Markdown renderer for the help files: `#`/`##`/`###` headings, `- ` bullets,
/// paragraphs (lines joined until a blank line); inline emphasis, code and links through
/// `AttributedString`. Enough for the help texts, and the same files read fine on a website.
struct HelpMarkdownView: View {
    let markdown: String
    /// Popovers: smaller type, no title heading.
    var compact = false

    enum Block: Hashable {
        case heading(Int, String)
        case bullet(String)
        case paragraph(String)
    }

    static func blocks(_ text: String) -> [Block] {
        var out: [Block] = []
        var para: [String] = []
        func flush() {
            if !para.isEmpty { out.append(.paragraph(para.joined(separator: " "))) }
            para = []
        }
        for raw in text.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty {
                flush()
            } else if let level = ["# ", "## ", "### "].firstIndex(where: { line.hasPrefix($0) }) {
                flush()
                out.append(.heading(level + 1, String(line.dropFirst(level + 2))))
            } else if line.hasPrefix("- ") || line.hasPrefix("* ") {
                flush()
                out.append(.bullet(String(line.dropFirst(2))))
            } else if case .bullet(let b)? = out.last, para.isEmpty, raw.hasPrefix("  ") {
                // A bullet's continuation line.
                out[out.count - 1] = .bullet(b + " " + line)
            } else {
                para.append(line)
            }
        }
        flush()
        return out
    }

    static func inline(_ s: String) -> AttributedString {
        (try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
            ?? AttributedString(s)
    }

    var body: some View {
        let blocks = Self.blocks(markdown)
        VStack(alignment: .leading, spacing: compact ? 7 : 10) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { i, block in
                switch block {
                case .heading(let level, let text):
                    Text(Self.inline(text))
                        .font(level == 1 ? .largeTitle.bold() : level == 2 ? .title2.bold() : .headline)
                        .padding(.top, i == 0 ? 0 : (level == 1 ? 4 : 10))
                case .bullet(let text):
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text("•").foregroundStyle(.secondary)
                        Text(Self.inline(text)).fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(.leading, 4)
                case .paragraph(let text):
                    Text(Self.inline(text)).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .font(compact ? .callout : .body)
        .lineSpacing(compact ? 1 : 2.5)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: Popovers

/// A (?) popover: the topic's title, its few lines, and "More…" to the help page.
struct HelpPopoverContent: View {
    let topic: HelpTopic
    @Environment(\.dismiss) private var dismiss
    #if os(macOS)
    @Environment(\.openWindow) private var openWindow
    #endif

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(topic.title).font(.headline)
            HelpMarkdownView(markdown: HelpContent.popover(topic), compact: true)
            HStack {
                Spacer()
                Button("More…") { more() }
                    #if os(macOS)
                    .buttonStyle(.link)
                    #else
                    .buttonStyle(.borderless)
                    #endif
            }
        }
        .padding(14)
        .frame(width: 300, alignment: .leading)
        .fixedSize(horizontal: false, vertical: true)
    }

    private func more() {
        dismiss()
        let page = topic.page
        #if os(macOS)
        let open = HelpOpener(openWindow: openWindow)
        #else
        let open = HelpOpener()
        #endif
        // After the popover has gone (iOS cannot present the sheet over it).
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { open(page) }
    }
}

/// A (?) next to a feature: a static button with its own popover. Reads no model state.
struct HelpButton: View {
    let topic: HelpTopic
    @State private var shown = false

    var body: some View {
        Button {
            shown.toggle()
        } label: {
            Image(systemName: "questionmark.circle")
                .font(.callout)
        }
        .buttonStyle(.plain)
        .foregroundStyle(.secondary)
        .help("About \(topic.title.lowercased())")
        .accessibilityLabel("Help: \(topic.title)")
        .popover(isPresented: $shown, arrowEdge: .bottom) {
            HelpPopoverContent(topic: topic)
                .presentationCompactAdaptation(.popover)
        }
        .task {
            // `--ui-demo=help-control`: the iterations popover opens by itself for the screenshot.
            guard UIDemo.scene == "help-control", topic == .iterations else { return }
            try? await Task.sleep(for: .seconds(2))
            shown = true
        }
    }
}

/// A press on a Rust-drawn badge: the topic and the badge's rect in the canvas's own
/// coordinates (points, top left), where the popover attaches.
struct CanvasHelp: Identifiable, Equatable {
    let id = UUID()
    let topic: HelpTopic
    let rect: CGRect
    /// The popover's side: above a badge in the lower half of the canvas, below one in the upper.
    let edge: Edge
}

/// The canvas's popover for its Rust-drawn badges (cosmic scale, π meter, mini-map). Reads
/// only `canvasHelp`, which changes on a press on a badge and on dismissal.
struct CanvasHelpPopover: ViewModifier {
    @Bindable var model: ExplorerModel

    func body(content: Content) -> some View {
        // The last badge stays the anchor while the popover closes (the item is nil by then).
        let anchor = model.canvasHelp ?? model.lastCanvasHelp
        content.popover(item: $model.canvasHelp, attachmentAnchor: .rect(.rect(anchor?.rect ?? .zero)),
                        arrowEdge: anchor?.edge ?? .top) { help in
            HelpPopoverContent(topic: help.topic)
                .presentationCompactAdaptation(.popover)
        }
    }
}

// MARK: The help window / sheet

#if os(macOS)
/// The help window scene: Help ▸ Almondbread Help (⌘?), and "More…" in every popover.
public struct HelpWindow: Scene {
    static let id = "help"
    public init() {}

    public var body: some Scene {
        Window("Almondbread Help", id: Self.id) {
            HelpView()
                .preferredColorScheme(.dark)
        }
        .defaultSize(width: 820, height: 640)
    }
}

/// The Help menu's item.
struct HelpMenuItem: View {
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("Almondbread Help") { HelpOpener(openWindow: openWindow)(.gettingStarted) }
            .keyboardShortcut("?", modifiers: .command)
    }
}
#endif

/// The help pages: a sidebar of pages and the page (macOS); a page with a page menu (iOS).
struct HelpView: View {
    @Bindable private var center = HelpCenter.shared
    #if os(iOS)
    @Environment(\.dismiss) private var dismiss
    #endif

    var body: some View {
        #if os(macOS)
        NavigationSplitView {
            List(HelpPage.allCases, selection: Binding(get: { center.page }, set: { if let p = $0 { center.page = p } })) { p in
                Label(p.title, systemImage: p.symbol).tag(p)
            }
            .frame(minWidth: 250)
            .navigationSplitViewColumnWidth(min: 250, ideal: 260, max: 320)
        } detail: {
            page
        }
        #else
        NavigationStack {
            page
                .navigationTitle(center.page.title)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        Menu {
                            Picker("Page", selection: $center.page) {
                                ForEach(HelpPage.allCases) { Label($0.title, systemImage: $0.symbol).tag($0) }
                            }
                        } label: {
                            Label("Pages", systemImage: "list.bullet")
                        }
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") { dismiss() }
                    }
                }
        }
        #endif
    }

    private var page: some View {
        ScrollView {
            HelpMarkdownView(markdown: HelpContent.page(center.page).body)
                .textSelection(.enabled)
                .frame(maxWidth: 640, alignment: .leading)
                .padding(.horizontal, 28)
                .padding(.vertical, 24)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .id(center.page)
    }
}

#if os(iOS)
/// iOS: the help sheet (opened by "More…").
struct HelpSheet: ViewModifier {
    @Bindable private var center = HelpCenter.shared

    func body(content: Content) -> some View {
        content.sheet(isPresented: $center.sheetShown) {
            HelpView()
        }
    }
}
#endif
