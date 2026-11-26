use iced::{
    Border, Color, Shadow, Theme, Vector,
    theme::Palette,
    widget::{Row, button, container, rule, scrollable, text, text_editor},
};

pub mod palette;

/// The colors are worked out at runtime from the user's theme (see `palette`). `App`
/// owns them, and every style function here takes them: each copies the few colors it
/// needs into the closure it returns (`use<>`: the closure doesn't borrow them).
pub use palette::Colors;

// Everything is 2px above the mockup's sizes (asked for 2026-11-21: easier to read on
// a QHD monitor). The mockup's were 13 / 14 / 15 / 12 / 11 / 10.
pub mod text_sizes {
    pub const BODY: f32 = 15.0;
    pub const TITLE: f32 = 16.0;
    pub const BRANCH: f32 = 17.0;
    pub const SMALL: f32 = 14.0;
    pub const LABEL: f32 = 13.0;
    pub const TAG: f32 = 12.0;
    /// A repo badge's initials.
    pub const BADGE: f32 = 10.0;
}

/// UI icon sizes, also 2px above the mockup. Odd sizes put a 24-unit icon's centre lines
/// on pixel centres, which keeps thin strokes sharp.
pub mod icon_sizes {
    /// Top bar buttons and the primary button.
    pub const BUTTON: f32 = 16.0;
    /// Sidebar branch icon and the file list's +/−/discard buttons.
    pub const ROW: f32 = 15.0;
    /// The repo header's chevron.
    pub const CHEVRON: f32 = 14.0;
    /// The diff's loading spinner.
    pub const SPINNER: f32 = 24.0;
    /// The sidebar's ⋯ and unarchive buttons (in a `layouts::SIDEBAR_ICON_BUTTON`).
    pub const SIDEBAR_BUTTON: f32 = 14.0;
}

pub mod layouts {
    // Bars and panels
    pub const TOP_BAR_HEIGHT: f32 = 48.0;
    // Panel widths: draggable (see app/panels.rs), from these defaults down to these
    // minimums. 170 is VS Code's sidebar minimum. The changes panel's minimum is
    // measured from its footer's labels instead (`changes::min_width`).
    pub const SIDEBAR_DEFAULT_WIDTH: f32 = 260.0;
    pub const SIDEBAR_MIN_WIDTH: f32 = 170.0;
    pub const CHANGES_DEFAULT_WIDTH: f32 = 280.0;
    pub const DIFF_MIN_WIDTH: f32 = 300.0;
    /// The 1px line between panels.
    pub const DIVIDER_WIDTH: f32 = 1.0;
    /// The grab area centered on a divider: odd, so it overhangs the line evenly.
    pub const DIVIDER_HANDLE_WIDTH: f32 = 5.0;
    // Buttons
    pub const BUTTON_HEIGHT: f32 = 32.0;
    pub const PRIMARY_BUTTON_HEIGHT: f32 = 34.0;
    pub const ICON_BUTTON: f32 = 24.0;
    /// The sidebar's ⋯ and unarchive buttons: smaller, like VS Code's tree-row actions,
    /// so they fit the repo rows without making them taller.
    pub const SIDEBAR_ICON_BUTTON: f32 = 20.0;
    /// A repo's badge (its initials on its color): wider than tall, for two letters.
    pub const BADGE_WIDTH: f32 = 24.0;
    pub const BADGE_HEIGHT: f32 = 18.0;
    /// Count chips (the sidebar's change counts): a fixed height, text centered in it.
    pub const CHIP_HEIGHT: f32 = 18.0;
    /// The repo menu, and its color swatches (a ring around a dot when picked).
    pub const REPO_MENU_WIDTH: f32 = 232.0;
    pub const SWATCH: f32 = 26.0;
    pub const SWATCH_GAP: f32 = 4.0;
    /// The repo menu's Archive item.
    pub const MENU_ITEM_HEIGHT: f32 = 30.0;
    /// File rows and section headers in the changes list.
    pub const LIST_ROW_HEIGHT: f32 = 32.0;
    /// Catppuccin's file icons are drawn on a 16px grid; any other size blurs their
    /// 1px strokes, so they stay at 16 while the text around them grew.
    pub const FILE_ICON: f32 = 16.0;
    // Commit box: three lines of BODY text plus its padding.
    pub const COMMIT_BOX_HEIGHT: f32 = 80.0;
    // Diff
    pub const LINE_NUMBER_WIDTH: f32 = 48.0;
    pub const DIFF_LINE_HEIGHT: f32 = 22.0;
    pub const DIFF_SIGN_WIDTH: f32 = 16.0;
    /// Advance width of one Fira Code glyph at the diff's text size: 1200 of its 1950
    /// font units per em (≈0.615em, measured from the font file; not the 0.6 of most
    /// monospace fonts). Every glyph has it, so an ASCII line's width is `chars × this`
    /// (other lines are measured: see `app::diff::code_width`).
    pub const DIFF_CHAR_WIDTH: f32 = 1200.0 / 1950.0 * super::text_sizes::SMALL;
    pub const DIFF_HEADER_HEIGHT: f32 = 48.0;
    // Error toast: a long message (e.g. a hook's output) scrolls inside it past this height.
    pub const TOAST_WIDTH: f32 = 420.0;
    pub const TOAST_MAX_HEIGHT: f32 = 240.0;
    /// The uppercase labels' letter-spacing: the mockup's 0.06em, at their size.
    pub const LABEL_TRACKING: f32 = 0.06 * super::text_sizes::LABEL;
    // Padding
    pub const PANEL_PADDING_X: f32 = 16.0;
    pub const PANEL_PADDING_Y: f32 = 12.0;
    pub const ROW_PADDING_X: f32 = 8.0;
    // Gaps
    pub const GAP: f32 = 8.0;
    pub const SECTION_GAP: f32 = 10.0;
    pub const REPO_GAP: f32 = 14.0;
    /// The sidebar's Archived list scrolls past this height (about six rows), so an
    /// open list never takes over the repos above it.
    pub const ARCHIVED_LIST_MAX_HEIGHT: f32 = 180.0;
}

