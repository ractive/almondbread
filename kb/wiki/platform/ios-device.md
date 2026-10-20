---
title: "iOS and iPadOS on a device: first run, signing, memory, thermal, background"
type: platform
status: draft
sources: ["[[sources/apple-device-tooling]]", "[[sources/apple-simulator-tooling]]"]
tags: [ios, ipad, device, signing, memory, thermal, testing]
---

# iOS and iPadOS on a device

How the iPhone/iPad app gets onto a real device, and what the renderer does about the
limits a device has and a Mac does not: a per-app memory limit, passive cooling, no GPU work
in the background. Prepared 2026-10-18 without a device (Xcode 26.6, M1 Pro); everything
marked *device only* is still unmeasured. Background: [iOS and iPadOS port considerations](ios-ipad-considerations.md),
[iOS and iPadOS on the simulator: setup, checks, what broke](ios-simulator.md).

## First run on a fresh iPad (the steps)

What the person does is marked **you**; the rest is `make ios-device`.

1. **You (Mac, once):** Xcode > Settings (⌘,) > Accounts > + > Apple ID: sign in with the
   Apple ID of the paid team `9KNMHVRSTW`. Nothing else in Xcode needs setting; the iPad
   itself needs no Apple ID.
2. **You (iPad):** connect it by USB, unlock it, tap *Trust This Computer* and enter the
   passcode.
3. `make ios-device-doctor`: shows whether Xcode has an account, whether an Apple
   Development certificate exists, and the connected device with its Developer Mode state.
