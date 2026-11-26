//! The left panel: every repo under the root and its worktrees.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use iced::widget::scrollable::Direction;
use iced::widget::text::Wrapping;
use iced::widget::{
    Container, button, column, container, keyed_column, mouse_area, row, rule, scrollable, text,
};
use iced::{Alignment::Center, Color, Element, Fill, Padding};
use iced_palace::widget::ellipsized_text;

use super::changes::sized_icon_button;
use super::{App, Message, Section};
use crate::git::{Repo, Workspace, Worktree};
use crate::theme::icons::{self, chevron, icon};
use crate::theme::{self, Colors, fonts, icon_sizes, layouts, text_sizes};

impl App {
    pub(super) fn sidebar<'a>(&'a self, workspace: &'a Workspace) -> Element<'a, Message> {
        let c = &self.colors;
        let selected = self.selected_worktree_path.as_deref();

        // The panel's padding goes inside the scrollable, so the bar runs the panel's full
        // height against the divider, like VS Code's, and its 10px come out of the rows'
        // width instead of being drawn over their right end (see `embedded_scrollbar`).
        let repos = scrollable(
            container(
                column![
                    container(theme::label(
                        c,
                        &format!("REPOSITORIES · {}", workspace.repos().len())
                    ))
                    .padding([0.0, 8.0]),
                    // Keyed by repo, so each row keeps its own hover state: in a plain
                    // column, archiving a repo slides the next one into its widget state,
                    // already "hovered", and its `on_enter` would never fire.
                    keyed_column(workspace.repos().iter().map(|repo| {
                        let open = self.is_open(&Section::Repo(repo.name().to_string()));
                        let hovered = self.hovered_repo.as_deref() == Some(repo.path());
                        (repo_key(repo), repo_view(c, repo, selected, open, hovered))
                    }))
                    .spacing(layouts::REPO_GAP),
                ]
                .spacing(layouts::REPO_GAP),
            )
            .padding([layouts::PANEL_PADDING_Y, layouts::ROW_PADDING_X]),
        )
        .direction(Direction::Vertical(theme::embedded_scrollbar()))
        .style(theme::scrollbar(c))
        .width(Fill)
        .height(Fill);

        // A column lays out its `Fill` child last, so the Archived section takes the
        // height it needs at the bottom and the repo list scrolls in what's left.
        let mut panel = column![repos];
        if let Some(archived) = self.archived_section(workspace.archived()) {
            panel = panel.push(archived);
        }

        container(panel)
            .width(Fill)
            .height(Fill)
            .style(theme::background(c.panel))
            .into()
    }

    /// The archived repos, pinned to the bottom of the sidebar under a divider like a
    /// VS Code view pane: a header that opens and closes the list, and the list, which
    /// scrolls on its own past `ARCHIVED_LIST_MAX_HEIGHT`. `None` when nothing is
    /// archived.
    fn archived_section<'a>(&self, archived: &'a [Repo]) -> Option<Element<'a, Message>> {
        if archived.is_empty() {
            return None;
        }
        let c = &self.colors;
        let open = self.state.sidebar.archived_open;

        let header = button(
            row![
                chevron(c, open),
                theme::label(c, &format!("ARCHIVED · {}", archived.len())),
            ]
            .spacing(6)
            .align_y(Center),
        )
        .on_press(Message::ToggleArchived)
        .width(Fill)
        .padding([4.0, 8.0])
        .style(theme::list_row(c, false));

        let mut section = column![
            rule::horizontal(1).style(theme::divider(c)),
            container(header).padding([6.0, layouts::ROW_PADDING_X]),
        ];
        if open {
            let rows = archived.iter().map(|repo| archived_view(c, repo));
            let list = scrollable(container(column(rows).spacing(2)).padding(
                Padding::from([0.0, layouts::ROW_PADDING_X]).bottom(layouts::PANEL_PADDING_Y),
            ))
            .direction(Direction::Vertical(theme::embedded_scrollbar()))
            .style(theme::scrollbar(c))
            .width(Fill);

            section = section.push(container(list).max_height(layouts::ARCHIVED_LIST_MAX_HEIGHT));
        }
        Some(section.into())
    }
}

/// A repo's key in the keyed list: its folder, hashed, since iced wants a `Copy` key.
fn repo_key(repo: &Repo) -> u64 {
    let mut hasher = DefaultHasher::new();
    repo.path().hash(&mut hasher);
    hasher.finish()
}

/// "1 worktree", "2 worktrees", as tall as the archive button that replaces it on hover,
/// so the row's height doesn't change when one swaps for the other.
fn worktree_count<'a>(c: &Colors, repo: &Repo) -> Container<'a, Message> {
    let count = repo.worktrees().len();
    let label = if count == 1 {
        "1 worktree".to_string()
    } else {
        format!("{count} worktrees")
    };
    container(text(label).size(text_sizes::SMALL).color(c.text_dim))
        .height(layouts::SIDEBAR_ICON_BUTTON)
        .align_y(Center)
}

