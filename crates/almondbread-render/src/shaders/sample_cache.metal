// ---------------------------------------------------------------------------------------------
// Motion sample cache (kb: techniques/sample-reuse, synthesis/motion-sample-reuse). Appended to
// mandelbrot.metal at compile time (it uses shade(), auxAt() and the ITER_ codes from there).
//
// Gesture frames keep their exact samples on a lattice fixed in the complex plane: lattice
// point (i, j) of a level is anchor + h * R(angle) * (i, -j). One cache level is a toroidal
// window of that lattice, a W x H texture set (iteration, DE, interior analysis) in which window
// texel t is texel (t + wrap) mod (W, H), so a pan only clears and computes the tiles it exposes
// (the CPU keeps per-tile state; sample_cache.rs). Every texel inside the window holds either the
// exact sample of its lattice point or ITER_EMPTY. Zooming switches the level (spacing halves or
// doubles); the lattice points of the coarser level are the even points of the finer one, so a
// switch copies the samples that coincide (scResample) and computes the rest.

struct CacheParams {
    int2  newOrigin;    // resample / seed: lattice index of window texel (0, 0) of the new window
    int2  oldBase;      // resample: old-window texel of new lattice index 0; seed: full texel
    uint2 newSize;      // cache texture (window) size
    uint2 newWrap;      // texel of window texel (0, 0)
    uint2 oldSize;      // resample: old cache size; seed: full-level size
    uint2 oldWrap;      // resample: old texel of old window texel (0, 0); seed: 0
    uint2 rectOrigin;   // shade: window-relative first texel of the shaded rectangle
    uint2 rectSize;     // shade: its size
    int   num;          // old index = oldBase + (I * num) >> shift (I: new lattice index)
    uint  shift;        // 1: the new level is finer (odd I have no old sample)
    float deScale;      // multiplier of copied distance estimates (texel units change)
    uint  count;        // clear: tiles in the list
    uint  hasAux;       // the interior analysis texture is bound
    uint  pad;
};

// Marks the listed tiles (texel tiles of the cache texture) empty: tiles entering the window.
kernel void scClear(texture2d<float, access::write> iterOut [[texture(0)]],
                    constant CacheParams& p                 [[buffer(0)]],
                    device const uint* tiles                [[buffer(1)]],
                    uint tid                                [[thread_position_in_grid]])
{
    const uint k = tid >> 8;
    if (k >= p.count) return;
    const uint slot = tiles[k], tilesX = p.newSize.x >> 4, w = tid & 255u;
    const uint2 xy = uint2((slot % tilesX) * 16u + (w & 15u), (slot / tilesX) * 16u + (w >> 4));
    iterOut.write(float4(ITER_EMPTY, 0.0f, 0.0f, 0.0f), xy);
}

// Fills a whole new window from an old one (a level switch: num/shift as above, old texture
// toroidal) or from the refined full level (seeding at rest: num = the step, no wrap). Lattice
// points without a sample there become ITER_EMPTY. Distance estimates are in texels of the
// spacing, so they scale with it; codes, counts and normal angles do not.
kernel void scResample(texture2d<float, access::read>  iterIn  [[texture(0)]],
                       texture2d<float, access::read>  deIn    [[texture(1)]],
                       texture2d<float, access::read>  auxIn   [[texture(2)]],
                       texture2d<float, access::write> iterOut [[texture(3)]],
                       texture2d<float, access::write> deOut   [[texture(4)]],
                       texture2d<float, access::write> auxOut  [[texture(5)]],
                       constant CacheParams& p                 [[buffer(0)]],
                       uint2 gid                               [[thread_position_in_grid]])
{
    if (any(gid >= p.newSize)) return;
    const int2 I = p.newOrigin + int2(gid);
    float4 it = float4(ITER_EMPTY, 0.0f, 0.0f, 0.0f), de = 0.0f, ax = 0.0f;
    const int2 m = I * p.num;
    const bool on = p.shift == 0 || all((m & 1) == 0);
    const int2 o = p.oldBase + (m >> int(p.shift));
    if (on && all(o >= 0) && all(o < int2(p.oldSize))) {
        const uint2 s = (uint2(o) + p.oldWrap) % p.oldSize;
        it = iterIn.read(s);
        if (it.x == ITER_PENDING || it.x == ITER_SKIP) {
            it = float4(ITER_EMPTY, 0.0f, 0.0f, 0.0f);
        } else if (it.x != ITER_EMPTY) {
            de = deIn.read(s);
            // (Escaped: DE in texels, 1000 = none. Analysed interior: |lambda|, arg lambda.)
            if (it.x >= 0.0f && de.x < 999.0f) de.x = min(de.x * p.deScale, 65000.0f);
            if (p.hasAux != 0) {
                ax = auxIn.read(s);
                if (it.x == ITER_INTERIOR) ax.x *= p.deScale;
            }
        }
    }
    const uint2 d = (gid + p.newWrap) % p.newSize;
    iterOut.write(it, d);
    deOut.write(de, d);
    if (p.hasAux != 0) auxOut.write(ax, d);
}

