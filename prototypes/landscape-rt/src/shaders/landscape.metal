// L1 landscape: Mandelbrot terrain as a geometry clipmap (kb: decisions/landscape-mode, L1).
//
// Data: per clipmap level ℓ (absolute sample spacing 2^ℓ in the complex plane) an N×N toroidal
// R32Float slice holding log2(DE / spacing) at camera-independent sample positions c = origin +
// (i, j)·2^ℓ (−0.5 = interior or unresolved = lake), and a (N/16)² RG32Float meta slice per
// 16×16 tile: (birth time, tag). The tag names the level and the wrap cycle of the tile, so a
// texel is valid only if it really holds the sample the ring wants; the birth time fades new
// tiles in over `fade` seconds (no popping when detail arrives).
//
// Geometry: ring ℓ is an M×M vertex grid (vertex spacing K samples) drawn as 16×16-quad blocks
// (instances), minus the hole the finer ring covers. Vertex heights are geomorphed towards the
// coarser ring's triangle surface by α (distance from the camera, continuous), and towards it
// as well where the data is missing or still fading in. Heights h/S = σ·ln(1 + k·DE/(σS)) are
// computed from DE every frame with the current S, so diving only changes them smoothly.

#include <metal_stdlib>
using namespace metal;

// ---------------------------------------------------------------- DE tiles (compute)

struct DeParams {
    uint  refLen;       // entries in ref[] (>= 2)
    uint  maxIter;
    float bailout2;
    float interiorEps;  // |w|·n² < eps → interior
    float now;          // birth time written into the meta texels
    uint  jobCount;
    uint  chunk;        // iterations per sample in this dispatch
    float pad1;
};

struct TileJob {        // 48 bytes, matches gpu.rs
    float2 dc0;         // c(tile sample 0,0) − c_ref
    float  spacing;     // complex units per sample
    uint   slot;        // texture array layer
    uint2  texel;       // texel of sample (0,0) (multiple of 16)
    float  tag;         // exact integer < 2^24
    uint   state;       // state slot; high bit: resume from it
    uint2  ptexel;      // parent level: texel of the sample at this tile's sample (0,0)
    uint   pslot;       // parent's layer (0xFFFFFFFF: no parent level, compute all 256)
    uint   pad;
};

constant float LAKE = -128.0f;   // stored value of interior samples

static inline float2 cmul(float2 a, float2 b) { return float2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x); }

// Perturbation with Zhuoran rebasing against a CPU reference orbit, float32, with dz/dc for the
// distance estimate and dz/dz1 for the derivative interior test (the app's `iterate` kernel,
// crates/almondbread-render/src/shaders/mandelbrot.metal, without chunking or statistics).
struct SampleState {     // 48 bytes, matches gpu.rs
    float2 dz, d, w;
    uint   m, n;            // n = DONE: the sample is written
    int    wExp, dExp;
    float  sdScaled;
    uint   pad;
};
constant uint DONE = 0xFFFFFFFFu;

