//! The toasts in the bottom-right corner, like VS Code's notifications. An error stays
//! until its × is clicked; a confirmation (a push or a fetch went through) closes
//! itself after `CONFIRMATION_TIME`, with a bar along its bottom counting down. Newer
//! toasts stack below the older ones, and they rise and fade in, then sink and fade
//! out while the stack closes the gap.

use std::time::{Duration, Instant};

use iced::widget::scrollable::Direction;
use iced::widget::{Space, container, keyed_column, opaque, row, scrollable, stack, text};
use iced::{Animation, Color, Element, Fill, Padding, Vector};

use super::slide::slide;
use super::{Message, changes};
use crate::theme::icons::{self, icon};
use crate::theme::{self, Colors, icon_sizes, layouts, motion, radii, text_sizes};

/// At most this many toasts are shown: a newer one past it sends an older one away, so
/// a burst of them can't fill the window.
const MOST_SHOWN: usize = 3;
/// How far a toast rises as it comes in, and sinks as it leaves: back the way it came.
const SHIFT: f32 = 12.0;
/// How long a confirmation stays before closing itself.
const CONFIRMATION_TIME: Duration = Duration::from_secs(4);
/// The countdown bar's height.
const BAR_HEIGHT: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// Something failed: stays until closed.
    Error,
    /// Something went through: closes itself.
    Done,
}

#[derive(Default)]
pub(super) struct Toasts {
    /// Oldest first. A closed toast stays here until it's done leaving.
    list: Vec<Toast>,
    next_id: u64,
}

struct Toast {
    /// What the stack is keyed by, so a toast leaving from the middle doesn't hand its
    /// place's widget state (the message's scroll, the ×'s hover) to the one after it.
    id: u64,
    kind: Kind,
    message: String,
    /// `true` while it's shown: it animates to `false` when it's closed.
    shown: Animation<bool>,
    /// When a confirmation started counting down; errors have none.
    since: Option<Instant>,
}

impl Toast {
    /// How much of a confirmation's time is left at `now`, from 1 (all of it) to 0.
    fn time_left(&self, now: Instant) -> Option<f32> {
        let since = self.since?;
        let passed = now.saturating_duration_since(since);
        Some(1.0 - (passed.as_secs_f32() / CONFIRMATION_TIME.as_secs_f32()).min(1.0))
    }
}

