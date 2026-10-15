//! R4 probe: Apple MetalFX upscaling (temporal and spatial) driven from Rust through
//! objc2-metal-fx 0.3.2 (kb: platform/metalfx, research/motion-sample-reuse).
//!
//! The [`Upscaler`] takes CPU images (linear RGB, row-major, y down), uploads them through
//! a blit in a command buffer of their own, encodes ONLY the MetalFX scaler into a second
//! command buffer (whose GPU time is reported), and reads the output back through a third.
//! Correctness over speed on the upload/readback path; the timed command buffer holds
//! nothing but the scaler.
//!
//! # Conventions (verified by `metalfx-probe test`, see main.rs)
//!
//! * Pixel coordinates: input pixel `(i, j)` covers `[i, i+1) x [j, j+1)`, centre at
//!   `(i + 0.5, j + 0.5)`, y down (row 0 is the top row, as in Metal textures).
//! * `jitter`: where this frame's sample lies inside its input pixel, in `[0, 1)` per axis
//!   (`[0.5, 0.5]` = pixel centre, i.e. no jitter). MetalFX's `jitterOffsetX/Y` is set to
//!   `JITTER_SIGN * (jitter - 0.5)` in input pixels (see [`JITTER_SIGN`]).
//! * `motion`: per input pixel, `p_prev - p_cur` in INPUT pixels: the offset from where
//!   this pixel's content is now to where the same content was in the previous frame's
//!   input image. Content moving right/down on screen has negative motion. This is
//!   MetalFX's own convention with `motionVectorScaleX/Y = 1.0` (header doc: "an object
//!   that moves down and to the right ... by 10 pixels would be (-10,-10)"), so the
//!   values are uploaded unchanged as rg16Float.
//! * depth: a constant depth32Float texture (value [`DEFAULT_DEPTH`], not reversed); our
//!   content has no depth. Changeable through [`Upscaler::set_depth`].

use std::ffi::c_void;
use std::ptr::NonNull;
use std::time::Instant;

use half::f16;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder, MTLCommandQueue,
    MTLCreateSystemDefaultDevice, MTLDevice, MTLOrigin, MTLPixelFormat, MTLResourceOptions, MTLSize, MTLStorageMode,
    MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};
use objc2_metal_fx::{
    MTLFXSpatialScaler, MTLFXSpatialScalerBase, MTLFXSpatialScalerColorProcessingMode, MTLFXSpatialScalerDescriptor,
    MTLFXTemporalScaler, MTLFXTemporalScalerBase, MTLFXTemporalScalerDescriptor,
};

// MTLCreateSystemDefaultDevice needs CoreGraphics linked (objc2-metal-fx links MetalFX itself).
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {}

type Tex = Retained<ProtocolObject<dyn MTLTexture>>;
type Buf = Retained<ProtocolObject<dyn MTLBuffer>>;
type Device = Retained<ProtocolObject<dyn MTLDevice>>;

/// MetalFX `jitterOffset = JITTER_SIGN * (jitter - 0.5)`, per axis, in input pixels.
/// Determined empirically by the `test` jitter-sign sweep (static scene, 4 sign variants).
pub const JITTER_SIGN: [f32; 2] = [-1.0, -1.0];

/// Constant depth written to the depth texture (depthReversed = false).
pub const DEFAULT_DEPTH: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Temporal,
    Spatial,
}

enum Scaler {
    Temporal(Retained<ProtocolObject<dyn MTLFXTemporalScaler>>),
    Spatial(Retained<ProtocolObject<dyn MTLFXSpatialScaler>>),
}

/// What the device and the created scaler report.
#[derive(Clone, Debug, Default)]
pub struct Info {
    pub device: String,
    /// Wall time of `new*ScalerWithDevice:` (synchronous initialization requested).
    pub create_ms: f64,
    /// `MTLDevice.currentAllocatedSize` delta across scaler creation (MetalFX internals).
    pub scaler_alloc_bytes: i64,
    /// Same delta across creation of our own textures and staging buffers.
    pub io_alloc_bytes: i64,
    pub color_usage: u64,
    pub depth_usage: u64,
    pub motion_usage: u64,
    pub output_usage: u64,
    /// Temporal only: the scaler's inputContentMinScale / MaxScale.
    pub input_content_min_scale: f32,
    pub input_content_max_scale: f32,
}

