import AlmondbreadCore
import SwiftUI

// The parameter lab (F10, kb: plans/f10-parameter-lab): z → zᵖ + c with three complex knobs, the
// start z₀, c and the exponent p. The main view shows one plane (the pixel is that knob, the
// other two fixed); three panels Rust draws show all three planes with a marker at each knob's
// value, and a press or drag there sets it. Rust owns the state (`Explorer::lab()`); the model
// mirrors it at the stats rate and sends what the controls here change. Entered from the
// formula picker's "Parameter lab" family. Shallow by design: zoom stops where float32 ends.

// MARK: Model

extension ExplorerModel {
    /// The formula picker's family of the lab (no formula of the list: a mode of its own).
    static let labFamily = "lab"
    static let labStateKey = "almondbread.lab.v1"

    /// What persists: on or off, the shown plane, the knobs (z₀ re, im, c re, im, p re, im).
    struct SavedLab: Codable {
        var on: Bool
        var plane: String
        var knobs: [Double]
    }

    /// Enters the lab with the knobs and plane it had last (Reset's values the first time). A
    /// tour ends first, here and not in Rust alone: the mirror takes the settings before the tour
    /// back from `endTour`; at once, the lab's way back is the view before the tour.
    public func enterLab() {
        guard let explorer, !isLab else { return }
        if tour != nil { endTour(fly: false) }
        if explorer.enterLab(plane: labPlane, knobs: labKnobs) {
            // (Rust left a Julia view first: the mirror follows, the lab with it.)
            pull(explorer.settings())
            host?.requestFrames()
        }
    }

    /// Leaves the lab for the view it was entered from.
    public func leaveLab() {
        guard let explorer, isLab else { return }
        _ = explorer.leaveLab()
        syncLab()
        host?.requestFrames()
    }

    /// Shows `plane` ("c", "z0", "p") in the main view.
    public func setLabPlane(_ plane: String) {
        guard let explorer, isLab, plane != labPlane else { return }
        _ = explorer.setLabPlane(plane: plane)
        labPlane = plane
        saveLab()
        host?.requestFrames()
    }

    /// Sets the knobs (Rust clamps them into their ranges; the mirror takes that over).
    public func setLabKnobs(_ knobs: LabKnobs) {
        guard let explorer, isLab else { return }
        _ = explorer.setLabKnobs(knobs: knobs)
        let clamped = explorer.lab().knobs
        if clamped != labKnobs { labKnobs = clamped }
        saveLab()
        host?.requestFrames()
    }

    /// Random knobs in interesting ranges (Rust's `lab::random`).
    public func labRandom() {
        guard let explorer, isLab else { return }
        labKnobs = explorer.labRandom(seed: UInt64.random(in: 1...UInt64.max))
        saveLab()
        host?.requestFrames()
    }

    /// Reset: z₀ = 0, c = −0.75, p = 2.
    public func labReset() {
        setLabKnobs(labDefaultKnobs())
    }

    /// Takes over Rust's lab state (on or off, plane, knobs: a panel drag changes them; another
    /// formula, a place, a paste or a tour leaves the lab) and saves a change. Assigns only on
    /// change.
    func syncLab() {
        guard let explorer else { return }
        let l = explorer.lab()
        let changed = l.on != isLab || (l.on && (l.plane != labPlane || l.knobs != labKnobs))
        guard changed else { return }
        if l.on != isLab {
            isLab = l.on
            let c = explorer.capabilities()
            if c != capabilities { capabilities = c }
        }
        if l.on {
            if l.plane != labPlane { labPlane = l.plane }
            if l.knobs != labKnobs { labKnobs = l.knobs }
        }
        saveLab()
    }

    /// The lab as the last launch left it (on: entered again, at its plane's home view).
    func restoreLab() {
        guard let data = Preferences.defaults.data(forKey: Self.labStateKey),
              let s = try? JSONDecoder().decode(SavedLab.self, from: data), s.knobs.count == 6 else { return }
        labPlane = ["c", "z0", "p"].contains(s.plane) ? s.plane : "c"
        let k = s.knobs
        labKnobs = LabKnobs(z0Re: k[0], z0Im: k[1], cRe: k[2], cIm: k[3], pRe: k[4], pIm: k[5])
        if s.on { enterLab() }
    }

