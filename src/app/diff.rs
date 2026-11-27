//! The right panel: the selected file's diff.

use std::borrow::Cow;
use std::f32::consts::TAU;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use iced::advanced::graphics::text::Paragraph;
use iced::advanced::text::{self as core_text, Paragraph as _};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::operation::{self, AbsoluteOffset};
use iced::widget::scrollable::Direction;
use iced::widget::text::{IntoFragment, LineHeight, Shaping, Span, Wrapping};
use iced::widget::{
    Id, Text, column, container, responsive, rich_text, row, rule, scrollable, sensor, space, span,
    stack, text,
};
use iced::{
    Alignment::Center,
    Color, Element, Fill, Padding, Pixels, Radians, Rotation, Size, Subscription, Task,
    alignment::{Horizontal::Right, Vertical},
};
use iced_palace::widget::ellipsized_text;

use super::changes::sized_icon_button;
use super::sticky::sticky;
use super::{App, Message, empty_state, on_thread};
use crate::git::{Diff, DiffLine, DiffSource};
use crate::highlight::{self, Flavor, LineColors};
use crate::theme::icons::{self, icon};
use crate::theme::{self, Colors, fonts, icon_sizes, layouts, text_sizes};

// Diff geometry, from the mockup: two 44px line-number columns padded 8 and 10 on the
// right, a 16px sign column, 24px after the code.
const OLD_NUMBER_PADDING: f32 = 8.0;
const NEW_NUMBER_PADDING: f32 = 10.0;
const GUTTER_WIDTH: f32 = layouts::LINE_NUMBER_WIDTH * 2.0
    + OLD_NUMBER_PADDING
    + NEW_NUMBER_PADDING
    + layouts::DIFF_SIGN_WIDTH;
const CODE_PADDING_RIGHT: f32 = 24.0;
/// Room under the last line, so it doesn't sit on the panel's edge.
const CODE_PADDING_BOTTOM: f32 = 8.0;

/// How many diff lines go in one `Chunk`.
const CHUNK_LINES: usize = 64;

/// How many lines show above a change the header's arrows jump to (see
/// `DiffView::change_target`): a hunk's usual context.
const CHANGE_CONTEXT: usize = 3;

/// The diff's scrollable, for the arrows' `scroll_to` (see `App::jump_to_change`).
const SCROLLABLE: Id = Id::new("diff");

/// Which way the header's arrows jump between changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChangeStep {
    Previous,
    Next,
}

/// The selected file's diff, laid out the way `diff_body` draws it. `load_diff` builds
/// it once per diff, so `view`, which runs after every message, does no per-line work.
///
/// The text is cut into chunks of `CHUNK_LINES` lines, and only the chunks around what's
/// visible are put on screen (see `ChunkWindow`). Shaping text is what costs (around
/// 60ms per 1000 lines in a release build, measured), and it happens for every text
/// widget that's laid out, visible or not: shaping a whole 5000-line file took over
/// 0.3s, and much longer in a debug build. Behind the text, the tints are one block per
/// run of same-kind lines for the whole file: blocks without text, so even thousands
/// are cheap. Every line is `DIFF_LINE_HEIGHT` tall, so it all lines up.
pub(super) struct DiffView {
    /// Which file and side this is, and its diff as git gave it: kept to tell whether a
    /// reload changed anything, and to work out the syntax colors.
    path: String,
    source: DiffSource,
    diff: Arc<Diff>,
    highlighting: Highlighting,
    /// A newer version of this same diff, shown in its place once it's highlighted (see
    /// `replace_when_highlighted`).
    next: Option<Box<DiffView>>,
    added: usize,
    removed: usize,
    chunks: Vec<Chunk>,
    runs: Vec<Run>,
    /// The first line of each change, in order. A change is a run of added and removed
    /// lines with no unchanged line in it, so an edited line (removed, then added) is
    /// one change, as in VS Code. Hunks wouldn't do: the whole file is one.
    changes: Vec<usize>,
    /// How wide the rows must be to fit the longest line, gutter and padding included.
    width: f32,
    /// A binary file: a message is shown instead of lines (it has none).
    binary: bool,
}

/// `CHUNK_LINES` consecutive diff lines (the last chunk may have fewer), one text per
/// column, with one line per diff line. One text per column rather than a row of
/// widgets per line keeps the widget count low.
struct Chunk {
    old_numbers: String,
    new_numbers: String,
    /// The sign column is two texts drawn over each other, since a text has one color:
    /// green `+` on added lines and red `−` on removed ones, each blank elsewhere.
    plus_signs: String,
    minus_signs: String,
    /// The code, in its syntax colors.
    code: ColoredText,
}

impl Chunk {
    /// `colors` has the lines' syntax colors (see `highlight::diff_colors`), or is
    /// `None` to draw them all in the default text color.
    fn new(lines: &[DiffLine], colors: Option<&[LineColors]>) -> Chunk {
        let column =
            |cell: fn(&DiffLine) -> String| lines.iter().map(cell).collect::<Vec<_>>().join("\n");
        fn number(number: Option<usize>) -> String {
            number.map(|n| n.to_string()).unwrap_or_default()
        }

        let mut code = ColoredText::default();
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                code.push_newline();
            }
            let line_colors = colors.map_or(&[][..], |colors| colors[index].as_slice());
            code.push_line(line.text(), line_colors);
        }

        Chunk {
            old_numbers: column(|line| number(line.old_number())),
            new_numbers: column(|line| number(line.new_number())),
            plus_signs: column(|line| match line {
                DiffLine::Added { .. } => "+".to_string(),
                _ => String::new(),
            }),
            minus_signs: column(|line| match line {
                DiffLine::Removed { .. } => "−".to_string(),
                _ => String::new(),
            }),
            code,
        }
    }
}

/// Text built piece by piece, each piece in its own color, for a `rich_text`.
#[derive(Default)]
struct ColoredText {
    text: String,
    /// Byte ranges of `text`, in order and covering all of it, each with its color
    /// (`None`: the default text color).
    colors: Vec<(Range<usize>, Option<Color>)>,
}

