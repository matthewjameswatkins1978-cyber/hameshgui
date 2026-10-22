//! Hamesh Blocks: reusable GUI building blocks on the canonical model.
//!
//! Builders emit ordinary `hamesh_core::Component`s with style props —
//! no separate runtime, layout engine or event system. A composite is
//! an arrangement of lower-level components with useful defaults.
//!
//! Every block carries a `block` prop (its type name) so the Eye can
//! explain it without reading source. Option structs are serde so a
//! validated structured (JSON) form can resolve to the same builders.

use hamesh_core::{App, Component, ComponentId, Role};
use serde::{Deserialize, Serialize};

/// Authoring scope: the app plus a parent stack. `branch` pushes,
/// builds children, pops — nesting without lifetime juggling.
pub struct Scope<'a> {
    pub app: &'a mut App,
    stack: Vec<String>,
}

impl<'a> Scope<'a> {
    pub fn root(app: &'a mut App, root_id: &str) -> Self {
        Self {
            app,
            stack: vec![root_id.to_string()],
        }
    }

    fn parent(&self) -> String {
        self.stack.last().cloned().expect("scope stack empty")
    }

    pub fn leaf(&mut self, comp: Component) {
        let p = self.parent();
        self.app.add(&p, comp);
    }

    pub fn branch(&mut self, comp: Component, kids: impl FnOnce(&mut Scope)) {
        let id = comp.id.0.clone();
        let p = self.parent();
        self.app.add(&p, comp);
        self.stack.push(id);
        kids(self);
        self.stack.pop();
    }

    pub fn id(&self, id: &str) -> ComponentId {
        ComponentId::new(id)
    }
}

fn styled(role: Role, id: &str, name: &str, block: &str) -> Component {
    let mut c = Component::new(id, role, name);
    c.props.insert("block".to_string(), block.to_string());
    c
}

// ---- Layout ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutOpts {
    pub pad: f32,
    pub gap: f32,
    pub wrap: bool,
    pub grow: Option<f32>,
    pub w: Option<f32>,
    pub h: Option<f32>,
}

impl Default for LayoutOpts {
    fn default() -> Self {
        Self {
            pad: hamesh_theme::space::M,
            gap: hamesh_theme::space::S,
            wrap: false,
            grow: None,
            w: None,
            h: None,
        }
    }
}

impl LayoutOpts {
    /// Edge-to-edge row: no padding, standard gap (headers, actions).
    pub fn flat() -> Self {
        Self {
            pad: 0.0,
            ..Default::default()
        }
    }
    /// Tight stack: no padding, hairline gap (label groups).
    pub fn tight() -> Self {
        Self {
            pad: 0.0,
            gap: hamesh_theme::space::XS,
            ..Default::default()
        }
    }
    /// Responsive tile grid: no padding, wide gap, wraps (metrics).
    pub fn tiles() -> Self {
        Self {
            pad: 0.0,
            gap: hamesh_theme::space::M,
            wrap: true,
            ..Default::default()
        }
    }
}

fn apply_layout(c: &mut Component, layout: &str, o: &LayoutOpts) {
    c.props.insert("layout".to_string(), layout.to_string());
    c.props.insert("pad".to_string(), o.pad.to_string());
    c.props.insert("gap".to_string(), o.gap.to_string());
    if o.wrap {
        c.props.insert("wrap".to_string(), "true".to_string());
    }
    if let Some(g) = o.grow {
        c.props.insert("flex_grow".to_string(), g.to_string());
    }
    if let Some(w) = o.w {
        c.props.insert("w".to_string(), w.to_string());
    }
    if let Some(h) = o.h {
        c.props.insert("h".to_string(), h.to_string());
    }
}

/// Horizontal arrangement. `wrap=true` turns it into a responsive row
/// that breaks equal tiles onto the next line (the metrics grid).
pub fn row(s: &mut Scope, id: &str, o: LayoutOpts, kids: impl FnOnce(&mut Scope)) {
    let mut c = styled(Role::Panel, id, id, "Row");
    apply_layout(&mut c, "row", &o);
    s.branch(c, kids);
}

/// Vertical arrangement.
pub fn col(s: &mut Scope, id: &str, o: LayoutOpts, kids: impl FnOnce(&mut Scope)) {
    let mut c = styled(Role::Panel, id, id, "Column");
    apply_layout(&mut c, "column", &o);
    s.branch(c, kids);
}

/// Flexible empty space. Ghost: holds layout room, paints nothing,
/// never intercepts the Hand.
pub fn spacer(s: &mut Scope, id: &str) {
    let mut c = styled(Role::Panel, id, "spacer", "Spacer");
    c.props.insert("ghost".to_string(), "true".to_string());
    c.props.insert("flex_grow".to_string(), "1".to_string());
    s.leaf(c);
}

