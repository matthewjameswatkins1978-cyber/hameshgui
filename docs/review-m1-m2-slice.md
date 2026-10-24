# Review notes: `hamesh/m1-m2-slice` → merge (for Lucy)

All pushed, CI green on every one (fmt, clippy
`-D warnings`, workspace tests, headless demo/m3/fault drills).
Local-only evidence (window/GPU/drills/snapshots/IME) was produced on
the dev machine per run and is referenced from the decision docs, not
committed — `target/` stays gitignored.

## What this slice delivers

1. **M1–M3** (commits up to `ebb0153`): renderer, native window, GPU
   path, parley text, chat foundations, external CLI protocol,
   fault drills + CI.
2. **M4 live loop** (`77ff3d5`, `90069a0`, `2686971`): composer submit
   → background pi worker → streamed feed; token-asserted smoke;
   Esc-abort vs Esc-quit.
3. **Bridges + drills** (`701e9c9`, `9cff8b0`, `83f1d83`, `88cad59`,
   `647492f9a9`): CLI drill and MCP drill, `isError` contract.
4. **Correctness fixes the drills forced** (`41998d0`, `a1068473c5`):
   per-connection inspector threads (a parked `wait` starved the
   server); race step proving rev discipline under concurrency.
5. **Text** (`863722c`, `386f19d`): grapheme-correct editing + fixture
   tests; IME preedit/commit model with live verification queued.
6. **Docs** (`015`–`019`, README rewrite): every consequential choice
   recorded with evidence and explicit non-goals.

## Suggested review focus

- `crates/hamesh-window/src/main.rs` — M4 link, abort rule, IME arm,
  wait parking: all thin delegation to tested cores; confirm nothing
  bypasses `m3_keys_apply` / `apply_op`.
- `crates/hamesh-window/src/inspect_srv.rs` — per-connection threads:
  confirm the UI thread is still the sole serializer (rev checks +
  apply in `apply_op`, single event-queue consumer).
- `apps/hamesh-mcp` — the FIFO lock + `isError` mapping; new deps
  (`rmcp 3`, `tokio`, `schemars`) are all permissive-licensed.
- `Cargo.lock` churn is from those dep additions only.

## Known non-blockers (do not hold the merge)

- IME needs a real IME + human minute (019); model + tests are in.
- GPU/CPU pixel divergence on the M3 scene (4–6%, rounded bubbles +
  glyph AA) is accepted: structure agrees, tolerance model owns it.
- `target/` evidence (PNGs, traces, drill logs) is local-only by
  design; decision docs cite exact filenames + rev chains.
- `docs/review-m1-m2-slice.md` (this file): keep as slice history or
  delete at merge — your call.
