//! The middle panel: header, commit box, staged/unstaged file list and the removal footer.

use iced::keyboard::{Key, key::Named};
use iced::widget::scrollable::Direction;
use iced::widget::text::Wrapping;
use iced::widget::text_editor::{Binding, KeyPress};
use iced::widget::{
    Button, Row, button, column, container, hover, row, rule, scrollable, space, text, text_editor,
};
use iced::{Alignment::Center, Border, Color, Element, Fill, Padding, alignment::Horizontal};
use iced_palace::widget::ellipsized_text;

use std::f32::consts::TAU;
use std::sync::LazyLock;

use iced::advanced::text::{self as advanced_text, Paragraph as _, Text};
use iced::widget::text::{LineHeight, Shaping};
use iced::{Font, Radians, Rotation, Size, alignment};

use super::file_icon::file_icon;
use super::reveal::reveal;
use super::sidebar::{badge, chip};
use super::{App, DiscardScope, Message, RemoteOp, Removal, Section, empty_state};
use crate::git::{ChangeKind, DiffSource, FileEntry, Status};
use crate::theme::icons::{self, chevron, icon};
use crate::theme::{self, Colors, fonts, icon_sizes, layouts, text_sizes};

impl App {
    pub(super) fn changes_panel(&self) -> Element<'_, Message> {
        let c = &self.colors;
        let status = match &self.status {
            // The selected worktree's status is on its way (see `App::load_status`).
            None if self.selected_worktree_path.is_some() => return space().into(),
            None => {
                return empty_state("Select a worktree", c.text_muted)
                    .center(Fill)
                    .into();
            }
            Some(Err(err)) => {
                return empty_state(err.to_string(), c.danger).center(Fill).into();
            }
            Some(Ok(status)) => status,
        };

