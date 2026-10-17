import AlmondbreadCore
import SwiftUI
import ImageIO
import UniformTypeIdentifiers

/// The control center: every per-window setting in grouped, searchable sections (kb:
/// decisions/controls, "Control center"). A trailing sidebar on the Mac and iPad (toolbar
/// button, ⌘I), a sheet on iPhone. App-wide preferences live in the Settings window (⌘,).
///
/// Performance (kb: platform/swiftui-performance): the sidebar holds only what can be set, so
/// nothing in it follows the 10 Hz stats (live values are in the HUD and its developer
/// details); buttons read change-only model properties. This body reads no model property,
/// each section is its own view struct (equal inputs: SwiftUI skips its body when another
/// section opens or the query changes). On the Mac the sections sit in a `LazyVStack` styled
/// like a grouped form: a grouped `Form` there is a non-lazy stack that lays out every open row
/// on any change (75-410 ms per section toggle). iOS keeps its `Form` (a collection view, lazy).
/// The search field keeps its own text; the sections filter on a debounced copy.
struct ControlCenterView: View {
    let model: ExplorerModel
    /// The applied query (the field's text, debounced).
    @State private var query = UIDemo.search ?? ""
    @State private var confirmReset = false
    /// The sidebar's width (the tile grids' columns follow it).
    @State private var sidebarWidth: CGFloat = 330
    @Environment(\.undoManager) private var undoManager
    @AppStorage("controlCenter.collapsed", store: UIDemo.enabled ? UIDemo.defaults : .standard)
    private var collapsedStorage = ""

    var body: some View {
        let _ = UIPerf.body("ControlCenterView")
        let collapsed = Set(collapsedStorage.split(separator: ",").map(String.init))
        let searching = !query.isEmpty
        let shown = ControlSection.allCases.filter { $0.matches(query) }
        sections(shown: shown, collapsed: searching ? [] : collapsed)
        .controlSize(.small)
        .safeAreaInset(edge: .top, spacing: 0) {
            ControlSearchField(query: $query)
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            resetAllBar
        }
        .onReceive(UIPerf.actions) { action in
            if case .toggle(let s) = action { withAnimation(.snappy(duration: 0.2)) { toggle(s) } }
        }
        #if os(macOS)
        // On the Mac the canvas runs on under the sidebar: the π panel (bottom right) moves
        // left of it while it shows.
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: {
            model.piMeterInsetRight = Double($0)
            sidebarWidth = $0
        }
        .onDisappear { model.piMeterInsetRight = 0 }
        #endif
    }

