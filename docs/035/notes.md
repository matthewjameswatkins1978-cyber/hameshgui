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

## Final review repairs (PR #5 follow-up)

1. **Anchored popup.** The first gallery composed the popup in flow,
   pushing content down. The popup is now an overlay (zero parent
   footprint, z=100 paint layer) positioned by `place_popup` +
   `popup_geom`: prefer below, flip above, shrink to the roomier
   side, clamp into the viewport, size to the widest match. The
   driver calls it after layout; the Eye asserts chrome rects are
   identical between closed and open frames (no reflow, verified).
2. **Hit-test agrees with paint.** `hamesh_scene::hit_test` is the
   single targeting truth: topmost visible node whose own paint
   clip contains the point. Scrolled-out rows, hidden nodes and
   ghosts cannot intercept; partially visible rows stay clickable;
   overlays keep intercepting. `click_human` builds the scene
   internally, so Specimen and the window inherit the fix with no
   API change. The Eye's probe `hit` is its own clip-aware
   topmost candidate.
3. **Second-opinion findings (Xiaomi MiMo 2.6 Flash, bounded).**
   Fixed: thumb-clamp panic on viewports < 24px; typeahead
   match-on-current now stays instead of restarting the buffer;
   picker row-pitch sync (`set_row_h` preserves selection/scroll)
   after finding a live 30-vs-32px pointer drift; list root pad for
   focus-ring clearance; placeholder and hover-icon tokens raised to
   6.48/AA. New: `hamesh_text::ellipsize` (grapheme-correct) + the
   list width contract (`viewport_w` → trim to remaining chrome,
   including the 16px text inset the measurer adds).
4b. **Driver extracted, no behavior change.** The drill driver now
   lives in its own `hamesh-gallery` crate (same intents a native loop
   forwards: pointer, wheel, keys), so a real window can drive the
   identical code paths. The lab binary is the drill only, and the
   `--gallery` window mode drives them through a real winit loop.
   Verified with genuine OS input (SendInput, Eye-aimed at map rects):
   click selected a layer, Tab moved focus, typed text filtered the
   popup, a scancode Down moved the highlight (VK-only arrows arrive
   as Unidentified), Enter committed the second match, F12 dumped the
   Eye map. `gal-native-800x600.png` shows the payoff in a real window.
   All three
   sizes still report zero Eye exceptions.
4. **Narrow-row collision fixed with evidence.** The 820px rail showed
   option text painting into the scrollbar (Eye: overlap). Rows now
   ellipsize to their measured budget; the 820 frame is clean with
   zero exceptions (`clipped=[] off_viewport=[] overlaps=0`).
