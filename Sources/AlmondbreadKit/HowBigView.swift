import AlmondbreadCore
import SwiftUI

/// "How big is big?": the zoom compared with googol, googolplex, Graham's number and TREE(3)
/// by the *kind* of size, as a gear train with one dial per Knuth up-arrow level, after Arthur
/// Ganson's "Machine with Concrete" (twelve 1:50 worm reductions whose last gear is set in
/// concrete). All maths is Rust (`zoomDials`, `bigNumberMarkers`, `bigNumberFacts`; kb:
/// synthesis/how-big-is-big). An explanatory panel, not an overlay: a window on the Mac (it
/// stays open and its dials follow the zoom live), a sheet from the control center on iOS.
enum HowBig {
    static let windowID = "how-big"
    /// Dials drawn: ↑ … ↑↑↑↑; the concrete block shows ↑⁵ and ↑⁶.
    static let levels: UInt32 = 6
    static let markers: [BigNumberInfo] = bigNumberMarkers(levels: levels)
    static let limitLog10: Double = maxZoomLog10()

    /// One colour per marker, the same on every dial and in the legends.
    static func color(_ id: String) -> Color {
        switch id {
        case "googol": Color(red: 0.42, green: 0.78, blue: 0.95)
        case "zoom-limit": Color(red: 0.95, green: 0.82, blue: 0.40)
        case "googolplex": Color(red: 0.55, green: 0.86, blue: 0.55)
        case "skewes-1": Color(red: 0.80, green: 0.62, blue: 0.95)
        case "googolplexian": Color(red: 0.40, green: 0.85, blue: 0.75)
        case "skewes-2": Color(red: 0.95, green: 0.55, blue: 0.80)
        case "mega": Color(red: 0.98, green: 0.65, blue: 0.35)
        case "tritri": Color(red: 0.90, green: 0.90, blue: 0.55)
        case "graham-g1": Color(red: 0.98, green: 0.45, blue: 0.40)
        default: .white
        }
    }
}

/// The model the panel follows: the window that opened it.
@MainActor @Observable
final class HowBigPresenter {
    static let shared = HowBigPresenter()
    weak var model: ExplorerModel?
}

/// The control-center entry point (and the menu's, on the Mac).
struct HowBigButton: View {
    let model: ExplorerModel?
    var title = "How big is big?"
    #if os(macOS)
    @Environment(\.openWindow) private var openWindow
    #else
    @State private var shown = false
    #endif

    var body: some View {
        Button {
            HowBigPresenter.shared.model = model
            #if os(macOS)
            openWindow(id: HowBig.windowID)
            #else
            shown = true
            #endif
        } label: {
            Label(title, systemImage: "gearshape.2")
        }
        .help("Your zoom next to a googol, a googolplex and Graham's number: one dial per up-arrow")
        .disabled(model == nil)
        #if os(iOS)
        .sheet(isPresented: $shown) {
            NavigationStack {
                ScrollView {
                    if let model { HowBigPanel(model: model).padding(.bottom, 24) }
                }
                .background(HowBigStyle.background)
                .navigationTitle("How big is big?")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) { Button("Done") { shown = false } }
                }
            }
            .presentationDetents([.medium, .large])
            .presentationBackgroundInteraction(.enabled(upThrough: .medium))
            .preferredColorScheme(.dark)
        }
        #endif
    }
}

#if os(macOS)
/// The Mac window's content.
struct HowBigWindow: View {
    var body: some View {
        ScrollView {
            if let model = HowBigPresenter.shared.model {
                HowBigPanel(model: model)
            } else {
                Text("Open it from a fractal window (View ▸ How Big Is Big?).")
                    .foregroundStyle(.secondary)
                    .padding(40)
            }
        }
        .background(HowBigStyle.background)
        .frame(minWidth: 360, idealWidth: 760, minHeight: 420, idealHeight: 860)
    }
}
#endif

