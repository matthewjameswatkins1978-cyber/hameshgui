//! hamesh-console: a real Pi session console as a reusable composition.
//!
//! An M3 chat surface (virtualized feed + grapheme composer + tree focus)
//! plus a live engine-status strip (streaming badge, model, session,
//! message count, Abort), composed ONLY from Blocks over driver-owned
//! widget state. Pointer and keyboard route through the shared
//! `hamesh_widgets::Hit` router and `handle_key` core — this driver owns
//! no parent climbing, no key enum, no nav mapping, exactly like tasks.
//!
//! The driver never talks to a Pi worker: the host feeds display state
//! via [`Console::set_status`] and translates the `"abort-run"` intent
//! into a worker cancel. Streaming deltas land through the same
//! `push_live` slot the live loop uses.

use hamesh_blocks as b;
use hamesh_core::{App, Component, Role};
use hamesh_widgets::{
    handle_key, rebuild_feed_nodes, sync_composer, sync_feed_count, Focus, Hit, Key, Widgets,
};

/// Engine display state. Plain driver-owned data: the host maps its
/// worker link onto this; the console only paints what it is told.
/// `model`/`session` are `"—"` until the first live fetch lands.
#[derive(Clone, Debug)]
pub struct EngineStatus {
    pub model: String,
    pub session: String,
    pub messages: i64,
    pub streaming: bool,
}

impl Default for EngineStatus {
    fn default() -> Self {
        Self {
            model: "—".to_string(),
            session: "—".to_string(),
            messages: 0,
            streaming: false,
        }
    }
}

/// A parsed `/attach` request, stashed for the host: the host
/// translates it into a worker respawn (same pattern as the abort
/// intent — the driver never touches a worker or a thread).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachRequest {
    pub id: String,
    pub dir: String,
    pub cwd: String,
}

/// A parsed `/new` request, stashed for the host: the host mints a
/// fresh session id and respawns the worker (the attach pattern, with
/// no id to restore). Empty `dir`/`cwd` mean "reuse the current ones".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRequest {
    pub dir: String,
    pub cwd: String,
}

pub struct Console {
    pub widgets: Widgets,
    pub status: EngineStatus,
    /// Pending attach request from an `/attach` submit or the Attach
    /// button prefill flow. The host takes it exactly once.
    pub pending_attach: Option<AttachRequest>,
    /// Pending new-session request from a `/new` submit or the New
    /// button prefill flow. The host takes it exactly once.
    pub pending_new: Option<NewRequest>,
}

impl Console {
    pub fn new() -> Self {
        hamesh_text::register_fonts_from_dir(&hamesh_text::bundled_fonts_dir());
        let widgets = Widgets {
            composer_id: "composer".to_string(),
            feed_id: "feed".to_string(),
            ..Default::default()
        };
        Self {
            widgets,
            status: EngineStatus::default(),
            pending_attach: None,
            pending_new: None,
        }
    }

    /// Host-fed display state: model/session from a worker `get_state`
    /// round trip, message count from the feed, streaming from the live
    /// run slot. Nothing here touches a worker or a thread.
    pub fn set_status(&mut self, model: &str, session: &str, messages: i64, streaming: bool) {
        self.status = EngineStatus {
            model: model.to_string(),
            session: session.to_string(),
            messages,
            streaming,
        };
    }

