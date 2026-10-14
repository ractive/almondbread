// L1 landscape: Mandelbrot terrain as a geometry clipmap (kb: decisions/landscape-mode, L1).
//
// Data: per clipmap level ℓ (absolute sample spacing 2^ℓ in the complex plane) an N×N toroidal
// R16Float slice holding DE / spacing at camera-independent sample positions c = origin +
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
    uint  pad1;
};

struct TileJob {        // 32 bytes, matches gpu.rs
    float2 dc0;         // c(tile sample 0,0) − c_ref
    float  spacing;     // complex units per sample
    uint   slot;        // texture array layer
    uint2  texel;       // texel of sample (0,0) (multiple of 16)
    float  tag;         // exact integer < 2^24
    uint   state;       // state slot; high bit: resume from it
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
kernel void deTiles(texture2d_array<half, access::write>  dataTex [[texture(0)]],
                    texture2d_array<float, access::write> metaTex [[texture(1)]],
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
    const float2 dc = job.dc0 + float2(lt.xy) * job.spacing;
    device SampleState& stp = states[(job.state & 0x7FFFFFFFu) * 256u + tidx];
    const bool resume = (job.state & 0x80000000u) != 0u;

    SampleState st;
    float2 z;
    float r2;
    bool escaped = false, interior = false, done = false;
    if (resume) {
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
    if (!done) {
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
            float lde = LAKE;
            if (escaped) {
                float zr = sqrt(r2);
                float v = log2(2.0f * zr * log(zr)) - log2(length(d)) - float(dExp);
                lde = isfinite(v) ? clamp(v, -50.0f, 60.0f) : -50.0f;
            }
            dataTex.write(half4(half(lde)), job.texel + lt.xy, job.slot);
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
    float4 shade2;          // x: cross-fade σ_lo → σ_hi
};

struct LevelRef {           // one data level as seen from a ring (32 bytes)
    int2  dataMod;          // data origin mod N (texel of data-local sample 0)
    int2  dataDiv;          // data origin div N (low bits; tags use 8 bits)
    int2  ringOrigin;       // ring vertex (0,0) in this level's data-local sample coords
    float spOverS;          // sample spacing / S
    uint  slot;             // layer; levelBits in the high part of `tagLevel`
};

struct RingParams {
    LevelRef lv[4];         // ℓ, ℓ+1, ℓ+2, ℓ+3
    uint4  levelBits;       // ((ℓ+k & 63) + 1) << 16 for k = 0..3 (0xFFFFFFFF: no such level)
    float2 pos0;            // ring vertex (0,0) relative to the camera, S units
    float  gS;              // vertex spacing / S
    int    K;               // vertex stride in level-ℓ samples
    float  alphaStart;      // L∞ distance (S) where α starts rising
    float  alphaWidth;      // ... and its width (α = 1 beyond start + width)
    int    blockVerts;      // vertices per block edge minus one (16)
    int    pad;
};

struct VOut {
    float4 pos [[position]];
    float3 world;           // camera-relative, S units (with curvature drop)
    float2 s0;              // level ℓ data-local sample coords
    float2 s1;              // level ℓ+1
    float  w0;              // detail weight of level ℓ (validity · (1 − α))
    float  w1;              // validity of level ℓ+1
};

constant uint TAGMASK = 255u;   // tag = (ℓ&63)+1 << 16 | divx & 255 << 8 | divy & 255 (< 2^24)

struct Fetch { float de; float valid; };

// Raw sample of level k at data-local integer sample coords s, with its fade-in weight.
static inline Fetch fetch(texture2d_array<float, access::sample> dataTex,
                          texture2d_array<float, access::read> metaTex,
                          constant Globals& g, constant RingParams& rp, int k, int2 s)
{
    constant LevelRef& L = rp.lv[k];
    const int N = int(g.hcurve.w);
    int2 a = L.dataMod + s;                 // >= 0 (s >= 0 inside the data ring)
    int2 texel = a % N;
    int2 div = L.dataDiv + a / N;
    uint tag = rp.levelBits[k] | ((uint(div.x) & TAGMASK) << 8) | (uint(div.y) & TAGMASK);
    float2 meta = metaTex.read(uint2(texel / 16), L.slot).xy;
    Fetch f;
    f.de = dataTex.read(uint2(texel), L.slot).x;
    float ok = (meta.y == float(tag)) ? 1.0f : 0.0f;
    f.valid = ok * saturate((g.up.w - meta.x) / g.hcurve.z);
    return f;
}

static inline float heightS(float lde, float spOverS, constant Globals& g) {
    // h/S = σ ln(1 + (k/σ)·DE/S); lde = log2(DE/spacing); interior (LAKE) is the lake at 0
    float de = lde > -60.0f ? exp2(lde) : 0.0f;
    return g.hcurve.x * log(1.0f + g.hcurve.y * de * spOverS);
}

// Level ℓ+2 surface at a ring-(ℓ+2) vertex q (ring-ℓ vertex units, multiple of 4); falls back
// to ℓ+3, then the lake level.
static inline float coarse2(texture2d_array<float, access::sample> dataTex,
                            texture2d_array<float, access::read> metaTex,
                            constant Globals& g, constant RingParams& rp, int2 q)
{
    Fetch f2 = fetch(dataTex, metaTex, g, rp, 2, rp.lv[2].ringOrigin + (q * rp.K) / 4);
    float h2 = heightS(f2.de, rp.lv[2].spOverS, g);
    if (f2.valid >= 1.0f) return h2;
    Fetch f3 = fetch(dataTex, metaTex, g, rp, 3, rp.lv[3].ringOrigin + (q * rp.K) / 8);
    float h3 = f3.valid > 0.0f ? heightS(f3.de, rp.lv[3].spOverS, g) : 0.0f;
    return mix(h3, h2, f2.valid);
}

// Surface of ring ℓ+1 at a ring-(ℓ+1) vertex u (ring-ℓ vertex units, even): its own sample
// blended (by validity) with ring ℓ+2's triangle surface there.
static inline float coarse1(texture2d_array<float, access::sample> dataTex,
                            texture2d_array<float, access::read> metaTex,
                            constant Globals& g, constant RingParams& rp, int2 u, thread float& valid)
{
    Fetch f1 = fetch(dataTex, metaTex, g, rp, 1, rp.lv[1].ringOrigin + (u * rp.K) / 2);
    valid = f1.valid;
    float h1 = heightS(f1.de, rp.lv[1].spOverS, g);
    if (f1.valid >= 1.0f) return h1;
    // ring ℓ+2 triangle surface at u: vertices at multiples of 4, diagonal (0,0)-(1,1)
    int2 a = (u / 4) * 4;                    // u >= 0
    int2 r = u - a;                          // components 0 or 2
    float t;
    if (r.x == 0 && r.y == 0) t = coarse2(dataTex, metaTex, g, rp, a);
    else if (r.y == 0) t = 0.5f * (coarse2(dataTex, metaTex, g, rp, a) + coarse2(dataTex, metaTex, g, rp, a + int2(4, 0)));
    else if (r.x == 0) t = 0.5f * (coarse2(dataTex, metaTex, g, rp, a) + coarse2(dataTex, metaTex, g, rp, a + int2(0, 4)));
    else t = 0.5f * (coarse2(dataTex, metaTex, g, rp, a) + coarse2(dataTex, metaTex, g, rp, a + int2(4, 4)));
    return mix(t, h1, f1.valid);
}

vertex VOut terrainVertex(uint vid                                   [[vertex_id]],
                          uint iid                                   [[instance_id]],
                          constant Globals& g                        [[buffer(0)]],
                          constant RingParams& rp                    [[buffer(1)]],
                          device const ushort2* blocks               [[buffer(2)]],
                          texture2d_array<float, access::sample> dataTex [[texture(0)]],
                          texture2d_array<float, access::read> metaTex   [[texture(1)]])
{
    const int B = rp.blockVerts;
    if (g.misc.w == 3.0f) {   // profiling: no fetches
        int2 v = int2(blocks[iid]) * B + int2(int(vid) % (B + 1), int(vid) / (B + 1));
        float2 p = rp.pos0 + float2(v) * rp.gS;
        VOut o;
        o.world = float3(p, -dot(p, p) * g.eye.w);
        o.pos = g.viewProj * float4(o.world, 1.0f);
        o.s0 = float2(v); o.s1 = float2(v); o.w0 = 0; o.w1 = 0;
        return o;
    }
    int2 local = int2(int(vid) % (B + 1), int(vid) / (B + 1));
    int2 v = int2(blocks[iid]) * B + local;           // ring-ℓ vertex coords, >= 0

    float2 p = rp.pos0 + float2(v) * rp.gS;
    float dinf = max(abs(p.x), abs(p.y));
    float alpha = saturate((dinf - rp.alphaStart) / rp.alphaWidth);

    Fetch f0 = fetch(dataTex, metaTex, g, rp, 0, rp.lv[0].ringOrigin + v * rp.K);
    float h0 = heightS(f0.de, rp.lv[0].spOverS, g);

    // ring ℓ+1's triangle surface at v (coarse vertices at even v, diagonal (0,0)-(1,1))
    int2 a = (v / 2) * 2;
    int2 r = v - a;
    float va = 0.0f, vb = 0.0f;
    float t;
    if (r.x == 0 && r.y == 0) { t = coarse1(dataTex, metaTex, g, rp, a, va); vb = va; }
    else {
        int2 b = a + int2(r.x != 0 ? 2 : 0, r.y != 0 ? 2 : 0);
        t = 0.5f * (coarse1(dataTex, metaTex, g, rp, a, va) + coarse1(dataTex, metaTex, g, rp, b, vb));
    }
    float w0 = f0.valid * (1.0f - alpha);
    float h = mix(t, h0, w0);

    float d2 = dot(p, p);
    float3 world = float3(p, h - d2 * g.eye.w);
    VOut o;
    o.pos = g.viewProj * float4(world, 1.0f);
    o.world = world;
    o.s0 = float2(rp.lv[0].ringOrigin + v * rp.K);
    o.s1 = float2(rp.lv[1].ringOrigin) + float2(v * rp.K) * 0.5f;
    o.w0 = w0;
    o.w1 = min(va, vb);
    return o;
}

// Filtered height (S units) and DE (in samples) of level k around data-local coords s.
struct Grad { float3 n; float de; };

static inline Grad gradAt(texture2d_array<float, access::sample> dataTex, sampler smp,
                          constant Globals& g, constant LevelRef& L, float2 s)
{
    const float invN = g.misc.x;
    float2 base = (float2(L.dataMod) + s + 0.5f) * invN;
    float e = invN;
    float dxp = dataTex.sample(smp, base + float2(e, 0), L.slot).x;
    float dxm = dataTex.sample(smp, base - float2(e, 0), L.slot).x;
    float dyp = dataTex.sample(smp, base + float2(0, e), L.slot).x;
    float dym = dataTex.sample(smp, base - float2(0, e), L.slot).x;
    float hxp = heightS(dxp, L.spOverS, g), hxm = heightS(dxm, L.spOverS, g);
    float hyp = heightS(dyp, L.spOverS, g), hym = heightS(dym, L.spOverS, g);
    float step2 = 2.0f * L.spOverS;
    Grad r;
    r.n = normalize(float3(-(hxp - hxm) / step2, -(hyp - hym) / step2, 1.0f));
    r.de = 0.25f * (dxp + dxm + dyp + dym);
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

fragment float4 terrainFragment(VOut in                                    [[stage_in]],
                                constant Globals& g                        [[buffer(0)]],
                                constant RingParams& rp                    [[buffer(1)]],
                                texture2d_array<float, access::sample> dataTex [[texture(0)]])
{
    constexpr sampler smp(filter::linear, address::repeat);
    float3 sun = g.sun.xyz;
    if (g.misc.w == 2.0f) return float4(0.5f + 0.5f * normalize(cross(dfdx(in.world), dfdy(in.world))), 1.0f);

    // geometric normal (faceted) as the last fallback
    float3 ng = normalize(cross(dfdx(in.world), dfdy(in.world)));
    if (ng.z < 0.0f) ng = -ng;

    // pixel footprint in level-ℓ samples: fade out detail the pixel cannot resolve
    float fp = max(length(dfdx(in.s0)), length(dfdy(in.s0)));
    float w0 = in.w0 * saturate(2.0f - fp);
    float w1 = in.w1 * saturate(2.0f - 0.5f * fp);

    Grad g0 = gradAt(dataTex, smp, g, rp.lv[0], in.s0);
    Grad g1 = gradAt(dataTex, smp, g, rp.lv[1], in.s1);
    float3 n = normalize(mix(mix(ng, g1.n, w1), g0.n, w0));
    // DE / S at the fragment (for colour); coarse fallback: the height inverted
    // log2(DE/S) at the fragment (strata colour) and lake coverage
    float l1 = g1.de + log2(rp.lv[1].spOverS);
    float l0 = g0.de + log2(rp.lv[0].spOverS);
    float lakeV = mix(g1.de, g0.de, in.w0);
    float log2deS = mix(l1, l0, in.w0);
    float deS = lakeV > -60.0f ? exp2(log2deS) : 0.0f;

    float3 viewV = in.world - g.eye.xyz;
    float dist = length(viewV);
    float3 vd = viewV / dist;

    float3 col;
    float lake = (in.w0 + in.w1 > 0.0f) ? smoothstep(-40.0f, -80.0f, lakeV) : 0.0f;
    float3 albedoOnly = 0.0f, lightOnly = 0.0f;
    if (g.misc.z == 1.0f) {
        // debug: level / validity colours
        col = float3(in.w0, in.w1, 0.2f + 0.6f * float(rp.lv[0].slot % 3) / 2.0f);
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
        float3 cl[2];
        float oc[2];
        float sg[2] = { g.shade.z, g.shade.w };
        for (int i = 0; i < 2; i++) {
            float deR = legacy ? deS * exp2(-g.shade.x) * 10.0f : deS / sg[i];   // DE / σ_layer
            float3 c = c0;
            float low = saturate(1.0f - deR / 3.0f);
            c = mix(c, float3(0.16, 0.27, 0.10), 0.8f * flat * low);
            float high = saturate((deR - 30.0f) / 100.0f);
            c = mix(c, float3(0.78, 0.74, 0.66), 0.5f * flat * high);
            cl[i] = mix(c, c * float3(0.75, 0.6, 0.55), steep);
            oc[i] = mix(0.55f, 1.0f, saturate(deR * 0.8f));
        }
        float3 c = mix(cl[0], cl[1], g.shade2.x);
        float occl = mix(oc[0], oc[1], g.shade2.x);

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
    if (g.misc.w == 4.0f) return float4(albedoOnly, hill);          // measurement: colour only
    if (g.misc.w == 5.0f) return float4(lightOnly, hill);           // measurement: lighting only
    float fd = 1.0f - exp(-dist / g.sun.w);
    col = mix(col, fogColor(vd, sun), saturate(fd));
    return float4(tonemap(col, in.pos.xy), hill);
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
    return float4(tonemap(c, in.pos.xy), 0.0f);
}
