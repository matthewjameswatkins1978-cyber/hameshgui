# 020 — Hamesh Blocks architecture

Date: 2026-10-22. Mission: Hamesh Blocks (visual component system).
Branch: `hamesh/blocks-b1`. Version: 0.2.0 (minor increment for the
new capability; no release/tag without authorisation).

## Reading the mission

The mission asks for three layers (primitives, polished components,
composites) on ONE canonical model, a design system first, then the
Agent X dashboard, then AI contracts, then generalisation (B1–B4).
Quality over count. Same runtime, layout engine, event system —
a composite is an arrangement of lower components, not new machinery.

## What the foundation already gives us (do not rewrite)

- `hamesh-core`: retained tree, stable ids, revisions, props, commands.
- `hamesh-layout`: Taffy flex row/column, `w/h/flex_grow/layout`
  props, overlay escape hatch, hit-testing, clipping truth.
- `hamesh-scene`: per-node color/text/caret/focus; sorted paint order.
- `hamesh-render` + `hamesh-window/gpu.rs`: CPU tiny-skia + wgpu/vello
  of the same Scene, tolerance harness, PNG evidence.
- `hamesh-text`: parley layout + swash raster on both renderers.
- Eye/Hand/Memory + CLI/MCP + drills + CI: the AI-first machinery
  every Block inherits for free.

## What is genuinely missing (the work)

1. **Design tokens.** No semantic color/type/spacing/radius system;
   `role_color()` hardcodes one dark theme inside scene code. New:
   a token table (dark theme first) with semantic names; components
   reference tokens, never raw values.
2. **Per-node style.** Layout hardcodes padding 12 / gap 8; render
   draws flat rects only (no radius, border, shadow); text is one
   size/color. New: style props (`pad`, `gap`, `radius`, `border`,
   `surface` token, `text` token/size) flowing core → layout →
   scene → both renderers.
3. **Layout breadth.** Flex row/column exists; no grid, wrap, or
   spacer semantics; gap/padding not configurable. New: grid via
   Taffy (already a dependency), wrap/flow, spacer (flex grow with
   no visuals). No new layout engine.

   Update 2026-10-22: Taffy grid verified available (0.15 default
   features include `grid`, `fr` in prelude) but DELIBERATELY not
   wired — flex row + `wrap` + grow produces the responsive equal
   tiles the dashboard needs with less API surface. True grid
   placement waits for B4 demand. `pad` (0 = flush) and `gap`
   overrides land in `child_style`; wrap via `FlexWrap::Wrap`.
4. **Icons.** No vector graphics at all. New: stroked polyline icons
   from Lucide path data (ISC licence, vendored per-icon with
   attribution in `assets/icons/`), drawn in both renderers.
   Lucide's 24×24 stroke grammar (2px, round caps) maps cleanly.
5. **Component builders.** No ergonomic composition: every screen
   hand-assembles Components. New: `hamesh-blocks` crate with
   builder functions (Card, Row, Column, Grid, Title/Body/Caption,
   Badge, Metric, Progress, Button, Icon, Divider, Spacer) that
   emit canonical Components + props. Rust API first; a validated
   structured (serde/JSON) form resolving to the same builders.
6. **Block metadata for the Eye.** `detail` already exposes props,
   rect, state. New: `block` prop (component type), typed value
   props (`value/min/max` for progress, `variant` for badge/button),
   and a machine-readable catalogue (`docs/blocks-catalogue.md` +
   JSON) so agents discover without reading source.

## Research posture (no redistribution)

Studied from knowledge of the public systems, using concepts only:

- **shadcn/ui**: sensible defaults + variants (primary/secondary/
  outline/ghost, sizes) composed per component; copy the
  *variants philosophy*, not code.
- **Radix**: accessible interaction patterns, focus management,
  keyboard behaviour per WAI-ARIA APG; adapt to native + AccessKit.
- **Base UI**: separate behaviour from presentation — our version:
  canonical components carry semantics; tokens carry visuals.
