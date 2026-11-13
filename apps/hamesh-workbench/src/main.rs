//! Hamesh Workbench: Native demonstration of reusable desktop menus,
//! semantic command routing, keyboard focus traversal, split panels, frame diagnostics,
//! and live visual profiles.

use hamesh_core::{App, Component, Role};
use hamesh_shell::{HameshApp, Shell, ShellConfig, ShellContext};
use winit::event::{ElementState, MouseButton};
use winit::keyboard::{Key, NamedKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualProfile {
    ClassicDesktop,
    ModernStudio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveTab {
    Controls,
    Layout,
    Diagnostics,
}

/// Commands executed either via Menu Bar, Context Menus, or Keyboard Shortcuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkbenchCommand {
    FileNew,
    FileOpen,
    FileSave,
    FileExit,
    ViewProfileClassic,
    ViewProfileModern,
    ViewToggleStatus,
    ToolsRunDiagnostics,
    ToolsResetFields,
    HelpAbout,
}

pub struct WorkbenchApp {
    profile: VisualProfile,
    active_tab: ActiveTab,
    open_menu: Option<&'static str>,
    menu_selected_idx: usize,
    focused_id: Option<String>,
    focus_ring: Vec<String>,
    checkbox_checked: bool,
    text_input_value: String,
    selected_list_item: usize,
    status_message: String,
    frame_count: u64,
    left_panel_width: f32,
    dragging_splitter: bool,
    show_status_bar: bool,
}

impl Default for WorkbenchApp {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkbenchApp {
    pub fn new() -> Self {
        Self {
            profile: VisualProfile::ModernStudio,
            active_tab: ActiveTab::Controls,
            open_menu: None,
            menu_selected_idx: 0,
            focused_id: Some("sample_text_input".to_string()),
            focus_ring: vec![
                "sample_text_input".to_string(),
                "checkbox_container".to_string(),
                "profile_btn_classic".to_string(),
                "profile_btn_modern".to_string(),
                "list_row_0".to_string(),
                "list_row_1".to_string(),
                "list_row_2".to_string(),
            ],
            checkbox_checked: true,
            text_input_value: "Hamesh Workbench v0.1".to_string(),
            selected_list_item: 0,
            status_message: "Ready. Use mouse or Alt+F/E/V/T/H or Tab for focus.".to_string(),
            frame_count: 0,
            left_panel_width: 240.0,
            dragging_splitter: false,
            show_status_bar: true,
        }
    }

    fn execute_command(&mut self, cmd: WorkbenchCommand, ctx: &mut ShellContext) {
        match cmd {
            WorkbenchCommand::FileNew => {
                self.text_input_value = "Untitled Document".to_string();
                self.status_message = "Executed: File -> New Document".to_string();
            }
            WorkbenchCommand::FileOpen => {
                if let Ok(content) = std::fs::read_to_string("workbench_demo.txt") {
                    self.text_input_value = content;
                    self.status_message =
                        "File -> Open: Read text from workbench_demo.txt".to_string();
                } else {
                    self.text_input_value =
                        "Loaded demo template (workbench_demo.txt not found on disk)".to_string();
                    self.status_message =
                        "Demo: No workbench_demo.txt found; loaded template into editor."
                            .to_string();
                }
            }
            WorkbenchCommand::FileSave => {
                match std::fs::write("workbench_demo.txt", &self.text_input_value) {
                    Ok(_) => {
                        self.status_message = format!(
                            "File -> Save: Written {} bytes to workbench_demo.txt",
                            self.text_input_value.len()
                        );
                    }
                    Err(e) => {
                        self.status_message = format!("File -> Save failed: {}", e);
                    }
                }
            }
            WorkbenchCommand::FileExit => {
                ctx.exit();
                return;
            }
            WorkbenchCommand::ViewProfileClassic => {
                self.profile = VisualProfile::ClassicDesktop;
                self.status_message =
                    "Switched to Classic Desktop profile [90s square boundaries].".to_string();
            }
            WorkbenchCommand::ViewProfileModern => {
                self.profile = VisualProfile::ModernStudio;
                self.status_message =
                    "Switched to Modern Studio profile [2026 refined spacing].".to_string();
            }
            WorkbenchCommand::ViewToggleStatus => {
                self.show_status_bar = !self.show_status_bar;
                self.status_message = format!("Status bar visible: {}", self.show_status_bar);
            }
            WorkbenchCommand::ToolsRunDiagnostics => {
                self.active_tab = ActiveTab::Diagnostics;
                self.status_message = "Executed: Tools -> Run Diagnostics Battery".to_string();
            }
            WorkbenchCommand::ToolsResetFields => {
                self.text_input_value.clear();
                self.checkbox_checked = false;
                self.selected_list_item = 0;
                self.status_message = "Executed: Tools -> Reset Form Fields".to_string();
            }
            WorkbenchCommand::HelpAbout => {
                self.status_message =
                    "Hamesh Workbench v0.1 — Universal Native GUI Architecture".to_string();
            }
        }
        self.open_menu = None;
        self.rebuild_ui(ctx.app);
        ctx.request_redraw();
    }

    fn rebuild_ui(&mut self, app: &mut App) {
        let root_id = app.root.0.clone();
        app.components.clear();
        app.commands.clear();
        app.components.insert(
            root_id.clone(),
            Component::new(&root_id, Role::Window, "root"),
        );
        app.revision += 1;

        let is_classic = self.profile == VisualProfile::ClassicDesktop;
        let font_family = if is_classic {
            "Segoe UI"
        } else {
            "Instrument Sans"
        };
        let menu_bg = if is_classic { "surface" } else { "header" };
        let card_bg = if is_classic { "surface" } else { "raised" };
        let border_size = "1";
        let radius = if is_classic { "0" } else { "6" };

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

        // 2. Menu Bar (File / Edit / View / Tools / Help)
        app.add(
            "workbench_root",
            Component::new("menu_bar", Role::Header, "Menu Bar")
                .with_prop("layout", "row")
                .with_prop("h", "30")
                .with_prop("surface", menu_bg)
                .with_prop("border", border_size)
                .with_prop("pad", "4")
                .with_prop("gap", "4"),
        );

        let menus = ["File", "Edit", "View", "Tools", "Help"];
        for m in menus {
            let is_open = self.open_menu == Some(m);
            let btn_surface = if is_open { "accent" } else { "header" };
            let text_color = if is_open { "bg" } else { "text" };
            app.add(
                "menu_bar",
                Component::new(&format!("menu_bar_btn_{m}"), Role::Button, m)
                    .with_prop("layout", "row")
                    .with_prop("h", "22")
                    .with_prop("pad", "6")
                    .with_prop("surface", btn_surface)
                    .with_prop("color", text_color)
                    .with_prop("radius", radius)
                    .with_prop("text", m),
            );
        }

        // 2b. Dropdown Menu Overlay (if a menu is open)
        if let Some(menu_name) = self.open_menu {
            let dropdown_items: &[(&str, &str, bool, Option<WorkbenchCommand>)] = match menu_name {
                "File" => &[
                    (
                        "menu_file_new",
                        "New Document (Ctrl+N)",
                        true,
                        Some(WorkbenchCommand::FileNew),
                    ),
                    (
                        "menu_file_open",
                        "Open File... (Ctrl+O)",
                        true,
                        Some(WorkbenchCommand::FileOpen),
                    ),
                    (
                        "menu_file_save",
                        "Save Changes (Ctrl+S)",
                        true,
                        Some(WorkbenchCommand::FileSave),
                    ),
                    (
                        "menu_file_exit",
                        "Exit Application",
                        true,
                        Some(WorkbenchCommand::FileExit),
                    ),
                ],
                "View" => &[
                    (
                        "menu_view_classic",
                        "Classic Desktop Profile",
                        true,
                        Some(WorkbenchCommand::ViewProfileClassic),
                    ),
                    (
                        "menu_view_modern",
                        "Modern Studio Profile",
                        true,
                        Some(WorkbenchCommand::ViewProfileModern),
                    ),
                    (
                        "menu_view_toggle_status",
                        "Toggle Status Bar",
                        true,
                        Some(WorkbenchCommand::ViewToggleStatus),
                    ),
                ],
                "Tools" => &[
                    (
                        "menu_tools_diag",
                        "Run Diagnostics Battery",
                        true,
                        Some(WorkbenchCommand::ToolsRunDiagnostics),
                    ),
                    (
                        "menu_tools_reset",
                        "Reset Form Fields",
                        true,
                        Some(WorkbenchCommand::ToolsResetFields),
                    ),
                ],
                _ => &[(
                    "menu_item_disabled",
                    "No actions in category (Disabled)",
                    false,
                    None,
                )],
            };

            let ox = match menu_name {
                "File" => "4",
                "Edit" => "44",
                "View" => "84",
                "Tools" => "126",
                "Help" => "170",
                _ => "4",
            };

            app.add(
                "workbench_root",
                Component::new("dropdown_overlay", Role::Overlay, "Menu Dropdown")
                    .with_prop("layout", "column")
                    .with_prop("w", "250")
                    .with_prop("surface", "raised")
                    .with_prop("border", "1")
                    .with_prop("radius", radius)
                    .with_prop("pad", "4")
                    .with_prop("gap", "2")
                    .with_prop("ox", ox)
                    .with_prop("oy", "32")
                    .with_prop("z", "100"),
            );

            for (idx, (id, label, enabled, _)) in dropdown_items.iter().enumerate() {
                let is_hovered = self.menu_selected_idx == idx;
                let text_color = if !*enabled {
                    "muted"
                } else if is_hovered {
                    "bg"
                } else {
                    "text"
                };
                let surface = if is_hovered && *enabled {
                    "accent"
                } else {
                    "raised"
                };

                app.add(
                    "dropdown_overlay",
                    Component::new(id, Role::Button, label)
                        .with_prop("layout", "row")
                        .with_prop("h", "24")
                        .with_prop("pad", "6")
                        .with_prop("surface", surface)
                        .with_prop("color", text_color)
                        .with_prop("radius", radius)
                        .with_prop("text", label),
                );
            }
        }

        // 3. Tab Bar
        app.add(
            "workbench_root",
            Component::new("tab_bar", Role::Panel, "Tab Bar")
                .with_prop("layout", "row")
                .with_prop("h", "36")
                .with_prop("surface", "surface")
                .with_prop("pad", "4")
                .with_prop("gap", "8"),
        );

        let tabs = [
            (ActiveTab::Controls, "tab_btn_controls", "Controls & Inputs"),
            (
                ActiveTab::Layout,
                "tab_btn_layout",
                "Resizable Split Panels",
            ),
            (ActiveTab::Diagnostics, "tab_btn_diag", "Frame Diagnostics"),
        ];

        for (t, id, label) in tabs {
            let is_active = self.active_tab == t;
            let tab_surface = if is_active { "window" } else { "surface" };
            let tab_color = if is_active { "accent" } else { "text_dim" };
            let border_spec = if is_active && is_classic { "1" } else { "0" };

            app.add(
                "tab_bar",
                Component::new(id, Role::Button, label)
                    .with_prop("layout", "row")
                    .with_prop("h", "28")
                    .with_prop("pad", "8")
                    .with_prop("surface", tab_surface)
                    .with_prop("color", tab_color)
                    .with_prop("border", border_spec)
                    .with_prop("radius", radius)
                    .with_prop("text", label),
            );
        }

        // 4. Main Content Area
        app.add(
            "workbench_root",
            Component::new("main_stage", Role::Panel, "Main Stage")
                .with_prop("layout", "row")
                .with_prop("flex_grow", "1")
                .with_prop("pad", "8")
                .with_prop("gap", "8")
                .with_prop("surface", "window"),
        );

        match self.active_tab {
            ActiveTab::Controls => {
                // Left Panel: Interactive controls
                app.add(
                    "main_stage",
                    Component::new("controls_left", Role::Panel, "Controls Left")
                        .with_prop("layout", "column")
                        .with_prop("w", "340")
                        .with_prop("pad", "12")
                        .with_prop("gap", "10")
                        .with_prop("surface", card_bg)
                        .with_prop("border", border_size)
                        .with_prop("radius", radius),
                );

                app.add(
                    "controls_left",
                    Component::new("profile_heading", Role::Text, "Profile Selection")
                        .with_prop("h", "20")
                        .with_prop("font_vars", "wght 700")
                        .with_prop("text", "Visual Profiles"),
                );

                // Profile toggle buttons
                let is_focused_classic = self.focused_id.as_deref() == Some("profile_btn_classic");
                app.add(
                    "controls_left",
                    Component::new("profile_btn_classic", Role::Button, "Classic Desktop")
                        .with_prop("h", "30")
                        .with_prop("pad", "8")
                        .with_prop("surface", if is_classic { "accent" } else { "surface" })
                        .with_prop("color", if is_classic { "bg" } else { "text" })
                        .with_prop("border", if is_focused_classic { "2" } else { "0" })
                        .with_prop("radius", radius)
                        .with_prop("text", "Classic Desktop (Square, Compact)"),
                );

                let is_focused_modern = self.focused_id.as_deref() == Some("profile_btn_modern");
                app.add(
                    "controls_left",
                    Component::new("profile_btn_modern", Role::Button, "Modern Studio")
                        .with_prop("h", "30")
                        .with_prop("pad", "8")
                        .with_prop("surface", if !is_classic { "accent" } else { "surface" })
                        .with_prop("color", if !is_classic { "bg" } else { "text" })
                        .with_prop("border", if is_focused_modern { "2" } else { "0" })
                        .with_prop("radius", radius)
                        .with_prop("text", "Modern Studio (Refined Spacing)"),
                );

                let is_focused_chk = self.focused_id.as_deref() == Some("checkbox_container");
                app.add(
                    "controls_left",
                    Component::new("checkbox_container", Role::Button, "Checkbox")
                        .with_prop("layout", "row")
                        .with_prop("h", "28")
                        .with_prop("gap", "8")
                        .with_prop("pad", "4")
                        .with_prop("surface", "surface")
                        .with_prop("border", if is_focused_chk { "2" } else { "0" })
                        .with_prop("radius", radius)
                        .with_prop(
                            "text",
                            if self.checkbox_checked {
                                "[X] Option Enabled (Space to toggle)"
                            } else {
                                "[ ] Option Disabled (Space to toggle)"
                            },
                        ),
                );

                app.add(
                    "controls_left",
                    Component::new("text_field_label", Role::Text, "Input Field Label")
                        .with_prop("h", "16")
                        .with_prop("text", "Text Field Input (Tab focuses, type to edit):"),
                );

                let is_focused_input = self.focused_id.as_deref() == Some("sample_text_input");
                let border_color = if is_focused_input { "accent" } else { "border" };
                app.add(
                    "controls_left",
                    Component::new("sample_text_input", Role::TextInput, "Sample Input")
                        .with_prop("h", "32")
                        .with_prop("pad", "6")
                        .with_prop("surface", "window")
                        .with_prop("border", if is_focused_input { "2" } else { "1" })
                        .with_prop("color", border_color)
                        .with_prop("radius", radius)
                        .with_prop("text", &self.text_input_value),
                );

                // Right Panel: Selectable List
                app.add(
                    "main_stage",
                    Component::new("controls_right", Role::Panel, "Controls Right")
                        .with_prop("layout", "column")
                        .with_prop("flex_grow", "1")
                        .with_prop("pad", "12")
                        .with_prop("gap", "6")
                        .with_prop("surface", card_bg)
                        .with_prop("border", border_size)
                        .with_prop("radius", radius),
                );

                app.add(
                    "controls_right",
                    Component::new("list_heading", Role::Text, "Selectable Item List")
                        .with_prop("h", "20")
                        .with_prop("font_vars", "wght 700")
                        .with_prop(
                            "text",
                            "Selectable Desktop Records (Click or Up/Down arrows)",
                        ),
                );

                let list_items = [
                    "001 - Hamesh Shell Lifecycle (Verified)",
                    "002 - Cross-Language C ABI Binding (Ready)",
                    "003 - Visual Profile: Classic Desktop (Active)",
                    "004 - Visual Profile: Modern Studio (Active)",
                    "005 - Monotonic Eye/Hand Audit (Verified)",
                ];

                for (idx, item) in list_items.iter().enumerate() {
                    let is_sel = self.selected_list_item == idx;
                    let is_focused = self.focused_id.as_deref() == Some(&format!("list_row_{idx}"));
                    let row_surface = if is_sel { "accent" } else { "surface" };
                    let row_color = if is_sel { "bg" } else { "text" };

                    app.add(
                        "controls_right",
                        Component::new(&format!("list_row_{idx}"), Role::ListItem, item)
                            .with_prop("h", "28")
                            .with_prop("pad", "8")
                            .with_prop("surface", row_surface)
                            .with_prop("color", row_color)
                            .with_prop("border", if is_focused { "2" } else { "0" })
                            .with_prop("radius", radius)
                            .with_prop("text", item),
                    );
                }
            }
            ActiveTab::Layout => {
                let left_w = format!("{:.0}", self.left_panel_width);
                app.add(
                    "main_stage",
                    Component::new("split_panel_left", Role::Panel, "Left Split Panel")
                        .with_prop("layout", "column")
                        .with_prop("w", &left_w)
                        .with_prop("pad", "12")
                        .with_prop("surface", card_bg)
                        .with_prop("border", border_size)
                        .with_prop("radius", radius)
                        .with_prop("text", "Left Panel (Drag splitter to resize)"),
                );

                app.add(
                    "main_stage",
                    Component::new("splitter_handle", Role::Button, "Splitter Handle")
                        .with_prop("w", "8")
                        .with_prop(
                            "surface",
                            if self.dragging_splitter {
                                "accent"
                            } else {
                                "border"
                            },
                        )
                        .with_prop("cursor", "col-resize"),
                );

                app.add(
                    "main_stage",
                    Component::new("split_panel_right", Role::Panel, "Right Split Panel")
                        .with_prop("layout", "column")
                        .with_prop("flex_grow", "1")
                        .with_prop("pad", "12")
                        .with_prop("surface", card_bg)
                        .with_prop("border", border_size)
                        .with_prop("radius", radius)
                        .with_prop("text", "Right Panel (Flex Fill Content Area)"),
                );
            }
            ActiveTab::Diagnostics => {
                app.add(
                    "main_stage",
                    Component::new("diag_panel", Role::Panel, "Diagnostics")
                        .with_prop("layout", "column")
                        .with_prop("flex_grow", "1")
                        .with_prop("pad", "16")
                        .with_prop("gap", "12")
                        .with_prop("surface", card_bg)
                        .with_prop("border", border_size)
                        .with_prop("radius", radius),
                );

                app.add(
                    "diag_panel",
                    Component::new("diag_title", Role::Text, "Engine Runtime Telemetry")
                        .with_prop("h", "24")
                        .with_prop("font_vars", "wght 700")
                        .with_prop("text", "Hamesh GUI Telemetry & Pipeline Performance"),
                );

                let diag_info = format!(
                    "Frames Painted: {} | Current App Revision: {} | Active Profile: {:?} | Focus: {:?}",
                    self.frame_count, app.revision, self.profile, self.focused_id
                );

                app.add(
                    "diag_panel",
                    Component::new("diag_stats", Role::Text, "Telemetry Stats")
                        .with_prop("h", "20")
                        .with_prop("text", &diag_info),
                );
            }
        }

        // 5. Status Bar
        if self.show_status_bar {
            app.add(
                "workbench_root",
                Component::new("status_bar", Role::Status, "Status Bar")
                    .with_prop("layout", "row")
                    .with_prop("h", "24")
                    .with_prop("surface", "header")
                    .with_prop("border", "1")
                    .with_prop("pad", "4")
                    .with_prop("gap", "12"),
            );

            app.add(
                "status_bar",
                Component::new("status_text", Role::Text, "Status Message")
                    .with_prop("flex_grow", "1")
                    .with_prop("color", "text_dim")
                    .with_prop("text", &self.status_message),
            );

            let profile_tag = match self.profile {
                VisualProfile::ClassicDesktop => "Profile: Classic Desktop [90s]",
                VisualProfile::ModernStudio => "Profile: Modern Studio [2026]",
            };

            app.add(
                "status_bar",
                Component::new("status_profile_tag", Role::Text, "Profile Indicator")
                    .with_prop("w", "200")
                    .with_prop("color", "accent")
                    .with_prop("text", profile_tag),
            );
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
            if self.dragging_splitter {
                self.dragging_splitter = false;
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
            return;
        }

        let (x, y) = ctx.cursor;
        let (_, scene) = self.view(ctx.app, ctx.size, 1);

        // Visibility-aware and clipping-aware hit test:
        let hit_id = hamesh_scene::hit_test(ctx.app, &scene, x, y);

        let Some(ref comp_id_str) = hit_id else {
            if self.open_menu.is_some() {
                self.open_menu = None;
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
            return;
        };

        let comp_id = comp_id_str.as_str();

        // 1. Menu Bar Clicks
        if let Some(m) = comp_id.strip_prefix("menu_bar_btn_") {
            if self.open_menu
                == Some(match m {
                    "File" => "File",
                    "Edit" => "Edit",
                    "View" => "View",
                    "Tools" => "Tools",
                    "Help" => "Help",
                    _ => "",
                })
            {
                self.open_menu = None;
            } else {
                self.open_menu = Some(match m {
                    "File" => "File",
                    "Edit" => "Edit",
                    "View" => "View",
                    "Tools" => "Tools",
                    _ => "Help",
                });
                self.menu_selected_idx = 0;
                self.status_message = format!("Menu '{m}' opened.");
            }
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
            return;
        }

        // 2. Dropdown Menu Item Clicks (Semantic ID routing)
        match comp_id {
            "menu_file_new" => {
                self.execute_command(WorkbenchCommand::FileNew, ctx);
                return;
            }
            "menu_file_open" => {
                self.execute_command(WorkbenchCommand::FileOpen, ctx);
                return;
            }
            "menu_file_save" => {
                self.execute_command(WorkbenchCommand::FileSave, ctx);
                return;
            }
            "menu_file_exit" => {
                self.execute_command(WorkbenchCommand::FileExit, ctx);
                return;
            }
            "menu_view_classic" => {
                self.execute_command(WorkbenchCommand::ViewProfileClassic, ctx);
                return;
            }
            "menu_view_modern" => {
                self.execute_command(WorkbenchCommand::ViewProfileModern, ctx);
                return;
            }
            "menu_view_toggle_status" => {
                self.execute_command(WorkbenchCommand::ViewToggleStatus, ctx);
                return;
            }
            "menu_tools_diag" => {
                self.execute_command(WorkbenchCommand::ToolsRunDiagnostics, ctx);
                return;
            }
            "menu_tools_reset" => {
                self.execute_command(WorkbenchCommand::ToolsResetFields, ctx);
                return;
            }
            _ => {}
        }

        // Dismiss open menu if clicked outside
        if self.open_menu.is_some() {
            self.open_menu = None;
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
        }

        // 3. Tab Clicks
        match comp_id {
            "tab_btn_controls" => {
                self.active_tab = ActiveTab::Controls;
                self.status_message = "Switched to Controls & Inputs tab.".to_string();
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
            "tab_btn_layout" => {
                self.active_tab = ActiveTab::Layout;
                self.status_message = "Switched to Resizable Split Panels tab.".to_string();
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
            "tab_btn_diag" => {
                self.active_tab = ActiveTab::Diagnostics;
                self.status_message = "Switched to Frame Diagnostics tab.".to_string();
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
            _ => {}
        }

        // 4. Controls & Input interactions
        match comp_id {
            "profile_btn_classic" => {
                self.focused_id = Some("profile_btn_classic".to_string());
                self.execute_command(WorkbenchCommand::ViewProfileClassic, ctx);
                return;
            }
            "profile_btn_modern" => {
                self.focused_id = Some("profile_btn_modern".to_string());
                self.execute_command(WorkbenchCommand::ViewProfileModern, ctx);
                return;
            }
            "checkbox_container" => {
                self.focused_id = Some("checkbox_container".to_string());
                self.checkbox_checked = !self.checkbox_checked;
                self.status_message = format!("Checkbox toggled: {}", self.checkbox_checked);
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
            "sample_text_input" => {
                self.focused_id = Some("sample_text_input".to_string());
                self.status_message = "Text input focused. Type or press Backspace.".to_string();
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            }
            "splitter_handle" => {
                self.dragging_splitter = true;
                self.status_message = "Dragging splitter...".to_string();
                return;
            }
            _ => {}
        }

        // List row clicks
        if let Some(idx_str) = comp_id.strip_prefix("list_row_") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                self.focused_id = Some(comp_id.to_string());
                self.selected_list_item = idx;
                self.status_message = format!("Selected List Record #{idx}");
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
        }
    }

    fn on_cursor_move(&mut self, ctx: &mut ShellContext, pos: (f32, f32)) {
        if self.dragging_splitter {
            let new_w = (pos.0 - 8.0).clamp(140.0, (ctx.size.0 as f32) - 160.0);
            if (new_w - self.left_panel_width).abs() > 1.0 {
                self.left_panel_width = new_w;
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
        }
    }

    fn on_key(&mut self, ctx: &mut ShellContext, key: &Key, state: ElementState, _repeat: bool) {
        if state != ElementState::Pressed {
            return;
        }

        // 1. Menu Navigation Keys (Escape, Arrows, Enter)
        if let Some(menu) = self.open_menu {
            let max_idx = match menu {
                "File" => 3,
                "View" => 2,
                "Tools" => 1,
                _ => 0,
            };

            match key {
                Key::Named(NamedKey::Escape) => {
                    self.open_menu = None;
                    self.status_message = "Menu closed with Escape.".to_string();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    if self.menu_selected_idx < max_idx {
                        self.menu_selected_idx += 1;
                    } else {
                        self.menu_selected_idx = 0;
                    }
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    if self.menu_selected_idx > 0 {
                        self.menu_selected_idx -= 1;
                    } else {
                        self.menu_selected_idx = max_idx;
                    }
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    let cmd = match (menu, self.menu_selected_idx) {
                        ("File", 0) => Some(WorkbenchCommand::FileNew),
                        ("File", 1) => Some(WorkbenchCommand::FileOpen),
                        ("File", 2) => Some(WorkbenchCommand::FileSave),
                        ("File", 3) => Some(WorkbenchCommand::FileExit),
                        ("View", 0) => Some(WorkbenchCommand::ViewProfileClassic),
                        ("View", 1) => Some(WorkbenchCommand::ViewProfileModern),
                        ("View", 2) => Some(WorkbenchCommand::ViewToggleStatus),
                        ("Tools", 0) => Some(WorkbenchCommand::ToolsRunDiagnostics),
                        ("Tools", 1) => Some(WorkbenchCommand::ToolsResetFields),
                        _ => None,
                    };
                    if let Some(c) = cmd {
                        self.execute_command(c, ctx);
                    }
                    return;
                }
                _ => {}
            }
        }

        // 2. Focus Navigation Keys (Tab / Shift-Tab)
        if let Key::Named(NamedKey::Tab) = key {
            if ctx.modifiers.shift_key() {
                // Shift-Tab: focus previous
                if let Some(ref cur) = self.focused_id {
                    if let Some(pos) = self.focus_ring.iter().position(|id| id == cur) {
                        let new_idx = if pos == 0 {
                            self.focus_ring.len() - 1
                        } else {
                            pos - 1
                        };
                        self.focused_id = Some(self.focus_ring[new_idx].clone());
                    }
                } else {
                    self.focused_id = self.focus_ring.last().cloned();
                }
            } else {
                // Tab: focus next
                if let Some(ref cur) = self.focused_id {
                    if let Some(pos) = self.focus_ring.iter().position(|id| id == cur) {
                        let new_idx = (pos + 1) % self.focus_ring.len();
                        self.focused_id = Some(self.focus_ring[new_idx].clone());
                    }
                } else {
                    self.focused_id = self.focus_ring.first().cloned();
                }
            }
            self.status_message = format!("Focus moved to: {:?}", self.focused_id);
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
            return;
        }

        // 3. Control-Specific Key Handling
        match self.focused_id.as_deref() {
            Some("sample_text_input") => match key {
                Key::Named(NamedKey::Backspace) => {
                    self.text_input_value.pop();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                Key::Named(NamedKey::Space) => {
                    self.text_input_value.push(' ');
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                Key::Character(s) => {
                    self.text_input_value.push_str(s.as_str());
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
                _ => {}
            },
            Some("checkbox_container") => {
                if let Key::Named(NamedKey::Space) | Key::Named(NamedKey::Enter) = key {
                    self.checkbox_checked = !self.checkbox_checked;
                    self.status_message =
                        format!("Checkbox toggled with Space: {}", self.checkbox_checked);
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
            }
            Some("profile_btn_classic") => {
                if let Key::Named(NamedKey::Space) | Key::Named(NamedKey::Enter) = key {
                    self.execute_command(WorkbenchCommand::ViewProfileClassic, ctx);
                }
            }
            Some("profile_btn_modern") => {
                if let Key::Named(NamedKey::Space) | Key::Named(NamedKey::Enter) = key {
                    self.execute_command(WorkbenchCommand::ViewProfileModern, ctx);
                }
            }
            _ => {}
        }
    }

    fn on_idle(&mut self, _ctx: &mut ShellContext) {
        self.frame_count += 1;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let smoke_secs: Option<u64> =
        std::env::args().find_map(|a| a.strip_prefix("--smoke-test=").and_then(|v| v.parse().ok()));

    let config = ShellConfig {
        title: "Hamesh Workbench — Native Multi-Profile Suite".to_string(),
        width: 860,
        height: 640,
        min_width: 600,
        min_height: 480,
        enable_inspect_server: true,
        inspect_port: None,
        smoke_seconds: smoke_secs,
    };

    println!("Starting Hamesh Workbench with semantic command routing & keyboard focus...");
    Shell::run(WorkbenchApp::new(), config)
}