impl ColoredText {
    /// Adds a diff line as it's drawn (see `display_text`), in its syntax colors (see
    /// `LineColors`), or in the default color if it has none.
    fn push_line(&mut self, line: &str, colors: &[(Range<usize>, Color)]) {
        let mut after_punctuation = false;
        if colors.is_empty() {
            self.push(line, None, &mut after_punctuation);
        }
        for (range, color) in colors {
            self.push(&line[range.clone()], Some(*color), &mut after_punctuation);
        }
    }

    /// Ends a line. The `\n` takes the color of the piece before it, since it isn't
    /// drawn: a piece of its own would just be one more span.
    fn push_newline(&mut self) {
        self.text.push('\n');
        match self.colors.last_mut() {
            Some((range, _)) => range.end = self.text.len(),
            None => self.colors.push((0..self.text.len(), None)),
        }
    }

    /// Adds `piece`, part of a line, in `color`. A neighbour of the same color merges
    /// with it, so there are fewer spans to draw.
    fn push(&mut self, piece: &str, color: Option<Color>, after_punctuation: &mut bool) {
        let start = self.text.len();
        push_display_text(&mut self.text, piece, after_punctuation);
        let end = self.text.len();
        match self.colors.last_mut() {
            Some((range, last)) if *last == color => range.end = end,
            _ if start < end => self.colors.push((start..end, color)),
            _ => {}
        }
    }
}

/// Consecutive diff lines of the same kind.
struct Run {
    kind: Kind,
    lines: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Context,
    Added,
    Removed,
}

impl Kind {
    fn of(line: &DiffLine) -> Kind {
        match line {
            DiffLine::Context { .. } => Kind::Context,
            DiffLine::Added { .. } => Kind::Added,
            DiffLine::Removed { .. } => Kind::Removed,
        }
    }
}

/// Where a `DiffView`'s syntax colors are at. Until they're done, its code isn't shown
/// (`diff_panel` shows `loading` instead), the way VS Code doesn't show a file before
/// it's colored.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Highlighting {
    NotStarted,
    Running,
    /// Colored, or left in the default text color: not a language we highlight, or the
    /// highlighter failed.
    Done,
}

impl DiffView {
    /// The diff of `source`'s side of the file at `path`. Unless it's in a language we
    /// don't highlight, it's loading until `start_highlighting`'s colors come in.
    pub(super) fn new(path: &str, source: DiffSource, diff: Diff) -> DiffView {
        let (added, removed) = diff.counts();

        let mut runs: Vec<Run> = Vec::new();
        for kind in diff.lines.iter().map(Kind::of) {
            match runs.last_mut() {
                Some(run) if run.kind == kind => run.lines += 1,
                _ => runs.push(Run { kind, lines: 1 }),
            }
        }

        // A binary file has no lines to color.
        let highlighting = if !diff.binary && highlight::highlights(path, &diff) {
            Highlighting::NotStarted
        } else {
            Highlighting::Done
        };

        DiffView {
            path: path.to_string(),
            source,
            added,
            removed,
            chunks: chunks(&diff, None),
            changes: change_starts(&runs),
            runs,
            width: content_width(&diff),
            binary: diff.binary,
            diff: Arc::new(diff),
            highlighting,
            next: None,
        }
    }

    /// Whether this is a diff of `source`'s side of the file at `path`.
    pub(super) fn is_of(&self, path: &str, source: DiffSource) -> bool {
        self.path == path && self.source == source
    }

    /// Whether this (or the newer version waiting to replace it) already is `diff` of
    /// that file and side. A reload that changed nothing (most refreshes) then keeps it,
    /// colors and all.
    pub(super) fn shows(&self, path: &str, source: DiffSource, diff: &Diff) -> bool {
        let newest = self.next.as_deref().unwrap_or(self);
        newest.is_of(path, source) && *newest.diff == *diff
    }

    /// Whether the code is still loading (see `Highlighting`), so not shown yet.
    pub(super) fn is_loading(&self) -> bool {
        self.highlighting != Highlighting::Done
    }

    /// (added, removed) line counts, for the header's +N −N. `None` for a binary file:
    /// it has no lines to count, and +0 −0 would read as "nothing changed".
    pub(super) fn counts(&self) -> Option<(usize, usize)> {
        (!self.binary).then_some((self.added, self.removed))
    }

    /// Whether it's a binary file, which has no lines to show.
    pub(super) fn is_binary(&self) -> bool {
        self.binary
    }

    /// How far down the diff scrolls in a viewport `viewport_height` tall: as far as
    /// iced lets it, its whole height (the lines and the room under them) less the
    /// viewport's, or 0 if it fits.
    fn max_scroll_y(&self, viewport_height: f32) -> f32 {
        let height = lines_height(self.diff.lines.len()) + CODE_PADDING_BOTTOM;
        (height - viewport_height).max(0.0)
    }

    /// Where to scroll to for the previous or next change from `scroll_y`, in a viewport
    /// `viewport_height` tall: with the change `CHANGE_CONTEXT` lines from the top, or
    /// scrolled to the bottom for one too near the end for that. `None` when there's no
    /// change that way, or the bottom is reached already (every change left is then on
    /// screen): its arrow is disabled.
    fn change_target(&self, step: ChangeStep, scroll_y: f32, viewport_height: f32) -> Option<f32> {
        // Offsets this close count as the same: the change just jumped to (fractional
        // scroll offsets) isn't the next one again.
        const SLACK: f32 = 0.5;
        let max = self.max_scroll_y(viewport_height);
        // A stale offset (the file got shorter) is wherever iced clamped it to.
        let scroll_y = scroll_y.min(max);
        let mut targets = self
            .changes
            .iter()
            .map(|&line| lines_height(line.saturating_sub(CHANGE_CONTEXT)).min(max));
        match step {
            ChangeStep::Previous => targets.rev().find(|&y| y < scroll_y - SLACK),
            ChangeStep::Next => targets.find(|&y| y > scroll_y + SLACK),
        }
    }

    /// Keeps this on screen until `next`, a newer version of the same diff, is
    /// highlighted, then shows `next` in its place. A refresh of the file being read
    /// (it was saved, say) then never takes its code off the screen, nor its scroll
    /// position, which a loading spinner would reset.
    pub(super) fn replace_when_highlighted(&mut self, next: DiffView) {
        self.next = Some(Box::new(next));
    }

