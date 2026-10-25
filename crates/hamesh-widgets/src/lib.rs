//! hamesh-widgets: M3 foundations — composer, virtualized feed, focus.
//!
//! Behavior lives here, shared by the headless drills and the real window:
//! both drivers call `handle_key`. The Eye sees widget state through plain
//! component props (`text`, `caret_x`, `focused`); the Hand drives it
//! through `handle_key` (human path) or direct mutation (labeled DirectOp).

use hamesh_core::{App, Component, ComponentId, Role};
use unicode_segmentation::UnicodeSegmentation;

pub mod select;
pub use select::{ListView, Nav, Picker};

/// Char offsets of every grapheme boundary in `text`, including 0 and the
/// end. The caret stays char-indexed (cheap, stable across edits); all
/// deletions and arrow moves snap to these boundaries so multi-char
/// graphemes (ZWJ sequences, skin tones, flags, combining marks) are never
/// split. See decision 017.
fn grapheme_bounds(text: &str) -> Vec<usize> {
    let mut bounds = vec![0];
    let mut seen = 0;
    for g in text.graphemes(true) {
        seen += g.chars().count();
        bounds.push(seen);
    }
    bounds
}

/// Byte range of char offsets `[start, end)` in `text`.
fn char_range_bytes(text: &str, start: usize, end: usize) -> (usize, usize) {
    let byte_at = |n: usize| {
        text.char_indices()
            .nth(n)
            .map(|(i, _)| i)
            .unwrap_or(text.len())
    };
    (byte_at(start), byte_at(end))
}

/// Single-line text composer. Caret is a char-boundary index; its pixel
/// position is measured by laying out the text *before* the caret, so it
/// stays exact under shaping, fallback and ligatures (O(n) per keystroke —
/// fine for a spike, noted for M3 hardening).
#[derive(Clone, Debug, Default)]
pub struct Composer {
    pub text: String,
    pub caret: usize,
    pub focused: bool,
    /// Marked text being composed by an IME (Preedit): shown inline at
    /// the caret but not yet committed. Never submitted on its own;
    /// Commit (or submit) folds it into `text` first. See decision 019.
    pub preedit: Option<String>,
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }

    fn clamp_caret(&mut self) {
        let len = self.text.chars().count();
        self.caret = self.caret.min(len);
    }

    pub fn insert(&mut self, s: &str) {
        let idx = self
            .text
            .char_indices()
            .nth(self.caret)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        self.text.insert_str(idx, s);
        self.caret += s.chars().count();
        self.clamp_caret();
    }

    /// Ime::Preedit: empty string (or cleared) drops the marked text.
    pub fn set_preedit(&mut self, s: &str) {
        if s.is_empty() {
            self.preedit = None;
        } else {
            self.preedit = Some(s.to_string());
        }
    }

    /// Ime::Commit or submit-while-composing: fold marked text into the
    /// buffer through the grapheme-correct insert path, then clear it.
    pub fn commit(&mut self, s: &str) {
        self.preedit = None;
        if !s.is_empty() {
            self.insert(s);
        }
    }

    pub fn clear_preedit(&mut self) {
        self.preedit = None;
    }

    /// Displayed text: committed text with any live preedit spliced at
    /// the caret. The caret itself stays at the composition start, so
    /// `caret_x` (measured over the committed prefix) is unaffected.
    pub fn shown_text(&self) -> String {
        match &self.preedit {
            None => self.text.clone(),
            Some(p) => {
                let byte = self
                    .text
                    .char_indices()
                    .nth(self.caret)
                    .map(|(i, _)| i)
                    .unwrap_or(self.text.len());
                let (head, tail) = self.text.split_at(byte);
                format!("{head}{p}{tail}")
            }
        }
    }

    pub fn backspace(&mut self) {
        if self.caret == 0 {
            return;
        }
        // Delete the whole grapheme before the caret, not one char:
        // ZWJ sequences, skin tones, flags and combining marks must go
        // together or the text is left corrupted.
        let bounds = grapheme_bounds(&self.text);
        let prev = bounds.iter().rev().find(|b| **b < self.caret);
        match prev {
            Some(&start) => {
                let (sb, eb) = char_range_bytes(&self.text, start, self.caret);
                self.text.drain(sb..eb);
                self.caret = start;
            }
            None => self.caret = 0,
        }
    }

    pub fn delete_forward(&mut self) {
        let len = self.text.chars().count();
        if self.caret >= len {
            return;
        }
        let bounds = grapheme_bounds(&self.text);
        let end = bounds
            .iter()
            .find(|b| **b > self.caret)
            .copied()
            .unwrap_or(len);
        let (sb, eb) = char_range_bytes(&self.text, self.caret, end);
        self.text.drain(sb..eb);
    }

    pub fn move_left(&mut self) {
        let bounds = grapheme_bounds(&self.text);
        self.caret = bounds
            .iter()
            .rev()
            .find(|b| **b < self.caret)
            .copied()
            .unwrap_or(0);
    }

    pub fn move_right(&mut self) {
        let len = self.text.chars().count();
        let bounds = grapheme_bounds(&self.text);
        self.caret = bounds
            .iter()
            .find(|b| **b > self.caret)
            .copied()
            .unwrap_or(len);
    }

    pub fn home(&mut self) {
        self.caret = 0;
    }

    pub fn end(&mut self) {
        self.caret = self.text.chars().count();
    }

    /// Pixel x of the caret within the laid-out text (16px, given width).
    pub fn caret_x(&self, max_width: f32) -> f32 {
        let prefix: String = self.text.chars().take(self.caret).collect();
        if prefix.is_empty() {
            return 0.0;
        }
        hamesh_text::TextLayout::new(&prefix, max_width, 16.0, None).width()
    }

    pub fn take_submission(&mut self) -> Option<String> {
        // Submitting while composing commits first: the marked text is
        // real user input, never silently dropped.
        if let Some(p) = self.preedit.take() {
            self.insert(&p);
        }
        let t = self.text.trim().to_string();
        self.text.clear();
        self.caret = 0;
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    }
}

