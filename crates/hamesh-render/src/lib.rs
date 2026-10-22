//! hamesh-render: CPU renderer with compatible scene semantics.
//!
//! GPU (wgpu/Vello) is the architectural default; this CPU backend proves
//! scene semantics early, works headless/CI, and defines the tolerance model
//! for later GPU-vs-CPU comparison.

use hamesh_scene::Scene;
use serde::{Deserialize, Serialize};
use tiny_skia::{
    Color, FillRule, Paint, Path, PathBuilder, Pixmap, Rect as SkRect, Stroke, Transform,
};

pub mod icons;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrameRef {
    pub scene_revision: u64,
    pub app_revision: u64,
    pub width: u32,
    pub height: u32,
}

pub struct Frame {
    pub pixmap: Pixmap,
    pub frame_ref: FrameRef,
}

/// Blit parley/swash glyph coverage for a node's text, clipped to the
/// node's own rect so text never bleeds into siblings.
fn blit_text(pix: &mut Pixmap, node: &hamesh_scene::SceneNode) {
    let avail_w = (node.rect.w - 16.0).max(8.0);
    let layout = hamesh_text::TextLayout::new(&node.text, avail_w, node.font_size);
    // Empty text still gets a line box: the caret must render in an
    // empty focused composer (this early-return once ate the caret).
    let lh = layout.height().max(18.0);
    let ox = node.rect.x + 8.0;
    let oy = node.rect.y + ((node.rect.h - lh) / 2.0).max(0.0);
    let glyphs = if layout.height() > 0.0 {
        hamesh_text::rasterize_layout(&layout)
    } else {
        Vec::new()
    };
    // Caret bar for the focused composer (scene truth, read by the Eye too).
    if node.focused {
        if let Some(cx) = node.caret_x {
            let bx = ox + cx;
            let bh = lh.max(8.0);
            if bx >= node.rect.x && bx + 2.0 <= node.rect.x + node.rect.w {
                let mut cp = Paint::default();
                cp.set_color(Color::from_rgba8(120, 220, 200, 255));
                if let Some(bar) = SkRect::from_xywh(bx, oy, 2.0, bh) {
                    pix.fill_rect(bar, &cp, Transform::identity(), None);
                }
            }
        }
    }
    let (fw, fh) = (pix.width() as i32, pix.height() as i32);
    for g in &glyphs {
        let (x0, y0) = hamesh_text::glyph_origin(g, ox, oy);
        for (i, a) in g.alpha.iter().enumerate() {
            if *a < 8 {
                continue;
            }
            let px = x0 + (i as u32 % g.w) as i32;
            let py = y0 + (i as u32 / g.w) as i32;
            // Clip to the node rect AND the frame.
            if px < node.rect.x as i32
                || py < node.rect.y as i32
                || px >= (node.rect.x + node.rect.w) as i32
                || py >= (node.rect.y + node.rect.h) as i32
                || px < 0
                || py < 0
                || px >= fw
                || py >= fh
            {
                continue;
            }
            let mut p = Paint::default();
            let [tr, tg, tb, _] = node.text_rgba;
            p.set_color(Color::from_rgba8(tr, tg, tb, *a));
            if let Some(dot) = SkRect::from_xywh(px as f32, py as f32, 1.0, 1.0) {
                pix.fill_rect(dot, &p, Transform::identity(), None);
            }
        }
    }
}

fn rounded_clip_rect(rect: &hamesh_layout::Rect) -> Option<SkRect> {
    SkRect::from_xywh(rect.x, rect.y, rect.w.max(1.0), rect.h.max(1.0))
}

