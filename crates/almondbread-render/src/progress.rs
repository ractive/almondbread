//! Render progress (kb: decisions/controls "Render progress"): how far the computation of a
//! settled view has got, for the thin line at the top of the canvas and the HUD text.
//!
//! The scheduler hands out a [`ProgressSample`] per refresh (read-only counters: stage, the
//! current level's admission and live counts, the youngest live pixel's iteration count from
//! the chunk stats, extension passes, AA samples). [`ProgressTracker`] turns the samples into
//! a [`RenderProgress`]: an honest fraction for the line and stable numbers for the HUD.

use crate::overlay::{Scene, rgba};
use crate::scheduler::FurtherInfo;

/// What the scheduler is doing for the current view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    /// Nothing to compute (settled, or no view).
    #[default]
    Idle,
    /// A deep view waits for its reference orbit (no job yet).
    Reference,
    /// The full level waits for its BLA / LA table.
    Tables,
    /// The quick preview level of a refined view (or a gesture frame).
    Preview,
    /// The full-resolution level.
    Full,
    /// Extension passes: unresolved pixels continued past the limit.
    Extension,
    /// Compute further: the passes after the extension passes, up to the iteration cap (kb:
    /// techniques/auto-iterations "Compute further").
    Further,
    /// Idle anti-aliasing samples (and the fade-in before them).
    AntiAlias,
    /// The Buddhabrot / Nebulabrot look accumulates orbits (BB, kb: plans/buddhabrot).
    Orbits,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::Idle => "idle",
            Stage::Reference => "reference",
            Stage::Tables => "tables",
            Stage::Preview => "preview",
            Stage::Full => "full",
            Stage::Extension => "extension",
            Stage::Further => "further",
            Stage::AntiAlias => "anti-aliasing",
            Stage::Orbits => "orbits",
        }
    }
}

/// One refresh's progress counters (see [`crate::Renderer::progress_sample`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgressSample {
    /// The job (generation) the counters belong to (0: none).
    pub generation: u64,
    /// Input moves the view (gesture frames, flights, the autopilot).
    pub interacting: bool,
    pub stage: Stage,
    /// The refined, extended and anti-aliased frame is presented and nothing is pending.
    pub settled: bool,
    /// The current level (its index in the job; levels of one job never move).
    pub level_index: usize,
    /// Current level: slots (16x16 tiles x 256), admitted so far, live after the last chunk,
    /// its limit, and the youngest live pixel's iteration count.
    pub level_total: usize,
    pub level_admitted: usize,
    pub level_live: usize,
    pub level_limit: usize,
    pub live_min_n: Option<u64>,
    /// Pixels of the current level that reached its limit unresolved so far.
    pub level_crossed: usize,
    /// The current level is an AA sample (in the AA stage, None between samples).
    pub level_aa: Option<u32>,
    /// The level's seed passes (M5) are not final yet: nodes left unfinished by the last
    /// round (None before the first round reported) and nodes in all.
    pub seeding: Option<(Option<usize>, usize)>,
    /// GPU seconds of the current level so far.
    pub level_gpu: f64,
    /// The view's nominal limit.
    pub soft_limit: usize,
    /// Slots of the full level (0 before it exists).
    pub full_total: usize,
    /// Unresolved pixels the last finished full or extension level left (None before one
    /// finished): what the "raise the limit?" hint needs.
    pub unresolved: Option<usize>,
    /// Extension passes so far (started), the hard limit they may go to, their GPU seconds so
    /// far and their budget (GPU seconds).
    pub extension_passes: usize,
    pub extend_to: usize,
    pub extension_gpu: f64,
    pub extension_budget: f64,
    /// AA samples composed, the target, the adaptive list's length once known, the composed
    /// samples' GPU seconds and their budget.
    pub aa_composed: u32,
    pub aa_target: u32,
    pub aa_pixels: Option<usize>,
    pub aa_gpu: f64,
    pub aa_budget: f64,
    /// GPU seconds of the full level (its centre samples), and whether it is done.
    pub full_gpu: f64,
    pub full_done: bool,
    /// The deep reference being computed: seconds so far, orbit iterations done, and the
    /// request's limit.
    pub reference: Option<(f64, usize, usize)>,
    /// The job iterates against a truncated reference (rendered again when it arrives).
    pub provisional: bool,
    /// GPU seconds of the job's iterate chunks so far.
    pub gpu_time: f64,
    /// Compute further of the view (None: not started), the full level's texels and the
    /// unresolved share below which a view counts as finished.
    pub further: Option<FurtherInfo>,
    pub full_texels: usize,
    pub finished_share: f64,
    /// The Buddhabrot look's accumulation (stage `Orbits`).
    pub orbits: Option<crate::scheduler::OrbitProgress>,
}

/// The line appears once the settled view has computed this long (seconds).
pub const SHOW_AFTER: f64 = 0.5;
/// ... if the estimate leaves more than this (seconds), or once it has taken this long.
const SHOW_MIN_LEFT: f64 = 0.6;
const SHOW_LATEST: f64 = 1.5;
/// Fade in, run to the end, fade out; input hides the line faster (seconds).
const FADE_IN: f64 = 0.2;
const COMPLETE: f64 = 0.3;
const FADE_OUT: f64 = 0.45;
const HIDE_FAST: f64 = 0.15;
/// The shown fraction follows the estimate with this time constant (seconds).
const FOLLOW: f64 = 0.25;
/// The line closes at most 1 / MAX_CLOSE of the remaining gap per second.
const MAX_CLOSE: f64 = 2.0;
/// While the remaining time is unknown (reference, tables, seeds), the line creeps towards
/// this fraction with this time constant: it moves, but claims little.
const CREEP_CAP: f64 = 0.2;
const CREEP_TAU: f64 = 25.0;
/// The ETA shows after the line has shown this long, once its estimate held still.
const ETA_AFTER: f64 = 3.0;
const ETA_SMOOTH: f64 = 2.0;
const ETA_WINDOW: f64 = 3.0;
const ETA_SPREAD: f64 = 0.3;
const ETA_LEVEL_AGE: f64 = 8.0;
/// Share of the view the AA list takes, assumed until it is built.
const AA_SHARE_GUESS: f64 = 0.5;

