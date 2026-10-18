import AlmondbreadCore
import Foundation
import QuartzCore
#if canImport(Darwin)
import Darwin
#endif
#if os(iOS)
import UIKit
import os
#endif

/// The process's memory footprint (what jetsam counts on iOS: `phys_footprint`) and what the
/// OS still allows (iOS: `os_proc_available_memory()`; 0 where there is no per-app limit, as on
/// macOS and the simulator). Measurement only; the policy is Rust's.
enum ProcessMemory {
    static var footprint: UInt64 {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
        let kr = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
            }
        }
        return kr == KERN_SUCCESS ? info.phys_footprint : 0
    }

    /// `--memory-available-mb=N` (tests): pretend the OS allows N MB more, so the simulator
    /// (which has no per-app limit) runs the device's budget path.
    static let emulated: UInt64? = {
        let prefix = "--memory-available-mb="
        return CommandLine.arguments.first { $0.hasPrefix(prefix) }
            .flatMap { UInt64($0.dropFirst(prefix.count)) }
            .map { $0 << 20 }
    }()

    static var available: UInt64 {
        if let emulated { return emulated }
        #if os(iOS) && targetEnvironment(simulator)
        return 0  // no per-app limit (os_proc_available_memory() says 0)
        #elseif os(iOS)
        // 0 on a device means the app is at or over its limit (os/proc.h), not "no limit":
        // pass 1 byte, so the budget drops to its floors.
        return max(1, UInt64(os_proc_available_memory()))
        #else
        return 0
        #endif
    }

    /// "footprint 412 MB, available 1630 MB" (selftest and logs).
    static func describe(_ explorer: Explorer?) -> String {
        let mb = { (b: UInt64) in String(format: "%.0f MB", Double(b) / 1_048_576) }
        var s = "memory: footprint \(mb(footprint)), available " + (available > 0 ? mb(available) : "no limit")
        if let m = explorer?.memoryInfo() {
            s += ", renderer \(mb(m.rendererBytes)) (GPU \(mb(m.gpuBytes)), reference \(mb(m.referenceBytes)), "
                + "tables \(mb(m.tablesBytes))), drawable \(m.width)x\(m.height)"
            if m.ceilingBytes > 0 {
                s += ", budget ceiling \(mb(m.ceilingBytes)): at most \(m.maxTexels) texels, \(m.maxIterations) iterations"
            }
        }
        s += ", thermal \(ProcessInfo.processInfo.thermalState.rawValue)"
        return s
    }
}

#if os(iOS)
/// Forwards the device's signals to the Rust renderer of one view (kb: platform/ios-device):
/// the memory still available (at launch, every 2 s while the scene is active, and on memory
/// warnings, which also purge the thumbnails), the thermal state (its frame-rate ceiling goes to
/// the display link) and the scene's background / foreground transitions (no GPU work in the
/// background). Rust decides what each signal means.
@MainActor
final class DeviceSignals {
    private weak var host: FractalUIView?
    private nonisolated(unsafe) var observers: [NSObjectProtocol] = []
    private nonisolated(unsafe) var sceneObservers: [NSObjectProtocol] = []
    private nonisolated(unsafe) var timer: Timer?
    private weak var scene: UIScene?
    /// The display link's frame-rate ceiling from the thermal state (0: none).
    private(set) var maxFPS: Int = 0
    private(set) var active = true

    init(host: FractalUIView) {
        self.host = host
        let nc = NotificationCenter.default
        observers.append(nc.addObserver(forName: UIApplication.didReceiveMemoryWarningNotification, object: nil,
                                        queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.memoryWarning() }
        })
        observers.append(nc.addObserver(forName: ProcessInfo.thermalStateDidChangeNotification, object: nil,
                                        queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.thermalChanged() }
        })
        thermalChanged()
        sendMemory(warning: false)
        startTimer()
    }

    deinit {
        timer?.invalidate()
        (observers + sceneObservers).forEach(NotificationCenter.default.removeObserver)
    }

    /// The view moved to a window: follow that window's scene into and out of the background.
    func attach(scene newScene: UIScene?) {
        guard newScene !== scene else { return }
        scene = newScene
        sceneObservers.forEach(NotificationCenter.default.removeObserver)
        sceneObservers = []
        guard let newScene else { return }
        let nc = NotificationCenter.default
        sceneObservers.append(nc.addObserver(forName: UIScene.didEnterBackgroundNotification, object: newScene,
                                             queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.setActive(false) }
        })
        sceneObservers.append(nc.addObserver(forName: UIScene.willEnterForegroundNotification, object: newScene,
                                             queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.setActive(true) }
        })
    }

    private func setActive(_ on: Bool) {
        guard on != active, let host else { return }
        active = on
        NSLog("Almondbread: scene %@ (GPU work %@)", on ? "foreground" : "background", on ? "resumed" : "stopped")
        host.setRendering(active: on)
        if on {
            sendMemory(warning: false)
            thermalChanged()
            startTimer()
        } else {
            timer?.invalidate()
            timer = nil
        }
    }

    private func startTimer() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.sendMemory(warning: false) }
        }
    }

    private func memoryWarning() {
        PlaceThumbnails.purge()
        sendMemory(warning: true)
        NSLog("Almondbread: memory warning; %@", ProcessMemory.describe(host?.base.explorer))
    }

    private func sendMemory(warning: Bool) {
        guard let host, let explorer = host.base.explorer else { return }
        if explorer.setMemoryAvailable(available: ProcessMemory.available, warning: warning) {
            host.requestFrames()
        }
    }

    private func thermalChanged() {
        guard let host, let explorer = host.base.explorer else { return }
        let state = ProcessInfo.processInfo.thermalState
        maxFPS = Int(explorer.setThermalState(state: UInt8(clamping: state.rawValue)))
        if state != .nominal {
            NSLog("Almondbread: thermal state %d, frame rate ceiling %d", state.rawValue, maxFPS)
        }
        host.applyFrameRate()
        host.requestFrames()
    }

    /// The display link's range on `screen` under the thermal ceiling: ProMotion screens ask
    /// for 120 Hz (with `CADisableMinimumFrameDurationOnPhone` on iPhones), 60 Hz screens get 60.
    func frameRateRange(screenMax: Int) -> CAFrameRateRange {
        let fps = Float(maxFPS > 0 ? min(maxFPS, screenMax) : screenMax)
        return CAFrameRateRange(minimum: min(60, fps), maximum: fps, preferred: fps)
    }
}
#endif
