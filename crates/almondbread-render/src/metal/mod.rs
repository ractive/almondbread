//! Metal implementation of [`GpuBackend`] on objc2-metal (kb: platform/render-architecture,
//! platform/metal-compute). Rust owns the device, the queue, the compiled library, every
//! buffer, texture and pipeline. The kernels are the crate's MSL source, compiled at runtime
//! with the Swift renderer's options (MSL 3.0, fast math).

mod surface;

pub use surface::{MetalLayerSurface, PresentationLog};

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder, MTLCommandQueue,
    MTLCompileOptions, MTLComputeCommandEncoder, MTLComputePipelineState, MTLCreateSystemDefaultDevice, MTLDataType,
    MTLDevice, MTLDrawable, MTLFunctionConstantValues, MTLLanguageVersion, MTLLibrary, MTLMathMode, MTLOrigin,
    MTLPixelFormat, MTLRegion, MTLResource, MTLResourceOptions, MTLSize, MTLStorageMode, MTLTexture,
    MTLTextureDescriptor, MTLTextureUsage,
};

use crate::backend::{
    ComputeEncoder, GpuBackend, GpuError, MathMode, Storage, Submission, SubmissionStatus, TextureFormat,
};

// MTLCreateSystemDefaultDevice needs CoreGraphics linked.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {}

/// The kernel source (moved here from the Swift package; kb: platform/render-architecture).
/// The motion sample cache's kernels follow (kb: techniques/sample-reuse).
pub const KERNEL_SOURCE: &str = concat!(
    include_str!("../shaders/mandelbrot.metal"),
    include_str!("../shaders/sample_cache.metal")
);

pub type MetalBuffer = Retained<ProtocolObject<dyn MTLBuffer>>;
pub type MetalTexture = Retained<ProtocolObject<dyn MTLTexture>>;
pub type MetalPipeline = Retained<ProtocolObject<dyn MTLComputePipelineState>>;
pub type MetalDrawable = Retained<ProtocolObject<dyn MTLDrawable>>;

/// Device, serial queue and kernel library. Cloning shares them (a cloned backend gives a
/// second renderer on the same queue, e.g. the fresh reference render of `verify --history`).
#[derive(Clone)]
pub struct MetalBackend {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    library: Retained<ProtocolObject<dyn MTLLibrary>>,
}

impl MetalBackend {
    /// The system default GPU with the kernels compiled from [`KERNEL_SOURCE`].
    pub fn new() -> Result<Self, GpuError> {
        Self::with_source(KERNEL_SOURCE)
    }

    pub fn with_source(source: &str) -> Result<Self, GpuError> {
        autoreleasepool(|_| {
            let device = MTLCreateSystemDefaultDevice().ok_or_else(|| GpuError("no Metal device".into()))?;
            let queue = device
                .newCommandQueue()
                .ok_or_else(|| GpuError("no command queue".into()))?;
            let options = MTLCompileOptions::new();
            options.setLanguageVersion(MTLLanguageVersion::Version3_0);
            // Fast math (FMA contraction, approximate log/atan2 in the epilogue): the Swift
            // renderer's setting, measured ~10 % faster than .safe on M1 Pro with no accuracy
            // change against a Double reference (kb: platform/metal-compute). Explicit
            // because accuracy and bit-identity with the Swift renderer depend on it.
            #[allow(deprecated)]
            options.setFastMathEnabled(true);
            let library = device
                .newLibraryWithSource_options_error(&NSString::from_str(source), Some(&options))
                .map_err(|e| GpuError(format!("kernel compile failed: {}", e.localizedDescription())))?;
            Ok(Self { device, queue, library })
        })
    }

    pub fn device(&self) -> &ProtocolObject<dyn MTLDevice> {
        &self.device
    }

    /// Runs `f` inside an autorelease pool (long headless loops call this per render, so
    /// autoreleased Metal objects do not pile up).
    pub fn pool<R>(f: impl FnOnce() -> R) -> R {
        autoreleasepool(|_| f())
    }

    fn pixel_format(format: TextureFormat) -> MTLPixelFormat {
        match format {
            TextureFormat::Rg32Float => MTLPixelFormat::RG32Float,
            TextureFormat::Rg16Float => MTLPixelFormat::RG16Float,
            TextureFormat::Rgba16Float => MTLPixelFormat::RGBA16Float,
            TextureFormat::Bgra8Unorm => MTLPixelFormat::BGRA8Unorm,
        }
    }
}

/// A committed command buffer.
pub struct MetalSubmission(Retained<ProtocolObject<dyn MTLCommandBuffer>>);

impl Submission for MetalSubmission {
    fn status(&self) -> SubmissionStatus {
        let s = self.0.status();
        if s == MTLCommandBufferStatus::Completed {
            SubmissionStatus::Completed
        } else if s == MTLCommandBufferStatus::Error {
            let msg = self
                .0
                .error()
                .map(|e| e.localizedDescription().to_string())
                .unwrap_or_else(|| "command buffer error".into());
            SubmissionStatus::Failed(msg)
        } else {
            SubmissionStatus::Pending
        }
    }

    fn gpu_time(&self) -> f64 {
        self.0.GPUEndTime() - self.0.GPUStartTime()
    }

    fn wait(&self) {
        self.0.waitUntilCompleted();
    }
}

/// One command buffer with one compute pass.
pub struct MetalEncoder {
    cb: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    enc: Retained<ProtocolObject<dyn MTLComputeCommandEncoder>>,
    present: Option<MetalDrawable>,
}

fn size3(v: [usize; 3]) -> MTLSize {
    MTLSize {
        width: v[0],
        height: v[1],
        depth: v[2],
    }
}

impl ComputeEncoder<MetalBackend> for MetalEncoder {
    fn set_pipeline(&mut self, pipeline: &MetalPipeline) {
        self.enc.setComputePipelineState(pipeline);
    }

