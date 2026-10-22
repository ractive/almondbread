//! Keeping work proportional to a reference's length off the frame thread (C1/C2 of the
//! 2026-10 review; kb: techniques/hang-prevention "Long references"): the GPU form of a
//! reference is made on the thread that computed it, and BLA/LA table builds of a replaced
//! reference are cancelled.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use almondbread_core::bla::{BLA_EPSILON, BlaTableX};
use almondbread_core::reference::{Reference, ReferenceAttachment, ReferencePrepare};

use crate::backend::{GpuBackend, GpuError, SentBuffer};

/// The GPU buffers of a reference and what the scheduler derives from its whole orbit.
pub(super) struct GpuReference<Buf> {
    /// float32 points.
    pub points: Buf,
    /// floatexp points in the padded GPU form (big references).
    pub extended: Option<Buf>,
    /// `log2(eps max |2 Z|)`: no BLA block is valid for a pixel farther than this.
    pub bla_reach_log2: f64,
}

/// Builds the GPU form of `r` with `upload` (any thread).
pub(super) fn gpu_reference<Buf>(
    r: &Reference,
    upload: &dyn Fn(&[u8]) -> Result<Buf, GpuError>,
) -> Result<GpuReference<Buf>, GpuError> {
    let points = upload(bytemuck::cast_slice(&r.points[..]))?;
    let extended = if r.extended.is_empty() {
        None
    } else {
        // FloatExpComplex has a padding word: copy into the padded GPU form. (F6: a folding
        // formula's points with each part its own exponent, the second in the padding word:
        // MSL `RefXS`.)
        if r.formula.folds() {
            let gpu: Vec<[u32; 4]> = if r.split.len() == r.extended.len() {
                r.split
                    .iter()
                    .map(|z| [z.mantissa[0].to_bits(), z.mantissa[1].to_bits(), z.exponent[0] as u32, z.exponent[1] as u32])
                    .collect()
            } else {
                r.extended
                    .iter()
                    .map(|z| {
                        let s = almondbread_core::formula::FloatExpSplit::from_double_exp(shared_to_double(z));
                        [s.mantissa[0].to_bits(), s.mantissa[1].to_bits(), s.exponent[0] as u32, s.exponent[1] as u32]
                    })
                    .collect()
            };
            return Ok(GpuReference {
                points,
                extended: Some(upload(bytemuck::cast_slice(&gpu))?),
                bla_reach_log2: bla_reach_log2(r),
            });
        }
        let gpu: Vec<[u32; 4]> = r
            .extended
            .iter()
            .map(|z| [z.mantissa[0].to_bits(), z.mantissa[1].to_bits(), z.exponent as u32, 0])
            .collect();
        Some(upload(bytemuck::cast_slice(&gpu))?)
    };
    Ok(GpuReference {
        points,
        extended,
        bla_reach_log2: bla_reach_log2(r),
    })
}

/// A shared-exponent point in extended range (each part exact).
fn shared_to_double(z: &almondbread_core::formula::FloatExpComplex) -> almondbread_core::formula::DoubleExpComplex {
    use almondbread_core::formula::DoubleExp;
    let part = |m: f32| DoubleExp::from_f64(f64::from(m)).times_power_of_two(i64::from(z.exponent));
    almondbread_core::formula::DoubleExpComplex::new(part(z.mantissa[0]), part(z.mantissa[1]))
}

/// `log2(eps max |2 Z|)` of the reference the kernel iterates against.
pub(super) fn bla_reach_log2(r: &Reference) -> f64 {
    let eps = BLA_EPSILON;
    if r.deep_kernel {
        BlaTableX::reach_log2(&r.extended, eps)
    } else {
        let n = r.points.len().saturating_sub(1);
        r.points[1.min(n)..n]
            .iter()
            .map(|z| eps * 2.0 * f64::from(z[0]).hypot(f64::from(z[1])))
            .fold(0.0, f64::max)
            .log2()
    }
}

/// The reference worker's hook for `backend`: uploads each reference it hands out on the
/// worker's thread (None: the backend uploads only on the renderer's thread).
pub(super) fn prepare_hook<B: GpuBackend>(backend: &B) -> Option<ReferencePrepare> {
    let upload = backend.uploader()?;
    Some(Arc::new(move |r: &Reference| -> Option<ReferenceAttachment> {
        match gpu_reference(r, &*upload) {
            Ok(g) => Some(Arc::new(g) as ReferenceAttachment),
            Err(e) => {
                eprintln!("almondbread: reference buffer: {e}");
                None
            }
        }
    }))
}