        column![
            self.changes_header(status),
            rule::horizontal(1).style(theme::divider(c)),
            self.commit_section(status),
            rule::horizontal(1).style(theme::divider(c)),
            scrollable(self.file_list(status))
                .direction(Direction::Vertical(theme::thin_scrollbar()))
                .style(theme::scrollbar(c))
                .height(Fill),
            rule::horizontal(1).style(theme::divider(c)),
            container(self.changes_footer(status))
                .padding([layouts::PANEL_PADDING_Y, layouts::PANEL_PADDING_X]),
        ]
        .into()
    }

    fn changes_header<'a>(&'a self, status: &'a Status) -> Element<'a, Message> {
        let c = &self.colors;
        let branch = status.branch.as_deref().unwrap_or("(detached)");
        let repo = self.selected_repo();

        // The branch, behind the repo's badge, then the repo under it. The upstream and
        // the "from base" counts aren't shown: Sync's ↑↓ already tells the upstream's,
        // and a broken base is told in the toast (see `apply_status`).
        let mut branch_line = row![].spacing(layouts::GAP).align_y(Center);
        if let Some(repo) = repo {
            branch_line = branch_line.push(badge(c, repo.name(), self.repo_color(repo.path())));
        }
        branch_line = branch_line.push(
            ellipsized_text(branch)
                .size(text_sizes::BRANCH)
                .font(fonts::UI_SEMIBOLD)
                .wrapping(Wrapping::None),
        );

        let mut header = column![branch_line].spacing(4);
        if let Some(repo) = repo {
            header = header.push(
                ellipsized_text(repo.name())
                    .size(text_sizes::SMALL)
                    .color(c.text_muted)
                    .wrapping(Wrapping::None),
            );
        }
        header
            .padding(Padding::from([layouts::PANEL_PADDING_Y, layouts::PANEL_PADDING_X]).top(14.0))
            .into()
    }

    fn commit_section<'a>(&'a self, status: &'a Status) -> Element<'a, Message> {
        let c = &self.colors;
        let staged_count = status.files.iter().filter(|f| f.staged.is_some()).count();
        let unstaged_count = status.files.iter().filter(|f| f.unstaged.is_some()).count();
        let counts = status.upstream.as_ref().and_then(|u| u.counts);
        let can_commit = staged_count > 0 && !self.commit_message.is_empty();

        // What the primary button shows and does, VS Code-style: Commit while there are
        // changes, then Sync while ahead/behind the upstream, Publish if there's none.
        // While a Sync/Publish runs it stays on that action, disabled, icon spinning.
        let (icon_bytes, title, counts_text, on_press) = match self.remote_op {
            Some((RemoteOp::Sync, _)) => (Some(icons::SYNC), "Sync changes", None, None),
            Some((RemoteOp::Publish, _)) => (Some(icons::SYNC), "Publish", None, None),
            None => match (staged_count, unstaged_count, counts) {
                (1.., _, _) => (
                    Some(icons::CHECK),
                    "Commit",
                    None,
                    can_commit.then_some(Message::Commit),
                ),
                (0, 0, Some((ahead, behind))) if ahead + behind > 0 => (
                    Some(icons::SYNC),
                    "Sync changes",
                    Some(format!("↑{ahead} ↓{behind}")),
                    Some(Message::Sync),
                ),
                (0, 0, None) if status.branch.is_some() => {
                    (Some(icons::UPLOAD), "Publish", None, Some(Message::Publish))
                }
                (0, 1.., _) => (Some(icons::CHECK), "Commit", None, None),
                _ => (None, "No changes", None, None),
            },
        };

        // The icon isn't text, so the button style's text color doesn't reach it; it gets
        // the same color as the label by hand (see `theme::primary_button`).
        let icon_color = if on_press.is_some() {
            c.on_accent
        } else {
            c.text_dim
        };
        // One turn per second since the operation started; 0 when idle.
        let angle = self
            .remote_op
            .map(|(_, started)| started.elapsed().as_secs_f32() * TAU)
            .unwrap_or(0.0);

        // Pushed piece by piece: a `None` child in `row!` would still take a gap.
        let mut content = row![].spacing(layouts::GAP).align_y(Center);
        if let Some(bytes) = icon_bytes {
            content = content.push(
                icon(bytes, icon_sizes::BUTTON, icon_color)
                    .rotation(Rotation::Floating(Radians(angle))),
            );
        }
        content = content.push(text(title).font(fonts::UI_SEMIBOLD));
        if let Some(counts_text) = counts_text {
            content = content.push(
                text(counts_text)
                    .font(fonts::UI_SEMIBOLD)
                    .size(text_sizes::SMALL),
            );
        }

        container(
            column![
                theme::label(c, "COMMIT MESSAGE"),
                text_editor(&self.commit_message)
                    .placeholder(if cfg!(target_os = "macos") {
                        "Message (⌘+Enter to commit)"
                    } else {
                        "Message (Ctrl+Enter to commit)"
                    })
                    .on_action(Message::CommitMessageEdited)
                    .key_binding(move |key_press| commit_shortcut(key_press, can_commit))
                    .padding([8.0, 10.0])
                    .height(layouts::COMMIT_BOX_HEIGHT)
                    .style(theme::commit_input(c)),
                button(container(content).center(Fill))
                    .on_press_maybe(on_press)
                    .height(layouts::PRIMARY_BUTTON_HEIGHT)
                    .width(Fill)
                    .style(theme::primary_button(c)),
            ]
            .spacing(layouts::GAP),
        )
        .padding([layouts::PANEL_PADDING_Y, layouts::PANEL_PADDING_X])
        .into()
    }

    fn file_list<'a>(&'a self, status: &'a Status) -> Element<'a, Message> {
        let c = &self.colors;
        if status.files.is_empty() {
            return empty_state("No changes in this worktree", c.text_muted)
                .center_x(Fill)
                .into();
        }

        // One group per side of the index; like VS Code, an empty group isn't shown,
        // and a collapsed one shows just its header.
        let group = |title: &str, section: Section| -> Option<Element<'a, Message>> {
            let staged = section == Section::Staged;
            let files: Vec<(&FileEntry, ChangeKind)> = status
                .files
                .iter()
                .filter_map(|file| {
                    let kind = if staged { file.staged } else { file.unstaged }?;
                    Some((file, kind))
                })
                .collect();
            if files.is_empty() {
                return None;
            }

            let shown = self.fold(&section);
            let mut group = column![section_header(c, title, files.len(), section, shown)];
            // Built while any of it shows, folding included.
            if shown > 0.0 {
                let rows = files
                    .into_iter()
                    .map(|(file, kind)| self.file_row(file, kind, staged));
                group = group.push(reveal(column(rows).spacing(1), shown, c.panel));
            }
            Some(group.into())
        };

        // `flatten` drops hidden groups, so a lone group gets no stray SECTION_GAP.
        let groups = [
            group("STAGED CHANGES", Section::Staged),
            group("CHANGES", Section::Changes),
        ]
        .into_iter()
        .flatten();

        container(column(groups).spacing(layouts::SECTION_GAP))
            .padding(layouts::GAP)
            .into()
    }

    fn file_row<'a>(
        &'a self,
        file: &'a FileEntry,
        kind: ChangeKind,
        staged: bool,
    ) -> Element<'a, Message> {
        let c = &self.colors;
        let source = if staged {
            DiffSource::Staged
        } else if kind == ChangeKind::Untracked {
            DiffSource::Untracked
        } else {
            DiffSource::Unstaged
        };
        let is_selected = self
            .selected_file
            .as_ref()
            .is_some_and(|(path, selected_source)| {
                *path == file.path && *selected_source == source
            });
        // VS Code's order: discard first, then stage; a staged file can only be unstaged.
        let actions = if staged {
            row![icon_button(
                c,
                icons::MINUS,
                Message::Unstage(file.path.clone()),
                is_selected,
            )]
        } else {
            let discard = DiscardScope::File {
                path: file.path.clone(),
                untracked: kind == ChangeKind::Untracked,
            };
            row![
                icon_button(
                    c,
                    icons::DISCARD,
                    Message::DiscardRequested(discard),
                    is_selected
                ),
                icon_button(
                    c,
                    icons::PLUS,
                    Message::Stage(file.path.clone()),
                    is_selected
                ),
            ]
        };
        let (dir, name) = file.path.rsplit_once('/').unwrap_or(("", &file.path));

        // The name gets the space first and only it is cut ("…") if it doesn't fit; the
        // folder gets what's left, cut too, and disappears when nothing is left (the
        // clip hides the lone "..." ellipsized text would draw at zero width).
        let name_and_dir = row![
            ellipsized_text(name).wrapping(Wrapping::None),
            container(
                ellipsized_text(dir)
                    .size(text_sizes::SMALL)
                    .color(c.text_dim)
                    .wrapping(Wrapping::None),
            )
            .width(Fill)
            .clip(true),
        ]
        .spacing(layouts::GAP)
        .align_y(Center);

        // Icon and status letter have fixed widths, so iced sizes them first and they
        // always keep their place; the name and folder share what's left in between.
        let row_button = button(
            row![
                file_icon(&file.path),
                container(name_and_dir).width(Fill),
                text(kind.to_code().to_string())
                    .font(fonts::UI_SEMIBOLD)
                    .color(kind_color(c, kind))
                    .width(STATUS_LETTER_WIDTH)
                    .align_x(Horizontal::Center),
            ]
            .spacing(layouts::GAP)
            .height(Fill)
            .align_y(Center),
        )
        .on_press(Message::FileSelected {
            path: file.path.clone(),
            source,
        })
        .width(Fill)
        .height(layouts::LIST_ROW_HEIGHT)
        .padding([0.0, 8.0])
        .style(theme::list_row(c, is_selected));

        // While hovered the row is `hover` (or `selected`, which wins); the actions sit on
        // a patch of that same color so the name slides under them, like VS Code.
        let row_color = if is_selected { c.selected } else { c.hover };

        // The buttons stop just short of the status letter, which stays visible, like
        // VS Code: row padding + letter + a small gap from the right edge.
        let actions_inset = 8.0 + STATUS_LETTER_WIDTH + 4.0;
        hover(row_button, hover_actions(actions, row_color, actions_inset))
    }

    fn changes_footer<'a>(&'a self, status: &'a Status) -> Element<'a, Message> {
        let c = &self.colors;
        let worktree_removable = !self.selected_worktree().is_some_and(|w| w.is_main());
        // Deliberate deviation from VS Code: only offer to delete branches with nothing
        // unmerged into the base (no base known → let `git branch -d` decide).
        // A configured base that doesn't resolve can't vouch for anything: fail closed.
        let branch_deletable = worktree_removable
            && status.branch.is_some()
            && self.base_error.is_none()
            && self
                .base_counts
                .as_ref()
                .is_none_or(|(_, (ahead, _))| *ahead == 0);
        // With uncommitted changes, `update` asks in a dialog before removing anything.
        let footer_button = |label: &'static str, removal: Removal, enabled: bool| {
            button(text(label).width(Fill).height(Fill).center())
                .on_press_maybe(enabled.then_some(Message::RemovalRequested(removal)))
                .height(layouts::BUTTON_HEIGHT)
                .width(Fill)
                .style(theme::danger_button(c))
        };

        row![
            footer_button(RM_WORKTREE, Removal::Worktree, worktree_removable),
            footer_button(DEL_BRANCH, Removal::Branch, branch_deletable),
        ]
        .spacing(layouts::GAP)
        .into()
    }
}

