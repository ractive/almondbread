// almondbread M7: float-float (2 x f32) arithmetic with a shared exponent (kb: techniques/double-float).
//
// Its own library, compiled with MTLMathMode.safe (no reassociation, IEEE rounding), never
// part of the fast float32 library in mandelbrot.metal: the error-free transforms below depend
// on exact rounding events that a fast-math compiler may legally erase, and the wider state
// would raise the register count of the float32 kernels (kb: platform/apple-gpu-architecture).
// `ffKnownAnswer` proves at creation time that the compiled code keeps them (src/floatfloat.rs).
//
// Kernels:
// - ffKnownAnswer: TwoSum / QuickTwoSum / TwoProd / ff mul / a 1000-step ff orbit on inputs
//   read from a buffer (no constant folding), checked on the CPU.
// - ffIterate: perturbation of a list of pixels against a float-float reference, deltas in
//   "fx" (complex float-float mantissa + int exponent, any depth), rebasing (kb:
//   techniques/rebasing), no BLA, no interior test, resumable in budgeted chunks. The pixel
//   list is the hook for a repair pass that re-iterates flagged pixels only (not wired: M7
//   found no float32 artefacts to repair, kb: questions/float32-error-long-orbits).
// - ffBench{F32,FF,FX}: z = z^2 + c throughput of each number type.

#include <metal_stdlib>
using namespace metal;

// ---- error-free transforms (Dekker/Knuth; Thall's GPU formulation) -------------------------

static inline float2 twoSum(float a, float b) {
    float s = a + b;
    float v = s - a;
    float e = (a - (s - v)) + (b - v);
    return float2(s, e);
}

static inline float2 quickTwoSum(float a, float b) {   // |a| >= |b|
    float s = a + b;
    float e = b - (s - a);
    return float2(s, e);
}

static inline float2 twoProd(float a, float b) {        // fma: exact low part
    float p = a * b;
    float e = fma(a, b, -p);
    return float2(p, e);
}

// ---- float-float (x = hi, y = lo) ----------------------------------------------------------

static inline float2 ffAdd(float2 x, float2 y) {
    float2 s = twoSum(x.x, y.x);
    float2 t = twoSum(x.y, y.y);
    float e = s.y + t.x;
    s = quickTwoSum(s.x, e);
    e = s.y + t.y;
    return quickTwoSum(s.x, e);
}

static inline float2 ffMul(float2 x, float2 y) {
    float2 p = twoProd(x.x, y.x);
    float e = p.y + (x.x * y.y + x.y * y.x);
    return quickTwoSum(p.x, e);
}

static inline float2 ffNeg(float2 x) { return float2(-x.x, -x.y); }
static inline float2 ffScale(float2 x, int k) { return float2(ldexp(x.x, k), ldexp(x.y, k)); }

// ---- fx: complex float-float mantissa with one exponent: (re + i im) 2^e ----------------------

constant int FX_ZERO = -(1 << 28);

struct FX {
    float2 re;
    float2 im;
    int e;
};

struct FxRef {          // buffer layout (24 bytes), Rust `floatfloat::FxRef`
    float2 re;
    float2 im;
    int e;
    int pad;
};

static inline FX fxLoad(FxRef r) { FX a; a.re = r.re; a.im = r.im; a.e = r.e; return a; }

static inline FX fxZero() { FX a; a.re = 0.0f; a.im = 0.0f; a.e = FX_ZERO; return a; }

static inline bool fxIsZero(FX a) { return a.e == FX_ZERO; }

// Mantissa max(|re.hi|, |im.hi|) into [0.5, 1).
static inline FX fxNorm(FX a) {
    float m = max(abs(a.re.x), abs(a.im.x));
    if (m == 0.0f || a.e == FX_ZERO) return fxZero();
    int k;
    frexp(m, k);
    a.re = ffScale(a.re, -k);
    a.im = ffScale(a.im, -k);
    a.e += k;
    return a;
}

