//! hamesh-tasks: a second independent consumer of the reusable kit.
//!
//! A development-tool composition (task queue + status filter + detail
//! inspector) built ONLY from Blocks composers over driver-owned widget
//! state. All pointer and keyboard routing goes through the shared
//! `hamesh_widgets::Hit` router — this driver owns no parent climbing
//! and no identity parsing. It is the genuine-reuse proof for 036: the
//! same tokens, controls and interaction contract as the gallery, in a
//! different application the kit never knew about.

use hamesh_blocks as b;
use hamesh_core::App;
use hamesh_widgets::{list_nav, picker_nav, Hit, Key, ListView, Picker};

pub const ROW_H: f32 = 32.0;
const FIELD_W: f32 = 240.0;
const ROOT_PAD: f32 = 12.0;
const FOOTER_H: f32 = 28.0;

/// One unit of work. Done-ness is the only mutation; everything else
/// (filtering, selection, reveal, progress) derives from it.
#[derive(Clone, Debug)]
pub struct Task {
    pub label: String,
    pub done: bool,
}

/// Keyboard intents are the shared `hamesh_widgets::Key` vocabulary:
/// no per-app key enum, no second mapping table.
///
/// Action commands in focus order. Tab walks them; Enter runs the
/// focused one if it is enabled.
pub const ACTIONS: [&str; 2] = ["toggle-done", "clear-filter"];

#[derive(Clone)]
pub struct Tasks {
    pub tasks: Vec<Task>,
    pub list: ListView,
    pub filter: Picker,
    /// 0 = list, 1 = filter, 2 = actions.
    pub focus: usize,
    /// Focused action inside zone 2 (index into `ACTIONS`).
    pub action_idx: usize,
    /// Selected TASK (item identity, never a row). Rows shift under
    /// filtering; this does not. The list cursor is re-derived from
    /// it after every query change via `filtered_row`.
    pub sel: Option<usize>,
    hover_action: Option<String>,
}

impl Tasks {
    pub fn new() -> Self {
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let labels = [
            "Wire the Hit router",
            "Paint button states",
            "Prove the sheet",
            "Review contrast pairs",
            "Hoist focus traversal",
            "Second composition",
            "Native gallery run",
            "Write the final report",
        ];
        let tasks = labels
            .iter()
            .map(|l| Task {
                label: l.to_string(),
                done: false,
            })
            .collect::<Vec<_>>();
        let items = labels.iter().map(|l| l.to_string()).collect();
        let mut list = ListView::new(ROW_H);
        list.set_count(tasks.len());
        assert!(list.select(0));
        Self {
            tasks,
            list,
            filter: Picker::new(items),
            focus: 0,
            action_idx: 0,
            sel: Some(0),
            hover_action: None,
        }
    }

    /// Tasks matching the filter query, as indices into `tasks`.
    /// Public: views (like the lab drill) render from the same window.
    pub fn shown(&self) -> Vec<usize> {
        self.filter.filtered()
    }

    /// Selected task: the stored item identity. Public: views read the
    /// same selection the driver acts on, through every filter change.
    pub fn selected_task(&self) -> Option<usize> {
        self.sel
    }

    /// Focused zone name for the Eye: list, filter or actions.
    pub fn focused_zone(&self) -> &str {
        ["list", "filter", "actions"][self.focus]
    }

    /// Re-derive the list cursor from the stored item after the query
    /// changes. A filtered-out item keeps its identity with no row.
    fn resync_selection(&mut self) {
        let shown = self.shown();
        // The model follows the same filter: a stale count would clamp
        // the cursor before the item mapping runs.
        self.list.set_count(shown.len());
        match self
            .sel
            .and_then(|i| hamesh_widgets::filtered_row(&shown, i))
        {
            Some(row) => {
                self.list.select(row);
            }
            None => {
                self.list.clear_selection();
            }
        }
    }

    pub fn done_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.done).count()
    }

    /// Whether an action can run right now. Toggle needs a selected
    /// task; Clear only means something while a filter query is active
    /// (open popup with a non-empty query). Disabled actions paint
    /// muted and refuse to run — through every input path, pointer
    /// and keyboard alike.
    pub fn action_enabled(&self, command: &str) -> bool {
        match command {
            "toggle-done" => self.selected_task().is_some(),
            "clear-filter" => self.filter.is_open() && !self.filter.query().is_empty(),
            _ => false,
        }
    }

    fn rows(&self) -> Vec<b::select::ListRow> {
        self.shown()
            .iter()
            .map(|&i| {
                let t = &self.tasks[i];
                let icon = if t.done { "list-checks" } else { "activity" };
                b::select::ListRow::new(&t.label).with_icon(icon)
            })
            .collect()
    }

    fn narrow(width: f32) -> bool {
        width < 1000.0
    }
}

