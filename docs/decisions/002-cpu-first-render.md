# 002 — CPU render first (tiny-skia), GPU next

Date: 2026-10-21. Status: accepted (revisitable).

Context: The mission defaults to GPU-first (wgpu/Vello), but M1/M2 must be
provable headless/CI-friendly from the first slice, and GPU dependency
research was still running.

Decision: ship a `tiny-skia 0.11` CPU backend behind compatible scene
semantics now; define the CPU-vs-GPU `Tolerance` comparison model up front
(per-channel tolerance + outlier fraction, exact pixel equality explicitly
rejected). Add wgpu/Vello presentation once researcher briefs land and a
native window exists.

Consequences: every frame is reproducible in CI; GPU work must satisfy the
pre-declared tolerance model instead of redefining correctness later.
