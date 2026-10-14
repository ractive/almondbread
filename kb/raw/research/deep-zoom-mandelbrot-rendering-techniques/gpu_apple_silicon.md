# GPU / Apple Silicon considerations for a Metal deep-zoom Mandelbrot renderer (as of Oct 2026)

Research budget note: about 15 tool calls. Several Apple developer pages failed to render in the fetch tool (JS-only pages). Claims marked **[unverified]** come from background knowledge or from community reports and were not confirmed against a primary source in this session.

## Apple GPU architecture facts relevant to compute (fp32/fp16/fp64, SIMD width, occupancy, threadgroup memory, dynamic caching, unified memory, divergence, watchdog)

### Takeaway
Apple GPUs (M1 class) are fp32/fp16 machines: 256 FMA/core/cycle for both fp32 and fp16, SIMD width 32, about 208 KB of registers and about 60 KB of threadgroup memory per core. There is no hardware fp64. Emulated fp64 runs at roughly 1/36 to 1/68 of fp32. Integer multiply is slow (32 per core-cycle, 1/8 of fp32 FMA), so float-float beats integer-limb arithmetic for perturbation inner loops. M3 and later add Dynamic Caching, which softens the occupancy cost of register-heavy kernels such as floatexp or double-float. macOS kills command buffers after a few seconds (about 5 s reported), so long renders must be split into chunked dispatches across several command buffers.