impl Default for Tasks {
    fn default() -> Self {
        Self::new()
    }
}

fn layout_of(app: &App, w: f32, h: f32) -> hamesh_layout::LayoutOutput {
    hamesh_layout::compute(app, w, h)
}

fn compose_with(t: &mut Tasks, width: f32, height: f32, list_h: f32) -> App {
    t.list.set_focused(t.focus == 0);
    t.filter.set_focused(t.focus == 1);
    let rows = t.rows();
    t.list.set_count(rows.len());
    let selected_label = t
        .selected_task()
        .and_then(|i| t.tasks.get(i))
        .map(|t| t.label.clone())
        .unwrap_or_default();
    let selected_done = t
        .selected_task()
        .and_then(|i| t.tasks.get(i))
        .map(|t| t.done)
        .unwrap_or(false);
    let done = t.done_count();
    let total = t.tasks.len();
    let hover_toggle = t.hover_action.as_deref() == Some("toggle-done");
    let hover_clear = t.hover_action.as_deref() == Some("clear-filter");
    // Per-button paint: disabled wins over hover, and only the focused
    // button in the focused zone carries the ring.
    let toggle_state = if !t.action_enabled("toggle-done") {
        b::ButtonState::Disabled
    } else if hover_toggle {
        b::ButtonState::Hover
    } else {
        b::ButtonState::Rest
    };
    let clear_state = if !t.action_enabled("clear-filter") {
        b::ButtonState::Disabled
    } else if hover_clear {
        b::ButtonState::Hover
    } else {
        b::ButtonState::Rest
    };
    let toggle_focused = t.focus == 2 && t.action_idx == 0;
    let clear_focused = t.focus == 2 && t.action_idx == 1;
    let mut app = App::new("taskapp");
    {
        let mut root = b::Scope::root(&mut app, "taskapp");
        b::row(
            &mut root,
            "header",
            b::LayoutOpts {
                pad: 12.0,
                ..Default::default()
            },
            |s| {
                b::title(s, "app-title", "Tasks");
                b::spacer(s, "header-space");
                b::col(
                    s,
                    "filter-col",
                    b::LayoutOpts {
                        w: Some(FIELD_W),
                        pad: 0.0,
                        gap: 4.0,
                        ..Default::default()
                    },
                    |u| {
                        b::select::picker(
                            u,
                            "filter",
                            b::select::PickerSpec {
                                picker: &mut t.filter,
                                field_h: 36.0,
                                row_h: ROW_H,
                                max_rows: 8,
                                placeholder: "Filter tasks…".to_string(),
                            },
                        );
                    },
                );
                b::button(
                    s,
                    "btn-toggle",
                    "Toggle done",
                    b::ButtonOpts::new(b::ButtonVariant::Primary, "toggle-done")
                        .with_state(toggle_state)
                        .focused(toggle_focused),
                );
                b::button(
                    s,
                    "btn-clear",
                    "Clear",
                    b::ButtonOpts::new(b::ButtonVariant::Ghost, "clear-filter")
                        .with_state(clear_state)
                        .focused(clear_focused),
                );
            },
        );
        b::row(
            &mut root,
            "body",
            b::LayoutOpts {
                grow: Some(1.0),
                pad: ROOT_PAD,
                gap: ROOT_PAD,
                ..Default::default()
            },
            |s| {
                b::select::list(
                    s,
                    "tasks",
                    b::select::ListSpec {
                        rows: &rows,
                        view: &mut t.list,
                        viewport_h: list_h,
                        row_h: ROW_H,
                        empty_label: "No tasks match".to_string(),
                        grow: None,
                        viewport_w: None,
                    },
                );
                if !Tasks::narrow(width) {
                    b::col(
                        s,
                        "side",
                        b::LayoutOpts {
                            w: Some(248.0),
                            pad: ROOT_PAD,
                            gap: 8.0,
                            grow: Some(0.0),
                            ..Default::default()
                        },
                        |u| {
                            b::caption(u, "side-title", "DETAIL");
                            b::body(u, "sel-label", &selected_label);
                            b::badge(
                                u,
                                "sel-status",
                                if selected_done { "Done" } else { "Open" },
                                if selected_done {
                                    b::Variant::Success
                                } else {
                                    b::Variant::Neutral
                                },
                            );
                            b::progress(
                                u,
                                "done-bar",
                                b::ProgressOpts::new(done as f32 / total.max(1) as f32 * 100.0),
                            );
                            b::caption(u, "done-count", &format!("{done}/{total} done"));
                        },
                    );
                }
            },
        );
        b::row(
            &mut root,
            "footer",
            b::LayoutOpts {
                h: Some(FOOTER_H),
                pad: 8.0,
                ..Default::default()
            },
            |s| {
                b::caption(s, "foot-status", "Tasks · HAMESH 036");
                b::spacer(s, "foot-space");
                b::caption(s, "foot-size", &format!("{width:.0}×{height:.0}"));
            },
        );
    }
    app
}

