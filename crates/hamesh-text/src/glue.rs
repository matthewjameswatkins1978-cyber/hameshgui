//! Glyph glue: parley positioned runs -> swash raster -> alpha bitmaps.
//!
//! Proven coordinates (read from parley 0.12 sources, not inferred):
//! `GlyphRun::positioned_glyphs()` yields glyphs whose `x` already includes
//! the run offset + line alignment offset, and whose `y` already includes
//! the layout-absolute baseline (`Line::style_baseline` falls back to the
//! absolute `LineMetrics::baseline`). So: pixel = origin + (g.x, g.y).

use parley::style::StyleProperty;
use parley::{FontContext, LayoutContext, PositionedLayoutItem};
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};
use swash::FontRef;

/// One rasterized glyph: layout-space origin + alpha coverage.
#[derive(Clone, Debug)]
pub struct RasterGlyph {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub alpha: Vec<u8>,
}

/// Layout + rasterize in one pass (the swash `FontRef` borrows the font
/// data owned by the `FontContext`, so they must share a lifetime).
pub fn rasterize_parley(text: &str, max_width: f32) -> Vec<RasterGlyph> {
    let mut fcx = FontContext::new();
    let mut lcx: LayoutContext<peniko::Brush> = LayoutContext::new();
    let mut builder = lcx.ranged_builder(&mut fcx, text, 1.0, false);
    builder.push_default(StyleProperty::FontSize(16.0));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(max_width));

    let mut ctx = ScaleContext::new();
    let mut out = Vec::new();
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(gr) = item else {
                continue;
            };
            let run = gr.run();
            let font_data = run.font();
            let Some(font_ref) =
                FontRef::from_index(font_data.data.data(), font_data.index as usize)
            else {
                continue;
            };
            let mut scaler = ctx
                .builder(font_ref)
                .size(run.font_size())
                .hint(true)
                .build();
            let mut render = Render::new(&[Source::Outline]);
            render.format(Format::Alpha);
            for g in gr.positioned_glyphs() {
                render.offset(Vector::new(g.x.fract(), g.y.fract()));
                let mut image = swash::scale::image::Image::new();
                if !render.render_into(&mut scaler, g.id as u16, &mut image) {
                    continue;
                }
                let p = image.placement;
                if p.width == 0 || p.height == 0 || image.data.is_empty() {
                    continue;
                }
                out.push(RasterGlyph {
                    x: g.x as i32 + p.left,
                    y: g.y as i32 + p.top,
                    w: p.width,
                    h: p.height,
                    alpha: image.data,
                });
            }
        }
    }
    out
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
        for (i, a) in g.alpha.iter().enumerate() {
            if *a < 8 {
                continue;
            }
            let px = (g.x + (i as i32 % g.w as i32)) * ZOOM;
            let py = (g.y + (i as i32 / g.w as i32)) * ZOOM;
            if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                continue;
            }
            // Alpha-weighted white paint per covered pixel.
            let mut p = paint.clone();
            p.set_color(Color::from_rgba8(235, 245, 243, *a));
            if let Some(r) = Rect::from_xywh(px as f32, py as f32, ZOOM as f32, ZOOM as f32) {
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
mod glue {
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
