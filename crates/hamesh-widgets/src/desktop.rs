//! hamesh-widgets::desktop: reusable native desktop controls and interaction primitives.
//!
//! Provides the core components and states needed for real desktop tools:
//! - MenuBar with hierarchical items, accelerators, keyboard traversal (Alt/F10, arrows, Esc, Enter)
//! - CommandRegistry for semantic command routing and keyboard shortcuts
//! - FocusRing for deterministic Tab / Shift+Tab focus traversal across interactive controls
//! - Reusable controls: Button, Checkbox, TextInput, TabControl, SelectableList, Splitter, StatusBar
//! - Style-aware builders respecting the active `VisualWorld` (proportions, density, borders, radii)
//!
//! All controls emit semantic components and properties into the shared Hamesh component tree,
//! making them natively usable from both Rust and foreign language bridges (Python, Pepin).

use hamesh_core::{App, Component, Role};
use hamesh_theme::VisualWorld;
use serde::{Deserialize, Serialize};

/// A single menu item within a dropdown menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
    pub mnemonic: Option<char>,
    pub command: Option<String>,
    pub enabled: bool,
    pub checked: Option<bool>,
}

impl MenuItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            mnemonic: None,
            command: None,
            enabled: true,
            checked: None,
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn with_mnemonic(mut self, m: char) -> Self {
        self.mnemonic = Some(m);
        self
    }

    pub fn with_command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }
}

/// A top-level menu category in the menu bar (e.g. File, Edit, View, Help).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuCategory {
    pub name: String,
    pub mnemonic: Option<char>,
    pub items: Vec<MenuItem>,
}

impl MenuCategory {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mnemonic: None,
            items: Vec::new(),
        }
    }

    pub fn with_mnemonic(mut self, m: char) -> Self {
        self.mnemonic = Some(m);
        self
    }

    pub fn with_item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn with_items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items.extend(items);
        self
    }
}

/// Reusable desktop menu bar state and layout coordinator.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MenuBar {
    pub menus: Vec<MenuCategory>,
    pub open_idx: Option<usize>,
    pub selected_item_idx: usize,
    pub active_by_keyboard: bool,
}

