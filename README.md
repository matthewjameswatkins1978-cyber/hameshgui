# Hamesh — the Eye and the Hand

An AI-first native GUI framework in Rust. First platform: Windows.
First real application: Agent X / Pi GUI.

Guiding concept: **See. Understand. Act. Prove it worked.**

Hamesh makes inspection and interaction native capabilities of the framework:
the Eye (machine-readable layout, semantics, screenshots) and the Hand
(human-equivalent event injection plus labeled direct operations) with
the Memory (bounded diagnostic traces and replay evidence). Any agent —
via the CLI or via MCP — can operate a live Hamesh app from a separate
process and prove what it did.

## Status (v0.1.0, `hamesh/m1-m2-slice`)

- [x] M1 headless + native window + GPU: Taffy layout → scene → CPU
      (tiny-skia) or wgpu 30 + vello 0.11, with loss recovery and CPU
      fallback. `--smoke-test=N` proves the window non-interactively
      (decisions 001, 002, 004, 006).
- [x] Real text on both renderers: parley layout + swash CPU blit and
      vello `draw_glyphs`; grapheme-correct composer editing (ZWJ, skin
      tones, flags, combining marks via `unicode-segmentation`) and an
      IME preedit/commit model with cursor-area tracking (decisions 007,
      009, 010, 015/017/019).
- [x] M2 Eye/Hand: stable IDs, compact map + detail, hit-testing,
      human-equivalent vs labeled-direct ops, per-thread inspector
      server (a parked `wait` never starves other ops — proven by the
      drill), stale-revision discipline with exact-winner races
      (decisions 013, 016).
- [x] M3 chat foundations: composer + virtualized feed + focus through
      one shared core; `--m3` window (decisions 011).
- [x] M4 live Pi loop: composer submit streams through a background
      `pi --mode rpc` worker into the feed; machine-checked smoke
      (`HAMESH-SMOKE-OK` token assertion); Esc aborts a live run, quits
      when idle (decisions 012, 015).
- [x] Two outside-operator bridges, both drill-regression-covered:
      `hamesh` CLI and MCP server (`hamesh-mcp`, rmcp 3.6, `isError`
      on STALE/DENIED/LOCAL) — map/detail/click/key/type/snapshot/wait
      with identical semantics (decisions 013, 016, 018).
- [x] Standing drills green: 3 headless fault drills in CI; CLI drill
      (stale-inject → repair → submit → wait → race) and MCP drill
      (handshake → ordered submit → stale repair) run locally against
      a real window (decisions 014, 016, 018).
- [ ] Accessibility bridge (AccessKit): partial, queued.
- [ ] IME live verification with a real IME: queued human dogfood,
      non-blocking (decision 019).

## Workspace

- `crates/hamesh-core` — canonical component model, identity, revisions.
- `crates/hamesh-layout` — Taffy layout + visibility/clipping truth + hit-test.
- `crates/hamesh-scene` — resolved visual representation.
- `crates/hamesh-render` — CPU renderer + PNG capture + CPU/GPU tolerance model.
- `crates/hamesh-text` — parley shaping/glue + caret measurement.
- `crates/hamesh-inspect` — compact map + component detail (the Eye).
- `crates/hamesh-automation` — human-equivalent vs direct-op injection (the Hand).
- `crates/hamesh-diag` — bounded diagnostic trace ring (the Memory).
- `crates/hamesh-widgets` — M3 composer/feed/focus + shared key core.
- `crates/hamesh-window` — winit window, GPU state, inspector server,
  Pi worker link (`hamesh-window-demo` binary).
- `crates/hamesh-pi` — clean-room `pi --mode rpc` JSONL adapter.
- `apps/demo` — headless M1/M2 (`hamesh-demo`) and M3 (`hamesh-m3`) drills.
- `apps/hamesh-faults` — standing fault drills (diagnose/repair/verify).
- `apps/hamesh-cli` — external Eye/Hand CLI (`hamesh` binary).
- `apps/hamesh-drill` — standing CLI regression drill (local-only).
- `apps/hamesh-mcp` — MCP server bridge over the inspector protocol.
- `apps/hamesh-mcp-drill` — standing MCP regression drill (local-only).

## Build, test, run

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p hamesh-demo            # M1/M2 headless drills
cargo run -p hamesh-demo --bin hamesh-m3   # M3 headless drills
cargo run -p hamesh-faults          # standing fault drills
```

Window (opens a real window; `--smoke-test=N` exits after N seconds
leaving map + PNG + trace evidence):

```powershell
cargo run -p hamesh-window -- --m3 [--gpu] [--inspect] [--pi] --smoke-test=60
```

Local-only drills (need a desktop; never in CI):

```powershell
cargo run -p hamesh-drill       # CLI: stale-inject -> repair -> wait -> race
cargo run -p hamesh-mcp-drill   # MCP: handshake -> ordered submit -> repair
```

MCP clients: run `hamesh-mcp` from the workspace root while the app runs
with `--inspect` (or set `HAMESH_PORT`/`HAMESH_TOKEN`). Seven tools,
`hamesh_map` first; mutating calls take `expect_rev`, retry on STALE.

Evidence outputs (gitignored): `target/hamesh-*.png`,
`target/hamesh-*.json`, `target/*drill*.log`.

## CI vs local evidence

| Check | Where | Why |
|---|---|---|
| fmt, clippy `-D warnings`, all tests, headless drills | GitHub Actions (every push) | Deterministic, no desktop needed |
| Window/GPU/smoke, CLI + MCP drills, snapshots, IME | Local runs on a real machine | CI must never fake GPU/desktop acceptance |

## Docs

- `docs/architecture.md` — core model and crate responsibilities.
- `docs/inspector-protocol.md` — Eye/Hand/Memory schemas and drill catalog.
- `docs/decisions/` — decision records 001–019 + review notes.
- `docs/review-m1-m2-slice.md` — merge checklist for this slice.

## Non-goals for this slice

New OS/window manager, custom graphics drivers, new language, browser
reimplementation, CSS replacement, all-platforms-day-one, widget
gallery, plugin marketplace, Pi rewrite, Agent X rewrite,
multi-agent management. Queued (not blocking): IME live dogfood,
session-scoped MCP discovery, selection/clipboard/undo, per-region
tolerance masks.

## License

MIT OR Apache-2.0.
