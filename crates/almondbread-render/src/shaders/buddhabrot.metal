// Buddhabrot and Nebulabrot (milestone BB, kb: plans/buddhabrot, sources/superliminal-buddhabrot):
// the histogram of where the orbits of escaping c go. A library of its own (compiled through
// `GpuBackend::library_pipelines` when the look is first shown), so the Mandelbrot pipelines and
// their source never change for it.
//
// - buddhaSample: each thread is a persistent walker that runs `steps` iterations per dispatch
//   (its state survives in `walkers`): it draws a proposal c, iterates it up to the largest limit
//   (closed-form cardioid / bulb rejection, Brent's periodicity test), and if it escapes iterates
//   it again, depositing every in-view orbit point into the histogram (one atomic add per point
//   and channel; the Nebulabrot's channel k takes orbits escaping at n <= limits[k]); then the
//   next proposal. One iteration per loop step whatever the phase, so a SIMD group's threads do
//   not wait for the longest orbit among them (kb: plans/buddhabrot "As built").
//   Mode 0, uniform: proposals uniform over the region; each deposits its own orbit.
//   Mode 1, Metropolis–Hastings (Boswell's method, kb: plans/buddhabrot "As built"): one chain
//   per thread whose target is uniform over S, the c whose orbit hits the view; a proposal is a
//   uniform jump (probability pJump, always while the chain has no state in S) or a small
//   symmetric mutation; it is accepted iff it is in S. Every step deposits the chain's current
//   orbit once: an accepted proposal its own (during its deposit pass), a rejected one adds to the
//   current state's hold count, deposited with that weight (one more pass over that orbit) when
//   the state is left. Integer counts, unbiased: M steps stand for M / rho uniform samples, rho
//   the share of uniform proposals in S (counted).
// - buddhaFold (live mode): accum = decay * reprojected(accum) + this frame's counts, per bin.
// - buddhaTone: histogram (rest) or accum (live) -> tone curve -> colour; the shown image and the
//   history image (linear) the overlay pass composites from.
//
// The structs below must match `buddha.rs` byte for byte (the layout kernel and test).

#include <metal_stdlib>
using namespace metal;

struct BuddhaParams {
    uint4  limits;       // escape limit per channel (x, y, z); w: the largest
    float2 center;       // view centre (c units)
    float2 rot;          // (cos, sin) of the view angle
    float2 regionMin;    // uniform proposals: lower-left corner
    float2 regionSize;
    float  invPixel;     // bins per c unit
    float  pJump;        // MH: probability of a uniform proposal
    float  rMax;         // MH: largest mutation radius (c units)
    float  rSpan;        // MH: radii are rMax * 2^(-rSpan * u)
    uint   width;        // bins
    uint   height;
    uint   channels;
    uint   steps;        // iterations per thread (walker) in this dispatch
    uint   seed;
    uint   mode;         // 0 uniform, 1 Metropolis-Hastings
    uint   generation;   // view generation: a chain re-checks its state when it changes
    uint   slot;         // counter slot of this dispatch
    uint   threads;
    uint   deposit;      // 0: count only (throughput and contention measurements)
    uint   drain;        // 1: finish the orbits in progress (and deposit held states), no new
                         // proposals: a histogram that ends here has no orbit cut short
    uint   pad1;
};

// Per-dispatch counters (COUNTERS u32 per slot).
constant uint C_PROPOSALS = 0;
constant uint C_UNIFORM   = 1;   // uniform proposals
constant uint C_UNIFORM_S = 2;   // ... in S
constant uint C_STEPS     = 3;   // steps that deposited an orbit (uniform: = C_UNIFORM)
constant uint C_HITS      = 4;   // 4, 5, 6: hits per channel
constant uint C_ACCEPTED  = 7;
constant uint C_ITS_LO    = 8;
constant uint C_ITS_HI    = 9;
constant uint C_ESCAPED   = 10;
constant uint COUNTERS    = 16;

