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
No property tests or fuzzing (a project rule: CI stays fast).

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

# Citations

- [Apple: MetricKit MXHangDiagnostic and main-thread hangs](../sources/apple-metrickit-hang-diagnostics.md)
