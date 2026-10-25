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

use hamesh_blocks as b;
use hamesh_core::App;
use hamesh_widgets::{ListView, Nav, Picker};

const ROW_H: f32 = 32.0;
const FIELD_H: f32 = 36.0;
const ROOT_PAD: f32 = 12.0;
const HEADER_PAD: f32 = 12.0;
const FOOTER_H: f32 = 28.0;

/// Keyboard intents a native loop forwards. Paging included: long
/// lists are unusable without it.
enum GKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Char(char),
    Enter,
    Escape,
    Backspace,
    Tab,
}

#[derive(Clone)]
struct Gallery {
    layer_rows: Vec<b::select::ListRow>,
    layers: ListView,
    fonts: Picker,
    focus: usize, // 0 = layers, 1 = picker
}

impl Gallery {
    fn new() -> Self {
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let families = hamesh_text::installed_family_names();
        assert!(!families.is_empty(), "need at least the bundled voices");
        let mut fonts = Picker::new(families);
        // Deterministic headline voice where available.
        let preferred = fonts
            .items()
            .iter()
            .position(|f| f == "Newsreader")
            .unwrap_or(0);
        fonts.set_selected(Some(preferred));

        let kinds = ["activity", "list-checks", "users"];
        let mut layer_rows = vec![
            b::select::ListRow::new("Background"),
            b::select::ListRow::new("Headline").with_icon("activity"),
            b::select::ListRow::new("Subhead").with_icon("activity"),
            b::select::ListRow::new("Hero image").with_icon("users"),
            b::select::ListRow::new("Caption").with_icon("list-checks"),
            b::select::ListRow::new("Outdated logo").with_icon("triangle-alert"),
        ];
        for i in 6..200 {
            let kind = kinds[i % kinds.len()];
            layer_rows.push(b::select::ListRow::new(&format!("Shape {i:03}")).with_icon(kind));
        }
        let mut layers = ListView::new(ROW_H);
        layers.set_count(layer_rows.len());
        assert!(layers.select(0));
        Self {
            layer_rows,
            layers,
            fonts,
            focus: 0,
        }
    }

    fn narrow(&self, width: f32) -> bool {
        width < 1000.0
    }

    fn short(&self, height: f32) -> bool {
        height < 620.0
    }

    fn max_popup_rows(&self, height: f32) -> usize {
        if self.short(height) {
            5
        } else {
            8
        }
    }

    /// Rail chrome: its pads, captions and gaps. The driver
    /// subtracts this plus the body pad from the measured body.
    fn rail_chrome() -> f32 {
        24.0 + 16.0 * 2.0 + 6.0 * 2.0
    }

    fn rail_w(&self, width: f32) -> f32 {
        if self.narrow(width) {
            168.0
        } else {
            208.0
        }
    }

    fn side_w(&self, width: f32) -> f32 {
        if self.narrow(width) {
            200.0
        } else {
            248.0
        }
    }
}

