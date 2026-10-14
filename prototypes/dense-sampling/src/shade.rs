//! CPU port of the kernel's `shade` (crates/almondbread-render/src/shaders/mandelbrot.metal,
//! FL look: palette anti-aliasing, DE outline, lighting) in f32, for the default look, and the
//! colour metrics (OKLab ΔE).

use almondbread_core::palette::{Look, Palette, mip_chain, mip_levels, oklab};

use crate::sample::{Class, Sample};

const PALETTE_SIZE: usize = 1024;

pub type Rgb = [f32; 3];

pub struct Shader {
    pal: Vec<[f32; 4]>,
    levels: u32,
    density: f32,
    offset: f32,
    outline: f32,
    outline_width: f32,
    edge: Rgb,
    interior: Rgb,
    shading: f32,
    light: [f32; 3],
    half: [f32; 3],
    specular: f32,
    shininess: f32,
}

fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

fn mix3(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Shader {
    pub fn default_look() -> Self {
        Self::look(&Look::default_look())
    }

    pub fn look(look: &Look) -> Self {
        let pal = mip_chain(&Palette::named(look.palette).table(PALETTE_SIZE));
        let a = (look.light_angle as f32).to_radians();
        let light = norm3([a.cos(), a.sin(), look.light_height as f32]);
        let half = norm3([light[0], light[1], light[2] + 1.0]);
        let lin = |hex: u32| almondbread_core::palette::linear(hex);
        Self {
            pal,
            levels: mip_levels(PALETTE_SIZE) as u32,
            density: look.density as f32,
            offset: 0.0,
            outline: look.outline as f32,
            outline_width: look.outline_width as f32,
            edge: lin(look.edge_color),
            interior: lin(look.interior),
            shading: look.shading as f32,
            light,
            half,
            specular: look.specular as f32,
            shininess: look.shininess as f32,
        }
    }

    fn palette_at(&self, t: f32, lod: f32) -> Rgb {
        let t = t - t.floor();
        let l0 = lod as u32;
        let l1 = (l0 + 1).min(self.levels - 1);
        let size0 = PALETTE_SIZE as u32;
        let mut col = [[0.0f32; 3]; 2];
        for (k, l) in [l0, l1].into_iter().enumerate() {
            let size = size0 >> l;
            let base = 2 * size0 - ((2 * size0) >> l);
            let x = t * size as f32 - 0.5;
            let fl = x.floor();
            let i0 = ((fl as i64 + size as i64) as u32) % size;
            let i1 = (i0 + 1) % size;
            let a = self.pal[(base + i0) as usize];
            let b = self.pal[(base + i1) as usize];
            col[k] = mix3([a[0], a[1], a[2]], [b[0], b[1], b[2]], x - fl);
        }
        mix3(col[0], col[1], lod - l0 as f32)
    }

    /// Final linear colour of one sample (pixelsPerTexel = 1: DE is in output pixels).
    pub fn shade(&self, s: &Sample) -> Rgb {
        match s.class {
            Class::Unresolved => return mix3(self.edge, [0.5; 3], 0.005),
            Class::Interior => return self.interior,
            Class::Exterior => {}
        }
        let nf = s.n as f32;
        let t = (nf * self.density).fract() + s.s * self.density;
        let d = s.de.max(1e-3);
        let footprint = self.density / (d * std::f32::consts::LN_2);
        let mut lod = (footprint * PALETTE_SIZE as f32)
            .max(1.0)
            .log2()
            .clamp(0.0, (self.levels - 1) as f32);
        if s.de >= 999.0 {
            lod = 0.0;
        }
        let dense = ((lod - 7.0) / 3.0).clamp(0.0, 1.0);
        let mut col = self.palette_at(t + self.offset, lod);
        let dist = s.de;
        if self.outline > 0.0 {
            let x = (dist / self.outline_width).clamp(0.0, 1.0);
            let f = x * x * (3.0 - 2.0 * x);
            let edge = (1.0 - f).max(0.75 * dense);
            col = mix3(col, self.edge, edge * self.outline);
        }
        if self.shading > 0.0 {
            let n = norm3([s.ang.cos(), s.ang.sin(), 1.0]);
            let l = self.light;
            let diffuse = (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]).max(0.0);
            let h = self.half;
            let spec = (n[0] * h[0] + n[1] * h[1] + n[2] * h[2]).max(0.0).powf(self.shininess);
            let flat = (l[2] * std::f32::consts::FRAC_1_SQRT_2).max(1e-3);
            let lit = diffuse / flat;
            let m = 1.0 + (lit - 1.0) * self.shading;
            let sp = self.specular * self.shading * spec;
            col = [col[0] * m + sp, col[1] * m + sp, col[2] * m + sp];
        }
        [col[0].max(0.0), col[1].max(0.0), col[2].max(0.0)]
    }
}

/// OKLab of a displayed linear colour (clamped to [0, 1] as the drawable does).
pub fn lab(c: Rgb) -> [f32; 3] {
    let l = oklab([
        c[0].clamp(0.0, 1.0) as f64,
        c[1].clamp(0.0, 1.0) as f64,
        c[2].clamp(0.0, 1.0) as f64,
    ]);
    [l[0] as f32, l[1] as f32, l[2] as f32]
}

/// ΔE in OKLab units x 100 (≈ 1–2 is a just-noticeable difference, unverified rule of thumb).
pub fn delta_e(a: Rgb, b: Rgb) -> f32 {
    let (x, y) = (lab(a), lab(b));
    100.0 * ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
}

pub fn srgb8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0 + 0.5) as u8
}
