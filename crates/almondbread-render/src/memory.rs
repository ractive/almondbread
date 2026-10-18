//! Memory and thermal policy for devices with a hard per-app memory limit (iOS jetsam) and
//! passive cooling (kb: platform/ios-device "Memory", "Thermal"). Platform-neutral: the
//! platform layer only forwards what the OS says (`os_proc_available_memory()`,
//! `ProcessInfo.thermalState`); the decisions are here.
//!
//! What grows with what (measured 2026-10-18, M1 Pro, release, iOS scheduler defaults; `fly
//! --ios 1` prints the peak by kind, kb: platform/ios-device "Measured"):
//!
//! - per output texel: the full level (rg32f + rg16f, rg32f interior aux), the preview
//!   allocation (the same plus rgba16f colour), two rgba16f history images, the fog pyramid:
//!   67 B on the device's GPU heap, plus three BGRA8 drawables of the layer: [`TEXEL_BYTES`];
//! - per reference point (deep references): float32 and floatexp points on the CPU and again
//!   on the GPU, and while it is computed the progressive prefixes and vector growth: about
//!   130 B per point of process footprint (manual 10 M limit at spiral-1e1000: +408 MB over
//!   1 M); a reference is made for twice the iteration limit: [`REFERENCE_POINT_BYTES`];
//! - fixed, about 108 MB: live-list state buffers (bounded by the live capacity, not by the
//!   image), unresolved lists, kernels, survey and overlays.
//!
//! Without a known limit (macOS, the simulator, the CLI) nothing is capped.

/// Bytes the renderer holds per output texel at the most (all per-pixel textures and buffers
/// allocated: interior colouring, previews, history, lists, fog).
pub const TEXEL_BYTES: u64 = 80;

/// Bytes per point of a deep reference with its tables, CPU and GPU copies together.
pub const REFERENCE_POINT_BYTES: u64 = 130;

/// Shares of the ceiling the per-pixel state and the reference may take; the rest is headroom
/// for the UI (thumbnails, SwiftUI), offscreen renders, the overlays' own renderers and the
/// gap between two polls of the available memory.
const TEXEL_SHARE: f64 = 0.40;
const REFERENCE_SHARE: f64 = 0.25;

/// Never below these (a budget that small means the device is out of memory anyway; the view
/// stays usable): 0.5 Mpx, and the default automatic limit.
const MIN_TEXELS: usize = 1024 * 512;
const MIN_ITERATIONS: usize = 200_000;

/// A new ceiling replaces the old one only when it differs by more than this factor (the
/// available memory moves with every allocation; a cap that followed it would re-render the
/// view each time the iteration cap changed). Memory warnings bypass it.
const HYSTERESIS: f64 = 1.25;

/// What the renderer holds, by kind (bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryReport {
    /// GPU memory of the device in this process (all buffers and textures), when the backend
    /// can tell; includes the GPU copies of the reference and tables.
    pub gpu: Option<u64>,
    /// CPU copies of the current reference (float32 + floatexp points).
    pub reference: u64,
    /// CPU copies of the BLA and LA tables.
    pub tables: u64,
    /// Kept for later: a spare reference, period caches.
    pub caches: u64,
}

impl MemoryReport {
    /// Everything together.
    pub fn total(&self) -> u64 {
        self.gpu.unwrap_or(0) + self.reference + self.tables + self.caches
    }
}

/// The memory the renderer may grow to, from the OS's figure of what is still available.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MemoryBudget {
    /// Bytes the renderer could hold before the OS kills the app: what it holds now plus what
    /// is still available. None: no limit known (nothing is capped).
    pub ceiling: Option<u64>,
}

impl MemoryBudget {
    /// No limit.
    pub const NONE: Self = Self { ceiling: None };

    /// The budget for a renderer holding `held` bytes when the OS reports `available` bytes
    /// left (`os_proc_available_memory()`; 0 means "no limit known", as on the simulator).
    pub fn from_available(held: u64, available: u64) -> Self {
        Self {
            ceiling: (available > 0).then(|| held.saturating_add(available)),
        }
    }

