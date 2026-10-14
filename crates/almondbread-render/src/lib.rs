//! almondbread-render: the GPU side of almondbread (kb: platform/render-architecture,
//! decisions/rust-core).
//!
//! - [`backend`]: the [`GpuBackend`] trait (buffers, textures, pipelines, command
//!   submission with GPU timestamps, indirect dispatch, presentation).
//! - [`scheduler`]: the chunked progressive [`Renderer`] (jobs and generations, GPU-side
//!   admission, live lists, K adaptation, gesture previews with dynamic resolution and
//!   bilinear upscale, refinement, unresolved-pixel extension passes, adaptive interior
//!   test, stats ring, cancellation), written once against the trait.
//! - [`gpu_types`]: `#[repr(C)]` mirrors of the kernel structs with layout assertions.
//! - `metal` (Apple targets only): the Metal backend on objc2-metal and the
//!   `CAMetalLayer` surface. The MSL kernels live in `src/shaders/`.
//!
//! Platform code lives here and in almondbread-ffi only, never in math or core. On
//! non-Apple targets the crate builds without a backend (Vulkan or wgpu later).

pub use almondbread_core as core;

pub mod backend;
pub mod gpu_types;
pub mod scheduler;

#[cfg(target_vendor = "apple")]
pub mod metal;

pub use backend::{ComputeEncoder, GpuBackend, GpuError, Storage, Submission, SubmissionStatus, TextureFormat};
pub use scheduler::{
    FrameOutcome, FrameTarget, OffscreenResult, Renderer, SchedulerConfig, ShownStats, SimulationResult,
};
