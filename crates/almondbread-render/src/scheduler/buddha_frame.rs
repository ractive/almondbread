//! The Buddhabrot and Nebulabrot looks in the frame loop (milestone BB, kb: plans/buddhabrot).
//!
//! While the look is on (Mandelbrot only, views no deeper than [`buddha::MIN_PIXEL`]) a refresh
//! does not run the escape-time job at all: at rest it adds sample dispatches to the view's
//! histogram (about `chunk_target` of GPU each, two in flight) until the view is clean and
//! presents the tone-mapped histogram as it grows; in motion, with `SchedulerConfig::buddha_live`,
//! each refresh samples a coarse histogram within the gesture budget and folds it into an
//! accumulation that follows the view (kb: plans/buddhabrot "Live mode"). Without the live mode
//! (or past the depth limit) motion shows the escape-time colouring of the same look.
//!
//! Shown images go through the history image like every other frame, so the overlay pass,
//! overlay-only refreshes and `capture_shown` work unchanged.

use almondbread_core::buddha::{self, BinMap, BuddhaMode};
use almondbread_core::formula::FormulaCapabilities;

use super::{FrameOutcome, FrameTarget, HistoryView, PALETTE_SIZE, Renderer};
use crate::backend::{ComputeEncoder, GpuBackend, GpuError, Submission};
use crate::buddha::{Buddha, HistKey};

/// Live mode: one live bin per `LIVE_DIV` x `LIVE_DIV` output pixels.
pub const LIVE_DIV: usize = 3;
/// Live mode: time constant (s) of the accumulation's decay while moving.
pub const LIVE_TAU: f64 = 0.12;
/// Live mode: the accumulation's expected hits per live bin (faintest channel) below which
/// motion falls back to the escape-time colouring for the rest of the gesture (measured: the
/// whole set ~110 on M1 Pro, a seahorse-valley view 0.15 wide ~18; kb: plans/buddhabrot
/// "Live mode").
pub const LIVE_MIN_HITS: f64 = 25.0;
/// ... and the estimate a gesture needs to start in the live mode.
pub const LIVE_RESUME_HITS: f64 = 35.0;
/// Live mode: refreshes the per-frame estimate is averaged over (exponentially).
pub const LIVE_SMOOTH: f64 = 8.0;
/// The faintest channel's mean hits per output pixel judged clean at the whole set by looking
/// (kb: plans/buddhabrot "As built"); the export accumulates to four times this. The resting view
/// stops by measured convergence instead ([`crate::buddha::CLEAN_NOISE`]).
pub const REST_TARGET_HITS: f64 = 1500.0;
/// Views narrower than this (complex units) take minutes to sharpen: the HUD says so.
pub const DEEP_WIDTH: f64 = 0.1;

/// What the last presented frame showed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuddhaShown {
    /// Not the look (escape-time colouring).
    #[default]
    None,
    /// The live accumulation (motion, or after it until the rest histogram is as clean).
    Live,
    /// The rest histogram, with this many completed proposals.
    Rest(u64),
}

/// The look's state in the renderer.
pub(super) struct BuddhaState<B: GpuBackend> {
    pub sampler: Option<Buddha<B>>,
    /// The kernels did not compile: the look shows the escape-time colouring (reported once).
    pub failed: bool,
    pub shown: BuddhaShown,
    /// The palette the shown frame was toned with.
    pub shown_palette: String,
    /// The rest view's sampling started at (refresh time).
    pub rest_start: f64,
    /// The rest histogram reached its target (the progress readout's end).
    pub rest_done_at: Option<f64>,
    /// The view is settled: clean, nothing in flight, the last state presented.
    pub settled: bool,
    /// The last refresh was a live (motion) frame.
    pub was_live: bool,
    /// Whether the look is active for the current view (the progress sample and `is_settled`).
    pub active: bool,
    /// Motion keeps the look (the live accumulation is clean enough, see [`LIVE_MIN_HITS`]).
    pub live_ok: bool,
    /// A gesture is running (`live_ok` was decided for it), and its live frames so far (the
    /// live estimate is this gesture's after a few).
    pub in_gesture: bool,
    pub gesture_live_frames: u32,
    /// The gesture's smoothed live estimate (hits per live bin).
    pub live_estimate: Option<f64>,
    /// The last refresh showed the look (frame statistics: switches).
    pub was_look: bool,
    /// The levels the shown frame was toned with, and when levels were last asked for.
    pub shown_levels: u64,
    pub levels_at: f64,
}

