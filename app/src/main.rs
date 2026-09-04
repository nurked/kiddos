//! The KidDOS application: window, event loop, drive persistence.
#![cfg_attr(windows, windows_subsystem = "windows")]

use kiddos_host::{config, keys, Paths, RealHost};
use kiddos_kernel::{Child, HostRequest, Kernel, Vfs};
use kiddos_render::Renderer;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Fullscreen, Window, WindowId};

const FACTORY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/factory.kdd"));
const COLS: u16 = 80;
const ROWS: u16 = 25;

static SAVE_LOCK: Mutex<()> = Mutex::new(());

fn save_drive(kernel: &Kernel, paths: &Paths) {
    let _g = SAVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = kernel.save_drive(&paths.drive) {
        log::error!("could not save drive: {e}");
    }
}

/// Load the drive, writing the factory image first if there is none. A
/// drive that fails to load is moved aside, never deleted.
fn load_drive(paths: &Paths) -> Vfs {
    if !paths.drive.exists() {
        if let Err(e) = std::fs::write(&paths.drive, FACTORY) {
            log::error!("cannot write drive: {e}");
        }
    }
    match Vfs::load(&paths.drive) {
        Ok(mut v) => {
            // new content (games, lessons, man pages) reaches old drives
            let tmp = paths.dir.join("factory.kdd");
            if std::fs::write(&tmp, FACTORY).is_ok() {
                match Vfs::load(&tmp) {
                    Ok(factory) => {
                        if let Err(e) = v.refresh_from_factory(&factory) {
                            log::warn!("could not refresh drive content: {e}");
                        }
                    }
                    Err(e) => log::warn!("factory image unreadable: {e}"),
                }
                let _ = std::fs::remove_file(&tmp);
            }
            v
        }
        Err(e) => {
            log::error!("drive unreadable ({e}); starting from factory");
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = std::fs::rename(&paths.drive, paths.dir.join(format!("drive-broken-{ts}.kdd")));
            let _ = std::fs::write(&paths.drive, FACTORY);
            Vfs::load(&paths.drive).unwrap_or_else(|_| Vfs::new())
        }
    }
}

struct App {
    kernel: Arc<Kernel>,
    paths: Paths,
    requests: Receiver<HostRequest>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    mods: ModifiersState,
    start: Instant,
    fullscreen: bool,
    windowed_pref: bool,
    crt: bool,
    reset_pending: bool,
    last_drawn: (u64, bool),
    _init: Child,
}

impl App {
    fn blink_on(&self) -> bool {
        (self.start.elapsed().as_millis() / 530) % 2 == 0
    }

    fn handle_requests(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(r) = self.requests.try_recv() {
            log::info!("host request: {r:?}");
            match r {
                HostRequest::Shutdown => {
                    self.finish(false);
                    event_loop.exit();
                }
                HostRequest::Reboot => {
                    self.finish(true);
                    event_loop.exit();
                }
                HostRequest::ResetDrive => self.reset_pending = true,
                HostRequest::ExitFullscreen => self.set_fullscreen(false),
                HostRequest::EnterFullscreen => self.set_fullscreen(true),
                HostRequest::Crt(on) => {
                    self.crt = on;
                    if let Some(r) = &mut self.renderer {
                        r.crt = on;
                    }
                }
                HostRequest::Font(_) => {}
            }
        }
    }

    fn set_fullscreen(&mut self, on: bool) {
        self.fullscreen = on;
        if let Some(w) = &self.window {
            w.set_fullscreen(if on { Some(Fullscreen::Borderless(None)) } else { None });
            w.set_cursor_visible(!on);
        }
    }

