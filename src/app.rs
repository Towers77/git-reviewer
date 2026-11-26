//! The application: state, messages, `update`, and `view`, which puts the top bar over
//! the three panels. Each panel lives in its own file as an `impl App` block; being child
//! modules, they can read `App`'s private fields directly.

mod changes;
mod diff;
mod file_icon;
mod panels;
mod popup;
mod sidebar;
mod sticky;
mod watch;

use std::collections::{BTreeSet, HashSet};
use std::env::args;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf, absolute};
use std::sync::{Arc, LazyLock, mpsc};
use std::thread;

use iced::futures::channel::oneshot;
use iced::widget::scrollable::Direction;
use iced::widget::{
    Container, button, column, container, opaque, row, rule, scrollable, space, stack, text,
    text_editor,
};
use iced::{Alignment::Center, Color, Element, Fill, Padding, Subscription, Task, Theme, window};
use rfd::{MessageButtons, MessageDialogResult, MessageLevel};
use std::time::{Duration, Instant};

use crate::config::{Config, ConfigError};
use crate::git::{ChangeKind, Diff, DiffSource, GitError, Repo, Status, Workspace, Worktree};
use crate::highlight::{Flavor, LineColors};
use crate::state::State;
use crate::theme::icons::{self, icon};
use crate::theme::palette::Accent;
use crate::theme::{self, Colors, fonts, icon_sizes, layouts, text_sizes};
use diff::DiffView;
use panels::Divider;
pub use panels::{PanelWidths, min_window_width};

#[derive(Debug, Clone)]
pub enum Message {
    WorktreeSelected(PathBuf),
    FileSelected {
        path: String,
        source: DiffSource,
    },
    /// The diff was scrolled; how far down it is now, in pixels.
    DiffScrolled(f32),
    /// The syntax colors of a diff's lines are worked out (see
    /// `DiffView::start_highlighting`); `None` if it's not a language we highlight.
    DiffHighlighted(Arc<Diff>, Option<Vec<LineColors>>),
    /// A rescan is done (see `App::rescan`); `None` if its thread panicked.
    Rescanned(Option<Scanned>),
    /// The selected worktree's status is read (see `App::load_status`): the request's
    /// number, and what was read (`None` if its thread panicked).
    StatusLoaded(u64, Option<LoadedStatus>),
    /// A file's diff is read (see `App::load_diff`): the request's number, which file
    /// and side, and the diff (`None` if its thread panicked).
    DiffLoaded(u64, (String, DiffSource), Option<Result<Diff, GitError>>),
    /// A git action that changes the selected worktree (stage, discard...) is done.
    ActionDone(Result<(), GitError>),
    /// A commit is done: the message it was made with, and how it went.
    Committed(String, Result<(), GitError>),
    /// A worktree removal is done: which worktree, and how it went.
    Removed(PathBuf, Result<(), GitError>),
    Refresh,
    /// Something changed on disk in a worktree (from the file watcher).
    FilesChanged,
    WatchFailed(String),
    /// Open the system folder picker to choose the root.
    ChooseRoot,
    /// The picker closed: the chosen folder, or `None` if it was cancelled.
    RootChosen(Option<PathBuf>),
    /// A frame passed while a Sync/Publish runs; only redraws the spinning icon.
    Tick,
    /// A discard button was clicked: ask for confirmation first.
    DiscardRequested(DiscardScope),
    /// The confirmation dialog closed; `true` if the user said yes.
    DiscardAnswered(DiscardPlan, bool),
    Stage(String),
    Unstage(String),
    StageAll,
    UnstageAll,
    CommitMessageEdited(text_editor::Action),
    Commit,
    FetchAll,
    Fetched(Result<(), GitError>),
    Sync,
    Synced(Result<(), GitError>),
    Publish,
    Published(Result<(), GitError>),
    /// A footer button (remove worktree / delete branch) was clicked.
    RemovalRequested(Removal),
    /// The confirmation for a worktree with uncommitted changes closed; `true` if yes.
    RemovalAnswered(RemovalPlan, bool),
    /// A section header was clicked: fold the section away, or open it again.
    ToggleSection(Section),
    /// "Archive repository" was clicked in a repo's menu, with the repo's folder.
    ArchiveRepo(PathBuf),
    /// A repo's ⋯ button was clicked, with the repo's folder: open its menu, or close it.
    RepoMenuToggled(PathBuf),
    /// The open menu was closed by a click outside it or Escape (see `popup`).
    MenuDismissed,
    /// A color was picked in a repo's menu, for its badge.
    RepoColorPicked(PathBuf, Accent),
    /// An archived repo's unarchive button was clicked, with the repo's folder.
    UnarchiveRepo(PathBuf),
    /// The sidebar's Archived header was clicked: open or close the list.
    ToggleArchived,
    /// The cursor entered (`hovered`) or left a repo's header in the sidebar.
    RepoHovered {
        repo: PathBuf,
        hovered: bool,
    },
    /// The error toast's close button was clicked.
    DismissError,
    /// The mouse went down on a divider between panels.
    DividerGrabbed(Divider),
    /// The cursor's x while a divider is dragged.
    DividerDragged(f32),
    DividerReleased,
    /// The window's new width.
    WindowResized(f32),
}

/// A list section that folds away when its header is clicked, like VS Code's.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Section {
    /// The staged files group.
    Staged,
    /// The unstaged files group ("CHANGES").
    Changes,
    /// A repo's worktree list in the sidebar, by the repo's name (its folder's name,
    /// unique under the root).
    Repo(String),
}

/// Which footer button was clicked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Removal {
    Worktree,
    /// Remove the worktree, then delete its branch.
    Branch,
}

/// What a removal will act on, fixed when the button is clicked, like `DiscardPlan`:
/// the dialog doesn't block the app, so another worktree may be selected by "Yes" time.
#[derive(Debug, Clone)]
pub struct RemovalPlan {
    removal: Removal,
    worktree: PathBuf,
    /// The branch checked out there; `None` when detached.
    branch: Option<String>,
    /// What the dialog calls the worktree: its branch, or its short commit if detached.
    name: String,
    /// Uncommitted changes when clicked. Any at all means asking first, then forcing git.
    changes: usize,
}

/// Which discard button was clicked.
#[derive(Debug, Clone)]
pub enum DiscardScope {
    File {
        path: String,
        untracked: bool,
    },
    /// Every unstaged change in the worktree, untracked files included.
    All,
}

/// Exactly what a confirmed discard will do, fixed when the button is clicked. The
/// dialog doesn't block the app, so nothing is re-read at "Yes" time: switching
/// worktrees or a file appearing meanwhile can't change what gets thrown away.
#[derive(Debug, Clone)]
pub struct DiscardPlan {
    worktree: PathBuf,
    /// `(path, is_untracked)`, as listed when the button was clicked.
    files: Vec<(String, bool)>,
}

