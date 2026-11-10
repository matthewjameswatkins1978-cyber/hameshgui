use hamesh_core::{App, Component, ComponentId, Role};
use hamesh_inspect::{compact_map, detail, probe};
use hamesh_layout::{compute, compute_full};
use hamesh_scene::build as build_scene;
use hamesh_text::TextLayout;
use hamesh_widgets::{rebuild_feed_nodes, Widgets};
use std::time::Instant;

fn setup_fonts() {
    hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
}

// ----------------------------------------------------------------------------
// BAT-01: Cross-application contamination
// ----------------------------------------------------------------------------
#[test]
fn test_bat_01_cross_app_contamination() {
    setup_fonts();

    // App A: Dashboard with specific metrics and monospace labels
    let mut app_a = App::new("root");
    app_a.get_mut("root").unwrap().props.insert("layout".into(), "column".into());
    app_a.get_mut("root").unwrap().props.insert("pad".into(), "10".into());

    app_a.add(
        "root",
        Component::new("header", Role::Header, "Header")
            .with_prop("h", "40")
            .with_prop("pad", "5"),
    );
    app_a.add(
        "header",
        Component::new("text_1", Role::Text, "Text1")
            .with_prop("text", "USER ACCOUNT: ALICE_992")
            .with_prop("font_size", "14"),
    );
    app_a.add(
        "root",
        Component::new("btn_action", Role::Button, "Button")
            .with_prop("text", "TRANSFER FUNDS")
            .with_prop("h", "32"),
    );
    app_a.add(
        "root",
        Component::new("footer", Role::Status, "Footer")
            .with_prop("h", "24")
            .with_prop("text", "STATUS OK"),
    );

    // App B: System Alert with identical IDs but completely different content & geometry
    let mut app_b = App::new("root");
    app_b.get_mut("root").unwrap().props.insert("layout".into(), "row".into());
    app_b.get_mut("root").unwrap().props.insert("pad".into(), "30".into());

    app_b.add(
        "root",
        Component::new("header", Role::Header, "Header")
            .with_prop("h", "120")
            .with_prop("pad", "20"),
    );
    app_b.add(
        "header",
        Component::new("text_1", Role::Text, "Text1")
            .with_prop("text", "CRITICAL EMERGENCY SHUTDOWN")
            .with_prop("font_size", "32"),
    );
    app_b.add(
        "root",
        Component::new("btn_action", Role::Button, "Button")
            .with_prop("text", "ABORT ALL SYSTEMS")
            .with_prop("h", "64"),
    );
    app_b.add(
        "root",
        Component::new("footer", Role::Status, "Footer")
            .with_prop("h", "50")
            .with_prop("text", "DANGER"),
    );

    // Alternating execution: A -> B -> A -> B -> A
    // In every turn, compute (cached) MUST match compute_full (uncached) exactly.
    for turn in 0..5 {
        let (current_app, name) = if turn % 2 == 0 {
            (&app_a, "AppA")
        } else {
            (&app_b, "AppB")
        };

        let cached_layout = compute(current_app, 800.0, 600.0);
        let full_layout = compute_full(current_app, 800.0, 600.0);

        assert_eq!(
            cached_layout.nodes.len(),
            full_layout.nodes.len(),
            "Turn {} ({}): node count mismatch",
            turn,
            name
        );

        for (id, cached_node) in &cached_layout.nodes {
            let full_node = full_layout
                .nodes
                .get(id)
                .unwrap_or_else(|| panic!("Missing node {} in full layout on turn {}", id, turn));

            assert_eq!(
                cached_node.rect, full_node.rect,
                "Turn {} ({}): rect mismatch for node {}: cached {:?} vs full {:?}",
                turn, name, id, cached_node.rect, full_node.rect
            );
            assert_eq!(
                cached_node.effectively_visible, full_node.effectively_visible,
                "Turn {} ({}): effectively_visible mismatch for node {}",
                turn, name, id
            );
        }
    }
}

