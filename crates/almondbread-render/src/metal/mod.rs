//! Metal implementation of [`GpuBackend`] on objc2-metal (kb: platform/render-architecture,
//! platform/metal-compute). Rust owns the device, the queue, the compiled library, every
//! buffer, texture and pipeline. The kernels are the crate's MSL source, compiled at runtime
//! with the Swift renderer's options (MSL 3.0, fast math).

mod surface;

pub use surface::{MetalLayerSurface, PresentationLog};

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

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
pub const KERNEL_SOURCE: &str = include_str!("../shaders/mandelbrot.metal");

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
    /// Command buffers committed through [`GpuBackend::begin`] and not yet finished, shared by
    /// the clones on this queue ([`GpuBackend::submissions_in_flight`]).
    committed: Arc<Mutex<Vec<CommandBuffer>>>,
}

/// A committed command buffer, kept only to read its status.
struct CommandBuffer(Retained<ProtocolObject<dyn MTLCommandBuffer>>);

// SAFETY: after `commit` the buffer is only retained, released and asked for its `status`,
// which Metal allows from any thread (its completion handlers run on Metal's own threads);
// nothing encodes into it any more.
unsafe impl Send for CommandBuffer {}
unsafe impl Sync for CommandBuffer {}

unsafe extern "C" {
    fn sysctlbyname(
        name: *const std::ffi::c_char,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    ) -> i32;
}

/// Performance cores (`hw.perflevel0.logicalcpu`; None on a machine without perf levels).
pub fn performance_cores() -> Option<usize> {
    let mut v: i32 = 0;
    let mut len = size_of::<i32>();
    // SAFETY: a NUL-terminated name and an out buffer of `len` bytes; no new value.
    let r = unsafe {
        sysctlbyname(
            c"hw.perflevel0.logicalcpu".as_ptr(),
            (&raw mut v).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (r == 0 && len == size_of::<i32>() && v > 0).then_some(v as usize)
}

/// The reference orbit's three-thread rule needs the performance-core count, which the
/// platform-free math crate cannot read (review P1, kb: techniques/parallel-reference-orbit).
fn tell_orbit_the_core_layout() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Some(n) = performance_cores() {
            almondbread_core::math::set_performance_cores(n);
        }
    });
}

