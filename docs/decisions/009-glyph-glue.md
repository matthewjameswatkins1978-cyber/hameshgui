# 009 — Glyph glue: parley positioned runs -> swash -> alpha bitmaps

Date: 2026-10-21. Status: accepted (CPU proof complete; GPU mapping open).
Source: `crates/hamesh-text/src/glue.rs` (4 tests green) + `text-spike.png`.

## Verified coordinates (read in parley 0.12 sources)

`GlyphRun::positioned_glyphs()` yields layout-absolute positions:
x already includes run offset + line `inline_min_coord` + alignment offset;
y already includes the absolute baseline (`style_baseline` falls back to
`LineMetrics::baseline`, documented "y-coordinate within the layout").
So: pixel = origin + (g.x, g.y). No line-top accumulation needed.

## Verified swash 0.2 plumbing

- `GlyphId = u16` (type alias); parley ids are u32 — cast.
- `FontRef::from_index(blob.data(), index as usize)` where the blob/index
  come from parley `FontData { data: Blob<u8>, index }` (pub fields).
- One `ScaleContext` per layout; one scaler per run
  (`.size(run.font_size()).hint(true)`); one `Render` with
  `Format::Alpha` + per-glyph fractional `offset()`.
- `Image { placement: zeno::Placement { left, top, width, height }, data }`;
  blit at `(g.x as i32 + left, g.y as i32 + top)`.

## Decisions

- **Outline-only for now** (`&[Source::Outline]`): an experiment rendering
  with the full source list produced pixel-identical output on our fonts,
  so bitmap/COLR paths buy nothing yet. Color emoji stays explicitly open.
- Proof artifact: `target/text-spike.png` (2x zoom) + test asserting lit
  pixels. Build-ignored, regenerable.

## False alarm, kept as lesson

The first proof PNG looked broken (glyphs "small and raised"). Before
changing code, a debug test dumped per-glyph id/x/y/advance: positions
were exact. At 2x zoom the render proved correct — x-height vs cap
height, midline em-dash, descenders, CJK em squares, all on one baseline.
What looked like breakage was correct typography at small size.
**Instrument before "fixing".**

## Open next

GPU mapping: same positioned data into vello `Scene::draw_glyphs`
(peniko FontData + vello Glyph { id, x, y }); atlas/caching strategy;
then wire rasterized text into the demo scene (replacing the label bars).
