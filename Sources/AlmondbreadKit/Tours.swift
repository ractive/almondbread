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
    /// A phone in portrait (narrow canvas): the card sits at the top, its text folds and scrolls.
    var compact = false
    @State private var collapsed = false
    /// The step text's height (the phone's card shows up to `textCap` of it, then scrolls).
    @State private var textHeight: CGFloat = 0
    static let textCap: CGFloat = 150

    var body: some View {
        if let t = model.tour {
            card(t)
        }
    }

    private func card(_ t: TourStepInfo) -> some View {
        let last = Int(t.index) + 1 >= Int(t.count)
        return VStack(alignment: .leading, spacing: compact ? 6 : 8) {
            HStack(alignment: .center) {
                Label(t.tourTitle, systemImage: "signpost.right")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Spacer(minLength: 8)
                Text("\(t.index + 1) of \(t.count)")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
                if compact {
                    // The phone's card folds to its title and buttons, the picture free.
                    Button {
                        withAnimation(.easeOut(duration: 0.2)) { collapsed.toggle() }
                    } label: {
                        Image(systemName: collapsed ? "chevron.down" : "chevron.up")
                            .font(.footnote.weight(.semibold))
                            .frame(width: 44, height: 30)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(collapsed ? "Show the tour text" : "Hide the tour text")
                    .accessibilityIdentifier("tour-fold")
                }
            }
            Text(t.title)
                .font(compact ? .subheadline.weight(.semibold) : .headline)
                .fixedSize(horizontal: false, vertical: true)
            // Steps are a few short paragraphs (kb: "Guided tours": keep them short). On a phone
            // the text scrolls within a third of the screen at most.
            if !(compact && collapsed) {
                if compact {
                    ScrollView {
                        HelpMarkdownView(markdown: t.text, compact: true, textFont: .footnote)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { textHeight = $0 }
                    }
                    .frame(height: min(max(textHeight, 20), Self.textCap))
                    .scrollBounceBehavior(.basedOnSize)
                    .accessibilityIdentifier("tour-text")
                } else {
                    HelpMarkdownView(markdown: t.text, compact: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            if !t.goal.isEmpty {
                TourGoalLine(model: model)
            }
            // The last step offers the next tour (kb: decisions/controls "Finding the tours").
            if last, let next = ExplorerModel.nextTour(after: t.tourId) {
                NextTourButton(model: model, next: next)
            }
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
            #if os(iOS)
            // Finger-sized buttons (kb: decisions/controls "Touch"); ← → on an iPad keyboard too.
            .buttonStyle(.bordered)
            .controlSize(.regular)
            #else
            .controlSize(.small)
            #endif
        }
        .padding(12)
        .frame(width: compact ? nil : 340)
        .frame(maxWidth: compact ? 480 : nil)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.white.opacity(0.12)))
        .shadow(color: .black.opacity(0.35), radius: 10, y: 3)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Tour step \(t.index + 1) of \(t.count): \(t.title)")
    }
}

/// A step's goal (`goal:`): "Try it" until the user did it, then "Well done!".
struct TourGoalLine: View {
    let model: ExplorerModel

    var body: some View {
        let done = model.tourGoalMet
        Label(done ? "Well done!" : "Your turn: try it", systemImage: done ? "checkmark.circle.fill" : "hand.point.up.left")
            .font(.callout.weight(.semibold))
            .foregroundStyle(done ? Color.green : Color.secondary)
            .contentTransition(.symbolEffect(.replace))
            .animation(.easeOut(duration: 0.25), value: done)
            .accessibilityIdentifier("tour-goal")
            .accessibilityValue(done ? "done" : "waiting")
    }
}

/// "Next tour: <title> ›" on a tour's last step: starts that tour right away (the view and
/// settings saved when the first tour started stay the ones the end restores).
struct NextTourButton: View {
    let model: ExplorerModel
    let next: TourInfo

    var body: some View {
        Button {
            model.startTour(next.id)
        } label: {
            HStack(spacing: 8) {
                PlaceThumbnail(id: next.thumbnail)
                    .aspectRatio(1.6, contentMode: .fill)
                    .frame(width: 48, height: 30)
                    .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
                VStack(alignment: .leading, spacing: 1) {
                    Text("Next tour").font(.caption2).foregroundStyle(.secondary)
                    Text(next.title).font(.callout.weight(.semibold)).lineLimit(1)
                }
                Spacer(minLength: 4)
                Image(systemName: "chevron.right").font(.footnote.weight(.semibold)).foregroundStyle(.secondary)
            }
            .padding(6)
            .background(.primary.opacity(0.07), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
        .buttonStyle(.plain)
        .help("Start the next tour, “\(next.title)”")
        .accessibilityLabel("Next tour: \(next.title)")
        .accessibilityIdentifier("tour-next-tour")
    }
}

/// A small link where a question comes up ("What's a period?" under "Minibrot, period 34"): it
/// starts the listed tour whose `topics:` has `topic` (kb: decisions/controls "Finding the
/// tours"). Nothing when no tour covers it (not bundled yet) or that tour is running.
struct TourTopicLink: View {
    let model: ExplorerModel
    let topic: String
    /// nil: "Tour: <the tour's title>".
    let label: String?

    var body: some View {
        if let t = ExplorerModel.tour(forTopic: topic), model.tour?.tourId != t.id {
            Button {
                model.startTour(t.id)
            } label: {
                Label(label ?? "Tour: \(t.title)", systemImage: "signpost.right")
                    .font(.caption)
                    .foregroundStyle(.tint)
                    #if os(iOS)
                    .frame(minHeight: 30)
                    .contentShape(Rectangle())
                    #endif
            }
            .buttonStyle(.borderless)
            .help("Take the tour “\(t.title)” (\(t.lengthText))")
            .accessibilityIdentifier("tour-topic-" + topic)
        }
    }
}

extension TourInfo {
    /// "9 steps · ~3 min".
    var lengthText: String { "\(steps) steps · ~\(minutes) min" }
}

/// The control center's Tours section: the walkthrough first, then a card per tour: a place's
/// thumbnail, title, summary, length, a checkmark once done, and Start.
struct ToursSectionRows: View {
    let model: ExplorerModel

    var body: some View {
        let _ = UIPerf.body("section.tours")
        let running = model.tour?.tourId
        let done = model.toursDone
        if let w = ExplorerModel.walkthrough {
            row(w, title: "Walkthrough: how to use the app", summary: "The controls, step by step.",
                running: running, done: done.contains(w.id))
        }
        ForEach(ExplorerModel.tours, id: \.id) { t in
            row(t, title: t.title, summary: t.summary, running: running, done: done.contains(t.id))
        }
    }

    private func row(_ t: TourInfo, title: String, summary: String, running: String?, done: Bool) -> some View {
        HStack(alignment: .top, spacing: 10) {
            PlaceThumbnail(id: t.thumbnail)
                .aspectRatio(1.6, contentMode: .fill)
                .frame(width: 64, height: 40)
                .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                .overlay(alignment: .topTrailing) {
                    if done {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 14, weight: .semibold))
                            .symbolRenderingMode(.palette)
                            .foregroundStyle(.white, .green)
                            .padding(2)
                    }
                }
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.callout.weight(.semibold))
                    .fixedSize(horizontal: false, vertical: true)
                Text(summary)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
                Text(done ? t.lengthText + " · done" : t.lengthText)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .accessibilityIdentifier("tour-length-" + t.id)
            }
            Spacer(minLength: 4)
            if running == t.id {
                Button("End") { model.endTour() }
                    .help("End the tour (Esc)")
            } else {
                Button("Start") { model.startTour(t.id) }
                    .help("Start the tour: → and ← step through it, Esc ends it")
                    .accessibilityIdentifier("tour-start-" + t.id)
            }
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
    }
}