    func saveLab() {
        guard labSaves else { return }
        let k = labKnobs
        let s = SavedLab(on: isLab, plane: labPlane, knobs: [k.z0Re, k.z0Im, k.cRe, k.cIm, k.pRe, k.pIm])
        if let data = try? JSONEncoder().encode(s) { Preferences.defaults.set(data, forKey: Self.labStateKey) }
    }
}

extension LabKnobs {
    /// One knob's value: "z0", "c" or "p".
    func value(_ plane: String) -> (re: Double, im: Double) {
        switch plane {
        case "z0": (z0Re, z0Im)
        case "p": (pRe, pIm)
        default: (cRe, cIm)
        }
    }

    /// The knobs with one part of one knob changed.
    func with(_ plane: String, re: Double? = nil, im: Double? = nil) -> LabKnobs {
        var k = self
        switch plane {
        case "z0":
            k.z0Re = re ?? k.z0Re
            k.z0Im = im ?? k.z0Im
        case "p":
            k.pRe = re ?? k.pRe
            k.pIm = im ?? k.pIm
        default:
            k.cRe = re ?? k.cRe
            k.cIm = im ?? k.cIm
        }
        return k
    }
}

/// A knob of the lab: its id ("z0", "c", "p"), symbol, its role in the formula (the rows'
/// header) and what its plane shows.
struct LabKnob: Identifiable {
    let id: String
    let symbol: String
    let role: String
    let plane: String
    let help: String

    /// In the order of the formula z → zᵖ + c: the start, the exponent, the constant.
    static let all = [
        LabKnob(id: "z0", symbol: "z₀", role: "where z starts", plane: "z₀ plane",
                help: "z₀: where z starts. Its plane is a Julia-like set: each point is one start"),
        LabKnob(id: "p", symbol: "p", role: "the exponent in zᵖ", plane: "p plane",
                help: "p: the exponent in zᵖ. Its plane is the X-set: each point is one exponent"),
        LabKnob(id: "c", symbol: "c", role: "added at each step", plane: "c plane",
                help: "c: the number added at each step. Its plane is a Mandelbrot-like set: each point is one c"),
    ]

    /// The slider ranges from Rust (`lab_range`): re and im.
    var range: (re: ClosedRange<Double>, im: ClosedRange<Double>) {
        let r = labRange(plane: id)
        guard r.count == 4 else { return (-2...2, -2...2) }
        return (r[0]...r[1], r[2]...r[3])
    }
}

// MARK: Controls

/// The lab's rows under the formula picker: which plane the main view shows, the three knobs
/// (a slider and a number field for each part), Random and Reset, and what the lab cannot do. On
/// the phone only the plane's row is here, the others lead the list (`LabListSection`).
struct LabControls: View {
    let model: ExplorerModel

    /// Pinned, all the rows took about 500 pt of a phone, more than its half sheet has: the
    /// search and the close button went off the top and the list had no room left. By the device,
    /// as `FractalRow.turnFolded`: the iPad's control-center pane reports a compact width too.
    static var inList: Bool {
        #if os(iOS)
        UIDevice.current.userInterfaceIdiom == .phone
        #else
        false
        #endif
    }

    static let note = "Shallow: zooming stops after about ×1000. If p is not a whole number, zᵖ jumps along the negative real axis, and the picture shows that line."

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Picker("Main view", selection: Binding(get: { model.labPlane }, set: { model.setLabPlane($0) })) {
                Text("c plane").tag("c")
                Text("z₀ plane").tag("z0")
                Text("p plane").tag("p")
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .help("The plane the big picture shows: each of its points is one value of that knob, the other two stay fixed")
            .accessibilityIdentifier("lab-plane")
            if !Self.inList {
                ForEach(LabKnob.all) { k in
                    LabKnobRows(model: model, knob: k)
                }
                LabButtons(model: model)
                Text(Self.note)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.bottom, 2)
    }
}

/// The phone's lab rows: the first section of the control center's list.
struct LabListSection: View {
    let model: ExplorerModel

