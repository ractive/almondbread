//! High-precision reference orbit Z_{n+1} = Z_n^2 + C (kb: techniques/reference-orbit-precision).
//!
//! The loop runs on preallocated [`Raw`] registers (no allocation per iteration) and is
//! specialised for 1...16 limbs (64...1024 bits): the registers are `[u64; N]`, so the
//! limb count is a compile-time constant and the Comba products unroll completely. Above
//! 16 limbs the same code runs on `Vec<u64>` registers.
//!
//! Two formulations:
//!   Squarings:       X^2, Y^2, (X+Y)^2;  X' = X^2 - Y^2 + Cx,  Y' = (X+Y)^2 - (X^2+Y^2) + Cy
//!   Multiplications: (X+Y)(X-Y), X*Y;    X' = (X+Y)(X-Y) + Cx, Y' = 2XY + Cy
//! The squaring form costs ~1.5 n^2 limb products against 2 n^2 but needs six additions
//! instead of four, and its Y' has absolute (not relative) error ~ulp(|Z|^2) when XY is
//! small. `Automatic` picks the faster one for the limb count (measured).
//!
//! Precision rule (see [`recommended_precision_bits`]):
//!     bits = ceil(-log2(pixelSpacing) + log2(imageSize) + log2(max(|C|, 1)) + 1) + guard
//! with guard = 48 by default, rounded up to whole 64-bit limbs.

use crate::bigfloat::{BigFloat, raw_double_exp, with_scratch};
use crate::complex::{BigComplex, make_float_exp_complex};
use crate::floatexp::{DoubleExpComplex, FloatExpComplex};
use crate::raw::{self, Raw, Storage};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Variant {
    /// Three squarings per iteration.
    Squarings,
    /// Two general multiplications per iteration.
    Multiplications,
    /// Fastest measured for the limb count.
    #[default]
    Automatic,
}

/// Limb counts up to this use the multiplication form under [`Variant::Automatic`].
pub const AUTOMATIC_MULTIPLICATION_LIMIT: usize = 16;

#[derive(Clone, Debug)]
pub struct OrbitResult {
    /// Points written: Z_0 ... Z_{count-1} (Z_0 = 0). Always >= 1.
    pub count: usize,
    /// The orbit escaped the bailout radius; its last point is the first outside.
    pub escaped: bool,
    /// Stopped early by the cancellation check.
    pub cancelled: bool,
    /// The last point at full precision (to resume or to verify).
    pub final_z: BigComplex,
}

impl OrbitResult {
    /// Iterations performed (count - 1).
    pub fn iterations(&self) -> usize {
        self.count - 1
    }
}

/// Polling and progress for long computations. Called every `check_interval` iterations.
pub struct Control<'a> {
    pub check_interval: usize,
    pub is_cancelled: &'a mut dyn FnMut() -> bool,
    pub progress: &'a mut dyn FnMut(usize),
}