impl Toasts {
    /// Shows `message` in a new toast of `kind`, unless an open one already says the
    /// same thing: an open confirmation then starts its time over instead.
    pub(super) fn push(&mut self, kind: Kind, message: String, now: Instant) {
        let since = (kind == Kind::Done).then_some(now);
        if let Some(open) = self
            .list
            .iter_mut()
            .find(|toast| toast.shown.value() && toast.kind == kind && toast.message == message)
        {
            open.since = since;
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.list.push(Toast {
            id,
            kind,
            message,
            shown: Animation::new(false)
                .duration(motion::DURATION)
                .easing(motion::EASING)
                .go(true, now),
            since,
        });

        // Past the most shown, older toasts leave, confirmations first: an error
        // shouldn't go unread because a few pushes went through. The new one stays.
        let extra = self.open().count().saturating_sub(MOST_SHOWN);
        let older = || self.open().filter(|toast| toast.id != id);
        let leaving: Vec<u64> = older()
            .filter(|toast| toast.kind == Kind::Done)
            .chain(older().filter(|toast| toast.kind == Kind::Error))
            .take(extra)
            .map(|toast| toast.id)
            .collect();
        for id in leaving {
            self.dismiss(id, now);
        }
    }

    /// Closes toast `id`: it starts leaving.
    pub(super) fn dismiss(&mut self, id: u64, now: Instant) {
        if let Some(toast) = self.list.iter_mut().find(|toast| toast.id == id) {
            toast.shown.go_mut(false, now);
        }
    }

    /// Closes the confirmations whose time is up.
    pub(super) fn expire(&mut self, now: Instant) {
        for toast in &mut self.list {
            if toast.shown.value() && toast.time_left(now) == Some(0.0) {
                toast.shown.go_mut(false, now);
            }
        }
    }

    /// Lets go of the toasts that are done leaving.
    pub(super) fn prune(&mut self, now: Instant) {
        self.list
            .retain(|toast| toast.shown.value() || toast.shown.is_animating(now));
    }

    /// The stack changes on its own: a toast is coming or going, or a confirmation is
    /// counting down. The app asks for frames until it's done.
    pub(super) fn is_moving(&self, now: Instant) -> bool {
        self.list
            .iter()
            .any(|toast| toast.shown.is_animating(now) || toast.since.is_some())
    }

    fn open(&self) -> impl Iterator<Item = &Toast> {
        self.list.iter().filter(|toast| toast.shown.value())
    }

    /// The stack, drawn at `now`, or `None` with no toast to draw.
    pub(super) fn view(&self, c: &Colors, now: Instant) -> Option<Element<'_, Message>> {
        if self.list.is_empty() {
            return None;
        }

        let stack = keyed_column(self.list.iter().map(|toast| {
            // 0 gone, 1 shown, a little past 1 at the overshoot (or below 0 leaving).
            let shown = toast.shown.interpolate(0.0, 1.0, now);
            let opacity = shown.clamp(0.0, 1.0);
            // Below its place by `SHIFT` when gone: it rises from there and sinks back.
            let offset = Vector::new(0.0, SHIFT * (1.0 - shown));
            // The gap above each card is part of its place, so it closes with it.
            let place = container(card(&c.faded(opacity), opacity, toast, now))
                .padding(Padding::ZERO.top(layouts::GAP));
            // `opaque`: in a stack, the mouse passes through to the layer below wherever
            // the top layer has nothing interactive, so a click on a message's text would
            // land on the row under it. It wraps the place rather than the card, so it
            // still blocks while the card moves (`slide` hides the cursor from it then).
            (toast.id, opaque(slide(place, shown, offset)))
        }));

        // Outside the toasts, clicks go through to the window below, on purpose.
        Some(
            container(stack)
                .align_right(Fill)
                .align_bottom(Fill)
                .padding(layouts::PANEL_PADDING_X)
                .into(),
        )
    }
}

/// One toast's card at `now`, in colors faded to `opacity` (see `Colors::faded`). A
/// long message, like a hook's output, scrolls inside it.
fn card<'a>(c: &Colors, opacity: f32, toast: &'a Toast, now: Instant) -> Element<'a, Message> {
    let (icon_bytes, color, border) = match toast.kind {
        Kind::Error => (icons::CIRCLE_ALERT, c.danger, c.danger_border),
        Kind::Done => (icons::CIRCLE_CHECK, c.added, c.added_border),
    };
    let message = scrollable(
        container(text(&toast.message).size(text_sizes::SMALL).width(Fill))
            // Room for the scrollbar, which is drawn over the content.
            .padding(Padding::ZERO.right(10.0)),
    )
    .direction(Direction::Vertical(theme::thin_scrollbar()))
    .style(theme::scrollbar(c))
    .width(Fill);

    let body = container(
        row![
            // 2px down: centers the icon on the message's first line.
            container(icon(icon_bytes, icon_sizes::BUTTON, color)).padding(Padding::ZERO.top(2.0)),
            message,
            changes::icon_button(c, icons::X, Message::DismissToast(toast.id), false),
        ]
        .spacing(layouts::GAP),
    )
    .padding([10.0, 12.0]);

    // The bar is a layer over the body rather than a row under it, so a long message,
    // which fills the card's height, can't squeeze it out.
    let content: Element<'a, Message> = match toast.time_left(now) {
        Some(left) => stack![body, countdown(color, left)].into(),
        None => body.into(),
    };

    container(content)
        .width(layouts::TOAST_WIDTH)
        .max_height(layouts::TOAST_MAX_HEIGHT)
        .style(theme::toast(c, border, opacity))
        .into()
}

