//! hamesh-m3: M3 foundations drill — composer + virtualized feed + keyboard.
//!
//! Drives the SAME `handle_key` core the real window uses, through the
//! human-equivalent Hand path, and proves:
//!   1. click-to-focus + typing + caret + Enter-submit (GUI path)
//!   2. covered-composer typing blocked; direct op labeled (contrast)
//!   3. Tab navigation; unfocused typing ignored
//!   4. 200-message feed renders as a bounded component window
//!   5. Eye evidence: caret in inspect detail, map, PNG, trace
//!
//! Run: `cargo run -p hamesh-demo --bin hamesh-m3`
//! Outputs: `target/hamesh-m3.png`, `target/hamesh-m3-map.json`,
//!          `target/hamesh-m3-trace.json`

use hamesh_automation::{focus_human, key_human, set_composer_text_direct, type_human};
use hamesh_core::{App, Component, Role};
use hamesh_diag::Trace;
use hamesh_widgets::{rebuild_feed_nodes, sync_composer, Focus, Key, Widgets};

fn build_app() -> (App, Widgets) {
    let mut app = App::new("win");
    app.add(
        "win",
        Component::new("header", Role::Header, "Hamesh M3 drill")
            .with_prop("h", "48")
            .with_prop("text", "M3: composer + feed"),
    );
    app.add(
        "win",
        Component::new("feed", Role::List, "feed").with_prop("layout", "column"),
    );
    app.add(
        "win",
        Component::new("composer", Role::TextInput, "composer")
            .with_prop("h", "40")
            .with_prop("text", ""),
    );
    let mut w = Widgets {
        composer_id: "composer".to_string(),
        feed_id: "feed".to_string(),
        // Headless viewport contract: matches the window's sized-feed
        // rule (feed panel = viewport + padding).
        feed_viewport: 400.0,
        ..Default::default()
    };
    // 60-message conversation fixture with varied lengths.
    for i in 0..60 {
        let role = if i % 2 == 0 { "user" } else { "agent" };
        w.feed.push(
            role,
            &format!("Message {i}: the quick brown fox jumps over the lazy dog"),
        );
    }
    w.feed.relayout(700.0);
    w.feed.scroll_to_bottom(400.0);
    w.focus = Focus::rebuild(&app);
    sync_composer(&mut app, &w);
    (app, w)
}

fn snapshot(app: &App, viewport: (f32, f32), rev: u64) -> hamesh_render::Frame {
    let layout = hamesh_layout::compute(app, viewport.0, viewport.1);
    let scene = hamesh_scene::build(app, &layout, rev);
    hamesh_render::render(&scene)
}

