import Foundation
import Metal
import MetalKit
import Observation
import QuartzCore

/// Drives the GPU passes (chunked iterate, colorize) for an MTKView, or for an offscreen texture.
///
/// Rendering is a *job* per view: one or two *levels* (an optional coarse preview, then the
/// full resolution), each iterated in resumable chunks of ~10–20 ms of GPU time
/// (kb: platform/gpu-watchdog-and-chunking). A chunk is one command buffer: size the indirect
/// dispatch, iterate every live pixel (plus freshly admitted ones) for at most K iterations,
/// record the live count. K follows from a measured throughput, so long frames never hold
/// the GPU for long and the watchdog never sees a multi-second command buffer.
///
/// The view redraws at display rate while a job runs; each draw harvests finished chunks,
/// encodes the next ones within a time budget and presents a progressive image (unfinished
/// pixels show the preview underneath). A changed view starts a new job (new generation);
/// work of older generations is ignored, and since the queue is serial and every admitted
/// pixel is rewritten by its own job, no stale chunk can show in a newer frame.
///
/// Main-actor bound: the model is UI state, and MTKView calls the delegate on the main thread.
@MainActor
public final class Renderer: NSObject, MTKViewDelegate {
    public let model: FractalModel
    let device: MTLDevice
    private let queue: MTLCommandQueue
    private let library: MTLLibrary
    private let preparePipeline: MTLComputePipelineState
    private let finishPipeline: MTLComputePipelineState
    private let colorizePipeline: MTLComputePipelineState
    /// Iterate pipelines specialised by function constants, keyed by `de | interior << 1`.
    private var iteratePipelines: [Int: MTLComputePipelineState] = [:]

    /// Iteration results for one texture size. Full-resolution and preview levels keep
    /// separate sets, so the preview can stay visible under the refining full level.
    private final class TextureSet {
        let iter: MTLTexture   // rg32Float: escape iteration n, smooth offset; -1 interior, -2 pending
        let de: MTLTexture     // rg16Float: distance estimate (texels), normal angle
        let color: MTLTexture? // rgba16Float linear colour of a preview level, for bilinear upsampling
        init(iter: MTLTexture, de: MTLTexture, color: MTLTexture?) { self.iter = iter; self.de = de; self.color = color }
        var width: Int { iter.width }
        var height: Int { iter.height }
    }
    private var fullSet: TextureSet?
    private var previewSets: [TextureSet] = []   // most recently used last

    // Chunk scheduler buffers, shared by all levels (levels run one after another).
    /// Live pixel states, ping-ponged: a chunk reads one and appends survivors to the other.
    private var stateBuffers: [MTLBuffer] = []
    /// Live counts belonging to `stateBuffers`.
    private let countBuffers: [MTLBuffer]
    private let statsBuffer: MTLBuffer       // ChunkStats ring, read by the CPU
    private let argsBuffer: MTLBuffer        // indirect dispatch arguments
    private let levelBuffer: MTLBuffer       // LevelCounters of the running level (GPU-side admission)
    private static let statSlots = 64
    private var statSlot = 0
    private var ping = 0
    /// Live-list capacity: bounds state memory (40 B per slot, two buffers) independently of
    /// the image size. The GPU admits fresh pixels into free capacity at the start of each
    /// chunk, so a large frame needs no CPU round trip per million pixels.
    private let liveCapacity: Int
    private let iterateThreads: Int

    private var reference: ReferenceOrbit?
    private var referenceTime = 0.0
    private var referenceBuffer: MTLBuffer?

    private var paletteID: String?
    private var paletteBuffer: MTLBuffer?
    private var paletteSize = 0

    private(set) var lastInteraction: CFTimeInterval = 0

    // Scheduling state.
    private var job: Job?
    private var generation = 0
    private var inFlight: [ChunkRecord] = []
    /// Effective throughput, lane-iterations per second (live-or-admitted pixels x K per GPU
    /// second). Adapted from every chunk; persists across jobs.
    private var rate: Double = 3e10
    /// GPU seconds per texel of the last completed level, for the preview decision.
    private var costPerTexel: Double = 0
    private var hasPresented = false
    private var framesInFlight = 0
    /// When the last fully refined, full-resolution frame was committed, and its texture size.
    private(set) var lastFinalFrame: (time: CFTimeInterval, size: String) = (0, "")
    private var lastFrameCommit: CFTimeInterval = 0
    /// Frames committed but not finished on the GPU. 1 and 2 measured the same latency; 2
    /// keeps the GPU busy when a frame's work is close to a refresh interval.
    private let maxFramesInFlight = 2
    /// Input younger than this counts as an ongoing gesture.
    private let idleDelay = 0.15
    /// GPU time a gesture frame's iteration may take: a 120 Hz refresh minus colorize.
    private let interactiveFrameBudget = 0.006
    private var lastStatsPublish: CFTimeInterval = 0

    /// GPU time per command buffer the scheduler aims for when idle (interaction uses
    /// `interactiveChunkTarget`). Offscreen renders use the same scheduler.
    public var chunkTarget: Double = 0.016
    public var interactiveChunkTarget: Double = 0.008
    /// Upper bound on queued chunk command buffers of the current job.
    private let maxInFlight = 3

    /// Called on every frame before rendering (for animations). Return true to keep animating.
    public var onFrame: ((_ dt: Double) -> Bool)?
    private var lastFrameTime: CFTimeInterval = 0

    /// Asks the hosting view to redraw.
    public var requestRedraw: (() -> Void)?
    /// Starts or stops continuous redraws (the view's display link).
    public var setAnimating: ((Bool) -> Void)?

    private let bailout: Double = 65536

    /// `ALMONDBREAD_TRACE=1`: print every harvested chunk (scheduler debugging, benchmarks).
    static let trace = ProcessInfo.processInfo.environment["ALMONDBREAD_TRACE"] == "1"

    fileprivate struct ComputeKey: Equatable {
        var viewport: Viewport
        var maxIter: Int
        var pointsPerPixel: Double
        var de: Bool
        var interior: Bool
        var width: Int        // output pixels
        var height: Int
        var supersampling: Int
    }

