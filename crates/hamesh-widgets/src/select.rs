//! Selection controls: the behaviour half of lists and pickers.
//!
//! These are pure state machines. They own selection, hover, scroll and
//! filter state and answer questions a composer can render and an event
//! loop can drive. They deliberately know nothing about layout, theme or
//! pixels: `hamesh_blocks::list` draws them, and an application's event
//! loop forwards pointer and key intents.
//!
//! The contract a consumer relies on:
//!
//! * geometry in, indices out (`row_at`, `hover_at`) — no hit-testing
//!   duplicated in the application;
//! * selection always stays revealed (`select`, `nav`, `wheel`);
//! * only the visible window is materialised (`visible`), so a list of
//!   fifty thousand rows costs the same to draw as a list of ten;
//! * the picker filters, highlights, commits and cancels internally, so
//!   an application never reimplements Escape, Enter or reveal behaviour.

use std::ops::Range;

/// A navigation intent, independent of any particular key code. An
/// application maps native keys to these once, in its input layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// A scrollable, selectable list of rows of uniform height.
///
/// Scrolling is expressed as a pixel offset from the top of the content.
/// The visible window is computed from it, which is what makes large
/// data sets cheap: the composer emits `visible()` rows and a single
/// leading spacer, never the whole model.
#[derive(Clone, Debug)]
pub struct ListView {
    count: usize,
    row_h: f32,
    selected: Option<usize>,
    hover: Option<usize>,
    focused: bool,
    scroll: f32,
    viewport_h: f32,
    typeahead: String,
}

impl ListView {
    /// A list of `row_h` tall rows, initially empty and unfocused.
    pub fn new(row_h: f32) -> Self {
        Self {
            count: 0,
            row_h: row_h.max(1.0),
            selected: None,
            hover: None,
            typeahead: String::new(),
            focused: false,
            scroll: 0.0,
            viewport_h: 0.0,
        }
    }

    pub fn row_h(&self) -> f32 {
        self.row_h
    }

