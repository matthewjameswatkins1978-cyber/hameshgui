//! hamesh-036-lab: the 036 default-appearance proof.
//!
//! A single design-system sheet composed ONLY from Blocks over
//! driver-owned widget state: button variants in every interaction
//! state, list selection states, and the type ramp — all painted from
//! the theme state-system contract. Headless: compose, lay out,
//! render, then ask the Eye. Run: `cargo run -p hamesh-036-lab -- target/036`

use hamesh_blocks as b;
use hamesh_core::App;
use hamesh_widgets::ListView;

const ROW_H: f32 = 32.0;

fn state_list() -> (Vec<b::select::ListRow>, ListView) {
    let rows = vec![
        b::select::ListRow::new("Inbox").with_icon("activity"),
        b::select::ListRow::new("Drafts").with_icon("list-checks"),
        b::select::ListRow::new("Sent").with_icon("users"),
        b::select::ListRow::new("Archive").with_icon("triangle-alert"),
    ];
    let mut view = ListView::new(ROW_H);
    view.set_count(rows.len());
    assert!(view.select(1), "selected row");
    // Hover the row below the selection: selected > hover > rest.
    assert!(view.hover_at_viewport(2.0 * ROW_H + 4.0));
    assert_eq!(view.hover(), Some(2));
    view.set_focused(true);
    (rows, view)
}

fn button_row(s: &mut b::Scope, row_id: &str, variant: b::ButtonVariant, name: &str, label: &str) {
    use b::{ButtonOpts, ButtonState};
    b::row(
        s,
        row_id,
        b::LayoutOpts {
            gap: 8.0,
            ..Default::default()
        },
        |t| {
            b::caption(t, &format!("{row_id}-name"), name);
            let states = [
                ("rest", ButtonState::Rest, false),
                ("hover", ButtonState::Hover, false),
                ("pressed", ButtonState::Pressed, false),
                ("disabled", ButtonState::Disabled, false),
                ("focused", ButtonState::Rest, true),
            ];
            for (state_name, state, focused) in states {
                b::button(
                    t,
                    &format!("btn-{row_id}-{state_name}"),
                    label,
                    ButtonOpts::new(variant.clone(), &format!("{row_id}-{state_name}"))
                        .with_state(state)
                        .focused(focused),
                );
            }
        },
    );
}

