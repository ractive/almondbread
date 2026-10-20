//! Accessibility of the controls Rust draws into the frame (kb: plans/a11y-canvas-controls):
//! the cockpit's level and lane icons, the help badges, the compass, the mini-map panels, the
//! Julia panel, the π meter and the cosmic size scale exist only as pixels and Rust hit areas,
//! so VoiceOver, Switch Control and the Mac's accessibility clients cannot see them. The host
//! describes each one as it was drawn in the last scene (a role, a label, a value, its rect)
//! and the canvas as a whole in one short sentence; the platform views turn these into
//! `UIAccessibilityElement`s / `NSAccessibilityElement`s and route their actions to the same
//! calls a click or a tap makes. Built only when an accessibility client asks (no per-frame
//! work); the words live here so every platform says the same.

use almondbread_core::cfmt::g;
use almondbread_core::cruise::{self, Level};
use almondbread_core::input::Cockpit;
use almondbread_core::viewport::Viewport;

use crate::backend::GpuBackend;
use crate::help::HelpTopic;
use crate::host::{Host, zoom_text};
use crate::pi_meter::PiMeterState;

/// What kind of control an element is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Does something when activated (the platform routes it; see the id).
    Button,
    /// A picture with a value (a mini-map, the Julia panel).
    Image,
    /// Text that changes (the π meter, the cosmic size scale).
    Text,
}

/// A control drawn in the last scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Accessible {
    /// Stable id, the platform's key for the element and its action: `cockpit-<level>`,
    /// `cockpit-lane`, `help-<topic>`, `compass`, `map-<i>`, `julia`, `cosmic`, `pi`.
    pub id: String,
    pub role: Role,
    pub label: String,
    pub value: String,
    /// The current driving level's icon, lane assist while on.
    pub selected: bool,
    /// View points from the top left: x, y, width, height.
    pub rect: [f64; 4],
}

/// A help badge's label ("Help: …", never just the panel's name: the round buttons on iOS
/// carry those).
pub fn help_label(topic: HelpTopic) -> String {
    let what = match topic {
        HelpTopic::CosmicScale => "cosmic size scale",
        HelpTopic::PiMeter => "π meter",
        HelpTopic::MiniMap => "mini-map",
        HelpTopic::Orbit => "orbit map",
        HelpTopic::Cockpit => "driving levels",
        HelpTopic::Julia => "Julia panel",
    };
    format!("Help: {what}")
}

/// The zoom rate flown, in words: "zooming in ×4.2 per second", "at rest".
pub fn rate_words(rate: f64) -> String {
    if !rate.is_finite() || rate.abs() < 0.02 {
        return "at rest".into();
    }
    let dir = if rate < 0.0 { "out" } else { "in" };
    format!("zooming {dir} {} per second", cruise::factor_text(rate))
}

/// The current level's value: "current", plus the gear (cruise) or the set speed (autopilot,
/// journey) and the rate flown.
pub fn level_value(c: &Cockpit) -> String {
    let mut parts = vec!["current".to_string()];
    if c.level == Level::Cruise
        && let Some(gear) = cruise::gear(c.set).filter(|_| c.set.is_finite() && c.set != 0.0)
    {
        parts.push(if c.set > 0.0 {
            format!("gear {gear}")
        } else {
            format!("reverse gear {gear}")
        });
    } else if matches!(c.level, Level::Autopilot | Level::Journey) && c.set.is_finite() && c.set != 0.0 {
        parts.push(format!("set speed {}", cruise::factor_text(c.set)));
    }
    if c.level != Level::Manual || c.rate.abs() >= 0.02 {
        parts.push(rate_words(c.rate));
    }
    parts.join(", ")
}

/// The compass's value: how far the view is turned from north.
pub fn compass_value(angle: f64) -> String {
    if !angle.is_finite() {
        return String::new();
    }
    // Counter-clockwise radians, shown as degrees in -180 ... 180.
    let mut d = angle.to_degrees() % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d < -180.0 {
        d += 360.0;
    }
    let r = d.round();
    if r == 0.0 {
        "north is up".into()
    } else if r > 0.0 {
        format!("turned {r}° counter-clockwise")
    } else {
        format!("turned {}° clockwise", -r)
    }
}

/// A mini-map panel's value: its level ("whole set, level 1 of 3", "zoom 1e+06×, level 2 of 3").
pub fn map_value(decade: i32, index: usize, count: usize) -> String {
    let what = if decade <= 0 {
        "whole set".to_string()
    } else {
        format!("zoom {}", zoom_text(f64::from(decade)))
    };
    format!("{what}, level {} of {count}", index + 1)
}

