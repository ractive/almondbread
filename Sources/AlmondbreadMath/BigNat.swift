// Arbitrary-size natural numbers as little-endian [UInt64] arrays, trimmed (no leading zero
// limbs; zero is []). Used only off the hot path: decimal parsing and formatting, division.

@usableFromInline
enum BigNat {
    @usableFromInline typealias N = [UInt64]

    static func trimmed(_ a: N) -> N {
        var a = a
        while let last = a.last, last == 0 { a.removeLast() }
        return a
    }

    static func bitLength(_ a: N) -> Int {
        guard let last = a.last else { return 0 }
        return (a.count - 1) * 64 + (64 - last.leadingZeroBitCount)
    }

    static func compare(_ a: N, _ b: N) -> Int {
        if a.count != b.count { return a.count < b.count ? -1 : 1 }
        var i = a.count - 1
        while i >= 0 {
            if a[i] != b[i] { return a[i] < b[i] ? -1 : 1 }
            i -= 1
        }
        return 0
    }

    /// a * m + add
    static func mulSmall(_ a: N, _ m: UInt64, add: UInt64 = 0) -> N {
        var r = N(repeating: 0, count: a.count + 1)
        var carry = add
        for i in 0..<a.count {
            let (h, l) = a[i].multipliedFullWidth(by: m)
            let (s, o) = l.addingReportingOverflow(carry)
            r[i] = s
            carry = h &+ (o ? 1 : 0)
        }
        r[a.count] = carry
        return trimmed(r)
    }

    /// (a / d, a % d) for a single limb d > 0.
    static func divSmall(_ a: N, _ d: UInt64) -> (N, UInt64) {
        var q = N(repeating: 0, count: a.count)
        var rem: UInt64 = 0
        var i = a.count - 1
        while i >= 0 {
            let (qq, rr) = d.dividingFullWidth((rem, a[i]))
            q[i] = qq
            rem = rr
            i -= 1
        }
        return (trimmed(q), rem)
    }

    static func add(_ a: N, _ b: N) -> N {
        let (x, y) = a.count >= b.count ? (a, b) : (b, a)
        var r = x + [0]
        var carry: UInt64 = 0
        for i in 0..<x.count {
            let yi: UInt64 = i < y.count ? y[i] : 0
            let (s0, o0) = r[i].addingReportingOverflow(yi)
            let (s1, o1) = s0.addingReportingOverflow(carry)
            r[i] = s1
            carry = (o0 ? 1 : 0) + (o1 ? 1 : 0)
        }
        r[x.count] = carry
        return trimmed(r)
    }

    /// a - b, requires a >= b.
    static func sub(_ a: N, _ b: N) -> N {
        var r = a
        var borrow: UInt64 = 0
        for i in 0..<a.count {
            let bi: UInt64 = i < b.count ? b[i] : 0
            let (s0, o0) = r[i].subtractingReportingOverflow(bi)
            let (s1, o1) = s0.subtractingReportingOverflow(borrow)
            r[i] = s1
            borrow = (o0 ? 1 : 0) + (o1 ? 1 : 0)
        }
        precondition(borrow == 0, "BigNat.sub underflow")
        return trimmed(r)
    }

    static func mul(_ a: N, _ b: N) -> N {
        if a.isEmpty || b.isEmpty { return [] }
        // Pad both to the same length and use the limb kernels (Karatsuba when large).
        let n = max(a.count, b.count)
        var x = a + N(repeating: 0, count: n - a.count)
        var y = b + N(repeating: 0, count: n - b.count)
        var r = N(repeating: 0, count: 2 * n)
        var s = N(repeating: 0, count: productScratchLimbs(n) + 1)
        x.withUnsafeMutableBufferPointer { xp in
            y.withUnsafeMutableBufferPointer { yp in
                r.withUnsafeMutableBufferPointer { rp in
                    s.withUnsafeMutableBufferPointer { sp in
                        limbsMul(rp.baseAddress!, xp.baseAddress!, yp.baseAddress!, n, sp.baseAddress!)
                    }
                }
            }
        }
        return trimmed(r)
    }

    static func shiftLeft(_ a: N, _ bits: Int) -> N {
        if a.isEmpty || bits == 0 { return a }
        let ls = bits / 64, bs = bits % 64
        var r = N(repeating: 0, count: ls) + a + [0]
        if bs > 0 {
            r.withUnsafeMutableBufferPointer { p in
                _ = limbsShiftLeft(p.baseAddress! + ls, p.baseAddress! + ls, a.count + 1, bs)
            }
        }
        return trimmed(r)
    }

    /// floor(a / 2^bits)
    static func shiftRight(_ a: N, _ bits: Int) -> N {
        if bits == 0 { return a }
        let ls = bits / 64, bs = bits % 64
        if ls >= a.count { return [] }
        var r = Array(a[ls...])
        if bs > 0 {
            r.withUnsafeMutableBufferPointer { p in
                _ = limbsShiftRight(p.baseAddress!, p.baseAddress!, p.count, bs)
            }
        }
        return trimmed(r)
    }

    static func isBitSet(_ a: N, _ pos: Int) -> Bool {
        let l = pos / 64
        if pos < 0 || l >= a.count { return false }
        return (a[l] >> UInt64(pos % 64)) & 1 != 0
    }

