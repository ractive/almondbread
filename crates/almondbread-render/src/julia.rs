//! The Julia companion panel (kb: plans/f1-julia-view, formulas/julia): the filled Julia set of
//! the orbit's point `k` (pointer, touch probe, tour probe; the zoom target while the autopilot
//! flies), framed on `|z| ≤ 2`, in a panel of the mini-map row. Its own small kernel library
//! (`shaders/julia.metal`, direct float32 iteration from `z₀ = pixel`), compiled when the panel
//! is first shown: nothing of it runs or is compiled while it is off, and the Mandelbrot
//! pipelines do not change.
//!
//! Update policy: a new picture when `k` moves by more than half a panel pixel (in z units), the
//! panel's size or the colours change; one submission in flight at a time (a fast pointer skips
//! the pictures in between, never queues them). While `k` moves a picture is one sample per pixel
//! in one dispatch; once it rests for [`REST`] a 2×2 picture follows, in bands of rows (one per
//! refresh) when it is slow ([`BAND_MS`]). At most [`almondbread_core::julia::MAX_ITERATIONS`] steps per sample; with
//! k's attracting cycle known, interior samples stop when they reach it (GPU times in the tests
//! and kb: plans/f1-julia-view).

use almondbread_core::julia::{self, Cycle};
use almondbread_core::settings::{InteriorMode, RenderSettings};
use bytemuck::{Pod, Zeroable};

use crate::backend::{
    ComputeEncoder, GpuBackend, GpuError, MathMode, Storage, Submission, SubmissionStatus, TextureFormat,
};

/// The panel's kernel library.
pub const JULIA_SOURCE: &str = include_str!("shaders/julia.metal");
const FUNCTIONS: [&str; 3] = ["juliaField", "juliaColor", "juliaLayout"];
const PALETTE_SIZE: usize = 1024;

/// `JuliaParams` in `julia.metal`, 96 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct JuliaParams {
    pub interior: [f32; 4],
    pub k: [f32; 2],
    pub cycle: [f32; 2],
    pub scale: f32,
    pub bailout2: f32,
    pub density: f32,
    pub offset: f32,
    pub cycle_eps2: f32,
    pub width: u32,
    pub height: u32,
    pub max_iter: u32,
    pub log_scale: u32,
    pub smooth_color: u32,
    pub palette_size: u32,
    pub samples: u32,
    pub period: u32,
    pub inv_log_lambda: f32,
    pub shade_interior: u32,
    pub row0: u32,
}

const _: () = assert!(std::mem::size_of::<JuliaParams>() == 96);

/// Distance to the cycle point (z units) that counts as captured.
const CYCLE_EPS: f64 = 1e-3;

/// The compiled kernels.
pub struct JuliaKernels<B: GpuBackend> {
    field: B::Pipeline,
    color: B::Pipeline,
    layout: B::Pipeline,
}

impl<B: GpuBackend> JuliaKernels<B> {
    pub fn compile(backend: &B) -> Result<Self, GpuError> {
        let mut p = backend
            .library_pipelines(JULIA_SOURCE, &FUNCTIONS, MathMode::Fast)?
            .into_iter();
        let mut next = || p.next().expect("one pipeline per function");
        Ok(Self {
            field: next(),
            color: next(),
            layout: next(),
        })
    }
}

/// The parameters of a picture of `k` at `size` with the main view's colours. `cycle`: k's
/// attracting cycle (orbits that reach it stop early, as interior; shaded by the time to reach
/// it unless the main view's interior is flat).
pub fn params(
    k: (f64, f64),
    size: (usize, usize),
    s: &RenderSettings,
    cycle: Option<Cycle>,
    samples: u32,
) -> JuliaParams {
    let i = s.interior_color;
    let shade = s.interior_mode != InteriorMode::Flat;
    let (cycle_point, period, inv_log_lambda) = match cycle {
        Some(c) => (
            [c.point.0 as f32, c.point.1 as f32],
            c.period as u32,
            if c.multiplier > 1e-12 {
                (1.0 / c.multiplier.log2()) as f32
            } else {
                0.0
            },
        ),
        None => ([0.0; 2], 0, 0.0),
    };
    JuliaParams {
        interior: [i[0], i[1], i[2], 1.0],
        k: [k.0 as f32, k.1 as f32],
        cycle: cycle_point,
        scale: julia::panel_scale(size.0, size.1) as f32,
        bailout2: (julia::BAILOUT * julia::BAILOUT) as f32,
        density: s.color_density as f32,
        offset: s.color_offset.rem_euclid(1.0) as f32,
        cycle_eps2: (CYCLE_EPS * CYCLE_EPS) as f32,
        width: size.0 as u32,
        height: size.1 as u32,
        max_iter: julia::MAX_ITERATIONS as u32,
        log_scale: u32::from(s.log_coloring),
        smooth_color: u32::from(s.smooth_coloring),
        palette_size: PALETTE_SIZE as u32,
        samples,
        period,
        inv_log_lambda,
        shade_interior: u32::from(shade),
        row0: 0,
    }
}

