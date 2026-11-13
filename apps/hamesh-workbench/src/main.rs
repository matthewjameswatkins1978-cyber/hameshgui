//! Hamesh Workbench: Native demonstration of reusable desktop menus,
//! semantic command routing, keyboard focus traversal, split panels, frame diagnostics,
//! and five coherent visual worlds.

use hamesh_core::{App, Component, Role};
use hamesh_shell::{HameshApp, Shell, ShellConfig, ShellContext};
use hamesh_theme::VisualWorld;
use hamesh_widgets::{
    CheckboxControl, CommandRegistry, FocusRing, MenuBar, MenuCategory, MenuItem,
    SelectableListControl, SplitterControl, StatusBarControl, TabControl,
};
use winit::event::{ElementState, MouseButton};
use winit::keyboard::{Key, NamedKey};

pub struct WorkbenchApp {
    pub world: VisualWorld,
    pub menu_bar: MenuBar,
    pub commands: CommandRegistry,
    pub focus_ring: FocusRing,
    pub tab_control: TabControl,
    pub list_control: SelectableListControl,
    pub splitter: SplitterControl,
    pub checkbox: CheckboxControl,
    pub status_bar: StatusBarControl,
    pub text_input_value: String,
    pub editor_content: String,
    pub progress_value: f32,
    pub frame_count: u64,
    pub show_status_bar: bool,
}

