// The Julia companion panel (kb: plans/f1-julia-view, formulas/julia): the filled Julia set of
// z <- z^2 + k, iterated directly in float32 from z0 = pixel (shallow; no perturbation). A
// library of its own (compiled through `GpuBackend::library_pipelines` when the panel is first
// shown), so the Mandelbrot pipelines and their source never change for it.
//
// - juliaField: one sample per pixel centre, the iteration texel (n, smooth offset) as the
//   Mandelbrot kernels store it (ITER_INTERIOR = -1 for "did not escape"); the GPU-vs-CPU test
//   compares it with `almondbread_core::julia::cpu_field`.
// - juliaColor: 2x2 samples per pixel, coloured with the main view's palette and colour density,
//   averaged in linear light and written sRGB-encoded to the panel's BGRA8 texture (the overlay
//   kernel reads it like a mini-map level), the whole picture or a band of rows at a time.

#include <metal_stdlib>
using namespace metal;

struct JuliaParams {
    float4 interior;     // linear RGB of the flat interior
    float2 k;            // the Julia parameter
    float2 cycle;        // a point of k's attracting cycle (period > 0)
    float  scale;        // z units per pixel
    float  bailout2;     // squared escape radius
    float  density;      // palette cycles per iteration (or per log unit)
    float  offset;       // palette phase, 0..1
    float  cycleEps2;    // squared distance to `cycle` that counts as captured
    uint   width;
    uint   height;
    uint   maxIter;
    uint   logScale;     // 1: colour by log(mu + 1)
    uint   smoothColor;  // 0: banded (integer) count
    uint   paletteSize;  // entries of the palette table (linear RGBA)
    uint   samples;      // 1 or 4 (2x2) per pixel
    uint   period;       // of k's attracting cycle; 0: none known (no capture test)
    float  invLogLambda; // 1 / log2 |lambda| of the cycle (0: superattracting)
    uint   shadeInterior;// 1: captured interior by the time to reach the cycle, else flat
    uint   row0;         // juliaColor: first row of this band (a picture in bands)
};

constant float JULIA_INTERIOR = -1.0f;

struct JuliaSample {
    float n;      // escape iteration, or JULIA_INTERIOR
    float s;      // smooth offset (escaped) or captured-time fraction (interior)
    float cap;    // interior: iteration at which the orbit reached the cycle (-1: never)
};

static inline JuliaSample juliaSample(float2 z, constant JuliaParams& p) {
    // With k's attracting cycle known, an orbit that comes within cycleEps of it is interior for
    // sure and stops there (the interior no longer costs maxIter steps per sample).
    const bool watch = p.period != 0;
    float r2 = dot(z, z);
    uint n = 0;
    while (n < p.maxIter && r2 <= p.bailout2) {
        z = float2(z.x * z.x - z.y * z.y, 2.0f * z.x * z.y) + p.k;
        n++;
        r2 = dot(z, z);
        if (watch) {
            float2 d = z - p.cycle;
            float d2 = dot(d, d);
            if (d2 < p.cycleEps2) {
                // Captured by the attracting cycle: interior for sure. A smooth fraction from
                // how far inside the capture disc the orbit landed (|d| shrinks by |lambda| per
                // period near the cycle).
                float f = p.invLogLambda != 0.0f
                        ? saturate(0.5f * log2(max(d2, 1e-30f) / p.cycleEps2) * p.invLogLambda)
                        : 0.0f;
                return JuliaSample{JULIA_INTERIOR, 0.0f, float(n) - f * float(p.period)};
            }
        }
    }
    if (r2 > p.bailout2) {
        return JuliaSample{float(n), 1.0f - log2(0.5f * log(r2)), -1.0f};
    }
    return JuliaSample{JULIA_INTERIOR, 0.0f, -1.0f};
}

static inline float2 pixelZ(float2 pix, constant JuliaParams& p) {
    return float2((pix.x - 0.5f * float(p.width)) * p.scale, (0.5f * float(p.height) - pix.y) * p.scale);
}

kernel void juliaField(texture2d<float, access::write> out [[texture(0)]],
                       constant JuliaParams& p             [[buffer(0)]],
                       uint2 gid                           [[thread_position_in_grid]])
{
    if (gid.x >= p.width || gid.y >= p.height) return;
    JuliaSample s = juliaSample(pixelZ(float2(gid) + 0.5f, p), p);
    out.write(float4(s.n, s.n == JULIA_INTERIOR ? 0.0f : s.s, 0.0f, 0.0f), gid);
}

static inline float3 juliaPalette(float t, constant JuliaParams& p, device const float4* pal) {
    float x = fract(t) * float(p.paletteSize) - 0.5f;
    float fl = floor(x);
    uint i0 = uint(int(fl) + int(p.paletteSize)) % p.paletteSize;
    uint i1 = (i0 + 1) % p.paletteSize;
    return mix(pal[i0].rgb, pal[i1].rgb, x - fl);
}

static inline float3 juliaShade(JuliaSample s, constant JuliaParams& p, device const float4* pal) {
    if (s.n == JULIA_INTERIOR) {
        if (s.cap < 0.0f || p.shadeInterior == 0) return p.interior.rgb;
        // Bands of the time to reach the cycle, one palette step per period, darkened so the
        // interior reads as inside.
        float t = s.cap / float(max(p.period, 1u)) * 0.07f;
        return juliaPalette(t + p.offset, p, pal) * 0.45f;
    }
    float t;
    if (p.logScale != 0) {
        float mu = s.n + (p.smoothColor != 0 ? s.s : 0.0f);
        t = log(max(mu, 0.0f) + 1.0f) * p.density;
    } else {
        t = fract(s.n * p.density) + (p.smoothColor != 0 ? s.s * p.density : 0.0f);
    }
    return juliaPalette(t + p.offset, p, pal);
}

static inline float3 juliaToSRGB(float3 c) {
    c = saturate(c);
    return select(1.055f * pow(c, float3(1.0f / 2.4f)) - 0.055f, 12.92f * c, c <= 0.0031308f);
}

kernel void juliaColor(texture2d<float, access::write> out [[texture(0)]],
                       constant JuliaParams& p             [[buffer(0)]],
                       device const float4* pal            [[buffer(1)]],
                       uint2 tid                           [[thread_position_in_grid]])
{
    const uint2 gid = uint2(tid.x, tid.y + p.row0);
    if (gid.x >= p.width || gid.y >= p.height) return;
    float3 acc = 0.0f;
    if (p.samples >= 4) {
        // 2x2 rotated grid.
        const float2 o[4] = {float2(0.375f, 0.125f), float2(0.875f, 0.375f),
                             float2(0.125f, 0.625f), float2(0.625f, 0.875f)};
        for (uint i = 0; i < 4; i++) acc += juliaShade(juliaSample(pixelZ(float2(gid) + o[i], p), p), p, pal);
        acc *= 0.25f;
    } else {
        acc = juliaShade(juliaSample(pixelZ(float2(gid) + 0.5f, p), p), p, pal);
    }
    out.write(float4(juliaToSRGB(acc), 1.0f), gid);
}

// The struct's size as this compiler lays it out (the Rust mirror's layout test).
kernel void juliaLayout(device uint* out [[buffer(0)]], uint gid [[thread_position_in_grid]])
{
    if (gid == 0) out[0] = uint(sizeof(JuliaParams));
}
