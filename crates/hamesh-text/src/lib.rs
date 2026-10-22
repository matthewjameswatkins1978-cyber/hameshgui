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

/// Horizontal text inset (px): glyphs draw this far inside their node
/// on each side. Layout measurement, scene ink and both renderers
/// share this constant so boxes, ink and pixels agree.
pub const TEXT_INSET_X: f32 = 8.0;

/// Empty-text line box (px): keeps a caret-sized box where there is
/// no ink. Shared by scene ink and both renderers.
pub const EMPTY_LINE_H: f32 = 18.0;

/// Vertical origin of the text run inside a node of height `h`:
/// line-box centering for real text, caret box for empty text.
pub fn run_oy(node_h: f32, text_empty: bool, natural_h: f32) -> f32 {
    let lh = if text_empty {
        EMPTY_LINE_H
    } else {
        natural_h.max(1.0)
    };
    ((node_h - lh) / 2.0).max(0.0)
}

/// Horizontal text alignment contract (single-line controls).
/// Start/Center/End position the shaped advance inside the content
/// box; multiline nodes always use Start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    Start,
    Center,
    End,
}

/// Parse a `halign` prop; unknown or missing values are Start.
/// (Explicit fallback keeps old screens stable by construction.)
pub fn parse_halign(s: Option<&str>) -> HAlign {
    match s {
        Some("center") => HAlign::Center,
        Some("end") => HAlign::End,
        _ => HAlign::Start,
    }
}

/// Resolve the node-local glyph-run x origin for single-line text.
/// Effective inset `E = max(pad, TEXT_INSET_X)`: padding the host
/// asked for, but never less than bearing protection, so Start
/// rendering is pixel-identical to the historical fixed origin.
/// Center/End use the shaped advance (never the ink silhouette,
/// which would jump with content): Center lands exactly centered
/// for any pad. Overflow (advance wider than content) extends past
/// the content edge and clips at the node rect.
pub fn align_origin_x(align: HAlign, pad: f32, node_w: f32, advance: f32) -> f32 {
    let e = pad.max(TEXT_INSET_X);
    let content_w = (node_w - 2.0 * e).max(0.0);
    match align {
        HAlign::Start => e,
        HAlign::Center => e + (content_w - advance) / 2.0,
        HAlign::End => node_w - e - advance,
    }
}
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

// Shared shaping contexts (thread-local): `FontContext::new` scans
// system fonts, which dominated frame time when built per layout
// (~900ms/frame for the dashboard). Layouts still own their data
// (Arc-backed), so sharing contexts changes nothing observable.
thread_local! {
    static FONT_CX: std::cell::RefCell<FontContext> =
        std::cell::RefCell::new(FontContext::new());
    static LAYOUT_CX: std::cell::RefCell<LayoutContext<peniko::Brush>> =
        std::cell::RefCell::new(LayoutContext::new());
}

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

/// Font selection for shaping: optional family plus OpenType
/// variation axes as (4-byte tag, value) pairs. `Default` is plain
/// shaping (no family, no variations) — every existing caller keeps
/// working unchanged. The scene resolves one per text node from its
/// `font` / `font_vars` props and every consumer (advance, trim,
/// ink, measure, render) shapes with the same spec, so boxes and
/// paint cannot disagree. Not yet wired: variable-coord raster
/// (swash/vello scale without coords); positions are exact, raster
/// falls back to default-instance outlines until then.
#[derive(Clone, Debug, Default)]
pub struct FontSpec {
    pub family: Option<String>,
    pub vars: Vec<([u8; 4], f32)>,
}

impl FontSpec {
    /// Stable identity for cache keys: family name plus each
    /// axis tag and value bits, in declared order.
    pub fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.family.hash(&mut h);
        for (tag, v) in &self.vars {
            tag.hash(&mut h);
            v.to_bits().hash(&mut h);
        }
        h.finish()
    }

    /// Parse the `font` / `font_vars` component props (`font_vars`
    /// is `"TAG value, ..."`, e.g. `"wdth 75, opsz 16"`). Malformed
    /// entries are skipped, never fatal.
    pub fn parse(font: Option<&str>, vars: Option<&str>) -> Self {
        let family = font.map(str::to_string);
        let mut out = Vec::new();
        if let Some(v) = vars {
            for item in v.split(',') {
                let mut parts = item.split_whitespace();
                if let (Some(tag), Some(val)) = (parts.next(), parts.next()) {
                    let b = tag.as_bytes();
                    if b.len() == 4 {
                        if let Ok(f) = val.parse::<f32>() {
                            out.push(([b[0], b[1], b[2], b[3]], f));
                        }
                    }
                }
            }
        }
        Self { family, vars: out }
    }
}

