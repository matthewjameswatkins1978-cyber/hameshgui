//! Pi worker panel: the C4 reuse proof. Same builders as the Agent X
//! dashboard (Card/Row/text/Badge/Metric/Progress/Button), different
//! data, no page-specific copies. Headless-verified (structure +
//! PNG); the live Eye/Hand path is exercised by the dashboard drill.

use hamesh_blocks::*;
use hamesh_core::App;

fn get(app: &App, k: &str, dflt: &str) -> String {
    app.state
        .get(k)
        .cloned()
        .unwrap_or_else(|| dflt.to_string())
}

/// Compact worker status composition. Reads initial values from state.
pub fn build_pi_panel() -> App {
    let mut app = App::new("win");
    app.set_state("pi_jobs", "3");
    app.set_state("pi_tokens", "1.2k");
    app.set_state("pi_queue", "40");
    app.set_state("pi_status", "Streaming");
    app.set_state("paused", "false");

    let (jobs, tokens, queue, status) = (
        get(&app, "pi_jobs", "3"),
        get(&app, "pi_tokens", "1.2k"),
        get(&app, "pi_queue", "40"),
        get(&app, "pi_status", "Streaming"),
    );
    let qval: f32 = queue.parse().unwrap_or(0.0);
    {
        let mut s = Scope::root(&mut app, "win");
        card(
            &mut s,
            "pi",
            CardOpts {
                grow: 1.0,
                ..Default::default()
            },
            |t| {
                row(t, "pi-header", LayoutOpts::flat(), |r| {
                    col(r, "pi-title-col", LayoutOpts::tight(), |c| {
                        title(c, "pi-title", "Pi worker");
                        subtle(c, "pi-sub", "background model loop");
                    });
                    spacer(r, "pi-hsp");
                    badge(r, "pi-status", &status, Variant::Info);
                });
                row(t, "pi-metrics", LayoutOpts::tiles(), |r| {
                    metric(r, "p-jobs", MetricOpts::new("Jobs", &jobs, "list-checks"));
                    metric(
                        r,
                        "p-tokens",
                        MetricOpts::new("Tokens", &tokens, "activity"),
                    );
                });
                col(t, "pi-prog", LayoutOpts::tight(), |c| {
                    row(c, "pi-prog-head", LayoutOpts::flat(), |r| {
                        heading(r, "pi-prog-title", "Queue");
                        spacer(r, "pi-psp");
                        caption(r, "pi-prog-val", &format!("{queue}%"));
                    });
                    progress(c, "pi-bar", ProgressOpts::new(qval));
                });
                row(t, "pi-action-row", LayoutOpts::flat(), |r| {
                    spacer(r, "pi-asp");
                    button(
                        r,
                        "pi-action",
                        "Pause worker",
                        ButtonOpts::new(ButtonVariant::Outline, "pause_worker"),
                    );
                });
            },
        );
    }
    refresh_pi_panel(&mut app);
    app
}

/// Re-sync displayed props from state (same pattern as the dashboard).
pub fn refresh_pi_panel(app: &mut App) {
    let set = |app: &mut App, id: &str, v: &str| {
        if let Some(c) = app.get_mut(id) {
            c.props.insert("text".to_string(), v.to_string());
        }
    };
    set(app, "p-jobs-value", &get(app, "pi_jobs", "3"));
    set(app, "p-tokens-value", &get(app, "pi_tokens", "1.2k"));
    let queue = get(app, "pi_queue", "40");
    set(app, "pi-prog-val", &format!("{queue}%"));
    if let Some(c) = app.get_mut("pi-bar") {
        c.props.insert("value".to_string(), queue);
    }
    set(app, "pi-status", &get(app, "pi_status", "Streaming"));
    let label = if get(app, "paused", "false") == "true" {
        "Resume worker"
    } else {
        "Pause worker"
    };
    set(app, "pi-action", label);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_panel_reuses_blocks() {
        // Same block types as the dashboard, different content: the
        // audit invariants hold without page-specific copies.
        let app = build_pi_panel();
        for id in [
            "pi",
            "pi-title",
            "pi-status",
            "p-jobs",
            "p-tokens",
            "pi-bar",
            "pi-action",
        ] {
            assert!(app.get(id).is_some(), "missing {id}");
        }
        assert_eq!(
            app.get("p-jobs")
                .unwrap()
                .props
                .get("block")
                .map(|s| s.as_str()),
            Some("Metric")
        );
        assert_eq!(
            app.get("pi-bar")
                .unwrap()
                .props
                .get("value")
                .map(|s| s.as_str()),
            Some("40")
        );
    }

    #[test]
    fn pi_panel_renders_surfaces() {
        let app = build_pi_panel();
        let layout = hamesh_layout::compute(&app, 480.0, 560.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        for n in &scene.nodes {
            assert!(!n.clipped, "{} clipped", n.id);
        }
        let f = hamesh_render::render(&scene);
        // Same proof as the dashboard: real surfaces, not glass.
        let d = f.pixmap.data();
        let (chunks, _) = d.as_chunks::<4>();
        let total = chunks.len();
        let painted = chunks.iter().filter(|c| **c != [10, 15, 17, 255]).count();
        assert!(
            painted as f64 / total as f64 > 0.4,
            "pi panel should paint surfaces, got {painted}/{total}"
        );
    }
}
