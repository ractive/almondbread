import CoreGraphics

/// Where the touch controls go on a canvas, and the margins the Rust panels keep from them
/// (kb: decisions/controls "Touch", "Round buttons" ▸ Fit). A pure value so the choice can be
/// checked for screens no simulator has (`swift test`, `TouchLayoutTests`).
///
/// - A **phone on its side** (vertical size class compact; every iPhone in landscape, the large
///   models included, whose horizontal class is regular): the round buttons in a row at the
///   bottom trailing corner, the throttle at the trailing edge above them.
/// - A **narrow canvas** (horizontal class compact or narrower than 560 points: iPhone portrait,
///   narrow split views, an iPad portrait canvas next to the control center): the button column
///   at the bottom trailing corner with the throttle on top.
/// - Otherwise (iPad): the buttons in a row at the top trailing corner, the throttle at mid
///   height on the trailing edge.
struct TouchLayout: Equatable {
    /// Below this canvas width (points) the phone's portrait layout is used.
    static let narrowWidth: CGFloat = 560
    /// The throttle lever's width (points).
    static let throttleWidth: CGFloat = 48
    /// Five 44-point round buttons, 10 apart.
    static let buttonRow: CGFloat = 5 * 44 + 4 * 10

    enum Corner: Equatable { case topTrailing, bottomTrailing }

    /// The HUD drops its statistics, the tour card goes on top (iPhone portrait, narrow split views).
    let compactWidth: Bool
    /// A phone on its side.
    let landscapePhone: Bool

    init(canvas: CGSize, horizontalCompact: Bool, verticalCompact: Bool) {
        compactWidth = horizontalCompact || canvas.width < Self.narrowWidth
        landscapePhone = verticalCompact
    }

    /// The buttons stack in a column, with the throttle on top of them.
    var buttonColumn: Bool { compactWidth && !landscapePhone }

    /// The buttons' corner. Before 2026-10-19 this followed `compactWidth` alone, so a large
    /// phone on its side (regular width class) got a top-trailing row while the throttle and the
    /// cockpit left room for a bottom one.
    var buttonCorner: Corner { compactWidth || landscapePhone ? .bottomTrailing : .topTrailing }

    /// The π panel's and the cosmic scale's right margin: the button column, or the throttle at
    /// the trailing edge.
    var trailingInset: Double { compactWidth && !landscapePhone ? 56 : Double(Self.throttleWidth + 12) }

    /// The π panel's bottom margin: above the bottom button row of a phone on its side.
    var bottomInset: Double { landscapePhone ? 44 + 12 : 0 }

    /// Platform controls along the top trailing corner (the iPad's button row): the compass sits
    /// below them, the π panel never moves above them.
    var topInset: Double { buttonCorner == .topTrailing ? 44 + 12 : 0 }

    /// The cockpit (bottom centre) keeps left of the button column, or of the button row of a
    /// phone on its side.
    var cockpitInsetRight: Double {
        landscapePhone ? Double(Self.buttonRow + 12) : compactWidth ? 56 : 0
    }
}
