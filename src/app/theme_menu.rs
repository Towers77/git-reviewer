//! The top bar's theme menu: a preset (a background and its text color, picked
//! together), hex fields to fine-tune either, and the accent, one of Catppuccin's or a
//! hex code. Every other color is worked out from these three (see `theme::palette`).

use iced::widget::text::Wrapping;
use iced::widget::{TextInput, button, column, container, row, space, text, text_input};
use iced::{Alignment::Center, Color, Element, Fill};

use super::popup::popup;
use super::sidebar::{SWATCH_COLUMNS, swatch};
use super::{App, Message, OpenMenu};
use crate::theme::icons::{self, icon};
use crate::theme::palette::{Accent, AccentChoice, PRESETS, Preset, Rgb, ThemeSettings};
use crate::theme::{self, Colors, fonts, icon_sizes, layouts, text_sizes};

/// A change made in the theme menu.
#[derive(Debug, Clone)]
pub enum ThemeEdit {
    Preset(Preset),
    /// Something was typed in a hex field: applied once it's a color.
    Hex(HexField, String),
    Accent(Accent),
    /// Back to the default: Neutral with mauve.
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexField {
    Background,
    Text,
    /// The Custom accent.
    Accent,
}

/// What the hex fields show. Kept apart from the theme because what's being typed
/// isn't a color until it's finished, and a field must not jump back mid-typing.
#[derive(Debug, Clone, Default)]
pub struct ThemeDrafts {
    background: String,
    text: String,
    accent: String,
}

impl ThemeDrafts {
    /// The fields for `theme`: its colors as hex codes, the accent's as it's shown on
    /// this background (a named accent's Mocha or Latte value).
    fn of(theme: &ThemeSettings, colors: &Colors) -> ThemeDrafts {
        ThemeDrafts {
            background: theme.background.to_string(),
            text: theme.text.to_string(),
            accent: colors.accent_hex(theme.accent).to_string(),
        }
    }

    fn get(&self, field: HexField) -> &str {
        match field {
            HexField::Background => &self.background,
            HexField::Text => &self.text,
            HexField::Accent => &self.accent,
        }
    }

    fn set(&mut self, field: HexField, typed: String) {
        match field {
            HexField::Background => self.background = typed,
            HexField::Text => self.text = typed,
            HexField::Accent => self.accent = typed,
        }
    }
}

impl App {
    /// Opens `menu`, or closes it if it's the one open. The theme menu's fields start
    /// from the theme in use.
    pub(super) fn toggle_menu(&mut self, menu: OpenMenu) {
        if self.menu.as_ref() == Some(&menu) {
            self.menu = None;
            return;
        }
        if menu == OpenMenu::Theme {
            self.theme_drafts = ThemeDrafts::of(&self.state.theme, &self.colors);
        }
        self.menu = Some(menu);
    }

    pub(super) fn edit_theme(&mut self, edit: ThemeEdit) {
        let mut theme = self.state.theme;
        match edit {
            ThemeEdit::Preset(preset) => {
                theme.background = preset.background;
                theme.text = preset.text;
            }
            ThemeEdit::Accent(accent) => theme.accent = AccentChoice::Named(accent),
            ThemeEdit::Reset => theme = ThemeSettings::default(),
            ThemeEdit::Hex(field, typed) => {
                let color = typed.parse::<Rgb>().ok();
                self.theme_drafts.set(field, typed);
                let Some(color) = color else {
                    return;
                };
                match field {
                    HexField::Background => theme.background = color,
                    HexField::Text => theme.text = color,
                    HexField::Accent => theme.accent = AccentChoice::Custom(color),
                }
                self.set_theme(theme);
                // The typed fields stay as typed; a named accent's field follows the
                // background, which may have just switched it between Mocha and Latte,
                // unless something half-typed is in it.
                let accent_typed = self.theme_drafts.accent.parse::<Rgb>().is_err();
                if let AccentChoice::Named(_) = theme.accent
                    && !accent_typed
                {
                    self.theme_drafts.accent = self.colors.accent_hex(theme.accent).to_string();
                }
                return;
            }
        }
        self.set_theme(theme);
        self.theme_drafts = ThemeDrafts::of(&self.state.theme, &self.colors);
    }

    /// Switches to `theme`: works its colors out, saves it, and if it went between
    /// dark and light, colors the open diff's code again in the other Catppuccin theme
    /// (the old colors stay until the new ones are in).
    fn set_theme(&mut self, theme: ThemeSettings) {
        if theme == self.state.theme {
            return;
        }
        let was_light = self.colors.light;
        self.state.theme = theme;
        self.colors = Colors::new(&theme);
        self.iced_theme = theme::app_theme(&self.colors);
        if self.colors.light != was_light
            && let Some(Ok(diff)) = &mut self.diff
        {
            diff.highlight_again();
        }
        self.save_state("the theme");
    }

