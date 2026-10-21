# 010 — Text in the scene: CPU blit + vello glyphs + tolerance verdict

Date: 2026-10-21. Status: accepted. Source: `hamesh-frame.png`,
`hamesh-window-gpu.png`, smoke-trace verdicts (all observed).

## What shipped

- **CPU** (`hamesh-render::blit_text`): per text node, parley layout at
  node width, swash alpha blit clipped to the node rect, vertically
  centered. Replaces the old label-bar placeholders.
- **GPU** (`gpu.rs::build_vscene` + `draw_text`): same `TextLayout`
  positioned data through vello `draw_glyphs` with a translate transform.
  `parley::FontData` passes straight into vello (single
  linebender_resource_handle 0.1.1 — compiler-verified, zero conversion).
- **Fresh eyes caught a real bug:** the first frame rendered container
  NAMES ("root", "main") as text. Scene text now comes ONLY from explicit
  `text` props (identity ≠ content); Text/Status node fills are
  transparent so labels sit on their parent surface.
- **Placement anchor, derived from zeno source** (`mask.rs`): swash
  BottomLeft-origin placement means `top` is path-space max-y, so screen
  rows start at `pen_y - top`, NOT `pen_y + top`. The wrong sign rendered
  text ~20px low, clipped by node bounds. Fixed in
  `hamesh_text::glyph_origin`; shared by CPU blit and proof PNG.

## Tolerance harness: first verdict

`--gpu --smoke-test` now renders the same Scene through BOTH paths and
reports `hamesh_render::compare` with the default tolerance
(per-channel 8, 1% outliers): **within tolerance** on the live demo
scene (rects + 5 text runs + translucency). This closes decision 002's
open item from "model" to "measured" — for this scene class.

## Performance note (honest)

Text layout per node per frame costs: uncached GPU path ran 12fps.
A per-(node,text,width) `TextLayout` cache in `GpuState` restored
~55fps (439 frames / 8s). The stateless CPU `render()` still lays out
per frame — fine for drills/screenshots, but a shared cache is queued
with the M3 composer work, when text becomes dynamic.