    /// Colors the code again, after the theme went between dark and light: a fresh copy
    /// of the newest version waits to replace this one (see `replace_when_highlighted`),
    /// so the code stays on screen, in its old colors, until the new ones are in.
    pub(super) fn highlight_again(&mut self) {
        let newest = self.next.as_deref().unwrap_or(self);
        let fresh = DiffView::new(&newest.path, newest.source, (*newest.diff).clone());
        // Not a language we highlight: nothing would ever replace this one.
        if fresh.is_loading() {
            self.replace_when_highlighted(fresh);
        }
    }

    /// The task that works out the code's syntax colors in `flavor`'s theme, on a thread
    /// of its own (see `on_thread`), since that can take a while (up to 0.7 s for both
    /// sides of a 5000-line file). `None` if there's nothing to start: it's already been
    /// started, or it's not a language we highlight. The result comes back as
    /// `Message::DiffHighlighted`.
    pub(super) fn start_highlighting(&mut self, flavor: Flavor) -> Option<Task<Message>> {
        if let Some(next) = &mut self.next {
            return next.start_highlighting(flavor);
        }
        if self.highlighting != Highlighting::NotStarted {
            return None;
        }
        self.highlighting = Highlighting::Running;

        let (path, diff) = (self.path.clone(), Arc::clone(&self.diff));
        let work = move || highlight::diff_colors(&path, &diff, flavor);
        let diff = Arc::clone(&self.diff);
        // A panicked thread leaves the code in the default text color.
        let done =
            move |colors: Option<_>| Message::DiffHighlighted(Arc::clone(&diff), colors.flatten());
        Some(on_thread(work).map(done))
    }

    /// Colors the code with what `start_highlighting` worked out for `diff` (`None`:
    /// no colors, so the default text color) and shows it. Ignored if `diff` is
    /// neither this diff nor its newer version: another file was opened, or this one
    /// changed again, while it ran.
    pub(super) fn set_colors(&mut self, diff: &Arc<Diff>, colors: Option<&[LineColors]>) {
        if Arc::ptr_eq(&self.diff, diff) {
            if let Some(colors) = colors {
                self.chunks = chunks(&self.diff, Some(colors));
            }
            self.highlighting = Highlighting::Done;
        } else if let Some(mut next) = self.next.take() {
            next.set_colors(diff, colors);
            if next.is_loading() {
                self.next = Some(next);
            } else {
                *self = *next;
            }
        }
    }
}

/// `diff`'s lines in `Chunk`s. `colors` has their syntax colors (see
/// `highlight::diff_colors`), or is `None` to draw the code in the default text color.
fn chunks(diff: &Diff, colors: Option<&[LineColors]>) -> Vec<Chunk> {
    let mut color_chunks = colors.map(|colors| colors.chunks(CHUNK_LINES));
    diff.lines
        .chunks(CHUNK_LINES)
        .map(|lines| Chunk::new(lines, color_chunks.as_mut().and_then(Iterator::next)))
        .collect()
}

/// Where each change starts (see `DiffView::changes`): the first line of every run of
/// added or removed lines that comes after unchanged ones, or first.
fn change_starts(runs: &[Run]) -> Vec<usize> {
    let mut starts = Vec::new();
    let (mut line, mut in_change) = (0, false);
    for run in runs {
        let changed = run.kind != Kind::Context;
        if changed && !in_change {
            starts.push(line);
        }
        in_change = changed;
        line += run.lines;
    }
    starts
}

impl App {
    /// A header bar (path, which side, +N −N) over the scrollable diff. While the diff
    /// loads (it's read, then highlighted), the header is there and the body waits.
    pub(super) fn diff_panel(&self) -> Element<'_, Message> {
        let c = &self.colors;
        let Some((path, source)) = &self.selected_file else {
            let clean = matches!(&self.status, Some(Ok(status)) if status.files.is_empty());
            return match (&self.status, clean) {
                (None, _) => space().into(),
                (Some(_), true) => {
                    empty_state("Nothing to review. Everything is committed.", c.text_muted)
                        .center(Fill)
                        .into()
                }
                (Some(_), false) => empty_state("Select a file to see its changes", c.text_muted)
                    .center(Fill)
                    .into(),
            };
        };

        // Only for a diff that's on screen, with changes to jump between.
        let nav = self
            .shown_diff()
            .filter(|diff| !diff.changes.is_empty())
            .map(|diff| {
                let can_jump = |step| {
                    diff.change_target(step, self.diff_scroll_y, self.diff_viewport_height)
                        .is_some()
                };
                ChangeNav {
                    previous: can_jump(ChangeStep::Previous),
                    next: can_jump(ChangeStep::Next),
                }
            });

        let (counts, body) = match (&self.diff, self.diff_loading_for()) {
            (Some(Err(err)), _) => {
                return empty_state(err.to_string(), c.danger).center(Fill).into();
            }
            // A binary file has no lines to show (VS Code says so too).
            (Some(Ok(diff)), None) if diff.is_binary() => (
                None,
                empty_state("Binary file not shown", c.text_muted)
                    .center(Fill)
                    .into(),
            ),
            (Some(Ok(diff)), None) => (diff.counts(), diff_body(c, diff, self.diff_scroll_y)),
            // Highlighting: the counts are known. Still being read: they're not.
            (Some(Ok(diff)), Some(elapsed)) => (diff.counts(), loading(c, elapsed)),
            (None, elapsed) => (None, loading(c, elapsed.unwrap_or_default())),
        };

        column![
            diff_header(c, path, *source, counts, nav),
            rule::horizontal(1).style(theme::divider(c)),
            body,
        ]
        .into()
    }

    /// The selected file's diff while its lines are on screen: read, highlighted, and
    /// not a binary file.
    fn shown_diff(&self) -> Option<&DiffView> {
        match &self.diff {
            Some(Ok(diff)) if !diff.is_loading() && !diff.is_binary() => Some(diff),
            _ => None,
        }
    }

    /// Scrolls the diff to the previous or next change (see `DiffView::change_target`),
    /// if there's one that way.
    pub(super) fn jump_to_change(&mut self, step: ChangeStep) -> Task<Message> {
        let Some(y) = self.shown_diff().and_then(|diff| {
            diff.change_target(step, self.diff_scroll_y, self.diff_viewport_height)
        }) else {
            return Task::none();
        };
        // Set here rather than left to `Message::DiffScrolled`: the scrollable only
        // reports its new offset when it next draws, so the lines around the target
        // wouldn't be built for that frame. iced builds the view after `update`, then
        // runs the scroll on it, then draws (checked in iced_winit's event loop).
        self.diff_scroll_y = y;
        operation::scroll_to(
            SCROLLABLE,
            AbsoluteOffset {
                x: None,
                y: Some(y),
            },
        )
    }
}

