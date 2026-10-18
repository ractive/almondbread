//! Guided tours for the UI (kb: decisions/controls "Guided tours"; core `tour`): the list of
//! bundled tours, and playback on an `Explorer`. Rust applies each step (view, settings,
//! overlays, the orbit probe and marker) and returns the step with the resulting settings, so
//! the UI mirrors them like after `go_to_place`; the end restores what the start saved. After
//! each call, resume the display link.

use almondbread_render::metal::media_time;

use almondbread_core::tour::Platform;

use crate::explorer::{Explorer, ExplorerSettings};
use crate::{CoreError, invalid};

/// A tour for the tour list.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TourInfo {
    pub id: String,
    pub title: String,
    /// One line.
    pub summary: String,
    /// The place whose detail offers the tour ("" if none).
    pub place: String,
    pub steps: u32,
    /// Not listed with the other tours (header `hidden: yes`, like the walkthrough): the UI
    /// offers it in its own place.
    pub hidden: bool,
}

/// The bundled tours, in list order, hidden ones included (`hidden` says which).
#[uniffi::export]
pub fn tours() -> Vec<TourInfo> {
    almondbread_core::tour::bundled()
        .into_iter()
        .map(|t| TourInfo {
            steps: t.steps.len() as u32,
            id: t.id,
            title: t.title,
            summary: t.summary,
            place: t.place.unwrap_or_default(),
            hidden: t.hidden,
        })
        .collect()
}

/// The platform a tour plays on by default: the Mac, or a touch screen (a tablet's steps).
fn default_platform() -> Platform {
    if cfg!(target_os = "ios") {
        Platform::Pad
    } else {
        Platform::Mac
    }
}

/// The running tour's step and the settings it shows (or, after the end, the restored ones).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TourStepInfo {
    /// False after the end (the settings are the restored ones).
    pub active: bool,
    pub tour_id: String,
    pub tour_title: String,
    /// 0-based.
    pub index: u32,
    pub count: u32,
    pub title: String,
    /// Markdown.
    pub text: String,
    pub settings: ExplorerSettings,
    /// The look the step shows ("" if the tour sets none: the user's stays).
    pub look: String,
    pub orbit: bool,
    pub orbit_length: u32,
    pub map: bool,
    pub map_levels: u32,
    pub cosmic: bool,
    pub pi_meter: bool,
    pub compass: bool,
    /// The UI element the step rings (`highlight:`; "" none).
    pub highlight: String,
    /// "open" or "closed": what the step does with the control center on entering ("" nothing).
    pub control_center: String,
    /// What the step asks the user to try (`goal:` zoom-in, zoom-out, move, turn; "" none);
    /// `Explorer::tour_goal_met` says when it is done.
    pub goal: String,
}

fn info(
    h: &almondbread_render::Host<almondbread_render::metal::MetalBackend>,
    ended: Option<(String, String)>,
) -> TourStepInfo {
    let snap = h.tour_snapshot();
    let (highlight, control_center, goal) = match &h.tour {
        Some(t) => {
            let s = t.step();
            (
                s.highlight.clone().unwrap_or_default(),
                match s.control_center {
                    Some(true) => "open",
                    Some(false) => "closed",
                    None => "",
                }
                .to_string(),
                s.goal.map(|g| g.name().to_string()).unwrap_or_default(),
            )
        }
        None => (String::new(), String::new(), String::new()),
    };
    let (active, tour_id, tour_title, index, count, title, text, look) = match (&h.tour, ended) {
        (Some(t), _) => (
            true,
            t.tour.id.clone(),
            t.tour.title.clone(),
            t.index() as u32,
            t.len() as u32,
            t.step().title.clone(),
            t.step().text.clone(),
            t.look().unwrap_or("").to_string(),
        ),
        (None, Some((id, title))) => (false, id, title, 0, 0, String::new(), String::new(), String::new()),
        (None, None) => (
            false,
            String::new(),
            String::new(),
            0,
            0,
            String::new(),
            String::new(),
            String::new(),
        ),
    };
    TourStepInfo {
        active,
        tour_id,
        tour_title,
        index,
        count,
        title,
        text,
        settings: ExplorerSettings::from(&snap.settings),
        look,
        orbit: snap.overlay.orbit,
        orbit_length: snap.overlay.orbit_length as u32,
        map: snap.overlay.map,
        map_levels: snap.overlay.map_levels as u32,
        cosmic: snap.cosmic,
        pi_meter: snap.pi_meter,
        compass: snap.compass,
        highlight,
        control_center,
        goal,
    }
}

#[uniffi::export]
impl Explorer {
    /// Starts the bundled tour `id` at its first step (other motion stops, like a jump; the
    /// settings and view are saved for the end).
    pub fn start_tour(&self, id: String) -> Result<TourStepInfo, CoreError> {
        self.start_tour_on(id, String::new())
    }

    /// Starts the bundled tour `id` as it plays on `platform` ("mac", "phone" or "pad"; "" the
    /// build's default): the steps and platform blocks for it (kb: decisions/controls "Guided
    /// tours").
    pub fn start_tour_on(&self, id: String, platform: String) -> Result<TourStepInfo, CoreError> {
        let p = if platform.is_empty() {
            default_platform()
        } else {
            Platform::named(&platform).ok_or_else(|| invalid(format!("unknown platform {platform}")))?
        };
        let tour = almondbread_core::tour::named(&id)
            .ok_or_else(|| invalid(format!("unknown tour {id}")))?
            .for_platform(p);
        self.with_host(|h| {
            h.start_tour(tour, media_time());
            Ok(info(h, None))
        })
    }

    /// Goes to step `index` (0-based, clamped): its view and settings ("Next" and "Back").
    /// None without a running tour.
    pub fn tour_go(&self, index: u32) -> Option<TourStepInfo> {
        self.with_host(|h| h.tour_go(index as usize, media_time()).then(|| info(h, None)))
    }

    /// The running step's goal (`goal:`) is done (false without one; poll it with the stats).
    pub fn tour_goal_met(&self) -> bool {
        self.with_host(|h| h.tour_goal_met())
    }

    /// The running tour's step, None without one.
    pub fn tour_step(&self) -> Option<TourStepInfo> {
        self.with_host(|h| h.tour.is_some().then(|| info(h, None)))
    }

    /// Ends the tour: the saved settings come back (returned, `active` false) and the view
    /// flies back to where the tour started. None without a running tour.
    pub fn end_tour(&self) -> Option<TourStepInfo> {
        self.with_host(|h| {
            let ended = h.tour.as_ref().map(|t| (t.tour.id.clone(), t.tour.title.clone()))?;
            h.end_tour(media_time());
            Some(info(h, Some(ended)))
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tours_are_listed() {
        let t = super::tours();
        assert!(
            t.iter()
                .any(|t| t.id == "feigenbaum" && t.steps >= 6 && t.place == "feigenbaum-point" && !t.hidden)
        );
        assert!(
            t.iter().any(|t| t.id == "welcome" && t.hidden),
            "the walkthrough is hidden"
        );
    }
}
