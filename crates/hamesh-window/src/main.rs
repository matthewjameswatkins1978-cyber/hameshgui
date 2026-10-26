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

use hamesh_automation::{click_component_human, invoke_direct};
use hamesh_core::{App, Component, Role};
use hamesh_diag::Trace;
use hamesh_widgets::Key as WKey;
use hamesh_widgets::{handle_key, rebuild_feed_nodes, sync_composer, Focus, Widgets};
mod dashboard;
mod gpu;
mod inspect_srv;
mod pi_panel;

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Cross-thread requests into the UI thread. The inspector server never
/// touches application state; it forwards here and the UI thread applies
/// the op with the same handlers as real input (Lucy's convergence rule).
#[derive(Debug)]
pub enum UserEvent {
    Inspect(inspect_srv::OpRequest),
}

/// A `wait` that has not resolved yet. Checked every frame in
/// `about_to_wait` so waiting never blocks the event loop.
struct PendingWait {
    target_rev: u64,
    deadline: Instant,
    respond: std::sync::mpsc::Sender<inspect_srv::OpOutcome>,
}

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
        Component::new("panel", Role::Panel, "main")
            .with_prop("layout", "column")
            .with_prop("flex_grow", "1"),
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

/// M3 Pi-style application tree: header + virtualized feed + composer.
fn m3_app() -> (App, Widgets) {
    let mut app = App::new("win");
    app.add(
        "win",
        Component::new("header", Role::Header, "Agent X chat")
            .with_prop("h", "48")
            .with_prop("text", "Agent X — Hamesh chat"),
    );
    app.add(
        "win",
        Component::new("feed", Role::List, "feed").with_prop("layout", "column"),
    );
    app.add(
        "win",
        Component::new("composer", Role::TextInput, "composer")
            .with_prop("h", "40")
            .with_prop("text", ""),
    );
    let mut w = Widgets {
        composer_id: "composer".to_string(),
        feed_id: "feed".to_string(),
        ..Default::default()
    };
    for i in 0..30 {
        let role = if i % 2 == 0 { "user" } else { "agent" };
        w.feed.push(
            role,
            &format!("Message {i}: the quick brown fox jumps over the lazy dog"),
        );
    }
    w.feed.relayout(700.0);
    w.feed.scroll_to_bottom(400.0);
    w.focus = Focus::rebuild(&app);
    sync_composer(&mut app, &w);
    (app, w)
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
    proxy: Option<EventLoopProxy<UserEvent>>,
    use_inspect: bool,
    inspect_serving: bool,
    pending_waits: Vec<PendingWait>,
    /// M3 Pi-style UI: conversation feed + composer, driven by the same
    /// `handle_key` core as the headless drills. `None` = M1 demo app.
    widgets: Option<Widgets>,
    use_m3: bool,
    /// Blocks dashboard mode (`--blocks`): the Agent X composition from
    /// hamesh-blocks replaces the M1 sample app. Same loop, Eye, Hand.
    use_blocks: bool,
    /// Blocks Pi panel (`--blocks-pi`): same builders, worker status
    /// composition. Implies `use_blocks` paths (refresh, Space).
    use_blocks_pi: bool,
    /// Gallery mode (`--gallery`): the 035 ListView + Picker workspace
    /// from hamesh-gallery. Same intents as the headless drill.
    use_gallery: bool,
    gallery: Option<hamesh_gallery::Gallery>,
    /// Tasks mode (`--tasks`): the 036 second composition (task queue +
    /// status filter + detail inspector) from hamesh-tasks. Same shared
    /// `Hit` router as the headless drill.
    use_tasks: bool,
    tasks: Option<hamesh_tasks::Tasks>,
    /// Sheet mode (`--sheet`): the 036 design-system sheet over the same
    /// shared `Hit` router and `Key` vocabulary as gallery and tasks.
    use_sheet: bool,
    sheet: Option<hamesh_sheet::Sheet>,
    /// Console mode (`--console`): the 037 Pi session console from
    /// hamesh-console (M3 chat + live engine status). Same shared
    /// router and key core; driver-owned state like tasks.
    use_console: bool,
    console: Option<hamesh_console::Console>,
    /// Editor mode (`--editor`): the 037 file editor from hamesh-editor
    /// (line list + composer + Save). Same shared router and key core.
    use_editor: bool,
    editor: Option<hamesh_editor::Editor>,
    shift_held: bool,
    m3_smoke_done: bool,
    console_smoke_done: bool,
    editor_smoke_done: bool,
    /// M4 live loop: background Pi worker link. `None` unless `--pi`.
    /// Never blocks the UI thread; drains between frames.
    pi: Option<PiLink>,
    /// Last session config used for a worker spawn (flags or attach):
    /// attach requests with empty dir/cwd fall back to these.
    pi_session: Option<SessionCfg>,
    use_pi: bool,
    cursor: (f32, f32),
    size: (u32, u32),
    frames: u64,
    /// Non-interactive verification: exit after this deadline, leaving
    /// trace + map + PNG evidence. `None` = interactive mode.
    smoke_deadline: Option<Instant>,
    /// Smoke exit status: set once by the deadline path when the M4
    /// verdict runs (0 PASS, 1 FAIL, 2 INCOMPLETE); `main` propagates it
    /// so the machine-checked smoke can actually fail a run.
    smoke_exit_code: Option<i32>,
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
            proxy: None,
            use_inspect: false,
            inspect_serving: false,
            pending_waits: Vec::new(),
            widgets: None,
            use_m3: false,
            use_blocks: false,
            use_blocks_pi: false,
            use_gallery: false,
            gallery: None,
            use_tasks: false,
            tasks: None,
            use_sheet: false,
            sheet: None,
            use_console: false,
            console: None,
            use_editor: false,
            editor: None,
            shift_held: false,
            m3_smoke_done: false,
            console_smoke_done: false,
            editor_smoke_done: false,
            pi: None,
            pi_session: None,
            use_pi: false,
            cursor: (0.0, 0.0),
            size: (800, 600),
            frames: 0,
            smoke_deadline: None,
            smoke_exit_code: None,
        }
    }

    fn layout_and_scene(&self) -> (hamesh_layout::LayoutOutput, hamesh_scene::Scene) {
        let layout = hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
        let scene = hamesh_scene::build(&self.app, &layout, self.scene_rev);
        (layout, scene)
    }

    /// Feed width for layout: viewport minus chrome. Kept in one place so
    /// headless drills, GPU cache keys and the window agree.
    fn feed_width(&self) -> f32 {
        (self.size.0 as f32 - 60.0).max(200.0)
    }

    /// Swap the M1 demo tree for the M3 chat tree (called once, on open).
    fn enter_m3(&mut self) {
        let (app, mut w) = m3_app();
        self.app = app;
        let width = self.feed_width();
        rebuild_feed_nodes(&mut self.app, &mut w, "feed", width);
        w.focus = Focus::rebuild(&self.app);
        sync_composer(&mut self.app, &w);
        self.widgets = Some(w);
        self.scene_rev += 1;
        self.trace.record(
            "m3.enter",
            "feed + composer live; same handle_key core as drills",
            self.app.revision,
            None,
        );
    }

    /// Rebuild the visible feed window after scroll/submit/resize.
    /// The feed panel is SIZED to the available space (never hugging the
    /// full content): hug-all overflowed the column and flex-shrink
    /// crushed the fixed header/composer. Visible nodes fill exactly this.
    /// Budget at 600px height: chrome (header, composer, gaps, padding)
    /// takes 152, so feed = h-128 and the window is feed minus padding.
    fn refresh_feed(&mut self) {
        let width = self.feed_width();
        let viewport = (self.size.1 as f32 - 152.0).max(176.0);
        if let Some(w) = self.widgets.as_mut() {
            w.feed_viewport = viewport;
            let feed_id = w.feed_id.clone();
            rebuild_feed_nodes(&mut self.app, w, "feed", width);
            if let Some(f) = self.app.get_mut(&feed_id) {
                f.props
                    .insert("h".to_string(), format!("{:.0}", viewport + 24.0));
            }
            self.scene_rev += 1;
        }
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

    /// THE press path: hit-test, activate the hit command (if any), and
    /// move keyboard focus when the hit target takes it. Real mouse,
    /// external inspector clicks and automation focus all converge here —
    /// one routing implementation, not three similar ones.
    fn apply_press(&mut self, x: f32, y: f32) -> (Option<String>, bool, Option<String>) {
        let layout = hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
        let r = hamesh_automation::click_human(&mut self.app, &layout, x, y);
        let hit = r.hit_component.clone();
        let mut focus = None;
        if let Some(id) = &hit {
            if let Some(w) = self.widgets.as_mut() {
                if let Some(pos) = w.focus.order.iter().position(|o| o == id) {
                    w.focus.current = pos;
                    hamesh_widgets::sync_composer(&mut self.app, w);
                    focus = Some(id.clone());
                }
            }
        }
        self.scene_rev += 1;
        // Blocks dashboards bind state to props: re-sync after every
        // press so mouse, keyboard and inspector paths all stay live.
        // Defensive on non-dashboard apps.
        dashboard::refresh_dashboard(&mut self.app);
        pi_panel::refresh_pi_panel(&mut self.app);
        (hit, r.success, focus)
    }

    /// M3 mouse: the shared press path, then trace.
    fn m3_click(&mut self) {
        let (hit, acted, focus) = self.apply_press(self.cursor.0, self.cursor.1);
        self.trace.record(
            "m3.click",
            &format!("hit={hit:?} acted={acted} focus={focus:?}"),
            self.app.revision,
            None,
        );
        println!("M3 click: hit={hit:?} acted={acted} focus={focus:?}");
    }

    /// Swap the M1 demo tree for the 035 gallery tree (called once, on open).
    fn enter_gallery(&mut self) {
        self.gallery = Some(hamesh_gallery::Gallery::new());
        self.refresh_gallery();
        self.trace.record(
            "gallery.enter",
            "ListView + Picker workspace live; same intents as the headless drill",
            self.app.revision,
            None,
        );
    }

    /// Recompose the gallery tree from widget state after any intent.
    /// The driver owns layout budgets and popup anchoring, so paint
    /// stays pure: it renders whatever tree is current.
    fn refresh_gallery(&mut self) {
        if let Some(g) = self.gallery.as_mut() {
            let (app, _) = hamesh_gallery::refresh(g, self.size.0 as f32, self.size.1 as f32);
            self.app = app;
            self.scene_rev += 1;
        }
    }

    /// Gallery mouse: the driver press path (shared Hit router), then
    /// trace. Inspector and OS clicks converge here.
    fn gallery_press(&mut self, x: f32, y: f32) {
        let (_, scene) = self.layout_and_scene();
        let note = match self.gallery.as_mut() {
            Some(g) => hamesh_gallery::press(g, &self.app, &scene, x, y),
            None => "gallery not entered".to_string(),
        };
        self.refresh_gallery();
        self.trace
            .record("gallery.click", &note, self.app.revision, None);
        println!("GALLERY click: {note}");
    }

    fn gallery_click(&mut self) {
        let (x, y) = (self.cursor.0, self.cursor.1);
        self.gallery_press(x, y);
    }

    /// Shared driver-application core for gallery: inspector keys and
    /// OS keys converge here. Never quits: the window arm owns exit.
    fn gallery_keys_apply(&mut self, keys: Vec<WKey>) -> String {
        let Some(g) = self.gallery.as_mut() else {
            return "gallery not entered".to_string();
        };
        let names: Vec<String> = g.layer_rows.iter().map(|r| r.label.clone()).collect();
        let mut notes = Vec::new();
        for k in keys {
            notes.push(hamesh_gallery::key(g, k, &names));
        }
        let note = notes.join(" | ");
        self.refresh_gallery();
        self.trace
            .record("gallery.key", &note, self.app.revision, None);
        note
    }

    /// Gallery keyboard: map the winit event onto the shared intent core.
    fn gallery_key(&mut self, event: &winit::event::KeyEvent, event_loop: &ActiveEventLoop) {
        use hamesh_widgets::Key;
        let Some(g) = self.gallery.as_mut() else {
            return;
        };
        if event.physical_key == PhysicalKey::Code(KeyCode::F12) {
            self.dump_inspection();
            return;
        }
        let mut keys: Vec<Key> = Vec::new();
        if let Some(text) = event.text.as_ref() {
            for c in text.chars() {
                // Tab arrives as text too; the physical key wins below.
                if c != '\r' && c != '\n' && c != '\t' {
                    keys.push(Key::Char(c));
                }
            }
        }
        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => keys.push(Key::Up),
            PhysicalKey::Code(KeyCode::ArrowDown) => keys.push(Key::Down),
            PhysicalKey::Code(KeyCode::PageUp) => keys.push(Key::PageUp),
            PhysicalKey::Code(KeyCode::PageDown) => keys.push(Key::PageDown),
            PhysicalKey::Code(KeyCode::Home) => keys.push(Key::Home),
            PhysicalKey::Code(KeyCode::End) => keys.push(Key::End),
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                keys.push(Key::Enter)
            }
            PhysicalKey::Code(KeyCode::Backspace) => keys.push(Key::Backspace),
            PhysicalKey::Code(KeyCode::Tab) => keys.push(Key::Tab),
            PhysicalKey::Code(KeyCode::Escape) => keys.push(Key::Escape),
            _ => {}
        }
        if keys.is_empty() {
            return;
        }
        // Escape with a closed popup quits; otherwise the popup owns it.
        let escape_quits = keys.len() == 1 && matches!(keys[0], Key::Escape) && !g.fonts.is_open();
        let note = self.gallery_keys_apply(keys);
        println!("GALLERY key: {note}");
        if escape_quits {
            event_loop.exit();
        }
    }

    /// Swap the M1 demo tree for the 036 tasks tree (called once, on open).
    fn enter_tasks(&mut self) {
        self.tasks = Some(hamesh_tasks::Tasks::new());
        self.refresh_tasks();
        self.trace.record(
            "tasks.enter",
            "task queue + filter + inspector live; shared Hit router as the drill",
            self.app.revision,
            None,
        );
    }

    /// Recompose the tasks tree from widget state after any intent.
    fn refresh_tasks(&mut self) {
        if let Some(t) = self.tasks.as_mut() {
            let (app, _) = hamesh_tasks::refresh(t, self.size.0 as f32, self.size.1 as f32);
            self.app = app;
            self.scene_rev += 1;
        }
    }

    /// Swap the demo tree for the 037 console tree (called once, on open).
    fn enter_console(&mut self) {
        self.console = Some(hamesh_console::Console::new());
        self.refresh_console();
        if self.pi.is_some() {
            if let Some(link) = self.pi.as_ref() {
                let _ = link.job_tx.send(PiJob::GetState);
            }
        }
        self.trace.record(
            "console.enter",
            "Pi session console live; same intents as the M3 drill",
            self.app.revision,
            None,
        );
    }

    /// Recompose the console tree from driver state after any intent.
    /// Streaming/messages mirror the live worker link every frame.
    fn refresh_console(&mut self) {
        if let Some(c) = self.console.as_mut() {
            let streaming = self.pi.as_ref().is_some_and(|l| l.live_index.is_some());
            let messages = c.widgets.feed.messages.len() as i64;
            let (model, session) = (c.status.model.clone(), c.status.session.clone());
            c.set_status(&model, &session, messages, streaming);
            let (app, _) = hamesh_console::refresh(c, self.size.0 as f32, self.size.1 as f32);
            self.app = app;
            self.scene_rev += 1;
        }
    }

    /// Swap the demo tree for the 037 editor tree (called once, on open).
    /// An optional `--file=` path opens immediately; otherwise the
    /// buffer starts empty and fileless.
    fn enter_editor(&mut self, file: Option<String>) {
        let mut e = hamesh_editor::Editor::new();
        let note = match file {
            Some(p) => match e.open(std::path::Path::new(&p)) {
                Ok(n) | Err(n) => n,
            },
            None => "editor open (no file)".to_string(),
        };
        self.editor = Some(e);
        self.refresh_editor();
        self.trace.record(
            "editor.enter",
            &format!("file editor live ({note}); same intents as tasks"),
            self.app.revision,
            None,
        );
    }

    /// Recompose the editor tree from driver state after any intent.
    fn refresh_editor(&mut self) {
        if let Some(e) = self.editor.as_mut() {
            let (app, _) = hamesh_editor::refresh(e, self.size.0 as f32, self.size.1 as f32);
            self.app = app;
            self.scene_rev += 1;
        }
    }

    /// Spawn (or respawn) the background Pi worker. Records the session
    /// for later attach defaults; dropping the old link retires its
    /// worker (channel close ends its loop, client drop kills the child).
    fn spawn_pi_link(&mut self, session: Option<SessionCfg>) {
        let (job_tx, job_rx) = std::sync::mpsc::channel::<PiJob>();
        let (evt_tx, evt_rx) = std::sync::mpsc::channel::<PiEvt>();
        let for_worker = session.clone();
        std::thread::spawn(move || pi_worker(job_rx, evt_tx, for_worker));
        self.pi = Some(PiLink {
            job_tx,
            evt_rx,
            live_index: None,
            follow: false,
        });
        self.pi_session = session.clone();
        self.trace.record(
            "pi.worker",
            "background pi worker spawned (--pi)",
            self.app.revision,
            None,
        );
        match session {
            Some(s) => {
                self.trace.record(
                    "pi.session",
                    &format!("attach sid={} dir={} cwd={}", s.id, s.dir, s.cwd),
                    self.app.revision,
                    None,
                );
                println!(
                    "PI session mode: attach sid={} (persistent, history restores)",
                    s.id
                );
            }
            None => {
                println!("PI live mode: composer submits to a background pi worker");
            }
        }
    }

    /// Translate the driver's abort intent into a worker cancel — but
    /// only while a run is actually live. Pointer click, keyboard Enter
    /// and inspector key all share this gate.
    fn console_abort(&mut self) -> String {
        if self.pi.as_ref().is_some_and(|l| l.live_index.is_some()) {
            if let Some(link) = self.pi.as_ref() {
                let _ = link.job_tx.send(PiJob::Abort);
            }
            "abort requested (cancel sent)".to_string()
        } else {
            "abort stale (run already settled)".to_string()
        }
    }

    /// Console mouse: the driver press path (shared Hit router), then
    /// trace. Driver state decides; the abort intent goes through
    /// `console_abort` so every input path shares one gate.
    fn console_press(&mut self, x: f32, y: f32) {
        let (_, scene) = self.layout_and_scene();
        let note = match self.console.as_mut() {
            Some(c) => hamesh_console::press(c, &self.app, &scene, x, y),
            None => "console not entered".to_string(),
        };
        let note = if note == "abort requested" {
            self.console_abort()
        } else if note == "attach requested" {
            self.console_attach()
        } else {
            note
        };
        self.refresh_console();
        self.trace
            .record("console.click", &note, self.app.revision, None);
        println!("CONSOLE click: {note}");
    }

    /// Translate the driver's attach intent into a worker respawn on a
    /// (possibly different) persistent session. Refused while a run is
    /// live — abort first. The feed clears and history restores through
    /// the same History event as launch attach.
    fn console_attach(&mut self) -> String {
        if self.pi.as_ref().is_some_and(|l| l.live_index.is_some()) {
            return "attach refused: run live, abort first".to_string();
        }
        let req = match self.console.as_mut().and_then(|c| c.pending_attach.take()) {
            Some(r) => r,
            None => return "attach refused: no request".to_string(),
        };
        let fallback = self.pi_session.clone().unwrap_or(SessionCfg {
            id: String::new(),
            dir: String::new(),
            cwd: String::new(),
        });
        let dir = if req.dir.is_empty() {
            fallback.dir
        } else {
            req.dir
        };
        let cwd = if req.cwd.is_empty() {
            fallback.cwd
        } else {
            req.cwd
        };
        if req.id.is_empty() || dir.is_empty() || cwd.is_empty() {
            return "usage: /attach <id> [dir] [cwd] (dir/cwd fall back to the current session)"
                .to_string();
        }
        // Retire the old worker first: dropping its link ends its loop
        // and kills its child, so two workers never share a session.
        self.pi = None;
        if let Some(c) = self.console.as_mut() {
            c.widgets.feed.messages.clear();
            c.widgets.feed.scroll_px = 0.0;
        }
        self.spawn_pi_link(Some(SessionCfg {
            id: req.id.clone(),
            dir,
            cwd,
        }));
        self.refresh_console();
        format!("attached sid={} (history restoring)", req.id)
    }

    /// Console keyboard: map the winit event onto the shared key core,
    /// exactly like the M3 path. F12 dumps Eye evidence anywhere.
    fn console_key(&mut self, event: &winit::event::KeyEvent) {
        if event.physical_key == PhysicalKey::Code(KeyCode::F12) {
            self.dump_inspection();
            return;
        }
        if self.console.is_none() {
            return;
        }
        let mut keys: Vec<WKey> = Vec::new();
        if let Some(text) = event.text.as_ref() {
            for c in text.chars() {
                // Enter arrives as text too; the physical key wins below.
                if c != '\r' && c != '\n' {
                    keys.push(WKey::Char(c));
                }
            }
        }
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                keys.push(WKey::Enter)
            }
            PhysicalKey::Code(KeyCode::Backspace) => keys.push(WKey::Backspace),
            PhysicalKey::Code(KeyCode::Delete) => keys.push(WKey::Delete),
            PhysicalKey::Code(KeyCode::ArrowLeft) => keys.push(WKey::Left),
            PhysicalKey::Code(KeyCode::ArrowRight) => keys.push(WKey::Right),
            PhysicalKey::Code(KeyCode::ArrowUp) => keys.push(WKey::Up),
            PhysicalKey::Code(KeyCode::ArrowDown) => keys.push(WKey::Down),
            PhysicalKey::Code(KeyCode::Home) => keys.push(WKey::Home),
            PhysicalKey::Code(KeyCode::End) => keys.push(WKey::End),
            PhysicalKey::Code(KeyCode::Tab) => keys.push(if self.shift_held {
                WKey::ShiftTab
            } else {
                WKey::Tab
            }),
            _ => {}
        }
        if keys.is_empty() {
            return;
        }
        let _ = self.console_keys_apply(keys);
    }

    /// Shared key-application core for the console: the winit path, the
    /// inspector path and the smoke script all land here — one rule for
    /// hand, eye and keyboard, mirroring `m3_keys_apply`.
    fn console_keys_apply(&mut self, keys: Vec<WKey>) -> String {
        // Escape during a live run aborts it; Escape when idle is a
        // no-op here — the window arm still quits.
        if keys.contains(&WKey::Escape) && self.pi.as_ref().is_some_and(|l| l.live_index.is_some())
        {
            if let Some(link) = self.pi.as_ref() {
                let _ = link.job_tx.send(PiJob::Abort);
            }
            self.trace.record(
                "console.pi_abort",
                "escape during live run",
                self.app.revision,
                None,
            );
        }
        // Detect a submission AFTER the batch: typing and Enter land in
        // one go, so a pre-read of the composer always sees it empty. A
        // submission happened iff Enter was applied while the composer
        // was focused and the feed grew by a user message.
        let had_enter = keys.contains(&WKey::Enter);
        let before = self.console.as_ref().map(|c| {
            (
                c.widgets.focus.focused() == Some(c.widgets.composer_id.as_str()),
                c.widgets.feed.messages.len(),
            )
        });
        let mut last = String::new();
        for k in &keys {
            if let Some(c) = self.console.as_mut() {
                last = hamesh_console::key(c, &mut self.app, k);
            }
        }
        self.refresh_console();
        // Keyboard Enter on the Abort button raises the same intent as a
        // pointer press; translate it through the shared abort gate.
        if last == "abort requested" {
            last = self.console_abort();
            self.refresh_console();
        }
        // Same for Attach: the slash-command submit and the Attach
        // button converge here with pointer and inspector key paths.
        if last == "attach requested" {
            last = self.console_attach();
            self.refresh_console();
        }
        let submitted: Option<String> = match (had_enter, before) {
            (true, Some((true, before_len))) => self
                .console
                .as_ref()
                .filter(|c| c.widgets.feed.messages.len() > before_len)
                .and_then(|c| {
                    c.widgets
                        .feed
                        .messages
                        .iter()
                        .rev()
                        .find(|m| m.role == "user")
                })
                .map(|m| m.text.clone()),
            _ => None,
        };
        if let (Some(text), Some(link)) = (submitted, self.pi.as_ref()) {
            let _ = link.job_tx.send(PiJob::Prompt(text.clone()));
            self.trace.record(
                "console.pi_queued",
                &format!("submitted {text:?} to worker"),
                self.app.revision,
                None,
            );
        }
        // One routing: every key batch leaves the same trace shape here.
        self.trace.record(
            "console.key",
            &format!("-> {last}"),
            self.app.revision,
            None,
        );
        last
    }

    /// Editor mouse: the driver press path (shared Hit router), then
    /// trace. Save runs through the driver's disabled gate, so pointer
    /// and keyboard refuse identically when clean or fileless.
    fn editor_press(&mut self, x: f32, y: f32) {
        let (_, scene) = self.layout_and_scene();
        let note = match self.editor.as_mut() {
            Some(e) => hamesh_editor::press(e, &self.app, &scene, x, y),
            None => "editor not entered".to_string(),
        };
        self.refresh_editor();
        self.trace
            .record("editor.click", &note, self.app.revision, None);
        println!("EDITOR click: {note}");
    }

    /// Editor keyboard: map the winit event onto the shared key core.
    /// F12 dumps Eye evidence anywhere.
    fn editor_key(&mut self, event: &winit::event::KeyEvent) {
        if event.physical_key == PhysicalKey::Code(KeyCode::F12) {
            self.dump_inspection();
            return;
        }
        if self.editor.is_none() {
            return;
        }
        let mut keys: Vec<WKey> = Vec::new();
        if let Some(text) = event.text.as_ref() {
            for c in text.chars() {
                // Enter arrives as text too; the physical key wins below.
                if c != '\r' && c != '\n' {
                    keys.push(WKey::Char(c));
                }
            }
        }
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                keys.push(WKey::Enter)
            }
            PhysicalKey::Code(KeyCode::Backspace) => keys.push(WKey::Backspace),
            PhysicalKey::Code(KeyCode::Delete) => keys.push(WKey::Delete),
            PhysicalKey::Code(KeyCode::ArrowLeft) => keys.push(WKey::Left),
            PhysicalKey::Code(KeyCode::ArrowRight) => keys.push(WKey::Right),
            PhysicalKey::Code(KeyCode::ArrowUp) => keys.push(WKey::Up),
            PhysicalKey::Code(KeyCode::ArrowDown) => keys.push(WKey::Down),
            PhysicalKey::Code(KeyCode::Home) => keys.push(WKey::Home),
            PhysicalKey::Code(KeyCode::End) => keys.push(WKey::End),
            PhysicalKey::Code(KeyCode::PageUp) => keys.push(WKey::PageUp),
            PhysicalKey::Code(KeyCode::PageDown) => keys.push(WKey::PageDown),
            PhysicalKey::Code(KeyCode::Tab) => keys.push(if self.shift_held {
                WKey::ShiftTab
            } else {
                WKey::Tab
            }),
            _ => {}
        }
        if keys.is_empty() {
            return;
        }
        let _ = self.editor_keys_apply(keys);
    }

    /// Shared key-application core for the editor: the winit path, the
    /// inspector path and the smoke script all land here — one rule for
    /// hand, eye and keyboard, mirroring the console core.
    fn editor_keys_apply(&mut self, keys: Vec<WKey>) -> String {
        let mut last = String::new();
        for k in &keys {
            if let Some(e) = self.editor.as_mut() {
                last = hamesh_editor::key(e, k.clone());
            }
        }
        self.refresh_editor();
        self.trace
            .record("editor.key", &format!("-> {last}"), self.app.revision, None);
        last
    }

    /// Tasks mouse: the driver press path (shared Hit router), then
    /// trace. Inspector and OS clicks converge here.
    fn tasks_press(&mut self, x: f32, y: f32) {
        let (_, scene) = self.layout_and_scene();
        let note = match self.tasks.as_mut() {
            Some(t) => hamesh_tasks::press(t, &self.app, &scene, x, y),
            None => "tasks not entered".to_string(),
        };
        self.refresh_tasks();
        self.trace
            .record("tasks.click", &note, self.app.revision, None);
        println!("TASKS click: {note}");
    }

    fn tasks_click(&mut self) {
        let (x, y) = (self.cursor.0, self.cursor.1);
        self.tasks_press(x, y);
    }

    /// Shared driver-application core for tasks: inspector keys and
    /// OS keys converge here. Never quits: the window arm owns exit.
    fn tasks_keys_apply(&mut self, keys: Vec<WKey>) -> String {
        let Some(t) = self.tasks.as_mut() else {
            return "tasks not entered".to_string();
        };
        let mut notes = Vec::new();
        for k in keys {
            notes.push(hamesh_tasks::key(t, k));
        }
        let note = notes.join(" | ");
        self.refresh_tasks();
        self.trace
            .record("tasks.key", &note, self.app.revision, None);
        note
    }

    /// Tasks keyboard: map the winit event onto the shared intent core.
    /// Shift+Tab walks focus back (three zones: list, filter, actions).
    fn tasks_key(&mut self, event: &winit::event::KeyEvent, event_loop: &ActiveEventLoop) {
        use hamesh_widgets::Key;
        let Some(t) = self.tasks.as_mut() else {
            return;
        };
        if event.physical_key == PhysicalKey::Code(KeyCode::F12) {
            self.dump_inspection();
            return;
        }
        let mut keys: Vec<Key> = Vec::new();
        if let Some(text) = event.text.as_ref() {
            for c in text.chars() {
                if c != '\r' && c != '\n' && c != '\t' {
                    keys.push(Key::Char(c));
                }
            }
        }
        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => keys.push(Key::Up),
            PhysicalKey::Code(KeyCode::ArrowDown) => keys.push(Key::Down),
            PhysicalKey::Code(KeyCode::PageUp) => keys.push(Key::PageUp),
            PhysicalKey::Code(KeyCode::PageDown) => keys.push(Key::PageDown),
            PhysicalKey::Code(KeyCode::Home) => keys.push(Key::Home),
            PhysicalKey::Code(KeyCode::End) => keys.push(Key::End),
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                keys.push(Key::Enter)
            }
            PhysicalKey::Code(KeyCode::Backspace) => keys.push(Key::Backspace),
            PhysicalKey::Code(KeyCode::Tab) => keys.push(if self.shift_held {
                Key::ShiftTab
            } else {
                Key::Tab
            }),
            PhysicalKey::Code(KeyCode::Escape) => keys.push(Key::Escape),
            _ => {}
        }
        if keys.is_empty() {
            return;
        }
        let escape_quits = keys.len() == 1 && matches!(keys[0], Key::Escape) && !t.filter.is_open();
        let note = self.tasks_keys_apply(keys);
        println!("TASKS key: {note}");
        if escape_quits {
            event_loop.exit();
        }
    }

    /// Swap the M1 demo tree for the 036 design-system sheet (called once, on open).
    fn enter_sheet(&mut self) {
        self.sheet = Some(hamesh_sheet::Sheet::new());
        self.refresh_sheet();
        self.trace.record(
            "sheet.enter",
            "design-system sheet live; same Hit/Key core as the headless drill",
            self.app.revision,
            None,
        );
    }

    /// Recompose the sheet from widget state after any intent.
    fn refresh_sheet(&mut self) {
        if let Some(t) = self.sheet.as_mut() {
            let (app, _) = hamesh_sheet::refresh(t, self.size.0 as f32, self.size.1 as f32);
            self.app = app;
            self.scene_rev += 1;
        }
    }

    /// Sheet mouse: the driver press path (shared Hit router), then
    /// trace. Inspector and OS clicks converge here.
    fn sheet_press(&mut self, x: f32, y: f32) {
        let (_, scene) = self.layout_and_scene();
        let note = match self.sheet.as_mut() {
            Some(t) => hamesh_sheet::press(t, &self.app, &scene, x, y),
            None => "sheet not entered".to_string(),
        };
        self.refresh_sheet();
        self.trace
            .record("sheet.click", &note, self.app.revision, None);
        println!("SHEET click: {note}");
    }

    fn sheet_click(&mut self) {
        let (x, y) = (self.cursor.0, self.cursor.1);
        self.sheet_press(x, y);
    }

    /// Shared driver-application core for the sheet: inspector keys
    /// and OS keys converge here. Never quits: the window arm owns exit.
    fn sheet_keys_apply(&mut self, keys: Vec<WKey>) -> String {
        let Some(t) = self.sheet.as_mut() else {
            return "sheet not entered".to_string();
        };
        let mut notes = Vec::new();
        for k in keys {
            notes.push(hamesh_sheet::key(t, k));
        }
        let note = notes.join(" | ");
        self.refresh_sheet();
        self.trace
            .record("sheet.key", &note, self.app.revision, None);
        note
    }

    /// Sheet keyboard: map the winit event onto the shared intent core.
    fn sheet_key(&mut self, event: &winit::event::KeyEvent, event_loop: &ActiveEventLoop) {
        use hamesh_widgets::Key;
        if self.sheet.is_none() {
            return;
        };
        if event.physical_key == PhysicalKey::Code(KeyCode::F12) {
            self.dump_inspection();
            return;
        }
        let mut keys: Vec<Key> = Vec::new();
        if let Some(text) = event.text.as_ref() {
            for c in text.chars() {
                if c != '\r' && c != '\n' && c != '\t' {
                    keys.push(Key::Char(c));
                }
            }
        }
        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => keys.push(Key::Up),
            PhysicalKey::Code(KeyCode::ArrowDown) => keys.push(Key::Down),
            PhysicalKey::Code(KeyCode::ArrowLeft) => keys.push(Key::Left),
            PhysicalKey::Code(KeyCode::ArrowRight) => keys.push(Key::Right),
            PhysicalKey::Code(KeyCode::PageUp) => keys.push(Key::PageUp),
            PhysicalKey::Code(KeyCode::PageDown) => keys.push(Key::PageDown),
            PhysicalKey::Code(KeyCode::Home) => keys.push(Key::Home),
            PhysicalKey::Code(KeyCode::End) => keys.push(Key::End),
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                keys.push(Key::Enter)
            }
            PhysicalKey::Code(KeyCode::Backspace) => keys.push(Key::Backspace),
            PhysicalKey::Code(KeyCode::Tab) => keys.push(if self.shift_held {
                Key::ShiftTab
            } else {
                Key::Tab
            }),
            PhysicalKey::Code(KeyCode::Escape) => keys.push(Key::Escape),
            _ => {}
        }
        if keys.is_empty() {
            return;
        }
        // The sheet has no popup to own Escape: it quits like the
        // gallery and tasks demos do when theirs is dismissed.
        if keys.len() == 1 && matches!(keys[0], Key::Escape) {
            event_loop.exit();
            return;
        }
        let note = self.sheet_keys_apply(keys);
        println!("SHEET key: {note}");
    }

    /// M3 keyboard: map the winit event onto the shared `handle_key` core.
    /// Returns true when the event was consumed by the M3 path.
    fn m3_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if self.widgets.is_none() {
            return false;
        }
        let mut keys: Vec<WKey> = Vec::new();
        if let Some(text) = event.text.as_ref() {
            for c in text.chars() {
                // Enter arrives as text too; the physical key wins below.
                if c != '\r' && c != '\n' {
                    keys.push(WKey::Char(c));
                }
            }
        }
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                keys.push(WKey::Enter)
            }
            PhysicalKey::Code(KeyCode::Backspace) => keys.push(WKey::Backspace),
            PhysicalKey::Code(KeyCode::Delete) => keys.push(WKey::Delete),
            PhysicalKey::Code(KeyCode::ArrowLeft) => keys.push(WKey::Left),
            PhysicalKey::Code(KeyCode::ArrowRight) => keys.push(WKey::Right),
            PhysicalKey::Code(KeyCode::ArrowUp) => keys.push(WKey::Up),
            PhysicalKey::Code(KeyCode::ArrowDown) => keys.push(WKey::Down),
            PhysicalKey::Code(KeyCode::Home) => keys.push(WKey::Home),
            PhysicalKey::Code(KeyCode::End) => keys.push(WKey::End),
            PhysicalKey::Code(KeyCode::Tab) => keys.push(if self.shift_held {
                WKey::ShiftTab
            } else {
                WKey::Tab
            }),
            _ => {}
        }
        if keys.is_empty() {
            return false;
        }
        let _ = self.m3_keys_apply(keys);
        true
    }

    /// KeyMsg (inspector protocol) to the shared key vocabulary. One
    /// mechanical mapping for every mode; drivers own the semantics.
    fn keymsg_to_wkey(key: &inspect_srv::KeyMsg) -> WKey {
        match key {
            inspect_srv::KeyMsg::Char(c) => WKey::Char(*c),
            inspect_srv::KeyMsg::Enter => WKey::Enter,
            inspect_srv::KeyMsg::Backspace => WKey::Backspace,
            inspect_srv::KeyMsg::Delete => WKey::Delete,
            inspect_srv::KeyMsg::Left => WKey::Left,
            inspect_srv::KeyMsg::Right => WKey::Right,
            inspect_srv::KeyMsg::Up => WKey::Up,
            inspect_srv::KeyMsg::Down => WKey::Down,
            inspect_srv::KeyMsg::Home => WKey::Home,
            inspect_srv::KeyMsg::End => WKey::End,
            inspect_srv::KeyMsg::Tab => WKey::Tab,
            inspect_srv::KeyMsg::ShiftTab => WKey::ShiftTab,
            inspect_srv::KeyMsg::Escape => WKey::Escape,
        }
    }

    /// Shared key-application core: the winit path maps events to keys,
    /// the inspector path maps KeyMsg to keys, both land here.
    fn m3_keys_apply(&mut self, keys: Vec<WKey>) -> String {
        // Escape during a live run aborts it: the window Esc arm and the
        // external key-Escape path both land here, so one rule covers
        // hand, eye, and keyboard. Escape when idle is a no-op here —
        // the window arm still quits. handle_key already no-ops Escape.
        if keys.contains(&WKey::Escape) && self.pi.as_ref().is_some_and(|l| l.live_index.is_some())
        {
            if let Some(link) = self.pi.as_ref() {
                let _ = link.job_tx.send(PiJob::Abort);
            }
            self.trace.record(
                "m3.pi_abort",
                "escape during live run",
                self.app.revision,
                None,
            );
        }
        // Detect a submission AFTER the loop: batches (smoke script,
        // pasted typing) type the text and press Enter in one go, so a
        // pre-read of the composer always sees it empty. A submission
        // happened iff Enter was applied while focused and the feed
        // grew by a user message. The UI thread owns this decision;
        // the worker only receives already-authorized text.
        let had_enter = keys.contains(&WKey::Enter);
        let before = self.widgets.as_ref().map(|w| {
            (
                w.focus.focused() == Some(w.composer_id.as_str()),
                w.feed.messages.len(),
            )
        });
        let mut last = String::new();
        for k in &keys {
            if let Some(w) = self.widgets.as_mut() {
                last = handle_key(&mut self.app, w, k);
            }
        }
        self.refresh_feed();
        let submitted: Option<String> = match (had_enter, before) {
            (true, Some((true, before_len))) => self
                .widgets
                .as_ref()
                .filter(|w| w.feed.messages.len() > before_len)
                .and_then(|w| w.feed.messages.iter().rev().find(|m| m.role == "user"))
                .map(|m| m.text.clone()),
            _ => None,
        };
        if let (Some(text), Some(link)) = (submitted, self.pi.as_ref()) {
            let _ = link.job_tx.send(PiJob::Prompt(text.clone()));
            self.trace.record(
                "m3.pi_queued",
                &format!("submitted {text:?} to worker"),
                self.app.revision,
                None,
            );
        }
        // One routing: every key batch — winit, inspector, smoke, tests —
        // leaves the same trace shape here.
        self.trace
            .record("m3.key", &format!("-> {last}"), self.app.revision, None);
        last
    }
}