// One 16×16 tile per threadgroup. Each dispatch runs at most `chunk` iterations per sample; a
// tile whose samples are not all finished keeps its loop state (states[job.state·256 + i]) and
// is resumed in a later frame (unfinished[job] = 1). Its meta texel (tag) is written only when
// the whole tile is done, so the renderer never sees a partly computed tile. This bounds the
// GPU time of a frame's batch: one tile with slow pixels (near-parabolic interior) used to take
// 10 ms by itself.
kernel void deTiles(texture2d_array<float, access::read_write> dataTex [[texture(0)]],
                    texture2d_array<float, access::write> metaTex [[texture(1)]],
                    texture2d_array<float, access::read_write> ridgeTex [[texture(2)]],
                    constant DeParams& p                    [[buffer(0)]],
                    device const float2* ref                [[buffer(1)]],
                    device const TileJob* jobs              [[buffer(2)]],
                    device SampleState* states              [[buffer(3)]],
                    device uint* unfinished                 [[buffer(4)]],
                    uint3 tg                                [[threadgroup_position_in_grid]],
                    uint3 lt                                [[thread_position_in_threadgroup]],
                    uint tidx                               [[thread_index_in_threadgroup]])
{
    const TileJob job = jobs[tg.x];
    const uint last = p.refLen - 1;
    // L2: the parent tile (level ℓ+1) is done before a tile is scheduled, and every even sample
    // (2i, 2j) of this tile is its sample (i, j): copied (log2(DE/spacing) + 1), not iterated.
    // Threads 0–63 (two SIMD groups) copy; threads 64–255 iterate the other 192 samples, three
    // per 2×2 block, so whole SIMD groups finish early: 25 % fewer SIMD cycles per tile.
    const bool hasParent = job.pslot != 0xFFFFFFFFu;
    uint2 sxy = lt.xy;
    bool copy = false;
    if (hasParent) {
        if (tidx < 64u) { sxy = uint2(2u * (tidx & 7u), 2u * (tidx >> 3)); copy = true; }
        else {
            uint u = tidx - 64u, b = u / 3u, k = u % 3u;
            sxy = uint2(2u * (b & 7u) + (k != 1u ? 1u : 0u), 2u * (b >> 3) + (k != 0u ? 1u : 0u));
        }
    }
    const float2 dc = job.dc0 + float2(sxy) * job.spacing;
    device SampleState& stp = states[(job.state & 0x7FFFFFFFu) * 256u + tidx];
    const bool resume = (job.state & 0x80000000u) != 0u;

    SampleState st;
    float2 z;
    float r2;
    bool escaped = false, interior = false, done = false;
    if (copy && !resume) {
        uint2 pt = job.ptexel + sxy / 2u;
        float v = dataTex.read(pt, job.pslot).x;
        dataTex.write(float4(v <= -60.0f ? LAKE : v + 1.0f), job.texel + sxy, job.slot);
        ridgeTex.write(ridgeTex.read(pt, job.pslot), job.texel + sxy, job.slot);
        st.n = DONE;
        stp = st;
        done = true;
        z = 0.0f; r2 = 0.0f;
    } else if (resume) {
        st = stp;
        done = st.n == DONE;
        z = ref[st.m] + st.dz;
        r2 = dot(z, z);
    } else {
        st.dz = dc; st.d = float2(job.spacing, 0.0f); st.w = float2(1.0f, 0.0f);
        st.m = 1; st.n = 1; st.wExp = 0; st.dExp = 0; st.sdScaled = job.spacing;
        z = ref[1] + st.dz;
        r2 = dot(z, z);
        escaped = r2 > p.bailout2;
        if (!escaped && (r2 < dot(st.dz, st.dz) || st.m == last)) { st.dz = z; st.m = 0; }
    }
    if (!done && !(copy && !resume)) {
        float2 dz = st.dz, d = st.d, w = st.w;
        uint m = st.m, n = st.n;
        int wExp = st.wExp, dExp = st.dExp;   // dz/dc = d · 2^dExp (overflows float32 near the set)
        float sdScaled = st.sdScaled;
        const uint stop = min(p.maxIter, n + p.chunk);
        while (!escaped && n < stop) {
            d = 2.0f * cmul(z, d) + float2(sdScaled, 0.0f);
            w = 2.0f * cmul(z, w);
            if ((n & 7u) == 0u) {
                if (max(abs(d.x), abs(d.y)) > 0x1p60f) { d *= 0x1p-60f; dExp += 60; sdScaled *= 0x1p-60f; }
                float a = max(abs(w.x), abs(w.y));
                if (a > 0x1p60f) { w *= 0x1p-60f; wExp += 60; }
                else if (wExp > 0 && a < 0x1p-60f) { w *= 0x1p60f; wExp -= 60; }
                else if (wExp == 0 && a * (float(n) * float(n)) < p.interiorEps) { interior = true; break; }
            }
            dz = cmul(2.0f * ref[m] + dz, dz) + dc;
            m++; n++;
            z = ref[m] + dz;
            r2 = dot(z, z);
            if (r2 > p.bailout2) { escaped = true; break; }
            if (r2 < dot(dz, dz) || m == last) { dz = z; m = 0; }
        }
        done = escaped || interior || n >= p.maxIter;
        if (done) {
            // stored as log2(DE / spacing): no range clamp (a clamp at a fixed DE/spacing made
            // square terraces, one per level); LAKE for interior / unresolved
            float lde = LAKE, ridge = 0.0f;
            if (escaped) {
                float zr = sqrt(r2);
                float v = log2(2.0f * zr * log(zr)) - log2(length(d)) - float(dExp);
                lde = isfinite(v) ? clamp(v, -50.0f, 60.0f) : -50.0f;
                // L2 equipotential ridges: r = sin²(πν/2), ν the smooth iteration count (the
                // CPU's mu: n − log2(ln|z| / ln 1e5)): one ridge per two dwell bands, so they
                // follow the set's outline at every scale. The height adds them only on plains
                // (DE ≫ σ, see heightS, which also fades them where two bands, ≈ DE wide, span
                // fewer than ~12 samples of the level, so they never alias).
                if (isfinite(v)) {
                    float nu = float(n) - log2(log(zr) / 11.512925f);
                    float sn = sin(1.5707963f * nu);
                    ridge = sn * sn;
                }
            }
            dataTex.write(float4(lde), job.texel + sxy, job.slot);
            ridgeTex.write(float4(ridge), job.texel + sxy, job.slot);
            st.n = DONE;
        } else {
            st.dz = dz; st.d = d; st.w = w; st.m = m; st.n = n; st.wExp = wExp; st.dExp = dExp; st.sdScaled = sdScaled;
        }
        stp = st;
    }
    threadgroup uint left;
    if (tidx == 0) left = 0;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    uint mine = done ? 0u : 1u;
    uint sum = simd_sum(mine);
    if (simd_is_first() && sum > 0) atomic_fetch_add_explicit((threadgroup atomic_uint*)&left, sum, memory_order_relaxed);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tidx == 0) {
        unfinished[tg.x] = left;
        if (left == 0) metaTex.write(float4(p.now, job.tag, 0.0f, 0.0f), job.texel / 16, job.slot);
    }
}

// ---------------------------------------------------------------- terrain (render)

struct Globals {
    float4x4 viewProj;      // camera-relative world (units of S) → clip, reversed Z
    float4 sun;             // xyz: direction to the sun (world), w: fog distance (S)
    float4 eye;             // xyz: eye position (camera-relative world: (0, 0, eyeZ)), w: 1/(2R) curvature
    float4 fwd;             // camera forward (xyz), w: tan(fov/2)
    float4 right;           // xyz, w: aspect (w/h)
    float4 up;              // xyz, w: time now (s)
    float4 hcurve;          // x: σ (S units), y: k/σ, z: fade seconds, w: N
    float4 misc;            // x: 1/N, y: strata shift, z: debug mode, w: profile/measurement mode
    float4 shade;           // x: log2(S_shade / S) (pop check: previous S), y: log2 S,
                            // z: σ_lo / S, w: σ_hi / S (colour layers)
    float4 shade2;          // x: cross-fade σ_lo → σ_hi, y: legacy colour, z: mask (0 hill, 1 lake, 2 fallback)
    float4 overlay;         // xy: crosshair centre (px), z: radius (0: off), w: zoom rate
    float4 detail;          // x: log2 of the largest detail wavelength (complex units), y: amplitude
                            // per octave (fraction of the wavelength), z: detail on, w: ridge height (σ)
    float4 detail2;         // x: vertex displacement on, y: colour variation on, z: log2 S, w: unused
};

