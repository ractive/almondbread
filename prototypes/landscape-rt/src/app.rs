//! The window (winit + a CAMetalLayer on its NSView). One frame per display refresh: the
//! blocking `nextDrawable` paces the loop, and `dt` is the measured interval snapped to whole
//! refresh periods so timer noise never becomes motion noise.
//!
//! `--script NAME` flies a scripted flight and prints frame-time statistics at the end;
//! `--background` makes it a test window: accessory app, never activated, ordered behind the
//! other windows on the fastest screen, App Nap off, and it quits by itself.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{msg_send, MainThreadMarker};
use objc2_core_foundation::CGSize;
use objc2_metal::{MTLCommandBuffer, MTLDrawable, MTLPixelFormat};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::{Window, WindowId};

use crate::flight::{Input, Script};
use crate::sim::{Settings, Sim};
use crate::stats;

#[derive(Clone, Debug, clap::Args)]
pub struct FlyArgs {
    #[command(flatten)]
    pub set: Settings,
    /// Fly a scripted flight (cruise, dive, turns, hover, tour) and print frame statistics.
    #[arg(long)]
    pub script: Option<String>,
    /// Test window: behind other windows, never activated, quits after the script.
    #[arg(long)]
    pub background: bool,
    /// Per-frame CSV of the scripted run: serial, frame start s, DE ms, render ms, tiles, blocks, S.
    #[arg(long)]
    pub csv: Option<std::path::PathBuf>,
    /// Window size in points.
    #[arg(long, default_value = "1280x800")]
    pub size: String,
}

struct App {
    args: FlyArgs,
    script: Option<Script>,
    window: Option<Window>,
    layer: Option<Retained<CAMetalLayer>>,
    sim: Option<Sim>,
    keys: HashSet<KeyCode>,
    last: Option<Instant>,
    refresh: f64,
    frame_starts: Vec<f64>,
    presented: Arc<Mutex<Vec<(u64, f64)>>>,
    t_start: Instant,
    last_title: f64,
    script_t: f64,
    _activity: Option<Retained<AnyObject>>,
    error: Option<String>,
    recent: Vec<f64>,
    reported: bool,
}

pub fn run(args: FlyArgs) -> Result<(), String> {
    let script = match &args.script {
        Some(n) => Some(Script::named(n).ok_or_else(|| format!("unknown script {n}"))?),
        None => None,
    };
    let mut builder = EventLoop::builder();
    if args.background {
        builder.with_activation_policy(ActivationPolicy::Accessory).with_activate_ignoring_other_apps(false);
    }
    let el = builder.build().map_err(|e| e.to_string())?;
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        args,
        script,
        window: None,
        layer: None,
        sim: None,
        keys: HashSet::new(),
        last: None,
        refresh: 1.0 / 60.0,
        frame_starts: Vec::new(),
        presented: Arc::new(Mutex::new(Vec::new())),
        t_start: Instant::now(),
        last_title: 0.0,
        script_t: 0.0,
        _activity: None,
        error: None,
        recent: Vec::new(),
        reported: false,
    };
    el.run_app(&mut app).map_err(|e| e.to_string())?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

impl App {
    fn input(&self) -> Input {
        let k = |c: KeyCode| self.keys.contains(&c) as i32 as f64;
        let shift = k(KeyCode::ShiftLeft).max(k(KeyCode::ShiftRight));
        // W / ↑ push (nose down), S / ↓ pull (nose up); with Shift they zoom in / out
        let push = k(KeyCode::KeyW).max(k(KeyCode::ArrowUp));
        let pull = k(KeyCode::KeyS).max(k(KeyCode::ArrowDown));
        let stick = push - pull;
        Input {
            pitch: if shift > 0.0 { 0.0 } else { stick },
            zoom: if shift > 0.0 { stick } else { 0.0 },
            turn: k(KeyCode::KeyA).max(k(KeyCode::ArrowLeft)) - k(KeyCode::KeyD).max(k(KeyCode::ArrowRight)),
            throttle: k(KeyCode::KeyE) - k(KeyCode::KeyQ),
        }
    }