enum HowBigStyle {
    static let background = Color(red: 0.07, green: 0.065, blue: 0.06)
    static let face = Color(red: 0.10, green: 0.095, blue: 0.085)
    static let brassLight = Color(red: 0.89, green: 0.74, blue: 0.45)
    static let brass = Color(red: 0.74, green: 0.58, blue: 0.31)
    static let brassDark = Color(red: 0.42, green: 0.31, blue: 0.15)
    static let ivory = Color(red: 0.93, green: 0.89, blue: 0.80)
    static let needle = Color(red: 0.93, green: 0.40, blue: 0.24)
    static let concrete = Color(red: 0.45, green: 0.45, blue: 0.43)
    static let card = Color(red: 0.11, green: 0.105, blue: 0.095)
}

/// One dial of the train: arrows, title, scale, what one unit means.
struct ArrowDial {
    let level: Int
    let title: String
    let unit: String
    /// The scale's maximum (the arc runs 0 … max).
    let max: Double
    let caption: String

    var arrows: String { String(repeating: "↑", count: level) }

    static let all: [ArrowDial] = [
        ArrowDial(level: 1, title: "Digits", unit: "digits", max: 100,
                  caption: "10↑d = 10^d: how many digits the zoom has. One turn of this dial is a googol (100 digits). Our deepest zoom turns it 30 times; a googolplex would turn it 10⁹⁸ times."),
        ArrowDial(level: 2, title: "Tower of tens", unit: "tens high", max: 5,
                  caption: "10↑↑h = 10^10^…^10, h tens high. A googol is a tower 2.30 high, a googolplex 3.30, our deepest zoom 2.54. It reads 2 at a zoom of 10¹⁰ (14 s in); the hour of zooming after that moves it by about half a unit."),
        ArrowDial(level: 3, title: "Towers of towers", unit: "towers", max: 3,
                  caption: "10↑↑↑h: a tower of tens whose height is itself a tower. Everything we can reach sits between 1.36 (googol) and 1.41. It barely twitches."),
        ArrowDial(level: 4, title: "Four arrows", unit: "", max: 3,
                  caption: "10↑↑↑↑h. Graham's first step g₁ = 3↑↑↑↑3 finally gets past 2 here; our deepest zoom reads 1.15."),
    ]
}

struct HowBigPanel: View {
    let model: ExplorerModel

    private var zoomLog10: Double { max(0, model.stats?.shown.zoomLog10 ?? 0) }

    var body: some View {
        let dials = zoomDials(zoomLog10: zoomLog10, levels: HowBig.levels)
        let facts = bigNumberFacts(zoomLog10: zoomLog10, zoomSpeedLn: model.stats?.zoomSpeed ?? 1.6)
        HowBigContent(zoomLog10: zoomLog10, dials: dials, facts: facts)
    }
}

/// The panel's content for a given zoom (no scroll view: also rendered offscreen by the
/// ui-demo screenshots).
struct HowBigContent: View {
    let zoomLog10: Double
    let dials: [Double]
    let facts: BigNumberFactsInfo

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            header
            GearTrain(dials: dials)
                .frame(height: 150)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 300), spacing: 14, alignment: .top)], spacing: 14) {
                ForEach(ArrowDial.all, id: \.level) { d in
                    DialCard(dial: d, value: dials[d.level - 1])
                }
                ConcreteCard(dials: dials)
            }
            FactsCard(facts: facts)
            BeyondCard()
            smallPrint
        }
        .padding(18)
        .frame(maxWidth: 900)
        .frame(maxWidth: .infinity)
        .foregroundStyle(HowBigStyle.ivory)
        .monospacedDigit()
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("How big is big?")
                .font(.system(.largeTitle, design: .serif).weight(.semibold))
                .foregroundStyle(HowBigStyle.brassLight)
            Text("Your zoom is \(HowBigFormat.zoom(zoomLog10)): a \(HowBigFormat.int(floor(zoomLog10) + 1))-digit number. Next to a googolplex or Graham's number no comparison of values makes sense, so this compares the *kind* of size: one dial per Knuth up-arrow, each turning unimaginably slower than the one before, like Arthur Ganson's gear train whose last wheel is set in concrete.")
                .font(.callout)
                .foregroundStyle(HowBigStyle.ivory.opacity(0.85))
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var smallPrint: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Small print")
                .font(.caption.weight(.semibold))
            Text("Dial k reads h with 10↑ᵏh = x (Knuth's up-arrows, 1976). Fractional heights use the linear approximation of the super-logarithm, taken to every arrow level: h(x) = x − 1 for 0 < x ≤ 1 and h(x) = 1 + h(dial k−1 of x) above. So 10↑↑2 = 10¹⁰ reads exactly 2, every dial reads 0 at zoom 1× and 1 at 10× (10 = 10↑1 = 10↑↑1), and between 1× and 10× every dial reads log₁₀ of the zoom. Other conventions give other decimals, never other whole numbers. Positions of Skewes' numbers use their published upper bounds, mega the middle of its known range, 3↑↑↑3 and g₁ our own tower arithmetic (good to about 0.001). Bits: a reference orbit needs about log₂ of the zoom (3.32 bits per digit; almondbread stops at 2⁻¹⁰⁰⁰⁰ ≈ 10⁻³⁰¹⁰ per pixel). Atoms: about 10⁸⁰ in the observable universe (an order of magnitude). Sources: Wikipedia (Knuth's up-arrow notation, Tetration, Graham's number, Steinhaus–Moser notation, Kruskal's tree theorem, Fast-growing hierarchy, Skewes's number, Googolplex, Names of large numbers, Observable universe), Make: \"Arthur Ganson's Machine with Concrete\" (2012).")
                .font(.caption2)
                .foregroundStyle(HowBigStyle.ivory.opacity(0.6))
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