/// A workspace scanned on its own thread (see `App::rescan`), and the
/// `App::workspace_version` it was started at. The workspace is in an `Arc` only
/// because a `Message` must be `Clone`; it never actually gets cloned, so
/// `Arc::unwrap_or_clone` takes it out for free.
#[derive(Clone)]
pub struct Scanned {
    version: u64,
    workspace: Arc<Result<Workspace, AppError>>,
}

/// By hand, since the git types have no `Debug` (a `Message` needs one).
impl fmt::Debug for Scanned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Scanned(version {})", self.version)
    }
}

/// A worktree's status and how far it is from its base, read on a thread of its own
/// (see `App::load_status`).
#[derive(Debug, Clone)]
pub struct LoadedStatus {
    status: Result<Status, GitError>,
    /// `None` with no base to compare with.
    base: Option<BaseCounts>,
    /// Whether that base came from the config: one that doesn't resolve is then a typo
    /// worth reporting.
    base_is_configured: bool,
}

/// The base a worktree is compared with (the config's, else origin/HEAD), and its
/// ahead/behind counts against it.
type BaseCounts = (String, Result<(usize, usize), GitError>);

/// Where the background rescan is at (see `App::rescan`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rescan {
    Idle,
    Running,
    /// Running, and another one is due once it's done.
    Queued,
}

/// A push/pull running in the background, which the primary button shows as busy.
#[derive(Debug, Clone, Copy, PartialEq)]
enum RemoteOp {
    Sync,
    Publish,
}

pub struct App {
    workspace: Result<Workspace, AppError>,
    rescan: Rescan,
    /// Goes up with every scan done right away (`scan_now`), so a background one
    /// started before it is dropped: it read the repos before whatever that was for
    /// (a worktree removed, another root).
    workspace_version: u64,
    /// The Sync or Publish in flight and when it started (the spinner's angle comes
    /// from the elapsed time). `None` when idle.
    remote_op: Option<(RemoteOp, Instant)>,
    /// A confirmation (discard or removal) is on screen; clicks that would open another
    /// are ignored.
    dialog_open: bool,
    selected_worktree_path: Option<PathBuf>,
    selected_file: Option<(String, DiffSource)>,
    /// `None` while there's no worktree selected, and while its status is being read.
    status: Option<Result<Status, GitError>>,
    /// The number of the newest status and diff reads asked for (see `load_status` and
    /// `load_diff`): an older one may come back later, and is then dropped.
    status_request: u64,
    diff_request: u64,
    /// When the newest diff read was asked for: the loading spinner's clock.
    diff_requested_at: Instant,
    /// A commit is running (see `Message::Commit`).
    committing: bool,
    /// `None` until a root is given on the command line or saved in the config.
    root: Option<PathBuf>,
    config: Config,
    /// False when the config file exists but couldn't be read: it's then never written,
    /// so whatever the user put in it isn't destroyed.
    config_is_writable: bool,
    /// The selected file's diff, already laid out for drawing (see `diff::DiffView`).
    diff: Option<Result<DiffView, GitError>>,
    /// How far down the diff is scrolled, in pixels: only the lines around it are built
    /// (see `diff::chunk_layers`).
    diff_scroll_y: f32,
    error: Option<AppError>,
    commit_message: text_editor::Content,
    base_counts: Option<(String, (usize, usize))>,
    /// Set when the selected repo's base from the config doesn't resolve. Kept apart
    /// from `error` so it neither hides nor gets hidden by git errors.
    base_error: Option<String>,
    /// Sections folded away by the user; every other section is open. Kept while the
    /// app runs, across worktree switches, like VS Code (not saved to the config).
    collapsed: HashSet<Section>,
    /// The repo whose sidebar header is under the cursor, by folder: it shows its archive
    /// button in place of its worktree count.
    hovered_repo: Option<PathBuf>,
    /// The repo whose ⋯ menu is open, by folder.
    repo_menu: Option<PathBuf>,
    /// What's remembered between runs in `state.toml`: the panel widths the user dragged
    /// to (what's shown may be narrower: see `PanelWidths::fitted`).
    state: State,
    /// The divider being dragged, if any, and the panel widths when the drag started
    /// (every move is worked out from them: see `PanelWidths::dragged`).
    dragging: Option<(Divider, PanelWidths)>,
    /// Infinite until the window first reports its size, which it does when it opens.
    window_width: f32,
    /// Every color the UI draws with, worked out from `state.theme`, and iced's theme
    /// made from them. Both are rebuilt only when the theme changes.
    colors: Colors,
    iced_theme: Theme,
}
impl App {
    /// The root comes from the command line when one is given (and is then remembered
    /// in the config), otherwise from the config.
    pub fn new() -> Self {
        let mut error = None;
        // Only an existing folder is used (and remembered): a typo on the command line
        // must not replace a good saved root.
        let cli_root = args()
            .nth(1)
            .map(PathBuf::from)
            .and_then(|path| match absolute(&path) {
                Ok(path) if path.is_dir() => Some(path),
                _ => {
                    error = Some(AppError::Message(format!(
                        "`{}` is not a folder; ignoring it",
                        path.display()
                    )));
                    None
                }
            });

        let loaded = Config::load();
        let config_is_writable = loaded.is_ok();
        let config = match loaded {
            Ok(mut config) => {
                if cli_root.is_some() && config.root != cli_root {
                    config.root = cli_root.clone();
                    if let Err(err) = config.save() {
                        error = Some(config_error(err));
                    }
                }
                config
            }
            // An unreadable or invalid file is reported and left untouched (saving
            // over it would destroy whatever the user wrote); the app runs on defaults.
            Err(err) => {
                error = Some(config_error(err));
                Config::default()
            }
        };

        let (state, bad_theme) = State::load();
        if let Some(warning) = &bad_theme {
            // The toast holds one message: a config or command-line error matters more.
            error.get_or_insert_with(|| AppError::Message(warning.clone()));
        }
        let colors = Colors::new(&state.theme);

        let mut app = App {
            workspace: Err(AppError::Message(String::new())), // replaced by the scan below
            rescan: Rescan::Idle,
            workspace_version: 0,
            remote_op: None,
            dialog_open: false,
            selected_worktree_path: None,
            selected_file: None,
            status: None,
            status_request: 0,
            diff_request: 0,
            diff_requested_at: Instant::now(),
            committing: false,
            root: cli_root.or_else(|| config.root.clone()),
            config,
            config_is_writable,
            diff: None,
            diff_scroll_y: 0.0,
            error,
            commit_message: text_editor::Content::new(),
            base_counts: None,
            base_error: None,
            collapsed: HashSet::new(),
            hovered_repo: None,
            repo_menu: None,
            state,
            dragging: None,
            window_width: f32::INFINITY,
            iced_theme: theme::app_theme(&colors),
            colors,
        };
        // Written now, so the file matches what the toast says.
        if bad_theme.is_some() {
            app.save_state("the default theme");
        }
        app.scan_now();
        app
    }