4. **You (iPad), if Developer Mode is off:** Settings > Privacy & Security > Developer Mode >
   On; the iPad restarts; after the restart unlock it and confirm *Turn On* with the passcode.
   The switch only appears once a Mac with Xcode has seen the device: if it is missing, open
   Xcode > Window > Devices and Simulators with the iPad connected (Xcode may also copy the
   device's debug symbols there for a few minutes; keep it connected and unlocked).
5. `make ios-device`: `xcodegen` + `make rust`, a signed **Release** build with automatic
   signing for the team (`-allowProvisioningUpdates` creates the Apple Development
   certificate, the App ID `ch.ractive.almondbread` and the profile;
   `-allowProvisioningDeviceRegistration` adds the iPad to the team's devices), then
   `devicectl device install app` and `devicectl device process launch`. The first time,
   macOS may ask whether `codesign` may use the new key: **you** enter the login password and
   choose *Always Allow*.
6. `make ios-device-selftest`: launches the self-test on the iPad (the app shows its first
   view, compares the shown frame with a fresh render, prints `renderer OK` and a memory line,
   quits), then copies `Documents/selftest.png` from the app container to
   `build/ios-device-selftest.png` (log: `build/ios-device-selftest.log`). Look at the PNG.
7. **You:** use the app; then the device-only checks below.

Options: `IOS_CONFIG=Debug`, `IOS_TEAM=…`, `IOS_DEVICE=<name or UDID>` (several devices).
`make ios-device-check` compiles the device build unsigned (no device, no account): it passed
2026-10-18. The simulator build (`make ios-sim`) stays unsigned; the signing settings are
command-line overrides of the device build only (`project.yml` unchanged), and the macOS
signing / release workflow is untouched.

### Troubleshooting

| Symptom | Cause, fix |
|---|---|
| `no iPhone/iPad connected` | cable, unlock, *Trust*; `xcrun devicectl list devices` must list it as connected |
| `not paired` | unlock, *Trust This Computer*, passcode |
| `Developer Mode is off` | step 4 |
| `Xcode is not signed in` / build log "No Accounts" | step 1 |
| "No signing certificate" | Xcode > Settings > Accounts > team > Manage Certificates > + Apple Development, or rerun (the flag makes one) |
| "Unable to find a destination" | Xcode still preparing the device (Devices and Simulators shows progress); keep it unlocked |
| install/launch "untrusted" | only for profiles the device does not trust: Settings > General > VPN & Device Management > the developer > Trust (not expected with a paid team) |
| launch "locked" | unlock the iPad |
| bundle ID taken | another team owns `ch.ractive.almondbread`: build with another `PRODUCT_BUNDLE_IDENTIFIER` |

The script (`scripts/ios-device.sh`) maps these messages to the hints; the full build log is
`build/ios-device-build.log`. The device JSON parser was tested against a hand-made
`devicectl` file (connected / not connected / watch), not a real device.

## Memory

The renderer's memory is almost all per-pixel GPU state; references and their tables are
small at the default limits. Measured with `fly --ios 1` (iOS scheduler defaults: 512 K live
slots, 2 M list entries), peak by kind from `Renderer::memory_report` (GPU = the Metal device's
`currentAllocatedSize`, without the CLI's 8 frame targets) (measured 2026-10-18, M1 Pro,
release, 18 s flights: jump to the place, then 5 s of Space zoom):

| Scenario | Size | Renderer peak | of it: reference + tables (CPU) |
|---|---|---:|---:|
| home zoom | 1024×768 | 158 MB | 1.7 MB |
| spiral-1e1000 | 1024×768 | 163 MB | 3.8 MB |
| home zoom | 2752×2064 (iPad Pro 13") | 470 MB | 2.0 MB |
| spiral-1e1000 | 2752×2064 | 454 MB | 3.8 MB |
| rock-bottom (1e-3007) | 2752×2064 | 449 MB | 1.4 MB |
| needle-minibrot-1e1000, star-1e2000, swirl-1e3000 (Mac defaults) | 2752×2064 | 579–581 MB | < 3 MB |
| spiral-1e1000, manual limit 1 M | 1024×768 | 160 MB | 10 MB |
| spiral-1e1000, manual limit 10 M | 1024×768 | 287 MB (process 671 MB) | 79 MB |
| the same under `--memory-limit 800` (limit capped to 786 k) | 1024×768 | 160 MB (process 251 MB) | 8.4 MB |

- Per output texel about **67 B** on the GPU heap (full level rg32f + rg16f + rg32f interior
  aux, the preview allocation with its rgba16f colour, two rgba16f history images, fog), plus
  three BGRA8 drawables of the layer (not in `currentAllocatedSize`): budgeted as 80 B.
  Fixed about **108 MB** with the iOS defaults (live-list state, unresolved lists, kernels,
  survey, overlays), ~200 MB with the Mac's.
- Deep places do not cost more than shallow ones: nucleus references are a period long
  (834 points at the 1e1000 needle) and the auto limit is capped at 200 k.
- A manual 10 M limit is the hazard: the whole process grew by 408 MB over a 1 M limit (CPU
  reference, its GPU copies, progressive prefixes and vector growth while it is computed):
  about **130 B per reference point**, and a reference is made for twice the limit.
- The self-test's memory line overstates normal use: by then it has made a fresh render of
  the view and an offscreen PNG, each with a renderer of its own. On the Mac the swift-run
  app showed footprint 1368 MB with the view's renderer at 534 MB (2560×1640, Mac defaults).
- The cap reaches every place a limit is set: automatic and manual limits, extension and
  "compute further" passes, longer references and journey prefetches (first try missed the
  further passes: a 76 MB reference under an 800 MB budget).
- The simulator says nothing here: its Metal reports 0 allocated bytes and its footprint
  (33–78 MB) leaves the GPU out; `os_proc_available_memory()` returns 0 there.

**Policy (Rust, `almondbread-render` `memory.rs`, `scheduler/device.rs`, `host.rs`):** Swift
forwards `os_proc_available_memory()` at launch, every 2 s while active and on every memory
warning (`Explorer.setMemoryAvailable(available, warning)`). Rust sets a ceiling = what it
holds + what is available (re-read only when it moves by more than ×1.25, a warning only ever
lowers it) and derives:

- **texel cap** 40 % of the ceiling at 80 B/texel (≥ 0.5 Mpx): a larger view renders at a
  lower scale and the layer stretches it (`Host::set_max_texels`, same aspect; checked on the
  simulator with `--memory-available-mb=250`: the iPad mini drew 928×1413 instead of
  1488×2266, filled the screen, shown frame = fresh render);
- **iteration cap** 25 % of the ceiling at 130 B per point, the reference twice the limit
  (≥ 200 k, rounded to 2^k × {1, 1.25, 1.5, 1.75} so it does not re-render on every reading);
  applies to auto and manual limits;
- on a **memory warning** first `release_caches` (spare reference, period caches, tables of
  older references, the preview allocation outside a gesture) and the thumbnails' bitmaps
  (`PlaceThumbnails.purge`).
- 0 from `os_proc_available_memory()` on a device means "at or over the limit"
  (src: [Apple device tooling and iOS process APIs (devicectl, xcodebuild provisioning, os/proc.h)](../sources/apple-device-tooling.md)): Swift passes 1 byte, so the budget drops to its
  floors.

What that means: a 3 GB iPad (~1.4 GB per app, inference from common reports, unverified)
keeps native resolution up to ~6.5 Mpx and limits of ~1 M; an 8 GB iPad caps nothing in
practice; 800 MB in all would render an iPad Pro 13" below native. No increased-memory-limit
entitlement: it is not needed at these numbers and every entitlement is one more thing the
first automatic provisioning has to get right.

## Thermal

`ProcessInfo.thermalStateDidChangeNotification` → `Explorer.setThermalState(raw)` → the
display link's frame-rate ceiling. Rust (`Thermal`):

| State | GPU per refresh | Per view | Frame rate |
|---|---|---|---|
| nominal, fair | unchanged | unchanged | display maximum |
| serious | chunk targets and gesture budget ×0.5 (coarser previews) | AA budget ×0.33, extension estimate ×0.33, no "compute further" | ≤ 60 Hz |
| critical | ×0.35 | refinement stops once the view's first full-resolution frame is shown (no AA, no further passes) | ≤ 30 Hz |

A cooler state restores the saved settings and asks for frames, so a paused view resumes.
Tested in Rust (`scheduler::device::tests`); the real curves are *device only* (Xcode >
Devices and Simulators > Device Conditions > Thermal State can force a state while the app
runs).

## Background

`UIScene.didEnterBackgroundNotification` of the view's scene → `Explorer.setActive(false)`:
Rust waits for every command buffer in flight (chunks, frames, the survey), submits nothing
more (`frame` returns 0 at once) and the drawable thread parks (its drawable goes back to the
pool); the display link pauses. `willEnterForeground` resumes; job and history are kept, so
the view continues where it was. Simulator: three background / foreground round trips
(launching Settings over the app), same process, the view intact afterwards; Rust test: a
suspended renderer commits nothing and settles after resume.

## Frame rate, watchdog, licence

- **ProMotion:** `CADisableMinimumFrameDurationOnPhone` is in the iOS Info.plist (iPhones need
  it for > 60 Hz; iPads do not); the `CADisplayLink` asks for the screen's
  `maximumFramesPerSecond` (120 on ProMotion, 60 elsewhere) under the thermal ceiling.
  *Device only:* whether frames keep up at 120 Hz.
- **GPU watchdog:** no change. Every command buffer is a chunk sized by the measured rate:
  16 ms idle, 8 ms in gestures (×0.5 / ×0.35 when hot), a failed chunk divides the rate by 8
  ([GPU watchdog and chunked, resumable kernels](gpu-watchdog-and-chunking.md)); the first chunk on a slower A-series GPU uses the
  Mac's starting rate and may run a few times longer than 16 ms, still far from any watchdog
  reported (inference). iOS's limit is unknown ([What is the exact Metal GPU watchdog limit on macOS and iOS?](../questions/metal-watchdog-limit.md)).
- **Licences in the iOS binary:** no LGPL. BigFloat is our own code (`almondbread-math` has no
  dependencies); there is no GMP/MPFR. `cargo tree --target aarch64-apple-ios` of the FFI crate:
  MIT / Apache-2.0 / Zlib crates and UniFFI (MPL-2.0, file-level copyleft, unmodified: its
  source is public, nothing to relink); `cargo deny check licenses` passes. The Swift package
  has no dependencies.

## Device only (to do with the iPad)

- `make ios-device-selftest`: the memory line gives the real `phys_footprint`, available
  memory and budget; record them here.
- Footprint after a few minutes of autopilot and a journey; a manual 10 M limit at
  spiral-1e1000 (the iteration cap should hold it).
- Thermal: a 10-minute journey on battery, the state reached, the frame rate; force serious /
  critical in Xcode's Device Conditions.
- Background mid-render (home gesture), back after a minute: no crash, the view resumes.
- Speed: gesture frame rate and preview resolution at home, 1e-300 and 1e-1000.