// MARK: Gear train

/// The train across the top: four brass gears, each smaller and slower, meshing into a block
/// of concrete. Each gear's rotation is its dial's reading (one turn per scale).
struct GearTrain: View {
    let dials: [Double]

    var body: some View {
        Canvas { ctx, size in
            let n = 4
            let concreteW = min(110, size.width * 0.2)
            let avail = size.width - concreteW - 8
            let radii: [CGFloat] = [1, 0.8, 0.64, 0.52].map { $0 }
            let total = radii.reduce(0, +) * 2 - CGFloat(n - 1) * 0.12
            let unit = min(avail / total, size.height * 0.42)
            var x: CGFloat = 0
            let cy = size.height * 0.45
            for i in 0..<n {
                let r = radii[i] * unit
                x += (i == 0 ? r : r - 0.12 * unit)
                let d = ArrowDial.all[i]
                let turns = dials[i] / d.max * (i % 2 == 0 ? 1 : -1)
                drawGear(ctx, center: CGPoint(x: x, y: cy), radius: r, teeth: 10 + 3 * (3 - i) + 6,
                         angle: .radians(turns * 2 * .pi))
                ctx.draw(Text(d.arrows).font(.system(size: max(10, r * 0.34), weight: .bold, design: .serif))
                            .foregroundColor(HowBigStyle.brassDark),
                         at: CGPoint(x: x, y: cy))
                ctx.draw(Text(HowBigFormat.dial(dials[i], level: d.level)).font(.system(size: 11, weight: .semibold).monospacedDigit())
                            .foregroundColor(HowBigStyle.ivory),
                         at: CGPoint(x: x, y: min(size.height - 7, cy + r + 12)))
                x += r
            }
            // The concrete block with the last gear in it.
            let block = CGRect(x: min(size.width - concreteW, x + unit * 0.3), y: cy - unit * 0.55, width: concreteW, height: unit * 1.1)
            let gx = block.minX + 4
            drawGear(ctx, center: CGPoint(x: gx, y: cy), radius: unit * 0.4, teeth: 12, angle: .zero)
            ctx.fill(Path(roundedRect: block, cornerRadius: 4), with: .color(HowBigStyle.concrete))
            // Aggregate speckles.
            var rng = SplitMix(seed: 7)
            for _ in 0..<70 {
                let p = CGPoint(x: block.minX + rng.next() * block.width, y: block.minY + rng.next() * block.height)
                let s = 1 + rng.next() * 2.5
                ctx.fill(Path(ellipseIn: CGRect(x: p.x, y: p.y, width: s, height: s)),
                         with: .color(.black.opacity(0.18 + 0.2 * rng.next())))
            }
            ctx.draw(Text("↑⁵ …").font(.system(size: 13, weight: .bold, design: .serif)).foregroundColor(.black.opacity(0.65)),
                     at: CGPoint(x: block.midX, y: block.midY - 8))
            ctx.draw(Text("concrete").font(.system(size: 10, weight: .semibold)).foregroundColor(.black.opacity(0.55)),
                     at: CGPoint(x: block.midX, y: block.midY + 9))
            ctx.draw(Text(HowBigFormat.dial(dials[4], level: 5)).font(.system(size: 11, weight: .semibold).monospacedDigit())
                        .foregroundColor(HowBigStyle.ivory),
                     at: CGPoint(x: block.midX, y: min(size.height - 7, cy + unit * 0.55 + 12)))
        }
        .accessibilityLabel("Gear train: one gear per arrow level")
        .accessibilityValue(ArrowDial.all.map { "\($0.arrows) \(HowBigFormat.dial(dials[$0.level - 1], level: $0.level))" }.joined(separator: ", "))
    }

