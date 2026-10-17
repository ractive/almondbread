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
constant bool kAT       [[function_constant(9)]];   // M6: the approximation table of a periodic
                                                    // reference (needs kLA)

// Iteration-texture codes in .x besides n >= 0.
constant float ITER_INTERIOR   = -1.0f;   // .y = 1: derivative test, 0: reached maxIter without the test
constant float ITER_PENDING    = -2.0f;   // admitted but still iterating
constant float ITER_UNRESOLVED = -3.0f;   // past the nominal limit, neither escaped nor proven
                                          // interior (.y = the nominal limit); may still resolve
constant float ITER_SKIP       = -4.0f;   // AA sample textures: this pixel takes no AA samples
constant float ITER_EMPTY      = -5.0f;   // sample cache: no sample here (yet)

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
    uint   wrap;          // toroidal levels (sample cache, kb: techniques/sample-reuse): texel gid
                          // holds window texel (gid + wrap) mod size, wrap = x | y << 16; 0: none
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
    uint   pRej;          // candidates must be longer than this rejected period
    uint   pad;
};
constant uint PROBE_NEWTON = 0x40000000u;
constant uint PROBE_LOW    = 0x03ffffffu;   // lead / period (26 bits)
constant uint PROBE_PASS   = 0x04000000u;   // Newton passes (bits 26..29)
constant uint PROBE_COST_PLAIN = 4u;        // main-loop steps per probe step (chunk budget):
constant uint PROBE_COST_DEEP  = 16u;       // float32 in the plain kernel, floatexp in the deep one
// Probe step actions.
constant uint PA_SCAN = 0u, PA_EVAL = 1u, PA_RESET = 2u, PA_PROVEN = 3u, PA_NEWTON = 4u, PA_REJECT = 5u,
              PA_STUCK = 6u, PA_REPEL = 7u;
