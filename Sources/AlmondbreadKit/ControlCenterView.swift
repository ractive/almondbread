import AlmondbreadCore
import SwiftUI
import ImageIO
import UniformTypeIdentifiers

/// The control center: every per-window setting in grouped, searchable sections (kb:
/// decisions/controls, "Control center"). A trailing sidebar on the Mac and iPad (toolbar
/// button, ⌘I), a sheet on iPhone. App-wide preferences live in the Settings window (⌘,).
struct ControlCenterView: View {
    @Bindable var model: ExplorerModel
    @State private var search = UIDemo.search ?? ""
    @State private var showFlyTo = false
    @State private var confirmReset = false
    @State private var journeySeed = ""
    @Environment(\.undoManager) private var undoManager
    @AppStorage("controlCenter.collapsed") private var collapsedStorage = ""

    var body: some View {
        Form {
            ForEach(ControlSection.allCases.filter(matches)) { section in
                Section {
                    if !isCollapsed(section) || !search.isEmpty {
                        content(section)
                    }
                } header: {
                    sectionHeader(section)
                } footer: {
                    if !isCollapsed(section) || !search.isEmpty, let f = section.footer {
                        Text(f)
                    }
                }
            }
            if ControlSection.allCases.filter(matches).isEmpty {
                Text("No setting matches “\(search)”.").foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .controlSize(.small)
        .sheet(isPresented: $showFlyTo) { FlyToSheet(model: model, isPresented: $showFlyTo) }
        .safeAreaInset(edge: .top, spacing: 0) {
            searchField
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            resetAllBar
        }
        #if os(macOS)
        // On the Mac the canvas runs on under the sidebar: the π panel (bottom right) moves
        // left of it while it shows.
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { model.piMeterInsetRight = Double($0) }
        .onDisappear { model.piMeterInsetRight = 0 }
        #endif
    }

    // MARK: Search and sections

    private var searchField: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Search settings", text: $search)
                .textFieldStyle(.plain)
            if !search.isEmpty {
                Button {
                    search = ""
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
    }

    private func matches(_ s: ControlSection) -> Bool {
        let q = search.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return true }
        return s.title.lowercased().contains(q) || s.keywords.contains { $0.contains(q) }
    }

    private var collapsed: Set<String> {
        Set(collapsedStorage.split(separator: ",").map(String.init))
    }

    private func isCollapsed(_ s: ControlSection) -> Bool { collapsed.contains(s.rawValue) }

    private func toggle(_ s: ControlSection) {
        var c = collapsed
        if c.contains(s.rawValue) { c.remove(s.rawValue) } else { c.insert(s.rawValue) }
        collapsedStorage = c.sorted().joined(separator: ",")
    }

    private func sectionHeader(_ s: ControlSection) -> some View {
        HStack(spacing: 8) {
            Button {
                withAnimation(.snappy(duration: 0.2)) { toggle(s) }
            } label: {
                HStack {
                    Label(s.title, systemImage: s.symbol)
                    Spacer()
                    Image(systemName: "chevron.right")
                        .rotationEffect(.degrees(isCollapsed(s) && search.isEmpty ? 0 : 90))
                        .foregroundStyle(.tertiary)
                        .font(.caption)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(.isHeader)
            .accessibilityHint(isCollapsed(s) ? "Expands the section" : "Collapses the section")
            if let scope = s.resetScope {
                Button {
                    model.reset(scope, undo: undoManager)
                } label: {
                    Image(systemName: "arrow.counterclockwise").font(.caption)
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
                .help("Reset \(s.title.lowercased()) to the defaults (⌘Z undoes)")
                .accessibilityLabel("Reset \(s.title)")
            }
        }
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

    @ViewBuilder
    private func content(_ s: ControlSection) -> some View {
        switch s {
        case .places: PlacesSection(model: model)
        case .view: viewRows
        case .journey: journeyRows
        case .look: lookRows
        case .lighting: lightingRows
        case .interior: interiorRows
        case .overlays: overlayRows
        case .quality: qualityRows
        case .diagnostics: diagnosticsRows
        case .controls: ControlsHelp(model: model)
        }
    }

    // MARK: View & navigation

    @ViewBuilder
    private var viewRows: some View {
        let v = model.stats?.shown
        coordinateRow("Re", v?.re ?? "")
        coordinateRow("Im", v?.im ?? "")
        LabeledContent("Zoom") {
            Text(Format.zoom(log10: v?.zoomLog10 ?? 0)).monospacedDigit()
        }
        LabeledContent("Scale") {
            Text("\(v?.scaleText ?? "") / pt").monospacedDigit()
        }
        LabeledContent("Zoom speed") {
            Text(Format.speed(model.zoomFactorPerSecond)).monospacedDigit()
        }
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
        Picker("Edge panning", selection: $model.edgePanning) {
            ForEach(EdgePanning.allCases) { Text($0.label).tag($0) }
        }
        // Autopilot and fly-to (kb: decisions/controls, autopilot).
        let s = model.stats
        HStack {
            Button {
                model.toggleAutopilot()
            } label: {
                Label(s?.autopilot.active == true ? "Stop" : "Autopilot",
                      systemImage: s?.autopilot.active == true ? "airplane.circle.fill" : "airplane")
            }
            .help("Zoom on continuously, steer with the pointer; click pauses, Esc stops (Z)")
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
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
        if let s, s.autopilot.active || s.flight.active {
            Text(s.flight.active ? "fly speed \(Format.speed(exp(s.flight.speed)))"
                 : "autopilot \(Format.speed(exp(s.autopilot.speed)))")
                .font(.caption).foregroundStyle(.secondary).monospacedDigit()
            Button("Stop (Esc)") { model.stopMotion() }
        }
        Toggle("Autopilot assist", isOn: $model.autopilotAssist)
            .help("Leans the autopilot's heading towards nearby detail while the pointer rests near the centre; it never steers against the pointer")
        // View rotation (kb: decisions/controls, "View rotation").
        LabeledContent("Rotation") {
            HStack {
                Text(String(format: "%.1f°", v?.angleDegrees ?? 0)).monospacedDigit()
                Button("Straighten") { model.straighten() }
                    .disabled((v?.angleDegrees ?? 0) == 0)
                    .help("Turn the view upright (/); , and . turn it, so does a trackpad rotation")
            }
        }
    }

    // MARK: Journey & cinematic effects

    @ViewBuilder
    private var journeyRows: some View {
        let j = model.stats?.journey
        HStack {
            Button {
                if j?.active == true && j?.suspended == true {
                    model.resumeJourney()
                } else if j?.active == true {
                    model.stopJourney()
                } else if let seed = ExplorerModel.journeySeed(from: journeySeed) {
                    model.startJourney(seed: seed)
                } else {
                    model.toggleJourney()
                }
            } label: {
                Label(j?.active == true ? (j?.suspended == true ? "Resume" : "Stop") : "Journey",
                      systemImage: j?.active == true ? "sparkles.rectangle.stack.fill" : "sparkles")
            }
            .help("An endless planned flight from landmark to landmark (J); a seed below replays that journey")
            Button {
                model.startDailyJourney()
            } label: {
                Label("Daily", systemImage: "calendar")
            }
            .help("Today's journey: the same for everyone today")
            if j?.active == true {
                Button {
                    model.copyJourney()
                } label: {
                    Label("Copy #\(j?.seed ?? 0)", systemImage: "doc.on.doc")
                }
                .help("Copy the journey's seed to share it")
            }
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
        TextField("Seed (empty: random)", text: $journeySeed)
            .help("A journey number, e.g. 4711; started from the whole set it is the same journey everywhere")
        if let j, j.active {
            Text(j.suspended ? "Journey #\(j.seed) paused" :
                 "#\(j.seed) · leg \(j.leg) · \(j.label.isEmpty ? j.phase : j.label) · \(j.phase)")
                .font(.caption).foregroundStyle(.secondary).monospacedDigit()
            Text("journey speed \(Format.speed(exp(j.speed))) (Q / E)")
                .font(.caption).foregroundStyle(.secondary).monospacedDigit()
        }
        Toggle("Cinematic effects", isOn: $model.cinematic)
            .help("Spin with the zoom, palette drift with depth and slow cycling while flying (autopilot, journeys, fly-to)")
        Group {
            LabeledContent("Spin") {
                Slider(value: $model.cinematicSpin, in: -15...15) { Text("Spin") }
                    .help(String(format: "%.1f° per e-fold of zoom (×2.7)", model.cinematicSpin))
            }
            LabeledContent("Hue drift") {
                Slider(value: $model.cinematicHueDrift, in: 0...15) { Text("Hue drift") }
                    .help(String(format: "%.1f° of hue per e-fold of depth", model.cinematicHueDrift))
            }
            LabeledContent("Phase drift") {
                Slider(value: $model.cinematicPhaseDrift, in: 0...0.1) { Text("Phase drift") }
                    .help(String(format: "%.3f palette cycles per e-fold of depth", model.cinematicPhaseDrift))
            }
            LabeledContent("Cycling in flight") {
                Slider(value: $model.cinematicFlyCycle, in: 0...0.1) { Text("Cycling in flight") }
                    .help(String(format: "%.3f palette cycles per second while flying", model.cinematicFlyCycle))
            }
        }
        .disabled(!model.cinematic)
    }

    private func coordinateRow(_ label: String, _ value: String) -> some View {
        LabeledContent(label) {
            Text(value)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
                .lineLimit(1)
                .minimumScaleFactor(0.7)
        }
    }

    // MARK: Look & colour

    @ViewBuilder
    private var lookRows: some View {
        Picker("Look", selection: Binding(get: { model.lookID }, set: { model.applyLook($0) })) {
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
        #if os(macOS)
        .pickerStyle(.menu)
        #else
        .pickerStyle(.navigationLink)
        #endif
        PalettePicker(model: model)
        sliderRow("Density", value: logBinding($model.colorDensity, range: 0.001...1), in: 0...1,
                  text: String(format: "%.4g", model.colorDensity))
        sliderRow("Offset", value: $model.colorOffset, in: 0...1, text: String(format: "%.2f", model.colorOffset))
        Toggle("Logarithmic", isOn: $model.logColoring)
        Toggle("Smooth", isOn: $model.smoothColoring)
        Toggle("Palette anti-aliasing", isOn: $model.paletteFilter)
        Toggle("Calm dense regions", isOn: $model.denseCalm)
            .help("Where the palette cycles faster than a pixel, show a dark calm colour with thin filaments instead of grey noise")
        Toggle("Colour cycling", isOn: Binding(get: { model.cycleSpeed != 0 },
                                               set: { model.cycleSpeed = $0 ? 0.05 : 0 }))
        if model.cycleSpeed != 0 {
            sliderRow("Cycle speed", value: logBinding($model.cycleSpeed, range: 0.005...0.5), in: 0...1,
                      text: String(format: "%.3g /s", model.cycleSpeed))
        }
        sliderRow("Depth tint", value: $model.depthTint, in: 0...0.2,
                  text: model.depthTint == 0 ? "off" : String(format: "%.3f / decade", model.depthTint))
    }

    // MARK: Lighting

    @ViewBuilder
    private var lightingRows: some View {
        sliderRow("Shading", value: $model.shading, in: 0...1, text: String(format: "%.2f", model.shading))
        sliderRow("Light angle", value: $model.lightAngle, in: 0...360, text: String(format: "%.0f°", model.lightAngle))
            .disabled(model.shading == 0)
        sliderRow("Light height", value: $model.lightHeight, in: 0.2...4, text: String(format: "%.2f", model.lightHeight))
            .disabled(model.shading == 0)
        sliderRow("Highlight", value: $model.specular, in: 0...1, text: String(format: "%.2f", model.specular))
            .disabled(model.shading == 0)
        sliderRow("Outline", value: $model.outline, in: 0...1, text: String(format: "%.2f", model.outline))
        sliderRow("Outline width", value: $model.outlineWidth, in: 0.5...6, text: String(format: "%.1f px", model.outlineWidth))
            .disabled(model.outline == 0)
        ColorPicker("Outline colour", selection: linearColorBinding(\.edgeColor), supportsOpacity: false)
            .disabled(model.outline == 0)
        sliderRow("Glow", value: $model.glow, in: 0...0.5, text: String(format: "%.2f", model.glow))
        sliderRow("Glow width", value: $model.glowWidth, in: 1...40, text: String(format: "%.0f px", model.glowWidth))
            .disabled(model.glow == 0)
        ColorPicker("Glow colour", selection: linearColorBinding(\.glowColor), supportsOpacity: false)
            .disabled(model.glow == 0)
    }

    // MARK: Interior

    @ViewBuilder
    private var interiorRows: some View {
        Picker("Colouring", selection: $model.interiorColoring) {
            ForEach(InteriorColoring.allCases) { Text($0.label).tag($0) }
        }
        Text(model.interiorColoring.help)
            .font(.caption)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
        if model.interiorColoring == .flat {
            ColorPicker("Colour", selection: linearColorBinding(\.interiorColor), supportsOpacity: false)
        } else {
            sliderRow("Density", value: logBinding($model.interiorDensity, range: 0.02...4), in: 0...1,
                      text: String(format: "%.3g", model.interiorDensity))
        }
        if model.interiorColoring == .angle {
            sliderRow("Stripes", value: $model.interiorStripes, in: 0...32,
                      text: String(format: "%.0f", model.interiorStripes))
        }
        Toggle("Interior detection", isOn: $model.interiorDetection)
    }

    // MARK: Overlays

    @ViewBuilder
    private var overlayRows: some View {
        Toggle("Orbit of the point under the pointer", isOn: $model.orbitEnabled)
        if model.orbitEnabled {
            sliderRow("Iterates", value: logBinding(intBinding($model.orbitLength), range: 1...1000), in: 0...1,
                      text: "\(model.orbitLength)")
            if let o = model.orbit, !o.text.isEmpty {
                LabeledContent("c") { Text(o.text).monospacedDigit().lineLimit(2) }
                LabeledContent("|z\(o.iterates)|") { Text(String(format: "%.4g", o.lastAbs)).monospacedDigit() }
            }
        }
        Toggle("Mini-map", isOn: $model.mapEnabled)
        if model.mapEnabled {
            Stepper("Levels: \(model.mapLevels)", value: $model.mapLevels, in: 1...4)
                .help("The whole set plus up to three deeper maps at thousands of times the zoom")
            sliderRow("Size", value: $model.mapSize, in: 100...320, text: String(format: "%.0f pt", model.mapSize))
            Picker("Corner", selection: $model.mapPlacement) {
                ForEach(MapPlacement.allCases) { Text($0.label).tag($0) }
            }
            if let note = model.orbit?.mapNote, !note.isEmpty {
                Text(note).font(.caption).foregroundStyle(.secondary)
            }
        }
        Toggle("π meter", isOn: $model.piMeterEnabled)
            .help("At the neck −3/4 + iε or the cusp 1/4 + ε: the escape count N times ε (√ε) closes in on π (P)")
        if model.piMeterEnabled {
            piMeterRows
        }
        CosmicScaleRows(model: model)
        Toggle("Status (HUD)", isOn: $model.showHUD)
    }

    @ViewBuilder
    private var piMeterRows: some View {
        if let m = model.piMeter, !m.point.isEmpty {
            if m.count > 0 {
                LabeledContent(m.point == "cusp" ? "N·√ε" : "N·ε") {
                    Text(String(format: "%.10g", m.product)).monospacedDigit()
                }
                LabeledContent("N") { Text("\(m.count) at ε \(String(format: "%.3g", m.countEps))").monospacedDigit() }
                LabeledContent("Digits of π") { Text("\(m.digits)").monospacedDigit() }
            }
            if m.counting {
                ProgressView(value: m.progress) { Text("Counting …").font(.caption) }
            }
            if m.tooDeep {
                Text("Closer than the meter counts (over 10⁹ iterations).").font(.caption).foregroundStyle(.secondary)
            }
        } else {
            Text("Centre the view on the neck −3/4 + iε or the cusp 1/4 + ε, or hunt π with a probe:")
                .font(.caption).foregroundStyle(.secondary)
        }
        HStack {
            Button("Hunt π at the neck") { model.piFlight("neck") }
            Button("Hunt π at the cusp") { model.piFlight("cusp") }
        }
        .help("Flies to seahorse valley (neck) or elephant valley (cusp) and slides a probe down the line to ε = 1e-8 or 1e-16 while the meter counts and draws its orbit; Q / E change the speed, Esc stops")
    }

    // MARK: Iterations & quality

    private static let iterationCaps = [10_000, 50_000, 200_000, 1_000_000, 10_000_000]

    @ViewBuilder
    private var qualityRows: some View {
        Toggle("Automatic iterations", isOn: $model.autoIterations)
        if model.autoIterations {
            LabeledContent("Current", value: (model.stats?.maxIterations ?? 0).formatted())
        } else {
            Slider(value: logBinding(intBinding($model.manualIterations), range: 100...10_000_000), in: 0...1) {
                Text("Maximum")
            }
            TextField("Maximum", value: $model.manualIterations, format: .number)
                .multilineTextAlignment(.trailing)
                .monospacedDigit()
        }
        Picker("Cap", selection: $model.maxIterationsCap) {
            ForEach(Self.iterationCaps, id: \.self) { Text($0.formatted()).tag($0) }
        }
        Picker("Anti-aliasing", selection: $model.supersampling) {
            ForEach(Supersampling.allCases) { Text($0.label).tag($0) }
        }
        Toggle("Interactive preview", isOn: $model.interactivePreview)
    }

    // MARK: Performance & diagnostics

    @ViewBuilder
    private var diagnosticsRows: some View {
        if let s = model.stats {
            LabeledContent("GPU (view)") { Text("\(Format.ms(s.gpuTime)) · \(s.chunks) chunks").monospacedDigit() }
            LabeledContent("Render size") { Text("\(s.renderWidth)×\(s.renderHeight)\(s.isPreview ? " preview" : "")").monospacedDigit() }
            LabeledContent("Iterations") { Text(s.maxIterations.formatted()).monospacedDigit() }
            LabeledContent("Reference") {
                Text("\(s.referenceLength.formatted()) pts · \(s.referenceBits) bit\(s.referencePeriod > 0 ? " · period \(s.referencePeriod)" : "")")
                    .monospacedDigit()
            }
            LabeledContent("Reference CPU") { Text(Format.ms(s.referenceTime) + (s.referencePending ? " · computing" : "")).monospacedDigit() }
            LabeledContent("Kernel") { Text(s.deep ? "deep (extended range)" : "plain float32") }
            LabeledContent("State") { Text(s.settled ? "settled" : (s.isRendering ? "refining" : "idle")) }
            if s.failedFrames > 0 {
                LabeledContent("Failed frames") { Text("\(s.failedFrames)").foregroundStyle(.red) }
            }
        } else {
            Text("No renderer").foregroundStyle(.secondary)
        }
    }

    // MARK: Helpers

    /// The model keeps colours in linear RGB (what the kernel blends in).
    private func linearColorBinding(_ path: ReferenceWritableKeyPath<ExplorerModel, SIMD3<Float>>) -> Binding<Color> {
        Binding {
            let c = model[keyPath: path]
            return Color(.sRGBLinear, red: Double(c.x), green: Double(c.y), blue: Double(c.z))
        } set: { color in
            let r = color.resolve(in: EnvironmentValues())
            model[keyPath: path] = SIMD3(r.linearRed, r.linearGreen, r.linearBlue)
        }
    }

    private func sliderRow(_ title: String, value: Binding<Double>, in range: ClosedRange<Double>, text: String) -> some View {
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

    private func intBinding(_ b: Binding<Int>) -> Binding<Double> {
        Binding { Double(b.wrappedValue) } set: { b.wrappedValue = Int($0.rounded()) }
    }

    /// Maps 0...1 onto `range` logarithmically.
    private func logBinding(_ b: Binding<Double>, range: ClosedRange<Double>) -> Binding<Double> {
        let lo = log(range.lowerBound), hi = log(range.upperBound)
        return Binding {
            (log(min(max(b.wrappedValue, range.lowerBound), range.upperBound)) - lo) / (hi - lo)
        } set: {
            b.wrappedValue = exp(lo + $0 * (hi - lo))
        }
    }
}

/// The control center's sections, in order, with what the search matches.
enum ControlSection: String, CaseIterable, Identifiable {
    case places, view, journey, look, lighting, interior, overlays, quality, diagnostics, controls
    var id: String { rawValue }

    var title: String {
        switch self {
        case .places: "Places"
        case .view: "View & navigation"
        case .journey: "Journey & cinematic effects"
        case .look: "Look & colour"
        case .lighting: "Lighting & outline"
        case .interior: "Interior"
        case .overlays: "Overlays"
        case .quality: "Iterations & quality"
        case .diagnostics: "Performance & diagnostics"
        case .controls: "Controls"
        }
    }

    var symbol: String {
        switch self {
        case .places: "mappin.and.ellipse"
        case .view: "scope"
        case .journey: "film"
        case .look: "paintpalette"
        case .lighting: "light.max"
        case .interior: "circle.inset.filled"
        case .overlays: "square.on.square.dashed"
        case .quality: "sparkles"
        case .diagnostics: "gauge.with.dots.needle.67percent"
        case .controls: "keyboard"
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
        case .places, .diagnostics, .controls: nil
        }
    }

    var keywords: [String] {
        switch self {
        case .places: ["location", "bookmark", "famous", "random", "fly", "go", "catalog"]
        case .view: ["coordinates", "re", "im", "zoom", "scale", "copy", "paste", "reset", "edge", "pan", "speed", "autopilot", "fly", "assist"]
        case .journey: ["journey", "seed", "daily", "share", "cinematic", "effects", "rotation", "rotate", "spin", "turn", "hue", "drift", "cycling", "landmark", "minibrot", "spiral", "julia"]
        case .look: ["palette", "look", "colour", "color", "density", "offset", "smooth", "log", "cycling", "tint", "dense", "grey", "filter"]
        case .lighting: ["shading", "light", "angle", "height", "highlight", "specular", "outline", "edge", "glow"]
        case .interior: ["inside", "bubbles", "period", "angle", "multiplier", "distance", "interior", "detection", "atom"]
        case .overlays: ["orbit", "map", "mini-map", "minimap", "breadcrumb", "hud", "status", "iterates", "overlay", "cosmic", "scale", "size", "universe", "planck", "pi", "π", "neck", "cusp"]
        case .quality: ["iterations", "auto", "maximum", "cap", "anti-aliasing", "aa", "samples", "preview", "quality"]
        case .diagnostics: ["gpu", "performance", "reference", "time", "chunks", "kernel", "deep", "stats"]
        case .controls: ["keys", "keyboard", "shortcuts", "mouse", "trackpad", "help", "gamepad"]
        }
    }

    var footer: String? {
        switch self {
        case .journey: "A journey flies on its own from landmark to landmark it finds in the maths: minibrots, the embedded Julia sets on the way to them, spirals and branch points. The same seed from the same start gives the same journey. Any input takes over; J or a click resumes, Esc stops. Effects apply to the autopilot, journeys, fly-to and recorded flights."
        case .look: "A look sets palette, lighting, outline and interior colour together. Colours cycle only while the view rests; depth tint shifts the palette as you zoom."
        case .lighting: "Lighting, outline and glow use the distance estimate (a little GPU work per iteration)."
        case .interior: "Interior colourings analyse each interior pixel's attracting cycle on the GPU (period, multiplier, distance) at any depth; switching between them recolours instantly, switching from flat renders the view again."
        case .overlays: "The orbit follows the pointer (the crosshair in keyboard mode): c as a ring, then z₁…zₙ. Click a mini-map to jump there at the current zoom. The size scale compares the view with real things. O, M and C toggle them. The π meter (P) counts the centre's iterations on a background thread, up to 10⁹ (about 13 s)."
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

    private let columns = [GridItem(.adaptive(minimum: 88), spacing: 6)]

    var body: some View {
        Picker("Category", selection: $category) {
            Text("All (\(ExplorerModel.places.count))").tag(PlaceCategory?.none)
            ForEach(categories, id: \.1) { c in
                Text(c.1).tag(PlaceCategory?.some(c.0))
            }
        }
        LazyVGrid(columns: columns, spacing: 6) {
            ForEach(shown, id: \.id) { p in
                Button {
                    selected = p.id
                } label: {
                    PlaceThumbnail(id: p.id)
                        .aspectRatio(1.6, contentMode: .fit)
                        .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 5, style: .continuous)
                            .strokeBorder(selected == p.id ? Color.accentColor : .white.opacity(0.12),
                                          lineWidth: selected == p.id ? 2 : 1))
                }
                .buttonStyle(.plain)
                .help("\(p.name) · \(Format.zoom(log10: p.zoomLog10))")
                .accessibilityLabel(p.name)
                .simultaneousGesture(TapGesture(count: 2).onEnded { model.goToPlace(p.id) })
            }
        }
        if let id = selected, let p = ExplorerModel.places.first(where: { $0.id == id }) {
            VStack(alignment: .leading, spacing: 3) {
                Text(p.name).font(.callout.weight(.semibold))
                Text("\(p.categoryName) · \(Format.zoom(log10: p.zoomLog10)) · \(p.feature)")
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

/// A place's thumbnail (HEIC package resource), decoded once, off the main thread.
struct PlaceThumbnail: View {
    let id: String
    @State private var image: CGImage?

    var body: some View {
        Group {
            if let image = image ?? PlaceThumbnails.cached(id) {
                Image(decorative: image, scale: 2).resizable()
            } else {
                Rectangle().fill(.quaternary)
            }
        }
        .task(id: id) { image = await PlaceThumbnails.decoded(id) }
    }
}

extension PlaceThumbnails {
    /// One serial queue decodes every thumbnail into a plain bitmap. Lazily decoded HEIC
    /// images were decoded by Core Animation on the main thread while SwiftUI's
    /// prepare-image threads decoded others: in the iOS simulator the HEIF decoder's session
    /// pool deadlocked the main thread when the control center opened (kb:
    /// platform/ios-simulator). Fully decoded bitmaps also scroll without decode hitches.
    private static let queue = DispatchQueue(label: "almondbread.place-thumbnails", qos: .userInitiated)
    private nonisolated(unsafe) static var cache: [String: CGImage] = [:]
    private static let lock = NSLock()

    static func cached(_ id: String) -> CGImage? {
        lock.lock()
        defer { lock.unlock() }
        return cache[id]
    }

    static func decoded(_ id: String) async -> CGImage? {
        if let c = cached(id) { return c }
        return await withCheckedContinuation { (k: CheckedContinuation<CGImage?, Never>) in
            queue.async {
                let image = cached(id) ?? decode(id)
                if let image {
                    lock.lock()
                    cache[id] = image
                    lock.unlock()
                }
                k.resume(returning: image)
            }
        }
    }

    /// The @2x thumbnail drawn into an sRGB bitmap (no lazy decode left for later).
    private static func decode(_ id: String) -> CGImage? {
        guard let url = url(id: id), let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary),
              let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: space, bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
                                    | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return ctx.makeImage()
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

    private let columns = [GridItem(.adaptive(minimum: 64), spacing: 6)]

    private var categories: [(key: String, label: String)] {
        var seen: [(key: String, label: String)] = []
        for p in model.library where !seen.contains(where: { $0.key == p.category }) {
            seen.append((p.category, p.categoryLabel))
        }
        return seen
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
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
            LazyVGrid(columns: columns, spacing: 6) {
                ForEach(model.library.filter { category.isEmpty || $0.category == category }, id: \.id) { p in
                    Button {
                        model.applyPalette(p.id)
                    } label: {
                        PaletteSwatch(palette: PaletteInfo(id: p.id, name: p.name, stops: p.stops))
                            .frame(height: 16)
                            .overlay(RoundedRectangle(cornerRadius: 3, style: .continuous)
                                .strokeBorder(model.paletteID == p.id ? Color.accentColor : .clear, lineWidth: 2))
                    }
                    .buttonStyle(.plain)
                    .help("\(p.name) · \(p.categoryLabel)\(p.attribution.isEmpty ? "" : " · \(p.attribution)")")
                    .accessibilityLabel(p.name)
                    .accessibilityAddTraits(model.paletteID == p.id ? .isSelected : [])
                }
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
        row("gauge.with.dots.needle.33percent", "Q / E", "Zoom speed − / + (\(Format.speed(model.zoomFactorPerSecond)))")
        row("point.topleft.down.to.point.bottomright.curvepath", "O", "Orbit overlay")
        row("map", "M · click a map · ⌥-click", "Mini-map · jump there · fly there")
        row("pi", "P", "π meter at the neck or the cusp")
        row("airplane", "Z", "Autopilot: steer with the mouse, click pauses")
        row("location.north.line", "⌘G / ⇧⌘G", "Fly to coordinates / copied ones")
        row("sparkles", "J", "Journey: on · resume after input · off")
        row("rotate.left", ",  .  /", "Turn left / right · upright (or trackpad rotate)")
        row("escape", "Esc", "Stop the autopilot, a flight or a journey")
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
        row("space", "Keyboard: Space / ⇧Space, Q / E", "Zoom, zoom speed")
        row("map", "Keyboard: O / M / P", "Orbit overlay / mini-map / π meter")
        row("airplane", "Autopilot button or Z", "Fly on; a finger steers, a tap pauses")
        row("sparkles", "Journey button or J", "Landmark to landmark; a tap pauses")
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
    @FocusedValue(\.explorerModel) private var model

    public init() {}

    public var body: some View {
        TabView {
            Form {
                Toggle("Restore colours and overlays at launch", isOn: $restore)
                Toggle("Show the status overlay (HUD)", isOn: $showHUD)
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
                Text("Left stick pans, triggers zoom, A boosts, shoulders change the zoom speed, Y resets. Takes effect in new windows.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            .formStyle(.grouped)
            .tabItem { Label("Navigation", systemImage: "move.3d") }
        }
        .frame(width: 460, height: 300)
        .onChange(of: edgePanning) {
            if let m = model, let e = EdgePanning(rawValue: edgePanning) { m.edgePanning = e }
        }
        .onChange(of: showHUD) { model?.showHUD = showHUD }
    }
}
#endif