// ----------------------------------------------------------------------------
// BAT-02: Stale geometry under randomized mutations
// ----------------------------------------------------------------------------
#[test]
fn test_bat_02_stale_geometry() {
    setup_fonts();

    let mut app = App::new("root");
    app.get_mut("root").unwrap().props.insert("layout".into(), "column".into());

    let node_count = 20;
    for i in 0..node_count {
        let id = format!("item_{}", i);
        app.add(
            "root",
            Component::new(&id, Role::Panel, &id)
                .with_prop("text", &format!("Initial content {}", i))
                .with_prop("h", "24"),
        );
    }

    // Warm-up cache
    let _ = compute(&app, 800.0, 600.0);

    // Deterministic pseudo-random mutations over 60 rounds
    let mut lcg = 42u64;
    for round in 0..60 {
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        let target_idx = (lcg as usize) % node_count;
        let target_id = format!("item_{}", target_idx);

        let c = app.get_mut(&target_id).unwrap();

        // Mutate text length
        let text_len = (lcg >> 16) % 5;
        let new_text = match text_len {
            0 => "Short".to_string(),
            1 => "Medium length string that wraps across multiple layout lines".to_string(),
            2 => "A very long detailed string with numbers 123456789 and punctuation! Extended line measurement.".to_string(),
            3 => "".to_string(), // empty boundary
            _ => format!("Dynamic round {} text token {}", round, lcg),
        };
        c.props.insert("text".into(), new_text);

        // Mutate height / padding / visibility
        let kind = (lcg >> 24) % 4;
        match kind {
            0 => {
                c.props.insert("h".into(), format!("{}", 15 + ((lcg % 40) as u32)));
            }
            1 => {
                let p = (lcg % 15) as u32;
                c.props.insert("pad".into(), format!("{}", p));
            }
            2 => {
                c.visible = (lcg % 2) == 0;
            }
            _ => {
                if (lcg % 5) == 0 {
                    c.props.insert("overlay".into(), "true".into());
                    c.props.insert("ox".into(), "10".into());
                    c.props.insert("oy".into(), "10".into());
                } else {
                    c.props.remove("overlay");
                }
            }
        }

        // Mutate viewport size
        let vp_w = 600.0 + ((lcg % 800) as f32);
        let vp_h = 400.0 + ((lcg % 600) as f32);

        let cached = compute(&app, vp_w, vp_h);
        let full = compute_full(&app, vp_w, vp_h);

        assert_eq!(
            cached.nodes.len(),
            full.nodes.len(),
            "Round {}: node count mismatch",
            round
        );

        for (id, cn) in &cached.nodes {
            let fn_node = full
                .nodes
                .get(id)
                .unwrap_or_else(|| panic!("Round {}: node {} missing from full layout", round, id));

            assert_eq!(
                cn.rect, fn_node.rect,
                "Round {}: rect mismatch on node {}: cached {:?} vs full {:?}",
                round, id, cn.rect, fn_node.rect
            );
            assert_eq!(
                cn.effectively_visible, fn_node.effectively_visible,
                "Round {}: effectively_visible mismatch on node {}",
                round, id
            );
        }
    }
}

// ----------------------------------------------------------------------------
// BAT-03: Text corruption across complex scripts & boundaries
// ----------------------------------------------------------------------------
#[test]
fn test_bat_03_text_corruption() {
    setup_fonts();

    let pathological_cases = [
        // Combining marks / diacritics
        ("zalgo", "Z͔͑ͫ̓aͣͥ͛lͫgͮo̓͊̈ ͬtͭeͮxͫtͬ"),
        ("combining_accents", "e\u{0301}\u{0300}\u{0308}a\u{030a}"),
        // Multi-codepoint emoji sequences
        ("family_emoji", "👨‍👩‍👧‍👦"),
        ("kiss_emoji", "👩‍❤️‍💋‍👨"),
        ("flag_emoji", "🏳️‍🌈"),
        ("skin_tone_emoji", "👍🏽"),
        // RTL
        ("arabic", "مرحبا بالعالم كيف حالك"),
        ("hebrew", "שלום עולם מה נשמע"),
        // Bidirectional mixed
        ("bidi", "Hello 1234 עולם 5678 World"),
        // Boundaries
        ("empty", ""),
        ("single_space", " "),
        ("tabs_newlines", "\t\t\n\n   \n\t"),
        // Repeated text
        ("repeated_long", &"The quick brown fox jumps over the lazy dog. ".repeat(200)),
    ];

    for (label, text) in &pathological_cases {
        // Shape once (first pass)
        let tl1 = TextLayout::new(text, f32::INFINITY, 14.0, None);
        let w1 = tl1.width();
        let h1 = tl1.height();
        let lines1 = tl1.line_count();

        // Shape second time (hitting thread-local layout shape cache)
        let tl2 = TextLayout::new(text, f32::INFINITY, 14.0, None);
        let w2 = tl2.width();
        let h2 = tl2.height();
        let lines2 = tl2.line_count();

        assert_eq!(
            w1, w2,
            "Case '{}': cached width mismatch {} vs {}",
            label, w1, w2
        );
        assert_eq!(
            h1, h2,
            "Case '{}': cached height mismatch {} vs {}",
            label, h1, h2
        );
        assert_eq!(
            lines1, lines2,
            "Case '{}': cached lines count mismatch {} vs {}",
            label, lines1, lines2
        );

        // Bump font generation to force cache invalidation, then shape again
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let tl3 = TextLayout::new(text, f32::INFINITY, 14.0, None);
        assert_eq!(
            w1,
            tl3.width(),
            "Case '{}': post-bump width mismatch {} vs {}",
            label,
            w1,
            tl3.width()
        );
        assert_eq!(
            h1,
            tl3.height(),
            "Case '{}': post-bump height mismatch {} vs {}",
            label,
            h1,
            tl3.height()
        );
    }
}

