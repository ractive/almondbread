// Allocation-free binary floating point on caller-owned limb storage.
//
// A RawFloat is a header (sign, exponent, zero flag) plus a pointer to n limbs. The value is
//     (-1)^neg * M / 2^(64n) * 2^exp,    M normalised: top bit of limb n-1 set,
// so |value| lies in [2^(exp-1), 2^exp). Zero has `zero == true` (limbs are kept zero too).
// All operations round to nearest, ties to even, exactly as if computed with unbounded
// precision and then rounded to n limbs (products use the full 2n-limb product; sums use one
// guard limb plus a jammed sticky bit, which is enough for correct rounding because a
// subtraction with alignment distance >= 2 bits cancels at most one leading bit).
//
// Outputs may alias inputs: every operation reads its inputs into scratch before writing.
// These are the building blocks of BigFloat and of the reference-orbit, period and Newton
// loops; `n` is passed explicitly so that call sites with a constant `n` specialise fully.

@usableFromInline
struct RawFloat {
    @usableFromInline var m: Limbs
    @usableFromInline var neg: Bool
    @usableFromInline var exp: Int
    @usableFromInline var zero: Bool

    @inlinable init(m: Limbs) {
        self.m = m
        neg = false
        exp = 0
        zero = true
    }
}

/// Scratch limbs needed by the RawFloat operations at n limbs.
@usableFromInline
func rawScratchLimbs(_ n: Int) -> Int {
    3 * (n + 1) + 2 * n + productScratchLimbs(n) + 8
}

@inline(__always) @usableFromInline
func rawSetZero(_ r: inout RawFloat, _ n: Int) {
    limbsZero(r.m, n)
    r.neg = false
    r.exp = 0
    r.zero = true
}

@inline(__always) @usableFromInline
func rawCopy(_ r: inout RawFloat, _ a: RawFloat, _ n: Int) {
    limbsCopy(r.m, a.m, n)
    r.neg = a.neg
    r.exp = a.exp
    r.zero = a.zero
}

/// Adds one unit in the last place of the n-limb mantissa at `m`; returns true on carry-out
/// (mantissa was all ones; it is then reset to 2^(64n-1) and the caller bumps the exponent).
@inline(__always) @usableFromInline
func mantissaIncrement(_ m: Limbs, _ n: Int) -> Bool {
    if limbsAddLimb(m, n, 1) != 0 {
        m[n &- 1] = UInt64(1) &<< 63
        return true
    }
    return false
}

// MARK: - Multiplication

/// Rounds a full 2n-limb product p of two normalised mantissas (p >= 2^(128n-2)) into `out`.
/// Returns the exponent adjustment (0 or -1).
@inline(__always) @usableFromInline
func roundProduct(_ p: Limbs, _ n: Int, _ out: Limbs) -> Int {
    var adjust = 0
    var roundBit: Bool
    var restNonZero: Bool   // bits of p[n-1] below the round bit
    if p[2 &* n &- 1] &>> 63 == 0 {
        // Shift the top n+1 limbs left by one bit.
        var i = n &- 1
        while i >= 0 {
            out[i] = (p[n &+ i] &<< 1) | (p[n &+ i &- 1] &>> 63)
            i &-= 1
        }
        roundBit = (p[n &- 1] &>> 62) & 1 != 0
        restNonZero = p[n &- 1] & ((UInt64(1) &<< 62) &- 1) != 0
        adjust = -1
    } else {
        var i = 0
        while i < n { out[i] = p[n &+ i]; i &+= 1 }
        roundBit = p[n &- 1] &>> 63 != 0
        restNonZero = p[n &- 1] &<< 1 != 0
    }
    if roundBit {
        // Round up unless exactly half-way and the mantissa is even.
        let up = (out[0] & 1 != 0) || restNonZero || !limbsIsZero(p, n &- 1)
        if up && mantissaIncrement(out, n) { adjust &+= 1 }
    }
    return adjust
}

