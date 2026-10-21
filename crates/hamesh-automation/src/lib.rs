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

/// Human-equivalent click at a point: hit-test, then apply the target's
/// command if the hit target is enabled and visible.
pub fn click_human(app: &mut App, layout: &LayoutOutput, x: f32, y: f32) -> AutomationResult {
    let before = app.revision;
    let mut notes = Vec::new();
    let hit = hamesh_layout::hit_test(app, layout, x, y);
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
}
