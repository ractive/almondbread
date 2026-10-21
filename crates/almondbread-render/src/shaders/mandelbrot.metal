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
constant bool kSeeded   [[function_constant(5)]];   // M5: fresh pixels of seeded tiles start from their
                                                    // quadtree leaf's local series expansion
                                                    // (kb: techniques/local-series-approximation)
constant bool kProbe    [[function_constant(6)]];   // interior probe of pixels unresolved at the
                                                    // limit (needs kInterior; see "Attracting-cycle probe")
constant bool kLA       [[function_constant(8)]];   // M6: period-aligned linear approximation
                                                    // (kb: techniques/linear-approximation-imagina)
// F2: the formula (kb: plans/f2-pluggable-formulas; see "Formula hooks"). Unset (every
// Mandelbrot pipeline): formula 0, power 2, conformal; so formula 0 compiles to the pre-F2
// kernels. 16.. leaves room for more feature flags below.
constant uint kFormulaFC   [[function_constant(16)]];   // 0 Mandelbrot, 1 z^p + c (generic), 2 Julia
constant uint kPowerFC     [[function_constant(17)]];   // p of formula 1
constant bool kConformalFC [[function_constant(18)]];   // false: 2x2 derivatives (F5, unused yet)
constant uint kFormula = is_function_constant_defined(kFormulaFC) ? kFormulaFC : 0u;
constant uint kPower = is_function_constant_defined(kPowerFC) ? kPowerFC : 2u;
constant bool kConformal = is_function_constant_defined(kConformalFC) ? kConformalFC : true;

// Iteration-texture codes in .x besides n >= 0.
constant float ITER_INTERIOR   = -1.0f;   // .y = 1: derivative test, 0: reached maxIter without the test
constant float ITER_PENDING    = -2.0f;   // admitted but still iterating
constant float ITER_UNRESOLVED = -3.0f;   // past the nominal limit, neither escaped nor proven
                                          // interior (.y = the nominal limit); may still resolve
constant float ITER_SKIP       = -4.0f;   // AA sample textures: this pixel takes no AA samples

// M5: a real reference's seeds are not used within this many texels of the real axis.
constant float AXIS_BAND = 16.0f;

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
    uint   seedGate;      // M5: admit fresh pixels only once the level's seeds are final
                          // (SeedStatus at buffer 20 of prepareChunk)
    // M3b (kDeep): texelScale and refOffset are mantissas in units of 2^scaleExp, so a texel
    // of 1e-1000 is texelScale * 2^scaleExp with texelScale in [1, 2). Plain kernels: 0.
    int    scaleExp;
    uint   critLast;      // F4 Julia: the critical orbit's last index (see kJulia)
    // View rotation (cos, sin) of the texel offset (kb: decisions/controls "View rotation");
    // (1, 0) unrotated, which keeps the plain products bit for bit.
    float2 rot;
    uint   fillBorder;    // M5 interior fill: admission indices of the tile borders (0: off)
    uint   fillEpoch;     // the level's stamp in tileBad[]
    uint   fillProof;     // a border pixel counts as interior if proven within this many
                          // iterations
    uint   realAxis;      // 1: the reference point is on the real axis (a real orbit): M5 seeds
                          // are not used within AXIS_BAND texels of the axis (the needle is a
                          // knife edge in Im c)
    uint   probe;         // interior probe of pixels unresolved at maxIter: at most this many
                          // probe steps per pixel (0: off)
    uint   probeHalf;     // probe state buffer: this chunk reads half probeHalf (capacity
                          // entries each) and writes the other
    uint   centerStart;   // F4 Julia: the centre orbit's first index (fresh pixels start there)
    uint   pad1;
};

// Morton (Z) order inside a 16 x 16 tile: index bits x0 y0 x1 y1 x2 y2 x3 y3 (low to high), so
// 4 consecutive texels are a 2 x 2 block, 16 a 4 x 4 block and a SIMD group of 32 an 8 x 4
// block (M5: a SIMD group starts from neighbouring local-SA leaves, kb:
// techniques/local-series-approximation; also more compact than two 16-texel rows).
static inline uint2 mortonDecode8(uint i) {
    uint x = (i & 1u) | ((i >> 1) & 2u) | ((i >> 2) & 4u) | ((i >> 3) & 8u);
    uint y = ((i >> 1) & 1u) | ((i >> 2) & 2u) | ((i >> 3) & 4u) | ((i >> 4) & 8u);
    return uint2(x, y);
}
static inline uint mortonEncode8(uint2 v) {
    uint x = v.x & 15u, y = v.y & 15u;
    return (x & 1u) | ((x & 2u) << 1) | ((x & 4u) << 2) | ((x & 8u) << 3)
         | ((y & 1u) << 1) | ((y & 2u) << 2) | ((y & 4u) << 3) | ((y & 8u) << 4);
}

// Admission index -> texel of a tiled level (kb: techniques/frame-reuse-and-progressive, FL):
// texels are admitted tile by tile (16 x 16, Morton order inside a tile); the tile order is
// raster order or a priority list (newly exposed tiles first, then by distance from the zoom
// target). Returns false for padding texels of edge tiles.
// M5 interior fill (Mariani-Silver on the tile grid, kb: techniques/guessing-and-subdivision):
// the first `fillBorder` = tiles x 60 admission indices are the tiles' borders (the ring of
// 60 texels, in tile order), the rest their 14 x 14 insides; a tile whose whole border was
// proven interior is filled interior without iterating its inside.
static inline uint2 ringTexel(uint k) {
    return k < 16u ? uint2(k, 0u) : k < 32u ? uint2(k - 16u, 15u) : k < 46u ? uint2(0u, k - 31u) : uint2(15u, k - 45u);
}
static inline uint ringIndex(uint2 l) {
    return l.y == 0u ? l.x : l.y == 15u ? 16u + l.x : l.x == 0u ? 31u + l.y : 45u + l.y;
}
static inline bool onRing(uint2 s) {
    const uint2 l = s & 15u;
    return l.x == 0u || l.y == 0u || l.x == 15u || l.y == 15u;
}

static inline bool tileTexel(uint a, uint tilesX, uint width, uint height, uint tileOrder,
                             device const uint* order, thread uint2& xy, uint fillBorder = 0u) {
    uint rank, tile;
    uint2 l;
    if (fillBorder == 0u) {
        rank = a >> 8;
        l = mortonDecode8(a & 255u);
    } else if (a < fillBorder) {
        rank = a / 60u;
        l = ringTexel(a - rank * 60u);
    } else {
        const uint b = a - fillBorder;
        rank = b / 196u;
        const uint k = b - rank * 196u;
        l = uint2(1u + k % 14u, 1u + k / 14u);
    }
    tile = tileOrder != 0 ? order[rank] : rank;
    xy = uint2((tile % tilesX) * 16u, (tile / tilesX) * 16u) + l;
    return xy.x < width && xy.y < height;
}