    private func drawGear(_ ctx: GraphicsContext, center c: CGPoint, radius r: CGFloat, teeth: Int, angle: Angle) {
        var path = Path()
        let inner = r * 0.86
        let steps = teeth * 4
        for s in 0...steps {
            let a = angle.radians + Double(s) / Double(steps) * 2 * .pi
            let q = s % 4
            let rr = (q == 1 || q == 2) ? r : inner
            let p = CGPoint(x: c.x + rr * cos(a), y: c.y + rr * sin(a))
            if s == 0 { path.move(to: p) } else { path.addLine(to: p) }
        }
        path.closeSubpath()
        let g = Gradient(colors: [HowBigStyle.brassLight, HowBigStyle.brass, HowBigStyle.brassDark])
        ctx.fill(path, with: .linearGradient(g, startPoint: CGPoint(x: c.x - r, y: c.y - r), endPoint: CGPoint(x: c.x + r, y: c.y + r)))
        ctx.stroke(path, with: .color(.black.opacity(0.5)), lineWidth: 0.8)
        // Spokes cut out of the web.
        for k in 0..<5 {
            let a = angle.radians + Double(k) / 5 * 2 * .pi
            var hole = Path()
            hole.addArc(center: c, radius: r * 0.66, startAngle: .radians(a + 0.18), endAngle: .radians(a + 2 * .pi / 5 - 0.18), clockwise: false)
            hole.addArc(center: c, radius: r * 0.36, startAngle: .radians(a + 2 * .pi / 5 - 0.3), endAngle: .radians(a + 0.3), clockwise: true)
            hole.closeSubpath()
            ctx.fill(hole, with: .color(HowBigStyle.background.opacity(0.9)))
        }
        ctx.fill(Path(ellipseIn: CGRect(x: c.x - r * 0.12, y: c.y - r * 0.12, width: r * 0.24, height: r * 0.24)),
                 with: .color(HowBigStyle.brassDark))
    }
}

/// Small deterministic random numbers for the concrete's texture.
struct SplitMix {
    var state: UInt64
    init(seed: UInt64) { state = seed }
    mutating func next() -> CGFloat {
        state &+= 0x9E37_79B9_7F4A_7C15
        var z = state
        z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
        z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
        z ^= z >> 31
        return CGFloat(z >> 11) / CGFloat(1 << 53)
    }
}

// MARK: Dials

struct DialCard: View {
    let dial: ArrowDial
    let value: Double