/// Runs `body` with a control that never cancels and reports nothing.
pub fn with_no_control<R>(check_interval: usize, body: impl FnOnce(&mut Control<'_>) -> R) -> R {
    let mut never = || false;
    let mut ignore = |_: usize| {};
    let mut c = Control {
        check_interval,
        is_cancelled: &mut never,
        progress: &mut ignore,
    };
    body(&mut c)
}

/// Bits for a reference in a view with the given pixel spacing (log2) and image size, for
/// |C| <= 2 and 48 guard bits.
pub fn recommended_precision_bits(pixel_spacing_log2: f64, image_size: usize) -> usize {
    recommended_precision_bits_with(pixel_spacing_log2, image_size, 2.0, 48)
}

pub fn recommended_precision_bits_with(
    pixel_spacing_log2: f64,
    image_size: usize,
    center_magnitude: f64,
    guard_bits: usize,
) -> usize {
    let raw = -pixel_spacing_log2
        + log2_int(image_size.max(1) as i64)
        + log2_int(center_magnitude.ceil().max(1.0) as i64)
        + 1.0;
    let bits = raw.ceil() as i64 + guard_bits as i64;
    (bits.max(1) as usize).div_ceil(64).max(1) * 64
}

fn log2_int(v: i64) -> f64 {
    BigFloat::from_i64(v, 64).log2_magnitude()
}

#[derive(Clone, Debug)]
pub struct ReferenceOrbitComputer {
    c: BigComplex,
    precision_bits: usize,
    pub variant: Variant,
}

impl ReferenceOrbitComputer {
    /// C is rounded to `precision_bits` (rounded up to whole limbs).
    pub fn new(c: &BigComplex, precision_bits: usize) -> Self {
        let bits = BigFloat::limb_count_for_bits(precision_bits) * 64;
        ReferenceOrbitComputer {
            c: c.with_precision(bits),
            precision_bits: bits,
            variant: Variant::Automatic,
        }
    }

    pub fn with_variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn c(&self) -> &BigComplex {
        &self.c
    }

    pub fn precision_bits(&self) -> usize {
        self.precision_bits
    }

    pub fn limb_count(&self) -> usize {
        self.precision_bits / 64
    }

    pub fn resolved_variant(&self) -> Variant {
        match self.variant {
            Variant::Automatic if self.limb_count() <= AUTOMATIC_MULTIPLICATION_LIMIT => Variant::Multiplications,
            Variant::Automatic => Variant::Squarings,
            v => v,
        }
    }

    /// Computes the orbit into caller-provided buffers (either may be `None`); each needs
    /// room for `max_iterations + 1` points. Stops at escape (|Z|^2 > bailout^2), after
    /// `max_iterations`, or when `control.is_cancelled()` returns true; it is polled, and
    /// `control.progress` called with the iterations done, every `check_interval`
    /// iterations. No allocation per iteration; safe on any thread.
    ///
    /// Panics if a buffer is shorter than `max_iterations + 1`.
    pub fn compute(
        &self,
        max_iterations: usize,
        bailout: f64,
        mut points: Option<&mut [[f32; 2]]>,
        mut extended: Option<&mut [FloatExpComplex]>,
        control: &mut Control<'_>,
    ) -> OrbitResult {
        let need = max_iterations + 1;
        if let Some(p) = points.as_deref_mut() {
            assert!(p.len() >= need, "orbit point buffer too small");
            p[0] = [0.0, 0.0];
        }
        if let Some(e) = extended.as_deref_mut() {
            assert!(e.len() >= need, "orbit extended buffer too small");
            e[0] = FloatExpComplex::ZERO;
        }
        let job = Job {
            c: &self.c,
            squarings: self.resolved_variant() == Variant::Squarings,
            max_iterations,
            bailout2: bailout * bailout,
        };
        let n = self.limb_count();
        let out = Outputs {
            points,
            extended,
            doubles: None,
        };
        self.dispatch(job, n, out, control)
    }

    fn dispatch(&self, job: Job<'_>, n: usize, out: Outputs<'_>, control: &mut Control<'_>) -> OrbitResult {
        macro_rules! dispatch {
            ($($k:literal)*) => {
                match n {
                    $($k => job.run::<[u64; $k]>(n, out, control),)*
                    _ => job.run::<Vec<u64>>(n, out, control),
                }
            };
        }
        dispatch!(1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16)
    }

    /// The orbit in extended-range f64 (each component its own f64 mantissa and i64 exponent;
    /// [`DoubleExpComplex`]), for CPU oracles that need more than float32 mantissas: one entry
    /// per point, `max_iterations + 1` at most.
    pub fn compute_double_exp(
        &self,
        max_iterations: usize,
        bailout: f64,
        control: &mut Control<'_>,
    ) -> (Vec<DoubleExpComplex>, OrbitResult) {
        let mut d = vec![DoubleExpComplex::ZERO; max_iterations + 1];
        let job = Job {
            c: &self.c,
            squarings: self.resolved_variant() == Variant::Squarings,
            max_iterations,
            bailout2: bailout * bailout,
        };
        let out = Outputs {
            points: None,
            extended: None,
            doubles: Some(&mut d),
        };
        let r = self.dispatch(job, self.limb_count(), out, control);
        d.truncate(r.count);
        (d, r)
    }

    /// Convenience: the orbit as vectors (points, extended, result).
    pub fn compute_vec(
        &self,
        max_iterations: usize,
        bailout: f64,
    ) -> (Vec<[f32; 2]>, Vec<FloatExpComplex>, OrbitResult) {
        let mut pts = vec![[0.0f32; 2]; max_iterations + 1];
        let mut ext = vec![FloatExpComplex::ZERO; max_iterations + 1];
        let r = with_no_control(4096, |c| {
            self.compute(max_iterations, bailout, Some(&mut pts), Some(&mut ext), c)
        });
        pts.truncate(r.count);
        ext.truncate(r.count);
        (pts, ext, r)
    }
}

struct Outputs<'a> {
    points: Option<&'a mut [[f32; 2]]>,
    extended: Option<&'a mut [FloatExpComplex]>,
    doubles: Option<&'a mut [DoubleExpComplex]>,
}

struct Job<'a> {
    c: &'a BigComplex,
    squarings: bool,
    max_iterations: usize,
    bailout2: f64,
}

/// Loads a BigFloat into a register (rounded to the register's limb count if wider).
pub(crate) fn load<S: AsMut<[u64]>>(r: &mut Raw<S>, v: &BigFloat) {
    let n = r.m.as_mut().len();
    let w = v.with_limb_count(n);
    raw::copy(r, w.view());
}