    private final class Level {
        let set: TextureSet
        let ratio: Double            // texels per output pixel
        var params: IterateParams    // template; chunk fields filled per dispatch
        let total: Int
        // As of the last harvested chunk (the GPU is ahead of these):
        var admitted = 0             // pixels admitted
        var live = 0                 // pixels still iterating
        var encoded = 0              // chunks encoded
        var done = false
        var gpuTime = 0.0
        var iterations = 0.0         // executed, from the chunk counters
        var interiorPixels = 0       // finished as interior (derivative test or maxIter)
        var useInterior = false      // derivative test on; decided when the level starts
        let preview: Bool            // a coarse level shown until the full level is done
        init(set: TextureSet, ratio: Double, preview: Bool, params: IterateParams) {
            self.set = set; self.ratio = ratio; self.preview = preview; self.params = params
            total = set.width * set.height
        }
        var started: Bool { encoded > 0 }
    }

    private final class Job {
        let generation: Int
        let key: ComputeKey
        let referenceBuffer: MTLBuffer
        var levels: [Level]
        let start = CACurrentMediaTime()
        var interactive = false      // a gesture frame: preview only, superseded every frame
        var presented = false        // a gesture frame is shown once; repeats only cost drawables
        var fullLevel: (() -> Level)?
        var gpuTime = 0.0
        var chunks = 0
        var iterations = 0.0
        var failed = false
        var finalPresented = false
        init(generation: Int, key: ComputeKey, referenceBuffer: MTLBuffer, levels: [Level]) {
            self.generation = generation; self.key = key
            self.referenceBuffer = referenceBuffer; self.levels = levels
        }
        var done: Bool { levels.allSatisfy(\.done) }
        var current: Level? { levels.first { !$0.done } }
        /// The level to show: the furthest one that has started.
        var display: Level { levels.last { $0.started } ?? levels[0] }
    }

    private struct ChunkRecord {
        let cb: MTLCommandBuffer
        let generation: Int
        let level: Level
        let slot: Int
        let iterations: Int     // K
        let adaptive: Bool      // K came from the throughput model (feeds `rate` back)
    }

    public init?(model: FractalModel) {
        checkGPULayouts()
        guard let device = MTLCreateSystemDefaultDevice(),
              let queue = device.makeCommandQueue() else { return nil }
        self.model = model
        self.device = device
        self.queue = queue
        do {
            library = try Renderer.makeLibrary(device: device)
            guard let prepare = library.makeFunction(name: "prepareChunk"),
                  let finish = library.makeFunction(name: "finishChunk"),
                  let colorize = library.makeFunction(name: "colorize") else {
                NSLog("Almondbread: kernel functions missing from library")
                return nil
            }
            preparePipeline = try device.makeComputePipelineState(function: prepare)
            finishPipeline = try device.makeComputePipelineState(function: finish)
            colorizePipeline = try device.makeComputePipelineState(function: colorize)
        } catch {
            NSLog("Almondbread: Metal setup failed: \(error)")
            return nil
        }
        #if os(macOS)
        var capacity = 1 << 20      // 2 x 40 MB
        #else
        var capacity = 1 << 19      // 2 x 20 MB: iOS memory limits are much tighter
        #endif
        if let v = ProcessInfo.processInfo.environment["ALMONDBREAD_CAPACITY"].flatMap(Int.init), v > 0 { capacity = v }
        liveCapacity = capacity
        let stateBytes = liveCapacity * MemoryLayout<PixelState>.stride
        guard let s0 = device.makeBuffer(length: stateBytes, options: .storageModePrivate),
              let s1 = device.makeBuffer(length: stateBytes, options: .storageModePrivate),
              let c0 = device.makeBuffer(length: 4, options: .storageModePrivate),
              let c1 = device.makeBuffer(length: 4, options: .storageModePrivate),
              let stats = device.makeBuffer(length: Renderer.statSlots * MemoryLayout<ChunkStats>.stride,
                                            options: .storageModeShared),
              let args = device.makeBuffer(length: 16, options: .storageModePrivate),
              let level = device.makeBuffer(length: 16, options: .storageModePrivate) else { return nil }
        stateBuffers = [s0, s1]
        countBuffers = [c0, c1]
        statsBuffer = stats
        argsBuffer = args
        levelBuffer = level
        iterateThreads = 256
        super.init()
        observeModel()
    }

    /// The kernel library: the shipped source compiled with our options (every build: SwiftPM
    /// and Xcode copy `Mandelbrot.metalsrc` verbatim), or a precompiled default.metallib in
    /// the package bundle as a fallback, e.g. if a build system compiled the source itself.
    private static func makeLibrary(device: MTLDevice) throws -> MTLLibrary {
        if let url = Bundle.module.url(forResource: "Mandelbrot", withExtension: "metalsrc") {
            let source = try String(contentsOf: url, encoding: .utf8)
            let options = MTLCompileOptions()
            options.languageVersion = .version3_0
            // Fast math (FMA contraction, approximate log/atan2 in the epilogue) measured ~10%
            // faster than .safe on M1 Pro with no measurable accuracy change against a Double
            // reference render; the kernel has no NaN/inf checks that fast math could remove
            // (the interior test rescales w by exact powers of two instead of relying on inf).
            // Revisit when the floatexp/BLA kernels land, which rely on exact exponent handling.
            options.fastMathEnabled = true   // the default; explicit because accuracy depends on it
            return try device.makeLibrary(source: source, options: options)
        }
        if let library = try? device.makeDefaultLibrary(bundle: Bundle.module) {
            NSLog("Almondbread: kernel source not in bundle; using the precompiled default.metallib (build-time math options)")
            return library
        }
        throw NSError(domain: "Almondbread", code: 1, userInfo: [NSLocalizedDescriptionKey:
            "No Metal kernels in \(Bundle.module.bundlePath): neither Mandelbrot.metalsrc nor default.metallib"])
    }