/// The bar along a confirmation's bottom edge, `left` (1 to 0) of its full length, which
/// shrinks toward its left end.
///
/// It runs between the card's rounded corners, not into them: iced can't clip to a
/// rounded border, and caps a quad's corner radius at half its height, so a 2px bar
/// would poke out of the corners' curve.
fn countdown<'a>(color: Color, left: f32) -> Element<'a, Message> {
    let full = layouts::TOAST_WIDTH - 2.0 * radii::CONTROL;
    container(
        container(Space::new().width(full * left).height(BAR_HEIGHT))
            .style(theme::background(color)),
    )
    // 1px up: inside the card's border, not over it.
    .padding(Padding::ZERO.left(radii::CONTROL).bottom(1.0))
    .align_bottom(Fill)
    .width(Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(toasts: &Toasts) -> Vec<&str> {
        toasts.open().map(|toast| toast.message.as_str()).collect()
    }

    #[test]
    fn the_same_message_twice_is_one_toast() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(Kind::Error, "push failed".into(), now);
        toasts.push(Kind::Error, "push failed".into(), now);
        assert_eq!(messages(&toasts), ["push failed"]);
    }

    #[test]
    fn a_closed_message_can_come_back() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(Kind::Error, "push failed".into(), now);
        toasts.dismiss(0, now);
        toasts.push(Kind::Error, "push failed".into(), now);
        assert_eq!(messages(&toasts), ["push failed"]);
        assert_eq!(toasts.list.len(), 2, "the closed one is still leaving");
    }

    #[test]
    fn past_the_most_shown_the_oldest_leaves() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        for message in ["a", "b", "c", "d"] {
            toasts.push(Kind::Error, message.into(), now);
        }
        assert_eq!(messages(&toasts), ["b", "c", "d"]);
    }

    #[test]
    fn past_the_most_shown_confirmations_leave_before_errors() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(Kind::Error, "error 1".into(), now);
        toasts.push(Kind::Done, "pushed".into(), now);
        toasts.push(Kind::Error, "error 2".into(), now);
        toasts.push(Kind::Error, "error 3".into(), now);
        assert_eq!(messages(&toasts), ["error 1", "error 2", "error 3"]);
        // The newest always shows, even if it's a confirmation.
        toasts.push(Kind::Done, "fetched".into(), now);
        assert_eq!(messages(&toasts), ["error 2", "error 3", "fetched"]);
    }

    #[test]
    fn a_confirmation_closes_itself_and_an_error_does_not() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(Kind::Error, "push failed".into(), now);
        toasts.push(Kind::Done, "fetched".into(), now);

        toasts.expire(now + CONFIRMATION_TIME / 2);
        assert_eq!(messages(&toasts), ["push failed", "fetched"]);
        toasts.expire(now + CONFIRMATION_TIME);
        assert_eq!(messages(&toasts), ["push failed"]);
    }

    #[test]
    fn the_same_confirmation_again_starts_its_time_over() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(Kind::Done, "fetched".into(), now);
        toasts.push(Kind::Done, "fetched".into(), now + CONFIRMATION_TIME / 2);

        toasts.expire(now + CONFIRMATION_TIME);
        assert_eq!(messages(&toasts), ["fetched"], "still has half its time");
        toasts.expire(now + CONFIRMATION_TIME * 3 / 2);
        assert!(messages(&toasts).is_empty());
    }

    #[test]
    fn the_bar_shrinks_from_full_to_nothing() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(Kind::Done, "fetched".into(), now);
        let toast = &toasts.list[0];
        assert_eq!(toast.time_left(now), Some(1.0));
        assert_eq!(toast.time_left(now + CONFIRMATION_TIME / 4), Some(0.75));
        assert_eq!(toast.time_left(now + CONFIRMATION_TIME * 2), Some(0.0));
    }

    #[test]
    fn a_toast_is_let_go_once_it_is_done_leaving() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push(Kind::Error, "a".into(), now);
        toasts.push(Kind::Error, "b".into(), now);
        toasts.dismiss(0, now);

        toasts.prune(now + motion::DURATION / 2);
        assert_eq!(toasts.list.len(), 2, "still leaving");
        toasts.prune(now + motion::DURATION * 2);
        assert_eq!(toasts.list.iter().map(|t| t.id).collect::<Vec<_>>(), [1]);
        assert!(!toasts.is_moving(now + motion::DURATION * 2));
    }
}
