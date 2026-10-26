---
title: Hang prevention (loop guards, extreme-value tests, main-thread watchdog)
type: technique
status: draft
sources: ["sources/apple-metrickit-hang-diagnostics"]
tags: [robustness, hangs, numerics, watchdog, engineering]
---

# Hang prevention

Two beach balls in one week (2026-10): the mini-map's level loop never ended past zoom 1e308
(the depth overflowed f64 to +inf), and a SwiftUI layout feedback loop while a popup menu was
open. This page collects the patterns that hang a deep-zoom app, the guards in the code, the
small tests that keep them, and the in-app watchdog that reports the ones we missed. Code
cites it as `kb: techniques/hang-prevention`.

## The patterns

At zoom 1e-3010 almost every derived number leaves f64's comfortable range somewhere: depth
in decades is ~3000, a scale ratio is 10^3000 (inf), `10^-d` underflows, a precision in bits
is 10 000. Loops whose end depends on such a value hang when it turns inf, NaN or merely
large (our audit, 2026-10-16):

- **Unbounded iterators with a computed stop**: `(1..).map(f).take_while(|x| x <= limit)`
  never ends when `f` stops growing (limit inf, or `f` constant: `eps = 0`).
- **f64 step loops**: `while k <= hi { ...; k += 1.0 }` stops changing `k` past 2^53 (and at
  inf): the cosmic ruler's decade ticks at a depth of 1e18 decades.
- **float → int casts as loop bounds**: `x as i64` saturates (inf → i64::MAX, NaN → 0), so
  `for d in (lo..=hi)` with `lo = xb.ceil() as i64` runs 2^63 times at `xb = -inf`; the
  autopilot's `(dt / 0.002).ceil() as usize` substeps at `dt = inf`.
- **Absurd sizes as allocations**: bits for a scale of 2^-1e9, `10^999999999` computed exactly
  when such text is pasted (minutes in `bignat::pow10`), a drawable of usize::MAX pixels.
- **Recursion on a degenerate input**: `frexp(0.0)` that renormalises subnormals by
  `x * 2^64` recurses forever on zero (a stack overflow, not a hang, but the same family).
- **Integer loops with a degenerate parameter**: Halton base 1 (`i /= 1`).
- **Swift**: `Int(x)` traps on NaN or inf; a layout that changes its own size proposal loops
  (the menu hang).

## The guards (as built, 2026-10-16)

