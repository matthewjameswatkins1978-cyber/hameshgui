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

pub mod select;

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
    // Flush: default padding would turn the 1px rule into a bar.
    c.props.insert("pad".to_string(), "0".to_string());
    c.props.insert("h".to_string(), "1".to_string());
    s.leaf(c);
}

/// Surface token chip: a rounded swatch of the named surface with its
/// token name beneath. Design-system evidence, not decoration: the chip
/// paints the exact token components reference.
pub fn swatch(s: &mut Scope, id: &str, surface_token: &str) {
    col(
        s,
        id,
        LayoutOpts {
            gap: hamesh_theme::space::XS,
            ..Default::default()
        },
        |t| {
            let mut chip = styled(Role::Panel, &format!("{id}-chip"), surface_token, "Swatch");
            chip.props
                .insert("surface".to_string(), surface_token.to_string());
            chip.props
                .insert("radius".to_string(), hamesh_theme::radius::MD.to_string());
            chip.props.insert("border".to_string(), "1".to_string());
            chip.props
                .insert("border_token".to_string(), "border".to_string());
            chip.props.insert("w".to_string(), "88".to_string());
            chip.props.insert("h".to_string(), "40".to_string());
            t.leaf(chip);
            caption(t, &format!("{id}-name"), surface_token);
        },
    );
}

// ---- Typography ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextOpts {
    pub size: f32,
    pub line_height: f32,
    pub token: String,
}

impl TextOpts {
    /// Full ramp entry: size + line height travel together so the
    /// type ramp and actual rendering stay consistent (acceptance 5).
    pub fn sized(size: f32, line_height: f32, token: &str) -> Self {
        Self {
            size,
            line_height,
            token: token.to_string(),
        }
    }
}

fn text_node(kind: &str, id: &str, text: &str, o: &TextOpts) -> Component {
    let mut c = styled(Role::Text, id, id, kind);
    c.props.insert("text".to_string(), text.to_string());
    c.props.insert("font_size".to_string(), o.size.to_string());
    c.props.insert("lh".to_string(), o.line_height.to_string());
    c.props.insert("text_token".to_string(), o.token.clone());
    c
}

/// Prominent screen/page title.
pub fn title(s: &mut Scope, id: &str, text: &str) {
    let (size, lh) = hamesh_theme::type_ramp::TITLE;
    s.leaf(text_node(
        "Title",
        id,
        text,
        &TextOpts::sized(size, lh, "text"),
    ));
}

/// Section heading.
pub fn heading(s: &mut Scope, id: &str, text: &str) {
    let (size, lh) = hamesh_theme::type_ramp::HEADING;
    s.leaf(text_node(
        "Heading",
        id,
        text,
        &TextOpts::sized(size, lh, "text"),
    ));
}

/// Standard body copy.
pub fn body(s: &mut Scope, id: &str, text: &str) {
    let (size, lh) = hamesh_theme::type_ramp::BODY;
    s.leaf(text_node(
        "Body",
        id,
        text,
        &TextOpts::sized(size, lh, "text"),
    ));
}

/// Small descriptive label.
pub fn caption(s: &mut Scope, id: &str, text: &str) {
    let (size, lh) = hamesh_theme::type_ramp::CAPTION;
    s.leaf(text_node(
        "Caption",
        id,
        text,
        &TextOpts::sized(size, lh, "muted"),
    ));
}

/// Muted secondary line (subtitles, hints).
pub fn subtle(s: &mut Scope, id: &str, text: &str) {
    let (size, lh) = hamesh_theme::type_ramp::BODY;
    s.leaf(text_node(
        "Subtle",
        id,
        text,
        &TextOpts::sized(size, lh, "text_dim"),
    ));
}

// ---- Surfaces ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardOpts {
    pub pad: f32,
    pub radius: f32,
    pub surface: String,
    pub border_w: f32,
    /// Gap between the card's direct sections. Distinctly larger
    /// than inner-group gaps so sections read as groups (proximity).
    pub gap: f32,
    /// Flex grow: root-level cards set 1 to fill their window instead
    /// of floating in dead space; nested cards keep 0 and hug content.
    pub grow: f32,
}

