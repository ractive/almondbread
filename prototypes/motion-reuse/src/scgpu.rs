//! GPU cost of a sample cache's bookkeeping (R4): the passes a GPU sample cache adds to every
//! gesture frame, besides iterating the new samples. Kernels compiled together with the app's
//! MSL source (so `shade` is the real one):
//!
//! - `scMove`: gather per new cell: the cached samples whose moved position lands in it (the
//!   cell's preimage under the pan/zoom covers at most 4 x 4 old cells), K slots per cell
//!   ("K-deep slot image"), no atomics; DE rescaled by the zoom.
//! - `scList`: cells with fewer than `fillMin` samples, appended to the admission list.
//! - `scInsert`: writes the new samples (here synthetic: the iteration itself is excluded).
//! - `scShade`: shades every cached sample once (DE in current pixels).
//! - `scResolve`: Gaussian (sigma 0.5 px) over the 3 x 3 cells' samples -> output.
//!
//! Synthetic content (random n, s, DE, angle); positions jittered. Timed per frame and per pass
//! at 1280x720, 1920x1080, 3456x2160 for zoom rates 1.6 and 7.6 e-folds/s and a pan.

use almondbread_core::palette::{Look, Palette, mip_chain, mip_levels};
use almondbread_render::gpu_types::ColorParams;
use almondbread_render::metal::{KERNEL_SOURCE, MetalBackend};
use almondbread_render::{ComputeEncoder, GpuBackend, Storage, Submission, TextureFormat};

const SC_SOURCE: &str = r#"
struct ScSample { float2 pos; float n; float s; float de; float ang; uint meta; float pad; };
struct ScParams { float2 c; float2 b; float a; float inva; uint w; uint h; uint k; uint fillMin;
                  float inv2s2; uint frame; };

kernel void scMove(device const ScSample* oldS [[buffer(0)]], device const uint* oldCount [[buffer(1)]],
                   device ScSample* newS [[buffer(2)]], device uint* newCount [[buffer(3)]],
                   constant ScParams& q [[buffer(4)]], uint2 gid [[thread_position_in_grid]])
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
            uint m = oldCount[oc];
            for (uint s = 0; s < m; s++) {
                ScSample v = oldS[oc * q.k + s];
                float2 p = (v.pos - q.c) * q.a + q.c + q.b;
                if (all(floor(p) == float2(gid)) && n < q.k) {
                    v.pos = p;
                    v.de *= q.a;
                    newS[cell * q.k + n] = v;
                    n++;
                }
            }
        }
    }
    newCount[cell] = n;
}

kernel void scZero(device uint* l [[buffer(0)]], uint i [[thread_position_in_grid]]) { if (i == 0) l[0] = 0; }

kernel void scList(device const uint* count [[buffer(0)]], device atomic_uint* listLen [[buffer(1)]],
                   device uint* list [[buffer(2)]], constant ScParams& q [[buffer(3)]],
                   uint2 gid [[thread_position_in_grid]], uint lane [[thread_index_in_simdgroup]])
{
    bool want = gid.x < q.w && gid.y < q.h && count[gid.y * q.w + gid.x] < q.fillMin;
    // One atomic per SIMD group (as aaList does).
    uint before = simd_prefix_exclusive_sum(uint(want));
    uint total = simd_sum(uint(want));
    uint base = 0;
    if (simd_is_first() && total > 0) base = atomic_fetch_add_explicit(listLen, total, memory_order_relaxed);
    base = simd_broadcast_first(base);
    if (want) list[base + before] = gid.y * q.w + gid.x;
}

kernel void scInsert(device const uint* list [[buffer(0)]], device const uint* listLen [[buffer(1)]],
                     device ScSample* S [[buffer(2)]], device uint* count [[buffer(3)]],
                     constant ScParams& q [[buffer(4)]], uint i [[thread_position_in_grid]])
{
    if (i >= listLen[0]) return;
    uint cell = list[i];
    uint n = count[cell];
    if (n >= q.k) return;
    uint h = cell * 2654435761u ^ q.frame * 40503u;
    ScSample v;
    v.pos = float2(cell % q.w, cell / q.w) + float2(float(h & 1023u), float((h >> 10) & 1023u)) / 1024.0f;
    v.n = float(100u + (h >> 20) % 900u);
    v.s = -3.0f + float((h >> 4) & 255u) / 256.0f;
    v.de = float((h >> 12) & 255u) / 32.0f;
    v.ang = float((h >> 3) & 1023u) / 163.0f;
    v.meta = q.frame << 2;
    S[cell * q.k + n] = v;
    count[cell] = n + 1;
}

