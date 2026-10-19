// App Store screenshot composer (draft): a dark canvas of an exact pixel size, a short
// caption at the top, the screenshot scaled to fit below it with rounded corners.
// Usage: swift scripts/appstore-caption.swift in.png out.png WIDTH HEIGHT "Caption" ["Subline"]
// An empty caption puts the screenshot alone, scaled to fill the canvas (cropped to its centre).
import AppKit
import CoreText

let a = CommandLine.arguments
guard a.count >= 6, let w = Int(a[3]), let h = Int(a[4]),
      let src = NSImage(contentsOfFile: a[1]),
      let img = src.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
    FileHandle.standardError.write("usage: appstore-caption.swift in.png out.png W H caption [subline]\n".data(using: .utf8)!)
    exit(2)
}
let caption = a[5]
let subline = a.count > 6 ? a[6] : ""
let W = CGFloat(w), H = CGFloat(h)
let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                    space: CGColorSpace(name: CGColorSpace.sRGB)!,
                    bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
ctx.interpolationQuality = .high
let iw = CGFloat(img.width), ih = CGFloat(img.height)

if caption.isEmpty {
    // Nearly the same shape: fill (crop a few pixels); otherwise fit on black, so nothing
    // (toolbar, cockpit) is cut off.
    let close = abs(iw / ih - W / H) / (W / H) < 0.03
    ctx.setFillColor(CGColor(gray: 0, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: W, height: H))
    let s = close ? max(W / iw, H / ih) : min(W / iw, H / ih)
    ctx.draw(img, in: CGRect(x: (W - iw * s) / 2, y: (H - ih * s) / 2, width: iw * s, height: ih * s))
} else {
    // Background: a deep blue-black gradient.
    let grad = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB)!,
                          colors: [CGColor(srgbRed: 0.03, green: 0.04, blue: 0.09, alpha: 1),
                                   CGColor(srgbRed: 0.08, green: 0.07, blue: 0.16, alpha: 1)] as CFArray,
                          locations: [0, 1])!
    ctx.drawLinearGradient(grad, start: CGPoint(x: 0, y: 0), end: CGPoint(x: 0, y: H), options: [])
    let short = min(W, H)
    let titleSize = short * (W > H ? 0.052 : 0.062)
    let subSize = titleSize * 0.5
    let margin = short * 0.05
    let band = titleSize * 1.5 + (subline.isEmpty ? 0 : subSize * 1.6) + margin * 1.6

    func line(_ text: String, size: CGFloat, weight: NSFont.Weight, color: NSColor, top: CGFloat) -> CGFloat {
        let font = NSFont.systemFont(ofSize: size, weight: weight)
        let para = NSMutableParagraphStyle()
        para.alignment = .center
        let attr = NSAttributedString(string: text, attributes: [.font: font, .foregroundColor: color, .paragraphStyle: para])
        let fs = CTFramesetterCreateWithAttributedString(attr)
        let box = CGSize(width: W - 2 * margin, height: .greatestFiniteMagnitude)
        let fit = CTFramesetterSuggestFrameSizeWithConstraints(fs, CFRange(), nil, box, nil)
        let rect = CGRect(x: margin, y: H - top - fit.height, width: W - 2 * margin, height: fit.height)
        let frame = CTFramesetterCreateFrame(fs, CFRange(), CGPath(rect: rect, transform: nil), nil)
        CTFrameDraw(frame, ctx)
        return top + fit.height
    }
    var y = line(caption, size: titleSize, weight: .bold, color: .white, top: margin * 1.1)
    if !subline.isEmpty {
        y = line(subline, size: subSize, weight: .regular, color: NSColor(white: 0.78, alpha: 1), top: y + subSize * 0.4)
    }
    let top = max(band, y + margin * 0.7)
    // The screenshot: fit the area below the caption.
    let areaW = W - 2 * margin * 0.6, areaH = H - top - margin * 0.6
    let s = min(areaW / iw, areaH / ih)
    let dw = iw * s, dh = ih * s
    let rect = CGRect(x: (W - dw) / 2, y: margin * 0.6 + (areaH - dh) / 2, width: dw, height: dh)
    let radius = short * 0.025
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -short * 0.006), blur: short * 0.03,
                  color: CGColor(gray: 0, alpha: 0.7))
    ctx.addPath(CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil))
    ctx.setFillColor(CGColor(gray: 0, alpha: 1))
    ctx.fillPath()
    ctx.restoreGState()
    ctx.saveGState()
    ctx.addPath(CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil))
    ctx.clip()
    ctx.draw(img, in: rect)
    ctx.restoreGState()
}
let out = NSBitmapImageRep(cgImage: ctx.makeImage()!)
try out.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: a[2]))
