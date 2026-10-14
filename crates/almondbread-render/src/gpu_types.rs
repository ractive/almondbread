//! `#[repr(C)]` mirrors of the structs in `shaders/mandelbrot.metal`. Field order and types
//! must match; the layout is pinned by compile-time assertions below (MSL `float2`: size 8,
//! align 8; `float4`: size 16, align 16; `uint2`: size 8, align 8).

use bytemuck::{Pod, Zeroable};
use core::mem::{offset_of, size_of};

/// Parameters of one iterate chunk (`IterateParams` in MSL), 80 bytes.
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
    /// 0 scanline, 1 scanline + collect unresolved, 2 admit from the list.
    pub list_mode: u32,
    pub list_capacity: u32,
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
    pub pad1: u32,
    /// Escapes at n >= softIter << k.
    pub escaped_past: [u32; 4],
}

impl ChunkStats {
    pub fn iterations(&self) -> u64 {
        (u64::from(self.iterations_hi) << 32) | u64::from(self.iterations_lo)
    }
}

/// Admission state of the running level (GPU only), 16 bytes.
pub const LEVEL_COUNTERS_SIZE: usize = 16;

/// Colorize parameters (`ColorParams` in MSL), 128 bytes.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ColorParams {
    pub interior: [f32; 4],
    pub edge: [f32; 4],
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
    pub pad: [f32; 2],
}

const _: () = {
    assert!(size_of::<IterateParams>() == 80);
    assert!(offset_of!(IterateParams, texel_scale) == 24);
    assert!(offset_of!(IterateParams, max_iter) == 32);
    assert!(offset_of!(IterateParams, chunk_iters) == 40);
    assert!(offset_of!(IterateParams, stat_slot) == 56);
    assert!(offset_of!(IterateParams, interior_eps) == 60);
    assert!(offset_of!(IterateParams, first_chunk) == 64);
    assert!(offset_of!(IterateParams, soft_iter) == 68);
    assert!(offset_of!(IterateParams, list_mode) == 72);
    assert!(offset_of!(IterateParams, list_capacity) == 76);

    assert!(size_of::<PixelState>() == 40);
    assert!(offset_of!(PixelState, pixel) == 24);
    assert!(offset_of!(PixelState, w_exp) == 36);

    assert!(size_of::<ChunkStats>() == 56);
    assert!(offset_of!(ChunkStats, iterations_hi) == 12);
    assert!(offset_of!(ChunkStats, admitted) == 20);
    assert!(offset_of!(ChunkStats, interior) == 24);
    assert!(offset_of!(ChunkStats, max_escape) == 32);
    assert!(offset_of!(ChunkStats, escaped_past) == 40);

    assert!(size_of::<ColorParams>() == 128);
    assert!(offset_of!(ColorParams, edge) == 16);
    assert!(offset_of!(ColorParams, light) == 32);
    assert!(offset_of!(ColorParams, density) == 40);
    assert!(offset_of!(ColorParams, palette_size) == 64);
    assert!(offset_of!(ColorParams, admitted) == 76);
    assert!(offset_of!(ColorParams, has_under) == 80);
    assert!(offset_of!(ColorParams, linear_out) == 84);
    assert!(offset_of!(ColorParams, admitted_on_gpu) == 88);
    assert!(offset_of!(ColorParams, de_scale) == 92);
    assert!(offset_of!(ColorParams, src_size) == 96);
    assert!(offset_of!(ColorParams, out_size) == 104);
    assert!(offset_of!(ColorParams, under_size) == 112);
};
