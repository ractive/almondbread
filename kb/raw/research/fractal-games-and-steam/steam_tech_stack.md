# Steam edition tech stack for almondbread (state: October 2026)

Scope: how to ship almondbread on Steam (Windows, Linux/Steam Deck, macOS) while reusing the
Rust workspace, and how to reorganise the code. Internal context was read from
`kb/wiki/decisions/rust-core.md`, `kb/wiki/platform/render-architecture.md`,
`kb/wiki/platform/rust-swift-ffi.md`, `kb/wiki/decisions/landscape-mode.md`,
`kb/wiki/decisions/gamification.md`, `Cargo.toml`, `crates/almondbread-render/src/backend.rs`
and `crates/almondbread-render/src/shaders/mandelbrot.metal` (535 lines).

Internal facts that drive the answers (from the repo, not the web):

- The `GpuBackend` trait is a compute-only subset: buffers (private/shared), 2D textures
  (rg32f, rg16f, rgba16f, bgra8), pipelines specialised by **bool function constants**
  (`kDE`, `kInterior`), one command buffer per chunk with one compute pass, `set_bytes`
  (small inline constants), `dispatch_threads`, `dispatch_indirect`, `present(drawable)`,
  and a polled `Submission` with `status()` / `gpu_time()` / `wait()`. No completion
  callbacks. (`crates/almondbread-render/src/backend.rs`, `kb/wiki/platform/render-architecture.md`)
- The `iterate` kernel uses **SIMD-group (subgroup) collectives** — `simd_prefix_exclusive_sum`,
  `simd_sum` — plus `threadgroup` arrays and `atomic_uint` counters for order-preserving live-list
  compaction; per-SIMD-group atomics were measured as 44% more GPU time, so the subgroup path is
  load-bearing for performance. (`mandelbrot.metal` lines 162–330)
- Kernels compile at runtime from source with MSL 3.0 and fast math on; iteration fields are
  bit-identical to the old Swift renderer. (`render-architecture.md`)
- `math` and `core` are `forbid(unsafe_code)`, platform-free, and already `cargo check` for
  Linux gnu/musl and Windows msvc; render/cli build there with "no GPU backend".
  (`rust-core.md`, `rust-swift-ffi.md`)
- Planned later: a 2.5D height-field ray marcher (landscape mode) with clipmap rings of 2D renders
  (`landscape-mode.md`); game modes: daily challenge, atlas (first discoverer names it), canyon
  racing (`gamification.md`).

---

## 1. Engine or no engine?

### Takeaway
Do not adopt an engine. The product is a compute renderer with a thin game layer; every engine
option either forces you to re-host your renderer inside its GPU abstraction (Bevy, Godot, Fyrox)
or wraps a native plugin in a much heavier runtime (Unity, Unreal). Build a custom Rust app on
**wgpu + winit**, with a UI library for menus, and take Bevy's ECS only if game-state complexity
later demands it (the Tiny Glade pattern).

### Cited Findings