kernel void scShade(device const ScSample* S [[buffer(0)]], device const uint* count [[buffer(1)]],
                    device half4* colour [[buffer(2)]], constant ScParams& q [[buffer(3)]],
                    constant ColorParams& c [[buffer(4)]], device const float4* pal [[buffer(5)]],
                    uint2 gid [[thread_position_in_grid]])
{
    if (gid.x >= q.w || gid.y >= q.h) return;
    uint cell = gid.y * q.w + gid.x;
    uint m = count[cell];
    for (uint s = 0; s < m; s++) {
        ScSample v = S[cell * q.k + s];
        uint cls = v.meta & 3u;
        float2 it = cls == 1u ? float2(-1.0f, 1.0f) : (cls == 2u ? float2(-3.0f, 0.0f) : float2(v.n, v.s));
        colour[cell * q.k + s] = half4(half3(shade(it, float2(v.de, v.ang), c, pal, 1.0f)), 1.0h);
    }
}

kernel void scResolve(device const ScSample* S [[buffer(0)]], device const uint* count [[buffer(1)]],
                      device const half4* colour [[buffer(2)]], constant ScParams& q [[buffer(3)]],
                      texture2d<float, access::write> out [[texture(0)]],
                      uint2 gid [[thread_position_in_grid]])
{
    if (gid.x >= q.w || gid.y >= q.h) return;
    float2 pc = float2(gid) + 0.5f;
    float3 sum = 0.0f;
    float ws = 0.0f;
    for (int dy = -1; dy <= 1; dy++) {
        for (int dx = -1; dx <= 1; dx++) {
            int2 cc = int2(gid) + int2(dx, dy);
            if (any(cc < 0) || cc.x >= int(q.w) || cc.y >= int(q.h)) continue;
            uint cell = uint(cc.y) * q.w + uint(cc.x);
            uint m = count[cell];
            for (uint s = 0; s < m; s++) {
                float2 d = S[cell * q.k + s].pos - pc;
                float w = exp(-dot(d, d) * q.inv2s2);
                sum += float3(colour[cell * q.k + s].rgb) * w;
                ws += w;
            }
        }
    }
    float3 col = ws > 1e-3f ? sum / ws : float3(0.0f);
    out.write(float4(col, 1.0f), gid);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ScParams {
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
    // SAFETY: plain #[repr(C)] data with no padding-sensitive invariants, read as bytes.
    unsafe { std::slice::from_raw_parts((v as *const T).cast::<u8>(), std::mem::size_of::<T>()) }
}

pub fn color_params() -> (ColorParams, Vec<[f32; 4]>) {
    let look = Look::default_look();
    let lin = almondbread_core::palette::linear;
    let a = look.light_angle.to_radians();
    let i = lin(look.interior);
    let e = lin(look.edge_color);
    let c = ColorParams {
        interior: [i[0], i[1], i[2], 1.0],
        edge: [e[0], e[1], e[2], 1.0],
        glow_color: [1.0, 1.0, 1.0, 1.0],
        light: [a.cos() as f32, a.sin() as f32],
        density: look.density as f32,
        ratio: 1.0,
        shading: look.shading as f32,
        outline: look.outline as f32,
        outline_width: look.outline_width as f32,
        palette_size: 1024,
        smooth: 1,
        de_scale: 1.0,
        specular: look.specular as f32,
        shininess: look.shininess as f32,
        light_height: look.light_height as f32,
        palette_levels: mip_levels(1024) as u32,
        palette_filter: 1.0,
        ..ColorParams::default()
    };
    (c, mip_chain(&Palette::named(look.palette).table(1024)))
}

pub fn run() {
    let source = format!("{KERNEL_SOURCE}\n{SC_SOURCE}");
    let b = MetalBackend::with_source(&source).expect("compile");
    let names = ["scMove", "scList", "scInsert", "scShade", "scResolve", "scZero"];
    let pipes: Vec<_> = names.iter().map(|n| b.pipeline(n, &[]).expect("pipeline")).collect();
    let (colors, pal) = color_params();
    let pal_buf = b.create_buffer_with_data(bytes_slice(&pal)).unwrap();
    const K: usize = 4;
    println!(
        "size        motion            move   list  insert  shade resolve | total ms (median of 120 frames, 3 rounds: min..max of medians)"
    );
    for &(w, h) in &[(1280usize, 720usize), (1920, 1080), (3456, 2160)] {
        let n = w * h;
        let samples = [
            b.create_buffer(n * K * 32, Storage::Private).unwrap(),
            b.create_buffer(n * K * 32, Storage::Private).unwrap(),
        ];
        let counts = [
            b.create_buffer(n * 4, Storage::Private).unwrap(),
            b.create_buffer(n * 4, Storage::Private).unwrap(),
        ];
        let list = b.create_buffer(n * 4, Storage::Private).unwrap();
        let list_len = b.create_buffer(16, Storage::Shared).unwrap();
        let colour = b.create_buffer(n * K * 8, Storage::Private).unwrap();
        let out = b
            .create_texture(w, h, TextureFormat::Rgba16Float, Storage::Private)
            .unwrap();
        let tg = (32usize, 8usize);
        for &(label, efolds, pan) in &[
            ("zoom 1.6/s", 1.6f64, 0.0f64),
            ("zoom 7.6/s", 7.6, 0.0),
            ("pan 13px", 0.0, 13.0),
        ] {
            let mut rounds: Vec<[f64; 6]> = Vec::new();
            for _round in 0..3 {
                let mut times: Vec<[f64; 5]> = Vec::new();
                let mut ping = 0;
                // Seed: empty cache, so the first frames fill every cell.
                for frame in 0..140u32 {
                    let a = (efolds / 60.0).exp() as f32;
                    let q = ScParams {
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
                    let mut t = [0.0f64; 5];
                    // Each pass in its own command buffer to time it.
                    let mut enc = b.begin("scMove").unwrap();
                    enc.set_pipeline(&pipes[0]);
                    enc.set_buffer(0, &samples[src], 0);
                    enc.set_buffer(1, &counts[src], 0);
                    enc.set_buffer(2, &samples[dst], 0);
                    enc.set_buffer(3, &counts[dst], 0);
                    enc.set_bytes(4, bytes(&q));
                    enc.dispatch_threads([w, h, 1], [tg.0, tg.1, 1]);
                    let s0 = enc.commit();
                    let mut enc = b.begin("scList").unwrap();
                    enc.set_pipeline(&pipes[5]);
                    enc.set_buffer(0, &list_len, 0);
                    enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
                    enc.set_pipeline(&pipes[1]);
                    enc.set_buffer(0, &counts[dst], 0);
                    enc.set_buffer(1, &list_len, 0);
                    enc.set_buffer(2, &list, 0);
                    enc.set_bytes(3, bytes(&q));
                    s0.wait();
                    t[0] = s0.gpu_time();
                    enc.dispatch_threads([w, h, 1], [tg.0, tg.1, 1]);
                    let s1 = enc.commit();
                    s1.wait();
                    t[1] = s1.gpu_time();
                    let mut enc = b.begin("scInsert").unwrap();
                    enc.set_pipeline(&pipes[2]);
                    enc.set_buffer(0, &list, 0);
                    enc.set_buffer(1, &list_len, 0);
                    enc.set_buffer(2, &samples[dst], 0);
                    enc.set_buffer(3, &counts[dst], 0);
                    enc.set_bytes(4, bytes(&q));
                    enc.dispatch_threads([n, 1, 1], [256, 1, 1]);
                    let s2 = enc.commit();
                    let mut enc = b.begin("scShade").unwrap();
                    enc.set_pipeline(&pipes[3]);
                    enc.set_buffer(0, &samples[dst], 0);
                    enc.set_buffer(1, &counts[dst], 0);
                    enc.set_buffer(2, &colour, 0);
                    enc.set_bytes(3, bytes(&q));
                    enc.set_bytes(4, bytes(&colors));
                    enc.set_buffer(5, &pal_buf, 0);
                    enc.dispatch_threads([w, h, 1], [tg.0, tg.1, 1]);
                    let s3 = enc.commit();
                    let mut enc = b.begin("scResolve").unwrap();
                    enc.set_pipeline(&pipes[4]);
                    enc.set_buffer(0, &samples[dst], 0);
                    enc.set_buffer(1, &counts[dst], 0);
                    enc.set_buffer(2, &colour, 0);
                    enc.set_bytes(3, bytes(&q));
                    enc.set_texture(0, &out);
                    enc.dispatch_threads([w, h, 1], [tg.0, tg.1, 1]);
                    let s4 = enc.commit();
                    s4.wait();
                    t[2] = s2.gpu_time();
                    t[3] = s3.gpu_time();
                    t[4] = s4.gpu_time();
                    ping = dst;
                    if frame >= 20 {
                        times.push(t);
                    }
                }
                let mut med = [0.0f64; 6];
                for p in 0..5 {
                    let mut v: Vec<f64> = times.iter().map(|t| t[p]).collect();
                    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    med[p] = v[v.len() / 2] * 1000.0;
                }
                let mut tot: Vec<f64> = times.iter().map(|t| t.iter().sum::<f64>()).collect();
                tot.sort_by(|a, b| a.partial_cmp(b).unwrap());
                med[5] = tot[tot.len() / 2] * 1000.0;
                rounds.push(med);
            }
            let best = |p: usize| rounds.iter().map(|r| r[p]).fold(f64::MAX, f64::min);
            let worst = |p: usize| rounds.iter().map(|r| r[p]).fold(0.0, f64::max);
            println!(
                "{w}x{h:<6} {label:<14} {:6.3} {:6.3} {:6.3} {:6.3} {:6.3} | {:6.3} .. {:6.3}",
                best(0),
                best(1),
                best(2),
                best(3),
                best(4),
                best(5),
                worst(5)
            );
        }
    }
}

pub fn bytes_slice<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: plain f32 data read as bytes.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}
