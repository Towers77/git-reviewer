//! The app's colors, worked out at runtime from three picks: a background, a text color
//! and an accent (`ThemeSettings`, saved in `state.toml`).
//!
//! Every other shade comes from those, the way Catppuccin builds its own ramp (to
//! within one step per channel, since Catppuccin hand-tuned a few values): lines,
//! hovers and secondary text are steps from the background toward the text, and the
//! darker panels are the background toward black (toward the text on a light one).
//! The status colors (red, yellow, green, blue) and the repo colors are Catppuccin
//! accents kept by name, resolved to their Mocha value on a dark background and their
//! Latte value on a light one, so they stay readable on either.

use std::fmt;
use std::str::FromStr;

use iced::Color;
use serde::{Deserialize, Serialize};

/// An opaque sRGB color, written as a hex code (`"#1e1e2e"`) in `state.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    const BLACK: Rgb = Rgb(0, 0, 0);
    const WHITE: Rgb = Rgb(255, 255, 255);

    /// This color moved `amount` (0 to 1) of the way toward `other`, channel by channel,
    /// rounded to whole channel values like a hex code.
    pub fn mix(self, other: Rgb, amount: f32) -> Rgb {
        let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
        Rgb(
            channel(self.0, other.0),
            channel(self.1, other.1),
            channel(self.2, other.2),
        )
    }

    /// WCAG relative luminance: 0 for black, 1 for white.
    pub fn luminance(self) -> f32 {
        let linear = |channel: u8| {
            let s = channel as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2)
    }

    pub fn color(self) -> Color {
        Color::from_rgb8(self.0, self.1, self.2)
    }

    /// This color, see-through: `alpha` 0 is invisible, 1 opaque.
    pub fn alpha(self, alpha: f32) -> Color {
        Color::from_rgba8(self.0, self.1, self.2, alpha)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRgbError(String);

impl fmt::Display for ParseRgbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\" is not a hex color like #1e1e2e", self.0)
    }
}

impl FromStr for Rgb {
    type Err = ParseRgbError;

    /// `#1e1e2e` or `1e1e2e`, in either case.
    fn from_str(text: &str) -> Result<Rgb, ParseRgbError> {
        let error = || ParseRgbError(text.to_string());
        let digits = text.trim().strip_prefix('#').unwrap_or(text.trim());
        if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(error());
        }
        let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).map_err(|_| error());
        Ok(Rgb(channel(0)?, channel(2)?, channel(4)?))
    }
}

impl TryFrom<String> for Rgb {
    type Error = ParseRgbError;

    fn try_from(text: String) -> Result<Rgb, ParseRgbError> {
        text.parse()
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl From<Rgb> for String {
    fn from(rgb: Rgb) -> String {
        rgb.to_string()
    }
}

/// Catppuccin's 14 accents. Every flavor has the same names; Latte's are darker and
/// stronger so they read on a light background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Accent {
    Rosewater,
    Flamingo,
    Pink,
    Mauve,
    Red,
    Maroon,
    Peach,
    Yellow,
    Green,
    Teal,
    Sky,
    Sapphire,
    Blue,
    Lavender,
}

impl Accent {
    pub const ALL: [Accent; 14] = [
        Accent::Rosewater,
        Accent::Flamingo,
        Accent::Pink,
        Accent::Mauve,
        Accent::Red,
        Accent::Maroon,
        Accent::Peach,
        Accent::Yellow,
        Accent::Green,
        Accent::Teal,
        Accent::Sky,
        Accent::Sapphire,
        Accent::Blue,
        Accent::Lavender,
    ];

    /// The colors a repo's badge can take: all but red, which means deleted or gone.
    pub const REPO: [Accent; 13] = [
        Accent::Rosewater,
        Accent::Flamingo,
        Accent::Pink,
        Accent::Mauve,
        Accent::Maroon,
        Accent::Peach,
        Accent::Yellow,
        Accent::Green,
        Accent::Teal,
        Accent::Sky,
        Accent::Sapphire,
        Accent::Blue,
        Accent::Lavender,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Accent::Rosewater => "Rosewater",
            Accent::Flamingo => "Flamingo",
            Accent::Pink => "Pink",
            Accent::Mauve => "Mauve",
            Accent::Red => "Red",
            Accent::Maroon => "Maroon",
            Accent::Peach => "Peach",
            Accent::Yellow => "Yellow",
            Accent::Green => "Green",
            Accent::Teal => "Teal",
            Accent::Sky => "Sky",
            Accent::Sapphire => "Sapphire",
            Accent::Blue => "Blue",
            Accent::Lavender => "Lavender",
        }
    }