/// What a picture depends on: `k` to half a panel pixel, the size, the samples and the colours.
#[derive(Clone, Debug, PartialEq)]
struct PictureKey {
    k: [i64; 2],
    size: (usize, usize),
    samples: u32,
    palette: String,
    colours: [u32; 8],
}

fn palette_key(s: &RenderSettings) -> String {
    match &s.palette_stops {
        Some(stops) if !stops.is_empty() => format!("stops:{stops:?}"),
        _ => s.palette_id.clone(),
    }
}

/// The panel's state: kernels, its texture, the picture shown and the one being drawn.
pub struct JuliaPanel<B: GpuBackend> {
    kernels: Option<JuliaKernels<B>>,
    /// The library did not compile: the panel stays empty (reported once).
    failed: bool,
    target: Option<(B::Texture, (usize, usize))>,
    palette: Option<(String, B::Buffer)>,
    key: Option<PictureKey>,
    pending: Option<(B::Submission, f64)>,
    /// `k` of the picture shown (or being drawn).
    pub k: Option<(f64, f64)>,
    /// A picture was submitted to the texture (the panel shows it).
    pub shown: bool,
    /// Counts submitted pictures (the overlay scene's key: a new picture needs a new frame).
    pub serial: u64,
    /// GPU milliseconds of the last finished submission (a picture or a band of one).
    pub gpu_ms: Option<f64>,
    /// GPU milliseconds of the current picture so far (all its bands).
    pub picture_ms: f64,
    /// k's attracting cycle, cached for the last k.
    cycle: Option<((f64, f64), Option<Cycle>)>,
    /// When `k` (to half a pixel) last changed: while it moves, pictures take one sample per
    /// pixel; once it rests for [`REST`], one more with 2×2.
    moved: Option<([i64; 2], f64)>,
    /// The 2×2 picture being drawn in bands: its parameters, the next band's first row and the
    /// band height.
    refine: Option<(JuliaParams, u32, u32)>,
}

/// The 2×2 picture is drawn in bands of rows, one per refresh, about [`BAND_MS`] of GPU each
/// (estimated from the one-sample picture of the same k: four times its time), at most
/// [`MAX_BANDS`] (also when there is no estimate). Its worst case (a near-parabolic k, most
/// pixels running all 1000 steps four times) is ~4–6 ms at Retina panel size, too much for one
/// 120 Hz frame next to the main view; most k take under 0.75 ms (one band; small bands use the GPU less well); the bands near the middle rows are the slowest.
pub const BAND_MS: f64 = 0.75;
pub const MAX_BANDS: u32 = 8;

/// Seconds `k` must rest before the 2×2 picture.
pub const REST: f64 = 0.2;

impl<B: GpuBackend> Default for JuliaPanel<B> {
    fn default() -> Self {
        Self {
            kernels: None,
            failed: false,
            target: None,
            palette: None,
            key: None,
            pending: None,
            k: None,
            shown: false,
            serial: 0,
            gpu_ms: None,
            picture_ms: 0.0,
            cycle: None,
            moved: None,
            refine: None,
        }
    }
}

impl<B: GpuBackend> JuliaPanel<B> {
    /// The panel's texture (bound as a map slot by the overlay).
    pub fn texture(&self) -> Option<&B::Texture> {
        self.target.as_ref().map(|(t, _)| t)
    }