/// The sidebar's archive/unarchive button.
fn archive_button<'a>(c: &Colors, bytes: &'static [u8], message: Message) -> Element<'a, Message> {
    sized_icon_button(
        c,
        bytes,
        message,
        false,
        layouts::SIDEBAR_ICON_BUTTON,
        icon_sizes::SIDEBAR_BUTTON,
    )
    .into()
}

/// A repo's header (chevron, name, and the worktree count or, while hovered, the archive
/// button), which folds its worktree list away when clicked, and the list itself while
/// open.
fn repo_view<'a>(
    c: &Colors,
    repo: &'a Repo,
    selected: Option<&Path>,
    open: bool,
    hovered: bool,
) -> Element<'a, Message> {
    let path = repo.path().to_path_buf();
    // A click on the archive button doesn't also fold the repo: iced's button skips a
    // click its content already handled.
    let end = if hovered {
        archive_button(c, icons::ARCHIVE, Message::ArchiveRepo(path.clone()))
    } else {
        worktree_count(c, repo).into()
    };

    // The name fills the row and is cut to fit, so the count chip always keeps its place
    // at the end (a row lays out `Fill` children last, with whatever width is left).
    let header = button(
        row![
            chevron(c, open),
            ellipsized_text(repo.name())
                .font(fonts::UI_SEMIBOLD)
                .wrapping(Wrapping::None)
                .width(Fill),
            end,
        ]
        .spacing(6)
        .align_y(Center),
    )
    .on_press(Message::ToggleSection(Section::Repo(
        repo.name().to_string(),
    )))
    .width(Fill)
    .padding([2.0, 8.0])
    .style(theme::list_row(c, false));

    // Only the header counts as hovering the repo, not its worktree rows.
    let header = mouse_area(header)
        .on_enter(Message::RepoHovered {
            repo: path.clone(),
            hovered: true,
        })
        .on_exit(Message::RepoHovered {
            repo: path,
            hovered: false,
        });

    let mut view = column![header].spacing(2);
    if open {
        view = view.push(
            column(
                repo.worktrees()
                    .iter()
                    .map(|wt| worktree_view(c, wt, selected)),
            )
            .spacing(2),
        );
    }
    view.into()
}

/// An archived repo: its name, dimmed, then the unarchive button and the worktree count,
/// placed like a repo header's. It has no chevron (its worktrees aren't listed), so the
/// row is indented by the chevron and its gap to keep the names in line with the
/// headers above.
fn archived_view<'a>(c: &Colors, repo: &'a Repo) -> Element<'a, Message> {
    container(
        row![
            ellipsized_text(repo.name())
                .color(c.text_muted)
                .wrapping(Wrapping::None)
                .width(Fill),
            archive_button(
                c,
                icons::UNARCHIVE,
                Message::UnarchiveRepo(repo.path().to_path_buf()),
            ),
            worktree_count(c, repo),
        ]
        .spacing(6)
        .align_y(Center),
    )
    .width(Fill)
    .padding(Padding::from([2.0, 8.0]).left(8.0 + icon_sizes::CHEVRON + 6.0))
    .into()
}

fn worktree_view<'a>(
    c: &Colors,
    worktree: &'a Worktree,
    selected: Option<&Path>,
) -> Element<'a, Message> {
    let label = worktree.branch().unwrap_or(&worktree.head()[..7]);
    let is_selected = selected == Some(worktree.path());
    let status = worktree.cached_status();
    let tag = |label: &'static str, border: Color, color: Color| {
        container(
            text(label)
                .size(text_sizes::TAG)
                .line_height(layouts::TAG_LINE_HEIGHT),
        )
        .padding(layouts::TAG_PADDING)
        .style(theme::tag(border, color))
    };

    // The main tag's border is the 33% step (`scrollbar_hover`), as the old one was.
    let main_tag = worktree
        .is_main()
        .then(|| tag("main", c.scrollbar_hover, c.text_muted));
    let gone_tag = status
        .is_some_and(|s| s.is_upstream_gone())
        .then(|| tag("gone", c.danger_border, c.danger));
    let change_pill = status
        .map(|s| s.change_count())
        .filter(|count| *count > 0)
        .map(|count| {
            container(
                text(count.to_string())
                    .size(text_sizes::LABEL)
                    .font(fonts::UI_SEMIBOLD)
                    .line_height(layouts::PILL_LINE_HEIGHT),
            )
            .padding(layouts::PILL_PADDING)
            .style(theme::pill(c))
        });

    // Badges are pushed only when present: a `None` child in `row!` still takes a
    // spacing gap, which would eat into the label's width.
    let badges = [main_tag, gone_tag, change_pill]
        .into_iter()
        .flatten()
        .map(Element::from);

    button(
        row![
            icon(icons::GIT_BRANCH, icon_sizes::ROW, c.text_muted),
            ellipsized_text(label).wrapping(Wrapping::None).width(Fill),
        ]
        .extend(badges)
        .spacing(layouts::GAP)
        .align_y(Center),
    )
    .on_press(Message::WorktreeSelected(worktree.path().to_path_buf()))
    .width(Fill)
    .padding(Padding::from([6.0, 8.0]).left(26.0))
    .style(theme::list_row(c, is_selected))
    .into()
}
