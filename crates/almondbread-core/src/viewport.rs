//! Where we look: the centre of the view and the size of one view point.
//!
//! The centre is a [`Center`] with accessors rather than two public `f64`s, so M3b can switch
//! its representation to an arbitrary-precision complex (decimal strings at the API) without
//! touching callers: everything outside this module reads `re()` / `im()` (an f64 view of the
//! centre), offsets the centre by small complex deltas, or asks for the delta between two
//! centres, which stays small and exact enough in f64 at any depth.

/// Centre of a view in the complex plane. Deliberately not `Copy`: the M3b representation
/// (arbitrary precision) will not be.
#[derive(Clone, Debug, PartialEq)]
pub struct Center {
    re: f64,
    im: f64,
}

impl Center {
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// Parses decimal coordinates (the format coordinates cross the FFI in).
    pub fn parse(re: &str, im: &str) -> Option<Self> {
        Some(Self::new(re.trim().parse().ok()?, im.trim().parse().ok()?))
    }

    /// Real part, rounded to f64.
    pub fn re(&self) -> f64 {
        self.re
    }

    /// Imaginary part, rounded to f64.
    pub fn im(&self) -> f64 {
        self.im
    }

    /// Decimal strings that round-trip exactly.
    pub fn to_strings(&self) -> (String, String) {
        (format!("{:e}", self.re), format!("{:e}", self.im))
    }

    /// `self + (dre, dim)`.
    pub fn offset(&self, dre: f64, dim: f64) -> Self {
        Self::new(self.re + dre, self.im + dim)
    }

    /// `self - other` in complex units (small for nearby centres).
    pub fn delta_from(&self, other: &Center) -> (f64, f64) {
        (self.re - other.re, self.im - other.im)
    }
}

/// Centre and size of one view point (not one pixel: independent of the display scale).
#[derive(Clone, Debug, PartialEq)]
pub struct Viewport {
    pub center: Center,
    /// Complex units per view point.
    pub scale: f64,
}

/// Scale of the home view.
pub const HOME_SCALE: f64 = 3.0 / 800.0;

impl Viewport {
    pub const fn new(re: f64, im: f64, scale: f64) -> Self {
        Self {
            center: Center::new(re, im),
            scale,
        }
    }

    pub const fn home() -> Self {
        Self::new(-0.75, 0.0, HOME_SCALE)
    }

    /// Magnification relative to the home view.
    pub fn zoom(&self) -> f64 {
        HOME_SCALE / self.scale
    }

    /// Double-precision limit: below this, neighbouring pixels collapse onto the same
    /// reference coordinate (lifted by arbitrary-precision references in M3b).
    pub fn min_scale(&self) -> f64 {
        4e-16 * 1f64.max(self.center.re.abs()).max(self.center.im.abs())
    }

    /// True when zooming in further is blocked by `min_scale`.
    pub fn at_precision_limit(&self) -> bool {
        self.scale <= self.min_scale() * 1.001
    }

    /// The whole set fitted into a view of `width` x `height` points.
    pub fn fitting(width: f64, height: f64) -> Self {
        if width <= 0.0 || height <= 0.0 {
            return Self::home();
        }
        Self::new(-0.75, 0.0, (3.2 / width).max(2.6 / height))
    }

    /// Complex coordinate of a point given in view points from the centre (y up).
    pub fn point_at_offset(&self, dx: f64, dy: f64) -> Center {
        self.center.offset(dx * self.scale, dy * self.scale)
    }

    /// Zooms by `factor` (> 1 zooms in) keeping the point at offset (dx, dy) fixed.
    pub fn zoom_by(&mut self, factor: f64, dx: f64, dy: f64) {
        let p = self.point_at_offset(dx, dy);
        // min_scale reads the centre before it moves, as the Swift renderer did.
        self.scale = (self.scale / factor).max(self.min_scale()).min(0.05);
        self.center = p.offset(-dx * self.scale, -dy * self.scale);
    }

    /// Moves the content by (dx, dy) view points (the camera moves the other way).
    pub fn pan_by_points(&mut self, dx: f64, dy: f64) {
        self.center = self.center.offset(-dx * self.scale, -dy * self.scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_point_fixed() {
        let mut v = Viewport::new(-0.75, 0.1, 1e-3);
        let before = v.point_at_offset(120.0, -60.0);
        v.zoom_by(1.5, 120.0, -60.0);
        let after = v.point_at_offset(120.0, -60.0);
        let (dr, di) = after.delta_from(&before);
        assert!(dr.abs() < 1e-15 && di.abs() < 1e-15);
        assert!((v.scale - 1e-3 / 1.5).abs() < 1e-18);
    }

    #[test]
    fn zoom_clamps_at_limits() {
        let mut v = Viewport::new(-0.75, 0.0, 1e-15);
        v.zoom_by(100.0, 0.0, 0.0);
        assert!(v.at_precision_limit());
        v.zoom_by(1e-30, 0.0, 0.0);
        assert_eq!(v.scale, 0.05);
    }
}
