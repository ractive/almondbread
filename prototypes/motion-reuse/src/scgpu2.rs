//! GPU sample-cache bookkeeping, second version (R4): compact structure-of-arrays slots
//! (12 B per sample: half2 offset inside its cell, float mu with the class in its sign, half DE,
//! half angle; u8 counts) and a resolve pass that loads an 18 x 18-cell tile per 16 x 16
//! threadgroup into threadgroup memory, shading each sample there (once per tile instead of
//! once per frame in a separate pass plus 9 reads per pixel). Same synthetic set-up as v1
//! (scgpu.rs).

use almondbread_render::metal::{KERNEL_SOURCE, MetalBackend};
use almondbread_render::{ComputeEncoder, GpuBackend, Storage, Submission, TextureFormat};

const SC2_SOURCE: &str = r#"
struct Sc2Params { float2 c; float2 b; float a; float inva; uint w; uint h; uint k; uint fillMin;
                   float inv2s2; uint frame; };
struct Raw { float mu; half de; half ang; };   // mu < 0: -1 interior, -3 unresolved

kernel void sc2Move(device const half2* oPos [[buffer(0)]], device const Raw* oRaw [[buffer(1)]],
                    device const uint* oCount [[buffer(2)]],
                    device half2* nPos [[buffer(3)]], device Raw* nRaw [[buffer(4)]],
                    device uint* nCount [[buffer(5)]],
                    constant Sc2Params& q [[buffer(6)]], uint2 gid [[thread_position_in_grid]])
{
    if (gid.x >= q.w || gid.y >= q.h) return;
    float2 lo = (float2(gid) - q.c - q.b) * q.inva + q.c;
    float2 hi = (float2(gid) + 1.0f - q.c - q.b) * q.inva + q.c;
    int2 mx = int2(q.w - 1, q.h - 1);
    int2 o0 = clamp(int2(floor(lo)), int2(0), mx);
    int2 o1 = clamp(int2(floor(hi)), o0, min(o0 + 3, mx));
    uint cell = gid.y * q.w + gid.x;
    uint n = 0;
    for (int y = o0.y; y <= o1.y; y++) {
        for (int x = o0.x; x <= o1.x; x++) {
            uint oc = uint(y) * q.w + uint(x);
            uint m = oCount[oc];
            for (uint s = 0; s < m; s++) {
                float2 p = (float2(x, y) + float2(oPos[oc * q.k + s]) - q.c) * q.a + q.c + q.b;
                float2 f = p - float2(gid);
                if (all(f >= 0.0f) && all(f < 1.0f) && n < q.k) {
                    Raw r = oRaw[oc * q.k + s];
                    r.de = half(float(r.de) * q.a);
                    nPos[cell * q.k + n] = half2(f);
                    nRaw[cell * q.k + n] = r;
                    n++;
                }
            }
        }
    }
    nCount[cell] = n;
}

kernel void sc2Zero(device uint* l [[buffer(0)]], uint i [[thread_position_in_grid]]) { if (i == 0) l[0] = 0; }

kernel void sc2List(device const uint* count [[buffer(0)]], device atomic_uint* listLen [[buffer(1)]],
                    device uint* list [[buffer(2)]], constant Sc2Params& q [[buffer(3)]],
                    uint2 gid [[thread_position_in_grid]])
{
    bool want = gid.x < q.w && gid.y < q.h && count[gid.y * q.w + gid.x] < q.fillMin;
    uint before = simd_prefix_exclusive_sum(uint(want));
    uint total = simd_sum(uint(want));
    uint base = 0;
    if (simd_is_first() && total > 0) base = atomic_fetch_add_explicit(listLen, total, memory_order_relaxed);
    base = simd_broadcast_first(base);
    if (want) list[base + before] = gid.y * q.w + gid.x;
}

kernel void sc2Insert(device const uint* list [[buffer(0)]], device const uint* listLen [[buffer(1)]],
                      device half2* P [[buffer(2)]], device Raw* R [[buffer(3)]], device uint* count [[buffer(4)]],
                      constant Sc2Params& q [[buffer(5)]], uint i [[thread_position_in_grid]])
{
    if (i >= listLen[0]) return;
    uint cell = list[i];
    uint n = count[cell];
    if (n >= q.k) return;
    uint h = cell * 2654435761u ^ q.frame * 40503u;
    P[cell * q.k + n] = half2(float2(float(h & 1023u), float((h >> 10) & 1023u)) / 1024.0f);
    Raw r;
    r.mu = float(100u + (h >> 20) % 900u) - 3.0f + float((h >> 4) & 255u) / 256.0f;
    r.de = half(float((h >> 12) & 255u) / 32.0f);
    r.ang = half(float((h >> 3) & 1023u) / 163.0f);
    R[cell * q.k + n] = r;
    count[cell] = n + 1;
}