pub mod radii {
    pub const CONTROL: f32 = 6.0;
    pub const TAG: f32 = 4.0;
    pub const BADGE: f32 = 5.0;
    /// Half a chip's height: round ends.
    pub const CHIP: f32 = 9.0;
    pub const MENU: f32 = 8.0;
}

/// The same fonts VS Code uses: the system UI font for everything, and Fira Code (his
/// editor font) for code. The system fonts can't be shipped (their licenses forbid it)
/// and don't need to be: iced loads the installed system fonts at startup.
pub mod fonts {
    use iced::{Font, font::Weight};

    /// Segoe UI on Windows; on macOS San Francisco, which the font database lists as
    /// `.SF NS`; elsewhere the bundled Inter. Each has the 400 and 600 weights used here.
    #[cfg(target_os = "windows")]
    const UI_FAMILY: &str = "Segoe UI";
    #[cfg(target_os = "macos")]
    const UI_FAMILY: &str = ".SF NS";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    const UI_FAMILY: &str = "Inter";

    pub const UI: Font = Font::with_name(UI_FAMILY);
    pub const UI_SEMIBOLD: Font = Font {
        weight: Weight::Semibold,
        ..UI
    };
    /// Only for the small text in badges and chips, which reads thin at semibold.
    pub const UI_BOLD: Font = Font {
        weight: Weight::Bold,
        ..UI
    };
    /// Bundled (assets/fonts, OFL). Only for the diff, where columns must line up;
    /// everything else uses UI, so rows never mix two fonts' vertical metrics.
    pub const MONO: Font = Font::with_name("Fira Code");
}

pub mod icons {
    use iced::{
        Color,
        widget::{Svg, svg},
    };