/// What the line and the HUD show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderProgress {
    /// The HUD text shows (the line shows while `alpha` > 0).
    pub visible: bool,
    /// The line's opacity (fades in and out).
    pub alpha: f64,
    /// The line's length, 0..1, never decreasing while it shows.
    pub fraction: f64,
    pub stage: Stage,
    /// The full level waits for its M5 seed passes (its pixels have not started).
    pub seeding: bool,
    /// The youngest live pixel's iteration count and the level's limit, once every pixel is
    /// admitted (before, the youngest pixel is a fresh one).
    pub steps: Option<(u64, u64)>,
    /// Share of the view's pixels still computing in the current level, and their count.
    pub pixels_left: Option<(f64, usize)>,
    /// The AA sample being computed and how many the view takes at most.
    pub aa_sample: Option<(u32, u32)>,
    /// Remaining seconds, once stable (smoothed).
    pub eta: Option<f64>,
    /// Pixels the last finished full or extension level left unresolved, and the full
    /// level's slot count (for a "raise the limit?" hint).
    pub unresolved: Option<(usize, usize)>,
    /// Compute further: the share of the view's texels not finished and the limit they have
    /// reached; while it runs, the pass's limit (else 0) and why it stopped.
    pub not_finished: Option<NotFinished>,
    /// The Buddhabrot look's accumulation while it shows (kb: plans/buddhabrot "Convergence").
    pub orbits: Option<crate::scheduler::OrbitProgress>,
    /// The resting view's Buddhabrot stopped at its GPU budget before converging (the settled
    /// line; "Continue" resumes it).
    pub orbits_paused: Option<crate::scheduler::OrbitProgress>,
}

/// "Buddhabrot" or "Nebulabrot".
fn look_name(o: &crate::scheduler::OrbitProgress) -> &'static str {
    if o.mode == almondbread_core::buddha::BuddhaMode::Nebulabrot {
        "Nebulabrot"
    } else {
        "Buddhabrot"
    }
}

/// "40 %" (never 100 before the end).
fn percent_text(p: f64) -> String {
    format!("{:.0} %", (100.0 * p).clamp(0.0, 99.0).floor())
}

/// A settled (or still computing) view with unresolved pixels (kb: techniques/auto-iterations
/// "Compute further").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NotFinished {
    pub share: f64,
    pub reached: u64,
    pub limit: u64,
    pub why: Option<&'static str>,
}

impl NotFinished {
    /// "12 % not finished", "0.4 % not finished".
    fn share_text(&self) -> String {
        let pct = 100.0 * self.share;
        if pct >= 99.95 {
            "nothing finished yet".to_string()
        } else if pct >= 9.5 {
            format!("{pct:.0} % not finished")
        } else if pct >= 0.95 {
            format!("{pct:.1} % not finished")
        } else {
            format!("{pct:.2} % not finished")
        }
    }

    /// The settled HUD line: "12 % not finished at 1.2 M steps (upper limit)".
    pub fn text(&self) -> String {
        let mut t = format!("{} at {} steps", self.share_text(), count_text(self.reached as f64));
        if let Some(w) = self.why {
            t.push_str(&format!(" ({w})"));
        }
        t
    }
}

impl RenderProgress {
    /// The HUD line, e.g. "refining · 2.1 M of 10 M steps · 3 % of pixels left · ~40 s";
    /// empty while hidden.
    pub fn text(&self) -> String {
        if !self.visible {
            // The Buddhabrot stopped at its budget: "Buddhabrot · paused at 60 % after 1 min".
            if let Some(o) = self.orbits_paused {
                return format!(
                    "{} · paused at {} after {}",
                    look_name(&o),
                    percent_text(o.sharp),
                    minutes_text(o.seconds)
                );
            }
            // Settled with pixels left unresolved: say so (compute further stopped).
            return self.not_finished.map_or_else(String::new, |n| n.text());
        }
        let mut parts = vec![self.stage_label().to_string()];
        if self.stage == Stage::Further {
            // "computing further · 1.2 M of 4 M steps · 12 % not finished · ~40 s"
            if let Some((n, limit)) = self.steps {
                parts.push(format!(
                    "{} of {} steps",
                    count_text(n as f64),
                    count_text(limit as f64)
                ));
            } else if let Some(n) = self.not_finished.filter(|n| n.limit > 0) {
                parts.push(format!("to {} steps", count_text(n.limit as f64)));
            }
            if let Some(n) = self.not_finished {
                parts.push(n.share_text());
            }
            if let Some(e) = self.eta {
                parts.push(eta_text(e));
            }
            return parts.join(" · ");
        }
        if let Some(o) = self.orbits {
            // "Buddhabrot · sharpening · 40 % · ~20 s", deep: "... · deep: takes minutes here"
            let mut parts = vec![
                look_name(&o).to_string(),
                "sharpening".to_string(),
                percent_text(o.progress),
            ];
            if let Some(e) = self.eta {
                parts.push(eta_text(e));
            }
            if o.deep {
                parts.push("deep: takes minutes here".to_string());
            }
            return parts.join(" · ");
        }
        if let Some((n, of)) = self.aa_sample {
            parts.push(format!("sample {n} of {of}"));
        }
        if let Some((n, limit)) = self.steps {
            parts.push(format!(
                "{} of {} steps",
                count_text(n as f64),
                count_text(limit as f64)
            ));
        }
        if let Some((share, count)) = self.pixels_left {
            parts.push(left_text(share, count));
        }
        if let Some(e) = self.eta {
            parts.push(eta_text(e));
        }
        parts.join(" · ")
    }

