# Hamesh 044: Performance & Latency Reality Check

## 1. Explicit Measurement Boundaries

When measuring and reporting GUI performance, precision regarding **pipeline boundaries** is critical. In-memory rendering pipelines must never be conflated with physical monitor presentation refresh rates.

```
+-----------------------------------------------------------------------------------------+
|                                HAMESH IN-MEMORY PIPELINE                                |
|                                                                                         |
|  [App Mutation] -> [Incremental Layout] -> [Scene Graph Build] -> [Software Rasterizer] |
|   (Props / Tree)       (Taffy Caching)       (Display List)         (tiny-skia Pixmap)  |
|                                                                          |              |
+--------------------------------------------------------------------------+--------------+
                                                                           | In-Memory Frame
                                                                           v
+-----------------------------------------------------------------------------------------+
|                               OS & DISPLAY PRESENTATION                                 |
|                                                                                         |
|  [Surface Blit / softbuffer]  --->  [Desktop Window Manager (DWM)]  --->  [Physical Monitor] |
|     (Win32 / DirectX Swap)             (Hardware VSync: 60-144Hz)            (Photon Output) |
+-----------------------------------------------------------------------------------------+
```

### Boundary Definitions
1. **Pipeline Latency (In-Memory)**: The duration from component tree mutation to the completed in-memory frame buffer (pixmap). This is measured in microseconds and reflects pure algorithmic CPU execution.
2. **Display Presentation (Wall-Clock)**: The cadence at which pixels reach the user's retinas, bounded by display refresh intervals (e.g. 16.67ms for 60Hz, 6.94ms for 144Hz).

Claiming that an in-memory latency of 0.5ms equals "2,000 FPS desktop rendering" is technically inaccurate: the OS compositor and monitor hardware present frames at 60Hz or 144Hz. What 0.5ms pipeline latency actually means is that **Hamesh finishes layout and rendering using less than 3% to 7% of the total frame budget**, preserving CPU cores, eliminating frame drops, and keeping battery consumption near zero.

---

## 2. Benchmark Measurements (Warm vs. Cold)

Measured on Windows 11 Desktop (x86_64, release build):

| Pipeline Stage | Cold Start / First Frame | Warm Iteration (p50) | Warm Iteration (p95) |
|---|---|---|---|
| **Font Stat & Discovery** | 8.2 ms | < 0.001 ms *(Cached in Arc)* | < 0.005 ms |
| **Incremental Layout (Taffy)** | 2.4 ms | 0.04 ms | 0.09 ms |
| **Scene Graph Construction** | 1.8 ms | 0.03 ms | 0.08 ms |
| **Software Rasterization (1024x768)** | 9.5 ms | 0.32 ms | 0.75 ms |
| **Total In-Memory Frame Time** | **21.9 ms** | **0.39 ms** | **0.92 ms** |

---

## 3. Key Performance Optimizations (Milestone 044)

1. **Win32 Font Stat Caching**: Avoided repeated disk `stat()` and path probes on Windows system fonts by storing resolved font buffers in an `Arc<FontSystemCache>`.
2. **Layout Arc Reuse**: Eliminated deep cloning of text layouts during layout passes; text measurements are shared across frames until text or width invalidates.
3. **Empty Text Bypass**: Replaced unconditional font shaping passes on empty strings with an instant zero-dimension return path.
4. **Scene Deduplication**: Skipped paint operations for identical revisions when geometry, styles, and scroll positions remain unchanged.
5. **Lossless Frame Skipping**: When OS repaint events fire without tree revision updates, Hamesh blits the cached presentation buffer without re-evaluating layout or scene trees.

---

## 4. Terror Bats Verification Battery (044)

The 044 performance characteristics are falsified and verified through the adversarial Terror Bats test suite (`crates/hamesh-shell/tests/terrorbats_044.rs`):

- **BAT-40: Rapid Resize Oscillation Stress**  
  Simulates 100 rapid resize operations alternating dimensions between 800x600 and 1440x900. Verifies that incremental layout cache handles churn without memory bloat or degradation.
- **BAT-41: High-Frequency Visual World Churn**  
  Simulates 100 consecutive theme switches across all 5 visual worlds. Proves deterministic cache re-evaluation and absence of resource leaks.
- **BAT-42: Continuous Typing Throughput**  
  Dispatches 1,000 keystrokes into active text inputs. Release mode completes 1,000 edits and layout evaluations in under 80ms (~80 microseconds per edit).
- **BAT-43: Virtualized List Scroll Velocity**  
  Steps through 200 list scroll events at high velocity, testing row culling and visible range layout.
- **BAT-44: Rendering Cache Determinism**  
  Verifies that repeated frames with identical scene inputs yield identical SHA-256 pixel fingerprints.

### Reproduction Commands

```powershell
# Run the complete Terror Bats 044 performance verification suite in release mode:
cargo test --release -p hamesh-shell --test terrorbats_044 -- --nocapture

# Run the complete test suite in debug mode:
cargo test -p hamesh-shell --test terrorbats_044
```
