import AlmondbreadCore
import CoreGraphics
import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Icons inside tour and help texts (kb: decisions/controls "Guided tours"): `{icon:<id>}` shows
/// the control's own icon in the text's colour. The cockpit is drawn by Rust, so its icons are
/// strokes from Rust (core `icon::Glyph`, the same strokes the cockpit draws, FFI
/// `inline_icons()`), drawn here into a small template image; the round buttons and the Mac
/// toolbar are SF Symbols, whose names come from the same Rust table (`symbol(_:)`, used by
/// those buttons too). Tours are checked at parse time (an unknown id is an error with its
/// line); here an unknown id stays as written.
@MainActor
enum InlineIcons {
    static let all: [String: InlineIconInfo] = Dictionary(
        inlineIcons().map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })

    /// The SF Symbol of control `id` (the toolbar and the round buttons show these).
    static func symbol(_ id: String) -> String {
        all[id]?.symbol ?? "questionmark"
    }

    /// A text split at its icons: text pieces and icon ids in order (an icon is `{icon:id}`
    /// with a known id; anything else stays text).
    enum Piece: Equatable {
        case text(String)
        case icon(String)
    }

    nonisolated static func pieces(_ s: String, known: (String) -> Bool) -> [Piece] {
        var out: [Piece] = []
        var rest = Substring(s)
        var text = ""
        while let open = rest.range(of: "{icon:") {
            guard let close = rest[open.upperBound...].firstIndex(of: "}") else { break }
            let id = rest[open.upperBound..<close].trimmingCharacters(in: .whitespaces)
            text += rest[..<open.lowerBound]
            if known(id) {
                if !text.isEmpty { out.append(.text(text)) }
                text = ""
                out.append(.icon(id))
            } else {
                text += rest[open.lowerBound...close]
            }
            rest = rest[rest.index(after: close)...]
        }
        text += rest
        if !text.isEmpty { out.append(.text(text)) }
        return out
    }

    /// Placeholder for an icon while the Markdown is parsed (so emphasis may span an icon).
    private static let mark: Character = "\u{FFFC}"

    /// The inline Markdown `s` as one `Text` with its icons, and what a screen reader should say
    /// for it (nil without icons). `style`: the text style it is shown in (the glyphs' size).
    static func text(_ s: String, style: Font.TextStyle, scale: CGFloat) -> (Text, String?) {
        let pieces = pieces(s, known: { all[$0] != nil })
        let ids = pieces.compactMap { if case .icon(let id) = $0 { id } else { nil } }
        guard !ids.isEmpty else { return (Text(HelpMarkdownView.inline(s)), nil) }
        let marked = pieces.map { p -> String in
            switch p {
            case .text(let t): t
            case .icon: String(mark)
            }
        }.joined()
        let attr = HelpMarkdownView.inline(marked)
        let (size, cap) = metrics(style)
        var out = Text("")
        var spoken = ""
        var k = 0
        var start = attr.startIndex
        var i = attr.startIndex
        func flush(to end: AttributedString.Index) {
            if start < end {
                let part = AttributedString(attr[start..<end])
                out = Text("\(out)\(Text(part))")
                spoken += String(part.characters)
            }
        }
        while i < attr.endIndex {
            if attr.characters[i] == mark, k < ids.count {
                flush(to: i)
                let info = all[ids[k]]!
                out = Text("\(out)\(icon(info, size: size, cap: cap, scale: scale))")
                spoken += info.label
                k += 1
                i = attr.characters.index(after: i)
                start = i
            } else {
                i = attr.characters.index(after: i)
            }
        }
        flush(to: attr.endIndex)
        return (out, spoken)
    }

    /// One icon as text: an SF Symbol (sized by the font like a letter), or the glyph's image,
    /// its box a little larger than the font's size, centred on the capitals' middle.
    private static func icon(_ info: InlineIconInfo, size: CGFloat, cap: CGFloat, scale: CGFloat) -> Text {
        if !info.symbol.isEmpty {
            return Text(Image(systemName: info.symbol))
        }
        let box = (size * 1.3).rounded()
        guard let cg = glyph(info, points: box, scale: scale) else { return Text(info.label) }
        return Text(Image(decorative: cg, scale: scale).renderingMode(.template))
            .baselineOffset((cap - box) / 2)
    }

    private static var images: [String: CGImage] = [:]

    /// The glyph's strokes in white on clear, `points` square at `scale` pixels per point.
    private static func glyph(_ info: InlineIconInfo, points: CGFloat, scale: CGFloat) -> CGImage? {
        let px = max(1, Int((points * scale).rounded()))
        let key = "\(info.id)@\(px)"
        if let im = images[key] { return im }
        guard let ctx = CGContext(data: nil, width: px, height: px, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        // Design points from the centre, y down.
        let u = CGFloat(px) / CGFloat(2 * info.halfExtent)
        ctx.translateBy(x: CGFloat(px) / 2, y: CGFloat(px) / 2)
        ctx.scaleBy(x: u, y: -u)
        ctx.setStrokeColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
        ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        for s in info.strokes {
            switch s {
            case .line(let pts, let width):
                guard pts.count >= 4 else { continue }
                ctx.beginPath()
                ctx.move(to: CGPoint(x: pts[0], y: pts[1]))
                for j in stride(from: 2, to: pts.count - 1, by: 2) {
                    ctx.addLine(to: CGPoint(x: pts[j], y: pts[j + 1]))
                }
                ctx.setLineWidth(width)
                ctx.strokePath()
            case .ring(let x, let y, let r, let width):
                ctx.setLineWidth(width)
                ctx.strokeEllipse(in: CGRect(x: x - r, y: y - r, width: 2 * r, height: 2 * r))
            case .disc(let x, let y, let r):
                ctx.fillEllipse(in: CGRect(x: x - r, y: y - r, width: 2 * r, height: 2 * r))
            }
        }
        let im = ctx.makeImage()
        images[key] = im
        return im
    }

    /// Point size and cap height of a text style as the platform shows it now (Dynamic Type).
    private static func metrics(_ style: Font.TextStyle) -> (CGFloat, CGFloat) {
        #if os(macOS)
        let f = NSFont.preferredFont(forTextStyle: platformStyle(style))
        #else
        let f = UIFont.preferredFont(forTextStyle: platformStyle(style))
        #endif
        return (f.pointSize, f.capHeight)
    }

    #if os(macOS)
    private typealias PlatformStyle = NSFont.TextStyle
    #else
    private typealias PlatformStyle = UIFont.TextStyle
    #endif

    private static func platformStyle(_ s: Font.TextStyle) -> PlatformStyle {
        switch s {
        case .largeTitle: .largeTitle
        case .title: .title1
        case .title2: .title2
        case .title3: .title3
        case .headline: .headline
        case .subheadline: .subheadline
        case .callout: .callout
        case .footnote: .footnote
        case .caption: .caption1
        case .caption2: .caption2
        default: .body
        }
    }
}
