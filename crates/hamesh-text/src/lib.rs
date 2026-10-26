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
    static FONTS_LOADED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Shaping-collection generation: bumped every time a thread loads
/// fonts. Shape caches key on it, so entries shaped against a
/// fallback collection can never be served after the real faces
/// arrive (and vice versa) — no cross-test or startup-order poison.
static FONT_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Current shaping-collection generation for cache keys.
pub fn font_generation() -> u64 {
    FONT_GEN.load(std::sync::atomic::Ordering::SeqCst)
}

/// Directory holding Hamesh's vendored OFL profile faces
/// (`assets/fonts` at the workspace root). Tests and apps resolve
/// it from here so every crate shares one path.
pub fn bundled_fonts_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts")
}

/// Register additional fonts (e.g. [`bundled_fonts_dir`]) with this
/// thread's shaping context. Must run before first shape on the
/// thread; each thread loads once (guarded), so tests call this
/// freely. Returns families added on the calling thread.
pub fn register_fonts_from_dir(dir: &std::path::Path) -> usize {
    FONT_CX.with(|fcx| {
        let mut fcx = fcx.borrow_mut();
        if FONTS_LOADED.get() {
            return 0;
        }
        let before = fcx.collection.family_names().count();
        fcx.collection.load_fonts_from_paths([dir]);
        FONTS_LOADED.set(true);
        // New faces change shaping: invalidate every shape cache by
        // moving to a new generation (keys carry it; stale entries
        // age out through the existing size caps, never served).
        FONT_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        fcx.collection.family_names().count().saturating_sub(before)
    })
}

/// True when the shaping collection knows this family. False means
/// shaping falls back — the interface stays correct, and the Eye
/// reports the substitution via `font_resolved` (family-level).
pub fn font_family_available(family: &str) -> bool {
    FONT_CX.with(|fcx| fcx.borrow_mut().collection.family_id(family).is_some())
}