// Texel -> admission index (the inverse; rank[] is the inverse of order[]).
static inline uint tileIndex(uint2 s, uint tilesX, uint tileOrder, device const uint* rank, uint fillBorder = 0u) {
    uint tile = (s.y >> 4) * tilesX + (s.x >> 4);
    uint r = tileOrder != 0 ? rank[tile] : tile;
    if (fillBorder == 0u) return (r << 8) | mortonEncode8(s);
    const uint2 l = s & 15u;
    return onRing(s) ? r * 60u + ringIndex(l) : fillBorder + r * 196u + (l.y - 1u) * 14u + (l.x - 1u);
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

// Interior probe state of one live pixel past maxIter (kInterior; see "Attracting-cycle probe"
// below). A separate buffer (binding 30, two halves: in = probeHalf, out = the other) so the
// main loop carries none of it. While probing, the PixelState holds dz * 2^e = the delta as
// floatexp (also in the plain kernel), w * 2^wExp = the multiplier lambda = (f^k)'(z_B),
// d * 2^dExp = the base point's delta and n = maxIter + probe steps.
struct PixelStateProbe {
    float2 zz;            // (f^k)''(z_B) mantissa
    int    zzExp;
    uint   mB;            // the base point's reference index
    uint   k;             // steps since the base point
    uint   info;          // PROBE_NEWTON | passes << 26 | lead (scan) or period (Newton)
    float2 oB;            // Newton passes: the scan's orbit point to resume from if they fail
    int    oBExp;         // (its delta, floatexp in the deep kernel, and reference index)
    uint   oM;
    uint   pRej;          // candidates must be longer than this rejected period (Component
                          // Newton: the bits of log2 |Delta| of its last pass)
    uint   seed;          // Component Newton: candidate index (bits 0..2), probe step at which
                          // the scan started (bits 3..31; its leads count from there)
};
constant uint PROBE_SEED   = 0x80000000u;   // Component Newton from z = 0 (period in the low bits)
constant uint PROBE_NEWTON = 0x40000000u;
constant uint PROBE_LOW    = 0x03ffffffu;   // lead / period (26 bits)
constant uint PROBE_PASS   = 0x04000000u;   // Newton passes (bits 26..29)
constant uint PROBE_COST_PLAIN = 4u;        // main-loop steps per probe step (chunk budget):
constant uint PROBE_COST_DEEP  = 16u;       // float32 in the plain kernel, floatexp in the deep one
// Probe step actions.
constant uint PA_SCAN = 0u, PA_EVAL = 1u, PA_RESET = 2u, PA_PROVEN = 3u, PA_NEWTON = 4u, PA_REJECT = 5u,
              PA_STUCK = 6u, PA_REPEL = 7u;
constant uint PROBE_PASSES = 8u;            // Newton passes per probe at most
constant uint SEED_PASSES = 8u;             // Component Newton passes per candidate period at most

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
    atomic_uint liveMinN;       // smallest n of the pixels still live after it (render progress)
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

// M6 LA tables (kb: techniques/linear-approximation-imagina "As built"; almondbread_core::la).
// Lookup boundaries j = 0 .. count-1: laBounds[j] = (L_j, first entry of j's chain); laBounds[count]
// is a sentinel (last, entry count). A pixel looks up at m == L_j (right after the exact step from
// a near-return, or at a synthetic boundary itself) and takes the longest valid block of the
// chain (stage 0 upwards; validity is monotone along it). laNext[m] = the next j with L_j >= m.
struct LaEntryX {         // the deep form: a BLA block (A, B, R, K with exponents)
    BlaEntryX blk;
    uint      len;        // iterations of the block
    uint      endJ;       // the next lookup boundary after it
};
struct LaEntry {          // the plain form (float32, r = 0: never valid)
    float2 a;
    float2 b;
    float  r;
    float  k;
    uint   len;
    uint   endJ;
};
struct LaParams {
    uint   count;
    uint   pad0;
    uint   pad1;
    uint   pad2;
};

// Admission state of the running level, on the GPU so admission needs no CPU round trip:
// each chunk fills whatever live capacity the previous one freed.
struct LevelCounters {
    uint admitted;        // fresh pixels admitted so far (scanline order)
    uint freshStart;      // this chunk's first fresh pixel
    uint freshCount;      // this chunk's fresh pixels
    uint undecided;       // M5 interior fill: border pixels alive below fillProof iterations
                          // after the last chunk (iterate adds them through buffer 24)
};

static inline float2 cmul(float2 a, float2 b) {
    return float2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// ---- Closed-form component test (kb: techniques/interior-detection "Closed-form component
// test"; built by almondbread_core::components, which documents the proof). Main cardioid:
// q (q + X) - y^2/4 < 0 with X = x - 1/4, q = X^2 + y^2; period-2 bulb: (x + 1)^2 + y^2 < 1/16.
// The CPU expands both, exactly (BigFloat, any depth), as polynomials of degree <= 4 in this
// level's texel offset (u, v) = (pix.x, -pix.y) scaled by 1/L into [-1, 1], normalised to a
// largest coefficient ~1. A texel is inside iff S < -(GAMMA |S|_1 + slack): the float32 sum S
// of the 15 terms is within ~25 ulps of |S|_1 (the sum of their magnitudes) of the exact value,
// GAMMA = 2^-16 leaves a factor 10, slack covers the CPU's rounding and dropped terms. So a
// texel is never called inside unless it is; texels within ~2^-16 L texels of the boundary
// keep iterating as before. Exterior texels are untouched.
struct ComponentParams {
    float2 anchor;        // c at texel offset (0, 0), float32 (colouring only)
    float2 ex, ey;        // c per texel along u and v, float32 (colouring only)
    float  cardioid[15];  // monomials 1, u, v, u2, uv, v2, u3, u2v, uv2, v3, u4, u3v, u2v2, uv3, v4
    float  bulb[15];
    float  slack[2];
    float  invL, L;       // u' = u / L (powers of two)
    float  log2Texel;
    uint   flags;         // bit 0: cardioid, bit 1: period-2 bulb
    // Component Newton (kb: techniques/interior-detection "Component Newton"): periods of
    // attracting cycles found at sample points of the view (most samples first), tried by the
    // interior probe as Newton from z = 0 before its scan; seedSteps: extra probe steps for them.
    uint   periods[4];
    uint   periodCount;
    uint   seedSteps;
};
constant float COMP_GAMMA = 0x1p-16f;

static inline void compMonomials(float u, float v, thread float* m) {
    const float u2 = u * u, uv = u * v, v2 = v * v, u3 = u2 * u, v3 = v2 * v;
    m[0] = 1.0f; m[1] = u; m[2] = v; m[3] = u2; m[4] = uv; m[5] = v2;
    m[6] = u3; m[7] = u2 * v; m[8] = u * v2; m[9] = v3;
    m[10] = u2 * u2; m[11] = u3 * v; m[12] = u2 * v2; m[13] = u * v3; m[14] = v2 * v2;
}

static inline float compSum(constant float* b, thread const float* m, thread float& mag) {
    float s = 0.0f, a = 0.0f;
    for (uint k = 0; k < 15u; k++) {
        const float t = b[k] * m[k];
        s += t;
        a += abs(t);
    }
    mag = a;
    return s;
}

static inline float2 csqrtf(float2 z) {
    const float r = length(z);
    const float re = sqrt(max(0.5f * (r + z.x), 0.0f));
    const float im = sqrt(max(0.5f * (r - z.x), 0.0f));
    return float2(re, z.y < 0.0f ? -im : im);
}

static inline float2 cdivf(float2 a, float2 b) {
    return float2(a.x * b.x + a.y * b.y, a.y * b.x - a.x * b.y) / max(dot(b, b), 1e-30f);
}

// Interior colouring of a texel the test proved (period 1 or 2, known exactly): multiplier
// lambda, the cycle's closest approach to 0 and the interior distance estimate in texels, from
// the closed forms at c in float32 (as precise as the cycle analysis there). Near the boundary
// (1 - |lambda|^2 < 2^-10) and wherever float32 c does not resolve the texel (texel below
// 2^-16 |c|, about zoom 1e3), the distance is twice
// the first-order distance to the polynomial's zero set, L |S| / |grad S| (the closed-form
// estimate tends to twice the true distance at the boundary, for both components), which is
// exact at any depth.
static inline void compColor(constant ComponentParams& cp, uint period, float2 uv, thread float2& lam,
                             thread float& minLog2, thread float& dist) {
    const float2 c = cp.anchor + uv.x * cp.ex + uv.y * cp.ey;
    float b;
    if (period == 1u) {
        const float2 r = csqrtf(float2(1.0f - 4.0f * c.x, -4.0f * c.y));
        lam = float2(1.0f, 0.0f) - r;
        minLog2 = log2(max(0.5f * length(lam), 1e-38f));
        b = (1.0f - dot(lam, lam)) * length(r) * 0.5f;
    } else {
        const float2 r = csqrtf(float2(-3.0f - 4.0f * c.x, -4.0f * c.y));
        const float2 z0 = 0.5f * (r - float2(1.0f, 0.0f)), z1 = -0.5f * (r + float2(1.0f, 0.0f));
        lam = 4.0f * float2(c.x + 1.0f, c.y);
        minLog2 = log2(max(min(length(z0), length(z1)), 1e-38f));
        const float2 fzz = 12.0f * cmul(z0, z0) + 4.0f * c;
        const float2 den = 4.0f * z0 + cdivf(cmul(fzz, -r), float2(1.0f, 0.0f) - lam);
        b = (1.0f - dot(lam, lam)) / max(length(den), 1e-30f);
    }
    // (Closed form only where float32 c resolves the texel well: near the cusp |dlambda/dc| is
    // large and a coarser gate drew the rounding of c as staircases.)
    if (cp.log2Texel >= log2(max(length(cp.anchor), 1e-30f)) - 16.0f && 1.0f - dot(lam, lam) >= 0x1p-10f) {
        dist = exp2(min(log2(max(b, 1e-38f)) - cp.log2Texel, 99.0f));
    } else {
        // (Monomials and S again: keeping them from the test would hold 16 registers across
        // the main loop.)
        constant float* q = period == 1u ? cp.cardioid : cp.bulb;
        float m[15], mag;
        compMonomials(uv.x * cp.invL, uv.y * cp.invL, m);
        const float s = compSum(q, m, mag);
        const float gu = q[1] + 2.0f * q[3] * m[1] + q[4] * m[2] + 3.0f * q[6] * m[3] + 2.0f * q[7] * m[4] + q[8] * m[5]
            + 4.0f * q[10] * m[6] + 3.0f * q[11] * m[7] + 2.0f * q[12] * m[8] + q[13] * m[9];
        const float gv = q[2] + q[4] * m[1] + 2.0f * q[5] * m[2] + q[7] * m[3] + 2.0f * q[8] * m[4] + 3.0f * q[9] * m[5]
            + q[11] * m[6] + 2.0f * q[12] * m[7] + 3.0f * q[13] * m[8] + 4.0f * q[14] * m[9];
        const float g = length(float2(gu, gv));
        dist = g > 0.0f ? min(2.0f * cp.L * (-s) / g, 1e30f) : 0.0f;
    }
}

// ---------------------------------------------------------------------------------------------
// M5: local series approximation (kb: techniques/local-series-approximation,
// questions/local-sa-float32). A quadtree node is a seed at its centre c0: it iterates exactly
// (perturbation against the reference, rebasing as the pixel loop) and carries
//   a = dz/dc * texel           (the DE derivative d of the pixel loop)
//   b = d2z/dc2 * texel^2       (deep: floatexp; plain: divided by the texel, b * texel)
// while |b| r <= 2 eps |a| over its radius r (texels). A child starts from its parent's state
// shifted by delta (texels): dz + a delta + b delta^2 / 2, a + b delta, b. Pixels start from
// their leaf node the same way and run the unchanged loop.
// The interior test's w = dz_n/dz_1 is a function of c too: seeds carry it and its derivative
//   wd = dw/dc * texel,  wd_{n+1} = 2 (a_n w_n + z_n wd_n)
// and samples start from w + wd delta. (Restarting w at the branch is wrong: a later passage
// of the orbit near 0 multiplies a restarted w by |2z| ~ 1e-300 at the deep needle, with no
// growth from the earlier part of the orbit to balance it: 96 % false interior, measured R5.)
// Seed passes are resumable: a node keeps its whole loop state, so a pass with a step budget
// stops anywhere and the next round continues it with the same result (watchdog, M5).
struct SeedNode {
    float2 dz;       // delta to the reference (deep: mantissa)
    float2 a;        // dz/dc * texel (deep: mantissa)
    float2 b;        // plain: d2z/dc2 * texel; deep: d2z/dc2 * texel^2 mantissa
    int    dzExp;    // deep exponents (plain: 0)
    int    aExp;
    int    bExp;
    uint   m;        // reference index
    uint   n;        // iteration of the branch state
    uint   steps;    // seed steps executed by this node (incl. the rejected one)
    float2 w;        // dz_n/dz_1 (interior test), floatexp mantissa
    float2 wd;       // dw/dc * texel, floatexp mantissa
    int    wExp;
    int    wdExp;
    uint   stamp;    // epoch << 2 | state (0 new, 1 running, 2 done): nodes of older seedings
                     // in the buffer are "new" for this one
    uint   path;     // accepted seed steps from the root to here: what a pixel saves
    float  icMin;    // kInteriorColor: log2 of the smallest |z| so far, and its iteration
    uint   icQ;
};

// One quadtree level of seeds; the leaf level's copy tells the iterate kernel where a pixel's
// node is and how it was seeded.
struct SeedParams {
    uint  gridW;         // nodes per row of this quadtree level
    uint  gridH;
    uint  size;          // node size in texels
    uint  offset;        // first node of this level in the node buffer
    uint  parentGridW;
    uint  parentOffset;
    uint  root;          // 1: no parent, start at z = 0
    uint  softIter;      // seeds stop here
    float radius;        // texels
    float eps;
    uint  epoch;         // this seeding (node stamps)
    uint  budget;        // seed steps per node in this pass
    float2 jitter;       // the seeded level's sample position inside a texel
    uint  tilesX;        // 16 x 16 tiles per row of the seeded level
    uint  pad0;
    float gateRel;       // a tile is seeded when its leaves' mean path (seed steps) is at
    float gateAbs;       // least gateAbs, or without a cost estimate (gateAbs 0) when their
                         // mean branch iteration is at least gateRel * softIter
    uint  tilesY;
    uint  truncate;      // 1 (gesture frames): a node out of budget branches where it is (an
                         // earlier branch is always valid, only less useful), so the seeds take
                         // one bounded round; not deterministic in the budget, which gesture
                         // frames need not be
};

// The seed round's status (buffer 20 of the seed kernels and of prepareChunk).
struct SeedStatus {
    atomic_uint unfinished;   // nodes of this round not done (incl. waiting for their parent)
    atomic_uint seededTiles;  // tiles seedGate let start from their seeds
    atomic_uint pathLo;       // sum over those tiles of their leaves' mean path (64 bit)
    atomic_uint pathHi;
};

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

static inline FE feSub(FE a, FE b) {
    return feAdd(a, FE{-b.m, b.e});
}

// a / b (b != 0; zero otherwise).
static inline FE feDiv(FE a, FE b) {
    const float n = dot(b.m, b.m);
    if (!(n > 0.0f)) return FE{float2(0.0f), FE_ZERO};
    return feNorm(cmul(a.m, float2(b.m.x, -b.m.y)) / n, a.e - b.e);
}

// log2 |a| (-1e30 for zero).
static inline float feLog2(FE a) {
    const float l = length(a.m);
    return l > 0.0f ? log2(l) + float(a.e) : -1e30f;
}

// log2(2^a + 2^b).
static inline float lgSum(float a, float b) {
    const float hi = max(a, b);
    return hi + log2(1.0f + exp2(max(min(a, b) - hi, -60.0f)));
}

// ---- Formula hooks (F2, kb: plans/f2-pluggable-formulas, synthesis/other-fractal-types §6).
// Every step site of the iterate and seed kernels calls these instead of writing z^2 + c
// inline. Each branches on kFormula, a function constant folded when the pipeline is compiled:
// formula 0 (Mandelbrot, also when the constant is not set at all, as for every pre-F2
// pipeline) is exactly the expression the site had before, so its pipelines are the pre-F2
// code (bit-identical fields, same timings). Formula 1 is the generic polynomial z^p + c with
// p = kPower (kb: formulas/multibrot): the perturbation step z' = z S + c with
// S = sum_{i<p} W^i Z^(p-1-i), W = Z + z (Horner), f'(z) = p z^(p-1), f''(z) = p (p-1) z^(p-2).
// kPower is a function constant too, so the small loops unroll.
// Naming: f* float2, f*X floatexp; *XT the variants that keep FE_ZERO (feTwice) where the site
// used it. Formula 2 (F4 Julia, z^2 + k) shares Mandelbrot's derivative hooks (`kFormula != 1u`)
// and only drops the +c of the steps.

// z^k, k >= 1.
static inline float2 fPow(float2 z, uint k) {
    float2 w = z;
    for (uint i = 1; i < k; i++) w = cmul(w, z);
    return w;
}

static inline FE fPowX(FE z, uint k) {
    FE w = z;
    for (uint i = 1; i < k; i++) w = feMul(w, z);
    return w;
}

// f'(z).
static inline float2 fPrime(float2 z) {
    if (kFormula != 1u) return 2.0f * z;
    return float(kPower) * fPow(z, kPower - 1u);
}

// f''(z).
static inline float2 fPrime2(float2 z) {
    if (kFormula != 1u) return float2(2.0f, 0.0f);
    const float k = float(kPower * (kPower - 1u));
    return kPower == 2u ? float2(k, 0.0f) : k * fPow(z, kPower - 2u);
}

// f'(z) d: the derivative updates (dz/dc, dz/dz1).
static inline float2 fMulPrime(float2 z, float2 d) {
    if (kFormula != 1u) return 2.0f * cmul(z, d);
    return cmul(fPrime(z), d);
}

// The perturbation step: z' = S w + dd for the delta z = s w (s = 1: w = sw = z), W = Z + sw.
static inline float2 fStep(float2 Z, float2 sw, float2 w, float2 dd) {
    if (kFormula == 0u) return cmul(2.0f * Z + sw, w) + dd;
    if (kFormula == 2u) return cmul(2.0f * Z + sw, w);   // F4 Julia: no +c
    const float2 W = Z + sw;
    float2 S = float2(1.0f, 0.0f), Zk = float2(1.0f, 0.0f);
    for (uint i = 1; i < kPower; i++) {
        Zk = cmul(Zk, Z);
        S = cmul(S, W) + Zk;
    }
    return cmul(S, w) + dd;
}

// f'(z) as floatexp (FE{z.m, z.e + 1} for Mandelbrot).
static inline FE fPrimeX(FE z) {
    if (kFormula != 1u) return FE{z.m, z.e + 1};
    return feMul(feNorm(float2(float(kPower), 0.0f), 0), fPowX(z, kPower - 1u));
}

// f'(z) keeping zero (feTwice for Mandelbrot).
static inline FE fPrimeXT(FE z) {
    if (kFormula != 1u) return feTwice(z);
    return fPrimeX(z);
}

static inline FE fPrime2X(FE z) {
    const float k = float(kPower * (kPower - 1u));
    return kPower == 2u ? feNorm(float2(k, 0.0f), 0) : feMul(feNorm(float2(k, 0.0f), 0), fPowX(z, kPower - 2u));
}

// The floatexp perturbation step z' = S z + C, S from W = Z + z ((2Z + z) for Mandelbrot,
// with 2Z as FE{Z.m, Z.e + 1}).
static inline FE fStepX(FE Zx, FE zx, FE C) {
    if (kFormula == 0u) return feAdd(feMul(feAdd(FE{Zx.m, Zx.e + 1}, zx), zx), C);
    if (kFormula == 2u) return feMul(feAdd(FE{Zx.m, Zx.e + 1}, zx), zx);   // F4 Julia: no +c
    const FE W = feAdd(Zx, zx);
    FE S = FE{float2(0.5f, 0.0f), 1}, Zk = S;
    for (uint i = 1; i < kPower; i++) {
        Zk = feMul(Zk, Zx);
        S = feAdd(feMul(S, W), Zk);
    }
    return feAdd(feMul(S, zx), C);
}

// As fStepX, 2Z by feTwice (the probe and the cycle analysis).
static inline FE fStepXT(FE Zx, FE zx, FE C) {
    if (kFormula == 0u) return feAdd(feMul(feAdd(feTwice(Zx), zx), zx), C);
    if (kFormula == 2u) return feMul(feAdd(feTwice(Zx), zx), zx);
    return fStepX(Zx, zx, C);
}

// Seeds (local SA, kb: techniques/local-series-approximation): b' = f''(z) a^2 / ts + f'(z) b
// (float32 units: a = dz/dc ts, b = d2z/dc2 ts).
static inline float2 fSeedB(float2 z, float2 A, float2 B, float invTs) {
    if (kFormula != 1u) return 2.0f * (cmul(A, A * invTs) + cmul(z, B));
    return cmul(fPrime2(z), cmul(A, A * invTs)) + cmul(fPrime(z), B);
}

// Floatexp seeds: b' = f''(z) a^2 + f'(z) b.
static inline FE fSeedBX(FE z, FE A, FE B) {
    if (kFormula != 1u) {
        const FE t = feAdd(feMul(A, A), feMul(z, B));
        return FE{t.m, t.e + 1};
    }
    return feAdd(feMul(fPrime2X(z), feMul(A, A)), feMul(fPrimeX(z), B));
}

// The seeds' interior derivative's c-derivative: wd' = f''(z) a w + f'(z) wd.
static inline FE fSeedWDX(FE z, FE A, FE W, FE WD) {
    if (kFormula != 1u) {
        const FE t2 = feAdd(feMul(A, W), feMul(z, WD));
        return FE{t2.m, t2.e + 1};
    }
    return feAdd(feMul(fPrime2X(z), feMul(A, W)), feMul(fPrimeX(z), WD));
}

// Probe: F'' of f^P, F'' <- f'(z) F'' + f''(z) lambda^2 (float32: f'' times the alignment sc).
static inline float2 fProbeF2(float2 z, float2 z2, float2 f, float2 l, float sc) {
    if (kFormula != 1u) return cmul(z2, f) + (2.0f * sc) * cmul(l, l);
    return cmul(z2, f) + sc * cmul(fPrime2(z), cmul(l, l));
}

static inline FE fProbeF2X(FE zf, FE z2, FE fzz, FE lam) {
    if (kFormula != 1u) return feAdd(feMul(z2, fzz), feTwice(feMul(lam, lam)));
    return feAdd(feMul(z2, fzz), feMul(fPrime2X(zf), feMul(lam, lam)));
}

// Interior cycle analysis: d2z/dc dz' = f'(z) cz + f''(z) dc lambda, d2z/dz2' = f''(z) lambda^2
// + f'(z) zz (z2 = f'(z)).
static inline FE fIcCZ(FE zf, FE z2, FE cz, FE dcx, FE lam) {
    if (kFormula != 1u) return feAdd(feMul(z2, cz), feMul(feTwice(dcx), lam));
    return feAdd(feMul(z2, cz), feMul(feMul(fPrime2X(zf), dcx), lam));
}

static inline FE fIcZZ(FE zf, FE z2, FE zz, FE lam) {
    if (kFormula != 1u) return feAdd(feMul(feTwice(lam), lam), feMul(z2, zz));
    return feAdd(feMul(feMul(fPrime2X(zf), lam), lam), feMul(z2, zz));
}

// F4: deep Julia sets (kFormula 2; kb: plans/f4-deep-julia, formulas/julia). The pixel is the
// start z_0 = centre + delta with k fixed: its delta starts at the pixel's offset (at the centre
// orbit's start p.centerStart, n = 0) and steps without +c (fStep and fStepX ignore it, the DE
// derivative dz/dz_0 gains no texel). The reference buffer holds the critical orbit (the orbit
// of 0 under z^2 + k, i.e. the Mandelbrot orbit of k; indices 0 ..= p.critLast), zeros, then the
// centre orbit: rebasing targets index 0 as in Mandelbrot, and each orbit ends at its own last
// index (segLast). Formula 0 folds all of this away.
constant bool kJulia = kFormula == 2u;

static inline uint segLast(uint m, uint last, uint critLast, uint centerStart) {
    return kJulia && m < centerStart ? critLast : last;
}

// n^2 for the derivative interior test (a Julia pixel starts at n = 0, where |w| n^2 = 0 would
// pass the test at once).
static inline float nSquared(uint n) {
    return kJulia && n == 0u ? 1e30f : float(n) * float(n);
}

// The DE's per-step texel term in the deep loop's scaled units (none for Julia).
static inline float2 tsdOf(FE TS, int dExp) {
    return kJulia ? float2(0.0f) : float2(TS.m.x * pow2f(min(TS.e - dExp, 100)), 0.0f);
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

// ---- M6: LA lookups and the AT (kb: techniques/linear-approximation-imagina "As built") ----

// |u| + K |c| < R for a deep entry, |u| = zn 2^ze, |c| = cAbsM 2^ce (relative to R's exponent, as
// the deep BLA test), and the block must end within the limit.
static inline bool laValidX(LaEntryX x, float zn, int ze, float cAbsM, int ce, uint n, uint maxIter) {
    if (!(x.blk.r > 0.0f) || n + x.len > maxIter) return false;
    const int kce = x.blk.ke + ce - x.blk.re;
    const float t = x.blk.r - (kce < -150 ? 0.0f : x.blk.k * cAbsM * pow2f(min(kce, 64)));
    return t > 0.0f && zn * pow2f(clamp(ze - x.blk.re, -127, 64)) < t;
}

static inline bool laValid(LaEntry x, float zn2, float cn, uint n, uint maxIter) {
    const float t = x.r - x.k * cn;
    return x.r > 0.0f && n + x.len <= maxIter && t > 0.0f && zn2 < t * t;
}

// The longest valid entry of a chain [first, end), or -1. Validity is monotone along the chain
// (each step is a prefix of the next stage's), so: the first entry decides whether any is
// valid (the common case where the delta is no longer linear: one test), then the last (pixels
// deep in the linear range: two tests), else a binary search. The order depends on the pixel's
// state alone, so results do not depend on how chunks cut the orbit.
static inline int laFindX(device const LaEntryX* E, uint first, uint end, float zn, int ze, float cAbsM, int ce,
                          uint n, uint maxIter) {
    if (first >= end || !laValidX(E[first], zn, ze, cAbsM, ce, n, maxIter)) return -1;
    uint lo = first, hi = end - 1;
    if (hi == lo || laValidX(E[hi], zn, ze, cAbsM, ce, n, maxIter)) return int(hi);
    while (hi - lo > 1) {
        const uint mid = (lo + hi) >> 1;
        if (laValidX(E[mid], zn, ze, cAbsM, ce, n, maxIter)) lo = mid; else hi = mid;
    }
    return int(lo);
}

static inline int laFind(device const LaEntry* E, uint first, uint end, float zn2, float cn, uint n, uint maxIter) {
    if (first >= end || !laValid(E[first], zn2, cn, n, maxIter)) return -1;
    uint lo = first, hi = end - 1;
    if (hi == lo || laValid(E[hi], zn2, cn, n, maxIter)) return int(hi);
    while (hi - lo > 1) {
        const uint mid = (lo + hi) >> 1;
        if (laValid(E[mid], zn2, cn, n, maxIter)) lo = mid; else hi = mid;
    }
    return int(lo);
}

// One thread: admits fresh pixels into free capacity, sizes the indirect dispatch for
// live + fresh pixels and clears the counters.
kernel void prepareChunk(constant IterateParams& p           [[buffer(0)]],
                         device uint* countIn                [[buffer(4)]],
                         device uint* countOut               [[buffer(5)]],
                         device ChunkStats* stats            [[buffer(6)]],
                         device uint* indirectArgs           [[buffer(7)]],
                         constant uint& threadsPerGroup      [[buffer(8)]],
                         device LevelCounters& level         [[buffer(9)]],
                         device uint* listCount              [[buffer(11)]],
                         device SeedStatus& seedStatus       [[buffer(20)]])
{
    if (p.firstChunk != 0) {
        *countIn = 0;
        level.admitted = 0;
        if (p.listMode != 0) *listCount = 0;
    }
    uint live = *countIn;
    uint fresh = min(p.total - level.admitted, p.capacity - live);
    // M5: a seeded level admits nothing until its last seed round finished every node (the
    // chunk is then a no-op; the next round and chunks follow).
    if (p.seedGate != 0 && atomic_load_explicit(&seedStatus.unfinished, memory_order_relaxed) != 0) fresh = 0;
    // M5 interior fill: the tile borders first; the insides only once every border pixel is
    // decided (finished, or past fillProof iterations, which spoils its tile), so a tile's fill
    // never depends on chunk timing, and the wait is at most fillProof iterations.
    if (p.fillBorder != 0) {
        if (level.admitted < p.fillBorder) fresh = min(fresh, p.fillBorder - level.admitted);
        else if (p.firstChunk == 0 && level.undecided != 0) fresh = 0;
    }
    level.undecided = 0;
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
    atomic_store_explicit(&stats[p.statSlot].liveMinN, 0xffffffffu, memory_order_relaxed);
}

// One thread: records the live count after the chunk for the CPU scheduler.
kernel void finishChunk(constant IterateParams& p            [[buffer(0)]],
                        device const uint* countOut          [[buffer(5)]],
                        device ChunkStats* stats             [[buffer(6)]])
{
    stats[p.statSlot].liveOut = *countOut;
}

// M5: local series approximation seed passes (kb: techniques/local-series-approximation).
// A seeding is a sequence of rounds, one command buffer each: seedBegin, then one seedLevel
// dispatch per quadtree level (root first; one thread per node), then seedGate. A node runs
// at most `budget` seed steps per round and keeps its whole state, so the result does not
// depend on how rounds cut the work (shown frame = fresh render). A child waits for its parent
// to be done (counted as unfinished). The round's unfinished count gates pixel admission in
// prepareChunk: no pixel starts before every leaf is final.
// Node centres are texel corners (u, v) = (i + 1/2) size; a sample at texel x has the complex
// offset of (x + jitter), the node that of ((u, v) + jitter - 1/2) (same level jitter).


// Interior probe, evaluation of a candidate period P at the base point z_B (kb:
// techniques/interior-detection "Attracting-cycle probe"; see the kernel): g = F(z_B) - z_B,
// lambda = F'(z_B), fzz = F''(z_B) for F = f^P, lgMag = log2 of the magnitudes g's float32
// noise is relative to. PA_PROVEN: an attracting fixed point of F exists (c is interior);
// PA_NEWTON: try again from z_B + Delta (delta = -Delta); located but not attracting enough:
// PA_REPEL (surely repelling: look for longer periods) or PA_STUCK (at the noise floor, give
// up); PA_REJECT.
static inline uint probeEval(FE g, FE lam, FE fzz, FE zB, float lgMag, uint P, uint info, thread FE& delta) {
    delta = FE{float2(0.0f), FE_ZERO};
    const FE d1 = feSub(lam, FE{float2(0.5f, 0.0f), 1});
    const float lgD = feLog2(d1);
    if (!(lgD > -1e29f)) return PA_REJECT;
    delta = feDiv(g, d1);
    const float lgEta = -22.0f + log2(1.0f + sqrt(float(P))) + lgMag;
    const float lgZB = feLog2(zB);
    const float lgL = 1.0f + feLog2(fzz);
    const float lgDelta = feLog2(delta);
    const float lgH = lgL + lgSum(feLog2(g), lgEta) - 2.0f * lgD;
    const float lgR = 1.0f + lgSum(lgDelta, lgEta - lgD);
    const float lamAbs = exp2(min(feLog2(lam), 8.0f)), spread = exp2(min(lgL + lgR, 8.0f));
    const float mu = 0x1p-20f * (float(P) + 16.0f);
    if (lgH <= -2.0f && lgR <= lgZB - 6.0f) {
        // The fixed point z* is located: |F'(z*) - lambda| <= L r.
        if (lamAbs + spread <= 1.0f - mu) return PA_PROVEN;
        // Surely repelling: a parent cycle inside a satellite component of period q P, or a
        // ghost on an exterior orbit (look for longer periods).
        if (lamAbs - spread > 1.0f + 0x1p-10f) return PA_REPEL;
        // Newton is at its noise floor (it cannot shrink r further): near-parabolic, no
        // float32 proof will come (give up).
        if (lgDelta <= lgEta - lgD + 1.0f) return PA_STUCK;
    }
    if ((lgH <= 1.0f || lgDelta <= lgZB - 2.0f) && lgDelta <= lgZB) return PA_NEWTON;
    return PA_REJECT;
}

// Interior-test value |w| n^2 of a floatexp w.
static inline float feWTest(FE w, uint n) {
    return max(abs(w.m.x), abs(w.m.y)) * (float(n) * float(n)) * pow2f(clamp(w.e, -126, 127));
}

static inline float mabs(float2 v) {
    return max(abs(v.x), abs(v.y));
}

// Deep views with texels of at least 2^this seed in float32 (scaled units), deeper in floatexp.
constant int SEED_FLOAT_MIN_EXP = -80;

// Texel offsets (x right, y down) as complex offsets in texel units, turned by the view
// rotation as the iterate kernel turns its pixels' (kb: decisions/controls "View rotation").
static inline float2 texelOffset(constant IterateParams& p, float2 d) {
    const float2 o = float2(d.x, -d.y);
    return float2(o.x * p.rot.x - o.y * p.rot.y, o.x * p.rot.y + o.y * p.rot.x);
}

static inline float2 seedDc(constant IterateParams& p, float2 q, float2 jitter) {
    return texelOffset(p, q + (jitter - 0.5f) - p.halfSize) * p.texelScale + p.refOffset;
}

kernel void seedBegin(device SeedStatus& status [[buffer(20)]])
{
    atomic_store_explicit(&status.unfinished, 0u, memory_order_relaxed);
    atomic_store_explicit(&status.seededTiles, 0u, memory_order_relaxed);
    atomic_store_explicit(&status.pathLo, 0u, memory_order_relaxed);
    atomic_store_explicit(&status.pathHi, 0u, memory_order_relaxed);
}

kernel void seedLevel(constant IterateParams& p    [[buffer(0)]],
                      device const float2* ref     [[buffer(1)]],
                      constant BlaParams& bla      [[buffer(14)]],
                      device const RefX* refx      [[buffer(16)]],
                      device const BlaEntryX* tableX [[buffer(17)]],
                      device SeedNode* nodes       [[buffer(18)]],
                      constant SeedParams& sp      [[buffer(19)]],
                      device SeedStatus& status    [[buffer(20)]],
                      uint tid                     [[thread_position_in_grid]])
{
    if (tid >= sp.gridW * sp.gridH) return;
    const uint idx = sp.offset + tid;
    // (Done: state 2, or 3 when the seed stopped proven interior.)
    const uint stampDone = (sp.epoch << 2) | 2u, stampRunning = (sp.epoch << 2) | 1u;
    SeedNode s = nodes[idx];
    if ((s.stamp | 1u) == (stampDone | 1u)) return;
    const uint2 nxy = uint2(tid % sp.gridW, tid / sp.gridW);
    const float2 q = (float2(nxy) + 0.5f) * float(sp.size);
    const float2 dc = seedDc(p, q, sp.jitter);
    const uint last = p.refLen - 1;
    const bool start = s.stamp != stampRunning;
    float2 delta = 0.0f;
    if (start) {
        if (sp.root != 0) {
            s.dz = 0.0f; s.a = 0.0f; s.b = 0.0f;
            s.dzExp = FE_ZERO; s.aExp = FE_ZERO; s.bExp = FE_ZERO;
            s.m = 0; s.n = 0;
            s.w = float2(0.5f, 0.0f); s.wExp = 1;
            s.wd = 0.0f; s.wdExp = FE_ZERO;
            s.path = 0;
            s.icMin = 1e30f;
            s.icQ = 0;
        } else {
            const uint2 pxy = nxy / 2u;
            const SeedNode par = nodes[sp.parentOffset + pxy.y * sp.parentGridW + pxy.x];
            if ((par.stamp | 1u) != (stampDone | 1u)) {
                atomic_fetch_add_explicit(&status.unfinished, 1u, memory_order_relaxed);
                return;
            }
            s = par;
            delta = texelOffset(p, q - (float2(pxy) + 0.5f) * float(2u * sp.size));
        }
        s.steps = 0;
    }
    uint steps = s.steps, path = s.path;
    uint m = s.m, n = s.n;
    float icMin = s.icMin;
    uint icQ = s.icQ;
    uint budget = sp.budget;
    bool done = true, provenInterior = false;
    FE W = FE{s.w, s.wExp}, WD = FE{s.wd, s.wdExp};
    if (start && sp.root == 0 && kInterior) W = feAdd(W, feMul(WD, feNorm(delta, 0)));
    if (kDeep && p.scaleExp < SEED_FLOAT_MIN_EXP) {
        const FE C = feNorm(dc, p.scaleExp);
        const FE TS = feNorm(float2(p.texelScale, 0.0f), p.scaleExp);
        FE dz = FE{s.dz, s.dzExp}, A = FE{s.a, s.aExp}, B = FE{s.b, s.bExp};
        if (start && sp.root == 0) {
            const FE D = feNorm(delta, 0);
            const FE BD = feMul(B, D);
            dz = feAdd(dz, feMul(feAdd(A, FE{BD.m, BD.e - 1}), D));
            A = feAdd(A, BD);
            FE z = feAdd(refAt(refx, m), dz);
            if (m != 0 && (m == last || feLess(z, dz))) { dz = z; m = 0; }
        }
        const FE R = feNorm(float2(sp.radius, 0.0f), 0);
        const FE L = feNorm(float2(2.0f * sp.eps, 0.0f), 0);
        const uint alignMask = kBLA ? bla.lookupEvery - 1u : 0u;
        // |c| + |texel| r: the largest |c| of the node's region (BLA validity).
        const FE cMax = feAdd(feNorm(float2(length(C.m), 0.0f), C.e),
                              feNorm(float2(TS.m.x * sp.radius, 0.0f), TS.e));
        while (n < sp.softIter) {
            if (budget == 0) { done = sp.truncate != 0u; break; }
            // BLA blocks (kBLA, deep views): z_{m+l} = A z_m + B c is affine in (z, c), so the
            // node's expansion maps exactly: dz' = A dz + B c, a' = A a + B texel, b' = A b,
            // valid for the whole region when |dz| + |a| r (1 + eps) + K (|c| + |texel| r) < R.
            if (kBLA && m != 0 && ((m - 1) & alignMask) == 0) {
                const FE zMax = feAdd(feNorm(float2(length(dz.m), 0.0f), dz.e),
                                      feNorm(float2(length(A.m) * sp.radius * (1.0f + sp.eps), 0.0f), A.e));
                const uint top = min(ctz(m - 1), bla.maxLevel);
                BlaEntryX best;
                uint len = 0;
                for (uint lev = bla.minLevel; lev <= top; lev++) {
                    const uint l = 1u << lev;
                    if (m + l > last || n + l > sp.softIter) break;
                    const BlaEntryX x = tableX[bla.offset[lev] + ((m - 1) >> lev)];
                    if (!(x.r > 0.0f)) break;
                    const int kce = x.ke + cMax.e - x.re;
                    const float t = x.r - (kce < -150 ? 0.0f : x.k * cMax.m.x * pow2f(min(kce, 64)));
                    if (!(t > 0.0f && zMax.m.x * pow2f(clamp(zMax.e - x.re, -127, 64)) < t)) break;
                    best = x;
                    len = l;
                }
                if (len != 0) {
                    const FE BA = FE{best.a, best.ae}, BB = FE{best.b, best.be};
                    const FE A2 = feAdd(feMul(BA, A), feMul(BB, TS));
                    const FE B2 = feMul(BA, B);
                    steps++;
                    if (feLess(feMul(A2, L), feMul(B2, R))) break;
                    dz = feAdd(feMul(BA, dz), feMul(BB, C));
                    A = A2;
                    B = B2;
                    if (kInterior) {
                        W = feMul(BA, W);
                        WD = feMul(BA, WD);
                    }
                    m += len;
                    n += len;
                    budget--;
                    path++;
                    const FE zb = feAdd(refAt(refx, m), dz);
                    if (m == last || feLess(zb, dz)) { dz = zb; m = 0; }
                    continue;
                }
            }
            const FE Zx = refAt(refx, m);
            const FE z = feAdd(Zx, dz);
            const FE z2 = fPrimeX(z);
            const FE A2 = feAdd(feMul(z2, A), TS);
            const FE B2 = fSeedBX(z, A, B);
            const FE dz2 = fStepX(Zx, dz, C);
            const FE zn = feAdd(refAt(refx, m + 1), dz2);
            steps++;
            if (zn.e > 0) {
                if (zn.e > 64) break;
                const float2 f = feFloat(zn);
                if (dot(f, f) > p.bailout2) break;
            }
            if (feLess(feMul(A2, L), feMul(B2, R))) break;
            if (kInteriorColor && n != 0) {   // (z_0 = 0 is not an approach)
                const float zl = length(z.m);
                const float l = zl > 0.0f ? log2(zl) + float(z.e) : -1e30f;
                if (l < icMin) { icMin = l; icQ = n; }
            }
            if (kInterior && n != 0) {   // (w_1 = 1, see the plain path)
                WD = fSeedWDX(z, A, W, WD);
                W = feMul(z2, W);
            }
            dz = dz2; A = A2; B = B2;
            m++; n++;
            budget--;
            path++;
            if (m == last || feLess(zn, dz)) { dz = zn; m = 0; }
            if (kInterior && (n & 7u) == 0u && feWTest(W, n) < p.interiorEps) {
                provenInterior = true;
                break;
            }
        }
        s.dz = dz.m; s.dzExp = dz.e; s.a = A.m; s.aExp = A.e; s.b = B.m; s.bExp = B.e;
    } else {
        // Float32 seeds: the plain kernel, and the deep kernel down to texels of 2^-80 (M5:
        // floatexp seed steps cost ~10x a float step, and the root level runs on few threads
        // where only the step latency counts: 47 ms of serial root seeds at the
        // spiral-32412-1e19 view). In units: a = dz/dc * texel, b = d2z/dc2 * texel, so a / ts
        // = dz/dc stays below ~1/ts and a^2 is never formed. Deep views compare max-norms
        // (squared norms of 1e-19 deltas underflow) and take BLA blocks from the floatexp
        // table, converted per lookup.
        const float sc = kDeep ? pow2f(p.scaleExp) : 1.0f;
        const float ts = p.texelScale * sc, invTs = 1.0f / ts;
        const float2 dcf = dc * sc;
        float2 dz, A, B;
        if (kDeep) {
            // Node state in floatexp (the deep pixel start reads it so), b in these units
            // (d2z/dc2 * texel, not * texel^2: exact round trips between rounds).
            dz = feFloat(FE{s.dz, s.dzExp});
            A = feFloat(FE{s.a, s.aExp});
            B = feFloat(FE{s.b, s.bExp});
        } else {
            dz = s.dz; A = s.a; B = s.b;
        }
        if (start && sp.root == 0) {
            dz += cmul(A, delta) + (0.5f * ts) * cmul(B, cmul(delta, delta));
            A += ts * cmul(B, delta);
            const float2 z = ref[m] + dz;
            if (kDeep) {
                if (m != 0 && (mabs(z) < mabs(dz) || m == last)) { dz = z; m = 0; }
            } else {
                if (m != 0 && (dot(z, z) < dot(dz, dz) || m == last)) { dz = z; m = 0; }
            }
        }
        // Accept while |B ts| r <= 2 eps |A|.
        const float lim = 2.0f * sp.eps / (ts * sp.radius);
        const uint alignMask = kBLA ? bla.lookupEvery - 1u : 0u;
        const float cMax = kBLA ? mabs(dcf) * 1.4143f + ts * sp.radius : 0.0f;
        while (n < sp.softIter) {
            if (budget == 0) { done = sp.truncate != 0u; break; }
            if (kDeep && kBLA && m != 0 && ((m - 1) & alignMask) == 0) {
                // As the floatexp seeds: the longest block valid for the whole node.
                const float zMax = mabs(dz) * 1.4143f + mabs(A) * 1.4143f * sp.radius * (1.0f + sp.eps);
                const uint top = min(ctz(m - 1), bla.maxLevel);
                BlaEntryX best;
                uint len = 0;
                for (uint lev = bla.minLevel; lev <= top; lev++) {
                    const uint l = 1u << lev;
                    if (m + l > last || n + l > sp.softIter) break;
                    const BlaEntryX x = tableX[bla.offset[lev] + ((m - 1) >> lev)];
                    if (!(x.r > 0.0f)) break;
                    const float t = x.r * pow2f(clamp(x.re, -126, 127)) - x.k * pow2f(clamp(x.ke, -126, 127)) * cMax;
                    if (!(t > 0.0f && zMax < t)) break;
                    best = x;
                    len = l;
                }
                if (len != 0) {
                    const float2 BA = scale2(best.a, clamp(best.ae, -250, 250));
                    const float2 BB = scale2(best.b, clamp(best.be, -250, 250));
                    const float2 A2 = cmul(BA, A) + BB * ts;
                    const float2 B2 = cmul(BA, B);
                    steps++;
                    if (!(mabs(B2) <= lim * mabs(A2))) break;
                    dz = cmul(BA, dz) + cmul(BB, dcf);
                    A = A2;
                    B = B2;
                    if (kInterior) {
                        const FE fa = feNorm(best.a, best.ae);
                        W = feMul(fa, W);
                        WD = feMul(fa, WD);
                    }
                    m += len;
                    n += len;
                    budget--;
                    path++;
                    const float2 zb = ref[m] + dz;
                    if (m == last || mabs(zb) < mabs(dz)) { dz = zb; m = 0; }
                    continue;
                }
            }
            const float2 Z = ref[m];
            const float2 z = Z + dz;
            const float2 A2 = fMulPrime(z, A) + float2(ts, 0.0f);
            const float2 B2 = fSeedB(z, A, B, invTs);
            const float2 dz2 = fStep(Z, dz, dz, dcf);
            const float2 zn = ref[m + 1] + dz2;
            steps++;
            const float r2 = dot(zn, zn);
            if (!(r2 <= p.bailout2)) break;
            if (kDeep) {
                if (!(mabs(B2) <= lim * mabs(A2))) break;
            } else {
                if (!(length(B2) <= lim * length(A2))) break;
            }
            if (kInteriorColor && n != 0) {   // (z_0 = 0 is not an approach)
                const float zz = mabs(z);
                const float l = zz > 0.0f ? log2(zz) + 0.5f * log2(dot(z / zz, z / zz)) : -1e30f;
                if (l < icMin) { icMin = l; icQ = n; }
            }
            // (w_1 = 1: the step from z_0 = 0 would zero it, as in the pixel loop's first step.)
            if (kInterior && n != 0) {
                const FE zf = feNorm(z, 0);
                WD = fSeedWDX(zf, feNorm(A, 0), W, WD);
                W = feMul(fPrimeX(zf), W);
            }
            dz = dz2; A = A2; B = B2;
            m++; n++;
            budget--;
            path++;
            if (kDeep) {
                if (mabs(zn) < mabs(dz) || m == last) { dz = zn; m = 0; }
            } else {
                if (r2 < dot(dz, dz) || m == last) { dz = zn; m = 0; }
            }
            if (kInterior && (n & 7u) == 0u && feWTest(W, n) < p.interiorEps) {
                provenInterior = true;
                break;   // interior seed: branch here (samples inherit w + wd delta)
            }
        }
        if (kDeep) {
            const FE fz = feNorm(dz, 0), fa = feNorm(A, 0), fb = feNorm(B, 0);
            s.dz = fz.m; s.dzExp = fz.e; s.a = fa.m; s.aExp = fa.e; s.b = fb.m; s.bExp = fb.e;
        } else {
            s.dz = dz; s.a = A; s.b = B;
            s.dzExp = 0; s.aExp = 0; s.bExp = 0;
        }
    }
    s.m = m; s.n = n; s.steps = steps; s.path = path;
    s.w = W.m; s.wExp = W.e; s.wd = WD.m; s.wdExp = WD.e;
    s.icMin = icMin; s.icQ = icQ;
    s.stamp = done ? (stampDone | (provenInterior ? 1u : 0u)) : stampRunning;
    if (!done) atomic_fetch_add_explicit(&status.unfinished, 1u, memory_order_relaxed);
    nodes[idx] = s;
}

// After the leaf level: which 16 x 16 tiles start their pixels from the seeds (one thread per
// tile). Starting a pixel at its leaf's reference index costs SIMD coherence (neighbouring
// leaves start at unrelated m, and pixels that run long keep the phase offset for their whole
// orbit: ~10-25 % per remaining step), so a tile is seeded only where its leaves saved a good
// share of a pixel's work: mean path (seed steps a pixel saves) >= gateAbs,
// which the scheduler sets to a share of the last level's steps per texel; without that
// estimate (a fresh renderer), mean branch iteration >= gateRel * softIter. Measured: home-100k,
// Feigenbaum, minibrot-p4, sea-8007 5-25 % slower seeded, dense/spiral/neck 3-6x faster.
kernel void seedGate(device const SeedNode* nodes [[buffer(18)]],
                     constant SeedParams& sp      [[buffer(19)]],
                     device SeedStatus& status    [[buffer(20)]],
                     device uint* tiles           [[buffer(22)]],
                     uint tid                     [[thread_position_in_grid]])
{
    if (tid >= sp.tilesX * sp.tilesY) return;
    const uint2 t = uint2(tid % sp.tilesX, tid / sp.tilesX);
    const uint per = max(16u / sp.size, 1u);
    const uint2 l0 = t * per;
    float pathSum = 0.0f, nSum = 0.0f;
    uint count = 0;
    for (uint y = l0.y; y < min(l0.y + per, sp.gridH); y++) {
        for (uint x = l0.x; x < min(l0.x + per, sp.gridW); x++) {
            const SeedNode nd = nodes[sp.offset + y * sp.gridW + x];
            pathSum += float(nd.path);
            nSum += float(nd.n);
            count++;
        }
    }
    const bool bySteps = sp.gateAbs > 0.0f;
    const float need = bySteps ? sp.gateAbs : sp.gateRel * float(sp.softIter);
    const bool seeded = count > 0 && (bySteps ? pathSum : nSum) >= need * float(count);
    tiles[tid] = seeded ? 1u : 0u;
    if (seeded) {
        atomic_fetch_add_explicit(&status.seededTiles, 1u, memory_order_relaxed);
        const uint v = uint(pathSum / float(count));
        const uint old = atomic_fetch_add_explicit(&status.pathLo, v, memory_order_relaxed);
        if (old + v < old) atomic_fetch_add_explicit(&status.pathHi, 1u, memory_order_relaxed);
    }
}

// Perturbation: z = Z + dz, with Z the reference orbit.
//   dz_{n+1} = 2 Z_n dz_n + dz_n^2 + dc = (2 Z_n + dz_n) dz_n + dc
// (Mandelbrot; every step and derivative goes through the formula hooks, kb: plans/f2-pluggable-formulas.)
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
                    device const SeedNode* seeds            [[buffer(20), function_constant(kSeeded)]],
                    constant SeedParams& leaf               [[buffer(21), function_constant(kSeeded)]],
                    device const uint* seedTiles            [[buffer(22), function_constant(kSeeded)]],
                    device atomic_uint* tileBad             [[buffer(23)]],
                    device atomic_uint* levelAtomic         [[buffer(24)]],
                    device const uint2* laBounds            [[buffer(25), function_constant(kLA)]],
                    device const LaEntryX* laX              [[buffer(26), function_constant(kLA)]],
                    device const LaEntry* laP               [[buffer(27), function_constant(kLA)]],
                    device const uint* laNext               [[buffer(28), function_constant(kLA)]],
                    constant LaParams& la                   [[buffer(29), function_constant(kLA)]],
                    device PixelStateProbe* probeBuf        [[buffer(30)]],
                    constant ComponentParams& comp          [[buffer(7)]],
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
    // (F4 Julia: the DE derivative dz/dz_0 gains no texel per step.)
    const float2 tsAddP = kJulia ? float2(0.0f) : tsDelta;
    // The last index of the orbit m is on (F4 Julia: the critical or the centre orbit; else
    // the one orbit, `last`).
#define LAST(mv) segLast((mv), last, p.critLast, p.centerStart)

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
            active = tileTexel(a, p.tilesX, p.width, p.height, p.tileOrder, order, xy, p.fillBorder);
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
    // M5 interior fill: an inside texel of a whole tile whose border is all proven interior.
    const uint tileOf = (gid.y >> 4) * p.tilesX + (gid.x >> 4);
    bool filled = false;
    if (p.fillBorder != 0 && fresh && active && !onRing(gid) && (gid.x | 15u) < p.width && (gid.y | 15u) < p.height
        && atomic_load_explicit(&tileBad[tileOf], memory_order_relaxed) != p.fillEpoch) {
        filled = true;
        interior = true;
        st.n = 0;
    }
    // Closed-form component test (see ComponentParams): a fresh texel provably inside the main
    // cardioid or the period-2 bulb is interior without iterating (also in previews and
    // gesture frames, where it costs ~50 flops per texel).
    uint compPeriod = 0u;
    if (comp.flags != 0u && fresh && active && !filled) {
        const float cu = pix.x * comp.invL, cv = -pix.y * comp.invL;
        if (max(abs(cu), abs(cv)) <= 1.0f) {
            float m[15], mag;
            compMonomials(cu, cv, m);
            if ((comp.flags & 1u) != 0u) {
                const float sc = compSum(comp.cardioid, m, mag);
                if (sc < -(COMP_GAMMA * mag + comp.slack[0])) compPeriod = 1u;
            }
            if (compPeriod == 0u && (comp.flags & 2u) != 0u) {
                const float sb = compSum(comp.bulb, m, mag);
                if (sb < -(COMP_GAMMA * mag + comp.slack[1])) compPeriod = 2u;
            }
        }
        if (compPeriod != 0u) {
            filled = true;
            interior = true;
            st.n = 0;
        }
    }
    float r2 = 0.0f;
    float2 z = 0.0f;
    uint iters = 0;
    uint steps = 0;

    // M5 (kSeeded, kb: techniques/local-series-approximation): a fresh pixel of a seeded tile
    // starts from its leaf node, offset by its sample position from the node centre (texels;
    // extension passes and AA samples of the level use the same leaves, AA with their own
    // jitter).
    SeedNode sd;
    float2 sdDelta = 0.0f;
    bool fromSeed = false;
    if (kSeeded && fresh && active) {
        fromSeed = seedTiles[(gid.y >> 4) * leaf.tilesX + (gid.x >> 4)] != 0;
        // Real reference: pixels within AXIS_BAND texels of the real axis iterate plainly. The
        // seeds' error (a start ~eps^2 |delta| off, delta up to a root node's radius) is
        // isotropic, but near the needle the escape depends on Im c alone: relative to Im c it is
        // |delta| / |Im c| times larger, so axis rows (exact: plain keeps Im = 0) and their
        // neighbours lost their escape counts (kb: techniques/local-series-approximation
        // "Real axis").
        if (fromSeed && p.realAxis != 0 && abs(dc.y) < AXIS_BAND * p.texelScale) fromSeed = false;
        if (fromSeed) {
            const uint2 lxy = gid / leaf.size;
            sd = seeds[leaf.offset + lxy.y * leaf.gridW + lxy.x];
            sdDelta = texelOffset(p, (float2(gid) + p.jitter)
                - ((float2(lxy) + 0.5f) * float(leaf.size) + leaf.jitter - 0.5f));
        }
    }

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
    // (A seeded pixel inherits its leaf's closest approach: the orbit up to the branch is the
    // seed's within the expansion's error.)
    if (kInteriorColor && fromSeed && sd.icMin < 1e29f) { ic.minLog2 = sd.icMin; ic.k = sd.icQ; }
    bool inB = kInteriorColor && (ic.capFlags & IC_ACTIVE) != 0;
    float icR2 = kInteriorColor ? exp2(min(2.0f * ic.minLog2, 127.0f)) : 0.0f;
    float icMinLog2 = ic.minLog2;
    uint icQ = ic.k;
    // Interior probe (see "Attracting-cycle probe" after the main loops): with p.probe on, a
    // pixel unresolved at maxIter stays live ("parked", n = maxIter) until a kProbe pipeline
    // runs it; n > maxIter: probing (its probe state is valid).
    const bool parked = kInterior && p.probe != 0 && active && !fresh && !inB && st.n >= p.maxIter;
    const bool inP = kProbe && parked && st.n > p.maxIter;
    // A probing pixel in a chunk of the lean pipeline (chunks in flight may still be lean when
    // probing started) keeps its probe state: copied to the slot it moves to. (Before, it was
    // left behind in the other half, and the next probe chunk read another pixel's.)
    const bool carryP = !kProbe && parked && st.n > p.maxIter;
#define IC_TRACK(r2v, nv) if (kInteriorColor && (r2v) < icR2) { icR2 = (r2v); icMinLog2 = 0.5f * log2(icR2); icQ = (nv); }

    if (active && !inB && !parked && !filled && kDeep) {
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
        if (fromSeed) {
            // Start from the leaf's expansion: dz + a delta + b delta^2/2, d = a + b delta,
            // w + wd delta.
            const FE D = feNorm(sdDelta, 0);
            const FE A = FE{sd.a, sd.aExp};
            // (Float32 seeds keep b = d2z/dc2 * texel, floatexp seeds b * texel.)
            const FE Bn = p.scaleExp >= SEED_FLOAT_MIN_EXP ? feMul(FE{sd.b, sd.bExp}, TS) : FE{sd.b, sd.bExp};
            const FE BD = feMul(Bn, D);
            const FE zx = feAdd(FE{sd.dz, sd.dzExp}, feMul(feAdd(A, FE{BD.m, BD.e - 1}), D));
            if (zx.e >= DEEP_PLAIN_EXP) { w = feFloat(zx); e = 0; } else { w = zx.m; e = zx.e; }
            const FE dx = feAdd(A, BD);
            dm = dx.m; dExp = dx.e;
            const FE wx = kInterior ? feAdd(FE{sd.w, sd.wExp}, feMul(FE{sd.wd, sd.wdExp}, D)) : FE{float2(1.0f, 0.0f), 0};
            zw = wx.m; wExp = wx.e;
            m = sd.m; n = sd.n;
            st.n = n;
        } else if (fresh) {
            // z_1 = c, d_1 = texel, w_1 = 1. (F4 Julia: z_0 = the pixel's offset from the centre
            // orbit's start, d_0 = texel, w_0 = 1, n = 0.)
            if (C.e >= DEEP_PLAIN_EXP) { w = feFloat(C); e = 0; } else { w = C.m; e = C.e; }
            dm = TS.m; dExp = TS.e;
            zw = float2(1.0f, 0.0f); wExp = 0;
            if (kJulia) {
                m = p.centerStart; n = 0;
                st.n = 0;
            } else {
                m = 1; n = 1;
                st.n = 0;   // the first iteration counts in this chunk
                steps = 1;
            }
        } else {
            w = st.dz; e = st.e; dm = st.d; dExp = st.dExp; zw = st.w; wExp = st.wExp;
            m = st.m; n = st.n;
        }
        float s;
        float2 dd;
        deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
        float2 tsd = tsdOf(TS, dExp);
        uint budget = p.chunkIters;
        const uint alignMask = kBLA ? bla.lookupEvery - 1u : 0u;
        uint lookedAt = 0xffffffffu;   // n of the last BLA lookup (one per lookup index visit)
        // M6 LA: the next lookup boundary j and its index L_j (la0: after a rebase).
        uint laJ = 0, laLook = 0xffffffffu, la0 = 0xffffffffu;
        if (kLA) {
            la0 = laBounds[0].x;
            laJ = laNext[min(m, last)];
            laLook = laBounds[laJ].x;
        }
        bool done = false;
        while (!done) {
            // ---- M6 LA (kLA): at m == L_j, the longest valid block of boundary j's chain (a
            // valid block keeps |z| <= 2^-24 |2 Z| at its inner steps: no rebase or escape inside;
            // the tests follow in the regime loop).
            if (kLA && m == laLook && n < p.maxIter && budget > 0 && n != lookedAt) {
                lookedAt = n;
                FE zx = feNorm(w, e);
                const int k = laFindX(laX, laBounds[laJ].y, laBounds[laJ + 1].y, length(zx.m), zx.e, cAbsM, C.e,
                                      n, p.maxIter);
                if (k >= 0) {
                    const LaEntryX x = laX[k];
                    const uint len = x.len;
                    const FE A = FE{x.blk.a, x.blk.ae}, B = FE{x.blk.b, x.blk.be};
                    zx = feAdd(feMul(A, zx), feMul(B, C));
                    if (kDE) {
                        FE dx = feAdd(feMul(A, feNorm(dm, dExp)), feMul(B, TS));
                        dm = dx.m; dExp = dx.e;
                        tsd = tsdOf(TS, dExp);
                    }
                    if (kInterior) {
                        FE wx = feMul(A, feNorm(zw, wExp));
                        zw = wx.m; wExp = wx.e;
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
                    laJ = x.endJ;
                    laLook = laBounds[laJ].x;
                    if (zx.e >= DEEP_PLAIN_EXP) { w = feFloat(zx); e = 0; } else { w = zx.m; e = zx.e; }
                    deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
                } else {
                    laJ = min(laJ + 1u, la.count);
                    laLook = laBounds[laJ].x;
                }
            }
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
                    if (m + l > LAST(m) || n + l > p.maxIter) break;
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
                        tsd = tsdOf(TS, dExp);
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
                    if (r2 < w2 || m == LAST(m)) {
                        w = z;
                        m = 0;
                        Z = 0.0f;
                        w2 = r2;
                        if (kLA) { laJ = 0; laLook = la0; }
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
                    if (kLA && m == laLook && n != lookedAt) break;
                    if (kDE) dm = fMulPrime(z, dm) + tsd;
                    if (kInterior) zw = fMulPrime(z, zw);
                    // (Also every step once |z| > 16: escape is near and |2z| up to 2^17 per step
                    // would overflow the DE mantissa within 8 steps; R5.)
                    if ((n & 7u) == 0u || max(abs(z.x), abs(z.y)) < 0x1p-16f
                        || (kDE && max(abs(z.x), abs(z.y)) > 16.0f)) {
                        if (kDE) {
                            float a = max(abs(dm.x), abs(dm.y));
                            if (a > 0x1p40f || (a < 0x1p-40f && a > 0.0f)) {
                                FE t = feNorm(dm, dExp);
                                dm = t.m; dExp = t.e;
                                tsd = tsdOf(TS, dExp);
                            }
                        }
                        if (kInterior) {
                            float a = max(abs(zw.x), abs(zw.y));
                            if (a > 0x1p40f || (a < 0x1p-40f && a > 0.0f)) {
                                FE t = feNorm(zw, wExp);
                                zw = t.m; wExp = t.e;
                                a = max(abs(zw.x), abs(zw.y));
                            }
                            if (a * nSquared(n) * pow2f(max(wExp, -126)) < p.interiorEps) {
                                interior = true;
                                done = true;
                                break;
                            }
                        }
                    }
                    w = fStep(Z, w, w, dd);
                    m++;
                    n++;
                    budget--;
                }
            } else {
                // ---- T regime: scaled steps; floatexp where |Z| is small.
                while (true) {
                    const float2 Z = ref[m];
                    if (m == 0 || m == LAST(m) || max(abs(Z.x), abs(Z.y)) < DEEP_SMALL_Z) {
                        FE zx = feNorm(w, e);
                        FE Zx = refAt(refx, m);
                        FE zf = feAdd(Zx, zx);
                        if (m == LAST(m) && zf.e > 0) {
                            // Only an escaped reference ends outside the bailout.
                            z = feFloat(zf);
                            r2 = dot(z, z);
                            if (r2 > p.bailout2) { escaped = true; done = true; break; }
                        }
                        if (m != 0 && (m == LAST(m) || feLess(zf, zx))) {
                            zx = zf;
                            m = 0;
                            Zx = FE{float2(0.0f), FE_ZERO};
                            if (kLA) { laJ = 0; laLook = la0; }
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
                        if (kLA && m == laLook && n != lookedAt) {
                            deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
                            break;
                        }
                        // Derivatives with the full orbit value zf = Z + z (floatexp).
                        if (kDE) {
                            FE dx = kJulia ? feMul(fPrimeX(zf), feNorm(dm, dExp)) : feAdd(feMul(fPrimeX(zf), feNorm(dm, dExp)), TS);
                            dm = dx.m; dExp = dx.e;
                            tsd = tsdOf(TS, dExp);
                        }
                        if (kInterior) {
                            FE wx = feMul(fPrimeX(zf), feNorm(zw, wExp));
                            zw = wx.m; wExp = wx.e;
                        }
                        // z <- (2Z + z) z + c in floatexp (the formula's step).
                        zx = fStepX(Zx, zx, C);
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
                    if (kLA && m == laLook && n != lookedAt) break;
                    const float2 sw = s * w;
                    const float2 zf = Z + sw;
                    IC_TRACK(dot(zf, zf), n)
                    if (kDE) dm = fMulPrime(zf, dm) + tsd;
                    if (kInterior) zw = fMulPrime(zf, zw);
                    if ((n & 7u) == 0u || max(abs(zf.x), abs(zf.y)) < 0x1p-16f) {
                        if (kDE) {
                            float a = max(abs(dm.x), abs(dm.y));
                            if (a > 0x1p40f || (a < 0x1p-40f && a > 0.0f)) {
                                FE t = feNorm(dm, dExp);
                                dm = t.m; dExp = t.e;
                                tsd = tsdOf(TS, dExp);
                            }
                        }
                        if (kInterior) {
                            float a = max(abs(zw.x), abs(zw.y));
                            if (a > 0x1p40f || (a < 0x1p-40f && a > 0.0f)) {
                                FE t = feNorm(zw, wExp);
                                zw = t.m; wExp = t.e;
                                a = max(abs(zw.x), abs(zw.y));
                            }
                            if (a * nSquared(n) * pow2f(max(wExp, -126)) < p.interiorEps) {
                                interior = true;
                                done = true;
                                break;
                            }
                        }
                    }
                    w = fStep(Z, sw, w, dd);
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
    } else if (active && !inB && !parked && !filled) {
        if (fromSeed) {
            // M5: start from the leaf's expansion (iterations counted from its n).
            const float ts = p.texelScale;
            st.dz = sd.dz + cmul(sd.a, sdDelta) + (0.5f * ts) * cmul(sd.b, cmul(sdDelta, sdDelta));
            st.d = sd.a + ts * cmul(sd.b, sdDelta);
            // w + wd delta, as the plain loop keeps it: w 2^wExp with wExp a multiple of 60 >= 0.
            st.w = float2(1.0f, 0.0f);
            st.wExp = 0;
            if (kInterior) {
                const FE wx = feAdd(FE{sd.w, sd.wExp}, feMul(FE{sd.wd, sd.wdExp}, feNorm(sdDelta, 0)));
                if (wx.e <= 0) {
                    st.w = scale2(wx.m, max(wx.e, -250));
                } else {
                    const int k = ((wx.e + 59) / 60) * 60;
                    st.w = scale2(wx.m, wx.e - k);
                    st.wExp = k;
                }
            }
            st.m = sd.m;
            st.n = sd.n;
            z = ref[st.m] + st.dz;
            r2 = dot(z, z);
            if (r2 > p.bailout2) {
                escaped = true;
            } else if (st.m != 0 && (r2 < dot(st.dz, st.dz) || st.m == last)) {
                st.dz = z;
                st.m = 0;
            }
        } else if (fresh && kJulia) {
            // F4 Julia: z_0 = the pixel, its delta to the centre orbit's start dc, d_0 = texel,
            // w_0 = 1, n = 0; tested at once (it may lie outside, or nearer 0 than the centre).
            st.dz = dc;
            st.d = tsDelta;
            st.w = float2(1.0f, 0.0f);
            st.wExp = 0;
            st.m = p.centerStart;
            st.n = 0;
            z = ref[st.m] + st.dz;
            r2 = dot(z, z);
            IC_TRACK(r2, 0u)
            if (r2 > p.bailout2) {
                escaped = true;
            } else if (r2 < dot(st.dz, st.dz)) {
                st.dz = z;
                st.m = 0;
            }
        } else if (fresh) {
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
        const float cn = (kBLA || kLA) ? length(dc) : 0.0f;
        // M6 LA: the next lookup boundary j and its index L_j.
        uint laJ = 0, laLook = 0xffffffffu, la0 = 0xffffffffu, lookedAt = 0xffffffffu;
        if (kLA) {
            la0 = laBounds[0].x;
            laJ = laNext[min(m, last)];
            laLook = laBounds[laJ].x;
        }
        // Lookups only at m = 1 (mod bla.lookupEvery): a lookup costs several exact steps
        // (dependent table loads, divergence), and a rule on m alone keeps results independent
        // of how chunks cut the orbit (shown frame = fresh render).
        const uint alignMask = kBLA ? bla.lookupEvery - 1u : 0u;
        uint budget = p.chunkIters;
        while (!escaped && !interior && n < p.maxIter && budget > 0) {
            // M6 LA (kLA): at m == L_j, the longest valid block of boundary j's chain.
            if (kLA && m == laLook && n != lookedAt) {
                lookedAt = n;
                const int k = laFind(laP, laBounds[laJ].y, laBounds[laJ + 1].y, dot(dz, dz), cn, n, p.maxIter);
                if (k >= 0) {
                    const LaEntry x = laP[k];
                    const float2 A = x.a, B = x.b;
                    const uint len = x.len;
                    budget--;
                    if (kDE) d = cmul(A, d) + B * p.texelScale;
                    if (kInterior) {
                        // |A| <= 2^40 (table build), as for BLA.
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
                            interior = true;
                            n += len;
                            break;
                        }
                    }
                    dz = cmul(A, dz) + cmul(B, dc);
                    m += len;
                    n += len;
                    laJ = x.endJ;
                    laLook = laBounds[laJ].x;
                    z = ref[m] + dz;
                    r2 = dot(z, z);
                    IC_TRACK(r2, n)
                    if (r2 > p.bailout2) { escaped = true; break; }
                    if (r2 < dot(dz, dz) || m == LAST(m)) {
                        dz = z;
                        m = 0;
                        laJ = 0;
                        laLook = la0;
                    }
                    continue;
                }
                laJ = min(laJ + 1u, la.count);
                laLook = laBounds[laJ].x;
            }
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
                    if (m + l > LAST(m) || n + l > p.maxIter) break;
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
                    if (r2 < dot(dz, dz) || m == LAST(m)) {
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
                if (kDE) d = fMulPrime(z, d) + tsAddP;
                if (kInterior) {
                    w = fMulPrime(z, w);
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
                    } else if (wExp == 0 && a * nSquared(n) < p.interiorEps) {
                        interior = true;
                        break;
                    }
                }
                dz = fStep(ref[m], dz, dz, dc);
                m++;
                n++;
                z = ref[m] + dz;
                r2 = dot(z, z);
                IC_TRACK(r2, n)
                if (r2 > p.bailout2) { escaped = true; break; }
                if (r2 < dot(dz, dz) || m == LAST(m)) {
                    dz = z;
                    m = 0;
                    if (kLA) { laJ = 0; laLook = la0; }
                }
                if (kBLA && m == 1) break;
                if (kLA && m == laLook) break;
            }
            budget -= max(n - n0, 1u);
        }
        iters += n - st.n;
        steps += p.chunkIters - min(budget, p.chunkIters);
        st.dz = dz; st.d = d; st.w = w; st.m = m; st.n = n; st.wExp = wExp;
    }

#undef LAST
    // ---- Attracting-cycle probe (kProbe, p.probe != 0; kb: techniques/interior-detection
    // "Attracting-cycle probe"). The |w| n^2 test proves only the fast-converging core of a
    // component: with multiplier lambda per period P the orbit needs |lambda|^(n/P) n^2 < eps,
    // so at the satellite (P 39, limit 1994 = 51 periods) only |lambda| < ~0.7 was proven and
    // the rest of the minibrot body and its bulbs stayed unresolved. A pixel unresolved at
    // maxIter parks (stays live, see `parked`); the scheduler runs the kProbe variant once only
    // parked pixels are left (the probe's registers would slow every main-loop pixel). It then
    // looks for the cycle itself in exact perturbation steps (no BLA; float32 in the plain
    // kernel, floatexp in the deep one) for at most p.probe steps (maxIter / 8 + 256, capped so
    // the probes add at most half of the level's work):
    //  Scan: base point z_B = the smallest |z| since the probe (re)started: the cycle's point
    //    nearest 0, where z is known to its own relative precision. A return within |z_B| / 4
    //    at k steps (k > pRej) is a candidate period P = k if z_B was the minimum for at least
    //    k steps before it too (a window of 2P around it: z_B is the near-0 point, not a point
    //    of a slow spiral that happens to come back).
    //  Evaluate (probeEval): F = f^P with g = F(z_B) - z_B, lambda = F'(z_B), F'' (both
    //    accumulated from z_B). Newton step Delta = -g / (lambda - 1). With L = 2 |F''| (a
    //    bound on |F''| near z_B), noise eta in g and h = L (|g| + eta) / |lambda - 1|^2 <= 1/4,
    //    Kantorovich puts a fixed point z* of F within r = 2 (|Delta| + eta / |lambda - 1|) of
    //    z_B, and |F'(z*)| <= |lambda| + L r. If that is <= 1 - mu, f has an attracting cycle:
    //    c is interior (a proof up to the noise bounds, not a heuristic: an exterior c has no
    //    attracting cycle). Also r <= |z_B| / 64, where F'' cannot change much.
    //  Otherwise: Newton (h <= 2 or |Delta| <= |z_B| / 4) moves the base point to z_B + Delta
    //    and iterates F again from there (8 passes per probe at most), keeping the scan's orbit
    //    point; a located but surely repelling fixed point (|lambda| - L r > 1 + 2^-10: the
    //    parent cycle inside a satellite bulb of period q P, or a ghost on an exterior orbit)
    //    resumes the scan from that point for periods > P only; one located at the noise floor
    //    but not provably attracting (near-parabolic) gives up; a hopeless candidate restarts
    //    the scan at the current point.
    // eta = 2^-22 (1 + sqrt P) (|dz| + |dz_B|, plus |Z_m| + |Z_mB| if the reference indices
    // differ): the float32 representation error of both points (rebasing keeps |dz| <= |z|),
    // with sqrt P for the rounding of the P steps. mu = 2^-20 (P + 16) covers the rounding of
    // lambda itself. (Measured margin: either bound 2^12 times smaller alone still gave no
    // false interior at the -3/4 neck and the 1/3 root; both 2^8 times smaller did.) Escaping
    // (|z| >= 2) or out of steps: unresolved, as before.
    // Component Newton (kb: techniques/interior-detection "Component Newton"), before the scan
    // when the CPU found periods of attracting cycles at sample points of the view
    // (comp.periods): near a component's edge (|lambda| ~ 0.99) the orbit takes thousands of
    // periods to settle, so the scan finds no candidate within its steps. Newton on
    // f^P(z) - z from z = 0 (the reference's Z_0 = 0: base delta 0 at index 0; the first pass
    // has lambda = 0 and steps to F(0)) needs no settled orbit: it converges in ~5 passes
    // across the component. Each pass is evaluated by probeEval as above (the same proof); a
    // pass must move by less than 2 and, from the 4th on, at least halve the step (else
    // Newton is not converging), at most SEED_PASSES passes; a surely repelling or stuck fixed
    // point or an escaping iterate fails the period. Then the next period from z = 0, and
    // after the last the scan from the pixel's orbit point (oB, oM) with its own leads.
    // comp.seedSteps extends the step limit by the passes the periods need.
    bool probing = inP, pDone = false, pProven = false;
    uint pPeriod = 0;
    PixelStateProbe ps;
    if (inP) ps = probeBuf[p.probeHalf * p.capacity + tid];
    // A probe step costs about PROBE_COST_* loop steps of the main loop: the chunk budget in those
    // units (at least one step for a pixel that starts the chunk here, so it always advances).
    const uint pCost = kDeep ? PROBE_COST_DEEP : PROBE_COST_PLAIN;
    const uint pRem = p.chunkIters > steps ? p.chunkIters - steps : 0u;
    const uint pBudget = pRem == 0u ? 0u : max(pRem / pCost, 1u);
    // (A pixel enters with a step to take: probing state always has n > maxIter.)
    if (kProbe && p.probe != 0 && active && !inB && !inP && !filled && !escaped && !interior
        && st.n == p.maxIter && pBudget > 0u) {
        if (kDeep) {
            const FE dzx = feNorm(st.dz, st.e);
            st.dz = dzx.m; st.e = dzx.e;
            st.w = float2(0.5f, 0.0f); st.wExp = 1;
        } else {
            st.w = float2(1.0f, 0.0f); st.wExp = 0;
        }
        st.d = st.dz; st.dExp = st.e;
        ps.mB = st.m;
        ps.zz = 0.0f; ps.zzExp = kDeep ? FE_ZERO : 0;
        ps.k = 0;
        ps.info = min(st.n - p.maxIter, PROBE_LOW);
        ps.pRej = 0;
        ps.seed = 0;
        if (comp.periodCount > 0u) {
            // Component Newton first: base point z = 0 (the reference's Z_0 = 0, delta 0),
            // Newton for the view's first period; the orbit point is kept for the scan.
            ps.oB = st.dz; ps.oBExp = st.e; ps.oM = st.m;
            st.dz = 0.0f; st.e = kDeep ? FE_ZERO : 0; st.m = 0;
            st.d = 0.0f; st.dExp = kDeep ? FE_ZERO : 0;
            ps.mB = 0;
            ps.info = PROBE_SEED | PROBE_NEWTON | comp.periods[0];
        }
        probing = true;
    }
    if (kProbe && probing) {
        uint m = st.m, mB = ps.mB, k = ps.k, info = ps.info;
        uint used = st.n - p.maxIter;
        const uint limit = min(p.probe + comp.seedSteps, PROBE_LOW);
        uint scan0 = ps.seed >> 3;   // the scan's leads count from its start
        uint spent = 0;
        if (kDeep) {
            // Floatexp: dz * 2^e, lambda, F'' and the base delta each with their exponent.
            const FE C = feNorm(dc, p.scaleExp);
            const FE one = FE{float2(0.5f, 0.0f), 1};
            FE dzx = FE{st.dz, st.e};
            FE lam = FE{st.w, st.wExp};
            FE fzz = FE{ps.zz, ps.zzExp};
            FE dB = FE{st.d, st.dExp};
            FE ZB = refAt(refx, mB);
            FE zB = feAdd(ZB, dB);
            while (spent < pBudget) {
                if (used >= limit) { pDone = true; break; }
                FE Zx = refAt(refx, m);
                const FE zf = feAdd(Zx, dzx);
                if (m == last || (m != 0 && feLess(zf, dzx))) {
                    dzx = zf;
                    m = 0;
                    Zx = FE{float2(0.0f), FE_ZERO};
                }
                // |z| >= 2: escapes (a Component Newton iterate: that period failed)
                bool seedNext = false;
                if (zf.e >= 3) {
                    if ((info & PROBE_SEED) == 0u) { pDone = true; break; }
                    seedNext = true;
                }
                const uint low = info & PROBE_LOW;
                uint act = PA_SCAN;
                if ((info & PROBE_NEWTON) != 0) {
                    act = k == low ? PA_EVAL : PA_SCAN;
                } else if (k > 0) {
                    const FE dv = feSub(zf, zB);
                    if (k > ps.pRej && feLess(FE{dv.m, dv.e + 2}, zB)) act = low >= k ? PA_EVAL : PA_RESET;
                    else if (feLess(zf, zB)) act = PA_RESET;
                }
                if (!seedNext && act == PA_EVAL) {
                    // g = F(z_B) - z_B, exact in Z where the reference indices agree.
                    const FE dd = feSub(dzx, dB);
                    const FE g = m == mB ? dd : feAdd(feSub(Zx, ZB), dd);
                    float lgMag = lgSum(feLog2(dzx), feLog2(dB));
                    if (m != mB) lgMag = lgSum(lgMag, lgSum(feLog2(Zx), feLog2(ZB)));
                    FE delta;
                    act = probeEval(g, lam, fzz, zB, lgMag, k, info, delta);
                    if (act == PA_PROVEN) { pProven = true; pDone = true; pPeriod = k; break; }
                    if ((info & PROBE_SEED) != 0u) {
                        // Component Newton: step on while it moves by less than 2 (the first
                        // pass from z_B = 0 has lambda = 0 and steps to F(0)); a surely
                        // repelling, stuck or non-converging one fails.
                        // From the 4th step on it must at least halve (converging Newton is
                        // quadratic by then; measured: never violated by an attracting pixel).
                        const uint passes = (info >> 26) & 15u;
                        const float lgDl = feLog2(delta);
                        const bool shrinks = passes < 3u || lgDl <= as_type<float>(ps.pRej) - 1.0f;
                        if (act != PA_STUCK && act != PA_REPEL && passes < SEED_PASSES && shrinks
                            && lgDl > -1e29f && lgDl < 1.0f) {
                            ps.pRej = as_type<uint>(lgDl);   // (pRej is free while seeding)
                            dB = feSub(dB, delta);
                            dzx = dB;
                            m = mB;
                            zB = feAdd(ZB, dB);
                            info = PROBE_SEED | PROBE_NEWTON | ((passes + 1u) << 26) | k;
                            k = 0; lam = one; fzz = FE{float2(0.0f), FE_ZERO};
                            spent++;
                            continue;
                        }
                        seedNext = true;
                    } else {
                        if (act == PA_STUCK || (act == PA_NEWTON && ((info >> 26) & 15u) >= PROBE_PASSES)) {
                            pDone = true;
                            break;
                        }
                        // (Also after a hopeless candidate: the next one is a multiple of it, F = f^(jP),
                        // evaluated once the orbit has had a few more periods to converge.)
                        if (act == PA_REPEL || act == PA_REJECT) ps.pRej = max(ps.pRej, k);
                        if (act == PA_NEWTON) {
                            // z_B <- z_B + Delta (delta = -Delta), iterate F again from there. (The
                            // scan's orbit point is kept to resume from.)
                            if ((info & PROBE_NEWTON) == 0) { ps.oB = dzx.m; ps.oBExp = dzx.e; ps.oM = m; }
                            dB = feSub(dB, delta);
                            dzx = dB;
                            m = mB;
                            zB = feAdd(ZB, dB);
                            info = PROBE_NEWTON | ((((info >> 26) & 15u) + 1u) << 26) | k;
                            k = 0; lam = one; fzz = FE{float2(0.0f), FE_ZERO};
                            spent++;
                            continue;
                        }
                        if ((info & PROBE_NEWTON) != 0) {
                            // Newton failed (e.g. converged to a repelling cycle: the parent's
                            // fixed point inside a satellite bulb of period q P): resume the scan
                            // from its orbit point, for periods longer than P only.
                            ps.pRej = max(ps.pRej, k);
                            dzx = FE{ps.oB, ps.oBExp};
                            m = ps.oM;
                            dB = dzx; mB = m; ZB = refAt(refx, m); zB = feAdd(ZB, dB);
                            k = 0; lam = one; fzz = FE{float2(0.0f), FE_ZERO};
                            info = (info & (15u << 26)) | (used - scan0);
                            spent++;
                            continue;
                        }
                    }
                }
                if (seedNext) {
                    // This period failed: the next one from z = 0, else the scan from the orbit
                    // point (its leads count from here).
                    const uint j = (ps.seed & 7u) + 1u;
                    if (j < comp.periodCount) {
                        ps.seed = j;
                        dB = FE{float2(0.0f), FE_ZERO}; mB = 0;
                        info = PROBE_SEED | PROBE_NEWTON | comp.periods[j];
                    } else {
                        ps.seed = used << 3;
                        ps.pRej = 0;
                        scan0 = used;
                        dB = FE{ps.oB, ps.oBExp}; mB = ps.oM;
                        info = 0u;
                    }
                    dzx = dB; m = mB; ZB = refAt(refx, mB); zB = feAdd(ZB, dB);
                    k = 0; lam = one; fzz = FE{float2(0.0f), FE_ZERO};
                    spent++;
                    continue;
                }
                if (act != PA_SCAN) {
                    // New base point here (a new minimum, a too early return, a hopeless candidate).
                    dB = dzx; mB = m; ZB = Zx; zB = feAdd(ZB, dB);
                    k = 0; lam = one; fzz = FE{float2(0.0f), FE_ZERO};
                    info = (info & (15u << 26)) | (used - scan0);
                }
                // F'' first (old lambda), then lambda, then the step.
                const FE z2 = fPrimeXT(zf);
                fzz = fProbeF2X(zf, z2, fzz, lam);
                lam = feMul(z2, lam);
                dzx = fStepXT(Zx, dzx, C);
                m++;
                k++;
                used++;
                spent++;
            }
            if (pProven) {
                st.m = mB; st.dz = dB.m; st.e = dB.e;
            } else {
                st.dz = dzx.m; st.e = dzx.e; st.m = m;
            }
            st.w = lam.m; st.wExp = lam.e;
            st.d = dB.m; st.dExp = dB.e;
            ps.zz = fzz.m; ps.zzExp = fzz.e;
        } else {
            // Float32 (plain kernel: deltas far above float32's floor), as the main loop's step;
            // lambda = l 2^le and F'' = f 2^fe keep exponents (the product of 2z over P steps
            // leaves the float range).
            float2 dz = st.dz, dB = st.d, l = st.w, f = ps.zz;
            int le = st.wExp, fe = ps.zzExp;
            float2 ZB = ref[mB];
            float2 zB = ZB + dB;
            float zB2 = dot(zB, zB);
            while (spent < pBudget) {
                if (used >= limit) { pDone = true; break; }
                float2 Z = ref[m];
                const float2 z = Z + dz;
                const float r2 = dot(z, z);
                if (r2 < dot(dz, dz) || m == last) {
                    dz = z;
                    m = 0;
                    Z = 0.0f;
                }
                // |z| >= 2: escapes (a Component Newton iterate: that period failed)
                bool seedNext = false;
                if (!(r2 < 4.0f)) {
                    if ((info & PROBE_SEED) == 0u) { pDone = true; break; }
                    seedNext = true;
                }
                const uint low = info & PROBE_LOW;
                uint act = PA_SCAN;
                if ((info & PROBE_NEWTON) != 0) {
                    act = k == low ? PA_EVAL : PA_SCAN;
                } else if (k > 0) {
                    const float2 dv = z - zB;
                    if (k > ps.pRej && 16.0f * dot(dv, dv) < zB2) act = low >= k ? PA_EVAL : PA_RESET;
                    else if (r2 < zB2) act = PA_RESET;
                }
                if (!seedNext && act == PA_EVAL) {
                    const float2 dd = dz - dB;
                    const float2 g = m == mB ? dd : (Z - ZB) + dd;
                    float mag = length(dz) + length(dB);
                    if (m != mB) mag += length(Z) + length(ZB);
                    FE delta;
                    act = probeEval(feNorm(g, 0), feNorm(l, le), feNorm(f, fe), feNorm(zB, 0),
                                    log2(max(mag, 1e-37f)), k, info, delta);
                    if (act == PA_PROVEN) { pProven = true; pDone = true; pPeriod = k; break; }
                    if ((info & PROBE_SEED) != 0u) {
                        // Component Newton (see the deep loop).
                        const uint passes = (info >> 26) & 15u;
                        const float lgDl = feLog2(delta);
                        const bool shrinks = passes < 3u || lgDl <= as_type<float>(ps.pRej) - 1.0f;
                        if (act != PA_STUCK && act != PA_REPEL && passes < SEED_PASSES && shrinks
                            && lgDl > -1e29f && lgDl < 1.0f) {
                            ps.pRej = as_type<uint>(lgDl);
                            dB -= feFloat(delta);
                            dz = dB;
                            m = mB;
                            zB = ZB + dB;
                            zB2 = dot(zB, zB);
                            l = float2(1.0f, 0.0f); le = 0; f = 0.0f; fe = 0;
                            info = PROBE_SEED | PROBE_NEWTON | ((passes + 1u) << 26) | k;
                            k = 0;
                            spent++;
                            continue;
                        }
                        seedNext = true;
                    } else {
                        if (act == PA_STUCK || (act == PA_NEWTON && ((info >> 26) & 15u) >= PROBE_PASSES)) {
                            pDone = true;
                            break;
                        }
                        // (Also after a hopeless candidate: the next one is a multiple of it, F = f^(jP),
                        // evaluated once the orbit has had a few more periods to converge.)
                        if (act == PA_REPEL || act == PA_REJECT) ps.pRej = max(ps.pRej, k);
                        if (act == PA_NEWTON) {
                            if ((info & PROBE_NEWTON) == 0) { ps.oB = dz; ps.oBExp = 0; ps.oM = m; }
                            dB -= feFloat(delta);
                            dz = dB;
                            m = mB;
                            zB = ZB + dB;
                            zB2 = dot(zB, zB);
                            l = float2(1.0f, 0.0f); le = 0; f = 0.0f; fe = 0;
                            info = PROBE_NEWTON | ((((info >> 26) & 15u) + 1u) << 26) | k;
                            k = 0;
                            spent++;
                            continue;
                        }
                        if ((info & PROBE_NEWTON) != 0) {
                            // Newton failed: resume the scan (see the deep loop).
                            ps.pRej = max(ps.pRej, k);
                            dz = ps.oB;
                            m = ps.oM;
                            dB = dz; mB = m; ZB = ref[m]; zB = ZB + dB; zB2 = dot(zB, zB);
                            k = 0; l = float2(1.0f, 0.0f); le = 0; f = 0.0f; fe = 0;
                            info = (info & (15u << 26)) | (used - scan0);
                            spent++;
                            continue;
                        }
                    }
                }
                if (seedNext) {
                    // This period failed: the next one from z = 0, else the scan from the orbit
                    // point (see the deep loop).
                    const uint j = (ps.seed & 7u) + 1u;
                    if (j < comp.periodCount) {
                        ps.seed = j;
                        dB = 0.0f; mB = 0;
                        info = PROBE_SEED | PROBE_NEWTON | comp.periods[j];
                    } else {
                        ps.seed = used << 3;
                        ps.pRej = 0;
                        scan0 = used;
                        dB = ps.oB; mB = ps.oM;
                        info = 0u;
                    }
                    dz = dB; m = mB; ZB = ref[mB]; zB = ZB + dB; zB2 = dot(zB, zB);
                    k = 0; l = float2(1.0f, 0.0f); le = 0; f = 0.0f; fe = 0;
                    spent++;
                    continue;
                }
                if (act != PA_SCAN) {
                    dB = dz; mB = m; ZB = Z; zB = z; zB2 = r2;
                    k = 0; l = float2(1.0f, 0.0f); le = 0; f = 0.0f; fe = 0;
                    info = (info & (15u << 26)) | (used - scan0);
                }
                // F'' first (old lambda; f aligned to l's exponent), then lambda, then the step.
                const float2 z2 = fPrime(z);
                if (f.x == 0.0f && f.y == 0.0f) fe = 2 * le;
                int sh = 2 * le - fe;
                if (sh > 100) { f = 0.0f; fe = 2 * le; sh = 0; }
                f = fProbeF2(z, z2, f, l, pow2f(max(sh, -127)));
                l = cmul(z2, l);
                const float al = max(abs(l.x), abs(l.y));
                if (al > 0x1p30f || (al < 0x1p-30f && al > 0.0f)) { const int t = expOf(al); l = scale2(l, -t); le += t; }
                const float af = max(abs(f.x), abs(f.y));
                if (af > 0x1p30f || (af < 0x1p-30f && af > 0.0f)) { const int t = expOf(af); f = scale2(f, -t); fe += t; }
                dz = fStep(Z, dz, dz, dc);
                m++;
                k++;
                used++;
                spent++;
            }
            if (pProven) {
                st.m = mB; st.dz = dB;
            } else {
                st.dz = dz; st.m = m;
            }
            st.e = 0;
            st.w = l; st.wExp = le;
            st.d = dB; st.dExp = 0;
            ps.zz = f; ps.zzExp = fe;
        }
        iters += spent;
        steps += spent;   // (one loop step per probe step: steps == iterations without BLA)
        if (pProven) {
            // Continue from the cycle point (the interior colouring's analysis starts here,
            // with the period as its search cap).
            interior = true;
            icQ = pPeriod;
        }
        ps.mB = mB; ps.k = k; ps.info = info;
        st.n = p.maxIter + used;
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
    if (kInteriorColor && active && !inB && !escaped && compPeriod == 0u
        && (interior || (!kInterior && st.n >= p.maxIter))) {
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
            const FE z2 = fPrimeXT(zf);
            // Old values on the right (derivatives first, then z).
            cz = fIcCZ(zf, z2, cz, dcx, lam);
            zz = fIcZZ(zf, z2, zz, lam);
            dcx = feAdd(feMul(z2, dcx), TS);
            lam = feMul(z2, lam);
            dzx = fStepXT(Zx, dzx, C);
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

    // (Parked: unresolved at maxIter with the probe on, waiting for a kProbe chunk.)
    bool alive = active && (inB ? !bDone : probing ? !pDone
        : (!escaped && !interior && (st.n < p.maxIter || (kInterior && p.probe != 0))));
    // Reached the limit neither escaped nor proven interior: unresolved, never drawn as
    // interior (with the test on), and collected for an extension pass.
    bool crossed = active && !inB && !alive && !escaped && !interior;
    if (kInteriorColor && compPeriod != 0u) {
        // Proven by the component test: its period, multiplier and distance in closed form.
        float2 lam;
        float minLog2, dist;
        compColor(comp, compPeriod, float2(pix.x, -pix.y), lam, minLog2, dist);
        iterOut.write(float4(ITER_INTERIOR, float(compPeriod), 0.0f, 0.0f), gid);
        deOut.write(float4(length(lam), atan2(lam.y, lam.x) * 0.15915494f, 0.0f, 0.0f), gid);
        auxOut.write(float4(dist, minLog2, 0.0f, 0.0f), gid);
    } else if (kInteriorColor && active && !alive && inB) {
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
    // M5 interior fill: a border pixel is good when it is proven interior within fillProof
    // iterations (a function of its orbit alone); one that finished otherwise, or is still
    // running at fillProof or beyond (it can only finish later), spoils its tile. Those still
    // below it are counted: the insides wait for them (prepareChunk).
    if (p.fillBorder != 0) {
        const bool ring = active && !inB && onRing(gid);
        if (ring && (alive ? st.n >= p.fillProof : !(interior && st.n < p.fillProof))) {
            atomic_store_explicit(&tileBad[tileOf], p.fillEpoch, memory_order_relaxed);
        }
        const uint und = simd_sum((ring && alive && st.n < p.fillProof) ? 1u : 0u);
        if (lane == 0 && und != 0) atomic_fetch_add_explicit(&levelAtomic[3], und, memory_order_relaxed);
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
    threadgroup uint sgMinN[32];
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
    // The youngest live pixel's n: how far the slowest pixels have got (render progress).
    uint simdMinN = simd_min(alive ? st.n : 0xffffffffu);
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
        sgMinN[sg] = simdMinN;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tidInGroup == 0) {
        uint sum = 0, itSum = 0, stepSum = 0, intSum = 0, crossSum = 0, escMax = 0, minN = 0xffffffffu;
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
            minN = min(minN, sgMinN[i]);
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
        if (minN != 0xffffffffu) atomic_fetch_min_explicit(&stats[p.statSlot].liveMinN, minN, memory_order_relaxed);
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
        const uint pOut = (1u - p.probeHalf) * p.capacity + tgBase + sgLive[sg] + offset;
        if (kProbe && probing) probeBuf[pOut] = ps;
        else if (carryP) probeBuf[pOut] = probeBuf[p.probeHalf * p.capacity + tid];
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
    uint   fillBorder;    // M5: the main level's admission order has tile borders first
    uint   fogLevels;     // fog over unresolved pixels: levels of the fog pyramid (0: none, the
                          // plain deep fog; kb: techniques/unresolved-fog)
    float  fogPoints;     // display points per iteration texel (fog widths in points)
    uint   pad3;
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
// `sampleWidth`: the width of the area this sample stands for, in output pixels. A pixel made
// of N samples (the supersampling grid, the progressive AA mean) filters each sample over its
// share of the pixel (1/sqrt(N)), not the whole pixel: filtering every sample over the full
// pixel and then averaging them filters twice, and fine rings lost their colour to the
// palette's mean grey after AA (chroma 86 % of a 256-sample reference at dense-5e-14; 97 %
// now; kb: techniques/anti-aliasing).
static inline float3 paletteColor(float2 it, float deTexels, constant ColorParams& c, device const float4* pal,
                                  float pixelsPerTexel, thread float& dense, thread float& calm,
                                  float sampleWidth) {
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
    float lod = clamp(log2(max(footprint * sampleWidth * float(c.paletteSize), 1.0f)), 0.0f,
                      float(c.paletteLevels - 1));
    const bool filtered = c.paletteFilter != 0.0f && deTexels < 999.0f;     // a filter and a DE
    if (!filtered) lod = 0.0f;
    // (dense and calm are looks of the pixel, judged at the pixel's footprint.)
    dense = filtered ? saturate((min(rawLod, float(c.paletteLevels - 1)) - 7.0f) / 3.0f) : 0.0f;
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

// Fog over unresolved pixels (kb: techniques/unresolved-fog): past the nominal limit, neither
// escaped nor proven interior, so compute further may still resolve them. Not the interior
// colour (user decision 2026-10-17: near the -3/4 neck almost the whole view was unresolved and
// read as interior) and not a flat patch either (the grey veil it replaced, user 2026-10-17):
// fog over the fractal, light where it meets resolved pixels and taking their colours there,
// darker and softer further in (fogResolve, from the fog pyramid), down to this deep fog:
// almost black with a faint cool blue-grey tint, so it still reads as "not computed" next to a
// black interior. Previews and gesture frames (no pyramid) draw it plain.
constant float3 FOG_DEEP = float3(0.0065f, 0.0080f, 0.0125f);   // linear
static inline float3 fogDeep() {
    return FOG_DEEP;
}

static inline float3 shade(float2 it, float2 de, float2 ax, constant ColorParams& c, device const float4* pal,
                           float pixelsPerTexel, float sampleWidth = 1.0f) {
    // Unresolved: the deep fog. The callers that have the fog pyramid draw fogAt() instead
    // (light at resolved neighbours); previews and gesture frames draw this.
    if (it.x == ITER_UNRESOLVED) return fogDeep();
    if (it.x < 0.0f) return interiorShade(it, de, ax, c, pal, pixelsPerTexel);
    float dense, calm;
    float3 col = paletteColor(it, de.x, c, pal, pixelsPerTexel, dense, calm, sampleWidth);
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
    if (tileIndex(s, c.tilesX, c.tileOrder, rank, c.fillBorder) >= admitted) return false;
    it = t.read(s).xy;
    return it.x != ITER_PENDING;
}

// The cycle analysis of an interior texel (only read when an interior mode uses it: without
// the analysis a placeholder texture is bound).
static inline float2 auxAt(texture2d<float, access::read> aux, uint2 s, constant ColorParams& c, float2 it) {
    return (c.interiorMode != 0 && it.x == ITER_INTERIOR && it.y >= 1.0f) ? aux.read(s).xy : float2(0.0f);
}

// ---------------------------------------------------------------------------------------------
// Fog pyramid (kb: techniques/unresolved-fog): how far an unresolved texel is from resolved ones,
// in display points, and the colours around it at that distance, for any size of unresolved
// region at the cost of a mip chain. Level 0 has a texel per 2 x 2 iteration texels: whether a
// sampled one is resolved (a) and its colour times a (premultiplied); each further level halves
// (box mean of 2 x 2), so a becomes the resolved share around. Even levels live in atlas A (level 0 at the origin, 2, 4, ...
// stacked to its right), odd levels in atlas B (1 at the origin, 3, 5, ... to its right), so a
// pass never reads and writes one texture. Built only for frames with unresolved pixels.
// The fog colour is resolved at pyramid level FOG_RES (a texel per 4 x 4 iteration texels): the
// fog is soft anyway, and at level 0 the walk cost 0.6 ms at 3456 x 2234 on M1 Pro.
constant uint FOG_RES = 1u;

struct FogParams {
    uint2 src;            // iteration level size
    uint  level;          // the level fogDown writes
    uint  pad;
};

// Size of pyramid level L of an iteration level of size `src`.
static inline uint2 fogLevelSize(uint2 src, uint L) {
    uint2 s = (src + 1u) / 2u;
    for (uint k = 0; k < L; k++) s = (s + 1u) / 2u;
    return max(s, uint2(1u));
}

// Origin of level L in its atlas.
static inline uint2 fogLevelOrigin(uint2 src, uint L) {
    if (L < 2u) return uint2(0u);
    uint2 s = fogLevelSize(src, L & 1u);
    const uint x = s.x;
    uint y = 0u;
    for (uint k = (L & 1u) + 2u; k < L; k += 2u) y += fogLevelSize(src, k).y;
    return uint2(x, y);
}

kernel void fogSeed(texture2d<float, access::read>  iters [[texture(0)]],
                    texture2d<float, access::read>  des   [[texture(1)]],
                    texture2d<float, access::read>  auxs  [[texture(2)]],
                    texture2d<float, access::write> atlas [[texture(3)]],
                    constant ColorParams& c               [[buffer(0)]],
                    device const float4* pal              [[buffer(1)]],
                    device const LevelCounters& level     [[buffer(2)]],
                    device const uint* rank               [[buffer(3)]],
                    uint2 gid                             [[thread_position_in_grid]])
{
    const uint2 size0 = fogLevelSize(c.srcSize, 0u);
    if (gid.x >= size0.x || gid.y >= size0.y) return;
    const uint admitted = c.admittedOnGPU != 0 ? level.admitted : c.admitted;
    // One texel per block, its position in the block cycling over 2 x 2 blocks (an ordered
    // pattern, so level 1 sees all four): the pyramid wants the colours around, blurred. The
    // pass is bound by memory (whole cache lines): all four texels cost 0.6 ms at 3456 x 2234
    // on M1 Pro, one 0.45 ms.
    const uint k = (gid.x & 1u) | ((gid.y & 1u) << 1);
    const uint2 s = min(gid * 2u + uint2(k >> 1, (k ^ (k >> 1)) & 1u), c.srcSize - 1u);
    float2 it;
    const bool resolved = finished(iters, s, c, rank, admitted, it) && it.x != ITER_UNRESOLVED
        && it.x != ITER_SKIP;
    const float3 col = resolved ? shade(it, des.read(s).xy, auxAt(auxs, s, c, it), c, pal, c.deScale) : float3(0.0f);
    const float a = resolved ? 1.0f : 0.0f;
    atlas.write(float4(col * a, a), gid);
}

kernel void fogDown(texture2d<float, access::read>  from [[texture(0)]],
                    texture2d<float, access::write> to   [[texture(1)]],
                    constant FogParams& p                [[buffer(0)]],
                    uint2 gid                            [[thread_position_in_grid]])
{
    const uint2 size = fogLevelSize(p.src, p.level);
    if (gid.x >= size.x || gid.y >= size.y) return;
    const uint2 fs = fogLevelSize(p.src, p.level - 1u);
    const uint2 fo = fogLevelOrigin(p.src, p.level - 1u);
    float4 sum = float4(0.0f);
    for (uint k = 0; k < 4u; k++) {
        sum += from.read(fo + min(gid * 2u + uint2(k & 1u, k >> 1), fs - 1u));
    }
    to.write(0.25f * sum, fogLevelOrigin(p.src, p.level) + gid);
}

// The fog of each level-FOG_RES texel (kb: techniques/unresolved-fog), into `fog` (that level's
// size; the colour passes sample it bilinearly at their unresolved texels). Walks the pyramid from fine to
// coarse: s_L = how surely level L's footprint holds resolved texels (saturated resolved share,
// bilinear); the expected first level with resolved texels is the distance in octaves, the
// colour the mean of each level's resolved colour weighted by that level being the first.
// Distance in points makes the look independent of zoom and display scale.
kernel void fogResolve(texture2d<float, access::sample> fa  [[texture(0)]],
                       texture2d<float, access::sample> fb  [[texture(1)]],
                       texture2d<float, access::write>  fog [[texture(2)]],
                       constant ColorParams& c              [[buffer(0)]],
                       uint2 gid                            [[thread_position_in_grid]])
{
    const uint2 s0 = fogLevelSize(c.srcSize, 0u), s1 = fogLevelSize(c.srcSize, 1u);
    const uint2 sr = fogLevelSize(c.srcSize, FOG_RES);
    if (gid.x >= sr.x || gid.y >= sr.y) return;
    constexpr sampler bl(coord::normalized, filter::linear, address::clamp_to_edge);
    const float3 deep = fogDeep();
    const float2 p0 = (float2(gid) + 0.5f) * float(1u << FOG_RES);   // level-0 coordinates
    const float2 inva = 1.0f / float2(fa.get_width(), fa.get_height());
    const float2 invb = 1.0f / float2(fb.get_width(), fb.get_height());
    // Deep inside a region (nothing resolved within level 4's bilinear footprint, +-32 texels)
    // the finer levels hold nothing either: start at level 5.
    uint first = 0u;
    if (c.fogLevels > 5u) {
        const uint2 o4 = fogLevelOrigin(c.srcSize, 4u);
        const float2 q = clamp(p0 * 0.0625f, float2(0.5f), float2(fogLevelSize(c.srcSize, 4u)) - 0.5f) + float2(o4);
        if (fa.sample(bl, q * inva, level(0)).a < 1e-4f) first = 5u;
    }
    uint2 size = first == 0u ? s0 : fogLevelSize(c.srcSize, first);
    float yA = 0.0f, yB = 0.0f;                       // stacking offsets of levels >= 2
    for (uint L = 2u; L < first; L++) {
        if ((L & 1u) != 0u) yB += float(fogLevelSize(c.srcSize, L).y); else yA += float(fogLevelSize(c.srcSize, L).y);
    }
    float survive = 1.0f, octaves = float(first);
    float3 border = float3(0.0f);
    float scale = 1.0f / float(1u << first);
    for (uint L = first; L < c.fogLevels && survive > 1e-3f; L++) {
        const bool odd = (L & 1u) != 0u;
        float2 origin = float2(0.0f);
        if (L >= 2u) origin = odd ? float2(float(s1.x), yB) : float2(float(s0.x), yA);
        const float2 q = clamp(p0 * scale, float2(0.5f), float2(size) - 0.5f) + origin;
        const float4 v = odd ? fb.sample(bl, q * invb, level(0)) : fa.sample(bl, q * inva, level(0));
        const float found = saturate(3.0f * v.a);
        if (v.a > 1e-6f) border += survive * found * v.rgb / v.a;
        survive *= 1.0f - found;
        octaves += survive;
        if (L >= 2u) {
            if (odd) yB += float(size.y); else yA += float(size.y);
        }
        size = max((size + 1u) / 2u, uint2(1u));
        scale *= 0.5f;
    }
    border += survive * deep;
    // Distance from the resolved pixels in points (log2): a level-L texel is 2^(L+1) iteration
    // texels wide.
    const float lg = octaves + 1.0f + log2(max(c.fogPoints, 1e-6f));
    // Light at the border, darker over the first ~100 points; deep inside, the colours around
    // (the coarse levels: very blurred) still tint it faintly, like light in a cloud.
    const float t = pow(saturate(lg / 7.0f), 0.7f);
    fog.write(float4(mix(border, deep + 0.02f * border, 0.2f + 0.8f * t), 1.0f), gid);
}

// The fog at unresolved texel s: the resolved fog texture (bilinear), or the deep fog without it.
static inline float3 fogAt(uint2 s, constant ColorParams& c, texture2d<float, access::sample> fog) {
    if (c.fogLevels == 0u) return fogDeep();
    constexpr sampler bl(coord::normalized, filter::linear, address::clamp_to_edge);
    const float2 p = (float2(s) + 0.5f) / float(2u << FOG_RES) / float2(fog.get_width(), fog.get_height());
    return fog.sample(bl, p, level(0)).rgb;
}

kernel void colorize(texture2d<float, access::read>  iters      [[texture(0)]],
                     texture2d<float, access::read>  des        [[texture(1)]],
                     texture2d<float, access::write> out        [[texture(2)]],
                     texture2d<float, access::sample> under     [[texture(3)]],
                     texture2d<float, access::read>  auxs       [[texture(4)]],
                     texture2d<float, access::sample> fog       [[texture(5)]],   // fogResolve's
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
                    acc += it.x == ITER_UNRESOLVED ? fogAt(s, c, fog)
                        : shade(it, des.read(s).xy, auxAt(auxs, s, c, it), c, pal, c.deScale, 1.0f / float(ss));
                    count += 1.0f;
                }
            }
        }
    } else {
        uint2 s = min(uint2(float2(gid) * c.ratio), maxSrc);
        if (finished(iters, s, c, rank, admitted, it)) {
            acc = it.x == ITER_UNRESOLVED ? fogAt(s, c, fog)
                : shade(it, des.read(s).xy, auxAt(auxs, s, c, it), c, pal, c.deScale);
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
    float  sampleWidth;   // mode 2: the palette filter's width of an AA sample, output pixels
                          // (1 / sqrt(samples): its share of the pixel); else unused
    uint   pad;
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
                    device const uint* furtherMask            [[buffer(5)]],   // mode 4
                    texture2d<float, access::sample> fog      [[texture(7)]],   // fogResolve's
                    uint2 gid                                 [[thread_position_in_grid]])
{
    if (gid.x >= c.outSize.x || gid.y >= c.outSize.y) return;
    constexpr sampler bilinear(coord::normalized, filter::linear, address::clamp_to_edge);
    const float2 pc = float2(gid) + 0.5f;
    // New-sample level: texel coordinate of this pixel's centre (texel t's sample sits at t + jitter).
    const float2 g = pc - q.outCentre;
    const float2 v = g * q.newRatio + q.newCentre;

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
            col = use ? mix(hist, shade(it, des.read(gid).xy, auxAt(auxs, gid, c, it), c, pal, 1.0f, q.sampleWidth), q.blend) : hist;
            conf = q.confCap;
        } else if (q.mode == 4) {
            // Compute further after AA: a texel unresolved when its passes started fades to its
            // new colour once resolved, or to the fog as it is now (the region shrinks, so its
            // border moves); every other pixel keeps its (anti-aliased) history.
            const uint i = gid.y * c.outSize.x + gid.x;
            const bool marked = ((furtherMask[i >> 5] >> (i & 31u)) & 1u) != 0u;
            col = (marked && done)
                ? mix(hist, it.x == ITER_UNRESOLVED ? fogAt(gid, c, fog)
                      : shade(it, des.read(gid).xy, auxAt(auxs, gid, c, it), c, pal, 1.0f), q.blend)
                : hist;
            conf = q.confCap;
        } else if (done) {
            float3 exact = it.x == ITER_UNRESOLVED ? fogAt(gid, c, fog)
                : shade(it, des.read(gid).xy, auxAt(auxs, gid, c, it), c, pal, 1.0f);
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

// "Compute further" (kb: techniques/auto-iterations "Compute further"): lists the texels still
// unresolved among admission indices [start, start + count) of a full level (tiles in its priority
// order, Morton order inside a tile), for an extension pass over that segment. Segments are at
// most `capacity` slots, so every unresolved texel of one fits: the passes no longer depend on the
// level's own all-or-nothing list (9.7 M unresolved at 4000 x 2500 near the -3/4 neck overflowed
// the 4 M entries and got no pass at all).
// The texels unresolved when compute further's passes start after AA, one bit each (row-major):
// its compose (mode 4) recolours only those, so anti-aliased pixels keep their samples.
kernel void unresolvedMask(texture2d<float, access::read> iters [[texture(0)]],
                           constant uint2& size                 [[buffer(0)]],
                           device uint* mask                    [[buffer(1)]],
                           uint tid                             [[thread_position_in_grid]])
{
    const uint n = size.x * size.y, base = tid * 32u;
    if (base >= n) return;
    uint bits = 0u;
    for (uint k = 0u; k < 32u && base + k < n; k++) {
        const uint i = base + k;
        if (iters.read(uint2(i % size.x, i / size.x)).x == ITER_UNRESOLVED) bits |= 1u << k;
    }
    mask[tid] = bits;
}

struct UnresolvedListParams {
    uint width;
    uint height;
    uint tilesX;
    uint tileOrder;
    uint start;          // first admission index of the segment
    uint count;          // admission indices in the segment
    uint capacity;       // list entries
    uint pad;
};

kernel void unresolvedList(texture2d<float, access::read> iters [[texture(0)]],
                           constant UnresolvedListParams& p     [[buffer(0)]],
                           device const uint* order             [[buffer(1)]],
                           device atomic_uint* count            [[buffer(2)]],
                           device uint* list                    [[buffer(3)]],
                           uint tid                             [[thread_position_in_grid]],
                           uint lane                            [[thread_index_in_simdgroup]])
{
    uint2 xy = uint2(0);
    bool want = tid < p.count && tileTexel(p.start + tid, p.tilesX, p.width, p.height, p.tileOrder, order, xy)
        && iters.read(xy).x == ITER_UNRESOLVED;
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
    float4 param;   // segment: half width; rect: corner radius, border width; glyph: atlas cell x0, y0, x1, y1; image: opacity, corner radius; triangle: p2
    uint4  info;    // kind (0 segment, 1 disc, 2 ring, 3 rect, 4 image, 5 glyph, 6 glow image, 7 triangle: p0, p1 in geom), map index | glyph: atlas width, halo width, spread (float bits)
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

// Signed distance to triangle p0 p1 p2 (negative inside; either winding).
static inline float triangleDistance(float2 p, float2 p0, float2 p1, float2 p2) {
    float2 e0 = p1 - p0, e1 = p2 - p1, e2 = p0 - p2;
    float2 v0 = p - p0, v1 = p - p1, v2 = p - p2;
    float2 q0 = v0 - e0 * saturate(dot(v0, e0) / max(dot(e0, e0), 1e-6f));
    float2 q1 = v1 - e1 * saturate(dot(v1, e1) / max(dot(e1, e1), 1e-6f));
    float2 q2 = v2 - e2 * saturate(dot(v2, e2) / max(dot(e2, e2), 1e-6f));
    float s = sign(e0.x * e2.y - e0.y * e2.x);
    float2 d = min(min(float2(dot(q0, q0), s * (v0.x * e0.y - v0.y * e0.x)),
                       float2(dot(q1, q1), s * (v1.x * e1.y - v1.y * e1.x))),
                   float2(dot(q2, q2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
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
                    device const uchar4* trail              [[buffer(6)]],   // π meter orbit trail (BGRA8)
                    device const uchar4* cloud              [[buffer(7)]],   // orbit dot cloud (BGRA8, alpha)
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
            case 6: {   // glow image (the π meter's orbit trail): sRGB BGRA8 pixels in a buffer,
                        // info.y x info.z, stretched over the rect and added to what is under it
                        // (black adds nothing). A buffer, not a texture: the CPU copy into a
                        // texture's tiled layout cost 0.26 ms per picture.
                // info.w 1: the orbit's dot cloud, straight sRGB colour with alpha, blended over.
                if (any(p < pr.geom.xy) || any(p >= pr.geom.zw)) continue;
                float2 ts = float2(pr.info.y, pr.info.z);
                uint2 t = uint2(min((p - pr.geom.xy) / (pr.geom.zw - pr.geom.xy) * ts, ts - 1.0f));
                if (pr.info.w == 1) {
                    float4 v = float4(cloud[t.y * pr.info.y + t.x]) / 255.0f;
                    float a = v.w * pr.param.x;
                    col = srgbToLinear(v.zyx) * a + col * (1.0f - a);
                    continue;
                }
                float4 v = float4(trail[t.y * pr.info.y + t.x]) / 255.0f;
                col += srgbToLinear(v.zyx) * pr.param.x;
                continue;
            }
            case 7:     // triangle
                cov = saturate(0.5f - triangleDistance(p, pr.geom.xy, pr.geom.zw, pr.param.xy));
                break;
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
/// crossed per view width), and the escaped sample with the highest escape count with its
/// texel (the cell's point nearest the set's boundary: survey steering aims there). Two
/// uint4 per cell: (samples, interior, pairs, edges), (peak x, peak y, peak n bits, 0). One
/// thread per cell, at most 16 x 16 samples each.
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
    uint2 peak = uint2(x0, y0);
    float peakN = -1.0f;
    for (uint y = y0; y < y1; y += step) {
        for (uint x = x0; x < x1; x += step) {
            float n = iters.read(uint2(x, y)).x;
            if (n == ITER_PENDING || n == ITER_SKIP) continue;
            samples++;
            bool in = n < 0.0f;
            if (in) interior++;
            else if (n > peakN) { peakN = n; peak = uint2(x, y); }
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
    uint i = gid.y * p.cols + gid.x;
    cells[2 * i] = uint4(samples, interior, pairs, edges);
    cells[2 * i + 1] = uint4(peak.x, peak.y, as_type<uint>(peakN), 0u);
}