// A walker: the orbit it iterates, and (MH) its chain.
struct Walker {
    float2 c;        // the orbit being iterated
    float2 z;
    float2 saved;    // Brent's periodicity test
    float2 curC;     // MH: the chain's state
    float2 flushC;   // MH: a left state still to deposit with weight flushW
    uint   n;
    uint   check;
    uint   phase;    // PH_*
    uint   escN;     // escape iteration of the orbit in a deposit pass
    uint   k;        // in-view points of this pass so far
    uint   weight;   // deposit weight of this pass (0: count only)
    uint   curEsc;   // MH: escape iteration of curC
    uint   hold;     // MH: steps curC was kept since it was last deposited
    uint   inS;      // MH: curC's orbit hits the view
    uint   gen;      // the view generation inS was decided for
    uint   rng;      // 0: not started
    uint   jump;     // the proposal being tested was a uniform one
    uint   flushEsc;
    uint   flushW;
    uint   recheck;  // MH: decide inS again in the new view
    uint   anchor;   // MH: curC is a state of an earlier view (a place to search near)
};

constant uint PH_NEW = 0;      // draw the next proposal (or a pending flush / re-check)
constant uint PH_TEST = 1;     // escape test of the proposal
constant uint PH_DEPOSIT = 2;  // the proposal's orbit: deposit with weight 1, count k
constant uint PH_FLUSH = 3;    // a left MH state's orbit with its hold count as weight
constant uint PH_RECHECK = 4;  // the chain's state in a new view: count k only

constant float BAILOUT2 = 4.0f;
constant float PERIOD_EPS2 = 1e-12f;

static inline uint hashU(uint x) {
    x ^= x >> 16; x *= 0x7feb352du;
    x ^= x >> 15; x *= 0x846ca68bu;
    x ^= x >> 16;
    return x;
}

