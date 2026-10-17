//! Fog over unresolved pixels (kb: techniques/unresolved-fog).
//!
//! A pixel past the nominal limit that neither escaped nor was proven interior is drawn as
//! fog: light where it meets resolved pixels (their colours, blurred), darker and softer
//! further in, almost black deep inside. How far "in" is comes from a pyramid of the
//! resolved share and colour of the iteration level (the `fogSeed` / `fogDown` kernels):
//! `fogResolve` walks it from fine to coarse until it finds resolved texels, which gives the
//! distance in octaves and the colours around at that scale, into a fog texture the colour
//! pass samples at its unresolved texels. Built only for frames with unresolved pixels, in the
//! colour pass's own command buffer, so a view without any pays nothing; about 1 ms at
//! 3456 x 2234 on M1 Pro with most of the view unresolved.

use bytemuck::bytes_of;

use crate::backend::{ComputeEncoder, GpuBackend, GpuError, Storage, TextureFormat};
use crate::gpu_types::{ColorParams, FogParams};

/// Pyramid levels kept: level L is 2^(L+1) iteration texels wide, so 12 levels reach past
/// 4000 texels (a fully unresolved Retina view still finds its border).
const MAX_LEVELS: u32 = 12;

/// The level whose resolution the fog colour is resolved at (`FOG_RES` in the kernels).
const FOG_RES: u32 = 1;

/// Size of pyramid level `l` for an iteration level of `src` texels (as `fogLevelSize`).
fn level_size(src: (usize, usize), l: u32) -> (usize, usize) {
    let (mut w, mut h) = (src.0.div_ceil(2), src.1.div_ceil(2));
    for _ in 0..l {
        w = w.div_ceil(2);
        h = h.div_ceil(2);
    }
    (w.max(1), h.max(1))
}

/// Levels for `src`: down to a level of at most 2 x 2 texels, at most [`MAX_LEVELS`].
pub fn levels(src: (usize, usize)) -> u32 {
    let mut n = 1;
    while n < MAX_LEVELS {
        let (w, h) = level_size(src, n - 1);
        if w.max(h) <= 2 {
            break;
        }
        n += 1;
    }
    n
}

/// Atlas sizes: A holds the even levels (0 at the origin, 2, 4, ... stacked to its right), B
/// the odd ones (1 at the origin, 3, 5, ... to its right).
fn atlas_sizes(src: (usize, usize), n: u32) -> [(usize, usize); 2] {
    let mut out = [(1, 1); 2];
    for (parity, size) in out.iter_mut().enumerate() {
        let base = level_size(src, parity as u32);
        let (mut right_w, mut right_h) = (0, 0);
        let mut l = parity as u32 + 2;
        while l < n {
            let (w, h) = level_size(src, l);
            right_w = right_w.max(w);
            right_h += h;
            l += 2;
        }
        *size = (base.0 + right_w, base.1.max(right_h));
    }
    out
}

/// The fog pyramid's kernels and textures (allocated on first use, grown when a level is
/// larger).
pub struct FogPyramid<B: GpuBackend> {
    seed: B::Pipeline,
    down: B::Pipeline,
    resolve: B::Pipeline,
    /// The pyramid's two atlases and the fog colour at level-0 resolution.
    textures: Option<[B::Texture; 3]>,
}

/// The iteration level a pyramid is built from, with the bindings the colour pass uses.
pub struct FogSource<'a, B: GpuBackend> {
    pub iter: &'a B::Texture,
    pub de: &'a B::Texture,
    pub aux: &'a B::Texture,
    pub palette: &'a B::Buffer,
    pub level_counters: &'a B::Buffer,
    pub rank: &'a B::Buffer,
    pub size: (usize, usize),
}

impl<B: GpuBackend> FogPyramid<B> {
    pub fn new(backend: &B) -> Result<Self, GpuError> {
        Ok(Self {
            seed: backend.pipeline("fogSeed", &[])?,
            down: backend.pipeline("fogDown", &[])?,
            resolve: backend.pipeline("fogResolve", &[])?,
            textures: None,
        })
    }

