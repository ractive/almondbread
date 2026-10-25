//! The GPU backend trait the scheduler is written against (kb: platform/render-architecture).
//!
//! A compute-only subset of a modern GPU API: buffers, 2D textures, specialised compute
//! pipelines from the kernel library, one serial queue of command submissions with GPU
//! timestamps, indirect dispatch, and presentation of a drawable. Metal implements it in
//! `metal/`; a Vulkan or wgpu backend would implement the same trait with the kernels ported.
//!
//! Handles are cheap to clone (reference counted) and keep their resource alive: a job keeps
//! its reference-orbit buffer and level textures while submissions still use them, and the
//! backend keeps everything a submitted command buffer binds alive until it completes.

use std::fmt;

/// Where a buffer or texture lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// GPU only.
    Private,
    /// CPU-visible (unified memory on Apple GPUs).
    Shared,
}

/// Texture formats the kernels use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureFormat {
    /// Iteration texels: escape iteration n, smooth offset.
    Rg32Float,
    /// Distance estimate, normal angle.
    Rg16Float,
    /// Linear colour of a preview level (bilinear upsampling).
    Rgba16Float,
    /// sRGB-encoded output (drawables, offscreen targets).
    Bgra8Unorm,
    /// One unsigned byte (the history's class epoch per pixel).
    R8Uint,
}

impl TextureFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rg32Float | Self::Rgba16Float => 8,
            Self::Rg16Float | Self::Bgra8Unorm => 4,
            Self::R8Uint => 1,
        }
    }
}

/// Status of a committed submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionStatus {
    Pending,
    Completed,
    /// The GPU reported an error (most likely a watchdog timeout).
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuError(pub String);

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GpuError {}

/// A committed command buffer.
pub trait Submission {
    fn status(&self) -> SubmissionStatus;
    /// GPU execution time (end - start) in seconds, once completed.
    fn gpu_time(&self) -> f64;
    /// Blocks until the GPU has finished it.
    fn wait(&self);
}

/// Records one compute pass into one command buffer. Binding indices are the kernel's
/// `[[buffer(i)]]` / `[[texture(i)]]` slots.
pub trait ComputeEncoder<B: GpuBackend + ?Sized> {
    fn set_pipeline(&mut self, pipeline: &B::Pipeline);
    fn set_bytes(&mut self, index: usize, bytes: &[u8]);
    fn set_buffer(&mut self, index: usize, buffer: &B::Buffer, offset: usize);
    fn set_texture(&mut self, index: usize, texture: &B::Texture);
    /// `threads` total (non-uniform threadgroups allowed), `per_group` threads per group.
    fn dispatch_threads(&mut self, threads: [usize; 3], per_group: [usize; 3]);
    /// Threadgroup count read from `args` at `offset` (three u32) when the GPU runs it.
    fn dispatch_indirect(&mut self, args: &B::Buffer, offset: usize, per_group: [usize; 3]);
    /// Presents `drawable` once the commands before have run.
    fn present(&mut self, drawable: &B::Drawable);
    /// Ends encoding and submits to the serial queue.
    fn commit(self) -> B::Submission;
}

/// Floating-point semantics a separately compiled library asks for (M7, kb:
/// techniques/double-float).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathMode {
    /// IEEE rounding, no reassociation (the float-float library).
    Safe,
    /// The main library's fast math (only to show what it does to the float-float code).
    Fast,
}

/// Makes a shared buffer initialised with the given bytes, from any thread.
pub type Uploader<Buf> = std::sync::Arc<dyn Fn(&[u8]) -> Result<SentBuffer<Buf>, GpuError> + Send + Sync>;

/// A buffer an [`Uploader`] made on one thread, to be used on another.
pub struct SentBuffer<Buf>(Buf);

impl<Buf> SentBuffer<Buf> {
    /// # Safety
    /// `buf` may be moved to, retained, released and bound on another thread: nothing reads or
    /// writes its contents from the CPU any more (Metal: a buffer initialised by
    /// `newBufferWithBytes`; retain and release are atomic).
    pub unsafe fn new(buf: Buf) -> Self {
        Self(buf)
    }

    pub fn get(&self) -> &Buf {
        &self.0
    }
}

// SAFETY: see `SentBuffer::new`, whose caller vouches for it.
unsafe impl<Buf> Send for SentBuffer<Buf> {}
unsafe impl<Buf> Sync for SentBuffer<Buf> {}

/// A GPU device with one serial queue and the kernel library compiled.
pub trait GpuBackend {
    type Buffer: Clone + 'static;
    type Texture: Clone;
    type Pipeline: Clone;
    type Drawable;
    type Submission: Submission;
    type Encoder: ComputeEncoder<Self>;

