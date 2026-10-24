//! hamesh-033-lab: an independent composition, used as evidence.
//!
//! Specimen is a poster editor; this is a dense operations console, built
//! from the same Blocks vocabulary. It exists to answer one question: do
//! the 033 framework improvements (paint-time clip, deterministic paint
//! order, the pixels->component mapper) help a GUI that was never written
//! for them?
//!
//! Headless on purpose: compose, lay out, render, then ask the Eye what
//! it sees. Run:
//!
//! ```text
//! cargo run -p hamesh-033-lab -- target/033
//! ```
//!
//! Outputs one PNG and one JSON report per viewport size.

use hamesh_blocks as b;
use hamesh_core::App;

/// The composition. Deliberately dense and text-heavy: real consoles are.
fn compose(width: f32, height: f32) -> App {
    let mut app = App::new("ops");
    app.get_mut("ops").unwrap().props.extend([
        ("pad".into(), "0".into()),
        ("gap".into(), "0".into()),
        ("surface".into(), "bg".into()),
    ]);
    let narrow = width < 1000.0;
    // Short viewports cannot hold tiles + throughput + feed + footer:
    // the Eye reported the footer off-viewport at 560px tall. Drop the
    // secondary card rather than pushing chrome out of reach.
    let short = height < 620.0;
    let nav_w = if narrow { 168.0 } else { 208.0 };
    let side_w = if narrow { 200.0 } else { 248.0 };
    {
        let mut root = b::Scope::root(&mut app, "ops");
        b::row(
            &mut root,
            "header",
            b::LayoutOpts {
                h: Some(56.0),
                pad: 12.0,
                ..Default::default()
            },
            |s| {
                // Align the wordmark with the column edge below it
                // (header pad 12 + 8 = rail content 28).
                s.leaf(
                    hamesh_core::Component::new("title-strut", hamesh_core::Role::Panel, "strut")
                        .with_prop("ghost", "true")
                        .with_prop("w", "8"),
                );
                b::title(s, "app-title", "Signal Ops");
                b::spacer(s, "header-space");
                // Narrow widths keep the count in the header: the number of
                // open alerts is never hidden, only their full list.
                b::badge(
                    s,
                    "state",
                    if narrow {
                        "DEGRADED · 3 ALERTS"
                    } else {
                        "DEGRADED"
                    },
                    b::Variant::Warning,
                );
                b::button(
                    s,
                    "ack",
                    "Acknowledge",
                    b::ButtonOpts::new(b::ButtonVariant::Ghost, "ack"),
                );
                b::button(
                    s,
                    "mute",
                    "Silence",
                    b::ButtonOpts::new(b::ButtonVariant::Primary, "mute"),
                );
            },
        );
        b::row(
            &mut root,
            "body",
            b::LayoutOpts {
                grow: Some(1.0),
                pad: 12.0,
                gap: 12.0,
                ..Default::default()
            },
            |s| {
                // Left rail: channel list.
                b::col(
                    s,
                    "rail",
                    b::LayoutOpts {
                        w: Some(nav_w),
                        pad: 12.0,
                        gap: 6.0,
                        grow: Some(0.0),
                        ..Default::default()
                    },
                    |s| {
                        b::caption(s, "rail-title", "CHANNELS");
                        for (i, name) in [
                            "uplink / primary",
                            "uplink / standby",
                            "telemetry bus",
                            "archive replay",
                            "operator console",
                        ]
                        .iter()
                        .enumerate()
                        {
                            b::button(
                                s,
                                &format!("chan-{i}"),
                                name,
                                b::ButtonOpts::new(
                                    if i == 0 {
                                        b::ButtonVariant::Outline
                                    } else {
                                        b::ButtonVariant::Ghost
                                    },
                                    &format!("chan-{i}"),
                                ),
                            );
                        }
                        b::spacer(s, "rail-space");
                        b::caption(s, "rail-foot", "5 of 9 active");
                    },
                );
                // Centre: metrics, throughput, event feed.
                b::col(
                    s,
                    "main",
                    b::LayoutOpts {
                        grow: Some(1.0),
                        pad: 0.0,
                        gap: 12.0,
                        ..Default::default()
                    },
                    |s| {
                        b::row(
                            s,
                            "tiles",
                            // Height comes from measured content: pad 16*2 +
                            // icon 20 + gaps + METRIC_VALUE + caption. 88px
                            // crushed the labels; the Eye named them clipped.
                            b::LayoutOpts {
                                h: Some(124.0),
                                pad: 0.0,
                                gap: 12.0,
                                grow: Some(0.0),
                                ..Default::default()
                            },
                            |s| {
                                for (id, label, value, icon) in [
                                    ("t-ok", "PACKETS", "18.4k", "activity"),
                                    ("t-lat", "LATENCY", "42 ms", "list-checks"),
                                    ("t-drop", "DROPPED", "0.7%", "triangle-alert"),
                                    ("t-peers", "PEERS", "12", "users"),
                                ] {
                                    b::metric(s, id, b::MetricOpts::new(label, value, icon));
                                }
                            },
                        );
                        // Narrow: the full alert column is gone, so the
                        // highest-priority alert and its action stay visible in
                        // a compact strip. Critical information is preserved,
                        // not merely eliminated to avoid overflow.
                        if narrow {
                            b::card(
                                s,
                                "alerts-compact",
                                b::CardOpts {
                                    grow: 0.0,
                                    pad: hamesh_theme::space::M,
                                    gap: hamesh_theme::space::S,
                                    ..Default::default()
                                },
                                |t| {
                                    b::caption(t, "alerts-compact-title", "OPEN ALERTS · 3");
                                    b::body(
                                        t,
                                        "alerts-compact-top",
                                        "Uplink / primary degraded: retransmits above threshold",
                                    );
                                    b::button(
                                        t,
                                        "escalate-compact",
                                        "Escalate to on-call",
                                        b::ButtonOpts::new(b::ButtonVariant::Outline, "escalate"),
                                    );
                                },
                            );
                        }
                        if !short {
                            b::card(
                                s,
                                "throughput",
                                b::CardOpts {
                                    grow: 0.0,
                                    gap: hamesh_theme::space::S,
                                    ..Default::default()
                                },
                                |t| {
                                    b::heading(t, "tp-title", "Throughput");
                                    b::progress(t, "tp-bar", b::ProgressOpts::new(64.0));
                                    b::caption(
                                        t,
                                        "tp-note",
                                        "64% of the 40 Gb/s ceiling · window 5 min",
                                    );
                                },
                            );
                        }
                        // Hug the content: a bordered card stretched to
                        // fill 500px of void draws attention to emptiness.
                        // A feed that scrolls is a later concern; until it
                        // does, the card is the size of what it holds.
                        b::card(
                            s,
                            "events",
                            b::CardOpts {
                                grow: 0.0,
                                gap: hamesh_theme::space::S,
                                ..Default::default()
                            },
                            |t| {
                                b::heading(t, "ev-title", "Event feed");
                                // Short viewports trade feed depth for the alert
                                // summary: the feed is secondary, the degraded
                                // uplink is not.
                                for (id, main, sub) in [
                                    ("e1", "Retransmit burst on uplink / primary", "12:04"),
                                    ("e2", "Peer 7 rejoined the mesh", "12:01"),
                                    ("e3", "Archive replay paused at 41%", "11:58"),
                                    ("e4", "Operator console reauthenticated", "11:52"),
                                    ("e5", "Latency above 40 ms for 3 minutes", "11:49"),
                                ]
                                .iter()
                                .take(if short { 3 } else { 5 })
                                {
                                    b::activity(t, id, main, sub);
                                }
                            },
                        );
                    },
                );
                // Right: alerts. Dropped on narrow windows: at 820px the
                // rail + side leave the feed too little room, and the Eye
                // reported the whole column off-viewport. Responsive
                // structure beats squeezing a column nobody can read.
                if !narrow {
                    b::col(
                        s,
                        "side",
                        b::LayoutOpts {
                            w: Some(side_w),
                            pad: 12.0,
                            gap: 8.0,
                            grow: Some(0.0),
                            ..Default::default()
                        },
                        |s| {
                            b::caption(s, "side-title", "OPEN ALERTS");
                            b::body(
                                s,
                                "a1",
                                "Uplink / primary degraded: retransmits above threshold",
                            );
                            b::body(s, "a2", "Peer 7 clock skew 180 ms");
                            b::body(s, "a3", "Archive replay paused by operator");
                            b::button(
                                s,
                                "escalate",
                                "Escalate to on-call",
                                b::ButtonOpts::new(b::ButtonVariant::Outline, "escalate"),
                            );
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
                b::caption(s, "foot-status", "Connected · 12 peers · 42 ms");
                b::spacer(s, "foot-space");
                b::caption(s, "foot-ver", "HAMESH 033 / CPU");
            },
        );
    }
    app
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/033".to_string());
    std::fs::create_dir_all(&out).expect("out dir");
    hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());

    let sizes = [(1440u32, 900u32), (1100, 720), (820, 560)];
    for (w, h) in sizes {
        let app = compose(w as f32, h as f32);
        let layout = hamesh_layout::compute(&app, w as f32, h as f32);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let frame = hamesh_render::render(&scene);
        let png = format!("{out}/ops-{w}x{h}.png");
        hamesh_render::save_png(&frame, &png).expect("png");

        let report = hamesh_inspect::visual_report(&scene);
        // Icon boxes are measured, not eyeballed: an icon that renders as
        // a dot is a sizing fact, and the Eye should say so.
        let icon_rects: Vec<_> = scene
            .nodes
            .iter()
            .filter(|n| n.id.ends_with("-icon"))
            .map(|n| (n.id.clone(), [n.rect.x, n.rect.y, n.rect.w, n.rect.h]))
            .collect();
        // Two probe samples: the centre of the first metric tile, and a
        // point in the event feed. Pixels -> component, every time.
        let probes: Vec<_> = ["t-ok", "e3"]
            .iter()
            .filter_map(|id| {
                let n = scene.nodes.iter().find(|n| n.id == *id)?;
                Some((
                    *id,
                    hamesh_inspect::probe(
                        &app,
                        &layout,
                        &scene,
                        n.rect.x + n.rect.w / 2.0,
                        n.rect.y + n.rect.h / 2.0,
                    ),
                ))
            })
            .collect();
        let doc = serde_json::json!({
            "size": [w, h],
            "png": png,
            "report": report,
            "icon_rects": icon_rects,
            "probes": probes
                .into_iter()
                .map(|(id, p)| serde_json::json!({
                    "asked_about": id,
                    "topmost_candidate": p.topmost_candidate,
                    "hit": p.hit,
                    "coverage": p.paint_order.iter().map(|h| format!("{:?}", h.coverage)).collect::<Vec<_>>(),
                    "paint_order": p.paint_order.iter().map(|h| h.id.clone()).collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
        });
        std::fs::write(
            format!("{out}/ops-{w}x{h}.json"),
            serde_json::to_string_pretty(&doc).expect("json"),
        )
        .expect("write json");
        println!(
            "{w}x{h}: clipped={:?} off_viewport={:?} overlaps={}",
            report.clipped_content,
            report.off_viewport,
            report.overlaps.len()
        );
    }
}