/// A Pi job for the worker thread: prompt the model, or abort the run.
/// Pure M4 verdict mapping (headless-tested): PASS exits 0, FAIL 1,
/// INCOMPLETE (still streaming at the deadline) 2.
fn pi_verdict(asked: bool, answered: bool, streaming: bool) -> (&'static str, i32) {
    if answered {
        ("PASS", 0)
    } else if !asked {
        ("FAIL(no-submission)", 1)
    } else if streaming {
        ("INCOMPLETE(still-streaming)", 2)
    } else {
        ("FAIL(no-token-in-reply)", 1)
    }
}

#[derive(Debug)]
enum PiJob {
    Prompt(String),
    /// Cancel the current run. Wired when Esc stops meaning quit during
    /// a live run (M4 follow-up); the worker already honors it today.
    #[allow(dead_code)]
    Abort,
    /// Fetch live engine state (model, session, counts). The console
    /// requests it on open and after every run settles; the driver only
    /// ever paints what this round trip reported.
    GetState,
}

/// Streamed Pi activity back to the UI thread. The UI never blocks on
/// the model: it drains these between frames and folds text into the
/// feed like any other state change.
#[derive(Debug)]
enum PiEvt {
    Started,
    Delta(String),
    Done { text: String, aborted: bool },
    Failed(String),
    State(hamesh_pi::EngineState),
    History(Vec<hamesh_pi::HistoryMessage>),
    ToolNote(String),
}

