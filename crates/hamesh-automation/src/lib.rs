//! hamesh-automation: the Hand. Two distinct mechanisms, always labeled.
//!
//! - HumanEquivalent: hit-test + enabled/visible/clipped/overlay checks,
//!   then route through the same state transition a human click would cause.
//! - DirectOp: invoke the typed command directly, bypassing hit-testing.
//!   Must never masquerade as a GUI test.

use hamesh_core::{App, Mechanism};
use hamesh_layout::LayoutOutput;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutomationResult {
    pub mechanism: Mechanism,
    pub action: String,
    pub target: String,
    pub success: bool,
    pub hit_component: Option<String>,
    pub revision_before: u64,
    pub revision_after: u64,
    pub notes: Vec<String>,
}

fn center(layout: &LayoutOutput, id: &str) -> Option<(f32, f32)> {
    layout
        .nodes
        .get(id)
        .map(|n| (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0))
}

/// Human-equivalent focus: click the target (full hit-test path) and, if
/// the click lands on it AND it is in the keyboard focus order, move
/// keyboard focus there — exactly like a human clicking into a field.
pub fn focus_human(
    app: &mut App,
    widgets: &mut hamesh_widgets::Widgets,
    layout: &LayoutOutput,
    id: &str,
) -> AutomationResult {
    let before = app.revision;
    // Click at the target's center through the full hit-test path, but
    // judge by WHERE the pointer lands, not by command routing: text
    // fields take focus without owning a command.
    let Some((x, y)) = center(layout, id) else {
        return AutomationResult {
            mechanism: Mechanism::HumanEquivalent,
            action: "focus".into(),
            target: id.into(),
            success: false,
            hit_component: None,
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["no layout rect for target".into()],
        };
    };
    let click = click_human(app, layout, x, y);
    if click.hit_component.as_deref() != Some(id) {
        return AutomationResult {
            mechanism: Mechanism::HumanEquivalent,
            action: "focus".into(),
            target: id.into(),
            success: false,
            hit_component: click.hit_component,
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["pointer did not reach target; focus unchanged".into()],
        };
    }
    let enabled = app.components.get(id).map(|c| c.enabled).unwrap_or(false);
    if !enabled {
        return AutomationResult {
            mechanism: Mechanism::HumanEquivalent,
            action: "focus".into(),
            target: id.into(),
            success: false,
            hit_component: Some(id.into()),
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["target disabled: no focus".into()],
        };
    }
    // Click hit the target. If it takes keyboard focus, move it.
    // (Buttons activate on click AND take focus in real toolkits.)
    if let Some(pos) = widgets.focus.order.iter().position(|o| o == id) {
        widgets.focus.current = pos;
        hamesh_widgets::sync_composer(app, widgets);
        AutomationResult {
            mechanism: Mechanism::HumanEquivalent,
            action: "focus".into(),
            target: id.into(),
            success: true,
            hit_component: Some(id.into()),
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["pointer reached target; keyboard focus moved".into()],
        }
    } else {
        AutomationResult {
            mechanism: Mechanism::HumanEquivalent,
            action: "focus".into(),
            target: id.into(),
            success: false,
            hit_component: Some(id.into()),
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["click landed but target is not focusable".into()],
        }
    }
}

/// Human-equivalent key press through the keyboard focus path.
/// Tab/ShiftTab always traverse focus; text entry requires the focused
/// composer to be visible and enabled — otherwise the key is ignored
/// and reported (like a human typing into the void).
pub fn key_human(
    app: &mut App,
    widgets: &mut hamesh_widgets::Widgets,
    key: &hamesh_widgets::Key,
) -> AutomationResult {
    let before = app.revision;
    let what = hamesh_widgets::handle_key(app, widgets, key);
    let ignored = what.contains("ignored") || what.contains("no-op");
    AutomationResult {
        mechanism: Mechanism::HumanEquivalent,
        action: "key".into(),
        target: format!("{key:?}"),
        success: !ignored,
        hit_component: None,
        revision_before: before,
        revision_after: app.revision,
        notes: vec![what],
    }
}

