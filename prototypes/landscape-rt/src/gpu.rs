//! Metal on objc2-metal (the stack of crates/almondbread-render/src/metal; kb: decisions/landscape-mode
//! L1 for why not wgpu). Owns the device, the queue, the pipelines, the clipmap textures and the
//! MSAA targets; encodes one compute pass (DE tiles) and one render pass (sky + rings) per frame.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use block2::RcBlock;
use bytemuck::{Pod, Zeroable};
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {}

pub const SHADER: &str = include_str!("shaders/landscape.metal");
pub const MSAA: usize = 4;
/// Tiles that can be in flight (computing or waiting to resume).
pub const STATE_SLOTS: usize = 8192;

pub type Device = Retained<ProtocolObject<dyn MTLDevice>>;
pub type Buffer = Retained<ProtocolObject<dyn MTLBuffer>>;
pub type Texture = Retained<ProtocolObject<dyn MTLTexture>>;
pub type CommandBuffer = Retained<ProtocolObject<dyn MTLCommandBuffer>>;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
pub struct DeParams {
    pub ref_len: u32,
    pub max_iter: u32,
    pub bailout2: f32,
    pub interior_eps: f32,
    pub now: f32,
    pub job_count: u32,
    pub chunk: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
pub struct TileJob {
    pub dc0: [f32; 2],
    pub spacing: f32,
    pub slot: u32,
    pub texel: [u32; 2],
    pub tag: f32,
    /// state slot; high bit set: resume
    pub state: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
pub struct Globals {
    pub view_proj: [[f32; 4]; 4], // columns
    pub sun: [f32; 4],
    pub eye: [f32; 4],
    pub fwd: [f32; 4],
    pub right: [f32; 4],
    pub up: [f32; 4],
    pub hcurve: [f32; 4],
    pub misc: [f32; 4],
    pub shade: [f32; 4],
    pub shade2: [f32; 4],
    pub overlay: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
pub struct LevelRef {
    pub data_mod: [i32; 2],
    pub data_div: [i32; 2],
    pub ring_origin: [i32; 2],
    pub sp_over_s: f32,
    pub slot: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
pub struct RingParams {
    pub lv: [LevelRef; 6],
    pub level_bits: [u32; 8],
    pub pos0: [f32; 2],
    pub g_s: f32,
    pub k: i32,
    pub alpha_start: f32,
    pub alpha_width: f32,
    pub block_verts: i32,
    pub pad: i32,
}

const _: () = assert!(std::mem::size_of::<TileJob>() == 32);
const _: () = assert!(std::mem::size_of::<LevelRef>() == 32);
const _: () = assert!(std::mem::size_of::<RingParams>() == 256);
const _: () = assert!(std::mem::size_of::<Globals>() == 224);

/// One ring's draw: its parameters and its visible blocks (offset/count into the frame's block list).
pub struct RingDraw {
    pub params: RingParams,
    pub first_block: usize,
    pub blocks: usize,
}

/// GPU times of finished command buffers, filled from Metal's completion handlers.
#[derive(Default)]
pub struct GpuTimes {
    /// (frame serial, kind 0 = compute 1 = render, seconds)
    pub done: Vec<(u64, u8, f64)>,
}

pub struct Gpu {
    pub device: Device,
    pub queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    de_pipeline: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    terrain_pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    sky_pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    depth_state: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    sky_depth_state: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    pub data_tex: Texture,
    pub meta_tex: Texture,
    index_buf: Buffer,
    index_count: usize,
    ref_buf: Option<Buffer>,
    /// loop state of unfinished tiles: STATE_SLOTS × 256 × 48 bytes
    state_buf: Buffer,
    pub ref_len: u32,
    msaa_color: Option<Texture>,
    depth: Option<Texture>,
    target_size: (usize, usize),
    pub times: Arc<Mutex<GpuTimes>>,
    pub color_format: MTLPixelFormat,
}

fn size3(w: usize, h: usize, d: usize) -> MTLSize {
    MTLSize { width: w, height: h, depth: d }
}

impl Gpu {
    pub fn new(n: usize, slots: usize, block_verts: usize) -> Result<Self, String> {
        autoreleasepool(|_| {
            let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
            let queue = device.newCommandQueue().ok_or("no command queue")?;
            let options = MTLCompileOptions::new();
            options.setLanguageVersion(MTLLanguageVersion::Version3_0);
            #[allow(deprecated)]
            options.setFastMathEnabled(true);
            let library = device
                .newLibraryWithSource_options_error(&NSString::from_str(SHADER), Some(&options))
                .map_err(|e| format!("shader compile failed: {}", e.localizedDescription()))?;
            let func = |name: &str| {
                library
                    .newFunctionWithName(&NSString::from_str(name))
                    .ok_or_else(|| format!("missing function {name}"))
            };
            let de_pipeline = device
                .newComputePipelineStateWithFunction_error(&*func("deTiles")?)
                .map_err(|e| format!("deTiles pipeline: {}", e.localizedDescription()))?;
            let color_format = MTLPixelFormat::BGRA8Unorm;
            let make_rp = |vs: &str, fs: &str| -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>, String> {
                let d = MTLRenderPipelineDescriptor::new();
                d.setVertexFunction(Some(&*func(vs)?));
                d.setFragmentFunction(Some(&*func(fs)?));
                // SAFETY: index 0 exists on every descriptor.
                let ca = unsafe { d.colorAttachments().objectAtIndexedSubscript(0) };
                ca.setPixelFormat(color_format);
                d.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);
                d.setRasterSampleCount(MSAA);
                device
                    .newRenderPipelineStateWithDescriptor_error(&d)
                    .map_err(|e| format!("{vs} pipeline: {}", e.localizedDescription()))
            };
            let terrain_pipeline = make_rp("terrainVertex", "terrainFragment")?;
            let sky_pipeline = make_rp("skyVertex", "skyFragment")?;
            let ds = MTLDepthStencilDescriptor::new();
            ds.setDepthCompareFunction(MTLCompareFunction::Greater); // reversed Z
            ds.setDepthWriteEnabled(true);
            let depth_state = device.newDepthStencilStateWithDescriptor(&ds).ok_or("depth state")?;
            let ds2 = MTLDepthStencilDescriptor::new();
            ds2.setDepthCompareFunction(MTLCompareFunction::Always);
            ds2.setDepthWriteEnabled(false);
            let sky_depth_state = device.newDepthStencilStateWithDescriptor(&ds2).ok_or("depth state")?;

            let tex = |fmt: MTLPixelFormat, size: usize, write: bool| -> Result<Texture, String> {
                let d = MTLTextureDescriptor::new();
                d.setTextureType(MTLTextureType::Type2DArray);
                d.setPixelFormat(fmt);
                // SAFETY: plain sizes within device limits (checked by Metal).
                unsafe {
                    d.setWidth(size);
                    d.setHeight(size);
                    d.setArrayLength(slots);
                }
                d.setStorageMode(MTLStorageMode::Private);
                let mut usage = MTLTextureUsage::ShaderRead;
                if write {
                    usage |= MTLTextureUsage::ShaderWrite;
                }
                d.setUsage(usage);
                device.newTextureWithDescriptor(&d).ok_or_else(|| "texture alloc".to_string())
            };
            // R32Float: R16F quantised log2(DE/spacing) in steps of 2^-7 at the finest levels, which
            // showed as terraces of ~0.002 S when flying low over plateaus
            if !device.supports32BitFloatFiltering() {
                return Err("this GPU cannot filter 32-bit float textures".into());
            }
            let data_tex = tex(MTLPixelFormat::R32Float, n, true)?;
            let meta_tex = tex(MTLPixelFormat::RG32Float, n / 16, true)?;

            // one block: block_verts² quads over (block_verts+1)² vertices, diagonal (0,0)-(1,1)
            let b = block_verts as u16;
            let mut idx: Vec<u16> = Vec::new();
            for j in 0..b {
                for i in 0..b {
                    let v00 = j * (b + 1) + i;
                    let v10 = v00 + 1;
                    let v01 = v00 + (b + 1);
                    let v11 = v01 + 1;
                    idx.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
                }
            }
            let index_buf = new_buffer(&device, bytemuck::cast_slice(&idx))?;
            let state_buf = device
                .newBufferWithLength_options(STATE_SLOTS * 256 * 48, MTLResourceOptions::StorageModePrivate)
                .ok_or("state buffer")?;

            let mut gpu = Gpu {
                device,
                queue,
                de_pipeline,
                terrain_pipeline,
                sky_pipeline,
                depth_state,
                sky_depth_state,
                data_tex,
                meta_tex,
                index_buf,
                index_count: idx.len(),
                ref_buf: None,
                state_buf,
                ref_len: 0,
                msaa_color: None,
                depth: None,
                target_size: (0, 0),
                times: Arc::new(Mutex::new(GpuTimes::default())),
                color_format,
            };
            gpu.clear_meta(n / 16, slots)?;
            Ok(gpu)
        })
    }

