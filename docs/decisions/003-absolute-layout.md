# 003 — Absolute geometry: accumulate, pin the root

Date: 2026-10-21. Status: accepted. Found by test evidence, not reasoning.

Symptom: `hamesh-inspect` test `map_and_detail` failed — a healthy button
reported `clipped_by_parent`. The demo showed a 800×600 viewport producing
a 72×32 root with children spilling out.

Root causes:
1. Taffy `layout.location` is parent-relative; we treated it as absolute.
2. Taffy `AvailableSpace::Definite(viewport)` does not force an auto-sized
   root to expand; our root defaulted to auto×32px.

Fix (`hamesh-layout::compute`): accumulate absolute offsets through
ancestors; pin the root style to `Dimension::length(viewport_w/h)`
explicitly; containers (Window/Panel) default to auto size with
`flex_grow = 1`, leaves default to 32px height.

Verification: `cargo test --workspace` (10 passed), `cargo run -p hamesh-demo`
shows `win (0,0 800x600)`, zero false clipping, all four Hand drills pass.
Lesson: layout truth must be asserted by failing tests + rendered PNGs,
never by reading the layout library's docs alone.