    fn set_bytes(&mut self, index: usize, bytes: &[u8]) {
        assert!(!bytes.is_empty() && bytes.len() <= 4096, "setBytes takes 1..4096 bytes");
        // SAFETY: the pointer is valid for `bytes.len()` bytes; Metal copies them now.
        unsafe {
            self.enc.setBytes_length_atIndex(
                NonNull::new(bytes.as_ptr() as *mut c_void).expect("non-null"),
                bytes.len(),
                index,
            );
        }
    }

    fn set_buffer(&mut self, index: usize, buffer: &MetalBuffer, offset: usize) {
        assert!(offset < buffer.length().max(1), "buffer offset out of range");
        // SAFETY: a live buffer; the kernels' accesses are bounded by the scheduler's sizes.
        unsafe { self.enc.setBuffer_offset_atIndex(Some(buffer), offset, index) }
    }

    fn set_texture(&mut self, index: usize, texture: &MetalTexture) {
        // SAFETY: a live texture; the command buffer retains it until completion.
        unsafe { self.enc.setTexture_atIndex(Some(texture), index) }
    }

    fn dispatch_threads(&mut self, threads: [usize; 3], per_group: [usize; 3]) {
        self.enc
            .dispatchThreads_threadsPerThreadgroup(size3(threads), size3(per_group));
    }

    fn dispatch_indirect(&mut self, args: &MetalBuffer, offset: usize, per_group: [usize; 3]) {
        assert!(offset + 12 <= args.length(), "indirect arguments out of range");
        // SAFETY: the arguments are written by `prepareChunk` earlier in this command buffer.
        unsafe {
            self.enc
                .dispatchThreadgroupsWithIndirectBuffer_indirectBufferOffset_threadsPerThreadgroup(
                    args,
                    offset,
                    size3(per_group),
                )
        }
    }

    fn present(&mut self, drawable: &MetalDrawable) {
        self.present = Some(drawable.clone());
    }

    fn commit(self) -> MetalSubmission {
        self.enc.endEncoding();
        if let Some(d) = &self.present {
            self.cb.presentDrawable(d);
        }
        self.cb.commit();
        MetalSubmission(self.cb)
    }
}

impl GpuBackend for MetalBackend {
    type Buffer = MetalBuffer;
    type Texture = MetalTexture;
    type Pipeline = MetalPipeline;
    type Drawable = MetalDrawable;
    type Submission = MetalSubmission;
    type Encoder = MetalEncoder;

    fn device_name(&self) -> String {
        self.device.name().to_string()
    }

    fn create_buffer(&self, len: usize, storage: Storage) -> Result<MetalBuffer, GpuError> {
        let mode = match storage {
            Storage::Private => MTLResourceOptions::StorageModePrivate,
            Storage::Shared => MTLResourceOptions::StorageModeShared,
        };
        self.device
            .newBufferWithLength_options(len.max(4), mode)
            .ok_or_else(|| GpuError(format!("buffer of {len} bytes")))
    }

    fn create_buffer_with_data(&self, data: &[u8]) -> Result<MetalBuffer, GpuError> {
        if data.is_empty() {
            return self.create_buffer(4, Storage::Shared);
        }
        // SAFETY: `data` is valid for its length; Metal copies it.
        unsafe {
            self.device.newBufferWithBytes_length_options(
                NonNull::new(data.as_ptr() as *mut c_void).expect("non-null"),
                data.len(),
                MTLResourceOptions::StorageModeShared,
            )
        }
        .ok_or_else(|| GpuError(format!("buffer of {} bytes", data.len())))
    }

