import AlmondbreadCore
import SwiftUI

/// A setting behind a section's "More…" (kb: decisions/controls "Control center", "Essential
/// rows and More"). The More line counts the ones that differ from their reference: for a field
/// a look sets (palette mapping, lighting, outline, glow, interior colour) the current look's
/// value, for any other its default. Rust's `lookSettings(id:)` is both at once: the defaults
/// with the look applied.
struct MoreSetting {
    let name: String
    let section: ControlSection
    /// Whether its row is there now (a conditional row that is hidden is not counted).
    let shown: @MainActor (ExplorerModel) -> Bool
    /// Whether the model's value differs from the reference.
    let differs: @MainActor (ExplorerModel, ExplorerSettings) -> Bool
    /// Puts the reference value back.
    let reset: @MainActor (ExplorerModel, ExplorerSettings) -> Void
}

enum ControlMore {
    /// The open Mores (section raw values, comma-separated), remembered like the sections.
    static let openKey = "controlCenter.moreOpen"

    /// The hidden settings of every section, in row order.
    @MainActor static let settings: [MoreSetting] = {
        func entry<T>(_ name: String, _ section: ControlSection,
                      _ path: ReferenceWritableKeyPath<ExplorerModel, T>,
                      same: @escaping (T, T) -> Bool,
                      shown: @escaping @MainActor (ExplorerModel) -> Bool = { _ in true },
                      ref: @escaping (ExplorerSettings) -> T) -> MoreSetting {
            MoreSetting(name: name, section: section, shown: shown,
                        differs: { m, r in !same(m[keyPath: path], ref(r)) },
                        reset: { m, r in m[keyPath: path] = ref(r) })
        }
        func number(_ name: String, _ section: ControlSection, _ path: ReferenceWritableKeyPath<ExplorerModel, Double>,
                    shown: @escaping @MainActor (ExplorerModel) -> Bool = { _ in true },
                    ref: @escaping (ExplorerSettings) -> Double) -> MoreSetting {
            entry(name, section, path, same: near, shown: shown, ref: ref)
        }
        func flag(_ name: String, _ section: ControlSection, _ path: ReferenceWritableKeyPath<ExplorerModel, Bool>,
                  ref: @escaping (ExplorerSettings) -> Bool) -> MoreSetting {
            entry(name, section, path, same: ==, ref: ref)
        }
        func colour(_ name: String, _ section: ControlSection, _ path: ReferenceWritableKeyPath<ExplorerModel, SIMD3<Float>>,
                    ref: @escaping (ExplorerSettings) -> SIMD3<Float>) -> MoreSetting {
            // (A colour picker's round trip moves a component by ~1e-7.)
            entry(name, section, path, same: { (a: SIMD3<Float>, b: SIMD3<Float>) in ((a - b) * (a - b)).max() < 1e-8 }, ref: ref)
        }
        return [
            // Navigation: app-wide preferences (their defaults; no look sets them).
            entry("Edge panning", .view, \.edgePanning, same: ==) { _ in .whileZooming },
            entry("Autopilot target lock", .view, \.autopilotLock, same: ==, shown: { $0.has(\.content) }) { _ in true },
            entry("Lane assist", .view, \.laneAssist, same: ==) { _ in false },
            // Journey & cinematic effects
            number("Spin", .journey, \.cinematicSpin) { $0.cinematicSpin },
            number("Hue drift", .journey, \.cinematicHueDrift) { $0.cinematicHueDrift },
            number("Phase drift", .journey, \.cinematicPhaseDrift) { $0.cinematicPhaseDrift },
            number("Cycling in flight", .journey, \.cinematicFlyCycle) { $0.cinematicFlyCycle },
            // Look & colour
            number("Offset", .look, \.colorOffset) { $0.colorOffset },
            flag("Logarithmic", .look, \.logColoring) { $0.logColoring },
            flag("Smooth", .look, \.smoothColoring) { $0.smoothColoring },
            flag("Palette anti-aliasing", .look, \.paletteFilter) { $0.paletteFilter },
            flag("Calm dense regions", .look, \.denseCalm) { $0.denseCalm },
            number("Colour cycling", .look, \.cycleSpeed) { $0.cycleSpeed },
            number("Depth tint", .look, \.depthTint) { $0.depthTint },
            // Lighting & outline (every one set by the looks)
            number("Light angle", .lighting, \.lightAngle) { $0.lightAngle },
            number("Light height", .lighting, \.lightHeight) { $0.lightHeight },
            number("Highlight", .lighting, \.specular) { $0.specular },
            number("Outline width", .lighting, \.outlineWidth) { $0.outlineWidth },
            colour("Outline colour", .lighting, \.edgeColor) { SIMD3($0.edgeRed, $0.edgeGreen, $0.edgeBlue) },
            number("Glow width", .lighting, \.glowWidth) { $0.glowWidth },
            colour("Glow colour", .lighting, \.glowColor) { SIMD3($0.glowRed, $0.glowGreen, $0.glowBlue) },
            // Interior
            number("Stripes", .interior, \.interiorStripes, shown: { $0.interiorColoring == .angle }) { $0.interiorStripes },
            flag("Interior detection", .interior, \.interiorDetection) { $0.interiorDetection },
            // Overlays (Swift-side defaults, as `reset(.overlays)` sets them)
            entry("Iterates", .overlays, \.orbitLength, same: ==,
                  shown: { $0.orbitEnabled && $0.has(\.orbitOverlay) }) { _ in 50 },
            entry("Mini-map levels", .overlays, \.mapLevels, same: ==, shown: { $0.mapEnabled }) { _ in 3 },
            entry("Mini-map size", .overlays, \.mapSize, same: near, shown: { $0.mapEnabled }) { _ in 180 },
            entry("Mini-map corner", .overlays, \.mapPlacement, same: ==, shown: { $0.mapEnabled }) { _ in .bottomLeft },
            entry("Cosmic size scale", .overlays, \.cosmicScale, same: ==) { _ in false },
            entry("Always show compass", .overlays, \.compassAlways, same: ==) { _ in false },
            entry("Autopilot target marker", .overlays, \.targetMarker, same: ==) { _ in true },
            entry("Status (HUD)", .overlays, \.showHUD, same: ==) { _ in true },
            entry("Developer details in the HUD", .overlays, \.hudDetails, same: ==) { _ in false },
            // Iterations & quality
            entry("Upper limit", .quality, \.maxIterationsCap, same: ==, shown: { $0.autoIterations }) { Int($0.maxIterationsCap) },
            entry("Anti-aliasing", .quality, \.supersampling, same: ==) { Supersampling(rawValue: Int($0.supersampling)) ?? .x9 },
            flag("Interactive preview", .quality, \.interactivePreview) { $0.interactivePreview },
        ]
    }()

