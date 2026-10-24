# 022 — Permanent typography instruments (font-relative optical centering)

Date: 2026-10-22. Approves and records Lucy's requirements + Matthew's
CorelDRAW principle: AI agents get professional design instruments —
machine-readable geometry — instead of borrowing human eyes.

## Canonical text geometry (one model, distinguished parts)

Per text node, in node-local space, the Eye (`text_geom`) reports:

- `rect` (layout box) — where the node sits; hit target, clipping.
- line box — parley `height()`; caret geometry lives here.
- `baseline` — first-line alphabetic baseline; shared baselines =
  aligned text across nodes (the invisible lines).
- `metrics: [ascent, descent, cap, line_height]` — font facts:
  ascent = baseline, descent = line − baseline, cap = ink top of
  "H" at this size (cached per size), line_height = parley height.
- `trim: [x0,y0,x1,y1]` — font-relative box: cap top to alphabetic
  baseline, x spanning the advance. Same for any string in this
  font/size. Renderers center THIS.
- `ink: [x0,y0,x1,y1]` — actual hinted raster union, bearings
  included. Object bounds for overflow/alignment checks; NEVER an
  alignment input (it jumps with content).

Sources are real, not guessed: parley 0.12 `LineMetrics`
(baseline, line height) + swash raster union for ink + a shaped
"H" reference for cap. No new font dependency (skrifa already in
the tree; not needed yet).

## Centering without instability

Single-line text nodes place glyphs so the trim center — not the
line box, never the ink silhouette — sits on the node center
(CPU `blit_text` + GPU `draw_text` share scene trim; caret stays
line-anchored). Multiline/empty/icon-only fall back to line-box
(`trim: None`). Changing Connected→Disconnected or adding accents
cannot move the box: the trim derives from font+size only. Glyphs
may overflow the trim (descender tails, diacritics); renderers align
by it but clip only to the node rect — trimming never clips.

This mirrors CSS `text-box-trim: trim-both` + `text-box-edge: cap
alphabetic`, Flutter's ascent/descent controls, Figma vertical trim —
reimplemented natively, with Hamesh's invariant added: boxes stay
stable (layout, rev discipline, drills unaffected); only glyph
placement is optical.

## Eye evidence (live)

`detail dash-status` on the running dashboard returns `block: Badge`
plus `text_geom { baseline, ink, metrics, trim }` — an agent checks
centering (`trim` centered in `rect`), cross-node baselines, and
overflow (`ink` vs `rect`) from map data. Standing blocks drill
asserts click→binding→stale→snapshot→trace as before (DRILL-PASS
revs 37→38 after this change).

## Standing tests

- `trim_is_font_relative_not_content_relative`: Connected vs
  Disconnected vs Café share the trim; empty/wrapped opt out;
  trim height is cap inside the line box.
- `ink_box_is_ground_truth` (kept): ink is raster truth for
  overflow checks, not placement.
- Inspect test: buttons carry trim+baseline+metrics.
- Headless dashboard asserts (bar/button geometry, wrap) unchanged.
- Live: blocks drill DRILL-PASS; smoke PNG shows Connected centered
  with even air, Inspected button centered.

## Reuse and adjacent work

- Every Blocks text control inherits this (badge, button, metric
  value/label, headings) — no per-component fixes.
- Gallery/Specimen (B4): trim/baseline guides become the diagnostic
  overlay; same `text_geom` feeds it.
- MCP bridge: `text_geom` serializes through existing detail replies.
- CorelDRAW mapping made literal: node rect = layout box, ink =
  object bounds, baseline = guide, trim = alignment box.
- Future: per-script trim (CJK/Indic), x-height trim for lowercase
  labels, gallery overlay, AccessKit baseline exposure.

## Design note (origins)

Badge pad 4→8 so pill and button text share the 16px origin
(pad + inset); absolute first-letter x still differs by design —
right-anchored controls keep their own widths.