    /// Change the row pitch without losing place: selection and
    /// scroll survive (clamped to the new geometry). The picker
    /// composer calls this so the painted pitch and the
    /// hit-test pitch can never drift apart.
    pub fn set_row_h(&mut self, row_h: f32) {
        self.row_h = row_h.max(1.0);
        self.clamp_scroll();
        if let Some(sel) = self.selected {
            self.reveal(sel);
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn hover(&self) -> Option<usize> {
        self.hover
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    pub fn viewport_h(&self) -> f32 {
        self.viewport_h
    }

    pub fn content_h(&self) -> f32 {
        self.count as f32 * self.row_h
    }

    pub fn max_scroll(&self) -> f32 {
        (self.content_h() - self.viewport_h).max(0.0)
    }

    /// Replace the model. Selection and scroll are clamped rather than
    /// dropped: reloading a list should not lose the user's place.
    pub fn set_count(&mut self, count: usize) {
        self.count = count;
        if let Some(sel) = self.selected {
            if sel >= count {
                self.selected = count.checked_sub(1);
            }
        }
        self.hover = self.hover.filter(|h| *h < count);
        self.clamp_scroll();
    }

    /// The viewport height is a layout fact the application already
    /// knows; passing it in keeps scrolling honest when a window
    /// resizes, without the widget reaching into the layout tree.
    pub fn set_viewport(&mut self, viewport_h: f32) {
        self.viewport_h = viewport_h.max(0.0);
        self.clamp_scroll();
        if let Some(sel) = self.selected {
            self.reveal(sel);
        }
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// Select a row and scroll it into view. Returns whether the
    /// selection actually changed.
    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.count {
            return false;
        }
        let changed = self.selected != Some(index);
        self.selected = Some(index);
        self.reveal(index);
        changed
    }

    pub fn clear_selection(&mut self) -> bool {
        let changed = self.selected.is_some();
        self.selected = None;
        changed
    }

    /// Move the selection by whole rows, clamping at both ends. With no
    /// selection the first move selects the first or last row, so a
    /// keyboard user never gets stuck.
    pub fn nav(&mut self, nav: Nav) -> bool {
        if self.count == 0 {
            return false;
        }
        let page = self.rows_per_page().max(1);
        let target = match nav {
            Nav::Up => match self.selected {
                Some(s) => s.saturating_sub(1),
                None => 0,
            },
            Nav::Down => match self.selected {
                Some(s) => (s + 1).min(self.count - 1),
                None => 0,
            },
            Nav::PageUp => match self.selected {
                Some(s) => s.saturating_sub(page),
                None => 0,
            },
            Nav::PageDown => match self.selected {
                Some(s) => (s + page).min(self.count - 1),
                None => 0,
            },
            Nav::Home => 0,
            Nav::End => self.count - 1,
        };
        // Repeat a boundary move is not a change; report it honestly so
        // callers can skip work (and so tests can assert idempotence).
        let changed = self.selected != Some(target);
        self.selected = Some(target);
        self.reveal(target);
        changed
    }

    pub fn rows_per_page(&self) -> usize {
        if self.row_h <= 0.0 {
            1
        } else {
            (self.viewport_h / self.row_h).floor().max(1.0) as usize
        }
    }

    /// Scroll by a pixel delta, keeping the selection where it is. This
    /// is what the wheel does: the user is looking around, not moving
    /// the selection.
    pub fn wheel(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        self.scroll = (self.scroll - dy).clamp(0.0, self.max_scroll());
        (self.scroll - before).abs() > f32::EPSILON
    }

    /// Scroll the minimum distance that makes `index` fully visible.
    pub fn reveal(&mut self, index: usize) {
        if self.viewport_h <= 0.0 || index >= self.count {
            return;
        }
        let top = index as f32 * self.row_h;
        let bottom = top + self.row_h;
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + self.viewport_h {
            self.scroll = bottom - self.viewport_h;
        }
        self.clamp_scroll();
    }

    /// Row index under a y offset measured in *content* coordinates
    /// (pointer y relative to the viewport top, plus `scroll()`).
    pub fn row_at(&self, content_y: f32) -> Option<usize> {
        if content_y < 0.0 {
            return None;
        }
        let row = (content_y / self.row_h).floor();
        if row < 0.0 {
            return None;
        }
        let row = row as usize;
        (row < self.count).then_some(row)
    }

    /// Row index under a pointer y measured relative to the viewport
    /// top, accounting for scroll. Convenience over `row_at`.
    pub fn row_at_viewport(&self, y_local: f32) -> Option<usize> {
        self.row_at(y_local + self.scroll)
    }

    /// Update hover from a pointer y relative to the viewport top.
    /// Returns whether the hovered row changed.
    pub fn hover_at_viewport(&mut self, y_local: f32) -> bool {
        let next = self.row_at_viewport(y_local);
        let changed = next != self.hover;
        self.hover = next;
        changed
    }

    pub fn clear_hover(&mut self) -> bool {
        let changed = self.hover.is_some();
        self.hover = None;
        changed
    }

    /// APG type-ahead: typing a character moves selection to the
    /// next row whose name starts with the typed prefix; rapid
    /// successive characters extend the prefix, and the search
    /// wraps around the end of the list. `name_at` maps a row to
    /// its searchable name so the widget itself stays string-free.
    /// A prefix with no match restarts from the single character,
    /// so fast typists never get stuck on a dead buffer. The host
    /// clears the buffer after ~500 ms without typing via
    /// `clear_typeahead`.
    pub fn typeahead(&mut self, ch: char, name_at: &dyn Fn(usize) -> String) -> bool {
        if ch.is_control() || self.count == 0 {
            return false;
        }
        self.typeahead.push(ch);
        let prefix = self.typeahead.clone();
        if self.jump_to_prefix(&prefix, name_at) {
            return true;
        }
        // No match for the accumulated prefix: restart from the
        // single character (standard single-select listbox behaviour).
        self.typeahead.clear();
        self.typeahead.push(ch);
        let prefix = self.typeahead.clone();
        self.jump_to_prefix(&prefix, name_at)
    }

    /// First row at/after the selection whose name starts with
    /// `prefix` (wrapping). `None` means no match anywhere — the
    /// only case that restarts the buffer.
    fn find_prefix(&self, prefix: &str, name_at: &dyn Fn(usize) -> String) -> Option<usize> {
        let prefix = prefix.to_lowercase();
        let start = self.selected.map(|s| s + 1).unwrap_or(0);
        (0..self.count)
            .map(|off| (start + off) % self.count)
            .find(|&i| name_at(i).to_lowercase().starts_with(&prefix))
    }

    fn jump_to_prefix(&mut self, prefix: &str, name_at: &dyn Fn(usize) -> String) -> bool {
        // A match on the current row is a match: staying put is
        // not failure (returning select()'s changed-flag here
        // restarted the buffer and jumped elsewhere).
        if let Some(i) = self.find_prefix(prefix, name_at) {
            self.select(i);
            true
        } else {
            false
        }
    }

    /// Drop the accumulated type-ahead prefix. The host calls
    /// this after ~500 ms without typing.
    pub fn clear_typeahead(&mut self) {
        self.typeahead.clear();
    }

    /// The half-open range of rows that can be painted. This is the
    /// virtualisation contract: one row of slack above and below the
    /// viewport so a partially scrolled row is drawn whole.
    pub fn visible(&self) -> Range<usize> {
        if self.count == 0 || self.viewport_h <= 0.0 {
            return 0..0;
        }
        let first = (self.scroll / self.row_h).floor().max(0.0) as usize;
        let last = ((self.scroll + self.viewport_h) / self.row_h).ceil() as usize;
        first.min(self.count)..last.clamp(first, self.count)
    }
}

/// A filtered, keyboard-driven selection control.
///
/// The picker owns the whole interaction: the query, the filtered view,
/// the highlight, reveal-in-list and the accept/cancel decisions. An
/// application supplies items, forwards keys and pointer positions, and
/// reads the committed index — nothing else.
#[derive(Clone, Debug)]
pub struct Picker {
    items: Vec<String>,
    query: String,
    open: bool,
    /// Index into `filtered()`, not into `items`, so filtering can move
    /// the highlight without the application re-mapping anything.
    highlight: Option<usize>,
    selected: Option<usize>,
    list: ListView,
    focused: bool,
}

impl Picker {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            query: String::new(),
            open: false,
            highlight: None,
            selected: None,
            list: ListView::new(30.0),
            focused: false,
        }
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        if let Some(sel) = self.selected {
            if sel >= self.items.len() {
                self.selected = None;
            }
        }
        self.refilter();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The committed value, if any. Applications read this; they never
    /// index `items` themselves to find out what the user chose.
    pub fn value(&self) -> Option<&str> {
        self.selected
            .and_then(|i| self.items.get(i))
            .map(|s| s.as_str())
    }

    pub fn set_selected(&mut self, index: Option<usize>) {
        self.selected = index.filter(|i| *i < self.items.len());
    }

    pub fn highlight(&self) -> Option<usize> {
        self.highlight
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// The text the field should show: the query while open, the
    /// committed value while closed, and an empty string when nothing is
    /// chosen (the composer decides what placeholder to draw).
    pub fn display(&self) -> String {
        if self.open {
            self.query.clone()
        } else {
            self.value().unwrap_or("").to_string()
        }
    }

    /// Open the popup, clearing the query so the full set is reachable
    /// immediately, and highlight the current value.
    pub fn open(&mut self) -> bool {
        if self.open {
            return false;
        }
        self.open = true;
        self.query.clear();
        self.refilter();
        self.highlight = self
            .selected
            .and_then(|sel| self.filtered().iter().position(|i| *i == sel))
            .or_else(|| (!self.filtered().is_empty()).then_some(0));
        self.list.set_count(self.filtered().len());
        self.list.select(self.highlight.unwrap_or(0));
        true
    }

    /// Cancel: close without changing the value and discard the query.
    pub fn close(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.open = false;
        self.query.clear();
        self.highlight = None;
        self.list.set_count(0);
        true
    }

    /// Toggle, for pointer activation of the field.
    pub fn toggle(&mut self) -> bool {
        if self.open {
            self.close()
        } else {
            self.open()
        }
    }

    /// Items matching the query, as indices into `items`. Case-insensitive
    /// substring matching: predictable, cheap, and it keeps the rank of
    /// the list stable while the user types.
    pub fn filtered(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..self.items.len()).collect();
        }
        let q = self.query.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    fn refilter(&mut self) {
        let n = self.filtered().len();
        self.list.set_count(n);
        self.highlight = match self.highlight {
            Some(_) if n == 0 => None,
            Some(h) => Some(h.min(n.saturating_sub(1))),
            None => None,
        };
        if let Some(h) = self.highlight {
            self.list.select(h);
        }
    }

    /// Append a typed character to the query.
    pub fn push_char(&mut self, c: char) -> bool {
        if !self.open || c.is_control() {
            return false;
        }
        self.query.push(c);
        self.highlight = None;
        self.refilter();
        self.highlight = (!self.filtered().is_empty()).then_some(0);
        if let Some(h) = self.highlight {
            self.list.select(h);
        }
        true
    }

    pub fn backspace(&mut self) -> bool {
        if !self.open || self.query.pop().is_none() {
            return false;
        }
        self.highlight = None;
        self.refilter();
        self.highlight = (!self.filtered().is_empty()).then_some(0);
        if let Some(h) = self.highlight {
            self.list.select(h);
        }
        true
    }

    pub fn clear_query(&mut self) -> bool {
        if !self.open || self.query.is_empty() {
            return false;
        }
        self.query.clear();
        self.highlight = None;
        self.refilter();
        self.highlight = (!self.filtered().is_empty()).then_some(0);
        if let Some(h) = self.highlight {
            self.list.select(h);
        }
        true
    }

    /// Move the highlight through the filtered rows.
    pub fn nav(&mut self, nav: Nav) -> bool {
        if !self.open {
            return false;
        }
        let before = self.highlight;
        let n = self.filtered().len();
        if n == 0 {
            return false;
        }
        let cur = self.highlight.unwrap_or(0);
        let page = self.list.rows_per_page().max(1);
        let next = match nav {
            Nav::Up => cur.saturating_sub(1),
            Nav::Down => (cur + 1).min(n - 1),
            Nav::PageUp => cur.saturating_sub(page),
            Nav::PageDown => (cur + page).min(n - 1),
            Nav::Home => 0,
            Nav::End => n - 1,
        };
        self.highlight = Some(next);
        self.list.select(next);
        before != Some(next)
    }

    /// Highlight the row under a pointer y measured from the top of the
    /// option list.
    pub fn highlight_at_viewport(&mut self, y_local: f32) -> bool {
        if !self.open {
            return false;
        }
        let Some(row) = self.list.row_at_viewport(y_local) else {
            return false;
        };
        if row >= self.filtered().len() {
            return false;
        }
        let before = self.highlight;
        self.highlight = Some(row);
        self.list.select(row);
        before != Some(row)
    }

    pub fn wheel(&mut self, dy: f32) -> bool {
        self.open && self.list.wheel(dy)
    }

    /// Accept the highlight: closes the popup and returns the chosen
    /// item index. `None` means "nothing to accept" (closed, or no
    /// matches), which is a legitimate outcome the caller need not
    /// special-case beyond leaving the value alone.
    pub fn commit(&mut self) -> Option<usize> {
        if !self.open {
            return None;
        }
        let filtered = self.filtered();
        let h = self.highlight?;
        let item = *filtered.get(h)?;
        self.selected = Some(item);
        self.close();
        Some(item)
    }

    /// Set the option list's metrics. The application knows the popup
    /// height it granted the control.
    pub fn set_viewport(&mut self, viewport_h: f32) {
        self.list.set_viewport(viewport_h);
    }

    /// Sync the option pitch to what the composer paints. Preserves
    /// highlight, selection and scroll (clamped); recreating the
    /// list here used to drop all three on every recompose.
    pub fn set_row_h(&mut self, row_h: f32) {
        self.list.set_row_h(row_h);
        self.list.set_count(self.filtered().len());
        if let Some(h) = self.highlight {
            self.list.select(h);
        }
    }

    /// Rows of the filtered list that the popup should paint.
    pub fn visible(&self) -> Range<usize> {
        self.list.visible()
    }

    pub fn scroll(&self) -> f32 {
        self.list.scroll()
    }

    pub fn filtered_len(&self) -> usize {
        self.filtered().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(n: usize, viewport: f32) -> ListView {
        let mut l = ListView::new(20.0);
        l.set_count(n);
        l.set_viewport(viewport);
        l
    }

    #[test]
    fn first_nav_selects_the_first_row() {
        let mut l = list(5, 100.0);
        assert_eq!(l.selected(), None);
        assert!(l.nav(Nav::Down));
        assert_eq!(l.selected(), Some(0));
        assert!(!l.nav(Nav::Up), "already at the top");
    }

    #[test]
    fn nav_clamps_at_both_ends() {
        let mut l = list(3, 100.0);
        l.select(0);
        assert!(!l.nav(Nav::Up));
        l.select(2);
        assert!(!l.nav(Nav::Down));
    }

    #[test]
    fn selection_is_revealed_when_it_leaves_the_viewport() {
        let mut l = list(50, 100.0); // five rows visible
        l.set_viewport(100.0);
        l.select(9);
        assert_eq!(l.scroll(), 100.0, "row 9 top is 180; view is 100 tall");
        let v = l.visible();
        assert!(v.contains(&9), "selected row must be painted: {v:?}");
    }

    #[test]
    fn wheel_scrolls_without_moving_the_selection() {
        let mut l = list(50, 100.0);
        l.select(1);
        assert!(l.wheel(-20.0), "wheel down one row");
        assert_eq!(l.scroll(), 20.0);
        assert_eq!(l.selected(), Some(1));
        assert!(l.wheel(10_000.0), "scrolling back to the top is a change");
        assert!(!l.wheel(1.0), "already at the top");
    }

    #[test]
    fn wheel_clamps_to_content() {
        let mut l = list(10, 100.0);
        l.wheel(-10_000.0);
        assert_eq!(l.scroll(), l.max_scroll());
        assert_eq!(l.max_scroll(), 100.0);
    }

    #[test]
    fn only_the_visible_window_is_emitted_for_a_huge_list() {
        let mut l = ListView::new(24.0);
        l.set_count(50_000);
        l.set_viewport(480.0); // twenty rows
        l.select(40_000);
        let v = l.visible();
        assert!(
            v.len() <= 22,
            "virtualised window must stay near the viewport, got {} rows",
            v.len()
        );
        assert!(v.contains(&40_000));
    }

    #[test]
    fn pointer_maps_to_rows_through_scroll() {
        let mut l = list(50, 100.0);
        l.wheel(-40.0); // scrolled 40px: rows 2.. start at viewport y 0
        assert_eq!(l.row_at_viewport(0.0), Some(2));
        assert_eq!(l.row_at_viewport(19.0), Some(2));
        assert_eq!(l.row_at_viewport(20.0), Some(3));
        // Content coordinates do not move when the wheel does.
        assert_eq!(l.row_at(10.0), Some(0));
        assert_eq!(l.row_at(10_000.0), None, "past the end of the model");
    }

    #[test]
    fn shrinking_the_model_clamps_selection_instead_of_losing_it() {
        let mut l = list(10, 100.0);
        l.select(9);
        l.set_count(3);
        assert_eq!(l.selected(), Some(2));
        l.set_count(0);
        assert_eq!(l.selected(), None);
    }

    #[test]
    fn resizing_the_viewport_keeps_the_selection_visible() {
        let mut l = list(50, 400.0);
        l.select(19);
        assert_eq!(l.scroll(), 0.0);
        l.set_viewport(100.0);
        assert!(l.visible().contains(&19));
    }

    fn typeahead_names() -> Vec<String> {
        vec![
            "Alpha".into(),
            "Archive".into(),
            "Beta".into(),
            "Instrument Sans".into(),
            "Inter".into(),
        ]
    }

    #[test]
    fn typeahead_moves_to_the_next_match_and_wraps() {
        let mut l = list(5, 100.0);
        let n = typeahead_names();
        assert!(l.typeahead('b', &|i| n[i].clone()));
        assert_eq!(l.selected(), Some(2));
        // A second 'b' makes "bb": no match, so the prefix
        // restarts at 'b', finds Beta again — and staying put
        // counts as a match, not a failure.
        assert!(l.typeahead('b', &|i| n[i].clone()));
        assert_eq!(l.selected(), Some(2));
    }

    #[test]
    fn typeahead_prefix_narrows_across_keystrokes() {
        let mut l = list(5, 100.0);
        let n = typeahead_names();
        assert!(l.typeahead('i', &|i| n[i].clone()));
        assert_eq!(l.selected(), Some(3), "first 'i' match");
        assert!(l.typeahead('n', &|i| n[i].clone()));
        assert_eq!(
            l.selected(),
            Some(4),
            "'in' moves to the next match after the current row"
        );
        // 'x' kills the prefix and matches nothing on its own:
        // selection stays where the 'in' search left it.
        assert!(!l.typeahead('x', &|i| n[i].clone()));
        assert_eq!(l.selected(), Some(4));
    }

    #[test]
    fn typeahead_is_case_insensitive_and_ignores_control_keys() {
        let mut l = list(5, 100.0);
        let n = typeahead_names();
        assert!(l.typeahead('A', &|i| n[i].clone()));
        assert_eq!(l.selected(), Some(0));
        assert!(
            !l.typeahead('\u{8}', &|i| n[i].clone()),
            "backspace is not a typeahead char"
        );
        l.clear_typeahead();
        assert!(l.typeahead('a', &|i| n[i].clone()));
        assert_eq!(
            l.selected(),
            Some(1),
            "fresh buffer finds the next 'a' after Alpha"
        );
    }

    fn picker() -> Picker {
        Picker::new(vec![
            "Newsreader".into(),
            "Instrument Sans".into(),
            "Inter".into(),
            "Fraunces".into(),
            "IBM Plex Mono".into(),
        ])
    }

    #[test]
    fn opening_highlights_the_current_value() {
        let mut p = picker();
        p.set_selected(Some(2));
        p.open();
        assert!(p.is_open());
        assert_eq!(
            p.filtered()[p.highlight().unwrap()],
            2,
            "highlight follows value"
        );
    }

    #[test]
    fn typing_filters_and_resets_the_highlight() {
        let mut p = picker();
        p.open();
        for c in "in".chars() {
            p.push_char(c);
        }
        // Only "Instrument Sans" and "Inter" contain "in".
        assert_eq!(p.filtered_len(), 2);
        assert_eq!(p.highlight(), Some(0));
        p.push_char('t');
        // "int" keeps only "Inter": "instrument" does not contain it.
        assert_eq!(p.filtered_len(), 1);
        assert_eq!(p.items()[p.filtered()[0]], "Inter");
    }

    #[test]
    fn escape_discards_the_query_and_keeps_the_value() {
        let mut p = picker();
        p.set_selected(Some(1));
        p.open();
        p.push_char('f');
        assert_eq!(p.query(), "f");
        p.close();
        assert!(!p.is_open());
        assert_eq!(p.query(), "");
        assert_eq!(p.value(), Some("Instrument Sans"));
        assert_eq!(p.display(), "Instrument Sans");
    }

    #[test]
    fn enter_commits_the_highlight_and_closes() {
        let mut p = picker();
        p.open();
        p.nav(Nav::Down);
        p.nav(Nav::Down);
        let chosen = p.commit().expect("a value");
        assert_eq!(chosen, 2);
        assert!(!p.is_open());
        assert_eq!(p.value(), Some("Inter"));
    }

    #[test]
    fn no_matches_means_no_commit_and_an_empty_highlight() {
        let mut p = picker();
        p.open();
        for c in "zzz".chars() {
            p.push_char(c);
        }
        assert_eq!(p.filtered_len(), 0);
        assert_eq!(p.highlight(), None);
        assert_eq!(p.commit(), None);
        assert_eq!(p.value(), None, "a failed search must not change the value");
        assert!(p.is_open(), "and it must not close on the user either");
    }

    #[test]
    fn backspace_widens_the_search_again() {
        let mut p = picker();
        p.open();
        p.push_char('f');
        p.push_char('r');
        let narrow = p.filtered_len();
        p.backspace();
        assert!(p.filtered_len() >= narrow);
        p.clear_query();
        assert_eq!(p.filtered_len(), 5);
    }

    #[test]
    fn highlight_is_revealed_in_a_long_option_list() {
        let items: Vec<String> = (0..500).map(|i| format!("Item {i:03}")).collect();
        let mut p = Picker::new(items);
        p.set_row_h(24.0);
        p.set_viewport(240.0); // ten rows
        p.open();
        p.nav(Nav::End);
        assert_eq!(p.highlight(), Some(499));
        let v = p.visible();
        assert!(
            v.contains(&499),
            "highlight must be scrolled into view: {v:?}"
        );
        assert!(v.len() <= 12, "popup stays virtualised, got {}", v.len());
    }

    #[test]
    fn set_row_h_keeps_place() {
        let mut l = list(50, 200.0);
        l.select(25);
        l.set_row_h(16.0);
        assert_eq!(l.row_h(), 16.0);
        assert_eq!(l.selected(), Some(25));
        assert!(l.scroll() <= l.max_scroll());
        assert!(l.visible().contains(&25));
    }

    #[test]
    fn picker_row_sync_preserves_highlight_and_scroll() {
        let items: Vec<String> = (0..100).map(|i| format!("Item {i:03}")).collect();
        let mut p = Picker::new(items);
        p.set_viewport(300.0);
        p.open();
        p.nav(Nav::End);
        let h = p.highlight();
        assert_eq!(h, Some(99));
        p.set_row_h(32.0);
        assert_eq!(p.highlight(), h, "highlight survives a recompose sync");
        assert!(p.visible().contains(&h.unwrap()));
    }

    #[test]
    fn pointer_highlight_maps_through_popup_scroll() {
        let items: Vec<String> = (0..200).map(|i| format!("Item {i:03}")).collect();
        let mut p = Picker::new(items);
        p.set_row_h(30.0);
        p.set_viewport(150.0);
        p.open();
        p.wheel(-60.0); // two rows down
        assert_eq!(
            p.highlight(),
            Some(0),
            "the wheel scrolls, it does not highlight"
        );
        p.highlight_at_viewport(0.0);
        assert_eq!(p.highlight(), Some(2));
        p.highlight_at_viewport(30.0);
        assert_eq!(p.highlight(), Some(3));
    }

    #[test]
    fn an_unavailable_item_list_still_opens_and_closes_cleanly() {
        let mut p = Picker::new(Vec::new());
        p.open();
        assert_eq!(p.filtered_len(), 0);
        assert_eq!(p.highlight(), None);
        assert_eq!(p.commit(), None);
        assert!(p.close());
        assert_eq!(p.display(), "");
    }
}