constant uint PROBE_PASSES = 8u;            // Newton passes per probe at most

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
// AT (kAT): for a periodic reference of period P, y = A z at m = 0 iterates y <- y^2 + c',
// c' = CC dc + RC, while |y|^2 < |A|^2 (R - (K + 1) |dc|): P iterations per step.
struct LaParams {
    uint   count;
    uint   period;        // AT period (0: none)
    uint   pad0;
    uint   pad1;
    float2 atA;   int atAe;  int pad2;
    float2 atAi;  int atAie; int pad3;    // 1 / A
    float2 atCC;  int atCCe; int pad4;    // A (A + B)
    float2 atRC;  int atRCe; int pad5;    // A Z_P
    float  atR;   int atRe;               // R and K + 1 of the period step
    float  atK1;  int atK1e;
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
    uint  gateMode;      // seedGate: 1 per-tile forecast, 2 every tile seeded
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

// Pixel state of a pixel inside the AT between chunks: st.e == AT_MODE, st.dz = y (float32),
// st.d / st.dExp = A dz/dc * texel, st.w / st.wExp = dz/dz1 (floatexp), m = 0. Keeping y itself
// (not z = y / A) makes the result independent of where chunks cut the AT run.
constant int AT_MODE = 0x40000000;

// |y|^2 bound of the AT for a pixel at c: |A|^2 (R - (K + 1) |c|), 0 when the period step is
// not valid at all for it (capped at 2^100: y stays in float32 range).
static inline float atRadius(constant LaParams& la, FE C) {
    const FE kc = feMul(FE{float2(la.atK1, 0.0f), la.atK1e}, feNorm(float2(length(C.m), 0.0f), C.e));
    const FE t = feAdd(FE{float2(la.atR, 0.0f), la.atRe}, FE{-kc.m, kc.e});
    if (!(t.m.x > 0.0f)) return 0.0f;
    const FE r = feNorm(float2(t.m.x * dot(la.atA, la.atA), 0.0f), t.e + 2 * la.atAe);
    if (r.e > 100) return 0x1p100f;
    return r.e < -100 ? 0.0f : feFloat(r).x;
}

// The AT loop: y <- y^2 + c' (P iterations each) while |y|^2 < rad. D = A dz/dc * texel and
// W = dz/dz1 ride along (floatexp): D' = 2 y D + CC texel, W' = 2 y W (W = A after the first
// step from z = 0, n = 0: dz_P/dz_1 of the remaining steps). Returns 0: left the AT (y no
// longer valid, or the limit is less than a period away), 1: chunk budget spent, 2: interior.
static inline uint atLoop(constant LaParams& la, float rad, float2 cp, FE CCts,
                          thread float2& y, thread FE& D, thread FE& W, thread uint& n, thread uint& budget,
                          uint maxIter, float interiorEps, thread float& icMinLog2, thread uint& icQ) {
    const float lA = log2(length(la.atA)) + float(la.atAe);
    while (true) {
        if (n + la.period > maxIter || !(dot(y, y) < rad)) return 0u;
        if (budget == 0) return 1u;
        const FE y2 = feNorm(2.0f * y, 0);
        if (kDE) D = feAdd(feMul(y2, D), CCts);
        if (kInterior) W = n == 0 ? FE{la.atA, la.atAe} : feMul(y2, W);
        y = cmul(y, y) + cp;
        n += la.period;
        budget--;
        if (kInteriorColor) {
            const float l = 0.5f * log2(dot(y, y)) - lA;
            if (l < icMinLog2) { icMinLog2 = l; icQ = n; }
        }
        if (kInterior && max(abs(W.m.x), abs(W.m.y)) * (float(n) * float(n)) * pow2f(clamp(W.e, -126, 127)) < interiorEps) return 2u;
    }
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
            const FE z2 = FE{z.m, z.e + 1};
            const FE A2 = feAdd(feMul(z2, A), TS);
            const FE t = feAdd(feMul(A, A), feMul(z, B));
            const FE B2 = FE{t.m, t.e + 1};
            const FE dz2 = feAdd(feMul(feAdd(FE{Zx.m, Zx.e + 1}, dz), dz), C);
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
                const FE t2 = feAdd(feMul(A, W), feMul(z, WD));
                WD = FE{t2.m, t2.e + 1};
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
            const float2 A2 = 2.0f * cmul(z, A) + float2(ts, 0.0f);
            const float2 B2 = 2.0f * (cmul(A, A * invTs) + cmul(z, B));
            const float2 dz2 = cmul(2.0f * Z + dz, dz) + dcf;
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
                const FE t2 = feAdd(feMul(feNorm(A, 0), W), feMul(zf, WD));
                WD = FE{t2.m, t2.e + 1};
                W = feMul(FE{zf.m, zf.e + 1}, W);
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
// share of a pixel's work (auto, gateMode 1): mean path (seed steps a pixel saves) >= gateAbs,
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
    if (sp.gateMode == 2u) {
        tiles[tid] = 1u;
        atomic_fetch_add_explicit(&status.seededTiles, 1u, memory_order_relaxed);
        return;
    }
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
            active = tileTexel(a, p.tilesX, p.width, p.height, p.tileOrder, order, xy, p.fillBorder);
            st.pixel = xy.y * p.width + xy.x;
        }
    } else {
        active = false;
        st.pixel = 0;
    }

    uint2 gid = uint2(st.pixel % p.width, st.pixel / p.width);
    // (A sample cache level is toroidal: the texel's sample position is its window texel's.)
    const uint2 gw = (gid + uint2(p.wrap & 0xffffu, p.wrap >> 16)) % uint2(p.width, p.height);
    float2 pix = float2(gw) + p.jitter - p.halfSize;
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
        // M6 AT (kAT): this pixel's |y|^2 bound (0: the period step is never valid for it) and c'.
        float atRad = 0.0f;
        float2 atCp = 0.0f;
        FE atCCts = FE{float2(0.0f), FE_ZERO};
        bool atMode = false;   // resumed inside the AT: w holds y (AT_MODE)
        if (kAT) {
            atRad = atRadius(la, C);
            const FE cpx = feAdd(feMul(FE{la.atCC, la.atCCe}, C), FE{la.atRC, la.atRCe});
            if (cpx.e > 60) atRad = 0.0f; else atCp = feFloat(cpx);
            atCCts = feMul(FE{la.atCC, la.atCCe}, TS);
        }
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
        } else if (fresh && kAT && atRad > 0.0f && la.period <= p.maxIter) {
            // M6: from z_0 = 0 at m = 0, so the AT takes the first periods.
            w = 0.0f; e = 0; dm = 0.0f; dExp = FE_ZERO;
            zw = float2(1.0f, 0.0f); wExp = 0;
            m = 0; n = 0;
            st.n = 0;
        } else if (fresh) {
            // z_1 = c, d_1 = texel, w_1 = 1.
            if (C.e >= DEEP_PLAIN_EXP) { w = feFloat(C); e = 0; } else { w = C.m; e = C.e; }
            dm = TS.m; dExp = TS.e;
            zw = float2(1.0f, 0.0f); wExp = 0;
            m = 1; n = 1;
            st.n = 0;   // the first iteration counts in this chunk
            steps = 1;
        } else if (kAT && st.e == AT_MODE) {
            atMode = true;
            w = st.dz; e = 0; dm = st.d; dExp = st.dExp; zw = st.w; wExp = st.wExp;
            m = 0; n = st.n;
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
        // M6 LA: the next lookup boundary j and its index L_j (la0: after a rebase).
        uint laJ = 0, laLook = 0xffffffffu, la0 = 0xffffffffu;
        if (kLA) {
            la0 = laBounds[0].x;
            laJ = laNext[min(m, last)];
            laLook = laBounds[laJ].x;
        }
        bool atSave = false;
        bool done = false;
        while (!done) {
            // ---- M6 AT at m = 0 (kAT): iterate y = A z while the period step is valid.
            if (kAT && m == 0 && atRad > 0.0f && n != lookedAt && n < p.maxIter && budget > 0) {
                lookedAt = n;
                const FE A = FE{la.atA, la.atAe};
                float2 y;
                FE D, W;
                bool ok = true;
                if (atMode) {
                    y = w; D = FE{dm, dExp}; W = FE{zw, wExp};
                } else {
                    const FE Y = feMul(A, feNorm(w, e));
                    ok = Y.e <= 62;
                    y = feFloat(Y);
                    D = kDE ? feMul(A, feNorm(dm, dExp)) : FE{float2(0.0f), FE_ZERO};
                    W = feNorm(zw, wExp);
                }
                if (ok) {
                    const uint n0 = n;
                    const uint r = atLoop(la, atRad, atCp, atCCts, y, D, W, n, budget, p.maxIter, p.interiorEps,
                                          icMinLog2, icQ);
                    if (kInteriorColor) icR2 = exp2(max(2.0f * icMinLog2, -160.0f));
                    if (r == 1u) {
                        // Budget spent inside the AT: keep y itself (chunk-independent results).
                        w = y; dm = D.m; dExp = D.e; zw = W.m; wExp = W.e;
                        atSave = true;
                        break;
                    }
                    if (n != n0 || atMode) {
                        // Back to z = y / A at m = 0 (a period boundary of the reference).
                        const FE Ai = FE{la.atAi, la.atAie};
                        const FE zx = feMul(Ai, feNorm(y, 0));
                        if (kDE) {
                            const FE dx = feMul(Ai, D);
                            dm = dx.m; dExp = dx.e;
                            tsd = float2(TS.m.x * pow2f(min(TS.e - dExp, 100)), 0.0f);
                        }
                        if (kInterior) { zw = W.m; wExp = W.e; }
                        if (zx.e >= DEEP_PLAIN_EXP) { w = feFloat(zx); e = 0; } else { w = zx.m; e = zx.e; }
                        deepRescale(w, e, s, dd, dc, p.scaleExp, C.e);
                    }
                    atMode = false;
                    if (r == 2u) { interior = true; break; }
                }
            }
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
                        tsd = float2(TS.m.x * pow2f(min(TS.e - dExp, 100)), 0.0f);
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
                    if (((kLA && m == laLook) || (kAT && m == 0 && atRad > 0.0f)) && n != lookedAt) break;
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
                        if (((kLA && m == laLook) || (kAT && m == 0 && atRad > 0.0f)) && n != lookedAt) {
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
                    if (kLA && m == laLook && n != lookedAt) break;
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
        st.dz = w; st.e = atSave ? AT_MODE : e; st.d = dm; st.dExp = dExp; st.w = zw; st.wExp = wExp; st.m = m; st.n = n;
    } else if (active && !inB && !parked && !filled) {
        // M6 AT (kAT): as in the deep path, with C and the texel at exponent 0.
        float atRad = 0.0f;
        float2 atCp = 0.0f;
        FE atCCts = FE{float2(0.0f), FE_ZERO};
        bool atMode = false;
        if (kAT) {
            const FE C = feNorm(dc, 0);
            atRad = atRadius(la, C);
            const FE cpx = feAdd(feMul(FE{la.atCC, la.atCCe}, C), FE{la.atRC, la.atRCe});
            if (cpx.e > 60) atRad = 0.0f; else atCp = feFloat(cpx);
            atCCts = feMul(FE{la.atCC, la.atCCe}, feNorm(tsDelta, 0));
        }
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
        } else if (fresh && kAT && atRad > 0.0f && la.period <= p.maxIter) {
            // M6: from z_0 = 0 at m = 0, so the AT takes the first periods.
            st.dz = 0.0f;
            st.d = 0.0f;
            st.w = float2(1.0f, 0.0f);
            st.wExp = 0;
            st.m = 0;
            st.n = 0;
            z = 0.0f;
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
        } else if (kAT && st.e == AT_MODE) {
            atMode = true;   // st holds y and the derivatives as floatexp (AT_MODE)
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
        bool atSave = false;
        float2 atY = 0.0f;
        FE atD = FE{float2(0.0f), FE_ZERO}, atW = atD;
        // Lookups only at m = 1 (mod bla.lookupEvery): a lookup costs several exact steps
        // (dependent table loads, divergence), and a rule on m alone keeps results independent
        // of how chunks cut the orbit (shown frame = fresh render).
        const uint alignMask = kBLA ? bla.lookupEvery - 1u : 0u;
        uint budget = p.chunkIters;
        while (!escaped && !interior && n < p.maxIter && budget > 0) {
            // M6 AT at m = 0 (kAT): iterate y = A z while the period step is valid.
            if (kAT && m == 0 && atRad > 0.0f && n != lookedAt) {
                lookedAt = n;
                const FE A = FE{la.atA, la.atAe};
                float2 y;
                FE D, W;
                bool ok = true;
                if (atMode) {
                    y = st.dz; D = FE{st.d, st.dExp}; W = FE{st.w, st.wExp};
                } else {
                    const FE Y = feMul(A, feNorm(dz, 0));
                    ok = Y.e <= 62;
                    y = feFloat(Y);
                    D = kDE ? feMul(A, feNorm(d, 0)) : FE{float2(0.0f), FE_ZERO};
                    W = feNorm(w, wExp);
                }
                if (ok) {
                    const uint n0 = n;
                    const uint r = atLoop(la, atRad, atCp, atCCts, y, D, W, n, budget, p.maxIter, p.interiorEps,
                                          icMinLog2, icQ);
                    if (kInteriorColor) icR2 = exp2(max(2.0f * icMinLog2, -160.0f));
                    if (r == 1u) {
                        atY = y; atD = D; atW = W;
                        atSave = true;
                        break;
                    }
                    if (n != n0 || atMode) {
                        // Back to z = y / A at m = 0.
                        const FE Ai = FE{la.atAi, la.atAie};
                        dz = feFloat(feMul(Ai, feNorm(y, 0)));
                        if (kDE) d = feFloat(feMul(Ai, D));
                        if (kInterior) {
                            if (W.e <= 0) {
                                w = scale2(W.m, max(W.e, -250));
                                wExp = 0;
                            } else {
                                const int k = ((W.e + 59) / 60) * 60;
                                w = scale2(W.m, W.e - k);
                                wExp = k;
                            }
                        }
                        z = dz;
                        r2 = dot(z, z);
                    }
                    atMode = false;
                    if (r == 2u) { interior = true; break; }
                    if (n >= p.maxIter || budget == 0) break;
                }
            }
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
                    if (r2 < dot(dz, dz) || m == last) {
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
                    if (kLA) { laJ = 0; laLook = la0; }
                }
                if (kBLA && m == 1) break;
                if ((kLA && m == laLook) || (kAT && m == 0 && atRad > 0.0f)) break;
            }
            budget -= max(n - n0, 1u);
        }
        iters += n - st.n;
        steps += p.chunkIters - min(budget, p.chunkIters);
        st.dz = dz; st.d = d; st.w = w; st.m = m; st.n = n; st.wExp = wExp;
        if (kAT) {
            st.e = atSave ? AT_MODE : 0;
            if (atSave) { st.dz = atY; st.d = atD.m; st.dExp = atD.e; st.w = atW.m; st.wExp = atW.e; }
        }
    }

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
        && st.n == p.maxIter && pBudget > 0u && !(kAT && st.e == AT_MODE)) {
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
        probing = true;
    }
    if (kProbe && probing) {
        uint m = st.m, mB = ps.mB, k = ps.k, info = ps.info;
        uint used = st.n - p.maxIter;
        const uint limit = min(p.probe, PROBE_LOW);
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
                if (zf.e >= 3) { pDone = true; break; }   // |z| >= 2: escapes
                const uint low = info & PROBE_LOW;
                uint act = PA_SCAN;
                if ((info & PROBE_NEWTON) != 0) {
                    act = k == low ? PA_EVAL : PA_SCAN;
                } else if (k > 0) {
                    const FE dv = feSub(zf, zB);
                    if (k > ps.pRej && feLess(FE{dv.m, dv.e + 2}, zB)) act = low >= k ? PA_EVAL : PA_RESET;
                    else if (feLess(zf, zB)) act = PA_RESET;
                }
                if (act == PA_EVAL) {
                    // g = F(z_B) - z_B, exact in Z where the reference indices agree.
                    const FE dd = feSub(dzx, dB);
                    const FE g = m == mB ? dd : feAdd(feSub(Zx, ZB), dd);
                    float lgMag = lgSum(feLog2(dzx), feLog2(dB));
                    if (m != mB) lgMag = lgSum(lgMag, lgSum(feLog2(Zx), feLog2(ZB)));
                    FE delta;
                    act = probeEval(g, lam, fzz, zB, lgMag, k, info, delta);
                    if (act == PA_PROVEN) { pProven = true; pDone = true; pPeriod = k; break; }
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
                        info = (info & (15u << 26)) | used;
                        spent++;
                        continue;
                    }
                }
                if (act != PA_SCAN) {
                    // New base point here (a new minimum, a too early return, a hopeless candidate).
                    dB = dzx; mB = m; ZB = Zx; zB = feAdd(ZB, dB);
                    k = 0; lam = one; fzz = FE{float2(0.0f), FE_ZERO};
                    info = (info & (15u << 26)) | used;
                }
                // F'' first (old lambda), then lambda, then the step.
                const FE z2 = feTwice(zf);
                fzz = feAdd(feMul(z2, fzz), feTwice(feMul(lam, lam)));
                lam = feMul(z2, lam);
                dzx = feAdd(feMul(feAdd(feTwice(Zx), dzx), dzx), C);
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
                if (!(r2 < 4.0f)) { pDone = true; break; }   // |z| >= 2: escapes
                const uint low = info & PROBE_LOW;
                uint act = PA_SCAN;
                if ((info & PROBE_NEWTON) != 0) {
                    act = k == low ? PA_EVAL : PA_SCAN;
                } else if (k > 0) {
                    const float2 dv = z - zB;
                    if (k > ps.pRej && 16.0f * dot(dv, dv) < zB2) act = low >= k ? PA_EVAL : PA_RESET;
                    else if (r2 < zB2) act = PA_RESET;
                }
                if (act == PA_EVAL) {
                    const float2 dd = dz - dB;
                    const float2 g = m == mB ? dd : (Z - ZB) + dd;
                    float mag = length(dz) + length(dB);
                    if (m != mB) mag += length(Z) + length(ZB);
                    FE delta;
                    act = probeEval(feNorm(g, 0), feNorm(l, le), feNorm(f, fe), feNorm(zB, 0),
                                    log2(max(mag, 1e-37f)), k, info, delta);
                    if (act == PA_PROVEN) { pProven = true; pDone = true; pPeriod = k; break; }
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
                        info = (info & (15u << 26)) | used;
                        spent++;
                        continue;
                    }
                }
                if (act != PA_SCAN) {
                    dB = dz; mB = m; ZB = Z; zB = z; zB2 = r2;
                    k = 0; l = float2(1.0f, 0.0f); le = 0; f = 0.0f; fe = 0;
                    info = (info & (15u << 26)) | used;
                }
                // F'' first (old lambda; f aligned to l's exponent), then lambda, then the step.
                const float2 z2 = 2.0f * z;
                if (f.x == 0.0f && f.y == 0.0f) fe = 2 * le;
                int sh = 2 * le - fe;
                if (sh > 100) { f = 0.0f; fe = 2 * le; sh = 0; }
                f = cmul(z2, f) + (2.0f * pow2f(max(sh, -127))) * cmul(l, l);
                l = cmul(z2, l);
                const float al = max(abs(l.x), abs(l.y));
                if (al > 0x1p30f || (al < 0x1p-30f && al > 0.0f)) { const int t = expOf(al); l = scale2(l, -t); le += t; }
                const float af = max(abs(f.x), abs(f.y));
                if (af > 0x1p30f || (af < 0x1p-30f && af > 0.0f)) { const int t = expOf(af); f = scale2(f, -t); fe += t; }
                dz = cmul(2.0f * Z + dz, dz) + dc;
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

    // (Parked: unresolved at maxIter with the probe on, waiting for a kProbe chunk.)
    bool alive = active && (inB ? !bDone : probing ? !pDone
        : (!escaped && !interior && (st.n < p.maxIter || (kInterior && p.probe != 0 && !(kAT && st.e == AT_MODE)))));
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
        if (kProbe && probing) probeBuf[(1u - p.probeHalf) * p.capacity + tgBase + sgLive[sg] + offset] = ps;
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
    uint   pad3[3];
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