/// The change arrows' keyboard shortcuts, VS Code's: Alt+F5 jumps to the next change,
/// Shift+Alt+F5 to the previous one (Option on a Mac), and held down they repeat. Only
/// keys no widget used come through (see `keyboard::listen`): the commit box, even
/// focused, leaves these alone (its key bindings have nothing for F5).
pub(super) fn change_shortcuts() -> Subscription<Message> {
    keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed {
            key: Key::Named(Named::F5),
            modifiers,
            ..
        } if modifiers.alt() && !modifiers.control() && !modifiers.logo() => {
            let step = if modifiers.shift() {
                ChangeStep::Previous
            } else {
                ChangeStep::Next
            };
            Some(Message::JumpToChange(step))
        }
        _ => None,
    })
}

/// Which of the header's previous / next change arrows can be clicked.
struct ChangeNav {
    previous: bool,
    next: bool,
}

/// How wide the diff's rows must be to fit its longest line, gutter and padding
/// included. Worked out once per diff (in `DiffView::new`), since lines that aren't
/// plain ASCII are measured, which means shaping their text.
fn content_width(diff: &Diff) -> f32 {
    let code = diff
        .lines
        .iter()
        .map(|line| code_width(line.text()))
        .fold(0.0, f32::max);
    GUTTER_WIDTH + code + CODE_PADDING_RIGHT
}

/// How wide a line's text is drawn. Plain ASCII, nearly every line, is all Fira Code,
/// where every glyph has the same advance, so it's just counted. Anything else may be
/// drawn from a fallback font (CJK characters and emoji are wider than Fira Code's
/// glyphs), so it's measured.
fn code_width(line: &str) -> f32 {
    let expanded = expand_tabs(line);
    if expanded.is_ascii() {
        expanded.len() as f32 * layouts::DIFF_CHAR_WIDTH
    } else {
        measure(&display_text(line))
    }
}

/// Measures drawn diff text with iced's own text engine, set up exactly like `mono`,
/// so fallback fonts and ligatures count the same as on screen.
fn measure(shown: &str) -> f32 {
    Paragraph::with_text(core_text::Text {
        content: shown,
        bounds: Size::INFINITE,
        size: Pixels(text_sizes::SMALL),
        line_height: LineHeight::Absolute(Pixels(layouts::DIFF_LINE_HEIGHT)),
        font: fonts::MONO,
        align_x: core_text::Alignment::Left,
        align_y: Vertical::Top,
        shaping: Shaping::Advanced,
        wrapping: Wrapping::None,
    })
    .min_width()
}

/// How long the diff's body stays empty before its loading spinner shows, so a file
/// that's quick to highlight (most are, in well under this) doesn't flash one.
const SPINNER_DELAY: Duration = Duration::from_millis(150);

/// The diff's body while its code is loading (read, then highlighted: see
/// `App::diff_loading_for`), `elapsed` in: empty at first (see `SPINNER_DELAY`), then a
/// spinner turning once a second. `App::subscription` redraws it every frame meanwhile.
fn loading<'a>(c: &Colors, elapsed: Duration) -> Element<'a, Message> {
    if elapsed < SPINNER_DELAY {
        return space().width(Fill).height(Fill).into();
    }

    let angle = elapsed.as_secs_f32() * TAU;
    container(
        icon(icons::SPINNER, icon_sizes::SPINNER, c.text_muted)
            .rotation(Rotation::Floating(Radians(angle))),
    )
    .center(Fill)
    .into()
}

/// `counts` is (added, removed), or `None` while the diff is still being read, and for
/// a binary file (see `DiffView::counts`). `nav` says which change arrows can be
/// clicked, or is `None` to leave them out.
fn diff_header<'a>(
    c: &Colors,
    path: &'a str,
    source: DiffSource,
    counts: Option<(usize, usize)>,
    nav: Option<ChangeNav>,
) -> Element<'a, Message> {
    // Tinted like the list's section it comes from: green for staged, yellow for changes.
    let (side, color, background) = match source {
        DiffSource::Staged => ("Staged", c.added, c.added_chip),
        DiffSource::Unstaged => ("Unstaged", c.modified, c.modified_chip),
        DiffSource::Untracked => ("Untracked", c.modified, c.modified_chip),
    };

    // Pushed piece by piece: a `None` child in `row!` would still take a gap.
    let mut content = row![
        ellipsized_text(path)
            .size(text_sizes::SMALL)
            .wrapping(Wrapping::None),
        container(text(side).size(text_sizes::LABEL))
            .padding([1.0, 6.0])
            .style(theme::filled_tag(color, background)),
        space().width(Fill),
    ]
    .spacing(10)
    .align_y(Center);
    if let Some((added, removed)) = counts {
        content = content
            .push(
                text(format!("+{added}"))
                    .size(text_sizes::SMALL)
                    .color(c.added),
            )
            .push(
                text(format!("−{removed}"))
                    .size(text_sizes::SMALL)
                    .color(c.deleted),
            );
    }
    if let Some(nav) = nav {
        // An arrow with nowhere to go has no `on_press`, so it's drawn disabled.
        let arrow = |bytes, step, enabled: bool| {
            sized_icon_button(
                c,
                bytes,
                enabled.then_some(Message::JumpToChange(step)),
                false,
                layouts::ICON_BUTTON,
                icon_sizes::ROW,
            )
        };
        content = content.push(
            row![
                arrow(icons::ARROW_UP, ChangeStep::Previous, nav.previous),
                arrow(icons::ARROW_DOWN, ChangeStep::Next, nav.next),
            ]
            .spacing(2),
        );
    }

    container(content)
        .height(layouts::DIFF_HEADER_HEIGHT)
        .padding([0.0, layouts::PANEL_PADDING_X])
        .align_y(Center)
        .into()
}

