// The parameter lab (F10, kb: plans/f10-parameter-lab): z <- z^p + c with three complex knobs,
// the start z0, the parameter c and the exponent p; the pixel is one of them (the plane), the
// other two are fixed. Direct float32 iteration (shallow by design: the zoom stops at the
// precision limit, `almondbread_core::lab::min_pixel`). A library of its own, compiled when the
// lab is first entered, so the Mandelbrot pipelines and their source never change for it.
//
// z^p = exp(p Log z) on the principal branch (arg in (-pi, pi]): a non-integer p shows a seam
// along the negative real axis of z, as the formula has. A small positive integer p (outside the
// p plane) takes repeated multiplication. 0^p is 0 for Re p > 0, else infinite (escaped).
//
// - labField: the main view's iteration texels (n, smooth offset; ITER_INTERIOR = -1) and
//   distance estimates (DE in texels, normal angle), as the Mandelbrot kernels store them, so
//   the main colour pass (`colorize` in mandelbrot.metal) colours them with every look. A band
//   of rows at a time.
// - labPanel: a small coloured picture of one plane (1 or 2x2 samples per pixel, the main
//   view's palette and density), sRGB into a BGRA8 texture the overlay kernel reads like a
//   mini-map level (the panels, drawn as the Julia panel is).
// - labLayout: the struct's size (the Rust mirror's layout test).
//
// Smooth count: s = (1 - log2(ln|z|) / log2(k)) w with k = max(Re p, 1.5) and the weight
// w = clamp(2 (Re p - 1), 0, 1): near infinity ln|z^p| = Re p ln|z| - Im p arg z, so the escape
// is double-exponential with base Re p (the Mandelbrot kernels' 1 - log2(ln|z|) is k = 2); towards
// Re p = 1 it is not, and the offset fades to the plain count (no seam in the p plane there).
//
// Compiled with fast math, which assumes no NaN or infinity: overflow and 0^p do produce them
// here, so the checks look at the float's bits (`bad`), which the compiler cannot fold away.

#include <metal_stdlib>
using namespace metal;

struct LabParams {
    float4 interior;     // panel: linear RGB of the interior
    float2 z0;           // the fixed knobs (the plane's own is replaced by the pixel's value)
    float2 c;
    float2 p;
    float2 centerHi;     // the view centre, float-float (hi + lo)
    float2 centerLo;
    float2 stepX;        // plane units per texel along x (rotation included)
    float2 stepY;        // ... along y (y down)
    uint   plane;        // 0: pixel = c, 1: pixel = z0, 2: pixel = p
    uint   width;        // texels
    uint   height;
    uint   maxIter;
    uint   intPower;     // p as a small integer 1..16 (repeated multiplication); 0: exp(p Log z)
    uint   row0;         // first row of this band
    uint   samples;      // labPanel: 1 or 4 (2x2) per pixel
    uint   paletteSize;
    float  bailout2;
    float  cycleEps2;    // squared distance to the saved z that counts as a cycle (interior)
    float  density;      // palette cycles per iteration (or per log unit)
    float  offset;       // palette phase, 0..1
    uint   logScale;
    uint   smoothColor;
    float  invTexel;     // texels per plane unit (DE in texels)
    uint   pad0;
    uint   pad1;
    uint   pad2;
};

constant float LAB_INTERIOR = -1.0f;

// NaN or infinity, by the bits (fast math would fold isfinite away).
static inline bool bad(float x) {
    return (as_type<uint>(x) & 0x7f800000u) == 0x7f800000u;
}

