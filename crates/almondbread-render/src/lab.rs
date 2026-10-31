//! The parameter lab on the GPU (F10, kb: plans/f10-parameter-lab, `almondbread_core::lab`):
//! the kernels (`shaders/lab.metal`, a library of its own, compiled when the lab is first
//! entered, so nothing of it runs or compiles otherwise and the Mandelbrot pipelines do not
//! change), their parameters, and the three plane panels of the overlay row (c, z₀, p), drawn as
//! the Julia panel is: small renderers of their own, one picture each, a marker at the current
//! value. The main view's field is in `scheduler/lab_frame.rs` (it goes through the main colour
//! pass).
//!
//! Panel update policy (as the Julia panel's): a panel shows its plane with the other two knobs
//! fixed, so it gets a new picture when one of those (or its size, or the colours) changes, never
//! for its own knob (only its marker moves). One sample per pixel while the knobs move; once
//! they rest for [`REST`], a 2×2 picture, in bands of rows when the one-sample picture was slow.

use almondbread_core::lab::{self, LabValues, Plane};
use almondbread_core::settings::RenderSettings;
use almondbread_core::viewport::Viewport;
use bytemuck::{Pod, Zeroable};

use crate::backend::{
    ComputeEncoder, GpuBackend, GpuError, MathMode, Storage, Submission, SubmissionStatus, TextureFormat,
};

/// The lab's kernel library.
pub const LAB_SOURCE: &str = include_str!("shaders/lab.metal");
const FUNCTIONS: [&str; 3] = ["labField", "labPanel", "labLayout"];
const PALETTE_SIZE: usize = 1024;

/// `LabParams` in `lab.metal`, 144 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct LabParams {
    pub interior: [f32; 4],
    pub z0: [f32; 2],
    pub c: [f32; 2],
    pub p: [f32; 2],
    pub center_hi: [f32; 2],
    pub center_lo: [f32; 2],
    pub step_x: [f32; 2],
    pub step_y: [f32; 2],
    pub plane: u32,
    pub width: u32,
    pub height: u32,
    pub max_iter: u32,
    pub int_power: u32,
    pub row0: u32,
    pub samples: u32,
    pub palette_size: u32,
    pub bailout2: f32,
    pub cycle_eps2: f32,
    pub density: f32,
    pub offset: f32,
    pub log_scale: u32,
    pub smooth_color: u32,
    pub inv_texel: f32,
    pub pad: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<LabParams>() == 144);

/// The compiled kernels (cheap to clone: the panels share the main view's).
pub struct LabKernels<B: GpuBackend> {
    pub field: B::Pipeline,
    pub panel: B::Pipeline,
    pub layout: B::Pipeline,
}

impl<B: GpuBackend> Clone for LabKernels<B> {
    fn clone(&self) -> Self {
        Self {
            field: self.field.clone(),
            panel: self.panel.clone(),
            layout: self.layout.clone(),
        }
    }
}

impl<B: GpuBackend> LabKernels<B> {
    pub fn compile(backend: &B) -> Result<Self, GpuError> {
        let mut p = backend
            .library_pipelines(LAB_SOURCE, &FUNCTIONS, MathMode::Fast)?
            .into_iter();
        let mut next = || p.next().expect("one pipeline per function");
        Ok(Self {
            field: next(),
            panel: next(),
            layout: next(),
        })
    }
}

fn pair(v: (f64, f64)) -> [f32; 2] {
    [v.0 as f32, v.1 as f32]
}

/// `x` as float32 hi + lo.
fn split(x: f64) -> (f32, f32) {
    let hi = x as f32;
    (hi, (x - f64::from(hi)) as f32)
}