    /// The value on a dark background (Catppuccin Mocha's).
    pub const fn mocha(self) -> Rgb {
        match self {
            Accent::Rosewater => Rgb(0xf5, 0xe0, 0xdc),
            Accent::Flamingo => Rgb(0xf2, 0xcd, 0xcd),
            Accent::Pink => Rgb(0xf5, 0xc2, 0xe7),
            Accent::Mauve => Rgb(0xcb, 0xa6, 0xf7),
            Accent::Red => Rgb(0xf3, 0x8b, 0xa8),
            Accent::Maroon => Rgb(0xeb, 0xa0, 0xac),
            Accent::Peach => Rgb(0xfa, 0xb3, 0x87),
            Accent::Yellow => Rgb(0xf9, 0xe2, 0xaf),
            Accent::Green => Rgb(0xa6, 0xe3, 0xa1),
            Accent::Teal => Rgb(0x94, 0xe2, 0xd5),
            Accent::Sky => Rgb(0x89, 0xdc, 0xeb),
            Accent::Sapphire => Rgb(0x74, 0xc7, 0xec),
            Accent::Blue => Rgb(0x89, 0xb4, 0xfa),
            Accent::Lavender => Rgb(0xb4, 0xbe, 0xfe),
        }
    }

    /// The value on a light background (Catppuccin Latte's).
    pub const fn latte(self) -> Rgb {
        match self {
            Accent::Rosewater => Rgb(0xdc, 0x8a, 0x78),
            Accent::Flamingo => Rgb(0xdd, 0x78, 0x78),
            Accent::Pink => Rgb(0xea, 0x76, 0xcb),
            Accent::Mauve => Rgb(0x88, 0x39, 0xef),
            Accent::Red => Rgb(0xd2, 0x0f, 0x39),
            Accent::Maroon => Rgb(0xe6, 0x45, 0x53),
            Accent::Peach => Rgb(0xfe, 0x64, 0x0b),
            Accent::Yellow => Rgb(0xdf, 0x8e, 0x1d),
            Accent::Green => Rgb(0x40, 0xa0, 0x2b),
            Accent::Teal => Rgb(0x17, 0x92, 0x99),
            Accent::Sky => Rgb(0x04, 0xa5, 0xe5),
            Accent::Sapphire => Rgb(0x20, 0x9f, 0xb5),
            Accent::Blue => Rgb(0x1e, 0x66, 0xf5),
            Accent::Lavender => Rgb(0x72, 0x87, 0xfd),
        }
    }

    fn resolve(self, light: bool) -> Rgb {
        if light { self.latte() } else { self.mocha() }
    }
}

/// The accent the user picked: one of Catppuccin's by name, which follows the
/// background between dark and light, or a typed hex code, which is used as typed.
/// In `state.toml` it's either the name (`"Mauve"`) or the code (`"#ff8800"`); no name
/// looks like a hex code, so the two can't be confused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum AccentChoice {
    Named(Accent),
    Custom(Rgb),
}

impl TryFrom<String> for AccentChoice {
    type Error = String;

    fn try_from(text: String) -> Result<AccentChoice, String> {
        if let Some(accent) = Accent::ALL.into_iter().find(|a| a.name() == text) {
            return Ok(AccentChoice::Named(accent));
        }
        text.parse().map(AccentChoice::Custom).map_err(|_| {
            format!("\"{text}\" is neither a Catppuccin accent (like \"Mauve\") nor a hex color")
        })
    }
}

impl From<AccentChoice> for String {
    fn from(choice: AccentChoice) -> String {
        match choice {
            AccentChoice::Named(accent) => accent.name().to_string(),
            AccentChoice::Custom(rgb) => rgb.to_string(),
        }
    }
}

/// The three colors the user picks; `Colors::new` works out the rest. The Neutral preset
/// with mauve until changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    pub background: Rgb,
    pub text: Rgb,
    pub accent: AccentChoice,
}

impl Default for ThemeSettings {
    fn default() -> ThemeSettings {
        let neutral = PRESETS[0];
        ThemeSettings {
            background: neutral.background,
            text: neutral.text,
            accent: AccentChoice::Named(Accent::Mauve),
        }
    }
}

/// A background and the text color made for it, picked together in the theme menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    pub background: Rgb,
    pub text: Rgb,
}

