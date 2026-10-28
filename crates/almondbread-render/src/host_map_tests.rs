//! The mini-map through a real host on Metal, judged by what reaches the screen: every frame is
//! presented into a shared texture the test reads back, and compared with the shown image
//! composited afresh with the current overlay (`capture_shown`). State-based: loops run until a
//! condition holds (with generous caps), never against the wall clock.

use super::*;
use crate::backend::{Storage, TextureFormat};
use crate::metal::MetalBackend;
use almondbread_core::settings::OverlaySettings;
use almondbread_core::viewport::Viewport;

const W: f64 = 800.0;
const H: f64 = 500.0;
/// An empty map panel on screen, BGRA8 (linear 0.02, 0.02, 0.03 in sRGB).
const EMPTY: [u8; 3] = [48, 39, 39];

/// A settings change that resizes the map panels, by name.
type Change = (&'static str, fn(&mut OverlaySettings));

struct Rig {
    host: Host<MetalBackend>,
    target: <MetalBackend as GpuBackend>::Texture,
    start: std::time::Instant,
}

impl Rig {
    fn new() -> Self {
        let backend = MetalBackend::new().expect("metal");
        let target = backend
            .create_texture(W as usize, H as usize, TextureFormat::Bgra8Unorm, Storage::Shared)
            .expect("target");
        let mut host = Host::new(Renderer::new(backend).expect("renderer"));
        host.resize(W, H, 1.0, 0.0);
        host.renderer.viewport = Viewport::new(-0.743643887037151, 0.13182590420533, 1.25e-6);
        host.overlay.settings.map = true;
        host.overlay.settings.map_levels = 3;
        Self {
            host,
            target,
            start: std::time::Instant::now(),
        }
    }

    fn frame(&mut self) {
        let start = self.start;
        let now = move || start.elapsed().as_secs_f64();
        let t = now();
        let target = self.target.clone();
        self.host
            .frame(t, &now, true, &mut |_| Some(((target.clone(), None), 0)));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    /// Refreshes until the view and every map level are settled (at most `cap`), then two more
    /// (an overlay-only present follows the last map frame); whether they settled.
    fn settle(&mut self, cap: usize) -> bool {
        let mut ok = false;
        for _ in 0..cap {
            self.frame();
            if self.host.renderer.is_settled() && !self.host.overlay.busy() {
                ok = true;
                break;
            }
        }
        self.frame();
        self.frame();
        ok
    }

    /// Panel `id`'s rect in pixels (ppp 1): x0, y0, x1, y1.
    fn rect(&self, id: &str) -> [usize; 4] {
        let r = self
            .host
            .overlay_panels()
            .into_iter()
            .find(|(p, _)| *p == id)
            .map(|(_, r)| r)
            .unwrap_or_else(|| panic!("no {id} panel"));
        [
            r[0] as usize,
            r[1] as usize,
            (r[0] + r[2]) as usize,
            (r[1] + r[3]) as usize,
        ]
    }

    /// The first map panel's rect in pixels.
    fn first_map(&self) -> [usize; 4] {
        let p = self.host.overlay.map_panels()[0].0;
        [p[0] as usize, p[1] as usize, p[2] as usize, p[3] as usize]
    }

    /// The frame last presented, BGRA8.
    fn on_screen(&self) -> Vec<u8> {
        let (w, h) = (W as usize, H as usize);
        let mut px = vec![0u8; w * h * 4];
        self.host
            .renderer
            .backend()
            .read_texture(&self.target, w, h, TextureFormat::Bgra8Unorm, &mut px)
            .expect("read");
        px
    }

    /// Pixels of panel `id` that differ (by more than 2 in a channel) between the frame on
    /// screen and the shown image composited afresh with the current overlay.
    fn stale_pixels(&mut self, id: &str) -> usize {
        let screen = self.on_screen();
        let (fresh, w, _) = self.host.renderer.capture_shown().expect("capture");
        let [x0, y0, x1, y1] = self.rect(id);
        let mut n = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * w + x) * 4;
                if (0..3).any(|c| screen[i + c].abs_diff(fresh[i + c]) > 2) {
                    n += 1;
                }
            }
        }
        n
    }

    /// The share of the inner pixels of `r` (8 px in from its edges) that are not the empty
    /// panel's colour, in the frame on screen.
    fn drawn(&self, r: [usize; 4]) -> f64 {
        let screen = self.on_screen();
        let w = W as usize;
        let [x0, y0, x1, y1] = r;
        let (mut drawn, mut all) = (0, 0);
        for y in y0 + 8..y1 - 8 {
            for x in x0 + 8..x1 - 8 {
                let i = (y * w + x) * 4;
                all += 1;
                if (0..3).any(|c| screen[i + c].abs_diff(EMPTY[c]) > 16) {
                    drawn += 1;
                }
            }
        }
        drawn as f64 / all.max(1) as f64
    }

    fn panel_width(&self) -> f32 {
        let p = self.host.overlay.map_panels()[0].0;
        p[2] - p[0]
    }
}