impl Default for CardOpts {
    fn default() -> Self {
        Self {
            pad: hamesh_theme::space::L,
            radius: hamesh_theme::radius::LG,
            surface: "surface".to_string(),
            border_w: 1.0,
            gap: hamesh_theme::space::XL,
            grow: 0.0,
        }
    }
}

/// Rounded, bordered container with generous padding.
pub fn card(s: &mut Scope, id: &str, o: CardOpts, kids: impl FnOnce(&mut Scope)) {
    let mut c = styled(Role::Panel, id, id, "Card");
    c.props.insert("layout".to_string(), "column".to_string());
    c.props.insert("pad".to_string(), o.pad.to_string());
    c.props.insert("gap".to_string(), o.gap.to_string());
    if o.grow > 0.0 {
        c.props.insert("flex_grow".to_string(), o.grow.to_string());
    }
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

/// Status pill: label colored by variant, border tinted to match.
/// (No dot glyph — the tinted border is the indicator.)
pub fn badge(s: &mut Scope, id: &str, text: &str, v: Variant) {
    let (size, lh) = hamesh_theme::type_ramp::LABEL;
    let mut c = styled(Role::Status, id, id, "Badge");
    c.props.insert("text".to_string(), text.to_string());
    c.props.insert("font_size".to_string(), size.to_string());
    c.props.insert("lh".to_string(), lh.to_string());
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
    // Internal text alignment (distinct from Taffy cross-axis `align`
    // below): the label centers inside the pill's content box.
    c.props.insert("halign".to_string(), "center".to_string());
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

/// Activity row: a status line with its time pushed right. One line
/// per event keeps the section compact enough to fit the window.
pub fn activity(s: &mut Scope, id: &str, main: &str, sub: &str) {
    row(s, id, LayoutOpts::flat(), |t| {
        body(t, &format!("{id}-main"), main);
        spacer(t, &format!("{id}-sp"));
        caption(t, &format!("{id}-sub"), sub);
    });
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
    // Readability floor: below this width tiles stack instead of
    // crushing (wrapping rows honor min widths when breaking).
    tile.props.insert("min_w".to_string(), "140".to_string());
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
        let (vsize, vlh) = hamesh_theme::type_ramp::METRIC_VALUE;
        t.leaf(text_node(
            "MetricValue",
            &format!("{id}-value"),
            &o.value,
            &TextOpts::sized(vsize, vlh, "text"),
        ));
        let (lsize, llh) = hamesh_theme::type_ramp::CAPTION;
        t.leaf(text_node(
            "MetricLabel",
            &format!("{id}-label"),
            &o.label,
            &TextOpts::sized(lsize, llh, "muted"),
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
    fn surface(&self) -> &'static str {
        match self {
            Self::Primary => "accent",
            Self::Outline => "surface",
            Self::Ghost => "raised",
        }
    }
    fn text(&self) -> &'static str {
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

/// Interaction state of a button. The driver owns the state (hover
/// from the pointer, pressed from the press, disabled from the model);
/// the composer only paints it, following the theme state-system
/// contract: disabled > pressed > hover > rest, focus as a ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonState {
    Rest,
    Hover,
    Pressed,
    Disabled,
}

impl ButtonState {
    fn surface(&self, variant: &ButtonVariant) -> &'static str {
        match (self, variant) {
            (Self::Disabled, _) => "surface",
            (_, ButtonVariant::Primary) => match self {
                Self::Rest => "accent",
                Self::Hover => "accent_hover",
                Self::Pressed => "accent_active",
                Self::Disabled => "surface",
            },
            (Self::Rest, ButtonVariant::Outline) => "surface",
            (Self::Hover, _) => "raised",
            (Self::Pressed, _) => "surface",
            (Self::Rest, v) => v.surface(),
        }
    }
    fn text(&self, variant: &ButtonVariant) -> &'static str {
        match self {
            Self::Disabled => "muted",
            _ if matches!(variant, ButtonVariant::Primary) => "bg",
            Self::Hover => "text",
            Self::Pressed => "text_dim",
            Self::Rest => variant.text(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ButtonOpts {
    pub variant: ButtonVariant,
    pub action: String,
    pub state: ButtonState,
    pub focused: bool,
}

impl ButtonOpts {
    pub fn new(variant: ButtonVariant, action: &str) -> Self {
        Self {
            variant,
            action: action.to_string(),
            state: ButtonState::Rest,
            focused: false,
        }
    }
    pub fn with_state(mut self, state: ButtonState) -> Self {
        self.state = state;
        self
    }
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

/// Labeled button with a named action. The host app owns the action —
/// Blocks never invent behaviour, only declare the intent. The driver
/// supplies interaction state; the composer paints the theme contract.
pub fn button(s: &mut Scope, id: &str, label: &str, o: ButtonOpts) {
    let (size, lh) = hamesh_theme::type_ramp::LABEL;
    let mut c = styled(Role::Button, id, label, "Button");
    // Disabled paint and disabled hit-testing are one decision: a
    // muted button never acts, and automation reports it honestly.
    c.enabled = !matches!(o.state, ButtonState::Disabled);
    c.props.insert("text".to_string(), label.to_string());
    c.props.insert("font_size".to_string(), size.to_string());
    c.props.insert("lh".to_string(), lh.to_string());
    c.props.insert(
        "text_token".to_string(),
        o.state.text(&o.variant).to_string(),
    );
    c.props.insert(
        "surface".to_string(),
        o.state.surface(&o.variant).to_string(),
    );
    c.props
        .insert("radius".to_string(), hamesh_theme::radius::SM.to_string());
    c.props.insert(
        "border".to_string(),
        if o.focused { "2" } else { "1" }.to_string(),
    );
    c.props.insert(
        "border_token".to_string(),
        if o.focused { "focus" } else { "border" }.to_string(),
    );
    c.props
        .insert("h".to_string(), hamesh_theme::control::H.to_string());
    c.props.insert("pad".to_string(), "8".to_string());
    // Internal text alignment: the label centers in the button.
    c.props.insert("halign".to_string(), "center".to_string());
    c.props
        .insert("variant".to_string(), o.variant.as_str().to_string());
    c.props.insert("action".to_string(), o.action.clone());
    s.leaf(c);
    s.app.register_command(&o.action, id);
}

// ---- Design profiles ----

/// One typographic voice: a face, its variation axes, and an
/// optional style. Empty `vars` means the default instance;
/// `style: Some("italic")` voices the italic face through font
/// matching. Faces that are absent on a machine fall back silently
/// (parley/fontdb) and the Eye reports the substitution.
#[derive(Clone, Copy, Debug)]
pub struct Voice {
    pub family: &'static str,
    pub vars: &'static str,
    pub style: Option<&'static str>,
}

/// A design language's type contract: which voice speaks each
/// block kind. Applied as a post-pass so any composition built
/// from blocks can wear a profile without builder churn.
#[derive(Clone, Copy, Debug)]
pub struct Profile {
    /// Display numerals and titles (condensed, authoritative).
    pub display: Voice,
    /// Readouts, labels, annotations (instrumental mono).
    pub technical: Voice,
    /// The guest appearance: subtitles and kickers (humanist
    /// break). `None` leaves them in the default face.
    pub accent: Option<Voice>,
}

/// Signal Cabinet: Barlow Condensed display, IBM Plex Mono
/// technical voice. Condensed bold carries density; mono carries
/// precision. Faces vendored under assets/fonts (OFL).
pub const SIGNAL_CABINET: Profile = Profile {
    display: Voice {
        family: "Barlow Condensed",
        vars: "",
        style: None,
    },
    technical: Voice {
        family: "IBM Plex Mono",
        vars: "",
        style: None,
    },
    accent: None,
};

/// Living Folio: Newsreader at display optical size for titles
/// and numerals, Plex Mono for the margin voice. Optical size
/// does the expressive work — never fake it by scaling up.
pub const LIVING_FOLIO: Profile = Profile {
    display: Voice {
        family: "Newsreader",
        vars: "opsz 72",
        style: None,
    },
    technical: Voice {
        family: "IBM Plex Mono",
        vars: "",
        style: None,
    },
    accent: None,
};

/// Unquiet Grid: Instrument Sans narrowed and weighted for
/// display, Plex Mono technical, Newsreader Italic kicker for
/// the guest appearance. Width contrast, not weight shouting —
/// one family keeps the grid calm, the italic breaks it once.
pub const UNQUIET_GRID: Profile = Profile {
    display: Voice {
        family: "Instrument Sans",
        vars: "wdth 80, wght 650",
        style: None,
    },
    technical: Voice {
        family: "IBM Plex Mono",
        vars: "",
        style: None,
    },
    accent: Some(Voice {
        family: "Newsreader",
        vars: "opsz 16",
        style: Some("italic"),
    }),
};

/// Reapplication contract: a profile owns `font`, `font_vars`
/// and `font_style` on the block kinds it voices (Title,
/// MetricValue, Caption, MetricLabel, Subtle). Reapplying
/// reconciles those three props on every node of a voiced kind
/// with the new profile's desired state — set when voiced,
/// removed when not — so Grid → Cabinet cannot leave `wdth 80`
/// or `italic` behind. Props on other kinds (Button, Badge, Body,
/// Heading, panels…) are never touched: genuine overrides there
/// survive every switch. Overrides on voiced kinds are
/// profile-managed — set them after `apply_profile`, or re-apply
/// to reset. `tracking` is never profile-managed and always
/// survives. The app revision bumps iff a prop actually changed,
/// so revision-dependent caches (frame cache, Eye staleness, drill
/// rev tracking) invalidate exactly when paint can differ — and an
/// idempotent reapply is a no-op down to the revision.
pub fn apply_profile(app: &mut App, p: &Profile) {
    let mut changed = false;
    for c in app.components.values_mut() {
        let kind = c.props.get("block").map(|s| s.as_str());
        let voiced = matches!(
            kind,
            Some("Title")
                | Some("MetricValue")
                | Some("Caption")
                | Some("MetricLabel")
                | Some("Subtle")
        );
        if !voiced {
            continue;
        }
        let v = match kind {
            Some("Title") | Some("MetricValue") => Some(&p.display),
            Some("Caption") | Some("MetricLabel") => Some(&p.technical),
            Some("Subtle") => p.accent.as_ref(),
            _ => None,
        };
        let (font, vars, style) = match v {
            Some(v) => (
                Some(v.family.to_string()),
                (!v.vars.is_empty()).then(|| v.vars.to_string()),
                v.style.map(str::to_string),
            ),
            None => (None, None, None),
        };
        reconcile_prop(c, "font", font, &mut changed);
        reconcile_prop(c, "font_vars", vars, &mut changed);
        reconcile_prop(c, "font_style", style, &mut changed);
    }
    if changed {
        app.bump();
    }
}

/// Reconcile one managed prop with its desired state: remove when
/// `None`, set when different. Reports whether anything changed.
fn reconcile_prop(
    c: &mut hamesh_core::Component,
    key: &str,
    want: Option<String>,
    changed: &mut bool,
) {
    let have = c.props.get(key).cloned();
    if have == want {
        return;
    }
    *changed = true;
    match want {
        Some(v) => {
            c.props.insert(key.to_string(), v);
        }
        None => {
            c.props.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swatch_paints_the_named_surface_token() {
        let mut app = App::new("win");
        {
            let mut s = Scope::root(&mut app, "win");
            swatch(&mut s, "sw", "accent");
        }
        let chip = app.get("sw-chip").unwrap();
        assert_eq!(
            chip.props.get("surface").map(|s| s.as_str()),
            Some("accent")
        );
        assert_eq!(chip.props.get("radius").map(|s| s.as_str()), Some("10"));
        let name = app.get("sw-name").unwrap();
        assert_eq!(name.props.get("text").map(|s| s.as_str()), Some("accent"));
    }

    #[test]
    fn button_states_follow_the_theme_contract() {
        // Every variant paints rest/hover/pressed/disabled from the
        // state contract; focus is a ring, never a fill change.
        fn look(
            variant: ButtonVariant,
            state: ButtonState,
            focused: bool,
        ) -> (String, String, String, String) {
            let mut app = App::new("win");
            {
                let mut s = Scope::root(&mut app, "win");
                button(
                    &mut s,
                    "b",
                    "Go",
                    ButtonOpts::new(variant, "go")
                        .with_state(state)
                        .focused(focused),
                );
            }
            let b = app.get("b").unwrap();
            let p = |k: &str| b.props.get(k).cloned().unwrap_or_default();
            (
                p("surface"),
                p("text_token"),
                p("border"),
                p("border_token"),
            )
        }
        // Primary walks the accent ramp; dark text stays legible.
        assert_eq!(
            look(ButtonVariant::Primary, ButtonState::Rest, false),
            ("accent".into(), "bg".into(), "1".into(), "border".into())
        );
        assert_eq!(
            look(ButtonVariant::Primary, ButtonState::Hover, false).0,
            "accent_hover"
        );
        assert_eq!(
            look(ButtonVariant::Primary, ButtonState::Pressed, false),
            (
                "accent_active".into(),
                "bg".into(),
                "1".into(),
                "border".into()
            )
        );
        // Outline/Ghost rise on hover, settle on press.
        assert_eq!(
            look(ButtonVariant::Outline, ButtonState::Hover, false).0,
            "raised"
        );
        assert_eq!(
            look(ButtonVariant::Outline, ButtonState::Pressed, false).0,
            "surface"
        );
        assert_eq!(
            look(ButtonVariant::Ghost, ButtonState::Rest, false).0,
            "raised"
        );
        // Disabled is uniform and quiet on every variant.
        for v in [
            ButtonVariant::Primary,
            ButtonVariant::Outline,
            ButtonVariant::Ghost,
        ] {
            let (surface, text, _, _) = look(v, ButtonState::Disabled, false);
            assert_eq!((surface.as_str(), text.as_str()), ("surface", "muted"));
        }
        // Focus rings the control without touching the fill.
        let rest = look(ButtonVariant::Outline, ButtonState::Rest, false);
        let focused = look(ButtonVariant::Outline, ButtonState::Rest, true);
        assert_eq!(focused.0, rest.0, "focus never moves paint");
        assert_eq!((focused.2.as_str(), focused.3.as_str()), ("2", "focus"));
    }

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
    fn profile_voices_land_on_the_right_blocks() {
        let mut app = App::new("win");
        {
            let mut s = Scope::root(&mut app, "win");
            card(&mut s, "c", CardOpts::default(), |t| {
                title(t, "t", "Agent X");
                caption(t, "cap", "75%");
                button(t, "b", "Go", ButtonOpts::new(ButtonVariant::Outline, "go"));
            });
        }
        apply_profile(&mut app, &SIGNAL_CABINET);
        let t = app.get("t").unwrap();
        assert_eq!(
            t.props.get("font").map(|s| s.as_str()),
            Some("Barlow Condensed")
        );
        // Static Bold needs no axis settings.
        assert!(!t.props.contains_key("font_vars"));
        let cap = app.get("cap").unwrap();
        assert_eq!(
            cap.props.get("font").map(|s| s.as_str()),
            Some("IBM Plex Mono")
        );
        assert!(!cap.props.contains_key("font_vars"));
        // Buttons keep the default face.
        let b = app.get("b").unwrap();
        assert!(!b.props.contains_key("font"));
        // Idempotent: a second pass changes nothing.
        let before = app.get("t").unwrap().props.clone();
        apply_profile(&mut app, &SIGNAL_CABINET);
        assert_eq!(app.get("t").unwrap().props, before);
    }

    #[test]
    fn profile_switching_leaves_no_stale_typography() {
        // Finding 1 regression: Grid → Cabinet → Folio → Grid on
        // one instance. Each switch must exactly voice the new
        // profile — no stale `font_vars` from the previous voice —
        // while genuine overrides elsewhere survive and the revision
        // bumps iff something actually changed.
        let mut app = App::new("win");
        {
            let mut s = Scope::root(&mut app, "win");
            card(&mut s, "c", CardOpts::default(), |t| {
                title(t, "t", "Agent X");
                caption(t, "cap", "75%");
                button(t, "b", "Go", ButtonOpts::new(ButtonVariant::Outline, "go"));
            });
        }
        // Genuine overrides: a manual face on an unvoiced kind, and
        // a tracking value no profile manages. Both must survive.
        app.get_mut("b")
            .unwrap()
            .props
            .insert("font".into(), "Manual".into());
        app.get_mut("cap")
            .unwrap()
            .props
            .insert("tracking".into(), "2".into());
        let voice = |app: &App| -> (Option<String>, Option<String>) {
            let t = app.get("t").unwrap();
            (
                t.props.get("font").cloned(),
                t.props.get("font_vars").cloned(),
            )
        };
        let r0 = app.revision;
        apply_profile(&mut app, &UNQUIET_GRID);
        assert!(app.revision > r0, "first voicing must bump");
        assert_eq!(
            voice(&app),
            (
                Some("Instrument Sans".to_string()),
                Some("wdth 80, wght 650".to_string())
            )
        );
        let r1 = app.revision;
        // Cabinet voices no axes: Grid's vars must be GONE, not stale.
        apply_profile(&mut app, &SIGNAL_CABINET);
        assert!(app.revision > r1, "voice change must bump");
        assert_eq!(voice(&app), (Some("Barlow Condensed".to_string()), None));
        let r2 = app.revision;
        apply_profile(&mut app, &LIVING_FOLIO);
        assert!(app.revision > r2, "voice change must bump");
        assert_eq!(
            voice(&app),
            (Some("Newsreader".to_string()), Some("opsz 72".to_string()))
        );
        let r3 = app.revision;
        apply_profile(&mut app, &UNQUIET_GRID);
        assert!(app.revision > r3, "voice change must bump");
        assert_eq!(
            voice(&app),
            (
                Some("Instrument Sans".to_string()),
                Some("wdth 80, wght 650".to_string())
            )
        );
        // Idempotent reapply: props AND revision unchanged.
        let (props_before, r4) = (app.get("t").unwrap().props.clone(), app.revision);
        apply_profile(&mut app, &UNQUIET_GRID);
        assert_eq!(app.revision, r4, "idempotent reapply must not bump");
        assert_eq!(app.get("t").unwrap().props, props_before);
        // Overrides survived every switch.
        assert_eq!(
            app.get("b").unwrap().props.get("font").map(|s| s.as_str()),
            Some("Manual")
        );
        assert_eq!(
            app.get("cap")
                .unwrap()
                .props
                .get("tracking")
                .map(|s| s.as_str()),
            Some("2")
        );
    }

    #[test]
    fn profile_switching_clears_accents_and_axes() {
        // Finding 1, style-branch half: Grid voices the Subtle
        // kicker italic; switching to Cabinet (no accent) must
        // remove `font_style`, and switching display voices must
        // swap `font_vars` — never accumulate. Revision bumps iff
        // something changed.
        let mut app = App::new("win");
        {
            let mut s = Scope::root(&mut app, "win");
            card(&mut s, "c", CardOpts::default(), |t| {
                title(t, "t", "Agent X");
                subtle(t, "sub", "overview");
            });
        }
        let r0 = app.revision;
        apply_profile(&mut app, &UNQUIET_GRID);
        assert!(app.revision > r0, "first voicing must bump");
        let sub = app.get("sub").unwrap();
        assert_eq!(
            sub.props.get("font_style").map(|s| s.as_str()),
            Some("italic")
        );
        assert_eq!(
            app.get("t")
                .unwrap()
                .props
                .get("font_vars")
                .map(|s| s.as_str()),
            Some("wdth 80, wght 650")
        );
        // Cabinet has no accent and static display: both the italic
        // and Grid's axes must be GONE, not stale.
        let r1 = app.revision;
        apply_profile(&mut app, &SIGNAL_CABINET);
        assert!(app.revision > r1, "voice change must bump");
        let sub = app.get("sub").unwrap();
        assert!(!sub.props.contains_key("font"), "no accent: no voice");
        assert!(
            !sub.props.contains_key("font_style"),
            "italic must be cleared"
        );
        assert!(!app.get("t").unwrap().props.contains_key("font_vars"));
        // Back to Grid restores the kicker exactly.
        apply_profile(&mut app, &UNQUIET_GRID);
        assert_eq!(
            app.get("sub")
                .unwrap()
                .props
                .get("font_style")
                .map(|s| s.as_str()),
            Some("italic")
        );
        // Idempotent reapply: revision stable.
        let r3 = app.revision;
        apply_profile(&mut app, &UNQUIET_GRID);
        assert_eq!(app.revision, r3, "idempotent reapply must not bump");
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