/// Persistent-session launch config, all from CLI flags. `None` means
/// the existing ephemeral worker (`--pi` alone).
#[derive(Clone, Debug)]
struct SessionCfg {
    id: String,
    dir: String,
    cwd: String,
}

fn session_cfg() -> Option<SessionCfg> {
    let arg =
        |name: &str| std::env::args().find_map(|a| a.strip_prefix(name).map(|s| s.to_string()));
    Some(SessionCfg {
        id: arg("--session-id=")?,
        dir: arg("--session-dir=")?,
        cwd: arg("--session-cwd=")?,
    })
}

/// Link to the background Pi worker. `live_index` is the feed slot
/// currently being streamed into (`None` when idle).
struct PiLink {
    job_tx: std::sync::mpsc::Sender<PiJob>,
    evt_rx: std::sync::mpsc::Receiver<PiEvt>,
    live_index: Option<usize>,
    follow: bool,
}

/// Run `pi --mode rpc` off-thread: prompt, stream deltas, report done.
/// With a session config, attaches to a persistent Pi session and ships
/// its restored history first. Never touches UI state; everything
/// crosses on channels.
fn pi_worker(
    jobs: std::sync::mpsc::Receiver<PiJob>,
    events: std::sync::mpsc::Sender<PiEvt>,
    session: Option<SessionCfg>,
) {
    let client = match &session {
        Some(s) => hamesh_pi::PiClient::spawn_session("pi", Some(&s.cwd), &s.dir, &s.id),
        None => hamesh_pi::PiClient::spawn_ephemeral("pi", None),
    };
    let mut client = match client {
        Ok(c) => c,
        Err(e) => {
            let _ = events.send(PiEvt::Failed(format!("spawn: {e}")));
            return;
        }
    };
    if let Err(e) = client.get_state(std::time::Duration::from_secs(20)) {
        let _ = events.send(PiEvt::Failed(format!("handshake: {e}")));
        return;
    }
    if session.is_some() {
        match client.get_history(std::time::Duration::from_secs(30)) {
            Ok(h) => {
                let _ = events.send(PiEvt::History(h));
            }
            Err(e) => {
                let _ = events.send(PiEvt::Failed(format!("history: {e}")));
                return;
            }
        }
    }
    while let Ok(job) = jobs.recv() {
        match job {
            PiJob::Abort => {
                let _ = client.abort(std::time::Duration::from_secs(10));
            }
            PiJob::GetState => match client.get_state(std::time::Duration::from_secs(20)) {
                Ok(st) => {
                    let _ = events.send(PiEvt::State(st));
                }
                Err(e) => {
                    let _ = events.send(PiEvt::Failed(format!("state: {e}")));
                }
            },
            PiJob::Prompt(text) => {
                let _ = events.send(PiEvt::Started);
                let r = client.prompt_and_wait(
                    &text,
                    &mut |p| {
                        let _ = events.send(match p {
                            hamesh_pi::PiProgress::Delta(d) => PiEvt::Delta(d),
                            hamesh_pi::PiProgress::ToolNote(n) => PiEvt::ToolNote(n),
                        });
                    },
                    std::time::Duration::from_secs(180),
                );
                match r {
                    Ok((text, aborted)) => {
                        let _ = events.send(PiEvt::Done { text, aborted });
                    }
                    Err(e) => {
                        let _ = events.send(PiEvt::Failed(format!("{e}")));
                    }
                }
            }
        }
    }
}

