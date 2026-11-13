//! hamesh-theme: visual styling profiles and design tokens.
//!
//! Provides five distinct visual worlds with coherent design philosophies:
//! 1. Classic Desktop (NeXTSTEP / Windows 2000 utilitarian clarity)
//! 2. Modern Studio (Deep refined dark tones, generative creative suites)
//! 3. Retro Computing (Monospace-first amber/green CRT terminal warmth)
//! 4. Technical Workstation (High-density cold slate engineering instruments)
//! 5. Editorial / Paper (Warm parchment, sharp ink black typography, book-like rules)
//!
//! Tokens decouple component structure from pixel colors and shape geometry.

use serde::{Deserialize, Serialize};

/// An sRGB color token: semantic name + RGBA values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub name: &'static str,
    pub rgba: [u8; 4],
}

/// The five distinct visual design philosophies for Hamesh desktop applications.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum VisualWorld {
    #[default]
    ModernStudio,
    ClassicDesktop,
    RetroComputing,
    TechnicalWorkstation,
    EditorialPaper,
}

impl VisualWorld {
    /// List of all supported visual worlds.
    pub const ALL: [VisualWorld; 5] = [
        VisualWorld::ClassicDesktop,
        VisualWorld::ModernStudio,
        VisualWorld::RetroComputing,
        VisualWorld::TechnicalWorkstation,
        VisualWorld::EditorialPaper,
    ];

    /// Short programmatic identifier.
    pub fn name(&self) -> &'static str {
        match self {
            VisualWorld::ClassicDesktop => "classic",
            VisualWorld::ModernStudio => "modern",
            VisualWorld::RetroComputing => "retro",
            VisualWorld::TechnicalWorkstation => "technical",
            VisualWorld::EditorialPaper => "editorial",
        }
    }

    /// User-facing display title.
    pub fn label(&self) -> &'static str {
        match self {
            VisualWorld::ClassicDesktop => "Classic Desktop",
            VisualWorld::ModernStudio => "Modern Studio",
            VisualWorld::RetroComputing => "Retro Computing",
            VisualWorld::TechnicalWorkstation => "Technical Workstation",
            VisualWorld::EditorialPaper => "Editorial / Paper",
        }
    }

    /// Architectural description of the visual world's philosophy.
    pub fn description(&self) -> &'static str {
        match self {
            VisualWorld::ClassicDesktop => {
                "Utilitarian 90s/2000s desktop with square borders, navy title bars, and dense layouts."
            }
            VisualWorld::ModernStudio => {
                "Refined 2026 dark studio with soft surface elevations, teal accents, and generous breathing room."
            }
            VisualWorld::RetroComputing => {
                "Monospace-first CRT aesthetic with glowing amber phosphor, sharp borders, and mechanical rhythm."
            }
            VisualWorld::TechnicalWorkstation => {
                "High-density cold slate workstation with fluorescent telemetry accents and precision tables."
            }
            VisualWorld::EditorialPaper => {
                "Warm parchment background with sharp ink black serif typography, hairline rules, and book margins."
            }
        }
    }

    /// Recommended primary typography font family for this world.
    pub fn font_family(&self) -> &'static str {
        match self {
            VisualWorld::ClassicDesktop => "Segoe UI",
            VisualWorld::ModernStudio => "Instrument Sans",
            VisualWorld::RetroComputing => "Consolas",
            VisualWorld::TechnicalWorkstation => "Consolas",
            VisualWorld::EditorialPaper => "Georgia",
        }
    }

    /// Corner radius for standard controls (px).
    pub fn corner_radius_sm(&self) -> f32 {
        match self {
            VisualWorld::ClassicDesktop => 0.0,
            VisualWorld::ModernStudio => 6.0,
            VisualWorld::RetroComputing => 0.0,
            VisualWorld::TechnicalWorkstation => 2.0,
            VisualWorld::EditorialPaper => 3.0,
        }
    }

    /// Corner radius for cards and panels (px).
    pub fn corner_radius_md(&self) -> f32 {
        match self {
            VisualWorld::ClassicDesktop => 0.0,
            VisualWorld::ModernStudio => 10.0,
            VisualWorld::RetroComputing => 0.0,
            VisualWorld::TechnicalWorkstation => 4.0,
            VisualWorld::EditorialPaper => 4.0,
        }
    }

    /// Standard border width (px).
    pub fn border_width(&self) -> f32 {
        match self {
            VisualWorld::ClassicDesktop => 1.0,
            VisualWorld::ModernStudio => 1.0,
            VisualWorld::RetroComputing => 1.0,
            VisualWorld::TechnicalWorkstation => 1.0,
            VisualWorld::EditorialPaper => 1.0,
        }
    }

    /// Standard control height (buttons, inputs) in px.
    pub fn control_height(&self) -> f32 {
        match self {
            VisualWorld::ClassicDesktop => 26.0,
            VisualWorld::ModernStudio => 34.0,
            VisualWorld::RetroComputing => 28.0,
            VisualWorld::TechnicalWorkstation => 24.0,
            VisualWorld::EditorialPaper => 36.0,
        }
    }

    /// Standard table / list row height in px.
    pub fn row_height(&self) -> f32 {
        match self {
            VisualWorld::ClassicDesktop => 24.0,
            VisualWorld::ModernStudio => 30.0,
            VisualWorld::RetroComputing => 26.0,
            VisualWorld::TechnicalWorkstation => 22.0,
            VisualWorld::EditorialPaper => 34.0,
        }
    }

    /// Default inter-element gap (px).
    pub fn gap(&self) -> f32 {
        match self {
            VisualWorld::ClassicDesktop => 4.0,
            VisualWorld::ModernStudio => 8.0,
            VisualWorld::RetroComputing => 4.0,
            VisualWorld::TechnicalWorkstation => 4.0,
            VisualWorld::EditorialPaper => 10.0,
        }
    }

    /// Standard padding step (px).
    pub fn padding(&self) -> f32 {
        match self {
            VisualWorld::ClassicDesktop => 6.0,
            VisualWorld::ModernStudio => 12.0,
            VisualWorld::RetroComputing => 6.0,
            VisualWorld::TechnicalWorkstation => 5.0,
            VisualWorld::EditorialPaper => 14.0,
        }
    }

    /// Resolve a semantic token name to RGBA in this visual world.
    pub fn resolve(&self, token_name: &str) -> [u8; 4] {
        resolve_token(*self, token_name)
    }
}