Rule: a loop whose end depends on computed values gets a hard cap with a sane fallback, an
`is_finite` check before it, or an integer counter instead of an f64 one. Obviously bounded
loops (fixed ranges, slices, Knuth division's two corrections, Newton with a step cap) stay as
they are.

| Where | Risk | Guard |
| --- | --- | --- |
| `overlay.rs` `map_decades`, `update_maps` | depth inf/NaN (the original hang), `want` huge | `is_finite`, at most 15 levels; panel ≤ 4096 px |
| `render/cosmic.rs` decade ticks | `k += 1.0` past 2^53 or at inf | integer counter, at most 81 ticks; `update` skips non-finite depth |
| `core/cosmic.rs` `rungs` | `k0 + 8` overflow at inf | `saturating_add` (≤ 9 ladders) |
| `pi_meter.rs` ruler and plot ticks | f64 `while` over decades | integer range, at most 65 |
| `pi_meter.rs` `set_layout` | `px = inf` → usize::MAX trail picture | positive finite only |
| `viewport.rs` centre bits | 2^-1e9 scale before the caller clamps it | capped at `-MIN_SCALE_LOG2 + 1024 + guard` |
| `decimal.rs` `BigFloat::parse` | exact 10^1e9 | decimal exponent ≤ 100 000, else `None` |
| `viewport.rs` `Center::parse` | a pasted million digits: 1.6 s quadratic parse, a 52 000-limb centre for the session | ≤ 20 000 digits (`MAX_COORDINATE_DIGITS`), else `None`; precision ≤ `MAX_CENTER_BITS` (2026-10-20, review M3) |
| `viewport.rs` `Scale::big_times` | a pan by f64::MAX points: `x * mantissa` = inf, `BigFloat::from_f64` asserts | product saturates at ±f64::MAX, NaN is 0 (review M4) |
| `floatexp.rs` `DoubleExp` `*`, `/`, `times_power_of_two` | Newton's derivative of a far-escaped orbit (bailout f64::MAX squares the exponent each step) wrapped i64 (debug panic; garbage steps in release) | exponent in i128, saturates to ±inf or zero; the finite check then stops Newton (review E1) |
| `orbit.rs` `recommended_precision_bits_with` | inf → i64::MAX bits | saturating, ≤ 2^20 bits |
| `input.rs` `Navigator::handle` / `advance` | NaN/inf events reach BigFloat (panic) or sizes; NaN dt | non-finite events ignored, negative sizes 0, NaN dt = 0 |
| `autopilot.rs` substeps | `dt = inf` → usize::MAX substeps | 1 ... 1000 |
| `host.rs` `resize` | inf size | ≤ 16 384 px a side |
| `floatfloat.rs` iterate loop | GPU never reports done | at most `max_iter / 64 + 1024` dispatches |
| `scheduler.rs` `halton` | base 0 / 1 | 0.5 |
| `frexp` / `exponent_of` / `exponent_significand` | recursion on 0 | zero returns first |
| CLI `pi_counts`, `nucleus` refine | `(1..)` with eps 0; bits loop | `1..=64`; 8 rounds, bits ≤ 2^17 |
| Swift `ContentView` flight percent | `Int(NaN)` traps | `String(format:)` |
| `floatexp.rs` `DoubleExp::from_f64` | a point 10^336 view widths away is inf in f64 view units; the orbit probe's conversion asserted, a panic on every refresh (2026-10-26) | non-finite carried as is (mantissa ±inf / NaN, exponent 0), like an overflow in `*`; no assert, not even a debug one (see [Panic log](#panic-log-and-the-offset-overflow-class-as-built-2026-10-26)) |
| `viewport.rs` `Center::new`, `Center::offset` | non-finite f64 into `BigFloat::from_f64` (asserts) | saturate like `big_times`: NaN is 0, ±inf is ±f64::MAX |
| `scheduler.rs` `reproject` | a jump between two deep views of similar scale: history offset inf (NaN when turned) went to the GPU as f32 | offsets past 1e30 history pixels: no history |

Lints (workspace `[lints.clippy]` in `Cargo.toml`, enforced by `make rust-check`'s
`-D warnings`): `infinite_iter` (deny), `maybe_infinite_iter`, `while_float`, `infinite_loop`
(warn). `while_float` flags plain `while x < y` on floats; compound conditions (`while k <= hi
&& ...`) slip through, so the audit still matters.

## Extreme-value tests

`extreme_tests.rs` in core, math and render: 13 plain unit tests that call the at-risk
functions with zoom 1e-3010 and 2^±(2^62), ±inf, NaN, size 0 and negative spans (map levels,
cosmic scale and panel, compass maths and layout, π meter layout and flight views, navigator,
viewport clamps, pasted view text, decimal parse, precision bits, tile binning, Halton) and
assert a quick, sane answer. `cargo test extreme_` runs them in a few milliseconds per crate.
No property tests or fuzzing (a project rule: CI stays fast). Since 2026-10-26 also
`extreme_double_exp_from_non_finite_f64` (math) and `extreme_offsets_far_past_f64_view_units`
(core: a 1e-337 view with a far point and a shallow view with a point 1e-337 off its centre,
straight and turned, through `offset_of`, `distance_in`, `delta_double_exp`, `FlyPath` both
ways and deep to deep, the autopilot's `snapped`, and non-finite centres).

## Panic log and the offset overflow class (as built, 2026-10-26)

**The incident.** A tour step's probe point 10^336 view widths away (the "What's the fog?"
tour started at 1e334) went through f64 view units to `DoubleExp::from_f64(inf)`, which
asserted. `Explorer::frame` catches panics (`catch_unwind`, then `recover_after_panic`), so the
app lived, but the panic fired on every display refresh, the view froze, and the panic hook
appended a full entry with a backtrace each time: 143 000 entries, 1.2 GB of
`~/Library/Logs/Almondbread/panic.log` in about an hour. The probe itself was fixed first
(overlay `update_probe` takes `Center::delta_double_exp`, exact BigFloat to DoubleExp).

**The log** (`crates/almondbread-ffi/src/panic_log.rs`, hook installed by `Explorer::new`):

- Two panics are the same when message and location are. The first of a kind gets the full
  entry (version, time, pid, thread, message, backtrace) and the previous hook (stderr).
- A repeat is only counted: no backtrace capture, nothing on stderr, no write. A line
  `--- repeated N more times (T in all): <message at location>` is written at most once a
  minute while the loop lasts, when a different panic comes, when a frame completes without a
  panic (`... (then a frame completed)`), and when the `Explorer` is dropped. The dedupe
  survives good frames: a panic that alternates with them (or fires in `input` between frames)
  stays one entry and counts. An app killed in a loop loses at most a minute of count.
- Before each write the file rotates at 5 MB to `panic.log.1` (one old file). A file over twice
  the cap (an old runaway log) is cut to its first 5 MB before the rename: the head holds the
  first entries, the rest of such a log is the same entry again.
- Per frame without a panic: one relaxed atomic swap.
- Tests: `panic_log::tests` (dedupe counts and the minute line, loop end, rotation in a temp
  dir, a runaway file cut to its head).

**What the user sees** of a panic loop: the view stops changing, and the HUD's developer
details show "failed frames N" in red (`ExplorerStats.failed_frames`, counted by
`recover_after_panic`). Nothing in the normal HUD. Proposed, not built: when frames panic
several refreshes in a row, the Rust progress line says "Rendering failed (internal error);
details in ~/Library/Logs/Almondbread/panic.log" (unverified as to wording; it touches the
progress line, which other work was changing at the time).