static inline FX fxAdd(FX a, FX b) {
    if (fxIsZero(a)) return b;
    if (fxIsZero(b)) return a;
    int d = a.e - b.e;
    if (d < 0) { FX t = a; a = b; b = t; d = -d; }
    if (d > 60) return a;                     // b below a's 48 bits
    FX r;
    r.re = ffAdd(a.re, ffScale(b.re, -d));
    r.im = ffAdd(a.im, ffScale(b.im, -d));
    r.e = a.e;
    return fxNorm(r);
}

static inline FX fxMul(FX a, FX b) {
    if (fxIsZero(a) || fxIsZero(b)) return fxZero();
    FX r;
    r.re = ffAdd(ffMul(a.re, b.re), ffNeg(ffMul(a.im, b.im)));
    r.im = ffAdd(ffMul(a.re, b.im), ffMul(a.im, b.re));
    r.e = a.e + b.e;
    return fxNorm(r);
}

static inline FX fxTwice(FX a) { if (!fxIsZero(a)) a.e += 1; return a; }

// |a|^2 < |b|^2 (approximate: decides rebasing only).
static inline bool fxLess(FX a, FX b) {
    if (fxIsZero(b)) return false;
    if (fxIsZero(a)) return true;
    int d = a.e - b.e;
    if (d > 1) return false;
    if (d < -1) return true;
    float ma = a.re.x * a.re.x + a.im.x * a.im.x;
    float mb = b.re.x * b.re.x + b.im.x * b.im.x;
    return ldexp(ma, 2 * d) < mb;
}

// |a|^2 as float (inf above 2^120).
static inline float fxMag2(FX a) {
    if (fxIsZero(a)) return 0.0f;
    if (a.e > 60) return INFINITY;
    float m = a.re.x * a.re.x + a.im.x * a.im.x;
    return ldexp(m, 2 * max(a.e, -70));
}

// ---- known-answer test ----------------------------------------------------------------------

kernel void ffKnownAnswer(device const float* in  [[buffer(0)]],
                          device float* out       [[buffer(1)]],
                          uint gid                [[thread_position_in_grid]]) {
    if (gid != 0) return;
    float2 r = twoSum(in[0], in[1]);                       // 1 + 1e-8: lo must keep 1e-8
    out[0] = r.x; out[1] = r.y;
    r = quickTwoSum(in[0], in[2]);                         // 1 + 2^-30
    out[2] = r.x; out[3] = r.y;
    r = twoProd(in[3], in[3]);                             // (1 + 2^-12)^2: lo = 2^-24
    out[4] = r.x; out[5] = r.y;
    r = ffMul(float2(in[4], in[5]), float2(in[6], 0.0f));  // ff(1/3) * 3 ~ 1 to 2^-44
    out[6] = r.x; out[7] = r.y;
    // 1000 steps of z = z^2 + c in ff and in float32, c in the main cardioid (bounded,
    // attracting: errors do not grow), compared with f64 on the CPU.
    float2 cr = float2(in[7], in[8]), ci = float2(in[9], in[10]);
    float2 zr = 0.0f, zi = 0.0f;
    float fr = 0.0f, fi = 0.0f, fcr = in[7], fci = in[9];
    for (int k = 0; k < 1000; k++) {
        float2 t = ffAdd(ffAdd(ffMul(zr, zr), ffNeg(ffMul(zi, zi))), cr);
        zi = ffAdd(ffScale(ffMul(zr, zi), 1), ci);
        zr = t;
        float u = fr * fr - fi * fi + fcr;
        fi = 2.0f * fr * fi + fci;
        fr = u;
    }
    out[8] = zr.x; out[9] = zr.y; out[10] = zi.x; out[11] = zi.y;
    out[12] = fr; out[13] = fi;
    // fx: (2^-1000-scale delta) * itself + c: exponents beyond float range.
    FX a; a.re = float2(in[11], 0.0f); a.im = float2(in[12], 0.0f); a.e = -1000;
    FX p = fxAdd(fxMul(a, a), a);                          // a^2 + a ~ a (a^2 is 2^-1000 smaller)
    out[14] = p.re.x; out[15] = p.re.y; out[16] = p.im.x; out[17] = p.im.y; out[18] = float(p.e);
}

// ---- perturbation with fx deltas --------------------------------------------------------------