    /// Pins down what a click on a discard button would throw away: the selected
    /// worktree and the exact unstaged files listed right now. `None` when there's
    /// nothing to discard (e.g. a stale click after the list changed).
    fn discard_plan(&self, scope: DiscardScope) -> Option<DiscardPlan> {
        let worktree = self.selected_worktree_path.clone()?;
        let files = match scope {
            DiscardScope::File { path, untracked } => vec![(path, untracked)],
            DiscardScope::All => {
                let Some(Ok(status)) = &self.status else {
                    return None;
                };
                status
                    .files
                    .iter()
                    .filter_map(|file| {
                        let kind = file.unstaged?;
                        Some((file.path.clone(), kind == ChangeKind::Untracked))
                    })
                    .collect()
            }
        };

        (!files.is_empty()).then_some(DiscardPlan { worktree, files })
    }

    /// Pins down what a click on a footer button would remove: the selected worktree,
    /// its branch, and how many uncommitted changes it has right now.
    fn removal_plan(&self, removal: Removal) -> Option<RemovalPlan> {
        let worktree = self.selected_worktree()?;
        let Some(Ok(status)) = &self.status else {
            return None;
        };
        let branch = worktree.branch().map(String::from);
        if removal == Removal::Branch && branch.is_none() {
            return None;
        }

        Some(RemovalPlan {
            removal,
            worktree: worktree.path().to_path_buf(),
            name: branch
                .clone()
                .unwrap_or_else(|| worktree.head()[..7].to_string()),
            branch,
            changes: status.files.len(),
        })
    }

    /// Carries out a removal on the worktree it was planned for (found by path, not
    /// whichever is selected now), in order on the git thread (see `in_order`): deleting
    /// a big worktree's files takes a while. `force` lets git throw away the worktree's
    /// uncommitted changes; the branch is still deleted with `-d`, which refuses to lose
    /// commits. `Message::Removed` follows.
    fn remove(&self, plan: RemovalPlan, force: bool) -> Task<Message> {
        let repo = match &self.workspace {
            Ok(workspace) => workspace.find_repo(&plan.worktree).cloned(),
            Err(_) => None,
        };
        let Some(repo) = repo else {
            return Task::none();
        };

        let worktree = plan.worktree.clone();
        in_order(move || {
            repo.remove_worktree(&plan.worktree, force).and_then(|()| {
                match (plan.removal, &plan.branch) {
                    (Removal::Branch, Some(branch)) => repo.delete_branch(branch),
                    _ => Ok(()),
                }
            })
        })
        .map(move |result| Message::Removed(worktree.clone(), or_crashed(result)))
    }

    /// Switches to another root folder: remembers it in the config, forgets the old
    /// selection, and rescans. The watcher follows on its own, since the worktree list
    /// it's keyed on changes.
    fn set_root(&mut self, root: PathBuf) -> Task<Message> {
        self.root = Some(root.clone());
        if self.config_is_writable {
            self.config.root = Some(root);
            if let Err(err) = self.config.save() {
                self.error = Some(config_error(err));
            }
        }

        self.scan_now();
        self.select_worktree(None)
    }

    /// Selects a worktree, or none, and forgets what was shown of the previous one. Its
    /// status is read next: the `Task` that does it.
    fn select_worktree(&mut self, path: Option<PathBuf>) -> Task<Message> {
        self.selected_worktree_path = path;
        self.selected_file = None;
        self.diff = None;
        // Not the previous worktree's while this one's loads: its files could be
        // clicked, in the wrong worktree.
        self.status = None;
        self.base_counts = None;
        self.base_error = None;
        self.load_status()
    }

    /// Scans the workspace right away, on the UI thread: for the moments that can't
    /// show the old one any longer (startup, another root). Everything else uses
    /// `rescan`.
    fn scan_now(&mut self) {
        self.workspace = scan_root(self.root.as_deref(), &self.state.sidebar.archived);
        self.workspace_version += 1;
        self.repo_menu = None;
    }

    /// Drops the result of a rescan that's running, and queues a fresh one: for changes
    /// that scan would undo, as it read the repos (or the archived list) before them.
    fn invalidate_scans(&mut self) {
        self.workspace_version += 1;
        if self.rescan == Rescan::Running {
            self.rescan = Rescan::Queued;
        }
    }