/// Two-pass driver budgeting, mirroring the gallery contract: pass 1 on
/// a throwaway clone measures the real body; pass 2 grants the list
/// exactly what remains. The header never depends on the list height.
pub fn refresh(t: &mut Tasks, w: f32, h: f32) -> (App, hamesh_layout::LayoutOutput) {
    // Selection names an item, rows are derived: reconcile the cursor
    // with the current filter on every frame, whatever changed it.
    t.resync_selection();
    let mut probe = t.clone();
    let app1 = compose_with(&mut probe, w, h, 96.0);
    let lay1 = layout_of(&app1, w, h);
    let body = lay1.nodes.get("body").map(|n| n.rect.h).unwrap_or(600.0);
    let list_h = ((body - ROOT_PAD * 2.0) / ROW_H).floor() * ROW_H;
    let list_h = list_h.max(96.0);
    if t.filter.is_open() {
        let full = b::select::popup_height(t.filter.filtered_len(), ROW_H, 8);
        if let Some(field) = lay1.nodes.get("filter-field") {
            t.filter
                .set_viewport(b::select::popup_geom(&field.rect, w, h, full, field.rect.w).3);
        }
    }
    let mut app = compose_with(t, w, h, list_h);
    let layout = layout_of(&app, w, h);
    if t.filter.is_open() {
        let metrics = b::select::PopupMetrics {
            row_h: ROW_H,
            max_rows: 8,
            matches: t.filter.filtered_len(),
            scroll: t.filter.scroll(),
        };
        if let Some((_, _, _, placed_h)) =
            b::select::place_popup(&mut app, &layout, "filter", &metrics, w, h)
        {
            t.filter.set_viewport(placed_h);
        }
    }
    let layout = layout_of(&app, w, h);
    (app, layout)
}

fn toggle_selected(t: &mut Tasks) -> String {
    match t.selected_task() {
        Some(i) => {
            t.tasks[i].done = !t.tasks[i].done;
            format!(
                "task {i} {}",
                if t.tasks[i].done { "done" } else { "reopened" }
            )
        }
        None => "nothing selected".to_string(),
    }
}

/// Run an action command. Toggle acts on the selection and dismisses
/// an open popup (the decision is taken); Clear erases the query but
/// keeps the popup open for refinement (distinct from Escape, which
/// cancels). Disabled actions refuse with a note instead of running.
/// Pointer and keyboard share this path.
fn act(t: &mut Tasks, command: &str) -> String {
    if !t.action_enabled(command) {
        return format!("{command} disabled: no action");
    }
    match command {
        "toggle-done" => {
            let note = toggle_selected(t);
            if t.filter.is_open() {
                t.filter.close();
                format!("{note} (popup dismissed)")
            } else {
                note
            }
        }
        "clear-filter" => {
            if t.filter.clear_query() {
                "filter cleared".to_string()
            } else {
                "filter already empty".to_string()
            }
        }
        other => format!("unknown command {other}"),
    }
}

