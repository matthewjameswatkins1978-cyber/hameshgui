//! hamesh-035-lab: the gallery proof for reusable selection controls.
//!
//! A creative-tool workspace (poster layers + installed-font picker +
//! responsive inspector) built ONLY from Blocks composers over
//! driver-owned widget state. It answers the 035 question: how much
//! custom layout and interaction code does an agent still write?
//! Answer, counted at the bottom of this file: one composer call per
//! control, one hit-test match per pointer intent, one key match per
//! keyboard intent. No row windows, no reveal math, no filtering, no
//! commit semantics, no popup measuring anywhere in this file.
//!
//! Headless on purpose: compose, interact through the same intents a
//! native loop would forward, lay out, render, then ask the Eye.
//! Run: `cargo run -p hamesh-035-lab -- target/035`

use hamesh_gallery::{hover, key, press, refresh, wheel, Gallery, ROW_H};
use hamesh_widgets::Key;

/// Chrome rects that must not move when the popup opens or closes.
fn snapshot_chrome(layout: &hamesh_layout::LayoutOutput) -> Vec<(String, (f32, f32, f32, f32))> {
    [
        "canvas",
        "layers",
        "layers-view",
        "rail",
        "side",
        "header",
        "footer",
        "fonts-field",
    ]
    .iter()
    .filter_map(|id| {
        layout
            .nodes
            .get(*id)
            .map(|n| (id.to_string(), (n.rect.x, n.rect.y, n.rect.w, n.rect.h)))
    })
    .collect()
}

fn center(scene: &hamesh_scene::Scene, id: &str) -> Option<(f32, f32)> {
    scene
        .nodes
        .iter()
        .find(|n| n.id == id)
        .map(|n| (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0))
}

