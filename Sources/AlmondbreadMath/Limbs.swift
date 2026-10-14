// Low-level unsigned multi-limb arithmetic on raw pointers.
//
// Limbs are UInt64, little-endian (limb 0 is least significant). Every routine works on
// caller-owned memory and never allocates, so the reference-orbit loop can run without
// touching the heap. Products use `multipliedFullWidth` and explicit carry chains; the
// basecase is column-wise (Comba / product scanning) with a three-word accumulator, which
// keeps the running sum in registers and, when `n` is a compile-time constant after
// inlining, lets LLVM unroll the whole product (kb: platform/mpfr-gmp-on-apple-silicon,
// fixed-size kernels beat GMP's basecase at ~8 limbs).

@usableFromInline typealias Limbs = UnsafeMutablePointer<UInt64>

/// Karatsuba takes over from the Comba basecase at this many limbs (tuned on M1 Pro).
@usableFromInline let karatsubaMulThreshold = 40
@usableFromInline let karatsubaSqrThreshold = 48

// MARK: - Linear operations

/// r = a + b over n limbs; returns the carry (0 or 1). r may alias a or b.
@inline(__always) @usableFromInline
func limbsAdd(_ r: Limbs, _ a: Limbs, _ b: Limbs, _ n: Int) -> UInt64 {
    var carry: UInt64 = 0
    var i = 0
    while i < n {
        let (s0, o0) = a[i].addingReportingOverflow(b[i])
        let (s1, o1) = s0.addingReportingOverflow(carry)
        r[i] = s1
        carry = (o0 ? 1 : 0) &+ (o1 ? 1 : 0)
        i &+= 1
    }
    return carry
}

/// r = a - b over n limbs; returns the borrow (0 or 1). r may alias a or b.
@inline(__always) @usableFromInline
func limbsSub(_ r: Limbs, _ a: Limbs, _ b: Limbs, _ n: Int) -> UInt64 {
    var borrow: UInt64 = 0
    var i = 0
    while i < n {
        let (s0, o0) = a[i].subtractingReportingOverflow(b[i])
        let (s1, o1) = s0.subtractingReportingOverflow(borrow)
        r[i] = s1
        borrow = (o0 ? 1 : 0) &+ (o1 ? 1 : 0)
        i &+= 1
    }
    return borrow
}

/// Adds the single limb `v` into r[0..<n]; returns the carry out.
@inline(__always) @usableFromInline
func limbsAddLimb(_ r: Limbs, _ n: Int, _ v: UInt64) -> UInt64 {
    var carry = v
    var i = 0
    while i < n && carry != 0 {
        let (s, o) = r[i].addingReportingOverflow(carry)
        r[i] = s
        carry = o ? 1 : 0
        i &+= 1
    }
    return carry
}

/// Subtracts the single limb `v` from r[0..<n]; returns the borrow out.
@inline(__always) @usableFromInline
func limbsSubLimb(_ r: Limbs, _ n: Int, _ v: UInt64) -> UInt64 {
    var borrow = v
    var i = 0
    while i < n && borrow != 0 {
        let (s, o) = r[i].subtractingReportingOverflow(borrow)
        r[i] = s
        borrow = o ? 1 : 0
        i &+= 1
    }
    return borrow
}

/// Compares a and b (n limbs each): -1, 0 or 1.
@inline(__always) @usableFromInline
func limbsCompare(_ a: Limbs, _ b: Limbs, _ n: Int) -> Int {
    var i = n &- 1
    while i >= 0 {
        if a[i] != b[i] { return a[i] < b[i] ? -1 : 1 }
        i &-= 1
    }
    return 0
}

@inline(__always) @usableFromInline
func limbsIsZero(_ a: Limbs, _ n: Int) -> Bool {
    var i = 0
    while i < n {
        if a[i] != 0 { return false }
        i &+= 1
    }
    return true
}

@inline(__always) @usableFromInline
func limbsCopy(_ r: Limbs, _ a: Limbs, _ n: Int) {
    if r == a { return }
    var i = 0
    while i < n { r[i] = a[i]; i &+= 1 }
}

@inline(__always) @usableFromInline
func limbsZero(_ r: Limbs, _ n: Int) {
    var i = 0
    while i < n { r[i] = 0; i &+= 1 }
}

