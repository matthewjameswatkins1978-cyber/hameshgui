# 038 slice 5 — Hand `type` parity for tasks and gallery

**Packet item:** `docs/037/followon-038a.md` (038-A).
**Branch:** `hamesh/038-agent-x`.
**Status:** implemented, live-verified, committed.

## What was wrong

The inspector `type` op worked for `--m3`, `--console` and `--editor` but
answered `NO_M3` for `--tasks` and `--gallery`, even though every one of
those apps already accepted typed characters through `key Char`. An
external operator could narrow a filter with 8 key calls or not at all
with `type`.

## What changed

- `crates/hamesh-tasks/src/lib.rs` — `Tasks::type_text(&str) -> String`.
  Focuses the filter zone, opens the popup on demand, feeds each char
  through `Picker::push_char` (the same call `Key::Char` makes), then
  re-derives the list cursor with `resync_selection`. Empty text is a
  no-op that returns `filter: nothing to type`.
- `crates/hamesh-gallery/src/lib.rs` — `Gallery::type_text(&str) -> String`.
  Same route in the picker zone; the driver owns its layer labels, so no
  caller argument is needed. Empty text returns `picker: nothing to type`.
- `crates/hamesh-window/src/main.rs` — `Op::Type` gains tasks and gallery
  arms (rev-stamped trace, `mechanism: HumanEquivalent`, `focused` zone in
  the reply). The `NO_M3` message for the remaining modes now says exactly
  where typing works and why sheet does not: *"typing needs an interactive
  mode (m3, console, editor, tasks or gallery); sheet has no text field"*.

No second input system: `type_text` is `key Char` batched, and both
drivers keep one key vocabulary (`hamesh_widgets::Key`).

## Tests

- `cargo test -p hamesh-tasks` — 12 passed; new
  `type_text_walks_the_same_route_as_the_keyboard` proves the typed path
  and the keyboard path land in identical `shown()`/selection state, that
  the painted tree drops the non-matching rows, and that Escape restores
  the queue.
- `cargo test -p hamesh-gallery` — 6 passed; new
  `type_text_walks_the_picker_route_without_caller_labels` proves the
  typed query, filtered set and focused zone equal the keyboard's.
- `cargo test --workspace` — no failures.
- `cargo fmt --all -- --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` silent.
- Standing drill `cargo run -p hamesh-drill` — DRILL-PASS (stale → repair
  → submit → wait → race → click).

## Live evidence (external Hand only, real native window)

`target/debug/hamesh-window-demo.exe --tasks --inspect`, driven with
`target/debug/hamesh.exe`:

| op | reply |
| --- | --- |
| `type filter-field router` | `success: true`, `hit: filter-field`, `focused: filter`, note `filter type: "router"` |
| `map` | one `tasks-row-0` (`Wire the Hit router`); every other row gone |
| `detail filter-value` | `props.text = "router"` |
| `type filter-field ''` | `success: true`, note `filter: nothing to type`, `app_rev` unchanged, query still `router` |
| `snapshot docs/038/evidence/tasks-type-parity.png` | bound to `app_rev 27`, `scene_rev 10` |

`--gallery --inspect`, same CLI:

| op | reply |
| --- | --- |
| `type fonts-field newsreader` | `success: true`, `hit: fonts-field`, `focused: picker`, note `picker type: "newsreader"` |
| `map` | popup holds exactly one option (`fonts-opt-0`) |
| `detail fonts-opt-0-label` | `props.text = "Newsreader"` |
| `detail fonts-value` | `props.text = "newsreader"` |
| `snapshot docs/038/evidence/gallery-type-parity.png` | bound to `app_rev 61`, `scene_rev 9` |

`--sheet --inspect`: `type sheet-title hello` refuses honestly —
`ok: false, code: NO_M3, error: "typing needs an interactive mode (m3,
console, editor, tasks or gallery); sheet has no text field"`.

## Visual inspection (pixels, not telemetry)

- `docs/038/evidence/tasks-type-parity.png` (800×600): filter field shows
  `router` with the teal focus ring; the open popup lists the single match
  `Wire the Hit router` directly under the field; the list holds that one
  row, selected, with its icon; header alignment (`Tasks`, field, primary
  `Toggle done`, ghost `Clear`) unchanged; nothing clipped or overlapping.
  Large dead area below a one-row list is inherent to a filtered queue,
  not a layout defect.
- `docs/038/evidence/gallery-type-parity.png` (800×600): picker field shows
  `newsreader` and the popup holds one highlighted option `Newsreader`;
  the poster headline reads *Set in Newsreader*, so the visible state
  agrees with the narrowed query. Pre-existing narrow-width behaviour:
  long layer names truncate with an ellipsis (`Hero i…`, `Shape…`) in the
  168 px rail at 800 px wide — unchanged by this slice, recorded honestly
  rather than fixed here.

## Not done / next

- Sheet keeps `NO_M3` on purpose (no text sink).
- Console already types via `hamesh_automation::type_human`; that route was
  left alone.
- Next useful Agent X slice: the composer's own key contract (Ctrl+Enter
  versus Enter) per the design brief, or new-session start alongside
  `/attach`.
