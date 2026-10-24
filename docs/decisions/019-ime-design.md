# 019 — IME preedit/commit: model shipped, OS edge documented

Status: accepted (model + wiring + tests; live-IME verification queued as
human dogfood, non-blocking).

## Context

The composer took only committed keystrokes. On Windows, CJK input arrives
as winit `Ime` events (`Preedit` marked text, then `Commit`), verified
against the vendored winit 0.30.13 source: `Ime::{Enabled,
Preedit(String, Option<(usize, usize)>), Commit(String), Disabled}`,
`Window::set_ime_allowed(bool)`, `Window::set_ime_cursor_area(pos, size)`.
Preedit cursor offsets are byte-wise; empty string means cleared.
Platform reality (decision 008, unchanged): winit on Windows is
IMM32-only, with known upstream gaps (Sogou #2780, TSF #4508). IME
behavior therefore cannot be fully proven without a human and a real
IME — this slice is honest about that boundary.

## Decision

- `Composer` gains `preedit: Option<String>`: marked text shown inline
  at the caret via `shown_text()` (prefix + preedit + suffix) but never
  committed until `Commit` — or submit, which commits first rather than
  dropping user input. Caret stays at the composition start, so
  `caret_x` (committed prefix only) is unaffected. `Commit` folds
  through the grapheme-correct insert path (decision 017 holds for CJK
  extensions and emoji commits too).
- `sync_composer` publishes the shown text plus a `preedit` prop, so the
  Eye observes composition exactly as the user sees it.
- `window_event` grows a 4-line `Ime` arm delegating to `ime_event`:
  Enabled/Disabled traced; Preedit/Commit apply only when the composer
  is focused, then refresh + trace. `set_ime_allowed(true)` on M3 entry;
  cursor area follows the caret on every Preedit (approximate rect +
  caret_x; the OS clamps).
- Explicitly NOT this slice: underline/decoration rendering of marked
  text (scene has no decoration runs — shown inline, documented),
  `ime-purpose` hints, surrounding-text/delete-surrounding, disabling
  IME on focus-leave (allowed while M3 is up; unfocused events ignored).

## Evidence

- 3 headless model tests pass (display-without-commit + caret
  invariance, commit-through-graphemes, submit-commits-first); full
  workspace gates green (fmt, clippy `-D warnings`, all tests).
- The winit arm is thin delegation to tested methods by design — stated
  plainly because no synthetic `WindowEvent::Ime` test exists
  (constructing one needs a live `ActiveEventLoop`) and no smoke can
  fire a real IME.
- Queued dogfood (non-blocking, needs Matthew + a real IME once):
  switch to MS Pinyin in the `--m3` window, compose 中文, confirm the
  marked text shows inline and commits on selection; candidate window
  position sane.

## Consequences

- CJK composition now has a correct, tested home in the model; the OS
  edge is wired and traced, awaiting only a human minute to confirm.
- No behavior change for existing paths: preedit defaults to `None`,
  `shown_text` == `text`, all prior tests untouched and green.