/// One conversation message.
#[derive(Clone, Debug)]
pub struct Message {
    pub role: String,
    pub text: String,
}

/// Virtualized feed: layout is cached per message, but the *component tree*
/// only ever contains the visible window. `visible()` is pure geometry.
#[derive(Clone, Debug, Default)]
pub struct Feed {
    pub messages: Vec<Message>,
    pub scroll_px: f32,
    heights: Vec<f32>,
    layout_width: f32,
}

impl Feed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, role: &str, text: &str) {
        self.messages.push(Message {
            role: role.to_string(),
            text: text.to_string(),
        });
        self.heights.push(-1.0);
    }

    /// Append an empty message and hand back its index, so streamed
    /// deltas have somewhere to land without exposing layout internals.
    pub fn push_live(&mut self, role: &str) -> usize {
        self.push(role, "");
        self.messages.len() - 1
    }

    pub fn relayout(&mut self, width: f32) {
        self.layout_width = width;
        for (i, m) in self.messages.iter().enumerate() {
            if self.heights[i] < 0.0 {
                let l = hamesh_text::TextLayout::new(&m.text, width - 16.0, 16.0, None);
                // Row = role line + text block + padding.
                self.heights[i] = 24.0 + l.height().max(18.0) + 12.0;
            }
        }
    }

    pub fn content_height(&self) -> f32 {
        self.heights.iter().sum()
    }

    pub fn scroll_by(&mut self, delta: f32, viewport_h: f32) {
        let max = (self.content_height() - viewport_h).max(0.0);
        self.scroll_px = (self.scroll_px + delta).clamp(0.0, max);
    }

    pub fn scroll_to_bottom(&mut self, viewport_h: f32) {
        self.scroll_px = (self.content_height() - viewport_h).max(0.0);
    }

    /// Indices intersecting [scroll, scroll + viewport_h).
    pub fn visible(&self, viewport_h: f32) -> std::ops::Range<usize> {
        let mut start = 0;
        let mut y = 0.0;
        for (i, h) in self.heights.iter().enumerate() {
            if y + h > self.scroll_px {
                start = i;
                break;
            }
            y += h;
            start = i + 1;
        }
        let mut end = start;
        y = self.heights[..start.min(self.heights.len())].iter().sum();
        for h in &self.heights[start..] {
            if y >= self.scroll_px + viewport_h {
                break;
            }
            y += h;
            end += 1;
        }
        start..end.min(self.messages.len())
    }
}

