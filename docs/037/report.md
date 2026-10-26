# HAMESH 037 — Milestone 1 report: Pi session console (App 1)

Baseline: branch `hamesh/037-real-apps` @ `fdc7a1a` (off PR #7 head
`8aab6be`; PR #7 reviewed APPROVE, still unmerged — recorded, not
blocking). Workshop rules in effect: local commits, push for
visibility, no hosted CI, targeted tests during development.

## 1. What was built and whether it works

**App 1 — Pi session console, working and desktop-verified.** A real
application, not a harness demo: M3 chat surface (virtualized feed +
grapheme composer + tree focus) plus a live engine-status strip
(streaming badge, model, session id, message count, Abort button),
operable end-to-end by an external agent through the advertised
interfaces only.

- Headless: `cargo test -p hamesh-console` 5/5 (idle status, live
  status, shared-core typing/submit, Tab-to-Abort, focus preservation).
- Window: `cargo test -p hamesh-window` 16/16, `fmt --check` clean,
  `clippy --all-targets` silent.
- Desktop: `--console --inspect --pi --smoke-test=60` →
  **SMOKE-PI-PASS** (focus script, token prompt, streamed reply,
  token assert, exit 0).
- External operator loop against a live window (no implementation
  knowledge used): map → detail → Tab → type → Enter → streamed run
  → abort mid-run via inspector click → Idle, with live model and
  session id on the strip. Evidence in `docs/037/evidence/`
  (screenshot, smoke map/trace/PNG). One genuine failure found and
  fixed through this loop (see §4).

## 2. What was reused (not rebuilt)

M3 `handle_key` core, `Focus`/`Hit`/`Key` vocabulary, Blocks
composers, `focus_human`/`type_human`, the Pi worker and its
channel protocol, the M3 smoke script/assert pattern, the window
event-loop/inspector/smoke harness. New code is composition and
driver state only: `hamesh-console` crate + one window mode
following the tasks precedent line by line.

## 3. What Hamesh lacked (found by building, not predicted)

1. **Focus did not survive recompose.** Every console refresh
   rebuilt `Focus` from zero, stranding external keyboard operators
   on the feed after any intervening refresh. Headless tests missed
   it (single refresh per test); the live operator loop caught it
   on the first `type` → `Enter` sequence. Fixed in the driver
   (focus preserved by id, fallback to head) + regression test.
2. **Inspector Hand was M3-only.** Key/Type/Click arms were gated on
   M3 widgets: driver-composed apps (tasks/gallery/sheet) are
   Eye-readable but not keyboard-operable, and clicks never reach
   driver semantics. Console is now the first driver-composed app
   with full external Hand parity. Tasks/gallery/sheet still lack
   it — ranked first below.
3. **Enter on a focused button is a known core gap.** The shared
   `handle_key` core ignores it, so Abort is pointer + host-Escape
   only. Pinned by a test that locks in the observed behavior
   instead of hiding it. Ranked second below.
4. **No engine-state round trip.** The worker protocol had no
   `GetState`; the status strip would have shown placeholders.
   Added `PiJob::GetState` / `PiEvt::State` (15 lines, same
   channel shape) — model/session now arrive from the live worker.
5. **Specimen is not operable.** The real client pins hamesh rev
   `c5b794c` and exposes no inspector server. Flagship gap, not a
   037a task.

## 4. Framework changes (all application-justified)

- New `hamesh-console` driver crate (product side, no harness logic).
- `PiJob::GetState` + `PiEvt::State` + worker/drain arms.
- `--console` window mode + inspector Key/Type/Click routing for it.
- Driver focus preservation + regression test.
- No new abstractions, no core rewrites, no dependency additions.

## 5. Remaining problems, ranked

1. **External Hand parity for tasks/gallery/sheet** — same routing
   pattern as console, small, unblocks independent operation of
   three existing apps. Highest value per cost.
2. **Shared-core button activation** — one gate (focused Button +
   Enter runs its command through the disabled check), keyboard
   operators need it everywhere. Small-medium, needs care to keep
   the core shared.
3. **Console status freshness at open** — enter-time `GetState`
   races worker spawn; covered today by post-`Done` refetch, but a
   slow first paint shows "—". Cosmetic, cheap to close.
4. **Specimen rev lag + no inspector surface** — needs its
   maintainer's call, not a drive-by upgrade.
5. **IME live verification + accessibility bridge** — unchanged,
   still queued; the Agent X question makes a11y more urgent (below).

## 6. Agent X outlook (feasibility facts, no code yet)