/// Darkest first: Neutral (the app's colors from before themes, and the default), then
/// Catppuccin's four flavors.
pub const PRESETS: [Preset; 5] = [
    Preset {
        name: "Neutral",
        background: Rgb(0x18, 0x19, 0x1c),
        text: Rgb(0xe6, 0xe6, 0xe9),
    },
    Preset {
        name: "Mocha",
        background: Rgb(0x1e, 0x1e, 0x2e),
        text: Rgb(0xcd, 0xd6, 0xf4),
    },
    Preset {
        name: "Macchiato",
        background: Rgb(0x24, 0x27, 0x3a),
        text: Rgb(0xca, 0xd3, 0xf5),
    },
    Preset {
        name: "Frappé",
        background: Rgb(0x30, 0x34, 0x46),
        text: Rgb(0xc6, 0xd0, 0xf5),
    },
    Preset {
        name: "Latte",
        background: Rgb(0xef, 0xf1, 0xf5),
        text: Rgb(0x4c, 0x4f, 0x69),
    },
];

/// Above this luminance a background counts as light: the accents switch to Latte's
/// and the darker panels mix toward the text instead of black.
const LIGHT_BACKGROUND: f32 = 0.4;
/// Above this luminance the accent is light enough to carry dark text; below it, white.
const LIGHT_ACCENT: f32 = 0.35;
/// How far the commit button's accent moves toward white under the mouse.
const ACCENT_HOVER: f32 = 0.15;
/// How strong a chip's background is: its text color, see-through.
const CHIP_ALPHA: f32 = 0.16;
/// The same for a repo's badge, a step stronger so its initials read as one block.
pub const BADGE_ALPHA: f32 = 0.18;

/// Every color the UI draws with, by role. Built once per theme change and copied into
/// the style functions, so nothing here is worked out per frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    /// A light background: Latte accents and Latte syntax colors.
    pub light: bool,

    // Surfaces, from the background.
    /// The diff and the commit box: the background itself (Catppuccin's base).
    pub background: Color,
    /// The sidebar and the changes panel (mantle).
    pub panel: Color,
    /// The top bar (crust).
    pub top_bar: Color,

    // Steps from the background toward the text.
    /// Panel dividers and the error toast's card (11%, surface0).
    pub border: Color,
    /// Rows and buttons under the mouse (11%, surface0).
    pub hover: Color,
    /// Button outlines and filled pills (22%, surface1).
    pub control_border: Color,
    pub pill: Color,
    /// A scrollbar thumb under the mouse (33%, surface2).
    pub scrollbar_hover: Color,
    /// The diff's line numbers (44.6%, overlay0).
    pub line_number: Color,
    /// Third-level text: folder paths, counts (55.4%, overlay1).
    pub text_dim: Color,
    /// Labels and secondary text (77.7%, subtext0).
    pub text_muted: Color,
    pub text: Color,

    // The accent.
    /// Worktree icons, the commit box's focus border and the commit button.
    pub accent: Color,
    /// The commit button under the mouse: the accent a step lighter, on any theme.
    pub accent_hover: Color,
    /// Text and icons on the accent: dark on a light accent, white on a dark one.
    pub on_accent: Color,
    /// The selected worktree and file rows: the accent over the panel. Opaque, since a
    /// row's hover buttons sit on a patch of it that must hide the name under them.
    pub selected: Color,
    /// A button hovered on top of a selected row: the same accent, a step stronger.
    pub selected_hover: Color,

    // Status colors.
    pub danger: Color,
    pub danger_border: Color,
    pub modified: Color,
    pub added: Color,
    /// A confirmation toast's border, like `danger_border` for an error's.
    pub added_border: Color,
    pub renamed: Color,
    pub deleted: Color,
    /// Behind count chips and the diff's Staged/Unstaged tag: the status color, faint,
    /// behind text in the color itself. Yellow for changes, green for staged ones.
    pub modified_chip: Color,
    pub added_chip: Color,
    /// Diff rows: the status color over the background.
    pub added_tint: Color,
    pub removed_tint: Color,
    /// The diff's line-number gutter on changed rows. Opaque, unlike the tints: the
    /// sticky gutter is drawn over the code when the diff scrolls sideways.
    pub added_gutter: Color,
    pub removed_gutter: Color,
}