impl HameshWindowApp {
    /// M3 smoke script: run once after open — focus the composer through
    /// the real focus path, type, submit, scroll. Proves the keyboard path
    /// end-to-end without a human at the keyboard.
    /// Drain finished/partial Pi output without blocking: each event
    /// appends to (or creates) an assistant message, then the visible
    /// window rebuilds. Runs every frame; all I/O stays on the worker.
    /// Empty assistant bubbles are skipped by the renderer; text streams
    /// in as deltas arrive.
    fn drain_pi(&mut self) {
        enum Act {
            Started,
            Delta(String),
            Done { text: String, aborted: bool },
            Failed(String),
            State(hamesh_pi::EngineState),
            History(Vec<hamesh_pi::HistoryMessage>),
            ToolNote(String),
        }
        let mut acts = Vec::new();
        if let Some(link) = self.pi.as_mut() {
            while let Ok(ev) = link.evt_rx.try_recv() {
                match ev {
                    PiEvt::Started => acts.push(Act::Started),
                    PiEvt::Delta(d) => acts.push(Act::Delta(d)),
                    PiEvt::Done { text, aborted } => acts.push(Act::Done { text, aborted }),
                    PiEvt::Failed(e) => acts.push(Act::Failed(e)),
                    PiEvt::State(st) => acts.push(Act::State(st)),
                    PiEvt::History(h) => acts.push(Act::History(h)),
                    PiEvt::ToolNote(n) => acts.push(Act::ToolNote(n)),
                }
            }
        }
        if acts.is_empty() {
            return;
        }
        let mut changed = false;
        for a in acts {
            match a {
                Act::Started => {
                    if let Some(w) = self.widgets.as_mut() {
                        let i = w.feed.push_live("agent");
                        if let Some(link) = self.pi.as_mut() {
                            link.live_index = Some(i);
                            link.follow = true;
                        }
                        changed = true;
                    } else if let Some(c) = self.console.as_mut() {
                        let i = c.widgets.feed.push_live("agent");
                        if let Some(link) = self.pi.as_mut() {
                            link.live_index = Some(i);
                            link.follow = true;
                        }
                        changed = true;
                    }
                }
                Act::Delta(d) => {
                    let idx = self.pi.as_ref().and_then(|l| l.live_index);
                    if let (Some(i), Some(w)) = (idx, self.widgets.as_mut()) {
                        if let Some(m) = w.feed.messages.get_mut(i) {
                            m.text.push_str(&d);
                            changed = true;
                        }
                    } else if let (Some(i), Some(c)) = (idx, self.console.as_mut()) {
                        if let Some(m) = c.widgets.feed.messages.get_mut(i) {
                            m.text.push_str(&d);
                            changed = true;
                        }
                    }
                }
                Act::Done { text, aborted } => {
                    if let Some(w) = self.widgets.as_mut() {
                        if let Some(link) = self.pi.as_mut() {
                            link.live_index = None;
                        }
                        let vp = w.feed_viewport.max(200.0);
                        w.feed.scroll_to_bottom(vp);
                        self.trace.record(
                            "m3.pi_done",
                            &format!(
                                "aborted={aborted} chars={} messages={}",
                                text.chars().count(),
                                w.feed.messages.len()
                            ),
                            self.app.revision,
                            None,
                        );
                        changed = true;
                    } else if let Some(c) = self.console.as_mut() {
                        if let Some(link) = self.pi.as_mut() {
                            link.live_index = None;
                        }
                        let vp = c.widgets.feed_viewport.max(200.0);
                        c.widgets.feed.scroll_to_bottom(vp);
                        self.trace.record(
                            "console.pi_done",
                            &format!(
                                "aborted={aborted} chars={} messages={}",
                                text.chars().count(),
                                c.widgets.feed.messages.len()
                            ),
                            self.app.revision,
                            None,
                        );
                        if let Some(link) = self.pi.as_ref() {
                            let _ = link.job_tx.send(PiJob::GetState);
                        }
                        changed = true;
                    }
                }
                Act::Failed(e) => {
                    if let Some(w) = self.widgets.as_mut() {
                        if let Some(link) = self.pi.as_mut() {
                            link.live_index = None;
                        }
                        w.feed.push("system", &format!("Pi error: {e}"));
                        let vp = w.feed_viewport.max(200.0);
                        w.feed.scroll_to_bottom(vp);
                        changed = true;
                    } else if let Some(c) = self.console.as_mut() {
                        if let Some(link) = self.pi.as_mut() {
                            link.live_index = None;
                        }
                        c.widgets.feed.push("system", &format!("Pi error: {e}"));
                        let vp = c.widgets.feed_viewport.max(200.0);
                        c.widgets.feed.scroll_to_bottom(vp);
                        changed = true;
                    }
                }
                Act::State(st) => {
                    if let Some(c) = self.console.as_mut() {
                        let messages = c.widgets.feed.messages.len() as i64;
                        let streaming = self.pi.as_ref().is_some_and(|l| l.live_index.is_some());
                        c.set_status(
                            st.model_name.as_deref().unwrap_or("—"),
                            st.session_id.as_deref().unwrap_or("—"),
                            messages,
                            streaming,
                        );
                        changed = true;
                    }
                }
                Act::ToolNote(n) => {
                    if let Some(c) = self.console.as_mut() {
                        c.widgets.feed.push("tool", &n);
                        let vp = c.widgets.feed_viewport.max(200.0);
                        c.widgets.feed.scroll_to_bottom(vp);
                        self.trace
                            .record("console.tool", &n, self.app.revision, None);
                        changed = true;
                    }
                }
                Act::History(h) => {
                    if let Some(c) = self.console.as_mut() {
                        for m in &h {
                            c.widgets.feed.push(&m.role, &m.text);
                        }
                        let vp = c.widgets.feed_viewport.max(200.0);
                        c.widgets.feed.scroll_to_bottom(vp);
                        self.trace.record(
                            "console.history",
                            &format!("restored {} messages", h.len()),
                            self.app.revision,
                            None,
                        );
                        if let Some(link) = self.pi.as_ref() {
                            let _ = link.job_tx.send(PiJob::GetState);
                        }
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.refresh_feed();
            self.refresh_console();
        }
    }

    /// End-of-smoke verdict for the M4 loop (only under --pi): the smoke
    /// prompt demands the token HAMESH-SMOKE-OK back, so a later agent
    /// message containing it proves the full path carried real content,
    /// not just event counts.
    fn m3_pi_assert(&mut self) -> i32 {
        const TOKEN: &str = "HAMESH-SMOKE-OK";
        let (asked, answered, streaming) = match self.widgets.as_ref() {
            Some(w) => {
                let mut asked_at: Option<usize> = None;
                let mut answered = false;
                for (i, m) in w.feed.messages.iter().enumerate() {
                    if m.role == "user" && m.text.contains(TOKEN) {
                        asked_at = Some(i);
                    }
                    if m.role == "agent"
                        && m.text.contains(TOKEN)
                        && asked_at.is_some_and(|a| i > a)
                    {
                        answered = true;
                    }
                }
                let streaming = self.pi.as_ref().is_some_and(|l| l.live_index.is_some());
                (asked_at.is_some(), answered, streaming)
            }
            None => (false, false, false),
        };
        let (verdict, code) = pi_verdict(asked, answered, streaming);
        self.trace
            .record("m3.pi_assert", verdict, self.app.revision, None);
        println!("SMOKE-PI-{verdict}");
        code
    }

    /// IME wiring (decision 019): thin delegation to the tested Composer
    /// model. Preedit/Commit land only when the composer is focused;
    /// everything is traced. The winit arm itself is deliberately tiny —
    /// it cannot be driven without a real OS IME, so the model owns the
    /// behavior and the headless tests own the proof.
    fn ime_event(&mut self, ime: Ime) {
        let focused = self
            .widgets
            .as_ref()
            .is_some_and(|w| w.focus.focused() == Some(w.composer_id.as_str()));
        match ime {
            Ime::Enabled => {
                self.trace
                    .record("m3.ime", "enabled", self.app.revision, None);
            }
            Ime::Preedit(s, _) => {
                if !focused {
                    return;
                }
                if let Some(w) = self.widgets.as_mut() {
                    w.composer.set_preedit(&s);
                    hamesh_widgets::sync_composer(&mut self.app, w);
                }
                self.refresh_feed();
                self.trace.record(
                    "m3.ime_preedit",
                    &format!("{} chars", s.chars().count()),
                    self.app.revision,
                    None,
                );
                self.update_ime_cursor_area();
            }
            Ime::Commit(s) => {
                if !focused {
                    return;
                }
                if let Some(w) = self.widgets.as_mut() {
                    w.composer.commit(&s);
                    hamesh_widgets::sync_composer(&mut self.app, w);
                }
                self.refresh_feed();
                self.trace.record(
                    "m3.ime_commit",
                    &format!("{} chars", s.chars().count()),
                    self.app.revision,
                    None,
                );
            }
            Ime::Disabled => {
                if let Some(w) = self.widgets.as_mut() {
                    w.composer.clear_preedit();
                    hamesh_widgets::sync_composer(&mut self.app, w);
                }
                self.refresh_feed();
                self.trace
                    .record("m3.ime", "disabled", self.app.revision, None);
            }
        }
    }

    /// Tell the OS where the caret is so the candidate window opens in a
    /// sane place. Approximate (composer rect + caret_x); the OS clamps.
    /// Refreshed on every Preedit; caret moves between compositions reuse
    /// the last position.
    fn update_ime_cursor_area(&self) {
        let (rect, cx) = match self.widgets.as_ref() {
            Some(w) => {
                let layout =
                    hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
                let rect = layout.nodes.get(&w.composer_id).map(|n| n.rect.clone());
                (rect, w.composer.caret_x(700.0))
            }
            None => (None, 0.0),
        };
        if let (Some(r), Some(win)) = (rect, self.window.as_ref()) {
            use winit::dpi::{LogicalPosition, LogicalSize};
            win.set_ime_cursor_area(
                LogicalPosition::new((r.x + cx) as f64, r.y as f64),
                LogicalSize::new((r.w - cx).max(8.0) as f64, r.h as f64),
            );
        }
    }

    fn m3_smoke_script(&mut self) {
        let layout = hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
        let (ok, hit) = match self.widgets.as_mut() {
            Some(w) => {
                let f = hamesh_automation::focus_human(&mut self.app, w, &layout, "composer");
                (f.success, f.hit_component)
            }
            None => (false, None),
        };
        self.trace.record(
            "m3.smoke_focus",
            &format!("success={ok} hit={hit:?}"),
            self.app.revision,
            None,
        );
        if !ok {
            return;
        }
        // One batch through the shared core (queues a Pi job when --pi).
        // Under --pi the prompt demands a verifiable token back, so the
        // end-of-smoke check can assert the reply instead of eyeballing it.
        let prompt = if self.use_pi {
            "Reply with only this exact token and nothing else: HAMESH-SMOKE-OK"
        } else {
            "m3 smoke"
        };
        let mut keys: Vec<WKey> = prompt.chars().map(WKey::Char).collect();
        keys.push(WKey::Enter);
        let out = self.m3_keys_apply(keys);
        self.trace.record(
            "m3.smoke_keys",
            &format!("applied keys -> {out}"),
            self.app.revision,
            None,
        );
        if let Some(w) = self.widgets.as_mut() {
            w.feed.scroll_by(-120.0, w.feed_viewport.max(200.0));
        }
        self.refresh_feed();
        let (n_msgs, composer_text, focused) = match self.widgets.as_ref() {
            Some(w) => (
                w.feed.messages.len(),
                w.composer.text.clone(),
                w.focus.focused().unwrap_or("?").to_string(),
            ),
            None => (0, "<no widgets>".to_string(), "?".to_string()),
        };
        self.trace.record(
            "m3.smoke_script",
            &format!("typed + submitted + scrolled; messages={n_msgs} composer={composer_text:?} focus={focused}"),
            self.app.revision,
            None,
        );
    }

    /// Console smoke script: run once after open — focus the composer
    /// through the real focus path, type, submit, scroll. Same proof as
    /// the M3 script, through the console driver instead of M3 widgets.
    fn console_smoke_script(&mut self) {
        let layout = hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
        let (ok, hit) = match self.console.as_mut() {
            Some(c) => {
                let f = hamesh_automation::focus_human(
                    &mut self.app,
                    &mut c.widgets,
                    &layout,
                    "composer",
                );
                (f.success, f.hit_component)
            }
            None => (false, None),
        };
        self.trace.record(
            "console.smoke_focus",
            &format!("success={ok} hit={hit:?}"),
            self.app.revision,
            None,
        );
        if !ok {
            return;
        }
        // One batch through the shared core (queues a Pi job when --pi).
        // Under --pi the prompt demands a verifiable token back, so the
        // end-of-smoke check can assert the reply instead of eyeballing it.
        let prompt = if self.use_pi {
            "Reply with only this exact token and nothing else: HAMESH-SMOKE-OK"
        } else {
            "console smoke"
        };
        let mut keys: Vec<WKey> = prompt.chars().map(WKey::Char).collect();
        keys.push(WKey::Enter);
        let out = self.console_keys_apply(keys);
        self.trace.record(
            "console.smoke_keys",
            &format!("applied keys -> {out}"),
            self.app.revision,
            None,
        );
        if let Some(c) = self.console.as_mut() {
            c.widgets
                .feed
                .scroll_by(-120.0, c.widgets.feed_viewport.max(200.0));
        }
        self.refresh_console();
        let (n_msgs, composer_text, focused) = match self.console.as_ref() {
            Some(c) => (
                c.widgets.feed.messages.len(),
                c.widgets.composer.text.clone(),
                c.widgets.focus.focused().unwrap_or("?").to_string(),
            ),
            None => (0, "<no console>".to_string(), "?".to_string()),
        };
        self.trace.record(
            "console.smoke_script",
            &format!("typed + submitted + scrolled; messages={n_msgs} composer={composer_text:?} focus={focused}"),
            self.app.revision,
            None,
        );
    }

    /// End-of-smoke verdict for the console loop (only under --pi):
    /// same token proof as the M3 loop, read off the console feed.
    fn console_pi_assert(&mut self) -> i32 {
        const TOKEN: &str = "HAMESH-SMOKE-OK";
        let (asked, answered, streaming) = match self.console.as_ref() {
            Some(c) => {
                let mut asked_at: Option<usize> = None;
                let mut answered = false;
                for (i, m) in c.widgets.feed.messages.iter().enumerate() {
                    if m.role == "user" && m.text.contains(TOKEN) {
                        asked_at = Some(i);
                    }
                    if m.role == "agent"
                        && m.text.contains(TOKEN)
                        && asked_at.is_some_and(|a| i > a)
                    {
                        answered = true;
                    }
                }
                let streaming = self.pi.as_ref().is_some_and(|l| l.live_index.is_some());
                (asked_at.is_some(), answered, streaming)
            }
            None => (false, false, false),
        };
        let (verdict, code) = pi_verdict(asked, answered, streaming);
        self.trace
            .record("console.pi_assert", verdict, self.app.revision, None);
        println!("SMOKE-PI-{verdict}");
        code
    }

    /// Editor smoke script: run once after open — write a fixture file,
    /// open it, move, type, and save through the keyboard gate. Proves
    /// the file round trip end-to-end without a human at the keyboard.
    fn editor_smoke_script(&mut self) {
        let path = std::env::temp_dir().join("hamesh-editor-smoke.txt");
        let fixture = "smoke one\nsmoke two\n";
        let (mut opened, mut saved_note) = (false, String::new());
        if std::fs::write(&path, fixture).is_ok() {
            if let Some(e) = self.editor.as_mut() {
                opened = e.open(&path).is_ok();
            }
        }
        self.trace.record(
            "editor.smoke_open",
            &format!("opened={opened}"),
            self.app.revision,
            None,
        );
        if !opened {
            return;
        }
        // Down to line 2, Enter to edit, type, Tab to Save, Enter.
        let _ = self.editor_keys_apply(vec![WKey::Down, WKey::Enter]);
        let type_keys: Vec<WKey> = " edited".chars().map(WKey::Char).collect();
        let _ = self.editor_keys_apply(type_keys);
        let _ = self.editor_keys_apply(vec![WKey::Tab, WKey::Enter]);
        if let Some(e) = self.editor.as_mut() {
            saved_note = format!("dirty={}", e.save_enabled());
        }
        self.trace.record(
            "editor.smoke_script",
            &format!("edited + saved through the gate; after save {saved_note}"),
            self.app.revision,
            None,
        );
    }

    /// End-of-smoke verdict for the editor: the driver lines and the
    /// file bytes must both show the edited line.
    fn editor_smoke_assert(&mut self) -> i32 {
        const EXPECTED: &str = "smoke one\nsmoke two edited\n";
        let (asked, answered) = match self.editor.as_ref() {
            Some(e) => {
                let driver_ok = e.lines.join("\n") + "\n" == EXPECTED && !e.save_enabled();
                let path = std::env::temp_dir().join("hamesh-editor-smoke.txt");
                let file_ok = std::fs::read_to_string(&path)
                    .map(|s| s == EXPECTED)
                    .unwrap_or(false);
                (driver_ok, file_ok)
            }
            None => (false, false),
        };
        let (verdict, code) = if answered && asked {
            ("PASS", 0)
        } else if !asked {
            ("FAIL(no-edit-in-driver)", 1)
        } else {
            ("FAIL(file-bytes-differ)", 1)
        };
        self.trace
            .record("editor.smoke_assert", verdict, self.app.revision, None);
        println!("SMOKE-EDITOR-{verdict}");
        code
    }

    /// Attach current revision stamps to an Ok payload (uniform envelope).
    fn stamped(&self, mut v: serde_json::Value) -> inspect_srv::OpOutcome {
        if let Some(o) = v.as_object_mut() {
            o.insert(
                "app_rev".to_string(),
                serde_json::Value::from(self.app.revision),
            );
            o.insert(
                "scene_rev".to_string(),
                serde_json::Value::from(self.scene_rev),
            );
        }
        inspect_srv::OpOutcome::Ok(v)
    }

    fn err(&self, code: &'static str, message: String) -> inspect_srv::OpOutcome {
        inspect_srv::OpOutcome::Err {
            code,
            message,
            app_rev: self.app.revision,
            scene_rev: self.scene_rev,
        }
    }

    fn check_stale(&self, expect: &Option<u64>) -> Option<inspect_srv::OpOutcome> {
        match expect {
            Some(e) if *e != self.app.revision => Some(self.err(
                "STALE",
                format!(
                    "app moved: expected rev {e}, live rev {}",
                    self.app.revision
                ),
            )),
            _ => None,
        }
    }

    /// Apply one external op on the UI thread, using the same handlers
    /// as real input. Takes the whole request so `wait` can park its
    /// responder instead of answering immediately.
    fn apply_op(&mut self, req: inspect_srv::OpRequest) {
        use hamesh_automation::type_human;
        use inspect_srv::{KeyMsg, Op, OpOutcome};
        let inspect_srv::OpRequest { op, respond } = req;
        let reply: Option<OpOutcome> = match op {
            Op::Map => {
                let (layout, scene) = self.layout_and_scene();
                let map = hamesh_inspect::compact_map(&self.app, &layout, &scene);
                let v = serde_json::to_value(&map).unwrap_or(serde_json::Value::Null);
                Some(self.stamped(v))
            }
            Op::Detail { id } => {
                let (layout, scene) = self.layout_and_scene();
                match hamesh_inspect::detail(&self.app, &layout, Some(&scene), &id) {
                    Some(d) => {
                        let v = serde_json::to_value(&d).unwrap_or(serde_json::Value::Null);
                        Some(self.stamped(v))
                    }
                    None => Some(self.err("NOT_FOUND", format!("unknown component {id}"))),
                }
            }
            Op::ClickId { id, expect_rev } => {
                if let Some(e) = self.check_stale(&expect_rev) {
                    Some(e)
                } else {
                    let layout =
                        hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
                    let center = layout
                        .nodes
                        .get(&id)
                        .map(|n| (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0));
                    match center {
                        Some((x, y)) => {
                            if self.use_console {
                                // Driver-owned state: the console press
                                // path interprets the hit, not the tree.
                                self.console_press(x, y);
                                Some(self.stamped(serde_json::json!({
                                    "console": "pressed",
                                    "mechanism": "HumanEquivalent",
                                })))
                            } else if self.use_editor {
                                self.editor_press(x, y);
                                Some(self.stamped(serde_json::json!({
                                    "editor": "pressed",
                                    "mechanism": "HumanEquivalent",
                                })))
                            } else if self.use_tasks {
                                self.tasks_press(x, y);
                                Some(self.stamped(serde_json::json!({
                                    "tasks": "pressed",
                                    "mechanism": "HumanEquivalent",
                                })))
                            } else if self.use_gallery {
                                self.gallery_press(x, y);
                                Some(self.stamped(serde_json::json!({
                                    "gallery": "pressed",
                                    "mechanism": "HumanEquivalent",
                                })))
                            } else if self.use_sheet {
                                self.sheet_press(x, y);
                                Some(self.stamped(serde_json::json!({
                                    "sheet": "pressed",
                                    "mechanism": "HumanEquivalent",
                                })))
                            } else {
                                let (hit, acted, focus) = self.apply_press(x, y);
                                self.trace.record(
                                    "ext.click",
                                    &format!("id={id} hit={hit:?} acted={acted}"),
                                    self.app.revision,
                                    None,
                                );
                                Some(self.stamped(serde_json::json!({
                                    "hit": hit, "acted": acted, "focus": focus,
                                    "mechanism": "HumanEquivalent",
                                })))
                            }
                        }
                        None => Some(self.err("NOT_FOUND", format!("no layout rect for {id}"))),
                    }
                }
            }
            Op::ClickAt { x, y, expect_rev } => {
                if let Some(e) = self.check_stale(&expect_rev) {
                    Some(e)
                } else if self.use_console {
                    self.console_press(x, y);
                    Some(self.stamped(serde_json::json!({
                        "console": "pressed",
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_editor {
                    self.editor_press(x, y);
                    Some(self.stamped(serde_json::json!({
                        "editor": "pressed",
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_tasks {
                    self.tasks_press(x, y);
                    Some(self.stamped(serde_json::json!({
                        "tasks": "pressed",
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_gallery {
                    self.gallery_press(x, y);
                    Some(self.stamped(serde_json::json!({
                        "gallery": "pressed",
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_sheet {
                    self.sheet_press(x, y);
                    Some(self.stamped(serde_json::json!({
                        "sheet": "pressed",
                        "mechanism": "HumanEquivalent",
                    })))
                } else {
                    let (hit, acted, focus) = self.apply_press(x, y);
                    self.trace.record(
                        "ext.click",
                        &format!("at=({x:.0},{y:.0}) hit={hit:?} acted={acted}"),
                        self.app.revision,
                        None,
                    );
                    Some(self.stamped(serde_json::json!({
                        "hit": hit, "acted": acted, "focus": focus,
                        "mechanism": "HumanEquivalent",
                    })))
                }
            }
            Op::Key { key, expect_rev } => {
                if let Some(e) = self.check_stale(&expect_rev) {
                    Some(e)
                } else if self.use_console && self.console.is_some() {
                    // Console owns its widgets: same key vocabulary and
                    // HumanEquivalent mechanism as the M3 path.
                    let k = match key {
                        KeyMsg::Char(c) => WKey::Char(c),
                        KeyMsg::Enter => WKey::Enter,
                        KeyMsg::Backspace => WKey::Backspace,
                        KeyMsg::Delete => WKey::Delete,
                        KeyMsg::Left => WKey::Left,
                        KeyMsg::Right => WKey::Right,
                        KeyMsg::Up => WKey::Up,
                        KeyMsg::Down => WKey::Down,
                        KeyMsg::Home => WKey::Home,
                        KeyMsg::End => WKey::End,
                        KeyMsg::Tab => WKey::Tab,
                        KeyMsg::ShiftTab => WKey::ShiftTab,
                        KeyMsg::Escape => WKey::Escape,
                    };
                    let out = self.console_keys_apply(vec![k]);
                    self.trace
                        .record("ext.key", &format!("-> {out}"), self.app.revision, None);
                    Some(self.stamped(serde_json::json!({
                        "result": out,
                        "focused": self.console.as_ref().and_then(|c| c.widgets.focus.focused()),
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_editor && self.editor.is_some() {
                    // Editor owns its driver state: same key vocabulary.
                    let k = match key {
                        KeyMsg::Char(c) => WKey::Char(c),
                        KeyMsg::Enter => WKey::Enter,
                        KeyMsg::Backspace => WKey::Backspace,
                        KeyMsg::Delete => WKey::Delete,
                        KeyMsg::Left => WKey::Left,
                        KeyMsg::Right => WKey::Right,
                        KeyMsg::Up => WKey::Up,
                        KeyMsg::Down => WKey::Down,
                        KeyMsg::Home => WKey::Home,
                        KeyMsg::End => WKey::End,
                        KeyMsg::Tab => WKey::Tab,
                        KeyMsg::ShiftTab => WKey::ShiftTab,
                        KeyMsg::Escape => WKey::Escape,
                    };
                    let out = self.editor_keys_apply(vec![k]);
                    self.trace
                        .record("ext.key", &format!("-> {out}"), self.app.revision, None);
                    Some(self.stamped(serde_json::json!({
                        "result": out,
                        "focused": self.editor.as_ref().map(|e| e.focused_zone()),
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_tasks && self.tasks.is_some() {
                    // Tasks owns its driver state (list/filter/actions zones).
                    let out = self.tasks_keys_apply(vec![Self::keymsg_to_wkey(&key)]);
                    self.trace
                        .record("ext.key", &format!("-> {out}"), self.app.revision, None);
                    Some(self.stamped(serde_json::json!({
                        "result": out,
                        "focused": self.tasks.as_ref().map(|t| t.focused_zone()),
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_gallery && self.gallery.is_some() {
                    // Gallery owns its driver state (layers/picker zones).
                    let out = self.gallery_keys_apply(vec![Self::keymsg_to_wkey(&key)]);
                    self.trace
                        .record("ext.key", &format!("-> {out}"), self.app.revision, None);
                    Some(self.stamped(serde_json::json!({
                        "result": out,
                        "focused": self.gallery.as_ref().map(|g| g.focused_zone()),
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_sheet && self.sheet.is_some() {
                    // Sheet owns its driver state (selection/buttons zones).
                    let out = self.sheet_keys_apply(vec![Self::keymsg_to_wkey(&key)]);
                    self.trace
                        .record("ext.key", &format!("-> {out}"), self.app.revision, None);
                    Some(self.stamped(serde_json::json!({
                        "result": out,
                        "focused": self.sheet.as_ref().map(|t| t.focused_zone()),
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.widgets.is_none() {
                    Some(self.err(
                        "NO_M3",
                        "keyboard needs an interactive mode (m3, console, editor, tasks, gallery or sheet)".to_string(),
                    ))
                } else {
                    let k = match key {
                        KeyMsg::Char(c) => WKey::Char(c),
                        KeyMsg::Enter => WKey::Enter,
                        KeyMsg::Backspace => WKey::Backspace,
                        KeyMsg::Delete => WKey::Delete,
                        KeyMsg::Left => WKey::Left,
                        KeyMsg::Right => WKey::Right,
                        KeyMsg::Up => WKey::Up,
                        KeyMsg::Down => WKey::Down,
                        KeyMsg::Home => WKey::Home,
                        KeyMsg::End => WKey::End,
                        KeyMsg::Tab => WKey::Tab,
                        KeyMsg::ShiftTab => WKey::ShiftTab,
                        KeyMsg::Escape => WKey::Escape,
                    };
                    let out = self.m3_keys_apply(vec![k]);
                    self.trace
                        .record("ext.key", &format!("-> {out}"), self.app.revision, None);
                    Some(self.stamped(serde_json::json!({
                        "result": out,
                        "focused": self.widgets.as_ref().and_then(|w| w.focus.focused()),
                        "mechanism": "HumanEquivalent",
                    })))
                }
            }
            Op::Type { id, text } => {
                if self.use_editor && self.editor.is_some() {
                    // Editor types straight into the composer and commits
                    // to the current line: the inspector `type` op is a
                    // paste with no focus tricks.
                    let note = {
                        let Some(e) = self.editor.as_mut() else {
                            return;
                        };
                        e.type_text(&text)
                    };
                    self.refresh_editor();
                    self.trace.record(
                        "ext.type",
                        &format!("id={id} editor: {note}"),
                        self.app.revision,
                        None,
                    );
                    Some(self.stamped(serde_json::json!({
                        "success": true,
                        "hit": "edit-field",
                        "notes": [note],
                        "mechanism": "HumanEquivalent",
                    })))
                } else if self.use_console && self.console.is_some() {
                    let layout =
                        hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
                    // type_human borrows console widgets + app disjointly.
                    let r = {
                        let Some(c) = self.console.as_mut() else {
                            return;
                        };
                        type_human(&mut self.app, &mut c.widgets, &layout, &id, &text)
                    };
                    self.refresh_console();
                    self.trace.record(
                        "ext.type",
                        &format!("id={id} success={} {:?}", r.success, r.notes),
                        self.app.revision,
                        None,
                    );
                    Some(self.stamped(serde_json::json!({
                        "success": r.success,
                        "hit": r.hit_component,
                        "notes": r.notes,
                        "mechanism": format!("{:?}", r.mechanism),
                    })))
                } else if self.widgets.is_none() {
                    Some(self.err(
                        "NO_M3",
                        "typing needs an interactive mode (m3, console or editor)".to_string(),
                    ))
                } else {
                    let layout =
                        hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
                    // type_human borrows widgets + app disjointly via split.
                    let r = {
                        let Some(w) = self.widgets.as_mut() else {
                            return;
                        };
                        type_human(&mut self.app, w, &layout, &id, &text)
                    };
                    self.refresh_feed();
                    self.trace.record(
                        "ext.type",
                        &format!("id={id} success={} {:?}", r.success, r.notes),
                        self.app.revision,
                        None,
                    );
                    Some(self.stamped(serde_json::json!({
                        "success": r.success,
                        "hit": r.hit_component,
                        "notes": r.notes,
                        "mechanism": format!("{:?}", r.mechanism),
                    })))
                }
            }
            Op::Snapshot { path, overlay } => {
                let (_layout, scene) = self.layout_and_scene();
                let mut frame = hamesh_render::render(&scene);
                // Diagnostic overlay: guides for one node, painted
                // into this capture only. App state, hit-testing and
                // normal rendering are untouched.
                let geom = overlay.as_deref().and_then(|id| {
                    scene.nodes.iter().find(|n| n.id == id).map(|n| {
                        hamesh_render::overlay::draw_guides(&mut frame.pixmap, n);
                        serde_json::json!({
                            "id": n.id,
                            "rect": n.rect,
                            "text_geom": {
                                "origin": n.origin,
                                "content": n.content,
                                "halign": n.halign,
                                "advance": n.advance,
                                "ink": n.ink,
                                "trim": n.trim,
                                "metrics": n.metrics,
                                "baseline": n.baseline,
                                "lines": n.lines,
                                "font_family": n.font_family,
                                "font_vars": n.font_vars,
                                "font_resolved": n.font_resolved,
                                "font_style": n.font_style,
                                "tracking": n.tracking,
                            },
                        })
                    })
                });
                if overlay.is_some() && geom.is_none() {
                    Some(self.err(
                        "NOT_FOUND",
                        format!(
                            "no scene node for overlay {}",
                            overlay.as_deref().unwrap_or("?")
                        ),
                    ))
                } else {
                    match hamesh_render::save_png(&frame, &path) {
                        Ok(()) => Some(self.stamped(serde_json::json!({
                            "path": path,
                            "overlay": geom,
                        }))),
                        Err(e) => Some(self.err("IO", e)),
                    }
                }
            }
            Op::Wait {
                app_rev,
                timeout_ms,
            } => {
                if self.app.revision != app_rev {
                    Some(self.stamped(serde_json::json!({ "waited": false })))
                } else {
                    self.pending_waits.push(PendingWait {
                        target_rev: app_rev,
                        deadline: Instant::now() + std::time::Duration::from_millis(timeout_ms),
                        respond,
                    });
                    // Parked: the responder now lives in the wait queue.
                    return;
                }
            }
        };
        if let Some(out) = reply {
            let _ = respond.send(out);
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

impl ApplicationHandler<UserEvent> for HameshWindowApp {
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
        if self.use_gallery {
            self.enter_gallery();
            println!("GALLERY open: ListView + Picker workspace — click, wheel, type, Tab, F12 dumps Eye evidence, Esc quits");
        }
        if self.use_tasks {
            self.enter_tasks();
            println!("TASKS open: task queue + filter + inspector — click, wheel, type, Tab, F12 dumps Eye evidence, Esc quits");
        }
        if self.use_sheet {
            self.enter_sheet();
            println!("SHEET open: design-system sheet — click, wheel, type, Tab, F12 dumps Eye evidence, Esc quits");
        }
        if self.use_m3 {
            self.enter_m3();
            self.refresh_feed();
            println!("M3 chat open: Tab focuses, type in composer, Enter submits, wheel scrolls");
            // IME allowed while the chat surface is up; Preedit/Commit
            // are accepted only when the composer itself is focused
            // (see ime_event), and the cursor area follows the caret.
            if let Some(win) = self.window.as_ref() {
                win.set_ime_allowed(true);
            }
        }
        if self.use_console {
            self.enter_console();
            println!("CONSOLE open: Pi session console — click, type, Tab, F12 dumps Eye evidence, Esc quits/aborts");
        }
        if self.use_editor {
            let file =
                std::env::args().find_map(|a| a.strip_prefix("--file=").map(|s| s.to_string()));
            self.enter_editor(file);
            println!("EDITOR open: file editor — click, type, Tab, F12 dumps Eye evidence, Esc cancels/quits");
        }
        if self.use_pi && self.pi.is_none() {
            self.spawn_pi_link(session_cfg());
        }
        if self.use_inspect && !self.inspect_serving {
            if let Some(proxy) = self.proxy.clone() {
                match std::net::TcpListener::bind("127.0.0.1:0") {
                    Ok(lis) => {
                        let port = lis.local_addr().map(|a| a.port()).unwrap_or(0);
                        let token = inspect_srv::dev_token();
                        match inspect_srv::write_discovery(port, &token) {
                            Ok(()) => {
                                let send = move |req: inspect_srv::OpRequest| {
                                    proxy.send_event(UserEvent::Inspect(req)).is_ok()
                                };
                                std::thread::spawn(move || inspect_srv::serve(lis, token, send));
                                self.inspect_serving = true;
                                self.trace.record(
                                    "inspect.serve",
                                    &format!("127.0.0.1:{port} (dev token)"),
                                    self.app.revision,
                                    None,
                                );
                                println!(
                                    "INSPECT serving on 127.0.0.1:{port} — local dev only, token required"
                                );
                            }
                            Err(e) => self.trace.record(
                                "inspect.discovery_failed",
                                &format!("{e}"),
                                self.app.revision,
                                None,
                            ),
                        }
                    }
                    Err(e) => self.trace.record(
                        "inspect.bind_failed",
                        &format!("{e}"),
                        self.app.revision,
                        None,
                    ),
                }
            }
        }
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

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Inspect(req) => self.apply_op(req),
        }
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
                if self.use_gallery {
                    self.refresh_gallery();
                }
                if self.use_tasks {
                    self.refresh_tasks();
                }
                if self.use_sheet {
                    self.refresh_sheet();
                }
                if self.use_console {
                    self.refresh_console();
                }
                if self.use_editor {
                    self.refresh_editor();
                }
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
                if self.use_gallery {
                    let (_, scene) = self.layout_and_scene();
                    let changed = match self.gallery.as_mut() {
                        Some(g) => hamesh_gallery::hover(
                            g,
                            &scene,
                            self.cursor.0,
                            self.cursor.1,
                            &self.app,
                        ),
                        None => false,
                    };
                    if changed {
                        self.refresh_gallery();
                    }
                }
                if self.use_tasks {
                    let (_, scene) = self.layout_and_scene();
                    let changed = match self.tasks.as_mut() {
                        Some(t) => {
                            hamesh_tasks::hover(t, &scene, self.cursor.0, self.cursor.1, &self.app)
                        }
                        None => false,
                    };
                    if changed {
                        self.refresh_tasks();
                    }
                }
                if self.use_sheet {
                    let (_, scene) = self.layout_and_scene();
                    let changed = match self.sheet.as_mut() {
                        Some(t) => {
                            hamesh_sheet::hover(t, &scene, self.cursor.0, self.cursor.1, &self.app)
                        }
                        None => false,
                    };
                    if changed {
                        self.refresh_sheet();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    if self.use_gallery {
                        self.gallery_click();
                    } else if self.use_tasks {
                        self.tasks_click();
                    } else if self.use_sheet {
                        self.sheet_click();
                    } else if self.use_m3 {
                        self.m3_click();
                    } else if self.use_console {
                        self.console_press(self.cursor.0, self.cursor.1);
                    } else if self.use_editor {
                        self.editor_press(self.cursor.0, self.cursor.1);
                    } else {
                        let (hit, acted, focus) = self.apply_press(self.cursor.0, self.cursor.1);
                        sync_label(&mut self.app);
                        self.trace.record(
                            "hand.mouse_click",
                            &format!(
                                "at=({:.0},{:.0}) hit={:?} acted={} focus={focus:?}",
                                self.cursor.0, self.cursor.1, hit, acted
                            ),
                            self.app.revision,
                            None,
                        );
                        println!(
                            "HAND mouse: hit={hit:?} acted={acted} counter={:?}",
                            self.app.state.get("counter")
                        );
                    }
                }
            }
            WindowEvent::Ime(ime) => {
                self.ime_event(ime);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if self.use_gallery {
                    self.gallery_key(&event, event_loop);
                    return;
                }
                if self.use_tasks {
                    self.tasks_key(&event, event_loop);
                    return;
                }
                if self.use_sheet {
                    self.sheet_key(&event, event_loop);
                    return;
                }
                if self.use_m3 && event.physical_key != PhysicalKey::Code(KeyCode::Escape) {
                    // M3 path consumes everything except Escape (quit).
                    self.m3_key(&event);
                    return;
                }
                if self.use_console && event.physical_key != PhysicalKey::Code(KeyCode::Escape) {
                    // Console path consumes everything except Escape (abort/quit).
                    self.console_key(&event);
                    return;
                }
                if self.use_editor && event.physical_key != PhysicalKey::Code(KeyCode::Escape) {
                    // Editor path consumes everything except Escape: the
                    // window arm routes Escape to cancel-or-quit below.
                    self.editor_key(&event);
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        // Live run: Escape aborts it and stays open (one
                        // rule via m3_keys_apply). Idle: Escape quits.
                        if self.use_m3 && self.pi.as_ref().is_some_and(|l| l.live_index.is_some()) {
                            self.m3_keys_apply(vec![WKey::Escape]);
                        } else if self.use_console
                            && self.pi.as_ref().is_some_and(|l| l.live_index.is_some())
                        {
                            self.console_keys_apply(vec![WKey::Escape]);
                        } else if self.use_editor {
                            // Edit zone: Escape cancels and stays open.
                            // Elsewhere: Escape quits (no live runs here).
                            if self.editor.as_ref().is_some_and(|e| e.focus == 1) {
                                self.editor_keys_apply(vec![WKey::Escape]);
                            } else {
                                event_loop.exit();
                            }
                        } else {
                            event_loop.exit();
                        }
                    }
                    PhysicalKey::Code(KeyCode::Space) => {
                        let layout = hamesh_layout::compute(
                            &self.app,
                            self.size.0 as f32,
                            self.size.1 as f32,
                        );
                        let target = if self.use_blocks_pi {
                            "pi-action"
                        } else if self.use_console {
                            "console-abort"
                        } else if self.use_editor {
                            "btn-save"
                        } else if self.use_blocks {
                            "dash-action"
                        } else {
                            "btn_inc"
                        };
                        let r = click_component_human(&mut self.app, &layout, target);
                        sync_label(&mut self.app);
                        dashboard::refresh_dashboard(&mut self.app);
                        pi_panel::refresh_pi_panel(&mut self.app);
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
            WindowEvent::ModifiersChanged(mods) => {
                self.shift_held = mods.state().shift_key();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.use_gallery {
                    // Positive delta scrolls up, for both winit delta kinds.
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                    };
                    let (layout, _) = self.layout_and_scene();
                    let scrolled = match self.gallery.as_mut() {
                        Some(g) => {
                            hamesh_gallery::wheel(g, &layout, self.cursor.0, self.cursor.1, dy)
                        }
                        None => false,
                    };
                    if scrolled {
                        self.refresh_gallery();
                        self.trace.record(
                            "gallery.wheel",
                            &format!("dy={dy}"),
                            self.app.revision,
                            None,
                        );
                    }
                } else if self.use_tasks {
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                    };
                    let (layout, _) = self.layout_and_scene();
                    let scrolled = match self.tasks.as_mut() {
                        Some(t) => {
                            hamesh_tasks::wheel(t, &layout, self.cursor.0, self.cursor.1, dy)
                        }
                        None => false,
                    };
                    if scrolled {
                        self.refresh_tasks();
                        self.trace.record(
                            "tasks.wheel",
                            &format!("dy={dy}"),
                            self.app.revision,
                            None,
                        );
                    }
                } else if self.use_sheet {
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                    };
                    let (layout, _) = self.layout_and_scene();
                    let scrolled = match self.sheet.as_mut() {
                        Some(t) => {
                            hamesh_sheet::wheel(t, &layout, self.cursor.0, self.cursor.1, dy)
                        }
                        None => false,
                    };
                    if scrolled {
                        self.refresh_sheet();
                        self.trace.record(
                            "sheet.wheel",
                            &format!("dy={dy}"),
                            self.app.revision,
                            None,
                        );
                    }
                } else if self.use_m3 {
                    let amount = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                        winit::event::MouseScrollDelta::PixelDelta(p) => -(p.y as f32),
                    };
                    let scrolled = if let Some(w) = self.widgets.as_mut() {
                        w.feed.scroll_by(-amount, 400.0);
                        Some(w.feed.scroll_px)
                    } else {
                        None
                    };
                    if let Some(px) = scrolled {
                        self.refresh_feed();
                        self.trace.record(
                            "m3.wheel",
                            &format!("scroll {px}"),
                            self.app.revision,
                            None,
                        );
                    }
                } else if self.use_console {
                    let amount = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                        winit::event::MouseScrollDelta::PixelDelta(p) => -(p.y as f32),
                    };
                    let scrolled = if let Some(c) = self.console.as_mut() {
                        c.widgets.feed.scroll_by(-amount, 400.0);
                        Some(c.widgets.feed.scroll_px)
                    } else {
                        None
                    };
                    if let Some(px) = scrolled {
                        self.refresh_console();
                        self.trace.record(
                            "console.wheel",
                            &format!("scroll {px}"),
                            self.app.revision,
                            None,
                        );
                    }
                } else if self.use_editor {
                    let amount = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 60.0,
                        winit::event::MouseScrollDelta::PixelDelta(p) => -(p.y as f32),
                    };
                    let layout =
                        hamesh_layout::compute(&self.app, self.size.0 as f32, self.size.1 as f32);
                    let scrolled = if let Some(e) = self.editor.as_mut() {
                        let (w, h) = (self.cursor.0, self.cursor.1);
                        Some(hamesh_editor::wheel(e, &layout, w, h, -amount))
                    } else {
                        None
                    };
                    if scrolled.is_some_and(|s| s) {
                        self.refresh_editor();
                        self.trace.record(
                            "editor.wheel",
                            &format!("dy={amount}"),
                            self.app.revision,
                            None,
                        );
                    }
                }
            }
            WindowEvent::RedrawRequested => self.paint(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Live Pi output lands here, never blocking the loop.
        self.drain_pi();
        // Resolve parked `wait` ops without blocking the event loop.
        if !self.pending_waits.is_empty() {
            let cur = self.app.revision;
            let now = Instant::now();
            let mut i = 0;
            while i < self.pending_waits.len() {
                let (done, expired) = {
                    let pw = &self.pending_waits[i];
                    (cur != pw.target_rev, now >= pw.deadline)
                };
                if done || expired {
                    let pw = self.pending_waits.remove(i);
                    let out = if done {
                        self.stamped(serde_json::json!({"waited": true}))
                    } else {
                        self.err(
                            "TIMEOUT",
                            format!("no change within deadline (still rev {cur})"),
                        )
                    };
                    let _ = pw.respond.send(out);
                } else {
                    i += 1;
                }
            }
        }
        if self.use_m3 && self.smoke_deadline.is_some() && !self.m3_smoke_done {
            self.m3_smoke_done = true;
            self.m3_smoke_script();
        }
        if self.use_console && self.smoke_deadline.is_some() && !self.console_smoke_done {
            self.console_smoke_done = true;
            self.console_smoke_script();
        }
        if self.use_editor && self.smoke_deadline.is_some() && !self.editor_smoke_done {
            self.editor_smoke_done = true;
            self.editor_smoke_script();
        }
        if let Some(deadline) = self.smoke_deadline {
            if Instant::now() >= deadline {
                // Fire once: with ControlFlow::Poll the loop can re-enter
                // about_to_wait before event_loop.exit() takes effect,
                // which used to double-dump inspection, snapshot, trace
                // and verdict lines.
                self.smoke_deadline = None;
                if self.use_pi && self.use_m3 {
                    self.smoke_exit_code = Some(self.m3_pi_assert());
                } else if self.use_pi && self.use_console {
                    self.smoke_exit_code = Some(self.console_pi_assert());
                } else if self.use_editor {
                    self.smoke_exit_code = Some(self.editor_smoke_assert());
                }
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
    // Vendored OFL profile faces load before first shape so every
    // run (and every machine) voices profiles identically.
    let loaded = hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
    eprintln!("profile fonts: {loaded} families from assets/fonts");
    let smoke_secs: Option<u64> =
        std::env::args().find_map(|a| a.strip_prefix("--smoke-test=").and_then(|v| v.parse().ok()));
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = HameshWindowApp::new();
    app.use_gpu = std::env::args().any(|a| a == "--gpu");
    if app.use_gpu {
        println!("GPU mode: wgpu + vello (falls back to CPU if init fails)");
    }
    app.use_m3 = std::env::args().any(|a| a == "--m3");
    if app.use_m3 {
        println!("M3 mode: feed + composer (Tab/typing/Enter/wheel)");
    }
    app.use_blocks = std::env::args().any(|a| a == "--blocks");
    if app.use_blocks {
        app.app = dashboard::build_dashboard();
        app.scene_rev += 1;
        println!("BLOCKS mode: Agent X dashboard from hamesh-blocks");
    }
    // Pi panel shares every path with the dashboard (refresh, Space,
    // Eye, Hand); only the composition differs.
    app.use_blocks_pi = std::env::args().any(|a| a == "--blocks-pi");
    app.use_gallery = std::env::args().any(|a| a == "--gallery");
    app.use_tasks = std::env::args().any(|a| a == "--tasks");
    app.use_sheet = std::env::args().any(|a| a == "--sheet");
    app.use_console = std::env::args().any(|a| a == "--console");
    if app.use_console {
        println!("CONSOLE mode: Pi session console from hamesh-console");
    }
    app.use_editor = std::env::args().any(|a| a == "--editor");
    if app.use_editor {
        println!("EDITOR mode: file editor from hamesh-editor");
    }
    if app.use_gallery {
        println!("GALLERY mode: ListView + Picker workspace from hamesh-gallery");
    }
    if app.use_blocks_pi {
        app.use_blocks = true;
        app.app = pi_panel::build_pi_panel();
        app.scene_rev += 1;
        println!("BLOCKS-PI mode: Pi worker panel from hamesh-blocks");
    }
    app.use_inspect = std::env::args().any(|a| a == "--inspect");
    if app.use_inspect {
        println!("INSPECT mode: local loopback inspector (dev only)");
    }
    app.use_pi = std::env::args().any(|a| a == "--pi");
    if app.use_pi {
        println!("PI mode: composer submits stream through a background pi worker (needs auth + network)");
    }
    app.proxy = Some(event_loop.create_proxy());
    if let Some(secs) = smoke_secs {
        app.smoke_deadline = Some(Instant::now() + std::time::Duration::from_secs(secs));
        println!("SMOKE mode: {secs}s");
    }
    event_loop.run_app(&mut app).expect("run app");
    // A machine-checked smoke must be able to fail the run: propagate
    // the M4 verdict (None = no verdict ran, exit clean).
    if let Some(code) = app.smoke_exit_code {
        std::process::exit(code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Headless M3 app with a Pi link whose worker side is just channels:
    /// no window, no model, no network. `live` simulates a run in flight.
    fn wired_app(live: bool) -> (HameshWindowApp, std::sync::mpsc::Receiver<PiJob>) {
        let mut app = HameshWindowApp::new();
        app.enter_m3();
        let (job_tx, job_rx) = std::sync::mpsc::channel();
        let (_evt_tx, evt_rx) = std::sync::mpsc::channel();
        app.pi = Some(PiLink {
            job_tx,
            evt_rx,
            live_index: if live { Some(0) } else { None },
            follow: false,
        });
        (app, job_rx)
    }

    #[test]
    fn escape_aborts_live_run() {
        let (mut app, job_rx) = wired_app(true);
        app.m3_keys_apply(vec![WKey::Escape]);
        match job_rx.try_recv() {
            Ok(PiJob::Abort) => {}
            other => panic!("expected Abort, got {other:?}"),
        }
    }

    #[test]
    fn escape_idle_sends_nothing() {
        let (mut app, job_rx) = wired_app(false);
        app.m3_keys_apply(vec![WKey::Escape]);
        assert!(
            job_rx.try_recv().is_err(),
            "idle Escape must not queue a job"
        );
    }

    #[test]
    fn pi_verdict_maps_to_exit_codes() {
        // A machine-checked smoke must be able to fail the run:
        // PASS 0, FAIL 1, still-streaming 2.
        assert_eq!(pi_verdict(true, true, false), ("PASS", 0));
        assert_eq!(pi_verdict(false, false, false), ("FAIL(no-submission)", 1));
        assert_eq!(
            pi_verdict(true, false, true),
            ("INCOMPLETE(still-streaming)", 2)
        );
        assert_eq!(
            pi_verdict(true, false, false),
            ("FAIL(no-token-in-reply)", 1)
        );
    }
}
