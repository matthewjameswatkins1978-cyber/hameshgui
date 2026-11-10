# Follow-on packet 038-A — Type-op parity for driver-composed apps

**Status:** proposed (awaits milestone review alongside PR #7 / PR #8).

## Objective

Close the last external-Hand gap left by the parity slice: the
inspector `type` op works for M3/console/editor but reports `NO_M3`
for tasks/gallery/sheet, even though every one of those apps accepts
typed input through `key Char` today. An independent operator should
be able to type wherever text can be entered, through either spelling.

## Baseline

`hamesh/037-real-apps` @ current head (includes console, editor,
tasks/gallery/sheet Key+Click inspector routing, `focused_zone()`
everywhere, Enter-on-console-abort).

## Scope (bounded)

1. Add `type_text(&mut self, text: &str) -> String` to the tasks and
   gallery drivers (focus the filter/picker field, open the popup if
   closed, feed characters through the same paths as `key Char`,
   return a human note). Mirror the console/editor `type_text`
   contract exactly (visible excerpt from the task: gallery needs its
   layer names internally rather than as a caller argument).
2. Wire the window `Op::Type` arms for tasks/gallery to those methods
   (same rev discipline, same `HumanEquivalent` stamp shape).
3. Sheet is explicitly out of scope: it has no text sink (buttons +
   selection list only), so `NO_M3` there stays honest. Update both
   `NO_M3` messages to say exactly where `type` works.
4. Headless tests per driver (type narrows, empty text is a no-op,
   disabled/closed states report honestly). Live verify per mode:
   `type` into the tasks filter narrows the queue; `type` into the
   gallery picker filters layers; both through a fresh window with
   snapshots in `docs/037/evidence/`.

## Acceptance (all must hold)

- `cargo test` for the touched drivers green; `fmt --check` clean;
  `clippy --all-targets` silent; CRLF convention held.
- Live, external-only: type → observe narrowing via `map`/`detail`
  for tasks and gallery; snapshots committed as evidence.
- `NO_M3` messages enumerate exactly the modes with no text sink.

## Constraints

- Typing reuses the `key Char` paths (batched, not a second input
  system). No driver changes beyond `type_text`. No new crates,
  no framework abstractions, no IME/a11y work.
- Keep the change reviewable in one PR-sized unit; do not merge
  without independent review.

## Explicitly out of scope

- Sheet `type` support (no text sink; documented as such).
- The streaming inspector-stall investigation (separate spike with
  its own time box; see report §12).
- Agent X integration (blocked on the direction call).
- New applications.
