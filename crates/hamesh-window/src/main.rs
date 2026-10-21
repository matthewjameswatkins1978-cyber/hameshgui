//! hamesh-window-demo: first REAL native window (M1 milestone gate).
//!
//! winit 0.30 window + softbuffer CPU presentation of the live Hamesh scene.
//! This spike proves the framework drives an actual OS window with the same
//! App/Layout/Scene truth as the headless drills — no parallel semantics.
//!
//! Controls (all through real winit events):
//!   mouse click  -> human-equivalent click at cursor (hit-test path)
//!   Space        -> human-equivalent click on the Increment button
//!   R            -> direct `reset` invocation (labeled DirectOp)
//!   I            -> dump inspection map + screenshot to target/
//!   Esc / close  -> exit
//!
//! Run: `cargo run -p hamesh-window --bin hamesh-window-demo`

use hamesh_automation::{click_component_human, click_human, invoke_direct};
use hamesh_core::{App, Component, Role};
use hamesh_diag::Trace;
mod gpu;

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

fn sample_app() -> App {
    let mut app = App::new("win");
    app.add(
        "win",
        Component::new("header", Role::Header, "Hamesh native window")
            .with_prop("h", "48")
            .with_prop("text", "Hamesh native window — Eye and Hand live"),
    );
    app.add(
        "win",
        Component::new("panel", Role::Panel, "main").with_prop("layout", "column"),
    );
    app.add(
        "panel",
        Component::new("counter_label", Role::Text, "counter label")
            .with_prop("h", "28")
            .with_prop("text", "Counter: 0"),
    );
    app.add(
        "panel",
        Component::new("btn_inc", Role::Button, "Increment primary")
            .with_prop("h", "40")
            .with_prop("text", "Increment"),
    );
    app.add(
        "panel",
        Component::new("btn_reset", Role::Button, "Reset")
            .with_prop("h", "36")
            .with_prop("text", "Reset"),
    );
    app.add(
        "panel",
        Component::new("status", Role::Status, "status")
            .with_prop("h", "28")
            .with_prop("text", "click, Space, R, I, Esc"),
    );
    app.register_command("increment", "btn_inc");
    app.register_command("reset", "btn_reset");
    app.set_state("counter", "0");
    app
}

fn sync_label(app: &mut App) {
    let c = app.state.get("counter").cloned().unwrap_or_default();
    if let Some(l) = app.get_mut("counter_label") {
        l.props.insert("text".to_string(), format!("Counter: {c}"));
    }
    if let Some(s) = app.get_mut("status") {
        s.props.insert("text".to_string(), format!("counter={c}"));
    }
}

struct HameshWindowApp {
    app: App,
    trace: Trace,
    scene_rev: u64,
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    gpu: Option<gpu::GpuState>,
    use_gpu: bool,
    cursor: (f32, f32),
    size: (u32, u32),
    frames: u64,
    /// Non-interactive verification: exit after this deadline, leaving
    /// trace + map + PNG evidence. `None` = interactive mode.
    smoke_deadline: Option<Instant>,
}

impl HameshWindowApp {
    fn new() -> Self {
        let mut app = sample_app();
        sync_label(&mut app);
        Self {
            app,
            trace: Trace::new(256),
            scene_rev: 1,
            window: None,
            context: None,
            surface: None,
            gpu: None,
            use_gpu: false,
            cursor: (0.0, 0.0),
            size: (800, 600),
            frames: 0,
            smoke_deadline: None,
        }
    }

    fn layout_and_scene(&self) -> (hamesh_layout::LayoutOutput, hamesh_scene::Scene) {
        let layout = hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
        let scene = hamesh_scene::build(&self.app, &layout, self.scene_rev);
        (layout, scene)
    }

