//! The parameter lab in the frame loop (F10, kb: plans/f10-parameter-lab, `crate::lab`).
//!
//! While the lab is on, a refresh does not run the escape-time job at all: the main view is the
//! chosen plane (c, z₀ or p) of `z ← zᵖ + c`, iterated directly in float32 by `labField` into
//! iteration and DE texels of the same form as the Mandelbrot kernels', and coloured by the main
//! colour pass (`colorize`), so palettes, density, lighting, outlines and the palette filter work
//! as everywhere (interior colourings other than flat need the cycle analysis: flat here).
//!
//! - Moving (a gesture, or the knobs or the view changed within [`REST`]): one field per
//!   refresh at the resolution the frame budget affords (the GPU time per texel measured on
//!   the last fields), at least a quarter of the output's; a reduced field is coloured at its
//!   own size and upsampled bilinearly (the preview path of the escape-time frames).
//! - At rest: the full-resolution field, then (supersampling on) a 2×2 one, each in bands of
//!   rows within the budget, shown when complete.
//! - Zoom stops at the precision limit (`almondbread_core::lab::min_pixel`): a deeper scale is
//!   clamped about the point the zoom was heading for, and the overlay says so.
//!
//! Shown images go through the history image like every other frame, so the overlay pass,
//! overlay-only refreshes and `capture_shown` work unchanged.

use std::collections::VecDeque;

use almondbread_core::lab::{self, LabValues, Plane};
use almondbread_core::settings::Supersampling;
use almondbread_core::viewport::{Center, Scale, Viewport};
use bytemuck::bytes_of;

use super::{FrameOutcome, FrameTarget, HistoryView, OffscreenResult, Renderer};
use crate::backend::{ComputeEncoder, GpuBackend, GpuError, Storage, Submission, SubmissionStatus, TextureFormat};
use crate::gpu_types::ColorParams;
use crate::lab::{LabKernels, LabParams, encode_field, view_params};

/// Seconds without a change (knobs, view, size) before the full-resolution field.
pub const REST: f64 = 0.12;
/// The coarsest moving field: a quarter of the output's resolution.
pub const MIN_PREVIEW: f64 = 0.25;
/// Rest fields larger than this (texels) skip the 2×2 pass (memory: 12 bytes a texel).
pub const MAX_AA_TEXELS: usize = 24_000_000;

/// What a field shows.
#[derive(Clone, Debug, PartialEq)]
struct FieldKey {
    values: LabValues,
    plane: Plane,
    center: (f64, f64),
    scale: f64,
    angle: f64,
    size: (usize, usize),
    max_iter: usize,
}

/// Iteration and DE textures (previews use their top-left part).
struct FieldTextures<B: GpuBackend> {
    iter: B::Texture,
    de: B::Texture,
    size: (usize, usize),
}

/// A rest field drawn in bands of rows.
struct BandJob {
    key: FieldKey,
    /// Texels per output pixel (1, or 2 for 2×2 supersampling).
    ratio: usize,
    params: LabParams,
    next_row: u32,
}

/// The lab's state in the renderer (`Renderer::lab`; None: the lab is off).
pub struct LabState<B: GpuBackend> {
    pub values: LabValues,
    pub plane: Plane,
    /// The view of each plane while another one is shown.
    views: [Option<Viewport>; 3],
    kernels: Option<LabKernels<B>>,
    /// The library did not compile: nothing is drawn (reported once).
    pub failed: bool,
    full: Option<FieldTextures<B>>,
    aa: Option<FieldTextures<B>>,
    preview_color: Option<(B::Texture, (usize, usize))>,
    /// The field key as of the last change, and when it changed.
    changed: Option<(FieldKey, f64)>,
    /// The shown image: its field, texels per pixel (< 1: a preview), and colours.
    shown: Option<(FieldKey, f64, (ColorParams, String))>,
    job: Option<BandJob>,
    /// Field submissions in flight with their texel counts (GPU time per texel).
    pending: VecDeque<(B::Submission, usize)>,
    /// GPU seconds per texel of the recent fields (smoothed; 0: none measured yet).
    pub cost_per_texel: f64,
    /// GPU milliseconds of the last finished field submission.
    pub last_field_ms: f64,
    /// The view of the last refresh (centre, scale per point): the zoom clamp's way back.
    prev_view: Option<((f64, f64), f64)>,
    /// When zooming last ran into the precision limit (the overlay's note).
    pub at_limit: Option<f64>,
    /// Nothing to do: the final image of the current field and colours is shown.
    pub settled: bool,
}

