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
    /// Font selection for shaping: resolved from the component's
    /// `font` / `font_vars` props at build time, so measurement and
    /// render use the identical spec (see FontSpec).
    pub shaping: hamesh_text::FontSpec,
}

impl PartialEq for TextMeasure {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.font_size.to_bits() == other.font_size.to_bits()
            && self.line_height.map(|v| v.to_bits()) == other.line_height.map(|v| v.to_bits())
            && self.shaping.fingerprint() == other.shaping.fingerprint()
    }
}

/// Measure a text leaf: known dimensions win, otherwise parley.
/// MinContent asks for the longest word (wrapping upper bound).
/// Width includes the renderer's text origin inset on both sides
/// (blit_text draws at rect.x + 8): measuring raw glyph width left
/// tight pills 8px too narrow and text crossed the border.
///
/// Public because trimmers (e.g. ellipsis) must budget the same
/// inset the measurer adds, or the node outgrows its allowance.
pub const TEXT_INSET_X: f32 = 8.0;
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
            .map(|wd| {
                hamesh_text::TextLayout::new_varied(
                    wd,
                    f32::INFINITY,
                    tm.font_size,
                    lh,
                    &tm.shaping,
                )
                .width()
            })
            .fold(0.0, f32::max),
    };
    let lay = hamesh_text::TextLayout::new_varied(
        &tm.text,
        max_w.max(0.0),
        tm.font_size,
        lh,
        &tm.shaping,
    );
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

    /// Overlap rectangle, or `None` when the rects are disjoint.
    /// Clip composition uses this: intersecting ancestor rects
    /// yields the region a descendant is still allowed to paint.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.w).min(other.x + other.w);
        let y1 = (self.y + self.h).min(other.y + other.h);
        if x1 > x0 && y1 > y0 {
            Some(Rect {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            })
        } else {
            None
        }
    }

    /// True when `inner` lies wholly inside this rect (0.5px slop
    /// absorbs float rounding from flex distribution).
    pub fn contains_rect(&self, inner: &Rect) -> bool {
        inner.x >= self.x - 0.5
            && inner.y >= self.y - 0.5
            && inner.x + inner.w <= self.x + self.w + 0.5
            && inner.y + inner.h <= self.y + self.h + 0.5
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutNode {
    pub rect: Rect,
    /// False when component.visible == false or an ancestor is hidden.
    pub effectively_visible: bool,
    /// True when the node's rect extends beyond its parent's rect.
    pub clipped_by_parent: bool,
    pub z: i32,
}

#[derive(Clone, Debug, PartialEq)]
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
/// `min_w` (readability floor: wrapping rows break instead of squeezing),
/// `scroll_y` (px of content hidden above the container top: the scroll
/// primitive — descendants shift up verbatim, the container keeps its
/// rect, and paint clips to it. Widgets clamp; layout does not).
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
        .unwrap_or_else(|| match parse("w") {
            // A declared width is a floor too: shrinking a pinned box
            // produces a control that is neither its own size nor
            // readable. Overflow is now named instead of hidden.
            Some(v) => taffy::style::LengthPercentageAuto::length(v),
            None => taffy::style::LengthPercentageAuto::AUTO,
        });
    // Same for height: an icon declaring h=20 used to collapse to 0px
    // inside an overflowing column, rendering as a dot. Declared sizes
    // are floors; only `auto` sizes compress.
    let min_h = match parse("h") {
        Some(v) => taffy::style::LengthPercentageAuto::length(v),
        None => taffy::style::LengthPercentageAuto::AUTO,
    };
    Style {
        size: Size {
            width: w,
            height: h,
        },
        min_size: Size {
            width: min_w,
            height: min_h,
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

#[derive(Clone, Debug, PartialEq)]
struct ComponentSnapshot {
    role: hamesh_core::Role,
    visible: bool,
    parent: Option<hamesh_core::ComponentId>,
    children: Vec<hamesh_core::ComponentId>,
    is_overlay: bool,
    style: Style,
    text_measure: Option<TextMeasure>,
}

fn is_text_leaf(comp: &hamesh_core::Component, has_kids: bool) -> bool {
    !has_kids
        && matches!(
            comp.role,
            hamesh_core::Role::Text | hamesh_core::Role::Status | hamesh_core::Role::Button
        )
}

fn extract_text_measure(comp: &hamesh_core::Component) -> TextMeasure {
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
    TextMeasure {
        text,
        font_size,
        line_height,
        shaping: hamesh_text::FontSpec::parse(
            comp.props.get("font").map(String::as_str),
            comp.props.get("font_vars").map(String::as_str),
            comp.props.get("font_style").map(String::as_str),
            comp.props.get("tracking").map(String::as_str),
        ),
    }
}

fn extract_layout_output(
    trees: &[TaffyTree<Option<TextMeasure>>],
    node_of: &HashMap<String, (usize, NodeId)>,
    order: &[String],
    app: &App,
    viewport_w: f32,
    viewport_h: f32,
) -> LayoutOutput {
    let mut nodes: HashMap<String, LayoutNode> = HashMap::with_capacity(order.len());
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        w: viewport_w,
        h: viewport_h,
    };

    // First pass: rects + effective visibility.
    for id in order {
        let comp = &app.components[id];
        let (tree_idx, tnode) = node_of[id];
        let l = trees[tree_idx].layout(tnode).expect("layout");
        let overlay_self = is_overlay(comp);
        // Taffy locations are parent-relative: accumulate absolute offset.
        // A scrolled ancestor (`scroll_y`) hides that many pixels of its
        // content above its top edge, so every descendant shifts up by
        // it. The container's own rect is untouched; paint clips to it
        // (scene clip truth) and the Eye names what was hidden.
        let mut abs_x = l.location.x;
        let mut abs_y = l.location.y;
        {
            let mut cur = comp.parent.clone();
            while let Some(pid) = cur {
                let Some(pc) = app.components.get(&pid.0) else {
                    break;
                };
                if is_overlay(pc) {
                    // Positioning-context boundary: the overlay's
                    // declared origin replaces everything above it.
                    // (Scroll above an overlay lives in another space.)
                    let (ox, oy) = overlay_origin(pc);
                    abs_x += ox;
                    abs_y += oy;
                    break;
                }
                if let Some((pti, pnode)) = node_of.get(&pid.0) {
                    if let Ok(pl) = trees[*pti].layout(*pnode) {
                        abs_x += pl.location.x;
                        abs_y += pl.location.y;
                    }
                }
                abs_y -= pc
                    .props
                    .get("scroll_y")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0)
                    .max(0.0);
                cur = pc.parent.clone();
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
        // Their subtree lays out in a separate tree against this rect,
        // so they take no space in their parent's flow.
        let rect = if overlay_self {
            let (ox, oy) = overlay_origin(comp);
            Rect {
                x: ox,
                y: oy,
                w: comp
                    .props
                    .get("w")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(l.size.width),
                h: comp
                    .props
                    .get("h")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(l.size.height),
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
                z: if overlay_self || under_overlay(app, id) {
                    100
                } else {
                    depth(app, id)
                },
            },
        );
    }

    // Second pass: clipped_by_parent — rect extends beyond parent rect.
    for id in order {
        if is_overlay(&app.components[id]) {
            continue;
        }
        let parent = app.components[id].parent.clone();
        if let Some(pid) = parent {
            if let (Some(n), Some(p)) = (nodes.get(id).cloned(), nodes.get(&pid.0).cloned()) {
                let clipped = n.rect.x < p.rect.x
                    || n.rect.y < p.rect.y
                    || n.rect.x + n.rect.w > p.rect.x + p.rect.w + 1.0
                    || n.rect.y + n.rect.h > p.rect.y + p.rect.h + 1.0;
                if let Some(nn) = nodes.get_mut(id) {
                    nn.clipped_by_parent = clipped;
                }
            }
        }
    }

    LayoutOutput { nodes, viewport }
}

struct BuildCtx<'a> {
    app: &'a App,
    trees: &'a mut Vec<TaffyTree<Option<TextMeasure>>>,
    node_of: &'a mut HashMap<String, (usize, NodeId)>,
    order: &'a mut Vec<String>,
    tree_roots: &'a mut Vec<String>,
    snapshots: Option<&'a mut HashMap<String, ComponentSnapshot>>,
}

fn build_tree_node(ctx: &mut BuildCtx, id: &str, tree_idx: usize) {
    let comp = ctx.app.components.get(id).expect("missing component");
    let mut kids = Vec::new();
    for c in &comp.children {
        let child = ctx.app.components.get(&c.0).expect("missing child");
        if is_overlay(child) {
            ctx.trees.push(TaffyTree::new());
            let overlay_idx = ctx.trees.len() - 1;
            ctx.tree_roots.push(c.0.clone());
            build_tree_node(ctx, &c.0, overlay_idx);
        } else {
            build_tree_node(ctx, &c.0, tree_idx);
            kids.push(ctx.node_of[&c.0].1);
        }
    }
    let style = child_style(comp);
    let has_kids = !kids.is_empty();
    let is_txt = is_text_leaf(comp, has_kids);
    let tm = if is_txt {
        Some(extract_text_measure(comp))
    } else {
        None
    };
    let node = if let Some(ref m) = tm {
        ctx.trees[tree_idx]
            .new_leaf_with_context(style.clone(), Some(m.clone()))
            .expect("taffy text leaf")
    } else if kids.is_empty() {
        ctx.trees[tree_idx]
            .new_leaf(style.clone())
            .expect("taffy leaf")
    } else {
        ctx.trees[tree_idx]
            .new_with_children(style.clone(), &kids)
            .expect("taffy node")
    };
    if let Some(snaps) = ctx.snapshots.as_mut() {
        snaps.insert(
            id.to_string(),
            ComponentSnapshot {
                role: comp.role.clone(),
                visible: comp.visible,
                parent: comp.parent.clone(),
                children: comp.children.clone(),
                is_overlay: is_overlay(comp),
                style,
                text_measure: tm,
            },
        );
    }
    ctx.node_of.insert(id.to_string(), (tree_idx, node));
    ctx.order.push(id.to_string());
}

/// Compute layout for the whole tree from scratch at a fixed viewport size.
/// Guaranteed full rebuild with no retained state.
pub fn compute_full(app: &App, viewport_w: f32, viewport_h: f32) -> LayoutOutput {
    let mut trees: Vec<TaffyTree<Option<TextMeasure>>> = vec![TaffyTree::new()];
    let mut node_of: HashMap<String, (usize, NodeId)> =
        HashMap::with_capacity(app.components.len());
    let mut order: Vec<String> = Vec::with_capacity(app.components.len());
    let mut tree_roots: Vec<String> = vec![app.root.0.clone()];

    {
        let mut ctx = BuildCtx {
            app,
            trees: &mut trees,
            node_of: &mut node_of,
            order: &mut order,
            tree_roots: &mut tree_roots,
            snapshots: None,
        };
        build_tree_node(&mut ctx, &app.root.0, 0);
    }

    {
        let root_node = node_of[&app.root.0].1;
        let mut root_style = trees[0].style(root_node).expect("root style").clone();
        root_style.size = Size {
            width: Dimension::length(viewport_w),
            height: Dimension::length(viewport_h),
        };
        trees[0]
            .set_style(root_node, root_style)
            .expect("root size");
    }
    for root_id in &tree_roots {
        let tnode = node_of[root_id].1;
        let ti = node_of[root_id].0;
        trees[ti]
            .compute_layout_with_measure(
                tnode,
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
    }

    extract_layout_output(&trees, &node_of, &order, app, viewport_w, viewport_h)
}

/// Persistent layout state retaining Taffy trees and incremental node invalidations.
///
/// Borrows the incremental layout technique from GPUI Fast and Taffy:
/// retains node mappings across frames, invalidates only changed styles or
/// text measurements, marks affected subtrees dirty, and lets Taffy skip
/// unaffected branches. Falls back cleanly to full rebuild whenever tree
/// topology (roots, children, overlays) changes.
#[derive(Default)]
pub struct LayoutCache {
    trees: Vec<TaffyTree<Option<TextMeasure>>>,
    node_of: HashMap<String, (usize, NodeId)>,
    order: Vec<String>,
    tree_roots: Vec<String>,
    snapshots: HashMap<String, ComponentSnapshot>,
    root_id: Option<String>,
    viewport: (f32, f32),
    font_gen: u64,
}

impl LayoutCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.trees.clear();
        self.node_of.clear();
        self.order.clear();
        self.tree_roots.clear();
        self.snapshots.clear();
        self.root_id = None;
        self.viewport = (0.0, 0.0);
        self.font_gen = 0;
    }

    fn rebuild(&mut self, app: &App, viewport_w: f32, viewport_h: f32) -> LayoutOutput {
        let mut trees: Vec<TaffyTree<Option<TextMeasure>>> = vec![TaffyTree::new()];
        let mut node_of: HashMap<String, (usize, NodeId)> =
            HashMap::with_capacity(app.components.len());
        let mut order: Vec<String> = Vec::with_capacity(app.components.len());
        let mut tree_roots: Vec<String> = vec![app.root.0.clone()];
        let mut snapshots: HashMap<String, ComponentSnapshot> =
            HashMap::with_capacity(app.components.len());

        {
            let mut ctx = BuildCtx {
                app,
                trees: &mut trees,
                node_of: &mut node_of,
                order: &mut order,
                tree_roots: &mut tree_roots,
                snapshots: Some(&mut snapshots),
            };
            build_tree_node(&mut ctx, &app.root.0, 0);
        }

        {
            let root_node = node_of[&app.root.0].1;
            let mut root_style = trees[0].style(root_node).expect("root style").clone();
            root_style.size = Size {
                width: Dimension::length(viewport_w),
                height: Dimension::length(viewport_h),
            };
            trees[0]
                .set_style(root_node, root_style)
                .expect("root size");
        }

        for root_id in &tree_roots {
            let tnode = node_of[root_id].1;
            let ti = node_of[root_id].0;
            trees[ti]
                .compute_layout_with_measure(
                    tnode,
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
        }

        let output = extract_layout_output(&trees, &node_of, &order, app, viewport_w, viewport_h);

        self.trees = trees;
        self.node_of = node_of;
        self.order = order;
        self.tree_roots = tree_roots;
        self.snapshots = snapshots;
        self.root_id = Some(app.root.0.clone());
        self.viewport = (viewport_w, viewport_h);
        self.font_gen = hamesh_text::font_generation();

        output
    }

    pub fn compute(&mut self, app: &App, viewport_w: f32, viewport_h: f32) -> LayoutOutput {
        let topology_matches = self.root_id.as_deref() == Some(&app.root.0)
            && self.order.len() == app.components.len()
            && self.order.iter().all(|id| {
                if let (Some(comp), Some(snap)) = (app.components.get(id), self.snapshots.get(id)) {
                    snap.role == comp.role
                        && snap.visible == comp.visible
                        && snap.parent == comp.parent
                        && snap.children == comp.children
                        && snap.is_overlay == is_overlay(comp)
                } else {
                    false
                }
            });

        if !topology_matches {
            return self.rebuild(app, viewport_w, viewport_h);
        }

        // Incremental path: topology is intact.
        let vp_changed = self.viewport != (viewport_w, viewport_h);
        if vp_changed {
            let root_node = self.node_of[&app.root.0].1;
            let mut root_style = self.trees[0].style(root_node).expect("root style").clone();
            root_style.size = Size {
                width: Dimension::length(viewport_w),
                height: Dimension::length(viewport_h),
            };
            self.trees[0]
                .set_style(root_node, root_style)
                .expect("root size");
            self.viewport = (viewport_w, viewport_h);
        }

        let current_font_gen = hamesh_text::font_generation();
        let font_gen_changed = self.font_gen != current_font_gen;
        if font_gen_changed {
            self.font_gen = current_font_gen;
        }

        for id in &self.order {
            let comp = &app.components[id];
            let (tree_idx, node) = self.node_of[id];
            let snap = self.snapshots.get_mut(id).unwrap();

            let new_style = child_style(comp);
            if new_style != snap.style {
                self.trees[tree_idx]
                    .set_style(node, new_style.clone())
                    .expect("set_style");
                snap.style = new_style;
            }

            if snap.text_measure.is_some() {
                let new_tm = extract_text_measure(comp);
                if font_gen_changed || snap.text_measure.as_ref() != Some(&new_tm) {
                    self.trees[tree_idx]
                        .set_node_context(node, Some(Some(new_tm.clone())))
                        .expect("set_context");
                    let _ = self.trees[tree_idx].mark_dirty(node);
                    snap.text_measure = Some(new_tm);
                }
            }
        }

        for root_id in &self.tree_roots {
            let tnode = self.node_of[root_id].1;
            let ti = self.node_of[root_id].0;
            self.trees[ti]
                .compute_layout_with_measure(
                    tnode,
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
        }

        extract_layout_output(
            &self.trees,
            &self.node_of,
            &self.order,
            app,
            viewport_w,
            viewport_h,
        )
    }
}

thread_local! {
    static THREAD_LAYOUT_CACHE: std::cell::RefCell<LayoutCache> = std::cell::RefCell::new(LayoutCache::new());
}

/// Compute layout for the whole tree, leveraging persistent incremental caching
/// across frames while preserving exact geometric results.
pub fn compute(app: &App, viewport_w: f32, viewport_h: f32) -> LayoutOutput {
    THREAD_LAYOUT_CACHE.with(|c| c.borrow_mut().compute(app, viewport_w, viewport_h))
}

/// Compute layout using an explicit caller-owned `LayoutCache`.
pub fn compute_with_cache(
    cache: &mut LayoutCache,
    app: &App,
    viewport_w: f32,
    viewport_h: f32,
) -> LayoutOutput {
    cache.compute(app, viewport_w, viewport_h)
}

/// Out-of-flow overlay: zero parent footprint, positioned by ox/oy/w/h.
fn is_overlay(comp: &hamesh_core::Component) -> bool {
    comp.props
        .get("overlay")
        .map(|s| s == "true")
        .unwrap_or(false)
}

/// Declared overlay origin (viewport space).
fn overlay_origin(comp: &hamesh_core::Component) -> (f32, f32) {
    (
        parse_or(comp.props.get("ox"), 200.0),
        parse_or(comp.props.get("oy"), 60.0),
    )
}

/// True when an overlay sits between this node and the root: the node
/// paints, clips and hit-tests in overlay space.
fn under_overlay(app: &App, id: &str) -> bool {
    let mut cur = app.components.get(id).and_then(|c| c.parent.clone());
    while let Some(p) = cur {
        let Some(pc) = app.components.get(&p.0) else {
            break;
        };
        if is_overlay(pc) {
            return true;
        }
        cur = pc.parent.clone();
    }
    false
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

/// Hit-test on layout rects: topmost (highest z, then hash order)
/// effectively-visible, enabled node containing the point.
///
/// Geometry only: layout rects survive scrolling and clipping, so a
/// scrolled-out row still "contains" its old point. Production input
/// (automation, Eye probes, application drivers) must use
/// `hamesh_scene::hit_test`, which intersects the paint clip. Kept
/// for unit tests of pure geometry.
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
    fn declared_height_is_a_floor_when_the_column_overflows() {
        // Field defect: a 20px icon inside a metric tile collapsed to
        // 0px height because the column had less room than its content,
        // so the icon painted as a dot. Declared sizes are floors now:
        // the box overflows (named by the Eye) instead of vanishing.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("tile", Role::Panel, "tile")
                .with_prop("layout", "column")
                .with_prop("w", "120")
                .with_prop("h", "40")
                .with_prop("pad", "0")
                .with_prop("gap", "0"),
        );
        app.add(
            "tile",
            Component::new("ico", Role::Panel, "icon")
                .with_prop("w", "20")
                .with_prop("h", "20")
                .with_prop("pad", "0"),
        );
        for i in 0..2 {
            app.add(
                "tile",
                Component::new(&format!("t{i}"), Role::Text, "text")
                    .with_prop("text", "VALUE")
                    .with_prop("h", "40")
                    .with_prop("pad", "0"),
            );
        }
        let layout = compute(&app, 400.0, 300.0);
        let ico = layout.get("ico").unwrap().rect.clone();
        assert!(
            (ico.h - 20.0).abs() < 0.5,
            "declared 20px height must survive overflow, got {}",
            ico.h
        );
        assert!((ico.w - 20.0).abs() < 0.5, "declared width is a floor");
    }

    #[test]
    fn scroll_y_shifts_descendants_but_not_the_container() {
        // The scroll primitive: a fixed-height viewport with three
        // 40px rows and scroll_y=50 shows rows shifted up by 50 — the
        // container keeps its own rect.
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("view", Role::Panel, "view")
                .with_prop("layout", "column")
                .with_prop("h", "80")
                .with_prop("pad", "0")
                .with_prop("gap", "0")
                .with_prop("scroll_y", "50"),
        );
        for i in ["r0", "r1", "r2"] {
            app.add(
                "view",
                Component::new(i, Role::Panel, i)
                    .with_prop("h", "40")
                    .with_prop("text", i)
                    .with_prop("font_size", "13"),
            );
        }
        let out = compute(&app, 400.0, 300.0);
        let view = out.get("view").unwrap().rect.clone();
        assert_eq!((view.x, view.y, view.h), (0.0, 0.0, 80.0));
        for (i, top) in [("r0", -50.0), ("r1", -10.0), ("r2", 30.0)] {
            let r = out.get(i).unwrap().rect.clone();
            assert_eq!(r.y, top, "row {i} shifted by scroll_y");
            assert_eq!(r.h, 40.0);
        }
    }

    #[test]
    fn scroll_y_stacks_through_nested_containers() {
        // Nested scrolled viewports accumulate: outer hides 20px,
        // inner hides 10px, the leaf shifts by 30.
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("outer", Role::Panel, "outer")
                .with_prop("layout", "column")
                .with_prop("h", "200")
                .with_prop("pad", "0")
                .with_prop("gap", "0")
                .with_prop("scroll_y", "20"),
        );
        app.add(
            "outer",
            Component::new("inner", Role::Panel, "inner")
                .with_prop("layout", "column")
                .with_prop("h", "100")
                .with_prop("pad", "0")
                .with_prop("gap", "0")
                .with_prop("scroll_y", "10"),
        );
        app.add(
            "inner",
            Component::new("leaf", Role::Panel, "leaf").with_prop("h", "40"),
        );
        let out = compute(&app, 400.0, 300.0);
        assert_eq!(out.get("outer").unwrap().rect.y, 0.0);
        assert_eq!(out.get("inner").unwrap().rect.y, -20.0);
        assert_eq!(out.get("leaf").unwrap().rect.y, -30.0);
    }

    #[test]
    fn overlay_takes_no_flow_space() {
        // Out-of-flow contract: a 500px overlay must not move or
        // shrink its in-flow siblings.
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("panel", Role::Panel, "panel").with_prop("h", "100"),
        );
        app.add(
            "win",
            Component::new("pop", Role::Panel, "pop")
                .with_prop("overlay", "true")
                .with_prop("ox", "50")
                .with_prop("oy", "70")
                .with_prop("w", "500")
                .with_prop("h", "500"),
        );
        let out = compute(&app, 400.0, 600.0);
        let panel = out.get("panel").unwrap().rect.clone();
        assert_eq!(
            (panel.x, panel.y, panel.w, panel.h),
            (0.0, 0.0, 400.0, 100.0)
        );
        let pop = out.get("pop").unwrap().rect.clone();
        assert_eq!((pop.x, pop.y, pop.w, pop.h), (50.0, 70.0, 500.0, 500.0));
    }

    #[test]
    fn overlay_children_position_in_overlay_space() {
        // The popup panel's rows lay out against the overlay rect,
        // not against the root flow.
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("pop", Role::Panel, "pop")
                .with_prop("overlay", "true")
                .with_prop("ox", "50")
                .with_prop("oy", "70")
                .with_prop("w", "200")
                .with_prop("h", "200")
                .with_prop("pad", "0")
                .with_prop("gap", "0"),
        );
        app.add(
            "pop",
            Component::new("row", Role::Panel, "row").with_prop("h", "40"),
        );
        let out = compute(&app, 400.0, 600.0);
        let row = out.get("row").unwrap().rect.clone();
        assert_eq!((row.x, row.y, row.w, row.h), (50.0, 70.0, 200.0, 40.0));
        assert_eq!(
            out.get("row").unwrap().z,
            100,
            "popup content paints on top"
        );
    }

    #[test]
    fn overlay_child_clips_to_the_overlay_rect() {
        // A row taller than the popup is honestly clipped to it.
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("pop", Role::Panel, "pop")
                .with_prop("overlay", "true")
                .with_prop("ox", "50")
                .with_prop("oy", "70")
                .with_prop("w", "200")
                .with_prop("h", "100")
                .with_prop("pad", "0")
                .with_prop("gap", "0"),
        );
        app.add(
            "pop",
            Component::new("row", Role::Panel, "row").with_prop("h", "300"),
        );
        let out = compute(&app, 400.0, 600.0);
        assert!(out.get("row").unwrap().clipped_by_parent);
        assert!(
            !out.get("pop").unwrap().clipped_by_parent,
            "roots answer to no parent"
        );
    }

    #[test]
    fn nested_overlay_positions_independently() {
        // An overlay inside an overlay still anchors to the viewport:
        // ox/oy are always viewport space.
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("outer", Role::Panel, "outer")
                .with_prop("overlay", "true")
                .with_prop("ox", "10")
                .with_prop("oy", "20")
                .with_prop("w", "300")
                .with_prop("h", "300")
                .with_prop("pad", "0")
                .with_prop("gap", "0"),
        );
        app.add(
            "outer",
            Component::new("inner", Role::Panel, "inner")
                .with_prop("overlay", "true")
                .with_prop("ox", "100")
                .with_prop("oy", "120")
                .with_prop("w", "50")
                .with_prop("h", "50"),
        );
        let out = compute(&app, 400.0, 600.0);
        let inner = out.get("inner").unwrap().rect.clone();
        assert_eq!(
            (inner.x, inner.y, inner.w, inner.h),
            (100.0, 120.0, 50.0, 50.0)
        );
    }

    #[test]
    fn undeclared_overlay_hugs_content() {
        // No w/h: shrink-wrap the content instead of the legacy
        // 240x120 fallback.
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("tip", Role::Panel, "tip")
                .with_prop("overlay", "true")
                .with_prop("ox", "10")
                .with_prop("oy", "10")
                .with_prop("pad", "0")
                .with_prop("gap", "0"),
        );
        app.add(
            "tip",
            Component::new("t", Role::Text, "t")
                .with_prop("text", "Hi")
                .with_prop("font_size", "16"),
        );
        let out = compute(&app, 400.0, 600.0);
        let tip = out.get("tip").unwrap().rect.clone();
        assert!(tip.w > 0.0 && tip.w < 100.0, "hugs the label: {tip:?}");
        assert!(tip.h > 0.0 && tip.h < 100.0, "hugs the line: {tip:?}");
    }

    #[test]
    fn scroll_y_without_a_prop_is_zero_and_negative_clamps() {
        let mut app = App::new("win");
        app.get_mut("win")
            .unwrap()
            .props
            .insert("pad".to_string(), "0".to_string());
        app.get_mut("win")
            .unwrap()
            .props
            .insert("gap".to_string(), "0".to_string());
        app.add(
            "win",
            Component::new("view", Role::Panel, "view")
                .with_prop("layout", "column")
                .with_prop("h", "80")
                .with_prop("pad", "0")
                .with_prop("gap", "0")
                .with_prop("scroll_y", "-25"),
        );
        app.add(
            "view",
            Component::new("r0", Role::Panel, "r0").with_prop("h", "40"),
        );
        let out = compute(&app, 400.0, 300.0);
        assert_eq!(out.get("r0").unwrap().rect.y, 0.0);
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

    #[test]
    fn differential_incremental_matches_full_rebuild_all_operations() {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("header", Role::Header, "Header")
                .with_prop("h", "48")
                .with_prop("text", "Initial Header"),
        );
        app.add(
            "win",
            Component::new("main", Role::Panel, "Main")
                .with_prop("layout", "column")
                .with_prop("flex_grow", "1")
                .with_prop("gap", "8"),
        );
        app.add(
            "main",
            Component::new("counter", Role::Text, "Counter")
                .with_prop("text", "Count: 0")
                .with_prop("font_size", "16"),
        );
        app.add(
            "main",
            Component::new("btn", Role::Button, "Button")
                .with_prop("text", "Submit")
                .with_prop("h", "36"),
        );
        app.add(
            "win",
            Component::new("popup", Role::Overlay, "Popup")
                .with_prop("overlay", "true")
                .with_prop("ox", "50")
                .with_prop("oy", "50")
                .with_prop("w", "200")
                .with_prop("h", "100")
                .with_prop("text", "Modal info"),
        );

        let mut cache = LayoutCache::new();

        // 1. Initial layout: incremental cache output MUST bitwise match compute_full
        let initial_inc = cache.compute(&app, 800.0, 600.0);
        let initial_full = compute_full(&app, 800.0, 600.0);
        assert_eq!(
            initial_inc, initial_full,
            "initial incremental layout matches full rebuild"
        );

        // 2. Incremental text mutations: 20 sequential updates
        for i in 1..=20 {
            app.get_mut("counter")
                .unwrap()
                .props
                .insert("text".to_string(), format!("Count: {i} - active"));
            let inc = cache.compute(&app, 800.0, 600.0);
            let full = compute_full(&app, 800.0, 600.0);
            assert_eq!(inc, full, "text mutation at step {i} matches full rebuild");
        }

        // 3. Incremental style mutations: flex_grow, heights, padding, gap
        for pad in [0, 4, 8, 16] {
            app.get_mut("main")
                .unwrap()
                .props
                .insert("pad".to_string(), pad.to_string());
            app.get_mut("btn")
                .unwrap()
                .props
                .insert("h".to_string(), (36 + pad).to_string());
            let inc = cache.compute(&app, 800.0, 600.0);
            let full = compute_full(&app, 800.0, 600.0);
            assert_eq!(
                inc, full,
                "style mutation at pad {pad} matches full rebuild"
            );
        }

        // 4. Viewport resizes
        for (w, h) in [
            (1024.0, 768.0),
            (1280.0, 800.0),
            (1920.0, 1080.0),
            (800.0, 600.0),
        ] {
            let inc = cache.compute(&app, w, h);
            let full = compute_full(&app, w, h);
            assert_eq!(inc, full, "resize to {w}x{h} matches full rebuild");
        }

        // 5. Scrolling (scroll_y)
        for sy in [0, 20, 50, 120] {
            app.get_mut("main")
                .unwrap()
                .props
                .insert("scroll_y".to_string(), sy.to_string());
            let inc = cache.compute(&app, 800.0, 600.0);
            let full = compute_full(&app, 800.0, 600.0);
            assert_eq!(inc, full, "scroll_y {sy} matches full rebuild");
        }

        // 6. Dynamic child addition (topology expansion)
        app.add(
            "main",
            Component::new("extra_label", Role::Text, "Extra")
                .with_prop("text", "Dynamically added child")
                .with_prop("h", "24"),
        );
        let inc = cache.compute(&app, 800.0, 600.0);
        let full = compute_full(&app, 800.0, 600.0);
        assert_eq!(inc, full, "topology addition matches full rebuild");

        // 7. Dynamic child removal (topology retraction)
        app.components.remove("extra_label");
        app.get_mut("main")
            .unwrap()
            .children
            .retain(|c| c.0 != "extra_label");
        let inc = cache.compute(&app, 800.0, 600.0);
        let full = compute_full(&app, 800.0, 600.0);
        assert_eq!(inc, full, "topology removal matches full rebuild");
    }
}
