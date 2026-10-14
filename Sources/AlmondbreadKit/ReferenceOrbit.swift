import simd

/// The reference orbit Z_0 = 0, Z_{n+1} = Z_n^2 + C, computed on the CPU.
///
/// Stored as float32 for the GPU: the perturbation iteration only needs Z to float precision
/// (kb: techniques/perturbation). Computed in Double for now, which limits zoom to about
/// 1e-13; an arbitrary-precision (MPFR) version replaces this for deeper zooms.
struct ReferenceOrbit {
    let centerRe: Double
    let centerIm: Double
    /// Viewport scale the reference was made for.
    let scale: Double
    let maxIter: Int
    let points: [SIMD2<Float>]

    /// Iterates until the orbit escapes `bailout` or `maxIter` steps are done.
    /// Always returns at least two points so the GPU loop has a valid step.
    init(centerRe: Double, centerIm: Double, scale: Double, maxIter: Int, bailout: Double) {
        self.centerRe = centerRe
        self.centerIm = centerIm
        self.scale = scale
        self.maxIter = maxIter

        var pts = [SIMD2<Float>]()
        pts.reserveCapacity(maxIter + 1)
        var x = 0.0, y = 0.0
        let b2 = bailout * bailout
        pts.append(SIMD2(0, 0))
        for _ in 0..<maxIter {
            let nx = x * x - y * y + centerRe
            y = 2 * x * y + centerIm
            x = nx
            pts.append(SIMD2(Float(x), Float(y)))
            if x * x + y * y > b2 { break }
        }
        if pts.count < 2 { pts.append(SIMD2(Float(centerRe), Float(centerIm))) }
        points = pts
    }
}
