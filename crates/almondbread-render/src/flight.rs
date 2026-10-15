//! Flight scripts into the renderer (kb: synthesis/flight-benchmark): [`FlightTarget`] for
//! the headless [`Renderer`] and for the app's [`Host`] (whose `input` also records the input
//! the next shown frame answers, for input-to-photon latency).

use almondbread_core::flight::FlightTarget;
use almondbread_core::input::{InputEvent, Navigator};
use almondbread_core::settings::RenderSettings;
use almondbread_core::viewport::Viewport;

use crate::backend::GpuBackend;
use crate::host::Host;
use crate::scheduler::Renderer;

impl<B: GpuBackend> FlightTarget for Renderer<B> {
    fn input(&mut self, event: InputEvent, now: f64) {
        self.handle_input(event, now);
    }
    fn navigator(&mut self) -> &mut Navigator {
        &mut self.navigator
    }
    fn viewport(&mut self) -> &mut Viewport {
        &mut self.viewport
    }
    fn settings(&mut self) -> &mut RenderSettings {
        &mut self.settings
    }
}

impl<B: GpuBackend + Clone> FlightTarget for Host<B> {
    fn input(&mut self, event: InputEvent, now: f64) {
        Host::input(self, event, now);
    }
    fn navigator(&mut self) -> &mut Navigator {
        &mut self.renderer.navigator
    }
    fn viewport(&mut self) -> &mut Viewport {
        &mut self.renderer.viewport
    }
    fn settings(&mut self) -> &mut RenderSettings {
        &mut self.renderer.settings
    }
}