// ----------------------------------------------------------------------------
// BAT-04: Cache invalidation on font generation & subtle style inputs
// ----------------------------------------------------------------------------
#[test]
fn test_bat_04_cache_invalidation() {
    setup_fonts();

    let mut app = App::new("root");
    app.get_mut("root").unwrap().props.insert("pad".into(), "10".into());

    app.add(
        "root",
        Component::new("item", Role::Panel, "Item")
            .with_prop("text", "Cache Invalidation Test")
            .with_prop("pad", "5"),
    );

    // Initial layout
    let layout1 = compute(&app, 500.0, 400.0);
    let rect1 = layout1.nodes.get("item").unwrap().rect.clone();

    // 1. Subtle style change: 1px padding
    {
        let item_mut = app.get_mut("item").unwrap();
        item_mut.props.insert("pad".into(), "6".into());
    }
    let layout2 = compute(&app, 500.0, 400.0);
    let full2 = compute_full(&app, 500.0, 400.0);
    assert_eq!(
        layout2.nodes.get("item").unwrap().rect,
        full2.nodes.get("item").unwrap().rect,
        "1px padding mutation: cached layout did not match compute_full"
    );

    // 2. Cycle-return trap: Return to original padding "5"
    {
        let item_mut = app.get_mut("item").unwrap();
        item_mut.props.insert("pad".into(), "5".into());
    }
    let layout3 = compute(&app, 500.0, 400.0);
    assert_eq!(
        layout3.nodes.get("item").unwrap().rect,
        rect1,
        "Cycle return to state 1: rect mismatch"
    );

    // 3. Font generation invalidation
    hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
    let layout4 = compute(&app, 500.0, 400.0);
    let full4 = compute_full(&app, 500.0, 400.0);
    assert_eq!(
        layout4.nodes.get("item").unwrap().rect,
        full4.nodes.get("item").unwrap().rect,
        "Post font-generation bump: cached layout did not match compute_full"
    );
}

