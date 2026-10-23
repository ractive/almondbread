//! almondbread-render: the GPU side of almondbread (kb: platform/render-architecture,
//! decisions/rust-core).
//!
//! - [`backend`]: the [`GpuBackend`] trait (buffers, textures, pipelines, command
//!   submission with GPU timestamps, indirect dispatch, presentation).
//! - [`scheduler`]: the chunked progressive [`Renderer`] (jobs and generations, GPU-side
//!   admission, live lists, K adaptation, gesture previews with dynamic resolution and
//!   bilinear upscale, refinement, unresolved-pixel extension passes, adaptive interior
//!   test, stats ring, cancellation), written once against the trait.
//! - [`host`]: [`Host`], the renderer hosted in an interactive view (size, input, one
//!   frame per refresh, interaction measurements); platform UIs wrap it.
//! - [`fog`]: the fog pyramid behind the look of unresolved pixels.
//! - [`floatfloat`]: M7 float-float kernels in their own safe-math library (known-answer
//!   test, verify-only field renderer, benchmarks); not used by the renderer.
//! - [`gpu_types`]: `#[repr(C)]` mirrors of the kernel structs with layout assertions.
//! - `metal` (Apple targets only): the Metal backend on objc2-metal and the
//!   `CAMetalLayer` surface. The MSL kernels live in `src/shaders/`.
//!
//! Platform code lives here and in almondbread-ffi only, never in math or core. On
//! non-Apple targets the crate builds without a backend (Vulkan or wgpu later).

pub use almondbread_core as core;

pub mod accessibility;
pub mod backend;
pub mod buddha;
pub mod cockpit;
pub mod compass;
pub mod cosmic;
pub mod flight;
pub mod floatfloat;
pub mod fog;
pub mod font;
pub mod gpu_types;
pub mod help;
pub mod host;
pub mod julia;
pub mod la_gpu;
pub mod mandelbrot_panel;
pub mod marker;
pub mod memory;
pub mod overlay;
pub mod pi_meter;
pub mod progress;
pub mod scheduler;
pub mod tour;

#[cfg(test)]
mod extreme_tests;

#[cfg(target_vendor = "apple")]
pub mod metal;

pub use backend::{
    ComputeEncoder, GpuBackend, GpuError, MathMode, Storage, Submission, SubmissionStatus, TextureFormat,
};
pub use host::{Host, InputOutcome, InteractionReport};
pub use scheduler::{
    BuddhaShown, FrameMismatch, FrameOutcome, FrameStats, FrameTarget, OffscreenResult, Renderer, SchedulerConfig,
    ShownStats, SimulationResult,
};
