import AlmondbreadCore
import SwiftUI

// Guided tours (kb: decisions/controls "Guided tours"): the tours are Markdown files in the Rust
// core (`crates/almondbread-core/tours/<id>.md`), parsed and played in Rust. Here: the card that
// shows the running step, the tour list in the control center and the View menu.

/// The running step: tour title, step counter, the step's title and text, Back / Next and
/// End tour. Compact, at the bottom trailing corner, so the orbit and the view stay visible.
/// Reads only the change-only `model.tour`.
struct TourCard: View {
    let model: ExplorerModel
    var compact = false

    var body: some View {
        if let t = model.tour {
            card(t)
        }
    }

    private func card(_ t: TourStepInfo) -> some View {
        let last = Int(t.index) + 1 >= Int(t.count)
        return VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                Label(t.tourTitle, systemImage: "signpost.right")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Spacer(minLength: 8)
                Text("\(t.index + 1) of \(t.count)")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
            Text(t.title)
                .font(.headline)
                .fixedSize(horizontal: false, vertical: true)
            // Steps are a few short paragraphs (kb: "Guided tours": keep them short).
            HelpMarkdownView(markdown: t.text, compact: true)
                .font(compact ? .footnote : .callout)
                .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 8) {
                Button("End tour") { model.endTour() }
                    .help("End the tour: your view and settings come back (Esc)")
                Spacer()
                Button("‹ Back") { model.tourBack() }
                    .disabled(t.index == 0)
                    .help("The previous step (←)")
                Button(last ? "Finish" : "Next ›") { model.tourNext() }
                    .buttonStyle(.borderedProminent)
                    .help(last ? "End the tour (→)" : "The next step (→)")
            }
            .controlSize(.small)
        }
        .padding(12)
        .frame(width: compact ? nil : 340)
        .frame(maxWidth: compact ? 360 : nil)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.white.opacity(0.12)))
        .shadow(color: .black.opacity(0.35), radius: 10, y: 3)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Tour step \(t.index + 1) of \(t.count): \(t.title)")
    }
}

/// The control center's Tours section: each tour with its summary and a Start button.
struct ToursSectionRows: View {
    let model: ExplorerModel

    var body: some View {
        let _ = UIPerf.body("section.tours")
        let running = model.tour?.tourId
        ForEach(ExplorerModel.tours, id: \.id) { t in
            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(t.title).font(.callout.weight(.semibold))
                    Text(t.summary)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    Text("\(t.steps) steps").font(.caption2).foregroundStyle(.tertiary)
                }
                Spacer(minLength: 4)
                if running == t.id {
                    Button("End") { model.endTour() }
                        .help("End the tour (Esc)")
                } else {
                    Button("Start") { model.startTour(t.id) }
                        .help("Start the tour: → and ← step through it, Esc ends it")
                }
            }
            #if os(iOS)
            .buttonStyle(.borderless)
            #endif
        }
    }
}

/// The tour of a place (its `place` key), if one has one.
extension ExplorerModel {
    static func tour(forPlace id: String) -> TourInfo? {
        tours.first { $0.place == id }
    }
}

#if os(macOS)
/// View ▸ Tours: every tour, and End Tour while one runs.
struct TourMenu: View {
    let model: ExplorerModel?

    var body: some View {
        Menu("Tours") {
            ForEach(ExplorerModel.tours, id: \.id) { t in
                Button(t.title) { model?.startTour(t.id) }
            }
            Divider()
            Button("End Tour (Esc)") { model?.endTour() }
                .disabled(model?.tour == nil)
        }
        .disabled(model == nil)
    }
}

extension UIDemo {
    /// `--ui-demo-tour-wait=3:14,7:12`: seconds before step n's capture (default 9; a moving
    /// probe is captured where its path is at that time).
    static func tourWait(step: Int) -> Double {
        let prefix = "--ui-demo-tour-wait="
        guard let arg = CommandLine.arguments.first(where: { $0.hasPrefix(prefix) }) else { return 9 }
        for pair in arg.dropFirst(prefix.count).split(separator: ",") {
            let kv = pair.split(separator: ":")
            if kv.count == 2, Int(kv[0]) == step, let s = Double(kv[1]) { return s }
        }
        return 9
    }

    /// `--ui-demo=tour-<id>`: starts the tour and captures every step once its flight landed
    /// and the view settled (`window-tour-<id>-<n>.png`), then ends it and captures the restored
    /// view (`-end`); prints each step's orbit text.
    @MainActor
    static func runTour(_ model: ExplorerModel, explorer: Explorer, id: String) {
        model.startTour(id)
        guard model.tour != nil else {
            print("ui-demo: unknown tour \(id)")
            exit(1)
        }
        var entered = Date()
        var ended = false
        Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { timer in
            MainActor.assumeIsolated {
                model.refreshStats()
                guard let w = NSApplication.shared.windows.first(where: { $0.isVisible && $0.contentView != nil
                    && !String(describing: type(of: $0)).contains("Popover") }) else { return }
                let t = Date().timeIntervalSince(entered)
                let flying = model.stats?.flight.active ?? true
                let settled = model.stats?.settled ?? false
                if ended {
                    guard t > 3, !flying, settled || t > 15 else { return }
                    timer.invalidate()
                    capture(window: w, explorer: explorer, scene: "tour-\(id)-end")
                    let v = explorer.viewport()
                    print("ui-demo: tour \(id) done, restored view \(v.re) \(v.scale) look \(model.lookID)")
                    fflush(stdout)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1) { exit(0) }
                    return
                }
                guard let step = model.tour else { return }
                let n = Int(step.index) + 1
                guard t > tourWait(step: n), !flying, settled || t > 25 else { return }
                capture(window: w, explorer: explorer, scene: "tour-\(id)-\(n)")
                let v = explorer.viewport()
                print("tour-demo: step \(n) of \(step.count) \"\(step.title)\" orbit: \(model.orbit?.text ?? "") view \(v.re) \(v.scale)")
                fflush(stdout)
                if n < Int(step.count) {
                    model.tourNext()
                } else {
                    model.endTour()
                    ended = true
                }
                entered = Date()
            }
        }
    }
}
#endif
