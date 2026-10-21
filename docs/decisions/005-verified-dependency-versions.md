# 005 — Verified dependency versions (Oct 2026, live crates.io)

Date: 2026-10-21. Status: accepted. Source: `cargo info` run by the
supervisor session (live crates.io index) — this is repository-grade
evidence, not model memory. It closes the verification checklists in
research briefs task-1.md and task-2.md (subagent artifacts, same job).

## Resolved table

| Crate | Verified version | License (verified) | Notes |
|---|---|---|---|
| winit | 0.30.13 stable (0.31.0-beta.3 latest) | Apache-2.0 | **Settles the briefs' license dispute in task-2's favor.** Spike pins `winit = "0.30"`. |
| wgpu | 30.0.1 | MIT OR Apache-2.0 | Single shared version with vello 0.11 (probe `cargo tree -i wgpu`) — brief risks #1/#4 closed. |
| vello | 0.11.0 | Apache-2.0 OR MIT | GPU path next. |
| vello_cpu | 0.3.0 | Apache-2.0 OR MIT | **Published — the brief's fear was wrong.** CPU fallback available. |
| vello_common | 0.3.0 | Apache-2.0 OR MIT | Published. |
| taffy | 0.15.0 | MIT | Matches workspace pin; already building. |
| parley | 0.12.0 | Apache-2.0 OR MIT | Text path A (with fontique + swash). |
| cosmic-text | 0.19.0 | MIT OR Apache-2.0 | Text path B. Decision between A/B still open. |
| accesskit | 0.25.1 | MIT OR Apache-2.0 | A11y provider. |
| accesskit_winit | 0.34.1 | Apache-2.0 | winit pairing to confirm at integration time (`cargo tree`). |
| fontdb | 0.24.0 | (see crates.io) | Only needed if path B. |
| rustybuzz | 0.20.1 | (see crates.io) | Only needed if path B. |
| softbuffer | 0.4.8 | MIT OR Apache-2.0 | Already presenting the native window. |
| pollster | 1.0.1 | Apache-2.0/MIT | For wgpu adapter/device `block_on` at startup. |

## Standing instructions for pins

No version above goes into `Cargo.toml` from memory — but the table above
*is* now verified evidence, so pins may reference it. At integration time,
`cargo tree -i <crate>` must still confirm single-version unification
(wgpu, raw-window-handle, winit pairings).

## Open (not version questions)

- Text path A (parley) vs B (cosmic-text): needs a rendering-glue spike, not more reading.
- AccessKit Windows TextPattern support and winit Windows IME backend state:
  verify against issue trackers + a Narrator/Pinyin smoke test before M3.
- Briefs' durable architecture guidance (device-loss rebuild, surface
  reconfigure on resize/DPI, Fifo pacing) stands and will be cited by the
  GPU-presentation implementation.