impl MenuBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_menu(&mut self, menu: MenuCategory) {
        self.menus.push(menu);
    }

    pub fn open(&mut self, idx: usize) {
        if idx < self.menus.len() {
            self.open_idx = Some(idx);
            self.selected_item_idx = 0;
            self.active_by_keyboard = false;
        }
    }

    pub fn toggle(&mut self, idx: usize) {
        if self.open_idx == Some(idx) {
            self.close();
        } else {
            self.open(idx);
        }
    }

    pub fn close(&mut self) {
        self.open_idx = None;
        self.selected_item_idx = 0;
        self.active_by_keyboard = false;
    }

    pub fn is_open(&self) -> bool {
        self.open_idx.is_some()
    }

    pub fn current_menu(&self) -> Option<&MenuCategory> {
        self.open_idx.and_then(|i| self.menus.get(i))
    }

    pub fn select_next_menu(&mut self) {
        if self.menus.is_empty() {
            return;
        }
        let cur = self.open_idx.unwrap_or(0);
        let next = (cur + 1) % self.menus.len();
        self.open_idx = Some(next);
        self.selected_item_idx = 0;
    }

    pub fn select_prev_menu(&mut self) {
        if self.menus.is_empty() {
            return;
        }
        let cur = self.open_idx.unwrap_or(0);
        let prev = if cur == 0 {
            self.menus.len() - 1
        } else {
            cur - 1
        };
        self.open_idx = Some(prev);
        self.selected_item_idx = 0;
    }

    pub fn select_next_item(&mut self) {
        if let Some(menu) = self.current_menu() {
            if !menu.items.is_empty() {
                self.selected_item_idx = (self.selected_item_idx + 1) % menu.items.len();
            }
        }
    }

    pub fn select_prev_item(&mut self) {
        if let Some(menu) = self.current_menu() {
            if !menu.items.is_empty() {
                self.selected_item_idx = if self.selected_item_idx == 0 {
                    menu.items.len() - 1
                } else {
                    self.selected_item_idx - 1
                };
            }
        }
    }

    /// Invokes the currently selected menu item command and dismisses the menu bar.
    pub fn activate_current(&mut self) -> Option<String> {
        let cmd = self.current_menu().and_then(|menu| {
            menu.items
                .get(self.selected_item_idx)
                .filter(|it| it.enabled)
                .and_then(|it| it.command.clone().or_else(|| Some(it.id.clone())))
        });
        self.close();
        cmd
    }

    /// Activates a menu item matching a mnemonic character.
    pub fn activate_mnemonic(&mut self, ch: char) -> Option<String> {
        let ch_lower = ch.to_ascii_lowercase();
        if let Some(menu) = self.current_menu() {
            for (idx, item) in menu.items.iter().enumerate() {
                let m = item.mnemonic.or_else(|| item.label.chars().next());
                if let Some(m_char) = m {
                    if m_char.to_ascii_lowercase() == ch_lower && item.enabled {
                        self.selected_item_idx = idx;
                        return self.activate_current();
                    }
                }
            }
        }
        None
    }

    /// Builds the menu bar and any active dropdown overlay into the app component tree.
    pub fn build_components(&self, app: &mut App, parent_id: &str, world: VisualWorld) {
        let bar_h = match world {
            VisualWorld::ClassicDesktop => "26",
            VisualWorld::TechnicalWorkstation => "24",
            _ => "30",
        };
        let radius = format!("{:.0}", world.corner_radius_sm());
        let pad = format!("{:.0}", world.padding().min(4.0));
        let gap = format!("{:.0}", world.gap().min(4.0));

        app.add(
            parent_id,
            Component::new("menu_bar", Role::Header, "Menu Bar")
                .with_prop("layout", "row")
                .with_prop("h", bar_h)
                .with_prop(
                    "surface",
                    if world == VisualWorld::ClassicDesktop {
                        "surface"
                    } else {
                        "header"
                    },
                )
                .with_prop("border", "1")
                .with_prop("border_token", "border")
                .with_prop("pad", &pad)
                .with_prop("gap", &gap),
        );

        let mut cumulative_x = 4.0;
        let mut overlay_ox = 4.0;

        for (idx, menu) in self.menus.iter().enumerate() {
            let is_open = self.open_idx == Some(idx);
            let btn_id = format!("menu_bar_btn_{}", menu.name);
            let surface = if is_open { "accent" } else { "header" };
            let color = if is_open { "bg" } else { "text" };

            let label = if let Some(m) = menu.mnemonic {
                format!("{} ({})", menu.name, m.to_ascii_uppercase())
            } else {
                menu.name.clone()
            };

            if is_open {
                overlay_ox = cumulative_x;
            }

            let btn_w = 16.0 + (menu.name.len() as f32 * 9.0);
            cumulative_x += btn_w + 4.0;

            app.add(
                "menu_bar",
                Component::new(&btn_id, Role::Button, &menu.name)
                    .with_prop("layout", "row")
                    .with_prop("h", "22")
                    .with_prop("pad", "6")
                    .with_prop("surface", surface)
                    .with_prop("color", color)
                    .with_prop("radius", &radius)
                    .with_prop("text", &label)
                    .with_prop("action", &format!("menu_toggle:{}", idx)),
            );
        }

        // Active Dropdown Overlay
        if let Some(open_idx) = self.open_idx {
            if let Some(menu) = self.menus.get(open_idx) {
                let overlay_ox_str = format!("{:.0}", overlay_ox);
                let overlay_oy_str = bar_h;

                app.add(
                    parent_id,
                    Component::new("menu_dropdown_overlay", Role::Overlay, "Menu Dropdown")
                        .with_prop("layout", "column")
                        .with_prop("w", "260")
                        .with_prop("surface", "raised")
                        .with_prop("border", "1")
                        .with_prop("border_token", "border")
                        .with_prop("radius", &radius)
                        .with_prop("pad", "4")
                        .with_prop("gap", "2")
                        .with_prop("ox", &overlay_ox_str)
                        .with_prop("oy", overlay_oy_str)
                        .with_prop("z", "200"),
                );

                for (item_idx, item) in menu.items.iter().enumerate() {
                    let is_selected = self.selected_item_idx == item_idx;
                    let text_color = if !item.enabled {
                        "muted"
                    } else if is_selected {
                        "bg"
                    } else {
                        "text"
                    };
                    let item_surface = if is_selected && item.enabled {
                        "accent"
                    } else {
                        "raised"
                    };

                    let display_text = match (&item.shortcut, item.checked) {
                        (Some(sc), Some(true)) => format!("[x] {}   ({})", item.label, sc),
                        (Some(sc), Some(false)) => format!("[ ] {}   ({})", item.label, sc),
                        (Some(sc), None) => format!("{}   ({})", item.label, sc),
                        (None, Some(true)) => format!("[x] {}", item.label),
                        (None, Some(false)) => format!("[ ] {}", item.label),
                        (None, None) => item.label.clone(),
                    };

                    let cmd = item.command.clone().unwrap_or_else(|| item.id.clone());

                    app.add(
                        "menu_dropdown_overlay",
                        Component::new(&item.id, Role::Button, &item.label)
                            .with_prop("layout", "row")
                            .with_prop("h", "24")
                            .with_prop("pad", "6")
                            .with_prop("surface", item_surface)
                            .with_prop("color", text_color)
                            .with_prop("radius", &radius)
                            .with_prop("text", &display_text)
                            .with_prop("action", &format!("menu_exec:{}", cmd)),
                    );
                }
            }
        }
    }
}