    private func iteratePipeline(de: Bool, interior: Bool) -> MTLComputePipelineState? {
        let k = (de ? 1 : 0) | (interior ? 2 : 0)
        if let p = iteratePipelines[k] { return p }
        let values = MTLFunctionConstantValues()
        var d = de, i = interior
        values.setConstantValue(&d, type: .bool, index: 0)
        values.setConstantValue(&i, type: .bool, index: 1)
        do {
            let f = try library.makeFunction(name: "iterate", constantValues: values)
            let p = try device.makeComputePipelineState(function: f)
            iteratePipelines[k] = p
            return p
        } catch {
            NSLog("Almondbread: iterate pipeline failed: \(error)")
            return nil
        }
    }

    /// Redraws whenever a model property that affects the image changes.
    /// `withObservationTracking` fires once per registration, so re-register after each change.
    private func observeModel() {
        withObservationTracking {
            let m = model
            _ = (m.viewport, m.autoIterations, m.manualIterations, m.maxIterationsCap, m.interiorDetection)
            _ = (m.paletteID, m.colorDensity, m.colorOffset, m.logColoring, m.smoothColoring, m.interiorColor)
            _ = (m.shading, m.lightAngle, m.outline, m.outlineWidth, m.supersampling, m.interactivePreview)
        } onChange: { [weak self] in
            // onChange runs before the new value is stored: redraw on the next main-loop turn.
            DispatchQueue.main.async {
                self?.requestRedraw?()
                self?.observeModel()
            }
        }
    }

    /// Marks the start of a user interaction (pan, zoom gesture).
    public func noteInteraction() {
        lastInteraction = CACurrentMediaTime()
        if let s = frameStats, s.pendingInput == nil { s.pendingInput = lastInteraction }
    }

    /// Per-frame measurements for `--interaction-test`; nil (no cost) otherwise.
    /// Mutated only on the main queue (handlers hop there), hence @unchecked.
    final class FrameStats: @unchecked Sendable {
        var drawCPU: [Double] = []          // seconds spent in drawFrame
        var presented: [Double] = []        // presentedTime of each frame that reached the screen
        var latency: [Double] = []          // oldest unshown input -> presentedTime
        var inputToDraw: [Double] = []      // oldest unshown input -> start of the draw that shows it
        var drawToPresent: [Double] = []    // draw start -> presentedTime
        var chunkGPU = 0.0                  // iterate GPU seconds harvested
        var colorizeGPU = 0.0
        var frames = 0                      // frames encoded
        var previewFrames = 0               // frames showing a preview level
        var skipped = 0                     // draws that encoded nothing (frames in flight)
        var pendingInput: Double?
    }
    var frameStats: FrameStats?