/// Hairline separator.
pub fn divider(s: &mut Scope, id: &str) {
    let mut c = styled(Role::Panel, id, "divider", "Divider");
    c.props.insert("surface".to_string(), "border".to_string());
    c.props.insert("h".to_string(), "1".to_string());
    s.leaf(c);
}

// ---- Typography ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextOpts {
    pub size: f32,
    pub token: String,
}

impl TextOpts {
    pub fn sized(size: f32, token: &str) -> Self {
        Self {
            size,
            token: token.to_string(),
        }
    }
}

fn text_node(kind: &str, id: &str, text: &str, o: &TextOpts) -> Component {
    let mut c = styled(Role::Text, id, id, kind);
    c.props.insert("text".to_string(), text.to_string());
    c.props.insert("font_size".to_string(), o.size.to_string());
    c.props.insert("text_token".to_string(), o.token.clone());
    c
}

/// Prominent screen/page title.
pub fn title(s: &mut Scope, id: &str, text: &str) {
    let (size, _) = hamesh_theme::type_ramp::TITLE;
    s.leaf(text_node("Title", id, text, &TextOpts::sized(size, "text")));
}

/// Section heading.
pub fn heading(s: &mut Scope, id: &str, text: &str) {
    let (size, _) = hamesh_theme::type_ramp::HEADING;
    s.leaf(text_node(
        "Heading",
        id,
        text,
        &TextOpts::sized(size, "text"),
    ));
}

/// Standard body copy.
pub fn body(s: &mut Scope, id: &str, text: &str) {
    let (size, _) = hamesh_theme::type_ramp::BODY;
    s.leaf(text_node("Body", id, text, &TextOpts::sized(size, "text")));
}

/// Small descriptive label.
pub fn caption(s: &mut Scope, id: &str, text: &str) {
    let (size, _) = hamesh_theme::type_ramp::CAPTION;
    s.leaf(text_node(
        "Caption",
        id,
        text,
        &TextOpts::sized(size, "muted"),
    ));
}

/// Muted secondary line (subtitles, hints).
pub fn subtle(s: &mut Scope, id: &str, text: &str) {
    let (size, _) = hamesh_theme::type_ramp::BODY;
    s.leaf(text_node(
        "Subtle",
        id,
        text,
        &TextOpts::sized(size, "text_dim"),
    ));
}

// ---- Surfaces ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardOpts {
    pub pad: f32,
    pub radius: f32,
    pub surface: String,
    pub border_w: f32,
}

impl Default for CardOpts {
    fn default() -> Self {
        Self {
            pad: hamesh_theme::space::L,
            radius: hamesh_theme::radius::LG,
            surface: "surface".to_string(),
            border_w: 1.0,
        }
    }
}

/// Rounded, bordered container with generous padding.
pub fn card(s: &mut Scope, id: &str, o: CardOpts, kids: impl FnOnce(&mut Scope)) {
    let mut c = styled(Role::Panel, id, id, "Card");
    c.props.insert("layout".to_string(), "column".to_string());
    c.props.insert("pad".to_string(), o.pad.to_string());
    c.props.insert("radius".to_string(), o.radius.to_string());
    c.props.insert("surface".to_string(), o.surface);
    if o.border_w > 0.0 {
        c.props.insert("border".to_string(), o.border_w.to_string());
    }
    s.branch(c, kids);
}

// ---- Badge ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Variant {
    Success,
    Warning,
    Error,
    Info,
    Neutral,
}

impl Variant {
    fn surface(&self) -> &str {
        // Tinted fills read as pills on dark surfaces; text stays bright.
        match self {
            Self::Success => "raised",
            Self::Warning => "raised",
            Self::Error => "raised",
            Self::Info => "raised",
            Self::Neutral => "raised",
        }
    }
    fn dot(&self) -> &str {
        match self {
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Info => "accent",
            Self::Neutral => "muted",
        }
    }
    fn text(&self) -> &str {
        match self {
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Info => "accent_bright",
            Self::Neutral => "text_dim",
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Info => "info",
            Self::Neutral => "neutral",
        }
    }
}

/// Status pill: dot + label, colored by variant.
pub fn badge(s: &mut Scope, id: &str, text: &str, v: Variant) {
    let (size, _) = hamesh_theme::type_ramp::LABEL;
    let mut c = styled(Role::Status, id, id, "Badge");
    c.props.insert("text".to_string(), text.to_string());
    c.props.insert("font_size".to_string(), size.to_string());
    c.props
        .insert("text_token".to_string(), v.text().to_string());
    c.props
        .insert("surface".to_string(), v.surface().to_string());
    c.props
        .insert("radius".to_string(), hamesh_theme::radius::LG.to_string());
    c.props.insert("border".to_string(), "1".to_string());
    c.props
        .insert("border_token".to_string(), v.dot().to_string());
    c.props
        .insert("variant".to_string(), v.as_str().to_string());
    // Shared 16px text origin with buttons (pad 8 + inset 8): pill
    // and button first letters sit on the same invisible line.
    c.props.insert("pad".to_string(), "8".to_string());
    // Pills center on the cross axis instead of stretching tall.
    c.props.insert("align".to_string(), "center".to_string());
    s.leaf(c);
}