    /// A picture is being drawn, or the 2×2 one (or a band of it) is still due.
    pub fn busy(&self) -> bool {
        self.pending.is_some() || self.refine.is_some() || self.key.as_ref().is_some_and(|k| k.samples < 4)
    }

    /// The panel is off: frees the texture and the picture (the kernels stay compiled).
    pub fn clear(&mut self) {
        self.target = None;
        self.key = None;
        self.k = None;
        self.shown = false;
        self.pending = None;
        self.moved = None;
        self.refine = None;
    }

    /// Collects a finished submission.
    fn poll(&mut self) {
        if let Some((sub, _)) = &self.pending {
            match sub.status() {
                SubmissionStatus::Pending => {}
                SubmissionStatus::Completed => {
                    let ms = sub.gpu_time() * 1000.0;
                    self.gpu_ms = Some(ms);
                    self.picture_ms += ms;
                    self.pending = None;
                }
                SubmissionStatus::Failed(e) => {
                    eprintln!("almondbread: julia panel: {e}");
                    self.pending = None;
                    // Draw it again.
                    self.key = None;
                    self.refine = None;
                }
            }
        }
    }

    /// Draws a new picture of `k` at `size` pixels when it changed (and none is in flight).
    pub fn update(
        &mut self,
        backend: &B,
        k: (f64, f64),
        size: (usize, usize),
        settings: &RenderSettings,
        now: f64,
    ) -> Result<(), GpuError> {
        self.poll();
        if self.failed || size.0 == 0 || size.1 == 0 || !(k.0.is_finite() && k.1.is_finite()) {
            return Ok(());
        }
        let scale = julia::panel_scale(size.0, size.1);
        let q = 0.5 * scale;
        let shade = settings.interior_mode != InteriorMode::Flat;
        let kq = [(k.0 / q).round() as i64, (k.1 / q).round() as i64];
        // (The first picture of a new k is a moving one only when k moved from another.)
        match self.moved {
            Some((m, _)) if m == kq => {}
            Some(_) => self.moved = Some((kq, now)),
            None => self.moved = Some((kq, f64::NEG_INFINITY)),
        }
        let resting = self.moved.is_some_and(|(_, t)| now - t >= REST);
        let samples = if resting { 4 } else { 1 };
        let key = PictureKey {
            k: kq,
            size,
            samples,
            palette: palette_key(settings),
            colours: [
                (settings.color_density as f32).to_bits(),
                (settings.color_offset as f32).to_bits(),
                u32::from(settings.log_coloring) | u32::from(settings.smooth_coloring) << 1 | u32::from(shade) << 2,
                settings.interior_color[0].to_bits(),
                settings.interior_color[1].to_bits(),
                settings.interior_color[2].to_bits(),
                0,
                0,
            ],
        };
        if self.pending.is_some() {
            return Ok(());
        }
        if self.key.as_ref() == Some(&key) {
            // The same picture: its next band, if it is drawn in bands.
            if let Some((p, row, band)) = self.refine {
                self.dispatch(backend, p, row, band, now)?;
            }
            return Ok(());
        }
        if self.kernels.is_none() {
            match JuliaKernels::compile(backend) {
                Ok(k) => self.kernels = Some(k),
                Err(e) => {
                    eprintln!("almondbread: julia panel kernels: {e}");
                    self.failed = true;
                    return Ok(());
                }
            }
        }
        if self.target.as_ref().is_none_or(|(_, s)| *s != size) {
            let t = backend.create_texture(size.0, size.1, TextureFormat::Bgra8Unorm, Storage::Private)?;
            self.target = Some((t, size));
            self.shown = false;
        }
        if self.palette.as_ref().is_none_or(|(p, _)| *p != key.palette) {
            let table = settings.palette_table(PALETTE_SIZE);
            let buf = backend.create_buffer_with_data(bytemuck::cast_slice(&table))?;
            self.palette = Some((key.palette.clone(), buf));
        }
        let cycle = match self.cycle {
            Some((ck, c)) if ck == k => c,
            _ => {
                let c = julia::attracting_cycle(k);
                self.cycle = Some((k, c));
                c
            }
        };
        let p = params(k, size, settings, cycle, samples);
        // Bands for a 2×2 picture: from the time of the one-sample picture just shown.
        let single = self
            .key
            .as_ref()
            .filter(|old| old.samples == 1 && old.k == key.k && old.size == key.size && self.k.is_some())
            .map(|_| self.picture_ms);
        let bands = if samples >= 4 {
            single.map_or(MAX_BANDS, |ms| ((4.0 * ms / BAND_MS).ceil() as u32).clamp(1, MAX_BANDS))
        } else {
            1
        };
        self.key = Some(key);
        self.k = Some(k);
        self.refine = None;
        self.dispatch(backend, p, 0, p.height.div_ceil(bands), now)
    }

