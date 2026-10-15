//! The map between two frames' pixel coordinates: a pan/zoom is a pure scale + translation
//! of the complex plane, so `cur = (prev - C) * a + C + b` with C the view centre in pixels.

use almondbread_core::viewport::Viewport;

#[derive(Clone, Copy, Debug)]
pub struct Affine {
    pub a: f64,
    pub b: [f64; 2],
    pub c: [f64; 2],
}

impl Affine {
    pub fn identity(w: usize, h: usize) -> Self {
        Self {
            a: 1.0,
            b: [0.0, 0.0],
            c: [w as f64 / 2.0, h as f64 / 2.0],
        }
    }

    /// Pixels of `prev` (one point per pixel) -> pixels of `cur`, both `w x h`.
    pub fn between(prev: &Viewport, cur: &Viewport, w: usize, h: usize) -> Self {
        let a = prev.scale.ratio(cur.scale);
        let (dx, dy) = cur.center.delta_in(&prev.center, cur.scale);
        Self {
            a,
            b: [-dx, dy],
            c: [w as f64 / 2.0, h as f64 / 2.0],
        }
    }

    pub fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        [
            (p[0] - self.c[0]) * self.a + self.c[0] + self.b[0],
            (p[1] - self.c[1]) * self.a + self.c[1] + self.b[1],
        ]
    }

    pub fn inverse(&self) -> Self {
        Self {
            a: 1.0 / self.a,
            b: [-self.b[0] / self.a, -self.b[1] / self.a],
            c: self.c,
        }
    }
}
