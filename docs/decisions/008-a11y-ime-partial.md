# 008 — Accessibility + IME: both PARTIAL, proceed with eyes open

Date: 2026-10-21. Status: accepted as integration constraints for M3.
Source: detached net-child brief (free-tier opencode, live repo/issue
evidence, 59 lines) + verified versions.

## AccessKit Windows — PARTIAL (core works, 3 deliberate stubs)

Implemented: `ITextRangeProvider` (Clone/Compare/Move/GetText/
GetBoundingRectangles/ScrollIntoView…), `ITextPatternProvider`
(GetSelection/RangeFromPoint). Landed via PR #145 (issue #34, 2023).
Caret movement, selection, granularity, value readout: work.

Stubs: `FindText` (in-field find-next broken), `RangeFromChild`
(no embedded objects), `GetVisibleRanges` (null), formatting
attributes deferred. Fresh churn: #836 (run-boundary panic) and #837
(`to_line_index` loop), both filed 2026-10-20 — **pin versions, do not
track master**. App contract #578 still open (TextRun fields to honor).

## winit Windows IME — PARTIAL (IMM32 only)

`winit-win32/src/ime.rs` is pure IMM32; zero TSF code, zero TSF issues.
Positioning OK for Microsoft/standard IMEs; **Sogou broken (#2780,
still open)**; preedit cursor fixed (#3967); **regression #4508 open
(`KeyboardInput` during composition → double-insert risk)**; no
surrounding text (needs TSF).

## Consequences for Hamesh M3

- a11y: adopt AccessKit + accesskit_winit (0.34.1/0.35.1 era, re-verify
  pairing at integration), implement the #578 TextRun contract, test
  NVDA + JAWS + Narrator separately (Narrator differs).
- IME: gate `KeyboardInput` while composition is active (#4508 defense),
  feed real caret position on every edit (winit tracks none), test
  Microsoft Pinyin + Sogou + Korean on Win10/Win11.
- Neither gap blocks starting M3; both are verified in the smoke tests
  or they do not ship.