/// The whole file, scrollable both ways: the gutter (line numbers and signs) and the
/// code, each the visible chunks' texts over a column of tint blocks (see `DiffView`).
/// The gutter stays at the left edge when scrolled sideways. `scroll_y` is how far down
/// it's scrolled, which decides the chunks that are built.
///
/// Lines don't wrap, so the code is as wide as its longest line (`DiffView::width`), or
/// the panel if that's wider, so the tints run edge to edge. (Nothing can just be `Fill`:
/// inside a both-ways scrollable that lays out empty.)
fn diff_body<'a>(c: &'a Colors, diff: &'a DiffView, scroll_y: f32) -> Element<'a, Message> {
    responsive(move |size| {
        let tint_width = diff.width.max(size.width) - GUTTER_WIDTH;
        // A fixed width, so the text's bounds don't change when the panel is resized:
        // that would make iced lay out every line again.
        let code_width = diff.width - GUTTER_WIDTH;
        let window = ChunkWindow::new(scroll_y, size.height, diff.chunks.len());

        // In a `stack`, the first layer sets the size (here, the whole file's), and the
        // rest are drawn over it.
        let gutter = stack(
            std::iter::once(column(diff.runs.iter().map(|run| gutter_run(c, run))).into())
                .chain(window.layers(&diff.chunks, |chunk| gutter_chunk(c, chunk))),
        );
        let tints = diff.runs.iter().map(|run| code_tint(c, run, tint_width));
        let code = stack(
            std::iter::once(column(tints).into())
                .chain(window.layers(&diff.chunks, |chunk| code_text(&chunk.code, code_width))),
        );

        // The gutter goes over a gap left for it, as a `Sticky` stack layer, so it stays
        // put when the code is scrolled sideways.
        let content = stack![row![space().width(GUTTER_WIDTH), code], sticky(gutter)];

        let body =
            scrollable(container(content).padding(Padding::ZERO.bottom(CODE_PADDING_BOTTOM)))
                .id(SCROLLABLE)
                .direction(Direction::Both {
                    vertical: theme::thin_scrollbar(),
                    horizontal: theme::thin_scrollbar(),
                })
                .on_scroll(|viewport| Message::DiffScrolled(viewport.absolute_offset().y))
                .style(theme::scrollbar(c))
                .width(Fill)
                .height(Fill);

        // Reports the viewport's height, which with the diff's own tells how far down
        // it scrolls (see `DiffView::max_scroll_y`): when it's first drawn, and on every
        // resize. `on_scroll` can't tell: iced never calls it for a diff that fits.
        sensor(body)
            .on_resize(|size| Message::DiffResized(size.height))
            .into()
    })
    .into()
}

/// The chunks put on screen: the ones the visible area touches, plus one more on each
/// side as a margin.
///
/// Each chunk is a `stack` layer pushed down to its place, and chunk `i` always goes in
/// layer `i % slots`. Scrolling one chunk further then changes the content of one layer
/// only: the others keep theirs, so iced keeps their shaped text and shapes just the
/// new chunk. (In a `column`, every chunk would move up one place, and iced matches
/// widgets to their previous state by place, so all of them would be shaped again.
/// `keyed_column` doesn't help: it only follows a single insert or removal.)
///
/// iced handles the scroll message and rebuilds the view before drawing the frame,
/// so the chunks are in place on the same frame the scroll shows.
#[derive(Debug, PartialEq)]
struct ChunkWindow {
    chunks: Range<usize>,
    slots: usize,
}

impl ChunkWindow {
    /// The window for a viewport `height` tall, scrolled `scroll_y` down, over `count`
    /// chunks. It's kept within the file, so a stale `scroll_y` (the file got shorter
    /// and the scrollable clamped its offset) still covers the end, where it now is.
    fn new(scroll_y: f32, height: f32, count: usize) -> ChunkWindow {
        let chunk_height = lines_height(CHUNK_LINES);
        // A viewport touches at most `ceil(height / chunk_height) + 1` chunks, and
        // there's one more on each side. Never more slots than chunks (and at least
        // one), so an unbounded `height` can't overflow or allocate endless slots.
        let slots = ((height / chunk_height).ceil() as usize)
            .saturating_add(3)
            .min(count.max(1));
        let first = ((scroll_y / chunk_height) as usize)
            .saturating_sub(1)
            .min(count.saturating_sub(slots));
        ChunkWindow {
            chunks: first..(first + slots).min(count),
            slots,
        }
    }

    /// One stack layer per slot: the chunk `draw` makes, moved down to its place, or
    /// nothing if no chunk in the window uses that slot.
    fn layers<'a>(
        &self,
        chunks: &'a [Chunk],
        draw: impl Fn(&'a Chunk) -> Element<'a, Message>,
    ) -> Vec<Element<'a, Message>> {
        let mut layers: Vec<Element<'a, Message>> =
            (0..self.slots).map(|_| space().into()).collect();
        for index in self.chunks.clone() {
            layers[index % self.slots] = container(draw(&chunks[index]))
                .padding(Padding::ZERO.top(lines_height(index * CHUNK_LINES)))
                .into();
        }
        layers
    }
}

/// The height of `lines` diff lines.
fn lines_height(lines: usize) -> f32 {
    lines as f32 * layouts::DIFF_LINE_HEIGHT
}

/// One chunk's line numbers and signs.
fn gutter_chunk<'a>(c: &Colors, chunk: &'a Chunk) -> Element<'a, Message> {
    row![
        line_numbers(c, &chunk.old_numbers, OLD_NUMBER_PADDING),
        line_numbers(c, &chunk.new_numbers, NEW_NUMBER_PADDING),
        stack![
            mono(chunk.plus_signs.as_str()).color(c.added),
            mono(chunk.minus_signs.as_str()).color(c.deleted),
        ]
        .width(layouts::DIFF_SIGN_WIDTH),
    ]
    .into()
}

/// One run's stretch of the gutter background. It's opaque even for unchanged lines,
/// since the sticky gutter covers code scrolled under it.
fn gutter_run<'a>(c: &Colors, run: &Run) -> Element<'a, Message> {
    let background = match run.kind {
        Kind::Context => c.background,
        Kind::Added => c.added_gutter,
        Kind::Removed => c.removed_gutter,
    };

    container(space())
        .width(GUTTER_WIDTH)
        .height(lines_height(run.lines))
        .style(theme::background(background))
        .into()
}

