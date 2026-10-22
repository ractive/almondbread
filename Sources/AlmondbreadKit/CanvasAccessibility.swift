import AlmondbreadCore
import CoreGraphics
import QuartzCore

// Accessibility of the canvas and of the controls Rust draws into it (kb:
// plans/a11y-canvas-controls): the cockpit's icons, the help badges, the compass, the mini-maps,
// the Julia panel, the π meter and the cosmic size scale are pixels and Rust hit areas, so the
// platform views describe them as accessibility elements: Rust says what was drawn where and in
// which words (`Explorer.accessibleControls`, `accessibilitySummary`); this file maps each id to
// its action (the same call a click or a tap makes) and keeps a cache both views share. Nothing
// runs per frame: the cache is filled when an accessibility client asks (at most every 0.1 s),
// and while VoiceOver or Switch Control runs, at the ~10 Hz stats cadence to announce layout
// changes.

/// What activating a canvas control does.
enum CanvasAction: Equatable {
    /// A cockpit icon: `ExplorerModel.cockpitClick` (switch level, toggle lane assist).
    case cockpit(String)
    /// A help badge: its popover (`helpPress` at the badge).
    case help
    /// The compass: turn to north (`compassTap`).
    case compass
    /// A mini-map panel: jump to the point at its centre (`mapClick`).
    case map
}

/// One control as the platform elements present it.
struct CanvasControl: Equatable {
    let id: String
    let role: AccessibleRole
    let label: String
    let value: String
    let selected: Bool
    /// View points from the top left.
    let frame: CGRect

    var action: CanvasAction? { CanvasAccessibility.action(for: id) }
}

enum CanvasAccessibility {
    /// The canvas element's label and identifier (UI tests find it as `fractal`).
    static let canvasLabel = "Mandelbrot set"
    static let canvasIdentifier = "fractal"

    static func action(for id: String) -> CanvasAction? {
        if id.hasPrefix("cockpit-") { return .cockpit(String(id.dropFirst("cockpit-".count))) }
        if id.hasPrefix("help-") { return .help }
        if id == "compass" { return .compass }
        if id.hasPrefix("map-") { return .map }
        // The Mandelbrot panel of a Julia view: a click on it goes back (Rust decides).
        if id == "mandelbrot-panel" { return .map }
        return nil
    }

    /// Rust's records as controls; drops any without a usable frame.
    static func controls(_ items: [AccessibleControl]) -> [CanvasControl] {
        items.compactMap { c in
            guard [c.x, c.y, c.width, c.height].allSatisfy(\.isFinite), c.width > 0, c.height > 0 else { return nil }
            return CanvasControl(id: c.id, role: c.role, label: c.label, value: c.value, selected: c.selected,
                                 frame: CGRect(x: c.x, y: c.y, width: c.width, height: c.height))
        }
    }

    /// The centre of `frame` (view points from the top left) as the hit tests take it: view
    /// points from the view centre, y up.
    static func centred(_ frame: CGRect, in size: CGSize) -> (x: Double, y: Double) {
        (Double(frame.midX - size.width / 2), Double(size.height / 2 - frame.midY))
    }

    /// Elements came or went or moved (VoiceOver is told the layout changed).
    static func layoutChanged(_ a: [CanvasControl], _ b: [CanvasControl]) -> Bool {
        a.count != b.count || zip(a, b).contains { $0.id != $1.id || !$0.frame.equalTo($1.frame) }
    }

    /// The control under `p` (view points from the top left); the smallest where they nest (a
    /// help badge in its panel's corner).
    static func control(at p: CGPoint, in controls: [CanvasControl]) -> CanvasControl? {
        controls.filter { $0.frame.contains(p) }.min { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height }
    }
}

/// The cache both platform views keep (main thread).
@MainActor
final class CanvasAccessibilityState {
    private(set) var controls: [CanvasControl] = []
    private(set) var summary = ""
    /// Counts reads from Rust (the views update their elements when it moved).
    private(set) var generation = 0
    private var fetchedAt: CFTimeInterval = -.infinity
    /// How old the cache may be when an element is read.
    static let maxAge: CFTimeInterval = 0.1

    /// The next read goes to Rust (after an action changed what is drawn).
    func invalidate() { fetchedAt = -.infinity }

    /// Reads controls and summary from Rust if the cache is older than `maxAge` (or `force`);
    /// returns true if the layout changed (ids or frames).
    @discardableResult
    func refresh(_ explorer: Explorer?, force: Bool = false) -> Bool {
        let now = CACurrentMediaTime()
        guard let explorer, force || now - fetchedAt >= Self.maxAge else { return false }
        fetchedAt = now
        generation &+= 1
        let new = CanvasAccessibility.controls(explorer.accessibleControls())
        summary = explorer.accessibilitySummary()
        let changed = CanvasAccessibility.layoutChanged(controls, new)
        controls = new
        return changed
    }
}

extension ExplorerModel {
    /// Activates a canvas control (VoiceOver double tap, Switch Control, the Mac's AXPress):
    /// the call a click or a tap on it makes. `size`: the canvas in points.
    @discardableResult
    func activate(_ control: CanvasControl, size: CGSize) -> Bool {
        let p = CanvasAccessibility.centred(control.frame, in: size)
        switch control.action {
        case .cockpit(let level):
            cockpitClick(level)
            return true
        case .help: return helpPress(x: p.x, y: p.y, size: size)
        case .compass: return compassTap(x: p.x, y: p.y)
        case .map: return mapClick(x: p.x, y: p.y, fly: false)
        case nil: return false
        }
    }

    /// The canvas's custom actions: zoom ×2 in or out, like the menu commands.
    func accessibilityZoom(inwards: Bool) {
        if inwards { zoomIn() } else { zoomOut() }
    }
}
