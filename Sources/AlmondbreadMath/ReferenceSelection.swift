// Reference selection: period detection, Newton's method for minibrot nuclei, size
// estimates, and the "best reference for this view" policy (kb: techniques/reference-selection).
//
// All high-precision loops run on RawFloat registers (no allocation per iteration); radii and
// derivatives that only need a few digits but huge range use DoubleExp.

// MARK: - Shared raw helpers

/// Extended-range value of a register.
@inline(__always) func rawDoubleExp(_ a: RawFloat, _ n: Int) -> DoubleExp {
    if a.zero { return .zero }
    var mant = rawDoubleMantissa(a, n)
    var e = a.exp
    if mant == 1 { mant = 0.5; e &+= 1 }
    return DoubleExp(mantissa: a.neg ? -mant : mant, exponent: Int64(e))
}

/// |x + iy| in extended range.
@inline(__always) func rawComplexMagnitude(_ x: RawFloat, _ y: RawFloat, _ n: Int) -> DoubleExp {
    DoubleExpComplex(re: rawDoubleExp(x, n), im: rawDoubleExp(y, n)).magnitude
}

/// (x, y) <- (x + iy)^2 + (cx + i cy), multiplication form (relative accuracy per part).
/// Uses t0...t3 as temporaries.
@inline(__always)
func rawComplexSquareAdd(_ x: inout RawFloat, _ y: inout RawFloat, _ cx: RawFloat, _ cy: RawFloat,
                         _ t0: inout RawFloat, _ t1: inout RawFloat, _ t2: inout RawFloat, _ t3: inout RawFloat,
                         _ n: Int, _ s: Limbs) {
    rawAdd(&t0, x, y, subtract: false, n, s)
    rawAdd(&t1, x, y, subtract: true, n, s)
    rawMul(&t2, t0, t1, n, s)
    rawMul(&t3, x, y, n, s)
    if !t3.zero { t3.exp &+= 1 }
    rawAdd(&x, t2, cx, subtract: false, n, s)
    rawAdd(&y, t3, cy, subtract: false, n, s)
}

// MARK: - Period detection

public enum PeriodDetection {
    public enum Method: Sendable {
        /// Ball arithmetic: R_{n+1} = 2|z_n| R_n + R_n^2 + r, first n with |z_n| < R_n.
        /// Rigorous enclosure of the disc's image (ignoring rounding), so it never misses the
        /// lowest period but can report a slightly lower one in skewed places.
        case ball
        /// First-order ball: R_n = |dz_n/dc| r. Cheaper bound, closer to KF's default.
        case ballFirstOrder
        /// Box (Munafo): first n at which the image of the square's four corners winds
        /// around 0. Four orbits per iteration.
        case box
    }

    /// Lowest period of a minibrot (mu-atom) found inside the disc or box of radius `radius`
    /// around `center`, or nil if none is found up to `maxPeriod` (or the region escapes).
    /// Runs at the center's precision.
    public static func findPeriod(center: BigComplex, radius: BigFloat, maxPeriod: Int,
                                  method: Method = .ball, checkInterval: Int = 4096,
                                  isCancelled: () -> Bool = { false }) -> Int? {
        switch method {
        case .ball, .ballFirstOrder:
            return ballPeriod(center: center, radius: radius.doubleExp.magnitude, maxPeriod: maxPeriod,
                              firstOrder: method == .ballFirstOrder, checkInterval: checkInterval,
                              isCancelled: isCancelled)
        case .box:
            return boxPeriod(center: center, halfSize: radius, maxPeriod: maxPeriod,
                             checkInterval: checkInterval, isCancelled: isCancelled)
        }
    }

