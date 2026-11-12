//! Hamesh Workbench: Native demonstration of reusable desktop menus,
//! interactive controls, split panels, frame diagnostics, and live visual profiles.

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

pub struct WorkbenchApp {
    profile: VisualProfile,
    active_tab: ActiveTab,
    open_menu: Option<&'static str>,
    checkbox_checked: bool,
    text_input_value: String,
    selected_list_item: usize,
    status_message: String,
    frame_count: u64,
    left_panel_width: f32,
    dragging_splitter: bool,
}

impl WorkbenchApp {
    pub fn new() -> Self {
        Self {
            profile: VisualProfile::ModernStudio,
            active_tab: ActiveTab::Controls,
            open_menu: None,
            checkbox_checked: true,
            text_input_value: "Hamesh Workbench v0.1".to_string(),
            selected_list_item: 0,
            status_message: "Ready. Select commands from menus or press Tab/Space/Enter.".to_string(),
            frame_count: 0,
            left_panel_width: 240.0,
            dragging_splitter: false,
        }
    }

    fn rebuild_ui(&mut self, app: &mut App) {
        let root_id = app.root.0.clone();
        app.components.clear();
        app.commands.clear();
        app.components.insert(root_id.clone(), Component::new(&root_id, Role::Window, "root"));
        app.revision += 1;

        let is_classic = self.profile == VisualProfile::ClassicDesktop;
        let font_family = if is_classic { "Segoe UI" } else { "Instrument Sans" };
        let menu_bg = if is_classic { "surface" } else { "header" };
        let card_bg = if is_classic { "surface" } else { "raised" };
        let border_size = if is_classic { "1" } else { "1" };
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
                Component::new(&format!("menu_item_{m}"), Role::Button, m)
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
            let dropdown_items: &[(&str, &str, bool)] = match menu_name {
                "File" => &[
                    ("file_new", "New Workbench Item", true),
                    ("file_open", "Open File...", true),
                    ("file_save", "Save Changes", true),
                    ("file_exit", "Exit Application", true),
                ],
                "View" => &[
                    ("view_classic", "Switch to Classic Desktop Profile", true),
                    ("view_modern", "Switch to Modern Studio Profile", true),
                    ("view_toggle_status", "Toggle Status Bar", true),
                ],
                "Tools" => &[
                    ("tools_diag", "Run Diagnostics Battery", true),
                    ("tools_reset", "Reset Form Fields", true),
                ],
                _ => &[
                    ("item_info", "No actions in this category", false),
                ],
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
                    .with_prop("w", "240")
                    .with_prop("surface", "raised")
                    .with_prop("border", "1")
                    .with_prop("radius", radius)
                    .with_prop("pad", "4")
                    .with_prop("gap", "2")
                    .with_prop("ox", ox)
                    .with_prop("oy", "32")
                    .with_prop("z", "100"),
            );

            for (id, label, enabled) in dropdown_items {
                let text_color = if *enabled { "text" } else { "muted" };
                let surface = "raised";
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
            (ActiveTab::Controls, "tab_controls", "Controls & Inputs"),
            (ActiveTab::Layout, "tab_layout", "Resizable Split Panels"),
            (ActiveTab::Diagnostics, "tab_diag", "Frame Diagnostics"),
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

        // 4. Main Content Area (Split panels or Tab content)
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
                        .with_prop("w", "320")
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
                app.add(
                    "controls_left",
                    Component::new("profile_btn_classic", Role::Button, "Classic Desktop")
                        .with_prop("h", "30")
                        .with_prop("pad", "8")
                        .with_prop("surface", if is_classic { "accent" } else { "surface" })
                        .with_prop("color", if is_classic { "bg" } else { "text" })
                        .with_prop("radius", radius)
                        .with_prop("text", "Classic Desktop (Square, Compact)"),
                );

                app.add(
                    "controls_left",
                    Component::new("profile_btn_modern", Role::Button, "Modern Studio")
                        .with_prop("h", "30")
                        .with_prop("pad", "8")
                        .with_prop("surface", if !is_classic { "accent" } else { "surface" })
                        .with_prop("color", if !is_classic { "bg" } else { "text" })
                        .with_prop("radius", radius)
                        .with_prop("text", "Modern Studio (Refined Spacing)"),
                );

                app.add(
                    "controls_left",
                    Component::new("checkbox_container", Role::Button, "Checkbox")
                        .with_prop("layout", "row")
                        .with_prop("h", "28")
                        .with_prop("gap", "8")
                        .with_prop("pad", "4")
                        .with_prop("surface", "surface")
                        .with_prop("radius", radius)
                        .with_prop("text", if self.checkbox_checked { "[X] Option Enabled" } else { "[ ] Option Disabled" }),
                );

                app.add(
                    "controls_left",
                    Component::new("text_field_label", Role::Text, "Input Field Label")
                        .with_prop("h", "16")
                        .with_prop("text", "Text Field Input:"),
                );

                app.add(
                    "controls_left",
                    Component::new("sample_text_input", Role::TextInput, "Sample Input")
                        .with_prop("h", "32")
                        .with_prop("pad", "6")
                        .with_prop("surface", "window")
                        .with_prop("border", "1")
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
                        .with_prop("text", "Selectable Desktop Records (click to select)"),
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
                    let row_surface = if is_sel { "accent" } else { "surface" };
                    let row_color = if is_sel { "bg" } else { "text" };

                    app.add(
                        "controls_right",
                        Component::new(&format!("list_row_{idx}"), Role::ListItem, item)
                            .with_prop("h", "28")
                            .with_prop("pad", "8")
                            .with_prop("surface", row_surface)
                            .with_prop("color", row_color)
                            .with_prop("radius", radius)
                            .with_prop("text", item),
                    );
                }
            }
            ActiveTab::Layout => {
                // Resizable Panels demonstration
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
                        .with_prop("text", "Left Panel (Resizable via Splitter)"),
                );

                // Splitter handle
                app.add(
                    "main_stage",
                    Component::new("splitter_handle", Role::Button, "Splitter Handle")
                        .with_prop("w", "8")
                        .with_prop("surface", if self.dragging_splitter { "accent" } else { "border" })
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
                        .with_prop("text", "Right Panel (Flex Fill Content)"),
                );
            }
            ActiveTab::Diagnostics => {
                // Diagnostics view
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
                    "Frames Painted: {} | Current App Revision: {} | Active Profile: {:?}",
                    self.frame_count, app.revision, self.profile
                );