/// The π meter's value: the latest count and how many digits agree, or what it waits for.
pub fn pi_value(s: &PiMeterState) -> String {
    if s.point.is_empty() {
        return "centre the view on the neck or the cusp".into();
    }
    let mut parts = vec![format!("at the {}", s.point)];
    if s.count > 0 && s.product.is_finite() {
        let scale = if s.point == "cusp" {
            s.count_eps.sqrt()
        } else {
            s.count_eps
        };
        parts.push(format!(
            "{} steps, product {}, {} digits of π",
            s.count,
            almondbread_core::pi_moment::product_text(s.product, scale),
            s.digits
        ));
    }
    if s.counting {
        parts.push(format!("counting, {:.0} %", 100.0 * s.progress));
    } else if s.too_deep {
        parts.push("too deep to count".into());
    }
    parts.join(", ")
}

/// The canvas in one sentence: zoom, centre, turn and the driving level (not 60 digits).
pub fn summary(v: &Viewport, c: &Cockpit) -> String {
    let mut s = format!(
        "zoom {}, centre {}, {}",
        zoom_text(v.zoom_log10()),
        g(v.center.re(), 7),
        g(v.center.im(), 7)
    );
    if v.angle != 0.0 {
        s.push_str(", ");
        s.push_str(&compass_value(v.angle));
    }
    if c.level != Level::Manual {
        s.push_str(", ");
        s.push_str(&crate::cockpit::level_name(c.level).to_lowercase());
        if let Some(gear) = cruise::gear(c.set).filter(|_| c.level == Level::Cruise && c.set != 0.0) {
            s.push_str(&format!(" gear {gear}"));
        }
    }
    s
}

impl<B: GpuBackend + Clone> Host<B> {
    /// The canvas as one short sentence ([`summary`]).
    pub fn accessibility_summary(&self, now: f64) -> String {
        summary(&self.renderer.viewport, &self.renderer.navigator.cockpit(now))
    }