static inline float2 labMul(float2 a, float2 b) {
    return float2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// z^p; `logz` gets Log z when the general path ran (else it is left alone).
static inline float2 labPow(float2 z, float2 p, uint ip, float r2, thread float2& logz) {
    if (ip != 0) {
        float2 r = z;
        for (uint k = 1; k < ip; k++) r = labMul(r, z);
        return r;
    }
    if (r2 == 0.0f) return p.x > 0.0f ? float2(0.0f) : float2(1e30f, 0.0f);
    logz = float2(0.5f * log(r2), atan2(z.y, z.x));
    float2 w = labMul(p, logz);
    return exp(w.x) * float2(cos(w.y), sin(w.y));
}

struct LabSample {
    float n;      // escape iteration, or LAB_INTERIOR
    float s;      // smooth offset (escaped); interior: 1 cycle found, 0 the limit
    float de;     // distance estimate in texels (escaped, with DE)
    float angle;  // normal angle (direction of z / dz)
};

static inline LabSample labSample(float2 v, constant LabParams& q, bool wantDE) {
    float2 z0 = q.z0, c = q.c, p = q.p;
    uint ip = q.intPower;
    if (q.plane == 0) c = v;
    else if (q.plane == 1) z0 = v;
    else { p = v; ip = 0; }
    float2 z = z0;
    // d = dz / d(pixel value): z0 plane starts at 1, the others at 0.
    float2 d = q.plane == 1 ? float2(1.0f, 0.0f) : float2(0.0f);
    float r2 = dot(z, z);
    uint n = 0;
    float2 saved = z;
    uint nextSave = 1;
    uint far = 0;
    while (n < q.maxIter && r2 <= q.bailout2) {
        float2 logz = float2(0.0f);
        float2 zp = labPow(z, p, ip, r2, logz);
        if (wantDE) {
            // d(z^p) = p z^(p-1) d (+ z^p Log z in the p plane); + 1 in the c plane.
            float2 zinv = r2 > 0.0f ? float2(z.x, -z.y) / r2 : float2(0.0f);
            float2 nd = labMul(labMul(p, labMul(zp, zinv)), d);
            if (q.plane == 2) {
                if (ip != 0 || r2 == 0.0f) logz = float2(0.5f * log(max(r2, 1e-38f)), atan2(z.y, z.x));
                nd += labMul(zp, logz);
            } else if (q.plane == 0) {
                nd.x += 1.0f;
            }
            d = nd;
        }
        z = zp + c;
        n++;
        if (bad(z.x) || bad(z.y)) {
            // Overflow or 0^p with Re p <= 0: escaped, infinitely far.
            far = 2;
            break;
        }
        r2 = dot(z, z);
        if (bad(r2)) {
            // |z| past 2^64 (a large Re p): escaped, |z| finite but its square is not.
            far = 1;
            break;
        }
        float2 dz = z - saved;
        if (dot(dz, dz) < q.cycleEps2) {
            return LabSample{LAB_INTERIOR, 1.0f, 0.0f, 0.0f};
        }
        if (n == nextSave) {
            saved = z;
            nextSave <<= 1;
        }
    }
    if (far == 0 && r2 <= q.bailout2) return LabSample{LAB_INTERIOR, 0.0f, 0.0f, 0.0f};
    // ln|z|, capped at 80 (as the f64 reference): from the larger component where |z|^2 overflows.
    float lnz = 80.0f;
    if (far == 0) {
        lnz = 0.5f * log(r2);
    } else if (far == 1) {
        float a = max(abs(z.x), abs(z.y)), b = min(abs(z.x), abs(z.y)) / a;
        lnz = log(a) + 0.5f * log(1.0f + b * b);
    }
    lnz = min(lnz, 80.0f);
    float k = max(p.x, 1.5f);
    float s = (1.0f - log2(lnz) / log2(k)) * saturate(2.0f * (p.x - 1.0f));
    if (bad(s)) s = 0.0f;
    float de = 0.0f, angle = 0.0f;
    if (wantDE) {
        float dl = length(d);
        if (far == 0 && !bad(dl) && dl > 0.0f) {
            de = 2.0f * sqrt(r2) * lnz / dl * q.invTexel;
            if (bad(de)) de = 0.0f;
            float2 u = labMul(z, float2(d.x, -d.y));
            angle = atan2(u.y, u.x);
        }
    }
    return LabSample{float(n), s, min(de, 65000.0f), angle};
}

// The plane's value at texel position `pix` (texel centres at i + 0.5).
static inline float2 labValue(float2 pix, constant LabParams& q) {
    float2 off = (pix.x - 0.5f * float(q.width)) * q.stepX + (pix.y - 0.5f * float(q.height)) * q.stepY;
    return q.centerHi + (q.centerLo + off);
}

kernel void labField(texture2d<float, access::write> iters [[texture(0)]],
                     texture2d<float, access::write> des   [[texture(1)]],
                     constant LabParams& q                 [[buffer(0)]],
                     uint2 tid                             [[thread_position_in_grid]])
{
    const uint2 gid = uint2(tid.x, tid.y + q.row0);
    if (gid.x >= q.width || gid.y >= q.height) return;
    LabSample s = labSample(labValue(float2(gid) + 0.5f, q), q, true);
    iters.write(float4(s.n, s.s, 0.0f, 0.0f), gid);
    des.write(float4(s.de, s.angle, 0.0f, 0.0f), gid);
}

static inline float3 labPalette(float t, constant LabParams& q, device const float4* pal) {
    float x = fract(t) * float(q.paletteSize) - 0.5f;
    float fl = floor(x);
    uint i0 = uint(int(fl) + int(q.paletteSize)) % q.paletteSize;
    uint i1 = (i0 + 1) % q.paletteSize;
    return mix(pal[i0].rgb, pal[i1].rgb, x - fl);
}

static inline float3 labShade(LabSample s, constant LabParams& q, device const float4* pal) {
    if (s.n == LAB_INTERIOR) return q.interior.rgb;
    float t;
    if (q.logScale != 0) {
        float mu = s.n + (q.smoothColor != 0 ? s.s : 0.0f);
        t = log(max(mu, 0.0f) + 1.0f) * q.density;
    } else {
        t = fract(s.n * q.density) + (q.smoothColor != 0 ? s.s * q.density : 0.0f);
    }
    return labPalette(t + q.offset, q, pal);
}

static inline float3 labToSRGB(float3 c) {
    c = saturate(c);
    return select(1.055f * pow(c, float3(1.0f / 2.4f)) - 0.055f, 12.92f * c, c <= 0.0031308f);
}

kernel void labPanel(texture2d<float, access::write> out [[texture(0)]],
                     constant LabParams& q             [[buffer(0)]],
                     device const float4* pal          [[buffer(1)]],
                     uint2 tid                         [[thread_position_in_grid]])
{
    const uint2 gid = uint2(tid.x, tid.y + q.row0);
    if (gid.x >= q.width || gid.y >= q.height) return;
    float3 acc = 0.0f;
    if (q.samples >= 4) {
        // 2x2 rotated grid.
        const float2 o[4] = {float2(0.375f, 0.125f), float2(0.875f, 0.375f),
                             float2(0.125f, 0.625f), float2(0.625f, 0.875f)};
        for (uint i = 0; i < 4; i++) acc += labShade(labSample(labValue(float2(gid) + o[i], q), q, false), q, pal);
        acc *= 0.25f;
    } else {
        acc = labShade(labSample(labValue(float2(gid) + 0.5f, q), q, false), q, pal);
    }
    out.write(float4(labToSRGB(acc), 1.0f), gid);
}

kernel void labLayout(device uint* out [[buffer(0)]], uint gid [[thread_position_in_grid]])
{
    if (gid == 0) out[0] = uint(sizeof(LabParams));
}