/// The parameters of a `size` texel picture of `plane` around `center` (f64), `texel` plane
/// units per texel, turned by the view's `rotation` (cos, sin), the iteration limit
/// `max_iter`. Colours (panels) are filled in by [`with_colours`].
pub fn params(
    values: &LabValues,
    plane: Plane,
    center: (f64, f64),
    rotation: (f64, f64),
    texel: f64,
    size: (usize, usize),
    max_iter: usize,
) -> LabParams {
    let v = values.sanitized();
    let (hx, lx) = split(center.0);
    let (hy, ly) = split(center.1);
    let (c, s) = rotation;
    // Screen x right, y down; the plane's y up: a texel down is (−sin, −cos) turned... the
    // offset (dx, −dy) turned by the angle.
    let step_x = [(c * texel) as f32, (s * texel) as f32];
    let step_y = [(s * texel) as f32, (-c * texel) as f32];
    LabParams {
        z0: pair(v.z0),
        c: pair(v.c),
        p: pair(v.p),
        center_hi: [hx, hy],
        center_lo: [lx, ly],
        step_x,
        step_y,
        plane: plane.index() as u32,
        width: size.0 as u32,
        height: size.1 as u32,
        max_iter: max_iter as u32,
        int_power: lab::integer_power(v.p).unwrap_or(0),
        bailout2: (lab::BAILOUT * lab::BAILOUT) as f32,
        cycle_eps2: (lab::CYCLE_EPS * lab::CYCLE_EPS) as f32,
        inv_texel: (1.0 / texel) as f32,
        density: 0.02,
        samples: 1,
        palette_size: PALETTE_SIZE as u32,
        smooth_color: 1,
        interior: [0.0, 0.0, 0.0, 1.0],
        ..LabParams::default()
    }
}

/// The main view's field parameters: `view` at `size` texels with `texel_points` view points per
/// texel (a preview: more; 2×2 supersampling: half).
pub fn view_params(
    values: &LabValues,
    plane: Plane,
    view: &Viewport,
    size: (usize, usize),
    texel_points: f64,
    max_iter: usize,
) -> LabParams {
    params(
        values,
        plane,
        (view.center.re(), view.center.im()),
        view.rotation(),
        view.scale.to_f64() * texel_points,
        size,
        max_iter,
    )
}

/// The main view's colours on panel parameters.
pub fn with_colours(mut p: LabParams, s: &RenderSettings) -> LabParams {
    let i = s.interior_color;
    p.interior = [i[0], i[1], i[2], 1.0];
    p.density = s.color_density as f32;
    p.offset = s.color_offset.rem_euclid(1.0) as f32;
    p.log_scale = u32::from(s.log_coloring);
    p.smooth_color = u32::from(s.smooth_coloring);
    p
}

/// A panel's picture of `plane` at `size` pixels (its fixed framing).
pub fn panel_params(values: &LabValues, plane: Plane, size: (usize, usize), s: &RenderSettings) -> LabParams {
    let home = lab::home_view(plane, size.0 as f64, size.1 as f64);
    let max_iter = lab::iterations(s.auto_iterations, s.manual_iterations, home.scale.to_f64());
    with_colours(
        params(
            values,
            plane,
            (home.center.re(), home.center.im()),
            (1.0, 0.0),
            home.scale.to_f64(),
            size,
            max_iter,
        ),
        s,
    )
}

/// Where `v` is in a `size` panel of `plane`: pixels from its top left.
pub fn panel_position(plane: Plane, size: (usize, usize), v: (f64, f64)) -> (f64, f64) {
    let home = lab::home_view(plane, size.0 as f64, size.1 as f64);
    let s = home.scale.to_f64();
    (
        size.0 as f64 / 2.0 + (v.0 - home.center.re()) / s,
        size.1 as f64 / 2.0 - (v.1 - home.center.im()) / s,
    )
}

/// The value at pixel position (x, y) of a `size` panel of `plane` (the inverse of
/// [`panel_position`]).
pub fn panel_value(plane: Plane, size: (usize, usize), x: f64, y: f64) -> (f64, f64) {
    let home = lab::home_view(plane, size.0 as f64, size.1 as f64);
    let s = home.scale.to_f64();
    (
        home.center.re() + (x - size.0 as f64 / 2.0) * s,
        home.center.im() - (y - size.1 as f64 / 2.0) * s,
    )
}