    /// Whether Abort means anything right now. The window checks this
    /// before translating the intent into a worker cancel.
    pub fn abort_enabled(&self) -> bool {
        self.status.streaming
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

/// Feed viewport budget at this height: chrome (status strip, composer,
/// abort row, gaps, padding) takes ~228px; the feed gets the rest.
fn viewport_for(height: f32) -> f32 {
    (height - 228.0).max(120.0)
}

/// Compose the whole tree from driver state. Status strip and abort row
/// are Blocks; feed and composer are the M3 nodes the widget sync fns
/// expect, built manually exactly as the M3 tree builds them.
pub fn compose(c: &mut Console, width: f32, height: f32) -> App {
    c.widgets.feed_viewport = viewport_for(height);
    let mut app = App::new("consoleapp");
    {
        let mut root = b::Scope::root(&mut app, "consoleapp");
        b::row(
            &mut root,
            "console-header",
            b::LayoutOpts {
                pad: 12.0,
                gap: 8.0,
                ..Default::default()
            },
            |s| {
                b::title(s, "console-title", "Console");
                b::spacer(s, "console-hspace");
                let (badge_text, variant) = if c.status.streaming {
                    ("Streaming", b::Variant::Success)
                } else {
                    ("Idle", b::Variant::Neutral)
                };
                b::badge(s, "console-status", badge_text, variant);
                b::caption(s, "console-model", &c.status.model);
                b::caption(s, "console-session", &c.status.session);
                b::caption(
                    s,
                    "console-count",
                    &format!("{} messages", c.status.messages),
                );
            },
        );
    }
    app.add(
        "consoleapp",
        Component::new("feed", Role::List, "feed").with_prop("layout", "column"),
    );
    app.add(
        "consoleapp",
        Component::new("composer", Role::TextInput, "composer")
            .with_prop("h", "40")
            .with_prop("text", ""),
    );
    {
        let mut root = b::Scope::root(&mut app, "consoleapp");
        b::row(
            &mut root,
            "console-actions",
            b::LayoutOpts {
                pad: 12.0,
                ..Default::default()
            },
            |s| {
                b::caption(s, "console-attach-hint", "/attach · /new");
                b::spacer(s, "console-aspacer");
                let attach_focused = c
                    .widgets
                    .focus
                    .focused()
                    .is_some_and(|f| f == "console-attach");
                b::button(
                    s,
                    "console-attach",
                    "Attach",
                    b::ButtonOpts::new(b::ButtonVariant::Ghost, "attach-hint")
                        .focused(attach_focused),
                );
                let new_focused = c
                    .widgets
                    .focus
                    .focused()
                    .is_some_and(|f| f == "console-new");
                b::button(
                    s,
                    "console-new",
                    "New",
                    b::ButtonOpts::new(b::ButtonVariant::Ghost, "new-session").focused(new_focused),
                );
                let state = if c.abort_enabled() {
                    b::ButtonState::Rest
                } else {
                    b::ButtonState::Disabled
                };
                let focused = c
                    .widgets
                    .focus
                    .focused()
                    .is_some_and(|f| f == "console-abort");
                b::button(
                    s,
                    "console-abort",
                    "Abort run",
                    b::ButtonOpts::new(b::ButtonVariant::Ghost, "abort-run")
                        .with_state(state)
                        .focused(focused),
                );
            },
        );
    }
    rebuild_feed_nodes(&mut app, &mut c.widgets, "feed", width - 60.0);
    sync_composer(&mut app, &c.widgets);
    sync_feed_count(&mut app, &c.widgets);
    // Preserve keyboard focus across recomposes: a refresh must never
    // strand the operator on the feed. Falls back to the head of the
    // order when the focused node is gone (e.g. Abort disabling).
    let prev = c.widgets.focus.focused().map(|s| s.to_string());
    c.widgets.focus = Focus::rebuild(&app);
    if let Some(id) = prev {
        if let Some(pos) = c.widgets.focus.order.iter().position(|o| o == &id) {
            c.widgets.focus.current = pos;
        }
    }
    app
}

/// Recompose after any state change. Single pass: the feed viewport is
/// budgeted from the height before the tree is built.
pub fn refresh(c: &mut Console, w: f32, h: f32) -> (App, hamesh_layout::LayoutOutput) {
    let app = compose(c, w, h);
    let layout = hamesh_layout::compute(&app, w, h);
    (app, layout)
}

/// Keyboard through the shared M3 core: typing, Tab/ShiftTab focus,
/// submit, feed scroll. The core has no Enter-on-button, so Enter on
/// the focused Abort button maps to the same abort intent as a pointer
/// press (same gate); everything else goes through the shared core.
/// Parse an `/attach` submit into an [`AttachRequest`]. Missing
/// dir/cwd fall back to the host's last session (`""` = unset);
/// a bare `/attach` reports usage instead of stashing anything.
fn parse_attach(text: &str) -> Result<AttachRequest, String> {
    let mut parts = text.split_whitespace();
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("/attach"), Some(id), dir, cwd) if !id.is_empty() => Ok(AttachRequest {
            id: id.to_string(),
            dir: dir.unwrap_or_default().to_string(),
            cwd: cwd.unwrap_or_default().to_string(),
        }),
        _ => Err("usage: /attach <id> [dir] [cwd]".to_string()),
    }
}

