//! `#[repr(C)]` mirrors of the structs in `shaders/mandelbrot.metal`. Field order and types
//! must match; the layout is pinned by compile-time assertions below (MSL `float2`: size 8,
//! align 8; `float4`: size 16, align 16; `uint2`: size 8, align 8).

use bytemuck::{Pod, Zeroable};
use core::mem::{offset_of, size_of};

/// Parameters of one iterate chunk (`IterateParams` in MSL), 112 bytes.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct IterateParams {
    pub half_size: [f32; 2],
    pub ref_offset: [f32; 2],
    pub jitter: [f32; 2],
    pub texel_scale: f32,
    pub bailout2: f32,
    /// The level's limit (an extension level: the hard limit).
    pub max_iter: u32,
    pub ref_len: u32,
    pub chunk_iters: u32,
    pub width: u32,
    pub total: u32,
    pub capacity: u32,
    pub stat_slot: u32,
    pub interior_eps: f32,
    pub first_chunk: u32,
    /// Nominal limit.
    pub soft_iter: u32,
    /// 0 admission order, 1 the same + collect unresolved, 2 admit from the list.
    pub list_mode: u32,
    pub list_capacity: u32,
    /// 16x16 tiles per row; admission index a -> tile rank a / 256 (FL).
    pub tiles_x: u32,
    pub height: u32,
    /// 0: raster tile order, 1: the order buffer.
    pub tile_order: u32,
    pub pad0: u32,
    /// Deep kernel (M3b): `texel_scale` and `ref_offset` are mantissas in units of
    /// 2^`scale_exp`. 0 for the plain kernel.
    pub scale_exp: i32,
    pub pad1: u32,
    pub pad2: u32,
    pub pad3: u32,
}

/// Loop state of one live pixel; only allocated on the GPU (sizes the state buffers).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct PixelState {
    pub dz: [f32; 2],
    pub d: [f32; 2],
    pub w: [f32; 2],
    pub pixel: u32,
    pub n: u32,
    pub m: u32,
    pub w_exp: i32,
    /// Deep kernel: delta and DE exponents.
    pub e: i32,
    pub d_exp: i32,
}

/// Interior cycle analysis state of one live pixel (`PixelStateIC` in MSL, kInteriorColor
/// pipelines only; a parallel buffer to the live state, same slots), 48 bytes. Complex values
/// are floatexp (mantissa, exponent). kb: techniques/coloring-methods "Interior colouring".
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct PixelStateIC {
    /// Orbit point the cycle search started at (full z).
    pub zs: [f32; 2],
    pub zs_exp: i32,
    /// Analysis steps done.
    pub k: u32,
    /// d²/dz² of f^k at zs.
    pub dzdz: [f32; 2],
    pub dzdz_exp: i32,
    /// log2 of the smallest |z| over the analysed steps.
    pub min_log2: f32,
    /// d²/dc dz of f^k (times the texel size).
    pub dcdz: [f32; 2],
    pub dcdz_exp: i32,
    /// Step limit (bits 0..29), second attempt (bit 30), analysing (bit 31).
    pub cap_flags: u32,
}

/// Per-chunk counters the CPU reads back (a 64-slot ring).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ChunkStats {
    pub live_in: u32,
    pub live_out: u32,
    pub iterations_lo: u32,
    pub iterations_hi: u32,
    pub fresh: u32,
    pub admitted: u32,
    pub interior: u32,
    /// Finished at maxIter unresolved.
    pub crossed: u32,
    pub max_escape: u32,
    /// Loop steps (BLA steps + perturbation steps): the work actually done.
    pub steps_lo: u32,
    /// Escapes at n >= softIter << k.
    pub escaped_past: [u32; 4],
    pub steps_hi: u32,
    pub pad2: u32,
}

impl ChunkStats {
    pub fn iterations(&self) -> u64 {
        (u64::from(self.iterations_hi) << 32) | u64::from(self.iterations_lo)
    }

    pub fn steps(&self) -> u64 {
        (u64::from(self.steps_hi) << 32) | u64::from(self.steps_lo)
    }
}

