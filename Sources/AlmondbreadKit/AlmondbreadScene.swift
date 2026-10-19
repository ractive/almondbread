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
        HangWatchdog.start()
        HangTest.start()
        #if os(macOS)
        // Toolbar tooltips after 0.3 s instead of AppKit's default of about a second (user
        // request). A registered default, so `defaults write ... NSInitialToolTipDelay` still wins.
        UserDefaults.standard.register(defaults: ["NSInitialToolTipDelay": 300])
        #endif
    }

    public var body: some Scene {
        WindowGroup {
            ContentView()
                .preferredColorScheme(.dark)
        }
        #if os(macOS)
        .defaultSize(width: 1280, height: 820)
        #endif
        // The iPad's menu bar and ⌘-key shortcuts (iPadOS) get the same commands as the Mac
        // (kb: decisions/controls "Touch").
        .commands { FractalCommands() }
        #if os(macOS)
        HelpWindow()
        Settings {
            AppSettingsView()
                .preferredColorScheme(.dark)
        }
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

struct FlyToVisibleKey: FocusedValueKey {
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
    var flyToVisible: Binding<Bool>? {
        get { self[FlyToVisibleKey.self] }
        set { self[FlyToVisibleKey.self] = newValue }
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

    @MainActor
    static func text() -> String? {
        #if os(macOS)
        NSPasteboard.general.string(forType: .string)
        #else
        UIPasteboard.general.string
        #endif
    }
}

struct FractalCommands: Commands {
    @FocusedValue(\.explorerModel) private var model
    @FocusedValue(\.inspectorVisible) private var inspectorVisible
    @FocusedValue(\.flyToVisible) private var flyToVisible

    var body: some Commands {
        CommandGroup(after: .pasteboard) {
            Divider()
            Button("Copy Coordinates") { model.map { Clipboard.copy($0.coordinatesText) } }
                .keyboardShortcut("c", modifiers: [.command, .shift])
                .disabled(model == nil)
            Button("Go to Copied Coordinates") { _ = model?.pasteCoordinates() }
                .keyboardShortcut("v", modifiers: [.command, .shift])
                .disabled(model == nil)
            // Animated: zoom out, pan, zoom in, landing exactly (kb: decisions/controls, fly to).
            Button("Fly to Copied Coordinates") { _ = model?.flyToCopiedCoordinates() }
                .keyboardShortcut("g", modifiers: [.command, .shift])
                .disabled(model == nil)
            Button("Fly to Coordinates…") { flyToVisible?.wrappedValue = true }
                .keyboardShortcut("g", modifiers: .command)
                .disabled(flyToVisible == nil)
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
            // Z and Esc are handled by the canvas's key monitor (a plain-key menu shortcut
            // would fire as well); the menu items are for discovery.
            Button("Autopilot On/Off (Z)") { model?.toggleAutopilot() }
                .disabled(model == nil)
            // Q / E and L likewise (kb: decisions/controls "Driving levels").
            Button("Cruise In / Faster (E)") { model?.pedalIn() }
                .disabled(model == nil)
            Button("Cruise Out / Slower (Q)") { model?.pedalOut() }
                .disabled(model == nil)
            Toggle("Lane Assist (L)", isOn: Binding(get: { model?.laneAssist ?? false },
                                                    set: { model?.laneAssist = $0 }))
                .disabled(model == nil)
            Button("Journey On/Off (J)") { model?.toggleJourney() }
                .disabled(model == nil)
            Button("Daily Journey") { model?.startDailyJourney() }
                .disabled(model == nil)
            Button("Seek Minibrot (N)") { model?.seekMinibrot() }
                .disabled(model == nil)
            Button("Copy Journey Seed") { model?.copyJourney() }
                .disabled(model?.journeyShareText == nil)
            Button("Straighten View (/)") { model?.straighten() }
                .disabled(model == nil)
            Toggle("Cinematic Effects", isOn: Binding(get: { model?.cinematic ?? false },
                                                      set: { model?.cinematic = $0 }))
                .disabled(model == nil)
            Button("Stop Cruise, Autopilot, Flight or Journey (Esc)") { model?.stopMotion() }
                .disabled(model == nil)
            Divider()
            Button(inspectorVisible?.wrappedValue == true ? "Hide Control Center" : "Show Control Center") {
                inspectorVisible?.wrappedValue.toggle()
            }
            .keyboardShortcut("i", modifiers: .command)
            .disabled(inspectorVisible == nil)
            Button("Search Settings") {
                inspectorVisible?.wrappedValue = true
                model?.requestSearchFocus()
            }
            .keyboardShortcut("f", modifiers: .command)
            .disabled(inspectorVisible == nil || model == nil)
            Toggle("Orbit Overlay", isOn: Binding(get: { model?.orbitEnabled ?? false },
                                                  set: { model?.orbitEnabled = $0 }))
                .keyboardShortcut("o", modifiers: [.command, .option])
                .disabled(model == nil)
            Toggle("Mini-map", isOn: Binding(get: { model?.mapEnabled ?? false },
                                             set: { model?.mapEnabled = $0 }))
                .keyboardShortcut("m", modifiers: [.command, .option])
                .disabled(model == nil)
            // Guided tours (kb: decisions/controls "Guided tours"); → ← and Esc on the canvas. The
            // π meter has no switch of its own: the tour "π in the Mandelbrot set" drives it.
            TourMenu(model: model)
            Toggle("Cosmic Size Scale", isOn: Binding(get: { model?.cosmicScale ?? false },
                                                      set: { model?.cosmicScale = $0 }))
                .disabled(model == nil)
            Toggle("Always Show Compass", isOn: Binding(get: { model?.compassAlways ?? false },
                                                        set: { model?.compassAlways = $0 }))
                .disabled(model == nil)
            Toggle("Autopilot Target Marker", isOn: Binding(get: { model?.targetMarker ?? true },
                                                            set: { model?.targetMarker = $0 }))
                .disabled(model == nil)
            Toggle("Status Overlay", isOn: Binding(get: { model?.showHUD ?? true },
                                                   set: { model?.showHUD = $0 }))
                .disabled(model == nil)
            Toggle("Developer Details in Status", isOn: Binding(get: { model?.hudDetails ?? false },
                                                                set: { model?.hudDetails = $0 }))
                .disabled(model == nil || model?.showHUD == false)
            Divider()
        }
        CommandGroup(replacing: .help) {
            #if os(macOS)
            HelpMenuItem()
            // The first-launch walkthrough again (kb: decisions/controls "Walkthrough").
            Button("Show the Walkthrough") { model?.startTour(ExplorerModel.walkthroughID) }
                .disabled(model == nil)
            #else
            // The iPad's menu bar (kb: decisions/controls "Touch"): the help sheet, the touch hints.
            Button("Almondbread Help") { HelpOpener()(.gettingStarted) }
                .keyboardShortcut("?", modifiers: .command)
            Button("Show Touch Hints") { TouchHints.shared.show() }
            Button("Show the Walkthrough") { model?.startTour(ExplorerModel.walkthroughID) }
                .disabled(model == nil)
            #endif
        }
    }
}

/// Window tests (`--selftest`, `--interaction-test`) run in the background: they must never
/// take focus from the user. No Dock icon or menu bar (accessory policy), no activation, the
/// window ordered behind others and never key, App Nap off so pacing is not throttled.
@MainActor
public enum TestLaunch {
    /// Any window test (`--selftest`, `--interaction-test`, `--ui-demo`, `--flight-bench=`):
    /// the app must stay in the background. Entry points check this before promoting a bare
    /// executable to a regular, active app.
    public static var enabled: Bool {
        #if os(macOS)
        SelfTest.enabled || InteractionTest.enabled || UIDemo.enabled || FlightBench.enabled || HangTest.enabled
        #else
        SelfTest.enabled || HangTest.enabled
        #endif
    }
    private static var activity: NSObjectProtocol?

    /// The iOS UI tests (XCTest's launch environment `ALMONDBREAD_UITEST=1`).
    static let uiTest = ProcessInfo.processInfo.environment["ALMONDBREAD_UITEST"] == "1"

    /// Settings are neither restored nor stored: every window test, every `--ui-demo` scene and
    /// the iOS UI tests start from the defaults and leave the user's settings as they were
    /// (an "Orbit Here" in a test used to switch the orbit and the mini-map on for the next
    /// normal launch). The first-run touch hints count as not seen there.
    static var ephemeral: Bool { enabled || UIDemo.enabled || uiTest }

    /// `ALMONDBREAD_TEST_FOREGROUND=1`: the interaction test's window comes to the front, key,
    /// on the ProMotion panel. Only for a person running the test themselves (`make
    /// interaction-test-120`): macOS drives a background window's display link at 20-60 Hz,
    /// so 120 Hz presentation can only be measured in front (kb: synthesis/benchmarks,
    /// "120 Hz on ProMotion"). Agents never set it.
    static var foreground: Bool {
        #if os(macOS)
        InteractionTest.enabled && ProcessInfo.processInfo.environment["ALMONDBREAD_TEST_FOREGROUND"] == "1"
        #else
        false
        #endif
    }

    static func prepare() {
        guard enabled, activity == nil else { return }
        activity = ProcessInfo.processInfo.beginActivity(options: [.userInitiated, .latencyCritical],
                                                         reason: "Almondbread window test")
        #if os(macOS)
        NSApplication.shared.setActivationPolicy(foreground ? .regular : .accessory)
        #endif
    }

    #if os(macOS)
    /// Orders the window behind all others. SwiftUI shows the window after its content view
    /// moved in, which would put it in front of the user's windows: order back again then.
    static func placeInBackground(_ window: NSWindow) {
        guard enabled, !foreground else { return }
        // `--ui-demo-size=WxH`: the whole window at that size (SwiftUI may still apply its
        // default size after this, so again with the later orderings).
        let size = UIDemo.enabled ? UIDemo.windowSize : nil
        func apply(_ w: NSWindow) {
            if let size, w.frame.size != size {
                w.setFrame(NSRect(x: w.frame.minX, y: w.frame.maxY - size.height, width: size.width, height: size.height),
                           display: true)
            }
            w.orderBack(nil)
        }
        apply(window)
        for delay in [0.0, 0.3, 1.0] {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak window] in if let window { apply(window) } }
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
        if foreground {
            NSApplication.shared.activate()
            window.makeKeyAndOrderFront(nil)
        } else {
            window.orderBack(nil)
        }
    }
    #endif
}
