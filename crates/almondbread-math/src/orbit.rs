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
use crate::formula::Formula;
use crate::parallel::{self, HelperStart, PARALLEL_MIN_ITERATIONS, PARALLEL_MIN_LIMBS, Shared, StepRegs, StopGuard};
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

/// Whether the orbit squares on three cores (kb: techniques/parallel-reference-orbit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Threads {
    /// One thread (the default: callers that already run many orbits in parallel, such
    /// as the CPU oracle, must not multiply their threads by three).
    #[default]
    Single,
    /// The caller plus two helper threads (squaring form only; the multiplication form
    /// stays single-threaded).
    Three,
    /// Three from [`PARALLEL_MIN_LIMBS`] limbs and [`PARALLEL_MIN_ITERATIONS`] iterations
    /// where the core layout allows ([`parallel::helpers_pay`]: six performance cores, or
    /// eight cores when the layout is unknown), else one (a helper on an efficiency core
    /// would set the pace). Used by the reference worker and Newton, one orbit at a time.
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
    // Capped (2^20 bits, far past the deepest view): a zero or absurd spacing gave i64::MAX
    // and an allocation that never finished (kb: techniques/hang-prevention).
    let bits = (raw.ceil() as i64).saturating_add(guard_bits as i64);
    (bits.clamp(1, 1 << 20) as usize).div_ceil(64).max(1) * 64
}

fn log2_int(v: i64) -> f64 {
    BigFloat::from_i64(v, 64).log2_magnitude()
}

#[derive(Clone, Debug)]
pub struct ReferenceOrbitComputer {
    c: BigComplex,
    precision_bits: usize,
    pub variant: Variant,
    pub threads: Threads,
    /// The iteration (F2): Mandelbrot runs the specialised loops below, any other formula one
    /// generic loop (matched once per computation, outside the limb-specialised loops).
    pub formula: Formula,
}

impl ReferenceOrbitComputer {
    /// C is rounded to `precision_bits` (rounded up to whole limbs).
    pub fn new(c: &BigComplex, precision_bits: usize) -> Self {
        let bits = BigFloat::limb_count_for_bits(precision_bits) * 64;
        ReferenceOrbitComputer {
            c: c.with_precision(bits),
            precision_bits: bits,
            variant: Variant::Automatic,
            threads: Threads::Single,
            formula: Formula::Mandelbrot,
        }
    }

    pub fn with_formula(mut self, formula: Formula) -> Self {
        self.formula = formula;
        self
    }

    pub fn with_variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn with_threads(mut self, threads: Threads) -> Self {
        self.threads = threads;
        self
    }

    /// Whether a computation of `max_iterations` runs on three threads.
    pub fn uses_helpers(&self, max_iterations: usize) -> bool {
        // (The three-thread split is Mandelbrot's X^2, Y^2, (X+Y)^2.)
        if self.resolved_variant() != Variant::Squarings || !self.formula.is_mandelbrot() {
            return false;
        }
        match self.threads {
            Threads::Single => false,
            Threads::Three => true,
            Threads::Automatic => {
                self.limb_count() >= PARALLEL_MIN_LIMBS
                    && max_iterations >= PARALLEL_MIN_ITERATIONS
                    && parallel::helpers_pay()
            }
        }
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
        points: Option<&mut [[f32; 2]]>,
        extended: Option<&mut [FloatExpComplex]>,
        control: &mut Control<'_>,
    ) -> OrbitResult {
        self.compute_from(None, max_iterations, bailout, points, extended, control)
    }

    /// Continues a computation that stopped at `from.count - 1` iterations (a shorter
    /// `max_iterations`, or cancelled) up to `max_iterations`, writing points
    /// `from.count ..` into the same buffers (earlier entries are left alone). The
    /// registers resume from `from.final_z` exactly, so the points equal those of one
    /// `compute(max_iterations)` call (kb: techniques/parallel-reference-orbit: the
    /// reference worker delivers a prefix early this way). An escaped `from` is returned
    /// unchanged.
    pub fn resume(
        &self,
        from: &OrbitResult,
        max_iterations: usize,
        bailout: f64,
        points: Option<&mut [[f32; 2]]>,
        extended: Option<&mut [FloatExpComplex]>,
        control: &mut Control<'_>,
    ) -> OrbitResult {
        if from.escaped || from.iterations() >= max_iterations {
            return from.clone();
        }
        self.compute_from(Some(from), max_iterations, bailout, points, extended, control)
    }