    pub fn stage_label(&self) -> &'static str {
        match self.stage {
            Stage::Idle => "done",
            Stage::Reference => "reference orbit",
            Stage::Tables => "preparing",
            Stage::Preview => "preview",
            Stage::Full if self.seeding => "seeding",
            Stage::Full => "refining",
            Stage::Extension => "resolving",
            Stage::Further => "computing further",
            Stage::AntiAlias => "anti-aliasing",
            Stage::Orbits => "orbits",
        }
    }
}

/// "900", "12 k", "350 k", "2.1 M", "10 M", "1.5 G".
pub fn count_text(v: f64) -> String {
    let (x, unit) = if v >= 999.5e6 {
        (v / 1e9, " G")
    } else if v >= 999.5e3 {
        (v / 1e6, " M")
    } else if v >= 999.5 {
        (v / 1e3, " k")
    } else {
        return format!("{v:.0}");
    };
    if x < 9.95 && (x - x.round()).abs() >= 0.05 {
        format!("{x:.1}{unit}")
    } else {
        format!("{x:.0}{unit}")
    }
}

/// "3 % of pixels left", "0.4 % of pixels left", "120 pixels left".
fn left_text(share: f64, count: usize) -> String {
    let pct = 100.0 * share;
    if pct >= 9.5 {
        format!("{pct:.0} % of pixels left")
    } else if pct >= 0.95 {
        format!("{pct:.1} % of pixels left")
    } else if count < 1000 {
        format!("{count} pixel{} left", if count == 1 { "" } else { "s" })
    } else {
        format!("{pct:.2} % of pixels left")
    }
}

/// "~7 s", "~40 s", "~3 min", "~1.5 h" (coarser as it grows, so it does not flicker).
pub fn eta_text(seconds: f64) -> String {
    let s = seconds.max(1.0);
    if s < 10.0 {
        format!("~{:.0} s", s.round())
    } else if s < 90.0 {
        format!("~{:.0} s", (s / 5.0).round() * 5.0)
    } else if s < 3600.0 {
        format!("~{:.0} min", (s / 60.0).round().max(2.0))
    } else {
        format!("~{:.1} h", s / 3600.0)
    }
}

/// "1 min", "3 min" (whole minutes, at least one).
fn minutes_text(seconds: f64) -> String {
    format!("{:.0} min", (seconds / 60.0).round().max(1.0))
}

/// One level being followed: its counters over the last seconds.
#[derive(Clone, Debug)]
struct LevelTrack {
    generation: u64,
    index: usize,
    stage: Stage,
    /// The AA sample it computes (None: not an AA sample level).
    aa: Option<u32>,
    total: usize,
    start: f64,
    /// (time, pixels left, youngest live pixel's n once all are admitted).
    history: std::collections::VecDeque<(f64, f64, Option<f64>)>,
    /// The largest youngest-pixel n seen since every pixel was admitted.
    n_max: Option<u64>,
    /// When its first pixels were admitted (after its seeds).
    pixels_since: Option<f64>,
}

/// Turns the scheduler's [`ProgressSample`]s into [`RenderProgress`] (kb: decisions/controls
/// "Render progress"). The line is time-based, `elapsed / (elapsed + remaining)`: the
/// remaining time comes from the current level's counters plus predictions for the stages
/// still to come. The estimates lean pessimistic, so the line speeds up towards the end
/// rather than racing ahead and stalling; it never moves backwards.
#[derive(Clone, Debug, Default)]
pub struct ProgressTracker {
    /// When the current computation of a settled view began (None: settled or moving).
    busy_since: Option<f64>,
    /// The job of the busy period being computed and when it started.
    job: Option<(u64, f64)>,
    shown_since: Option<f64>,
    /// Finished (true: run to the end first) or interrupted at this time: fading out.
    ending: Option<(f64, bool)>,
    /// The fraction and opacity when the ending began.
    end_from: (f64, f64),
    last_now: f64,
    fraction: f64,
    level: Option<LevelTrack>,
    /// Durations (seconds) of finished levels in this busy period: the preview (and its
    /// slots), the full level, the last AA sample.
    preview: Option<(f64, usize)>,
    full: Option<f64>,
    aa_sample: Option<f64>,
    /// ETA: the smoothed finish time, the recent raw finish times, and whether it shows.
    finish: Option<f64>,
    finishes: std::collections::VecDeque<(f64, f64)>,
    eta_shown: bool,
    /// The last remaining-time estimate (diagnostics: `almondbread progress`).
    pub remaining: Option<f64>,
    shown: RenderProgress,
}

impl ProgressTracker {
    pub fn current(&self) -> &RenderProgress {
        &self.shown
    }

    /// The line needs frames: it shows or fades, or is about to show.
    pub fn animating(&self) -> bool {
        self.shown.alpha > 0.0 || (self.busy_since.is_some() && self.shown_since.is_none())
    }

    pub fn update(&mut self, s: &ProgressSample, now: f64) {
        self.step(s, now);
        // (Kept while idle too: what the settled view left unresolved.)
        self.shown.unresolved = s.unresolved.map(|u| (u, s.full_total));
        self.shown.not_finished = not_finished(s);
        self.shown.orbits_paused = s.orbits.filter(|o| o.stopped && !s.interacting);
    }