/// Encodes a band of the main view's field: rows `p.row0 .. p.row0 + rows` into `iter`
/// (rg32Float) and `de` (rg16Float).
pub fn encode_field<B: GpuBackend>(
    enc: &mut B::Encoder,
    kernels: &LabKernels<B>,
    p: &LabParams,
    rows: u32,
    iter: &B::Texture,
    de: &B::Texture,
) {
    enc.set_pipeline(&kernels.field);
    enc.set_bytes(0, bytemuck::bytes_of(p));
    enc.set_texture(0, iter);
    enc.set_texture(1, de);
    enc.dispatch_threads([p.width as usize, rows as usize, 1], [16, 16, 1]);
}

/// Seconds the knobs must rest before the panels' 2×2 pictures.
pub const REST: f64 = 0.2;
/// A panel's 2×2 picture is drawn in bands of about this much GPU time (ms), one per refresh.
pub const BAND_MS: f64 = 1.0;
pub const MAX_BANDS: u32 = 8;

/// What a panel picture depends on: the other two knobs, its size, the samples and colours.
#[derive(Clone, Debug, PartialEq)]
struct PanelKey {
    params: LabParams,
    samples: u32,
    palette: String,
}

/// One plane's panel.
pub struct PanelPicture<B: GpuBackend> {
    target: Option<(B::Texture, (usize, usize))>,
    key: Option<PanelKey>,
    pending: Option<B::Submission>,
    /// The knobs (without the panel's own) as of the last change, and when they changed.
    moved: Option<(LabParams, f64)>,
    refine: Option<(LabParams, u32, u32)>,
    /// A picture was submitted (the panel shows it).
    pub shown: bool,
    pub serial: u64,
    /// GPU milliseconds of the current picture so far (all its bands).
    pub picture_ms: f64,
}

impl<B: GpuBackend> Default for PanelPicture<B> {
    fn default() -> Self {
        Self {
            target: None,
            key: None,
            pending: None,
            moved: None,
            refine: None,
            shown: false,
            serial: 0,
            picture_ms: 0.0,
        }
    }
}

impl<B: GpuBackend> PanelPicture<B> {
    pub fn texture(&self) -> Option<&B::Texture> {
        self.target.as_ref().map(|(t, _)| t)
    }

    fn busy(&self) -> bool {
        self.pending.is_some() || self.refine.is_some() || self.key.as_ref().is_some_and(|k| k.samples < 4)
    }

    fn poll(&mut self) {
        if let Some(sub) = &self.pending {
            match sub.status() {
                SubmissionStatus::Pending => {}
                SubmissionStatus::Completed => {
                    self.picture_ms += sub.gpu_time() * 1000.0;
                    self.pending = None;
                }
                SubmissionStatus::Failed(e) => {
                    eprintln!("almondbread: lab panel: {e}");
                    self.pending = None;
                    self.key = None;
                    self.refine = None;
                }
            }
        }
    }
}

/// The three panels (c, z₀, p in [`Plane::ALL`] order).
pub struct LabPanels<B: GpuBackend> {
    pub panels: [PanelPicture<B>; 3],
    palette: Option<(String, B::Buffer)>,
}

impl<B: GpuBackend> Default for LabPanels<B> {
    fn default() -> Self {
        Self {
            panels: Default::default(),
            palette: None,
        }
    }
}

fn palette_key(s: &RenderSettings) -> String {
    match &s.palette_stops {
        Some(stops) if !stops.is_empty() => format!("stops:{stops:?}"),
        _ => s.palette_id.clone(),
    }
}

impl<B: GpuBackend> LabPanels<B> {
    /// Some picture is being drawn, or a 2×2 one is still due.
    pub fn busy(&self) -> bool {
        self.panels.iter().any(PanelPicture::busy)
    }

