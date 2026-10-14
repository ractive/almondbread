//! The reference orbit on three cores (R6; kb: techniques/parallel-reference-orbit).
//!
//! One iteration of the squaring form needs X^2, Y^2 and (X+Y)^2: three independent
//! products. Three threads (the caller and two scoped helpers) each hold the whole state
//! Z = X + iY, square one of the three, publish it, read the other two and do the
//! additions themselves, so all three step to the same next Z. That is one all-to-all
//! exchange per iteration (a cache-line transfer, ~0.1 us on M1 Pro) instead of a
//! publish-then-collect round trip. Products go through atomics, double-buffered by
//! iteration parity: a thread can be at most one iteration ahead of the slowest, since
//! it needs every product of the current iteration before it computes the next.
//!
//! The operations and their inputs are exactly those of the serial loop, so the orbit is
//! bit-identical. It pays from about [`PARALLEL_MIN_LIMBS`] limbs (measured). Waits spin
//! briefly, then yield, so a descheduled thread on a loaded machine costs time slices,
//! not a stall.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::raw::{self, Raw, Storage, View};

/// Smallest limb count at which the orbit uses helper threads by default (measured on
/// M1 Pro, kb: synthesis/benchmarks R6).
pub const PARALLEL_MIN_LIMBS: usize = 24;

/// Fewest iterations for which starting helpers pays (thread start ~20 us each).
pub const PARALLEL_MIN_ITERATIONS: usize = 256;

/// Limbs per 128-byte cache line (Apple silicon).
const LINE_LIMBS: usize = 16;

/// A register shared between threads, on cache lines of its own (header in the first
/// line). Relaxed element accesses are ordered by the release/acquire on the counters.
pub(crate) struct Slot {
    /// [neg, exp, zero, pad..., limbs...] padded to whole lines.
    words: Box<[AtomicU64]>,
}

impl Slot {
    fn new(n: usize) -> Self {
        let len = (LINE_LIMBS + n).div_ceil(LINE_LIMBS) * LINE_LIMBS + LINE_LIMBS;
        Slot {
            words: (0..len).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    #[inline(always)]
    fn store(&self, v: View<'_>) {
        let w = &self.words;
        w[0].store(v.neg as u64 | (v.zero as u64) << 1, Ordering::Relaxed);
        w[1].store(v.exp as u64, Ordering::Relaxed);
        for (d, s) in w[LINE_LIMBS..].iter().zip(v.m) {
            d.store(*s, Ordering::Relaxed);
        }
    }

    #[inline(always)]
    fn load<S: AsMut<[u64]>>(&self, r: &mut Raw<S>) {
        let w = &self.words;
        let flags = w[0].load(Ordering::Relaxed);
        r.neg = flags & 1 != 0;
        r.zero = flags & 2 != 0;
        r.exp = w[1].load(Ordering::Relaxed) as i64;
        for (d, s) in r.m.as_mut().iter_mut().zip(&w[LINE_LIMBS..]) {
            *d = s.load(Ordering::Relaxed);
        }
    }
}

/// A counter on its own cache line, so spinning on one does not steal the line another
/// thread writes.
#[repr(align(128))]
struct Counter(AtomicU64);

/// State shared by the three threads of one orbit computation.
pub(crate) struct Shared {
    /// products[parity][role]: X^2, Y^2, (X+Y)^2 of the iteration with that parity.
    products: [[Slot; 3]; 2],
    /// Iterations whose product each role has published (iteration i publishes i + 1).
    ready: [Counter; 3],
    stop: Counter,
}

impl Shared {
    pub(crate) fn new(n: usize) -> Self {
        Shared {
            products: core::array::from_fn(|_| core::array::from_fn(|_| Slot::new(n))),
            ready: core::array::from_fn(|_| Counter(AtomicU64::new(0))),
            stop: Counter(AtomicU64::new(0)),
        }
    }

    fn stopped(&self) -> bool {
        self.stop.0.load(Ordering::Relaxed) != 0
    }
}

/// Tells the helpers to exit when dropped (also on a panic in the caller, so the scope
/// that joins them cannot hang).
pub(crate) struct StopGuard<'a>(pub(crate) &'a Shared);

impl Drop for StopGuard<'_> {
    fn drop(&mut self) {
        self.0.stop.0.store(1, Ordering::Release);
    }
}