    #if os(macOS)
    private func sections(shown: [ControlSection], collapsed: Set<String>) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                // Every row is a child of the lazy stack (ControlCardRows), so opening a section
                // builds and lays out only the rows that come into view.
                ForEach(shown) { section in
                    let open = !collapsed.contains(section.rawValue)
                    ControlSectionHeader(section: section, model: model, open: open, undoManager: undoManager) {
                        withAnimation(.snappy(duration: 0.2)) { toggle(section) }
                    }
                    .padding(.horizontal, 6)
                    .padding(.bottom, open ? 6 : 8)
                    if open {
                        ControlCardRows {
                            ControlSectionRows(section: section, model: model)
                        }
                        if let f = section.footer {
                            Text(f)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                                .padding(.horizontal, 6)
                                .padding(.top, 6)
                        }
                        Color.clear.frame(height: 16)
                    }
                }
                if shown.isEmpty {
                    Text("No setting matches “\(query)”.").foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, 14)
            .padding(.top, 10)
        }
        .labeledContentStyle(ControlRowLabeledStyle())
        .toggleStyle(ControlRowToggleStyle())
        .environment(\.controlRowWidth, max(100, sidebarWidth - 48))
    }
    #else
    private func sections(shown: [ControlSection], collapsed: Set<String>) -> some View {
        Form {
            ForEach(shown) { section in
                let open = !collapsed.contains(section.rawValue)
                Section {
                    if open {
                        ControlSectionRows(section: section, model: model)
                    }
                } header: {
                    ControlSectionHeader(section: section, model: model, open: open, undoManager: undoManager) {
                        withAnimation(.snappy(duration: 0.2)) { toggle(section) }
                    }
                } footer: {
                    if open, let f = section.footer {
                        Text(f)
                    }
                }
            }
            if shown.isEmpty {
                Text("No setting matches “\(query)”.").foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }
    #endif

    private func toggle(_ s: ControlSection) {
        var c = Set(collapsedStorage.split(separator: ",").map(String.init))
        if c.contains(s.rawValue) { c.remove(s.rawValue) } else { c.insert(s.rawValue) }
        collapsedStorage = c.sorted().joined(separator: ",")
    }

    /// "Reset all…" at the bottom: settings only, or settings and the view.
    private var resetAllBar: some View {
        HStack {
            Spacer()
            Button(role: .destructive) {
                confirmReset = true
            } label: {
                Label("Reset all…", systemImage: "arrow.counterclockwise.circle")
            }
            .help("Every setting of this window back to the defaults (⌘Z undoes)")
            .confirmationDialog("Reset all settings?", isPresented: $confirmReset, titleVisibility: .visible) {
                Button("Reset settings") { model.reset(.all, undo: undoManager) }
                Button("Reset settings and view") { model.reset(.all, withView: true, undo: undoManager) }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Look, palette, lighting, interior, overlays, iterations and quality go back to the defaults. Edit ▸ Undo (⌘Z) brings them back. App-wide preferences stay (Settings, ⌘,).")
            }
            Spacer()
        }
        .padding(.vertical, 8)
        .background(.bar)
    }
}

/// The search field. It owns the typed text, so a keystroke re-renders only the field; the
/// form gets the text as `query` once typing pauses (120 ms; clearing applies at once).
/// Filtering is cheap, but every changed query inserts or removes whole sections, and the
/// first character opens every matching one (measured 0.5-0.8 s per such keystroke before).
struct ControlSearchField: View {
    @Binding var query: String
    @State private var text = UIDemo.search ?? ""

    var body: some View {
        let _ = UIPerf.body("ControlSearchField")
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Search settings", text: $text)
                .textFieldStyle(.plain)
            if !text.isEmpty {
                Button {
                    text = ""
                } label: {
                    Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Clear search")
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.bar)
        .task(id: text) {
            let q = text.trimmingCharacters(in: .whitespaces)
            if !q.isEmpty { try? await Task.sleep(for: .milliseconds(120)) }
            if !Task.isCancelled, q != query { query = q }
        }
        .onReceive(UIPerf.actions) { action in
            if case .search(let s) = action { text = s }
        }
    }
}

/// A section's header: title (tap: open or close) and its ↺.
struct ControlSectionHeader: View {
    let section: ControlSection
    let model: ExplorerModel
    let open: Bool
    let undoManager: UndoManager?
    let toggle: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Button(action: toggle) {
                HStack {
                    // Primary and headline weight: Form section headers default to small secondary text,
                    // which read weaker than the rows inside (user, 2026-10-16).
                    Label(section.title, systemImage: section.symbol)
                        .font(.headline)
                        .foregroundStyle(.primary)
                    Spacer()
                    Image(systemName: "chevron.right")
                        .rotationEffect(.degrees(open ? 90 : 0))
                        .foregroundStyle(.tertiary)
                        .font(.caption)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(.isHeader)
            .accessibilityHint(open ? "Collapses the section" : "Expands the section")
            if let topic = section.helpTopic {
                HelpButton(topic: topic)
            }
            if let scope = section.resetScope {
                Button {
                    model.reset(scope, undo: undoManager)
                } label: {
                    Image(systemName: "arrow.counterclockwise").font(.caption)
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
                .help("Reset \(section.title.lowercased()) to the defaults (⌘Z undoes)")
                .accessibilityLabel("Reset \(section.title)")
            }
        }
    }
}

/// One section's rows. Its inputs are a section and the model reference, so SwiftUI skips its
/// body unless the model properties it read change (an accordion toggle used to re-run every
/// open section's rows, the palette grid and the thumbnail grid included).
struct ControlSectionRows: View {
    let section: ControlSection
    let model: ExplorerModel

    var body: some View {
        let _ = UIPerf.body("rows.\(section.rawValue)")
        switch section {
        case .places: PlacesSection(model: model)
        case .tours: ToursSectionRows(model: model)
        case .view: ViewSectionRows(model: model)
        case .journey: JourneySectionRows(model: model)
        case .look: LookSectionRows(model: model)
        case .lighting: LightingSectionRows(model: model)
        case .interior: InteriorSectionRows(model: model)
        case .overlays: OverlaySectionRows(model: model)
        case .quality: QualitySectionRows(model: model)
        case .controls: ControlsHelp(model: model)
        }
    }
}

// MARK: View & navigation

struct ViewSectionRows: View {
    @Bindable var model: ExplorerModel
    @State private var showFlyTo = false

    var body: some View {
        let _ = UIPerf.body("section.view")
        HStack {
            Button {
                Clipboard.copy(model.coordinatesText)
            } label: {
                Label("Copy", systemImage: "doc.on.doc")
            }
            .help("Copy the centre and scale at full precision (⇧⌘C)")
            Button {
                model.pasteCoordinates()
            } label: {
                Label("Go to copied", systemImage: "arrow.right.doc.on.clipboard")
            }
            .help("Jump to coordinates on the clipboard (⇧⌘V)")
            Spacer()
            Button {
                model.reset()
            } label: {
                Label("Reset", systemImage: "arrow.counterclockwise")
            }
            .help("Back to the whole set (R, ⌘0)")
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
        ControlRows.picker("Edge panning", selection: $model.edgePanning) {
            ForEach(EdgePanning.allCases) { Text($0.label).tag($0) }
        }
        // Autopilot and fly-to (kb: decisions/controls, autopilot).
        HStack {
            Button {
                model.toggleAutopilot()
            } label: {
                Label(model.autopilotActive ? "Stop" : "Autopilot",
                      systemImage: model.autopilotActive ? "airplane.circle.fill" : "airplane")
            }
            .help("Zoom on continuously towards interesting places; click pauses, Esc stops, steering hands over to cruise (Z)")
            if model.flightActive {
                Button {
                    model.stopMotion()
                } label: {
                    Label("Stop flight", systemImage: "stop.circle")
                }
                .help("Stop the flight (Esc)")
            } else {
                Button {
                    showFlyTo = true
                } label: {
                    Label("Fly to…", systemImage: "location.north.line")
                }
                .help("Fly to coordinates (⌘G)")
                Button {
                    model.flyToCopiedCoordinates()
                } label: {
                    Label("Copied", systemImage: "doc.on.clipboard")
                }
                .help("Fly to coordinates on the clipboard (⇧⌘G)")
            }
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
        .sheet(isPresented: $showFlyTo) { FlyToSheet(model: model, isPresented: $showFlyTo) }
        Toggle("Autopilot target lock", isOn: $model.autopilotLock)
            .help("The autopilot aims at spirals, branch points and minibrots, and backs out of featureless regions. Off: it flies straight. Steering yourself hands over to cruise")
        // Lane assist (kb: decisions/controls "Driving levels"): cruise only, key L.
        Toggle("Lane assist", isOn: $model.laneAssist)
            .help("While you cruise (E / Q), keeps you out of featureless areas with a gentle nudge; your steering always wins (L)")
        #if os(macOS)
        Picker("Two-finger scroll", selection: $model.scrollZooms) {
            Text("Pan").tag(false)
            Text("Zoom").tag(true)
        }
        .help("On a trackpad: two fingers pan the view, or zoom (up and down; then click and drag pans). Pinching always zooms")
        #endif
        // View rotation (kb: decisions/controls, "View rotation"); the compass shows the angle.
        LabeledContent("Rotation") {
            Button("Straighten") { model.straighten() }
                .disabled(!model.viewTurned)
                .help("Turn the view upright (/); , and . turn it, so does a trackpad rotation")
        }
    }
}

// MARK: Journey & cinematic effects

struct JourneySectionRows: View {
    @Bindable var model: ExplorerModel
    @State private var journeySeed = ""

    var body: some View {
        let _ = UIPerf.body("section.journey")
        JourneyControls(model: model, journeySeed: journeySeed)
        Picker("Journey", selection: $model.journeyStops) {
            Text("Continuous").tag(false)
            Text("Stop at each find").tag(true)
        }
        .help("Continuous: rest a moment at each landmark and fly on. Stop at each find: stay until you choose: J next, Z deeper here, X somewhere else. J starts this mode, ⇧J the other.")
        TextField("Seed (empty: random)", text: $journeySeed)
            .help("A journey number, e.g. 4711; started from the whole set it is the same journey everywhere")
        Toggle("Cinematic effects", isOn: $model.cinematic)
            .help("Spin with the zoom, palette drift with depth and slow cycling while flying (autopilot, journeys, fly-to)")
        Group {
            LabeledContent("Spin") {
                Slider(value: $model.cinematicSpin, in: -15...15) { Text("Spin") }.labelsHidden()
                    .help(String(format: "%.1f° per e-fold of zoom (×2.7)", model.cinematicSpin))
            }
            LabeledContent("Hue drift") {
                Slider(value: $model.cinematicHueDrift, in: 0...15) { Text("Hue drift") }.labelsHidden()
                    .help(String(format: "%.1f° of hue per e-fold of depth", model.cinematicHueDrift))
            }
            LabeledContent("Phase drift") {
                Slider(value: $model.cinematicPhaseDrift, in: 0...0.1) { Text("Phase drift") }.labelsHidden()
                    .help(String(format: "%.3f palette cycles per e-fold of depth", model.cinematicPhaseDrift))
            }
            LabeledContent("Cycling in flight") {
                Slider(value: $model.cinematicFlyCycle, in: 0...0.1) { Text("Cycling in flight") }.labelsHidden()
                    .help(String(format: "%.3f palette cycles per second while flying", model.cinematicFlyCycle))
            }
        }
        .disabled(!model.cinematic)
    }
}

/// Journey buttons (change-only model state: active, paused, seed; the HUD shows the leg,
/// phase and speed).
struct JourneyControls: View {
    let model: ExplorerModel
    let journeySeed: String

    var body: some View {
        let _ = UIPerf.body("leaf.journey")
        let active = model.journeyActive
        let suspended = model.journeySuspended
        HStack {
            Button {
                if active && suspended {
                    model.resumeJourney()
                } else if active {
                    model.stopJourney()
                } else if let seed = ExplorerModel.journeySeed(from: journeySeed) {
                    model.startJourney(seed: seed)
                } else {
                    model.toggleJourney()
                }
            } label: {
                Label(active ? (suspended ? "Resume" : "Stop") : "Journey",
                      systemImage: active ? "sparkles.rectangle.stack.fill" : "sparkles")
            }
            .help("An endless planned flight from landmark to landmark (J); a seed below replays that journey")
            Button {
                model.startDailyJourney()
            } label: {
                Label("Daily", systemImage: "calendar")
            }
            .help("Today's journey: the same for everyone today")
            if active {
                Button {
                    model.copyJourney()
                } label: {
                    Label("Copy #\(model.journeySeed)", systemImage: "doc.on.doc")
                }
                .help("Copy the journey's seed to share it")
            }
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
    }
}

// MARK: Look & colour

struct LookSectionRows: View {
    @Bindable var model: ExplorerModel

    var body: some View {
        let _ = UIPerf.body("section.look")
        #if os(macOS)
        ControlRows.picker("Look", selection: Binding(get: { model.lookID }, set: { model.applyLook($0) })) {
            looks
        }
        #else
        Picker("Look", selection: Binding(get: { model.lookID }, set: { model.applyLook($0) })) {
            looks
        }
        .pickerStyle(.navigationLink)
        #endif
        PalettePicker(model: model)
        ControlRows.slider("Density", value: ControlRows.log($model.colorDensity, range: 0.001...1), in: 0...1,
                           text: String(format: "%.4g", model.colorDensity))
        ControlRows.slider("Offset", value: $model.colorOffset, in: 0...1, text: String(format: "%.2f", model.colorOffset))
        Toggle("Logarithmic", isOn: $model.logColoring)
        Toggle("Smooth", isOn: $model.smoothColoring)
        Toggle("Palette anti-aliasing", isOn: $model.paletteFilter)
        Toggle("Calm dense regions", isOn: $model.denseCalm)
            .help("Where the palette cycles faster than a pixel, show a dark calm colour with thin filaments instead of grey noise")
        Toggle("Colour cycling", isOn: Binding(get: { model.cycleSpeed != 0 },
                                               set: { model.cycleSpeed = $0 ? 0.05 : 0 }))
        if model.cycleSpeed != 0 {
            ControlRows.slider("Cycle speed", value: ControlRows.log($model.cycleSpeed, range: 0.005...0.5), in: 0...1,
                               text: String(format: "%.3g /s", model.cycleSpeed))
        }
        ControlRows.slider("Depth tint", value: $model.depthTint, in: 0...0.2,
                           text: model.depthTint == 0 ? "off" : String(format: "%.3f / decade", model.depthTint))
    }
}

extension LookSectionRows {
    private var looks: some View {
        ForEach(ExplorerModel.looks, id: \.id) { l in
            HStack {
                if let p = ExplorerModel.palettes.first(where: { $0.id == l.paletteId }) {
                    PaletteSwatch(palette: p).frame(width: 64, height: 12)
                }
                Text(l.name)
            }
            .tag(l.id)
        }
    }
}

// MARK: Lighting

struct LightingSectionRows: View {
    @Bindable var model: ExplorerModel

    var body: some View {
        let _ = UIPerf.body("section.lighting")
        ControlRows.slider("Shading", value: $model.shading, in: 0...1, text: String(format: "%.2f", model.shading))
        ControlRows.slider("Light angle", value: $model.lightAngle, in: 0...360, text: String(format: "%.0f°", model.lightAngle))
            .disabled(model.shading == 0)
        ControlRows.slider("Light height", value: $model.lightHeight, in: 0.2...4, text: String(format: "%.2f", model.lightHeight))
            .disabled(model.shading == 0)
        ControlRows.slider("Highlight", value: $model.specular, in: 0...1, text: String(format: "%.2f", model.specular))
            .disabled(model.shading == 0)
        ControlRows.slider("Outline", value: $model.outline, in: 0...1, text: String(format: "%.2f", model.outline))
        ControlRows.slider("Outline width", value: $model.outlineWidth, in: 0.5...6, text: String(format: "%.1f px", model.outlineWidth))
            .disabled(model.outline == 0)
        ControlRows.color("Outline colour", ControlRows.linearColor(model, \.edgeColor))
            .disabled(model.outline == 0)
        ControlRows.slider("Glow", value: $model.glow, in: 0...0.5, text: String(format: "%.2f", model.glow))
        ControlRows.slider("Glow width", value: $model.glowWidth, in: 1...40, text: String(format: "%.0f px", model.glowWidth))
            .disabled(model.glow == 0)
        ControlRows.color("Glow colour", ControlRows.linearColor(model, \.glowColor))
            .disabled(model.glow == 0)
    }
}

// MARK: Interior

struct InteriorSectionRows: View {
    @Bindable var model: ExplorerModel

    var body: some View {
        let _ = UIPerf.body("section.interior")
        ControlRows.picker("Colouring", selection: $model.interiorColoring) {
            ForEach(InteriorColoring.allCases) { Text($0.label).tag($0) }
        }
        Text(model.interiorColoring.help)
            .font(.caption)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
        if model.interiorColoring == .flat {
            ControlRows.color("Colour", ControlRows.linearColor(model, \.interiorColor))
        } else {
            ControlRows.slider("Density", value: ControlRows.log($model.interiorDensity, range: 0.02...4), in: 0...1,
                               text: String(format: "%.3g", model.interiorDensity))
        }
        if model.interiorColoring == .angle {
            ControlRows.slider("Stripes", value: $model.interiorStripes, in: 0...32,
                               text: String(format: "%.0f", model.interiorStripes))
        }
        Toggle("Interior detection", isOn: $model.interiorDetection)
    }
}

// MARK: Overlays

struct OverlaySectionRows: View {
    @Bindable var model: ExplorerModel

    var body: some View {
        let _ = UIPerf.body("section.overlays")
        Toggle("Orbit of the point under the pointer", isOn: $model.orbitEnabled)
        if model.orbitEnabled {
            ControlRows.slider("Iterates", value: ControlRows.log(ControlRows.int($model.orbitLength), range: 1...1000), in: 0...1,
                               text: "\(model.orbitLength)")
        }
        Toggle("Mini-map", isOn: $model.mapEnabled)
        if model.mapEnabled {
            LabeledContent("Levels") {
                HStack(spacing: 4) {
                    Text("\(model.mapLevels)").monospacedDigit()
                    Stepper("Levels", value: $model.mapLevels, in: 1...4).labelsHidden()
                }
            }
            .help("The whole set plus up to three deeper maps at thousands of times the zoom")
            ControlRows.slider("Size", value: $model.mapSize, in: 100...320, text: String(format: "%.0f pt", model.mapSize))
            ControlRows.picker("Corner", selection: $model.mapPlacement) {
                ForEach(MapPlacement.allCases) { Text($0.label).tag($0) }
            }
        }
        Toggle("π meter", isOn: $model.piMeterEnabled)
            .help("At the neck −3/4 + iε or the cusp 1/4 + ε: the escape count N times ε (√ε) closes in on π (P)")
        if model.piMeterEnabled {
            PiMeterRows(model: model)
        }
        CosmicScaleRows(model: model)
        Toggle("Always show compass", isOn: $model.compassAlways)
            .help("Otherwise the compass shows while the view is turned. Click it to turn north, drag it to turn")
        Toggle("Autopilot target marker", isOn: $model.targetMarker)
            .help("A small ring where the autopilot or a journey is heading, with the pointer's wished direction as a faint line")
        Toggle("Status (HUD)", isOn: $model.showHUD)
        Toggle("Developer details in the HUD", isOn: $model.hudDetails)
            .disabled(!model.showHUD)
            .help("Iterations, GPU time, render size, reference orbit, kernel and render state")
    }
}

/// The π meter's hunts (the reading itself is the π panel on the canvas).
struct PiMeterRows: View {
    let model: ExplorerModel

    var body: some View {
        Text("Centre the view on the neck −3/4 + iε or the cusp 1/4 + ε (the panel on the canvas shows the count), or hunt π with a probe:")
            .font(.caption).foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
        HStack {
            Button("Hunt π at the neck") { model.piFlight("neck") }
            Button("Hunt π at the cusp") { model.piFlight("cusp") }
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
        .help("Flies to seahorse valley (neck) or elephant valley (cusp) and slides a probe down the line to ε = 1e-8 or 1e-16 while the meter counts and draws its orbit; Q / E change the speed, Esc stops")
    }
}

// MARK: Iterations & quality

struct QualitySectionRows: View {
    @Bindable var model: ExplorerModel

    private static let iterationCaps = [10_000, 50_000, 200_000, 1_000_000, 10_000_000]

    var body: some View {
        let _ = UIPerf.body("section.quality")
        Toggle("Automatic iterations", isOn: $model.autoIterations)
        if !model.autoIterations {
            Slider(value: ControlRows.log(ControlRows.int($model.manualIterations), range: 100...10_000_000,
                                          snap: ControlRows.twoDigits), in: 0...1) {
                Text("Maximum")
            }
            TextField("Maximum", value: $model.manualIterations, format: .number)
                .multilineTextAlignment(.trailing)
                .monospacedDigit()
        }
        if model.autoIterations {
            // Bounds only the automatic limit (a manual limit is used as set).
            ControlRows.picker("Upper limit", selection: $model.maxIterationsCap) {
                ForEach(Self.iterationCaps, id: \.self) { Text($0.formatted()).tag($0) }
            }
            .help("The most iterations automatic mode may use. It raises the limit only as far as the view needs.")
        }
        ControlRows.picker("Anti-aliasing", selection: $model.supersampling) {
            ForEach(Supersampling.allCases) { Text($0.label).tag($0) }
        }
        Toggle("Interactive preview", isOn: $model.interactivePreview)
    }
}

// MARK: Grouped-form look on the Mac (kb: platform/swiftui-performance)

#if os(macOS)
/// A section's rows on a rounded background with separators between them, like a grouped
/// `Form` section. Each row is its own child of the sidebar's `LazyVStack` (with its slice of the
/// card: rounded corners on the first and the last), so the stack builds only the visible rows.
/// Rows marked `controlRowTight` (tile grid rows) sit closer and without a separator.
struct ControlCardRows<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        if #available(macOS 15.0, *) {
            Group(subviews: content) { rows in
                let last = rows.count - 1
                ForEach(Array(rows.enumerated()), id: \.element.id) { i, row in
                    let tight = row.containerValues.controlRowTight
                    let afterTight = i > 0 && rows[i - 1].containerValues.controlRowTight
                    let r: CGFloat = 8
                    row
                        .padding(.horizontal, 10)
                        .padding(.top, tight && afterTight ? 0 : 7)
                        .padding(.bottom, tight ? 6 : 7)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .overlay(alignment: .top) {
                            if i > 0 && !(tight && afterTight) { Divider().padding(.leading, 10) }
                        }
                        .background(.background.secondary, in: UnevenRoundedRectangle(
                            topLeadingRadius: i == 0 ? r : 0, bottomLeadingRadius: i == last ? r : 0,
                            bottomTrailingRadius: i == last ? r : 0, topTrailingRadius: i == 0 ? r : 0,
                            style: .continuous))
                }
            }
        } else {
            VStack(alignment: .leading, spacing: 10) { content }
                .padding(.horizontal, 10).padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.background.secondary, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
    }
}

@available(macOS 15.0, *)
extension ContainerValues {
    /// A row of a tile grid: no separator to the row above it if that is one too.
    @Entry var controlRowTight = false
}

/// Label leading, value or control trailing (a grouped form row).
struct ControlRowLabeledStyle: LabeledContentStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(alignment: .firstTextBaseline) {
            configuration.label
            Spacer(minLength: 8)
            configuration.content
        }
    }
}

/// Label leading, a small switch trailing (a grouped form's toggle).
struct ControlRowToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack {
            configuration.label
            Spacer(minLength: 8)
            Toggle(isOn: configuration.$isOn) { configuration.label }
                .toggleStyle(.switch)
                .labelsHidden()
                .controlSize(.mini)
        }
    }
}
#endif

/// Same-shape tiles in rows: on the Mac each row of the grid is a row of the sidebar's lazy
/// stack (only the visible ones are built: kb platform/swiftui-performance); the columns follow
/// the sidebar's width. iOS: one grid in one form row.
struct TileRows<Tile: View>: View {
    let ids: [String]
    var minWidth: CGFloat
    var spacing: CGFloat
    var aspect: CGFloat = 1
    var height: CGFloat?
    @ViewBuilder var tile: (Int) -> Tile
    @Environment(\.controlRowWidth) private var width

    var body: some View {
        #if os(macOS)
        let columns = max(1, Int((width + spacing) / (minWidth + spacing)))
        ForEach(Array(stride(from: 0, to: ids.count, by: columns)), id: \.self) { start in
            let row = start..<min(ids.count, start + columns)
            TileGrid(minWidth: minWidth, spacing: spacing, aspect: aspect, height: height, columns: columns) {
                ForEach(Array(row), id: \.self) { i in tile(i).id(ids[i]) }
            }
            .modifier(TightRow())
        }
        #else
        TileGrid(minWidth: minWidth, spacing: spacing, aspect: aspect, height: height) {
            ForEach(ids.indices, id: \.self) { i in tile(i).id(ids[i]) }
        }
        #endif
    }
}

#if os(macOS)
/// Marks a tile grid row (`ControlCardRows`: no separator between such rows).
struct TightRow: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 15.0, *) {
            content.containerValue(\.controlRowTight, true)
        } else {
            content
        }
    }
}
#endif