/// r = a << bits (0 < bits < 64) over n limbs; returns the bits shifted out (low bits of
/// the result limb). Processes from the top, so r may alias a.
@inline(__always) @usableFromInline
func limbsShiftLeft(_ r: Limbs, _ a: Limbs, _ n: Int, _ bits: Int) -> UInt64 {
    let back = UInt64(64 &- bits)
    let sh = UInt64(bits)
    let out = a[n &- 1] &>> back
    var i = n &- 1
    while i > 0 {
        r[i] = (a[i] &<< sh) | (a[i &- 1] &>> back)
        i &-= 1
    }
    r[0] = a[0] &<< sh
    return out
}

/// r = a >> bits (0 < bits < 64) over n limbs; returns the bits shifted out, left-aligned
/// in the returned limb. Processes from the bottom, so r may alias a.
@inline(__always) @usableFromInline
func limbsShiftRight(_ r: Limbs, _ a: Limbs, _ n: Int, _ bits: Int) -> UInt64 {
    let back = UInt64(64 &- bits)
    let sh = UInt64(bits)
    let out = a[0] &<< back
    var i = 0
    while i < n &- 1 {
        r[i] = (a[i] &>> sh) | (a[i &+ 1] &<< back)
        i &+= 1
    }
    r[n &- 1] = a[n &- 1] &>> sh
    return out
}

/// Number of significant bits in a[0..<n] (0 if zero).
@inline(__always) @usableFromInline
func limbsBitLength(_ a: Limbs, _ n: Int) -> Int {
    var i = n &- 1
    while i >= 0 {
        if a[i] != 0 { return i &* 64 &+ (64 &- a[i].leadingZeroBitCount) }
        i &-= 1
    }
    return 0
}

/// True if any bit below bit position `pos` of a[0..<n] is set.
@inline(__always) @usableFromInline
func limbsAnyBitBelow(_ a: Limbs, _ n: Int, _ pos: Int) -> Bool {
    if pos <= 0 { return false }
    let full = min(pos >> 6, n)
    var i = 0
    while i < full {
        if a[i] != 0 { return true }
        i &+= 1
    }
    let rem = pos & 63
    if rem != 0 && full < n {
        return a[full] & ((UInt64(1) &<< UInt64(rem)) &- 1) != 0
    }
    return false
}

/// Bit `pos` of a[0..<n] (0 beyond the end).
@inline(__always) @usableFromInline
func limbsBit(_ a: Limbs, _ n: Int, _ pos: Int) -> Bool {
    let l = pos >> 6
    if pos < 0 || l >= n { return false }
    return (a[l] &>> UInt64(pos & 63)) & 1 != 0
}

/// r[0..<rn] = bits [shift, shift + 64*rn) of a[0..<n] (zero beyond the end), shift >= 0.
@inline(__always) @usableFromInline
func limbsExtract(_ r: Limbs, _ rn: Int, _ a: Limbs, _ n: Int, _ shift: Int) {
    let ls = shift >> 6
    let bs = UInt64(shift & 63)
    var i = 0
    while i < rn {
        let j = i &+ ls
        let lo: UInt64 = j < n ? a[j] : 0
        if bs == 0 {
            r[i] = lo
        } else {
            let hi: UInt64 = j &+ 1 < n ? a[j &+ 1] : 0
            r[i] = (lo &>> bs) | (hi &<< (64 &- bs))
        }
        i &+= 1
    }
}

// MARK: - Products by one limb

/// r = a * m over n limbs; returns the high limb.
@inline(__always) @usableFromInline
func limbsMul1(_ r: Limbs, _ a: Limbs, _ n: Int, _ m: UInt64) -> UInt64 {
    var carry: UInt64 = 0
    var i = 0
    while i < n {
        let (h, l) = a[i].multipliedFullWidth(by: m)
        let (s, o) = l.addingReportingOverflow(carry)
        r[i] = s
        carry = h &+ (o ? 1 : 0)
        i &+= 1
    }
    return carry
}

/// r += a * m over n limbs; returns the carry limb.
@inline(__always) @usableFromInline
func limbsAddMul1(_ r: Limbs, _ a: Limbs, _ n: Int, _ m: UInt64) -> UInt64 {
    var carry: UInt64 = 0
    var i = 0
    while i < n {
        let (h, l) = a[i].multipliedFullWidth(by: m)
        let (s0, o0) = l.addingReportingOverflow(carry)
        let (s1, o1) = r[i].addingReportingOverflow(s0)
        r[i] = s1
        carry = h &+ (o0 ? 1 : 0) &+ (o1 ? 1 : 0)
        i &+= 1
    }
    return carry
}

