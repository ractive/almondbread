//! The f64 reference orbit the float32 perturbation kernel iterates against
//! (kb: techniques/perturbation). Limits zoom to about 1e-13; M3b replaces it with the
//! arbitrary-precision orbit of `almondbread_math` behind the same fields.

use crate::viewport::{Center, Viewport};

/// `Z_0 = 0, Z_{n+1} = Z_n^2 + C` in f64, stored as float32 pairs for the GPU.
#[derive(Clone, Debug)]
pub struct DoubleReference {
    pub center: Center,
    /// Viewport scale the reference was made for.
    pub scale: f64,
    pub max_iter: usize,
    pub points: Vec<[f32; 2]>,
}

impl DoubleReference {
    /// Iterates until the orbit escapes `bailout` or `max_iter` steps are done. Always at
    /// least two points, so the GPU loop has a valid step. Same arithmetic as the Swift
    /// `ReferenceOrbit` (no FMA contraction), so the points are bit-identical.
    pub fn compute(viewport: &Viewport, max_iter: usize, bailout: f64) -> Self {
        let (cr, ci) = (viewport.center.re(), viewport.center.im());
        let mut pts = Vec::with_capacity(max_iter + 1);
        let (mut x, mut y) = (0.0f64, 0.0f64);
        let b2 = bailout * bailout;
        pts.push([0.0f32, 0.0]);
        for _ in 0..max_iter {
            let nx = x * x - y * y + cr;
            y = 2.0 * x * y + ci;
            x = nx;
            pts.push([x as f32, y as f32]);
            if x * x + y * y > b2 {
                break;
            }
        }
        if pts.len() < 2 {
            pts.push([cr as f32, ci as f32]);
        }
        Self {
            center: viewport.center.clone(),
            scale: viewport.scale,
            max_iter,
            points: pts,
        }
    }

    /// Whether this reference serves a view (rebasing makes any nearby reference valid,
    /// kb: techniques/rebasing): long enough, within `view_radius` of the centre, and made at
    /// a scale within 4x (coordinates are only as good as the scale they were chosen at).
    pub fn serves(&self, viewport: &Viewport, max_iter: usize, view_radius: f64) -> bool {
        let (dr, di) = self.center.delta_from(&viewport.center);
        self.max_iter >= max_iter
            && dr.abs() < view_radius
            && di.abs() < view_radius
            && self.scale <= viewport.scale * 4.0
            && self.scale >= viewport.scale / 4.0
    }
}