    fn read_buffer(&self, buffer: &MetalBuffer, offset: usize, out: &mut [u8]) {
        assert!(offset + out.len() <= buffer.length(), "read past the buffer");
        assert!(
            buffer.storageMode() == MTLStorageMode::Shared,
            "read_buffer needs a shared buffer"
        );
        // SAFETY: shared storage is CPU-visible, the range is in bounds, and the scheduler
        // only reads stats slots of completed submissions.
        unsafe {
            let src = (buffer.contents().as_ptr() as *const u8).add(offset);
            std::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), out.len());
        }
    }

    fn create_texture(
        &self,
        width: usize,
        height: usize,
        format: TextureFormat,
        storage: Storage,
    ) -> Result<MetalTexture, GpuError> {
        // SAFETY: plain descriptor construction with valid sizes.
        let d = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                Self::pixel_format(format),
                width.max(1),
                height.max(1),
                false,
            )
        };
        d.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
        d.setStorageMode(match storage {
            Storage::Private => MTLStorageMode::Private,
            Storage::Shared => MTLStorageMode::Shared,
        });
        self.device
            .newTextureWithDescriptor(&d)
            .ok_or_else(|| GpuError(format!("texture {width}x{height} {format:?}")))
    }

    fn texture_size(&self, texture: &MetalTexture) -> (usize, usize) {
        (texture.width(), texture.height())
    }

    fn read_texture(
        &self,
        texture: &MetalTexture,
        width: usize,
        height: usize,
        format: TextureFormat,
        out: &mut [u8],
    ) -> Result<(), GpuError> {
        let row = width * format.bytes_per_pixel();
        assert_eq!(out.len(), row * height, "readback size");
        assert!(
            width <= texture.width() && height <= texture.height(),
            "region outside the texture"
        );
        let origin = MTLOrigin { x: 0, y: 0, z: 0 };
        let size = MTLSize {
            width,
            height,
            depth: 1,
        };
        autoreleasepool(|_| {
            if texture.storageMode() == MTLStorageMode::Shared {
                // Wait for any queued writer.
                let cb = self
                    .queue
                    .commandBuffer()
                    .ok_or_else(|| GpuError("command buffer".into()))?;
                cb.commit();
                cb.waitUntilCompleted();
                // SAFETY: `out` holds exactly `row * height` bytes; the region is in bounds.
                unsafe {
                    texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                        NonNull::new(out.as_mut_ptr() as *mut c_void).expect("non-null"),
                        row,
                        MTLRegion { origin, size },
                        0,
                    );
                }
                return Ok(());
            }
            let staging = self.create_buffer(row * height, Storage::Shared)?;
            let cb = self
                .queue
                .commandBuffer()
                .ok_or_else(|| GpuError("command buffer".into()))?;
            let blit = cb.blitCommandEncoder().ok_or_else(|| GpuError("blit encoder".into()))?;
            // SAFETY: source region and destination range are in bounds.
            unsafe {
                blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                    texture, 0, 0, origin, size, &staging, 0, row, row * height,
                );
            }
            blit.endEncoding();
            cb.commit();
            cb.waitUntilCompleted();
            self.read_buffer(&staging, 0, out);
            Ok(())
        })
    }

    fn read_shared_texture(
        &self,
        texture: &MetalTexture,
        width: usize,
        height: usize,
        format: TextureFormat,
        out: &mut [u8],
    ) {
        let row = width * format.bytes_per_pixel();
        assert_eq!(out.len(), row * height, "readback size");
        assert!(
            width <= texture.width() && height <= texture.height() && texture.storageMode() == MTLStorageMode::Shared,
            "region outside the texture or not shared"
        );
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width,
                height,
                depth: 1,
            },
        };
        // SAFETY: `out` holds exactly `row * height` bytes; the region is in bounds; the
        // caller guarantees no pending writer.
        unsafe {
            texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                NonNull::new(out.as_mut_ptr() as *mut c_void).expect("non-null"),
                row,
                region,
                0,
            );
        }
    }

    fn pipeline(&self, function: &str, constants: &[bool]) -> Result<MetalPipeline, GpuError> {
        autoreleasepool(|_| {
            let name = NSString::from_str(function);
            let f = if constants.is_empty() {
                self.library
                    .newFunctionWithName(&name)
                    .ok_or_else(|| GpuError(format!("kernel {function} missing")))?
            } else {
                let values = MTLFunctionConstantValues::new();
                for (i, &v) in constants.iter().enumerate() {
                    let mut b = v;
                    // SAFETY: a valid bool for MTLDataType::Bool; Metal copies the value.
                    unsafe {
                        values.setConstantValue_type_atIndex(NonNull::from(&mut b).cast(), MTLDataType::Bool, i);
                    }
                }
                self.library
                    .newFunctionWithName_constantValues_error(&name, &values)
                    .map_err(|e| GpuError(format!("kernel {function}: {}", e.localizedDescription())))?
            };
            self.device
                .newComputePipelineStateWithFunction_error(&f)
                .map_err(|e| GpuError(format!("pipeline {function}: {}", e.localizedDescription())))
        })
    }

    fn pipeline_limits(&self, pipeline: &MetalPipeline) -> (usize, usize) {
        (
            pipeline.threadExecutionWidth(),
            pipeline.maxTotalThreadsPerThreadgroup(),
        )
    }

    fn library_pipelines(
        &self,
        source: &str,
        functions: &[&str],
        math: MathMode,
    ) -> Result<Vec<MetalPipeline>, GpuError> {
        use objc2::runtime::NSObjectProtocol;
        autoreleasepool(|_| {
            let options = MTLCompileOptions::new();
            options.setLanguageVersion(MTLLanguageVersion::Version3_0);
            // `mathMode` (macOS 15 / iOS 18) where available, else the deprecated switch.
            if options.respondsToSelector(objc2::sel!(setMathMode:)) {
                options.setMathMode(match math {
                    MathMode::Safe => MTLMathMode::Safe,
                    MathMode::Fast => MTLMathMode::Fast,
                });
            } else {
                #[allow(deprecated)]
                options.setFastMathEnabled(math == MathMode::Fast);
            }
            let library = self
                .device
                .newLibraryWithSource_options_error(&NSString::from_str(source), Some(&options))
                .map_err(|e| GpuError(format!("library compile failed: {}", e.localizedDescription())))?;
            functions
                .iter()
                .map(|name| {
                    let f = library
                        .newFunctionWithName(&NSString::from_str(name))
                        .ok_or_else(|| GpuError(format!("kernel {name} missing")))?;
                    self.device
                        .newComputePipelineStateWithFunction_error(&f)
                        .map_err(|e| GpuError(format!("pipeline {name}: {}", e.localizedDescription())))
                })
                .collect()
        })
    }

    fn begin(&self, label: &str) -> Result<MetalEncoder, GpuError> {
        let cb = self
            .queue
            .commandBuffer()
            .ok_or_else(|| GpuError("command buffer".into()))?;
        cb.setLabel(Some(&NSString::from_str(label)));
        let enc = cb
            .computeCommandEncoder()
            .ok_or_else(|| GpuError("compute encoder".into()))?;
        Ok(MetalEncoder { cb, enc, present: None })
    }
}