/// Device capabilities relevant to MetalFX.
#[derive(Clone, Debug)]
pub struct DeviceSupport {
    pub device: String,
    pub temporal: bool,
    pub spatial: bool,
    pub temporal_min_scale: f32,
    pub temporal_max_scale: f32,
    pub temporal_metal4: bool,
    pub spatial_metal4: bool,
}

pub fn device_support() -> Result<DeviceSupport, String> {
    autoreleasepool(|_| {
        let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
        // SAFETY: class methods taking a live device.
        unsafe {
            Ok(DeviceSupport {
                device: device.name().to_string(),
                temporal: MTLFXTemporalScalerDescriptor::supportsDevice(&device),
                spatial: MTLFXSpatialScalerDescriptor::supportsDevice(&device),
                temporal_min_scale: MTLFXTemporalScalerDescriptor::supportedInputContentMinScaleForDevice(&device),
                temporal_max_scale: MTLFXTemporalScalerDescriptor::supportedInputContentMaxScaleForDevice(&device),
                temporal_metal4: MTLFXTemporalScalerDescriptor::supportsMetal4FX(&device),
                spatial_metal4: MTLFXSpatialScalerDescriptor::supportsMetal4FX(&device),
            })
        }
    })
}

pub struct Upscaler {
    pub kind: Kind,
    pub in_w: usize,
    pub in_h: usize,
    pub out_w: usize,
    pub out_h: usize,
    device: Device,
    queue: Retained<ProtocolObject<dyn objc2_metal::MTLCommandQueue>>,
    scaler: Scaler,
    color: Tex,
    motion: Option<Tex>,
    depth: Option<Tex>,
    exposure: Option<Tex>,
    output: Tex,
    readback: Buf,
    /// MetalFX jitterOffset = jitter_sign * (jitter - 0.5). Public so the test can sweep it.
    pub jitter_sign: [f32; 2],
    depth_value: f32,
    depth_reversed: bool,
    depth_dirty: bool,
    pub info: Info,
}

fn texture(device: &Device, format: MTLPixelFormat, w: usize, h: usize, usage: MTLTextureUsage) -> Result<Tex, String> {
    // SAFETY: plain descriptor construction with valid sizes.
    let d =
        unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(format, w, h, false) };
    d.setUsage(usage);
    d.setStorageMode(MTLStorageMode::Private);
    device
        .newTextureWithDescriptor(&d)
        .ok_or_else(|| format!("texture {w}x{h} {format:?}"))
}

fn region(w: usize, h: usize) -> (MTLOrigin, MTLSize) {
    (
        MTLOrigin { x: 0, y: 0, z: 0 },
        MTLSize {
            width: w,
            height: h,
            depth: 1,
        },
    )
}

impl Upscaler {
    pub fn new(kind: Kind, in_w: usize, in_h: usize, out_w: usize, out_h: usize) -> Result<Self, String> {
        Self::with_options(kind, in_w, in_h, out_w, out_h, true)
    }

