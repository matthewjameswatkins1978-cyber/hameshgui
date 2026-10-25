# 035 gallery evidence

Headless proof that the reusable list and picker work in a real
composition. Each size runs the same scripted drill through the
consumer driver (`apps/hamesh-035-lab`): pointer select + hover,
wheel scroll, keyboard nav + type-ahead + paging, Tab focus move,
picker open + filter + erase + commit + dismiss, canvas headline in
the picked font. Every step asserts; any failure panics the run.

## Frames

Per size, four moments: `*-1-initial`, `*-2-scrolled`, `*-3-picker-open`,
`*-4-picked`. All twelve ship here; the JSON beside each size carries
the Eye report, probes, selection, font, scroll and timings.

## What the Eye says (final state, all sizes)

`clipped=[] off_viewport=[] overlaps=0` — with one designed-for
exception class, enforced by the drill: a partially scrolled edge row
clips honestly against its viewport (at most 2 row-groups per step,
`layers-row-*` or `fonts-opt-*` only, counted as rows not nodes).

## Responsive behaviour (application budgeting, framework primitives)

- `>=1000px`: inspector present (248/200px). `<1000px`: inspector
  dropped, rail narrows 208→168. State and actions survive; only
  panels shed.
- Short viewports (`<620px`): picker popup caps at 5 rows instead of
  8; layers viewport shrinks and scrolls (10 rows at 820×560).
- Headline display size tiers 64/44/40 so the picked family stays on
  one line (a fixed 64px wrapped mid-phrase on narrow canvases —
  caught by looking, fixed by tiering, verified by re-looking).
- The layers viewport snaps to whole rows: resting states never park
  a partial row across the footer caption. Mid-scroll partials are
  transient and clipped honestly.
- The driver budgets in two passes (measure chrome, then grant the
  layers viewport what remains) instead of hand-arithmetic: the first
  attempt at arithmetic budgets left the footer 4px off-viewport and
  the Eye named it.

## Defects the Eye/drill caught during construction

1. Composer emitted zero rows (count synced nowhere for struct-literal
   specs) — fixed by making `list()` own the count sync; empty rail
   on first screenshot led straight to it.
2. `scroll_y` applied on the measuring pass leaked scroll into the
   driver — fixed by running pass 1 on a throwaway widget clone.
3. Row identity props were named `list`=index while the driver read
   `row` — press hit chrome. Fixed by writing both `list` (control)
   and `row` (index); blocks test pins the contract.
4. Probe topmost is the label/icon leaf, not the row — assertions now
   climb the tree instead of matching id prefixes.
5. Partial edge rows flag themselves plus icon/label leaves — the
   drill counts rows, not nodes.

## Where Hamesh eliminated application work

The gallery driver is ~200 lines of intent mapping (hit→row, key→nav,
focus→flag) over components that already know how to behave. No row
windowing, reveal math, filtering, commit semantics, popup measuring,
scroll clamping or type-ahead state anywhere outside the framework.
The installed-font picker lists real system families through
`installed_family_names` — no directory scans, no fallback button.
