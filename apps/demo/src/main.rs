//! hamesh-demo: first vertical slice (M1 + M2 headless).
//!
//! Builds a sample window tree, computes layout, builds a scene, renders a
//! CPU frame to PNG, prints a compact inspection map, then runs the
//! Eye-and-Hand fault drills:
//!   1. healthy button click (human-equivalent) -> counter increments
//!   2. disabled button -> human click blocked with evidence
//!   3. overlay intercept -> aimed at button, hit veil
//!   4. direct op -> labeled as DirectOp, never a GUI proof
//!
//! Run: `cargo run -p hamesh-demo`
//! Outputs: `target/hamesh-frame.png`, `target/hamesh-map.json`,
//!          `target/hamesh-trace.json`

use hamesh_automation::{click_component_human, invoke_direct};
use hamesh_core::{App, Component, Role};
use hamesh_diag::Trace;

fn sample_app() -> App {
    let mut app = App::new("win");
    app.add(
        "win",
        Component::new("header", Role::Header, "Hamesh demo")
            .with_prop("h", "48")
            .with_prop("text", "Hamesh — the Eye and the Hand"),
    );
    app.add(
        "win",
        Component::new("panel", Role::Panel, "main").with_prop("layout", "column"),
    );
    app.add(
        "panel",
        Component::new("counter_label", Role::Text, "counter label")
            .with_prop("h", "28")
            .with_prop("text", "Counter: 0"),
    );
    app.add(
        "panel",
        Component::new("btn_inc", Role::Button, "Increment primary")
            .with_prop("h", "40")
            .with_prop("text", "Increment"),
    );
    app.add(
        "panel",
        Component::new("btn_reset", Role::Button, "Reset")
            .with_prop("h", "36")
            .with_prop("text", "Reset"),
    );
    app.add(
        "panel",
        Component::new("status", Role::Status, "status")
            .with_prop("h", "28")
            .with_prop("text", "ready"),
    );
    app.register_command("increment", "btn_inc");
    app.register_command("reset", "btn_reset");
    app.set_state("counter", "0");
    app
}

fn sync_label(app: &mut App) {
    let c = app.state.get("counter").cloned().unwrap_or_default();
    if let Some(l) = app.get_mut("counter_label") {
        l.props.insert("text".to_string(), format!("Counter: {c}"));
    }
    if let Some(s) = app.get_mut("status") {
        s.props.insert("text".to_string(), format!("counter={c}"));
    }
}

