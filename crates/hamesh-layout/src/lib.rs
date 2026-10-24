//! hamesh-layout: constraints + Flex layout via Taffy, with visibility/clipping truth.
//!
//! The layout crate owns computed geometry. It never duplicates component
//! semantics; it resolves them into rectangles plus visibility facts the Eye
//! needs (outside viewport, clipped by parent, hidden).

use hamesh_core::App;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use taffy::prelude::*;

/// Text content for leaf measurement. Taffy calls back into parley
/// so text nodes size to their content instead of collapsing.
/// `line_height` is the declared ramp value (None = font default);
/// measure and render share it so boxes and pixels agree.
#[derive(Clone, Debug)]
pub struct TextMeasure {
    pub text: String,
    pub font_size: f32,
    pub line_height: Option<f32>,
}

/// Measure a text leaf: known dimensions win, otherwise parley.
/// MinContent asks for the longest word (wrapping upper bound).
/// Width includes the renderer's text origin inset on both sides
/// (blit_text draws at rect.x + 8): measuring raw glyph width left
/// tight pills 8px too narrow and text crossed the border.
fn measure_text(
    ctx: Option<&TextMeasure>,
    known: Size<Option<f32>>,
    avail: Size<AvailableSpace>,
) -> Size<f32> {
    const TEXT_INSET_X: f32 = 8.0;
    if let (Some(w), Some(h)) = (known.width, known.height) {
        return Size {
            width: w,
            height: h,
        };
    }
    let Some(tm) = ctx else {
        return Size::ZERO;
    };
    let lh = tm.line_height;
    let max_w = match avail.width {
        AvailableSpace::Definite(x) => (x - 2.0 * TEXT_INSET_X).max(0.0),
        AvailableSpace::MaxContent => f32::INFINITY,
        AvailableSpace::MinContent => tm
            .text
            .split_whitespace()
            .map(|wd| hamesh_text::TextLayout::new(wd, f32::INFINITY, tm.font_size, lh).width())
            .fold(0.0, f32::max),
    };
    let lay = hamesh_text::TextLayout::new(&tm.text, max_w.max(0.0), tm.font_size, lh);
    // Pixel containment: fractional advances need the next whole pixel
    // (raster floors origins). Without ceil, rect.w - inset can land a
    // fraction short of the advance and paint wraps what layout kept
    // single-line. Shrink:0 above keeps rect.w >= this measurement.
    let contained = lay.width().ceil();
    Size {
        width: known.width.unwrap_or(contained + 2.0 * TEXT_INSET_X),
        height: known.height.unwrap_or(lay.height()),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutNode {
    pub rect: Rect,
    /// False when component.visible == false or an ancestor is hidden.
    pub effectively_visible: bool,
    /// True when the node's rect extends beyond its parent's rect.
    pub clipped_by_parent: bool,
    pub z: i32,
}

#[derive(Debug)]
pub struct LayoutOutput {
    pub nodes: HashMap<String, LayoutNode>,
    pub viewport: Rect,
}

impl LayoutOutput {
    pub fn get(&self, id: &str) -> Option<&LayoutNode> {
        self.nodes.get(id)
    }
}

/// Style hints carried in component props:
/// `w`, `h` (px), `flex_grow`, `layout: column|row`, `overlay: true`,
/// `pad` (px, default 12, 0 = flush), `gap` (px, default 8),
/// `wrap: true` (flex-wrap for responsive equal-tile rows),
/// `align: start|center|end` (cross-axis, default stretch),
/// `basis` (flex base px; 0 = share space equally regardless of content),
/// `min_w` (readability floor: wrapping rows break instead of squeezing).
fn child_style(comp: &hamesh_core::Component) -> Style {
    let parse_nonneg = |k: &str, dflt: f32| -> f32 {
        comp.props
            .get(k)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v >= 0.0)
            .unwrap_or(dflt)
    };
    let parse = |k: &str| -> Option<f32> {
        comp.props
            .get(k)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v > 0.0)
    };
    // Containers hug their children (auto) and grow into free space.
    // NOTE: Role::List MUST be here — a virtualized feed's panel sizes
    // to its visible window. Missing it pinned the feed to 32px and the
    // flex shrink then crushed every message row (caught by the M3 PNG).
    let is_container = matches!(
        comp.role,
        hamesh_core::Role::Window | hamesh_core::Role::Panel | hamesh_core::Role::List
    );
    let w = parse("w")
        .map(Dimension::length)
        .unwrap_or(Dimension::auto());
    // Text leaves measure their content (see measure_text); buttons
    // measure their label the same way so rows size them. Every other
    // leaf keeps the legacy 32px default so old screens don't shift.
    // Text is incompressible (measured width stays valid only there):
    // flex shrink must never squeeze labels, or paint wraps what
    // layout kept single-line. Siblings (spacers) absorb the pressure.
    let is_text = matches!(
        comp.role,
        hamesh_core::Role::Text | hamesh_core::Role::Status | hamesh_core::Role::Button
    );
    let h = match (parse("h"), is_container, is_text) {
        (Some(v), _, _) => Dimension::length(v),
        (None, true, _) => Dimension::auto(),
        (None, false, true) => Dimension::auto(),
        (None, false, false) => Dimension::length(32.0),
    };
    let grow = comp
        .props
        .get("flex_grow")
        .and_then(|v| v.parse::<f32>().ok())
        // CSS-correct default: nothing grows unless asked. The legacy
        // blanket grow=1 made same-role siblings (tiles, bars, icons)
        // fight over free space. Structural fill (feed, main panels)
        // opts in explicitly; Role::List keeps 1 (virtualized viewport).
        .unwrap_or(if matches!(comp.role, hamesh_core::Role::List) {
            1.0
        } else {
            0.0
        });
    // Cross-axis placement inside the parent: start|center|end|stretch
    // (default stretch preserves legacy behavior; badges opt out).
    // Taffy spells the variants START/CENTER/END/STRETCH.
    let align = match comp.props.get("align").map(|s| s.as_str()) {
        Some("start") => Some(AlignSelf::START),
        Some("center") => Some(AlignSelf::CENTER),
        Some("end") => Some(AlignSelf::END),
        _ => None,
    };
    let dir = match comp.props.get("layout").map(|s| s.as_str()) {
        Some("row") => FlexDirection::Row,
        _ => FlexDirection::Column,
    };
    // Text leaves hug their glyphs: default pad 0 (a label's box is its
    // line box; padding is the parent's job). Containers keep 12.
    // Explicit `pad` always wins — badges/buttons declare their own.
    let pad_default = if is_text { 0.0 } else { 12.0 };
    let pad = parse_nonneg("pad", pad_default);
    let gap = parse_nonneg("gap", 8.0);
    let wrap = comp.props.get("wrap").map(|s| s == "true").unwrap_or(false);
    // Zero is a meaningful base (share equally), so unlike `parse`
    // this accepts 0; absent stays auto.
    let basis = comp
        .props
        .get("basis")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v >= 0.0)
        .map(Dimension::length)
        .unwrap_or(Dimension::auto());
    // Readability floor: `min_w` keeps tiles/controls from crushing
    // below usable width, so wrapping rows break instead of squeezing.
    let min_w = comp
        .props
        .get("min_w")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v > 0.0)
        .map(taffy::style::LengthPercentageAuto::length)
        .unwrap_or(taffy::style::LengthPercentageAuto::AUTO);
    Style {
        size: Size {
            width: w,
            height: h,
        },
        min_size: Size {
            width: min_w,
            height: taffy::style::LengthPercentageAuto::AUTO,
        },
        flex_direction: dir,
        flex_grow: grow,
        flex_basis: basis,
        // Incompressible text: shrink would invalidate measurement.
        flex_shrink: if is_text { 0.0 } else { 1.0 },
        align_self: align,
        flex_wrap: if wrap {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        gap: Size {
            width: length(gap),
            height: length(gap),
        },
        padding: taffy::Rect {
            left: length(pad),
            right: length(pad),
            top: length(pad),
            bottom: length(pad),
        },
        ..Default::default()
    }
}

