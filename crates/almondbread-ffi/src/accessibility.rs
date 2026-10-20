//! Accessibility of the controls Rust draws (kb: plans/a11y-canvas-controls): the platform views
//! ask for these only when an accessibility client (VoiceOver, Switch Control, the Mac's
//! accessibility API, UI tests) queries them, at most every ~0.1 s; nothing per frame. The
//! words and rects come from `almondbread_render::accessibility`; the platform maps each id to
//! its element's traits and action (the same calls a click or a tap makes).

use almondbread_render::metal::media_time;

use crate::explorer::Explorer;

/// What kind of control an element is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AccessibleRole {
    /// Does something when activated (the platform routes it by `id`).
    Button,
    /// A picture with a value (a mini-map, the Julia panel).
    Image,
    /// Text that changes (the π meter, the cosmic size scale).
    Text,
}

/// A control drawn in the last frame, for an accessibility element.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AccessibleControl {
    /// `cockpit-<manual|cruise|autopilot|journey|lane>`, `help-<topic>`, `compass`, `map-<i>`,
    /// `julia`, `cosmic`, `pi`.
    pub id: String,
    pub role: AccessibleRole,
    pub label: String,
    pub value: String,
    /// The current driving level's icon; lane assist while on.
    pub selected: bool,
    /// View points from the top left.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[uniffi::export]
impl Explorer {
    /// Every control Rust drew in the last frame, in reading order (compass, maps, Julia panel,
    /// cosmic scale, π meter, cockpit; each panel's help badge after it).
    pub fn accessible_controls(&self) -> Vec<AccessibleControl> {
        use almondbread_render::accessibility::Role;
        self.with_host(|h| h.accessible_controls(media_time()))
            .into_iter()
            .map(|a| AccessibleControl {
                id: a.id,
                role: match a.role {
                    Role::Button => AccessibleRole::Button,
                    Role::Image => AccessibleRole::Image,
                    Role::Text => AccessibleRole::Text,
                },
                label: a.label,
                value: a.value,
                selected: a.selected,
                x: a.rect[0],
                y: a.rect[1],
                width: a.rect[2],
                height: a.rect[3],
            })
            .collect()
    }

    /// The view in one short sentence for the canvas's accessibility value: zoom, centre (7
    /// digits), turn and driving level.
    pub fn accessibility_summary(&self) -> String {
        self.with_host(|h| h.accessibility_summary(media_time()))
    }
}