**(a) wgpu + winit custom app**
- wgpu 30.0.0 was released 2026-07-01 and 30.0.1 on 2026-08-21; majors in the last year were
  v27 (2025-10-01), v28 (2025-12-17), v29 (2026-03-18), v30 (2026-07-01) — roughly one breaking
  major per quarter. — [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- crates.io (queried 2026-10-14): wgpu 30.0.1, winit 0.30.13 (updated 2026-09-04),
  egui 0.36.2 (2026-09-08). — [crates.io wgpu](https://crates.io/crates/wgpu),
  [crates.io winit](https://crates.io/crates/winit), [crates.io egui](https://crates.io/crates/egui)
- wgpu 30 added explicit surface colour-space selection (HDR / wide-gamut output). —
  [wgpu CHANGELOG v30.0.0](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- egui is "very mouse-oriented"; gamepad navigation has historically come from add-on crates such
  as bevy-egui-kbgp rather than egui itself. — [bevy_egui_kbgp docs](https://docs.rs/bevy-egui-kbgp)
  (No source found confirming built-in egui gamepad focus navigation in 2026; see Gaps.)

**(b) Bevy**
- Release history: 0.14 (2024-07-04), 0.15 (2024-11-29), 0.16 (2025-04-24), 0.17 (2025-09-30),
  0.18 (2026-01-13), 0.19 (2026-06-19), 0.20 (2026-10-08). Still pre-1.0. —
  [Bevy news](https://bevy.org/news/); [crates.io bevy](https://crates.io/crates/bevy)
- Bevy 0.20 (227 contributors, 817 PRs): Solari path tracer (runs on macOS via Metal without a
  built-in denoiser), DLSS-RR 4.5 via `dlss_wgpu`, mesh shaders, **adopted WESL and removed its
  custom WGSL dialect (custom shaders must be renamed `.wgsl`→`.wesl`)**, Feathers widgets
  (dropdown, list view, colour input), BSN scene format changes; **no editor ships**; panics in
  systems now route to an error handler. — [Bevy 0.20 release notes](https://bevy.org/news/bevy-0-20/)
- Bevy is built on wgpu (implied by `dlss_wgpu` and Solari-on-Metal in the 0.20 notes). —
  [Bevy 0.20](https://bevy.org/news/bevy-0-20/)
- Tiny Glade (Pounce Light, two-person studio) "was made with a custom version of Bevy". —
  [Wikipedia: Tiny Glade](https://en.wikipedia.org/wiki/Tiny_Glade). Secondary sources say the
  shipped game uses Bevy's ECS with its own Vulkan renderer, not Bevy's renderer. —
  [Lemmy thread](https://lemmy.world/post/25423912); [GamingOnLinux (Rust + Vulkan, native Linux)](https://gamingonlinux.com/2024/09/chill-diorama-builder-tiny-glade-is-out-now/comment_id=269684).
  A Pounce Light fork of Bevy exists on GitHub. — [TinyPinch (fork of PounceLight/bevy)](https://github.com/TinyPinch)
  (Primary developer statement not retrieved; treat the "ECS-only + custom Vulkan renderer"
  detail as secondary.)

**(c) Godot 4 + godot-rust (gdext)**
- godot-rust `godot` crate is at 0.5.5 (2026-08-09) — still 0.x. —
  [crates.io godot](https://crates.io/crates/godot)
- Godot 4.7 docs: compute shaders are GLSL 450, only on RenderingDevice renderers (Forward+ or
  Mobile), work is recorded into a compute list then `submit()`/`sync()`; long work can trigger a
  Windows TDR and must be split; RIDs must be freed manually; local RenderingDevices cannot be
  debugged with RenderDoc. — [Godot docs: Using compute shaders](https://docs.godotengine.org/en/stable/tutorials/shaders/compute_shaders.html)
- Godot gained a native Metal backend in 4.4 (Apple Silicon only at first); the driver borrows
  heavily from MoltenVK. — [Phoronix](https://www.phoronix.com/news/Godot-4.4-Dev-1-Engine);
  [Godot Metal driver README mirror](https://git.objectionable.solutions/Sara/behaviour-tree-test/src/commit/df66ea74d7ab575b03ce48c358e9cdf89370dd31/drivers/metal).
  Anecdotal reports of multi-second first-launch shader translation stalls on Metal. —
  [Apple Developer Forums](https://developer.apple.com/forums/thread/824902)
- GodotSteam is maintained against current SDKs (1.62–1.64 builds, Godot 4.4–4.6.1). —
  [GodotSteam changelog](https://godotsteam.com/changelog/gdextension);
  [GodotSteam SDK blog](https://godotsteam.com/blog/category/sdk)

**(d) Unity / Unreal with Rust as a native plugin**
- Unity cancelled the Runtime Fee in September 2024 for all versions including Unity 6; Personal
  stays free with the revenue/funding ceiling raised from $100k to $200k. —
  [Unity blog: canceling the Runtime Fee](https://unity.com/blog/unity-is-canceling-the-runtime-fee)
- Unity Pro is listed at $2,310/yr per seat (or $210/mo), required above $200k funding/revenue;
  Enterprise above $25M. — [Unity pricing updates](https://unity.com/pricing-updates)
  (Figures from search snippet of the pricing page; verify before quoting.)
- Unreal: 5% royalty on lifetime gross revenue above $1M per product, 3.5% if launched on the Epic
  Games Store first/simultaneously; quarters under $10k owe nothing. —
  [Unreal Engine FAQ](https://www.unrealengine.com/en-US/faq);
  [Unreal royalty reporting news](https://www.unrealengine.com/news/unreal-engines-improved-royalty-reporting-system)

**(e) Fyrox and others**
- Fyrox reached 1.0 (1.0.1 on 2026-03-28). — [crates.io fyrox](https://crates.io/crates/fyrox)
  (Did not research Fyrox's compute support; see Gaps.)

### Inferences
- **The renderer is the product, and engines are built around their own renderer.** Our needs —
  subgroup prefix sums in a compute kernel, indirect dispatch, per-command-buffer GPU timestamps
  feeding a throughput model, 16 ms chunk budgets, presenting compute output — are all things an
  engine either hides (Unity/Unreal) or exposes through a second-class path (Godot's GLSL
  RenderingDevice with manual RID lifetime; Bevy's render-world plumbing). In all four cases the
  `GpuBackend` + scheduler would be re-hosted inside someone else's frame loop and someone else's
  breakage schedule.
- **Bevy:** best fit of the engines (same language, wgpu underneath, compute possible), but you
  would pay a ~3–6-month breaking-change tax (seven releases in 27 months; 0.20 alone forces a
  shader-dialect migration) for features you do not use (meshes, PBR, scenes, assets). Using only
  `bevy_ecs` (as Tiny Glade reportedly did) is the sane Bevy option, and it is optional: our game
  state (a camera, a few rules, a leaderboard) does not need an ECS. Revisit only if canyon racing
  grows into many entities (ghosts, checkpoints, particles).
- **Godot + gdext:** worst-of-both: our compute would have to be rewritten in GLSL for
  RenderingDevice, timestamps/watchdog control are not exposed the way the scheduler needs, the
  Rust binding is 0.x, and the Mac build would run Godot's Metal driver instead of our tuned
  objc2-metal path. Its real strengths (editor, UI, GodotSteam, gamepad UI navigation) are
  attractive for menus only — not enough.
- **Unity/Unreal:** licensing is now benign for an indie (Unity Personal free under $200k; Unreal
  5% only above $1M), so cost is not the blocker. The blocker is architecture: a C-ABI native
  plugin that owns its own GPU device cannot cheaply share textures with the engine's swapchain;
  you would end up using the engine as a UI/menu shell around a black box, shipping a
  100–200 MB runtime for a fractal viewer, and hiring/learning C#/C++ for no rendering gain.
- **Custom wgpu app (recommended):** the scheduler already polls submissions and takes time as an
  argument, which is exactly the shape of a winit event loop. The only real gaps are UI quality
  and gamepad navigation; both are solvable in a few hundred lines (a custom in-game menu drawn by
  our own compositor, or egui with a focus-navigation layer) and Steam Input can map the Deck
  controls to keyboard/mouse-like actions.
- Hiring/community: Rust gamedev is small; but this project's contributors are Rust contributors
  already, and none of the engines would reduce the specialised GPU work.

### Gaps
- Primary Pounce Light statement on exactly which Bevy parts Tiny Glade ships (only secondary
  sources found).
- Whether egui 0.36 has native gamepad/focus navigation; no source found.
- Unity's 2026 terms page could not be read in full (only snippets); Fyrox compute/UI capabilities
  not researched; no data on Iced/Slint gamepad support in 2026.
- Bevy compute-shader ergonomics in 0.20 were not covered by the release notes.

---

## 2. GPU backend for Windows/Linux: wgpu vs ash (Vulkan) vs DX12, and shader sharing

### Takeaway
Implement `GpuBackend` on **wgpu** with hand-ported **WGSL** kernels; keep the Metal backend on
macOS. wgpu 30 covers every feature the scheduler uses (subgroups incl. prefix sums, indirect
dispatch, pass timestamps, immediates, f32 atomics if needed, passthrough shaders as an escape
hatch). Do not write raw Vulkan/DX12 unless profiling shows a wgpu overhead that matters — for
three big dispatches per 16 ms chunk it won't.

### Cited Findings
- `Features::SUBGROUP`: subgroup built-ins in compute and fragment, Vulkan/DX12/Metal, native
  only; `SUBGROUP_BARRIER`: Vulkan/Metal. — [wgpu Features docs (30.0.1)](https://docs.rs/wgpu/latest/wgpu/struct.Features.html)
- wgpu's Metal adapter enables `SUBGROUP | SUBGROUP_BARRIER` when the device supports SIMD-scoped
  operations; naga's MSL writer emits `metal::simd_sum(...)` etc. for subgroup operations. —
  [wgpu-hal metal/adapter.rs](https://github.com/gfx-rs/wgpu/blob/trunk/wgpu-hal/src/metal/adapter.rs);
  [naga back/msl/writer.rs](https://github.com/gfx-rs/wgpu/blob/trunk/naga/src/back/msl/writer.rs)
- WebGPU/WGSL subgroup work continued into 2026 (`subgroup_id`, `num_subgroups` in Chrome 144;
  `subgroup_uniformity` in 145; `@subgroup_size` / subgroup-size-control in Chrome 151–152). —
  [Chrome: New in WebGPU 144](https://developer.chrome.com/blog/new-in-webgpu-144);
  [145](https://developer.chrome.com/blog/new-in-webgpu-145);
  [151–152](https://developer.chrome.com/blog/new-in-webgpu-151-152)
- wgpu 26–27 renamed `min/max_subgroup_size` from `Limits` to `AdapterInfo`. —
  [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- `TIMESTAMP_QUERY` (timestamps at compute/render pass begin/end): Vulkan, DX12, Metal, GL, WebGPU.
  `TIMESTAMP_QUERY_INSIDE_ENCODERS` / `_INSIDE_PASSES`: Vulkan, DX12, GL, and Metal **only on
  AMD/Intel, not Apple GPUs**. — [wgpu Features docs](https://docs.rs/wgpu/latest/wgpu/struct.Features.html)
- `IMMEDIATES` (formerly push constants; renamed in v28 to match the WebGPU "immediate data"
  proposal, `var<immediate>`): DX12, Vulkan, Metal, GL (emulated), WebGPU. DX12 issue advises a
  struct for immediates. — [wgpu Features docs](https://docs.rs/wgpu/latest/wgpu/struct.Features.html);
  [wgpu CHANGELOG v28](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- Indirect dispatch is core API; v29 renamed `dispatch_indirect` → `dispatch_workgroups_indirect`. —
  [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- `PASSTHROUGH_SHADERS` (stable since v29; was `EXPERIMENTAL_PASSTHROUGH_SHADERS`) lets you hand
  native SPIR-V / MSL / metallib / DXIL / GLSL to wgpu, with explicit pipeline layouts. Metal
  compute MSL passthrough was added in v25-era (#7326). —
  [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- `SHADER_FLOAT32_ATOMIC`: Metal (MSL 3.0+, Apple7+/Mac2), Vulkan (`VK_EXT_shader_atomic_float`);
  64-bit integer atomics behind `SHADER_INT64_ATOMIC_*` (Metal 3.1 only min/max). `SHADER_F64`:
  Vulkan only. `PIPELINE_CACHE`: Vulkan only. — [wgpu Features docs](https://docs.rs/wgpu/latest/wgpu/struct.Features.html);
  [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- WGSL has pipeline-overridable constants (`override`), the analogue of Metal function constants
  (WGSL spec feature; wgpu exposes them via `PipelineCompilationOptions.constants`). — [wgpu CHANGELOG v30 snippet showing `compilation_options`](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- Fast math: wgpu's Metal backend creates `MTLCompileOptions` setting only language version,
  `preserveInvariance = true` and optional logging — it exposes no math-mode control. —
  [wgpu-hal metal/device.rs](https://github.com/gfx-rs/wgpu/blob/trunk/wgpu-hal/src/metal/device.rs).
  Apple: `mathMode` replaces `fastMathEnabled`; safe mode forbids result-changing transforms;
  relaxed keeps Inf/NaN. — [Apple MTLCompileOptions.mathMode](https://developer.apple.com/documentation/metal/mtlcompileoptions/mathmode).
  Even with fast math off, MSL permits FP32 denormals to be flushed to zero with no query. —
  [Apple Developer Forums thread 844598](https://developer.apple.com/forums/thread/844598)
- Slang: Khronos took over Slang (Nov 2024, Apache-2.0); targets SPIR-V, HLSL, GLSL, WGSL and MSL;
  named a preferred shading language in LunarG's 2026 Vulkan ecosystem survey. —
  [Khronos press release](https://www.khronos.org/news/press/khronos-group-launches-slang-initiative-hosting-open-source-compiler-contributed-by-nvidia);
  [Khronos Shading Languages Symposium 2026](https://www.khronos.org/events/shading-languages-symposium-2026)
- Comparable Rust projects: **Vello** (0.11.0, 2026-10-02) runs on wgpu with hand-written WGSL
  compute shaders (mostly prefix-sum variants), a custom `#import` preprocessor, and CPU ports of
  the shaders for fallback; it was explicitly a research project on subgroups and device-scoped
  barriers. — [Vello ARCHITECTURE.md](https://skia.googlesource.com/external/github.com/linebender/vello/+/refs/tags/v0.1.0/ARCHITECTURE.md);
  [crates.io vello](https://crates.io/crates/vello). wgpu v30.0.1 even fixed indirect dispatch on
  the iOS simulator because "compute renderers such as vello previously failed". —
  [wgpu CHANGELOG](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)
- **CubeCL / Burn** (cubecl 0.11.0, 2026-10-06): kernels written in Rust (`#[cube]`) compiled to
  WGSL, SPIR-V, Metal, CUDA; "planes" map to Vulkan subgroups / Metal SIMD-groups with runtime
  `PLANE_DIM`. — [CubeCL repo](https://github.com/tracel-ai/cubecl);
  [crates.io cubecl](https://crates.io/crates/cubecl)
- **rust-gpu**: community-maintained since Embark handed it over (2024); README says early stage,
  not production-ready (snapshot ~Jan 2025). — [awesome.ecosyste.ms rust-gpu](https://awesome.ecosyste.ms/projects/github.com%2FRust-GPU%2Frust-gpu);
  [NVIDIA Rust+GPU ecosystem map](https://nvidia.github.io/cuda-rust/appendix/ecosystem.html)
- `ash` last published 0.38.0+1.3.281 (2024-04-01) — no release in 2.5 years. —
  [crates.io ash](https://crates.io/crates/ash)

### Inferences
- **Feature mapping is complete:** buffers/textures/storage textures, `override` bools for
  `kDE`/`kInterior`, `set_bytes` → immediates (or a small uniform ring), `dispatch_indirect` →
  `dispatch_workgroups_indirect`, `simd_prefix_exclusive_sum`/`simd_sum` → WGSL
  `subgroupExclusiveAdd`/`subgroupAdd`, `threadgroup` → `var<workgroup>`, `atomic_uint` →
  `atomic<u32>`, per-submission `gpu_time()` → a `QuerySet` of 2 timestamps per compute pass
  (`timestamp_writes` on the pass is enough; inside-pass timestamps are not needed). Submission
  status maps to `queue.on_submitted_work_done` / `device.poll`. Present: render or copy the
  colorised storage texture to the surface texture (surface formats generally can't be storage
  targets everywhere, so plan one blit).
- **What wgpu costs you:** (1) one breaking major per quarter — pin a version and upgrade
  deliberately; (2) no fast-math control and no guarantee of bit-identical f32 across backends —
  so the "bit-identical iteration fields" acceptance rule must become a tolerance rule (the CPU
  oracle + verify metrics already support that) for non-Metal backends; (3) subgroup size varies
  (32 on NVIDIA, 32/64 on AMD, 8–32 on Intel, 32 on Apple) — the kernel's `sgLive[32]` threadgroup
  arrays assume ≤32 subgroups per group, which must be sized from `AdapterInfo.subgroup_min_size`
  or the workgroup size; (4) validation overhead per call is irrelevant at 3 dispatches per chunk.
- **Watchdog/TDR:** Windows TDR (~2 s default) and Linux GPU hangs behave like the Metal watchdog;
  the existing chunking + "failed chunk divides the rate by 8" logic carries over, but device loss
  on Windows is a full device-lost event in wgpu, so the backend must recreate the device rather
  than just retry a command buffer.
- **Why not ash/DX12:** two raw backends to maintain for one product, `ash` has no release since
  2024, and a Vulkan-only path still leaves Windows driver quality to chance. wgpu on Windows picks
  DX12 or Vulkan; wgpu on Linux/Deck uses Vulkan (RADV on the Deck). If a hot path ever needs a
  feature wgpu lacks, `PASSTHROUGH_SHADERS` (SPIR-V/DXIL) and `wgpu_hal` interop are the escape
  hatch without leaving wgpu.
- **Why not wgpu on macOS too:** the objc2-metal backend is tuned (drawable-pool predictor,
  `addPresentedHandler`, fast-math parity), and wgpu's Metal path has no inside-pass timestamps
  on Apple GPUs and no math-mode control. Keep Metal native on Apple; the trait already isolates it.
- **Shader sharing — recommendation: hand-maintain MSL and WGSL now, generate later.**
  The kernel set is small (prepare, iterate, finish, colorize; ~535 lines). Options ranked:
  1. *Hand-port to WGSL, keep MSL* (Vello's approach): lowest risk, preserves Metal parity and
     fast-math; cost is dual maintenance, mitigated by the existing CPU oracle + `verify` metrics
     run against both backends in CI.
  2. *WGSL as single source, naga → MSL* for the Metal backend: kills duplication, but loses
     function-constant/fast-math parity and the bit-identical guarantee; only worth it once
     formulas multiply.
  3. *Slang* → MSL + WGSL/SPIR-V: the most capable single source (generics, modules), but adds a
     C++ toolchain (or prebuilt binaries) to the build and its WGSL/MSL output quality for
     subgroup-heavy compute is unverified here.
  4. *CubeCL / rust-gpu* (kernels in Rust): attractive in theory (one language, generics for
     float32/floatexp), but CubeCL is ML-oriented with a fast-moving 0.x API and rust-gpu is
     self-described as not production-ready — not for a shipping product in 2026.
  Long-term, the wiki's "kernels generated from the formula opcode model" line points to option 2
  or a small in-house codegen emitting both MSL and WGSL from the opcode model.
- **Ray marcher (landscape mode):** a height-field ray marcher is plain compute (or a fullscreen
  fragment pass) over storage textures — wgpu handles it; no ray-tracing extensions needed.

### Gaps
- No benchmark found comparing wgpu vs raw Vulkan for a few-large-dispatch compute workload; the
  "overhead is irrelevant" claim is inference.
- Whether naga's WGSL→SPIR-V/HLSL emits any relaxed-precision or fast-math decorations by
  default was not verified (grep of the SPIR-V writer found none).
- Slang's MSL/WGSL output for subgroup prefix operations not tested or documented in sources found.

---

## 3. Steamworks from Rust

### Takeaway
Use **steamworks-rs** (crate `steamworks`) behind a cargo feature in a dedicated crate. It is
active in 2026, wraps the interfaces we need (UserStats = achievements/stats/leaderboards, UGC =
Workshop, RemoteStorage = Cloud, Input, Timeline, Screenshots), and links Valve's redistributable
`steam_api` dynamically on all three OSes.

### Cited Findings
- Versions: 0.13.1 (2026-05-05), 0.13.0 (2026-04-14), 0.12.x (mid-2025), 0.11.0 (2024-02). —
  [crates.io steamworks](https://crates.io/crates/steamworks)
- Recent commits: 2026-08-27 ("Replace unmaintained paste dependency"), 2026-05 achievements
  example fixes; 43 open issues, 13 open PRs, ~485 stars. README compatibility table: 0.13.0 and
  git → Steamworks SDK **1.64** (MSRV 1.80.0); 0.12.0 → SDK 1.62; 0.11.0 → 1.58a. So the crate
  tracks Valve's SDK within months (GodotSteam reports SDK 1.64 as the latest it shipped against).
  — [steamworks-rs README](https://github.com/Noxime/steamworks-rs/blob/master/README.md);
  [GitHub commits API](https://api.github.com/repos/Noxime/steamworks-rs/commits);
  [GodotSteam SDK blog](https://godotsteam.com/blog/category/sdk)
- Wrapped interfaces (docs.rs 0.13.1): `Client`, `Server`, `Apps`, `Friends`, `Input`,
  `Matchmaking`, `MatchmakingServers`, `Networking`, `RemotePlay`, `RemoteStorage`, `UGC`, `User`,
  `UserStats`, `Utils`, plus modules `networking_messages`, `networking_sockets`, `screenshots`,
  `stats`, `timeline`. — [docs.rs steamworks](https://docs.rs/steamworks/latest/steamworks/)
- `steamworks-sys` uses bindgen and links `steam_api64.dll`/`steam_api.lib` (Windows),
  `libsteam_api.so` (linux32/linux64/linuxarm64), `libsteam_api.dylib` (osx), copying them to the
  build output; the libraries are loaded dynamically and must sit next to the binary. Crate code is
  MIT/Apache-2.0 except the bundled Steam files. —
  [steamworks-sys build.rs](https://github.com/Noxime/steamworks-rs/blob/master/steamworks-sys/build.rs);
  [steamworks-rs README](https://github.com/Noxime/steamworks-rs)

### Inferences
- Coverage maps cleanly to the game design: daily challenge → leaderboards (`UserStats`, one board
  per day, score + time, details blob carrying the coordinate); atlas → either leaderboards with
  attached UGC or a tiny own backend (Steam has no "first claim wins" primitive; Workshop UGC can
  carry named discoveries but ordering/uniqueness must be enforced server-side); canyon racing
  ghosts → leaderboard attachments (UGC) for flight-recorder files; settings/bookmarks → Steam
  Cloud via `RemoteStorage` or Auto-Cloud (no code); Deck controls → Steam Input action sets.
- Alternatives are worse: writing against the flat C API yourself re-does what `steamworks-sys`
  generates; Facepunch.Steamworks is C#-only and only relevant with Unity. Fallback if the crate
  stalls: fork it (bindgen over the SDK headers keeps it cheap to regenerate).
- Keep Steam strictly optional at runtime: a `SteamPlatform` trait implementation behind
  `feature = "steam"`, with a no-op/local implementation for the DRM-free/itch/App Store builds.

### Gaps
- Quality of `UGC` and `Input` wrappers (completeness of callbacks) not verified by code review.

---

## 4. macOS on Steam: reuse the SwiftUI app or the Rust UI?

### Takeaway
Ship the **cross-platform Rust app (winit + wgpu-UI layer, but using the native Metal backend)**
on Steam for macOS, and keep the SwiftUI app for the Mac App Store and iOS. Steam forbids the App
Sandbox entitlement that the Mac App Store requires, so the two Mac builds are different bundles
anyway; making the Steam Mac build the same code as the Windows/Linux build keeps game features,
menus and Steam integration single-sourced.

### Cited Findings
- Steam requires new macOS apps to be 64-bit and **notarized**; developers tick "App Bundles Are
  Notarized". Recommended entitlements: `com.apple.security.cs.disable-library-validation` and
  `com.apple.security.cs.allow-dyld-environment-variables` (overlay injection). "Steam is not
  currently compatible with the `com.apple.security.app-sandbox` entitlement" — leave it out even
  if you also ship on the Mac App Store. — [Steamworks: Platforms](https://partner.steamgames.com/doc/store/application/platforms)
- The page still states Intel Mac / OS X 10.11 minimum and does not discuss Apple Silicon. —
  [Steamworks: Platforms](https://partner.steamgames.com/doc/store/application/platforms)
- The current Apple build pipeline is cargo → XCFramework (macOS arm64, iOS, iOS-sim) → Swift
  package; Swift does only UI and input decoding; the render loop is Rust behind an `Explorer`
  object fed a `CAMetalLayer` pointer. — `kb/wiki/platform/rust-swift-ffi.md`

### Inferences
- **Option A — SwiftUI app + Steamworks:** best Mac-native feel and zero extra UI work for the
  explorer, but the game layer (menus, daily challenge, leaderboards, ghosts, achievements UI)
  would be built twice (SwiftUI and Rust UI), Steam callbacks would need a Swift↔Rust bridge, and
  the Steam variant still needs its own unsandboxed signing configuration. Every game feature then
  has two UIs to test.
- **Option B — Rust shell on macOS (recommended for Steam):** the same `almondbread-app` binary
  with `MetalBackend` (via `raw-window-handle` → `CAMetalLayer` from winit's NSView) instead of
  the wgpu backend. One game UI, one Steam integration, one input/gamepad path; the Mac-native
  explorer remains in the App Store. The cost is that the Steam Mac build looks like a game, not a
  Mac app — which is right for Steam.
- Mac is a small share of Steam; do not let it shape the architecture. The cheapest acceptable
  Steam Mac build is Option B; if it lags, ship Steam on Windows + Linux/Deck first.
- Universal binary: Steam's page is silent on arm64; ship arm64 (+ optionally x86_64 via `lipo`).
  Metal 3.0/MSL 3.0 already excludes old Intel Macs; set the store's minimum OS accordingly.

### Gaps
- No current Steam statement on Apple Silicon/universal-binary requirements or macOS share in
  2026 found (the Steam Hardware Survey was not consulted here).

---

## 5. Recommended code organisation, packaging and CI

### Takeaway
Add four crates — `almondbread-wgpu` (backend), `almondbread-game` (rules, modes, scoring;
platform-neutral), `almondbread-platform` traits with `almondbread-steam` behind a feature, and
`almondbread-app` (winit shell + UI) — and keep `math`/`core`/`render` untouched except for
generalising the shader and surface abstractions. One repo, two product lines: Xcode/Swift for
the App Store, cargo for Steam.

### Cited Findings
- Steam Deck: Valve tests the native Linux build first if one exists and falls back to the
  Windows build under Proton if Linux fails; developers can request a re-review naming the
  platform. — [GamingOnLinux: Valve clarifies Deck testing](https://www.gamingonlinux.com/2022/02/valve-clarifies-how-they-test-native-linux-or-proton-for-steam-deck/page=1/)
- Some developers ship native Linux and still recommend Proton for stability, and there is no
  official way to make Proton the default when a native build exists. —
  [GamingOnLinux: Escape Simulator](https://gamingonlinux.com/2025/06/escape-simulator-has-a-native-linux-version-again-devs-still-recommend-proton)
- Steam Mac builds must be notarized and must not use App Sandbox. —
  [Steamworks: Platforms](https://partner.steamgames.com/doc/store/application/platforms)
- macOS notarization in CI is typically a separate, re-runnable step after signing (slow/flaky),
  followed by stapling; Windows Authenticode signing from GitHub secrets (`.pfx`) with SignTool
  timestamp flags depending on the CA. —
  [AWS Labs signing runbook](https://awslabs.github.io/filemoverexpress/docs/Signing-Runbook);
  [xcode-notarize action](https://github.com/GuillaumeFalourd/xcode-notarize);
  [Wails signing guide](https://wails.io/docs/guides/signing)

### Inferences

**Crate layout**

```text
crates/
  almondbread-math      (unchanged, forbid(unsafe))
  almondbread-core      (unchanged; + game-neutral pieces: share codes, seeds)
  almondbread-render    GpuBackend trait, scheduler, Host, shaders/{msl,wgsl}/,
                        metal/ (cfg apple)  -- add a Surface trait so Host is not CAMetalLayer-typed
  almondbread-wgpu      NEW: impl GpuBackend for wgpu (Vulkan/DX12; optional Metal for testing),
                        WGSL kernels, wgpu Surface; behind cfg(not apple) by default
  almondbread-game      NEW, platform-neutral, no GPU: daily-challenge seeding and scoring,
                        atlas classification + naming rules, mission/race rules, flight recorder,
                        replay/ghost format; pure functions over core types; fully unit-tested
  almondbread-platform  NEW: traits Achievements, Leaderboards, CloudSave, Ugc, Overlay;
                        a `local` impl (files, no network)
  almondbread-steam     NEW: impl of the platform traits on steamworks-rs; feature "steam"
  almondbread-app       NEW: the Steam/desktop binary: winit event loop, input (keyboard, mouse,
                        gamepad via gilrs or Steam Input), UI (egui or own immediate UI),
                        picks MetalBackend on macOS, WgpuBackend elsewhere
  almondbread-cli       + `--backend wgpu|metal` so verify/bench/regress run per backend in CI
  almondbread-ffi       Apple only; may later expose almondbread-game to SwiftUI if the App
                        Store edition adopts the daily challenge
```

- **What stays platform-neutral:** math, core, game, platform traits, the scheduler. Platform
  code lives only in `render/metal`, `wgpu`, `steam`, `app`, `ffi`.
- **Two product lines from one repo:** App Store = Xcode project + XCFramework (as today, sandboxed,
  no Steam). Steam = `cargo build -p almondbread-app --features steam` per OS, depots per OS.
  Optionally a DRM-free build without `steam` (itch.io).
- **Correctness across backends:** extend `almondbread-cli verify` to compare wgpu output against
  the CPU oracle with tolerances; run it in CI on a Linux runner with a software Vulkan driver
  (lavapipe) for functional checks, and on real GPUs locally for performance. Keep bit-identity
  as the rule only for Metal-vs-Metal.
- **CI:** GitHub Actions matrix: `macos-latest` (arm64: Metal tests, app bundle, codesign,
  notarize, staple), `windows-latest` (msvc build, Authenticode sign), `ubuntu` building inside
  the Steam Runtime ("sniper") SDK container so the Linux binary runs on the Deck's runtime; then
  `steamcmd` uploads to a beta branch. cargo-dist/`dist` is aimed at CLI tool releases (installers,
  Homebrew) and does not upload Steam depots; use it only for the CLI, and script the app
  packaging (bundle layout, `steam_api` lib copy, `steam_appid.txt` only for dev).
- **Cross-compilation:** do not cross-compile the shipping binaries — native runners per OS are
  simpler for signing and linking `steam_api`. Keep the existing `cargo check` cross targets as a
  fast guard.
- **Order of work:** (1) wgpu backend + WGSL port behind the CLI, verified against the oracle;
  (2) minimal `almondbread-app` on Linux/Windows (explorer parity); (3) game crate (daily
  challenge) + steam crate (leaderboards, achievements, cloud); (4) Deck pass (Steam Input action
  set, 1280×800 UI, 30/40 Hz power budget — chunk budget adapts already); (5) Mac Steam build via
  the same app with MetalBackend.

### Gaps
- cargo-dist's current (2026) support for macOS notarization / Windows signing was not verified
  from its docs; the statement that it doesn't upload Steam depots is inference.
- Steam Runtime (sniper/"steamrt") SDK container specifics and whether Valve now requires it for
  Deck Verified were not fetched; the Steamworks Linux/Steam Runtime docs should be consulted.
- No primary source checked for Steam Input API coverage in steamworks-rs beyond the `Input`
  struct's existence.