/// Spins until `ready()`, then yields between polls. False if `shared` stopped first.
#[inline(always)]
fn wait_until(shared: &Shared, mut ready: impl FnMut() -> bool) -> bool {
    let mut spins = 0u32;
    loop {
        if ready() {
            return true;
        }
        if shared.stopped() {
            return false;
        }
        if spins < 4096 {
            spins += 1;
            core::hint::spin_loop();
        } else {
            std::thread::yield_now();
        }
    }
}

/// One thread's registers for the squaring step.
pub(crate) struct StepRegs<S> {
    pub(crate) x2: Raw<S>,
    pub(crate) y2: Raw<S>,
    pub(crate) s2: Raw<S>,
    pub(crate) sum: Raw<S>,
    pub(crate) nrm: Raw<S>,
    pub(crate) d: Raw<S>,
}

impl<S: Storage> StepRegs<S> {
    pub(crate) fn new(n: usize) -> Self {
        let reg = || Raw::<S>::new_zero(n);
        StepRegs {
            x2: reg(),
            y2: reg(),
            s2: reg(),
            sum: reg(),
            nrm: reg(),
            d: reg(),
        }
    }
}

/// One iteration as thread `role` (0: X^2, 1: Y^2, 2: (X+Y)^2): its own product, the
/// exchange, then the serial loop's additions. `seq` = iteration + 1. False if stopped.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub(crate) fn step<S: Storage>(
    shared: &Shared,
    role: usize,
    seq: u64,
    x: &mut Raw<S>,
    y: &mut Raw<S>,
    cx: &Raw<S>,
    cy: &Raw<S>,
    r: &mut StepRegs<S>,
    s: &mut [u64],
) -> bool {
    let slots = &shared.products[(seq & 1) as usize];
    match role {
        0 => {
            raw::sqr(&mut r.x2, x.view(), s);
            slots[0].store(r.x2.view());
        }
        1 => {
            raw::sqr(&mut r.y2, y.view(), s);
            slots[1].store(r.y2.view());
        }
        _ => {
            raw::add(&mut r.sum, x.view(), y.view(), false, s);
            raw::sqr(&mut r.s2, r.sum.view(), s);
            slots[2].store(r.s2.view());
        }
    }
    shared.ready[role].0.store(seq, Ordering::Release);
    for k in 0..3 {
        if k == role {
            continue;
        }
        if !wait_until(shared, || shared.ready[k].0.load(Ordering::Acquire) >= seq) {
            return false;
        }
        match k {
            0 => slots[0].load(&mut r.x2),
            1 => slots[1].load(&mut r.y2),
            _ => slots[2].load(&mut r.s2),
        }
    }
    // Exactly the additions of the serial squaring loop.
    raw::add(&mut r.nrm, r.x2.view(), r.y2.view(), false, s);
    raw::add(&mut r.d, r.x2.view(), r.y2.view(), true, s);
    raw::add(x, r.d.view(), cx.view(), false, s);
    raw::add(&mut r.d, r.s2.view(), r.nrm.view(), true, s);
    raw::add(y, r.d.view(), cy.view(), false, s);
    true
}

/// Start of a computation as the helpers see it: C, Z at iteration `first`.
pub(crate) struct HelperStart<'a> {
    pub(crate) c: [&'a Raw<Vec<u64>>; 2],
    pub(crate) z: [&'a Raw<Vec<u64>>; 2],
    pub(crate) first: usize,
}

/// A helper thread: iterates as `role` from `start` until the caller stops.
pub(crate) fn helper<S: Storage>(shared: &Shared, role: usize, n: usize, start: &HelperStart<'_>) {
    let reg = || Raw::<S>::new_zero(n);
    let (mut x, mut y, mut cx, mut cy) = (reg(), reg(), reg(), reg());
    raw::copy(&mut cx, start.c[0].view());
    raw::copy(&mut cy, start.c[1].view());
    raw::copy(&mut x, start.z[0].view());
    raw::copy(&mut y, start.z[1].view());
    let mut regs = StepRegs::<S>::new(n);
    let mut scratch = vec![0u64; raw::raw_scratch_limbs(n)];
    let mut seq = start.first as u64;
    loop {
        seq += 1;
        if !step(shared, role, seq, &mut x, &mut y, &cx, &cy, &mut regs, &mut scratch) {
            return;
        }
    }
}
