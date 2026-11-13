//! hamesh-shell: Reusable native application lifecycle, window management,
//! and event dispatch for Hamesh GUI.
//!
//! Downstream applications implement `HameshApp` and launch via `Shell::run(app, config)`.
//! Windowing (winit), presentation (softbuffer), pixel format conversion,
//! dirty checking, and Eye/Hand TCP inspection are handled cleanly by the shell.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hamesh_core::App;
use hamesh_layout::LayoutOutput;
use hamesh_scene::Scene;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState};
use winit::window::{Window, WindowAttributes, WindowId};

pub mod inspect_srv;

/// Shell configuration for application startup.
#[derive(Clone, Debug)]
pub struct ShellConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub min_width: u32,
    pub min_height: u32,
    pub enable_inspect_server: bool,
    pub inspect_port: Option<u16>,
    pub smoke_seconds: Option<u64>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            title: "Hamesh Application".to_string(),
            width: 800,
            height: 600,
            min_width: 320,
            min_height: 240,
            enable_inspect_server: true,
            inspect_port: None,
            smoke_seconds: None,
        }
    }
}

/// Context passed to `HameshApp` event methods.
pub struct ShellContext<'a> {
    pub app: &'a mut App,
    pub size: (u32, u32),
    pub cursor: (f32, f32),
    pub modifiers: ModifiersState,
    pub window: Option<&'a Window>,
    redraw_requested: bool,
    exit_requested: bool,
}

impl<'a> ShellContext<'a> {
    pub fn request_redraw(&mut self) {
        self.redraw_requested = true;
    }

    pub fn exit(&mut self) {
        self.exit_requested = true;
    }
}

/// Core application lifecycle trait for Hamesh applications.
pub trait HameshApp: 'static {
    /// Initialize application state and tree.
    fn init(&mut self, ctx: &mut ShellContext);

    /// Handle mouse click or release.
    fn on_mouse_click(
        &mut self,
        _ctx: &mut ShellContext,
        _button: MouseButton,
        _state: ElementState,
    ) {
    }

    /// Handle cursor motion.
    fn on_cursor_move(&mut self, _ctx: &mut ShellContext, _position: (f32, f32)) {}

    /// Handle mouse wheel scroll.
    fn on_mouse_wheel(&mut self, _ctx: &mut ShellContext, _delta: MouseScrollDelta) {}

    /// Handle keyboard input.
    fn on_key(&mut self, _ctx: &mut ShellContext, _key: &Key, _state: ElementState, _repeat: bool) {
    }

    /// Handle IME text input.
    fn on_ime(&mut self, _ctx: &mut ShellContext, _ime: Ime) {}

    /// Handle window resize.
    fn on_resize(&mut self, _ctx: &mut ShellContext, _new_size: (u32, u32)) {}

    /// Handle periodic frame tick or idle drain (called in `about_to_wait`).
    fn on_idle(&mut self, _ctx: &mut ShellContext) {}

    /// Optional hook to handle custom external inspector operations.
    fn on_inspect_op(
        &mut self,
        _ctx: &mut ShellContext,
        _op: &inspect_srv::Op,
    ) -> Option<inspect_srv::OpOutcome> {
        None
    }

    /// Build the live Scene for the current frame.
    /// Default implementation uses `hamesh_layout::compute` and `hamesh_scene::build`.
    fn view(&mut self, app: &App, size: (u32, u32), scene_rev: u64) -> (LayoutOutput, Scene) {
        let layout = hamesh_layout::compute(app, size.0 as f32, size.1 as f32);
        let scene = hamesh_scene::build(app, &layout, scene_rev);
        (layout, scene)
    }
}

/// User event sent to winit event loop (e.g. from background inspector server).
#[derive(Debug)]
pub enum ShellUserEvent {
    Inspect(inspect_srv::OpRequest),
}

struct PendingWait {
    target_rev: u64,
    deadline: Instant,
    respond: std::sync::mpsc::Sender<inspect_srv::OpOutcome>,
}