/// Pointer press through the SHARED router: hit-test, resolve with
/// `Hit::of`, drive the widget. No per-app climbing or id parsing.
pub fn press(t: &mut Tasks, app: &App, scene: &hamesh_scene::Scene, x: f32, y: f32) -> String {
    match hamesh_scene::hit_test(app, scene, x, y) {
        None => {
            if t.filter.is_open() {
                t.filter.close();
                "press hit nothing (popup dismissed)".to_string()
            } else {
                "press hit nothing".to_string()
            }
        }
        Some(id) => match Hit::of(app, &id) {
            Hit::ListRow { row, .. } => {
                t.focus = 0;
                // The row names an item; the item is what we keep.
                t.sel = t.shown().get(row).copied();
                let note = if t.list.select(row) {
                    format!("selected task row {row}")
                } else {
                    format!("task row {row} already selected")
                };
                if t.filter.is_open() {
                    t.filter.close();
                    format!("{note} (popup dismissed)")
                } else {
                    note
                }
            }
            Hit::PickOption { option, .. } => {
                t.focus = 1;
                // Options filter the queue; the list follows the query.
                // Highlight-then-commit keeps pointer and keyboard equal.
                if let Some(r) = scene
                    .nodes
                    .iter()
                    .find(|n| n.id == "filter-popup")
                    .map(|n| n.rect.clone())
                {
                    t.filter.highlight_at_viewport(y - r.y);
                }
                // Committing a filter option narrows the queue without
                // closing: the list IS the result. Keep the popup open
                // so typing can refine further; Escape or outside press
                // dismisses.
                let _ = option;
                "filter narrows the queue".to_string()
            }
            Hit::PickerField { .. } => {
                t.focus = 1;
                if t.filter.toggle() {
                    "filter opened".to_string()
                } else {
                    "filter closed".to_string()
                }
            }
            Hit::Action { command } => {
                t.focus = 2;
                act(t, &command)
            }
            Hit::Chrome(hit) => {
                if t.filter.is_open() {
                    t.filter.close();
                    format!("hit {hit} (chrome, popup dismissed)")
                } else {
                    format!("hit {hit} (chrome, no action)")
                }
            }
        },
    }
}

pub fn hover(t: &mut Tasks, scene: &hamesh_scene::Scene, x: f32, y: f32, app: &App) -> bool {
    let Some(id) = hamesh_scene::hit_test(app, scene, x, y) else {
        t.hover_action = None;
        return false;
    };
    if let Hit::Action { command } = Hit::of(app, &id) {
        t.hover_action = Some(command);
        return true;
    }
    // List rows paint hover from widget state, like the gallery.
    if let Hit::ListRow { .. } = Hit::of(app, &id) {
        if let Some(r) = scene
            .nodes
            .iter()
            .find(|n| n.id == "tasks-view")
            .map(|n| n.rect.clone())
        {
            return t.list.hover_at_viewport(y - r.y);
        }
    }
    if let Hit::PickOption { .. } = Hit::of(app, &id) {
        if let Some(r) = scene
            .nodes
            .iter()
            .find(|n| n.id == "filter-popup")
            .map(|n| n.rect.clone())
        {
            t.filter.highlight_at_viewport(y - r.y);
            return true;
        }
    }
    t.hover_action = None;
    false
}

pub fn wheel(t: &mut Tasks, layout: &hamesh_layout::LayoutOutput, x: f32, y: f32, dy: f32) -> bool {
    let over = |id: &str| layout.nodes.get(id).is_some_and(|n| n.rect.contains(x, y));
    if over("filter-popup") {
        t.filter.wheel(dy)
    } else if over("tasks-view") {
        t.list.wheel(dy)
    } else {
        false
    }
}