/// Find a visual world by case-insensitive name or alias.
pub fn world_by_name(name: &str) -> Option<VisualWorld> {
    let lower = name.trim().to_lowercase();
    match lower.as_str() {
        "classic" | "classic_desktop" | "classic-desktop" | "windows" | "nextstep" => {
            Some(VisualWorld::ClassicDesktop)
        }
        "modern" | "modern_studio" | "modern-studio" | "studio" | "dark" => {
            Some(VisualWorld::ModernStudio)
        }
        "retro" | "retro_computing" | "retro-computing" | "crt" | "terminal" | "amber" => {
            Some(VisualWorld::RetroComputing)
        }
        "technical" | "technical_workstation" | "technical-workstation" | "cad" | "bloomberg" => {
            Some(VisualWorld::TechnicalWorkstation)
        }
        "editorial" | "editorial_paper" | "editorial-paper" | "paper" | "light" | "swiss" => {
            Some(VisualWorld::EditorialPaper)
        }
        _ => None,
    }
}

// -----------------------------------------------------------------------------
// Modern Studio Tokens (The original default theme)
// -----------------------------------------------------------------------------
pub const MODERN_BG: [u8; 4] = [10, 15, 17, 255];
pub const MODERN_WINDOW: [u8; 4] = [14, 20, 22, 255];
pub const MODERN_SURFACE: [u8; 4] = [30, 42, 46, 255];
pub const MODERN_RAISED: [u8; 4] = [38, 52, 56, 255];
pub const MODERN_HEADER: [u8; 4] = [20, 30, 33, 255];
pub const MODERN_TEXT: [u8; 4] = [235, 245, 243, 255];
pub const MODERN_TEXT_DIM: [u8; 4] = [168, 188, 186, 255];
pub const MODERN_MUTED: [u8; 4] = [122, 142, 140, 255];
pub const MODERN_ACCENT: [u8; 4] = [45, 151, 134, 255];
pub const MODERN_ACCENT_BRIGHT: [u8; 4] = [120, 220, 200, 255];
pub const MODERN_ACCENT_HOVER: [u8; 4] = [66, 174, 156, 255];
pub const MODERN_ACCENT_ACTIVE: [u8; 4] = [38, 142, 124, 255];
pub const MODERN_SUCCESS: [u8; 4] = [74, 178, 128, 255];
pub const MODERN_WARNING: [u8; 4] = [218, 170, 84, 255];
pub const MODERN_ERROR: [u8; 4] = [202, 96, 96, 255];
pub const MODERN_BORDER: [u8; 4] = [62, 80, 84, 255];
pub const MODERN_TRACK: [u8; 4] = [52, 68, 72, 255];
pub const MODERN_FOCUS: [u8; 4] = [120, 220, 200, 255];

