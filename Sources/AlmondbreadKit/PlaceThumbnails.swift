import CoreGraphics
import Foundation
import ImageIO
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Thumbnails of the places catalogue (`places()` in the Rust core; kb: synthesis/places),
/// shipped as package resources: `Places/<id>.heic` (320x200) and `Places/<id>@2x.heic`
/// (640x400), rendered by `almondbread places --render-thumbs Sources/AlmondbreadKit/Resources/Places`.
///
/// The app decodes all of them once, at launch, on a background queue, at the size the
/// control center draws them (`preload`), so the Places grid shows them in its first frame
/// (kb: platform/swiftui-performance).
public enum PlaceThumbnails {
    public static func url(id: String, retina: Bool = true) -> URL? {
        Bundle.module.url(forResource: retina ? "\(id)@2x" : id, withExtension: "heic", subdirectory: "Places")
    }

    /// The widest a grid cell gets, in points: adaptive columns of at least 88 pt in a sidebar
    /// of 290-440 pt are at most ~135 pt wide (two columns just below the switch to three); an
    /// iPhone sheet has three or four columns of ~100-120 pt.
    #if os(macOS)
    static let maxPointWidth: CGFloat = 140
    #else
    static let maxPointWidth: CGFloat = 120
    #endif

    /// One serial queue decodes every thumbnail into a plain bitmap. Lazily decoded HEIC
    /// images were decoded by Core Animation on the main thread while SwiftUI's
    /// prepare-image threads decoded others: in the iOS simulator the HEIF decoder's session
    /// pool deadlocked the main thread when the control center opened (kb:
    /// platform/ios-simulator). Fully decoded bitmaps also scroll without decode hitches.
    private static let queue = DispatchQueue(label: "almondbread.place-thumbnails", qos: .utility)
    private nonisolated(unsafe) static var cache: [String: CGImage] = [:]
    private nonisolated(unsafe) static var pixelWidth = 280
    private nonisolated(unsafe) static var started = false
    private static let lock = NSLock()

    /// Decodes every place's thumbnail at `maxPointWidth` times the largest screen scale, in
    /// the background (about 0.1 s on an M1; ~14 MB at 2x, ~23 MB at 3x). Once per process.
    @MainActor
    static func preload() {
        lock.lock()
        let first = !started
        started = true
        lock.unlock()
        guard first else { return }
        #if os(macOS)
        let scale = NSScreen.screens.map(\.backingScaleFactor).max() ?? 2
        #else
        let scale = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.screen.scale }.max() ?? 3
        #endif
        let width = Int((maxPointWidth * scale).rounded())
        let ids = ExplorerModel.allPlaces.map(\.id)
        let report = UIPerf.enabled
        queue.async {
            lock.lock()
            pixelWidth = width
            lock.unlock()
            let t0 = CFAbsoluteTimeGetCurrent()
            var bytes = 0
            for id in ids {
                if let image = cached(id) ?? decode(id, width: width) {
                    bytes += image.bytesPerRow * image.height
                    store(id, image)
                }
            }
            if report {
                print(String(format: "ui-perf: thumbnails pre-decoded: %d at %d px wide in %.0f ms, %.1f MB",
                             ids.count, width, (CFAbsoluteTimeGetCurrent() - t0) * 1000, Double(bytes) / 1_048_576))
                fflush(stdout)
            }
        }
    }

    static func cached(_ id: String) -> CGImage? {
        lock.lock()
        defer { lock.unlock() }
        return cache[id]
    }

    /// Forgets the decoded bitmaps (a memory warning, kb: platform/ios-device "Memory"); shown
    /// thumbnails stay, others decode again on demand.
    static func purge() {
        lock.lock()
        cache.removeAll()
        lock.unlock()
    }

    private static func store(_ id: String, _ image: CGImage) {
        lock.lock()
        cache[id] = image
        lock.unlock()
    }

    /// The thumbnail, waiting for the launch decode (or decoding it) off the main thread.
    static func decoded(_ id: String) async -> CGImage? {
        if let c = cached(id) { return c }
        return await withCheckedContinuation { (k: CheckedContinuation<CGImage?, Never>) in
            queue.async {
                lock.lock()
                let width = pixelWidth
                lock.unlock()
                let image = cached(id) ?? decode(id, width: width)
                if let image { store(id, image) }
                k.resume(returning: image)
            }
        }
    }

    /// The smallest file at least `width` pixels wide, downsampled to `width` and drawn into an
    /// sRGB bitmap (no lazy decode left for later).
    private static func decode(_ id: String, width: Int) -> CGImage? {
        guard let url = url(id: id, retina: width > 320), let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                  kCGImageSourceCreateThumbnailFromImageAlways: true,
                  kCGImageSourceCreateThumbnailWithTransform: true,
                  kCGImageSourceShouldCacheImmediately: true,
                  kCGImageSourceThumbnailMaxPixelSize: width,
              ] as CFDictionary),
              let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: space, bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
                                    | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        ctx.interpolationQuality = .high
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return ctx.makeImage()
    }
}
