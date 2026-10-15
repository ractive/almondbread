//! M7: float-float (2 x f32 + exponent) kernels in a library of their own (kb:
//! techniques/double-float, questions/double-float-speed-on-m1).
//!
//! M7 first measured whether float32 rounding artefacts exist and found none visible (kb:
//! questions/float32-error-long-orbits, "M7"): the GPU's error is smaller than what moving the
//! pixel grid by 0.001 to 0.2 pixel does to the oracle itself. So this module is scaffolding
//! with no runtime cost: nothing in the renderer, the app or `make bench` touches it. It
//! gives
//!
//! - the separate library, compiled with `MathMode::Safe` through
//!   [`GpuBackend::library_pipelines`] only when a [`FloatFloat`] is created;
//! - the known-answer test ([`FloatFloat::known_answer_test`]) that [`FloatFloat::new`] runs
//!   before handing the pipelines out; when it fails, `new` returns an error and callers
//!   stay on float32 (the fallback);
//! - [`FloatFloat::iterate_pixels`]: perturbation of a *list* of pixels with float-float
//!   deltas and reference (any depth: one exponent per complex value), resumable in
//!   budgeted command buffers. A list, so a future repair pass can re-iterate flagged
//!   pixels only; [`FloatFloat::render_view`] runs every pixel of a view (`verify --ff 1`,
//!   `precision --ff 1`);
//! - throughput benchmarks of float32, float-float and fx ([`FloatFloat::bench`]).

use bytemuck::{Pod, Zeroable};

use crate::backend::{ComputeEncoder, GpuBackend, GpuError, MathMode, Submission, SubmissionStatus};
use almondbread_core::math::{DoubleExp, DoubleExpComplex};
use almondbread_core::oracle::{ITER_UNRESOLVED, IterationField, OracleReference};
use almondbread_core::viewport::Viewport;

/// The float-float kernels (kb: techniques/double-float). Never compiled into the fast library.
pub const FF_SOURCE: &str = include_str!("shaders/floatfloat.metal");

const FUNCTIONS: [&str; 5] = ["ffKnownAnswer", "ffIterate", "ffBenchF32", "ffBenchFF", "ffBenchFX"];

/// Exponent of a zero fx value.
pub const FX_ZERO: i32 = -(1 << 28);

/// A complex fx value `(re.0 + re.1 + i (im.0 + im.1)) 2^e`, max(|re.0|, |im.0|) in [0.5, 1)
/// (the kernel's `FxRef`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FxRef {
    pub re: [f32; 2],
    pub im: [f32; 2],
    pub e: i32,
    pub pad: i32,
}

/// `x = 2^k f` with f in [0.5, 1) for finite x != 0.
fn frexp(x: f64) -> (f64, i64) {
    let bits = x.to_bits();
    let be = ((bits >> 52) & 0x7ff) as i64;
    if be == 0 {
        let (f, k) = frexp(x * 2f64.powi(64));
        return (f, k - 64);
    }
    (f64::from_bits((bits & !(0x7ff << 52)) | (1022 << 52)), be - 1022)
}

fn ldexp(x: f64, k: i64) -> f64 {
    // Two steps keep the factor in range for |k| up to ~2000.
    let h = (k / 2).clamp(-1022, 1023) as i32;
    x * 2f64.powi(h) * 2f64.powi((k - h as i64).clamp(-1074, 1023) as i32)
}

fn split(x: f64) -> [f32; 2] {
    let hi = x as f32;
    [hi, (x - hi as f64) as f32]
}

impl FxRef {
    pub const ZERO: FxRef = FxRef {
        re: [0.0; 2],
        im: [0.0; 2],
        e: FX_ZERO,
        pad: 0,
    };

    /// `(re + i im) 2^e`, rounded to float-float (48 bits).
    pub fn from_scaled(re: f64, im: f64, e: i64) -> Self {
        let a = re.abs().max(im.abs());
        if a == 0.0 || !a.is_finite() {
            return Self::ZERO;
        }
        let (_, k) = frexp(a);
        let (re, im) = (ldexp(re, -k), ldexp(im, -k));
        Self {
            re: split(re),
            im: split(im),
            e: (e + k) as i32,
            pad: 0,
        }
    }