struct FfParams {       // Rust `floatfloat::FfParams`
    uint count;         // list entries
    uint maxIter;
    uint budget;        // iterations per pixel per dispatch
    uint last;          // index of the reference's last point
    float bailout2;
    uint pad0, pad1, pad2;
};

struct FfState {        // 32 bytes, Rust `floatfloat::FfState`
    float2 dre;
    float2 dim;
    int de;
    uint m;
    uint n;
    uint done;          // 0 running, 1 finished
};

kernel void ffIterate(constant FfParams& p        [[buffer(0)]],
                      device const FxRef* refs     [[buffer(1)]],
                      device const FxRef* dcs      [[buffer(2)]],
                      device FfState* states       [[buffer(3)]],
                      device float2* out           [[buffer(4)]],
                      device atomic_uint* active   [[buffer(5)]],
                      uint gid                     [[thread_position_in_grid]]) {
    if (gid >= p.count) return;
    FfState st = states[gid];
    if (st.done != 0) return;
    FX dc = fxLoad(dcs[gid]);
    FX d; d.re = st.dre; d.im = st.dim; d.e = st.de;
    uint m = st.m, n = st.n;
    for (uint k = 0; k < p.budget; k++) {
        FX zm = fxLoad(refs[m]);
        FX zf = fxAdd(zm, d);
        float r2 = fxMag2(zf);
        if (r2 > p.bailout2) {
            out[gid] = float2(float(n), 1.0f - log2(0.5f * log(r2)));
            states[gid].done = 1;
            return;
        }
        if (m == p.last || (m != 0 && fxLess(zf, d))) {
            d = zf;
            m = 0;
            zm = fxZero();
        }
        if (n >= p.maxIter) {
            out[gid] = float2(-1.0f, 0.0f);            // ITER_INTERIOR (the limit)
            states[gid].done = 1;
            return;
        }
        d = fxAdd(fxMul(fxAdd(fxTwice(zm), d), d), dc);
        m++;
        n++;
    }
    st.dre = d.re; st.dim = d.im; st.de = d.e; st.m = m; st.n = n;
    states[gid] = st;
    atomic_fetch_add_explicit(active, 1u, memory_order_relaxed);
}

// ---- throughput of each number type (kb: questions/double-float-speed-on-m1) ----------------

struct BenchParams { uint steps; float cre; float cim; float spread; };

kernel void ffBenchF32(constant BenchParams& p [[buffer(0)]], device float* out [[buffer(1)]],
                       uint gid [[thread_position_in_grid]]) {
    float cr = p.cre + p.spread * float(gid & 1023u), ci = p.cim + p.spread * float(gid >> 10);
    float zr = 0.0f, zi = 0.0f;
    for (uint k = 0; k < p.steps; k++) {
        float t = zr * zr - zi * zi + cr;
        zi = 2.0f * zr * zi + ci;
        zr = t;
    }
    out[gid] = zr + zi;
}

kernel void ffBenchFF(constant BenchParams& p [[buffer(0)]], device float* out [[buffer(1)]],
                      uint gid [[thread_position_in_grid]]) {
    float2 cr = float2(p.cre + p.spread * float(gid & 1023u), 0.0f);
    float2 ci = float2(p.cim + p.spread * float(gid >> 10), 0.0f);
    float2 zr = 0.0f, zi = 0.0f;
    for (uint k = 0; k < p.steps; k++) {
        float2 t = ffAdd(ffAdd(ffMul(zr, zr), ffNeg(ffMul(zi, zi))), cr);
        zi = ffAdd(ffScale(ffMul(zr, zi), 1), ci);
        zr = t;
    }
    out[gid] = zr.x + zi.x;
}

kernel void ffBenchFX(constant BenchParams& p [[buffer(0)]], device float* out [[buffer(1)]],
                      uint gid [[thread_position_in_grid]]) {
    FX c; c.re = float2(p.cre + p.spread * float(gid & 1023u), 0.0f);
    c.im = float2(p.cim + p.spread * float(gid >> 10), 0.0f); c.e = 0;
    c = fxNorm(c);
    FX z = fxZero();
    for (uint k = 0; k < p.steps; k++) {
        z = fxAdd(fxMul(z, z), c);
    }
    out[gid] = z.re.x + float(z.e);
}