    fn init(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let (w, h) = parse_size(&self.args.size);
        let start_speed = self.script.as_ref().map(|s| s.start_speed).unwrap_or(1.0);
        let mut sim = Sim::new(self.args.set.clone(), start_speed)?;
        if let Some(sc) = &self.script {
            sim.flight.auto_zoom = sim.flight.auto_zoom || sc.auto_zoom;
        }
        eprintln!("{}", sim.ref_note);
        let t = Instant::now();
        let n = sim.prefill();
        eprintln!("prefill: {n} tiles in {:.2} s", t.elapsed().as_secs_f64());

        let mut attrs = Window::default_attributes().with_title("landscape-rt").with_inner_size(LogicalSize::new(w, h));
        // the fastest screen (ProMotion) for tests; its bottom-right corner
        let fastest = el.available_monitors().max_by_key(|m| m.refresh_rate_millihertz().unwrap_or(0));
        if self.args.background {
            attrs = attrs.with_active(false);
            if let Some(m) = &fastest {
                let sf = m.scale_factor();
                let p = m.position().to_logical::<f64>(sf);
                let sz = m.size().to_logical::<f64>(sf);
                attrs = attrs.with_position(LogicalPosition::new(p.x + sz.width - w - 10.0, p.y + sz.height - h - 30.0));
            }
        }
        for m in el.available_monitors() {
            eprintln!("monitor {:?} {:?} at {:?} size {:?} scale {}", m.name(), m.refresh_rate_millihertz(), m.position(), m.size(), m.scale_factor());
        }
        let window = el.create_window(attrs).map_err(|e| e.to_string())?;
        if self.args.background
            && let Some(m) = &fastest
        {
            let sf = m.scale_factor();
            let p = m.position().to_logical::<f64>(sf);
            let sz = m.size().to_logical::<f64>(sf);
            window.set_outer_position(LogicalPosition::new(p.x + sz.width - w - 10.0, p.y + sz.height - h - 30.0));
        }
        eprintln!("window on {:?} at {:?}", window.current_monitor().and_then(|m| m.name()), window.outer_position());
        if let Some(m) = window.current_monitor().or(fastest)
            && let Some(mhz) = m.refresh_rate_millihertz()
        {
            self.refresh = 1000.0 / mhz as f64;
        }
        eprintln!("display refresh {:.1} Hz", 1.0 / self.refresh);
        sim.frame_target_ms = Some(self.refresh * 1e3 * 0.9);

        let RawWindowHandle::AppKit(handle) = window.window_handle().map_err(|e| e.to_string())?.as_raw() else {
            return Err("not an AppKit window".into());
        };
        let layer = CAMetalLayer::new();
        layer.setDevice(Some(&sim.gpu.device));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        layer.setFramebufferOnly(true);
        // the shader writes a measurement mask into alpha: never composite with it
        layer.setOpaque(true);
        layer.setMaximumDrawableCount(3);
        let size = window.inner_size();
        layer.setContentsScale(window.scale_factor());
        layer.setDrawableSize(CGSize { width: size.width as f64, height: size.height as f64 });
        // SAFETY: winit's NSView, alive as long as the window; main thread.
        unsafe {
            let view = handle.ns_view.as_ptr() as *mut AnyObject;
            let view: &AnyObject = &*view;
            let _: () = msg_send![view, setWantsLayer: true];
            let _: () = msg_send![view, setLayer: &*layer];
            if self.args.background {
                let win: *mut AnyObject = msg_send![view, window];
                if !win.is_null() {
                    let _: () = msg_send![&*win, orderBack: std::ptr::null::<AnyObject>()];
                }
            }
        }
        if self.args.background {
            self._activity = Some(disable_app_nap());
        }
        self.window = Some(window);
        self.layer = Some(layer);
        self.sim = Some(sim);
        self.t_start = Instant::now();
        Ok(())
    }