// Yardstick: read 16 B per pixel, write one rgba16f texel (memory bandwidth under the
// current GPU load).
kernel void sc2Copy(device const float4* src [[buffer(0)]], constant Sc2Params& q [[buffer(1)]],
                    texture2d<float, access::write> out [[texture(0)]], uint2 gid [[thread_position_in_grid]])
{
    if (gid.x >= q.w || gid.y >= q.h) return;
    out.write(src[gid.y * q.w + gid.x], gid);
}

#define TILE 16
#define HALO 18
#define KMAX 4
kernel void sc2Resolve(device const half2* P [[buffer(0)]], device const Raw* R [[buffer(1)]],
                       device const uint* count [[buffer(2)]], constant Sc2Params& q [[buffer(3)]],
                       constant ColorParams& c [[buffer(4)]], device const float4* pal [[buffer(5)]],
                       texture2d<float, access::write> out [[texture(0)]],
                       uint2 gid [[thread_position_in_grid]], uint2 lid [[thread_position_in_threadgroup]],
                       uint2 tgid [[threadgroup_position_in_grid]])
{
    threadgroup half4 tcol[HALO * HALO * KMAX];   // rgb + offset x packed below
    threadgroup half2 tpos[HALO * HALO * KMAX];
    threadgroup uchar tcnt[HALO * HALO];
    uint tid = lid.y * TILE + lid.x;
    int2 origin = int2(tgid) * TILE - 1;
    for (uint t = tid; t < HALO * HALO; t += TILE * TILE) {
        int2 cc = origin + int2(t % HALO, t / HALO);
        uint m = 0;
        if (all(cc >= 0) && cc.x < int(q.w) && cc.y < int(q.h)) {
            uint cell = uint(cc.y) * q.w + uint(cc.x);
            m = min(uint(count[cell]), uint(KMAX));
            for (uint s = 0; s < m; s++) {
                Raw r = R[cell * q.k + s];
                float2 it = r.mu < 0.0f ? float2(r.mu, 1.0f) : float2(floor(r.mu), r.mu - floor(r.mu) - 3.0f);
                float3 col = shade(it, float2(float(r.de), float(r.ang)), c, pal, 1.0f);
                tcol[t * KMAX + s] = half4(half3(col), 1.0h);
                tpos[t * KMAX + s] = P[cell * q.k + s];
            }
        }
        tcnt[t] = uchar(m);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (gid.x >= q.w || gid.y >= q.h) return;
    float3 sum = 0.0f;
    float ws = 0.0f;
    for (int dy = 0; dy < 3; dy++) {
        for (int dx = 0; dx < 3; dx++) {
            uint t = (lid.y + dy) * HALO + lid.x + dx;
            uint m = tcnt[t];
            for (uint s = 0; s < m; s++) {
                float2 d = float2(dx - 1, dy - 1) + float2(tpos[t * KMAX + s]) - 0.5f;
                float w = exp(-dot(d, d) * q.inv2s2);
                sum += float3(tcol[t * KMAX + s].rgb) * w;
                ws += w;
            }
        }
    }
    out.write(float4(ws > 1e-3f ? sum / ws : float3(0.0f), 1.0f), gid);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Params {
    c: [f32; 2],
    b: [f32; 2],
    a: f32,
    inva: f32,
    w: u32,
    h: u32,
    k: u32,
    fill_min: u32,
    inv2s2: f32,
    frame: u32,
}

fn bytes<T: Copy>(v: &T) -> &[u8] {
    // SAFETY: plain #[repr(C)] data read as bytes.
    unsafe { std::slice::from_raw_parts((v as *const T).cast::<u8>(), std::mem::size_of::<T>()) }
}

pub fn run() {
    let fused = std::env::var("SC_FUSED").is_ok_and(|v| v == "1");
    let source = format!("{KERNEL_SOURCE}\n{SC2_SOURCE}");
    let b = MetalBackend::with_source(&source).expect("compile");
    let names = ["sc2Move", "sc2List", "sc2Insert", "sc2Resolve", "sc2Zero", "sc2Copy"];
    let pipes: Vec<_> = names.iter().map(|n| b.pipeline(n, &[]).expect("pipeline")).collect();
    let (colors, pal) = crate::scgpu::color_params();
    let pal_buf = b.create_buffer_with_data(crate::scgpu::bytes_slice(&pal)).unwrap();
    const K: usize = 4;
    println!("v2 (SoA 12 B/sample, tiled resolve with shading)");
    println!(
        "size        motion            move   list  insert resolve | total ms (median of 120 frames; min..max over 3 rounds)"
    );
    for &(w, h) in &[(1280usize, 720usize), (1920, 1080), (3456, 2160)] {
        let n = w * h;
        let pos = [
            b.create_buffer(n * K * 4, Storage::Private).unwrap(),
            b.create_buffer(n * K * 4, Storage::Private).unwrap(),
        ];
        let raw = [
            b.create_buffer(n * K * 8, Storage::Private).unwrap(),
            b.create_buffer(n * K * 8, Storage::Private).unwrap(),
        ];
        let counts = [
            b.create_buffer(n * 4, Storage::Private).unwrap(),
            b.create_buffer(n * 4, Storage::Private).unwrap(),
        ];
        let list = b.create_buffer(n * 4, Storage::Private).unwrap();
        let list_len = b.create_buffer(16, Storage::Shared).unwrap();
        let out = b
            .create_texture(w, h, TextureFormat::Rgba16Float, Storage::Private)
            .unwrap();
        let src16 = b.create_buffer(n * 16, Storage::Private).unwrap();
        let mut copy = Vec::new();
        for _ in 0..60 {
            let q = Params {
                w: w as u32,
                h: h as u32,
                ..Params::default()
            };
            let mut enc = b.begin("sc2Copy").unwrap();
            enc.set_pipeline(&pipes[5]);
            enc.set_buffer(0, &src16, 0);
            enc.set_bytes(1, bytes(&q));
            enc.set_texture(0, &out);
            enc.dispatch_threads([w, h, 1], [32, 8, 1]);
            let s = enc.commit();
            s.wait();
            copy.push(s.gpu_time() * 1000.0);
        }
        copy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{w}x{h:<6} yardstick copy 16 B/px -> rgba16f: median {:.3} ms, min {:.3} ms",
            copy[30], copy[0]
        );
        for &(label, efolds, pan) in &[
            ("zoom 1.6/s", 1.6f64, 0.0f64),
            ("zoom 7.6/s", 7.6, 0.0),
            ("pan 13px", 0.0, 13.0),
        ] {
            let mut rounds: Vec<[f64; 5]> = Vec::new();
            for _round in 0..3 {
                let mut times: Vec<[f64; 4]> = Vec::new();
                let mut inflight = std::collections::VecDeque::new();
                let mut ping = 0;
                for frame in 0..140u32 {
                    let a = (efolds / 60.0).exp() as f32;
                    let q = Params {
                        c: [w as f32 / 2.0, h as f32 / 2.0],
                        b: [-(pan as f32) * (w as f32 / 1920.0), 0.0],
                        a,
                        inva: 1.0 / a,
                        w: w as u32,
                        h: h as u32,
                        k: K as u32,
                        fill_min: 1,
                        inv2s2: 2.0,
                        frame,
                    };
                    let (src, dst) = (ping, 1 - ping);
                    if fused {
                        // All passes in one command buffer, frames pipelined (two in flight),
                        // as an app would submit them: the GPU stays clocked up.
                        let mut enc = b.begin("sc2Frame").unwrap();
                        enc.set_pipeline(&pipes[0]);
                        enc.set_buffer(0, &pos[src], 0);
                        enc.set_buffer(1, &raw[src], 0);
                        enc.set_buffer(2, &counts[src], 0);
                        enc.set_buffer(3, &pos[dst], 0);
                        enc.set_buffer(4, &raw[dst], 0);
                        enc.set_buffer(5, &counts[dst], 0);
                        enc.set_bytes(6, bytes(&q));
                        enc.dispatch_threads([w, h, 1], [32, 8, 1]);
                        enc.set_pipeline(&pipes[4]);
                        enc.set_buffer(0, &list_len, 0);
                        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
                        enc.set_pipeline(&pipes[1]);
                        enc.set_buffer(0, &counts[dst], 0);
                        enc.set_buffer(1, &list_len, 0);
                        enc.set_buffer(2, &list, 0);
                        enc.set_bytes(3, bytes(&q));
                        enc.dispatch_threads([w, h, 1], [32, 8, 1]);
                        enc.set_pipeline(&pipes[2]);
                        enc.set_buffer(0, &list, 0);
                        enc.set_buffer(1, &list_len, 0);
                        enc.set_buffer(2, &pos[dst], 0);
                        enc.set_buffer(3, &raw[dst], 0);
                        enc.set_buffer(4, &counts[dst], 0);
                        enc.set_bytes(5, bytes(&q));
                        enc.dispatch_threads([n, 1, 1], [256, 1, 1]);
                        enc.set_pipeline(&pipes[3]);
                        enc.set_buffer(0, &pos[dst], 0);
                        enc.set_buffer(1, &raw[dst], 0);
                        enc.set_buffer(2, &counts[dst], 0);
                        enc.set_bytes(3, bytes(&q));
                        enc.set_bytes(4, bytes(&colors));
                        enc.set_buffer(5, &pal_buf, 0);
                        enc.set_texture(0, &out);
                        enc.dispatch_threads([w.div_ceil(16) * 16, h.div_ceil(16) * 16, 1], [16, 16, 1]);
                        let sub = enc.commit();
                        inflight.push_back((frame, sub));
                        if inflight.len() > 2 {
                            let (f0, s) = inflight.pop_front().unwrap();
                            s.wait();
                            if f0 >= 20 {
                                times.push([s.gpu_time(), 0.0, 0.0, 0.0]);
                            }
                        }
                        ping = dst;
                        continue;
                    }
                    let mut enc = b.begin("sc2Move").unwrap();
                    enc.set_pipeline(&pipes[0]);
                    enc.set_buffer(0, &pos[src], 0);
                    enc.set_buffer(1, &raw[src], 0);
                    enc.set_buffer(2, &counts[src], 0);
                    enc.set_buffer(3, &pos[dst], 0);
                    enc.set_buffer(4, &raw[dst], 0);
                    enc.set_buffer(5, &counts[dst], 0);
                    enc.set_bytes(6, bytes(&q));
                    enc.dispatch_threads([w, h, 1], [32, 8, 1]);
                    let s0 = enc.commit();
                    let mut enc = b.begin("sc2List").unwrap();
                    enc.set_pipeline(&pipes[4]);
                    enc.set_buffer(0, &list_len, 0);
                    enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
                    enc.set_pipeline(&pipes[1]);
                    enc.set_buffer(0, &counts[dst], 0);
                    enc.set_buffer(1, &list_len, 0);
                    enc.set_buffer(2, &list, 0);
                    enc.set_bytes(3, bytes(&q));
                    enc.dispatch_threads([w, h, 1], [32, 8, 1]);
                    let s1 = enc.commit();
                    let mut enc = b.begin("sc2Insert").unwrap();
                    enc.set_pipeline(&pipes[2]);
                    enc.set_buffer(0, &list, 0);
                    enc.set_buffer(1, &list_len, 0);
                    enc.set_buffer(2, &pos[dst], 0);
                    enc.set_buffer(3, &raw[dst], 0);
                    enc.set_buffer(4, &counts[dst], 0);
                    enc.set_bytes(5, bytes(&q));
                    enc.dispatch_threads([n, 1, 1], [256, 1, 1]);
                    let s2 = enc.commit();
                    let mut enc = b.begin("sc2Resolve").unwrap();
                    enc.set_pipeline(&pipes[3]);
                    enc.set_buffer(0, &pos[dst], 0);
                    enc.set_buffer(1, &raw[dst], 0);
                    enc.set_buffer(2, &counts[dst], 0);
                    enc.set_bytes(3, bytes(&q));
                    enc.set_bytes(4, bytes(&colors));
                    enc.set_buffer(5, &pal_buf, 0);
                    enc.set_texture(0, &out);
                    enc.dispatch_threads([w.div_ceil(16) * 16, h.div_ceil(16) * 16, 1], [16, 16, 1]);
                    let s3 = enc.commit();
                    s3.wait();
                    let t = [s0.gpu_time(), s1.gpu_time(), s2.gpu_time(), s3.gpu_time()];
                    ping = dst;
                    if frame >= 20 {
                        times.push(t);
                    }
                }
                for (f0, s) in inflight.drain(..) {
                    s.wait();
                    if f0 >= 20 {
                        times.push([s.gpu_time(), 0.0, 0.0, 0.0]);
                    }
                }
                let mut med = [0.0f64; 5];
                for p in 0..4 {
                    let mut v: Vec<f64> = times.iter().map(|t| t[p]).collect();
                    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    med[p] = v[v.len() / 2] * 1000.0;
                }
                let mut tot: Vec<f64> = times.iter().map(|t| t.iter().sum::<f64>()).collect();
                tot.sort_by(|a, b| a.partial_cmp(b).unwrap());
                med[4] = tot[tot.len() / 2] * 1000.0;
                rounds.push(med);
            }
            let best = |p: usize| rounds.iter().map(|r| r[p]).fold(f64::MAX, f64::min);
            let worst = |p: usize| rounds.iter().map(|r| r[p]).fold(0.0, f64::max);
            println!(
                "{w}x{h:<6} {label:<14} {:6.3} {:6.3} {:6.3} {:6.3} | {:6.3} .. {:6.3}",
                best(0),
                best(1),
                best(2),
                best(3),
                best(4),
                worst(4)
            );
        }
    }
}
