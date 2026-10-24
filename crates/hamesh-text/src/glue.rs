//! Glyph glue: parley positioned runs -> swash raster -> alpha bitmaps.
//!
//! Proven coordinates (read from parley 0.12 sources, not inferred):
//! `GlyphRun::positioned_glyphs()` yields glyphs whose `x` already includes
//! the run offset + line alignment offset, and whose `y` already includes
//! the layout-absolute baseline (`Line::style_baseline` falls back to the
//! absolute `LineMetrics::baseline`). So: pixel = origin + (g.x, g.y).

use crate::{TextLayout, TextRun};
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};
use swash::FontRef;

/// One rasterized glyph. `pen_*` is the layout-space pen position
/// (baseline origin); `(left, top)` is the swash BottomLeft-origin
/// placement: bitmap col/row 0 sits at
/// `(pen_x + left, pen_y - top)` in layout space. Screen pixel:
/// `floor(ox + pen_x + left) + col, floor(oy + pen_y - top) + row`.
/// (Derived from zeno mask.rs: BottomLeft origin flips rows top-down
/// and `placement.top = floor(bounds.min.y) + height`, i.e. path-space
/// max-y, so it SUBTRACTS from the pen baseline. Treating `top` as a
/// downward offset was a real bug caught by the frame PNG.)
#[derive(Clone, Debug)]
pub struct RasterGlyph {
    pub pen_x: f32,
    pub pen_y: f32,
    pub left: i32,
    pub top: i32,
    pub w: u32,
    pub h: u32,
    pub alpha: Vec<u8>,
}

/// Screen-space origin of the bitmap's top-left, given a layout origin.
pub fn glyph_origin(g: &RasterGlyph, ox: f32, oy: f32) -> (i32, i32) {
    (
        (ox + g.pen_x + g.left as f32).floor() as i32,
        (oy + g.pen_y - g.top as f32).floor() as i32,
    )
}

/// Layout + rasterize in one pass (16px, kept for the spike proof).
pub fn rasterize_parley(text: &str, max_width: f32) -> Vec<RasterGlyph> {
    rasterize_layout(&TextLayout::new(text, max_width, 16.0, None))
}

/// Bounded raster memoization: identical (text, width, size, lh,
/// shaping) shapes identically, so steady-state frames hit instead of
/// re-running swash per node per frame. Same 512-entry wholesale
/// eviction as the ink/trim caches. Cloned on hit (cheap vs raster).
type RasterCache = std::sync::Mutex<
    std::collections::HashMap<(u64, String, u32, u32, Option<u32>, u64), Vec<RasterGlyph>>,
>;
static RASTER_CACHE: std::sync::OnceLock<RasterCache> = std::sync::OnceLock::new();

pub fn rasterize_cached(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_height: Option<f32>,
    spec: &crate::FontSpec,
) -> Vec<RasterGlyph> {
    let key = (
        crate::font_generation(),
        text.to_string(),
        max_width.to_bits(),
        font_size.to_bits(),
        line_height.map(|v| v.to_bits()),
        spec.fingerprint(),
    );
    let cache =
        RASTER_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(hit) = cache.lock().expect("raster cache").get(&key) {
        return hit.clone();
    }
    let v = rasterize_layout(&TextLayout::new_varied(
        text,
        max_width,
        font_size,
        line_height,
        spec,
    ));
    let mut c = cache.lock().expect("raster cache");
    if c.len() >= 512 {
        c.clear();
    }
    c.insert(key, v.clone());
    v
}

/// Rasterize owned positioned runs (CPU path). The swash `FontRef`
/// borrows the Arc-backed font data owned by each `TextRun`, so this
/// is lifetime-clean for both CPU and GPU consumers.
pub fn rasterize_layout(layout: &TextLayout) -> Vec<RasterGlyph> {
    let mut ctx = ScaleContext::new();
    let mut out = Vec::new();
    for run in layout.runs() {
        rasterize_run(&mut ctx, &run, &mut out);
    }
    out
}