// ---- Icon ----

/// Restrained line icon (Lucide path, stroked in `token` color).
/// The node fills with `surface` so it blends into its parent tile.
pub fn icon(s: &mut Scope, id: &str, name: &str, token: &str, surface: &str, size: f32) {
    let mut c = styled(Role::Panel, id, id, "Icon");
    c.props.insert("icon".to_string(), name.to_string());
    c.props.insert("text_token".to_string(), token.to_string());
    c.props.insert("surface".to_string(), surface.to_string());
    c.props.insert("w".to_string(), size.to_string());
    c.props.insert("h".to_string(), size.to_string());
    // Painted leaf: no interior padding (same bar lesson).
    c.props.insert("pad".to_string(), "0".to_string());
    s.leaf(c);
}

// ---- Metric ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricOpts {
    pub label: String,
    pub value: String,
    pub icon_name: String,
    pub icon_token: String,
}

impl MetricOpts {
    pub fn new(label: &str, value: &str, icon_name: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
            icon_name: icon_name.to_string(),
            icon_token: "text_dim".to_string(),
        }
    }
}

/// Statistic tile: icon, prominent value, muted label. Tiles in a
/// wrapping row share space equally — pass `grow` via a wrapping Row.
pub fn metric(s: &mut Scope, id: &str, o: MetricOpts) {
    let mut tile = styled(Role::Panel, id, id, "Metric");
    tile.props
        .insert("layout".to_string(), "column".to_string());
    tile.props
        .insert("pad".to_string(), hamesh_theme::space::L.to_string());
    tile.props
        .insert("gap".to_string(), hamesh_theme::space::S.to_string());
    tile.props
        .insert("radius".to_string(), hamesh_theme::radius::MD.to_string());
    tile.props
        .insert("surface".to_string(), "raised".to_string());
    tile.props.insert("border".to_string(), "1".to_string());
    tile.props.insert("flex_grow".to_string(), "1".to_string());
    // Base zero: tiles share the row equally regardless of content.
    tile.props.insert("basis".to_string(), "0".to_string());
    tile.props.insert("value".to_string(), o.value.clone());
    tile.props
        .insert("metric_label".to_string(), o.label.clone());
    s.branch(tile, |t| {
        icon(
            t,
            &format!("{id}-icon"),
            &o.icon_name,
            &o.icon_token,
            "raised",
            20.0,
        );
        let (vsize, _) = hamesh_theme::type_ramp::METRIC_VALUE;
        t.leaf(text_node(
            "MetricValue",
            &format!("{id}-value"),
            &o.value,
            &TextOpts::sized(vsize, "text"),
        ));
        let (lsize, _) = hamesh_theme::type_ramp::CAPTION;
        t.leaf(text_node(
            "MetricLabel",
            &format!("{id}-label"),
            &o.label,
            &TextOpts::sized(lsize, "muted"),
        ));
    });
}

// ---- Progress ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressOpts {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub fill: String,
    pub h: f32,
}

impl ProgressOpts {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            min: 0.0,
            max: 100.0,
            fill: "accent".to_string(),
            h: 6.0,
        }
    }
}

/// Thin progress bar: track surface + accent fill from a real number.
pub fn progress(s: &mut Scope, id: &str, o: ProgressOpts) {
    let mut c = styled(Role::Panel, id, id, "Progress");
    c.props.insert("surface".to_string(), "track".to_string());
    c.props.insert("radius".to_string(), "3".to_string());
    // A bar has no interior: container padding would inflate it
    // (an empty h=6 leaf with default pad 12 lays out 24 tall).
    c.props.insert("pad".to_string(), "0".to_string());
    c.props.insert("h".to_string(), o.h.to_string());
    c.props.insert("value".to_string(), o.value.to_string());
    c.props.insert("min".to_string(), o.min.to_string());
    c.props.insert("max".to_string(), o.max.to_string());
    c.props.insert("fill".to_string(), o.fill);
    s.leaf(c);
}

// ---- Button ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ButtonVariant {
    Primary,
    Outline,
    Ghost,
}

