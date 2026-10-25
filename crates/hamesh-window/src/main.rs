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
    shift_held: bool,
    m3_smoke_done: bool,
    /// M4 live loop: background Pi worker link. `None` unless `--pi`.
    /// Never blocks the UI thread; drains between frames.
    pi: Option<PiLink>,
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
            shift_held: false,
            m3_smoke_done: false,
            pi: None,
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

    /// Gallery mouse: the shared press path, then trace.
    fn gallery_click(&mut self) {
        let (_, scene) = self.layout_and_scene();
        let note = match self.gallery.as_mut() {
            Some(g) => hamesh_gallery::press(g, &self.app, &scene, self.cursor.0, self.cursor.1),
            None => "gallery not entered".to_string(),
        };
        self.refresh_gallery();
        self.trace
            .record("gallery.click", &note, self.app.revision, None);
        println!("GALLERY click: {note}");
    }

    /// Gallery keyboard: map the winit event onto the shared intent core.
    fn gallery_key(&mut self, event: &winit::event::KeyEvent, event_loop: &ActiveEventLoop) {
        use hamesh_gallery::GKey;
        let Some(g) = self.gallery.as_mut() else {
            return;
        };
        if event.physical_key == PhysicalKey::Code(KeyCode::F12) {
            self.dump_inspection();
            return;
        }
        let mut keys: Vec<GKey> = Vec::new();
        if let Some(text) = event.text.as_ref() {
            for c in text.chars() {
                // Tab arrives as text too; the physical key wins below.
                if c != '\r' && c != '\n' && c != '\t' {
                    keys.push(GKey::Char(c));
                }
            }
        }
        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => keys.push(GKey::Up),
            PhysicalKey::Code(KeyCode::ArrowDown) => keys.push(GKey::Down),
            PhysicalKey::Code(KeyCode::PageUp) => keys.push(GKey::PageUp),
            PhysicalKey::Code(KeyCode::PageDown) => keys.push(GKey::PageDown),
            PhysicalKey::Code(KeyCode::Home) => keys.push(GKey::Home),
            PhysicalKey::Code(KeyCode::End) => keys.push(GKey::End),
            PhysicalKey::Code(KeyCode::Enter) | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                keys.push(GKey::Enter)
            }
            PhysicalKey::Code(KeyCode::Backspace) => keys.push(GKey::Backspace),
            PhysicalKey::Code(KeyCode::Tab) => keys.push(GKey::Tab),
            PhysicalKey::Code(KeyCode::Escape) => keys.push(GKey::Escape),
            _ => {}
        }
        if keys.is_empty() {
            return;
        }
        // Escape with a closed popup quits; otherwise the popup owns it.
        let escape_quits = keys.len() == 1 && matches!(keys[0], GKey::Escape) && !g.fonts.is_open();
        let names: Vec<String> = g.layer_rows.iter().map(|r| r.label.clone()).collect();
        let mut notes = Vec::new();
        for k in keys {
            notes.push(hamesh_gallery::key(g, k, &names));
        }
        let note = notes.join(" | ");
        self.refresh_gallery();
        self.trace
            .record("gallery.key", &note, self.app.revision, None);
        println!("GALLERY key: {note}");
        if escape_quits {
            event_loop.exit();
        }
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
/// Never touches UI state; everything crosses on channels.
fn pi_worker(jobs: std::sync::mpsc::Receiver<PiJob>, events: std::sync::mpsc::Sender<PiEvt>) {
    let mut client = match hamesh_pi::PiClient::spawn_ephemeral("pi", None) {
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
    while let Ok(job) = jobs.recv() {
        match job {
            PiJob::Abort => {
                let _ = client.abort(std::time::Duration::from_secs(10));
            }
            PiJob::Prompt(text) => {
                let _ = events.send(PiEvt::Started);
                let r = client.prompt_and_wait(
                    &text,
                    &mut |d: &str| {
                        let _ = events.send(PiEvt::Delta(d.to_string()));
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
        }
        let mut acts = Vec::new();
        if let Some(link) = self.pi.as_mut() {
            while let Ok(ev) = link.evt_rx.try_recv() {
                match ev {
                    PiEvt::Started => acts.push(Act::Started),
                    PiEvt::Delta(d) => acts.push(Act::Delta(d)),
                    PiEvt::Done { text, aborted } => acts.push(Act::Done { text, aborted }),
                    PiEvt::Failed(e) => acts.push(Act::Failed(e)),
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
                    }
                }
                Act::Delta(d) => {
                    let idx = self.pi.as_ref().and_then(|l| l.live_index);
                    if let (Some(i), Some(w)) = (idx, self.widgets.as_mut()) {
                        if let Some(m) = w.feed.messages.get_mut(i) {
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
                    }
                }
            }
        }
        if changed {
            self.refresh_feed();
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
                        None => Some(self.err("NOT_FOUND", format!("no layout rect for {id}"))),
                    }
                }
            }
            Op::ClickAt { x, y, expect_rev } => {
                if let Some(e) = self.check_stale(&expect_rev) {
                    Some(e)
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
                } else if self.widgets.is_none() {
                    Some(self.err("NO_M3", "keyboard needs --m3 mode".to_string()))
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
                if self.widgets.is_none() {
                    Some(self.err("NO_M3", "typing needs --m3 mode".to_string()))
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
        if self.use_pi && self.pi.is_none() {
            let (job_tx, job_rx) = std::sync::mpsc::channel::<PiJob>();
            let (evt_tx, evt_rx) = std::sync::mpsc::channel::<PiEvt>();
            std::thread::spawn(move || pi_worker(job_rx, evt_tx));
            self.pi = Some(PiLink {
                job_tx,
                evt_rx,
                live_index: None,
                follow: false,
            });
            self.trace.record(
                "pi.worker",
                "background pi worker spawned (--pi)",
                self.app.revision,
                None,
            );
            println!("PI live mode: composer submits to a background pi worker");
        }
        if self.use_pi && self.pi.is_none() {
            let (job_tx, job_rx) = std::sync::mpsc::channel::<PiJob>();
            let (evt_tx, evt_rx) = std::sync::mpsc::channel::<PiEvt>();
            std::thread::spawn(move || pi_worker(job_rx, evt_tx));
            self.pi = Some(PiLink {
                job_tx,
                evt_rx,
                live_index: None,
                follow: false,
            });
            self.trace.record(
                "pi.worker",
                "background pi worker spawned (--pi)",
                self.app.revision,
                None,
            );
            println!("PI live mode: composer submits to a background pi worker");
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
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    if self.use_gallery {
                        self.gallery_click();
                    } else if self.use_m3 {
                        self.m3_click();
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
                if self.use_m3 && event.physical_key != PhysicalKey::Code(KeyCode::Escape) {
                    // M3 path consumes everything except Escape (quit).
                    self.m3_key(&event);
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        // Live run: Escape aborts it and stays open (one
                        // rule via m3_keys_apply). Idle: Escape quits.
                        if self.use_m3 && self.pi.as_ref().is_some_and(|l| l.live_index.is_some()) {
                            self.m3_keys_apply(vec![WKey::Escape]);
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
        if let Some(deadline) = self.smoke_deadline {
            if Instant::now() >= deadline {
                // Fire once: with ControlFlow::Poll the loop can re-enter
                // about_to_wait before event_loop.exit() takes effect,
                // which used to double-dump inspection, snapshot, trace
                // and verdict lines.
                self.smoke_deadline = None;
                if self.use_pi && self.use_m3 {
                    self.smoke_exit_code = Some(self.m3_pi_assert());
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
