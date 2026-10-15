//! Presentation into a `CAMetalLayer` the UI hands over once (R2b hosts the renderer this way;
//! kb: platform/render-architecture).
//!
//! `nextDrawable` blocks while every drawable of the layer's pool is in use. With two frames
//! in flight the presentation queue can get one frame deeper than the pool and then stays
//! there: each refresh waits ~a refresh for a drawable (measured 2026-10-14 in the Swift
//! renderer: "neck zoom" draw CPU p50 9.4 ms, of which 8.9 ms in `currentDrawable`; one frame
//! in flight removed the wait but cost 15 % frame rate on heavy views).
//!
//! R2b predicted the pool from presented-handler counts instead and skipped a refresh when the
//! prediction said "empty". That prediction lags the real pool (the presented callback comes
//! about a refresh after the drawable is free again, and Core Animation may release a
//! drawable before it reports it presented), so heavy zooms skipped 12–21 refreshes as "no
//! drawable" at 120 Hz where the blocking Swift renderer skipped none (kb: synthesis/benchmarks,
//! M2.5 open issue).
//!
//! FL: a background thread pre-acquires the next drawable with the plain, blocking
//! `nextDrawable` and parks it in a one-slot mailbox. The main thread never blocks;
//! `drawable_ready` is true exactly when the real pool has handed out a drawable, so a
//! refresh is skipped only when the blocking acquire would really have waited. The thread
//! then waits for the next free drawable, which is the Swift renderer's behaviour minus the
//! stall on the main thread. Presentation is still counted (interaction-test logs).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::CGSize;
use objc2_metal::{MTLDrawable, MTLPixelFormat, MTLTexture};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use super::{MetalBackend, MetalDrawable};
use crate::scheduler::FrameTarget;

type LayerDrawable = Retained<ProtocolObject<dyn CAMetalDrawable>>;

/// A drawable crossing from the acquire thread to the frame thread.
struct Parked(LayerDrawable);
// SAFETY: Metal drawables may be used from any thread (Apple: `nextDrawable` and presenting
// are thread-safe); the mailbox hands each one to exactly one owner.
unsafe impl Send for Parked {}

/// The layer, retained by the acquire thread too.
struct SendLayer(Retained<CAMetalLayer>);
// SAFETY: `nextDrawable` is documented thread-safe; the thread calls nothing else on it.
unsafe impl Send for SendLayer {}

/// One-slot mailbox between the acquire thread and the frame thread.
#[derive(Default)]
struct Mailbox {
    slot: Mutex<Option<Parked>>,
    /// Signalled when the slot changes (filled or emptied) or on stop.
    changed: Condvar,
    stop: AtomicBool,
}

