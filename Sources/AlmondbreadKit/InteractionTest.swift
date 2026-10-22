#if os(macOS)
import AlmondbreadCore
import AppKit
import QuartzCore

/// `--interaction-test`: drives the real window and the Rust renderer with synthetic pans,
/// zooms and keys through the view's own input path, one input per display refresh (like a
/// trackpad), and prints frame pacing and latency per scenario, then quits. Interaction
/// smoothness is an acceptance criterion; nobody watches the screen during agent runs, so it
/// is measured: presentation times come from the drawables (Rust's presented handlers), draw
/// CPU is the time inside `Explorer.frame`, and the Swift-side time of the same call gives
/// the FFI overhead per frame. Runs in the background (TestLaunch) and quits by itself.
@MainActor
final class InteractionTest: NSObject {
    static let enabled = CommandLine.arguments.contains("--interaction-test")
    static let shared = InteractionTest()

    private weak var view: FractalNSView?
    private var link: CADisplayLink?
    private var scenario = -1
    private var phaseStart: CFTimeInterval = 0
    private var tick = 0
    private var settling = true
    private var inputEnd: CFTimeInterval = 0
    private var reported = false
    private var refineLines: [String] = []
    private var ffiLines: [String] = []
    private var failures = 0
    /// Display-link callbacks of the measured interval: timestamps and granted durations
    /// (`targetTimestamp - timestamp`): what rate the system actually drives the window at.
    private var linkTimes: [CFTimeInterval] = []
    private var linkDurations: [CFTimeInterval] = []

    private struct View {
        let re: String, im: String, scale: Double
        /// The fractal (F3): Multibrot scenarios switch to theirs, every other one is Mandelbrot.
        var formula = "mandelbrot"
        /// BB (kb: plans/buddhabrot): a look for the scenario ("buddhabrot", "nebulabrot");
        /// nil: the look the test started with.
        var look: String? = nil
    }
    private struct Scenario {
        let name: String
        let viewport: View
        let input: (FractalNSView, Int, Double) -> Void   // view, tick, seconds since input start
        /// Seconds of input (and measurement).
        var seconds = 3.0
        /// F4 (kb: plans/f4-deep-julia): a Julia set as the main view: its k (re, im), or "twin"
        /// for the Julia twin of the viewport; nil: the Mandelbrot set.
        var julia: (re: String, im: String)? = nil
        /// A Julia view's Mandelbrot panel (kb: decisions/controls "Mandelbrot panel"): on at
        /// every entry; false turns it off before the settling phase.
        var mandelbrotPanel = true
    }
    private let settleSeconds = 1.0
    private var startLook: String?