impl<B: GpuBackend> Default for BuddhaState<B> {
    fn default() -> Self {
        Self {
            sampler: None,
            failed: false,
            shown: BuddhaShown::None,
            shown_palette: String::new(),
            rest_start: 0.0,
            rest_done_at: None,
            settled: false,
            was_live: false,
            active: false,
            live_ok: true,
            in_gesture: false,
            gesture_live_frames: 0,
            live_estimate: None,
            was_look: false,
            shown_levels: 0,
            levels_at: f64::NEG_INFINITY,
        }
    }
}

/// The look's progress for the HUD.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OrbitProgress {
    pub mode: BuddhaMode,
    /// The faintest channel's mean hits per pixel, and the hits the estimated convergence
    /// needs (before the first estimate, [`REST_TARGET_HITS`]).
    pub mean_hits: f64,
    pub target: f64,
    /// Progress to the end of sampling, 0..1 (convergence or the GPU budget, whichever first;
    /// `Rest::progress`).
    pub progress: f64,
    /// The share of the hits convergence needs, 0..1 (`Rest::sharpness`).
    pub sharp: f64,
    /// The shown image's estimated noise (mean 8-bit levels) once measured.
    pub noise: Option<f64>,
    /// Sampling stopped at its GPU budget before converging ("Continue" resumes).
    pub stopped: bool,
    /// The view is narrower than [`DEEP_WIDTH`].
    pub deep: bool,
    /// Uniform-equivalent samples of `c` so far.
    pub samples: f64,
    /// Seconds since the view's sampling started.
    pub seconds: f64,
    /// Metropolis–Hastings sampling (small views).
    pub metropolis: bool,
}

impl<B: GpuBackend> Renderer<B> {
    /// The orbit-density look this frame would show (off: none, or the formula has none).
    pub fn buddha_mode(&self) -> BuddhaMode {
        if FormulaCapabilities::of(self.settings.formula).buddhabrot {
            self.settings.buddha
        } else {
            BuddhaMode::Off
        }
    }

    /// The Buddhabrot sampler (tests, the CLI's measurements).
    pub fn buddha_sampler(&mut self) -> Option<&mut Buddha<B>> {
        self.buddha.sampler.as_mut()
    }

    /// The look's progress for the HUD (None: the look is not showing).
    pub fn orbit_progress(&self, now: f64) -> Option<OrbitProgress> {
        if !self.buddha.active {
            return None;
        }
        let r = self.buddha.sampler.as_ref()?.rest.as_ref()?;
        let m = &r.key.map;
        let target = match r.noise {
            Some(n) if n > 0.0 => r.mean_hits() * (n / crate::buddha::CLEAN_NOISE).powi(2),
            _ => REST_TARGET_HITS,
        };
        Some(OrbitProgress {
            mode: r.key.mode,
            mean_hits: r.mean_hits(),
            target,
            progress: r.progress(),
            sharp: r.sharpness(),
            noise: r.noise,
            stopped: r.out_of_budget() && !r.converged(),
            deep: (m.pixel * m.width as f64) < DEEP_WIDTH,
            samples: r.samples(),
            seconds: self.buddha.rest_done_at.unwrap_or(now) - self.buddha.rest_start,
            metropolis: r.done.steps != r.done.uniform,
        })
    }