    /// `sync_init`: requiresSynchronousInitialization (temporal only). With false, MetalFX
    /// returns quickly and compiles its fast pipelines in the background (slower first frames).
    pub fn with_options(
        kind: Kind,
        in_w: usize,
        in_h: usize,
        out_w: usize,
        out_h: usize,
        sync_init: bool,
    ) -> Result<Self, String> {
        autoreleasepool(|_| {
            let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
            let queue = device.newCommandQueue().ok_or("no command queue")?;
            let mut info = Info {
                device: device.name().to_string(),
                ..Default::default()
            };
            let rgba16 = MTLPixelFormat::RGBA16Float;
            let alloc0 = device.currentAllocatedSize() as i64;
            let t0 = Instant::now();
            // SAFETY: descriptor setters with valid values; creation takes a live device.
            let scaler = unsafe {
                match kind {
                    Kind::Temporal => {
                        if !MTLFXTemporalScalerDescriptor::supportsDevice(&device) {
                            return Err("temporal scaler not supported on this device".into());
                        }
                        let d = MTLFXTemporalScalerDescriptor::new();
                        d.setColorTextureFormat(rgba16);
                        d.setDepthTextureFormat(MTLPixelFormat::Depth32Float);
                        d.setMotionTextureFormat(MTLPixelFormat::RG16Float);
                        d.setOutputTextureFormat(rgba16);
                        d.setInputWidth(in_w);
                        d.setInputHeight(in_h);
                        d.setOutputWidth(out_w);
                        d.setOutputHeight(out_h);
                        d.setAutoExposureEnabled(false);
                        d.setRequiresSynchronousInitialization(sync_init);
                        let s = d.newTemporalScalerWithDevice(&device).ok_or_else(|| {
                            format!("newTemporalScalerWithDevice returned nil for {in_w}x{in_h} -> {out_w}x{out_h}")
                        })?;
                        Scaler::Temporal(s)
                    }
                    Kind::Spatial => {
                        if !MTLFXSpatialScalerDescriptor::supportsDevice(&device) {
                            return Err("spatial scaler not supported on this device".into());
                        }
                        let d = MTLFXSpatialScalerDescriptor::new();
                        d.setColorTextureFormat(rgba16);
                        d.setOutputTextureFormat(rgba16);
                        d.setInputWidth(in_w);
                        d.setInputHeight(in_h);
                        d.setOutputWidth(out_w);
                        d.setOutputHeight(out_h);
                        d.setColorProcessingMode(MTLFXSpatialScalerColorProcessingMode::Linear);
                        let s = d.newSpatialScalerWithDevice(&device).ok_or_else(|| {
                            format!("newSpatialScalerWithDevice returned nil for {in_w}x{in_h} -> {out_w}x{out_h}")
                        })?;
                        Scaler::Spatial(s)
                    }
                }
            };
            info.create_ms = t0.elapsed().as_secs_f64() * 1e3;
            let alloc1 = device.currentAllocatedSize() as i64;
            info.scaler_alloc_bytes = alloc1 - alloc0;

            // SAFETY: property reads on a live scaler.
            let (color_usage, motion_usage, depth_usage, output_usage) = unsafe {
                match &scaler {
                    Scaler::Temporal(s) => {
                        info.input_content_min_scale = s.inputContentMinScale();
                        info.input_content_max_scale = s.inputContentMaxScale();
                        (
                            s.colorTextureUsage(),
                            s.motionTextureUsage(),
                            s.depthTextureUsage(),
                            s.outputTextureUsage(),
                        )
                    }
                    Scaler::Spatial(s) => (
                        s.colorTextureUsage(),
                        MTLTextureUsage::empty(),
                        MTLTextureUsage::empty(),
                        s.outputTextureUsage(),
                    ),
                }
            };
            info.color_usage = color_usage.0 as u64;
            info.motion_usage = motion_usage.0 as u64;
            info.depth_usage = depth_usage.0 as u64;
            info.output_usage = output_usage.0 as u64;

            let color = texture(&device, rgba16, in_w, in_h, color_usage | MTLTextureUsage::ShaderRead)?;
            let output = texture(&device, rgba16, out_w, out_h, output_usage)?;
            let (motion, depth, exposure) = if kind == Kind::Temporal {
                (
                    Some(texture(
                        &device,
                        MTLPixelFormat::RG16Float,
                        in_w,
                        in_h,
                        motion_usage | MTLTextureUsage::ShaderRead,
                    )?),
                    Some(texture(
                        &device,
                        MTLPixelFormat::Depth32Float,
                        in_w,
                        in_h,
                        depth_usage | MTLTextureUsage::ShaderRead,
                    )?),
                    Some(texture(
                        &device,
                        MTLPixelFormat::R16Float,
                        1,
                        1,
                        MTLTextureUsage::ShaderRead,
                    )?),
                )
            } else {
                (None, None, None)
            };
            let readback = device
                .newBufferWithLength_options(out_w * out_h * 8, MTLResourceOptions::StorageModeShared)
                .ok_or("readback buffer")?;
            info.io_alloc_bytes = device.currentAllocatedSize() as i64 - alloc1;

            let up = Self {
                kind,
                in_w,
                in_h,
                out_w,
                out_h,
                device,
                queue,
                scaler,
                color,
                motion,
                depth,
                exposure,
                output,
                readback,
                jitter_sign: JITTER_SIGN,
                depth_value: DEFAULT_DEPTH,
                depth_reversed: false,
                depth_dirty: true,
                info,
            };
            up.configure();
            if let Some(e) = &up.exposure {
                // Exposure 1.0 (MetalFX multiplies input colour by R of texel (0,0)).
                let one = f16::from_f32(1.0).to_bits().to_ne_bytes();
                up.upload_now(e, &one, 2, 1, 1)?;
            }
            Ok(up)
        })
    }