impl Colors {
    pub fn new(settings: &ThemeSettings) -> Colors {
        let background = settings.background;
        let text = settings.text;
        let light = background.luminance() > LIGHT_BACKGROUND;
        let named = |accent: Accent| accent.resolve(light);
        let step = |amount: f32| background.mix(text, amount).color();

        let (panel, top_bar) = if light {
            (background.mix(text, 0.06), background.mix(text, 0.12))
        } else {
            (
                background.mix(Rgb::BLACK, 0.2),
                background.mix(Rgb::BLACK, 0.42),
            )
        };
        let accent = match settings.accent {
            AccentChoice::Named(accent) => named(accent),
            AccentChoice::Custom(rgb) => rgb,
        };
        // The theme's darkest color: the top bar on a dark theme, the text on a light one.
        let on_accent = if accent.luminance() > LIGHT_ACCENT {
            if light { text.color() } else { top_bar.color() }
        } else {
            Color::WHITE
        };
        let red = named(Accent::Red);
        let green = named(Accent::Green);
        let yellow = named(Accent::Yellow);

        Colors {
            light,
            background: background.color(),
            panel: panel.color(),
            top_bar: top_bar.color(),
            border: step(0.11),
            hover: step(0.11),
            control_border: step(0.22),
            pill: step(0.22),
            scrollbar_hover: step(0.33),
            line_number: step(0.446),
            text_dim: step(0.554),
            text_muted: step(0.777),
            text: text.color(),
            accent: accent.color(),
            accent_hover: accent.mix(Rgb::WHITE, ACCENT_HOVER).color(),
            on_accent,
            selected: panel.mix(accent, 0.16).color(),
            selected_hover: panel.mix(accent, 0.3).color(),
            danger: red.color(),
            danger_border: red.alpha(0.35),
            modified: yellow.color(),
            added: green.color(),
            added_border: green.alpha(0.35),
            renamed: named(Accent::Blue).color(),
            deleted: red.color(),
            modified_chip: yellow.alpha(CHIP_ALPHA),
            added_chip: green.alpha(CHIP_ALPHA),
            added_tint: green.alpha(0.12),
            removed_tint: red.alpha(0.12),
            added_gutter: background.mix(green, 0.2).color(),
            removed_gutter: background.mix(red, 0.2).color(),
        }
    }

    /// Every color made `opacity` as solid (1 unchanged, 0 invisible): a toast drawn
    /// with them fades in or out as a whole, since iced has no opacity of its own.
    pub fn faded(&self, opacity: f32) -> Colors {
        let f = |color: Color| color.scale_alpha(opacity);
        Colors {
            light: self.light,
            background: f(self.background),
            panel: f(self.panel),
            top_bar: f(self.top_bar),
            border: f(self.border),
            hover: f(self.hover),
            control_border: f(self.control_border),
            pill: f(self.pill),
            scrollbar_hover: f(self.scrollbar_hover),
            line_number: f(self.line_number),
            text_dim: f(self.text_dim),
            text_muted: f(self.text_muted),
            text: f(self.text),
            accent: f(self.accent),
            accent_hover: f(self.accent_hover),
            on_accent: f(self.on_accent),
            selected: f(self.selected),
            selected_hover: f(self.selected_hover),
            danger: f(self.danger),
            danger_border: f(self.danger_border),
            modified: f(self.modified),
            added: f(self.added),
            added_border: f(self.added_border),
            renamed: f(self.renamed),
            deleted: f(self.deleted),
            modified_chip: f(self.modified_chip),
            added_chip: f(self.added_chip),
            added_tint: f(self.added_tint),
            removed_tint: f(self.removed_tint),
            added_gutter: f(self.added_gutter),
            removed_gutter: f(self.removed_gutter),
        }
    }

    /// A Catppuccin accent in this theme's flavor: for the repo badges.
    pub fn named(&self, accent: Accent) -> Color {
        accent.resolve(self.light).color()
    }

    /// The accent `choice` stands for on this theme's background, as a hex code: what
    /// the theme menu's Custom field starts from.
    pub fn accent_hex(&self, choice: AccentChoice) -> Rgb {
        match choice {
            AccentChoice::Named(accent) => accent.resolve(self.light),
            AccentChoice::Custom(rgb) => rgb,
        }
    }
}