    /// Scans the workspace again, on a thread of its own (see `on_thread`): that's a
    /// few git calls per worktree (half a second for 25 worktrees, measured), and the
    /// app must keep responding meanwhile. The result comes back as
    /// `Message::Rescanned`.
    ///
    /// One at a time: a rescan asked for while one runs is queued, once however often
    /// it's asked for (file changes come in bursts), and starts when that one is done.
    /// The running one may have read the repos before whatever changed.
    fn rescan(&mut self) -> Task<Message> {
        match self.rescan {
            Rescan::Idle => {
                self.rescan = Rescan::Running;
                let (root, version) = (self.root.clone(), self.workspace_version);
                let archived = self.state.sidebar.archived.clone();
                on_thread(move || {
                    let workspace = Arc::new(scan_root(root.as_deref(), &archived));
                    Scanned { version, workspace }
                })
                .map(Message::Rescanned)
            }
            Rescan::Running | Rescan::Queued => {
                self.rescan = Rescan::Queued;
                Task::none()
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);

        // Many messages can load a diff (opening a file, staging, a refresh...), so a
        // new one's highlighting is started here, after any of them.
        let flavor = Flavor::for_background(self.colors.light);
        match &mut self.diff {
            Some(Ok(diff)) => match diff.start_highlighting(flavor) {
                Some(highlighting) => Task::batch([task, highlighting]),
                None => task,
            },
            _ => task,
        }
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WorktreeSelected(path) => return self.select_worktree(Some(path)),
            Message::Refresh | Message::FilesChanged => {
                // The Refresh button also re-reads the config's bases, so fixing one
                // doesn't need a restart. (The root stays: it decides what's watched.)
                if matches!(message, Message::Refresh) {
                    match Config::load() {
                        Ok(config) => self.config.bases = config.bases,
                        Err(err) => self.error = Some(config_error(err)),
                    }
                }
                return self.rescan();
            }
            Message::Rescanned(scan) => {
                let queued = self.rescan == Rescan::Queued;
                self.rescan = Rescan::Idle;
                let mut tasks = Vec::new();
                match scan {
                    // Dropped if the workspace was scanned right away meanwhile (see
                    // `workspace_version`).
                    Some(Scanned { version, workspace }) if version == self.workspace_version => {
                        self.workspace = Arc::unwrap_or_clone(workspace);
                        // A menu stays open only on a repo that's still there; else it
                        // would open by itself if the repo came back.
                        let menu_repo_kept = self.workspace.as_ref().is_ok_and(|workspace| {
                            workspace
                                .repos()
                                .iter()
                                .any(|repo| Some(repo.path()) == self.repo_menu.as_deref())
                        });
                        if !menu_repo_kept {
                            self.repo_menu = None;
                        }
                        // The selected worktree may be gone (removed outside the app).
                        let task = if self.selected_worktree().is_none() {
                            self.select_worktree(None)
                        } else {
                            self.load_status()
                        };
                        tasks.push(task);
                    }
                    Some(_) => {}
                    None => {
                        self.error = Some(AppError::Message(
                            "Refreshing stopped unexpectedly (an internal error)".to_string(),
                        ));
                    }
                }
                if queued {
                    tasks.push(self.rescan());
                }
                return Task::batch(tasks);
            }
            Message::StatusLoaded(request, loaded) => {
                // Only the newest read asked for: an older one read the repo before
                // whatever the newer one was asked for.
                if request != self.status_request {
                    return Task::none();
                }
                match loaded {
                    Some(loaded) => self.apply_status(loaded),
                    None => self.status = Some(Err(thread_crashed())),
                }
                return self.retarget_selected_file();
            }
            Message::DiffLoaded(request, file, diff) => {
                // Only the newest read asked for, and only while that file and side are
                // still the selected ones.
                if request == self.diff_request && self.selected_file.as_ref() == Some(&file) {
                    self.show_diff(&file.0, file.1, or_crashed(diff));
                }
            }
            Message::ActionDone(result) => {
                self.report(result);
                return self.load_status();
            }
            Message::Committed(message, result) => {
                self.committing = false;
                // Cleared, unless it was edited while the commit ran.
                if result.is_ok() && self.commit_message.text() == message {
                    self.commit_message = text_editor::Content::new();
                }
                self.report(result);
                return self.load_status();
            }
            Message::Removed(worktree, result) => {
                let removed = result.is_ok();
                self.report(result);
                // A scan that started before this still lists the removed worktree.
                self.invalidate_scans();
                let mut tasks = vec![self.rescan()];
                if removed && self.selected_worktree_path.as_ref() == Some(&worktree) {
                    tasks.push(self.select_worktree(None));
                } else {
                    tasks.push(self.load_status());
                }
                return Task::batch(tasks);
            }
            Message::WatchFailed(reason) => {
                self.error = Some(AppError::Message(format!(
                    "Auto-refresh unavailable ({reason}); use Refresh"
                )));
            }
            Message::ChooseRoot => {
                let mut dialog = rfd::AsyncFileDialog::new()
                    .set_title("Choose the folder that holds your repos");
                if let Some(root) = &self.root {
                    dialog = dialog.set_directory(root);
                }
                // The dialog is the OS's own; awaiting it as a task keeps the app's
                // event loop running while it's open.
                return Task::perform(dialog.pick_folder(), |folder| {
                    Message::RootChosen(folder.map(|handle| handle.path().to_path_buf()))
                });
            }
            Message::RootChosen(Some(root)) => return self.set_root(root),
            Message::RootChosen(None) => {} // picker cancelled
            Message::FileSelected { path, source } => {
                // The file already shown: nothing to do (as in VS Code).
                if matches!(&self.diff, Some(Ok(view)) if view.is_of(&path, source)) {
                    return Task::none();
                }
                let file = (path, source);
                self.selected_file = Some(file);
                // Loading, with no scrollable meanwhile: the new one starts at the top.
                self.diff = None;
                self.diff_scroll_y = 0.0;
                return self.load_diff();
            }
            Message::DiffScrolled(y) => self.diff_scroll_y = y,
            Message::DiffHighlighted(diff, colors) => {
                if let Some(Ok(view)) = &mut self.diff {
                    view.set_colors(&diff, colors.as_deref());
                }
            }
            Message::Stage(path) => return self.act(move |worktree| worktree.stage(&path)),
            Message::Unstage(path) => return self.act(move |worktree| worktree.unstage(&path)),
            Message::StageAll => return self.act(Worktree::stage_all),
            Message::UnstageAll => return self.act(Worktree::unstage_all),
            Message::CommitMessageEdited(action) => {
                self.commit_message.perform(action);
            }
            // One at a time: a second click (or ⌘/Ctrl+Enter) while one runs does nothing.
            Message::Commit if !self.committing => {
                if let Some(worktree) = self.selected_worktree().cloned() {
                    self.committing = true;
                    let message = self.commit_message.text();
                    return in_order(move || {
                        let result = worktree.commit(&message);
                        (message, result)
                    })
                    .map(|done| {
                        let (message, result) =
                            done.unwrap_or_else(|| (String::new(), Err(thread_crashed())));
                        Message::Committed(message, result)
                    });
                }
            }
            Message::Commit => {} // one is already running
            Message::FetchAll => {
                if let Ok(workspace) = &self.workspace {
                    let repos = workspace.repos().to_vec();
                    return on_thread(move || Repo::fetch_all(&repos))
                        .map(|result| Message::Fetched(or_crashed(result)));
                }
            }
            Message::Fetched(result) => {
                self.report(result);
                // `fetch --prune` can make upstreams gone in any worktree, so rescan
                // everything rather than just the selected status.
                return self.rescan();
            }
            // One push/pull at a time: the button is disabled while one runs, and these
            // guards also cover a message that was already queued.
            Message::Sync if self.remote_op.is_none() => {
                if let Some(worktree) = self.selected_worktree().cloned() {
                    self.remote_op = Some((RemoteOp::Sync, Instant::now()));
                    // In order: a pull changes the worktree, like staging does.
                    return in_order(move || worktree.sync())
                        .map(|result| Message::Synced(or_crashed(result)));
                }
            }
            Message::Synced(result) => {
                self.remote_op = None;
                self.report(result);
                return self.load_status();
            }
            Message::Publish if self.remote_op.is_none() => {
                if let Some(worktree) = self.selected_worktree().cloned() {
                    self.remote_op = Some((RemoteOp::Publish, Instant::now()));
                    // Not in order: a push changes nothing in the worktree, so staging
                    // and committing needn't wait for the network.
                    return on_thread(move || worktree.publish())
                        .map(|result| Message::Published(or_crashed(result)));
                }
            }
            Message::Published(result) => {
                self.remote_op = None;
                self.report(result);
                return self.load_status();
            }
            Message::DiscardRequested(scope) => {
                // One dialog at a time: a second click while one is open does nothing.
                if self.dialog_open {
                    return Task::none();
                }
                let Some(plan) = self.discard_plan(scope) else {
                    return Task::none();
                };
                // The OS's own dialog: unmissable, and it can't be clicked through by
                // accident. Yes/No rather than custom labels, which Windows only shows
                // with extra setup.
                let dialog = rfd::AsyncMessageDialog::new()
                    .set_level(MessageLevel::Warning)
                    .set_title("Discard changes")
                    .set_description(discard_question(&plan))
                    .set_buttons(MessageButtons::YesNo);

                self.dialog_open = true;
                return Task::perform(dialog.show(), move |answer| {
                    Message::DiscardAnswered(plan, answer == MessageDialogResult::Yes)
                });
            }
            Message::DiscardAnswered(plan, confirmed) => {
                self.dialog_open = false;
                // The worktree the plan was made for, looked up by path: not whichever
                // one happens to be selected now.
                let worktree = match &self.workspace {
                    Ok(workspace) if confirmed => workspace.find_worktree(&plan.worktree).cloned(),
                    _ => None,
                };
                if let Some(worktree) = worktree {
                    return in_order(move || worktree.discard(&plan.files))
                        .map(|result| Message::ActionDone(or_crashed(result)));
                }
            }
            Message::Sync | Message::Publish => {} // one is already running
            Message::Tick => {}                    // nothing to update; the redraw is the point
            Message::RemovalRequested(removal) => {
                if self.dialog_open {
                    return Task::none();
                }
                let Some(plan) = self.removal_plan(removal) else {
                    return Task::none();
                };
                // Nothing uncommitted means nothing to lose: no question asked.
                if plan.changes == 0 {
                    return self.remove(plan, false);
                }
                // The same OS dialog as discarding (see `DiscardRequested`).
                let dialog = rfd::AsyncMessageDialog::new()
                    .set_level(MessageLevel::Warning)
                    .set_title(match removal {
                        Removal::Worktree => "Remove worktree",
                        Removal::Branch => "Delete branch",
                    })
                    .set_description(removal_question(&plan))
                    .set_buttons(MessageButtons::YesNo);

                self.dialog_open = true;
                return Task::perform(dialog.show(), move |answer| {
                    Message::RemovalAnswered(plan, answer == MessageDialogResult::Yes)
                });
            }
            Message::RemovalAnswered(plan, confirmed) => {
                self.dialog_open = false;
                if confirmed {
                    return self.remove(plan, true);
                }
            }
            Message::ToggleSection(section) => {
                // `remove` is false when it wasn't collapsed: then collapse it.
                if !self.collapsed.remove(&section) {
                    self.collapsed.insert(section);
                }
            }
            Message::ArchiveRepo(repo) => {
                // An archived repo's worktrees aren't listed or refreshed any more, so
                // its selected worktree would be left on screen going stale.
                let task = if self
                    .selected_repo()
                    .is_some_and(|selected| selected.path() == repo)
                {
                    self.select_worktree(None)
                } else {
                    Task::none()
                };
                // Moved rather than rescanned: a rescan runs git in every repo, which
                // takes most of a second. One that's running read the old archived
                // list, so it would put the repo back: it's redone.
                if let Ok(workspace) = &mut self.workspace {
                    workspace.archive(&repo);
                }
                self.invalidate_scans();
                // Its row is gone, so its `on_exit` never comes: without this, the repo
                // would come back from the Archived list showing its ⋯ button.
                if self.hovered_repo.as_ref() == Some(&repo) {
                    self.hovered_repo = None;
                }
                self.repo_menu = None;
                self.state.sidebar.archived.insert(repo);
                self.save_state("the archived repos");
                return task;
            }
            Message::UnarchiveRepo(repo) => {
                // Only this repo is reopened. The watcher picks its worktrees up, since
                // the list it's keyed on changes.
                let result = match &mut self.workspace {
                    Ok(workspace) => workspace.unarchive(&repo),
                    Err(_) => Ok(()),
                };
                // A repo git can't open stays archived, so the list and the file agree.
                if result.is_ok() {
                    self.state.sidebar.archived.remove(&repo);
                    self.save_state("the archived repos");
                    // As when archiving: a running rescan read the old archived list.
                    self.invalidate_scans();
                }
                self.report(result);
            }
            Message::RepoHovered { repo, hovered } => {
                if hovered {
                    self.hovered_repo = Some(repo);
                } else if self.hovered_repo.as_ref() == Some(&repo) {
                    // Only the row being left: when moving between rows, the next one's
                    // enter may arrive before this exit.
                    self.hovered_repo = None;
                }
            }
            Message::RepoMenuToggled(repo) => {
                self.repo_menu = if self.repo_menu.as_ref() == Some(&repo) {
                    None
                } else {
                    Some(repo)
                };
            }
            Message::MenuDismissed => self.repo_menu = None,
            // The menu stays open, so the new pick shows in its swatches.
            Message::RepoColorPicked(repo, color) => {
                self.state.sidebar.repo_colors.insert(repo, color);
                self.save_state("the repo's color");
            }
            Message::ToggleArchived => {
                self.state.sidebar.archived_open = !self.state.sidebar.archived_open;
                self.save_state("the Archived list's state");
            }
            Message::DismissError => self.error = None,
            Message::DividerGrabbed(divider) => {
                self.dragging = Some((divider, self.state.panels));
            }
            Message::DividerDragged(x) => {
                if let Some((divider, start)) = self.dragging {
                    self.state.panels = start.dragged(divider, x, self.window_width);
                }
            }
            Message::DividerReleased => {
                self.dragging = None;
                // Saved once, when the drag ends, rather than on every move.
                self.save_state("the panel widths");
            }
            Message::WindowResized(width) => self.window_width = width,
        }
        Task::none()
    }