    /// Zero the meta textures (tag 0 never matches: level bits are offset by 1).
    fn clear_meta(&mut self, size: usize, slots: usize) -> Result<(), String> {
        let zeros = vec![0u8; size * size * 8];
        let staging = new_buffer(&self.device, &zeros)?;
        let cb = self.queue.commandBuffer().ok_or("cb")?;
        let blit = cb.blitCommandEncoder().ok_or("blit")?;
        for s in 0..slots {
            // SAFETY: the buffer holds a full slice; region within the texture.
            unsafe {
                blit.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                    &staging,
                    0,
                    size * 8,
                    size * size * 8,
                    size3(size, size, 1),
                    &self.meta_tex,
                    s,
                    0,
                    MTLOrigin { x: 0, y: 0, z: 0 },
                );
            }
        }
        blit.endEncoding();
        cb.commit();
        cb.waitUntilCompleted();
        Ok(())
    }

    pub fn set_reference(&mut self, orbit: &[[f32; 2]]) -> Result<(), String> {
        self.ref_buf = Some(new_buffer(&self.device, bytemuck::cast_slice(orbit))?);
        self.ref_len = orbit.len() as u32;
        Ok(())
    }

    fn ensure_targets(&mut self, w: usize, h: usize) -> Result<(), String> {
        if self.target_size == (w, h) && self.msaa_color.is_some() {
            return Ok(());
        }
        let mk = |fmt: MTLPixelFormat| -> Result<Texture, String> {
            let d = MTLTextureDescriptor::new();
            d.setTextureType(MTLTextureType::Type2DMultisample);
            d.setPixelFormat(fmt);
            // SAFETY: plain sizes.
            unsafe {
                d.setWidth(w);
                d.setHeight(h);
                d.setSampleCount(MSAA);
            }
            d.setStorageMode(MTLStorageMode::Memoryless);
            d.setUsage(MTLTextureUsage::RenderTarget);
            self.device.newTextureWithDescriptor(&d).ok_or_else(|| "msaa alloc".to_string())
        };
        self.msaa_color = Some(mk(self.color_format)?);
        self.depth = Some(mk(MTLPixelFormat::Depth32Float)?);
        self.target_size = (w, h);
        Ok(())
    }

