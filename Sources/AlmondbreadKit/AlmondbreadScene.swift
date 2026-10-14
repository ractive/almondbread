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

struct FractalModelKey: FocusedValueKey {
    typealias Value = FractalModel
}

struct InspectorVisibleKey: FocusedValueKey {
    typealias Value = Binding<Bool>
}

extension FocusedValues {
    var fractalModel: FractalModel? {
        get { self[FractalModelKey.self] }
        set { self[FractalModelKey.self] = newValue }
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
    @FocusedValue(\.fractalModel) private var model
    @FocusedValue(\.inspectorVisible) private var inspectorVisible

    var body: some Commands {
        CommandGroup(after: .pasteboard) {
            Divider()
            Button("Copy Coordinates") { model.map { Clipboard.copy($0.coordinatesText) } }
                .keyboardShortcut("c", modifiers: [.command, .shift])
                .disabled(model == nil)
        }
        CommandGroup(before: .toolbar) {
            Button("Zoom In") { model?.viewport.zoom(by: 2, aroundOffset: 0, 0) }
                .keyboardShortcut("=", modifiers: .command)
                .disabled(model == nil)
            Button("Zoom Out") { model?.viewport.zoom(by: 0.5, aroundOffset: 0, 0) }
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
    static func placeInBackground(_ window: NSWindow) {
        guard enabled else { return }
        window.orderBack(nil)
    }
    #endif
}
