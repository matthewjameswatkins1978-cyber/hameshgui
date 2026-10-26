//! hamesh-editor: a small file editor as a reusable composition.
//!
//! File lines live in a `ListView` (virtualized, keyboard-navigated),
//! the selected line edits in a `Composer` (grapheme-correct, IME-ready),
//! and a status strip plus Save button round out the chrome — all built
//! ONLY from Blocks over driver-owned state. Pointer and keyboard route
//! through the shared `Hit` router and `Key` core; this driver owns no
//! parent climbing and no key enum, exactly like tasks and console.
//!
//! Byte fidelity is a correctness property here, not polish: open
//! records the file's line-ending style and trailing-newline state, and
//! save reproduces both, so a clean open/save round-trips byte-identical.

use std::path::{Path, PathBuf};

use hamesh_blocks as b;
use hamesh_core::{App, Component, Role};
use hamesh_widgets::{list_nav, Composer, Hit, Key, ListView};

pub const ROW_H: f32 = 28.0;

/// Actions in focus order. Tab walks them; Enter runs the focused one.
pub const ACTIONS: [&str; 1] = ["save-file"];

pub struct Editor {
    pub lines: Vec<String>,
    pub list: ListView,
    pub composer: Composer,
    /// Line text as it was when editing began (Escape restores it).
    edit_base: String,
    pub path: Option<PathBuf>,
    /// Last saved (or opened) content. Dirty means lines differ.
    saved: Vec<String>,
    /// Original line-ending style and trailing newline, reproduced on save.
    crlf: bool,
    trailing_newline: bool,
    /// 0 = lines, 1 = edit, 2 = actions.
    pub focus: usize,
    pub action_idx: usize,
    hover_action: Option<String>,
}

impl Editor {
    pub fn new() -> Self {
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let mut list = ListView::new(ROW_H);
        list.set_count(0);
        Self {
            lines: Vec::new(),
            list,
            composer: Composer::new(),
            edit_base: String::new(),
            path: None,
            saved: Vec::new(),
            crlf: false,
            trailing_newline: false,
            focus: 0,
            action_idx: 0,
            hover_action: None,
        }
    }