// ----------------------------------------------------------------------------
// BAT-05: Structural mutations (insertions, removals, reparentings)
// ----------------------------------------------------------------------------
#[test]
fn test_bat_05_structural_mutation() {
    setup_fonts();

    let mut app = App::new("root");
    app.get_mut("root").unwrap().props.insert("layout".into(), "column".into());

    for i in 0..10 {
        let id = format!("node_{}", i);
        app.add(
            "root",
            Component::new(&id, Role::Button, &id)
                .with_prop("text", &format!("Button {}", i))
                .with_prop("h", "30"),
        );
    }

    // Baseline layout
    let _ = compute(&app, 800.0, 600.0);

    // Mutation 1: Insert 3 elements into middle of child list
    for k in 100..103 {
        let id = format!("node_{}", k);
        let c = Component::new(&id, Role::Button, &id)
            .with_prop("text", &format!("Mid-inserted {}", k))
            .with_prop("h", "35");
        app.components.insert(id.clone(), c);
        let root_mut = app.get_mut("root").unwrap();
        root_mut.children.insert(5, ComponentId::new(&id));
    }
    let layout_m1 = compute(&app, 800.0, 600.0);
    let full_m1 = compute_full(&app, 800.0, 600.0);
    assert_eq!(layout_m1.nodes.len(), full_m1.nodes.len());
    for (id, n) in &layout_m1.nodes {
        assert_eq!(n.rect, full_m1.nodes.get(id).unwrap().rect);
    }

    // Mutation 2: Remove alternate elements
    {
        let root_mut = app.get_mut("root").unwrap();
        let remaining: Vec<ComponentId> = root_mut
            .children
            .iter()
            .enumerate()
            .filter(|(idx, _)| idx % 2 == 0)
            .map(|(_, id)| id.clone())
            .collect();
        root_mut.children = remaining;
    }
    let layout_m2 = compute(&app, 800.0, 600.0);
    let full_m2 = compute_full(&app, 800.0, 600.0);
    assert_eq!(layout_m2.nodes.len(), full_m2.nodes.len());
    for (id, n) in &layout_m2.nodes {
        assert_eq!(n.rect, full_m2.nodes.get(id).unwrap().rect);
    }

    // Mutation 3: Reverse children order
    {
        let root_mut = app.get_mut("root").unwrap();
        root_mut.children.reverse();
    }
    let layout_m3 = compute(&app, 800.0, 600.0);
    let full_m3 = compute_full(&app, 800.0, 600.0);
    for (id, n) in &layout_m3.nodes {
        assert_eq!(n.rect, full_m3.nodes.get(id).unwrap().rect);
    }
}

// ----------------------------------------------------------------------------
// BAT-06: Long-running stress test (latency stability, memory boundaries)
// ----------------------------------------------------------------------------
#[test]
fn test_bat_06_long_running_stress() {
    setup_fonts();

    let mut app = App::new("root");
    let mut w = Widgets {
        feed_id: "feed".to_string(),
        feed_viewport: 500.0,
        ..Default::default()
    };
    let stream_idx = w.feed.push_live("agent");

    let iters = 500;
    let mut early_latencies = Vec::new();
    let mut late_latencies = Vec::new();

    for i in 0..iters {
        w.feed.append_to_message(stream_idx, " streaming test token");
        w.feed.scroll_by(5.0, 500.0);
        rebuild_feed_nodes(&mut app, &mut w, "feed", 780.0);

        let t0 = Instant::now();
        let layout = compute(&app, 800.0, 600.0);
        let scene = build_scene(&app, &layout, (i + 1) as u64);
        let _ = hamesh_render::render(&scene);
        let elapsed = t0.elapsed().as_micros() as f64;

        if i >= 50 && i < 150 {
            early_latencies.push(elapsed);
        } else if i >= 400 && i < 500 {
            late_latencies.push(elapsed);
        }
    }

    let early_avg: f64 = early_latencies.iter().sum::<f64>() / early_latencies.len() as f64;
    let late_avg: f64 = late_latencies.iter().sum::<f64>() / late_latencies.len() as f64;

    println!(
        "Stress test latencies: early avg = {:.2} us, late avg = {:.2} us",
        early_avg, late_avg
    );

    // Verify no catastrophic latency cliff (late avg should not exceed 3x early avg)
    assert!(
        late_avg < early_avg * 3.0,
        "Latency cliff observed: early avg {:.2} us vs late avg {:.2} us",
        early_avg,
        late_avg
    );
}

// ----------------------------------------------------------------------------
// BAT-07: Eye and Hand integrity
// ----------------------------------------------------------------------------
#[test]
fn test_bat_07_eye_and_hand_integrity() {
    setup_fonts();

    let mut app = App::new("root");
    app.get_mut("root").unwrap().props.insert("pad".into(), "0".into());

    app.add(
        "root",
        Component::new("action_btn", Role::Button, "Button")
            .with_prop("w", "150")
            .with_prop("h", "40")
            .with_prop("text", "Click Me"),
    );

    // Compute layout & scene
    let layout = compute(&app, 800.0, 600.0);
    let scene = build_scene(&app, &layout, 1);

    // Probe right inside button bounds: (50.0, 20.0)
    let p = probe(&app, &layout, &scene, 50.0, 20.0);
    assert_eq!(
        p.topmost_candidate.as_deref(),
        Some("action_btn"),
        "Eye probe did not find button at (50, 20)"
    );

    // Probe outside button: (300.0, 300.0)
    let p_outside = probe(&app, &layout, &scene, 300.0, 300.0);
    assert_eq!(
        p_outside.topmost_candidate.as_deref(),
        Some("root"),
        "Eye probe outside button did not resolve to root"
    );

    // Inspect detail
    let d = detail(&app, &layout, Some(&scene), "action_btn").expect("detail for action_btn");
    assert_eq!(d.id, "action_btn");
    assert!(d.rect.is_some());
    let rect = d.rect.unwrap();
    assert_eq!(rect.w, 150.0);
    assert_eq!(rect.h, 40.0);

    // Inspect compact map
    let cm = compact_map(&app, &layout, &scene);
    assert!(cm.nodes.iter().any(|n| n.id == "action_btn"));
}