/// All known family names, sorted and de-duplicated: bundled
/// faces, profile faces and installed system fonts. Loads system
/// fonts on first call for the calling thread (idempotent; each
/// thread keeps its own shaping collection) and bumps the font
/// generation when the set grows, like `register_fonts_from_dir`.
/// Powers the installed-font picker: prefer this over hand-rolled
/// directory scans or a "next available font" fallback.
pub fn installed_family_names() -> Vec<String> {
    FONT_CX.with(|fcx| {
        let mut fcx = fcx.borrow_mut();
        let before = fcx.collection.family_names().count();
        fcx.collection.load_system_fonts();
        if fcx.collection.family_names().count() != before {
            FONT_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        let mut names: Vec<String> = fcx
            .collection
            .family_names()
            .map(|s| s.to_string())
            .collect();
        names.sort();
        names.dedup();
        names
    })
}

/// Fit a single line into `max_advance` px, appending "…" when trimmed.
///
/// Grapheme-correct: the cut never splits a ZWJ sequence, skin tone or
/// flag (decision 017 travels with text wherever it goes).
/// Binary-searches the longest fitting prefix; returns the original
/// when it already fits, bare "…" when nothing else does, and empty
/// when not even the ellipsis fits. Trailing spaces are trimmed
/// before the ellipsis so rows do not end on a gap.
pub fn ellipsize(text: &str, max_advance: f32, font_size: f32, spec: &FontSpec) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    if max_advance <= 0.0 {
        return String::new();
    }
    if TextLayout::new_varied(text, f32::INFINITY, font_size, None, spec).width() <= max_advance {
        return text.to_string();
    }
    let ell = "…";
    let ell_w = TextLayout::new_varied(ell, f32::INFINITY, font_size, None, spec).width();
    if ell_w > max_advance {
        return String::new();
    }
    let gs: Vec<&str> = text.graphemes(true).collect();
    let (mut lo, mut hi) = (0usize, gs.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cand: String = gs[..mid].concat();
        let w = TextLayout::new_varied(&cand, f32::INFINITY, font_size, None, spec).width();
        if w + ell_w <= max_advance {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let prefix: String = gs[..lo].concat();
    format!("{}{}", prefix.trim_end_matches(' '), ell)
}

/// Owned parley layout: safe to hand to CPU (swash) and GPU (vello)
/// consumers. The `Layout` owns its font data (Arc-backed), so no
/// `FontContext` needs to outlive it — verified by the compiler.
pub struct TextLayout {
    layout: parley::Layout<peniko::Brush>,
    vars: Vec<([u8; 4], f32)>,
}

/// One run's positioned data, fully owned for consumer convenience.
#[derive(Clone, Debug)]
pub struct TextRun {
    pub font: parley::FontData,
    pub size: f32,
    /// Variation coords this run was shaped with (empty = default).
    /// Rasterizers must scale with these or outlines disagree with
    /// positions on variable faces.
    pub vars: Vec<([u8; 4], f32)>,
    /// (glyph id, layout-absolute x, layout-absolute y)
    pub glyphs: Vec<(u32, f32, f32)>,
}

/// Font selection for shaping: optional family plus OpenType
/// variation axes as (4-byte tag, value) pairs. `Default` is plain
/// shaping (no family, no variations) — every existing caller keeps
/// working unchanged. The scene resolves one per text node from its
/// `font` / `font_vars` props and every consumer (advance, trim,
/// ink, measure, render) shapes with the same spec, so boxes and
/// paint cannot disagree. The CPU rasterizer scales with the run's
/// coords, so varied outlines match varied positions; vello 0.11 has
/// no variation-coords API, so the GPU path keeps default-instance
/// outlines at varied positions until the vello upgrade.
#[derive(Clone, Debug, Default)]
pub struct FontSpec {
    pub family: Option<String>,
    pub vars: Vec<([u8; 4], f32)>,
    /// Requested style (`font_style` prop): italic faces resolve
    /// through font matching, `None` keeps the default. Part of
    /// cache identity like every other shaping input.
    pub style: Option<parley::style::FontStyle>,
    /// Extra inter-glyph space in px (`tracking` prop, may be
    /// negative). Positions shift; the rasterizer needs no change
    /// (it paints glyphs where shaping put them).
    pub tracking: f32,
}

impl FontSpec {
    /// Stable identity for cache keys: family name, style, tracking
    /// plus each axis tag and value bits, in declared order.
    pub fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.family.hash(&mut h);
        // FontStyle carries no Hash; bucket the variants by hand.
        // Oblique (and futures) share one bucket: shaping still
        // distinguishes them, only the cache is coarser.
        let s = match self.style {
            None => 0u8,
            Some(parley::style::FontStyle::Italic) => 1u8,
            Some(_) => 2u8,
        };
        s.hash(&mut h);
        self.tracking.to_bits().hash(&mut h);
        for (tag, v) in &self.vars {
            tag.hash(&mut h);
            v.to_bits().hash(&mut h);
        }
        h.finish()
    }

    /// Parse the `font` / `font_vars` / `font_style` / `tracking`
    /// component props (`font_vars` is `"TAG value, ..."`, e.g.
    /// `"wdth 75, opsz 16"`; `font_style` is `"italic"` or absent;
    /// `tracking` is extra px between glyphs, may be negative).
    /// Malformed entries are skipped, never fatal.
    pub fn parse(
        font: Option<&str>,
        vars: Option<&str>,
        style: Option<&str>,
        tracking: Option<&str>,
    ) -> Self {
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
        let style = Self::parse_style(style);
        let tracking = tracking.and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.0);
        Self {
            family,
            vars: out,
            style,
            tracking,
        }
    }

    /// Parse a `font_style` prop: `"italic"` voices the italic face
    /// through font matching; anything else (or absent) is upright.
    /// Shared by scene resolution and renderer reconstruction so a
    /// node means the same style on every path.
    pub fn parse_style(style: Option<&str>) -> Option<parley::style::FontStyle> {
        match style {
            Some(s) if s.eq_ignore_ascii_case("italic") => Some(parley::style::FontStyle::Italic),
            _ => None,
        }
    }
}

impl TextLayout {
    /// Lay out text. `line_height` is an absolute px line box
    /// (parley `LineHeight::Absolute`); `None` keeps the font's
    /// preferred metrics. Builders declare it from the type ramp so
    /// tokens and rendering agree; legacy nodes omit it.
    pub fn new(text: &str, max_width: f32, font_size: f32, line_height: Option<f32>) -> Self {
        Self::new_varied(
            text,
            max_width,
            font_size,
            line_height,
            &FontSpec::default(),
        )
    }