/// The internal winit application runner.
struct ShellRunner<A: HameshApp> {
    app_impl: A,
    app_state: App,
    config: ShellConfig,
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    size: (u32, u32),
    cursor: (f32, f32),
    modifiers: ModifiersState,
    scene_rev: u64,
    presented_rev: u64,
    frames: u64,
    cpu_surface_dims: Option<(u32, u32)>,
    cpu_pixels: Vec<u32>,
    proxy: Option<EventLoopProxy<ShellUserEvent>>,
    pending_waits: Vec<PendingWait>,
    smoke_deadline: Option<Instant>,
    smoke_exit_code: Option<i32>,
    last_layout: Option<LayoutOutput>,
    last_scene: Option<Scene>,
}

impl<A: HameshApp> ShellRunner<A> {
    fn new(app_impl: A, config: ShellConfig) -> Self {
        let size = (config.width.max(1), config.height.max(1));
        Self {
            app_impl,
            app_state: App::new("root"),
            config,
            window: None,
            context: None,
            surface: None,
            size,
            cursor: (0.0, 0.0),
            modifiers: ModifiersState::default(),
            scene_rev: 1,
            presented_rev: 0,
            frames: 0,
            cpu_surface_dims: None,
            cpu_pixels: Vec::new(),
            proxy: None,
            pending_waits: Vec::new(),
            smoke_deadline: None,
            smoke_exit_code: None,
            last_layout: None,
            last_scene: None,
        }
    }