    /// Constant depth value and `depthReversed` flag for the temporal scaler.
    pub fn set_depth(&mut self, value: f32, reversed: bool) {
        self.depth_value = value;
        self.depth_reversed = reversed;
        self.depth_dirty = true;
        self.configure();
    }

    fn configure(&self) {
        // SAFETY: property setters with live textures of the descriptor's formats and sizes.
        unsafe {
            match &self.scaler {
                Scaler::Temporal(s) => {
                    s.setInputContentWidth(self.in_w);
                    s.setInputContentHeight(self.in_h);
                    s.setColorTexture(Some(&self.color));
                    s.setDepthTexture(self.depth.as_deref());
                    s.setMotionTexture(self.motion.as_deref());
                    s.setOutputTexture(Some(&self.output));
                    s.setExposureTexture(self.exposure.as_deref());
                    s.setPreExposure(1.0);
                    s.setMotionVectorScaleX(1.0);
                    s.setMotionVectorScaleY(1.0);
                    s.setDepthReversed(self.depth_reversed);
                }
                Scaler::Spatial(s) => {
                    s.setInputContentWidth(self.in_w);
                    s.setInputContentHeight(self.in_h);
                    s.setColorTexture(Some(&self.color));
                    s.setOutputTexture(Some(&self.output));
                }
            }
        }
    }