static inline float3 shade(float2 it, float2 de, float2 ax, constant ColorParams& c, device const float4* pal,
                           float pixelsPerTexel, float sampleWidth = 1.0f) {
    // Unresolved (past the nominal limit, neither escaped nor proven interior): a dark grey
    // veil (25 %) over the outline colour, so "not computed yet" no longer looks like the
    // interior (user decision 2026-10-17: near the -3/4 neck almost the whole view was
    // unresolved and read as interior). Before, it was the outline colour lifted by 0.5 %;
    // FL: M2's 3 % lift showed grey halos around minibrots in gesture frames (no extension
    // passes), and minibrot bodies the interior test does not prove show this veil too.
    if (it.x == ITER_UNRESOLVED) return mix(c.edge.rgb, float3(0.18f, 0.18f, 0.20f), 0.25f);
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
                    acc += shade(it, des.read(s).xy, auxAt(auxs, s, c, it), c, pal, c.deScale, 1.0f / float(ss));
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
    // Sample cache (kb: techniques/sample-reuse): the new samples are the cache's lattice, turned
    // by newRot against the output; window texel t (sample at t itself, no jitter) is colour
    // texel (t + wrap) mod size, premultiplied (alpha 0: no sample). cache 0: a preview level.
    float2 newRot;
    uint2  wrap;
    uint   cache;
    float  sampleWidth;   // mode 2: the palette filter's width of an AA sample, output pixels
                          // (1 / sqrt(samples): its share of the pixel); else unused
};