    /// Shows a failed action's error in the toast. Success leaves the toast alone: like
    /// VS Code's error notifications, it stays until it's closed or a newer error
    /// replaces it, so it can't vanish before it's read.
    fn report<T>(&mut self, result: Result<T, GitError>) {
        if let Err(err) = result {
            self.error = Some(err.into());
        }
    }

    /// Writes `state.toml`. A failure is shown in the toast, naming `what` wasn't saved;
    /// the change still applies until the app closes.
    fn save_state(&mut self, what: &str) {
        if let Err(err) = self.state.save() {
            self.error = Some(AppError::Message(format!("Couldn't save {what}: {err}")));
        }
    }

    /// Watches every worktree's folder so the app refreshes itself after edits or
    /// terminal git commands; also follows the window's width, and the cursor while a
    /// divider is dragged.
    pub fn subscription(&self) -> Subscription<Message> {
        // Always on, even without a workspace: the window reports its first size once,
        // when it opens.
        let window_width = panels::window_width();
        let drag = match self.dragging {
            Some(_) => panels::drag_events(),
            None => Subscription::none(),
        };

        let Ok(workspace) = &self.workspace else {
            return Subscription::batch([window_width, drag]);
        };

        let mut paths: Vec<PathBuf> = workspace
            .repos()
            .iter()
            .flat_map(|repo| repo.worktrees())
            .map(|worktree| worktree.path().to_path_buf())
            .collect();
        // Sorted so the same worktrees always hash the same, and iced keeps the
        // running watcher instead of restarting it on every refresh.
        paths.sort();

        // A message per frame only while a Sync/Publish runs or the diff is loading, to
        // animate their spinners; idle, the app draws nothing.
        let spinner = if self.remote_op.is_some() || self.diff_loading_for().is_some() {
            window::frames().map(|_| Message::Tick)
        } else {
            Subscription::none()
        };

        Subscription::batch([watch::subscription(paths), spinner, window_width, drag])
    }

