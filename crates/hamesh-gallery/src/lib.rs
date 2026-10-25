//! hamesh-gallery: the reusable creative-workspace driver.
//!
//! The consumer half of the 035 selection controls. Owns widget state
//! (`ListView`, `Picker`), composes the whole tree from Blocks, and
//! exposes the same intents a native loop forwards (pointer, wheel,
//! keys) that the headless drills exercise. A native window drives
//! these exact functions; nothing here knows about winit or pixels.

use hamesh_blocks as b;
use hamesh_core::App;
use hamesh_widgets::{ListView, Nav, Picker};

pub const ROW_H: f32 = 32.0;
const FIELD_H: f32 = 36.0;
const ROOT_PAD: f32 = 12.0;
const HEADER_PAD: f32 = 12.0;
const FOOTER_H: f32 = 28.0;

/// Keyboard intents a native loop forwards. Paging included: long
/// lists are unusable without it.
pub enum GKey {
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
pub struct Gallery {
    pub layer_rows: Vec<b::select::ListRow>,
    pub layers: ListView,
    pub fonts: Picker,
    pub focus: usize, // 0 = layers, 1 = picker
    pub last_typed: Option<std::time::Instant>,
}

impl Gallery {
    pub fn new() -> Self {
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
            last_typed: None,
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

impl Default for Gallery {
    fn default() -> Self {
        Self::new()
    }
}

/// Sync focus flags, then build the whole tree from widget state.
/// Single pass by construction: the caller budgets `layers_h`.
pub fn compose_with(g: &mut Gallery, width: f32, height: f32, layers_h: f32) -> App {
    g.layers.set_focused(g.focus == 0);
    g.fonts.set_focused(g.focus == 1);
    let narrow = g.narrow(width);
    let max_popup = g.max_popup_rows(height);
    // Hoisted: `g.rail_w` borrows all of `g`, which the composer
    // closures below cannot hold alongside `&mut g.layers`.
    let rail_w = g.rail_w(width);
    let rail_content_w = rail_w - 24.0;
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
                    // Fixed width: a hugging field would jitter as its
                    // text changes, and the popup anchors to a moving
                    // target. Widths are application layout (like the
                    // rail and inspector widths below).
                    b::LayoutOpts {
                        w: Some(240.0),
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
                        w: Some(rail_w),
                        pad: ROOT_PAD,
                        gap: 6.0,
                        grow: Some(0.0),
                        ..Default::default()
                    },
                    |t| {
                        b::caption(t, "rail-title", "LAYERS");
                        // rail_content_w is hoisted above the closures.
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
                                // Rail content width: the composer trims
                                // labels to what remains after chrome.
                                viewport_w: Some(rail_content_w),
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
pub fn refresh(g: &mut Gallery, w: f32, h: f32) -> (App, hamesh_layout::LayoutOutput) {
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
    // Grant the popup its actual placed height BEFORE composing the
    // live tree: the composer paints exactly the rows placement will
    // show, so navigation, virtualisation and pointer mapping agree
    // with rendered geometry by construction. Pass 1 measured the
    // field; the header never depends on the layers budget, so the
    // precomputed height is what placement will grant.
    if g.fonts.is_open() {
        let full = b::select::popup_height(g.fonts.filtered_len(), ROW_H, g.max_popup_rows(h));
        if let Some(field) = lay1.nodes.get("fonts-field") {
            let placed_h = b::select::popup_geom(&field.rect, w, h, full, field.rect.w).3;
            g.fonts.set_viewport(placed_h);
        }
    }
    let mut app = compose_with(g, w, h, layers);
    let layout = layout_of(&app, w, h);
    // Anchor an open popup to its field: overlay geometry that
    // never reflows the header, body or footer.
    if g.fonts.is_open() {
        let metrics = b::select::PopupMetrics {
            row_h: ROW_H,
            max_rows: g.max_popup_rows(h),
            matches: g.fonts.filtered_len(),
            scroll: g.fonts.scroll(),
        };
        if let Some((_, _, _, placed_h)) =
            b::select::place_popup(&mut app, &layout, "fonts", &metrics, w, h)
        {
            // Convergence guard: the precompute reads the same field
            // rect placement does, so this is a no-op in practice.
            // The viewport (never the tree) is what must agree here.
            g.fonts.set_viewport(placed_h);
        }
    }
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
pub fn press(g: &mut Gallery, app: &App, scene: &hamesh_scene::Scene, x: f32, y: f32) -> String {
    match hamesh_scene::hit_test(app, scene, x, y) {
        None => {
            if g.fonts.is_open() {
                g.fonts.close();
                "press hit nothing (popup dismissed)".to_string()
            } else {
                "press hit nothing".to_string()
            }
        }
        Some(id) => match route(app, &id) {
            Route::LayerRow(row) => {
                g.focus = 0;
                let note = if g.layers.select(row) {
                    format!("selected layer {row}")
                } else {
                    format!("layer {row} already selected")
                };
                // An open popup never survives a press elsewhere:
                // the row still gets its click; the popup cancels
                // without touching the committed value.
                if g.fonts.is_open() {
                    g.fonts.close();
                    format!("{note} (popup dismissed)")
                } else {
                    note
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
            Route::Chrome(hit) => {
                if g.fonts.is_open() {
                    g.fonts.close();
                    format!("hit {hit} (chrome, popup dismissed)")
                } else {
                    format!("hit {hit} (chrome, no action)")
                }
            }
        },
    }
}

pub fn hover(g: &mut Gallery, scene: &hamesh_scene::Scene, x: f32, y: f32, app: &App) -> bool {
    let Some(id) = hamesh_scene::hit_test(app, scene, x, y) else {
        return false;
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
        Route::LayerRow(_) => view
            .map(|r| g.layers.hover_at_viewport(y - r.y))
            .unwrap_or(false),
        Route::PickOpt(_) => popup
            .map(|r| {
                g.fonts.highlight_at_viewport(y - r.y);
                true
            })
            .unwrap_or(false),
        _ => false,
    }
}

pub fn wheel(
    g: &mut Gallery,
    layout: &hamesh_layout::LayoutOutput,
    x: f32,
    y: f32,
    dy: f32,
) -> bool {
    let over = |id: &str| layout.nodes.get(id).is_some_and(|n| n.rect.contains(x, y));
    if over("fonts-popup") {
        g.fonts.wheel(dy)
    } else if over("layers-view") {
        g.layers.wheel(dy)
    } else {
        false
    }
}

pub fn key(g: &mut Gallery, k: GKey, label_names: &[String]) -> String {
    if matches!(k, GKey::Tab) {
        // Moving focus dismisses the popup (cancel: the committed
        // value is preserved). A fresh focus never inherits an open
        // popup from the other control.
        g.fonts.close();
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
                // Idle timeout: a stale prefix never steers a new search.
                let now = std::time::Instant::now();
                if g.last_typed
                    .is_some_and(|t| now.duration_since(t).as_millis() > 500)
                {
                    g.layers.clear_typeahead();
                }
                g.last_typed = Some(now);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        g: &mut Gallery,
        w: f32,
        h: f32,
    ) -> (App, hamesh_layout::LayoutOutput, hamesh_scene::Scene) {
        let (app, layout) = refresh(g, w, h);
        let scene = hamesh_scene::build(&app, &layout, 1);
        (app, layout, scene)
    }

    fn center_of(scene: &hamesh_scene::Scene, id: &str) -> (f32, f32) {
        let n = scene.nodes.iter().find(|n| n.id == id).unwrap();
        (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0)
    }

    fn names_of(g: &Gallery) -> Vec<String> {
        g.layer_rows.iter().map(|r| r.label.clone()).collect()
    }

    #[test]
    fn outside_press_dismisses_popup_and_keeps_the_click() {
        let mut g = Gallery::new();
        let before = g.fonts.value().unwrap_or("").to_string();
        g.fonts.open();
        let (app, _layout, scene) = frame(&mut g, 1440.0, 900.0);
        let (cx, cy) = center_of(&scene, "layers-row-3");
        let note = press(&mut g, &app, &scene, cx, cy);
        assert!(!g.fonts.is_open(), "{note}");
        assert_eq!(
            g.layers.selected(),
            Some(3),
            "the row still gets its click: {note}"
        );
        assert_eq!(
            g.fonts.value().unwrap_or(""),
            before,
            "dismissal preserves the value"
        );
        assert!(
            note.contains("dismissed"),
            "the note says what happened: {note}"
        );
    }

    #[test]
    fn tab_dismisses_popup_and_moves_focus() {
        let mut g = Gallery::new();
        g.focus = 1;
        g.fonts.open();
        let names = names_of(&g);
        let _ = key(&mut g, GKey::Tab, &names);
        assert!(!g.fonts.is_open());
        assert_eq!(g.focus, 0);
        let _ = key(&mut g, GKey::Tab, &names);
        assert_eq!(g.focus, 1);
        assert!(!g.fonts.is_open());
    }

    #[test]
    fn escape_cancels_without_committing() {
        let mut g = Gallery::new();
        let before = g.fonts.value().unwrap_or("").to_string();
        g.focus = 1;
        g.fonts.open();
        let names = names_of(&g);
        let _ = key(&mut g, GKey::Down, &names);
        let _ = key(&mut g, GKey::Escape, &names);
        assert!(!g.fonts.is_open());
        assert_eq!(g.fonts.value().unwrap_or(""), before);
    }

    #[test]
    fn short_viewport_syncs_actual_placed_height() {
        // End-to-end wiring at a short viewport: the driver grants
        // the actual placed height before composing, so the painted
        // window, reveal, paging and pointer mapping all agree with
        // rendered geometry. (True placement shrink cannot trigger
        // in this chrome — the popup caps at 5x32px under 620px
        // while the field sits near the top — so shrink geometry
        // itself is covered by the blocks placement tests, where the
        // viewport is directly drivable.)
        let mut g = Gallery::new();
        g.focus = 1;
        g.fonts.open();
        // Drive the highlight to the bottom, then shorten the viewport.
        let names = names_of(&g);
        let _ = key(&mut g, GKey::End, &names);
        let (app, _layout, scene) = frame(&mut g, 820.0, 300.0);
        let pop = scene
            .nodes
            .iter()
            .find(|n| n.id == "fonts-popup")
            .expect("popup");
        assert!(
            pop.rect.y + pop.rect.h <= 300.5,
            "short popup fits: {:?}",
            pop.rect
        );
        let full = g.fonts.filtered_len().min(g.max_popup_rows(300.0)) as f32 * ROW_H;
        assert!(
            pop.rect.h <= full + 0.5,
            "placement grants at most the request: {} <= {full}",
            pop.rect.h
        );
        let hi = g.fonts.highlight().expect("highlight survives the crush");
        assert!(
            g.fonts.visible().contains(&hi),
            "highlight revealed in actual geometry"
        );
        // Paging steps by actual rows and the pointer maps through scroll.
        let _ = key(&mut g, GKey::PageDown, &names);
        let hi2 = g.fonts.highlight().unwrap();
        assert!(g.fonts.visible().contains(&hi2));
        let _ = key(&mut g, GKey::Home, &names);
        assert!(g.fonts.visible().contains(&g.fonts.highlight().unwrap()));
        let _ = app;
    }
}