impl<B: GpuBackend> LabState<B> {
    pub fn new(values: LabValues, plane: Plane) -> Self {
        Self {
            values: values.sanitized(),
            plane,
            views: Default::default(),
            kernels: None,
            failed: false,
            full: None,
            aa: None,
            preview_color: None,
            changed: None,
            shown: None,
            job: None,
            pending: VecDeque::new(),
            cost_per_texel: 0.0,
            last_field_ms: 0.0,
            prev_view: None,
            at_limit: None,
            settled: false,
        }
    }

    /// The kernels (compiled on first use; the overlay's panels share them).
    pub fn kernels(&mut self, backend: &B) -> Option<LabKernels<B>> {
        if self.kernels.is_none() && !self.failed {
            match LabKernels::compile(backend) {
                Ok(k) => self.kernels = Some(k),
                Err(e) => {
                    eprintln!("almondbread: lab kernels: {e}");
                    self.failed = true;
                }
            }
        }
        self.kernels.clone()
    }

    fn poll(&mut self) {
        while let Some((sub, texels)) = self.pending.front() {
            match sub.status() {
                SubmissionStatus::Pending => break,
                SubmissionStatus::Completed => {
                    let t = sub.gpu_time();
                    if *texels > 0 && t > 0.0 {
                        let c = t / *texels as f64;
                        self.cost_per_texel = if self.cost_per_texel > 0.0 {
                            0.6 * self.cost_per_texel + 0.4 * c
                        } else {
                            c
                        };
                    }
                    self.last_field_ms = t * 1000.0;
                    self.pending.pop_front();
                }
                SubmissionStatus::Failed(e) => {
                    eprintln!("almondbread: lab field: {e}");
                    self.pending.pop_front();
                    self.shown = None;
                    self.job = None;
                }
            }
        }
    }
}

fn ensure_textures<B: GpuBackend>(
    backend: &B,
    slot: &mut Option<FieldTextures<B>>,
    size: (usize, usize),
) -> Result<(), GpuError> {
    if slot.as_ref().is_none_or(|t| t.size != size) {
        *slot = Some(FieldTextures {
            iter: backend.create_texture(size.0, size.1, TextureFormat::Rg32Float, Storage::Private)?,
            de: backend.create_texture(size.0, size.1, TextureFormat::Rg16Float, Storage::Private)?,
            size,
        });
    }
    Ok(())
}

impl<B: GpuBackend> Renderer<B> {
    /// The lab is the main view.
    pub fn lab_on(&self) -> bool {
        self.lab.is_some()
    }

    /// The lab's kernels (compiled on first use; None: the lab is off or they did not compile).
    pub fn lab_kernels(&mut self) -> Option<LabKernels<B>> {
        let backend = &self.backend;
        self.lab.as_mut()?.kernels(backend)
    }

    /// Enters the lab with these knobs, showing `plane` (its home view fitted to `view_points`):
    /// the escape-time job and its history are dropped.
    pub fn enter_lab(&mut self, values: LabValues, plane: Plane, view_points: (f64, f64)) {
        self.drain_in_flight();
        self.wait_frames();
        self.job = None;
        self.history_view = None;
        self.lab = Some(Box::new(LabState::new(values, plane)));
        self.navigator.set_lab_plane(Some(plane));
        self.navigator.cancel_flight();
        self.viewport = lab::home_view(plane, view_points.0, view_points.1);
    }

    /// Leaves the lab (its knobs and plane are returned); the caller sets the view.
    pub fn leave_lab(&mut self) -> Option<(LabValues, Plane)> {
        let lab = self.lab.take()?;
        self.wait_frames();
        self.navigator.set_lab_plane(None);
        self.navigator.cancel_flight();
        self.history_view = None;
        self.job = None;
        Some((lab.values, lab.plane))
    }

