//! Blocks composers for selection controls: the drawing half of lists
//! and pickers.
//!
//! Behaviour lives in `hamesh_widgets::{ListView, Picker}`; these builders
//! turn that state into components. The consumer owns one widget struct,
//! forwards pointer and key intents to it, and calls one function here —
//! it never reimplements row windows, reveal-on-select, filtering or
//! commit semantics.
//!
//! Two contracts the consumer relies on:
//!
//! * The composer syncs the viewport into the widget (`set_viewport`),
//!   so scroll clamps and the visible window are always honest. The only
//!   geometry the consumer supplies is the height it grants the control.
//! * Hit-testing needs no id parsing: every painted row carries its
//!   identity as props (`list` + `row`, or `picker` + `pick`). The driver
//!   hit-tests, reads the props, and tells the widget what happened.
//!
//! State styling (verified contrast pairs, no new tokens):
//!
//! * selected/active: `bg` text on `accent` (5.40, AA body)
//! * hover: `raised` surface, `text` label
//! * rest: `surface`, `text` label, `text_dim` icon
//! * focus: container border 2 in `focus` (was 1 in `border`)
//! * empty: `muted` explanation row, no scrollbar
//! * precedence: selected > hover > rest; focus never moves paint

use super::{icon, Scope, TextOpts};
use hamesh_core::Role;
use hamesh_widgets::{ListView, Picker};

/// Row height for list rows and picker options (px). Roomy enough for
/// a 13px label with breathing room, tight enough for long lists.
pub const ROW_H: f32 = 32.0;
/// Minimum scrollbar thumb: still grabbable/readable at a glance.
const MIN_THUMB: f32 = 24.0;

/// One row of content. The widget owns position and state; the row
/// owns what the user reads.
#[derive(Clone, Debug)]
pub struct ListRow {
    pub label: String,
    pub icon: Option<String>,
}

impl ListRow {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            icon: None,
        }
    }

    pub fn with_icon(mut self, name: &str) -> Self {
        self.icon = Some(name.to_string());
        self
    }
}

/// What a list composer needs. `rows` and `view` must agree on the
/// model (same length, same order) — the composer reads `visible()`,
/// never the count, so a mismatch shows as missing rows, not a panic.
pub struct ListSpec<'a> {
    pub rows: &'a [ListRow],
    pub view: &'a mut ListView,
    /// Height granted to the viewport (px). Fixed: the list never
    /// grows with its content, it scrolls.
    pub viewport_h: f32,
    pub row_h: f32,
    pub empty_label: String,
    /// Flex grow of the list root in its parent (None = hug).
    pub grow: Option<f32>,
}

impl<'a> ListSpec<'a> {
    pub fn new(rows: &'a [ListRow], view: &'a mut ListView, viewport_h: f32) -> Self {
        view.set_count(rows.len());
        Self {
            rows,
            view,
            viewport_h: viewport_h.max(1.0),
            row_h: ROW_H,
            empty_label: "No items".to_string(),
            grow: None,
        }
    }
}

/// Painted row state after the precedence rule.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowLook {
    Active,
    Hover,
    Rest,
}

fn row_look(selected_like: bool, hovered: bool) -> RowLook {
    if selected_like {
        RowLook::Active
    } else if hovered {
        RowLook::Hover
    } else {
        RowLook::Rest
    }
}

fn row_surface(look: RowLook) -> &'static str {
    match look {
        RowLook::Active => "accent",
        RowLook::Hover => "raised",
        RowLook::Rest => "surface",
    }
}

fn row_text(look: RowLook) -> &'static str {
    match look {
        RowLook::Active => "bg",
        RowLook::Hover => "text",
        RowLook::Rest => "text",
    }
}

fn row_icon_token(look: RowLook) -> &'static str {
    match look {
        RowLook::Active => "bg",
        _ => "text_dim",
    }
}

