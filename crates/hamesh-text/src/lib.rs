//! hamesh-text spike: parley vs cosmic-text layout decision.
//!
//! Both engines are driven with system fonts over the SAME sample text
//! (Latin + CJK, exercising fallback) at the same size and width.
//! The decision is made on observed evidence: runs/glyphs produced,
//! API shape, and what rendering glue each path needs.
//!
//! This crate does NOT render yet — it proves layout + fallback + metrics.

use parley::style::StyleProperty;
use parley::{FontContext, LayoutContext};

pub mod glue;
pub use glue::{glyph_origin, proof_png, rasterize_layout, rasterize_parley, RasterGlyph};

#[derive(Clone, Debug, PartialEq)]
pub struct TextMetrics {
    /// Which engine produced this.
    pub engine: &'static str,
    pub lines: usize,
    pub runs: usize,
    pub glyphs: usize,
    pub width: f32,
    pub height: f32,
}

pub const SAMPLE: &str = "Hello Hamesh — the Eye and the Hand 世界";

/// Owned parley layout: safe to hand to CPU (swash) and GPU (vello)
/// consumers. The `Layout` owns its font data (Arc-backed), so no
/// `FontContext` needs to outlive it — verified by the compiler.
pub struct TextLayout {
    layout: parley::Layout<peniko::Brush>,
}

/// One run's positioned data, fully owned for consumer convenience.
#[derive(Clone, Debug)]
pub struct TextRun {
    pub font: parley::FontData,
    pub size: f32,
    /// (glyph id, layout-absolute x, layout-absolute y)
    pub glyphs: Vec<(u32, f32, f32)>,
}

impl TextLayout {
    pub fn new(text: &str, max_width: f32, font_size: f32) -> Self {
        let mut fcx = FontContext::new();
        let mut lcx: LayoutContext<peniko::Brush> = LayoutContext::new();
        let mut builder = lcx.ranged_builder(&mut fcx, text, 1.0, false);
        builder.push_default(StyleProperty::FontSize(font_size));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(max_width));
        Self { layout }
    }

    pub fn width(&self) -> f32 {
        self.layout.width()
    }

    pub fn height(&self) -> f32 {
        self.layout.height()
    }

    pub fn line_count(&self) -> usize {
        let n = self.layout.lines().len();
        n
    }

    /// Owned per-run positioned glyphs. Coordinates are layout-absolute
    /// (see decision 009): add the node's origin to place them.
    pub fn runs(&self) -> Vec<TextRun> {
        let mut out = Vec::new();
        for line in self.layout.lines() {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(gr) = item else {
                    continue;
                };
                let run = gr.run();
                out.push(TextRun {
                    font: run.font().clone(),
                    size: run.font_size(),
                    glyphs: gr.positioned_glyphs().map(|g| (g.id, g.x, g.y)).collect(),
                });
            }
        }
        out
    }
}

pub fn layout_parley(text: &str, max_width: f32) -> TextMetrics {
    let layout = TextLayout::new(text, max_width, 16.0);
    let owned = layout.runs();
    TextMetrics {
        engine: "parley",
        lines: layout.line_count(),
        runs: owned.len(),
        glyphs: owned.iter().map(|r| r.glyphs.len()).sum(),
        width: layout.width(),
        height: layout.height(),
    }
}

pub fn layout_cosmic(text: &str, max_width: f32) -> TextMetrics {
    use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
    let mut font_system = FontSystem::new();
    let metrics = Metrics::new(16.0, 20.0);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(Some(max_width), None);
    buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, false);
    let mut runs = 0;
    let mut glyphs = 0;
    // Cosmic `lines` are hard paragraphs; visual wrapping shows as
    // distinct `line_y` values across layout runs. Count those.
    let mut visual_tops: Vec<i32> = Vec::new();
    for run in buffer.layout_runs() {
        runs += 1;
        glyphs += run.glyphs.len();
        let top = run.line_y as i32;
        if !visual_tops.contains(&top) {
            visual_tops.push(top);
        }
    }
    let width = buffer.size().0.unwrap_or(0.0);
    TextMetrics {
        engine: "cosmic-text",
        lines: visual_tops.len(),
        runs,
        glyphs,
        width,
        height: visual_tops.len() as f32 * 20.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parley_lays_out_sample_with_fallback() {
        let m = layout_parley(SAMPLE, 400.0);
        println!("parley: {m:?}");
        assert!(m.lines >= 1, "no lines: {m:?}");
        assert!(m.glyphs > 20, "too few glyphs (fallback broken?): {m:?}");
        assert!(m.width > 0.0 && m.height > 0.0);
    }

    #[test]
    fn cosmic_lays_out_sample_with_fallback() {
        let m = layout_cosmic(SAMPLE, 400.0);
        println!("cosmic: {m:?}");
        assert!(m.lines >= 1, "no lines: {m:?}");
        assert!(m.glyphs > 20, "too few glyphs (fallback broken?): {m:?}");
        assert!(m.width > 0.0);
    }

    #[test]
    fn both_wrap_long_text() {
        let long = SAMPLE.repeat(10);
        let p = layout_parley(&long, 200.0);
        let c = layout_cosmic(&long, 200.0);
        println!("parley wrapped: {p:?}");
        println!("cosmic wrapped: {c:?}");
        assert!(p.lines > 1, "parley did not wrap: {p:?}");
        assert!(c.lines > 1, "cosmic did not wrap: {c:?}");
    }
}