/// The tour of a place (one of its `place:` ids), if one has one.
extension ExplorerModel {
    static func tour(forPlace id: String) -> TourInfo? {
        tours.first { $0.places.contains(id) }
    }
}

/// View ▸ Tours (the Mac's and the iPad's menu bar): every tour, and End Tour while one runs.
struct TourMenu: View {
    let model: ExplorerModel?

    var body: some View {
        Menu("Tours") {
            if ExplorerModel.walkthrough != nil {
                Button("Walkthrough") { model?.startTour(ExplorerModel.walkthroughID) }
                Divider()
            }
            ForEach(ExplorerModel.tours, id: \.id) { t in
                // A checkmark once done (kb: decisions/controls "Finding the tours").
                if model?.toursDone.contains(t.id) == true {
                    Button { model?.startTour(t.id) } label: { Label(t.title, systemImage: "checkmark") }
                } else {
                    Button(t.title) { model?.startTour(t.id) }
                }
            }
            Divider()
            Button("End Tour (Esc)") { model?.endTour() }
                .disabled(model?.tour == nil)
        }
        .disabled(model == nil)
    }
}

#if os(macOS)
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
                print("tour-demo: step \(n) of \(step.count) \"\(step.title)\" \(settled ? "settled" : "not settled") after \(String(format: "%.1f", t)) s, orbit: \(model.orbit?.text ?? "") view \(v.re) \(v.scale)")
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
