# 007 — Text layout: parley (path A), cosmic-text stays as fallback

Date: 2026-10-21. Status: accepted for M3. Source: `crates/hamesh-text`
spike (3 tests green) + live crates.io versions.

## Observed evidence (not memory)

Both engines driven identically: system fonts, 16px, same sample
(`"Hello Hamesh — the Eye and the Hand 世界"`, Latin + CJK fallback):

| | parley 0.12.0 | cosmic-text 0.19.0 |
|---|---|---|
| Sample glyphs | 380 | 367 (shaping differences, not failure) |
| Long-text wrap (200px) | 17 lines | 17 visual lines |
| CJK fallback | works, but logs `ICU4X data error: No segmentation model for complex script` per layout (wraps anyway) | works, silent |
| System fonts | `FontContext::new()` discovers them | `FontSystem::new()` loads them |

API friction found by the compiler, not by reading:
- parley: `Line::glyphs()` needs the layout lifetime — count via
  runs → clusters → glyphs; tail-expression temporaries outlive locals
  (bind scalars first). Mild tax, documented in the spike.
- cosmic 0.19: setters no longer take `&mut FontSystem`
  (`set_size(w, h)`, `set_text(text, attrs, shaping, align)`);
  `lines` counts hard paragraphs — visual lines come from `line_y`.

## Decision

**parley** for M3. Reasons, in order:
1. Same Linebender family as the chosen renderer: `peniko::Brush` is
   already in our tree via vello, and parley's `DrawGlyphs` feeds the
   vello glyph path directly. One family, one color/glyph model.
2. No fontdb/MPL questions (parley → fontique + swash, both
   Apache-2.0 OR MIT).
3. `parley::editing` (cursor/selection/editor) exists for the M3
   composer; cosmic's `Buffer` editing is also real but would still
   need glyphon-or-custom glue for our dual CPU/GPU presentation.

cosmic-text stays a labeled fallback: if parley's 0.x API churn or the
ICU4X CJK segmentation gap bites, the spike proves cosmic reaches the
same layout numbers through a smaller API.

## Rendering glue (open, next spike)

Positioned glyph runs exist on both paths; rasterization into
tiny-skia (CPU) and vello `draw_glyphs` (GPU) is NOT yet built.
Parley-side needs swash → atlas → blit; that is the next bounded spike.
