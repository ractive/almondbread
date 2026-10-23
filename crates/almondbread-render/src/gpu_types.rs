//! `#[repr(C)]` mirrors of the structs in `shaders/mandelbrot.metal`. Field order and types
//! must match; the layout is pinned by compile-time assertions below (MSL `float2`: size 8,
//! align 8; `float4`: size 16, align 16; `uint2`: size 8, align 8).

use bytemuck::{Pod, Zeroable};
use core::mem::{offset_of, size_of};

/// Parameters of one iterate chunk (`IterateParams` in MSL), 136 bytes.
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
    /// M5: 1 = admit fresh pixels only once the level's seed round reported every node final.
    pub seed_gate: u32,
    /// Deep kernel (M3b): `texel_scale` and `ref_offset` are mantissas in units of
    /// 2^`scale_exp`. 0 for the plain kernel.
    pub scale_exp: i32,
    /// F4 Julia references (kb: plans/f4-deep-julia, `almondbread_core::julia_reference`): the
    /// critical orbit's last index (0 otherwise).
    pub crit_last: u32,
    /// View rotation `(cos, sin)` applied to the texel offset before scaling (cinematic
    /// rotation, kb: decisions/controls "View rotation"); `(1, 0)` unrotated. Every
    /// constructor must set it (`[0, 0]` would collapse the view to a point).
    pub rot: [f32; 2],
    /// M5 interior fill: admission indices of the tile borders (tiles x 60; 0: off) and the
    /// level's stamp in the tile-bad buffer.
    pub fill_border: u32,
    pub fill_epoch: u32,
    /// A border pixel counts as interior when proven within this many iterations.
    pub fill_proof: u32,
    /// 1: the reference point lies on the real axis (its orbit is real). M5 seeds are then not
    /// used within 16 texels of the axis, where the escape depends on `Im c` alone (kb:
    /// techniques/local-series-approximation "Real axis").
    pub real_axis: u32,
    /// Interior probe (kb: techniques/interior-detection "Attracting-cycle probe"): a pixel
    /// still unresolved at `max_iter` searches its orbit for an attracting cycle for at most
    /// this many more steps; 0: off.
    pub probe: u32,
    /// The probe state buffer half this chunk reads (it writes the other).
    pub probe_half: u32,
    /// F4 Julia references: the index where the centre orbit starts (fresh pixels start
    /// there); 0 otherwise.
    pub center_start: u32,
    pub pad1: u32,
}

/// Closed-form interior test of the main cardioid and the period-2 bulb for one level
/// (`ComponentParams` in MSL, iterate binding 7; kb: techniques/interior-detection
/// "Closed-form component test"; built by `almondbread_core::components`).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ComponentParams {
    /// c at texel offset (0, 0) and per texel along u and v, float32 (colouring only).
    pub anchor: [f32; 2],
    pub ex: [f32; 2],
    pub ey: [f32; 2],
    /// Normalised polynomials in (u / L, v / L) (monomial order of `components::MONOMIALS`).
    pub cardioid: [f32; 15],
    pub bulb: [f32; 15],
    /// Absolute margins of the two tests.
    pub slack: [f32; 2],
    /// 1 / L and L (powers of two).
    pub inv_l: f32,
    pub l: f32,
    /// log2 of the texel size (interior distance in texels).
    pub log2_texel: f32,
    /// Bit 0: cardioid, bit 1: period-2 bulb; 0: no test.
    pub flags: u32,
    /// Component Newton (kb: techniques/interior-detection "Component Newton"): periods of the
    /// attracting cycles found at sample points of the view, most samples first
    /// (`almondbread_core::cycles`); the interior probe tries Newton from z = 0 for each.
    pub periods: [u32; 4],
    pub period_count: u32,
    /// Probe steps per pixel added for those Newton tries.
    pub seed_steps: u32,
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

/// Interior probe state of one live pixel past the limit (`PixelStateProbe` in MSL; one buffer
/// of two halves of `capacity` entries, binding 30), 48 bytes: d²f^k/dz² at the base point
/// (floatexp), the base point's reference index, steps since it, and Newton flag | passes |
/// lead or period. kb: techniques/interior-detection "Attracting-cycle probe".
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct PixelStateProbe {
    pub zz: [f32; 2],
    pub zz_exp: i32,
    pub m_b: u32,
    pub k: u32,
    pub info: u32,
    /// Newton passes: the scan's orbit point (delta, exponent, reference index) to resume
    /// from if they fail, and the rejected period (candidates must be longer; during Component
    /// Newton the bits of log2 of the last step).
    pub o_b: [f32; 2],
    pub o_b_exp: i32,
    pub o_m: u32,
    pub p_rej: u32,
    /// Component Newton: the period candidate tried (bits 0..2) and the probe step at which
    /// the scan started (bits 3..31).
    pub seed: u32,
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
    /// Smallest iteration count n of the pixels still live after the chunk (`u32::MAX`: none):
    /// how far the slowest pixels have got (render progress).
    pub live_min_n: u32,
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
    /// Palette hue turn `(cos, sin)` in OKLab (cinematic hue drift); `(0, 0)` or `(1, 0)`: none.
    pub hue_rot: [f32; 2],
    /// Dense regions (calm look): the deep colour (linear RGB, w = 1 on, 0 the old mean grey)
    /// and the filament colour.
    pub deep: [f32; 4],
    pub filament: [f32; 4],
    /// M5 interior fill: the main level's admission order has the tile borders first.
    pub fill_border: u32,
    /// Fog over unresolved pixels (kb: techniques/unresolved-fog): levels of the fog pyramid
    /// bound with this pass (0: none, the plain deep fog), and display points per iteration
    /// texel.
    pub fog_levels: u32,
    pub fog_points: f32,
    /// 1: unresolved pixels draw as the interior colour, not the fog
    /// (`FormulaCapabilities::bounded_interior`, the Burning Ship).
    pub bounded_interior: u32,
}

