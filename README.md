# Hamesh — the Eye and the Hand

An AI-first native GUI framework in Rust. First platform: Windows.
First real application: Agent X / Pi GUI.

Guiding concept: **See. Understand. Act. Prove it worked.**

Hamesh makes inspection and interaction native capabilities of the framework:
the Eye (machine-readable layout, semantics, screenshots) and the Hand
(human-equivalent event injection plus labeled direct operations) with
the Memory (bounded diagnostic traces and replay evidence).

## Status (v0.1.0, Phase 1 slice)

- [x] M1 headless: retained component tree → Taffy Flex layout → scene →
      CPU frame (tiny-skia) with PNG capture (`cargo run -p hamesh-demo`).
- [x] M2 headless: stable IDs, compact map + component detail, hit-testing,
      human-equivalent click vs labeled direct op, fault drills
      (disabled button, overlay intercept, clipping diagnostics).
- [ ] M1 native window (winit + GPU via wgpu/Vello): in research.
- [ ] Text editing foundation (Parley / COSMIC-Text evaluation): in research.
- [ ] Accessibility bridge (AccessKit): planned.

## Workspace

- `crates/hamesh-core` — canonical component model, identity, revisions.
- `crates/hamesh-layout` — Taffy layout + visibility/clipping truth + hit-test.
- `crates/hamesh-scene` — resolved visual representation (paint order, colors).
- `crates/hamesh-render` — CPU renderer + PNG capture + CPU/GPU tolerance model.
- `crates/hamesh-inspect` — compact map + component detail (the Eye).
- `crates/hamesh-automation` — human-equivalent vs direct-op injection (the Hand).
- `crates/hamesh-diag` — bounded diagnostic trace ring (the Memory).
- `apps/demo` — sample interface + Eye/Hand fault drills + evidence output.

## Build, test, run

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo run -p hamesh-demo
```

Evidence outputs (gitignored build artifacts):
`target/hamesh-frame.png`, `target/hamesh-frame-2.png`,
`target/hamesh-map.json`, `target/hamesh-trace.json`.

## Docs

- `docs/architecture.md` — core model and crate responsibilities.
- `docs/inspector-protocol.md` — Eye/Hand/Memory schemas and drill catalog.
- `docs/decisions/` — dependency and architecture decision records.

## License

MIT OR Apache-2.0.