/// A semantic desktop command definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
    pub category: String,
}

/// Central registry mapping actions, shortcuts, and labels.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommandRegistry {
    pub commands: Vec<Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        shortcut: Option<&str>,
        category: impl Into<String>,
    ) {
        self.commands.push(Command {
            id: id.into(),
            label: label.into(),
            shortcut: shortcut.map(str::to_string),
            category: category.into(),
        });
    }

    pub fn find_by_shortcut(&self, shortcut: &str) -> Option<&Command> {
        let sc_norm = shortcut.to_lowercase().replace(' ', "");
        self.commands.iter().find(|cmd| {
            cmd.shortcut
                .as_deref()
                .map(|s| s.to_lowercase().replace(' ', ""))
                .as_deref()
                == Some(&sc_norm)
        })
    }

    pub fn find_by_id(&self, id: &str) -> Option<&Command> {
        self.commands.iter().find(|c| c.id == id)
    }
}

/// Focus ring traversal coordinator for Tab / Shift+Tab keyboard cycling.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FocusRing {
    pub order: Vec<String>,
    pub current_idx: Option<usize>,
}

impl FocusRing {
    pub fn new(order: Vec<String>) -> Self {
        let current_idx = if order.is_empty() { None } else { Some(0) };
        Self { order, current_idx }
    }

    pub fn set_order(&mut self, order: Vec<String>) {
        self.order = order;
        if let Some(cur) = self.current_idx {
            if cur >= self.order.len() {
                self.current_idx = if self.order.is_empty() { None } else { Some(0) };
            }
        } else if !self.order.is_empty() {
            self.current_idx = Some(0);
        }
    }

    pub fn current(&self) -> Option<&str> {
        self.current_idx
            .and_then(|idx| self.order.get(idx).map(String::as_str))
    }

    pub fn set_focus(&mut self, id: &str) -> bool {
        if let Some(pos) = self.order.iter().position(|it| it == id) {
            self.current_idx = Some(pos);
            true
        } else {
            false
        }
    }

    pub fn tab(&mut self) -> Option<&str> {
        if self.order.is_empty() {
            return None;
        }
        let next = match self.current_idx {
            Some(i) => (i + 1) % self.order.len(),
            None => 0,
        };
        self.current_idx = Some(next);
        self.current()
    }

    pub fn shift_tab(&mut self) -> Option<&str> {
        if self.order.is_empty() {
            return None;
        }
        let prev = match self.current_idx {
            Some(i) => {
                if i == 0 {
                    self.order.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.order.len() - 1,
        };
        self.current_idx = Some(prev);
        self.current()
    }

    /// Rebuilds focus order automatically from an App component tree.
    pub fn rebuild_from_app(&mut self, app: &App) {
        let mut collected = Vec::new();
        Self::collect_nodes(app, &app.root.0, &mut collected);
        self.set_order(collected);
    }

    fn collect_nodes(app: &App, id: &str, out: &mut Vec<String>) {
        if let Some(c) = app.components.get(id) {
            if c.enabled
                && c.visible
                && matches!(c.role, Role::TextInput | Role::Button | Role::ListItem)
            {
                out.push(id.to_string());
            }
            for child in &c.children {
                Self::collect_nodes(app, &child.0, out);
            }
        }
    }
}

/// Reusable Checkbox desktop control.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckboxControl {
    pub id: String,
    pub label: String,
    pub checked: bool,
    pub disabled: bool,
}

impl CheckboxControl {
    pub fn new(id: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked,
            disabled: false,
        }
    }