fn rasterize_run(ctx: &mut ScaleContext, run: &TextRun, out: &mut Vec<RasterGlyph>) {
    let Some(font_ref) = FontRef::from_index(run.font.data.data(), run.font.index as usize) else {
        return;
    };
    // Scale with the run's variation coords: on a variable face the
    // default-instance outlines are wider than the shaped advances,
    // so scaling without coords crowds glyphs into each other.
    // Unknown tags are ignored by swash; empty vars are a no-op.
    let coords: Vec<swash::Setting<f32>> = run
        .vars
        .iter()
        .map(|(tag, v)| swash::Setting {
            tag: swash::tag_from_bytes(tag),
            value: *v,
        })
        .collect();
    let mut scaler = ctx
        .builder(font_ref)
        .size(run.size)
        .hint(true)
        .variations(coords.iter().copied())
        .build();
    let mut render = Render::new(&[Source::Outline]);
    render.format(Format::Alpha);
    for (id, gx, gy) in run.glyphs.iter().copied() {
        rasterize_one(&mut scaler, &mut render, id, gx, gy, out);
    }
}

fn rasterize_one(
    scaler: &mut swash::scale::Scaler<'_>,
    render: &mut Render,
    id: u32,
    gx: f32,
    gy: f32,
    out: &mut Vec<RasterGlyph>,
) {
    render.offset(Vector::new(gx.fract(), gy.fract()));
    let mut image = swash::scale::image::Image::new();
    if !render.render_into(scaler, id as u16, &mut image) {
        return;
    }
    let p = image.placement;
    if p.width == 0 || p.height == 0 || image.data.is_empty() {
        return;
    }
    out.push(RasterGlyph {
        pen_x: gx,
        pen_y: gy,
        left: p.left,
        top: p.top,
        w: p.width,
        h: p.height,
        alpha: image.data,
    });
}

/// Blit white glyphs onto a dark pixmap and save it. Returns (glyphs, lit_pixels).
/// Draws at 2x for proof readability; also asserts glyph x-origins advance
/// monotonically (catches zero-advance placement bugs deterministically).
pub fn proof_png(glyphs: &[RasterGlyph], path: &str) -> (usize, usize) {
    use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};
    const ZOOM: i32 = 2;
    let (w, h) = (1000u32, 200u32);
    let mut pix = Pixmap::new(w, h).expect("pixmap");
    pix.fill(Color::from_rgba8(10, 15, 17, 255));
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(235, 245, 243, 255));
    let mut lit = 0usize;
    for g in glyphs {
        let (x0, y0) = glyph_origin(g, 0.0, 0.0);
        for (i, a) in g.alpha.iter().enumerate() {
            if *a < 8 {
                continue;
            }
            let px = x0 + (i as u32 % g.w) as i32;
            let py = y0 + (i as u32 / g.w) as i32;
            if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                continue;
            }
            // Alpha-weighted white paint per covered pixel.
            let mut p = paint.clone();
            p.set_color(Color::from_rgba8(235, 245, 243, *a));
            if let Some(r) = Rect::from_xywh(
                (px * ZOOM) as f32,
                (py * ZOOM) as f32,
                ZOOM as f32,
                ZOOM as f32,
            ) {
                pix.fill_rect(r, &p, Transform::identity(), None);
                lit += 1;
            }
        }
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).expect("proof png dir");
        }
    }
    pix.save_png(path).expect("save proof png");
    (glyphs.len(), lit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SAMPLE;

    #[test]
    fn raster_produces_lit_pixels() {
        let glyphs = rasterize_parley(SAMPLE, 460.0);
        assert!(!glyphs.is_empty(), "no glyphs rasterized");
        let (n, lit) = proof_png(&glyphs, "target/text-spike.png");
        println!("rasterized {n} glyphs, {lit} lit pixels -> target/text-spike.png");
        assert!(
            lit > 500,
            "too few lit pixels ({lit}); placement or raster broken?"
        );
    }
}