    static func ballPeriod(center: BigComplex, radius r: DoubleExp, maxPeriod: Int, firstOrder: Bool,
                           checkInterval: Int, isCancelled: () -> Bool) -> Int? {
        let n = BigFloat.limbCount(forBits: center.precisionBits)
        let ws = RawWorkspace(limbs: n, registers: 8)
        var x = ws.register(0), y = ws.register(1), cx = ws.register(2), cy = ws.register(3)
        var t0 = ws.register(4), t1 = ws.register(5), t2 = ws.register(6), t3 = ws.register(7)
        ws.load(&cx, center.re)
        ws.load(&cy, center.im)
        let s = ws.scratch
        var radiusAcc = DoubleExp.zero           // R_n
        var dz = DoubleExpComplex.zero           // dz_n/dc
        let two = DoubleExp(2.0)
        let escape = DoubleExp(1e10)
        var zAbs = DoubleExp.zero                // |z_{n-1}|
        for p in 1...max(1, maxPeriod) {
            if firstOrder {
                // dz_p = 2 z_{p-1} dz_{p-1} + 1
                let z = DoubleExpComplex(re: rawDoubleExp(x, n), im: rawDoubleExp(y, n))
                dz = (z * dz).timesTwo + .one
            } else {
                radiusAcc = two * zAbs * radiusAcc + radiusAcc * radiusAcc + r
            }
            rawComplexSquareAdd(&x, &y, cx, cy, &t0, &t1, &t2, &t3, n, s)
            zAbs = rawComplexMagnitude(x, y, n)
            let bound = firstOrder ? dz.magnitude * r : radiusAcc
            if zAbs < bound { return p }
            if escape < zAbs - bound { return nil }   // the whole disc escaped
            if !firstOrder && escape < radiusAcc { return nil }
            if p % checkInterval == 0 && isCancelled() { return nil }
        }
        withExtendedLifetime(ws) {}
        return nil
    }

    static func boxPeriod(center: BigComplex, halfSize: BigFloat, maxPeriod: Int,
                          checkInterval: Int, isCancelled: () -> Bool) -> Int? {
        let n = BigFloat.limbCount(forBits: center.precisionBits)
        let h = halfSize.magnitude
        // Corners in counter-clockwise order.
        let corners = [
            BigComplex(re: center.re - h, im: center.im - h),
            BigComplex(re: center.re + h, im: center.im - h),
            BigComplex(re: center.re + h, im: center.im + h),
            BigComplex(re: center.re - h, im: center.im + h),
        ]
        let ws = RawWorkspace(limbs: n, registers: 4 * 4 + 6)
        var zx = (0..<4).map { ws.register($0) }
        var zy = (0..<4).map { ws.register(4 + $0) }
        var cx = (0..<4).map { ws.register(8 + $0) }
        var cy = (0..<4).map { ws.register(12 + $0) }
        for k in 0..<4 {
            ws.load(&cx[k], corners[k].re)
            ws.load(&cy[k], corners[k].im)
        }
        var t0 = ws.register(16), t1 = ws.register(17), t2 = ws.register(18), t3 = ws.register(19)
        var p1 = ws.register(20), p2 = ws.register(21)
        let s = ws.scratch
        for p in 1...max(1, maxPeriod) {
            var escaped = false
            for k in 0..<4 {
                var x = zx[k], y = zy[k]
                rawComplexSquareAdd(&x, &y, cx[k], cy[k], &t0, &t1, &t2, &t3, n, s)
                zx[k] = x; zy[k] = y
                if x.exp > 20 || y.exp > 20 { escaped = true }
            }
            if escaped { return nil }
            // Crossing-number test of the origin against the quadrilateral z0 z1 z2 z3.
            var inside = false
            for k in 0..<4 {
                let a = k, b = (k + 1) & 3
                let aUp = !zy[a].zero && !zy[a].neg
                let bUp = !zy[b].zero && !zy[b].neg
                if aUp == bUp { continue }
                // The edge crosses the positive real axis iff cross(a, b) has the sign of b.y.
                rawMul(&p1, zx[a], zy[b], n, s)
                rawMul(&p2, zy[a], zx[b], n, s)
                let c = rawCompare(p1, p2, n)
                if c != 0 && (c > 0) == bUp { inside.toggle() }
            }
            if inside { return p }
            if p % checkInterval == 0 && isCancelled() { return nil }
        }
        withExtendedLifetime(ws) {}
        return nil
    }

    /// Atom-domain period: the n >= 1 at which |z_n| reaches its last new minimum before
    /// escape or `maxIterations` (Peitgen & Richter; kb: techniques/reference-selection).
    public static func atomDomainPeriod(c: BigComplex, maxIterations: Int) -> Int {
        let n = BigFloat.limbCount(forBits: c.precisionBits)
        let ws = RawWorkspace(limbs: n, registers: 8)
        var x = ws.register(0), y = ws.register(1), cx = ws.register(2), cy = ws.register(3)
        var t0 = ws.register(4), t1 = ws.register(5), t2 = ws.register(6), t3 = ws.register(7)
        ws.load(&cx, c.re)
        ws.load(&cy, c.im)
        var best = DoubleExp(1e300)
        var period = 1
        for i in 1...max(1, maxIterations) {
            rawComplexSquareAdd(&x, &y, cx, cy, &t0, &t1, &t2, &t3, n, ws.scratch)
            let a = rawComplexMagnitude(x, y, n)
            if a < best { best = a; period = i }
            if DoubleExp(2.0) < a { break }
        }
        withExtendedLifetime(ws) {}
        return period
    }
}