    static func near(_ a: Double, _ b: Double) -> Bool { abs(a - b) <= 1e-9 * max(1, abs(a), abs(b)) }

    /// The reference record for a look (cached: one FFI call per look).
    @MainActor static func reference(look: String) -> ExplorerSettings {
        if let r = references[look] { return r }
        let r = lookSettings(id: look)
        references[look] = r
        return r
    }
    @MainActor private static var references: [String: ExplorerSettings] = [:]

    /// The names of `section`'s hidden settings that differ from the reference now.
    @MainActor static func changed(_ section: ControlSection, model: ExplorerModel) -> [String] {
        let r = reference(look: model.lookID)
        return settings.filter { $0.section == section && $0.shown(model) && $0.differs(model, r) }.map(\.name)
    }

    /// Puts `section`'s changed hidden settings back to the reference (the More line's ↺).
    @MainActor static func reset(_ section: ControlSection, model: ExplorerModel) {
        let r = reference(look: model.lookID)
        for s in settings where s.section == section && s.shown(model) && s.differs(model, r) { s.reset(model, r) }
    }

    /// How many rows the section's More holds now (the accessibility label's "More, 6 settings"):
    /// its settings whose row is there plus the rows that are no setting of their own (buttons,
    /// the cycle speed under its switch, the palette import).
    @MainActor static func rowCount(_ section: ControlSection, model: ExplorerModel) -> Int {
        let extra: Int = switch section {
        // Autopilot / fly-to buttons, Rotation's Straighten; iOS: Seek minibrot.
        case .view: 2 + (isIOS && model.has(\.seek) ? 1 : 0)
        case .look: (model.cycleSpeed != 0 ? 1 : 0) + 1
        default: 0
        }
        return settings.filter { $0.section == section && $0.shown(model) }.count + extra
    }

