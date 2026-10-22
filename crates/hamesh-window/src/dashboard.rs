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
        card(&mut s, "dash", CardOpts::default(), |t| {
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
        });
    }
    refresh_dashboard(&mut app);
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