    /// The lab is off: frees the pictures.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Draws new pictures where their knobs, size or colours changed (one submission in flight
    /// per panel).
    pub fn update(
        &mut self,
        backend: &B,
        kernels: &LabKernels<B>,
        values: &LabValues,
        size: (usize, usize),
        settings: &RenderSettings,
        now: f64,
    ) -> Result<(), GpuError> {
        if size.0 == 0 || size.1 == 0 {
            return Ok(());
        }
        let pk = palette_key(settings);
        if self.palette.as_ref().is_none_or(|(p, _)| *p != pk) {
            let table = settings.palette_table(PALETTE_SIZE);
            let buf = backend.create_buffer_with_data(bytemuck::cast_slice(&table))?;
            self.palette = Some((pk.clone(), buf));
        }
        let (_, palette) = self.palette.as_ref().expect("palette");
        for plane in Plane::ALL {
            let panel = &mut self.panels[plane.index()];
            panel.poll();
            // The panel's own knob does not change its picture.
            let mut fixed = *values;
            fixed.set(plane, LabValues::default().get(plane));
            let base = panel_params(&fixed, plane, size, settings);
            match panel.moved {
                Some((m, _)) if m == base => {}
                Some(_) => panel.moved = Some((base, now)),
                None => panel.moved = Some((base, f64::NEG_INFINITY)),
            }
            let resting = panel.moved.is_some_and(|(_, t)| now - t >= REST);
            let samples = if resting { 4 } else { 1 };
            let key = PanelKey {
                params: base,
                samples,
                palette: pk.clone(),
            };
            if panel.pending.is_some() {
                continue;
            }
            if panel.key.as_ref() == Some(&key) {
                if let Some((p, row, band)) = panel.refine {
                    dispatch(backend, kernels, palette, panel, p, row, band)?;
                }
                continue;
            }
            if panel.target.as_ref().is_none_or(|(_, s)| *s != size) {
                let t = backend.create_texture(size.0, size.1, TextureFormat::Bgra8Unorm, Storage::Private)?;
                panel.target = Some((t, size));
                panel.shown = false;
            }
            let mut p = base;
            p.samples = samples;
            let single = panel
                .key
                .as_ref()
                .filter(|old| old.samples == 1 && old.params == key.params)
                .map(|_| panel.picture_ms);
            let bands = if samples >= 4 {
                single.map_or(MAX_BANDS, |ms| ((4.0 * ms / BAND_MS).ceil() as u32).clamp(1, MAX_BANDS))
            } else {
                1
            };
            panel.key = Some(key);
            panel.refine = None;
            dispatch(backend, kernels, palette, panel, p, 0, p.height.div_ceil(bands))?;
        }
        Ok(())
    }
}

fn dispatch<B: GpuBackend>(
    backend: &B,
    kernels: &LabKernels<B>,
    palette: &B::Buffer,
    panel: &mut PanelPicture<B>,
    mut p: LabParams,
    row0: u32,
    band: u32,
) -> Result<(), GpuError> {
    let band = band.max(1);
    p.row0 = row0;
    if row0 == 0 {
        panel.picture_ms = 0.0;
    }
    let rows = band.min(p.height - row0);
    let (target, _) = panel.target.as_ref().expect("target");
    let mut enc = backend.begin("lab panel")?;
    enc.set_pipeline(&kernels.panel);
    enc.set_bytes(0, bytemuck::bytes_of(&p));
    enc.set_buffer(1, palette, 0);
    enc.set_texture(0, target);
    enc.dispatch_threads([p.width as usize, rows as usize, 1], [16, 16, 1]);
    panel.pending = Some(enc.commit());
    let next = row0 + rows;
    panel.refine = (next < p.height).then_some((p, next, band));
    panel.shown = true;
    panel.serial += 1;
    Ok(())
}

