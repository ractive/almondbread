//! The flight benchmark in the app's window (mode B, kb: synthesis/flight-benchmark): the
//! benchmark flight played live through the real per-refresh path. Each `frame` feeds the
//! script's events due by now into the host (as decoded input: input-to-photon latency is
//! measured), then records the refresh, and the drawables' presentation times. Warm-up laps
//! first, then the measured ones; the report is JSON for `almondbread benchmark --live`.

use almondbread_core::flight::{FlightPlayer, FlightScript, LiveRecorder, apply_json_settings, start_viewport};
use almondbread_core::viewport::Viewport;
use almondbread_render::Host;
use almondbread_render::metal::{MetalBackend, MetalLayerSurface, PresentationLog};
use serde_json::{Value, json};

enum Phase {
    /// The start view renders and settles before a lap (at most `SETTLE_MAX` s).
    Settle {
        since: f64,
        placed: bool,
    },
    Fly {
        start: f64,
        rec: Box<LiveRecorder>,
    },
    Done,
}

const SETTLE_MAX: f64 = 15.0;

pub struct FlightBench {
    script: FlightScript,
    hz: f64,
    laps: usize,
    warmup: usize,
    lap: usize,
    player: FlightPlayer,
    phase: Phase,
    measured: Vec<Value>,
    warmups: Vec<Value>,
    prev: Option<Viewport>,
    t_prev: f64,
    last_now: f64,
    pub report: Option<String>,
}

impl FlightBench {
    pub fn new(script: FlightScript, hz: f64, laps: usize, warmup: usize) -> Self {
        Self {
            script,
            hz: if hz > 0.0 { hz } else { 60.0 },
            laps: laps.max(1),
            warmup,
            lap: 0,
            player: FlightPlayer::default(),
            phase: Phase::Settle {
                since: f64::NAN,
                placed: false,
            },
            measured: Vec::new(),
            warmups: Vec::new(),
            prev: None,
            t_prev: -1.0,
            last_now: f64::NAN,
            report: None,
        }
    }

    pub fn done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    /// A line for the window test's log.
    pub fn progress(&self) -> String {
        match &self.phase {
            Phase::Settle { .. } => format!("lap {} of {}: settling", self.lap + 1, self.laps + self.warmup),
            Phase::Fly { .. } => format!(
                "lap {} of {}{}: t {:.1} s",
                self.lap + 1,
                self.laps + self.warmup,
                if self.lap < self.warmup { " (warm-up)" } else { "" },
                self.t_prev.max(0.0)
            ),
            Phase::Done => "done".into(),
        }
    }

    /// A drawable reached the screen at `time`.
    pub fn presented(&mut self, time: f64) {
        if let Phase::Fly { rec, .. } = &mut self.phase {
            rec.presented(time);
        }
    }

    /// Before the host's frame: place the start view, wait for it to settle, start a lap,
    /// feed the events due.
    pub fn before(
        &mut self,
        host: &mut Host<MetalBackend>,
        surface: &mut MetalLayerSurface,
        log: &mut Option<PresentationLog>,
        now: f64,
    ) {
        let (w_pts, h_pts) = host.view_points();
        let unit = if self.script.size.0 > 0 {
            w_pts / self.script.size.0 as f64
        } else {
            1.0
        };
        match &mut self.phase {
            Phase::Settle { since, placed } => {
                if !*placed {
                    *placed = true;
                    *since = now;
                    let r = &mut host.renderer;
                    r.navigator.cancel_flight();
                    r.navigator.release_all();
                    if let Some(s) = &self.script.settings {
                        apply_json_settings(s, &mut r.settings);
                    }
                    match start_viewport(self.script.start.as_ref(), w_pts, h_pts, unit) {
                        Ok(v) => r.viewport = v,
                        Err(e) => eprintln!("flight bench: {e}"),
                    }
                    return;
                }
                let settled = host.renderer.is_settled() && !host.renderer.retry_pending();
                if settled || now - *since > SETTLE_MAX {
                    self.player = FlightPlayer::new(unit);
                    self.prev = Some(host.renderer.viewport.clone());
                    self.t_prev = -1.0 / self.hz;
                    self.last_now = now;
                    let l = PresentationLog::default();
                    surface.set_presentation_log(Some(l.clone()));
                    *log = Some(l);
                    host.begin_measurement(now);
                    self.phase = Phase::Fly {
                        start: now,
                        rec: Box::new(LiveRecorder::new(&self.script, self.hz, now)),
                    };
                    self.feed(host, now);
                }
            }
            Phase::Fly { .. } => self.feed(host, now),
            Phase::Done => {}
        }
    }