extension EnvironmentValues {
    /// The width of a control center row's content (the sidebar's width less its margins).
    @Entry var controlRowWidth: CGFloat = 282
}

// MARK: Row helpers

enum ControlRows {
    /// A menu picker as a labelled row: the label leading, the menu trailing at its own size
    /// (outside a `Form` a picker puts its menu right after the label and stretches it).
    static func picker<V: Hashable, C: View>(_ title: String, selection: Binding<V>,
                                             @ViewBuilder content: () -> C) -> some View {
        LabeledContent(title) {
            Picker(title, selection: selection, content: content)
                .labelsHidden()
                .fixedSize()
        }
    }

    static func color(_ title: String, _ selection: Binding<Color>) -> some View {
        LabeledContent(title) {
            ColorPicker(title, selection: selection, supportsOpacity: false).labelsHidden()
        }
    }

    /// The model keeps colours in linear RGB (what the kernel blends in).
    @MainActor
    static func linearColor(_ model: ExplorerModel, _ path: ReferenceWritableKeyPath<ExplorerModel, SIMD3<Float>>) -> Binding<Color> {
        Binding {
            let c = model[keyPath: path]
            return Color(.sRGBLinear, red: Double(c.x), green: Double(c.y), blue: Double(c.z))
        } set: { color in
            let r = color.resolve(in: EnvironmentValues())
            model[keyPath: path] = SIMD3(r.linearRed, r.linearGreen, r.linearBlue)
        }
    }

