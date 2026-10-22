//! Agent X dashboard: the B2 reference composition, built from blocks.
//!
//! Values are data-bound: builders read `app.state`, and
//! `refresh_dashboard` re-syncs text/value props after any state change.
//! The window calls `refresh_dashboard` after every press (mouse,
//! keyboard, inspector) so bindings stay live on all input paths.

use hamesh_blocks::*;
use hamesh_core::App;

fn get(app: &App, k: &str, dflt: &str) -> String {
    app.state
        .get(k)
        .cloned()
        .unwrap_or_else(|| dflt.to_string())
}

fn set_text(app: &mut App, id: &str, text: &str) {
    if let Some(c) = app.get_mut(id) {
        c.props.insert("text".to_string(), text.to_string());
    }
}

fn set_value(app: &mut App, id: &str, value: &str) {
    if let Some(c) = app.get_mut(id) {
        c.props.insert("value".to_string(), value.to_string());
    }
}

/// Compose the dashboard. Reads initial values from state.
pub fn build_dashboard() -> App {
    let mut app = App::new("win");
    app.set_state("agents", "4");
    app.set_state("tasks", "12");
    app.set_state("errors", "0");
    app.set_state("progress", "75");
    app.set_state("status", "Connected");
    app.set_state("design_inspected", "false");

    let (agents, tasks, errors, progress_s, status) = (
        get(&app, "agents", "4"),
        get(&app, "tasks", "12"),
        get(&app, "errors", "0"),
        get(&app, "progress", "75"),
        get(&app, "status", "Connected"),
    );
    let pval: f32 = progress_s.parse().unwrap_or(0.0);
    {
        let mut s = Scope::root(&mut app, "win");
        // Root card fills the window: an app panel, not a floating box.
        card(
            &mut s,
            "dash",
            CardOpts {
                grow: 1.0,
                ..Default::default()
            },
            |t| {
                row(t, "dash-header", LayoutOpts::flat(), |r| {
                    col(r, "dash-title-col", LayoutOpts::tight(), |c| {
                        title(c, "dash-title", "Agent X");
                        subtle(c, "dash-sub", "Autonomous operations overview");
                    });
                    spacer(r, "dash-hsp");
                    badge(r, "dash-status", &status, Variant::Success);
                });
                row(t, "dash-metrics", LayoutOpts::tiles(), |r| {
                    metric(r, "m-agents", MetricOpts::new("Agents", &agents, "users"));
                    metric(
                        r,
                        "m-tasks",
                        MetricOpts::new("Tasks", &tasks, "list-checks"),
                    );
                    metric(
                        r,
                        "m-errors",
                        MetricOpts::new("Errors", &errors, "triangle-alert"),
                    );
                });
                col(t, "dash-prog", LayoutOpts::tight(), |c| {
                    row(c, "dash-prog-head", LayoutOpts::flat(), |r| {
                        heading(r, "dash-prog-title", "Development progress");
                        spacer(r, "dash-psp");
                        caption(r, "dash-prog-val", &format!("{progress_s}%"));
                    });
                    progress(c, "dash-bar", ProgressOpts::new(pval));
                });
                row(t, "dash-action-row", LayoutOpts::flat(), |r| {
                    spacer(r, "dash-asp");
                    button(
                        r,
                        "dash-action",
                        "Inspect design",
                        ButtonOpts::new(ButtonVariant::Outline, "inspect_design"),
                    );
                });
                // Activity section: fills the lower panel with live
                // status content instead of empty charcoal.
                divider(t, "dash-act-sep");
                col(t, "dash-act", LayoutOpts::tight(), |c| {
                    row(c, "dash-act-head", LayoutOpts::flat(), |r| {
                        heading(r, "dash-act-title", "Recent activity");
                        spacer(r, "dash-act-sp");
                        caption(r, "dash-act-live", "Live");
                    });
                    activity(c, "a1", "Deployment queued", "agent-3 · 2m ago");
                    activity(c, "a2", "Task batch completed", "12 tasks · 18m ago");
                    activity(c, "a3", "Error budget healthy", "0 errors · 1h ago");
                });
            },
        );
    }
    refresh_dashboard(&mut app);
    // Signal Cabinet, local cut: condensed display voice on titles
    // and numerals, mono on readouts and labels (029, Phase B).
    apply_profile(&mut app, &SIGNAL_CABINET_LOCAL);
    app
}