    #if os(iOS)
    private static let isIOS = true
    #else
    private static let isIOS = false
    #endif

    /// Whether a section has a More at all (Places, Tours and Controls have none).
    static func hasMore(_ section: ControlSection) -> Bool { !section.moreKeywords.isEmpty }
}

/// A section's "More…" line at the end of its essential rows and, when open, the rows behind it.
/// Closed by default; the open state is remembered per section. A search whose query matches a
/// hidden row opens it (`forced`), as the search opens collapsed sections; a click during a
/// search flips it for that search only (`SearchToggles`), the remembered state stays.
struct MoreRows<Content: View>: View {
    let section: ControlSection
    let model: ExplorerModel
    var forced = false
    @ViewBuilder var content: Content
    @AppStorage(ControlMore.openKey, store: TestLaunch.ephemeral ? UIDemo.defaults : .standard)
    private var openStorage = ""
    @Environment(SearchToggles.self) private var searchToggles: SearchToggles?

    var body: some View {
        let stored = Set(openStorage.split(separator: ",").map(String.init))
        let storedOpen = stored.contains(section.rawValue)
        let id = SearchToggles.moreID(section)
        let searching = searchToggles?.searching ?? false
        let open = SearchToggles.isOpen(stored: storedOpen, searchDefault: forced || storedOpen,
                                        searching: searching, flipped: searchToggles?.isFlipped(id) ?? false)
        MoreLine(section: section, model: model, open: open) {
            if let searchToggles, searchToggles.searching {
                withAnimation(.snappy(duration: 0.2)) { searchToggles.toggle(id) }
                return
            }
            var s = stored
            if s.contains(section.rawValue) { s.remove(section.rawValue) } else { s.insert(section.rawValue) }
            withAnimation(.snappy(duration: 0.2)) { openStorage = s.sorted().joined(separator: ",") }
        }
        if open { content }
    }
}

/// "› More…  · 2 changed  ↺": the disclosure (an accent chevron that turns down when open,
/// the whole row its hit target, a little air above it) and, when hidden settings differ from
/// their reference, how many with a reset of just those. A leaf: it alone follows the hidden
/// values (kb: decisions/controls "Essential rows and More").
struct MoreLine: View {
    let section: ControlSection
    let model: ExplorerModel
    let open: Bool
    let toggle: () -> Void

    var body: some View {
        let changed = ControlMore.changed(section, model: model)
        let count = ControlMore.rowCount(section, model: model)
        HStack(spacing: 6) {
            Button(action: toggle) {
                HStack(spacing: 6) {
                    // Only the chevron in the accent colour: the label in the rows' own colour
                    // reads on every look's tint and is not taken for a link (the tour link).
                    Image(systemName: "chevron.right")
                        .font(.caption.weight(.bold))
                        .rotationEffect(.degrees(open ? 90 : 0))
                        .frame(width: 12)
                        .foregroundStyle(Color.accentColor)
                    Text("More…").fontWeight(.semibold)
                    if !changed.isEmpty {
                        Text("· \(changed.count) changed").foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 4)
                }
                .foregroundStyle(.primary)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("More, \(count) \(section.title.lowercased()) setting\(count == 1 ? "" : "s")")
            .accessibilityValue((open ? "expanded" : "collapsed")
                                + (changed.isEmpty ? "" : ", \(changed.count) changed: \(changed.joined(separator: ", "))"))
            .accessibilityIdentifier("more-\(section.rawValue)")
            .help((open ? "Hide" : "Show") + " the section's \(count) other settings"
                  + (changed.isEmpty ? "" : "; changed: \(changed.joined(separator: ", "))"))
            if !changed.isEmpty {
                Button {
                    ControlMore.reset(section, model: model)
                } label: {
                    Image(systemName: "arrow.counterclockwise").font(.caption)
                        .frame(minWidth: 20, minHeight: 20)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
                .help("Put back \(changed.joined(separator: ", ")) (the look's values, else the defaults)")
                .accessibilityLabel("Reset the changed hidden settings")
                .accessibilityIdentifier("more-\(section.rawValue)-reset")
            }
        }
        .padding(.top, 3)
        #if os(iOS)
        .frame(minHeight: 36)
        #endif
    }
}