    /// iced's theme: asked for by iced (see main.rs), a cheap clone of a shared one.
    pub fn theme(&self) -> Theme {
        self.iced_theme.clone()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let c = &self.colors;
        let body: Element<'_, Message> = match &self.workspace {
            // No root yet, or it can't be read (moved, deleted): offer to pick one.
            Err(err) => {
                let color = if self.root.is_some() {
                    c.danger
                } else {
                    c.text_muted
                };

                container(
                    column![
                        empty_state(err.to_string(), color),
                        button(
                            text("Choose folder…")
                                .font(fonts::UI_SEMIBOLD)
                                .height(Fill)
                                .center(),
                        )
                        .on_press(Message::ChooseRoot)
                        .height(layouts::PRIMARY_BUTTON_HEIGHT)
                        .padding([0.0, 16.0])
                        .style(theme::primary_button(c)),
                    ]
                    .align_x(Center),
                )
                .center(Fill)
                .into()
            }
            Ok(workspace) => {
                let widths = self.state.panels.fitted(self.window_width);
                // The dividers' grab areas lie over the panels rather than between them.
                stack![
                    row![
                        container(self.sidebar(workspace))
                            .width(widths.sidebar)
                            .height(Fill),
                        rule::vertical(1).style(theme::divider(c)),
                        container(self.changes_panel())
                            .width(widths.changes)
                            .height(Fill)
                            .style(theme::background(c.panel)),
                        rule::vertical(1).style(theme::divider(c)),
                        container(self.diff_panel()).width(Fill).height(Fill),
                    ]
                    .height(Fill),
                    panels::handles(widths),
                ]
                .into()
            }
        };

        let window = column![
            self.top_bar(),
            rule::horizontal(1).style(theme::divider(c)),
            body
        ];

