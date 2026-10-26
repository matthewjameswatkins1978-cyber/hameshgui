# HAMESH 037 — Experiment plan: prove it outside the laboratory

**Baseline (starting condition, accounted):** PR #7 is reviewed (APPROVE)
but **unmerged** (`OPEN`, head `8aab6be`). All 037 work runs on branch
`hamesh/036-beautiful-by-default @ 8aab6be`, which *includes* the keyboard
fix. `main` lacks it. Merge would change only which ref names the code
the operator exercises — the operator-visible behaviour under test is
identical either way. No result below depends on the merge.

## 1. Interface inventory (what the operator is allowed)

Advertised, documented, public (all in-repo, current branch):

- **Eye (read-only):** `hamesh map` → compact map
  `{app_rev, scene_rev, nodes[{id, role, name, rect, visible, clipped,
  enabled, z}]}`; `hamesh detail <id>` → live props/state/diagnostics.
  `hamesh snapshot [path]` → PNG bound to the exact revs.
- **Hand (mutating, rev-checked):** `hamesh click <id|x,y>`,
  `hamesh key <Key...>`, `hamesh type <id> <text>` — every op takes
  `--expect-rev`; a moved rev rejects with `STALE` (CLI exit 2),
  nothing changes. Clicks run the HumanEquivalent mechanism through
  the same press path as the physical mouse (Lucy's convergence rule,
  `hamesh-window` `apply_press`); keys run the shared key core.
- **Wait:** `hamesh wait [rev] [timeout_ms]` — observe change without polling.
- **Memory (witness, operator-forbidden until debrief):**
  `target/hamesh-window-trace.json` — rev-stamped trace the host uses
  to confirm or contradict the operator's claims.
- **Docs the operator may read:** `docs/inspector-protocol.md` and the
  `hamesh --help` usage line only. No crate sources, no trace files,
  no prior drive scripts (`work035/`, `taskdrive.ps1`), no decision docs.

**Specimen finding (shapes the experiment):** `D:/Projects/Specimen-app`
is a genuine Hamesh *kit* client (text/layout/render/inspect crates at
pinned rev `c5b794c`) but exposes **no** Eye/Hand inspector server — its
"inspector" is an in-app typography panel. The only live-operable app
behind the advertised interfaces is `hamesh-window`
(`--tasks/--gallery/--sheet/--m3` + `--inspect`). 037 therefore runs the
operator scenarios against the **Tasks window**; Specimen is assessed at
interface level in Phase 3 (a real client with no operator surface is
itself a finding).

## 2. Scenarios (specified before running)

Host: `cargo run -p hamesh-window -- --tasks --inspect` (stays open;
CLI ops need no window foreground). Ground truth of a fresh Tasks
window: 8 tasks, `sel=Some(0)`, filter closed, `done=0/8`.

### S1 — Discovery
Brief the operator: a task-management app runs here; its inspector is
on loopback (discovery file `target/hamesh-inspect.json`); CLI usage
string; nothing else. Task: report (a) the app's purpose in one
sentence, (b) inventory of interactive controls with ids, (c) current
state (tasks, selection, filter, progress), (d) the exact commands to
mark a task done.
**Accept:** (a)–(d) match ground truth (queue list + rows, filter
field/popup, `btn-toggle`, `btn-clear`, read-only detail rail; 8
tasks, row 0 selected, filter closed, 0/8; valid key/click command
sequences). **Fail:** invented controls, misread `enabled`, or
unexecutable commands.

### S2 — Execution (multistep, focus+input+nav+state)
Task: mark *"Write the final report"* done — filter the queue to find
it, select it, toggle it done, clear the filter so the full queue
returns. Rule: observe (map/detail) → act → verify through the Eye
before the next step; finish with a snapshot.
**Accept:** fresh Eye read shows task 7 done, `1/8`, query empty, 8
rows shown; step log shows one verify per act. Outcome compared
against existing native-SendInput drive traces (`docs/036/*stdout*`)
for native-vs-inspector equivalence — reuse, no new native driving.

### S3 — Recovery (stale + unexpected state, no privilege)
Fault (i): operator records `app_rev`; host performs one unrelated CLI
click (rev moves); operator's next mutating op carries the old
`--expect-rev` → must observe `STALE`/exit 2, re-map, retry with the
fresh rev and complete the step. Fault (ii): host pre-opens the filter
with a live query; operator's planned Toggle must notice the popup via
the Eye and handle it deliberately (proceed knowingly or Escape first).
**Accept:** STALE detected from the CLI reply alone, recovery uses
re-inspection only (no source reads, no trace peeking), transcript
shows the STALE→remap→retry loop with real rev numbers, task completes.

## 3. Independence protocol

Primary: a delegated subagent receives **only** the task brief, the CLI
usage string, the discovery-file path and `docs/inspector-protocol.md`
— no implementation details, no workarounds — and returns its full
transcript. The host verifies end state independently (own Eye reads +
trace witness) and scores the transcript against the criteria above.
Fallback (reported as a limitation, never as equivalent evidence): the
host runs the scenarios hands-tied. Scripted-looking success is scored
as failure of the scenario, not of the framework — the distinction is
the point of 037.

## 4. Evidence to collect (per scenario)

Successes, failures, interventions; every Eye observation and Hand
action with revs; trace excerpts as witness; ambiguous/misleading
state descriptions quoted verbatim; native-vs-inspector outcome
comparison; whether recovery used only exposed information.

## 5. Phase 3 / 4 method (no implementation in 037)

Phase 3: read decisions `008`/`019`, grep AccessKit integration points,
and test each mission question against S1–S3 observations (which a11y
info is missing? which text entry is unverified? is Eye semantics
enough? can desktop verification get more repeatable cheaply?).
Phase 4: rank observed failures by severity × users × architecture ×
cost; recommend **one** bounded follow-on milestone with an acceptance
test. Deliverables land in `docs/037/`: this plan, `evidence/`,
`report.md` (assessment + weaknesses + unknowns), one follow-on packet.