// Crosshair: a thin ring with a gap and a centre dot; amber while zooming in, blue out.
static inline float3 crosshair(float3 c, float2 px, constant Globals& g) {
    if (g.overlay.z <= 0.0f) return c;
    float2 d = px - g.overlay.xy;
    float r = length(d);
    float R = g.overlay.z * (1.0f + 0.25f * abs(g.overlay.w));
    float ring = saturate(1.5f - abs(r - R)) * (abs(d.x) > 0.35f * R || abs(d.y) > 0.35f * R ? 1.0f : 0.0f);
    float dot_ = saturate(2.0f - r);
    float a = max(ring, dot_) * 0.85f;
    float3 tint = g.overlay.w > 0.05f ? float3(1.0, 0.75, 0.3) : (g.overlay.w < -0.05f ? float3(0.5, 0.75, 1.0) : float3(1.0));
    return mix(c, tint, a);
}

struct LevelRef {           // one data level as seen from a ring (40 bytes, matches gpu.rs)
    int2   dataMod;         // data origin mod N (texel of data-local sample 0)
    int2   dataDiv;         // data origin div N (low bits; tags use 8 bits)
    float2 ro;              // ring vertex (0,0) in this level's data-local sample coords
    float  spOverS;         // sample spacing / S
    uint   slot;            // texture array layer
    uint   levelBits;       // ((ℓ & 63) + 1) << 16 (0xFFFFFFFF: no such level)
    int    lvl;             // absolute level: spacing 2^lvl complex units
};

constant int MAXLV = 18;
constant int NOCT = 18;     // detail octaves per ring: absolute levels ℓ − 6 ... ℓ + 11

struct RingParams {         // 904 bytes, matches gpu.rs
    LevelRef lv[MAXLV];     // `fine` finer levels (fragment shading only), then ℓ, ℓ+1, ... up
                            // to the coarsest active level (nLv entries in all)
    float2 pos0;            // ring vertex (0,0) relative to the camera, S units
    float  gS;              // vertex spacing / S
    int    K;               // vertex stride in level-ℓ samples
    float  alphaStart;      // L∞ distance (S) where α starts rising
    float  alphaWidth;      // ... and its width (α = 1 beyond start + width)
    int    blockVerts;      // vertices per block edge minus one (16)
    int    nLv;             // valid entries in lv[]
    int    fine;            // index of the ring's own level ℓ in lv[]
    int    lvl;             // ℓ (absolute)
    float2 nbase[NOCT];     // detail lattice: (absolute sample index of data-local 0 at level ℓ)
                            // × 2^(ℓ − oct) mod 128 cells, plus a per-octave offset (exact, CPU)
};

struct VOut {
    float4 pos [[position]];
    float3 world;           // camera-relative, S units (with curvature drop)
    float2 s0;              // level ℓ data-local sample coords
    float  w0;              // detail weight of level ℓ (validity · (1 − α)), debug
    float  w1;              // validity of level ℓ+1, debug
};

constant uint TAGMASK = 255u;   // tag = (ℓ&63)+1 << 16 | divx & 255 << 8 | divy & 255 (< 2^24)

struct Fetch { float de; float ridge; float valid; };

// Fade-in weight of the tile holding data-local integer sample s (level L): 0 unless its tag is
// the one the ring wants (the data texture may hold anything else there).
static inline float validInt(texture2d_array<float, access::read> metaTex, constant Globals& g,
                             constant LevelRef& L, int2 s)
{
    if (L.levelBits == 0xFFFFFFFFu) return 0.0f;
    const int N = int(g.hcurve.w);
    if (any(s < int2(0)) || any(s >= int2(N))) return 0.0f;   // outside this level's window
    int2 a = L.dataMod + s;
    int2 texel = a % N;
    int2 div = L.dataDiv + a / N;
    uint tag = L.levelBits | ((uint(div.x) & TAGMASK) << 8) | (uint(div.y) & TAGMASK);
    float2 meta = metaTex.read(uint2(texel / 16), L.slot).xy;
    return (meta.y == float(tag) ? 1.0f : 0.0f) * saturate((g.up.w - meta.x) / g.hcurve.z);
}

// Raw sample of level L at data-local integer sample coords s, with its fade-in weight.
static inline Fetch fetch(texture2d_array<float, access::sample> dataTex,
                          texture2d_array<float, access::read> metaTex,
                          texture2d_array<float, access::sample> ridgeTex,
                          constant Globals& g, constant LevelRef& L, int2 s)
{
    const int N = int(g.hcurve.w);
    int2 a = L.dataMod + clamp(s, int2(0), int2(N - 1));
    Fetch f;
    f.de = dataTex.read(uint2(a % N), L.slot).x;
    f.ridge = g.detail.w > 0.0f ? ridgeTex.read(uint2(a % N), L.slot).x : 0.0f;
    f.valid = validInt(metaTex, g, L, s);
    return f;
}

// Height (S units): h = σ ln(1 + (k/σ)·DE/S), lde = log2(DE/spacing), interior (LAKE) is the
// lake at 0; plus, on plains (DE/σ from 1.5 up, full beyond 10), equipotential ridges of
// detail.w·σ from the stored ridge phase r = sin²(πν/2): set-derived macro relief where the DE
// field itself is featureless. Near the boundary (DE < 1.5 σ) the canyons keep their shape.
static inline float heightS(float lde, float ridge, float spOverS, constant Globals& g) {
    float de = lde > -60.0f ? exp2(lde) : 0.0f;
    float deS = de * spOverS;
    float h = g.hcurve.x * log(1.0f + g.hcurve.y * deS);
    return h + g.detail.w * g.hcurve.x * ridge * saturate(de / 6.0f - 1.0f) * smoothstep(1.5f, 10.0f, deS / g.hcurve.x);
}