    fn step(&mut self, s: &ProgressSample, now: f64) {
        let dt = (now - self.last_now).clamp(0.0, 0.25);
        self.last_now = now;
        let working = !s.interacting && !s.settled;
        if s.interacting {
            // Input: no line while the view moves; a shown one fades quickly.
            if self.shown_since.is_some() && self.ending.is_none() {
                self.ending = Some((now, false));
                self.end_from = (self.fraction, self.shown.alpha);
            }
            if self.shown_since.is_none() {
                self.reset();
            }
            self.busy_since = None;
        } else if working {
            if self.busy_since.is_none() || self.ending.is_some_and(|(_, done)| done) {
                self.reset();
                self.busy_since = Some(now);
            }
        } else if self.busy_since.is_some() && self.ending.is_none() {
            // Settled.
            if self.shown_since.is_some() {
                self.ending = Some((now, true));
                self.end_from = (self.fraction, self.shown.alpha);
            } else {
                self.reset();
            }
            self.busy_since = None;
        }
        if let Some((t, completed)) = self.ending {
            self.fade(s, now, t, completed);
            return;
        }
        let Some(since) = self.busy_since else {
            self.shown = RenderProgress::default();
            return;
        };
        if self.job.is_none_or(|(g, _)| g != s.generation) {
            self.job = Some((s.generation, now));
        }
        self.track_level(s, now);
        let elapsed = now - since;
        self.remaining = self.estimate(s, now);
        let target = match self.remaining {
            Some(r) if elapsed > 0.0 => elapsed / (elapsed + r.max(0.0)),
            _ => self.fraction + (CREEP_CAP - self.fraction).max(0.0) * (1.0 - (-dt / CREEP_TAU).exp()),
        };
        if target > self.fraction {
            // (At most a share of what is left per second: one optimistic estimate cannot
            // throw the line ahead, where it would then stall.)
            let step = (target - self.fraction) * (1.0 - (-dt / FOLLOW).exp());
            self.fraction += step.min((1.0 - self.fraction) * dt / MAX_CLOSE);
        }
        self.fraction = self.fraction.clamp(0.0, 0.995);
        // Shown after SHOW_AFTER, unless the end is in sight: a line that appears only to run
        // to the end at once is a flash (after SHOW_LATEST it shows regardless).
        // Unknown counts only in the stages that are unknown for long (a level's first moments
        // are unknown too, briefly).
        let long_unknown = matches!(s.stage, Stage::Reference | Stage::Tables)
            || (s.stage == Stage::Full && s.seeding.is_some() && s.level_admitted == 0);
        let worth = match self.remaining {
            Some(r) => r > SHOW_MIN_LEFT,
            None => long_unknown,
        };
        if self.shown_since.is_none() && elapsed >= SHOW_AFTER && (worth || elapsed >= SHOW_LATEST) {
            self.shown_since = Some(now);
        }
        self.update_eta(now);
        let alpha = self.shown_since.map_or(0.0, |t| ((now - t) / FADE_IN).clamp(0.0, 1.0));
        self.shown = self.describe(s, alpha);
    }

    fn reset(&mut self) {
        *self = Self {
            last_now: self.last_now,
            ..Self::default()
        };
    }

    /// Fades the line out: a finished computation first runs the line to the end. The HUD
    /// text goes at once.
    fn fade(&mut self, s: &ProgressSample, now: f64, t: f64, completed: bool) {
        let age = now - t;
        let (from, from_alpha) = self.end_from;
        let (alpha, fraction) = if completed {
            let k = (age / COMPLETE).clamp(0.0, 1.0);
            let f = from + (1.0 - from) * (1.0 - (1.0 - k) * (1.0 - k));
            let fade_in = ((now - self.shown_since.unwrap_or(t)) / FADE_IN).clamp(0.0, 1.0);
            (fade_in * (1.0 - ((age - COMPLETE) / FADE_OUT).clamp(0.0, 1.0)), f)
        } else {
            (from_alpha * (1.0 - (age / HIDE_FAST).clamp(0.0, 1.0)), from)
        };
        if alpha <= 0.0 {
            self.reset();
            self.shown = RenderProgress::default();
            return;
        }
        self.fraction = fraction;
        let mut p = self.describe(s, alpha);
        p.visible = false;
        p.eta = None;
        self.shown = p;
    }

    /// Follows the current level's counters (a new level starts a new track; a finished one
    /// leaves its duration for the predictions).
    fn track_level(&mut self, s: &ProgressSample, now: f64) {
        let same = self.level.as_ref().is_some_and(|l| {
            l.generation == s.generation && l.index == s.level_index && l.stage == s.stage && l.aa == s.level_aa
        });
        if !same {
            if let Some(old) = self.level.take() {
                let secs = now - old.start;
                match old.stage {
                    Stage::Preview => self.preview = Some((secs, old.total.max(1))),
                    Stage::Full => self.full = Some(secs),
                    // (Only a sample level: not the moments between samples.)
                    Stage::AntiAlias if old.aa.is_some() => self.aa_sample = Some(secs),
                    _ => {}
                }
            }
            self.level = Some(LevelTrack {
                generation: s.generation,
                index: s.level_index,
                stage: s.stage,
                aa: s.level_aa,
                total: s.level_total,
                start: now,
                history: Default::default(),
                n_max: None,
                pixels_since: None,
            });
        }
        let level = self.level.as_mut().expect("level");
        level.total = s.level_total;
        let all_in = s.level_total > 0 && s.level_admitted >= s.level_total;
        if s.level_admitted > 0 && level.pixels_since.is_none() {
            level.pixels_since = Some(now);
        }
        if all_in && let Some(n) = s.live_min_n {
            level.n_max = Some(level.n_max.map_or(n, |m| m.max(n)));
        }
        let left = pixels_left(s) as f64;
        let n = if all_in { level.n_max.map(|n| n as f64) } else { None };
        level.history.push_back((now, left, n));
        while level.history.len() > 2 && level.history.front().is_some_and(|h| now - h.0 > 21.0) {
            level.history.pop_front();
        }
    }