/// Re-sync displayed props from state. Defensive: unknown ids are
/// skipped, so calling this on a non-dashboard app is a no-op.
pub fn refresh_dashboard(app: &mut App) {
    let (agents, tasks, errors, progress) = (
        get(app, "agents", "4"),
        get(app, "tasks", "12"),
        get(app, "errors", "0"),
        get(app, "progress", "75"),
    );
    set_text(app, "m-agents-value", &agents);
    set_text(app, "m-tasks-value", &tasks);
    set_text(app, "m-errors-value", &errors);
    set_text(app, "m-agents-label", "Agents");
    set_text(app, "dash-prog-val", &format!("{progress}%"));
    set_value(app, "dash-bar", &progress);
    set_text(app, "dash-status", &get(app, "status", "Connected"));
    if get(app, "design_inspected", "false") == "true" {
        set_text(app, "dash-action", "Inspected ✓");
    }
    // Keep the canonical value props honest for the Eye.
    set_value(app, "m-agents", &agents);
    set_value(app, "m-tasks", &tasks);
    set_value(app, "m-errors", &errors);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_structure() {
        let app = build_dashboard();
        let dash = app.get("dash").unwrap();
        assert_eq!(dash.props.get("block").map(|s| s.as_str()), Some("Card"));
        for id in ["m-agents", "m-tasks", "m-errors"] {
            let m = app.get(id).unwrap();
            assert_eq!(m.props.get("block").map(|s| s.as_str()), Some("Metric"));
        }
        let bar = app.get("dash-bar").unwrap();
        assert_eq!(bar.props.get("block").map(|s| s.as_str()), Some("Progress"));
        assert_eq!(bar.props.get("value").map(|s| s.as_str()), Some("75"));
    }

    #[test]
    fn refresh_resyncs_from_state() {
        let mut app = build_dashboard();
        app.set_state("agents", "9");
        app.set_state("progress", "50");
        app.set_state("design_inspected", "true");
        refresh_dashboard(&mut app);
        assert_eq!(
            app.get("m-agents-value")
                .unwrap()
                .props
                .get("text")
                .map(|s| s.as_str()),
            Some("9")
        );
        assert_eq!(
            app.get("dash-prog-val")
                .unwrap()
                .props
                .get("text")
                .map(|s| s.as_str()),
            Some("50%")
        );
        assert_eq!(
            app.get("dash-action")
                .unwrap()
                .props
                .get("text")
                .map(|s| s.as_str()),
            Some("Inspected ✓")
        );
    }

    #[test]
    fn dashboard_tiles_shrink_then_wrap() {
        // Wide: three equal tiles in one row. Phone-narrow: tiles
        // shrink to min-content, then the row breaks and they stack.
        let app = build_dashboard();
        let wide = hamesh_layout::compute(&app, 760.0, 560.0);
        let wa = wide.get("m-agents").unwrap();
        let wb = wide.get("m-tasks").unwrap();
        assert_eq!(wa.rect.y, wb.rect.y, "wide: one row");
        // Basis zero shares space equally; whole-pixel rounding may
        // leave a 1px remainder on one tile.
        assert!((wa.rect.w - wb.rect.w).abs() <= 1.0, "wide: equal tiles");
        let narrow = hamesh_layout::compute(&app, 240.0, 900.0);
        let na = narrow.get("m-agents").unwrap();
        let nb = narrow.get("m-tasks").unwrap();
        assert!(
            nb.rect.y >= na.rect.y + na.rect.h - 1.0,
            "narrow: tiles must stack, agents.y={} h={} tasks.y={}",
            na.rect.y,
            na.rect.h,
            nb.rect.y
        );
    }

    #[test]
    fn composition_audit_reads_as_groups() {
        // Machine-checked composition (§4): real measurements over
        // the canonical geometry. Errors fail; observations stay out.
        // Viewport is the real window size (800x600): the reference
        // composition must fit its window without clipping.
        let app = build_dashboard();
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let r = |id: &str| layout.get(id).unwrap().rect.clone();
        let sn = |id: &str| scene.nodes.iter().find(|n| n.id == id).unwrap();
        // 1. Title/subtitle form one group: visible glyph gap small.
        let tg = r("dash-title");
        let sg = r("dash-sub");
        let gap = sg.y - (tg.y + tg.h);
        assert!((0.0..=12.0).contains(&gap), "title/sub group gap: {gap}");
        // 2. Progress heading paints whole: single line, trim present.
        let pt = sn("dash-prog-title");
        assert_eq!(pt.lines, 1, "prog heading must not wrap");
        assert!(pt.trim.is_some(), "prog heading needs trim");
        // 2b. Badge + button contents are centered AND right-anchored:
        // independent layout relationships, verified together.
        for id in ["dash-status", "dash-action"] {
            let n = sn(id);
            assert_eq!(n.halign, "center", "{id} declares center");
            let cx = n.origin[0] + n.advance / 2.0;
            let w = layout.get(id).unwrap().rect.w;
            assert!(
                (cx - w / 2.0).abs() < 1.0,
                "{id} content not centered: {cx} vs {w}"
            );
        }
        let hdr = r("dash-header");
        let st = r("dash-status");
        assert!(
            (st.x + st.w - (hdr.x + hdr.w)).abs() < 1.0,
            "badge right-anchored"
        );
        let ar = r("dash-action-row");
        let ab = r("dash-action");
        assert!(
            (ab.x + ab.w - (ar.x + ar.w)).abs() < 1.0,
            "button right-anchored"
        );
        // 3. Tiles equal, aligned in one row.
        let (a, b, c) = (r("m-agents"), r("m-tasks"), r("m-errors"));
        assert!(
            (a.w - b.w).abs() <= 1.0 && (b.w - c.w).abs() <= 1.0,
            "equal tiles"
        );
        assert!(a.y == b.y && b.y == c.y, "one row");
        // 4. No clipping anywhere in the composition.
        for n in &scene.nodes {
            assert!(!n.clipped, "{} clipped", n.id);
        }
        // 5. No sibling overlap within each container row/col.
        for parent in [
            "dash-header",
            "dash-metrics",
            "dash-action-row",
            "dash-prog-head",
        ] {
            let kids: Vec<_> = app.get(parent).unwrap().children.iter().collect();
            for (i, x) in kids.iter().enumerate() {
                for y in &kids[i + 1..] {
                    let (rx, ry) = (r(&x.0), r(&y.0));
                    let overlap_x = (rx.x < ry.x + ry.w) && (ry.x < rx.x + rx.w);
                    let overlap_y = (rx.y < ry.y + ry.h) && (ry.y < rx.y + rx.h);
                    // Ghost spacers share rows by design (they hold space).
                    let ghost = |id: &str| sn(id).ghost;
                    if !(overlap_x && overlap_y) || ghost(&x.0) || ghost(&y.0) {
                        continue;
                    }
                    panic!("overlap: {} vs {:?} vs {:?}", parent, rx, ry);
                }
            }
        }
        // 6. Section rhythm: gaps between card sections exceed the
        // largest inner gap (proximity = grouping).
        let card_gap: f32 = app
            .get("dash")
            .unwrap()
            .props
            .get("gap")
            .unwrap()
            .parse()
            .unwrap();
        let inner = [("dash-title-col", 4.0), ("dash-prog", 4.0)];
        for (id, g) in inner {
            let cg: f32 = app
                .get(id)
                .unwrap()
                .props
                .get("gap")
                .unwrap_or(&"0".to_string())
                .parse()
                .unwrap_or(0.0);
            assert!(
                card_gap > g && card_gap > cg,
                "card {card_gap} must exceed inner {g}/{cg}"
            );
        }
    }

    #[test]
    fn medium_width_keeps_one_row() {
        // 560px viewport: tiles still share a row (min_w 140 fits).
        let app = build_dashboard();
        let layout = hamesh_layout::compute(&app, 560.0, 560.0);
        let (a, b) = (
            layout.get("m-agents").unwrap(),
            layout.get("m-tasks").unwrap(),
        );
        assert_eq!(a.rect.y, b.rect.y, "560px: one row");
    }

    #[test]
    fn frame_pipeline_stays_interactive() {
        // Perf guard (§H2): geometry + raster for the whole dashboard
        // must stay far below a frame budget. Catches 10x regressions
        // (e.g. uncached shaping per frame), not machine noise.
        let app = build_dashboard();
        let layout = hamesh_layout::compute(&app, 760.0, 560.0);
        let t0 = std::time::Instant::now();
        for _ in 0..5 {
            let scene = hamesh_scene::build(&app, &layout, 1);
            let _ = hamesh_render::render(&scene);
        }
        let ms = t0.elapsed().as_millis();
        println!("5x scene+render (layout once): {ms}ms");
        assert!(ms < 5000, "frame pipeline too slow: {ms}ms for 5 frames");
    }

    #[test]
    fn dashboard_renders_mostly_surfaces() {
        let app = build_dashboard();
        let layout = hamesh_layout::compute(&app, 760.0, 560.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        assert!(scene.nodes.iter().any(|n| n.id == "dash-action"));
        let bar = layout.get("dash-bar").unwrap();
        assert_eq!(bar.rect.h, 6.0, "progress bar honors h=6");
        let btn = layout.get("dash-action").unwrap();
        assert!(
            btn.rect.w > 50.0,
            "button sizes to its label, got {}",
            btn.rect.w
        );
        let f = hamesh_render::render(&scene);
        // The composition paints real surfaces, not background glass.
        let d = f.pixmap.data();
        let (chunks, _) = d.as_chunks::<4>();
        let total = chunks.len();
        let painted = chunks.iter().filter(|c| **c != [10, 15, 17, 255]).count();
        assert!(
            painted as f64 / total as f64 > 0.5,
            "dashboard should mostly paint surfaces, got {painted}/{total}"
        );
    }
}