// A cache colour texel by window texel (zero outside the window).
static inline float4 cacheColor(texture2d<float, access::sample> t, int2 i, constant ComposeParams& q,
                                float2 size) {
    if (any(i < 0) || any(float2(i) >= size)) return float4(0.0f);
    // (t + wrap) mod size with t, wrap < size: one conditional subtraction, no division.
    const uint2 n = uint2(size), u = uint2(i) + q.wrap;
    return t.read(select(u, u - n, u >= n));
}

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
    const float2 g = pc - q.outCentre;
    const float2 v = q.cache != 0
        ? q.newCentre + q.newRatio * float2(g.x * q.newRot.x + g.y * q.newRot.y, g.y * q.newRot.x - g.x * q.newRot.y)
        : (pc - q.outCentre) * q.newRatio + q.newCentre;

    // The bilinear preview: the fallback where nothing better exists.
    float3 base = c.interior.rgb;
    // Sample cache: the 2 x 2 lattice samples around the pixel (point samples in mode 0; their
    // bilinear mean, over the ones present, as the base).
    float3 cacheSum = 0.0f;
    float cacheW = 0.0f;
    bool cacheMiss = false;
    if (c.hasUnder > 0.0f && q.cache != 0) {
        const int2 b0 = int2(floor(v));
        const float2 f = v - float2(b0);
        float4 bl = 0.0f;
        for (int j = 0; j < 2; j++) {
            for (int i = 0; i < 2; i++) {
                const int2 t = b0 + int2(i, j);
                const float4 s = cacheColor(preview, t, q, c.underSize);
                bl += s * ((i == 0 ? 1.0f - f.x : f.x) * (j == 0 ? 1.0f - f.y : f.y));
                const float2 d = (float2(t) - v) / q.newRatio;
                const float w = exp(-dot(d, d) * q.invTwoSigma2) * s.a;
                cacheSum += s.rgb * w;
                cacheW += w;
            }
        }
        // (Only a cell with all four samples is interpolated: one with fewer would show its
        // samples as flat squares where they are sparse.)
        if (bl.a > 1e-3f) base = bl.rgb / bl.a;
        cacheMiss = bl.a < 0.999f;
    } else if (c.hasUnder > 0.0f) {
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

    if (cacheMiss) {
        // Not all four cache samples around (a level just switched finer, or not filled yet):
        // the history where there is one, else the samples of the 4 x 4 texels around.
        if (wh > 0.0f) {
            base = hist;
        } else {
            // (A Gaussian over the samples there, about 1.5 texels wide: sparse samples blend
            // instead of showing as squares.)
            const int2 b0 = int2(floor(v));
            float3 acc = 0.0f;
            float wsum = 0.0f;
            for (int j = -1; j <= 2; j++) {
                for (int i = -1; i <= 2; i++) {
                    const int2 t = b0 + int2(i, j);
                    const float4 s = cacheColor(preview, t, q, c.underSize);
                    if (s.a > 0.0f) {
                        const float2 d = float2(t) - v;
                        const float w = exp(-0.5f * dot(d, d));
                        acc += s.rgb * w;
                        wsum += w;
                    }
                }
            }
            if (wsum > 0.0f) base = acc / wsum;
            hist = base;
        }
    }

    float3 col;
    float conf;
    if (q.mode == 0) {
        // Point samples of the jittered preview within the 2 x 2 texels around this pixel.
        float3 sum = cacheSum;
        float wn = cacheW;
        if (q.newRatio > 0.0f && q.cache == 0) {
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
                if (any(p < pr.geom.xy) || any(p >= pr.geom.zw)) continue;
                float2 ts = float2(pr.info.y, pr.info.z);
                uint2 t = uint2(min((p - pr.geom.xy) / (pr.geom.zw - pr.geom.xy) * ts, ts - 1.0f));
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
    uint width;          // level texels (sample cache: the surveyed rectangle)
    uint height;
    uint cols;           // survey cells
    uint rows;
    uint2 origin;        // sample cache: the rectangle's first window texel; window texel t is
    uint wrap;           // texel (t + wrap) mod tex (packed x | y << 16); plain levels: 0, 0, size
    uint tex;
};

static inline float surveyRead(texture2d<float, access::read> iters, constant SurveyParams& p, uint2 xy) {
    const uint2 tex = uint2(p.tex & 0xffffu, p.tex >> 16);
    return iters.read((xy + p.origin + uint2(p.wrap & 0xffffu, p.wrap >> 16)) % tex).x;
}

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
            float n = surveyRead(iters, p, uint2(x, y));
            if (n == ITER_PENDING || n == ITER_SKIP || n == ITER_EMPTY) continue;
            samples++;
            bool in = n < 0.0f;
            if (in) interior++;
            for (uint k = 0; k < 2; k++) {
                uint2 q = k == 0 ? uint2(x + 1, y) : uint2(x, y + 1);
                if (q.x >= p.width || q.y >= p.height) continue;
                float m = surveyRead(iters, p, q);
                if (m == ITER_PENDING || m == ITER_SKIP || m == ITER_EMPTY) continue;
                pairs++;
                bool min = m < 0.0f;
                if (min != in || (!in && m != n)) edges++;
            }
        }
    }
    cells[gid.y * p.cols + gid.x] = uint4(samples, interior, pairs, edges);
}