// -----------------------------------------------------------------------------
// Classic Desktop Tokens (Utilitarian Windows 2000 / NeXTSTEP)
// -----------------------------------------------------------------------------
pub const CLASSIC_BG: [u8; 4] = [192, 192, 192, 255];
pub const CLASSIC_WINDOW: [u8; 4] = [212, 208, 200, 255];
pub const CLASSIC_SURFACE: [u8; 4] = [192, 192, 192, 255];
pub const CLASSIC_RAISED: [u8; 4] = [224, 224, 224, 255];
pub const CLASSIC_HEADER: [u8; 4] = [0, 0, 128, 255]; // Classic Navy Blue title bar
pub const CLASSIC_TEXT: [u8; 4] = [0, 0, 0, 255];
pub const CLASSIC_TEXT_DIM: [u8; 4] = [64, 64, 64, 255];
pub const CLASSIC_MUTED: [u8; 4] = [128, 128, 128, 255];
pub const CLASSIC_ACCENT: [u8; 4] = [0, 0, 128, 255]; // Classic Navy Accent
pub const CLASSIC_ACCENT_BRIGHT: [u8; 4] = [16, 32, 160, 255];
pub const CLASSIC_ACCENT_HOVER: [u8; 4] = [24, 48, 180, 255];
pub const CLASSIC_ACCENT_ACTIVE: [u8; 4] = [0, 0, 96, 255];
pub const CLASSIC_SUCCESS: [u8; 4] = [0, 128, 0, 255];
pub const CLASSIC_WARNING: [u8; 4] = [180, 120, 0, 255];
pub const CLASSIC_ERROR: [u8; 4] = [192, 0, 0, 255];
pub const CLASSIC_BORDER: [u8; 4] = [128, 128, 128, 255];
pub const CLASSIC_TRACK: [u8; 4] = [160, 160, 160, 255];
pub const CLASSIC_FOCUS: [u8; 4] = [0, 0, 0, 255];

// -----------------------------------------------------------------------------
// Retro Computing Tokens (Phosphor Amber CRT Terminal)
// -----------------------------------------------------------------------------
pub const RETRO_BG: [u8; 4] = [12, 14, 18, 255];
pub const RETRO_WINDOW: [u8; 4] = [18, 22, 26, 255];
pub const RETRO_SURFACE: [u8; 4] = [24, 30, 36, 255];
pub const RETRO_RAISED: [u8; 4] = [34, 42, 50, 255];
pub const RETRO_HEADER: [u8; 4] = [14, 18, 22, 255];
pub const RETRO_TEXT: [u8; 4] = [255, 176, 0, 255]; // Amber Phosphor P3
pub const RETRO_TEXT_DIM: [u8; 4] = [204, 140, 0, 255];
pub const RETRO_MUTED: [u8; 4] = [130, 90, 0, 255];
pub const RETRO_ACCENT: [u8; 4] = [255, 176, 0, 255];
pub const RETRO_ACCENT_BRIGHT: [u8; 4] = [255, 210, 64, 255];
pub const RETRO_ACCENT_HOVER: [u8; 4] = [255, 196, 48, 255];
pub const RETRO_ACCENT_ACTIVE: [u8; 4] = [210, 140, 0, 255];
pub const RETRO_SUCCESS: [u8; 4] = [51, 255, 51, 255]; // Phosphor Green P1
pub const RETRO_WARNING: [u8; 4] = [255, 176, 0, 255];
pub const RETRO_ERROR: [u8; 4] = [255, 68, 68, 255];
pub const RETRO_BORDER: [u8; 4] = [90, 65, 10, 255];
pub const RETRO_TRACK: [u8; 4] = [40, 30, 10, 255];
pub const RETRO_FOCUS: [u8; 4] = [255, 210, 64, 255];

