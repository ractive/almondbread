//! M6: Imagina-style LA tables and the AT on the GPU (kb: techniques/linear-approximation-imagina
//! "As built"). The tables are built by `almondbread_core::la` on a thread per reference; the
//! iterate kernel reads them through function constants `kLA` (8) and `kAT` (9) and buffers
//! 25..29, so pipelines without them are unchanged.

use std::sync::Arc;
use std::thread::JoinHandle;

use almondbread_core::la::{LaBuild, LaEntryX, LaParams, LaTable};
use almondbread_core::reference::Reference;

use crate::backend::{ComputeEncoder, GpuBackend, GpuError};

/// When the iterate kernel uses the LA table instead of BLA (`ALMONDBREAD_LA=0|1|auto`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaMode {
    /// Never (no table is built).
    Off,
    /// Whenever a table exists (A/B runs, the skip diff).
    On,
    /// Where the table's forecast beats BLA's ([`prefer_la`]).
    Auto,
}

impl LaMode {
    pub fn parse(v: &str) -> Option<Self> {
        match v {
            "0" | "off" => Some(Self::Off),
            "1" | "on" => Some(Self::On),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// An LA table on the GPU, for the reference with serial `serial`.
pub(crate) struct LaGpu<B: GpuBackend> {
    pub serial: u64,
    bounds: B::Buffer,
    entries: B::Buffer,
    next: B::Buffer,
    /// Kernel parameters with and without the AT.
    params: LaParams,
    params_no_at: LaParams,
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
            params_no_at: self.params_no_at,
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
            params: table.gpu_params(true),
            params_no_at: table.gpu_params(false),
            table,
        })
    }

    /// Whether the table has an AT.
    pub fn has_at(&self) -> bool {
        self.params.period != 0
    }

    /// Binds the table for an iterate chunk of the `deep` kernel (`dummy` in the other path's
    /// entry slot).
    pub fn bind(&self, enc: &mut B::Encoder, deep: bool, at: bool, dummy: &B::Buffer) {
        enc.set_buffer(25, &self.bounds, 0);
        enc.set_buffer(26, if deep { &self.entries } else { dummy }, 0);
        enc.set_buffer(27, if deep { dummy } else { &self.entries }, 0);
        enc.set_buffer(28, &self.next, 0);
        let p = if at { &self.params } else { &self.params_no_at };
        enc.set_bytes(29, bytemuck::bytes_of(p));
    }
}

/// Builds the LA table of `r` (validity `epsilon`, as BLA's), from the floatexp points for the
/// deep kernel and from the float32 points the plain kernel iterates against otherwise.
/// Stage parameters can be overridden for studies: `ALMONDBREAD_LA_F` (synthetic coarsening
/// factor), `ALMONDBREAD_LA_SMAX` (longest natural step before a synthetic split).
pub(crate) fn build_table(r: &Reference, epsilon: f64) -> Box<dyn FnOnce() -> Arc<LaTable> + Send> {
    let mut b = LaBuild::default();
    if let Some(f) = std::env::var("ALMONDBREAD_LA_F").ok().and_then(|v| v.parse().ok()) {
        b.synthetic_factor = f;
    }
    if let Some(m) = std::env::var("ALMONDBREAD_LA_SMAX").ok().and_then(|v| v.parse().ok()) {
        b.synthetic_max = m;
    }
    if r.deep_kernel {
        let points = r.extended.clone();
        Box::new(move || Arc::new(LaTable::build_with(&points, epsilon, &b)))
    } else {
        let points = r.points.clone();
        Box::new(move || Arc::new(LaTable::build_plain_with(&points, epsilon, &b)))
    }
}

/// A table build on its thread, for the reference with this serial.
pub(crate) type LaBuildJob = (u64, JoinHandle<Arc<LaTable>>);

#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use super::LaMode;
    use crate::metal::MetalBackend;
    use crate::scheduler::{Renderer, SaMode, SchedulerConfig};
    use almondbread_core::IterationField;
    use almondbread_core::locations::location_named;

    /// (field, steps, LA used) of an offscreen render of `name` at `w` x `h` (the location's
    /// pixel size at 1600 wide), local SA off, with the given LA mode, AT and chunk target.
    fn render(name: &str, w: usize, h: usize, la: LaMode, at: bool, chunk: f64) -> (IterationField, u64, bool) {
        let mut config = SchedulerConfig {
            la,
            la_at: at,
            chunk_target: chunk,
            ..SchedulerConfig::default()
        };
        config.local_sa.mode = SaMode::Off;
        let mut r = Renderer::with_config(MetalBackend::new().expect("metal"), config).expect("renderer");
        let loc = location_named(name).expect("location");
        r.settings = almondbread_core::RenderSettings::plain();
        r.settings.auto_iterations = false;
        r.settings.manual_iterations = loc.max_iterations();
        r.settings.supersampling = almondbread_core::Supersampling::Off;
        r.viewport = loc.viewport_for_width(1600);
        let out = r.render_offscreen(w, h, true).expect("render");
        let field = IterationField::from_texels(w, h, out.iterations.as_deref().expect("texels"));
        (field, out.steps_done, out.la_used)
    }

    /// Results do not depend on how chunks cut the orbit (shown frame = fresh render), in the
    /// deep path with the AT (pixels resume inside it), without it, and in the plain path.
    #[test]
    fn la_chunk_independent() {
        for (name, at) in [("needle-1e305", true), ("needle-1e94", false), ("deep-5e-14", false)] {
            let (a, _, used) = render(name, 320, 200, LaMode::On, at, 0.0002);
            let (b, _, _) = render(name, 320, 200, LaMode::On, at, 0.05);
            assert!(used, "{name}");
            assert_eq!(a.bitwise_differences(&b), 0, "{name}");
        }
    }

    /// LA with its AT changes (almost) no pixel's class against BLA at a needle minibrot and
    /// takes fewer steps; auto picks it there.
    #[test]
    fn la_agrees_with_bla() {
        let (off, off_steps, _) = render("needle-1e305", 400, 250, LaMode::Off, true, 0.016);
        let (on, on_steps, used) = render("needle-1e305", 400, 250, LaMode::Auto, true, 0.016);
        assert!(used);
        assert!(on_steps * 3 < off_steps * 2, "{on_steps} vs {off_steps}");
        let class = |f: &IterationField, i: usize| f.n[i] >= 0.0;
        let differ = (0..off.n.len()).filter(|&i| class(&off, i) != class(&on, i)).count();
        assert!(differ * 2000 <= off.n.len(), "{differ} pixels changed class");
    }
}