    static func slider(_ title: String, value: Binding<Double>, in range: ClosedRange<Double>, text: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(title)
                Spacer()
                Text(text).monospacedDigit().foregroundStyle(.secondary)
            }
            Slider(value: value, in: range)
                .labelsHidden()
                .accessibilityLabel(title)
                .accessibilityValue(text)
        }
    }

    static func int(_ b: Binding<Int>) -> Binding<Double> {
        Binding { Double(b.wrappedValue) } set: { b.wrappedValue = Int($0.rounded()) }
    }

    /// Maps 0...1 onto `range` logarithmically; `snap` rounds what the slider sets.
    static func log(_ b: Binding<Double>, range: ClosedRange<Double>,
                    snap: @escaping (Double) -> Double = { $0 }) -> Binding<Double> {
        let lo = Foundation.log(range.lowerBound), hi = Foundation.log(range.upperBound)
        return Binding {
            (Foundation.log(min(max(b.wrappedValue, range.lowerBound), range.upperBound)) - lo) / (hi - lo)
        } set: {
            b.wrappedValue = min(max(snap(exp(lo + $0 * (hi - lo))), range.lowerBound), range.upperBound)
        }
    }

    /// Rounds to two significant digits (1,000,000, 1,100,000, ..., 9,900,000; 100, 110, ...):
    /// readable values from a log slider, 1-10 % apart.
    static func twoDigits(_ v: Double) -> Double {
        guard v.isFinite, v > 0 else { return v }
        let step = pow(10, floor(log10(v)) - 1)
        return (v / step).rounded() * step
    }
}