    /// The top bar's theme button, with its menu under it while open.
    pub(super) fn theme_button(&self) -> Element<'_, Message> {
        let c = &self.colors;
        let open = self.menu == Some(OpenMenu::Theme);
        // A whole-pixel inset keeps the icon's thin strokes sharp.
        let inset = (layouts::BUTTON_HEIGHT - icon_sizes::BUTTON) / 2.0;
        let button = button(icon(icons::PALETTE, icon_sizes::BUTTON, c.text))
            .on_press(Message::ThemeMenuToggled)
            .width(layouts::BUTTON_HEIGHT)
            .height(layouts::BUTTON_HEIGHT)
            .padding(inset)
            .style(theme::ghost_button(c, c.text, open));

        popup(
            button,
            open.then(|| self.theme_menu()),
            Message::MenuDismissed,
        )
    }

    fn theme_menu(&self) -> Element<'_, Message> {
        let c = &self.colors;
        let current = &self.state.theme;

        let header = row![
            text("Theme").font(fonts::UI_SEMIBOLD).width(Fill),
            button(text("Reset").size(text_sizes::SMALL))
                .on_press(Message::Theme(ThemeEdit::Reset))
                .padding([2.0, 6.0])
                .style(theme::ghost_button(c, c.text_muted, false)),
        ]
        .align_y(Center);

        let presets = row(PRESETS.iter().map(|preset| {
            let picked = current.background == preset.background && current.text == preset.text;
            button(
                text(preset.name)
                    .size(text_sizes::TAG)
                    .font(fonts::UI_SEMIBOLD)
                    .wrapping(Wrapping::None)
                    .width(Fill)
                    .height(Fill)
                    .center(),
            )
            .on_press(Message::Theme(ThemeEdit::Preset(*preset)))
            .width(Fill)
            .height(layouts::PRESET_HEIGHT)
            .padding([0.0, 2.0])
            .style(theme::preset_button(
                c,
                preset.background.color(),
                preset.text.color(),
                picked,
            ))
            .into()
        }))
        .spacing(layouts::GAP);

        let fields = row![
            self.hex_field(
                "Background",
                HexField::Background,
                current.background.color()
            ),
            self.hex_field("Text", HexField::Text, current.text.color()),
        ]
        .spacing(layouts::GAP);

        let accents = Accent::ALL.chunks(SWATCH_COLUMNS).map(|line| {
            row(line.iter().map(|accent| {
                let picked = current.accent == AccentChoice::Named(*accent);
                let pick = Message::Theme(ThemeEdit::Accent(*accent));
                swatch(c, c.named(*accent), picked, pick)
            }))
            .spacing(layouts::GAP)
            .into()
        });
        let custom = row![
            text("Custom")
                .size(text_sizes::TAG)
                .color(c.text_dim)
                .width(layouts::CUSTOM_LABEL_WIDTH),
            color_box(c, c.accent),
            self.hex_input(HexField::Accent)
                .width(layouts::CUSTOM_HEX_WIDTH),
        ]
        .spacing(6)
        .align_y(Center);

        container(
            column![
                header,
                column![theme::label(c, "PRESETS"), presets, fields].spacing(8),
                column![
                    theme::label(c, "ACCENT"),
                    column(accents).spacing(layouts::GAP),
                    custom
                ]
                .spacing(8),
            ]
            .spacing(16),
        )
        .width(layouts::THEME_MENU_WIDTH)
        .padding(12)
        .style(theme::menu(c))
        .into()
    }

    /// A labeled hex field with a square of its color beside it.
    fn hex_field<'a>(
        &'a self,
        label: &'a str,
        field: HexField,
        color: Color,
    ) -> Element<'a, Message> {
        let c = &self.colors;
        column![
            text(label).size(text_sizes::TAG).color(c.text_dim),
            row![color_box(c, color), self.hex_input(field).width(Fill)]
                .spacing(6)
                .align_y(Center),
        ]
        .spacing(4)
        .width(Fill)
        .into()
    }

    /// A hex field, outlined in red while what's typed isn't a color yet.
    fn hex_input(&self, field: HexField) -> TextInput<'_, Message> {
        let draft = self.theme_drafts.get(field);
        let valid = draft.parse::<Rgb>().is_ok();
        text_input("#rrggbb", draft)
            .on_input(move |typed| Message::Theme(ThemeEdit::Hex(field, typed)))
            .font(fonts::MONO)
            .size(text_sizes::LABEL)
            .padding([4.0, 8.0])
            .style(theme::hex_input(&self.colors, valid))
    }
}

/// A small square of `color`.
fn color_box<'a>(c: &Colors, color: Color) -> Element<'a, Message> {
    container(space())
        .width(layouts::COLOR_BOX)
        .height(layouts::COLOR_BOX)
        .style(theme::color_box(c, color))
        .into()
}
