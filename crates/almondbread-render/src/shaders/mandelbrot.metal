// Almondbread GPU kernels (Rust render crate copy; kb: platform/render-architecture). Compiled at
// runtime from source with the same options as the Swift renderer (MSL 3.0, fast math).
//
// Pass 1 `iterate`: perturbation iteration per texel against a reference orbit computed on
// the CPU, with Zhuoran rebasing (kb: techniques/rebasing). Resumable and chunked
// (kb: platform/gpu-watchdog-and-chunking): each dispatch runs at most `chunkIters`
// iterations per pixel, then either writes the finished pixel to the output textures or
// appends its loop state to the next live list. Live state moves with its list slot (not
// indexed by pixel), so state memory is bounded by the live-list capacity, not the image
// size, and reads/writes stay coalesced. The CPU admits fresh pixels into free capacity in
// scanline order. Output: the exact escape iteration n plus the smooth offset, the exterior
// distance estimate and the surface normal angle.
// Pass 2 `colorize`: maps iteration data to colours, averaging supersamples, falling back to
// the coarser preview (coloured first at its own resolution, then upsampled bilinearly, not
// blocky nearest texels) or the interior colour where a pixel is not finished yet.
// Changing colour settings only re-runs pass 2.
//
// The structs below must match the Rust side byte for byte (gpu_types.rs, compile-time asserts).
// Differences from the Swift copy (Sources/AlmondbreadKit/Shaders/Mandelbrot.metalsrc): colorize
// takes the used sub-region sizes in ColorParams (srcSize, outSize, underSize), so a preview
// level can live in the top-left corner of a texture allocated once at its maximum size.
// `iterate` is unchanged: iteration fields are bit-identical to the Swift renderer.

#include <metal_stdlib>
using namespace metal;

// Specialisation (kb: platform/metal-compute): one pipeline per feature set, so a disabled
// feature costs no registers or per-iteration branches.
constant bool kDE       [[function_constant(0)]];   // dz/dc for distance estimate / normals
constant bool kInterior [[function_constant(1)]];   // dz/dz1 derivative interior test
constant bool kBLA      [[function_constant(2)]];   // BLA skip table (kb: techniques/bla)
constant bool kDeep     [[function_constant(3)]];   // M3b: rescaled float32 deltas at any depth
                                                    // (kb: techniques/extended-range-floats)
constant bool kInteriorColor [[function_constant(4)]];  // analyse interior cycles (period,
                                                    // multiplier, interior DE) for the interior
                                                    // colourings (kb: techniques/coloring-methods)

// Iteration-texture codes in .x besides n >= 0.
constant float ITER_INTERIOR   = -1.0f;   // .y = 1: derivative test, 0: reached maxIter without the test
constant float ITER_PENDING    = -2.0f;   // admitted but still iterating
constant float ITER_UNRESOLVED = -3.0f;   // past the nominal limit, neither escaped nor proven
                                          // interior (.y = the nominal limit); may still resolve
constant float ITER_SKIP       = -4.0f;   // AA sample textures: this pixel takes no AA samples

struct IterateParams {
    float2 halfSize;      // iteration texture size / 2, in texels
    float2 refOffset;     // view centre minus reference point (complex), small
    float2 jitter;        // sample position inside the texel, 0..1 (0.5 = centre)
    float  texelScale;    // complex units per texel
    float  bailout2;      // squared escape radius
    uint   maxIter;       // hard limit: unresolved pixels keep iterating up to here
    uint   refLen;        // number of valid entries in ref[] (>= 2)
    uint   chunkIters;    // iteration budget per pixel in this dispatch
    uint   width;         // texture width, for fresh pixel index -> (x, y)
    uint   total;         // texels in the level
    uint   capacity;      // live-list capacity (state buffer slots)
    uint   statSlot;      // ChunkStats ring slot of this chunk
    float  interiorEps;   // derivative test threshold on max(|w.x|, |w.y|) * n^2
    uint   firstChunk;    // 1: no live list yet (level start)
    uint   softIter;      // nominal limit (escapedPast buckets, unresolved marker)
    uint   listMode;      // 0: scanline admission; 1: scanline, and collect the pixels left
                          // unresolved at maxIter into listOut; 2: admit from listIn (an
                          // extension pass to a higher maxIter), collect into listOut
    uint   listCapacity;  // entries of the unresolved list
    uint   tilesX;        // 16x16 tiles per row: admission index a -> tile rank a / 256, texel
                          // a % 256 inside it (FL: spatially coherent, priority-ordered refinement)
    uint   height;        // level height in texels (padding texels of edge tiles are skipped)
    uint   tileOrder;     // 0: tiles in raster order; 1: tile = order[rank] (buffer 13)
    uint   pad0;
    // M3b (kDeep): texelScale and refOffset are mantissas in units of 2^scaleExp, so a texel
    // of 1e-1000 is texelScale * 2^scaleExp with texelScale in [1, 2). Plain kernels: 0.
    int    scaleExp;
    uint   pad1;
    // View rotation (cos, sin) of the texel offset (kb: decisions/controls "View rotation");
    // (1, 0) unrotated, which keeps the plain products bit for bit.
    float2 rot;
};

// Admission index -> texel of a tiled level (kb: techniques/frame-reuse-and-progressive, FL):
// texels are admitted tile by tile (16 x 16, row-major inside a tile); the tile order is raster
// order or a priority list (newly exposed tiles first, then by distance from the zoom target).
// Returns false for padding texels of edge tiles.
static inline bool tileTexel(uint a, uint tilesX, uint width, uint height, uint tileOrder,
                             device const uint* order, thread uint2& xy) {
    uint rank = a >> 8, within = a & 255u;
    uint tile = tileOrder != 0 ? order[rank] : rank;
    xy = uint2((tile % tilesX) * 16u + (within & 15u), (tile / tilesX) * 16u + (within >> 4));
    return xy.x < width && xy.y < height;
}

// Texel -> admission index (the inverse; rank[] is the inverse of order[]).
static inline uint tileIndex(uint2 s, uint tilesX, uint tileOrder, device const uint* rank) {
    uint tile = (s.y >> 4) * tilesX + (s.x >> 4);
    uint r = tileOrder != 0 ? rank[tile] : tile;
    return (r << 8) | ((s.y & 15u) << 4) | (s.x & 15u);
}

// Loop state of one live pixel. BLA needs nothing beyond (dz, m, n), so a level may switch
// between the BLA and the plain pipeline at any chunk boundary. The deep kernel (M3b) carries
// the delta and the DE derivative with exponents: dz * 2^e (e = 0: a plain float32 delta) and
// d * 2^dExp (kb: techniques/extended-range-floats).
struct PixelState {
    float2 dz;            // delta to the reference orbit (deep: mantissa)
    float2 d;             // dz/dc * texelScale (DE), if kDE (deep: mantissa)
    float2 w;             // dz_n/dz_1 mantissa, if kInterior
    uint   pixel;         // scanline index into the level textures
    uint   n;             // iterations done
    uint   m;             // reference index (differs from n after rebasing)
    int    wExp;          // w = w * 2^wExp (keeps w in float range on long exterior orbits)
    int    e;             // deep: delta exponent (0: plain regime)
    int    dExp;          // deep: DE exponent
};

// Interior cycle analysis of one live pixel (kInteriorColor only; a buffer parallel to the
// live state, same slots). Complex values are floatexp: mantissa, exponent.
struct PixelStateIC {
    float2 zs;            // orbit point the cycle search started at (full z)
    int    zsExp;
    uint   k;             // analysis steps done
    float2 dzdz;          // d^2/dz^2 f^k
    int    dzdzExp;
    float  minLog2;       // log2 of the smallest |z| over the analysed steps
    float2 dcdz;          // d^2/dc dz f^k, times the texel size
    int    dcdzExp;
    uint   capFlags;      // q (bits 0..23), tries (24..30), analysing (31)
};
constant uint IC_ACTIVE   = 0x80000000u;
constant uint IC_TRY_UNIT = 0x01000000u;   // tries (bits 24..30): restarts after no closure by q
constant uint IC_CAP_MASK = 0x00ffffffu;   // q, the atom-domain index (bits 0..23)
// Cycle closure: |z_{s+k} - z_s| <= 1e-3 |z_s|.
constant float IC_TAU2    = 1e-6f;

struct ChunkStats {       // per-chunk counters, read back by the CPU
    uint        liveIn;
    uint        liveOut;
    atomic_uint iterationsLo;   // iterations executed by this chunk, 64-bit split
    atomic_uint iterationsHi;
    uint        fresh;          // pixels admitted by this chunk
    uint        admitted;       // pixels admitted in the level after this chunk
    atomic_uint interior;       // pixels finished as interior (derivative test or maxIter)
    atomic_uint crossed;        // pixels that reached maxIter unresolved in this chunk
    atomic_uint maxEscape;      // largest escape iteration in this chunk
    atomic_uint stepsLo;        // loop steps (BLA steps + perturbation steps), 64-bit split
    atomic_uint escapedPast[4]; // escapes at n >= softIter << k (k = 0..3), for auto iterations
    atomic_uint stepsHi;
    uint        pad2;
};

// BLA table layout (kb: techniques/bla; built by almondbread_core::bla): level `lev` holds one
// entry per block of 2^lev reference steps starting at m = 1 + j 2^lev, at offset[lev] + j.
// Levels minLevel ..= maxLevel are stored (a 1-step block costs as much as the exact step).
struct BlaParams {
    uint minLevel;
    uint maxLevel;
    uint lookupEvery; // lookups at m = 1 (mod lookupEvery), a power of two >= 2^minLevel
    uint pad1;
    uint offset[32];
};

// z_{m+l} = A z_m + B c, valid while |z| + K |c| < R.
struct BlaEntry {
    float2 a;
    float2 b;
    float  r;
    float  k;
};

// Deep BLA entry (M3b, kb: techniques/bla "M3b"): the same affine map and validity with every
// quantity carrying its own exponent: A = a 2^ae, B = b 2^be (complex mantissas, max-abs in
// [0.5, 1)), R = r 2^re, K = k 2^ke (r, k in [0.5, 1); r = 0: never valid).
struct BlaEntryX {
    float2 a;
    float2 b;
    int    ae;
    int    be;
    float  r;
    int    re;
    float  k;
    int    ke;
};

// A reference point in floatexp (`FloatExpComplex` on the Rust side): m * 2^e, the larger
// component's mantissa magnitude in [0.5, 1), zero as m = 0.
struct RefX {
    float2 m;
    int    e;
};

// Admission state of the running level, on the GPU so admission needs no CPU round trip:
// each chunk fills whatever live capacity the previous one freed.
struct LevelCounters {
    uint admitted;        // fresh pixels admitted so far (scanline order)
    uint freshStart;      // this chunk's first fresh pixel
    uint freshCount;      // this chunk's fresh pixels
    uint pad;
};

static inline float2 cmul(float2 a, float2 b) {
    return float2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}


// ---------------------------------------------------------------------------------------------
// Extended range for the deep kernel (M3b, kb: techniques/extended-range-floats). FE is a
// complex floatexp: m * 2^e with max(|m.x|, |m.y|) in [0.5, 1); zero is m = 0 with e = FE_ZERO,
// so it loses every alignment and comparison without special cases. Exponents stay far from
// int overflow: products add at most a few of them.
constant int FE_ZERO = -(1 << 28);

struct FE {
    float2 m;
    int    e;
};

// 2^k as float, k clamped to 127 above; 0 below the normal range (fast math flushes anyway).
static inline float pow2f(int k) {
    return k < -126 ? 0.0f : as_type<float>(uint(min(k, 127) + 127) << 23);
}

// m * 2^k for k in about [-252, 254] (two exact steps: a single 2^k would leave the float range).
static inline float2 scale2(float2 m, int k) {
    int h = k >> 1;
    return m * pow2f(h) * pow2f(k - h);
}

// a = f 2^k with f in [0.5, 1), for a normal positive float.
static inline int expOf(float a) {
    return int((as_type<uint>(a) >> 23) & 0xffu) - 126;
}

static inline FE feNorm(float2 m, int e) {
    float a = max(abs(m.x), abs(m.y));
    if (!(a > 0.0f)) return FE{float2(0.0f), FE_ZERO};
    int k = expOf(a);
    return FE{scale2(m, -k), e + k};
}