    /// The budget to keep after a new reading `next`: unchanged unless the ceiling moved by more
    /// than the hysteresis (or appeared, or `pressure`: a memory warning, which always lowers).
    pub fn update(self, next: Self, pressure: bool) -> Self {
        match (self.ceiling, next.ceiling) {
            (_, None) if pressure => self,
            (_, None) => next,
            (None, Some(_)) => next,
            (Some(old), Some(new)) => {
                if pressure {
                    Self {
                        ceiling: Some(old.min(new)),
                    }
                } else if (new as f64) > (old as f64) * HYSTERESIS || (new as f64) * HYSTERESIS < old as f64 {
                    next
                } else {
                    self
                }
            }
        }
    }

    /// Output texels the per-pixel state may cover (None: no cap).
    pub fn max_texels(&self) -> Option<usize> {
        let c = self.ceiling?;
        Some(((c as f64 * TEXEL_SHARE / TEXEL_BYTES as f64) as usize).max(MIN_TEXELS))
    }

    /// The iteration limit whose reference (twice as long) and tables fit (None: no cap).
    /// Rounded down to a power of two times 1, 1.25, 1.5 or 1.75 so small changes of the
    /// ceiling keep the limit (a new limit re-renders the view).
    pub fn max_iterations(&self) -> Option<usize> {
        let c = self.ceiling?;
        let points = c as f64 * REFERENCE_SHARE / REFERENCE_POINT_BYTES as f64;
        let raw = (points / 2.0) as usize;
        Some(quantize(raw).max(MIN_ITERATIONS))
    }
}

/// `n` rounded down to 2^k x {1, 1.25, 1.5, 1.75}.
fn quantize(n: usize) -> usize {
    if n < 8 {
        return n;
    }
    let k = usize::BITS - 1 - n.leading_zeros();
    let base = 1usize << k;
    let quarter = base / 4;
    base + (n - base) / quarter * quarter
}

/// The pixel size for a view of `width` x `height` pixels within `max_texels`: the same
/// aspect, scaled down uniformly (None: it fits).
pub fn fit_texels(width: usize, height: usize, max_texels: usize) -> Option<(usize, f64)> {
    let texels = width * height;
    if texels <= max_texels || texels == 0 {
        return None;
    }
    let f = (max_texels as f64 / texels as f64).sqrt();
    Some((((width as f64) * f).floor() as usize, f))
}

/// `ProcessInfo.thermalState` (iOS, macOS): the platform forwards it, the renderer adapts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Thermal {
    #[default]
    Nominal,
    Fair,
    /// Fans at maximum or passive cooling saturated: less GPU per refresh, no long tails of
    /// work (compute further), at most 60 Hz.
    Serious,
    /// The system is about to throttle hard: as `Serious`, at most 30 Hz, and refinement stops
    /// once the view's first full-resolution frame is shown (no anti-aliasing passes).
    Critical,
}

impl Thermal {
    /// From the platform's raw value (`ProcessInfo.ThermalState.rawValue`: 0 nominal .. 3
    /// critical); unknown values count as critical.
    pub fn from_raw(v: u8) -> Self {
        match v {
            0 => Self::Nominal,
            1 => Self::Fair,
            2 => Self::Serious,
            _ => Self::Critical,
        }
    }

    /// Scale of the per-refresh GPU budgets (chunk targets, gesture frame budget).
    pub fn budget_scale(self) -> f64 {
        match self {
            Self::Nominal | Self::Fair => 1.0,
            Self::Serious => 0.5,
            Self::Critical => 0.35,
        }
    }

    /// Scale of the per-view budgets (anti-aliasing, extension passes).
    pub fn view_budget_scale(self) -> f64 {
        match self {
            Self::Nominal | Self::Fair => 1.0,
            Self::Serious => 0.33,
            Self::Critical => 0.2,
        }
    }

    /// "Compute further" (passes past the limit, up to a minute of GPU per view) allowed.
    pub fn allows_further(self) -> bool {
        self < Self::Serious
    }

    /// Refinement stops once the first full-resolution frame of a view is shown.
    pub fn pauses_refinement(self) -> bool {
        self == Self::Critical
    }

