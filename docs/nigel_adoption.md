# NIGEL Adoption Readiness: Technical Handoff & Architecture Guide

**Target Repository**: `d:\Projects\NIGEL`  
**Framework**: Hamesh GUI (Milestones 042–044)  
**Status**: Ready for Adoption Planning (Documentation Only; Do Not Migrate Yet)

---

## 1. Executive Summary

NIGEL (Autonomous Agent Orchestration and Monitoring Environment) requires a dependable, low-latency, attractive native desktop interface for real-time observability, agent control, command palette dispatch, and log inspection.

Milestones 042–044 have established Hamesh as an ideal native foundation for NIGEL:
1. **Low Footprint**: Sub-millisecond in-memory frame latency leaves CPU and GPU resources fully available for NIGEL's agent processes and local model inferences.
2. **First-Class Inspection**: Hamesh's retained component tree and revision tracking allow automated testing and external agent inspection via MCP or CLI.
3. **Professional Aesthetics**: NIGEL can utilize the **Technical Workstation** visual world (`technical`) for dense real-time telemetry or **Modern Studio** (`modern`) for operator ergonomics.
4. **Direct Native Rust Integration**: NIGEL is written in Rust. It should consume Hamesh as a direct Cargo dependency (`hamesh-shell`, `hamesh-widgets`, `hamesh-theme`), avoiding foreign C ABI or subprocess overhead entirely.

---

## 2. Cargo Dependency Configuration

In `d:\Projects\NIGEL\Cargo.toml`:

```toml
[dependencies]
hamesh-core = { path = "../hameshgui/crates/hamesh-core" }
hamesh-shell = { path = "../hameshgui/crates/hamesh-shell" }
hamesh-widgets = { path = "../hameshgui/crates/hamesh-widgets" }
hamesh-theme = { path = "../hameshgui/crates/hamesh-theme" }
```

---

## 3. Architecture & Component Mapping

NIGEL's desktop UI maps cleanly to Hamesh's reusable desktop controls:

| NIGEL Capability | Hamesh Component / Widget | Behavior & Styling |
|---|---|---|
| **Global Command Palette** | `CommandRegistry` + `MenuBar` | Shortcut dispatch (`Ctrl+P`, `Ctrl+Shift+P`), action routing |
| **Agent Roster / Process Feed** | `SelectableListControl` | High-density process list, status tags, up/down arrow navigation |
| **Real-time Telemetry Panel** | Split layout with `SplitterControl` | Resizable split between agent tree and live stdout/stderr streams |
| **Status Bar & Alert Strip** | `StatusBarControl` | Profile tag (`Technical Workstation [2026]`), Vitals (CPU/RAM/Heartbeat) |
| **Log Stream View** | Scrollable `Component` (`Role::Panel`) | Monospace virtualized log feed with ANSI color token support |

---

## 4. Concrete Code Skeleton for NIGEL

Here is the proposed architectural skeleton implementing `HameshApp`:

```rust
use hamesh_core::{App, Component, Role};
use hamesh_shell::{HameshApp, ShellContext, ShellConfig};
use hamesh_theme::VisualWorld;
use hamesh_widgets::desktop::{
    CommandRegistry, FocusRing, MenuBar, MenuCategory, MenuItem,
    SelectableListControl, SplitterControl, StatusBarControl, TabControl,
};
use winit::event::{ElementState, MouseButton, VirtualKeyCode};

pub struct NigelDesktopApp {
    world: VisualWorld,
    commands: CommandRegistry,
    menu_bar: MenuBar,
    status_bar: StatusBarControl,
    tab_control: TabControl,
    agent_roster: SelectableListControl,
    splitter: SplitterControl,
    focus_ring: FocusRing,
    active_agent_log: String,
}

impl NigelDesktopApp {
    pub fn new() -> Self {
        let mut commands = CommandRegistry::new();
        commands.register("agent_spawn", "Spawn Agent", Some("Ctrl+N"), "Agents");
        commands.register("agent_kill", "Terminate Agent", Some("Ctrl+K"), "Agents");
        commands.register("view_telemetry", "Show Telemetry", Some("Ctrl+1"), "View");

        let mut menu_bar = MenuBar::new("nigel_menu");
        let mut agents_menu = MenuCategory::new("Agents", 'A');
        agents_menu.add_item(MenuItem::new("item_spawn", "Spawn Agent").with_command("agent_spawn"));
        agents_menu.add_item(MenuItem::new("item_kill", "Terminate Agent").with_command("agent_kill"));
        menu_bar.add_category(agents_menu);

        let agent_roster = SelectableListControl::new(
            "agent_list",
            vec![
                "Agent-01 (Coordinator) [IDLE]".to_string(),
                "Agent-02 (Compiler) [RUNNING]".to_string(),
                "Agent-03 (Inspector) [VERIFYING]".to_string(),
            ],
        );

        let splitter = SplitterControl::new("telemetry_splitter", 300.0, 200.0, 600.0);
        let mut status_bar = StatusBarControl::new(
            "nigel_status",
            "NIGEL Orchestrator Online. All agents healthy.",
        );
        status_bar.set_profile_tag("Technical Workstation [NIGEL]");
        status_bar.set_telemetry("CPU: 1.2% | Memory: 42MB | Agents: 3");

        Self {
            world: VisualWorld::TechnicalWorkstation,
            commands,
            menu_bar,
            status_bar,
            tab_control: TabControl::new("tabs", vec![
                ("telemetry", "Telemetry & Processes"),
                ("logs", "Live Log Stream"),
            ]),
            agent_roster,
            splitter,
            focus_ring: FocusRing::new(vec!["agent_list_row_0".into(), "agent_list_row_1".into()]),
            active_agent_log: "Initial boot sequence complete.\nListening on RPC channel.\n".into(),
        }
    }

    pub fn rebuild_ui(&mut self, app: &mut App) {
        let root_id = app.root.0.clone();
        app.components.clear();
        app.commands.clear();

        let mut root = Component::new(&root_id, Role::Window, "root");
        root.props.insert("theme".into(), self.world.name().into());
        app.components.insert(root_id.clone(), root);
        app.revision += 1;

        // 1. Root Container
        app.add(&root_id, Component::new("root_panel", Role::Panel, "NIGEL Root")
            .with_prop("layout", "column")
            .with_prop("w", "100%")
            .with_prop("h", "100%")
            .with_prop("surface", "window")
            .with_prop("font", self.world.font_family()));

        // 2. Menu Bar
        self.menu_bar.build_components(app, "root_panel", self.world);

        // 3. Main Stage (Splitter: Agent List | Live Telemetry)
        app.add("root_panel", Component::new("main_stage", Role::Panel, "Stage")
            .with_prop("layout", "row")
            .with_prop("flex_grow", "1")
            .with_prop("pad", "4")
            .with_prop("gap", "4"));

        // Left Panel: Agent Roster
        let left_w = format!("{:.0}", self.splitter.position);
        app.add("main_stage", Component::new("roster_panel", Role::Panel, "Roster")
            .with_prop("layout", "column")
            .with_prop("w", &left_w)
            .with_prop("surface", "raised")
            .with_prop("border", "1"));

        self.agent_roster.build_components(app, "roster_panel", self.focus_ring.current(), self.world);

        // Splitter Handle
        self.splitter.build_component(app, "main_stage", self.world);

        // Right Panel: Live Telemetry / Logs
        app.add("main_stage", Component::new("logs_panel", Role::Panel, "Logs")
            .with_prop("layout", "column")
            .with_prop("flex_grow", "1")
            .with_prop("surface", "window")
            .with_prop("pad", "6"));

        app.add("logs_panel", Component::new("logs_text", Role::Text, "Log Stream")
            .with_prop("flex_grow", "1")
            .with_prop("color", "text")
            .with_prop("text", &self.active_agent_log));

        // 4. Status Bar
        self.status_bar.build_components(app, "root_panel", self.world);
    }
}

impl HameshApp for NigelDesktopApp {
    fn init(&mut self, ctx: &mut ShellContext) {
        self.rebuild_ui(ctx.app);
        ctx.request_redraw();
    }

    fn on_mouse_click(&mut self, ctx: &mut ShellContext, button: MouseButton, state: ElementState) {
        // Standard interactive routing via Hamesh components
        self.rebuild_ui(ctx.app);
        ctx.request_redraw();
    }
}

// In NIGEL's main entry point:
pub fn launch_desktop_gui() {
    let app = NigelDesktopApp::new();
    let config = ShellConfig {
        title: "NIGEL Orchestrator".into(),
        width: 1200,
        height: 800,
        ..Default::default()
    };
    hamesh_shell::run_app(app, config);
}
```

---

## 5. Adoption Checklist for Future Milestone

When NIGEL begins its GUI integration:
- [ ] Add `hamesh-*` paths or crate dependencies to NIGEL's workspace.
- [ ] Implement an asynchronous crossbeam/tokio channel bridging NIGEL's agent event loop with the GUI thread.
- [ ] Hook `StatusBarControl` telemetry to NIGEL's system metric collectors.
- [ ] Connect `SelectableListControl` to live process lifecycle events (spawn/exit).
- [ ] Implement command execution via `CommandRegistry` for agent management.