fn compose(width: f32, height: f32) -> (App, hamesh_layout::LayoutOutput) {
    let (rows, mut view) = state_list();
    // Short viewports compact: tighter rhythm, body copy folds into
    // the heading line. Nothing on the sheet may clip at any size.
    let short = height < 620.0;
    // Tall viewports earn a surface-token strip: proof, not filler.
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
                button_row(s, "primary", b::ButtonVariant::Primary, "Primary", "Create");
                button_row(s, "outline", b::ButtonVariant::Outline, "Outline", "Cancel");
                button_row(s, "ghost", b::ButtonVariant::Ghost, "Ghost", "More");
                b::row(
                    s,
                    "mid",
                    b::LayoutOpts {
                        grow: Some(1.0),
                        gap,
                        ..Default::default()
                    },
                    |t| {
                        b::col(
                            t,
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
                                        view: &mut view,
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
                            t,
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
                        |t| {
                            for tok in [
                                "window",
                                "surface",
                                "raised",
                                "header",
                                "accent",
                                "accent_hover",
                                "accent_active",
                            ] {
                                b::swatch(t, &format!("sw-{tok}"), tok);
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
                b::caption(s, "foot-status", "Hamesh · 036 appearance proof");
                b::spacer(s, "foot-space");
                b::caption(s, "foot-size", &format!("{width:.0}×{height:.0}"));
            },
        );
    }
    let layout = hamesh_layout::compute(&app, width, height);
    (app, layout)
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/036".to_string());
    std::fs::create_dir_all(&out).expect("out dir");
    hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());

    for (w, h) in [(1440u32, 900u32), (1100, 720), (820, 560)] {
        let (app, layout) = compose(w as f32, h as f32);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let frame = hamesh_render::render(&scene);
        hamesh_render::save_png(&frame, &format!("{out}/ds-{w}x{h}.png")).expect("png");

        let rep = hamesh_inspect::visual_report(&scene);
        assert!(
            rep.clipped_content.is_empty(),
            "{w}x{h}: nothing may clip: {:?}",
            rep.clipped_content
        );
        assert!(
            rep.off_viewport.is_empty(),
            "{w}x{h}: nothing may leave the viewport: {:?}",
            rep.off_viewport
        );
        assert!(
            rep.overlaps.is_empty(),
            "{w}x{h}: no overlaps: {:?}",
            rep.overlaps
        );

        // State truth, painted: every variant × state resolves to the
        // contract's tokens, and the disabled control is honestly off.
        let expect = [
            ("btn-primary-rest", "accent", "bg"),
            ("btn-primary-hover", "accent_hover", "bg"),
            ("btn-primary-pressed", "accent_active", "bg"),
            ("btn-primary-disabled", "surface", "muted"),
            ("btn-outline-rest", "surface", "text"),
            ("btn-outline-hover", "raised", "text"),
            ("btn-ghost-rest", "raised", "text_dim"),
            ("btn-ghost-hover", "raised", "text"),
        ];
        for (id, surface, text) in expect {
            let c = app
                .get(id)
                .unwrap_or_else(|| panic!("{w}x{h}: {id} painted"));
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
        let dis = app.get("btn-primary-disabled").unwrap();
        assert!(!dis.enabled, "disabled paint means disabled hit-testing");
        // Disabled still wins the hit (a human would hit it); the
        // driver, not the composer, reports "disabled" from `enabled`.
        let r = scene
            .nodes
            .iter()
            .find(|n| n.id == "btn-primary-disabled")
            .unwrap()
            .rect
            .clone();
        let hit = hamesh_scene::hit_test(&app, &scene, r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert_eq!(hit.as_deref(), Some("btn-primary-disabled"));

        // Selection states share the same contract.
        let sel = app.get("demo-list-row-1").unwrap();
        assert_eq!(sel.props.get("surface").map(|s| s.as_str()), Some("accent"));
        let hov = app.get("demo-list-row-2").unwrap();
        assert_eq!(hov.props.get("surface").map(|s| s.as_str()), Some("raised"));
        let root = app.get("demo-list").unwrap();
        assert_eq!(
            root.props.get("border_token").map(|s| s.as_str()),
            Some("focus")
        );

        println!("{w}x{h}: Eye clean, states honest");
        let doc = serde_json::json!({
            "size": [w, h],
            "report": rep,
        });
        std::fs::write(
            format!("{out}/ds-{w}x{h}.json"),
            serde_json::to_string_pretty(&doc).expect("json"),
        )
        .expect("write json");
    }
    println!("036 appearance ok");
    run_tasks(&out);
}

fn center(scene: &hamesh_scene::Scene, id: &str) -> (f32, f32) {
    let n = scene.nodes.iter().find(|n| n.id == id).unwrap();
    (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0)
}

/// The second composition: a functional task queue driven entirely by
/// shared routing (`Hit::of`) and theme-contract composers. Exercises
/// select, filter, toggle-done and focus traversal at every size.
fn run_tasks(out: &str) {
    for (w, h) in [(1440u32, 900u32), (1100, 720), (820, 560)] {
        let mut t = hamesh_tasks::Tasks::new();
        let (app, layout) = hamesh_tasks::refresh(&mut t, w as f32, h as f32);
        let scene = hamesh_scene::build(&app, &layout, 1);
        // Select a row, then toggle it done through the action button.
        let (cx, cy) = center(&scene, "tasks-row-2");
        let note = hamesh_tasks::press(&mut t, &app, &scene, cx, cy);
        assert!(note.contains("selected task row 2"), "{note}");
        let (app2, layout2) = hamesh_tasks::refresh(&mut t, w as f32, h as f32);
        let scene2 = hamesh_scene::build(&app2, &layout2, 2);
        let (bx, by) = center(&scene2, "btn-toggle");
        let note = hamesh_tasks::press(&mut t, &app2, &scene2, bx, by);
        assert!(note.contains("done"), "{note}");
        assert_eq!(t.done_count(), 1);
        // Filter narrows the queue live; the committed count survives.
        let _ = hamesh_tasks::key(&mut t, hamesh_tasks::TKey::Tab);
        let _ = hamesh_tasks::key(&mut t, hamesh_tasks::TKey::Tab);
        assert_eq!(t.focus, 1);
        let _ = hamesh_tasks::key(&mut t, hamesh_tasks::TKey::Char('o'));
        assert!(t.filter.is_open());
        assert!(t.shown().len() < 8 && !t.shown().is_empty());
        let (app3, layout3) = hamesh_tasks::refresh(&mut t, w as f32, h as f32);
        let scene3 = hamesh_scene::build(&app3, &layout3, 3);
        let frame = hamesh_render::render(&scene3);
        hamesh_render::save_png(&frame, &format!("{out}/task-{w}x{h}.png")).expect("png");
        let rep = hamesh_inspect::visual_report(&scene3);
        assert!(
            rep.clipped_content.is_empty(),
            "{w}x{h}: {:?}",
            rep.clipped_content
        );
        assert!(
            rep.off_viewport.is_empty(),
            "{w}x{h}: {:?}",
            rep.off_viewport
        );
        // The open filter popup floats above the queue by design (same
        // contract as the 035 gallery): only same-layer overlaps count.
        let bad: Vec<_> = rep
            .overlaps
            .iter()
            .filter(|o| !(o.over.starts_with("filter-popup") || o.over.starts_with("filter-opt-")))
            .collect();
        assert!(bad.is_empty(), "{w}x{h}: {bad:?}");
        // Done-ness is painted: the selected task's badge follows it.
        let sel = t.selected_task();
        let done_here = sel
            .and_then(|i| t.tasks.get(i))
            .map(|x| x.done)
            .unwrap_or(false);
        let badge = app3.get("sel-status");
        if w >= 1000 {
            let badge = badge.expect("detail rail at desktop widths");
            assert_eq!(
                badge.props.get("text").map(|s| s.as_str()),
                Some(if done_here { "Done" } else { "Open" })
            );
        } else {
            assert!(badge.is_none(), "rail drops on narrow windows");
        }
        println!("{w}x{h}: tasks Eye clean, queue functional");
    }
    println!("036 tasks ok");
}