const RM_WORKTREE: &str = "Rm. worktree";
const DEL_BRANCH: &str = "Del. branch";

/// The narrowest the panel can be with the footer buttons' labels still on one line:
/// its padding, the gap, and two buttons as wide as the widest label plus the buttons'
/// default padding. Measured once, because the labels' width depends on the system
/// font (Segoe UI, San Francisco).
pub(super) fn min_width() -> f32 {
    static MIN_WIDTH: LazyLock<f32> = LazyLock::new(|| {
        let widest = [RM_WORKTREE, DEL_BRANCH]
            .map(label_width)
            .into_iter()
            .fold(0.0, f32::max);
        let padding = button::DEFAULT_PADDING;
        let button = widest + padding.left + padding.right;
        // Rounded up: a label wraps when it's even a fraction wider than its room.
        (2.0 * layouts::PANEL_PADDING_X + layouts::GAP + 2.0 * button).ceil()
    });
    *MIN_WIDTH
}

/// How wide `label` is on one line, as a plain `text` draws it: the default font and
/// size (main.rs sets no default font, so it's `Font::DEFAULT`). iced measures with the
/// same font system it renders with, so no window is needed.
fn label_width(label: &str) -> f32 {
    type Paragraph = <iced::Renderer as advanced_text::Renderer>::Paragraph;

    Paragraph::with_text(Text {
        content: label,
        bounds: Size::INFINITE,
        size: text_sizes::BODY.into(),
        line_height: LineHeight::default(),
        font: Font::DEFAULT,
        align_x: advanced_text::Alignment::Default,
        align_y: alignment::Vertical::Top,
        shaping: Shaping::default(),
        wrapping: Wrapping::None,
    })
    .min_width()
}