/// BLA table layout (`BlaParams` in MSL), 144 bytes; entries are `[f32; 6]`
/// (`almondbread_core::bla::BlaEntry`, `BlaEntry` in MSL: A, B, R, K).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct BlaParams {
    pub min_level: u32,
    pub max_level: u32,
    /// Lookups at m = 1 (mod lookup_every), a power of two >= 2^min_level.
    pub lookup_every: u32,
    pub pad1: u32,
    pub offset: [u32; 32],
}

/// Admission state of the running level (GPU only), 16 bytes.
pub const LEVEL_COUNTERS_SIZE: usize = 16;

/// Colorize parameters (`ColorParams` in MSL), 224 bytes.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ColorParams {
    pub interior: [f32; 4],
    pub edge: [f32; 4],
    pub glow_color: [f32; 4],
    pub light: [f32; 2],
    pub density: f32,
    pub offset: f32,
    pub ratio: f32,
    pub shading: f32,
    pub outline: f32,
    pub outline_width: f32,
    pub palette_size: u32,
    pub log_scale: u32,
    pub smooth: u32,
    pub admitted: u32,
    pub has_under: f32,
    pub linear_out: u32,
    pub admitted_on_gpu: u32,
    pub de_scale: f32,
    pub src_size: [u32; 2],
    pub out_size: [u32; 2],
    pub under_size: [f32; 2],
    pub tiles_x: u32,
    pub tile_order: u32,
    pub specular: f32,
    pub shininess: f32,
    pub light_height: f32,
    pub glow: f32,
    pub glow_width: f32,
    pub palette_levels: u32,
    pub palette_filter: f32,
    /// Interior colouring (`InteriorMode::code`; 0 also when the textures hold no analysis).
    pub interior_mode: u32,
    pub interior_density: f32,
    pub interior_stripes: f32,
    pub pad2: [f32; 2],
    /// Dense regions (calm look): the deep colour (linear RGB, w = 1 on, 0 the old mean grey)
    /// and the filament colour.
    pub deep: [f32; 4],
    pub filament: [f32; 4],
}

/// Parameters of the `compose` kernel (FL: history reprojection, temporal upsampling,
/// refinement fade, AA accumulation), 80 bytes.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ComposeParams {
    pub hist_origin: [f32; 2],
    pub hist_step: f32,
    pub hist_decay: f32,
    pub hist_size: [f32; 2],
    pub jitter: [f32; 2],
    pub new_centre: [f32; 2],
    pub new_ratio: f32,
    pub conf_cap: f32,
    pub base_weight: f32,
    pub inv_two_sigma2: f32,
    pub blend: f32,
    pub mode: u32,
    pub out_centre: [f32; 2],
    pub pad: [f32; 2],
}

/// Parameters of the `aaList` kernel (adaptive anti-aliasing), 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct AaListParams {
    pub width: u32,
    pub height: u32,
    pub tiles_x: u32,
    pub tile_order: u32,
    pub total: u32,
    pub capacity: u32,
    pub de_threshold: f32,
    pub pad: u32,
}

/// Parameters of the `survey` kernel (autopilot), 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SurveyParams {
    pub width: u32,
    pub height: u32,
    pub cols: u32,
    pub rows: u32,
}