/// Deterministic keyboard focus order over focusable components.
#[derive(Clone, Debug, Default)]
pub struct Focus {
    pub order: Vec<String>,
    pub current: usize,
}

impl Focus {
    pub fn rebuild(app: &App) -> Self {
        let mut order = Vec::new();
        collect_focusable(app, &app.root.0, &mut order);
        Self { order, current: 0 }
    }

    pub fn focused(&self) -> Option<&str> {
        self.order.get(self.current).map(|s| s.as_str())
    }

    pub fn tab(&mut self) {
        if !self.order.is_empty() {
            self.current = (self.current + 1) % self.order.len();
        }
    }

    pub fn shift_tab(&mut self) {
        if !self.order.is_empty() {
            self.current = (self.current + self.order.len() - 1) % self.order.len();
        }
    }
}

fn collect_focusable(app: &App, id: &str, out: &mut Vec<String>) {
    if let Some(c) = app.components.get(id) {
        if c.enabled && c.visible && matches!(c.role, Role::TextInput | Role::Button | Role::List) {
            out.push(id.to_string());
        }
        for child in &c.children {
            collect_focusable(app, &child.0, out);
        }
    }
}

/// Key events, shared by real winit input and synthetic drill input.
#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Tab,
    ShiftTab,
    Escape,
}

/// Widget state owned by the application driver (demo / window).
#[derive(Clone, Debug, Default)]
pub struct Widgets {
    pub composer: Composer,
    pub feed: Feed,
    pub focus: Focus,
    /// Id of the composer component (for prop sync + focus checks).
    pub composer_id: String,
    /// Id of the feed list component.
    pub feed_id: String,
    /// Visible feed height in px, set by the driver from the window size.
    /// Scroll clamps and the visible window derive from it — never from
    /// the hugged content height (that overflow shrank the header and
    /// composer via flex-shrink; caught by the M3 window PNG).
    pub feed_viewport: f32,
}