    /// Markers on this dial: those with a finite reading (off the scale ones are listed).
    private var markers: [BigNumberInfo] {
        HowBig.markers.filter { $0.dials.count >= dial.level && $0.dials[dial.level - 1].isFinite }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                Text(dial.arrows)
                    .font(.system(.title2, design: .serif).weight(.bold))
                    .foregroundStyle(HowBigStyle.brassLight)
                Text(dial.title)
                    .font(.system(.headline, design: .serif))
                Spacer()
                Text(HowBigFormat.dial(value, level: dial.level))
                    .font(.headline.monospacedDigit())
                    .foregroundStyle(HowBigStyle.needle)
            }
            DialFace(dial: dial, value: value, markers: HowBig.markers)
                .frame(maxWidth: .infinity)
                .frame(height: 230)
            Text(dial.caption)
                .font(.caption)
                .foregroundStyle(HowBigStyle.ivory.opacity(0.8))
                .fixedSize(horizontal: false, vertical: true)
            legend
        }
        .padding(14)
        .background(HowBigStyle.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(HowBigStyle.brassDark.opacity(0.6)))
    }

    @ViewBuilder
    private var legend: some View {
        let rows = HowBig.markers.filter { $0.dials.count >= dial.level }
        VStack(alignment: .leading, spacing: 3) {
            ForEach(rows, id: \.id) { m in
                let v = m.dials[dial.level - 1]
                HStack(spacing: 6) {
                    Circle().fill(HowBig.color(m.id)).frame(width: 7, height: 7)
                    Text(m.name).lineLimit(1)
                    Spacer(minLength: 4)
                    Text(legendValue(v))
                        .foregroundStyle(HowBigStyle.ivory.opacity(v.isFinite && v <= dial.max ? 0.9 : 0.5))
                }
                .font(.caption2)
            }
        }
    }

    private func legendValue(_ v: Double) -> String {
        if !v.isFinite { return "beyond this dial" }
        if dial.level == 1 {
            return v <= 1e6 ? "\(HowBigFormat.int(v)) digits" : "\(HowBigFormat.sci(v)) digits"
        }
        let s = HowBigFormat.dial(v, level: dial.level)
        return v > dial.max ? "\(s), off the scale" : s
    }
}

/// A clock-like dial: brass toothed bezel that turns with the reading, ivory scale over 270°,
/// marker pips, the needle, and the reading in the middle.
struct DialFace: View {
    let dial: ArrowDial
    let value: Double
    let markers: [BigNumberInfo]