/// The commit box's key handling: Ctrl+Enter (Cmd+Enter on macOS, which is what
/// `modifiers.command()` means) commits; every other key behaves as usual.
/// iced calls this for every key press, focused or not, hence the focus check.
fn commit_shortcut(key_press: KeyPress, can_commit: bool) -> Option<Binding<Message>> {
    let is_shortcut = matches!(key_press.status, text_editor::Status::Focused { .. })
        && key_press.modifiers.command()
        && key_press.key == Key::Named(Named::Enter);

    if is_shortcut {
        // When committing isn't possible yet, swallow the shortcut rather than let it
        // insert a newline, like VS Code.
        return can_commit.then_some(Binding::Custom(Message::Commit));
    }

    Binding::from_key_press(key_press)
}

/// "STAGED CHANGES" and its file count in a chip (green for staged, yellow for changes)
/// behind a chevron, with its stage-all / unstage-all buttons on the right. Clicking
/// anywhere else on it folds the group away or opens it again.
fn section_header<'a>(
    c: &Colors,
    title: &str,
    count: usize,
    section: Section,
    shown: f32,
) -> Element<'a, Message> {
    let (actions, chip_color, chip_background) = if section == Section::Staged {
        (
            row![icon_button(c, icons::MINUS, Message::UnstageAll, false)],
            c.added,
            c.added_chip,
        )
    } else {
        (
            row![
                icon_button(
                    c,
                    icons::DISCARD,
                    Message::DiscardRequested(DiscardScope::All),
                    false
                ),
                icon_button(c, icons::PLUS, Message::StageAll, false),
            ],
            c.modified,
            c.modified_chip,
        )
    };

    let title = button(
        row![
            chevron(c, shown),
            theme::label(c, title),
            chip(count, chip_color, chip_background),
        ]
        .spacing(6)
        .height(Fill)
        .align_y(Center),
    )
    .on_press(Message::ToggleSection(section))
    .width(Fill)
    .height(layouts::LIST_ROW_HEIGHT)
    .padding([0.0, 8.0])
    .style(theme::list_row(c, false));

    // The header is hovered whenever its actions show, so they sit on `hover` too.
    hover(title, hover_actions(actions, c.hover, 4.0))
}