        // The toast floats over the window as a second layer, and while a divider is
        // dragged its overlay goes on top of everything. The window stays the first
        // layer either way, so its widgets keep their state (scroll offsets, the commit
        // box's cursor) when a toast or a drag comes or goes.
        let mut layers = stack![window].width(Fill).height(Fill);
        if let Some(toast) = self.error_toast() {
            layers = layers.push(toast);
        }
        if self.dragging.is_some() {
            layers = layers.push(panels::drag_overlay());
        }
        layers.into()
    }

    /// The latest error, on a card in the bottom-right corner like VS Code's
    /// notifications. It stays until its × is clicked or a newer error replaces it (see
    /// `report`). A long message, like a hook's output, scrolls inside the card.
    fn error_toast(&self) -> Option<Element<'_, Message>> {
        let error = self.error.as_ref()?;
        let c = &self.colors;

        let message = scrollable(
            container(text(error.to_string()).size(text_sizes::SMALL).width(Fill))
                // Room for the scrollbar, which is drawn over the content.
                .padding(Padding::ZERO.right(10.0)),
        )
        .direction(Direction::Vertical(theme::thin_scrollbar()))
        .style(theme::scrollbar(c))
        .width(Fill);

        let card = container(
            row![
                // 2px down: centers the icon on the message's first line.
                container(icon(icons::CIRCLE_ALERT, icon_sizes::BUTTON, c.danger))
                    .padding(Padding::ZERO.top(2.0)),
                message,
                changes::icon_button(c, icons::X, Message::DismissError, false),
            ]
            .spacing(layouts::GAP),
        )
        .padding([10.0, 12.0])
        .width(layouts::TOAST_WIDTH)
        .max_height(layouts::TOAST_MAX_HEIGHT)
        .style(theme::toast(c));

        // `opaque`: in a stack, the mouse passes through to the layer below wherever the
        // top layer has nothing interactive, so clicks on the message's text would land
        // on whatever row is under the card. Outside the card they still do, on purpose.
        Some(
            container(opaque(card))
                .align_right(Fill)
                .align_bottom(Fill)
                .padding(layouts::PANEL_PADDING_X)
                .into(),
        )
    }

    fn top_bar(&self) -> Element<'_, Message> {
        let c = &self.colors;
        let bar_button = |icon_bytes: &'static [u8], label: &'static str, message: Message| {
            button(
                row![icon(icon_bytes, icon_sizes::BUTTON, c.text), text(label)]
                    .spacing(6)
                    .height(Fill)
                    .align_y(Center),
            )
            .on_press(message)
            .height(layouts::BUTTON_HEIGHT)
            .padding([0.0, 12.0])
            .style(theme::outline_button(c))
        };

        container(
            row![
                text("Git Reviewer")
                    .font(fonts::UI_SEMIBOLD)
                    .size(text_sizes::TITLE),
                // The root path doubles as the button to change it.
                button(
                    text(
                        self.root
                            .as_ref()
                            .map(|root| root.display().to_string())
                            .unwrap_or_else(|| "Choose folder…".to_string())
                    )
                    // Same size as the title: in a centered row, text of different sizes
                    // sits at different baselines (~0.8px apart for 16 vs 14 in Segoe UI).
                    .size(text_sizes::TITLE)
                    .color(c.text_muted),
                )
                .on_press(Message::ChooseRoot)
                .padding([4.0, 8.0])
                .style(theme::list_row(c, false)),
                space().width(Fill),
                bar_button(icons::DOWNLOAD, "Fetch all", Message::FetchAll),
                bar_button(icons::ROTATE_CW, "Refresh", Message::Refresh),
            ]
            .spacing(12)
            .align_y(Center),
        )
        .height(layouts::TOP_BAR_HEIGHT)
        .padding([0.0, layouts::PANEL_PADDING_X])
        .align_y(Center)
        .style(theme::background(c.top_bar))
        .into()
    }

    /// Reads the selected worktree's status, and how far it is from its base, on a
    /// thread of its own (see `on_thread`). It comes back as `Message::StatusLoaded`,
    /// which then reloads the selected file's diff (see `retarget_selected_file`).
    fn load_status(&mut self) -> Task<Message> {
        self.status_request += 1;
        let Some(worktree) = self.selected_worktree().cloned() else {
            self.status = None;
            self.base_counts = None;
            self.base_error = None;
            return Task::none();
        };

        // A base set in the config wins over the repo's origin/HEAD.
        let configured_base = self
            .selected_repo()
            .and_then(|repo| self.config.base_for(repo.name()))
            .map(String::from);
        let repo = self.selected_repo().cloned();
        let request = self.status_request;
        on_thread(move || {
            let base_is_configured = configured_base.is_some();
            let base = configured_base.or_else(|| repo.and_then(|repo| repo.default_base()));
            LoadedStatus {
                status: worktree.status(),
                base: base.map(|base| {
                    let counts = worktree.ahead_behind(&base);
                    (base, counts)
                }),
                base_is_configured,
            }
        })
        .map(move |loaded| Message::StatusLoaded(request, loaded))
    }

    /// Shows a status `load_status` read.
    fn apply_status(&mut self, loaded: LoadedStatus) {
        self.status = Some(loaded.status);

        // Keep the selected row's sidebar badges in sync with what was just loaded,
        // so stage/unstage/commit update the pill without a full rescan.
        if let (Some(status), Some(path), Ok(workspace)) = (
            &self.status,
            &self.selected_worktree_path,
            &mut self.workspace,
        ) && let Some(worktree) = workspace.find_worktree_mut(path)
        {
            worktree.set_cached_status(status.as_ref().ok().cloned());
        }

        self.base_error = None;
        self.base_counts = match loaded.base {
            Some((base, Ok(counts))) => Some((base, counts)),
            Some((base, Err(_))) => {
                // A configured base that doesn't resolve is a typo in the config;
                // say so instead of quietly dropping the "from base" line.
                if loaded.base_is_configured {
                    self.base_error = Some(format!("Base `{base}` from the config not found"));
                }
                None
            }
            None => None,
        };
    }

    /// Runs a git action that changes the selected worktree (stage, unstage...) in
    /// order on the git thread (see `in_order`). `Message::ActionDone` follows.
    fn act(
        &self,
        action: impl FnOnce(&Worktree) -> Result<(), GitError> + Send + 'static,
    ) -> Task<Message> {
        let Some(worktree) = self.selected_worktree().cloned() else {
            return Task::none();
        };
        in_order(move || action(&worktree)).map(|result| Message::ActionDone(or_crashed(result)))
    }

    /// After anything that changes the index (stage, unstage, commit, refresh), keeps the
    /// selected file on the side it was showing if that side still exists, otherwise
    /// moves it to its other side, or clears the selection once the file has no changes.
    /// Deriving this from the new status (instead of from the action) means a failed git
    /// command leaves the selection alone. The `Task` reloads the diff.
    fn retarget_selected_file(&mut self) -> Task<Message> {
        // No readable status (worktree gone, git failing): nothing to show a diff of.
        let Some(Ok(status)) = &self.status else {
            self.selected_file = None;
            self.diff = None;
            return Task::none();
        };
        let Some((path, source)) = &self.selected_file else {
            return Task::none();
        };

        let entry = status.files.iter().find(|file| file.path == *path);
        let staged = entry
            .and_then(|file| file.staged)
            .map(|_| DiffSource::Staged);
        let unstaged = entry.and_then(|file| file.unstaged).map(|kind| match kind {
            ChangeKind::Untracked => DiffSource::Untracked,
            _ => DiffSource::Unstaged,
        });
        let new_source = match source {
            DiffSource::Staged => staged.or(unstaged),
            DiffSource::Unstaged | DiffSource::Untracked => unstaged.or(staged),
        };

        match new_source {
            Some(new_source) => {
                self.selected_file = Some((path.clone(), new_source));
                self.load_diff()
            }
            None => {
                self.selected_file = None;
                self.diff = None;
                Task::none()
            }
        }
    }

    /// Reads the selected file's diff again, on a thread of its own (see `on_thread`).
    /// It comes back as `Message::DiffLoaded` (see `show_diff`); its colors come after
    /// that, from `update`.
    fn load_diff(&mut self) -> Task<Message> {
        self.diff_request += 1;
        let (Some(worktree), Some(file)) = (
            self.selected_worktree().cloned(),
            self.selected_file.clone(),
        ) else {
            return Task::none();
        };

        // The spinner's clock starts for a file (or side) that isn't on screen yet. A
        // reload of the one shown, even one still loading, keeps it running.
        if !matches!(&self.diff, Some(Ok(view)) if view.is_of(&file.0, file.1)) {
            self.diff_requested_at = Instant::now();
        }

        let request = self.diff_request;
        let (path, source) = file.clone();
        on_thread(move || worktree.diff(&path, source))
            .map(move |diff| Message::DiffLoaded(request, file.clone(), diff))
    }

    /// Shows a diff `load_diff` read, of `source`'s side of the file at `path`.
    fn show_diff(&mut self, path: &str, source: DiffSource, diff: Result<Diff, GitError>) {
        match diff {
            // Nothing changed (most refreshes): keep what's on screen, colors and all.
            Ok(diff) if matches!(&self.diff, Some(Ok(view)) if view.shows(path, source, &diff)) => {
            }
            Ok(diff) => {
                let new = DiffView::new(path, source, diff);
                match &mut self.diff {
                    // The file on screen changed: it stays until the new version is
                    // ready (see `DiffView::replace_when_highlighted`).
                    Some(Ok(view))
                        if view.is_of(path, source) && !view.is_loading() && new.is_loading() =>
                    {
                        view.replace_when_highlighted(new);
                    }
                    _ => {
                        // The scrollable (and where it's scrolled to) only carries over
                        // if both diffs are shown. Otherwise one isn't there for a while
                        // (an error, or a loading diff, sits in its place), and the new
                        // one starts at the top.
                        let shown = matches!(&self.diff, Some(Ok(view)) if !view.is_loading());
                        let keeps_scrollable = shown && !new.is_loading();
                        if !keeps_scrollable {
                            self.diff_scroll_y = 0.0;
                        }
                        self.diff = Some(Ok(new));
                    }
                }
            }
            Err(err) => self.diff = Some(Err(err)),
        }
    }

    /// How long the selected file's code has been loading (read, then highlighted),
    /// counted from when it was asked for; `None` while it's shown, or there's none.
    fn diff_loading_for(&self) -> Option<Duration> {
        let loading = match (&self.selected_file, &self.diff) {
            (Some(_), None) => true,
            (_, Some(Ok(diff))) => diff.is_loading(),
            _ => false,
        };
        loading.then(|| self.diff_requested_at.elapsed())
    }

    fn is_open(&self, section: &Section) -> bool {
        !self.collapsed.contains(section)
    }

    fn selected_worktree(&self) -> Option<&Worktree> {
        match (&self.selected_worktree_path, &self.workspace) {
            (Some(selected), Ok(workspace)) => workspace.find_worktree(selected),
            _ => None,
        }
    }

    fn selected_repo(&self) -> Option<&Repo> {
        match (&self.selected_worktree_path, &self.workspace) {
            (Some(selected), Ok(workspace)) => workspace.find_repo(selected),
            _ => None,
        }
    }
}

