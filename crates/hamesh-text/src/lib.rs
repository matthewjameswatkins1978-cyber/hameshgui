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

pub fn layout_parley(text: &str, max_width: f32) -> TextMetrics {
    let mut fcx = FontContext::new();
    let mut lcx: LayoutContext<peniko::Brush> = LayoutContext::new();
    let mut builder = lcx.ranged_builder(&mut fcx, text, 1.0, false);
    builder.push_default(StyleProperty::FontSize(16.0));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(max_width));
    let mut runs = 0;
    let mut glyphs = 0;
    // Note: `Line::glyphs` needs the layout lifetime, so count through
    // runs -> clusters -> glyphs. Mild ergonomic tax, recorded for the decision.
    for line in layout.lines() {
        for run in line.runs() {
            runs += 1;
            for cluster in run.clusters() {
                glyphs += cluster.glyphs().count();
            }
        }
    }
    // Bind scalars before constructing: the lines() iterator must drop
    // before `layout` does (tail-expression temporaries outlive locals).
    let line_count = layout.lines().len();
    let width = layout.width();
    let height = layout.height();
    TextMetrics {
        engine: "parley",
        lines: line_count,
        runs,
        glyphs,
        width,
        height,
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
