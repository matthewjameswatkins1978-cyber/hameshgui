# 011 — M3 foundations: composer, virtualized feed, keyboard focus

Date: 2026-10-21. Status: accepted (headless drills + live window).
Sources: `crates/hamesh-widgets`, `apps/demo/src/m3.rs`,
`target/hamesh-m3.png`, `target/hamesh-window-gpu.png` (all observed).

## What shipped

- **`hamesh-widgets`**: `Composer` (insert/backspace/delete/arrows/home/end,
  caret measured by laying out the pre-caret prefix — exact under shaping),
  `Feed` (cached per-message heights, pure-geometry `visible()` window,
  scroll clamp), `Focus` (deterministic Tab order from the component tree),
  and `handle_key` — ONE keyboard core shared by the headless drills and
  the real winit window.
- **Hand**: `focus_human` (click-to-focus via hit-test; command ownership
  NOT required — a text field takes focus without owning a command),
  `key_human`, `type_human` (focus + per-char keys),
  `set_composer_text_direct` (labeled DirectOp contrast).
- **Eye**: caret is scene truth (`SceneNode.caret_x` + `focused`), drawn
  by both renderers, covered by a pixel-count unit test.
- **Demos**: `hamesh-m3` headless drills (type/submit, covered-composer
  block, Tab navigation, 200-msg virtualization to <40 components);
  `--m3` window mode with real keys, wheel scroll, click-to-focus, and a
  scripted smoke run (focus → type → submit → scroll).

## Two real bugs caught by rendering, not reasoning

1. **Names painted as text (again,Scene-side).** Already fixed in 010 for
   containers; the M3 feed initially rendered message NAMES. Same rule
   held: only explicit `text` props render.
2. **Flex-shrink crushed the fixed header/composer.** The feed hugged ALL
   children while the window stayed fixed → Taffy shrank the 48px header
   and 40px composer to fit. Fix: the feed panel is SIZED to the
   available space (`viewport = height - 152`, feed h = viewport + 24)
   and only the visible window becomes components. `Role::List` joined
   Window/Panel as a layout container; `Widgets.feed_viewport` carries
   the contract to scroll clamps and the visible window. The Eye (rects
   h=48/472/40 exactly) confirmed the fix.

## Tolerance note

M3 scene diverges 4.3% CPU-vs-GPU (rounded message bubbles on GPU vs
square fills on CPU + glyph rasterizer AA differences). Structure, text
content, caret and layout all agree; the delta is the known renderer
difference the tolerance model exists to bound, not a semantic break.
Per-corner/per-glyph diff attribution is open work, not a blocker.
