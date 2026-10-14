import simd

// Mirrors of the Metal structs in Shaders/Mandelbrot.metalsrc. Field order and types must match.
// Layouts (MSL float2/SIMD2<Float>: size 8, align 8; float4/SIMD4<Float>: size 16, align 16):
//   IterateParams: 3 x float2 @0,8,16; texelScale 24, bailout2 28, maxIter 32, refLen 36,
//                  chunkIters 40, width 44, total 48, capacity 52, statSlot 56,
//                  interiorEps 60, firstChunk 64, pad0..2 68..76 -> 80 bytes.
//   PixelState:    dz @0, d 8, w 16, pixel 24, n 28, m 32, wExp 36 -> 40 bytes.
//   ChunkStats:    liveIn 0, liveOut 4, iterationsLo 8, iterationsHi 12, fresh 16,
//                  admitted 20, interior 24, pad1 28 -> 32 bytes.
//   LevelCounters: admitted 0, freshStart 4, freshCount 8, pad 12 -> 16 bytes (GPU only).
//   ColorParams:   interior @0, edge @16, light @32, density 40, offset 44, ratio 48,
//                  shading 52, outline 56, outlineWidth 60, paletteSize 64, logScale 68,
//                  smooth 72, admitted 76, hasUnder 80, linearOut 84, admittedOnGPU 88,
//                  deScale 92 -> 96 bytes.
// `checkGPULayouts()` asserts these in debug builds.

func checkGPULayouts() {
    assert(MemoryLayout<IterateParams>.stride == 80 && MemoryLayout<IterateParams>.offset(of: \.texelScale) == 24
           && MemoryLayout<IterateParams>.offset(of: \.chunkIters) == 40
           && MemoryLayout<IterateParams>.offset(of: \.firstChunk) == 64
           && MemoryLayout<IterateParams>.offset(of: \.pad2) == 76)
    assert(MemoryLayout<PixelState>.stride == 40 && MemoryLayout<PixelState>.offset(of: \.pixel) == 24
           && MemoryLayout<PixelState>.offset(of: \.wExp) == 36)
    assert(MemoryLayout<ChunkStats>.stride == 32 && MemoryLayout<ChunkStats>.offset(of: \.iterationsHi) == 12
           && MemoryLayout<ChunkStats>.offset(of: \.admitted) == 20
           && MemoryLayout<ChunkStats>.offset(of: \.interior) == 24)
    assert(MemoryLayout<ColorParams>.stride == 96 && MemoryLayout<ColorParams>.offset(of: \.light) == 32
           && MemoryLayout<ColorParams>.offset(of: \.paletteSize) == 64
           && MemoryLayout<ColorParams>.offset(of: \.admitted) == 76
           && MemoryLayout<ColorParams>.offset(of: \.linearOut) == 84
           && MemoryLayout<ColorParams>.offset(of: \.deScale) == 92)
}

struct IterateParams {
    var halfSize: SIMD2<Float>
    var refOffset: SIMD2<Float>
    var jitter: SIMD2<Float>
    var texelScale: Float
    var bailout2: Float
    var maxIter: UInt32
    var refLen: UInt32
    var chunkIters: UInt32 = 0
    var width: UInt32
    var total: UInt32
    var capacity: UInt32
    var statSlot: UInt32 = 0
    var interiorEps: Float
    var firstChunk: UInt32 = 0
    var pad0: UInt32 = 0
    var pad1: UInt32 = 0
    var pad2: UInt32 = 0
}

/// Loop state of one live pixel; only allocated on the GPU (sizes the state buffers).
struct PixelState {
    var dz: SIMD2<Float>
    var d: SIMD2<Float>
    var w: SIMD2<Float>
    var pixel: UInt32
    var n: UInt32
    var m: UInt32
    var wExp: Int32
}

struct ChunkStats {
    var liveIn: UInt32
    var liveOut: UInt32
    var iterationsLo: UInt32
    var iterationsHi: UInt32
    var fresh: UInt32
    var admitted: UInt32
    var interior: UInt32
    var pad1: UInt32
    var iterations: UInt64 { UInt64(iterationsHi) << 32 | UInt64(iterationsLo) }
}

struct ColorParams {
    var interior: SIMD4<Float>
    var edge: SIMD4<Float>
    var light: SIMD2<Float>
    var density: Float
    var offset: Float
    var ratio: Float
    var shading: Float
    var outline: Float
    var outlineWidth: Float
    var paletteSize: UInt32
    var logScale: UInt32
    var smooth: UInt32
    var admitted: UInt32 = 0
    var hasUnder: Float = 0
    var linearOut: UInt32 = 0
    var admittedOnGPU: UInt32 = 0
    var deScale: Float = 1
}