    fn feed(&mut self, host: &mut Host<MetalBackend>, now: f64) {
        let Phase::Fly { start, rec } = &mut self.phase else {
            return;
        };
        let t = now - *start;
        if now - self.last_now > 1.5 / self.hz {
            rec.late_refreshes += 1;
        }
        self.last_now = now;
        if t > self.script.duration {
            return;
        }
        self.player.feed(host, &self.script.events, self.t_prev, t, now);
        if rec.landing_in(self.t_prev, t) {
            host.renderer.first_job_frame = None;
        }
    }

    /// After the host's frame: record the refresh; end the lap after the flight.
    pub fn after(
        &mut self,
        host: &mut Host<MetalBackend>,
        surface: &mut MetalLayerSurface,
        log: &mut Option<PresentationLog>,
        now: f64,
        presented: bool,
    ) {
        let Phase::Fly { start, rec } = &mut self.phase else {
            return;
        };
        let t = now - *start;
        if t <= self.script.duration {
            let moving = self.prev.as_ref() != Some(&host.renderer.viewport);
            if moving {
                self.prev = Some(host.renderer.viewport.clone());
            }
            let r = &host.renderer;
            let settled = r.is_settled() && !r.retry_pending();
            rec.refresh(t, now, moving, presented, settled, r.first_job_frame);
            self.t_prev = t;
            return;
        }
        // Lap over: the last presentation times, the host's latency measurement.
        if let Some(l) = log.take() {
            let shown: Vec<(u64, f64)> = l.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
            for (tag, time) in shown {
                host.on_presented(tag, time);
                rec.presented(time);
            }
        }
        surface.set_presentation_log(None);
        let ir = host.end_measurement(now);
        let mut report = rec.report();
        report["latency_p50_ms"] = json!(ir.latency_p50);
        report["latency_p95_ms"] = json!(ir.latency_p95);
        report["interaction"] = json!({
            "presented": ir.presented, "fps": ir.fps, "interval_p50_ms": ir.interval_p50,
            "interval_p95_ms": ir.interval_p95, "interval_max_ms": ir.interval_max,
            "draw_cpu_p50_ms": ir.draw_cpu_p50, "draw_cpu_p95_ms": ir.draw_cpu_p95,
            "iterate_gpu_ms_per_frame": ir.iterate_gpu_per_frame, "colorize_gpu_ms_per_frame": ir.colorize_gpu_per_frame,
            "preview_pct": ir.preview_pct, "skipped": ir.skipped, "draws": ir.draws, "encoded": ir.encoded,
        });
        let warm = self.lap < self.warmup;
        eprintln!(
            "flight bench: lap {}{}: {:.1} % of due frames shown, p99 {:.1} ms, sharp {:.2} s in all",
            self.lap + 1,
            if warm { " (warm-up)" } else { "" },
            100.0 * report["presented_frac"].as_f64().unwrap_or(f64::NAN),
            report["p99_ms"].as_f64().unwrap_or(f64::NAN),
            report["sharp_total_s"].as_f64().unwrap_or(f64::NAN)
        );
        if warm {
            self.warmups.push(report);
        } else {
            self.measured.push(report);
        }
        self.lap += 1;
        if self.lap < self.laps + self.warmup {
            self.phase = Phase::Settle {
                since: now,
                placed: false,
            };
            return;
        }
        self.phase = Phase::Done;
        let (w, h) = host.pixel_size();
        let (wp, hp) = host.view_points();
        let out = json!({
            "mode": "live window",
            "flight": self.script.benchmark.as_ref().map(|b| json!({"name": b.name, "version": b.version})),
            "hz": self.hz,
            "pixels": format!("{w}x{h}"),
            "view_points": format!("{wp}x{hp}"),
            "laps": self.measured,
            "warmup_laps": self.warmups,
        });
        self.report = Some(serde_json::to_string_pretty(&out).unwrap_or_default());
    }
}