    var body: some View {
        Canvas { ctx, size in
            let r = min(size.width, size.height) / 2 - 4
            let c = CGPoint(x: size.width / 2, y: size.height / 2)
            let start = 135.0, sweep = 270.0
            // The digits dial turns round and round (one turn per googol).
            let wraps = dial.level == 1
            let (startA, sweepA) = wraps ? (-90.0, 360.0) : (start, sweep)
            func angle(_ v: Double) -> Double {
                let f = wraps ? v.truncatingRemainder(dividingBy: dial.max) / dial.max : min(max(v / dial.max, 0), 1.02)
                return (startA + f * sweepA) * .pi / 180
            }
            func pt(_ a: Double, _ rr: CGFloat) -> CGPoint { CGPoint(x: c.x + rr * cos(a), y: c.y + rr * sin(a)) }

            // Bezel: toothed brass ring, turned by the reading.
            let turn = value.isFinite ? value / dial.max * 2 * .pi : 0
            var teeth = Path()
            let nTeeth = 48
            for s in 0...(nTeeth * 4) {
                let a = turn + Double(s) / Double(nTeeth * 4) * 2 * .pi
                let q = s % 4
                let rr = (q == 1 || q == 2) ? r : r * 0.95
                if s == 0 { teeth.move(to: pt(a, rr)) } else { teeth.addLine(to: pt(a, rr)) }
            }
            teeth.closeSubpath()
            let brass = Gradient(colors: [HowBigStyle.brassLight, HowBigStyle.brass, HowBigStyle.brassDark, HowBigStyle.brass])
            ctx.fill(teeth, with: .conicGradient(brass, center: c, angle: .radians(turn)))
            let faceR = r * 0.88
            ctx.fill(Path(ellipseIn: CGRect(x: c.x - faceR, y: c.y - faceR, width: 2 * faceR, height: 2 * faceR)),
                     with: .radialGradient(Gradient(colors: [HowBigStyle.face.opacity(0.9), Color.black]),
                                           center: c, startRadius: 0, endRadius: faceR))
            ctx.stroke(Path(ellipseIn: CGRect(x: c.x - faceR, y: c.y - faceR, width: 2 * faceR, height: 2 * faceR)),
                       with: .color(HowBigStyle.brassDark), lineWidth: 1.5)

            // Scale: ticks and numerals.
            let major: Double = dial.level == 1 ? 20 : 1
            let minorSteps = dial.level == 1 ? 5 : 10
            var v = 0.0
            while v <= dial.max + 1e-9 {
                if !(wraps && v >= dial.max - 1e-9) {
                    let a = angle(v)
                    var tick = Path()
                    tick.move(to: pt(a, faceR * 0.97))
                    tick.addLine(to: pt(a, faceR * 0.84))
                    ctx.stroke(tick, with: .color(HowBigStyle.ivory), lineWidth: 1.6)
                    let label = dial.level == 1 && v == 0 ? "0" : HowBigFormat.int(v)
                    ctx.draw(Text(label).font(.system(size: max(9, faceR * 0.11), weight: .medium, design: .serif))
                                .foregroundColor(HowBigStyle.ivory),
                             at: pt(a, faceR * 0.72))
                    for m in 1..<minorSteps where v + Double(m) * major / Double(minorSteps) <= dial.max {
                        let am = angle(v + Double(m) * major / Double(minorSteps))
                        var t = Path()
                        t.move(to: pt(am, faceR * 0.97))
                        t.addLine(to: pt(am, faceR * 0.91))
                        ctx.stroke(t, with: .color(HowBigStyle.ivory.opacity(0.55)), lineWidth: 0.8)
                    }
                }
                v += major
            }

            // Marker pips on the rim (the legend says which is which).
            for m in markers where m.dials.count >= dial.level {
                let mv = m.dials[dial.level - 1]
                guard mv.isFinite, wraps ? mv <= dial.max : mv <= dial.max else { continue }
                let a = angle(mv)
                let p = pt(a, faceR * 1.0)
                var tri = Path()
                tri.move(to: p)
                tri.addLine(to: pt(a - 0.05, faceR * 1.1))
                tri.addLine(to: pt(a + 0.05, faceR * 1.1))
                tri.closeSubpath()
                ctx.fill(tri, with: .color(HowBig.color(m.id)))
                ctx.stroke(tri, with: .color(.black.opacity(0.6)), lineWidth: 0.6)
            }

            // Needle.
            if value.isFinite {
                let a = angle(value)
                var needle = Path()
                needle.move(to: pt(a + .pi, faceR * 0.16))
                needle.addLine(to: pt(a + .pi / 2, faceR * 0.035))
                needle.addLine(to: pt(a, faceR * 0.9))
                needle.addLine(to: pt(a - .pi / 2, faceR * 0.035))
                needle.closeSubpath()
                var shadow = ctx
                shadow.addFilter(.shadow(color: .black.opacity(0.6), radius: 3, x: 1, y: 2))
                shadow.fill(needle, with: .color(HowBigStyle.needle))
            }
            ctx.fill(Path(ellipseIn: CGRect(x: c.x - faceR * 0.06, y: c.y - faceR * 0.06, width: faceR * 0.12, height: faceR * 0.12)),
                     with: .color(HowBigStyle.brassLight))

            // Reading.
            let reading = dial.level == 1 ? "\(HowBigFormat.int(floor(value) + 1)) digits" : HowBigFormat.dial(value, level: dial.level)
            ctx.draw(Text(reading).font(.system(size: max(11, faceR * 0.13), weight: .semibold).monospacedDigit())
                        .foregroundColor(HowBigStyle.needle),
                     at: CGPoint(x: c.x, y: c.y + faceR * 0.30))
            let sub = dial.level == 1 ? "googol turns: \(HowBigFormat.int(floor(value / 100)))" : "\(dial.arrows) \(dial.unit)"
            ctx.draw(Text(sub).font(.system(size: max(8, faceR * 0.08), design: .serif))
                        .foregroundColor(HowBigStyle.ivory.opacity(0.6)),
                     at: CGPoint(x: c.x, y: c.y + faceR * 0.44))
        }
        .accessibilityLabel("\(dial.title) dial")
        .accessibilityValue(HowBigFormat.dial(value, level: dial.level))
    }
}