// Per-tile sample statistics for the planner's cost estimate (one threadgroup = one 16 x 16
// tile): sum of escape counts, samples escaped, interior | unresolved << 16, largest escape count.
static inline void tileStats(float2 it, bool valid, uint slot, uint tidInGroup, uint lane, uint sg,
                             device uint4* est, threadgroup uint4* tg) {
    const bool esc = valid && it.x >= 0.0f;
    const uint4 v = uint4(simd_sum(esc ? uint(it.x) : 0u), simd_sum(esc ? 1u : 0u),
                          simd_sum(valid && it.x == ITER_INTERIOR ? 1u : 0u)
                              | (simd_sum(valid && it.x == ITER_UNRESOLVED ? 1u : 0u) << 16),
                          simd_max(esc ? uint(it.x) : 0u));
    if (lane == 0) tg[sg] = v;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tidInGroup == 0) {
        uint4 t = tg[0];
        for (uint k = 1; k < 8; k++) {
            t.xyz += tg[k].xyz;
            t.w = max(t.w, tg[k].w);
        }
        est[slot] = t;
    }
}

// Colours the cache texels of the visible rectangle (premultiplied linear colour, alpha 0 where
// there is no sample) with the current colour parameters: samples are re-shaded every frame, so
// the outline width and the palette filter follow the current pixel size. Also counts what the
// view holds (samples, interior, unresolved, escape iterations as a 64-bit sum) for the
// interior-test decision (kb: techniques/interior-detection): a frame's new tiles alone are a
// biased sample of the view. counts: samples, interior, unresolved, escape iterations (lo, hi),
// interior at the limit without the test. Threadgroups are the 16 x 16 tiles (the rectangle is
// tile-aligned), which also write their tile's statistics.
kernel void scShade(texture2d<float, access::read>  iters  [[texture(0)]],
                    texture2d<float, access::read>  des    [[texture(1)]],
                    texture2d<float, access::read>  auxs   [[texture(2)]],
                    texture2d<float, access::write> out    [[texture(3)]],
                    constant ColorParams& c                [[buffer(0)]],
                    device const float4* pal               [[buffer(1)]],
                    constant CacheParams& p                [[buffer(2)]],
                    device atomic_uint* counts             [[buffer(3)]],
                    device uint4* est                      [[buffer(4)]],
                    uint2 gid                              [[thread_position_in_grid]],
                    uint tidInGroup                        [[thread_index_in_threadgroup]],
                    uint lane                              [[thread_index_in_simdgroup]],
                    uint sg                                [[simdgroup_index_in_threadgroup]])
{
    threadgroup uint4 tg[8];
    const uint2 s = (p.rectOrigin + gid + p.newWrap) % p.newSize;
    const float2 it = iters.read(s).xy;
    const bool valid = it.x != ITER_EMPTY && it.x != ITER_PENDING;
    out.write(valid ? float4(shade(it, des.read(s).xy, auxAt(auxs, s, c, it), c, pal, c.deScale), 1.0f)
                    : float4(0.0f), s);
    tileStats(it, valid, (s.y >> 4) * (p.newSize.x >> 4) + (s.x >> 4), tidInGroup, lane, sg, est, tg);
    const uint nValid = simd_sum(valid ? 1u : 0u);
    const uint nInterior = simd_sum(valid && it.x == ITER_INTERIOR ? 1u : 0u);
    const uint nUnresolved = simd_sum(valid && it.x == ITER_UNRESOLVED ? 1u : 0u);
    const uint nIter = simd_sum(valid && it.x >= 0.0f ? uint(it.x) : 0u);
    const uint nLimit = simd_sum(valid && it.x == ITER_INTERIOR && it.y == 0.0f ? 1u : 0u);
    if (lane == 0 && nValid > 0) {
        atomic_fetch_add_explicit(&counts[0], nValid, memory_order_relaxed);
        atomic_fetch_add_explicit(&counts[1], nInterior, memory_order_relaxed);
        atomic_fetch_add_explicit(&counts[2], nUnresolved, memory_order_relaxed);
        const uint old = atomic_fetch_add_explicit(&counts[3], nIter, memory_order_relaxed);
        if (old + nIter < old) atomic_fetch_add_explicit(&counts[4], 1u, memory_order_relaxed);
        if (nLimit > 0) atomic_fetch_add_explicit(&counts[5], nLimit, memory_order_relaxed);
    }
}

// scShade for a list of texture tiles only (a pan with unchanged colours: only the tiles whose
// samples changed, or that entered the shaded rectangle, need colouring again).
kernel void scShadeTiles(texture2d<float, access::read>  iters [[texture(0)]],
                         texture2d<float, access::read>  des   [[texture(1)]],
                         texture2d<float, access::read>  auxs  [[texture(2)]],
                         texture2d<float, access::write> out   [[texture(3)]],
                         constant ColorParams& c               [[buffer(0)]],
                         device const float4* pal              [[buffer(1)]],
                         constant CacheParams& p               [[buffer(2)]],
                         device const uint* tiles              [[buffer(3)]],
                         device uint4* est                     [[buffer(4)]],
                         uint tid                              [[thread_position_in_grid]],
                         uint tidInGroup                       [[thread_index_in_threadgroup]],
                         uint lane                             [[thread_index_in_simdgroup]],
                         uint sg                               [[simdgroup_index_in_threadgroup]])
{
    threadgroup uint4 tg[8];
    const uint k = min(tid >> 8, p.count - 1);
    const uint slot = tiles[k], tilesX = p.newSize.x >> 4, w = tid & 255u;
    const uint2 s = uint2((slot % tilesX) * 16u + (w & 15u), (slot / tilesX) * 16u + (w >> 4));
    const float2 it = iters.read(s).xy;
    const bool valid = it.x != ITER_EMPTY && it.x != ITER_PENDING;
    out.write(valid ? float4(shade(it, des.read(s).xy, auxAt(auxs, s, c, it), c, pal, c.deScale), 1.0f)
                    : float4(0.0f), s);
    tileStats(it, valid, slot, tidInGroup, lane, sg, est, tg);
}