    /// Seconds still to go: the current level from its counters, plus the stages after it.
    /// None while unknown (reference, tables, seeds, a level's first moments).
    fn estimate(&self, s: &ProgressSample, now: f64) -> Option<f64> {
        let wall_per_gpu = self
            .job
            .map(|(_, t0)| now - t0)
            .filter(|_| s.gpu_time > 0.25)
            .map_or(1.2, |w| (w / s.gpu_time).clamp(1.0, 4.0));
        match s.stage {
            Stage::Idle => Some(0.0),
            // The orbit has a count (the request's limit bounds it), but what follows, the
            // whole view, is unknown yet.
            Stage::Reference | Stage::Tables => None,
            Stage::Preview => {
                let level = self.level_remaining(s, now)?;
                let secs = self.level_age(now) + level;
                let full = if s.full_total > 0 {
                    secs * s.full_total as f64 / s.level_total.max(1) as f64
                } else {
                    0.0
                };
                Some(level + full + self.after_full(s, full, wall_per_gpu))
            }
            Stage::Full => {
                if s.seeding.is_some() && s.level_admitted == 0 {
                    return None;
                }
                let level = self.level_remaining(s, now)?;
                let full = self.level_age(now) + level;
                Some(level + self.after_full(s, full, wall_per_gpu))
            }
            Stage::Extension => {
                let level = self.level_remaining(s, now)?;
                // A further pass when this one stops short of the hard limit and budget is
                // left (passes are never cut short: count what is left of the budget).
                let more = if s.level_limit < s.extend_to {
                    (s.extension_budget - s.extension_gpu).max(0.0) * wall_per_gpu
                } else {
                    0.0
                };
                let full = self.full.unwrap_or(1.0);
                Some(level + more + self.aa_remaining(s, full, wall_per_gpu))
            }
            // The scheduler's estimate of the passes still to come (bounded by the budget).
            Stage::Further => {
                let f = s.further?;
                let full = self.full.unwrap_or(1.0);
                Some(f.remaining_gpu * wall_per_gpu + self.aa_remaining(s, full, wall_per_gpu))
            }
            // The progress (hits towards convergence, or the budget) grows linearly with time.
            Stage::Orbits => {
                let o = s.orbits?;
                if o.progress >= 1.0 {
                    return Some(0.0);
                }
                (o.seconds > 0.3 && o.progress > 0.0).then(|| (1.0 - o.progress) * o.seconds / o.progress)
            }
            Stage::AntiAlias => {
                let full = self.full.unwrap_or_else(|| self.level_age(now));
                if s.level_aa.is_none() || pixels_left(s) == 0 {
                    // Between samples (the fade-in, the AA list building): the samples to come.
                    return Some(self.aa_remaining(s, full, wall_per_gpu).max(0.1));
                }
                // (A sample's first moments: as long as the last one, or the predicted one.)
                let level = self.level_remaining(s, now).unwrap_or_else(|| {
                    let predicted = self.aa_sample.unwrap_or(full * aa_share(s));
                    (predicted - self.level_age(now)).max(0.05)
                });
                let sample = self.level_age(now) + level;
                let spent = s.aa_gpu + s.level_gpu + level / wall_per_gpu;
                let next = s.aa_composed + 2; // the sample after this one
                let more = if next >= s.aa_target || spent > s.aa_budget {
                    0.0
                } else {
                    let gpu = (sample / wall_per_gpu).max(1e-3);
                    let n = (((s.aa_budget - spent) / gpu).floor() + 1.0).min(f64::from(s.aa_target - next));
                    n * sample
                };
                Some(level + more)
            }
        }
    }

    /// Seconds since the current level's pixels started (0 before).
    fn pixels_age(&self, now: f64) -> f64 {
        self.level
            .as_ref()
            .and_then(|l| l.pixels_since)
            .map_or(0.0, |t| now - t)
    }

    fn level_age(&self, now: f64) -> f64 {
        self.level.as_ref().map_or(0.0, |l| now - l.start)
    }

    /// The stages after a full level of `full` seconds: extension passes and AA samples.
    fn after_full(&self, s: &ProgressSample, full: f64, wall_per_gpu: f64) -> f64 {
        let ext = if s.extend_to > s.soft_limit && s.level_crossed > 0 {
            // Bounded by the passes' budget; a pass costs up to 4x what its pixels took.
            (s.extension_budget * wall_per_gpu).min(4.0 * full)
        } else {
            0.0
        };
        ext + self.aa_remaining(s, full, wall_per_gpu)
    }

    /// AA samples still to come after a full level of `full` seconds: each costs the full
    /// level's time times the listed share (the last sample's time once one finished); they
    /// run until their GPU time passes the budget.
    fn aa_remaining(&self, s: &ProgressSample, full: f64, wall_per_gpu: f64) -> f64 {
        if s.aa_target <= 1 || s.aa_composed + 1 >= s.aa_target {
            return 0.0;
        }
        let sample = self.aa_sample.unwrap_or(full * aa_share(s));
        if sample <= 0.0 {
            return 0.0;
        }
        let gpu = sample / wall_per_gpu;
        let left = (s.aa_budget - s.aa_gpu).max(0.0);
        let n = ((left / gpu).floor() + 1.0).min(f64::from(s.aa_target - 1 - s.aa_composed));
        n * sample
    }

