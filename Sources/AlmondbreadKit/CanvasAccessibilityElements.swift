import AlmondbreadCore
import SwiftUI

// The platform elements for the canvas and the Rust-drawn controls (CanvasAccessibility.swift;
// kb: plans/a11y-canvas-controls). Each element keeps its id for the view's lifetime (VoiceOver
// keeps its focus on the object) and reads its words through the view's cache, which goes to
// Rust at most every 0.1 s.

/// What both platform views offer their elements.
@MainActor
protocol CanvasAccessibilityHost: AnyObject {
    /// Updates the cache (at most every 0.1 s) and the elements from it.
    func refreshAccessibility()
    /// Runs a control's action; true if it did something.
    func activate(_ control: CanvasControl) -> Bool
    /// The canvas element's value.
    var canvasAccessibilityValue: String { get }
    var model: ExplorerModel { get }
}

#if os(macOS)
import AppKit

/// A Rust-drawn control as an AppKit accessibility element of the fractal view.
final class CanvasControlElement: NSAccessibilityElement {
    private(set) var control: CanvasControl
    private weak var view: FractalNSView?

    init(_ control: CanvasControl, view: FractalNSView) {
        self.control = control
        self.view = view
        super.init()
    }

    func update(_ c: CanvasControl) { control = c }

    private func fresh() -> CanvasControl {
        MainActor.assumeIsolated { view?.refreshAccessibility() }
        return control
    }

    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? {
        switch control.role {
        case .button: .button
        case .image: .image
        case .text: .staticText
        }
    }
    override func accessibilityLabel() -> String? { fresh().label }
    override func accessibilityValue() -> Any? {
        let v = fresh().value
        return v.isEmpty ? nil : v
    }
    override func accessibilityIdentifier() -> String { control.id }
    override func isAccessibilitySelected() -> Bool { fresh().selected }
    override func accessibilityParent() -> Any? { view }
    override func accessibilityFrame() -> NSRect {
        MainActor.assumeIsolated { view?.screenRect(canvas: fresh().frame) ?? .zero }
    }
    override func accessibilityPerformPress() -> Bool {
        MainActor.assumeIsolated {
            guard let view, control.action != nil else { return false }
            return view.activate(control)
        }
    }
}

/// The canvas itself: an image whose value is the view in one sentence; zoom in / out as custom
/// actions.
final class CanvasElement: NSAccessibilityElement {
    private weak var view: FractalNSView?

    init(view: FractalNSView) {
        self.view = view
        super.init()
    }

    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .image }
    override func accessibilityLabel() -> String? { CanvasAccessibility.canvasLabel }
    override func accessibilityIdentifier() -> String { CanvasAccessibility.canvasIdentifier }
    override func accessibilityValue() -> Any? {
        MainActor.assumeIsolated {
            view?.refreshAccessibility()
            return view?.canvasAccessibilityValue
        }
    }
    override func accessibilityParent() -> Any? { view }
    override func accessibilityFrame() -> NSRect {
        MainActor.assumeIsolated {
            guard let view else { return .zero }
            return NSAccessibility.screenRect(fromView: view, rect: view.bounds)
        }
    }
    override func accessibilityCustomActions() -> [NSAccessibilityCustomAction]? {
        [("Zoom in", true), ("Zoom out", false)].map { name, inwards in
            NSAccessibilityCustomAction(name: name) { [weak self] in
                MainActor.assumeIsolated { self?.view?.model.accessibilityZoom(inwards: inwards) }
                return true
            }
        }
    }
}

#else
import UIKit

/// A Rust-drawn control as a UIKit accessibility element of the fractal view.
final class CanvasControlElementUI: UIAccessibilityElement {
    private(set) var control: CanvasControl
    private weak var view: FractalUIView?

    init(_ control: CanvasControl, view: FractalUIView) {
        self.control = control
        self.view = view
        super.init(accessibilityContainer: view)
        update(control)
    }

    func update(_ c: CanvasControl) {
        control = c
        accessibilityIdentifier = c.id
        accessibilityFrameInContainerSpace = c.frame
        var t: UIAccessibilityTraits = switch c.role {
        case .button: .button
        case .image: .image
        case .text: [.staticText, .updatesFrequently]
        }
        if c.selected { t.insert(.selected) }
        accessibilityTraits = t
    }

    private func fresh() -> CanvasControl {
        MainActor.assumeIsolated { view?.refreshAccessibility() }
        return control
    }

    override var accessibilityLabel: String? {
        get { fresh().label }
        set {}
    }
    override var accessibilityValue: String? {
        get {
            let v = fresh().value
            return v.isEmpty ? nil : v
        }
        set {}
    }
    override func accessibilityActivate() -> Bool {
        MainActor.assumeIsolated {
            guard let view, control.action != nil else { return false }
            return view.activate(control)
        }
    }
}

/// The canvas: touches pass through to the gestures (direct interaction), its value is the view
/// in one sentence (the UI tests' state string with `ALMONDBREAD_UITEST=1`).
final class CanvasElementUI: UIAccessibilityElement {
    private weak var view: FractalUIView?

    init(view: FractalUIView) {
        self.view = view
        super.init(accessibilityContainer: view)
        accessibilityIdentifier = CanvasAccessibility.canvasIdentifier
        accessibilityTraits = .allowsDirectInteraction
        accessibilityCustomActions = [("Zoom in", true), ("Zoom out", false)].map { name, inwards in
            UIAccessibilityCustomAction(name: name) { [weak view] _ in
                MainActor.assumeIsolated { view?.model.accessibilityZoom(inwards: inwards) }
                return true
            }
        }
    }

    override var accessibilityLabel: String? {
        get { CanvasAccessibility.canvasLabel }
        set {}
    }
    override var accessibilityValue: String? {
        get { MainActor.assumeIsolated { view?.canvasAccessibilityValue } }
        set {}
    }
}
#endif
