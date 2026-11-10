//! hamesh-sheet: the 036 design-system sheet as a reusable driver.
//!
//! The same composition the headless lab renders, owned by widget
//! state and driven through the shared `Hit` / `Key` vocabulary.
//! This is the native route for the sheet: `hamesh-window --sheet`
//! forwards real OS input into these exact functions, so visual
//! states (hover, pressed, focus, selection) are proven live, not
//! just painted.

use hamesh_blocks as b;
use hamesh_core::App;
use hamesh_widgets::{list_nav, Hit, Key, ListView};

pub const ROW_H: f32 = 32.0;

/// Demo buttons: 3 variants × 5 states. The id doubles as the action
/// command, so pointer and keyboard resolve through `Hit::Action`
/// with zero id parsing in the driver.
pub struct DemoButton {
    pub id: &'static str,
    pub action: &'static str,
    pub variant: b::ButtonVariant,
    pub label: &'static str,
    pub demo: b::ButtonState,
    pub demo_focused: bool,
}

pub const BUTTONS: [DemoButton; 15] = [
    DemoButton {
        id: "btn-primary-rest",
        action: "primary-rest",
        variant: b::ButtonVariant::Primary,
        label: "Create",
        demo: b::ButtonState::Rest,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-primary-hover",
        action: "primary-hover",
        variant: b::ButtonVariant::Primary,
        label: "Create",
        demo: b::ButtonState::Hover,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-primary-pressed",
        action: "primary-pressed",
        variant: b::ButtonVariant::Primary,
        label: "Create",
        demo: b::ButtonState::Pressed,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-primary-disabled",
        action: "primary-disabled",
        variant: b::ButtonVariant::Primary,
        label: "Create",
        demo: b::ButtonState::Disabled,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-primary-focused",
        action: "primary-focused",
        variant: b::ButtonVariant::Primary,
        label: "Create",
        demo: b::ButtonState::Rest,
        demo_focused: true,
    },
    DemoButton {
        id: "btn-outline-rest",
        action: "outline-rest",
        variant: b::ButtonVariant::Outline,
        label: "Cancel",
        demo: b::ButtonState::Rest,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-outline-hover",
        action: "outline-hover",
        variant: b::ButtonVariant::Outline,
        label: "Cancel",
        demo: b::ButtonState::Hover,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-outline-pressed",
        action: "outline-pressed",
        variant: b::ButtonVariant::Outline,
        label: "Cancel",
        demo: b::ButtonState::Pressed,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-outline-disabled",
        action: "outline-disabled",
        variant: b::ButtonVariant::Outline,
        label: "Cancel",
        demo: b::ButtonState::Disabled,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-outline-focused",
        action: "outline-focused",
        variant: b::ButtonVariant::Outline,
        label: "Cancel",
        demo: b::ButtonState::Rest,
        demo_focused: true,
    },
    DemoButton {
        id: "btn-ghost-rest",
        action: "ghost-rest",
        variant: b::ButtonVariant::Ghost,
        label: "More",
        demo: b::ButtonState::Rest,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-ghost-hover",
        action: "ghost-hover",
        variant: b::ButtonVariant::Ghost,
        label: "More",
        demo: b::ButtonState::Hover,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-ghost-pressed",
        action: "ghost-pressed",
        variant: b::ButtonVariant::Ghost,
        label: "More",
        demo: b::ButtonState::Pressed,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-ghost-disabled",
        action: "ghost-disabled",
        variant: b::ButtonVariant::Ghost,
        label: "More",
        demo: b::ButtonState::Disabled,
        demo_focused: false,
    },
    DemoButton {
        id: "btn-ghost-focused",
        action: "ghost-focused",
        variant: b::ButtonVariant::Ghost,
        label: "More",
        demo: b::ButtonState::Rest,
        demo_focused: true,
    },
];

pub struct Sheet {
    pub list: ListView,
    pub hover_btn: Option<String>,
    pub pressed_btn: Option<String>,
    /// 0 = selection list, 1 = buttons grid.
    pub focus: usize,
    pub btn_idx: usize,
}

impl Sheet {
    pub fn new() -> Self {
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let mut list = ListView::new(ROW_H);
        list.set_count(4);
        assert!(list.select(1));
        assert!(list.hover_at_viewport(2.0 * ROW_H + 4.0));
        list.set_focused(true);
        Self {
            list,
            hover_btn: None,
            pressed_btn: None,
            focus: 0,
            btn_idx: 0,
        }
    }

