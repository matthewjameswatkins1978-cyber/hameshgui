//! hamesh-render: CPU renderer with compatible scene semantics.
//!
//! GPU (wgpu/Vello) is the architectural default; this CPU backend proves
//! scene semantics early, works headless/CI, and defines the tolerance model
//! for later GPU-vs-CPU comparison.

use hamesh_scene::Scene;
use serde::{Deserialize, Serialize};
use tiny_skia::{Color, Paint, Pixmap, Rect as SkRect, Transform};

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
    let layout = hamesh_text::TextLayout::new(&node.text, avail_w, 16.0);
    if layout.height() <= 0.0 {
        return;
    }
    let ox = node.rect.x + 8.0;
    let oy = node.rect.y + ((node.rect.h - layout.height()) / 2.0).max(0.0);
    let glyphs = hamesh_text::rasterize_layout(&layout);
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
            p.set_color(Color::from_rgba8(235, 245, 243, *a));
            if let Some(dot) = SkRect::from_xywh(px as f32, py as f32, 1.0, 1.0) {
                pix.fill_rect(dot, &p, Transform::identity(), None);
            }
        }
    }
}

fn rounded_clip_rect(rect: &hamesh_layout::Rect) -> Option<SkRect> {
    SkRect::from_xywh(rect.x, rect.y, rect.w.max(1.0), rect.h.max(1.0))
}

pub fn render(scene: &Scene) -> Frame {
    let w = scene.viewport_w.max(1.0) as u32;
    let h = scene.viewport_h.max(1.0) as u32;
    let mut pix = Pixmap::new(w, h).expect("pixmap");
    // Background.
    pix.fill(Color::from_rgba8(10, 15, 17, 255));

    for node in &scene.nodes {
        if !node.visible {
            continue;
        }
        let Some(r) = rounded_clip_rect(&node.rect) else {
            continue;
        };
        let [rr, gg, bb, aa] = node.color;
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(rr, gg, bb, aa));
        // Simple rounded look: draw rect; overlay translucency shows through.
        pix.fill_rect(r, &paint, Transform::identity(), None);
        // Real text: parley layout + swash raster, blitted per covered
        // pixel and clipped to the node's rect. Layout runs per frame
        // here (spike cost); a glyph cache is the known follow-up.
        if !node.text.trim().is_empty() {
            blit_text(&mut pix, node);
        }
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
}
