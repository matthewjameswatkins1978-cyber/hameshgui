# 038 slice 7 — the console says when the engine is broken

**Branch:** `hamesh/038-agent-x`.
**Status:** implemented, live-verified, committed.

## Why

038 packet §2 asks the first Agent X slice to display *session state and
errors*. Slice 4 gave the console a session; a failed worker action only
appended a `system` feed line, which scrolls away and is invisible to
the inspector unless the operator reads the feed. A dead engine and a
healthy idle one looked identical in the status strip.

> Note on provenance: this slice was first delegated to a worker in an
> isolated worktree. It stalled with no edits after ~25 minutes (a
> pending provider call, no events for 6 minutes), was interrupted, and
> the slice was implemented here instead. The worktree and branch were
> removed; nothing from the abandoned attempt was merged.

## What changed

`crates/hamesh-console/src/lib.rs` (driver — display state only):

- `Console::error: Option<String>`, `set_error(Option<&str>)`,
  `has_error()`. `set_error` keeps at most `ERROR_MAX_CHARS = 60` chars
  and appends `…`, because the strip is a headline: the full text is
  already a system message in the feed and an unbounded caption would
  crowd the buttons at 800 px.
- The status badge becomes `Error` (`b::Variant::Error`) while an error
  is unresolved — it outranks `Streaming`, because a broken engine is
  more urgent than a run in flight.
- The `console-error` caption takes the actions-row hint slot (the hint
  `/attach · /new` returns when the error clears). Choosing the hint's
  slot rather than a new header caption is what keeps the row unclipped
  at 800 px: the header already carries title, badge, model, session and
  count.
- Small honesty fix seen in this slice's own screenshot: the count reads
  `1 message`, not `1 messages`.

`crates/hamesh-window/src/main.rs` (host):

- `Act::Failed(e)` (console mode) → `set_error(Some(&e))`; the feed line
  stays as the detail.
- `Act::State(_)` → `set_error(None)`: a worker that answers `GetState`
  is healthy, so the headline's cause is gone. This is the clear path
  that matters, because a successful attach or `/new` produces `State`,
  not `Started`.
- `Act::Started` and `Act::Done` → `set_error(None)` for the same reason.

## Tests

`cargo test -p hamesh-console` — 13 passed, three new:

- `error_state_paints_a_headline_and_turns_the_badge`: no node when
  clean; `boom` paints the caption, the hint yields, the badge reads
  `Error` with variant `error`, no node clipped at 800×600 or 1440×900;
  clearing removes the node and restores the hint and the `Idle` badge.
- `long_errors_are_bounded_and_never_clip`: a 400-char message becomes
  exactly 61 chars ending in `…`, unclipped at both sizes.
- `error_outranks_the_streaming_badge_until_cleared`: `Streaming` →
  `Error` → back to `Streaming` when cleared.

## Live evidence (real window, real Pi CLI)

Failure forced honestly by pointing the worker at a session dir that
cannot exist (`--session-dir=Z:\nope`), so `pi` spawns and the handshake
closes:

| step | observation |
| --- | --- |
| launch `--console --pi --inspect --session-id=ax-err --session-dir=Z:\nope` | `console-status` = `Error`, variant `error`; `console-error` = `handshake: connection closed`; feed holds `Pi error: handshake: connection closed`; `console-count` = `1 message` |
| `snapshot docs/038/evidence/console-error-state.png` | bound to `app_rev 20`, `scene_rev 9` |
| `/attach ax-live-038 <dir> <cwd>` + Enter | `attached sid=ax-live-038 (history restoring)` |
| after the handshake | `console-status` = `Idle`, `console-session` = `ax-live-038`, `console-model` = `Muse Spark 1.3 Contributor`, `console-error` **gone** |

So the strip turns red on a genuine failure and clears itself once the
engine answers again — not on a timer, not on a guess.

## Visual inspection (pixels)

`docs/038/evidence/console-error-state.png` (800×600): `Console` title,
red-outlined `Error` badge (visually distinct from the teal `Streaming`
pill), `—` model and `—` session, `1 message`; the feed's system line
reads `Pi error: handshake: connection closed`; the actions row shows
the dim headline in the hint slot with `Attach`, `New`, `Abort run`
(disabled) still aligned right; nothing clipped, nothing overlapping.

Recorded, not fixed here: the composer still paints no surface or border
when idle (kit-wide `Role::TextInput` styling, same as the M3 demo) —
this is the visible dark band between feed and actions row, and it
wants its own design pass.

## Not done / next

- Model selection from the console.
- Composer affordance.
- Retry-after-failure (the strip reports; it does not yet offer a retry
  action) — worth pairing with a `/retry` slash command when a real
  failure mode recurs.
