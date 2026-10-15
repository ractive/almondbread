//! Iteration fields from the real GPU kernel (offscreen renders through the scheduler), decoded
//! into samples with their positions in output pixels of the frame and their iteration cost.

use almondbread_core::settings::Supersampling;
use almondbread_core::viewport::Viewport;
use almondbread_render::Renderer;
use almondbread_render::metal::MetalBackend;

use crate::shade::{EXTERIOR, INTERIOR, UNRESOLVED, f16_to_f32};

/// One point sample of the fractal, positioned in output pixels of the current frame (x right,
/// y down, pixel (i, j) covers [i, i+1) x [j, j+1)).
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub x: f64,
    pub y: f64,
    pub n: f32,
    pub s: f32,
    /// Distance estimate in output pixels of the current frame.
    pub de: f32,
    pub ang: f32,
    pub class: u8,
    /// Iterations it cost.
    pub cost: u32,
    /// Frame it was computed in.
    pub birth: u32,
    /// Iteration limit it was computed with.
    pub limit: u32,
}

/// A field of `w x h` texels covering the frame (texel = 1/ratio output pixels, sampled at
/// `jitter` inside each texel), as samples in output pixels, row-major.
pub struct Field {
    pub w: usize,
    pub h: usize,
    pub ratio: f64,
    pub jitter: [f32; 2],
    pub samples: Vec<Sample>,
    pub gpu_ms: f64,
}

impl Field {
    pub fn cost(&self) -> u64 {
        self.samples.iter().map(|s| s.cost as u64).sum()
    }
}

pub struct Gpu {
    pub r: Renderer<MetalBackend>,
    pub wall: f64,
    pub gpu: f64,
}

impl Gpu {
    pub fn new() -> Self {
        let backend = MetalBackend::new().expect("metal");
        let mut r = Renderer::new(backend).expect("renderer");
        r.settings.auto_iterations = false;
        r.settings.max_iterations_cap = 10_000_000;
        r.config.offscreen_extension = 1;
        Self { r, wall: 0.0, gpu: 0.0 }
    }

    /// Renders the frame `vp` (`out_w x out_h` output pixels at one point per pixel) as a
    /// field of ratio `ratio` per axis (ceil sizes as the scheduler's preview levels) with
    /// `ss x ss` sub-texels per texel, sampled at `jitter` inside each (sub-)texel.
    #[allow(clippy::too_many_arguments)]
    pub fn field(
        &mut self,
        vp: &Viewport,
        out_w: usize,
        out_h: usize,
        ratio: f64,
        ss: usize,
        jitter: [f32; 2],
        max_iter: usize,
        birth: u32,
    ) -> Field {
        let w = ((out_w as f64 * ratio).ceil() as usize).max(1);
        let h = ((out_h as f64 * ratio).ceil() as usize).max(1);
        self.r.viewport = Viewport {
            center: vp.center.clone(),
            scale: vp.scale / ratio,
        };
        self.r.settings.manual_iterations = max_iter;
        self.r.settings.supersampling = Supersampling::from_factor(ss).expect("ss");
        self.r.offscreen_jitter = jitter;
        let t0 = std::time::Instant::now();
        let res = MetalBackend::pool(|| self.r.render_offscreen(w, h, true)).expect("offscreen render");
        self.wall += t0.elapsed().as_secs_f64();
        self.gpu += res.iterate_gpu_time;
        let it = res.iterations.expect("iterations");
        let de = res.de.expect("de");
        let (tw, th) = (w * ss, h * ss);
        assert_eq!(it.len(), tw * th);
        // Texel (i, j) of the level: its sample sits at (i + jitter) texels from the level's
        // top-left; the level's centre (tw/2, th/2) is the view centre (output (W/2, H/2)).
        let px_per_texel = 1.0 / (ratio * ss as f64);
        let (cx, cy) = (tw as f64 / 2.0, th as f64 / 2.0);
        let (ox, oy) = (out_w as f64 / 2.0, out_h as f64 / 2.0);
        let mut samples = Vec::with_capacity(tw * th);
        for j in 0..th {
            for i in 0..tw {
                let t = it[j * tw + i];
                let d = de[j * tw + i];
                let x = ox + (i as f64 + jitter[0] as f64 - cx) * px_per_texel;
                let y = oy + (j as f64 + jitter[1] as f64 - cy) * px_per_texel;
                let (class, cost) = if t[0] >= 0.0 {
                    (EXTERIOR, t[0] as u32)
                } else if t[0] == -1.0 {
                    (INTERIOR, if t[1] > 0.0 { t[1] as u32 } else { max_iter as u32 })
                } else if t[0] == -3.0 {
                    (UNRESOLVED, max_iter as u32)
                } else {
                    panic!("unexpected texel {t:?}");
                };
                samples.push(Sample {
                    x,
                    y,
                    n: t[0].max(0.0),
                    s: t[1],
                    de: f16_to_f32(d[0]) * px_per_texel as f32,
                    ang: f16_to_f32(d[1]),
                    class,
                    cost,
                    birth,
                    limit: max_iter as u32,
                });
            }
        }
        Field {
            w: tw,
            h: th,
            ratio: ratio * ss as f64,
            jitter,
            samples,
            gpu_ms: res.iterate_gpu_time * 1000.0,
        }
    }
}