impl ButtonVariant {
    fn surface(&self) -> &str {
        match self {
            Self::Primary => "accent",
            Self::Outline => "surface",
            Self::Ghost => "raised",
        }
    }
    fn text(&self) -> &str {
        match self {
            Self::Primary => "bg",
            Self::Outline => "text",
            Self::Ghost => "text_dim",
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Outline => "outline",
            Self::Ghost => "ghost",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ButtonOpts {
    pub variant: ButtonVariant,
    pub action: String,
}

impl ButtonOpts {
    pub fn new(variant: ButtonVariant, action: &str) -> Self {
        Self {
            variant,
            action: action.to_string(),
        }
    }
}

/// Labeled button with a named action. The host app owns the action —
/// Blocks never invent behaviour, only declare the intent.
pub fn button(s: &mut Scope, id: &str, label: &str, o: ButtonOpts) {
    let (size, _) = hamesh_theme::type_ramp::LABEL;
    let mut c = styled(Role::Button, id, label, "Button");
    c.props.insert("text".to_string(), label.to_string());
    c.props.insert("font_size".to_string(), size.to_string());
    c.props
        .insert("text_token".to_string(), o.variant.text().to_string());
    c.props
        .insert("surface".to_string(), o.variant.surface().to_string());
    c.props
        .insert("radius".to_string(), hamesh_theme::radius::SM.to_string());
    c.props.insert("border".to_string(), "1".to_string());
    c.props
        .insert("h".to_string(), hamesh_theme::control::H.to_string());
    c.props.insert("pad".to_string(), "8".to_string());
    c.props
        .insert("variant".to_string(), o.variant.as_str().to_string());
    c.props.insert("action".to_string(), o.action.clone());
    s.leaf(c);
    s.app.register_command(&o.action, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_nesting_builds_tree() {
        let mut app = App::new("win");
        {
            let mut s = Scope::root(&mut app, "win");
            card(&mut s, "c", CardOpts::default(), |t| {
                title(t, "t", "Agent X");
            });
        }
        let c = app.get("c").unwrap();
        assert_eq!(c.props.get("block").map(|s| s.as_str()), Some("Card"));
        assert_eq!(c.parent.as_ref().unwrap().0, "win");
        let t = app.get("t").unwrap();
        assert_eq!(t.parent.as_ref().unwrap().0, "c");
        assert_eq!(t.props.get("text").map(|s| s.as_str()), Some("Agent X"));
    }

    #[test]
    fn metric_tile_structure() {
        let mut app = App::new("win");
        {
            let mut s = Scope::root(&mut app, "win");
            row(
                &mut s,
                "tiles",
                LayoutOpts {
                    wrap: true,
                    ..Default::default()
                },
                |t| {
                    metric(t, "m-agents", MetricOpts::new("Agents", "4", "users"));
                },
            );
        }
        let m = app.get("m-agents").unwrap();
        assert_eq!(m.props.get("block").map(|s| s.as_str()), Some("Metric"));
        assert_eq!(m.props.get("value").map(|s| s.as_str()), Some("4"));
        let ic = app.get("m-agents-icon").unwrap();
        assert_eq!(ic.props.get("icon").map(|s| s.as_str()), Some("users"));
        let v = app.get("m-agents-value").unwrap();
        assert_eq!(v.props.get("text").map(|s| s.as_str()), Some("4"));
    }

    #[test]
    fn button_registers_command() {
        let mut app = App::new("win");
        {
            let mut s = Scope::root(&mut app, "win");
            button(
                &mut s,
                "go",
                "Inspect design",
                ButtonOpts::new(ButtonVariant::Outline, "inspect_design"),
            );
        }
        assert_eq!(
            app.commands.get("inspect_design").map(|s| s.as_str()),
            Some("go")
        );
        let b = app.get("go").unwrap();
        assert_eq!(b.props.get("variant").map(|s| s.as_str()), Some("outline"));
    }

    #[test]
    fn catalogue_covers_every_builder() {
        // The machine catalogue is the Eye's discovery path: every
        // block prop the builders emit must be documented with the
        // shape agents need (description, props, actions, example).
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../catalogue.json")).expect("catalogue parses");
        let blocks = v
            .get("blocks")
            .and_then(|b| b.as_array())
            .expect("blocks array");
        let mut names: Vec<&str> = Vec::new();
        for b in blocks {
            let name = b.get("name").and_then(|n| n.as_str()).expect("block name");
            names.push(name);
            assert!(
                b.get("description").and_then(|d| d.as_str()).is_some(),
                "{name} needs a description"
            );
            assert!(
                b.get("props").and_then(|p| p.as_array()).is_some(),
                "{name} needs a props array"
            );
            assert!(
                b.get("example").and_then(|e| e.as_str()).is_some(),
                "{name} needs an example"
            );
        }
        for expected in [
            "Row", "Column", "Spacer", "Divider", "Title", "Heading", "Body", "Caption", "Subtle",
            "Card", "Badge", "Icon", "Metric", "Progress", "Button",
        ] {
            assert!(names.contains(&expected), "catalogue missing {expected}");
        }
    }
}