/// Rounds a short product (columns >= n-2 only, see mulColumns) into `out` when the
/// rounding decision is certain, returning the exponent adjustment; nil means "uncertain,
/// compute the full product". The skipped columns add less than n units to limb n-1
/// (sum over i+j <= n-3 of a_i b_j B^(i+j) < n B^(n-1)), so the decision is certain when the
/// bits of limb n-1 below the round bit are non-zero (no tie possible) and stay below the
/// round bit after adding n (no change of round bit, no carry into the mantissa). This
/// fails with probability ~n/2^62, and the result is then identical to the full product's.
@inline(__always) @usableFromInline
func roundShortProduct(_ p: Limbs, _ n: Int, _ out: Limbs) -> Int? {
    let l = p[n &- 1]
    let topSet = p[2 &* n &- 1] &>> 63 != 0
    let rbPos: UInt64 = topSet ? 63 : 62
    let mask = (UInt64(1) &<< rbPos) &- 1
    let rest = l & mask
    if rest == 0 || rest &+ UInt64(n) &+ 1 > mask { return nil }
    var adjust = 0
    if topSet {
        var i = 0
        while i < n { out[i] = p[n &+ i]; i &+= 1 }
    } else {
        var i = n &- 1
        while i >= 0 {
            out[i] = (p[n &+ i] &<< 1) | (p[n &+ i &- 1] &>> 63)
            i &-= 1
        }
        adjust = -1
    }
    if (l &>> rbPos) & 1 != 0 && mantissaIncrement(out, n) { adjust &+= 1 }
    return adjust
}

/// Below this many limbs products are always full (the short product saves nothing).
@usableFromInline let shortProductMinLimbs = 3
/// Up to this many limbs the short Comba product beats a full Karatsuba product
/// (measured on M1 Pro; above it the full Karatsuba product is used).
@usableFromInline let shortProductMaxLimbs = 400

/// r = a * b.
@inline(__always) @usableFromInline
func rawMul(_ r: inout RawFloat, _ a: RawFloat, _ b: RawFloat, _ n: Int, _ scratch: Limbs) {
    if a.zero || b.zero { rawSetZero(&r, n); return }
    let neg = a.neg != b.neg
    let e = a.exp &+ b.exp
    if n >= shortProductMinLimbs && n <= shortProductMaxLimbs {
        if !mulShortFixed(scratch, a.m, b.m, n) { mulColumns(scratch, a.m, b.m, n, n &- 2) }
        if let adj = roundShortProduct(scratch, n, r.m) {
            r.neg = neg
            r.exp = e &+ adj
            r.zero = false
            return
        }
    }
    limbsMul(scratch, a.m, b.m, n, scratch + 2 &* n)
    let adj = roundProduct(scratch, n, r.m)
    r.neg = neg
    r.exp = e &+ adj
    r.zero = false
}

/// r = a * a.
@inline(__always) @usableFromInline
func rawSqr(_ r: inout RawFloat, _ a: RawFloat, _ n: Int, _ scratch: Limbs) {
    if a.zero { rawSetZero(&r, n); return }
    let e = a.exp &* 2
    if n >= shortProductMinLimbs && n <= shortProductMaxLimbs {
        if !sqrShortFixed(scratch, a.m, n) { sqrColumns(scratch, a.m, n, n &- 2) }
        if let adj = roundShortProduct(scratch, n, r.m) {
            r.neg = false
            r.exp = e &+ adj
            r.zero = false
            return
        }
    }
    limbsSqr(scratch, a.m, n, scratch + 2 &* n)
    let adj = roundProduct(scratch, n, r.m)
    r.neg = false
    r.exp = e &+ adj
    r.zero = false
}

// MARK: - Addition