    fn button_state(&self, b: &DemoButton) -> b::ButtonState {
        if matches!(b.demo, b::ButtonState::Disabled) {
            return b::ButtonState::Disabled;
        }
        if self.pressed_btn.as_deref() == Some(b.action) {
            return b::ButtonState::Pressed;
        }
        if self.hover_btn.as_deref() == Some(b.action) {
            return b::ButtonState::Hover;
        }
        b.demo
    }

    fn button_focused(&self, i: usize) -> bool {
        BUTTONS[i].demo_focused || (self.focus == 1 && self.btn_idx == i)
    }

    pub fn action_enabled(action: &str) -> bool {
        !BUTTONS
            .iter()
            .find(|b| b.action == action)
            .is_some_and(|b| matches!(b.demo, b::ButtonState::Disabled))
    }
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}

fn state_list() -> Vec<b::select::ListRow> {
    vec![
        b::select::ListRow::new("Inbox").with_icon("activity"),
        b::select::ListRow::new("Drafts").with_icon("list-checks"),
        b::select::ListRow::new("Sent").with_icon("users"),
        b::select::ListRow::new("Archive").with_icon("triangle-alert"),
    ]
}

/// One matrix row: a fixed-width name cell plus five fixed-width
/// button cells. Fixed cells keep every state column aligned across
/// variants (hugging labels would stagger each row's start).
const NAME_W: f32 = 76.0;
const BTN_W: f32 = 104.0;

fn button_row(s: &mut b::Scope, t: &mut Sheet, row: usize, name: &str, label: &str) {
    b::row(
        s,
        &format!("row-{name}"),
        b::LayoutOpts {
            gap: 8.0,
            ..Default::default()
        },
        |r| {
            b::col(
                r,
                &format!("row-{name}-name-cell"),
                b::LayoutOpts {
                    w: Some(NAME_W),
                    pad: 0.0,
                    ..Default::default()
                },
                |c| {
                    b::caption(c, &format!("row-{name}-name"), label);
                },
            );
            for (i, bdef) in BUTTONS.iter().enumerate().skip(row * 5).take(5) {
                b::col(
                    r,
                    &format!(
                        "btn-cell-{}-{}",
                        name,
                        ["rest", "hover", "pressed", "disabled", "focused"][i % 5]
                    ),
                    b::LayoutOpts {
                        w: Some(BTN_W),
                        pad: 0.0,
                        ..Default::default()
                    },
                    |c| {
                        b::button(
                            c,
                            &format!(
                                "btn-{}-{}",
                                name,
                                ["rest", "hover", "pressed", "disabled", "focused"][i % 5]
                            ),
                            bdef.label,
                            b::ButtonOpts::new(bdef.variant.clone(), bdef.action)
                                .with_state(t.button_state(bdef))
                                .focused(t.button_focused(i)),
                        );
                    },
                );
            }
        },
    );
}

fn layout_of(app: &App, w: f32, h: f32) -> hamesh_layout::LayoutOutput {
    hamesh_layout::compute(app, w, h)
}

