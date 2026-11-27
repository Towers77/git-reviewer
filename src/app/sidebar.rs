//! The left panel: every repo under the root and its worktrees.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use iced::widget::scrollable::Direction;
use iced::widget::text::Wrapping;
use iced::widget::{
    Button, Container, button, column, container, keyed_column, mouse_area, row, rule, scrollable,
    space, text,
};
use iced::{Alignment::Center, Color, Element, Fill, Padding};
use iced_palace::widget::ellipsized_text;

use super::changes::sized_icon_button;
use super::popup::popup;
use super::reveal::reveal;
use super::{App, Message, OpenMenu, Section};
use crate::git::{Repo, Workspace, Worktree};
use crate::theme::icons::{self, chevron, icon};
use crate::theme::palette::Accent;
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
                    keyed_column(
                        workspace
                            .repos()
                            .iter()
                            .map(|repo| (repo_key(repo), self.repo_view(repo, selected))),
                    )
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
                // Not animated: the list is pinned to the bottom, so it would push the
                // repos above it around instead of folding.
                chevron(c, if open { 1.0 } else { 0.0 }),
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

/// "1 worktree", "2 worktrees", as tall as the ⋯ button that replaces it on hover, so the
/// row's height doesn't change when one swaps for the other.
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

/// The sidebar's small icon buttons: ⋯ and unarchive.
fn sidebar_button<'a>(c: &Colors, bytes: &'static [u8], message: Message) -> Button<'a, Message> {
    sized_icon_button(
        c,
        bytes,
        Some(message),
        false,
        layouts::SIDEBAR_ICON_BUTTON,
        icon_sizes::SIDEBAR_BUTTON,
    )
}

impl App {
    /// A repo's header (chevron, badge, name, and the worktree count or, while hovered
    /// or its menu is open, the ⋯ button), which folds its worktree list away when
    /// clicked, and the list itself while open.
    fn repo_view<'a>(&'a self, repo: &'a Repo, selected: Option<&Path>) -> Element<'a, Message> {
        let c = &self.colors;
        let path = repo.path().to_path_buf();
        let shown = self.fold(&Section::Repo(repo.name().to_string()));
        let menu_open =
            matches!(&self.menu, Some(OpenMenu::Repo(open)) if open.as_path() == repo.path());
        let hovered = self.hovered_repo.as_deref() == Some(repo.path());

        // A click on the ⋯ button doesn't also fold the repo: iced's button skips a click
        // its content already handled.
        let end: Element<'a, Message> = if hovered || menu_open {
            let more = sidebar_button(c, icons::ELLIPSIS, Message::RepoMenuToggled(path.clone()));
            if menu_open {
                more.style(theme::open_icon_button(c)).into()
            } else {
                more.into()
            }
        } else {
            worktree_count(c, repo).into()
        };

        // The name fills the row and is cut to fit, so the count always keeps its place
        // at the end (a row lays out `Fill` children last, with whatever width is left).
        let header = button(
            row![
                chevron(c, shown),
                badge(c, repo.name(), self.repo_color(repo.path())),
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
        .style({
            let (held, normal) = (theme::held_row(c), theme::list_row(c, false));
            move |theme, status| {
                if menu_open {
                    held(theme, status)
                } else {
                    normal(theme, status)
                }
            }
        });

        // Only the header counts as hovering the repo, not its worktree rows.
        let header = mouse_area(header)
            .on_enter(Message::RepoHovered {
                repo: path.clone(),
                hovered: true,
            })
            .on_exit(Message::RepoHovered {
                repo: path.clone(),
                hovered: false,
            });
        let menu = menu_open.then(|| self.repo_menu(repo));
        let header = popup(header, menu, Message::MenuDismissed);

        // The worktrees are built while any of them shows, folding included. The gap
        // above them is inside the fold, so a folded repo is just its header.
        let mut view = column![header];
        if shown > 0.0 {
            let worktrees = column(
                repo.worktrees()
                    .iter()
                    .map(|wt| worktree_view(c, wt, selected)),
            )
            .spacing(2)
            .padding(Padding::ZERO.top(2.0));
            view = view.push(reveal(worktrees, shown, c.panel));
        }
        view.into()
    }

    /// A repo's ⋯ menu: its badge colors, then "Archive repository".
    fn repo_menu<'a>(&self, repo: &Repo) -> Element<'a, Message> {
        let c = &self.colors;
        let picked = self.repo_color(repo.path());

        let swatches = Accent::REPO.chunks(SWATCH_COLUMNS).map(|line| {
            row(line.iter().map(|accent| {
                let pick = Message::RepoColorPicked(repo.path().to_path_buf(), *accent);
                swatch(c, c.named(*accent), *accent == picked, pick)
            }))
            .spacing(layouts::SWATCH_GAP)
            .into()
        });

        let archive = button(
            row![
                icon(icons::ARCHIVE, icon_sizes::ROW, c.text_muted),
                text("Archive repository"),
            ]
            .spacing(layouts::GAP)
            .height(Fill)
            .align_y(Center),
        )
        .on_press(Message::ArchiveRepo(repo.path().to_path_buf()))
        .width(Fill)
        .height(layouts::MENU_ITEM_HEIGHT)
        .padding([0.0, 8.0])
        .style(theme::list_row(c, false));

        container(
            column![
                container(theme::label(c, "COLOR")).padding([4.0, 6.0]),
                container(column(swatches).spacing(layouts::SWATCH_GAP))
                    .padding(Padding::from([4.0, 6.0]).bottom(8.0)),
                rule::horizontal(1).style(theme::divider(c)),
                archive,
            ]
            .spacing(4),
        )
        .width(layouts::REPO_MENU_WIDTH)
        .padding(6)
        .style(theme::menu(c))
        .into()
    }

    /// The color of a repo's badge: the one picked in its menu, or else one worked out
    /// from its folder, which then never changes (see `default_repo_color`).
    pub(super) fn repo_color(&self, repo: &Path) -> Accent {
        self.state
            .sidebar
            .repo_colors
            .get(repo)
            .copied()
            .unwrap_or_else(|| default_repo_color(repo))
    }
}

/// The swatches per line in the repo and theme menus: 13 or 14 colors in two lines.
pub(super) const SWATCH_COLUMNS: usize = 7;

/// A color to pick in a menu: a dot of `color`, ringed while it's the `picked` one.
pub(super) fn swatch<'a>(
    c: &Colors,
    color: Color,
    picked: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let dot = container(space())
        .width(Fill)
        .height(Fill)
        .style(theme::dot(color));
    button(dot)
        .on_press(on_press)
        .width(layouts::SWATCH)
        .height(layouts::SWATCH)
        .padding(4)
        .style(theme::swatch(c, picked))
        .into()
}