/// The control center's sections, in order, with what the search matches.
enum ControlSection: String, CaseIterable, Identifiable {
    case places, tours, view, journey, look, lighting, interior, overlays, quality, controls
    var id: String { rawValue }

    var title: String {
        switch self {
        case .places: "Places"
        case .tours: "Tours"
        case .view: "View & navigation"
        case .journey: "Journey & cinematic effects"
        case .look: "Look & colour"
        case .lighting: "Lighting & outline"
        case .interior: "Interior"
        case .overlays: "Overlays"
        case .quality: "Iterations & quality"
        case .controls: "Controls"
        }
    }

    var symbol: String {
        switch self {
        case .places: "mappin.and.ellipse"
        case .tours: "signpost.right"
        case .view: "scope"
        case .journey: "film"
        case .look: "paintpalette"
        case .lighting: "light.max"
        case .interior: "circle.inset.filled"
        case .overlays: "square.on.square.dashed"
        case .quality: "sparkles"
        case .controls: "keyboard"
        }
    }

    /// What the header's (?) explains (kb: decisions/controls "Help"); nil: no (?).
    var helpTopic: HelpTopic? {
        switch self {
        case .quality: .iterations
        case .look: .look
        case .interior: .interior
        case .overlays: .overlays
        case .controls: .controls
        default: nil
        }
    }

    /// What the section's ↺ resets (None: nothing to reset).
    var resetScope: ExplorerModel.ResetScope? {
        switch self {
        case .view: .view
        case .journey: .cinematic
        case .look: .look
        case .lighting: .lighting
        case .interior: .interior
        case .overlays: .overlays
        case .quality: .quality
        case .places, .tours, .controls: nil
        }
    }