    /// Submits rows `row0 .. row0 + band` of the picture `p` (the next band is left in
    /// `refine`).
    fn dispatch(&mut self, backend: &B, mut p: JuliaParams, row0: u32, band: u32, now: f64) -> Result<(), GpuError> {
        let band = band.max(1);
        p.row0 = row0;
        if row0 == 0 {
            self.picture_ms = 0.0;
        }
        let rows = band.min(p.height - row0);
        let kernels = self.kernels.as_ref().expect("kernels");
        let (target, _) = self.target.as_ref().expect("target");
        let (_, palette) = self.palette.as_ref().expect("palette");
        let mut enc = backend.begin("julia panel")?;
        enc.set_pipeline(&kernels.color);
        enc.set_bytes(0, bytemuck::bytes_of(&p));
        enc.set_buffer(1, palette, 0);
        enc.set_texture(0, target);
        enc.dispatch_threads([p.width as usize, rows as usize, 1], [16, 16, 1]);
        self.pending = Some((enc.commit(), now));
        let next = row0 + rows;
        self.refine = (next < p.height).then_some((p, next, band));
        self.shown = true;
        self.serial += 1;
        Ok(())
    }
}

/// The panel's iteration field of `k` at `w` × `h` (one sample per pixel centre; with `cycle`
/// the capture test as in the panel), read back: the GPU-vs-CPU test.
pub fn gpu_field<B: GpuBackend>(
    backend: &B,
    kernels: &JuliaKernels<B>,
    k: (f64, f64),
    w: usize,
    h: usize,
    cycle: Option<Cycle>,
) -> Result<(almondbread_core::oracle::IterationField, f64), GpuError> {
    let p = params(k, (w, h), &RenderSettings::default(), cycle, 1);
    let tex = backend.create_texture(w, h, TextureFormat::Rg32Float, Storage::Private)?;
    let mut enc = backend.begin("julia field")?;
    enc.set_pipeline(&kernels.field);
    enc.set_bytes(0, bytemuck::bytes_of(&p));
    enc.set_texture(0, &tex);
    enc.dispatch_threads([w, h, 1], [16, 16, 1]);
    let sub = enc.commit();
    sub.wait();
    if let SubmissionStatus::Failed(e) = sub.status() {
        return Err(GpuError(e));
    }
    let mut bytes = vec![0u8; w * h * 8];
    backend.read_texture(&tex, w, h, TextureFormat::Rg32Float, &mut bytes)?;
    let texels: &[f32] = bytemuck::cast_slice(&bytes);
    Ok((
        almondbread_core::oracle::IterationField {
            width: w,
            height: h,
            n: texels.iter().step_by(2).copied().collect(),
            s: texels.iter().skip(1).step_by(2).copied().collect(),
        },
        sub.gpu_time(),
    ))
}

/// `sizeof(JuliaParams)` as the Metal compiler lays it out.
pub fn msl_params_size<B: GpuBackend>(backend: &B, kernels: &JuliaKernels<B>) -> Result<u32, GpuError> {
    let out = backend.create_buffer(16, Storage::Shared)?;
    let mut enc = backend.begin("julia layout")?;
    enc.set_pipeline(&kernels.layout);
    enc.set_buffer(0, &out, 0);
    enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
    let sub = enc.commit();
    sub.wait();
    let mut b = [0u8; 4];
    backend.read_buffer(&out, 0, &mut b);
    Ok(u32::from_le_bytes(b))
}