/// Rounded-rectangle path (cubic quarter-circle approximation).
/// Radius clamps to half the smaller side so thin bars stay sane.
fn rounded_path(rect: &hamesh_layout::Rect, radius: f32) -> Option<Path> {
    let r = radius.min(rect.w / 2.0).min(rect.h / 2.0);
    if r <= 0.0 {
        return None;
    }
    let (x, y, w, h) = (rect.x, rect.y, rect.w, rect.h);
    let k = r * 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

pub fn render(scene: &Scene) -> Frame {
    let w = scene.viewport_w.max(1.0) as u32;
    let h = scene.viewport_h.max(1.0) as u32;
    let mut pix = Pixmap::new(w, h).expect("pixmap");
    // Background.
    pix.fill(Color::from_rgba8(10, 15, 17, 255));

    for node in &scene.nodes {
        if !node.visible || node.ghost {
            continue;
        }
        let [rr, gg, bb, aa] = node.color;
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(rr, gg, bb, aa));
        if node.radius > 0.0 {
            if let Some(path) = rounded_path(&node.rect, node.radius) {
                pix.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            if node.border_w > 0.0 {
                // Inset half the stroke so the border stays inside the rect.
                let half = node.border_w / 2.0;
                let inset = hamesh_layout::Rect {
                    x: node.rect.x + half,
                    y: node.rect.y + half,
                    w: (node.rect.w - node.border_w).max(1.0),
                    h: (node.rect.h - node.border_w).max(1.0),
                };
                if let Some(bp) = rounded_path(&inset, (node.radius - half).max(0.0)) {
                    let mut paint_b = Paint::default();
                    let [br, bg, bb2, ba] = node.border_rgba;
                    paint_b.set_color(Color::from_rgba8(br, bg, bb2, ba));
                    let stroke = Stroke {
                        width: node.border_w,
                        ..Default::default()
                    };
                    pix.stroke_path(&bp, &paint_b, &stroke, Transform::identity(), None);
                }
            }
        } else {
            let Some(r) = rounded_clip_rect(&node.rect) else {
                continue;
            };
            // Simple rounded look: draw rect; overlay translucency shows through.
            pix.fill_rect(r, &paint, Transform::identity(), None);
            if node.border_w > 0.0 {
                let mut paint_b = Paint::default();
                let [br, bg, bb2, ba] = node.border_rgba;
                paint_b.set_color(Color::from_rgba8(br, bg, bb2, ba));
                let stroke = Stroke {
                    width: node.border_w,
                    ..Default::default()
                };
                let mut pb = PathBuilder::new();
                pb.move_to(r.x(), r.y());
                pb.line_to(r.x() + r.width(), r.y());
                pb.line_to(r.x() + r.width(), r.y() + r.height());
                pb.line_to(r.x(), r.y() + r.height());
                pb.close();
                if let Some(bp) = pb.finish() {
                    pix.stroke_path(&bp, &paint_b, &stroke, Transform::identity(), None);
                }
            }
        }
        // Progress fill: bar over the track, same height, fraction width.
        if let Some(frac) = node.fill_frac {
            if frac > 0.0 {
                let bw = (node.rect.w * frac).max(1.0);
                let bar = hamesh_layout::Rect {
                    x: node.rect.x,
                    y: node.rect.y,
                    w: bw,
                    h: node.rect.h,
                };
                let mut paint_f = Paint::default();
                let [fr, fg, fb, fa] = node.fill_rgba;
                paint_f.set_color(Color::from_rgba8(fr, fg, fb, fa));
                if node.radius > 0.0 {
                    if let Some(bp) = rounded_path(&bar, node.radius.min(bar.h / 2.0)) {
                        pix.fill_path(
                            &bp,
                            &paint_f,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                } else if let Some(r) = SkRect::from_xywh(bar.x, bar.y, bar.w, bar.h) {
                    pix.fill_rect(r, &paint_f, Transform::identity(), None);
                }
            }
        }
        // Icon glyph: stroked Lucide path in the text color, unknown
        // names silently draw nothing (the Eye still shows the prop).
        if let Some(ref iname) = node.icon {
            if let Some(bez) = icons::icon_bez(iname) {
                let (scale, dx, dy) = icons::icon_placement(node.rect.w, node.rect.h);
                let ox = node.rect.x as f64 + dx;
                let oy = node.rect.y as f64 + dy;
                if let Some(ipath) = icons::bez_to_tinyskia(&bez, ox, oy, scale) {
                    let mut paint_i = Paint::default();
                    let [ir, ig, ib, _] = node.text_rgba;
                    paint_i.set_color(Color::from_rgba8(ir, ig, ib, 255));
                    let stroke = Stroke {
                        width: (2.0 * scale) as f32,
                        line_cap: tiny_skia::LineCap::Round,
                        line_join: tiny_skia::LineJoin::Round,
                        ..Default::default()
                    };
                    pix.stroke_path(&ipath, &paint_i, &stroke, Transform::identity(), None);
                }
            }
        }
        // Real text: parley layout + swash raster, blitted per covered
        // pixel and clipped to the node's rect. The caret draws even for
        // empty text, so blit_text itself decides what to skip — never
        // gate the whole call on text presence (that ate the caret twice).
        blit_text(&mut pix, node);
    }

    Frame {
        frame_ref: FrameRef {
            scene_revision: scene.scene_revision,
            app_revision: scene.app_revision,
            width: w,
            height: h,
        },
        pixmap: pix,
    }
}

pub fn save_png(frame: &Frame, path: &str) -> Result<(), String> {
    frame
        .pixmap
        .save_png(path)
        .map_err(|e| format!("save_png failed: {e:?}"))
}

/// Tolerance model for CPU-vs-GPU comparison.
/// Exact pixel equality is NOT required across renderers.
#[derive(Clone, Debug)]
pub struct Tolerance {
    /// Max per-channel absolute difference.
    pub per_channel: u8,
    /// Fraction of pixels allowed to exceed per_channel (0.0..1.0).
    pub outlier_fraction: f32,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            per_channel: 8,
            outlier_fraction: 0.01,
        }
    }
}

pub fn compare(a: &Pixmap, b: &Pixmap, tol: &Tolerance) -> Result<(), String> {
    if a.width() != b.width() || a.height() != b.height() {
        return Err(format!(
            "size mismatch {}x{} vs {}x{}",
            a.width(),
            a.height(),
            b.width(),
            b.height()
        ));
    }
    let da = a.data();
    let db = b.data();
    let mut bad = 0usize;
    let total = (a.width() as usize) * (a.height() as usize);
    for px in 0..total {
        let o = px * 4;
        let mut px_bad = false;
        for c in 0..4 {
            let d = da[o + c].abs_diff(db[o + c]);
            if d > tol.per_channel {
                px_bad = true;
                break;
            }
        }
        if px_bad {
            bad += 1;
        }
    }
    let frac = bad as f32 / total.max(1) as f32;
    if frac > tol.outlier_fraction {
        return Err(format!(
            "pixels out of tolerance: {bad}/{total} ({frac:.3} > {:.3})",
            tol.outlier_fraction
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hamesh_core::{App, Component, Role};

    #[test]
    fn renders_and_compares_self() {
        let mut app = App::new("win");
        app.add("win", Component::new("b1", Role::Button, "Increment"));
        let layout = hamesh_layout::compute(&app, 320.0, 200.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let f = render(&scene);
        assert_eq!((f.frame_ref.width, f.frame_ref.height), (320, 200));
        compare(&f.pixmap, &f.pixmap.clone(), &Tolerance::default()).expect("self equal");
    }

    /// A focused composer with text renders both glyph pixels AND a caret
    /// bar at the measured caret x. Deterministic pixel evidence for the
    /// caret path (no screenshot eyeballing).
    #[test]
    fn caret_bar_is_drawn() {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("c", Role::TextInput, "composer")
                .with_prop("h", "40")
                .with_prop("text", "hi")
                .with_prop("caret_x", "10.0")
                .with_prop("focused", "true"),
        );
        let layout = hamesh_layout::compute(&app, 320.0, 200.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let node = scene.nodes.iter().find(|n| n.id == "c").unwrap();
        assert!(node.focused);
        assert_eq!(node.caret_x, Some(10.0));
        let f = render(&scene);
        // Caret bar: x in [ox+10, ox+12), spanning the node height.
        let ox = node.rect.x + 8.0;
        let mut caret_pixels = 0;
        let data = f.pixmap.data();
        let w = f.pixmap.width() as usize;
        for y in (node.rect.y as usize)..((node.rect.y + node.rect.h) as usize) {
            for x in ((ox + 10.0) as usize)..((ox + 12.0) as usize + 1) {
                let o = (y * w + x) * 4;
                // Caret color is teal-green (120, 220, 200): green dominant.
                if data[o + 1] > 150 && data[o] < 200 {
                    caret_pixels += 1;
                }
            }
        }
        assert!(
            caret_pixels >= 10,
            "caret bar missing: {caret_pixels} teal pixels near x={ox}"
        );
    }

    fn px(pix: &tiny_skia::Pixmap, x: usize, y: usize) -> [u8; 4] {
        let o = (y * pix.width() as usize + x) * 4;
        let d = pix.data();
        [d[o], d[o + 1], d[o + 2], d[o + 3]]
    }

    #[test]
    fn rounded_corners_leave_background() {
        // radius=20 on a 100px card: the corner pixel shows the parent
        // surface through the rounded cutout; the center takes the fill.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("card", Role::Panel, "card")
                .with_prop("w", "100")
                .with_prop("h", "100")
                .with_prop("surface", "raised")
                .with_prop("radius", "20"),
        );
        let layout = hamesh_layout::compute(&app, 200.0, 200.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let f = render(&scene);
        let win_bg = scene.nodes.iter().find(|n| n.id == "win").unwrap().color;
        let node = scene.nodes.iter().find(|n| n.id == "card").unwrap();
        let cx = (node.rect.x + 2.0) as usize;
        let cy = (node.rect.y + 2.0) as usize;
        assert_eq!(px(&f.pixmap, cx, cy), win_bg, "corner cutout shows parent");
        let mx = (node.rect.x + node.rect.w / 2.0) as usize;
        let my = (node.rect.y + node.rect.h / 2.0) as usize;
        assert_eq!(
            px(&f.pixmap, mx, my),
            hamesh_theme::resolve("raised"),
            "center must be surface"
        );
    }

    #[test]
    fn ghost_spacer_paints_nothing() {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("sp", Role::Panel, "spacer").with_prop("ghost", "true"),
        );
        let layout = hamesh_layout::compute(&app, 100.0, 100.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        assert!(scene.nodes.iter().find(|n| n.id == "sp").unwrap().ghost);
        let f = render(&scene);
        // Whole frame is the root fill: the ghost left no rect behind.
        let win_bg = scene.nodes.iter().find(|n| n.id == "win").unwrap().color;
        let d = f.pixmap.data();
        let (chunks, _) = d.as_chunks::<4>();
        assert!(chunks.iter().all(|c| *c == win_bg));
    }

    #[test]
    fn progress_bar_draws_track_and_fill() {
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("bar", Role::Panel, "progress")
                .with_prop("w", "100")
                .with_prop("h", "10")
                .with_prop("surface", "track")
                .with_prop("value", "50")
                .with_prop("fill", "accent"),
        );
        let layout = hamesh_layout::compute(&app, 200.0, 200.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let node = scene.nodes.iter().find(|n| n.id == "bar").unwrap();
        assert_eq!(node.fill_frac, Some(0.5));
        let f = render(&scene);
        let y = (node.rect.y + node.rect.h / 2.0) as usize;
        let left = px(&f.pixmap, (node.rect.x + 5.0) as usize, y);
        let right = px(&f.pixmap, (node.rect.x + node.rect.w - 5.0) as usize, y);
        assert_eq!(
            left,
            hamesh_theme::resolve("accent"),
            "filled half is accent"
        );
        assert_eq!(right, hamesh_theme::resolve("track"), "empty half is track");
    }

    #[test]
    fn icon_node_strokes_in_text_color() {
        // The users glyph must leave text-colored stroke pixels; an
        // unknown icon name must draw nothing and never crash.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("ic", Role::Panel, "agents-icon")
                .with_prop("w", "48")
                .with_prop("h", "48")
                .with_prop("icon", "users")
                .with_prop("text_token", "accent_bright"),
        );
        app.add(
            "win",
            Component::new("bad", Role::Panel, "nope")
                .with_prop("w", "48")
                .with_prop("h", "48")
                .with_prop("icon", "nope"),
        );
        let layout = hamesh_layout::compute(&app, 200.0, 200.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        assert_eq!(
            scene.nodes.iter().find(|n| n.id == "ic").unwrap().icon,
            Some("users".to_string())
        );
        let f = render(&scene);
        let node = scene.nodes.iter().find(|n| n.id == "ic").unwrap();
        let ink = hamesh_theme::resolve("accent_bright");
        let mut stroke_pixels = 0;
        let d = f.pixmap.data();
        let w = f.pixmap.width() as usize;
        let x0 = node.rect.x as usize;
        let y0 = node.rect.y as usize;
        for y in y0..(y0 + node.rect.h as usize) {
            for x in x0..(x0 + node.rect.w as usize) {
                let o = (y * w + x) * 4;
                if d[o] == ink[0] && d[o + 1] == ink[1] && d[o + 2] == ink[2] {
                    stroke_pixels += 1;
                }
            }
        }
        assert!(
            stroke_pixels > 20,
            "users glyph missing: {stroke_pixels} ink pixels"
        );
    }
}
