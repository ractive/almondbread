//! M6: Imagina-style LA tables on the GPU (kb: techniques/linear-approximation-imagina
//! "As built"). The tables are built by `almondbread_core::la` on a thread per reference; the
//! iterate kernel reads them through function constant `kLA` (8) and buffers 25..29, so
//! pipelines without them are unchanged.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use almondbread_core::la::{LaBuild, LaEntryX, LaParams, LaTable};
use almondbread_core::reference::Reference;

use crate::backend::{ComputeEncoder, GpuBackend, GpuError};

/// An LA table on the GPU, for the reference with serial `serial`.
pub(crate) struct LaGpu<B: GpuBackend> {
    pub serial: u64,
    bounds: B::Buffer,
    entries: B::Buffer,
    next: B::Buffer,
    /// Kernel parameters.
    params: LaParams,
    pub table: Arc<LaTable>,
}

impl<B: GpuBackend> Clone for LaGpu<B> {
    fn clone(&self) -> Self {
        Self {
            serial: self.serial,
            bounds: self.bounds.clone(),
            entries: self.entries.clone(),
            next: self.next.clone(),
            params: self.params,
            table: self.table.clone(),
        }
    }
}

impl<B: GpuBackend> LaGpu<B> {
    /// Uploads `table`: extended-range entries for the deep kernel, float32 ones for the
    /// plain kernel (both are bound; the kernel reads the one of its path).
    pub fn upload(backend: &B, serial: u64, table: Arc<LaTable>, deep: bool) -> Result<Self, GpuError> {
        let bounds = backend.create_buffer_with_data(bytemuck::cast_slice(&table.bounds))?;
        let entries = if deep {
            let mut e = table.entries.clone();
            if e.is_empty() {
                e.push(LaEntryX::default());
            }
            backend.create_buffer_with_data(bytemuck::cast_slice(&e))?
        } else {
            let mut e = table.plain_entries();
            if e.is_empty() {
                e.push(Default::default());
            }
            backend.create_buffer_with_data(bytemuck::cast_slice(&e))?
        };
        let next = backend.create_buffer_with_data(bytemuck::cast_slice(&table.next_j))?;
        Ok(Self {
            serial,
            bounds,
            entries,
            next,
            params: table.gpu_params(),
            table,
        })
    }

    /// Binds the table for an iterate chunk of the `deep` kernel (`dummy` in the other path's
    /// entry slot).
    pub fn bind(&self, enc: &mut B::Encoder, deep: bool, dummy: &B::Buffer) {
        enc.set_buffer(25, &self.bounds, 0);
        enc.set_buffer(26, if deep { &self.entries } else { dummy }, 0);
        enc.set_buffer(27, if deep { dummy } else { &self.entries }, 0);
        enc.set_buffer(28, &self.next, 0);
        enc.set_bytes(29, bytemuck::bytes_of(&self.params));
    }
}

/// Builds the LA table of `r` (validity `epsilon`, as BLA's), from the floatexp points for the
/// deep kernel and from the float32 points the plain kernel iterates against otherwise.
/// The build polls the flag it gets (None: cancelled); the orbit is shared, not copied.
pub(crate) type LaBuildFn = Box<dyn FnOnce(&AtomicBool) -> Option<Arc<LaTable>> + Send>;

pub(crate) fn build_table(r: &Reference, epsilon: f64) -> LaBuildFn {
    let b = LaBuild::default();
    if r.deep_kernel {
        let points = r.extended.clone();
        Box::new(move |cancel| LaTable::build_cancellable(&points, epsilon, &b, cancel).map(Arc::new))
    } else {
        let points = r.points.clone();
        Box::new(move |cancel| LaTable::build_plain_cancellable(&points, epsilon, &b, cancel).map(Arc::new))
    }
}

#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use crate::metal::MetalBackend;
    use crate::scheduler::{Renderer, SchedulerConfig};
    use almondbread_core::IterationField;
    use almondbread_core::locations::location_named;

    /// (field, LA used) of an offscreen render of `name` at `w` x `h` (the location's pixel size
    /// at 1600 wide) with the given chunk target.
    fn render(name: &str, w: usize, h: usize, chunk: f64) -> (IterationField, bool) {
        let config = SchedulerConfig {
            chunk_target: chunk,
            ..SchedulerConfig::default()
        };
        let mut r = Renderer::with_config(MetalBackend::new().expect("metal"), config).expect("renderer");
        let loc = location_named(name).expect("location");
        r.settings = almondbread_core::RenderSettings::plain();
        r.settings.auto_iterations = false;
        r.settings.manual_iterations = loc.max_iterations();
        r.settings.supersampling = almondbread_core::Supersampling::Off;
        r.viewport = loc.viewport_for_width(1600);
        let out = r.render_offscreen(w, h, true).expect("render");
        let field = IterationField::from_texels(w, h, out.iterations.as_deref().expect("texels"));
        (field, out.la_used)
    }

    /// Results do not depend on how chunks cut the orbit (shown frame = fresh render), at a
    /// needle minibrot where the nucleus reference takes the LA table.
    #[test]
    fn la_chunk_independent() {
        let (a, used) = render("needle-1e305", 320, 200, 0.0002);
        let (b, _) = render("needle-1e305", 320, 200, 0.05);
        assert!(used);
        assert_eq!(a.bitwise_differences(&b), 0);
    }
}