/// r -= a * m over n limbs; returns the borrow limb.
@inline(__always) @usableFromInline
func limbsSubMul1(_ r: Limbs, _ a: Limbs, _ n: Int, _ m: UInt64) -> UInt64 {
    var carry: UInt64 = 0
    var i = 0
    while i < n {
        let (h, l) = a[i].multipliedFullWidth(by: m)
        let (s0, o0) = l.addingReportingOverflow(carry)
        let (s1, o1) = r[i].subtractingReportingOverflow(s0)
        r[i] = s1
        carry = h &+ (o0 ? 1 : 0) &+ (o1 ? 1 : 0)
        i &+= 1
    }
    return carry
}

// MARK: - Basecase products (Comba)
//
// Column k sums a_i * b_(k-i). The low and high halves of the products go into two
// independent two-word accumulators, so the add chains do not serialise through one carry
// (measured ~1.6x faster than a single three-word accumulator at 16 limbs on M1 Pro).
// `k0` > 0 skips the low columns (short product): limbs below k0 are then not written and
// limbs from k0 up lack the carries of the skipped columns (see roundShortProduct).

/// r[k0..<2n] = high columns of a * b. r must not alias a or b.
@inline(__always) @usableFromInline
func mulColumns(_ r: Limbs, _ a: Limbs, _ b: Limbs, _ n: Int, _ k0: Int) {
    var cin0: UInt64 = 0, cin1: UInt64 = 0
    var k = k0
    let top = 2 &* n &- 1
    while k < top {
        var i = k < n ? 0 : k &- n &+ 1
        let hiIdx = k < n ? k : n &- 1
        var lo0 = cin0, lo1 = cin1
        var hi0: UInt64 = 0, hi1: UInt64 = 0
        while i <= hiIdx {
            let (h, l) = a[i].multipliedFullWidth(by: b[k &- i])
            let (s0, o0) = lo0.addingReportingOverflow(l)
            lo0 = s0; lo1 &+= o0 ? 1 : 0
            let (s1, o1) = hi0.addingReportingOverflow(h)
            hi0 = s1; hi1 &+= o1 ? 1 : 0
            i &+= 1
        }
        r[k] = lo0
        let (t, o) = lo1.addingReportingOverflow(hi0)
        cin0 = t
        cin1 = hi1 &+ (o ? 1 : 0)
        k &+= 1
    }
    r[top] = cin0
}

/// r[k0..<2n] = high columns of a * a. r must not alias a.
@inline(__always) @usableFromInline
func sqrColumns(_ r: Limbs, _ a: Limbs, _ n: Int, _ k0: Int) {
    var cin0: UInt64 = 0, cin1: UInt64 = 0
    var k = k0
    let top = 2 &* n &- 1
    while k < top {
        var i = k < n ? 0 : k &- n &+ 1
        var j = k &- i
        var lo0: UInt64 = 0, lo1: UInt64 = 0
        var hi0: UInt64 = 0, hi1: UInt64 = 0
        while i < j {
            let (h, l) = a[i].multipliedFullWidth(by: a[j])
            let (s0, o0) = lo0.addingReportingOverflow(l)
            lo0 = s0; lo1 &+= o0 ? 1 : 0
            let (s1, o1) = hi0.addingReportingOverflow(h)
            hi0 = s1; hi1 &+= o1 ? 1 : 0
            i &+= 1; j &-= 1
        }
        // Double the off-diagonal sums (each < 2^127 for the column counts used here).
        lo1 = (lo1 &<< 1) | (lo0 &>> 63); lo0 = lo0 &<< 1
        hi1 = (hi1 &<< 1) | (hi0 &>> 63); hi0 = hi0 &<< 1
        if i == j {
            let (h, l) = a[i].multipliedFullWidth(by: a[i])
            let (s0, o0) = lo0.addingReportingOverflow(l)
            lo0 = s0; lo1 &+= o0 ? 1 : 0
            let (s1, o1) = hi0.addingReportingOverflow(h)
            hi0 = s1; hi1 &+= o1 ? 1 : 0
        }
        let (u0, p0) = lo0.addingReportingOverflow(cin0)
        lo1 = lo1 &+ cin1 &+ (p0 ? 1 : 0)
        r[k] = u0
        let (t, o) = lo1.addingReportingOverflow(hi0)
        cin0 = t
        cin1 = hi1 &+ (o ? 1 : 0)
        k &+= 1
    }
    r[top] = cin0
}

/// r[0..<2n] = a * b. r must not alias a or b.
@inline(__always) @usableFromInline
func mulBasecase(_ r: Limbs, _ a: Limbs, _ b: Limbs, _ n: Int) {
    mulColumns(r, a, b, n, 0)
}