/// A repo's color until one is picked: one of `Accent::REPO`, from a hash of its folder.
/// That looks random from repo to repo but is the same on every run, so it needn't be
/// saved. FNV-1a by hand over the path's UTF-8, since std's hashers and its own path
/// encoding may change between Rust versions.
fn default_repo_color(repo: &Path) -> Accent {
    let hash = repo
        .to_string_lossy()
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
    Accent::REPO[(hash % Accent::REPO.len() as u64) as usize]
}

/// A repo's two-letter initials: the first letters of its first two words
/// (`git-reviewer` → GR), or its first two letters if it's one word (`infra` → IN).
fn initials(name: &str) -> String {
    let mut words = name
        .split(['-', '_', '.', ' '])
        .filter(|word| !word.is_empty());
    let letters: String = match (words.next(), words.next()) {
        (Some(first), Some(second)) => first
            .chars()
            .take(1)
            .chain(second.chars().take(1))
            .collect(),
        (Some(only), None) => only.chars().take(2).collect(),
        _ => name.chars().take(2).collect(),
    };
    // Cut again after uppercasing, which can make one letter two (ß → SS).
    letters.to_uppercase().chars().take(2).collect()
}

/// A count in a chip: `color` text on `background`, centered in a fixed height.
pub(super) fn chip<'a>(count: usize, color: Color, background: Color) -> Element<'a, Message> {
    container(
        text(count.to_string())
            .size(text_sizes::TAG)
            .font(fonts::UI_BOLD),
    )
    .height(layouts::CHIP_HEIGHT)
    .padding([0.0, 6.0])
    .align_y(Center)
    .style(theme::chip(color, background))
    .into()
}

/// A repo's badge: its initials, centered in a fixed box, on its color.
pub(super) fn badge<'a>(c: &Colors, name: &str, color: Accent) -> Element<'a, Message> {
    container(
        text(initials(name))
            .size(text_sizes::BADGE)
            .font(fonts::UI_BOLD),
    )
    .width(layouts::BADGE_WIDTH)
    .height(layouts::BADGE_HEIGHT)
    .padding(Padding::ZERO.bottom(layouts::BADGE_TEXT_LIFT))
    .align_x(Center)
    .align_y(Center)
    .style(theme::badge(c.named(color)))
    .into()
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
            sidebar_button(
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
    let gone = status.is_some_and(|s| s.is_upstream_gone());

    // The icon says what kind of worktree it is, in place of the old "main" and "gone"
    // tags: a book for the repo's main one (like VS Code's repository icon), a cut
    // branch in red when its upstream is gone, a branch otherwise.
    let (icon_bytes, icon_color, label_color) = if gone {
        (icons::GIT_BRANCH_BROKEN, c.danger, c.text_muted)
    } else if worktree.is_main() {
        (icons::BOOK_CHECK, c.accent, c.text)
    } else {
        (icons::GIT_BRANCH, c.accent, c.text)
    };

    let mut content = row![
        icon(icon_bytes, icon_sizes::ROW, icon_color),
        ellipsized_text(label)
            .color(label_color)
            .wrapping(Wrapping::None)
            .width(Fill),
    ]
    .spacing(layouts::GAP)
    .align_y(Center);
    // Pushed only when there's a count: a `None` child would still take a gap.
    if let Some(count) = status.map(|s| s.change_count()).filter(|count| *count > 0) {
        content = content.push(chip(count, c.modified, c.modified_chip));
    }

    button(content)
        .on_press(Message::WorktreeSelected(worktree.path().to_path_buf()))
        .width(Fill)
        .padding(Padding::from([6.0, 8.0]).left(26.0))
        .style(theme::list_row(c, is_selected))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initials() {
        assert_eq!(initials("git-reviewer"), "GR");
        assert_eq!(initials("billing_service"), "BS");
        assert_eq!(initials("my.app"), "MA");
        assert_eq!(initials("infra"), "IN");
        assert_eq!(initials("x"), "X");
        assert_eq!(initials("-web-"), "WE");
        assert_eq!(initials("ñandú-api"), "ÑA");
        assert_eq!(initials("ßx"), "SS");
    }

    #[test]
    fn test_default_repo_color_is_stable_and_never_red() {
        let repo = Path::new(r"C:\repos\web-app");
        assert_eq!(default_repo_color(repo), default_repo_color(repo));
        // Repos spread over the colors, and none get red.
        let colors: std::collections::HashSet<Accent> = (0..200)
            .map(|i| default_repo_color(Path::new(&format!(r"C:\repos\repo-{i}"))))
            .collect();
        assert!(colors.len() > 10, "{colors:?}");
        assert!(!colors.contains(&Accent::Red));
    }
}