    /// Encodes the pyramid of `src` and the fog colour it gives into `enc` and returns the fog
    /// texture (level `FOG_RES`: a texel per 4 x 4 iteration texels) the colour pass samples; sets
    /// the fog fields of `c` (the colour parameters of that pass; admission, sizes and colours
    /// as for it). `points_per_texel`: display points per iteration texel.
    pub fn encode(
        &mut self,
        backend: &B,
        enc: &mut B::Encoder,
        src: &FogSource<'_, B>,
        c: &mut ColorParams,
        points_per_texel: f64,
    ) -> Result<B::Texture, GpuError> {
        let n = levels(src.size);
        let [a, b] = atlas_sizes(src.size, n);
        let (w0, h0) = level_size(src.size, 0);
        let sizes = [a, b, level_size(src.size, FOG_RES)];
        let fits = self.textures.as_ref().is_some_and(|t| {
            (0..3).all(|k| {
                let (w, h) = backend.texture_size(&t[k]);
                w >= sizes[k].0 && h >= sizes[k].1
            })
        });
        if !fits {
            let make =
                |(w, h): (usize, usize)| backend.create_texture(w, h, TextureFormat::Rgba16Float, Storage::Private);
            self.textures = Some([make(sizes[0])?, make(sizes[1])?, make(sizes[2])?]);
        }
        let [atlas_a, atlas_b, fog] = self.textures.clone().expect("fog textures");
        let atlas = [atlas_a, atlas_b];
        let mut seed = *c;
        seed.src_size = [src.size.0 as u32, src.size.1 as u32];
        seed.fog_levels = 0;
        enc.set_pipeline(&self.seed);
        enc.set_texture(0, src.iter);
        enc.set_texture(1, src.de);
        enc.set_texture(2, src.aux);
        enc.set_texture(3, &atlas[0]);
        enc.set_bytes(0, bytes_of(&seed));
        enc.set_buffer(1, src.palette, 0);
        enc.set_buffer(2, src.level_counters, 0);
        enc.set_buffer(3, src.rank, 0);
        let group = |p: &B::Pipeline| {
            let (w, max) = backend.pipeline_limits(p);
            [w, (max / w / 2).max(1), 1]
        };
        enc.dispatch_threads([w0, h0, 1], group(&self.seed));
        enc.set_pipeline(&self.down);
        for l in 1..n {
            let p = FogParams {
                src: seed.src_size,
                level: l,
                pad: 0,
            };
            let (from, to) = if l % 2 == 1 {
                (&atlas[0], &atlas[1])
            } else {
                (&atlas[1], &atlas[0])
            };
            enc.set_texture(0, from);
            enc.set_texture(1, to);
            enc.set_bytes(0, bytes_of(&p));
            let (w, h) = level_size(src.size, l);
            enc.dispatch_threads([w, h, 1], group(&self.down));
        }
        c.fog_levels = n;
        c.fog_points = points_per_texel as f32;
        c.src_size = seed.src_size;
        enc.set_pipeline(&self.resolve);
        enc.set_texture(0, &atlas[0]);
        enc.set_texture(1, &atlas[1]);
        enc.set_texture(2, &fog);
        enc.set_bytes(0, bytes_of(&*c));
        let (wr, hr) = sizes[2];
        enc.dispatch_threads([wr, hr, 1], group(&self.resolve));
        Ok(fog)
    }
}

/// [`FogPyramid::encode`] with the pyramid made on first use; None (the plain deep fog) if its
/// kernels or textures fail.
pub fn build<B: GpuBackend>(
    fog: &mut Option<FogPyramid<B>>,
    backend: &B,
    enc: &mut B::Encoder,
    src: &FogSource<'_, B>,
    c: &mut ColorParams,
    points_per_texel: f64,
) -> Option<B::Texture> {
    if fog.is_none() {
        match FogPyramid::new(backend) {
            Ok(f) => *fog = Some(f),
            Err(e) => {
                eprintln!("almondbread: fog pipelines: {e}");
                return None;
            }
        }
    }
    match fog.as_mut()?.encode(backend, enc, src, c, points_per_texel) {
        Ok(a) => Some(a),
        Err(e) => {
            eprintln!("almondbread: fog pyramid: {e}");
            c.fog_levels = 0;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_and_atlas_cover_the_pyramid() {
        for src in [(1, 1), (3, 2), (1600, 1000), (3456, 2234), (6912, 4468), (5, 4000)] {
            let n = levels(src);
            assert!((1..=MAX_LEVELS).contains(&n));
            let last = level_size(src, n - 1);
            assert!(
                last.0.max(last.1) <= 2 || n == MAX_LEVELS,
                "{src:?}: {n} levels end at {last:?}"
            );
            let sizes = atlas_sizes(src, n);
            // Every level's rectangle lies inside its atlas.
            let mut y = [0usize; 2];
            for l in 0..n {
                let parity = (l % 2) as usize;
                let (w, h) = level_size(src, l);
                let (x0, y0) = if l < 2 {
                    (0, 0)
                } else {
                    let o = (level_size(src, parity as u32).0, y[parity]);
                    y[parity] += h;
                    o
                };
                assert!(
                    x0 + w <= sizes[parity].0 && y0 + h <= sizes[parity].1,
                    "{src:?} level {l}"
                );
            }
        }
    }
}
