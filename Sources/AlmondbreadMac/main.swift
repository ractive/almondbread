import AlmondbreadKit
import AppKit
import SwiftUI

// `swift run AlmondbreadMac` entry point: the macOS app without an Xcode project. Rendering is
// Rust (kb: platform/render-architecture); the headless tools are the Rust CLI `almondbread`
// (render / verify / bench / regress). Window checks run in the background and quit:
//   --selftest [--selftest-png=out.png]   render the first view through the real view, print "renderer OK"
//   --interaction-test                    frame pacing and latency of pans, zooms and keys

struct AlmondbreadMacApp: App {
    init() {
        // A bare executable (not an .app bundle) starts as a background process with no
        // Dock icon or menu bar; promote it so the window comes to the front.
        let arguments = CommandLine.arguments
        if Bundle.main.bundleIdentifier == nil && !arguments.contains("--selftest")
            && !arguments.contains("--interaction-test") {
            NSApplication.shared.setActivationPolicy(.regular)
            DispatchQueue.main.async { NSApplication.shared.activate(ignoringOtherApps: true) }
        }
    }

    var body: some Scene { AlmondbreadScene() }
}

if let command = ["--render", "--verify", "--bench"].first(where: CommandLine.arguments.contains) {
    print("AlmondbreadMac \(command): moved to the Rust CLI: target/release/almondbread \(command.dropFirst(2)) ... (make rust-cli)")
    exit(2)
}
AlmondbreadMacApp.main()