Agent X is C#/.NET Avalonia; Hamesh is Rust. A "Hamesh-powered slice"
inside the Agent X process means crossing that boundary (FFI/embed —
expensive, and against "reuse, don't replace"). The realistic paths:

- **(a) Hamesh operates Agent X externally.** Agent X already has
  UIA-driven smoke and accessible names; Hamesh's Hand is
  framework-native, not OS-automation. This path *is* the queued
  accessibility-bridge work wearing a concrete hat.
- **(b) Parallel Hamesh-native client.** The console built here
  already covers the functional slice (connect, messages/streaming,
  prompts, status, keyboard/mouse operation) minus transcript and
  session-registry features. Real but duplicates Agent X's UI.
- **(c) Agent X calls Hamesh components.** Wrong direction across
  runtimes; rejected.

No code until the direction is chosen — that choice is product
judgement, not an implementation detail. Recommendation: pick (a)
or (b) at the milestone review; (a) aligns with the a11y backlog,
(b) is immediately buildable.

## 7. Next step (taken): App 2 mini text editor

Built `hamesh-editor`: file lines in a virtualized `ListView`, the
selected line edited in a `Composer`, status strip plus Save — zero
framework changes, maximal reuse of the tasks/console patterns.

## 8. App 2 verification (desktop evidence in `docs/037/evidence/`)

- Headless: 7/7 (`cargo test -p hamesh-editor`) — byte-exact
  CRLF/LF round trips, dirty/save gate, Escape restore,
  Tab traversal, Eye-clean frames, press-to-edit, type-to-edit.
- Window: `--editor --inspect --smoke-test` → scripted fixture
  open/edit/save with a file-bytes assert (`SMOKE-EDITOR-PASS`).
- External operator loop on a real CRLF file, keyboard only:
  map/detail → Down → Enter → type → Tab → Enter (save) →
  file bytes `one|two!|three` with CRLF intact → type more →
  Escape → line restored to the *saved* text, badge clean.
- Two defects caught along the way, both fixed with regression
  coverage: an 8px chrome overflow clipping the edit field at
  800x600 (measured, budget now 188px from evidence), and a stale
  cancel baseline that reverted past the save point (cancel now
  restores the saved line; commit and save refresh the baseline).
- Still open at the time: ranked weaknesses 1–2 from §5 — now
  worked (see §10). Remaining: editor `--file=` handling for missing
  files (currently an honest note, no dialog).

## 10. Ranked fixes landed: external Hand parity + Enter-on-button

- **Parity for tasks/gallery/sheet.** Inspector `Key`/`ClickId`/
  `ClickAt` now route to the driver `press`/`key` cores with the same
  rev discipline and trace shape as console/editor (`*_keys_apply`
  + `*_press` per mode, one shared `keymsg_to_wkey` mapping instead
  of a fourth copy). `focused_zone()` on all three drivers (+ console
  + editor already had it) so the Eye reports focus honestly.
  Headless: tasks 11/11, gallery 5/5, sheet 7/7. Live, keyboard-only:
  tasks (Tab traversal with zone names, Char filter, Tab Tab Enter
  Clear, stale-inject → STALE → retry, row press with popup-occlusion
  honestly hit-tested, Escape dismiss); gallery (Tab to picker, type
  to narrow, row press); sheet (Tab to buttons, Enter activates,
  button press). Evidence: `*-live-operated.png` × 3.
- **Enter-on-button for console.** The shared core still has no
  button activation (deliberately: commands are driver-owned, and a
  god-function dispatcher was explicitly rejected in 036). Instead
  the console driver maps Enter on the focused Abort to the same
  abort intent as a pointer press, and one window-level gate
  (`console_abort`) turns it into a worker cancel — pointer, Enter
  and inspector key share it. Live proof: Tab Tab Enter cancelled a
  streaming run mid-response (`abort requested (cancel sent)` → Idle,
  screenshot `console-enter-abort.png`). Tasks/sheet need no such
  change (their zone actions already run on Enter); editor needs none
  (zone-gated Save already works).
- **Incidents, honestly recorded.** (i) A ~100-paragraph streaming
  run made the inspector stop answering (`TIMEOUT ui busy`, UI thread
  silent for 60s+ while the process stayed alive); cause not yet
  isolated — needs a bounded follow-up, not a conclusion. A moderate
  run immediately afterwards behaved perfectly. (ii) One `detail`
  read returned an empty envelope during a Done→GetState transition;
  retry succeeded. Both are open unknowns, not verdicts.

## 11. Standing recommendation

Merge decisions for PR #7 + 037 branch, and the Agent X direction
call (a: accessibility-bridge operation, b: parallel native
client, c ruled out), remain the milestone review items.
