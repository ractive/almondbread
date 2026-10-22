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
/// motion falls back to the escape-time colouring, and above which it comes back (measured: the
/// whole set ~110 on M1 Pro, a seahorse-valley view 0.15 wide ~18; kb: plans/buddhabrot
/// "Live mode").
pub const LIVE_MIN_HITS: f64 = 25.0;
pub const LIVE_RESUME_HITS: f64 = 35.0;
/// Rest: the faintest channel's mean hits per output pixel at which the view is clean and
/// sampling stops (kb: plans/buddhabrot "As built": time to a clean image).
pub const REST_TARGET_HITS: f64 = 1500.0;

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
            shown_levels: 0,
            levels_at: f64::NEG_INFINITY,
        }
    }
}

/// The look's progress for the HUD.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OrbitProgress {
    pub mode: BuddhaMode,
    /// The faintest channel's mean hits per pixel, and the target.
    pub mean_hits: f64,
    pub target: f64,
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
        Some(OrbitProgress {
            mode: r.key.mode,
            mean_hits: r.mean_hits(),
            target: REST_TARGET_HITS,
            samples: r.samples(),
            seconds: self.buddha.rest_done_at.unwrap_or(now) - self.buddha.rest_start,
            metropolis: r.done.steps != r.done.uniform,
        })
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
        // expected hits per live bin (with hysteresis), else the escape-time colouring.
        if interacting {
            if let Some(e) = self.live_hits_estimate() {
                let need = if self.buddha.live_ok {
                    LIVE_MIN_HITS
                } else {
                    LIVE_RESUME_HITS
                };
                self.buddha.live_ok = e >= need;
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
            // Motion starts from the rest image when it is the cleaner one.
            let seed = !self.buddha.was_live
                && b.rest.as_ref().is_some_and(|r| {
                    r.key.mode == mode
                        && r.done.proposals > 0
                        && b.live
                            .as_ref()
                            .is_none_or(|l| l.key.is_none() || r.mean_hits() >= l.hits_per_bin / 2.0)
                });
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
        let done = mean >= REST_TARGET_HITS || (self.thermal.pauses_refinement() && r.done.proposals > 0);
        if done {
            self.buddha.rest_done_at.get_or_insert(now);
        } else {
            while b.in_flight() < 2 {
                b.sample_rest(&self.backend, self.config.chunk_target)?;
            }
        }
        // The tone curve's levels of the growing histogram, four times a second (and once more
        // when it is done).
        if now - self.buddha.levels_at >= 0.25
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
        } else if r.done.proposals == 0 {
            // Nothing sampled yet: keep the previous image a moment (one dispatch).
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
        if let Some(show) = want.or_else(|| overlay_stale.then_some(self.buddha.shown)) {
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