// ---- world-anchored detail noise (L2)
// Octave `oct` is value noise on the integer lattice of absolute level oct (cell = 2^oct
// complex units): a fixed function of the position in the complex plane, whichever ring or data
// level evaluates it, so it never swims. Its lattice position is nbase[oct] + s·2^(ℓ − oct)
// (s: ring-level data-local coords); nbase holds the absolute part mod 128 cells, computed
// exactly on the CPU. The noise (value and gradient per cell) comes from a periodic texture
// of 128 cells. One FMA and one sample per octave: hashing four lattice corners per octave
// cost 2.8 ms per frame at 2560×1600.
static inline float3 detailAt(texture2d<half, access::sample> noiseTex, constant RingParams& rp,
                              float2 s, float lo, float top, float deS, constant Globals& g, int maxOct)
{
    constexpr sampler ns(filter::linear, address::repeat);
    float3 acc = 0.0f;
    int o1 = min(int(floor(top + 1.0f)), rp.lvl + 11);
    int o0 = max(max(int(ceil(lo - 1.0f)), o1 - maxOct + 1), rp.lvl - 6);
    float lsS = log2(rp.lv[rp.fine].spOverS) - float(rp.lvl);     // log2(complex unit / S)
    for (int oct = o1; oct >= o0; oct--) {
        float w = saturate(top - float(oct) + 1.0f) * saturate(float(oct) - lo + 1.0f);
        float lamS = exp2(float(oct) + lsS);         // wavelength in S
        w *= saturate(2.0f - 8.0f * lamS / max(deS, 1e-30f));
        if (w <= 0.0f) continue;
        float2 x = rp.nbase[oct - rp.lvl + 6] + s * exp2(float(rp.lvl - oct));
        float3 n = float3(noiseTex.sample(ns, x * (1.0f / 128.0f)).xyz);
        float A = g.detail.y * lamS * w;
        acc += float3(A * n.x, A * n.yz / lamS);
    }
    return acc;
}

// Detail displacement of ring vertices: the octaves of at least 4 samples of the evaluated
// level ℓ + k (2 vertex spacings), so a level and the ring that geomorphs to it agree. s: the
// vertex in ring-level coords.
static inline float vdetail(texture2d<half, access::sample> noiseTex, constant RingParams& rp, int k,
                            float2 s, float lde, constant Globals& g)
{
    if (g.detail2.x == 0.0f || lde <= -60.0f) return 0.0f;
    float deS = exp2(lde) * rp.lv[rp.fine + k].spOverS;
    return detailAt(noiseTex, rp, s, float(rp.lvl + k) + 2.0f, g.detail.x, deS, g, 4).x;
}

// Bilinear surface of level L at fractional data-local coords s: (height, validity), heights
// weighted by validity.
static inline float2 bilin(texture2d_array<float, access::sample> dataTex,
                           texture2d_array<float, access::read> metaTex,
                           texture2d_array<float, access::sample> ridgeTex,
                           constant Globals& g, constant LevelRef& L, float2 s)
{
    int2 i0 = int2(floor(s));
    float2 t = s - float2(i0);
    float hv = 0.0f, vs = 0.0f;
    for (int j = 0; j < 2; j++)
        for (int i = 0; i < 2; i++) {
            Fetch f = fetch(dataTex, metaTex, ridgeTex, g, L, i0 + int2(i, j));
            float w = (i ? t.x : 1.0f - t.x) * (j ? t.y : 1.0f - t.y) * f.valid;
            hv += w * heightS(f.de, f.ridge, L.spOverS, g);
            vs += w;
        }
    return float2(vs > 0.0f ? hv / vs : 0.0f, vs);
}

// Fallback surface at ring vertex v (ring-ℓ vertex units) from levels ℓ+3 and coarser: the
// finest level that is fully valid there (the scheduler only computes a tile after its parent,
// so one is always found up the pyramid), with the partially valid (fading in) finer levels
// blended on top by their validity. Bilinear, so it is smooth: L1 point-sampled ℓ+3..ℓ+5 at the
// vertices, which made flat tile-shaped steps ("slabs") and ended at ℓ+5 (height 0: water).
static inline float fallbackSurface(texture2d_array<float, access::sample> dataTex,
                                    texture2d_array<float, access::read> metaTex,
                                    texture2d_array<float, access::sample> ridgeTex,
                                    constant Globals& g, constant RingParams& rp, int2 v)
{
    float hs[MAXLV], vs[MAXLV];
    int n = rp.nLv - rp.fine;
    int top = n - 1;
    for (int k = 3; k < n; k++) {
        constant LevelRef& L = rp.lv[rp.fine + k];
        float2 sk = L.ro + float2(v * rp.K) * exp2(-float(k));
        float2 r = bilin(dataTex, metaTex, ridgeTex, g, L, sk);
        hs[k] = r.x; vs[k] = r.y;
        if (r.y >= 0.999f) { top = k; break; }
    }
    if (top < 3) return 0.0f;
    float h = hs[top];
    for (int k = top - 1; k >= 3; k--) h = mix(h, hs[k], vs[k]);
    return h;
}