fn main() {
    let mut trace = Trace::new(256);
    let viewport = (800.0, 600.0);

    let mut app = sample_app();
    sync_label(&mut app);
    trace.record(
        "app.build",
        &format!(
            "revision={} components={}",
            app.revision,
            app.components.len()
        ),
        app.revision,
        None,
    );

    // Layout -> scene -> frame.
    let layout = hamesh_layout::compute(&app, viewport.0, viewport.1);
    let scene = hamesh_scene::build(&app, &layout, 1);
    let frame = hamesh_render::render(&scene);
    let png_path = "target/hamesh-frame.png";
    hamesh_render::save_png(&frame, png_path).expect("save png");
    trace.record(
        "frame.render",
        &format!(
            "scene_rev={} app_rev={} {}x{} -> {png_path}",
            frame.frame_ref.scene_revision,
            frame.frame_ref.app_revision,
            frame.frame_ref.width,
            frame.frame_ref.height
        ),
        app.revision,
        Some(png_path.to_string()),
    );

    // Eye: compact map.
    let map = hamesh_inspect::compact_map(&app, &layout, &scene);
    std::fs::write(
        "target/hamesh-map.json",
        serde_json::to_string_pretty(&map).unwrap(),
    )
    .expect("write map");
    println!(
        "MAP app_rev={} scene_rev={} nodes={} png={png_path}",
        map.app_revision,
        map.scene_revision,
        map.nodes.len()
    );
    for n in &map.nodes {
        println!(
            "  {:12} {:9} vis={} clip={} en={} z={:3} ({:.0},{:.0} {:.0}x{:.0}) {}",
            n.id,
            n.role,
            n.visible,
            n.clipped,
            n.enabled,
            n.z,
            n.rect.x,
            n.rect.y,
            n.rect.w,
            n.rect.h,
            n.name
        );
    }

    // Hand drill 1: healthy human-equivalent click.
    let r1 = click_component_human(&mut app, &layout, "btn_inc");
    sync_label(&mut app);
    trace.record(
        "hand.click",
        &format!(
            "target=btn_inc success={} hit={:?}",
            r1.success, r1.hit_component
        ),
        app.revision,
        Some(png_path.to_string()),
    );
    println!(
        "DRILL1 human click btn_inc: success={} counter={:?} notes={:?}",
        r1.success,
        app.state.get("counter"),
        r1.notes
    );
    assert!(r1.success && app.state.get("counter").unwrap() == "1");

    // Re-layout after state change (label text changed; geometry stable).
    let layout2 = hamesh_layout::compute(&app, viewport.0, viewport.1);
    let scene2 = hamesh_scene::build(&app, &layout2, 2);
    let frame2 = hamesh_render::render(&scene2);
    hamesh_render::save_png(&frame2, "target/hamesh-frame-2.png").expect("save png2");

    // Hand drill 2: disabled button blocks human path.
    app.get_mut("btn_reset").unwrap().enabled = false;
    let layout3 = hamesh_layout::compute(&app, viewport.0, viewport.1);
    let r2 = click_component_human(&mut app, &layout3, "btn_reset");
    trace.record(
        "hand.click_blocked",
        &format!(
            "target=btn_reset success={} notes={:?}",
            r2.success, r2.notes
        ),
        app.revision,
        None,
    );
    println!(
        "DRILL2 disabled click btn_reset: success={} notes={:?}",
        r2.success, r2.notes
    );
    assert!(!r2.success);

    // Hand drill 3: overlay intercept.
    app.add(
        "win",
        Component::new("veil", Role::Overlay, "debug veil")
            .with_prop("overlay", "true")
            .with_prop("ox", "0")
            .with_prop("oy", "0")
            .with_prop("w", "800")
            .with_prop("h", "600"),
    );
    let layout4 = hamesh_layout::compute(&app, viewport.0, viewport.1);
    let r3 = click_component_human(&mut app, &layout4, "btn_inc");
    trace.record(
        "hand.intercept",
        &format!(
            "aimed=btn_inc hit={:?} success={} notes={:?}",
            r3.hit_component, r3.success, r3.notes
        ),
        app.revision,
        None,
    );
    println!(
        "DRILL3 overlay intercept: aimed=btn_inc hit={:?} success={} notes={:?}",
        r3.hit_component, r3.success, r3.notes
    );
    assert!(!r3.success);

    // Direct op still works but is labeled DirectOp (not GUI proof).
    let r4 = invoke_direct(&mut app, "reset");
    println!(
        "DRILL4 direct invoke reset: success={} mechanism={:?} counter={:?}",
        r4.success,
        r4.mechanism,
        app.state.get("counter")
    );
    assert!(r4.success);

    // Eye detail on the button proves the framework can explain itself.
    let layout5 = hamesh_layout::compute(&app, viewport.0, viewport.1);
    if let Some(d) = hamesh_inspect::detail(&app, &layout5, None, "btn_inc") {
        println!(
            "DETAIL btn_inc: enabled={} visible={} clipped={} rect={:?} diag={:?}",
            d.enabled, d.effectively_visible, d.clipped_by_parent, d.rect, d.diagnostics
        );
    }

    std::fs::write("target/hamesh-trace.json", trace.export_json()).expect("write trace");
    println!("EVIDENCE: {png_path} target/hamesh-frame-2.png target/hamesh-map.json target/hamesh-trace.json");
    println!("OK: M1 render + M2 eye/hand drills passed (headless CPU path).");
}
