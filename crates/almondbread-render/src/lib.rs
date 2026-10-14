//! almondbread-render: skeleton for R2 (kb: decisions/rust-core).
//!
//! Planned: a `GpuBackend` trait, the chunked scheduler with preview/refine and timing,
//! the MSL kernels, and a Metal backend on `objc2-metal` behind
//! `#[cfg(target_vendor = "apple")]` (Vulkan or wgpu backends later). Platform code lives
//! here and in almondbread-ffi only, never in math or core.

pub use almondbread_core as core;
