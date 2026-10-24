# 023 — Composition diagnosis: why the dashboard looked awkward (C1/C2)

Date: 2026-10-22. Mission: visual composition & design intelligence.

## Measured causes (Eye geometry at 800x600, before fixes)

1. **Phantom padding (the big one).** Text leaves inherited the
   container default `pad: 12` — 24px of dead box around every
   label. Title measured h56 for 28px glyphs, subtitle h41 for
   15px, metric value h54, progress heading h45. The 4px `tight`
   gaps were dwarfed by dead line-box space, so nothing grouped.
   Fix: text roles default `pad: 0` in the shared framework; the
   box hugs its line. Explicit `pad` still wins (badge 8, button 8).
   Regression: `text_leaf_defaults_to_no_padding`.
2. **Line heights unenforced.** The ramp defined (size, lh) but only
   size reached parley; `lh` values were decorative. Fix: `lh` prop
   flows TextOpts → layout measure → scene → both renderers via
   `TextLayout::new(..., Option<lh>)` (`LineHeight::Absolute`).
   Legacy nodes omit it (font defaults). Regression:
   `declared_line_height_sizes_the_box`.
3. **Paint/layout wrap disagreement.** Flex shrink narrowed the
   progress heading below its measured width; paint then wrapped
   ("Development" + clipped "progress") what layout kept single-line.
   Fixes, all shared: text leaves `flex_shrink: 0` (a measured label
   is incompressible; spacers absorb pressure), pixel containment
   (`ceil(advance)` in measure — without it rect.w - inset lands a
   fraction short and parley breaks the line), and a render contract:
   scene records `lines` at final width, renderers never re-wrap
   single-line nodes (`wrap_w = INF`), overflow clips at the rect
   with Eye-visible ink overflow. Regression:
   `text_leaf_refuses_flex_shrink` + audit assert `lines == 1`.
4. **Flat section rhythm.** Card children shared gap 8 with inner
   groups — no proximity hierarchy. Fix: `CardOpts.gap` default XL
   (24); inner groups stay 4–16. Rule: section gaps exceed the
   largest inner gap (asserted in the composition audit).
5. **Metric tiles dispersed.** Consequence of (1): pad-12 text boxes
   pushed icon/value/label apart (tile h160 with 8px nominal gaps).
   After (1): tile content 20+8+32+8+16+pad 32 ≈ 116 — dense, value
   dominant, no page-specific numbers.

## What was NOT wrong

- Optical centering (022) held: badge/pill text centered.
- Progress bar geometry (h6), button sizing, tile equality, wrap
  behavior, stale discipline — all green before and after.
- Badge anchor: centered on the header row; no forced first-letter
  alignment with the title (different widths, right-anchored).

## Composition audit (§4, proportionate)

`composition_audit_reads_as_groups` in dashboard tests checks real
geometry: title/sub gap ≤ 12px, prog heading single-line + trimmed,
equal tiles in one row, zero clipped nodes, no sibling overlaps
(ghosts exempt), card gap > inner gaps. Medium width (560px) keeps
one row; narrow (240px) stacks via `min_w` 140 readability floor
(new `min_w` layout prop; basis 0 shares equally above it).
Contrast guardrails in hamesh-theme (WCAG ratios, AA for body text).
No aesthetic scores, no beauty formulas — the agent supplies
judgement, Hamesh supplies measurements.