    /// A shared BGRA texture to render into offscreen (recorder) and read back.
    pub fn offscreen_target(&self, w: usize, h: usize) -> Result<Texture, String> {
        let d = MTLTextureDescriptor::new();
        d.setPixelFormat(self.color_format);
        // SAFETY: plain sizes.
        unsafe {
            d.setWidth(w);
            d.setHeight(h);
        }
        d.setStorageMode(MTLStorageMode::Shared);
        d.setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
        self.device.newTextureWithDescriptor(&d).ok_or_else(|| "target alloc".to_string())
    }

    pub fn command_buffer(&self) -> CommandBuffer {
        self.queue.commandBuffer().expect("command buffer")
    }

    /// Records the GPU time of `cb` under (serial, kind) when it completes.
    pub fn track(&self, cb: &CommandBuffer, serial: u64, kind: u8) {
        let times = Arc::clone(&self.times);
        let handler = RcBlock::new(move |cb: NonNull<ProtocolObject<dyn MTLCommandBuffer>>| {
            // SAFETY: Metal passes the live, completed command buffer.
            let cb = unsafe { cb.as_ref() };
            let t = cb.GPUEndTime() - cb.GPUStartTime();
            if let Ok(mut g) = times.lock() {
                g.done.push((serial, kind, t));
            }
        });
        // SAFETY: the block only touches a mutex-protected Vec.
        unsafe { cb.addCompletedHandler(RcBlock::as_ptr(&handler)) };
    }

