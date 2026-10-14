//! Presentation into a `CAMetalLayer` the UI hands over once (R2b hosts the renderer this way;
//! kb: platform/render-architecture).
//!
//! `nextDrawable` blocks while every drawable of the layer's pool is in use. With two frames
//! in flight the presentation queue can get one frame deeper than the pool and then stays
//! there: each refresh waits ~a refresh for a drawable (measured 2026-10-14 in the Swift
//! renderer: "neck zoom" draw CPU p50 9.4 ms, of which 8.9 ms in `currentDrawable`; one frame
//! in flight removed the wait but cost 15 % frame rate on heavy views). This surface predicts
//! the pool instead: a drawable returns to the pool once its successor is on screen, so
//! `in use = acquired - max(presented - 1, 0)`; when that reaches the pool size it reports
//! "none available" and the scheduler skips the refresh instead of blocking. Presentation is
//! counted from `addPresentedHandler`, on any thread, with an atomic.

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::CGSize;
use objc2_metal::{MTLDrawable, MTLPixelFormat};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use super::{MetalBackend, MetalDrawable};
use crate::scheduler::FrameTarget;

/// A `CAMetalLayer` configured for the renderer.
pub struct MetalLayerSurface {
    layer: Retained<CAMetalLayer>,
    acquired: u64,
    presented: Arc<AtomicU64>,
    /// When presentation counts last moved (a hidden window may never report presentation).
    last_progress: f64,
    last_presented: u64,
    /// Refreshes skipped because the pool was predicted empty.
    pub skipped: u64,
}

impl MetalLayerSurface {
    /// Wraps a layer the UI created (an `MTKView`'s or a plain `CAMetalLayer`).
    ///
    /// # Safety
    /// `layer` must point to a live `CAMetalLayer`; it is retained here.
    pub unsafe fn from_raw(layer: *mut c_void, backend: &MetalBackend) -> Option<Self> {
        // SAFETY: the caller guarantees a live CAMetalLayer.
        let layer = unsafe { Retained::retain(layer.cast::<CAMetalLayer>())? };
        Some(Self::new(layer, backend))
    }

    pub fn new(layer: Retained<CAMetalLayer>, backend: &MetalBackend) -> Self {
        layer.setDevice(Some(backend.device()));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        // The colorize kernel writes the drawable directly.
        layer.setFramebufferOnly(false);
        Self {
            layer,
            acquired: 0,
            presented: Arc::new(AtomicU64::new(0)),
            last_progress: 0.0,
            last_presented: 0,
            skipped: 0,
        }
    }

    pub fn layer(&self) -> &CAMetalLayer {
        &self.layer
    }

    /// Drawable size in pixels (the UI calls this on resize).
    pub fn set_drawable_size(&self, width: usize, height: usize) {
        self.layer.setDrawableSize(CGSize {
            width: width as f64,
            height: height as f64,
        });
    }

    /// Drawables believed in use (acquired and not yet replaced on screen).
    pub fn in_use(&self) -> u64 {
        let presented = self.presented.load(Ordering::Acquire);
        self.acquired.saturating_sub(presented.saturating_sub(1))
    }

    /// The next drawable's texture and the drawable to present, or None if acquiring one
    /// would block (see the module docs). Pass it as `render_frame`'s `acquire`.
    pub fn next_drawable(&mut self, now: f64) -> Option<FrameTarget<MetalBackend>> {
        let presented = self.presented.load(Ordering::Acquire);
        if presented != self.last_presented {
            self.last_presented = presented;
            self.last_progress = now;
        }
        let pool = self.layer.maximumDrawableCount() as u64;
        // Hidden or headless layers may not report presentation: after 0.25 s without
        // progress fall back to the plain (possibly blocking) acquire.
        let stalled = now - self.last_progress > 0.25;
        if self.in_use() >= pool && !stalled {
            self.skipped += 1;
            return None;
        }
        let drawable: Retained<ProtocolObject<dyn CAMetalDrawable>> = self.layer.nextDrawable()?;
        if stalled {
            // Resynchronise: count only what is outstanding from now on.
            self.acquired = self.presented.load(Ordering::Acquire);
            self.last_progress = now;
        }
        self.acquired += 1;
        let texture = drawable.texture();
        let counter = Arc::clone(&self.presented);
        let handler = RcBlock::new(move |_d: std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>| {
            counter.fetch_add(1, Ordering::AcqRel);
        });
        let generic: MetalDrawable = ProtocolObject::from_retained(drawable);
        // SAFETY: the block is retained by the drawable and only touches an atomic.
        unsafe { generic.addPresentedHandler(RcBlock::as_ptr(&handler)) };
        Some((texture, Some(generic)))
    }
}
