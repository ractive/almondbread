//! The compass for the UI (kb: decisions/controls, "Compass"): its setting and the press, drag
//! and release the platform forwards. Hit test and motion live in Rust; the platform only
//! routes a press that the compass took (and its drag and release) here instead of to the
//! canvas. Coordinates: view points from the view centre, y up. After each call, resume the
//! display link.

use almondbread_render::metal::media_time;

use crate::explorer::Explorer;

#[uniffi::export]
impl Explorer {
    /// "Always show compass" (otherwise it shows while the view is turned and fades out a
    /// few seconds after it is back at north); `inset_top`: extra top margin in points keeping
    /// platform buttons clear (iPad's top-right buttons).
    pub fn set_compass(&self, always: bool, inset_top: f64) {
        self.with_host(|h| {
            h.overlay.compass.visibility.always = always;
            h.overlay.compass.inset_top = if inset_top.is_finite() {
                inset_top.clamp(0.0, 2000.0)
            } else {
                0.0
            };
        });
    }

    /// "Autopilot Target Marker" (kb: decisions/controls "Target marker"): a ring where the
    /// autopilot or a journey leg is heading, its label and the pointer wish's line.
    pub fn set_target_marker(&self, on: bool) {
        self.with_host(|h| h.overlay.marker.enabled = on);
    }

    /// The target marker's wish line is on the screen (as of the last frame; UI tests check
    /// that a lifted finger leaves none behind).
    pub fn wish_line_shown(&self) -> bool {
        self.with_host(|h| h.overlay.marker.wish_shown())
    }

    /// A press (mouse down, touch down) at (x, y): true if the visible compass took it. Then
    /// send its moves to `compass_drag` and its end to `compass_release`, and do not treat it
    /// (or its double click) as a canvas click.
    pub fn compass_press(&self, x: f64, y: f64) -> bool {
        x.is_finite() && y.is_finite() && self.with_host(|h| h.compass_press(x, y, media_time()))
    }

    /// The pointer of a compass press moved to (x, y): the needle follows it, turning the view.
    pub fn compass_drag(&self, x: f64, y: f64) {
        if x.is_finite() && y.is_finite() {
            self.with_host(|h| h.compass_drag(x, y, media_time()));
        }
    }

    /// Where the compass is (as last laid out, view points from the centre, y up) and whether
    /// it shows: `[x, y]`, or empty while hidden (window tests aim their clicks with it).
    pub fn compass_center(&self) -> Vec<f64> {
        self.with_host(|h| {
            let now = media_time();
            h.overlay
                .compass
                .center()
                .filter(|_| h.overlay.compass.shown(now))
                .map_or_else(Vec::new, |(x, y)| vec![x, y])
        })
    }

    /// The compass press ended: a click (no drag) eases the view back to north.
    pub fn compass_release(&self) {
        self.with_host(|h| h.compass_release(media_time()));
    }
}