    fn encode_upload(
        &self,
        cb: &ProtocolObject<dyn MTLCommandBuffer>,
        tex: &Tex,
        bytes: &[u8],
        bpp: usize,
        w: usize,
        h: usize,
    ) -> Result<(), String> {
        assert_eq!(bytes.len(), w * h * bpp);
        // SAFETY: `bytes` is valid for its length; Metal copies it into a new buffer.
        let staging = unsafe {
            self.device.newBufferWithBytes_length_options(
                NonNull::new(bytes.as_ptr() as *mut c_void).expect("non-null"),
                bytes.len(),
                MTLResourceOptions::StorageModeShared,
            )
        }
        .ok_or("staging buffer")?;
        let blit = cb.blitCommandEncoder().ok_or("blit encoder")?;
        let (origin, size) = region(w, h);
        // SAFETY: source range and destination region are in bounds.
        unsafe {
            blit.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                &staging, 0, w * bpp, w * h * bpp, size, tex, 0, 0, origin,
            );
        }
        blit.endEncoding();
        Ok(())
    }

    fn upload_now(&self, tex: &Tex, bytes: &[u8], bpp: usize, w: usize, h: usize) -> Result<(), String> {
        let cb = self.queue.commandBuffer().ok_or("command buffer")?;
        self.encode_upload(&cb, tex, bytes, bpp, w, h)?;
        cb.commit();
        cb.waitUntilCompleted();
        check(&cb)
    }

    fn set_frame_params(&self, jitter: [f32; 2], reset: bool) {
        if let Scaler::Temporal(s) = &self.scaler {
            // SAFETY: plain float/bool property setters.
            unsafe {
                s.setJitterOffsetX(self.jitter_sign[0] * (jitter[0] - 0.5));
                s.setJitterOffsetY(self.jitter_sign[1] * (jitter[1] - 0.5));
                s.setReset(reset);
            }
        }
    }

    fn encode_scaler(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>) {
        // SAFETY: all textures are set and match the descriptor.
        unsafe {
            match &self.scaler {
                Scaler::Temporal(s) => s.encodeToCommandBuffer(cb),
                Scaler::Spatial(s) => s.encodeToCommandBuffer(cb),
            }
        }
    }

    /// See the crate docs for the conventions. Returns linear RGB `out_w * out_h` and the GPU
    /// time (ms) of a command buffer that contains only the scaler encode.
    pub fn upscale(
        &mut self,
        color: &[[f32; 3]],
        motion: Option<&[[f32; 2]]>,
        jitter: [f32; 2],
        reset: bool,
    ) -> Result<(Vec<[f32; 3]>, f64), String> {
        let n_in = self.in_w * self.in_h;
        if color.len() != n_in {
            return Err(format!("color has {} pixels, want {n_in}", color.len()));
        }
        if let Some(m) = motion
            && m.len() != n_in
        {
            return Err(format!("motion has {} pixels, want {n_in}", m.len()));
        }
        autoreleasepool(|_| {
            let up = self.queue.commandBuffer().ok_or("command buffer")?;
            let mut rgba = Vec::with_capacity(n_in * 8);
            for c in color {
                for v in [c[0], c[1], c[2], 1.0] {
                    rgba.extend_from_slice(&f16::from_f32(v).to_bits().to_ne_bytes());
                }
            }
            self.encode_upload(&up, &self.color, &rgba, 8, self.in_w, self.in_h)?;
            if let Some(mt) = &self.motion {
                let mut mv = Vec::with_capacity(n_in * 4);
                for i in 0..n_in {
                    let m = motion.map(|m| m[i]).unwrap_or([0.0, 0.0]);
                    for v in m {
                        mv.extend_from_slice(&f16::from_f32(v).to_bits().to_ne_bytes());
                    }
                }
                self.encode_upload(&up, mt, &mv, 4, self.in_w, self.in_h)?;
            }
            if self.depth_dirty
                && let Some(dt) = &self.depth
            {
                let bytes: Vec<u8> = (0..n_in).flat_map(|_| self.depth_value.to_ne_bytes()).collect();
                self.encode_upload(&up, dt, &bytes, 4, self.in_w, self.in_h)?;
            }
            up.commit();
            self.depth_dirty = false;

            self.set_frame_params(jitter, reset);
            let cb = self.queue.commandBuffer().ok_or("command buffer")?;
            self.encode_scaler(&cb);
            cb.commit();

            let rd = self.queue.commandBuffer().ok_or("command buffer")?;
            let blit = rd.blitCommandEncoder().ok_or("blit encoder")?;
            let (origin, size) = region(self.out_w, self.out_h);
            // SAFETY: source region and destination range are in bounds.
            unsafe {
                blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                    &self.output, 0, 0, origin, size, &self.readback, 0, self.out_w * 8, self.out_w * self.out_h * 8,
                );
            }
            blit.endEncoding();
            rd.commit();
            rd.waitUntilCompleted();
            check(&up)?;
            check(&cb)?;
            check(&rd)?;
            let gpu_ms = (cb.GPUEndTime() - cb.GPUStartTime()) * 1e3;
            let n_out = self.out_w * self.out_h;
            // SAFETY: shared buffer of n_out * 8 bytes, written by the completed readback.
            let px = unsafe { std::slice::from_raw_parts(self.readback.contents().as_ptr() as *const u16, n_out * 4) };
            let out = (0..n_out)
                .map(|i| {
                    [
                        f16::from_bits(px[4 * i]).to_f32(),
                        f16::from_bits(px[4 * i + 1]).to_f32(),
                        f16::from_bits(px[4 * i + 2]).to_f32(),
                    ]
                })
                .collect();
            Ok((out, gpu_ms))
        })
    }

    /// Benchmark helper: encodes the scaler on the textures already uploaded (no upload, no
    /// readback), waits, returns the command buffer's GPU time in ms.
    /// Benchmark helper: `jitters.len()` scaler command buffers committed back to back (GPU
    /// kept busy, so clocks stay up), then waits; returns each command buffer's GPU ms.
    pub fn bench_batch(&mut self, jitters: &[[f32; 2]]) -> Result<Vec<f64>, String> {
        autoreleasepool(|_| {
            let mut cbs = Vec::with_capacity(jitters.len());
            for &j in jitters {
                self.set_frame_params(j, false);
                let cb = self.queue.commandBuffer().ok_or("command buffer")?;
                self.encode_scaler(&cb);
                cb.commit();
                cbs.push(cb);
            }
            let mut out = Vec::with_capacity(cbs.len());
            for cb in &cbs {
                cb.waitUntilCompleted();
                check(cb)?;
                out.push((cb.GPUEndTime() - cb.GPUStartTime()) * 1e3);
            }
            Ok(out)
        })
    }

    pub fn bench_frame(&mut self, jitter: [f32; 2], reset: bool) -> Result<f64, String> {
        autoreleasepool(|_| {
            self.set_frame_params(jitter, reset);
            let cb = self.queue.commandBuffer().ok_or("command buffer")?;
            self.encode_scaler(&cb);
            cb.commit();
            cb.waitUntilCompleted();
            check(&cb)?;
            Ok((cb.GPUEndTime() - cb.GPUStartTime()) * 1e3)
        })
    }
}