    /// "Continue" after the resting view stopped at its GPU budget unconverged: as much again.
    /// Returns whether it resumed.
    pub fn continue_buddha(&mut self) -> bool {
        if !self.buddha.active {
            return false;
        }
        let Some(r) = self.buddha.sampler.as_mut().and_then(|b| b.rest.as_mut()) else {
            return false;
        };
        if r.converged() || !r.out_of_budget() {
            return false;
        }
        r.budget = r.done.gpu_time + crate::buddha::REST_BUDGET;
        self.buddha.rest_done_at = None;
        self.buddha.settled = false;
        true
    }

    /// What the last presented frame showed (the look's live or rest image, or the escape-time
    /// colouring) and the live mode's current estimate (measurements: `buddha --live`).
    pub fn buddha_shown(&self) -> (BuddhaShown, Option<f64>) {
        (self.buddha.shown, self.buddha.live_estimate)
    }

    /// The live accumulation's expected hits per live bin at equilibrium (faintest channel): from
    /// the last live frames while moving, else from the rest histogram's rate (None: unknown).
    fn live_hits_estimate(&self) -> Option<f64> {
        let b = self.buddha.sampler.as_ref()?;
        let decay = (-self.refresh_interval.max(0.004) / LIVE_TAU).exp();
        if self.buddha.was_live
            && let Some(l) = &b.live
            && l.frames > 2
        {
            let ch = l.mode.channels().max(1);
            let per_frame =
                (0..ch).map(|k| b.last_live.hits[k]).min().unwrap_or(0) as f64 / (l.width * l.height) as f64;
            return Some(per_frame / (1.0 - decay));
        }
        let r = b.rest.as_ref()?;
        (r.done.gpu_time > 0.05).then(|| {
            let per_second = r.mean_hits() / r.done.gpu_time;
            per_second * 0.8 * self.gesture_budget() * (LIVE_DIV * LIVE_DIV) as f64 / (1.0 - decay)
        })
    }

    /// A gesture can start in the live mode from a clean enough image of this view: the rest
    /// histogram (summed into live bins) or the live accumulation of this view has at least
    /// [`LIVE_MIN_HITS`] per live bin. (Starting from less shows black or speckle at once.)
    fn live_start_clean(&self, map: &BinMap, mode: BuddhaMode) -> bool {
        let Some(b) = self.buddha.sampler.as_ref() else {
            return false;
        };
        let div = (LIVE_DIV * LIVE_DIV) as f64;
        // (The gesture's first refresh has moved already: the image is the one of the view
        // just before, reprojected.)
        let rest = b
            .rest
            .as_ref()
            .is_some_and(|r| r.key.mode == mode && near(&r.key.map, map, 1.0) && r.mean_hits() * div >= LIVE_MIN_HITS);
        let live = b.live.as_ref().is_some_and(|l| {
            l.key
                .as_ref()
                .is_some_and(|k| k.mode == mode && near(&k.map, map, LIVE_DIV as f64))
                && l.hits_per_bin >= LIVE_MIN_HITS
        });
        rest || live
    }

    /// Leaves the look (off, another formula, too deep, or motion without the live mode): the
    /// escape-time path takes over; its gesture frames must not reproject the look's image.
    fn leave_buddha(&mut self, release: bool) {
        if self.buddha.shown != BuddhaShown::None {
            self.history_view = None;
            self.buddha.shown = BuddhaShown::None;
        }
        self.buddha.active = false;
        self.buddha.settled = false;
        self.buddha.was_live = false;
        if release && let Some(b) = &mut self.buddha.sampler {
            b.wait_all(&self.backend);
            b.release();
        }
    }

