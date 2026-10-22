//! The renderer under a device's constraints (kb: platform/ios-device): a memory budget and
//! memory warnings, the thermal state, and suspension while the app is in the background.
//! The policies are in [`crate::memory`]; this applies them to the scheduler's state.

use super::{GpuBackend, Renderer, SchedulerConfig};
use crate::memory::{MemoryBudget, MemoryReport, Thermal};

/// The scheduler settings the thermal state scales, as they were at nominal.
#[derive(Clone, Debug)]
pub(super) struct ThermalBase {
    chunk_target: f64,
    interactive_chunk_target: f64,
    aa_chunk_target: f64,
    aa_budget: f64,
    extension_budget: f64,
    further: bool,
}

impl ThermalBase {
    fn of(c: &SchedulerConfig) -> Self {
        Self {
            chunk_target: c.chunk_target,
            interactive_chunk_target: c.interactive_chunk_target,
            aa_chunk_target: c.aa_chunk_target,
            aa_budget: c.aa_budget,
            extension_budget: c.extension_budget,
            further: c.further,
        }
    }

    fn apply(&self, c: &mut SchedulerConfig, t: Thermal) {
        let s = t.budget_scale();
        let v = t.view_budget_scale();
        c.chunk_target = self.chunk_target * s;
        c.interactive_chunk_target = self.interactive_chunk_target * s;
        c.aa_chunk_target = self.aa_chunk_target * s;
        c.aa_budget = self.aa_budget * v;
        // Extension passes are never cut short once started; a smaller estimate only starts
        // fewer of them.
        c.extension_budget = self.extension_budget * v.max(0.2);
        c.further = self.further && t.allows_further();
    }
}

impl<B: GpuBackend> Renderer<B> {
    /// What the renderer holds now, by kind.
    pub fn memory_report(&self) -> MemoryReport {
        let reference_bytes =
            |r: &almondbread_core::reference::Reference| (r.points.len() * 8 + r.extended.len() * 16 + r.split.len() * 16) as u64;
        let mut tables = 0u64;
        if self.bla.is_some() {
            tables += self.bla_info.2 as u64;
        }
        if self.la.is_some() {
            tables += self.la_info.2 as u64;
        }
        let caches = self.spare_reference.as_ref().map_or(0, reference_bytes)
            + self.period_refs.as_ref().map_or(0, |(_, v)| {
                (v.len() * std::mem::size_of::<almondbread_core::math::DoubleExpComplex>()) as u64
            });
        MemoryReport {
            gpu: self.backend.allocated_bytes(),
            reference: self.reference.as_ref().map_or(0, reference_bytes),
            tables,
            caches,
        }
    }

    /// The memory budget in force.
    pub fn memory_budget(&self) -> MemoryBudget {
        self.memory_budget
    }

    /// A new reading of the memory still available to the app (`os_proc_available_memory()`;
    /// 0: no limit known). `pressure`: a memory warning came with it (drops caches first, and
    /// the budget only goes down). Returns the budget in force afterwards.
    pub fn set_memory_available(&mut self, available: u64, pressure: bool) -> MemoryBudget {
        if pressure {
            self.release_caches();
        }
        let next = MemoryBudget::from_available(self.memory_report().total(), available);
        self.memory_budget = self.memory_budget.update(next, pressure);
        self.memory_budget
    }

    /// Replaces the budget outright (headless emulation of a device: `fly --memory-limit`).
    pub fn set_memory_budget(&mut self, budget: MemoryBudget) {
        self.memory_budget = budget;
    }

    /// Drops what can be rebuilt (a memory warning): the spare reference, cached cycle
    /// searches, tables of older references, and the preview allocation (made again by the
    /// next gesture). The current view's state stays.
    pub fn release_caches(&mut self) {
        self.spare_reference = None;
        self.period_refs = None;
        self.period_cache = None;
        self.component_cache = None;
        if self.bla.as_ref().is_some_and(|t| t.serial != self.reference_serial) {
            self.bla = None;
        }
        if self.la.as_ref().is_some_and(|t| t.serial != self.reference_serial) {
            self.la = None;
        }
        if !self.job.as_ref().is_some_and(|j| j.interactive) {
            self.preview_alloc = None;
        }
    }

    /// The iteration-limit ceiling the memory budget allows (usize::MAX: none).
    pub(super) fn memory_iteration_cap(&self) -> usize {
        self.memory_budget
            .max_iterations()
            .unwrap_or(usize::MAX)
            .min(almondbread_core::settings::MAX_ITERATION_LIMIT)
    }

    /// The iteration cap of automatic limits, extension and "compute further" passes, and
    /// longer references: the settings' cap within the memory budget (and within
    /// `MAX_ITERATION_LIMIT`, the float32-exact escape count).
    pub(super) fn iteration_cap(&self) -> usize {
        self.settings.max_iterations_cap.min(self.memory_iteration_cap())
    }

    /// The thermal state in force.
    pub fn thermal(&self) -> Thermal {
        self.thermal
    }

    /// Adapts the GPU budgets to the device's thermal state (see [`Thermal`]).
    pub fn set_thermal(&mut self, t: Thermal) {
        if t == self.thermal {
            return;
        }
        let base = self
            .thermal_base
            .get_or_insert_with(|| ThermalBase::of(&self.config))
            .clone();
        base.apply(&mut self.config, t);
        if t <= Thermal::Fair {
            self.thermal_base = None;
        }
        self.thermal = t;
    }

    /// Refinement of the current (non-gesture) job stops here: critical thermal state, and its
    /// first full-resolution frame is on screen.
    pub(super) fn refinement_paused(&self) -> bool {
        self.thermal.pauses_refinement()
            && self
                .job
                .as_ref()
                .is_some_and(|j| !j.interactive && j.complete_presented)
    }