**The audit** of BigFloat/Center offsets turned into f64 view units (`delta_in`, `distance_in`,
`offset_of`, `delta_from`) and then into DoubleExp or float maths. `delta_in` returns ±inf past
f64's range, `offset_of` ±inf or NaN (inf·cos − inf·sin in a turned view). The panic sinks are
`DoubleExp::from_f64` and `BigFloat::from_f64` (both asserted finite); a cast index of an
infinite offset would be another, none was found on these paths.

| Site | Far offset reachable? | Outcome |
| --- | --- | --- |
| Orbit overlay probe (`overlay.rs` `update_probe`) | yes (tour probe far from a deep view) | fixed before: `delta_double_exp`; now also no panic in `from_f64` |
| Tour marker (`render/tour.rs`), autopilot marker (`marker.rs`) | yes | already checked `is_finite`, not drawn |
| Mini-map levels and marker (`overlay.rs`) | no: a level is kept only while the view is within 0.3 panel of its centre | none needed |
| Mandelbrot panel marker (`mandelbrot_panel::marker_at`) | no: the panel is zoomed out to contain the way-back view, which contains k | none needed |
| Julia panel k | no offset (k is the pointer's point) | none needed |
| Cosmic scale, compass | scales and angles only | existing extreme tests |
| π meter `base_offset` | yes (a probe's pinch point, a deep turned view elsewhere: NaN) | fixed: the "pinch point left the view" test is a negated `<=`, so NaN drops the probe; no own test (unverified beyond reading) |
| `FlyPath::new` (fly_to, tours, places) | yes | grows its unit until the offset is finite; tested both ways |
| Journey, autopilot lock, seek | distances in comparisons (inf compares false/true correctly), `atan2` of inf is finite | none needed; `snapped` tested |
| Scheduler reference offsets (`delta_in(&reference.point, ...)`) | no: a reference whose point is not near the view is not usable (`reference.rs` distance check) | none needed |
| Scheduler history reprojection (`reproject`) | yes: a jump between two deep views of similar scale | fixed: no history past 1e30 px; `extreme_history_reprojection_far_past_f64` (render, Metal) |
| Scheduler history tile footprint (`to_history`) | yes | inf/NaN compare false: tiles count as outside, correct |
| `Center::new`, `Center::offset` with non-finite f64 | via computed tour paths and callers | fixed: saturate |

**Decision on `DoubleExp::from_f64`.** It no longer asserts, in any build. Its type already
carries ±inf from overflow in `*`, `/` and `times_power_of_two`, and every caller that cares
checks finiteness; in the app a panic there could only drop a frame and fire again on the next.
A debug-only assert was rejected: debug and release would then behave differently on exactly
the inputs the extreme tests feed. `BigFloat::from_f64` keeps its assert (BigFloat has no
infinity); its app-side entry points (`Center::new`, `Center::offset`, `Scale::big_times`)
saturate instead.

## The main-thread watchdog

`Sources/AlmondbreadKit/HangWatchdog.swift`, started by `AlmondbreadScene` on macOS and iOS:

- A background thread posts a block to the main queue every 0.5 s. A heartbeat unanswered for
  more than 2 s is a hang (macOS shows the beach ball at about that point; Apple calls 250 ms a
  hang, src: [Apple: MetricKit MXHangDiagnostic and main-thread hangs](../sources/apple-metrickit-hang-diagnostics.md)).
- Once per hang it writes `~/Library/Logs/Almondbread/hang-<yyyyMMdd-HHmmss>.txt` (iOS: the
  container's Library/Logs; `--hang-dir=path` overrides): duration, whether the main thread is
  inside `Explorer.frame`, the last 32 UI events and settings changes (a ring buffer fed by
  `FractalHostBase.send` and `ExplorerModel.push*`), and the main thread's stack. When the
  main thread answers, a `recovered: ... after N s` line is appended.
- The stack is sampled from the watchdog thread with public Mach calls: `thread_suspend`,
  `thread_get_state(ARM_THREAD_STATE64)`, a frame-pointer walk with `vm_read_overwrite` (arm64
  keeps frame pointers), `thread_resume`. Nothing allocates or locks while the main thread is
  stopped (it may hold malloc's lock); symbols come from `dladdr` afterwards, with image base
  and offset for `atos`. Crash reporters use the same calls; no private API (unverified for App
  Review, but these are documented `mach/` calls).
- Cost: one tiny main-queue block per 0.5 s and a lock per recorded event. It never blocks the
  main thread and ignores its own failures.
- Off during window tests (`--selftest`, `--interaction-test`, ...) unless `--hang-test`.
  (The env switch `ALMONDBREAD_HANG_WATCHDOG` was removed 2026-10-17; the watchdog is always on in the app.)
- `--hang-test`: a background window blocks its main thread for 3 s, then prints the report and
  quits (exit 0 when the report and the recovery line were written).
- MetricKit: `MXMetricManager` hang diagnostics (`MXHangDiagnostic`, iOS 14+, macOS 12+) are
  saved next to the reports as `hang-metrickit-<time>.json` when the system delivers them (App
  Store builds; daily or next launch; src: [Apple: MetricKit MXHangDiagnostic and main-thread hangs](../sources/apple-metrickit-hang-diagnostics.md)).

## GPU queue (as built, 2026-10-19)

A hang the watchdog found in the iPad simulator during the minibrot-periods tour's "Period 2688"
step (300 000 iterations, 6.3e-16 span): the main thread stood in `-[MTLCommandQueue
commandBuffer]` (`MetalBackend::begin` from `encode_compose`, inside `Explorer.frame`) for up to
24.8 s, eight reports in two minutes. Metal hands out a bounded number of command buffers per
queue (64 by default) and `commandBuffer()` blocks until one completes. The simulator's GPU took
seconds per iterate chunk; the scheduler's own frame gate counts only two frames in flight but
drops its count after 0.25 s without a completion (so a lost drawable cannot stall it), after
which every refresh committed another compose buffer behind the long chunks until the queue was
exhausted.

Guard: `GpuBackend::submissions_in_flight` (Metal: buffers committed through `begin` whose
status is not completed or error; shared by the clones on one queue) and `Host::frame` skips the
whole refresh while 24 or more are queued (`GPU_QUEUE_LIMIT`; normal frames keep under ten), no
navigator step, no overlay, no submission; `Host::gpu_queue_skips` counts them. Unit test
`counts_submissions_in_flight`.

Evidence (UI test `TourPerformanceUITests.testPeriodStepStaysResponsive`, AB iPad mini simulator,
iOS 26.5, M1 Pro host): longest main-thread stall during the step 24.76 s before, 0.15 s after;
the slowest accessibility query 25.3 s before, 0.05 s after. The watchdog's longest heartbeat
delay is in the iOS UI-test value as `mainlag:` (`HangWatchdog.longestDelay`).

What stays: a GPU saturated by long compute buffers also starves the simulator's compositor, so
XCTest's screenshot request times out and Core Animation's commit can wait on the render server
(one 2.6 s report, outside `Explorer.frame`, in the run after the fix). That is the simulator's
GPU sharing; whether a real device's compositor is ever starved by the iterate chunks is not
measured (unverified).

## Long references (as built, 2026-10-20)

An external code review (2026-10, findings C1-C3) found work proportional to an iteration
limit on the frame thread. The limit is user-set: a manual limit is accepted up to
`MANUAL_ITERATIONS_MAX`, and the automatic cap can be raised to 10 M.

- **f64 orbit (C1).** Shallow views (scale per point >= 1e-12) computed the f64 reference
  synchronously in `prepare_reference`, as long as the extension passes may iterate, into a
  vector that reserved the whole limit: a manual 1e9 at a centre inside the set meant 8 GB and
  seconds per refresh, and a failed GPU upload left the old reference current, so the next
  refresh did it all again. Now up to `F64_SYNC_POINTS` (2^18 points, 0.94 ms at a centre that
  never escapes, measured 2026-10-20, M1 Pro, release) it stays on the frame thread; that covers
  every reach of the default cap of 200 000, so the default path is unchanged. Longer orbits are
  an `F64` request on the `ReferenceWorker` (cancellable every 65 536 iterations): the view
  waits, frames show the history, and the orbit is taken when it arrives; a good reference that
  is only shorter than the extension's reach serves meanwhile (the passes stop at its end).
  `set_reference` returns false and clears the reference when an upload fails, so the view
  waits instead of rendering on a reference that does not fit it. Offscreen renders still
  compute it in line (they wait anyway).
- **Allocations (C1).** Orbits grow as they are computed instead of reserving the request: the
  f64 orbit past 2^18 points, a big centre orbit past 2^20 points in stages that double its
  storage (resuming is exact, so the points are unchanged). Twice a 1e9 limit had reserved
  48 GB before the first iteration.
- **Copies on the frame thread (C2).** `Reference::points` and `extended` are shared
  (`Arc`), so table builds and copies no longer clone the orbit. The reference worker runs a
  hook on its own thread for every reference it hands out (`ReferencePrepare`): the GPU upload,
  the padded floatexp form and the BLA reach, through `GpuBackend::uploader` (Metal: the device
  is thread-safe). Taking a 4 M-point f64 reference from the worker costs the frame thread
  0.001 ms; uploading it there 11.8 ms (measured 2026-10-20, M1 Pro, release,
  `worker_reference_is_uploaded_off_the_frame_thread`). Compute further's longer orbit checks its
  prefix and uploads on its own thread too.
- **Detached builds (C2).** A BLA/LA table build is a `TableBuild`: dropping it (a new
  reference replaced the old one) sets its cancel flag, and the builds poll it before each
  level or stage. Before, a superseded build ran to the end and held its intermediates.
- **Cancellation inside selection (C3).** The worker's cancel flag reaches the minibrot size
  estimate (`minibrot_size_cancellable`, `period` full-precision squarings) and the exact
  Newton phase (polled every 4096 iterations inside a step); the selection checks it between
  Newton and the size estimate. A dropped `ReferenceWorker` cancels what it runs.

Tests: `long_reference_tests.rs` in render (a manual 1e9 limit: longest refresh 0.08 ms while
the orbit computes; a 2e7 limit: 0.20 ms until the orbit arrives; the upload; a replaced
reference cancels its build), `reference_gpu::tests`, `reference::tests` (growth, staged
growth exact, the f64 orbit on the worker), `selection::cancel_tests`.

## Example report

From `--hang-test` (measured 2026-10-16, M1 Pro, `swift build -c release`; abridged): the
report came 2.2 s into the 3 s block, the recovery line when the main thread woke.

```text
Almondbread hang report
time: 2026-10-16T21:10:17Z
process: AlmondbreadMac pid 84495, version dev (-)
main thread: no answer to a heartbeat for 2.22 s (threshold 2.0 s)
renderer: not in a frame call (the last one ended 3.25 s ago)

recent events (seconds before the report, oldest first):
  -  3.817 s  hang test: blocking the main thread for 3 s
  -  3.667 s  settings: pushOverlay from attach(_:host:)
  -  3.571 s  input: insets(top: 52.0, left: 0.0, bottom: 0.0, right: 0.0)

main thread stack (sampled from the watchdog thread; ...):
  0  libsystem_kernel.dylib     0x000000018c38d308  base+0x4308  __semwait_signal + 8
  1  libsystem_c.dylib          0x000000018c269cc0  base+0xdcc0  nanosleep + 220
  2  Foundation                 0x000000018dd4ab34  base+0xd0b34  +[NSThread sleepForTimeInterval:] + 152
  3  AlmondbreadMac             0x0000000102a70060  base+0xb8060  $s14AlmondbreadKit8HangTestO5start... + 244
  ...
 12  CoreFoundation             ...  __CFRUNLOOP_IS_SERVICING_THE_MAIN_DISPATCH_QUEUE__ + 16
 22  AppKit                     ...  -[NSApplication run] + 368

recovered: the main thread answered the heartbeat 2.98 s after it was sent
```

## Not covered

- A hang inside Rust on the main thread shows Rust frames only by image offset when the
  static library's symbols are not exported (`atos` with the binary resolves them).
- The SwiftUI layout loop of the menu hang shows as deep SwiftUI/AppKit layout frames; the
  ring buffer then says which setting or event preceded it.
- Hangs on other threads (the reference worker, the orbit threads) are not watched; the
  scheduler's offscreen loop waits on GPU submissions and has no cap of its own.
- Compute further's f64 orbit thread (at most 2^24 points) is not cancelled when the view
  changes; it ends on its own within ~60 ms.

# Citations

- [Apple: MetricKit MXHangDiagnostic and main-thread hangs](../sources/apple-metrickit-hang-diagnostics.md)
