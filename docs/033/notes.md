# 033 — visual foundations, evidence

An independent composition (`apps/hamesh-033-lab`, an ops console — not
Specimen) rendered headlessly at three sizes. Every claim below was
checked with `hamesh-inspect` telemetry after looking at the pixels.

## Artifacts

| Size | Frame | Report |
|---|---|---|
| 1440x900 | `ops-1440x900.png` | `ops-1440x900.json` |
| 1100x720 | `ops-1100x720.png` | `ops-1100x720.json` |
| 820x560 | `ops-820x560.png` | `ops-820x560.json` |

`clipped=[] off_viewport=[] overlaps=0` at all three sizes, and every
probe resolves a point to its topmost component and hit target.

## Framework defects this composition exposed

1. **Icons crushed to zero height.** Declared `h=20` on an icon inside an
   overflowing column produced a 20x**0** box — the icon painted as a dot.
   Cause: `flex_shrink: 1.0` for every non-text node, so a declared size
   was a wish. Fixed in `hamesh-layout`: declared `w`/`h` are now floors
   (`min_size`), so boxes overflow — which the Eye names — instead of
   collapsing. Found by looking at the render, then measuring the rect.
2. **Equal-z paint order was unspecified.** Two same-layer siblings could
   paint in different orders between runs (HashMap iteration), so the
   same app could show a different topmost node. Fixed in `hamesh-scene`:
   order is `(z, tree pre-order)`. Found by a flaky test of my own
   (`sibling overlap must be reported` flipped direction), then proven
   stable over 8 consecutive builds.
3. **Escaped content painted outside its panel.** Now impossible: the
   scene computes an effective clip per node and the renderer bounds every
   fill/border/progress/icon/glyph by it. Pixel test:
   `escaped_content_cannot_paint_outside_its_panel`.

## My visual pass (three passes, then stopped)

Pass 1 (first render) — visible problems I found by looking:
- metric tiles showed an orphan dot where icons should be;
- the Event feed card was ~500px of bordered void;
- "Escalate to on-call" sat ~700px below the alerts it belongs to;
- the title sat 8px left of the column edge beneath it.

Actions: measured the icon rect (20x0) → framework floor fix; raised the
tile height to fit measured content (the Eye named the clipped labels);
made the side column and throughput card drop on narrow/short viewports
(the Eye reported them off-viewport); clustered the escalate action with
its alerts; added an 8px strut to align the wordmark with the column.
Pass 2 and 3 confirmed each fix and the final frames above.

## Remaining limitations (honest)

- At 820x560 the alert column is **dropped**, so alerts are simply absent.
  Better would be a header count that expands on demand. Not done.
- At 900px tall the centre column keeps ~340px of free background below
  the feed once the card hugs its content. A scrolling feed would fill it;
  that is a feature, not a layout bug.
- Metrics overflow honestly rather than crush, so a too-short tile now
  clips its label (and says so) instead of silently shrinking the icon.
  Preferred, but it moves the fix into composition.