/// Parse an `/new` submit into a [`NewRequest`]. Optional dir/cwd fall
/// back to the host's current session (`""` = unset); a bare `/new`
/// starts a fresh session in the same place, which is the common case.
fn parse_new(text: &str) -> Result<NewRequest, String> {
    let mut parts = text.split_whitespace();
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("/new"), dir, cwd, None) => Ok(NewRequest {
            dir: dir.unwrap_or_default().to_string(),
            cwd: cwd.unwrap_or_default().to_string(),
        }),
        _ => Err("usage: /new [dir] [cwd]".to_string()),
    }
}

/// Prefill the composer with a slash-command prefix and hand over the
/// composer: the one hand-over the Attach and New buttons share.
fn prefill(c: &mut Console, prefix: &str) {
    c.widgets.composer.text = prefix.to_string();
    c.widgets.composer.caret = prefix.chars().count();
    c.widgets.composer.preedit = None;
    if let Some(pos) = c.widgets.focus.order.iter().position(|o| o == "composer") {
        c.widgets.focus.current = pos;
    }
}

pub fn key(c: &mut Console, app: &mut App, k: &Key) -> String {
    if matches!(k, Key::Enter) && c.widgets.focus.focused() == Some("console-abort") {
        return if c.abort_enabled() {
            "abort requested".to_string()
        } else {
            "abort-run disabled: nothing to abort".to_string()
        };
    }
    if matches!(k, Key::Enter) && c.widgets.focus.focused() == Some("console-attach") {
        // Attach button: prefill the slash command and hand over the
        // composer, pointer-press equivalent.
        prefill(c, "/attach ");
        return "attach: type /attach <id> [dir] [cwd]".to_string();
    }
    if matches!(k, Key::Enter) && c.widgets.focus.focused() == Some("console-new") {
        // New button: the same hand-over, for a fresh session.
        prefill(c, "/new ");
        return "new: type /new [dir] [cwd]".to_string();
    }
    if matches!(k, Key::Enter) && c.widgets.focus.focused() == Some("composer") {
        // Fold any preedit so the prefix test sees the real text.
        c.widgets.composer.commit("");
        if c.widgets.composer.text.starts_with("/attach")
            || c.widgets.composer.text.starts_with("/new")
        {
            let text = std::mem::take(&mut c.widgets.composer.text);
            c.widgets.composer.caret = 0;
            if text.starts_with("/new") {
                return match parse_new(&text) {
                    Ok(req) => {
                        c.pending_new = Some(req);
                        "new session requested".to_string()
                    }
                    Err(usage) => usage,
                };
            }
            return match parse_attach(&text) {
                Ok(req) => {
                    c.pending_attach = Some(req);
                    "attach requested".to_string()
                }
                Err(usage) => usage,
            };
        }
    }
    let note = handle_key(app, &mut c.widgets, k);
    sync_feed_count(app, &c.widgets);
    note
}