impl MetalBackend {
    /// The system default GPU with the kernels compiled from [`KERNEL_SOURCE`].
    pub fn new() -> Result<Self, GpuError> {
        // `ALMONDBREAD_KERNEL_SALT=x` changes the iterate kernel's code (a test that
        // never passes), so Metal's shader cache misses as after an app update: the cold cost
        // of pipeline variants (kb: platform/metal-compute "Pipeline warm-up").
        if let Ok(salt) = std::env::var("ALMONDBREAD_KERNEL_SALT") {
            let n = salt
                .bytes()
                .fold(0x8000_0000u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)))
                | 0x8000_0000;
            let source = KERNEL_SOURCE.replacen(
                "    const uint liveIn = *countIn;",
                &format!("    if (p.total == {n}u) {{ return; }}\n    const uint liveIn = *countIn;"),
                1,
            );
            return Self::with_source(&source);
        }
        Self::with_source(KERNEL_SOURCE)
    }

    pub fn with_source(source: &str) -> Result<Self, GpuError> {
        tell_orbit_the_core_layout();
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
            Ok(Self {
                device,
                queue,
                library,
                committed: Arc::new(Mutex::new(Vec::new())),
            })
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

/// One command buffer with one compute pass. Dropped without [`ComputeEncoder::commit`] (a
/// panic between `begin` and `commit`, caught by the frame's recovery), it still ends the
/// pass: Metal requires `endEncoding` before an encoder is released, and the command buffer
/// is then released uncommitted (nothing runs).
pub struct MetalEncoder {
    cb: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    enc: Retained<ProtocolObject<dyn MTLComputeCommandEncoder>>,
    present: Option<MetalDrawable>,
    committed: Arc<Mutex<Vec<CommandBuffer>>>,
    ended: bool,
}

impl Drop for MetalEncoder {
    fn drop(&mut self) {
        if !self.ended {
            self.enc.endEncoding();
        }
    }
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

    fn commit(mut self) -> MetalSubmission {
        self.enc.endEncoding();
        self.ended = true;
        if let Some(d) = &self.present {
            self.cb.presentDrawable(d);
        }
        self.cb.commit();
        if let Ok(mut v) = self.committed.lock() {
            v.push(CommandBuffer(self.cb.clone()));
        }
        MetalSubmission(self.cb.clone())
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

    fn allocated_bytes(&self) -> Option<u64> {
        Some(self.device.currentAllocatedSize() as u64)
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

    fn uploader(&self) -> Option<crate::backend::Uploader<MetalBuffer>> {
        // MTLDevice is thread-safe; buffers made on the reference worker's thread are used on
        // the frame thread after the hand-over (kb: platform/metal-compute).
        let backend = self.clone();
        Some(Arc::new(move |data: &[u8]| {
            let b = backend.create_buffer_with_data(data)?;
            // SAFETY: Metal copied `data` into the new buffer; nothing touches its contents
            // from the CPU again, and retain/release are thread-safe.
            Ok(unsafe { crate::backend::SentBuffer::new(b) })
        }))
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
        self.pipeline_with(function, constants, &[])
    }

    fn pipeline_with(
        &self,
        function: &str,
        constants: &[bool],
        uints: &[(usize, u32)],
    ) -> Result<MetalPipeline, GpuError> {
        autoreleasepool(|_| {
            let name = NSString::from_str(function);
            let f = if constants.is_empty() && uints.is_empty() {
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
                for &(i, v) in uints {
                    let mut u = v;
                    // SAFETY: a valid u32 for MTLDataType::UInt; Metal copies the value.
                    unsafe {
                        values.setConstantValue_type_atIndex(NonNull::from(&mut u).cast(), MTLDataType::UInt, i);
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

    fn pipelines_in_background(
        &self,
        requests: Vec<(&'static str, Vec<bool>)>,
    ) -> Option<std::sync::mpsc::Receiver<(usize, MetalPipeline)>> {
        // A few threads take the requests in order (a cold variant of the iterate kernel costs
        // 60-250 ms of one core; 57 of them on one thread took ~6 s, longer than a fast fly-to
        // needs to reach the deep kernel).
        let (tx, rx) = std::sync::mpsc::channel();
        let queue = std::sync::Arc::new(std::sync::Mutex::new(
            requests
                .into_iter()
                .enumerate()
                .collect::<std::collections::VecDeque<_>>(),
        ));
        let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 3).clamp(1, 4));
        let mut spawned = false;
        for k in 0..threads {
            let (tx, queue, backend) = (tx.clone(), queue.clone(), self.clone());
            let ok = std::thread::Builder::new()
                .name(format!("almondbread-pipelines-{k}"))
                .spawn(move || {
                    loop {
                        let Some((i, (function, constants))) = queue.lock().ok().and_then(|mut q| q.pop_front()) else {
                            return;
                        };
                        match backend.pipeline(function, &constants) {
                            Ok(p) => {
                                if tx.send((i, p)).is_err() {
                                    return;
                                }
                            }
                            Err(e) => eprintln!("almondbread: pipeline warm-up {function}: {e}"),
                        }
                    }
                })
                .is_ok();
            spawned |= ok;
        }
        spawned.then_some(rx)
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
        Ok(MetalEncoder {
            cb,
            enc,
            present: None,
            committed: Arc::clone(&self.committed),
            ended: false,
        })
    }

    fn submissions_in_flight(&self) -> usize {
        let Ok(mut v) = self.committed.lock() else {
            return 0;
        };
        v.retain(|cb| {
            !matches!(
                cb.0.status(),
                MTLCommandBufferStatus::Completed | MTLCommandBufferStatus::Error
            )
        });
        v.len()
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

    /// Committed buffers count until the GPU finishes them (the host's queue guard).
    #[test]
    fn counts_submissions_in_flight() {
        let b = MetalBackend::new().expect("metal");
        assert_eq!(b.submissions_in_flight(), 0);
        let subs: Vec<_> = (0..5).map(|_| b.begin("count").expect("begin").commit()).collect();
        // A clone on the same queue sees the same count.
        assert!(b.clone().submissions_in_flight() <= 5);
        for s in subs {
            s.wait();
        }
        assert_eq!(b.submissions_in_flight(), 0);
    }

    /// An encoder dropped without commit (a panic mid-encoding) ends its pass; the queue
    /// keeps working (review C5). Run with `MTL_DEBUG_LAYER=1` to have Metal check the
    /// endEncoding rule.
    #[test]
    fn dropped_encoder_ends_its_pass() {
        let b = MetalBackend::new().expect("metal");
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut enc = b.begin("abandoned").expect("begin");
            enc.set_bytes(0, &[0u8; 5000]); // asserts: more than setBytes takes
        }));
        assert!(r.is_err());
        drop(b.begin("abandoned twice").expect("begin"));
        let s = b.begin("after").expect("begin").commit();
        s.wait();
        assert!(matches!(s.status(), SubmissionStatus::Completed), "{:?}", s.status());
        assert_eq!(b.submissions_in_flight(), 0);
    }

    /// The performance-core count reaches the orbit's helper rule.
    #[test]
    fn core_layout_reaches_the_orbit() {
        let _b = MetalBackend::new().expect("metal");
        let p = performance_cores().expect("Apple silicon has perf levels");
        assert!(p >= 1);
        assert_eq!(
            almondbread_core::math::helpers_pay(),
            p >= almondbread_core::math::MIN_PERFORMANCE_CORES
        );
    }

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
             out[15] = sizeof(SurveyParams); out[16] = sizeof(PixelStateProbe); out[17] = sizeof(ComponentParams); }}\n"
        );
        let b = MetalBackend::with_source(&probe).expect("metal");
        let p = b.pipeline("layoutProbe", &[]).unwrap();
        let out = b.create_buffer(128, Storage::Shared).unwrap();
        let mut enc = b.begin("probe").unwrap();
        enc.set_pipeline(&p);
        enc.set_buffer(0, &out, 0);
        enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
        enc.commit().wait();
        let mut bytes = [0u8; 72];
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
                size_of::<crate::gpu_types::SurveyParams>() as u32,
                size_of::<crate::gpu_types::PixelStateProbe>() as u32,
                size_of::<crate::gpu_types::ComponentParams>() as u32,
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

    /// M5: seed rounds are resumable: a tiny step budget (many rounds) gives the same field
    /// bit for bit as one unbounded round, in the plain and in the deep kernel (shown frame =
    /// fresh render whatever the rounds' timing).
    #[test]
    fn local_sa_rounds_resumable() {
        use crate::scheduler::LocalSa;
        for (name, w, h) in [
            ("dense-5e-14", 320, 200),
            ("sea-1e20", 320, 200),
            ("dendrite-1e300", 320, 200),
        ] {
            let small = LocalSa {
                budget: 5,
                ..LocalSa::default()
            };
            let big = LocalSa {
                budget: u32::MAX,
                ..LocalSa::default()
            };
            let (a, _, ra) = sa_render(small, name, w, h, true);
            let (b, _, rb) = sa_render(big, name, w, h, true);
            assert!(ra.expect("seeded").rounds > 3, "{name}");
            assert_eq!(rb.expect("seeded").rounds, 1, "{name}");
            assert_eq!(a.bitwise_differences(&b), 0, "{name}");
        }
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

    /// The learned iteration limit survives the idle anti-aliasing and is what the next view
    /// starts from (kb: techniques/auto-iterations). At the -3/4 neck the passes find escapes
    /// far past the depth limit, so the learner raises it; the view then anti-aliases (9
    /// samples) and idles, and a short pan gesture follows. A from-scratch renderer at the
    /// settled view (as the flight benchmark's jump check makes) has its own learner: its
    /// `learn: ... -> 0` trace line is not the live renderer's.
    #[test]
    fn learned_limit_survives_aa() {
        use crate::host::Host;
        use crate::scheduler::Renderer;
        use objc2_quartz_core::CAMetalLayer;
        let backend = MetalBackend::new().expect("metal");
        let mut surface = MetalLayerSurface::new(CAMetalLayer::new(), &backend);
        surface.set_drawable_size(640, 410);
        let mut host = Host::new(Renderer::new(backend).expect("renderer"));
        assert_eq!(host.renderer.settings.supersampling.factor(), 3, "the app's default AA");
        host.resize(320.0, 205.0, 2.0, 0.0);
        host.renderer.viewport =
            almondbread_core::Viewport::new(-0.7500016386523336, -0.002107342465685259, 6.77052e-8);
        // (A cap keeps the extension passes short: the neck's slow escapes would otherwise run
        // them to ~110 000 iterations, a second of GPU; the learned limit is ~7 000.)
        host.renderer.settings.max_iterations_cap = 30_000;
        let depth = almondbread_core::settings::depth_iterations(&host.renderer.viewport);
        let start = std::time::Instant::now();
        let now = || start.elapsed().as_secs_f64();
        // (until_settled: stop once settled; pan: move the view as a gesture each refresh.)
        let mut frames = |host: &mut Host<MetalBackend>, n: usize, pan: bool, until_settled: bool| {
            for _ in 0..n {
                let t = now();
                if pan {
                    host.renderer.note_interaction(t);
                    host.renderer.viewport.pan_by_points(0.5, 0.0);
                }
                let ready = surface.drawable_ready(t);
                host.frame(t, &now, ready, &mut |f| surface.next_drawable(t, f).map(|d| (d, 0)));
                std::thread::sleep(std::time::Duration::from_millis(4));
                if until_settled && host.renderer.is_settled() {
                    return true;
                }
            }
            false
        };
        assert!(frames(&mut host, 3000, false, true), "the view did not settle");
        let learned = host.renderer.settings.learned_iterations;
        assert!(learned > depth, "learned {learned}, depth {depth}");
        // Idle after the AA: nothing changes.
        frames(&mut host, 100, false, false);
        assert_eq!(host.renderer.settings.learned_iterations, learned);
        // A fresh renderer at the view does not touch the live one's state.
        let mut fresh = Renderer::new(host.renderer.backend().clone()).expect("renderer");
        fresh.viewport = host.renderer.viewport.clone();
        fresh.render_offscreen(320, 205, false).expect("render");
        assert_eq!(host.renderer.settings.learned_iterations, learned);
        // The next view (a pan) starts from the learned limit.
        frames(&mut host, 60, true, false);
        assert!(frames(&mut host, 3000, false, true), "the pan did not settle");
        assert!(
            host.renderer.stats.max_iterations >= learned,
            "next view at {} after learning {learned}",
            host.renderer.stats.max_iterations
        );
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

    /// Component Newton (kb: techniques/interior-detection "Component Newton"): at the edge of
    /// a period-22 component (|λ| up to ~0.995, the orbit needs thousands of periods to settle)
    /// the interior probe proves the interior from z = 0 at the view's period, which the |w| n²
    /// test and the probe's scan left unresolved at the limit (79 % proven of 86.8 % interior
    /// before, `neck_truth` at 10^7), and never a pixel that escapes.
    #[test]
    fn component_newton_proves_slow_interior() {
        use crate::scheduler::{BAILOUT, Renderer};
        use almondbread_core::oracle::{ITER_INTERIOR, cpu_render};
        use almondbread_core::viewport::{Center, Scale, Viewport};
        let mut r = Renderer::new(MetalBackend::new().expect("metal")).expect("renderer");
        r.settings = almondbread_core::RenderSettings::plain();
        r.settings.auto_iterations = false;
        r.settings.manual_iterations = 1151;
        r.settings.supersampling = almondbread_core::Supersampling::Off;
        let (w, h, scale) = (160, 100, 1.48e-5);
        let (re, im) = (-0.787_329_172_181_723_8, 0.137_594_319_554_082_24);
        r.viewport = Viewport::with_center(Center::new(re, im), Scale::from_f64(scale));
        let out = r.render_offscreen(w, h, true).expect("render");
        let iters = out.iterations.expect("iterations");
        let cpu = cpu_render(re, im, scale, w, h, 20_000, BAILOUT, 0.0);
        let interior = (0..w * h).filter(|&k| iters[k][0] == ITER_INTERIOR).count();
        let false_interior = (0..w * h)
            .filter(|&k| iters[k][0] == ITER_INTERIOR && cpu.n[k] >= 0.0)
            .count();
        println!(
            "component Newton: {interior} of {} interior, {false_interior} escape",
            w * h
        );
        assert_eq!(false_interior, 0);
        assert!(interior * 100 >= 84 * w * h, "{interior}");
    }
}

#[cfg(test)]
mod upload_tests {
    use super::*;

    /// The π meter's trail picture at Retina size (820 x 400 BGRA8) goes up as a shared
    /// buffer: a few uploads a second must stay far below the frame budget. (A shared
    /// texture took 0.26 ms per picture, nearly all of it the copy into the tiled layout.)
    #[test]
    fn trail_upload_is_cheap() {
        let b = MetalBackend::new().expect("metal");
        let data: Vec<u8> = (0..820 * 400 * 4).map(|i| (i * 7 % 251) as u8).collect();
        let buf = b.create_buffer_with_data(&data).expect("upload");
        let mut back = vec![0u8; data.len()];
        b.read_buffer(&buf, 0, &mut back);
        assert_eq!(back, data);
        let t0 = std::time::Instant::now();
        for _ in 0..20 {
            std::hint::black_box(b.create_buffer_with_data(&data).expect("upload"));
        }
        let per = t0.elapsed().as_secs_f64() / 20.0;
        eprintln!("trail upload 820x400 as a buffer: {:.3} ms", per * 1000.0);
        if !cfg!(debug_assertions) {
            assert!(per < 0.002, "{per}");
        }
    }
}
