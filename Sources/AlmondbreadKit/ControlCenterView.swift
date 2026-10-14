import AlmondbreadCore
import SwiftUI

/// The control center: every per-window setting in grouped, searchable sections (kb:
/// decisions/controls, "Control center"). A trailing sidebar on the Mac and iPad (toolbar
/// button, ⌘I), a sheet on iPhone. App-wide preferences live in the Settings window (⌘,).
struct ControlCenterView: View {
    @Bindable var model: ExplorerModel
    @State private var search = ""
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
        .safeAreaInset(edge: .top, spacing: 0) {
            searchField
        }
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
    }

    @ViewBuilder
    private func content(_ s: ControlSection) -> some View {
        switch s {
        case .places: PlacesSection(model: model)
        case .view: viewRows
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
        Toggle("Status (HUD)", isOn: $model.showHUD)
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
    case places, view, look, lighting, interior, overlays, quality, diagnostics, controls
    var id: String { rawValue }

    var title: String {
        switch self {
        case .places: "Places"
        case .view: "View & navigation"
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
        case .look: "paintpalette"
        case .lighting: "light.max"
        case .interior: "circle.inset.filled"
        case .overlays: "square.on.square.dashed"
        case .quality: "sparkles"
        case .diagnostics: "gauge.with.dots.needle.67percent"
        case .controls: "keyboard"
        }
    }

    var keywords: [String] {
        switch self {
        case .places: ["location", "bookmark", "famous", "random", "fly", "go", "catalog"]
        case .view: ["coordinates", "re", "im", "zoom", "scale", "copy", "paste", "reset", "edge", "pan", "speed"]
        case .look: ["palette", "look", "colour", "color", "density", "offset", "smooth", "log", "cycling", "tint", "dense", "grey", "filter"]
        case .lighting: ["shading", "light", "angle", "height", "highlight", "specular", "outline", "edge", "glow"]
        case .interior: ["inside", "bubbles", "period", "angle", "multiplier", "distance", "interior", "detection", "atom"]
        case .overlays: ["orbit", "map", "mini-map", "minimap", "breadcrumb", "hud", "status", "iterates", "overlay"]
        case .quality: ["iterations", "auto", "maximum", "cap", "anti-aliasing", "aa", "samples", "preview", "quality"]
        case .diagnostics: ["gpu", "performance", "reference", "time", "chunks", "kernel", "deep", "stats"]
        case .controls: ["keys", "keyboard", "shortcuts", "mouse", "trackpad", "help", "gamepad"]
        }
    }

    var footer: String? {
        switch self {
        case .look: "A look sets palette, lighting, outline and interior colour together. Colours cycle only while the view rests; depth tint shifts the palette as you zoom."
        case .lighting: "Lighting, outline and glow use the distance estimate (a little GPU work per iteration)."
        case .interior: "Interior colourings analyse each interior pixel's attracting cycle on the GPU (period, multiplier, distance) at any depth; switching between them recolours instantly, switching from flat renders the view again."
        case .overlays: "The orbit follows the pointer (the crosshair in keyboard mode): c as a ring, then z₁…zₙ. Click a mini-map to jump there at the current zoom. O and M toggle them."
        case .quality: "While you move, each frame renders as much as fits in a refresh; at rest the view refines and gathers anti-aliasing samples near the boundary."
        default: nil
        }
    }
}

/// Places: the curated catalog of famous locations (thumbnails by category, Go / Fly, random
/// place). Extension point: the catalog comes from a Rust places library that is being built
/// separately; until it is wired, this section says so. `PlacesProvider` is the seam.
@MainActor
protocol PlacesProvider {
    var categories: [String] { get }
}

struct PlacesSection: View {
    let model: ExplorerModel

    var body: some View {
        ContentUnavailableView {
            Label("Places", systemImage: "mappin.and.ellipse")
        } description: {
            Text("A catalog of famous locations with thumbnails, grouped by category, with Go, Fly and Random. It arrives with the places library.")
        }
        .controlSize(.small)
    }
}

/// The palette picker: swatches of the built-in palettes in a grid (separate from the look,
/// which is a colouring preset). Extension point for a palette library: `setCustomPalette`
/// takes any palette as stops.
struct PalettePicker: View {
    @Bindable var model: ExplorerModel

    private let columns = [GridItem(.adaptive(minimum: 64), spacing: 6)]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Palette")
                Spacer()
                Text(model.palette.name).foregroundStyle(.secondary)
            }
            LazyVGrid(columns: columns, spacing: 6) {
                ForEach(ExplorerModel.palettes, id: \.id) { p in
                    Button {
                        model.paletteID = p.id
                    } label: {
                        PaletteSwatch(palette: p)
                            .frame(height: 16)
                            .overlay(RoundedRectangle(cornerRadius: 3, style: .continuous)
                                .strokeBorder(model.paletteID == p.id ? Color.accentColor : .clear, lineWidth: 2))
                    }
                    .buttonStyle(.plain)
                    .help(p.name)
                    .accessibilityLabel(p.name)
                    .accessibilityAddTraits(model.paletteID == p.id ? .isSelected : [])
                }
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
        row("map", "M · click a map", "Mini-map · jump there")
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
        row("map", "Keyboard: O / M", "Orbit overlay / mini-map")
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