/// r = a + b, or a - b when `subtract`.
@inline(__always) @usableFromInline
func rawAdd(_ r: inout RawFloat, _ a: RawFloat, _ b: RawFloat, subtract: Bool, _ n: Int, _ scratch: Limbs) {
    let bNeg = b.neg != subtract
    if b.zero {
        rawCopy(&r, a, n)
        return
    }
    if a.zero {
        rawCopy(&r, b, n)
        r.neg = bNeg
        return
    }
    // x = the operand of larger magnitude.
    var x = a, y = b
    var xNeg = a.neg, yNeg = bNeg
    let effectiveSub = xNeg != yNeg
    if a.exp < b.exp || (effectiveSub && a.exp == b.exp && limbsCompare(a.m, b.m, n) < 0) {
        swap(&x, &y)
        swap(&xNeg, &yNeg)
    }
    let d = x.exp &- y.exp
    let w = n &+ 1
    // Frame of w limbs: A = x << 64 (zero guard limb), B = (y << 64) >> d with a jammed
    // sticky bit; R = A +- B.
    let R = scratch, B = scratch + w
    if d < 64 {
        // Fast path (operands of similar size, the common case): no bits are lost.
        if d == 0 {
            B[0] = 0
            var i = 0
            while i < n { B[i &+ 1] = y.m[i]; i &+= 1 }
        } else {
            let bs = UInt64(d), back = 64 &- bs
            B[0] = y.m[0] &<< back
            var i = 1
            while i < n { B[i] = (y.m[i &- 1] &>> bs) | (y.m[i] &<< back); i &+= 1 }
            B[n] = y.m[n &- 1] &>> bs
        }
    } else {
        let F = scratch + 2 &* w
        F[0] = 0
        var i = 0
        while i < n { F[i &+ 1] = y.m[i]; i &+= 1 }
        if d >= 64 &* w {
            limbsZero(B, w)
            B[0] = 1
        } else {
            limbsExtract(B, w, F, w, d)
            if limbsAnyBitBelow(F, w, d) { B[0] |= 1 }
        }
    }
    var e = x.exp
    if !effectiveSub {
        R[0] = B[0]
        var carry: UInt64 = 0
        var i = 0
        while i < n {
            let (s0, o0) = x.m[i].addingReportingOverflow(B[i &+ 1])
            let (s1, o1) = s0.addingReportingOverflow(carry)
            R[i &+ 1] = s1
            carry = (o0 ? 1 : 0) &+ (o1 ? 1 : 0)
            i &+= 1
        }
        if carry != 0 {
            let lost = R[0] & 1
            _ = limbsShiftRight(R, R, w, 1)
            R[w &- 1] |= UInt64(1) &<< 63
            R[0] |= lost
            e &+= 1
        }
    } else {
        R[0] = 0 &- B[0]
        var borrow: UInt64 = B[0] != 0 ? 1 : 0
        var i = 0
        while i < n {
            let (s0, o0) = x.m[i].subtractingReportingOverflow(B[i &+ 1])
            let (s1, o1) = s0.subtractingReportingOverflow(borrow)
            R[i &+ 1] = s1
            borrow = (o0 ? 1 : 0) &+ (o1 ? 1 : 0)
            i &+= 1
        }
        // Normalise.
        if R[w &- 1] &>> 63 == 0 {
            var top = w &- 1
            while top >= 0 && R[top] == 0 { top &-= 1 }
            if top < 0 { rawSetZero(&r, n); return }
            let limbShift = w &- 1 &- top
            let bitShift = R[top].leadingZeroBitCount
            if limbShift > 0 {
                var j = w &- 1
                while j >= limbShift { R[j] = R[j &- limbShift]; j &-= 1 }
                while j >= 0 { R[j] = 0; j &-= 1 }
            }
            if bitShift > 0 { _ = limbsShiftLeft(R, R, w, bitShift) }
            e &-= limbShift &* 64 &+ bitShift
        }
    }
    // Round the guard limb away.
    let g = R[0]
    var i = 0
    while i < n { r.m[i] = R[i &+ 1]; i &+= 1 }
    if g &>> 63 != 0 {
        if (g &<< 1) != 0 || (r.m[0] & 1) != 0 {
            if mantissaIncrement(r.m, n) { e &+= 1 }
        }
    }
    r.neg = xNeg
    r.exp = e
    r.zero = false
}

/// Compares |a| and |b|: -1, 0, 1.
@inline(__always) @usableFromInline
func rawCompareMagnitude(_ a: RawFloat, _ b: RawFloat, _ n: Int) -> Int {
    if a.zero { return b.zero ? 0 : -1 }
    if b.zero { return 1 }
    if a.exp != b.exp { return a.exp < b.exp ? -1 : 1 }
    return limbsCompare(a.m, b.m, n)
}

/// Compares a and b: -1, 0, 1.
@inline(__always) @usableFromInline
func rawCompare(_ a: RawFloat, _ b: RawFloat, _ n: Int) -> Int {
    if a.zero && b.zero { return 0 }
    let aSign = a.zero ? 0 : (a.neg ? -1 : 1)
    let bSign = b.zero ? 0 : (b.neg ? -1 : 1)
    if aSign != bSign { return aSign < bSign ? -1 : 1 }
    let c = rawCompareMagnitude(a, b, n)
    return aSign < 0 ? -c : c
}

// MARK: - Generic rounding from a wide integer

/// Rounds the integer W = w[0..<k] (plus an extra sticky bit below it when `stickyIn`) to a
/// normalised n-limb mantissa in `out`. Returns the bit length of the rounded result, i.e.
/// W*2^u rounds to out/2^(64n) * 2^(result + u). W must be non-zero.
@usableFromInline
func roundWide(_ w: Limbs, _ k: Int, stickyIn: Bool, _ n: Int, _ out: Limbs) -> Int {
    var bl = limbsBitLength(w, k)
    precondition(bl > 0, "roundWide of zero")
    let shift = bl &- 64 &* n
    if shift <= 0 {
        // Exact (the sticky bit lies below a zero round bit, so it never rounds up).
        let s = -shift
        // out = W << s, as n limbs.
        let ls = s >> 6, bs = s & 63
        limbsZero(out, n)
        var i = 0
        while i < k && i &+ ls < n { out[i &+ ls] = w[i]; i &+= 1 }
        if bs > 0 { _ = limbsShiftLeft(out, out, n, bs) }
        return bl
    }
    limbsExtract(out, n, w, k, shift)
    if limbsBit(w, k, shift &- 1) {
        let up = (out[0] & 1 != 0) || stickyIn || limbsAnyBitBelow(w, k, shift &- 1)
        if up && mantissaIncrement(out, n) { bl &+= 1 }
    }
    return bl
}

