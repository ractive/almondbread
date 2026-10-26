//! Extreme offsets in the history reprojection (kb: techniques/hang-prevention "Panic log and
//! the offset overflow class"): a jump between two deep views of about the same scale put an
//! infinite (or, turned, NaN) history origin into the compose pass.

use almondbread_core::viewport::{Center, Scale, Viewport};

use super::{HistoryView, Renderer};
use crate::gpu_types::{ColorParams, ComposeParams};
use crate::metal::MetalBackend;

fn history(view: Viewport, scale: Scale) -> HistoryView {
    HistoryView {
        viewport: view,
        scale,
        width: 800,
        height: 500,
        colors: (ColorParams::default(), String::new()),
        offset: 0.0,
    }
}

#[test]
fn extreme_history_reprojection_far_past_f64() {
    let deep = Scale::from_log2(-337.0 * std::f64::consts::LOG2_10);
    let here = Center::new(-0.75, 0.1).with_min_precision(1400);
    let far = Center::new(0.3, -0.5).with_min_precision(1400);
    for angle in [0.0, 0.7] {
        // Deep view, history of a deep view elsewhere: no history, nothing non-finite.
        let h = history(Viewport::with_center(here.clone(), deep).with_angle(angle), deep);
        let view = Viewport::with_center(far.clone(), deep).with_angle(angle);
        let mut q = ComposeParams::default();
        Renderer::<MetalBackend>::reproject(&h, &view, deep, (800, 500), &mut q);
        assert_eq!(q.hist_decay, 0.0, "angle {angle}: no history");
        assert!(q.hist_origin.iter().all(|v| v.is_finite()));
        // Shallow view, history 1e-337 off its centre: the history is used, finite.
        let s = Scale::from_f64(1e-2);
        let near = here.offset_scaled(3.0, 0.0, deep);
        let h = history(Viewport::with_center(here.clone(), s).with_angle(angle), s);
        let view = Viewport::with_center(near, s).with_angle(angle);
        let mut q = ComposeParams::default();
        Renderer::<MetalBackend>::reproject(&h, &view, s, (800, 500), &mut q);
        assert!(q.hist_decay > 0.0, "angle {angle}: history used");
        assert!(q.hist_origin.iter().all(|v| v.is_finite()), "{:?}", q.hist_origin);
    }
}
