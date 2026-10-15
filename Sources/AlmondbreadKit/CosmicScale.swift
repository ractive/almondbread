import AlmondbreadCore
import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// The cosmic size scale (drawn by Rust; kb: decisions/controls, "Cosmic size scale"): the
/// screen's physical size for the "screen" anchor and the control center rows.
enum ScreenMetrics {
    #if os(macOS)
    /// Metres per view point on `screen` from the display's physical size (EDID); 0 when the
    /// display does not report a plausible one (Rust then takes the view as 30 cm wide).
    static func metresPerPoint(_ screen: NSScreen?) -> Double {
        guard let screen,
              let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
        else { return 0 }
        let mm = CGDisplayScreenSize(CGDirectDisplayID(number.uint32Value))
        guard mm.width > 0, screen.frame.width > 0 else { return 0 }
        let m = Double(mm.width) / 1000 / Double(screen.frame.width)
        // A point is 0.1 ... 1 mm on any real display; anything else is a bogus EDID.
        return (0.0001...0.001).contains(m) ? m : 0
    }
    #else
    /// iOS reports no physical size: about 160 points per inch on iPhone, 132 on iPad
    /// (unverified rule of thumb; kb: decisions/controls, "Cosmic size scale").
    static func metresPerPoint(_ screen: UIScreen?) -> Double {
        let pointsPerInch: Double = UIDevice.current.userInterfaceIdiom == .pad ? 132 : 160
        return 0.0254 / pointsPerInch
    }
    #endif
}

/// Control center rows: the switch and the anchor.
struct CosmicScaleRows: View {
    @Bindable var model: ExplorerModel
    private let anchors = cosmicAnchors()

    var body: some View {
        Toggle("Cosmic size scale", isOn: $model.cosmicScale)
            .help("How big the view is compared with real things, from the universe to the Planck length (C)")
        if model.cosmicScale {
            Picker("Compare", selection: $model.cosmicAnchor) {
                ForEach(anchors, id: \.key) { Text($0.label).tag($0.key) }
            }
            .help("Screen: the view is your window, how big is the whole set? Set = X: the whole set is X, how big is the view?")
        }
    }
}