impl Default for WorkbenchApp {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkbenchApp {
    pub fn new() -> Self {
        let mut commands = CommandRegistry::new();
        commands.register("file_new", "New Document", Some("Ctrl+N"), "File");
        commands.register("file_open", "Open File...", Some("Ctrl+O"), "File");
        commands.register("file_save", "Save Changes", Some("Ctrl+S"), "File");
        commands.register("file_exit", "Exit Application", Some("Alt+F4"), "File");

        commands.register("edit_undo", "Undo", Some("Ctrl+Z"), "Edit");
        commands.register("edit_redo", "Redo", Some("Ctrl+Y"), "Edit");
        commands.register("edit_cut", "Cut", Some("Ctrl+X"), "Edit");
        commands.register("edit_copy", "Copy", Some("Ctrl+C"), "Edit");
        commands.register("edit_paste", "Paste", Some("Ctrl+V"), "Edit");

        commands.register(
            "view_classic",
            "Classic Desktop Profile",
            Some("Ctrl+1"),
            "View",
        );
        commands.register(
            "view_modern",
            "Modern Studio Profile",
            Some("Ctrl+2"),
            "View",
        );
        commands.register(
            "view_retro",
            "Retro Computing Profile",
            Some("Ctrl+3"),
            "View",
        );
        commands.register(
            "view_technical",
            "Technical Workstation Profile",
            Some("Ctrl+4"),
            "View",
        );
        commands.register(
            "view_editorial",
            "Editorial / Paper Profile",
            Some("Ctrl+5"),
            "View",
        );
        commands.register("view_toggle_status", "Toggle Status Bar", None, "View");

        commands.register("tools_diag", "Run Diagnostics Battery", Some("F5"), "Tools");
        commands.register("tools_reset", "Reset Form Fields", None, "Tools");
        commands.register(
            "tools_capture",
            "Capture All 5 Visual Receipts",
            None,
            "Tools",
        );

        commands.register("help_about", "About Hamesh Workbench", Some("F1"), "Help");

        let mut menu_bar = MenuBar::new();
        menu_bar.add_menu(
            MenuCategory::new("File")
                .with_mnemonic('F')
                .with_item(
                    MenuItem::new("menu_file_new", "New Document")
                        .with_shortcut("Ctrl+N")
                        .with_command("file_new"),
                )
                .with_item(
                    MenuItem::new("menu_file_open", "Open File...")
                        .with_shortcut("Ctrl+O")
                        .with_command("file_open"),
                )
                .with_item(
                    MenuItem::new("menu_file_save", "Save Changes")
                        .with_shortcut("Ctrl+S")
                        .with_command("file_save"),
                )
                .with_item(
                    MenuItem::new("menu_file_exit", "Exit")
                        .with_shortcut("Alt+F4")
                        .with_command("file_exit"),
                ),
        );

        menu_bar.add_menu(
            MenuCategory::new("Edit")
                .with_mnemonic('E')
                .with_item(
                    MenuItem::new("menu_edit_undo", "Undo")
                        .with_shortcut("Ctrl+Z")
                        .with_command("edit_undo"),
                )
                .with_item(
                    MenuItem::new("menu_edit_redo", "Redo")
                        .with_shortcut("Ctrl+Y")
                        .with_command("edit_redo"),
                )
                .with_item(
                    MenuItem::new("menu_edit_cut", "Cut")
                        .with_shortcut("Ctrl+X")
                        .with_command("edit_cut"),
                )
                .with_item(
                    MenuItem::new("menu_edit_copy", "Copy")
                        .with_shortcut("Ctrl+C")
                        .with_command("edit_copy"),
                )
                .with_item(
                    MenuItem::new("menu_edit_paste", "Paste")
                        .with_shortcut("Ctrl+V")
                        .with_command("edit_paste"),
                ),
        );

        menu_bar.add_menu(
            MenuCategory::new("View")
                .with_mnemonic('V')
                .with_item(
                    MenuItem::new("menu_view_classic", "Classic Desktop")
                        .with_shortcut("Ctrl+1")
                        .with_command("view_classic"),
                )
                .with_item(
                    MenuItem::new("menu_view_modern", "Modern Studio")
                        .with_shortcut("Ctrl+2")
                        .with_command("view_modern"),
                )
                .with_item(
                    MenuItem::new("menu_view_retro", "Retro Computing")
                        .with_shortcut("Ctrl+3")
                        .with_command("view_retro"),
                )
                .with_item(
                    MenuItem::new("menu_view_technical", "Technical Workstation")
                        .with_shortcut("Ctrl+4")
                        .with_command("view_technical"),
                )
                .with_item(
                    MenuItem::new("menu_view_editorial", "Editorial / Paper")
                        .with_shortcut("Ctrl+5")
                        .with_command("view_editorial"),
                )
                .with_item(
                    MenuItem::new("menu_view_status", "Toggle Status Bar")
                        .with_command("view_toggle_status"),
                ),
        );

        menu_bar.add_menu(
            MenuCategory::new("Tools")
                .with_mnemonic('T')
                .with_item(
                    MenuItem::new("menu_tools_diag", "Run Diagnostics Battery")
                        .with_shortcut("F5")
                        .with_command("tools_diag"),
                )
                .with_item(
                    MenuItem::new("menu_tools_reset", "Reset Form Fields")
                        .with_command("tools_reset"),
                )
                .with_item(
                    MenuItem::new("menu_tools_capture", "Capture Visual Receipts (5 Worlds)")
                        .with_command("tools_capture"),
                ),
        );

        menu_bar.add_menu(
            MenuCategory::new("Help").with_mnemonic('H').with_item(
                MenuItem::new("menu_help_about", "About Hamesh Workbench")
                    .with_shortcut("F1")
                    .with_command("help_about"),
            ),
        );

        let tab_control = TabControl::new(
            "workbench_tabs",
            vec![
                ("controls", "Controls Gallery"),
                ("editor", "Tool Editor"),
                ("telemetry", "Split Layout & Telemetry"),
            ],
        );

        let list_control = SelectableListControl::new(
            "workbench_records",
            vec![
                "001 - Hamesh Shell Lifecycle (Verified)".to_string(),
                "002 - Cross-Language C ABI Binding (Ready)".to_string(),
                "003 - Reusable Desktop Controls (Extracted)".to_string(),
                "004 - 5 Visual Worlds Architecture (Active)".to_string(),
                "005 - Incremental Layout & Fast Rendering (040)".to_string(),
            ],
        );

        let focus_ring = FocusRing::new(vec![
            "sample_text_input".to_string(),
            "sample_checkbox".to_string(),
            "btn_world_classic".to_string(),
            "btn_world_modern".to_string(),
            "btn_world_retro".to_string(),
            "btn_world_technical".to_string(),
            "btn_world_editorial".to_string(),
            "workbench_records_row_0".to_string(),
            "workbench_records_row_1".to_string(),
            "workbench_records_row_2".to_string(),
            "workbench_records_row_3".to_string(),
            "workbench_records_row_4".to_string(),
        ]);

        let splitter = SplitterControl::new("sidebar_splitter", 280.0, 180.0, 500.0);
        let checkbox = CheckboxControl::new("sample_checkbox", "Enable Telemetry Streaming", true);
        let mut status_bar = StatusBarControl::new(
            "workbench_status_bar",
            "Ready. Use Alt+F/E/V/T/H, Tab for focus, or Ctrl+1..5 to switch worlds.",
        );
        status_bar.set_profile_tag("Modern Studio [2026]");
        status_bar.set_telemetry("Frames: 0 | CPU: Clean");

        Self {
            world: VisualWorld::ModernStudio,
            menu_bar,
            commands,
            focus_ring,
            tab_control,
            list_control,
            splitter,
            checkbox,
            status_bar,
            text_input_value: "Hamesh Workbench v0.2.0".to_string(),
            editor_content: "Welcome to Hamesh Desktop Tool Editor.\nType here to test multi-line text and keyboard input across profiles.\nAll state is preserved when switching between all 5 visual worlds!".to_string(),
            progress_value: 65.0,
            frame_count: 0,
            show_status_bar: true,
        }
    }