/// r[0..<2n] = a * a. r must not alias a.
@inline(__always) @usableFromInline
func sqrBasecase(_ r: Limbs, _ a: Limbs, _ n: Int) {
    sqrColumns(r, a, n, 0)
}

// MARK: - Karatsuba

/// Scratch limbs needed by `limbsMul`/`limbsSqr` for n-limb operands (beyond the result).
@usableFromInline
func productScratchLimbs(_ n: Int) -> Int {
    if n < min(karatsubaMulThreshold, karatsubaSqrThreshold) { return 0 }
    let l = n - n / 2
    return 4 * (l + 1) + productScratchLimbs(l + 1)
}

/// r[0..<2n] = a * b, r must not alias a or b; `scratch` needs productScratchLimbs(n).
@inline(__always) @usableFromInline
func limbsMul(_ r: Limbs, _ a: Limbs, _ b: Limbs, _ n: Int, _ scratch: Limbs) {
    if n < karatsubaMulThreshold {
        mulBasecase(r, a, b, n)
    } else {
        mulKaratsuba(r, a, b, n, scratch)
    }
}

/// r[0..<2n] = a * a, r must not alias a; `scratch` needs productScratchLimbs(n).
@inline(__always) @usableFromInline
func limbsSqr(_ r: Limbs, _ a: Limbs, _ n: Int, _ scratch: Limbs) {
    if n < karatsubaSqrThreshold {
        sqrBasecase(r, a, n)
    } else {
        sqrKaratsuba(r, a, n, scratch)
    }
}

/// a = a0 + a1 * B^h with a0 of h limbs and a1 of l = n - h limbs (l = h or h + 1).
/// Writes a0 + a1 into s[0...l] (l + 1 limbs).
@inline(__always)
func karatsubaHalfSum(_ s: Limbs, _ a: Limbs, _ h: Int, _ l: Int) {
    let c = limbsAdd(s, a, a + h, h)
    var top = c
    if l > h {
        let (v, o) = a[h + h].addingReportingOverflow(c)
        s[h] = v
        top = o ? 1 : 0
    }
    s[l] = top
}

/// Adds z1 = sum - z0 - z2 into r at limb offset h (shared tail of mul and sqr).
@inline(__always)
func karatsubaCombine(_ r: Limbs, _ z1: Limbs, _ n: Int, _ h: Int, _ l: Int) {
    let w = 2 * l + 2
    // z1 -= z0 (2h limbs), z1 -= z2 (2l limbs); the result is a0*b1 + a1*b0 >= 0.
    var bw = limbsSub(z1, z1, r, 2 * h)
    _ = limbsSubLimb(z1 + 2 * h, w - 2 * h, bw)
    bw = limbsSub(z1, z1, r + 2 * h, 2 * l)
    _ = limbsSubLimb(z1 + 2 * l, w - 2 * l, bw)
    // r[h...] += z1; z1 < 2^(64(2l+1)) and r has 2n - h >= 2l + 1 limbs above h.
    let len = min(w, 2 * n - h)
    let c = limbsAdd(r + h, r + h, z1, len)
    _ = limbsAddLimb(r + h + len, 2 * n - h - len, c)
}

@usableFromInline
func mulKaratsuba(_ r: Limbs, _ a: Limbs, _ b: Limbs, _ n: Int, _ scratch: Limbs) {
    let h = n / 2, l = n - h
    let sa = scratch, sb = scratch + (l + 1), z1 = scratch + 2 * (l + 1)
    let rest = scratch + 4 * (l + 1)
    limbsMul(r, a, b, h, rest)
    limbsMul(r + 2 * h, a + h, b + h, l, rest)
    karatsubaHalfSum(sa, a, h, l)
    karatsubaHalfSum(sb, b, h, l)
    limbsMul(z1, sa, sb, l + 1, rest)
    karatsubaCombine(r, z1, n, h, l)
}

@usableFromInline
func sqrKaratsuba(_ r: Limbs, _ a: Limbs, _ n: Int, _ scratch: Limbs) {
    let h = n / 2, l = n - h
    let sa = scratch, z1 = scratch + 2 * (l + 1)
    let rest = scratch + 4 * (l + 1)
    limbsSqr(r, a, h, rest)
    limbsSqr(r + 2 * h, a + h, l, rest)
    karatsubaHalfSum(sa, a, h, l)
    limbsSqr(z1, sa, l + 1, rest)
    karatsubaCombine(r, z1, n, h, l)
}
