# Hamesh architecture (Phase 1 slice, v0.1.0)

One canonical meaning for components, state, commands, input and behaviour.
Humans, agents, CLI tools, MCP clients and test harnesses operate on the
same system through the same handlers.

## Data flow

```
App (core) ──retained tree + revision + commands + state
  │ compute(viewport)
  ▼
LayoutOutput (layout) ── absolute rects + effectively_visible
  │                      + clipped_by_parent + z + hit_test()
  ▼
Scene (scene) ── paint-ordered nodes + resolved colors/text
  │               + scene_revision tied to app_revision
  ├──► Frame (render) ── tiny-skia pixmap / wgpu+vello + FrameRef + PNG
  │                       + Tolerance model for CPU/GPU comparison
  ├──► CompactMap / Detail (inspect) ── the Eye
  └──► key core / click (automation) ── the Hand (one routing:
         winit keyboard, inspector ops, smoke script all land in
         m3_keys_apply / apply_press)
         └──► Trace (diag) ── the Memory (bounded ring, JSON export)
```

Outside operators enter through `crates/hamesh-window`'s inspector
server (token-gated TCP, one thread per connection) or `apps/hamesh-mcp`
(MCP over stdio, FIFO-serialized onto the same protocol). A background
Pi worker (`crates/hamesh-pi`, JSONL over `pi --mode rpc` stdio) streams
model output into the feed without ever blocking the UI thread.

No subsystem duplicates application truth. Layout owns geometry,
scene owns resolved visuals, core owns identity/state/commands.

## Key invariants

1. **Stable identity.** `ComponentId` is never reused within an app history;
   revisions increase monotonically (`App::bump`).
2. **Taffy locations are parent-relative.** `compute()` accumulates absolute
   offsets through ancestors; root is pinned to the viewport size.
3. **Containers hug, leaves fix.** Window/Panel default to auto size with
   `flex_grow = 1`; leaf widgets default to 32px height unless `h` is set.
4. **Existence ≠ operability.** The Eye reports `effectively_visible`,
   `clipped_by_parent`, `enabled`, z-order and hit-test winner separately.
5. **Mechanism is always labeled.** `HumanEquivalent` traverses hit-test +
   focus routing; `DirectOp` bypasses it and must never masquerade as a GUI
   test. Every `AutomationResult` records mechanism + revisions before/after.
6. **Frames are revision-bound.** `FrameRef { scene_revision, app_revision }`
   ties every screenshot to the exact state that produced it.
7. **Memory is bounded.** `Trace::new(capacity)` evicts oldest first;
   minimum capacity 16; export is JSON.
8. **One routing.** Physical keyboard, inspector KeyMsg, smoke script and
   tests all converge on `m3_keys_apply`; clicks converge on
   `apply_press`. Trace shape is identical regardless of driver.
9. **Stale-safe revisions.** Mutating inspector ops take `expect_rev`;
   mismatch ⇒ rejected unchanged (`STALE`). Concurrent submits on one
   rev resolve to exactly one winner — the UI thread is the serializer.
10. **Grapheme-correct editing.** Deletions and arrow moves snap to UAX #29
    boundaries; the caret stays char-indexed; IME preedit is display-only
    until Commit (or submit, which commits first).
11. **Errors are structural.** Inspector failures carry `code`; the MCP
    bridge additionally raises `isError`. Content stays parseable either
    way.

## Fault/repair catalog (standing drills, all green)

Headless (`cargo run -p hamesh-faults`, in CI): covered-submit,
clipped-composer, dead-submit — diagnose, repair, verify.

Live CLI drill (local-only): stale-inject → repair → submit → wait
(immediate/timeout/wake) → race (exactly one winner) → click-focus,
witnessed by the window's own trace.

Live MCP drill (local-only): handshake → ordered submit → in-band
stale repair → `isError` both ways.

## What is live

- Native winit 0.30 window: softbuffer CPU + wgpu 30/vello 0.11 GPU
  (`--gpu`), loss recovery + CPU fallback, `--smoke-test=N`.
- Parley text on both renderers; grapheme editing; IME preedit/commit
  model with cursor-area tracking (live-IME dogfood queued).
- M3 chat (composer + virtualized feed + focus), M4 live Pi loop with
  token-asserted smoke and Esc-abort.
- External CLI + MCP bridges, per-connection inspector server,
  wait parking, stale discipline — all drill-covered.
- GitHub CI gates (fmt/clippy/tests/headless) green on every push;
  GPU/desktop evidence stays local by design.

## What is NOT yet built

- IME verification with a real IME (human dogfood, non-blocking).
- Session-scoped MCP discovery; MCP `isError` already structural.
- Selection, clipboard, undo; multiline editing; full text editor.
- AccessKit bridge beyond partial; drag-and-drop.
- Deterministic replay with controllable clocks.
- Per-region tolerance masks for the GPU/CPU comparison.
- And, as always for this slice: no new OS, drivers, language,
  browser, CSS replacement, all-platforms-day-one, widget gallery,
  plugin marketplace, Pi/Agent-X rewrites, or multi-agent management.

## Decisions

`docs/decisions/001`–`019` record every consequential choice with
evidence. `docs/review-m1-m2-slice.md` is the merge checklist.