    /// Encodes the DE tile jobs into `cb`; returns the per-job "samples left" buffer.
    pub fn encode_tiles(&self, cb: &CommandBuffer, jobs: &[TileJob], params: DeParams) -> Option<Buffer> {
        if jobs.is_empty() {
            return None;
        }
        let rb = self.ref_buf.as_ref()?;
        let left = new_buffer(&self.device, &vec![0u8; jobs.len() * 4]).expect("flags");
        let job_buf = new_buffer(&self.device, bytemuck::cast_slice(jobs)).expect("job buffer");
        let enc = cb.computeCommandEncoder().expect("compute encoder");
        enc.setComputePipelineState(&self.de_pipeline);
        // SAFETY: live resources; indices match the kernel signature; jobs bounded by the dispatch.
        unsafe {
            enc.setTexture_atIndex(Some(&self.data_tex), 0);
            enc.setTexture_atIndex(Some(&self.meta_tex), 1);
            let p = bytemuck::bytes_of(&params);
            enc.setBytes_length_atIndex(NonNull::new(p.as_ptr() as *mut c_void).unwrap(), p.len(), 0);
            enc.setBuffer_offset_atIndex(Some(rb), 0, 1);
            enc.setBuffer_offset_atIndex(Some(&job_buf), 0, 2);
            enc.setBuffer_offset_atIndex(Some(&self.state_buf), 0, 3);
            enc.setBuffer_offset_atIndex(Some(&left), 0, 4);
        }
        enc.dispatchThreadgroups_threadsPerThreadgroup(size3(jobs.len(), 1, 1), size3(16, 16, 1));
        enc.endEncoding();
        Some(left)
    }