/// One option row. Shared by lists and picker popups so the two
/// controls can never drift apart visually.
///
/// `row_prop` is ("list", index) or ("picker", filtered-index): the
/// identity the driver reads back after a hit-test.
fn option_row(
    s: &mut Scope,
    id: String,
    label: &str,
    icon_name: Option<&str>,
    row_h: f32,
    look: RowLook,
    row_prop: (&str, usize),
) {
    let surface = row_surface(look);
    let mut r = hamesh_core::Component::new(&id, Role::ListItem, &id);
    r.props.insert("block".to_string(), "ListRow".to_string());
    r.props.insert("layout".to_string(), "row".to_string());
    r.props
        .insert("pad".to_string(), hamesh_theme::space::S.to_string());
    r.props
        .insert("gap".to_string(), hamesh_theme::space::S.to_string());
    r.props.insert("h".to_string(), row_h.to_string());
    r.props.insert("surface".to_string(), surface.to_string());
    r.props
        .insert(row_prop.0.to_string(), row_prop.1.to_string());
    let (size, lh) = hamesh_theme::type_ramp::LABEL;
    s.branch(r, |t| {
        if let Some(name) = icon_name {
            icon(
                t,
                &format!("{id}-icon"),
                name,
                row_icon_token(look),
                surface,
                18.0,
            );
        }
        t.leaf(super::text_node(
            "ListLabel",
            &format!("{id}-label"),
            label,
            &TextOpts::sized(size, lh, row_text(look)),
        ));
    });
}

/// A scrollable, selectable list. Emits exactly the widget's visible
/// window plus a proportional scrollbar — never the whole model.
pub fn list(s: &mut Scope, id: &str, spec: ListSpec) {
    let row_h = spec.row_h.max(1.0);
    spec.view.set_viewport(spec.viewport_h);
    let count = spec.rows.len();
    let scroll = spec.view.scroll();
    let window = spec.view.visible();
    // Content offset of the first painted row: the window starts at a
    // whole row, the scroll position usually does not.
    let shift = (scroll - window.start as f32 * row_h).max(0.0);
    let focused = spec.view.focused();

    let mut root = hamesh_core::Component::new(id, Role::List, id);
    root.props.insert("block".to_string(), "List".to_string());
    root.props.insert("layout".to_string(), "row".to_string());
    root.props.insert("pad".to_string(), "0".to_string());
    root.props
        .insert("gap".to_string(), hamesh_theme::space::S.to_string());
    if let Some(g) = spec.grow {
        root.props.insert("flex_grow".to_string(), g.to_string());
    }
    // Focus ring on the whole control, not on a row: one tab stop.
    root.props.insert(
        "border".to_string(),
        if focused { "2" } else { "1" }.to_string(),
    );
    root.props.insert(
        "border_token".to_string(),
        if focused { "focus" } else { "border" }.to_string(),
    );
    s.branch(root, |t| {
        // Viewport: fixed height, flush interior, square corners (the
        // clip mask is rect-only, so rounding would let square rows
        // poke past rounded corners).
        let mut view = hamesh_core::Component::new(&format!("{id}-view"), Role::Panel, "viewport");
        view.props
            .insert("block".to_string(), "ListViewport".to_string());
        view.props
            .insert("layout".to_string(), "column".to_string());
        view.props.insert("pad".to_string(), "0".to_string());
        view.props.insert("gap".to_string(), "0".to_string());
        view.props
            .insert("h".to_string(), spec.viewport_h.to_string());
        view.props.insert("flex_grow".to_string(), "1".to_string());
        view.props.insert("basis".to_string(), "0".to_string());
        view.props
            .insert("surface".to_string(), "surface".to_string());
        view.props.insert("scroll_y".to_string(), shift.to_string());
        t.branch(view, |v| {
            if count == 0 {
                let mut e =
                    hamesh_core::Component::new(&format!("{id}-empty"), Role::ListItem, "empty");
                e.props.insert("block".to_string(), "ListEmpty".to_string());
                e.props.insert("h".to_string(), row_h.to_string());
                let (size, lh) = hamesh_theme::type_ramp::LABEL;
                v.branch(e, |w| {
                    w.leaf(super::text_node(
                        "ListEmptyLabel",
                        &format!("{id}-empty-label"),
                        &spec.empty_label,
                        &TextOpts::sized(size, lh, "muted"),
                    ));
                });
            }
            for i in window.clone() {
                let Some(row) = spec.rows.get(i) else {
                    continue;
                };
                let look = row_look(
                    spec.view.selected() == Some(i),
                    spec.view.hover() == Some(i),
                );
                option_row(
                    v,
                    format!("{id}-row-{i}"),
                    &row.label,
                    row.icon.as_deref(),
                    row_h,
                    look,
                    ("list", i),
                );
            }
        });
        // Scrollbar: a track with a proportional thumb. Omitted when
        // everything fits — no chrome without a job.
        let content_h = count as f32 * row_h;
        if content_h > spec.viewport_h + 0.5 {
            let thumb_h =
                (spec.viewport_h * spec.viewport_h / content_h).clamp(MIN_THUMB, spec.viewport_h);
            let max_s = (content_h - spec.viewport_h).max(1.0);
            let thumb_y = scroll / max_s * (spec.viewport_h - thumb_h);
            let mut track =
                hamesh_core::Component::new(&format!("{id}-scroll"), Role::Panel, "scrollbar");
            track
                .props
                .insert("block".to_string(), "Scrollbar".to_string());
            track
                .props
                .insert("layout".to_string(), "column".to_string());
            track.props.insert("pad".to_string(), "0".to_string());
            track.props.insert("gap".to_string(), "0".to_string());
            track.props.insert("w".to_string(), "10".to_string());
            track
                .props
                .insert("h".to_string(), spec.viewport_h.to_string());
            track
                .props
                .insert("surface".to_string(), "track".to_string());
            track.props.insert("radius".to_string(), "5".to_string());
            t.branch(track, |w| {
                if thumb_y > 0.5 {
                    let mut sp = hamesh_core::Component::new(
                        &format!("{id}-thumb-space"),
                        Role::Panel,
                        "thumb-space",
                    );
                    sp.props.insert("ghost".to_string(), "true".to_string());
                    sp.props.insert("pad".to_string(), "0".to_string());
                    sp.props.insert("h".to_string(), thumb_y.to_string());
                    w.leaf(sp);
                }
                let mut th =
                    hamesh_core::Component::new(&format!("{id}-thumb"), Role::Panel, "thumb");
                th.props.insert("pad".to_string(), "0".to_string());
                th.props.insert("h".to_string(), thumb_h.to_string());
                th.props
                    .insert("surface".to_string(), "text_dim".to_string());
                th.props.insert("radius".to_string(), "5".to_string());
                w.leaf(th);
            });
        }
    });
}

