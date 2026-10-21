# 015 — M4 live loop: composer submit streams through a background Pi worker

Status: accepted (spike proven live 2026-10-21, `--m3 --gpu --inspect --pi`).

## Context

M3 gave us the chat surface (feed + composer + focus) and decision 012 gave
us a clean-room `hamesh-pi` adapter (JSONL over `pi --mode rpc` stdio, split
reader/writer halves). M4 closes the loop: a submission in the window comes
back as streamed text in the feed, with the UI thread never blocking.

## Decision

- A **background worker thread** owns the only `PiClient`. It speaks to the
  UI thread over two `mpsc` channels: `PiJob::{Prompt, Abort}` in,
  `PiEvt::{Started, Delta, Done, Failed}` out. No shared state, no locks.
- The **UI thread owns the submit decision**: `m3_keys_apply` detects a
  submission *after* the key batch applies (feed grew by a user message),
  because batches type the text and press Enter in one go — a pre-read of
  the composer always sees it empty (found live, fixed same turn).
- `about_to_wait` **drains** pending events every frame and folds them into
  the feed (empty agent bubble on `Started`, append on `Delta`,
  scroll-to-bottom + trace on `Done`, system message on `Failed`).
  `push_live` on `Feed` hands out the streaming index without exposing
  layout internals.
- Both producers (winit keyboard, inspector `KeyMsg`) land in
  `m3_keys_apply`, so external Eye/Hand submissions stream identically.
- `PiJob::Abort` is honored by the worker today; the UI keeps Esc = quit
  for now (`#[allow(dead_code)]` + comment, follow-up when Esc stops
  meaning quit during a live run).

## Evidence

- Live 75s window run: trace shows `pi.worker` → `m3.pi_queued
  ("m3 smoke")` → `m3.pi_done (aborted=false, chars=970, messages=32)`;
  snapshot PNG shows the user bubble plus the streamed agent reply.
- Gates unchanged: `cargo fmt --check`, `clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace` all green.

## Consequences

- Network/auth failures surface as feed `system` messages, never a hung UI.
- The live prompt in smoke mode is the fixed string `m3 smoke`; a
  scripted-meaningful prompt and abort UX are queued follow-ups.
- GPU/CPU pixel divergence on the M3 scene (4–6%, rounded bubbles + glyph
  AA) is unchanged and still out of scope: structure agrees, tolerance
  model owns it.

## Addendum 2026-10-21 — machine-checked regression

- The smoke prompt now demands the token `HAMESH-SMOKE-OK` back, and the
  deadline path records `m3.pi_assert` (`PASS` / `FAIL(..)` /
  `INCOMPLETE(..)`) plus a `SMOKE-PI-*` stdout line. Live 90s proof:
  queued → done (chars=15, exact token) → `PASS`. The `m3 smoke`
  and abort-UX follow-ups above are both done: Escape during a live run
  aborts it via `m3.pi_abort` (window Esc arm and external key-Escape
  share the rule); Escape when idle still quits.