    /// The display link's frame-rate ceiling (0: the display's maximum).
    pub fn max_fps(self) -> u32 {
        match self {
            Self::Nominal | Self::Fair => 0,
            Self::Serious => 60,
            Self::Critical => 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1 << 20;

    #[test]
    fn no_limit_caps_nothing() {
        let b = MemoryBudget::from_available(500 * MB, 0);
        assert_eq!(b, MemoryBudget::NONE);
        assert_eq!(b.max_texels(), None);
        assert_eq!(b.max_iterations(), None);
    }

    #[test]
    fn small_device_caps_texels_and_iterations() {
        // A 3 GB iPad: ~1.4 GB per app, ~150 MB taken by the UI: 2360 x 1640 (iPad 10th
        // generation) fits at native size; ~1 M iterations.
        let b = MemoryBudget::from_available(100 * MB, 1150 * MB);
        let t = b.max_texels().unwrap();
        assert!(t >= 2360 * 1640, "{t}");
        let it = b.max_iterations().unwrap();
        assert!((500_000..=2_000_000).contains(&it), "{it}");
        // 800 MB in all: 2752 x 2064 (iPad Pro 13") is rendered smaller.
        let tight = MemoryBudget::from_available(100 * MB, 700 * MB);
        assert!(tight.max_texels().unwrap() < 2752 * 2064);
        // An 8 GB iPad with ~5 GB per app: no practical cap at native size.
        let big = MemoryBudget::from_available(200 * MB, 5000 * MB);
        assert!(big.max_texels().unwrap() > 2752 * 2064);
        assert!(big.max_iterations().unwrap() > 2_000_000);
    }

    #[test]
    fn floors_hold() {
        let b = MemoryBudget::from_available(0, 10 * MB);
        assert_eq!(b.max_texels(), Some(MIN_TEXELS));
        assert_eq!(b.max_iterations(), Some(MIN_ITERATIONS));
    }

    #[test]
    fn hysteresis_and_pressure() {
        let a = MemoryBudget::from_available(100 * MB, 1000 * MB);
        let near = MemoryBudget::from_available(150 * MB, 900 * MB);
        assert_eq!(a.update(near, false), a);
        let far = MemoryBudget::from_available(100 * MB, 600 * MB);
        assert_eq!(a.update(far, false), far);
        // A warning lowers at once, never raises.
        assert_eq!(a.update(near, true).ceiling, Some(1050 * MB));
        let up = MemoryBudget::from_available(100 * MB, 1050 * MB);
        assert_eq!(a.update(up, true), a);
        assert_eq!(MemoryBudget::NONE.update(a, false), a);
    }

    #[test]
    fn quantized_limits() {
        assert_eq!(quantize(1000), 896);
        assert_eq!(quantize(1024), 1024);
        assert_eq!(quantize(1300), 1280);
        assert_eq!(quantize(5), 5);
        for n in [9usize, 100, 12_345, 987_654_321] {
            let q = quantize(n);
            assert!(q <= n && q * 4 > n * 3, "{n} {q}");
        }
    }

    #[test]
    fn fit_keeps_aspect() {
        assert_eq!(fit_texels(100, 100, 20_000), None);
        let (w, f) = fit_texels(2752, 2064, 4_000_000).unwrap();
        let h = (2064.0 * f).floor() as usize;
        assert!(w * h <= 4_000_000);
        assert!((w as f64 / h as f64 - 2752.0 / 2064.0).abs() < 0.01);
    }

    #[test]
    fn thermal_levels() {
        assert_eq!(Thermal::from_raw(0), Thermal::Nominal);
        assert_eq!(Thermal::from_raw(2), Thermal::Serious);
        assert_eq!(Thermal::from_raw(9), Thermal::Critical);
        assert!(Thermal::Fair.allows_further());
        assert!(!Thermal::Serious.allows_further());
        assert!(Thermal::Critical.pauses_refinement());
        assert_eq!(Thermal::Serious.max_fps(), 60);
        assert_eq!(Thermal::Nominal.max_fps(), 0);
    }
}