#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use super::*;
    use crate::metal::MetalBackend;
    use almondbread_core::oracle::ITER_INTERIOR;

    const RABBIT: (f64, f64) = (-0.122_561_166_876_653_6, 0.744_861_766_619_744_2);

    fn settle(p: &mut JuliaPanel<MetalBackend>) {
        while p.pending.is_some() {
            std::thread::sleep(std::time::Duration::from_millis(1));
            p.poll();
        }
    }

    /// Submits and waits until the picture of `k` is done (every band); the slowest band's GPU
    /// milliseconds (what one refresh carries).
    fn finish(
        p: &mut JuliaPanel<MetalBackend>,
        b: &MetalBackend,
        k: (f64, f64),
        size: (usize, usize),
        s: &RenderSettings,
        now: f64,
    ) -> f64 {
        p.update(b, k, size, s, now).expect("update");
        settle(p);
        let mut slowest = p.gpu_ms.unwrap_or(0.0);
        while p.refine.is_some() {
            p.update(b, k, size, s, now).expect("update");
            settle(p);
            slowest = slowest.max(p.gpu_ms.unwrap_or(0.0));
        }
        slowest
    }

    /// The panel's float32 field against the f64 reference (plan F1 "Verification"): cardioid
    /// interior, the basilica (c = −1), the rabbit (period-3 nucleus), the Misiurewicz point i (a
    /// dendrite, no interior), seahorse valley (just outside the set: chaotic near the boundary,
    /// float32 and f64 orbits part there) and dust (0.5 + 0.5i). The class (escaped or not)
    /// agrees for all but 0.1 % of the pixels, the escape iteration for 97 % (the smooth offset
    /// to a quarter step where it agrees); with the capture
    /// test (k's attracting cycle) the class is the same as without.
    #[test]
    fn gpu_field_matches_the_cpu_reference() {
        let b = MetalBackend::new().expect("metal");
        let kernels = JuliaKernels::compile(&b).expect("julia library");
        assert_eq!(msl_params_size(&b, &kernels).expect("layout"), 96);
        let (w, h) = (360, 259);
        for (name, k) in [
            ("cardioid", (0.25, 0.3)),
            ("basilica", (-1.0, 0.0)),
            ("rabbit", RABBIT),
            ("misiurewicz i", (0.0, 1.0)),
            ("seahorse", (-0.745, 0.113)),
            ("dust", (0.5, 0.5)),
        ] {
            let cpu = julia::cpu_field(k, w, h, julia::MAX_ITERATIONS);
            let cycle = julia::attracting_cycle(k);
            // (Twice: the first run includes the GPU's clock ramp.)
            gpu_field(&b, &kernels, k, w, h, None).expect("gpu field");
            let (gpu, secs) = gpu_field(&b, &kernels, k, w, h, None).expect("gpu field");
            let (captured, csecs) = gpu_field(&b, &kernels, k, w, h, cycle).expect("gpu field");
            let total = w * h;
            let mut class = 0;
            let mut iter = 0;
            let mut capture_class = 0;
            let mut smooth_err = 0f64;
            let mut smooth_bad = 0;
            for i in 0..total {
                let (a, g) = (cpu.n[i], gpu.n[i]);
                if (g == ITER_INTERIOR) != (captured.n[i] == ITER_INTERIOR) {
                    capture_class += 1;
                }
                if (a == ITER_INTERIOR) != (g == ITER_INTERIOR) {
                    class += 1;
                } else if a != ITER_INTERIOR {
                    if a != g {
                        iter += 1;
                    } else {
                        let e = f64::from((cpu.s[i] - gpu.s[i]).abs());
                        smooth_err = smooth_err.max(e);
                        smooth_bad += usize::from(e > 0.25);
                    }
                }
            }
            let inside = cpu.n.iter().filter(|&&n| n == ITER_INTERIOR).count();
            eprintln!(
                "julia {name}: cycle {:?}, interior {:.1} %, class differs {class}, n differs {iter}, max |ds| (same n) {smooth_err:.4}, capture changes {capture_class}; GPU {:.3} ms, with capture {:.3} ms",
                cycle.map(|c| c.period),
                100.0 * inside as f64 / total as f64,
                secs * 1000.0,
                csecs * 1000.0
            );
            assert!(class * 1000 <= total, "{name}: {class} of {total} pixels change class");
            assert!(
                iter * 100 <= 3 * total,
                "{name}: {iter} of {total} pixels change escape iteration"
            );
            assert!(
                smooth_bad * 100 <= total,
                "{name}: {smooth_bad} pixels with the smooth offset off by over a quarter step"
            );
            assert!(capture_class * 1000 <= total, "{name}: capture changes {capture_class}");
            match name {
                "dust" | "misiurewicz i" | "seahorse" => {
                    assert!(inside * 1000 <= total, "{name}: {inside} interior");
                    assert!(cycle.is_none(), "{name}");
                }
                _ => {
                    assert!(inside * 20 >= total, "{name}: {inside} interior");
                    assert!(cycle.is_some(), "{name}");
                }
            }
        }
    }

    /// The coloured picture: a new k re-renders, the same k (to half a pixel) does not, one in
    /// flight at a time; a moving k takes one sample per pixel, a resting one 2x2. GPU times of
    /// the worst cases (best of three: the first includes the GPU's clock ramp).
    #[test]
    fn panel_redraws_only_when_k_moves() {
        let b = MetalBackend::new().expect("metal");
        let mut p = JuliaPanel::<MetalBackend>::default();
        let s = RenderSettings::default();
        let size = (360, 259);
        p.update(&b, (-1.0, 0.0), size, &s, 0.0).expect("update");
        assert!(p.shown && p.busy());
        assert_eq!(p.key.as_ref().map(|k| k.samples), Some(4));
        finish(&mut p, &b, (-1.0, 0.0), size, &s, 0.0);
        assert_eq!(p.serial, u64::from(MAX_BANDS), "no estimate yet: the most bands");
        let serial = p.serial;
        // The same k (to a fraction of a pixel): no new picture.
        p.update(&b, (-1.0 + 1e-5, 0.0), size, &s, 0.02).expect("update");
        assert_eq!(p.serial, serial);
        assert!(!p.busy());
        // A moving k: one sample, then 2x2 once it rests.
        p.update(&b, (-0.9, 0.0), size, &s, 1.0).expect("update");
        assert_eq!((p.serial, p.key.as_ref().map(|k| k.samples)), (serial + 1, Some(1)));
        settle(&mut p);
        assert!(p.busy(), "the 2x2 picture is due");
        p.update(&b, (-0.9, 0.0), size, &s, 1.1).expect("update");
        assert_eq!(p.serial, serial + 1);
        p.update(&b, (-0.9, 0.0), size, &s, 1.01 + REST).expect("update");
        assert_eq!((p.serial, p.key.as_ref().map(|k| k.samples)), (serial + 2, Some(4)));
        finish(&mut p, &b, (-0.9, 0.0), size, &s, 1.01 + REST);
        assert!(!p.busy());
        let mut shaded = s.clone();
        shaded.interior_mode = InteriorMode::Bubbles;
        let mut t = 10.0;
        for (name, k, st) in [
            ("basilica", (-1.0, 0.0), &s),
            ("rabbit, shaded interior", RABBIT, &shaded),
            ("cardioid edge (near-parabolic)", (-0.75, 0.01), &s),
            ("cardioid", (0.25, 0.3), &s),
            ("dust", (0.5, 0.5), &s),
        ] {
            let (mut moving, mut resting, mut slowest) = (f64::INFINITY, f64::INFINITY, f64::INFINITY);
            for _ in 0..3 {
                // As in use: k arrives moving (one sample), then rests (2x2 in bands, as many as
                // the one-sample picture's time asks for).
                p.moved = Some(([i64::MIN, 0], t));
                finish(&mut p, &b, k, size, st, t);
                assert_eq!(p.key.as_ref().map(|k| k.samples), Some(1));
                moving = moving.min(p.picture_ms);
                let band = finish(&mut p, &b, k, size, st, t + REST + 0.01);
                assert_eq!(p.key.as_ref().map(|k| k.samples), Some(4));
                resting = resting.min(p.picture_ms);
                slowest = slowest.min(band);
                t += 1.0;
            }
            assert!(slowest < 3.0, "{name}: a band of {slowest} ms");
            eprintln!(
                "julia panel 360x259, {name}: {moving:.3} ms GPU moving (1 sample), {resting:.3} ms resting (2x2; slowest band {slowest:.3} ms)"
            );
        }
    }
}