    static func anyBitBelow(_ a: N, _ pos: Int) -> Bool {
        if pos <= 0 { return false }
        let full = min(pos / 64, a.count)
        for i in 0..<full where a[i] != 0 { return true }
        let rem = pos % 64
        if rem != 0 && full < a.count {
            return a[full] & ((UInt64(1) << UInt64(rem)) - 1) != 0
        }
        return false
    }

    static func pow10(_ k: Int) -> N {
        precondition(k >= 0)
        var result: N = [1]
        var base: N = [10]
        var e = k
        while e > 0 {
            if e & 1 != 0 { result = mul(result, base) }
            e >>= 1
            if e > 0 { base = mul(base, base) }
        }
        return result
    }

    /// Knuth algorithm D: (u / v, u % v), v != 0.
    static func divMod(_ u: N, _ v: N) -> (N, N) {
        let v = trimmed(v), u = trimmed(u)
        precondition(!v.isEmpty, "division by zero")
        if compare(u, v) < 0 { return ([], u) }
        if v.count == 1 {
            let (q, r) = divSmall(u, v[0])
            return (q, r == 0 ? [] : [r])
        }
        let n = v.count, m = u.count - n
        let s = v[n - 1].leadingZeroBitCount
        // Normalise so the divisor's top bit is set.
        var vn = v
        var un = u + [0]
        if s > 0 {
            vn.withUnsafeMutableBufferPointer { p in _ = limbsShiftLeft(p.baseAddress!, p.baseAddress!, n, s) }
            un.withUnsafeMutableBufferPointer { p in _ = limbsShiftLeft(p.baseAddress!, p.baseAddress!, m + n + 1, s) }
        }
        var q = N(repeating: 0, count: m + 1)
        let vTop = vn[n - 1], vNext = vn[n - 2]
        var j = m
        while j >= 0 {
            var qhat: UInt64
            var rhat: UInt64
            var rhatOverflow = false
            if un[j + n] >= vTop {
                qhat = UInt64.max
                let (r, o) = un[j + n - 1].addingReportingOverflow(vTop)
                rhat = r
                rhatOverflow = o || un[j + n] > vTop
            } else {
                (qhat, rhat) = vTop.dividingFullWidth((un[j + n], un[j + n - 1]))
            }
            while !rhatOverflow {
                let (ph, pl) = qhat.multipliedFullWidth(by: vNext)
                if ph > rhat || (ph == rhat && pl > un[j + n - 2]) {
                    qhat -= 1
                    let (r, o) = rhat.addingReportingOverflow(vTop)
                    rhat = r
                    rhatOverflow = o
                } else {
                    break
                }
            }
            // un[j...j+n] -= qhat * vn
            let borrow: UInt64 = un.withUnsafeMutableBufferPointer { up in
                vn.withUnsafeMutableBufferPointer { vp in
                    let b = limbsSubMul1(up.baseAddress! + j, vp.baseAddress!, n, qhat)
                    let (t, o) = up[j + n].subtractingReportingOverflow(b)
                    up[j + n] = t
                    return o ? 1 : 0
                }
            }
            if borrow != 0 {
                qhat -= 1
                un.withUnsafeMutableBufferPointer { up in
                    vn.withUnsafeMutableBufferPointer { vp in
                        let c = limbsAdd(up.baseAddress! + j, up.baseAddress! + j, vp.baseAddress!, n)
                        up[j + n] = up[j + n] &+ c
                    }
                }
            }
            q[j] = qhat
            j -= 1
        }
        var r = Array(un[0..<n])
        if s > 0 {
            r.withUnsafeMutableBufferPointer { p in _ = limbsShiftRight(p.baseAddress!, p.baseAddress!, n, s) }
        }
        return (trimmed(q), trimmed(r))
    }

    /// Decimal digits of a (no sign, "0" for zero).
    static func decimalString(_ a: N) -> String {
        if a.isEmpty { return "0" }
        var chunks: [UInt64] = []
        var x = a
        let base: UInt64 = 10_000_000_000_000_000_000 // 10^19
        while !x.isEmpty {
            let (q, r) = divSmall(x, base)
            chunks.append(r)
            x = q
        }
        var s = String(chunks.last!)
        for c in chunks.dropLast().reversed() {
            let part = String(c)
            s += String(repeating: "0", count: 19 - part.count) + part
        }
        return s
    }

    /// Parses a run of decimal digits.
    static func fromDecimalDigits<S: Sequence>(_ digits: S) -> N where S.Element == UInt8 {
        var r: N = []
        var chunk: UInt64 = 0
        var chunkLen = 0
        for d in digits {
            chunk = chunk * 10 + UInt64(d)
            chunkLen += 1
            if chunkLen == 19 {
                r = mulSmall(r, 10_000_000_000_000_000_000, add: chunk)
                chunk = 0
                chunkLen = 0
            }
        }
        if chunkLen > 0 {
            var p: UInt64 = 1
            for _ in 0..<chunkLen { p *= 10 }
            r = mulSmall(r, p, add: chunk)
        }
        return trimmed(r)
    }
}