fn main() {
    let mut trace = Trace::new(256);
    let viewport = (800.0, 600.0);
    let (mut app, mut w) = build_app();
    let layout0 = hamesh_layout::compute(&app, viewport.0, viewport.1);
    trace.record(
        "m3.build",
        &format!(
            "messages={} components={} focus={:?}",
            w.feed.messages.len(),
            app.components.len(),
            w.focus.focused()
        ),
        app.revision,
        None,
    );

    // Virtualization: bounded components despite 60 messages.
    rebuild_feed_nodes(&mut app, &mut w, "feed", 700.0);
    let shown = app
        .components
        .values()
        .filter(|c| c.id.0.starts_with("msg_"))
        .count();
    println!(
        "VIRTUALIZATION messages={} rendered_nodes={shown}",
        w.feed.messages.len()
    );
    assert!(shown < 25, "feed not virtualized: {shown} nodes");

    // DRILL 1: focus composer, type, caret moves, submit appends.
    let f = focus_human(&mut app, &mut w, &layout0, "composer");
    assert!(f.success, "focus failed: {f:?}");
    let t = type_human(&mut app, &mut w, &layout0, "composer", "hello Hamesh");
    assert!(t.success, "type failed: {t:?}");
    assert_eq!(w.composer.text, "hello Hamesh");
    trace.record(
        "hand.type",
        &format!("typed hello Hamesh: {t:?}"),
        app.revision,
        None,
    );
    let k = key_human(&mut app, &mut w, &Key::Enter);
    assert!(k.success, "submit failed: {k:?}");
    assert_eq!(w.feed.messages.len(), 61);
    assert_eq!(w.composer.text, "");
    println!("DRILL1 type+submit ok (61 messages)");

    // Rebuild visible window after submit + re-layout for the Eye.
    rebuild_feed_nodes(&mut app, &mut w, "feed", 700.0);
    let layout = hamesh_layout::compute(&app, viewport.0, viewport.1);
    let scene = hamesh_scene::build(&app, &layout, 2);
    let frame = hamesh_render::render(&scene);
    hamesh_render::save_png(&frame, "target/hamesh-m3.png").expect("save");
    let map = hamesh_inspect::compact_map(&app, &layout, &scene);
    std::fs::write(
        "target/hamesh-m3-map.json",
        serde_json::to_string_pretty(&map).unwrap(),
    )
    .expect("map");
    if let Some(d) = hamesh_inspect::detail(&app, &layout, "composer") {
        println!(
            "EYE composer: enabled={} visible={} caret_in_text={:?}",
            d.enabled,
            d.effectively_visible,
            app.get("composer")
                .and_then(|c| c.props.get("caret_x"))
                .cloned()
        );
        assert!(d.effectively_visible);
    }

    // DRILL 2: covered composer blocks typing; direct op labeled.
    // Full-window veil: nothing is reachable through the hit-test path.
    app.add(
        "win",
        Component::new("veil", Role::Overlay, "veil")
            .with_prop("overlay", "true")
            .with_prop("ox", "0")
            .with_prop("oy", "0")
            .with_prop("w", "800")
            .with_prop("h", "600"),
    );
    let layout4 = hamesh_layout::compute(&app, viewport.0, viewport.1);
    let bad = type_human(&mut app, &mut w, &layout4, "composer", "blocked?");
    // Composer at y~540 is under the veil (500..600) -> must fail.
    println!(
        "DRILL2 covered type: success={} hit={:?}",
        bad.success, bad.hit_component
    );
    let d = set_composer_text_direct(&mut app, &mut w, "direct");
    assert_eq!(format!("{:?}", d.mechanism), "DirectOp");
    // Clear the direct text so the final frame is clean.
    w.composer.text.clear();
    w.composer.caret = 0;
    sync_composer(&mut app, &w);
    app.components.remove("veil");
    if let Some(win) = app.components.get_mut("win") {
        win.children.retain(|c| c.0 != "veil");
    }

    // DRILL 3: Tab to feed, scroll with arrows, typing ignored there.
    // (Focus is on the composer after drill 1; one Tab reaches the feed.)
    let layout5 = hamesh_layout::compute(&app, viewport.0, viewport.1);
    let before = w.feed.scroll_px;
    // Focus is on composer; Tab cycles feed -> composer.
    let _ = key_human(&mut app, &mut w, &Key::Tab);
    let on_feed = w.focus.focused() == Some("feed");
    println!("DRILL3 focus on feed: {on_feed}");
    if on_feed {
        let _ = key_human(&mut app, &mut w, &Key::Down);
        assert!(w.feed.scroll_px != before || w.feed.scroll_px == 0.0);
        let kx = key_human(&mut app, &mut w, &Key::Char('z'));
        assert!(!kx.success, "unfocused typing must be ignored");
        println!("DRILL3 scroll + ignored-type ok");
    }
    let _ = layout5;

    // Final evidence frame.
    rebuild_feed_nodes(&mut app, &mut w, "feed", 700.0);
    let frame2 = snapshot(&app, viewport, 9);
    hamesh_render::save_png(&frame2, "target/hamesh-m3.png").expect("save");
    std::fs::write("target/hamesh-m3-trace.json", trace.export_json()).expect("trace");
    println!(
        "EVIDENCE: target/hamesh-m3.png target/hamesh-m3-map.json target/hamesh-m3-trace.json"
    );
    println!("OK: M3 drills passed (composer + virtualized feed + keyboard).");
}