    /// Save (or wipe) the drive and stop the kernel. With `reboot`, start a
    /// fresh copy of this program before we leave.
    fn finish(&mut self, reboot: bool) {
        self.kernel.shutdown();
        if self.reset_pending {
            let _ = std::fs::remove_file(&self.paths.drive);
            self.kernel.log("drive reset");
        } else {
            save_drive(&self.kernel, &self.paths);
        }
        if reboot {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).spawn();
            }
        }
    }
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title("KidDOS")
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0));
        if !self.windowed_pref {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        window.set_cursor_visible(self.windowed_pref);
        self.fullscreen = !self.windowed_pref;
        match Renderer::new(window.clone(), COLS, ROWS) {
            Ok(mut r) => {
                r.crt = self.crt;
                self.renderer = Some(r);
            }
            Err(e) => {
                log::error!("renderer: {e}");
                event_loop.exit();
            }
        }
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // In kiosk mode the window does not close; parents leave
                // fullscreen first (exit-fullscreen) and then may close it.
                if !self.fullscreen {
                    self.finish(false);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if keys::is_parent_chord(&event, self.mods) && event.state.is_pressed() {
                    self.kernel.interrupt_foreground();
                    self.kernel.push_text("\nparent\n");
                    return;
                }
                if let Some(k) = keys::map(&event, self.mods) {
                    self.kernel.push_key(k);
                }
            }
            WindowEvent::RedrawRequested => {
                let blink = self.blink_on();
                if let Some(r) = &mut self.renderer {
                    let t = self.start.elapsed().as_secs_f32();
                    let result = {
                        let screen = self.kernel.screen.lock();
                        self.last_drawn = (screen.generation(), blink);
                        r.draw(&screen, blink, t)
                    };
                    match result {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                            if let Some(w) = &self.window {
                                let s = w.inner_size();
                                r.resize(s.width, s.height);
                            }
                        }
                        Err(e) => log::warn!("draw: {e}"),
                    }
                }
                let bells = self.kernel.screen.lock().take_bells();
                if bells > 0 {
                    self.kernel.host().beep(880, 80);
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _e: ()) {
        self.handle_requests(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_requests(event_loop);
        let generation = self.kernel.screen.lock().generation();
        let blink = self.blink_on();
        if (generation, blink) != self.last_drawn {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16)));
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let paths = Paths::new();
    if let Err(e) = paths.ensure() {
        eprintln!("cannot create {}: {e}", paths.dir.display());
        std::process::exit(1);
    }
    let (config, windowed) = config::load(&paths.config);
    let crt = config.crt;
    let vfs = load_drive(&paths);

    let mut builder = EventLoop::<()>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false);
    }
    let event_loop = builder.build().expect("event loop");
    let proxy = event_loop.create_proxy();
    let (tx, rx): (Sender<HostRequest>, Receiver<HostRequest>) = std::sync::mpsc::channel();
    let host = Arc::new(RealHost::new(
        paths.clone(),
        tx,
        Box::new(move || {
            let _ = proxy.send_event(());
        }),
    ));
    let kernel = Kernel::new(vfs, host, config, COLS, ROWS);
    kiddos_builtins::register_all(&kernel);
    kiddos_shell::register(&kernel);
    kiddos_basic::register(&kernel);
    kiddos_cart::register(&kernel);
    kiddos_wasm::register(&kernel);
    kiddos_vi::register(&kernel);
    kiddos_tutor::Tutor::install(&kernel);
    kernel.log("boot");
    let init = kernel.boot();

    // autosave: every half second, if anything changed
    {
        let kernel = kernel.clone();
        let paths = paths.clone();
        std::thread::Builder::new()
            .name("autosave".into())
            .spawn(move || {
                let mut last = kernel.vfs_changes();
                loop {
                    std::thread::sleep(Duration::from_millis(500));
                    if kernel.shutting_down() {
                        return;
                    }
                    let now = kernel.vfs_changes();
                    if now != last {
                        save_drive(&kernel, &paths);
                        last = now;
                    }
                }
            })
            .expect("autosave thread");
    }

    let mut app = App {
        kernel,
        paths,
        requests: rx,
        window: None,
        renderer: None,
        mods: ModifiersState::empty(),
        start: Instant::now(),
        fullscreen: false,
        windowed_pref: windowed || std::env::var("KIDDOS_WINDOWED").is_ok(),
        crt,
        reset_pending: false,
        last_drawn: (0, false),
        _init: init,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        log::error!("event loop: {e}");
    }
    if !app.kernel.shutting_down() {
        app.finish(false);
    }
}
