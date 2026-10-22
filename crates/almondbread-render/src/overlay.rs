//! Overlays drawn over the fractal in Rust/Metal (kb: decisions/controls, "Orbit overlay and
//! mini-map"): the orbit of the point under the pointer and the mini-map with its nested
//! "breadcrumb" levels. Platform-neutral: a scene of vector primitives in output pixels,
//! binned into 16x16 tiles on the CPU, composited by the `overlay` kernel after the compose
//! pass (or alone, from the history, when only the overlay changed). Overlays never enter the
//! history, the shown-frame checks or exports.
//!
//! The maps are rendered by small renderers of their own (one per level, at the panel's pixel
//! size), stepped only while the main view is not in a gesture, with short chunks, so they
//! never take the gesture frame budget; each level is kept until the view leaves its area.
//! The Julia panel (kb: plans/f1-julia-view, [`crate::julia`]) is one more panel at the end of
//! the map row, the Julia set of the orbit's point, drawn by its own kernel.

use std::cell::Cell;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use almondbread_core::math::{DoubleExp, NewtonOptions, PeriodMethod, find_nucleus_for, find_period_for};
use almondbread_core::orbit::{
    MAP_LINE_POINTS, OrbitClass, OrbitPoints, OrbitProbe, PROBE_MIN_STEPS, ProbePoint, ProbeReference, orbit_points,
};
use almondbread_core::settings::{Corner, InteriorMode, OverlaySettings, RenderSettings, Supersampling};
use almondbread_core::viewport::{Center, Scale, Viewport};
use bytemuck::{Pod, Zeroable};

use crate::backend::{GpuBackend, GpuError, Storage, TextureFormat};
use crate::scheduler::{Renderer, SchedulerConfig};

pub const TILE: usize = 16;

/// One primitive (`OverlayPrim` in MSL), 96 bytes.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct OverlayPrim {
    pub geom: [f32; 4],
    pub color0: [f32; 4],
    pub color1: [f32; 4],
    pub clip: [f32; 4],
    pub param: [f32; 4],
    pub info: [u32; 4],
}

/// `OverlayParams` in MSL, 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct OverlayParams {
    pub out_size: [u32; 2],
    pub tiles_x: u32,
    pub full: u32,
}

const _: () = {
    assert!(std::mem::size_of::<OverlayPrim>() == 96);
    assert!(std::mem::size_of::<OverlayParams>() == 16);
};

const KIND_SEGMENT: u32 = 0;
const KIND_DISC: u32 = 1;
const KIND_RING: u32 = 2;
const KIND_RECT: u32 = 3;
const KIND_IMAGE: u32 = 4;
const KIND_GLYPH: u32 = 5;
const KIND_GLOW: u32 = 6;
const KIND_TRIANGLE: u32 = 7;

/// Premultiplied linear RGBA from linear RGB and alpha.
pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    [r * a, g * a, b * a, a]
}

const NO_CLIP: [f32; 4] = [-1e9, -1e9, 1e9, 1e9];

/// A list of primitives in output pixels (origin top-left, y down), drawn in order.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub prims: Vec<OverlayPrim>,
    clip: Option<[f32; 4]>,
}

impl Scene {
    fn push(&mut self, mut p: OverlayPrim) {
        p.clip = self.clip.unwrap_or(NO_CLIP);
        self.prims.push(p);
    }

    /// Following primitives are drawn only inside `r` (x0, y0, x1, y1); None: everywhere.
    pub fn set_clip(&mut self, r: Option<[f32; 4]>) {
        self.clip = r;
    }

    pub fn segment(&mut self, a: [f64; 2], b: [f64; 2], half_width: f32, c0: [f32; 4], c1: [f32; 4]) {
        self.push(OverlayPrim {
            geom: [a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32],
            color0: c0,
            color1: c1,
            param: [half_width, 0.0, 0.0, 0.0],
            info: [KIND_SEGMENT, 0, 0, 0],
            ..OverlayPrim::default()
        });
    }

    pub fn disc(&mut self, c: [f64; 2], radius: f32, color: [f32; 4]) {
        self.push(OverlayPrim {
            geom: [c[0] as f32, c[1] as f32, radius, 0.0],
            color0: color,
            info: [KIND_DISC, 0, 0, 0],
            ..OverlayPrim::default()
        });
    }

    pub fn ring(&mut self, c: [f64; 2], radius: f32, width: f32, color: [f32; 4]) {
        self.push(OverlayPrim {
            geom: [c[0] as f32, c[1] as f32, radius, width],
            color0: color,
            info: [KIND_RING, 0, 0, 0],
            ..OverlayPrim::default()
        });
    }

    pub fn rect(&mut self, r: [f32; 4], radius: f32, fill: [f32; 4], border: [f32; 4], border_width: f32) {
        self.push(OverlayPrim {
            geom: r,
            color0: fill,
            color1: border,
            param: [radius, border_width, 0.0, 0.0],
            info: [KIND_RECT, 0, 0, 0],
            ..OverlayPrim::default()
        });
    }

    /// A filled triangle (the compass needle).
    pub fn triangle(&mut self, a: [f64; 2], b: [f64; 2], c: [f64; 2], color: [f32; 4]) {
        self.push(OverlayPrim {
            geom: [a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32],
            color0: color,
            param: [c[0] as f32, c[1] as f32, 0.0, 0.0],
            info: [KIND_TRIANGLE, 0, 0, 0],
            ..OverlayPrim::default()
        });
    }

    pub fn image(&mut self, r: [f32; 4], map: u32, opacity: f32, radius: f32) {
        self.push(OverlayPrim {
            geom: r,
            param: [opacity, radius, 0.0, 0.0],
            info: [KIND_IMAGE, map, 0, 0],
            ..OverlayPrim::default()
        });
    }

    /// The glow picture (the π meter's orbit trail, [`OverlayDraw::trail`], `size` pixels)
    /// stretched over `r` and added to the image under it at `opacity`.
    pub fn glow_image(&mut self, r: [f32; 4], size: (usize, usize), opacity: f32) {
        self.push(OverlayPrim {
            geom: r,
            param: [opacity, 0.0, 0.0, 0.0],
            info: [KIND_GLOW, size.0 as u32, size.1 as u32, 0],
            ..OverlayPrim::default()
        });
    }

    /// The orbit's dot cloud ([`OverlayDraw::cloud`], `size` pixels: straight sRGB colour and
    /// alpha) stretched over `r` and blended over the image under it at `opacity`.
    pub fn cloud_image(&mut self, r: [f32; 4], size: (usize, usize), opacity: f32) {
        self.push(OverlayPrim {
            geom: r,
            param: [opacity, 0.0, 0.0, 0.0],
            info: [KIND_GLOW, size.0 as u32, size.1 as u32, 1],
            ..OverlayPrim::default()
        });
    }

    /// Text in the overlay font (`crate::font`, an SDF atlas) at `size` pixels per em, the
    /// top of its capitals at `origin`; `halo` is the outline colour (premultiplied; alpha 0:
    /// none) and `halo_width` its width in pixels. Returns the width drawn.
    pub fn text_halo(
        &mut self,
        origin: [f32; 2],
        size: f32,
        text: &str,
        color: [f32; 4],
        halo: [f32; 4],
        halo_width: f32,
    ) -> f32 {
        let (glyphs, width) = crate::font::layout(text, size);
        let base = [origin[0], origin[1] + crate::font::cap_height() * size];
        // The atlas stores distances out to SPREAD texels: the halo cannot reach further.
        let reach = crate::font::SPREAD * size / crate::font::EM - 0.25;
        let halo_width = halo_width.clamp(0.0, reach.max(0.0));
        for g in glyphs {
            self.push(OverlayPrim {
                geom: [
                    base[0] + g.quad[0],
                    base[1] + g.quad[1],
                    base[0] + g.quad[2],
                    base[1] + g.quad[3],
                ],
                color0: color,
                color1: halo,
                param: g.atlas,
                info: [
                    KIND_GLYPH,
                    crate::font::ATLAS_W,
                    halo_width.to_bits(),
                    crate::font::SPREAD.to_bits(),
                ],
                ..OverlayPrim::default()
            });
        }
        width
    }

    /// Text without a halo.
    pub fn text(&mut self, origin: [f32; 2], size: f32, text: &str, color: [f32; 4]) -> f32 {
        self.text_halo(origin, size, text, color, [0.0; 4], 0.0)
    }

    /// Text with a dark outline (readable on any palette, bright or dark).
    pub fn label(&mut self, origin: [f32; 2], size: f32, text: &str, color: [f32; 4]) -> f32 {
        let halo = rgba(0.0, 0.0, 0.0, 0.85 * color[3].clamp(0.0, 1.0));
        self.text_halo(origin, size, text, color, halo, (0.2 * size).max(2.5))
    }

    /// Pixel bounding box of primitive `i` (inflated for anti-aliasing), clipped.
    fn bounds(p: &OverlayPrim) -> [f32; 4] {
        let g = p.geom;
        let b = match p.info[0] {
            KIND_SEGMENT => {
                let w = p.param[0] + 1.5;
                [
                    g[0].min(g[2]) - w,
                    g[1].min(g[3]) - w,
                    g[0].max(g[2]) + w,
                    g[1].max(g[3]) + w,
                ]
            }
            KIND_DISC => [
                g[0] - g[2] - 1.5,
                g[1] - g[2] - 1.5,
                g[0] + g[2] + 1.5,
                g[1] + g[2] + 1.5,
            ],
            KIND_RING => {
                let r = g[2] + 0.5 * g[3] + 1.5;
                [g[0] - r, g[1] - r, g[0] + r, g[1] + r]
            }
            KIND_TRIANGLE => {
                let q = p.param;
                [
                    g[0].min(g[2]).min(q[0]) - 1.5,
                    g[1].min(g[3]).min(q[1]) - 1.5,
                    g[0].max(g[2]).max(q[0]) + 1.5,
                    g[1].max(g[3]).max(q[1]) + 1.5,
                ]
            }
            _ => [g[0] - 1.5, g[1] - 1.5, g[2] + 1.5, g[3] + 1.5],
        };
        [
            b[0].max(p.clip[0]),
            b[1].max(p.clip[1]),
            b[2].min(p.clip[2]),
            b[3].min(p.clip[3]),
        ]
    }

    /// Calls `f` with every tile (index in a `tx` x `ty` grid of a `w` x `h` target) that
    /// primitive `p` touches; a segment's tiles can come more than once.
    fn for_tiles(p: &OverlayPrim, w: usize, h: usize, tx: usize, ty: usize, mut f: impl FnMut(usize)) {
        let (wf, hf) = (w as f32, h as f32);
        let b = Self::bounds(p);
        let (x0, y0) = (b[0].max(0.0), b[1].max(0.0));
        let (x1, y1) = (b[2].min(wf - 1.0), b[3].min(hf - 1.0));
        if !(x0 <= x1 && y0 <= y1) {
            return;
        }
        if p.info[0] == KIND_SEGMENT {
            // Clip the segment to the target (plus its width) and walk it.
            let r = p.param[0] + 1.5;
            let Some((a, bb)) = clip_segment(
                [p.geom[0], p.geom[1]],
                [p.geom[2], p.geom[3]],
                [x0 - r, y0 - r, x1 + r, y1 + r],
            ) else {
                return;
            };
            let len = ((bb[0] - a[0]).hypot(bb[1] - a[1])).max(1e-3);
            let steps = (len / (TILE as f32 * 0.5)).ceil() as usize + 1;
            let reach = r.ceil();
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let (px, py) = (a[0] + (bb[0] - a[0]) * t, a[1] + (bb[1] - a[1]) * t);
                let cx0 = ((px - reach).max(x0) as usize) / TILE;
                let cx1 = ((px + reach).min(x1).max(0.0) as usize) / TILE;
                let cy0 = ((py - reach).max(y0) as usize) / TILE;
                let cy1 = ((py + reach).min(y1).max(0.0) as usize) / TILE;
                for cy in cy0..=cy1.min(ty - 1) {
                    for cx in cx0..=cx1.min(tx - 1) {
                        f(cy * tx + cx);
                    }
                }
            }
        } else {
            for cy in (y0 as usize / TILE)..=((y1 as usize / TILE).min(ty - 1)) {
                for cx in (x0 as usize / TILE)..=((x1 as usize / TILE).min(tx - 1)) {
                    f(cy * tx + cx);
                }
            }
        }
    }

    /// Bins the primitives into 16x16 tiles of a `w` x `h` target: per tile the indices of
    /// the primitives touching it, in draw order (CSR: `start[t]..start[t + 1]`), and the
    /// tiles that have any. Segments are walked along their length (a long segment touches
    /// a thin band of tiles, not its bounding box). Two passes over the primitives (count,
    /// then fill) and no list per tile: large panels touch thousands of tiles, and a vector
    /// per tile cost 0.6 ms a frame at Retina size (measured 2026-10-15, the π meter's inset).
    pub fn bin(&self, w: usize, h: usize) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
        let tx = w.div_ceil(TILE).max(1);
        let ty = h.div_ceil(TILE).max(1);
        let n = tx * ty;
        // Primitives are visited in order, so a tile's entries come in draw order; a tile a
        // segment touches twice is recognised by its last entry being that primitive.
        let mut last = vec![u32::MAX; n];
        let mut count = vec![0u32; n];
        for (i, p) in self.prims.iter().enumerate() {
            let i = i as u32;
            Self::for_tiles(p, w, h, tx, ty, |t| {
                if last[t] != i {
                    last[t] = i;
                    count[t] += 1;
                }
            });
        }
        let mut start = Vec::with_capacity(n + 1);
        let mut active = Vec::new();
        let mut total = 0u32;
        for (t, &c) in count.iter().enumerate() {
            start.push(total);
            if c > 0 {
                active.push(t as u32);
            }
            total += c;
        }
        start.push(total);
        let mut list = vec![0u32; total as usize];
        let mut cursor: Vec<u32> = start[..n].to_vec();
        last.fill(u32::MAX);
        for (i, p) in self.prims.iter().enumerate() {
            let i = i as u32;
            Self::for_tiles(p, w, h, tx, ty, |t| {
                if last[t] != i {
                    last[t] = i;
                    list[cursor[t] as usize] = i;
                    cursor[t] += 1;
                }
            });
        }
        (start, list, active)
    }
}