impl Default for Colors {
    fn default() -> Colors {
        Colors::new(&ThemeSettings::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOCHA_BASE: Rgb = Rgb(0x1e, 0x1e, 0x2e);
    const MOCHA_TEXT: Rgb = Rgb(0xcd, 0xd6, 0xf4);

    #[test]
    fn test_steps_rebuild_catppuccin_mocha() {
        // Catppuccin's own values are hand-tuned by a step here and there (surface1's
        // green is 0x47; the straight mix gives 0x46), so: within one per channel.
        let close = |amount: f32, expected: &str| {
            let got = MOCHA_BASE.mix(MOCHA_TEXT, amount);
            let want: Rgb = expected.parse().unwrap();
            let near = |a: u8, b: u8| a.abs_diff(b) <= 1;
            assert!(
                near(got.0, want.0) && near(got.1, want.1) && near(got.2, want.2),
                "{amount}: got {got}, Catppuccin has {want}"
            );
        };
        close(0.11, "#313244"); // surface0
        close(0.22, "#45475a"); // surface1
        close(0.33, "#585b70"); // surface2
        close(0.446, "#6c7086"); // overlay0
        close(0.554, "#7f849c"); // overlay1
        close(0.777, "#a6adc8"); // subtext0
        assert_eq!(MOCHA_BASE.mix(Rgb::BLACK, 0.2).to_string(), "#181825"); // mantle
        assert_eq!(MOCHA_BASE.mix(Rgb::BLACK, 0.42).to_string(), "#11111b"); // crust
    }

    #[test]
    fn test_light_background_uses_latte_accents() {
        let dark = Colors::default();
        assert!(!dark.light);
        assert_eq!(dark.added, Accent::Green.mocha().color());
        assert_eq!(dark.named(Accent::Teal), Accent::Teal.mocha().color());

        let latte = Colors::new(&ThemeSettings {
            background: Rgb(0xef, 0xf1, 0xf5),
            text: Rgb(0x4c, 0x4f, 0x69),
            ..ThemeSettings::default()
        });
        assert!(latte.light);
        assert_eq!(latte.added, Accent::Green.latte().color());
        assert_eq!(latte.accent, Accent::Mauve.latte().color());
        assert_eq!(latte.named(Accent::Teal), Accent::Teal.latte().color());
        // Light panels darken toward the text, not black.
        assert_eq!(
            latte.panel,
            Rgb(0xef, 0xf1, 0xf5)
                .mix(Rgb(0x4c, 0x4f, 0x69), 0.06)
                .color()
        );
    }

    #[test]
    fn test_custom_accent_is_kept_as_typed() {
        let orange = Rgb(0xff, 0x88, 0x00);
        let colors = Colors::new(&ThemeSettings {
            background: Rgb(0xef, 0xf1, 0xf5),
            accent: AccentChoice::Custom(orange),
            ..ThemeSettings::default()
        });
        assert_eq!(colors.accent, orange.color());
    }

    #[test]
    fn test_text_on_accent_contrasts() {
        // Mocha's pastel mauve carries dark text (the default theme's top bar color);
        // Latte's deep mauve carries white.
        let neutral = PRESETS[0].background;
        assert_eq!(
            Colors::default().on_accent,
            neutral.mix(Rgb::BLACK, 0.42).color()
        );
        let latte = Colors::new(&ThemeSettings {
            background: Rgb(0xef, 0xf1, 0xf5),
            ..ThemeSettings::default()
        });
        assert_eq!(latte.on_accent, Color::WHITE);
        // A light accent on a light theme: the dark text, not the light top bar.
        let yellow = Colors::new(&ThemeSettings {
            background: Rgb(0xef, 0xf1, 0xf5),
            text: Rgb(0x4c, 0x4f, 0x69),
            accent: AccentChoice::Custom(Rgb(0xff, 0xdd, 0x00)),
        });
        assert_eq!(yellow.on_accent, Rgb(0x4c, 0x4f, 0x69).color());
    }

    #[test]
    fn test_parse_hex() {
        assert_eq!("#1E1e2e".parse(), Ok(MOCHA_BASE));
        assert_eq!(" 1e1e2e ".parse(), Ok(MOCHA_BASE));
        assert!("#1e1e2".parse::<Rgb>().is_err());
        assert!("#1e1e2g".parse::<Rgb>().is_err());
        assert!("#1e1e2e0".parse::<Rgb>().is_err());
        assert!("".parse::<Rgb>().is_err());
        assert_eq!(MOCHA_BASE.to_string(), "#1e1e2e");
    }

    #[test]
    fn test_settings_round_trip() {
        let named = ThemeSettings::default();
        let text = toml::to_string(&named).unwrap();
        assert!(text.contains("accent = \"Mauve\""), "{text}");
        assert_eq!(toml::from_str::<ThemeSettings>(&text).unwrap(), named);

        let custom = ThemeSettings {
            accent: AccentChoice::Custom(Rgb(0xff, 0x88, 0x00)),
            ..ThemeSettings::default()
        };
        let text = toml::to_string(&custom).unwrap();
        assert!(text.contains("accent = \"#ff8800\""), "{text}");
        assert_eq!(toml::from_str::<ThemeSettings>(&text).unwrap(), custom);
    }

    #[test]
    fn test_settings_reject_bad_values() {
        assert!(toml::from_str::<ThemeSettings>("background = \"dark\"").is_err());
        assert!(toml::from_str::<ThemeSettings>("accent = \"Purple\"").is_err());
    }
}