/// One run's tint behind the code.
fn code_tint<'a>(c: &Colors, run: &Run, width: f32) -> Element<'a, Message> {
    let tint = match run.kind {
        Kind::Context => Color::TRANSPARENT,
        Kind::Added => c.added_tint,
        Kind::Removed => c.removed_tint,
    };

    container(space())
        .width(width)
        .height(lines_height(run.lines))
        .style(theme::background(tint))
        .into()
}

/// A column of line numbers, one per line, right-aligned.
fn line_numbers<'a>(c: &Colors, numbers: &'a str, padding_right: f32) -> Element<'a, Message> {
    container(
        mono(numbers)
            .color(c.line_number)
            .width(layouts::LINE_NUMBER_WIDTH)
            .align_x(Right),
    )
    .padding(Padding::ZERO.right(padding_right))
    .into()
}

/// Diff text: Fira Code on a fixed line height, never wrapped.
///
/// `Shaping::Advanced` is what turns on Fira Code's ligatures (`=>`, `->`, `!=` drawn as
/// one symbol, as in VS Code). iced's default, `Auto`, shapes ASCII-only text with
/// `Basic`, which skips them, and nearly all code is ASCII. A ligature keeps the width
/// of the characters it replaces, so the line-width math is unaffected.
fn mono<'a>(content: impl IntoFragment<'a>) -> Text<'a> {
    text(content)
        .font(fonts::MONO)
        .size(text_sizes::SMALL)
        .line_height(Pixels(layouts::DIFF_LINE_HEIGHT))
        .wrapping(Wrapping::None)
        .shaping(Shaping::Advanced)
}

/// A chunk's code: `mono`'s text, but one span per colored piece. A `rich_text` always
/// shapes with `Shaping::Advanced`, and the text engine only shapes a span apart from
/// its neighbours when the font changes, not the color, so ligatures still form
/// across pieces (checked in cosmic-text's `ShapeWord::new` and `Attrs::compatible`).
fn code_text(code: &ColoredText, width: f32) -> Element<'_, Message> {
    let spans: Vec<Span<'_>> = code
        .colors
        .iter()
        .map(|(range, color)| span(&code.text[range.clone()]).color_maybe(*color))
        .collect();

    rich_text(spans)
        .font(fonts::MONO)
        .size(text_sizes::SMALL)
        .line_height(Pixels(layouts::DIFF_LINE_HEIGHT))
        .wrapping(Wrapping::None)
        .width(width)
        .into()
}

/// Tabs as 4 spaces (VS Code's default), so they render predictably and the width math
/// above (one glyph = one column) holds.
fn expand_tabs(line: &str) -> Cow<'_, str> {
    if line.contains('\t') {
        Cow::Owned(line.replace('\t', "    "))
    } else {
        Cow::Borrowed(line)
    }
}

/// A diff line as it's drawn: tabs expanded, carriage returns dropped, and an invisible
/// word joiner (U+2060) between every two adjacent ASCII punctuation characters.
///
/// A `\r` left inside a line (a stray one, or a file with mixed line endings) would
/// start a new line in the text engine, and every line below it would sit one line off
/// its line numbers, since each column is one text (see `DiffView`).
///
/// The joiners make Fira Code's ligatures work for every operator. The text engine
/// (cosmic-text) shapes each run between Unicode line-break opportunities separately,
/// and there's one after `-` and `!`, so `->` and `!=` reached the font as two pieces
/// and never combined (`=>` did). A word joiner forbids the break, so the operator is
/// shaped as one. It's zero-width, so the width math is unchanged; checked by rendering
/// a line of operators with and without joiners and comparing their ends.
fn display_text(line: &str) -> String {
    let mut shown = String::with_capacity(line.len() + line.len() / 4);
    push_display_text(&mut shown, line, &mut false);
    shown
}