static inline uint pcg(thread uint& s) {
    s = s * 747796405u + 2891336453u;
    uint w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

static inline float unit(thread uint& s) {
    return float(pcg(s) >> 8) * (1.0f / 16777216.0f);
}

static inline bool inMainComponents(float2 c) {
    const float xq = c.x - 0.25f;
    const float q = xq * xq + c.y * c.y;
    if (q * (q + xq) <= 0.25f * c.y * c.y) return true;
    const float x1 = c.x + 1.0f;
    return x1 * x1 + c.y * c.y <= 0.0625f;
}

static inline bool inRegion(float2 c, constant BuddhaParams& p) {
    const float2 r = c - p.regionMin;
    return r.x >= 0.0f && r.y >= 0.0f && r.x < p.regionSize.x && r.y < p.regionSize.y;
}

kernel void buddhaSample(constant BuddhaParams& p   [[buffer(0)]],
                         device atomic_uint* hist   [[buffer(1)]],
                         device Walker* walkers     [[buffer(2)]],
                         device atomic_uint* counters [[buffer(3)]],
                         uint tid                   [[thread_position_in_grid]])
{
    if (tid >= p.threads) return;
    uint proposals = 0, uniform = 0, uniformS = 0, steps = 0, accepted = 0, escaped = 0;
    uint hits[3] = {0, 0, 0};
    const bool write = p.deposit != 0;
    const bool mh = p.mode == 1;
    const uint plane = p.width * p.height;
    const float2 mid = 0.5f * float2(p.width, p.height);
    Walker w = walkers[tid];
    if (w.rng == 0) {
        w = Walker{};
        w.rng = hashU(p.seed ^ hashU(tid * 0x9E3779B9u + 0x632BE5ABu)) | 1u;
        w.phase = PH_NEW;
        w.gen = p.generation;
    }
    uint rng = w.rng;
    if (w.gen != p.generation) {
        // Another view: a held state is deposited (into this view: live mode moves a little per
        // frame; the rest histogram restarts with fresh walkers), and the chain's state is
        // judged again once the orbit in progress is done (it goes on across views).
        w.gen = p.generation;
        if (w.inS != 0 && w.hold != 0 && w.flushW == 0) {
            w.flushC = w.curC;
            w.flushEsc = w.curEsc;
            w.flushW = w.hold;
            w.hold = 0;
        }
        w.recheck = w.inS;
    }
    float2 c = w.c, z = w.z, saved = w.saved;
    uint n = w.n, check = w.check, phase = w.phase, escN = w.escN, k = w.k, weight = w.weight;
    bool take[3];
    for (uint ch = 0; ch < 3; ch++) take[ch] = ch < p.channels && escN <= p.limits[ch];
    for (uint it = 0; it < p.steps; it++) {
        if (phase == PH_NEW) {
            z = 0.0f;
            saved = 0.0f;
            n = 0;
            k = 0;
            check = 8;
            if (mh && w.flushW != 0) {
                c = w.flushC;
                escN = w.flushEsc;
                weight = w.flushW;
                w.flushW = 0;
                phase = PH_FLUSH;
            } else if (p.drain != 0) {
                if (mh && w.inS != 0 && w.hold != 0) {
                    w.flushC = w.curC;
                    w.flushEsc = w.curEsc;
                    w.flushW = w.hold;
                    w.hold = 0;
                    continue;
                }
                break;
            } else if (mh && w.recheck != 0) {
                w.recheck = 0;
                c = w.curC;
                escN = w.curEsc;
                weight = 0;
                phase = PH_RECHECK;
            } else {
                proposals++;
                // Uniform jumps: always without a state; with one, pJump; searching near an
                // old view's state (no state in this view's S yet), half the time.
                const bool jump = !mh || (w.inS == 0 && w.anchor == 0)
                                  || unit(rng) < (w.inS != 0 ? p.pJump : 0.5f);
                if (jump) {
                    c = p.regionMin + p.regionSize * float2(unit(rng), unit(rng));
                    uniform++;
                } else {
                    const float r = p.rMax * exp2(-p.rSpan * unit(rng));
                    const float a = 6.2831853f * unit(rng);
                    c = w.curC + r * float2(cos(a), sin(a));
                }
                w.jump = jump ? 1u : 0u;
                if (!inRegion(c, p) || inMainComponents(c)) {
                    // Rejected at once (never escapes, or outside the region): an empty orbit.
                    if (mh && w.inS != 0) { w.hold++; steps++; }
                    if (!mh) steps++;
                    continue;
                }
                phase = PH_TEST;
            }
            for (uint ch = 0; ch < 3; ch++) take[ch] = ch < p.channels && escN <= p.limits[ch];
            continue;
        }
        z = float2(z.x * z.x - z.y * z.y, 2.0f * z.x * z.y) + c;
        n++;
        if (phase == PH_TEST) {
            if (dot(z, z) > BAILOUT2) {
                escaped++;
                escN = n;
                phase = PH_DEPOSIT;
                weight = 1;
                z = 0.0f;
                n = 0;
                k = 0;
                for (uint ch = 0; ch < 3; ch++) take[ch] = ch < p.channels && escN <= p.limits[ch];
                continue;
            }
            const float2 d = z - saved;
            if (dot(d, d) < PERIOD_EPS2 || n >= p.limits.w) {
                // Did not escape: an empty orbit.
                if (mh && w.inS != 0) { w.hold++; steps++; }
                if (!mh) steps++;
                phase = PH_NEW;
            } else if (n == check) {
                saved = z;
                check <<= 1;
            }
            continue;
        }
        // A pass over an escaping orbit: deposit (or count) its in-view points.
        const float2 dz = (z - p.center) * p.invPixel;
        const float x = dz.x * p.rot.x + dz.y * p.rot.y + mid.x;
        const float y = mid.y - (-dz.x * p.rot.y + dz.y * p.rot.x);
        if (x >= 0.0f && y >= 0.0f && x < float(p.width) && y < float(p.height)) {
            k++;
            if (weight != 0) {
                const uint b = uint(y) * p.width + uint(x);
                for (uint ch = 0; ch < 3; ch++) {
                    if (take[ch]) {
                        if (write) atomic_fetch_add_explicit(&hist[ch * plane + b], weight, memory_order_relaxed);
                        hits[ch] += weight;
                    }
                }
            }
        }
        if (n >= escN) {
            if (phase == PH_DEPOSIT) {
                if (!mh) {
                    steps++;
                    if (k != 0) uniformS++;
                } else {
                    if (w.jump != 0 && k != 0) uniformS++;
                    if (k != 0) {
                        if (w.inS != 0 && w.hold != 0) {
                            w.flushC = w.curC;
                            w.flushEsc = w.curEsc;
                            w.flushW = w.hold;
                        }
                        w.curC = c;
                        w.curEsc = escN;
                        w.hold = 0;
                        w.inS = 1;
                        w.anchor = 1;
                        accepted++;
                        steps++;
                    } else if (w.inS != 0) {
                        w.hold++;
                        steps++;
                    }
                }
            } else if (phase == PH_RECHECK) {
                w.inS = k != 0 ? 1u : 0u;
            }
            phase = PH_NEW;
        }
    }
    w.c = c;
    w.z = z;
    w.saved = saved;
    w.n = n;
    w.check = check;
    w.phase = phase;
    w.escN = escN;
    w.k = k;
    w.weight = weight;
    w.rng = rng;
    walkers[tid] = w;
    device atomic_uint* cnt = counters + p.slot * COUNTERS;
    atomic_fetch_add_explicit(&cnt[C_PROPOSALS], proposals, memory_order_relaxed);
    atomic_fetch_add_explicit(&cnt[C_UNIFORM], uniform, memory_order_relaxed);
    atomic_fetch_add_explicit(&cnt[C_UNIFORM_S], uniformS, memory_order_relaxed);
    atomic_fetch_add_explicit(&cnt[C_STEPS], steps, memory_order_relaxed);
    atomic_fetch_add_explicit(&cnt[C_ACCEPTED], accepted, memory_order_relaxed);
    atomic_fetch_add_explicit(&cnt[C_ESCAPED], escaped, memory_order_relaxed);
    for (uint ch = 0; ch < 3; ch++) {
        if (hits[ch] != 0) atomic_fetch_add_explicit(&cnt[C_HITS + ch], hits[ch], memory_order_relaxed);
    }
    const uint its = p.steps;
    const uint old = atomic_fetch_add_explicit(&cnt[C_ITS_LO], its, memory_order_relaxed);
    if (old + its < old) atomic_fetch_add_explicit(&cnt[C_ITS_HI], 1u, memory_order_relaxed);
}

// Adds a finished dispatch's counters (one slot) into the epoch totals (64-bit per counter).
// Runs after the sample dispatch in the same command buffer.
kernel void buddhaTotals(device const uint* slot [[buffer(0)]], device uint* totals [[buffer(1)]],
                         uint tid [[thread_position_in_grid]])
{
    if (tid != 0) return;
    for (uint i = 0; i < COUNTERS; i++) {
        const uint add = slot[i];
        const uint old = totals[2 * i];
        totals[2 * i] = old + add;
        if (old + add < old) totals[2 * i + 1] += 1u;
    }
}

// ---- MH chains between views (sequential Monte Carlo) ----
//
// A new view (or a warm-up stage): every chain judges its state again in the new view
// (buddhaRecheck: one pass over the state's orbit, counting in-view points), then the chains that
// fell out of S copy the state of a random chain that is still in (buddhaResample, from the
// snapshot the recheck wrote, so no chain reads another while it changes). If the old states were
// uniform over the old view's S and the new view lies inside the old one (so its S lies inside the
// old S), the survivors are uniform over the new S, and so are copies of them; the MH steps that
// follow make the copies diverge.

struct ChainSnap {
    float2 c;
    uint   esc;
    uint   inS;
};

kernel void buddhaRecheck(constant BuddhaParams& p [[buffer(0)]],
                          device Walker* walkers   [[buffer(2)]],
                          device ChainSnap* snap   [[buffer(4)]],
                          uint tid                 [[thread_position_in_grid]])
{
    if (tid >= p.threads) return;
    Walker w = walkers[tid];
    const float2 mid = 0.5f * float2(p.width, p.height);
    uint k = 0;
    if (w.anchor != 0 && w.rng != 0) {
        float2 z = 0.0f;
        for (uint n = 0; n < w.curEsc && k == 0; n++) {
            z = float2(z.x * z.x - z.y * z.y, 2.0f * z.x * z.y) + w.curC;
            const float2 dz = (z - p.center) * p.invPixel;
            const float x = dz.x * p.rot.x + dz.y * p.rot.y + mid.x;
            const float y = mid.y - (-dz.x * p.rot.y + dz.y * p.rot.x);
            if (x >= 0.0f && y >= 0.0f && x < float(p.width) && y < float(p.height)) k++;
        }
    }
    // Orbits in progress and held counts belong to the old view: dropped (skipping a step does
    // not change what the chain converges to).
    w.inS = k != 0 ? 1u : 0u;
    w.phase = PH_NEW;
    w.hold = 0;
    w.flushW = 0;
    w.recheck = 0;
    w.gen = p.generation;
    walkers[tid] = w;
    snap[tid] = ChainSnap{w.curC, w.curEsc, w.inS};
}

kernel void buddhaResample(constant BuddhaParams& p   [[buffer(0)]],
                           device Walker* walkers     [[buffer(2)]],
                           device const ChainSnap* snap [[buffer(4)]],
                           uint tid                   [[thread_position_in_grid]])
{
    if (tid >= p.threads) return;
    if (snap[tid].inS != 0) return;
    uint rng = hashU(p.seed ^ hashU(tid * 0x85EBCA6Bu + 0xC2B2AE35u)) | 1u;
    for (uint t = 0; t < 16; t++) {
        const uint j = pcg(rng) % p.threads;
        const ChainSnap s = snap[j];
        if (s.inS != 0) {
            Walker w = walkers[tid];
            if (w.rng == 0) {
                w.rng = rng;
                w.phase = PH_NEW;
                w.gen = p.generation;
            }
            w.curC = s.c;
            w.curEsc = s.esc;
            w.inS = 1;
            w.anchor = 1;
            walkers[tid] = w;
            return;
        }
    }
}

kernel void buddhaClear(device uint* buf [[buffer(0)]], constant uint& n [[buffer(1)]],
                        uint tid [[thread_position_in_grid]])
{
    if (tid < n) buf[tid] = 0;
}

// ---- Live mode: an accumulation that follows the view ----

struct FoldParams {
    float2 origin;       // old-accum bin coordinates of new bin (0, 0)'s centre
    float2 stepX;        // ... per new bin to the right
    float2 stepY;        // ... per new bin down
    uint   width;        // bins (old and new accum: the same size)
    uint   height;
    uint   channels;
    float  decay;        // weight of the old accumulation
    float  areaRatio;    // new bin area / old bin area (hits per bin scale with it)
    float  rho;          // share of uniform proposals in S (1: uniform sampling)
    uint   reproject;    // 0: the old accum has no data (cleared)
    uint   slot;         // counter slot of this frame's sampling
};

// accum: (hits of channels 0..2, uniform-equivalent samples) per bin.
kernel void buddhaFold(constant FoldParams& f       [[buffer(0)]],
                       device uint* frame           [[buffer(1)]],
                       device const float4* accIn   [[buffer(2)]],
                       device float4* accOut        [[buffer(3)]],
                       device const uint* counters  [[buffer(4)]],
                       uint2 gid                    [[thread_position_in_grid]])
{
    if (gid.x >= f.width || gid.y >= f.height) return;
    const uint plane = f.width * f.height;
    const uint b = gid.y * f.width + gid.x;
    float4 old = 0.0f;
    if (f.reproject != 0) {
        // Bilinear from the old accumulation (by hand: buffers), zero outside it.
        const float2 q = f.origin + f.stepX * float(gid.x) + f.stepY * float(gid.y) - 0.5f;
        const float2 q0 = floor(q);
        const float2 t = q - q0;
        const int2 i0 = int2(q0);
        float4 sum = 0.0f;
        for (int j = 0; j < 2; j++) {
            for (int i = 0; i < 2; i++) {
                const int2 s = i0 + int2(i, j);
                if (s.x < 0 || s.y < 0 || s.x >= int(f.width) || s.y >= int(f.height)) continue;
                const float w = (i == 0 ? 1.0f - t.x : t.x) * (j == 0 ? 1.0f - t.y : t.y);
                sum += w * accIn[uint(s.y) * f.width + uint(s.x)];
            }
        }
        old = f.decay * float4(sum.xyz * f.areaRatio, sum.w);
    }
    // This frame's uniform-equivalent samples: its steps over rho.
    const float samples = float(counters[f.slot * COUNTERS + C_STEPS]) / f.rho;
    float4 add = float4(0.0f, 0.0f, 0.0f, samples);
    add.x = float(frame[b]);
    frame[b] = 0;
    if (f.channels > 1) {
        add.y = float(frame[plane + b]);
        add.z = float(frame[2 * plane + b]);
        frame[plane + b] = 0;
        frame[2 * plane + b] = 0;
    }
    accOut[b] = old + add;
}

// Seeds the live accumulation from the rest histogram when motion starts (the image does not
// drop to one frame's noise): each live bin sums its div x div rest bins.
struct SeedLiveParams {
    uint  div;
    uint  restWidth;
    uint  restHeight;
    uint  width;         // live bins
    uint  height;
    uint  channels;
    float samples;       // uniform-equivalent samples behind the rest histogram
    int   offX;          // rest bins the live grid overhangs on the left / top
    int   offY;
    uint  pad;
};

kernel void buddhaSeedLive(constant SeedLiveParams& s [[buffer(0)]],
                           device const uint* rest     [[buffer(1)]],
                           device float4* accOut       [[buffer(2)]],
                           uint2 gid                   [[thread_position_in_grid]])
{
    if (gid.x >= s.width || gid.y >= s.height) return;
    const uint plane = s.restWidth * s.restHeight;
    float3 sum = 0.0f;
    for (uint j = 0; j < s.div; j++) {
        const int y = int(gid.y * s.div + j) - s.offY;
        if (y < 0 || y >= int(s.restHeight)) continue;
        for (uint i = 0; i < s.div; i++) {
            const int x = int(gid.x * s.div + i) - s.offX;
            if (x < 0 || x >= int(s.restWidth)) continue;
            const uint b = uint(y) * s.restWidth + uint(x);
            sum.x += float(rest[b]);
            if (s.channels > 1) {
                sum.y += float(rest[plane + b]);
                sum.z += float(rest[2 * plane + b]);
            }
        }
    }
    accOut[gid.y * s.width + gid.x] = float4(sum, s.samples);
}

// ---- Display ----

struct ToneParams {
    float4 scale;        // live: per channel 1 / mean density; rest: from the totals
    float4 white;        // per channel: the density (in view means) shown white
    float4 black;        // ... and black (the haze of short orbits under it)
    uint   srcWidth;     // histogram / accum bins
    uint   srcHeight;
    uint   outWidth;
    uint   outHeight;
    uint   channels;
    uint   source;       // 0: rest histogram (uint planes), 1: live accum (float4)
    uint   paletteSize;  // entries of palette level 0
    float  gamma;
    float  palettePeak;  // palette position of its brightest entry (Buddhabrot ramp end)
    uint   pad0;
    uint   pad1;
    uint   pad2;
};

static inline float3 linearToSRGB(float3 c) {
    c = saturate(c);
    return select(1.055f * pow(c, float3(1.0f / 2.4f)) - 0.055f, 12.92f * c, c <= 0.0031308f);
}

static inline float3 dithered(float3 linear, uint2 gid) {
    uint h = gid.x * 1973u + gid.y * 9277u;
    h = (h ^ (h >> 13)) * 0x5bd1e995u;
    h ^= h >> 15;
    const float n1 = float(h & 0xffffu) * (1.0f / 65535.0f);
    const float n2 = float(h >> 16) * (1.0f / 65535.0f);
    return saturate(linearToSRGB(linear) + (n1 + n2 - 1.0f) / 255.0f);
}

static inline float3 densityAt(constant ToneParams& t, device const uint* hist, device const float4* acc,
                               uint2 b) {
    const uint plane = t.srcWidth * t.srcHeight;
    const uint i = b.y * t.srcWidth + b.x;
    if (t.source == 0) {
        float3 v = float3(float(hist[i]), 0.0f, 0.0f);
        if (t.channels > 1) {
            v.y = float(hist[plane + i]);
            v.z = float(hist[2 * plane + i]);
        }
        return v;
    }
    const float4 a = acc[i];
    return a.w > 0.0f ? a.xyz / a.w : float3(0.0f);
}

kernel void buddhaTone(constant ToneParams& t          [[buffer(0)]],
                       device const uint* hist         [[buffer(1)]],
                       device const float4* acc        [[buffer(2)]],
                       device const float4* pal        [[buffer(3)]],
                       device const uint* totals       [[buffer(4)]],
                       texture2d<float, access::write> out     [[texture(0)]],
                       texture2d<float, access::write> history [[texture(1)]],
                       uint2 gid                        [[thread_position_in_grid]])
{
    if (gid.x >= t.outWidth || gid.y >= t.outHeight) return;
    float3 v;
    if (t.srcWidth == t.outWidth && t.srcHeight == t.outHeight) {
        v = densityAt(t, hist, acc, gid);
    } else {
        // A coarser histogram (live mode): bilinear in density.
        const float2 q = (float2(gid) + 0.5f) * float2(t.srcWidth, t.srcHeight) / float2(t.outWidth, t.outHeight) - 0.5f;
        const float2 q0 = clamp(floor(q), float2(0.0f), float2(t.srcWidth - 1, t.srcHeight - 1));
        const float2 f = saturate(q - q0);
        const uint2 a = uint2(q0);
        const uint2 b2 = min(a + 1u, uint2(t.srcWidth - 1, t.srcHeight - 1));
        v = mix(mix(densityAt(t, hist, acc, a), densityAt(t, hist, acc, uint2(b2.x, a.y)), f.x),
                mix(densityAt(t, hist, acc, uint2(a.x, b2.y)), densityAt(t, hist, acc, b2), f.x), f.y);
    }
    float3 scale = t.scale.xyz;
    if (t.source == 0) {
        // The rest epoch's hit totals (64-bit): 1 / mean = bins / hits.
        const float bins = float(t.srcWidth) * float(t.srcHeight);
        for (uint k = 0; k < 3; k++) {
            const uint i = 2u * (C_HITS + k);
            const float hits = float(totals[i]) + 4294967296.0f * float(totals[i + 1]);
            scale[k] = hits > 0.0f ? bins / hits : 0.0f;
        }
    }
    // In view means, from the black point (the haze of short orbits) to the white point.
    const float3 x = pow(saturate((v * scale - t.black.xyz) / max(t.white.xyz - t.black.xyz, 1e-3f)),
                         float3(t.gamma));
    float3 col;
    if (t.channels > 1) {
        col = x;
    } else {
        // The palette's rising half: dark at 0, its brightest entry at 1, fading in from black.
        const float pos = x.x * t.palettePeak;
        const float fi = pos * float(t.paletteSize);
        const uint i0 = min(uint(fi), t.paletteSize - 1);
        const uint i1 = min(i0 + 1, t.paletteSize - 1);
        const float3 c = mix(pal[i0].rgb, pal[i1].rgb, fi - float(i0));
        col = c * smoothstep(0.0f, 0.25f, x.x);
    }
    out.write(float4(dithered(col, gid), 1.0f), gid);
    history.write(float4(col, 1.0f), gid);
}

// The rest histogram's levels: per channel, how many bins hold a count in each 1/16 octave
// (bucket 0: empty bins; bucket b: counts in [2^((b-1)/16), 2^(b/16))), and the epoch's hit
// totals at the same moment. The CPU turns them into the tone curve's black and white points.
constant uint LEVEL_BUCKETS = 512;

kernel void buddhaLevels(constant ToneParams& t       [[buffer(0)]],
                         device const uint* hist      [[buffer(1)]],
                         device const uint* totals    [[buffer(4)]],
                         device atomic_uint* levels   [[buffer(5)]],
                         uint tid                     [[thread_position_in_grid]])
{
    const uint plane = t.srcWidth * t.srcHeight;
    if (tid == 0) {
        for (uint k = 0; k < 6; k++) {
            atomic_store_explicit(&levels[3 * LEVEL_BUCKETS + k], totals[2u * C_HITS + k], memory_order_relaxed);
        }
    }
    if (tid >= plane) return;
    for (uint ch = 0; ch < t.channels; ch++) {
        const uint v = hist[ch * plane + tid];
        const uint b = v == 0 ? 0u : min(LEVEL_BUCKETS - 1, 1u + uint(log2(float(v)) * 16.0f));
        atomic_fetch_add_explicit(&levels[ch * LEVEL_BUCKETS + b], 1u, memory_order_relaxed);
    }
}

// The structs' sizes as this compiler lays them out (the Rust mirrors' layout test).
kernel void buddhaLayout(device uint* out [[buffer(0)]], uint gid [[thread_position_in_grid]])
{
    if (gid == 0) {
        out[0] = uint(sizeof(BuddhaParams));
        out[1] = uint(sizeof(Walker));
        out[2] = uint(sizeof(FoldParams));
        out[3] = uint(sizeof(ToneParams));
        out[4] = uint(sizeof(SeedLiveParams));
        out[5] = uint(sizeof(ChainSnap));
    }
}