    public nonisolated func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        MainActor.assumeIsolated { requestRedraw?() }
    }

    public nonisolated func draw(in view: MTKView) {
        MainActor.assumeIsolated { drawFrame(in: view) }
    }

    // MARK: Interactive frames

    private func drawFrame(in view: MTKView) {
        let drawableSize = view.drawableSize
        renderFrame(width: Int(drawableSize.width), height: Int(drawableSize.height),
                    pointsPerPixel: drawableSize.width > 0 ? Double(view.bounds.width) / drawableSize.width : 0) {
            view.currentDrawable.map { drawable in (drawable.texture, { cb in cb.present(drawable) }, drawable) }
        }
    }

    /// One display refresh, independent of the view (`simulate` drives it headlessly):
    /// harvest, start or keep the job, encode chunks, colorize into the target `acquire`
    /// returns (with a hook that presents it), schedule the next refresh.
    private func renderFrame(width: Int, height: Int, pointsPerPixel: Double,
                             acquire: () -> (MTLTexture, (MTLCommandBuffer) -> Void, MTLDrawable?)?) {
        let now = CACurrentMediaTime()
        defer { frameStats?.drawCPU.append(CACurrentMediaTime() - now) }
        let dt = lastFrameTime > 0 ? min(now - lastFrameTime, 0.1) : 1.0 / 60
        lastFrameTime = now
        var keepGoing = false
        if let onFrame {
            keepGoing = onFrame(dt)
            if keepGoing { noteInteraction() }
        }

        guard width > 0, height > 0, pointsPerPixel > 0 else { return }
        let interacting = now - lastInteraction < idleDelay

        // Never block the main thread on the GPU: with frames still queued, skip this display
        // refresh (the next one picks up the latest input) instead of waiting for a drawable.
        // Safety: a drawable that is never shown (hidden window) might not report presentation.
        if framesInFlight > 0 && now - lastFrameCommit > 0.25 { framesInFlight = 0 }
        if framesInFlight >= maxFramesInFlight {
            frameStats?.skipped += 1
            setAnimating?(true)
            return
        }
        harvest()
        let key = makeKey(width: width, height: height, pointsPerPixel: pointsPerPixel)
        if job?.failed == true { job = nil }
        if interacting {
            // Every frame of a gesture shows its own view: a preview level sized to fit the
            // frame budget, computed to completion (kb: techniques/frame-reuse-and-progressive).
            if job?.key != key || job?.interactive == false { startJob(key: key, mode: .interactive) }
        } else if let j = job, j.key == key {
            // Input idle: refine the gesture's last frame at full resolution over its preview.
            if j.interactive { extendForRefinement(j) }
        } else if let j = job, !j.interactive, !j.levels[0].done, now - j.start < 0.25 {
            // A settings change while idle: let a young first level finish before replacing
            // it, so the next slider step does not discard nearly finished work.
        } else {
            startJob(key: key, mode: .refine)
        }
        guard let job else { return }

        if Renderer.trace {
            print(String(format: "draw %.1f ms: job %d %@ level %dx%d interactive %@", (now - lastFrameTime) * 1000,
                         job.generation, job.display.preview ? "preview" : "full", job.display.set.width,
                         job.display.set.height, job.interactive ? "yes" : "no"))
        }
        if job.interactive {
            encodeInteractive(job: job)
        } else {
            encodeChunks(job: job, budget: chunkTarget)
        }

        // While refining, hold the previous image for a moment until the first level is
        // complete, so cheap views appear finished in one step instead of flashing unfinished
        // pixels. Gesture frames are complete by construction and always shown.
        let present = (job.interactive && (!job.presented || !job.done)) || !hasPresented || job.levels[0].done || now - job.start > 0.15
        if present, let (target, presentHook, drawable) = acquire(),
           let cb = encodeColorize(job: job, target: target) {
            presentHook(cb)
            if let stats = frameStats {
                stats.frames += 1
                if job.display.preview { stats.previewFrames += 1 }
                let input = stats.pendingInput
                stats.pendingInput = nil
                if let input { stats.inputToDraw.append(now - input) }
                #if os(macOS)   // presentation times are only collected for the macOS interaction test
                drawable?.addPresentedHandler { d in
                    let t = d.presentedTime
                    DispatchQueue.main.async {
                        guard t > 0 else { return }
                        stats.presented.append(t)
                        stats.drawToPresent.append(t - now)
                        if let input { stats.latency.append(t - input) }
                    }
                }
                #endif
                cb.addCompletedHandler { cb in
                    let g = cb.gpuEndTime - cb.gpuStartTime
                    DispatchQueue.main.async { stats.colorizeGPU += g }
                }
            }
            if job.done && !job.finalPresented && SelfTest.enabled {
                let size = "\(target.width)x\(target.height)"
                let gpu = job.gpuTime, chunks = job.chunks
                cb.addCompletedHandler { cb in
                    let ok = cb.error == nil
                    DispatchQueue.main.async {
                        SelfTest.finish(ok: ok, String(format: "renderer %@ %@ drawable, iterate GPU %.1f ms in %d chunks",
                                                       ok ? "OK" : "FAILED", size, gpu * 1000, chunks))
                    }
                }
            }
            // In flight until the GPU is done. Counting until presentation instead halved the
            // frame rate (the presented callback lands a refresh late) without lowering latency;
            // ~28 ms of draw-to-glass at 120 Hz is the window compositor (kb: synthesis/benchmarks).
            framesInFlight += 1
            lastFrameCommit = now
            cb.addCompletedHandler { [weak self] _ in
                DispatchQueue.main.async { if let self, self.framesInFlight > 0 { self.framesInFlight -= 1 } }
            }
            cb.commit()
            hasPresented = true
            job.presented = true
            if job.done && !job.interactive {
                job.finalPresented = true
                if !job.display.preview {
                    lastFinalFrame = (now, "\(job.display.set.width)x\(job.display.set.height)")
                }
            }
        }
        publishStats(job: job, force: job.done && !job.interactive)

        let busy = interacting || !job.finalPresented || job.key != key
        setAnimating?(keepGoing || busy)
    }

    private func makeKey(width: Int, height: Int, pointsPerPixel: Double) -> ComputeKey {
        ComputeKey(viewport: model.viewport, maxIter: model.maxIterations, pointsPerPixel: pointsPerPixel,
                   de: model.deNeeded, interior: model.interiorDetection, width: width, height: height,
                   supersampling: model.supersampling.rawValue)
    }

    // MARK: Jobs and chunks

    private enum JobMode { case interactive, refine, offscreen }

    private func startJob(key: ComputeKey, mode: JobMode) {
        let ss = Double(key.supersampling)
        let texW = key.width * key.supersampling, texH = key.height * key.supersampling
        guard let full = textureSet(width: texW, height: texH, preview: false) else { return }

        let viewport = key.viewport
        let viewRadius = viewport.scale * key.pointsPerPixel * Double(key.width + key.height)
        prepareReference(viewport: viewport, maxIter: key.maxIter, viewRadius: viewRadius)
        guard let reference, let referenceBuffer else { return }

        func params(ratio: Double, w: Int, h: Int) -> IterateParams {
            // Texel (0,0) is the top-left; the kernel flips y so imaginary grows upwards.
            IterateParams(
                halfSize: SIMD2(Float(w) / 2, Float(h) / 2),
                refOffset: SIMD2(Float(viewport.centerRe - reference.centerRe),
                                 Float(viewport.centerIm - reference.centerIm)),
                jitter: SIMD2(0.5, 0.5),
                texelScale: Float(viewport.scale * key.pointsPerPixel / ratio),
                bailout2: Float(bailout * bailout),
                maxIter: UInt32(key.maxIter),
                refLen: UInt32(reference.points.count),
                width: UInt32(w),
                total: UInt32(w * h),
                capacity: UInt32(liveCapacity),
                interiorEps: 1e-3)   // Chéritat's ε (kb: techniques/interior-detection)
        }

        func level(ratio: Double, preview: Bool) -> Level? {
            let w = max(1, Int((Double(key.width) * ratio).rounded(.up)))
            let h = max(1, Int((Double(key.height) * ratio).rounded(.up)))
            guard let set = preview ? textureSet(width: w, height: h, preview: true) : full else { return nil }
            return Level(set: set, ratio: ratio, preview: preview, params: params(ratio: ratio, w: w, h: h))
        }
        var levels: [Level] = []
        let fullTexels = Double(texW * texH)
        switch mode {
        case .interactive:
            // Dynamic resolution: the largest scale (1/8 steps, 1/4 ... 1) whose measured cost
            // fits the gesture frame budget; costPerTexel is refreshed by every completed level,
            // so the preview follows the cost as the view moves.
            var ratio = 0.5
            if costPerTexel > 0 {
                let fit = (interactiveFrameBudget / (costPerTexel * Double(key.width * key.height))).squareRoot()
                ratio = min(1, max(0.25, (fit * 8).rounded(.down) / 8))
            }
            if !model.interactivePreview { ratio = ss }
            if ratio >= ss {
                levels.append(Level(set: full, ratio: ss, preview: false, params: params(ratio: ss, w: texW, h: texH)))
            } else if let l = level(ratio: ratio, preview: true) {
                levels.append(l)
            }
        case .refine:
            // Preview first when the full level is expected to take more than about a frame;
            // quarter resolution if even half would.
            let fullCost = costPerTexel * fullTexels
            if model.interactivePreview && fullCost > 1.0 / 90 {
                let ratio = fullCost / ss / ss * 0.25 <= 1.0 / 60 ? 0.5 : 0.25
                if let l = level(ratio: ratio, preview: true) { levels.append(l) }
            }
            levels.append(Level(set: full, ratio: ss, preview: false, params: params(ratio: ss, w: texW, h: texH)))
        case .offscreen:
            levels.append(Level(set: full, ratio: ss, preview: false, params: params(ratio: ss, w: texW, h: texH)))
        }
        guard !levels.isEmpty else { return }
        generation += 1
        job = Job(generation: generation, key: key, referenceBuffer: referenceBuffer, levels: levels)
        job?.interactive = mode == .interactive
        job?.fullLevel = {
            Level(set: full, ratio: ss, preview: false, params: params(ratio: ss, w: texW, h: texH))
        }
    }

    /// Turns a finished gesture frame into a refinement job: same view, the full level added
    /// after the preview, which stays visible underneath while the full level fills in.
    private func extendForRefinement(_ job: Job) {
        job.interactive = false
        if !job.levels.contains(where: { !$0.preview }), let full = job.fullLevel?() {
            job.levels.append(full)
            job.finalPresented = false
        }
    }

    /// A gesture frame: the preview level in ceil(texels / capacity) chunks with K = maxIter,
    /// so it is complete when the frame's colorize runs. If the measured cost says that would
    /// take long (heavy views), fall back to budgeted progressive chunks so no command
    /// buffer gets near the watchdog.
    private func encodeInteractive(job: Job) {
        guard let level = job.levels.first else { return }
        guard level.encoded == 0 else {
            if !level.done { encodeChunks(job: job, budget: interactiveChunkTarget) }
            return
        }
        let estimate = costPerTexel * Double(level.total)
        if estimate > 0.05 {
            encodeChunks(job: job, budget: interactiveChunkTarget)
            return
        }
        let chunks = (level.total + liveCapacity - 1) / liveCapacity
        for _ in 0..<chunks { encodeChunk(job: job, level: level, k: Int(level.params.maxIter)) }
    }

    /// Encodes chunk command buffers for `job` until about `budget` seconds of estimated GPU
    /// work are queued or `maxInFlight` chunks are outstanding. Returns the number encoded.
    @discardableResult
    private func encodeChunks(job: Job, budget: Double) -> Int {
        var spent = 0.0
        var encoded = 0
        while encoded < 8 && spent < budget, !job.failed,
              inFlight.filter({ $0.generation == job.generation }).count < maxInFlight,
              inFlight.count < 2 * maxInFlight {
            guard let level = job.current else { break }
            // A level must be known finished before the next starts: they share the live lists.
            if let i = job.levels.firstIndex(where: { $0 === level }), i > 0, !job.levels[i - 1].done { break }
            // Upper bound on the threads of this chunk, from the last harvested counts.
            let threads = min(liveCapacity, level.live + level.total - level.admitted)
            // K from the throughput model: work <= threads x K lane-iterations.
            let target = max(budget - spent, 0.002)
            let k = Int(min(Double(level.params.maxIter), max(16, target * rate / Double(max(1, threads)))))

            guard encodeChunk(job: job, level: level, k: k, adaptive: true) else { break }
            spent += Double(threads) * Double(k) / rate
            encoded += 1
        }
        return encoded
    }

    /// Encodes and commits one chunk (its own command buffer: the watchdog counts per
    /// command buffer, and its GPU time measures this chunk alone).
    @discardableResult
    private func encodeChunk(job: Job, level: Level, k: Int, adaptive: Bool = false) -> Bool {
        if !level.started { level.useInterior = shouldUseInteriorTest(key: job.key) }
        guard let pipeline = iteratePipeline(de: job.key.de, interior: level.useInterior) else {
        job.failed = true
        return false
        }
        guard let cb = queue.makeCommandBuffer(), let enc = cb.makeComputeCommandEncoder() else { return false }
        cb.label = "iterate chunk"
        var p = level.params
        p.chunkIters = UInt32(k)
        p.statSlot = UInt32(statSlot)
        p.firstChunk = level.started ? 0 : 1
        var tg = UInt32(iterateThreads)
        let src = ping, dst = 1 - ping

        enc.setComputePipelineState(preparePipeline)
        enc.setBytes(&p, length: MemoryLayout<IterateParams>.stride, index: 0)
        enc.setBuffer(countBuffers[src], offset: 0, index: 4)
        enc.setBuffer(countBuffers[dst], offset: 0, index: 5)
        enc.setBuffer(statsBuffer, offset: 0, index: 6)
        enc.setBuffer(argsBuffer, offset: 0, index: 7)
        enc.setBytes(&tg, length: 4, index: 8)
        enc.setBuffer(levelBuffer, offset: 0, index: 9)
        enc.dispatchThreads(MTLSize(width: 1, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 1, height: 1, depth: 1))

        enc.setComputePipelineState(pipeline)
        enc.setTexture(level.set.iter, index: 0)
        enc.setTexture(level.set.de, index: 1)
        enc.setBuffer(job.referenceBuffer, offset: 0, index: 1)
        enc.setBuffer(stateBuffers[src], offset: 0, index: 2)
        enc.setBuffer(stateBuffers[dst], offset: 0, index: 3)
        enc.dispatchThreadgroups(indirectBuffer: argsBuffer, indirectBufferOffset: 0,
                                 threadsPerThreadgroup: MTLSize(width: iterateThreads, height: 1, depth: 1))

        enc.setComputePipelineState(finishPipeline)
        enc.dispatchThreads(MTLSize(width: 1, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 1, height: 1, depth: 1))
        enc.endEncoding()
        cb.commit()

        ping = dst
        level.encoded += 1
        inFlight.append(ChunkRecord(cb: cb, generation: job.generation, level: level, slot: statSlot,
                                    iterations: k, adaptive: adaptive))
        statSlot = (statSlot + 1) % Renderer.statSlots
        return true
    }

    /// Processes completed chunks in commit order: throughput, live counts, level completion.
    private func harvest() {
        while let rec = inFlight.first, rec.cb.status == .completed || rec.cb.status == .error {
            inFlight.removeFirst()
            let gpu = rec.cb.gpuEndTime - rec.cb.gpuStartTime
            if let error = rec.cb.error {
                // Most likely a watchdog timeout: assume the GPU is much slower than measured
                // and restart the job with smaller chunks (kb: platform/gpu-watchdog-and-chunking).
                NSLog("Almondbread: iterate chunk failed (\(error)); retrying with smaller chunks")
                rate /= 8
                if let job, job.generation == rec.generation { job.failed = true }
                continue
            }
            let stats = statsBuffer.contents().bindMemory(to: ChunkStats.self, capacity: Renderer.statSlots)[rec.slot]
            let work = Double(Int(stats.liveIn) + Int(stats.fresh)) * Double(rec.iterations)
            if rec.adaptive && gpu > 2e-4 && work > 0 {
                // Damped: at most x4 per chunk, so one odd chunk cannot swing K wildly.
                rate = min(max(work / gpu, rate / 4), rate * 4)
            }
            if Renderer.trace {
                print(String(format: "chunk gen %d slot %2d: liveIn %8d fresh %8d K %8d -> liveOut %8d  iters %.3g  interior %d (test %@)  gpu %.2f ms  rate %.3g",
                             rec.generation, rec.slot, stats.liveIn, stats.fresh, rec.iterations, stats.liveOut,
                             Double(stats.iterations), stats.interior, rec.level.useInterior ? "on" : "off", gpu * 1000, rate))
            }
            frameStats?.chunkGPU += gpu
            guard let job, job.generation == rec.generation else { continue }
            job.gpuTime += gpu
            job.chunks += 1
            job.iterations += Double(stats.iterations)
            let level = rec.level
            level.gpuTime += gpu
            level.iterations += Double(stats.iterations)
            level.interiorPixels += Int(stats.interior)
            level.admitted = Int(stats.admitted)
            level.live = Int(stats.liveOut)
            if level.live == 0 && level.admitted == level.total { finishLevel(level) }
        }
    }

    private func finishLevel(_ level: Level) {
        guard !level.done else { return }
        level.done = true
        if level.gpuTime > 0 { costPerTexel = level.gpuTime / Double(level.total) }
        // Interior statistics for the next level's decision. Without the test, interior pixels
        // ran to maxIter: take them out of the exterior iteration count.
        let n = Double(level.total)
        let interiorFraction = Double(level.interiorPixels) / n
        var exterior = level.iterations
        if !level.useInterior { exterior -= Double(level.interiorPixels) * Double(level.params.maxIter) }
        interiorStats = (interiorFraction, max(0, exterior) / n)
    }

    /// Interior fraction and exterior iterations per pixel of the last completed level.
    private var interiorStats: (fraction: Double, exteriorIterations: Double)?

    /// The derivative test costs ~50% more per iteration (measured with the every-8th-iteration
    /// check, M1 Pro); it pays off when interior pixels would otherwise burn maxIter each.
    /// Decided per level from the last level's statistics, as GBy advises: on when a minibrot
    /// or the main set is in view (kb: techniques/interior-detection).
    private func shouldUseInteriorTest(key: ComputeKey) -> Bool {
        guard key.interior else { return false }
        guard let s = interiorStats else { return true }   // unknown: be safe at high maxIter
        let saved = s.fraction * Double(key.maxIter)
        return saved > interiorTestOverhead * s.exteriorIterations
    }
    private let interiorTestOverhead = 0.6

    private func publishStats(job: Job, force: Bool) {
        let now = CACurrentMediaTime()
        guard force || now - lastStatsPublish > 0.1 else { return }
        lastStatsPublish = now
        let m = model
        // Only publish when changed: every write would invalidate SwiftUI views reading it.
        if m.shownViewport != job.key.viewport { m.shownViewport = job.key.viewport }
        if m.shownMaxIterations != job.key.maxIter { m.shownMaxIterations = job.key.maxIter }
        if m.lastReferenceTime != referenceTime { m.lastReferenceTime = referenceTime }
        if let r = reference, m.lastReferenceLength != r.points.count { m.lastReferenceLength = r.points.count }
        let shown = job.display
        let size = SIMD2(shown.set.width, shown.set.height)
        if m.renderSize != size { m.renderSize = size }
        if m.isPreview != shown.preview { m.isPreview = shown.preview }
        if m.lastGPUTime != job.gpuTime { m.lastGPUTime = job.gpuTime }
        if m.lastChunks != job.chunks { m.lastChunks = job.chunks }
        if m.isRendering == job.done { m.isRendering = !job.done }
    }

    // MARK: Colorize

    /// Colorize into `target`. A preview level is first coloured at its own resolution into
    /// its colour texture (linear light), which the final pass samples bilinearly wherever the
    /// shown level is unfinished, or everywhere when the preview itself is shown.
    private func encodeColorize(job: Job, target: MTLTexture) -> MTLCommandBuffer? {
        ensurePalette()
        let shown = job.display
        let preview = shown.preview ? shown : job.levels.first { $0.preview && $0.done }
        guard let cb = queue.makeCommandBuffer(), let enc = cb.makeComputeCommandEncoder() else { return nil }
        enc.label = "colorize"
        enc.setComputePipelineState(colorizePipeline)
        enc.setBuffer(paletteBuffer, offset: 0, index: 1)
        enc.setBuffer(levelBuffer, offset: 0, index: 2)
        let tw = colorizePipeline.threadExecutionWidth
        let th = max(1, colorizePipeline.maxTotalThreadsPerThreadgroup / tw / 2)
        func pass(level: Level?, out: MTLTexture, ratio: Double, under: MTLTexture?, linear: Bool) {
            var c = colorParams(ratio: ratio)
            if let level {
                // The running level's admission count lives on the GPU, ahead of the CPU's copy.
                c.admitted = UInt32(level.done ? level.total : 0)
                c.admittedOnGPU = level.started && !level.done ? 1 : 0
                c.deScale = Float(level.ratio)
            }
            c.hasUnder = under == nil ? 0 : 1
            c.linearOut = linear ? 1 : 0
            let src = (level ?? shown).set
            enc.setTexture(src.iter, index: 0)
            enc.setTexture(src.de, index: 1)
            enc.setTexture(out, index: 2)
            enc.setTexture(under ?? out, index: 3)
            enc.setBytes(&c, length: MemoryLayout<ColorParams>.stride, index: 0)
            enc.dispatchThreads(MTLSize(width: out.width, height: out.height, depth: 1),
                                threadsPerThreadgroup: MTLSize(width: tw, height: th, depth: 1))
        }
        var underColor: MTLTexture?
        if let preview, let color = preview.set.color {
            pass(level: preview, out: color, ratio: 1, under: nil, linear: true)
            underColor = color
        }
        // Showing the preview: no main level (nothing "finished"), every pixel samples it.
        pass(level: shown.preview ? nil : shown, out: target, ratio: shown.ratio, under: underColor, linear: false)
        enc.endEncoding()
        return cb
    }

    /// Full-level textures are reused while the size holds; preview sets are cached by size,
    /// since the gesture preview resolution follows the measured cost frame by frame.
    private func textureSet(width: Int, height: Int, preview: Bool) -> TextureSet? {
        if !preview, let s = fullSet, s.width == width, s.height == height { return s }
        if preview, let i = previewSets.firstIndex(where: { $0.width == width && $0.height == height }) {
            let s = previewSets.remove(at: i)
            previewSets.append(s)
            return s
        }
        func make(_ format: MTLPixelFormat) -> MTLTexture? {
            let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: format, width: width, height: height, mipmapped: false)
            d.usage = [.shaderRead, .shaderWrite]
            d.storageMode = .private
            return device.makeTexture(descriptor: d)
        }
        guard let iter = make(.rg32Float), let de = make(.rg16Float) else { return nil }
        let s = TextureSet(iter: iter, de: de, color: preview ? make(.rgba16Float) : nil)
        if preview {
            previewSets.append(s)
            if previewSets.count > 4 { previewSets.removeFirst() }
        } else {
            fullSet = s
        }
        return s
    }

    private func ensurePalette() {
        let palette = model.palette
        guard palette.id != paletteID else { return }
        let table = palette.table()
        paletteBuffer = device.makeBuffer(bytes: table, length: table.count * MemoryLayout<SIMD4<Float>>.stride, options: .storageModeShared)
        paletteSize = table.count
        paletteID = palette.id
    }

    /// Reuses the current reference orbit while it stays near the view and is long enough;
    /// rebasing makes any nearby reference valid, so panning needs no CPU work
    /// (kb: techniques/rebasing). In-flight chunks keep the old buffer alive (Metal retains it).
    private func prepareReference(viewport: Viewport, maxIter: Int, viewRadius: Double) {
        if let r = reference, r.maxIter >= maxIter,
           abs(r.centerRe - viewport.centerRe) < viewRadius,
           abs(r.centerIm - viewport.centerIm) < viewRadius,
           // Reference coordinates are only as good as the scale they were chosen at.
           r.scale <= viewport.scale * 4, r.scale >= viewport.scale / 4 {
            return
        }
        let start = CACurrentMediaTime()
        let r = ReferenceOrbit(centerRe: viewport.centerRe, centerIm: viewport.centerIm,
                               scale: viewport.scale, maxIter: maxIter, bailout: bailout)
        referenceBuffer = device.makeBuffer(bytes: r.points, length: r.points.count * MemoryLayout<SIMD2<Float>>.stride,
                                            options: .storageModeShared)
        reference = r
        // Published with the other stats (throttled): a model write per zoom frame would
        // re-render the HUD every frame.
        referenceTime = CACurrentMediaTime() - start
    }

    private func colorParams(ratio: Double) -> ColorParams {
        let m = model
        let a = m.lightAngle * .pi / 180
        let interior = m.interiorColor
        return ColorParams(
            interior: SIMD4(interior, 1),
            edge: SIMD4(0, 0, 0, 1),
            light: SIMD2(Float(cos(a)), Float(sin(a))),
            density: Float(m.colorDensity),
            offset: Float(m.colorOffset),
            ratio: Float(ratio),
            shading: Float(m.shading),
            outline: Float(m.outline),
            outlineWidth: Float(m.outlineWidth),
            paletteSize: UInt32(paletteSize),
            logScale: m.logColoring ? 1 : 0,
            smooth: m.smoothColoring ? 1 : 0)
    }

    // MARK: Offscreen rendering (headless checks, export)

    public struct OffscreenResult {
        /// BGRA8 pixels, sRGB-encoded, `width * 4` bytes per row.
        public let pixels: [UInt8]
        public let width: Int
        public let height: Int
        /// GPU time of all iterate chunks, seconds.
        public let iterateGPUTime: Double
        /// GPU time of the colorize pass, seconds.
        public let colorizeGPUTime: Double
        /// Wall-clock time of the whole render, seconds.
        public let wallTime: Double
        /// Number of iterate command buffers.
        public let chunks: Int
        /// Iterations executed by the GPU (float-accumulated, approximate).
        public let iterationsDone: UInt64
        /// Raw iteration texels (n, smooth offset; n < 0 interior), row-major, if requested.
        public let iterations: [SIMD2<Float>]?
    }

    /// Renders the current model into an offscreen texture through the same chunked scheduler
    /// as the view (no preview level), at one view point per pixel. Blocks until done.
    public func renderOffscreen(width: Int, height: Int, readback: Bool = false) -> OffscreenResult? {
        let start = CACurrentMediaTime()
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        d.usage = [.shaderWrite, .shaderRead]
        d.storageMode = .shared
        guard let target = device.makeTexture(descriptor: d) else { return nil }

        var attempts = 0
        var finished: Job?
        while finished == nil && attempts < 4 {
            attempts += 1
            drainInFlight()
            job = nil
            startJob(key: makeKey(width: width, height: height, pointsPerPixel: 1), mode: .offscreen)
            guard let job else { return nil }
            while !job.done && !job.failed {
                harvest()
                if job.done || job.failed { break }
                if encodeChunks(job: job, budget: chunkTarget) == 0 {
                    guard let first = inFlight.first else { job.failed = true; break }
                    first.cb.waitUntilCompleted()
                }
            }
            if !job.failed { finished = job }
        }
        guard let job = finished, let cb = encodeColorize(job: job, target: target) else { return nil }
        cb.commit()
        cb.waitUntilCompleted()
        if let error = cb.error {
            NSLog("Almondbread: offscreen render failed: \(error)")
            return nil
        }
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        pixels.withUnsafeMutableBytes { buf in
            target.getBytes(buf.baseAddress!, bytesPerRow: width * 4,
                            from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
        }
        let wall = CACurrentMediaTime() - start
        publishStats(job: job, force: true)
        var iters: [SIMD2<Float>]?
        if readback { iters = readIterations(job.levels.last!.set.iter) }
        return OffscreenResult(pixels: pixels, width: width, height: height,
                               iterateGPUTime: job.gpuTime, colorizeGPUTime: cb.gpuEndTime - cb.gpuStartTime,
                               wallTime: wall, chunks: job.chunks, iterationsDone: UInt64(job.iterations),
                               iterations: iters)
    }

    // MARK: Scripted sessions (headless checks of the interactive pipeline)

    public struct SimulationResult {
        /// The last presented frame, BGRA8 sRGB, `width * 4` bytes per row.
        public let pixels: [UInt8]
        public let width: Int
        public let height: Int
        /// Iteration texels of the level the final frame showed, row-major.
        public let iterations: [SIMD2<Float>]
        public let levelWidth: Int
        public let levelHeight: Int
        /// The view and settings of the final frame.
        public let viewport: Viewport
        public let maxIter: Int
        /// Complex units per texel of the final frame's level.
        public let texelScale: Double
        public let preview: Bool
        public let frames: Int
        /// Seconds from the last input to the refined final frame.
        public let finalAfter: Double
        public let referenceLength: Int
    }

    /// Drives the interactive scheduler headlessly through `renderFrame`, the code the view
    /// runs per display refresh: `input(frame, dt)` runs before each of the first
    /// `inputFrames` refreshes and moves `model.viewport` (counted as a gesture), then the
    /// renderer idles until the refined final frame is presented, as when a key is released.
    /// Paced in real time, since the scheduler's gesture/idle logic is time based.
    public func simulate(width: Int, height: Int, pointsPerPixel: Double, refreshRate: Double = 120,
                         inputFrames: Int, timeout: Double = 30,
                         input: (_ frame: Int, _ dt: Double) -> Void) -> SimulationResult? {
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        d.usage = [.shaderWrite, .shaderRead]
        d.storageMode = .shared
        guard let target = device.makeTexture(descriptor: d) else { return nil }
        let interval = 1 / refreshRate
        var tick = CACurrentMediaTime()
        var frames = 0
        var lastColorize: MTLCommandBuffer?
        func refresh() {
            renderFrame(width: width, height: height, pointsPerPixel: pointsPerPixel) {
                (target, { cb in lastColorize = cb }, nil)
            }
            frames += 1
            // Completion handlers hop to the main queue: keep the run loop turning while waiting.
            tick += interval
            while CACurrentMediaTime() < tick {
                RunLoop.main.run(until: Date(timeIntervalSinceNow: max(0.0005, tick - CACurrentMediaTime())))
            }
        }
        var prev = CACurrentMediaTime()
        for i in 0..<inputFrames {
            let now = CACurrentMediaTime()
            noteInteraction()
            input(i, i == 0 ? interval : now - prev)
            prev = now
            refresh()
        }
        let inputEnd = lastInteraction
        let deadline = CACurrentMediaTime() + timeout
        while !(job.map { $0.finalPresented && !$0.interactive } ?? false) {
            guard CACurrentMediaTime() < deadline else { return nil }
            refresh()
        }
        guard let job else { return nil }
        let finalAfter = lastFinalFrame.time - inputEnd
        lastColorize?.waitUntilCompleted()
        drainInFlight()
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        pixels.withUnsafeMutableBytes { buf in
            target.getBytes(buf.baseAddress!, bytesPerRow: width * 4,
                            from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
        }
        let shown = job.display
        guard let iters = readIterations(shown.set.iter) else { return nil }
        return SimulationResult(pixels: pixels, width: width, height: height, iterations: iters,
                                levelWidth: shown.set.width, levelHeight: shown.set.height,
                                viewport: job.key.viewport, maxIter: job.key.maxIter,
                                texelScale: job.key.viewport.scale * job.key.pointsPerPixel / shown.ratio, preview: shown.preview,
                                frames: frames, finalAfter: finalAfter, referenceLength: Int(shown.params.refLen))
    }

    private func drainInFlight() {
        inFlight.last?.cb.waitUntilCompleted()
        harvest()
    }

    private func readIterations(_ tex: MTLTexture) -> [SIMD2<Float>]? {
        let w = tex.width, h = tex.height
        let rowBytes = w * MemoryLayout<SIMD2<Float>>.stride
        guard let buf = device.makeBuffer(length: rowBytes * h, options: .storageModeShared),
              let cb = queue.makeCommandBuffer(), let blit = cb.makeBlitCommandEncoder() else { return nil }
        blit.copy(from: tex, sourceSlice: 0, sourceLevel: 0, sourceOrigin: MTLOrigin(x: 0, y: 0, z: 0),
                  sourceSize: MTLSize(width: w, height: h, depth: 1), to: buf, destinationOffset: 0,
                  destinationBytesPerRow: rowBytes, destinationBytesPerImage: rowBytes * h)
        blit.endEncoding()
        cb.commit()
        cb.waitUntilCompleted()
        let p = buf.contents().bindMemory(to: SIMD2<Float>.self, capacity: w * h)
        return Array(UnsafeBufferPointer(start: p, count: w * h))
    }
}

/// Launch check for builds (`make selftest`): with `--selftest` or `ALMONDBREAD_SELFTEST=1`
/// the app renders its first view through the real MTKView path, prints "renderer OK ..."
/// once the GPU has completed a fully refined, presented frame, and quits. Exit status 1 if
/// the renderer could not be created, 2 if no frame completed within 30 s.
public enum SelfTest {
    public static let enabled = CommandLine.arguments.contains("--selftest")
        || ProcessInfo.processInfo.environment["ALMONDBREAD_SELFTEST"] == "1"

    @MainActor
    static func start() {
        guard enabled else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 30) {
            finish(ok: false, "renderer FAILED: no completed frame within 30 s", status: 2)
        }
    }

    @MainActor
    static func finish(ok: Bool, _ message: String, status: Int32 = 1) {
        print(message)
        fflush(stdout)
        exit(ok ? 0 : status)
    }
}
