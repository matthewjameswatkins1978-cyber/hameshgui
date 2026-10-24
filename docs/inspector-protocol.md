# Inspector protocol (Eye / Hand / Memory) — v0.1.0

Machine-readable, versioned, local-only. All schemas are `serde` JSON.
Two transports speak it: the `hamesh` CLI (TCP) and `hamesh-mcp`
(MCP tools over stdio, 1:1 mapping — decision 018).

## Transport

- TCP on `127.0.0.1`, ephemeral port, per-run token. Discovery:
  `target/hamesh-inspect.json` (`{"port": N, "token": "..."}`),
  written by the window when started with `--inspect`. Never bind
  beyond loopback; never use without the token.
- One JSON object per line. Request:
  `{"id": 1, "token": "...", "op": "<name>", "params": {...}}`.
  Reply (uniform envelope): `{"id": 1, "ok": true|false,
  "result"?: ..., "code"?: "...", "message"?: "...",
  "app_rev": N, "scene_rev": M}`.
- Connections are independent and concurrent (one server thread each —
  a parked `wait` never starves other ops). Per-connection request
  order is preserved. The UI thread stays the serializer: rev checks
  and state changes run there, in send order.

## Eye ops (read-only, never take `expect_rev`)

- `map` → `{"app_rev", "scene_rev", "viewport_w/h", "nodes[]"}`
  (`{id, role, name, rect, visible, clipped, enabled, z}`).
  Fetch first; details on demand. Message nodes carry `text` in
  `detail`, not in the map (virtualized feed: only the visible
  window exists as components).
- `detail {"id"}` → `{id, role, name, enabled, visible,
  effectively_visible, clipped_by_parent, rect?, z?, props,
  state_keys, diagnostics[]}` or `NOT_FOUND`. `props` carries live
  values (`text`, `caret_x`, `preedit`, `focus`, `count`).
  Diagnostics are observations, never invented causes.
  Text nodes also carry `text_geom`: the resolved placement
  contract — `origin` (glyph origin, node-local px), `content`
  (usable area inside padding), `halign` (as declared:
  `start|center|end`; effective Start/End flip for RTL paragraphs),
  `advance` (shaped advance width), `ink` (raster-ink bounds),
  `trim` (font-relative trim box), `metrics` (ascent/descent/…),
  `baseline`, `lines`. This is the final truth: both renderers
  and the Eye consume it verbatim.
  Recipe for centered text: `|origin.x + advance/2 −
  (content.left+content.right)/2| < 1.0px` ⇒ centered.
  Multiline nodes fall back to `start`; check `lines == 1` first.
- `snapshot {"path"?, "overlay"?}` → renders the current frame to PNG.
  Replies with the path + revs. With `overlay` set to a component
  id, paints that component's diagnostic guides into the capture
  ONLY (white bounds, gray content, teal trim, amber ink, blue
  baseline + content-centre cross) and returns the same resolved
  `rect` + `text_geom` the Eye reports — image and numbers cannot
  disagree. Normal rendering, state and hit-testing stay overlay-free.

## Hand ops (mutating, honor `expect_rev`)

Every mutating op accepts `expect_rev` (the `app_rev` from the last
reply). If the live rev differs, the op is rejected unchanged with
`code: STALE`. Re-map and retry with the fresh rev. Rejected ops
change nothing — this is also how races resolve: of two submits on
one rev, exactly one applies.

- `click_id {"id", expect_rev?}` / `click_at {"x", "y", expect_rev?}`
  → `{hit, acted, focus, mechanism: HumanEquivalent}`. Focus follows
  the hit (verified live: click composer ⇒ focus composer).
- `key {"key", expect_rev?}` — one of `Tab ShiftTab Enter Escape
  Char:x Backspace Delete Left Right Up Down Home End`. Routes through
  the same shared key core as the physical keyboard. `Enter` submits
  (empty composer ⇒ `submitted empty (ignored)`); `Escape` aborts a
  live Pi run, no-op otherwise.
- `type {"id", "text"}` — types full text into a component by id, no
  focus tricks. Grapheme-correct; CJK/emoji/ZWJ safe.

## Wait op (observe change without polling)

- `wait {"app_rev", "timeout_ms"}` — if rev already moved, replies at
  once `{waited: false}`. Else parks (non-blocking; the event loop
  keeps running) until a change lands (`{waited: true}` + new rev)
  or the deadline hits (`ok: false, code: TIMEOUT`).
- Covers: immediate return, timeout, and wake-on-change (all three
  machine-checked by the standing CLI drill).

## Reply codes

`ok: true` ⇒ applied (or read). `ok: false` + `code`:
`STALE` (re-map + retry), `DENIED` (bad token), `NOT_FOUND`,
`BAD_OP`, `TIMEOUT` (wait deadline / UI busy), `BUSY` (UI gone),
`LOCAL` (transport failure, MCP bridge only), `NO_M3` (keyboard
needs `--m3 mode`). The MCP bridge additionally raises `isError`
on every `ok: false` reply; the inspector JSON stays in content.

## CLI exit codes (`hamesh`)

`0` ok · `1` transport/protocol/timeout error · `2` stale/denied
(retry after re-inspecting) · `3` usage error. Flags
`--port/--token/--expect-rev` precede the op; discovery file fills
in what flags omit.

## MCP tools (`hamesh-mcp`)

`hamesh_map/detail/click/key/type/snapshot/wait` — same ops, same
params (plus optional `port`/`token`, else env then discovery),
same rev discipline, `isError` on failure. Run from the workspace
root so discovery resolves. Auth: per-call explicit args, then
`HAMESH_PORT`/`HAMESH_TOKEN`, then the discovery file.

## Evidence + drills

- Every reply and every trace record is rev-stamped; snapshots bind
  `app_rev`/`scene_rev` to the exact state that produced them.
- Standing CLI drill (`cargo run -p hamesh-drill`): stale-inject →
  repair → submit → wait (all three shapes) → race → click, then the
  window's own trace as independent witness. Local-only, ~85s.
- Standing MCP drill (`cargo run -p hamesh-mcp-drill`): handshake →
  ordered submit → in-band stale repair → `isError` both ways.
  Local-only, ~75s.
- Full op history and verdicts: `docs/decisions/013`, `016`, `018`.