    pub fn from_double_exp(z: DoubleExpComplex) -> Self {
        let exp = |d: DoubleExp| if d.is_zero() { i64::MIN / 4 } else { d.exponent };
        let e = exp(z.re).max(exp(z.im));
        if e == i64::MIN / 4 {
            return Self::ZERO;
        }
        let part = |d: DoubleExp| {
            if d.is_zero() {
                0.0
            } else {
                ldexp(d.mantissa, (d.exponent - e).max(-1100))
            }
        };
        Self::from_scaled(part(z.re), part(z.im), e)
    }

    /// (re, im) mantissas as f64 and the exponent.
    pub fn parts(&self) -> (f64, f64, i64) {
        (
            self.re[0] as f64 + self.re[1] as f64,
            self.im[0] as f64 + self.im[1] as f64,
            self.e as i64,
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct FfParams {
    count: u32,
    max_iter: u32,
    budget: u32,
    last: u32,
    bailout2: f32,
    pad: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct FfState {
    dre: [f32; 2],
    dim: [f32; 2],
    de: i32,
    m: u32,
    n: u32,
    done: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct BenchParams {
    steps: u32,
    cre: f32,
    cim: f32,
    spread: f32,
}

const _: () = assert!(std::mem::size_of::<FxRef>() == 24);
const _: () = assert!(std::mem::size_of::<FfState>() == 32);
const _: () = assert!(std::mem::size_of::<FfParams>() == 32);

/// One check of the known-answer test.
#[derive(Clone, Debug)]
pub struct Check {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

/// Result of [`FloatFloat::known_answer_test`].
#[derive(Clone, Debug)]
pub struct KnownAnswer {
    pub math: MathMode,
    pub checks: Vec<Check>,
}

impl KnownAnswer {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }

    pub fn summary(&self) -> String {
        let lines: Vec<String> = self
            .checks
            .iter()
            .map(|c| format!("  {} {}: {}", if c.passed { "ok  " } else { "FAIL" }, c.name, c.detail))
            .collect();
        format!(
            "float-float known-answer test ({:?} math): {}\n{}",
            self.math,
            if self.passed() { "passed" } else { "FAILED" },
            lines.join("\n")
        )
    }
}

/// What a float-float run cost.
#[derive(Clone, Debug, Default)]
pub struct FfStats {
    pub gpu_time: f64,
    pub dispatches: usize,
    /// Iterations done (escape count, or the limit, per pixel).
    pub iterations: u64,
    /// CPU seconds for the reference orbit (render_view only).
    pub reference_time: f64,
}

/// Number type of [`FloatFloat::bench`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchKind {
    F32,
    FloatFloat,
    Fx,
}

/// The float-float pipelines of one backend.
pub struct FloatFloat<B: GpuBackend> {
    backend: B,
    kat: B::Pipeline,
    iterate: B::Pipeline,
    bench: [B::Pipeline; 3],
    pub math: MathMode,
}

impl<B: GpuBackend + Clone> FloatFloat<B> {
    /// Compiles the library with `math` (no known-answer test: [`Self::new`] is the safe way).
    pub fn compile(backend: &B, math: MathMode) -> Result<Self, GpuError> {
        let mut p = backend.library_pipelines(FF_SOURCE, &FUNCTIONS, math)?.into_iter();
        let mut next = || p.next().expect("one pipeline per function");
        Ok(Self {
            backend: backend.clone(),
            kat: next(),
            iterate: next(),
            bench: [next(), next(), next()],
            math,
        })
    }

    /// The safe library, after its known-answer test passed. An error (compile failure or a
    /// failed test, with the test's report) means: no float-float here, stay on float32.
    pub fn new(backend: &B) -> Result<(Self, KnownAnswer), GpuError> {
        let ff = Self::compile(backend, MathMode::Safe)?;
        let kat = ff.known_answer_test()?;
        if !kat.passed() {
            return Err(GpuError(kat.summary()));
        }
        Ok((ff, kat))
    }

    fn run(&self, label: &str, f: impl FnOnce(&mut B::Encoder)) -> Result<f64, GpuError> {
        let mut enc = self.backend.begin(label)?;
        f(&mut enc);
        let sub = enc.commit();
        sub.wait();
        match sub.status() {
            SubmissionStatus::Failed(e) => Err(GpuError(format!("{label}: {e}"))),
            _ => Ok(sub.gpu_time()),
        }
    }

    fn read_f32(&self, buf: &B::Buffer, count: usize) -> Vec<f32> {
        let mut bytes = vec![0u8; count * 4];
        self.backend.read_buffer(buf, 0, &mut bytes);
        bytemuck::cast_slice(&bytes).to_vec()
    }

    /// Runs `ffKnownAnswer` on inputs from a buffer and checks the outputs on the CPU: do the
    /// compiled TwoSum, QuickTwoSum and TwoProd keep their exact low parts, does float-float
    /// arithmetic carry ~48 bits, and fx exponents beyond float range?
    pub fn known_answer_test(&self) -> Result<KnownAnswer, GpuError> {
        let third = 1.0f64 / 3.0;
        let (c_re, c_im) = (-0.5f64, 0.3f64); // main cardioid: attracting fixed point, |2z| ~ 0.84
        let (cr, ci) = (split(c_re), split(c_im));
        let one12 = 1.0f32 + 2f32.powi(-12);
        let input: Vec<f32> = vec![
            1.0,
            1e-8,
            2f32.powi(-30),
            one12,
            split(third)[0],
            split(third)[1],
            3.0,
            cr[0],
            cr[1],
            ci[0],
            ci[1],
            0.75,
            -0.5,
        ];
        let inp = self.backend.create_buffer_with_data(bytemuck::cast_slice(&input))?;
        let out = self.backend.create_buffer_with_data(&[0u8; 4 * 20])?;
        self.run("ff known answer", |enc| {
            enc.set_pipeline(&self.kat);
            enc.set_buffer(0, &inp, 0);
            enc.set_buffer(1, &out, 0);
            enc.dispatch_threads([1, 1, 1], [1, 1, 1]);
        })?;
        let o = self.read_f32(&out, 20);
        let mut checks = Vec::new();
        let mut check = |name: &'static str, passed: bool, detail: String| checks.push(Check { name, passed, detail });
        check(
            "TwoSum(1, 1e-8)",
            o[0] == 1.0 && o[1] == 1e-8f32,
            format!("({:e}, {:e}), want (1, 1e-8)", o[0], o[1]),
        );
        check(
            "QuickTwoSum(1, 2^-30)",
            o[2] == 1.0 && o[3] == 2f32.powi(-30),
            format!("({:e}, {:e}), want (1, 2^-30)", o[2], o[3]),
        );
        check(
            "TwoProd(1+2^-12, 1+2^-12)",
            o[4] == 1.0 + 2f32.powi(-11) && o[5] == 2f32.powi(-24),
            format!("({:e}, {:e}), want (1+2^-11, 2^-24)", o[4], o[5]),
        );
        let prod = o[6] as f64 + o[7] as f64;
        check(
            "ff(1/3) * 3",
            (prod - 1.0).abs() < 2f64.powi(-44),
            format!("1 {:+e}, want |err| < 2^-44", prod - 1.0),
        );
        // f64 orbit with the same c (hi + lo exactly).
        let (c64r, c64i) = (cr[0] as f64 + cr[1] as f64, ci[0] as f64 + ci[1] as f64);
        let (mut zr, mut zi) = (0.0f64, 0.0f64);
        for _ in 0..1000 {
            (zr, zi) = (zr * zr - zi * zi + c64r, 2.0 * zr * zi + c64i);
        }
        let ff_err = (o[8] as f64 + o[9] as f64 - zr).hypot(o[10] as f64 + o[11] as f64 - zi);
        let f32_err = (o[12] as f64 - zr).hypot(o[13] as f64 - zi);
        check(
            "1000-step ff orbit vs f64",
            ff_err < 1e-12,
            format!("|err| {ff_err:.2e} (float32 {f32_err:.2e}), want < 1e-12"),
        );
        check(
            "fx a^2 + a at 2^-1000",
            o[14] == 0.75 && o[15] == 0.0 && o[16] == -0.5 && o[17] == 0.0 && o[18] == -1000.0,
            format!("({}, {}) 2^{}, want (0.75, -0.5) 2^-1000", o[14], o[16], o[18]),
        );
        Ok(KnownAnswer {
            math: self.math,
            checks,
        })
    }

    /// Perturbation of `dcs.len()` pixels (dc relative to the reference point, fx) against
    /// `reference` (Z_0 = 0 ... Z_last, fx) with rebasing, to `max_iter`, in command buffers
    /// of about `chunk_seconds` GPU time. Returns (n, s) per pixel in the iteration-field
    /// encoding (n = -1: the limit).
    pub fn iterate_pixels(
        &self,
        reference: &[FxRef],
        dcs: &[FxRef],
        max_iter: usize,
        bailout: f64,
        chunk_seconds: f64,
    ) -> Result<(Vec<[f32; 2]>, FfStats), GpuError> {
        let count = dcs.len();
        let mut stats = FfStats::default();
        if count == 0 || reference.is_empty() {
            return Ok((Vec::new(), stats));
        }
        let refs = self.backend.create_buffer_with_data(bytemuck::cast_slice(reference))?;
        let dcb = self.backend.create_buffer_with_data(bytemuck::cast_slice(dcs))?;
        let init = vec![
            FfState {
                de: FX_ZERO,
                ..Default::default()
            };
            count
        ];
        let states = self.backend.create_buffer_with_data(bytemuck::cast_slice(&init))?;
        let out = self
            .backend
            .create_buffer_with_data(bytemuck::cast_slice(&vec![[ITER_UNRESOLVED, 0.0f32]; count]))?;
        let (simd, max_threads) = self.backend.pipeline_limits(&self.iterate);
        let group = (simd * 4).min(max_threads).max(1);
        let mut budget = 2048u32;
        loop {
            let active = self.backend.create_buffer_with_data(&[0u8; 4])?;
            let params = FfParams {
                count: count as u32,
                max_iter: max_iter as u32,
                budget,
                last: (reference.len() - 1) as u32,
                bailout2: (bailout * bailout) as f32,
                pad: [0; 3],
            };
            let t = self.run("ff iterate", |enc| {
                enc.set_pipeline(&self.iterate);
                enc.set_bytes(0, bytemuck::bytes_of(&params));
                enc.set_buffer(1, &refs, 0);
                enc.set_buffer(2, &dcb, 0);
                enc.set_buffer(3, &states, 0);
                enc.set_buffer(4, &out, 0);
                enc.set_buffer(5, &active, 0);
                enc.dispatch_threads([count, 1, 1], [group, 1, 1]);
            })?;
            stats.gpu_time += t;
            stats.dispatches += 1;
            let mut a = [0u8; 4];
            self.backend.read_buffer(&active, 0, &mut a);
            if u32::from_le_bytes(a) == 0 {
                break;
            }
            // Keep each command buffer near the chunk target (watchdog, kb:
            // platform/render-architecture).
            let scale = (chunk_seconds / t.max(1e-4)).clamp(0.25, 4.0);
            budget = ((budget as f64 * scale) as u32).clamp(64, 1 << 20);
        }
        let mut bytes = vec![0u8; count * 8];
        self.backend.read_buffer(&out, 0, &mut bytes);
        let field: Vec<[f32; 2]> = bytemuck::cast_slice(&bytes).to_vec();
        stats.iterations = field
            .iter()
            .map(|t| if t[0] >= 0.0 { t[0] as u64 } else { max_iter as u64 })
            .sum();
        Ok((field, stats))
    }

    /// Every pixel of `view` (centre reference at the oracle's precision, the oracle's pixel
    /// grid): the float-float counterpart of `oracle::cpu_render_deep`.
    pub fn render_view(
        &self,
        view: &Viewport,
        width: usize,
        height: usize,
        max_iter: usize,
        bailout: f64,
    ) -> Result<(IterationField, FfStats), GpuError> {
        let t0 = std::time::Instant::now();
        let r = OracleReference::compute(&view.center, view.scale, width.max(height), max_iter, bailout);
        let reference: Vec<FxRef> = r.extended.iter().map(|z| FxRef::from_double_exp(*z)).collect();
        let reference_time = t0.elapsed().as_secs_f64();
        let dcs = view_offsets(view, width, height);
        let (texels, mut stats) = self.iterate_pixels(&reference, &dcs, max_iter, bailout, 0.05)?;
        stats.reference_time = reference_time;
        Ok((IterationField::from_texels(width, height, &texels), stats))
    }

    /// z = z^2 + c iterations per second of `kind` over `threads` threads of `steps` steps
    /// each (bounded orbits: c in the main cardioid).
    pub fn bench(&self, kind: BenchKind, threads: usize, steps: u32) -> Result<f64, GpuError> {
        let pipeline = &self.bench[kind as usize];
        let out = self
            .backend
            .create_buffer(threads * 4, crate::backend::Storage::Shared)?;
        let params = BenchParams {
            steps,
            cre: -0.1,
            cim: 0.1,
            spread: 1e-4,
        };
        let (simd, max_threads) = self.backend.pipeline_limits(pipeline);
        let group = (simd * 4).min(max_threads).max(1);
        let t = self.run("ff bench", |enc| {
            enc.set_pipeline(pipeline);
            enc.set_bytes(0, bytemuck::bytes_of(&params));
            enc.set_buffer(1, &out, 0);
            enc.dispatch_threads([threads, 1, 1], [group, 1, 1]);
        })?;
        Ok(threads as f64 * steps as f64 / t.max(1e-9))
    }
}

/// The per-pixel offsets from the view centre as fx (the oracle's grid: texel centres, y up,
/// turned by the view angle).
pub fn view_offsets(view: &Viewport, width: usize, height: usize) -> Vec<FxRef> {
    let (tm, te) = view.scale.parts();
    let (rs, rc) = if view.angle == 0.0 {
        (0.0, 1.0)
    } else {
        view.angle.sin_cos()
    };
    let (hw, hh) = (width as f64 / 2.0, height as f64 / 2.0);
    let mut out = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let (ox, oy) = (x as f64 + 0.5 - hw, -(y as f64 + 0.5 - hh));
            let (ox, oy) = if view.angle == 0.0 {
                (ox, oy)
            } else {
                (ox * rc - oy * rs, ox * rs + oy * rc)
            };
            out.push(FxRef::from_scaled(ox * tm, oy * tm, te));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx_conversions() {
        let a = FxRef::from_scaled(3.0, -1.0, 10);
        assert_eq!(a.re, [0.75, 0.0]);
        assert_eq!(a.im, [-0.25, 0.0]);
        assert_eq!(a.e, 12);
        let third = FxRef::from_scaled(1.0 / 3.0, 0.0, -2000);
        let (re, _, e) = third.parts();
        assert_eq!(e, -2001);
        assert!((re - 2.0 / 3.0).abs() < 2f64.powi(-48));
        assert_eq!(FxRef::from_scaled(0.0, 0.0, 5), FxRef::ZERO);
        let z = DoubleExpComplex::new(
            DoubleExp::from_f64(0.5).times_power_of_two(-3000),
            DoubleExp::from_f64(-0.25).times_power_of_two(-3000),
        );
        let f = FxRef::from_double_exp(z);
        assert_eq!((f.re[0], f.im[0], f.e), (0.5, -0.25, -3000));
    }

    #[cfg(target_vendor = "apple")]
    #[test]
    fn known_answer_and_view() {
        use crate::metal::MetalBackend;
        let backend = MetalBackend::new().expect("metal");
        let (ff, kat) = FloatFloat::new(&backend).unwrap_or_else(|e| panic!("{e}"));
        println!("{}", kat.summary());
        // A small deep view against the CPU oracle: float-float must agree far better than
        // a 0.001-pixel grid shift would (kb: questions/float32-error-long-orbits, M7).
        let view = Viewport::with_center(
            almondbread_core::viewport::Center::parse("0", "1").unwrap(),
            almondbread_core::viewport::Scale::parse("1e-300").unwrap(),
        );
        let (w, h, it) = (48, 32, 4000);
        let (field, _) = ff.render_view(&view, w, h, it, 65536.0).expect("ff render");
        let (cpu, _) = almondbread_core::oracle::cpu_render_deep(&view.center, view.scale, w, h, it, 65536.0, 0.0);
        let (report, _) = almondbread_core::oracle::compare(&field, &cpu);
        println!("{}", report.summary());
        assert_eq!(report.false_interior + report.false_exterior, 0);
        assert!(report.escape_differs <= 1, "{}", report.summary());
        assert!(report.median_error < 1e-6, "{}", report.summary());
    }
}