// -----------------------------------------------------------------------------
// Technical Workstation Tokens (Cold Slate / Engineering Telemetry)
// -----------------------------------------------------------------------------
pub const TECH_BG: [u8; 4] = [16, 20, 24, 255];
pub const TECH_WINDOW: [u8; 4] = [22, 26, 30, 255];
pub const TECH_SURFACE: [u8; 4] = [32, 38, 44, 255];
pub const TECH_RAISED: [u8; 4] = [44, 52, 60, 255];
pub const TECH_HEADER: [u8; 4] = [26, 32, 38, 255];
pub const TECH_TEXT: [u8; 4] = [240, 244, 248, 255];
pub const TECH_TEXT_DIM: [u8; 4] = [160, 172, 184, 255];
pub const TECH_MUTED: [u8; 4] = [110, 122, 134, 255];
pub const TECH_ACCENT: [u8; 4] = [0, 230, 118, 255]; // Fluorescent Status Green
pub const TECH_ACCENT_BRIGHT: [u8; 4] = [105, 255, 174, 255];
pub const TECH_ACCENT_HOVER: [u8; 4] = [51, 235, 145, 255];
pub const TECH_ACCENT_ACTIVE: [u8; 4] = [0, 185, 94, 255];
pub const TECH_SUCCESS: [u8; 4] = [0, 230, 118, 255];
pub const TECH_WARNING: [u8; 4] = [255, 171, 0, 255]; // Amber Alert
pub const TECH_ERROR: [u8; 4] = [255, 61, 0, 255]; // Safety Orange/Red
pub const TECH_BORDER: [u8; 4] = [60, 72, 84, 255];
pub const TECH_TRACK: [u8; 4] = [40, 48, 56, 255];
pub const TECH_FOCUS: [u8; 4] = [0, 230, 118, 255];

// -----------------------------------------------------------------------------
// Editorial / Paper Tokens (Warm Parchment / Swiss Typography)
// -----------------------------------------------------------------------------
pub const EDIT_BG: [u8; 4] = [242, 238, 230, 255];
pub const EDIT_WINDOW: [u8; 4] = [248, 246, 240, 255];
pub const EDIT_SURFACE: [u8; 4] = [255, 254, 250, 255];
pub const EDIT_RAISED: [u8; 4] = [238, 234, 224, 255];
pub const EDIT_HEADER: [u8; 4] = [242, 238, 230, 255];
pub const EDIT_TEXT: [u8; 4] = [28, 28, 30, 255]; // Ink black
pub const EDIT_TEXT_DIM: [u8; 4] = [75, 75, 80, 255];
pub const EDIT_MUTED: [u8; 4] = [130, 128, 125, 255];
pub const EDIT_ACCENT: [u8; 4] = [142, 36, 54, 255]; // Deep Burgundy
pub const EDIT_ACCENT_BRIGHT: [u8; 4] = [180, 50, 72, 255];
pub const EDIT_ACCENT_HOVER: [u8; 4] = [160, 44, 64, 255];
pub const EDIT_ACCENT_ACTIVE: [u8; 4] = [120, 28, 44, 255];
pub const EDIT_SUCCESS: [u8; 4] = [46, 125, 84, 255];
pub const EDIT_WARNING: [u8; 4] = [194, 110, 12, 255];
pub const EDIT_ERROR: [u8; 4] = [198, 40, 40, 255];
pub const EDIT_BORDER: [u8; 4] = [216, 210, 198, 255];
pub const EDIT_TRACK: [u8; 4] = [226, 222, 212, 255];
pub const EDIT_FOCUS: [u8; 4] = [142, 36, 54, 255];