/// Compute layout for the whole tree at a fixed viewport size.
///
/// Text leaves (`Text`/`Status` roles) carry their content as Taffy
/// node context and are measured with parley, so labels size to
/// their text. Every other leaf measures zero (style sizes win).
pub fn compute(app: &App, viewport_w: f32, viewport_h: f32) -> LayoutOutput {
    let mut taffy: TaffyTree<Option<TextMeasure>> = TaffyTree::new();
    let mut node_of: HashMap<String, NodeId> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    // Post-order creation: children first so parents can reference them.
    fn build(
        app: &App,
        id: &str,
        taffy: &mut TaffyTree<Option<TextMeasure>>,
        node_of: &mut HashMap<String, NodeId>,
        order: &mut Vec<String>,
    ) {
        let comp = app.components.get(id).expect("missing component");
        let mut kids = Vec::new();
        for c in &comp.children {
            build(app, &c.0, taffy, node_of, order);
            kids.push(node_of[&c.0]);
        }
        let style = child_style(comp);
        let node = if kids.is_empty()
            && matches!(
                comp.role,
                hamesh_core::Role::Text | hamesh_core::Role::Status | hamesh_core::Role::Button
            ) {
            let font_size = comp
                .props
                .get("font_size")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(16.0);
            let line_height = comp
                .props
                .get("lh")
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| *v > 0.0);
            let text = comp.props.get("text").cloned().unwrap_or_default();
            taffy
                .new_leaf_with_context(
                    style,
                    Some(TextMeasure {
                        text,
                        font_size,
                        line_height,
                    }),
                )
                .expect("taffy text leaf")
        } else if kids.is_empty() {
            taffy.new_leaf(style).expect("taffy leaf")
        } else {
            taffy.new_with_children(style, &kids).expect("taffy node")
        };
        node_of.insert(id.to_string(), node);
        order.push(id.to_string());
    }

    build(app, &app.root.0, &mut taffy, &mut node_of, &mut order);

    let root_node = node_of[&app.root.0];
    // Root fills the viewport exactly: Taffy AvailableSpace alone does not
    // force an auto-sized root to expand, so pin it explicitly.
    {
        let mut root_style = taffy.style(root_node).expect("root style").clone();
        root_style.size = Size {
            width: Dimension::length(viewport_w),
            height: Dimension::length(viewport_h),
        };
        taffy.set_style(root_node, root_style).expect("root size");
    }
    taffy
        .compute_layout_with_measure(
            root_node,
            Size {
                width: AvailableSpace::Definite(viewport_w),
                height: AvailableSpace::Definite(viewport_h),
            },
            |inputs, _node, ctx, style| {
                taffy::compute::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, avail| measure_text(ctx.and_then(|c| c.as_ref()), known, avail),
                )
            },
        )
        .expect("taffy layout");

    // Parent rects are needed for clipped_by_parent detection.
    let mut nodes: HashMap<String, LayoutNode> = HashMap::new();
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        w: viewport_w,
        h: viewport_h,
    };

    // First pass: rects + effective visibility.
    for id in &order {
        let comp = &app.components[&id.clone()];
        let tnode = node_of[&id.clone()];
        let l = taffy.layout(tnode).expect("layout");
        // Taffy locations are parent-relative: accumulate absolute offset.
        let mut abs_x = l.location.x;
        let mut abs_y = l.location.y;
        {
            let mut cur = comp.parent.clone();
            while let Some(pid) = cur {
                if let Some(pnode) = node_of.get(&pid.0) {
                    if let Ok(pl) = taffy.layout(*pnode) {
                        abs_x += pl.location.x;
                        abs_y += pl.location.y;
                    }
                }
                cur = app.components.get(&pid.0).and_then(|pc| pc.parent.clone());
            }
        }
        let mut eff = comp.visible;
        let mut p = comp.parent.as_ref();
        while let Some(pid) = p {
            if let Some(pc) = app.components.get(&pid.0) {
                if !pc.visible {
                    eff = false;
                    break;
                }
                p = pc.parent.as_ref();
            } else {
                break;
            }
        }
        // Overlay components float above layout at a fixed spot with z boost.
        let is_overlay = comp
            .props
            .get("overlay")
            .map(|s| s == "true")
            .unwrap_or(false);
        let rect = if is_overlay {
            Rect {
                x: parse_or(comp.props.get("ox"), 200.0),
                y: parse_or(comp.props.get("oy"), 60.0),
                w: parse_or(comp.props.get("w"), 240.0),
                h: parse_or(comp.props.get("h"), 120.0),
            }
        } else {
            Rect {
                x: abs_x,
                y: abs_y,
                w: l.size.width,
                h: l.size.height,
            }
        };
        nodes.insert(
            id.clone(),
            LayoutNode {
                rect,
                effectively_visible: eff,
                clipped_by_parent: false, // second pass
                z: if is_overlay { 100 } else { depth(app, id) },
            },
        );
    }

    // Second pass: clipped_by_parent — rect extends beyond parent rect.
    for id in &order {
        let parent = app.components[&id.clone()].parent.clone();
        if let Some(pid) = parent {
            if let (Some(n), Some(p)) =
                (nodes.get(&id.clone()).cloned(), nodes.get(&pid.0).cloned())
            {
                let clipped = n.rect.x < p.rect.x
                    || n.rect.y < p.rect.y
                    || n.rect.x + n.rect.w > p.rect.x + p.rect.w + 1.0
                    || n.rect.y + n.rect.h > p.rect.y + p.rect.h + 1.0;
                if let Some(nn) = nodes.get_mut(&id.clone()) {
                    nn.clipped_by_parent = clipped;
                }
            }
        }
    }

    LayoutOutput { nodes, viewport }
}