    fn dispatch_event<F>(&mut self, event_loop: &ActiveEventLoop, default_redraw: bool, f: F)
    where
        F: FnOnce(&mut A, &mut ShellContext),
    {
        let mut ctx = ShellContext {
            app: &mut self.app_state,
            size: self.size,
            cursor: self.cursor,
            modifiers: self.modifiers,
            window: self.window.as_deref(),
            redraw_requested: false,
            exit_requested: false,
        };

        f(&mut self.app_impl, &mut ctx);

        let redraw = ctx.redraw_requested || default_redraw;
        let exit = ctx.exit_requested;

        if exit {
            event_loop.exit();
            return;
        }
        if redraw {
            self.scene_rev += 1;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn paint(&mut self) {
        let (w, h) = self.size;
        if w == 0 || h == 0 || self.surface.is_none() {
            return;
        }
        let (Some(ww), Some(hh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) else {
            return;
        };

        let dims_changed = self.cpu_surface_dims != Some((w, h));
        let scene_changed = self.scene_rev != self.presented_rev;

        if !dims_changed && !scene_changed && !self.cpu_pixels.is_empty() {
            return;
        }

        if dims_changed || scene_changed || self.cpu_pixels.is_empty() {
            let (layout, scene) = self
                .app_impl
                .view(&self.app_state, self.size, self.scene_rev);
            self.last_layout = Some(layout);
            self.last_scene = Some(scene.clone());

            let frame = hamesh_render::render(&scene);
            let src = frame.pixmap.data();

            let target_len = (w as usize) * (h as usize);
            if self.cpu_pixels.len() != target_len {
                self.cpu_pixels.resize(target_len, 0);
            }

            // Opaque-pixel fast-path vectorized conversion:
            let (pixels, _remainder) = src.as_chunks::<4>();
            for (pixel, chunk) in self.cpu_pixels.iter_mut().zip(pixels) {
                let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
                let (r, g, b) = if a == 255 {
                    (r as u32, g as u32, b as u32)
                } else if a == 0 {
                    (0, 0, 0)
                } else {
                    let a = a as u32;
                    (
                        (r as u32) * 255 / a,
                        (g as u32) * 255 / a,
                        (b as u32) * 255 / a,
                    )
                };
                *pixel = (r << 16) | (g << 8) | b;
            }

            if dims_changed {
                if let Some(surface) = self.surface.as_mut() {
                    let _ = surface.resize(ww, hh);
                    self.cpu_surface_dims = Some((w, h));
                }
            }
        }

        if let Some(surface) = self.surface.as_mut() {
            if let Ok(mut buffer) = surface.buffer_mut() {
                buffer.copy_from_slice(&self.cpu_pixels);
                if buffer.present().is_ok() {
                    self.frames += 1;
                    self.presented_rev = self.scene_rev;
                }
            }
        }
    }
}

impl<A: HameshApp> ApplicationHandler<ShellUserEvent> for ShellRunner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title(&self.config.title)
            .with_inner_size(PhysicalSize::new(self.config.width, self.config.height))
            .with_min_inner_size(PhysicalSize::new(
                self.config.min_width,
                self.config.min_height,
            ));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let context = softbuffer::Context::new(Arc::clone(&window)).expect("softbuffer context");
        let surface =
            softbuffer::Surface::new(&context, Arc::clone(&window)).expect("softbuffer surface");

        self.window = Some(Arc::clone(&window));
        self.context = Some(context);
        self.surface = Some(surface);

        if let Some(secs) = self.config.smoke_seconds {
            self.smoke_deadline = Some(Instant::now() + Duration::from_secs(secs));
        }

        self.dispatch_event(event_loop, true, |app, ctx| {
            app.init(ctx);
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                let new_size = (size.width.max(1), size.height.max(1));
                if self.size != new_size {
                    self.size = new_size;
                    self.dispatch_event(event_loop, true, |app, ctx| {
                        app.on_resize(ctx, new_size);
                    });
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.scene_rev += 1;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                let pos = self.cursor;
                self.dispatch_event(event_loop, false, |app, ctx| {
                    app.on_cursor_move(ctx, pos);
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.dispatch_event(event_loop, false, |app, ctx| {
                    app.on_mouse_click(ctx, button, state);
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.dispatch_event(event_loop, false, |app, ctx| {
                    app.on_mouse_wheel(ctx, delta);
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = event.logical_key.clone();
                let state = event.state;
                let repeat = event.repeat;
                self.dispatch_event(event_loop, false, |app, ctx| {
                    app.on_key(ctx, &key, state, repeat);
                });
            }
            WindowEvent::Ime(ime) => {
                self.dispatch_event(event_loop, false, |app, ctx| {
                    app.on_ime(ctx, ime);
                });
            }
            WindowEvent::RedrawRequested => {
                self.paint();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: ShellUserEvent) {
        match event {
            ShellUserEvent::Inspect(req) => {
                self.handle_inspect(event_loop, req);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.dispatch_event(event_loop, false, |app, ctx| {
            app.on_idle(ctx);
        });

        // Check pending inspector wait operations:
        if !self.pending_waits.is_empty() {
            let cur = self.app_state.revision;
            let now = Instant::now();
            let mut i = 0;
            while i < self.pending_waits.len() {
                if cur >= self.pending_waits[i].target_rev {
                    let w = self.pending_waits.swap_remove(i);
                    let _ = w
                        .respond
                        .send(inspect_srv::OpOutcome::Ok(serde_json::json!({
                            "resumed_at_rev": cur,
                            "timed_out": false,
                        })));
                } else if now >= self.pending_waits[i].deadline {
                    let w = self.pending_waits.swap_remove(i);
                    let _ = w
                        .respond
                        .send(inspect_srv::OpOutcome::Ok(serde_json::json!({
                            "resumed_at_rev": cur,
                            "timed_out": true,
                        })));
                } else {
                    i += 1;
                }
            }
        }

        // Check smoke-test deadline:
        if let Some(deadline) = self.smoke_deadline {
            if Instant::now() >= deadline {
                self.smoke_deadline = None;
                self.smoke_exit_code = Some(0);
                event_loop.exit();
                return;
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
    }
}

impl<A: HameshApp> ShellRunner<A> {
    fn handle_inspect(&mut self, _event_loop: &ActiveEventLoop, req: inspect_srv::OpRequest) {
        let inspect_srv::OpRequest { op, respond } = req;

        // Custom app interceptor hook:
        {
            let mut custom_outcome = None;
            self.dispatch_event(_event_loop, false, |app, ctx| {
                custom_outcome = app.on_inspect_op(ctx, &op);
            });
            if let Some(outcome) = custom_outcome {
                let _ = respond.send(outcome);
                return;
            }
        }

        // Standard built-in Eye/Hand operations:
        let outcome = match op {
            inspect_srv::Op::Map => {
                let (layout, scene) =
                    self.app_impl
                        .view(&self.app_state, self.size, self.scene_rev);
                let map = hamesh_inspect::compact_map(&self.app_state, &layout, &scene);
                inspect_srv::OpOutcome::Ok(
                    serde_json::to_value(&map).unwrap_or(serde_json::Value::Null),
                )
            }
            inspect_srv::Op::Detail { id } => {
                let (layout, scene) =
                    self.app_impl
                        .view(&self.app_state, self.size, self.scene_rev);
                match hamesh_inspect::detail(&self.app_state, &layout, Some(&scene), &id) {
                    Some(d) => inspect_srv::OpOutcome::Ok(
                        serde_json::to_value(&d).unwrap_or(serde_json::Value::Null),
                    ),
                    None => inspect_srv::OpOutcome::Err {
                        code: "NOT_FOUND",
                        message: format!("Component '{id}' not found"),
                        app_rev: self.app_state.revision,
                        scene_rev: self.scene_rev,
                    },
                }
            }
            inspect_srv::Op::Snapshot { path, overlay } => {
                let (_layout, scene) =
                    self.app_impl
                        .view(&self.app_state, self.size, self.scene_rev);
                let frame = hamesh_render::render(&scene);
                let mut pix = frame.pixmap;
                if let Some(ref target_id) = overlay {
                    if let Some(n) = scene.nodes.iter().find(|node| node.id == *target_id) {
                        hamesh_render::overlay::draw_guides(&mut pix, n);
                    }
                }
                match pix.save_png(&path) {
                    Ok(()) => inspect_srv::OpOutcome::Ok(serde_json::json!({
                        "saved_to": path,
                        "width": pix.width(),
                        "height": pix.height(),
                        "overlay": overlay,
                    })),
                    Err(e) => inspect_srv::OpOutcome::Err {
                        code: "IO_ERROR",
                        message: format!("Failed to save snapshot: {e}"),
                        app_rev: self.app_state.revision,
                        scene_rev: self.scene_rev,
                    },
                }
            }
            inspect_srv::Op::Wait {
                app_rev,
                timeout_ms,
            } => {
                if self.app_state.revision >= app_rev {
                    inspect_srv::OpOutcome::Ok(serde_json::json!({
                        "resumed_at_rev": self.app_state.revision,
                        "timed_out": false,
                    }))
                } else {
                    self.pending_waits.push(PendingWait {
                        target_rev: app_rev,
                        deadline: Instant::now() + Duration::from_millis(timeout_ms),
                        respond,
                    });
                    return;
                }
            }
            _ => inspect_srv::OpOutcome::Err {
                code: "UNSUPPORTED",
                message:
                    "Operation not supported by generic shell; implement on_inspect_op in HameshApp"
                        .to_string(),
                app_rev: self.app_state.revision,
                scene_rev: self.scene_rev,
            },
        };
        let _ = respond.send(outcome);
    }
}

/// The reusable Shell entry point.
pub struct Shell;

impl Shell {
    /// Launch a Hamesh application with the given configuration.
    pub fn run<A: HameshApp>(
        app: A,
        config: ShellConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());

        let event_loop = EventLoop::<ShellUserEvent>::with_user_event()
            .build()
            .map_err(|e| format!("failed to build event loop: {e}"))?;

        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));

        let proxy = event_loop.create_proxy();

        if config.enable_inspect_server {
            let addr = format!("127.0.0.1:{}", config.inspect_port.unwrap_or(0));
            if let Ok(lis) = std::net::TcpListener::bind(&addr) {
                let port = lis.local_addr().map(|a| a.port()).unwrap_or(0);
                let token = inspect_srv::dev_token();
                let _ = inspect_srv::write_discovery(port, &token);
                let proxy_clone = proxy.clone();
                let send = move |req: inspect_srv::OpRequest| {
                    proxy_clone.send_event(ShellUserEvent::Inspect(req)).is_ok()
                };
                std::thread::spawn(move || inspect_srv::serve(lis, token, send));
            }
        }

        let mut runner = ShellRunner::new(app, config);
        runner.proxy = Some(proxy);

        event_loop
            .run_app(&mut runner)
            .map_err(|e| format!("event loop error: {e}"))?;

        if let Some(code) = runner.smoke_exit_code {
            if code != 0 {
                std::process::exit(code);
            }
        }

        Ok(())
    }
}
