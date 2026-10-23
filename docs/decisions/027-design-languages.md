# 027 — Three Hamesh design languages (Phase A skeleton)

Status: proposal. No code. Profiles are data, not engine changes;
applications select one by name and Hamesh resolves tokens.

Font facts verified 2026-10-22 against google/fonts METADATA.pb
(all OFL — adoptable, not merely learnable):
- **Fraunces** (Undercase Type): variable SOFT 0–100, WONK 0–1,
  opsz 9–144, wght 100–900. Expressive optical + idiosyncrasy axes.
- **Barlow Condensed** (Tribby): static 100–900 + italics.
  Industrial condensed grotesque, no variable axes in GF set.
- **IBM Plex Mono** (IBM/Bold Monday): static 100–700 + italics,
  incl. Cyrillic/Greek-adjacent coverage via latin-ext/cyrillic.
- **EB Garamond** (Duffner/Pardo): variable wght 400–800,
  broad language coverage (Greek/Cyrillic/Vietnamese).
- **Newsreader** (Production Type): variable opsz 6–72,
  wght 200–800. True optical sizes (6pt micro → 72pt display cuts).
- **Instrument Sans** (Fuenzalida/Egstad): variable wdth 75–100,
  wght 400–700. Width axis = the quiet-condensation tool.

## 1. The Unquiet Grid (modern, precise, quietly unconventional)

Visual grammar: strong 12-col-style structure, generous negative
space, one deliberate asymmetry per composition (off-grid anchor,
broken rule, oversized numeral). Restraint first; the quirk earns
its place or it goes.

Typography:
- Display: Instrument Sans, wdth 75–85, wght 600–700, tight
  tracking (−1 to −3%). Narrow + heavy = authority without bulk.
- Headline/supporting: Instrument Sans Regular/Medium at natural
  width. One family keeps the grid calm.
- Annotation/technical: IBM Plex Mono Regular/Medium. The mono
  signals instrumentality against the grotesque voice.
- Newsreader Italic sparingly for editorial counterpoint
  (pull-quotes, kickers) — the humanist break in the machine grid.

Why it works: two voices (grotesque + mono) plus one guest
appearance (italic serif). Width contrast, not weight shouting.

## 2. The Living Folio (classical, humanist, editorial)

Visual grammar: book proportions (margins wider than gutters),
drop hierarchy (title → standfirst → body → marginalia),
considered leading (1.4–1.6 body), delicate rules and folios.

Typography:
- Display: Newsreader at opsz 72, wght 600–700. Optical size does
  the expressive work — never fake it by scaling up 16pt cuts.
- Supporting/title: Newsreader opsz 16–24, wght 400–500.
- Body: Newsreader opsz 6–16 (or EB Garamond where its broader
  language coverage matters), leading 1.45+.
- Annotation/marginalia: IBM Plex Mono small, or EB Garamond
  italic for sidenotes. Technical vs literary margin voices.
- EB Garamond instead of Newsreader where Greek/Cyrillic text
  appears — coverage is a correctness concern, not a preference.

Why it works: optical sizes match the job (display cut for
display, text cut for text); the serif never shouts because the
size axis already gave it presence.

## 3. The Signal Cabinet (instrumental, technical, expressive)

Visual grammar: calibrated systems (grids, scales, tick marks),
bold display blocks, technical annotation as texture, one
expressive departure per piece (a Fraunces headline inside the
instrument panel, or a WONK-y detail).

Typography:
- Display: Barlow Condensed 600–800, uppercase, tight leading
  (0.9–1.0). Condensed bold = maximum signal per line.
  Alternative display: Fraunces Black opsz 144, SOFT 0, WONK 0–1
  for the expressive departure.
- Supporting: Barlow Condensed Regular/Medium or IBM Plex Mono
  for labels, readouts, specifications.
- Annotation: IBM Plex Mono — coordinates, versions, timings.
  The mono voice IS the cabinet's operating system.
- Fraunces italic for the human aside inside technical matter.

Why it works: condensed grotesque carries density; mono carries
precision; Fraunces carries the one human moment. Contrast is
structural (width + texture), not decorative.

## Cross-language rules

- Ordinary controls stay readable: UI chrome uses the
  application type ramp, never display faces at text sizes.
- Never set display type below its optical intent
  (72pt cuts don't do captions; 6pt cuts don't do posters).
- Fallback chain required: preferred → metric-compatible
  alternative → system font, reported (never silent tofu).
  Specimen owns the artwork's fonts; the workspace keeps its own.
- Mixing languages is allowed where justified; each mixture must
  state which language leads and why.

## Serialisation sketch (target shape, not yet implemented)

```json
{
  "language": "unquiet-grid | living-folio | signal-cabinet",
  "roles": {
    "display": {"family": "Instrument Sans", "axes": {"wdth": 80, "wght": 650}, "tracking": -0.02},
    "headline": {"family": "Instrument Sans", "axes": {"wdth": 100, "wght": 500}},
    "supporting": {"family": "Instrument Sans", "axes": {"wdth": 100, "wght": 400}},
    "body": {"family": "Newsreader", "axes": {"opsz": 16, "wght": 400}, "leading": 1.5},
    "annotation": {"family": "IBM Plex Mono", "axes": {"wght": 400}}
  },
  "departures": ["newsreader-italic-pullquote"]
}
```

Variable axes beyond wght (wdth/opsz/SOFT/WONK) need
fontdb/Parley axis support verified before any profile depends
on them — verified Phase B: shaping, measurement and the CPU
rasterizer all carry variation coords end to end (regression
tests prove spans track advances). The three profiles are live
as `Profile` consts in hamesh-blocks, voiced by OFL faces
vendored under `assets/fonts` (deterministic on every machine,
including CI). `font_style` resolves italic faces through font
matching (byte-identity regression test) and `tracking` shifts
advances by exactly glyphs × px; the Grid's Newsreader Italic
kicker is live on the dashboard. Substitution is never silent:
the Eye reports `font_resolved` per node — family-level only.
It means the named family was found; it does NOT guarantee the
requested style matched a real file (synthesis stays possible
for families without one) or that every codepoint is covered
(tofu stays possible). Style evidence comes from the
byte-identity test, not the flag.

Not yet wired: SOFT/WONK axes (parsed, untested on a live
face) and GPU variable outlines (vello 0.11 has no coords API:
positions exact, outlines default-instance — no parity claimed).
