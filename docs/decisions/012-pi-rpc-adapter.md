# 012 — Pi RPC adapter pathfinder (M4 contract proven)

Date: 2026-10-21. Status: proven against live `pi`. Source: `crates/hamesh-pi`
(9 tests green) + `REAL-PI-OK model=muse-spark-1.3-contributor` from a real
`pi --mode rpc` child process.

## Protocol (read from AgentX.Core, verified against live CLI help + wire)

- Transport: child stdio, one JSON object per line (`\n`), `Read` framing
  tolerant of CRLF. Spawn: `pi --mode rpc --no-extensions --no-skills`
  plus `--no-session` (ephemeral) or `--session-dir/--session-id`.
- Commands `{id, type, ...fields}`; responses `{type:"response", id, ...}`.
- Unsolicited records are events: `message_update` with
  `assistantMessageEvent:{type:"text_delta",delta}`, and `agent_settled`
  with `aborted`. Everything else passes through as Raw (forward compat).
- `prompt` returns a disposition: `"handled"` means NO run follows;
  otherwise collect deltas until `agent_settled`. (Matches AgentX's
  `PromptAndWaitAsync` exactly.)

## Design (clean-room Rust, std + serde_json only)

- `PiTransport` splits into independent `PiReader`/`PiWriter` halves.
- Typed client mirrors the proven surface: connect, get_state,
  get_available_models, set_model, thinking levels, send_prompt,
  prompt_and_wait (progress callback, idle timeout), abort,
  get_messages, run_bash, wait-for-settled semantics.
- Errors are explicit: Transport / Command / Timeout / Protocol / Closed.
  A hung Pi can never hang the UI silently.
- Tests use a scripted fake Pi over TCP loopback, including adversarial
  inputs (malformed line counted, unknown event type passed through).

## Two real deadlocks found and fixed by tests

1. **Shared-lock reader/writer**: one Mutex around a duplex transport
   deadlocks the first command (reader blocks in recv holding the lock;
   the writer waits for the lock; the server waits for bytes). Fixed by
   the split-halves design.
2. **Drop-then-join**: dropping the client left the reader thread holding
   a dup fd open, so the fake never saw EOF and `join` hung forever.
   Fixed with `shutdown()` on drop (TCP `Shutdown::Both`; child kill),
   so EOF propagates and the reader exits.

## Verified live (not faked)

`PiClient` spawned real `pi` (via `cmd /C` — Rust can't exec `.cmd`
shims directly; environment note, not a protocol issue) and parsed a live
`get_state`: model + session id + message count. The fake-server suite
proves the state machine; the live probe proves the wire format.

## Explicitly NOT done (M4 remainder)

- No window wiring: composer submit does not yet call the adapter, and
  streamed deltas do not yet flow into the feed (next slice).
- No session persistence UI, model picker, thinking-level control,
  abort/cancel button, run status, or reconnection UI.
- No MCP adapter (local CLI-first per the inspector direction).
- Agent X itself untouched (read-only reference, as required).
