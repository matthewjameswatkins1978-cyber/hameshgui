# 038 slice 6 — starting a new session from the console

**Branch:** `hamesh/038-agent-x`.
**Status:** implemented, live-verified, committed.

## Why

038 packet §2 asks the first Agent X slice to support *starting or
attaching* to a real Pi CLI session. Slice 4 delivered `/attach`
(restore an existing session); there was still no way to start a fresh
one from inside the app — the operator had to relaunch the window.

## What changed

`crates/hamesh-console/src/lib.rs` (driver):

- `NewRequest { dir, cwd }` + `Console::pending_new`, mirroring the
  attach intent: the driver stashes, the host takes it exactly once.
  Empty `dir`/`cwd` mean "reuse the place the current session runs in",
  which is what a bare `/new` produces.
- `parse_new("/new [dir] [cwd]")`: extra fields are refused with a usage
  message rather than guessed.
- A `console-new` ghost button ("New", command `new-session`) in the
  actions row, prefilling `/new ` and handing the composer over —
  pointer press and Enter-on-button share one `prefill` helper with
  Attach (the duplication it removed was real: two copies of the same
  four lines).
- Submit handling generalises: the composer Enter path now recognises
  both `/attach…` and `/new…`, keeping the two stashes distinct.
- The actions-row hint became `/attach · /new`, because both commands
  are discoverable by pressing their button (each prefills its own
  syntax), and the longer string would have crowded 800 px.

`crates/hamesh-window/src/main.rs` (host):

- `console_new()`: same gates and retire-first discipline as attach
  (refused while a run is live, refuses when no request is stashed,
  refuses without a resolved dir/cwd), mints `ax-<unix-secs>`, clears
  the feed, respawns the worker on the fresh id.
- Both respawn paths now (a) reset the strip to `—` so the previous
  session's identity never lingers during a handshake, and (b) request
  `GetState` immediately. Without (b) the strip kept showing the session
  that had just been left until the next completed prompt — caught by
  live observation, then fixed and re-verified.
- The `"new session requested"` note is translated in both the key and
  pointer paths, exactly like attach.

## Tests

`cargo test -p hamesh-console` — 10 passed, including two new ones:

- `new_command_parses_and_stashes`: bare `/new` → empty dir/cwd; explicit
  dir/cwd carried through; `/new a b c` → usage refusal, nothing submitted,
  attach stash untouched.
- `new_button_prefills_and_enters`: pointer press prefills `/new ` and
  moves focus to the composer; Tab-Tab-Enter from the composer does the
  same.

`tab_reaches_abort_and_enter_runs_the_shared_gate` was updated for the
real new Tab order (feed → composer → attach → new → feed; Abort joins
after New while streaming) and `tree_is_eye_clean_with_idle_status` now
asserts the `console-new` node at both window sizes.

## Live evidence (real window, real Pi CLI)

Launched:
`hamesh-window-demo.exe --console --pi --inspect --session-id=ax-live-038
--session-dir=%TEMP%\ax-038-sessions --session-cwd=D:\Projects\hameshgui`,
driven only through `hamesh.exe` (inspector Hand).

| step | reply / observation |
| --- | --- |
| `detail console-session` (before) | `ax-live-038` — attached session from launch |
| `key Tab`, `type composer /new`, `key Enter` | `result: "new session sid=ax-1793044466 (fresh, no history)"`, `focused: composer` |
| `detail console-session` (after handshake) | `ax-1793044466`; `console-model` = `Muse Spark 1.3 Contributor`; `console-count` = `0 messages` |
| `type composer "Reply with exactly: NEW-OK"`, `key Enter` | submitted; run settled Idle with 2 messages |
| `detail msg_0` / `msg_1` | user `Reply with exactly: NEW-OK`, agent `NEW-OK` |
| `map` | 0 clipped nodes at 800×600 |
| `snapshot docs/038/evidence/console-new-session.png` | bound to `app_rev 20`, `scene_rev 22` |

So the fresh session is not a cosmetic id: the new worker really
handshakes, answers a prompt and updates the strip.

## Visual inspection (pixels)

`docs/038/evidence/console-new-session.png` (800×600): header reads
`Console`, `Idle` badge, `Muse Spark 1.3 Contributor`, `ax-1793044466`,
`2 messages`; feed shows the prompt and the `NEW-OK` reply; actions row
reads `/attach · /new` with `Attach`, `New` (teal focus ring — keyboard
focus is visible on the button that was actually focused), and `Abort
run` painted disabled. Nothing clipped or overlapping.

One honest observation, not fixed here: the composer paints no
surface or border when empty and unfocused, so at rest the input area is
invisible (it is a bare `Role::TextInput` in every Hamesh composition,
including the M3 demo). That is a kit-wide visual decision, not a
console bug introduced by this slice; it deserves its own design pass
rather than a token poke at the end of a behaviour slice.

## Not done / next

- Model selection from the console, and `/abort`-style slash commands.
- Composer affordance (see above).
- The delegated companion slice on this branch: console error state.