/// Route one key through the human keyboard path. Returns a short
/// description of what happened (for traces). Focus + visibility +
/// enabled are enforced for text entry, exactly as a human would find.
pub fn handle_key(app: &mut App, w: &mut Widgets, key: &Key) -> String {
    // All scroll geometry derives from the driver-set viewport.
    let vh = if w.feed_viewport > 0.0 {
        w.feed_viewport
    } else {
        400.0
    };
    match key {
        Key::Tab => {
            w.focus.tab();
            return format!("focus -> {}", w.focus.focused().unwrap_or("(none)"));
        }
        Key::ShiftTab => {
            w.focus.shift_tab();
            return format!("focus -> {}", w.focus.focused().unwrap_or("(none)"));
        }
        Key::Escape => {
            return "escape (no-op)".to_string();
        }
        _ => {}
    }
    let focused = w.focus.focused().unwrap_or("").to_string();
    if focused == w.composer_id {
        match key {
            Key::Char(c) => {
                w.composer.insert(&c.to_string());
                sync_composer(app, w);
                return format!("typed {c:?}");
            }
            Key::Backspace => {
                w.composer.backspace();
                sync_composer(app, w);
                return "backspace".to_string();
            }
            Key::Delete => {
                w.composer.delete_forward();
                sync_composer(app, w);
                return "delete".to_string();
            }
            Key::Left => {
                w.composer.move_left();
                sync_composer(app, w);
                return "caret left".to_string();
            }
            Key::Right => {
                w.composer.move_right();
                sync_composer(app, w);
                return "caret right".to_string();
            }
            Key::Home => {
                w.composer.home();
                sync_composer(app, w);
                return "caret home".to_string();
            }
            Key::End => {
                w.composer.end();
                sync_composer(app, w);
                return "caret end".to_string();
            }
            Key::Enter => {
                if let Some(text) = w.composer.take_submission() {
                    w.feed.push("user", &text);
                    // New content arrives at the bottom; follow it.
                    w.feed.scroll_to_bottom(vh);
                    sync_composer(app, w);
                    sync_feed_count(app, w);
                    return format!("submitted {text:?}");
                }
                return "submit empty (ignored)".to_string();
            }
            Key::Up | Key::Down => {
                // Single-line composer: vertical arrows scroll the feed.
                let d = if *key == Key::Up { -40.0 } else { 40.0 };
                w.feed.scroll_by(d, vh);
                return format!("feed scroll {d}");
            }
            _ => {}
        }
    } else if focused == w.feed_id {
        match key {
            Key::Up => {
                w.feed.scroll_by(-60.0, vh);
                return "feed scroll up".to_string();
            }
            Key::Down => {
                w.feed.scroll_by(60.0, vh);
                return "feed scroll down".to_string();
            }
            Key::Enter => {
                return "feed enter (no-op)".to_string();
            }
            _ => {}
        }
    }
    "unfocused key (ignored)".to_string()
}

/// Mirror composer truth into component props (the Eye reads these).
pub fn sync_composer(app: &mut App, w: &Widgets) {
    let avail = 700.0;
    let cx = w.composer.caret_x(avail);
    if let Some(c) = app.get_mut(&w.composer_id) {
        c.props.insert("text".to_string(), w.composer.shown_text());
        c.props.insert("caret_x".to_string(), format!("{cx:.1}"));
        c.props.insert(
            "preedit".to_string(),
            w.composer.preedit.clone().unwrap_or_default(),
        );
        c.props.insert(
            "focused".to_string(),
            (w.focus.focused() == Some(w.composer_id.as_str())).to_string(),
        );
    }
}

pub fn sync_feed_count(app: &mut App, w: &Widgets) {
    let n = w.feed.messages.len().to_string();
    if let Some(c) = app.get_mut(&w.feed_id) {
        c.props.insert("count".to_string(), n);
    }
}