/// The user's report (2026-10-28): after the enlarge button the whole-set panel stayed half
/// drawn. Each map level renders progressively into its own texture, but the overlay was only
/// composited again when its scene changed, and a level's progress was not part of the scene:
/// with the main view settled, the screen kept the level's first partial frame. Now the panel
/// on screen ends up exactly the finished map after enlarging, shrinking and a "Mini-map size"
/// change, and the old picture shows, stretched, while the new one renders.
#[test]
fn resized_map_panels_complete_on_screen() {
    let mut rig = Rig::new();
    assert!(rig.settle(6000), "the view and its maps settle");
    assert_eq!(rig.stale_pixels("maps"), 0, "the maps on screen are the finished ones");
    let small = rig.panel_width();
    let steps: [Change; 3] = [
        ("enlarged", |s| s.map_large = true),
        ("shrunk", |s| s.map_large = false),
        ("resized", |s| s.map_size = 240.0),
    ];
    for (what, change) in steps {
        let before = rig.panel_width();
        change(&mut rig.host.overlay.settings);
        // The first refreshes after the change: the old picture, stretched, never an empty panel.
        for i in 0..3 {
            rig.frame();
            let drawn = rig.drawn(rig.first_map());
            assert!(
                drawn > 0.5,
                "{what}: refresh {i}: only {:.0} % of the panel drawn",
                drawn * 100.0
            );
        }
        assert!(rig.settle(6000), "{what}: the maps settle");
        assert_ne!(rig.panel_width(), before, "{what}: the panel changed size");
        let stale = rig.stale_pixels("maps");
        assert_eq!(
            stale, 0,
            "{what}: {stale} map pixels on screen are not the finished map"
        );
        assert!(rig.drawn(rig.first_map()) > 0.5, "{what}: the panel is drawn");
    }
    assert!(rig.panel_width() > small, "240 points is wider than the default 180");
}

/// In a Julia view the Mandelbrot panel (the way back) takes the companion's place at the map
/// panels' size: enlarged, it shows its old picture stretched, then ends up exactly the finished
/// picture on screen, as the maps do.
#[test]
fn resized_mandelbrot_panel_completes_on_screen() {
    let mut rig = Rig::new();
    assert!(rig.host.julia_twin());
    assert!(
        rig.settle(6000),
        "the Julia view, its maps and the Mandelbrot panel settle"
    );
    assert!(rig.host.overlay.mandelbrot.shown());
    assert_eq!(rig.stale_pixels("mandelbrot-panel"), 0);
    let before = rig.rect("mandelbrot-panel");
    rig.host.overlay.settings.map_large = true;
    for i in 0..3 {
        rig.frame();
        let drawn = rig.drawn(rig.rect("mandelbrot-panel"));
        assert!(
            drawn > 0.5,
            "refresh {i}: only {:.0} % of the panel drawn",
            drawn * 100.0
        );
    }
    assert!(rig.settle(6000), "the panel settles");
    assert_ne!(rig.rect("mandelbrot-panel"), before, "the panel changed size");
    for id in ["mandelbrot-panel", "maps"] {
        let stale = rig.stale_pixels(id);
        assert_eq!(
            stale, 0,
            "{stale} pixels of {id} on screen are not the finished picture"
        );
    }
}