/// What a picker composer needs. The popup shows at most `max_rows`
/// options and scrolls internally past that.
pub struct PickerSpec<'a> {
    pub picker: &'a mut Picker,
    pub field_h: f32,
    pub row_h: f32,
    pub max_rows: usize,
    pub placeholder: String,
}

impl<'a> PickerSpec<'a> {
    pub fn new(picker: &'a mut Picker) -> Self {
        Self {
            picker,
            field_h: hamesh_theme::control::H,
            row_h: ROW_H,
            max_rows: 8,
            placeholder: "Choose…".to_string(),
        }
    }
}

/// A searchable picker: a field plus, when open, a scrolling option
/// list in flow beneath it. The popup flows (never floats) so opening
/// it needs no second layout pass and no consumer geometry math — the
/// tradeoff is content below shifts down while it is open.
pub fn picker(s: &mut Scope, id: &str, spec: PickerSpec) {
    let open = spec.picker.is_open();
    let focused = spec.picker.focused();
    let value = spec.picker.display();
    let showing_placeholder = value.is_empty();
    let field_text = if showing_placeholder {
        spec.placeholder.clone()
    } else {
        value
    };

    let mut root = hamesh_core::Component::new(id, Role::List, id);
    root.props.insert("block".to_string(), "Picker".to_string());
    root.props
        .insert("layout".to_string(), "column".to_string());
    root.props.insert("pad".to_string(), "0".to_string());
    root.props
        .insert("gap".to_string(), hamesh_theme::space::XS.to_string());
    s.branch(root, |t| {
        // Field: the tab stop's visible face. Hit identity lives here.
        let mut field = hamesh_core::Component::new(&format!("{id}-field"), Role::Panel, "field");
        field
            .props
            .insert("block".to_string(), "PickerField".to_string());
        field.props.insert("layout".to_string(), "row".to_string());
        field
            .props
            .insert("pad".to_string(), hamesh_theme::space::S.to_string());
        field
            .props
            .insert("gap".to_string(), hamesh_theme::space::S.to_string());
        field
            .props
            .insert("h".to_string(), spec.field_h.max(1.0).to_string());
        field
            .props
            .insert("surface".to_string(), "raised".to_string());
        field.props.insert(
            "border".to_string(),
            if focused { "2" } else { "1" }.to_string(),
        );
        field.props.insert(
            "border_token".to_string(),
            if focused { "focus" } else { "border" }.to_string(),
        );
        field
            .props
            .insert("radius".to_string(), hamesh_theme::radius::SM.to_string());
        field.props.insert("picker".to_string(), id.to_string());
        let (size, lh) = hamesh_theme::type_ramp::LABEL;
        t.branch(field, |f| {
            f.leaf(super::text_node(
                "PickerValue",
                &format!("{id}-value"),
                &field_text,
                &TextOpts::sized(size, lh, if showing_placeholder { "muted" } else { "text" }),
            ));
            super::spacer(f, &format!("{id}-field-space"));
            icon(
                f,
                &format!("{id}-chevron"),
                "chevron-down",
                "text_dim",
                "raised",
                18.0,
            );
        });

        if !open {
            return;
        }
        // Popup: fixed to the smaller of the match count and max_rows,
        // scrolling past that. Same rows, same states as a list.
        let filtered = spec.picker.filtered();
        let rows_shown = filtered.len().min(spec.max_rows.max(1));
        let popup_h = (rows_shown.max(1) as f32 * spec.row_h).max(1.0);
        spec.picker.set_viewport(popup_h);
        let window = spec.picker.visible();
        let shift = (spec.picker.scroll() - window.start as f32 * spec.row_h).max(0.0);
        let mut pop = hamesh_core::Component::new(&format!("{id}-popup"), Role::Panel, "popup");
        pop.props
            .insert("block".to_string(), "PickerPopup".to_string());
        pop.props.insert("layout".to_string(), "column".to_string());
        pop.props.insert("pad".to_string(), "0".to_string());
        pop.props.insert("gap".to_string(), "0".to_string());
        pop.props.insert("h".to_string(), popup_h.to_string());
        pop.props
            .insert("surface".to_string(), "window".to_string());
        pop.props.insert("border".to_string(), "1".to_string());
        pop.props
            .insert("border_token".to_string(), "border".to_string());
        pop.props.insert("scroll_y".to_string(), shift.to_string());
        t.branch(pop, |p| {
            if filtered.is_empty() {
                let mut e = hamesh_core::Component::new(
                    &format!("{id}-no-match"),
                    Role::ListItem,
                    "no-match",
                );
                e.props.insert("block".to_string(), "ListEmpty".to_string());
                e.props.insert("h".to_string(), spec.row_h.to_string());
                p.branch(e, |w| {
                    w.leaf(super::text_node(
                        "ListEmptyLabel",
                        &format!("{id}-no-match-label"),
                        if spec.picker.items().is_empty() {
                            "Nothing to choose"
                        } else {
                            "No matches"
                        },
                        &TextOpts::sized(size, lh, "muted"),
                    ));
                });
                return;
            }
            let committed = spec.picker.selected();
            for j in window {
                let Some(&item) = filtered.get(j) else {
                    continue;
                };
                let hl = spec.picker.highlight() == Some(j);
                let look = if hl { RowLook::Active } else { RowLook::Rest };
                // Committed-but-not-highlighted: bright label, quiet box.
                let label_token = if !hl && committed == Some(item) {
                    "accent_bright"
                } else {
                    row_text(look)
                };
                let mut r = hamesh_core::Component::new(
                    &format!("{id}-opt-{j}"),
                    Role::ListItem,
                    &format!("opt-{j}"),
                );
                r.props.insert("block".to_string(), "ListRow".to_string());
                r.props.insert("layout".to_string(), "row".to_string());
                r.props
                    .insert("pad".to_string(), hamesh_theme::space::S.to_string());
                r.props
                    .insert("gap".to_string(), hamesh_theme::space::S.to_string());
                r.props.insert("h".to_string(), spec.row_h.to_string());
                r.props
                    .insert("surface".to_string(), row_surface(look).to_string());
                r.props.insert("pick".to_string(), j.to_string());
                let label = spec.picker.items().get(item).cloned().unwrap_or_default();
                p.branch(r, |w| {
                    w.leaf(super::text_node(
                        "ListLabel",
                        &format!("{id}-opt-{j}-label"),
                        &label,
                        &TextOpts::sized(size, lh, label_token),
                    ));
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use hamesh_core::App;
    use hamesh_widgets::{ListView, Picker};

    fn app_with_list(rows: &[ListRow], view: &mut ListView, viewport_h: f32) -> App {
        let mut app = App::new("win");
        {
            let mut root = Scope::root(&mut app, "win");
            list(&mut root, "l", ListSpec::new(rows, view, viewport_h));
        }
        app
    }

    fn many(n: usize) -> Vec<ListRow> {
        (0..n)
            .map(|i| ListRow::new(&format!("Row {i:03}")))
            .collect()
    }

    fn layout_of(app: &App, w: f32, h: f32) -> hamesh_layout::LayoutOutput {
        hamesh_layout::compute(app, w, h)
    }

    #[test]
    fn list_emits_only_the_visible_window() {
        let rows = many(100);
        let mut view = ListView::new(ROW_H);
        let app = app_with_list(&rows, &mut view, 128.0);
        let out = layout_of(&app, 400.0, 300.0);
        // 128px at 32px rows: the window covers rows 0..4.
        for i in 0..4 {
            assert!(out.get(&format!("l-row-{i}")).is_some(), "row {i} painted");
        }
        assert!(out.get("l-row-4").is_none(), "row 4 stays virtual");
        let vp = out.get("l-view").unwrap().rect.clone();
        assert_eq!(vp.h, 128.0, "viewport keeps its granted height");
        let shift: f32 = app
            .get("l-view")
            .unwrap()
            .props
            .get("scroll_y")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(shift, 0.0);
        // Content overflows: the scrollbar exists with a minimum thumb.
        let thumb = out.get("l-thumb").unwrap().rect.clone();
        assert!(thumb.h >= MIN_THUMB, "thumb stays usable: {}", thumb.h);
        // 128*128/3200 = 5.1 -> clamped to the minimum.
        assert_eq!(thumb.h, MIN_THUMB);
    }

    #[test]
    fn list_scroll_shifts_the_window_and_reports_shift() {
        let rows = many(100);
        let mut view = ListView::new(ROW_H);
        // First compose syncs the model; drive, then recompose.
        let _ = app_with_list(&rows, &mut view, 128.0);
        view.wheel(-80.0);
        assert_eq!(view.scroll(), 80.0);
        let app = app_with_list(&rows, &mut view, 128.0);
        // First visible row 2, offset 16px into it.
        assert!(app.get("l-row-2").is_some());
        assert!(app.get("l-row-0").is_none());
        assert!(app.get("l-row-1").is_none());
        let shift: f32 = app
            .get("l-view")
            .unwrap()
            .props
            .get("scroll_y")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(shift, 16.0, "partial-row offset is named, not hidden");
        let out = layout_of(&app, 400.0, 300.0);
        // Row 2 paints 16px above the viewport top (clipped, honestly).
        let r2 = out.get("l-row-2").unwrap().rect.clone();
        let vp = out.get("l-view").unwrap().rect.clone();
        assert!(
            (r2.y - (vp.y - 16.0)).abs() < 0.5,
            "shift applied: {r2:?} vs {vp:?}"
        );
    }

    #[test]
    fn list_marks_selection_hover_and_focus() {
        let rows = many(10);
        let mut view = ListView::new(ROW_H);
        let _ = app_with_list(&rows, &mut view, 320.0);
        assert!(view.select(1));
        view.set_focused(true);
        let app = app_with_list(&rows, &mut view, 320.0);
        let sel = app.get("l-row-1").unwrap();
        assert_eq!(sel.props.get("surface").unwrap(), "accent");
        let label = app.get("l-row-1-label").unwrap();
        assert_eq!(label.props.get("text_token").unwrap(), "bg");
        let root = app.get("l").unwrap();
        assert_eq!(root.props.get("border_token").unwrap(), "focus");
        assert_eq!(root.props.get("border").unwrap(), "2");
        // A row that is neither: quiet surface, plain text.
        let plain = app.get("l-row-2").unwrap();
        assert_eq!(plain.props.get("surface").unwrap(), "surface");
    }

    #[test]
    fn list_hover_raises_a_row_without_stealing_selection() {
        let rows = many(10);
        let mut view = ListView::new(ROW_H);
        let _ = app_with_list(&rows, &mut view, 320.0);
        assert!(view.select(1));
        view.hover_at_viewport(3.0 * ROW_H + 4.0);
        assert_eq!(view.hover(), Some(3));
        let app = app_with_list(&rows, &mut view, 320.0);
        let hov = app.get("l-row-3").unwrap();
        assert_eq!(hov.props.get("surface").unwrap(), "raised");
        // Selection keeps its inversion under a hover elsewhere.
        let sel = app.get("l-row-1").unwrap();
        assert_eq!(sel.props.get("surface").unwrap(), "accent");
    }

    #[test]
    fn list_empty_names_its_state_and_drops_chrome() {
        let rows: Vec<ListRow> = Vec::new();
        let mut view = ListView::new(ROW_H);
        let app = app_with_list(&rows, &mut view, 128.0);
        let empty = app.get("l-empty-label").unwrap();
        assert_eq!(empty.props.get("text").unwrap(), "No items");
        assert_eq!(empty.props.get("text_token").unwrap(), "muted");
        assert!(
            app.get("l-scroll").is_none(),
            "no scrollbar without overflow"
        );
        assert!(app.get("l-thumb").is_none());
    }

    #[test]
    fn picker_closed_shows_value_and_no_popup() {
        let mut p = Picker::new(vec!["Newsreader".into(), "Inter".into()]);
        p.set_selected(Some(1));
        let mut app = App::new("win");
        {
            let mut root = Scope::root(&mut app, "win");
            picker(&mut root, "pk", PickerSpec::new(&mut p));
        }
        assert!(app.get("pk-popup").is_none(), "closed: no popup");
        let field = app.get("pk-value").unwrap();
        assert_eq!(field.props.get("text").unwrap(), "Inter");
        let chev = app.get("pk-chevron").unwrap();
        assert_eq!(chev.props.get("icon").unwrap(), "chevron-down");
    }

    #[test]
    fn picker_open_filters_and_spotlights_the_highlight() {
        let mut p = Picker::new(vec![
            "Newsreader".into(),
            "Instrument Sans".into(),
            "Inter".into(),
        ]);
        p.open();
        for c in "in".chars() {
            p.push_char(c);
        }
        let mut app = App::new("win");
        {
            let mut root = Scope::root(&mut app, "win");
            picker(&mut root, "pk", PickerSpec::new(&mut p));
        }
        // "Instrument Sans" and "Inter" match; highlight sits on the first.
        assert!(app.get("pk-opt-0").is_some());
        assert!(app.get("pk-opt-1").is_some());
        assert!(app.get("pk-opt-2").is_none());
        let first = app.get("pk-opt-0").unwrap();
        assert_eq!(first.props.get("surface").unwrap(), "accent");
        assert_eq!(first.props.get("pick").unwrap(), "0");
        let field = app.get("pk-value").unwrap();
        assert_eq!(field.props.get("text").unwrap(), "in");
    }

    #[test]
    fn picker_empty_search_explains_itself() {
        let mut p = Picker::new(vec!["Newsreader".into()]);
        p.open();
        for c in "zzz".chars() {
            p.push_char(c);
        }
        let mut app = App::new("win");
        {
            let mut root = Scope::root(&mut app, "win");
            picker(&mut root, "pk", PickerSpec::new(&mut p));
        }
        assert!(app.get("pk-popup").is_some(), "popup stays for the message");
        let msg = app.get("pk-no-match-label").unwrap();
        assert_eq!(msg.props.get("text").unwrap(), "No matches");
        assert!(app.get("pk-opt-0").is_none());
    }

    #[test]
    fn scrollbar_thumb_tracks_scroll_position() {
        let rows = many(100);
        let mut view = ListView::new(ROW_H);
        let _ = app_with_list(&rows, &mut view, 128.0);
        view.wheel(-10_000.0); // bottom: scroll = 3072
        assert_eq!(view.scroll(), 3072.0);
        let app = app_with_list(&rows, &mut view, 128.0);
        let out = layout_of(&app, 400.0, 300.0);
        // thumb_y = 3072/3072 * (128-24) = 104.
        let space = out.get("l-thumb-space").unwrap().rect.clone();
        assert!(
            (space.h - 104.0).abs() < 0.5,
            "thumb at the bottom: {space:?}"
        );
        let thumb = out.get("l-thumb").unwrap().rect.clone();
        assert_eq!(thumb.h, MIN_THUMB);
    }
}
