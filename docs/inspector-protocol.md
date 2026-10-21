# Inspector protocol (Eye / Hand / Memory) — v0.1.0

Machine-readable, versioned, local-only. All schemas are `serde` JSON.

## Eye: `hamesh-inspect`

- `compact_map(app, layout, scene) -> CompactMap`
  `{ app_revision, scene_revision, viewport_w/h, nodes[] }` where each node
  is `{ id, role, name, rect, visible, clipped, enabled, z }`.
  Progressive disclosure: fetch the map first, then details on demand.
- `detail(app, layout, id) -> ComponentDetail | None`
  `{ id, role, name, enabled, visible, effectively_visible, clipped_by_parent,
  rect?, z?, props, state_keys, diagnostics[] }`.
  Diagnostics are observations (`disabled`, `invisible via ancestor`,
  `rect extends beyond parent`), never invented causes.

## Hand: `hamesh-automation`

- `click_human(app, layout, x, y) -> AutomationResult` — hit-test at a point,
  verify enabled/visible, route through the owner's command.
- `click_component_human(app, layout, id) -> AutomationResult` — move to the
  component center, click, and verify the hit winner is the aim target;
  otherwise report `intercepted or occluded` with the actual winner.
- `invoke_direct(app, command_id) -> AutomationResult` — typed capability
  without hit-testing. Always `mechanism: DirectOp`.
- `AutomationResult`
  `{ mechanism, action, target, success, hit_component?, revision_before,
  revision_after, notes[] }`. Tests and traces must record `mechanism`.

## Memory: `hamesh-diag`

- `Trace::new(capacity)` bounded ring; `record(kind, detail, app_revision,
  frame_ref?)`; `export_json()`.
- `FrameRef { scene_revision, app_revision, width, height }` binds every
  screenshot and every automation result to exact revisions.

## Evidence bundle (demo)

`cargo run -p hamesh-demo` emits:
`target/hamesh-frame.png`, `target/hamesh-frame-2.png`,
`target/hamesh-map.json`, `target/hamesh-trace.json` —
map + frames + trace share `app_revision`/`scene_revision` so a second
agent can reproduce the failure and verify the repair.