// Level ℓ+2 surface at a ring-(ℓ+2) vertex q (ring-ℓ vertex units, multiple of 4).
static inline float coarse2(texture2d_array<float, access::sample> dataTex,
                            texture2d_array<float, access::read> metaTex,
                            texture2d_array<float, access::sample> ridgeTex,
                            texture2d<half, access::sample> noiseTex,
                            constant Globals& g, constant RingParams& rp, int2 q)
{
    if (rp.nLv - rp.fine < 3) return 0.0f;
    constant LevelRef& L = rp.lv[rp.fine + 2];
    int2 s2 = int2(L.ro) + (q * rp.K) / 4;
    Fetch f2 = fetch(dataTex, metaTex, ridgeTex, g, L, s2);
    float h2 = heightS(f2.de, f2.ridge, L.spOverS, g) + vdetail(noiseTex, rp, 2, rp.lv[rp.fine].ro + float2(q * rp.K), f2.de, g);
    if (f2.valid >= 1.0f) return h2;
    return mix(fallbackSurface(dataTex, metaTex, ridgeTex, g, rp, q), h2, f2.valid);
}

// Surface of ring ℓ+1 at a ring-(ℓ+1) vertex u (ring-ℓ vertex units, even): its own sample
// blended (by validity) with ring ℓ+2's triangle surface there.
static inline float coarse1(texture2d_array<float, access::sample> dataTex,
                            texture2d_array<float, access::read> metaTex,
                            texture2d_array<float, access::sample> ridgeTex,
                            texture2d<half, access::sample> noiseTex,
                            constant Globals& g, constant RingParams& rp, int2 u, thread float& valid)
{
    if (rp.nLv - rp.fine < 2) { valid = 0.0f; return 0.0f; }
    constant LevelRef& L = rp.lv[rp.fine + 1];
    int2 s1 = int2(L.ro) + (u * rp.K) / 2;
    Fetch f1 = fetch(dataTex, metaTex, ridgeTex, g, L, s1);
    valid = f1.valid;
    float h1 = heightS(f1.de, f1.ridge, L.spOverS, g) + vdetail(noiseTex, rp, 1, rp.lv[rp.fine].ro + float2(u * rp.K), f1.de, g);
    if (f1.valid >= 1.0f) return h1;
    // ring ℓ+2 triangle surface at u: vertices at multiples of 4, diagonal (0,0)-(1,1)
    int2 a = (u / 4) * 4;                    // u >= 0
    int2 r = u - a;                          // components 0 or 2
    float t;
    if (r.x == 0 && r.y == 0) t = coarse2(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a);
    else if (r.y == 0) t = 0.5f * (coarse2(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a) + coarse2(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a + int2(4, 0)));
    else if (r.x == 0) t = 0.5f * (coarse2(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a) + coarse2(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a + int2(0, 4)));
    else t = 0.5f * (coarse2(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a) + coarse2(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a + int2(4, 4)));
    return mix(t, h1, f1.valid);
}

vertex VOut terrainVertex(uint vid                                   [[vertex_id]],
                          uint iid                                   [[instance_id]],
                          constant Globals& g                        [[buffer(0)]],
                          constant RingParams& rp                    [[buffer(1)]],
                          device const ushort2* blocks               [[buffer(2)]],
                          texture2d_array<float, access::sample> dataTex [[texture(0)]],
                          texture2d_array<float, access::read> metaTex   [[texture(1)]],
                          texture2d<half, access::sample> noiseTex   [[texture(2)]],
                          texture2d_array<float, access::sample> ridgeTex [[texture(3)]])
{
    const int B = rp.blockVerts;
    int2 local = int2(int(vid) % (B + 1), int(vid) / (B + 1));
    int2 v = int2(blocks[iid]) * B + local;           // ring-ℓ vertex coords, >= 0
    float2 p = rp.pos0 + float2(v) * rp.gS;
    constant LevelRef& L0 = rp.lv[rp.fine];
    if (g.misc.w == 3.0f || g.misc.w == 6.0f) {   // profiling: no fetches
        VOut o;
        o.world = float3(p, -dot(p, p) * g.eye.w);
        o.pos = g.viewProj * float4(o.world, 1.0f);
        o.s0 = L0.ro + float2(v * rp.K); o.w0 = 0; o.w1 = 0;
        return o;
    }
    float dinf = max(abs(p.x), abs(p.y));
    float alpha = saturate((dinf - rp.alphaStart) / rp.alphaWidth);

    int2 s0 = int2(L0.ro) + v * rp.K;
    Fetch f0 = fetch(dataTex, metaTex, ridgeTex, g, L0, s0);
    float h0 = heightS(f0.de, f0.ridge, L0.spOverS, g) + vdetail(noiseTex, rp, 0, float2(s0), f0.de, g);

    // ring ℓ+1's triangle surface at v (coarse vertices at even v, diagonal (0,0)-(1,1)), with
    // the planet curvature drop: the coarse target is the coarse ring's straight edge in 3D, so
    // its curvature is interpolated too (L1 morphed the height only and dropped the vertex by
    // its own d²/2R: hairline cracks along ring edges, sky showing through)
    int2 a = (v / 2) * 2;
    int2 r = v - a;
    float va = 0.0f, vb = 0.0f;
    float t;
    float2 pa = rp.pos0 + float2(a) * rp.gS;
    if (r.x == 0 && r.y == 0) { t = coarse1(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a, va) - dot(pa, pa) * g.eye.w; vb = va; }
    else {
        int2 b = a + int2(r.x != 0 ? 2 : 0, r.y != 0 ? 2 : 0);
        float2 pb = rp.pos0 + float2(b) * rp.gS;
        t = 0.5f * (coarse1(dataTex, metaTex, ridgeTex, noiseTex, g, rp, a, va) - dot(pa, pa) * g.eye.w
                    + coarse1(dataTex, metaTex, ridgeTex, noiseTex, g, rp, b, vb) - dot(pb, pb) * g.eye.w);
    }
    float w0 = f0.valid * (1.0f - alpha);
    float z = mix(t, h0 - dot(p, p) * g.eye.w, w0);
    VOut o;
    o.world = float3(p, z);
    o.pos = g.viewProj * float4(o.world, 1.0f);
    o.s0 = L0.ro + float2(v * rp.K);
    o.w0 = w0;
    o.w1 = min(va, vb);
    return o;
}

