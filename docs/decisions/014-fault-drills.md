# 014 — Standing fault drills: diagnose, repair, verify

Date: 2026-10-21. Status: in CI (`cargo run -p hamesh-faults` must exit 0).
Source: `apps/hamesh-faults`, `.github/workflows/ci.yml`.

## Why

The Eye/Hand promise is only real if an agent can repeatedly meet a NEW
broken control and fix it. Ad-hoc debugging (decisions 003, 009–011)
proved the capability once; these three drills keep it proven on every
commit, headless and deterministic.

## The three faults

1. **covered-submit** — transparent overlay over the submit button.
   Eye: topmost map node is the veil (z=100). Hand: aimed-at-btn hits
   veil, reported as intercepted. Repair: remove the veil. Verify:
   the same click lands and runs the command.
2. **clipped-composer** — composer widened past the window (w=2000).
   Eye: `detail` reports `clipped_by_parent` with a reachability
   warning. Repair: constrain the width. Verify: clipping cleared,
   diagnostics empty.
3. **dead-submit** — enabled button, no command bound. Eye: hit is
   correct. Hand: click lands but reports no effect, state unchanged,
   diagnostic names the missing command. Repair: rebind. Verify: the
   same click advances state; the typed path is unaffected.

## Drill contract (each fault)

DISCOVER (compact map) → REPRODUCE (failed interaction + evidence) →
LOCATE (detail diagnostics + hit-test truth) → REPAIR (the tree edit a
source change would compile to) → VERIFY (interaction succeeds, state
changed). Any unverified step exits non-zero.

## External (separate-process) version

The headless drills prove the shared Eye/Hand semantics the CLI drives.
The manual external equivalent for a live window:

```
hamesh map                                  # discover
hamesh click submit                         # reproduce (hit=veil?)
hamesh detail submit                        # locate
# ... repair in source, rebuild, restart with --inspect ...
hamesh wait <old_rev>                       # block until the new build lands
hamesh click submit                         # verify
hamesh snapshot target/fixed.png            # visual evidence
```

`wait` + `expect_rev` make the repair loop race-free: the agent never
acts on a stale tree, and the snapshot binds the claim to a frame.