/// Build (or rebuild) the visible feed nodes. Only the visible window
/// becomes components — this is the virtualization proof.
pub fn rebuild_feed_nodes(app: &mut App, w: &mut Widgets, feed_panel_id: &str, width: f32) {
    // Drop old item nodes.
    let olds: Vec<ComponentId> = app
        .components
        .values()
        .filter(|c| c.parent.as_ref().map(|p| p.0.as_str()) == Some(feed_panel_id))
        .map(|c| c.id.clone())
        .collect();
    for o in olds {
        app.components.remove(&o.0);
    }
    if let Some(p) = app.components.get_mut(feed_panel_id) {
        p.children.clear();
    }
    w.feed.relayout(width);
    // The visible window derives from the driver's viewport, and the
    // feed panel is sized to match (see refresh_feed). Never hug the
    // full content: that overflowed the window and flex-shrink crushed
    // the fixed header/composer.
    let viewport_h = if w.feed_viewport > 0.0 {
        w.feed_viewport
    } else {
        400.0
    };
    let range = w.feed.visible(viewport_h);
    let mut y = -w.feed.scroll_px + w.feed.heights[..range.start].iter().sum::<f32>();
    for i in range {
        let m = &w.feed.messages[i];
        let h = w.feed.heights[i];
        let id = format!("msg_{i}");
        let mut item = Component::new(&id, Role::ListItem, &format!("message {i}"))
            .with_prop("role", &m.role)
            .with_prop("text", &m.text)
            .with_prop("h", &format!("{h:.0}"))
            .with_prop("y_off", &format!("{y:.0}"));
        item.parent = Some(ComponentId::new(feed_panel_id));
        let cid = item.id.clone();
        app.components.insert(id, item);
        if let Some(p) = app.components.get_mut(feed_panel_id) {
            p.children.push(cid);
        }
        y += h;
        let _ = y;
    }
    app.bump();
    sync_feed_count(app, w);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn harness() -> (App, Widgets) {
        let mut app = App::new("win");
        app.add("win", Component::new("feed", Role::List, "feed"));
        app.add(
            "win",
            Component::new("composer", Role::TextInput, "composer").with_prop("h", "36"),
        );
        let w = Widgets {
            composer_id: "composer".to_string(),
            feed_id: "feed".to_string(),
            feed_viewport: 400.0,
            focus: Focus::rebuild(&app),
            ..Default::default()
        };
        // Focus starts on the feed (document order); Tab reaches composer.
        (app, w)
    }

    #[test]
    fn typing_and_caret() {
        let (mut app, mut w) = harness();
        // Focus composer: feed -> composer via Tab.
        assert_eq!(w.focus.focused(), Some("feed"));
        handle_key(&mut app, &mut w, &Key::Tab);
        assert_eq!(w.focus.focused(), Some("composer"));
        for c in "Hi!".chars() {
            handle_key(&mut app, &mut w, &Key::Char(c));
        }
        assert_eq!(w.composer.text, "Hi!");
        handle_key(&mut app, &mut w, &Key::Left);
        handle_key(&mut app, &mut w, &Key::Backspace);
        assert_eq!(w.composer.text, "H!");
        // caret_x grows monotonically with text.
        let x1 = w.composer.caret_x(700.0);
        handle_key(&mut app, &mut w, &Key::Char('!'));
        let x2 = w.composer.caret_x(700.0);
        assert!(x2 > x1, "caret did not advance: {x1} -> {x2}");
        // Submit appends to the feed.
        handle_key(&mut app, &mut w, &Key::Enter);
        assert_eq!(w.feed.messages.len(), 1);
        assert_eq!(w.composer.text, "");
    }

    #[test]
    fn virtualized_feed_window() {
        let (mut app, mut w) = harness();
        for i in 0..200 {
            w.feed.push(
                if i % 2 == 0 { "user" } else { "agent" },
                &format!("message number {i} with some text to wrap"),
            );
        }
        rebuild_feed_nodes(&mut app, &mut w, "feed", 700.0);
        let range = w.feed.visible(400.0);
        let rendered = app.components.len();
        // 200 messages, but only the visible window becomes components.
        assert!(rendered < 40, "not virtualized: {rendered} components");
        assert!(range.end - range.start < 30, "window too big: {range:?}");
        // Scrolling moves the window.
        w.feed.scroll_by(500.0, 400.0);
        let range2 = w.feed.visible(400.0);
        assert_ne!(range, range2, "scroll did not move window");
        rebuild_feed_nodes(&mut app, &mut w, "feed", 700.0);
        assert!(app.components.len() < 40);
    }

    #[test]
    fn unfocused_typing_ignored() {
        let (mut app, mut w) = harness();
        // Focus is on the feed; typing must not reach the composer.
        handle_key(&mut app, &mut w, &Key::Char('x'));
        assert_eq!(w.composer.text, "");
    }

    /// Grapheme fixtures (decision 017): ZWJ family (5 chars, 1 grapheme),
    /// waving hand + skin tone (2 chars), flag (2 chars), e + combining
    /// acute (2 chars). Char-based editing splits these; the composer must
    /// delete and cross them whole.
    const ZWJ_FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    const HAND_TONE: &str = "\u{1F44B}\u{1F3FD}";

    #[test]
    fn backspace_deletes_whole_graphemes() {
        let mut c = Composer::new();
        c.insert("a");
        c.insert(ZWJ_FAMILY);
        c.insert("e\u{301}");
        c.insert(HAND_TONE);
        assert_eq!(c.caret, 1 + 5 + 2 + 2);
        c.backspace();
        assert_eq!(c.text, format!("a{ZWJ_FAMILY}e\u{301}"), "skin tone split");
        c.backspace();
        assert_eq!(c.text, format!("a{ZWJ_FAMILY}"), "combining mark split");
        c.backspace();
        assert_eq!(c.text, "a", "ZWJ family split");
        assert_eq!(c.caret, 1);
        c.backspace();
        assert_eq!(c.text, "");
        assert_eq!(c.caret, 0);
        // Backspace at zero is a no-op, not a panic.
        c.backspace();
        assert_eq!(c.text, "");
    }

    #[test]
    fn arrows_cross_graphemes_whole() {
        let mut c = Composer::new();
        c.insert(HAND_TONE);
        assert_eq!(c.caret, 2);
        c.move_left();
        assert_eq!(c.caret, 0, "arrow stopped inside skin-tone emoji");
        c.move_right();
        assert_eq!(c.caret, 2, "arrow stopped inside skin-tone emoji");
        c.move_right();
        assert_eq!(c.caret, 2, "arrow ran past the end");
    }

    #[test]
    fn delete_forward_deletes_whole_grapheme() {
        let mut c = Composer::new();
        c.insert(ZWJ_FAMILY);
        c.home();
        c.delete_forward();
        assert_eq!(c.text, "", "forward delete split the ZWJ family");
        // Delete at end is a no-op.
        c.delete_forward();
        assert_eq!(c.text, "");
    }

    #[test]
    fn grapheme_positions_stay_valid() {
        // Mid-text edits keep the caret on a boundary: insert inside a
        // flag, then walk and delete — nothing may split or panic.
        let mut c = Composer::new();
        c.insert("\u{1F1EB}\u{1F1EE}");
        assert_eq!(c.caret, 2);
        c.move_left();
        assert_eq!(c.caret, 0);
        c.insert("x");
        assert_eq!(c.text, "x\u{1F1EB}\u{1F1EE}");
        c.move_right();
        c.move_right();
        assert_eq!(c.caret, 3);
        c.backspace();
        assert_eq!(c.text, "x");
    }

    #[test]
    fn preedit_shows_but_does_not_commit() {
        // Decision 019: marked text is display-only until Commit.
        let mut c = Composer::new();
        c.insert("ab");
        c.move_left();
        c.set_preedit("\u{3042}\u{3044}");
        assert_eq!(c.text, "ab", "preedit leaked into the buffer");
        assert_eq!(c.shown_text(), "a\u{3042}\u{3044}b");
        assert_eq!(c.caret, 1, "caret must stay at composition start");
        // caret_x measures the committed prefix only.
        let bare = Composer {
            text: "a".to_string(),
            caret: 1,
            ..Default::default()
        };
        assert_eq!(c.caret_x(700.0), bare.caret_x(700.0));
        // Empty preedit clears.
        c.set_preedit("");
        assert_eq!(c.shown_text(), "ab");
        assert!(c.preedit.is_none());
    }

    #[test]
    fn commit_folds_through_grapheme_insert() {
        let mut c = Composer::new();
        c.set_preedit("\u{3042}");
        c.commit("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}");
        assert!(c.preedit.is_none());
        assert_eq!(c.caret, 5, "caret counts chars of the committed text");
        // Whole-grapheme delete still holds after a commit.
        c.backspace();
        assert_eq!(c.text, "");
        assert_eq!(c.caret, 0);
    }

    #[test]
    fn submit_commits_preedit_first() {
        let mut c = Composer::new();
        c.insert("hi ");
        c.set_preedit("\u{4E2D}\u{6587}");
        let out = c.take_submission();
        assert_eq!(out, Some("hi \u{4E2D}\u{6587}".to_string()));
        assert!(c.preedit.is_none());
        assert_eq!(c.text, "");
    }
}
