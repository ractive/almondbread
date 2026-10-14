import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// The app's scene, shared by the SwiftPM executable and the Xcode app targets.
public struct AlmondbreadScene: Scene {
    public init() {
        TestLaunch.prepare()
    }

    public var body: some Scene {
        WindowGroup {
            ContentView()
                .preferredColorScheme(.dark)
        }
        #if os(macOS)
        .defaultSize(width: 1280, height: 820)
        .commands { FractalCommands() }
        #endif
    }
}

// MARK: Focused values: menu commands act on the frontmost window's model.

struct ExplorerModelKey: FocusedValueKey {
    typealias Value = ExplorerModel
}

struct InspectorVisibleKey: FocusedValueKey {
    typealias Value = Binding<Bool>
}

extension FocusedValues {
    var explorerModel: ExplorerModel? {
        get { self[ExplorerModelKey.self] }
        set { self[ExplorerModelKey.self] = newValue }
    }
    var inspectorVisible: Binding<Bool>? {
        get { self[InspectorVisibleKey.self] }
        set { self[InspectorVisibleKey.self] = newValue }
    }
}

enum Clipboard {
    @MainActor
    static func copy(_ text: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #else
        UIPasteboard.general.string = text
        #endif
    }
}

#if os(macOS)
struct FractalCommands: Commands {
    @FocusedValue(\.explorerModel) private var model
    @FocusedValue(\.inspectorVisible) private var inspectorVisible

    var body: some Commands {
        CommandGroup(after: .pasteboard) {
            Divider()
            Button("Copy Coordinates") { model.map { Clipboard.copy($0.coordinatesText) } }
                .keyboardShortcut("c", modifiers: [.command, .shift])
                .disabled(model == nil)
        }
        CommandGroup(before: .toolbar) {
            Button("Zoom In") { model?.zoomIn() }
                .keyboardShortcut("=", modifiers: .command)
                .disabled(model == nil)
            Button("Zoom Out") { model?.zoomOut() }
                .keyboardShortcut("-", modifiers: .command)
                .disabled(model == nil)
            Button("Reset View") { model?.reset() }
                .keyboardShortcut("0", modifiers: .command)
                .disabled(model == nil)
            Divider()
            Button(inspectorVisible?.wrappedValue == true ? "Hide Inspector" : "Show Inspector") {
                inspectorVisible?.wrappedValue.toggle()
            }
            .keyboardShortcut("i", modifiers: .command)
            .disabled(inspectorVisible == nil)
            Divider()
        }
    }
}
#endif

/// Window tests (`--selftest`, `--interaction-test`) run in the background: they must never
/// take focus from the user. No Dock icon or menu bar (accessory policy), no activation, the
/// window ordered behind others and never key, App Nap off so pacing is not throttled.
@MainActor
enum TestLaunch {
    static var enabled: Bool {
        #if os(macOS)
        SelfTest.enabled || InteractionTest.enabled
        #else
        SelfTest.enabled
        #endif
    }
    private static var activity: NSObjectProtocol?

    static func prepare() {
        guard enabled, activity == nil else { return }
        activity = ProcessInfo.processInfo.beginActivity(options: [.userInitiated, .latencyCritical],
                                                         reason: "Almondbread window test")
        #if os(macOS)
        NSApplication.shared.setActivationPolicy(.accessory)
        #endif
    }

    #if os(macOS)
    /// Orders the window behind all others. SwiftUI shows the window after its content view
    /// moved in, which would put it in front of the user's windows: order back again then.
    static func placeInBackground(_ window: NSWindow) {
        guard enabled else { return }
        window.orderBack(nil)
        for delay in [0.0, 0.3, 1.0] {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak window] in window?.orderBack(nil) }
        }
    }

    /// Window tests that measure presentation need part of the window uncovered (an occluded
    /// window's drawables report no presentation time): in the bottom-right corner of its
    /// screen (wholly on it, one refresh rate) and still behind every other window, it
    /// usually shows wherever the user's windows leave desktop free. Never key, never in front.
    /// The screen with the highest refresh rate (a ProMotion panel next to a 60 Hz external
    /// display), unless `ALMONDBREAD_TEST_SCREEN=main`: frame pacing at 120 Hz is what the
    /// interaction test has to measure (kb: synthesis/benchmarks, FL).
    static func placeAtScreenEdge(_ window: NSWindow) {
        let useMain = ProcessInfo.processInfo.environment["ALMONDBREAD_TEST_SCREEN"] == "main"
        let fastest = NSScreen.screens.max { $0.maximumFramesPerSecond < $1.maximumFramesPerSecond }
        guard enabled, let screen = (useMain ? nil : fastest) ?? window.screen ?? NSScreen.main else { return }
        let f = screen.visibleFrame
        window.setFrameOrigin(NSPoint(x: f.maxX - window.frame.width, y: f.minY))
        window.orderBack(nil)
    }
    #endif
}