/// Appends `piece`, a whole line or part of one, to `shown` as `display_text` draws it.
/// `after_punctuation` says whether the line so far ends in ASCII punctuation (a
/// joiner may be due between that and `piece`), and is updated, so a line pushed in
/// pieces comes out the same as pushed whole.
fn push_display_text(shown: &mut String, piece: &str, after_punctuation: &mut bool) {
    for character in piece.chars() {
        match character {
            '\r' => {}
            '\t' => {
                shown.push_str("    ");
                *after_punctuation = false;
            }
            _ => {
                let is_punctuation = character.is_ascii_punctuation();
                if *after_punctuation && is_punctuation {
                    shown.push('\u{2060}');
                }
                shown.push(character);
                *after_punctuation = is_punctuation;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joiners_go_between_adjacent_punctuation_only() {
        assert_eq!(display_text("a -> b"), "a -\u{2060}> b");
        assert_eq!(display_text("x != y"), "x !\u{2060}= y");
        assert_eq!(display_text("<!--"), "<\u{2060}!\u{2060}-\u{2060}-");
        assert_eq!(display_text("a - b"), "a - b");
        assert_eq!(display_text("let x = 1;"), "let x = 1;");
    }

    #[test]
    fn test_tabs_are_expanded_first() {
        assert_eq!(display_text("\tx => y"), "    x =\u{2060}> y");
    }

    #[test]
    fn test_carriage_returns_are_dropped() {
        assert_eq!(display_text("a\rb\r"), "ab");
    }

    #[test]
    fn test_view_has_one_line_per_diff_line_and_runs_cover_them() {
        let diff = Diff {
            lines: vec![
                DiffLine::Context {
                    old: 1,
                    new: 1,
                    text: "same".to_string(),
                },
                DiffLine::Removed {
                    old: 2,
                    text: "old".to_string(),
                },
                DiffLine::Removed {
                    old: 3,
                    text: "older".to_string(),
                },
                DiffLine::Added {
                    new: 2,
                    text: "new".to_string(),
                },
                DiffLine::Context {
                    old: 4,
                    new: 3,
                    text: String::new(),
                },
            ],
            binary: false,
        };
        let view = DiffView::new("x", DiffSource::Unstaged, diff);

        assert_eq!((view.added, view.removed), (1, 2));
        assert_eq!(view.chunks.len(), 1);
        let chunk = &view.chunks[0];
        assert_eq!(chunk.old_numbers, "1\n2\n3\n\n4");
        assert_eq!(chunk.new_numbers, "1\n\n\n2\n3");
        assert_eq!(chunk.code.text, "same\nold\nolder\nnew\n");
        // Uncolored: one default-color span for all of it.
        assert_eq!(chunk.code.colors, vec![(0..chunk.code.text.len(), None)]);
        assert_eq!(chunk.plus_signs, "\n\n\n+\n");
        assert_eq!(chunk.minus_signs, "\n−\n−\n\n");
        let runs: Vec<_> = view.runs.iter().map(|run| run.lines).collect();
        assert_eq!(runs, vec![1, 2, 1, 1]);
    }

    /// A diff `len` lines long where the lines at `added` (indexes) are added, and the
    /// rest unchanged.
    fn diff_with_added(len: usize, added: &[usize]) -> Diff {
        let lines = (0..len)
            .map(|index| {
                let text = index.to_string();
                if added.contains(&index) {
                    DiffLine::Added { new: index, text }
                } else {
                    DiffLine::Context {
                        old: index,
                        new: index,
                        text,
                    }
                }
            })
            .collect();
        Diff {
            lines,
            binary: false,
        }
    }

    #[test]
    fn test_a_change_is_a_run_of_changed_lines() {
        // The edited lines 1–3 (removed, then added) are one change; 5 is another.
        let diff = Diff {
            lines: vec![
                DiffLine::Context {
                    old: 1,
                    new: 1,
                    text: String::new(),
                },
                DiffLine::Removed {
                    old: 2,
                    text: String::new(),
                },
                DiffLine::Removed {
                    old: 3,
                    text: String::new(),
                },
                DiffLine::Added {
                    new: 2,
                    text: String::new(),
                },
                DiffLine::Context {
                    old: 4,
                    new: 3,
                    text: String::new(),
                },
                DiffLine::Added {
                    new: 4,
                    text: String::new(),
                },
            ],
            binary: false,
        };
        let view = DiffView::new("x", DiffSource::Unstaged, diff);
        assert_eq!(view.changes, vec![1, 5]);

        // A new file is one change, from its first line; an unchanged one has none.
        let new_file = DiffView::new("x", DiffSource::Untracked, diff_with_added(3, &[0, 1, 2]));
        assert_eq!(new_file.changes, vec![0]);
        let unchanged = DiffView::new("x", DiffSource::Unstaged, diff_with_added(3, &[]));
        assert!(unchanged.changes.is_empty());
    }

    #[test]
    fn test_change_targets_step_through_changes_and_stop_at_the_ends() {
        use ChangeStep::{Next, Previous};
        // 200 lines, changed at 2 (too near the top for 3 lines of context), 50, and
        // 198 (too near the bottom to reach the top), in a viewport 10 lines tall.
        let view = DiffView::new(
            "x",
            DiffSource::Unstaged,
            diff_with_added(200, &[2, 50, 198]),
        );
        let viewport = lines_height(10);
        let bottom = lines_height(200) + CODE_PADDING_BOTTOM - viewport;
        let middle = lines_height(50 - CHANGE_CONTEXT);
        let target = |step, scroll_y| view.change_target(step, scroll_y, viewport);

        // At the top, the first change is on screen already.
        assert_eq!(target(Next, 0.0), Some(middle));
        assert_eq!(target(Previous, 0.0), None);
        assert_eq!(target(Next, middle), Some(bottom));
        // A fraction of a pixel off the change just jumped to still counts as on it.
        assert_eq!(target(Next, middle + 0.3), Some(bottom));
        assert_eq!(target(Previous, middle + 0.3), Some(0.0));
        // At the bottom, nothing is further down.
        assert_eq!(target(Next, bottom), None);
        assert_eq!(target(Previous, bottom), Some(middle));
        // Between changes: the ones on either side.
        assert_eq!(target(Previous, middle + 100.0), Some(middle));
        // A stale offset past the bottom counts as the bottom.
        assert_eq!(target(Next, bottom * 2.0), None);
        assert_eq!(target(Previous, bottom * 2.0), Some(middle));

        // A diff that fits doesn't scroll, so there's nowhere to jump to.
        let tall = lines_height(500);
        assert_eq!(view.change_target(Next, 0.0, tall), None);
        assert_eq!(view.change_target(Previous, 0.0, tall), None);
    }

    #[test]
    fn test_chunks_split_every_chunk_lines() {
        let diff = Diff {
            lines: (1..=CHUNK_LINES * 2 + 1)
                .map(|n| DiffLine::Context {
                    old: n,
                    new: n,
                    text: n.to_string(),
                })
                .collect(),
            binary: false,
        };
        let view = DiffView::new("x", DiffSource::Unstaged, diff);

        assert_eq!(view.chunks.len(), 3);
        assert!(view.chunks[1].old_numbers.starts_with("65\n"));
        assert_eq!(view.chunks[2].code.text, "129");
    }

    #[test]
    fn test_colored_lines_keep_their_joiners_and_colors() {
        let (red, blue) = (Color::from_rgb8(255, 0, 0), Color::from_rgb8(0, 0, 255));
        let mut code = ColoredText::default();
        // "\tx -> y" with "x -" red and "> y" blue: the joiner between `-` and `>`
        // goes in although they're in different pieces.
        code.push_line("\tx -> y", &[(0..4, red), (4..7, blue)]);
        code.push_newline();
        code.push_line("plain", &[]);

        assert_eq!(code.text, "    x -\u{2060}> y\nplain");
        // The same as the line pushed whole.
        assert_eq!(code.text, display_text("\tx -> y") + "\nplain");
        let blue_start = "    x -".len();
        let plain_start = code.text.len() - "plain".len();
        assert_eq!(
            code.colors,
            vec![
                (0..blue_start, Some(red)),
                // The newline joins the piece before it.
                (blue_start..plain_start, Some(blue)),
                (plain_start..code.text.len(), None),
            ]
        );
    }

    #[test]
    fn test_colors_go_to_their_chunk_and_only_for_the_shown_diff() {
        let red = Color::from_rgb8(255, 0, 0);
        let diff = Diff {
            lines: (1..=CHUNK_LINES + 1)
                .map(|n| DiffLine::Added {
                    new: n,
                    text: "ab".to_string(),
                })
                .collect(),
            binary: false,
        };
        // Only the last line (alone in the second chunk) is colored.
        let mut colors = vec![LineColors::new(); diff.lines.len()];
        colors[CHUNK_LINES] = vec![(0..2, red)];
        let mut view = DiffView::new("x.rs", DiffSource::Unstaged, diff);
        assert!(view.is_loading());

        // Colors worked out for another diff (one shown before) are dropped.
        let other = Diff {
            lines: Vec::new(),
            binary: false,
        };
        view.set_colors(&Arc::new(other), Some(&colors));
        assert_eq!(view.chunks[1].code.colors, vec![(0..2, None)]);
        assert!(view.is_loading());

        let shown = Arc::clone(&view.diff);
        view.set_colors(&shown, Some(&colors));
        assert_eq!(view.chunks[0].code.colors.len(), 1);
        assert_eq!(view.chunks[0].code.colors[0].1, None);
        assert_eq!(view.chunks[1].code.colors, vec![(0..2, Some(red))]);
        assert!(!view.is_loading());
    }

    /// A one-line Rust diff.
    fn rust_diff(text: &str) -> Diff {
        Diff {
            lines: vec![DiffLine::Added {
                new: 1,
                text: text.to_string(),
            }],
            binary: false,
        }
    }

    #[test]
    fn test_reloads_are_recognized_and_highlighting_starts_once() {
        let diff = || rust_diff("fn main() {}");
        let mut view = DiffView::new("main.rs", DiffSource::Staged, diff());

        assert!(view.shows("main.rs", DiffSource::Staged, &diff()));
        assert!(!view.shows("main.rs", DiffSource::Unstaged, &diff()));
        assert!(!view.shows("lib.rs", DiffSource::Staged, &diff()));
        assert!(!view.shows("main.rs", DiffSource::Staged, &rust_diff("")));
        assert!(view.start_highlighting(Flavor::Mocha).is_some());
        assert!(view.start_highlighting(Flavor::Mocha).is_none());
        assert!(view.is_loading());
    }

    #[test]
    fn test_binary_files_are_neither_highlighted_nor_counted() {
        let diff = Diff {
            lines: Vec::new(),
            binary: true,
        };
        // A name we'd highlight, but there are no lines.
        let view = DiffView::new("logo.rs", DiffSource::Unstaged, diff);

        assert!(!view.is_loading());
        assert!(view.is_binary());
        assert_eq!(view.counts(), None);
    }

    #[test]
    fn test_files_we_dont_highlight_are_never_loading() {
        let mut view = DiffView::new("notes.txt", DiffSource::Unstaged, rust_diff("plain"));

        assert!(!view.is_loading());
        assert!(view.start_highlighting(Flavor::Mocha).is_none());
    }

    #[test]
    fn test_a_newer_version_replaces_the_shown_one_once_highlighted() {
        let source = DiffSource::Unstaged;
        let mut view = DiffView::new("main.rs", source, rust_diff("fn old() {}"));
        let old = Arc::clone(&view.diff);
        view.set_colors(&old, None);

        view.replace_when_highlighted(DiffView::new("main.rs", source, rust_diff("fn new() {}")));
        // The old version stays on screen, while reloads compare with the new one.
        assert!(!view.is_loading());
        assert!(Arc::ptr_eq(&view.diff, &old));
        assert!(view.shows("main.rs", source, &rust_diff("fn new() {}")));
        // Highlighting starts for the new version.
        assert!(view.start_highlighting(Flavor::Mocha).is_some());

        // Stale colors (for the old version) change nothing.
        view.set_colors(&old, None);
        assert!(Arc::ptr_eq(&view.diff, &old));

        let new = Arc::clone(&view.next.as_ref().expect("waiting").diff);
        view.set_colors(&new, None);
        assert!(Arc::ptr_eq(&view.diff, &new));
        assert!(view.next.is_none());
        assert!(!view.is_loading());
    }

    #[test]
    fn test_highlighting_again_keeps_the_code_on_screen_until_it_is_done() {
        let source = DiffSource::Unstaged;
        let mut view = DiffView::new("main.rs", source, rust_diff("fn main() {}"));
        let shown = Arc::clone(&view.diff);
        view.set_colors(&shown, None);

        view.highlight_again();
        assert!(!view.is_loading());
        assert!(Arc::ptr_eq(&view.diff, &shown));
        assert!(view.start_highlighting(Flavor::Latte).is_some());

        let fresh = Arc::clone(&view.next.as_ref().expect("waiting").diff);
        view.set_colors(&fresh, None);
        assert!(Arc::ptr_eq(&view.diff, &fresh));

        // Plain text is never highlighted, so there's nothing to wait for.
        let mut plain = DiffView::new("notes.txt", source, rust_diff("plain"));
        plain.highlight_again();
        assert!(plain.next.is_none());
    }

    #[test]
    fn test_chunk_window_covers_the_viewport_plus_a_margin() {
        let chunk = lines_height(CHUNK_LINES);
        // A viewport under one chunk tall: up to 2 chunks touched, plus 1 each side.
        let window = |scroll_y: f32, count| ChunkWindow::new(scroll_y, chunk / 2.0, count);

        assert_eq!(
            window(0.0, 100),
            ChunkWindow {
                chunks: 0..4,
                slots: 4
            }
        );
        // Scrolled into chunk 10: from chunk 9 (the margin above) on.
        assert_eq!(window(chunk * 10.5, 100).chunks, 9..13);
        // Past the end (a stale offset): the last chunks.
        assert_eq!(window(chunk * 500.0, 100).chunks, 96..100);
        // A file shorter than the window.
        assert_eq!(window(0.0, 2).chunks, 0..2);
        assert_eq!(window(0.0, 0).chunks, 0..0);
    }

    /// Loads Fira Code into iced's font system, as the app does at startup.
    fn load_fira_code() {
        iced::advanced::graphics::text::font_system()
            .write()
            .expect("font system")
            .load_font(Cow::Borrowed(include_bytes!(
                "../../assets/fonts/FiraCode-Regular.ttf"
            )));
    }

    #[test]
    fn test_counted_ascii_width_matches_the_measured_one() {
        load_fira_code();
        let line = "\tlet x = a -> b != c; // <= => 0x1F";
        let measured = measure(&display_text(line));
        assert!((code_width(line) - measured).abs() < 0.01, "{measured}");
    }
}