    /// The current level's remaining seconds: pixels left over their recent resolve rate,
    /// and, once every pixel is admitted, the youngest pixel's steps to the limit over its
    /// recent speed (an upper bound: pixels speed up as others finish). The resolve rate is
    /// right while most pixels finish, the step bound when the last slow ones decide: the
    /// estimate is their geometric mean, never more than the bound.
    fn level_remaining(&self, s: &ProgressSample, now: f64) -> Option<f64> {
        let level = self.level.as_ref()?;
        let &(_, left, n) = level.history.back()?;
        if left <= 0.0 {
            return Some(0.0);
        }
        let window = (0.35 * (now - level.start)).clamp(0.75, 20.0);
        let old = level.history.iter().find(|h| now - h.0 <= window)?;
        let dt = now - old.0;
        if dt < 0.3 {
            return None;
        }
        let rate = (old.1 - left) / dt;
        let by_pixels = if rate > 0.0 { left / rate } else { f64::INFINITY };
        // (From the first entry with a step count: admission may have ended in the window.)
        let by_steps = match (level.history.iter().find(|h| now - h.0 <= window && h.2.is_some()), n) {
            (Some(&(t, _, Some(a))), Some(b)) if b > a && now - t >= 0.15 => {
                (s.level_limit as f64 - b).max(0.0) / ((b - a) / (now - t))
            }
            _ => f64::INFINITY,
        };
        let r = if by_steps.is_finite() {
            (by_pixels.min(by_steps) * by_steps).sqrt()
        } else {
            by_pixels
        };
        r.is_finite().then_some(r)
    }

    fn update_eta(&mut self, now: f64) {
        let Some(r) = self.remaining.filter(|r| r.is_finite()) else {
            self.finishes.clear();
            self.finish = None;
            self.eta_shown = false;
            return;
        };
        let raw = now + r;
        let dt = self.finishes.back().map_or(0.0, |&(t, _)| now - t);
        // (Longer waits smooth over longer: a minute-long estimate moves by seconds per second.)
        // (Smoothed only while shown: an estimate that was not shown leaves no trail.)
        self.finish = Some(match self.finish.filter(|_| self.eta_shown) {
            Some(f) => {
                let tau = (0.1 * (f - now)).clamp(ETA_SMOOTH, 4.0 * ETA_SMOOTH);
                f + (raw - f) * (1.0 - (-dt / tau).exp())
            }
            None => raw,
        });
        self.finishes.push_back((now, raw));
        while self.finishes.front().is_some_and(|&(t, _)| now - t > ETA_WINDOW) {
            self.finishes.pop_front();
        }
        let span = self.finishes.back().map_or(0.0, |b| b.0) - self.finishes.front().map_or(0.0, |f| f.0);
        let (lo, hi) = self
            .finishes
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &(_, f)| {
                (lo.min(f), hi.max(f))
            });
        let left = (self.finish.unwrap_or(raw) - now).max(1.0);
        let spread = (hi - lo) / left;
        // (And the level's rates measured over a while: a level's first seconds, right after
        // its seeds, under-read its pace; an early "~50 min" turned out 5.)
        let shown_long = self.shown_since.is_some_and(|t| now - t >= ETA_AFTER)
            && (self.pixels_age(now) >= ETA_LEVEL_AGE || left < 2.0 * ETA_LEVEL_AGE);
        if !self.eta_shown {
            self.eta_shown = shown_long && span >= 0.6 * ETA_WINDOW && spread < ETA_SPREAD;
        } else if spread > 2.0 * ETA_SPREAD {
            self.eta_shown = false;
        }
    }

    fn describe(&self, s: &ProgressSample, alpha: f64) -> RenderProgress {
        let visible = self.shown_since.is_some();
        let all_in = s.level_total > 0 && s.level_admitted >= s.level_total;
        let seeding = s.stage == Stage::Full && s.seeding.is_some() && s.level_admitted == 0;
        let steps = self
            .level
            .as_ref()
            .and_then(|l| l.n_max)
            .filter(|_| all_in && s.level_live > 0 && s.stage != Stage::Idle)
            .map(|n| (n, s.level_limit as u64));
        let left = pixels_left(s);
        let pixels_left = match s.stage {
            _ if left == 0 => None,
            Stage::AntiAlias if s.level_aa.is_none() => None,
            Stage::Preview | Stage::AntiAlias if s.level_admitted > 0 => {
                Some((left as f64 / s.level_total.max(1) as f64, left))
            }
            Stage::Full | Stage::Extension | Stage::Further if s.level_admitted > 0 && s.full_total > 0 => {
                Some((left as f64 / s.full_total as f64, left))
            }
            _ => None,
        };
        let aa_sample = s
            .level_aa
            .filter(|_| s.stage == Stage::AntiAlias && s.aa_target > 1)
            .map(|n| (n, s.aa_target - 1));
        RenderProgress {
            visible,
            alpha,
            fraction: self.fraction,
            stage: s.stage,
            seeding,
            steps,
            pixels_left,
            aa_sample,
            eta: self
                .finish
                .filter(|_| self.eta_shown && visible)
                .map(|f| (f - self.last_now).max(1.0)),
            unresolved: s.unresolved.map(|u| (u, s.full_total)),
            not_finished: not_finished(s),
            orbits: s.orbits,
            orbits_paused: None,
        }
    }
}

/// Compute further's state for the HUD: while it runs, and once the settled view still has at
/// least `finished_share` of its texels unresolved (hidden while the view moves).
fn not_finished(s: &ProgressSample) -> Option<NotFinished> {
    if s.interacting || s.full_texels == 0 {
        return None;
    }
    let f = s.further?;
    let share = f.unresolved as f64 / s.full_texels as f64;
    if !f.running && (share < s.finished_share || f.unresolved == 0) {
        return None;
    }
    Some(NotFinished {
        share,
        reached: f.reached as u64,
        limit: f.limit as u64,
        why: f.stop.and_then(|w| match w {
            crate::scheduler::FurtherStop::Limit => Some("upper limit"),
            crate::scheduler::FurtherStop::Budget => Some("time limit"),
            crate::scheduler::FurtherStop::Reach => Some("orbit length"),
            _ => None,
        }),
    })
}

/// The share of the view an AA sample computes (the adaptive list), a guess until it is built.
fn aa_share(s: &ProgressSample) -> f64 {
    match (s.aa_pixels, s.full_total) {
        (Some(n), t) if t > 0 => n as f64 / t as f64,
        _ => AA_SHARE_GUESS,
    }
}