    pub const DOWNLOAD: &[u8] = include_bytes!("../assets/icons/arrow-down-to-line.svg");
    pub const UPLOAD: &[u8] = include_bytes!("../assets/icons/arrow-up-from-line.svg");
    pub const CHECK: &[u8] = include_bytes!("../assets/icons/check.svg");
    pub const SYNC: &[u8] = include_bytes!("../assets/icons/sync.svg");
    pub const DISCARD: &[u8] = include_bytes!("../assets/icons/discard.svg");
    pub const CHEVRON_DOWN: &[u8] = include_bytes!("../assets/icons/chevron-down.svg");
    pub const CHEVRON_RIGHT: &[u8] = include_bytes!("../assets/icons/chevron-right.svg");
    pub const GIT_BRANCH: &[u8] = include_bytes!("../assets/icons/git-branch.svg");
    /// The branch icon with its curve cut: a worktree whose upstream is gone.
    pub const GIT_BRANCH_BROKEN: &[u8] = include_bytes!("../assets/icons/git-branch-broken.svg");
    /// A repo's main worktree, like VS Code's repository icon.
    pub const BOOK_CHECK: &[u8] = include_bytes!("../assets/icons/book-check.svg");
    pub const ELLIPSIS: &[u8] = include_bytes!("../assets/icons/ellipsis.svg");
    pub const MINUS: &[u8] = include_bytes!("../assets/icons/minus.svg");
    pub const PLUS: &[u8] = include_bytes!("../assets/icons/plus.svg");
    pub const ROTATE_CW: &[u8] = include_bytes!("../assets/icons/rotate-cw.svg");
    pub const CIRCLE_ALERT: &[u8] = include_bytes!("../assets/icons/circle-alert.svg");
    pub const X: &[u8] = include_bytes!("../assets/icons/x.svg");
    pub const SPINNER: &[u8] = include_bytes!("../assets/icons/spinner.svg");
    pub const ARCHIVE: &[u8] = include_bytes!("../assets/icons/archive.svg");
    pub const UNARCHIVE: &[u8] = include_bytes!("../assets/icons/archive-restore.svg");

    /// A foldable section's chevron: down while it's open, right while it's collapsed.
    pub fn chevron<'a>(c: &super::Colors, open: bool) -> Svg<'a> {
        let bytes = if open { CHEVRON_DOWN } else { CHEVRON_RIGHT };
        icon(bytes, super::icon_sizes::CHEVRON, c.text_muted)
    }

    pub fn icon<'a>(bytes: &'static [u8], size: f32, color: Color) -> Svg<'a> {
        svg(svg::Handle::from_memory(bytes))
            .width(size)
            .height(size)
            .style(move |_, _| svg::Style { color: Some(color) })
    }
}

/// An uppercase section label ("COMMIT MESSAGE", "CHANGES · 3"), letter-spaced like the
/// mockup. iced 0.14 can't space letters (its text engine can, but iced never passes
/// the setting on), so each character is its own text, in a row spaced `LABEL_TRACKING`
/// apart. That drops kerning, which spaced capitals don't need: measured, the
/// characters' widths add up exactly to the whole label's, spaces included.
pub fn label<'a, Message: 'a>(c: &Colors, content: &str) -> Row<'a, Message> {
    Row::with_children(content.chars().map(|character| {
        text(character.to_string())
            .size(text_sizes::LABEL)
            .font(fonts::UI_SEMIBOLD)
            .color(c.text_muted)
            .into()
    }))
    .spacing(layouts::LABEL_TRACKING)
}

pub fn divider(c: &Colors) -> impl Fn(&Theme) -> rule::Style + use<> {
    let color = c.border;
    move |theme| rule::Style {
        color,
        ..rule::default(theme)
    }
}

pub fn outline_button(c: &Colors) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let (text, border, hover) = (c.text, c.control_border, c.hover);
    move |_, status| {
        let base = button::Style {
            background: None,
            text_color: text,
            border: Border {
                color: border,
                width: 1.0,
                radius: radii::CONTROL.into(),
            },
            ..button::Style::default()
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered | button::Status::Pressed => base.with_background(hover),
            button::Status::Disabled => dimmed(base),
        }
    }
}

pub fn danger_button(c: &Colors) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let (danger, border, hover) = (c.danger, c.danger_border, c.hover);
    move |_, status| {
        let base = button::Style {
            background: None,
            text_color: danger,
            border: Border {
                color: border,
                width: 1.0,
                radius: radii::CONTROL.into(),
            },
            ..button::Style::default()
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered | button::Status::Pressed => base.with_background(hover),
            button::Status::Disabled => dimmed(base),
        }
    }
}