- **React Aria**: pointer/keyboard/i18n behaviour notes; grapheme
  work (017) already aligns with its text principles.
- **OpenUI**: constrained vocabulary + declarative JSON composition
  for AI generation — direct model for our structured form.
- **Lucide**: ISC-licensed 24×24 stroke icons; vendor path data
  per icon with `LICENSE` attribution.
- **Fluent 2**: semantic color roles, spacing ramp, type ramp,
  corner radius scale — model for our token table.
- **WAI-ARIA APG**: button/progressbar/meter interaction patterns
  as behavioural references.

Nothing from GPT-6/ChatGPT internals (not open source): only the
*visible compositional vocabulary* (Box/Row/Card/Badge/...) which
is a public concept, reimplemented natively.

## Milestones (shipped incrementally on this branch)

- **B1**: tokens + style props through layout/scene/renderers +
  layout breadth (grid/wrap/spacer/pad/gap) + icon pipeline.
  Headless + PNG proof; drills still green.
- **B2**: block builders + Agent X dashboard as a real composition;
  screenshot iteration until genuinely refined.
- **B3**: Eye metadata + machine catalogue + validated structured
  form + agent modification path (static validated first).
- **B4**: Pi worker panel + settings panel from the same blocks;
  API refinement; catalogue + "how to write a block" docs.

## Non-goals for this mission

New OS/window-manager work, React/HTML/CSS under Hamesh, the full
advanced catalogue (menus/popovers/dialogs/tables/charts come after
the dashboard proves the system), a theming megaproject before the
first dark theme works, parallel styling semantics.

## Addendum 2026-10-22 — Lucy's quality bar (gallery, variety, judgement)

Lucy adds three principles, incorporated without interrupting B1:

1. **Living gallery.** Every Block gets a home in a real Hamesh
   application showing sizes, states, configurations, compositions
   — inspectable and exercisable through Eye/Hand. The gallery
   develops alongside the components (first cut in B4 next to the
   Pi/settings panels) and doubles as visual lab + regression
   suite + machine catalogue frontend.
2. **Variety, not uniformity.** Consistent quality, not identical
   appearance. The token table is designed for this: a second
   theme (light or warm) in B4 proves primitives stay excellent
   across styles. No theming megaproject before the dark theme
   convinces.
3. **Visual judgement in acceptance.** A component is not finished
   when tests pass: screenshot it beside other components, check
   type/balance/contrast/states, get independent eyes (agent or
   human) on the PNGs. Measurable defects and aesthetic calls are
   recorded separately.

## Addendum 2026-10-22 — layout lessons from the first composition

Composing the dashboard exposed three layout-truth gaps, all fixed
in the shared core (not the dashboard):

1. **Text measures.** Text/Status/Button leaves carry their string
   as Taffy node context and are measured with parley
   (`compute_layout_with_measure`). Labels, badges and buttons now
   size to content; explicit `h` still wins; M3 feed rows (ListItem
   + explicit h) and M1 screens are unaffected.
2. **CSS-correct grow.** The legacy blanket `flex_grow=1` on every
   Panel made siblings fight over free space. Default is now 0;
   structural fill opts in (`Role::List` keeps 1, tiles opt in).
3. **Cross-axis align.** New `align: start|center|end` prop
   (Taffy `AlignSelf`) so pills don't stretch tall in headers.
4. **Progress fill is opt-in.** `fill_frac` resolves only when
   `block == "Progress"` — Metric tiles reuse `value` as Eye
   metadata without painting bars.
5. **Painted leaves take pad 0.** An empty h=6 leaf with the
   default pad 12 lays out 24 tall; progress/icon builders zero
   it. Badge/button pills keep their padding deliberately.
6. **Equal tiles need basis zero.** `basis` prop wired
   (flex base; the `> 0` parse filter would have eaten 0, same
   class as the old pad bug). Metric tiles share rows equally
   modulo whole-pixel rounding; rows shrink to min-content,
   then wrap.