    /// Whether the section matches a search query (title or keywords, case-insensitive).
    func matches(_ query: String) -> Bool {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return true }
        return title.lowercased().contains(q) || keywords.contains { $0.contains(q) }
    }

    var keywords: [String] {
        switch self {
        case .places: ["location", "bookmark", "famous", "random", "fly", "go", "catalog"]
        case .tours: ["tour", "guided", "learn", "maths", "math", "feigenbaum", "period", "doubling", "chaos", "orbit", "explain"]
        case .view: ["coordinates", "copy", "paste", "reset", "edge", "pan", "autopilot", "fly", "lock", "assist", "lane", "cruise", "rotation", "straighten"]
        case .journey: ["journey", "seed", "daily", "share", "stop", "stops", "find", "continuous", "cinematic", "effects", "rotation", "rotate", "spin", "turn", "hue", "drift", "cycling", "landmark", "minibrot", "spiral", "julia"]
        case .look: ["palette", "look", "colour", "color", "density", "offset", "smooth", "log", "cycling", "tint", "dense", "grey", "filter"]
        case .lighting: ["shading", "light", "angle", "height", "highlight", "specular", "outline", "edge", "glow"]
        case .interior: ["inside", "bubbles", "period", "angle", "multiplier", "distance", "interior", "detection", "atom"]
        case .overlays: ["orbit", "map", "mini-map", "minimap", "breadcrumb", "hud", "status", "iterates", "overlay", "cosmic", "scale", "size", "universe", "planck", "pi", "π", "neck", "cusp", "developer", "details", "diagnostics", "gpu", "performance", "reference", "kernel", "stats"]
        case .quality: ["iterations", "auto", "maximum", "cap", "upper limit", "anti-aliasing", "aa", "samples", "preview", "quality"]
        case .controls: ["keys", "keyboard", "shortcuts", "mouse", "trackpad", "help", "gamepad"]
        }
    }

    var footer: String? {
        switch self {
        case .tours: "A tour walks you through a piece of the maths, step by step: it flies the view, sets the colours and overlays and moves the orbit's point for you. → and ← step through it, Esc ends it and brings back your view and settings. Look around in between as you like."
        case .journey: "A journey flies on its own from landmark to landmark it finds in the maths: minibrots, the embedded Julia sets on the way to them, spirals and branch points. The same seed from the same start gives the same journey. Steering or Q down to zero hands over to cruise, other input pauses it; J or a click resumes, Esc stops. With \"Stop at each find\" it waits at each landmark: J flies on, Z goes deeper here, X somewhere else. Effects apply to the autopilot, journeys, fly-to and recorded flights."
        case .look: "A look sets palette, lighting, outline and interior colour together. Colours cycle only while the view rests; depth tint shifts the palette as you zoom."
        case .lighting: "Lighting, outline and glow use the distance estimate (a little GPU work per iteration)."
        case .interior: "Interior colourings analyse each interior pixel's attracting cycle on the GPU (period, multiplier, distance) at any depth; switching between them recolours instantly, switching from flat renders the view again."
        case .overlays: "The orbit follows the pointer (the crosshair in keyboard mode): c as a ring, then z₁…zₙ. Click a mini-map to jump there at the current zoom. The size scale compares the view with real things. O, M and C toggle them. The π meter (P) counts the centre's iterations on a background thread, up to 10⁹ (about 13 s). The target marker rings where the autopilot or a journey is heading. The HUD shows the zoom, the centre and the speed, and the cockpit at the bottom the driving level (M manual, C cruise, A autopilot, J journey), the speed gauge and lane assist; its developer details add the renderer's statistics."
        case .quality: "While you move, each frame renders as much as fits in a refresh; at rest the view refines and gathers anti-aliasing samples near the boundary."
        default: nil
        }
    }
}

/// Places: the catalogue of famous views (Rust core `places()`, kb: synthesis/places) as a
/// thumbnail grid by category. Selecting one shows what it is; Go jumps there, Fly flies there
/// (the fly-to path), Random picks one (of the shown category).
struct PlacesSection: View {
    let model: ExplorerModel
    @State private var category: PlaceCategory?
    @State private var selected: String?

    private var categories: [(PlaceCategory, String)] {
        var seen: [(PlaceCategory, String)] = []
        for p in ExplorerModel.places where !seen.contains(where: { $0.0 == p.category }) {
            seen.append((p.category, p.categoryName))
        }
        return seen
    }

    private var shown: [PlaceInfo] {
        ExplorerModel.places.filter { category == nil || $0.category == category }
    }


    var body: some View {
        let _ = UIPerf.body("PlacesSection")
        let _ = UIPerf.placesAppeared()
        ControlRows.picker("Category", selection: $category) {
            Text("All (\(ExplorerModel.places.count))").tag(PlaceCategory?.none)
            ForEach(categories, id: \.1) { c in
                Text(c.1).tag(PlaceCategory?.some(c.0))
            }
        }
        // The HUD's zoom at each place in this window (place zooms count from the whole set).
        let zoomOffset = model.explorer?.placeZoomOffset() ?? 0
        TileRows(ids: shown.map(\.id), minWidth: 88, spacing: 6, aspect: 1.6) { i in
            let p = shown[i]
            Button {
                selected = p.id
            } label: {
                PlaceThumbnail(id: p.id)
                    .aspectRatio(1.6, contentMode: .fit)
                    .overlay(alignment: .bottomTrailing) { PlaceZoomPill(zoomLog10: p.zoomLog10 + zoomOffset) }
                    .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                    // Only the selected tile has a border (every shape is one more layer to
                    // build when the section opens).
                    .overlay {
                        if selected == p.id {
                            RoundedRectangle(cornerRadius: 5, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 2)
                        }
                    }
            }
            .buttonStyle(.plain)
            // The name on hover (the tile shows only the zoom).
            .help(p.name)
            .accessibilityLabel(p.name)
            .simultaneousGesture(TapGesture(count: 2).onEnded { model.goToPlace(p.id) })
            #if os(iOS)
            // No hover on touch: a tap selects and shows the name and description below the
            // grid; a long press names the place and offers Go / Fly.
            .contextMenu {
                Section(p.name) {
                    Button { model.goToPlace(p.id) } label: { Label("Go", systemImage: "arrow.right.circle") }
                    Button { model.flyToPlace(p.id) } label: { Label("Fly", systemImage: "airplane") }
                }
            }
            #endif
        }
        if let id = selected, let p = ExplorerModel.places.first(where: { $0.id == id }) {
            VStack(alignment: .leading, spacing: 3) {
                Text(p.name).font(.callout.weight(.semibold))
                Text("\(p.categoryName) · \(Format.zoom(log10: p.zoomLog10 + (model.explorer?.placeZoomOffset() ?? 0))) · \(p.feature)")
                    .font(.caption).foregroundStyle(.secondary)
                Text(p.description).font(.caption).fixedSize(horizontal: false, vertical: true)
            }
        }
        HStack {
            Button {
                if let id = selected { model.goToPlace(id) }
            } label: {
                Label("Go", systemImage: "arrow.right.circle")
            }
            .disabled(selected == nil)
            .help("Jump there (double-click a thumbnail)")
            Button {
                if let id = selected { model.flyToPlace(id) }
            } label: {
                Label("Fly", systemImage: "airplane")
            }
            .disabled(selected == nil)
            .help("Fly there: out, across and in; Esc stops")
            // A place with a guided tour (kb: decisions/controls "Guided tours").
            if let id = selected, let tour = ExplorerModel.tour(forPlace: id) {
                Button {
                    model.startTour(tour.id)
                } label: {
                    Label("Tour", systemImage: "signpost.right")
                }
                .help("Take the guided tour “\(tour.title)”: → and ← step through it, Esc ends it")
            }
            Spacer()
            Button {
                selected = model.randomPlace(category: category).id
            } label: {
                Label("Random", systemImage: "dice")
            }
            .help("A random place\(category == nil ? "" : " of this category")")
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
    }
}

/// The place's zoom in a small dark pill at the bottom right of its thumbnail (the name is
/// the tooltip). Plain text: nothing changes per frame.
struct PlaceZoomPill: View {
    let zoomLog10: Double

