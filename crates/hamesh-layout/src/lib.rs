//! hamesh-layout: constraints + Flex layout via Taffy, with visibility/clipping truth.
//!
//! The layout crate owns computed geometry. It never duplicates component
//! semantics; it resolves them into rectangles plus visibility facts the Eye
//! needs (outside viewport, clipped by parent, hidden).

use hamesh_core::App;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use taffy::prelude::*;

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
/// `wrap: true` (flex-wrap for responsive equal-tile rows).
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
    let h = match (parse("h"), is_container) {
        (Some(v), _) => Dimension::length(v),
        (None, true) => Dimension::auto(),
        (None, false) => Dimension::length(32.0),
    };
    let grow = comp
        .props
        .get("flex_grow")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(if is_container { 1.0 } else { 0.0 });
    let dir = match comp.props.get("layout").map(|s| s.as_str()) {
        Some("row") => FlexDirection::Row,
        _ => FlexDirection::Column,
    };
    let pad = parse_nonneg("pad", 12.0);
    let gap = parse_nonneg("gap", 8.0);
    let wrap = comp.props.get("wrap").map(|s| s == "true").unwrap_or(false);
    Style {
        size: Size {
            width: w,
            height: h,
        },
        flex_direction: dir,
        flex_grow: grow,
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
pub fn compute(app: &App, viewport_w: f32, viewport_h: f32) -> LayoutOutput {
    let mut taffy: TaffyTree<()> = TaffyTree::new();
    let mut node_of: HashMap<String, NodeId> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    // Post-order creation: children first so parents can reference them.
    fn build(
        app: &App,
        id: &str,
        taffy: &mut TaffyTree<()>,
        node_of: &mut HashMap<String, NodeId>,
        order: &mut Vec<String>,
    ) {
        let comp = app.components.get(id).expect("missing component");
        let mut kids = Vec::new();
        for c in &comp.children {
            build(app, &c.0, taffy, node_of, order);
            kids.push(node_of[&c.0]);
        }
        let node = taffy
            .new_with_children(child_style(comp), &kids)
            .expect("taffy node");
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
        .compute_layout(
            root_node,
            Size {
                width: AvailableSpace::Definite(viewport_w),
                height: AvailableSpace::Definite(viewport_h),
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
}