    pub fn toggle(&mut self) {
        if !self.disabled {
            self.checked = !self.checked;
        }
    }

    pub fn build_component(
        &self,
        app: &mut App,
        parent_id: &str,
        is_focused: bool,
        world: VisualWorld,
    ) {
        let text_mark = if self.checked {
            format!("[X] {}", self.label)
        } else {
            format!("[ ] {}", self.label)
        };
        let h = format!("{:.0}", world.control_height().min(30.0));
        let radius = format!("{:.0}", world.corner_radius_sm());

        app.add(
            parent_id,
            Component::new(&self.id, Role::Button, &self.label)
                .with_prop("layout", "row")
                .with_prop("h", &h)
                .with_prop("gap", "8")
                .with_prop("pad", "6")
                .with_prop("surface", "surface")
                .with_prop("border", if is_focused { "2" } else { "0" })
                .with_prop("border_token", "focus")
                .with_prop("radius", &radius)
                .with_prop("text", &text_mark)
                .with_prop("action", &format!("toggle:{}", self.id)),
        );
    }
}

/// Reusable TabControl desktop control with tab switching and keyboard navigation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabControl {
    pub id: String,
    pub tabs: Vec<(String, String)>, // (id, label)
    pub active_idx: usize,
}

impl TabControl {
    pub fn new(id: impl Into<String>, tabs: Vec<(impl Into<String>, impl Into<String>)>) -> Self {
        Self {
            id: id.into(),
            tabs: tabs
                .into_iter()
                .map(|(a, b)| (a.into(), b.into()))
                .collect(),
            active_idx: 0,
        }
    }

    pub fn select(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_idx = idx;
        }
    }

    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_idx = (self.active_idx + 1) % self.tabs.len();
        }
    }

    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_idx = if self.active_idx == 0 {
                self.tabs.len() - 1
            } else {
                self.active_idx - 1
            };
        }
    }

    pub fn active_id(&self) -> Option<&str> {
        self.tabs.get(self.active_idx).map(|(id, _)| id.as_str())
    }

    pub fn build_components(&self, app: &mut App, parent_id: &str, world: VisualWorld) {
        let bar_h = match world {
            VisualWorld::ClassicDesktop => "28",
            VisualWorld::TechnicalWorkstation => "26",
            _ => "36",
        };
        let radius = format!("{:.0}", world.corner_radius_sm());

        app.add(
            parent_id,
            Component::new(&self.id, Role::Panel, "Tab Bar")
                .with_prop("layout", "row")
                .with_prop("h", bar_h)
                .with_prop("surface", "surface")
                .with_prop("pad", "4")
                .with_prop("gap", "6"),
        );

        for (idx, (tab_id, label)) in self.tabs.iter().enumerate() {
            let is_active = self.active_idx == idx;
            let tab_surface = if is_active { "window" } else { "surface" };
            let tab_color = if is_active { "accent" } else { "text_dim" };
            let border = if is_active { "1" } else { "0" };

            let btn_id = format!("{}_btn_{}", self.id, tab_id);

            app.add(
                &self.id,
                Component::new(&btn_id, Role::Button, label)
                    .with_prop("layout", "row")
                    .with_prop("h", "26")
                    .with_prop("pad", "8")
                    .with_prop("surface", tab_surface)
                    .with_prop("color", tab_color)
                    .with_prop("border", border)
                    .with_prop("border_token", "border")
                    .with_prop("radius", &radius)
                    .with_prop("text", label)
                    .with_prop("action", &format!("tab_select:{}:{}", self.id, idx)),
            );
        }
    }
}

/// Reusable SelectableList desktop control with vertical item navigation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectableListControl {
    pub id: String,
    pub items: Vec<String>,
    pub selected_idx: usize,
}

