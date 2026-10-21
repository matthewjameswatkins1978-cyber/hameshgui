# Hamesh architecture (Phase 1 slice, v0.1.0)

One canonical meaning for components, state, commands, input and behaviour.
Humans, agents, CLI tools and test harnesses operate on the same system.

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
  ├──► Frame (render) ── tiny-skia pixmap + FrameRef + save_png()
  │                       + Tolerance model for CPU/GPU comparison
  ├──► CompactMap / Detail (inspect) ── the Eye
  └──► click_human / invoke_direct (automation) ── the Hand
         └──► Trace (diag) ── the Memory (bounded ring, JSON export)
```

No subsystem duplicates application truth. Layout owns geometry,
scene owns resolved visuals, core owns identity/state/commands.

## Key invariants

1. **Stable identity.** `ComponentId` is never reused within an app history;
   revisions increase monotonically (`App::bump`).
2. **Taffy locations are parent-relative.** `compute()` accumulates absolute
   offsets through ancestors; root is pinned to the viewport size
   (Taffy `AvailableSpace` alone does not expand an auto-sized root).
3. **Containers hug, leaves fix.** Window/Panel default to auto size with
   `flex_grow = 1`; leaf widgets default to 32px height unless `h` is set.
4. **Existence ≠ operability.** The Eye reports `effectively_visible`,
   `clipped_by_parent`, `enabled`, z-order and hit-test winner separately,
   so an agent can distinguish covered/clipped/disabled/invisible.
5. **Mechanism is always labeled.** `HumanEquivalent` traverses hit-test +
   focus routing; `DirectOp` bypasses it and must never masquerade as a GUI
   test. Every `AutomationResult` records mechanism + revisions before/after.
6. **Frames are revision-bound.** `FrameRef { scene_revision, app_revision }`
   ties every screenshot to the exact state that produced it.
7. **Memory is bounded.** `Trace::new(capacity)` evicts oldest first;
   minimum capacity 16; export is JSON.

## Fault-drill catalog (M2)

The demo proves the framework can catch, not just commit, these faults:

| # | Fault | Expected Eye/Hand behaviour |
|---|-------|------------------------------|
| 1 | Healthy button | human click succeeds, counter increments, trace records revisions |
| 2 | Disabled button | human click hits target but reports blocked, state unchanged |
| 3 | Transparent overlay | aimed-at-A hits veil B, reported as intercepted/occluded |
| 4 | Direct invocation | succeeds but labeled `DirectOp`, never GUI proof |
| 5 | Clipping | `detail()` reports rect-beyond-parent with reachability warning |

## What is NOT yet built

- Native window + GPU presentation (winit/wgpu/Vello): research running.
- Real text shaping/editing (Parley / COSMIC-Text): research running.
- AccessKit bridge, IME, clipboard, scrolling, drag-and-drop.
- Deterministic replay with controllable clocks.
- Networked inspector/MCP adapter (local-only by design so far).
