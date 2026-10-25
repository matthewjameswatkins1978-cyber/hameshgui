# 036 native proof

Real OS input (SendInput, Eye-aimed at map rects) into `hamesh-window`
`--gallery` and `--tasks`. Both drivers run the same shared code as
the headless drills (`Hit::of` routing, `Key` vocabulary, nav helpers).

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
