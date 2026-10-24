# 021 — Optical centering: research (no implementation yet)

Date: 2026-10-22. Prompt: Matthew's catch that pill/button text sits
high and first letters don't share a line, plus his CorelDRAW point:
Hamesh must SEE alignment itself, not borrow human eyes.

## Sources consulted

- **MDN `text-box-trim` + `text-box-edge` (fetched, verified):** the
  web platform's answer. Trim the line box to content edges; e.g.
  `text-box-edge: cap alphabetic` trims top to cap height, bottom to
  the baseline. Confirms the principle: center/align on CONTENT
  edges (cap height, baseline, ink), never the raw line box.
- **Flutter `TextHeightBehavior` (fetched, verified):** controls
  whether line height applies to first ascent / last descent and
  how leading distributes. Same family: choose which font metrics
  participate in the box.
- **Figma vertical trim (concept, well-known):** "cap height to
  baseline" product behavior. Same principle again.
- **On-disk APIs (verified in vendor sources, not inferred):**
  - parley 0.12 `LineMetrics { line_height, baseline, ... }` —
    baseline available per line; NO ascent/descent/cap split.
  - skrifa (in lockfile) `Metrics { ascent, descent, leading,
    cap_height?, x_height?, ... }` + `GlyphMetrics::bounds(id)` —
    REAL font metrics and per-glyph bounds, no raster needed.
  - hamesh-text already resolves fonts through swash; same
    `FontRef` pattern serves skrifa.
- **Vector-editor model (CorelDRAW/Inkscape concept):** selection =
  OBJECT bounds (ink + stroke), alignment operates on those, layout
  boxes are separate. Our mapping: node rect = layout box,
  ink box = object bounds. Agents compare ink boxes for alignment —
  same model, no new inference machinery.

## Findings

1. Geometric line-box centering is provably wrong for caps-only or
   no-descender text: up to ~0.25em of dead descender space sits
   below the ink. At 13px that's ~3px — visible, and Matthew saw it.
2. No heuristic (descender tables, magic fractions) is needed: exact
   ink boxes are computable from font tables + shaped positions.
3. Baselines are the cross-node invisible lines: parley exposes them
   per line. Same-size text shares baseline offsets by construction.
4. Placement and boxes must stay decoupled: boxes stable (layout,
   rev discipline, drills unaffected), glyph placement optical.
   (CSS trims boxes; we deliberately trim placement instead —
   stability is a Hamesh invariant.)

## Proposed design (awaiting go-ahead)

- `hamesh-text::ink_box(text, max_w, size) -> Option<InkBox>`:
  shaped ink bounds + first baseline, layout space. Pure function,
  small bounded cache (dashboard text is static → steady-state free).
- Scene carries `ink` + `baseline` (node-local). Renderers place
  glyphs from ink (vertical optical center), caret stays
  line-anchored. Empty/whitespace → existing fallback.
- Eye `detail` gains typed `ink` + `baseline` fields: agents check
  centering and alignment from map data, CorelDRAW-style.
- Proofs, not eyeballs: ink-centroid pixel test, headless
  pill/button centering asserts, drill asserts ink centering live,
  perf A/B on smoke frames before/after.
- Builder part (origins): badge pad 4 -> 8 so pill and button text
  share the same 16px origin; absolute first-letter x still differs
  by design (right-anchored different widths — will state this
  plainly rather than force equal widths).
