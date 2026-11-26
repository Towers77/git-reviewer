use iced::{
    Border, Color, Shadow, Theme, Vector,
    theme::Palette,
    widget::{Row, button, container, rule, scrollable, text, text_editor},
};

pub mod palette;

pub mod colors {
    use iced::{Color, color};

    pub const BACKGROUND: Color = color!(0x18191c);
    pub const SURFACE: Color = color!(0x1f2023);
    pub const SIDEBAR: Color = color!(0x1c1d20);
    pub const BORDER: Color = color!(0x2e2f34);
    pub const CONTROL_BORDER: Color = color!(0x36373d);
    pub const TAG_BORDER: Color = color!(0x44464d);
    pub const HOVER: Color = color!(0x26272b);
    pub const SELECTED: Color = color!(0x2a3550);
    /// A button hovered on top of a SELECTED row: the same blue, a step lighter.
    pub const SELECTED_HOVER: Color = color!(0x3a4a70);
    pub const FOCUS: Color = color!(0x5b9bff);
    pub const TEXT: Color = color!(0xe6e6e9);
    pub const TEXT_MUTED: Color = color!(0xa1a3ab);
    pub const TEXT_DIM: Color = color!(0x8b8e97);
    pub const LINE_NUMBER: Color = color!(0x6f727b);
    pub const PILL: Color = color!(0x3a3b41);
    pub const SCROLLBAR_HOVER: Color = color!(0x4a4c53);
    pub const PRIMARY: Color = color!(0x3d7be0);
    /// Text and icons on PRIMARY buttons: the app's dark background, not white.
    pub const ON_PRIMARY: Color = BACKGROUND;
    pub const DANGER: Color = color!(0xf0857a);
    pub const DANGER_BORDER: Color = color!(0x5a3a37);
    pub const MODIFIED: Color = color!(0xe5b567);
    pub const ADDED: Color = color!(0x7cc48a);
    pub const RENAMED: Color = color!(0x6fb3e8);
    pub const DELETED: Color = DANGER;
    pub const ADDED_TINT: Color = color!(0x7cc48a, 0.13);
    pub const REMOVED_TINT: Color = color!(0xf0857a, 0.13);
    pub const ADDED_GUTTER: Color = color!(0x252f2a);
    pub const REMOVED_GUTTER: Color = color!(0x342728);
}

// Everything is 2px above the mockup's sizes (asked for 2026-11-21: easier to read on
// a QHD monitor). The mockup's were 13 / 14 / 15 / 12 / 11 / 10.
pub mod text_sizes {
    pub const BODY: f32 = 15.0;
    pub const TITLE: f32 = 16.0;
    pub const BRANCH: f32 = 17.0;
    pub const SMALL: f32 = 14.0;
    pub const LABEL: f32 = 13.0;
    pub const TAG: f32 = 12.0;
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
    /// The sidebar's archive/unarchive buttons (in a `layouts::SIDEBAR_ICON_BUTTON`).
    pub const SIDEBAR_BUTTON: f32 = 14.0;
}

pub mod layouts {
    // `Pixels` only appears in the Windows-tuned tag/pill line heights below.
    #[cfg(target_os = "windows")]
    use iced::Pixels;
    use iced::{Padding, widget::text::LineHeight};

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
    /// The sidebar's archive/unarchive buttons: smaller, like VS Code's tree-row actions,
    /// so they fit the repo rows without making them taller.
    pub const SIDEBAR_ICON_BUTTON: f32 = 20.0;
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