// Filtered surface of level L around data-local coords s: height gradient (S per S) and log2 DE
// (in samples) at the centre.
struct Grad { float2 gr; float de; };

static inline Grad gradAt(texture2d_array<float, access::sample> dataTex,
                          texture2d_array<float, access::sample> ridgeTex, sampler smp,
                          constant Globals& g, constant LevelRef& L, float2 s)
{
    // centre and forward differences: 3 filtered taps (the centre also gives DE and ν)
    const float invN = g.misc.x;
    float2 base = (float2(L.dataMod) + s + 0.5f) * invN;
    float dc = dataTex.sample(smp, base, L.slot).x;
    float dxp = dataTex.sample(smp, base + float2(invN, 0), L.slot).x;
    float dyp = dataTex.sample(smp, base + float2(0, invN), L.slot).x;
    float rc = 0.0f, rx = 0.0f, ry = 0.0f;
    if (g.detail.w > 0.0f) {
        rc = ridgeTex.sample(smp, base, L.slot).x;
        rx = ridgeTex.sample(smp, base + float2(invN, 0), L.slot).x;
        ry = ridgeTex.sample(smp, base + float2(0, invN), L.slot).x;
    }
    float hc = heightS(dc, rc, L.spOverS, g);
    float hx = heightS(dxp, rx, L.spOverS, g), hy = heightS(dyp, ry, L.spOverS, g);
    Grad r;
    r.gr = float2(hx - hc, hy - hc) / L.spOverS;
    r.de = dc;
    return r;
}

static inline float3 pal(float t, float3 a, float3 b, float3 c, float3 d) {
    return a + b * cos(6.2831853f * (c * t + d));
}

static inline float3 skyColor(float3 d, float3 sun) {
    float t = pow(saturate(max(d.z, 0.0f) * 1.5f), 0.6f);
    float3 c = mix(float3(0.62, 0.72, 0.84), float3(0.09, 0.24, 0.62), t);
    float sd = max(dot(d, sun), 0.0f);
    c += float3(1.0, 0.75, 0.45) * (0.25f * pow(sd, 8.0f));
    c += float3(1.0, 0.9, 0.7) * (0.6f * pow(sd, 64.0f));
    c += float3(6.0, 5.5, 4.5) * smoothstep(0.9993f, 0.9997f, sd);
    return c;
}

static inline float3 fogColor(float3 d, float3 sun) {
    float sd = max(dot(d, sun), 0.0f);
    return mix(float3(0.62, 0.72, 0.84), float3(0.95, 0.85, 0.7), 0.5f * pow(sd, 6.0f));
}

static inline float3 tonemap(float3 c, float2 pix) {
    float3 x = c * 0.8f;
    float3 y = (x * (2.51f * x + 0.03f)) / (x * (2.43f * x + 0.59f) + 0.14f);
    float3 s = pow(saturate(y), float3(1.0f / 2.2f));
    // static ordered dither against banding in the sky gradient
    float dth = fract(52.9829189f * fract(dot(pix, float2(0.06711056f, 0.00583715f)))) - 0.5f;
    return s + dth / 255.0f;
}

static inline float validAt(texture2d_array<float, access::read> metaTex, constant Globals& g,
                            constant LevelRef& L, float2 s)
{
    return validInt(metaTex, g, L, int2(floor(s)));
}

