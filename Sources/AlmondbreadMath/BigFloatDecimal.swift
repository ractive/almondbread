// Exact decimal parsing and correctly rounded decimal formatting for BigFloat
// (coordinates and bookmarks such as "-0.7436438870371587047...").

extension BigFloat {
    /// Parses `[+-]digits[.digits][(e|E)[+-]digits]` (surrounding whitespace allowed) and
    /// rounds the exact decimal value to nearest-even at `precisionBits`. Nil if malformed.
    public init?(_ text: String, precisionBits: Int) {
        var bytes = Array(text.utf8)
        while let f = bytes.first, f == 32 || f == 9 || f == 10 || f == 13 { bytes.removeFirst() }
        while let l = bytes.last, l == 32 || l == 9 || l == 10 || l == 13 { bytes.removeLast() }
        var i = 0
        var negative = false
        if i < bytes.count, bytes[i] == UInt8(ascii: "-") || bytes[i] == UInt8(ascii: "+") {
            negative = bytes[i] == UInt8(ascii: "-")
            i += 1
        }
        var digits: [UInt8] = []
        var fractionDigits = 0
        var sawDigit = false
        while i < bytes.count, bytes[i] >= 48, bytes[i] <= 57 {
            digits.append(bytes[i] - 48); sawDigit = true; i += 1
        }
        if i < bytes.count, bytes[i] == UInt8(ascii: ".") {
            i += 1
            while i < bytes.count, bytes[i] >= 48, bytes[i] <= 57 {
                digits.append(bytes[i] - 48); fractionDigits += 1; sawDigit = true; i += 1
            }
        }
        if !sawDigit { return nil }
        var exp10 = 0
        if i < bytes.count, bytes[i] == UInt8(ascii: "e") || bytes[i] == UInt8(ascii: "E") {
            i += 1
            var expNeg = false
            if i < bytes.count, bytes[i] == UInt8(ascii: "-") || bytes[i] == UInt8(ascii: "+") {
                expNeg = bytes[i] == UInt8(ascii: "-")
                i += 1
            }
            var sawExp = false
            var e = 0
            while i < bytes.count, bytes[i] >= 48, bytes[i] <= 57 {
                if e < 100_000_000 { e = e * 10 + Int(bytes[i] - 48) }
                sawExp = true; i += 1
            }
            if !sawExp { return nil }
            exp10 = expNeg ? -e : e
        }
        if i != bytes.count { return nil }
        // Strip leading zeros (they do not change the value) and trailing zeros (fold into exp).
        var start = 0
        while start < digits.count - 1, digits[start] == 0 { start += 1 }
        var end = digits.count
        var dec = exp10 - fractionDigits
        while end - start > 1, digits[end - 1] == 0 { end -= 1; dec += 1 }
        let d = BigNat.fromDecimalDigits(digits[start..<end])
        self.init(decimalSignificand: d, exponent10: dec, negative: negative, precisionBits: precisionBits)
    }

    /// value = ±D · 10^exponent10, correctly rounded.
    init(decimalSignificand d: [UInt64], exponent10: Int, negative: Bool, precisionBits: Int) {
        let n = BigFloat.limbCount(forBits: precisionBits)
        if d.isEmpty { self.init(zeroLimbs: n); return }
        if exponent10 >= 0 {
            let v = BigNat.mul(d, BigNat.pow10(exponent10))
            self.init(negative: negative, integerSignificand: v, exponent: 0, precisionBits: precisionBits)
            return
        }
        // D / 10^k: scale D by 2^s so the quotient has at least 64n + 2 bits, then round with
        // the remainder as sticky bit.
        let t = BigNat.pow10(-exponent10)
        let s = max(0, 64 * n + 2 + BigNat.bitLength(t) - BigNat.bitLength(d) + 1)
        let (q, r) = BigNat.divMod(BigNat.shiftLeft(d, s), t)
        self.init(zeroLimbs: n)
        var qq = q
        let bl = qq.withUnsafeMutableBufferPointer { qp in
            limbs.withUnsafeMutableBufferPointer { lp in
                roundWide(qp.baseAddress!, qp.count, stickyIn: !r.isEmpty, n, lp.baseAddress!)
            }
        }
        isNegative = negative
        exponent = bl - s
    }

    /// Significant decimal digits that guarantee a round trip at this precision.
    public var roundTripDigits: Int {
        // ceil(p * log10(2)) + 1
        (precisionBits * 30103 + 99_999) / 100_000 + 1
    }

    /// Decimal string with `significantDigits` significant digits (default: round-trip),
    /// rounded half-to-even, trailing zeros removed. Positional notation for decimal
    /// exponents -7 < x < 21, otherwise scientific (`1.25e-300`).
    public func toDecimalString(significantDigits: Int? = nil) -> String {
        if isZero { return "0" }
        let digitsWanted = max(1, significantDigits ?? roundTripDigits)
        let (_, sig, e2) = exactParts
        let m = BigNat.trimmed(sig)
        // Estimate floor(log10 |v|).
        var x10 = Int((log2Magnitude * 0.30102999566398120).rounded(.down))
        let upper = BigNat.pow10(digitsWanted)
        let lower = BigNat.pow10(digitsWanted - 1)
        var nDigits: [UInt64] = []
        while true {
            let k = digitsWanted - 1 - x10
            var num = m
            var den: [UInt64] = [1]
            if k >= 0 { num = BigNat.mul(num, BigNat.pow10(k)) } else { den = BigNat.pow10(-k) }
            if e2 >= 0 { num = BigNat.shiftLeft(num, e2) } else { den = BigNat.shiftLeft(den, -e2) }
            var (q, r) = BigNat.divMod(num, den)
            // Fix x10 = floor(log10 |v|) on the truncated quotient, then round half to even.
            if BigNat.compare(q, upper) >= 0 { x10 += 1; continue }
            if BigNat.compare(q, lower) < 0 { x10 -= 1; continue }
            let twice = BigNat.shiftLeft(r, 1)
            let c = BigNat.compare(twice, den)
            if c > 0 || (c == 0 && (q.first ?? 0) & 1 == 1) { q = BigNat.add(q, [1]) }
            if BigNat.compare(q, upper) == 0 { q = lower; x10 += 1 }
            nDigits = q
            break
        }
        var s = Array(BigNat.decimalString(nDigits))
        while s.count > 1, s.last == "0" { s.removeLast() }
        var out = isNegative ? "-" : ""
        if x10 > -7 && x10 < 21 {
            if x10 >= 0 {
                let intCount = x10 + 1
                if s.count <= intCount {
                    out += String(s) + String(repeating: "0", count: intCount - s.count)
                } else {
                    out += String(s[0..<intCount]) + "." + String(s[intCount...])
                }
            } else {
                out += "0." + String(repeating: "0", count: -x10 - 1) + String(s)
            }
        } else {
            out += String(s[0])
            if s.count > 1 { out += "." + String(s[1...]) }
            out += x10 < 0 ? "e-\(-x10)" : "e+\(x10)"
        }
        return out
    }
}