/// ↑⁵ and beyond: still readable, but nothing we can name moves them.
struct ConcreteCard: View {
    let dials: [Double]

    private func g1(_ i: Int) -> String {
        guard let d = HowBig.markers.first(where: { $0.id == "graham-g1" })?.dials, d.count > i else { return "?" }
        return HowBigFormat.dial(d[i], level: i + 1)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                Text("↑↑↑↑↑+")
                    .font(.system(.title3, design: .serif).weight(.bold))
                Text("Set in concrete")
                    .font(.system(.headline, design: .serif))
            }
            .foregroundStyle(Color.black.opacity(0.78))
            Text("Five arrows reads \(HowBigFormat.dial(dials[4], level: 5)), six arrows \(HowBigFormat.dial(dials[5], level: 6)); each further dial creeps closer to 1. A googol, our deepest zoom and a googolplex all read 1.05–1.07 on the fifth dial. Like Ganson's last gear, it is still connected; it just never moves.")
                .font(.callout)
                .foregroundStyle(Color.black.opacity(0.8))
                .fixedSize(horizontal: false, vertical: true)
            Text("Graham's g₁ = 3↑↑↑↑3 reads \(g1(3)) on the four-arrow dial and \(g1(4)) on the fifth. The rest of Graham's number does not fit on any of them (below).")
                .font(.caption)
                .foregroundStyle(Color.black.opacity(0.7))
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(14)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background {
            ConcreteTexture()
                .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
    }
}

struct ConcreteTexture: View {
    var body: some View {
        Canvas { ctx, size in
            ctx.fill(Path(CGRect(origin: .zero, size: size)), with: .color(HowBigStyle.concrete))
            var rng = SplitMix(seed: 42)
            let n = Int(size.width * size.height / 90)
            for _ in 0..<n {
                let s = 0.8 + rng.next() * 3
                let p = CGPoint(x: rng.next() * size.width, y: rng.next() * size.height)
                let light = rng.next() > 0.5
                ctx.fill(Path(ellipseIn: CGRect(x: p.x, y: p.y, width: s, height: s * (0.6 + rng.next() * 0.6))),
                         with: .color(light ? Color.white.opacity(0.08 + rng.next() * 0.1) : Color.black.opacity(0.1 + rng.next() * 0.15)))
            }
        }
    }
}

// MARK: Facts and the numbers beyond the dials

struct FactsCard: View {
    let facts: BigNumberFactsInfo

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Along the way")
                .font(.system(.title3, design: .serif).weight(.semibold))
                .foregroundStyle(HowBigStyle.brassLight)
            fact("timer", "At \(HowBigFormat.speed(facts.speedFactor)) you pass a googol after \(HowBigFormat.duration(facts.secondsToGoogol)) and reach our limit, \(HowBigFormat.pow10(HowBig.limitLog10)), after \(HowBigFormat.duration(facts.secondsToLimit)).")
            fact("hourglass", "A googolplex zoom at that speed: about \(HowBigFormat.sci(facts.yearsToGoogolplex)) years. The universe is 1.4·10¹⁰ years old.")
            fact("memorychip", "Deep zoom needs about 3.32 bits per digit of zoom. Here: \(HowBigFormat.int(facts.bitsHere)) bits; at 10³⁰⁰⁰, about 10 000.")
            fact("atom", "A googolplex zoom would need about \(HowBigFormat.sci(facts.bitsGoogolplex)) bits for one number: more bits than the ~10⁸⁰ atoms in the observable universe, by a factor of 10²⁰.")
            fact("gearshape.2", "Ganson's Machine with Concrete: 12 worm gears of 1:50 at 200 rpm, so its last gear would turn once in \(HowBigFormat.sci(facts.gansonYears)) years. Our ↑↑ dial, from 2 at 10¹⁰ to 2.54 at our limit, takes an hour of zooming.")
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(HowBigStyle.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(HowBigStyle.brassDark.opacity(0.6)))
    }

    private func fact(_ symbol: String, _ text: String) -> some View {
        Label {
            Text(text).fixedSize(horizontal: false, vertical: true)
        } icon: {
            Image(systemName: symbol).foregroundStyle(HowBigStyle.brassLight)
        }
        .font(.callout)
    }
}