    private lazy var scenarios: [Scenario] = {
        // The home view fitted to 1280x820 points (Viewport::fitting).
        let home = View(re: "-0.75", im: "0", scale: max(3.2 / 1280, 2.6 / 820))
        let sea = View(re: "-0.743643887037151", im: "0.13182590420533", scale: 3.75e-3 / 1e5)
        // Circular pan, 10 pt per frame; zoom in 2% per frame around an off-centre point.
        let pan: (FractalNSView, Int, Double) -> Void = { v, i, _ in
            let a = Double(i) * 0.05
            v.send(.pan(dx: 10 * cos(a), dy: 10 * sin(a)))
        }
        let zoom: (FractalNSView, Int, Double) -> Void = { v, i, _ in
            v.send(.magnify(magnification: i < 90 ? 0.02 : 1 / 1.02 - 1, x: 120, y: -60))
        }
        // Continuous zoom from the former Double precision limit (1e-14) through the switch to
        // big references (M3b: no clamp any more).
        let limit = View(re: "-0.743643887037151", im: "0.13182590420533", scale: 1e-14)
        // M3b (kb: synthesis/benchmarks M3b): a fast zoom from 1e-12 to ~1e-40 per point (Space
        // speed is too slow to cross that in 3 s: 0.36 e-folds per frame instead) through the
        // f64 limit and the plain/deep kernel switch, into the seahorse valley near the
        // period-8007 minibrot; and pans/zooms around c = i at 1e-100 and 1e-300.
        let seaDeep = View(re: "-0.743643887037158704752191506114774", im: "0.131825904205311970493132056385139", scale: 1e-12)
        let deepZoom: (FractalNSView, Int, Double) -> Void = { v, _, _ in v.send(.magnify(magnification: 0.43, x: 200, y: 100)) }
        let dendrite100 = View(re: "0", im: "1", scale: 1e-100)
        let dendrite300 = View(re: "0", im: "1", scale: 1e-300)
        let deep: (FractalNSView, Int, Double) -> Void = { v, _, _ in v.send(.magnify(magnification: 0.05, x: 200, y: 100)) }
        // Keyboard, scheduled in seconds (tick counts depend on the achieved frame rate): hold
        // D, add W at 1 s (diagonal), release both at 2.5 s; Space zoom with one E press at
        // 1.25 s, released at 2.5 s.
        var sent = Set<String>()
        func once(_ id: String, _ due: Bool, _ action: () -> Void) {
            if due && !sent.contains(id) { sent.insert(id); action() }
        }
        let wasd: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 { sent.removeAll() }
            once("d", true) { InteractionTest.key(v, "d", down: true) }
            once("w", t >= 1) { InteractionTest.key(v, "w", down: true) }
            once("up", t >= 2.5) { InteractionTest.key(v, "d", down: false); InteractionTest.key(v, "w", down: false) }
        }
        let space: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 { sent.removeAll() }
            once("space", true) { InteractionTest.key(v, " ", down: true) }
            once("e", t >= 1.25) { InteractionTest.key(v, "e", down: true); InteractionTest.key(v, "e", down: false) }
            once("up", t >= 2.5) { InteractionTest.key(v, " ", down: false) }
        }
        // The -3/4 neck of the wedge report (zoom 2.2e5): wheel-like zoom in and out.
        let neck = View(re: "-0.7500016386523336", im: "-0.0021073424656852589", scale: 1.69263e-8)

        // Controls revision (kb: decisions/controls): the zoom target's fixed point, the
        // keyboard crosshair's glide and edge panning, driven per refresh like the keys above.
        let target = (x: 380.0, y: -200.0)
        var c0: (re: Double, im: Double)?
        var zoomStart = 0.0
        // Pointer mode, pointer off-centre, Space held 2.5 s: the coordinate under the
        // pointer must stay put (< 1 px per second of zooming).
        let pointerSpace: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 {
                sent.removeAll()
                v.send(.pointer(x: target.x, y: target.y))
                c0 = InteractionTest.point(v, target.x, target.y)
                zoomStart = CACurrentMediaTime()
                InteractionTest.key(v, " ", down: true)
            }
            once("up", t >= 2.5) {
                InteractionTest.key(v, " ", down: false)
                self.checkDrift("pointer space", v, c0, target.x, target.y, seconds: CACurrentMediaTime() - zoomStart,
                                note: String(format: "pointer at (%.0f, %.0f) pt, mode %@", target.x, target.y,
                                             "\(v.base.mode)"))
            }
        }
        // From pointer mode, D tapped for 0.1 s while Space is held: keyboard mode, the
        // crosshair glides from the pointer to the centre (no overshoot, ~0.27 s to 1 %), the
        // zoom follows it; after arrival the centre must stay put.
        var glide = (start: 0.0, last: 0.0, pct1: Double?.none, arrived: Double?.none, overshoot: false, monotonic: true)
        let keyboardGlide: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 {
                sent.removeAll()
                v.send(.pointer(x: target.x, y: target.y))
                c0 = nil
                InteractionTest.key(v, " ", down: true)
                InteractionTest.key(v, "d", down: true)
                let d0 = hypot(target.x, target.y)
                glide = (CACurrentMediaTime(), d0, nil, nil, false, true)
            }
            if let f = v.base.lastFrame, f.keyboard {
                let d = hypot(f.x, f.y)
                if d > glide.last + 1e-9 { glide.monotonic = false }
                // Started at (+x, −y): crossing either axis would be an overshoot.
                if f.x < -1e-9 || f.y > 1e-9 { glide.overshoot = true }
                glide.last = d
                let now = CACurrentMediaTime() - glide.start
                if glide.pct1 == nil && d < 0.01 * hypot(target.x, target.y) { glide.pct1 = now }
                if glide.arrived == nil && d == 0 { glide.arrived = now }
                // The fixed point is measured once the tapped key's pan has glided out too
                // (FL: released keys ease out over ~0.5 s instead of stopping dead).
                if c0 == nil && glide.arrived != nil && now >= 0.9 {
                    c0 = InteractionTest.point(v, 0, 0)
                    zoomStart = CACurrentMediaTime()
                }
            }
            once("dup", t >= 0.1) { InteractionTest.key(v, "d", down: false) }
            once("up", t >= 2.5) {
                InteractionTest.key(v, " ", down: false)
                let ok = !glide.overshoot && glide.monotonic && (glide.pct1 ?? 1) <= 0.35 && glide.arrived != nil
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "keyboard glide: mode %@, crosshair from (%.0f, %.0f) pt: 1 %% after %.0f ms, at the centre after %.0f ms, overshoot %@, monotonic %@ %@",
                                               "\(v.base.mode)", target.x, target.y, (glide.pct1 ?? .nan) * 1000,
                                               (glide.arrived ?? .nan) * 1000, glide.overshoot ? "YES" : "no",
                                               glide.monotonic ? "yes" : "NO", ok ? "OK" : "FAILED"))
                self.checkDrift("keyboard glide", v, c0, 0, 0, seconds: CACurrentMediaTime() - zoomStart,
                                note: "centre after arrival")
            }
        }
        // A click stream (kb: decisions/controls "Animated zoom steps"): 15 real mouse-down / up
        // pairs 0.18 s apart (click counts 1-15) through FractalNSView's handlers, then rest.
        // Clicks 2-15 each add ×2 to the running glide: ×2^14 in all, about the clicked point,
        // and the glide runs on without a break until 2.87 s (pacing is measured on it).
        let clickAt = (x: 150.0, y: 80.0)
        let clickStream: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 {
                sent.removeAll()
                c0 = InteractionTest.point(v, clickAt.x, clickAt.y)
                zoomStart = CACurrentMediaTime()
            }
            for k in 1...15 {
                once("click\(k)", t >= Double(k - 1) * 0.18) { InteractionTest.click(v, clickAt.x, clickAt.y, count: k) }
            }
            once("check", t >= 2.95) {
                let ratio = self.startScale("click stream") / (v.explorer?.viewport().scale ?? .nan)
                let ok = abs(ratio / 16384 - 1) < 1e-3
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "click stream: 15 clicks zoomed ×%.1f (expected ×16384) %@", ratio, ok ? "OK" : "FAILED"))
                self.checkDrift("click stream", v, c0, clickAt.x, clickAt.y, seconds: CACurrentMediaTime() - zoomStart,
                                note: "clicked point")
            }
        }
        // Pointer mode, pointer 8 pt inside the right edge: no pan without a zoom (setting
        // "while zooming"), then Space: the view pans right at the ramped key speed; the
        // pointer leaves the window at 2 s: the pan stops, the zoom goes on.
        var edge = (prev: ViewportInfo?.none, start: ViewportInfo?.none, pan: 0.0, panAfterExit: 0.0, idleMoved: false,
                    x: 0.0, expected: 0.0, panStart: 0.0, panSeconds: 0.0)
        let edgePan: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                let ins = v.safeAreaInsets
                let w = v.bounds.width - ins.left - ins.right, h = v.bounds.height - ins.top - ins.bottom
                let band = 0.08 * Double(min(w, h))
                let x = Double(v.bounds.width / 2 - ins.right) - 8
                let r = max(0, min(1, 1 - 8 / band))
                edge = (nil, explorer.viewport(), 0, 0, false, x, 0.4 * r * r * (3 - 2 * r), 0, 0)
                v.send(.pointer(x: x, y: -100))
            }
            let vp = explorer.viewport()
            if t < 0.5 {
                if vp != edge.start { edge.idleMoved = true }
                return
            }
            once("space", true) {
                InteractionTest.key(v, " ", down: true)
                edge.prev = vp
                edge.panStart = CACurrentMediaTime()
            }
            // Pan of this step in points: the centre's move minus what zooming at the target
            // alone moves it (the pan is applied before the zoom, at the previous scale).
            if let p = edge.prev, let re = Double(vp.re), let pre = Double(p.re) {
                let tx = sent.contains("exit") ? 0 : edge.x
                let pan = (re - (pre + tx * (p.scale - vp.scale))) / p.scale
                if sent.contains("exit") { edge.panAfterExit += pan } else { edge.pan += pan }
            }
            edge.prev = vp
            once("exit", t >= 2.0) {
                v.send(.pointerExited)
                edge.panSeconds = CACurrentMediaTime() - edge.panStart
            }
            once("up", t >= 2.5) {
                InteractionTest.key(v, " ", down: false)
                let rate = edge.pan / max(edge.panSeconds, 1e-3) / Double(v.bounds.width)
                // FL motion feel: the pan eases in (critically damped, ω = 30/s: the average
                // over the run loses 2/ω s of speed) and glides out after the pointer left
                // (2/ω = 1/9 s of speed at ω = 18/s).
                let expected = edge.expected * (1 - (2.0 / 30) / max(edge.panSeconds, 0.1))
                let glide = edge.expected * Double(v.bounds.width) * 2 / 18
                let ok = !edge.idleMoved && abs(rate - expected) <= 0.1 * expected
                    && abs(edge.panAfterExit - glide) <= 0.35 * glide
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "edge pan: pointer %.0f pt right of centre: %.3f view widths/s while zooming (expected %.3f with the eased start), moved without zoom %@, glided %.1f pt after the pointer left (expected %.1f) %@",
                                               edge.x, rate, expected, edge.idleMoved ? "YES" : "no",
                                               edge.panAfterExit, glide, ok ? "OK" : "FAILED"))
            }
        }
        // Overlays (kb: decisions/controls, "Orbit overlay and mini-map"): the orbit of 200
        // iterates following a pointer circling the seahorse view (the view itself idle: every
        // frame is an overlay-only frame), and a zoom with orbit and three map levels on.
        let overlays = { (v: FractalNSView, orbit: Bool, map: Bool, julia: Bool) in
            v.explorer?.setOverlay(settings: OverlaySettings(orbit: orbit, orbitLength: 200, map: map, mapLevels: 3,
                                                             mapSize: 180, mapCorner: .bottomLeft, julia: julia))
        }
        let overlayOn = { (v: FractalNSView, orbit: Bool, map: Bool) in overlays(v, orbit, map, false) }
        // The Julia panel (kb: plans/f1-julia-view): the pointer sweeps across the whole set at
        // the home view (k moves many panel pixels every frame: a one-sample picture each
        // refresh, the worst case), with the orbit and the mini-map on; "sweep" is the same with
        // the panel off. "julia pan": the home view pans under a resting pointer (k changes
        // every frame while the main view renders; "overlays pan" without the panel).
        let sweep = { (julia: Bool) -> (FractalNSView, Int, Double) -> Void in
            { v, i, _ in
                if i == 0 { overlays(v, true, true, julia) }
                let a = Double(i) * 0.03
                v.send(.pointer(x: -60 + 190 * cos(a), y: 160 * sin(a)))
            }
        }
        let overlayPan = { (julia: Bool) -> (FractalNSView, Int, Double) -> Void in
            { v, i, t in
                if i == 0 {
                    overlays(v, true, true, julia)
                    v.send(.pointer(x: 90, y: 120))
                }
                pan(v, i, t)
            }
        }
        let orbitPointer: (FractalNSView, Int, Double) -> Void = { v, i, _ in
            if i == 0 { overlayOn(v, true, false) }
            let a = Double(i) * 0.06
            v.send(.pointer(x: 220 * cos(a), y: 150 * sin(a)))
        }
        // The orbit's dot cloud on the whole-set map (orbit on, maps off): the pointer rests
        // on a point at the main cardioid's edge (golden-mean angle: undecided after 10^5
        // steps, so the cloud fills to its 10^5 points) and moves half a point every 0.75 s
        // (a new probe; the cloud grows again, rebuilt at each ~19 % of growth).
        let orbitCloud: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 { overlayOn(v, true, false) }
            v.send(.pointer(x: 0.5 * Double(Int(t / 0.75)), y: 0))
        }
        // The cosmic size scale alone during a zoom (its panel is rebuilt every frame).
        let cosmicZoom: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 {
                v.explorer?.setCosmicScale(settings: CosmicScaleSettings(enabled: true, metresPerPoint: 0))
            }
            zoom(v, i, t)
        }
        let overlayZoom: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 {
                overlayOn(v, true, true)
                v.send(.pointer(x: 120, y: -60))
            }
            zoom(v, i, t)
        }
        // Autopilot (kb: decisions/controls, autopilot, "Driving levels"): Z, the synthetic
        // pointer circles 220 pt off-centre: steering hands over to cruise (C) at the
        // autopilot's speed, which goes on zooming and steering; Esc at 2.4 s (eased stop).
        // Checks it zoomed, handed over and steered.
        var ap = (start: ViewportInfo?.none, active: false, cruise: false, early: false)
        let autopilot: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                ap = (explorer.viewport(), false, false, false)
                v.send(.pointer(x: 0, y: 0))
                InteractionTest.key(v, "z", down: true)
                InteractionTest.key(v, "z", down: false)
            }
            let a = 0.8 * t
            v.send(.pointer(x: 220 * cos(a), y: 220 * sin(a)))
            if explorer.stats().autopilot.active { ap.active = true }
            // The circling pointer is a direction wish in A, never a hand-over (kb:
            // decisions/controls "Pointer wish"); a drag at 1.6 s hands over to cruise.
            if ap.active && explorer.stats().cruise.level == .cruise {
                if t < 1.6 { ap.early = true } else { ap.cruise = true }
            }
            once("drag", t >= 1.6) { v.send(.pan(dx: 6, dy: 0)) }
            once("esc", t >= 2.4) {
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                let vp = explorer.viewport()
                let zoomed = (ap.start?.zoomLog10).map { vp.zoomLog10 - $0 } ?? 0
                let moved = ap.start.flatMap { s in Double(vp.re).flatMap { re in Double(s.re).map { (re - $0) / vp.scale } } } ?? 0
                let ok = ap.active && !ap.early && ap.cruise && zoomed > 0.5 && abs(moved) > 1
                    && !explorer.stats().autopilot.active
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "autopilot: on %@, pointer kept A %@, drag handed over to cruise %@, zoomed ×%.1f in 2.4 s, centre moved %.0f pt by steering, off after Esc %@ %@",
                                               ap.active ? "yes" : "NO", ap.early ? "NO" : "yes", ap.cruise ? "yes" : "NO",
                                               pow(10, zoomed), moved,
                                               explorer.stats().autopilot.active ? "NO" : "yes", ok ? "OK" : "FAILED"))
            }
        }
        // Fly to (kb: decisions/controls, fly to): from the 1e5 view to 1e-11 per point near the
        // seahorse point (f64 references: deeper, the flight waits for big references by
        // design; `make regress` covers 1e-34 and 1e-300) at ~6x the default speed; must land
        // exactly.
        let flyTarget = (re: "-0.74364388703715870475219150611477977822", im: "0.13182590420531197049313205638514067897",
                         scale: "1e-11")
        var flew = false
        var landedAt: Double?
        let flyTo: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                flew = false
                landedAt = nil
                try? explorer.flyTo(re: flyTarget.re, im: flyTarget.im, scale: flyTarget.scale)
                for _ in 0..<8 { InteractionTest.key(v, "e", down: true); InteractionTest.key(v, "e", down: false) }
                v.requestFrames()
            }
            if explorer.stats().flight.active { flew = true } else if flew && landedAt == nil { landedAt = t }
            once("check", t >= 2.9) {
                let s = explorer.stats()
                let vp = explorer.viewport()
                let ok = flew && !s.flight.active && vp.scaleText == "1e-11" && vp.re.hasPrefix("-0.7436438870371587")
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "fly to: flew %@, landed %@ after %.2f s at scale %@ (target %@), speed %.2f e/s %@",
                                               flew ? "yes" : "NO", s.flight.active ? "NOT YET" : "yes", landedAt ?? .nan,
                                               vp.scaleText, flyTarget.scale, s.flight.speed, ok ? "OK" : "FAILED"))
            }
        }
        // Every look (the inspector's Look picker: `applyLook`) and every palette (the Palette
        // picker: `paletteID`), one every 0.1 s, idle (progressive AA running) or during a
        // pan, at home and deep. Regression for "selected Lagoon and almondbread died".
        let switches: [(ExplorerModel) -> Void] = ExplorerModel.looks.filter { !$0.orbitDensity }.map { l in { $0.applyLook(l.id) } }
            + ExplorerModel.palettes.map { p in { $0.paletteID = p.id } }
        var switched = 0
        @MainActor func switcher(gesture: Bool) -> (FractalNSView, Int, Double) -> Void {
            { v, i, t in
                if i == 0 { switched = 0; sent.removeAll() }
                if gesture { pan(v, i, t) }
                let due = Int(t / 0.1)
                while switched <= due && switched < switches.count {
                    switches[switched](v.model)
                    switched += 1
                }
                if switched == switches.count, !sent.contains("looks") {
                    sent.insert("looks")
                    self.refineLines.append("looks: \(switches.count) looks and palettes applied (\(gesture ? "panning" : "idle"))")
                }
            }
        }
        // View rotation (kb: decisions/controls, "View rotation"): hold "." (turn) and Space
        // (zoom) together for 2.5 s at 1e5; the view must have turned and zoomed, and the
        // settled frame must match a fresh render of the rotated view.
        var turnStart = (angle: 0.0, zoom: 0.0)
        let rotateZoom: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                let vp = explorer.viewport()
                turnStart = (vp.angleDegrees, vp.zoomLog10)
                v.send(.pointer(x: 150, y: 80))
                InteractionTest.key(v, ".", down: true)
                InteractionTest.key(v, " ", down: true)
            }
            once("up", t >= 2.5) {
                InteractionTest.key(v, ".", down: false)
                InteractionTest.key(v, " ", down: false)
                let vp = explorer.viewport()
                let turned = vp.angleDegrees - turnStart.angle
                let ok = abs(turned) > 60 && vp.zoomLog10 - turnStart.zoom > 0.5
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "rotate zoom: turned %.1f° and zoomed ×%.1f in 2.5 s %@",
                                               turned, pow(10, vp.zoomLog10 - turnStart.zoom), ok ? "OK" : "FAILED"))
            }
        }
        // Cinematic autopilot: effects on, Z, Esc at 2.4 s: the view turns with the zoom.
        var cine = (angle: 0.0, zoom: 0.0)
        let cinematicAutopilot: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                v.model.cinematic = true
                let vp = explorer.viewport()
                cine = (vp.angleDegrees, vp.zoomLog10)
                v.send(.pointer(x: 0, y: 0))
                InteractionTest.key(v, "z", down: true)
                InteractionTest.key(v, "z", down: false)
            }
            once("esc", t >= 2.4) {
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                let vp = explorer.viewport()
                let efolds = (vp.zoomLog10 - cine.zoom) * log(10)
                let turned = vp.angleDegrees - cine.angle
                // 4° per e-fold at the default spin (the eased stop adds a little more later).
                let ok = efolds > 0.5 && abs(turned - 4 * efolds) < 0.5
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "cinematic autopilot: %.2f e-folds, turned %.2f° (expected %.2f) %@",
                                               efolds, turned, 4 * efolds, ok ? "OK" : "FAILED"))
            }
            once("off", t >= 2.9) { v.model.cinematic = false }
        }
        // Compass (kb: decisions/controls, "Compass"): the cinematic autopilot spins a view
        // turned to 50° (the compass shows and turns every frame: its overlay cost is in the
        // draw CPU); at 0.3 s a drag a quarter turn around the compass centre turns the view by
        // −90° (plus the spin), at 1.6 s a click eases it back to north (0.4 s) while the spin
        // goes on, Esc at 2.5 s.
        var compassAt = (x: 0.0, y: 0.0, angle: 0.0)
        let compass: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                v.model.cinematic = true
                explorer.setAngle(degrees: 50)
                v.send(.pointer(x: 0, y: 0))
                InteractionTest.key(v, "z", down: true)
                InteractionTest.key(v, "z", down: false)
            }
            let r = 15.0
            once("press", t >= 0.3) {
                let c = explorer.compassCenter()
                guard c.count == 2 else {
                    self.failures += 1
                    self.refineLines.append("compass: not shown at 50° FAILED")
                    return
                }
                compassAt = (c[0], c[1], explorer.viewport().angleDegrees)
                _ = v.model.compassPress(x: c[0] + r, y: c[1])
            }
            if t >= 0.3 && t < 1.0 && sent.contains("press") {
                let phi = min((t - 0.3) / 0.6, 1) * .pi / 2
                v.model.compassDrag(x: compassAt.x + r * cos(phi), y: compassAt.y + r * sin(phi))
            }
            once("release", t >= 1.0) {
                v.model.compassDrag(x: compassAt.x, y: compassAt.y + r)
                v.model.compassRelease()
                let turned = explorer.viewport().angleDegrees - compassAt.angle
                // −90° by the drag, plus up to a few degrees of spin meanwhile.
                let ok = turned < -85 && turned > -95
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "compass drag: a quarter turn of the pointer turned the view %.1f° %@",
                                               turned, ok ? "OK" : "FAILED"))
            }
            once("click", t >= 1.6) {
                compassAt.angle = explorer.viewport().angleDegrees
                _ = v.model.compassTap(x: compassAt.x, y: compassAt.y)
            }
            once("north", t >= 2.2) {
                let a = explorer.viewport().angleDegrees
                // North 0.4 s after the click; the spin then goes on (4° per e-fold).
                let ok = abs(a) < 2 && abs(compassAt.angle) > 10
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "compass click: %.1f° to %.2f° 0.6 s later, spinning on %@",
                                               compassAt.angle, a, ok ? "OK" : "FAILED"))
            }
            once("esc", t >= 2.5) {
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
            }
            once("off", t >= 2.9) { v.model.cinematic = false }
        }
        // Autopilot target lock (kb: decisions/controls, "Autopilot"): Z with the pointer resting
        // at the centre; within 2.4 s the lock must aim at a boundary target (searched on a
        // thread: frame pacing must not suffer), then Esc.
        var lockLabel = ""
        let autopilotLock: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                lockLabel = ""
                explorer.setAutopilotLock(on: true)
                v.send(.pointer(x: 0, y: 0))
                InteractionTest.key(v, "z", down: true)
                InteractionTest.key(v, "z", down: false)
            }
            let l = explorer.stats().autopilot.lockLabel
            if l.hasPrefix("towards") { lockLabel = l }
            once("esc", t >= 2.4) {
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                let ok = !lockLabel.isEmpty
                if !ok { self.failures += 1 }
                self.refineLines.append("autopilot lock: \(lockLabel.isEmpty ? "no target" : lockLabel) \(ok ? "OK" : "FAILED")")
                explorer.setAutopilotLock(on: v.model.autopilotLock)
            }
        }
        // Journey (kb: decisions/controls, "Journey"): J from the 1e5 view; within 2.8 s the
        // first leg must be planned and flying; Esc then stops it (the view settles for the
        // shown-vs-fresh check).
        var journeyLegs = 0
        let journey: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                journeyLegs = 0
                InteractionTest.key(v, "j", down: true)
                InteractionTest.key(v, "j", down: false)
            }
            let j = explorer.stats().journey
            journeyLegs = max(journeyLegs, Int(j.leg))
            once("esc", t >= 2.8) {
                let s = explorer.stats()
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                let ok = journeyLegs >= 1 && s.journey.active && !explorer.stats().journey.active
                if !ok { self.failures += 1 }
                self.refineLines.append("journey: #\(s.journey.seed) leg \(journeyLegs) \"\(s.journey.label)\" \(s.journey.phase), off after Esc \(explorer.stats().journey.active ? "NO" : "yes") \(ok ? "OK" : "FAILED")")
            }
        }
        // Reverse cruise (kb: decisions/controls "Driving levels"): Q six times (gear 6, ×2/s out) from the
        // 1e5 view; by 2.4 s it must be cruising out (more than 0.4 e-folds so far, the depth
        // boost building up; the renderer back-off may halve it); Esc then brakes it to rest.
        var cruiseStart = 0.0
        var cruiseSeen = false
        let cruiseOut: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                cruiseSeen = false
                cruiseStart = explorer.viewport().zoomLog10
                for _ in 0..<6 {
                    InteractionTest.key(v, "q", down: true)
                    InteractionTest.key(v, "q", down: false)
                }
            }
            if explorer.stats().cruise.level == .cruise { cruiseSeen = true }
            once("esc", t >= 2.4) {
                let out = (cruiseStart - explorer.viewport().zoomLog10) * log(10.0)
                let c = explorer.stats().cruise
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                let braking = explorer.stats().cruise.set == 0
                let ok = cruiseSeen && out > 0.4 && braking
                if !ok { self.failures += 1 }
                self.refineLines.append(String(format: "cruise out: %.2f e-folds out in 2.4 s (rate %.2f e/s, boost ×%.2f), braking after Esc %@ %@",
                                               out, c.rate, c.boost, braking ? "yes" : "NO", ok ? "OK" : "FAILED"))
            }
        }
        // Journey with stops (kb: decisions/controls "Journey with stops"): J in the "stop at
        // each find" mode from the 1e5 view at a high journey speed (E ×18: past the top gear,
        // 8.6 e-folds/s); at the first stop X
        // (somewhere else) must start the next leg; Esc at 2.9 s.
        var stopSeen = ""
        var legAtStop = 0
        let journeyStops: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                stopSeen = ""
                legAtStop = 0
                explorer.setJourneyStops(on: true)
                InteractionTest.key(v, "j", down: true)
                InteractionTest.key(v, "j", down: false)
                for _ in 0..<18 {
                    InteractionTest.key(v, "e", down: true)
                    InteractionTest.key(v, "e", down: false)
                }
            }
            let j = explorer.stats().journey
            if j.atStop && j.phase == "stopped" && stopSeen.isEmpty {
                stopSeen = j.title
                legAtStop = Int(j.leg)
                InteractionTest.key(v, "x", down: true)
                InteractionTest.key(v, "x", down: false)
            }
            once("esc", t >= 6.9) {
                let s = explorer.stats().journey
                let went = Int(s.leg) > legAtStop || s.phase == "planning"
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                explorer.setJourneyStops(on: v.model.journeyStops)
                let ok = !stopSeen.isEmpty && went && !explorer.stats().journey.active
                if !ok { self.failures += 1 }
                self.refineLines.append("journey stops: stopped at \"\(stopSeen)\" (leg \(legAtStop)), after X leg \(s.leg) \(s.phase) \"\(s.label)\" \(ok ? "OK" : "FAILED")")
            }
        }
        // π hunt (kb: decisions/controls "π meter"): the π tour's hunt step (the guided flight
        // with the meter and its orbit trail; the counting thread publishes the trail picture,
        // the frame uploads it) from the home view; Esc twice at 2.8 s. Checks the meter counted
        // at the neck, that one Esc only arms and a second ends the tour, and that the end
        // switched it off; the frame pacing and draw CPU are measured like every scenario.
        let piHunt: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            if i == 0 {
                sent.removeAll()
                v.model.startTour("pi")
                v.model.tourGo(2)
            }
            once("end", t >= 2.8) {
                let m = explorer.piMeter()
                // Esc twice ends a tour: the first only arms (the tour runs on), the second ends it.
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                let armed = v.model.tour != nil && v.model.tourEscapeArmed
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                let ended = v.model.tour == nil
                let off = !explorer.piMeter().enabled
                let ok = m.enabled && m.point == "neck" && m.count > 0 && armed && ended && off
                if !ok { self.failures += 1 }
                self.refineLines.append("pi hunt: \(m.point) ε \(String(format: "%.2g", m.eps)) N \(m.count) \(m.digits) digits, Esc armed \(armed) then ended \(ended), off after the tour \(off) \(ok ? "OK" : "FAILED")")
            }
        }
        // Full screen (kb: decisions/controls "Full screen") through a probe instead of the
        // window's own full screen (window tests never enter it): in "full screen", Z, then
        // Esc stops the autopilot and stays; a held Esc's repeats stay; once the stop has
        // eased out, Esc (nothing moving) leaves; F enters, F leaves. Plus the control
        // center's close-and-restore across full screen.
        var fsOn = false
        var fsToggles = 0
        var fsChecks: [String] = []
        let escFullScreen: (FractalNSView, Int, Double) -> Void = { v, i, t in
            guard let explorer = v.explorer else { return }
            @MainActor func esc(repeating: Bool = false) {
                InteractionTest.key(v, "\u{1b}", down: true, keyCode: 53, isARepeat: repeating)
                if !repeating { InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53) }
            }
            @MainActor func check(_ name: String, _ ok: Bool) {
                if !ok { self.failures += 1 }
                fsChecks.append("\(name) \(ok ? "OK" : "FAILED")")
            }
            if i == 0 {
                sent.removeAll()
                fsChecks = []
                fsOn = true
                fsToggles = 0
                v.fullScreenProbe = (isOn: { fsOn }, toggle: { fsOn.toggle(); fsToggles += 1 })
                v.send(.pointer(x: 0, y: 0))
                InteractionTest.key(v, "z", down: true)
                InteractionTest.key(v, "z", down: false)
            }
            once("esc", t >= 1.2) {
                let flying = explorer.stats().autopilot.active
                esc()
                check("Esc while flying stops it, stays in full screen", flying && fsOn && fsToggles == 0)
                for _ in 0..<5 { esc(repeating: true) }
                check("held Esc (repeats) stays", fsOn && fsToggles == 0)
            }
            once("esc2", t >= 2.4) {
                let moving = explorer.stats().autopilot.active || explorer.stats().flight.active
                esc()
                check("Esc with nothing moving (autopilot \(moving ? "still on" : "off")) leaves", !moving && !fsOn && fsToggles == 1)
                InteractionTest.key(v, "f", down: true)
                InteractionTest.key(v, "f", down: false)
                check("F enters", fsOn && fsToggles == 2)
                InteractionTest.key(v, "f", down: true, isARepeat: true)
                check("held F (repeat) does nothing", fsOn && fsToggles == 2)
                InteractionTest.key(v, "f", down: true)
                InteractionTest.key(v, "f", down: false)
                check("F leaves", !fsOn && fsToggles == 3)
                // Not in full screen: an Esc with nothing moving is the system's (unhandled).
                let e = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [],
                                         timestamp: ProcessInfo.processInfo.systemUptime,
                                         windowNumber: v.window?.windowNumber ?? 0, context: nil,
                                         characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}",
                                         isARepeat: false, keyCode: 53)
                check("Esc outside full screen passes to the system", (e.map { !v.handleKey($0) } ?? false) && fsToggles == 3)
                InteractionTest.key(v, "\u{1b}", down: false, keyCode: 53)
                v.fullScreenProbe = nil
                // The control center: closed in full screen, restored as it was on leaving.
                var sidebar = FullScreenSidebar()
                var shown = true
                sidebar.update(fullScreen: true, shown: &shown)
                let closed = !shown
                shown = true   // opened in full screen
                sidebar.update(fullScreen: true, shown: &shown)
                let stillClosed = !shown
                sidebar.update(fullScreen: false, shown: &shown)
                let restoredOpen = shown
                shown = false
                sidebar.update(fullScreen: true, shown: &shown)
                sidebar.update(fullScreen: false, shown: &shown)
                check("control center closes in full screen and is restored", closed && stillClosed && restoredOpen && !shown)
                self.refineLines.append("esc full screen: " + fsChecks.joined(separator: "; "))
            }
        }
        let seaDeep20 = View(re: "-0.743643887037158704752191506114774", im: "0.131825904205311970493132056385139", scale: 1e-20)
        // M5 benchmark views at their benchmark pixel size (1600 px wide framing; the window's
        // points are 2 px on a Retina display, so the scale per point is twice that).
        let dense = View(re: "-1.4080321340304542", im: "0.13680236432900417", scale: 2 * 7.384e-11 / 1600)
        let neck2e5 = View(re: "-0.7500016386523336", im: "-0.0021073424656852589", scale: 2 * 2.1666e-5 / 1600)
        let spiral = View(re: "-0.13415002160674054265664", im: "-0.64991906714433853974391", scale: 2 * 4.19804e-19)
        let siegel = View(re: "-0.39054087021839984", im: "0.5867879073469688", scale: 3.75e-3 / 1373)
        // F4 deep Julia sets (kb: plans/f4-deep-julia): the twin of the seahorse view at 1e-12
        // zoomed fast to ~1e-40 (the deep zoom above, as a Julia set of a deep k); a pan near a
        // preimage of 0 in the Douady rabbit at 1e-100 and a zoom at one in the dendrite (k = i)
        // at 1e-300 (step-1 gate views, the window's points 2 px).
        let rabbit100 = View(re: "0.13582194234832306384726920844974750963803955544158433695104985552061703718356220531423914345539064577159014155396788582297739",
                             im: "-0.98408937840354738561303053036751272330190031920474351403645897719626417827245062205230473648568576800988084871583518785926966",
                             scale: 2e-100)
        let dendrite300j = View(re: "-1.316217188613384182892301938270293124103893412616323208720307172274596124394581156543859132946211679304026231908661961024336819202086203360168036350362781617862595986112142227701026008880468481898614759175229768762325953986190536194031254729435187793227165530004224246648352068591889334276616386741309913104214101345554899367",
                                im: "0.6785478674150520237952461553430622991323380058277815194421221054868252650933002485440546541350165849932149350420951796822304685241394963385899335024961001636274825482729138595485370773903166706946136063947425419828687390342905631095799823421179334798563580045190217945069289191899700554854070505266217696214286174348262202054",
                                scale: 2e-300)
        let rabbitK = (re: "-0.1225611668766536", im: "0.7448617666197442")
        // The Mandelbrot panel of a deep twin (2e-32 per point at the Misiurewicz point c = i,
        // where the twin shows the dendrite): the twin zooms in; "panel late" switches the panel on as the zoom starts, so the panel's deep picture
        // (its own reference and chunks) renders during the measured frames; "no panel" is the
        // same zoom with the panel off.
        let dendrite32 = View(re: "0", im: "1", scale: 2e-32)
        let panelLateZoom: (FractalNSView, Int, Double) -> Void = { v, i, t in
            if i == 0 { _ = v.model.explorer?.toggleMandelbrotPanel() }
            zoom(v, i, t)
        }
        let buddhaHome = View(re: "-0.75", im: "0", scale: max(3.2 / 1280, 2.6 / 820), look: "buddhabrot")
        let nebulaSea = View(re: "-0.75", im: "0.1", scale: 3.75e-3 / 30, look: "nebulabrot")
        let multibrot5 = View(re: "0.84733531576585034", im: "0.58201571377117265", scale: 1.1e-8, formula: "multibrot5")
        let multibrot8 = View(re: "-0.578757811778842465396026167", im: "0.890676734071857679572348838",
                              scale: 5.2e-24, formula: "multibrot8")
        let mandelbar20 = View(re: "0.90314932019453019", im: "1.4984558966474309", scale: 4.75e-12, formula: "mandelbar")
        let mandelbar620 = View(re: "0.903149322150161032238953132749", im: "1.4984558964696706694156891274",
                                scale: 2.5e-25, formula: "mandelbar")
        return [Scenario(name: "orbit pointer", viewport: sea, input: orbitPointer),
                Scenario(name: "sweep", viewport: home, input: sweep(false)),
                Scenario(name: "julia sweep", viewport: home, input: sweep(true)),
                Scenario(name: "overlays pan", viewport: home, input: overlayPan(false)),
                Scenario(name: "julia pan", viewport: home, input: overlayPan(true)),
                Scenario(name: "orbit cloud", viewport: siegel, input: orbitCloud),
                Scenario(name: "overlays zoom", viewport: sea, input: overlayZoom),
                Scenario(name: "cosmic zoom", viewport: sea, input: cosmicZoom),
                Scenario(name: "home pan", viewport: home, input: pan),
                Scenario(name: "neck zoom", viewport: neck, input: zoom),
                Scenario(name: "wasd 1e5", viewport: sea, input: wasd),
                Scenario(name: "space zoom", viewport: sea, input: space),
                Scenario(name: "home zoom", viewport: home, input: zoom),
                Scenario(name: "1e5 pan", viewport: sea, input: pan),
                Scenario(name: "1e5 zoom", viewport: sea, input: zoom),
                Scenario(name: "limit zoom", viewport: limit, input: deep),
                Scenario(name: "pointer space", viewport: sea, input: pointerSpace),
                Scenario(name: "keyboard glide", viewport: sea, input: keyboardGlide),
                Scenario(name: "edge pan", viewport: sea, input: edgePan),
                Scenario(name: "click stream", viewport: sea, input: clickStream),
                Scenario(name: "deep zoom 1e-12 to 1e-40", viewport: seaDeep, input: deepZoom),
                Scenario(name: "1e-100 pan", viewport: dendrite100, input: pan),
                Scenario(name: "1e-300 zoom", viewport: dendrite300, input: zoom),
                Scenario(name: "autopilot", viewport: sea, input: autopilot),
                Scenario(name: "fly to", viewport: sea, input: flyTo),
                Scenario(name: "rotate zoom", viewport: sea, input: rotateZoom),
                Scenario(name: "cinematic autopilot", viewport: sea, input: cinematicAutopilot),
                Scenario(name: "compass", viewport: sea, input: compass),
                Scenario(name: "autopilot lock", viewport: sea, input: autopilotLock),
                Scenario(name: "journey", viewport: sea, input: journey),
                Scenario(name: "cruise out", viewport: sea, input: cruiseOut),
                // 7 s: the first leg (E × 10) eases into and out of its turn and onto the stop
                // (kb: decisions/controls "Journey", smooth legs: 2.4 s abrupt, now 3.3-5.1 s).
                Scenario(name: "journey stops", viewport: sea, input: journeyStops, seconds: 7.0),
                Scenario(name: "pi hunt", viewport: home, input: piHunt),
                Scenario(name: "esc full screen", viewport: sea, input: escFullScreen),
                Scenario(name: "looks home idle", viewport: home, input: switcher(gesture: false)),
                Scenario(name: "looks home pan", viewport: home, input: switcher(gesture: true)),
                Scenario(name: "looks deep idle", viewport: seaDeep20, input: switcher(gesture: false)),
                Scenario(name: "looks deep pan", viewport: seaDeep20, input: switcher(gesture: true)),
                // M5 (kb: techniques/local-series-approximation): local SA's views.
                Scenario(name: "dense pan", viewport: dense, input: pan),
                Scenario(name: "dense zoom", viewport: dense, input: zoom),
                Scenario(name: "neck2e5 pan", viewport: neck2e5, input: pan),
                Scenario(name: "sea-1e20 pan", viewport: seaDeep20, input: pan),
                Scenario(name: "sea-1e20 zoom", viewport: seaDeep20, input: zoom),
                Scenario(name: "spiral pan", viewport: spiral, input: pan),
                Scenario(name: "spiral zoom", viewport: spiral, input: zoom),
                Scenario(name: "julia twin deep zoom", viewport: seaDeep, input: deepZoom, julia: (re: "twin", im: "")),
                Scenario(name: "julia 1e-100 pan", viewport: rabbit100, input: pan, julia: rabbitK),
                Scenario(name: "julia 1e-300 zoom", viewport: dendrite300j, input: zoom, julia: (re: "0", im: "1")),
                Scenario(name: "julia twin 1e-32 zoom", viewport: dendrite32, input: zoom, julia: (re: "twin", im: "")),
                Scenario(name: "julia twin 1e-32 zoom panel late", viewport: dendrite32, input: panelLateZoom,
                         julia: (re: "twin", im: ""), mandelbrotPanel: false),
                Scenario(name: "julia twin 1e-32 zoom no panel", viewport: dendrite32, input: zoom,
                         julia: (re: "twin", im: ""), mandelbrotPanel: false),
                // F3 (kb: plans/f3-multibrot): z^5 + c at its seek-found period-11 minibrot, and
                // z^8 + c at its period-19 minibrot beside a Misiurewicz point (1e-24 per point).
                Scenario(name: "multibrot5 pan", viewport: multibrot5, input: pan),
                Scenario(name: "multibrot8 deep zoom", viewport: multibrot8, input: zoom),
                // BB (kb: plans/buddhabrot "Live mode"): the orbit-density looks stay on while
                // the view moves (a coarse live accumulation within the gesture budget).
                Scenario(name: "buddha pan", viewport: buddhaHome, input: pan),
                Scenario(name: "buddha zoom", viewport: buddhaHome, input: zoom),
                Scenario(name: "nebula zoom", viewport: nebulaSea, input: zoom),
                // F5 (kb: plans/f5-mandelbar): the Mandelbar set at its seek-found period-20
                // minibrot (5e-12 per point) and the period-620 one below it (2.5e-25 per point).
                Scenario(name: "mandelbar pan", viewport: mandelbar20, input: pan),
                Scenario(name: "mandelbar deep zoom", viewport: mandelbar620, input: zoom)]
    }()

    /// The complex coordinate at (x, y) view points from the centre.
    static func point(_ v: FractalNSView, _ x: Double, _ y: Double) -> (re: Double, im: Double)? {
        guard let vp = v.explorer?.viewport(), let re = Double(vp.re), let im = Double(vp.im) else { return nil }
        return (re + x * vp.scale, im + y * vp.scale)
    }

    /// The fixed-point property of the zoom target: how far the coordinate that was under
    /// (x, y) moved on screen, in pixels per second of zooming (must be < 1).
    private func checkDrift(_ name: String, _ v: FractalNSView, _ c0: (re: Double, im: Double)?, _ x: Double, _ y: Double,
                            seconds: Double, note: String) {
        guard let c0, let c1 = Self.point(v, x, y), let vp = v.explorer?.viewport(), seconds > 0.2 else {
            failures += 1
            refineLines.append("\(name): fixed point not measured (\(note)) FAILED")
            return
        }
        let px = hypot(c1.re - c0.re, c1.im - c0.im) / vp.scale * Double(v.window?.backingScaleFactor ?? 2)
        let rate = px / seconds
        let ok = rate < 1
        if !ok { failures += 1 }
        refineLines.append(String(format: "%@: zoom target drift %.2g px over %.2f s of zooming (%.2g px/s, ×%.3g zoom; %@) %@",
                                  name, px, seconds, rate, startScale(name) / vp.scale, note, ok ? "OK" : "FAILED"))
    }

    private func startScale(_ name: String) -> Double {
        scenarios.first { $0.name == name }?.viewport.scale ?? .nan
    }

    func attach(view: FractalNSView) {
        // ALMONDBREAD_SCENARIOS="home zoom,1e5 pan" runs a subset.
        if let only = ProcessInfo.processInfo.environment["ALMONDBREAD_SCENARIOS"] {
            let names = Set(only.split(separator: ",").map(String.init))
            scenarios = scenarios.filter { names.contains($0.name) }
        }
        guard self.view == nil else { return }
        self.view = view
        // Let the window appear and the first frame render.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { self.start() }
    }

    private func start() {
        guard let view, let window = view.window, view.explorer != nil else {
            print("interaction-test: no window or renderer")
            exit(1)
        }
        window.setContentSize(NSSize(width: 1280, height: 820))
        TestLaunch.placeAtScreenEdge(window)

        let fps = window.screen?.maximumFramesPerSecond ?? 60
        print("interaction-test: window key \(window.isKeyWindow), app active \(NSApp.isActive), "
              + "occlusion visible \(window.occlusionState.contains(.visible))")
        print("interaction-test: Rust renderer, window 1280x820 pt, backing scale \(window.backingScaleFactor), display \(window.screen?.localizedName ?? "?") \(fps) Hz (main: \(window.screen == NSScreen.main))")
        print("| scenario | frames | fps | link Hz (granted) | interval p50 / p95 / max ms | latency p50 / p95 ms (input→draw + draw→screen p50) | draw CPU p50 / p95 ms | GPU ms/frame (iter + colorize) | preview % | preview scale | skipped draws | draws / encoded |")
        print("|---|---:|---:|---|---|---|---|---|---:|---:|---:|---|")
        if ProcessInfo.processInfo.environment["ALMONDBREAD_TEST_LINK"] == "cv", let screen = window.screen {
            // A CVDisplayLink on the window's display drives both the view's frames and the
            // inputs: the CADisplayLink of a background (accessory, never key) window gets
            // ~30-60 callbacks per second whatever rate it asks for (kb: synthesis/benchmarks,
            // "120 Hz on ProMotion"); this shows what presentation achieves when frames are
            // offered at the panel's rate.
            startCVLink(screen: screen, view: view)
            print("interaction-test: frames and inputs driven by a CVDisplayLink on \(screen.localizedName)")
        } else {
            let link = view.displayLink(target: self, selector: #selector(step(_:)))
            link.preferredFrameRateRange = CAFrameRateRange(minimum: min(60, Float(fps)), maximum: Float(fps), preferred: Float(fps))
            view.setPreferredFrameRate(Float(fps))
            link.add(to: .main, forMode: .common)
            self.link = link
        }
        next()
    }

    private func next() {
        scenario += 1
        guard scenario < scenarios.count, let view, let explorer = view.explorer else {
            // (The look the test started with, also in the saved settings.)
            if let look = startLook, let view, look != view.model.lookID { view.model.applyLook(look) }
            link?.invalidate()
            if let cvLink { CVDisplayLinkStop(cvLink) }
            refineLines.forEach { print($0) }
            ffiLines.forEach { print($0) }
            print(failures == 0 ? "interaction-test: done" : "interaction-test: FAILED (\(failures) checks)")
            fflush(stdout)
            exit(failures == 0 ? 0 : 1)
        }
        let v = scenarios[scenario].viewport
        // The fractal: leave a Julia view (F4), set the scenario's formula (F3: a switch goes to
        // its whole set), its view, then open the Julia set where the scenario names one (its k
        // at the view, or the twin of the view).
        if explorer.settings().formula == "julia" { _ = explorer.backFromJulia() }
        // The scenario's look (BB), else the one the test started with.
        if startLook == nil { startLook = view.model.lookID }
        if let look = v.look ?? startLook, look != view.model.lookID { view.model.applyLook(look) }
        do {
            _ = try explorer.setFormula(formula: v.formula)
            try explorer.setViewport(re: v.re, im: v.im, scale: v.scale)
            if let k = scenarios[scenario].julia, k.re != "twin" {
                guard explorer.openJulia(re: k.re, im: k.im) else {
                    print("interaction-test: \(scenarios[scenario].name): bad Julia k")
                    exit(1)
                }
                try explorer.setViewport(re: v.re, im: v.im, scale: v.scale)
            }
        } catch {
            print("interaction-test: \(scenarios[scenario].name): \(error)")
            exit(1)
        }
        if scenarios[scenario].julia?.re == "twin", !explorer.juliaTwin() {
            print("interaction-test: \(scenarios[scenario].name): no Julia twin")
            exit(1)
        }
        if scenarios[scenario].julia != nil, !scenarios[scenario].mandelbrotPanel {
            _ = explorer.toggleMandelbrotPanel()
        }
        view.model.refreshStats()
        // Every scenario starts in pointer mode with the pointer off the canvas (the real
        // pointer is ignored during the test): zooms go to the centre unless a scenario
        // moves the synthetic pointer. Pointer travel switches back from keyboard mode.
        view.send(.pointerExited)
        view.send(.pointer(x: 0, y: 0))
        view.send(.pointer(x: 50, y: 0))
        view.send(.pointerExited)
        explorer.setEdgePan(mode: .whileZooming)
        // Overlays off unless a scenario turns them on (measures that off costs nothing).
        explorer.setOverlay(settings: OverlaySettings(orbit: false, orbitLength: 50, map: false, mapLevels: 3,
                                                      mapSize: 180, mapCorner: .bottomLeft, julia: false))
        explorer.setCosmicScale(settings: CosmicScaleSettings(enabled: false, metresPerPoint: 0))
        view.requestFrames()
        settling = true
        reported = false
        phaseStart = CACurrentMediaTime()
        tick = 0
    }

    private var cvLink: CVDisplayLink?
    private var cvPending = false

    private func startCVLink(screen: NSScreen, view: FractalNSView) {
        guard let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber else { return }
        var link: CVDisplayLink?
        CVDisplayLinkCreateWithCGDisplay(CGDirectDisplayID(number.uint32Value), &link)
        guard let link else { return }
        view.externalClock = true
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        let scale = Double(info.numer) / Double(info.denom) / 1e9
        CVDisplayLinkSetOutputHandler(link) { [weak self] _, _, output, _, _ in
            // Host time on CACurrentMediaTime's clock (mach absolute time in seconds).
            let t = Double(output.pointee.hostTime) * scale
            let period = output.pointee.videoRefreshPeriod > 0
                ? Double(output.pointee.videoRefreshPeriod) / Double(output.pointee.videoTimeScale) : 1.0 / 120
            DispatchQueue.main.async {
                guard let self, !self.cvPending else { return }
                self.cvPending = true
                defer { self.cvPending = false }
                self.view?.stepExternal(timestamp: t - period)
                self.tick(timestamp: t - period, duration: period)
            }
            return kCVReturnSuccess
        }
        CVDisplayLinkStart(link)
        cvLink = link
    }

    @objc private func step(_ link: CADisplayLink) {
        tick(timestamp: link.timestamp, duration: link.targetTimestamp - link.timestamp)
    }

    private func tick(timestamp: CFTimeInterval, duration: CFTimeInterval) {
        guard let view, let explorer = view.explorer, scenario < scenarios.count else { return }
        let now = CACurrentMediaTime()
        let name = scenarios[scenario].name
        if settling {
            // Settle: the view renders and refines without input, then measurement starts.
            if now - phaseStart > settleSeconds {
                settling = false
                phaseStart = now
                explorer.beginMeasurement()
                view.base.frameTimes = []
                linkTimes = []
                linkDurations = []
            }
            return
        }
        if now - phaseStart < scenarios[scenario].seconds {
            linkTimes.append(timestamp)
            linkDurations.append(duration)
            scenarios[scenario].input(view, tick, now - phaseStart)
            tick += 1
            return
        }
        if !reported {
            let r = explorer.endMeasurement()
            report(name, r)
            let swift = view.base.frameTimes ?? []
            view.base.frameTimes = nil
            if !swift.isEmpty {
                let mean = swift.reduce(0, +) / Double(swift.count) * 1000
                ffiLines.append(String(format: "%@: frame() %d calls, Swift-side mean %.3f ms, inside Rust %.3f ms: FFI + call %.1f µs per frame; skipped %d in flight + %d no drawable",
                                       name, swift.count, mean, r.drawCpuMean, (mean - r.drawCpuMean) * 1000,
                                       r.skippedInFlight, r.skippedNoTarget))
            }
            reported = true
            inputEnd = now
            let v = explorer.viewport()
            let s = explorer.stats()
            if name == "space zoom" {
                refineLines.append(String(format: "space zoom: zoom speed now %.2f e/s (E pressed once from 1.6), zoom %.3g, keys held %d",
                                          s.zoomSpeed, v.zoom, s.heldKeys))
            }
            if name == "wasd 1e5", let re = Double(v.re), let im = Double(v.im),
               let re0 = Double(scenarios[scenario].viewport.re), let im0 = Double(scenarios[scenario].viewport.im) {
                refineLines.append(String(format: "wasd 1e5: centre moved %.3g, %.3g view widths, keys held after release %d",
                                          (re - re0) / (v.scale * 1280), (im - im0) / (v.scale * 1280), s.heldKeys))
            }
            if s.heldKeys > 0 {
                refineLines.append("\(name): \(s.heldKeys) keys still held at input end (released)")
                view.send(.focusLost)
            }
            if name == "limit zoom" || name.hasPrefix("deep zoom") || name.hasPrefix("1e-") {
                refineLines.append(String(format: "%@: final scale %@ / pt (zoom %@), reference %llu pts%@, %u bit, %@ kernel",
                                          name, v.scaleText, Format.zoom(log10: v.zoomLog10), s.referenceLength,
                                          s.referencePeriod > 0 ? " (period \(s.referencePeriod))" : "",
                                          s.referenceBits, s.deep ? "deep" : "plain"))
            }
        }
        // After input stops, the fully refined full-resolution frame must arrive.
        // Measured from the last real input (keyboard scenarios release keys before the phase ends).
        let final = explorer.finalFrame()
        // Settled: refined, faded in and anti-aliased (FL's progressive AA), or 3 s at most.
        let settled = explorer.stats().settled || now - inputEnd > 3
        if final.time > final.lastInput && now - inputEnd > 0.05 && settled {
            refineLines.append(String(format: "%@: full-res %@ final frame %.0f ms after input stopped, settled (AA) %@",
                                      name, final.size, (final.time - final.lastInput) * 1000,
                                      explorer.stats().settled
                                        ? String(format: "%.0f ms", (now - final.lastInput) * 1000) : "NOT within 3 s"))
            // The shown frame must match a fresh headless render of the same view: no stale,
            // unfinished or misclassified pixels from the live pipeline (wedge bug, kb:
            // techniques/interior-detection, Pitfalls).
            if let m = explorer.displayedFrameMismatch() {
                let ok = m.percent <= 0.05
                if !ok { failures += 1 }
                refineLines.append(String(format: "%@: shown %dx%d frame vs fresh render: %d pixels (%.3f%%) differ in class %@",
                                          name, m.width, m.height, m.pixels, m.percent, ok ? "OK" : "FAILED"))
            }
            next()
        } else if now - inputEnd > 10 {
            failures += 1
            refineLines.append("\(name): NO full-res final frame within 10 s")
            next()
        }
    }

    /// A synthetic key event through the same handler the window's key monitor uses.
    static func key(_ view: FractalNSView, _ chars: String, down: Bool, keyCode: UInt16 = 0, isARepeat: Bool = false,
                    modifiers: NSEvent.ModifierFlags = []) {
        guard let e = NSEvent.keyEvent(with: down ? .keyDown : .keyUp, location: .zero, modifierFlags: modifiers,
                                       timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: view.window?.windowNumber ?? 0, context: nil,
                                       characters: chars, charactersIgnoringModifiers: chars,
                                       isARepeat: isARepeat, keyCode: keyCode) else { return }
        _ = view.handleKey(e)
    }

    /// A left click (mouse down and up, click count `count`) at (x, y) view points from the
    /// centre, through the view's own handlers.
    static func click(_ view: FractalNSView, _ x: Double, _ y: Double, count: Int) {
        let p = view.convert(NSPoint(x: view.bounds.midX + x, y: view.bounds.midY + y), to: nil)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            guard let e = NSEvent.mouseEvent(with: type, location: p, modifierFlags: [],
                                             timestamp: ProcessInfo.processInfo.systemUptime,
                                             windowNumber: view.window?.windowNumber ?? 0, context: nil,
                                             eventNumber: 0, clickCount: count, pressure: type == .leftMouseDown ? 1 : 0)
            else { return }
            if type == .leftMouseDown { view.mouseDown(with: e) } else { view.mouseUp(with: e) }
        }
    }

    private func report(_ name: String, _ r: InteractionReport) {
        let span = (linkTimes.last ?? 0) - (linkTimes.first ?? 0)
        let linkHz = linkTimes.count > 1 && span > 0 ? Double(linkTimes.count - 1) / span : 0
        let granted = linkDurations.sorted()
        let grantedHz = granted.isEmpty ? 0 : 1 / max(granted[granted.count / 2], 1e-6)
        print(String(format: "| %@ | %d | %.1f | %.1f (%.0f) | %.1f / %.1f / %.1f | %.1f / %.1f (%.1f + %.1f) | %.2f / %.2f | %.2f + %.2f | %.0f | %.2f | %d | %d / %d |",
                     name, r.presented, r.fps, linkHz, grantedHz, r.intervalP50, r.intervalP95, r.intervalMax,
                     r.latencyP50, r.latencyP95, r.inputToDrawP50, r.drawToPresentP50,
                     r.drawCpuP50, r.drawCpuP95, r.iterateGpuPerFrame, r.colorizeGpuPerFrame,
                     r.previewPct, r.previewScale, r.skipped, r.draws, r.encoded))
        if r.presented == 0 {
            let visible = view?.window?.occlusionState.contains(.visible) ?? false
            refineLines.append("\(name): no presentation times (window \(visible ? "visible" : "occluded")): frame rate and latency not measurable")
        }
        fflush(stdout)
    }
}
#endif