/// The main view's field of `view` at `w` × `h` texels (one sample per texel, `ppp` view points
/// per texel), read back: the GPU-vs-CPU tests and the precision measurements. Returns the
/// field and the GPU seconds.
#[allow(clippy::too_many_arguments)]
pub fn gpu_field<B: GpuBackend>(
    backend: &B,
    kernels: &LabKernels<B>,
    values: &LabValues,
    plane: Plane,
    view: &Viewport,
    (w, h): (usize, usize),
    ppp: f64,
    max_iter: usize,
) -> Result<(almondbread_core::oracle::IterationField, f64), GpuError> {
    let p = view_params(values, plane, view, (w, h), ppp, max_iter);
    let iter = backend.create_texture(w, h, TextureFormat::Rg32Float, Storage::Private)?;
    let de = backend.create_texture(w, h, TextureFormat::Rg16Float, Storage::Private)?;
    let mut enc = backend.begin("lab field")?;
    encode_field::<B>(&mut enc, kernels, &p, h as u32, &iter, &de);
    let sub = enc.commit();
    sub.wait();
    if let SubmissionStatus::Failed(e) = sub.status() {
        return Err(GpuError(e));
    }
    let mut bytes = vec![0u8; w * h * 8];
    backend.read_texture(&iter, w, h, TextureFormat::Rg32Float, &mut bytes)?;
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

/// `sizeof(LabParams)` as the Metal compiler lays it out.
pub fn msl_params_size<B: GpuBackend>(backend: &B, kernels: &LabKernels<B>) -> Result<u32, GpuError> {
    let out = backend.create_buffer(16, Storage::Shared)?;
    let mut enc = backend.begin("lab layout")?;
    enc.set_pipeline(&kernels.layout);
    enc.set_buffer(0, &out, 0);
    enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
    let sub = enc.commit();
    sub.wait();
    let mut b = [0u8; 4];
    backend.read_buffer(&out, 0, &mut b);
    Ok(u32::from_le_bytes(b))
}

/// How a GPU field compares with the f64 reference: pixels whose class (escaped or not)
/// differs, escaped in both with another escape iteration, and the total.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Agreement {
    pub class: usize,
    pub iteration: usize,
    pub total: usize,
}

impl Agreement {
    pub fn of(cpu: &almondbread_core::oracle::IterationField, gpu: &almondbread_core::oracle::IterationField) -> Self {
        use almondbread_core::oracle::ITER_INTERIOR;
        let mut a = Self {
            total: cpu.n.len(),
            ..Self::default()
        };
        for (&c, &g) in cpu.n.iter().zip(&gpu.n) {
            if (c == ITER_INTERIOR) != (g == ITER_INTERIOR) {
                a.class += 1;
            } else if c != ITER_INTERIOR && (c - g).abs() > 1.0 {
                a.iteration += 1;
            }
        }
        a
    }

    /// Share of pixels that agree in class and (escaped) in iteration to ±1.
    pub fn share(&self) -> f64 {
        1.0 - (self.class + self.iteration) as f64 / self.total.max(1) as f64
    }
}

#[cfg(all(test, target_vendor = "apple"))]
mod tests {
    use super::*;
    use crate::metal::MetalBackend;
    use almondbread_core::oracle::ITER_INTERIOR;

    fn cases() -> Vec<(&'static str, LabValues, Plane)> {
        let d = LabValues::default();
        let x = LabValues {
            z0: (0.12, -0.08),
            c: (-0.4, 0.6),
            p: (2.6, 0.15),
        };
        vec![
            ("c plane, defaults (the Mandelbrot set)", d, Plane::C),
            ("z0 plane, defaults (the neck's Julia set)", d, Plane::Z0),
            ("p plane, defaults (the X-set)", d, Plane::P),
            ("c plane, p = 2.6 + 0.15i", x, Plane::C),
            ("z0 plane, p = 2.6 + 0.15i", x, Plane::Z0),
            ("p plane, z0 = 0.12 - 0.08i, c = -0.4 + 0.6i", x, Plane::P),
        ]
    }

