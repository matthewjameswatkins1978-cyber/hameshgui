# HAMESH 037 — Independent operator probe (Phase 2 evidence)

## Setup

A fresh-context `worker` subagent was given ONLY the advertised
surface: `docs/inspector-protocol.md`, the discovery file, and the
`hamesh` CLI binary. Explicitly forbidden: all sources, all decision
docs, all prior drive scripts, all writes, git, network. Task: the
S1 Discovery scenario — (a) app purpose, (b) control inventory with
ids, (c) current state, (d) exact commands to mark a task done — plus
a full observation/action transcript. Host window: `--tasks
--inspect` on loopback, left running and untouched by the supervisor
throughout.

## What the operator did (observed from its transcript, not claimed)

1. Read the protocol doc + discovery file; derived port/token auth.
2. Recovered from its own first failure (unquoted PowerShell args →
   `ParserError`; retried quoted with `&` — no help requested).
3. `map` → full inventory: 8 `tasks-row-N` items with icon/label
   children, `btn-toggle` (`toggle-done`, enabled), `btn-clear`
   (`clear-filter`, `enabled:false` + disabled diagnostic),
   `filter-field`/`filter`/`filter-value`/`filter-chevron`, `tasks`
   list + `tasks-view` viewport, footer text, viewport size.
4. ~43 batched `detail` reads: every label text, both action
   commands, enabled/disabled states, selection surfaces, focus ring
   (`border_token:focus` on the list), placeholder vs value text.
5. `click btn-toggle` with a self-chosen `--expect-rev` from its own
   map — unprompted rev discipline.
6. Saw `{"tasks":"pressed"}` with unchanged `app_rev` and flagged
   its own result **ambiguous** instead of claiming success, then
   re-read button/list/filter/row state to verify.
7. Verified the toggle through evidence: row-0 icon flipped
   `activity` → `list-checks` (control: row-2 icon unchanged).
8. `click tasks-row-1-icon` with `--expect-rev`; verified selection
   moved row 0 → row 1 via surface changes.
9. Re-read the full protocol packet mid-task to resolve a harness
   framing confusion (see §4), then continued within bounds.

## Independent verification (supervisor, separate reads, same window)

- Map: 43 nodes, 8 rows. Row-0 icon `list-checks` (done marker
  present — the operator's Toggle genuinely applied). Row-1
  `surface=accent` (selected). Filter value = placeholder text.
  `btn-clear` `enabled=False`. Footer `Tasks · HAMESH 036`.
  `btn-toggle` present and enabled.
- (a) Purpose: accurate. (b) Inventory: accurate and complete,
  including the disabled state with its diagnostic. (c) State:
  accurate (8 rows with labels, row 0 done + row 1 selected,
  no active filter, 1/8 inferred, honestly notes no progress
  widget exists). (d) Commands: correct CLI form with quoting
  guidance learned from its own error, `expect_rev` + STALE-retry
  documented, row-selection caveat stated.

## Assessment: genuine, not scripted

Evidence for genuine autonomous completion: unprompted rev
discipline; self-recovered shell quoting failure; self-flagged
ambiguous result followed by verification reads; residual
uncertainties stated rather than papered over; no implementation
leakage anywhere in the transcript (no source paths, no internals,
no workarounds — every inference is grounded in Eye output).
The operator understood the interface from the advertised surface
alone and completed a real state change through it.

Two honesty notes. (1) The log's node count (51) differs from my
independent count (43) by exactly 8 nodes; cause unresolved
(possibly a transient popup at its map time, possibly a counting
difference) — none of the verified claims depend on it. (2) The
operator read the studio packet mid-task while diagnosing harness
confusion; read-only, self-disclosed, and outside its brief — see §4.

## Weaknesses found (ranked, all verified above)

1. **Revisions are structural, not monotonic** (highest value).
   `App::new` starts `revision` at 1 and `refresh()` rebuilds the
   whole tree, so `app_rev` is a deterministic function of the
   compose path: identical trees share revs (observed: rev 45
   across toggle/select/press; rev moves only when node shape
   changes: 45/57/62 with popup transitions). Consequence: STALE
   correctly rejects bogus revs (proven) and catches shape changes,
   but same-shape concurrent mutations (e.g. two operators racing
   a selection-dependent Toggle) would NOT trip the rev check —
   single-threaded serialization saves ordering, but an operator
   can act on a selection it no longer holds. Tasks mitigates this
   with item-identity selection, not with revs. Recommendation:
   document the semantic honestly; consider a monotonic generation
   counter (cheap: bump on every applied mutation) if concurrent
   multi-operator use is ever claimed.
2. **Click stamps are opaque.** Every click returns
   `"tasks":"pressed"` (console/editor: `"console"/"editor":
   "pressed") regardless of outcome — the operator flagged this
   itself as unexplained. Recommend including the driver note in
   the stamp (the information already exists).
3. **No progress/count state in narrow viewports.** The model HAS
   `done_count` and the wide layout paints a progress bar, but at
   800x600 the rail is hidden and the Eye exposes no count —
   the operator correctly reported "no progress widget exists".
   Viewport-dependent Eye coverage gap.
4. **Ambient harness context can misdirect an operator.** The
   worker framed itself as a Studio job with acceptance mechanics,
   demanded repo artifacts its packet forbade, and invoked
   stop-loss — all from harness-context cues, not the task. It
   correctly refused to violate its packet and asked for a ruling
   instead. Lesson for operator harnesses: keep mission framing out
   of the operator's context, or expect this failure mode. (No
   Hamesh change implied.)

## Residual unknowns

- Selection-dependence of toggle verified once (row 0); second
  selection not cross-verified (operator's own caveat, accepted).
- The 43-vs-51 node count discrepancy (above).
- Whether the streaming stall (see report §10) can strike an
  operator mid-task; not observed in this probe (no streaming run).

## Verdict for 037

An independent AI operator, given only the advertised interfaces,
discovered the app, understood its controls and states, and
completed a real state change — observed end-to-end, verified
independently. Genuine. The framework's Eye/Hand contract held up;
the weaknesses above are the highest-value findings, in order.