    /// Shows another plane: the current one keeps its view for later, the new one comes back to
    /// its own (or its home view fitted to `view_points`).
    pub fn set_lab_plane(&mut self, plane: Plane, view_points: (f64, f64)) {
        let Some(lab) = self.lab.as_mut() else { return };
        if lab.plane == plane {
            return;
        }
        lab.views[lab.plane.index()] = Some(self.viewport.clone());
        let next = lab.views[plane.index()]
            .take()
            .unwrap_or_else(|| lab::home_view(plane, view_points.0, view_points.1));
        lab.plane = plane;
        lab.prev_view = None;
        self.navigator.set_lab_plane(Some(plane));
        self.navigator.cancel_flight();
        self.navigator.jumped();
        self.viewport = next;
    }

    /// The lab's home view of the shown plane (Reset).
    pub fn lab_home(&self, view_points: (f64, f64)) -> Option<Viewport> {
        self.lab
            .as_ref()
            .map(|l| lab::home_view(l.plane, view_points.0, view_points.1))
    }

    /// The main view's colour parameters in the lab: the current look, flat interior (the
    /// lab has no cycle analysis), no fog.
    fn lab_colors(&mut self) -> (ColorParams, String) {
        let mut c = self.color_params();
        c.interior_mode = 0;
        c.bounded_interior = 0;
        c.fog_levels = 0;
        (c, self.palette_key())
    }

    /// The precision limit: a scale below it is clamped about the point the zoom was heading
    /// for (inferred from the last refresh's view, exact for a pure zoom).
    fn clamp_lab_zoom(&mut self, ppp: f64, now: f64) {
        let Some(lab) = self.lab.as_mut() else { return };
        let v = &self.viewport;
        let c = (v.center.re(), v.center.im());
        let min = lab::min_pixel(c) / ppp.max(1e-6);
        let s = v.scale.to_f64();
        if s < min {
            let (mut cx, mut cy) = c;
            if let Some(((px, py), ps)) = lab.prev_view
                && ps > s
            {
                let f = s / ps;
                let g = (min / ps).min(1.0);
                if 1.0 - f > 1e-12 {
                    let (ax, ay) = ((cx - f * px) / (1.0 - f), (cy - f * py) / (1.0 - f));
                    cx = ax + (px - ax) * g;
                    cy = ay + (py - ay) * g;
                }
            }
            let angle = v.angle;
            self.viewport = Viewport::with_center(Center::new(cx, cy), Scale::from_f64(min)).with_angle(angle);
            lab.at_limit = Some(now);
        }
        let v = &self.viewport;
        lab.prev_view = Some(((v.center.re(), v.center.im()), v.scale.to_f64()));
    }