/// The GPU form `prepare_hook` attached to `r`, if any.
pub(super) fn attached<B: GpuBackend>(r: &Reference) -> Option<GpuReference<B::Buffer>> {
    let g = r
        .prepared
        .as_ref()?
        .downcast_ref::<GpuReference<SentBuffer<B::Buffer>>>()?;
    Some(GpuReference {
        points: g.points.get().clone(),
        extended: g.extended.as_ref().map(|b| b.get().clone()),
        bla_reach_log2: g.bla_reach_log2,
    })
}

/// A table build on a thread of its own for the reference with `serial`. Dropping it (a
/// newer reference replaced that one) cancels the build: it stops before its next level or
/// stage instead of finishing a table nobody takes (C2 of the 2026-10 review).
pub(super) struct TableBuild<T> {
    pub serial: u64,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<Option<T>>>,
}

impl<T: Send + 'static> TableBuild<T> {
    /// Starts `build` (it polls the flag it gets; None: cancelled) on a thread named `name`.
    pub fn spawn(
        name: &str,
        serial: u64,
        build: impl FnOnce(&AtomicBool) -> Option<T> + Send + 'static,
    ) -> std::io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let handle = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || build(&flag))?;
        Ok(Self {
            serial,
            cancel,
            handle: Some(handle),
        })
    }

    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// The table (waits for it). None: cancelled, or the build panicked.
    pub fn join(mut self) -> Option<T> {
        match self.handle.take()?.join() {
            Ok(t) => t,
            Err(_) => {
                eprintln!("almondbread: table build failed");
                None
            }
        }
    }

    /// The flag dropping this build sets (tests).
    #[cfg(test)]
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }
}

impl<T> Drop for TableBuild<T> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use almondbread_core::bla::BlaTable;
    use almondbread_core::formula::Formula;
    use std::time::{Duration, Instant};

    /// A dropped build stops at its next level instead of running to the end.
    #[test]
    fn dropped_build_is_cancelled() {
        let started = Arc::new(AtomicBool::new(false));
        let s = started.clone();
        let finished_at = Arc::new(std::sync::Mutex::new(None));
        let f = finished_at.clone();
        let build = TableBuild::spawn("test-build", 1, move |cancel| {
            s.store(true, Ordering::Relaxed);
            // A build of many levels: each waits 10 ms, 1000 of them unless cancelled.
            for _ in 0..1000 {
                if cancel.load(Ordering::Relaxed) {
                    *f.lock().unwrap() = Some(Instant::now());
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Some(())
        })
        .expect("thread");
        while !started.load(Ordering::Relaxed) {
            std::thread::yield_now();
        }
        let flag = build.cancel_flag();
        let dropped = Instant::now();
        drop(build);
        assert!(flag.load(Ordering::Relaxed));
        while finished_at.lock().unwrap().is_none() {
            assert!(dropped.elapsed() < Duration::from_secs(2), "build kept running");
            std::thread::sleep(Duration::from_millis(1));
        }
        let stop = finished_at.lock().unwrap().unwrap();
        assert!(stop.duration_since(dropped) < Duration::from_millis(100));
    }

    /// The real table builds return None once cancelled, and the same table otherwise.
    #[test]
    fn table_builds_honour_cancel() {
        let r = Reference::compute_f64(
            &almondbread_core::viewport::Viewport::new(-0.75, 0.1, 1e-10),
            100_000,
            65536.0,
        );
        let yes = AtomicBool::new(true);
        let no = AtomicBool::new(false);
        assert!(BlaTable::build_cancellable(&r.points, BLA_EPSILON, Formula::Mandelbrot, &yes).is_none());
        let t = BlaTable::build_cancellable(&r.points, BLA_EPSILON, Formula::Mandelbrot, &no).expect("table");
        assert_eq!(t.entries, BlaTable::build(&r.points, BLA_EPSILON).entries);
        let p = almondbread_core::la::LaBuild::default();
        assert!(almondbread_core::la::LaTable::build_plain_cancellable(&r.points, BLA_EPSILON, &p, &yes).is_none());
    }
}
