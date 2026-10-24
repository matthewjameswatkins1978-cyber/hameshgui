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
    /// Declared ramp line height (`lh` prop); None = font default.
    /// Renderers resolve identically to measure.
    pub line_height: Option<f32>,
    /// Measured line count at the final width. Render contract: nodes
    /// that measured single-line never re-wrap at paint time (paint
    /// wraps only what layout wrapped); overflow clips at the rect.
    pub lines: usize,
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
    /// Resolved glyph-run origin (node-local): the single placement
    /// both renderers paint at, verbatim. Includes halign, content
    /// inset and the optical vertical shift — FINAL, not an estimate.
    pub origin: [f32; 2],
    /// Content rectangle (node-local): padding-declared usable area.
    pub content: [f32; 4],
    /// Resolved horizontal alignment: start|center|end.
    pub halign: String,
    /// Shaped advance width (px) the origin was resolved against.
    pub advance: f32,
    /// Caret x (layout-space, relative to the text origin) for a focused
    /// text input. `None` = no caret. Part of scene truth so CPU and GPU
    /// renderers (and the Eye) agree on where editing happens.
    pub caret_x: Option<f32>,
    pub focused: bool,
    /// Font family from the `font` prop (`None` = default UI font).
    /// Part of scene truth so both renderers shape identically.
    pub font_family: Option<String>,
    /// OpenType variation axes from the `font_vars` prop
    /// (`"wdth 75, opsz 16"`). Empty = no variations.
    pub font_vars: Vec<([u8; 4], f32)>,
    /// False when the requested family is absent and shaping fell
    /// back: the interface stays correct, and the Eye reports the
    /// substitution here (family-level). True when no `font` prop.
    pub font_resolved: bool,
    /// Requested style (`font_style` prop): `Some("italic")` voices
    /// the italic face through font matching. `None` = upright.
    pub font_style: Option<String>,
    /// Extra inter-glyph px (`tracking` prop, may be negative).
    /// Part of scene truth: advances and origins include it.
    pub tracking: f32,
    /// Parent component id (`None` for the window root). The Eye uses
    /// it to walk from a pixel back up the tree.
    pub parent: Option<String>,
    /// Effective paint clip: the intersection of every ancestor rect
    /// and the viewport. Paint is bounded by this, so a child cannot
    /// draw outside its panel — the defect class that used to need a
    /// hand-written regression in every application.
    pub clip: Rect,
    /// True when this node's own rect escapes [`clip`] and paint is
    /// therefore cut. Prevention is silent by nature; this flag keeps
    /// it honest by naming exactly how much content was hidden.
    pub overflow: bool,
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
            // Final placement is resolved ONCE here and shared by CPU,
            // GPU and Eye (decision: Eye describes what appears, not an
            // intermediate estimate). Horizontal: `halign` positions the
            // shaped advance in the content box (single-line only;
            // multiline stays Start). Vertical: the font-relative trim
            // centers on the box; ink/trim/baseline below are FINAL
            // (optical shift already applied). Renderers place glyphs
            // at `origin` verbatim; the caret stays line-anchored.
            let font_size = num("font_size", 16.0).max(8.0);
            let line_height = c
                .props
                .get("lh")
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| *v > 0.0);
            // Same pad default as layout (text hugs, containers pad):
            // the content box must agree or placement diverges.
            let is_text_role = matches!(
                c.role,
                hamesh_core::Role::Text | hamesh_core::Role::Status | hamesh_core::Role::Button
            );
            let pad = c
                .props
                .get("pad")
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| *v >= 0.0)
                .unwrap_or(if is_text_role { 0.0 } else { 12.0 });
            // Optional typeface selection: `font` names the family,
            // `font_vars` lists axis settings. Parsed once per node;
            // every text consumer below shapes with the same spec.
            let font_spec = hamesh_text::FontSpec::parse(
                c.props.get("font").map(String::as_str),
                c.props.get("font_vars").map(String::as_str),
                c.props.get("font_style").map(String::as_str),
                c.props.get("tracking").map(String::as_str),
            );
            // Substitution signal: a named family the collection does
            // not know falls back. Layout and paint stay correct;
            // only this flag tells the Eye a substitution happened.
            let font_resolved = font_spec
                .family
                .as_deref()
                .map(hamesh_text::font_family_available)
                .unwrap_or(true);
            let avail_w = (l.rect.w - 2.0 * hamesh_text::TEXT_INSET_X).max(8.0);
            // Wrap rule shared with both renderers: wrap iff the final
            // width wraps. Single-line nodes lay out unwrapped so paint
            // can never re-wrap what layout kept whole.
            let avail_lines = hamesh_text::TextLayout::new_varied(
                &text,
                avail_w,
                font_size,
                line_height,
                &font_spec,
            )
            .line_count();
            let wrap_w = if text.is_empty() || avail_lines <= 1 {
                f32::INFINITY
            } else {
                avail_w
            };
            let lay = hamesh_text::TextLayout::new_varied(
                &text,
                wrap_w,
                font_size,
                line_height,
                &font_spec,
            );
            let oy = hamesh_text::run_oy(l.rect.h, text.is_empty(), lay.height());
            let advance = hamesh_text::trim_box(&text, wrap_w, font_size, line_height, &font_spec)
                .map(|t| t.advance)
                .unwrap_or(lay.width());
            let halign = if avail_lines <= 1 {
                hamesh_text::parse_halign(c.props.get("halign").map(|s| s.as_str()))
            } else {
                hamesh_text::HAlign::Start
            };
            // Logical direction: parley shapes BiDi itself; Start/End
            // follow the paragraph's dominant direction (RTL flips
            // them). Center is direction-independent. Reported halign
            // stays as declared (intent); the origin is the physical
            // result.
            let eff_align = match (halign, lay.is_rtl()) {
                (hamesh_text::HAlign::Start, true) => hamesh_text::HAlign::End,
                (hamesh_text::HAlign::End, true) => hamesh_text::HAlign::Start,
                (h, _) => h,
            };
            let tx = hamesh_text::align_origin_x(eff_align, pad, l.rect.w, advance);
            // Optical vertical shift from the final trim (None-safe).
            let ty = match hamesh_text::trim_box(&text, wrap_w, font_size, line_height, &font_spec)
            {
                Some(t) => {
                    let (y0, y1) = (t.top + oy, t.bottom + oy);
                    oy + (l.rect.h / 2.0 - (y0 + y1) / 2.0)
                }
                None => oy,
            };
            let shift = ty - oy;
            let ink = hamesh_text::ink_box(&text, wrap_w, font_size, line_height, &font_spec)
                .map(|b| [b.x0 + tx, b.y0 + oy + shift, b.x1 + tx, b.y1 + oy + shift]);
            let (trim, metrics, baseline) =
                match hamesh_text::trim_box(&text, wrap_w, font_size, line_height, &font_spec) {
                    Some(t) => (
                        Some([
                            tx,
                            t.top + oy + shift,
                            tx + t.advance,
                            t.bottom + oy + shift,
                        ]),
                        Some([t.ascent, t.descent, t.cap, t.line_height]),
                        Some(t.baseline + oy + shift),
                    ),
                    None => (None, None, None),
                };
            // Content box (node-local): what padding declared.
            let content = [
                pad,
                pad,
                (l.rect.w - pad).max(pad),
                (l.rect.h - pad).max(pad),
            ];
            let halign_name = match halign {
                hamesh_text::HAlign::Center => "center",
                hamesh_text::HAlign::End => "end",
                hamesh_text::HAlign::Start => "start",
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
                line_height,
                lines: lay.line_count(),
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
                origin: [tx, oy + shift],
                content,
                halign: halign_name.to_string(),
                advance,
                caret_x,
                focused,
                font_family: font_spec.family.clone(),
                font_vars: font_spec.vars.clone(),
                font_resolved,
                font_style: c.props.get("font_style").cloned(),
                tracking: font_spec.tracking,
                // Filled by the clip pass below (needs the whole
                // tree, not just this component).
                parent: None,
                clip: layout.viewport.clone(),
                overflow: false,
            })
        })
        .collect();
    // ---- Effective clip + overflow (paint-time truth) -------------
    // Walking parents here (not in layout) keeps layout's job intact:
    // layout decides where a box goes, the scene decides what paint is
    // allowed to touch. Nodes placed deliberately outside flow (the
    // Eye's own guides, modals) opt out with `overlay`/`clip=false`;
    // their children still clip to them.
    for n in &mut nodes {
        let me = app.components.get(&n.id);
        let opts_out = me
            .map(|c| {
                c.props.get("overlay").map(|v| v == "true").unwrap_or(false)
                    || c.props.get("clip").map(|v| v == "false").unwrap_or(false)
            })
            .unwrap_or(false);
        n.parent = me.and_then(|c| c.parent.as_ref().map(|p| p.0.clone()));
        let mut clip = layout.viewport.clone();
        if !opts_out {
            let mut cursor = n.parent.clone();
            while let Some(pid) = cursor {
                if let Some(pl) = layout.nodes.get(&pid) {
                    clip = match clip.intersect(&pl.rect) {
                        Some(r) => r,
                        None => {
                            // Entirely outside an ancestor: nothing may
                            // paint. A zero rect keeps the arithmetic
                            // honest instead of clamping to a sliver.
                            Rect {
                                x: 0.0,
                                y: 0.0,
                                w: 0.0,
                                h: 0.0,
                            }
                        }
                    };
                }
                let Some(pc) = app.components.get(&pid) else {
                    break;
                };
                let stop = pc
                    .props
                    .get("overlay")
                    .map(|v| v == "true")
                    .unwrap_or(false)
                    || pc.props.get("clip").map(|v| v == "false").unwrap_or(false);
                if stop {
                    break;
                }
                cursor = pc.parent.as_ref().map(|p| p.0.clone());
            }
        }
        n.overflow = !clip.contains_rect(&n.rect);
        n.clip = clip;
    }
    // Deterministic paint order. `z` alone was not enough: equal-z
    // siblings inherited HashMap iteration order, so the same app
    // could paint (and the Eye could report) two different topmost
    // nodes across runs. Tie-break on tree order — a pre-order walk
    // following each parent's declared child list — so paint order is
    // a property of the tree, not of hashing.
    let mut order = std::collections::HashMap::new();
    {
        let mut seq = 0usize;
        let mut stack = vec![app.root.0.clone()];
        while let Some(id) = stack.pop() {
            if order.insert(id.clone(), seq).is_none() {
                seq += 1;
            }
            if let Some(c) = app.components.get(&id) {
                // Reverse so the first declared child is visited first.
                for child in c.children.iter().rev() {
                    stack.push(child.0.clone());
                }
            }
        }
    }
    nodes.sort_by(|a, b| {
        let (za, zb) = (a.z, b.z);
        let (oa, ob) = (
            order.get(&a.id).copied().unwrap_or(usize::MAX),
            order.get(&b.id).copied().unwrap_or(usize::MAX),
        );
        za.cmp(&zb).then(oa.cmp(&ob))
    });
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
    fn equal_z_paint_order_is_deterministic_tree_order() {
        // Regression for the documented equal-z defect: two overlay
        // siblings in the same z layer used to paint in hash order, so
        // "what is on top" changed between runs. Tree order is the
        // contract: later-declared siblings paint on top.
        for attempt in 0..8 {
            let mut app = App::new("win");
            for name in ["lower", "upper"] {
                app.add(
                    "win",
                    Component::new(name, Role::Panel, name)
                        .with_prop("overlay", "true")
                        .with_prop("ox", "10")
                        .with_prop("oy", "10")
                        .with_prop("w", "40")
                        .with_prop("h", "40"),
                );
            }
            let layout = hamesh_layout::compute(&app, 200.0, 200.0);
            let scene = build(&app, &layout, attempt);
            let ids: Vec<&str> = scene
                .nodes
                .iter()
                .filter(|n| n.id == "lower" || n.id == "upper")
                .map(|n| n.id.as_str())
                .collect();
            assert_eq!(
                ids,
                vec!["lower", "upper"],
                "attempt {attempt}: equal-z order must follow declaration"
            );
        }
    }

    #[test]
    fn child_clip_is_the_ancestor_intersection_and_flags_overflow() {
        // Layout still places boxes where flex says; the scene now
        // states what paint may touch. Three stacked LABELS in a short
        // panel push the third past the panel edge. Text is
        // incompressible (panels shrink, labels cannot), which is why
        // the field defect was a label and never a panel.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("panel", Role::Panel, "panel")
                .with_prop("w", "100")
                .with_prop("h", "40")
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
        let layout = hamesh_layout::compute(&app, 400.0, 300.0);
        let scene = build(&app, &layout, 1);
        let panel = layout.get("panel").unwrap().rect.clone();
        let inside = scene.nodes.iter().find(|n| n.id == "row1").unwrap();
        assert!(!inside.overflow, "row1 sits inside the panel");
        assert!((inside.clip.h - panel.h).abs() < 0.5);
        let escaped = scene.nodes.iter().find(|n| n.id == "row2").unwrap();
        assert!(
            escaped.overflow,
            "row2 escapes the panel bottom and must report overflow"
        );
        assert!(
            (escaped.clip.y + escaped.clip.h - (panel.y + panel.h)).abs() < 0.5,
            "clip must end at the panel bottom, got {:?}",
            escaped.clip
        );
    }

    #[test]
    fn overlay_nodes_opt_out_of_ancestor_clipping() {
        // The Eye's own guides and modals live outside flow on purpose.
        // `overlay` keeps them paintable; everything else clips.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("panel", Role::Panel, "panel")
                .with_prop("w", "40")
                .with_prop("h", "40")
                .with_prop("pad", "0"),
        );
        app.add(
            "panel",
            Component::new("guide", Role::Panel, "guide")
                .with_prop("overlay", "true")
                .with_prop("ox", "200")
                .with_prop("oy", "200")
                .with_prop("w", "20")
                .with_prop("h", "20"),
        );
        let layout = hamesh_layout::compute(&app, 400.0, 300.0);
        let scene = build(&app, &layout, 1);
        let guide = scene.nodes.iter().find(|n| n.id == "guide").unwrap();
        assert!(!guide.overflow, "overlay must not be clipped by its parent");
        assert!(
            (guide.clip.w - 400.0).abs() < 0.5,
            "overlay clips to viewport"
        );
    }

    #[test]
    fn font_props_flow_into_scene_truth() {
        // `font` / `font_vars` resolve once per node and every text
        // consumer (advance, trim, ink, measure, render) shapes with
        // the same spec — boxes and paint cannot disagree. The
        // vendored Instrument Sans resolves (no substitution).
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("b", Role::Status, "badge")
                .with_prop("text", "Connected")
                .with_prop("font_size", "13")
                .with_prop("pad", "8")
                .with_prop("w", "200")
                .with_prop("halign", "center")
                .with_prop("font", "Instrument Sans")
                .with_prop("font_vars", "wdth 75"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let n = scene.nodes.iter().find(|n| n.id == "b").unwrap();
        assert_eq!(n.font_family.as_deref(), Some("Instrument Sans"));
        assert_eq!(n.font_vars, vec![(*b"wdth", 75.0)]);
        assert!(n.font_resolved, "vendored face must not substitute");
        // Scene truth matches the public API on identical inputs.
        let spec = hamesh_text::FontSpec {
            family: Some("Instrument Sans".to_string()),
            vars: vec![(*b"wdth", 75.0)],
            style: None,
            tracking: 0.0,
        };
        let direct =
            hamesh_text::trim_box("Connected", f32::INFINITY, 13.0, None, &spec).expect("trim");
        assert!(
            (n.advance - direct.advance).abs() < 0.01,
            "scene advance {} vs API {}",
            n.advance,
            direct.advance
        );
    }

    #[test]
    fn malformed_font_vars_are_skipped_not_fatal() {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("b", Role::Status, "badge")
                .with_prop("text", "Hi")
                .with_prop("font", "Instrument Sans")
                .with_prop("font_vars", "bogus, wdth nope, toolongtag 1, opsz 16"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let n = scene.nodes.iter().find(|n| n.id == "b").unwrap();
        assert_eq!(n.font_vars, vec![(*b"opsz", 16.0)]);
    }

    #[test]
    fn missing_family_falls_back_but_reports_substitution() {
        // Fallback contract: an unknown family must never break the
        // interface (layout valid, advance positive, ink present),
        // and the Eye must expose the substitution via font_resolved.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("b", Role::Status, "badge")
                .with_prop("text", "Connected")
                .with_prop("font", "NoSuchFaceXYZ")
                .with_prop("font_size", "13")
                .with_prop("lh", "18"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let n = scene.nodes.iter().find(|n| n.id == "b").unwrap();
        assert_eq!(n.font_family.as_deref(), Some("NoSuchFaceXYZ"));
        assert!(!n.font_resolved, "unknown family must report substitution");
        assert!(n.advance > 1.0, "fallback must still shape");
        assert!(n.ink.is_some(), "fallback must still paint");
        assert_eq!(n.lines, 1);
    }

    #[test]
    fn fresh_threads_shape_with_vendored_faces() {
        // Cache-poisoning regression: a thread that never explicitly
        // registered must still resolve bundled faces (auto-register
        // on first shape), so parallel tests can never cache fallback
        // geometry under a key a voiced thread later reuses.
        let shape_plex = || {
            // Pristine collection: Plex is vendored, never systemic.
            assert!(
                !hamesh_text::font_family_available("IBM Plex Mono"),
                "fresh thread must start unregistered"
            );
            let mut app = App::new("win");
            app.add(
                "win",
                Component::new("b", Role::Status, "badge")
                    .with_prop("text", "Connected")
                    .with_prop("font", "IBM Plex Mono")
                    .with_prop("font_size", "13")
                    .with_prop("lh", "18"),
            );
            let layout = hamesh_layout::compute(&app, 800.0, 600.0);
            let scene = build(&app, &layout, 1);
            let n = scene.nodes.iter().find(|n| n.id == "b").unwrap();
            assert!(n.font_resolved, "fresh thread must auto-register");
            n.advance
        };
        let other = std::thread::scope(|s| s.spawn(shape_plex).join().unwrap());
        let here = shape_plex();
        assert_eq!(here, other, "one shaper, one answer, any thread");
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

    fn centered_badge(text: &str, w: f32) -> (App, SceneNode) {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("b", Role::Status, "badge")
                .with_prop("text", text)
                .with_prop("font_size", "13")
                .with_prop("lh", "18")
                .with_prop("pad", "8")
                .with_prop("w", &w.to_string())
                .with_prop("halign", "center"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let node = scene.nodes.iter().find(|n| n.id == "b").unwrap().clone();
        (app, node)
    }

    #[test]
    fn halign_center_resolves_to_content_center() {
        // Live badge geometry: 95px box must center the advance.
        for (text, w) in [
            ("Connected", 95.0),
            ("Disconnected", 200.0),
            ("Caf\u{e9}", 140.0),
            ("Inspect design", 260.0),
        ] {
            let (_, n) = centered_badge(text, w);
            assert_eq!(n.halign, "center");
            let cx = n.origin[0] + n.advance / 2.0;
            assert!((cx - w / 2.0).abs() < 0.5, "{text}: center {cx} vs {w}");
        }
    }

    #[test]
    fn relabel_keeps_center_without_content_offsets() {
        // Dynamic labels: changing text re-resolves; no per-word tweak.
        let (mut app, _) = centered_badge("Connected", 200.0);
        let w = 200.0;
        for text in [
            "Connected",
            "Disconnected",
            "Inspected \u{2713}",
            "Caf\u{e9}",
            "Errors",
        ] {
            app.get_mut("b")
                .unwrap()
                .props
                .insert("text".to_string(), text.to_string());
            let layout = hamesh_layout::compute(&app, 800.0, 600.0);
            let scene = build(&app, &layout, 1);
            let n = scene.nodes.iter().find(|n| n.id == "b").unwrap();
            let cx = n.origin[0] + n.advance / 2.0;
            assert!((cx - w / 2.0).abs() < 0.5, "{text}: center {cx}");
        }
    }

    #[test]
    fn left_biased_placement_trips_the_centering_check() {
        // The drill's machine check must FAIL on the old behaviour:
        // resolve the same badge with Start and prove the offset the
        // drill asserts against exceeds its 1px tolerance.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("b", Role::Status, "badge")
                .with_prop("text", "Connected")
                .with_prop("font_size", "13")
                .with_prop("lh", "18")
                .with_prop("pad", "8")
                .with_prop("w", "95")
                .with_prop("halign", "start"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let n = scene.nodes.iter().find(|n| n.id == "b").unwrap();
        let off = (n.origin[0] + n.advance / 2.0 - 95.0 / 2.0).abs();
        assert!(
            off > 1.0,
            "start-aligned badge must read off-centre, got {off}"
        );
    }

    #[test]
    fn rtl_start_means_right() {
        // Parley shapes the BiDi run; logical Start resolves to the
        // physical right for RTL paragraphs (Hebrew for "connected").
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("h", Role::Status, "he")
                .with_prop("text", "מחובר")
                .with_prop("font_size", "13")
                .with_prop("pad", "8")
                .with_prop("w", "200")
                .with_prop("halign", "start"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let n = scene.nodes.iter().find(|n| n.id == "h").unwrap();
        assert!(n.advance > 10.0, "Hebrew must shape, got {:?}", n.advance);
        assert!(
            (n.origin[0] + n.advance - (200.0 - 8.0)).abs() < 1.0,
            "RTL start at right edge: {:?}",
            n.origin
        );
    }

    #[test]
    fn multiline_falls_back_to_start() {
        // Wrapping text never gets a single-line shortcut: the run
        // stays left-anchored and paint wraps identically.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("m", Role::Text, "multi")
                .with_prop("text", "aaa bbb ccc ddd eee fff ggg hhh")
                .with_prop("font_size", "16")
                .with_prop("w", "120")
                .with_prop("halign", "center"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let n = scene.nodes.iter().find(|n| n.id == "m").unwrap();
        assert!(n.lines > 1, "must wrap, got {}", n.lines);
        assert_eq!(n.origin[0], 8.0, "wrapped text stays left-anchored");
    }

    #[test]
    fn start_and_end_have_defined_semantics() {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("s", Role::Status, "s")
                .with_prop("text", "Hi")
                .with_prop("font_size", "13")
                .with_prop("pad", "8")
                .with_prop("w", "200")
                .with_prop("halign", "start"),
        );
        app.add(
            "win",
            Component::new("e", Role::Status, "e")
                .with_prop("text", "Hi")
                .with_prop("font_size", "13")
                .with_prop("pad", "8")
                .with_prop("w", "200")
                .with_prop("halign", "end"),
        );
        app.add(
            "win",
            Component::new("u", Role::Status, "u")
                .with_prop("text", "Hi")
                .with_prop("font_size", "13")
                .with_prop("pad", "8")
                .with_prop("w", "200")
                .with_prop("halign", "sideways"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = build(&app, &layout, 1);
        let g = |id: &str| scene.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(g("s").origin[0], 8.0, "start = content left");
        assert_eq!(g("u").halign, "start", "unknown falls back to start");
        let adv = g("e").advance;
        assert!(
            (g("e").origin[0] - (200.0 - 8.0 - adv)).abs() < 0.01,
            "end mirrors start"
        );
    }
}