    /// One refresh with the look on; None: this refresh shows the escape-time colouring (the
    /// caller goes on with its job).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn buddha_frame(
        &mut self,
        now: f64,
        width: usize,
        height: usize,
        points_per_pixel: f64,
        interacting: bool,
        keep_going: bool,
        acquire: &mut dyn FnMut(bool) -> Option<FrameTarget<B>>,
    ) -> Option<FrameOutcome> {
        let on = self.buddha_mode().is_on();
        let o = self.buddha_frame_look(now, width, height, points_per_pixel, interacting, keep_going, acquire);
        let look = o.is_some();
        if on
            && look != self.buddha.was_look
            && let Some(fs) = &mut self.frame_stats
        {
            fs.look_switches += 1;
        }
        self.buddha.was_look = look;
        o
    }

    #[allow(clippy::too_many_arguments)]
    fn buddha_frame_look(
        &mut self,
        now: f64,
        width: usize,
        height: usize,
        points_per_pixel: f64,
        interacting: bool,
        keep_going: bool,
        acquire: &mut dyn FnMut(bool) -> Option<FrameTarget<B>>,
    ) -> Option<FrameOutcome> {
        if !interacting {
            self.buddha.in_gesture = false;
        }
        let mode = self.buddha_mode();
        if !mode.is_on() || self.buddha.failed {
            if self.buddha.active || self.buddha.sampler.as_ref().is_some_and(|b| b.bytes() > 0) {
                self.leave_buddha(true);
            }
            return None;
        }
        let map = BinMap::new(&self.viewport, width, height, points_per_pixel);
        if !map.supported() {
            self.leave_buddha(true);
            return None;
        }
        if interacting && !self.config.buddha_live {
            self.leave_buddha(false);
            return None;
        }
        if self.buddha.sampler.is_none() {
            match Buddha::new(&self.backend) {
                Ok(b) => self.buddha.sampler = Some(b),
                Err(e) => {
                    eprintln!("almondbread: Buddhabrot kernels: {e}");
                    self.buddha.failed = true;
                    return None;
                }
            }
        }
        // Motion keeps the look only where the live accumulation is more than noise: its
        // expected hits per live bin, else the escape-time colouring. Decided once when the
        // gesture starts (from the rest histogram's rate) and revised only towards the
        // fallback: a gesture that fell back stays there until it ends. (Re-deciding every
        // refresh flipped the look and the fallback frame by frame: after a fallback the
        // estimate came from the rest histogram's rate, stale and higher than the live one, so
        // the next refresh went live again; measured 1000 flips in a 10 s zoom; kb:
        // plans/buddhabrot "Live mode".)
        if interacting {
            if !self.buddha.in_gesture {
                self.buddha.in_gesture = true;
                self.buddha.gesture_live_frames = 0;
                self.buddha.live_estimate = self.live_hits_estimate();
                // (Unknown rate: as the last gesture decided.)
                let rate_ok = self
                    .buddha
                    .live_estimate
                    .map_or(self.buddha.live_ok, |e| e >= LIVE_RESUME_HITS);
                self.buddha.live_ok = rate_ok && self.live_start_clean(&map, mode);
            } else if self.buddha.live_ok
                && self.buddha.gesture_live_frames > 2
                && let Some(e) = self.live_hits_estimate()
            {
                // (Smoothed over about LIVE_SMOOTH refreshes: one frame's hits vary with what
                // else the GPU runs; a single slow frame must not end the live mode.)
                let k = 1.0 / LIVE_SMOOTH;
                let s = self.buddha.live_estimate.map_or(e, |p| (1.0 - k) * p + k * e);
                self.buddha.live_estimate = Some(s);
                self.buddha.live_ok = s >= LIVE_MIN_HITS;
            }
            if !self.buddha.live_ok {
                self.leave_buddha(false);
                return None;
            }
        }
        let mut b = self.buddha.sampler.take().expect("sampler");
        let gpu_before = b.harvested_gpu;
        let result = self.buddha_frame_with(&mut b, now, map, mode, interacting, keep_going, acquire);
        // The sample dispatches' GPU time in the frame statistics (as iterate chunks).
        let gpu = b.harvested_gpu - gpu_before;
        if let Some(fs) = &mut self.frame_stats {
            fs.chunk_gpu += gpu;
            if interacting {
                fs.preview_chunk_gpu += gpu;
            }
        }
        self.buddha.sampler = Some(b);
        match result {
            Ok(o) => {
                self.buddha.active = true;
                self.update_progress(now);
                Some(o)
            }
            Err(e) => {
                eprintln!("almondbread: Buddhabrot frame: {e}");
                self.buddha.failed = true;
                self.leave_buddha(true);
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn buddha_frame_with(
        &mut self,
        b: &mut Buddha<B>,
        now: f64,
        map: BinMap,
        mode: BuddhaMode,
        interacting: bool,
        keep_going: bool,
        acquire: &mut dyn FnMut(bool) -> Option<FrameTarget<B>>,
    ) -> Result<FrameOutcome, GpuError> {
        let mut outcome = FrameOutcome {
            animating: true,
            presented: false,
        };
        b.harvest(&self.backend);
        let palette_key = self.palette_key();
        b.ensure_palette(&self.backend, &palette_key, &self.settings.palette_table(PALETTE_SIZE))?;
        let out = (map.width, map.height);
        self.ensure_history(out.0, out.1)?;
        let key = HistKey { map, mode };
        if interacting {
            // Live: a coarse histogram per refresh within the gesture budget.
            let (lw, lh) = (map.width.div_ceil(LIVE_DIV), map.height.div_ceil(LIVE_DIV));
            let lkey = HistKey {
                map: BinMap {
                    pixel: map.pixel * LIVE_DIV as f64,
                    width: lw,
                    height: lh,
                    ..map
                },
                mode,
            };
            b.ensure_live(&self.backend, mode, lw, lh)?;
            let Some((target, drawable)) = acquire(false) else {
                if let Some(fs) = &mut self.frame_stats {
                    fs.skipped += 1;
                    fs.skipped_no_target += 1;
                }
                return Ok(outcome);
            };
            // Motion starts from the rest image, unless the live accumulation of this very view
            // (a gesture that just ended here) is the cleaner one. Anything else would reproject
            // an accumulation of another view (black where it does not reach).
            let entering = !self.buddha.was_live;
            let live_here = b
                .live
                .as_ref()
                .filter(|l| {
                    l.key
                        .as_ref()
                        .is_some_and(|k| k.mode == mode && near(&k.map, &map, LIVE_DIV as f64))
                })
                .map(|l| l.hits_per_bin);
            let seed = entering
                && b.rest.as_ref().is_some_and(|r| {
                    r.key.mode == mode
                        && near(&r.key.map, &map, 1.0)
                        && r.done.proposals > 0
                        && live_here.is_none_or(|h| r.mean_hits() * (LIVE_DIV * LIVE_DIV) as f64 >= h)
                });
            if entering && !seed && live_here.is_none() {
                b.reset_live();
            }
            let budget = 0.8 * self.gesture_budget();
            b.sample_live(&self.backend, &lkey, budget)?;
            let mut enc = self.backend.begin("buddha live")?;
            if seed {
                let r = b.rest.as_ref().expect("rest");
                let skey = HistKey {
                    map: BinMap {
                        pixel: r.key.map.pixel * LIVE_DIV as f64,
                        width: lw,
                        height: lh,
                        ..r.key.map
                    },
                    mode,
                };
                b.seed_live(&mut enc, &skey, LIVE_DIV);
            }
            let decay = (-self.refresh_interval.max(0.004) / LIVE_TAU).exp();
            let history = self.history.clone().expect("history");
            let hist_out = history[1 - self.history_ping].clone();
            b.encode_fold_tone(&mut enc, &lkey, decay, &target, &hist_out, out);
            self.present_buddha(enc, &target, drawable.as_ref(), &hist_out, &key, now, true)?;
            self.buddha.shown = BuddhaShown::Live;
            self.buddha.shown_palette = palette_key;
            self.buddha.was_live = true;
            self.buddha.gesture_live_frames += 1;
            self.buddha.settled = false;
            outcome.presented = true;
            return Ok(outcome);
        }
        self.buddha.was_live = false;

        // Rest.
        if b.ensure_rest(&self.backend, &key)? {
            self.buddha.rest_start = now;
            self.buddha.rest_done_at = None;
        }
        let r = b.rest.as_ref().expect("rest");
        let mean = r.mean_hits();
        // Sampling stops once the image no longer changes visibly (its noise, measured by the
        // change checks, is below `CLEAN_NOISE`) or the view's GPU budget is used (kb:
        // plans/buddhabrot "Convergence").
        let done = r.converged() || r.out_of_budget() || (self.thermal.pauses_refinement() && r.done.proposals > 0);
        if done {
            self.buddha.rest_done_at.get_or_insert(now);
        } else {
            while b.in_flight() < 2 {
                b.sample_rest(&self.backend, self.config.chunk_target)?;
            }
            if b.change_due() {
                b.request_change(&self.backend)?;
            }
        }
        // The tone curve's levels of the growing histogram, four times a second, and once more
        // when it is done (then not again: a settled view does no GPU work).
        // (Levels of a young histogram are noise: the first ones from REST_SHOW_HITS on.)
        let show_hits = if done { 0.0 } else { crate::buddha::REST_SHOW_HITS };
        let ready = b.levels_ready(show_hits);
        if (!done && mean >= show_hits && (now - self.buddha.levels_at >= 0.25 || !ready))
            || (done && self.buddha.rest_done_at.is_some_and(|t| self.buddha.levels_at < t))
        {
            b.request_levels(&self.backend)?;
            self.buddha.levels_at = now;
        }
        let r = b.rest.as_ref().expect("rest");
        // The live accumulation of this very view (the motion just ended) is shown until the
        // rest histogram is as clean.
        let live_view = BinMap {
            pixel: map.pixel * LIVE_DIV as f64,
            width: map.width.div_ceil(LIVE_DIV),
            height: map.height.div_ceil(LIVE_DIV),
            ..map
        };
        let live_better = b.live.as_ref().is_some_and(|l| {
            l.key.as_ref().is_some_and(|k| k.map == live_view && k.mode == mode) && l.hits_per_bin > mean
        });
        let want = if live_better {
            (self.buddha.shown != BuddhaShown::Live || self.buddha.shown_palette != palette_key)
                .then_some(BuddhaShown::Live)
        } else if r.done.proposals == 0 || !b.levels_ready(show_hits) {
            // Nothing sampled or measured yet: keep the previous image a moment (a dispatch and
            // a levels pass).
            None
        } else {
            let s = BuddhaShown::Rest(r.done.proposals);
            (self.buddha.shown != s
                || self.buddha.shown_palette != palette_key
                || self.buddha.shown_levels != b.levels_serial)
                .then_some(s)
        };
        let overlay_stale = self.overlay_stale();
        let mut missed = false;
        let redraw = (overlay_stale && self.buddha.shown != BuddhaShown::None).then_some(self.buddha.shown);
        if let Some(show) = want.or(redraw) {
            if self.frames_in_flight.len() < self.config.max_frames_in_flight
                && let Some((target, drawable)) = acquire(false)
            {
                let mut enc = self.backend.begin("buddha tone")?;
                let history = self.history.clone().expect("history");
                let hist_out = history[1 - self.history_ping].clone();
                if show == BuddhaShown::Live {
                    b.encode_tone_live(&mut enc, &target, &hist_out, out);
                } else {
                    b.encode_tone_rest(&mut enc, &target, &hist_out, out);
                }
                self.present_buddha(enc, &target, drawable.as_ref(), &hist_out, &key, now, false)?;
                // The view's first full-resolution image (the interaction test's "final frame").
                if matches!(show, BuddhaShown::Rest(_)) && !matches!(self.buddha.shown, BuddhaShown::Rest(_)) {
                    self.last_final_frame = (now, format!("{}x{}", out.0, out.1));
                }
                self.buddha.shown = show;
                self.buddha.shown_palette = palette_key;
                self.buddha.shown_levels = b.levels_serial;
                outcome.presented = true;
            } else {
                missed = true;
            }
        }
        let latest = !live_better
            && self.buddha.shown == BuddhaShown::Rest(r.done.proposals)
            && self.buddha.shown_levels == b.levels_serial;
        self.buddha.settled = done && b.in_flight() == 0 && !b.levels_pending() && latest && !missed;
        outcome.animating = keep_going || !self.buddha.settled || self.overlay_stale();
        Ok(outcome)
    }

    /// Composites the overlay over the toned image, presents and commits; `hist_out` (the
    /// toned linear image) becomes the history.
    #[allow(clippy::too_many_arguments)]
    fn present_buddha(
        &mut self,
        mut enc: B::Encoder,
        target: &B::Texture,
        drawable: Option<&B::Drawable>,
        hist_out: &B::Texture,
        key: &HistKey,
        now: f64,
        live: bool,
    ) -> Result<(), GpuError> {
        self.encode_overlay(&mut enc, hist_out, target, false);
        if let Some(d) = drawable {
            enc.present(d);
        }
        let sub = enc.commit();
        self.frames_committed += 1;
        self.frames_in_flight.push((self.frames_committed, sub));
        self.last_frame_commit = now;
        self.has_presented = true;
        self.history_ping = 1 - self.history_ping;
        let colors = self.color_params();
        self.history_view = Some(HistoryView {
            viewport: self.viewport.clone(),
            scale: self.viewport.scale * (key.map.pixel / self.viewport.scale.to_f64()),
            width: key.map.width,
            height: key.map.height,
            colors: (colors, self.palette_key()),
            offset: colors.offset,
        });
        if let Some(fs) = &mut self.frame_stats {
            fs.frames += 1;
            if live {
                fs.preview_frames += 1;
            }
        }
        Ok(())
    }

    /// Offscreen: the look's image of the current view at `width` x `height` (one bin per
    /// pixel), accumulated to the rest target or for `seconds` of wall time, whichever first.
    /// Blocks. BGRA8 sRGB pixels.
    pub fn render_buddha_offscreen(
        &mut self,
        width: usize,
        height: usize,
        seconds: f64,
        target_hits: f64,
    ) -> Result<(Vec<u8>, crate::buddha::Counters), GpuError> {
        use crate::backend::{Storage, TextureFormat};
        let mode = self.buddha_mode();
        if !mode.is_on() {
            return Err(GpuError("the Buddhabrot look is off".into()));
        }
        let map = BinMap::new(&self.viewport, width, height, 1.0);
        if !map.supported() {
            return Err(GpuError(format!(
                "too deep for the Buddhabrot (pixel {:.1e} < {:.1e})",
                map.pixel,
                buddha::MIN_PIXEL
            )));
        }
        // A sampler of its own: the view's histogram stays.
        if self.buddha.sampler.is_none() {
            self.buddha.sampler = Some(Buddha::new(&self.backend)?);
        }
        let mut b = self.buddha.sampler.as_ref().expect("sampler").fresh(&self.backend)?;
        let key = HistKey { map, mode };
        let start = std::time::Instant::now();
        let budget = self.config.chunk_target;
        let res = b.accumulate(&self.backend, &key, budget, |r| {
            r.mean_hits() >= target_hits || start.elapsed().as_secs_f64() >= seconds
        });
        let pixels = res.and_then(|()| {
            b.ensure_palette(
                &self.backend,
                &self.palette_key(),
                &self.settings.palette_table(PALETTE_SIZE),
            )?;
            let target = self
                .backend
                .create_texture(width, height, TextureFormat::Bgra8Unorm, Storage::Shared)?;
            let lin = self
                .backend
                .create_texture(width, height, TextureFormat::Rgba16Float, Storage::Private)?;
            b.request_levels(&self.backend)?;
            b.poll_levels(&self.backend, true);
            let mut enc = self.backend.begin("buddha export")?;
            b.encode_tone_rest(&mut enc, &target, &lin, (width, height));
            if self.overlay.is_some() {
                let out = self
                    .backend
                    .create_texture(width, height, TextureFormat::Bgra8Unorm, Storage::Shared)?;
                self.encode_overlay(&mut enc, &lin, &out, true);
                enc.commit().wait();
                let mut px = vec![0u8; width * height * 4];
                self.backend
                    .read_texture(&out, width, height, TextureFormat::Bgra8Unorm, &mut px)?;
                return Ok(px);
            }
            enc.commit().wait();
            let mut px = vec![0u8; width * height * 4];
            self.backend
                .read_texture(&target, width, height, TextureFormat::Bgra8Unorm, &mut px)?;
            Ok(px)
        });
        let counters = b.rest.as_ref().map(|r| r.done).unwrap_or_default();
        pixels.map(|p| (p, counters))
    }
}

/// `a` (bins of `div` output pixels) shows about the view of `b` (one refresh of motion apart):
/// the same grid in output pixels, centres less than a tenth of the view apart, scales within
/// 10 %.
fn near(a: &BinMap, b: &BinMap, div: f64) -> bool {
    let pixel = a.pixel / div;
    let width = b.pixel * b.width as f64;
    let d = ((a.center[0] - b.center[0]).powi(2) + (a.center[1] - b.center[1]).powi(2)).sqrt();
    (a.width as f64 - b.width as f64 / div).abs() <= 1.0 && d <= 0.1 * width && (pixel / b.pixel - 1.0).abs() <= 0.1
}

#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use super::*;
    use crate::backend::{Storage, TextureFormat};
    use crate::metal::MetalBackend;
    use almondbread_core::viewport::Viewport;

    /// The flicker the user saw (kb: plans/buddhabrot "Live mode"): a steady zoom from the whole
    /// set through the live mode's limit shows the look, then at most one switch to the
    /// escape-time colouring, never back and forth (before the fix: every refresh).
    #[test]
    fn zoom_switches_mode_at_most_once() {
        let (w, h) = (1280, 800);
        let b = MetalBackend::new().expect("metal");
        let mut r = Renderer::new(b.clone()).expect("renderer");
        r.settings.buddha = BuddhaMode::Buddhabrot;
        r.viewport = Viewport::new(-0.75, 0.1, 3.2 / w as f64);
        let target = b
            .create_texture(w, h, TextureFormat::Bgra8Unorm, Storage::Shared)
            .unwrap();
        let mut acquire = |_: bool| -> Option<FrameTarget<MetalBackend>> { Some((target.clone(), None)) };
        let dt = 1.0 / 120.0;
        let mut now = 1.0;
        // Rest until clean (or 20 s), as the app does before a gesture.
        let start = std::time::Instant::now();
        while !r.is_settled() && start.elapsed().as_secs_f64() < 20.0 {
            r.render_frame(now, w, h, 1.0, &mut acquire);
            now += dt;
            std::thread::sleep(std::time::Duration::from_secs_f64(dt));
        }
        let mut shown = Vec::new();
        // 4 s at 2 octaves per second: 3.2 wide to 0.2 wide.
        for _ in 0..480 {
            r.viewport.zoom_by(2f64.powf(2.0 * dt), 0.0, 0.0);
            r.note_interaction(now);
            r.render_frame(now, w, h, 1.0, &mut acquire);
            let live = r.buddha_shown().0 == BuddhaShown::Live;
            if shown.last() != Some(&live) {
                shown.push(live);
            }
            now += dt;
            std::thread::sleep(std::time::Duration::from_secs_f64(dt));
        }
        r.finish_frames();
        println!("zoom: shown segments {shown:?}");
        assert!(shown.len() <= 2, "the look and the fallback alternated: {shown:?}");
        assert!(shown.len() == 1 || (shown[0] && !shown[1]), "{shown:?}");
    }
}