    /// One refresh of the lab (None: the lab is off).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lab_frame(
        &mut self,
        now: f64,
        width: usize,
        height: usize,
        points_per_pixel: f64,
        interacting: bool,
        keep_going: bool,
        acquire: &mut dyn FnMut(bool) -> Option<FrameTarget<B>>,
    ) -> Option<FrameOutcome> {
        self.lab.as_ref()?;
        self.clamp_lab_zoom(points_per_pixel, now);
        let mut lab = self.lab.take()?;
        let r = self.lab_frame_with(
            &mut lab,
            now,
            (width, height),
            points_per_pixel,
            interacting,
            keep_going,
            acquire,
        );
        self.lab = Some(lab);
        Some(r.unwrap_or_else(|e| {
            eprintln!("almondbread: lab frame: {e}");
            FrameOutcome {
                animating: true,
                presented: false,
            }
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn lab_frame_with(
        &mut self,
        lab: &mut LabState<B>,
        now: f64,
        size: (usize, usize),
        ppp: f64,
        interacting: bool,
        keep_going: bool,
        acquire: &mut dyn FnMut(bool) -> Option<FrameTarget<B>>,
    ) -> Result<FrameOutcome, GpuError> {
        let mut outcome = FrameOutcome {
            animating: true,
            presented: false,
        };
        let Some(kernels) = lab.kernels(&self.backend) else {
            outcome.animating = keep_going;
            return Ok(outcome);
        };
        lab.poll();
        self.ensure_palette()?;
        self.ensure_history(size.0, size.1)?;
        let pixel = self.viewport.scale.to_f64() * ppp;
        let key = FieldKey {
            values: lab.values,
            plane: lab.plane,
            center: (self.viewport.center.re(), self.viewport.center.im()),
            scale: self.viewport.scale.to_f64(),
            angle: self.viewport.angle,
            size,
            max_iter: lab::iterations(self.settings.auto_iterations, self.settings.manual_iterations, pixel),
        };
        if lab.changed.as_ref().is_none_or(|(k, _)| *k != key) {
            lab.changed = Some((key.clone(), now));
            lab.job = None;
        }
        let since = lab.changed.as_ref().map_or(f64::INFINITY, |(_, t)| now - *t);
        let moving = interacting || since < REST;
        let colours = self.lab_colors();
        let aa = self.settings.supersampling != Supersampling::Off && size.0 * size.1 * 4 <= MAX_AA_TEXELS;
        let rest_ratio = if aa { 2 } else { 1 };
        let budget = 0.6 * self.refresh_interval.clamp(1.0 / 240.0, 1.0 / 30.0) * self.thermal.budget_scale();
        let frames_free = self.frames_in_flight.len() < self.config.max_frames_in_flight;
        let shown_key = lab.shown.as_ref().map(|(k, r, _)| (k.clone(), *r));

        if moving {
            // A field of this view at the resolution the budget affords, shown at once.
            let new_field = shown_key.as_ref().is_none_or(|(k, _)| *k != key);
            let recolour = !new_field && lab.shown.as_ref().is_some_and(|(_, _, c)| *c != colours);
            if (new_field || recolour) && frames_free {
                let texels = (size.0 * size.1) as f64;
                let ratio = if new_field {
                    if lab.cost_per_texel > 0.0 {
                        (budget / (lab.cost_per_texel * texels)).sqrt().clamp(MIN_PREVIEW, 1.0)
                    } else {
                        0.5
                    }
                } else {
                    shown_key.as_ref().map_or(1.0, |(_, r)| *r)
                };
                if let Some((target, drawable)) = acquire(false) {
                    let ratio = if ratio > 0.9 && new_field { 1.0 } else { ratio };
                    if new_field || ratio < 1.0 || ratio > 1.0 {
                        self.lab_encode_field(lab, &kernels, &key, ratio, ppp)?;
                    }
                    self.lab_present(lab, &target, drawable.as_ref(), ratio, &colours, ppp, now)?;
                    lab.shown = Some((key.clone(), ratio, colours.clone()));
                    outcome.presented = true;
                }
            }
            lab.settled = false;
            outcome.animating = true;
            return Ok(outcome);
        }

        // At rest: the full-resolution field, then the 2x2 one, in bands.
        let shown_ratio = shown_key.as_ref().filter(|(k, _)| *k == key).map_or(0.0, |(_, r)| *r);
        if lab.job.is_none() && frames_free {
            let next = if shown_ratio < 1.0 {
                Some(1)
            } else if shown_ratio < rest_ratio as f64 {
                Some(rest_ratio)
            } else {
                None
            };
            if let Some(ratio) = next {
                let tsize = (size.0 * ratio, size.1 * ratio);
                let texel_points = ppp / ratio as f64;
                let params = view_params(
                    &key.values,
                    key.plane,
                    &self.viewport,
                    tsize,
                    texel_points,
                    key.max_iter,
                );
                lab.job = Some(BandJob {
                    key: key.clone(),
                    ratio,
                    params,
                    next_row: 0,
                });
            }
        }
        let mut presented_final = false;
        if let Some(job) = lab.job.as_mut() {
            let ratio = job.ratio;
            let slot = if ratio == 1 { &mut lab.full } else { &mut lab.aa };
            let tsize = (job.params.width as usize, job.params.height as usize);
            ensure_textures(&self.backend, slot, tsize)?;
            let tex = slot.as_ref().expect("textures");
            let row_texels = f64::from(job.params.width);
            let rows_budget = if lab.cost_per_texel > 0.0 {
                (budget / (lab.cost_per_texel * row_texels)).floor() as u32
            } else {
                job.params.height.div_ceil(8)
            };
            let rows = rows_budget.clamp(16, job.params.height);
            let mut enc = self.backend.begin("lab band")?;
            let mut p = job.params;
            p.row0 = job.next_row;
            let n = rows.min(p.height - p.row0);
            encode_field::<B>(&mut enc, &kernels, &p, n, &tex.iter, &tex.de);
            lab.pending.push_back((enc.commit(), (n * p.width) as usize));
            job.next_row += n;
            if job.next_row >= job.params.height {
                // Complete: colour and show it (the queue runs the bands first).
                if frames_free && let Some((target, drawable)) = acquire(true) {
                    let r = ratio as f64;
                    self.lab_present(lab, &target, drawable.as_ref(), r, &colours, ppp, now)?;
                    let key = lab.job.take().expect("job").key;
                    lab.shown = Some((key, r, colours.clone()));
                    self.last_final_frame = (now, format!("{}x{}", tsize.0, tsize.1));
                    outcome.presented = true;
                    presented_final = true;
                } else {
                    // No target now: draw the last band again next refresh (cheap enough).
                    job.next_row -= n;
                }
            }
        }
        if !presented_final && frames_free {
            // The same field in new colours (palette, density, cycling), or the overlay alone.
            let recolour = lab.job.is_none() && lab.shown.as_ref().is_some_and(|(k, _, c)| *k == key && *c != colours);
            if recolour {
                if let Some((target, drawable)) = acquire(false) {
                    let r = shown_key.as_ref().map_or(1.0, |(_, r)| *r);
                    self.lab_present(lab, &target, drawable.as_ref(), r, &colours, ppp, now)?;
                    if let Some(s) = lab.shown.as_mut() {
                        s.2 = colours.clone();
                    }
                    outcome.presented = true;
                }
            } else if self.overlay_stale()
                && let Some((target, drawable)) = acquire(false)
            {
                let mut enc = self.encode_overlay_only(&target)?;
                if let Some(d) = &drawable {
                    enc.present(d);
                }
                let sub = enc.commit();
                self.frames_committed += 1;
                self.frames_in_flight.push((self.frames_committed, sub));
                self.last_frame_commit = now;
                outcome.presented = true;
            }
        }
        lab.settled = lab.job.is_none()
            && lab.pending.is_empty()
            && lab
                .shown
                .as_ref()
                .is_some_and(|(k, r, c)| *k == key && *r >= rest_ratio as f64 && *c == colours);
        outcome.animating = keep_going || !lab.settled || self.overlay_stale();
        Ok(outcome)
    }

    /// Encodes a moving field of `key` at `ratio` texels per pixel (≤ 1) into the full-size
    /// textures (a preview uses their top-left part), as a submission of its own (timed).
    fn lab_encode_field(
        &mut self,
        lab: &mut LabState<B>,
        kernels: &LabKernels<B>,
        key: &FieldKey,
        ratio: f64,
        ppp: f64,
    ) -> Result<(), GpuError> {
        let size = key.size;
        ensure_textures(&self.backend, &mut lab.full, size)?;
        let tsize = (
            ((size.0 as f64 * ratio).ceil() as usize).clamp(1, size.0),
            ((size.1 as f64 * ratio).ceil() as usize).clamp(1, size.1),
        );
        // Texel size so the preview covers the view exactly.
        let texel_points = ppp * size.0 as f64 / tsize.0 as f64;
        let p = view_params(
            &key.values,
            key.plane,
            &self.viewport,
            tsize,
            texel_points,
            key.max_iter,
        );
        let tex = lab.full.as_ref().expect("textures");
        let mut enc = self.backend.begin("lab field")?;
        encode_field::<B>(&mut enc, kernels, &p, p.height, &tex.iter, &tex.de);
        lab.pending.push_back((enc.commit(), tsize.0 * tsize.1));
        Ok(())
    }

    /// Colours the shown field (`ratio` texels per pixel: < 1 a preview in the full textures'
    /// corner, 1 the full textures, 2 the 2×2 ones) into the history and presents it with the
    /// overlay.
    #[allow(clippy::too_many_arguments)]
    fn lab_present(
        &mut self,
        lab: &mut LabState<B>,
        target: &B::Texture,
        drawable: Option<&B::Drawable>,
        ratio: f64,
        colours: &(ColorParams, String),
        ppp: f64,
        now: f64,
    ) -> Result<(), GpuError> {
        let (tw, th) = self.backend.texture_size(target);
        let history = self.history.clone().expect("history");
        let hist_out = history[1 - self.history_ping].clone();
        let mut enc = self.backend.begin("lab colour")?;
        self.lab_encode_colour(&mut enc, lab, &hist_out, (tw, th), ratio, &colours.0)?;
        self.encode_overlay(&mut enc, &hist_out, target, true);
        if let Some(d) = drawable {
            enc.present(d);
        }
        let sub = enc.commit();
        self.frames_committed += 1;
        self.frames_in_flight.push((self.frames_committed, sub));
        self.last_frame_commit = now;
        self.has_presented = true;
        self.history_ping = 1 - self.history_ping;
        self.history_view = Some(HistoryView {
            viewport: self.viewport.clone(),
            scale: self.viewport.scale * ppp,
            width: tw,
            height: th,
            colors: colours.clone(),
            offset: colours.0.offset,
        });
        if let Some(fs) = &mut self.frame_stats {
            fs.frames += 1;
            if ratio < 1.0 {
                fs.preview_frames += 1;
            }
        }
        Ok(())
    }

    /// The colour pass of a lab field into `out` (linear light, `out_size`).
    fn lab_encode_colour(
        &mut self,
        enc: &mut B::Encoder,
        lab: &mut LabState<B>,
        out: &B::Texture,
        out_size: (usize, usize),
        ratio: f64,
        colors: &ColorParams,
    ) -> Result<(), GpuError> {
        let palette = self.palette_buffer.clone().expect("palette");
        let (tw, th) = self.threadgroup();
        let base = ColorParams {
            admitted: u32::MAX,
            admitted_on_gpu: 0,
            tile_order: 0,
            fill_border: 0,
            has_under: 0.0,
            linear_out: 1,
            out_size: [out_size.0 as u32, out_size.1 as u32],
            ..*colors
        };
        enc.set_pipeline(&self.colorize_pipeline);
        enc.set_buffer(1, &palette, 0);
        enc.set_buffer(2, &self.level_buffer, 0);
        enc.set_buffer(3, &self.dummy_buffer, 0);
        if ratio < 1.0 {
            // The preview: coloured at its own size, then upsampled bilinearly.
            let tex = lab.full.as_ref().ok_or_else(|| GpuError("lab: no field".into()))?;
            let psize = (
                ((out_size.0 as f64 * ratio).ceil() as usize).clamp(1, tex.size.0),
                ((out_size.1 as f64 * ratio).ceil() as usize).clamp(1, tex.size.1),
            );
            if lab.preview_color.as_ref().is_none_or(|(_, s)| *s != tex.size) {
                let t = self.backend.create_texture(
                    tex.size.0,
                    tex.size.1,
                    TextureFormat::Rgba16Float,
                    Storage::Private,
                )?;
                lab.preview_color = Some((t, tex.size));
            }
            let (color, _) = lab.preview_color.as_ref().expect("preview colour");
            let c = ColorParams {
                ratio: 1.0,
                de_scale: ratio as f32,
                src_size: [psize.0 as u32, psize.1 as u32],
                out_size: [psize.0 as u32, psize.1 as u32],
                ..base
            };
            enc.set_texture(0, &tex.iter);
            enc.set_texture(1, &tex.de);
            enc.set_texture(2, color);
            enc.set_texture(3, color);
            enc.set_texture(4, &tex.iter);
            enc.set_texture(5, &tex.iter);
            enc.set_bytes(0, bytes_of(&c));
            enc.dispatch_threads([psize.0, psize.1, 1], [tw, th, 1]);
            let up = ColorParams {
                ratio: ratio as f32,
                de_scale: ratio as f32,
                admitted: 0,
                has_under: 1.0,
                under_size: [psize.0 as f32, psize.1 as f32],
                src_size: [psize.0 as u32, psize.1 as u32],
                ..base
            };
            enc.set_texture(2, out);
            enc.set_texture(3, color);
            enc.set_bytes(0, bytes_of(&up));
            enc.dispatch_threads([out_size.0, out_size.1, 1], [tw, th, 1]);
        } else {
            let tex = if ratio >= 2.0 {
                lab.aa.as_ref()
            } else {
                lab.full.as_ref()
            }
            .ok_or_else(|| GpuError("lab: no field".into()))?;
            let c = ColorParams {
                ratio: ratio as f32,
                de_scale: ratio as f32,
                src_size: [tex.size.0 as u32, tex.size.1 as u32],
                ..base
            };
            enc.set_texture(0, &tex.iter);
            enc.set_texture(1, &tex.de);
            enc.set_texture(2, out);
            enc.set_texture(3, out);
            enc.set_texture(4, &tex.iter);
            enc.set_texture(5, &tex.iter);
            enc.set_bytes(0, bytes_of(&c));
            enc.dispatch_threads([out_size.0, out_size.1, 1], [tw, th, 1]);
        }
        Ok(())
    }

    /// Offscreen: the lab's picture of the current view at `width` x `height` (2×2 supersampled
    /// unless supersampling is off), with the overlay if there is one. Blocks. `readback`: the
    /// iteration texels (one sample per pixel: a field of its own).
    pub fn render_lab_offscreen(
        &mut self,
        width: usize,
        height: usize,
        readback: bool,
    ) -> Result<OffscreenResult, GpuError> {
        let start = std::time::Instant::now();
        let mut lab = self.lab.take().ok_or_else(|| GpuError("the lab is off".into()))?;
        let r = self.render_lab_offscreen_with(&mut lab, width, height, readback, start);
        self.lab = Some(lab);
        r
    }

    fn render_lab_offscreen_with(
        &mut self,
        lab: &mut LabState<B>,
        width: usize,
        height: usize,
        readback: bool,
        start: std::time::Instant,
    ) -> Result<OffscreenResult, GpuError> {
        let kernels = lab
            .kernels(&self.backend)
            .ok_or_else(|| GpuError("the lab's kernels did not compile".into()))?;
        self.ensure_palette()?;
        let ratio = if self.settings.supersampling == Supersampling::Off {
            1
        } else {
            2
        };
        let pixel = self.viewport.scale.to_f64();
        let max_iter = lab::iterations(self.settings.auto_iterations, self.settings.manual_iterations, pixel);
        let tsize = (width * ratio, height * ratio);
        let slot = if ratio == 1 { &mut lab.full } else { &mut lab.aa };
        ensure_textures(&self.backend, slot, tsize)?;
        let p = view_params(
            &lab.values,
            lab.plane,
            &self.viewport,
            tsize,
            1.0 / ratio as f64,
            max_iter,
        );
        let tex = slot.as_ref().expect("textures");
        // Bands of at most 256 rows: no single dispatch near the GPU watchdog.
        let mut gpu = 0.0;
        let mut row = 0;
        while row < p.height {
            let mut q = p;
            q.row0 = row;
            let n = 256.min(p.height - row);
            let mut enc = self.backend.begin("lab offscreen field")?;
            encode_field::<B>(&mut enc, &kernels, &q, n, &tex.iter, &tex.de);
            let sub = enc.commit();
            sub.wait();
            if let SubmissionStatus::Failed(e) = sub.status() {
                return Err(GpuError(format!("lab field failed: {e}")));
            }
            gpu += sub.gpu_time();
            row += n;
        }
        let colours = self.lab_colors();
        let target = self
            .backend
            .create_texture(width, height, TextureFormat::Bgra8Unorm, Storage::Shared)?;
        let lin = self
            .backend
            .create_texture(width, height, TextureFormat::Rgba16Float, Storage::Private)?;
        let mut enc = self.backend.begin("lab offscreen colour")?;
        self.lab_encode_colour(&mut enc, lab, &lin, (width, height), ratio as f64, &colours.0)?;
        self.encode_overlay(&mut enc, &lin, &target, true);
        let sub = enc.commit();
        sub.wait();
        if let SubmissionStatus::Failed(e) = sub.status() {
            return Err(GpuError(format!("lab colour failed: {e}")));
        }
        let mut pixels = vec![0u8; width * height * 4];
        self.backend
            .read_texture(&target, width, height, TextureFormat::Bgra8Unorm, &mut pixels)?;
        let iterations = if readback {
            let (f, _) = crate::lab::gpu_field(
                &self.backend,
                &kernels,
                &lab.values,
                lab.plane,
                &self.viewport,
                (width, height),
                1.0,
                max_iter,
            )?;
            Some(f.n.iter().zip(&f.s).map(|(&n, &s)| [n, s]).collect())
        } else {
            None
        };
        self.stats.max_iterations = max_iter;
        Ok(OffscreenResult {
            pixels,
            width,
            height,
            iterate_gpu_time: gpu,
            colorize_gpu_time: sub.gpu_time(),
            wall_time: start.elapsed().as_secs_f64(),
            chunks: 0,
            iterations_done: 0,
            steps_done: 0,
            bla_forecast: None,
            bla_used: false,
            la_used: false,
            iterations,
            seeds: None,
            orbits: None,
        })
    }
}
