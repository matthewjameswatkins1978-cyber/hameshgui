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

/// Diagnostic overlay: CorelDRAW-style guides for one inspected node,
/// drawn as a pure pixmap post-pass. Only ever applied to an explicit
/// diagnostic capture — never to normal rendering, hit-testing, state
/// or exported artwork. Every guide derives from the scene's resolved
/// geometry (the same values the Eye reports), so the image and the
/// geometry record cannot disagree.
pub mod overlay {
    use super::*;

    fn dot(pix: &mut Pixmap, x: i32, y: i32, rgba: [u8; 4]) {
        if x < 0 || y < 0 || x >= pix.width() as i32 || y >= pix.height() as i32 {
            return;
        }
        let mut p = Paint::default();
        p.set_color(Color::from_rgba8(rgba[0], rgba[1], rgba[2], 255));
        if let Some(r) = SkRect::from_xywh(x as f32, y as f32, 1.0, 1.0) {
            pix.fill_rect(r, &p, Transform::identity(), None);
        }
    }

    fn rect_outline(pix: &mut Pixmap, r: [f32; 4], rgba: [u8; 4]) {
        let (x0, y0, x1, y1) = (r[0] as i32, r[1] as i32, r[2] as i32, r[3] as i32);
        for x in x0..=x1 {
            dot(pix, x, y0, rgba);
            dot(pix, x, y1, rgba);
        }
        for y in y0..=y1 {
            dot(pix, x0, y, rgba);
            dot(pix, x1, y, rgba);
        }
    }

    fn hline(pix: &mut Pixmap, x0: i32, x1: i32, y: i32, rgba: [u8; 4]) {
        for x in x0.min(x1)..=x0.max(x1) {
            dot(pix, x, y, rgba);
        }
    }

    fn vline(pix: &mut Pixmap, x: i32, y0: i32, y1: i32, rgba: [u8; 4]) {
        for y in y0.min(y1)..=y0.max(y1) {
            dot(pix, x, y, rgba);
        }
    }

