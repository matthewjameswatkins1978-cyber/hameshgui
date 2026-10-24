# 033 — visual foundations, evidence

An independent composition (`apps/hamesh-033-lab`, an ops console — not
Specimen) rendered headlessly at three sizes. Every claim below was checked
with `hamesh-inspect` telemetry after looking at the pixels.

Regenerate: `cargo run -p hamesh-033-lab -- target/033`

## Artifacts

| Size | Frame | Report |
|---|---|---|
| 1440x900 | `ops-1440x900.png` | `ops-1440x900.json` |
| 1100x720 | `ops-1100x720.png` | `ops-1100x720.json` |
| 820x560 | `ops-820x560.png` | `ops-820x560.json` |

All three: `clipped=[] off_viewport=[] overlaps=0`, and every probe resolves
a point to a geometric candidate set with an explicit coverage reason.

## Framework defects this composition exposed

1. **Icons crushed to zero height.** A declared `h=20` inside an overflowing
   column produced a 20x**0** box; the icon painted as a dot. Cause:
   `flex_shrink: 1.0` for every non-text node. Fixed in `hamesh-layout`:
   declared `w`/`h` are floors, so boxes overflow (named by the Eye) instead
   of collapsing.
2. **Equal-z paint order was unspecified.** Same-layer siblings painted in
   HashMap order, so the same app could show a different topmost node between
   runs. Fixed in `hamesh-scene`: order is `(z, tree pre-order)`.
3. **Escaped content painted outside its panel.** Now impossible: the scene
   computes an effective clip per node and both renderers honour it.

## Review repairs

**Narrow width (820x560).** The alert column used to disappear, leaving
`DEGRADED` with no explanation. Now the header badge carries the count
(`DEGRADED · 3 ALERTS`), a compact strip carries the highest-priority alert
and a working escalation action, and the feed drops two secondary rows to
pay for it. See `responsive-priority.md` for the generalised principle.

**Rendering correctness.**
- Clipping is now *mask-based*, not rect-clamped. Clamping reshaped content:
  it invented a border along the clip edge, moved rounded corners, and
  re-positioned icons. The mask cuts pixels, so geometry is untouched.
- CPU pixel regressions: `border_strokes_cannot_escape_the_ancestor_clip`,
  `clipped_icon_keeps_its_placement_and_only_loses_pixels` (inside the clip
  the glyph is identical to an unclipped reference within a 2-unit masked
  blend tolerance; outside it is background).
- GPU: `build_vscene` emits a vello clip layer per overflowing node and
  reports the count on `GpuStats::last_clipped_layers`; the contract is
  tested without a device (`overflowing_nodes_emit_clip_layers`).
  **Pixel parity on real GPU hardware is outstanding**: no adapter is
  available in this environment, so the GPU path is verified at the encoder
  level only. This is recorded rather than claimed.

**Visual Mapper accuracy.** `probe` now states exactly what it guarantees.
It returns *geometric* candidates ordered by `(z, tree order)`, each with a
`coverage` reason (`paint_candidate`, `outside_ancestor_clip`, `ghost`,
`hidden`), and the topmost field is `topmost_candidate` — not `topmost`. It
does not sample pixels: a rounded corner or glyph silhouette can leave a
candidate with no ink at that coordinate. Exact geometry for text stays in
`detail(...).text_geom` (ink, trim, advance, baseline). A pixel-identification
engine was deliberately not built.

## My visual pass (three passes, then stopped)

Pass 1 (first render) — problems found by looking:
- orphan dots where icons should be (measured `20x0`);
- the Event feed card was ~500px of bordered void;
- "Escalate to on-call" sat ~700px below the alerts it belongs to;
- the title sat 8px left of the column edge beneath it.

Fixes: size floor in the framework; tile height from measured content;
responsive collapse of the side column and throughput card; escalate action
clustered with its alerts; an 8px strut aligning the wordmark. Passes 2 and 3
confirmed each fix; frames above are the result.

## Remaining limitations (honest)

- GPU **pixel** parity is unverified (encoder-level only), as above.
- At 900px tall the centre column keeps ~340px of free background below the
  feed: a scrolling feed would fill it, which is a feature, not a layout bug.
- A too-short tile now clips its label and says so, rather than crushing the
  icon. Preferred, but it moves the fix into composition.
- Narrow layouts summarise alerts but cannot expand the full list in place
  (no disclosure control in Blocks yet); the summary names the count and the
  top item, and the action works.
