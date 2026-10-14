import Foundation

/// Held-key navigation shared by the macOS and iPad views (kb: decisions/controls):
/// WASD / arrows pan continuously at a fixed fraction of the view width per second (the same
/// feel at every depth), Space zooms in toward the pointer, Shift+Space out, Q / E scale the
/// zoom speed. Motion is integrated per display frame from the set of held keys, so key
/// repeat timing does not matter and key-up stops motion at once.
@MainActor
final class KeyControls {
    enum Key: Hashable { case up, down, left, right, zoom }

    /// Pan speed in view widths per second.
    static let panSpeed = 0.4

    private(set) var held: Set<Key> = []
    var shift = false
    var isMoving: Bool { !held.isEmpty }

    /// Handles a key press. `chars` is charactersIgnoringModifiers; `arrow` a decoded arrow key.
    /// Returns true if the key is ours (the caller then swallows it: no system beep).
    func press(_ chars: String, arrow: Key? = nil, isRepeat: Bool, model: FractalModel) -> Bool {
        if let arrow {
            held.insert(arrow)
            return true
        }
        switch chars.lowercased() {
        case "w": held.insert(.up)
        case "s": held.insert(.down)
        case "a": held.insert(.left)
        case "d": held.insert(.right)
        case " ": held.insert(.zoom)
        case "q": model.setZoomSpeed(model.zoomSpeed * 0.8)
        case "e": model.setZoomSpeed(model.zoomSpeed * 1.25)
        default: return false
        }
        return true
    }

    func release(_ chars: String, arrow: Key? = nil) -> Bool {
        if let arrow {
            held.remove(arrow)
            return true
        }
        switch chars.lowercased() {
        case "w": held.remove(.up)
        case "s": held.remove(.down)
        case "a": held.remove(.left)
        case "d": held.remove(.right)
        case " ": held.remove(.zoom)
        case "q", "e": break
        default: return false
        }
        return true
    }

    /// Focus lost: a key-up may never arrive, so stop everything.
    func releaseAll() {
        held.removeAll()
        shift = false
    }

    /// Pan in view points (content follows the camera: positive y moves the camera down, as
    /// `Viewport.pan` expects) and the zoom factor for a frame of `dt` seconds.
    func step(dt: Double, viewWidth: Double, zoomSpeed: Double) -> (dx: Double, dy: Double, zoom: Double) {
        var x = 0.0, y = 0.0
        if held.contains(.left) { x += 1 }
        if held.contains(.right) { x -= 1 }
        if held.contains(.up) { y -= 1 }
        if held.contains(.down) { y += 1 }
        let len = (x * x + y * y).squareRoot()
        let v = len > 0 ? KeyControls.panSpeed * viewWidth * dt / len : 0   // diagonals not faster
        let z = held.contains(.zoom) ? exp((shift ? -1 : 1) * zoomSpeed * dt) : 1
        return (x * v, y * v, z)
    }
}