/// The status letter column at the right end of a file row (wide enough for a
/// semibold "M" at the body size). Letters are centered in it: they differ in width
/// (M is wider than D or A), so right-aligned they started at different places and the
/// column looked ragged; centered, they share one center line.
const STATUS_LETTER_WIDTH: f32 = 14.0;

/// The buttons a row shows only while hovered, right-aligned over the row, `inset`
/// from its right edge. They sit on a patch of `background` (the hovered row's color)
/// so text underneath doesn't show through; clicks anywhere else fall through to the
/// row itself.
fn hover_actions<'a>(
    buttons: Row<'a, Message>,
    background: Color,
    inset: f32,
) -> Element<'a, Message> {
    row![
        space().width(Fill),
        container(buttons.spacing(4))
            .padding(Padding::ZERO.left(4.0))
            .style(move |_| container::Style {
                background: Some(background.into()),
                border: Border {
                    radius: theme::radii::CONTROL.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    ]
    .height(Fill)
    .align_y(Center)
    .padding(Padding::ZERO.right(inset))
    .into()
}

/// A small square button holding a row-size icon (24px around 15px). `on_selected_row`
/// picks a hover color that still shows on the selected row's accent.
pub(super) fn icon_button<'a>(
    c: &Colors,
    bytes: &'static [u8],
    message: Message,
    on_selected_row: bool,
) -> Button<'a, Message> {
    sized_icon_button(
        c,
        bytes,
        Some(message),
        on_selected_row,
        layouts::ICON_BUTTON,
        icon_sizes::ROW,
    )
}

/// `icon_button` at another size: a `size`-square button holding an `icon_size` icon.
/// With no `on_press` it's disabled, and its icon is faded.
pub(super) fn sized_icon_button<'a>(
    c: &Colors,
    bytes: &'static [u8],
    on_press: Option<Message>,
    on_selected_row: bool,
    size: f32,
    icon_size: f32,
) -> Button<'a, Message> {
    // Pinned at a whole-pixel offset rather than centered: centering 15px in 24px puts
    // the icon at 4.5px, a half pixel, which smears its thin strokes across two pixel
    // columns (the + looked dim). Half a pixel off-center isn't visible; blur is.
    let before = ((size - icon_size) / 2.0).floor();
    let after = size - icon_size - before;

    // Faded here: the disabled style (`theme::icon_button`) fades the button's text
    // color, but an icon has its own color, which that doesn't reach. By half, as there.
    // With `opacity`, not the color's alpha: iced draws an SVG in its color's red, green
    // and blue only (checked in iced_wgpu's and iced_tiny_skia's SVG rasterizing).
    let opacity: f32 = if on_press.is_some() { 1.0 } else { 0.5 };

    button(icon(bytes, icon_size, c.text_muted).opacity(opacity))
        .on_press_maybe(on_press)
        .width(size)
        .height(size)
        .padding(Padding {
            top: before,
            left: before,
            right: after,
            bottom: after,
        })
        .style(theme::icon_button(c, on_selected_row))
}

fn kind_color(c: &Colors, kind: ChangeKind) -> Color {
    match kind {
        ChangeKind::Added => c.added,
        ChangeKind::Renamed => c.renamed,
        ChangeKind::Copied => c.added,
        ChangeKind::Modified => c.modified,
        ChangeKind::Deleted => c.deleted,
        ChangeKind::TypeChanged => c.modified,
        ChangeKind::Untracked => c.added,
    }
}