    // Sidebar tags ("main", "gone"): the label's line box and the padding around it,
    // chosen so the lowercase letters sit centered between the borders. Segoe UI's line
    // box keeps tall room above the letters (for accents), which put them 2px low, with
    // the "g" touching the bottom border; so on Windows the box is shortened and the
    // room moved below it. Same outer height either way. Measured by rendering the tags
    // and counting pixels: 5px above and 5px below the letters' body on Windows. Inter
    // sits centered with the defaults; San Francisco's metrics are close to Inter's.
    #[cfg(target_os = "windows")]
    pub const TAG_LINE_HEIGHT: LineHeight = LineHeight::Absolute(Pixels(13.0));
    #[cfg(target_os = "windows")]
    pub const TAG_PADDING: Padding = Padding {
        top: 1.0,
        right: 5.0,
        bottom: 4.0,
        left: 5.0,
    };
    #[cfg(not(target_os = "windows"))]
    pub const TAG_LINE_HEIGHT: LineHeight = LineHeight::Relative(1.3);
    #[cfg(not(target_os = "windows"))]
    pub const TAG_PADDING: Padding = Padding {
        top: 1.0,
        right: 5.0,
        bottom: 1.0,
        left: 5.0,
    };

    // The sidebar's change-count pill: the same Segoe UI effect put its digits 1px low
    // (6px above, 4px below); this gives 5 and 5 at the same height, measured the same
    // way. Not measured on macOS yet: Inter showed the opposite (1px high).
    #[cfg(target_os = "windows")]
    pub const PILL_LINE_HEIGHT: LineHeight = LineHeight::Absolute(Pixels(14.0));
    #[cfg(target_os = "windows")]
    pub const PILL_PADDING: Padding = Padding {
        top: 2.0,
        right: 6.0,
        bottom: 3.0,
        left: 6.0,
    };
    #[cfg(not(target_os = "windows"))]
    pub const PILL_LINE_HEIGHT: LineHeight = LineHeight::Relative(1.3);
    #[cfg(not(target_os = "windows"))]
    pub const PILL_PADDING: Padding = Padding {
        top: 1.0,
        right: 6.0,
        bottom: 1.0,
        left: 6.0,
    };
}

pub mod radii {
    pub const CONTROL: f32 = 6.0;
    pub const TAG: f32 = 4.0;
    pub const PILL: f32 = 9.0;
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
    pub const MINUS: &[u8] = include_bytes!("../assets/icons/minus.svg");
    pub const PLUS: &[u8] = include_bytes!("../assets/icons/plus.svg");
    pub const ROTATE_CW: &[u8] = include_bytes!("../assets/icons/rotate-cw.svg");
    pub const CIRCLE_ALERT: &[u8] = include_bytes!("../assets/icons/circle-alert.svg");
    pub const X: &[u8] = include_bytes!("../assets/icons/x.svg");
    pub const SPINNER: &[u8] = include_bytes!("../assets/icons/spinner.svg");
    pub const ARCHIVE: &[u8] = include_bytes!("../assets/icons/archive.svg");
    pub const UNARCHIVE: &[u8] = include_bytes!("../assets/icons/archive-restore.svg");

    /// A foldable section's chevron: down while it's open, right while it's collapsed.
    pub fn chevron<'a>(open: bool) -> Svg<'a> {
        let bytes = if open { CHEVRON_DOWN } else { CHEVRON_RIGHT };
        icon(bytes, super::icon_sizes::CHEVRON, super::colors::TEXT_MUTED)
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
pub fn label<'a, Message: 'a>(content: &str) -> Row<'a, Message> {
    Row::with_children(content.chars().map(|character| {
        text(character.to_string())
            .size(text_sizes::LABEL)
            .font(fonts::UI_SEMIBOLD)
            .color(colors::TEXT_MUTED)
            .into()
    }))
    .spacing(layouts::LABEL_TRACKING)
}

pub fn divider(theme: &Theme) -> rule::Style {
    rule::Style {
        color: colors::BORDER,
        ..rule::default(theme)
    }
}

pub fn outline_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: None,
        text_color: colors::TEXT,
        border: Border {
            color: colors::CONTROL_BORDER,
            width: 1.0,
            radius: radii::CONTROL.into(),
        },
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered | button::Status::Pressed => base.with_background(colors::HOVER),
        button::Status::Disabled => dimmed(base),
    }
}