    var body: some View {
        Section {
            ForEach(LabKnob.all) { k in
                LabKnobRows(model: model, knob: k)
            }
            // (A style of their own: in a list row a default button takes the whole row's touches.)
            LabButtons(model: model).buttonStyle(.bordered)
        } footer: {
            Text(LabControls.note)
        }
    }
}

/// Random and Reset.
struct LabButtons: View {
    let model: ExplorerModel

    var body: some View {
        HStack(spacing: 8) {
            Button("Random") { model.labRandom() }
                .help("New values: p between 1.5 and 5, z₀ near 0, c near the edge of the set")
                .accessibilityIdentifier("lab-random")
            Button("Reset") { model.labReset() }
                .help("z₀ = 0, c = −0.75, p = 2: the Mandelbrot set")
                .accessibilityIdentifier("lab-reset")
            Spacer(minLength: 0)
        }
    }
}

/// One knob: a header (symbol and role), then a row each for the real and the imaginary part,
/// labelled re and im, with a slider and a number field; the fields read "a + b i".
struct LabKnobRows: View {
    let model: ExplorerModel
    let knob: LabKnob

    var body: some View {
        let v = model.labKnobs.value(knob.id)
        let range = knob.range
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                Text(knob.symbol).bold()
                Text(knob.role).foregroundStyle(.secondary)
            }
            .help(knob.help)
            part("re", value: v.re, range: range.re) { model.setLabKnobs(model.labKnobs.with(knob.id, re: $0)) }
            part("im", value: v.im, range: range.im) { model.setLabKnobs(model.labKnobs.with(knob.id, im: $0)) }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("lab-knob-\(knob.id)")
    }

    private func part(_ name: String, value: Double, range: ClosedRange<Double>,
                      set: @escaping (Double) -> Void) -> some View {
        HStack(spacing: 6) {
            Text(name)
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .frame(width: 16, alignment: .leading)
                .accessibilityHidden(true)
            Slider(value: Binding(get: { min(max(value, range.lowerBound), range.upperBound) }, set: set), in: range)
                .accessibilityLabel("\(knob.symbol) \(name == "re" ? "real part" : "imaginary part")")
                .accessibilityIdentifier("lab-\(knob.id)-\(name)")
            // "+" before the imaginary part's field and "i" after it; the real part's row keeps
            // the same room empty, so both fields line up.
            Text(name == "im" ? "+" : " ").foregroundStyle(.secondary).frame(width: 8)
            TextField(name, value: Binding(get: { value }, set: set),
                      format: .number.precision(.fractionLength(4)))
                .multilineTextAlignment(.trailing)
                .monospacedDigit()
                .frame(width: 72)
                #if os(iOS)
                .keyboardType(.numbersAndPunctuation)
                #endif
                .accessibilityIdentifier("lab-\(knob.id)-\(name)-field")
            Text(name == "im" ? "i" : " ").foregroundStyle(.secondary).frame(width: 6)
        }
    }
}

// MARK: UI demo scenes

/// `--ui-demo=lab` (the c plane), `lab-z0`, `lab-p`: the lab with fixed knobs (p = 2.07 − 0.196i,
/// z₀ = 0.056 − 0.074i, c = −0.035 + 0.806i: seed 3 of Random), the orbit on; `lab-reset`: Reset's
/// knobs (the Mandelbrot set and its neighbours).
enum LabDemo {
    @MainActor
    static func configure(_ model: ExplorerModel, explorer: Explorer, scene: String) {
        model.orbitEnabled = scene != "lab-reset"
        model.orbitLength = 40
        model.mapEnabled = false
        model.labPlane = scene == "lab-z0" ? "z0" : scene == "lab-p" ? "p" : "c"
        model.labKnobs = scene == "lab-reset" ? labDefaultKnobs()
            : LabKnobs(z0Re: 0.0555, z0Im: -0.0739, cRe: -0.0347, cIm: 0.8060, pRe: 2.0731, pIm: -0.1961)
        // (With the view's size: the plane's home view is fitted to the view.)
        model.afterViewSized { model.enterLab() }
    }
}