static inline FE feAdd(FE a, FE b) {
    if (a.e < b.e) { FE t = a; a = b; b = t; }
    int d = a.e - b.e;
    return d > 40 ? a : feNorm(a.m + scale2(b.m, -d), a.e);
}

static inline FE feMul(FE a, FE b) {
    return feNorm(cmul(a.m, b.m), a.e + b.e);
}

// |a| < |b|.
static inline bool feLess(FE a, FE b) {
    float na = dot(a.m, a.m), nb = dot(b.m, b.m);
    if (!(nb > 0.0f)) return false;
    if (!(na > 0.0f)) return true;
    int d = a.e - b.e;
    if (d > 2) return false;
    if (d < -2) return true;
    return na * pow2f(2 * d) < nb;
}

static inline float2 feFloat(FE a) {
    return scale2(a.m, max(a.e, -300));
}

// |a - b| <= tau |b| (tau^2 = IC_TAU2): the cycle search's closure test.
static inline bool feClose(FE a, FE b) {
    FE d = feAdd(a, FE{-b.m, b.e});
    float nd = dot(d.m, d.m), nb = dot(b.m, b.m);
    if (!(nd > 0.0f)) return true;
    if (!(nb > 0.0f)) return false;
    int k = d.e - b.e;
    if (k > 0) return false;
    if (k < -40) return true;
    return nd * pow2f(2 * k) <= IC_TAU2 * nb;
}

static inline FE feTwice(FE a) {
    return FE{a.m, a.e == FE_ZERO ? FE_ZERO : a.e + 1};
}

// After the delta's exponent e changed: s = 2^e and dd = c 2^-e for the scaled step. A delta
// far below |c| (2^-60) is re-expressed relative to c's exponent `ce`, so dd stays in range.
static inline void deepRescale(thread float2& w, thread int& e, thread float& s, thread float2& dd,
                               float2 dc, int scaleExp, int ce) {
    if (e != 0 && e < ce - 60) {
        w = scale2(w, e - ce);
        e = ce;
    }
    s = pow2f(e);
    dd = dc * pow2f(min(scaleExp - e, 100));
}

static inline FE refAt(device const RefX* refx, uint m) {
    RefX r = refx[m];
    return FE{r.m, (r.m.x == 0.0f && r.m.y == 0.0f) ? FE_ZERO : r.e};
}

// Regime boundaries of the deep loop: a delta of at least 2^-41 is a plain float32 (squared
// norms stay normal), below 2^-50 it is carried as mantissa and exponent.
constant int   DEEP_PLAIN_EXP = -40;
constant float DEEP_TINY2 = 0x1p-100f;
// |Z| below this: the scaled step and its tests go through floatexp (the dropped S w^2 term and
// rebases can only matter there).
constant float DEEP_SMALL_Z = 0x1p-32f;

// One thread: admits fresh pixels into free capacity, sizes the indirect dispatch for
// live + fresh pixels and clears the counters.
kernel void prepareChunk(constant IterateParams& p           [[buffer(0)]],
                         device uint* countIn                [[buffer(4)]],
                         device uint* countOut               [[buffer(5)]],
                         device ChunkStats* stats            [[buffer(6)]],
                         device uint* indirectArgs           [[buffer(7)]],
                         constant uint& threadsPerGroup      [[buffer(8)]],
                         device LevelCounters& level         [[buffer(9)]],
                         device uint* listCount              [[buffer(11)]])
{
    if (p.firstChunk != 0) {
        *countIn = 0;
        level.admitted = 0;
        if (p.listMode != 0) *listCount = 0;
    }
    uint live = *countIn;
    uint fresh = min(p.total - level.admitted, p.capacity - live);
    level.freshStart = level.admitted;
    level.freshCount = fresh;
    level.admitted += fresh;
    indirectArgs[0] = (live + fresh + threadsPerGroup - 1) / threadsPerGroup;
    indirectArgs[1] = 1;
    indirectArgs[2] = 1;
    *countOut = 0;
    stats[p.statSlot].liveIn = live;
    stats[p.statSlot].liveOut = 0;
    stats[p.statSlot].fresh = fresh;
    stats[p.statSlot].admitted = level.admitted;
    atomic_store_explicit(&stats[p.statSlot].interior, 0u, memory_order_relaxed);
    atomic_store_explicit(&stats[p.statSlot].crossed, 0u, memory_order_relaxed);
    atomic_store_explicit(&stats[p.statSlot].maxEscape, 0u, memory_order_relaxed);
    for (uint k = 0; k < 4; k++) atomic_store_explicit(&stats[p.statSlot].escapedPast[k], 0u, memory_order_relaxed);
    atomic_store_explicit(&stats[p.statSlot].iterationsLo, 0u, memory_order_relaxed);
    atomic_store_explicit(&stats[p.statSlot].iterationsHi, 0u, memory_order_relaxed);
    atomic_store_explicit(&stats[p.statSlot].stepsLo, 0u, memory_order_relaxed);
    atomic_store_explicit(&stats[p.statSlot].stepsHi, 0u, memory_order_relaxed);
}

// One thread: records the live count after the chunk for the CPU scheduler.
kernel void finishChunk(constant IterateParams& p            [[buffer(0)]],
                        device const uint* countOut          [[buffer(5)]],
                        device ChunkStats* stats             [[buffer(6)]])
{
    stats[p.statSlot].liveOut = *countOut;
}

