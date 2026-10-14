import AlmondbreadKit
import AppKit
import SwiftUI

// `swift run AlmondbreadMac` entry point. Normal launch opens the app; `--render` renders a
// PNG headlessly (for checking output and timing the kernels), e.g.
//   AlmondbreadMac --render out.png --center -0.743643887037151,0.13182590420533 \
//                  --scale 1e-10 --size 1600x1000 [--iterations 5000] [--shading 0.8]
//                  [--outline 0.6] [--palette fire] [--density 0.02] [--ss 2] [--repeat 3]
// `--verify` diffs a GPU render against the CPU Double oracle; `--bench` runs the fixed
// benchmark set (see VerifyCommand.swift, BenchCommand.swift). `--interaction-test` opens the
// window and measures pan/zoom frame pacing; `--selftest` renders one frame and quits.

struct AlmondbreadMacApp: App {
    init() {
        // A bare executable (not an .app bundle) starts as a background process with no
        // Dock icon or menu bar; promote it so the window comes to the front.
        if Bundle.main.bundleIdentifier == nil && !arguments.contains("--selftest")
            && !arguments.contains("--interaction-test") {
            NSApplication.shared.setActivationPolicy(.regular)
            DispatchQueue.main.async { NSApplication.shared.activate(ignoringOtherApps: true) }
        }
    }

    var body: some Scene { AlmondbreadScene() }
}

let arguments = CommandLine.arguments
let commandArguments = Array(arguments.dropFirst())
if arguments.contains("--render") {
    exit(MainActor.assumeIsolated { RenderCommand.run(arguments: commandArguments) })
} else if arguments.contains("--verify") {
    exit(MainActor.assumeIsolated { VerifyCommand.run(arguments: commandArguments) })
} else if arguments.contains("--bench") {
    exit(MainActor.assumeIsolated { BenchCommand.run(arguments: commandArguments) })
} else {
    AlmondbreadMacApp.main()
}