/// Human-equivalent typing: click-to-focus, then one key event per char.
/// Fails honestly when the target is covered, disabled, invisible, or
/// not a text input — a human could not type into it either.
pub fn type_human(
    app: &mut App,
    widgets: &mut hamesh_widgets::Widgets,
    layout: &LayoutOutput,
    target_id: &str,
    text: &str,
) -> AutomationResult {
    let before = app.revision;
    let focus = focus_human(app, widgets, layout, target_id);
    if !focus.success {
        return AutomationResult {
            mechanism: Mechanism::HumanEquivalent,
            action: "type".into(),
            target: target_id.into(),
            success: false,
            hit_component: focus.hit_component,
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["could not focus target; no text entered".into()],
        };
    }
    let mut entered = 0;
    for c in text.chars() {
        let r = key_human(app, widgets, &hamesh_widgets::Key::Char(c));
        if r.success {
            entered += 1;
        } else {
            break;
        }
    }
    AutomationResult {
        mechanism: Mechanism::HumanEquivalent,
        action: "type".into(),
        target: target_id.into(),
        success: entered == text.chars().count(),
        hit_component: Some(target_id.into()),
        revision_before: before,
        revision_after: app.revision,
        notes: vec![format!(
            "entered {entered}/{} chars via focus path",
            text.chars().count()
        )],
    }
}

/// Direct composer mutation. Bypasses focus, visibility and hit-testing
/// by design — labeled DirectOp, never GUI proof.
pub fn set_composer_text_direct(
    app: &mut App,
    widgets: &mut hamesh_widgets::Widgets,
    text: &str,
) -> AutomationResult {
    let before = app.revision;
    widgets.composer.text = text.to_string();
    widgets.composer.caret = text.chars().count();
    hamesh_widgets::sync_composer(app, widgets);
    AutomationResult {
        mechanism: Mechanism::DirectOp,
        action: "set-text".into(),
        target: widgets.composer_id.clone(),
        success: true,
        hit_component: None,
        revision_before: before,
        revision_after: app.revision,
        notes: vec!["direct op: did NOT traverse focus/hit-test path".into()],
    }
}