impl SelectableListControl {
    pub fn new(id: impl Into<String>, items: Vec<impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            items: items.into_iter().map(Into::into).collect(),
            selected_idx: 0,
        }
    }

    pub fn select(&mut self, idx: usize) {
        if idx < self.items.len() {
            self.selected_idx = idx;
        }
    }

    pub fn select_next(&mut self) {
        if !self.items.is_empty() && self.selected_idx + 1 < self.items.len() {
            self.selected_idx += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
        }
    }

    pub fn page_down(&mut self, count: usize) {
        if !self.items.is_empty() {
            self.selected_idx = (self.selected_idx + count).min(self.items.len() - 1);
        }
    }

    pub fn page_up(&mut self, count: usize) {
        self.selected_idx = self.selected_idx.saturating_sub(count);
    }

    pub fn home(&mut self) {
        self.selected_idx = 0;
    }

    pub fn end(&mut self) {
        if !self.items.is_empty() {
            self.selected_idx = self.items.len() - 1;
        }
    }

    pub fn selected_item(&self) -> Option<&str> {
        self.items.get(self.selected_idx).map(String::as_str)
    }

    pub fn build_components(
        &self,
        app: &mut App,
        parent_id: &str,
        focused_id: Option<&str>,
        world: VisualWorld,
    ) {
        let row_h = format!("{:.0}", world.row_height());
        let radius = format!("{:.0}", world.corner_radius_sm());

        app.add(
            parent_id,
            Component::new(&self.id, Role::List, "Selectable List")
                .with_prop("layout", "column")
                .with_prop("flex_grow", "1")
                .with_prop("gap", "4")
                .with_prop("pad", "4")
                .with_prop("surface", "raised"),
        );

        for (idx, item) in self.items.iter().enumerate() {
            let row_id = format!("{}_row_{}", self.id, idx);
            let is_sel = self.selected_idx == idx;
            let is_focused = focused_id == Some(&row_id);

            let row_surface = if is_sel { "accent" } else { "surface" };
            let row_color = if is_sel { "bg" } else { "text" };

            app.add(
                &self.id,
                Component::new(&row_id, Role::ListItem, item)
                    .with_prop("h", &row_h)
                    .with_prop("pad", "8")
                    .with_prop("surface", row_surface)
                    .with_prop("color", row_color)
                    .with_prop("border", if is_focused { "2" } else { "0" })
                    .with_prop("border_token", "focus")
                    .with_prop("radius", &radius)
                    .with_prop("text", item)
                    .with_prop("action", &format!("list_select:{}:{}", self.id, idx)),
            );
        }
    }
}

/// Reusable Splitter desktop control for resizable panels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplitterControl {
    pub id: String,
    pub position: f32,
    pub min_pos: f32,
    pub max_pos: f32,
    pub dragging: bool,
}

impl SplitterControl {
    pub fn new(id: impl Into<String>, position: f32, min_pos: f32, max_pos: f32) -> Self {
        Self {
            id: id.into(),
            position: position.clamp(min_pos, max_pos),
            min_pos,
            max_pos,
            dragging: false,
        }
    }

    pub fn drag_to(&mut self, pos: f32) {
        self.position = pos.clamp(self.min_pos, self.max_pos);
    }

    pub fn start_drag(&mut self) {
        self.dragging = true;
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
    }

    pub fn build_component(&self, app: &mut App, parent_id: &str, _world: VisualWorld) {
        app.add(
            parent_id,
            Component::new(&self.id, Role::Button, "Splitter Handle")
                .with_prop("w", "8")
                .with_prop("surface", if self.dragging { "accent" } else { "border" })
                .with_prop("cursor", "col-resize")
                .with_prop("action", &format!("splitter_drag:{}", self.id)),
        );
    }
}

/// Reusable StatusBar desktop control.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StatusBarControl {
    pub id: String,
    pub message: String,
    pub telemetry: Option<String>,
    pub profile_tag: Option<String>,
}