    /// Every control drawn in the last scene, in reading order: compass, maps, Julia panel,
    /// cosmic scale, π meter, cockpit, each followed by its help badge.
    pub fn accessible_controls(&self, now: f64) -> Vec<Accessible> {
        let (wp, hp) = self.view_points();
        let (w, _) = self.pixel_size();
        if w == 0 || !(wp > 0.0 && hp > 0.0) {
            return Vec::new();
        }
        let ppp = wp / w as f64;
        let px_rect = |r: [f32; 4]| {
            let f = |v: f32| f64::from(v) * ppp;
            [f(r[0]), f(r[1]), f(r[2] - r[0]), f(r[3] - r[1])]
        };
        // Centre-based, y-up points to a top-left rect.
        let centred = |x: f64, y: f64, hw: f64, hh: f64| [wp / 2.0 + x - hw, hp / 2.0 - y - hh, 2.0 * hw, 2.0 * hh];
        let badges = self.help_badges();
        let mut out = Vec::new();
        let push_help = |out: &mut Vec<Accessible>, topics: &[HelpTopic]| {
            for (topic, (x, y), r) in badges.iter().filter(|b| topics.contains(&b.0)) {
                // The badge's touch target, not just the 14-point circle.
                let r = r.max(11.0);
                out.push(Accessible {
                    id: format!("help-{}", topic.id()),
                    role: Role::Button,
                    label: help_label(*topic),
                    value: String::new(),
                    selected: false,
                    rect: centred(*x, *y, r, r),
                });
            }
        };
        let panel = |id: &str| self.overlay.panels.iter().find(|(p, _)| *p == id).map(|(_, r)| *r);
        let view = &self.renderer.viewport;
        // ---- Compass.
        if self.overlay.compass.shown_in_last_frame()
            && let Some((x, y)) = self.overlay.compass.center()
        {
            let r = almondbread_core::compass::RADIUS;
            out.push(Accessible {
                id: "compass".into(),
                role: Role::Button,
                label: "Compass".into(),
                value: compass_value(view.angle),
                selected: false,
                rect: centred(x, y, r, r),
            });
        }
        // ---- Mini-maps (or the orbit's whole-set map).
        let maps = self.overlay.map_panels();
        let map_label = if self.overlay.settings.map {
            "Mini-map panel"
        } else {
            "Orbit map"
        };
        for (i, (r, decade)) in maps.iter().enumerate() {
            out.push(Accessible {
                id: format!("map-{i}"),
                role: Role::Image,
                label: map_label.into(),
                value: map_value(*decade, i, maps.len()),
                selected: false,
                rect: px_rect(*r),
            });
        }
        push_help(&mut out, &[HelpTopic::MiniMap, HelpTopic::Orbit]);
        // ---- Julia panel.
        if let Some(r) = panel("julia") {
            out.push(Accessible {
                id: "julia".into(),
                role: Role::Image,
                label: "Julia set panel".into(),
                value: self
                    .overlay
                    .julia
                    .k
                    .map_or_else(String::new, |k| format!("c = {}", crate::overlay::complex_text(k))),
                selected: false,
                rect: px_rect(r),
            });
        }
        push_help(&mut out, &[HelpTopic::Julia]);
        // ---- Cosmic size scale.
        if let Some(r) = panel("cosmic") {
            let value = self.overlay.cosmic.reading().map_or_else(String::new, |t| {
                format!("{} {}", t.line.trim(), t.detail.trim()).trim().to_string()
            });
            out.push(Accessible {
                id: "cosmic".into(),
                role: Role::Text,
                label: "Cosmic size scale".into(),
                value,
                selected: false,
                rect: px_rect(r),
            });
        }
        push_help(&mut out, &[HelpTopic::CosmicScale]);
        // ---- π meter.
        if let Some(r) = panel("pi") {
            out.push(Accessible {
                id: "pi".into(),
                role: Role::Text,
                label: "π meter".into(),
                value: pi_value(&self.overlay.pi.state()),
                selected: false,
                rect: px_rect(r),
            });
        }
        push_help(&mut out, &[HelpTopic::PiMeter]);
        // ---- Cockpit: the level icons and the lane icon.
        if panel("cockpit").is_some() {
            let c = self.renderer.navigator.cockpit(now);
            for (id, (x, y), (hw, hh)) in self.cockpit_controls() {
                let (label, selected, value) = match id {
                    "lane" => ("Lane assist", c.assist, if c.assist { "on" } else { "off" }.to_string()),
                    _ => {
                        let level = match id {
                            "manual" => Level::Manual,
                            "cruise" => Level::Cruise,
                            "autopilot" => Level::Autopilot,
                            _ => Level::Journey,
                        };
                        let on = level == c.level;
                        let value = if on { level_value(&c) } else { String::new() };
                        (crate::cockpit::level_name(level), on, value)
                    }
                };
                out.push(Accessible {
                    id: format!("cockpit-{id}"),
                    role: Role::Button,
                    label: label.into(),
                    value,
                    selected,
                    rect: centred(x, y, hw, hh),
                });
            }
        }
        push_help(&mut out, &[HelpTopic::Cockpit]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cockpit(level: Level, set: f64, rate: f64) -> Cockpit {
        Cockpit {
            level,
            set,
            rate,
            boost: 1.0,
            landing: false,
            assist: false,
            nudging: false,
        }
    }

    #[test]
    fn level_values_say_gear_and_rate() {
        assert_eq!(level_value(&cockpit(Level::Manual, 0.0, 0.0)), "current");
        let gear = cruise::gear(1.0).expect("a gear");
        let v = level_value(&cockpit(Level::Cruise, 1.0, 1.0));
        assert!(v.starts_with(&format!("current, gear {gear}, zooming in ×")), "{v}");
        let v = level_value(&cockpit(Level::Cruise, -1.0, -1.0));
        assert!(v.contains("reverse gear") && v.contains("zooming out"), "{v}");
        let v = level_value(&cockpit(Level::Autopilot, 1.5, 0.0));
        assert!(v.contains("set speed ×") && v.ends_with("at rest"), "{v}");
    }

    #[test]
    fn compass_values() {
        assert_eq!(compass_value(0.0), "north is up");
        assert_eq!(compass_value(35f64.to_radians()), "turned 35° counter-clockwise");
        assert_eq!(compass_value((-120f64).to_radians()), "turned 120° clockwise");
        assert_eq!(compass_value(350f64.to_radians()), "turned 10° clockwise");
        assert_eq!(compass_value(f64::NAN), "");
    }

    #[test]
    fn map_values_name_the_level() {
        assert_eq!(map_value(0, 0, 3), "whole set, level 1 of 3");
        assert_eq!(map_value(6, 1, 3), "zoom 1e+06×, level 2 of 3");
    }

    #[test]
    fn pi_values() {
        assert_eq!(
            pi_value(&PiMeterState::default()),
            "centre the view on the neck or the cusp"
        );
        let s = PiMeterState {
            point: "neck".into(),
            eps: 1e-6,
            count: 3_141_592,
            count_eps: 1e-6,
            product: 3.141_592_1,
            digits: 7,
            counting: true,
            progress: 0.4,
            too_deep: false,
        };
        let v = pi_value(&s);
        assert!(v.starts_with("at the neck, 3141592 steps, product 3.14159"), "{v}");
        assert!(v.ends_with("counting, 40 %"), "{v}");
    }

    #[test]
    fn summary_is_short() {
        let v = Viewport::home();
        let s = summary(&v, &cockpit(Level::Manual, 0.0, 0.0));
        assert!(s.starts_with("zoom 1×, centre "), "{s}");
        assert!(s.len() < 80, "{s}");
        let mut turned = v.clone();
        turned.angle = 35f64.to_radians();
        let s = summary(&turned, &cockpit(Level::Cruise, 1.0, 1.0));
        assert!(
            s.contains("turned 35° counter-clockwise") && s.contains("cruise gear"),
            "{s}"
        );
    }

    #[test]
    fn help_labels_never_equal_a_round_button() {
        for t in [
            HelpTopic::CosmicScale,
            HelpTopic::PiMeter,
            HelpTopic::MiniMap,
            HelpTopic::Orbit,
            HelpTopic::Cockpit,
            HelpTopic::Julia,
        ] {
            assert!(help_label(t).starts_with("Help: "));
        }
    }
}