pub fn key(t: &mut Tasks, k: Key) -> String {
    if matches!(k, Key::Tab | Key::ShiftTab) {
        // Tab moves focus and never discards a live query: the popup
        // stays open while the query is non-empty so Clear remains
        // reachable by keyboard. An empty query still dismisses.
        // Inside the actions zone Tab walks the buttons themselves.
        if t.filter.query().is_empty() {
            t.filter.close();
        }
        let fwd = matches!(k, Key::Tab);
        (t.focus, t.action_idx) = match (t.focus, fwd) {
            (2, true) if t.action_idx + 1 < ACTIONS.len() => (2, t.action_idx + 1),
            (0, false) => (2, ACTIONS.len() - 1),
            (2, false) if t.action_idx > 0 => (2, t.action_idx - 1),
            (f, true) => ((f + 1) % 3, 0),
            (f, false) => ((f + 2) % 3, 0),
        };
        return format!("focus -> {}", ["list", "filter", "actions"][t.focus]);
    }
    if t.focus == 0 {
        if let Some(moved) = list_nav(&mut t.list, &k) {
            // The cursor moved rows: re-anchor the item identity.
            t.sel = t.list.selected().and_then(|r| t.shown().get(r).copied());
            return format!("tasks nav: {moved}");
        }
        match k {
            Key::Enter => toggle_selected(t),
            Key::Escape => {
                if t.filter.is_open() {
                    t.filter.close();
                    "filter dismissed".to_string()
                } else {
                    "list: no popup to dismiss".to_string()
                }
            }
            _ => "list: key ignored".to_string(),
        }
    } else if t.focus == 1 {
        if let Some(moved) = picker_nav(&mut t.filter, &k) {
            return format!("filter nav: {moved}");
        }
        match k {
            Key::Char(c) => {
                if !t.filter.is_open() {
                    t.filter.open();
                }
                let typed = t.filter.push_char(c);
                t.resync_selection();
                format!("filter type: {typed}")
            }
            Key::Backspace => {
                let erased = t.filter.backspace();
                t.resync_selection();
                format!("filter erase: {erased}")
            }
            Key::Enter => "filter: list follows the query".to_string(),
            Key::Escape => format!("filter dismissed: {}", t.filter.close()),
            _ => unreachable!("navigation and Tab handled above"),
        }
    } else {
        match k {
            // Enter runs the focused button through the same gate as a
            // pointer press: disabled means disabled, everywhere.
            Key::Enter => act(t, ACTIONS[t.action_idx]),
            Key::Escape => {
                if t.filter.is_open() {
                    t.filter.close();
                    "filter dismissed".to_string()
                } else {
                    "actions: nothing to dismiss".to_string()
                }
            }
            _ => "actions: pointer or Enter".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        t: &mut Tasks,
        w: f32,
        h: f32,
    ) -> (App, hamesh_layout::LayoutOutput, hamesh_scene::Scene) {
        let (app, layout) = refresh(t, w, h);
        let scene = hamesh_scene::build(&app, &layout, 1);
        (app, layout, scene)
    }

    fn center_of(scene: &hamesh_scene::Scene, id: &str) -> (f32, f32) {
        let n = scene.nodes.iter().find(|n| n.id == id).unwrap();
        (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0)
    }

    #[test]
    fn routing_comes_from_the_shared_hit_router() {
        // No per-app climbing: the generic Hit resolves every node the
        // sheet composes, including nested label leaves.
        let mut t = Tasks::new();
        let (app, _layout, _scene) = frame(&mut t, 1440.0, 900.0);
        assert!(matches!(
            Hit::of(&app, "tasks-row-2-label"),
            Hit::ListRow { row: 2, .. }
        ));
        assert!(matches!(Hit::of(&app, "btn-toggle"), Hit::Action { .. }));
        assert!(matches!(
            Hit::of(&app, "filter-field"),
            Hit::PickerField { .. }
        ));
    }

    #[test]
    fn toggle_done_flips_badge_and_progress() {
        let mut t = Tasks::new();
        assert_eq!(t.done_count(), 0);
        // Selection moves through intents, never direct widget pokes.
        let _ = key(&mut t, Key::Down);
        assert_eq!(t.sel, Some(1));
        assert_eq!(toggle_selected(&mut t), "task 1 done");
        assert!(t.tasks[1].done);
        assert_eq!(t.done_count(), 1);
        let (app, _layout, _scene) = frame(&mut t, 1440.0, 900.0);
        let badge = app.get("sel-status").unwrap();
        assert_eq!(badge.props.get("text").map(|s| s.as_str()), Some("Done"));
        let bar = app.get("done-bar").unwrap();
        assert_eq!(bar.props.get("value").unwrap(), "12.5");
    }

    #[test]
    fn filter_narrows_the_queue() {
        let mut t = Tasks::new();
        t.focus = 1;
        t.filter.open();
        for c in "router".chars() {
            t.filter.push_char(c);
        }
        assert_eq!(t.shown(), vec![0]);
        let (app, _layout, _scene) = frame(&mut t, 1440.0, 900.0);
        assert!(
            app.get("tasks-row-1").is_none(),
            "unmatched rows leave the tree"
        );
        assert!(app.get("tasks-row-0").is_some());
    }

    #[test]
    fn outside_press_and_tab_dismiss_without_losing_state() {
        let mut t = Tasks::new();
        t.focus = 1;
        t.filter.open();
        let (app, _layout, scene) = frame(&mut t, 1440.0, 900.0);
        let (cx, cy) = center_of(&scene, "tasks-row-3");
        let note = press(&mut t, &app, &scene, cx, cy);
        assert!(!t.filter.is_open(), "{note}");
        assert!(note.contains("dismissed"), "{note}");
        assert_eq!(t.list.selected(), Some(3));
        // Reopen, then Tab away: same cancel contract as the gallery.
        t.filter.open();
        let _ = key(&mut t, Key::Tab);
        assert!(!t.filter.is_open());
        // The row press left focus on the list, so Tab lands on filter.
        assert_eq!(t.focus, 1);
    }

    #[test]
    fn selection_names_the_item_not_the_row() {
        // Identity survives the complete filter cycle: select, narrow
        // past the item, dismiss, and the same task is still selected.
        let mut t = Tasks::new();
        let (app, _layout, scene) = frame(&mut t, 1440.0, 900.0);
        let (cx, cy) = center_of(&scene, "tasks-row-2");
        let _ = press(&mut t, &app, &scene, cx, cy);
        assert_eq!(t.sel, Some(2));
        // Narrow to a query that excludes task 2: the row clears,
        // the stored item does not move.
        t.focus = 1;
        for c in "wire".chars() {
            let _ = key(&mut t, Key::Char(c));
        }
        let (app2, _layout2, _scene2) = frame(&mut t, 1440.0, 900.0);
        assert_eq!(t.shown(), vec![0]);
        assert_eq!(t.sel, Some(2));
        assert_eq!(t.list.selected(), None, "no row for a hidden item");
        assert!(app2.get("tasks-row-2").is_none());
        // Dismissing restores the full queue with the same task row.
        let _ = key(&mut t, Key::Escape);
        let (app3, _layout3, _scene3) = frame(&mut t, 1440.0, 900.0);
        assert!(!t.filter.is_open());
        assert_eq!(t.selected_task(), Some(2));
        assert_eq!(t.list.selected(), Some(2));
        assert!(app3.get("tasks-row-2").is_some());
        // Actions follow the item, not the row it happened to be on.
        t.focus = 0;
        let note = key(&mut t, Key::Enter);
        assert!(note.contains("task 2 done"), "{note}");
        assert!(t.tasks[2].done);
    }

    #[test]
    fn focused_zone_names_the_zone() {
        let mut t = Tasks::new();
        assert_eq!(t.focused_zone(), "list");
        t.focus = 1;
        assert_eq!(t.focused_zone(), "filter");
        t.focus = 2;
        assert_eq!(t.focused_zone(), "actions");
    }

    #[test]
    fn tab_walks_zones_then_buttons_then_wraps() {
        let mut t = Tasks::new();
        assert_eq!((t.focus, t.action_idx), (0, 0));
        let _ = key(&mut t, Key::Tab);
        assert_eq!((t.focus, t.action_idx), (1, 0));
        let _ = key(&mut t, Key::Tab);
        assert_eq!((t.focus, t.action_idx), (2, 0));
        let _ = key(&mut t, Key::Tab);
        assert_eq!((t.focus, t.action_idx), (2, 1), "second button");
        let _ = key(&mut t, Key::Tab);
        assert_eq!((t.focus, t.action_idx), (0, 0), "wraps to the list");
        // Shift+Tab walks the same path backward.
        let _ = key(&mut t, Key::ShiftTab);
        assert_eq!((t.focus, t.action_idx), (2, 1));
        let _ = key(&mut t, Key::ShiftTab);
        assert_eq!((t.focus, t.action_idx), (2, 0));
        let _ = key(&mut t, Key::ShiftTab);
        assert_eq!((t.focus, t.action_idx), (1, 0));
    }

    #[test]
    fn focused_button_carries_the_ring() {
        let mut t = Tasks::new();
        t.focus = 2;
        t.action_idx = 1;
        let (app, _layout, _scene) = frame(&mut t, 1440.0, 900.0);
        let clear = app.get("btn-clear").unwrap();
        assert_eq!(
            clear.props.get("border_token").map(|s| s.as_str()),
            Some("focus")
        );
        assert_eq!(clear.props.get("border").map(|s| s.as_str()), Some("2"));
        let toggle = app.get("btn-toggle").unwrap();
        assert_eq!(
            toggle.props.get("border_token").map(|s| s.as_str()),
            Some("border")
        );
    }

    #[test]
    fn enter_runs_the_focused_action_and_disabled_refuses() {
        let mut t = Tasks::new();
        // Clear is disabled with a closed filter: Enter reports, runs nothing.
        t.focus = 2;
        t.action_idx = 1;
        assert!(!t.action_enabled("clear-filter"));
        let note = key(&mut t, Key::Enter);
        assert!(note.contains("disabled"), "{note}");
        // Open the filter and type: a non-empty query enables Clear.
        // Tab preserves the query so the buttons stay reachable.
        t.filter.open();
        t.focus = 1;
        let _ = key(&mut t, Key::Char('o'));
        assert!(t.action_enabled("clear-filter"));
        t.focus = 2;
        t.action_idx = 1;
        let note = key(&mut t, Key::Enter);
        assert!(note.contains("filter cleared"), "{note}");
        // Clear refines: the query is gone but the popup stays open.
        assert!(t.filter.is_open());
        assert!(t.filter.query().is_empty());
        assert_eq!(t.shown().len(), 8);
        // Escape still cancels out of it.
        t.focus = 1;
        let _ = key(&mut t, Key::Escape);
        assert!(!t.filter.is_open());
        // Pointer on a disabled button is equally refused.
        let (app, _layout, scene) = frame(&mut t, 1440.0, 900.0);
        let (cx, cy) = center_of(&scene, "btn-clear");
        let note = press(&mut t, &app, &scene, cx, cy);
        assert!(note.contains("disabled"), "{note}");
    }

    #[test]
    fn clear_is_reachable_by_keyboard_alone() {
        // End-to-end through normal Key events only: type a query,
        // Tab to Clear, activate. No direct focus or filter pokes.
        let mut t = Tasks::new();
        assert_eq!((t.focus, t.action_idx), (0, 0));
        assert_eq!(key(&mut t, Key::Tab), "focus -> filter");
        assert_eq!(t.focus, 1);
        let note = key(&mut t, Key::Char('o'));
        assert!(note.contains("filter type"), "{note}");
        assert!(t.filter.is_open());
        assert!(!t.filter.query().is_empty());
        assert!(t.shown().len() < 8);
        assert!(t.action_enabled("clear-filter"));
        // Tab into the actions zone: the query survives, Clear stays enabled.
        assert_eq!(key(&mut t, Key::Tab), "focus -> actions");
        assert_eq!((t.focus, t.action_idx), (2, 0));
        assert!(t.filter.is_open(), "Tab must not discard the query");
        assert!(t.action_enabled("clear-filter"));
        assert_eq!(key(&mut t, Key::Tab), "focus -> actions");
        assert_eq!((t.focus, t.action_idx), (2, 1), "Clear button");
        assert!(t.action_enabled("clear-filter"));
        let note = key(&mut t, Key::Enter);
        assert!(note.contains("filter cleared"), "{note}");
        assert!(t.filter.query().is_empty());
        assert_eq!(t.shown().len(), 8, "full list returns");
        assert_eq!((t.focus, t.action_idx), (2, 1), "focus stays on Clear");
        // The focused button carries the ring.
        let (app, _layout, _scene) = frame(&mut t, 1440.0, 900.0);
        let clear = app.get("btn-clear").unwrap();
        assert_eq!(
            clear.props.get("border_token").map(|s| s.as_str()),
            Some("focus")
        );
        assert_eq!(clear.props.get("border").map(|s| s.as_str()), Some("2"));
    }

    #[test]
    fn action_buttons_drive_real_commands() {
        let mut t = Tasks::new();
        let (app, _layout, scene) = frame(&mut t, 1440.0, 900.0);
        let (cx, cy) = center_of(&scene, "btn-toggle");
        let note = press(&mut t, &app, &scene, cx, cy);
        assert!(note.contains("done"), "{note}");
        assert!(t.tasks[0].done, "row 0 was selected: {note}");
        assert_eq!(t.focus, 2);
        // An action press runs first: clear refines without dismissing.
        t.filter.open();
        t.focus = 1;
        let _ = key(&mut t, Key::Char('o'));
        let (app2, _layout2, scene2) = frame(&mut t, 1440.0, 900.0);
        let (dx, dy) = center_of(&scene2, "btn-clear");
        let note = press(&mut t, &app2, &scene2, dx, dy);
        assert!(note.contains("filter cleared"), "{note}");
        assert!(t.filter.is_open(), "clear refines, Escape cancels");
        assert!(t.filter.query().is_empty());
    }
}
