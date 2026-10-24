# 006 — GPU presentation: wgpu 30 + vello 0.11 behind the same Scene

Date: 2026-10-21. Status: accepted (spike proven on RX 5700 XT, Windows).

Context: M1 demands GPU-first rendering. All signatures below were read
from the vendored registry sources (`cargo tree` + `src/*.rs`), not from
memory or the research briefs.

Decisions:
- `wgpu = "30"`, `vello = "0.11"`, `pollster = "1"`. `cargo tree -i wgpu`
  proves ONE shared `wgpu 30.0.1` for workspace + vello — brief risks #1/#4
  closed with repository evidence.
- Reuse vello's own `util::{RenderContext, RenderSurface}` helper (surface
  creation, intermediate Rgba8Unorm target, `TextureBlitter`) instead of
  hand-rolling wgpu state. Scene translation is 1:1 (one rounded rect per
  visible Hamesh node); `RenderParams` carries the same viewport.
- `crates/hamesh-window --gpu` selects the path; init failure falls back
  to CPU with a trace record (never a silent swap).
- Failure model implemented, not just documented:
  - `CurrentSurfaceTexture` (wgpu 30 returns an enum, not a Result):
    Success/Suboptimal present; Timeout/Occluded skip the frame;
    Outdated/Lost reconfigure + retry once.
  - `Device::set_device_lost_callback` EXISTS (brief question #6 answered:
    yes) — wired to an mpsc channel checked each frame; a lost device
    yields `GpuError::DeviceFailed`, and the caller drops GPU state for
    the CPU path.

Verification (observed): `--gpu --smoke-test=8` — renderer created,
"GPU first frame: 7 nodes presented", 435 frames in 8s, Eye map + trace
written. `cargo test` green, `clippy -D warnings` clean, `fmt` clean.

GPU-vs-CPU comparison against the Tolerance model (decision 002) is still
open: the two paths render the same Scene but pixel-diff harnessing is
not yet built. Text is still unshaped bars on both paths.