### Cited Findings
- M1/A15 GPU: 256 fp32 FMA per core-cycle (A14: 128); fp32 adds 128 per core-cycle; fp16 FMA also 256 per core-cycle. fp16 therefore brings no ALU throughput gain over fp32 on M1, only register and bandwidth savings — [philipturner/metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- Register file about 208 KB per core; threadgroup memory about 60 KB per core; 8 KB L1 data cache; 12 KB instruction cache; 384–3072 threads per core depending on register use — [metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- SIMD-group = 32 threads — [metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- "ALU utilization maxes out at 24 simds/core." Apple's compiler prefers spilling registers to device memory over reducing occupancy below that — [metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- Int32 multiply (IMUL32/IMAD32): 4-cycle throughput, i.e. 32 per core-cycle versus 256 fp32 FMAs. 32x32→64 multiply about 8 cycles; IMULHI32 about 8 cycles. IADD64 is emulated (about 4 cycles) — [metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- FFMA32 latency is given as about 2.21 cycles in one table and as 6 cycles in the ALU bottlenecks section. The page does not reconcile the two figures (**conflicting**) — [metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- No native fp64. Emulated fp64 (an "FP59 e11m48" format) costs about 1:36 for add, 1:52 for mul and 1:68 for FMA relative to fp32. The author says a non-IEEE e8m48 format could roughly triple throughput — [metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- M1 Pro: 16 GPU cores at 1.296 GHz, about 5.3 TFLOPS fp32. The same page also references a 14-core M1 Pro variant; both bins exist — [metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- Philip Turner (Apple dev forums) argued that integer-based fp64 emulation beats double-single, and that 64-bit integer ops are too slow to use. His theoretical estimate for full IEEE fp64 emulation is ">27x for multiply and >>4x for addition, not including the time to process exponents" — [Apple Developer Forums thread 718737](https://developer.apple.com/forums/thread/718737)
- M3 Dynamic Caching: before M3, a shader's worst-case register use set occupancy for the whole dispatch. M3 allocates registers from on-chip storage according to live usage, so more SIMD-groups run concurrently. Register, threadgroup and tile memory now share one flexible on-chip pool, and Apple says no app changes are required — [Apple tech talk 111375](https://developer-mdn.apple.com/videos/play/tech-talks/111375); [AppleInsider explainer](https://appleinsider.com/articles/23/11/10/what-apples-three-gpu-enhancements-in-a17-pro-and-m3-actually-do); [MacStories](https://www.macstories.net/linked/jason-snells-hands-on-with-the-m3-macbook-pros-and-imac/)
- M5-generation talk: second-generation dynamic caching with lower register access latency and an occupancy management unit. Apple warns that too many SIMD-groups can exceed on-chip storage and cause spills and memory stalls — [Apple tech talk 111431](https://developer.apple.com/kr/videos/play/tech-talks/111431/)
- macOS GPU watchdog: community reports put it at about 5 seconds, counted per command buffer rather than per dispatch, so several dispatches in one command buffer do not reset the timer. Time spent waiting on a fence also counts. The error shows up as `kIOGPUCommandBufferCallbackErrorTimeout`, and Apple documents an error for a device whose access was revoked after too many timeouts. Mitigation is to split work across command buffers. `AGX_RELAX_CDM_CTXSTORE_TIMEOUT=1` was reported as an unofficial workaround (**community reports, not Apple docs**) — [MLX issue #3302](https://upd.dev/ml-explore/mlx/issues/3302); [MLX issue #3267](https://upd.dev/ml-explore/mlx/issues/3267); [Apple forums 46817](https://developer.apple.com/forums/thread/46817); [MTLCommandBufferError.Code](https://developer.apple.com/documentation/metal/mtlcommandbuffererror-swift.struct/code.md)
- Fraktaler 3 hits the same problem with OpenCL. Larger tiles are faster, but tiles that are too large, or excessive perturbation or BLA step limits, cause OS GPU timeouts. In one test location, a 960x1080 tile took 1m34s versus 3 minutes with the default 128x128 tiles — [Fraktaler 3 manual](https://fraktaler.mathr.co.uk/)
- Unified memory / `storageModeShared`: CPU and GPU share one physical memory, so a reference orbit written by the CPU into a shared `MTLBuffer` needs no copy (**[unverified] in this session**; standard Apple guidance, see [Apple: Choosing a resource storage mode for Apple GPUs](https://developer.apple.com/documentation/metal/choosing-a-resource-storage-mode-for-apple-gpus))

### Inferences
- Size the perturbation kernel for fp32 FMA. Integer-limb arithmetic is 8x slower at the multiply level on M1, so use float-float (two fp32) instead of u32-limb fixed point where precision beyond 24 bits is needed in the delta loop.
- Divergence: one SIMD-group of 32 pixels runs as long as its slowest pixel. Practical mitigations:
  - Iteration chunking. Each dispatch advances every live pixel by N iterations (e.g. 1k–100k, tuned to stay well under 5 s per command buffer; target about 16–50 ms for UI responsiveness), and per-pixel state (dz, iteration count, reference index) persists in a buffer between dispatches.
  - Compaction. After each chunk, build a list of still-escaping pixels with an atomic counter and launch the next chunk with indirect dispatch over only those pixels. This is a "work queue" approach that Metal supports through `dispatchThreadgroups(indirectBuffer:)`.
  - Persistent threads. Because there is no preemption and the watchdog is per command buffer, persistent "while queue not empty" kernels are riskier on macOS than on CUDA and must be bounded by an iteration budget.
- Register-heavy floatexp/double-float kernels lose occupancy on M1/M2, because worst-case registers set occupancy; M3+ is more forgiving. Keep separate specialized kernels per number type, so the fp32 fast path is not penalized by the register footprint of the floatexp path.
- fp16 is useless for deltas (range and precision) but could store smooth-iteration or colouring outputs.

### Gaps
- No primary Apple document found stating the exact watchdog threshold, or whether it differs with no display attached or on headless machines.
- No microbenchmark found for M3/M4/M5 fp32 throughput per core or any change to int32 multiply rate.
- The occupancy-versus-register-count table for Apple GPUs was not found (metal-benchmarks gives no table).

## Double-float (float-float) emulation and Metal fast-math pitfalls

### Takeaway
Double-float gives about 48 significand bits with fp32 exponent range. It is a precision fix, not a range fix. Its error-free transforms (TwoSum, TwoProd via fma) break if the compiler reassociates or contracts expressions. Metal's runtime compiler historically enables fast math by default (**[unverified]**), so any double-float code compiled with `makeLibrary(source:options:)` must set `MTLCompileOptions.mathMode = .safe` (or disable fastMathEnabled on older SDKs) and be validated numerically on real hardware. Typical cost is an order of magnitude or more over native fp32.

### Cited Findings
- Double-single (hi+lo) gives up to about 48 binary digits (about 14 decimal) with f32 exponent range. It is not IEEE binary64, and precision tapers toward 24 bits when the low limb becomes subnormal (result magnitude about 2^-126 to 2^-102) — [luma.gl GPU floating-point precision guide](https://luma.gl/docs/api-guide/shaders/gpu-floating-point-precision)
- TwoSum, FastTwoSum, Dekker split and TwoProd depend on exact rounding events, and "a compiler that reassociates the expression can therefore erase the information." Parentheses, temporaries, runtime zeros, or `fma` spelling are "not portable guarantees" — [luma.gl](https://luma.gl/docs/api-guide/shaders/gpu-floating-point-precision)
- The Dekker split constant multiply (4097·a) can overflow for large finite inputs — [luma.gl](https://luma.gl/docs/api-guide/shaders/gpu-floating-point-precision)
- luma.gl automatically switches to an integer-controlled double-single path (u32 limbs, round-to-nearest-even) on Apple WebGPU adapters, because Metal/WGSL may reassociate or contract float expressions. It recommends testing especially on Metal and reports numerical error alongside runtime in its Mandelbrot benchmark, because "the fastest implementation is not useful if the compiler has optimized away its residual terms" — [luma.gl](https://luma.gl/docs/api-guide/shaders/gpu-floating-point-precision); [luma.gl fp64 Mandelbrot example](https://luma.gl/next/examples/experimental/fp64)
- Emulated extended precision costs "more than an order of magnitude" over native fp32. Classic WebGL double-single measured about 1.5 ulp for multiplication and about 2 ulp for division — [luma.gl fp64 module docs](https://luma.gl/next/docs/api-reference/shadertools/shader-modules/fp64) (summarized via search snippet)
- Multiplication round-off can be recovered with one FMA (TwoProd). General TwoSum needs 6 flops (the source says 5 add/sub for the error term); FastTwoSum needs 2 when |a| ≥ |b| — [Wanted: Floating-Point Add Round-off Error instruction (arXiv 1603.00491)](https://arxiv.org/pdf/1603.00491)
- Thall's paper describes df64 (pairs of floats) arithmetic, exp and trig for GPUs in Cg, with accuracy data on several GPUs. Exact per-op flop counts were not retrieved — [SIGGRAPH archive](https://history.siggraph.org/?p=139442); [hgpu listing](https://hgpu.org/?p=1806)
- For double-double on CUDA (same algorithmic structure, one level up), the measured average overhead is about 37.7x the base precision's operations — [Least Squares on GPUs in Multiple Double Precision, arXiv 2110.08375](https://arxiv.org/pdf/2110.08375)
- Apple metal-benchmarks: hardware fp32 FMA exists at full rate, so `fma()` in MSL maps to native FFMA32 — [metal-benchmarks](https://github.com/philipturner/metal-benchmarks)
- `MTLCompileOptions.mathMode` (`.safe`, `.relaxed`, `.fast`) supersedes the deprecated `fastMathEnabled`; `mathFloatingPointFunctions` (`.fast`/`.precise`) controls library-function precision. **The Apple page did not render; option semantics and OS versions are [unverified]** (likely macOS 15 / iOS 18 era) — [Apple: MTLCompileOptions.mathMode](https://developer.apple.com/documentation/metal/mtlcompileoptions/mathmode)

### Inferences
- Approximate op counts using the standard algorithms (derived, not measured on Apple):
  - df add (full TwoSum + renormalization): about 11–20 fp32 ops.
  - df mul with fma-based TwoProd: about 3–8 ops.
  - A complex df square (the Mandelbrot hot path): about 30–50 fp32 ops versus about 3–4 for plain fp32. Expect roughly 10–20x slowdown versus fp32 perturbation.
- Since native fp32 FMA is full rate and int mul is 1/8 rate on Apple GPUs, classic fma-based float-float should outrun integer-controlled double-single on Metal, if the compiler is stopped from reassociating. Test with a known-answer kernel, e.g. compute TwoSum(1, 1e-8) and check that the low limb is nonzero, at app startup.
- Safer approach: set `.safe` math mode only for the library containing double-float code and keep `.fast`/`.relaxed` for plain fp32 kernels. Mark error-term code paths with `precise::`/explicit `fma()`. Note that metal-benchmarks shows fp32 throughput is the same either way; fast math mainly affects transcendental and NaN handling, so `.safe` costs little for perturbation.
- In deep-zoom perturbation, the delta dz needs range more than precision. Double-float is mostly useful for the reference orbit storage at shallow depth (<1e-7 to 1e-14 zoom, where plain fp32 deltas can't resolve pixel spacing relative to the reference), or for a direct (non-perturbation) renderer up to about 1e13 zoom.

### Gaps
- No primary source verifying whether Metal's default runtime compile options enable fast math, or exactly what `.relaxed` permits (contraction yes/no).
- No measured Apple-GPU benchmark of double-float add/mul throughput found.
- Thall's per-operation cost table was not retrieved.

## Extended-range float (floatexp) on GPU and when to switch number types

### Takeaway
floatexp = fp32 mantissa + int32 exponent. It is the standard way past the fp32 range limit (about 1e-38) for perturbation deltas. Renderers pick the cheapest type per zoom depth or location: fp32 (shallow), rescaled fp32 / floatexp-with-BLA (mid), full floatexp (deep), and doubleexp where fp32 mantissa precision is insufficient. Fraktaler 3 picks by benchmarking ("wisdom").

### Cited Findings
- floatexp combines a normal float (usually 32-bit single precision is sufficient) with an explicit integer exponent to extend range — [Munafo, Mu-Ency: floatexp](https://mrob.com/pub/muency/floatexp.html)
- Fraktaler 3 number types: float (low range, can cause iteration-band glitches with high powers), double, floatexp, doubleexp (double-precision floatexp, for when single-precision floatexp is not precise enough), float128, and long double on x86. "Wisdom" benchmarks devices and types and selects the best per location. Version 3.1 was released 2025-12-15 and added doubleexp and float128 — [Fraktaler 3 manual](https://fraktaler.mathr.co.uk/)
- FractalShark GPU types: float, double, double-float, a "2x32 + exponent" type (pair of fp32 plus a shared exponent, about 48-bit mantissa) and "float + exponent" HDR types used with linear approximation. The README points to FractalShark.pdf §§8–9 for details — [FractalShark README](https://github.com/mattsaccount364/FractalShark)
- Double-single is a precision fix only, so a floatexp-style exponent is still needed for very deep zooms — [luma.gl](https://luma.gl/docs/api-guide/shaders/gpu-floating-point-precision)

### Inferences
- Cost model on Apple GPU: a floatexp multiply is one fp32 mul, one int add, and an occasional renormalize (extract exponent via `frexp` or bit ops, then `ldexp`). Add needs exponent alignment (compare, shift via `ldexp`, select), which is branchy and costs about 5–10x an fp32 op. Integer ops here are adds and shifts, which run at a much higher rate than int multiplies.
- Common switch strategy, following the KF/Fraktaler lineage:
  - Zoom < ~1e30: plain fp32 deltas (with fp32 reference orbit) work for perturbation if pixel spacing stays above about 1e-38 across all iterations. Practically, use fp32 up to about 1e30 with a safety margin, as denormals and underflow of dz near the reference cause glitches.
  - ~1e30 to ~1e300+: "rescaled fp32". Store dz as fp32 times a per-pixel scale exponent that is renormalized only when |dz| drifts. This is nearly fp32 speed most iterations, with full floatexp only near reference escape or rebase.
  - Beyond: full floatexp for BLA coefficients and deltas.
- With BLA, the BLA table coefficients need floatexp range at depth even when per-iteration deltas can be rescaled.

### Gaps
- No published benchmark for floatexp versus fp32 cost on Metal or any GPU was found. FractalShark.pdf likely has numbers but was not retrieved.
- The "rescaled float" technique details (as in Fraktaler 3 / Imagina) were not confirmed from a primary source here.

## Existing GPU deep-zoom implementations and their design

### Takeaway
The serious GPU deep zoomers are CUDA (FractalShark) or OpenCL (Fraktaler 3, KF2+). All use a CPU (or GPU) high-precision reference orbit plus a GPU perturbation/LA kernel in fp32-based types with an explicit exponent. No mature Metal-native deep zoomer with BLA was found. WebGPU projects stay shallow (≤1e250) or use double-single without floatexp.

### Cited Findings
- **FractalShark** (Matt Reiman, CUDA):
  - Two CUDA linear-approximation implementations ported from FractalZoomer; the author says he found no other public CUDA LA implementation.
  - Precision types: float, double, double-float, 2x32+exponent, HDR float+exponent.
  - GPU reference orbit ("Ref2/v2", a persistent fused CUDA kernel using NTT multiplication).
  - Multithreaded CPU reference orbit "MT3": 3 threads, two for squaring and one coordinating, best on an AMD 5950X; MPIR+AVX2 on Windows, GMP on Linux.
  - Reference-orbit compression saved several GB at period 600M.
  - Requires compute capability 7.5+.
  - Author's claims: GPU reference orbit about 10x faster than multithreaded CPU on an RTX 4090 at 16384 limbs (~158k digits); about 70x versus single-threaded MPIR on an RTX 5090; GPU reference orbit likely wins beyond about 1e10000 zoom; deepest example about 1e650452. (**author's own benchmarks**)
  - Source: [FractalShark](https://github.com/mattsaccount364/FractalShark)
- **Fraktaler 3** (Claude Heiland-Allen):
  - OpenCL on CPU and GPU devices at once, alongside CPU code. GLSL is used only for colouring snippets; the GUI uses OpenGL ES 3 / WebGL 2.
  - BLA with a configurable step limit ("a few thousand is usually sufficient"); BLA skip levels reduce memory.
  - Tile-based scheduling; OpenCL-on-CPU is typically faster than its plain CPU code.
  - The web version is significantly slower than native.
  - Source: [Fraktaler 3](https://fraktaler.mathr.co.uk/)
- **Imagina** (5E-324): files include `FloatExp.h`, `FloatExpVector4.h`, `HInfLAEvaluator.cpp` (hierarchical LA), `JitEvaluator.cpp`; depends on MPIR. The README does not document a GPU backend and says the project "is being rewritten" — [Imagina](https://github.com/5E-324/Imagina)
- **luma.gl fp64 Mandelbrot**: a WebGL/WebGPU fragment-shader demo comparing f32 with double-single on the same zoom path. This is a direct (non-perturbation) renderer, so depth is limited to about 1e13 — [luma.gl example](https://luma.gl/next/examples/experimental/fp64)
- **Corgi** (KiranWells): GPU compute shaders, tested on the Vulkan backend. It uses "a combination of several precision-extending techniques" and reaches over 1e250 magnification, often in real time. Artifacts appear above about 1e90 near some minibrots — [Corgi](https://github.com/KiranWells/corgi)
- **fractbox engine**: WebGPU with an extended-precision reference orbit plus an f32 delta kernel (3D distance-estimated, so only indirectly relevant) — [fractbox-engine](https://github.com/fractbox/fractbox-engine)

### Inferences
- Design converges on: high-precision reference orbit on CPU (GMP/MPFR), stored as fp32/floatexp (or double) per iteration in a GPU buffer; BLA table built on CPU or GPU; the GPU kernel does rebasing (Zhuoran) plus BLA lookups per pixel. A Metal renderer can follow FractalShark's type ladder (float → 2x32 → float+exp HDR), since Apple lacks fp64 just like consumer NVIDIA effectively does (1/64 rate).
- Kalles Fraktaler 2+ OpenCL, Shadertoy perturbation demos and Phil Thompson's very-plotter were not researched in this session.

### Gaps
- No reported speed numbers (pixels/s or iterations/s) for any GPU deep zoomer were found except FractalShark's reference-orbit speedups.
- KF2+ OpenCL design (which types, how it handles fp64-less GPUs) not retrieved.
- No Metal-native deep zoom (Swift/Metal) project found in searches; one may exist.

## CPU side on Apple Silicon (reference orbit, GMP/MPFR, multithreading, NEON)

### Takeaway
GMP runs very well on M1. Most mpn inner loops run below 1 cycle/limb, and in 2020 M1 took #2 on GMPbench (behind a 4.4 GHz Ryzen). mpn_addmul_1 is weaker because ARM64 has a single carry flag and no high-word multiply-add. FractalShark's 3-thread reference orbit (two threads squaring, one coordinating) is the known parallelization pattern. Hand-tuned fixed-size multiplication can beat GMP basecase by up to 80% on M1 for mid-size limbs.

### Cited Findings
- GMP developers on M1: "almost every inner loop runs at < 1 cycle/limb"; the 3.2 GHz M1 took #2 on GMPbench; mpn_addmul_1 is slower due to a single carry flag and no high-word multiply-add — [gmp-discuss, Nov 2020](https://gmplib.org/list-archives/gmp-discuss/2020-November/006600.html); [GMP ARM corner](https://gmplib.org/devel/arm)
- GMP's arm64 `mul_1` lists Apple M1 at 1 cycle/limb versus 7–8 on Cortex-A53 — [GMP mpn/arm64/mul_1.asm](https://third-party-mirror.googlesource.com/gmp/+/refs/heads/master/mpn/arm64/mul_1.asm)
- GMPbench has an Apple M1 (3200 MHz, clang 17) entry; the score was not captured — [GMPbench](https://gmplib.org/gmpbench)
- Fixed-size multiplication kernels (N=1–15 limbs) run 80% faster than mpn_mul_basecase on M1 for N=8 (single-author result) — [gmp-devel, Apr 2024](https://gmplib.org/list-archives/gmp-devel/2024-April/006255.html)
- Ahlbäck & Johansson report up to 2x over GMP for basecase-sized multiplication on recent ARM64/x86-64 — [basecasemul.pdf](https://fredrikj.net/math/basecasemul.pdf)
- FractalShark MT3: 3 threads (two squaring, one coordinating); MT5 currently falls back to MT3 — [FractalShark](https://github.com/mattsaccount364/FractalShark)

### Inferences
- At typical deep zooms (1e100 to 1e10000, i.e. about 350–35k bits), reference orbit cost is dominated by 2–3 multiplications per iteration at basecase or Toom sizes. On M1 Pro, the 3-thread split (computing x², y², (x+y)² or x·y concurrently on P-cores) is a direct port. Rust `rug` wraps GMP/MPFR with the same performance. A custom fixed-point mpn-style implementation with fixed limb count can win (no allocation, no exponent handling), per the 80% basecase result.
- AMX/SME is not useful for multiprecision carry chains. NEON lacks 64x64→128 multiply, so scalar `umulh`/`mul` (what GMP uses) is the right path.
- A CPU fallback perturbation renderer with NEON processes 4 fp32 or 2 fp64 lanes. On CPU, fp64 deltas (plus floatexp beyond 1e300) are the natural choice, unlike the GPU.

### Gaps
- No MPFR-specific M1 benchmark found. No published M1 Pro reference-orbit timing for any deep-zoom program found.
- NEON CPU-fallback deep-zoom performance numbers not found.

## Metal-specific tooling and pipeline tips (runtime compilation, function constants, indirect dispatch, MTLSharedEvent, MetalFX)

### Takeaway
Use `makeLibrary(source:options:)` with explicit `mathMode`, and function constants to specialize kernels per formula, number type and feature (BLA on/off, derivatives). Add indirect dispatch for compacted live-pixel lists, shared events or completion handlers for CPU/GPU overlap (e.g. the next reference orbit or BLA build while the GPU renders), and optionally MetalFX upscaling for interactive previews. Most of these Apple API points could not be verified in this session because Apple's doc pages did not render.

### Cited Findings
- `MTLCompileOptions.mathMode` / `mathFloatingPointFunctions` exist and replace `fastMathEnabled` (**semantics [unverified]**) — [Apple docs](https://developer.apple.com/documentation/metal/mtlcompileoptions/mathmode)
- `MTLCommandBuffer` exposes status, error codes and `kernelEndTime`, useful for timing chunks to adapt iteration budget per dispatch — [MTLCommandBufferStatus](https://developer.apple.com/tutorials/data/documentation/metal/mtlcommandbufferstatus.md); [kernelEndTime](https://developer.apple.com/documentation/metal/mtlcommandbuffer/kernelendtime.md)
- Long fence/event waits inside a command buffer count toward the watchdog (community report), so do not park GPU work waiting on CPU events in the same buffer for long — [MLX #3302](https://upd.dev/ml-explore/mlx/issues/3302)

### Inferences
- Function constants (`MTLFunctionConstantValues` + `makeFunction(name:constantValues:)`) let one MSL source generate per-number-type and per-formula pipelines with dead code removed. This keeps register pressure of the fp32 path low, which matters on M1/M2 where worst-case registers set occupancy. **[unverified API details; standard Metal feature]**
- Pipeline: for each frame, commit command buffers of about 10–30 ms of work each, and use GPU timestamps (`gpuStartTime`/`gpuEndTime`) to adapt the iteration chunk size. This gives progressive display and no watchdog kills.
- MetalFX spatial upscaling could render previews at 50% resolution while zooming. Temporal upscaling needs motion vectors and jitter, which are easy to derive for 2D zoom/pan, but it could smear fractal detail (**speculative**).
- `storageModeShared` buffers for the reference orbit and BLA table avoid copies. The CPU can write the next reference orbit into a second buffer while the GPU renders with the first (double buffering, synchronized via `MTLSharedEvent` or command-buffer completion handlers).

### Gaps
- Apple WWDC sessions on compute optimization ("Optimize Metal performance for Apple silicon", "Discover advances in Metal for A17 Pro/M3") were not fetched; their specific recommendations (e.g. threadgroup sizing, avoiding 64-bit ints) remain unverified here.
- No source found on MetalFX use for fractal or scientific rendering.
