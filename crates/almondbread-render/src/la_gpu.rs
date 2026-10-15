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