/// Resolve a semantic token name for a specific visual world.
pub fn resolve_token(world: VisualWorld, name: &str) -> [u8; 4] {
    match world {
        VisualWorld::ClassicDesktop => match name {
            "bg" => CLASSIC_BG,
            "window" => CLASSIC_WINDOW,
            "surface" => CLASSIC_SURFACE,
            "raised" => CLASSIC_RAISED,
            "header" => CLASSIC_HEADER,
            "text" => CLASSIC_TEXT,
            "text_dim" => CLASSIC_TEXT_DIM,
            "muted" => CLASSIC_MUTED,
            "accent" => CLASSIC_ACCENT,
            "accent_bright" => CLASSIC_ACCENT_BRIGHT,
            "accent_hover" => CLASSIC_ACCENT_HOVER,
            "accent_active" => CLASSIC_ACCENT_ACTIVE,
            "success" => CLASSIC_SUCCESS,
            "warning" => CLASSIC_WARNING,
            "error" => CLASSIC_ERROR,
            "border" => CLASSIC_BORDER,
            "track" => CLASSIC_TRACK,
            "focus" => CLASSIC_FOCUS,
            "highlight" => [255, 255, 255, 255],
            "shadow" => [64, 64, 64, 255],
            _ => CLASSIC_SURFACE,
        },
        VisualWorld::ModernStudio => match name {
            "bg" => MODERN_BG,
            "window" => MODERN_WINDOW,
            "surface" => MODERN_SURFACE,
            "raised" => MODERN_RAISED,
            "header" => MODERN_HEADER,
            "text" => MODERN_TEXT,
            "text_dim" => MODERN_TEXT_DIM,
            "muted" => MODERN_MUTED,
            "accent" => MODERN_ACCENT,
            "accent_bright" => MODERN_ACCENT_BRIGHT,
            "accent_hover" => MODERN_ACCENT_HOVER,
            "accent_active" => MODERN_ACCENT_ACTIVE,
            "success" => MODERN_SUCCESS,
            "warning" => MODERN_WARNING,
            "error" => MODERN_ERROR,
            "border" => MODERN_BORDER,
            "track" => MODERN_TRACK,
            "focus" => MODERN_FOCUS,
            "highlight" => MODERN_ACCENT_BRIGHT,
            "shadow" => MODERN_BG,
            _ => MODERN_SURFACE,
        },
        VisualWorld::RetroComputing => match name {
            "bg" => RETRO_BG,
            "window" => RETRO_WINDOW,
            "surface" => RETRO_SURFACE,
            "raised" => RETRO_RAISED,
            "header" => RETRO_HEADER,
            "text" => RETRO_TEXT,
            "text_dim" => RETRO_TEXT_DIM,
            "muted" => RETRO_MUTED,
            "accent" => RETRO_ACCENT,
            "accent_bright" => RETRO_ACCENT_BRIGHT,
            "accent_hover" => RETRO_ACCENT_HOVER,
            "accent_active" => RETRO_ACCENT_ACTIVE,
            "success" => RETRO_SUCCESS,
            "warning" => RETRO_WARNING,
            "error" => RETRO_ERROR,
            "border" => RETRO_BORDER,
            "track" => RETRO_TRACK,
            "focus" => RETRO_FOCUS,
            "highlight" => RETRO_ACCENT_BRIGHT,
            "shadow" => RETRO_BG,
            _ => RETRO_SURFACE,
        },
        VisualWorld::TechnicalWorkstation => match name {
            "bg" => TECH_BG,
            "window" => TECH_WINDOW,
            "surface" => TECH_SURFACE,
            "raised" => TECH_RAISED,
            "header" => TECH_HEADER,
            "text" => TECH_TEXT,
            "text_dim" => TECH_TEXT_DIM,
            "muted" => TECH_MUTED,
            "accent" => TECH_ACCENT,
            "accent_bright" => TECH_ACCENT_BRIGHT,
            "accent_hover" => TECH_ACCENT_HOVER,
            "accent_active" => TECH_ACCENT_ACTIVE,
            "success" => TECH_SUCCESS,
            "warning" => TECH_WARNING,
            "error" => TECH_ERROR,
            "border" => TECH_BORDER,
            "track" => TECH_TRACK,
            "focus" => TECH_FOCUS,
            "highlight" => TECH_ACCENT_BRIGHT,
            "shadow" => TECH_BG,
            _ => TECH_SURFACE,
        },
        VisualWorld::EditorialPaper => match name {
            "bg" => EDIT_BG,
            "window" => EDIT_WINDOW,
            "surface" => EDIT_SURFACE,
            "raised" => EDIT_RAISED,
            "header" => EDIT_HEADER,
            "text" => EDIT_TEXT,
            "text_dim" => EDIT_TEXT_DIM,
            "muted" => EDIT_MUTED,
            "accent" => EDIT_ACCENT,
            "accent_bright" => EDIT_ACCENT_BRIGHT,
            "accent_hover" => EDIT_ACCENT_HOVER,
            "accent_active" => EDIT_ACCENT_ACTIVE,
            "success" => EDIT_SUCCESS,
            "warning" => EDIT_WARNING,
            "error" => EDIT_ERROR,
            "border" => EDIT_BORDER,
            "track" => EDIT_TRACK,
            "focus" => EDIT_FOCUS,
            "highlight" => [255, 255, 255, 255],
            "shadow" => EDIT_BORDER,
            _ => EDIT_SURFACE,
        },
    }
}