    /// The float32 field against the f64 reference at each plane's home view (plan F10
    /// "Verification"): the class agrees for all but a few pixels next to the boundary, the
    /// escape iteration (±1) almost everywhere.
    #[test]
    fn gpu_field_matches_the_cpu_reference() {
        let b = MetalBackend::new().expect("metal");
        let k = LabKernels::compile(&b).expect("lab library");
        assert_eq!(msl_params_size(&b, &k).expect("layout"), 144);
        let (w, h) = (320, 220);
        for (name, v, plane) in cases() {
            let view = lab::home_view(plane, w as f64, h as f64);
            let cpu = lab::cpu_field(&v, plane, &view, w, h, 1.0, 500);
            gpu_field(&b, &k, &v, plane, &view, (w, h), 1.0, 500).expect("field");
            let (gpu, secs) = gpu_field(&b, &k, &v, plane, &view, (w, h), 1.0, 500).expect("field");
            let a = Agreement::of(&cpu, &gpu);
            let inside = cpu.n.iter().filter(|&&n| n == ITER_INTERIOR).count();
            eprintln!(
                "lab {name}: interior {:.1} %, class differs {}, iteration differs {}, agree {:.3} %; GPU {:.3} ms",
                100.0 * inside as f64 / a.total as f64,
                a.class,
                a.iteration,
                100.0 * a.share(),
                secs * 1000.0
            );
            assert!(a.class * 200 <= a.total, "{name}: {a:?}");
            assert!(a.share() >= 0.97, "{name}: {a:?}");
        }
    }

    /// A point on the boundary of the set in `plane` (bisection between an interior and an
    /// escaping point of the home view, f64).
    fn boundary(v: &LabValues, plane: Plane) -> (f64, f64) {
        let view = lab::home_view(plane, 64.0, 44.0);
        let f = lab::cpu_field(v, plane, &view, 64, 44, 1.0, 1000);
        let (cx, cy) = (view.center.re(), view.center.im());
        let near = |i: usize| {
            let p = lab::pixel_value(&view, (i % 64) as f64, (i / 64) as f64, 64, 44, 1.0);
            ((p.0 - cx).hypot(p.1 - cy), p)
        };
        let pick = |want: bool| {
            (0..f.n.len())
                .filter(|&i| (f.n[i] == ITER_INTERIOR) == want)
                .map(near)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .expect("pixel")
                .1
        };
        let (mut a, mut b) = (pick(true), pick(false));
        for _ in 0..60 {
            let m = (0.5 * (a.0 + b.0), 0.5 * (a.1 + b.1));
            if lab::escape(v, plane, m, 1000).0 == ITER_INTERIOR {
                a = m;
            } else {
                b = m;
            }
        }
        a
    }

    /// Where float32 stops being good enough (the zoom limit, kb: plans/f10-parameter-lab "Zoom
    /// limit"): at a boundary point of each case, fields at pixel sizes from 1e-3 down; the GPU's
    /// agreement with the f64 reference next to the reference's agreement with itself shifted by
    /// a quarter pixel (near the boundary the picture is chaotic: what a quarter-pixel shift
    /// changes is not a float32 error). The limit `lab::min_pixel` is where the GPU still
    /// disagrees less than that shift.
    #[test]
    fn precision_limit() {
        let b = MetalBackend::new().expect("metal");
        let k = LabKernels::compile(&b).expect("lab library");
        let (w, h) = (200, 140);
        for (name, v, plane) in cases() {
            let at = boundary(&v, plane);
            let limit = lab::min_pixel(at);
            let mut line = format!("lab precision {name} at {:.6} {:+.6}i:", at.0, at.1);
            let mut at_limit = None;
            // Escaped neighbours (right) with the very same smooth count: float32 coordinates
            // that rounded to the same value (blocks).
            let same = |f: &almondbread_core::oracle::IterationField| {
                let (mut n, mut eq) = (0usize, 0usize);
                for y in 0..h {
                    for x in 0..w - 1 {
                        let (i, j) = (y * w + x, y * w + x + 1);
                        if f.n[i] >= 0.0 && f.n[j] >= 0.0 {
                            n += 1;
                            eq += usize::from(f.n[i] == f.n[j] && f.s[i] == f.s[j]);
                        }
                    }
                }
                eq as f64 / n.max(1) as f64
            };
            for pixel in [1e-4, 1e-5, 3e-6, 1e-6, 3e-7, 1e-7, 3e-8, limit] {
                let view = Viewport::new(at.0, at.1, pixel);
                let cpu = lab::cpu_field(&v, plane, &view, w, h, 1.0, 1000);
                let shifted = Viewport::new(at.0 + 0.25 * pixel, at.1 + 0.25 * pixel, pixel);
                let cpu_s = lab::cpu_field(&v, plane, &shifted, w, h, 1.0, 1000);
                let (gpu, _) = gpu_field(&b, &k, &v, plane, &view, (w, h), 1.0, 1000).expect("field");
                let g = Agreement::of(&cpu, &gpu).share();
                let q = Agreement::of(&cpu, &cpu_s).share();
                let blocks = same(&gpu);
                line.push_str(&format!(
                    "\n  {pixel:.1e}{}: agree {:.1} % (quarter-pixel shift {:.1} %), same neighbours {:.1} % (f64 {:.1} %)",
                    if pixel == limit { " (limit)" } else { "" },
                    100.0 * g,
                    100.0 * q,
                    100.0 * blocks,
                    100.0 * same(&cpu)
                ));
                if pixel == limit {
                    at_limit = Some((g, q, blocks));
                }
            }
            eprintln!("{line}");
            let (g, q, blocks) = at_limit.expect("limit");
            assert!(
                g + 0.02 >= q,
                "{name}: at the limit the GPU agrees {g:.3}, a quarter pixel {q:.3}"
            );
            assert!(
                blocks < 0.02,
                "{name}: {:.1} % of the neighbours equal at the limit",
                100.0 * blocks
            );
        }
    }

