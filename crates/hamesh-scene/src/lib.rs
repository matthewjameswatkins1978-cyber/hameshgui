//! hamesh-scene: resolved visual representation.
//!
//! The scene is derived from App truth + Layout truth. It carries paint order,
//! resolved colors/text and per-node visibility facts. Revisions tie frames
//! to the exact app/layout state they were built from.

use hamesh_core::App;
use hamesh_layout::{LayoutOutput, Rect};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneNode {
    pub id: String,
    pub role: String,
    pub name: String,
    pub rect: Rect,
    pub z: i32,
    pub visible: bool,
    pub clipped: bool,
    pub enabled: bool,
    pub color: [u8; 4],
    pub text: String,
    /// Corner radius (px) from the `radius` prop. Both renderers honor it.
    pub radius: f32,
    /// Border width (px) from `border`, color from the `border_token`
    /// prop (default `border`). 0 = no border.
    pub border_w: f32,
    pub border_rgba: [u8; 4],
    /// Progress fill 0..1 from `value`/`min`/`max` props. `None` = not
    /// a progress node; the renderers paint track + fill token.
    pub fill_frac: Option<f32>,
    /// Bar color for progress nodes, from the `fill` token prop
    /// (default `accent`). Ignored unless `fill_frac` is set.
    pub fill_rgba: [u8; 4],
    /// Icon glyph name from the `icon` prop. `None` = no icon.
    pub icon: Option<String>,
    /// Text size (px) from `font_size` (default 16) and color from
    /// the `text_token` prop (default near-white `text`).
    pub font_size: f32,
    pub text_rgba: [u8; 4],
    /// Ghost nodes (spacers) participate in layout and hit-testing
    /// policy but paint nothing.
    pub ghost: bool,
    /// Shaped ink bounds (node-local x0,y0,x1,y1): what the glyphs
    /// actually paint, bearings included. The Eye reads object bounds
    /// CorelDRAW-style; alignment never uses this (it would jump with
    /// content). `None` = no ink (empty text, icon-only nodes).
    pub ink: Option<[f32; 4]>,
    /// Font-relative trim (node-local x0,y0,x1,y1): cap top to
    /// alphabetic baseline, same for any string in this font/size.
    /// Renderers center THIS, never the ink silhouette.
    pub trim: Option<[f32; 4]>,
    /// Font metrics for this node's text: [ascent, descent, cap,
    /// line_height] in px. Distinguished from ink on purpose: the Eye
    /// reports both so agents stop guessing which box they see.
    pub metrics: Option<[f32; 4]>,
    /// First baseline (node-local y), for cross-node baseline checks.
    pub baseline: Option<f32>,
    /// Caret x (layout-space, relative to the text origin) for a focused
    /// text input. `None` = no caret. Part of scene truth so CPU and GPU
    /// renderers (and the Eye) agree on where editing happens.
    pub caret_x: Option<f32>,
    pub focused: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub scene_revision: u64,
    pub app_revision: u64,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub nodes: Vec<SceneNode>,
}

fn role_color(role: &str, name: &str) -> [u8; 4] {
    // One polished dark theme: near-black surfaces, restrained teal/green.
    match role {
        s if s.contains("Window") => [14, 20, 22, 255],
        s if s.contains("Header") => [20, 30, 33, 255],
        s if s.contains("Button") => {
            if name.to_lowercase().contains("primary") || name.to_lowercase().contains("increment")
            {
                [45, 151, 134, 255] // teal accent
            } else {
                [38, 52, 56, 255]
            }
        }
        s if s.contains("Overlay") => [120, 40, 40, 180], // translucent red for fault drills
        // Text/Status nodes are labels on their parent surface: no fill of
        // their own (transparent), so the parent shows through. Their
        // content is the rasterized text, not the rect.
        s if s.contains("Text") | s.contains("Status") => [0, 0, 0, 0],
        _ => [30, 42, 46, 255],
    }
}