const _: () = {
    assert!(size_of::<SurveyParams>() == 16);
    assert!(size_of::<AaListParams>() == 32);
    assert!(size_of::<IterateParams>() == 112);
    assert!(offset_of!(IterateParams, scale_exp) == 96);
    assert!(offset_of!(IterateParams, tiles_x) == 80);
    assert!(offset_of!(IterateParams, tile_order) == 88);
    assert!(offset_of!(IterateParams, texel_scale) == 24);
    assert!(offset_of!(IterateParams, max_iter) == 32);
    assert!(offset_of!(IterateParams, chunk_iters) == 40);
    assert!(offset_of!(IterateParams, stat_slot) == 56);
    assert!(offset_of!(IterateParams, interior_eps) == 60);
    assert!(offset_of!(IterateParams, first_chunk) == 64);
    assert!(offset_of!(IterateParams, soft_iter) == 68);
    assert!(offset_of!(IterateParams, list_mode) == 72);
    assert!(offset_of!(IterateParams, list_capacity) == 76);

    assert!(size_of::<PixelState>() == 48);
    assert!(offset_of!(PixelState, e) == 40);
    assert!(offset_of!(PixelState, pixel) == 24);
    assert!(offset_of!(PixelState, w_exp) == 36);

    assert!(size_of::<ChunkStats>() == 64);
    assert!(offset_of!(ChunkStats, steps_lo) == 36);
    assert!(offset_of!(ChunkStats, steps_hi) == 56);
    assert!(size_of::<BlaParams>() == 144);
    assert!(offset_of!(BlaParams, offset) == 16);
    assert!(offset_of!(ChunkStats, iterations_hi) == 12);
    assert!(offset_of!(ChunkStats, admitted) == 20);
    assert!(offset_of!(ChunkStats, interior) == 24);
    assert!(offset_of!(ChunkStats, max_escape) == 32);
    assert!(offset_of!(ChunkStats, escaped_past) == 40);

    assert!(size_of::<PixelStateIC>() == 48);
    assert!(offset_of!(PixelStateIC, dzdz) == 16);
    assert!(offset_of!(PixelStateIC, dcdz) == 32);
    assert!(offset_of!(PixelStateIC, cap_flags) == 44);

    assert!(size_of::<ColorParams>() == 224);
    assert!(offset_of!(ColorParams, deep) == 192);
    assert!(offset_of!(ColorParams, interior_mode) == 172);
    assert!(offset_of!(ColorParams, pad2) == 184);
    assert!(offset_of!(ColorParams, edge) == 16);
    assert!(offset_of!(ColorParams, glow_color) == 32);
    assert!(offset_of!(ColorParams, light) == 48);
    assert!(offset_of!(ColorParams, density) == 56);
    assert!(offset_of!(ColorParams, palette_size) == 80);
    assert!(offset_of!(ColorParams, admitted) == 92);
    assert!(offset_of!(ColorParams, has_under) == 96);
    assert!(offset_of!(ColorParams, linear_out) == 100);
    assert!(offset_of!(ColorParams, admitted_on_gpu) == 104);
    assert!(offset_of!(ColorParams, de_scale) == 108);
    assert!(offset_of!(ColorParams, src_size) == 112);
    assert!(offset_of!(ColorParams, out_size) == 120);
    assert!(offset_of!(ColorParams, under_size) == 128);
    assert!(offset_of!(ColorParams, tiles_x) == 136);
    assert!(offset_of!(ColorParams, specular) == 144);
    assert!(offset_of!(ColorParams, palette_levels) == 164);

    assert!(size_of::<ComposeParams>() == 80);
    assert!(offset_of!(ComposeParams, hist_size) == 16);
    assert!(offset_of!(ComposeParams, new_ratio) == 40);
    assert!(offset_of!(ComposeParams, mode) == 60);
    assert!(offset_of!(ComposeParams, out_centre) == 64);
};

/// R5 prototype (kb: techniques/local-series-approximation): one quadtree seed's branch
/// state (`SeedNode` in MSL), 72 bytes. Plain kernel: `b` is d²z/dc² · texel, exponents 0;
/// deep kernel: floatexp mantissas with their exponents.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SeedNode {
    pub dz: [f32; 2],
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub dz_exp: i32,
    pub a_exp: i32,
    pub b_exp: i32,
    pub m: u32,
    pub n: u32,
    /// Seed steps this node executed (accepted ones plus the rejected last).
    pub steps: u32,
    /// Interior test: w = dz_n/dz_1 and dw/dc · texel (floatexp mantissas, exponents).
    pub w: [f32; 2],
    pub wd: [f32; 2],
    pub w_exp: i32,
    pub wd_exp: i32,
}

/// One quadtree level of seeds (`SeedParams` in MSL), 48 bytes; the leaf level's copy tells
/// the iterate kernel where a pixel's node is.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SeedParams {
    pub grid_w: u32,
    pub grid_h: u32,
    pub size: u32,
    pub offset: u32,
    pub parent_grid_w: u32,
    pub parent_offset: u32,
    pub root: u32,
    pub soft_iter: u32,
    pub radius: f32,
    pub eps: f32,
    pub pad0: u32,
    pub pad1: u32,
}

const _: () = {
    assert!(size_of::<SeedNode>() == 72);
    assert!(offset_of!(SeedNode, w) == 48);
    assert!(offset_of!(SeedNode, dz_exp) == 24);
    assert!(offset_of!(SeedNode, m) == 36);
    assert!(size_of::<SeedParams>() == 48);
    assert!(offset_of!(SeedParams, radius) == 32);
};