    /// Paint guides for `node` (screen space = node-local + rect
    /// origin). No-ink / no-trim nodes still get bounds + content.
    pub fn draw_guides(pix: &mut Pixmap, node: &hamesh_scene::SceneNode) {
        const BOUNDS: [u8; 4] = [235, 245, 243, 255];
        const CONTENT: [u8; 4] = [122, 142, 140, 255];
        const TRIM: [u8; 4] = [120, 220, 200, 255];
        const INK: [u8; 4] = [218, 170, 84, 255];
        const GUIDE: [u8; 4] = [110, 170, 255, 255];
        let (ox, oy) = (node.rect.x, node.rect.y);
        let at = |b: [f32; 4]| [b[0] + ox, b[1] + oy, b[2] + ox, b[3] + oy];
        rect_outline(
            pix,
            [ox, oy, ox + node.rect.w - 1.0, oy + node.rect.h - 1.0],
            BOUNDS,
        );
        rect_outline(pix, at(node.content), CONTENT);
        // Ink first, trim over it: where observation meets contract,
        // the contract wins the pixel.
        if let Some(b) = node.ink {
            rect_outline(pix, at(b), INK);
        }
        if let Some(t) = node.trim {
            rect_outline(pix, at(t), TRIM);
        }
        if let Some(base) = node.baseline {
            hline(
                pix,
                ox as i32,
                (ox + node.rect.w) as i32,
                (oy + base) as i32,
                GUIDE,
            );
        }
        // Content-centre cross: the alignment target.
        let cx = (node.content[0] + node.content[2]) / 2.0 + ox;
        let cy = (node.content[1] + node.content[3]) / 2.0 + oy;
        vline(pix, cx as i32, oy as i32, (oy + node.rect.h) as i32, GUIDE);
        hline(pix, ox as i32, (ox + node.rect.w) as i32, cy as i32, GUIDE);
    }
}

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
/// node's own rect so text never bleeds into siblings. Placement is
/// the scene's resolved origin, verbatim — this function performs no
/// positioning math of its own (one geometry contract, §H2).
fn blit_text(pix: &mut Pixmap, node: &hamesh_scene::SceneNode, paint_rect: &hamesh_layout::Rect) {
    // Bounds the glyph blit may touch: the node rect, further narrowed
    // by the ancestor clip only when this node actually overflows it
    // (non-overflowing nodes pay no extra per-pixel work).
    let (clip_x0, clip_y0) = (paint_rect.x, paint_rect.y);
    let (clip_x1, clip_y1) = (paint_rect.x + paint_rect.w, paint_rect.y + paint_rect.h);
    let avail_w = (node.rect.w - 2.0 * hamesh_text::TEXT_INSET_X).max(8.0);
    // Same wrap rule as scene build: single-line nodes lay out
    // unwrapped so paint can never re-wrap what layout kept whole.
    let wrap_w = if node.text.is_empty() || node.lines <= 1 {
        f32::INFINITY
    } else {
        avail_w
    };
    let layout = hamesh_text::TextLayout::new_varied(
        &node.text,
        wrap_w,
        node.font_size,
        node.line_height,
        &hamesh_text::FontSpec {
            family: node.font_family.clone(),
            vars: node.font_vars.clone(),
            style: hamesh_text::FontSpec::parse_style(node.font_style.as_deref()),
            tracking: node.tracking,
        },
    );
    // Empty text still gets a line box: the caret must render in an
    // empty focused composer (this early-return once ate the caret).
    let line_h = if node.text.is_empty() {
        hamesh_text::EMPTY_LINE_H
    } else {
        layout.height().max(1.0)
    };
    let oy_line = node.rect.y + ((node.rect.h - line_h) / 2.0).max(0.0);
    // Resolved origin from the scene (halign + inset + optical shift).
    let (ox, oy) = (node.rect.x + node.origin[0], node.rect.y + node.origin[1]);
    let glyphs = if layout.height() > 0.0 {
        // Cached raster: identical text shapes identically, so steady
        // frames skip swash (the layout above is only for the line box).
        hamesh_text::glue::rasterize_cached(
            &node.text,
            wrap_w,
            node.font_size,
            node.line_height,
            &hamesh_text::FontSpec {
                family: node.font_family.clone(),
                vars: node.font_vars.clone(),
                style: hamesh_text::FontSpec::parse_style(node.font_style.as_deref()),
                tracking: node.tracking,
            },
        )
    } else {
        Vec::new()
    };
    // Caret bar for the focused composer (scene truth, read by the Eye too).
    // Line-anchored: it spans the line box at the line origin, while
    // glyphs above ride the optical shift.
    if node.focused {
        if let Some(cx) = node.caret_x {
            let bx = ox + cx;
            let bh = line_h.max(8.0);
            if bx >= clip_x0 && bx + 2.0 <= clip_x1 {
                let mut cp = Paint::default();
                cp.set_color(Color::from_rgba8(120, 220, 200, 255));
                if let Some(bar) = SkRect::from_xywh(bx, oy_line, 2.0, bh) {
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
            // Clip to the paintable rect (node rect ∩ ancestor clip)
            // AND the frame.
            if px < clip_x0 as i32
                || py < clip_y0 as i32
                || px >= clip_x1 as i32
                || py >= clip_y1 as i32
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

/// Clip mask for a node that overflows its ancestor bounds.
///
/// Rects are *clipped*, never clamped: clamping a rect would redraw a
/// border along the clip edge and move rounded corners, inventing
/// geometry that is not in the scene. The mask cuts pixels instead, so
/// what remains is exactly the part of the original paint that the
/// ancestor allowed. Only overflowing nodes allocate one.
fn clip_mask(clip: &hamesh_layout::Rect, w: u32, h: u32) -> Option<tiny_skia::Mask> {
    let mut mask = tiny_skia::Mask::new(w, h)?;
    let rect = SkRect::from_xywh(clip.x, clip.y, clip.w.max(0.0), clip.h.max(0.0))?;
    let path = PathBuilder::from_rect(rect);
    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
    Some(mask)
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

/// Content identity for the frame cache. A cache is an optimisation,
/// never an alternative source of truth: the key must uniquely describe
/// the thing being cached. Revisions + dimensions do NOT — independent
/// scenes share them routinely (every headless test builds rev 1 at a
/// fixed viewport). So the key is a hash over the serialized scene:
/// every field paint can read is in the identity by construction, and
/// any future visual field that serializes joins it automatically.
/// Two scenes share a key only if they paint identically.
fn fingerprint(scene: &Scene) -> u64 {
    use std::hash::{Hash, Hasher};
    let bytes = serde_json::to_vec(scene).expect("scene serializes");
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

pub fn render(scene: &Scene) -> Frame {
    let w = scene.viewport_w.max(1.0) as u32;
    let h = scene.viewport_h.max(1.0) as u32;
    // Steady-state frames are identical: same content + viewport =
    // same pixels. Skip the whole paint on repeat (bounded LRU).
    // The shaping generation joins the key: same scene shaped after
    // new fonts arrive paints different pixels.
    let key = (hamesh_text::font_generation(), fingerprint(scene), w, h);
    {
        let cache =
            FRAME_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()));
        if let Some(pix) = cache
            .lock()
            .expect("frame cache")
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, p)| p.clone())
        {
            return Frame {
                frame_ref: FrameRef {
                    scene_revision: scene.scene_revision,
                    app_revision: scene.app_revision,
                    width: w,
                    height: h,
                },
                pixmap: pix,
            };
        }
    }
    let pix = render_uncached(scene, w, h);
    {
        let cache =
            FRAME_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()));
        let mut c = cache.lock().expect("frame cache");
        while c.len() >= 4 {
            c.pop_front();
        }
        c.push_back((key, pix.clone()));
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

type FrameCache = std::sync::Mutex<std::collections::VecDeque<((u64, u64, u32, u32), Pixmap)>>;
static FRAME_CACHE: std::sync::OnceLock<FrameCache> = std::sync::OnceLock::new();

fn render_uncached(scene: &Scene, w: u32, h: u32) -> Pixmap {
    let mut pix = Pixmap::new(w, h).expect("pixmap");
    // Background.
    pix.fill(Color::from_rgba8(10, 15, 17, 255));

    for node in &scene.nodes {
        if !node.visible || node.ghost {
            continue;
        }
        // Paint-time ancestor clip. An overflowing node is masked to its
        // ancestor bounds, so escaped content is impossible rather than
        // merely reported. Geometry is untouched: fills, borders,
        // progress bars and icons keep their position and size, and
        // text keeps its resolved origin (bounded per-pixel below).
        let mask = if node.overflow {
            clip_mask(&node.clip, w, h)
        } else {
            None
        };
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
                    mask.as_ref(),
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
                    pix.stroke_path(&bp, &paint_b, &stroke, Transform::identity(), mask.as_ref());
                }
            }
        } else {
            let Some(r) = rounded_clip_rect(&node.rect) else {
                continue;
            };
            // Simple rounded look: draw rect; overlay translucency shows through.
            pix.fill_rect(r, &paint, Transform::identity(), mask.as_ref());
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
                    pix.stroke_path(&bp, &paint_b, &stroke, Transform::identity(), mask.as_ref());
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
                            mask.as_ref(),
                        );
                    }
                } else if let Some(r) = SkRect::from_xywh(bar.x, bar.y, bar.w, bar.h) {
                    pix.fill_rect(r, &paint_f, Transform::identity(), mask.as_ref());
                }
            }
        }
        // Icon glyph: stroked Lucide path in the text color, unknown
        // names silently draw nothing (the Eye still shows the prop).
        if let Some(ref iname) = node.icon {
            if let Some(bez) = icons::icon_bez(iname) {
                // Placement uses the node's own rect: clipping must hide
                // overflowing pixels, never rescale or recentre the glyph.
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
                    pix.stroke_path(
                        &ipath,
                        &paint_i,
                        &stroke,
                        Transform::identity(),
                        mask.as_ref(),
                    );
                }
            }
        }
        // Real text: parley layout + swash raster, blitted per covered
        // pixel and clipped to the node's rect. The caret draws even for
        // empty text, so blit_text itself decides what to skip — never
        // gate the whole call on text presence (that ate the caret twice).
        // Text keeps its resolved origin and is bounded per-pixel: the
        // same contract as the other primitives, without moving glyphs.
        let text_rect = if node.overflow {
            node.rect
                .intersect(&node.clip)
                .unwrap_or(hamesh_layout::Rect {
                    x: node.rect.x,
                    y: node.rect.y,
                    w: 0.0,
                    h: 0.0,
                })
        } else {
            node.rect.clone()
        };
        blit_text(&mut pix, node, &text_rect);
    }

    pix
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
        // Caret bar: x in [ox+10, ox+12) at the scene-resolved origin
        // (composer pad 12: origin x = 12, not the old fixed 8).
        let ox = node.rect.x + node.origin[0];
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
    fn overlay_guides_match_reported_geometry() {
        // H3 contract: guides derive from the scene's resolved geometry
        // (the same values the Eye reports); normal rendering stays
        // overlay-free. Bounds corner pixel carries the guide color in
        // the diagnostic frame and the surface color in the clean one.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("b", Role::Status, "badge")
                .with_prop("text", "Connected")
                .with_prop("font_size", "13")
                .with_prop("lh", "18")
                .with_prop("pad", "8")
                .with_prop("w", "200")
                .with_prop("halign", "center"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let node = scene.nodes.iter().find(|n| n.id == "b").unwrap();
        let clean = render(&scene);
        let (cx, cy) = (node.rect.x as usize, node.rect.y as usize);
        assert_ne!(
            px(&clean.pixmap, cx, cy),
            [235, 245, 243, 255],
            "clean frame must be overlay-free"
        );
        let mut diag = render(&scene);
        overlay::draw_guides(&mut diag.pixmap, node);
        assert_eq!(
            px(&diag.pixmap, cx, cy),
            [235, 245, 243, 255],
            "bounds guide missing at reported rect"
        );
        // Trim guide (teal) sits at the reported trim top edge.
        let t = node.trim.expect("trim");
        let (tx, ty) = ((node.rect.x + t[0]) as usize, (node.rect.y + t[1]) as usize);
        assert_eq!(
            px(&diag.pixmap, tx + 4, ty),
            [120, 220, 200, 255],
            "trim guide missing at reported trim"
        );
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
    fn escaped_content_cannot_paint_outside_its_panel() {
        // The field defect, as pixels: a label whose box spills past the
        // panel used to paint over whatever was below it while layout
        // tests stayed green (only controls were asserted). Paint is
        // now bounded by the ancestor clip, so the escape is impossible:
        // outside the panel the frame keeps the window background.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("panel", Role::Panel, "panel")
                .with_prop("w", "120")
                .with_prop("h", "24")
                .with_prop("pad", "0")
                .with_prop("gap", "0")
                .with_prop("surface", "raised"),
        );
        for i in 0..3 {
            app.add(
                "panel",
                Component::new(&format!("lbl{i}"), Role::Text, "label")
                    .with_prop("text", "ESCAPED")
                    .with_prop("h", "20")
                    .with_prop("pad", "0")
                    .with_prop("flex_grow", "0"),
            );
        }
        let layout = hamesh_layout::compute(&app, 300.0, 300.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let lbl2 = scene.nodes.iter().find(|n| n.id == "lbl2").unwrap();
        assert!(lbl2.overflow, "precondition: the third label escapes");
        let f = render(&scene);
        let win_bg = scene.nodes.iter().find(|n| n.id == "win").unwrap().color;
        // Sample inside the escaped label but outside the panel.
        let sx = (lbl2.rect.x + lbl2.rect.w / 2.0) as usize;
        let sy = (lbl2.rect.y + lbl2.rect.h / 2.0) as usize;
        assert!(
            (sy as f32) > lbl2.clip.y + lbl2.clip.h,
            "sample must lie past the clip (y={sy})"
        );
        assert_eq!(
            px(&f.pixmap, sx, sy),
            win_bg,
            "escaped label must leave no pixel outside its panel"
        );
        // Inside the panel the label still paints (clipping is not hiding
        // everything): sample the first label's row.
        let lbl0 = scene.nodes.iter().find(|n| n.id == "lbl0").unwrap();
        let mut painted = false;
        for x in (lbl0.rect.x as usize)..((lbl0.rect.x + lbl0.rect.w) as usize) {
            for y in (lbl0.rect.y as usize)..((lbl0.rect.y + lbl0.rect.h) as usize) {
                if px(&f.pixmap, x, y) != win_bg
                    && px(&f.pixmap, x, y) != hamesh_theme::resolve("raised")
                {
                    painted = true;
                }
            }
        }
        assert!(painted, "the in-panel label must still paint its text");
    }

    #[test]
    fn border_strokes_cannot_escape_the_ancestor_clip() {
        // A stroked border is half outside its rect and its corners
        // bulge; clamping the rect would also invent an edge along the
        // clip line. The mask must cut the stroke instead, so below the
        // panel there is nothing at all.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("panel", Role::Panel, "panel")
                .with_prop("w", "80")
                .with_prop("h", "40")
                .with_prop("pad", "0"),
        );
        app.add(
            "panel",
            Component::new("plate", Role::Panel, "plate")
                .with_prop("h", "60")
                .with_prop("pad", "0")
                .with_prop("border", "4")
                .with_prop("border_token", "accent")
                .with_prop("surface", "raised")
                .with_prop("flex_grow", "0"),
        );
        let layout = hamesh_layout::compute(&app, 200.0, 200.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let plate = scene.nodes.iter().find(|n| n.id == "plate").unwrap();
        assert!(plate.overflow, "precondition: the plate overflows");
        let f = render(&scene);
        let win_bg = scene.nodes.iter().find(|n| n.id == "win").unwrap().color;
        let accent = hamesh_theme::resolve("accent");
        let below = (plate.clip.y + plate.clip.h + 3.0) as usize;
        for x in (plate.rect.x as usize)..((plate.rect.x + plate.rect.w) as usize) {
            let got = px(&f.pixmap, x, below);
            assert_ne!(got, accent, "border stroke escaped at x={x}");
            assert_eq!(got, win_bg, "paint leaked below the panel at x={x}");
        }
    }

    #[test]
    fn clipped_icon_keeps_its_placement_and_only_loses_pixels() {
        // Clipping must hide overflowing pixels, never rescale or recentre
        // the glyph. The icon sits in normal flow (a 40px spacer above it)
        // so only the panel height changes between the two builds: its rect
        // is identical, one is cut, one is not. Inside the clip the pixels
        // must be identical (no move, no resize); below it the cut frame is
        // background where the reference still shows ink.
        let build = |clip_h: f32| {
            let mut app = App::new("win");
            app.add(
                "win",
                Component::new("panel", Role::Panel, "panel")
                    .with_prop("w", "120")
                    .with_prop("h", &clip_h.to_string())
                    .with_prop("pad", "0")
                    .with_prop("gap", "0"),
            );
            app.add(
                "panel",
                Component::new("sp", Role::Panel, "spacer")
                    .with_prop("ghost", "true")
                    .with_prop("h", "40")
                    .with_prop("pad", "0"),
            );
            app.add(
                "panel",
                Component::new("ico", Role::Panel, "icon")
                    .with_prop("icon", "activity")
                    .with_prop("text_token", "accent")
                    .with_prop("surface", "bg")
                    .with_prop("w", "48")
                    .with_prop("h", "48")
                    .with_prop("pad", "0"),
            );
            let layout = hamesh_layout::compute(&app, 240.0, 240.0);
            (app, layout)
        };
        // Tight panel: the icon (y 40..88) is cut at y=60.
        let (app_tight, lay_tight) = build(60.0);
        let scene_tight = hamesh_scene::build(&app_tight, &lay_tight, 1);
        let ico = scene_tight
            .nodes
            .iter()
            .find(|n| n.id == "ico")
            .unwrap()
            .clone();
        assert!(ico.overflow, "precondition: the icon is cut");
        assert!(
            (ico.rect.h - 48.0).abs() < 0.5,
            "clip must not resize the box"
        );
        // Tall panel: same icon rect, nothing cut.
        let (app_tall, lay_tall) = build(200.0);
        let scene_tall = hamesh_scene::build(&app_tall, &lay_tall, 1);
        let ico_tall = scene_tall
            .nodes
            .iter()
            .find(|n| n.id == "ico")
            .unwrap()
            .clone();
        assert!(!ico_tall.overflow);
        assert!((ico_tall.rect.x - ico.rect.x).abs() < 0.5);
        assert!((ico_tall.rect.y - ico.rect.y).abs() < 0.5);
        let f_tight = render(&scene_tight);
        let f_tall = render(&scene_tall);
        let win_bg = scene_tight
            .nodes
            .iter()
            .find(|n| n.id == "win")
            .unwrap()
            .color;
        let (x0, y0) = (ico.rect.x as usize, ico.rect.y as usize);
        let clip_bottom = (ico.clip.y + ico.clip.h) as usize;
        let mut ink_inside = 0;
        let mut cut_ink = 0;
        for y in y0..(y0 + ico.rect.h as usize) {
            for x in x0..(x0 + ico.rect.w as usize) {
                let tight = px(&f_tight.pixmap, x, y);
                let tall = px(&f_tall.pixmap, x, y);
                if y < clip_bottom {
                    // Masked blending rounds by up to one unit per channel;
                    // a repositioned or rescaled glyph would differ by far
                    // more than that, so a tight tolerance still proves the
                    // geometry is untouched.
                    let delta = tight
                        .iter()
                        .zip(tall.iter())
                        .map(|(a, b)| (*a as i32 - *b as i32).abs())
                        .max()
                        .unwrap_or(0);
                    assert!(
                        delta <= 2,
                        "inside the clip the glyph must not move at ({x},{y}): \
                         tight={tight:?} tall={tall:?} delta={delta}"
                    );
                    if tall != win_bg {
                        ink_inside += 1;
                    }
                } else {
                    assert_eq!(tight, win_bg, "cut pixels must be background at ({x},{y})");
                    if tall != win_bg {
                        cut_ink += 1;
                    }
                }
            }
        }
        assert!(
            ink_inside > 0,
            "the visible part of the icon must still paint"
        );
        assert!(
            cut_ink > 0,
            "precondition: the reference had ink below the cut"
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
                .with_prop("block", "Progress")
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
    fn frame_cache_never_swaps_distinct_scenes() {
        // Independent review (Lucy): the frame cache was keyed on
        // (scene_rev, app_rev, w, h), which independent scenes share
        // routinely — the rounded card and the progress bar below are
        // both 200x200 rev 1. Render in both orders plus repeats (the
        // cache-hit path): every frame must carry its own pixels.
        let mut card_app = App::new("win");
        card_app.add(
            "win",
            Component::new("card", Role::Panel, "card")
                .with_prop("w", "100")
                .with_prop("h", "100")
                .with_prop("surface", "raised")
                .with_prop("radius", "20"),
        );
        let card_scene = hamesh_scene::build(
            &card_app,
            &hamesh_layout::compute(&card_app, 200.0, 200.0),
            1,
        );
        let mut bar_app = App::new("win");
        bar_app.add(
            "win",
            Component::new("bar", Role::Panel, "progress")
                .with_prop("block", "Progress")
                .with_prop("w", "100")
                .with_prop("h", "10")
                .with_prop("surface", "track")
                .with_prop("value", "50")
                .with_prop("fill", "accent"),
        );
        let bar_scene =
            hamesh_scene::build(&bar_app, &hamesh_layout::compute(&bar_app, 200.0, 200.0), 1);
        assert_eq!(
            (card_scene.scene_revision, card_scene.viewport_w),
            (bar_scene.scene_revision, bar_scene.viewport_w),
            "test premise: identical revision/dimension tuples"
        );
        let card = card_scene.nodes.iter().find(|n| n.id == "card").unwrap();
        let win_bg = card_scene
            .nodes
            .iter()
            .find(|n| n.id == "win")
            .unwrap()
            .color;
        let bar = bar_scene.nodes.iter().find(|n| n.id == "bar").unwrap();
        let check_card = |f: &Frame| {
            assert_eq!(
                px(
                    &f.pixmap,
                    (card.rect.x + 2.0) as usize,
                    (card.rect.y + 2.0) as usize
                ),
                win_bg,
                "card corner cutout shows parent"
            );
            assert_eq!(
                px(
                    &f.pixmap,
                    (card.rect.x + card.rect.w / 2.0) as usize,
                    (card.rect.y + card.rect.h / 2.0) as usize
                ),
                hamesh_theme::resolve("raised"),
                "card center must be surface"
            );
        };
        let check_bar = |f: &Frame| {
            let y = (bar.rect.y + bar.rect.h / 2.0) as usize;
            assert_eq!(
                px(&f.pixmap, (bar.rect.x + 5.0) as usize, y),
                hamesh_theme::resolve("accent"),
                "filled half is accent"
            );
            assert_eq!(
                px(&f.pixmap, (bar.rect.x + bar.rect.w - 5.0) as usize, y),
                hamesh_theme::resolve("track"),
                "empty half is track"
            );
        };
        // Both orders, then repeats: the second rendering of each
        // scene comes from the cache and must still be correct.
        let f1 = render(&card_scene);
        check_card(&f1);
        let f2 = render(&bar_scene);
        check_bar(&f2);
        let f3 = render(&bar_scene);
        check_bar(&f3);
        let f4 = render(&card_scene);
        check_card(&f4);
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

    #[test]
    fn centered_label_paints_centered() {
        // Standing anti-regression: a centered control must paint its
        // ink centered, not left-biased. Ink centroid x within 1.5px
        // of box center; left and right gaps agree within 2px.
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("b", Role::Status, "badge")
                .with_prop("text", "Connected")
                .with_prop("font_size", "13")
                .with_prop("lh", "18")
                .with_prop("pad", "8")
                .with_prop("w", "200")
                .with_prop("halign", "center"),
        );
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let node = scene.nodes.iter().find(|n| n.id == "b").unwrap();
        let f = render(&scene);
        let (tw, th) = (f.pixmap.width() as i32, f.pixmap.height() as i32);
        let (mut sx, mut n) = (0i64, 0i64);
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for y in 0..th {
            for x in 0..tw {
                let o = ((y * tw + x) * 4) as usize;
                let d = f.pixmap.data();
                // Text ink: near-white glyph pixels on dark surfaces.
                if d[o] > 180 && d[o + 1] > 180 && d[o + 2] > 180 {
                    sx += x as i64;
                    n += 1;
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
            }
        }
        assert!(n > 50, "no text ink painted");
        let cx = sx as f32 / n as f32;
        let box_cx = node.rect.x + node.rect.w / 2.0;
        assert!(
            (cx - box_cx).abs() < 1.5,
            "ink centroid {cx} vs box {box_cx}"
        );
        let (lg, rg) = (
            lo as f32 - node.rect.x,
            node.rect.x + node.rect.w - hi as f32,
        );
        assert!(
            (lg - rg).abs() < 2.0,
            "gaps asymmetric: left {lg} right {rg}"
        );
    }

    #[test]
    fn varied_font_renders_narrower_proof() {
        // End-to-end demonstration: the same label in the vendored
        // Instrument Sans at wdth 75 vs the default face. Advances
        // must differ and both inks must stay inside their boxes.
        // Registered first, so this holds on every machine. Writes
        // target/varied-proof.png for human eyes.
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let mut app = App::new("win");
        app.add(
            "win",
            Component::new("r", Role::Panel, "r")
                .with_prop("layout", "row")
                .with_prop("pad", "0")
                .with_prop("gap", "8"),
        );
        for (id, font, vars) in [("a", None, ""), ("b", Some("Instrument Sans"), "wdth 75")] {
            let mut c = Component::new(id, Role::Status, id)
                .with_prop("text", "Hamburg")
                .with_prop("font_size", "40")
                .with_prop("pad", "8")
                .with_prop("w", "300")
                .with_prop("h", "80")
                .with_prop("halign", "center");
            if let Some(f) = font {
                c = c.with_prop("font", f);
            }
            if !vars.is_empty() {
                c = c.with_prop("font_vars", vars);
            }
            app.add("r", c);
        }
        let layout = hamesh_layout::compute(&app, 800.0, 600.0);
        let scene = hamesh_scene::build(&app, &layout, 1);
        let (a, b) = (
            scene.nodes.iter().find(|n| n.id == "a").unwrap(),
            scene.nodes.iter().find(|n| n.id == "b").unwrap(),
        );
        assert!(
            (a.advance - b.advance).abs() >= 1.0,
            "vendored Instrument Sans must resolve, not substitute"
        );
        // Advances differ: the wdth axis moved shaping. (No direction is
        // asserted: cross-family width ordering depends on the fallback
        // face. The proof PNG shows the actual difference for human eyes.)
        eprintln!(
            "varied proof: default={:.1} condensed={:.1}",
            a.advance, b.advance
        );
        for n in [a, b] {
            // Ink bounds are node-local: they must fit the node box.
            if let Some(ink) = n.ink {
                assert!(
                    ink[0] >= -0.5 && ink[2] <= n.rect.w + 0.5,
                    "ink escapes box for {}",
                    n.id
                );
            }
        }
        let f = render(&scene);
        std::fs::create_dir_all("target")
            .map_err(|e| format!("cannot create target dir: {e:?}"))
            .unwrap();
        save_png(&f, "target/varied-proof.png")
            .map_err(|e| format!("cannot write proof png: {e}"))
            .unwrap();
    }
}