pub fn primary_button(c: &Colors) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let (accent, on_accent, pill, text_dim) = (c.accent, c.on_accent, c.pill, c.text_dim);
    move |_, status| {
        let base = button::Style {
            background: Some(accent.into()),
            text_color: on_accent,
            border: Border {
                color: accent,
                width: 0.0,
                radius: radii::CONTROL.into(),
            },
            ..button::Style::default()
        };

        match status {
            // Not `dimmed`: half-faded text on a half-faded accent is unreadable. A
            // neutral grey button reads as "off" and stays legible.
            button::Status::Disabled => button::Style {
                background: Some(pill.into()),
                text_color: text_dim,
                ..base
            },
            _ => base,
        }
    }
}

fn dimmed(style: button::Style) -> button::Style {
    button::Style {
        text_color: style.text_color.scale_alpha(0.5),
        border: Border {
            color: style.border.color.scale_alpha(0.5),
            ..style.border
        },
        background: style.background.map(|bg| bg.scale_alpha(0.5)),
        ..style
    }
}

pub fn list_row(
    c: &Colors,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let (selected_color, text, hover) = (c.selected, c.text, c.hover);
    move |_, status| {
        let base = button::Style {
            background: if selected {
                Some(selected_color.into())
            } else {
                None
            },
            text_color: text,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: radii::CONTROL.into(),
            },
            ..button::Style::default()
        };

        match status {
            _ if selected => base,
            button::Status::Active => base,
            button::Status::Hovered | button::Status::Pressed => base.with_background(hover),
            button::Status::Disabled => dimmed(base),
        }
    }
}

