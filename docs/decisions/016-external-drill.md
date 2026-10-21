# 016 — Standing external-CLI regression drill (stale-inject → repair → submit)

Status: accepted (proven live 2026-10-21, `DRILL-PASS`).

## Context

Decisions 013/015 built the Eye/Hand path and proved it by hand. Hand-run
proof rots. The project needs a standing drill an unfamiliar agent can
re-run: drive the live window through the inspector protocol only, hit the
classic outside-operator mistake on purpose, repair it the way the protocol
prescribes, and verify — deterministically, with no model in the loop.

## Decision

- New binary `apps/hamesh-drill` (local-only, never CI: opens a real
  window). Run from the workspace root: `cargo run -p hamesh-drill`.
  Exit 0 + `DRILL-PASS`, else nonzero + `DRILL-FAIL <reason>`.
- The drill builds both ends, spawns
  `hamesh-window-demo --m3 --inspect --smoke-test=60` (no `--pi`: fully
  deterministic), waits for discovery, then:
  1. `map` → rev R1, composer present;
  2. `type composer "external drill"` → R2 > R1;
  3. `key Enter --expect-rev R1` → must fail exit 2 / `STALE` (the fault);
  4. re-`map` → R3 == R2 (rejected op changed nothing; the repair is a
     fresh rev);
  5. `key Enter --expect-rev R3` → ok, R4 > R3;
  6. `snapshot` → PNG exists and non-empty;
  7. after the smoke clock, the window's own trace must contain an
     `m3.key` record with `submitted` + the drill text (independent
     channel, not the protocol replies).
- One-routing fix this drill forced (same turn): the `m3.key` trace
  record moved from the winit wrapper into `m3_keys_apply`, so winit,
  inspector, smoke, and tests all leave the same shape. The inspector
  `ext.key` record stays as the transport-level envelope.

## Evidence

- Live run: `DRILL-PASS stale-inject -> repair -> submit verified
  (revs 8->9->9->10)`; snapshot `target/hamesh-drill-snapshot.png`.
- Gates unchanged: `cargo fmt --check`, `clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace` all green.

## Consequences

- The Eye/Hand loop is now regression-covered end to end without a human
  watching: stale handling, resubmission, and content landing are all
  machine-checked.
- The drill takes ~75s wall time (waits out the smoke clock for the
  trace dump); keep it out of CI and out of the default workflow.
