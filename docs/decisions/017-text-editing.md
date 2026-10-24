# 017 — Text editing evaluation: graphemes now, IME next

Status: accepted (shipped with grapheme fix + 4 fixture tests).

## Context

The composer stored text as `String` with a char-indexed caret: memory-safe
(no byte-index panics) but grapheme-wrong. Agents paste arbitrary Unicode
(code, emoji, CJK, combining marks); a composer that splits ZWJ sequences
or skin-tone emoji on backspace corrupts user text visibly. This turn
evaluated the editing core against adversarial fixtures and fixed what was
small and safe.

## Findings

- Char-based backspace/delete/arrows split multi-char graphemes: the ZWJ
  family (5 chars, 1 grapheme), 👋🏽 (2 chars), flags (2 RI chars),
  e + combining acute (2 chars). Mid-sequence carets also break shaping
  runs downstream (parley shapes what's there; dangling ZWJ renders tofu).
- Insert and caret-pixel measurement were already safe: insertion never
  splits, and `caret_x` lays out the char prefix through parley, so it
  stays exact under shaping, fallback, and ligatures.
- `unicode-segmentation 1.13.3` verified (`cargo info`): MIT OR
  Apache-2.0, rust-version 1.85, no_std-capable — fits the license and
  footprint policy. Reuse, not invention.

## Decision

- Keep char-indexed storage and caret (stable, cheap, no API churn);
  snap deletions and arrow moves to grapheme boundaries computed on
  demand (`grapheme_bounds` + `char_range_bytes`, O(n) per keystroke —
  fine for a composer, same standing note as caret layout).
- Parley remains the shaping/measure path; no editing adapter, no second
  text model. One shaper, boundaries from UAX #29.
- Fixture tests lock it: whole-grapheme backspace (family, combining,
  tone), whole-grapheme arrows, forward delete, mid-flag insert/walk.
- Explicitly NOT this slice: IME Preedit/Commit (needs marked-text
  rendering + caret interplay — a design piece, queued next), selection /
  clipboard, multiline, undo. The composer stays single-line; submission
  semantics unchanged.

## Evidence

- 4 new `hamesh-widgets` tests pass; full workspace gates green
  (fmt, clippy `-D warnings`, all tests).
- The one genuine test failure during the slice was my own fixture
  arithmetic (family is 5 Rust chars, not 7) — implementation was
  correct; fixed the test, not the code.

## Consequences

- Backspace/arrows can never strand half a grapheme again, on any input
  path (winit, inspector, smoke, tests share `Composer`).
- IME remains the next text piece: wire winit `Ime::{Preedit, Commit}`
  with visible marked text rather than commit-only (commit without
  preedit display leaves CJK users typing blind).