/// Clipped nodes must all be scrolled edge rows — the honest,
/// designed-for cut. Anything else is a layout defect.
fn assert_only_edge_rows_clip(report: &hamesh_inspect::VisualReport, max: usize, step: &str) {
    // A clipped row flags itself plus its icon and label leaves;
    // count rows, not nodes.
    let mut rows: Vec<&str> = report
        .clipped_content
        .iter()
        .map(|id| {
            id.strip_suffix("-icon")
                .or_else(|| id.strip_suffix("-label"))
                .unwrap_or(id)
        })
        .collect();
    rows.sort();
    rows.dedup();
    assert!(
        rows.iter()
            .all(|id| id.starts_with("layers-row-") || id.starts_with("fonts-opt-")),
        "{step}: only scrolled edge rows may clip: {:?}",
        report.clipped_content
    );
    assert!(
        rows.len() <= max,
        "{step}: at most {max} partial edge rows: {:?}",
        rows
    );
    assert!(
        report.off_viewport.is_empty(),
        "{step}: nothing may leave the viewport"
    );
    // A floating popup legitimately covers content beneath it (it
    // paints on the z=100 overlay layer, above everything). Only
    // same-layer overlaps are defects.
    let bad: Vec<_> = report
        .overlaps
        .iter()
        .filter(|o| !(o.over.starts_with("fonts-popup") || o.over.starts_with("fonts-opt-")))
        .collect();
    assert!(bad.is_empty(), "{step}: overlaps: {bad:?}");
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/035".to_string());
    std::fs::create_dir_all(&out).expect("out dir");

    let mut summary = Vec::new();
    for (w, h) in [(1440u32, 900u32), (1100, 720), (820, 560)] {
        let t0 = std::time::Instant::now();
        let mut g = Gallery::new();
        let label_names: Vec<String> = g.layer_rows.iter().map(|r| r.label.clone()).collect();

        // 1. Initial frame. The bottom partial row clips by design.
        let mut rev = 1u64;
        let (mut app, mut layout) = refresh(&mut g, w as f32, h as f32);
        let mut scene = hamesh_scene::build(&app, &layout, rev);
        let mut frame = hamesh_render::render(&scene);
        hamesh_render::save_png(&frame, &format!("{out}/gal-{w}x{h}-1-initial.png")).expect("png");
        let rep1 = hamesh_inspect::visual_report(&scene);
        assert_only_edge_rows_clip(&rep1, 1, "initial");
        let closed_chrome = snapshot_chrome(&layout);

        // 2. Pointer: select a row visible at every size, hover its
        // neighbour. Selection stays put while hover previews.
        let (cx, cy) = center(&scene, "layers-row-5").expect("row 5 visible");
        let note = press(&mut g, &app, &scene, cx, cy);
        assert_eq!(g.layers.selected(), Some(5), "{note}");
        hover(&mut g, &scene, cx, cy + ROW_H + 4.0, &app);
        assert_eq!(g.layers.hover(), Some(6));
        assert_eq!(g.layers.selected(), Some(5));

        // 3. Wheel: five rows down the list.
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        let scrolled = wheel(&mut g, &layout, cx, cy, -160.0);
        assert!(scrolled && g.layers.scroll() == 160.0);
        scene = hamesh_scene::build(&app, &layout, rev);
        frame = hamesh_render::render(&scene);
        hamesh_render::save_png(&frame, &format!("{out}/gal-{w}x{h}-2-scrolled.png")).expect("png");
        let rep2 = hamesh_inspect::visual_report(&scene);
        assert_only_edge_rows_clip(&rep2, 2, "scrolled");

        // 4. Keyboard on the list: move, then type ahead.
        assert!(key(&mut g, Key::Down, &label_names).ends_with("true"));
        assert!(key(&mut g, Key::Down, &label_names).ends_with("true"));
        assert_eq!(g.layers.selected(), Some(7));
        let typeahead_note = key(&mut g, Key::Char('s'), &label_names);
        let after = g.layers.selected().unwrap();
        assert_eq!(after, 8, "{typeahead_note}");
        assert!(label_names[after].to_lowercase().starts_with('s'));
        assert!(key(&mut g, Key::Up, &label_names).ends_with("true"));
        assert_eq!(g.layers.selected(), Some(7));
        // Full keyboard range: jump, page, then home.
        key(&mut g, Key::End, &label_names);
        assert_eq!(g.layers.selected(), Some(199));
        let per_page = g.layers.rows_per_page().max(1);
        key(&mut g, Key::PageUp, &label_names);
        assert_eq!(g.layers.selected(), Some(199 - per_page));
        key(&mut g, Key::Home, &label_names);
        assert_eq!(g.layers.selected(), Some(0));
        key(&mut g, Key::PageDown, &label_names);
        assert_eq!(g.layers.selected(), Some(per_page));

        // 5. Picker: Tab over, open from the field, filter, commit.
        key(&mut g, Key::Tab, &label_names);
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        let (fx, fy) = center(&scene, "fonts-field").expect("field");
        let note = press(&mut g, &app, &scene, fx, fy);
        assert!(g.fonts.is_open(), "{note}");
        for c in "ne".chars() {
            key(&mut g, Key::Char(c), &label_names);
        }
        let narrowed = g.fonts.filtered_len();
        assert!(key(&mut g, Key::Backspace, &label_names).ends_with("true"));
        assert!(
            g.fonts.filtered_len() >= narrowed,
            "erasing widens the search"
        );
        key(&mut g, Key::Char('e'), &label_names);
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        frame = hamesh_render::render(&scene);
        hamesh_render::save_png(&frame, &format!("{out}/gal-{w}x{h}-3-picker-open.png"))
            .expect("png");
        // The popup floats: every chrome rect must be identical to
        // the closed frame. The Eye audits the open state too.
        assert_eq!(
            snapshot_chrome(&layout),
            closed_chrome,
            "opening the popup must not reflow"
        );
        let rep_open = hamesh_inspect::visual_report(&scene);
        assert_only_edge_rows_clip(&rep_open, 2, "open popup");
        // A floating popup legitimately covers content beneath it
        // (it paints on the z=100 overlay layer, above everything).
        let pop = scene
            .nodes
            .iter()
            .find(|n| n.id == "fonts-popup")
            .expect("popup");
        assert!(
            pop.rect.x >= 0.0 && pop.rect.y >= 0.0,
            "popup on-screen: {:?}",
            pop.rect
        );
        assert!(
            pop.rect.x + pop.rect.w <= w as f32 + 0.5 && pop.rect.y + pop.rect.h <= h as f32 + 0.5,
            "popup inside viewport: {:?}",
            pop.rect
        );
        // 5b. Resize with the popup open: it re-anchors inside the
        // smaller viewport instead of stranding content off-screen.
        let (app2, layout2) = refresh(&mut g, w as f32, h as f32 - 40.0);
        let scene2 = hamesh_scene::build(&app2, &layout2, rev);
        let pop2 = scene2
            .nodes
            .iter()
            .find(|n| n.id == "fonts-popup")
            .expect("popup");
        assert!(
            pop2.rect.x >= 0.0 && pop2.rect.y >= 0.0,
            "resized popup on-screen: {:?}",
            pop2.rect
        );
        assert!(
            pop2.rect.x + pop2.rect.w <= w as f32 + 0.5
                && pop2.rect.y + pop2.rect.h <= h as f32 - 40.0 + 0.5,
            "resized popup fits: {:?}",
            pop2.rect
        );
        let rep5b = hamesh_inspect::visual_report(&scene2);
        assert_only_edge_rows_clip(&rep5b, 2, "resized open popup");

        let matches = g.fonts.filtered();
        assert!(!matches.is_empty(), "query 'ne' must match installed fonts");
        assert!(
            matches
                .iter()
                .all(|&i| g.fonts.items()[i].to_lowercase().contains("ne")),
            "every visible option matches the query"
        );
        let committed = key(&mut g, Key::Enter, &label_names);
        let value = g.fonts.value().unwrap_or("").to_string();
        assert!(
            !g.fonts.is_open() && value.to_lowercase().contains("ne"),
            "{committed} -> {value}"
        );
        let dismissed = key(&mut g, Key::Escape, &label_names);
        assert!(
            dismissed.ends_with("false"),
            "nothing to dismiss: {dismissed}"
        );
        // Pointer path: reopen and click the second option outright.
        // (This exercises the painted-row <-> widget-row agreement
        // that keyboard-only drills cannot see.)
        let note = press(&mut g, &app, &scene, fx, fy);
        assert!(g.fonts.is_open(), "{note}");
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        // Click the first visible option, wherever the highlight
        // rests after the reopen reveal.
        let s = g.fonts.visible().start;
        let expect = g
            .fonts
            .items()
            .get(g.fonts.filtered()[s])
            .cloned()
            .unwrap_or_default();
        let (ox, oy) = center(&scene, &format!("fonts-opt-{s}")).expect("visible option painted");
        let note = press(&mut g, &app, &scene, ox, oy);
        assert!(!g.fonts.is_open(), "{note}");
        assert_eq!(
            g.fonts.value().unwrap_or(""),
            expect,
            "pointer commits exactly the pointed row"
        );

        // 5c. Outside click dismisses the popup; the row still gets
        // its click and the committed value is preserved.
        // (The pointer commit above left the popup closed: reopen.)
        let note = press(&mut g, &app, &scene, fx, fy);
        assert!(g.fonts.is_open(), "{note}");
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        let first_visible = (g.layers.scroll() / ROW_H) as usize;
        let (lx, ly) = center(&scene, &format!("layers-row-{first_visible}")).expect("row visible");
        let value_before = g.fonts.value().unwrap_or("").to_string();
        let note = press(&mut g, &app, &scene, lx, ly);
        assert!(!g.fonts.is_open(), "{note}");
        assert!(
            note.contains("dismissed"),
            "the note says what happened: {note}"
        );
        assert_eq!(g.layers.selected(), Some(first_visible), "{note}");
        assert_eq!(
            g.fonts.value().unwrap_or(""),
            value_before,
            "dismissal preserves the committed value"
        );

        // 5d. Tab dismisses the popup and moves focus on.
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        let (fx2, fy2) = center(&scene, "fonts-field").expect("field");
        let note = press(&mut g, &app, &scene, fx2, fy2);
        assert!(g.fonts.is_open(), "{note}");
        assert_eq!(g.focus, 1);
        // No refresh needed: key() drives widget state only.
        key(&mut g, Key::Tab, &label_names);
        assert!(!g.fonts.is_open(), "tab dismisses the popup");
        assert_eq!(g.focus, 0, "tab moves focus on");
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        assert!(
            scene.nodes.iter().all(|n| n.id != "fonts-popup"),
            "dismissed popup leaves the tree"
        );
        assert_eq!(
            g.fonts.value().unwrap_or(""),
            value_before,
            "tab dismissal preserves the value"
        );

        // 5e. Escape cancels without committing.
        key(&mut g, Key::Tab, &label_names);
        assert_eq!(g.focus, 1);
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        let (fx3, fy3) = center(&scene, "fonts-field").expect("field");
        let note = press(&mut g, &app, &scene, fx3, fy3);
        assert!(g.fonts.is_open(), "{note}");
        // No refresh needed: key() drives widget state only.
        key(&mut g, Key::Down, &label_names);
        assert!(
            g.fonts.highlight().unwrap_or(999) > 0,
            "highlight moved off the top"
        );
        key(&mut g, Key::Escape, &label_names);
        assert!(!g.fonts.is_open(), "escape dismisses the popup");
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        assert!(
            scene.nodes.iter().all(|n| n.id != "fonts-popup"),
            "escaped popup leaves the tree"
        );
        assert_eq!(
            g.fonts.value().unwrap_or(""),
            value_before,
            "escape preserves the committed value"
        );

        // 5f. Short viewport: the same presync path runs at a
        // non-default height and the popup re-anchors on resize.
        // (True shrink cannot trigger in this chrome — 5x32px max
        // under 620px with a top-anchored field — so shrink geometry
        // is covered by the blocks placement tests instead.)
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        let (fx4, fy4) = center(&scene, "fonts-field").expect("field");
        let note = press(&mut g, &app, &scene, fx4, fy4);
        assert!(g.fonts.is_open(), "{note}");
        key(&mut g, Key::End, &label_names);
        rev += 1;
        // A real resize at every size, never below a viable chrome:
        // 1440x560 drops the popup budget 8->5 rows, exercising
        // the sync across a max-rows transition with the highlight
        // pinned at the bottom.
        let h2 = (h.saturating_sub(340)).max(480);
        let (app_c, layout_c) = refresh(&mut g, w as f32, h2 as f32);
        let scene_c = hamesh_scene::build(&app_c, &layout_c, rev);
        let pop_c = scene_c
            .nodes
            .iter()
            .find(|n| n.id == "fonts-popup")
            .expect("popup");
        assert!(
            pop_c.rect.x >= 0.0
                && pop_c.rect.y >= 0.0
                && pop_c.rect.x + pop_c.rect.w <= w as f32 + 0.5
                && pop_c.rect.y + pop_c.rect.h <= h2 as f32 + 0.5,
            "short popup fits: {:?}",
            pop_c.rect
        );
        let hi = g.fonts.highlight().expect("highlight survives the resize");
        assert!(
            g.fonts.visible().contains(&hi),
            "resized highlight revealed in actual geometry"
        );
        key(&mut g, Key::PageDown, &label_names);
        assert!(
            g.fonts.visible().contains(&g.fonts.highlight().unwrap()),
            "resized paging stays visible"
        );
        frame = hamesh_render::render(&scene_c);
        hamesh_render::save_png(&frame, &format!("{out}/gal-{w}x{h2}-5-short-open.png"))
            .expect("png");
        let rep_c = hamesh_inspect::visual_report(&scene_c);
        assert_only_edge_rows_clip(&rep_c, 2, "short open popup");
        // Leave the popup closed so the final frame matches the
        // established closed-state evidence.
        key(&mut g, Key::Escape, &label_names);
        assert!(!g.fonts.is_open(), "short popup dismisses cleanly");

        // 6. Final frame: the canvas headline is set in the picked font.
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        frame = hamesh_render::render(&scene);
        hamesh_render::save_png(&frame, &format!("{out}/gal-{w}x{h}-4-picked.png")).expect("png");
        let rep4 = hamesh_inspect::visual_report(&scene);
        assert_only_edge_rows_clip(&rep4, 2, "final");
        let hl = scene
            .nodes
            .iter()
            .find(|n| n.id == "poster-headline")
            .expect("headline");
        assert!(
            hl.font_resolved,
            "headline family must resolve, not substitute"
        );

        // 7. Probes: a visible row and the picker field.
        let first_visible = (g.layers.scroll() / ROW_H) as usize;
        let probe_row = format!("layers-row-{first_visible}");
        let pr = scene.nodes.iter().find(|n| n.id == probe_row).cloned();
        let pf = scene.nodes.iter().find(|n| n.id == "fonts-field").cloned();
        let probes: Vec<_> = [pr, pf]
            .into_iter()
            .flatten()
            .map(|n| {
                let p = hamesh_inspect::probe(
                    &app,
                    &layout,
                    &scene,
                    n.rect.x + n.rect.w / 2.0,
                    n.rect.y + n.rect.h / 2.0,
                );
                serde_json::json!({
                    "asked_about": n.id,
                    "topmost_candidate": p.topmost_candidate,
                    "hit": p.hit,
                })
            })
            .collect();
        for p in &probes {
            // Topmost is the asked node or its descendant (labels and
            // icons paint on top of their row/field): climb the tree,
            // not the id string.
            let top = p["topmost_candidate"].as_str().unwrap_or("");
            let asked = p["asked_about"].as_str().unwrap_or("");
            let mut cur = Some(top.to_string());
            let mut found = false;
            while let Some(cid) = cur {
                if cid == asked {
                    found = true;
                    break;
                }
                cur = app
                    .get(&cid)
                    .and_then(|c| c.parent.as_ref().map(|q| q.0.clone()));
            }
            assert!(
                found,
                "probe must resolve to the painted control or its subtree: {p}"
            );
        }

        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{w}x{h}: clipped={:?} off_viewport={:?} overlaps={} selected={:?} font={:?} scroll={:.0} ({ms:.0}ms)",
            rep4.clipped_content,
            rep4.off_viewport,
            rep4.overlaps.len(),
            g.layers.selected(),
            g.fonts.value(),
            g.layers.scroll(),
        );
        let doc = serde_json::json!({
            "size": [w, h],
            "selected": g.layers.selected(),
            "font": g.fonts.value(),
            "scroll": g.layers.scroll(),
            "focus": g.focus,
            "elapsed_ms": ms,
            "final_report": rep4,
            "probes": probes,
        });
        std::fs::write(
            format!("{out}/gal-{w}x{h}.json"),
            serde_json::to_string_pretty(&doc).expect("json"),
        )
        .expect("write json");
        summary.push((w, h));
    }
    println!("gallery ok: {summary:?}");
}