/// Pixels of the current level not finished: not admitted yet, or still live.
fn pixels_left(s: &ProgressSample) -> usize {
    s.level_total.saturating_sub(s.level_admitted) + s.level_live
}

// MARK: The line

/// Height of the line (view points).
const LINE_HEIGHT: f64 = 2.0;

/// Draws the progress line along the top edge of the canvas (below the toolbar: `insets` are
/// the canvas insets in view points, top, left, bottom, right) into a `size` pixel target, and
/// appends what it drew to `key`. Nothing while hidden.
pub fn draw_line(
    scene: &mut Scene,
    size: (usize, usize),
    ppp: f64,
    insets: (f64, f64, f64, f64),
    p: &RenderProgress,
    key: &mut String,
) {
    let alpha = p.alpha.clamp(0.0, 1.0) as f32;
    if alpha <= 0.0 || ppp <= 0.0 || size.0 == 0 || size.1 == 0 {
        return;
    }
    let px = 1.0 / ppp;
    let (w, h) = (size.0 as f64, size.1 as f64);
    let lh = (LINE_HEIGHT * px).round().max(1.0);
    let y0 = (insets.0.max(0.0) * px).round().min(h - lh);
    let x0 = (insets.1.max(0.0) * px).round();
    let x1 = (w - insets.3.max(0.0) * px).round();
    if x1 <= x0 {
        return;
    }
    let xe = (x0 + (x1 - x0) * p.fraction.clamp(0.0, 1.0)).round();
    let a = |v: f32| v * alpha;
    scene.set_clip(None);
    // Track: a faint dark band, so the line reads on bright images too.
    scene.rect(
        [x0 as f32, y0 as f32, x1 as f32, (y0 + lh) as f32],
        0.0,
        rgba(0.0, 0.0, 0.0, a(0.3)),
        [0.0; 4],
        0.0,
    );
    if xe > x0 {
        scene.rect(
            [x0 as f32, y0 as f32, xe as f32, (y0 + lh) as f32],
            0.0,
            rgba(0.40, 0.72, 1.0, a(0.95)),
            [0.0; 4],
            0.0,
        );
        // A brighter head over the last 48 points.
        let tail = (48.0 * px).min(xe - x0);
        let yc = y0 + lh / 2.0;
        scene.segment(
            [xe - tail, yc],
            [xe, yc],
            (lh / 2.0) as f32,
            rgba(0.85, 0.95, 1.0, 0.0),
            rgba(0.92, 0.97, 1.0, a(0.95)),
        );
    }
    let q = (alpha * 32.0).round() / 32.0;
    key.push_str(&format!("progress {y0:.0} {x0:.0} {x1:.0} {xe:.0} {q:.3} "));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full level of `total` pixels whose pixels resolve at a constant rate over `secs`
    /// seconds (admission done at a third), then AA off: the samples a 60 Hz refresh sees.
    fn level_at(t: f64, total: usize, secs: f64, limit: usize) -> ProgressSample {
        let k = (t / secs).clamp(0.0, 1.0);
        let resolved = (k * total as f64) as usize;
        let admitted = ((3.0 * k).min(1.0) * total as f64) as usize;
        let live = admitted.saturating_sub(resolved);
        ProgressSample {
            generation: 1,
            stage: if k >= 1.0 { Stage::Idle } else { Stage::Full },
            settled: k >= 1.0,
            level_total: total,
            level_admitted: admitted,
            level_live: live,
            level_limit: limit,
            live_min_n: (live > 0 && admitted == total).then(|| ((k - 1.0 / 3.0) * 1.5 * limit as f64) as u64),
            soft_limit: limit,
            full_total: total,
            aa_target: 1,
            gpu_time: t,
            ..ProgressSample::default()
        }
    }

    fn run(secs: f64, dt: f64, mut each: impl FnMut(f64, &RenderProgress)) -> ProgressTracker {
        let mut p = ProgressTracker::default();
        let steps = ((secs + 1.5) / dt).ceil() as usize;
        for i in 0..steps {
            let t = i as f64 * dt;
            p.update(&level_at(t, 4_000_000, secs, 1_000_000), t);
            each(t, p.current());
        }
        p
    }

    #[test]
    fn quick_views_show_nothing() {
        run(0.4, 1.0 / 60.0, |_, p| {
            assert_eq!(p.alpha, 0.0);
            assert!(!p.visible && p.text().is_empty());
        });
    }

    #[test]
    fn long_views_show_after_half_a_second_and_never_go_back() {
        let mut last = 0.0;
        let mut first = None;
        let mut eta = None;
        run(20.0, 1.0 / 60.0, |t, p| {
            if p.alpha > 0.0 && first.is_none() {
                first = Some(t);
            }
            if p.alpha > 0.0 {
                assert!(p.fraction + 1e-9 >= last, "the line went back at {t}");
                last = p.fraction;
            }
            if t < 20.0 && p.eta.is_some() && eta.is_none() {
                eta = Some(t);
            }
            // Honest: never far ahead of the real share of the time.
            if t < 20.0 && p.alpha > 0.0 {
                assert!(
                    p.fraction <= t / 20.0 + 0.1,
                    "{} ahead of {} at {t}",
                    p.fraction,
                    t / 20.0
                );
            }
        });
        let first = first.expect("shown");
        assert!((SHOW_AFTER..SHOW_AFTER + 0.1).contains(&first), "shown at {first}");
        let eta = eta.expect("an ETA");
        assert!(eta >= SHOW_AFTER + ETA_AFTER, "ETA at {eta}");
        assert!(last > 0.99, "ran to the end: {last}");
    }

    #[test]
    fn input_hides_the_line_at_once() {
        let mut p = ProgressTracker::default();
        let mut t = 0.0;
        for _ in 0..180 {
            p.update(&level_at(t, 1000, 10.0, 1000), t);
            t += 1.0 / 60.0;
        }
        assert!(p.current().alpha > 0.0 && p.current().visible);
        let mut s = level_at(t, 1000, 10.0, 1000);
        s.interacting = true;
        p.update(&s, t);
        assert!(!p.current().visible, "the HUD text goes at once");
        for _ in 0..12 {
            t += 1.0 / 60.0;
            p.update(&s, t);
        }
        assert_eq!(p.current().alpha, 0.0);
        assert!(!p.animating());
    }

    #[test]
    fn formats() {
        assert_eq!(count_text(900.0), "900");
        assert_eq!(count_text(2_100_000.0), "2.1 M");
        assert_eq!(count_text(10_000_000.0), "10 M");
        assert_eq!(count_text(212_345.0), "212 k");
        assert_eq!(count_text(999_999.0), "1 M");
        assert_eq!(eta_text(7.4), "~7 s");
        assert_eq!(eta_text(41.0), "~40 s");
        assert_eq!(eta_text(250.0), "~4 min");
        assert_eq!(left_text(0.03, 120_000), "3.0 % of pixels left");
        assert_eq!(left_text(0.0001, 120), "120 pixels left");
        let p = RenderProgress {
            visible: true,
            stage: Stage::Full,
            steps: Some((2_100_000, 10_000_000)),
            pixels_left: Some((0.03, 120_000)),
            eta: Some(41.0),
            ..RenderProgress::default()
        };
        assert_eq!(
            p.text(),
            "refining · 2.1 M of 10 M steps · 3.0 % of pixels left · ~40 s"
        );
    }

    /// Compute further (kb: techniques/auto-iterations "Compute further"): the HUD while it
    /// runs and once the settled view still has unresolved pixels.
    #[test]
    fn not_finished_texts() {
        let mut p = RenderProgress {
            visible: true,
            stage: Stage::Further,
            steps: Some((1_200_000, 4_000_000)),
            not_finished: Some(NotFinished {
                share: 0.12,
                reached: 1_000_000,
                limit: 4_000_000,
                why: None,
            }),
            ..RenderProgress::default()
        };
        assert_eq!(p.text(), "computing further · 1.2 M of 4 M steps · 12 % not finished");
        p.steps = None;
        assert_eq!(p.text(), "computing further · to 4 M steps · 12 % not finished");
        p.visible = false;
        p.not_finished = Some(NotFinished {
            share: 0.038,
            reached: 200_000,
            limit: 0,
            why: Some("upper limit"),
        });
        assert_eq!(p.text(), "3.8 % not finished at 200 k steps (upper limit)");
        p.not_finished = None;
        assert_eq!(p.text(), "");
        // Settled and finished (below the share): nothing; moving: nothing.
        let mut s = ProgressSample {
            settled: true,
            full_texels: 1_000_000,
            finished_share: 0.001,
            further: Some(FurtherInfo {
                unresolved: 500,
                reached: 200_000,
                ..FurtherInfo::default()
            }),
            ..ProgressSample::default()
        };
        assert_eq!(not_finished(&s), None);
        s.further.as_mut().expect("further").unresolved = 50_000;
        assert!((not_finished(&s).expect("not finished").share - 0.05).abs() < 1e-12);
        s.interacting = true;
        assert_eq!(not_finished(&s), None);
    }

    #[test]
    fn the_line_sits_below_the_top_inset() {
        let p = RenderProgress {
            alpha: 1.0,
            fraction: 0.5,
            ..RenderProgress::default()
        };
        let mut scene = Scene::default();
        let mut key = String::new();
        draw_line(&mut scene, (2000, 1000), 0.5, (28.0, 0.0, 0.0, 0.0), &p, &mut key);
        assert!(!scene.prims.is_empty());
        // 28 points below the top at 2 pixels per point; the bar ends half way.
        assert!(key.contains("progress 56 0 2000 1000"), "{key}");
        let mut empty = Scene::default();
        draw_line(
            &mut empty,
            (2000, 1000),
            0.5,
            (28.0, 0.0, 0.0, 0.0),
            &RenderProgress::default(),
            &mut key,
        );
        assert!(empty.prims.is_empty());
    }

    /// BB (kb: plans/buddhabrot "Convergence"): the Buddhabrot's HUD lines while it sharpens,
    /// at a deep view, and paused at its budget.
    #[test]
    fn orbit_lines() {
        use crate::scheduler::OrbitProgress;
        use almondbread_core::buddha::BuddhaMode;
        let o = OrbitProgress {
            mode: BuddhaMode::Buddhabrot,
            progress: 0.404,
            sharp: 0.404,
            seconds: 4.0,
            ..OrbitProgress::default()
        };
        let mut p = RenderProgress {
            visible: true,
            stage: Stage::Orbits,
            orbits: Some(o),
            ..RenderProgress::default()
        };
        assert_eq!(p.text(), "Buddhabrot · sharpening · 40 %");
        p.orbits = Some(OrbitProgress {
            mode: BuddhaMode::Nebulabrot,
            deep: true,
            ..o
        });
        p.eta = Some(20.0);
        assert_eq!(
            p.text(),
            "Nebulabrot · sharpening · 40 % · ~20 s · deep: takes minutes here"
        );
        // Never 100 % before the end.
        p.orbits = Some(OrbitProgress { progress: 0.999, ..o });
        p.eta = None;
        assert_eq!(p.text(), "Buddhabrot · sharpening · 99 %");
        let paused = RenderProgress {
            orbits_paused: Some(OrbitProgress {
                stopped: true,
                sharp: 0.12,
                seconds: 64.0,
                ..o
            }),
            ..RenderProgress::default()
        };
        assert_eq!(paused.text(), "Buddhabrot · paused at 12 % after 1 min");
    }
}