/// A `CAMetalLayer` configured for the renderer.
pub struct MetalLayerSurface {
    layer: Retained<CAMetalLayer>,
    mailbox: Arc<Mailbox>,
    presented: Arc<AtomicU64>,
    /// Refreshes skipped because no drawable was ready.
    pub skipped: u64,
    /// Drawables handed out so far; the tag of the last one (never reset).
    serial: u64,
    /// The drawable size last set (parked drawables of another size are dropped).
    size: (usize, usize),
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
        let mailbox = Arc::new(Mailbox::default());
        let size = layer.drawableSize();
        let s = Self {
            layer,
            mailbox,
            presented: Arc::new(AtomicU64::new(0)),
            skipped: 0,
            serial: 0,
            size: (size.width as usize, size.height as usize),
            log: None,
        };
        s.spawn_acquirer();
        s
    }

    /// The acquire thread: whenever the slot is empty, wait (blocking, off the main thread)
    /// for the pool's next drawable and park it. It ends when the surface is dropped (after
    /// its current `nextDrawable`, which times out after a second at most).
    fn spawn_acquirer(&self) {
        let mailbox = Arc::clone(&self.mailbox);
        let layer = SendLayer(self.layer.clone());
        let spawned = std::thread::Builder::new()
            .name("almondbread-drawable".into())
            .spawn(move || {
                let layer = layer;
                loop {
                    {
                        let mut slot = mailbox.slot.lock().unwrap_or_else(|e| e.into_inner());
                        while slot.is_some() && !mailbox.stop.load(Ordering::Acquire) {
                            slot = mailbox.changed.wait(slot).unwrap_or_else(|e| e.into_inner());
                        }
                    }
                    if mailbox.stop.load(Ordering::Acquire) {
                        return;
                    }
                    let size = layer.0.drawableSize();
                    if size.width < 1.0 || size.height < 1.0 {
                        // Not laid out yet: nextDrawable would return nil at once.
                        std::thread::sleep(Duration::from_millis(4));
                        continue;
                    }
                    let d = autoreleasepool(|_| layer.0.nextDrawable());
                    match d {
                        Some(d) => {
                            let mut slot = mailbox.slot.lock().unwrap_or_else(|e| e.into_inner());
                            *slot = Some(Parked(d));
                            mailbox.changed.notify_all();
                        }
                        // Timed out (hidden window) or not displayable: try again shortly.
                        None => std::thread::sleep(Duration::from_millis(4)),
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("almondbread: drawable thread: {e}");
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

    /// Frames presented so far (presented handlers).
    pub fn presented(&self) -> u64 {
        self.presented.load(Ordering::Acquire)
    }

    /// Pixels per point (the window's backing scale); ignored unless finite and positive.
    pub fn set_contents_scale(&self, scale: f64) {
        if scale.is_finite() && scale > 0.0 {
            self.layer.setContentsScale(scale);
        }
    }

    /// Drawable size in pixels (the UI calls this on resize). A parked drawable of the old
    /// size is dropped (back to the pool) when it is next looked at.
    pub fn set_drawable_size(&mut self, width: usize, height: usize) {
        self.size = (width, height);
        self.layer.setDrawableSize(CGSize {
            width: width as f64,
            height: height as f64,
        });
    }

    /// Takes the parked drawable if it has the current size; drops a stale one.
    fn take_parked(&self) -> Option<LayerDrawable> {
        let mut slot = self.mailbox.slot.lock().unwrap_or_else(|e| e.into_inner());
        let fits = slot.as_ref().is_some_and(|p| {
            let t = p.0.texture();
            (t.width(), t.height()) == self.size
        });
        let taken = slot.take();
        self.mailbox.changed.notify_all();
        if fits { taken.map(|p| p.0) } else { None }
    }

    /// Whether [`Self::next_drawable`] would return a drawable without blocking (pass it to
    /// `render_frame_gated` before encoding a frame's work): a drawable of the current size
    /// is parked.
    pub fn drawable_ready(&mut self, _now: f64) -> bool {
        let mut slot = self.mailbox.slot.lock().unwrap_or_else(|e| e.into_inner());
        match slot.as_ref() {
            Some(p) => {
                let t = p.0.texture();
                if (t.width(), t.height()) == self.size {
                    true
                } else {
                    // Resized since it was acquired: back to the pool, fetch a new one.
                    *slot = None;
                    self.mailbox.changed.notify_all();
                    false
                }
            }
            None => false,
        }
    }

    /// The next drawable's texture and the drawable to present, or None if none is ready.
    /// Pass it as `render_frame`'s `acquire`. With `wait` (the view's refined final frame) it
    /// waits up to `max_wait` seconds for the acquire thread (showing that frame one or two
    /// refreshes late measured +15–50 ms to the final frame in the neck-zoom scenario).
    pub fn next_drawable(&mut self, _now: f64, wait: bool) -> Option<FrameTarget<MetalBackend>> {
        self.next_drawable_within(if wait { 0.017 } else { 0.0 })
    }

    fn next_drawable_within(&mut self, max_wait: f64) -> Option<FrameTarget<MetalBackend>> {
        let mut drawable = self.take_parked();
        if drawable.is_none() && max_wait > 0.0 {
            let deadline = std::time::Instant::now() + Duration::from_secs_f64(max_wait);
            let mut slot = self.mailbox.slot.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if let Some(p) = slot.take() {
                    let t = p.0.texture();
                    self.mailbox.changed.notify_all();
                    if (t.width(), t.height()) == self.size {
                        drawable = Some(p.0);
                        break;
                    }
                    continue;
                }
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                if left.is_zero() {
                    break;
                }
                slot = self
                    .mailbox
                    .changed
                    .wait_timeout(slot, left)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        }
        let Some(drawable) = drawable else {
            self.skipped += 1;
            return None;
        };
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
        // The iOS simulator's drawables do not implement presented handlers (sending the
        // message raises an Objective-C exception, which aborts in Rust; kb:
        // platform/ios-simulator): count the frame as presented, without a time.
        if Self::presented_handlers_supported(&generic) {
            // SAFETY: the block is retained by the drawable and only touches an atomic.
            unsafe { generic.addPresentedHandler(RcBlock::as_ptr(&handler)) };
        } else {
            self.presented.fetch_add(1, Ordering::AcqRel);
        }
        Some((texture, Some(generic)))
    }

    /// Whether drawables answer `addPresentedHandler:` (checked once).
    fn presented_handlers_supported(d: &MetalDrawable) -> bool {
        use std::sync::OnceLock;
        static SUPPORTED: OnceLock<bool> = OnceLock::new();
        *SUPPORTED.get_or_init(|| {
            let obj: &objc2::runtime::AnyObject = d.as_ref();
            // SAFETY: respondsToSelector: is NSObject protocol, safe on any object.
            let yes: bool = unsafe { objc2::msg_send![obj, respondsToSelector: objc2::sel!(addPresentedHandler:)] };
            yes
        })
    }
}

impl Drop for MetalLayerSurface {
    fn drop(&mut self) {
        self.mailbox.stop.store(true, Ordering::Release);
        self.mailbox.changed.notify_all();
        // A parked drawable goes back to the pool with the slot.
        if let Ok(mut slot) = self.mailbox.slot.lock() {
            *slot = None;
        }
    }
}