/// Liang-Barsky: the part of segment a-b inside rectangle r.
pub(crate) fn clip_segment(a: [f32; 2], b: [f32; 2], r: [f32; 4]) -> Option<([f32; 2], [f32; 2])> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-dx, a[0] - r[0]),
        (dx, r[2] - a[0]),
        (-dy, a[1] - r[1]),
        (dy, r[3] - a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then(|| ([a[0] + dx * t0, a[1] + dy * t0], [a[0] + dx * t1, a[1] + dy * t1]))
}

// MARK: Font

/// The font atlas as the kernel reads it (`crate::font`).
pub fn font_bytes() -> Vec<u8> {
    crate::font::ATLAS.to_vec()
}

// MARK: GPU

/// A scene uploaded for the `overlay` kernel.
pub struct OverlayDraw<B: GpuBackend> {
    pub prims: B::Buffer,
    pub tile_start: B::Buffer,
    pub tile_list: B::Buffer,
    pub active: B::Buffer,
    pub active_count: usize,
    pub size: (usize, usize),
    /// Map textures by index (unused slots bind the first, or the target).
    pub maps: Vec<B::Texture>,
    /// The π meter's orbit trail picture, BGRA8 rows (glow primitives read it).
    pub trail: Option<B::Buffer>,
    /// The orbit's dot cloud on the whole-set map, BGRA8 rows with alpha ([`orbit_cloud_picture`]).
    pub cloud: Option<B::Buffer>,
    /// Changes whenever the scene does (the renderer re-presents when it differs from the
    /// shown one).
    pub version: u64,
}

impl<B: GpuBackend> OverlayDraw<B> {
    pub fn upload(
        backend: &B,
        scene: &Scene,
        size: (usize, usize),
        maps: Vec<B::Texture>,
        trail: Option<B::Buffer>,
        version: u64,
    ) -> Result<Self, GpuError> {
        let (start, list, active) = scene.bin(size.0, size.1);
        let nonempty = |v: &[u32]| if v.is_empty() { vec![0u32] } else { v.to_vec() };
        let prims: Vec<OverlayPrim> = if scene.prims.is_empty() {
            vec![OverlayPrim::default()]
        } else {
            scene.prims.clone()
        };
        Ok(Self {
            prims: backend.create_buffer_with_data(bytemuck::cast_slice(&prims))?,
            tile_start: backend.create_buffer_with_data(bytemuck::cast_slice(&start))?,
            tile_list: backend.create_buffer_with_data(bytemuck::cast_slice(&nonempty(&list)))?,
            active: backend.create_buffer_with_data(bytemuck::cast_slice(&nonempty(&active)))?,
            active_count: active.len(),
            size,
            maps,
            trail,
            cloud: None,
            version,
        })
    }
}

// MARK: Maps

/// Complex units per pixel and centre of the whole-set map for a `w` x `h` panel.
/// Mini-map level depths (decades below the whole set) for a view `depth` decades deep: the
/// whole set (0), then `want - 1` levels spread evenly between it and the view, snapped to a
/// 3-decade grid and at least 1.5 decades shallower than the view. Until 2026-10-16 the levels
/// were the deepest grid steps just above the view, which left one huge jump after the whole set
/// (whole set, 1e15, 1e18 at 1e20; user feedback). Even spacing shows the marker of the next
/// level as a crosshair more often (a rectangle needs a gap below ~1.4 decades), but every map
/// says something. Hysteresis: a level from `prev` (same slot) stays while its even target is
/// within one grid step, so zooming re-renders a level only every few decades.
pub(crate) fn map_decades(depth: f64, want: usize, prev: &[i32]) -> Vec<i32> {
    const STEP: f64 = 3.0;
    let mut out = vec![0];
    // At most 15 levels (the setting allows a few); a non-finite depth (an f64 overflow past
    // 1e308 made the old level loop endless, kb: techniques/hang-prevention) has only the whole set.
    let slots = want.saturating_sub(1).min(15);
    let deepest = depth - 1.5;
    if slots == 0 || !depth.is_finite() || deepest < STEP {
        return out;
    }
    let same_slots = prev.len() == slots;
    for k in 1..=slots {
        let target = depth * k as f64 / want as f64;
        let mut d = (target / STEP).round() * STEP;
        if same_slots {
            let p = f64::from(prev[k - 1]);
            if (p - target).abs() <= STEP && p <= deepest {
                d = p;
            }
        }
        let d = d.clamp(STEP, (deepest / STEP).floor() * STEP) as i32;
        if d > *out.last().expect("whole set") {
            out.push(d);
        }
    }
    out
}

/// Map row metrics (points): margin from the canvas edges, gap between panels, the panel's
/// height per width, and the room kept above a bottom row for its info lines.
const MAP_MARGIN: f64 = 12.0;
const MAP_GAP: f64 = 8.0;
const MAP_ASPECT: f64 = 0.72;
const MAP_INFO: f64 = 44.0;

/// The map row of this frame (kb: decisions/controls "Mini-map on small screens").
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MapFit {
    /// Levels shown: the setting's, or fewer where the row has no room for them.
    pub levels: usize,
    /// Panel width (points): the setting's, or down to 2/3 of it to keep a level.
    pub width: f64,
    /// Margins from the platform's controls besides the safe area (points): right (the
    /// trailing column or throttle; at the bottom also a phone's bottom button row) and top
    /// (the iPad's top-trailing buttons, for a top-right row).
    pub right: f64,
    pub top: f64,
}

/// How many map levels fit and how wide: the row keeps clear of the platform's trailing
/// controls (`trailing`: the button column or the throttle; `cockpit.inset_right`: the bottom
/// button row or column, which a bottom row also keeps left of; `top_controls`: the iPad's
/// top-trailing row). Panels narrow down to 2/3 of `map_size` before a level is dropped. A
/// bottom-left row also leaves the cockpit a place: beside the row, or lifted above it while
/// the lifted cockpit stays in the lower half of the canvas (the upper half is the HUD's and
/// the tour card's); otherwise fewer levels (a phone on its side: one map beside a two-row
/// cockpit, where two maps lifted it into the HUD).
#[allow(clippy::too_many_arguments)]
pub(crate) fn fit_maps(
    want: usize,
    map_size: f64,
    corner: Corner,
    size: (usize, usize),
    ppp: f64,
    insets: (f64, f64, f64, f64),
    trailing: f64,
    top_controls: f64,
    cockpit: &crate::cockpit::CockpitOverlay,
) -> MapFit {
    let (w, h) = (size.0 as f64 * ppp, size.1 as f64 * ppp);
    let bottom = matches!(corner, Corner::BottomLeft | Corner::BottomRight);
    let right = if bottom {
        trailing.max(cockpit.inset_right)
    } else {
        trailing
    }
    .max(0.0);
    let top = if corner == Corner::TopRight {
        top_controls.max(0.0)
    } else {
        0.0
    };
    let left_edge = insets.1.max(0.0) + MAP_MARGIN;
    let avail = w - left_edge - insets.3.max(0.0) - right - MAP_MARGIN;
    let min_w = (map_size * 2.0 / 3.0).max(80.0).min(map_size);
    let fit = |levels: usize, width: f64| MapFit {
        levels,
        width,
        right,
        top,
    };
    let want = want.max(1);
    for n in (1..=want).rev() {
        let k = n as f64;
        let width = map_size.min((avail - (k - 1.0) * MAP_GAP) / k);
        if width < min_w {
            continue;
        }
        if corner != Corner::BottomLeft || !cockpit.enabled {
            return fit(n, width);
        }
        let row_right = left_edge + k * width + (k - 1.0) * MAP_GAP;
        if cockpit.fits(size, ppp, insets, row_right) {
            return fit(n, width);
        }
        let lift = width * MAP_ASPECT + MAP_MARGIN + MAP_INFO;
        let lifted = cockpit.layout_lifted(size, ppp, insets, 0.0, lift);
        if lifted[1] * ppp >= h / 2.0 {
            return fit(n, width);
        }
    }
    // No room even for one panel at 2/3 size (a tiny canvas): one, as wide as there is room.
    fit(1, map_size.min(avail).max(min_w.min(80.0)))
}

/// The map row's wishes: map levels (the setting's with the mini-map on, the whole-set map for
/// the orbit alone; at most three with the Julia panel, whose picture takes the overlay kernel's
/// fourth map texture) and panels in all (plus the Julia panel; at least one).
pub(crate) fn row_wants(s: &OverlaySettings) -> (usize, usize) {
    let maps = if s.map { s.map_levels } else { usize::from(s.orbit) };
    let maps = if s.julia { maps.min(3) } else { maps };
    (maps, (maps + usize::from(s.julia)).max(1))
}

/// Map levels in a row of `fit` panels: the Julia panel keeps its place, the deepest levels
/// go first.
pub(crate) fn row_map_levels(fit: usize, map_slots: usize, julia: bool) -> usize {
    fit.saturating_sub(usize::from(julia)).min(map_slots)
}

/// Top-left pixels of the first `n` panels of the map row (`panel` pixels each) from the
/// chosen corner: clear of the toolbar and a sidebar over the canvas (safe-area `insets`,
/// points) and of the platform's controls (`fit`). The maps come first, the Julia panel after
/// them.
pub(crate) fn row_origins(
    n: usize,
    corner: Corner,
    panel: (usize, usize),
    size: (usize, usize),
    insets: (f64, f64, f64, f64),
    fit: &MapFit,
    px_per_pt: f64,
) -> Vec<[f32; 2]> {
    let margin = (MAP_MARGIN * px_per_pt) as f32;
    let gap = (MAP_GAP * px_per_pt) as f32;
    let (pw, ph) = (panel.0 as f32, panel.1 as f32);
    let (w, h) = (size.0 as f32, size.1 as f32);
    let (top, left, bottom, right) = insets;
    let px = |v: f64| (v * px_per_pt) as f32;
    let (x0, x1) = (px(left.max(0.0)) + margin, w - px(right.max(0.0) + fit.right) - margin);
    let (y0, y1) = (px(top.max(0.0) + fit.top) + margin, h - px(bottom.max(0.0)) - margin);
    (0..n)
        .map(|i| {
            let off = i as f32 * (pw + gap);
            match corner {
                Corner::BottomLeft => [x0 + off, y1 - ph],
                Corner::BottomRight => [x1 - pw - off, y1 - ph],
                Corner::TopLeft => [x0 + off, y0],
                Corner::TopRight => [x1 - pw - off, y0],
            }
        })
        .collect()
}

/// `re ± im·i` with four decimals (the Julia panel's value of k), a true minus sign.
pub fn complex_text(k: (f64, f64)) -> String {
    let num = |v: f64| {
        let t = format!("{:.4}", v.abs());
        if v < 0.0 && t.chars().any(|c| c.is_ascii_digit() && c != '0') {
            format!("\u{2212}{t}")
        } else {
            t
        }
    };
    let sign = if k.1 < 0.0 { "\u{2212}" } else { "+" };
    format!("{} {sign} {}i", num(k.0), num(k.1.abs()))
}