pub fn build(app: &App, layout: &LayoutOutput, scene_revision: u64) -> Scene {
    let mut nodes: Vec<SceneNode> = app
        .components
        .values()
        .filter_map(|c| {
            let l = layout.nodes.get(&c.id.0)?;
            let role = format!("{:?}", c.role);
            // Only explicit `text` props become rendered text. The
            // component name is identity (for the Eye), never content:
            // painting names of containers ("root", "main") as text
            // was a real bug caught by the frame PNG.
            let text = c.props.get("text").cloned().unwrap_or_default();
            let caret_x = c.props.get("caret_x").and_then(|v| v.parse::<f32>().ok());
            let focused = c.props.get("focused").map(|s| s == "true").unwrap_or(false);
            // Style resolution (decision 020): explicit token props win;
            // otherwise the legacy role palette keeps old screens stable.
            let num = |k: &str, d: f32| {
                c.props
                    .get(k)
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(d)
            };
            let color = c
                .props
                .get("surface")
                .map(|t| hamesh_theme::resolve(t))
                .unwrap_or_else(|| role_color(&role, &c.name));
            // Progress fill is opt-in: only `block == "Progress"` nodes
            // interpret value/min/max as a fill fraction. Other blocks
            // (e.g. Metric tiles) reuse `value` as Eye metadata.
            let is_progress = c.props.get("block").map(|s| s.as_str()) == Some("Progress");
            let fill_frac = if is_progress {
                match (c.props.get("value"), c.props.get("min"), c.props.get("max")) {
                    (Some(v), min, max) => {
                        let v: f32 = v.parse().unwrap_or(0.0);
                        let lo: f32 = min.and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        let hi: f32 = max.and_then(|s| s.parse().ok()).unwrap_or(100.0);
                        Some(((v - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0))
                    }
                    _ => None,
                }
            } else {
                None
            };
            // Ink + trim in node-local space. Ink is what the glyphs
            // paint (Eye object bounds); trim is the font-relative
            // cap-to-baseline box (renderer alignment target). Same
            // line origin both sides: +inset horizontally, +run origin
            // vertically. Trim x spans the advance width (stable).
            let font_size = num("font_size", 16.0).max(8.0);
            let avail_w = (l.rect.w - 2.0 * hamesh_text::TEXT_INSET_X).max(8.0);
            let lay = hamesh_text::TextLayout::new(&text, avail_w, font_size);
            let oy = hamesh_text::run_oy(l.rect.h, text.is_empty(), lay.height());
            let ink = hamesh_text::ink_box(&text, avail_w, font_size).map(|b| {
                [
                    b.x0 + hamesh_text::TEXT_INSET_X,
                    b.y0 + oy,
                    b.x1 + hamesh_text::TEXT_INSET_X,
                    b.y1 + oy,
                ]
            });
            let (trim, metrics, baseline) = match hamesh_text::trim_box(&text, avail_w, font_size) {
                Some(t) => (
                    Some([
                        hamesh_text::TEXT_INSET_X,
                        t.top + oy,
                        hamesh_text::TEXT_INSET_X + t.advance,
                        t.bottom + oy,
                    ]),
                    Some([t.ascent, t.descent, t.cap, t.line_height]),
                    Some(t.baseline + oy),
                ),
                None => (None, None, None),
            };
            Some(SceneNode {
                id: c.id.0.clone(),
                role: role.clone(),
                name: c.name.clone(),
                rect: l.rect.clone(),
                z: l.z,
                visible: l.effectively_visible,
                clipped: l.clipped_by_parent,
                enabled: c.enabled,
                color,
                text,
                radius: num("radius", 0.0).max(0.0),
                border_w: num("border", 0.0).max(0.0),
                border_rgba: c
                    .props
                    .get("border_token")
                    .map(|t| hamesh_theme::resolve(t))
                    .unwrap_or_else(|| hamesh_theme::resolve("border")),
                fill_frac,
                fill_rgba: c
                    .props
                    .get("fill")
                    .map(|t| hamesh_theme::resolve(t))
                    .unwrap_or_else(|| hamesh_theme::resolve("accent")),
                icon: c.props.get("icon").cloned(),
                font_size,
                text_rgba: c
                    .props
                    .get("text_token")
                    .map(|t| hamesh_theme::resolve(t))
                    .unwrap_or_else(|| hamesh_theme::resolve("text")),
                ghost: c.props.get("ghost").map(|s| s == "true").unwrap_or(false),
                ink,
                trim,
                metrics,
                baseline,
                caret_x,
                focused,
            })
        })
        .collect();
    nodes.sort_by_key(|n| n.z);
    Scene {
        scene_revision,
        app_revision: app.revision,
        viewport_w: layout.viewport.w,
        viewport_h: layout.viewport.h,
        nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hamesh_core::{Component, Role};

    #[test]
    fn scene_sorted_and_referenced() {
        let mut app = App::new("win");
        app.add("win", Component::new("b1", Role::Button, "Increment"));
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 7);
        assert_eq!(scene.app_revision, app.revision);
        assert_eq!(scene.scene_revision, 7);
        assert!(scene.nodes.iter().any(|n| n.id == "b1"));
    }

    #[test]
    fn style_props_resolve_to_scene_truth() {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("card", Role::Panel, "card")
                .with_prop("surface", "raised")
                .with_prop("radius", "10")
                .with_prop("border", "2")
                .with_prop("border_token", "accent")
                .with_prop("font_size", "20")
                .with_prop("text_token", "muted"),
        );
        app.add(
            "win",
            Component::new("bar", Role::Panel, "progress")
                .with_prop("block", "Progress")
                .with_prop("value", "75")
                .with_prop("fill", "success"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let card = scene.nodes.iter().find(|n| n.id == "card").unwrap();
        assert_eq!(card.color, hamesh_theme::resolve("raised"));
        assert_eq!(card.radius, 10.0);
        assert_eq!(card.border_w, 2.0);
        assert_eq!(card.border_rgba, hamesh_theme::resolve("accent"));
        assert_eq!(card.font_size, 20.0);
        assert_eq!(card.text_rgba, hamesh_theme::resolve("muted"));
        assert!(card.icon.is_none() && card.fill_frac.is_none() && !card.ghost);
        let bar = scene.nodes.iter().find(|n| n.id == "bar").unwrap();
        assert_eq!(bar.fill_frac, Some(0.75));
        assert_eq!(bar.fill_rgba, hamesh_theme::resolve("success"));
    }
}
