//! hamesh-inspect: the Eye. Machine-readable views of the live interface.
//!
//! Progressive disclosure: `compact_map` for the whole window, `detail` for
//! one component, `frame_info` to tie screenshots to revisions.

use hamesh_core::App;
use hamesh_layout::LayoutOutput;
use hamesh_scene::Scene;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapNode {
    pub id: String,
    pub role: String,
    pub name: String,
    pub parent: Option<String>,
    pub rect: hamesh_layout::Rect,
    pub visible: bool,
    pub clipped: bool,
    /// Rect escapes the effective clip and paint is therefore cut.
    pub overflow: bool,
    pub enabled: bool,
    pub z: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactMap {
    pub app_revision: u64,
    pub scene_revision: u64,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub nodes: Vec<MapNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentDetail {
    pub id: String,
    pub role: String,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub effectively_visible: bool,
    pub clipped_by_parent: bool,
    pub rect: Option<hamesh_layout::Rect>,
    pub z: Option<i32>,
    pub props: std::collections::HashMap<String, String>,
    /// Canonical text geometry (CorelDRAW-style selection model):
    /// layout rect + line box are separate from font trim, ink and
    /// baseline. Agents check centering/alignment from THESE, never
    /// by eyeballing pixels. `None` = non-text node.
    pub text_geom: Option<TextGeom>,
    pub state_keys: Vec<String>,
    pub diagnostics: Vec<String>,
    /// Parent component id: one hop of the ancestry a pixel belongs to.
    pub parent: Option<String>,
    /// Effective paint clip (intersection of ancestor rects + viewport).
    pub clip: Option<hamesh_layout::Rect>,
    /// True when the rect escapes the clip and paint is cut.
    pub overflow: bool,
}

pub fn compact_map(app: &App, _layout: &LayoutOutput, scene: &Scene) -> CompactMap {
    CompactMap {
        app_revision: app.revision,
        scene_revision: scene.scene_revision,
        viewport_w: scene.viewport_w,
        viewport_h: scene.viewport_h,
        nodes: scene
            .nodes
            .iter()
            .map(|n| MapNode {
                id: n.id.clone(),
                role: n.role.clone(),
                name: n.name.clone(),
                parent: n.parent.clone(),
                rect: n.rect.clone(),
                visible: n.visible,
                clipped: n.clipped,
                overflow: n.overflow,
                enabled: n.enabled,
                z: n.z,
            })
            .collect(),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextGeom {
    /// Node-local ink bounds [x0,y0,x1,y1]: what glyphs paint.
    pub ink: Option<[f32; 4]>,
    /// Node-local font trim [x0,y0,x1,y1]: cap top to baseline.
    /// Renderers center this; stable across text changes.
    pub trim: Option<[f32; 4]>,
    /// Font metrics [ascent, descent, cap, line_height] in px.
    pub metrics: Option<[f32; 4]>,
    /// Node-local first baseline y. Shared baselines = aligned text.
    pub baseline: Option<f32>,
    /// Measured line count at the final width. 1 = paint lays the
    /// run unwrapped (never re-wraps layout); >1 = intentional wrap.
    pub lines: usize,
    /// Resolved glyph-run origin (node-local): FINAL painted placement
    /// shared by CPU/GPU. Agents check centering against this.
    pub origin: Option<[f32; 2]>,
    /// Content rectangle (node-local): padding-declared usable area.
    pub content: Option<[f32; 4]>,
    /// Resolved horizontal alignment: start|center|end.
    pub halign: Option<String>,
    /// Shaped advance width (px) the origin was resolved against.
    pub advance: Option<f32>,
}

pub fn detail(
    app: &App,
    layout: &LayoutOutput,
    scene: Option<&Scene>,
    id: &str,
) -> Option<ComponentDetail> {
    let c = app.components.get(id)?;
    let l = layout.nodes.get(id);
    let mut diagnostics = Vec::new();
    if !c.enabled {
        diagnostics.push(
            "component is disabled: human-equivalent clicks will hit it but have no effect"
                .to_string(),
        );
    }
    if !c.visible {
        diagnostics.push("component.visible == false: not painted, not hit-testable".to_string());
    }
    if let Some(n) = l {
        if !n.effectively_visible {
            diagnostics.push(
                "effectively invisible via ancestor: not painted, not hit-testable".to_string(),
            );
        }
        if n.clipped_by_parent {
            diagnostics.push(
                "rect extends beyond parent rect: partially/fully clipped, may be unreachable"
                    .to_string(),
            );
        }
    } else {
        diagnostics.push("no layout node: outside computed tree".to_string());
    }
    let sn = scene.and_then(|s| s.nodes.iter().find(|n| n.id == id));
    if let Some(n) = sn {
        if n.overflow {
            let shown = n
                .rect
                .intersect(&n.clip)
                .map(|r| format!("{:.0}x{:.0}", r.w, r.h))
                .unwrap_or_else(|| "nothing".to_string());
            diagnostics.push(format!(
                "paint clipped by ancestor bounds: {shown} of {:.0}x{:.0} visible; content outside the panel is hidden (fix the layout, not the clip)",
                n.rect.w, n.rect.h
            ));
        }
    }
    if let Some(n) = sn {
        if n.text.is_empty() && n.trim.is_none() {
            diagnostics.push("no text content: text_geom is None".to_string());
        }
    }
    Some(ComponentDetail {
        id: id.to_string(),
        role: format!("{:?}", c.role),
        name: c.name.clone(),
        enabled: c.enabled,
        visible: c.visible,
        effectively_visible: l.map(|n| n.effectively_visible).unwrap_or(false),
        clipped_by_parent: l.map(|n| n.clipped_by_parent).unwrap_or(false),
        rect: l.map(|n| n.rect.clone()),
        z: l.map(|n| n.z),
        props: c.props.clone(),
        text_geom: sn.map(|n| TextGeom {
            ink: n.ink,
            trim: n.trim,
            metrics: n.metrics,
            baseline: n.baseline,
            lines: n.lines,
            origin: Some(n.origin),
            content: Some(n.content),
            halign: Some(n.halign.clone()),
            advance: Some(n.advance),
        }),
        state_keys: app.state.keys().cloned().collect(),
        diagnostics,
        parent: sn.and_then(|n| n.parent.clone()),
        clip: sn.map(|n| n.clip.clone()),
        overflow: sn.map(|n| n.overflow).unwrap_or(false),
    })
}

/// One node painting at a probed point, in paint order (last = topmost).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeHit {
    pub id: String,
    pub role: String,
    pub name: String,
    pub parent: Option<String>,
    pub z: i32,
    /// Position in paint order within the scene (0 = painted first).
    pub paint_index: usize,
    /// True when this node's paint actually reaches the point: inside
    /// both its rect and its ancestor clip.
    pub paints_here: bool,
}

/// Pixels -> components. Given a point in a screenshot, report what
/// paints there, in order, plus what a click would reach. This is the
/// reverse of `map`: the mapper's entry point when an agent sees a
/// problem in one region and needs its identity and telemetry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Probe {
    pub x: f32,
    pub y: f32,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub paint_order: Vec<ProbeHit>,
    /// Topmost node whose paint reaches the point (what the eye sees).
    pub topmost: Option<String>,
    /// Framework hit-test result (what a click would reach).
    pub hit: Option<String>,
}

/// Probe the scene at `(x, y)`.
pub fn probe(app: &App, layout: &LayoutOutput, scene: &Scene, x: f32, y: f32) -> Probe {
    let mut paint_order = Vec::new();
    for (i, n) in scene.nodes.iter().enumerate() {
        if !n.rect.contains(x, y) {
            continue;
        }
        let paints_here = n.visible && !n.ghost && n.clip.contains(x, y);
        paint_order.push(ProbeHit {
            id: n.id.clone(),
            role: n.role.clone(),
            name: n.name.clone(),
            parent: n.parent.clone(),
            z: n.z,
            paint_index: i,
            paints_here,
        });
    }
    let topmost = paint_order
        .iter()
        .rev()
        .find(|h| h.paints_here)
        .map(|h| h.id.clone());
    Probe {
        x,
        y,
        viewport_w: scene.viewport_w,
        viewport_h: scene.viewport_h,
        paint_order,
        topmost,
        hit: hamesh_layout::hit_test(app, layout, x, y),
    }
}

/// A later-painted node covering an earlier one.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Overlap {
    pub under: String,
    pub over: String,
    /// The covering region, in viewport coordinates.
    pub area: hamesh_layout::Rect,
}

/// Whole-window visual truth an agent checks before trusting a frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualReport {
    pub viewport_w: f32,
    pub viewport_h: f32,
    /// Painted nodes whose rect escapes their ancestor clip: content is
    /// being cut. Prevention keeps it invisible, so this names it.
    pub clipped_content: Vec<String>,
    /// Rect escapes the *viewport*: unreachable by hand.
    pub off_viewport: Vec<String>,
    /// Pairs where a later sibling paints over an earlier one. Sibling
    /// overlap is the expensive silent defect: one element hides another
    /// and layout tests still pass.
    pub overlaps: Vec<Overlap>,
}