    /// Open a UTF-8 text file. Records content, style and saved state;
    /// selects the first line and loads it into the composer.
    pub fn open(&mut self, path: &Path) -> Result<String, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let text =
            String::from_utf8(bytes).map_err(|_| format!("not UTF-8: {}", path.display()))?;
        self.crlf = text.contains("\r\n");
        self.trailing_newline = text.ends_with('\n');
        self.lines = text.lines().map(|l| l.to_string()).collect();
        self.saved = self.lines.clone();
        self.path = Some(path.to_path_buf());
        self.focus = 0;
        self.action_idx = 0;
        self.sync_list();
        if self.lines.is_empty() {
            self.list.clear_selection();
            self.load_composer();
        }
        Ok(format!(
            "opened {} ({} lines)",
            path.display(),
            self.lines.len()
        ))
    }

    /// Save back to the open file, reproducing the original line-ending
    /// style and trailing newline. Refused with a note when clean or
    /// when no file is open — through every input path alike.
    pub fn save(&mut self) -> Result<String, String> {
        if !self.save_enabled() {
            return Err(if self.path.is_none() {
                "save-file disabled: no file open".to_string()
            } else {
                "save-file disabled: no changes".to_string()
            });
        }
        let path = self.path.clone().unwrap();
        let sep = if self.crlf { "\r\n" } else { "\n" };
        let mut out = self.lines.join(sep);
        if self.trailing_newline {
            out.push_str(sep);
        }
        std::fs::write(&path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        self.saved = self.lines.clone();
        Ok(format!(
            "saved {} ({} lines)",
            path.display(),
            self.lines.len()
        ))
    }

    pub fn save_enabled(&self) -> bool {
        self.path.is_some() && self.lines != self.saved
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn char_count(&self) -> usize {
        self.lines.iter().map(|l| l.chars().count()).sum()
    }

    fn sync_list(&mut self) {
        self.list.set_count(self.lines.len());
        if self.lines.is_empty() {
            self.list.clear_selection();
        } else {
            let row = self.list.selected().unwrap_or(0).min(self.lines.len() - 1);
            self.list.select(row);
        }
        self.load_composer();
    }

    fn load_composer(&mut self) {
        match self.list.selected().and_then(|r| self.lines.get(r)) {
            Some(line) => {
                self.composer.text = line.clone();
                self.composer.caret = line.chars().count();
                self.edit_base = line.clone();
            }
            None => {
                self.composer.text.clear();
                self.composer.caret = 0;
                self.edit_base.clear();
            }
        }
        self.composer.preedit = None;
    }

    /// Write the composer back into the selected line.
    fn commit_line(&mut self) {
        if let Some(row) = self.list.selected() {
            if let Some(line) = self.lines.get_mut(row) {
                *line = self.composer.text.clone();
            }
        }
    }

    fn rows(&self) -> Vec<b::select::ListRow> {
        self.lines
            .iter()
            .map(|l| b::select::ListRow::new(l))
            .collect()
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

fn layout_of(app: &App, w: f32, h: f32) -> hamesh_layout::LayoutOutput {
    hamesh_layout::compute(app, w, h)
}

fn compose_with(e: &mut Editor, width: f32, list_h: f32) -> App {
    e.list.set_focused(e.focus == 0);
    let rows = e.rows();
    e.list.set_count(rows.len());
    let dirty = e.lines != e.saved;
    let (badge_text, variant) = if dirty {
        ("Modified", b::Variant::Warning)
    } else {
        ("Saved", b::Variant::Neutral)
    };
    let path_text = e
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "no file".to_string());
    let counts = format!("{} lines · {} chars", e.line_count(), e.char_count());
    let hover_save = e.hover_action.as_deref() == Some("save-file");
    let save_state = if !e.save_enabled() {
        b::ButtonState::Disabled
    } else if hover_save {
        b::ButtonState::Hover
    } else {
        b::ButtonState::Rest
    };
    let save_focused = e.focus == 2 && e.action_idx == 0;
    let edit_focused = e.focus == 1;
    let mut app = App::new("editorapp");
    {
        let mut root = b::Scope::root(&mut app, "editorapp");
        b::row(
            &mut root,
            "header",
            b::LayoutOpts {
                pad: 12.0,
                gap: 8.0,
                ..Default::default()
            },
            |s| {
                b::title(s, "app-title", "Editor");
                b::spacer(s, "header-space");
                b::badge(s, "dirty-badge", badge_text, variant);
                b::button(
                    s,
                    "btn-save",
                    "Save",
                    b::ButtonOpts::new(b::ButtonVariant::Primary, "save-file")
                        .with_state(save_state)
                        .focused(save_focused),
                );
            },
        );
        b::row(
            &mut root,
            "subheader",
            b::LayoutOpts {
                pad: 12.0,
                gap: 8.0,
                ..Default::default()
            },
            |s| {
                b::caption(s, "path-label", &path_text);
                b::spacer(s, "sub-space");
                b::caption(s, "count-label", &counts);
            },
        );
        b::select::list(
            &mut root,
            "lines",
            b::select::ListSpec {
                rows: &rows,
                view: &mut e.list,
                viewport_h: list_h,
                row_h: ROW_H,
                empty_label: "No file open".to_string(),
                grow: None,
                viewport_w: None,
            },
        );
        // The edit field mirrors composer truth the way sync_composer
        // does for M3: text, caret, preedit and focus are Eye-readable.
        // (Owned strings bind first: with_prop takes &str.)
        let caret_x = e.composer.caret_x(width - 48.0);
        let shown = e.composer.shown_text();
        let caret_s = format!("{caret_x:.1}");
        let preedit = e.composer.preedit.clone().unwrap_or_default();
        let focused_s = edit_focused.to_string();
        let mut field = Component::new("edit-field", Role::TextInput, "edit")
            .with_prop("h", "36")
            .with_prop("text", &shown)
            .with_prop("caret_x", &caret_s)
            .with_prop("preedit", &preedit)
            .with_prop("focused", &focused_s);
        if edit_focused {
            field
                .props
                .insert("border_token".to_string(), "focus".to_string());
            field.props.insert("border".to_string(), "2".to_string());
        } else {
            field
                .props
                .insert("border_token".to_string(), "border".to_string());
            field.props.insert("border".to_string(), "1".to_string());
        }
        app.add("editorapp", field);
    }
    app
}

/// Single-pass driver budgeting: chrome above and below the list
/// measures 188px at 800x600 (header 60 + gaps + subheader 48 +
/// edit field 36 + root insets); the list gets the rest.
pub fn refresh(e: &mut Editor, w: f32, h: f32) -> (App, hamesh_layout::LayoutOutput) {
    let list_h = (h - 188.0).max(96.0);
    let app = compose_with(e, w, list_h);
    let layout = layout_of(&app, w, h);
    (app, layout)
}

/// Run the save command through the shared disabled gate: pointer and
/// keyboard refuse identically when clean or fileless.
fn act(e: &mut Editor, command: &str) -> String {
    match command {
        "save-file" => match e.save() {
            Ok(note) => note,
            Err(note) => format!("{note}: no action"),
        },
        other => format!("unknown command {other}"),
    }
}

/// Pointer press through the SHARED router: hit-test, resolve with
/// `Hit::of`, drive the widgets. Clicking a row selects it and hands
/// the operator the composer, ready to type.
pub fn press(e: &mut Editor, app: &App, scene: &hamesh_scene::Scene, x: f32, y: f32) -> String {
    match hamesh_scene::hit_test(app, scene, x, y) {
        None => "press hit nothing".to_string(),
        Some(id) => match Hit::of(app, &id) {
            Hit::ListRow { row, .. } => {
                if row >= e.lines.len() {
                    return format!("row {row} out of range (no action)");
                }
                e.focus = 0;
                e.list.select(row);
                e.load_composer();
                // Selected means editable: hand over the composer at once.
                e.focus = 1;
                format!("selected line {row} (editing)")
            }
            Hit::Action { command } => {
                e.focus = 2;
                act(e, &command)
            }
            Hit::Chrome(hit) => format!("hit {hit} (chrome, no action)"),
            other => format!("hit {other:?} (no editor intent)"),
        },
    }
}

pub fn hover(e: &mut Editor, scene: &hamesh_scene::Scene, x: f32, y: f32, app: &App) -> bool {
    let Some(id) = hamesh_scene::hit_test(app, scene, x, y) else {
        e.hover_action = None;
        return false;
    };
    if let Hit::Action { command } = Hit::of(app, &id) {
        e.hover_action = Some(command);
        return true;
    }
    if let Hit::ListRow { .. } = Hit::of(app, &id) {
        if let Some(r) = scene
            .nodes
            .iter()
            .find(|n| n.id == "lines-view")
            .map(|n| n.rect.clone())
        {
            return e.list.hover_at_viewport(y - r.y);
        }
    }
    e.hover_action = None;
    false
}

pub fn wheel(
    e: &mut Editor,
    layout: &hamesh_layout::LayoutOutput,
    x: f32,
    y: f32,
    dy: f32,
) -> bool {
    let over = |id: &str| layout.nodes.get(id).is_some_and(|n| n.rect.contains(x, y));
    if over("lines-view") {
        e.list.wheel(dy)
    } else {
        false
    }
}

pub fn key(e: &mut Editor, k: Key) -> String {
    if matches!(k, Key::Tab | Key::ShiftTab) {
        // Three zones: lines, edit, actions. No popup to dismiss.
        let fwd = matches!(k, Key::Tab);
        (e.focus, e.action_idx) = match (e.focus, fwd) {
            (2, true) if e.action_idx + 1 < ACTIONS.len() => (2, e.action_idx + 1),
            (0, false) => (2, ACTIONS.len() - 1),
            (2, false) if e.action_idx > 0 => (2, e.action_idx - 1),
            (f, true) => ((f + 1) % 3, 0),
            (f, false) => ((f + 2) % 3, 0),
        };
        return format!("focus -> {}", ["lines", "edit", "actions"][e.focus]);
    }
    if e.focus == 0 {
        if let Some(moved) = list_nav(&mut e.list, &k) {
            e.load_composer();
            return format!("lines nav: {moved}");
        }
        match k {
            // Enter hands over the composer; Char both enters edit mode
            // and inserts, so type-to-edit never drops a keystroke.
            Key::Enter => {
                e.focus = 1;
                "focus -> edit".to_string()
            }
            Key::Char(c) if !c.is_control() => {
                e.focus = 1;
                e.composer.insert(&c.to_string());
                e.commit_line();
                format!("edit type: {c:?}")
            }
            Key::Escape => "lines: nothing to dismiss".to_string(),
            _ => "lines: key ignored".to_string(),
        }
    } else if e.focus == 1 {
        match k {
            Key::Char(c) if !c.is_control() => {
                e.composer.insert(&c.to_string());
                e.commit_line();
                format!("edit type: {c:?}")
            }
            Key::Backspace => {
                e.composer.backspace();
                e.commit_line();
                "edit erase".to_string()
            }
            Key::Delete => {
                e.composer.delete_forward();
                e.commit_line();
                "edit delete".to_string()
            }
            Key::Left => {
                e.composer.move_left();
                "caret left".to_string()
            }
            Key::Right => {
                e.composer.move_right();
                "caret right".to_string()
            }
            Key::Home => {
                e.composer.home();
                "caret home".to_string()
            }
            Key::End => {
                e.composer.end();
                "caret end".to_string()
            }
            // Commit is a no-op on content (edits are live) but returns
            // focus to the lines, symmetric with entering edit mode.
            Key::Enter => {
                e.focus = 0;
                "line committed".to_string()
            }
            // Cancel restores the line as it was when editing began.
            Key::Escape => {
                if let Some(row) = e.list.selected() {
                    if let Some(line) = e.lines.get_mut(row) {
                        *line = e.edit_base.clone();
                    }
                    e.load_composer();
                }
                e.focus = 0;
                "edit cancelled".to_string()
            }
            _ => "edit: key ignored".to_string(),
        }
    } else {
        match k {
            // Enter runs the focused action through the same gate as a
            // pointer press: disabled means disabled, everywhere.
            Key::Enter => act(e, ACTIONS[e.action_idx]),
            Key::Escape => "actions: nothing to dismiss".to_string(),
            _ => "actions: pointer or Enter".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(content: &str, crlf: bool, trailing: bool) -> (TempFile, PathBuf) {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "hamesh-editor-test-{}-{}.txt",
            std::process::id(),
            rand_suffix()
        ));
        let mut out = content.replace('\n', if crlf { "\r\n" } else { "\n" });
        if trailing {
            out.push_str(if crlf { "\r\n" } else { "\n" });
        }
        std::fs::write(&p, &out).unwrap();
        (TempFile(p.clone()), p)
    }

    struct TempFile(PathBuf);
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn rand_suffix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        format!("{n}-{n:x}")
    }

    fn frame(
        e: &mut Editor,
        w: f32,
        h: f32,
    ) -> (App, hamesh_layout::LayoutOutput, hamesh_scene::Scene) {
        let (app, layout) = refresh(e, w, h);
        let scene = hamesh_scene::build(&app, &layout, 1);
        (app, layout, scene)
    }

    fn center_of(scene: &hamesh_scene::Scene, id: &str) -> (f32, f32) {
        let n = scene.nodes.iter().find(|n| n.id == id).unwrap();
        (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0)
    }

    #[test]
    fn open_save_roundtrips_bytes_exactly() {
        for (crlf, trailing) in [(false, true), (true, true), (false, false)] {
            let content = "first\nsecond line\n\nfourth";
            let (_g, p) = fixture(content, crlf, trailing);
            let expected = std::fs::read(&p).unwrap();
            let mut e = Editor::new();
            let note = e.open(&p).unwrap();
            assert!(note.contains("4 lines"), "{note}");
            assert_eq!(e.lines.len(), 4);
            // Save untouched: bytes must be identical.
            e.save().unwrap_err();
            let note = e.save().unwrap_err();
            assert!(note.contains("no changes"), "{note}");
            e.composer.insert("x");
            e.commit_line();
            e.save().unwrap();
            let after = std::fs::read(&p).unwrap();
            assert_ne!(after, expected);
            // Restore and prove exactness: reopen, clear the mark.
            e.lines[0].pop();
            e.save().unwrap();
            assert_eq!(std::fs::read(&p).unwrap(), expected);
        }
    }

    #[test]
    fn edit_flow_marks_dirty_and_saves() {
        let (_g, p) = fixture("alpha\nbeta\ngamma", false, true);
        let mut e = Editor::new();
        e.open(&p).unwrap();
        assert!(!e.save_enabled());
        assert_eq!(e.list.selected(), Some(0));
        let _ = key(&mut e, Key::Down);
        assert_eq!(e.list.selected(), Some(1));
        assert_eq!(e.composer.text, "beta");
        let _ = key(&mut e, Key::Enter);
        assert_eq!(e.focus, 1);
        let _ = key(&mut e, Key::Char('X'));
        assert_eq!(e.lines[1], "betaX");
        assert!(e.save_enabled());
        let _ = key(&mut e, Key::Escape);
        assert_eq!(e.lines[1], "beta");
        assert!(!e.save_enabled());
        // Re-edit and save through the keyboard gate.
        let _ = key(&mut e, Key::Enter);
        let _ = key(&mut e, Key::Char('!'));
        e.focus = 2;
        e.action_idx = 0;
        let note = key(&mut e, Key::Enter);
        assert!(note.contains("saved"), "{note}");
        assert!(!e.save_enabled());
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "alpha\nbeta!\ngamma\n"
        );
    }

    #[test]
    fn tab_walks_zones_and_save_gate_refuses() {
        let (_g, p) = fixture("one\ntwo", false, true);
        let mut e = Editor::new();
        e.open(&p).unwrap();
        assert_eq!((e.focus, e.action_idx), (0, 0));
        let _ = key(&mut e, Key::Tab);
        assert_eq!((e.focus, e.action_idx), (1, 0));
        let _ = key(&mut e, Key::Tab);
        assert_eq!((e.focus, e.action_idx), (2, 0), "save button");
        let _ = key(&mut e, Key::Tab);
        assert_eq!((e.focus, e.action_idx), (0, 0), "wraps to lines");
        let _ = key(&mut e, Key::ShiftTab);
        assert_eq!((e.focus, e.action_idx), (2, 0));
        // Clean: Enter on Save refuses honestly, pointer too.
        e.focus = 2;
        let note = key(&mut e, Key::Enter);
        assert!(note.contains("disabled"), "{note}");
        let (app, _layout, scene) = frame(&mut e, 800.0, 600.0);
        let (x, y) = center_of(&scene, "btn-save");
        assert!(press(&mut e, &app, &scene, x, y).contains("disabled"));
    }

    #[test]
    fn tree_is_eye_clean() {
        let (_g, p) = fixture("lorem ipsum\ndolor sit", false, true);
        let mut e = Editor::new();
        e.open(&p).unwrap();
        for (w, h) in [(800.0, 600.0), (1280.0, 800.0)] {
            let (app, _layout, scene) = frame(&mut e, w, h);
            for id in [
                "app-title",
                "dirty-badge",
                "btn-save",
                "path-label",
                "count-label",
                "lines",
                "edit-field",
            ] {
                assert!(app.get(id).is_some(), "missing {id} at {w}x{h}");
            }
            for n in &scene.nodes {
                assert!(!n.clipped, "{} clipped at {w}x{h}", n.id);
            }
        }
        // Focus ring follows the edit zone.
        e.focus = 1;
        let (app, _, _) = frame(&mut e, 800.0, 600.0);
        let field = app.get("edit-field").unwrap();
        assert_eq!(
            field.props.get("border_token").map(|s| s.as_str()),
            Some("focus")
        );
        assert_eq!(field.props.get("border").map(|s| s.as_str()), Some("2"));
    }

    #[test]
    fn press_selects_and_hands_over_composer() {
        let (_g, p) = fixture("aaa\nbbb\nccc", false, true);
        let mut e = Editor::new();
        e.open(&p).unwrap();
        let (app, _layout, scene) = frame(&mut e, 800.0, 600.0);
        let (x, y) = center_of(&scene, "lines-row-2");
        let note = press(&mut e, &app, &scene, x, y);
        assert!(note.contains("selected line 2"), "{note}");
        assert_eq!(e.list.selected(), Some(2));
        assert_eq!(e.composer.text, "ccc");
        assert_eq!(e.focus, 1, "ready to type");
    }

    #[test]
    fn type_to_edit_never_drops_a_keystroke() {
        let (_g, p) = fixture("abc", false, false);
        let mut e = Editor::new();
        e.open(&p).unwrap();
        // Typing with the list focused enters edit mode AND inserts at
        // the caret (end of the loaded line) — nothing is dropped.
        let note = key(&mut e, Key::Char('Z'));
        assert!(note.contains("edit type"), "{note}");
        assert_eq!(e.focus, 1);
        assert_eq!(e.lines[0], "abcZ");
    }
}