/// Pointer through the shared Hit router. Abort returns an intent the
/// host translates into a worker cancel; feed rows take keyboard focus.
pub fn press(c: &mut Console, app: &App, scene: &hamesh_scene::Scene, x: f32, y: f32) -> String {
    match hamesh_scene::hit_test(app, scene, x, y) {
        None => "press hit nothing".to_string(),
        Some(id) => match Hit::of(app, &id) {
            Hit::Action { command } if command == "abort-run" => {
                if c.abort_enabled() {
                    "abort requested".to_string()
                } else {
                    "abort-run disabled: nothing to abort".to_string()
                }
            }
            Hit::Action { command } if command == "attach-hint" => {
                // Prefill the slash command and hand over the composer:
                // pointer equivalent of Tab-to-Attach + Enter.
                prefill(c, "/attach ");
                "attach: type /attach <id> [dir] [cwd]".to_string()
            }
            Hit::Action { command } if command == "new-session" => {
                // Pointer equivalent of Tab-to-New + Enter.
                prefill(c, "/new ");
                "new: type /new [dir] [cwd]".to_string()
            }
            Hit::Action { command } => format!("unknown command {command}"),
            Hit::ListRow { .. } => {
                if let Some(pos) = c.widgets.focus.order.iter().position(|o| o == "feed") {
                    c.widgets.focus.current = pos;
                    // The next refresh repaints the ring from driver
                    // state; no throwaway tree needed here.
                    "focus -> feed".to_string()
                } else {
                    "feed not focusable".to_string()
                }
            }
            Hit::Chrome(hit) => format!("hit {hit} (chrome, no action)"),
            other => format!("hit {other:?} (no console intent)"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        c: &mut Console,
        w: f32,
        h: f32,
    ) -> (App, hamesh_layout::LayoutOutput, hamesh_scene::Scene) {
        let (app, layout) = refresh(c, w, h);
        let scene = hamesh_scene::build(&app, &layout, 1);
        (app, layout, scene)
    }

    fn center_of(scene: &hamesh_scene::Scene, id: &str) -> (f32, f32) {
        let n = scene.nodes.iter().find(|n| n.id == id).unwrap();
        (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0)
    }

    #[test]
    fn tree_is_eye_clean_with_idle_status() {
        let mut c = Console::new();
        for (w, h) in [(800.0, 600.0), (1280.0, 800.0)] {
            let (app, _layout, scene) = frame(&mut c, w, h);
            for id in [
                "console-title",
                "console-status",
                "console-model",
                "console-session",
                "console-count",
                "feed",
                "composer",
                "console-attach-hint",
                "console-attach",
                "console-new",
                "console-abort",
            ] {
                assert!(app.get(id).is_some(), "missing {id} at {w}x{h}");
            }
            for n in &scene.nodes {
                assert!(!n.clipped, "{} clipped at {w}x{h}", n.id);
            }
        }
        let (app, _, _) = frame(&mut c, 800.0, 600.0);
        assert_eq!(
            app.get("console-status")
                .unwrap()
                .props
                .get("text")
                .map(|s| s.as_str()),
            Some("Idle")
        );
        // Idle Abort paints disabled and refuses through the same gate.
        assert!(!c.abort_enabled());
        let (app2, _, scene2) = frame(&mut c, 800.0, 600.0);
        let (x, y) = center_of(&scene2, "console-abort");
        assert!(press(&mut c, &app2, &scene2, x, y).contains("nothing to abort"));
    }

    #[test]
    fn status_refresh_paints_live_values() {
        let mut c = Console::new();
        c.set_status("mimo-flash", "ax-test-1", 7, true);
        let (app, _, _) = frame(&mut c, 800.0, 600.0);
        let text = |id: &str| {
            app.get(id)
                .unwrap()
                .props
                .get("text")
                .cloned()
                .unwrap_or_default()
        };
        assert_eq!(text("console-status"), "Streaming");
        assert_eq!(
            app.get("console-status")
                .unwrap()
                .props
                .get("variant")
                .map(|s| s.as_str()),
            Some("success")
        );
        assert_eq!(text("console-model"), "mimo-flash");
        assert_eq!(text("console-session"), "ax-test-1");
        assert_eq!(text("console-count"), "7 messages");
        assert!(c.abort_enabled());
        let (app2, _, scene2) = frame(&mut c, 800.0, 600.0);
        let (x, y) = center_of(&scene2, "console-abort");
        assert_eq!(press(&mut c, &app2, &scene2, x, y), "abort requested");
    }

    #[test]
    fn focus_survives_recompose() {
        // Regression: every refresh used to rebuild focus from zero,
        // stranding the operator on the feed after any intervening
        // refresh (e.g. the one a `type` op triggers before Enter).
        let mut c = Console::new();
        let (mut app, _) = refresh(&mut c, 800.0, 600.0);
        let _ = key(&mut c, &mut app, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("composer"));
        let (mut app2, _) = refresh(&mut c, 800.0, 600.0);
        assert_eq!(c.widgets.focus.focused(), Some("composer"));
        let _ = key(&mut c, &mut app2, &Key::Char('x'));
        assert_eq!(c.widgets.focus.focused(), Some("composer"));
        assert_eq!(
            app2.get("composer")
                .unwrap()
                .props
                .get("text")
                .map(|s| s.as_str()),
            Some("x")
        );
    }

    #[test]
    fn typing_and_submit_go_through_the_shared_core() {
        let mut c = Console::new();
        let (mut app, _) = refresh(&mut c, 800.0, 600.0);
        // Focus starts on the feed; Tab reaches the composer.
        assert_eq!(c.widgets.focus.focused(), Some("feed"));
        assert_eq!(key(&mut c, &mut app, &Key::Tab), "focus -> composer");
        for ch in "hi".chars() {
            let _ = key(&mut c, &mut app, &Key::Char(ch));
        }
        assert_eq!(
            app.get("composer")
                .unwrap()
                .props
                .get("text")
                .map(|s| s.as_str()),
            Some("hi")
        );
        let note = key(&mut c, &mut app, &Key::Enter);
        assert!(note.contains("submitted"), "{note}");
        assert!(c.widgets.feed.messages.iter().any(|m| m.text == "hi"));
        // Empty submit is ignored, not an empty user message.
        let before = c.widgets.feed.messages.len();
        let note = key(&mut c, &mut app, &Key::Enter);
        assert!(note.contains("ignored"), "{note}");
        assert_eq!(c.widgets.feed.messages.len(), before);
    }

    #[test]
    fn tab_reaches_abort_and_enter_runs_the_shared_gate() {
        let mut c = Console::new();
        let (mut app, _) = refresh(&mut c, 800.0, 600.0);
        // Idle: Abort is disabled, so Tab skips it (feed -> composer
        // -> attach -> new).
        assert_eq!(c.widgets.focus.focused(), Some("feed"));
        let _ = key(&mut c, &mut app, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("composer"));
        let _ = key(&mut c, &mut app, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("console-attach"));
        let _ = key(&mut c, &mut app, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("console-new"));
        let _ = key(&mut c, &mut app, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("feed"));
        // Streaming: Abort enables and joins the Tab order after Attach
        // and New; Enter raises the same intent as a pointer press, for
        // the host to translate into a worker cancel.
        c.set_status("m", "s", 1, true);
        let (mut app2, _) = refresh(&mut c, 800.0, 600.0);
        let _ = key(&mut c, &mut app2, &Key::Tab);
        let _ = key(&mut c, &mut app2, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("console-attach"));
        let _ = key(&mut c, &mut app2, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("console-new"));
        let _ = key(&mut c, &mut app2, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("console-abort"));
        let note = key(&mut c, &mut app2, &Key::Enter);
        assert_eq!(note, "abort requested");
        // Gone idle with focus still on Abort: Enter refuses honestly —
        // the same note a pointer press produces.
        c.set_status("m", "s", 1, false);
        let note = key(&mut c, &mut app2, &Key::Enter);
        assert!(note.contains("disabled"), "{note}");
    }

    #[test]
    fn attach_slash_command_parses_and_stashes() {
        let mut c = Console::new();
        let (mut app, _) = refresh(&mut c, 800.0, 600.0);
        // Focus the composer, type the command, submit.
        let _ = key(&mut c, &mut app, &Key::Tab);
        for ch in "/attach ax-2 d:/x d:/x".chars() {
            let _ = key(&mut c, &mut app, &Key::Char(ch));
        }
        let note = key(&mut c, &mut app, &Key::Enter);
        assert_eq!(note, "attach requested");
        assert_eq!(
            c.pending_attach,
            Some(AttachRequest {
                id: "ax-2".to_string(),
                dir: "d:/x".to_string(),
                cwd: "d:/x".to_string(),
            })
        );
        // Dir/cwd fall back to empty (host fills from last session).
        let (mut app2, _) = refresh(&mut c, 800.0, 600.0);
        for ch in "/attach ax-3".chars() {
            let _ = key(&mut c, &mut app2, &Key::Char(ch));
        }
        let _ = key(&mut c, &mut app2, &Key::Enter);
        // Normal prompts are untouched by the slash-command path.
        let (mut app3, _) = refresh(&mut c, 800.0, 600.0);
        for ch in "hello".chars() {
            let _ = key(&mut c, &mut app3, &Key::Char(ch));
        }
        let note = key(&mut c, &mut app3, &Key::Enter);
        assert!(note.contains("submitted"), "{note}");
        assert!(c.pending_attach.is_some(), "attach stash survives");
    }

    #[test]
    fn new_command_parses_and_stashes() {
        let mut c = Console::new();
        let (mut app, _) = refresh(&mut c, 800.0, 600.0);
        let _ = key(&mut c, &mut app, &Key::Tab); // composer
        for ch in "/new".chars() {
            let _ = key(&mut c, &mut app, &Key::Char(ch));
        }
        let note = key(&mut c, &mut app, &Key::Enter);
        assert_eq!(note, "new session requested");
        // A bare /new means "same place, fresh session": the host fills
        // dir/cwd from the session it is on.
        assert_eq!(
            c.pending_new,
            Some(NewRequest {
                dir: String::new(),
                cwd: String::new(),
            })
        );
        assert!(c.pending_attach.is_none(), "the two commands stay distinct");
        // Explicit dir/cwd win.
        let (mut app2, _) = refresh(&mut c, 800.0, 600.0);
        for ch in "/new d:/s d:/w".chars() {
            let _ = key(&mut c, &mut app2, &Key::Char(ch));
        }
        let _ = key(&mut c, &mut app2, &Key::Enter);
        assert_eq!(
            c.pending_new,
            Some(NewRequest {
                dir: "d:/s".to_string(),
                cwd: "d:/w".to_string(),
            })
        );
        // Extra fields are refused, not guessed.
        let (mut app3, _) = refresh(&mut c, 800.0, 600.0);
        for ch in "/new a b c".chars() {
            let _ = key(&mut c, &mut app3, &Key::Char(ch));
        }
        let note = key(&mut c, &mut app3, &Key::Enter);
        assert!(note.contains("usage"), "{note}");
        assert!(c.widgets.feed.messages.is_empty(), "nothing submitted");
    }

    #[test]
    fn new_button_prefills_and_enters() {
        let mut c = Console::new();
        let (app, _layout, scene) = frame(&mut c, 800.0, 600.0);
        let (x, y) = center_of(&scene, "console-new");
        let note = press(&mut c, &app, &scene, x, y);
        assert!(note.contains("/new"), "{note}");
        assert_eq!(c.widgets.composer.text, "/new ");
        assert_eq!(c.widgets.focus.focused(), Some("composer"));
        // Keyboard equivalence: Tab to the button, Enter prefills.
        let (mut app2, _, _scene2) = frame(&mut c, 800.0, 600.0);
        let _ = key(&mut c, &mut app2, &Key::Tab); // composer -> attach
        let _ = key(&mut c, &mut app2, &Key::Tab); // attach -> new
        assert_eq!(c.widgets.focus.focused(), Some("console-new"));
        let note = key(&mut c, &mut app2, &Key::Enter);
        assert!(note.contains("/new"), "{note}");
        assert_eq!(c.widgets.composer.text, "/new ");
    }

    #[test]
    fn attach_bare_command_reports_usage() {
        let mut c = Console::new();
        let (mut app, _) = refresh(&mut c, 800.0, 600.0);
        let _ = key(&mut c, &mut app, &Key::Tab);
        for ch in "/attach".chars() {
            let _ = key(&mut c, &mut app, &Key::Char(ch));
        }
        let note = key(&mut c, &mut app, &Key::Enter);
        assert!(note.contains("usage"), "{note}");
        assert!(c.pending_attach.is_none());
        assert!(c.widgets.feed.messages.is_empty(), "nothing submitted");
    }

    #[test]
    fn attach_button_prefills_and_enters() {
        let mut c = Console::new();
        let (app, _layout, scene) = frame(&mut c, 800.0, 600.0);
        let (x, y) = center_of(&scene, "console-attach");
        let note = press(&mut c, &app, &scene, x, y);
        assert!(note.contains("/attach"), "{note}");
        assert_eq!(c.widgets.composer.text, "/attach ");
        assert_eq!(c.widgets.focus.focused(), Some("composer"));
        // Same path through the keyboard: Tab to the button, Enter.
        // (Press already put keyboard focus on the composer, so one
        // Tab reaches Attach — the button the press just used.)
        let (mut app2, _, scene2) = frame(&mut c, 800.0, 600.0);
        let _ = key(&mut c, &mut app2, &Key::Tab);
        assert_eq!(c.widgets.focus.focused(), Some("console-attach"));
        let note = key(&mut c, &mut app2, &Key::Enter);
        assert!(note.contains("/attach"), "{note}");
        let _ = app;
        let _ = scene2;
    }
}