    pub fn set_visual_world(&mut self, world: VisualWorld) {
        self.world = world;
        let tag = format!("{} [{}]", world.label(), world.name());
        self.status_bar.set_profile_tag(tag);
        self.status_bar.set_message(format!(
            "Switched to visual world: {} — {}",
            world.label(),
            world.description()
        ));
    }

    pub fn execute_command(&mut self, cmd: &str, ctx: &mut ShellContext) {
        match cmd {
            "file_new" => {
                self.text_input_value = "Untitled Document".to_string();
                self.editor_content.clear();
                self.status_bar
                    .set_message("Executed: File -> New Document");
            }
            "file_open" => {
                if let Ok(content) = std::fs::read_to_string("workbench_demo.txt") {
                    self.editor_content = content;
                    self.status_bar
                        .set_message("File -> Open: Read text from workbench_demo.txt");
                } else {
                    self.editor_content =
                        "Template: Hamesh Universal Desktop Foundation".to_string();
                    self.status_bar
                        .set_message("Demo: Loaded default template into tool editor.");
                }
            }
            "file_save" => match std::fs::write("workbench_demo.txt", &self.editor_content) {
                Ok(_) => {
                    self.status_bar.set_message(format!(
                        "File -> Save: Written {} bytes to workbench_demo.txt",
                        self.editor_content.len()
                    ));
                }
                Err(e) => {
                    self.status_bar
                        .set_message(format!("File -> Save failed: {}", e));
                }
            },
            "file_exit" => {
                ctx.exit();
                return;
            }
            "edit_undo" => {
                self.status_bar.set_message("Executed: Edit -> Undo");
            }
            "edit_redo" => {
                self.status_bar.set_message("Executed: Edit -> Redo");
            }
            "edit_cut" => {
                self.status_bar.set_message("Executed: Edit -> Cut");
            }
            "edit_copy" => {
                self.status_bar.set_message("Executed: Edit -> Copy");
            }
            "edit_paste" => {
                self.status_bar.set_message("Executed: Edit -> Paste");
            }
            "view_classic" => {
                self.set_visual_world(VisualWorld::ClassicDesktop);
            }
            "view_modern" => {
                self.set_visual_world(VisualWorld::ModernStudio);
            }
            "view_retro" => {
                self.set_visual_world(VisualWorld::RetroComputing);
            }
            "view_technical" => {
                self.set_visual_world(VisualWorld::TechnicalWorkstation);
            }
            "view_editorial" => {
                self.set_visual_world(VisualWorld::EditorialPaper);
            }
            "view_toggle_status" => {
                self.show_status_bar = !self.show_status_bar;
                self.status_bar
                    .set_message(format!("Status bar visible: {}", self.show_status_bar));
            }
            "tools_diag" => {
                self.tab_control.select(2); // Telemetry tab
                self.status_bar
                    .set_message("Executed: Tools -> Run Diagnostics Battery");
            }
            "tools_reset" => {
                self.text_input_value.clear();
                self.checkbox.checked = false;
                self.list_control.selected_idx = 0;
                self.progress_value = 0.0;
                self.status_bar
                    .set_message("Executed: Tools -> Reset Form Fields");
            }
            "tools_capture" => {
                self.capture_visual_receipts(ctx.app);
                self.status_bar.set_message(
                    "Tools -> Capture: Rendered visual receipts for all 5 worlds to target/visual_receipts/.",
                );
            }
            "help_about" => {
                self.status_bar.set_message(
                    "Hamesh Workbench v0.2.0 — Universal Native GUI Architecture for Rust, Python & Pepin.",
                );
            }
            other => {
                self.status_bar
                    .set_message(format!("Unknown command: {}", other));
            }
        }
        self.menu_bar.close();
        self.rebuild_ui(ctx.app);
        ctx.request_redraw();
    }

