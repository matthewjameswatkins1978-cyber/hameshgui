//! hamesh-theme: design tokens for Hamesh Blocks.
//!
//! One dark theme with restrained teal/green accents (decision 020).
//! Components reference semantic token names; the scene resolves them
//! to RGBA. A future light/high-contrast theme changes this table,
//! never component semantics.

/// An sRGB color token: semantic name + RGBA values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub name: &'static str,
    pub rgba: [u8; 4],
}

// Backgrounds and surfaces (charcoal-dark, subtle borders).
pub const BG: Token = Token {
    name: "bg",
    rgba: [10, 15, 17, 255],
};
pub const WINDOW: Token = Token {
    name: "window",
    rgba: [14, 20, 22, 255],
};
pub const SURFACE: Token = Token {
    name: "surface",
    rgba: [30, 42, 46, 255],
};
pub const RAISED: Token = Token {
    name: "raised",
    rgba: [38, 52, 56, 255],
};
pub const HEADER: Token = Token {
    name: "header",
    rgba: [20, 30, 33, 255],
};

// Text: primary, secondary, muted.
pub const TEXT: Token = Token {
    name: "text",
    rgba: [235, 245, 243, 255],
};
pub const TEXT_DIM: Token = Token {
    name: "text_dim",
    rgba: [168, 188, 186, 255],
};
pub const MUTED: Token = Token {
    name: "muted",
    rgba: [122, 142, 140, 255],
};

// Accent + status.
pub const ACCENT: Token = Token {
    name: "accent",
    rgba: [45, 151, 134, 255],
};
pub const ACCENT_BRIGHT: Token = Token {
    name: "accent_bright",
    rgba: [120, 220, 200, 255],
};
pub const SUCCESS: Token = Token {
    name: "success",
    rgba: [74, 178, 128, 255],
};
pub const WARNING: Token = Token {
    name: "warning",
    rgba: [218, 170, 84, 255],
};
pub const ERROR: Token = Token {
    name: "error",
    rgba: [202, 96, 96, 255],
};

// Lines and tracks.
pub const BORDER: Token = Token {
    name: "border",
    rgba: [62, 80, 84, 255],
};
pub const TRACK: Token = Token {
    name: "track",
    rgba: [52, 68, 72, 255],
};

// Focus ring.
pub const FOCUS: Token = Token {
    name: "focus",
    rgba: [120, 220, 200, 255],
};

/// Resolve a semantic token name to RGBA. Unknown names fall back to
/// the surface color so a typo degrades to a visible-but-wrong fill
/// instead of invisible content.
pub fn resolve(name: &str) -> [u8; 4] {
    match name {
        "bg" => BG.rgba,
        "window" => WINDOW.rgba,
        "surface" => SURFACE.rgba,
        "raised" => RAISED.rgba,
        "header" => HEADER.rgba,
        "text" => TEXT.rgba,
        "text_dim" => TEXT_DIM.rgba,
        "muted" => MUTED.rgba,
        "accent" => ACCENT.rgba,
        "accent_bright" => ACCENT_BRIGHT.rgba,
        "success" => SUCCESS.rgba,
        "warning" => WARNING.rgba,
        "error" => ERROR.rgba,
        "border" => BORDER.rgba,
        "track" => TRACK.rgba,
        "focus" => FOCUS.rgba,
        _ => SURFACE.rgba,
    }
}

/// Spacing scale (px). Components use these steps, not magic numbers.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Corner radii (px).
pub mod radius {
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 10.0;
    pub const LG: f32 = 14.0;
}

/// Type ramp: (size px, line height px).
pub mod type_ramp {
    pub const TITLE: (f32, f32) = (28.0, 34.0);
    pub const HEADING: (f32, f32) = (18.0, 24.0);
    pub const BODY: (f32, f32) = (15.0, 21.0);
    pub const LABEL: (f32, f32) = (13.0, 18.0);
    pub const CAPTION: (f32, f32) = (12.0, 16.0);
    pub const METRIC_VALUE: (f32, f32) = (26.0, 32.0);
}

/// Control heights (px).
pub mod control {
    pub const H: f32 = 36.0;
    pub const H_LARGE: f32 = 40.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_tokens_resolve() {
        assert_eq!(resolve("accent"), [45, 151, 134, 255]);
        assert_eq!(resolve("text"), TEXT.rgba);
    }

    #[test]
    fn unknown_token_falls_back_to_surface() {
        assert_eq!(resolve("nope"), SURFACE.rgba);
    }
}