    var body: some View {
        Text(Format.zoom(log10: zoomLog10))
            .font(.system(size: 9, weight: .semibold))
            .monospacedDigit()
            .lineLimit(1)
            .foregroundStyle(.white)
            .padding(.horizontal, 4)
            .padding(.vertical, 1.5)
            .background(.black.opacity(0.6), in: RoundedRectangle(cornerRadius: 3.5, style: .continuous))
            .padding(3)
            .accessibilityHidden(true)
    }
}

/// A place's thumbnail from the pre-decoded cache (PlaceThumbnails.preload at launch); if the
/// launch decode has not reached it yet, it waits for it off the main thread.
struct PlaceThumbnail: View {
    let id: String
    @State private var image: CGImage?
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        let _ = UIPerf.body("PlaceThumbnail")
        let shown = image ?? PlaceThumbnails.cached(id)
        if let shown {
            let _ = UIPerf.thumbnailVisible(id, immediate: image == nil)
            Image(decorative: shown, scale: displayScale).resizable()
        } else {
            // Only a tile without its image waits (no task per tile when the cache has it).
            Rectangle().fill(.quaternary)
                .task(id: id) { image = await PlaceThumbnails.decoded(id) }
        }
    }
}

/// The palette picker: the palette library's swatches in a grid by category (separate from
/// the look, which is a colouring preset). Choosing one applies its recommended density,
/// offset and deep colour; "Import palette…" reads .ugr / .map / JSON files into "My
/// palettes" (kept across launches).
struct PalettePicker: View {
    @Bindable var model: ExplorerModel
    @AppStorage("palettes.category") private var category = ""
    @State private var importing = false
    @State private var importNote: String?


    private var categories: [(key: String, label: String)] {
        var seen: [(key: String, label: String)] = []
        for p in model.library where !seen.contains(where: { $0.key == p.category }) {
            seen.append((p.category, p.categoryLabel))
        }
        return seen
    }

    var body: some View {
        let _ = UIPerf.body("PalettePicker")
        // The Mac: header, category, each grid row and the import button are rows of their own
        // (lazy, kb: platform/swiftui-performance); iOS: one form row.
        #if os(macOS)
        parts
        #else
        VStack(alignment: .leading, spacing: 6) { parts }
        #endif
    }

    @ViewBuilder private var parts: some View {
            HStack {
                Text("Palette")
                Spacer()
                Text(model.palette.name).foregroundStyle(.secondary)
            }
            Picker("Category", selection: $category) {
                Text("All (\(model.library.count))").tag("")
                ForEach(categories, id: \.key) { c in Text(c.label).tag(c.key) }
            }
            .labelsHidden()
            let shown = model.library.filter { category.isEmpty || $0.category == category }
            TileRows(ids: shown.map(\.id), minWidth: 64, spacing: 6, height: 16) { i in
                let p = shown[i]
                Button {
                    model.applyPalette(p.id)
                } label: {
                    PaletteSwatch(palette: PaletteInfo(id: p.id, name: p.name, stops: p.stops))
                        .frame(height: 16)
                        .overlay {
                            if model.paletteID == p.id {
                                RoundedRectangle(cornerRadius: 3, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 2)
                            }
                        }
                }
                .buttonStyle(.plain)
                .help("\(p.name) · \(p.categoryLabel)\(p.attribution.isEmpty ? "" : " · \(p.attribution)")")
                .accessibilityLabel(p.name)
                .accessibilityAddTraits(model.paletteID == p.id ? .isSelected : [])
            }
            HStack {
                Button {
                    importing = true
                } label: {
                    Label("Import palette…", systemImage: "square.and.arrow.down")
                }
                .help("Ultra Fractal .ugr, Fractint .map or almondbread JSON")
                if let importNote {
                    Text(importNote).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
            }
            .fileImporter(isPresented: $importing, allowedContentTypes: [.data, .json, .plainText],
                          allowsMultipleSelection: true) { result in
                switch result {
                case .success(let urls):
                    var n = 0
                    var failures: [String] = []
                    for url in urls {
                        do { n += try model.importPalettes(from: url) } catch { failures.append("\(error)") }
                    }
                    importNote = failures.isEmpty ? "Imported \(n) palette\(n == 1 ? "" : "s") into My palettes"
                        : "Imported \(n); failed: \(failures.joined(separator: "; "))"
                    if n > 0 { category = "user" }
                case .failure(let e):
                    importNote = "\(e.localizedDescription)"
                }
            }
    }
}

/// Keys and gestures.
struct ControlsHelp: View {
    let model: ExplorerModel

    var body: some View {
        #if os(macOS)
        row("scope", "Move the mouse", "Pointer mode: zoom at the cursor")
        row("keyboard", "Hold W A S D / arrows", "Pan; keyboard mode: zoom at the centre")
        row("space", "Hold Space / ⇧Space", "Zoom in / out at the crosshair")
        row("arrow.up.left.and.arrow.down.right", "Pinch, ⌘/⌥-scroll or wheel", "Zoom at the crosshair")
        row("plus.magnifyingglass", "+  −", "Zoom step at the crosshair")
        row("hand.draw", "Two-finger scroll or drag", "Pan")
        row("cursorarrow.click.2", "Double-click / ⌥ double-click", "Zoom in / out there")
        row("arrow.up.and.down.and.arrow.left.and.right", "Pointer at an edge", "Pan that way (while zooming)")
        row("gauge.with.dots.needle.33percent", "E / Q", "Cruise: one step in / out per press, hold for more; to zero and stop")
        row("space", "Space held + Q / E", "Space zoom speed − / + (\(Format.speed(model.zoomFactorPerSecond)))")
        row("road.lanes", "L", "Lane assist in cruise: keeps you out of featureless areas")
        row("point.topleft.down.to.point.bottomright.curvepath", "O", "Orbit overlay")
        row("map", "M · click a map · ⌥-click", "Mini-map · jump there · fly there")
        row("pi", "P", "π meter at the neck or the cusp")
        row("airplane", "Z", "Autopilot; steering hands over to cruise, click pauses")
        row("location.north.line", "⌘G / ⇧⌘G", "Fly to coordinates / copied ones")
        row("sparkles", "J  /  ⇧J", "Journey: on · resume · off  /  in the other mode")
        row("forward", "At a journey stop", "J next · Z deeper here · X somewhere else")
        row("signpost.right", "In a tour  → / ← / Esc", "Next step / back / end the tour")
        row("rotate.left", ",  .  /", "Turn left / right · upright (or trackpad rotate)")
        row("escape", "Esc", "Stop the cruise, the autopilot, a flight or a journey; then leave full screen")
        row("arrow.up.backward.and.arrow.down.forward", "F or ⌃⌘F", "Full screen on / off")
        row("arrow.counterclockwise", "R or ⌘0", "Reset view")
        row("sidebar.trailing", "⌘I", "Control center")
        row("gearshape", "⌘,", "Settings")
        #else
        row("hand.draw", "Drag", "Pan")
        row("arrow.up.left.and.arrow.down.right", "Pinch", "Zoom at the fingers, and pan")
        row("hand.tap", "Double tap", "Zoom in")
        row("hand.tap", "Two-finger tap", "Zoom out")
        row("map", "Tap a mini-map", "Jump there")
        row("scope", "Trackpad or mouse pointer", "Zoom at the pointer")
        row("keyboard", "Keyboard: W A S D / arrows", "Pan; zoom at the centre")
        row("space", "Keyboard: Space / ⇧Space", "Zoom")
        row("gauge.with.dots.needle.33percent", "Keyboard: E / Q, L", "Cruise in / out, lane assist")
        row("map", "Keyboard: O / M / P", "Orbit overlay / mini-map / π meter")
        row("airplane", "Autopilot button or Z", "Fly on; a finger steers (cruise), a tap pauses")
        row("sparkles", "Journey button or J", "Landmark to landmark; a tap pauses")
        row("forward", "At a journey stop", "Next · deeper here · somewhere else")
        row("minus.magnifyingglass", "Long-press autopilot", "Cruise in / out, stop, lane assist")
        row("rotate.left", "Two-finger rotate", "Turn the view")
        #endif
    }