    fn device_name(&self) -> String;

    /// Bytes of GPU memory this process holds on the device (buffers, textures, drawables),
    /// if the backend can tell (Metal: `currentAllocatedSize`). Feeds the memory report and
    /// budget (kb: platform/ios-device "Memory").
    fn allocated_bytes(&self) -> Option<u64> {
        None
    }

    fn create_buffer(&self, len: usize, storage: Storage) -> Result<Self::Buffer, GpuError>;
    /// A shared buffer initialised with `data`.
    fn create_buffer_with_data(&self, data: &[u8]) -> Result<Self::Buffer, GpuError>;
    /// [`GpuBackend::create_buffer_with_data`] callable from any thread: the reference worker
    /// uploads a long orbit on its own thread instead of the frame thread (C2 of the 2026-10
    /// review). None: buffers are made only where the backend lives.
    fn uploader(&self) -> Option<Uploader<Self::Buffer>> {
        None
    }
    /// Copies from a shared buffer. The caller ensures no pending submission writes it.
    fn read_buffer(&self, buffer: &Self::Buffer, offset: usize, out: &mut [u8]);

    /// A 2D texture readable and writable by kernels.
    fn create_texture(
        &self,
        width: usize,
        height: usize,
        format: TextureFormat,
        storage: Storage,
    ) -> Result<Self::Texture, GpuError>;
    fn texture_size(&self, texture: &Self::Texture) -> (usize, usize);
    /// Copies the region (0, 0, width, height) into `out` (tightly packed rows), waiting for
    /// the queue. Private textures go through a staging copy.
    fn read_texture(
        &self,
        texture: &Self::Texture,
        width: usize,
        height: usize,
        format: TextureFormat,
        out: &mut [u8],
    ) -> Result<(), GpuError>;

    /// Copies the region (0, 0, width, height) of a *shared* texture into `out` without
    /// waiting for the queue: the caller knows every submission writing it has completed
    /// (headless capture of frames while later frames are already queued).
    fn read_shared_texture(
        &self,
        texture: &Self::Texture,
        width: usize,
        height: usize,
        format: TextureFormat,
        out: &mut [u8],
    );

    /// The kernel `function` specialised with boolean function constants 0, 1, ...
    fn pipeline(&self, function: &str, constants: &[bool]) -> Result<Self::Pipeline, GpuError>;
    /// [`GpuBackend::pipeline`] plus unsigned-integer function constants `(index, value)` (F2:
    /// `kFormula`, `kPower`). With none it must be exactly [`GpuBackend::pipeline`].
    fn pipeline_with(
        &self,
        function: &str,
        constants: &[bool],
        uints: &[(usize, u32)],
    ) -> Result<Self::Pipeline, GpuError> {
        if uints.is_empty() {
            self.pipeline(function, constants)
        } else {
            Err(GpuError(format!(
                "{function}: integer function constants are not supported"
            )))
        }
    }
    /// Compiles `requests` (kernel name, function constants) one after the other on a thread of
    /// its own and sends `(index into requests, pipeline)` as each is ready: the renderer's
    /// warm-up, so a frame that first needs a variant does not compile it (kb:
    /// platform/metal-compute "Pipeline warm-up"). None: this backend compiles only where
    /// [`GpuBackend::pipeline`] is called.
    fn pipelines_in_background(
        &self,
        requests: Vec<(&'static str, Vec<bool>)>,
    ) -> Option<std::sync::mpsc::Receiver<(usize, Self::Pipeline)>> {
        let _ = requests;
        None
    }
    /// (SIMD width, max threads per threadgroup) of a pipeline.
    fn pipeline_limits(&self, pipeline: &Self::Pipeline) -> (usize, usize);

    /// Starts a command buffer with one compute pass.
    fn begin(&self, label: &str) -> Result<Self::Encoder, GpuError>;

    /// Command buffers committed on this queue that the GPU has not finished. Metal hands out
    /// at most 64 per queue and `commandBuffer()` blocks the caller (the main thread) until one
    /// frees up, so the host skips refreshes while many are queued (kb:
    /// techniques/hang-prevention "GPU queue"). Backends that never block return 0.
    fn submissions_in_flight(&self) -> usize {
        0
    }

    /// Compiles `source` as a library of its own with `math` semantics and returns a pipeline
    /// per name in `functions` (M7: the float-float kernels, which must not share the fast
    /// library's compile options or registers). Backends without one return an error.
    fn library_pipelines(
        &self,
        source: &str,
        functions: &[&str],
        math: MathMode,
    ) -> Result<Vec<Self::Pipeline>, GpuError> {
        let _ = (source, functions, math);
        Err(GpuError("separate libraries not supported by this backend".into()))
    }
}
