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
/// Wears Signal Cabinet, the default profile.
pub fn build_dashboard() -> App {
    build_dashboard_with(&SIGNAL_CABINET)
}

/// Compose the dashboard wearing a design profile. The default
/// composition wears Signal Cabinet; variety is proved by dressing
/// the same composition in a second profile (see folio test).
pub fn build_dashboard_with(profile: &Profile) -> App {
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
    apply_profile(&mut app, profile);
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

    #[test]
    fn folio_profile_gives_a_different_voice() {
        // Variety proof: the same composition dressed in Living Folio
        // must speak with different advances than Signal Cabinet —
        // profiles are data, not a one-off treatment. Vendored faces
        // are registered first, so this holds on every machine.
        voice_proof(&LIVING_FOLIO, "folio", "target/folio-proof.png");
    }

    #[test]
    fn grid_profile_gives_a_third_voice() {
        // The Unquiet Grid: narrowed, weighted Instrument Sans display
        // against the same Plex Mono technical voice. Three profiles,
        // one composition, three unmistakable treatments.
        voice_proof(&UNQUIET_GRID, "grid", "target/grid-proof.png");
        // The guest appearance: the subtitle speaks Newsreader Italic.
        let alt = build_dashboard_with(&UNQUIET_GRID);
        let sub = alt.get("dash-sub").unwrap();
        assert_eq!(
            sub.props.get("font_style").map(|s| s.as_str()),
            Some("italic")
        );
        let layout = hamesh_layout::compute(&alt, 800.0, 600.0);
        let scene = hamesh_scene::build(&alt, &layout, 1);
        let sn = scene.nodes.iter().find(|n| n.id == "dash-sub").unwrap();
        assert!(
            sn.font_resolved,
            "italic kicker must resolve, not substitute"
        );
    }

    /// Shared variety proof: `profile` voices the composition, the
    /// title advance must differ from Cabinet, the composition stays
    /// sound (single-line heading, no clipping), and a proof PNG is
    /// saved for human eyes.
    fn voice_proof(profile: &Profile, name: &str, png: &str) {
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let cab = build_dashboard();
        let alt = build_dashboard_with(profile);
        // Independence: identical content — only the voices differ.
        let words = |app: &App| -> Vec<(String, String)> {
            let mut v: Vec<(String, String)> = app
                .components
                .values()
                .filter_map(|c| c.props.get("text").map(|t| (c.id.0.clone(), t.clone())))
                .collect();
            v.sort();
            v
        };
        assert_eq!(words(&cab), words(&alt), "{name} must not change content");
        let layout_c = hamesh_layout::compute(&cab, 800.0, 600.0);
        let layout_a = hamesh_layout::compute(&alt, 800.0, 600.0);
        let scene_c = hamesh_scene::build(&cab, &layout_c, 1);
        let scene_a = hamesh_scene::build(&alt, &layout_a, 1);
        let tc = scene_c.nodes.iter().find(|n| n.id == "dash-title").unwrap();
        let ta = scene_a.nodes.iter().find(|n| n.id == "dash-title").unwrap();
        eprintln!(
            "{name} proof: cabinet title={:.1} {name} title={:.1}",
            tc.advance, ta.advance
        );
        assert!(
            (tc.advance - ta.advance).abs() >= 1.0,
            "{name} must voice the title differently: {:.1} vs {:.1}",
            tc.advance,
            ta.advance
        );
        assert!(
            ta.font_resolved,
            "{name} display face must resolve, not substitute"
        );
        let pt = scene_a
            .nodes
            .iter()
            .find(|n| n.id == "dash-prog-title")
            .unwrap();
        assert_eq!(pt.lines, 1, "{name} prog heading must not wrap");
        for n in &scene_a.nodes {
            if let Some(ink) = n.ink {
                assert!(
                    ink[0] >= -0.5 && ink[2] <= n.rect.w + 0.5,
                    "{name} ink escapes box for {}",
                    n.id
                );
            }
        }
        let f = hamesh_render::render(&scene_a);
        std::fs::create_dir_all("target").unwrap();
        hamesh_render::save_png(&f, png).unwrap();
        // Cabinet reference from the SAME run: identical content,
        // viewport and state — the three PNGs are directly comparable.
        let fc = hamesh_render::render(&scene_c);
        hamesh_render::save_png(&fc, "target/cabinet-proof.png").unwrap();
    }
}
