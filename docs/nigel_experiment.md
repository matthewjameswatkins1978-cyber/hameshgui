# Smallest NIGEL-on-Hamesh Experiment: Architecture & Implementation Proposal

**Target Repository**: `d:\Projects\NIGEL`  
**Purpose**: Proposal for the smallest useful first-application experiment on Hamesh.  
**Constraint**: Preparation only. Do NOT initiate migration until approved.

---

## 1. Objective of the First Experiment

The objective is to validate Hamesh as a practical, high-performance desktop framework for NIGEL using the smallest viable vertical slice: a single-window native desktop document editor.

The experiment will demonstrate:
1. **One Real Editable Document**: Backed by NIGEL's existing document model and text buffer.
2. **Standard Menu Commands**: File (New, Open, Save, Exit), Edit (Undo, Redo, Cut, Copy, Paste), and View (Themes).
3. **Working Keyboard & Clipboard**: Arrow key navigation, character insertion/deletion, Ctrl+C/Ctrl+V clipboard integration.
4. **Multiline Text & Scrolling**: Text wrapping and vertical scroll offset within the editor viewport.
5. **Correct Focus & Visual Identity**: Tab focus switching and immediate profile switching across Hamesh's visual worlds (defaulting to **Technical Workstation** for telemetry/engineering or **Editorial Paper** for reading/writing).

---

## 2. Capability Matrix: What Hamesh Supplies vs. What Must Be Implemented

| Capability | Status | Hamesh Engine / NIGEL Implementation |
|---|---|---|
| **Window Creation & Event Loop** | **Supplied** | `hamesh-shell`: Win32/winit event loop, softbuffer blit, HiDPI scaling |
| **Command Palette & Menus** | **Supplied** | `hamesh-widgets`: `MenuBar`, `MenuItem`, `CommandRegistry` with hotkeys |
| **Status Bar & Telemetry** | **Supplied** | `hamesh-widgets`: `StatusBarControl` with dynamic profile and vitals |
| **Theme System (5 Worlds)** | **Supplied** | `hamesh-theme`: `VisualWorld` (Technical, Modern, Editorial, Classic, Retro) |
| **Text Layout & Rendering** | **Supplied** | `hamesh-text`: Parley/Swash font system with Arc-cached layouts |
| **Sub-millisecond Pipeline** | **Supplied** | `hamesh-layout` + `hamesh-render`: Taffy flexbox + tiny-skia rasterization |
| **Multi-line Text Input Buffer** | **Supplied** | `Role::TextInput` handles typing, Backspace, Delete, and Caret tracking |
| **OS Clipboard Integration** | **To Implement** | Integrate `arboard` (or Win32 clipboard API) for Copy/Cut/Paste |
| **Multi-line Text Selection** | **To Implement** | Visual selection range rendering (`selection_start` .. `selection_end`) |
| **Document Scroll Clamp** | **To Implement** | Vertical scroll offset clamping with mouse-wheel and scrollbar handle |
| **NIGEL Model Binding** | **To Implement** | Adapter syncing NIGEL's document struct to Hamesh component properties |

---

## 3. Concrete Implementation Plan

### Step 1: Add Direct Dependencies to NIGEL (`Cargo.toml`)
```toml
[dependencies]
hamesh-core = { path = "../hameshgui/crates/hamesh-core" }
hamesh-shell = { path = "../hameshgui/crates/hamesh-shell" }
hamesh-widgets = { path = "../hameshgui/crates/hamesh-widgets" }
hamesh-theme = { path = "../hameshgui/crates/hamesh-theme" }
arboard = "3" # Native clipboard support
```

### Step 2: Define `NigelDocumentApp` Struct
```rust
use hamesh_core::{App, Component, Role};
use hamesh_shell::{HameshApp, ShellContext, ShellConfig};
use hamesh_theme::VisualWorld;
use hamesh_widgets::desktop::{CommandRegistry, FocusRing, MenuBar, StatusBarControl};

pub struct NigelDocumentApp {
    world: VisualWorld,
    commands: CommandRegistry,
    menu_bar: MenuBar,
    status_bar: StatusBarControl,
    focus_ring: FocusRing,
    
    // Existing NIGEL Document Model (retained intact)
    document_title: String,
    document_body: String,
    caret_offset: usize,
    scroll_top: f32,
    is_dirty: bool,
}
```

### Step 3: Implement Native Lifecycle (`HameshApp`)
- `init`: Build initial menu bar, document editor panel, and status bar.
- `on_key_down`:
  - Route shortcuts (`Ctrl+S`, `Ctrl+C`, `Ctrl+V`, `Ctrl+Z`) through `CommandRegistry`.
  - Route navigation keys (Arrows, Home, End, PageUp, PageDown) to text model.
- `on_mouse_scroll`: Adjust `scroll_top` clamped between 0 and maximum document height.
- `rebuild_ui`: Sync active document text and cursor position to Hamesh's component tree.

---

## 4. Acceptance Criteria for the Experiment

1. Launching NIGEL opens an 1100x750 native window in under 30ms cold start.
2. Typing, backspacing, and multiline editing execute with <1ms in-memory latency per keystroke.
3. Menus (File, Edit, View) open with Alt hotkeys and mouse clicks.
4. Copying text to Windows clipboard and pasting into Notepad works flawlessly.
5. Switching between **Technical Workstation** and **Editorial Paper** preserves all typed text, cursor positions, and dirty state.