impl StatusBarControl {
    pub fn new(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            message: message.into(),
            telemetry: None,
            profile_tag: None,
        }
    }

    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
    }

    pub fn set_telemetry(&mut self, telem: impl Into<String>) {
        self.telemetry = Some(telem.into());
    }

    pub fn set_profile_tag(&mut self, tag: impl Into<String>) {
        self.profile_tag = Some(tag.into());
    }

    pub fn build_components(&self, app: &mut App, parent_id: &str, _world: VisualWorld) {
        app.add(
            parent_id,
            Component::new(&self.id, Role::Status, "Status Bar")
                .with_prop("layout", "row")
                .with_prop("h", "24")
                .with_prop("surface", "header")
                .with_prop("border", "1")
                .with_prop("border_token", "border")
                .with_prop("pad", "4")
                .with_prop("gap", "12"),
        );

        let msg_id = format!("{}_msg", self.id);
        app.add(
            &self.id,
            Component::new(&msg_id, Role::Text, "Status Message")
                .with_prop("flex_grow", "1")
                .with_prop("color", "text_dim")
                .with_prop("text", &self.message),
        );

        if let Some(telem) = &self.telemetry {
            let telem_id = format!("{}_telem", self.id);
            app.add(
                &self.id,
                Component::new(&telem_id, Role::Text, "Telemetry")
                    .with_prop("w", "180")
                    .with_prop("color", "muted")
                    .with_prop("text", telem),
            );
        }

        if let Some(tag) = &self.profile_tag {
            let tag_id = format!("{}_tag", self.id);
            app.add(
                &self.id,
                Component::new(&tag_id, Role::Text, "Profile Tag")
                    .with_prop("w", "220")
                    .with_prop("color", "accent")
                    .with_prop("text", tag),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_navigation_and_activation() {
        let mut bar = MenuBar::new();
        let file_menu = MenuCategory::new("File")
            .with_mnemonic('F')
            .with_item(MenuItem::new("file_new", "New").with_command("cmd_new"))
            .with_item(MenuItem::new("file_open", "Open").with_command("cmd_open"));
        let edit_menu = MenuCategory::new("Edit")
            .with_mnemonic('E')
            .with_item(MenuItem::new("edit_copy", "Copy").with_command("cmd_copy"));

        bar.add_menu(file_menu);
        bar.add_menu(edit_menu);

        assert!(!bar.is_open());
        bar.open(0);
        assert!(bar.is_open());
        assert_eq!(bar.current_menu().unwrap().name, "File");

        bar.select_next_item();
        assert_eq!(bar.selected_item_idx, 1);

        bar.select_next_menu();
        assert_eq!(bar.current_menu().unwrap().name, "Edit");
        assert_eq!(bar.selected_item_idx, 0);

        let activated = bar.activate_current();
        assert_eq!(activated, Some("cmd_copy".to_string()));
        assert!(!bar.is_open());
    }

    #[test]
    fn focus_ring_traversal() {
        let mut ring = FocusRing::new(vec![
            "btn_1".to_string(),
            "input_1".to_string(),
            "chk_1".to_string(),
        ]);
        assert_eq!(ring.current(), Some("btn_1"));
        assert_eq!(ring.tab(), Some("input_1"));
        assert_eq!(ring.tab(), Some("chk_1"));
        assert_eq!(ring.tab(), Some("btn_1")); // wrap
        assert_eq!(ring.shift_tab(), Some("chk_1")); // reverse wrap
        assert!(ring.set_focus("input_1"));
        assert_eq!(ring.current(), Some("input_1"));
    }

    #[test]
    fn command_registry_shortcuts() {
        let mut reg = CommandRegistry::new();
        reg.register("file_save", "Save Document", Some("Ctrl+S"), "File");
        reg.register("file_open", "Open File", Some("Ctrl+O"), "File");

        assert_eq!(
            reg.find_by_shortcut("ctrl + s").map(|c| c.id.as_str()),
            Some("file_save")
        );
        assert_eq!(
            reg.find_by_shortcut("CTRL+O").map(|c| c.id.as_str()),
            Some("file_open")
        );
        assert_eq!(reg.find_by_shortcut("Ctrl+Z"), None);
    }

    #[test]
    fn selectable_list_navigation() {
        let mut list = SelectableListControl::new("list_1", vec!["Item 0", "Item 1", "Item 2"]);
        assert_eq!(list.selected_idx, 0);
        list.select_next();
        assert_eq!(list.selected_idx, 1);
        list.select_next();
        assert_eq!(list.selected_idx, 2);
        list.select_next(); // clamped at end
        assert_eq!(list.selected_idx, 2);
        list.select_prev();
        assert_eq!(list.selected_idx, 1);
        list.home();
        assert_eq!(list.selected_idx, 0);
    }
}