impl Job<'_> {
    fn run<S: Storage>(&self, n: usize, mut out: Outputs<'_>, control: &mut Control<'_>) -> OrbitResult {
        let reg = || Raw::<S>::new_zero(n);
        let (mut x, mut y, mut cx, mut cy) = (reg(), reg(), reg(), reg());
        load(&mut cx, &self.c.re);
        load(&mut cy, &self.c.im);
        let mut t = [reg(), reg(), reg(), reg(), reg(), reg()];
        let interval = control.check_interval.max(1);
        let (mut done, mut escaped, mut cancelled) = (0, false, false);
        with_scratch(n, |s| {
            while done < self.max_iterations {
                let end = self.max_iterations.min(done + interval);
                let (reached, esc) = if self.squarings {
                    orbit_loop_squarings(&mut x, &mut y, &cx, &cy, &mut t, s, done, end, self.bailout2, &mut out)
                } else {
                    orbit_loop_multiplications(&mut x, &mut y, &cx, &cy, &mut t, s, done, end, self.bailout2, &mut out)
                };
                done = reached;
                if esc {
                    escaped = true;
                    break;
                }
                (control.progress)(done);
                if done < self.max_iterations && (control.is_cancelled)() {
                    cancelled = true;
                    break;
                }
            }
        });
        OrbitResult {
            count: done + 1,
            escaped,
            cancelled,
            final_z: BigComplex::new(BigFloat::from_raw(&x), BigFloat::from_raw(&y)),
        }
    }
}

/// Stores Z at index i and tests escape. Returns true if escaped.
#[inline(always)]
fn emit_point<S: AsRef<[u64]>>(x: &Raw<S>, y: &Raw<S>, i: usize, bailout2: f64, out: &mut Outputs<'_>) -> bool {
    let xd = raw::to_f64_fast(x.view());
    let yd = raw::to_f64_fast(y.view());
    if let Some(p) = out.points.as_deref_mut() {
        p[i] = [xd as f32, yd as f32];
    }
    if let Some(e) = out.extended.as_deref_mut() {
        e[i] = make_float_exp_complex(x.view(), y.view());
    }
    if let Some(d) = out.doubles.as_deref_mut() {
        d[i] = DoubleExpComplex::new(raw_double_exp(x.view()), raw_double_exp(y.view()));
    }
    xd * xd + yd * yd > bailout2
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn orbit_loop_squarings<S: Storage>(
    x: &mut Raw<S>,
    y: &mut Raw<S>,
    cx: &Raw<S>,
    cy: &Raw<S>,
    t: &mut [Raw<S>; 6],
    s: &mut [u64],
    start: usize,
    end: usize,
    bailout2: f64,
    out: &mut Outputs<'_>,
) -> (usize, bool) {
    let [x2, y2, sum, s2, nrm, d] = t;
    let mut i = start;
    while i < end {
        raw::sqr(x2, x.view(), s);
        raw::sqr(y2, y.view(), s);
        raw::add(sum, x.view(), y.view(), false, s);
        raw::sqr(s2, sum.view(), s);
        raw::add(nrm, x2.view(), y2.view(), false, s);
        raw::add(d, x2.view(), y2.view(), true, s);
        raw::add(x, d.view(), cx.view(), false, s);
        raw::add(d, s2.view(), nrm.view(), true, s);
        raw::add(y, d.view(), cy.view(), false, s);
        i += 1;
        if emit_point(x, y, i, bailout2, out) {
            return (i, true);
        }
    }
    (i, false)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn orbit_loop_multiplications<S: Storage>(
    x: &mut Raw<S>,
    y: &mut Raw<S>,
    cx: &Raw<S>,
    cy: &Raw<S>,
    t: &mut [Raw<S>; 6],
    s: &mut [u64],
    start: usize,
    end: usize,
    bailout2: f64,
    out: &mut Outputs<'_>,
) -> (usize, bool) {
    let [sum, d, p1, p2, _, _] = t;
    let mut i = start;
    while i < end {
        raw::add(sum, x.view(), y.view(), false, s);
        raw::add(d, x.view(), y.view(), true, s);
        raw::mul(p1, sum.view(), d.view(), s);
        raw::mul(p2, x.view(), y.view(), s);
        raw::add(x, p1.view(), cx.view(), false, s);
        if !p2.zero {
            p2.exp += 1;
        }
        raw::add(y, p2.view(), cy.view(), false, s);
        i += 1;
        if emit_point(x, y, i, bailout2, out) {
            return (i, true);
        }
    }
    (i, false)
}