struct BeyondCard: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Beyond every dial")
                .font(.system(.title3, design: .serif).weight(.semibold))
                .foregroundStyle(HowBigStyle.brassLight)
            entry("Graham's number",
                  "g₁ = 3↑↑↑↑3. Then g₂ = 3↑↑…↑3 with g₁ arrows, g₃ with g₂ arrows, and so on: Graham's number is g₆₄, 64 rounds deep in the *number of arrows*. Every dial above has a fixed count of arrows; g₂ already needs g₁ of them.")
            entry("Moser's number",
                  "From Steinhaus–Moser polygons. Mega, its first step, is a tower of about 257 tens (2.14 on the ↑↑↑ dial). Moser's number itself is only pinned from above: it is less than 3→3→4→2 = g₂, so vanishingly small next to Graham's number. We can't place it on a dial honestly.")
            entry("TREE(3)",
                  "Even the number of arrows can't describe it. The fast-growing hierarchy counts growth with ordinals: up-arrows sit at level ω, Graham's number is below level ω+1 (at 64), and TREE grows faster than level Γ₀, far past every ladder of arrows and towers of arrow counts. A known lower bound, A^A(187196)(1) (Ackermann), is called extremely weak.")
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(HowBigStyle.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(HowBigStyle.brassDark.opacity(0.6)))
    }

    private func entry(_ title: String, _ text: LocalizedStringKey) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(.system(.headline, design: .serif))
            Text(text)
                .font(.callout)
                .foregroundStyle(HowBigStyle.ivory.opacity(0.85))
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

// MARK: Formatting

enum HowBigFormat {
    static func int(_ v: Double) -> String {
        guard v.isFinite else { return "∞" }
        return Int(v.rounded()).formatted(.number.grouping(.automatic))
    }

    /// A dial reading: digits as a whole number of digits, higher dials with 3 decimals.
    static func dial(_ v: Double, level: Int) -> String {
        guard v.isFinite else { return "∞" }
        if level == 1 { return v < 1e6 ? String(format: "%.1f", v) : sci(v) }
        if v > 1e6 { return sci(v) }
        return String(format: v < 100 ? "%.3f" : "%.1f", v)
    }

    static let superscripts: [Character: Character] = [
        "0": "⁰", "1": "¹", "2": "²", "3": "³", "4": "⁴", "5": "⁵", "6": "⁶", "7": "⁷", "8": "⁸", "9": "⁹", "-": "⁻",
    ]

    /// "4.6·10⁹²".
    static func sci(_ v: Double) -> String {
        guard v.isFinite, v > 0 else { return v.isFinite ? "0" : "∞" }
        var e = floor(log10(v))
        var m = v / pow(10, e)
        if m >= 9.95 { m /= 10; e += 1 }
        let exp = String(String(Int(e)).map { superscripts[$0] ?? $0 })
        return String(format: "%.1f", m) + "·10" + exp
    }

    static func zoom(_ l: Double) -> String {
        l < 6 ? "\(int(pow(10, l)))×" : mantissa(l)
    }

    /// "1.2·10³⁴⁵×" for any depth (from log10).
    private static func mantissa(_ l: Double) -> String {
        var e = floor(l)
        var m = pow(10, l - e)
        if m >= 9.95 { m /= 10; e += 1 }
        let exp = String(String(Int(e)).map { superscripts[$0] ?? $0 })
        return String(format: "%.1f", m) + "·10" + exp + "×"
    }

    /// "10³⁰⁰⁸" (the exponent rounded).
    static func pow10(_ l: Double) -> String {
        "10" + String(String(Int(l.rounded())).map { superscripts[$0] ?? $0 })
    }

    static func speed(_ f: Double) -> String { String(format: "×%.2f/s", f) }

    static func duration(_ s: Double) -> String {
        if s < 90 { return String(format: "%.0f s", s) }
        if s < 3600 { return String(format: "%d min %02d s", Int(s) / 60, Int(s) % 60) }
        return String(format: "%.1f h", s / 3600)
    }
}