pub fn danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: None,
        text_color: colors::DANGER,
        border: Border {
            color: colors::DANGER_BORDER,
            width: 1.0,
            radius: radii::CONTROL.into(),
        },
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered | button::Status::Pressed => base.with_background(colors::HOVER),
        button::Status::Disabled => dimmed(base),
    }
}

pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(colors::PRIMARY.into()),
        text_color: colors::ON_PRIMARY,
        border: Border {
            color: colors::PRIMARY,
            width: 0.0,
            radius: radii::CONTROL.into(),
        },
        ..button::Style::default()
    };

    match status {
        // Not `dimmed`: half-faded dark text on a half-faded blue is unreadable. A
        // neutral grey button reads as "off" and stays legible.
        button::Status::Disabled => button::Style {
            background: Some(colors::PILL.into()),
            text_color: colors::TEXT_DIM,
            ..base
        },
        _ => base,
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

pub fn list_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let base = button::Style {
            background: if selected {
                Some(colors::SELECTED.into())
            } else {
                None
            },
            text_color: colors::TEXT,
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
            button::Status::Hovered | button::Status::Pressed => {
                base.with_background(colors::HOVER)
            }
            button::Status::Disabled => dimmed(base),
        }
    }
}

pub fn tag(border: Color, text: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        text_color: Some(text),
        border: Border {
            color: border,
            width: 1.0,
            radius: radii::TAG.into(),
        },
        ..container::Style::default()
    }
}

/// The small square +/−/discard buttons in the file list. Their hover has to stand out
/// from the row under them, which is itself highlighted: one step lighter than HOVER
/// (PILL) on a plain row, one step lighter than SELECTED on the selected row.
pub fn icon_button(on_selected_row: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let base = button::Style {
            background: None,
            text_color: colors::TEXT_MUTED,
            border: Border {
                radius: radii::TAG.into(),
                ..Border::default()
            },
            ..button::Style::default()
        };
        let hover = if on_selected_row {
            colors::SELECTED_HOVER
        } else {
            colors::PILL
        };

        match status {
            button::Status::Active => base,
            button::Status::Hovered | button::Status::Pressed => base.with_background(hover),
            button::Status::Disabled => dimmed(base),
        }
    }
}

pub fn commit_input(_theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let border_color = match status {
        text_editor::Status::Focused { .. } => colors::FOCUS,
        _ => colors::CONTROL_BORDER,
    };

    text_editor::Style {
        background: colors::BACKGROUND.into(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: radii::CONTROL.into(),
        },
        placeholder: colors::TEXT_DIM,
        value: colors::TEXT,
        selection: colors::PRIMARY.scale_alpha(0.4),
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

/// Every scrollbar in the app: no track, a PILL-grey thumb that lightens on hover/drag.
pub fn scrollbar(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let rail = |active: bool| scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: if active {
                colors::SCROLLBAR_HOVER.into()
            } else {
                colors::PILL.into()
            },
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

/// A filled, square-ish label like the diff header's "Unstaged".
pub fn filled_tag(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(colors::TEXT),
        background: Some(colors::PILL.into()),
        border: Border {
            radius: radii::TAG.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The error toast's card: a step lighter than the panels under it, with a red border
/// and a shadow to lift it off them.
pub fn toast(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(colors::HOVER.into()),
        border: Border {
            color: colors::DANGER_BORDER,
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

pub fn pill(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(colors::TEXT),
        background: Some(colors::PILL.into()),
        border: Border {
            radius: radii::PILL.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

pub fn app_theme() -> Theme {
    Theme::custom(
        "Git Reviewer",
        Palette {
            background: colors::BACKGROUND,
            text: colors::TEXT,
            primary: colors::PRIMARY,
            success: colors::ADDED,
            warning: colors::MODIFIED,
            danger: colors::DANGER,
        },
    )
}