fn compose(t: &mut Sheet, width: f32, height: f32) -> App {
    t.list.set_focused(t.focus == 0);
    let rows = state_list();
    let short = height < 620.0;
    let tall = height >= 900.0;
    let (pad, gap) = if short { (6.0, 4.0) } else { (12.0, 8.0) };
    let mut app = App::new("sheet");
    {
        let mut root = b::Scope::root(&mut app, "sheet");
        b::row(
            &mut root,
            "header",
            b::LayoutOpts {
                pad: if short { 8.0 } else { 12.0 },
                ..Default::default()
            },
            |s| {
                b::title(s, "sheet-title", "Design System");
                b::spacer(s, "sheet-space");
                b::caption(s, "sheet-ver", "HAMESH 036 · DEFAULT");
            },
        );
        b::col(
            &mut root,
            "body",
            b::LayoutOpts {
                grow: Some(1.0),
                pad,
                gap,
                ..Default::default()
            },
            |s| {
                b::caption(s, "sec-btn", "BUTTONS");
                button_row(s, t, 0, "primary", "Primary");
                button_row(s, t, 1, "outline", "Outline");
                button_row(s, t, 2, "ghost", "Ghost");
                b::row(
                    s,
                    "mid",
                    b::LayoutOpts {
                        grow: Some(1.0),
                        gap,
                        ..Default::default()
                    },
                    |m| {
                        b::col(
                            m,
                            "mid-left",
                            b::LayoutOpts {
                                gap,
                                ..Default::default()
                            },
                            |u| {
                                b::caption(u, "sec-sel", "SELECTION");
                                b::select::list(
                                    u,
                                    "demo-list",
                                    b::select::ListSpec {
                                        rows: &rows,
                                        view: &mut t.list,
                                        viewport_h: 4.0 * ROW_H,
                                        row_h: ROW_H,
                                        empty_label: "No items".to_string(),
                                        grow: None,
                                        viewport_w: None,
                                    },
                                );
                            },
                        );
                        b::col(
                            m,
                            "mid-right",
                            b::LayoutOpts {
                                grow: Some(1.0),
                                gap,
                                ..Default::default()
                            },
                            |u| {
                                b::caption(u, "sec-type", "TYPE");
                                b::heading(u, "type-head", "Set in the default voice");
                                if !short {
                                    b::body(u, "type-body", "Body copy at the text ramp size.");
                                }
                                b::card(
                                    u,
                                    "contract",
                                    b::CardOpts {
                                        pad: if short { 6.0 } else { 8.0 },
                                        gap: if short { 6.0 } else { 8.0 },
                                        ..b::CardOpts::default()
                                    },
                                    |v| {
                                        b::caption(v, "contract-k", "CONTRACT");
                                        b::body(v, "contract-1", "Selected over hover over rest.");
                                        b::body(v, "contract-2", "Focus rings, never refills.");
                                        if !short {
                                            b::body(
                                                v,
                                                "contract-3",
                                                "Disabled paint means disabled action.",
                                            );
                                        }
                                    },
                                );
                            },
                        );
                    },
                );
                if tall {
                    b::caption(s, "sec-surf", "SURFACES");
                    b::row(
                        s,
                        "surf-row",
                        b::LayoutOpts {
                            gap,
                            ..Default::default()
                        },
                        |r| {
                            for tok in [
                                "window",
                                "surface",
                                "raised",
                                "header",
                                "accent",
                                "accent_hover",
                                "accent_active",
                            ] {
                                b::swatch(r, &format!("sw-{tok}"), tok);
                            }
                        },
                    );
                }
            },
        );
        b::row(
            &mut root,
            "footer",
            b::LayoutOpts {
                h: Some(28.0),
                pad: 8.0,
                ..Default::default()
            },
            |s| {
                b::caption(s, "foot-status", "Hamesh · 036 design system");
                b::spacer(s, "foot-space");
                b::caption(s, "foot-size", &format!("{width:.0}×{height:.0}"));
            },
        );
    }
    let _ = height;
    app
}

/// Single-pass refresh: the sheet grants its own fixed budgets, so one
/// compose is exact (no probe pass needed).
pub fn refresh(t: &mut Sheet, w: f32, h: f32) -> (App, hamesh_layout::LayoutOutput) {
    let app = compose(t, w, h);
    let layout = layout_of(&app, w, h);
    (app, layout)
}

/// Activate a demo button: disabled refuses, anything else latches the
/// pressed look and reports what happened.
fn activate(t: &mut Sheet, action: &str) -> String {
    if !Sheet::action_enabled(action) {
        return format!("{action} disabled: no action");
    }
    t.pressed_btn = Some(action.to_string());
    format!("activated {action}")
}

/// Pointer press through the SHARED router: hit-test, resolve with
/// `Hit::of`, drive the widget. No per-app climbing or id parsing.
pub fn press(t: &mut Sheet, app: &App, scene: &hamesh_scene::Scene, x: f32, y: f32) -> String {
    match hamesh_scene::hit_test(app, scene, x, y) {
        None => "press hit nothing".to_string(),
        Some(id) => match Hit::of(app, &id) {
            Hit::ListRow { row, .. } => {
                t.focus = 0;
                // The pointer left the buttons: stale hover must not paint.
                t.hover_btn = None;
                // A press elsewhere clears the pressed latch: only the
                // actually pressed button wears Pressed.
                t.pressed_btn = None;
                if t.list.select(row) {
                    format!("selected demo row {row}")
                } else {
                    format!("demo row {row} already selected")
                }
            }
            Hit::Action { command } => {
                t.focus = 1;
                // Cursor follows the pointer so keyboard and pointer agree.
                if let Some(i) = BUTTONS.iter().position(|b| b.action == command) {
                    t.btn_idx = i;
                }
                act_on(t, &command)
            }
            Hit::Chrome(hit) => {
                t.pressed_btn = None;
                format!("hit {hit} (chrome, no action)")
            }
            _ => "press ignored".to_string(),
        },
    }
}