/// Sync focus flags, then build the whole tree from widget state.
/// Single pass by construction: the caller budgets `layers_h`.
fn compose_with(g: &mut Gallery, width: f32, height: f32, layers_h: f32) -> App {
    g.layers.set_focused(g.focus == 0);
    g.fonts.set_focused(g.focus == 1);
    let narrow = g.narrow(width);
    let max_popup = g.max_popup_rows(height);
    let selected_label = g
        .layers
        .selected()
        .and_then(|i| g.layer_rows.get(i))
        .map(|r| r.label.clone())
        .unwrap_or_default();
    let selected_idx = g
        .layers
        .selected()
        .map(|i| i.to_string())
        .unwrap_or_default();
    let font_value = g.fonts.value().unwrap_or("").to_string();
    let scroll_px = format!("{:.0}", g.layers.scroll());

    let mut app = App::new("gallery");
    {
        let mut root = b::Scope::root(&mut app, "gallery");
        // Header: title plus the font picker. Auto height: the popup
        // expands it in flow (no second pass, no overlay math).
        b::row(
            &mut root,
            "header",
            b::LayoutOpts {
                pad: HEADER_PAD,
                ..Default::default()
            },
            |s| {
                b::title(s, "app-title", "Gallery");
                b::spacer(s, "header-space");
                b::col(
                    s,
                    "picker-col",
                    b::LayoutOpts {
                        pad: 0.0,
                        gap: 4.0,
                        ..Default::default()
                    },
                    |t| {
                        b::select::picker(
                            t,
                            "fonts",
                            b::select::PickerSpec {
                                picker: &mut g.fonts,
                                field_h: FIELD_H,
                                row_h: ROW_H,
                                max_rows: max_popup,
                                placeholder: "Choose a font…".to_string(),
                            },
                        );
                    },
                );
            },
        );
        // Body: layers rail, canvas, inspector.
        b::row(
            &mut root,
            "body",
            b::LayoutOpts {
                grow: Some(1.0),
                pad: ROOT_PAD,
                gap: ROOT_PAD,
                ..Default::default()
            },
            |s| {
                b::col(
                    s,
                    "rail",
                    b::LayoutOpts {
                        w: Some(g.rail_w(width)),
                        pad: ROOT_PAD,
                        gap: 6.0,
                        grow: Some(0.0),
                        ..Default::default()
                    },
                    |t| {
                        b::caption(t, "rail-title", "LAYERS");
                        b::select::list(
                            t,
                            "layers",
                            b::select::ListSpec {
                                rows: &g.layer_rows,
                                view: &mut g.layers,
                                viewport_h: layers_h,
                                row_h: ROW_H,
                                empty_label: "No layers".to_string(),
                                grow: None,
                            },
                        );
                        b::caption(t, "rail-foot", &format!("{} layers", g.layer_rows.len()));
                    },
                );
                // Canvas: the remaining space. The headline is set in
                // the picked family — the consumer's payoff, in one prop.
                b::col(
                    s,
                    "canvas",
                    b::LayoutOpts {
                        grow: Some(1.0),
                        pad: 16.0,
                        gap: 8.0,
                        ..Default::default()
                    },
                    |t| {
                        b::heading(t, "poster-kicker", "POSTER");
                        let mut hl = hamesh_core::Component::new(
                            "poster-headline",
                            hamesh_core::Role::Text,
                            "headline",
                        );
                        hl.props
                            .insert("text".to_string(), format!("Set in {font_value}"));
                        // Responsive display size: one line at every width.
                        // (A fixed 64px wraps mid-phrase on narrow canvases.)
                        let display = if width < 1000.0 {
                            40.0
                        } else if width < 1200.0 {
                            44.0
                        } else {
                            64.0
                        };
                        hl.props
                            .insert("font_size".to_string(), display.to_string());
                        hl.props.insert("font".to_string(), font_value.clone());
                        hl.props
                            .insert("text_token".to_string(), "text".to_string());
                        t.leaf(hl);
                        b::body(
                            t,
                            "poster-sub",
                            "A creative workspace assembled from reusable controls.",
                        );
                    },
                );
                // Inspector: dropped on narrow windows (responsive
                // structure, like 033: shed panels, never state).
                if !narrow {
                    b::col(
                        s,
                        "side",
                        b::LayoutOpts {
                            w: Some(g.side_w(width)),
                            pad: ROOT_PAD,
                            gap: 8.0,
                            grow: Some(0.0),
                            ..Default::default()
                        },
                        |t| {
                            b::caption(t, "side-title", "INSPECTOR");
                            for (k, v) in [
                                ("Layer", selected_label.clone()),
                                ("Index", selected_idx.clone()),
                                ("Font", font_value.clone()),
                                ("Scroll", scroll_px.clone()),
                            ] {
                                b::caption(t, &format!("insp-k-{k}"), k);
                                b::body(t, &format!("insp-v-{k}"), &v);
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
                h: Some(FOOTER_H),
                pad: 8.0,
                ..Default::default()
            },
            |s| {
                b::caption(s, "foot-status", "Gallery · HAMESH 035");
                b::spacer(s, "foot-space");
                b::caption(s, "foot-size", &format!("{width:.0}×{height:.0}"));
            },
        );
    }
    app
}

fn layout_of(app: &App, w: f32, h: f32) -> hamesh_layout::LayoutOutput {
    hamesh_layout::compute(app, w, h)
}

/// Two-pass driver budgeting. Pass 1 measures the real chrome
/// (header height depends on content, never on arithmetic); pass 2
/// grants the layers viewport exactly what remains. The header
/// never depends on the layers height, so one re-pass is exact.
fn refresh(g: &mut Gallery, w: f32, h: f32) -> (App, hamesh_layout::LayoutOutput) {
    // Pass 1 budgets small: the rail always fits, so the body rect
    // is exact and the layers viewport takes precisely what remains.
    // Pass 1 runs on a throwaway clone: composing syncs widget
    // viewports (and may reveal), which must not leak into the
    // driver's live scroll state.
    let mut probe = g.clone();
    let app1 = compose_with(&mut probe, w, h, 96.0);
    let lay1 = layout_of(&app1, w, h);
    let body = lay1.nodes.get("body").map(|n| n.rect.h).unwrap_or(600.0);
    // Whole rows only: a fractional viewport would park a partial row
    // across the footer caption at rest. Mid-scroll partials are
    // transient and clipped honestly; resting states stay exact.
    let layers = ((body - ROOT_PAD * 2.0 - Gallery::rail_chrome()) / ROW_H).floor() * ROW_H;
    let layers = layers.max(96.0);
    let app = compose_with(g, w, h, layers);
    let layout = layout_of(&app, w, h);
    (app, layout)
}

/// Where a hit belongs. Labels, icons and rows all resolve to the
/// row: the driver never parses ids, it reads props up the tree.
enum Route {
    LayerRow(usize),
    PickOpt(usize),
    PickerField,
    Chrome(String),
}

fn route(app: &App, id: &str) -> Route {
    let mut cur = Some(id.to_string());
    while let Some(cid) = cur {
        let Some(c) = app.get(&cid) else { break };
        if let Some(r) = c.props.get("row").and_then(|v| v.parse::<usize>().ok()) {
            if c.props.contains_key("list") {
                return Route::LayerRow(r);
            }
        }
        if let Some(p) = c.props.get("pick").and_then(|v| v.parse::<usize>().ok()) {
            return Route::PickOpt(p);
        }
        if c.props.contains_key("picker") {
            return Route::PickerField;
        }
        cur = c.parent.as_ref().map(|p| p.0.clone());
    }
    Route::Chrome(id.to_string())
}

/// Pointer press through the consumer's whole routing: hit-test, read
/// the row/option identity props, drive the widget. Returns a note.
fn press(g: &mut Gallery, app: &App, scene: &hamesh_scene::Scene, x: f32, y: f32) -> String {
    match hamesh_scene::hit_test(app, scene, x, y) {
        None => "press hit nothing".to_string(),
        Some(id) => match route(app, &id) {
            Route::LayerRow(row) => {
                g.focus = 0;
                if g.layers.select(row) {
                    format!("selected layer {row}")
                } else {
                    format!("layer {row} already selected")
                }
            }
            Route::PickOpt(pick) => {
                g.focus = 1;
                // Human two-step: highlight under the pointer, then commit.
                let popup = scene
                    .nodes
                    .iter()
                    .find(|n| n.id == "fonts-popup")
                    .map(|n| n.rect.clone());
                if let Some(r) = popup {
                    g.fonts.highlight_at_viewport(y - r.y);
                }
                match g.fonts.commit() {
                    Some(item) => format!("picked option {pick} = {item}"),
                    None => "commit found nothing (unchanged)".to_string(),
                }
            }
            Route::PickerField => {
                g.focus = 1;
                if g.fonts.toggle() {
                    "picker opened".to_string()
                } else {
                    "picker closed".to_string()
                }
            }
            Route::Chrome(hit) => format!("hit {hit} (chrome, no action)"),
        },
    }
}

fn hover(g: &mut Gallery, scene: &hamesh_scene::Scene, x: f32, y: f32, app: &App) {
    let Some(id) = hamesh_scene::hit_test(app, scene, x, y) else {
        return;
    };
    let view = scene
        .nodes
        .iter()
        .find(|n| n.id == "layers-view")
        .map(|n| n.rect.clone());
    let popup = scene
        .nodes
        .iter()
        .find(|n| n.id == "fonts-popup")
        .map(|n| n.rect.clone());
    match route(app, &id) {
        Route::LayerRow(_) => {
            if let Some(r) = view {
                g.layers.hover_at_viewport(y - r.y);
            }
        }
        Route::PickOpt(_) => {
            if let Some(r) = popup {
                g.fonts.highlight_at_viewport(y - r.y);
            }
        }
        _ => {}
    }
}

fn wheel(g: &mut Gallery, layout: &hamesh_layout::LayoutOutput, x: f32, y: f32, dy: f32) -> bool {
    let over = |id: &str| layout.nodes.get(id).is_some_and(|n| n.rect.contains(x, y));
    if over("fonts-popup") {
        g.fonts.wheel(dy)
    } else if over("layers-view") {
        g.layers.wheel(dy)
    } else {
        false
    }
}

fn key(g: &mut Gallery, k: GKey, label_names: &[String]) -> String {
    if matches!(k, GKey::Tab) {
        g.focus = (g.focus + 1) % 2;
        g.layers.clear_typeahead();
        return format!(
            "focus -> {}",
            if g.focus == 0 { "layers" } else { "picker" }
        );
    }
    if g.focus == 0 {
        match k {
            GKey::Up => format!("layers nav: {}", g.layers.nav(Nav::Up)),
            GKey::Down => format!("layers nav: {}", g.layers.nav(Nav::Down)),
            GKey::PageUp => format!("layers nav: {}", g.layers.nav(Nav::PageUp)),
            GKey::PageDown => format!("layers nav: {}", g.layers.nav(Nav::PageDown)),
            GKey::Home => format!("layers nav: {}", g.layers.nav(Nav::Home)),
            GKey::End => format!("layers nav: {}", g.layers.nav(Nav::End)),
            GKey::Char(c) => {
                let names = label_names.to_vec();
                format!(
                    "layers typeahead: {}",
                    g.layers.typeahead(c, &|i| names[i].clone())
                )
            }
            GKey::Backspace => {
                g.layers.clear_typeahead();
                "typeahead cleared".to_string()
            }
            GKey::Enter | GKey::Escape => "layers: no popup to confirm or dismiss".to_string(),
            GKey::Tab => unreachable!("handled above"),
        }
    } else {
        match k {
            GKey::Up => format!("picker nav: {}", g.fonts.nav(Nav::Up)),
            GKey::Down => format!("picker nav: {}", g.fonts.nav(Nav::Down)),
            GKey::PageUp => format!("picker nav: {}", g.fonts.nav(Nav::PageUp)),
            GKey::PageDown => format!("picker nav: {}", g.fonts.nav(Nav::PageDown)),
            GKey::Home => format!("picker nav: {}", g.fonts.nav(Nav::Home)),
            GKey::End => format!("picker nav: {}", g.fonts.nav(Nav::End)),
            GKey::Char(c) => {
                if !g.fonts.is_open() {
                    g.fonts.open();
                }
                format!("picker type: {}", g.fonts.push_char(c))
            }
            GKey::Backspace => format!("picker erase: {}", g.fonts.backspace()),
            GKey::Enter => match g.fonts.commit() {
                Some(i) => format!("picker committed: {}", g.fonts.items()[i].clone()),
                None => "picker commit found nothing".to_string(),
            },
            GKey::Escape => format!("picker dismissed: {}", g.fonts.close()),
            GKey::Tab => unreachable!("handled above"),
        }
    }
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
    assert!(
        report.overlaps.is_empty(),
        "{step}: overlaps: {:?}",
        report.overlaps
    );
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
        assert!(key(&mut g, GKey::Down, &label_names).ends_with("true"));
        assert!(key(&mut g, GKey::Down, &label_names).ends_with("true"));
        assert_eq!(g.layers.selected(), Some(7));
        let typeahead_note = key(&mut g, GKey::Char('s'), &label_names);
        let after = g.layers.selected().unwrap();
        assert_eq!(after, 8, "{typeahead_note}");
        assert!(label_names[after].to_lowercase().starts_with('s'));
        assert!(key(&mut g, GKey::Up, &label_names).ends_with("true"));
        assert_eq!(g.layers.selected(), Some(7));
        // Full keyboard range: jump, page, then home.
        key(&mut g, GKey::End, &label_names);
        assert_eq!(g.layers.selected(), Some(199));
        let per_page = g.layers.rows_per_page().max(1);
        key(&mut g, GKey::PageUp, &label_names);
        assert_eq!(g.layers.selected(), Some(199 - per_page));
        key(&mut g, GKey::Home, &label_names);
        assert_eq!(g.layers.selected(), Some(0));
        key(&mut g, GKey::PageDown, &label_names);
        assert_eq!(g.layers.selected(), Some(per_page));

        // 5. Picker: Tab over, open from the field, filter, commit.
        key(&mut g, GKey::Tab, &label_names);
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        let (fx, fy) = center(&scene, "fonts-field").expect("field");
        let note = press(&mut g, &app, &scene, fx, fy);
        assert!(g.fonts.is_open(), "{note}");
        for c in "ne".chars() {
            key(&mut g, GKey::Char(c), &label_names);
        }
        let narrowed = g.fonts.filtered_len();
        assert!(key(&mut g, GKey::Backspace, &label_names).ends_with("true"));
        assert!(
            g.fonts.filtered_len() >= narrowed,
            "erasing widens the search"
        );
        key(&mut g, GKey::Char('e'), &label_names);
        rev += 1;
        (app, layout) = refresh(&mut g, w as f32, h as f32);
        scene = hamesh_scene::build(&app, &layout, rev);
        frame = hamesh_render::render(&scene);
        hamesh_render::save_png(&frame, &format!("{out}/gal-{w}x{h}-3-picker-open.png"))
            .expect("png");
        let matches = g.fonts.filtered();
        assert!(!matches.is_empty(), "query 'ne' must match installed fonts");
        assert!(
            matches
                .iter()
                .all(|&i| g.fonts.items()[i].to_lowercase().contains("ne")),
            "every visible option matches the query"
        );
        let committed = key(&mut g, GKey::Enter, &label_names);
        let value = g.fonts.value().unwrap_or("").to_string();
        assert!(
            !g.fonts.is_open() && value.to_lowercase().contains("ne"),
            "{committed} -> {value}"
        );
        let dismissed = key(&mut g, GKey::Escape, &label_names);
        assert!(
            dismissed.ends_with("false"),
            "nothing to dismiss: {dismissed}"
        );

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