// Perturbation: z = Z + dz, with Z the reference orbit.
//   dz_{n+1} = 2 Z_n dz_n + dz_n^2 + dc = (2 Z_n + dz_n) dz_n + dc
// Rebasing: when |Z_m + dz| < |dz|, or the reference runs out, continue from the start of
// the reference with dz := Z_m + dz. One reference then suffices (no glitch detection).
//
// Distance estimate: d = (dz/dc) * texelScale, i.e. the derivative per texel, so it stays in
// float32 range (kb: techniques/distance-estimation). d_{n+1} = 2 z_n d_n + texelScale.
//
// Interior (kb: techniques/interior-detection): w = dz_n/dz_1 of the full orbit z = Z + dz
// (not the delta's own derivative, which is only right for a periodic nucleus reference),
// w_1 = 1, w_{n+1} = 2 z_n w_n updated before z. Interior when |w| n^2 < eps (eps = 1e-3).
// Not Chéritat's fixed |w| < eps: an exterior orbit passing slowly through a parabolic gate
// (c near a bulb root, e.g. the -3/4 neck) has |w| dip to ~ C / n^2 mid-passage and recover
// afterwards, so a fixed threshold paints exterior wedges black at depth; interior w decays
// geometrically and beats n^2 within a few periods (kb: techniques/interior-detection, Pitfalls).
// The test uses max(|w.x|, |w.y|): no squared norm to underflow, and at most sqrt(2) looser.
// Exterior w grows without bound on long orbits, so it carries a power-of-two exponent:
// rescaling by 2^±60 is exact and never biases the test towards "interior".
//
// BLA (kb: techniques/bla, kBLA): at reference indices m = 1 (mod lookupEvery) the pixel takes the
// longest block starting at m whose validity |dz| + K |dc| < R holds (levels tested upwards;
// validity is monotone in the level) and applies dz = A dz + B dc, d = A d + B texelScale,
// w = A w; otherwise exact perturbation steps up to the next lookup index. A valid block keeps
// |dz| <= 2^-24 |2 Z| at every inner step, so neither a rebase nor an escape can fall inside it;
// both are tested after it as after a step. The chunk budget counts loop steps, not iterations
// (a block is one step of GPU work), and a block never runs past maxIter. Without kBLA the
// loop is the plain one (bit-identical fields). A lookup costs several exact steps (dependent
// table loads, SIMD divergence): the scheduler only takes this pipeline where the table's
// forecast promises long blocks (M4: deep views only).
kernel void iterate(texture2d<float, access::write> iterOut [[texture(0)]],
                    texture2d<float, access::write> deOut   [[texture(1)]],
                    constant IterateParams& p               [[buffer(0)]],
                    device const float2* ref                [[buffer(1)]],
                    device const PixelState* stateIn        [[buffer(2)]],
                    device PixelState* stateOut             [[buffer(3)]],
                    device const uint* countIn              [[buffer(4)]],
                    device atomic_uint* countOut            [[buffer(5)]],
                    device ChunkStats* stats                [[buffer(6)]],
                    device const LevelCounters& level       [[buffer(9)]],
                    device const uint* listIn               [[buffer(10)]],
                    device atomic_uint* listCount           [[buffer(11)]],
                    device uint* listOut                    [[buffer(12)]],
                    device const uint* order                [[buffer(13)]],
                    constant BlaParams& bla                 [[buffer(14)]],
                    device const BlaEntry* table            [[buffer(15)]],
                    device const RefX* refx                 [[buffer(16)]],
                    device const BlaEntryX* tableX          [[buffer(17)]],
                    texture2d<float, access::write> auxOut  [[texture(2), function_constant(kInteriorColor)]],
                    device const PixelStateIC* icIn         [[buffer(18), function_constant(kInteriorColor)]],
                    device PixelStateIC* icOut              [[buffer(19), function_constant(kInteriorColor)]],
                    uint tid                                [[thread_position_in_grid]],
                    uint tidInGroup                         [[thread_index_in_threadgroup]],
                    uint lane                               [[thread_index_in_simdgroup]],
                    uint sg                                 [[simdgroup_index_in_threadgroup]],
                    uint sgCount                            [[simdgroups_per_threadgroup]])
{
    const uint liveIn = *countIn;
    const uint freshStart = level.freshStart, freshCount = level.freshCount;
    const uint last = p.refLen - 1;
    const float2 tsDelta = float2(p.texelScale, 0.0f);

    PixelState st;
    bool active = true;     // this thread has a pixel
    bool fresh = false;
    if (tid < liveIn) {
        st = stateIn[tid];
    } else if (tid - liveIn < freshCount) {
        fresh = true;
        uint a = freshStart + (tid - liveIn);
        if (p.listMode == 2) {
            st.pixel = listIn[a];            // lists hold texel indices (y * width + x)
        } else {
            uint2 xy;
            active = tileTexel(a, p.tilesX, p.width, p.height, p.tileOrder, order, xy);
            st.pixel = xy.y * p.width + xy.x;
        }
    } else {
        active = false;
        st.pixel = 0;
    }

    uint2 gid = uint2(st.pixel % p.width, st.pixel / p.width);
    float2 pix = float2(gid) + p.jitter - p.halfSize;
    float2 off = float2(pix.x, -pix.y);
    off = float2(off.x * p.rot.x - off.y * p.rot.y, off.x * p.rot.y + off.y * p.rot.x);
    float2 dc = off * p.texelScale + p.refOffset;

    bool escaped = false, interior = false;
    float r2 = 0.0f;
    float2 z = 0.0f;
    uint iters = 0;
    uint steps = 0;

    // Interior cycle analysis (kInteriorColor): a pixel proven interior continues here instead
    // of finishing, see below; `inB` marks one already analysing (the main loop is done).
    // Before that, the main loop tracks the orbit's closest approach to 0 (atom domain): the
    // index q of the smallest |z_n| is a multiple of the period (Farey denominators near a
    // component's boundary), and the analysis looks for the closure only at divisors of q.
    // Exact steps only: BLA blocks are skipped (they are never valid where |Z| is small).
    PixelStateIC ic;
    ic.capFlags = 0;
    ic.k = 0;
    ic.minLog2 = 1e30f;
    if (kInteriorColor && active && !fresh) ic = icIn[tid];
    bool inB = kInteriorColor && (ic.capFlags & IC_ACTIVE) != 0;
    float icR2 = kInteriorColor ? exp2(min(2.0f * ic.minLog2, 127.0f)) : 0.0f;
    float icMinLog2 = ic.minLog2;
    uint icQ = ic.k;
#define IC_TRACK(r2v, nv) if (kInteriorColor && (r2v) < icR2) { icR2 = (r2v); icMinLog2 = 0.5f * log2(icR2); icQ = (nv); }

    if (active && !inB && kDeep) {
        // M3b deep loop (kb: techniques/extended-range-floats "Rescaled iterations", rebasing,
        // bla). The pixel's delta is z = w 2^e. Two regimes:
        //  P (e == 0): |z| >= 2^-50, w is the plain float32 delta; the loop is the plain one
        //    (squared-norm escape and rebase tests are safe at that size).
        //  T (e < 0): |z| < 2^-41 (hysteresis to P), w kept in [2^-16, 2^16] by renormalising.
        //    Step w <- (2Z + s w) w + dd with s = 2^e (0 when it underflows: then |S w^2| is far
        //    below float32 rounding of 2 Z w, since |Z| >= 2^-32 here) and dd = c 2^-e. No escape
        //    or rebase test is needed while |Z| >= 2^-32 > 2|z|; where |Z| is smaller, at m = 0
        //    and at the reference's end, the step and its tests run in floatexp with the
        //    reference point's own exponent (Pauldelbrot's "full iterations"; mathr).
        // Test, then step: a resumed pixel repeats its last test, which is idempotent.
        // Derivatives carry exponents: d = dz/dc * texel = dm 2^dExp, w' = dz/dz1 = zw 2^wExp.
        const FE C = feNorm(dc, p.scaleExp);
        const FE TS = feNorm(float2(p.texelScale, 0.0f), p.scaleExp);
        const float cAbsM = length(C.m);      // |c| = cAbsM 2^C.e
        float2 w, dm, zw;
        int e, dExp, wExp;
        uint m, n;
        if (fresh) {
            // z_1 = c, d_1 = texel, w_1 = 1.
            if (C.e >= DEEP_PLAIN_EXP) { w = feFloat(C); e = 0; } else { w = C.m; e = C.e; }
            dm = TS.m; dExp = TS.e;
            zw = float2(1.0f, 0.0f); wExp = 0;
            m = 1; n = 1;
            st.n = 0;   // the first iteration counts in this chunk
            steps = 1;
        } else {
            w = st.dz; e = st.e; dm = st.d; dExp = st.dExp; zw = st.w; wExp = st.wExp;
            m = st.m; n = st.n;
        }
        float s;
        float2 dd;
        deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
        float2 tsd = float2(TS.m.x * pow2f(min(TS.e - dExp, 100)), 0.0f);
        uint budget = p.chunkIters;
        const uint alignMask = kBLA ? bla.lookupEvery - 1u : 0u;
        uint lookedAt = 0xffffffffu;   // n of the last BLA lookup (one per lookup index visit)
        bool done = false;
        while (!done) {
            // ---- BLA (both regimes): at m = 1 (mod lookupEvery), the longest valid block.
            // Consecutive blocks need no test in between: a valid block's first step already
            // has |z| <= 2^-24 |2 Z_m|, so no rebase or escape is due at its start.
            if (kBLA && m != 0 && ((m - 1) & alignMask) == 0 && n < p.maxIter && budget > 0 && n != lookedAt) {
                lookedAt = n;
                FE zx = feNorm(w, e);
                const float zn = length(zx.m);
                const uint top = min(ctz(m - 1), bla.maxLevel);
                BlaEntryX best;
                uint len = 0;
                for (uint lev = bla.minLevel; lev <= top; lev++) {
                    const uint l = 1u << lev;
                    if (m + l > last || n + l > p.maxIter) break;
                    const BlaEntryX x = tableX[bla.offset[lev] + ((m - 1) >> lev)];
                    if (!(x.r > 0.0f)) break;
                    // |z| + K |c| < R, all relative to 2^re.
                    const int kce = x.ke + C.e - x.re;
                    const float t = x.r - (kce < -150 ? 0.0f : x.k * cAbsM * pow2f(min(kce, 64)));
                    if (!(t > 0.0f && zn * pow2f(clamp(zx.e - x.re, -127, 64)) < t)) break;
                    best = x;
                    len = l;
                }
                if (len != 0) {
                    const FE A = FE{best.a, best.ae}, B = FE{best.b, best.be};
                    zx = feAdd(feMul(A, zx), feMul(B, C));
                    if (kDE) {
                        FE dx = feAdd(feMul(A, feNorm(dm, dExp)), feMul(B, TS));
                        dm = dx.m; dExp = dx.e;
                        tsd = float2(TS.m.x * pow2f(min(TS.e - dExp, 100)), 0.0f);
                    }
                    if (kInterior) {
                        FE wx = feMul(A, feNorm(zw, wExp));
                        zw = wx.m; wExp = wx.e;
                        // (w is now dz_{n+len}/dz_1: the n^2 guard uses n + len.)
                        float nn = float(n + len);
                        if (max(abs(zw.x), abs(zw.y)) * nn * nn * pow2f(max(wExp, -126)) < p.interiorEps) {
                            interior = true;
                            n += len;
                            break;
                        }
                    }
                    m += len;
                    n += len;
                    budget--;
                    if (zx.e >= DEEP_PLAIN_EXP) { w = feFloat(zx); e = 0; } else { w = zx.m; e = zx.e; }
                    deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
                    // Tests follow in the regime loop (a valid block keeps |z| <= 2^-24 |2Z| at
                    // every inner step: no rebase or escape inside it).
                }
            }
            if (e == 0) {
                // ---- P regime: the plain loop (plus the BLA exit and the T entry).
                while (true) {
                    float2 Z = ref[m];
                    z = Z + w;
                    r2 = dot(z, z);
                    if (r2 > p.bailout2) { escaped = true; done = true; break; }
                    IC_TRACK(r2, n)
                    float w2 = dot(w, w);
                    if (r2 < w2 || m == last) {
                        w = z;
                        m = 0;
                        Z = 0.0f;
                        w2 = r2;
                    }
                    if (w2 < DEEP_TINY2) {
                        // Too small for squared tests: carry it with an exponent (T).
                        FE t = feNorm(w, 0);
                        w = t.m;
                        e = t.e == FE_ZERO ? C.e : t.e;
                        deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
                        break;
                    }
                    if (n >= p.maxIter || budget == 0) { done = true; break; }
                    if (kBLA && m != 0 && ((m - 1) & alignMask) == 0 && n != lookedAt) break;
                    if (kDE) dm = 2.0f * cmul(z, dm) + tsd;
                    if (kInterior) zw = 2.0f * cmul(z, zw);
                    // (Also every step once |z| > 16: escape is near and |2z| up to 2^17 per step
                    // would overflow the DE mantissa within 8 steps; R5.)
                    if ((n & 7u) == 0u || max(abs(z.x), abs(z.y)) < 0x1p-16f
                        || (kDE && max(abs(z.x), abs(z.y)) > 16.0f)) {
                        if (kDE) {
                            float a = max(abs(dm.x), abs(dm.y));
                            if (a > 0x1p40f || (a < 0x1p-40f && a > 0.0f)) {
                                FE t = feNorm(dm, dExp);
                                dm = t.m; dExp = t.e;
                                tsd = float2(TS.m.x * pow2f(min(TS.e - dExp, 100)), 0.0f);
                            }
                        }
                        if (kInterior) {
                            float a = max(abs(zw.x), abs(zw.y));
                            if (a > 0x1p40f || (a < 0x1p-40f && a > 0.0f)) {
                                FE t = feNorm(zw, wExp);
                                zw = t.m; wExp = t.e;
                                a = max(abs(zw.x), abs(zw.y));
                            }
                            if (a * (float(n) * float(n)) * pow2f(max(wExp, -126)) < p.interiorEps) {
                                interior = true;
                                done = true;
                                break;
                            }
                        }
                    }
                    w = cmul(2.0f * Z + w, w) + dd;
                    m++;
                    n++;
                    budget--;
                }
            } else {
                // ---- T regime: scaled steps; floatexp where |Z| is small.
                while (true) {
                    const float2 Z = ref[m];
                    if (m == 0 || m == last || max(abs(Z.x), abs(Z.y)) < DEEP_SMALL_Z) {
                        FE zx = feNorm(w, e);
                        FE Zx = refAt(refx, m);
                        FE zf = feAdd(Zx, zx);
                        if (m == last && zf.e > 0) {
                            // Only an escaped reference ends outside the bailout.
                            z = feFloat(zf);
                            r2 = dot(z, z);
                            if (r2 > p.bailout2) { escaped = true; done = true; break; }
                        }
                        if (m != 0 && (m == last || feLess(zf, zx))) {
                            zx = zf;
                            m = 0;
                            Zx = FE{float2(0.0f), FE_ZERO};
                        }
                        if (kInteriorColor) {
                            float zl = length(zf.m);
                            float l = zl > 0.0f ? log2(zl) + float(zf.e) : -1e30f;
                            if (l < icMinLog2) { icMinLog2 = l; icR2 = exp2(max(2.0f * l, -160.0f)); icQ = n; }
                        }
                        if (zx.e >= DEEP_PLAIN_EXP) {
                            w = feFloat(zx); e = 0; s = 1.0f;
                            dd = dc * pow2f(min(p.scaleExp, 100));
                            break;
                        }
                        w = zx.m; e = zx.e;
                        if (n >= p.maxIter || budget == 0) { done = true; break; }
                        if (kBLA && m != 0 && ((m - 1) & alignMask) == 0 && n != lookedAt) {
                            deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
                            break;
                        }
                        // Derivatives with the full orbit value zf = Z + z (floatexp).
                        if (kDE) {
                            FE dx = feAdd(feMul(FE{zf.m, zf.e + 1}, feNorm(dm, dExp)), TS);
                            dm = dx.m; dExp = dx.e;
                            tsd = float2(TS.m.x * pow2f(min(TS.e - dExp, 100)), 0.0f);
                        }
                        if (kInterior) {
                            FE wx = feMul(FE{zf.m, zf.e + 1}, feNorm(zw, wExp));
                            zw = wx.m; wExp = wx.e;
                        }
                        // z <- (2Z + z) z + c in floatexp.
                        zx = feAdd(feMul(feAdd(FE{Zx.m, Zx.e + 1}, zx), zx), C);
                        m++;
                        n++;
                        budget--;
                        // (Tested after every full step: they happen where the orbit passes
                        // closest to 0, the one place an interior pixel's |w| is small.)
                        if (kInterior
                            && max(abs(zw.x), abs(zw.y)) * (float(n) * float(n)) * pow2f(max(wExp, -126)) < p.interiorEps) {
                            interior = true;
                            done = true;
                            w = zx.m; e = zx.e;
                            break;
                        }
                        if (zx.e >= DEEP_PLAIN_EXP) {
                            w = feFloat(zx); e = 0; s = 1.0f;
                            dd = dc * pow2f(min(p.scaleExp, 100));
                            break;
                        }
                        w = zx.m;
                        e = zx.e;
                        deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
                        continue;
                    }
                    if (n >= p.maxIter || budget == 0) { done = true; break; }
                    if (kBLA && m != 0 && ((m - 1) & alignMask) == 0 && n != lookedAt) break;
                    const float2 sw = s * w;
                    const float2 zf = Z + sw;
                    IC_TRACK(dot(zf, zf), n)
                    if (kDE) dm = 2.0f * cmul(zf, dm) + tsd;
                    if (kInterior) zw = 2.0f * cmul(zf, zw);
                    if ((n & 7u) == 0u || max(abs(zf.x), abs(zf.y)) < 0x1p-16f) {
                        if (kDE) {
                            float a = max(abs(dm.x), abs(dm.y));
                            if (a > 0x1p40f || (a < 0x1p-40f && a > 0.0f)) {
                                FE t = feNorm(dm, dExp);
                                dm = t.m; dExp = t.e;
                                tsd = float2(TS.m.x * pow2f(min(TS.e - dExp, 100)), 0.0f);
                            }
                        }
                        if (kInterior) {
                            float a = max(abs(zw.x), abs(zw.y));
                            if (a > 0x1p40f || (a < 0x1p-40f && a > 0.0f)) {
                                FE t = feNorm(zw, wExp);
                                zw = t.m; wExp = t.e;
                                a = max(abs(zw.x), abs(zw.y));
                            }
                            if (a * (float(n) * float(n)) * pow2f(max(wExp, -126)) < p.interiorEps) {
                                interior = true;
                                done = true;
                                break;
                            }
                        }
                    }
                    w = cmul(2.0f * Z + sw, w) + dd;
                    m++;
                    n++;
                    budget--;
                    const float a = max(abs(w.x), abs(w.y));
                    if (a > 0x1p16f || a < 0x1p-16f) {
                        if (a > 0.0f) {
                            const int k = expOf(a);
                            w = scale2(w, -k);
                            e += k;
                            if (e >= DEEP_PLAIN_EXP) {
                                w = scale2(w, e); e = 0; s = 1.0f;
                                dd = dc * pow2f(min(p.scaleExp, 100));
                                break;
                            }
                            deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
                        }
                    }
                }
            }
        }
        iters += n - st.n;
        steps += p.chunkIters - min(budget, p.chunkIters);
        st.dz = w; st.e = e; st.d = dm; st.dExp = dExp; st.w = zw; st.wExp = wExp; st.m = m; st.n = n;
    } else if (active && !inB) {
        if (fresh) {
            // First step done here, from z_0 = 0: z_1 = c, dz_1 = dc, d_1 = texelScale,
            // w_1 = 1 (the generic step would multiply w by z_0 = 0).
            st.dz = dc;
            st.d = tsDelta;
            st.w = float2(1.0f, 0.0f);
            st.wExp = 0;
            st.m = 1;
            st.n = 1;
            iters = 1;
            steps = 1;
            z = ref[1] + st.dz;
            r2 = dot(z, z);
            IC_TRACK(r2, 1u)
            if (r2 > p.bailout2) {
                escaped = true;
            } else if (r2 < dot(st.dz, st.dz) || st.m == last) {
                st.dz = z;
                st.m = 0;
            }
        } else {
            z = ref[st.m] + st.dz;
        }

        // Locals for the hot loop (the compiler keeps them in registers).
        float2 dz = st.dz, d = st.d, w = st.w;
        uint m = st.m, n = st.n;
        int wExp = st.wExp;
        const float cn = kBLA ? length(dc) : 0.0f;
        // Lookups only at m = 1 (mod bla.lookupEvery): a lookup costs several exact steps
        // (dependent table loads, divergence), and a rule on m alone keeps results independent
        // of how chunks cut the orbit (shown frame = fresh render).
        const uint alignMask = kBLA ? bla.lookupEvery - 1u : 0u;
        uint budget = p.chunkIters;
        while (!escaped && !interior && n < p.maxIter && budget > 0) {
            // BLA phase (kBLA): the longest valid block starting at m (levels lev <= ctz(m - 1)
            // have an entry here); |dz| + K |dc| < R is tested squared. Blocks are taken while
            // valid and aligned; then exact steps to the next lookup index follow.
            if (kBLA && m != 0 && ((m - 1) & alignMask) == 0) {
                const uint top = min(ctz(m - 1), bla.maxLevel);
                const float zn2 = dot(dz, dz);
                float2 A = 0.0f, B = 0.0f;
                uint len = 0;
                for (uint lev = bla.minLevel; lev <= top; lev++) {
                    const uint l = 1u << lev;
                    if (m + l > last || n + l > p.maxIter) break;
                    const BlaEntry e = table[bla.offset[lev] + ((m - 1) >> lev)];
                    const float t = e.r - e.k * cn;
                    if (!(t > 0.0f && zn2 < t * t)) break;
                    A = e.a;
                    B = e.b;
                    len = l;
                }
                if (len != 0) {
                    budget--;
                    if (kDE) d = cmul(A, d) + B * p.texelScale;
                    if (kInterior) {
                        // |A| <= 2^40 (table build): rescale first so |w A| stays in range,
                        // then the usual test.
                        float a = max(abs(w.x), abs(w.y));
                        if (a > 0x1p60f) {
                            w *= 0x1p-60f;
                            wExp += 60;
                        }
                        w = cmul(A, w);
                        a = max(abs(w.x), abs(w.y));
                        if (a > 0x1p60f) {
                            w *= 0x1p-60f;
                            wExp += 60;
                        } else if (wExp > 0 && a < 0x1p-60f) {
                            w *= 0x1p60f;
                            wExp -= 60;
                        } else if (wExp == 0 && a * (float(n + len) * float(n + len)) < p.interiorEps) {
                            // (w is now dz_{n+len}/dz_1: the n^2 guard against parabolic
                            // exteriors must use n + len.)
                            interior = true;
                            break;
                        }
                    }
                    dz = cmul(A, dz) + cmul(B, dc);
                    m += len;
                    n += len;
                    z = ref[m] + dz;
                    r2 = dot(z, z);
                    IC_TRACK(r2, n)
                    if (r2 > p.bailout2) { escaped = true; break; }
                    if (r2 < dot(dz, dz) || m == last) {
                        dz = z;
                        m = 0;
                    }
                    continue;
                }
            }
            // Exact perturbation steps: the whole budget without BLA; with BLA up to the next
            // lookup index (m = 0: one step), or until a rebase brings the pixel back to m = 1.
            uint run = kBLA ? min(budget, alignMask + 1u - ((m - 1u) & alignMask)) : budget;
            const uint stop = min(p.maxIter, n + run);
            const uint n0 = n;
            while (n < stop) {
                if (kDE) d = 2.0f * cmul(z, d) + tsDelta;
                if (kInterior) {
                    w = 2.0f * cmul(z, w);
                }
                // Test and rescale only every 8th iteration: half the extra cost of the
                // derivative test. Safe from overflow: |w| <= 2^60 after a check, and |2z| > 4
                // only in the few steps before escape (|z| 2 -> 4 -> 16 -> 256 -> 65536), so
                // 8 steps grow it by at most ~2^42 (float32 max 2^128). Detection is at most 7
                // iterations late.
                if (kInterior && (n & 7u) == 0u) {
                    float a = max(abs(w.x), abs(w.y));
                    if (a > 0x1p60f) {
                        w *= 0x1p-60f;
                        wExp += 60;
                    } else if (wExp > 0 && a < 0x1p-60f) {
                        w *= 0x1p60f;
                        wExp -= 60;
                    } else if (wExp == 0 && a * (float(n) * float(n)) < p.interiorEps) {
                        interior = true;
                        break;
                    }
                }
                dz = cmul(2.0f * ref[m] + dz, dz) + dc;
                m++;
                n++;
                z = ref[m] + dz;
                r2 = dot(z, z);
                IC_TRACK(r2, n)
                if (r2 > p.bailout2) { escaped = true; break; }
                if (r2 < dot(dz, dz) || m == last) {
                    dz = z;
                    m = 0;
                }
                if (kBLA && m == 1) break;
            }
            budget -= max(n - n0, 1u);
        }
        iters += n - st.n;
        steps += p.chunkIters - min(budget, p.chunkIters);
        st.dz = dz; st.d = d; st.w = w; st.m = m; st.n = n; st.wExp = wExp;
    }

    // ---- Interior cycle analysis (kInteriorColor; kb: techniques/coloring-methods, "Interior
    // colouring as built"). A pixel proven interior (or at the limit without the test, which
    // the flat colouring draws as interior) keeps iterating its orbit, now settled near the
    // attracting cycle, from z_s: exact perturbation steps in floatexp (any depth, no BLA)
    // for up to q steps (the atom-domain index tracked above). The period p is the first k
    // that divides q at which the orbit closes, |z_{s+k} - z_s| <= 1e-3 |z_s|, with an
    // attracting multiplier |lambda_k| < 1. (Closure alone is not enough at depth: near a
    // repelling or parabolic cycle the orbit comes back that close too, e.g. after 39 steps at
    // sea-8007, whose period is 8007; such k do not divide q.) Along the way it accumulates the
    // multiplier lambda = (f^p)'(z_s) = prod 2 z, the derivatives dc = d f^p/dc (per texel),
    // dzdz and dcdz for the interior distance estimate (1 - |lambda|^2) /
    // |dcdz + dzdz dc / (1 - lambda)| (Wikipedia "Interior distance estimation"; mathr), and
    // the smallest |z| over the cycle. No closure by k = q: one retry from where it stopped
    // (the start may not have been settled), then it gives up (period 0: the flat interior
    // colour). The class never changes (verify and the shown-frame checks compare classes), and
    // the loop state lives in a separate buffer, so pipelines without the analysis are
    // unchanged.
    bool bDone = false;
    uint bPeriod = 0;
    float2 bLambda = 0.0f;
    float bDistance = 0.0f;
    if (kInteriorColor && active && !inB) {
        ic.minLog2 = icMinLog2;
        ic.k = icQ;
    }
    if (kInteriorColor && active && !inB && !escaped && (interior || (!kInterior && st.n >= p.maxIter))) {
        FE dzx = feNorm(st.dz, kDeep ? st.e : 0);
        st.dz = dzx.m; st.e = dzx.e;               // the delta as floatexp from here on
        st.w = float2(1.0f, 0.0f); st.wExp = 0;     // lambda
        st.d = 0.0f; st.dExp = FE_ZERO;            // dc
        ic.k = 0;
        ic.capFlags = IC_ACTIVE | uint(clamp(icQ, 1u, IC_CAP_MASK));
        inB = true;
    }
    if (inB) {
        const FE C = feNorm(dc, p.scaleExp);
        const FE TS = feNorm(float2(p.texelScale, 0.0f), p.scaleExp);
        FE dzx = feNorm(st.dz, st.e);
        FE lam = feNorm(st.w, st.wExp);
        FE dcx = feNorm(st.d, st.dExp);
        FE zz = feNorm(ic.dzdz, ic.dzdzExp);
        FE cz = feNorm(ic.dcdz, ic.dcdzExp);
        FE zs = feNorm(ic.zs, ic.zsExp);
        float minLog2 = ic.minLog2;
        uint m = st.m, n = st.n, k = ic.k;
        const uint cap = ic.capFlags & IC_CAP_MASK;
        const uint budget = p.chunkIters > steps ? p.chunkIters - steps : 0u;
        uint used = 0;
        while (used < budget) {
            FE Zx = kDeep ? refAt(refx, m) : feNorm(ref[m], 0);
            FE zf = feAdd(Zx, dzx);
            if (m == last || (m != 0 && feLess(zf, dzx))) {
                dzx = zf;
                m = 0;
                Zx = FE{float2(0.0f), FE_ZERO};
            }
            // Escapes after all (a false interior, very rare): give up, keep the class.
            if (zf.e > 16) { bDone = true; break; }
            if (k == 0) {
                zs = zf;
                lam = feNorm(float2(1.0f, 0.0f), 0);
                dcx = FE{float2(0.0f), FE_ZERO};
                zz = dcx;
                cz = dcx;
                minLog2 = 1e30f;
            } else if (cap % k == 0 && feClose(zf, zs) && feLess(lam, feNorm(float2(1.0f, 0.0f), 0))) {
                bDone = true;
                bPeriod = k;
                break;
            }
            float zl = length(zf.m);
            minLog2 = min(minLog2, zl > 0.0f ? log2(zl) + float(zf.e) : -1e30f);
            const FE z2 = feTwice(zf);
            // Old values on the right (derivatives first, then z).
            cz = feAdd(feMul(z2, cz), feMul(feTwice(dcx), lam));
            zz = feAdd(feMul(feTwice(lam), lam), feMul(z2, zz));
            dcx = feAdd(feMul(z2, dcx), TS);
            lam = feMul(z2, lam);
            dzx = feAdd(feMul(feAdd(feTwice(Zx), dzx), dzx), C);
            m++;
            n++;
            k++;
            used++;
            if (k > cap) {
                // Not closed (or not yet settled: slow convergence near a component's boundary):
                // start again from here, up to max(2q, q + 4096) steps in all.
                uint tries = ((ic.capFlags >> 24) & 0x7fu) + 1u;
                if (tries < 127u && tries * cap < max(cap, 4096u)) {
                    ic.capFlags = (ic.capFlags & ~(0x7fu << 24)) | (tries << 24);
                    k = 0;
                } else {
                    bDone = true;
                    break;
                }
            }
        }
        iters += used;
        steps += used;
        st.dz = dzx.m; st.e = dzx.e; st.w = lam.m; st.wExp = lam.e; st.d = dcx.m; st.dExp = dcx.e;
        st.m = m; st.n = n;
        ic.zs = zs.m; ic.zsExp = zs.e; ic.k = k;
        ic.dzdz = zz.m; ic.dzdzExp = zz.e; ic.dcdz = cz.m; ic.dcdzExp = cz.e; ic.minLog2 = minLog2;
        if (bPeriod != 0) {
            bLambda = feFloat(lam);
            // Interior distance in texels (dc and dcdz carry the texel size).
            float2 one = float2(1.0f, 0.0f) - bLambda;
            float2 inv = float2(one.x, -one.y) / max(dot(one, one), 1e-30f);
            FE den = feAdd(cz, feMul(zz, feMul(dcx, feNorm(inv, 0))));
            float num = 1.0f - dot(bLambda, bLambda);
            float dl = length(den.m);
            bDistance = (num > 0.0f && dl > 0.0f)
                ? min(num / dl * pow2f(clamp(-den.e, -126, 127)), 1e30f) : 0.0f;
        }
    }

    bool alive = active && (inB ? !bDone : (!escaped && !interior && st.n < p.maxIter));
    // Reached the limit neither escaped nor proven interior: unresolved, never drawn as
    // interior (with the test on), and collected for an extension pass.
    bool crossed = active && !inB && !alive && !escaped && !interior;
    if (kInteriorColor && active && !alive && inB) {
        // Interior with its cycle analysed: .y = period (0: none found), de = (|lambda|,
        // arg lambda in turns), aux = (interior distance in texels, log2 min |z| over the cycle).
        iterOut.write(float4(ITER_INTERIOR, float(bPeriod), 0.0f, 0.0f), gid);
        deOut.write(float4(length(bLambda), atan2(bLambda.y, bLambda.x) * 0.15915494f, 0.0f, 0.0f), gid);
        auxOut.write(float4(bDistance, bPeriod != 0 ? ic.minLog2 : 0.0f, 0.0f, 0.0f), gid);
    } else if (active && !alive) {
        if (escaped) {
            // Smooth iteration count n + s with s = 1 - log2(log|z|), stored split: the exact
            // escape iteration n (float32 holds integers to 2^24) and the small offset s in
            // [-3.47, -2.47) for bailout 2^16, so large counts keep their fraction and the
            // verifier sees n.
            float s = 1.0f - log2(0.5f * log(r2));
            iterOut.write(float4(float(st.n), s, 0.0f, 0.0f), gid);
            if (kDE) {
                float zr = sqrt(r2);
                float2 dd = st.d;
                int dShift = 0;
                if (kDeep) {
                    // The deep DE mantissa is renormalised every 8 steps only, and grows by up to
                    // |2z| ~ 2^17 per step just before escape: |d|^2 can overflow (de = 0, dotted
                    // outline arcs in smooth exterior; found in R5). Normalise it here.
                    const float a = max(abs(dd.x), abs(dd.y));
                    if (a > 0.0f && isfinite(a)) {
                        dShift = expOf(a);
                        dd = scale2(dd, -dShift);
                    }
                }
                float dl = length(dd);
                float de = dl > 0.0f ? 2.0f * zr * log(zr) / dl : 0.0f;   // in texels
                // (Clamp after scaling: the mantissa alone can pass 65000 while the distance does
                // not; clamping first drew thick outline bands in deep views, found in R5.)
                if (kDeep) de = min(de * pow2f(clamp(-(st.dExp + dShift), -126, 100)), 65000.0f);
                float2 u = cmul(z, float2(dd.x, -dd.y));                    // direction of z / d
                deOut.write(float4(min(de, 65000.0f), atan2(u.y, u.x), 0.0f, 0.0f), gid);
            } else {
                deOut.write(float4(1000.0f, 0.0f, 0.0f, 0.0f), gid);
            }
        } else if (interior || !kInterior) {
            // Proven interior, or the hard limit without the test (no way to tell: interior).
            iterOut.write(float4(ITER_INTERIOR, interior ? 1.0f : 0.0f, 0.0f, 0.0f), gid);
            deOut.write(float4(0.0f), gid);
        } else {
            iterOut.write(float4(ITER_UNRESOLVED, float(p.softIter), 0.0f, 0.0f), gid);
            deOut.write(float4(0.0f), gid);
        }
    } else if (alive && fresh && p.listMode != 2) {
        // (An extension pixel keeps showing "unresolved" until it resolves.)
        iterOut.write(float4(ITER_PENDING, 0.0f, 0.0f, 0.0f), gid);
    }
    // Unresolved at the limit: remembered for the extension level (one atomic per
    // threadgroup, like the live-list compaction below).
    bool collect = p.listMode != 0 && crossed && kInterior;

    // Compaction: one atomic per threadgroup appends its live pixels contiguously, in thread
    // order. Threadgroups start roughly in index order, so survivors stay roughly in scanline
    // order and a SIMD-group keeps spatially close pixels (similar reference index m, coherent
    // ref[] reads). Per-SIMD-group atomics scrambled the order chunk after chunk: 44% more GPU
    // time on a 2e5-iteration view at 16 ms chunks on M1 Pro (kb: synthesis/benchmarks).
    threadgroup uint sgLive[32];
    threadgroup uint sgIters[32];
    threadgroup uint sgSteps[32];
    threadgroup uint sgInterior[32];
    threadgroup uint sgCrossed[32];
    threadgroup uint sgMaxEscape[32];
    threadgroup uint4 sgPast[32];
    threadgroup uint sgCollect[32];
    threadgroup uint tgListBase;
    threadgroup uint tgBase;
    uint live = alive ? 1u : 0u;
    uint offset = simd_prefix_exclusive_sum(live);
    uint total = simd_sum(live);
    uint simdIters = simd_sum(iters);
    uint simdSteps = simd_sum(steps);
    uint simdInterior = simd_sum((active && !alive && !escaped) ? 1u : 0u);
    uint simdCrossed = simd_sum(crossed ? 1u : 0u);
    uint collectOffset = simd_prefix_exclusive_sum(collect ? 1u : 0u);
    uint simdCollect = simd_sum(collect ? 1u : 0u);
    uint escN = (active && escaped) ? st.n : 0u;
    uint simdMaxEscape = simd_max(escN);
    // Escapes past the nominal limit: only the extension produces them, so these sums are
    // nonzero in few chunks; packed 4 x 8 bits per SIMD-group would overflow, so 4 sums.
    uint4 past = uint4(escN >= p.softIter && escN > 0u, escN >= (p.softIter << 1) && escN > 0u,
                       escN >= (p.softIter << 2) && escN > 0u, escN >= (p.softIter << 3) && escN > 0u);
    uint4 simdPast = uint4(simd_sum(past.x), simd_sum(past.y), simd_sum(past.z), simd_sum(past.w));
    if (lane == 0) {
        sgLive[sg] = total;
        sgIters[sg] = simdIters;
        sgSteps[sg] = simdSteps;
        sgInterior[sg] = simdInterior;
        sgCrossed[sg] = simdCrossed;
        sgMaxEscape[sg] = simdMaxEscape;
        sgPast[sg] = simdPast;
        sgCollect[sg] = simdCollect;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tidInGroup == 0) {
        uint sum = 0, itSum = 0, stepSum = 0, intSum = 0, crossSum = 0, escMax = 0;
        uint4 pastSum = 0;
        uint collectSum = 0;
        for (uint i = 0; i < sgCount; i++) {
            uint t = sgLive[i];
            sgLive[i] = sum;
            sum += t;
            itSum += sgIters[i];
            stepSum += sgSteps[i];
            intSum += sgInterior[i];
            crossSum += sgCrossed[i];
            escMax = max(escMax, sgMaxEscape[i]);
            pastSum += sgPast[i];
            uint c = sgCollect[i];
            sgCollect[i] = collectSum;
            collectSum += c;
        }
        tgListBase = collectSum > 0 ? atomic_fetch_add_explicit(listCount, collectSum, memory_order_relaxed) : 0;
        for (uint k = 0; k < 4; k++) {
            if (pastSum[k] > 0) atomic_fetch_add_explicit(&stats[p.statSlot].escapedPast[k], pastSum[k], memory_order_relaxed);
        }
        if (escMax > 0) atomic_fetch_max_explicit(&stats[p.statSlot].maxEscape, escMax, memory_order_relaxed);
        if (intSum > 0) atomic_fetch_add_explicit(&stats[p.statSlot].interior, intSum, memory_order_relaxed);
        if (crossSum > 0) atomic_fetch_add_explicit(&stats[p.statSlot].crossed, crossSum, memory_order_relaxed);
        tgBase = sum > 0 ? atomic_fetch_add_explicit(countOut, sum, memory_order_relaxed) : 0;
        // 64-bit count from two native uint atomics: every add that wraps the low word
        // carries exactly once. (A float atomic here cost ~25 ms per million-pixel chunk on
        // M1 Pro: Apple GPUs serialise contended float atomics.)
        if (itSum > 0) {
            uint old = atomic_fetch_add_explicit(&stats[p.statSlot].iterationsLo, itSum, memory_order_relaxed);
            if (old + itSum < old) atomic_fetch_add_explicit(&stats[p.statSlot].iterationsHi, 1u, memory_order_relaxed);
        }
        if (stepSum > 0) {
            uint old = atomic_fetch_add_explicit(&stats[p.statSlot].stepsLo, stepSum, memory_order_relaxed);
            if (old + stepSum < old) atomic_fetch_add_explicit(&stats[p.statSlot].stepsHi, 1u, memory_order_relaxed);
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (alive) {
        stateOut[tgBase + sgLive[sg] + offset] = st;
        if (kInteriorColor) icOut[tgBase + sgLive[sg] + offset] = ic;
    }
    if (collect) {
        uint slot = tgListBase + sgCollect[sg] + collectOffset;
        if (slot < p.listCapacity) listOut[slot] = st.pixel;   // overflow: stays unresolved
    }
}

struct ColorParams {
    float4 interior;      // linear RGB
    float4 edge;          // linear RGB of the boundary outline
    float4 glowColor;     // linear RGB of the glow near the boundary
    float2 light;         // unit light direction in the complex plane
    float  density;       // palette cycles per iteration (or per log unit)
    float  offset;        // palette phase, 0..1
    float  ratio;         // iteration texels per output pixel (2 = 2x2 supersampling, 0.5 = preview)
    float  shading;       // 0..1 strength of normal-map lighting
    float  outline;       // 0..1 strength of distance-estimate outline
    float  outlineWidth;  // in output pixels
    uint   paletteSize;
    uint   logScale;      // 1: colour by log(mu + 1)
    uint   smooth;        // 0: banded (integer) iteration count
    uint   admitted;      // texels (admission order) of the main level written this frame
    float  hasUnder;      // 1: unfinished pixels sample the coloured preview (bilinear)
    uint   linearOut;     // 1: write linear colour (preview colour texture), else sRGB + dither
    uint   admittedOnGPU; // 1: the main level is running, take `admitted` from LevelCounters
    float  deScale;       // iteration texels per *final* output pixel, for DE outline widths
    uint2  srcSize;       // used region of the iteration textures (the level size)
    uint2  outSize;       // used region of the output texture
    float2 underSize;     // used region of the preview colour texture, in texels
    uint   tilesX;        // tiles per row of the main level (admission order)
    uint   tileOrder;     // 0: raster tile order; 1: rank[] (buffer 3)
    float  specular;      // 0..1 strength of the specular highlight
    float  shininess;     // Blinn-Phong exponent
    float  lightHeight;   // elevation of the light (0 grazing ... large: from the front)
    float  glow;          // 0..1 strength of the glow near the boundary
    float  glowWidth;     // glow falloff, output pixels
    uint   paletteLevels; // pre-filtered palette levels (1024, 512, ... entries), level 0 first
    float  paletteFilter; // 1: filter the palette by the colour gradient (palette anti-aliasing)
    uint   interiorMode;  // 0 flat, 1 bubbles, 2 period, 3 angle, 4 distance (0 unless the
                          // textures hold the cycle analysis)
    float  interiorDensity; // palette cycles of the interior modes
    float  interiorStripes; // angle mode: stripes per turn
    float2 hueRot;        // palette hue turn (cos, sin) in OKLab (cinematic hue drift); y = 0: none
    float4 deep;          // dense regions: calm colour (linear RGB), w = 1 on (0: mean grey)
    float4 filament;      // dense regions: colour of the thin lines along the boundary
};

// Palette lookup filtered by the pixel's footprint in palette space (kb: techniques/coloring-methods,
// FL "palette anti-aliasing"): the smooth count changes by about 1 / (d ln 2) per pixel at
// distance d pixels from the set (d = distance estimate, mu = -log2 G of the potential G and
// |grad G| ~ G / d), so near the boundary the palette cycles several times per pixel and a
// point lookup is noise. The palette is stored as a cyclic mip chain (box-filtered halvings);
// the lookup picks the level whose texel matches the footprint. Converges to what an
// infinitely supersampled image would show for that pixel's colour band average.
static inline float3 paletteAt(float t, float lod, constant ColorParams& c, device const float4* pal) {
    t = fract(t);
    uint l0 = uint(lod);
    uint l1 = min(l0 + 1, c.paletteLevels - 1);
    float3 col[2];
    for (uint k = 0; k < 2; k++) {
        uint l = k == 0 ? l0 : l1;
        uint size = c.paletteSize >> l;
        uint base = 2u * c.paletteSize - (2u * c.paletteSize >> l);   // sum of the larger levels
        float x = t * float(size) - 0.5f;
        float fl = floor(x);
        uint i0 = uint(int(fl) + int(size)) % size;
        uint i1 = (i0 + 1) % size;
        col[k] = mix(pal[base + i0].rgb, pal[base + i1].rgb, x - fl);
    }
    float3 rgb = mix(col[0], col[1], lod - float(l0));
    if (c.hueRot.y != 0.0f) {
        // Cinematic hue drift (kb: decisions/controls "Cinematic effects"): turn the hue in
        // OKLab (lightness and chroma kept), linear sRGB in and out (Ottosson's matrices).
        float3 lms = float3(0.4122214708f * rgb.r + 0.5363325363f * rgb.g + 0.0514459929f * rgb.b,
                            0.2119034982f * rgb.r + 0.6806995451f * rgb.g + 0.1073969566f * rgb.b,
                            0.0883024619f * rgb.r + 0.2817188376f * rgb.g + 0.6299787005f * rgb.b);
        lms = pow(max(lms, 0.0f), float3(1.0f / 3.0f));
        float L = 0.2104542553f * lms.x + 0.7936177850f * lms.y - 0.0040720468f * lms.z;
        float A = 1.9779984951f * lms.x - 2.4285922050f * lms.y + 0.4505937099f * lms.z;
        float B = 0.0259040371f * lms.x + 0.7827717662f * lms.y - 0.8086757660f * lms.z;
        float A2 = A * c.hueRot.x - B * c.hueRot.y;
        float B2 = A * c.hueRot.y + B * c.hueRot.x;
        float l_ = L + 0.3963377774f * A2 + 0.2158037573f * B2;
        float m_ = L - 0.1055613458f * A2 - 0.0638541728f * B2;
        float s_ = L - 0.0894841775f * A2 - 1.2914855480f * B2;
        l_ = l_ * l_ * l_; m_ = m_ * m_ * m_; s_ = s_ * s_ * s_;
        rgb = max(float3(4.0767416621f * l_ - 3.3077115913f * m_ + 0.2309699292f * s_,
                         -1.2684380046f * l_ + 2.6097574011f * m_ - 0.3413193965f * s_,
                         -0.0041960863f * l_ - 0.7034186147f * m_ + 1.7076372176f * s_), 0.0f);
    }
    return rgb;
}

// The palette colour of a smooth count, filtered by the pixel's footprint in palette space
// (kb: techniques/coloring-methods, FL "palette anti-aliasing"): the smooth count changes by
// about 1 / (d ln 2) per pixel at distance d pixels from the set (mu = -log2 G of the
// potential G, and |grad G| ~ G / d), so near the boundary the palette cycles several times
// per pixel and a point lookup is noise. The palette is stored as a cyclic mip chain
// (box-filtered halvings); the lookup takes the level whose entries match the footprint,
// which is what infinite supersampling would average to. `dense` (0..1) says how far the
// filter had to go (lod 7 -> 10: 1/8 to a whole palette cycle per pixel).
static inline float3 paletteColor(float2 it, float deTexels, constant ColorParams& c, device const float4* pal,
                                  float pixelsPerTexel, thread float& dense, thread float& calm) {
    float t, dtdmu;
    float mu = it.x + (c.smooth != 0 ? it.y : 0.0f);
    if (c.logScale != 0) {
        t = log(mu + 1.0f) * c.density;
        dtdmu = c.density / (mu + 1.0f);
    } else {
        // fract(n * density) first, so a large n does not swallow the fraction.
        t = fract(it.x * c.density) + (c.smooth != 0 ? it.y * c.density : 0.0f);
        dtdmu = c.density;
    }
    float d = max(deTexels / pixelsPerTexel, 1e-3f);
    float footprint = dtdmu / (d * 0.6931472f);
    float rawLod = log2(max(footprint * float(c.paletteSize), 1.0f));
    float lod = clamp(rawLod, 0.0f, float(c.paletteLevels - 1));
    if (c.paletteFilter == 0.0f || deTexels >= 999.0f) lod = 0.0f;    // no filter, or no DE
    dense = saturate((lod - 7.0f) / 3.0f);
    // Calm dense regions (kb: techniques/coloring-methods, "Dense regions"): from 1/16 of a
    // palette cycle per pixel (rawLod 6) to half a cycle (9) the colour is per-pixel noise
    // (unfiltered) or the palette's mean grey (filtered); fade to the deep colour instead.
    calm = (c.deep.w > 0.0f && deTexels < 999.0f) ? smoothstep(6.0f, 9.0f, rawLod) : 0.0f;
    return paletteAt(t + c.offset, lod, c, pal);
}

// Interior colourings from the cycle analysis (kb: techniques/coloring-methods, "Interior
// colouring as built"): it.y = period p (0: none found -> flat), de = (|lambda|, arg lambda in
// turns), ax = (interior distance in texels, log2 of the cycle's closest approach to 0).
static inline float3 interiorShade(float2 it, float2 de, float2 ax, constant ColorParams& c,
                                   device const float4* pal, float pixelsPerTexel) {
    if (c.interiorMode == 0 || it.x != ITER_INTERIOR || !(it.y >= 1.0f)) return c.interior.rgb;
    const float r = saturate(de.x);               // |lambda|: 0 at the centre, 1 at the boundary
    const float depth = sqrt(saturate(1.0f - r * r));
    float3 col;
    if (c.interiorMode == 1) {
        // Bubbles: bands of log2 |z*| (the cycle's closest approach to 0), which vanishes at
        // the nucleus: rings around every nucleus, self-similar at any depth.
        float t = -ax.y * c.interiorDensity;
        col = paletteAt(t + c.offset, 0.0f, c, pal);
        float band = fract(t);
        col *= mix(0.55f, 1.0f, smoothstep(0.0f, 0.12f, band) * (1.0f - 0.35f * band));
    } else if (c.interiorMode == 2) {
        // Period: one palette colour per period (golden-ratio steps keep neighbours apart),
        // brighter towards the centre of each component.
        float t = fract(it.y * 0.618034f) + c.offset;
        col = paletteAt(t, 2.0f, c, pal) * mix(0.35f, 1.0f, depth);
    } else if (c.interiorMode == 3) {
        // Internal angle: the palette once around arg lambda, stripes along the internal rays
        // (fading out at the centre, where they converge) and rings of internal radius.
        float a = de.y;
        float stripe = smoothstep(-0.3f, 0.3f, cos(6.2831853f * c.interiorStripes * a));
        stripe = mix(1.0f, stripe, smoothstep(0.08f, 0.35f, r));
        // Rings of equal |lambda|, denser towards the boundary (equal steps of log(1 - r)).
        float ring = smoothstep(-0.6f, 0.6f, cos(6.2831853f * 1.5f * log2(max(1.0f - r, 1e-6f))));
        col = paletteAt(a * max(round(c.interiorDensity * 4.0f), 1.0f) + c.offset, 0.0f, c, pal);
        col *= mix(0.3f, 1.0f, stripe) * mix(0.7f, 1.0f, ring);
    } else {
        // Interior distance: palette by log2 of the distance in pixels, dark at the boundary.
        float d = ax.x / pixelsPerTexel;
        float t = log2(1.0f + d) * c.interiorDensity;
        col = paletteAt(t + c.offset, 0.0f, c, pal) * smoothstep(0.0f, 1.0f, saturate(d / 6.0f));
    }
    return max(col, 0.0f);
}

static inline float3 shade(float2 it, float2 de, float2 ax, constant ColorParams& c, device const float4* pal,
                           float pixelsPerTexel) {
    // Unresolved (past the nominal limit, neither escaped nor proven interior): drawn in the
    // outline colour lifted by 0.5 % towards grey. They hug the boundary (slow exterior near
    // minibrots and parabolic points, which extension passes may still resolve), so they read
    // as boundary; FL: M2's 3 % grey lift showed as flat grey halos around minibrots in
    // gesture frames, which get no extension passes. The same in every view, so stepping the
    // limit does not recolour it.
    if (it.x == ITER_UNRESOLVED) return mix(c.edge.rgb, float3(0.5f), 0.005f);
    if (it.x < 0.0f) return interiorShade(it, de, ax, c, pal, pixelsPerTexel);
    float dense, calm;
    float3 col = paletteColor(it, de.x, c, pal, pixelsPerTexel, dense, calm);
    float dist = de.x / pixelsPerTexel;                            // distance in output pixels
    if (calm > 0.0f) {
        // Dense regions (kb: techniques/coloring-methods, "Dense regions"): a calm deep colour
        // with thin bright filaments where the distance estimate puts the set inside the
        // pixel (the closest fraction of the near-boundary pixels), instead of noise or grey.
        float fil = smoothstep(0.2f, 0.7f, dist) * 0.5f;
        col = mix(col, mix(c.deep.rgb, c.filament.rgb, fil), calm);
    }
    if (c.outline > 0.0f) {
        // Boundary emphasis and filament coverage, before the lighting: within outlineWidth
        // pixels of the set the colour goes towards the edge colour (Chéritat's DE coverage
        // when the width is ~1.4). Where the palette filter averaged many cycles, the colour
        // is a grey of the whole palette: darken that too. Filaments then read as dark lace
        // whose relief the lighting below still shows (highlights on their lit side).
        float f = smoothstep(0.0f, 1.0f, clamp(dist / c.outlineWidth, 0.0f, 1.0f));
        float edge = c.deep.w > 0.0f ? (1.0f - f) * (1.0f - calm) : max(1.0f - f, 0.75f * dense);
        col = mix(col, c.edge.rgb, edge * c.outline);
    }
    if (c.shading > 0.0f) {
        // Normal-map lighting (Chéritat): the surface normal leans along u = z / (dz/dc), the
        // direction away from the set, lifted into 3D (kb: techniques/coloring-methods). Lambert
        // plus a Blinn-Phong highlight towards a viewer straight above.
        float2 u = float2(cos(de.y), sin(de.y));
        float3 n = normalize(float3(u, 1.0f));
        float3 l = normalize(float3(c.light, c.lightHeight));
        float diffuse = max(dot(n, l), 0.0f);
        float3 h = normalize(l + float3(0.0f, 0.0f, 1.0f));
        float spec = pow(max(dot(n, h), 0.0f), c.shininess);
        // Normalised by the mean over normal directions (the normal is tilted 45 degrees, so
        // that mean is l.z / sqrt 2): lighting redistributes brightness, it does not dim.
        float flat = max(l.z * 0.70710678f, 1e-3f);
        float lit = diffuse / flat;
        // (Calm dense regions keep a fifth of the relief: full lighting there is noise.)
        float sh = c.shading * (1.0f - 0.8f * calm);
        col = col * mix(1.0f, lit, sh) + c.specular * sh * spec * float3(1.0f);
    }
    if (c.glow > 0.0f) {
        // A soft halo along the boundary (screen-space width, so it reads at every depth).
        col += c.glowColor.rgb * c.glow * exp(-dist / max(c.glowWidth, 1e-3f));
    }
    return max(col, 0.0f);
}

static inline float3 linearToSRGB(float3 c) {
    c = saturate(c);
    float3 lo = c * 12.92f;
    float3 hi = 1.055f * pow(c, float3(1.0f / 2.4f)) - 0.055f;
    return select(hi, lo, c <= 0.0031308f);
}

// Dither by one 8-bit step (triangular noise from two hashes): the slow gradients of deep-zoom
// exteriors and of the lighting otherwise show contour bands in the bgra8 drawable.
static inline float3 ditheredSRGB(float3 linear, uint2 gid) {
    uint h = gid.x * 1973u + gid.y * 9277u;
    h = (h ^ (h >> 13)) * 0x5bd1e995u;
    h ^= h >> 15;
    float n1 = float(h & 0xffffu) * (1.0f / 65535.0f);
    float n2 = float(h >> 16) * (1.0f / 65535.0f);
    return saturate(linearToSRGB(linear) + (n1 + n2 - 1.0f) / 255.0f);
}

// A texel's value if it is finished in this frame: admitted (admission index < admitted) and
// not pending.
static inline bool finished(texture2d<float, access::read> t, uint2 s, constant ColorParams& c,
                            device const uint* rank, uint admitted, thread float2& it) {
    if (tileIndex(s, c.tilesX, c.tileOrder, rank) >= admitted) return false;
    it = t.read(s).xy;
    return it.x != ITER_PENDING;
}

// The cycle analysis of an interior texel (only read when an interior mode uses it: without
// the analysis a placeholder texture is bound).
static inline float2 auxAt(texture2d<float, access::read> aux, uint2 s, constant ColorParams& c, float2 it) {
    return (c.interiorMode != 0 && it.x == ITER_INTERIOR && it.y >= 1.0f) ? aux.read(s).xy : float2(0.0f);
}

kernel void colorize(texture2d<float, access::read>  iters      [[texture(0)]],
                     texture2d<float, access::read>  des        [[texture(1)]],
                     texture2d<float, access::write> out        [[texture(2)]],
                     texture2d<float, access::sample> under     [[texture(3)]],
                     texture2d<float, access::read>  auxs       [[texture(4)]],
                     constant ColorParams& c                    [[buffer(0)]],
                     device const float4* pal                   [[buffer(1)]],
                     device const LevelCounters& level          [[buffer(2)]],
                     device const uint* rank                    [[buffer(3)]],
                     uint2 gid                                  [[thread_position_in_grid]])
{
    if (gid.x >= c.outSize.x || gid.y >= c.outSize.y) return;
    const uint admitted = c.admittedOnGPU != 0 ? level.admitted : c.admitted;

    uint2 maxSrc = c.srcSize - 1;
    float3 acc = float3(0.0f);
    float count = 0.0f;
    float2 it;

    if (c.ratio >= 1.0f) {
        // Box filter over the ss x ss block of texels covering this pixel, in linear light.
        uint ss = uint(c.ratio);
        uint2 base = gid * ss;
        for (uint j = 0; j < ss; j++) {
            for (uint i = 0; i < ss; i++) {
                uint2 s = min(base + uint2(i, j), maxSrc);
                if (finished(iters, s, c, rank, admitted, it)) {
                    acc += shade(it, des.read(s).xy, auxAt(auxs, s, c, it), c, pal, c.deScale);
                    count += 1.0f;
                }
            }
        }
    } else {
        uint2 s = min(uint2(float2(gid) * c.ratio), maxSrc);
        if (finished(iters, s, c, rank, admitted, it)) {
            acc = shade(it, des.read(s).xy, auxAt(auxs, s, c, it), c, pal, c.deScale);
            count = 1.0f;
        }
    }

    if (count == 0.0f) {
        // Not finished yet: show the coarse preview underneath while the full level refines;
        // without one, still-iterating pixels look like interior until they escape.
        if (c.hasUnder > 0.0f) {
            constexpr sampler bilinear(coord::normalized, filter::linear, address::clamp_to_edge);
            // Texel position in the used region, clamped half a texel inside it (what
            // clamp_to_edge does for a texture of exactly that size), then normalised to the
            // allocated texture.
            float2 p = (float2(gid) + 0.5f) / float2(c.outSize) * c.underSize;
            p = clamp(p, float2(0.5f), c.underSize - 0.5f);
            float2 uv = p / float2(under.get_width(), under.get_height());
            acc = under.sample(bilinear, uv).rgb;   // linear light, so the blend is correct
        } else {
            acc = c.interior.rgb;
        }
        count = 1.0f;
    }

    if (c.linearOut != 0) {
        out.write(float4(acc / count, 1.0f), gid);
        return;
    }
    out.write(float4(ditheredSRGB(acc / count, gid), 1.0f), gid);
}

// ---------------------------------------------------------------------------------------------
// Compose (FL, kb: techniques/frame-reuse-and-progressive, techniques/anti-aliasing): every
// presented interactive frame. The *history* is the last shown image in linear light with a
// per-pixel confidence (alpha) and the viewport it belongs to. Each frame reprojects it into
// the current view (pure scale + translation, so the reprojection is exact; Catmull-Rom keeps
// it sharp) and writes the result as the new history and, sRGB-encoded, to the drawable.
//
// mode 0 (gesture): temporal upsampling. The preview level is jittered (Halton) each frame,
//   so its texels land on different sub-pixel positions; each texel is a point sample that
//   adds weight exp(-d^2 / 2 sigma^2) at distance d (output pixels) to the pixels around it.
//   The reprojected history keeps the detail accumulated so far; its confidence decays with
//   magnification (a zoom-in stretches each history texel over m^2 pixels) and is capped, so
//   new samples keep replacing it. Pixels without history or samples (newly exposed) take the
//   bilinear preview.
// mode 1 (refine): exact full-resolution pixels fade in over the history, per pixel, with
//   factor `blend` = 1 - exp(-dt / tau) per frame: no popping, frame-rate independent.
// mode 2 (anti-aliasing): a finished jittered full-resolution sample joins the running mean
//   (`blend` = 1 / samples).
// mode 3 (exact): exact pixels replace the history (colour changes while idle).
struct ComposeParams {
    float2 histOrigin;    // history texel coordinate (texel centres at i + 0.5) of output pixel 0's centre
    float  histStep;      // history texels per output pixel
    float  histDecay;     // confidence multiplier (magnification penalty); 0: no history
    float2 histSize;      // history texture size
    float2 jitter;        // sample position of the new-sample texels inside their texel, 0..1
    float2 newCentre;     // texel coordinate of the view centre in the new-sample level (its size / 2)
    float  newRatio;      // new-sample texels per output pixel (0: no point samples)
    float  confCap;       // confidence cap of the history
    float  baseWeight;    // weight of the bilinear preview where history and samples exist
    float  invTwoSigma2;  // 1 / (2 sigma^2) of the sample kernel, output pixels
    float  blend;         // mode 1: fade factor; mode 2: weight of the new AA sample
    uint   mode;
    float2 outCentre;     // output size / 2
    float2 histRot;       // (cos, sin): output pixel offsets turned into history axes (view rotation)
};

// Catmull-Rom from five bilinear taps (the four corner taps of the 4x4 kernel dropped, their
// weight is tiny). `p` in texel coordinates (centres at i + 0.5).
static inline float4 catmullRom(texture2d<float, access::sample> t, sampler s, float2 p, float2 size) {
    float2 pos1 = floor(p - 0.5f) + 0.5f;
    float2 f = p - pos1;
    float2 w0 = f * (-0.5f + f * (1.0f - 0.5f * f));
    float2 w1 = 1.0f + f * f * (-2.5f + 1.5f * f);
    float2 w2 = f * (0.5f + f * (2.0f - 1.5f * f));
    float2 w3 = f * f * (-0.5f + 0.5f * f);
    float2 w12 = w1 + w2;
    float2 p12 = (pos1 + w2 / w12) / size;
    float2 p0 = (pos1 - 1.0f) / size;
    float2 p3 = (pos1 + 2.0f) / size;
    float4 r = t.sample(s, float2(p12.x, p0.y)) * (w12.x * w0.y)
             + t.sample(s, float2(p0.x, p12.y)) * (w0.x * w12.y)
             + t.sample(s, float2(p12.x, p12.y)) * (w12.x * w12.y)
             + t.sample(s, float2(p3.x, p12.y)) * (w3.x * w12.y)
             + t.sample(s, float2(p12.x, p3.y)) * (w12.x * w3.y);
    float norm = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    return r / norm;
}

kernel void compose(texture2d<float, access::read>   iters    [[texture(0)]],   // exact / AA level
                    texture2d<float, access::read>   des      [[texture(1)]],
                    texture2d<float, access::sample> preview  [[texture(2)]],   // preview colour (linear)
                    texture2d<float, access::sample> histIn   [[texture(3)]],
                    texture2d<float, access::write>  histOut  [[texture(4)]],
                    texture2d<float, access::write>  out      [[texture(5)]],
                    texture2d<float, access::read>   auxs     [[texture(6)]],
                    constant ColorParams& c                   [[buffer(0)]],
                    device const float4* pal                  [[buffer(1)]],
                    device const LevelCounters& level         [[buffer(2)]],
                    device const uint* rank                   [[buffer(3)]],
                    constant ComposeParams& q                 [[buffer(4)]],
                    uint2 gid                                 [[thread_position_in_grid]])
{
    if (gid.x >= c.outSize.x || gid.y >= c.outSize.y) return;
    constexpr sampler bilinear(coord::normalized, filter::linear, address::clamp_to_edge);
    const float2 pc = float2(gid) + 0.5f;
    // New-sample level: texel coordinate of this pixel's centre (texel t's sample sits at t + jitter).
    const float2 v = (pc - q.outCentre) * q.newRatio + q.newCentre;

    // The bilinear preview: the fallback where nothing better exists.
    float3 base = c.interior.rgb;
    if (c.hasUnder > 0.0f) {
        float2 p = clamp(v - q.jitter + 0.5f, float2(0.5f), c.underSize - 0.5f);
        base = preview.sample(bilinear, p / float2(preview.get_width(), preview.get_height())).rgb;
    }

    // Reprojected history.
    float3 hist = base;
    float wh = 0.0f;
    if (q.histDecay > 0.0f) {
        float2 g = float2(gid);
        float2 hp = q.histOrigin + float2(g.x * q.histRot.x - g.y * q.histRot.y,
                                          g.x * q.histRot.y + g.y * q.histRot.x) * q.histStep;
        if (all(hp >= 0.0f) && all(hp <= q.histSize)) {
            float4 h = q.histStep == 1.0f && all(fract(hp) == 0.5f)
                ? histIn.read(uint2(hp)) : catmullRom(histIn, bilinear, hp, q.histSize);
            hist = max(h.rgb, 0.0f);
            wh = clamp(h.a, 0.0f, q.confCap) * q.histDecay;
        }
    }

    float3 col;
    float conf;
    if (q.mode == 0) {
        // Point samples of the jittered preview within the 2 x 2 texels around this pixel.
        float3 sum = 0.0f;
        float wn = 0.0f;
        if (q.newRatio > 0.0f) {
            int2 t0 = int2(floor(v - q.jitter));
            for (int j = 0; j < 2; j++) {
                for (int i = 0; i < 2; i++) {
                    int2 t = t0 + int2(i, j);
                    if (any(t < 0) || any(float2(t) >= c.underSize)) continue;
                    float2 d = (float2(t) + q.jitter - v) / q.newRatio;
                    float w = exp(-dot(d, d) * q.invTwoSigma2);
                    sum += preview.read(uint2(t)).rgb * w;
                    wn += w;
                }
            }
        }
        float wb = wh + wn < 1e-3f ? 1.0f : q.baseWeight;
        col = (hist * wh + sum + base * wb) / (wh + wn + wb);
        conf = min(q.confCap, wh + wn);
    } else {
        float3 prior = wh > 0.0f ? (hist * wh + base * q.baseWeight) / (wh + q.baseWeight) : base;
        const uint admitted = c.admittedOnGPU != 0 ? level.admitted : c.admitted;
        float2 it;
        bool done = finished(iters, gid, c, rank, admitted, it);
        if (q.mode == 2) {
            // An AA sample: unresolved samples do not count (they would grey the mean), and
            // pixels off the adaptive list (ITER_SKIP) keep their mean.
            bool use = done && it.x != ITER_UNRESOLVED && it.x != ITER_SKIP;
            col = use ? mix(hist, shade(it, des.read(gid).xy, auxAt(auxs, gid, c, it), c, pal, 1.0f), q.blend) : hist;
            conf = q.confCap;
        } else if (done) {
            float3 exact = shade(it, des.read(gid).xy, auxAt(auxs, gid, c, it), c, pal, 1.0f);
            col = q.mode == 1 ? mix(prior, exact, q.blend) : exact;
            conf = q.confCap;
        } else {
            col = prior;
            conf = wh;
        }
    }
    histOut.write(float4(col, conf), gid);
    out.write(float4(ditheredSRGB(col, gid), 1.0f), gid);
}

// ---------------------------------------------------------------------------------------------
// Adaptive anti-aliasing list (FL, kb: techniques/anti-aliasing "adaptive"): which pixels of
// the refined frame gain from more samples. Exterior pixels within `deThreshold` pixels of the
// set (by the distance estimate; all escaped pixels when there is none) and interior pixels
// with an escaped neighbour (the boundary). Far exterior is smooth (and palette-filtered) and
// deep interior is flat and the most expensive to iterate: both are skipped, marked ITER_SKIP
// in the AA sample texture. Listed in admission (tile) order, compacted per SIMD group.
struct AaListParams {
    uint  width;
    uint  height;
    uint  tilesX;
    uint  tileOrder;
    uint  total;         // admission slots (tiles x 256)
    uint  capacity;      // list entries
    float deThreshold;   // pixels
    uint  pad;
};

kernel void aaList(texture2d<float, access::read>  iters  [[texture(0)]],
                   texture2d<float, access::read>  des    [[texture(1)]],
                   texture2d<float, access::write> aaIter [[texture(2)]],
                   constant AaListParams& p               [[buffer(0)]],
                   device const uint* order               [[buffer(1)]],
                   device atomic_uint* count              [[buffer(2)]],
                   device uint* list                      [[buffer(3)]],
                   uint tid                               [[thread_position_in_grid]],
                   uint lane                              [[thread_index_in_simdgroup]])
{
    uint2 xy = uint2(0);
    bool in = tid < p.total && tileTexel(tid, p.tilesX, p.width, p.height, p.tileOrder, order, xy);
    bool want = false;
    if (in) {
        float2 it = iters.read(xy).xy;
        if (it.x >= 0.0f) {
            float d = des.read(xy).x;
            want = d < p.deThreshold || d >= 999.0f;
        } else if (it.x == ITER_INTERIOR) {
            for (int j = -1; j <= 1 && !want; j++) {
                for (int i = -1; i <= 1; i++) {
                    int2 n = int2(xy) + int2(i, j);
                    if (any(n < 0) || n.x >= int(p.width) || n.y >= int(p.height)) continue;
                    if (iters.read(uint2(n)).x >= 0.0f) { want = true; break; }
                }
            }
        }
        if (!want) aaIter.write(float4(ITER_SKIP, 0.0f, 0.0f, 0.0f), xy);
    }
    uint off = simd_prefix_exclusive_sum(want ? 1u : 0u);
    uint total = simd_sum(want ? 1u : 0u);
    uint base = 0;
    if (lane == 0 && total > 0) base = atomic_fetch_add_explicit(count, total, memory_order_relaxed);
    base = simd_broadcast_first(base);
    if (want && base + off < p.capacity) list[base + off] = xy.y * p.width + xy.x;
}

// ---------------------------------------------------------------------------------------------
// Overlay (orbit overlay, mini-map; kb: decisions/controls "Orbit overlay and mini-map"): vector
// primitives in output pixels, composited over the shown image without entering the history.
// The CPU bins primitives into 16x16-pixel tiles; a thread per pixel walks its tile's list in
// draw order and blends each primitive's analytic coverage (1-pixel ramp of the signed
// distance: anti-aliased and crisp at any backing scale). Sparse mode runs only the tiles
// with primitives (after a compose pass wrote the rest); full mode also copies the history to
// the target (a refresh where only the overlay changed).
struct OverlayPrim {
    float4 geom;    // segment: p0, p1 | disc, ring: centre, radius, ring width | rect, image, glyph: x0, y0, x1, y1
    float4 color0;  // premultiplied linear rgba (segment: at p0)
    float4 color1;  // segment: at p1; rect: border colour; glyph: halo colour
    float4 clip;    // x0, y0, x1, y1 (pixel centres outside are not drawn)
    float4 param;   // segment: half width; rect: corner radius, border width; glyph: atlas cell x0, y0, x1, y1; image: opacity, corner radius
    uint4  info;    // kind (0 segment, 1 disc, 2 ring, 3 rect, 4 image, 5 glyph), map index | glyph: atlas width, halo width, spread (float bits)
};

struct OverlayParams {
    uint2  outSize;
    uint   tilesX;
    uint   full;      // 1: every pixel, copying the history; 0: listed tiles only
};

static inline float3 srgbToLinear(float3 c) {
    return select(pow((c + 0.055f) / 1.055f, float3(2.4f)), c / 12.92f, c <= 0.04045f);
}

// Overlay text (kb: platform/overlay-text): a glyph quad of a signed-distance-field atlas
// (R8 bytes, `atlasW` wide; 128 + d / spread * 127, d in texels, inside positive). Returns the
// distance to the outline in output pixels at p, sampled bilinearly inside the glyph's cell.
static inline float glyphDistance(device const uchar* atlas, uint atlasW, float spread, float2 p,
                                  float4 quad, float4 cell) {
    float s = (quad.z - quad.x) / max(cell.z - cell.x, 1e-3f);      // pixels per texel
    float2 t = clamp(cell.xy + (p - quad.xy) / s, cell.xy + 0.5f, cell.zw - 0.5f) - 0.5f;
    float2 f = fract(t);
    uint2 i = uint2(t);
    uint2 j = min(i + 1u, uint2(cell.zw) - 1u);
    float a = float(atlas[i.y * atlasW + i.x]), b = float(atlas[i.y * atlasW + j.x]);
    float c = float(atlas[j.y * atlasW + i.x]), d = float(atlas[j.y * atlasW + j.x]);
    float v = mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
    return (v - 128.0f) / 127.0f * spread * s;
}

static inline float roundedBox(float2 p, float4 r, float radius) {
    float2 c = 0.5f * (r.xy + r.zw), h = 0.5f * (r.zw - r.xy) - radius;
    float2 q = abs(p - c) - h;
    return length(max(q, 0.0f)) + min(max(q.x, q.y), 0.0f) - radius;
}

kernel void overlay(texture2d<float, access::read>  src     [[texture(0)]],   // linear colour (history)
                    texture2d<float, access::write> out     [[texture(1)]],
                    texture2d<float, access::read>  map0    [[texture(2)]],
                    texture2d<float, access::read>  map1    [[texture(3)]],
                    texture2d<float, access::read>  map2    [[texture(4)]],
                    texture2d<float, access::read>  map3    [[texture(5)]],
                    constant OverlayParams& q               [[buffer(0)]],
                    device const OverlayPrim* prims         [[buffer(1)]],
                    device const uint* tileStart            [[buffer(2)]],
                    device const uint* tileList             [[buffer(3)]],
                    device const uint* active               [[buffer(4)]],
                    device const uchar* font                [[buffer(5)]],   // overlay font SDF atlas
                    uint3 tid                               [[thread_position_in_grid]])
{
    uint2 gid;
    uint tile;
    if (q.full != 0) {
        gid = tid.xy;
        if (gid.x >= q.outSize.x || gid.y >= q.outSize.y) return;
        tile = (gid.y >> 4) * q.tilesX + (gid.x >> 4);
    } else {
        tile = active[tid.z];
        gid = uint2((tile % q.tilesX) * 16u + tid.x, (tile / q.tilesX) * 16u + tid.y);
        if (gid.x >= q.outSize.x || gid.y >= q.outSize.y) return;
    }
    float3 col = max(src.read(gid).rgb, 0.0f);
    const float2 p = float2(gid) + 0.5f;
    for (uint i = tileStart[tile]; i < tileStart[tile + 1]; i++) {
        const OverlayPrim pr = prims[tileList[i]];
        if (any(p < pr.clip.xy) || any(p > pr.clip.zw)) continue;
        float4 c = pr.color0;
        float cov = 0.0f;
        switch (pr.info.x) {
            case 0: {   // segment, colour along it
                float2 a = pr.geom.xy, b = pr.geom.zw, ab = b - a;
                float t = saturate(dot(p - a, ab) / max(dot(ab, ab), 1e-6f));
                cov = saturate(pr.param.x + 0.5f - length(p - a - ab * t));
                c = mix(pr.color0, pr.color1, t);
                break;
            }
            case 1:     // disc
                cov = saturate(pr.geom.z + 0.5f - length(p - pr.geom.xy));
                break;
            case 2:     // ring
                cov = saturate(0.5f * pr.geom.w + 0.5f - abs(length(p - pr.geom.xy) - pr.geom.z));
                break;
            case 3: {   // rounded rect: fill, then border
                float d = roundedBox(p, pr.geom, pr.param.x);
                float fill = saturate(0.5f - d);
                float bw = pr.param.y;
                float border = bw > 0.0f ? saturate(0.5f * bw + 0.5f - abs(d + 0.5f * bw)) : 0.0f;
                float4 f = pr.color0 * fill;
                f = pr.color1 * border + f * (1.0f - pr.color1.a * border);
                col = f.rgb + col * (1.0f - f.a);
                continue;
            }
            case 4: {   // map image (sRGB-encoded texture), rounded corners
                float d = roundedBox(p, pr.geom, pr.param.y);
                cov = saturate(0.5f - d);
                if (cov <= 0.0f) continue;
                uint2 t = uint2(clamp(p - pr.geom.xy, float2(0.0f), pr.geom.zw - pr.geom.xy - 1.0f));
                float3 m = pr.info.y == 0 ? map0.read(t).rgb : pr.info.y == 1 ? map1.read(t).rgb
                         : pr.info.y == 2 ? map2.read(t).rgb : map3.read(t).rgb;
                c = float4(srgbToLinear(m) * pr.param.x, pr.param.x);
                break;
            }
            default: {  // glyph: fill, then the halo under it
                if (any(p < pr.geom.xy - 1.0f) || any(p > pr.geom.zw + 1.0f)) continue;
                float d = glyphDistance(font, pr.info.y, as_type<float>(pr.info.w), p, pr.geom, pr.param);
                float fill = saturate(d + 0.5f);
                float hw = as_type<float>(pr.info.z);
                // Opaque next to the glyph, fading out over its outer 40 % (a soft outline).
                float halo = hw > 0.0f ? saturate((d + hw) / max(0.4f * hw, 1.0f)) : 0.0f;
                float4 g = pr.color0 * fill;
                g += pr.color1 * halo * (1.0f - g.a);
                col = g.rgb + col * (1.0f - g.a);
                continue;
            }
        }
        c *= cov;
        col = c.rgb + col * (1.0f - c.a);
    }
    out.write(float4(ditheredSRGB(col, gid), 1.0f), gid);
}

// MARK: Autopilot survey (kb: decisions/controls, autopilot)

struct SurveyParams {
    uint width;          // level texels
    uint height;
    uint cols;           // survey cells
    uint rows;
};

/// A coarse picture of a finished level for the autopilot: per cell, sampled texels, interior
/// (or unresolved) ones, and the share of neighbour pairs whose class or integer escape count
/// differ (band edges: their rate times the level width is the number of colour bands
/// crossed per view width). One thread per cell, at most 16 x 16 samples each.
kernel void survey(texture2d<float, access::read> iters [[texture(0)]],
                   constant SurveyParams& p             [[buffer(0)]],
                   device uint4* cells                  [[buffer(1)]],
                   uint2 gid                            [[thread_position_in_grid]])
{
    if (gid.x >= p.cols || gid.y >= p.rows) return;
    uint x0 = gid.x * p.width / p.cols, x1 = (gid.x + 1) * p.width / p.cols;
    uint y0 = gid.y * p.height / p.rows, y1 = (gid.y + 1) * p.height / p.rows;
    uint step = max(1u, max((x1 - x0 + 15) / 16, (y1 - y0 + 15) / 16));
    uint samples = 0, interior = 0, pairs = 0, edges = 0;
    for (uint y = y0; y < y1; y += step) {
        for (uint x = x0; x < x1; x += step) {
            float n = iters.read(uint2(x, y)).x;
            if (n == ITER_PENDING || n == ITER_SKIP) continue;
            samples++;
            bool in = n < 0.0f;
            if (in) interior++;
            for (uint k = 0; k < 2; k++) {
                uint2 q = k == 0 ? uint2(x + 1, y) : uint2(x, y + 1);
                if (q.x >= p.width || q.y >= p.height) continue;
                float m = iters.read(q).x;
                if (m == ITER_PENDING || m == ITER_SKIP) continue;
                pairs++;
                bool min = m < 0.0f;
                if (min != in || (!in && m != n)) edges++;
            }
        }
    }
    cells[gid.y * p.cols + gid.x] = uint4(samples, interior, pairs, edges);
}