fn act_on(t: &mut Sheet, command: &str) -> String {
    activate(t, command)
}

pub fn hover(t: &mut Sheet, scene: &hamesh_scene::Scene, x: f32, y: f32, app: &App) -> bool {
    let Some(id) = hamesh_scene::hit_test(app, scene, x, y) else {
        t.hover_btn = None;
        return false;
    };
    match Hit::of(app, &id) {
        Hit::Action { command } => {
            let changed = t.hover_btn.as_deref() != Some(&command);
            t.hover_btn = Some(command);
            changed
        }
        Hit::ListRow { .. } => {
            t.hover_btn = None;
            if let Some(r) = scene
                .nodes
                .iter()
                .find(|n| n.id == "demo-list-view")
                .map(|n| n.rect.clone())
            {
                t.list.hover_at_viewport(y - r.y)
            } else {
                false
            }
        }
        _ => {
            t.hover_btn = None;
            false
        }
    }
}

pub fn wheel(t: &mut Sheet, layout: &hamesh_layout::LayoutOutput, x: f32, y: f32, dy: f32) -> bool {
    let over = |id: &str| layout.nodes.get(id).is_some_and(|n| n.rect.contains(x, y));
    if over("demo-list-view") {
        t.list.wheel(dy)
    } else {
        false
    }
}