    /// Encodes the frame's render pass (sky, then the rings) resolving into `target`.
    pub fn encode_render(&mut self, cb: &CommandBuffer, target: &ProtocolObject<dyn MTLTexture>, globals: &Globals, rings: &[RingDraw], blocks: &[[u16; 2]]) {
        let (w, h) = (target.width(), target.height());
        self.ensure_targets(w, h).expect("render targets");
        let pass = MTLRenderPassDescriptor::new();
        // SAFETY: attachment 0 exists.
        let ca = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        ca.setTexture(self.msaa_color.as_deref());
        ca.setResolveTexture(Some(target));
        ca.setLoadAction(MTLLoadAction::DontCare);
        ca.setStoreAction(MTLStoreAction::MultisampleResolve);
        let da = pass.depthAttachment();
        da.setTexture(self.depth.as_deref());
        da.setClearDepth(0.0);
        da.setLoadAction(MTLLoadAction::Clear);
        da.setStoreAction(MTLStoreAction::DontCare);
        let enc = cb.renderCommandEncoderWithDescriptor(&pass).expect("render encoder");
        let gbytes = bytemuck::bytes_of(globals);
        let gptr = NonNull::new(gbytes.as_ptr() as *mut c_void).unwrap();
        // sky
        enc.setRenderPipelineState(&self.sky_pipeline);
        enc.setDepthStencilState(Some(&self.sky_depth_state));
        // SAFETY: bytes copied now; a fullscreen triangle.
        unsafe {
            enc.setFragmentBytes_length_atIndex(gptr, gbytes.len(), 0);
            enc.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
        }
        // rings
        if !blocks.is_empty() {
            let block_buf = new_buffer(&self.device, bytemuck::cast_slice(blocks)).expect("block buffer");
            enc.setRenderPipelineState(&self.terrain_pipeline);
            enc.setDepthStencilState(Some(&self.depth_state));
            enc.setCullMode(MTLCullMode::None);
            // SAFETY: live resources at the indices the shaders declare.
            unsafe {
                enc.setVertexBytes_length_atIndex(gptr, gbytes.len(), 0);
                enc.setFragmentBytes_length_atIndex(gptr, gbytes.len(), 0);
                enc.setVertexTexture_atIndex(Some(&self.data_tex), 0);
                enc.setVertexTexture_atIndex(Some(&self.meta_tex), 1);
                enc.setFragmentTexture_atIndex(Some(&self.data_tex), 0);
                enc.setFragmentTexture_atIndex(Some(&self.meta_tex), 1);
            }
            for r in rings {
                if r.blocks == 0 {
                    continue;
                }
                let rb = bytemuck::bytes_of(&r.params);
                let rptr = NonNull::new(rb.as_ptr() as *mut c_void).unwrap();
                // SAFETY: instance count bounded by the block list slice.
                unsafe {
                    enc.setVertexBytes_length_atIndex(rptr, rb.len(), 1);
                    enc.setFragmentBytes_length_atIndex(rptr, rb.len(), 1);
                    enc.setVertexBuffer_offset_atIndex(Some(&block_buf), r.first_block * 4, 2);
                    enc.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                        MTLPrimitiveType::Triangle,
                        self.index_count,
                        MTLIndexType::UInt16,
                        &self.index_buf,
                        0,
                        r.blocks,
                    );
                }
            }
        }
        enc.endEncoding();
    }
}

pub fn new_buffer(device: &Device, bytes: &[u8]) -> Result<Buffer, String> {
    let len = bytes.len().max(4);
    let mut tmp;
    let src = if bytes.is_empty() {
        tmp = vec![0u8; 4];
        tmp.as_mut_slice() as &[u8]
    } else {
        bytes
    };
    // SAFETY: the pointer is valid for `len` bytes; Metal copies them.
    unsafe {
        device
            .newBufferWithBytes_length_options(NonNull::new(src.as_ptr() as *mut c_void).unwrap(), len, MTLResourceOptions::StorageModeShared)
            .ok_or_else(|| "buffer alloc".to_string())
    }
}

/// Reads a shared BGRA8 texture into `out` (row-major, 4 bytes per pixel).
pub fn read_texture(tex: &ProtocolObject<dyn MTLTexture>, out: &mut Vec<u8>) {
    let (w, h) = (tex.width(), tex.height());
    out.resize(w * h * 4, 0);
    // SAFETY: `out` holds w*h*4 bytes; the texture is shared and the GPU is done with it.
    unsafe {
        tex.getBytes_bytesPerRow_fromRegion_mipmapLevel(
            NonNull::new(out.as_mut_ptr() as *mut c_void).unwrap(),
            w * 4,
            MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: size3(w, h, 1) },
            0,
        );
    }
}

impl Gpu {
    /// Copies one slot of the data texture (R32Float) into host memory.
    pub fn read_data_slot(&self, slot: usize, n: usize) -> Vec<f32> {
        let buf = new_buffer(&self.device, &vec![0u8; n * n * 4]).expect("readback buffer");
        let cb = self.command_buffer();
        let blit = cb.blitCommandEncoder().expect("blit");
        // SAFETY: the buffer holds the whole slice.
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                &self.data_tex,
                slot,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
                size3(n, n, 1),
                &buf,
                0,
                n * 4,
                n * n * 4,
            );
        }
        blit.endEncoding();
        cb.commit();
        cb.waitUntilCompleted();
        let mut out = vec![0f32; n * n];
        // SAFETY: shared buffer of n*n*4 bytes, GPU done.
        unsafe { std::ptr::copy_nonoverlapping(buf.contents().as_ptr() as *const f32, out.as_mut_ptr(), n * n) };
        out
    }
}