/// A row that looks hovered whether it is or not: a repo header while its menu is open
/// (the menu takes the mouse, so the row itself never sees the hover).
pub fn held_row(c: &Colors) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let (text, hover) = (c.text, c.hover);
    move |_, _| button::Style {
        background: Some(hover.into()),
        text_color: text,
        border: Border {
            radius: radii::CONTROL.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// A repo's badge: its initials in its color, on the same color faint.
pub fn badge(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        text_color: Some(color),
        background: Some(color.scale_alpha(palette::BADGE_ALPHA).into()),
        border: Border {
            radius: radii::BADGE.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// A round dot of `color`: a color swatch's fill.
pub fn dot(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(color.into()),
        border: Border {
            radius: (layouts::SWATCH / 2.0).into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// A count chip: `text` on `background`, with round ends.
pub fn chip(text: Color, background: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        text_color: Some(text),
        background: Some(background.into()),
        border: Border {
            radius: radii::CHIP.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// A dropdown menu's card: the panel color with a line around it and a shadow under it.
pub fn menu(c: &Colors) -> impl Fn(&Theme) -> container::Style + use<> {
    let (background, border) = (c.panel, c.control_border);
    move |_| container::Style {
        background: Some(background.into()),
        border: Border {
            color: border,
            width: 1.0,
            radius: radii::MENU.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        ..container::Style::default()
    }
}

/// A color swatch: a ring in the text color around it when it's the picked one, and a
/// fainter ring on hover. The swatch itself is the button's content.
pub fn swatch(
    c: &Colors,
    picked: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let (ring, hover_ring) = (c.text, c.text_dim);
    move |_, status| {
        let color = match status {
            _ if picked => ring,
            button::Status::Hovered | button::Status::Pressed => hover_ring,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            border: Border {
                color,
                width: 2.0,
                radius: (layouts::SWATCH / 2.0).into(),
            },
            ..button::Style::default()
        }
    }
}

/// The small square +/−/discard buttons in the file list. Their hover has to stand out
/// from the row under them, which is itself highlighted: one step stronger than `hover`
/// (`pill`) on a plain row, one step stronger than `selected` on the selected row.
pub fn icon_button(
    c: &Colors,
    on_selected_row: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let text = c.text_muted;
    let hover = if on_selected_row {
        c.selected_hover
    } else {
        c.pill
    };
    move |_, status| {
        let base = button::Style {
            background: None,
            text_color: text,
            border: Border {
                radius: radii::TAG.into(),
                ..Border::default()
            },
            ..button::Style::default()
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered | button::Status::Pressed => base.with_background(hover),
            button::Status::Disabled => dimmed(base),
        }
    }
}

/// An `icon_button` whose menu is open: it keeps its hover background meanwhile.
pub fn open_icon_button(c: &Colors) -> impl Fn(&Theme, button::Status) -> button::Style + use<> {
    let (text, background) = (c.text_muted, c.pill);
    move |_, _| button::Style {
        background: Some(background.into()),
        text_color: text,
        border: Border {
            radius: radii::TAG.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn commit_input(
    c: &Colors,
) -> impl Fn(&Theme, text_editor::Status) -> text_editor::Style + use<> {
    let c = *c;
    move |_, status| {
        let border_color = match status {
            text_editor::Status::Focused { .. } => c.accent,
            _ => c.control_border,
        };

        text_editor::Style {
            background: c.background.into(),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: radii::CONTROL.into(),
            },
            placeholder: c.text_dim,
            value: c.text,
            selection: c.accent.scale_alpha(0.4),
        }
    }
}

/// The mockup's 10px scrollbar with a 2px inset: a 6px thumb, 2px from the edge.
pub fn thin_scrollbar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new()
        .width(6)
        .scroller_width(6)
        .margin(2)
}

/// `thin_scrollbar` in iced's embedded mode: while the content overflows, the scrollable
/// pads its right side by the bar's full 10px and draws the bar there, so the bar never
/// covers the content. With no overflow nothing is reserved.
pub fn embedded_scrollbar() -> scrollable::Scrollbar {
    thin_scrollbar().spacing(0)
}

/// Every scrollbar in the app: no track, a `pill` thumb that lightens on hover/drag.
pub fn scrollbar(c: &Colors) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style + use<> {
    let (thumb, thumb_hover) = (c.pill, c.scrollbar_hover);
    move |theme, status| {
        let rail = |active: bool| scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: if active { thumb_hover } else { thumb }.into(),
                border: Border {
                    radius: 3.0.into(),
                    ..Border::default()
                },
            },
        };

        let (horizontal_active, vertical_active) = match status {
            scrollable::Status::Active { .. } => (false, false),
            scrollable::Status::Hovered {
                is_horizontal_scrollbar_hovered,
                is_vertical_scrollbar_hovered,
                ..
            } => (
                is_horizontal_scrollbar_hovered,
                is_vertical_scrollbar_hovered,
            ),
            scrollable::Status::Dragged {
                is_horizontal_scrollbar_dragged,
                is_vertical_scrollbar_dragged,
                ..
            } => (
                is_horizontal_scrollbar_dragged,
                is_vertical_scrollbar_dragged,
            ),
        };

        scrollable::Style {
            vertical_rail: rail(vertical_active),
            horizontal_rail: rail(horizontal_active),
            gap: None,
            ..scrollable::default(theme, status)
        }
    }
}

/// A filled, square-ish label like the diff header's "Unstaged".
pub fn filled_tag(c: &Colors) -> impl Fn(&Theme) -> container::Style + use<> {
    let (text, pill) = (c.text, c.pill);
    move |_| container::Style {
        text_color: Some(text),
        background: Some(pill.into()),
        border: Border {
            radius: radii::TAG.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The error toast's card: a step lighter than the panels under it, with a red border
/// and a shadow to lift it off them.
pub fn toast(c: &Colors) -> impl Fn(&Theme) -> container::Style + use<> {
    let (background, border) = (c.hover, c.danger_border);
    move |_| container::Style {
        background: Some(background.into()),
        border: Border {
            color: border,
            width: 1.0,
            radius: radii::CONTROL.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..container::Style::default()
    }
}

/// A plain background: the panels and the top bar.
pub fn background(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::background(color)
}

/// iced's own theme, for what the style functions above don't set (the default text
/// color, mainly). Built when the colors change, not per frame.
pub fn app_theme(c: &Colors) -> Theme {
    Theme::custom(
        "Git Reviewer",
        Palette {
            background: c.background,
            text: c.text,
            primary: c.accent,
            success: c.added,
            warning: c.modified,
            danger: c.danger,
        },
    )
}