pub fn key(t: &mut Sheet, k: Key) -> String {
    if matches!(k, Key::Tab | Key::ShiftTab) {
        let fwd = matches!(k, Key::Tab);
        // Two zones wrap in both directions.
        t.focus = match (t.focus, fwd) {
            (0, true) => 1,
            (1, false) => 0,
            (0, false) => 1,
            (f, _) => (f + 1) % 2,
        };
        return format!("focus -> {}", ["list", "buttons"][t.focus]);
    }
    if t.focus == 0 {
        if let Some(moved) = list_nav(&mut t.list, &k) {
            return format!("demo nav: {moved}");
        }
        match k {
            Key::Enter => match t.list.selected() {
                Some(r) => format!("demo row {r} confirmed"),
                None => "demo: nothing selected".to_string(),
            },
            Key::Escape => "demo: nothing to dismiss".to_string(),
            _ => "list: key ignored".to_string(),
        }
    } else {
        match k {
            Key::Left => {
                t.btn_idx = t.btn_idx.saturating_sub(1);
                format!("button cursor: {}", BUTTONS[t.btn_idx].id)
            }
            Key::Right => {
                t.btn_idx = (t.btn_idx + 1).min(BUTTONS.len() - 1);
                format!("button cursor: {}", BUTTONS[t.btn_idx].id)
            }
            Key::Up => {
                t.btn_idx = t.btn_idx.saturating_sub(5);
                format!("button cursor: {}", BUTTONS[t.btn_idx].id)
            }
            Key::Down => {
                t.btn_idx = (t.btn_idx + 5).min(BUTTONS.len() - 1);
                format!("button cursor: {}", BUTTONS[t.btn_idx].id)
            }
            Key::Home => {
                t.btn_idx = 0;
                format!("button cursor: {}", BUTTONS[t.btn_idx].id)
            }
            Key::End => {
                t.btn_idx = BUTTONS.len() - 1;
                format!("button cursor: {}", BUTTONS[t.btn_idx].id)
            }
            Key::Enter => act_on(t, BUTTONS[t.btn_idx].action),
            Key::Escape => "buttons: nothing to dismiss".to_string(),
            _ => "buttons: arrows move, Enter activates".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        t: &mut Sheet,
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
        let mut t = Sheet::new();
        let (app, _layout, _scene) = frame(&mut t, 1440.0, 900.0);
        assert!(matches!(
            Hit::of(&app, "btn-primary-rest"),
            Hit::Action { .. }
        ));
        assert!(matches!(
            Hit::of(&app, "demo-list-row-1-label"),
            Hit::ListRow { row: 1, .. }
        ));
    }

    #[test]
    fn button_matrix_paints_the_contract() {
        // Every demo cell paints its documented state with no input.
        let mut t = Sheet::new();
        let (app, _layout, _scene) = frame(&mut t, 1440.0, 900.0);
        let expect = [
            ("btn-primary-rest", "accent", "bg"),
            ("btn-primary-hover", "accent_hover", "bg"),
            ("btn-primary-pressed", "accent_active", "bg"),
            ("btn-primary-disabled", "surface", "muted"),
            ("btn-ghost-hover", "raised", "text"),
        ];
        for (id, surface, text) in expect {
            let c = app.get(id).unwrap_or_else(|| panic!("{id} painted"));
            assert_eq!(
                c.props.get("surface").map(|s| s.as_str()),
                Some(surface),
                "{id}"
            );
            assert_eq!(
                c.props.get("text_token").map(|s| s.as_str()),
                Some(text),
                "{id}"
            );
        }
        let dis = app.get("btn-ghost-disabled").unwrap();
        assert!(!dis.enabled, "disabled paint means disabled hit-testing");
    }

    #[test]
    fn hover_press_and_latch_follow_the_contract() {
        // Primary rest/hover/pressed all differ: accent, accent_hover,
        // accent_active. Hover first, then press, then move away.
        let mut t = Sheet::new();
        let (app, _layout, scene) = frame(&mut t, 1440.0, 900.0);
        let (hx, hy) = center_of(&scene, "btn-primary-rest");
        assert!(hover(&mut t, &scene, hx, hy, &app));
        let (app2, _layout2, _scene2) = frame(&mut t, 1440.0, 900.0);
        assert_eq!(
            app2.get("btn-primary-rest")
                .unwrap()
                .props
                .get("surface")
                .map(|s| s.as_str()),
            Some("accent_hover"),
            "live hover paints"
        );
        let (app3, _layout3, scene3) = frame(&mut t, 1440.0, 900.0);
        let (cx, cy) = center_of(&scene3, "btn-primary-rest");
        let note = press(&mut t, &app3, &scene3, cx, cy);
        assert!(note.contains("activated primary-rest"), "{note}");
        let (app4, _layout4, scene4) = frame(&mut t, 1440.0, 900.0);
        assert_eq!(
            app4.get("btn-primary-rest")
                .unwrap()
                .props
                .get("surface")
                .map(|s| s.as_str()),
            Some("accent_active"),
            "activation latches pressed"
        );
        // A press elsewhere clears the latch (and the stale hover).
        let (cx, cy) = center_of(&scene4, "demo-list-row-0");
        let note = press(&mut t, &app4, &scene4, cx, cy);
        assert!(note.contains("selected demo row 0"), "{note}");
        let (app5, _layout5, scene5) = frame(&mut t, 1440.0, 900.0);
        assert_eq!(
            app5.get("btn-primary-rest")
                .unwrap()
                .props
                .get("surface")
                .map(|s| s.as_str()),
            Some("accent"),
            "latch cleared by the next press"
        );
        // Disabled never latches, through any path.
        let (dx, dy) = center_of(&scene5, "btn-ghost-disabled");
        let note = press(&mut t, &app5, &scene5, dx, dy);
        assert!(note.contains("disabled"), "{note}");
        assert!(t.pressed_btn.is_none());
    }

    #[test]
    fn keyboard_reaches_every_button_and_respects_disabled() {
        let mut t = Sheet::new();
        let _ = key(&mut t, Key::Tab);
        assert_eq!(t.focus, 1);
        let _ = key(&mut t, Key::End);
        assert_eq!(t.btn_idx, 14);
        assert_eq!(BUTTONS[14].action, "ghost-focused");
        let _ = key(&mut t, Key::Home);
        assert_eq!(t.btn_idx, 0);
        // Ghost/disabled refuses through the keyboard too.
        t.btn_idx = 13;
        let note = key(&mut t, Key::Enter);
        assert!(note.contains("disabled"), "{note}");
        assert!(t.pressed_btn.is_none());
        let before = t.btn_idx;
        assert_eq!(before, 13);
        t.btn_idx = 1;
        let note = key(&mut t, Key::Enter);
        assert!(note.contains("activated primary-hover"), "{note}");
        assert_eq!(t.pressed_btn.as_deref(), Some("primary-hover"));
        assert_eq!(t.btn_idx, 1, "activation never moves the cursor");
    }

    #[test]
    fn list_zone_uses_shared_navigation() {
        let mut t = Sheet::new();
        let _ = key(&mut t, Key::Down);
        assert_eq!(t.list.selected(), Some(2));
        let note = key(&mut t, Key::Enter);
        assert!(note.contains("demo row 2 confirmed"), "{note}");
    }

    #[test]
    fn shift_tab_walks_back() {
        let mut t = Sheet::new();
        let _ = key(&mut t, Key::ShiftTab);
        assert_eq!(t.focus, 1, "list wraps back into buttons");
        let _ = key(&mut t, Key::ShiftTab);
        assert_eq!(t.focus, 0);
    }
}