/// Human-equivalent click at a point: hit-test, then apply the target's
/// command if the hit target is enabled and visible.
pub fn click_human(app: &mut App, layout: &LayoutOutput, x: f32, y: f32) -> AutomationResult {
    let before = app.revision;
    let mut notes = Vec::new();
    // Paint truth, not layout rects: clipped-away content cannot
    // intercept the Hand. The scene rebuilds from the live tree, so
    // a press always targets what is actually on screen.
    let scene = hamesh_scene::build(app, layout, app.revision);
    let hit = hamesh_scene::hit_test(app, &scene, x, y);
    match hit {
        None => AutomationResult {
            mechanism: Mechanism::HumanEquivalent,
            action: "click".into(),
            target: format!("({x:.1},{y:.1})"),
            success: false,
            hit_component: None,
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["no hit-test target at point".into()],
        },
        Some(id) => {
            let comp = app.components.get(&id).cloned();
            match comp {
                None => AutomationResult {
                    mechanism: Mechanism::HumanEquivalent,
                    action: "click".into(),
                    target: format!("({x:.1},{y:.1})"),
                    success: false,
                    hit_component: Some(id),
                    revision_before: before,
                    revision_after: app.revision,
                    notes: vec!["hit unknown component".into()],
                },
                Some(c) => {
                    if !c.enabled {
                        notes.push(format!("hit {id} but it is disabled: no effect (correct)"));
                        return AutomationResult {
                            mechanism: Mechanism::HumanEquivalent,
                            action: "click".into(),
                            target: id.clone(),
                            success: false,
                            hit_component: Some(id),
                            revision_before: before,
                            revision_after: app.revision,
                            notes,
                        };
                    }
                    if let Some(n) = layout.nodes.get(&id) {
                        if !n.effectively_visible {
                            notes.push(format!("hit {id} but effectively invisible"));
                        }
                        if n.clipped_by_parent {
                            notes.push(format!("{id} clipped by parent: click may be partial"));
                        }
                    }
                    // Route through the command owned by this component, if any.
                    let cmd = app
                        .commands
                        .iter()
                        .find(|(_, owner)| *owner == &id)
                        .map(|(k, _)| k.clone());
                    match cmd {
                        Some(cmd_id) => {
                            apply_command(app, &cmd_id);
                            notes.push(format!("routed click -> command {cmd_id}"));
                            AutomationResult {
                                mechanism: Mechanism::HumanEquivalent,
                                action: "click".into(),
                                target: id.clone(),
                                success: true,
                                hit_component: Some(id),
                                revision_before: before,
                                revision_after: app.revision,
                                notes,
                            }
                        }
                        None => {
                            notes.push(format!("hit {id} but it owns no command"));
                            AutomationResult {
                                mechanism: Mechanism::HumanEquivalent,
                                action: "click".into(),
                                target: id.clone(),
                                success: false,
                                hit_component: Some(id),
                                revision_before: before,
                                revision_after: app.revision,
                                notes,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Human-equivalent click on a component by id (moves pointer to its center).
pub fn click_component_human(app: &mut App, layout: &LayoutOutput, id: &str) -> AutomationResult {
    let Some((x, y)) = center(layout, id) else {
        return AutomationResult {
            mechanism: Mechanism::HumanEquivalent,
            action: "click".into(),
            target: id.into(),
            success: false,
            hit_component: None,
            revision_before: app.revision,
            revision_after: app.revision,
            notes: vec!["no layout rect for target".into()],
        };
    };
    let mut r = click_human(app, layout, x, y);
    r.target = id.to_string();
    // Detect interception: aimed at A but hit B (overlay case).
    if r.hit_component.as_deref() != Some(id) {
        r.notes.push(format!(
            "aimed at {id} but hit {:?}: intercepted or occluded",
            r.hit_component
        ));
        r.success = false;
    }
    r
}

/// Direct typed invocation. Bypasses hit-testing by design.
pub fn invoke_direct(app: &mut App, command_id: &str) -> AutomationResult {
    let before = app.revision;
    if app.commands.contains_key(command_id) {
        apply_command(app, command_id);
        AutomationResult {
            mechanism: Mechanism::DirectOp,
            action: "invoke".into(),
            target: command_id.into(),
            success: true,
            hit_component: None,
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["direct op: did NOT traverse hit-test/focus path".into()],
        }
    } else {
        AutomationResult {
            mechanism: Mechanism::DirectOp,
            action: "invoke".into(),
            target: command_id.into(),
            success: false,
            hit_component: None,
            revision_before: before,
            revision_after: app.revision,
            notes: vec!["unknown command".into()],
        }
    }
}

/// Canonical state transitions for demo commands.
fn apply_command(app: &mut App, command_id: &str) {
    match command_id {
        "increment" => {
            let cur: i64 = app
                .state
                .get("counter")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            app.set_state("counter", &(cur + 1).to_string());
        }
        "reset" => {
            app.set_state("counter", "0");
        }
        "inspect_design" => {
            // Dashboard action: records that a human reviewed the design.
            // The window re-syncs the button label via refresh_dashboard.
            app.set_state("design_inspected", "true");
        }
        "pause_worker" => {
            // Pi panel action: toggles paused state; the window
            // re-syncs the button label via refresh_pi_panel.
            let paused = app.state.get("paused").map(|s| s.as_str()) == Some("true");
            app.set_state("paused", if paused { "false" } else { "true" });
        }
        _ => {
            app.bump();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hamesh_core::{Component, Role};

    fn app_with_button() -> (App, LayoutOutput) {
        let mut app = App::new("win");
        app.add("win", Component::new("btn", Role::Button, "Increment"));
        app.register_command("increment", "btn");
        app.set_state("counter", "0");
        let layout = hamesh_layout::compute(&app, 400.0, 300.0);
        (app, layout)
    }

    #[test]
    fn human_click_increments() {
        let (mut app, layout) = app_with_button();
        let r = click_component_human(&mut app, &layout, "btn");
        assert!(r.success, "{r:?}");
        assert_eq!(r.mechanism, Mechanism::HumanEquivalent);
        assert_eq!(app.state.get("counter").unwrap(), "1");
    }

    #[test]
    fn disabled_button_blocks_human_but_direct_still_labeled() {
        let (mut app, layout) = app_with_button();
        app.get_mut("btn").unwrap().enabled = false;
        let r = click_component_human(&mut app, &layout, "btn");
        assert!(!r.success);
        assert_eq!(app.state.get("counter").unwrap(), "0");
        let d = invoke_direct(&mut app, "increment");
        assert_eq!(d.mechanism, Mechanism::DirectOp);
        assert!(d.success);
    }

    #[test]
    fn overlay_intercepts() {
        let mut app = App::new("win");
        app.add("win", Component::new("btn", Role::Button, "Increment"));
        app.register_command("increment", "btn");
        app.set_state("counter", "0");
        // Transparent overlay parked over the button's area.
        app.add(
            "win",
            Component::new("veil", Role::Overlay, "veil")
                .with_prop("overlay", "true")
                .with_prop("ox", "0")
                .with_prop("oy", "0")
                .with_prop("w", "800")
                .with_prop("h", "600"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let r = click_component_human(&mut app, &layout, "btn");
        assert!(!r.success, "overlay should intercept: {r:?}");
        assert_ne!(r.hit_component.as_deref(), Some("btn"));
    }

    fn scrolled_list() -> (App, LayoutOutput) {
        // filler 500 + viewport h80 scrolled 40 + two rows h40.
        // r0 sits at 440..480 (fully clipped); r1 spans 480..520.
        use hamesh_core::{Component, Role};
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("filler", Role::Panel, "filler").with_prop("h", "500"),
        );
        app.add(
            "win",
            Component::new("view", Role::Panel, "view")
                .with_prop("layout", "column")
                .with_prop("h", "80")
                .with_prop("pad", "0")
                .with_prop("gap", "0")
                .with_prop("scroll_y", "40"),
        );
        app.add(
            "view",
            Component::new("r0", Role::Panel, "row").with_prop("h", "40"),
        );
        app.add(
            "view",
            Component::new("r1", Role::Panel, "row").with_prop("h", "40"),
        );
        let layout = hamesh_layout::compute(&app, 400.0, 600.0);
        (app, layout)
    }

    #[test]
    fn clipped_away_rows_cannot_be_clicked_but_partial_ones_can() {
        let (mut app, layout) = scrolled_list();
        // (10,470) is inside r0's rect but outside its paint clip:
        // the filler underneath wins, and owns no command.
        let r = click_human(&mut app, &layout, 10.0, 470.0);
        assert_eq!(r.hit_component.as_deref(), Some("filler"));
        assert!(!r.success);
        // (10,510) is r1's visible half: hit, but rows own no
        // commands (drivers interpret row identity instead).
        let r = click_human(&mut app, &layout, 10.0, 510.0);
        assert_eq!(r.hit_component.as_deref(), Some("r1"));
    }

    fn app_with_composer() -> (App, hamesh_widgets::Widgets, LayoutOutput) {
        use hamesh_core::{Component, Role};
        let mut app = App::new("win");
        app.add("win", Component::new("feed", Role::List, "feed"));
        app.add(
            "win",
            Component::new("composer", Role::TextInput, "composer").with_prop("h", "36"),
        );
        let w = hamesh_widgets::Widgets {
            composer_id: "composer".to_string(),
            feed_id: "feed".to_string(),
            feed_viewport: 400.0,
            focus: hamesh_widgets::Focus::rebuild(&app),
            ..Default::default()
        };
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        (app, w, layout)
    }

    #[test]
    fn typing_enters_text_and_submit_appends() {
        let (mut app, mut w, layout) = app_with_composer();
        let r = type_human(&mut app, &mut w, &layout, "composer", "hello");
        assert!(r.success, "{r:?}");
        assert_eq!(r.mechanism, Mechanism::HumanEquivalent);
        assert_eq!(w.composer.text, "hello");
        let k = key_human(&mut app, &mut w, &hamesh_widgets::Key::Enter);
        assert!(k.success, "{k:?}");
        assert_eq!(w.feed.messages.len(), 1);
        assert_eq!(w.composer.text, "");
    }

    #[test]
    fn covered_composer_blocks_typing() {
        use hamesh_core::{Component, Role};
        let (mut app, mut w, _) = app_with_composer();
        app.add(
            "win",
            Component::new("veil", Role::Overlay, "veil")
                .with_prop("overlay", "true")
                .with_prop("ox", "0")
                .with_prop("oy", "0")
                .with_prop("w", "800")
                .with_prop("h", "600"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let r = type_human(&mut app, &mut w, &layout, "composer", "nope");
        assert!(!r.success, "covered composer must block typing: {r:?}");
        assert_eq!(w.composer.text, "");
        // Direct op still works, labeled as such.
        let d = set_composer_text_direct(&mut app, &mut w, "nope");
        assert_eq!(d.mechanism, Mechanism::DirectOp);
        assert!(d.success);
    }

    #[test]
    fn tab_moves_focus_and_unfocused_typing_fails() {
        let (mut app, mut w, layout) = app_with_composer();
        // Focus starts on the feed; Tab reaches the composer.
        let t = key_human(&mut app, &mut w, &hamesh_widgets::Key::Tab);
        assert!(t.success);
        assert_eq!(w.focus.focused(), Some("composer"));
        let t = key_human(&mut app, &mut w, &hamesh_widgets::Key::Tab);
        assert!(t.success);
        assert_eq!(w.focus.focused(), Some("feed"));
        // Typing now targets nothing: key_human reports ignored.
        let k = key_human(&mut app, &mut w, &hamesh_widgets::Key::Char('x'));
        assert!(!k.success);
        assert_eq!(w.composer.text, "");
        let _ = layout;
    }
}
