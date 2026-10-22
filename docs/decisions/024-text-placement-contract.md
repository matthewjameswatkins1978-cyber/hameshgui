# 024 — Resolved text placement contract (H1/H2)

Date: 2026-10-22. Mission: horizontal text alignment.

## The missing capability

Both renderers anchored glyphs at a fixed left inset (`rect.x+8`);
no shared Start/Center/End semantics existed, and the Eye reported
pre-shift geometry while paint applied an optical shift privately.
Badge/Button text was visibly left-biased (right gap 25px vs left 8px);
the Eye could not describe final placement.

## The contract

One resolution point — `hamesh-scene::build`, per text node:

- **Content box**: padding-declared usable area, node-local.
- **`halign`**: `start|center|end` prop (default start; unknown→start;
  multiline forces start). Builders declare it; Badge/Button center.
- **Direction**: parley's own BiDi analysis flips Start↔End for RTL
  paragraphs; Center is direction-independent. Reported `halign`
  stays as declared (intent); the origin is the physical result.
- **Advance**: shaped advance width (never the raster silhouette,
  which would jump with content).
- **Origin**: `align_origin_x` — effective inset `E = max(pad,
  TEXT_INSET_X)` (bearing protection, preserves historical Start
  rendering exactly), content width `w − 2E`, Center = E +
  (content − advance)/2.
- **Vertical**: existing font-relative trim centers on the box.
- **Final geometry**: ink/trim/baseline shifted by the optical
  offset and stored final; `origin`, `content`, `halign`,
  `advance`, `lines` stored on the node.

CPU, GPU and Eye consume the node verbatim — no renderer-local
positioning math remains (the `+8` constants and shift blocks are
gone). Caret stays line-anchored (separate path, unchanged).

## Padding reconciliation (§4 Q3)

- Requested padding: the `pad` prop (text leaves default 0, containers 12).
- Content rectangle: rect inset by `pad` on all sides.
- Intrinsic measurement: advance + 2·`max(pad, INSET)`; the inset half
  is bearing protection, symmetric by construction.
- Glyph origin: resolved `origin` (see above), never a hardcoded offset.
- Clip region: the component rect (unchanged).

## Compatibility

- Start rendering is pixel-identical to the old fixed origin
  (`max(pad, 8)` with pad < 8 = old `+8`; tests prove it).
- Pad ≥ 8 centered controls gain symmetric insets (the fix).
- Multiline, caret, IME, hit-testing, Pi loop: untouched (full suite green).
- Frame cache + shared font contexts + raster cache: steady-state
  CPU cost collapsed (~920ms → ~60ms/frame headless); GPU path unchanged.