extension DoubleExpComplex {
    var timesTwo: DoubleExpComplex { DoubleExpComplex(re: re.timesPowerOfTwo(1), im: im.timesPowerOfTwo(1)) }
}

// MARK: - Newton's method for nuclei

public enum NucleusFinder {
    public struct Result: Sendable {
        public var nucleus: BigComplex
        public var period: Int
        /// Newton steps taken.
        public var steps: Int
        /// log2 |last Newton step| (-infinity if the last step was exactly zero).
        public var lastStepLog2: Double
    }

    /// Solves z_p(c) = 0 by Newton's method from `start`, at the start point's precision
    /// (`precisionBits` overrides). Each step iterates z <- z^2 + c, dz <- 2 z dz + 1 for p
    /// steps and sets c <- c - z/dz. Converges quadratically; stops when the step is within
    /// 16 bits of the working precision or stops shrinking near it. Returns nil if it does
    /// not converge in `maxSteps`, if dz vanishes, or if c moves farther than `maxDistance`
    /// from the start (when given).
    public static func findNucleus(near start: BigComplex, period p: Int, precisionBits: Int? = nil,
                                   maxSteps: Int = 64, maxDistance: BigFloat? = nil,
                                   isCancelled: () -> Bool = { false }) -> Result? {
        precondition(p >= 1)
        let bits = BigFloat.limbCount(forBits: precisionBits ?? start.precisionBits) * 64
        let n = bits / 64
        var c = start.withPrecision(bits: bits)
        let ws = RawWorkspace(limbs: n, registers: 14)
        var zx = ws.register(0), zy = ws.register(1), dx = ws.register(2), dy = ws.register(3)
        var cx = ws.register(4), cy = ws.register(5)
        var t0 = ws.register(6), t1 = ws.register(7), t2 = ws.register(8), t3 = ws.register(9)
        var u0 = ws.register(10), u1 = ws.register(11), one = ws.register(12)
        let s = ws.scratch
        rawSetDouble(&one, 1.0, n)
        var previousStepLog2 = Double.infinity
        for step in 1...maxSteps {
            if isCancelled() { return nil }
            ws.load(&cx, c.re)
            ws.load(&cy, c.im)
            rawSetZero(&zx, n); rawSetZero(&zy, n); rawSetZero(&dx, n); rawSetZero(&dy, n)
            for _ in 0..<p {
                // dz <- 2 z dz + 1 (uses the old z)
                rawMul(&t0, zx, dx, n, s)
                rawMul(&t1, zy, dy, n, s)
                rawAdd(&u0, t0, t1, subtract: true, n, s)
                rawMul(&t0, zx, dy, n, s)
                rawMul(&t1, zy, dx, n, s)
                rawAdd(&u1, t0, t1, subtract: false, n, s)
                if !u0.zero { u0.exp &+= 1 }
                if !u1.zero { u1.exp &+= 1 }
                rawAdd(&dx, u0, one, subtract: false, n, s)
                rawCopy(&dy, u1, n)
                // z <- z^2 + c
                rawComplexSquareAdd(&zx, &zy, cx, cy, &t0, &t1, &t2, &t3, n, s)
            }
            if dx.zero && dy.zero { return nil }
            let z = BigComplex(re: ws.store(zx), im: ws.store(zy))
            let dz = BigComplex(re: ws.store(dx), im: ws.store(dy))
            let delta = z / dz
            c = c - delta
            let stepLog2 = delta.re.isZero && delta.im.isZero ? -Double.infinity
                : max(delta.re.log2Magnitude, delta.im.log2Magnitude)
            if let md = maxDistance {
                let off = c - start
                if max(off.re.log2Magnitude, off.im.log2Magnitude) > md.log2Magnitude { return nil }
            }
            let cLog2 = max(c.re.log2Magnitude, c.im.log2Magnitude, -Double(bits))
            let floor = cLog2 - Double(bits) + 16
            if stepLog2 <= floor
                || (stepLog2 < cLog2 - Double(bits) / 2 && stepLog2 > previousStepLog2 - 1) {
                withExtendedLifetime(ws) {}
                return Result(nucleus: c, period: p, steps: step, lastStepLog2: stepLog2)
            }
            previousStepLog2 = stepLog2
        }
        return nil
    }

