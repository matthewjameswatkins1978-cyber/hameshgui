# 036 native proof

Real OS input (SendInput, Eye-aimed at map rects) into `hamesh-window`
`--sheet`, `--gallery` and `--tasks`. Every driver runs the same
shared code as its headless drill (`Hit::of` routing, `Key`
vocabulary, nav helpers).

## Design-system sheet (`--sheet`, 800x600)

The sheet is a real driver (`hamesh-sheet`), not a static render:
real hover, click, Tab walk, arrow move and Enter activation were
driven through a winit window and inspected frame by frame.

- hover ghost-rest with a real pointer move (`sheet-native-800x600-hover.png`)
- click activated it and latched the pressed look (`...-pressed.png`;
  trace: `SHEET click: activated ghost-rest`)
- Tab walked list -> buttons, Home/Right moved the button cursor,
  Enter activated primary-hover (`...-focused.png`; trace: `focus ->
  buttons`, `button cursor: btn-primary-hover`, `activated
  primary-hover`)
- Full stdout trace in `sheet-native-800x600-stdout.txt`.

Two honest calibration notes from the runs. First, the ghost
variant's pressed state differs from rest only in label brightness
(surface stays `raised`); the matrix shows it, but it is subtle by
design — primary carries the strong pressed ramp. Second, the OS
SendInput path proved flaky run to run (identical inputs sometimes
landing nowhere while the app stayed alive and responsive); the
drive re-foregrounds and verifies every input with a retry loop, and
every accepted frame above comes from a fully green verified run.

## Gallery (`--gallery`, 800×600)

Re-ran the 035 native drive after the `GKey` → `Key` migration:

- click selected layer 2 (`GALLERY click: selected layer 2`)
- Tab moved focus to the picker (`focus -> picker`)
- typed `ne`, Down moved the highlight, Enter committed
  (`picker committed: DeVinne Txt BT`)
- reopened, then an outside click selected layer 0 and dismissed
  the popup (`selected layer 0 (popup dismissed)`)

No behavior change from the key unification. Full stdout in
`gal-native-800x600-036-stdout.txt`.

## Tasks (`--tasks`, 800×600)

New drive (`work035/taskdrive.ps1`):

- click selected task row 2 (`selected task row 2`)
- click opened the filter (`filter opened`)
- typed `o` — the queue filters live (`filter type: true`)
- `task-native-800x600-filtered.png`: popup floats over the queue
- click on Toggle done: `task 2 done (popup dismissed)`
- `task-native-800x600-toggled.png`: popup gone, row stays
  selected, field back to its placeholder (cancel discards the query)

This drive caught a real defect: the action press did not dismiss an
open popup (gallery's row press did). Fixed in the driver with a
regression test (`action_buttons_drive_real_commands` now reopens the
filter and asserts dismissal).

## Frames

- `task-native-800x600-filtered.png` — live filter, floating popup.
- `task-native-800x600-toggled.png` — post-action: no popup,
  selection preserved, focus ring on Toggle done (actions zone).
- `gal-native-800x600-036.png` — gallery dismissal state, post-`Key`
  migration.