fn is_ancestor(scene: &Scene, maybe_ancestor: &str, id: &str) -> bool {
    let mut cursor = scene
        .nodes
        .iter()
        .find(|n| n.id == id)
        .and_then(|n| n.parent.clone());
    let mut guard = 0;
    while let Some(pid) = cursor {
        if pid == maybe_ancestor {
            return true;
        }
        guard += 1;
        if guard > 64 {
            break;
        }
        cursor = scene
            .nodes
            .iter()
            .find(|n| n.id == pid)
            .and_then(|n| n.parent.clone());
    }
    false
}

/// Scan the whole scene for clipped content, off-viewport nodes and
/// sibling overlaps. Deterministic and cheap enough to run per frame
/// at inspection time (not in the paint path).
pub fn visual_report(scene: &Scene) -> VisualReport {
    let viewport = hamesh_layout::Rect {
        x: 0.0,
        y: 0.0,
        w: scene.viewport_w,
        h: scene.viewport_h,
    };
    let painted: Vec<&hamesh_scene::SceneNode> = scene
        .nodes
        .iter()
        .filter(|n| n.visible && !n.ghost)
        .collect();
    let mut clipped_content = Vec::new();
    let mut off_viewport = Vec::new();
    for n in &painted {
        if n.overflow {
            clipped_content.push(n.id.clone());
        }
        if !viewport.contains_rect(&n.rect) {
            off_viewport.push(n.id.clone());
        }
    }
    let mut overlaps = Vec::new();
    for (i, a) in painted.iter().enumerate() {
        for b in painted.iter().skip(i + 1) {
            // Ancestor/descendant containment is intended structure.
            if is_ancestor(scene, &a.id, &b.id) || is_ancestor(scene, &b.id, &a.id) {
                continue;
            }
            if let Some(area) = a.rect.intersect(&b.rect) {
                overlaps.push(Overlap {
                    under: a.id.clone(),
                    over: b.id.clone(),
                    area,
                });
            }
        }
    }
    VisualReport {
        viewport_w: scene.viewport_w,
        viewport_h: scene.viewport_h,
        clipped_content,
        off_viewport,
        overlaps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hamesh_core::{Component, Role};

    #[test]
    fn map_and_detail() {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("b1", Role::Button, "Increment").with_prop("text", "Increment"),
        );
        let layout = hamesh_layout::compute(&app, 400.0, 300.0);
        let scene = hamesh_scene::build(&app, &layout, 3);
        let map = compact_map(&app, &layout, &scene);
        assert!(map.nodes.iter().any(|n| n.id == "b1"));
        let d = detail(&app, &layout, Some(&scene), "b1").unwrap();
        assert!(d.effectively_visible);
        assert!(d.diagnostics.is_empty());
        // Buttons carry font trim (cap-alphabetic) for the Eye.
        let g = d.text_geom.expect("button text_geom");
        assert!(g.trim.is_some(), "button trim missing");
        assert!(g.baseline.is_some(), "button baseline missing");
        assert!(g.metrics.is_some(), "button metrics missing");
    }

    #[test]
    fn probe_maps_a_point_to_paint_order_and_hit() {
        // Pixels -> components: a region of a screenshot resolves to
        // every node painting there, the topmost one, and what a click
        // would actually reach.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("panel", Role::Panel, "panel")
                .with_prop("w", "200")
                .with_prop("h", "200")
                .with_prop("pad", "0"),
        );
        app.add(
            "panel",
            Component::new("btn", Role::Button, "Save")
                .with_prop("text", "Save")
                .with_prop("w", "80")
                .with_prop("h", "30")
                .with_prop("pad", "8"),
        );
        let layout = hamesh_layout::compute(&app, 400.0, 300.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let b = layout.get("btn").unwrap().rect.clone();
        let p = probe(&app, &layout, &scene, b.x + b.w / 2.0, b.y + b.h / 2.0);
        assert_eq!(p.topmost.as_deref(), Some("btn"));
        assert_eq!(p.hit.as_deref(), Some("btn"));
        assert!(
            p.paint_order.iter().any(|h| h.id == "panel"),
            "the panel behind the button must be in the paint order"
        );
        let last = p.paint_order.last().unwrap();
        assert_eq!(last.id, "btn", "paint order is back-to-front");
        // Away from the button the window root still paints (it fills
        // the viewport), and nothing interactive hits.
        let empty = probe(&app, &layout, &scene, 399.0, 299.0);
        assert_eq!(empty.topmost.as_deref(), Some("win"));
        assert_ne!(
            empty.hit.as_deref(),
            Some("btn"),
            "the button must not be hit outside its rect"
        );
    }

    #[test]
    fn visual_report_names_clipped_content_and_sibling_overlap() {
        // Two silent defect classes, named deterministically:
        // content cut by ancestor bounds, and a sibling painting over
        // another (which layout tests never notice).
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("panel", Role::Panel, "panel")
                .with_prop("w", "100")
                .with_prop("h", "24")
                .with_prop("pad", "0")
                .with_prop("gap", "0"),
        );
        for i in 0..3 {
            app.add(
                "panel",
                Component::new(&format!("row{i}"), Role::Text, "row")
                    .with_prop("text", "LABEL")
                    .with_prop("h", "20")
                    .with_prop("pad", "0")
                    .with_prop("flex_grow", "0"),
            );
        }
        // Two overlays sharing a rect: the later one hides the earlier.
        for name in ["under", "over"] {
            app.add(
                "win",
                Component::new(name, Role::Panel, name)
                    .with_prop("overlay", "true")
                    .with_prop("ox", "200")
                    .with_prop("oy", "200")
                    .with_prop("w", "40")
                    .with_prop("h", "40"),
            );
        }
        let layout = hamesh_layout::compute(&app, 400.0, 300.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let report = visual_report(&scene);
        assert!(
            report.clipped_content.iter().any(|id| id == "row2"),
            "cut content must be named: {:?}",
            report.clipped_content
        );
        assert!(
            report
                .overlaps
                .iter()
                .any(|o| o.under == "under" && o.over == "over"),
            "sibling overlap must be reported: {:?}",
            report.overlaps
        );
        // Ancestor/descendant containment is structure, not a defect.
        assert!(
            !report
                .overlaps
                .iter()
                .any(|o| o.under == "panel" || o.over == "panel"),
            "containment must not be reported as overlap"
        );
    }
}
