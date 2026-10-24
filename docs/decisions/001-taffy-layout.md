# 001 — Layout via Taffy 0.15

Date: 2026-10-21. Status: accepted (revisitable).

Context: Hamesh needs Flex/Grid without reinventing layout. Candidates were
a hand-rolled flexbox vs adopting Taffy.

Decision: depend on `taffy 0.15` for constraint resolution; Hamesh owns the
component-to-style mapping (`w`/`h`/`flex_grow`/`layout` props), absolute
geometry accumulation, visibility/clipping truth and hit-testing.

Consequences: Taffy API drift is quarantined in `hamesh-layout`; scene,
render, inspect and automation only consume `LayoutOutput`.