/// Backward-compatible resolution function defaulting to ModernStudio.
pub fn resolve(name: &str) -> [u8; 4] {
    resolve_token(VisualWorld::ModernStudio, name)
}

/// Fallback role color resolution per visual world.
pub fn role_color(world: VisualWorld, role: &str, name: &str) -> [u8; 4] {
    let lower_name = name.to_lowercase();
    let is_primary = lower_name.contains("primary") || lower_name.contains("increment");

    match role {
        s if s.contains("Window") => resolve_token(world, "window"),
        s if s.contains("Header") => resolve_token(world, "header"),
        s if s.contains("Button") => {
            if is_primary {
                resolve_token(world, "accent")
            } else {
                resolve_token(world, "raised")
            }
        }
        s if s.contains("Overlay") => match world {
            VisualWorld::ClassicDesktop => [224, 224, 224, 240],
            VisualWorld::EditorialPaper => [255, 254, 250, 245],
            _ => [38, 52, 56, 240],
        },
        s if s.contains("Text") || s.contains("Status") => [0, 0, 0, 0],
        _ => resolve_token(world, "surface"),
    }
}

// -----------------------------------------------------------------------------
// Legacy constants for backward compatibility
// -----------------------------------------------------------------------------
pub const BG: Token = Token {
    name: "bg",
    rgba: MODERN_BG,
};
pub const WINDOW: Token = Token {
    name: "window",
    rgba: MODERN_WINDOW,
};
pub const SURFACE: Token = Token {
    name: "surface",
    rgba: MODERN_SURFACE,
};
pub const RAISED: Token = Token {
    name: "raised",
    rgba: MODERN_RAISED,
};
pub const HEADER: Token = Token {
    name: "header",
    rgba: MODERN_HEADER,
};
pub const TEXT: Token = Token {
    name: "text",
    rgba: MODERN_TEXT,
};
pub const TEXT_DIM: Token = Token {
    name: "text_dim",
    rgba: MODERN_TEXT_DIM,
};
pub const MUTED: Token = Token {
    name: "muted",
    rgba: MODERN_MUTED,
};
pub const ACCENT: Token = Token {
    name: "accent",
    rgba: MODERN_ACCENT,
};
pub const ACCENT_BRIGHT: Token = Token {
    name: "accent_bright",
    rgba: MODERN_ACCENT_BRIGHT,
};
pub const ACCENT_HOVER: Token = Token {
    name: "accent_hover",
    rgba: MODERN_ACCENT_HOVER,
};
pub const ACCENT_ACTIVE: Token = Token {
    name: "accent_active",
    rgba: MODERN_ACCENT_ACTIVE,
};
pub const SUCCESS: Token = Token {
    name: "success",
    rgba: MODERN_SUCCESS,
};
pub const WARNING: Token = Token {
    name: "warning",
    rgba: MODERN_WARNING,
};
pub const ERROR: Token = Token {
    name: "error",
    rgba: MODERN_ERROR,
};
pub const BORDER: Token = Token {
    name: "border",
    rgba: MODERN_BORDER,
};
pub const TRACK: Token = Token {
    name: "track",
    rgba: MODERN_TRACK,
};
pub const FOCUS: Token = Token {
    name: "focus",
    rgba: MODERN_FOCUS,
};

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
    fn known_tokens_resolve_default() {
        assert_eq!(resolve("accent"), [45, 151, 134, 255]);
        assert_eq!(resolve("text"), TEXT.rgba);
    }

    #[test]
    fn unknown_token_falls_back_to_surface() {
        assert_eq!(resolve("nope"), SURFACE.rgba);
    }

    #[test]
    fn all_five_visual_worlds_resolve_core_tokens() {
        for world in VisualWorld::ALL {
            for token in [
                "bg", "window", "surface", "raised", "text", "accent", "border",
            ] {
                let rgba = resolve_token(world, token);
                assert_eq!(rgba[3], 255, "{:?} {} alpha must be 255", world, token);
            }
        }
    }

    #[test]
    fn world_by_name_parsing() {
        assert_eq!(world_by_name("classic"), Some(VisualWorld::ClassicDesktop));
        assert_eq!(world_by_name("modern"), Some(VisualWorld::ModernStudio));
        assert_eq!(world_by_name("retro"), Some(VisualWorld::RetroComputing));
        assert_eq!(
            world_by_name("technical"),
            Some(VisualWorld::TechnicalWorkstation)
        );
        assert_eq!(
            world_by_name("editorial"),
            Some(VisualWorld::EditorialPaper)
        );
        assert_eq!(world_by_name("unknown"), None);
    }

    fn luminance([r, g, b, _]: [u8; 4]) -> f32 {
        let f = |c: u8| {
            let v = c as f32 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
    }

    fn contrast(a: [u8; 4], b: [u8; 4]) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn text_pairs_meet_wcag_aa_modern() {
        for (fg, bg, min) in [
            (TEXT.rgba, SURFACE.rgba, 4.5),
            (TEXT.rgba, RAISED.rgba, 4.5),
            (TEXT_DIM.rgba, RAISED.rgba, 4.5),
            (MUTED.rgba, RAISED.rgba, 3.0),
            (SUCCESS.rgba, RAISED.rgba, 3.0),
            (TEXT.rgba, ACCENT.rgba, 3.0),
            (BG.rgba, ACCENT.rgba, 4.5),
            (BG.rgba, ACCENT_HOVER.rgba, 4.5),
            (BG.rgba, ACCENT_ACTIVE.rgba, 4.5),
            (MUTED.rgba, SURFACE.rgba, 3.0),
        ] {
            let c = contrast(fg, bg);
            assert!(c >= min, "contrast {c:.2} < {min} for {fg:?} on {bg:?}");
        }
    }

    #[test]
    fn text_pairs_meet_wcag_classic_and_editorial() {
        // Classic Desktop (black on light gray)
        let c_classic = contrast(CLASSIC_TEXT, CLASSIC_SURFACE);
        assert!(c_classic >= 4.5, "Classic text contrast: {c_classic:.2}");

        // Editorial Paper (ink black on paper cream)
        let c_edit = contrast(EDIT_TEXT, EDIT_SURFACE);
        assert!(c_edit >= 4.5, "Editorial text contrast: {c_edit:.2}");

        // Technical Workstation (white on cold slate)
        let c_tech = contrast(TECH_TEXT, TECH_SURFACE);
        assert!(c_tech >= 4.5, "Technical text contrast: {c_tech:.2}");

        // Retro Computing (amber on CRT black)
        let c_retro = contrast(RETRO_TEXT, RETRO_SURFACE);
        assert!(c_retro >= 4.5, "Retro text contrast: {c_retro:.2}");
    }
}