    /// Lay out text with the given font selection: optional family plus
    /// OpenType variation axes (e.g. family `"Instrument Sans"`,
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
        // Every shaping thread carries the vendored faces: the first
        // shape on a thread loads them (guarded, amortized), so no
        // caller — app, drill, or test — can shape a fallback where
        // a profile face was meant. Explicit `register_fonts_from_dir`
        // calls (e.g. at startup) stay the deterministic path; this
        // only closes the ordering hole that poisoned shared caches.
        register_fonts_from_dir(&bundled_fonts_dir());
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
                if let Some(st) = spec.style {
                    builder.push_default(StyleProperty::FontStyle(st));
                }
                if spec.tracking != 0.0 {
                    builder.push_default(StyleProperty::LetterSpacing(spec.tracking));
                }
                let mut layout = builder.build(text);
                layout.break_all_lines(Some(max_width));
                Self {
                    layout,
                    vars: spec.vars.clone(),
                }
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
                    vars: self.vars.clone(),
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
    // Outline union, not rasterization: identical placement mapping,
    // no alpha walk. Raster ink stays the CPU paint path; ink bounds
    // are a diagnostic and must never cost a raster per query.
    let (x0, y0, x1, y1) = glue::outline_bounds_union(&layout)?;
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
    std::collections::HashMap<(u64, String, u32, u32, Option<u32>, u64), Option<InkBox>>,
>;
static INK_CACHE: std::sync::OnceLock<InkCache> = std::sync::OnceLock::new();

fn ink_key(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_height: Option<f32>,
    spec: &FontSpec,
) -> (u64, String, u32, u32, Option<u32>, u64) {
    (
        font_generation(),
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
type CapCache = std::sync::Mutex<std::collections::HashMap<(u64, u32, u64), f32>>;
static CAP_CACHE: std::sync::OnceLock<CapCache> = std::sync::OnceLock::new();

pub fn cap_height_ref(font_size: f32, spec: &FontSpec) -> f32 {
    let key = (font_generation(), font_size.to_bits(), spec.fingerprint());
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
        font_generation(),
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
    std::collections::HashMap<(u64, String, u32, u32, Option<u32>, u64), Option<FontTrim>>,
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
        assert_eq!(
            ink_box("Connected", 400.0, 13.0, None, &FontSpec::default()),
            Some(ink)
        );
        // Descenders extend ink below the baseline.
        let desc = ink_box("Inspected", 400.0, 13.0, None, &FontSpec::default()).expect("ink");
        assert!(desc.y1 > desc.baseline, "p tail should pass the baseline");
    }

    #[test]
    fn ink_outline_agrees_with_raster_within_a_pixel() {
        // The outline-union fast path must stay honest against the
        // raster ground truth it replaced: same text, same placement
        // mapping, within antialiasing tolerance.
        register_fonts_from_dir(&bundled_fonts_dir());
        for text in ["Connected", "Inspected", "Hamburg — pig"] {
            let layout = TextLayout::new_varied(text, 400.0, 13.0, None, &FontSpec::default());
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
            let ink = ink_box(text, 400.0, 13.0, None, &FontSpec::default()).expect("ink");
            for (got, want, axis) in [
                (ink.x0, x0, "x0"),
                (ink.y0, y0, "y0"),
                (ink.x1, x1, "x1"),
                (ink.y1, y1, "y1"),
            ] {
                assert!(
                    (got - want).abs() <= 1.0,
                    "{text} {axis}: outline {got} vs raster {want}"
                );
            }
        }
    }

    #[test]
    fn ink_stays_affordable_on_cache_misses() {
        // Regression for the streaming stall: every streaming delta is
        // an ink-cache miss, so miss cost must be bounded. The raster
        // + pixel-walk path this replaced took ~19s on 16KB; the
        // outline-union path must stay three orders of magnitude under
        // that (generous bound: debug builds, loaded machines).
        register_fonts_from_dir(&bundled_fonts_dir());
        let unit = "Bridges are among humanity's oldest and most eloquent structures. ";
        let text: String = unit.repeat(300);
        assert!(text.len() > 16000);
        let t0 = std::time::Instant::now();
        let ink = ink_box(&text, 700.0, 16.0, None, &FontSpec::default());
        let dt = t0.elapsed();
        assert!(ink.is_some());
        assert!(
            dt < std::time::Duration::from_secs(5),
            "16KB ink miss took {dt:?}"
        );
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
        // Design profiles need wdth/opsz and beyond. Shapes one word
        // in the vendored Instrument Sans (variable) at two widths.
        // Advances MUST differ if family + axes work end to end.
        // Registered first, so this holds on every machine.
        register_fonts_from_dir(&bundled_fonts_dir());
        let probe = |wdth: f32| -> f32 {
            TextLayout::new_varied(
                "Hamburg",
                f32::INFINITY,
                64.0,
                None,
                &FontSpec {
                    family: Some("Instrument Sans".to_string()),
                    vars: vec![(*b"wdth", wdth)],
                    style: None,
                    tracking: 0.0,
                },
            )
            .width()
        };
        let (narrow, wide) = (probe(75.0), probe(100.0));
        assert!(
            (narrow - wide).abs() >= 1.0,
            "vendored Instrument Sans must resolve, not substitute"
        );
        assert!(
            narrow < wide,
            "wdth 75 must shape narrower than wdth 100: {narrow} vs {wide}"
        );
    }

    #[test]
    fn varied_outlines_match_varied_positions() {
        // Raster follow-up: the scaler must use the run's variation
        // coords, or default-instance outlines crowd into condensed
        // advances. Ink spans must track their own advance, not the
        // other instance's. Vendored faces are registered first.
        register_fonts_from_dir(&bundled_fonts_dir());
        let spec = |wdth: Option<f32>| FontSpec {
            family: Some("Instrument Sans".to_string()),
            vars: wdth.map(|w| vec![(*b"wdth", w)]).unwrap_or_default(),
            style: None,
            tracking: 0.0,
        };
        let lay = |s: &FontSpec| TextLayout::new_varied("Hamburg", f32::INFINITY, 64.0, None, s);
        let (ld, lv) = (lay(&spec(None)), lay(&spec(Some(75.0))));
        assert!(
            (ld.width() - lv.width()).abs() >= 1.0,
            "vendored Instrument Sans must resolve, not substitute"
        );
        let span = |l: &TextLayout| -> f32 {
            let g = glue::rasterize_layout(l);
            assert!(!g.is_empty(), "expected raster glyphs");
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for r in &g {
                lo = lo.min(r.pen_x + r.left as f32);
                hi = hi.max(r.pen_x + r.left as f32 + r.w as f32);
            }
            hi - lo
        };
        let (sd, sv) = (span(&ld), span(&lv));
        eprintln!(
            "raster proof: default adv={:.1} span={:.1} / condensed adv={:.1} span={:.1}",
            ld.width(),
            sd,
            lv.width(),
            sv
        );
        assert!(
            (sv - sd).abs() > 10.0,
            "varied outlines must differ from default: {sv} vs {sd}"
        );
        for (adv, sp, name) in [(ld.width(), sd, "default"), (lv.width(), sv, "condensed")] {
            assert!(
                (adv - sp).abs() < 12.0,
                "{name} ink span {sp} must track its advance {adv}"
            );
        }
    }

    #[test]
    fn font_spec_parse_keeps_valid_axes_drops_garbage() {
        let s = FontSpec::parse(
            Some("Instrument Sans"),
            Some("wdth 75, bogus, opsz nope, toolongtag 1, wght 700"),
            Some("italic"),
            Some("-0.5"),
        );
        assert_eq!(s.family.as_deref(), Some("Instrument Sans"));
        assert_eq!(s.vars, vec![(*b"wdth", 75.0), (*b"wght", 700.0)]);
        assert_eq!(s.style, Some(parley::style::FontStyle::Italic));
        assert_eq!(s.tracking, -0.5);
        let empty = FontSpec::parse(None, None, None, None);
        assert!(empty.family.is_none() && empty.vars.is_empty());
        assert!(empty.style.is_none() && empty.tracking == 0.0);
        // Fingerprint distinguishes shaping configurations.
        assert_ne!(s.fingerprint(), empty.fingerprint());
    }

    #[test]
    fn ellipsis_trims_grapheme_correctly() {
        let spec = FontSpec::parse(None, None, None, None);
        // Fits: untouched.
        assert_eq!(ellipsize("Hi", 1000.0, 16.0, &spec), "Hi");
        // Trimmed: shorter, ends with the ellipsis.
        let long = "Outdated logo";
        let cut = ellipsize(long, 40.0, 16.0, &spec);
        assert!(cut.ends_with("…") && cut.len() < long.len(), "{cut}");
        // Nothing fits, not even the ellipsis.
        assert_eq!(ellipsize(long, 0.0, 16.0, &spec), "");
        // ZWJ family: never split in half.
        let fam = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}tail-of-text-here";
        let cut = ellipsize(fam, 60.0, 16.0, &spec);
        use unicode_segmentation::UnicodeSegmentation;
        assert!(cut.ends_with("…"));
        let body = cut.trim_end_matches("…");
        assert!(
            body.is_empty() || fam.starts_with(body) && body.graphemes(true).count() >= 1,
            "no half-grapheme: {cut}"
        );
        assert!(
            !body.contains('\u{200D}')
                || body.contains("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
            "joiner only inside a whole family"
        );
    }

    #[test]
    fn installed_families_cover_bundled_fonts_without_duplicates() {
        register_fonts_from_dir(&bundled_fonts_dir());
        let first = installed_family_names();
        assert!(
            first.contains(&"Newsreader".to_string()),
            "bundled voice listed"
        );
        // Idempotent: a second call neither duplicates nor loses.
        assert_eq!(first, installed_family_names());
        // The list agrees with lookup: no ghost entries sampled.
        for f in first.iter().take(50) {
            assert!(font_family_available(f), "listed family must resolve: {f}");
        }
    }

    #[test]
    fn bundled_faces_register_and_resolve() {
        // The vendored OFL faces must shape identically on every
        // machine (including CI): register, then every profile
        // voice resolves and a bogus family does not.
        let n = register_fonts_from_dir(&bundled_fonts_dir());
        eprintln!("test fonts: {n} families registered on this thread");
        for f in [
            "Barlow Condensed",
            "IBM Plex Mono",
            "Newsreader",
            "Instrument Sans",
        ] {
            assert!(font_family_available(f), "missing vendored face {f}");
        }
        assert!(!font_family_available("NoSuchFaceXYZ"));
    }

    #[test]
    fn italic_style_resolves_the_italic_face() {
        // `font_style: italic` must match the italic file through
        // font matching — not synthesize, not ignore. Upright and
        // italic Newsreader shape differently (different outlines).
        register_fonts_from_dir(&bundled_fonts_dir());
        let lay = |style: Option<parley::style::FontStyle>| {
            TextLayout::new_varied(
                "Hamburg",
                f32::INFINITY,
                64.0,
                None,
                &FontSpec {
                    family: Some("Newsreader".to_string()),
                    vars: vec![],
                    style,
                    tracking: 0.0,
                },
            )
        };
        let (up, it) = (lay(None), lay(Some(parley::style::FontStyle::Italic)));
        eprintln!(
            "style proof: upright={:.1} italic={:.1}",
            up.width(),
            it.width()
        );
        assert!(
            (up.width() - it.width()).abs() >= 1.0,
            "italic must shape a different face, not the upright"
        );
    }

    #[test]
    fn italic_selects_the_italic_file_by_bytes() {
        // Finding 2: advance differences are necessary but not
        // sufficient — prove the shaped run's font bytes ARE the
        // italic file (ruling out fallback and synthesis alike).
        // Skips only where the vendored faces are absent.
        register_fonts_from_dir(&bundled_fonts_dir());
        if !font_family_available("Newsreader") {
            eprintln!("SKIP: vendored Newsreader absent");
            return;
        }
        let dir = bundled_fonts_dir();
        let roman = std::fs::read(dir.join("Newsreader[opsz,wght].ttf")).expect("roman face");
        let italic =
            std::fs::read(dir.join("Newsreader-Italic[opsz,wght].ttf")).expect("italic face");
        assert_ne!(roman, italic, "sanity: the two files differ");
        let lay = |style: Option<parley::style::FontStyle>| {
            TextLayout::new_varied(
                "Hamburg",
                f32::INFINITY,
                64.0,
                None,
                &FontSpec {
                    family: Some("Newsreader".to_string()),
                    vars: vec![],
                    style,
                    tracking: 0.0,
                },
            )
            .runs()
        };
        let up = lay(None);
        let it = lay(Some(parley::style::FontStyle::Italic));
        assert!(!up.is_empty() && !it.is_empty(), "expected shaped runs");
        assert_eq!(
            up[0].font.data.data(),
            roman.as_slice(),
            "upright run must be the roman file"
        );
        assert_eq!(
            it[0].font.data.data(),
            italic.as_slice(),
            "italic run must be the italic file"
        );
    }

    #[test]
    fn tracking_shifts_advance_deterministically() {
        // Tracking is extra px per glyph (parley letter-spacing
        // adds after each glyph, trailing included): the line grows
        // by glyph_count * tracking. Negative narrows.
        register_fonts_from_dir(&bundled_fonts_dir());
        let lay = |tracking: f32| {
            TextLayout::new_varied(
                "Hamburg",
                f32::INFINITY,
                40.0,
                None,
                &FontSpec {
                    family: Some("Instrument Sans".to_string()),
                    vars: vec![],
                    style: None,
                    tracking,
                },
            )
        };
        let (plain, wide, tight) = (lay(0.0), lay(2.0), lay(-1.0));
        let per = 7.0; // "Hamburg" = 7 glyphs, spacing after each
        assert!(
            (wide.width() - plain.width() - per * 2.0).abs() < 1.0,
            "tracking +2 must add 14px: {} vs {}",
            wide.width(),
            plain.width()
        );
        assert!(
            (plain.width() - tight.width() - per * 1.0).abs() < 1.0,
            "tracking -1 must remove 7px: {} vs {}",
            plain.width(),
            tight.width()
        );
    }
}