fn parse_or(v: Option<&String>, d: f32) -> f32 {
    v.and_then(|s| s.parse::<f32>().ok()).unwrap_or(d)
}

fn depth(app: &App, id: &str) -> i32 {
    let mut d = 0;
    let mut cur = app.components.get(id).and_then(|c| c.parent.clone());
    while let Some(p) = cur {
        d += 1;
        cur = app.components.get(&p.0).and_then(|c| c.parent.clone());
    }
    d
}

/// Hit-test: topmost (highest z, then later in tree) effectively-visible,
/// enabled node containing the point. Returns component id or None.
pub fn hit_test(app: &App, layout: &LayoutOutput, x: f32, y: f32) -> Option<String> {
    let mut best: Option<(i32, String)> = None;
    for (id, n) in &layout.nodes {
        if !n.effectively_visible {
            continue;
        }
        // Ghost spacers hold layout space but never intercept the Hand.
        if app
            .components
            .get(id)
            .is_some_and(|c| c.props.get("ghost").map(|s| s == "true").unwrap_or(false))
        {
            continue;
        }
        if !n.rect.contains(x, y) {
            continue;
        }
        // Hidden-by-clipping still hit-tests at its (possibly partial) rect;
        // the automation layer reports the clipping as a diagnostic.
        match &best {
            Some((z, _)) if *z > n.z => {}
            _ => best = Some((n.z, id.clone())),
        }
    }
    // Disabled components still win the hit-test (a human would hit them and
    // nothing would happen); the automation layer reports "disabled".
    let _ = app;
    // Disabled components still win the hit-test (a human would hit them
    // and nothing would happen); only ghosts are skipped.
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hamesh_core::{Component, Role};

    fn sample() -> App {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("panel", Role::Panel, "main").with_prop("layout", "column"),
        );
        app.add(
            "panel",
            Component::new("btn", Role::Button, "Increment").with_prop("h", "36"),
        );
        app
    }

    #[test]
    fn layout_and_hit() {
        let app = sample();
        let out = compute(&app, 800.0, 600.0);
        let btn = out.get("btn").expect("btn layout");
        assert!(btn.rect.w > 0.0 && btn.rect.h > 0.0);
        let hit = hit_test(&app, &out, btn.rect.x + 2.0, btn.rect.y + 2.0);
        assert_eq!(hit.as_deref(), Some("btn"));
    }

    #[test]
    fn hidden_is_not_hittable() {
        let mut app = sample();
        app.get_mut("btn").unwrap().visible = false;
        let out = compute(&app, 800.0, 600.0);
        let btn = out.get("btn").unwrap();
        assert!(!btn.effectively_visible);
        let hit = hit_test(&app, &out, btn.rect.x + 2.0, btn.rect.y + 2.0);
        assert_ne!(hit.as_deref(), Some("btn"));
    }

    #[test]
    fn pad_zero_is_flush() {
        // pad=0 removes the container inset: child spans full width.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("panel", Role::Panel, "flush").with_prop("pad", "0"),
        );
        app.add(
            "panel",
            Component::new("kid", Role::Panel, "kid").with_prop("h", "20"),
        );
        let out = compute(&app, 400.0, 300.0);
        let kid = out.get("kid").unwrap();
        // Full width minus nothing: flush edges (root has its own pad,
        // so measure against the panel, not the viewport).
        let panel = out.get("panel").unwrap();
        assert!(
            (kid.rect.x - panel.rect.x).abs() < 0.5,
            "kid.x={} panel.x={}",
            kid.rect.x,
            panel.rect.x
        );
        assert!(
            (kid.rect.w - panel.rect.w).abs() < 1.0,
            "kid.w={} panel.w={}",
            kid.rect.w,
            panel.rect.w
        );
    }

    #[test]
    fn wrap_breaks_equal_tiles() {
        // A row of two 200px tiles in a 300px viewport wraps: the
        // second tile lands on the next line. Responsive metrics grid.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("row", Role::Panel, "tiles")
                .with_prop("layout", "row")
                .with_prop("wrap", "true")
                .with_prop("pad", "0")
                .with_prop("gap", "0"),
        );
        for i in 0..2 {
            app.add(
                "row",
                Component::new(&format!("t{i}"), Role::Panel, "tile")
                    .with_prop("w", "200")
                    .with_prop("h", "60"),
            );
        }
        let out = compute(&app, 300.0, 300.0);
        let t0 = out.get("t0").unwrap();
        let t1 = out.get("t1").unwrap();
        assert!(
            t1.rect.y > t0.rect.y + 10.0,
            "t1.y={} t0.y={}",
            t1.rect.y,
            t0.rect.y
        );
    }

    #[test]
    fn ghost_spacer_never_wins_hit() {
        // A ghost spacer holds layout space but must not intercept the
        // Hand: the hit falls through to whatever is beneath (here, root).
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("sp", Role::Panel, "spacer")
                .with_prop("ghost", "true")
                .with_prop("h", "60"),
        );
        let out = compute(&app, 300.0, 300.0);
        let sp = out.get("sp").unwrap();
        let hit = hit_test(&app, &out, sp.rect.x + 2.0, sp.rect.y + 2.0);
        assert_ne!(hit.as_deref(), Some("sp"));
    }

    #[test]
    fn text_leaf_measures_content() {
        // In a row, a text leaf's width comes from parley: a 28px
        // title measures wider than the same text at 12px, and
        // neither collapses while it has content. (In a column the
        // cross axis stretches, which is also correct.)
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("row", Role::Panel, "row").with_prop("layout", "row"),
        );
        app.add(
            "row",
            Component::new("big", Role::Text, "big")
                .with_prop("text", "Agent X")
                .with_prop("font_size", "28"),
        );
        app.add(
            "row",
            Component::new("small", Role::Text, "small")
                .with_prop("text", "Agent X")
                .with_prop("font_size", "12"),
        );
        let out = compute(&app, 600.0, 300.0);
        let big = out.get("big").unwrap().rect.w;
        let small = out.get("small").unwrap().rect.w;
        assert!(big > 40.0, "28px title must measure wide, got {big}");
        assert!(
            small > 10.0 && small < big,
            "12px narrower: {small} < {big}"
        );
    }

    #[test]
    fn text_leaf_defaults_to_no_padding() {
        // C1 root cause: text leaves carried the container default
        // pad 12 — 24px of phantom padding that dwarfed real gaps.
        // A label's box is now its line box; padding is the parent's
        // job. Explicit `pad` still applies (see pill test below).
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("t", Role::Text, "t")
                .with_prop("text", "Hi")
                .with_prop("font_size", "16"),
        );
        let out = compute(&app, 400.0, 300.0);
        let r = out.get("t").unwrap().rect.clone();
        let line = hamesh_text::TextLayout::new("Hi", f32::INFINITY, 16.0, None).height();
        assert!(
            (r.h - line).abs() < 1.0,
            "label box {r:?} must hug its line ({line})"
        );
    }

    #[test]
    fn declared_line_height_sizes_the_box() {
        // Ramp line heights are real: `lh` flows into parley and the
        // measured box matches it.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("t", Role::Text, "t")
                .with_prop("text", "Hi")
                .with_prop("font_size", "28")
                .with_prop("lh", "34"),
        );
        let out = compute(&app, 400.0, 300.0);
        let h = out.get("t").unwrap().rect.h;
        assert!((h - 34.0).abs() < 1.0, "lh=34 must size the box, got {h}");
    }

    #[test]
    fn text_leaf_refuses_flex_shrink() {
        // Paint/layout agreement: a heading beside a greedy spacer keeps
        // its measured width instead of squeezing (which wrapped paint
        // but not layout — the "Development" truncation).
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("row", Role::Panel, "row")
                .with_prop("layout", "row")
                .with_prop("pad", "0")
                .with_prop("gap", "0"),
        );
        app.add(
            "row",
            Component::new("h", Role::Text, "h")
                .with_prop("text", "Development progress")
                .with_prop("font_size", "18"),
        );
        app.add(
            "row",
            Component::new("sp", Role::Panel, "sp")
                .with_prop("ghost", "true")
                .with_prop("flex_grow", "1"),
        );
        let out = compute(&app, 744.0, 100.0);
        let w = out.get("h").unwrap().rect.w;
        let full =
            hamesh_text::TextLayout::new("Development progress", f32::INFINITY, 18.0, None).width();
        assert!(w >= full - 1.0, "heading kept {w}, measured {full}");
    }

    #[test]
    fn bar_height_six_not_padding() {
        // Painted thin leaves carry pad 0 (builders enforce it); an
        // h=6 leaf with no pad lays out exactly 6 tall.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("col", Role::Panel, "col").with_prop("layout", "column"),
        );
        app.add(
            "col",
            Component::new("bar", Role::Panel, "bar")
                .with_prop("h", "6")
                .with_prop("pad", "0"),
        );
        let out = compute(&app, 400.0, 300.0);
        assert_eq!(out.get("bar").unwrap().rect.h, 6.0);
    }

    #[test]
    fn explicit_h_still_wins_over_measure() {
        // Legacy screens pin text height: the measure must not override.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("s", Role::Status, "status")
                .with_prop("text", "counter=0")
                .with_prop("h", "28"),
        );
        let out = compute(&app, 400.0, 300.0);
        assert_eq!(out.get("s").unwrap().rect.h, 28.0);
    }

    #[test]
    fn pill_text_fits_inside_its_box() {
        // Matthew caught "Connected" crossing its pill border: the
        // renderer draws glyphs at +8px, so the measured box must
        // contain the full glyph run plus padding, not just parley.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("pill", Role::Status, "pill")
                .with_prop("text", "Connected")
                .with_prop("font_size", "13")
                .with_prop("pad", "4"),
        );
        let out = compute(&app, 400.0, 300.0);
        let w = out.get("pill").unwrap().rect.w;
        let glyphs = hamesh_text::TextLayout::new("Connected", f32::INFINITY, 13.0, None).width();
        assert!(
            w >= glyphs + 8.0,
            "pill box {w} must contain glyphs {glyphs} plus padding"
        );
    }
}