    fn draw(&mut self, el: &ActiveEventLoop) {
        let input = match (&self.script, &mut self.sim) {
            (Some(s), Some(sim)) if s.low.is_some() => crate::record::low_input(sim, self.script_t, s.low.unwrap()),
            (Some(s), _) => s.input_at(self.script_t),
            _ => self.input(),
        };
        let (Some(layer), Some(sim)) = (&self.layer, &mut self.sim) else { return };
        let Some(drawable) = objc2::rc::autoreleasepool(|_| layer.nextDrawable()) else { return };
        let now = Instant::now();
        let raw = self.last.map(|l| (now - l).as_secs_f64()).unwrap_or(self.refresh);
        self.last = Some(now);
        // whole refresh periods: the frame will be shown n refreshes after the previous one
        // the frame period: the display's, unless frames come faster (occluded window: drawables
        // are not vsynced) — then the median of the recent intervals
        self.recent.push(raw);
        if self.recent.len() > 31 {
            self.recent.remove(0);
        }
        let med = crate::stats::pct(&self.recent, 0.5);
        let period = if med < 0.75 * self.refresh { med.max(1.0 / 500.0) } else { self.refresh };
        let n = (raw / period).round().max(1.0);
        let dt = (n * period).min(0.1);
        self.frame_starts.push(now.duration_since(self.t_start).as_secs_f64());

        self.script_t += dt;
        let tex = drawable.texture();
        let cb = sim.frame(dt, input, &tex);
        let serial = sim.serial;
        let log = Arc::clone(&self.presented);
        let handler = RcBlock::new(move |d: std::ptr::NonNull<ProtocolObject<dyn MTLDrawable>>| {
            // SAFETY: Core Animation passes the live drawable being presented.
            let t = unsafe { d.as_ref() }.presentedTime();
            if t > 0.0
                && let Ok(mut l) = log.lock()
            {
                l.push((serial, t));
            }
        });
        let generic: Retained<ProtocolObject<dyn MTLDrawable>> = ProtocolObject::from_retained(drawable);
        // SAFETY: the block only touches a mutex-protected Vec.
        unsafe { generic.addPresentedHandler(RcBlock::as_ptr(&handler)) };
        cb.presentDrawable(&generic);
        cb.commit();

        let t = now.duration_since(self.t_start).as_secs_f64();
        if t - self.last_title > 0.25 {
            self.last_title = t;
            if let Some(w) = &self.window {
                w.set_title(&format!("landscape-rt  {}", sim.hud()));
            }
        }
        if let Some(s) = &self.script
            && self.script_t > s.duration() + 0.3
        {
            self.report();
            el.exit();
        }
    }

    fn report(&mut self) {
        if self.reported {
            return;
        }
        self.reported = true;
        let Some(sim) = &mut self.sim else { return };
        // let the last frames complete
        std::thread::sleep(std::time::Duration::from_millis(150));
        let pres = self.presented.lock().map(|v| v.clone()).unwrap_or_default();
        let name = self.script.as_ref().map(|s| s.name.clone()).unwrap_or_default();
        println!("script {name}: {} frames, refresh {:.1} Hz, window {}", sim.recs.len(), 1.0 / self.refresh, self.args.size);
        if pres.is_empty() {
            println!("NOTE: no frame was presented (window fully covered or off screen): frame pacing below is not vsynced");
        }
        stats::report_frames(&self.frame_starts, &pres, sim, self.refresh);
        if let Some(p) = &self.args.csv {
            let mut out = String::from("serial,start,presented,de_ms,render_ms,tiles,blocks,s\n");
            for (i, r) in sim.recs.iter().enumerate() {
                let g = sim.gpu_ms.iter().find(|g| g.0 == r.serial).copied().unwrap_or((0, 0.0, 0.0));
                let p = pres.iter().find(|p| p.0 == r.serial).map(|p| p.1).unwrap_or(0.0);
                out += &format!("{},{:.5},{:.5},{:.3},{:.3},{},{},{:.3e}\n", r.serial, self.frame_starts.get(i).copied().unwrap_or(0.0), p, g.1, g.2, r.tiles, r.blocks, r.s);
            }
            let _ = std::fs::write(p, out);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_none()
            && let Err(e) = self.init(el)
        {
            self.error = Some(e);
            el.exit();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(layer) = &self.layer {
                    layer.setDrawableSize(CGSize { width: size.width.max(1) as f64, height: size.height.max(1) as f64 });
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(layer) = &self.layer {
                    layer.setContentsScale(scale_factor);
                }
            }
            WindowEvent::Focused(false) => self.keys.clear(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            match code {
                                KeyCode::Escape => el.exit(),
                                KeyCode::KeyZ if !event.repeat => {
                                    if let Some(sim) = &mut self.sim {
                                        sim.flight.auto_zoom = !sim.flight.auto_zoom;
                                    }
                                }
                                KeyCode::KeyR if !event.repeat => {
                                    if let Some(sim) = &mut self.sim {
                                        let set = sim.set.clone();
                                        if let Ok(mut s) = Sim::new(set, 1.0) {
                                            s.prefill();
                                            *sim = s;
                                        }
                                    }
                                }
                                _ => {}
                            }
                            self.keys.insert(code);
                        }
                        ElementState::Released => {
                            self.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => self.draw(el),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn parse_size(s: &str) -> (f64, f64) {
    let mut it = s.split('x').map(|v| v.parse::<f64>().expect("size WxH"));
    (it.next().unwrap(), it.next().unwrap())
}

fn disable_app_nap() -> Retained<AnyObject> {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
    let _ = MainThreadMarker::new();
    let info = NSProcessInfo::processInfo();
    let token = info.beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiated | NSActivityOptions::LatencyCritical,
        &NSString::from_str("landscape-rt frame measurement"),
    );
    // SAFETY: the token is an NSObject; keep it retained as AnyObject.
    unsafe { Retained::cast_unchecked(token) }
}
