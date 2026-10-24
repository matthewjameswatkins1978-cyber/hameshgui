# 013 — External Eye/Hand protocol (separate-process CLI first)

Date: 2026-10-21. Status: proven against the live window. Source:
`crates/hamesh-window/src/inspect_srv.rs`, `apps/hamesh-cli`,
`target/hamesh-ext-caret3.png` (caret drawn on external click+snapshot).

## Design (answers Lucy's review §1, §6)

- **One routing implementation.** The inspector server thread NEVER
  touches application state: it forwards each op as a winit user event
  and the UI thread applies the SAME handlers as real input
  (`apply_press` for mouse/clicks, `handle_key` core for keys,
  `type_human` for typing). The real mouse path was converged onto
  `apply_press` too — no parallel semantics.
- **Local CLI first, MCP later.** `hamesh [--port --token] <op>`: map,
  detail, click (id or x,y), key, type, snapshot, wait. JSONL over
  127.0.0.1 only, per-run token in `target/hamesh-inspect.json`,
  dev-only `--inspect` flag. No network exposure, no credentials on
  the wire beyond the loopback token.
- **Stale-safe.** Every response carries app_rev/scene_rev; mutating ops
  accept `expect_rev` and refuse with STALE (exit 2) instead of acting
  on a dead layout. `wait` blocks nothing: the UI thread parks the
  responder and answers when the revision actually moves (or timeout).
- **Permissions are explicit:** read ops (map/detail/snapshot) vs acting
  ops (click/key/type), each labeled HumanEquivalent in results — the
  DirectOp path stays in-process-only and can never masquerade as GUI
  evidence through this interface.

## Proven end-to-end from separate OS processes

map → detail → click composer (focus) → type → Enter (submit) →
snapshot PNG → stale-click refusal, every step a separate `hamesh`
invocation against the running window. The submitted message appeared
in the feed; the focused empty composer shows its caret.

## First independent debugging exercise (bonus, same machinery)

The empty-composer caret was missing from both renderers. Found via the
external loop, not source reading: external `map` gave exact geometry,
`detail` gave caret_x/focused state (correct!), `snapshot` showed no
caret pixels — state right, pixels wrong, so the bug was in the
*rendering path*, twice: once an early-return on empty text (CPU call
site), once a caret-inside-text-check (GPU). Fixed, rebuilt, re-verified
through the same external calls. No human described the screen.

## Open (deliberately)

- MCP adapter over this exact protocol (no new semantics).
- A broken-control fixture + agent-led repair drill as a standing
  regression (the caret hunt was ad hoc).
- `wait` is poll-per-frame; fine for dev, replace with revision
  broadcast if the subscriber count ever grows.
- OS access controls (named-pipe ACLs / socket permissions) for any
  non-dev use. Refused: unauthenticated listeners, 0.0.0.0 binds.
