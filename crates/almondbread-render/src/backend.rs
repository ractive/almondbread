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
}

impl TextureFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rg32Float | Self::Rgba16Float => 8,
            Self::Rg16Float | Self::Bgra8Unorm => 4,
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

/// A GPU device with one serial queue and the kernel library compiled.
pub trait GpuBackend {
    type Buffer: Clone;
    type Texture: Clone;
    type Pipeline: Clone;
    type Drawable;
    type Submission: Submission;
    type Encoder: ComputeEncoder<Self>;

    fn device_name(&self) -> String;

    fn create_buffer(&self, len: usize, storage: Storage) -> Result<Self::Buffer, GpuError>;
    /// A shared buffer initialised with `data`.
    fn create_buffer_with_data(&self, data: &[u8]) -> Result<Self::Buffer, GpuError>;
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

    /// The kernel `function` specialised with boolean function constants 0, 1, ...
    fn pipeline(&self, function: &str, constants: &[bool]) -> Result<Self::Pipeline, GpuError>;
    /// (SIMD width, max threads per threadgroup) of a pipeline.
    fn pipeline_limits(&self, pipeline: &Self::Pipeline) -> (usize, usize);

    /// Starts a command buffer with one compute pass.
    fn begin(&self, label: &str) -> Result<Self::Encoder, GpuError>;
}
