//! Presentation into a `CAMetalLayer` the UI hands over once (R2b hosts the renderer this way;
//! kb: platform/render-architecture).
//!
//! `nextDrawable` blocks while every drawable of the layer's pool is in use. With two frames
//! in flight the presentation queue can get one frame deeper than the pool and then stays
//! there: each refresh waits ~a refresh for a drawable (measured 2026-10-14 in the Swift
//! renderer: "neck zoom" draw CPU p50 9.4 ms, of which 8.9 ms in `currentDrawable`; one frame
//! in flight removed the wait but cost 15 % frame rate on heavy views). This surface predicts
//! the pool instead (`in_use`: acquired minus presented, see there); when that reaches the
//! pool size, `drawable_ready` is false and the scheduler skips the refresh before encoding
//! any work instead of blocking. Presentation is counted from `addPresentedHandler`, on any
//! thread, with an atomic.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
    /// Drawables handed out so far; the tag of the last one (never reset).
    serial: u64,
    /// `(tag, presentedTime)` of presented drawables, while measuring (interaction tests).
    log: Option<PresentationLog>,
}

/// Presentation times reported by drawables, `(tag, presentedTime)` on the
/// `CACurrentMediaTime` clock. Filled from Core Animation's threads.
pub type PresentationLog = Arc<Mutex<Vec<(u64, f64)>>>;

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
            serial: 0,
            log: None,
        }
    }

    /// Starts (Some) or stops (None) logging presentation times of the drawables acquired
    /// from now on.
    pub fn set_presentation_log(&mut self, log: Option<PresentationLog>) {
        self.log = log;
    }

    /// Tag of the drawable the last successful [`Self::next_drawable`] returned.
    pub fn last_tag(&self) -> u64 {
        self.serial
    }

    pub fn layer(&self) -> &CAMetalLayer {
        &self.layer
    }

    /// Pixels per point (the window's backing scale); ignored unless finite and positive.
    pub fn set_contents_scale(&self, scale: f64) {
        if scale.is_finite() && scale > 0.0 {
            self.layer.setContentsScale(scale);
        }
    }

    /// Drawable size in pixels (the UI calls this on resize).
    pub fn set_drawable_size(&self, width: usize, height: usize) {
        self.layer.setDrawableSize(CGSize {
            width: width as f64,
            height: height as f64,
        });
    }

    /// Whether [`Self::next_drawable`] would return a drawable without blocking (pass it to
    /// `render_frame_gated` before encoding a frame's work).
    pub fn drawable_ready(&mut self, now: f64) -> bool {
        let presented = self.presented.load(Ordering::Acquire);
        if presented != self.last_presented {
            self.last_presented = presented;
            self.last_progress = now;
        }
        let pool = self.layer.maximumDrawableCount() as u64;
        self.in_use() < pool || self.stalled(now)
    }

    /// Hidden or headless layers may not report presentation: after 0.25 s without progress
    /// the plain (possibly blocking) acquire takes over.
    fn stalled(&self, now: f64) -> bool {
        now - self.last_progress > 0.25
    }

    /// Drawables believed in use: acquired and not yet presented. Strictly a drawable is
    /// free again only once its successor is on screen (`acquired - (presented - 1)`), but
    /// the presented callback arrives about a refresh after the fact, which cancels the -1:
    /// with the strict count the interaction test skipped ~40 % of refreshes (36-38 fps at
    /// 60 Hz) although a drawable was free; with this one, 54-59 fps and the acquire waits
    /// at most ~1 ms (measured 2026-10-14, M1 Pro; kb: platform/render-architecture).
    pub fn in_use(&self) -> u64 {
        let presented = self.presented.load(Ordering::Acquire);
        self.acquired.saturating_sub(presented)
    }

    /// The next drawable's texture and the drawable to present, or None if acquiring one
    /// would block (see the module docs). Pass it as `render_frame`'s `acquire`.
    /// With `wait` (the view's refined final frame) it takes the plain acquire, which may
    /// block until a drawable is free (at most about a refresh): showing that frame one or
    /// two refreshes late measured +15-50 ms to the final frame in the neck-zoom scenario.
    pub fn next_drawable(&mut self, now: f64, wait: bool) -> Option<FrameTarget<MetalBackend>> {
        if !wait && !self.drawable_ready(now) {
            self.skipped += 1;
            return None;
        }
        let stalled = self.stalled(now);
        let drawable: Retained<ProtocolObject<dyn CAMetalDrawable>> = self.layer.nextDrawable()?;
        if stalled {
            // Resynchronise: count only what is outstanding from now on.
            self.acquired = self.presented.load(Ordering::Acquire);
            self.last_progress = now;
        }
        self.acquired += 1;
        let texture = drawable.texture();
        self.serial += 1;
        let tag = self.serial;
        let counter = Arc::clone(&self.presented);
        let log = self.log.clone();
        let handler = RcBlock::new(move |d: std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>| {
            counter.fetch_add(1, Ordering::AcqRel);
            if let Some(log) = &log {
                // SAFETY: Core Animation passes the live drawable being presented.
                let t = unsafe { d.as_ref() }.presentedTime();
                // 0: the drawable was dropped, never shown.
                if t > 0.0
                    && let Ok(mut l) = log.lock()
                {
                    l.push((tag, t));
                }
            }
        });
        let generic: MetalDrawable = ProtocolObject::from_retained(drawable);
        // SAFETY: the block is retained by the drawable and only touches an atomic.
        unsafe { generic.addPresentedHandler(RcBlock::as_ptr(&handler)) };
        Some((texture, Some(generic)))
    }
}
