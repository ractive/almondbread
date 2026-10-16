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

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use almondbread_core::math::{DoubleExp, NewtonOptions, PeriodMethod, find_nucleus, find_period};
use almondbread_core::orbit::{OrbitClass, OrbitPoints, OrbitProbe, ProbePoint, ProbeReference, orbit_points};
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

fn full_set_view(w: usize, h: usize) -> Viewport {
    let s = (3.2 / w as f64).max(2.6 / h as f64);
    Viewport::new(-0.7, 0.0, s)
}

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
    probe_key: Option<(Viewport, [i64; 2], usize, u64)>,
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
    /// The π meter's uploaded trail picture and its serial.
    trail: Option<(u64, B::Buffer)>,
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
            trail: None,
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
        self.settings.any() || self.cosmic.enabled() || self.pi.enabled
    }

    /// Work left (probe analysing, maps rendering, a click resolving, a π count): keep
    /// frames coming.
    pub fn busy(&self) -> bool {
        self.pi.busy()
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
        let s = self.settings.clone().sanitized();
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
        if !s.any() || empty {
            self.maps = None;
            self.probe = None;
            self.probe_key = None;
            let line = renderer.progress.current().alpha > 0.0;
            if !(self.cosmic.enabled() || self.pi.enabled || self.compass.shown(now) || line) || empty {
                renderer.overlay = None;
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
        // ---- Maps (the orbit overlay shows the whole-set map too).
        let want_levels = if s.map { s.map_levels } else { 1 };
        let panel = (
            ((s.map_size * px_per_pt).round() as usize).clamp(32, 4096),
            ((s.map_size * 0.72 * px_per_pt).round() as usize).clamp(24, 4096),
        );
        if s.any()
            && let Err(e) = self.update_maps(renderer, &view, want_levels, panel, size, &s, px_per_pt)
        {
            eprintln!("almondbread: overlay maps: {e}");
            self.maps = None;
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
        let key = self.build_scene(&mut scene, renderer, &view, size, ppp, &s, now);
        if self.scene_key.as_deref() != Some(key.as_str()) || renderer.overlay.is_none() {
            self.version += 1;
            self.scene_key = Some(key);
            if self.font.is_none() {
                self.font = renderer.backend().create_buffer_with_data(&font_bytes()).ok();
            }
            let maps: Vec<B::Texture> = self
                .maps
                .as_ref()
                .map(|m| m.levels.iter().map(|l| l.target.clone()).collect())
                .unwrap_or_default();
            let trail = self.trail.as_ref().map(|(_, t)| t.clone());
            match OverlayDraw::upload(renderer.backend(), &scene, size, maps, trail, self.version) {
                Ok(d) => {
                    renderer.overlay_font = self.font.clone();
                    renderer.overlay = Some(d);
                }
                Err(e) => eprintln!("almondbread: overlay upload: {e}"),
            }
        }
    }

    fn update_probe(&mut self, renderer: &Renderer<B>, view: &Viewport, ppp: f64, length: usize) {
        let (tx, ty) = renderer.navigator.zoom_target();
        // Key: the view, the target to 1/64 point, the length and the reference.
        let serial = renderer.reference_serial();
        let key = (
            view.clone(),
            [(tx * 64.0).round() as i64, (ty * 64.0).round() as i64],
            length,
            serial,
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
        let c = view.point_at_offset(tx, ty);
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
                        reference: Some(r),
                        dc: almondbread_core::math::DoubleExpComplex::new(sc(dx), sc(dy)),
                    },
                    almondbread_core::math::DoubleExpComplex::new(sc(ox), sc(oy)),
                    [view.center.re(), view.center.im()],
                )
            }
            None => (
                ProbePoint {
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
        let max_iter = renderer.stats.max_iterations.max(renderer.max_iterations()).max(1000);
        self.probe = Some(OrbitProbe::new(point, max_iter));
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
        want: usize,
        panel: (usize, usize),
        size: (usize, usize),
        s: &OverlaySettings,
        px_per_pt: f64,
    ) -> Result<(), GpuError> {
        // Level decades: 0 (whole set), then `want - 1` levels spread evenly between it and the
        // view (`map_decades`).
        let full = full_set_view(panel.0, panel.1);
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
        let decades = map_decades(depth, want, &prev);
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
        let margin = (12.0 * px_per_pt) as f32;
        let gap = (8.0 * px_per_pt) as f32;
        let (pw, ph) = (panel.0 as f32, panel.1 as f32);
        let (w, h) = (size.0 as f32, size.1 as f32);
        // Clear of the toolbar and a sidebar over the canvas (safe-area insets, points).
        let (top, left, bottom, right) = renderer.navigator.insets();
        let px = |v: f64| (v * px_per_pt) as f32;
        let (x0, x1) = (px(left) + margin, w - px(right) - margin);
        let (y0, y1) = (px(top) + margin, h - px(bottom) - margin);
        let n = stack.levels.len();
        stack.panels = (0..n)
            .map(|i| {
                let off = i as f32 * (pw + gap);
                match s.map_corner {
                    Corner::BottomLeft => [x0 + off, y1 - ph],
                    Corner::BottomRight => [x1 - pw - off, y1 - ph],
                    Corner::TopLeft => [x0 + off, y0],
                    Corner::TopRight => [x1 - pw - off, y0],
                }
            })
            .collect();
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
    pub fn click(&mut self, renderer: &Renderer<B>, x: f64, y: f64, ppp: f64, fly: bool) -> bool {
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
            let ratio = map_scale.ratio(view_px);
            let view_scale = renderer.viewport.scale;
            let max_iter = renderer.max_iterations().max(1000);
            let mut probe_view = Viewport::with_center(c.clone(), view_scale);
            probe_view.ensure_precision();
            let bits = probe_view.needed_center_bits().max(c.precision_bits());
            let handle = std::thread::spawn(move || resolve_click(c, map_scale, ratio, bits, max_iter, fly));
            self.snap = Some(SnapJob { handle });
            return true;
        }
        false
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

    #[allow(clippy::too_many_arguments)]
    fn build_scene(
        &self,
        scene: &mut Scene,
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
                    rgba(0.02, 0.02, 0.03, 0.55),
                    rgba(1.0, 1.0, 1.0, 0.22),
                    (1.0 * px) as f32,
                );
                if l.shown {
                    scene.image(r, i as u32, 0.92, radius);
                }
                scene.set_clip(Some(r));
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
                    let mp: Vec<[f64; 2]> = self.points.plane.iter().map(|&z| to_map(z)).collect();
                    let n = mp.len();
                    for k in 0..n.saturating_sub(1) {
                        let a = 0.85 * (1.0 - 0.7 * k as f32 / n.max(2) as f32);
                        scene.segment(
                            mp[k],
                            mp[k + 1],
                            (0.5 * px) as f32,
                            rgba(ORBIT_COOL[0], ORBIT_COOL[1], ORBIT_COOL[2], a),
                            rgba(ORBIT_COOL[0], ORBIT_COOL[1], ORBIT_COOL[2], a),
                        );
                    }
                    for p in mp.iter().skip(1) {
                        scene.disc(*p, (1.2 * px) as f32, rgba(1.0, 1.0, 1.0, 0.8));
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
            // Info under (or above) the first panel: the orbit's fate and a click note.
            let size = (13.0 * px).max(11.0) as f32;
            let line = 1.35 * size;
            let o0 = stack.panels[0];
            let below = matches!(s.map_corner, Corner::TopLeft | Corner::TopRight);
            let mut lines: Vec<String> = Vec::new();
            if s.orbit
                && let Some(p) = &self.probe
            {
                lines.push(format!("c {}", p.class.text()));
                lines.push(format!("|z{}| {}", self.points.plane.len(), sci(self.points.last_abs)));
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
            let t = format!(
                "c {}   |z{}| {}",
                p.class.text(),
                self.points.plane.len(),
                sci(self.points.last_abs)
            );
            scene.label(
                [(12.0 * px) as f32, (h - 24.0 * px) as f32],
                size,
                &t,
                rgba(1.0, 1.0, 1.0, 0.92),
            );
            key.push_str(&t);
        }
        if self.cosmic.enabled() {
            let left = (12.0 * px) as f32;
            self.cosmic.draw(scene, size, ppp, [left, cosmic_bottom], now, &mut key);
        }
        // The π meter keeps clear of the maps in the right corners and of what is at the
        // bottom left (maps, the cosmic scale).
        let mut reserve = crate::pi_meter::Reserve::default();
        if let Some(stack) = &self.maps
            && !stack.panels.is_empty()
        {
            let (pw, ph) = (stack.panel.0 as f32, stack.panel.1 as f32);
            let n = stack.panels.len() as f32;
            let extent = (12.0 * px) as f32 + n * pw + (n - 1.0) * (8.0 * px) as f32;
            // (Plus the info lines next to the first panel.)
            let block = ph + (12.0 * px) as f32 + (44.0 * px) as f32;
            match s.map_corner {
                Corner::BottomRight => reserve.bottom_right = block,
                Corner::TopRight => reserve.top_right = block,
                Corner::BottomLeft => reserve.bottom_left = extent,
                Corner::TopLeft => {}
            }
        }
        if self.cosmic.enabled() {
            reserve.bottom_left = reserve.bottom_left.max((364.0 * px) as f32);
        }
        key.push_str(&self.pi.draw(scene, w, h, px, reserve));
        self.compass.draw(scene, size, ppp, view.angle, now, &mut key);
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

/// "1.234e-5" style for the overlay text.
fn sci(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return format!("{v}");
    }
    if (1e-3..1e4).contains(&v.abs()) {
        format!("{v:.4}")
    } else {
        format!("{v:.3e}")
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
fn resolve_click(c: Center, map_scale: Scale, ratio: f64, bits: usize, max_iter: usize, fly: bool) -> MapJump {
    if ratio <= 4.0 {
        return MapJump {
            center: c,
            note: "jumped to the clicked point".into(),
            fly,
        };
    }
    let big = c.with_min_precision(bits);
    let radius = map_scale.big_times(1.0, 64);
    let period = find_period(
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
    match find_nucleus(big.big(), p, &opts, &mut || false) {
        Some(n) => {
            let size = almondbread_core::math::minibrot_size_complex(&n.nucleus, p);
            let quarter = DoubleExp::from_f64(0.25);
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
            "c escapes at n 37",
            "period 3  |λ| 0.412  arg +0.250",
            "×1e6 whole set",
            "|z50| 1.234e-5",
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
    fn labels_are_one_prim_per_glyph() {
        let mut s = Scene::default();
        let w = s.label([10.0, 10.0], 26.0, "a b", rgba(1.0, 1.0, 1.0, 1.0));
        assert_eq!(s.prims.len(), 2);
        assert!(w > 26.0 && w < 60.0, "{w}");
        // The halo width is limited to what the atlas stores.
        assert!(f32::from_bits(s.prims[0].info[2]) <= crate::font::SPREAD * 26.0 / crate::font::EM);
    }
}