/// Parameters of the `fogDown` kernel (fog pyramid), 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FogParams {
    pub src: [u32; 2],
    pub level: u32,
    pub pad: u32,
}

/// Parameters of the `compose` kernel (FL: history reprojection, temporal upsampling,
/// refinement fade, AA accumulation), 88 bytes.
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
    /// Rotation `(cos, sin)` of output pixel offsets into history pixel offsets (texture
    /// axes, y down): the history was composed at another view angle. `(1, 0)`: none.
    pub hist_rot: [f32; 2],
    /// Mode 2 (AA): the palette filter's width of a sample in output pixels, `1/sqrt(samples)`
    /// (its share of the pixel; filtering each over the whole pixel greyed fine rings).
    pub sample_width: f32,
    pub pad: u32,
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

/// Parameters of the `unresolvedList` kernel ("compute further" segments), 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct UnresolvedListParams {
    pub width: u32,
    pub height: u32,
    pub tiles_x: u32,
    pub tile_order: u32,
    pub start: u32,
    pub count: u32,
    pub capacity: u32,
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
    assert!(size_of::<UnresolvedListParams>() == 32);
    assert!(size_of::<IterateParams>() == 144);
    assert!(size_of::<ComponentParams>() == 192);
    assert!(offset_of!(ComponentParams, cardioid) == 24);
    assert!(offset_of!(ComponentParams, slack) == 144);
    assert!(offset_of!(ComponentParams, flags) == 164);
    assert!(offset_of!(IterateParams, probe) == 128);
    assert!(offset_of!(IterateParams, fill_border) == 112);
    assert!(offset_of!(IterateParams, scale_exp) == 96);
    assert!(offset_of!(IterateParams, rot) == 104);
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
    assert!(size_of::<PixelStateProbe>() == 48);
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

    assert!(size_of::<ColorParams>() == 240);
    assert!(offset_of!(ColorParams, fill_border) == 224);
    assert!(offset_of!(ColorParams, fog_levels) == 228);
    assert!(offset_of!(ColorParams, fog_points) == 232);
    assert!(size_of::<FogParams>() == 16);
    assert!(offset_of!(ColorParams, deep) == 192);
    assert!(offset_of!(ColorParams, interior_mode) == 172);
    assert!(offset_of!(ColorParams, hue_rot) == 184);
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

    assert!(size_of::<ComposeParams>() == 88);
    assert!(offset_of!(ComposeParams, sample_width) == 80);
    assert!(offset_of!(ComposeParams, hist_size) == 16);
    assert!(offset_of!(ComposeParams, new_ratio) == 40);
    assert!(offset_of!(ComposeParams, mode) == 60);
    assert!(offset_of!(ComposeParams, out_centre) == 64);
    assert!(offset_of!(ComposeParams, hist_rot) == 72);
};

/// M5 (kb: techniques/local-series-approximation): one quadtree seed's branch state and
/// resumable loop state (`SeedNode` in MSL), 88 bytes. Plain kernel: `b` is d²z/dc² · texel,
/// exponents 0; deep kernel: floatexp mantissas with their exponents.
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
    /// `epoch << 2 | state` (0 new, 1 running, 2 done).
    pub stamp: u32,
    /// Accepted seed steps from the root to this node (what a pixel starting here saves).
    pub path: u32,
    /// Interior colouring: log2 of the smallest |z| so far and its iteration.
    pub ic_min: f32,
    pub ic_q: u32,
}

/// One quadtree level of seeds (`SeedParams` in MSL), 80 bytes; the leaf level's copy tells
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
    /// This seeding (node stamps).
    pub epoch: u32,
    /// Seed steps per node in one pass.
    pub budget: u32,
    /// The seeded level's sample position inside a texel.
    pub jitter: [f32; 2],
    pub tiles_x: u32,
    pub pad0: u32,
    pub gate_rel: f32,
    pub gate_abs: f32,
    pub tiles_y: u32,
    /// 1: a node out of budget branches where it is (gesture frames: one bounded round).
    pub truncate: u32,
}

/// The seed round's status (`SeedStatus` in MSL).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct SeedStatus {
    pub unfinished: u32,
    pub seeded_tiles: u32,
    pub path_lo: u32,
    pub path_hi: u32,
}

const _: () = {
    assert!(size_of::<SeedNode>() == 88);
    assert!(offset_of!(SeedNode, w) == 48);
    assert!(offset_of!(SeedNode, dz_exp) == 24);
    assert!(offset_of!(SeedNode, m) == 36);
    assert!(offset_of!(SeedNode, stamp) == 72);
    assert!(size_of::<SeedParams>() == 80);
    assert!(offset_of!(SeedParams, radius) == 32);
    assert!(offset_of!(SeedParams, jitter) == 48);
    assert!(size_of::<SeedStatus>() == 16);
};
