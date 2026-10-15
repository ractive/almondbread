//! CPU port of the kernel's `shade` for the default look (adapted from
//! prototypes/dense-sampling/src/shade.rs, R3) and the OKLab metrics. All strategies and the
//! reference are shaded here from GPU iteration data, so colour differences are the strategies'.

use almondbread_core::palette::{Look, Palette, mip_chain, mip_levels, oklab};

const PALETTE_SIZE: usize = 1024;

pub type Rgb = [f32; 3];

pub const EXTERIOR: u8 = 0;
pub const INTERIOR: u8 = 1;
pub const UNRESOLVED: u8 = 2;

pub struct Shader {
    pal: Vec<[f32; 4]>,
    levels: u32,
    density: f32,
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

pub fn mix3(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Shader {
    pub fn default_look() -> Self {
        let look = Look::default_look();
        let pal = mip_chain(&Palette::named(look.palette).table(PALETTE_SIZE));
        let a = (look.light_angle as f32).to_radians();
        let light = norm3([a.cos(), a.sin(), look.light_height as f32]);
        let half = norm3([light[0], light[1], light[2] + 1.0]);
        let lin = almondbread_core::palette::linear;
        Self {
            pal,
            levels: mip_levels(PALETTE_SIZE) as u32,
            density: look.density as f32,
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

    /// Linear colour of one sample: escape n, smooth offset s, DE in *output pixels of the
    /// frame it is shown in*, normal angle.
    pub fn shade(&self, class: u8, n: f32, s: f32, de_px: f32, ang: f32) -> Rgb {
        match class {
            UNRESOLVED => return mix3(self.edge, [0.5; 3], 0.005),
            INTERIOR => return self.interior,
            _ => {}
        }
        let t = (n * self.density).fract() + s * self.density;
        let d = de_px.max(1e-3);
        let footprint = self.density / (d * std::f32::consts::LN_2);
        let lod = (footprint * PALETTE_SIZE as f32)
            .max(1.0)
            .log2()
            .clamp(0.0, (self.levels - 1) as f32);
        let dense = ((lod - 7.0) / 3.0).clamp(0.0, 1.0);
        let mut col = self.palette_at(t, lod);
        if self.outline > 0.0 {
            let x = (de_px / self.outline_width).clamp(0.0, 1.0);
            let f = x * x * (3.0 - 2.0 * x);
            let edge = (1.0 - f).max(0.75 * dense);
            col = mix3(col, self.edge, edge * self.outline);
        }
        if self.shading > 0.0 {
            let nn = norm3([ang.cos(), ang.sin(), 1.0]);
            let l = self.light;
            let diffuse = (nn[0] * l[0] + nn[1] * l[1] + nn[2] * l[2]).max(0.0);
            let h = self.half;
            let spec = (nn[0] * h[0] + nn[1] * h[1] + nn[2] * h[2])
                .max(0.0)
                .powf(self.shininess);
            let flat = (l[2] * std::f32::consts::FRAC_1_SQRT_2).max(1e-3);
            let lit = diffuse / flat;
            let m = 1.0 + (lit - 1.0) * self.shading;
            let sp = self.specular * self.shading * spec;
            col = [col[0] * m + sp, col[1] * m + sp, col[2] * m + sp];
        }
        [col[0].max(0.0), col[1].max(0.0), col[2].max(0.0)]
    }
}

pub fn lab(c: Rgb) -> [f32; 3] {
    let l = oklab([
        c[0].clamp(0.0, 1.0) as f64,
        c[1].clamp(0.0, 1.0) as f64,
        c[2].clamp(0.0, 1.0) as f64,
    ]);
    [l[0] as f32, l[1] as f32, l[2] as f32]
}

/// OKLab ΔE x 100.
pub fn delta_e_lab(x: [f32; 3], y: [f32; 3]) -> f32 {
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

pub fn f16_to_f32(h: u16) -> f32 {
    let s = ((h >> 15) & 1) as u32;
    let e = ((h >> 10) & 0x1f) as u32;
    let m = (h & 0x3ff) as u32;
    let bits = if e == 0 {
        if m == 0 {
            s << 31
        } else {
            // subnormal
            let mut e2 = 127 - 15 + 1;
            let mut m2 = m;
            while m2 & 0x400 == 0 {
                m2 <<= 1;
                e2 -= 1;
            }
            (s << 31) | ((e2 as u32) << 23) | ((m2 & 0x3ff) << 13)
        }
    } else if e == 31 {
        (s << 31) | 0x7f80_0000 | (m << 13)
    } else {
        (s << 31) | ((e + 127 - 15) << 23) | (m << 13)
    };
    f32::from_bits(bits)
}
