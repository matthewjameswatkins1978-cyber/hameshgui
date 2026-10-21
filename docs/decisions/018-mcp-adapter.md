# 018 — MCP adapter: every MCP client becomes a Hand

Status: accepted (proven live 2026-10-21: full MCP session drove a real
submission through the bridge).

## Context

`hamesh-cli` proved an outside operator works, but it is one bespoke
binary. The mission thesis is that agents operate the app — so the
inspector protocol should meet agents where they already are: MCP. Any
MCP-capable client (Claude, Cursor, Pi itself) becomes a Hand with no new
Hamesh-specific tooling.

## Decision

- New binary `apps/hamesh-mcp`: an MCP server over stdio (official
  `rmcp 3.6`, Apache-2.0 verified via `cargo info`) with 7 tools mapping
  1:1 onto the inspector protocol (`hamesh_map/detail/click/key/type/
  snapshot/wait`). The bridge translates, never invents: the tool reply
  IS the inspector reply JSON, rev stamps and all.
- Auth per call: explicit `port`/`token` args win, then `HAMESH_PORT`/
  `HAMESH_TOKEN`, then the discovery file — so a window restart (new
  port/token) needs no server restart.
- **The bridge owns ordering.** MCP clients fire calls concurrently, but
  the Eye is sequential (revs!). Live verification caught this red-handed:
  a batched Enter executed before its type and submitted empty. One
  global FIFO mutex around the inspector round-trip restores arrival
  order; a `wait` legitimately holds it.
- STALE is in-band (`code` field), with the re-map-and-retry rule in each
  mutating tool's description — same discipline as the CLI, no new
  semantics. No `isError` translation in this slice.
- Local-only, like the rest of the Eye/Hand path. Never expose.
- Deps: `rmcp 3` (+`transport-io`), `tokio 1` (rt/macros/io-std),
  `schemars 1`, workspace serde/serde_json. All permissive licenses.

## Evidence

- Live scripted session against a headed window: `initialize` (protocol
  2025-06-18) → `tools/list` (7 tools, valid schemas) → `hamesh_map`
  (rev 8) → `hamesh_type` (rev 9, 13/13 chars) → `hamesh_key Enter` with
  `expect_rev` → `submitted "hello via mcp"` → `hamesh_snapshot` → clean
  shutdown. Window trace independently shows
  `m3.key: -> submitted "hello via mcp"`.
- Gates unchanged: `cargo fmt --check`, `clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace` all green.

## Consequences

- Any MCP client can now Eye (map/detail/snapshot/wait) and Hand
  (click/key/type) a live Hamesh app, with the same stale discipline the
  drill regression-covers for the CLI.
- Open: `isError` mapping for STALE/DENIED, session-scoped discovery,
  and a standing MCP drill (today the drill covers the CLI; the MCP
  session script lives in `target/` as scratch, not yet a binary).