    /// Renders frame captures for each of the five visual worlds to disk.
    pub fn capture_visual_receipts(&mut self, app: &mut App) {
        let _ = std::fs::create_dir_all("target/visual_receipts");
        let orig_world = self.world;

        for world in VisualWorld::ALL {
            self.set_visual_world(world);
            self.rebuild_ui(app);
            let layout = hamesh_layout::compute(app, 1024.0, 768.0);
            let scene = hamesh_scene::build(app, &layout, 1);
            let frame = hamesh_render::render(&scene);
            let path = format!("target/visual_receipts/world_{}.png", world.name());
            let _ = frame.pixmap.save_png(&path);
        }

        self.set_visual_world(orig_world);
        self.rebuild_ui(app);
    }

    pub fn rebuild_ui(&mut self, app: &mut App) {
        let root_id = app.root.0.clone();
        app.components.clear();
        app.commands.clear();

        let mut root_comp = Component::new(&root_id, Role::Window, "root");
        root_comp
            .props
            .insert("theme".to_string(), self.world.name().to_string());
        app.components.insert(root_id.clone(), root_comp);
        app.revision += 1;

        let world = self.world;
        let font_family = world.font_family();
        let border_size = format!("{:.0}", world.border_width());
        let radius = format!("{:.0}", world.corner_radius_sm());
        let card_radius = format!("{:.0}", world.corner_radius_md());
        let pad_standard = format!("{:.0}", world.padding());
        let gap_standard = format!("{:.0}", world.gap());

        // 1. Root Window container
        app.add(
            &root_id,
            Component::new("workbench_root", Role::Panel, "Workbench Root")
                .with_prop("layout", "column")
                .with_prop("w", "100%")
                .with_prop("h", "100%")
                .with_prop("surface", "window")
                .with_prop("font", font_family),
        );

        // 2. Menu Bar with Dropdown Overlays (Reusable Component)
        self.menu_bar.build_components(app, "workbench_root", world);

        // 3. Tab Bar (Reusable Component)
        self.tab_control
            .build_components(app, "workbench_root", world);

        // 4. Main Stage Area
        app.add(
            "workbench_root",
            Component::new("main_stage", Role::Panel, "Main Stage")
                .with_prop("layout", "row")
                .with_prop("flex_grow", "1")
                .with_prop("pad", &pad_standard)
                .with_prop("gap", &gap_standard)
                .with_prop("surface", "window"),
        );

        match self.tab_control.active_idx {
            0 => {
                // TAB 1: Controls Gallery
                let left_w = format!("{:.0}", self.splitter.position);

                // Left Panel: Interactive controls
                app.add(
                    "main_stage",
                    Component::new("controls_left", Role::Panel, "Controls Left")
                        .with_prop("layout", "column")
                        .with_prop("w", &left_w)
                        .with_prop("pad", &pad_standard)
                        .with_prop("gap", &gap_standard)
                        .with_prop("surface", "raised")
                        .with_prop("border", &border_size)
                        .with_prop("border_token", "border")
                        .with_prop("radius", &card_radius),
                );

                app.add(
                    "controls_left",
                    Component::new("worlds_heading", Role::Text, "Visual Worlds")
                        .with_prop("h", "20")
                        .with_prop("font_vars", "wght 700")
                        .with_prop("text", "Visual Worlds (Ctrl+1..5):"),
                );

                // Five World Switcher Buttons
                for w in VisualWorld::ALL {
                    let btn_id = format!("btn_world_{}", w.name());
                    let is_active = self.world == w;
                    let is_focused = self.focus_ring.current() == Some(&btn_id);
                    let surface = if is_active { "accent" } else { "surface" };
                    let color = if is_active { "bg" } else { "text" };

                    app.add(
                        "controls_left",
                        Component::new(&btn_id, Role::Button, w.label())
                            .with_prop("h", &format!("{:.0}", world.control_height()))
                            .with_prop("pad", "6")
                            .with_prop("surface", surface)
                            .with_prop("color", color)
                            .with_prop("border", if is_focused { "2" } else { "0" })
                            .with_prop("border_token", "focus")
                            .with_prop("radius", &radius)
                            .with_prop("text", w.label())
                            .with_prop("action", &format!("view_{}", w.name())),
                    );
                }

                // Checkbox Control
                let is_chk_focused = self.focus_ring.current() == Some("sample_checkbox");
                self.checkbox
                    .build_component(app, "controls_left", is_chk_focused, world);

                // Text Input
                app.add(
                    "controls_left",
                    Component::new("text_field_label", Role::Text, "Input Label")
                        .with_prop("h", "20")
                        .with_prop("text", "Text Field (Tab to focus):"),
                );

                let is_input_focused = self.focus_ring.current() == Some("sample_text_input");
                app.add(
                    "controls_left",
                    Component::new("sample_text_input", Role::TextInput, "Sample Input")
                        .with_prop("h", &format!("{:.0}", world.control_height()))
                        .with_prop("pad", "6")
                        .with_prop("surface", "window")
                        .with_prop("border", if is_input_focused { "2" } else { "1" })
                        .with_prop(
                            "border_token",
                            if is_input_focused { "focus" } else { "border" },
                        )
                        .with_prop("radius", &radius)
                        .with_prop("text", &self.text_input_value)
                        .with_prop("caret_x", "0")
                        .with_prop("focused", if is_input_focused { "true" } else { "false" }),
                );

                // Progress Bar
                app.add(
                    "controls_left",
                    Component::new("progress_label", Role::Text, "Progress Label")
                        .with_prop("h", "16")
                        .with_prop("text", "Progress Metric:"),
                );

                app.add(
                    "controls_left",
                    Component::new("sample_progress", Role::Panel, "Progress Bar")
                        .with_prop("block", "Progress")
                        .with_prop("h", "14")
                        .with_prop("surface", "track")
                        .with_prop("fill", "accent")
                        .with_prop("border", "1")
                        .with_prop("border_token", "border")
                        .with_prop("radius", &radius)
                        .with_prop("value", &format!("{:.0}", self.progress_value))
                        .with_prop("min", "0")
                        .with_prop("max", "100"),
                );

                // Splitter Handle
                self.splitter.build_component(app, "main_stage", world);

                // Right Panel: Selectable List
                app.add(
                    "main_stage",
                    Component::new("controls_right", Role::Panel, "Controls Right")
                        .with_prop("layout", "column")
                        .with_prop("flex_grow", "1")
                        .with_prop("pad", &pad_standard)
                        .with_prop("gap", &gap_standard)
                        .with_prop("surface", "raised")
                        .with_prop("border", &border_size)
                        .with_prop("border_token", "border")
                        .with_prop("radius", &card_radius),
                );

                app.add(
                    "controls_right",
                    Component::new("list_heading", Role::Text, "Selectable Records")
                        .with_prop("h", "20")
                        .with_prop("font_vars", "wght 700")
                        .with_prop(
                            "text",
                            "Selectable Desktop Records (Click or Up/Down arrows):",
                        ),
                );

                self.list_control.build_components(
                    app,
                    "controls_right",
                    self.focus_ring.current(),
                    world,
                );
            }
            1 => {
                // TAB 2: Practical Tool Editor
                app.add(
                    "main_stage",
                    Component::new("editor_panel", Role::Panel, "Tool Editor Panel")
                        .with_prop("layout", "column")
                        .with_prop("flex_grow", "1")
                        .with_prop("pad", &pad_standard)
                        .with_prop("gap", &gap_standard)
                        .with_prop("surface", "raised")
                        .with_prop("border", &border_size)
                        .with_prop("border_token", "border")
                        .with_prop("radius", &card_radius),
                );

                app.add(
                    "editor_panel",
                    Component::new("editor_title", Role::Text, "Document Title")
                        .with_prop("h", "24")
                        .with_prop("font_vars", "wght 700")
                        .with_prop(
                            "text",
                            "Universal Tool Editor (Supports typing, state preservation across worlds):",
                        ),
                );

                let is_editor_focused = self.focus_ring.current() == Some("editor_text_area");
                app.add(
                    "editor_panel",
                    Component::new("editor_text_area", Role::TextInput, "Editor Text Area")
                        .with_prop("flex_grow", "1")
                        .with_prop("pad", "12")
                        .with_prop("surface", "window")
                        .with_prop("border", if is_editor_focused { "2" } else { "1" })
                        .with_prop(
                            "border_token",
                            if is_editor_focused { "focus" } else { "border" },
                        )
                        .with_prop("radius", &radius)
                        .with_prop("text", &self.editor_content)
                        .with_prop("caret_x", "0")
                        .with_prop("focused", if is_editor_focused { "true" } else { "false" }),
                );
            }
            _ => {
                // TAB 3: Split Layout & Telemetry
                let left_w = format!("{:.0}", self.splitter.position);
                app.add(
                    "main_stage",
                    Component::new("telemetry_left", Role::Panel, "Left Telemetry")
                        .with_prop("layout", "column")
                        .with_prop("w", &left_w)
                        .with_prop("pad", &pad_standard)
                        .with_prop("gap", &gap_standard)
                        .with_prop("surface", "raised")
                        .with_prop("border", &border_size)
                        .with_prop("border_token", "border")
                        .with_prop("radius", &card_radius),
                );

                app.add(
                    "telemetry_left",
                    Component::new("telem_left_title", Role::Text, "Architecture Metrics")
                        .with_prop("h", "20")
                        .with_prop("font_vars", "wght 700")
                        .with_prop("text", "Hamesh Telemetry:"),
                );

                let metrics = [
                    format!("Active Visual World: {}", world.label()),
                    format!("Font Family: {}", world.font_family()),
                    format!("Corner Radius: {:.0}px", world.corner_radius_sm()),
                    format!("Density Padding: {:.0}px", world.padding()),
                    format!("Control Height: {:.0}px", world.control_height()),
                    format!("Splitter Pos: {:.0}px", self.splitter.position),
                    format!("Frames Painted: {}", self.frame_count),
                ];

                for (idx, m) in metrics.iter().enumerate() {
                    app.add(
                        "telemetry_left",
                        Component::new(&format!("metric_{}", idx), Role::Text, m)
                            .with_prop("h", "20")
                            .with_prop("color", "text")
                            .with_prop("text", m),
                    );
                }

                // Splitter Handle
                self.splitter.build_component(app, "main_stage", world);

                // Right Panel
                app.add(
                    "main_stage",
                    Component::new("telemetry_right", Role::Panel, "Right Telemetry")
                        .with_prop("layout", "column")
                        .with_prop("flex_grow", "1")
                        .with_prop("pad", &pad_standard)
                        .with_prop("gap", &gap_standard)
                        .with_prop("surface", "raised")
                        .with_prop("border", &border_size)
                        .with_prop("border_token", "border")
                        .with_prop("radius", &card_radius),
                );

                let desc_heading = format!("Design Philosophy: {}", world.label());
                app.add(
                    "telemetry_right",
                    Component::new("diag_desc_title", Role::Text, "World Philosophy")
                        .with_prop("h", "20")
                        .with_prop("font_vars", "wght 700")
                        .with_prop("text", &desc_heading),
                );

                app.add(
                    "telemetry_right",
                    Component::new("diag_desc", Role::Text, "World Description")
                        .with_prop("h", "60")
                        .with_prop("color", "text_dim")
                        .with_prop("text", world.description()),
                );
            }
        }

        // 5. Status Bar (Reusable Component)
        if self.show_status_bar {
            self.status_bar
                .build_components(app, "workbench_root", world);
        }
    }
}

impl HameshApp for WorkbenchApp {
    fn init(&mut self, ctx: &mut ShellContext) {
        self.rebuild_ui(ctx.app);
        ctx.request_redraw();
    }

