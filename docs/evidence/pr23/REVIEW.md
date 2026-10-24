# Finding 3 — visual review evidence and disposition

Rendered from one run, identical content, viewport 800x600 and app state
(`voice_proof` tests): `cabinet.png`, `folio.png`, `grid.png`.
Title advances: Cabinet 77.6 / Folio 100.7 / Grid 82.8.

## Model reviews attempted

- **Argon (opencode-go/gemini-4-argon-contributor)**: stood down by Matthew
  (expensive). Two calls made: a probe (vision OK) and a full brief whose
  reply was unattributable (a three-variant verdict to a one-line preamble
  with one image attached) plus instruction violations (scores and
  redesigns despite explicit bans) and a misread of Cabinet as light serif.
  Not recorded as evidence.
- **DeepSeek V4.1 Flash (free)**: full blind brief answered. Mixed quality —
  see claim disposition below. No scores were requested; none given.
- **MiniMax M3 (explicitly authorised)**: full blind brief answered.
  Well-behaved (no scores, honest scoping, admits limits) but shares the
  Cabinet-as-serif misread, so its treatment-specific judgements are not
  recorded as validation. Its process observations (alignment discipline,
  redundant encoding) match independent measurement.

Free-tier vision census (all probed with the Cabinet PNG): vision-capable —
deepseek-v4.1-flash-free, glm-5.3-flash-free, mimo-v2.6-flash-free,
nemotron-3.5-super-vl-free. Text-only (honest refusals) — beam-free,
flint-40b-free, nemotron-3-ultra-free, nemotron-3.5-lightning-free.

## Claim disposition (measured, not voted)

1. **"Mixed label faces (Agents mono vs Tasks/Errors sans)" — FALSE.**
   All three metric labels share one builder and identical
   `font`/`font_vars` props resolving to IBM Plex Mono
   (`font_resolved=true`). Same inputs through a deterministic shaper
   cannot yield different faces. Direct zoom confirms uniform mono
   letterforms.

2. **"Row 3 loses right alignment (~25px inset)" — FALSE.**
   Eye geometry, all three profiles: right edges 763.2 / 763.6 / 763.4
   (test `activity_rows_share_one_right_edge`). The shorter string
   ("0 errors · 1h ago") was misread as misaligned; its right edge
   coincides with the others to 0.4px.

3. **"Button 'de sign' kerning gap" — FALSE at shaping level.**
   Per-glyph advances for "Inspect design" at 13px:
   e→s = 7.23px, identical to its neighbours (7.23 / 7.23 / 6.50).
   No kerning anomaly; the small-size impression is antialiasing noise.
   Button is unvoiced in all profiles, so this cannot discriminate
   between treatments either way.

4. **"Muted captions under AA contrast" — TRUE (shared, pre-existing).**
   12px captions render in the muted token at **4.24:1** solid-color
   contrast on the card surface (measured from rendered pixels;
   token math agrees), below WCAG AA 4.5:1 for normal text. Affects
   all three treatments identically; predates profiles. Fixing it
   means re-tokening muted text everywhere — a design-wide visual
   change left for Matthew/Lucy, not made unilaterally here.

5. **"~90px void below the activity rows" — TRUE as measurement.**
   Content ends ~y470, card continues to ~y590, identically in all
   three renders. Shared-composition fact, not a profile defect;
   changing content now would break evidence identity. Taste-adjacent:
   fixed-height card vs sized-to-cap is a design call.

## Model reviews: outcome

All three candidate critics were tried against the same blind brief
(identical PNGs, no aesthetic steer, defects-vs-taste required,
no scores, no redesigns). None produced a trustworthy
treatment-level critique:

- **Argon (paid)**: stood down by Matthew (cost). Two calls made:
  a probe (vision OK) and a full brief whose reply was
  unattributable (a three-variant verdict to a one-line preamble
  with one image attached) plus instruction violations and a
  Cabinet-as-light-serif misread. Not recorded as evidence.
- **DeepSeek V4.1 Flash (free)**: answered the full brief. Mixed:
  no scores (complied), but recommended merging treatments
  (redesign-adjacent) and misdescribed Cabinet ("light/regular
  weight" numerals — they are Barlow Condensed Bold). Its
  treatment judgements are unreliable; its two measurable shared
  claims verified out (contrast, void — see above).
- **MiniMax M3 (explicitly authorised paid call)**: best-behaved
  (no scores, honest scoping, asks for what it cannot see) but
  shares the Cabinet-as-serif misread, so its treatment
  verdicts are not recorded as validation. Its process
  observations (alignment discipline, redundant encoding)
  match independent measurement.

Conclusion: vision-language critics at this tier cannot reliably
tell the treatments apart at 800x600 and confabulate specifics.
The objective half below stands on measurement instead; the
taste half remains human.

## Standing judgement

Objective half: closed by measurement above (1 falsified, 2 falsified,
3 falsified at shaping, 4 verified-shared-preexisting, 5 measured-shared).
No treatment-specific objective defect found in any of the three.
Taste (Matthew, 2026-10-23): all three accepted — "nice", differences
noted as subtle. Contrast token: accepted as-is for now (revisitable)
— no re-tokening; higher-impact work takes precedence.