impl TextLayout {
    /// Lay out text. `line_height` is an absolute px line box
    /// (parley `LineHeight::Absolute`); `None` keeps the font's
    /// preferred metrics. Builders declare it from the type ramp so
    /// tokens and rendering agree; legacy nodes omit it.
    pub fn new(text: &str, max_width: f32, font_size: f32, line_height: Option<f32>) -> Self {
        Self::new_varied(text, max_width, font_size, line_height, &FontSpec::default())
    }

    /// Lay out text with the given font selection: optional family plus
    /// OpenType variation axes (e.g. family `"Bahnschrift"`,
    /// `vars: [(b"wdth", 75.0)]`). `FontSpec::default()` keeps the
    /// current behavior (default family, no variations). Axes the
    /// resolved font lacks are ignored by the shaper - check the
    /// design-profile probe before depending on a specific axis.
    pub fn new_varied(
        text: &str,
        max_width: f32,
        font_size: f32,
        line_height: Option<f32>,
        spec: &FontSpec,
    ) -> Self {
        FONT_CX.with(|fcx| {
            LAYOUT_CX.with(|lcx| {
                let mut fcx = fcx.borrow_mut();
                let mut lcx = lcx.borrow_mut();
                let mut builder = lcx.ranged_builder(&mut fcx, text, 1.0, false);
                if let Some(f) = spec.family.as_deref() {
                    builder.push_default(StyleProperty::FontFamily(
                        parley::style::FontFamily::named(f),
                    ));
                }
                builder.push_default(StyleProperty::FontSize(font_size));
                if let Some(lh) = line_height {
                    builder.push_default(StyleProperty::LineHeight(
                        parley::style::LineHeight::Absolute(lh),
                    ));
                }
                if !spec.vars.is_empty() {
                    let vars: Vec<parley::setting::FontVariation> = spec
                        .vars
                        .iter()
                        .map(|(tag, v)| {
                            parley::setting::FontVariation::new(parley::setting::Tag::new(tag), *v)
                        })
                        .collect();
                    builder.push_default(StyleProperty::FontVariations((&vars[..]).into()));
                }
                let mut layout = builder.build(text);
                layout.break_all_lines(Some(max_width));
                Self { layout }
            })
        })
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

    /// Baseline of the first line in layout space, if the text laid
    /// out at least one line. The Eye uses it for cross-node
    /// baseline alignment checks (the CorelDRAW-style invisible lines).
    pub fn first_baseline(&self) -> Option<f32> {
        self.layout.lines().next().map(|l| l.metrics().baseline)
    }

    /// Dominant paragraph direction from parley's own BiDi analysis
    /// (BaseDirection::Auto infers it). Start/End resolve against it.
    pub fn is_rtl(&self) -> bool {
        self.layout.is_rtl()
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

/// Shaped ink bounds in layout space (text origin at 0,0), plus the
/// first baseline. The union of hinted raster coverage: ground truth
/// for what the renderers actually paint, bearings included.
/// `None` = no ink (empty or whitespace-only text).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InkBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub baseline: f32,
}

impl InkBox {
    pub fn w(&self) -> f32 {
        (self.x1 - self.x0).max(0.0)
    }
    pub fn h(&self) -> f32 {
        (self.y1 - self.y0).max(0.0)
    }
    pub fn cx(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }
    pub fn cy(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }
}

fn ink_uncached(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_height: Option<f32>,
    spec: &FontSpec,
) -> Option<InkBox> {
    if text.trim().is_empty() {
        return None;
    }
    let layout = TextLayout::new_varied(text, max_width, font_size, line_height, spec);
    let baseline = layout.first_baseline()?;
    let glyphs = glue::rasterize_layout(&layout);
    let mut x0 = f32::INFINITY;
    let mut y0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for g in &glyphs {
        let (gx, gy) = glue::glyph_origin(g, 0.0, 0.0);
        for (i, a) in g.alpha.iter().enumerate() {
            if *a < 8 {
                continue;
            }
            let px = gx + (i as u32 % g.w) as i32;
            let py = gy + (i as u32 / g.w) as i32;
            x0 = x0.min(px as f32);
            y0 = y0.min(py as f32);
            x1 = x1.max(px as f32 + 1.0);
            y1 = y1.max(py as f32 + 1.0);
        }
    }
    if x0 == f32::INFINITY {
        return None;
    }
    Some(InkBox {
        x0,
        y0,
        x1,
        y1,
        baseline,
    })
}

/// Bounded memoization: ink is a pure function of shaped text, and
/// live frames re-query the same strings constantly. 512 entries,
/// cleared wholesale when full (simplest correct eviction).
type InkCache = std::sync::Mutex<
    std::collections::HashMap<(String, u32, u32, Option<u32>, u64), Option<InkBox>>
>;
static INK_CACHE: std::sync::OnceLock<InkCache> = std::sync::OnceLock::new();

fn ink_key(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_height: Option<f32>,
    spec: &FontSpec,
) -> (String, u32, u32, Option<u32>, u64) {
    (
        text.to_string(),
        max_width.to_bits(),
        font_size.to_bits(),
        line_height.map(|v| v.to_bits()),
        spec.fingerprint(),
    )
}

/// Shaped ink bounds for text, cached. Same inputs the renderers use:
/// `max_width` is the wrapping width, `font_size`/`line_height` in px.
pub fn ink_box(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_height: Option<f32>,
    spec: &FontSpec,
) -> Option<InkBox> {
    let key = ink_key(text, max_width, font_size, line_height, spec);
    let cache = INK_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(hit) = cache.lock().expect("ink cache").get(&key) {
        return *hit;
    }
    let v = ink_uncached(text, max_width, font_size, line_height, spec);
    let mut c = cache.lock().expect("ink cache");
    if c.len() >= 512 {
        c.clear();
    }
    c.insert(key, v);
    v
}

/// Font-relative cap height (px) for `font_size` and shaping: ink top
/// of "H" vs its baseline. Depends only on font+size, never content,
/// so a trim derived from it cannot jump when the label text changes.
/// Cached per (size, shaping); fallback 0.7*size keeps boxes sane if
/// shaping fails.
type CapCache = std::sync::Mutex<std::collections::HashMap<(u32, u64), f32>>;
static CAP_CACHE: std::sync::OnceLock<CapCache> = std::sync::OnceLock::new();

pub fn cap_height_ref(font_size: f32, spec: &FontSpec) -> f32 {
    let key = (font_size.to_bits(), spec.fingerprint());
    let cache = CAP_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(hit) = cache.lock().expect("cap cache").get(&key) {
        return *hit;
    }
    let v = ink_uncached("H", 400.0, font_size, None, spec)
        .map(|b| (b.baseline - b.y0).max(1.0))
        .unwrap_or(font_size * 0.7);
    let mut c = cache.lock().expect("cap cache");
    if c.len() >= 64 {
        c.clear();
    }
    c.insert(key, v);
    v
}

/// Font-relative trim in layout space (text origin 0,0): top is
/// baseline minus cap height, bottom is the alphabetic baseline
/// (CSS `text-box-edge: cap alphabetic`). Same box for any string
/// in the same font/size: stable across Connected/Disconnected,
/// accented text, fallback runs. Single-line only (`None` for empty
/// or wrapped text): multiline keeps line-box placement. Descenders
/// and accents overflow the trim by design; renderers align by it
/// but never clip to it (clip stays at the node rect).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontTrim {
    pub top: f32,
    pub bottom: f32,
    pub baseline: f32,
    pub cap: f32,
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
    pub advance: f32,
}

pub fn trim_box(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_height: Option<f32>,
    spec: &FontSpec,
) -> Option<FontTrim> {
    let key = (
        text.to_string(),
        max_width.to_bits(),
        font_size.to_bits(),
        line_height.map(|v| v.to_bits()),
        spec.fingerprint(),
    );
    let cache = TRIM_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(hit) = cache.lock().expect("trim cache").get(&key) {
        return *hit;
    }
    let v = trim_uncached(text, max_width, font_size, line_height, spec);
    let mut c = cache.lock().expect("trim cache");
    if c.len() >= 512 {
        c.clear();
    }
    c.insert(key, v);
    v
}

type TrimCache = std::sync::Mutex<
    std::collections::HashMap<(String, u32, u32, Option<u32>, u64), Option<FontTrim>>
>;
static TRIM_CACHE: std::sync::OnceLock<TrimCache> = std::sync::OnceLock::new();

fn trim_uncached(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_height: Option<f32>,
    spec: &FontSpec,
) -> Option<FontTrim> {
    if text.trim().is_empty() {
        return None;
    }
    let layout = TextLayout::new_varied(text, max_width, font_size, line_height, spec);
    if layout.line_count() != 1 {
        return None;
    }
    let baseline = layout.first_baseline()?;
    let line_height = layout.height().max(1.0);
    let ascent = baseline.max(1.0);
    let descent = (line_height - baseline).max(0.0);
    let cap = cap_height_ref(font_size, spec);
    Some(FontTrim {
        top: baseline - cap,
        bottom: baseline,
        baseline,
        cap,
        ascent,
        descent,
        line_height,
        advance: layout.width(),
    })
}

pub fn layout_parley(text: &str, max_width: f32) -> TextMetrics {
    let layout = TextLayout::new(text, max_width, 16.0, None);
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

    #[test]
    fn ink_box_is_ground_truth() {
        assert!(ink_box("", 400.0, 13.0, None, &FontSpec::default()).is_none());
        assert!(ink_box("   ", 400.0, 13.0, None, &FontSpec::default()).is_none());
        let ink = ink_box("Connected", 400.0, 13.0, None, &FontSpec::default()).expect("ink");
        let lay = TextLayout::new("Connected", 400.0, 13.0, None);
        // Bearings trimmed: ink narrower than the advance box.
        assert!(
            ink.w() < lay.width(),
            "ink {} vs advance {}",
            ink.w(),
            lay.width()
        );
        assert!(ink.w() > 0.0 && ink.h() > 0.0);
        // Baseline sane: inside the ink span for caps/lowercase mix
        // (no descenders in "Connected", so ink bottom is the baseline).
        assert!(
            ink.baseline >= ink.y1 - 1.0,
            "baseline {} vs ink bottom {}",
            ink.baseline,
            ink.y1
        );
        // Cached call agrees.
        assert_eq!(ink_box("Connected", 400.0, 13.0, None, &FontSpec::default()), Some(ink));
        // Descenders extend ink below the baseline.
        let desc = ink_box("Inspected", 400.0, 13.0, None, &FontSpec::default()).expect("ink");
        assert!(desc.y1 > desc.baseline, "p tail should pass the baseline");
    }

    #[test]
    fn halign_centers_arbitrary_labels() {
        // Live badge numbers: rect w=95 pad=8, advance 62.88.
        let x = align_origin_x(HAlign::Center, 8.0, 95.0, 62.88);
        assert!((x - 16.06).abs() < 0.01, "badge origin {x}");
        // Right gap equals left gap: actually centered.
        assert!(((95.0 - (x + 62.88)) - x).abs() < 0.01, "badge symmetric");
        // Start is the historical fixed origin for any pad.
        assert_eq!(align_origin_x(HAlign::Start, 0.0, 200.0, 50.0), 8.0);
        assert_eq!(align_origin_x(HAlign::Start, 8.0, 95.0, 62.88), 8.0);
        assert_eq!(align_origin_x(HAlign::Start, 16.0, 200.0, 50.0), 16.0);
        // End mirrors Start against the right content edge.
        assert_eq!(
            align_origin_x(HAlign::End, 8.0, 116.0, 82.0),
            116.0 - 8.0 - 82.0
        );
        // Unknown props fall back to Start (old screens unchanged).
        assert_eq!(parse_halign(None), HAlign::Start);
        assert_eq!(parse_halign(Some("center")), HAlign::Center);
        assert_eq!(parse_halign(Some("end")), HAlign::End);
        assert_eq!(parse_halign(Some("middle")), HAlign::Start);
        // Center is exact for any pad, including pad < inset.
        for pad in [0.0f32, 4.0, 8.0, 16.0] {
            let e = pad.max(TEXT_INSET_X);
            let x = align_origin_x(HAlign::Center, pad, 200.0, 60.0);
            assert!(
                ((x - e) + 30.0 - (200.0 - 2.0 * e) / 2.0).abs() < 0.01,
                "pad {pad}: {x}"
            );
        }
    }

    #[test]
    fn trim_is_font_relative_not_content_relative() {
        // Stability: Connected -> Disconnected must not move the box.
        let a = trim_box("Connected", 400.0, 13.0, None, &FontSpec::default()).expect("trim a");
        let b = trim_box("Disconnected", 400.0, 13.0, None, &FontSpec::default()).expect("trim b");
        assert!(
            (a.top - b.top).abs() < 0.01,
            "top moved: {:?} vs {:?}",
            a,
            b
        );
        assert!(
            (a.bottom - b.bottom).abs() < 0.01,
            "bottom moved: {:?} vs {:?}",
            a,
            b
        );
        // Accents share the box: trim never clips diacritics by construction
        // (renderers align by trim, clip only to node rect).
        let c = trim_box("Caf\u{e9}", 400.0, 13.0, None, &FontSpec::default()).expect("trim c");
        assert!(
            (a.top - c.top).abs() < 0.01,
            "accent moved top: {:?} vs {:?}",
            a,
            c
        );
        // Empty and wrapped text opt out to line-box placement.
        assert!(trim_box("", 400.0, 13.0, None, &FontSpec::default()).is_none());
        assert!(trim_box("a b c d e f g", 10.0, 13.0, None, &FontSpec::default()).is_none());
        // Cap-alphabetic trim sits inside the line box: top at or below
        // the line top, bottom (baseline) strictly above the line bottom
        // whenever the font carries descender space.
        assert!(a.top >= 0.0, "trim top inside line: {:?}", a);
        assert!(a.bottom < a.line_height, "trim bottom inside line: {:?}", a);
        assert!(
            (a.bottom - a.top - a.cap).abs() < 0.01,
            "trim height is cap: {:?}",
            a
        );
    }

    #[test]
    fn variable_axes_reach_shaping_when_supported() {
        // Phase B groundwork: design profiles need wdth/opsz and
        // beyond. Shapes one word in Bahnschrift (variable, ships
        // with Windows) at two widths through the public API.
        // Advances MUST differ if family + axes work end to end.
        // Skips (never fails) where the font is absent - CI makes no
        // such guarantee, and absence is not a Hamesh defect.
        let probe = |wdth: f32| -> f32 {
            TextLayout::new_varied(
                "Hamburg",
                f32::INFINITY,
                64.0,
                None,
                &FontSpec {
                    family: Some("Bahnschrift".to_string()),
                    vars: vec![(*b"wdth", wdth)],
                },
            )
            .width()
        };
        let (narrow, wide) = (probe(75.0), probe(100.0));
        if (narrow - wide).abs() < 1.0 {
            eprintln!("SKIP: no variable Bahnschrift on this machine");
            return;
        }
        assert!(
            narrow < wide,
            "wdth 75 must shape narrower than wdth 100: {narrow} vs {wide}"
        );
    }

    #[test]
    fn font_spec_parse_keeps_valid_axes_drops_garbage() {
        let s = FontSpec::parse(Some("Bahnschrift"), Some("wdth 75, bogus, opsz nope, toolongtag 1, wght 700"));
        assert_eq!(s.family.as_deref(), Some("Bahnschrift"));
        assert_eq!(s.vars, vec![(*b"wdth", 75.0), (*b"wght", 700.0)]);
        let empty = FontSpec::parse(None, None);
        assert!(empty.family.is_none() && empty.vars.is_empty());
        // Fingerprint distinguishes shaping configurations.
        assert_ne!(s.fingerprint(), empty.fingerprint());
    }
}