    fn on_mouse_click(&mut self, ctx: &mut ShellContext, button: MouseButton, state: ElementState) {
        if button != MouseButton::Left {
            return;
        }

        if state == ElementState::Released {
            if self.splitter.dragging {
                self.splitter.end_drag();
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
            return;
        }

        let (x, y) = ctx.cursor;
        let (_, scene) = self.view(ctx.app, ctx.size, 1);
        let hit_id = hamesh_scene::hit_test(ctx.app, &scene, x, y);

        let Some(ref comp_id_str) = hit_id else {
            if self.menu_bar.is_open() {
                self.menu_bar.close();
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
            return;
        };

        let comp_id = comp_id_str.as_str();

        // 1. Menu Bar Header Clicks
        if let Some(m_name) = comp_id.strip_prefix("menu_bar_btn_") {
            if let Some(idx) = self.menu_bar.menus.iter().position(|m| m.name == m_name) {
                self.menu_bar.toggle(idx);
                self.status_bar
                    .set_message(format!("Menu '{}' toggled.", m_name));
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
        }

        // 2. Dropdown Menu Item Clicks
        if let Some(menu) = self.menu_bar.current_menu() {
            if let Some(item) = menu.items.iter().find(|it| it.id == comp_id) {
                if item.enabled {
                    let cmd = item.command.clone().unwrap_or_else(|| item.id.clone());
                    self.execute_command(&cmd, ctx);
                    return;
                }
            }
        }

        // 3. Tab Buttons
        if let Some(rest) = comp_id.strip_prefix("workbench_tabs_btn_") {
            if let Some(idx) = self
                .tab_control
                .tabs
                .iter()
                .position(|(tid, _)| tid == rest)
            {
                self.tab_control.select(idx);
                self.status_bar
                    .set_message(format!("Switched to tab: {}", self.tab_control.tabs[idx].1));
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
        }

        // 4. World Buttons
        if let Some(w_name) = comp_id.strip_prefix("btn_world_") {
            if let Some(w) = hamesh_theme::world_by_name(w_name) {
                self.set_visual_world(w);
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
        }

        // 5. Checkbox Toggle
        if comp_id == "sample_checkbox" {
            self.checkbox.toggle();
            self.focus_ring.set_focus("sample_checkbox");
            self.status_bar.set_message(format!(
                "Checkbox toggled: {}",
                if self.checkbox.checked {
                    "Enabled"
                } else {
                    "Disabled"
                }
            ));
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
            return;
        }

        // 6. List Selection
        if let Some(row_str) = comp_id.strip_prefix("workbench_records_row_") {
            if let Ok(idx) = row_str.parse::<usize>() {
                self.list_control.select(idx);
                self.focus_ring.set_focus(comp_id);
                self.status_bar
                    .set_message(format!("Selected Record: #{}", idx + 1));
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
        }

        // 7. Splitter Start Drag
        if comp_id == "sidebar_splitter" {
            self.splitter.start_drag();
            return;
        }

        // 8. General Focus Setting
        if self.focus_ring.set_focus(comp_id) {
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
        }
    }

    fn on_cursor_move(&mut self, ctx: &mut ShellContext, (x, _y): (f32, f32)) {
        if self.splitter.dragging {
            self.splitter.drag_to(x);
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
        }
    }

    fn on_key(&mut self, ctx: &mut ShellContext, key: &Key, state: ElementState, _repeat: bool) {
        if state != ElementState::Pressed {
            return;
        }

        let modifiers = ctx.modifiers;

        // 1. Menu Bar Navigation when Menu is Open
        if self.menu_bar.is_open() {
            match key {
                Key::Named(NamedKey::Escape) => {
                    self.menu_bar.close();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    self.menu_bar.select_prev_menu();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowRight) => {
                    self.menu_bar.select_next_menu();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.menu_bar.select_next_item();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.menu_bar.select_prev_item();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    if let Some(cmd) = self.menu_bar.activate_current() {
                        self.execute_command(&cmd, ctx);
                        return;
                    }
                }
                Key::Character(ref ch) => {
                    if let Some(first_char) = ch.chars().next() {
                        if let Some(cmd) = self.menu_bar.activate_mnemonic(first_char) {
                            self.execute_command(&cmd, ctx);
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        // 2. Alt/F10 to enter menu bar
        if matches!(key, Key::Named(NamedKey::F10))
            || (modifiers.alt_key()
                && matches!(key, Key::Character(ref c) if c.eq_ignore_ascii_case("f")))
        {
            self.menu_bar.open(0);
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
            return;
        }

        // 3. Global Shortcuts (Ctrl+1..5, Ctrl+N, Ctrl+O, Ctrl+S)
        if modifiers.control_key() {
            match key {
                Key::Character(ref c) if c == "1" => {
                    self.execute_command("view_classic", ctx);
                    return;
                }
                Key::Character(ref c) if c == "2" => {
                    self.execute_command("view_modern", ctx);
                    return;
                }
                Key::Character(ref c) if c == "3" => {
                    self.execute_command("view_retro", ctx);
                    return;
                }
                Key::Character(ref c) if c == "4" => {
                    self.execute_command("view_technical", ctx);
                    return;
                }
                Key::Character(ref c) if c == "5" => {
                    self.execute_command("view_editorial", ctx);
                    return;
                }
                Key::Character(ref c) if c.eq_ignore_ascii_case("n") => {
                    self.execute_command("file_new", ctx);
                    return;
                }
                Key::Character(ref c) if c.eq_ignore_ascii_case("o") => {
                    self.execute_command("file_open", ctx);
                    return;
                }
                Key::Character(ref c) if c.eq_ignore_ascii_case("s") => {
                    self.execute_command("file_save", ctx);
                    return;
                }
                _ => {}
            }
        }

        // 4. Focus Navigation (Tab / Shift+Tab)
        if matches!(key, Key::Named(NamedKey::Tab)) {
            if modifiers.shift_key() {
                self.focus_ring.shift_tab();
            } else {
                self.focus_ring.tab();
            }
            if let Some(f) = self.focus_ring.current() {
                self.status_bar.set_message(format!("Focus -> {}", f));
            }
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
            return;
        }

        // 5. Active Focused Control Input
        let current_focus = self.focus_ring.current().unwrap_or("").to_string();

        if current_focus == "sample_text_input" {
            match key {
                Key::Character(ref c) => {
                    self.text_input_value.push_str(c);
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                Key::Named(NamedKey::Backspace) => {
                    self.text_input_value.pop();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                _ => {}
            }
        } else if current_focus == "editor_text_area" {
            match key {
                Key::Character(ref c) => {
                    self.editor_content.push_str(c);
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                Key::Named(NamedKey::Enter) => {
                    self.editor_content.push('\n');
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                Key::Named(NamedKey::Backspace) => {
                    self.editor_content.pop();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                _ => {}
            }
        } else if current_focus == "sample_checkbox" {
            if matches!(key, Key::Named(NamedKey::Space)) {
                self.checkbox.toggle();
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
        } else if current_focus.starts_with("workbench_records_row_") {
            match key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.list_control.select_next();
                    let new_id =
                        format!("workbench_records_row_{}", self.list_control.selected_idx);
                    self.focus_ring.set_focus(&new_id);
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.list_control.select_prev();
                    let new_id =
                        format!("workbench_records_row_{}", self.list_control.selected_idx);
                    self.focus_ring.set_focus(&new_id);
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                _ => {}
            }
        }
    }

    fn view(
        &mut self,
        app: &App,
        (w, h): (u32, u32),
        scene_rev: u64,
    ) -> (hamesh_layout::LayoutOutput, hamesh_scene::Scene) {
        self.frame_count += 1;
        self.status_bar
            .set_telemetry(format!("Frames: {} | 040 Cache Clean", self.frame_count));
        let layout = hamesh_layout::compute(app, w as f32, h as f32);
        let scene = hamesh_scene::build(app, &layout, scene_rev);
        (layout, scene)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let capture_flag = std::env::args().any(|a| a == "--capture-screenshots" || a == "--capture");
    if capture_flag {
        let mut workbench = WorkbenchApp::new();
        let mut app = App::new("root");
        workbench.capture_visual_receipts(&mut app);
        println!("Captured visual receipts for all 5 worlds to target/visual_receipts/");
        return Ok(());
    }

    let smoke_secs: Option<u64> =
        std::env::args().find_map(|a| a.strip_prefix("--smoke-test=").and_then(|v| v.parse().ok()));

    let workbench = WorkbenchApp::new();

    let config = ShellConfig {
        title: "Hamesh Workbench — Universal Desktop Architecture".to_string(),
        width: 1024,
        height: 768,
        smoke_seconds: smoke_secs,
        ..Default::default()
    };

    Shell::run(workbench, config)
}