fn check(cb: &ProtocolObject<dyn MTLCommandBuffer>) -> Result<(), String> {
    if cb.status() == MTLCommandBufferStatus::Error {
        return Err(cb
            .error()
            .map(|e| e.localizedDescription().to_string())
            .unwrap_or_else(|| "command buffer error".into()));
    }
    Ok(())
}

/// Per-pixel motion (MetalFX convention, input pixels, `p_prev - p_cur`) for a pure zoom/pan.
///
/// Pixel centres are `p = (i + 0.5, j + 0.5)`, the view centre is `c = (in_w/2, in_h/2)` in
/// both frames. The previous frame's input pixel position of the content now at `p` is
/// `p_prev = c + (p - c) * step + shift_px`, where
/// * `step` = current pixel size / previous pixel size (complex-plane units per pixel,
///   `h_cur / h_prev`; < 1 when zooming in),
/// * `shift_px` = (current view centre - previous view centre) in PREVIOUS-frame input
///   pixels, y down (`(centre_cur - centre_prev) / h_prev`, with the imaginary axis flipped
///   if it points up on screen).
///
/// Returned motion is `p_prev - p = (p - c) * (step - 1) + shift_px`.
pub fn affine_motion(in_w: usize, in_h: usize, step: f64, shift_px: [f64; 2]) -> Vec<[f32; 2]> {
    let (cx, cy) = (in_w as f64 * 0.5, in_h as f64 * 0.5);
    let mut out = Vec::with_capacity(in_w * in_h);
    for j in 0..in_h {
        let dy = j as f64 + 0.5 - cy;
        for i in 0..in_w {
            let dx = i as f64 + 0.5 - cx;
            out.push([
                (dx * (step - 1.0) + shift_px[0]) as f32,
                (dy * (step - 1.0) + shift_px[1]) as f32,
            ]);
        }
    }
    out
}

/// Halton radical inverse (base 2 and 3 give the renderer's jitter sequence).
pub fn halton(mut i: u32, base: u32) -> f32 {
    let mut f = 1.0f64;
    let mut r = 0.0f64;
    while i > 0 {
        f /= base as f64;
        r += f * (i % base) as f64;
        i /= base;
    }
    r as f32
}