    private func row(_ symbol: String, _ input: String, _ action: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: symbol)
                .frame(width: 20)
                .foregroundStyle(.secondary)
            Text(input)
            Spacer()
            Text(action).foregroundStyle(.secondary).multilineTextAlignment(.trailing)
        }
        .font(.caption)
        .accessibilityElement(children: .combine)
    }
}

/// The palette's gradient as a small horizontal bar.
struct PaletteSwatch: View {
    let palette: PaletteInfo

    var body: some View {
        RoundedRectangle(cornerRadius: 3, style: .continuous)
            .fill(LinearGradient(gradient: palette.swatchGradient, startPoint: .leading, endPoint: .trailing))
            .overlay(RoundedRectangle(cornerRadius: 3, style: .continuous).strokeBorder(.white.opacity(0.15)))
    }
}

extension PaletteInfo {
    /// One full cycle, closed back to the first stop like the GPU table.
    var swatchGradient: Gradient {
        func color(_ hex: UInt32) -> Color {
            Color(.sRGB, red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255)
        }
        var stops = self.stops.map { Gradient.Stop(color: color($0.rgb), location: $0.position) }
        if let first = self.stops.first {
            stops.append(Gradient.Stop(color: color(first.rgb), location: 1))
        }
        return Gradient(stops: stops)
    }
}

#if os(macOS)
/// App-wide preferences (⌘,): defaults that apply to every window and launch.
public struct AppSettingsView: View {
    @AppStorage(Preferences.edgePanKey) private var edgePanning = EdgePanning.whileZooming.rawValue
    @AppStorage(Preferences.gamepadKey) private var gamepad = true
    @AppStorage(Preferences.zoomUnitsKey) private var zoomUnits = ZoomUnits.magnification.rawValue
    @AppStorage(Preferences.restoreKey) private var restore = true
    @AppStorage(Preferences.hudKey) private var showHUD = true
    @AppStorage(Preferences.hudDetailsKey) private var hudDetails = false
    @FocusedValue(\.explorerModel) private var model

    public init() {}

    public var body: some View {
        TabView {
            Form {
                Toggle("Restore colours and overlays at launch", isOn: $restore)
                Toggle("Show the status overlay (HUD)", isOn: $showHUD)
                Toggle("Developer details in the status overlay", isOn: $hudDetails)
                    .disabled(!showHUD)
                Picker("Zoom shown as", selection: $zoomUnits) {
                    ForEach(ZoomUnits.allCases) { Text($0.label).tag($0.rawValue) }
                }
                Button("Reset colours, interior and overlays to the defaults") {
                    if let model {
                        model.resetSettings()
                    } else {
                        UserDefaults.standard.removeObject(forKey: Preferences.savedSettingsKey)
                    }
                }
            }
            .formStyle(.grouped)
            .tabItem { Label("General", systemImage: "gearshape") }

            Form {
                Picker("Edge panning", selection: $edgePanning) {
                    ForEach(EdgePanning.allCases) { Text($0.label).tag($0.rawValue) }
                }
                Text("With the pointer near an edge, the view also pans towards it. “While zooming” pans only while Space is held.")
                    .font(.caption).foregroundStyle(.secondary)
                Toggle("Game controllers", isOn: $gamepad)
                Text("Left stick pans, triggers zoom, A boosts, shoulders are the cruise pedals (Q / E), Y resets. Takes effect in new windows.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            .formStyle(.grouped)
            .tabItem { Label("Navigation", systemImage: "move.3d") }
        }
        .frame(width: 460, height: 330)
        .onChange(of: edgePanning) {
            if let m = model, let e = EdgePanning(rawValue: edgePanning) { m.edgePanning = e }
        }
        .onChange(of: showHUD) { model?.showHUD = showHUD }
        .onChange(of: hudDetails) { model?.hudDetails = hudDetails }
    }
}
#endif

/// Tiles in adaptive columns, like `LazyVGrid(columns: [GridItem(.adaptive(minimum:), spacing:)])`
/// but with the size computed from the width and the count alone: every tile has the same
/// shape (an aspect ratio or a fixed height), so measuring the grid never asks the tiles. Inside
/// the control center's (non-lazy) form a lazy grid re-estimated its tiles on every layout
/// pass of the sidebar (kb: platform/swiftui-performance).
struct TileGrid: Layout {
    var minWidth: CGFloat
    var spacing: CGFloat
    /// Width over height of a tile, or a fixed `height`.
    var aspect: CGFloat = 1
    var height: CGFloat?
    /// A fixed column count (a row of `TileRows`); nil: as many as fit.
    var columns: Int?

    private func geometry(width: CGFloat) -> (columns: Int, tile: CGSize) {
        let n = columns ?? max(1, Int((width + spacing) / (minWidth + spacing)))
        let w = max(0, (width - spacing * CGFloat(n - 1)) / CGFloat(n))
        return (n, CGSize(width: w, height: height ?? w / aspect))
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        // A stack may propose an infinite width (its ideal-size pass): three columns then.
        let width = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? (minWidth * 3 + spacing * 2)
        let g = geometry(width: width)
        let rows = (subviews.count + g.columns - 1) / g.columns
        return CGSize(width: width, height: rows == 0 ? 0 : CGFloat(rows) * g.tile.height + CGFloat(rows - 1) * spacing)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let g = geometry(width: bounds.width)
        let tile = ProposedViewSize(g.tile)
        for (i, s) in subviews.enumerated() {
            let x = bounds.minX + CGFloat(i % g.columns) * (g.tile.width + spacing)
            let y = bounds.minY + CGFloat(i / g.columns) * (g.tile.height + spacing)
            s.place(at: CGPoint(x: x, y: y), proposal: tile)
        }
    }
}