    /// Panels: a new picture only when another knob moves (its own knob only moves the marker),
    /// one sample while moving and 2x2 at rest; GPU times at Retina panel size.
    #[test]
    fn panels_redraw_when_the_other_knobs_move() {
        let b = MetalBackend::new().expect("metal");
        let k = LabKernels::compile(&b).expect("lab library");
        let mut p = LabPanels::<MetalBackend>::default();
        let s = RenderSettings::default();
        let size = (360, 259);
        let settle = |p: &mut LabPanels<MetalBackend>| {
            for _ in 0..10_000 {
                p.panels.iter_mut().for_each(PanelPicture::poll);
                if p.panels.iter().all(|x| x.pending.is_none()) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        };
        let mut v = LabValues::default();
        // (The first pictures arrive at rest: 2x2 in bands, all drawn first.)
        for _ in 0..20 {
            p.update(&b, &k, &v, size, &s, 0.0).expect("update");
            settle(&mut p);
        }
        assert!(!p.busy());
        let serials = |p: &LabPanels<MetalBackend>| p.panels.each_ref().map(|x| x.serial);
        let first = serials(&p);
        assert!(first.iter().all(|&n| n >= 1));
        // Moving c: the c panel keeps its picture, z0 and p get new ones.
        v.c = (-0.7, 0.1);
        p.update(&b, &k, &v, size, &s, 1.0).expect("update");
        settle(&mut p);
        let after = serials(&p);
        assert_eq!(after[0], first[0]);
        assert!(after[1] > first[1] && after[2] > first[2]);
        // At rest: 2x2, in bands.
        let mut t = 1.0 + REST + 0.01;
        for _ in 0..40 {
            p.update(&b, &k, &v, size, &s, t).expect("update");
            settle(&mut p);
            t += 0.01;
        }
        assert!(!p.busy());
        for plane in Plane::ALL {
            let x = &p.panels[plane.index()];
            eprintln!(
                "lab panel {} 360x259: {:.3} ms GPU (2x2, all bands)",
                plane.name(),
                x.picture_ms
            );
        }
        // Positions and values round-trip.
        let (x, y) = panel_position(Plane::P, size, (2.3, -0.4));
        let back = panel_value(Plane::P, size, x, y);
        assert!((back.0 - 2.3).abs() < 1e-9 && (back.1 + 0.4).abs() < 1e-9);
    }
}