fn full_set_view(w: usize, h: usize) -> Viewport {
    let s = (3.2 / w as f64).max(2.6 / h as f64);
    Viewport::new(-0.7, 0.0, s)
}

/// The whole-set map level of `formula` (F4: a Julia set's whole view, centred on 0; F3: a
/// Multibrot set's home view, centred on 0, as Reset shows it).
fn full_view_of(formula: almondbread_core::Formula, w: usize, h: usize) -> Viewport {
    if formula.is_mandelbrot() {
        full_set_view(w, h)
    } else {
        Viewport::fitting_for(formula, w as f64, h as f64)
    }
}

/// The whole-set map's dot cloud as a picture of `size` pixels (BGRA8: straight sRGB colour,
/// alpha), `centre` and `scale` (complex units per pixel) placing it like the map. One pixel
/// per point: an orbit of 10^5 steps is a cloud, not a polyline (kb: decisions/controls "Orbit
/// overlay"). Per pixel the visits and the newest visit: newer points are brighter and warmer
/// (white), older ones faint and cool, more visits more opaque; every dot gets a dark rim (the
/// empty neighbours of occupied pixels) so the cloud reads on bright and dark palettes. Cost:
/// one pass over the points and two over the pixels (0.43 ms for 10^5 points on a Retina
/// panel, measured 2026-10-17, M1 Pro, release). None when no point lands on the map.
pub fn orbit_cloud_picture(points: &[[f32; 2]], centre: [f64; 2], scale: f64, size: (usize, usize)) -> Option<Vec<u8>> {
    let (w, h) = size;
    if points.is_empty() || w == 0 || h == 0 || scale.is_nan() || scale <= 0.0 {
        return None;
    }
    let mut visits = vec![0u32; w * h];
    let mut newest = vec![0u32; w * h];
    let (hw, hh) = (w as f64 / 2.0, h as f64 / 2.0);
    let inv = 1.0 / scale;
    let mut any = false;
    for (k, p) in points.iter().enumerate() {
        let x = hw + (f64::from(p[0]) - centre[0]) * inv;
        let y = hh - (f64::from(p[1]) - centre[1]) * inv;
        if !(x >= 0.0 && y >= 0.0 && x < w as f64 && y < h as f64) {
            continue;
        }
        let i = y as usize * w + x as usize;
        visits[i] += 1;
        newest[i] = k as u32;
        any = true;
    }
    if !any {
        return None;
    }
    // Colour and alpha by age in 64 steps (straight sRGB bytes), and opacity by visits in
    // powers of two: tables, not a powf per pixel.
    const AGES: usize = 64;
    let enc = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let s = if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0 + 0.5) as u8
    };
    let ages: Vec<([u8; 3], f32)> = (0..AGES)
        .map(|j| {
            let t = j as f32 / (AGES - 1) as f32;
            let c: [f32; 3] = std::array::from_fn(|i| ORBIT_COOL[i] + (1.0 - ORBIT_COOL[i]) * t);
            ([enc(c[2]), enc(c[1]), enc(c[0])], 0.4 + 0.6 * t)
        })
        .collect();
    let to_age = (AGES - 1) as f32 / (points.len() - 1).max(1) as f32;
    // The rim: empty pixels next to a dot (the occupied mask dilated by one pixel, rows then
    // columns).
    let mut row = vec![0u8; w * h];
    for y in 0..h {
        let v = &visits[y * w..(y + 1) * w];
        let r = &mut row[y * w..(y + 1) * w];
        for x in 0..w {
            r[x] = u8::from(v[x] > 0 || (x > 0 && v[x - 1] > 0) || (x + 1 < w && v[x + 1] > 0));
        }
    }
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let n = visits[i];
            if n > 0 {
                let (c, age_alpha) = ages[(newest[i] as f32 * to_age) as usize];
                let density = (0.5 + 0.17 * (31 - n.leading_zeros()) as f32).min(1.0);
                let a = density * age_alpha;
                out[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], (a * 255.0 + 0.5) as u8]);
            } else if row[i] != 0 || (y > 0 && row[i - w] != 0) || (y + 1 < h && row[i + w] != 0) {
                out[i * 4 + 3] = 140;
            }
        }
    }
    Some(out)
}

/// The dot cloud's key: probe serial, growth step, panel size.
type CloudKey = (u64, u64, (usize, usize));

/// One map level: a small renderer of its own at the panel's pixel size.
struct MapLevel<B: GpuBackend> {
    renderer: Renderer<B>,
    target: B::Texture,
    viewport: Viewport,
    /// The level's zoom relative to the whole-set map, as a power of ten (0: the whole set).
    decade: i32,
    size: (usize, usize),
    /// A frame is presented (the panel shows something).
    shown: bool,
}

/// The current maps and how they are laid out.
struct MapStack<B: GpuBackend> {
    levels: Vec<MapLevel<B>>,
    /// Top-left of each panel in output pixels, and the panel size in pixels.
    panels: Vec<[f32; 2]>,
    panel: (usize, usize),
    /// Renderers of dropped levels, for reuse.
    spare: Vec<MapLevel<B>>,
}

/// What a click on a map asks for.
#[derive(Clone, Debug)]
pub struct MapJump {
    pub center: Center,
    /// The click was underdetermined at the map's resolution and was moved to a nearby
    /// minibrot (kb: decisions/controls): what was done, for the UI.
    pub note: String,
    /// Option/Alt-click: fly there (autopilot) rather than jump.
    pub fly: bool,
}

/// A map click being resolved on a thread (snapping to a minibrot can take a while).
struct SnapJob {
    handle: JoinHandle<MapJump>,
}

// MARK: The overlay manager

/// Overlay state of one view: settings, the orbit probe, the maps, the uploaded scene.
pub struct Overlay<B: GpuBackend> {
    pub settings: OverlaySettings,
    probe_key: Option<(Viewport, [i64; 2], usize, u64, almondbread_core::Formula)>,
    probe: Option<OrbitProbe>,
    points: OrbitPoints,
    probe_reference: Option<(u64, Arc<ProbeReference>)>,
    maps: Option<MapStack<B>>,
    scene_key: Option<String>,
    version: u64,
    font: Option<B::Buffer>,
    snap: Option<SnapJob>,
    /// The last map click's note and when it was made.
    pub note: Option<(String, f64)>,
    /// A finished map click waiting to be applied by the host.
    pub pending_jump: Option<MapJump>,
    /// The π meter (its own settings and thread; kb: techniques/pi-in-the-mandelbrot-set).
    pub pi: crate::pi_meter::PiMeter,
    /// The cosmic size scale panel (kb: decisions/controls, "Cosmic size scale").
    pub cosmic: crate::cosmic::CosmicOverlay,
    /// The compass (kb: decisions/controls, "Compass").
    pub compass: crate::compass::CompassOverlay,
    /// The HUD cockpit (kb: decisions/controls, "Driving levels").
    pub cockpit: crate::cockpit::CockpitOverlay,
    /// The autopilot's target marker (kb: decisions/controls, "Target marker").
    pub marker: crate::marker::TargetMarker,
    /// A guided tour's probe point and marker (kb: decisions/controls "Guided tours").
    pub tour: crate::tour::TourOverlay,
    /// The tour card's rect (view points from the top left: x, y, width, height) while a
    /// platform tour card shows; the π meter keeps clear of it.
    pub tour_card: Option<[f64; 4]>,
    /// The π meter's uploaded trail picture and its serial.
    trail: Option<(u64, B::Buffer)>,
    /// Counts new probes (the dot cloud's key).
    probe_serial: u64,
    /// The orbit's uploaded dot cloud (None: nothing on the map) under its key: probe, growth
    /// step, panel size ([`Self::cloud_key`]).
    cloud: Option<(CloudKey, Option<B::Buffer>)>,
    /// The panels' help badges as laid out in the last scene (kb: decisions/controls "Help").
    pub help: Vec<crate::help::HelpBadge>,
    /// The panels as laid out in the last scene (pixels: x0, y0, x1, y1): `cosmic`, `cockpit`,
    /// `pi`, `pi-trail`, `maps` (the map panels), `julia`. UI tests check they keep clear of each
    /// other and of the platform's controls.
    pub panels: Vec<(&'static str, [f32; 4])>,
    drawn_cosmic: Cell<Option<[f32; 4]>>,
    drawn_cockpit: Cell<Option<[f32; 4]>>,
    drawn_maps: Cell<Option<[f32; 4]>>,
    drawn_julia: Cell<Option<[f32; 4]>>,
    /// The Julia companion panel (kb: plans/f1-julia-view): its picture, and where it sits in
    /// the map row (top-left pixel, panel pixels) while it is on.
    pub julia: crate::julia::JuliaPanel<B>,
    julia_at: Option<([f32; 2], (usize, usize))>,
    /// The panel's k at full precision (F4: what a click on the panel opens; kept while the
    /// pointer is over the panel, so moving onto it does not change the picture).
    pub julia_k: Option<Center>,
    /// A click on the panel asked to open its Julia set as the main view (the host applies it,
    /// kb: plans/f4-deep-julia).
    pub open_julia: Option<Center>,
}

impl<B: GpuBackend> Default for Overlay<B> {
    fn default() -> Self {
        Self {
            settings: OverlaySettings::default(),
            probe_key: None,
            probe: None,
            points: OrbitPoints::default(),
            probe_reference: None,
            maps: None,
            scene_key: None,
            version: 0,
            font: None,
            snap: None,
            note: None,
            pending_jump: None,
            pi: crate::pi_meter::PiMeter::default(),
            cosmic: Default::default(),
            compass: Default::default(),
            cockpit: Default::default(),
            tour_card: None,
            marker: Default::default(),
            tour: Default::default(),
            trail: None,
            probe_serial: 0,
            cloud: None,
            help: Vec::new(),
            panels: Vec::new(),
            drawn_cosmic: Cell::new(None),
            drawn_cockpit: Cell::new(None),
            drawn_maps: Cell::new(None),
            drawn_julia: Cell::new(None),
            julia: Default::default(),
            julia_at: None,
            julia_k: None,
            open_julia: None,
        }
    }
}

/// Orbit colours: the point c, then the iterates fading along the orbit.
const ORBIT_WARM: [f32; 3] = [1.0, 0.86, 0.45];
const ORBIT_COOL: [f32; 3] = [0.55, 0.85, 1.0];

impl<B: GpuBackend + Clone> Overlay<B> {
    /// The class of the probed point (overlay text and the FFI).
    pub fn orbit_class(&self) -> Option<&OrbitClass> {
        self.probe.as_ref().map(|p| &p.class)
    }

    pub fn orbit_points(&self) -> &OrbitPoints {
        &self.points
    }

    /// The probed point's fate in plain words (the overlay's line, the FFI), None without a
    /// probe.
    pub fn orbit_text(&self) -> Option<String> {
        self.probe.as_ref().map(OrbitProbe::text)
    }

    /// The probe's dot cloud (the whole-set map's points).
    pub fn orbit_cloud(&self) -> Option<&almondbread_core::orbit::OrbitCloud> {
        self.probe.as_ref().map(|p| &p.cloud)
    }

    /// The dot cloud's key: the probe, its growth in steps of ~19 % (a quarter of a doubling;
    /// the picture is rebuilt only then, ~45 times on the way to 10^5 points) or done, and the
    /// panel size.
    fn cloud_key(&self, panel: (usize, usize)) -> Option<CloudKey> {
        let p = self.probe.as_ref()?;
        let seen = p.cloud.seen;
        let step = if p.done() {
            u64::MAX
        } else if seen == 0 {
            0
        } else {
            ((seen as f64).log2() * 4.0) as u64 + 1
        };
        Some((self.probe_serial, step, panel))
    }