fragment float4 terrainFragment(VOut in                                    [[stage_in]],
                                constant Globals& g                        [[buffer(0)]],
                                constant RingParams& rp                    [[buffer(1)]],
                                texture2d_array<float, access::sample> dataTex [[texture(0)]],
                                texture2d_array<float, access::read> metaTex   [[texture(1)]],
                                texture2d<half, access::sample> noiseTex   [[texture(2)]],
                                texture2d_array<float, access::sample> ridgeTex [[texture(3)]])
{
    constexpr sampler smp(filter::linear, address::repeat);
    constant LevelRef& L0 = rp.lv[rp.fine];
    float3 sun = g.sun.xyz;
    if (g.misc.w == 2.0f || g.misc.w == 6.0f) return float4(0.5f + 0.5f * normalize(cross(dfdx(in.world), dfdy(in.world))), 1.0f);

    // geometric normal (faceted) as the last fallback
    float3 ng = normalize(cross(dfdx(in.world), dfdy(in.world)));
    if (ng.z < 0.0f) ng = -ng;

    // Data weights over all levels, fine to coarse, checked here per fragment. The finest
    // levels that resolve the pixel come first, also levels finer than the ring's own (the
    // rings are smaller than the data windows, L2: 256² vertices instead of 448², a third of the
    // vertex work, while the shading keeps the data resolution), handing their share on where a
    // pixel spans more than 2.5 of their samples or they are not valid. The choice depends on
    // the position, the footprint and the data only, not on the ring (no α here), so a ring
    // snapping to a new position changes nothing. Every coarser level takes its validity's
    // share of what is left. The scheduler keeps the valid levels a complete pyramid, so the loop
    // always ends on data: L1 stopped at ℓ+2 and shaded the rest from the faceted geometry, a
    // different material (the "slabs").
    float al = saturate((max(abs(in.world.x), abs(in.world.y)) - rp.alphaStart) / rp.alphaWidth);
    float fp = max(length(dfdx(in.s0)), length(dfdy(in.s0)));   // footprint, level-ℓ samples
    float rem = 1.0f;
    float2 grad = 0.0f;
    float lake = 0.0f, eSum = 0.0f, lSum = 0.0f, vFine = 0.0f;
    int used = 0;
    // the finest level that can resolve the pixel (fk ≤ 2.5): skip the finer ones arithmetically
    int j0 = clamp(rp.fine + int(ceil(log2(max(fp, 1e-6f) / 2.5f))), 0, rp.fine);
    for (int j = j0; j < rp.nLv && used < 2 && rem > 0.003f; j++) {
        int k = j - rp.fine;                          // level relative to the ring's
        constant LevelRef& L = rp.lv[j];
        float sc = exp2(-float(k));
        float fk = fp * sc;
        if (k < 0 && fk > 2.5f) continue;             // too fine for this pixel
        float2 sk = (in.s0 - L0.ro) * sc + L.ro;
        float v = validAt(metaTex, g, L, sk);
        // a finer level's window ends inside this ring: fade it out by the L∞ distance from the
        // camera (continuous; the window itself moves in 16-sample steps, and a fade keyed to its
        // edge jumped with it: the pop check's worst frames), from 400 to 460 samples (the
        // window reaches at least ~470 samples from the camera on every side), or the switch of
        // level shows as a line (gradients differ slightly between levels)
        if (k < 0) {
            float r = max(abs(in.world.x), abs(in.world.y)) / L.spOverS;
            v *= saturate((460.0f - r) / 60.0f);
        }
        if (k <= 1) vFine += v;
        if (j < rp.nLv - 1) v *= saturate(2.5f - fk);
        float w = rem * v;
        if (w > 0.002f) {
            Grad gk = gradAt(dataTex, ridgeTex, smp, g, L, sk);
            float lk = smoothstep(-40.0f, -80.0f, gk.de);
            grad += w * gk.gr;
            lake += w * lk;
            float e = w * (1.0f - lk);
            eSum += e;
            lSum += e * (gk.de + log2(L.spOverS));
            used++;
        }
        rem -= w;
    }
    float covered = 1.0f - rem;
    if (covered > 1e-4f) { grad /= covered; lake /= covered; }

    // geometry fallback only where no level has data at all
    float hS = in.world.z + dot(in.world.xy, in.world.xy) * g.eye.w;
    float lakeGeo = hS < 2e-4f ? 1.0f : 0.0f;
    float deSgeo = max((exp(hS / g.hcurve.x) - 1.0f) / g.hcurve.y, 1e-12f);
    float log2deS = eSum > 1e-6f ? lSum / eSum : log2(deSgeo);
    // the loop stops after two contributing levels; their weights are normalized above, so
    // only a fragment that found no data at all falls back to the geometry
    float noData = covered > 0.002f ? 0.0f : 1.0f;
    lake = mix(lake, lakeGeo, noData);
    float deS = exp2(log2deS);

    float3 viewV = in.world - g.eye.xyz;
    float dist = length(viewV);
    float3 vd = viewV / dist;

    // detail synthesis below the computed resolution (L2): world-anchored noise octaves from
    // the largest detail wavelength down to ~1.5 pixels, on the data normal (fragment) and, for
    // octaves of ≥ 4 samples, in the geometry (vertex). Zero change while hovering: the lattice
    // is fixed in the complex plane and the octave range depends on S and the pixel footprint.
    float3 det = 0.0f;
    float rough = 0.0f;
    if (g.detail.z > 0.0f && lake < 0.99f) {
        float fpS = fp * L0.spOverS;                 // pixel footprint, S
        float lo = log2(1.5f * fpS) - log2(L0.spOverS) + float(L0.lvl);
        det = detailAt(noiseTex, rp, in.s0, lo, g.detail.x, deS, g, 6);
        rough = det.x / max(g.detail.y * exp2(g.detail.x - g.detail2.z), 1e-9f);
    }
    float3 nData = normalize(float3(-(grad + det.yz) * covered, 1.0f));
    float3 n = normalize(nData + ng * noData);

    float3 col;
    float3 albedoOnly = 0.0f, lightOnly = 0.0f;
    if (g.misc.z == 1.0f) {
        // debug: level / validity colours
        col = float3(in.w0, in.w1, 0.2f + 0.6f * float(L0.slot % 3) / 2.0f);
        return float4(col, 1.0f);
    }
    {
        // Colour is a property of the world position, like rock strata:
        // - strata keyed to log2 of the absolute DE (complex units): they never slide while
        //   diving (L0 keyed them to DE/S, then to DE/σ: rings that contracted in every σ
        //   transition, the user's "collapsing rings" on hilltops);
        // - vegetation, crust and valley darkening keyed to DE/σ of the colour layer (σ fixed
        //   per layer), cross-faded between the two layers during a transition.
        float log2deAbs = log2deS + g.shade.y;
        bool legacy = g.shade2.y == 1.0f;            // before the fix: strata by DE/σ, terms by DE/S
        float x = legacy ? max(log2deS, -20.0f) * 0.4f + g.misc.y : log2deAbs * 0.4f;
        float3 c0 = pal(x, float3(0.56, 0.36, 0.24), float3(0.24, 0.16, 0.11), float3(1.0), float3(0.0, 0.06, 0.13));
        float layer = pow(0.5f + 0.5f * cos(6.2831853f * x * 3.0f), 8.0f);
        // the thin layers fade out where a pixel spans more than ~1/4 of their period
        float lw = fwidth(x * 3.0f);
        c0 = mix(c0, float3(0.80, 0.72, 0.60), 0.35f * layer * saturate(1.5f - 4.0f * lw));
        float flat = saturate((n.z - 0.82f) / 0.12f);
        float steep = saturate((0.6f - n.z) / 0.4f);
        // world-anchored variation (L2): the detail relief's value tints vegetation and rock
        float var = g.detail2.y > 0.0f ? rough : 0.0f;
        float3 cl[2];
        float oc[2];
        float sg[2] = { g.shade.z, g.shade.w };
        for (int i = 0; i < 2; i++) {
            float deR = legacy ? deS * exp2(-g.shade.x) * 10.0f : deS / sg[i];   // DE / σ_layer
            float3 c = c0;
            float low = saturate(1.0f - deR / 3.0f);
            float3 green = mix(float3(0.16, 0.27, 0.10), float3(0.30, 0.33, 0.13), saturate(0.5f + 0.9f * var));
            c = mix(c, green, 0.8f * flat * low);
            float high = saturate((deR - 30.0f) / 100.0f);
            float plain = smoothstep(3.0f, 12.0f, deR) * flat;
            float3 meadow = mix(float3(0.42, 0.40, 0.22), float3(0.30, 0.36, 0.17), saturate(0.5f + 0.8f * var));
            meadow *= 1.0f + 0.35f * var;
            c = mix(c, meadow, (g.detail2.y > 0.0f ? 0.55f : 0.0f) * plain * (1.0f - high));
            c = mix(c, float3(0.78, 0.74, 0.66), 0.5f * flat * high);
            cl[i] = mix(c, c * float3(0.75, 0.6, 0.55), steep);
            cl[i] *= 1.0f + 0.25f * var * steep;
            oc[i] = mix(0.55f, 1.0f, saturate(deR * 0.8f));
        }
        float3 c = mix(cl[0], cl[1], g.shade2.x);
        float occl = mix(oc[0], oc[1], g.shade2.x);
        // cavities of the detail relief are darker
        occl *= saturate(1.0f + 0.6f * min(rough, 0.0f));

        float dif = max(dot(n, sun), 0.0f);
        float skyL = (0.5f + 0.5f * n.z) * occl;
        float3 back = normalize(float3(-sun.x, -sun.y, 0.0f));
        float bounce = max(dot(n, back), 0.0f) * 0.12f * occl;
        albedoOnly = c;
        lightOnly = (float3(1.75, 1.45, 1.05) * dif + mix(float3(0.09, 0.24, 0.62), float3(0.62, 0.72, 0.84), 0.4f) * (0.42f * skyL)
                     + float3(0.6, 0.45, 0.3) * bounce) * 0.4f;
        float3 rock = c * (float3(1.75, 1.45, 1.05) * dif + mix(float3(0.09, 0.24, 0.62), float3(0.62, 0.72, 0.84), 0.4f) * (0.42f * skyL)
                           + float3(0.6, 0.45, 0.3) * bounce);

        // lake: deep water, sky reflection by Fresnel, sun glint
        float3 nw = float3(0, 0, 1);
        float cosI = saturate(-dot(vd, nw));
        float fres = 0.02f + 0.98f * pow(1.0f - cosI, 5.0f);
        float3 rv = reflect(vd, nw);
        float3 water = mix(float3(0.01, 0.06, 0.07), skyColor(rv, sun), max(fres, 0.25f));
        float3 hv = normalize(sun - vd);
        water += float3(1.75, 1.45, 1.05) * (2.0f * pow(max(hv.z, 0.0f), 400.0f));
        col = mix(rock, water, lake);
    }
    // alpha: hilltop mask for measurements (flat ground well above the valleys)
    float hill = (n.z > 0.85f && log2deS - log2(g.hcurve.x) > 1.5f && lake < 0.5f) ? 1.0f : 0.0f;
    // shade2.z = 1: the measurement mask is the lake (interior) instead of hilltops; the
    // vertex height says "lake level" independently of the fragment's data blend
    if (g.shade2.z == 1.0f) hill = (in.world.z + dot(in.world.xy, in.world.xy) * g.eye.w < 1e-4f) ? 1.0f : 0.0f;
    // shade2.z = 2: fallback mask: 1 = no valid data level (geometry-only shading: the L1
    // "slabs"), 0.5 = levels ℓ and ℓ+1 missing (shaded from ℓ+2 or coarser: blurry fallback)
    if (g.shade2.z == 2.0f) hill = noData > 0.5f ? 1.0f : (vFine < 0.5f && al < 0.5f ? 0.5f : 0.0f);
    if (g.misc.w == 4.0f) return float4(albedoOnly, hill);          // measurement: colour only
    if (g.misc.w == 5.0f) return float4(lightOnly, hill);           // measurement: lighting only
    float fd = 1.0f - exp(-dist / g.sun.w);
    col = mix(col, fogColor(vd, sun), saturate(fd));
    return float4(crosshair(tonemap(col, in.pos.xy), in.pos.xy, g), hill);
}

// ---------------------------------------------------------------- sky (fullscreen)

struct SkyOut { float4 pos [[position]]; float2 ndc; };

vertex SkyOut skyVertex(uint vid [[vertex_id]]) {
    float2 p = float2((vid << 1) & 2, vid & 2) * 2.0f - 1.0f;
    SkyOut o;
    o.pos = float4(p, 0.0f, 1.0f);
    o.ndc = p;
    return o;
}

fragment float4 skyFragment(SkyOut in [[stage_in]], constant Globals& g [[buffer(0)]]) {
    float th = g.fwd.w;
    float3 d = normalize(g.fwd.xyz + g.right.xyz * (in.ndc.x * th) + g.up.xyz * (in.ndc.y * th / g.right.w));
    float3 c = skyColor(d, g.sun.xyz);
    float haze = saturate(1.0f - max(d.z, 0.0f) * 6.0f);
    c = mix(c, fogColor(d, g.sun.xyz), 0.6f * haze * haze);
    return float4(crosshair(tonemap(c, in.pos.xy), in.pos.xy, g), 0.0f);
}
