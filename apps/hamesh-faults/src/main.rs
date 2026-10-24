//! hamesh-faults: standing broken-control drills (repeatable regression).
//!
//! Each drill injects ONE realistic fault into an otherwise healthy app,
//! then runs the full agent loop WITHOUT human help:
//!   DISCOVER (compact map) -> REPRODUCE (failed interaction + evidence)
//!   -> LOCATE (detail diagnostics + hit-test truth) -> REPAIR (the same
//!   tree mutation a source edit would compile to) -> VERIFY (interaction
//!   succeeds, state changed, evidence written).
//!
//! Exit code is non-zero on the first unverified drill, so CI can run it.
//! The external (separate-process) version of each drill is the documented
//! CLI transcript in docs/decisions/014; the headless version here proves
//! the same Eye/Hand semantics the CLI drives.
//!
//! Faults:
//!   covered-submit — transparent overlay intercepts the submit click.
//!   clipped-composer — composer wider than the window: present, focused,
//!                     but its caret end is unreachable and clipped.
//!   dead-submit — submit button owns no command: clicks land, nothing
//!                 happens (enabled but purposeless).

use hamesh_automation::{click_component_human, type_human};
use hamesh_core::{App, Component, Role};
use hamesh_widgets::{handle_key, sync_composer, Focus, Key, Widgets};

fn base_app() -> (App, Widgets) {
    let mut app = App::new("win");
    app.add(
        "win",
        Component::new("panel", Role::Panel, "main").with_prop("layout", "column"),
    );
    app.add(
        "panel",
        Component::new("composer", Role::TextInput, "composer").with_prop("h", "36"),
    );
    app.add(
        "panel",
        Component::new("btn", Role::Button, "Submit").with_prop("h", "36"),
    );
    app.register_command("submit", "btn");
    app.set_state("submitted", "0");
    let mut w = Widgets {
        composer_id: "composer".to_string(),
        feed_id: "feed".to_string(),
        feed_viewport: 400.0,
        ..Default::default()
    };
    w.focus = Focus::rebuild(&app);
    sync_composer(&mut app, &w);
    (app, w)
}

fn layout(app: &App) -> hamesh_layout::LayoutOutput {
    hamesh_layout::compute(app, 800.0, 600.0)
}

fn check(cond: bool, what: &str) {
    if cond {
        println!("  ok: {what}");
    } else {
        eprintln!("  FAIL: {what}");
        std::process::exit(1);
    }
}

fn main() {
    println!("== F1 covered-submit: overlay intercepts the submit click ==");
    {
        let (mut app, _) = base_app();
        app.add(
            "win",
            Component::new("veil", Role::Overlay, "veil")
                .with_prop("overlay", "true")
                .with_prop("ox", "0")
                .with_prop("oy", "0")
                .with_prop("w", "800")
                .with_prop("h", "600"),
        );
        let l = layout(&app);
        // DISCOVER: the map shows the veil on top.
        let map = hamesh_inspect::compact_map(&app, &l, &hamesh_scene::build(&app, &l, 1));
        let top = map.nodes.iter().max_by_key(|n| n.z).unwrap();
        println!("  discover: topmost node = {} z={}", top.id, top.z);
        check(top.id == "veil", "veil is the topmost hit-test target");
        // REPRODUCE: aimed at btn, hit veil.
        let r = click_component_human(&mut app, &l, "btn");
        check(!r.success, "click must fail (intercepted)");
        check(
            r.hit_component.as_deref() == Some("veil"),
            &format!("hit must be veil, got {:?}", r.hit_component),
        );
        check(
            r.notes.iter().any(|n| n.contains("intercepted")),
            "diagnostic names the interception",
        );
        // LOCATE is the same evidence: hit != aim + overlay role at z=100.
        // REPAIR: remove the veil (the code change this stands in for).
        app.components.remove("veil");
        if let Some(win) = app.get_mut("win") {
            win.children.retain(|c| c.0 != "veil");
        }
        let l2 = layout(&app);
        // VERIFY: the click now lands and runs the command.
        let r2 = click_component_human(&mut app, &l2, "btn");
        check(
            r2.success && r2.hit_component.as_deref() == Some("btn"),
            "click lands after repair",
        );
    }

    println!("== F2 clipped-composer: present but half off-window ==");
    {
        let (mut app, _w) = base_app();
        // Widen the composer far beyond the viewport.
        if let Some(c) = app.get_mut("composer") {
            c.props.insert("w".to_string(), "2000".to_string());
        }
        let l = layout(&app);
        let d = hamesh_inspect::detail(&app, &l, None, "composer").expect("detail");
        println!(
            "  discover: clipped={} rect={:?}",
            d.clipped_by_parent, d.rect
        );
        check(
            d.clipped_by_parent,
            "composer must report clipped_by_parent",
        );
        check(
            d.diagnostics.iter().any(|x| x.contains("clipped")),
            "diagnostic names the clipping",
        );
        // REPAIR: constrain to the window.
        if let Some(c) = app.get_mut("composer") {
            c.props.insert("w".to_string(), "700".to_string());
        }
        let l2 = layout(&app);
        let d2 = hamesh_inspect::detail(&app, &l2, None, "composer").expect("detail");
        check(!d2.clipped_by_parent, "clipping cleared after repair");
        check(d2.diagnostics.is_empty(), "no diagnostics remain");
    }

    println!("== F3 dead-submit: enabled button, no command ==");
    {
        let (mut app, mut w) = base_app();
        // Break the command binding (the defect).
        app.commands.clear();
        app.bump();
        let l = layout(&app);
        // REPRODUCE: click lands cleanly but nothing happens.
        let r = click_component_human(&mut app, &l, "btn");
        println!(
            "  discover: hit={:?} success={}",
            r.hit_component, r.success
        );
        check(!r.success, "click must report no-effect");
        check(
            app.state.get("submitted").map(|s| s.as_str()) != Some("1"),
            "state must be unchanged by the dead click",
        );
        // LOCATE: hit the right target, but it owns no command.
        check(
            r.notes.iter().any(|n| n.contains("no command")),
            "diagnostic names the missing command",
        );
        // REPAIR: rebind the command.
        app.register_command("submit", "btn");
        let l2 = layout(&app);
        // VERIFY: the same click now runs the command (revision moves).
        let before = app.revision;
        let r2 = click_component_human(&mut app, &l2, "btn");
        check(
            r2.success && r2.hit_component.as_deref() == Some("btn"),
            "click lands after repair",
        );
        check(app.revision > before, "state advanced after repair");
        // The typed path is unaffected by the button fault.
        let t = type_human(&mut app, &mut w, &l2, "composer", "hi");
        check(t.success, "typing works after repair context");
        let k = handle_key(&mut app, &mut w, &Key::Enter);
        check(k.contains("submitted"), "Enter submits after repair");
        check(w.feed.messages.len() == 1, "message landed in the feed");
    }

    println!("OK: all fault drills diagnosed, repaired and verified.");
}
