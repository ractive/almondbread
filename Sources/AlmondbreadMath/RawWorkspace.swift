// A preallocated block of RawFloat registers plus scratch for the hot loops.

@usableFromInline
final class RawWorkspace {
    @usableFromInline let n: Int
    @usableFromInline let registerCount: Int
    @usableFromInline let block: UnsafeMutablePointer<UInt64>
    @usableFromInline let scratch: UnsafeMutablePointer<UInt64>

    @usableFromInline init(limbs n: Int, registers: Int) {
        self.n = n
        registerCount = registers
        let scratchCount = rawScratchLimbs(n)
        block = .allocate(capacity: n * registers + scratchCount)
        block.initialize(repeating: 0, count: n * registers + scratchCount)
        scratch = block + n * registers
    }

    deinit { block.deallocate() }

    /// A zero register backed by slot i.
    @inline(__always) @usableFromInline func register(_ i: Int) -> RawFloat {
        precondition(i < registerCount)
        return RawFloat(m: block + i * n)
    }

    /// Loads a BigFloat into register r (rounded to n limbs if it is wider).
    @usableFromInline func load(_ r: inout RawFloat, _ v: BigFloat) {
        let w = v.withLimbCount(n)
        w.withRaw { raw in rawCopy(&r, raw, n) }
    }

    /// Copies register r out as a BigFloat.
    @usableFromInline func store(_ r: RawFloat) -> BigFloat {
        var b = BigFloat(zeroLimbs: n)
        let nn = n
        b.withMutableRaw { dst in rawCopy(&dst, r, nn) }
        return b
    }
}