/// Closes every confirmation dialog's text.
const WARNING: &str = "This can't be undone.";

/// The removal confirmation, saying what will be lost.
fn removal_question(plan: &RemovalPlan) -> String {
    let action = match plan.removal {
        Removal::Worktree => format!("Remove the worktree {}?", plan.name),
        Removal::Branch => format!("Remove the worktree and delete the branch {}?", plan.name),
    };
    let changes = match plan.changes {
        1 => "1 uncommitted change".to_string(),
        count => format!("{count} uncommitted changes"),
    };
    format!("{action} Its {changes} will be lost.\n\n{WARNING}")
}

/// The discard confirmation, saying exactly what the plan will lose.
fn discard_question(plan: &DiscardPlan) -> String {
    match plan.files.as_slice() {
        [(path, true)] => format!("Delete the untracked file {path}?\n\n{WARNING}"),
        [(path, false)] => format!("Discard the changes in {path}?\n\n{WARNING}"),
        files => {
            let untracked = files.iter().filter(|(_, untracked)| *untracked).count();
            let count = files.len();
            match untracked {
                0 => format!("Discard the changes in {count} files?\n\n{WARNING}"),
                _ => format!(
                    "Discard the changes in {count} files? {untracked} of them are \
                     untracked and will be deleted.\n\n{WARNING}"
                ),
            }
        }
    }
}

/// What the error toast (and the empty window, when there's no workspace) shows.
#[derive(Debug, Clone)]
pub enum AppError {
    /// A failed git command, shown with git's own message.
    Git(GitError),
    /// The app's own problems (config, file watcher, root folder), shown as written.
    Message(String),
}

impl From<GitError> for AppError {
    fn from(err: GitError) -> Self {
        AppError::Git(err)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Git(err) => err.fmt(f),
            AppError::Message(message) => f.write_str(message),
        }
    }
}

fn config_error(err: ConfigError) -> AppError {
    AppError::Message(format!("Config: {err}"))
}

/// The workspace under `root`, or why there isn't one. The `archived` repos (by folder)
/// are opened without their statuses.
fn scan_root(root: Option<&Path>, archived: &BTreeSet<PathBuf>) -> Result<Workspace, AppError> {
    match root {
        // A relative root (hand-written in the config) would depend on where the app
        // was launched from, which for a macOS app opened from Finder is `/`.
        Some(root) if root.is_relative() => Err(AppError::Message(format!(
            "Config: root `{}` must be an absolute path",
            root.display()
        ))),
        Some(root) => Workspace::scan(root, |repo| archived.contains(repo)).map_err(AppError::from),
        None => Err(AppError::Message(
            "No folder to show yet. Choose the folder that holds your repos.".to_string(),
        )),
    }
}

/// A centered, muted message for panels with nothing to show (shared by the panels).
fn empty_state<'a>(message: impl Into<String>, color: Color) -> Container<'a, Message> {
    container(text(message.into()).color(color).center()).padding([24.0, 8.0])
}

/// A task that runs `work` on a thread of its own and produces its result, or `None` if
/// the thread panicked (its panic message is printed).
///
/// Blocking work (git, highlighting) can't just go in a `Task::perform` `async` block:
/// iced polls a new task once on the UI thread, before its thread pool sees it
/// (iced_winit's `update`), so work that never waits would run right there and freeze
/// the app. Waiting on the channel makes that first poll return at once. (iced's own
/// `task::blocking` does the same, but iced 0.14 doesn't make it public.)
fn on_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Task<Option<T>> {
    let (sender, receiver) = oneshot::channel();
    thread::spawn(move || {
        // An error only means nothing waits for the result any more.
        let _ = sender.send(work());
    });
    Task::perform(receiver, Result::ok)
}

/// A task that runs `work` on the app's git thread, after everything given to it
/// before, and produces its result, or `None` if `work` panicked (its panic message is
/// printed; the thread carries on with the next job).
///
/// For git actions that change a repo (stage, commit, discard, pull...). Two of those at
/// once would clash on git's `index.lock`, the second failing; one at a time, they also
/// happen in the order they were clicked. (Reads go through `on_thread` instead: they
/// take no lock, see `GIT_OPTIONAL_LOCKS` in `git::Git::run`.)
fn in_order<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Task<Option<T>> {
    type Job = Box<dyn FnOnce() + Send>;
    static GIT_THREAD: LazyLock<mpsc::Sender<Job>> = LazyLock::new(|| {
        let (sender, jobs) = mpsc::channel::<Job>();
        thread::spawn(move || {
            for job in jobs {
                // A panicking job drops its result's sender, which says so (`None`).
                let _ = panic::catch_unwind(AssertUnwindSafe(job));
            }
        });
        sender
    });

    let (sender, receiver) = oneshot::channel();
    let job: Job = Box::new(move || {
        // An error only means nothing waits for the result any more.
        let _ = sender.send(work());
    });
    // Fails only if the git thread is gone, which `catch_unwind` prevents; the job's
    // dropped sender would then say `None`, like a panic.
    let _ = GIT_THREAD.send(job);
    Task::perform(receiver, Result::ok)
}

/// The result of git work done on another thread (see `on_thread` and `in_order`):
/// `None`, from a thread that panicked, becomes an error saying so.
fn or_crashed<T>(result: Option<Result<T, GitError>>) -> Result<T, GitError> {
    result.unwrap_or_else(|| Err(thread_crashed()))
}

/// What git work on another thread reports if that thread panicked.
fn thread_crashed() -> GitError {
    GitError::CommandFailed("it stopped unexpectedly (an internal error)".to_string())
}