    /// Rebuilds and uploads the dot cloud when its key changed (the orbit overlay with the
    /// whole-set map showing).
    fn update_cloud(&mut self, renderer: &Renderer<B>, s: &OverlaySettings) {
        let level0 = self
            .maps
            .as_ref()
            .and_then(|m| m.levels.first().map(|l| (l.viewport.clone(), m.panel)));
        let key = match (&level0, s.orbit) {
            (Some((_, panel)), true) => self.cloud_key(*panel),
            _ => None,
        };
        let Some(key) = key else {
            self.cloud = None;
            return;
        };
        if self.cloud.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let (vp, panel) = level0.expect("level 0");
        let cloud = &self.probe.as_ref().expect("probe").cloud;
        let picture = orbit_cloud_picture(
            &cloud.points,
            [vp.center.re(), vp.center.im()],
            vp.scale.to_f64(),
            panel,
        );
        // A new shared buffer per picture: one a submitted frame still reads is never written.
        let buffer = picture.and_then(|p| match renderer.backend().create_buffer_with_data(&p) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("almondbread: orbit cloud upload: {e}");
                None
            }
        });
        self.cloud = Some((key, buffer));
    }

    /// Starts the π meter's guided flight to `point` (kb: decisions/controls "π meter"): the
    /// navigator's fly-to eases the view to the valley at the pinch point, then the meter
    /// slides its probe down the line. `short_side`: the view's shorter side in points.
    pub fn start_pi_flight(
        &mut self,
        renderer: &mut Renderer<B>,
        point: almondbread_core::pi_moment::PiPoint,
        short_side: f64,
        now: f64,
    ) {
        let start = renderer.viewport.clone();
        let target = point.scenic_view(1.0, short_side).with_angle(start.angle);
        renderer.navigator.cancel_flight();
        renderer.navigator.fly_to(&target, &start, now);
        self.pi
            .start_flight(point, almondbread_core::pi_moment::MAX_EPS, point.flight_eps());
        renderer.note_interaction(now);
    }

    /// Anything to draw (the host skips all overlay work otherwise): the overlay settings,
    /// the cosmic scale or the π meter.
    pub fn active(&self) -> bool {
        self.settings.any() || self.cosmic.enabled() || self.pi.enabled || self.cockpit.enabled
    }

    /// Work left (probe analysing, maps rendering, a click resolving, a π count): keep
    /// frames coming.
    pub fn busy(&self) -> bool {
        self.pi.busy()
            || (self.julia_at.is_some() && self.julia.busy())
            || self.probe.as_ref().is_some_and(|p| !p.done())
            || self.snap.is_some()
            || self
                .maps
                .as_ref()
                .is_some_and(|m| m.levels.iter().any(|l| !l.renderer.is_settled() || !l.shown))
    }

    /// Updates the probe and the maps for the current view, builds the scene and hands it to
    /// the renderer (None when there is nothing to draw). Called once per refresh, before
    /// the renderer's frame. `size`: output pixels, `ppp`: view points per pixel.
    pub fn prepare(&mut self, renderer: &mut Renderer<B>, size: (usize, usize), ppp: f64, now: f64) {
        let mut s = self.settings.clone().sanitized();
        // What the formula lacks is not drawn (F4 Julia: the orbit overlay, its dot cloud and
        // the Julia panel; the settings stay for the way back).
        let caps = almondbread_core::FormulaCapabilities::of(renderer.settings.formula);
        s.orbit &= caps.orbit_overlay;
        s.julia &= caps.julia_panel;
        if size.0 > 0 && size.1 > 0 && ppp > 0.0 {
            let short = size.0.min(size.1) as f64 * ppp;
            let rate = renderer.navigator.zoom_speed();
            self.pi.set_layout(size.0, size.1, 1.0 / ppp);
            let navigating = renderer.navigator.flight().is_some();
            if let Some(v) = self.pi.flight_view(&renderer.viewport, navigating, short, now, rate) {
                renderer.viewport = v;
                renderer.note_interaction(now);
            }
        }
        self.pi.update(&renderer.viewport);
        // The π meter's orbit trail: a new picture becomes a new texture (a few per second).
        match self.pi.trail_picture() {
            Some(img) if self.trail.as_ref().is_none_or(|(s, _)| *s != img.serial) => {
                // A new shared buffer per picture: one a submitted frame still reads is never
                // written.
                match renderer.backend().create_buffer_with_data(&img.bgra) {
                    Ok(t) => self.trail = Some((img.serial, t)),
                    Err(e) => eprintln!("almondbread: trail upload: {e}"),
                }
            }
            Some(_) => {}
            None => self.trail = None,
        }
        let empty = size.0 == 0 || size.1 == 0 || ppp <= 0.0;
        if !s.julia || empty {
            self.julia.clear();
            self.julia_at = None;
            self.julia_k = None;
        }
        if !s.any() || empty {
            self.maps = None;
            self.probe = None;
            self.probe_key = None;
            let line = renderer.progress.current().alpha > 0.0;
            if !(self.cosmic.enabled()
                || self.pi.enabled
                || self.compass.shown(now)
                || line
                || self.cockpit.enabled
                || self.marker.visible()
                || self.tour.marker.is_some())
                || empty
            {
                renderer.overlay = None;
                self.help.clear();
                return;
            }
        }
        let view = renderer.viewport.clone();
        let px_per_pt = 1.0 / ppp;
        self.cosmic.update(&view, size, ppp, now);
        // ---- Orbit probe.
        if !s.any() {
            // Only the cosmic scale or the π meter: no probe, no maps.
        } else if s.orbit {
            self.update_probe(renderer, &view, ppp, s.orbit_length);
        } else {
            self.probe = None;
            self.probe_key = None;
        }
        // ---- Maps (the orbit overlay shows the whole-set map too) and the Julia panel at the
        // end of their row. The overlay kernel binds four map textures: with the Julia panel on,
        // at most three map levels.
        let (map_slots, want_levels) = row_wants(&s);
        // Fewer or narrower panels where the platform's controls or the cockpit leave no room
        // (kb: decisions/controls "Mini-map on small screens"). The Julia panel keeps its place
        // and the deepest map levels go first (one panel's room: the Julia panel alone).
        let fit = fit_maps(
            want_levels,
            s.map_size,
            s.map_corner,
            size,
            ppp,
            renderer.navigator.insets(),
            self.cosmic.inset_right,
            self.pi.inset_top,
            &self.cockpit,
        );
        let panel = (
            ((fit.width * px_per_pt).round() as usize).clamp(32, 4096),
            ((fit.width * MAP_ASPECT * px_per_pt).round() as usize).clamp(24, 4096),
        );
        let map_fit = MapFit {
            levels: row_map_levels(fit.levels, map_slots, s.julia),
            ..fit
        };
        if map_fit.levels == 0 {
            self.maps = None;
        } else if s.any()
            && let Err(e) = self.update_maps(renderer, &view, &map_fit, panel, size, &s, px_per_pt)
        {
            eprintln!("almondbread: overlay maps: {e}");
            self.maps = None;
        }
        self.update_cloud(renderer, &s);
        if s.julia && !empty {
            let n = self.maps.as_ref().map_or(0, |m| m.levels.len());
            let origin = row_origins(
                n + 1,
                s.map_corner,
                panel,
                size,
                renderer.navigator.insets(),
                &fit,
                px_per_pt,
            )[n];
            self.julia_at = Some((origin, panel));
            // The pointer over the panel itself keeps the panel's k (F4: the click there opens
            // the set the panel shows).
            let (px, py) = renderer.navigator.probe_point();
            let (x, y) = (
                size.0 as f64 / 2.0 + px * px_per_pt,
                size.1 as f64 / 2.0 - py * px_per_pt,
            );
            let k = match &self.julia_k {
                Some(k) if self.tour.probe.is_none() && self.julia_hit(x, y) => k.clone(),
                _ => self.probe_c(renderer, &view),
            };
            self.julia_k = Some(k.clone());
            let settings = renderer.settings.clone();
            if let Err(e) = self
                .julia
                .update(renderer.backend(), (k.re(), k.im()), panel, &settings, now)
            {
                eprintln!("almondbread: julia panel: {e}");
            }
        }
        // ---- Compass: top right, left of map panels there.
        if self.compass.shown(now) {
            let left_of = self
                .maps
                .as_ref()
                .filter(|_| s.any() && s.map_corner == Corner::TopRight)
                .and_then(|m| m.panels.iter().map(|p| p[0]).reduce(f32::min))
                .map(|x0| (f64::from(x0) - size.0 as f64 / 2.0) * ppp - 8.0);
            let view_pt = (size.0 as f64 * ppp, size.1 as f64 * ppp);
            self.compass.layout(view_pt, renderer.navigator.insets(), left_of);
        }
        // ---- Scene.
        let mut scene = Scene::default();
        let mut help = Vec::new();
        let key = self.build_scene(&mut scene, &mut help, renderer, &view, size, ppp, &s, now);
        self.help = help;
        self.panels = self.collect_panels();
        if self.scene_key.as_deref() != Some(key.as_str()) || renderer.overlay.is_none() {
            self.version += 1;
            self.scene_key = Some(key);
            if self.font.is_none() {
                self.font = renderer.backend().create_buffer_with_data(&font_bytes()).ok();
            }
            let mut maps: Vec<B::Texture> = self
                .maps
                .as_ref()
                .map(|m| m.levels.iter().map(|l| l.target.clone()).collect())
                .unwrap_or_default();
            // The Julia panel's picture in the slot after the maps.
            if self.julia_at.is_some()
                && let Some(t) = self.julia.texture()
            {
                maps.push(t.clone());
            }
            let trail = self.trail.as_ref().map(|(_, t)| t.clone());
            match OverlayDraw::upload(renderer.backend(), &scene, size, maps, trail, self.version) {
                Ok(mut d) => {
                    d.cloud = self.cloud.as_ref().and_then(|(_, b)| b.clone());
                    renderer.overlay_font = self.font.clone();
                    renderer.overlay = Some(d);
                }
                Err(e) => eprintln!("almondbread: overlay upload: {e}"),
            }
        }
    }

    /// The point the orbit (and the Julia panel) shows: a tour's probe, else a touch probe or
    /// the pointer (the zoom target: the crosshair in keyboard mode or while the autopilot flies).
    fn probe_c(&self, renderer: &Renderer<B>, view: &Viewport) -> Center {
        match &self.tour.probe {
            Some(c) => c.clone(),
            None => {
                let (x, y) = renderer.navigator.probe_point();
                view.point_at_offset(x, y)
            }
        }
    }

    fn update_probe(&mut self, renderer: &Renderer<B>, view: &Viewport, ppp: f64, length: usize) {
        // A tour may set the point (kb: decisions/controls "Guided tours"); else a touch probe or
        // the pointer (kb: decisions/controls "Touch").
        let (tx, ty) = match &self.tour.probe {
            Some(c) => view.offset_of(c),
            None => renderer.navigator.probe_point(),
        };
        // Key: the view, the target to 1/64 point, the length, the reference and the formula
        // (F3: a switch at a shallow view, where no reference changes, follows the new orbit).
        let serial = renderer.reference_serial();
        let key = (
            view.clone(),
            [(tx * 64.0).round() as i64, (ty * 64.0).round() as i64],
            length,
            serial,
            renderer.settings.formula,
        );
        if self.probe_key.as_ref() == Some(&key) {
            return;
        }
        let deep = view.scale.to_f64() < 1e-12;
        let reference = if deep {
            renderer.reference().filter(|r| r.big).map(|r| {
                if self.probe_reference.as_ref().is_none_or(|(s, _)| *s != serial) {
                    self.probe_reference = Some((serial, Arc::new(ProbeReference::from_reference(r))));
                }
                (
                    r.point.clone(),
                    self.probe_reference.as_ref().expect("reference").1.clone(),
                )
            })
        } else {
            None
        };
        let c = match &self.tour.probe {
            Some(c) => c.clone(),
            None => view.point_at_offset(tx, ty),
        };
        let scale = view.scale.double_exp();
        let (point, ref_offset, centre) = match reference {
            Some((rp, r)) => {
                // dc = c - C_ref and C_ref - C_view, in units of the view scale (exact in
                // BigFloat), back to DoubleExp.
                let (dx, dy) = c.delta_in(&rp, view.scale);
                let (ox, oy) = rp.delta_in(&view.center, view.scale);
                let sc = |v: f64| DoubleExp::from_f64(v) * scale;
                (
                    ProbePoint {
                        formula: renderer.settings.formula,
                        reference: Some(r),
                        dc: almondbread_core::math::DoubleExpComplex::new(sc(dx), sc(dy)),
                    },
                    almondbread_core::math::DoubleExpComplex::new(sc(ox), sc(oy)),
                    [view.center.re(), view.center.im()],
                )
            }
            None => (
                ProbePoint {
                    formula: renderer.settings.formula,
                    reference: None,
                    dc: almondbread_core::math::DoubleExpComplex::new(
                        DoubleExp::from_f64(c.re()),
                        DoubleExp::from_f64(c.im()),
                    ),
                },
                almondbread_core::math::DoubleExpComplex::ZERO,
                [view.center.re(), view.center.im()],
            ),
        };
        let _ = ppp;
        self.points = orbit_points(&point, length, centre, ref_offset, scale);
        // Plane offsets to screen axes (a rotated view, kb: decisions/controls "View rotation").
        if view.angle != 0.0 {
            for p in &mut self.points.view {
                let (x, y) = view.unrotate(p[0], p[1]);
                *p = [x, y];
            }
        }
        // At least 10^5 steps: slow escapes near the necks are what the dot cloud is for.
        let max_iter = renderer
            .stats
            .max_iterations
            .max(renderer.max_iterations())
            .max(PROBE_MIN_STEPS);
        self.probe = Some(OrbitProbe::new(point, max_iter));
        self.probe_serial += 1;
        self.probe_key = Some(key);
    }

    /// Advances the probe for about `budget` seconds.
    pub fn step_probe(&mut self, budget: f64) {
        let Some(p) = &mut self.probe else { return };
        let start = Instant::now();
        while !p.done() && start.elapsed().as_secs_f64() < budget {
            p.step(2048);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn update_maps(
        &mut self,
        renderer: &Renderer<B>,
        view: &Viewport,
        fit: &MapFit,
        panel: (usize, usize),
        size: (usize, usize),
        s: &OverlaySettings,
        px_per_pt: f64,
    ) -> Result<(), GpuError> {
        // Level decades: 0 (whole set), then `want - 1` levels spread evenly between it and the
        // view (`map_decades`).
        let full = full_view_of(renderer.settings.formula, panel.0, panel.1);
        let view_px = view.scale; // per point; maps are per pixel
        // In logs: the f64 ratio overflows to infinity past ~1e308, and the level loop below
        // then never ended (the app hung when the map was on at such depths).
        let depth = full.scale.log10() - (view_px * (1.0 / px_per_pt)).log10();
        if !depth.is_finite() {
            return Ok(());
        }
        let prev: Vec<i32> = self
            .maps
            .as_ref()
            .map(|m| m.levels.iter().map(|l| l.decade).filter(|&d| d > 0).collect())
            .unwrap_or_default();
        let decades = map_decades(depth, fit.levels, &prev);
        let stack = self.maps.get_or_insert_with(|| MapStack {
            levels: Vec::new(),
            panels: Vec::new(),
            panel,
            spare: Vec::new(),
        });
        if stack.panel != panel {
            stack.levels.clear();
            stack.spare.clear();
            stack.panel = panel;
        }
        // A formula change (F4: a Julia set opened or left) makes every level anew.
        if stack
            .levels
            .iter()
            .any(|l| l.renderer.settings.formula != renderer.settings.formula)
        {
            stack.spare.extend(std::mem::take(&mut stack.levels));
        }
        // Drop levels no longer wanted (or whose area the view left), add new ones.
        let mut kept: Vec<MapLevel<B>> = Vec::new();
        for d in &decades {
            // In logs too: 10^-d underflows f64 past d ≈ 308.
            let scale = Scale::from_log2(full.scale.log2() - *d as f64 * std::f64::consts::LOG2_10);
            let existing = stack.levels.iter().position(|l| l.decade == *d);
            let usable = existing.is_some_and(|i| {
                let l = &stack.levels[i];
                let (dx, dy) = view.center.delta_in(&l.viewport.center, scale);
                *d == 0 || (dx.abs() < panel.0 as f64 * 0.3 && dy.abs() < panel.1 as f64 * 0.3)
            });
            if usable {
                let i = existing.expect("level");
                kept.push(stack.levels.remove(i));
                continue;
            }
            let vp = if *d == 0 {
                full.clone()
            } else {
                let mut v = Viewport::with_center(view.center.clone(), scale);
                v.ensure_precision();
                v
            };
            // A renderer from a dropped level if there is one (making one compiles pipelines:
            // a few ms, not to be paid every three decades of a zoom).
            let spare = stack
                .levels
                .iter()
                .position(|l| !decades.contains(&l.decade) || l.decade == *d);
            let (mut r, target) = match spare.map(|i| stack.levels.remove(i)).or_else(|| stack.spare.pop()) {
                Some(l) => (l.renderer, l.target),
                None => {
                    let config = SchedulerConfig {
                        live_capacity: 1 << 16,
                        max_list_capacity: 1 << 16,
                        chunk_target: 0.002,
                        interactive_chunk_target: 0.002,
                        max_chunks_in_flight: 1,
                        extension_budget: 0.05,
                        aa_budget: 0.02,
                        aa_chunk_target: 0.002,
                        ..SchedulerConfig::default()
                    };
                    let r = Renderer::with_config(renderer.backend().clone(), config)?;
                    let target = renderer.backend().create_texture(
                        panel.0,
                        panel.1,
                        TextureFormat::Bgra8Unorm,
                        Storage::Private,
                    )?;
                    (r, target)
                }
            };
            r.viewport = vp.clone();
            r.settings = map_settings(&renderer.settings);
            kept.push(MapLevel {
                renderer: r,
                target,
                viewport: vp,
                decade: *d,
                size: panel,
                shown: false,
            });
        }
        stack.spare.extend(std::mem::take(&mut stack.levels));
        stack.spare.truncate(4);
        stack.levels = kept;
        // Layout: a row from the chosen corner, the whole set first.
        let n = stack.levels.len();
        stack.panels = row_origins(
            n,
            s.map_corner,
            panel,
            size,
            renderer.navigator.insets(),
            fit,
            px_per_pt,
        );
        Ok(())
    }

    /// Steps the map renderers (call when the main view is not in a gesture): each renders
    /// its level progressively into its texture with 2 ms chunks.
    pub fn step_maps(&mut self, main: &RenderSettings, now: f64) {
        let Some(stack) = &mut self.maps else { return };
        for l in &mut stack.levels {
            let want = map_settings(main);
            if l.renderer.settings.palette_id != want.palette_id
                || l.renderer.settings.palette_stops != want.palette_stops
                || l.renderer.settings.color_density != want.color_density
            {
                l.renderer.settings = want;
            }
            if l.shown && l.renderer.is_settled() {
                continue;
            }
            let t = l.target.clone();
            let out = l
                .renderer
                .render_frame(now, l.size.0, l.size.1, 1.0, &mut |_| Some((t.clone(), None)));
            if out.presented {
                l.shown = true;
            }
        }
    }

    /// A click at `(x, y)` output pixels: on a map panel, starts resolving the point there
    /// (snapping it to a nearby minibrot when the map is much coarser than the view) and
    /// returns true; elsewhere false.
    /// A mini-map panel (not its help badge) is under (x, y) pixels: a click there jumps.
    pub fn map_hit(&self, x: f64, y: f64, ppp: f64) -> bool {
        if crate::help::hit(&self.help, x, y, 1.0 / ppp).is_some() {
            return false;
        }
        if self.julia_hit(x, y) {
            return true;
        }
        let Some(stack) = &self.maps else { return false };
        let (pw, ph) = (stack.panel.0 as f64, stack.panel.1 as f64);
        stack.panels.iter().any(|p| {
            let (px, py) = (x - f64::from(p[0]), y - f64::from(p[1]));
            (0.0..pw).contains(&px) && (0.0..ph).contains(&py)
        })
    }

    pub fn click(&mut self, renderer: &Renderer<B>, x: f64, y: f64, ppp: f64, fly: bool) -> bool {
        // The help badge in the panel's corner is not a place to jump to.
        if crate::help::hit(&self.help, x, y, 1.0 / ppp).is_some() {
            return false;
        }
        // A click on the Julia panel opens its Julia set as the main view (F4, kb:
        // plans/f4-deep-julia; the host applies it). It never zooms the view under it.
        if self.julia_hit(x, y) {
            self.open_julia = self.julia_k.clone();
            return true;
        }
        let Some(stack) = &self.maps else { return false };
        let (pw, ph) = (stack.panel.0 as f64, stack.panel.1 as f64);
        for (i, p) in stack.panels.iter().enumerate() {
            let (px, py) = (x - f64::from(p[0]), y - f64::from(p[1]));
            if !(0.0..pw).contains(&px) || !(0.0..ph).contains(&py) {
                continue;
            }
            let l = &stack.levels[i];
            let map_scale = l.viewport.scale; // per map pixel
            let c = l
                .viewport
                .center
                .offset_scaled(px + 0.5 - pw / 2.0, ph / 2.0 - py - 0.5, map_scale);
            let view_px = renderer.viewport.scale * ppp;
            // (Minibrot snapping is Mandelbrot's: a Julia map jumps to the point itself, F4.)
            let ratio = if renderer.settings.formula.is_julia() {
                1.0
            } else {
                map_scale.ratio(view_px)
            };
            let view_scale = renderer.viewport.scale;
            let max_iter = renderer.max_iterations().max(1000);
            let mut probe_view = Viewport::with_center(c.clone(), view_scale);
            probe_view.ensure_precision();
            let bits = probe_view.needed_center_bits().max(c.precision_bits());
            let formula = renderer.settings.formula;
            let handle = std::thread::spawn(move || resolve_click(formula, c, map_scale, ratio, bits, max_iter, fly));
            self.snap = Some(SnapJob { handle });
            return true;
        }
        false
    }

    /// The Julia panel is under (x, y) pixels.
    fn julia_hit(&self, x: f64, y: f64) -> bool {
        self.julia_at.is_some_and(|(o, (pw, ph))| {
            let (px, py) = (x - f64::from(o[0]), y - f64::from(o[1]));
            (0.0..pw as f64).contains(&px) && (0.0..ph as f64).contains(&py)
        })
    }

    /// Collects a resolved click (the host applies `pending_jump`).
    pub fn poll_click(&mut self, now: f64) {
        if self.snap.as_ref().is_some_and(|s| s.handle.is_finished())
            && let Some(job) = self.snap.take()
            && let Ok(j) = job.handle.join()
        {
            self.note = Some((j.note.clone(), now));
            self.pending_jump = Some(j);
        }
    }

    /// The Julia panel at `o` (top-left pixel, `size` pixels; `px` pixels per point): a frame
    /// like the maps', the picture, its title, the value of k and a help badge.
    fn draw_julia(
        &self,
        scene: &mut Scene,
        help: &mut Vec<crate::help::HelpBadge>,
        o: [f32; 2],
        size: (f32, f32),
        px: f32,
        key: &mut String,
    ) {
        let r = [o[0], o[1], o[0] + size.0, o[1] + size.1];
        let radius = 8.0 * px;
        let pad = 3.0 * px;
        scene.set_clip(None);
        scene.rect(
            [r[0] - pad, r[1] - pad, r[2] + pad, r[3] + pad],
            radius + pad,
            rgba(0.02, 0.02, 0.03, 1.0),
            rgba(1.0, 1.0, 1.0, 0.22),
            px,
        );
        // The picture is the slot after the map levels.
        let slot = self.maps.as_ref().map_or(0, |m| m.levels.len()) as u32;
        if self.julia.shown && self.julia.texture().is_some() {
            scene.image(r, slot, 1.0, radius);
        }
        scene.set_clip(Some(r));
        let text = (11.0 * px).max(9.0);
        scene.label(
            [o[0] + 0.7 * text, o[1] + 0.7 * text],
            text,
            "Julia set",
            rgba(1.0, 1.0, 1.0, 0.85),
        );
        if let Some(k) = self.julia.k {
            let small = (10.0 * px).max(8.5);
            let value = format!("c = {}", complex_text(k));
            let cap = crate::font::cap_height() * small;
            scene.label(
                [o[0] + 0.7 * text, r[3] - 0.7 * text - cap],
                small,
                &value,
                rgba(1.0, 1.0, 1.0, 0.8),
            );
            key.push_str(&value);
        }
        scene.set_clip(None);
        let c = crate::help::top_right(r, px);
        crate::help::draw(scene, help, crate::help::HelpTopic::Julia, c, px, key);
        key.push_str(&format!("julia {} {} {:?} ", self.julia.serial, slot, o));
    }

    /// The mini-map panels as drawn in the last scene: each panel's rect (pixels: x0, y0, x1,
    /// y1) and its level's depth in decades below the whole set (0: the whole set). Empty while
    /// no map shows (accessibility elements, kb: plans/a11y-canvas-controls).
    pub fn map_panels(&self) -> Vec<([f32; 4], i32)> {
        let (Some(_), Some(stack)) = (self.drawn_maps.get(), &self.maps) else {
            return Vec::new();
        };
        let (pw, ph) = (stack.panel.0 as f32, stack.panel.1 as f32);
        stack
            .panels
            .iter()
            .zip(&stack.levels)
            .map(|(o, l)| ([o[0], o[1], o[0] + pw, o[1] + ph], l.decade))
            .collect()
    }

    /// The panels of the scene just built ([`Self::panels`]).
    fn collect_panels(&self) -> Vec<(&'static str, [f32; 4])> {
        let [pi, trail] = self.pi.drawn();
        [
            ("cosmic", self.drawn_cosmic.get()),
            ("cockpit", self.drawn_cockpit.get()),
            ("maps", self.drawn_maps.get()),
            ("julia", self.drawn_julia.get()),
            ("pi", pi),
            ("pi-trail", trail),
        ]
        .into_iter()
        .filter_map(|(id, r)| r.map(|r| (id, r)))
        .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn build_scene(
        &self,
        scene: &mut Scene,
        help: &mut Vec<crate::help::HelpBadge>,
        renderer: &Renderer<B>,
        view: &Viewport,
        size: (usize, usize),
        ppp: f64,
        s: &OverlaySettings,
        now: f64,
    ) -> String {
        let px = 1.0 / ppp; // pixels per point
        let (w, h) = (size.0 as f64, size.1 as f64);
        let mut key = format!("{w}x{h} {px} ");
        // The cosmic scale sits bottom left, above the maps when they are there too.
        let mut cosmic_bottom = (h - 12.0 * px) as f32;
        let to_px = |v: [f64; 2]| [w / 2.0 + v[0] * px, h / 2.0 - v[1] * px];
        // ---- A tour's marker, under the orbit.
        self.tour.draw(scene, size, ppp, view, &mut key);
        // ---- Orbit in the view.
        if s.orbit && !self.points.view.is_empty() {
            let pts: Vec<[f64; 2]> = self.points.view.iter().map(|&v| to_px(v)).collect();
            let n = pts.len();
            let fade = |k: usize| 1.0 - 0.8 * k as f32 / n.max(2) as f32;
            let col = |k: usize, a: f32| {
                let t = k as f32 / n.max(2) as f32;
                let c: Vec<f32> = (0..3).map(|i| ORBIT_WARM[i] * (1.0 - t) + ORBIT_COOL[i] * t).collect();
                rgba(c[0], c[1], c[2], a * fade(k))
            };
            let lw = (0.75 * px) as f32;
            for k in 0..n.saturating_sub(1) {
                scene.segment(
                    pts[k],
                    pts[k + 1],
                    lw + px as f32,
                    rgba(0.0, 0.0, 0.0, 0.45 * fade(k)),
                    rgba(0.0, 0.0, 0.0, 0.45 * fade(k + 1)),
                );
                scene.segment(pts[k], pts[k + 1], lw, col(k, 0.9), col(k + 1, 0.9));
            }
            for (k, p) in pts.iter().enumerate().skip(1) {
                scene.disc(*p, (2.6 * px) as f32, rgba(0.0, 0.0, 0.0, 0.5 * fade(k)));
                scene.disc(*p, (1.8 * px) as f32, col(k, 1.0));
            }
            // c itself: a larger dot with a ring.
            scene.disc(pts[0], (5.0 * px) as f32, rgba(0.0, 0.0, 0.0, 0.55));
            scene.disc(
                pts[0],
                (3.4 * px) as f32,
                rgba(ORBIT_WARM[0], ORBIT_WARM[1], ORBIT_WARM[2], 1.0),
            );
            scene.ring(pts[0], (6.5 * px) as f32, (1.2 * px) as f32, rgba(1.0, 1.0, 1.0, 0.8));
            key.push_str(&format!("orbit {:?} ", self.points.view.first()));
            key.push_str(&format!("{} {:?} ", n, self.points.view.last()));
        }
        // ---- Maps.
        if let Some(stack) = &self.maps {
            let (pw, ph) = (stack.panel.0 as f32, stack.panel.1 as f32);
            let radius = (8.0 * px) as f32;
            for (i, l) in stack.levels.iter().enumerate() {
                let o = stack.panels[i];
                let r = [o[0], o[1], o[0] + pw, o[1] + ph];
                let pad = (3.0 * px) as f32;
                scene.set_clip(None);
                scene.rect(
                    [r[0] - pad, r[1] - pad, r[2] + pad, r[3] + pad],
                    radius + pad,
                    // Opaque: the view must not show through a map (it read as part of it).
                    rgba(0.02, 0.02, 0.03, 1.0),
                    rgba(1.0, 1.0, 1.0, 0.22),
                    (1.0 * px) as f32,
                );
                if l.shown {
                    scene.image(r, i as u32, 1.0, radius);
                }
                scene.set_clip(Some(r));
                // The orbit's dot cloud on the whole-set map, under the marker and the line.
                if i == 0
                    && s.orbit
                    && let Some((k, Some(_))) = &self.cloud
                {
                    scene.cloud_image(r, stack.panel, 1.0);
                    key.push_str(&format!("cloud {} {} ", k.0, k.1));
                }
                // Marker: the next level's area, or the view's on the deepest map.
                let (mc, mscale) = match stack.levels.get(i + 1) {
                    Some(next) => (next.viewport.center.clone(), next.viewport.scale * stack.panel.0 as f64),
                    None => (view.center.clone(), view.scale * (w * ppp)),
                };
                let (dx, dy) = mc.delta_in(&l.viewport.center, l.viewport.scale);
                let cx = f64::from(o[0]) + f64::from(pw) / 2.0 + dx;
                let cy = f64::from(o[1]) + f64::from(ph) / 2.0 - dy;
                let mw = mscale.ratio(l.viewport.scale);
                let last = stack.levels.get(i + 1).is_none();
                let mh = mw * if last { h / w } else { f64::from(ph / pw) };
                let marker = rgba(1.0, 0.85, 0.3, 0.95);
                if mw >= 6.0 * px {
                    // The view's rectangle, turned with the view (maps stay north-up).
                    let (c, sn) = if last { view.rotation() } else { (1.0, 0.0) };
                    let corner = |u: f64, v: f64| {
                        let (x, y) = (u * mw / 2.0, v * mh / 2.0);
                        [cx + x * c - y * sn, cy - (x * sn + y * c)]
                    };
                    let (p00, p10, p11, p01) = (
                        corner(-1.0, 1.0),
                        corner(1.0, 1.0),
                        corner(1.0, -1.0),
                        corner(-1.0, -1.0),
                    );
                    let hw = (0.7 * px) as f32;
                    for (a, b) in [(p00, p10), (p10, p11), (p11, p01), (p01, p00)] {
                        scene.segment(
                            a,
                            b,
                            hw + px as f32 * 0.8,
                            rgba(0.0, 0.0, 0.0, 0.5),
                            rgba(0.0, 0.0, 0.0, 0.5),
                        );
                        scene.segment(a, b, hw, marker, marker);
                    }
                } else {
                    // Sub-pixel at this scale: a crosshair.
                    let a = 7.0 * px;
                    let g = 2.0 * px;
                    let hw = (0.7 * px) as f32;
                    for (p0, p1) in [
                        ([cx - a, cy], [cx - g, cy]),
                        ([cx + g, cy], [cx + a, cy]),
                        ([cx, cy - a], [cx, cy - g]),
                        ([cx, cy + g], [cx, cy + a]),
                    ] {
                        scene.segment(
                            p0,
                            p1,
                            hw + px as f32 * 0.8,
                            rgba(0.0, 0.0, 0.0, 0.5),
                            rgba(0.0, 0.0, 0.0, 0.5),
                        );
                        scene.segment(p0, p1, hw, marker, marker);
                    }
                    scene.disc([cx, cy], (1.3 * px) as f32, marker);
                }
                key.push_str(&format!("map {} {} {cx:.2} {cy:.2} {mw:.2} ", l.decade, l.shown));
                // The orbit at full-set scale on the whole-set map.
                if i == 0 && s.orbit && !self.points.plane.is_empty() {
                    let to_map = |z: [f64; 2]| {
                        let (ddx, ddy) = (
                            (z[0] - l.viewport.center.re()) / l.viewport.scale.to_f64(),
                            (z[1] - l.viewport.center.im()) / l.viewport.scale.to_f64(),
                        );
                        [
                            f64::from(o[0]) + f64::from(pw) / 2.0 + ddx,
                            f64::from(o[1]) + f64::from(ph) / 2.0 - ddy,
                        ]
                    };
                    // The start as a line (the rest is the cloud).
                    let mp: Vec<[f64; 2]> = self
                        .points
                        .plane
                        .iter()
                        .take(MAP_LINE_POINTS)
                        .map(|&z| to_map(z))
                        .collect();
                    let n = mp.len();
                    for k in 0..n.saturating_sub(1) {
                        let f = 1.0 - k as f32 / n.max(2) as f32;
                        let a = 0.05 + 0.65 * f * f;
                        scene.segment(
                            mp[k],
                            mp[k + 1],
                            (0.3 * px) as f32,
                            rgba(ORBIT_COOL[0], ORBIT_COOL[1], ORBIT_COOL[2], a),
                            rgba(ORBIT_COOL[0], ORBIT_COOL[1], ORBIT_COOL[2], a),
                        );
                    }
                    // (Fading fast: near a neck the first steps zigzag across the map, and a
                    // web of 50 equal lines hid the cloud.)
                    for (k, p) in mp.iter().enumerate().skip(1) {
                        let f = 1.0 - k as f32 / n.max(2) as f32;
                        scene.disc(*p, (0.9 * px) as f32, rgba(1.0, 1.0, 1.0, 0.2 + 0.6 * f));
                    }
                    scene.disc(
                        mp[0],
                        (2.2 * px) as f32,
                        rgba(ORBIT_WARM[0], ORBIT_WARM[1], ORBIT_WARM[2], 1.0),
                    );
                }
                // Level label: its magnification.
                let size = (11.0 * px).max(9.0) as f32;
                let label = if l.decade == 0 {
                    "whole set".to_string()
                } else {
                    format!("×1e{}", l.decade)
                };
                scene.label(
                    [o[0] + 0.7 * size, o[1] + 0.7 * size],
                    size,
                    &label,
                    rgba(1.0, 1.0, 1.0, 0.85),
                );
            }
            scene.set_clip(None);
            // Help badge in the whole-set panel's top-right corner (its emptiest part).
            if let Some(o) = stack.panels.first() {
                let topic = if s.map {
                    crate::help::HelpTopic::MiniMap
                } else {
                    crate::help::HelpTopic::Orbit
                };
                let c = crate::help::top_right([o[0], o[1], o[0] + pw, o[1] + ph], px as f32);
                crate::help::draw(scene, help, topic, c, px as f32, &mut key);
            }
        }
        // ---- The Julia panel at the end of the row.
        if let Some((o, (jw, jh))) = self.julia_at {
            self.draw_julia(scene, help, o, (jw as f32, jh as f32), px as f32, &mut key);
        }
        // ---- Info under (or above) the row's first panel: the orbit's fate and a click note.
        let first = self
            .maps
            .as_ref()
            .and_then(|m| m.panels.first().map(|o| (*o, m.panel.1 as f32)))
            .or(self.julia_at.map(|(o, (_, jh))| (o, jh as f32)));
        if let Some((o0, ph)) = first {
            let size = (13.0 * px).max(11.0) as f32;
            let line = 1.35 * size;
            let below = matches!(s.map_corner, Corner::TopLeft | Corner::TopRight);
            let mut lines: Vec<String> = Vec::new();
            if s.orbit
                && let Some(p) = &self.probe
            {
                lines.push(p.text());
            }
            if let Some((note, t)) = &self.note
                && now - t < 4.0
            {
                lines.push(note.clone());
            }
            let cap = crate::font::cap_height() * size;
            let y0 = if below {
                o0[1] + ph + 0.8 * size
            } else {
                o0[1] - 0.8 * size - cap - line * lines.len().saturating_sub(1) as f32
            };
            for (k, t) in lines.iter().enumerate() {
                scene.label([o0[0], y0 + k as f32 * line], size, t, rgba(1.0, 1.0, 1.0, 0.92));
                key.push_str(t);
            }
            if s.map_corner == Corner::BottomLeft {
                cosmic_bottom = if lines.is_empty() { o0[1] - 3.0 * px as f32 } else { y0 } - 14.0 * px as f32;
            }
        } else if s.orbit
            && let Some(p) = &self.probe
        {
            let size = (13.0 * px).max(11.0) as f32;
            let t = p.text();
            scene.label(
                [(12.0 * px) as f32, (h - 24.0 * px) as f32],
                size,
                &t,
                rgba(1.0, 1.0, 1.0, 0.92),
            );
            key.push_str(&t);
        }
        // The cockpit at the bottom, right of the cosmic scale (bottom left) where it fits;
        // where it does not (a phone, a narrow window), the scale stacks above the cockpit
        // (kb: decisions/controls "Cosmic size scale"). Right of a bottom-left map row when it
        // fits there, else above it (a phone in portrait: no map covers a 44-point cell; kb:
        // decisions/controls "Touch").
        let insets = renderer.navigator.insets();
        // The map panels and the Julia panel as drawn (pixels: x0, y0, x1, y1), whatever their
        // corner; the row is both.
        let union = |r: Option<[f32; 4]>, q: [f32; 4]| {
            Some(r.map_or(q, |r| [r[0].min(q[0]), r[1].min(q[1]), r[2].max(q[2]), r[3].max(q[3])]))
        };
        let maps_drawn = self.maps.as_ref().filter(|m| !m.panels.is_empty()).and_then(|stack| {
            let (pw, ph) = (stack.panel.0 as f32, stack.panel.1 as f32);
            stack
                .panels
                .iter()
                .fold(None, |r, o| union(r, [o[0], o[1], o[0] + pw, o[1] + ph]))
        });
        let julia_drawn = self
            .julia_at
            .map(|(o, (jw, jh))| [o[0], o[1], o[0] + jw as f32, o[1] + jh as f32]);
        let map_row = match (maps_drawn, julia_drawn) {
            (r, Some(j)) => union(r, j),
            (r, None) => r,
        };
        self.drawn_maps.set(maps_drawn);
        self.drawn_julia.set(julia_drawn);
        // The row's panel height (pixels): maps and the Julia panel share it.
        let row_panel_h = first.map(|(_, ph)| ph);
        // The bottom-left map row, in points (its right edge from the canvas's left edge, its
        // height with the margin and the info lines above it), for the cockpit.
        let maps_bottom_left = row_panel_h
            .zip(map_row)
            .filter(|_| s.map_corner == Corner::BottomLeft)
            .map(|(ph, r)| {
                let block = f64::from(ph) / px + MAP_MARGIN + MAP_INFO;
                (f64::from(r[2]) / px, block)
            });
        let cosmic_w = self.cosmic.enabled().then(|| self.cosmic.width_pt(w / px));
        let mut left = cosmic_w.map_or(0.0, |cw| 12.0 + cw + 24.0);
        self.cockpit.lift.set(0.0);
        // (The Mac too: a narrow window, the control center over the canvas.)
        let maps_across = maps_bottom_left;
        let mut stack_cosmic = false;
        if self.cockpit.enabled {
            let beside = f64::max(left, maps_across.map_or(0.0, |m| m.0));
            if self.cockpit.fits(size, ppp, insets, beside) {
                left = beside;
            } else {
                stack_cosmic = cosmic_w.is_some();
                left = 0.0;
                if let Some((across, high)) = maps_across {
                    if self.cockpit.fits(size, ppp, insets, across) {
                        left = across;
                    } else {
                        self.cockpit.lift.set(high);
                    }
                }
            }
        }
        let cockpit_rect = self.cockpit.enabled.then(|| {
            let r = self.cockpit.layout(size, ppp, insets, left);
            [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32]
        });
        if stack_cosmic && let Some(c) = cockpit_rect {
            cosmic_bottom = cosmic_bottom.min(c[1] - (10.0 * px) as f32);
        }
        let mut cosmic_rect = None;
        if self.cosmic.enabled() {
            let mut corner = [(12.0 * px) as f32, cosmic_bottom];
            // A short canvas (a phone on its side): stacked over the maps and the cockpit the
            // scale would climb into the upper half, under the HUD; it goes to the top right
            // instead, left of the trailing controls (the HUD sits top left).
            if stack_cosmic && w > h {
                let measured = self.cosmic.draw(
                    &mut Scene::default(),
                    &mut Vec::new(),
                    size,
                    ppp,
                    corner,
                    now,
                    &mut String::new(),
                );
                if let Some(r) = measured
                    && f64::from(r[1]) < 0.45 * h
                {
                    let ph = r[3] - r[1];
                    let right = w - (12.0 + self.cosmic.inset_right + insets.3.max(0.0)) * px;
                    corner = [
                        right as f32 - (r[2] - r[0]),
                        ((12.0 + insets.0.max(0.0)) * px) as f32 + ph,
                    ];
                }
            }
            cosmic_rect = self.cosmic.draw(scene, help, size, ppp, corner, now, &mut key);
        }
        // The π meter keeps clear of the maps in the right corners, of what is at the bottom
        // left (maps, the cosmic scale), of the cockpit and of a tour card (kb:
        // decisions/controls "π meter on narrow canvases").
        let mut reserve = crate::pi_meter::Reserve::default();
        if let Some(ph) = row_panel_h
            && let Some(row) = map_row
        {
            // (Plus the info lines next to the first panel.)
            let block = ph + (MAP_MARGIN * px) as f32 + (MAP_INFO * px) as f32;
            match s.map_corner {
                Corner::BottomRight => reserve.bottom_right = block,
                Corner::TopRight => reserve.top_right = block,
                Corner::BottomLeft => {
                    reserve.bottom_left = row[2];
                    reserve.keep_clear([0.0, h as f32 - block, row[2], h as f32]);
                }
                Corner::TopLeft => {}
            }
        }
        if let Some(r) = cosmic_rect {
            reserve.bottom_left = reserve.bottom_left.max(r[2] + (12.0 * px) as f32);
            reserve.keep_clear([r[0], r[1] - (22.0 * px) as f32, r[2], r[3]]);
        }
        if let Some(c) = cockpit_rect {
            reserve.keep_clear(c);
        }
        self.drawn_cosmic.set(cosmic_rect);
        self.drawn_cockpit.set(cockpit_rect);
        if let Some([x, y, cw, ch]) = self.tour_card {
            reserve.keep_clear([
                (x * px) as f32,
                (y * px) as f32,
                ((x + cw) * px) as f32,
                ((y + ch) * px) as f32,
            ]);
        }
        // (Below the platform's top controls and the safe area: the Mac's toolbar.)
        reserve.top = (self.pi.inset_top.max(insets.0).max(0.0) * px) as f32;
        reserve.edge = [(insets.3.max(0.0) * px) as f32, (insets.2.max(0.0) * px) as f32];
        if self.pi.enabled {
            key.push_str(&format!("reserve {reserve:?} "));
        }
        key.push_str(&self.pi.draw(scene, help, w, h, px, reserve));
        // The target marker under the panels (it sits on the picture).
        self.marker.draw(scene, size, ppp, view, now, &mut key);
        self.compass.draw(scene, size, ppp, view.angle, now, &mut key);
        self.cockpit.draw(
            scene,
            help,
            size,
            ppp,
            insets,
            left,
            &renderer.navigator.cockpit(now),
            now,
            &mut key,
        );
        // The render progress line along the top edge (kb: decisions/controls "Render progress").
        crate::progress::draw_line(
            scene,
            size,
            ppp,
            renderer.navigator.insets(),
            renderer.progress.current(),
            &mut key,
        );
        key
    }
}

/// The map renderers' settings: the main look without anti-aliasing passes beyond 4 samples,
/// no interior analysis (cheap), no colour cycling.
fn map_settings(main: &RenderSettings) -> RenderSettings {
    let mut s = main.clone();
    s.supersampling = Supersampling::X4;
    s.interior_mode = InteriorMode::Flat;
    s.cycle_speed = 0.0;
    s.depth_tint = 0.0;
    s.interactive_preview = false;
    s.max_iterations_cap = s.max_iterations_cap.min(50_000);
    s
}

/// Resolves a map click (on a thread). The map pixel defines c only to `map_scale`; where
/// that is much coarser than the view (`ratio` view pixels per map pixel > 4) a jump to the
/// exact point would land somewhere arbitrary within the pixel, usually in smooth exterior.
/// Instead it looks for the lowest-period minibrot touching the pixel (ball period
/// detection, Newton to its nucleus at the view's precision) and jumps to the minibrot's
/// cusp (nucleus + size/4 along its orientation), a boundary point with structure at every
/// depth. No minibrot found: the pixel's centre.
/// (F3: in the current formula's set; a Multibrot minibrot's cusp on its real axis at
/// `w = (d-1)/d d^(-1/(d-1))`, 1/4 for Mandelbrot.)
fn resolve_click(
    formula: almondbread_core::Formula,
    c: Center,
    map_scale: Scale,
    ratio: f64,
    bits: usize,
    max_iter: usize,
    fly: bool,
) -> MapJump {
    if ratio <= 4.0 {
        return MapJump {
            center: c,
            note: "jumped to the clicked point".into(),
            fly,
        };
    }
    let big = c.with_min_precision(bits);
    let radius = map_scale.big_times(1.0, 64);
    let period = find_period_for(
        formula,
        big.big(),
        &radius,
        max_iter.min(200_000),
        PeriodMethod::Ball,
        1024,
        &mut || false,
    );
    let Some(p) = period else {
        return MapJump {
            center: c,
            note: "no minibrot in the clicked pixel: jumped to its centre".into(),
            fly,
        };
    };
    let opts = NewtonOptions {
        precision_bits: Some(bits),
        max_steps: 64,
        max_distance: Some(map_scale.big_times(4.0, 64)),
    };
    match find_nucleus_for(formula, big.big(), p, &opts, &mut || false) {
        Some(n) if !formula.is_conformal() => {
            // F5: a non-conformal minibrot is a skewed copy with no closed-form cusp: the boundary
            // point along +re from the nucleus, by bisection on escape (within 64 periods) to
            // 2^-40 of its size.
            let size = almondbread_core::math::minibrot_size_complex_for(formula, &n.nucleus, p)
                .magnitude()
                .to_f64();
            let nucleus = Center::from_big(n.nucleus);
            let escapes = |t: f64| {
                let c = nucleus.offset(t * size, 0.0).with_min_precision(bits);
                let comp = almondbread_core::math::ReferenceOrbitComputer::new(c.big(), bits).with_formula(formula);
                let r = almondbread_core::math::with_no_control(4096, |ctl| {
                    comp.compute(64 * p, formula.bailout(), None, None, ctl)
                });
                r.escaped
            };
            let (mut lo, mut hi) = (0.0f64, 4.0f64);
            if size > 0.0 && size.is_finite() && escapes(hi) {
                for _ in 0..42 {
                    let mid = 0.5 * (lo + hi);
                    if escapes(mid) { hi = mid } else { lo = mid }
                }
            }
            MapJump {
                center: nucleus.offset(hi * size, 0.0),
                note: format!("snapped to the edge of the period-{p} minibrot near the click"),
                fly,
            }
        }
        Some(n) => {
            let size = almondbread_core::math::minibrot_size_complex_for(formula, &n.nucleus, p);
            let d = f64::from(formula.power());
            let quarter = DoubleExp::from_f64((d - 1.0) / d * d.powf(-1.0 / (d - 1.0)));
            let (sx, sy) = ((size.re * quarter).to_f64(), (size.im * quarter).to_f64());
            let nucleus = Center::from_big(n.nucleus);
            let cusp = nucleus.offset(sx, sy);
            MapJump {
                center: cusp,
                note: format!("snapped to the period-{p} minibrot's cusp near the click"),
                fly,
            }
        }
        None => MapJump {
            center: c,
            note: format!("period {p} found, Newton failed: jumped to the pixel"),
            fly,
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn map_levels_spread_evenly() {
        use super::map_decades;
        assert_eq!(map_decades(1.0, 3, &[]), vec![0]);
        assert_eq!(map_decades(20.0, 3, &[]), vec![0, 6, 12]);
        assert_eq!(map_decades(1000.0, 3, &[]), vec![0, 333, 666]);
        assert_eq!(map_decades(300.0, 4, &[]), vec![0, 75, 150, 225]);
        // Hysteresis: zooming from 20 to 22 decades keeps the levels.
        assert_eq!(map_decades(22.0, 3, &[6, 12]), vec![0, 6, 12]);
        // Shallow: levels never closer to the view than 1.5 decades, never repeated.
        assert_eq!(map_decades(5.0, 3, &[]), vec![0, 3]);
        assert_eq!(map_decades(4.0, 3, &[]), vec![0]);
    }

    use super::*;

    #[test]
    fn binning_keeps_draw_order_and_walks_segments() {
        let mut s = Scene::default();
        s.segment(
            [0.0, 0.0],
            [319.0, 0.0],
            1.0,
            rgba(1.0, 1.0, 1.0, 1.0),
            rgba(1.0, 1.0, 1.0, 1.0),
        );
        s.disc([8.0, 8.0], 3.0, rgba(1.0, 0.0, 0.0, 1.0));
        s.segment(
            [0.0, 0.0],
            [319.0, 199.0],
            1.0,
            rgba(1.0, 1.0, 1.0, 1.0),
            rgba(1.0, 1.0, 1.0, 1.0),
        );
        let (start, list, active) = s.bin(320, 200);
        assert_eq!(start.len(), 20 * 13 + 1);
        // Tile 0 has all three, in order.
        assert_eq!(&list[start[0] as usize..start[1] as usize], &[0, 1, 2]);
        // The diagonal touches a band of tiles, far fewer than its bounding box.
        assert!(active.len() < 20 * 13 / 2, "{}", active.len());
        // Off-screen segments add nothing.
        let mut t = Scene::default();
        t.segment(
            [-500.0, -500.0],
            [-400.0, -300.0],
            1.0,
            rgba(1.0, 1.0, 1.0, 1.0),
            rgba(1.0, 1.0, 1.0, 1.0),
        );
        assert!(t.bin(320, 200).2.is_empty());
    }

    #[test]
    fn font_covers_the_overlay_texts() {
        for t in [
            "escapes after 48 213 steps",
            "settles into a period-34 cycle (period not found)",
            "still undecided after 100 000 steps; following the orbit: 12 000 steps so far ...",
            "×1e6 whole set",
        ] {
            for ch in t.chars().filter(|c| !c.is_whitespace()) {
                let g = crate::font::glyph(ch).unwrap();
                assert!(ch == '?' || g != crate::font::glyph('?').unwrap(), "{ch}");
            }
        }
        assert_eq!(
            font_bytes().len(),
            (crate::font::ATLAS_W * crate::font::ATLAS_H) as usize
        );
    }

    #[test]
    fn cloud_picture_places_dots_with_rims() {
        // A 20 x 10 map, 0.1 per pixel, centred on 0: point (0.05, 0.05) is pixel (10, 4).
        let pts = [[0.05f32, 0.05], [5.0, 5.0], [0.05, 0.05], [-0.95, -0.45]];
        let img = orbit_cloud_picture(&pts, [0.0, 0.0], 0.1, (20, 10)).expect("picture");
        assert_eq!(img.len(), 20 * 10 * 4);
        let px = |x: usize, y: usize| &img[(y * 20 + x) * 4..(y * 20 + x) * 4 + 4];
        let (a, b) = (px(10, 4), px(0, 9));
        // Two visits, the newer at 2/3 of the orbit: opaque, between cool and white; the newest
        // point is white.
        assert!(a[3] > 100 && a[2] < 255, "{a:?}");
        assert!(b[3] > 100 && b[0] == 255 && b[1] == 255 && b[2] == 255, "{b:?}");
        // A dark rim around a dot, nothing far away; the point off the map is dropped.
        assert_eq!(&px(11, 4)[..3], &[0, 0, 0]);
        assert!(px(11, 4)[3] > 0 && px(9, 3)[3] > 0);
        assert_eq!(px(15, 2)[3], 0);
        assert!(orbit_cloud_picture(&[[9.0, 9.0]], [0.0, 0.0], 0.1, (20, 10)).is_none());
        assert!(orbit_cloud_picture(&[], [0.0, 0.0], 0.1, (20, 10)).is_none());
    }

    /// A full cloud (10^5 points, a ring like the orbit at a golden-mean point of the main
    /// cardioid's edge) on a Retina-sized whole-set panel (180 x 130 points at 2 px): one
    /// rebuild, which happens at most ~45 times per probe, never every frame.
    #[test]
    fn cloud_picture_of_1e5_points_is_cheap() {
        let pts: Vec<[f32; 2]> = (0..100_000)
            .map(|k| {
                let a = k as f32 * 2.399_963;
                let r = 0.3 + 0.05 * (k as f32 * 0.001).sin();
                [-0.1 + r * a.cos(), 0.5 + r * a.sin()]
            })
            .collect();
        let size = (360, 259);
        let view = full_set_view(size.0, size.1);
        let centre = [view.center.re(), view.center.im()];
        let scale = view.scale.to_f64();
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = Instant::now();
            std::hint::black_box(orbit_cloud_picture(&pts, centre, scale, size));
            best = best.min(t.elapsed().as_secs_f64());
        }
        eprintln!("orbit cloud picture, 1e5 points, 360x259: {:.3} ms", best * 1000.0);
        if !cfg!(debug_assertions) {
            assert!(best < 0.005, "{best}");
        }
    }

    #[test]
    fn labels_are_one_prim_per_glyph() {
        let mut s = Scene::default();
        let w = s.label([10.0, 10.0], 26.0, "a b", rgba(1.0, 1.0, 1.0, 1.0));
        assert_eq!(s.prims.len(), 2);
        assert!(w > 26.0 && w < 60.0, "{w}");
        // The halo width is limited to what the atlas stores.
        assert!(f32::from_bits(s.prims[0].info[2]) <= crate::font::SPREAD * 26.0 / crate::font::EM);
    }

    /// The map row keeps clear of the touch controls and leaves the cockpit a place below the
    /// HUD's half (kb: decisions/controls "Mini-map on small screens"); points as on the
    /// shared simulators (iPhone 17e, iPad mini) and a Mac window.
    #[test]
    fn map_row_fits_beside_the_controls_and_the_cockpit() {
        let touch = |inset_right: f64| {
            let mut c = crate::cockpit::CockpitOverlay::default();
            c.enabled = true;
            c.touch = true;
            c.inset_right = inset_right;
            c
        };
        // (size points, ppp, safe insets, trailing, top controls, cockpit inset, levels wanted,
        // expected levels, expected width)
        let cases = [
            // iPhone portrait: two narrowed panels left of the button column, cockpit lifted.
            (
                (390.0, 844.0),
                1.0 / 3.0,
                (47.0, 0.0, 34.0, 0.0),
                56.0,
                0.0,
                56.0,
                2,
                2,
                151.0,
            ),
            (
                (390.0, 844.0),
                1.0 / 3.0,
                (47.0, 0.0, 34.0, 0.0),
                56.0,
                0.0,
                56.0,
                3,
                2,
                151.0,
            ),
            // iPhone on its side: two maps would lift the cockpit into the HUD; one map beside
            // the two-row cockpit.
            (
                (844.0, 390.0),
                1.0 / 3.0,
                (0.0, 47.0, 21.0, 47.0),
                60.0,
                0.0,
                272.0,
                2,
                1,
                180.0,
            ),
            // iPad mini, both orientations: unchanged (three full panels).
            (
                (744.0, 1133.0),
                0.5,
                (24.0, 0.0, 20.0, 0.0),
                60.0,
                56.0,
                0.0,
                3,
                3,
                180.0,
            ),
            (
                (1133.0, 744.0),
                0.5,
                (24.0, 0.0, 20.0, 0.0),
                60.0,
                56.0,
                0.0,
                3,
                3,
                180.0,
            ),
        ];
        for (pt, ppp, insets, trailing, top, cockpit_inset, want, levels, width) in cases {
            let size = ((pt.0 / ppp) as usize, (pt.1 / ppp) as usize);
            let cockpit = touch(cockpit_inset);
            let f = fit_maps(
                want,
                180.0,
                Corner::BottomLeft,
                size,
                ppp,
                insets,
                trailing,
                top,
                &cockpit,
            );
            assert_eq!(f.levels, levels, "{pt:?} want {want}: {f:?}");
            assert!((f.width - width).abs() < 0.5, "{pt:?} want {want}: {f:?}");
            // The row ends left of the trailing controls.
            let row_right = insets.1 + 12.0 + f.levels as f64 * (f.width + 8.0) - 8.0;
            assert!(
                row_right <= pt.0 - insets.3 - trailing.max(cockpit_inset) - 12.0 + 1e-9,
                "{pt:?}: {f:?}"
            );
            // The cockpit beside the row, or lifted above it into the lower half only.
            if !cockpit.fits(size, ppp, insets, row_right) {
                let lift = f.width * MAP_ASPECT + MAP_MARGIN + MAP_INFO;
                let r = cockpit.layout_lifted(size, ppp, insets, 0.0, lift);
                assert!(r[1] * ppp >= pt.1 / 2.0, "{pt:?}: lifted cockpit at {}", r[1] * ppp);
            }
        }
        // Without the cockpit (HUD off) the row only keeps clear of the controls.
        let mut off = crate::cockpit::CockpitOverlay::default();
        off.inset_right = 272.0;
        let f = fit_maps(
            2,
            180.0,
            Corner::BottomLeft,
            (2532, 1170),
            1.0 / 3.0,
            (0.0, 47.0, 21.0, 47.0),
            60.0,
            0.0,
            &off,
        );
        assert_eq!(f.levels, 2);
        // A top-right row stays below the iPad's top-trailing buttons and left of the throttle.
        let f = fit_maps(
            2,
            180.0,
            Corner::TopRight,
            (1488, 2266),
            0.5,
            (24.0, 0.0, 20.0, 0.0),
            60.0,
            56.0,
            &off,
        );
        assert_eq!((f.right, f.top), (60.0, 56.0));
    }

    /// The Julia panel takes the last place of the map row (kb: plans/f1-julia-view): the
    /// orbit's whole-set map, three map levels and the panel, on the shared simulators' screens
    /// and a Mac window. Where the row has no room for all, map levels go first; a phone on its
    /// side keeps the Julia panel alone (one panel beside the two-row cockpit).
    #[test]
    fn julia_panel_takes_the_last_place_of_the_row() {
        let touch = |inset_right: f64| {
            let mut c = crate::cockpit::CockpitOverlay::default();
            c.enabled = true;
            c.touch = true;
            c.inset_right = inset_right;
            c
        };
        let mut s = OverlaySettings {
            orbit: true,
            map: true,
            julia: true,
            ..OverlaySettings::default()
        };
        assert_eq!(row_wants(&s), (3, 4));
        s.map_levels = 4;
        assert_eq!(row_wants(&s), (3, 4), "four textures: three maps with the Julia panel");
        s.map = false;
        assert_eq!(row_wants(&s), (1, 2));
        s.orbit = false;
        assert_eq!(row_wants(&s), (0, 1));
        // (size points, ppp, insets, trailing, top, cockpit inset, expected maps)
        let cases = [
            (
                "iPhone portrait",
                (390.0, 844.0),
                1.0 / 3.0,
                (47.0, 0.0, 34.0, 0.0),
                56.0,
                0.0,
                56.0,
                1,
            ),
            (
                "iPhone landscape",
                (844.0, 390.0),
                1.0 / 3.0,
                (0.0, 47.0, 21.0, 47.0),
                60.0,
                0.0,
                272.0,
                0,
            ),
            (
                "iPad mini portrait",
                (744.0, 1133.0),
                0.5,
                (24.0, 0.0, 20.0, 0.0),
                60.0,
                56.0,
                0.0,
                3,
            ),
            (
                "iPad mini landscape",
                (1133.0, 744.0),
                0.5,
                (24.0, 0.0, 20.0, 0.0),
                60.0,
                56.0,
                0.0,
                3,
            ),
            (
                "Mac 1280x820",
                (1280.0, 820.0),
                0.5,
                (52.0, 0.0, 0.0, 0.0),
                0.0,
                0.0,
                0.0,
                3,
            ),
        ];
        let s = OverlaySettings {
            orbit: true,
            map: true,
            julia: true,
            ..OverlaySettings::default()
        };
        let (maps, want) = row_wants(&s);
        for (name, pt, ppp, insets, trailing, top, cockpit_inset, expect) in cases {
            let size = ((pt.0 / ppp) as usize, (pt.1 / ppp) as usize);
            let cockpit = if name.starts_with("Mac") {
                let mut c = crate::cockpit::CockpitOverlay::default();
                c.enabled = true;
                c
            } else {
                touch(cockpit_inset)
            };
            let f = fit_maps(
                want,
                180.0,
                Corner::BottomLeft,
                size,
                ppp,
                insets,
                trailing,
                top,
                &cockpit,
            );
            let levels = row_map_levels(f.levels, maps, true);
            assert_eq!(levels, expect, "{name}: {f:?}");
            // The Julia panel is the row's last place and ends left of the trailing controls.
            let px = 1.0 / ppp;
            let panel = (
                (f.width * px).round() as usize,
                (f.width * MAP_ASPECT * px).round() as usize,
            );
            let o = row_origins(levels + 1, Corner::BottomLeft, panel, size, insets, &f, px);
            let right = f64::from(o[levels][0]) * ppp + f.width;
            assert!(
                right <= pt.0 - insets.3 - trailing.max(cockpit_inset) - 12.0 + 0.5,
                "{name}: the Julia panel ends at {right}"
            );
            eprintln!(
                "{name}: {levels} map levels and the Julia panel, {:.0} points wide",
                f.width
            );
        }
    }
}