/// Seconds on the `CACurrentMediaTime` clock (mach absolute time): the clock of display-link
/// timestamps and `presentedTime`, so hosts and the renderer agree on "now".
pub fn media_time() -> f64 {
    objc2_quartz_core::CACurrentMediaTime()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_types::{
        BlaParams, ChunkStats, ColorParams, ComposeParams, IterateParams, LEVEL_COUNTERS_SIZE, PixelState, SeedNode,
        SeedParams,
    };

    /// The MSL struct sizes as the Metal compiler lays them out equal the Rust ones.
    #[test]
    fn msl_layouts_match() {
        let probe = format!(
            "{KERNEL_SOURCE}\nkernel void layoutProbe(device uint* out [[buffer(0)]]) {{\n\
             out[0] = sizeof(IterateParams); out[1] = sizeof(PixelState); out[2] = sizeof(ChunkStats);\n\
             out[3] = sizeof(ColorParams); out[4] = sizeof(LevelCounters); out[5] = alignof(ColorParams);\n\
             out[6] = alignof(IterateParams); out[7] = sizeof(ComposeParams); out[8] = sizeof(BlaParams);\n\
             out[9] = sizeof(BlaEntry); out[10] = sizeof(BlaEntryX); out[11] = sizeof(RefX);\n\
             out[12] = sizeof(PixelStateIC); out[13] = sizeof(SeedNode); out[14] = sizeof(SeedParams);\n\
             out[15] = sizeof(CacheParams); out[16] = sizeof(SurveyParams); }}\n"
        );
        let b = MetalBackend::with_source(&probe).expect("metal");
        let p = b.pipeline("layoutProbe", &[]).unwrap();
        let out = b.create_buffer(128, Storage::Shared).unwrap();
        let mut enc = b.begin("probe").unwrap();
        enc.set_pipeline(&p);
        enc.set_buffer(0, &out, 0);
        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
        enc.commit().wait();
        let mut bytes = [0u8; 68];
        b.read_buffer(&out, 0, &mut bytes);
        let v: Vec<u32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        assert_eq!(
            v,
            vec![
                size_of::<IterateParams>() as u32,
                size_of::<PixelState>() as u32,
                size_of::<ChunkStats>() as u32,
                size_of::<ColorParams>() as u32,
                LEVEL_COUNTERS_SIZE as u32,
                align_of::<ColorParams>() as u32,
                align_of::<IterateParams>() as u32,
                size_of::<ComposeParams>() as u32,
                size_of::<BlaParams>() as u32,
                size_of::<almondbread_core::bla::BlaEntry>() as u32,
                size_of::<almondbread_core::bla::BlaEntryX>() as u32,
                size_of::<almondbread_core::math::FloatExpComplex>() as u32,
                size_of::<crate::gpu_types::PixelStateIC>() as u32,
                size_of::<SeedNode>() as u32,
                size_of::<SeedParams>() as u32,
                size_of::<crate::gpu_types::CacheParams>() as u32,
                size_of::<crate::gpu_types::SurveyParams>() as u32,
            ]
        );
    }

    /// An offscreen render of a benchmark location with the given local SA settings: (field,
    /// loop steps incl. seeds, the seed report).
    fn sa_render(
        sa: crate::scheduler::LocalSa,
        name: &str,
        w: usize,
        h: usize,
        interior: bool,
    ) -> (
        almondbread_core::IterationField,
        u64,
        Option<crate::scheduler::SeedReport>,
    ) {
        use crate::scheduler::{Renderer, SchedulerConfig};
        use almondbread_core::locations::location_named;
        let config = SchedulerConfig {
            local_sa: sa,
            ..SchedulerConfig::default()
        };
        let mut r = Renderer::with_config(MetalBackend::new().expect("metal"), config).expect("renderer");
        let loc = location_named(name).expect("location");
        r.settings = almondbread_core::RenderSettings::plain();
        r.settings.auto_iterations = false;
        r.settings.manual_iterations = loc.max_iterations();
        r.settings.supersampling = almondbread_core::Supersampling::Off;
        r.settings.interior_detection = interior;
        r.viewport = loc.viewport_for_width(w);
        let out = r.render_offscreen(w, h, true).expect("render");
        let field = almondbread_core::IterationField::from_texels(w, h, out.iterations.as_deref().unwrap());
        let steps = out.steps_done + out.seeds.as_ref().map_or(0, |s| s.steps());
        (field, steps, out.seeds)
    }

    fn sa(mode: crate::scheduler::SaMode) -> crate::scheduler::LocalSa {
        crate::scheduler::LocalSa {
            mode,
            ..crate::scheduler::LocalSa::default()
        }
    }

    /// M5 (kb: techniques/local-series-approximation): at the dense view local SA takes
    /// several times fewer steps with the plain kernel's class statistics; at the deep needle
    /// minibrot (interior test on, period-255 nucleus reference) it paints no exterior pixel
    /// interior (restarting the interior derivative at the branch painted 96 %, R5).
    #[test]
    fn local_sa_dense_and_needle() {
        use crate::scheduler::SaMode;
        use almondbread_core::oracle::compare;
        let (plain, plain_steps, none) = sa_render(sa(SaMode::Off), "dense-5e-14", 400, 250, true);
        assert!(none.is_none());
        let (on, on_steps, report) = sa_render(sa(SaMode::On), "dense-5e-14", 400, 250, true);
        let report = report.expect("seeded");
        assert_eq!(report.tiles_seeded, report.tiles);
        assert!(on_steps * 3 < plain_steps, "{on_steps} vs {plain_steps}");
        let (rep, _) = compare(&on, &plain);
        assert!(rep.mismatch_pct() < 0.05, "{}", rep.summary());
        // Auto seeds every tile here (the leaves saved thousands of iterations).
        let (auto, _, report) = sa_render(sa(SaMode::Auto), "dense-5e-14", 400, 250, true);
        let report = report.expect("seeded");
        assert_eq!(report.tiles_seeded, report.tiles);
        assert_eq!(auto.bitwise_differences(&on), 0);
        let (plain, _, _) = sa_render(sa(SaMode::Off), "needle-1e305", 320, 200, true);
        let (on, _, _) = sa_render(sa(SaMode::On), "needle-1e305", 320, 200, true);
        let (rep, _) = compare(&on, &plain);
        assert!(rep.false_interior_pct() < 0.01, "{}", rep.summary());
    }

    /// M5 with view rotation (kb: decisions/controls "View rotation"): seeds turn their node
    /// offsets like the pixels, so a rotated dense view seeded matches the plain one (a wrong
    /// turn would put every pixel on another orbit), in the plain and the deep kernel.
    #[test]
    fn local_sa_rotated() {
        use crate::scheduler::{Renderer, SaMode, SchedulerConfig};
        use almondbread_core::locations::location_named;
        use almondbread_core::oracle::compare;
        for name in ["dense-5e-14", "sea-1e20"] {
            let render = |mode: SaMode| {
                let config = SchedulerConfig {
                    local_sa: sa(mode),
                    ..SchedulerConfig::default()
                };
                let mut r = Renderer::with_config(MetalBackend::new().expect("metal"), config).expect("renderer");
                let loc = location_named(name).expect("location");
                r.settings = almondbread_core::RenderSettings::plain();
                r.settings.auto_iterations = false;
                r.settings.manual_iterations = loc.max_iterations();
                r.settings.supersampling = almondbread_core::Supersampling::Off;
                r.viewport = loc.viewport_for_width(400).with_angle(0.7);
                let out = r.render_offscreen(400, 250, true).expect("render");
                let steps = out.steps_done + out.seeds.as_ref().map_or(0, |s| s.steps());
                (
                    almondbread_core::IterationField::from_texels(400, 250, out.iterations.as_deref().unwrap()),
                    steps,
                )
            };
            let (off, off_steps) = render(SaMode::Off);
            let (on, on_steps) = render(SaMode::On);
            // (Unrotated at this size: dense 7x, sea-1e20 1.45x fewer steps.)
            assert!(on_steps * 4 < off_steps * 3, "{name}: {on_steps} vs {off_steps}");
            let (rep, _) = compare(&on, &off);
            assert!(rep.mismatch_pct() < 0.05, "{name}: {}", rep.summary());
            assert!(rep.escape_differs_pct() < 25.0, "{name}: {}", rep.summary());
        }
    }

    /// M5: where the seeds branch early (interior-heavy home view at 100k iterations) the
    /// auto gate seeds no tile, and the field is the plain one bit for bit.
    #[test]
    fn local_sa_auto_gate_off() {
        use crate::scheduler::SaMode;
        let (plain, _, _) = sa_render(sa(SaMode::Off), "home-100k", 320, 200, true);
        let (auto, _, report) = sa_render(sa(SaMode::Auto), "home-100k", 320, 200, true);
        assert_eq!(report.expect("seeded").tiles_seeded, 0);
        assert_eq!(auto.bitwise_differences(&plain), 0);
    }

    /// M5: seed rounds are resumable: a tiny step budget (many rounds) gives the same field
    /// bit for bit as one unbounded round, in the plain and in the deep kernel (shown frame =
    /// fresh render whatever the rounds' timing).
    #[test]
    fn local_sa_rounds_resumable() {
        use crate::scheduler::{LocalSa, SaMode};
        for (name, w, h) in [
            ("dense-5e-14", 320, 200),
            ("sea-1e20", 320, 200),
            ("dendrite-1e300", 320, 200),
        ] {
            let small = LocalSa {
                budget: 5,
                ..sa(SaMode::On)
            };
            let big = LocalSa {
                budget: u32::MAX,
                ..sa(SaMode::On)
            };
            let (a, _, ra) = sa_render(small, name, w, h, true);
            let (b, _, rb) = sa_render(big, name, w, h, true);
            assert!(ra.expect("seeded").rounds > 3, "{name}");
            assert_eq!(rb.expect("seeded").rounds, 1, "{name}");
            assert_eq!(a.bitwise_differences(&b), 0, "{name}");
        }
    }

    /// M5 interior-only tile fill (kb: techniques/guessing-and-subdivision): at the elephant
    /// valley (28 % interior, slow to prove near the cusp) filling tiles whose border is all
    /// proven interior skips a third of the steps and gives the unfilled field bit for bit;
    /// with an analysed interior colouring nothing is filled.
    #[test]
    fn interior_fill_exact() {
        use crate::scheduler::{Renderer, SaMode, SchedulerConfig};
        use almondbread_core::locations::location_named;
        let render = |fill: SaMode, inside: almondbread_core::InteriorMode| {
            let config = SchedulerConfig {
                local_sa: sa(SaMode::Off),
                interior_fill: fill,
                ..SchedulerConfig::default()
            };
            let mut r = Renderer::with_config(MetalBackend::new().expect("metal"), config).expect("renderer");
            let loc = location_named("elephant").expect("location");
            r.settings = almondbread_core::RenderSettings::plain();
            r.settings.auto_iterations = false;
            r.settings.manual_iterations = loc.max_iterations();
            r.settings.supersampling = almondbread_core::Supersampling::Off;
            r.settings.interior_mode = inside;
            r.viewport = loc.viewport_for_width(480);
            let out = r.render_offscreen(480, 300, true).expect("render");
            let field = almondbread_core::IterationField::from_texels(480, 300, out.iterations.as_deref().unwrap());
            (field, out.steps_done)
        };
        use almondbread_core::InteriorMode;
        let (off, off_steps) = render(SaMode::Off, InteriorMode::Flat);
        let (on, on_steps) = render(SaMode::On, InteriorMode::Flat);
        assert!(on_steps * 10 < off_steps * 8, "{on_steps} vs {off_steps}");
        assert_eq!(on.bitwise_differences(&off), 0);
        let (_, a) = render(SaMode::Off, InteriorMode::Period);
        let (_, b) = render(SaMode::On, InteriorMode::Period);
        assert_eq!(a, b);
    }

    /// Offscreen fields at `scale` around the M4 slow view with the given BLA mode:
    /// (field, used, steps, iterations).
    fn bla_render(
        scale: f64,
        iterations: usize,
        mode: crate::scheduler::BlaMode,
        async_from: usize,
    ) -> (almondbread_core::IterationField, bool, u64, u64) {
        use crate::scheduler::{Renderer, SchedulerConfig};
        // (Local SA off: the seeds would take the blocks the pixels are measured on.)
        let config = SchedulerConfig {
            bla: mode,
            bla_async_from: async_from,
            local_sa: sa(crate::scheduler::SaMode::Off),
            ..SchedulerConfig::default()
        };
        let mut r = Renderer::with_config(MetalBackend::new().expect("metal"), config).expect("renderer");
        r.settings = almondbread_core::RenderSettings::plain();
        r.settings.auto_iterations = false;
        r.settings.manual_iterations = iterations;
        r.settings.supersampling = almondbread_core::Supersampling::Off;
        r.settings.shading = 0.5; // distance estimate through BLA blocks
        r.viewport = almondbread_core::Viewport::new(-1.4080321340304542, 0.13680236432900417, scale);
        let out = r.render_offscreen(320, 200, true).expect("render");
        let field = almondbread_core::IterationField::from_texels(320, 200, out.iterations.as_deref().unwrap());
        (field, out.bla_used, out.steps_done, out.iterations_done)
    }

    /// The motion sample cache (kb: techniques/sample-reuse) holds exact samples: after a
    /// settled view (rest seed), a zoom in (finer level, resampled), a zoom out (coarser) and a
    /// pan, every cached sample equals a fresh render of its lattice point with the same
    /// reference (class always; escape count up to float32 rounding of the point), and the
    /// shown refined frame equals a fresh render.
    #[test]
    fn sample_cache_exact() {
        use crate::scheduler::Renderer;
        let mut r = Renderer::new(MetalBackend::new().expect("metal")).expect("renderer");
        r.viewport = almondbread_core::Viewport::new(-0.743643887037151, 0.13182590420533, 3.75e-6 * 4.0);
        // (One reference throughout: a fixed limit, moves within its reach. Samples of other
        // references differ from a fresh render as the refined frames do: float32.)
        r.settings.auto_iterations = false;
        r.settings.manual_iterations = 2000;
        // The cache is opt-in (`ALMONDBREAD_SAMPLE_CACHE=1`).
        r.sample_cache_config().enabled = true;
        let (w, h) = (320, 200);
        r.simulate(w, h, 1.0, 120.0, 0, 30.0, &mut |_, _, _| {})
            .expect("settle");
        type Step = Box<dyn Fn(&mut almondbread_core::Viewport, f64)>;
        let steps: [(&str, Step); 3] = [
            ("zoom in", Box::new(|v, dt| v.zoom_by((1.4 * dt).exp(), 30.0, 10.0))),
            ("zoom out", Box::new(|v, dt| v.zoom_by((-1.4 * dt).exp(), 0.0, 0.0))),
            ("pan", Box::new(|v, dt| v.pan_by_points(-150.0 * dt, 60.0 * dt))),
        ];
        for (name, step) in steps {
            r.reset_sample_cache_stats();
            r.cache_check_renderer = Some(Box::new(Renderer::new(MetalBackend::new().expect("metal")).expect("r")));
            r.cache_checks.clear();
            let serial = r.reference_serial();
            r.simulate(w, h, 1.0, 120.0, 90, 30.0, &mut |_, dt, v| step(v, dt))
                .expect("gesture");
            let c = r.cache_checks[0].clone().expect("cache");
            let s = r.sample_cache_stats().clone();
            assert!(s.frames > 60 && s.fallback_frames == 0, "{name}: {s:?}");
            assert!(c.samples > 20_000, "{name}: {c:?}");
            assert_eq!(r.reference_serial(), serial, "{name}: the reference changed");
            // (Escape counts: the lattice point's dc = i h + (anchor - reference) rounds in float32
            // unlike the check render's (its own centre): ~1e-7 of |dc|, which flips a few
            // escape counts in chaotic places, as two fresh renders centred differently do.)
            assert!(c.class_differ == 0 && c.escape_pct() < 0.5, "{name}: {c:?}");
            let m = r.displayed_frame_mismatch().expect("refined");
            assert!(m.percent < 0.05, "{name}: {m:?}");
        }
    }

    /// M4 regression (kb: synthesis/benchmarks M4): at the boundary-dense 4.6e-14 view BLA
    /// would cost more than it skips, so `auto` leaves the kernel as it was (bit-identical to
    /// BLA off); forced on, it changes no pixel's class and the smooth count far less than
    /// float32 already does.
    #[test]
    fn bla_slow_view() {
        use crate::scheduler::BlaMode;
        use almondbread_core::oracle::compare;
        let (off, _, _, _) = bla_render(4.615e-14 * 5.0, 5791, BlaMode::Off, 50_000);
        let (auto, used, steps, iters) = bla_render(4.615e-14 * 5.0, 5791, BlaMode::Auto, 50_000);
        assert!(!used);
        assert_eq!(steps, iters);
        assert_eq!(auto.bitwise_differences(&off), 0);
        let (on, used, steps, iters) = bla_render(4.615e-14 * 5.0, 5791, BlaMode::On, 50_000);
        assert!(used && steps < iters);
        let (rep, _) = compare(&on, &off);
        assert!(rep.mismatch_pct() < 0.05, "{}", rep.summary());
        // (Float32 itself is off by a median 0.067 against the f64 oracle here, and 54 % of
        // pixels by more than 0.05: boundary pixels are chaotic, any rounding moves them.)
        assert!(rep.median_error < 0.01, "{}", rep.summary());
    }

    /// Deep (1e-20 per pixel around the same f64 reference; not the true set at that depth,
    /// but the same orbit for both runs): `auto` takes the table, skips most steps, and the
    /// field equals the skip-off one to float32 noise. The table builds on a thread here
    /// (`bla_async_from` 0) and the offscreen render waits for it.
    #[test]
    fn bla_deep_view() {
        use crate::scheduler::BlaMode;
        use almondbread_core::oracle::compare;
        let (off, used, _, _) = bla_render(1e-20, 12_000, BlaMode::Off, 0);
        assert!(!used);
        let (on, used, steps, iters) = bla_render(1e-20, 12_000, BlaMode::Auto, 0);
        assert!(used);
        assert!(steps * 4 < iters, "steps {steps} iterations {iters}");
        let (rep, _) = compare(&on, &off);
        assert_eq!(rep.false_interior + rep.false_exterior, 0, "{}", rep.summary());
        assert!(rep.max_error < 1e-4, "{}", rep.summary());
    }

    /// Default auto iterations at a named location, offscreen: (limit used, escaped fraction).
    fn auto_render(location: &str, width: usize, height: usize) -> (usize, f64) {
        use crate::scheduler::Renderer;
        let mut r = Renderer::new(MetalBackend::new().expect("metal")).expect("renderer");
        r.settings = almondbread_core::RenderSettings::plain();
        r.settings.supersampling = almondbread_core::Supersampling::Off;
        r.viewport = almondbread_core::locations::location_named(location)
            .expect("location")
            .viewport_for_width(width);
        let out = r.render_offscreen(width, height, true).expect("render");
        let texels = out.iterations.expect("iterations");
        let escaped = texels.iter().filter(|t| t[0] >= 0.0).count();
        (r.stats.max_iterations, escaped as f64 / texels.len() as f64)
    }

    /// sea-8007 at default settings (kb: techniques/auto-iterations, "Nothing escaped"): the
    /// depth limit (~29 600) is below every escape, so the first pass leaves the whole view
    /// unresolved (it rendered as uniform grey). The learner raises the limit 4x and renders
    /// the view again; the rings of seahorses around the minibrot escape.
    #[test]
    fn auto_iterations_nothing_escaped() {
        let (limit, escaped) = auto_render("sea-8007", 320, 200);
        assert!(limit > 100_000, "limit {limit}");
        assert!(escaped > 0.95, "escaped {escaped}");
    }

    /// A view entirely inside the main cardioid settles at the depth limit: the derivative test
    /// proves every pixel interior, nothing is unresolved, no re-render.
    #[test]
    fn auto_iterations_interior_view() {
        use crate::scheduler::Renderer;
        let mut r = Renderer::new(MetalBackend::new().expect("metal")).expect("renderer");
        r.settings = almondbread_core::RenderSettings::plain();
        r.settings.supersampling = almondbread_core::Supersampling::Off;
        r.viewport = almondbread_core::Viewport::new(-0.2, 0.0, 1e-6);
        let depth = almondbread_core::settings::depth_iterations(&r.viewport);
        let out = r.render_offscreen(320, 200, true).expect("render");
        let texels = out.iterations.expect("iterations");
        assert!(texels.iter().all(|t| t[0] == almondbread_core::oracle::ITER_INTERIOR));
        assert_eq!(r.stats.max_iterations, depth);
        assert_eq!(r.settings.learned_iterations, 0);
    }

    /// Every look and every interior mode switched at runtime on one interactive renderer
    /// (the user saw the app die selecting a look): frames keep presenting, none fails, and
    /// switching an overlay on and off in between changes nothing about that.
    #[test]
    fn switch_looks_and_interior_modes() {
        use crate::host::Host;
        use crate::scheduler::Renderer;
        use almondbread_core::InteriorMode;
        use almondbread_core::palette::Look;
        use objc2_quartz_core::CAMetalLayer;

        let backend = MetalBackend::new().expect("metal");
        let mut surface = MetalLayerSurface::new(CAMetalLayer::new(), &backend);
        surface.set_drawable_size(320, 200);
        let mut host = Host::new(Renderer::new(backend).expect("renderer"));
        host.resize(160.0, 100.0, 2.0, 0.0);
        let start = std::time::Instant::now();
        let now = || start.elapsed().as_secs_f64();
        let mut frames = |host: &mut Host<MetalBackend>, n: usize| {
            for _ in 0..n {
                let t = now();
                let ready = surface.drawable_ready(t);
                host.frame(t, &now, ready, &mut |f| surface.next_drawable(t, f).map(|d| (d, 0)));
                std::thread::sleep(std::time::Duration::from_millis(4));
            }
        };
        frames(&mut host, 20);
        for (i, look) in Look::all().iter().enumerate() {
            look.apply(&mut host.renderer.settings);
            host.renderer.settings.interior_mode = InteriorMode::ALL[i % InteriorMode::ALL.len()];
            host.overlay.settings.orbit = i % 2 == 0;
            host.overlay.settings.map = i % 3 == 0;
            frames(&mut host, 15);
        }
        for m in InteriorMode::ALL {
            host.renderer.settings.interior_mode = m;
            frames(&mut host, 10);
        }
        host.overlay.settings.orbit = false;
        host.overlay.settings.map = false;
        frames(&mut host, 30);
        assert_eq!(host.renderer.finish_frames(), 0, "frames failed on the GPU");
    }

    /// Interior cycle analysis (interior colourings): the period every interior pixel reports
    /// at the centres of known components, shallow (plain kernel) and deep (needle-1e305,
    /// period-255 minibrot, deep kernel). Flat colouring keeps the legacy `.y` flag.
    #[test]
    fn interior_analysis_periods() {
        use crate::scheduler::Renderer;
        use almondbread_core::InteriorMode;
        let periods = |viewport: almondbread_core::Viewport, mode: InteriorMode| -> Vec<u32> {
            let mut r = Renderer::new(MetalBackend::new().expect("metal")).expect("renderer");
            r.settings = almondbread_core::RenderSettings::plain();
            r.settings.supersampling = almondbread_core::Supersampling::Off;
            r.settings.interior_mode = mode;
            r.viewport = viewport;
            let out = r.render_offscreen(64, 40, true).expect("render");
            let texels = out.iterations.expect("iterations");
            texels
                .iter()
                .filter(|t| t[0] == almondbread_core::oracle::ITER_INTERIOR)
                .map(|t| t[1] as u32)
                .collect()
        };
        let all = |v: &[u32], p: u32| !v.is_empty() && v.iter().all(|&x| x == p);
        let cardioid = almondbread_core::Viewport::new(-0.1, 0.05, 1e-3);
        assert!(all(&periods(cardioid.clone(), InteriorMode::Period), 1));
        assert!(all(
            &periods(almondbread_core::Viewport::new(-1.0, 0.0, 1e-3), InteriorMode::Angle),
            2
        ));
        assert!(all(
            &periods(
                almondbread_core::Viewport::new(-0.1226, 0.7449, 2e-4),
                InteriorMode::Bubbles
            ),
            3
        ));
        // Flat: no analysis (the derivative test's flag).
        assert!(all(&periods(cardioid, InteriorMode::Flat), 1));
        let deep = almondbread_core::locations::location_named("needle-1e305")
            .expect("location")
            .viewport_for_width(64);
        let mut zoomed = deep.clone();
        zoomed.scale = zoomed.scale * 0.02;
        let v = periods(zoomed, InteriorMode::Period);
        assert!(all(&v, 255), "{:?}", &v[..v.len().min(20)]);
    }

    /// "Selected Lagoon and almondbread died" (2026-10-14): a colour change while progressive
    /// AA samples were on the GPU dropped their levels; when the stale chunks were harvested
    /// they indexed a level that was gone (index out of bounds, a Swift trap through the FFI).
    /// Every palette and look is applied while AA chunks are in flight; nothing may panic and
    /// the view must settle.
    #[test]
    fn colour_change_during_progressive_aa() {
        use crate::backend::{GpuBackend, Storage, TextureFormat};
        use crate::scheduler::Renderer;
        use almondbread_core::palette::{Look, Palette};

        let backend = MetalBackend::new().expect("metal");
        let target = backend
            .create_texture(640, 400, TextureFormat::Bgra8Unorm, Storage::Shared)
            .expect("target");
        let mut r = Renderer::new(backend).expect("renderer");
        r.settings.supersampling = almondbread_core::Supersampling::from_factor(3).expect("x9");
        r.viewport = almondbread_core::Viewport::new(-0.743643887037151, 0.13182590420533, 3e-6);
        let start = std::time::Instant::now();
        let now = || start.elapsed().as_secs_f64();
        type Change = Box<dyn Fn(&mut almondbread_core::RenderSettings)>;
        let mut changes: Vec<Change> = Vec::new();
        for p in Palette::all() {
            changes.push(Box::new(move |s| s.palette_id = p.id.to_string()));
        }
        for l in Look::all() {
            changes.push(Box::new(move |s| l.apply(s)));
        }
        let mut next = 0;
        let mut during_aa = 0;
        for _ in 0..20_000 {
            let t = now();
            r.render_frame(t, 640, 400, 1.0, &mut |_| Some((target.clone(), None)));
            if next < changes.len() && r.aa_chunks_in_flight() {
                changes[next](&mut r.settings);
                next += 1;
                during_aa += 1;
            }
            if next == changes.len() && r.is_settled() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(
            during_aa >= changes.len() / 2,
            "only {during_aa} changes hit AA in flight"
        );
        assert!(r.is_settled(), "did not settle after {next} colour changes");
        assert_eq!(r.finish_frames(), 0, "failed frames");
    }

    /// The hosting path of R2b, headless: a `CAMetalLayer` (no window) as the surface, the
    /// per-refresh `render_frame` presenting drawables, a gesture then idle refinement.
    #[test]
    fn layer_surface_frames() {
        use crate::scheduler::Renderer;
        use almondbread_core::input::InputEvent;
        use objc2_quartz_core::CAMetalLayer;

        let backend = MetalBackend::new().expect("metal");
        let mut surface = MetalLayerSurface::new(CAMetalLayer::new(), &backend);
        surface.set_drawable_size(320, 200);
        let mut r = Renderer::new(backend).expect("renderer");
        r.viewport = almondbread_core::Viewport::new(-0.75, 0.0, 3.0 / 160.0);
        let start = std::time::Instant::now();
        let now = || start.elapsed().as_secs_f64();
        let mut presented = 0;
        let mut finished = false;
        for frame in 0..600 {
            let t = now();
            if frame < 20 {
                r.handle_input(
                    InputEvent::WheelZoom {
                        delta: 0.2,
                        x: 10.0,
                        y: 5.0,
                    },
                    t,
                );
            }
            let out = r.render_frame(t, 320, 200, 0.5, &mut |_| surface.next_drawable(t, false));
            presented += usize::from(out.presented);
            if frame > 20 && !out.animating {
                finished = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        assert!(presented > 10, "presented {presented}");
        assert!(finished, "renderer kept animating");
        assert!(!r.stats.is_preview && !r.stats.is_rendering);
        assert_eq!(r.stats.render_size, (320, 200));
    }

    /// IEEE half float bits to f32 (DE texels are Rg16Float).
    fn half(h: u16) -> f32 {
        let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
        let e = i32::from((h >> 10) & 0x1f);
        let m = f32::from(h & 0x3ff);
        s * match e {
            0 => m * 2f32.powi(-24),
            31 => f32::INFINITY,
            _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
        }
    }

    /// Deep-kernel distance estimate regressions (found in R5, kb: synthesis/benchmarks R5):
    /// the DE was clamped to 65000 before scaling by its exponent (thick outline bands in
    /// smooth exterior), and the DE mantissa overflowed in the steps before escape (DE 0,
    /// dotted outline arcs). In sea-1e20's smooth exterior neither may happen: no escaped
    /// pixel has DE 0, and DE agrees between neighbours away from the boundary.
    #[test]
    fn deep_distance_estimate() {
        use crate::scheduler::Renderer;
        use almondbread_core::locations::location_named;
        let mut r = Renderer::new(MetalBackend::new().expect("metal")).expect("renderer");
        let loc = location_named("sea-1e20").expect("location");
        r.settings = almondbread_core::RenderSettings::plain();
        r.settings.auto_iterations = false;
        r.settings.manual_iterations = loc.max_iterations();
        r.settings.supersampling = almondbread_core::Supersampling::Off;
        r.settings.shading = 0.5; // distance estimate on
        let (w, h) = (400, 250);
        r.viewport = loc.viewport_for_width(w);
        let out = r.render_offscreen(w, h, true).expect("render");
        let iters = out.iterations.expect("iterations");
        let de: Vec<f32> = r
            .read_distance_estimates()
            .expect("de")
            .iter()
            .map(|t| half(t[0]))
            .collect();
        let escaped = |k: usize| iters[k][0] >= 0.0;
        // DE 0 itself is legitimate on the boundary (below half-float range); between two
        // neighbours more than 2 texels from the boundary it is the overflow.
        let zero = (1..w * h - 1)
            .filter(|&k| escaped(k) && de[k] <= 0.0 && k % w > 0 && k % w < w - 1)
            .filter(|&k| de[k - 1] > 2.0 && de[k + 1] > 2.0)
            .count();
        // Smooth exterior (DE > 8 texels): a neighbour's DE within a factor 2.
        let (mut smooth, mut jumps) = (0, 0);
        for y in 0..h {
            for x in 0..w - 1 {
                let (a, b) = (y * w + x, y * w + x + 1);
                if escaped(a) && escaped(b) && de[a] > 8.0 && de[a] < 60000.0 {
                    smooth += 1;
                    if de[b] <= 0.5 * de[a] || de[b] >= 2.0 * de[a] {
                        jumps += 1;
                    }
                }
            }
        }
        println!("deep DE: {zero} isolated DE 0, {jumps} of {smooth} smooth neighbours jump");
        assert_eq!(zero, 0, "escaped pixels with DE 0 in smooth exterior");
        assert!(smooth > 1000 && jumps * 1000 < smooth, "{jumps} of {smooth}");
    }
}