    fn paint(&mut self) {
        let (w, h) = self.size;
        if w == 0 || h == 0 {
            return;
        }
        if self.use_gpu {
            self.paint_gpu();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }
        // Render the scene BEFORE borrowing the surface (borrow discipline).
        let (_layout, scene) = self.layout_and_scene();
        let frame = hamesh_render::render(&scene);
        let Some(window) = &self.window else { return };
        let Some(surface) = &mut self.surface else {
            return;
        };
        let src = frame.pixmap.data();
        // tiny-skia stores premultiplied RGBA; softbuffer wants 0RGB u32.
        let mut dst = vec![0u32; (w as usize) * (h as usize)];
        let (pixels, _remainder) = src.as_chunks::<4>();
        for (pixel, chunk) in dst.iter_mut().zip(pixels) {
            let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            let (r, g, b) = if a == 0 {
                (0, 0, 0)
            } else if a == 255 {
                (r as u32, g as u32, b as u32)
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
        if let (Some(ww), Some(hh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
            if surface.resize(ww, hh).is_ok() {
                if let Ok(mut buffer) = surface.buffer_mut() {
                    buffer.copy_from_slice(&dst);
                    if buffer.present().is_ok() {
                        self.frames += 1;
                    }
                }
            }
        }
        window.request_redraw();
    }

    fn paint_gpu(&mut self) {
        let (_layout, scene) = self.layout_and_scene();
        let Some(gpu) = self.gpu.as_mut() else {
            self.trace.record(
                "gpu.unavailable",
                "GPU requested but GpuState is missing; frame skipped",
                self.app.revision,
                None,
            );
            return;
        };
        match gpu.paint(&scene) {
            Ok(nodes) => {
                self.frames += 1;
                if self.frames == 1 {
                    self.trace.record(
                        "gpu.first_frame",
                        &format!("presented {nodes} vello nodes"),
                        self.app.revision,
                        None,
                    );
                    println!("GPU first frame: {nodes} nodes presented");
                }
            }
            Err(e) => {
                self.trace
                    .record("gpu.error", &format!("{e}"), self.app.revision, None);
            }
        }
    }

    /// GPU readback for the tolerance harness: render offscreen and save.
    /// No-op unless `--gpu` was passed and init succeeded.
    fn dump_gpu_snapshot(&mut self) {
        if !self.use_gpu {
            return;
        }
        let (_layout, scene) = self.layout_and_scene();
        if let Some(gpu) = self.gpu.as_mut() {
            match gpu.snapshot(&scene, "target/hamesh-window-gpu.png") {
                Ok((n, gpix)) => {
                    // Tolerance harness (decision 002): same Scene through
                    // the CPU renderer, compared pixel-by-pixel.
                    let cpu = hamesh_render::render(&scene);
                    let verdict = match hamesh_render::compare(
                        &gpix,
                        &cpu.pixmap,
                        &hamesh_render::Tolerance::default(),
                    ) {
                        Ok(()) => "within tolerance".to_string(),
                        Err(e) => format!("DIVERGED: {e}"),
                    };
                    self.trace.record(
                        "gpu.snapshot",
                        &format!("{n} nodes -> target/hamesh-window-gpu.png; cpu-vs-gpu {verdict}"),
                        self.app.revision,
                        Some("target/hamesh-window-gpu.png".to_string()),
                    );
                    println!(
                        "GPU snapshot: {n} nodes -> target/hamesh-window-gpu.png; cpu-vs-gpu {verdict}"
                    );
                }
                Err(e) => {
                    self.trace.record(
                        "gpu.snapshot_failed",
                        &format!("{e}"),
                        self.app.revision,
                        None,
                    );
                    println!("GPU snapshot FAILED: {e}");
                }
            }
        }
    }

    fn dump_inspection(&mut self) {
        let (layout, scene) = self.layout_and_scene();
        let map = hamesh_inspect::compact_map(&self.app, &layout, &scene);
        let _ = std::fs::write(
            "target/hamesh-window-map.json",
            serde_json::to_string_pretty(&map).unwrap(),
        );
        let frame = hamesh_render::render(&scene);
        let _ = hamesh_render::save_png(&frame, "target/hamesh-window.png");
        self.trace.record(
            "eye.dump",
            &format!(
                "app_rev={} scene_rev={} frames={} -> target/hamesh-window-map.json + target/hamesh-window.png",
                self.app.revision, self.scene_rev, self.frames
            ),
            self.app.revision,
            Some("target/hamesh-window.png".to_string()),
        );
        println!(
            "EYE app_rev={} scene_rev={} frames={} nodes={}",
            map.app_revision,
            map.scene_revision,
            self.frames,
            map.nodes.len()
        );
    }
}

impl ApplicationHandler for HameshWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Hamesh — the Eye and the Hand")
            .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        self.window = Some(Arc::clone(&window));
        if self.use_gpu {
            match gpu::GpuState::new(Arc::clone(&window), 800, 600) {
                Ok(state) => {
                    self.trace.record(
                        "gpu.ready",
                        "wgpu 30 + vello 0.11 renderer created",
                        self.app.revision,
                        None,
                    );
                    println!("GPU path: wgpu + vello ready");
                    self.gpu = Some(state);
                }
                Err(e) => {
                    self.trace.record(
                        "gpu.init_failed",
                        &format!("{e}; staying on CPU path"),
                        self.app.revision,
                        None,
                    );
                    println!("GPU init failed ({e}); CPU path active");
                    self.use_gpu = false;
                }
            }
        }
        if !self.use_gpu {
            let context =
                softbuffer::Context::new(Arc::clone(&window)).expect("softbuffer context");
            let surface = softbuffer::Surface::new(&context, Arc::clone(&window))
                .expect("softbuffer surface");
            self.context = Some(context);
            self.surface = Some(surface);
        }
        let size = window.inner_size();
        self.size = (size.width.max(1), size.height.max(1));
        self.trace.record(
            "window.open",
            &format!("{}x{} physical", self.size.0, self.size.1),
            self.app.revision,
            None,
        );
        println!(
            "HAMESH window open: {}x{} — click, Space, R, I, Esc",
            self.size.0, self.size.1
        );
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                let _ = std::fs::write("target/hamesh-window-trace.json", self.trace.export_json());
                println!("trace -> target/hamesh-window-trace.json");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.size = (size.width.max(1), size.height.max(1));
                self.scene_rev += 1;
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(self.size.0, self.size.1);
                }
                self.trace.record(
                    "window.resize",
                    &format!("{}x{}", self.size.0, self.size.1),
                    self.app.revision,
                    None,
                );
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    let layout =
                        hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
                    let r = click_human(&mut self.app, &layout, self.cursor.0, self.cursor.1);
                    sync_label(&mut self.app);
                    self.scene_rev += 1;
                    self.trace.record(
                        "hand.mouse_click",
                        &format!(
                            "at=({:.0},{:.0}) hit={:?} success={}",
                            self.cursor.0, self.cursor.1, r.hit_component, r.success
                        ),
                        self.app.revision,
                        None,
                    );
                    println!(
                        "HAND mouse: hit={:?} success={} counter={:?}",
                        r.hit_component,
                        r.success,
                        self.app.state.get("counter")
                    );
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::Space) => {
                        let layout = hamesh_layout::compute(
                            &self.app,
                            self.size.0 as f32,
                            self.size.1 as f32,
                        );
                        let r = click_component_human(&mut self.app, &layout, "btn_inc");
                        sync_label(&mut self.app);
                        self.scene_rev += 1;
                        self.trace.record(
                            "hand.key_space",
                            &format!("target=btn_inc success={}", r.success),
                            self.app.revision,
                            None,
                        );
                        println!(
                            "HAND space: success={} counter={:?}",
                            r.success,
                            self.app.state.get("counter")
                        );
                    }
                    PhysicalKey::Code(KeyCode::KeyR) => {
                        let r = invoke_direct(&mut self.app, "reset");
                        sync_label(&mut self.app);
                        self.scene_rev += 1;
                        self.trace.record(
                            "hand.key_r_direct",
                            &format!("mechanism={:?} success={}", r.mechanism, r.success),
                            self.app.revision,
                            None,
                        );
                        println!(
                            "HAND R (direct): counter={:?}",
                            self.app.state.get("counter")
                        );
                    }
                    PhysicalKey::Code(KeyCode::KeyI) => self.dump_inspection(),
                    _ => {}
                }
                let _ = event;
            }
            WindowEvent::RedrawRequested => self.paint(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(deadline) = self.smoke_deadline {
            if Instant::now() >= deadline {
                self.dump_inspection();
                self.dump_gpu_snapshot();
                let _ = std::fs::write("target/hamesh-window-trace.json", self.trace.export_json());
                println!(
                    "SMOKE frames={} trace=target/hamesh-window-trace.json",
                    self.frames
                );
                event_loop.exit();
                return;
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    // `--smoke-test=N`: open the real window, paint for N seconds, dump
    // Eye evidence (map + PNG + trace), exit 0. Proves the OS window opened
    // and presented frames without needing a human to click around.
    let smoke_secs: Option<u64> =
        std::env::args().find_map(|a| a.strip_prefix("--smoke-test=").and_then(|v| v.parse().ok()));
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = HameshWindowApp::new();
    app.use_gpu = std::env::args().any(|a| a == "--gpu");
    if app.use_gpu {
        println!("GPU mode: wgpu + vello (falls back to CPU if init fails)");
    }
    if let Some(secs) = smoke_secs {
        app.smoke_deadline = Some(Instant::now() + std::time::Duration::from_secs(secs));
        println!("SMOKE mode: {secs}s");
    }
    event_loop.run_app(&mut app).expect("run app");
}