// ----------------------------------------------------------------------------
// BAT-08: Performance claim falsification (worst-case locality)
// ----------------------------------------------------------------------------
#[test]
fn test_bat_08_performance_claim_falsification() {
    setup_fonts();

    let mut app = App::new("root");
    for i in 0..50 {
        let id = format!("item_{}", i);
        app.add(
            "root",
            Component::new(&id, Role::Panel, &id)
                .with_prop("text", &format!("Item text {}", i))
                .with_prop("h", "20"),
        );
    }

    // Initial warm
    let _ = compute(&app, 800.0, 600.0);

    // Benchmark: Worst-case locality (every single node is mutated every frame)
    let iters = 50;
    let mut incremental_worst_times = Vec::new();
    let mut full_times = Vec::new();

    for frame in 0..iters {
        // Dirty every node
        for i in 0..50 {
            let id = format!("item_{}", i);
            let c = app.get_mut(&id).unwrap();
            c.props.insert("text".into(), format!("Frame {} text {}", frame, i));
            let p = (frame + i) % 4;
            c.props.insert("pad".into(), format!("{}", p));
        }

        let t0 = Instant::now();
        let _ = compute(&app, 800.0, 600.0);
        incremental_worst_times.push(t0.elapsed().as_micros() as f64);

        let t1 = Instant::now();
        let _ = compute_full(&app, 800.0, 600.0);
        full_times.push(t1.elapsed().as_micros() as f64);
    }

    let avg_inc: f64 = incremental_worst_times.iter().sum::<f64>() / iters as f64;
    let avg_full: f64 = full_times.iter().sum::<f64>() / iters as f64;

    println!(
        "Worst-case 100%-dirty layout: incremental avg = {:.2} us, compute_full avg = {:.2} us",
        avg_inc, avg_full
    );

    // Incremental layout should not be pathologically worse than full rebuild (overhead bound <= 35%)
    assert!(
        avg_inc <= avg_full * 1.35,
        "Incremental layout suffered pathological overhead on 100% dirty frames: inc {:.2} us vs full {:.2} us",
        avg_inc,
        avg_full
    );
}

// ----------------------------------------------------------------------------
// BAT-09: Interruption and recovery
// ----------------------------------------------------------------------------
#[test]
fn test_bat_09_interruption_and_recovery() {
    setup_fonts();

    let mut app = App::new("root");
    app.add(
        "root",
        Component::new("node_1", Role::Text, "Node1")
            .with_prop("text", "Healthy initial state"),
    );

    // Frame 1: Normal compute
    let l1 = compute(&app, 600.0, 400.0);
    assert_eq!(l1.nodes.len(), 2);

    // Frame 2: Simulated interrupted work (a panic caught within a child task/scope)
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut app_aborted = App::new("root");
        app_aborted.add("root", Component::new("poison", Role::Text, "Poison"));
        let _ = compute(&app_aborted, 600.0, 400.0);
        panic!("Simulated mid-frame worker abort!");
    }));
    assert!(result.is_err(), "Panic must be caught");

    // Frame 3: Recovery. Subsequent normal compute must succeed with clean state
    let c1_mut = app.get_mut("node_1").unwrap();
    c1_mut.props.insert("text".into(), "Post-recovery state".into());

    let l3 = compute(&app, 600.0, 400.0);
    let l3_full = compute_full(&app, 600.0, 400.0);

    assert_eq!(
        l3.nodes.len(),
        l3_full.nodes.len(),
        "Post-abort recovery: node count mismatch"
    );
    for (id, n) in &l3.nodes {
        assert_eq!(
            n.rect,
            l3_full.nodes.get(id).unwrap().rect,
            "Post-abort recovery: rect mismatch for {}",
            id
        );
    }
}