                app.add(
                    "diag_panel",
                    Component::new("diag_stats", Role::Text, "Telemetry Stats")
                        .with_prop("h", "20")
                        .with_prop("text", &diag_info),
                );

                app.add(
                    "diag_panel",
                    Component::new("diag_desc", Role::Text, "Description")
                        .with_prop("text", "All metrics derive from the live engine state. Eye/Hand inspection TCP server is active."),
                );
            }
        }

        // 5. Status Bar
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

        // 1. Menu bar clicks
        if y >= 0.0 && y <= 32.0 {
            let menus = [
                ("File", 4.0..40.0),
                ("Edit", 44.0..80.0),
                ("View", 84.0..122.0),
                ("Tools", 126.0..168.0),
                ("Help", 170.0..210.0),
            ];

            for (m, range) in menus {
                if range.contains(&x) {
                    if self.open_menu == Some(m) {
                        self.open_menu = None;
                    } else {
                        self.open_menu = Some(m);
                        self.status_message = format!("Menu '{m}' opened.");
                    }
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                    return;
                }
            }
        }

        // 2. Dropdown menu clicks
        if let Some(menu) = self.open_menu {
            if y > 32.0 && y <= 160.0 && x <= 260.0 {
                match menu {
                    "View" => {
                        if y <= 60.0 {
                            self.profile = VisualProfile::ClassicDesktop;
                            self.status_message = "Switched to Classic Desktop visual profile.".to_string();
                        } else if y <= 86.0 {
                            self.profile = VisualProfile::ModernStudio;
                            self.status_message = "Switched to Modern Studio visual profile.".to_string();
                        }
                    }
                    "File" => {
                        if y > 110.0 {
                            ctx.exit();
                            return;
                        } else {
                            self.status_message = format!("Executed {menu} action.");
                        }
                    }
                    _ => {
                        self.status_message = format!("Menu item clicked in {menu}.");
                    }
                }
                self.open_menu = None;
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
                return;
            } else {
                // Click outside menu closes it
                self.open_menu = None;
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
        }

        // 3. Tab bar clicks
        if y > 30.0 && y <= 66.0 {
            if (4.0..140.0).contains(&x) {
                self.active_tab = ActiveTab::Controls;
                self.status_message = "Switched to Controls & Inputs tab.".to_string();
            } else if (144.0..280.0).contains(&x) {
                self.active_tab = ActiveTab::Layout;
                self.status_message = "Switched to Resizable Panels tab.".to_string();
            } else if (284.0..420.0).contains(&x) {
                self.active_tab = ActiveTab::Diagnostics;
                self.status_message = "Switched to Diagnostics tab.".to_string();
            }
            self.rebuild_ui(ctx.app);
            ctx.request_redraw();
            return;
        }

        // 4. Tab-specific clicks
        match self.active_tab {
            ActiveTab::Controls => {
                // Profile buttons in Controls tab
                if (20.0..320.0).contains(&x) {
                    if (100.0..132.0).contains(&y) {
                        self.profile = VisualProfile::ClassicDesktop;
                        self.status_message = "Profile: Classic Desktop selected.".to_string();
                        self.rebuild_ui(ctx.app);
                        ctx.request_redraw();
                    } else if (136.0..168.0).contains(&y) {
                        self.profile = VisualProfile::ModernStudio;
                        self.status_message = "Profile: Modern Studio selected.".to_string();
                        self.rebuild_ui(ctx.app);
                        ctx.request_redraw();
                    } else if (172.0..202.0).contains(&y) {
                        self.checkbox_checked = !self.checkbox_checked;
                        self.status_message = format!("Checkbox toggled: {}", self.checkbox_checked);
                        self.rebuild_ui(ctx.app);
                        ctx.request_redraw();
                    }
                }

                // List row clicks
                if x > 340.0 && (100.0..250.0).contains(&y) {
                    let row_idx = ((y - 100.0) / 30.0) as usize;
                    if row_idx < 5 {
                        self.selected_list_item = row_idx;
                        self.status_message = format!("Selected List Record #{row_idx}");
                        self.rebuild_ui(ctx.app);
                        ctx.request_redraw();
                    }
                }
            }
            ActiveTab::Layout => {
                let split_x = self.left_panel_width + 8.0;
                if (split_x - 6.0..split_x + 14.0).contains(&x) {
                    self.dragging_splitter = true;
                    self.status_message = "Dragging splitter...".to_string();
                }
            }
            ActiveTab::Diagnostics => {}
        }
    }

    fn on_cursor_move(&mut self, ctx: &mut ShellContext, pos: (f32, f32)) {
        if self.dragging_splitter {
            let new_w = (pos.0 - 8.0).clamp(120.0, (ctx.size.0 as f32) - 150.0);
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

        match key {
            Key::Named(NamedKey::Tab) => {
                self.active_tab = match self.active_tab {
                    ActiveTab::Controls => ActiveTab::Layout,
                    ActiveTab::Layout => ActiveTab::Diagnostics,
                    ActiveTab::Diagnostics => ActiveTab::Controls,
                };
                self.status_message = format!("Tab key navigated to: {:?}", self.active_tab);
                self.rebuild_ui(ctx.app);
                ctx.request_redraw();
            }
            Key::Named(NamedKey::Escape) => {
                if self.open_menu.is_some() {
                    self.open_menu = None;
                    self.status_message = "Menu closed with Escape.".to_string();
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
            }
            Key::Named(NamedKey::Space) => {
                if self.active_tab == ActiveTab::Controls {
                    self.checkbox_checked = !self.checkbox_checked;
                    self.status_message = format!("Space toggled checkbox: {}", self.checkbox_checked);
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if self.active_tab == ActiveTab::Controls && self.selected_list_item > 0 {
                    self.selected_list_item -= 1;
                    self.status_message = format!("Selected List Record #{}", self.selected_list_item);
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if self.active_tab == ActiveTab::Controls && self.selected_list_item < 4 {
                    self.selected_list_item += 1;
                    self.status_message = format!("Selected List Record #{}", self.selected_list_item);
                    self.rebuild_ui(ctx.app);
                    ctx.request_redraw();
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
    let smoke_secs: Option<u64> = std::env::args()
        .find_map(|a| a.strip_prefix("--smoke-test=").and_then(|v| v.parse().ok()));

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

    println!("Starting Hamesh Workbench on reusable hamesh-shell...");
    Shell::run(WorkbenchApp::new(), config)
}