    /// Size estimate of the minibrot with nucleus c and period p (relative to the main
    /// cardioid's size 1): 1 / (b l^2) with l = prod 2 z_i, b = 1 + sum 1/l_i over
    /// i = 1 ..< p (mathr's estimate; kb: techniques/reference-selection). Returns |size|.
    public static func minibrotSize(nucleus c: BigComplex, period p: Int) -> DoubleExp {
        minibrotSizeComplex(nucleus: c, period: p).magnitude
    }

    /// Complex size estimate: its magnitude is the minibrot's scale relative to the whole set,
    /// its argument the minibrot's rotation (a point w of the whole set maps to c + size * w).
    public static func minibrotSizeComplex(nucleus c: BigComplex, period p: Int) -> DoubleExpComplex {
        let n = BigFloat.limbCount(forBits: c.precisionBits)
        let ws = RawWorkspace(limbs: n, registers: 8)
        var x = ws.register(0), y = ws.register(1), cx = ws.register(2), cy = ws.register(3)
        var t0 = ws.register(4), t1 = ws.register(5), t2 = ws.register(6), t3 = ws.register(7)
        ws.load(&cx, c.re)
        ws.load(&cy, c.im)
        var l = DoubleExpComplex.one
        var b = DoubleExpComplex.one
        if p > 1 {
            for _ in 1..<p {
                rawComplexSquareAdd(&x, &y, cx, cy, &t0, &t1, &t2, &t3, n, ws.scratch)
                let z = DoubleExpComplex(re: rawDoubleExp(x, n), im: rawDoubleExp(y, n))
                l = (z * l).timesTwo
                if l.re.isZero && l.im.isZero { return .zero }
                b = b + DoubleExpComplex.one / l
            }
        }
        withExtendedLifetime(ws) {}
        return DoubleExpComplex.one / (b * l * l)
    }
}

// MARK: - Reference selection policy

public enum ReferenceSelector {
    public struct Choice: Sendable {
        /// The reference point C, at the requested precision.
        public var point: BigComplex
        /// The nucleus period when `isNucleus` (the orbit then only needs `period` iterations
        /// and Z_period = 0); nil for the view centre.
        public var period: Int?
        public var isNucleus: Bool
        /// Minibrot size estimate for a nucleus.
        public var minibrotSize: DoubleExp?
        public var newtonSteps: Int
        /// The period the detector reported, also when Newton failed or its nucleus lay
        /// outside the view (then `isNucleus` is false and `point` is the centre).
        public var detectedPeriod: Int?
    }

    /// Picks the reference for a view: the nucleus of the lowest-period minibrot found in the
    /// disc of radius `radius` around `center` (use the view's half-diagonal) if Newton
    /// converges to a point inside that disc, else the centre itself.
    /// Run it on a background queue; cost is about (period detection: up to maxIterations
    /// complex squarings) + (Newton: ~10 steps of period-p iterations with derivative).
    public static func selectReference(center: BigComplex, radius: BigFloat, maxIterations: Int,
                                       precisionBits: Int? = nil,
                                       method: PeriodDetection.Method = .ball,
                                       isCancelled: () -> Bool = { false }) -> Choice {
        let bits = BigFloat.limbCount(forBits: precisionBits ?? center.precisionBits) * 64
        let c0 = center.withPrecision(bits: bits)
        var fallback = Choice(point: c0, period: nil, isNucleus: false, minibrotSize: nil, newtonSteps: 0,
                              detectedPeriod: nil)
        guard let p = PeriodDetection.findPeriod(center: c0, radius: radius, maxPeriod: maxIterations,
                                                 method: method, isCancelled: isCancelled) else {
            return fallback
        }
        fallback.detectedPeriod = p
        guard let nr = NucleusFinder.findNucleus(near: c0, period: p, precisionBits: bits,
                                                 maxDistance: radius.timesPowerOfTwo(2),
                                                 isCancelled: isCancelled) else {
            return fallback
        }
        let off = nr.nucleus - c0
        let dist = off.magnitudeDoubleExp
        if radius.doubleExp.magnitude < dist { return fallback }
        let size = NucleusFinder.minibrotSize(nucleus: nr.nucleus, period: p)
        return Choice(point: nr.nucleus, period: p, isNucleus: true, minibrotSize: size, newtonSteps: nr.steps,
                      detectedPeriod: p)
    }
}
