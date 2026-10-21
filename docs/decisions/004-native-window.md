# 004 — Native window via winit 0.30 + softbuffer CPU presentation

Date: 2026-10-21. Status: accepted (spike proven, GPU next).

Context: M1 requires an actual Hamesh window, not just headless PNGs.
GPU dependency research was still running, so versions were pinned by
direct local evidence instead: `cargo search`/`cargo info` plus reading
the vendored sources in `~/.cargo/registry/src` for exact APIs
(`Context::new(display)`, `Surface::new(&ctx, window)`, winit 0.30
`ApplicationHandler`).

Decision:
- `winit = "0.30"` (stable line; 0.30.13 vendored — 0.31 was beta-only at
  the time, rejected for the spike).
- `softbuffer = "0.4"` (0.4.8 vendored) to present the tiny-skia frame.
  The same `hamesh_render::render(scene)` path feeds both the PNG capture
  and the window blit — no parallel semantics. Premultiplied RGBA is
  un-premultiplied into 0RGB u32 on blit.
- `crates/hamesh-window` owns only OS plumbing (window, surface, event
  routing into `hamesh-automation`, resize → re-layout). All clicks/keys
  route through the labeled Hand mechanisms.
- `--smoke-test=N` flag: open the real window, present for N seconds, dump
  map + PNG + trace, exit 0. Non-interactive proof the OS surface presented.

Verification (observed): real 800×600 window opened; 478 frames presented
in 6s; trace recorded genuine OS resizes (1904×984 → 800×600); map shows
7 nodes, all visible, zero clipped. Screenshot confirms structure but also
the known gap: text renders as solid bars — no glyph rasterization yet
(typography milestone pending researcher briefs).

Consequences: the window path is CPU-bound by design for now; wgpu/Vello
presentation must reuse `Scene` + `FrameRef` and satisfy the Tolerance
model from decision 002.