    /// The app goes to the background: wait for every command buffer in flight (iOS refuses
    /// GPU work from the background; work committed before completes) and submit nothing more
    /// until [`Self::resume`]. The job and history stay, so the view continues where it was.
    pub fn suspend(&mut self) {
        if self.suspended {
            return;
        }
        self.drain_in_flight();
        self.wait_frames();
        if let Some((sub, _)) = &self.pending_survey {
            super::Submission::wait(sub);
        }
        self.suspended = true;
    }

    /// Back in the foreground.
    pub fn resume(&mut self) {
        self.suspended = false;
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended
    }
}

#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use crate::backend::{GpuBackend, Storage, TextureFormat};
    use crate::host::Host;
    use crate::memory::{MemoryBudget, Thermal};
    use crate::metal::MetalBackend;
    use crate::scheduler::Renderer;
    use almondbread_core::input::InputEvent;

    fn renderer(w: usize, h: usize) -> Renderer<MetalBackend> {
        let mut r = Renderer::new(MetalBackend::new().expect("metal")).expect("renderer");
        r.handle_input(
            InputEvent::Resize {
                width: w as f64,
                height: h as f64,
            },
            0.0,
        );
        r
    }

    /// Refreshes at 60 Hz from `t0` for `seconds` into a shared target; returns frames presented
    /// and the time after.
    fn run(r: &mut Renderer<MetalBackend>, t0: f64, seconds: f64, w: usize, h: usize) -> (usize, f64) {
        let target = r
            .backend()
            .create_texture(w, h, TextureFormat::Bgra8Unorm, Storage::Shared)
            .expect("target");
        let mut presented = 0;
        let refreshes = (seconds * 60.0).ceil() as usize;
        for k in 0..refreshes {
            let t = t0 + k as f64 / 60.0;
            let o = r.render_frame(t, w, h, 1.0, &mut |_| Some((target.clone(), None)));
            presented += usize::from(o.presented);
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
        (presented, t0 + refreshes as f64 / 60.0)
    }

    #[test]
    fn texel_cap_shrinks_the_drawable_and_restores_it() {
        let mut host = Host::new(renderer(0, 0));
        assert_eq!(host.resize(1000.0, 800.0, 2.0, 0.0), (2000, 1600));
        let (w, h) = host.set_max_texels(Some(1_000_000), 0.0).expect("smaller");
        assert!(w * h <= 1_000_000 && w * h > 900_000, "{w}x{h}");
        assert!((w as f64 / h as f64 - 1.25).abs() < 0.01);
        // A later layout change keeps the cap.
        let (w2, h2) = host.resize(800.0, 1000.0, 2.0, 0.0);
        assert!(w2 * h2 <= 1_000_000);
        assert_eq!(host.set_max_texels(None, 0.0), Some((1600, 2000)));
    }

    #[test]
    fn budget_caps_the_iteration_limit() {
        let mut r = renderer(320, 200);
        r.settings.auto_iterations = false;
        r.settings.manual_iterations = 10_000_000;
        assert_eq!(r.max_iterations(), 10_000_000);
        r.set_memory_budget(MemoryBudget {
            ceiling: Some(800 << 20),
        });
        let cap = r.max_iterations();
        assert!((200_000..1_000_000).contains(&cap), "{cap}");
        r.set_memory_budget(MemoryBudget::NONE);
        assert_eq!(r.max_iterations(), 10_000_000);
        // No limit known (the simulator): nothing changes.
        assert_eq!(r.set_memory_available(0, false), MemoryBudget::NONE);
        let b = r.set_memory_available(1 << 30, true);
        assert!(b.ceiling.is_some_and(|c| c >= 1 << 30));
    }

    #[test]
    fn thermal_scales_budgets_and_restores_them() {
        let mut r = renderer(320, 200);
        let base = r.config.clone();
        r.set_thermal(Thermal::Serious);
        assert_eq!(r.config.chunk_target, base.chunk_target * 0.5);
        assert!(!r.config.further);
        r.set_thermal(Thermal::Critical);
        assert!(r.config.aa_budget < base.aa_budget * 0.5);
        r.set_thermal(Thermal::Nominal);
        assert_eq!(r.config.chunk_target, base.chunk_target);
        assert_eq!(r.config.aa_budget, base.aa_budget);
        assert_eq!(r.config.further, base.further);
    }

    #[test]
    fn suspended_renderer_submits_nothing_and_resumes() {
        let (w, h) = (320, 200);
        let mut r = renderer(w, h);
        let (n, t) = run(&mut r, 0.0, 0.1, w, h);
        assert!(n > 0);
        r.suspend();
        assert!(r.is_suspended());
        let committed = r.frames_committed();
        let (n, t) = run(&mut r, t, 0.2, w, h);
        assert_eq!(n, 0);
        assert_eq!(r.frames_committed(), committed);
        r.resume();
        let (n, _) = run(&mut r, t, 3.0, w, h);
        assert!(n > 0);
        assert!(r.is_settled());
    }

    #[test]
    fn critical_thermal_stops_after_the_first_full_frame() {
        let (w, h) = (320, 200);
        let mut r = renderer(w, h);
        r.settings.supersampling = almondbread_core::Supersampling::from_factor(3).expect("ss");
        r.set_thermal(Thermal::Critical);
        let (_, t) = run(&mut r, 0.0, 1.0, w, h);
        assert!(r.refinement_paused());
        let committed = r.frames_committed();
        run(&mut r, t, 0.5, w, h);
        assert_eq!(r.frames_committed(), committed, "no frames while paused");
        r.set_thermal(Thermal::Nominal);
        assert!(!r.refinement_paused());
    }
}
