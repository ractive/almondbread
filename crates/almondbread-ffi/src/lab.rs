//! The parameter lab for the UI (F10, kb: plans/f10-parameter-lab, `almondbread_core::lab`):
//! entering and leaving it, its three knobs (z₀, c, p), the plane the main view shows, Random
//! and Reset. Rust owns the state (the panels' drags change the knobs too); the UI shows and
//! sends them and reads them back with `lab()`. While the lab is on, `capabilities()` is the
//! lab's: what it lacks is hidden. After each call, resume the display link.

use almondbread_core::lab::{self, LabValues, Plane};
use almondbread_render::metal::media_time;

use crate::explorer::{Explorer, FormulaCapabilities};

/// The three knobs.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct LabKnobs {
    pub z0_re: f64,
    pub z0_im: f64,
    pub c_re: f64,
    pub c_im: f64,
    pub p_re: f64,
    pub p_im: f64,
}

impl From<LabValues> for LabKnobs {
    fn from(v: LabValues) -> Self {
        Self {
            z0_re: v.z0.0,
            z0_im: v.z0.1,
            c_re: v.c.0,
            c_im: v.c.1,
            p_re: v.p.0,
            p_im: v.p.1,
        }
    }
}

impl From<LabKnobs> for LabValues {
    fn from(k: LabKnobs) -> Self {
        LabValues {
            z0: (k.z0_re, k.z0_im),
            c: (k.c_re, k.c_im),
            p: (k.p_re, k.p_im),
        }
        .sanitized()
    }
}

/// The lab's state.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LabInfo {
    /// The lab is the main view.
    pub on: bool,
    /// The plane the main view shows: "c", "z0" or "p".
    pub plane: String,
    pub knobs: LabKnobs,
    /// Zooming ran into the precision limit in the last few seconds (the view says so too).
    pub at_limit: bool,
    /// GPU milliseconds of the last field of the main view (the HUD, tests).
    pub field_ms: f64,
}

/// A knob's slider range: `[re_min, re_max, im_min, im_max]` (values are clamped to it).
#[uniffi::export]
pub fn lab_range(plane: String) -> Vec<f64> {
    let p = Plane::parse(&plane).unwrap_or_default();
    let ((a, b), (c, d)) = p.range();
    vec![a, b, c, d]
}

/// Reset's knobs: z₀ = 0, c = −0.75, p = 2.
#[uniffi::export]
pub fn lab_default_knobs() -> LabKnobs {
    LabValues::default().into()
}

impl FormulaCapabilities {
    /// The lab's capabilities: shallow direct iteration with a distance estimate (lighting,
    /// outlines) and its own orbit overlay; no perturbation, no interior proofs or colourings
    /// by period, no seek, π meter, rays, content (places, tours, journeys), Julia panel or
    /// Buddhabrot.
    pub(crate) fn lab() -> Self {
        Self {
            formula: "lab".into(),
            perturbation: false,
            rebasing: "none".into(),
            bla: "none".into(),
            la: false,
            sa_seeds: false,
            interior_derivative: false,
            interior_probe: false,
            closed_form_components: false,
            de: true,
            seek: false,
            atom_domain: false,
            pi_meter: false,
            external_rays: false,
            orbit_overlay: true,
            content: false,
            julia_panel: false,
            buddhabrot: false,
        }
    }
}

#[uniffi::export]
impl Explorer {
    /// Enters the lab with `knobs`, showing `plane` ("c", "z0", "p"): its home view; the view
    /// and the travel journal from before come back with `leave_lab`. A tour, the autopilot, a
    /// journey and the π meter stop. False when it is on already.
    pub fn enter_lab(&self, plane: String, knobs: LabKnobs) -> bool {
        let plane = Plane::parse(&plane).unwrap_or_default();
        self.with_host(|h| {
            let ok = h.enter_lab(knobs.into(), plane, media_time());
            if ok {
                h.overlay.pi.enabled = false;
            }
            ok
        })
    }

    /// Leaves the lab for the view it was entered from. False when it was off.
    pub fn leave_lab(&self) -> bool {
        self.with_host(|h| h.leave_lab().is_some())
    }

    /// The lab's state (knobs as Rust has them: the panels' drags change them too).
    pub fn lab(&self) -> LabInfo {
        self.with_host(|h| {
            let now = media_time();
            let (values, plane) = h.lab_state().unwrap_or_default();
            let l = h.renderer.lab.as_ref();
            LabInfo {
                on: l.is_some(),
                plane: plane.name().into(),
                knobs: values.into(),
                at_limit: l
                    .and_then(|l| l.at_limit)
                    .is_some_and(|t| now - t < almondbread_render::overlay::LAB_LIMIT_NOTE),
                field_ms: l.map_or(0.0, |l| l.last_field_ms),
            }
        })
    }

    /// Sets the knobs (clamped into their ranges). False when the lab is off.
    pub fn set_lab_knobs(&self, knobs: LabKnobs) -> bool {
        self.with_host(|h| h.set_lab_values(knobs.into()))
    }

    /// Shows `plane` ("c", "z0", "p") in the main view (its last view there, else its home).
    pub fn set_lab_plane(&self, plane: String) -> bool {
        let Some(plane) = Plane::parse(&plane) else {
            return false;
        };
        self.with_host(|h| h.set_lab_plane(plane))
    }

    /// Random knobs in interesting ranges (`almondbread_core::lab::random`): returns them.
    pub fn lab_random(&self, seed: u64) -> LabKnobs {
        let v = lab::random(seed);
        self.with_host(|h| h.set_lab_values(v));
        v.into()
    }
}
