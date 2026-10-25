# HAMESH 036: Beautiful by Default — final report

Branch: `hamesh/036-beautiful-by-default` (5 commits on `main @ 3194ed5`).
All evidence below was observed on this machine or on hosted CI; nothing
is claimed without a check behind it.

## DESIGN — one default visual language

- **Theme state-system contract** (`hamesh-theme`): `ACCENT_HOVER` /
  `ACCENT_ACTIVE` derived from the existing accent (no new hues),
  documented precedence (disabled > pressed/hover/active > rest, focus
  as a ring that never moves paint), contrast suite extended — BG text
  on hover/active and MUTED on SURFACE all hold ≥3:1.
- **Buttons** paint the contract: 3 variants × Rest/Hover/Pressed/
  Disabled + focus ring, `enabled=false` on Disabled so paint and
  hit-testing agree (automation reports "disabled" honestly).
- **`swatch` composer** paints the exact token components reference.
- Ghost-hover was a no-op in the first mapping; the 036 sheet drill
  caught it and the mapping now brightens Ghost text on hover.
- Visual proof: `apps/hamesh-036-lab` renders a design-system sheet
  (button matrix, selection states, type ramp, contract card,
  tall-only SURFACES strip, short-viewport compaction) Eye-clean at
  1440×900 / 1100×720 / 820×560 — inspected, not just telemetered.

## CONTROLS — finished reusable pieces

- `Hit::of` router (`hamesh-widgets`): one parent-climb resolving
  list rows, picker options (pick-beats-picker precedence), picker
  fields, button actions and inert chrome. 7-case regression.
- Shared `Key` vocabulary: `GKey`/`TKey` deleted; gallery, tasks,
  window native paths, both drills and automation speak
  `widgets::Key` (grew `PageUp`/`PageDown`; all matches were
  wildcard-complete, so the extension is non-breaking).
- `list_nav` / `picker_nav` helpers centralize key→Nav mapping with
  a mapping regression (nav keys act, `Char`/`Enter`/`Tab` stay
  driver-owned).
- `handle_key` (M3 composer/feed) unchanged; paging added there too.

## DEVELOPER EXPERIENCE — measured, not asserted

Per-app code that no longer exists or never had to be written:

- Gallery `route()` + `Route` enum deleted (21 lines); blocks
  pointer-mapping test climbs via `Hit::of` (8 lines deleted).
- 12 key→nav match arms per app collapsed into 2 shared-helper
  calls each; two key enums deleted outright.
- The tasks driver (608 lines incl. 6 tests) contains zero lines of
  parent climbing, identity parsing, key-enum declaration or nav
  mapping — the first composition that only declares controls.
- Duplicate component ids now fail fast: a `debug_assert!` in
  `App::add` (all production inserts are one-shot; full suite
  proves no false positives). It caught two real collisions during
  this milestone — including a self-parent cycle that hung every
  tree climb forever (an app root and a list both named `tasks`).

Deliberately NOT unified (documented): gallery/tasks zone focus vs
`handle_key`'s tree-rebuilt focus. Merging them now would mean a
god-function dispatcher or losing the Tab-dismiss-then-advance
contract. The convergence point is the shared pieces above.

## GALLERY — native and interactive

- `hamesh-window --gallery` re-verified with real OS input after the
  `Key` migration: click select, Tab focus, type-ahead filter, arrow
  nav, Enter commit, reopen, outside-click dismissal — trace +
  `docs/036/gal-native-800x600-036.*` (frame inspected: headline set
  in the picked live font, popup gone, selection kept).
- The 035 headless drill still passes unchanged (same intents).

## REUSE — second independent composition

- `hamesh-tasks`: queue + live filter + toggle-done/clear actions +
  detail rail (status badge, progress) — built ONLY from Blocks
  composers over driver-owned state, on the shared router.
- Headless drill at 3 sizes: select, filter-narrows-live,
  action-toggle flips badge+progress, focus traversal; Eye
  strict-clean; `task-*.png` inspected.
- Native `--tasks` window mode + OS-input drive (`taskdrive.ps1`):
  real clicks select/filter/toggle; `task-native-*.png` inspected.
  The drive caught a real hole — action presses left the popup open
  — fixed with a regression test (command runs before dismiss so
  Clear can still read the open query).

## QUALITY

- `cargo test --workspace`: 185 passed, 0 failed.
- `cargo fmt --check` clean; `clippy --workspace --all-targets -D warnings` clean.
- Hosted CI: green on every pushed branch head (runs 40218234925 for
  `cbd918e`, 40219239836 for `6252271`, 40220205159 for `a258d44` —
  all `completed` / `success`; PR checks attach on open).
- Working copies kept CRLF per repo convention (`cargo fmt` write
  mode normalizes to LF; restored via LineDoctor after every run —
  recorded here so the next worker does not rediscover it).

## REPO

- Branch `hamesh/036-beautiful-by-default`, 5 commits, pushed and
  remote-verified. No merges, no releases.
- `docs/036/`: `notes.md` (native proof), this report, native PNGs +
  stdout traces. Headless frames under `target/036/` (gitignored,
  reproducible via `cargo run -p hamesh-036-lab`).

## ATTENTION

- Open question carried forward, not a blocker: whether zone focus
  (gallery/tasks) should eventually merge into tree-rebuilt focus
  (`Widgets::Focus`). Evidence for a future decision, not a 036 gap.
- The tasks queue list stretches its viewport at tall sizes (honest
  empty space, same as any real queue with few items) — acceptable,
  noted so nobody "fixes" framework-correct behavior.
- Taffy edge (not fixed, documented): a list root placed directly in
  a row beside a fixed sibling can hang layout; the rail pattern
  (list inside a grow column) is the established shape. A framework
  fix belongs to a layout milestone, not here.