/// Rounds a normalised mantissa of `srcN` limbs to `n` limbs (n < srcN) into out; returns
/// the exponent adjustment (0 or 1).
@usableFromInline
func roundMantissa(_ src: Limbs, _ srcN: Int, _ n: Int, _ out: Limbs) -> Int {
    let bl = roundWide(src, srcN, stickyIn: false, n, out)
    return bl &- 64 &* srcN
}

// MARK: - Conversions of a single value

/// 2^e as a Double (0 below the subnormal range, inf above the normal range).
@inline(__always) @usableFromInline
func pow2Double(_ e: Int) -> Double {
    if e > 1023 { return .infinity }
    if e >= -1022 { return Double(bitPattern: UInt64(e &+ 1023) &<< 52) }
    if e >= -1074 { return Double(bitPattern: UInt64(1) &<< UInt64(e &+ 1074)) }
    return 0
}

/// The top limb with a jammed sticky bit if any lower limb is non-zero: converting this
/// 64-bit value to Double or Float rounds the same as the full mantissa would.
@inline(__always) @usableFromInline
func jammedTopLimb(_ m: Limbs, _ n: Int) -> UInt64 {
    var t = m[n &- 1]
    if n > 1 && !limbsIsZero(m, n &- 1) { t |= 1 }
    return t
}

/// Mantissa in [0.5, 1] as a Double (may round up to exactly 1.0) and the exponent.
@inline(__always) @usableFromInline
func rawDoubleMantissa(_ a: RawFloat, _ n: Int) -> Double {
    Double(jammedTopLimb(a.m, n)) * 0x1p-64
}

/// Nearest Double (ties-to-even except in the subnormal range, where it may double-round).
@inline(__always) @usableFromInline
func rawToDouble(_ a: RawFloat, _ n: Int) -> Double {
    if a.zero { return 0 }
    var mant = rawDoubleMantissa(a, n)
    var e = a.exp
    if mant == 1.0 { mant = 0.5; e &+= 1 }
    // Split the scaling so that mant * 2^e stays exact for e down to the subnormal range.
    let v: Double
    if e >= -1000 {
        v = mant * pow2Double(e)
    } else {
        v = (mant * pow2Double(e &+ 200)) * 0x1p-200
    }
    return a.neg ? -v : v
}

/// Quick Double approximation for hot loops (truncates the mantissa to its top limb).
@inline(__always) @usableFromInline
func rawToDoubleFast(_ a: RawFloat, _ n: Int) -> Double {
    if a.zero { return 0 }
    let v: Double
    if a.exp >= -1000 {
        v = Double(a.m[n &- 1]) * 0x1p-64 * pow2Double(a.exp)
    } else {
        v = Double(a.m[n &- 1]) * 0x1p-64 * pow2Double(a.exp &+ 200) * 0x1p-200
    }
    return a.neg ? -v : v
}

/// Sets r = d exactly (n >= 1 limbs hold all 53 bits). Non-finite d is a precondition failure.
@usableFromInline
func rawSetDouble(_ r: inout RawFloat, _ d: Double, _ n: Int) {
    precondition(d.isFinite, "BigFloat from a non-finite Double")
    limbsZero(r.m, n)
    if d == 0 { r.neg = false; r.exp = 0; r.zero = true; return }
    r.neg = d < 0
    let mag = d.magnitude
    // mag = significandBits * 2^(exponent - 52) for normals (with the hidden bit).
    let sig: UInt64
    let e2: Int
    if mag.isNormal {
        sig = mag.significandBitPattern | (UInt64(1) << 52)
        e2 = Int(mag.exponent) - 52
    } else {
        sig = mag.significandBitPattern
        e2 = -1074
    }
    let lz = sig.leadingZeroBitCount
    r.m[n &- 1] = sig << UInt64(lz)
    // value = (sig << lz) / 2^64 * 2^(64 - lz + e2)
    r.exp = 64 - lz + e2
    r.zero = false
}