    fn compute_from(
        &self,
        from: Option<&OrbitResult>,
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
        let first = from.map_or(0, |f| f.iterations());
        let job = Job {
            c: &self.c,
            start: from.map(|f| (&f.final_z, first)),
            squarings: self.resolved_variant() == Variant::Squarings,
            helpers: self.uses_helpers(max_iterations - first),
            max_iterations,
            bailout2: bailout * bailout,
            formula: self.formula,
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
            start: None,
            squarings: self.resolved_variant() == Variant::Squarings,
            helpers: self.uses_helpers(max_iterations),
            max_iterations,
            bailout2: bailout * bailout,
            formula: self.formula,
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
    /// Resume from Z after this many iterations (else from Z_0 = 0).
    start: Option<(&'a BigComplex, usize)>,
    squarings: bool,
    /// Square on three threads (squaring form only).
    helpers: bool,
    max_iterations: usize,
    bailout2: f64,
    formula: Formula,
}

/// Loads a BigFloat into a register (rounded to the register's limb count if wider).
pub(crate) fn load<S: AsMut<[u64]>>(r: &mut Raw<S>, v: &BigFloat) {
    let n = r.m.as_mut().len();
    let w = v.with_limb_count(n);
    raw::copy(r, w.view());
}

impl Job<'_> {
    fn run<S: Storage>(&self, n: usize, out: Outputs<'_>, control: &mut Control<'_>) -> OrbitResult {
        if self.helpers && self.squarings {
            let shared = Shared::new(n);
            let regs: [Raw<Vec<u64>>; 4] = core::array::from_fn(|_| Raw::new_zero(n));
            let [mut cx, mut cy, mut zx, mut zy] = regs;
            load(&mut cx, &self.c.re);
            load(&mut cy, &self.c.im);
            if let Some((z, _)) = self.start {
                load(&mut zx, &z.re);
                load(&mut zy, &z.im);
            }
            let start = HelperStart {
                c: [&cx, &cy],
                z: [&zx, &zy],
                first: self.start.map_or(0, |(_, f)| f),
            };
            std::thread::scope(|scope| {
                for role in 1..3 {
                    let (sh, st) = (&shared, &start);
                    std::thread::Builder::new()
                        .name("almondbread-orbit".into())
                        .spawn_scoped(scope, move || parallel::helper::<S>(sh, role, n, st))
                        .expect("orbit helper thread");
                }
                let _stop = StopGuard(&shared);
                self.run_on::<S>(n, out, control, Some(&shared))
            })
        } else {
            self.run_on::<S>(n, out, control, None)
        }
    }

    fn run_on<S: Storage>(
        &self,
        n: usize,
        mut out: Outputs<'_>,
        control: &mut Control<'_>,
        shared: Option<&Shared>,
    ) -> OrbitResult {
        let reg = || Raw::<S>::new_zero(n);
        let (mut x, mut y, mut cx, mut cy) = (reg(), reg(), reg(), reg());
        load(&mut cx, &self.c.re);
        load(&mut cy, &self.c.im);
        let mut t = [reg(), reg(), reg(), reg(), reg(), reg()];
        // n limbs even when unused: fixed-size storage requires it (debug_assert in Storage::zeroed).
        let mut regs = StepRegs::<S>::new(n);
        let interval = control.check_interval.max(1);
        let (mut done, mut escaped, mut cancelled) = (0, false, false);
        if let Some((z, first)) = self.start {
            load(&mut x, &z.re);
            load(&mut y, &z.im);
            done = first;
        }
        with_scratch(n, |s| {
            while done < self.max_iterations {
                let end = self.max_iterations.min(done + interval);
                let (reached, esc) = if let Formula::Multibrot(p) = self.formula {
                    orbit_loop_power(
                        &mut x,
                        &mut y,
                        &cx,
                        &cy,
                        &mut t,
                        s,
                        done,
                        end,
                        self.bailout2,
                        &mut out,
                        p,
                    )
                } else if let Some(sh) = shared {
                    orbit_loop_squarings_shared(
                        &mut x,
                        &mut y,
                        &cx,
                        &cy,
                        &mut regs,
                        s,
                        done,
                        end,
                        self.bailout2,
                        &mut out,
                        sh,
                    )
                } else if self.squarings {
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

/// [`orbit_loop_squarings`] on three threads: this one squares X, the helpers of
/// `shared` square Y and X+Y (the same operations on the same values, so the same orbit).
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn orbit_loop_squarings_shared<S: Storage>(
    x: &mut Raw<S>,
    y: &mut Raw<S>,
    cx: &Raw<S>,
    cy: &Raw<S>,
    regs: &mut StepRegs<S>,
    s: &mut [u64],
    start: usize,
    end: usize,
    bailout2: f64,
    out: &mut Outputs<'_>,
    shared: &Shared,
) -> (usize, bool) {
    let mut i = start;
    while i < end {
        let ok = parallel::step(shared, 0, i as u64 + 1, x, y, cx, cy, regs, s);
        debug_assert!(ok, "orbit helpers stopped");
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

/// The generic polynomial step `Z <- Z^p + C` (F2: any formula but Mandelbrot): `p - 1` complex
/// multiplications, four products each.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn orbit_loop_power<S: Storage>(
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
    power: u32,
) -> (usize, bool) {
    let [wx, wy, p1, p2, p3, p4] = t;
    let mut i = start;
    while i < end {
        raw::copy(wx, x.view());
        raw::copy(wy, y.view());
        for _ in 1..power {
            raw::mul(p1, wx.view(), x.view(), s);
            raw::mul(p2, wy.view(), y.view(), s);
            raw::mul(p3, wx.view(), y.view(), s);
            raw::mul(p4, wy.view(), x.view(), s);
            raw::add(wx, p1.view(), p2.view(), true, s);
            raw::add(wy, p3.view(), p4.view(), false, s);
        }
        raw::add(x, wx.view(), cx.view(), false, s);
        raw::add(y, wy.view(), cy.view(), false, s);
        i += 1;
        if emit_point(x, y, i, bailout2, out) {
            return (i, true);
        }
    }
    (i, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Orbit = (Vec<[f32; 2]>, Vec<FloatExpComplex>, OrbitResult);

    fn point(bits: usize) -> BigComplex {
        let mut c = BigComplex::parse(
            "-0.743643887037158704752191506114774123456789012345678901234567890123456789",
            "0.131825904205311970493132056385139987654321098765432109876543210987654321",
            bits,
        )
        .unwrap();
        c.re += BigFloat::from_f64(std::f64::consts::FRAC_1_PI, bits).times_power_of_two(-(bits as i64 - 3));
        c
    }

    fn same(a: &Orbit, b: &Orbit) {
        assert_eq!(a.0, b.0);
        assert_eq!(a.1, b.1);
        assert_eq!(a.2.count, b.2.count);
        assert_eq!(a.2.escaped, b.2.escaped);
        assert!(a.2.final_z.re == b.2.final_z.re && a.2.final_z.im == b.2.final_z.im);
    }

    /// Three threads give the serial orbit exactly, for fixed-size and heap registers,
    /// through an escape and across control intervals.
    #[test]
    fn three_threads_match_one() {
        for bits in [192usize, 1024, 1088, 2048] {
            let one = ReferenceOrbitComputer::new(&point(bits), bits)
                .with_variant(Variant::Squarings)
                .with_threads(Threads::Single);
            let three = one.clone().with_threads(Threads::Three);
            assert!(three.uses_helpers(10) && !one.uses_helpers(1 << 20));
            for iters in [1usize, 2, 5000] {
                same(&one.compute_vec(iters, 1e10), &three.compute_vec(iters, 1e10));
            }
            // A point outside the set escapes.
            let out = BigComplex::parse("0.2501", "0.0000001", bits).unwrap();
            let one = ReferenceOrbitComputer::new(&out, bits)
                .with_variant(Variant::Squarings)
                .with_threads(Threads::Single);
            let a = one.compute_vec(100_000, 2.0);
            let b = one.clone().with_threads(Threads::Three).compute_vec(100_000, 2.0);
            assert!(a.2.escaped);
            same(&a, &b);
        }
    }

    /// compute(k) then resume(n) equals compute(n), single- and multi-threaded, and a
    /// cancelled computation resumes where it stopped.
    #[test]
    fn resume_continues_exactly() {
        for (bits, threads) in [
            (128usize, Threads::Single),
            (1024, Threads::Single),
            (1536, Threads::Three),
        ] {
            let comp = ReferenceOrbitComputer::new(&point(bits), bits).with_threads(threads);
            let n = 3000;
            let whole = comp.compute_vec(n, 1e10);
            let mut pts = vec![[0f32; 2]; n + 1];
            let mut ext = vec![FloatExpComplex::ZERO; n + 1];
            let mut r = with_no_control(64, |ctl| comp.compute(700, 1e10, Some(&mut pts), Some(&mut ext), ctl));
            assert_eq!(r.iterations(), 700);
            let mut polls = 0;
            let mut cancel = || {
                polls += 1;
                polls == 3
            };
            let mut ignore = |_: usize| {};
            let mut ctl = Control {
                check_interval: 100,
                is_cancelled: &mut cancel,
                progress: &mut ignore,
            };
            r = comp.resume(&r, 2000, 1e10, Some(&mut pts), Some(&mut ext), &mut ctl);
            assert!(r.cancelled && r.iterations() == 1000);
            r = with_no_control(4096, |ctl| {
                comp.resume(&r, n, 1e10, Some(&mut pts), Some(&mut ext), ctl)
            });
            pts.truncate(r.count);
            ext.truncate(r.count);
            same(&whole, &(pts, ext, r));
        }
    }

    /// F2: the generic power loop at p = 2 is the Mandelbrot orbit (to the working precision:
    /// it rounds x^2 - y^2 where the specialised loops round (x+y)(x-y)), and p = 3 is z^3 + c.
    #[test]
    fn generic_power_two_is_mandelbrot() {
        for bits in [128usize, 1024, 1088] {
            let c = point(bits);
            let m = ReferenceOrbitComputer::new(&c, bits).compute_vec(2000, 1e10);
            let g = ReferenceOrbitComputer::new(&c, bits)
                .with_formula(Formula::Multibrot(2))
                .with_threads(Threads::Three);
            assert!(!g.uses_helpers(1 << 20));
            let g = g.compute_vec(2000, 1e10);
            assert_eq!(m.2.count, g.2.count);
            for (a, b) in m.0.iter().zip(&g.0) {
                assert!(
                    (a[0] - b[0]).abs() <= 1e-6 * (1.0 + a[0].abs())
                        && (a[1] - b[1]).abs() <= 1e-6 * (1.0 + a[1].abs())
                );
            }
            let d = &m.2.final_z - &g.2.final_z;
            let tol = -(bits as f64) + 40.0;
            assert!(
                d.re.log2_magnitude() < tol && d.im.log2_magnitude() < tol,
                "{bits} bits"
            );
        }
        let c = BigComplex::parse("-0.1", "0.8", 128).unwrap();
        let (pts, _, _) = ReferenceOrbitComputer::new(&c, 128)
            .with_formula(Formula::Multibrot(3))
            .compute_vec(20, 1e10);
        let mut z = (0.0f64, 0.0f64);
        for p in &pts[1..] {
            z = Formula::Multibrot(3).step(z, (-0.1, 0.8));
            assert!((p[0] as f64 - z.0).abs() < 1e-6 && (p[1] as f64 - z.1).abs() < 1e-6);
        }
    }
}
